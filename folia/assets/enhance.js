// Small behaviours shared by the pages of the site and the browser app. The site works without this
// file (it is links); before the app has taken over (`window.__betulaApp`) panels keep their scroll
// position across page loads, and a link followed while the app is starting waits for it. In both
// modes: the shortcuts (Esc closes the preview or leaves the module page, F opens the previewed
// module full screen, Ctrl+K or "/" jumps to the search; in the app M marks the module the visitor
// is at and P plans it into the Studienplan), the theme switch, the filter sheet, the swipe along
// the phone's bottom bar from tab to tab, the widths of the filter panel and the module preview
// (dragged, kept in localStorage), the room the panels make for the ground at the end of a page,
// and the way back to the top of a page („Nach oben").
(() => {
  const root = document.documentElement;
  // The page's language, as its address says it (`folia_locale::Locale::split`): the prefix of its
  // addresses and its words (docs/folia/i18n.md). The first is the default, without a prefix.
  const LANGUAGES = [
    { prefix: "", linkCopied: "Link kopiert", copied: "Kopiert" },
    { prefix: "/en", linkCopied: "Link copied", copied: "Copied" },
  ];
  const language = () => LANGUAGES.find((l) => l.prefix && (location.pathname === l.prefix || location.pathname.startsWith(l.prefix + "/"))) || LANGUAGES[0];
  const appRuns = () => window.__betulaApp === true;
  const narrow = matchMedia("(max-width: 900px)");
  const phone = () => narrow.matches;
  // Text-like controls only: a focused filter chip (checkbox) must not swallow Esc or "/".
  const typing = (el) => el && ((el.tagName === "INPUT" && !["checkbox", "radio", "button", "submit"].includes(el.type)) || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);
  // The sheet of the phone layout: the catalog's filter panel, or a page's sidebar of filters.
  // Open, the page behind it is dimmed and stands still (html.sheet-open), and a tap on it closes
  // the sheet. However it goes (its buttons, a swipe down, a tap beside it, Esc, Back), the page
  // hears `betula:sheet-closed`: the catalog's list follows what was picked in the sheet only
  // then. A sheet whose page takes what is picked there as a draft (`data-draft`: the catalog in
  // the browser app) is a step of its own in the history, as a sheet is in an app, so that Back
  // closes it instead of leaving the page; where Back does leave the page with a sheet open, the
  // page hears `betula:sheet-left`.
  const filters = () => document.getElementById("filters") || document.querySelector(".sidebar.sheet");
  let step = false; // the current history entry is the open sheet's own step
  let afterStep = null; // what follows once the browser has gone back past that step
  const openSheet = () => {
    const sheet = filters();
    if (!sheet) return;
    sheet.classList.add("open");
    root.classList.add("sheet-open");
    if (!step && phone() && sheet.hasAttribute("data-draft")) {
      history.pushState({ betulaSheet: true }, "");
      step = true;
    }
  };
  const closeSheet = (how = "closed") => {
    const sheet = filters();
    const was = sheet?.classList.contains("open");
    sheet?.classList.remove("open");
    root.classList.remove("sheet-open");
    if (!was) return;
    const tell = () => sheet.dispatchEvent(new Event("betula:sheet-" + how, { bubbles: true }));
    // The sheet's step goes before the list follows: the list's own entry takes its place.
    const own = step && history.state?.betulaSheet === true;
    step = false;
    if (own) { afterStep = tell; history.back(); } else tell();
  };
  // The app may replace the page under an open sheet (Back, a tab): nothing may stay dimmed.
  const tidySheet = () => { if (root.classList.contains("sheet-open") && !document.querySelector(".filters.open, .sidebar.sheet.open")) root.classList.remove("sheet-open"); };
  addEventListener("popstate", (e) => {
    // The browser app hands a step of the history to its router a frame later, so that the page
    // can answer first (folia/crates/app/src/pending.rs): that is the same step again, not another one.
    if (e.betulaReplay) return;
    if (afterStep) { const then = afterStep; afterStep = null; then(); } // the sheet's own step, taken back above
    else if (step) { step = false; closeSheet(); } // Back closes the sheet
    else closeSheet("left");
  });

  // ---- classic mode: scroll positions of the panels survive a page load ----
  const listKey = () => location.search.replace(/([?&])(page|open)=[^&]*/g, "$1");
  function saveScroll() {
    try {
      document.querySelectorAll("[data-keep-scroll]").forEach((el) => {
        const name = el.dataset.keepScroll;
        sessionStorage.setItem("betula.scroll." + name, JSON.stringify({ top: el.scrollTop, key: name === "rows" ? listKey() : "" }));
      });
    } catch {}
  }
  function restoreScroll() {
    try {
      document.querySelectorAll("[data-keep-scroll]").forEach((el) => {
        const name = el.dataset.keepScroll;
        if (name === "detail") return;
        const saved = JSON.parse(sessionStorage.getItem("betula.scroll." + name) || "null");
        if (saved && (name !== "rows" || saved.key === listKey())) el.scrollTop = saved.top;
      });
    } catch {}
  }
  restoreScroll();
  addEventListener("pagehide", () => { if (!appRuns()) saveScroll(); });

  // The switch between the languages (app::languages): the language it leads to is kept before the
  // page is left, so that every later visit opens in it (`language_script` in the head).
  const keepLanguage = (e) => {
    const link = e.target.closest?.("a[data-language]");
    if (!link) return;
    try { localStorage.setItem("betula.language", link.dataset.language); } catch {}
  };
  document.addEventListener("click", keepLanguage, true);
  document.addEventListener("auxclick", keepLanguage, true);

  // The app filters while typing; Enter must not load a page on top of that.
  document.addEventListener("submit", (e) => {
    if (appRuns() && e.target.matches("form[data-live-search]")) e.preventDefault();
  });

  // ---- a link followed while the app is starting ----
  // The app takes the page over a few seconds after it loads (boot.js: the catalog, the bundle,
  // sql.js; three on a phone). A link followed meanwhile loaded the next page: on a phone a blank
  // screen, and the whole start again, the catalog read anew, so a tab tapped right after opening
  // the app froze it for seconds. Now the app is waited for: a tab is current at once, and the app
  // goes where the link leads as soon as it runs (`betulaStarted`, called by boot.js). Where it
  // does not start (an error, or nothing for 6 s), the page loads as before. A link with a key
  // held, into another window, another language or out of the app is left alone, as is a place on
  // this page.
  const starting = () => window.__betulaStarting === true && !appRuns();
  let waiting = null; // { href, tab, timer }: the link followed last while the app was starting
  const follow = (started) => {
    const link = waiting;
    waiting = null;
    if (!link) return;
    clearTimeout(link.timer);
    if (!started || !appRuns()) { location.assign(link.href); return; }
    // The app's own tab (it knows where the visitor left its area), else the address.
    const tab = link.tab && document.querySelector(`.bottomnav > .nav[data-area="${link.tab}"]`);
    if (tab) { tab.click(); return; }
    const a = Object.assign(document.createElement("a"), { href: link.href, hidden: true });
    document.body.append(a);
    a.click();
    a.remove();
  };
  window.betulaStarted = (started) => follow(started);
  document.addEventListener("click", (e) => {
    if (!starting() || e.defaultPrevented || e.button !== 0 || e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) return;
    const link = e.target.closest?.("a[href]");
    if (!link || link.target || link.hasAttribute("download") || link.dataset.action || link.dataset.language || /\bexternal\b/.test(link.rel)) return;
    const url = new URL(link.href, location.href);
    const path = url.pathname.slice(language().prefix.length) || "/";
    if (url.origin !== location.origin || !url.pathname.startsWith(language().prefix) || /^\/(api|assets|pkg|access|cards|calendar|models)(\/|$)|\.[a-z0-9]{2,5}$/i.test(path)) return;
    if (url.pathname === location.pathname && url.search === location.search && url.hash) return;
    e.preventDefault();
    // A module's row on a phone is its page (as below, and as the app does).
    const row = link.matches("a.row[data-id]") && phone() ? link : null;
    const href = row ? language().prefix + "/catalog/module/" + encodeURIComponent(row.dataset.id) : url.href;
    const tab = link.matches(".bottomnav > .nav, .rail .nav") ? link.dataset.area : null;
    if (tab) {
      for (const nav of document.querySelectorAll(".bottomnav > .nav, .rail .nav")) {
        if (nav.dataset.area === tab) nav.setAttribute("aria-current", "page"); else nav.removeAttribute("aria-current");
      }
    }
    clearTimeout(waiting?.timer);
    waiting = { href, tab, timer: setTimeout(() => follow(appRuns()), 6000) };
  }, true);

  // A tap beside the open sheet, on the dimmed page (the target is then the document itself).
  document.addEventListener("click", (e) => {
    tidySheet();
    if (root.classList.contains("sheet-open") && e.target === root) { e.preventDefault(); closeSheet(); }
  }, true);

  // The sheet is swiped down to close it: it follows the finger, and is let go when it was pulled
  // far enough, or flicked; otherwise it slides back. `to` takes how far the finger is below
  // where the drag began, `since` is when the finger came down (a quick flick may reach the page
  // as one single move, whose speed is then measured from there).
  function dragSheet(sheet, since) {
    let dy = 0, at = since, speed = 0;
    sheet.classList.add("dragging");
    return {
      to(offset) {
        const now = performance.now(), next = Math.max(0, offset);
        // px per ms, mostly of the last moves: a flick at the end counts, a slow start does not.
        if (now > at) speed = 0.7 * ((next - dy) / (now - at)) + 0.3 * speed;
        dy = next;
        at = now;
        sheet.style.transform = `translateY(${dy}px)`;
      },
      release() {
        const flicked = performance.now() - at < 100 && speed > 0.45;
        sheet.style.transform = "";
        sheet.classList.remove("dragging");
        if (dy > Math.min(160, sheet.offsetHeight * 0.3) || (dy > 16 && flicked)) closeSheet();
      },
    };
  }
  const openSheetNow = () => (phone() ? document.querySelector(".filters.open, .sidebar.sheet.open") : null);
  // What scrolls between the finger and the sheet: its body, or a picker's list in it.
  const scrollerIn = (el, sheet) => {
    for (let node = el; node && node !== sheet; node = node.parentElement) {
      if (node.scrollHeight > node.clientHeight + 1 && /auto|scroll/.test(getComputedStyle(node).overflowY)) return node;
    }
    return null;
  };
  // Touches decide with their first move. Inside the sheet, one that starts downwards where
  // nothing is scrolled down (the head, the button row, the top of the list) drags the sheet;
  // everything else there scrolls the sheet's own list, which keeps the scroll to itself
  // (`overscroll-behavior`). A touch on the dimmed page moves nothing: the stylesheet takes the
  // page's scrolling away (`overflow: hidden`), this also holds browsers that scroll it anyway.
  let touch = null;
  document.addEventListener("touchstart", (e) => {
    touch = null;
    const sheet = openSheetNow();
    if (!sheet || e.touches.length !== 1) return;
    const inside = sheet.contains(e.target);
    touch = { sheet, inside, scroller: inside ? scrollerIn(e.target, sheet) : null, x: e.touches[0].clientX, y: e.touches[0].clientY, at: performance.now(), mode: null, drag: null };
  }, { passive: true });
  document.addEventListener("touchmove", (e) => {
    if (!touch) return;
    if (e.touches.length !== 1) { touch.drag?.release(); touch = null; return; }
    const dx = e.touches[0].clientX - touch.x, dy = e.touches[0].clientY - touch.y;
    if (!touch.mode) {
      const down = dy > 0 && dy >= Math.abs(dx);
      if (!touch.inside) touch.mode = "hold";
      else if (e.target.closest?.('input[type="range"]')) touch.mode = "own"; // the knobs of the credit slider
      // (A touch the browser does not let go of, one that lands in a fling, keeps scrolling.)
      else if (down && e.cancelable && !(touch.scroller?.scrollTop > 0)) { touch.mode = "drag"; touch.drag = dragSheet(touch.sheet, touch.at); }
      else touch.mode = touch.scroller ? "own" : "hold";
    }
    if (touch.mode !== "own" && e.cancelable) e.preventDefault();
    if (touch.mode === "drag") touch.drag.to(dy - 6); // a tap that wobbles does not move the sheet
  }, { passive: false });
  const endTouch = () => { touch?.drag?.release(); touch = null; };
  document.addEventListener("touchend", (e) => { if (!e.touches.length) endTouch(); });
  document.addEventListener("touchcancel", endTouch);
  // A mouse (a narrow window on a desktop) drags the sheet at its head.
  document.addEventListener("pointerdown", (e) => {
    if (e.pointerType === "touch" || e.button !== 0) return;
    const head = e.target.closest?.(".filters .panel-head, .sidebar.sheet .panel-head");
    const sheet = head?.closest(".filters, .sidebar.sheet");
    if (!sheet || sheet !== openSheetNow() || e.target.closest("a, button, input")) return;
    e.preventDefault();
    const startY = e.clientY;
    const drag = dragSheet(sheet, performance.now());
    try { sheet.setPointerCapture(e.pointerId); } catch {}
    const move = (ev) => drag.to(ev.clientY - startY);
    const stop = () => {
      sheet.removeEventListener("pointermove", move);
      sheet.removeEventListener("pointerup", stop);
      sheet.removeEventListener("pointercancel", stop);
      drag.release();
    };
    sheet.addEventListener("pointermove", move);
    sheet.addEventListener("pointerup", stop);
    sheet.addEventListener("pointercancel", stop);
  });

  // ---- the bottom bar of a phone: a swipe along it goes to the tab beside the current one ----
  // (owner, 2026-09-30: „wenn man nach links swiped soll ein tab nach links gehen und beim rechts
  // swipe eine tab nach rechts"; the next day the other way round, as a finger expects it: „die
  // ganze Leiste zu bewegen und den selector stehen zu lassen und erst wenn man los lässt geht das
  // dann wieder zur original Location zurück", and of the prototype's ways „nur tabs + ein
  // Element", design/tabbar/swipe.html). A finger that moves along the bar rather than up or down
  // takes the row of tabs with it, inside the bar, which stays; the mark of the current tab stays
  // where it is, so a finger to the left brings the tab on the right under it: as far as that tab,
  // then less and less, and less and less from the start where no tab lies that way. Let go a third
  // of the way there or further, or flicked, and that tab is clicked: the page follows as it follows
  // a tap, without a movement of its own (owner: „keine Seitenanimationen"), and the row springs
  // back to its place with the mark on that tab. Otherwise the row springs back alone. Up or down,
  // the bar scrolls the page as before.
  //
  // The mark that stays is the bar's lens (`.bottomnav-lens`, app.css, made on its first swipe),
  // laid over the current tab's mark: the mark's colour with a copy of the row inside, light, which
  // moves as the row does, so what is under the lens is light and the rest dark, cut at its edge;
  // each name turns dark as it comes close (`--near` of its tab). It stands in for the tab's own mark
  // from the first move until the tab it went to is the current one, and then the tab's own takes
  // over in the same frame. Letting go changes nothing but the force on things (one fixed curve for
  // all of it felt „ein wenig klunky"): the row springs back from where it is and as fast as it
  // went, the lens leaves from a standstill, both on one damped spring with the small swing of the
  // marks that slide (app.css --spring). The glide is Web Animations, which the compositor runs
  // while the tab's page is built, and a finger that catches it takes everything where it is.
  const TAB_SLOP = 8; // px a finger moves before it counts as a swipe or a scroll (the week's carousel: 8)
  const TAB_ROOM = 14; // px the row goes past the last tab it may bring, at most
  // The spring: how long a swing takes (s), and how much of it is damped (1: nothing goes past its
  // place). Without motion short and without the swing.
  const TAB_SPRING = { response: 0.3, damping: 0.75 };
  const TAB_CALM = { response: 0.2, damping: 0.99 };
  const TAB_DT = 1 / 120; // s from one keyframe of the glide to the next
  let tabDrag = null; // the finger on the bar
  let tabSwipe = null; // the bar while it is not at rest: its tabs, its lens, where they stand
  let tabSwiped = 0; // until when a click on the bar is the end of a swipe, not a tap
  const currentTab = (bar) => bar.querySelector(':scope > .nav[aria-current="page"]');
  // The tabs of the bar at rest, measured as a swipe begins: the left edge and the middle of each
  // tab's mark from the bar's left edge, the size of a mark, and where the row lies.
  const tabsOf = (bar) => {
    const tabs = [...bar.querySelectorAll(":scope > .nav")].filter((tab) => tab.offsetWidth > 0);
    const current = tabs.findIndex((tab) => tab.getAttribute("aria-current") === "page");
    const marks = tabs.map((tab) => tab.querySelector(".ind")?.getBoundingClientRect());
    if (current < 0 || tabs.length < 2 || marks.some((mark) => !mark)) return null;
    const box = bar.getBoundingClientRect(), first = tabs[0].getBoundingClientRect(), last = tabs[tabs.length - 1].getBoundingClientRect();
    return {
      bar,
      tabs,
      current,
      lefts: marks.map((m) => m.left - box.left),
      mids: marks.map((m) => m.left + m.width / 2 - box.left),
      step: (marks[marks.length - 1].left - marks[0].left) / (marks.length - 1),
      mark: { y: marks[0].top - box.top, w: marks[0].width, h: marks[0].height },
      row: { x: first.left - box.left, y: first.top - box.top, w: last.right - first.left, h: first.height },
    };
  };
  // The bar's lens, with a copy of the row as it is now (its counts too), none of it a link.
  const lensOf = (g) => {
    let lens = g.bar.querySelector(":scope > .bottomnav-lens");
    if (!lens) {
      lens = document.createElement("div");
      lens.className = "bottomnav-lens";
      lens.setAttribute("aria-hidden", "true");
      lens.inert = true;
      lens.append(Object.assign(document.createElement("div"), { className: "bottomnav-copy" }));
      g.bar.append(lens);
    }
    const copy = lens.firstChild;
    copy.replaceChildren(...g.tabs.map((tab) => {
      const span = document.createElement("span");
      span.className = "nav";
      span.append(...[...tab.childNodes].map((node) => node.cloneNode(true)));
      return span;
    }));
    const sizes = { "--lens-y": g.mark.y, "--lens-w": g.mark.w, "--lens-h": g.mark.h, "--copy-y": g.row.y - g.mark.y, "--copy-w": g.row.w, "--copy-h": g.row.h };
    for (const [prop, px] of Object.entries(sizes)) lens.style.setProperty(prop, px + "px");
    return { lens, copy };
  };
  const unit = (v) => Math.min(1, Math.max(0, v));
  const dots = (v) => { const dot = 1 / (devicePixelRatio || 1); return Math.round(v / dot) * dot; };
  // Each name dark as it comes close to the middle of the lens.
  const nearTabs = (s) => {
    const { g } = s, middle = s.X + g.mark.w / 2;
    g.tabs.forEach((tab, i) => {
      const near = unit(1 - Math.abs(g.mids[i] + s.T - middle) / g.step).toFixed(2);
      if (tab.style.getPropertyValue("--near") !== near) tab.style.setProperty("--near", near);
    });
  };
  // The row `T` px from its place and the lens's left edge at `X`, on whole device pixels as the
  // tabs' own marks, and the copy in the lens over the row.
  const putTabs = (s, T, X) => {
    s.T = T;
    s.X = X;
    const t = dots(T), x = dots(X);
    for (const tab of s.g.tabs) tab.style.transform = `translateX(${t}px)`;
    s.lens.style.transform = `translateX(${x}px)`;
    s.copy.style.transform = `translateX(${s.g.row.x + t - x}px)`;
    nearTabs(s);
  };
  // Past its room a pull takes the row less and less far, never past `room` px.
  const band = (over, room) => (room > 0 ? room * (1 - 1 / (1 + over / (3 * room))) : 0);
  // How many tabs a swipe may go that way (`dir` +1: to the right): one, where there is one.
  const reachOf = (g, dir) => Math.min(1, dir > 0 ? g.tabs.length - 1 - g.current : g.current);
  // Where a pull puts the lens over the row, `q` px from the current tab's middle (to the right
  // positive): as far as the tab it may reach, then held back; held back from the start where no
  // tab lies that way. `p0` is where the finger took the lens: a finger that catches the glide of a
  // quick swipe to the tab at an end of the row takes it past that tab, and the pull goes on from
  // there, held back only beyond it (held back from the tab, the row jumped against the finger).
  const lensOver = (g, q, p0) => {
    const lo = Math.min(-reachOf(g, -1) * g.step, p0), hi = Math.max(reachOf(g, 1) * g.step, p0);
    return q > hi ? hi + band(q - hi, TAB_ROOM) : q < lo ? lo - band(lo - q, TAB_ROOM) : q;
  };
  // Where a spring towards `x1` stands every TAB_DT s, let go at `x0` (px) going `v0` (px per s),
  // until it can no longer be a tenth of a px away.
  const springTo = (x0, v0, x1, { response, damping: z }) => {
    const w0 = (2 * Math.PI) / response, wd = w0 * Math.sqrt(1 - z * z), a = x0 - x1, b = (v0 + z * w0 * a) / wd;
    const out = [], far = Math.hypot(a, b);
    for (let t = 0; t < 1.5; t += TAB_DT) {
      const fade = Math.exp(-z * w0 * t);
      out.push(x1 + fade * (a * Math.cos(wd * t) + b * Math.sin(wd * t)));
      if (far * fade < 0.1) break;
    }
    out.push(x1);
    return out;
  };
  const sampleAt = (values, k) => {
    const i = Math.min(Math.floor(k), values.length - 1), j = Math.min(i + 1, values.length - 1);
    return values[i] + (values[j] - values[i]) * unit(k - i);
  };
  const stopGlide = (glide) => {
    cancelAnimationFrame(glide.frame);
    clearTimeout(glide.timer);
    for (const anim of glide.anims) anim.cancel();
  };
  // A finger takes the bar on its way: where the row and the lens stand just then, the row put back
  // at rest to be measured again.
  const catchTabs = (s) => {
    const glide = s.glide, k = (glide.anims[0].currentTime ?? 0) / 1000 / TAB_DT;
    stopGlide(glide);
    s.glide = null;
    for (const tab of s.g.tabs) tab.style.removeProperty("transform");
    return { T: sampleAt(glide.Ts, k), X: sampleAt(glide.Xs, k) };
  };
  // At rest: the tab's own mark takes over from the lens, in one frame, without its fade.
  const settleTabs = () => {
    const s = tabSwipe;
    if (!s) return;
    tabSwipe = null;
    if (s.glide) stopGlide(s.glide);
    const { bar } = s;
    bar.dataset.swipe = "settle";
    for (const tab of s.g.tabs) {
      tab.style.removeProperty("transform");
      tab.style.removeProperty("--near");
    }
    s.lens.style.removeProperty("transform");
    s.copy.style.removeProperty("transform");
    s.copy.replaceChildren();
    requestAnimationFrame(() => requestAnimationFrame(() => { if (bar.dataset.swipe === "settle") delete bar.dataset.swipe; }));
  };
  // The finger takes the bar: at rest, or on its way back where the row and the lens stand just
  // then (the tab it went to being the current one). `null` where the bar has nothing to swipe.
  const takeTabs = (bar) => {
    let s = tabSwipe, caught = null;
    if (s?.glide && s.bar === bar) caught = catchTabs(s);
    else {
      settleTabs();
      s = null;
    }
    const g = tabsOf(bar);
    if (!g) { settleTabs(); return null; }
    if (s && g.tabs.includes(s.target)) g.current = g.tabs.indexOf(s.target);
    const next = { bar, g, ...lensOf(g), T: 0, X: 0, glide: null, target: g.tabs[g.current] };
    tabSwipe = next;
    bar.dataset.swipe = "drag";
    putTabs(next, caught ? caught.T : 0, caught ? caught.X : g.lefts[g.current]);
    return next;
  };
  // The finger at `x` at the time `at` (the event's): the row follows (a finger that wobbles moves
  // nothing). How fast the finger goes is kept in px per ms, mostly of the last moves, as the sheet
  // measures a flick, and where the row was when, for how fast it goes once the finger has gone.
  // The events' own times: a page busy with the tab it goes to hands on several moves at once.
  const followTab = (d, x, at) => {
    const { s } = d, dx = x - d.x, pull = dx > TAB_SLOP ? dx - TAB_SLOP : dx < -TAB_SLOP ? dx + TAB_SLOP : 0;
    if (pull !== d.pull) {
      if (at > d.at) d.speed = 0.7 * ((pull - d.pull) / (at - d.at)) + 0.3 * d.speed;
      d.pull = pull;
      d.at = at;
      d.p = lensOver(s.g, d.p0 - pull, d.p0);
      putTabs(s, d.X + s.g.mark.w / 2 - s.g.mids[s.g.current] - d.p, d.X);
    }
    d.trail.push({ at, T: s.T });
    while (d.trail.length > 2 && at - d.trail[0].at > 100) d.trail.shift();
  };
  // The tab the lens goes to: the one the finger took it a third of the way to, the way the finger
  // went (a sixth when flicked, the first one from 12 px on); where it went nowhere, the nearest.
  const decideTab = (d, at) => {
    const { g } = d.s, way = Math.sign(d.p - d.p0), v = -d.speed;
    const flicked = way !== 0 && at - d.at < 100 && Math.sign(v) === way && Math.abs(v) > 0.45;
    const s = d.p / g.step, s0 = d.p0 / g.step, lead = flicked ? 5 / 6 : 2 / 3;
    let n = way > 0 ? Math.floor(s + lead) : way < 0 ? Math.ceil(s - lead) : Math.round(s);
    if (flicked && Math.abs(d.p - d.p0) >= 12) n = way > 0 ? Math.max(n, Math.floor(s0) + 1) : Math.min(n, Math.ceil(s0) - 1);
    return g.current + Math.max(-reachOf(g, -1), Math.min(reachOf(g, 1), n));
  };
  // How fast the row went at the end (px per ms), over its last 80 ms: nothing if it was held.
  const rowSpeed = (trail, at) => {
    const recent = trail.filter((t) => at - t.at <= 80), a = recent[0], b = recent[recent.length - 1];
    return recent.length > 1 && b.at - a.at > 4 ? Math.max(-1, Math.min(1, (b.T - a.T) / (b.at - a.at))) : 0;
  };
  // Let go: the row springs back to its place from where it is and as fast as it went, the lens to
  // the tab `to` from a standstill, and the copy with both; keyframes every TAB_DT for the
  // compositor, the names follow frame by frame. The tab's own link is clicked as the glide begins,
  // as a tap clicks it: the app takes it (R21: the tab is current at once, its page comes a frame
  // later, `Pending`), before the app the browser loads its page. Not a frame later: a tap right
  // after the swipe, on the tab it left, would then reach the app while the router is still there,
  // change nothing, and be undone by the swipe's page. Once the glide is over and that tab the
  // current one (before the app with the next page, which takes as long as it takes: a few seconds
  // at most, then the lens gives up), the tab's own mark takes over.
  const glideTabs = (s, to, v) => {
    const { g } = s, calm = matchMedia("(prefers-reduced-motion: reduce)").matches, spring = calm ? TAB_CALM : TAB_SPRING;
    const Ts = springTo(s.T, calm ? 0 : v * 1000, 0, spring), Xs = springTo(s.X, 0, g.lefts[to], spring);
    while (Ts.length < Xs.length) Ts.push(0);
    while (Xs.length < Ts.length) Xs.push(g.lefts[to]);
    const t = Ts.map(dots), x = Xs.map(dots), frames = (values) => values.map((value) => ({ transform: `translateX(${value}px)` }));
    const timing = { duration: (Ts.length - 1) * TAB_DT * 1000, easing: "linear", fill: "forwards" }, row = frames(t);
    s.bar.dataset.swipe = "glide";
    s.target = g.tabs[to];
    const anims = [...g.tabs.map((tab) => tab.animate(row, timing)), s.lens.animate(frames(x), timing), s.copy.animate(frames(t.map((value, k) => g.row.x + value - x[k])), timing)];
    const glide = s.glide = { Ts, Xs, anims, began: performance.now(), frame: 0, timer: 0 };
    if (to !== g.current) s.target.click();
    // At rest once that tab is the current one, or another (tapped meanwhile), or the bar is gone.
    const over = () => {
      if (tabSwipe !== s || s.glide !== glide) return;
      const now = currentTab(s.bar);
      if (!s.bar.isConnected || now === s.target || (now && now !== g.tabs[g.current]) || performance.now() - glide.began > 4000) settleTabs();
      else glide.timer = setTimeout(over, 100);
    };
    const tick = () => {
      if (tabSwipe !== s || s.glide !== glide) return;
      const k = (anims[0].currentTime ?? 0) / 1000 / TAB_DT;
      s.T = sampleAt(Ts, k);
      s.X = sampleAt(Xs, k);
      nearTabs(s);
      if (k < Ts.length - 1) glide.frame = requestAnimationFrame(tick);
      else over();
    };
    glide.frame = requestAnimationFrame(tick);
    // A page whose frames have stopped (hidden) still comes to rest.
    glide.timer = setTimeout(over, timing.duration + 400);
  };
  // The finger goes: lifted at `x` at the time `at`, or taken by the browser (`x` null: a pinch, a
  // scroll, a gesture of the system), which goes back to the tab it came from.
  const letGoTab = (x, at) => {
    const d = tabDrag;
    tabDrag = null;
    if (!d?.s) return; // a tap, the link's own (and a glide under it goes on)
    tabSwiped = performance.now() + 400;
    try { d.bar.releasePointerCapture(d.id); } catch {}
    if (tabSwipe !== d.s) return;
    if (!d.bar.isConnected) { settleTabs(); return; }
    let to = d.s.g.current, v = 0;
    if (x !== null) {
      followTab(d, x, at);
      to = decideTab(d, at);
      v = rowSpeed(d.trail, at);
    }
    glideTabs(d.s, to, v);
  };
  document.addEventListener("pointerdown", (e) => {
    if (!e.isPrimary) return;
    letGoTab(null); // a finger the page never heard go
    tabSwiped = 0; // a new touch: its click is a tap
    const bar = e.button === 0 ? e.target.closest?.(".bottomnav") : null;
    if (!bar) return;
    // Before the app the lens on its way waits for the tab's page, and until it comes the bar takes
    // taps only. In the app a finger catches the glide.
    const s = tabSwipe;
    if (s?.glide && s.bar === bar && bar.isConnected && !appRuns() && currentTab(bar) !== s.target) return;
    tabDrag = { bar, id: e.pointerId, x: e.clientX, y: e.clientY, at: e.timeStamp, speed: 0, pull: 0, s: null };
  });
  document.addEventListener("pointermove", (e) => {
    const d = tabDrag;
    if (!d || e.pointerId !== d.id) return;
    if (!(e.buttons & 1)) { letGoTab(null); return; } // let go where the page did not hear it
    if (!d.s) {
      const dx = e.clientX - d.x, dy = e.clientY - d.y;
      if (Math.abs(dx) < TAB_SLOP && Math.abs(dy) < TAB_SLOP) return;
      // The first move past the slop decides: along the bar the finger has the row, else the page.
      d.s = Math.abs(dx) > Math.abs(dy) && d.bar.isConnected ? takeTabs(d.bar) : null;
      if (!d.s) { tabDrag = null; return; }
      const { g } = d.s;
      // Where the lens stands, and where over the row from the current tab's middle.
      d.X = d.s.X;
      d.p0 = d.p = d.s.X + g.mark.w / 2 - d.s.T - g.mids[g.current];
      d.trail = [{ at: d.at, T: d.s.T }];
      try { d.bar.setPointerCapture(d.id); } catch {}
    }
    followTab(d, e.clientX, e.timeStamp);
  });
  document.addEventListener("pointerup", (e) => { if (e.pointerId === tabDrag?.id) letGoTab(e.clientX, e.timeStamp); });
  document.addEventListener("pointercancel", (e) => { if (e.pointerId === tabDrag?.id) letGoTab(null); });
  // The moves of a swipe are the bar's alone. Left to the browser, a quick one ends in a fling of
  // nothing (the bar lets the browser pan up and down only), and the next tap anywhere, up to a
  // second later, only stops that fling instead of being a tap.
  document.addEventListener("touchmove", (e) => { if (tabDrag?.s && e.cancelable) e.preventDefault(); }, { passive: false });
  // Whatever the browser makes of a finger that swiped, it is no tap; a tap while the lens is on
  // its way ends the glide first. And a link dragged with a mouse (a narrow window on a desktop)
  // would leave the page's hands, the browser's own drag cancels the pointer.
  document.addEventListener("click", (e) => {
    if (!e.isTrusted || !e.target.closest?.(".bottomnav")) return;
    if (performance.now() < tabSwiped) e.preventDefault();
    else if (tabSwipe?.glide) settleTabs();
  }, true);
  document.addEventListener("dragstart", (e) => { if (e.target.closest?.(".bottomnav")) e.preventDefault(); });
  // A page the browser kept (Back after a page load) shows its own tab's mark again.
  addEventListener("pageshow", (e) => { if (e.persisted) { tabDrag = null; settleTabs(); } });

  document.addEventListener("click", (e) => {
    const target = e.target.closest("[data-action]");
    switch (target?.dataset.action) {
      case "theme": {
        const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
        root.dataset.theme = dark ? "light" : "dark";
        // The browser's own chrome follows the chosen theme, not the system's (the head script
        // does the same on load; the colours are --bg of the two themes).
        for (const meta of document.querySelectorAll("meta[name=theme-color]")) meta.content = dark ? "#f1f2f4" : "#0a0c11";
        try { localStorage.setItem("betula.theme", root.dataset.theme); } catch {}
        break;
      }
      case "sheet-open":
        e.preventDefault();
        openSheet();
        break;
      case "sheet-close":
        e.preventDefault();
        closeSheet();
        break;
      case "back": {
        // The link leads to the list of the area. If that list is where the visitor came from,
        // go back through the history instead: the same entry as before, and no new one. The app
        // knows and says so (`data-back`); the classic site can only ask the referrer.
        const cameFromIt = appRuns() ? target.dataset.back === "history" : document.referrer.startsWith(location.origin);
        if (cameFromIt && history.length > 1) { e.preventDefault(); history.back(); }
        break;
      }
      case "jump": {
        // To a section of the page, without a history entry (Esc and „Zurück" still leave the page).
        const section = document.getElementById(target.getAttribute("href").slice(1));
        if (!section) break;
        e.preventDefault();
        // The panels drawn only near the screen (app.css, `content-visibility`) are drawn for the
        // way there: with only their guessed heights the glide ended off the section.
        root.classList.add("jumping");
        const done = () => root.classList.remove("jumping");
        addEventListener("scrollend", done, { once: true });
        setTimeout(done, 2000);
        section.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "start" });
        break;
      }
      case "search": {
        // Into the search at the top, as Ctrl+K and / do (the way in of the start page). Without
        // this, or with a modifier, the link opens the catalog, where the search is too.
        const search = document.getElementById("topsearch");
        if (!search || e.button !== 0 || e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) break;
        e.preventDefault();
        search.focus();
        search.select();
        break;
      }
      case "to-top":
        toTop(target);
        break;
      case "copy-link":
      case "copy-text": {
        // The address of the page, or what the control carries (`data-text`: the marked modules
        // as text, or with `data-absolute` a path of this site that becomes a whole address).
        e.preventDefault();
        const label = target.querySelector("[data-label]") || target.querySelector("span");
        const link = target.dataset.action === "copy-link";
        const carried = target.dataset.text || "";
        const text = link ? location.href : "absolute" in target.dataset ? new URL(carried, location.origin).href : carried;
        navigator.clipboard?.writeText(text).then(() => {
          if (!label || label.dataset.was) return;
          label.dataset.was = label.textContent;
          label.textContent = link ? language().linkCopied : language().copied;
          setTimeout(() => { label.textContent = label.dataset.was; delete label.dataset.was; }, 1600);
        }).catch(() => {});
        break;
      }
    }
  });

  // ---- widths of the filter panel and the module preview: drag the edge, arrow keys on the
  // focused edge, a double click resets. Personal, so kept in localStorage and not in the URL.
  //
  // The page follows the handle live (owner, 2026-09-20: that is how it has to feel): the width
  // is a custom property on <html>, written at most once a frame. Every write lays out the whole
  // page, which is fine as long as the page keeps up. Where it does not (three frames in a row
  // far over budget: a very long list, a slow machine), the rest of that drag moves only the
  // panel itself, above its neighbour, and the page is laid out once when the handle is let go.
  // Pages have to be cheap to lay out for this to stay live; `e2e/resize-perf.mjs` measures it.
  // `data-resize-budget` on <html> sets the budget in ms (the tests use it to force the fallback).
  const sideWidth = () => (document.getElementById("filters") || document.getElementById("sidebar"))?.getBoundingClientRect().width ?? 272;
  const RESIZE = {
    "resize-filters": {
      key: "betula.filters.width", prop: "--w-filters", grows: 1,
      min: () => 232, max: () => Math.max(232, Math.min(440, innerWidth * 0.42)),
      panel: () => document.getElementById("filters") || document.getElementById("sidebar"),
      place: (handle, width) => { if (width === null) handle.style.removeProperty("--at"); else handle.style.setProperty("--at", width + "px"); },
    },
    "resize-preview": {
      key: "betula.preview.width", prop: "--preview-w", grows: -1,
      min: () => 360, max: () => { const work = document.querySelector(".work"); return work ? Math.max(360, work.clientWidth - sideWidth() - 128) : 2400; },
      panel: () => document.querySelector(".work > .detail"),
      place: (handle, width) => { if (width === null) handle.style.removeProperty("--at"); else handle.style.setProperty("--at", width + "px"); },
    },
  };
  const resizerOf = (target) => {
    const handle = target.closest ? target.closest("[data-action^='resize-']") : null;
    return handle && RESIZE[handle.dataset.action] ? { handle, edge: RESIZE[handle.dataset.action] } : null;
  };
  const within = (edge, px) => Math.round(Math.min(edge.max(), Math.max(edge.min(), px)));
  const setWidth = (edge, px) => {
    const width = within(edge, px);
    root.style.setProperty(edge.prop, width + "px");
    try { localStorage.setItem(edge.key, String(width)); } catch {}
  };
  document.addEventListener("pointerdown", (e) => {
    const hit = e.button === 0 ? resizerOf(e.target) : null;
    if (!hit) return;
    e.preventDefault();
    const { edge, handle } = hit;
    const panel = edge.panel();
    if (!panel) return;
    const startWidth = panel.getBoundingClientRect().width;
    const startX = e.clientX; // the grabbed point stays under the pointer
    root.classList.add("resizing");
    root.dataset.resizeMode = "live";
    const budget = Number(root.dataset.resizeBudget) || 34; // ms; a frame at 60 Hz has 16.7
    let latest = startWidth, applied = null, live = true, wrote = false, slow = 0, previous = 0, loop = 0;
    const tick = (now) => {
      // How long the frame after a write took tells whether the page keeps up.
      if (live && wrote && previous) {
        slow = now - previous > budget ? slow + 1 : 0;
        if (slow >= 3) { live = false; root.dataset.resizeMode = "panel"; }
      }
      previous = now;
      wrote = false;
      const width = within(edge, latest);
      if (width !== applied) {
        applied = width;
        wrote = true;
        if (live) root.style.setProperty(edge.prop, width + "px");
        else { panel.style.width = width + "px"; edge.place(handle, width); }
      }
      loop = requestAnimationFrame(tick);
    };
    loop = requestAnimationFrame(tick);
    const move = (ev) => { latest = startWidth + edge.grows * (ev.clientX - startX); };
    const stop = (ev) => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", stop);
      removeEventListener("pointercancel", stop);
      cancelAnimationFrame(loop);
      if (ev.type === "pointerup") latest = startWidth + edge.grows * (ev.clientX - startX);
      panel.style.width = "";
      edge.place(handle, null);
      root.classList.remove("resizing");
      delete root.dataset.resizeMode;
      setWidth(edge, latest);
    };
    addEventListener("pointermove", move);
    addEventListener("pointerup", stop);
    addEventListener("pointercancel", stop);
  });
  document.addEventListener("dblclick", (e) => {
    const hit = resizerOf(e.target);
    if (!hit) return;
    root.style.removeProperty(hit.edge.prop);
    try { localStorage.removeItem(hit.edge.key); } catch {}
  });

  // ---- the list by keyboard: arrows move through the rows, Enter opens the focused one ----
  // Rows are links, so moving the focus is all it takes: Enter then follows the link natively.
  function moveInList(step) {
    const rows = [...document.querySelectorAll(".rows a.row")];
    if (!rows.length) return false;
    const active = document.activeElement;
    let index = rows.indexOf(active);
    if (index < 0) index = rows.findIndex((row) => row.getAttribute("aria-current") === "true");
    const next = index < 0 ? (step > 0 ? 0 : rows.length - 1) : Math.min(rows.length - 1, Math.max(0, index + step));
    rows[next].focus({ preventScroll: true });
    rows[next].scrollIntoView({ block: "nearest" });
    return true;
  }

  // ---- a table of contents follows the page (`<nav data-spy>`, the start page) ----
  // The section whose top has passed the upper part of the scroller is the one the page is at; at
  // the very end it is the last one, however short it is. Scroll events do not bubble, so they are
  // caught on their way down; the work happens once per frame.
  let spyFrame = 0;
  const spy = () => {
    spyFrame = 0;
    const nav = document.querySelector("nav[data-spy]");
    const scroller = document.getElementById("page-scroll");
    if (!nav || !scroller) return;
    const links = [...nav.querySelectorAll('a[href^="#"]')];
    const line = scroller.getBoundingClientRect().top + 96;
    let current = links[0];
    for (const link of links) {
      const section = document.getElementById(link.getAttribute("href").slice(1));
      if (section && section.getBoundingClientRect().top <= line) current = link;
    }
    if (scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2) current = links[links.length - 1];
    for (const link of links) {
      if (link === current) link.setAttribute("aria-current", "location");
      else link.removeAttribute("aria-current");
    }
  };
  document.addEventListener("scroll", () => { if (!spyFrame) spyFrame = requestAnimationFrame(spy); }, { capture: true, passive: true });

  // ---- the ground: the footer after the end of a page ----
  // On a wide screen every page is one scroll area (`.work.flowing`, app.css „one scroll area")
  // with the ground as its end: the browser scrolls page and ground as one (owner, 2026-09-29: the
  // two steps before — first the page, then the window for the ground — felt „unfassbar janky").
  // A phone scrolls page and ground with the window. What is left to do is below.
  const flowing = () => document.querySelector("#content > .work.flowing");
  const glide = () => (matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth");

  // ---- one scroll area (`.work.flowing`; app.css „one scroll area") ----
  // The page and the ground after it scroll natively as one; nothing here runs while it scrolls
  // but the wood where the browser cannot tie it to the scroll itself. Once the area stands still
  // (150 ms), the pinned panels' content ends above the ground (`--cover`: the ground covers
  // exactly what goes, so nobody sees it) and „Nach oben" stands above it. As the ground goes
  // back down their content follows it in the same frame (owner, 2026-09-30: once the ground was
  // gone the sidebar still waited for the scroll to end): growing under the ground shows nothing
  // that is not to be seen. The heads of the list's columns stay under the list's head: its height
  // is `--list-head-h`. The scrollbar's width is `--bar`: it stands in the gap right of the page.
  // A phone scrolls the window, and none of this applies there (app.css has all of it for wide
  // screens only): it measured every page as it was built, a layout in each of its frames, and gave
  // each new page `--bar` and `--cover`, which restyled the whole page once more; a quarter of what
  // a tab switch cost a phone. `--bar` there is the top bar's height, too, and `0px` on the page took
  // it from the skeleton of the rows (app.css, `.rows-pending`).
  const tiesWood = CSS.supports("animation-timeline: scroll()") && CSS.supports("timeline-scope: --page");
  let areaTimer = 0, areaFrame = 0, woodMoved = false, headSeen = null, covered = 0, coveredArea = null, areaOn = false;
  const headWatch = new ResizeObserver(([entry]) => {
    // Its height as drawn, with the fraction a scaled screen gives it: rounded, the heads of the
    // columns stood a pixel off it and the rows showed through.
    const area = flowing(), height = entry?.borderBoxSize?.[0]?.blockSize ?? entry?.target.getBoundingClientRect().height;
    if (area && height) area.style.setProperty("--list-head-h", height + "px");
  });
  // How much of the pinned panels the ground covers: from 8 px above it to their end.
  const coverOf = (area) => {
    const ground = area.querySelector(":scope > .ground");
    return ground && !phone() ? Math.max(0, Math.round(area.getBoundingClientRect().bottom - 1 - (ground.getBoundingClientRect().top - 8))) : 0;
  };
  // Written only when it changes: a custom property on the page restyles all of it.
  const putCover = (area, cover) => {
    if (area === coveredArea && cover === covered) return;
    covered = cover;
    coveredArea = area;
    area.style.setProperty("--cover", cover + "px");
    const button = document.getElementById("to-top");
    if (cover) button?.style.setProperty("--lift", cover + "px"); else button?.style.removeProperty("--lift");
  };
  const areaSettle = () => {
    areaTimer = 0;
    if (phone()) return;
    const area = flowing();
    if (!area) { covered = 0; coveredArea = null; document.getElementById("to-top")?.style.removeProperty("--lift"); return; }
    putCover(area, coverOf(area));
  };
  // The window became a phone's (a narrow window, a turned tablet): what the area was given goes.
  const areaOff = () => {
    areaOn = false;
    cancelAnimationFrame(areaFrame);
    clearTimeout(areaTimer);
    areaFrame = areaTimer = 0;
    headWatch.disconnect();
    headSeen = null;
    const area = flowing();
    for (const prop of ["--bar", "--cover", "--list-head-h"]) area?.style.removeProperty(prop);
    area?.querySelector(":scope > .list.short")?.classList.remove("short");
    document.getElementById("to-top")?.style.removeProperty("--lift");
    covered = 0;
    coveredArea = null;
    if (woodMoved) { document.querySelector(".wood")?.style.removeProperty("transform"); woodMoved = false; }
  };
  const areaFollow = () => {
    areaFrame = 0;
    if (phone()) return;
    const area = flowing();
    const head = area?.querySelector(":scope > .list > .list-head") ?? null;
    if (head !== headSeen) { headWatch.disconnect(); if (head) headWatch.observe(head); headSeen = head; }
    if (area) {
      const bar = area.offsetWidth - area.clientWidth;
      if (area.style.getPropertyValue("--bar") !== bar + "px") area.style.setProperty("--bar", bar + "px");
      if (covered) { const cover = coverOf(area); if (cover < covered) putCover(area, cover); }
    }
    // A list short enough to stand whole above the ground is pinned like the filter panel: the
    // ground slides over its empty end and its rows stay where they are, as before. A longer one
    // flows with the page and its end comes before the ground.
    const list = area?.querySelector(":scope > .list");
    if (list) {
      const rows = list.querySelector(".rows"), last = rows?.lastElementChild;
      const needs = last ? last.getBoundingClientRect().bottom - list.getBoundingClientRect().top + 12 : 0;
      const short = needs <= innerHeight - 64 - 208;
      if (list.classList.contains("short") !== short) list.classList.toggle("short", short);
    }
    if (tiesWood) return;
    const wood = document.querySelector(".wood");
    if (!wood) return;
    if (!area) { if (woodMoved) { wood.style.removeProperty("transform"); woodMoved = false; } return; }
    const reach = 208, rise = Math.min(reach, Math.max(0, area.scrollTop - (area.scrollHeight - area.clientHeight - reach)));
    wood.style.transform = rise ? `translateY(${-rise}px)` : "";
    woodMoved = true;
  };
  const areaSoon = () => {
    if (phone()) { if (areaOn) areaOff(); return; }
    areaOn = true;
    if (!areaFrame) areaFrame = requestAnimationFrame(areaFollow);
    clearTimeout(areaTimer);
    areaTimer = setTimeout(areaSettle, 150);
  };
  document.addEventListener("scroll", (e) => { if (!phone() && e.target === flowing()) areaSoon(); }, { capture: true, passive: true });
  addEventListener("resize", areaSoon);
  new MutationObserver(areaSoon).observe(document.body, { childList: true, subtree: true });
  areaSoon();

  // ---- back to the top („Nach oben", `ui::ToTop`) ----
  // Once the page is more than a screen down, a button in its corner takes it back to its top
  // (owner, 2026-09-26: after a while in the catalog's list it was hard to get back up). The page
  // is its scroll area on a wide screen, the window on a phone. The way up jumps to a
  // screen above the top and glides the rest, a frame at a time, written here: the catalog's list
  // is virtual, and the browser's own smooth scrolling over it would build every row it passes
  // and end where the list makes up for a row that turned out taller than it was taken for (the
  // list scrolls by the difference, and a script's scroll ends a smooth one). Each frame here puts
  // the page where the glide is, whatever the list did in between, so it arrives at the top. A
  // wheel, a touch, a click or a key stops it; where less motion is wanted the page is up at once.
  const pageScroller = () => (phone() ? document.scrollingElement : flowing());
  let topFrame = 0;
  let topPage = null; // the page and the button it was last worked out for
  let topButton = null;
  const showTop = () => {
    topFrame = 0;
    topPage = pageScroller();
    topButton = document.getElementById("to-top");
    if (!topButton) return;
    const far = Boolean(topPage) && topPage.scrollTop > topPage.clientHeight;
    if (topButton.hasAttribute("data-shown") !== far) topButton.toggleAttribute("data-shown", far);
  };
  const showTopSoon = () => { if (!topFrame) topFrame = requestAnimationFrame(showTop); };
  document.addEventListener("scroll", showTopSoon, { capture: true, passive: true });
  addEventListener("resize", showTopSoon);
  // The app's pages replace each other, and the app takes the page over with a button of its own.
  // Another page or another button starts at the page's top, where the button is hidden; a page
  // that comes back further down is scrolled there, and that scroll shows it. Asked where it is
  // right away, a page being built was laid out ahead of time, in each of its frames (at the
  // start of the app, the whole page twice); nothing else that changes (rows coming and going, the
  // skeleton in the frame after a click) makes the page say where it is either.
  new MutationObserver(() => {
    const page = pageScroller(), button = document.getElementById("to-top");
    if (page === topPage && button === topButton) return;
    topPage = page;
    topButton = button;
    if (button?.hasAttribute("data-shown") && !topFrame) button.removeAttribute("data-shown");
  }).observe(document.body, { childList: true, subtree: true });
  // A page that loads further down (a reload) or comes back from the browser's memory.
  addEventListener("load", showTopSoon);
  addEventListener("pageshow", (e) => { if (e.persisted) showTopSoon(); });
  // The button stands over the page but is no part of what scrolls: the wheel over it turns the
  // page under it, as over the page itself. A phone scrolls the window anyway.
  document.addEventListener("wheel", (e) => {
    if (e.ctrlKey || !e.target.closest?.("#to-top")) return;
    const page = pageScroller();
    if (!page || page === document.scrollingElement) return;
    const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaMode === 2 ? e.deltaY * page.clientHeight : e.deltaY;
    page.scrollBy({ top: dy, behavior: Math.abs(dy) >= 50 ? glide() : "instant" });
  }, { passive: true });
  let rising = 0; // the next frame of a glide to the top
  const STOP_RISING = ["wheel", "touchstart", "pointerdown", "keydown"];
  const stopRising = () => {
    if (!rising) return;
    cancelAnimationFrame(rising);
    rising = 0;
    for (const type of STOP_RISING) removeEventListener(type, stopRising, true);
  };
  function toTop(button) {
    const page = pageScroller();
    if (!page) return;
    stopRising();
    const put = (y) => { if (page === document.scrollingElement) window.scrollTo(0, y); else page.scrollTop = y; };
    // A keyboard goes on from the start of the page, where „Zum Inhalt springen" leads, and not
    // from the button, which is gone at the top.
    if (button.matches(":focus-visible")) {
      const content = document.getElementById("content");
      content?.setAttribute("tabindex", "-1");
      content?.addEventListener("blur", () => content.removeAttribute("tabindex"), { once: true });
      content?.focus({ preventScroll: true });
    }
    const from = Math.min(page.scrollTop, page.clientHeight);
    if (glide() === "instant") { put(0); return; }
    put(from);
    const start = performance.now();
    const step = (now) => {
      const t = Math.min(1, (now - start) / 320);
      put(Math.round(from * (1 - t) ** 3));
      if (t < 1) rising = requestAnimationFrame(step);
      else stopRising();
    };
    rising = requestAnimationFrame(step);
    for (const type of STOP_RISING) addEventListener(type, stopRising, { capture: true, passive: true });
  }

  // ---- shortcuts (each is written next to its button) ----
  addEventListener("keydown", (e) => {
    // An open picker has the keyboard to itself (its Esc closes the picker, nothing else).
    if (e.target.closest?.(".combo[data-open]")) return;
    // The filter toggles are links that act as switches: the space bar flips them, too.
    if (e.key === " " && e.target.matches?.('a[role="checkbox"], a[role="radio"]')) { e.preventDefault(); e.target.click(); return; }
    if ((e.key === "ArrowDown" || e.key === "ArrowUp") && !e.ctrlKey && !e.metaKey && !e.altKey) {
      // Only where arrows have no other job: not in fields, not in the filter panel.
      const active = document.activeElement;
      const free = !typing(active) && !active?.closest?.("#filters");
      if (free && moveInList(e.key === "ArrowDown" ? 1 : -1)) { e.preventDefault(); return; }
    }
    const hit = e.key === "ArrowLeft" || e.key === "ArrowRight" ? resizerOf(e.target) : null;
    if (hit) {
      e.preventDefault();
      const step = (e.key === "ArrowRight" ? 24 : -24) * hit.edge.grows;
      const panel = hit.edge.panel();
      if (panel) setWidth(hit.edge, panel.getBoundingClientRect().width + step);
      return;
    }
    const plain = !e.ctrlKey && !e.metaKey && !e.altKey;
    if (plain && e.key === "Escape") {
      if (filters()?.classList.contains("open")) { closeSheet(); return; }
      if (typing(document.activeElement)) { document.activeElement.blur(); return; }
      // The preview first; on a module's own page Esc goes back to where the visitor came from.
      (document.querySelector('[data-action="close-detail"]') || document.querySelector('[data-action="back"]'))?.click();
    } else if (plain && (e.key === "f" || e.key === "F") && !typing(document.activeElement)) {
      const full = document.querySelector('[data-action="fullscreen"]');
      if (full) { e.preventDefault(); full.click(); }
    } else if (plain && (e.key === "m" || e.key === "M") && !typing(document.activeElement) && appRuns()) {
      // „Merken" for what the visitor is at: the row the keyboard is on, else the module that is
      // open (its preview, or its page). The buttons belong to the browser app.
      const row = document.activeElement?.closest?.(".row-wrap");
      const mark = row ? row.querySelector('[data-action="mark"]') : document.querySelector('.hero [data-action="mark"]');
      if (mark) { e.preventDefault(); mark.click(); }
    } else if (plain && (e.key === "p" || e.key === "P") && !typing(document.activeElement) && appRuns()) {
      // „Einplanen" for the module that is open (its preview, or its page); rows have no plan
      // button of their own. The keyboard on another row of the list than the open one would make
      // it ambiguous which module is meant, so P waits there.
      const row = document.activeElement?.closest?.(".row-wrap");
      const plan = row && !row.querySelector('[aria-current="true"]') ? null : document.querySelector('.hero [data-action="plan"]');
      if (plan) { e.preventDefault(); plan.click(); }
    } else if (((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") || (plain && e.key === "/" && !typing(document.activeElement))) {
      e.preventDefault();
      const search = document.getElementById("topsearch");
      search?.focus();
      search?.select();
    }
  });

  // A week of „Termine" off screen is not laid out (`content-visibility: auto`, app.css): it paints
  // nothing but its own background. While it is skipped it says so (`data-skipped`), and that
  // background is a skeleton of its days: a fast scroll that comes to it before the browser has
  // laid it out sees the week being filled in, not an empty panel.
  document.addEventListener("contentvisibilityautostatechange", (e) => {
    if (e.target.classList?.contains("agenda-week")) e.target.toggleAttribute("data-skipped", e.skipped);
  }, true);
})();
