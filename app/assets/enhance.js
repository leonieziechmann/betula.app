// Small behaviours shared by the server-rendered pages and the browser app. Everything works
// without this file (links and forms). Before the app has taken over (`window.__btuApp`), it
// makes the classic site smoother: filters apply on change, panels keep their scroll position
// across page loads. In both modes: the shortcuts (Esc closes the preview or leaves the module
// page, F opens the previewed module full screen, Ctrl+K or "/" jumps to the search), the theme
// switch, the filter sheet, and the width of the module preview (dragged, kept in localStorage).
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
    if (!form) return;
    if (appRuns()) {
      // The app re-renders the filter panel: keep its scroll position and the focused control.
      const body = form.querySelector("[data-keep-scroll]");
      const top = body ? body.scrollTop : 0;
      const { name, value, type } = e.target;
      requestAnimationFrame(() => requestAnimationFrame(() => {
        const next = document.querySelector('#filters [data-keep-scroll]');
        if (next) next.scrollTop = top;
        if (!name) return;
        const selector = type === "checkbox" ? `#filters [name="${name}"][value="${value}"]` : `#filters [name="${name}"]`;
        document.querySelector(selector)?.focus({ preventScroll: true });
      }));
      return;
    }
    if (phone()) return; // the sheet has its own apply button
    clearTimeout(timer);
    timer = setTimeout(() => submit(form), e.target.type === "number" || e.target.type === "text" ? 350 : 0);
  }, true);

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

  // ---- width of the module preview: drag the left edge, arrow keys, double click resets ----
  const WIDTH_KEY = "btu.preview.width";
  const setWidth = (px, remember) => {
    const work = document.querySelector(".work");
    const max = work ? Math.max(360, work.clientWidth - 400) : 2400;
    const width = Math.round(Math.min(max, Math.max(360, px)));
    root.style.setProperty("--preview-w", width + "px");
    if (remember) { try { localStorage.setItem(WIDTH_KEY, String(width)); } catch {} }
  };
  document.addEventListener("pointerdown", (e) => {
    const handle = e.target.closest('[data-action="resize-preview"]');
    if (!handle || e.button !== 0) return;
    e.preventDefault();
    const panel = handle.parentElement.getBoundingClientRect();
    const right = panel.right + (e.clientX - panel.left); // keep the grabbed point under the pointer
    root.classList.add("resizing");
    const move = (ev) => setWidth(right - ev.clientX, false);
    const stop = (ev) => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", stop);
      removeEventListener("pointercancel", stop);
      root.classList.remove("resizing");
      setWidth(right - ev.clientX, true);
    };
    addEventListener("pointermove", move);
    addEventListener("pointerup", stop);
    addEventListener("pointercancel", stop);
  });
  document.addEventListener("dblclick", (e) => {
    if (!e.target.closest('[data-action="resize-preview"]')) return;
    root.style.removeProperty("--preview-w");
    try { localStorage.removeItem(WIDTH_KEY); } catch {}
  });

  // ---- shortcuts (each is written next to its button) ----
  addEventListener("keydown", (e) => {
    const resizer = e.target.closest ? e.target.closest('[data-action="resize-preview"]') : null;
    if (resizer && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      setWidth(resizer.parentElement.getBoundingClientRect().width + (e.key === "ArrowLeft" ? 32 : -32), true);
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
