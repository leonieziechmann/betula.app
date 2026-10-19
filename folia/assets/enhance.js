// Small behaviours shared by the pages of the site and the browser app. The site works without this
// file (it is links); before the app has taken over (`window.__betulaApp`) panels keep their scroll
// position across page loads, and a link followed while the app is starting waits for it. In both
// modes: the shortcuts (Esc closes the preview or leaves the module page, F opens the previewed
// module full screen, Ctrl+K or "/" jumps to the search, Alt and a digit — Ctrl and Alt off Windows —
// goes to a tab of the navigation; in the app M marks the module the visitor is at and P plans it
// into the Studienplan), the theme switch, the filter sheet (and a page's
// dialogs where a phone shows them as sheets), the swipe along the phone's bottom bar from tab to
// tab, the widths of the filter panel and the module preview (dragged, kept in localStorage), the
// room the panels make for the ground at the end of a page, and the way back to the top of a page
// („Nach oben").
(() => {
  const root = document.documentElement;
  // The page's language, as its address says it (`folia_locale::Locale::split`): the prefix of its
  // addresses and its words (docs/folia/i18n.md). The first is the default, without a prefix.
  const LANGUAGES = [
    { prefix: "", linkCopied: "Link kopiert", copied: "Kopiert", ctrl: "Strg" },
    { prefix: "/en", linkCopied: "Link copied", copied: "Copied", ctrl: "Ctrl" },
  ];
  const language = () => LANGUAGES.find((l) => l.prefix && (location.pathname === l.prefix || location.pathname.startsWith(l.prefix + "/"))) || LANGUAGES[0];
  const appRuns = () => window.__betulaApp === true;
  // The pages stand on stages, one for each tab, one of them in front (folia/crates/shell/src/stage.rs;
  // the server's page is on one as well): what belongs to the page is looked for on the stage in
  // front, the rest (the bars, the dialogs around the pages) outside every stage. A page behind has
  // parts of the same name (every list a `#preview`), which are not the visitor's.
  const stage = () => document.querySelector("#content > .page-stage.front");
  const offStage = (el) => !el.closest("#content > .page-stage");
  const find = (selector) => {
    const front = stage();
    if (!front) return document.querySelector(selector);
    return front.querySelector(selector) || [...document.querySelectorAll(selector)].find(offStage) || null;
  };
  const findAll = (selector) => {
    const front = stage();
    if (!front) return [...document.querySelectorAll(selector)];
    return [...front.querySelectorAll(selector), ...[...document.querySelectorAll(selector)].filter(offStage)];
  };
  const byId = (id) => find(`[id="${CSS.escape(id)}"]`);
  const narrow = matchMedia("(max-width: 900px)");
  const phone = () => narrow.matches;
  // Text-like controls only: a focused filter chip (checkbox) must not swallow Esc or "/".
  const typing = (el) => el && ((el.tagName === "INPUT" && !["checkbox", "radio", "button", "submit"].includes(el.type)) || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);

  // The start of the installed app (owner, 2026-10-15: concept A of design/launch/konzepte.html).
  // The head script has laid the picture of the system's splash screen over the page (`.launch`,
  // app.css), where the system had its own. It stands while the system fades its splash from above
  // it (300 ms; on iOS the switch from the launch screen is at once), then the icon flies to the
  // page's own, the first `.logo` on the screen (the rail's, or on a phone the start page's), and
  // turns from its circle into the icon's square, its leaf and colours into the logo's (Brave's
  // leaf is a little smaller, Android's own splash keeps ink and paper in the dark); the wordmark at
  // the bottom flies to the page's own, the first on the screen (the top bar's, or the start
  // page's), while the background clears. The page's own are not there while they fly: they come
  // in the frame in which the picture goes, where its icon and wordmark land (owner, 2026-10-18:
  // „das Icon soll erst existieren wenn es hin geflogen ist", `launch-logo`, `launch-mark`, app.css).
  // Without a logo on the screen, or with reduced motion, the picture fades; without a wordmark on
  // the screen, the launch's fades as the icon sets off. A tap takes it away at once, and once the
  // icon flies, taps reach the page.
  const launch = document.querySelector("html > .launch");
  // Faded by itself already (app.css, launch-gone): this script came too late.
  if (launch && getComputedStyle(launch).opacity !== "1") launch.remove();
  else if (launch) {
    launch.style.animation = "none";
    const icon = launch.querySelector(".launch-icon");
    const tile = icon.querySelector("b");
    const leaf = icon.querySelector("i");
    const name = launch.querySelector(".launch-name .wordmark");
    const ios = launch.classList.contains("ios");
    const timing = (delay, duration, easing) => ({ delay, duration, easing, fill: "both" });
    // Played, or taken away by a tap: once.
    let begun = false;
    let wait = 0;
    const go = () => launch.remove();
    const fade = (duration) => launch.animate([{ opacity: 1 }, { opacity: 0 }], timing(0, duration, "ease-out")).finished.then(go, go);
    // The first of the page's own on the screen.
    const shown = (selector) => [...document.querySelectorAll(selector)].filter((el) => !launch.contains(el)).find((el) => {
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.bottom > 0 && r.right > 0 && r.top < innerHeight && r.left < innerWidth;
    });
    // The way of concept A from one box to another, middle to middle: a little more up than across
    // in its first half, where it has already lost most of its size. How much it shrinks.
    const fly = (el, target) => {
      const from = el.getBoundingClientRect();
      const to = target.getBoundingClientRect();
      const k = to.height / from.height;
      const dx = to.left + to.width / 2 - (from.left + from.width / 2);
      const dy = to.top + to.height / 2 - (from.top + from.height / 2);
      el.animate([
        { transform: "none" },
        { transform: `translate(${dx * 0.47}px, ${dy * 0.6}px) scale(${k + (1 - k) * 0.364})`, offset: 0.5, easing: "cubic-bezier(.3,0,.2,1)" },
        { transform: `translate(${dx}px, ${dy}px) scale(${k})` },
      ], timing(0, 640, "cubic-bezier(.45,0,.55,1)"));
      return k;
    };
    const play = () => {
      if (begun) return;
      begun = true;
      launch.style.pointerEvents = "none";
      const logo = shown(".logo");
      if (!logo || matchMedia("(prefers-reduced-motion: reduce)").matches) return fade(300);
      const root = document.documentElement;
      root.classList.add("launch-logo");
      const k = fly(icon, logo);
      if (!ios) tile.animate([{ borderRadius: "50%" }, { borderRadius: "21.875%" }], timing(0, 300, "ease-in-out"));
      // iOS's shadow goes as it sets off; the logo's ring and shadow come on the last stretch, in the
      // icon's own size, which the flight scales.
      const ring = getComputedStyle(logo).boxShadow.replace(/(-?[\d.]+)px/g, (_, v) => `${v / k}px`);
      tile.animate([{ boxShadow: getComputedStyle(tile).boxShadow }, { boxShadow: "none", offset: 0.375 }, { boxShadow: "none", offset: 0.625 }, { boxShadow: ring }], timing(0, 640, "linear"));
      // The leaf as large as the logo's on the way's first stretch; the logo's colours on its last,
      // where the icon is small: ink and paper changing places pass through grey.
      const page = getComputedStyle(document.documentElement);
      leaf.animate([{ transform: getComputedStyle(leaf).transform }, { transform: `scale(${icon.offsetWidth / 96}) translateY(6.5px)` }], timing(0, 360, "ease-in-out"));
      for (const [el, colour] of [[tile, "--logo-tile"], [leaf, "--logo-leaf"]]) {
        el.animate([{ backgroundColor: getComputedStyle(el).backgroundColor }, { backgroundColor: page.getPropertyValue(colour).trim() }], timing(400, 240, "ease-in-out"));
      }
      const mark = name && shown(".wordmark:not(.ground .wordmark)");
      if (mark) {
        root.classList.add("launch-mark");
        fly(name, mark);
      } else name?.animate([{ opacity: 1 }, { opacity: 0 }], timing(0, 180, "ease-out"));
      launch.animate([{ backgroundColor: getComputedStyle(launch).backgroundColor }, { backgroundColor: "transparent" }], timing(200, 440, "ease-in-out"));
      // Landed: the page's own come in the frame the picture goes.
      const land = () => {
        root.classList.remove("launch-logo", "launch-mark");
        go();
      };
      Promise.all(launch.getAnimations({ subtree: true }).map((a) => a.finished)).then(land, land);
    };
    // Two frames: the picture has been painted, and from then on the system fades its splash.
    requestAnimationFrame(() => requestAnimationFrame(() => { wait = setTimeout(play, ios ? 150 : 350); }));
    launch.addEventListener("pointerdown", () => {
      if (begun) return;
      begun = true;
      clearTimeout(wait);
      launch.style.pointerEvents = "none";
      fade(150);
    });
  }
  // Where the installed app's page begins under the status bar, for the place of its next start
  // (folia_app::HEAD_SCRIPT, `betula.launch.top`): a finger's place on the screen less its place in
  // the page, at the first touch while the page is not zoomed, and only what the bars around the
  // page leave room for. iOS places its start by the screen alone.
  if (!("standalone" in navigator) && matchMedia("(display-mode: standalone)").matches) {
    const measure = (event) => {
      if (event.pointerType !== "touch" || Math.abs((window.visualViewport?.scale ?? 1) - 1) > 0.01) return;
      removeEventListener("pointerdown", measure, true);
      const upright = innerHeight >= innerWidth;
      const bars = (upright ? Math.max(screen.width, screen.height) : Math.min(screen.width, screen.height)) - innerHeight;
      const top = Math.round((event.screenY - event.clientY) * 100) / 100;
      if (top > 0 && top <= bars) {
        try {
          localStorage.setItem("betula.launch.top", (upright ? "p" : "l") + top);
        } catch {}
      }
    };
    addEventListener("pointerdown", measure, { capture: true, passive: true });
  }

  // The sheet of the phone layout: the catalog's filter panel, or a page's sidebar of filters.
  // Open, the page behind it is dimmed and stands still (html.sheet-open), and a tap on it closes
  // the sheet. A sheet whose page takes what is picked there as a draft (`data-draft`: the catalog
  // in the browser app) applies it only with its button („Module anzeigen", `sheet-apply`): the
  // page hears `betula:sheet-applied`, and the list follows as one step of the history. Any other
  // way it goes (its ×, a swipe down, a tap beside it, Esc, Back) throws the draft away (owner,
  // 2026-10-18: „Die Filter sollen bei der Mobile Version verworfen werden, wenn man das Fenster
  // schließt"): the page hears `betula:sheet-closed`, or `betula:sheet-left` where Back left the
  // page with the sheet open. The sheet has no entry in the history: where the browser has
  // `CloseWatcher` (Android), Back closes it as it closes what else lies over a page
  // (folia/crates/shell/src/nav.rs); without it (an iPhone) Back leaves the page, and the sheet
  // goes with it.
  const filters = () => byId("filters") || find(".sidebar.sheet");
  let sheetWatch = null; // the open sheet's CloseWatcher
  const dropWatch = () => { const w = sheetWatch; sheetWatch = null; try { w?.destroy(); } catch {} };
  const openSheet = () => {
    const sheet = filters();
    if (!sheet) return;
    sheet.classList.add("open");
    root.classList.add("sheet-open");
    if (!sheetWatch && phone() && typeof window.CloseWatcher === "function") {
      try {
        sheetWatch = new CloseWatcher();
        sheetWatch.onclose = () => { sheetWatch = null; closeSheet(); };
      } catch { sheetWatch = null; }
    }
  };
  const closeSheet = (how = "closed") => {
    const sheet = filters();
    const was = sheet?.classList.contains("open");
    sheet?.classList.remove("open");
    root.classList.remove("sheet-open");
    dropWatch();
    if (was) sheet.dispatchEvent(new Event("betula:sheet-" + how, { bubbles: true }));
  };
  // The app may replace the page under an open sheet (Back, a tab): nothing may stay dimmed.
  const tidySheet = () => { if (root.classList.contains("sheet-open") && !find(".filters.open, .sidebar.sheet.open")) { root.classList.remove("sheet-open"); dropWatch(); } };
  addEventListener("popstate", (e) => {
    // The browser app hands a step of the history to its router a frame later, so that the page
    // can answer first (folia/crates/shell/src/pending.rs): that is the same step again, not another one.
    if (e.betulaReplay) return;
    closeSheet("left");
  });

  // The step before in the history is where `link` leads: Back goes there instead of a new step,
  // the same entry as before, and the history does not grow. The browser says it where it has the
  // Navigation API (Chrome, Safari from 26.2, Firefox from 147); else the app's guess
  // (`data-back`), and the classic site's, which can only ask the referrer. Not a step of another
  // load of a page (the site's pages before the app took over, another language): going back
  // there would load that page anew.
  const samePage = (a, b) => {
    const norm = (u) => u.origin + u.pathname.replace(/^((?:\/[a-z]{2})?\/programs\/[^/]+)\/?$/, "$1/plan") + u.search;
    try { return norm(new URL(a, location.href)) === norm(new URL(b, location.href)); } catch { return false; }
  };
  const cameFrom = (link) => {
    const nav = window.navigation;
    if (appRuns() && nav?.currentEntry && typeof nav.entries === "function") {
      const before = nav.entries()[nav.currentEntry.index - 1];
      return !!before && before.sameDocument && samePage(before.url, link.href);
    }
    return appRuns() ? link.dataset.back === "history" : document.referrer.startsWith(location.origin);
  };

  // ---- classic mode: scroll positions of the panels survive a page load ----
  const listKey = () => location.search.replace(/([?&])(page|open)=[^&]*/g, "$1");
  function saveScroll() {
    try {
      findAll("[data-keep-scroll]").forEach((el) => {
        const name = el.dataset.keepScroll;
        sessionStorage.setItem("betula.scroll." + name, JSON.stringify({ top: el.scrollTop, key: name === "rows" ? listKey() : "" }));
      });
    } catch {}
  }
  function restoreScroll() {
    try {
      findAll("[data-keep-scroll]").forEach((el) => {
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
  // as one single move, whose speed is then measured from there). How far down it is, as a share
  // of its height, is `--drag` on it: a dialog's dim backdrop fades by it.
  function dragSheet(sheet, since) {
    let dy = 0, at = since, speed = 0;
    const height = sheet.offsetHeight || 1;
    sheet.classList.add("dragging");
    return {
      to(offset) {
        const now = performance.now(), next = Math.max(0, offset);
        // px per ms, mostly of the last moves: a flick at the end counts, a slow start does not.
        if (now > at) speed = 0.7 * ((next - dy) / (now - at)) + 0.3 * speed;
        dy = next;
        at = now;
        sheet.style.transform = `translateY(${dy}px)`;
        sheet.style.setProperty("--drag", Math.min(1, dy / height).toFixed(3));
      },
      release() {
        const flicked = performance.now() - at < 100 && speed > 0.45;
        sheet.style.transform = "";
        sheet.style.removeProperty("--drag");
        sheet.classList.remove("dragging");
        if (dy > Math.min(160, height * 0.3) || (dy > 16 && flicked)) (sheet.tagName === "DIALOG" ? dropDialog(sheet) : closeSheet());
      },
    };
  }
  // A page's dialog that a phone shows as a sheet from below (`dialog[data-sheet]`: the areas of
  // „Mein Studium", „Studiengang wechseln", „Module hinzufügen") is dragged down as the filter
  // sheet is (owner, 2026-10-05: „dass man die einfach wieder runter sliden kann"). Let go, it goes
  // on down from where the finger left it (app.css, `.dropped`) and closes, as its × and Esc close
  // it: the page hears the dialog's `close`.
  const dropDialog = (dialog) => {
    dialog.classList.add("dropped");
    setTimeout(() => {
      if (dialog.open) dialog.close();
      dialog.classList.remove("dropped");
    }, 200);
  };
  const openSheetNow = () => (phone() ? find(".filters.open, .sidebar.sheet.open, dialog[data-sheet][open]:not(.dropped)") : null);
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
    const head = e.target.closest?.(".filters .panel-head, .sidebar.sheet .panel-head, dialog[data-sheet] header");
    const sheet = head?.closest(".filters, .sidebar.sheet, dialog[data-sheet]");
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
  // Where a glide stands `k` keyframes in: never before its first one nor past its last. An
  // animation the compositor has just started can be a frame ahead of the page's clock, its time
  // a little below 0 (on a phone busy with the page a swipe has gone to, the finger of the next
  // swipe catches the glide just then): before, that was no keyframe at all, and the bar stood
  // still with its lens and the wrong icon in it, no swipe moving it again.
  const sampleAt = (values, k) => {
    const at = k > 0 ? Math.min(k, values.length - 1) : 0, i = Math.floor(at), j = Math.min(i + 1, values.length - 1);
    return values[i] + (values[j] - values[i]) * (at - i);
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

  // ---- a module's sheet on a phone ----
  // (owner, 2026-10-06: „wenn man wie bei der Übersicht nach bereichen so ein menu bekommt, dass
  // sich dann von unten öffnet. Hierbei finde ich, dass es sinn ergeben würde, wenn es sich erstmal
  // bis zur hälfte oder einemdrittel öffnet und dann kann man nach unten swipen oder es nach oben um
  // es zu schließen oder den vollen bereich zu verwenden. Achte dabei darauf, dass man auch noch
  // scrollen können muss in dem fenster.") A module picked in a list comes up from below to half
  // the screen (app.css `.detail.is-module`), as a phone's own sheets do. A finger on it moves it:
  // half up, any move up or down, and the content does not scroll yet; all up (`is-full`), a move
  // down where the content is at its top, else the content scrolls and keeps its scroll to itself.
  // Pulled up past all up, the rest of the finger's way scrolls the content, and let go there the
  // content goes on as the finger went. A tap on its grabber takes it all up and back. Closed
  // (pulled away, its ×, a tap beside it), it slides down and its close link is followed: back
  // through the history where the page without the module is the step before (`data-back`), so
  // that Back does not bring it again. Esc and Back close it at once (the page follows the
  // address). A touch beside it moves nothing.
  //
  // Each of its moves is one damped spring (owner, 2026-10-16: „Wenn man etwas anklickt, dann
  // sollte es eine animation geben, wie es sich öffnet … wenn man mit genug speed oder swipe weite
  // das zu macht, dann soll von ganz oben nach ganz zu gehen. Ja und dann muss die animation zu der
  // mitte auch physics based sein … Das muss sich einfach natürlich anfühlen"; a fixed curve took
  // over at its own speed whatever the finger's, a flick down from all up stopped at half up, and
  // the skeleton's entrance ended where the module came, which then stood there at once): it comes
  // up from below on it, and let go it goes on from where it is and as fast as it went. Where to is
  // where it would come to rest thrown at that speed, slowing down by SHEET_FRICTION: all up in the
  // upper half of the way to half up, half up down to a third of the way from there to the bottom,
  // below that away; and a flick goes at least to the next stop its way. So a slow pull ends at the
  // stop it is nearest, and a quick or a long swipe down from all up takes it away, past half up
  // (from about 1.4 px per ms where a finger lets go 100 to 200 px down from all up). The page
  // behind it is less dimmed the further down from half up it is. The glide is Web Animations,
  // which the compositor runs while the module is being built; a finger that catches it takes the
  // sheet where it is. A sheet that comes in the place of another (the module in the place of its
  // skeleton, another module through a link in it) goes on where that one was: on its way, at rest
  // all up or half up, or under the finger.
  // The springs as the bar's (TAB_SPRING): to a stop with a swing too small to see; away softer and
  // without one, so that a thrown sheet keeps going rather than shooting off; without motion short.
  const SHEET_SPRING = { response: 0.42, damping: 0.86 };
  const SHEET_AWAY = { response: 0.6, damping: 0.99 };
  const SHEET_CALM = { response: 0.22, damping: 0.99 };
  const SHEET_FRICTION = 0.0025; // px per ms²: how fast a sheet thrown would slow down, to choose its stop
  const SHEET_FLICK = 0.45; // px per ms: a let-go this fast goes at least to the next stop its way (the bar's swipe: 0.45)
  const SHEET_PAST = 150; // px past the bottom the spring of a sheet that goes away aims, so that it leaves at speed
  // The module's sheet in the page in front; not its copy in the picture of the page held in front
  // of the next one (`.pending-page.held`, which app.css does not show), nor one open on a page kept
  // behind (folia/crates/shell/src/stage.rs).
  const modules = document.getElementsByClassName("is-module");
  const liveSheet = () => {
    for (const el of modules) if (el.classList.contains("detail") && !el.closest(".pending-page") && !el.closest("#content > .page-stage:not(.front)")) return el;
    return null;
  };
  const moduleSheet = () => (phone() && appRuns() ? liveSheet() : null);
  // How far down the sheet is drawn (px; 0 all up), on its way included: where its glide has it
  // (`glideY`, below), else where its style puts it.
  const sheetY = (sheet) => (sheetGlide?.sheet === sheet ? glideY(sheetGlide) : new DOMMatrixReadOnly(getComputedStyle(sheet).transform).m42);
  // Where it stands half up (app.css --sheet-half: 48 % of the screen from the top), and where it is
  // out of sight, its top at the bottom of the screen.
  const sheetHalf = (sheet) => Math.max(0, innerHeight * 0.48 - sheet.offsetTop);
  const sheetAway = (sheet) => Math.max(1, innerHeight - sheet.offsetTop);
  // The page behind is dimmed while the sheet is half up or higher, and less the further down from
  // there it is (`--sheet-shade`, app.css): the sheet at `y`, its stops at `half` and `away`.
  const shadeSheet = (y, half, away) => {
    const shade = y <= half ? 1 : unit((away - y) / Math.max(1, away - half));
    if (shade >= 1) root.style.removeProperty("--sheet-shade");
    else root.style.setProperty("--sheet-shade", shade.toFixed(3));
  };
  // Where a sheet let go at `y` going `v` (px per ms, downwards positive) comes to rest.
  const sheetGoal = (sheet, y, v) => {
    const half = sheetHalf(sheet), away = sheetAway(sheet), end = y + (v * Math.abs(v)) / (2 * SHEET_FRICTION);
    let to = end < half / 2 ? "full" : end < half + (away - half) / 3 ? "half" : "gone";
    const at = { full: 0, half, gone: away };
    if (v > SHEET_FLICK && at[to] <= y) to = y < half ? "half" : "gone";
    else if (v < -SHEET_FLICK && at[to] >= y) to = y > half ? "half" : "full";
    return to;
  };
  let sheetGlide = null; // the sheet on its way: { sheet, to, half, away, ys, frames, timing, anim, begun, frame, then, coming }
  // Where the glide has the sheet now.
  const glideY = (g) => {
    const began = g.anim.startTime ?? g.begun;
    return sampleAt(g.ys, began == null ? 0 : (document.timeline.currentTime - began) / 1000 / TAB_DT);
  };
  const stopSheet = () => {
    const g = sheetGlide;
    if (!g) return;
    sheetGlide = null;
    cancelAnimationFrame(g.frame);
    g.anim.cancel();
  };
  // There: its classes hold it where the glide ended (app.css), and what was to follow follows.
  const restSheet = (g) => {
    if (sheetGlide !== g) return;
    stopSheet();
    shadeSheet(g.to === "gone" ? g.away : 0, g.half, g.away);
    g.then?.();
  };
  // Every frame of a glide the page behind takes the shade of where the sheet is; as the sheet comes
  // up, the page's thread may be building the module, and the page darkens by itself (app.css).
  const tickSheet = (g) => {
    if (sheetGlide !== g || g.coming) return;
    shadeSheet(glideY(g), g.half, g.away);
    g.frame = requestAnimationFrame(() => tickSheet(g));
  };
  const runSheet = (g, sheet) => {
    g.sheet = sheet;
    g.anim = sheet.animate(g.frames, g.timing);
    g.anim.onfinish = () => restSheet(g);
    sheet.classList.toggle("is-full", g.to === "full");
    sheet.classList.toggle("is-gone", g.to === "gone");
    sheet.style.removeProperty("transform");
  };
  // The sheet from `y` (px from all up) going `v` px per ms to `to` ("full", "half" or "gone") on
  // the spring: keyframes every TAB_DT for the compositor. Its classes say where it goes from the
  // first frame on, so that where the glide ends is where they hold it. `then` once it is there;
  // `coming`: it comes up from below.
  const glideSheet = (sheet, y, v, to, then, coming = false) => {
    stopSheet();
    const calm = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const half = sheetHalf(sheet), away = sheetAway(sheet);
    let ys = springTo(y, calm ? 0 : v * 1000, to === "full" ? 0 : to === "half" ? half : away + SHEET_PAST, calm ? SHEET_CALM : to === "gone" ? SHEET_AWAY : SHEET_SPRING);
    // All up is as high as it goes, and away it is once out of sight.
    if (to === "full") ys = ys.map((y) => Math.max(0, y));
    const out = to === "gone" ? ys.findIndex((y) => y >= away) : -1;
    if (out > 0) ys = ys.slice(0, out + 1);
    const g = sheetGlide = {
      to, half, away, ys, then, coming, frame: 0,
      frames: ys.map((y) => ({ transform: `translateY(${dots(y)}px)` })),
      timing: { duration: Math.max(1, (ys.length - 1) * TAB_DT * 1000), easing: "linear", fill: "forwards" },
    };
    runSheet(g, sheet);
    // When it begins: with the first frame that draws it, which the page's thread knows before it
    // hears from the compositor (too busy building the module, it may hear only much later).
    requestAnimationFrame((at) => { if (g.begun == null) g.begun = at; });
    tickSheet(g);
  };
  // Away it goes, then the close link of the sheet that is there then is followed. A note that
  // waits for interaction over it goes down with it at once (`sheet-away`, folia_design::waitnote;
  // owner, 2026-10-18: „wenn man da die Modul Ansicht schließt sollte die notification auch mit eher
  // Animation nach unten raus geschlossen werden").
  const closeModule = (sheet, y = sheetY(sheet), v = 0) => {
    if (!sheet.querySelector('[data-action="close-detail"]') || sheet.classList.contains("is-gone")) return;
    document.dispatchEvent(new Event("sheet-away"));
    glideSheet(sheet, y, v, "gone", () => {
      const link = liveSheet()?.querySelector('[data-action="close-detail"]');
      if (!link) return;
      if (cameFrom(link)) history.back(); else link.click();
    });
  };
  // The content going on after the finger let go (px per ms, the finger's way), slower and slower
  // as a phone's own scrolling does, until an end or a finger stops it.
  let fling = null;
  const stopFling = () => {
    if (fling) cancelAnimationFrame(fling.frame);
    fling = null;
  };
  const flingContent = (scroller, v) => {
    stopFling();
    if (!scroller || Math.abs(v) < 0.05) return;
    const f = fling = { frame: 0 };
    let speed = -v, last = performance.now();
    const step = (now) => {
      if (fling !== f || !scroller.isConnected) return;
      const dt = Math.min(48, Math.max(0, now - last)), before = scroller.scrollTop;
      last = now;
      scroller.scrollTop = before + speed * dt;
      speed *= Math.pow(0.998, dt);
      if (Math.abs(speed) < 0.02 || (dt > 0 && scroller.scrollTop === before)) fling = null;
      else f.frame = requestAnimationFrame(step);
    };
    f.frame = requestAnimationFrame(step);
  };
  // How fast the finger went at the end (px per ms, downwards positive), over its last 100 ms:
  // nothing if it rested before it let go.
  const fingerSpeed = (trail, at) => {
    const a = trail[0], b = trail[trail.length - 1];
    if (!a || a === b || at - b[0] > 80 || b[0] - a[0] < 8) return 0;
    return Math.max(-8, Math.min(8, (b[1] - a[1]) / (b[0] - a[0])));
  };
  let sheetTouch = null;
  document.addEventListener("touchstart", (e) => {
    sheetTouch = null;
    const sheet = moduleSheet();
    if (!sheet || e.touches.length !== 1) return;
    stopFling();
    const t = e.touches[0];
    const scroller = sheet.querySelector(":scope > .scroll");
    const top = sheet.getBoundingClientRect().top;
    sheetTouch = {
      sheet, inside: sheet.contains(e.target), scroller, x: t.clientX, y: t.clientY, at: e.timeStamp,
      // All up, and there already: the content may scroll.
      full: sheet.classList.contains("is-full") && top - sheet.offsetTop < 1, scrolled: scroller?.scrollTop || 0,
      grabber: t.clientY - top < 26, mode: null, from: 0, half: 0, away: 0, trail: [],
    };
  }, { passive: true });
  const moveSheet = (e) => {
    const s = sheetTouch;
    if (!s) return;
    if (e.touches.length !== 1 || !s.sheet.isConnected) { sheetTouch = null; return; }
    const t = e.touches[0];
    const dx = t.clientX - s.x, dy = t.clientY - s.y;
    if (!s.mode) {
      if (!s.inside || s.sheet.classList.contains("is-gone")) s.mode = "hold"; // on its way out, nothing stops it
      else if (Math.abs(dx) > Math.abs(dy)) s.mode = "own"; // sideways: the content's (a week, a table)
      else if (!e.cancelable) s.mode = "own"; // the browser scrolls already
      else if (!s.full || (dy > 0 && s.scrolled <= 0)) {
        // The finger takes the sheet: at rest under the point it touched, on its way where it is.
        s.mode = "drag";
        s.from = sheetY(s.sheet) - (sheetGlide?.sheet === s.sheet ? dy : 0);
        s.half = sheetHalf(s.sheet);
        s.away = sheetAway(s.sheet);
        stopSheet();
      } else s.mode = "own";
    }
    if (s.mode === "own") return;
    if (e.cancelable) e.preventDefault();
    if (s.mode !== "drag") return;
    const y = s.from + dy;
    // Past all up the content scrolls on with the finger.
    if (y < 0 && s.scroller) s.scroller.scrollTop = s.scrolled - y;
    s.sheet.style.transform = `translateY(${Math.max(0, y)}px)`;
    shadeSheet(y, s.half, s.away);
    s.trail.push([e.timeStamp, y]);
    while (s.trail.length > 2 && e.timeStamp - s.trail[0][0] > 100) s.trail.shift();
  };
  document.addEventListener("touchmove", moveSheet, { passive: false });
  const letGoSheet = (at) => {
    const s = sheetTouch;
    sheetTouch = null;
    if (!s || !s.sheet.isConnected) return;
    if (!s.mode && s.inside && s.grabber && at - s.at < 400) {
      const going = sheetGlide?.sheet === s.sheet ? sheetGlide.to : s.sheet.classList.contains("is-full") ? "full" : "half";
      if (going !== "gone") glideSheet(s.sheet, sheetY(s.sheet), 0, going === "full" ? "half" : "full");
      return;
    }
    if (s.mode !== "drag") return;
    const y = s.trail.length ? s.trail[s.trail.length - 1][1] : s.from;
    const v = fingerSpeed(s.trail, at);
    // Pulled past all up: all up it stays, and the content goes on as the finger went.
    const to = y < 0 ? "full" : sheetGoal(s.sheet, y, v);
    if (to === "gone") closeModule(s.sheet, y, v);
    else glideSheet(s.sheet, Math.max(0, y), y < 0 ? 0 : v, to);
    if (y < 0) flingContent(s.scroller, v);
  };
  const endSheet = (e) => { if (!e.touches.length) letGoSheet(e.timeStamp); };
  const cancelSheet = () => {
    const s = sheetTouch;
    sheetTouch = null;
    if (s?.mode !== "drag" || !s.sheet.isConnected) return;
    // Taken from the finger: to the stop it is nearest, but not away.
    const y = Math.max(0, s.trail.length ? s.trail[s.trail.length - 1][1] : s.from), to = sheetGoal(s.sheet, y, 0);
    glideSheet(s.sheet, y, 0, to === "gone" ? "half" : to);
  };
  document.addEventListener("touchend", endSheet);
  document.addEventListener("touchcancel", cancelSheet);
  // Its ×, and a tap beside it: down it slides, then the close link is followed.
  document.addEventListener("click", (e) => {
    const sheet = e.isTrusted ? moduleSheet() : null;
    if (!sheet || e.button !== 0) return;
    const close = e.target.closest?.('[data-action="close-detail"]');
    if ((close && sheet.contains(close)) || e.target === root) {
      e.preventDefault();
      closeModule(sheet);
    }
  }, true);
  // A sheet that comes is seen before it is drawn. It comes up from below, unless it comes in the
  // place of one that just went and was not closed (its skeleton, another module through a link in
  // it): then it goes on where that one was — on its way, at rest all up or half up, or under the
  // finger. One that goes without another in its place takes its glide and the shade with it. But
  // a skeleton that goes while the address still asks for a module (the step's wait is over, the
  // module's answer is not there yet: on a phone a tenth of a second and more, and the page shows
  // nothing beside itself until it is) stays on the screen as it was, on its way, until the module
  // comes in its place, the address no longer asks for one, or nothing came for three seconds.
  let shownSheet = null; // the sheet in the page as last seen
  let lostSheet = null; // the one that went without another in its place, for a moment: { sheet, at }
  let standin = null; // the skeleton kept on the screen: { sheet, timer }
  const asksModule = () => /[?&]open=/.test(location.search);
  const dropStandin = () => {
    const s = standin;
    standin = null;
    if (!s) return;
    clearTimeout(s.timer);
    s.sheet.classList.add("is-gone"); // and not kept again
    s.sheet.remove();
  };
  const takeOver = (sheet, before) => {
    if (sheetGlide?.sheet === before) {
      const was = sheetGlide.anim;
      runSheet(sheetGlide, sheet);
      // Where the one before is, though the page's thread may not have heard yet that it began;
      // not drawn yet, this one begins with the frame that draws it.
      const began = was.startTime ?? sheetGlide.begun;
      if (began != null) sheetGlide.anim.startTime = began;
      was.cancel();
    } else sheet.classList.toggle("is-full", before.classList.contains("is-full"));
    if (sheetTouch?.sheet !== before) return;
    sheetTouch.sheet = sheet;
    sheetTouch.scroller = sheet.querySelector(":scope > .scroll");
    if (sheetTouch.mode === "drag") sheet.style.transform = before.style.transform;
    // The finger's events go on to where it touched, which left the page with the sheet that went:
    // the document no longer hears them there.
    if (!sheetTouch.inside || before.isConnected) return;
    before.addEventListener("touchmove", moveSheet, { passive: false });
    before.addEventListener("touchend", endSheet);
    before.addEventListener("touchcancel", cancelSheet);
  };
  new MutationObserver(() => {
    if (standin && !asksModule()) dropStandin();
    const sheet = moduleSheet();
    if (sheet === shownSheet) return;
    const before = shownSheet;
    shownSheet = sheet;
    const now = performance.now();
    if (before && before === standin?.sheet && sheet) {
      // The module came: in the skeleton's place, where that is.
      clearTimeout(standin.timer);
      standin = null;
      before.remove();
      takeOver(sheet, before);
      return;
    }
    if (before && !before.isConnected) {
      if (!sheet && !standin && asksModule() && before.classList.contains("sk-detail") && !before.classList.contains("is-gone")) {
        shownSheet = before;
        standin = { sheet: before, timer: setTimeout(dropStandin, 3000) };
        document.body.append(before);
        takeOver(before, before);
        return;
      }
      lostSheet = { sheet: before, at: now };
    }
    if (!sheet) {
      root.style.removeProperty("--sheet-shade");
      const lost = lostSheet;
      // Nothing came in its place: its glide ends with it (a finished one has followed already).
      setTimeout(() => { if (lostSheet === lost && !shownSheet && sheetGlide?.sheet === lost?.sheet) stopSheet(); }, 150);
      return;
    }
    const was = lostSheet && now - lostSheet.at < 150 ? lostSheet.sheet : null;
    lostSheet = null;
    if (was && (sheetGlide?.sheet === was || !was.classList.contains("is-gone"))) takeOver(sheet, was);
    else glideSheet(sheet, sheetAway(sheet), 0, "half", null, true);
  }).observe(document.body, { childList: true, subtree: true });
  // A window that grows past a phone's width or shrinks to it: the module stands where the page's
  // layout puts it at once, without anything of a glide, and without coming up from below.
  narrow.addEventListener("change", () => {
    dropStandin();
    stopSheet();
    stopFling();
    root.style.removeProperty("--sheet-shade");
    for (const el of modules) el.style.removeProperty("transform");
    shownSheet = moduleSheet();
  });

  // ---- a module's Termine: the list under its week ----
  // (owner, 2026-10-17: „die Termine können ruhig ausklappbar sein, so dass nicht so viel vertical
  // space verschwendet wird. Und dann sollte es so sein, dass wenn man auf einen Termin im Kalender
  // klickt, dass sich die Termine automatisch ausklappen und dann der angeklickte Termin highlighted
  // wird. Idealer Weise ohne den view zu verschieben aber wenn sich sein muss, dann mach eine
  // animation rein, so dass man versteht was mit der Cam passiert ist.") Under a week the list is
  // folded (`details.evfold`, folia/crates/widgets/src/module.rs); its line unfolds and folds it as
  // a thing does, its height from nothing to its own and back. A slot of the week (`a.slot`,
  // `data-action="termin"`, its lines in `data-lines`) unfolds it, marks itself and its lines, and
  // the lines light up a moment (app.css `.picked`, `.lit`). The week stays where it is: the list
  // unfolds under it. Only where a line is out of sight then does what scrolls glide, as little as
  // brings it in, while the list unfolds; a module's sheet half up goes all up for it on its spring.
  const FOLD_OPEN = 300; // ms
  const FOLD_SHUT = 200;
  const folding = new WeakMap(); // a list on its way: { anim, to }
  // `el` from `from` px high to `to`, then `done`; what it holds stays inside it on the way.
  const growFold = (el, from, to, ms, done) => {
    folding.get(el)?.anim.cancel();
    if (glide() === "instant") { done?.(); return; }
    el.classList.add("unfolding");
    const anim = el.animate([{ height: `${from}px`, opacity: from ? 1 : 0 }, { height: `${to}px`, opacity: to ? 1 : 0 }], { duration: ms, easing: to ? "cubic-bezier(.2, .7, .2, 1)" : "cubic-bezier(.4, 0, 1, 1)" });
    const way = { anim, to };
    folding.set(el, way);
    const end = () => { if (folding.get(el) === way) folding.delete(el); el.classList.remove("unfolding"); };
    anim.onfinish = () => { end(); done?.(); };
    anim.oncancel = end;
  };
  const foldList = (fold) => fold.querySelector(":scope > .evlist");
  // Open, and its height grows from what is shown now (nothing, or where a fold was on its way).
  const unfoldList = (fold) => {
    const list = foldList(fold);
    if (!list) return;
    const shown = fold.open ? list.getBoundingClientRect().height : 0;
    folding.get(list)?.anim.cancel();
    fold.open = true;
    const to = list.offsetHeight;
    if (to > shown + 1) growFold(list, shown, to, FOLD_OPEN);
  };
  const shutList = (fold) => {
    const list = foldList(fold);
    for (const el of fold.closest(".section")?.querySelectorAll('.slot[aria-current="true"]') ?? []) el.removeAttribute("aria-current");
    for (const el of fold.querySelectorAll(".ev.picked")) el.classList.remove("picked", "lit");
    if (!list) { fold.open = false; return; }
    growFold(list, list.getBoundingClientRect().height, 0, FOLD_SHUT, () => { fold.open = false; });
  };
  // What scrolls the list: the module's sheet or its panel, or the page.
  const scrollerOf = (el) => {
    for (let p = el.parentElement; p && p !== document.body; p = p.parentElement) {
      if (/(auto|scroll)/.test(getComputedStyle(p).overflowY)) return p;
    }
    return document.scrollingElement;
  };
  // The part of the screen where `scroller` shows its content: below what stands over the page at
  // its top, above the bar at the bottom; `lift` px higher for a sheet on its way all up.
  const viewOf = (scroller, lift) => {
    if (scroller !== document.scrollingElement) {
      const r = scroller.getBoundingClientRect();
      return { top: r.top - lift + scroller.clientTop, bottom: Math.min(innerHeight, r.top - lift + scroller.clientTop + scroller.clientHeight) };
    }
    // A bar in the upper half of the screen covers what is above its bottom, one in the lower half
    // (a phone's, which floats over the edge) what is below its top.
    let top = 0, bottom = innerHeight;
    for (const bar of document.querySelectorAll(".topbar, .bottomnav")) {
      const r = bar.getBoundingClientRect(), place = getComputedStyle(bar).position;
      if (!r.height || (place !== "fixed" && place !== "sticky")) continue;
      if (r.top + r.bottom < innerHeight) top = Math.max(top, r.bottom);
      else bottom = Math.min(bottom, r.top);
    }
    return { top, bottom };
  };
  // A glide of what scrolls by `by` px, frame by frame: what unfolds below grows meanwhile and lets
  // it go further than it could at first. A wheel, a touch, a click or a key stops it.
  let gliding = 0;
  const STOP_GLIDING = ["wheel", "touchstart", "pointerdown", "keydown"];
  const stopGliding = () => {
    if (!gliding) return;
    cancelAnimationFrame(gliding);
    gliding = 0;
    for (const type of STOP_GLIDING) removeEventListener(type, stopGliding, true);
  };
  const glideBy = (scroller, by) => {
    stopGliding();
    const put = (y) => { if (scroller === document.scrollingElement) window.scrollTo(0, y); else scroller.scrollTop = y; };
    const from = scroller.scrollTop, to = from + by;
    if (glide() === "instant") { put(to); return; }
    const start = performance.now(), ms = Math.min(560, 320 + Math.abs(by) / 3);
    const step = (now) => {
      const t = Math.min(1, (now - start) / ms);
      put(from + by * (1 - (1 - t) ** 3));
      if (t < 1) gliding = requestAnimationFrame(step);
      else stopGliding();
    };
    gliding = requestAnimationFrame(step);
    for (const type of STOP_GLIDING) addEventListener(type, stopGliding, { capture: true, passive: true });
  };
  const showTermin = (slot) => {
    const section = slot.closest(".section");
    const lines = (slot.dataset.lines || "").split(" ").filter(Boolean).map((id) => section?.querySelector(`#${CSS.escape(id)}`)).filter(Boolean);
    if (!lines.length) return false;
    for (const el of section.querySelectorAll('.slot[aria-current="true"]')) el.removeAttribute("aria-current");
    for (const el of section.querySelectorAll(".ev.picked")) el.classList.remove("picked", "lit");
    slot.setAttribute("aria-current", "true");
    const fold = lines[0].closest("details.evfold");
    if (fold) unfoldList(fold);
    for (const line of lines) {
      line.classList.add("picked");
      void line.offsetWidth; // the light comes again with each tap
      line.classList.add("lit");
    }
    // Where the lines stand once the list is open (its height grows meanwhile; they are where they
    // will be already) and the sheet they are in all up (`lift` px higher than it is), and how far
    // what scrolls has to go to show them: nothing where they are in sight, else as little as
    // brings them in, the first of them before the rest.
    const scroller = scrollerOf(lines[0]);
    const sheet = moduleSheet();
    const inSheet = Boolean(sheet?.contains(scroller));
    const lift = inSheet ? Math.max(0, sheetY(sheet)) : 0;
    const view = viewOf(scroller, lift), gap = 12;
    const first = lines[0].getBoundingClientRect().top - lift, last = lines[lines.length - 1].getBoundingClientRect().bottom - lift;
    let by = 0;
    if (last > view.bottom - gap) by = last - (view.bottom - gap);
    if (first - by < view.top + gap) by = first - (view.top + gap);
    if (inSheet && !sheet.classList.contains("is-full")) glideSheet(sheet, lift, 0, "full");
    if (Math.abs(by) >= 1) glideBy(scroller, by);
    return true;
  };

  // ---- a tab held: what it holds besides its page ----
  // (owner, 2026-10-06: „wenn man die buttons in der nav bar lange gedrückt hält, dass dann eine
  // special aktion kommt. Bei mein Studium währe das dann eine auswahl von Regelstudienplan,
  // Wahlpflicht, Alle Studiengänge. Das sollte natürlich auch mit [Rechts]klick funktionieren. Bei
  // dem katalog könnte sich dann gleich der katalog mit offenem Filter öffnen (das wäre aber mobile
  // only weil auf dem pc ist der ja eh immer offen)"). A finger that stays on a tab of the bar for
  // TAB_HOLD ms without moving, or a right click (or the menu key) on a tab of the bar or the rail:
  // a tab the app marked `data-hold="menu"` gets its menu from the app (`betula:tab-menu` on the
  // tab, folia/crates/shell/src/chrome.rs `TabMenu`); the catalog's (`data-hold="filters"`) opens
  // the catalog's list with its filters, on a phone only. The way to the profile page
  // (`data-hold="profiles"`, chrome.rs `ProfileButton`; owner, 2026-10-14: „Beim Handy long hold")
  // is held the same way, the box at a phone's head with a finger, the rail's with a right click,
  // and gets the menu of the profiles (`betula:profile-menu`, chrome.rs `ProfileMenu`; a tap on the
  // phone's box opens it as well, chrome.rs `ProfileButton`). The touch
  // that held is no tap (its click goes) and no swipe. The bar's tabs and the phone's box never get
  // the browser's own menu of a link (nor, app.css, its preview of one).
  const TAB_HOLD = 420; // ms: a little before the browser's own long press (Android: 400–500)
  let tabHold = null; // the finger on a tab that may hold: { tab, id, x, y, timer }
  let tabHeld = false; // the touch on the bar held a tab: its click is no tap
  const dropHold = () => { clearTimeout(tabHold?.timer); tabHold = null; };
  const onCatalogList = () => location.pathname === (language().prefix || "") + "/catalog";
  // The catalog's list with its filters: here at once, else once the tab's list is there.
  const catalogFilters = (tab) => {
    if (onCatalogList() && byId("filters")) { openSheet(); return; }
    const a = Object.assign(document.createElement("a"), { href: tab.dataset.list || tab.href, hidden: true });
    document.body.append(a);
    a.click();
    a.remove();
    const until = performance.now() + 6000;
    const wait = () => {
      if (onCatalogList() && byId("filters") && !document.querySelector(".pending-page")) openSheet();
      else if (performance.now() < until) requestAnimationFrame(wait);
    };
    requestAnimationFrame(wait);
  };
  // What the tab holds, if it holds anything here: true where it was done.
  const holdTab = (tab) => {
    if (!appRuns()) return false;
    switch (tab.dataset.hold) {
      case "menu":
        tab.dispatchEvent(new Event("betula:tab-menu", { bubbles: true }));
        return true;
      case "profiles":
        tab.dispatchEvent(new Event("betula:profile-menu", { bubbles: true }));
        return true;
      case "filters":
        if (!phone()) return false;
        catalogFilters(tab);
        return true;
      default:
        return false;
    }
  };
  const fireHold = () => {
    const hold = tabHold;
    tabHold = null;
    if (!hold?.tab.isConnected || !holdTab(hold.tab)) return;
    tabHeld = true;
    if (tabDrag?.id === hold.id) tabDrag = null;
    navigator.vibrate?.(8);
  };
  document.addEventListener("pointerdown", (e) => {
    dropHold();
    tabHeld = false;
    if (!e.isPrimary || e.pointerType === "mouse" || e.button !== 0) return;
    const tab = e.target.closest?.(".bottomnav > .nav[data-hold], .top-profile[data-hold]");
    if (tab) tabHold = { tab, id: e.pointerId, x: e.clientX, y: e.clientY, timer: setTimeout(fireHold, TAB_HOLD) };
  });
  document.addEventListener("pointermove", (e) => {
    if (tabHold && e.pointerId === tabHold.id && Math.max(Math.abs(e.clientX - tabHold.x), Math.abs(e.clientY - tabHold.y)) >= TAB_SLOP) dropHold();
  });
  document.addEventListener("pointerup", (e) => { if (e.pointerId === tabHold?.id) dropHold(); });
  document.addEventListener("pointercancel", (e) => { if (e.pointerId === tabHold?.id) dropHold(); });
  document.addEventListener("contextmenu", (e) => {
    const tab = e.target.closest?.(".bottomnav > .nav, .rail .nav, .profile-btn[data-hold]");
    if (!tab) return;
    const bar = tab.closest(".bottomnav, .top-profile");
    if (bar) e.preventDefault();
    // The browser's long press of a finger comes after ours, or a little before it.
    if (tabHeld) return;
    if (tabHold?.tab === tab) { clearTimeout(tabHold.timer); fireHold(); return; }
    if (holdTab(tab)) { e.preventDefault(); if (bar) tabHeld = true; }
  });
  // The click that ends the touch, before anything else hears it: where the hold opened a sheet,
  // it lands on the dimmed page and would close it again.
  addEventListener("click", (e) => {
    if (!tabHeld || !e.isTrusted) return;
    tabHeld = false;
    e.preventDefault();
    e.stopPropagation();
  }, true);
  // A page the browser kept (Back after a page load) shows its own tab's mark again.
  addEventListener("pageshow", (e) => { if (e.persisted) { tabDrag = null; settleTabs(); } });

  // The browser's own chrome in the colours of the theme (they are --bg of the two themes), and the
  // manifest the page names with it (`folia_app::MANIFEST_DARK`): an installed app takes the colours
  // of its splash screen and of the strip under the gesture bar from there, as the head script
  // names it before the first paint.
  const paintChrome = (dark) => {
    for (const meta of document.querySelectorAll("meta[name=theme-color]")) meta.content = dark ? "#0a0c11" : "#f1f2f4";
    for (const link of document.querySelectorAll("link[rel=manifest]")) {
      const href = link.getAttribute("href").replace(/\/manifest(-dark)?\.webmanifest$/, dark ? "/manifest-dark.webmanifest" : "/manifest.webmanifest");
      if (href !== link.getAttribute("href")) link.setAttribute("href", href);
    }
  };
  // Light or dark (`theme`: what the switch in the rail chose), or the system's (`system`): on the
  // document at once, kept for the next page (the head script applies it before the first paint),
  // the browser's own chrome with it, and told to the page (`betula:theme`), whose choice shows it.
  const setTheme = (theme) => {
    const chosen = theme === "light" || theme === "dark" ? theme : null;
    if (chosen) root.dataset.theme = chosen;
    else delete root.dataset.theme;
    paintChrome(chosen ? chosen === "dark" : matchMedia("(prefers-color-scheme: dark)").matches);
    try { if (chosen) localStorage.setItem("betula.theme", chosen); else localStorage.removeItem("betula.theme"); } catch {}
    dispatchEvent(new Event("betula:theme"));
  };
  // The system turns dark or light while the page is open (a phone at nightfall): the stylesheet
  // follows by itself, the chrome here, unless a theme of its own was chosen.
  // (Safari before 14 has no `addEventListener` on a media query; there the chrome waits for a page load.)
  matchMedia("(prefers-color-scheme: dark)").addEventListener?.("change", (e) => { if (!root.dataset.theme) paintChrome(e.matches); });

  document.addEventListener("click", (e) => {
    const target = e.target.closest("[data-action]");
    switch (target?.dataset.action) {
      case "theme": {
        const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
        setTheme(dark ? "light" : "dark");
        break;
      }
      case "theme-set":
        // Light, dark or the system's, as the profile page offers them (folia/crates/profile).
        setTheme(target.dataset.themeValue);
        break;
      case "sheet-open":
        e.preventDefault();
        openSheet();
        break;
      case "sheet-close":
        e.preventDefault();
        closeSheet();
        break;
      case "sheet-apply":
        e.preventDefault();
        closeSheet("applied");
        break;
      case "back": {
        // The link leads to the list of the area. If that list is where the visitor came from,
        // go back through the history instead: the same entry as before, and no new one. The app
        // knows and says so (`data-back`); the classic site can only ask the referrer.
        if (cameFrom(target) && history.length > 1) { e.preventDefault(); history.back(); }
        break;
      }
      case "jump": {
        // To a section of the page, without a history entry (Esc and „Zurück" still leave the page).
        const section = byId(target.getAttribute("href").slice(1));
        if (!section) break;
        e.preventDefault();
        // A section that folds (a faculty of the program overview) opens on the way there.
        if (section.tagName === "DETAILS") section.open = true;
        // The panels drawn only near the screen (app.css, `content-visibility`) are drawn for the
        // way there: with only their guessed heights the glide ended off the section.
        root.classList.add("jumping");
        const done = () => root.classList.remove("jumping");
        addEventListener("scrollend", done, { once: true });
        setTimeout(done, 2000);
        section.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "start" });
        break;
      }
      case "fold": {
        // The line of a module's folded Termine („a module's Termine"): open and shut as a thing does.
        const fold = target.parentElement;
        if (!fold?.matches("details.evfold")) break;
        e.preventDefault();
        // On its way it turns back.
        const way = folding.get(foldList(fold) ?? fold);
        if (way ? way.to > 0 : fold.open) shutList(fold);
        else unfoldList(fold);
        break;
      }
      case "termin":
        // A slot of a module's week: its lines in the list under it.
        if (e.button === 0 && !e.ctrlKey && !e.metaKey && !e.shiftKey && !e.altKey && showTermin(target)) e.preventDefault();
        break;
      case "runout": {
        // „Alle … anzeigen" under a list cut short (app.css `.runout`): the rest unfolds below it, and
        // the first of the rest takes the focus the button had, which goes.
        const list = target.closest(".section")?.querySelector(".linklist.runout");
        if (!list || list.classList.contains("all")) break;
        const from = list.offsetHeight;
        list.classList.add("all");
        target.setAttribute("aria-expanded", "true");
        growFold(list, from, list.offsetHeight, FOLD_OPEN);
        list.children[5]?.focus({ preventScroll: true });
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
  const sideWidth = () => (byId("filters") || byId("sidebar"))?.getBoundingClientRect().width ?? 272;
  const RESIZE = {
    "resize-filters": {
      key: "betula.filters.width", prop: "--w-filters", grows: 1,
      min: () => 232, max: () => Math.max(232, Math.min(440, innerWidth * 0.42)),
      panel: () => byId("filters") || byId("sidebar"),
      place: (handle, width) => { if (width === null) handle.style.removeProperty("--at"); else handle.style.setProperty("--at", width + "px"); },
    },
    "resize-preview": {
      key: "betula.preview.width", prop: "--preview-w", grows: -1,
      min: () => 360, max: () => { const work = find(".work"); return work ? Math.max(360, work.clientWidth - sideWidth() - 128) : 2400; },
      panel: () => find(".work > .detail"),
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
    const rows = findAll(".rows a.row");
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
    const nav = find("nav[data-spy]");
    const scroller = byId("page-scroll");
    if (!nav || !scroller) return;
    const links = [...nav.querySelectorAll('a[href^="#"]')];
    const line = scroller.getBoundingClientRect().top + 96;
    let current = links[0];
    for (const link of links) {
      const section = byId(link.getAttribute("href").slice(1));
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
  const flowing = () => document.querySelector("#content > .page-stage.front > .work.flowing");
  const glide = () => (matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth");

  // ---- one scroll area (`.work.flowing`; app.css „one scroll area") ----
  // The page and the ground after it scroll natively as one; nothing here runs while it scrolls
  // but the wood where the browser cannot tie it to the scroll itself. Once the area stands still
  // (150 ms), the pinned panels' content ends above the ground (`--cover`: the ground covers
  // exactly what goes, so nobody sees it) and „Nach oben" stands above it. As the ground goes
  // back down their content follows it in the same frame (owner, 2026-09-30: once the ground was
  // gone the sidebar still waited for the scroll to end): growing under the ground shows nothing
  // that is not to be seen. A page that stands still with the ground up is `data-rest`: the panels
  // the ground cut off end above it with round corners, which the ground draws (owner, 2026-10-04);
  // the first scroll takes them away (owner, 2026-09-30: drawn along with the ground they looked
  // off). The heads of the list's columns stay under the list's head: its height is
  // `--list-head-h`. The scrollbar's width is `--bar`: it stands in the gap right of the page.
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
    area.toggleAttribute("data-rest", covered > 0);
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
    area?.removeAttribute("data-rest");
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
  document.addEventListener("scroll", (e) => {
    if (phone()) return;
    const area = flowing();
    if (e.target !== area) return;
    if (area.hasAttribute("data-rest")) area.removeAttribute("data-rest");
    areaSoon();
  }, { capture: true, passive: true });
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
  // Not so the takeover: the app's page stands where the server's stood, as far down (boot.js), and
  // its button shows as the server's did, at once (it went at the takeover, until the page was
  // scrolled again).
  const newPage = () => {
    const page = pageScroller(), button = document.getElementById("to-top");
    if (page === topPage && button === topButton) return;
    const shown = starting() && Boolean(topButton?.hasAttribute("data-shown"));
    topPage = page;
    topButton = button;
    if (shown && button) {
      button.style.transition = "none";
      button.setAttribute("data-shown", "");
      requestAnimationFrame(() => requestAnimationFrame(() => button.style.removeProperty("transition")));
    } else if (button?.hasAttribute("data-shown") && !topFrame) button.removeAttribute("data-shown");
  };
  new MutationObserver(newPage).observe(document.body, { childList: true, subtree: true });
  // Another stage came to the front (folia/crates/shell/src/stage.rs): its page, its scroll area,
  // as far as it was scrolled; no sheet of the page behind stays open over it.
  addEventListener("betula:stage", () => {
    // A sheet left open on the page that went behind goes, as when Back leaves a page with it.
    for (const sheet of document.querySelectorAll("#content > .page-stage:not(.front) :is(.filters.open, .sidebar.sheet.open)")) {
      sheet.classList.remove("open");
      sheet.dispatchEvent(new Event("betula:sheet-left", { bubbles: true }));
    }
    dropWatch();
    tidySheet();
    areaSoon();
    newPage();
    showTopSoon();
  });
  // A page that loads further down (a reload) or comes back from the browser's memory.
  addEventListener("load", showTopSoon);
  addEventListener("pageshow", (e) => { if (e.persisted) showTopSoon(); });
  // The button stands over the page but is no part of what scrolls: the wheel over it turns the
  // page under it, as over the page itself. A phone scrolls the window anyway. A mouse's notch
  // glides as the browser's own wheel does, but written here a frame at a time: over the virtual
  // list the browser's smooth scrolling ended where the list made up for rows measured anew (a
  // script's scroll ends a smooth one), half its way short (top.mjs). Each frame moves the page
  // by its share of the way, so that what the list makes up in between stays made up; another
  // notch adds its way to what is left. A touchpad's small steps, and less motion: at once.
  let turning = 0; // the next frame of a wheel's glide
  let turn = null; // the page it turns, its way, how much of it is gone, and since when
  const stopTurning = () => {
    cancelAnimationFrame(turning);
    turning = 0;
    turn = null;
  };
  document.addEventListener("wheel", (e) => {
    if (e.ctrlKey || !e.target.closest?.("#to-top")) return;
    const page = pageScroller();
    if (!page || page === document.scrollingElement) return;
    const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaMode === 2 ? e.deltaY * page.clientHeight : e.deltaY;
    if (Math.abs(dy) < 50 || glide() === "instant") { page.scrollBy({ top: dy, behavior: "instant" }); return; }
    const left = turn?.page === page ? turn.way - turn.gone : 0;
    turn = { page, way: left + dy, gone: 0, start: performance.now() };
    if (turning) return;
    const step = (now) => {
      // A frame can have begun before the notch that it is the first frame of.
      const t = Math.min(1, Math.max(0, (now - turn.start) / 200));
      const gone = Math.round(turn.way * (1 - (1 - t) ** 3));
      if (gone !== turn.gone) turn.page.scrollTop += gone - turn.gone;
      turn.gone = gone;
      if (t < 1) turning = requestAnimationFrame(step);
      else stopTurning();
    };
    turning = requestAnimationFrame(step);
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
    stopTurning();
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

  // ---- the navigation's keys ----
  // Every tab of the navigation is a digit away (owner, 2026-10-16: „wenn man ctrl/alt drückt
  // sollen dann die shortcuts bei den nav items angezeigt werden. Wichtig ist, dass … so übliche
  // shortcuts, wie ctrl+c/s/p usw. weiter funktionieren"): 1 the start page, 2 the modules,
  // 3 „Studium", 4 the Merkliste, 5 the Stundenplan (`data-key` of the tabs, chrome.rs `NavItems`),
  // each a click on its tab. With the keys a page has to itself on each system: Alt on Windows, Ctrl
  // and Alt elsewhere (⌃⌥ on a Mac, the keys of a page's own access keys there). Ctrl and a digit
  // chooses the browser's tab everywhere, Alt and a digit Firefox's on Linux, ⌥ and a digit types a
  // character on a Mac; what Ctrl, Alt or ⌘ do with any other key (Ctrl+C, S, P and the rest) stays
  // the browser's and the system's, and so does AltGr. Pressed, the keys show at once what leads to
  // each tab, in full (`data-nav-keys` on the root, `--nav-keys` the words before the digit, app.css;
  // owner, 2026-10-16: „zeig ruhig die shortcuts gleich an, dann findet vlt jemand die auch. und zeig
  // mal die volle kombo. Also "alt + 1" oder so"); another key or the mouse with them, and they go
  // until the keys are let go.
  const system = navigator.userAgentData?.platform || navigator.platform || "";
  const mac = /mac|iphone|ipad|ipod/i.test(system);
  const windows = /^win/i.test(system);
  root.style.setProperty("--nav-keys", JSON.stringify(windows ? "Alt + " : mac ? "⌃ + ⌥ + " : `${language().ctrl} + Alt + `));
  // The navigation's keys and no other: no Shift, no ⌘ or Windows key, no AltGr (on Windows, Ctrl
  // and Alt; a Mac's ⌥ reads as AltGr in Firefox).
  const navHeld = (e) => e.altKey && (windows ? !e.ctrlKey : e.ctrlKey) && !e.shiftKey && !e.metaKey && !(!mac && e.getModifierState?.("AltGraph"));
  const MODIFIERS = new Set(["Alt", "AltGraph", "Control", "Shift", "Meta", "OS", "Fn"]);
  // An Alt held this long was held for the keys: let go, it opens no menu of the browser. Tapped,
  // it is the browser's as ever.
  const NAV_HELD_MS = 250;
  let navSince = 0; // when the keys showed
  let navSpent = false; // another key or the mouse went with the keys
  const navShown = () => root.hasAttribute("data-nav-keys");
  const navHide = () => {
    navSince = 0;
    root.removeAttribute("data-nav-keys");
  };
  // The keys as `e` finds them: shown while the navigation's are held, and gone.
  const navFollow = (e) => {
    if (!navHeld(e) || navSpent) navHide();
    else if (!navShown()) {
      navSince = performance.now();
      root.setAttribute("data-nav-keys", "");
    }
  };
  // The tab of a digit in the navigation on the screen: the rail's, or a phone's bar's.
  const navTab = (digit) => [...document.querySelectorAll(`.rail nav > .nav[data-key="${digit}"], .bottomnav > .nav[data-key="${digit}"]`)].find((tab) => tab.getClientRects().length);
  // A modal dialog has the page to itself, as it has the mouse.
  const modalOpen = () => { try { return !!document.querySelector("dialog:modal"); } catch { return false; } };
  addEventListener("keydown", (e) => {
    const digit = /^Digit([1-9])$/.exec(e.code)?.[1];
    if (digit && navHeld(e)) {
      const tab = !modalOpen() && navTab(digit);
      if (!tab) return;
      e.preventDefault();
      // Once while the key is held: again, a tab leads its area back to where it starts.
      if (!e.repeat) tab.click();
      return;
    }
    // A key held repeats itself, a modifier too (Windows).
    if (e.repeat && MODIFIERS.has(e.key)) return;
    if (MODIFIERS.has(e.key) && !navShown()) navFollow(e);
    else {
      if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) navSpent = true;
      navHide();
    }
  }, true);
  addEventListener("keyup", (e) => {
    if (!e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey) navSpent = false;
    if (navShown() && MODIFIERS.has(e.key) && !navHeld(e) && performance.now() - navSince >= NAV_HELD_MS) e.preventDefault();
    navFollow(e);
  }, true);
  addEventListener("pointerdown", (e) => {
    if (e.altKey || e.ctrlKey || e.metaKey) navSpent = true;
    navHide();
  }, true);
  // Off to another window or the browser's own bar (Alt+Tab, Alt+D): the keys come back let go.
  addEventListener("blur", () => { navSpent = false; navHide(); });
  document.addEventListener("visibilitychange", () => { if (document.hidden) { navSpent = false; navHide(); } });

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
      // One Esc, one thing: nothing it closes here goes on as a close request to a watcher.
      if (filters()?.classList.contains("open")) { e.preventDefault(); closeSheet(); return; }
      if (typing(document.activeElement)) { document.activeElement.blur(); return; }
      // What lies over the page (a menu, the selection, a dialog: folia/crates/shell/src/nav.rs)
      // takes this Esc itself; the module beside the page waits for the next one.
      if (root.hasAttribute("data-layers") || root.hasAttribute("data-state") || modalOpen()) return;
      // The preview first; on a module's own page Esc goes back to where the visitor came from.
      (find('[data-action="close-detail"]') || find('[data-action="back"]'))?.click();
    } else if (plain && (e.key === "f" || e.key === "F") && !typing(document.activeElement)) {
      // Where its button is shown: a phone's sheet has none (app.css).
      const full = find('[data-action="fullscreen"]');
      if (full?.getClientRects().length) { e.preventDefault(); full.click(); }
    } else if (plain && (e.key === "m" || e.key === "M") && !typing(document.activeElement) && appRuns() && !e.target.closest?.("dialog")) {
      // „Merken" for what the visitor is at: the row the keyboard is on, else the module that is
      // open (its preview, or its page). The buttons belong to the browser app.
      const row = document.activeElement?.closest?.(".row-wrap");
      const mark = row ? row.querySelector('[data-action="mark"]') : find('.hero [data-action="mark"]');
      if (mark) { e.preventDefault(); mark.click(); }
    } else if (plain && (e.key === "p" || e.key === "P") && !typing(document.activeElement) && appRuns() && !e.target.closest?.("dialog")) {
      // „Einplanen" for the module that is open (its preview, or its page); rows have no plan
      // button of their own. The keyboard on another row of the list than the open one would make
      // it ambiguous which module is meant, so P waits there. What it planned the note at the
      // bottom says, whose ← and → move it (folia/crates/widgets/src/plannote.rs).
      const row = document.activeElement?.closest?.(".row-wrap");
      const plan = row && !row.querySelector('[aria-current="true"]') ? null : find('.hero [data-action="plan"]');
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
