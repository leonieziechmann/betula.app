// Small behaviours shared by the server-rendered pages and the browser app. Everything works
// without this file (links and forms). Before the app has taken over (`window.__betulaApp`), it
// makes the classic site smoother: filters apply on change, panels keep their scroll position
// across page loads. In both modes: the shortcuts (Esc closes the preview or leaves the module
// page, F opens the previewed module full screen, Ctrl+K or "/" jumps to the search; in the app
// M marks the module the visitor is at and P plans it into the Studienplan), the theme
// switch, the filter sheet, and the widths of the filter panel and the module preview (dragged,
// kept in localStorage).
(() => {
  const root = document.documentElement;
  const appRuns = () => window.__betulaApp === true;
  const phone = () => matchMedia("(max-width: 900px)").matches;
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
    // can answer first (app/src/pending.rs): that is the same step again, not another one.
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

  function submit(form) {
    saveScroll();
    for (const el of form.elements) if (el.name && el.value === "" && el.type !== "checkbox") el.disabled = true; // keep the URL clean
    if (form.requestSubmit) form.requestSubmit(); else form.submit();
  }
  let timer;
  document.addEventListener("change", (e) => {
    const form = e.target.closest("form[data-autosubmit]");
    if (!form || appRuns()) return; // the app has its own handlers
    if (phone()) return; // the sheet has its own apply button
    clearTimeout(timer);
    timer = setTimeout(() => submit(form), e.target.type === "number" || e.target.type === "text" ? 350 : 0);
  }, true);

  // Classic mode: the credit slider writes into the two number fields, which the form submits.
  document.addEventListener("input", (e) => {
    const slider = e.target.type === "range" && !appRuns() ? e.target.closest(".slider") : null;
    if (!slider) return;
    const [low, high] = slider.querySelectorAll("input");
    if (e.target === low && +low.value > +high.value) low.value = high.value;
    if (e.target === high && +high.value < +low.value) high.value = low.value;
    slider.style.setProperty("--from", low.value / low.max);
    slider.style.setProperty("--to", high.value / high.max);
    const fields = slider.closest("form")?.elements;
    if (fields?.ects_min) fields.ects_min.value = +low.value > 0 ? low.value : "";
    if (fields?.ects_max) fields.ects_max.value = +high.value < +high.max ? high.value : "";
  });

  // The app filters while typing; Enter must not load a page on top of that.
  document.addEventListener("submit", (e) => {
    if (appRuns() && e.target.matches("form[data-live-search]")) e.preventDefault();
  });

  // Classic mode on a phone: a module is its own page, never a preview (the app does this by
  // itself, and also knows which row to show when the visitor comes back).
  document.addEventListener("click", (e) => {
    const row = e.target.closest?.("a.row[data-id]");
    if (!row || appRuns() || !phone() || e.defaultPrevented) return;
    e.preventDefault();
    location.href = "/catalog/module/" + encodeURIComponent(row.dataset.id);
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
        section.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "start" });
        break;
      }
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
          label.textContent = link ? "Link kopiert" : "Kopiert";
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
      place: (handle, width) => { handle.style.left = width === null ? "" : width + "px"; },
    },
    "resize-preview": {
      key: "betula.preview.width", prop: "--preview-w", grows: -1,
      min: () => 360, max: () => { const work = document.querySelector(".work"); return work ? Math.max(360, work.clientWidth - sideWidth() - 128) : 2400; },
      panel: () => document.querySelector(".work > .detail"),
      place: (handle, width) => { handle.style.right = width === null ? "" : width - 12 + "px"; },
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
})();
