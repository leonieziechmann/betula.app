// The page in front (folia/crates/shell/src/stage.rs): every tab's page stays built on a stage of its
// own in `main`, and what a check looks for is on the one in front or around the pages, never on a
// page kept behind, which has parts of the same name. Loaded into a check's pages with
// `context.addInitScript({ path })`: `onPage(selector)` and `allOnPage(selector)` are
// `document.querySelector` and `querySelectorAll` (as an array) without the stages behind. A step's
// selector for Playwright takes what is seen instead (`>> visible=true`).
(() => {
  const behind = (el) => el.closest("#content > .page-stage:not(.front)");
  window.onPage = (selector) => [...document.querySelectorAll(selector)].find((el) => !behind(el)) ?? null;
  window.allOnPage = (selector) => [...document.querySelectorAll(selector)].filter((el) => !behind(el));
})();
