// Small behaviours shared by the server-rendered pages and the browser app. Everything works
// without this file (links and forms). Before the app has taken over (`window.__btuApp`), it
// makes the classic site smoother: filters apply on change, panels keep their scroll position
// across page loads. In both modes: the shortcuts (Esc closes the preview or leaves the module
// page, F opens the previewed module full screen, Ctrl+K or "/" jumps to the search), the theme
// switch, the filter sheet, and the widths of the filter panel and the module preview (dragged,
// kept in localStorage).
(() => {
  const root = document.documentElement;
  const appRuns = () => window.__btuApp === true;
  const phone = () => matchMedia("(max-width: 900px)").matches;
  // Text-like controls only: a focused filter chip (checkbox) must not swallow Esc or "/".
  const typing = (el) => el && ((el.tagName === "INPUT" && !["checkbox", "radio", "button", "submit"].includes(el.type)) || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);
  const filters = () => document.getElementById("filters");
  let inAppSteps = 0;

  // ---- classic mode: scroll positions of the panels survive a page load ----
  const listKey = () => location.search.replace(/([?&])(page|open)=[^&]*/g, "$1");
  function saveScroll() {
    try {
      document.querySelectorAll("[data-keep-scroll]").forEach((el) => {
        const name = el.dataset.keepScroll;
        sessionStorage.setItem("btu.scroll." + name, JSON.stringify({ top: el.scrollTop, key: name === "rows" ? listKey() : "" }));
      });
    } catch {}
  }
  function restoreScroll() {
    try {
      document.querySelectorAll("[data-keep-scroll]").forEach((el) => {
        const name = el.dataset.keepScroll;
        if (name === "detail") return;
        const saved = JSON.parse(sessionStorage.getItem("btu.scroll." + name) || "null");
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

  document.addEventListener("click", (e) => {
    if (e.target.closest("a[href]")) inAppSteps++;
    const target = e.target.closest("[data-action]");
    if (!target) return;
    switch (target.dataset.action) {
      case "theme": {
        const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
        root.dataset.theme = dark ? "light" : "dark";
        try { localStorage.setItem("btu.theme", root.dataset.theme); } catch {}
        break;
      }
      case "sheet-open":
        e.preventDefault();
        filters()?.classList.add("open");
        break;
      case "sheet-close":
        e.preventDefault();
        filters()?.classList.remove("open");
        break;
      case "back": {
        // Back to where the visitor came from (list, filters and scroll position included),
        // if that was this site; otherwise follow the link.
        const cameFromHere = appRuns() ? inAppSteps > 1 : document.referrer.startsWith(location.origin);
        if (cameFromHere && history.length > 1) { e.preventDefault(); history.back(); }
        break;
      }
    }
  });

  // ---- widths of the filter panel and the module preview: drag the edge, arrow keys on the
  // focused edge, a double click resets. Personal, so kept in localStorage and not in the URL.
  const RESIZE = {
    "resize-filters": { key: "btu.filters.width", prop: "--w-filters", grows: 1, min: () => 232, max: () => 440, panel: () => filters() },
    "resize-preview": {
      key: "btu.preview.width", prop: "--preview-w", grows: -1, min: () => 360, panel: () => document.querySelector(".work > .detail"),
      max: () => { const work = document.querySelector(".work"); return work ? Math.max(360, work.clientWidth - 400) : 2400; },
    },
  };
  const resizerOf = (target) => {
    const handle = target.closest ? target.closest("[data-action^='resize-']") : null;
    return handle && RESIZE[handle.dataset.action] ? { handle, edge: RESIZE[handle.dataset.action] } : null;
  };
  const setWidth = (edge, px, remember) => {
    const width = Math.round(Math.min(edge.max(), Math.max(edge.min(), px)));
    root.style.setProperty(edge.prop, width + "px");
    if (remember) { try { localStorage.setItem(edge.key, String(width)); } catch {} }
  };
  document.addEventListener("pointerdown", (e) => {
    const hit = e.button === 0 ? resizerOf(e.target) : null;
    if (!hit) return;
    e.preventDefault();
    const { edge } = hit;
    const panel = edge.panel();
    if (!panel) return;
    const startWidth = panel.getBoundingClientRect().width;
    const startX = e.clientX; // the grabbed point stays under the pointer
    root.classList.add("resizing");
    const widthAt = (ev) => startWidth + edge.grows * (ev.clientX - startX);
    const move = (ev) => setWidth(edge, widthAt(ev), false);
    const stop = (ev) => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", stop);
      removeEventListener("pointercancel", stop);
      root.classList.remove("resizing");
      setWidth(edge, widthAt(ev), true);
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
      if (panel) setWidth(hit.edge, panel.getBoundingClientRect().width + step, true);
      return;
    }
    const plain = !e.ctrlKey && !e.metaKey && !e.altKey;
    if (plain && e.key === "Escape") {
      if (filters()?.classList.contains("open")) { filters().classList.remove("open"); return; }
      if (typing(document.activeElement)) { document.activeElement.blur(); return; }
      // The preview first; on a module's own page Esc goes back to where the visitor came from.
      (document.querySelector('[data-action="close-detail"]') || document.querySelector('[data-action="back"]'))?.click();
    } else if (plain && (e.key === "f" || e.key === "F") && !typing(document.activeElement)) {
      const full = document.querySelector('[data-action="fullscreen"]');
      if (full) { e.preventDefault(); full.click(); }
    } else if (((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") || (plain && e.key === "/" && !typing(document.activeElement))) {
      e.preventDefault();
      const search = document.getElementById("topsearch");
      search?.focus();
      search?.select();
    }
  });
})();
