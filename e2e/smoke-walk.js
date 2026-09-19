// Browser smoke walk (docs/frontend-rewrite.md §7, phase 0).
//
// This file runs INSIDE the page and needs no tooling: a runner injects it and awaits
// `window.__smokeWalk(options)`. `run.mjs` is the Playwright runner for CI; any other
// driver works the same way. The walk
//   1. opens the program overview and collects every program link,
//   2. visits every program through client-side navigation, with every tab,
//      the first area chips and every plan variant,
//   3. jumps straight from program to program without waiting for the page to settle,
//      mixed with Back/Forward (the path that froze the old app),
// and reports every console error, uncaught error and unhandled rejection it saw.
// The runner fails the build when `report.errors` is not empty or `report.ok` is false.
//
// Pages take part through `data-walk` attributes, so the walk does not depend on the
// design or the URL scheme:
//   data-walk="program-link"   links to a program page (overview)
//   data-walk="program-page"   root of a rendered program page; data-walk-id = slug
//   data-walk="tab"            tab links of the program page
//   data-walk="area-chip"      area filter chips
//   data-walk="plan-variant"   plan variant switches
(function () {
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  const DEFAULTS = {
    overviewUrl: "/studiengaenge",
    selectors: {
      programLink: '[data-walk="program-link"]',
      programPage: '[data-walk="program-page"]',
      tab: '[data-walk="tab"]',
      areaChip: '[data-walk="area-chip"]',
      planVariant: '[data-walk="plan-variant"]',
    },
    maxPrograms: Infinity, // lower it for a quick local run
    chipsPerProgram: 3,
    settleTimeoutMs: 4000,
    // Warnings that count as errors (hydration mismatches are reported as warnings).
    failOnWarning: /hydrat|mismatch|disposed|panicked/i,
  };

  function installErrorTrap(options) {
    if (window.__smokeWalkErrors) return window.__smokeWalkErrors;
    const errors = [];
    const push = (kind, text) => errors.push({ kind, url: location.pathname + location.search, text: String(text).slice(0, 600) });
    window.addEventListener("error", (e) => push("error", e.message || e.error));
    window.addEventListener("unhandledrejection", (e) => push("unhandledrejection", (e.reason && (e.reason.message || e.reason)) || e));
    const consoleError = console.error.bind(console);
    console.error = (...args) => { push("console.error", args.map(String).join(" ")); consoleError(...args); };
    const consoleWarn = console.warn.bind(console);
    console.warn = (...args) => {
      const text = args.map(String).join(" ");
      if (options.failOnWarning.test(text)) push("console.warn", text);
      consoleWarn(...args);
    };
    window.__smokeWalkErrors = errors;
    return errors;
  }

  // Client-side navigation exactly like a user click: the router intercepts anchor clicks.
  function follow(href) {
    const a = document.createElement("a");
    a.href = href;
    a.style.display = "none";
    document.body.appendChild(a);
    a.click();
    a.remove();
  }

  async function waitFor(predicate, timeoutMs) {
    const deadline = performance.now() + timeoutMs;
    while (performance.now() < deadline) {
      const value = predicate();
      if (value) return value;
      await sleep(15);
    }
    return null;
  }

  window.__smokeWalk = async function (overrides) {
    const options = { ...DEFAULTS, ...(overrides || {}), selectors: { ...DEFAULTS.selectors, ...((overrides || {}).selectors || {}) } };
    const sel = options.selectors;
    const errors = installErrorTrap(options);
    const documentMarker = (window.__smokeWalkDocument = window.__smokeWalkDocument || Math.random());
    const report = { ok: false, programs: 0, visited: 0, tabs: 0, chips: 0, variants: 0, jumps: 0, notRendered: [], fullPageLoads: 0, seconds: 0, errors };
    const started = performance.now();

    if (location.pathname !== options.overviewUrl) follow(options.overviewUrl);
    const links = await waitFor(() => {
      const found = [...document.querySelectorAll(sel.programLink)];
      return found.length ? found : null;
    }, options.settleTimeoutMs);
    if (!links) {
      errors.push({ kind: "walk", url: location.pathname, text: "no program links on " + options.overviewUrl });
      return report;
    }
    const programs = [...new Set(links.map((a) => a.getAttribute("href")))].slice(0, options.maxPrograms);
    report.programs = programs.length;

    const rendered = (href) => {
      const page = document.querySelector(sel.programPage);
      return page && location.pathname.startsWith(href) ? page : null;
    };

    // Pass 1: every program, every tab, chips and plan variants, waiting for each page.
    for (const href of programs) {
      follow(href);
      if (!(await waitFor(() => rendered(href), options.settleTimeoutMs))) {
        report.notRendered.push(href);
        continue;
      }
      report.visited++;
      const tabs = [...new Set([...document.querySelectorAll(sel.tab)].map((a) => a.getAttribute("href")))];
      for (const tab of tabs) {
        follow(tab);
        await waitFor(() => location.pathname === tab, 1000);
        report.tabs++;
        for (const chip of [...document.querySelectorAll(sel.areaChip)].slice(0, options.chipsPerProgram)) {
          chip.click();
          report.chips++;
        }
        for (const variant of document.querySelectorAll(sel.planVariant)) {
          variant.click();
          report.variants++;
        }
      }
    }

    // Pass 2: fast jumps from program to program without settling, with Back/Forward.
    for (let i = 0; i < programs.length; i++) {
      follow(programs[i]);
      report.jumps++;
      if (i % 3 === 0) {
        await waitFor(() => rendered(programs[i]), 600);
        const chip = document.querySelector(sel.areaChip);
        if (chip) chip.click();
      } else if (i % 3 === 1) {
        await sleep(0);
      }
      if (i % 10 === 9) {
        history.back();
        history.back();
        await sleep(0);
        history.forward();
      }
    }
    await sleep(500);

    // A full page load would have reset this marker: navigation must stay client-side.
    if (window.__smokeWalkDocument !== documentMarker) report.fullPageLoads++;
    report.seconds = Math.round((performance.now() - started) / 100) / 10;
    report.ok = errors.length === 0 && report.notRendered.length === 0 && report.visited === report.programs && report.fullPageLoads === 0;
    return report;
  };
})();
