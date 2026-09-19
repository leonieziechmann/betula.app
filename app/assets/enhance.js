// Small behaviours shared by the server-rendered pages and the browser app. Everything works
// without this file (links and forms). Before the app has taken over (`window.__btuApp`), it
// makes the classic site smoother: filters apply on change, panels keep their scroll position
// across page loads. In both modes: Esc closes what feels like a popup, Ctrl+K or "/" jumps to
// the search, the theme switch remembers the choice, the filter sheet opens and closes.
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

  addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      if (filters()?.classList.contains("open")) { filters().classList.remove("open"); return; }
      if (typing(document.activeElement)) { document.activeElement.blur(); return; }
      document.querySelector('[data-action="close-detail"]')?.click();
    } else if (((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") || (e.key === "/" && !typing(document.activeElement))) {
      e.preventDefault();
      const search = document.getElementById("topsearch");
      search?.focus();
      search?.select();
    }
  });
})();
