// Progressive enhancement for the server-rendered pages. Everything here also works without
// this file (links and a form); with it the site feels like an app until the real one takes over:
// filters apply on change, panels keep their scroll position, Esc closes what feels like a
// popup, Ctrl+K or "/" jumps to the search, and the theme switch remembers the choice.
(() => {
  const root = document.documentElement;
  const phone = () => matchMedia("(max-width: 900px)").matches;
  const typing = (el) => el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);
  const filters = () => document.getElementById("filters");

  // Scroll positions of the panels survive a page load; the list only for the same filter.
  const listKey = () => location.search.replace(/([?&])page=\d+/, "$1");
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
  addEventListener("pagehide", saveScroll);

  function submit(form) {
    saveScroll();
    // Empty fields would only clutter the URL.
    for (const el of form.elements) if (el.name && el.value === "" && el.type !== "checkbox") el.disabled = true;
    if (form.requestSubmit) form.requestSubmit(); else form.submit();
  }
  let timer;
  document.addEventListener("change", (e) => {
    const form = e.target.closest("form[data-autosubmit]");
    if (!form || phone()) return; // on a phone the sheet has its own "apply" button
    clearTimeout(timer);
    timer = setTimeout(() => submit(form), e.target.type === "number" || e.target.type === "text" ? 350 : 0);
  });

  document.addEventListener("click", (e) => {
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
    }
  });

  addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      if (filters()?.classList.contains("open")) { filters().classList.remove("open"); return; }
      if (typing(document.activeElement)) { document.activeElement.blur(); return; }
      document.querySelector('[data-action="close-detail"]')?.click();
    } else if (((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") || (e.key === "/" && !typing(document.activeElement))) {
      e.preventDefault();
      document.getElementById("topsearch")?.focus();
    }
  });
})();
