// Small behaviours shared by the pages of the site and the browser app. The site works without this
// file (it is links); before the app has taken over (`window.__betulaApp`) panels keep their scroll
// position across page loads, and a link followed while the app is starting waits for it. In both
// modes: the keyboard (the regions of a page, the filters as a mode, the stops of Tab, and the keys
// of Betula's own: Esc closes the preview or leaves the module page, F leads into the filters, V
// opens the previewed module full screen, Ctrl+K or "/" jumps to the search, Alt and a digit — Ctrl
// and Alt off Windows — goes to a tab of the navigation; in the app M marks the module the visitor
// is at and P plans it into the Studienplan), the theme switch, the filter sheet (and a page's
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

  // ---- a change of tab on a phone: the pages on an axis (Achse) ----
  // (owner, 2026-10-18, of the ways of the prototype: „Ich muss sagen ich finde achse fühlt sich am
  // besten an. Quasi so, als währe es nicht da aber man versteht trotzdem, wie das funktioniert", and
  // of its page following a finger on the bar: „das verhalten im Prototypen passt für mich"; the next
  // day, of it on a phone: „Wenn man einfach normal auf die Knöpfe drückt, dann ist das von der
  // Animation her super. Aber ich habe das Gefühl, dass die Aktion mit einem kleinen delay
  // ausgeführt wird … Es wäre wirklich perfekt, wenn es sich so anfühlt als würde es sofort los
  // gehen", of a quick swipe „dann sieht die Animation echt ganz schön laggy aus. Das darf auf keinen
  // fall passieren", of one at its usual speed „dann sehe ich die Animation gar nicht. Da sollte es
  // vielleicht ein max speed geben. Aber trotzdem so, dass man dann noch weiter machen kann also
  // nichts blockiert", and of quick taps „dann flackert die seite … Es muss ein Konzept für schnelle
  // aufeinander folgende aktionen geben, so dass es sich flüssig und natürlich anfühlt";
  // folia/design/tabwechsel/konzept.html, „Die Bewegung am Handy: Achse" and „Schnell
  // hintereinander"). Where a phone's view is shown, a change of tab between two pages that stand
  // (stage.rs, `betula:stage-going` and `betula:stage` with `step`) moves them on an axis: the page
  // that comes slides in 36 px from the side of its tab and gets clear over the page that goes,
  // which slides 36 px away to the side of its own tab under it; a far jump the same way, the tabs
  // between staying behind.
  //
  // The pages on their way are layers over the window (app.css `.page-stage.moving`), each as far
  // down as it was scrolled and each as opaque as the window's ground, one over the other with the
  // page they go to on top: what lies under a page shows only as far as that page is not clear yet.
  // So the screen is never empty, and the page that comes is there from the first frame (a fifth of
  // it, half after 45 ms). Every change starts where the pages stand and as fast as they go:
  // another tap, Back or a finger on the bar on their way puts the page it goes to on top, each page
  // still seen as much as it was (`onTop`); however many come, nothing jumps, flickers or waits.
  // Where a page stands and how clear the page on top is are damped springs, worked out ahead and
  // played as Web Animations, which the compositor runs while the app takes its step.
  //
  // A finger along the bar (the swipe below) says where the pages should be: as far along the way
  // to the tab beside as the bar's lens is. The pages follow it on a spring, a whole way in a
  // quarter second at most, so that a quick swipe is seen as a motion and a slow one goes with the
  // finger; let go, they go on from where they are and as fast as they went, to the tab the bar
  // goes to. A finger that comes again on their way takes them where they are, and until it has
  // turned them, what was on its way goes on.
  //
  // Little is left for the moment of a change: a finger on a tab makes the page in front and the
  // tab's page layers while it is down (`axisPrepare`), and the app gets the click of a tap or of a
  // swipe a frame after the pages started (`later`), so that its work does not hold up their first
  // frames; a finger, a key or a click that comes before has the app take it at once.
  //
  // Not on a wide screen (the page is there in the next frame), not where a sheet or a dialog lies
  // over one of the pages, not after the browser animated the step already (Safari's swipe back from
  // the edge, `hasUAVisualTransition`); with less motion a short dissolve, nothing slides. A finger
  // on the page (a wheel, a key) puts the pages at rest at once.
  const AXIS = {
    px: 36, // how far a page slides
    come: { response: 0.3, damping: 1 }, // the spring of the way of the page that comes: s a swing takes, how much of it is damped
    go: { response: 0.13, damping: 1 }, // of the page that goes
    clear: 0.06, // s: how fast the page on top gets clear after a tap, at its full pace from the start (1/e of the rest left)
    let: 0.09, // s: and after a finger on the bar let go, from as fast as it came, no faster
    follow: { response: 0.12, damping: 1 }, // the spring the pages follow a finger on the bar on
    most: 4, // ways a second the pages go at most after a finger: a whole way in a quarter second
  };
  const AXIS_CALM = { ...AXIS, px: 0, come: { response: 0.2, damping: 1 }, go: { response: 0.12, damping: 1 }, clear: 0.05, let: 0.07 };
  const AXIS_DT = 1 / 120; // s from one keyframe to the next
  const axisWay = () => (matchMedia("(prefers-reduced-motion: reduce)").matches ? AXIS_CALM : AXIS);
  const REST = Object.freeze({ x: 0, o: 1 });
  // Where a page is gone to: out to `side` (+1 the right).
  const away = (v, side) => side * v.px;
  const clear01 = (o) => Math.min(1, Math.max(0, o));
  // The pages on their way: each stage's card (its pose, `x` px and `o` how clear, and how fast both
  // go), how they lie one over the other (`order`, the top last), the one they go to (`goal`), what
  // plays (`run`), a finger on the bar (`grip`), the top of the pages in the window (`top`), until
  // when the browser has animated a step (`ua`), and the layers made for a tap that did not come
  // (`idle`).
  const axis = { cards: new Map(), order: [], goal: null, run: null, grip: null, top: 0, ua: 0, idle: 0 };
  const tabOf = (name) => [...document.querySelectorAll(".bottomnav > .nav")].findIndex((tab) => tab.dataset.area === name);
  // The side the tab of `name` lies on, seen from the tab of `of`.
  const side = (name, of) => Math.sign(tabOf(name) - tabOf(of)) || 1;
  const axisStage = (name) => document.querySelector(`#content > .page-stage[data-stage="${CSS.escape(String(name))}"]`);
  const axisOn = () => phone() && appRuns() && typeof Element.prototype.animate === "function" && CSS.supports("content-visibility", "hidden");
  // What lies over a page: a module's sheet, a sheet of filters, a selection's bar, a dialog.
  const overlaid = (el) => !el || Boolean(el.querySelector(".work > .detail:not(.aside), .filters.open, .sidebar.sheet.open, .st-selbar, dialog[open]"));
  const pose = (card, p) => {
    card.pos = p;
    card.el.style.transform = `translate3d(${p.x.toFixed(2)}px, 0, 0)`;
    card.el.style.opacity = String(clear01(p.o));
  };
  const restack = () => axis.order.forEach((name, i) => { const card = axis.cards.get(name); if (card) card.el.style.zIndex = String(2 + i); });
  // How much of each page the eye sees: as clear as it is, of what the pages over it leave.
  const seen = () => {
    const out = new Map();
    let left = 1;
    for (let i = axis.order.length - 1; i >= 0; i--) {
      const card = axis.cards.get(axis.order[i]);
      if (!card) continue;
      const o = clear01(card.pos.o);
      out.set(card.name, o * left);
      left *= 1 - o;
    }
    return out;
  };
  // `card` on top, each page seen as much as it was: on top it is as clear as it was seen, and every
  // page under it as clear as what is left over it lets it be seen as much again. Nothing changes
  // on the screen; what goes on from here is that page getting clear.
  const onTop = (card) => {
    const order = axis.order;
    if (order[order.length - 1] === card.name) return;
    const w = seen(), rest = order.filter((name) => name !== card.name);
    let left = 1 - (w.get(card.name) ?? 0);
    pose(card, { x: card.pos.x, o: w.get(card.name) ?? 0 });
    for (let i = rest.length - 1; i >= 0; i--) {
      const under = axis.cards.get(rest[i]), wi = w.get(rest[i]) ?? 0;
      if (!under) continue;
      pose(under, { x: under.pos.x, o: left > 1e-6 ? Math.min(1, wi / left) : under.pos.o });
      under.vel = { x: under.vel.x, o: 0 };
      left -= wi;
    }
    axis.order = [...rest, card.name];
    restack();
  };
  // The first page that moves: the window keeps room for the farthest any page was scrolled, so that
  // it stays where the page in front is, and the ground under the pages goes with them.
  const axisBegin = () => {
    const content = document.getElementById("content"), front = stage();
    axis.top = content.getBoundingClientRect().top + scrollY;
    const highs = [...content.querySelectorAll(":scope > .page-stage")].map((el) => Number(el.dataset.height) || 0);
    if (front && !front.classList.contains("moving")) highs.push(front.getBoundingClientRect().height);
    content.style.minHeight = `${Math.ceil(Math.max(0, ...highs) + innerHeight)}px`;
    root.classList.add("stages-moving");
  };
  // A stage as a layer of its own, on top of the others: the page in front as it is in the window,
  // one behind as far down and as high as it was when it went (stage.rs, `data-scroll`,
  // `data-height`).
  const lift = (name, at) => {
    const known = axis.cards.get(name);
    if (known) return known;
    const el = axisStage(name), page = el?.querySelector(":scope > .work");
    if (!page) return null;
    const inWindow = el.classList.contains("front");
    const high = inWindow ? el.getBoundingClientRect().height : Number(el.dataset.height) || 0;
    if (inWindow) el.dataset.height = String(high);
    if (!axis.cards.size) axisBegin();
    // Set where they apply, not on the stage: what a stage sets its elements inherit, and each of
    // them would work out its style anew (app.css).
    const ground = page.querySelector(":scope > .ground");
    page.style.paddingTop = `${axis.top}px`;
    ground?.style.setProperty("top", `calc(max(${axis.top + high}px, 100lvh) + 12px)`);
    for (const part of page.children) part.style.flexShrink = "0";
    el.classList.add("moving");
    page.scrollTop = inWindow ? scrollY : Number(el.dataset.scroll) || 0;
    const card = { name, el, page, ground, pos: REST, vel: { x: 0, o: 0 } };
    pose(card, at);
    axis.cards.set(name, card);
    axis.order.push(name);
    restack();
    return card;
  };
  const unlift = (card) => {
    card.el.classList.remove("moving");
    for (const prop of ["transform", "opacity", "z-index"]) card.el.style.removeProperty(prop);
    card.page.style.removeProperty("padding-top");
    card.ground?.style.removeProperty("top");
    for (const part of card.page.children) part.style.removeProperty("flex-shrink");
  };
  const drop = (name) => {
    const card = axis.cards.get(name);
    if (!card) return;
    axis.cards.delete(name);
    axis.order = axis.order.filter((n) => n !== name);
    unlift(card);
    restack();
  };
  // Every card in `plans` from where it is and as fast as it goes: its way to `x` on its `spring`,
  // and clear (`o` 1, where it is not null) on a spring of `clear` s, which after a tap starts at its
  // full pace (`kick`; faster where the page already came faster), after a finger as fast as the
  // page came. Worked out every AXIS_DT s until all rest, and played; then `axisSettle`.
  const axisPlay = (plans) => {
    const h = AXIS_DT / 2;
    const tracks = [...plans].map(([card, plan]) => {
      const x = { ...card.pos }, v = { x: card.vel.x, o: 0 };
      if (plan.o !== null) v.o = plan.kick ? Math.max(card.vel.o, (plan.o - x.o) / plan.clear) : card.vel.o;
      return { card, plan, x, v, at: [], speed: [] };
    });
    for (let f = 0; f < 360; f++) {
      let calm = f > 0;
      for (const tr of tracks) {
        tr.at.push({ ...tr.x });
        tr.speed.push({ ...tr.v });
        const { plan } = tr;
        if (Math.abs(tr.x.x - plan.x) > 0.2 || Math.abs(tr.v.x) > 5 || (plan.o !== null && (Math.abs(tr.x.o - plan.o) > 0.004 || Math.abs(tr.v.o) > 0.1))) calm = false;
      }
      if (calm) break;
      for (let s = 0; s < 2; s++) {
        for (const tr of tracks) {
          const { plan } = tr, w0 = (2 * Math.PI) / plan.spring.response, k = w0 * w0, d = 2 * plan.spring.damping * w0;
          tr.v.x += (-k * (tr.x.x - plan.x) - d * tr.v.x) * h;
          tr.x.x += tr.v.x * h;
          if (plan.o === null) continue;
          const c = 1 / plan.clear;
          tr.v.o += (-c * c * (tr.x.o - plan.o) - 2 * c * tr.v.o) * h;
          tr.x.o += tr.v.o * h;
          // Clear is as clear as it gets: what would go past it stays there.
          if (tr.x.o >= 1) { tr.x.o = 1; tr.v.o = Math.min(0, tr.v.o); }
        }
      }
    }
    for (const tr of tracks) {
      tr.at.push({ x: tr.plan.x, o: tr.plan.o ?? tr.x.o });
      tr.speed.push({ x: 0, o: 0 });
    }
    const n = tracks[0]?.at.length ?? 0;
    if (n < 2) { axisSettle(); return; }
    const timing = { duration: (n - 1) * AXIS_DT * 1000, easing: "linear", fill: "forwards" };
    const anims = tracks.map((tr) => tr.card.el.animate(tr.at.map((p) => ({ transform: `translate3d(${p.x.toFixed(2)}px, 0, 0)`, opacity: clear01(p.o) })), timing));
    // A frame in at its first frame: the frame before showed where it starts (the pages at rest,
    // where a finger or the motion before had them), so the first one shows them on their way.
    for (const anim of anims) anim.currentTime = Math.min(1000 / 60, timing.duration);
    const run = { tracks, anims, n, timer: 0 };
    run.timer = setTimeout(() => { if (axis.run === run) axisSettle(); }, timing.duration + 40);
    axis.run = run;
  };
  // The cards stop where they are just then, as fast as they go.
  const axisHalt = () => {
    const run = axis.run;
    if (!run) return;
    axis.run = null;
    clearTimeout(run.timer);
    const k = Math.max(0, Math.min((run.anims[0]?.currentTime ?? 0) / 1000 / AXIS_DT, run.n - 1)), i = Math.floor(k), j = Math.min(i + 1, run.n - 1), f = k - i;
    const mix = (a, b) => ({ x: a.x + (b.x - a.x) * f, o: a.o + (b.o - a.o) * f });
    for (const tr of run.tracks) {
      if (axis.cards.get(tr.card.name) !== tr.card) continue;
      tr.card.vel = mix(tr.speed[i], tr.speed[j]);
      pose(tr.card, mix(tr.at[i], tr.at[j]));
    }
    for (const anim of run.anims) anim.cancel();
  };
  const stopFollow = () => {
    if (axis.grip?.frame) cancelAnimationFrame(axis.grip.frame);
    if (axis.grip) axis.grip.frame = 0;
  };
  // At rest at once: the page in front in the window, as far down as it showed, the others behind.
  const axisFinish = () => {
    axisHalt();
    stopFollow();
    axis.grip = null;
    clearTimeout(axis.idle);
    for (const card of axis.cards.values()) unlift(card);
    axis.cards.clear();
    axis.order = [];
    axis.goal = null;
    document.getElementById("content")?.style.removeProperty("min-height");
    root.classList.remove("stages-moving");
  };
  // The pages have come to rest; if the app went elsewhere meanwhile (a tab whose page did not
  // stand), they go there first.
  const axisSettle = () => {
    axisHalt();
    const front = stage()?.dataset.stage;
    if (front && front !== axis.goal && axis.cards.has(front) && !axis.grip) aim(front);
    else axisFinish();
  };
  // The pages go to `goal`: it comes on top, from where it is or new from the side of its tab, and
  // gets clear; the page that was the goal goes, quickly, and every other page goes on to the side
  // of its own tab, under it. After a finger on the bar (`drag`) on from as fast as they went.
  const aim = (goal, { drag = false } = {}) => {
    const v = axisWay(), before = axis.goal ?? goal;
    axisHalt();
    clearTimeout(axis.idle);
    const card = lift(goal, { x: away(v, side(goal, before)), o: 0 });
    if (!card) { axisFinish(); return; }
    onTop(card);
    // A page all under the pages over it is not seen: it goes behind at once, one layer less for
    // the screen to draw (quick taps would pile them up). Not the app's page in front.
    const w = seen(), front = stage()?.dataset.stage;
    for (const name of [...axis.order]) if (name !== goal && name !== front && (w.get(name) ?? 0) < 0.004) drop(name);
    const clear = drag ? v.let : v.clear, kick = !drag;
    const plans = new Map([[card, { x: 0, spring: v.come, o: 1, clear, kick }]]);
    // The pages under it go to the side of their tab; one that was still coming gets clear all the
    // same, under it, so that what lies under that one is covered and can go behind.
    for (const other of axis.cards.values()) {
      if (other !== card) plans.set(other, { x: away(v, side(other.name, goal)), spring: other.name === before && before !== goal ? v.go : v.come, o: other.pos.o > 0.001 ? 1 : null, clear, kick });
    }
    axis.goal = goal;
    axisPlay(plans);
  };
  // Whether a change of stages (stage.rs: `from`, `to`, and whether both pages stand, `step`) moves
  // the pages: between the pages of two tabs on a phone, nothing over either of them, not where
  // the browser animated the step already, not while a finger holds the pages.
  const moves = (d) => axisOn() && d.step === true && performance.now() >= axis.ua && !axis.grip?.take && tabOf(d.from) >= 0 && tabOf(d.to) >= 0 && !overlaid(axisStage(d.from)) && !overlaid(axisStage(d.to));
  // Before the app swaps the stages: the page that goes becomes a layer of its own while it is
  // still drawn (hidden and drawn again in the same task, every element of it would work out its
  // style anew), as it is in the window. Where a tap or a finger started the pages already, there
  // is nothing left to do.
  addEventListener("betula:stage-going", (e) => {
    const d = e.detail || {};
    if (!moves(d)) { axisFinish(); return; }
    if (axis.goal !== d.to && lift(d.from, REST)) axis.goal ??= d.from;
  });
  // After: the page that comes, from the side of its tab.
  addEventListener("betula:stage", (e) => {
    const d = e.detail || {};
    if (!moves(d) || (!axis.cards.has(d.from) && axis.goal !== d.to)) { axisFinish(); return; }
    // The page in front follows the window, which the app scrolled to where the page was left.
    const front = axis.cards.get(d.to);
    if (front) front.page.scrollTop = scrollY;
    if (axis.goal !== d.to) aim(d.to);
  });
  addEventListener("scroll", () => {
    const card = axis.cards.size ? axis.cards.get(stage()?.dataset.stage) : null;
    if (card && Math.abs(card.page.scrollTop - scrollY) > 0.5) card.page.scrollTop = scrollY;
  }, { passive: true });
  // The app's click of a tap still to come goes first (`later`, below), so that the pages come to
  // rest where the app is going, not where it was.
  const axisStill = (e) => {
    flushLater();
    if (axis.cards.size && !axis.grip && !e.target?.closest?.(".bottomnav")) axisFinish();
  };
  addEventListener("pointerdown", axisStill, true);
  addEventListener("wheel", axisStill, { capture: true, passive: true });
  addEventListener("keydown", axisStill, true);
  // A step the browser animated already, as Safari does for a swipe back from the edge.
  const animatedStep = (e) => { if (e.hasUAVisualTransition) axis.ua = performance.now() + 300; };
  addEventListener("popstate", animatedStep, true);
  window.navigation?.addEventListener?.("navigate", animatedStep);

  // The app's click of a change the pages have started (a tap's, a swipe's): once they are on their
  // way (their animations started, the compositor running them), so that the app's work does not
  // hold up their first frames; at once when a finger, a key, a wheel (`axisStill`) or another
  // click comes first, in the order they came. A page whose frames have stopped (hidden) has it
  // after 150 ms all the same.
  let later = null;
  const flushLater = () => {
    const job = later;
    if (!job) return;
    later = null;
    cancelAnimationFrame(job.frame);
    clearTimeout(job.timer);
    clearTimeout(job.slow);
    job.link.click();
    // A mark set ahead of the app (`markTab`) goes back to where the app is, where it went nowhere.
    if (job.marked) setTimeout(() => { const front = stage()?.dataset.stage; if (front && front !== job.link.dataset.area && tabOf(front) >= 0) markTab(front); }, 400);
  };
  const sendLater = (link, marked = false) => {
    flushLater();
    const job = { link, marked, frame: 0, timer: 0, slow: 0 };
    const send = () => { if (later === job) flushLater(); };
    const started = axis.run?.anims[0]?.ready;
    if (started) started.then(() => { if (later === job) job.timer = setTimeout(send); }, send);
    else job.frame = requestAnimationFrame(() => { job.timer = setTimeout(send); });
    job.slow = setTimeout(send, 150);
    later = job;
  };

  // The tab of `name` current on the bars at once, as the app has it once it has the click (the
  // start does the same before the app, above): its mark answers the tap in the frame the pages
  // start in (design/tabbar/swipe.html).
  const markTab = (name) => {
    for (const nav of document.querySelectorAll(".bottomnav > .nav, .rail .nav")) {
      if (nav.dataset.area === name) nav.setAttribute("aria-current", "page"); else nav.removeAttribute("aria-current");
    }
  };
  // Whether a tap on the tab of `name` moves the pages, the page in front being that of `cur`.
  const axisTo = (name, cur) => Boolean(name && cur && name !== cur && axisOn() && !axis.grip?.take && tabOf(cur) >= 0 && tabOf(name) >= 0 && performance.now() >= axis.ua && !overlaid(axisStage(cur)) && !overlaid(axisStage(name)) && window.__betulaStands?.(name) === true);
  // A finger on a tab of the bar: the page in front and the pages it may bring become layers while
  // it is down, as a change would make them (`lift`, most of its work before its first frame): on
  // another tab that tab's page (a tap), on the current one the pages of the tabs beside it (a
  // swipe along the bar starts there). Nothing of it is seen: the page in front stands where it
  // stood, the others are not clear yet. Undone when no change came of it.
  const axisPrepare = (tab) => {
    const name = tab?.dataset.area, cur = stage()?.dataset.stage;
    if (!name || !cur || axis.grip) return;
    const tabs = [...document.querySelectorAll(".bottomnav > .nav")], at = tabs.findIndex((t) => t.dataset.area === cur);
    const bring = name !== cur ? [name] : [tabs[at - 1]?.dataset.area, tabs[at + 1]?.dataset.area];
    const ready = bring.filter((n) => axisTo(n, cur));
    if (!ready.length || (!axis.cards.size && !lift(cur, REST))) return;
    for (const n of ready) if (!axis.cards.has(n)) lift(n, { x: away(axisWay(), side(n, axis.goal ?? cur)), o: 0 });
    clearTimeout(axis.idle);
    axis.idle = setTimeout(() => { if (!axis.run && !axis.grip && !later && axis.cards.size) axisFinish(); }, 700);
  };
  // A tap (a click, a key) on a tab of the bar whose page stands: the pages start now, and the app
  // gets the click once they are on their way (`later`). Not the click that ends a swipe (the bar's
  // own, below).
  addEventListener("click", (e) => {
    if (!e.isTrusted || e.defaultPrevented || e.button !== 0 || e.ctrlKey || e.metaKey || e.shiftKey || e.altKey) return;
    const tab = e.target.closest?.(".bottomnav > .nav[data-area]");
    if (!tab || performance.now() < tabSwiped) return;
    flushLater();
    const name = tab.dataset.area, cur = stage()?.dataset.stage;
    // The current tab tapped again (its page goes back to where it starts, chrome.rs `TabAgain`):
    // the pages made ready for nothing go first, the page back in the window.
    if (name === cur && axis.cards.size && !axis.run && !axis.grip) axisFinish();
    if (!axisTo(name, cur)) return;
    if (!axis.cards.size && !lift(cur, REST)) return;
    axis.goal ??= cur;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (tabSwipe?.glide) settleTabs();
    aim(name);
    markTab(name);
    sendLater(tab, true);
  }, true);

  // A finger along the bar (the swipe below) takes the pages once it has the bar (`axisGrab`, `p0`
  // where it took the lens, px from the current tab's middle); each move says where it wants them
  // (`axisDrag`, `q` the lens's way from the current tab in tabs, + to the right), and they follow
  // frame by frame (`axisFollow`); let go, they go on to the tab the bar goes to (`axisRelease`).
  const axisGrab = (g, p0) => {
    stopFollow();
    axis.grip = null;
    const cur = g.tabs[g.current]?.dataset.area, front = stage();
    if (!axisOn() || !cur || front?.dataset.stage !== cur || overlaid(front)) return;
    clearTimeout(axis.idle);
    const q0 = p0 / g.step;
    axis.grip = { g, cur, q0, q: q0, s: Math.max(-1, Math.min(1, q0)), vs: 0, last: 0, frame: 0, take: null, order: null, way: 0, from: 0, roles: null, brought: null };
  };
  const along = (path, u) => {
    if (u <= path[0][0]) return path[0][1];
    for (let k = 1; k < path.length; k++) {
      const [u1, p1] = path[k], [u0, p0] = path[k - 1];
      if (u <= u1) {
        const f = (u - u0) / (u1 - u0 || 1);
        return { x: p0.x + (p1.x - p0.x) * f, o: p0.o + (p1.o - p0.o) * f };
      }
    }
    return path[path.length - 1][1];
  };
  // Where each page goes as the finger takes the bar towards the tab of `name`, `way` (+1 right),
  // from `from` of the way on (taken on its way, the lens between two tabs, a page starts from where
  // it stood, as far along): that page on top, from where it is or new from the side of its tab, to
  // rest and clear; the others to the side of their own tab, under it, as clear as they are.
  const rolesFor = (name, way, from) => {
    const grip = axis.grip, v = axisWay(), roles = new Map();
    let card = axis.cards.get(name);
    if (!card) {
      card = lift(name, { x: away(v, way), o: 0 });
      if (!card) return null;
      grip.brought = name;
    }
    onTop(card);
    for (const [n, c] of axis.cards) {
      const p = { ...c.pos };
      roles.set(n, n === name ? [[from, p], [1, REST]] : [[from, p], [1, { x: away(v, side(n, name)), o: p.o }]]);
    }
    return roles;
  };
  // The pages where the follower `s` (the way they follow the finger on, in tabs) puts them.
  const showGrip = (grip) => {
    const v = axisWay(), want = Math.abs(grip.s) < 1e-4 ? 0 : Math.sign(grip.s), at = grip.g.current + want;
    const name = want && at >= 0 && at < grip.g.tabs.length ? grip.g.tabs[at].dataset.area : null;
    // Only a page that stands comes along (stage.rs, chrome.rs `__betulaStands`).
    const way = name && (axis.cards.has(name) || window.__betulaStands?.(name) === true) ? want : 0;
    const from = want && Math.sign(grip.q0) === want ? Math.min(1, Math.abs(grip.q0)) : 0, u = Math.min(1, Math.abs(grip.s));
    if (!grip.take) {
      // Not yet past where the finger took them: what was on its way goes on.
      if (!want || u <= from + 1e-3) return;
      axisHalt();
      if (!axis.cards.size && !lift(grip.cur, REST)) { stopFollow(); axis.grip = null; return; }
      axis.goal ??= grip.cur;
      grip.take = new Map([...axis.cards].map(([n, card]) => [n, { ...card.pos }]));
      grip.order = [...axis.order];
    }
    if (way !== grip.way || (way && from !== grip.from)) {
      // The other way: what was brought for the first goes, every page stands where it was taken,
      // as they lay.
      if (grip.brought) drop(grip.brought);
      grip.brought = null;
      axis.order = grip.order.filter((n) => axis.cards.has(n));
      restack();
      for (const [n, p] of grip.take) { const card = axis.cards.get(n); if (card) pose(card, p); }
      grip.way = way;
      grip.from = from;
      grip.roles = way ? rolesFor(name, way, from) : null;
    }
    if (grip.roles) {
      for (const [n, path] of grip.roles) { const card = axis.cards.get(n); if (card) pose(card, along(path, u)); }
    } else if (want && !name) {
      // No tab that way: the page gives a little, as the bar does.
      const card = axis.cards.get(grip.cur), p = grip.take.get(grip.cur) ?? REST;
      if (card) pose(card, { x: p.x - (v.px ? want * u * innerWidth * 0.14 : 0), o: p.o });
    }
  };
  // Frame by frame the pages after the finger: a damped spring towards where it wants them, never
  // faster than `most` ways a second.
  const axisFollow = (now) => {
    const grip = axis.grip;
    if (!grip) return;
    grip.frame = 0;
    const v = axisWay(), dt = grip.last ? Math.min(0.05, Math.max(0, (now - grip.last) / 1000)) : 1 / 60;
    grip.last = now;
    const target = Math.max(-1, Math.min(1, grip.q)), w0 = (2 * Math.PI) / v.follow.response, k = w0 * w0, d = 2 * v.follow.damping * w0;
    const n = Math.max(1, Math.ceil(dt * 240)), h = dt / n;
    for (let i = 0; i < n; i++) {
      grip.vs = Math.max(-v.most, Math.min(v.most, grip.vs + (-k * (grip.s - target) - d * grip.vs) * h));
      grip.s += grip.vs * h;
    }
    showGrip(grip);
    if (axis.grip === grip && (Math.abs(grip.s - target) > 1e-3 || Math.abs(grip.vs) > 1e-3)) grip.frame = requestAnimationFrame(axisFollow);
  };
  const axisDrag = (q) => {
    const grip = axis.grip;
    if (!grip) return;
    grip.q = q;
    if (!grip.frame) {
      grip.last = 0;
      grip.frame = requestAnimationFrame(axisFollow);
    }
  };
  // Let go: the pages go on from where they are and as fast as they followed the finger (at most
  // `most` ways a second), to the page the bar goes to (`to`, a tab's index) where it came along,
  // else back.
  const axisRelease = (to) => {
    const grip = axis.grip;
    stopFollow();
    axis.grip = null;
    if (!grip) return;
    if (!grip.take) {
      // Never taken: what was on its way goes on; layers made for nothing go.
      if (axis.cards.size && !axis.run) aim(axis.goal ?? grip.cur);
      return;
    }
    const want = Math.sign(grip.s), u = Math.min(1, Math.abs(grip.s)), dudt = want * grip.vs;
    for (const card of axis.cards.values()) card.vel = { x: 0, o: 0 };
    for (const [n, path] of grip.roles ?? []) {
      const card = axis.cards.get(n), a = Math.max(path[0][0], Math.min(u, 0.98)), pa = along(path, a), pb = along(path, a + 0.02);
      if (card) card.vel = { x: ((pb.x - pa.x) / 0.02) * dudt, o: ((pb.o - pa.o) / 0.02) * dudt };
    }
    // To the page the finger brought or back to where it was; a tab whose page did not come takes
    // its step as a tap does.
    const name = grip.g.tabs[to]?.dataset.area;
    aim(name && axis.cards.has(name) ? name : grip.cur, { drag: true });
  };

  // ---- the bottom bar of a phone: a swipe along it goes to the tab beside the current one ----
  // (owner, 2026-09-30: „wenn man nach links swiped soll ein tab nach links gehen und beim rechts
  // swipe eine tab nach rechts"; the next day the other way round, as a finger expects it: „die
  // ganze Leiste zu bewegen und den selector stehen zu lassen und erst wenn man los lässt geht das
  // dann wieder zur original Location zurück", and of the prototype's ways „nur tabs + ein
  // Element", design/tabbar/swipe.html). A finger that moves along the bar rather than up or down
  // takes the row of tabs with it, inside the bar, which stays; the mark of the current tab stays
  // where it is, so a finger to the left brings the tab on the right under it: as far as that tab,
  // then less and less, and less and less from the start where no tab lies that way. Let go a third
  // of the way there or further, or flicked, and that tab is clicked, and the row springs back to
  // its place with the mark on that tab. Otherwise the row springs back alone. The pages go along
  // with the finger where the tab's page stands (the Achse above; before it, owner, 2026-09-30:
  // „keine Seitenanimationen"); elsewhere the page follows as it follows a tap. Up or down, the bar
  // scrolls the page as before.
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
      axisDrag(d.p / s.g.step);
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
  // as a tap clicks it, a frame after it began (`later`, the Achse above), so that the pages and the
  // lens have their first frames before the app's work: the app takes it (R21: the tab is current
  // at once, its page comes a frame later, `Pending`), before the app the browser loads its page. A
  // finger, a key or a click that comes before that frame has the click go to the app at once,
  // first: a tap right after the swipe, on the tab it left, would else reach the app while the
  // router is still there, change nothing, and be undone by the swipe's page. Once the glide is
  // over and that tab the current one (before the app with the next page, which takes as long as it
  // takes: a few seconds at most, then the lens gives up), the tab's own mark takes over.
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
    if (to !== g.current) sendLater(s.target);
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
    if (tabSwipe !== d.s || !d.bar.isConnected) {
      axisRelease(d.s.g.current);
      if (tabSwipe === d.s) settleTabs();
      return;
    }
    let to = d.s.g.current, v = 0;
    if (x !== null) {
      followTab(d, x, at);
      to = decideTab(d, at);
      v = rowSpeed(d.trail, at);
    }
    // The pages go on first, then the tab's link is clicked: its stages swap behind them.
    axisRelease(to);
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
    // A finger on a tab: its page and the page in front made ready to move, while it is down.
    axisPrepare(e.target.closest?.(".bottomnav > .nav[data-area]"));
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
      axisGrab(g, d.p0);
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
  // Where a sheet let go at `y` going `v` (px per ms, downwards positive) comes to rest, its stops
  // all up (0), half up (`half`) and away (`away`); the module's sheet and the sheet of a choice.
  const goalOf = (y, v, half, away) => {
    const end = y + (v * Math.abs(v)) / (2 * SHEET_FRICTION);
    let to = end < half / 2 ? "full" : end < half + (away - half) / 3 ? "half" : "gone";
    const at = { full: 0, half, gone: away };
    if (v > SHEET_FLICK && at[to] <= y) to = y < half ? "half" : "gone";
    else if (v < -SHEET_FLICK && at[to] >= y) to = y > half ? "half" : "full";
    return to;
  };
  const sheetGoal = (sheet, y, v) => goalOf(y, v, sheetHalf(sheet), sheetAway(sheet));
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
  // A module (`open=`) or a Termin in its own view (`termin=`, folia/crates/widgets/src/termin.rs).
  const asksModule = () => /[?&](open|termin)=/.test(location.search);
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

  // ---- the sheet of a choice in focus on a phone (the Stundenplan's groups) ----
  // (owner, 2026-10-26: „Der Modale Dialog muss auf jeden Fall mit der swipe Geste nach unten
  // geschlossen werden können und mit der swipe Geste nach oben maximiert werden können. Wenn man
  // den Modal maximiert, sollen während dessen die Termine aus der 3 spalten Ansicht in die
  // Vergleichsliste Morphen. Passend zum Progress.") The sheet under the week (section.sp-pick,
  // open.rs PickSheet) stands half up with the groups as tiles, or all up comparing them (`.full`,
  // the address's `vergleich=1`). A finger moves it as it moves the module's sheet: half up, any
  // move up or down; all up, a move down on its head, or on its list where that is at its top, else
  // the list scrolls. Between half up and all up the sheet's top is where the finger has it, and
  // each tile becomes its group's line of „Vergleichen" as far as the sheet has come (`--p`, app.css
  // `.sp-pick.morph`: a copy of the groups over the sheet's body, measured from both its ways as the
  // finger takes it); below half up the sheet goes down with the finger. Let go, it glides on the
  // spring of the module's sheet to where it would come to rest (`goalOf`), the copy with it frame
  // by frame (its cards are laid out anew in each, which the compositor cannot do): all up, the page
  // follows to „Vergleichen" (its link „Vergleichen ›"), half up back to the tiles (‹), away its ×
  // is followed, as „Fertig" is. „Vergleichen ›", ‹ and a tap on its grabber take the same way.
  const picks = document.getElementsByClassName("sp-pick");
  const livePick = () => {
    for (const el of picks) if (el.tagName === "SECTION" && !el.closest(".pending-page") && !el.closest("#content > .page-stage:not(.front)")) return el;
    return null;
  };
  const pickSheet = () => (phone() && appRuns() ? livePick() : null);
  // Where its top is all up: under the safe area and 10 px more (app.css `.sp-pick.full`).
  const pickTop = () => {
    const probe = document.createElement("div");
    probe.style.cssText = "position:fixed;left:0;top:calc(env(safe-area-inset-top, 0px) + 10px);width:1px;height:1px;visibility:hidden;pointer-events:none";
    document.body.append(probe);
    const top = probe.getBoundingClientRect().top;
    probe.remove();
    return top;
  };
  let pickMorph = null; // the sheet off its stops: { sheet, layer, top, half, away, y }
  let pickGlide = null; // on its way: { m, to, ys, at, frame }
  let pickTouch = null; // the finger on it
  // The sheet held where it stands, the two ways of its body laid out unseen under a copy of its
  // groups (app.css `.sp-pick.morph`): each group a card from its tile (from „+N" where the tiles
  // are folded) to its line, and what the tiles alone have, to fade; measured from the copy's
  // corner. Half up it stands as tall as with its tiles: all up, less its list and with the tiles
  // in their place.
  const morphPick = (sheet) => {
    const body = sheet.querySelector(":scope > .sp-pick-body");
    const grp = body?.querySelector(":scope > .grp"), list = body?.querySelector(":scope > .sp-pick-list");
    if (!grp || !list) return null;
    const full = sheet.classList.contains("full");
    const box = sheet.getBoundingClientRect();
    const top = pickTop();
    sheet.style.top = box.top + "px";
    sheet.style.maxHeight = "none";
    sheet.style.setProperty("--p", full ? "1" : "0");
    sheet.classList.add("morph");
    const layer = document.createElement("div");
    layer.className = "sp-morph grp";
    layer.setAttribute("aria-hidden", "true");
    layer.inert = true;
    body.append(layer);
    const o = layer.getBoundingClientRect();
    const rect = (el) => { const r = el.getBoundingClientRect(); return [r.left - o.left, r.top - o.top, r.width, r.height]; };
    const place = (el, k, [x, y, w, h]) => {
      el.style.setProperty(`--x${k}`, x.toFixed(2) + "px");
      el.style.setProperty(`--y${k}`, y.toFixed(2) + "px");
      el.style.setProperty(`--w${k}`, w.toFixed(2) + "px");
      el.style.setProperty(`--h${k}`, h.toFixed(2) + "px");
    };
    const fade = (el, track) => {
      if (!el) return;
      const f = document.createElement("div");
      f.className = track ? "mf track" : "mf";
      place(f, 0, rect(el));
      if (!track) f.append(el.cloneNode(true));
      layer.append(f);
    };
    fade(grp.querySelector(".grp-seg"), true);
    fade(grp.querySelector(":scope > .grp-h"));
    fade(grp.querySelector(":scope > .grp-n"));
    const more = grp.querySelector(".gs.more");
    fade(more);
    const tiles = new Map([...grp.querySelectorAll(".gs.pick[data-g]")].map((tile) => [tile.dataset.g, tile]));
    const lines = [...list.querySelectorAll(".sp-cmp-row[data-g]")];
    // The cards' ways overlap on the sheet's: the last one starts first, the first one last (app.css).
    const spread = lines.length > 1 ? 0.3 : 0;
    lines.forEach((line, i) => {
      const tile = tiles.get(line.dataset.g);
      const card = document.createElement("div");
      card.className = line.classList.contains("on") ? "mc on" : "mc";
      card.style.setProperty("--s", (spread * (lines.length - 1 - i) / Math.max(1, lines.length - 1)).toFixed(3));
      card.style.setProperty("--span", (1 - spread).toFixed(3));
      place(card, 0, rect(tile || more || line));
      place(card, 1, rect(line));
      const clip = document.createElement("div");
      clip.className = "mc-clip";
      clip.append(line.cloneNode(true));
      card.append(clip);
      if (tile) card.append(tile.cloneNode(true));
      layer.append(card);
    });
    const bodyH = body.getBoundingClientRect().height, tilesH = grp.getBoundingClientRect().height;
    const halfTop = full ? Math.max(top + 40, innerHeight - (box.height - bodyH + tilesH)) : box.top;
    // The week behind is as tall as the sheet half up leaves it (app.css `--pick-h`, which open.rs
    // sets half up): the sheet opened all up has not set it yet.
    if (full) root.style.setProperty("--pick-h", Math.round(innerHeight - halfTop) + "px");
    return { sheet, layer, top, half: Math.max(1, halfTop - top), away: Math.max(1, innerHeight - top), y: box.top - top };
  };
  // The sheet `y` px down from all up: its top there down to half up, the copy as far on its way
  // (`--p`); further down it goes down as it stands, and above all up only a little.
  const putPick = (m, y) => {
    m.y = y;
    const top = y < 0 ? -band(-y, 12) : Math.min(y, m.half);
    m.sheet.style.top = dots(m.top + top) + "px";
    m.sheet.style.transform = y > m.half ? `translateY(${dots(y - m.half)}px)` : "";
    m.sheet.style.setProperty("--p", unit(1 - top / m.half).toFixed(4));
  };
  // At a stop: the copy goes and the sheet's classes hold it (app.css).
  const restPick = (m) => {
    if (pickMorph === m) pickMorph = null;
    m.layer.remove();
    m.sheet.classList.remove("morph");
    for (const prop of ["top", "max-height", "transform", "--p"]) m.sheet.style.removeProperty(prop);
  };
  const stopPick = () => {
    const g = pickGlide;
    if (!g) return;
    pickGlide = null;
    cancelAnimationFrame(g.frame);
  };
  // A finger takes the sheet where it is, on its way too.
  const holdPick = (sheet) => {
    stopPick();
    if (pickMorph && pickMorph.sheet !== sheet) restPick(pickMorph);
    if (!pickMorph) pickMorph = morphPick(sheet);
    return pickMorph;
  };
  // There: all up and half up the page follows where it is not there yet (the sheet's link), and
  // the copy goes once the sheet's classes say the same, so that what the copy showed is what the
  // page shows then. Away, its × is followed, and the sheet goes with the choice; should it stay,
  // it comes back half up.
  const arrivePick = (m, to) => {
    const sheet = m.sheet;
    if (!sheet.isConnected) { if (pickMorph === m) pickMorph = null; return; }
    if (to === "gone") {
      sheet.querySelector(':scope > .sp-pick-head [data-action="close-focus"]')?.click();
      setTimeout(() => { if (sheet.isConnected && pickMorph === m && !pickGlide && !pickTouch) glidePick(m, "half", 0); }, 1500);
      return;
    }
    const full = to === "full";
    if (sheet.classList.contains("full") === full) { restPick(m); return; }
    const done = () => {
      watch.disconnect();
      clearTimeout(late);
      if (pickMorph === m && !pickGlide && !pickTouch) restPick(m);
    };
    const watch = new MutationObserver(() => { if (!sheet.isConnected || sheet.classList.contains("full") === full) done(); });
    watch.observe(sheet, { attributes: true, attributeFilter: ["class"] });
    const late = setTimeout(done, 1500);
    (full ? sheet.querySelector(":scope > .sp-pick-body > .grp .go2") : sheet.querySelector(":scope > .sp-pick-head > .ib:not(.end)"))?.click();
  };
  // From where it is to `to` ("full", "half" or "gone"), let go going `v` px per ms, on the
  // module's sheet's spring.
  const glidePick = (m, to, v) => {
    stopPick();
    const calm = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const goal = to === "full" ? 0 : to === "half" ? m.half : m.away + SHEET_PAST;
    let ys = springTo(m.y, calm ? 0 : v * 1000, goal, calm ? SHEET_CALM : to === "gone" ? SHEET_AWAY : SHEET_SPRING);
    if (to === "full") ys = ys.map((y) => Math.max(0, y));
    const out = to === "gone" ? ys.findIndex((y) => y >= m.away) : -1;
    if (out > 0) ys = ys.slice(0, out + 1);
    const g = (pickGlide = { m, to, ys, at: null, frame: 0 });
    const step = (now) => {
      if (pickGlide !== g) return;
      if (!m.sheet.isConnected) { pickGlide = null; if (pickMorph === m) pickMorph = null; return; }
      if (g.at == null) g.at = now;
      const k = (now - g.at) / 1000 / TAB_DT;
      putPick(m, sampleAt(ys, k));
      if (k < ys.length - 1) { g.frame = requestAnimationFrame(step); return; }
      pickGlide = null;
      arrivePick(m, to);
    };
    g.frame = requestAnimationFrame(step);
  };
  document.addEventListener("touchstart", (e) => {
    pickTouch = null;
    // A sheet that went with its choice leaves nothing to hold.
    if (pickMorph && !pickMorph.sheet.isConnected) { stopPick(); pickMorph = null; }
    const sheet = pickSheet();
    if (!sheet || e.touches.length !== 1 || !sheet.contains(e.target) || moduleSheet()) return;
    const t = e.touches[0];
    const list = sheet.querySelector(":scope > .sp-pick-body > .sp-pick-list");
    pickTouch = {
      sheet, x: t.clientX, y: t.clientY, at: e.timeStamp, mode: null, from: 0, trail: [],
      // All up and at rest there: its list scrolls.
      full: sheet.classList.contains("full") && !pickMorph, inList: Boolean(list?.contains(e.target)), scrolled: list?.scrollTop || 0,
      grabber: t.clientY - sheet.getBoundingClientRect().top < 26,
    };
  }, { passive: true });
  const movePick = (e) => {
    const s = pickTouch;
    if (!s) return;
    if (e.touches.length !== 1 || !s.sheet.isConnected) { pickTouch = null; return; }
    const t = e.touches[0];
    const dx = t.clientX - s.x, dy = t.clientY - s.y;
    if (!s.mode) {
      if (s.full && s.inList) {
        // On its list all up: the list's, but where it is at its top a move down takes the sheet.
        if (Math.abs(dx) > Math.abs(dy) || !e.cancelable || dy <= 0 || s.scrolled > 0) s.mode = "own";
      } else if (Math.abs(dx) < TAB_SLOP / 2 && Math.abs(dy) < TAB_SLOP / 2) {
        // Not yet a move: a tap on a tile may still be one, and nothing under the sheet scrolls.
        if (e.cancelable) e.preventDefault();
        return;
      } else if (!e.cancelable) s.mode = "own"; // the browser scrolls already
      else if (Math.abs(dx) > Math.abs(dy)) s.mode = "hold"; // sideways: nothing moves
      if (!s.mode) {
        // The finger takes the sheet: under the point it touched, on its way where it is.
        const m = holdPick(s.sheet);
        s.mode = m ? "drag" : "hold";
        s.from = m?.y ?? 0;
      }
    }
    if (s.mode === "own") return;
    if (e.cancelable) e.preventDefault();
    if (s.mode !== "drag" || !pickMorph) return;
    const y = s.from + dy;
    putPick(pickMorph, y);
    s.trail.push([e.timeStamp, y]);
    while (s.trail.length > 2 && e.timeStamp - s.trail[0][0] > 100) s.trail.shift();
  };
  document.addEventListener("touchmove", movePick, { passive: false });
  document.addEventListener("touchend", (e) => {
    if (e.touches.length) return;
    const s = pickTouch;
    pickTouch = null;
    if (!s || !s.sheet.isConnected) return;
    if (!s.mode && s.grabber && e.timeStamp - s.at < 400) {
      // A tap on its grabber: all up, or back half up.
      const going = pickGlide?.to ?? (s.sheet.classList.contains("full") ? "full" : "half");
      const m = holdPick(s.sheet);
      if (m) glidePick(m, going === "full" ? "half" : "full", 0);
      return;
    }
    if (s.mode !== "drag" || !pickMorph) return;
    const m = pickMorph, y = s.trail.length ? s.trail[s.trail.length - 1][1] : s.from;
    const v = fingerSpeed(s.trail, e.timeStamp);
    glidePick(m, goalOf(Math.max(0, y), v, m.half, m.away), v);
  });
  document.addEventListener("touchcancel", () => {
    const s = pickTouch;
    pickTouch = null;
    if (s?.mode !== "drag" || !pickMorph) return;
    // Taken from the finger: to the stop it is nearest, but not away.
    const m = pickMorph, to = goalOf(Math.max(0, m.y), 0, m.half, m.away);
    glidePick(m, to === "gone" ? "half" : to, 0);
  });
  // „Vergleichen ›" and ‹ take the same way, the tiles turning into the lines and back.
  document.addEventListener("click", (e) => {
    if (!e.isTrusted || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    const sheet = pickSheet();
    const link = sheet ? e.target.closest?.(".sp-pick-body > .grp .go2, .sp-pick-head > .ib:not(.end)") : null;
    if (!link || !sheet.contains(link)) return;
    const m = holdPick(sheet);
    if (!m) return;
    e.preventDefault();
    e.stopPropagation();
    glidePick(m, link.classList.contains("go2") ? "full" : "half", 0);
  }, true);
  // A window that grows past a phone's width: the sheet goes as the page has it.
  narrow.addEventListener("change", () => {
    stopPick();
    if (pickMorph) restPick(pickMorph);
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
      // A page of panels runs under the bar on a wide screen: what it shows starts below the bar
      // (its `scroll-padding`).
      const r = scroller.getBoundingClientRect(), under = parseFloat(getComputedStyle(scroller).scrollPaddingTop) || 0;
      return { top: r.top - lift + scroller.clientTop + under, bottom: Math.min(innerHeight, r.top - lift + scroller.clientTop + scroller.clientHeight) };
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

  // Betula's single keys on or off (folia/crates/design/src/keys.rs, `LettersSwitch`): kept by this
  // browser, read by the script in the head before the page is drawn.
  const setLetters = (off) => {
    try { if (off) localStorage.setItem("betula.letters.v1", "off"); else localStorage.removeItem("betula.letters.v1"); } catch {}
    if (off) root.dataset.letters = "off";
    else delete root.dataset.letters;
    dispatchEvent(new Event("betula:letters"));
  };

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
      case "letters-set":
        setLetters(target.dataset.lettersValue === "off");
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
      case "share-link":
      case "copy-link":
      case "copy-text": {
        // The address of the page, or what the control carries (`data-text`: the marked modules
        // as text, or with `data-absolute` a path of this site that becomes a whole address).
        // „Teilen" (`share-link`, a Termin's: folia/crates/widgets/src/termin.rs) hands the address
        // to the device's own share sheet where a touch screen has one, else copies it as
        // „Link kopiert" does.
        e.preventDefault();
        const label = target.querySelector("[data-label]") || target.querySelector("span");
        const action = target.dataset.action;
        const link = action !== "copy-text";
        const carried = target.dataset.text || "";
        const text = action === "copy-link" ? location.href : "absolute" in target.dataset ? new URL(carried, location.origin).href : carried;
        if (action === "share-link" && navigator.share && matchMedia("(pointer: coarse)").matches) {
          navigator.share({ title: target.dataset.title || document.title, url: text }).catch(() => {});
          break;
        }
        navigator.clipboard?.writeText(text).then(() => {
          if (!label || "was" in label.dataset) return;
          label.dataset.was = label.textContent;
          label.textContent = link ? language().linkCopied : language().copied;
          target.classList.add("said");
          setTimeout(() => { label.textContent = label.dataset.was; delete label.dataset.was; target.classList.remove("said"); }, 1600);
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
    // 96 px below where the page shows: below the bar, where a page of panels runs under it.
    const line = scroller.getBoundingClientRect().top + (parseFloat(getComputedStyle(scroller).scrollPaddingTop) || 0) + 96;
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
  // `--list-head-h`; „Mein Studium"'s semester is such a list in the page, under the overview
  // (`LISTS`, app.css „Mein Studium"). The scrollbar's width is `--bar`: it stands in the gap right
  // of the page.
  // A phone scrolls the window, and none of this applies there (app.css has all of it for wide
  // screens only): it measured every page as it was built, a layout in each of its frames, and gave
  // each new page `--bar` and `--cover`, which restyled the whole page once more; a quarter of what
  // a tab switch cost a phone. `--bar` there is the top bar's height, too, and `0px` on the page took
  // it from the skeleton of the rows (app.css, `.rows-pending`).
  const tiesWood = CSS.supports("animation-timeline: scroll()") && CSS.supports("timeline-scope: --page");
  // The list of the area and its head: the catalog's and the Merkliste's, or „Mein Studium"'s
  // semester (its rows `.st-rows`).
  const LISTS = ":scope > .list, :scope > .page > .page-inner.study > .st-sems";
  const LIST_HEADS = ":scope > .list > .list-head, :scope > .page > .page-inner.study > .st-sems .st-sem-head";
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
    area?.querySelector(LISTS)?.classList.remove("short");
    document.getElementById("to-top")?.style.removeProperty("--lift");
    covered = 0;
    coveredArea = null;
    if (woodMoved) { document.querySelector(".wood")?.style.removeProperty("transform"); woodMoved = false; }
  };
  const areaFollow = () => {
    areaFrame = 0;
    if (phone()) return;
    takeBarWheel();
    const area = flowing();
    // A page that comes anew in the same place (the start page as the day comes) stands at its top
    // without a scroll: the frost asks for the page again.
    if (area !== frostArea) frostSoon();
    const head = area?.querySelector(LIST_HEADS) ?? null;
    if (head !== headSeen) { headWatch.disconnect(); if (head) headWatch.observe(head); headSeen = head; }
    if (area) {
      const bar = area.offsetWidth - area.clientWidth;
      if (area.style.getPropertyValue("--bar") !== bar + "px") area.style.setProperty("--bar", bar + "px");
      if (covered) { const cover = coverOf(area); if (cover < covered) putCover(area, cover); }
    }
    // The times and facts beside a module's text go along with it (app.css `.module-side`); where
    // they are taller than the area shows, they go up with the page until their end shows, 8 px
    // above the area's, and stay there.
    const side = area?.querySelector(".module-grid > aside > .module-side");
    if (side) {
      const room = area.clientHeight - (parseFloat(getComputedStyle(area).paddingTop) || 0) - 8;
      const stick = Math.min(0, Math.round(room - side.offsetHeight)) + "px";
      if (side.style.getPropertyValue("--stick") !== stick) side.style.setProperty("--stick", stick);
    }
    // A list short enough to stand whole above the ground is pinned like the filter panel: the
    // ground slides over its empty end and its rows stay where they are, as before. A longer one
    // flows with the page and its end comes before the ground.
    const list = area?.querySelector(LISTS);
    if (list) {
      const rows = list.querySelector(".rows, .st-rows"), last = rows?.lastElementChild;
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
  // How high the page shows: a page of panels runs under the bar on a wide screen, and shows what
  // is below it (its `scroll-padding`).
  const shownOf = (page) => page.clientHeight - (page === document.scrollingElement ? 0 : parseFloat(getComputedStyle(page).scrollPaddingTop) || 0);
  let topFrame = 0;
  let topPage = null; // the page and the button it was last worked out for
  let topButton = null;
  const showTop = () => {
    topFrame = 0;
    topPage = pageScroller();
    topButton = document.getElementById("to-top");
    if (!topButton) return;
    const far = Boolean(topPage) && topPage.scrollTop > shownOf(topPage);
    if (topButton.hasAttribute("data-shown") !== far) topButton.toggleAttribute("data-shown", far);
  };
  const showTopSoon = () => { if (!topFrame) topFrame = requestAnimationFrame(showTop); };
  document.addEventListener("scroll", showTopSoon, { capture: true, passive: true });
  addEventListener("resize", showTopSoon);

  // The frost under the bar (app.css `.frost`): once the page is scrolled, a blur lies under the
  // crown and the search over what runs beneath (`data-scrolled`), on a phone as on a wide screen. On the start
  // page the crown's leaves wait while its photo lies under the search, and slide down from above
  // once it has left the bar, the whole way at once, and back up as soon as it comes back under
  // it (owner, 2026-10-26: „dass sich die blätter von oben rein schieben beim handy"; on a wide
  // screen the same: „die animation für die Blätter gibt es auch immer noch nicht"; and then:
  // „dass die Animation nicht mit dem scrollen mitläuft, sondern dass es einen breaking point
  // gibt, wo die dann umschaltet"; until then they came as far as the page had gone on past the
  // photo, over their own height of the scroll): `--leaves` on the crown and the frost, from 0 to
  // 1, the frost's colour coming with them, the whole way in 150 ms (LEAVES_FAST; „Ich will schon
  // dass sich das snappy anfühlt. Aber dass wenn man mal bild runter drückt das nicht sofort da
  // ist, vielleicht so 100-200ms"), so that a quick swipe or Bild ↓ shows them come instead of
  // just being there. On every other page they are there (1), and a page that comes in front
  // brings them as fast. Whatever the system's setting on motion: where less of it was wanted they were there or
  // not, at once, and the owner, who has it so, took that for no animation at all („das mit
  // weniger bewegung war richtig. Aber ich finde das sollte nicht dazu gehören"). On the server's
  // page and in the app alike, a page in front of another as well (its stage gives the window its
  // own scroll); the app's crown, new at the takeover, is told where they are as it comes (the
  // body's children).
  const LEAVES_FAST = 1000 / 150; // the whole way per second: in 150 ms
  let leavesShown = null;
  let leavesTarget = 1;
  let leavesFrame = 0;
  let leavesAt = 0;
  let frostBar = "0px"; // the width of the page's scrollbar, which the frost leaves clear (app.css)
  const leavesPut = () => {
    const value = String(Math.round(leavesShown * 1000) / 1000);
    for (const el of document.querySelectorAll("body > .crown, body > .frost")) {
      if (el.style.getPropertyValue("--leaves") !== value) el.style.setProperty("--leaves", value);
      if (el.classList.contains("frost") && el.style.getPropertyValue("--scrollbar") !== frostBar) el.style.setProperty("--scrollbar", frostBar);
    }
  };
  const leavesStep = (now) => {
    leavesFrame = 0;
    const seconds = leavesAt ? Math.min(now - leavesAt, 50) / 1000 : 1 / 60;
    leavesAt = now;
    const most = LEAVES_FAST * seconds;
    const gap = leavesTarget - leavesShown;
    leavesShown = Math.abs(gap) <= most ? leavesTarget : leavesShown + Math.sign(gap) * most;
    leavesPut();
    if (leavesShown !== leavesTarget) leavesFrame = requestAnimationFrame(leavesStep);
    else leavesAt = 0;
  };
  const leavesTo = (target, still) => {
    leavesTarget = target;
    if (leavesShown === null || still) {
      leavesShown = target;
      cancelAnimationFrame(leavesFrame);
      leavesFrame = 0;
      leavesAt = 0;
    }
    leavesPut();
    if (leavesShown !== leavesTarget && !leavesFrame) leavesFrame = requestAnimationFrame(leavesStep);
  };
  // On a wide screen a page of panels runs under the bar (app.css), and the frost lies behind the
  // bar once it is scrolled; not a list's page nor „Mein Studium"'s semesters, whose heads stay at
  // the top (a browser without `:has` has none of it).
  const runsUnderBar = (area) => {
    try { return area.matches(":not(:has(> .list, > .page > .page-inner.study > .st-sems))"); } catch { return false; }
  };
  let frostFrame = 0;
  let frostArea = null; // the page the frost was last worked out for (a wide screen's)
  const frost = () => {
    frostFrame = 0;
    const area = phone() ? null : flowing();
    frostArea = area;
    const scrolled = phone() ? scrollY > 4 : Boolean(area) && area.scrollTop > 4 && runsUnderBar(area);
    if (root.hasAttribute("data-scrolled") !== scrolled) root.toggleAttribute("data-scrolled", scrolled);
    frostBar = (area ? area.offsetWidth - area.clientWidth : 0) + "px";
    let gone = true;
    // The start page's photo and the bar's lower edge: a phone's crown, a wide screen's head (where
    // the page shows, its `scroll-padding`); the photo has left the bar once its lower edge is past
    // it, and the leaves come then.
    const hero = [...(stage() || document).querySelectorAll(".day-hero")].find((el) => el.offsetParent !== null);
    const edge = !hero ? undefined : phone() ? document.querySelector(".crown")?.getBoundingClientRect().bottom : area ? area.getBoundingClientRect().top + (parseFloat(getComputedStyle(area).scrollPaddingTop) || 0) : undefined;
    if (edge !== undefined) gone = edge - hero.getBoundingClientRect().bottom >= -1;
    if (root.hasAttribute("data-photo-gone") !== gone) root.toggleAttribute("data-photo-gone", gone);
    leavesTo(gone ? 1 : 0, false);
  };
  const frostSoon = () => { if (!frostFrame) frostFrame = requestAnimationFrame(frost); };
  addEventListener("scroll", frostSoon, { passive: true });
  document.addEventListener("scroll", (e) => { if (!phone() && e.target === flowing()) frostSoon(); }, { capture: true, passive: true });
  addEventListener("resize", frostSoon);
  addEventListener("betula:stage", frostSoon);
  // The start page filled with a Termin and back (home/today.rs): its photo comes and goes.
  addEventListener("betula:frost", frostSoon);
  new MutationObserver(frostSoon).observe(document.body, { childList: true });
  frost();

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
  const turnPage = (page, e) => {
    const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaMode === 2 ? e.deltaY * shownOf(page) : e.deltaY;
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
  };
  document.addEventListener("wheel", (e) => {
    if (e.ctrlKey || !e.target.closest?.("#to-top")) return;
    const page = pageScroller();
    if (!page || page === document.scrollingElement) return;
    turnPage(page, e);
  }, { passive: true });
  // A page of panels runs under the bar on a wide screen (app.css), and the wheel over the bar and
  // the room around it, where the page shows, turns it as well, as over the button. The browser
  // turned it there only now and then (once from the page's top, then no more: the bar is no part
  // of what scrolls), so the bar takes the wheel itself: a listener on the bar alone, as it holds
  // the wheel back, so that the wheel anywhere else does not wait for the page's script. Not a
  // list's page nor „Mein Studium"'s semesters, which do not run under the bar: there, as over the
  // rail, the wheel over the bar does nothing. The app's bar is new at the takeover: whichever
  // bar there is is asked for as the area follows the page (`areaFollow`).
  let wheelBar = null;
  const barWheel = (e) => {
    if (e.ctrlKey || phone() || Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
    const page = flowing();
    if (!page || !runsUnderBar(page)) return;
    e.preventDefault();
    turnPage(page, e);
  };
  function takeBarWheel() {
    const bar = document.querySelector("body > .main > .topbar");
    if (bar === wheelBar) return;
    wheelBar?.removeEventListener("wheel", barWheel);
    wheelBar = bar;
    bar?.addEventListener("wheel", barWheel, { passive: false });
  }
  takeBarWheel();
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
    const from = Math.min(page.scrollTop, shownOf(page));
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

  // ---- the keyboard (folia/design/tastatur/konzept.html; docs/folia/frontend.md R24) ----
  // Betula's keys as a system: a page says what its parts are, and the keys follow from that.
  //
  // - `data-region` names a page's regions: `nav` (the rail, a phone's bar), `search`, `side` (a
  //   page's sidebar, the catalog's filters) and `content`. Ctrl + F6 goes from one to the next
  //   (with Shift back; F6 alone stays the browser's), F into `side`, / and Ctrl + K into the
  //   search, Esc out of the others into the content.
  // - `data-mode` makes a region a mode (owner, 2026-10-20: „Ich will nur die sidebar (z.B. filter)
  //   mit label und rahmen hervorgehoben haben um das als mode zu visualisieren bei dem rest soll das
  //   nicht passieren"): while the keyboard is in it, it wears a black ring and „Esc zum Verlassen"
  //   (after a key only, until the mouse: `data-kb` on the root), Tab goes from `data-section` to
  //   section and stops at either end (the focus only nudges, a screen reader hears it once), the
  //   arrows go from control to control across the sections (↑ ↓ by where they stand, keeping the
  //   column; ← → in reading order; an option group's ← → choose), letters typed find a filter (a
  //   green bubble), the section with the keyboard is lit as the open row of a list is, and a bar
  //   at its foot names the keys of the control that no button can carry. Esc or a click leads out.
  // - `data-stop` makes a group one stop of the Tab key (the rail, the list's rows, the heads of the
  //   columns, a module's buttons): Tab comes in where the keyboard was last in it (else at the
  //   current one, else the first in view) and leaves past it; the arrows of its axis (`x` ← →,
  //   `y` ↑ ↓) go from one to the next. A mode is one stop as well, seen from outside.
  // - `data-cycle` makes Tab go round within a region (the catalog's content: owner, 2026-10-20: „so
  //   dass man mit n mal tab dann wieder beim content landet"), past the regions inside it.
  // - `data-kb-as` lets a control stand for another where the arrows go (the field a digit opens
  //   above a knob of the credits stands for its knob).
  // What a control does with a key itself (a chip's + − Entf, a knob's + − and digits, a picker's
  // keys: Rust) it takes (`defaultPrevented`), and that key is not looked at here. Betula's single
  // keys (F, V, M, P, /, ?, with Shift too) can be switched off (`data-letters="off"`, WCAG 2.1.4,
  // `folia_design::keys::LettersSwitch`); typing in a mode finds all the same, as it only works there.
  const KB = {
    de: {
      leave: (key) => `${key}zum Verlassen`,
      find: "Finden:",
      findIn: (name) => `${name} durchsuchen`,
      none: "nichts",
      like: "ähnlich",
      noneSaid: "Nichts heißt so",
      likeSaid: "Ähnlich: ",
      of: (n, m) => `${n} von ${m}`,
      section: (name) => `Abschnitt ${name}`,
      end: "Ende.",
      start: "Anfang.",
      opened: (name) => `${name} aufgeklappt`,
      bar: {
        chip3: [["+", "mit"], ["−", "ohne"], ["Del", "aus"]],
        chip2: [["+", "an"], ["Del", "aus"]],
        radio: [["← →", "wählen"]],
        person: [["← →", "mit, ohne"], ["Del", "entfernen"]],
        slider: [["+", "mehr"], ["−", "weniger"], ["0–9", "Zahl"]],
        find: [["↑ ↓", "Treffer"], ["Enter", "mit"], ["Shift+Enter", "ohne"]],
      },
      names: { Mod: "Strg", Ctrl: "Strg", Del: "Entf", Home: "Pos1", End: "Ende", PgUp: "Bild ↑", PgDn: "Bild ↓", Space: "Leertaste" },
    },
    en: {
      leave: (key) => `${key}to leave`,
      find: "Find:",
      findIn: (name) => `Search ${name}`,
      none: "nothing",
      like: "similar",
      noneSaid: "Nothing is called that",
      likeSaid: "Similar: ",
      of: (n, m) => `${n} of ${m}`,
      section: (name) => `Section ${name}`,
      end: "End.",
      start: "Start.",
      opened: (name) => `${name} opened`,
      bar: {
        chip3: [["+", "with"], ["−", "without"], ["Del", "off"]],
        chip2: [["+", "on"], ["Del", "off"]],
        radio: [["← →", "choose"]],
        person: [["← →", "with, without"], ["Del", "remove"]],
        slider: [["+", "more"], ["−", "less"], ["0–9", "number"]],
        find: [["↑ ↓", "matches"], ["Enter", "with"], ["Shift+Enter", "without"]],
      },
      names: { Mod: "Ctrl", Ctrl: "Ctrl", Del: "Del", Home: "Home", End: "End", PgUp: "Page ↑", PgDn: "Page ↓", Space: "Space" },
    },
  };
  const kb = () => (language().prefix === "/en" ? KB.en : KB.de);
  const onMac = () => root.classList.contains("mac");
  const MAC_SIGNS = { Mod: "⌘", Ctrl: "⌃", Alt: "⌥", Shift: "⇧", Enter: "↩", Del: "⌫", Home: "fn ←", End: "fn →", PgUp: "fn ↑", PgDn: "fn ↓" };
  const escHtml = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  // Keys as Betula writes them (`folia_design::keys`, the same rules): on a PC each key a cap with
  // „+" between them (Strg + K, Shift + Enter), on a Mac one cap of its signs (⌘K, ⇧↩).
  const capNames = (spec) => (spec === "+" || !spec.includes("+") ? [spec] : spec.split("+")).map((k) => (onMac() && MAC_SIGNS[k]) || kb().names[k] || k);
  const caps = (spec) => {
    const keys = capNames(spec);
    if (keys.length === 1) return `<kbd>${escHtml(keys[0])}</kbd>`;
    if (onMac()) return `<kbd>${escHtml(keys.join(""))}</kbd>`;
    return `<span class="kk">${keys.map((k) => `<kbd>${escHtml(k)}</kbd>`).join('<i class="plus">+</i>')}</span>`;
  };
  const lettersOn = () => root.dataset.letters !== "off";
  // A field one types into; a knob of the credits is none (its arrows go on, as everywhere in a mode).
  const texty = (el) => typing(el) && el.type !== "range";

  // What a screen reader is told that the screen shows otherwise (the end of the filters, what the
  // bubble found): one region for the page, polite.
  let liveEl = null;
  let liveTimer = 0;
  const say = (text) => {
    if (!liveEl) {
      liveEl = document.createElement("div");
      liveEl.className = "visually-hidden";
      liveEl.setAttribute("role", "status");
      document.body.append(liveEl);
    }
    clearTimeout(liveTimer);
    liveEl.textContent = "";
    liveTimer = setTimeout(() => { liveEl.textContent = text; }, 60);
  };

  // What the Tab key stops at.
  const TABBABLE = 'a[href], button, input:not([type="hidden"]), select, textarea, summary, [tabindex]';
  const shown = (el) => (el.checkVisibility ? el.checkVisibility({ visibilityProperty: true }) : el.getClientRects().length > 0);
  const tabbable = (el) => el.tabIndex >= 0 && !el.disabled && !el.closest("[inert]") && (el.tagName !== "SUMMARY" || el.parentElement?.tagName === "DETAILS") && shown(el);
  const tabbables = (scope) => [...scope.querySelectorAll(TABBABLE)].filter(tabbable);
  // The page's own parts: the stage in front, and what stands around the stages (the rail, the bar
  // at the top), not a page kept behind.
  const onPage = (el) => !el.closest("#content > .page-stage:not(.front)");
  const regionOf = (el) => el?.closest?.("[data-region]") || null;
  const regionName = (el) => {
    const region = regionOf(el);
    return region ? region.dataset.region : null;
  };
  const modeOf = (el) => el?.closest?.("[data-mode]") || null;
  // A control that stands for another where the arrows go (`data-kb-as`).
  const asEl = (el) => (el?.dataset?.kbAs && document.getElementById(el.dataset.kbAs)) || el;

  // ---- groups that are one stop of the Tab key ----
  const groupOf = (el) => el?.closest?.("[data-stop], [data-mode]") || null;
  const lastIn = new WeakMap(); // a group → where the keyboard was last in it
  const lastPlace = new WeakMap(); // a group → the place of that in a virtual list (`data-i`)
  // Seen: its middle is on the screen and not under something that stays in front of it (the
  // sticky heads of a list's columns).
  const inView = (el) => {
    const r = el.getBoundingClientRect();
    const x = r.left + Math.min(r.width / 2, 24);
    const y = r.top + r.height / 2;
    if (y < 0 || y > innerHeight || x < 0 || x > innerWidth) return false;
    const there = document.elementFromPoint(x, y);
    return !there || el === there || el.contains(there);
  };
  const itemsOf = (group) => tabbables(group).filter((el) => el.closest("[data-stop], [data-mode]") === group);
  // Where the keyboard was last in a group, also where the list built that row anew.
  const lastOf = (group) => {
    let last = lastIn.get(group);
    if (last && !last.isConnected && last.dataset?.id) last = group.querySelector(`[data-id="${CSS.escape(last.dataset.id)}"]`);
    return last && last.isConnected && group.contains(last) && tabbable(last) ? last : null;
  };
  const entryOf = (group) => {
    if (group.hasAttribute("data-mode")) return modeEntry(group);
    const last = lastOf(group);
    if (last) return last;
    const items = itemsOf(group);
    return items.find((el) => el.matches('[aria-current="true"], [aria-current="page"], [aria-selected="true"]') && inView(el))
      || items.find((el) => el.matches('[aria-current="true"], [aria-current="page"], [aria-selected="true"]'))
      || items.find(inView) || items[0] || null;
  };
  // The stops of Tab in `scope`, in order: each group once, at its entry; past the regions inside
  // `scope` where it asks (`skip`).
  const stopsIn = (scope, skip) => {
    const out = [];
    const seen = new Set();
    for (const el of tabbables(scope)) {
      if (!onPage(el)) continue;
      if (skip) {
        const region = regionOf(el);
        if (region && region !== scope && scope.contains(region)) continue;
      }
      const group = groupOf(el);
      if (group && group !== scope && scope.contains(group)) {
        if (seen.has(group)) continue;
        seen.add(group);
        const entry = entryOf(group);
        if (entry) out.push(entry);
      } else out.push(el);
    }
    return out;
  };
  // Where `el` stands among `stops`: itself, its group's entry, or between the two stops around it.
  const placeIn = (stops, el) => {
    const at = stops.indexOf(el);
    if (at >= 0) return at;
    const group = groupOf(el);
    const byGroup = group ? stops.findIndex((s) => groupOf(s) === group) : -1;
    if (byGroup >= 0) return byGroup;
    const after = stops.findIndex((s) => el.compareDocumentPosition(s) & Node.DOCUMENT_POSITION_FOLLOWING);
    return (after < 0 ? stops.length : after) - 0.5;
  };
  // Focus without the browser's own scroll; what scrolls brings it into view, a row of a list as
  // far as needed.
  const focusOn = (el, how = "nearest") => {
    if (!el) return;
    el.focus({ preventScroll: true });
    if (el.closest("[data-mode]")) inBand(el);
    // What is seen stays where it is: a scroll moves a virtual list's rows, and the focus with them.
    else if (!inView(el)) el.scrollIntoView?.({ block: how, inline: "nearest" });
  };

  // Into a group where the keyboard was last in it, also where that went off a virtual list's
  // rendered part (`data-reveal`: the list scrolls the row's place, `data-i`, into view on
  // `betula:reveal-row`, catalog.rs `VirtualRows`); else at `el`.
  const enter = (el) => {
    const group = el?.closest?.("[data-stop]");
    const last = group && lastIn.get(group);
    const place = last && !last.isConnected && group.hasAttribute("data-reveal") ? lastPlace.get(group) : null;
    if (place == null || !last.dataset.id) return focusOn(el);
    group.setAttribute("data-reveal-at", place);
    dispatchEvent(new Event("betula:reveal-row"));
    let tries = 0;
    const again = () => {
      const row = group.isConnected && group.querySelector(`[data-id="${CSS.escape(last.dataset.id)}"]`);
      if (row && tabbable(row)) return focusOn(row);
      if (++tries < 20) requestAnimationFrame(again);
      else focusOn(el);
    };
    requestAnimationFrame(again);
  };

  // ---- the keyboard's use: the ring, the lit section and the bar only after a key ----
  // Whether the keyboard was in use before this key (the mode showed as one): Esc leaves a mode only
  // then; after the mouse it is the page's as ever (it closes the module beside the list).
  let kbWas = false;
  addEventListener("keydown", (e) => {
    kbWas = root.hasAttribute("data-kb");
    if (!MODIFIERS.has(e.key) && !kbWas) root.setAttribute("data-kb", "");
  }, true);
  addEventListener("pointerdown", (e) => {
    if (e.pointerType !== "" && root.hasAttribute("data-kb")) root.removeAttribute("data-kb");
  }, true);

  // ---- a mode ----
  const modeName = (mode) => mode.getAttribute("aria-label") || "";
  // The parts the engine adds to a mode: the bar at its foot (the bubble comes with the first
  // letter typed).
  const modeParts = (mode) => {
    let bar = mode.querySelector(":scope > .key-bar");
    if (!bar) {
      bar = document.createElement("div");
      bar.className = "key-bar";
      bar.setAttribute("aria-hidden", "true");
      mode.append(bar);
    }
    return { bar };
  };
  // The tag on the ring, over the mode's top edge: on the page itself, since what scrolls the page
  // (the catalog's one scroll area) cuts off whatever of the panel stands above it; placed where
  // the mode is, there while the keyboard is in it (and only after a key: app.css, `data-kb`).
  let tagEl = null;
  const placeTag = () => {
    const mode = modeOf(document.activeElement);
    if (!tagEl) {
      tagEl = document.createElement("span");
      tagEl.className = "mode-tag";
      tagEl.setAttribute("aria-hidden", "true");
      document.body.append(tagEl);
    }
    if (!mode || !shown(mode)) return tagEl.removeAttribute("data-on");
    tagEl.innerHTML = kb().leave(caps("Esc"));
    const r = mode.getBoundingClientRect();
    tagEl.style.left = `${Math.round(r.left + 10)}px`;
    tagEl.style.top = `${Math.round(r.top - 9)}px`;
    tagEl.setAttribute("data-on", "");
  };
  addEventListener("resize", placeTag);
  const sectionOf = (el) => el?.closest?.("[data-section]") || null;
  // A section is entered at its start: its first control, an option group at its chosen option.
  const sectionEntry = (section) => {
    const first = tabbables(section).find((el) => sectionOf(el) === section);
    if (!first) return null;
    const group = first.closest('[role="radiogroup"]');
    return (group && [...group.querySelectorAll('[role="radio"][aria-checked="true"]')].find(tabbable)) || first;
  };
  const sectionsOf = (mode) => [...mode.querySelectorAll("[data-section]")].filter((s) => shown(s) && sectionEntry(s));
  // Where F lands: where the keyboard was last in the mode, never its head („Zurücksetzen"); else at
  // the start of its first section.
  const modeEntry = (mode) => {
    const last = lastOf(mode);
    if (last && sectionOf(last)?.dataset.section !== "plain") return last;
    const first = sectionsOf(mode).find((s) => s.dataset.section !== "plain");
    return first ? sectionEntry(first) : null;
  };
  // What scrolls in a mode, and the band of it the keyboard stays in: 20 % off either edge.
  const modeScroller = (el, mode) => {
    for (let at = el.parentElement; at && at !== mode; at = at.parentElement) {
      if (/(auto|scroll)/.test(getComputedStyle(at).overflowY) && at.scrollHeight > at.clientHeight) return at;
    }
    return null;
  };
  const calm = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
  const inBand = (el) => {
    const mode = modeOf(el);
    const box = mode && modeScroller(el, mode);
    if (!box) return el.scrollIntoView?.({ block: "nearest" });
    const b = box.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    if (r.top >= b.top + b.height * 0.2 && r.bottom <= b.bottom - b.height * 0.2) return;
    const top = Math.max(0, Math.min(box.scrollHeight - box.clientHeight, box.scrollTop + (r.top + r.height / 2) - (b.top + b.height / 2)));
    box.scrollTo({ top, behavior: calm() ? "auto" : "smooth" });
  };
  // „Weitere Filter" open as the keyboard goes past their head, or finds something in them.
  const openMore = (details) => {
    if (!details || details.open) return false;
    details.open = true;
    say(kb().opened(details.querySelector(":scope > summary")?.textContent.trim() || ""));
    return true;
  };
  // At either end the focus only nudges; a screen reader hears once in a while where it is and the
  // way out (owner, 2026-10-20: no note, „das checkt man auch selbst").
  let bumpTimer = 0;
  let bumpSaid = 0;
  const bump = (mode, where) => {
    mode.removeAttribute("data-bump");
    void mode.offsetWidth;
    mode.setAttribute("data-bump", where);
    clearTimeout(bumpTimer);
    bumpTimer = setTimeout(() => mode.removeAttribute("data-bump"), 400);
    const now = performance.now();
    if (now - bumpSaid > 3000) {
      bumpSaid = now;
      const way = document.getElementById(mode.getAttribute("aria-describedby") || "")?.textContent || "";
      say(`${where === "end" ? kb().end : kb().start} ${way}`.trim());
    }
  };
  // Tab: the start of the next section (Shift: of the one before); none past either end. Past the
  // head of „Weitere Filter" they open.
  const sectionTab = (e, mode, el) => {
    e.preventDefault();
    const back = e.shiftKey;
    const from = asEl(el);
    const here = sectionOf(from);
    if (!back && here?.matches("details") && from.tagName === "SUMMARY") openMore(here);
    const sections = sectionsOf(mode);
    const at = sections.indexOf(here);
    const next = sections[at + (back ? -1 : 1)];
    if (at < 0 || !next) return bump(mode, back ? "start" : "end");
    focusOn(sectionEntry(next));
  };
  // The rows of a mode, top to bottom, as they stand (chips that wrap, the two knobs of the
  // credits); an option group is a row of its own.
  const rowsOf = (mode) => {
    const rows = [];
    let row = null;
    let top = null;
    for (const el of tabbables(mode)) {
      // The bubble and a field that stands for a control (the number above a knob) are no row.
      if (el.closest(".find-bubble") || el.dataset.kbAs) continue;
      const t = el.getBoundingClientRect().top;
      if (row && Math.abs(t - top) <= 6) row.push(el);
      else {
        row = [el];
        rows.push(row);
        top = t;
      }
    }
    return rows;
  };
  // Where a control is across: a knob of the credits where its knob is, else its middle.
  const xOf = (el) => {
    const r = el.getBoundingClientRect();
    if (el.type === "range") {
      const min = +el.min || 0;
      const max = +el.max || 100;
      const at = (+el.value - min) / (max - min || 1);
      return r.left + r.height / 2 + (r.width - r.height) * at;
    }
    return r.left + r.width / 2;
  };
  const chosenIn = (row) => row.find((el) => el.getAttribute("aria-checked") === "true") || row[0];
  const radioRow = (row) => row.every((el) => el.getAttribute("role") === "radio" && el.closest('[role="radiogroup"]') === row[0].closest('[role="radiogroup"]'));
  let stickyX = null;
  let movingV = false;
  // ↑ ↓: the control in the row above or below nearest where the keyboard came from across.
  const moveV = (mode, el, dir) => {
    const from = asEl(el);
    if (dir > 0 && from.tagName === "SUMMARY" && openMore(from.parentElement)) {
      const first = sectionsOf(from.parentElement).find((s) => s !== from.parentElement);
      return focusOn(first ? sectionEntry(first) : null);
    }
    const rows = rowsOf(mode);
    const at = rows.findIndex((row) => row.includes(from));
    const row = rows[at + dir];
    if (at < 0) return;
    if (!row) return bump(mode, dir > 0 ? "end" : "start");
    const x = stickyX ?? xOf(from);
    const to = radioRow(row) ? chosenIn(row) : row.slice().sort((a, b) => Math.abs(xOf(a) - x) - Math.abs(xOf(b) - x))[0];
    movingV = true;
    focusOn(to);
    movingV = false;
    stickyX = x;
  };
  // ← →: the control before or after in reading order; an option group counts once, at its choice.
  const moveH = (mode, el, dir) => {
    const from = asEl(el);
    if (dir > 0 && from.tagName === "SUMMARY" && openMore(from.parentElement)) {
      const first = sectionsOf(from.parentElement).find((s) => s !== from.parentElement);
      return focusOn(first ? sectionEntry(first) : null);
    }
    const order = [];
    for (const row of rowsOf(mode)) {
      for (const one of row) {
        if (sectionOf(one)?.dataset.section === "plain") continue;
        const group = one.getAttribute("role") === "radio" && one.closest('[role="radiogroup"]');
        if (group && one !== from && (one.getAttribute("aria-checked") !== "true")) continue;
        if (group && order.some((o) => o.closest('[role="radiogroup"]') === group)) continue;
        order.push(one);
      }
    }
    const at = order.indexOf(from);
    if (at < 0) return;
    const to = order[at + dir];
    if (!to) return bump(mode, dir > 0 ? "end" : "start");
    focusOn(to);
  };
  // An option group: ← → choose the one before or after (owner, 2026-10-20: „links rechts option
  // auswählen und hoch und runter für prev/next element"), Pos1 and Ende the first and the last.
  const radioKey = (e, el) => {
    const group = el.closest('[role="radiogroup"]');
    if (!group) return false;
    const radios = [...group.querySelectorAll('[role="radio"]')].filter(tabbable);
    const at = radios.indexOf(el);
    const to = { ArrowRight: at + 1, ArrowLeft: at - 1, Home: 0, End: radios.length - 1 }[e.key];
    if (to === undefined) return false;
    e.preventDefault();
    const next = radios[Math.max(0, Math.min(radios.length - 1, to))];
    if (next && next !== el) {
      next.focus({ preventScroll: true });
      next.click();
    }
    return true;
  };

  // ---- finding in a mode: a green bubble, which typing opens (owner, 2026-10-20: „Es braucht keine
  // Suchbar … Hier könnte man wieder zu dem Suchblob zurück gehen, da könnte man über grün
  // nachdenken") ----
  // It is a text field like any other (⌫, Ctrl + ⌫), but no stop of Tab. What matches is marked as
  // the search marks a module (a highlight of the text, nothing in the page changed), the best match
  // is outlined and, once the typing rests, scrolled into the middle of the panel. It finds the
  // sections and „Weitere Filter" too, whose filters it opens. A typo is forgiven as the catalog's
  // search forgives it (`folia_search::fuzzy::typos_at_start`: from four letters one, from eight two,
  // the first letter right), but only where nothing matches as typed.
  const FIND = { mode: null, el: null, input: null, note: null, hits: [], at: -1, fuzzy: false, origin: null, timer: 0 };
  const fold = (s) => s.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/ß/g, "ss");
  // A text folded as the search folds it, and where each folded character came from.
  const folded = (s) => {
    let n = "";
    const at = [];
    for (let i = 0; i < s.length; i++) for (const c of fold(s[i])) { n += c; at.push(i); }
    return { n, at };
  };
  const editDistance = (a, b) => {
    const d = Array.from({ length: a.length + 1 }, (_, i) => Array.from({ length: b.length + 1 }, (_, j) => (i === 0 ? j : j === 0 ? i : 0)));
    for (let i = 1; i <= a.length; i++) for (let j = 1; j <= b.length; j++) {
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1);
    }
    return d[a.length][b.length];
  };
  const typosAtStart = (q, w) => {
    if (q.length < 4 || q[0] !== w[0]) return false;
    const allowed = q.length >= 8 ? 2 : 1;
    let best = Infinity;
    for (const len of [q.length - 1, q.length, q.length + 1]) if (len <= w.length) best = Math.min(best, editDistance(q, w.slice(0, len)));
    return best >= 1 && best <= allowed;
  };
  // The text of a label as it reads, without what is in its children (a count, a legend), and the
  // text nodes it is made of, to mark in.
  const labelText = (lab) => {
    const own = [...lab.childNodes].filter((n) => n.nodeType === 3 && n.textContent.trim());
    const nodes = own.length ? own : [...lab.childNodes].filter((n) => n.nodeType === 3);
    let text = "";
    const parts = [];
    for (const node of nodes) {
      parts.push({ node, from: text.length });
      text += node.textContent;
    }
    return { text, parts };
  };
  // What can be found in a mode: its sections by their names, its chips and options by theirs.
  const findables = (mode) => {
    const out = [];
    const reachable = (el) => shown(el) || !!el.closest("details:not([open])");
    for (const section of mode.querySelectorAll('[data-section]:not([data-section="plain"])')) {
      if (!reachable(section)) continue;
      const lab = section.tagName === "DETAILS" ? section.querySelector(":scope > summary") : document.getElementById(section.getAttribute("aria-labelledby") || "");
      const { text, parts } = lab ? labelText(lab) : { text: section.getAttribute("aria-label") || "", parts: [] };
      if (text.trim()) out.push({ kind: section.tagName === "DETAILS" ? "more" : "section", el: section, text, parts });
    }
    for (const el of mode.querySelectorAll('[role="checkbox"], [role="radio"]')) {
      if (!reachable(el)) continue;
      const lab = el.querySelector(".chip-label, .seg-label") || el.querySelector("span:not(.num):not(.box):not(.icon)");
      if (!lab) continue;
      const { text, parts } = labelText(lab);
      if (text.trim()) out.push({ kind: el.getAttribute("role") === "radio" ? "radio" : "chip", el, text, parts });
    }
    return out;
  };
  // How a target matches the words typed: each word has to match one of its words. 0 its first word
  // starts so, 1 another word does, 2 a part of a word, 3 the start of a word with a typo (asked for
  // only where nothing matches with 0 to 2). With the stretches of its text that matched.
  const matchOf = (target, words, typos) => {
    const { n, at } = folded(target.text);
    const its = [...n.matchAll(/[\p{L}\p{N}]+/gu)].map((m) => ({ w: m[0], i: m.index }));
    let tier = 0;
    const ranges = [];
    for (const q of words) {
      let best = null;
      its.forEach(({ w, i }, k) => {
        let hit = null;
        if (w.startsWith(q)) hit = { tier: k ? 1 : 0, a: i, b: i + q.length };
        else if (w.includes(q)) { const p = i + w.indexOf(q); hit = { tier: 2, a: p, b: p + q.length }; }
        else if (typos && typosAtStart(q, w)) hit = { tier: 3, a: i, b: i + w.length };
        if (hit && (!best || hit.tier < best.tier)) best = hit;
      });
      if (!best) return null;
      tier = Math.max(tier, best.tier);
      ranges.push([at[best.a], at[best.b - 1] + 1]);
    }
    return { tier, ranges };
  };
  const HIGHLIGHTS = typeof CSS !== "undefined" && CSS.highlights && typeof Highlight === "function";
  const markHits = () => {
    if (!HIGHLIGHTS) return;
    const hit = [];
    const like = [];
    for (const h of FIND.hits) {
      for (const [a, b] of h.ranges) {
        for (const { node, from } of h.parts) {
          const s = Math.max(a, from) - from;
          const e = Math.min(b, from + node.textContent.length) - from;
          if (e <= s) continue;
          const range = new Range();
          range.setStart(node, s);
          range.setEnd(node, e);
          (h.tier === 3 ? like : hit).push(range);
        }
      }
    }
    CSS.highlights.set("find-hit", new Highlight(...hit));
    CSS.highlights.set("find-like", new Highlight(...like));
  };
  const unmark = () => {
    if (HIGHLIGHTS) { CSS.highlights.delete("find-hit"); CSS.highlights.delete("find-like"); }
    for (const el of document.querySelectorAll("[data-find-target]")) el.removeAttribute("data-find-target");
  };
  // The control a match leads to: a chip or an option itself, a section's start.
  const targetOf = (h) => (!h ? null : h.kind === "section" ? sectionEntry(h.el) : h.kind === "more" ? h.el.querySelector(":scope > summary") : h.el);
  const showFind = ({ scroll = false } = {}) => {
    const h = FIND.hits[FIND.at];
    if (h && h.el.closest("details:not([open])")) openMore(h.el.closest("details:not([open])"));
    unmark();
    markHits();
    const target = h ? h.el : null;
    if (target) target.setAttribute("data-find-target", h.kind === "section" || h.kind === "more" ? "section" : "");
    lightSection(h ? (h.kind === "section" ? h.el : sectionOf(h.el)) : sectionOf(FIND.origin));
    const q = FIND.input.value.trim();
    FIND.note.textContent = !q ? "" : !FIND.hits.length ? kb().none : `${FIND.fuzzy ? `${kb().like} · ` : ""}${FIND.at + 1}/${FIND.hits.length}`;
    FIND.input.style.width = `${Math.max(1, FIND.input.value.length) + 0.6}ch`;
    if (scroll && target) {
      clearTimeout(FIND.timer);
      // The scroll waits until the typing rests, so that two targets do not toss the panel about.
      FIND.timer = setTimeout(() => inBand(target), 220);
    }
  };
  const sayFind = () => {
    if (!FIND.input.value.trim()) return;
    const h = FIND.hits[FIND.at];
    if (!h) return say(kb().noneSaid);
    const what = h.kind === "section" ? kb().section(h.text.trim()) : h.text.trim();
    const state = h.kind === "chip" ? h.el.getAttribute("aria-label") || "" : "";
    say(`${FIND.fuzzy ? kb().likeSaid : ""}${state && state !== what ? state : what}, ${kb().of(FIND.at + 1, FIND.hits.length)}`);
  };
  const runFind = () => {
    const words = [...fold(FIND.input.value).matchAll(/[\p{L}\p{N}]+/gu)].map((m) => m[0]);
    let hits = [];
    FIND.fuzzy = false;
    if (words.length) {
      const all = findables(FIND.mode);
      for (const t of all) { const m = matchOf(t, words, false); if (m) hits.push({ ...t, ...m }); }
      if (!hits.length) {
        for (const t of all) { const m = matchOf(t, words, true); if (m) hits.push({ ...t, ...m }); }
        FIND.fuzzy = hits.length > 0;
      }
    }
    FIND.hits = hits;
    FIND.at = hits.reduce((best, h, i) => (best < 0 || h.tier < hits[best].tier ? i : best), -1);
    showFind({ scroll: true });
    sayFind();
    updateBar();
  };
  const bubbleOf = (mode) => {
    let bubble = mode.querySelector(":scope > .find-bubble");
    if (!bubble) {
      bubble = document.createElement("div");
      bubble.className = "find-bubble";
      bubble.innerHTML = `<span aria-hidden="true"></span><input type="text" tabindex="-1" autocomplete="off" spellcheck="false"><span class="find-note" aria-hidden="true"></span>`;
      mode.append(bubble);
      const input = bubble.querySelector("input");
      input.addEventListener("input", runFind);
      input.addEventListener("blur", () => closeFind(false));
    }
    bubble.firstChild.textContent = kb().find;
    bubble.querySelector("input").setAttribute("aria-label", kb().findIn(modeName(mode)));
    return bubble;
  };
  const typeInto = (mode, ch) => {
    const bubble = bubbleOf(mode);
    if (!bubble.hasAttribute("data-on")) {
      FIND.origin = document.activeElement;
      bubble.setAttribute("data-on", "");
    }
    Object.assign(FIND, { mode, el: bubble, input: bubble.querySelector("input"), note: bubble.querySelector(".find-note") });
    FIND.input.focus({ preventScroll: true });
    FIND.input.value = ch;
    FIND.input.setSelectionRange(ch.length, ch.length);
    runFind();
  };
  // The bubble goes when the keyboard leaves it: back where the typing began (`back`), or wherever
  // the keyboard went.
  let closing = false;
  const closeFind = (back) => {
    if (!FIND.el || closing) return;
    closing = true;
    const { el, origin, mode } = FIND;
    clearTimeout(FIND.timer);
    el.removeAttribute("data-on");
    FIND.input.value = "";
    FIND.hits = [];
    FIND.at = -1;
    unmark();
    FIND.el = null;
    if (back) focusOn(origin && origin.isConnected && tabbable(origin) ? origin : modeEntry(mode));
    closing = false;
    updateBar();
  };
  // Enter sets what it found (a chip: with; Shift + Enter: without; again: off), chooses an option,
  // or goes into a section; the keyboard goes there. Tab only goes there.
  const actOnFind = (shift) => {
    const h = FIND.hits[FIND.at];
    const to = targetOf(h);
    // Nothing found: the bubble stays, with what was typed.
    if (!h || !to) return;
    closeFind(false);
    if (h.kind === "more") {
      openMore(h.el);
      const first = sectionsOf(h.el).find((s) => s !== h.el);
      return focusOn(first ? sectionEntry(first) : to);
    }
    focusOn(to);
    if (h.kind === "chip") to.dispatchEvent(new KeyboardEvent("keydown", { key: shift ? "-" : "+", bubbles: true, cancelable: true }));
    else if (h.kind === "radio" && to.getAttribute("aria-checked") !== "true") to.click();
  };
  const findKey = (e) => {
    const q = FIND.input.value.trim();
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const dir = e.key === "ArrowDown" ? 1 : -1;
      if (!q) {
        const { mode, origin } = FIND;
        closeFind(true);
        return origin && moveV(mode, origin, dir);
      }
      if (FIND.hits.length < 2) return;
      FIND.at = (FIND.at + dir + FIND.hits.length) % FIND.hits.length;
      showFind({ scroll: true });
      sayFind();
    } else if (e.key === "Enter") {
      e.preventDefault();
      actOnFind(e.shiftKey);
    } else if (e.key === "Tab") {
      const to = targetOf(FIND.hits[FIND.at]);
      if (to && !e.shiftKey) {
        e.preventDefault();
        closeFind(false);
        return focusOn(to);
      }
      const { mode } = FIND;
      closeFind(true);
      sectionTab(e, mode, document.activeElement);
    } else if (e.key === "Escape") {
      e.preventDefault();
      closeFind(true);
    }
  };

  // ---- the bar at the foot of a mode: only what no button can carry (owner, 2026-10-20: „Da muss
  // nur das ungewöhnliche rein") ----
  const barKind = (el) => {
    if (!el) return null;
    if (el === FIND.input && FIND.el) return "find";
    const control = asEl(el);
    if (control.getAttribute("role") === "checkbox" && control.classList.contains("chip")) return control.hasAttribute("data-tri") ? "chip3" : "chip2";
    // A chosen lecturer: its switch is the person's stop, Entf takes the person out (catalog.rs).
    if (control.getAttribute("role") === "radio" && control.closest(".person")) return "person";
    if (control.getAttribute("role") === "radio") return "radio";
    if (control.type === "range") return "slider";
    return null;
  };
  const updateBar = () => {
    const el = document.activeElement;
    const mode = modeOf(el);
    for (const bar of document.querySelectorAll(".key-bar[data-on]")) if (!mode || !mode.contains(bar)) bar.removeAttribute("data-on");
    if (!mode) return;
    const { bar } = modeParts(mode);
    const kind = barKind(el);
    const keys = kind && kb().bar[kind];
    bar.innerHTML = keys ? keys.map(([k, w]) => `<span>${k.split(" ").map(caps).join("")}${escHtml(w)}</span>`).join("") : "";
    bar.toggleAttribute("data-on", !!keys);
  };
  // The section with the keyboard, lit as the open row of a list (owner, 2026-10-20: „mit der bg
  // color und dem Seitenstreifen").
  const lightSection = (section) => {
    for (const lit of document.querySelectorAll("[data-kb-current]")) if (lit !== section) lit.removeAttribute("data-kb-current");
    if (section && section.dataset.section !== "plain") section.setAttribute("data-kb-current", "");
  };

  // Where the keyboard was last, and by what it is known again where the page builds it anew (a row
  // of a list, by its module): a key pressed with the focus fallen to the page goes on from there.
  let lastAt = null;
  // Only while the keyboard is in use: a click beside the rows lets the focus go, as it should.
  const recover = () => {
    const now = document.activeElement;
    if ((now && now !== document.body) || !lastAt || !lastAt.id || !root.hasAttribute("data-kb")) return;
    // The element itself where the page only moved it (a virtual list reordering its rows), else
    // its successor in its group, or where the page built the group anew.
    const scope = lastAt.group?.isConnected ? lastAt.group : stage() || document;
    const again = lastAt.el.isConnected && tabbable(lastAt.el) ? lastAt.el : [...scope.querySelectorAll(`[data-id="${CSS.escape(lastAt.id)}"]`)].find((el) => el.tagName === lastAt.el.tagName && tabbable(el));
    if (again) again.focus({ preventScroll: true });
  };
  // A focus that falls to the page as the page moves or builds its element anew is taken back a
  // moment later.
  addEventListener("focusout", (e) => {
    if (!e.relatedTarget && e.target?.dataset?.id) for (const ms of [30, 120, 400]) setTimeout(recover, ms);
  });
  addEventListener("focusin", (e) => {
    const el = e.target;
    lastAt = { el, group: el.closest?.("[data-stop], [data-mode]") || null, id: el.dataset?.id || null };
    if (!movingV) stickyX = null;
    const group = groupOf(el);
    if (group && el !== FIND.input) {
      lastIn.set(group, asEl(el));
      lastPlace.set(group, el.closest?.("[data-i]")?.dataset.i ?? null);
    }
    // A group inside a mode remembers where it was too (the mode itself is the group above).
    const mode = modeOf(el);
    placeTag();
    if (mode) {
      modeParts(mode);
      if (!FIND.el) lightSection(sectionOf(asEl(el)));
      if (el !== FIND.input && !el.closest(".lp-edit")) lastIn.set(mode, el);
    } else lightSection(null);
    updateBar();
  });
  addEventListener("focusout", (e) => {
    if (!modeOf(e.relatedTarget) && !FIND.el) lightSection(null);
    if (!e.relatedTarget) setTimeout(() => { updateBar(); placeTag(); }, 0);
  });

  // ---- going between regions ----
  const regionsNow = () => ["nav", "search", "side", "content"].map((name) => ({ name, el: findAll(`[data-region="${name}"]`).find((r) => onPage(r) && shown(r)) || (name === "content" ? stage() : null) })).filter((r) => r.el);
  // Into the content: its list where the keyboard was last in it, else its first stop. Where a page
  // names no content region (`strict`), none.
  const contentHome = (strict) => {
    const content = find('[data-region="content"]') || (strict ? null : stage());
    if (!content) return null;
    const list = [...content.querySelectorAll('[data-stop="y"]')].find((g) => !g.closest("[data-mode]") && shown(g) && itemsOf(g).length);
    // An empty list: the first way on in the content, not a panel's handle.
    return (list && entryOf(list)) || stopsIn(content, true).find((s) => s.getAttribute("role") !== "separator") || null;
  };
  const sideRegion = () => findAll('[data-region="side"]').find(onPage) || null;
  const intoSide = () => {
    const side = sideRegion();
    if (!side) return false;
    if (phone() ? !side.classList.contains("open") : !shown(side)) {
      // A phone's or a narrow window's sheet: open it first.
      const open = findAll('[data-action="sheet-open"]').find(shown);
      if (!open) return false;
      open.click();
      requestAnimationFrame(() => requestAnimationFrame(() => focusOn(modeEntry(side))));
      return true;
    }
    focusOn(modeEntry(side));
    return true;
  };
  const intoSearch = () => {
    const search = document.getElementById("topsearch");
    if (!search) return false;
    search.focus();
    search.select();
    return true;
  };
  // What „Befehle" asks of this script (folia/crates/shell/src/commands.rs, `betula:act`, its detail
  // „<action> <value>"): what the switches and the keys do here — light or dark, the single keys,
  // into the filters (F) or the search (/).
  document.addEventListener("betula:act", (e) => {
    const [action, value] = String(e.detail || "").split(" ");
    if (action === "theme-set") setTheme(value);
    else if (action === "letters-set") setLetters(value === "off");
    else if (action === "go" && value === "side") intoSide();
    else if (action === "go" && value === "search") intoSearch();
  });
  const intoRegion = (region) => {
    if (region.name === "side") return intoSide();
    if (region.name === "search") return intoSearch();
    if (region.name === "content") return enter(contentHome(false)), true;
    const entry = region.el.matches("[data-stop]") ? entryOf(region.el) : stopsIn(region.el, false)[0];
    return focusOn(entry), true;
  };
  const cycleRegions = (dir) => {
    const regions = regionsNow();
    const here = regionName(document.activeElement);
    const at = regions.findIndex((r) => r.name === here);
    const next = regions[((at < 0 ? (dir > 0 ? -1 : 0) : at) + dir + regions.length) % regions.length];
    if (next) intoRegion(next);
  };

  // ---- Tab ----
  const tabKey = (e) => {
    const el = document.activeElement;
    if (!el || el === document.body || e.ctrlKey || e.metaKey || e.altKey) return;
    const mode = modeOf(el);
    if (mode) return sectionTab(e, mode, el);
    const cycle = el.closest("[data-cycle]");
    if (cycle && onPage(cycle)) {
      const stops = stopsIn(cycle, true);
      if (!stops.length) return;
      e.preventDefault();
      const at = placeIn(stops, el);
      const n = stops.length;
      const next = e.shiftKey ? stops[(Math.ceil(at) - 1 + n) % n] : stops[(Math.floor(at) + 1) % n];
      return enter(next);
    }
    // Elsewhere as the browser goes, but past a group at once and into one where it was last.
    const stops = stopsIn(document.body, false);
    const at = placeIn(stops, el);
    const next = e.shiftKey ? stops[Math.ceil(at) - 1] : stops[Math.floor(at) + 1];
    const group = groupOf(el);
    if (!next || (!group && !groupOf(next))) return;
    e.preventDefault();
    enter(next);
  };

  // ---- the arrows in a group that is one stop ----
  const groupKey = (e, el) => {
    const group = el.closest("[data-stop]");
    if (!group || modeOf(group)) return false;
    const axis = group.dataset.stop;
    const y = axis.includes("y") && (e.key === "ArrowDown" || e.key === "ArrowUp");
    // ← and → go to the note of a module planned while it stands (folia/crates/widgets/src/plannote.rs).
    const x = axis.includes("x") && (e.key === "ArrowRight" || e.key === "ArrowLeft") && !document.querySelector(".planning-note");
    if (!x && !y) return false;
    const items = itemsOf(group);
    const at = items.indexOf(el);
    const to = items[at + (e.key === "ArrowDown" || e.key === "ArrowRight" ? 1 : -1)];
    e.preventDefault();
    if (to) focusOn(to);
    return true;
  };

  // ---- keys ----
  // What lies over the page and has the keyboard to itself: a dialog, a menu, a question. A key
  // pressed there is its own (a menu's entry shows the key it is), never one for the page behind.
  const OVER = "dialog, [role=dialog], [role=menu], [role=alertdialog]";
  addEventListener("keydown", (e) => {
    if (e.isComposing) return;
    recover();
    const el = document.activeElement || document.body;
    // An open picker has the keyboard to itself (its Esc closes the picker, nothing else).
    if (e.target.closest?.(".combo[data-open]")) return;
    if (FIND.el && e.target === FIND.input) return findKey(e);
    // The filter toggles are links that act as switches: the space bar flips them, too.
    if (e.key === " " && !e.defaultPrevented && e.target.matches?.('a[role="checkbox"], a[role="radio"]')) { e.preventDefault(); e.target.click(); return; }
    const plain = !e.ctrlKey && !e.metaKey && !e.altKey;
    const over = !!e.target.closest?.(OVER) || modalOpen();
    // Ctrl + F6: the next region (Shift: the one before). F6 alone stays the browser's.
    if (e.key === "F6" && (e.ctrlKey || e.metaKey) && !e.altKey && !over) { e.preventDefault(); return cycleRegions(e.shiftKey ? -1 : 1); }
    if (e.defaultPrevented || over) return;
    if (e.key === "Tab") return tabKey(e);
    const mode = modeOf(el);
    const arrow = /^Arrow/.test(e.key) && plain && !e.shiftKey;
    // In a mode: the arrows from control to control (in a field of numbers ↑ and ↓ as well, whose
    // ← → move in the text), Pos1 and Ende in a section, Esc out, letters find.
    const vertical = e.key === "ArrowUp" || e.key === "ArrowDown";
    if (mode && (!texty(el) || (arrow && vertical && (el.type === "number" || el.dataset?.kbAs)))) {
      if (arrow) {
        if (el.getAttribute("role") === "radio" && (e.key === "ArrowLeft" || e.key === "ArrowRight") && radioKey(e, el)) return;
        e.preventDefault();
        if (e.key === "ArrowDown" || e.key === "ArrowUp") return moveV(mode, el, e.key === "ArrowDown" ? 1 : -1);
        return moveH(mode, el, e.key === "ArrowRight" ? 1 : -1);
      }
      if ((e.key === "Home" || e.key === "End") && plain) {
        if (el.getAttribute("role") === "radio" && radioKey(e, el)) return;
        const section = sectionOf(el);
        const all = section ? tabbables(section).filter((c) => sectionOf(c) === section) : [];
        if (all.length) { e.preventDefault(); return focusOn(e.key === "Home" ? all[0] : all[all.length - 1]); }
      }
      if (plain && e.key === "Escape" && kbWas) { e.preventDefault(); return enter(contentHome(false)); }
      // Typing finds, from wherever the keyboard is in the mode; only there, so it needs no switch.
      if (plain && e.key.length === 1 && /[\p{L}\p{N}]/u.test(e.key)) { e.preventDefault(); return typeInto(mode, e.key); }
    }
    if (mode && plain && e.key === "Escape" && texty(el) && kbWas) {
      // A field in the mode (the numbers of the credits): Esc leaves the field and the mode.
      e.preventDefault();
      return enter(contentHome(false));
    }
    if (arrow && !mode && !texty(el) && groupKey(e, el)) return;
    // With the keyboard nowhere (a click beside the rows), ↑ and ↓ go into the list.
    if (vertical && arrow && (el === document.body || el === document.documentElement)) {
      const home = contentHome(true);
      if (home && home.closest('[data-stop="y"]')) { e.preventDefault(); return enter(home); }
    }
    // In the search: ↓ into the list (owner's concept: „↓ in die Liste").
    if (e.key === "ArrowDown" && plain && el.id === "topsearch") {
      const home = contentHome(true);
      if (home && home.closest('[data-stop="y"]')) { e.preventDefault(); return enter(home); }
    }
    const hit = e.key === "ArrowLeft" || e.key === "ArrowRight" ? resizerOf(e.target) : null;
    if (hit) {
      e.preventDefault();
      const step = (e.key === "ArrowRight" ? 24 : -24) * hit.edge.grows;
      const panel = hit.edge.panel();
      if (panel) setWidth(hit.edge, panel.getBoundingClientRect().width + step);
      return;
    }
    // ← and → while a Termin stands in its own view (folia/crates/widgets/src/termin.rs): the date
    // before and after it, as the arrows beside its date (owner, 2026-10-27: „bau als Nächstes die
    // Tasten ← → fürs Datum"). Not in a field, nor where the arrows are another's (a mode, a group
    // of one stop, a panel's edge: above; the note of a module planned). One date a press: a key
    // held does not run through the series.
    if (arrow && !vertical && !mode && !texty(el) && !document.querySelector(".planning-note")) {
      const to = find(`.is-termin .tk-date a[data-step="${e.key === "ArrowLeft" ? "prev" : "next"}"]`);
      if (to) {
        e.preventDefault();
        if (!e.repeat) to.click();
        return;
      }
    }
    if (plain && e.key === "Escape") {
      // One Esc, one thing: nothing it closes here goes on as a close request to a watcher.
      if (filters()?.classList.contains("open")) { e.preventDefault(); closeSheet(); return; }
      // The search and the navigation: back into the content (the list where it was).
      const region = regionName(el);
      if ((region === "search" || region === "nav") && !root.hasAttribute("data-layers")) {
        const home = contentHome(true);
        if (home) { e.preventDefault(); return enter(home); }
      }
      if (typing(el)) { el.blur(); return; }
      // What lies over the page (a menu, the selection, a dialog: folia/crates/shell/src/nav.rs)
      // takes this Esc itself; the module beside the page waits for the next one.
      if (root.hasAttribute("data-layers") || root.hasAttribute("data-state")) return;
      // The preview first; on a module's own page Esc goes back to where the visitor came from.
      // The keyboard goes back to the row of the module that was open.
      const close = find('[data-action="close-detail"]');
      if (close && el.closest?.("#preview")) {
        const row = find('.rows a.row[aria-current="true"]');
        if (row) row.focus({ preventScroll: true });
      }
      // Then the choice in focus over the plan's week („Fertig"), then a page's „Zurück".
      (close || find('[data-action="close-focus"]') || find('[data-action="back"]'))?.click();
      return;
    }
    // Enter on the row whose module is open: the module in full (Enter opened it beside the list).
    if (e.key === "Enter" && plain && !e.shiftKey && el.matches?.('a.row[aria-current="true"]')) {
      const full = find('[data-action="fullscreen"]');
      if (full?.getClientRects().length) { e.preventDefault(); full.click(); return; }
    }
    // Ctrl + K: „Befehle" (folia/crates/shell/src/commands.rs), whose button stands in the search
    // field; before the app runs, the search itself.
    if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      const commands = appRuns() && find('[data-action="commands"]');
      if (commands) commands.click();
      else intoSearch();
      return;
    }
    // Betula's single keys: only where nothing is typed, and only while they are on.
    if (!plain || texty(el) || !lettersOn()) return;
    const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    if (e.key === "/") { e.preventDefault(); intoSearch(); return; }
    if (e.key === "?") {
      const button = find('[data-action="keys"]');
      if (!button) return;
      e.preventDefault();
      const region = regionName(el);
      const from = region === "side" ? "side" : el.closest?.(".is-termin") ? "termin" : el.closest?.("#preview") ? "module" : region === "content" ? "list" : region || "";
      button.setAttribute("data-from", from);
      button.click();
      return;
    }
    if (key === "f" && !e.shiftKey) {
      if (intoSide()) e.preventDefault();
      return;
    }
    if (key === "v" && !e.shiftKey) {
      // „Vollbild" where its button is shown: a phone's sheet has none (app.css).
      const full = find('[data-action="fullscreen"]');
      if (full?.getClientRects().length) { e.preventDefault(); full.click(); }
      return;
    }
    if (key === "m" && appRuns()) {
      // „Merken" for what the visitor is at: the row the keyboard is on, else the module that is
      // open (its preview, or its page). The buttons belong to the browser app. With Shift, its
      // arrow (folia/design/tasten/konzept.html, „Knopf mit Pfeil"): „Sammlung und Notiz"
      // (folia/crates/widgets/src/collect.rs). Shift, not the letter's case: with Caps Lock M marks.
      if (e.shiftKey) {
        // While the note after marking stands, its field is the arrow (marknote.rs has the key);
        // with rows selected (the Merkliste's bar, folia/crates/bookmarks/src/pick.rs) the bar's
        // „Sammlung", for all of them.
        if (find('.wait-note [data-action="collect"]')) return;
        const bar = find('.pick-bar [data-action="collect"]');
        const row = el.closest?.(".row-wrap");
        const more = bar || (row ? row.querySelector('[data-action="collect"]') : find('.hero [data-action="collect"]'));
        if (more) { e.preventDefault(); more.click(); }
        return;
      }
      const row = el.closest?.(".row-wrap");
      const mark = row ? row.querySelector('[data-action="mark"]') : find('.hero [data-action="mark"]');
      if (mark) { e.preventDefault(); mark.click(); }
      return;
    }
    if (key === "p" && appRuns()) {
      // „Einplanen" for the module that is open (its preview, or its page); rows have no plan
      // button of their own. The keyboard on another row of the list than the open one would make
      // it ambiguous which module is meant, so P waits there. What it planned the note at the
      // bottom says, whose ← and → move it (folia/crates/widgets/src/plannote.rs). With Shift, its
      // arrow (folia/design/tasten/konzept.html): planned, the menu of what is planned
      // (planmenu.rs), else „Einplanen in …" (planinto.rs); P as such where there is no arrow.
      const row = el.closest?.(".row-wrap");
      const plan = row && !row.querySelector('[aria-current="true"]') ? null : find('.hero [data-action="plan"]');
      const more = e.shiftKey && plan?.closest(".plan-pair")?.querySelector('[data-action="plan-more"]');
      // The arrow has the focus, as after a click on it: the menu gives it back there.
      if (more) { e.preventDefault(); more.focus({ preventScroll: true }); more.click(); }
      else if (plan) { e.preventDefault(); plan.click(); }
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
