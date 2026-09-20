// Checks the programs area: the overview by faculty, its sidebar, and the frame every page shares.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node programs.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Walks: the sidebar is in the same place on every page → overview (faculties in order, every program
// once, nothing cut off) → filters (links, the sidebar is not rebuilt, the search keeps them) → jump to a
// faculty without a history entry → program page (views in the sidebar) → phone (filters in a sheet) →
// the overview without JavaScript.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };

const open = async (options, path) => {
  const context = await browser.newContext(options);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__btuApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await page.evaluate(() => { window.__marker = 1; });
  const step = async (name, action, until, arg) => {
    const started = Date.now();
    await action();
    try {
      await page.waitForFunction(until, arg, { timeout: 8000 });
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    timings[name] = Date.now() - started;
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
  };
  return { page, step, context };
};
const box = (page, selector) => page.evaluate((s) => { const r = document.querySelector(s)?.getBoundingClientRect(); return r ? [Math.round(r.left), Math.round(r.top), Math.round(r.width)] : null; }, selector);

// ---------- desktop ----------
{
  const { page, step, context } = await open({ viewport: { width: 1500, height: 900 } }, "/catalog");
  // The frame: on every page a sidebar exactly where the catalog has its filter panel.
  const frame = await box(page, "#filters");
  await step("programs", () => page.click('.rail a[href="/programs"]'), () => location.pathname === "/programs" && document.querySelectorAll(".program-pill").length > 100);
  check(JSON.stringify(await box(page, "#sidebar")) === JSON.stringify(frame), `programs: the sidebar is at ${await box(page, "#sidebar")}, the filter panel was at ${frame}`);

  const overview = await page.evaluate(() => {
    const sections = [...document.querySelectorAll("section.faculty")];
    const pills = [...document.querySelectorAll(".program-pill")];
    return {
      codes: sections.map((s) => s.querySelector(".faculty-code").textContent),
      counted: sections.map((s) => Number(s.querySelector(".tab-count").textContent.replace(/\D/g, ""))).reduce((a, b) => a + b, 0),
      pills: pills.length,
      distinct: new Set(pills.map((p) => p.getAttribute("href"))).size,
      total: Number(document.querySelector(".summary .count").textContent.replace(/\D/g, "")),
      // A segment says only what differs from its program („dual, Praxis"); its full name is its label.
      alike: sections.flatMap((s) => [...s.querySelectorAll(".subject")]).filter((row) => { const names = [...row.querySelectorAll(".program-pill")].map((p) => p.getAttribute("aria-label")); return new Set(names).size !== names.length; }).map((row) => row.querySelector(".subject-name").textContent),
      // The matrix: every column starts on one vertical line, in all sections.
      lines: ["bachelor", "master", "other"].map((stage) => new Set([...document.querySelectorAll(`.cell[data-stage="${stage}"]`)].map((cell) => Math.round(cell.getBoundingClientRect().left))).size),
      overflowing: [...document.querySelectorAll(".cell")].filter((cell) => cell.scrollWidth > cell.clientWidth + 1).length,
      cut: [...document.querySelectorAll("#sidebar .toc a, #sidebar .hint, #sidebar .chip")].filter((el) => el.getBoundingClientRect().right > document.getElementById("sidebar").getBoundingClientRect().right + 0.5).map((el) => el.textContent.slice(0, 30)),
    };
  });
  check(overview.codes.slice(0, 6).join() === "Fakultät 1,Fakultät 2,Fakultät 3,Fakultät 4,Fakultät 5,Fakultät 6" && overview.codes.at(-1) === "Ohne Zuordnung", `overview: sections are ${overview.codes.join(" | ")}`);
  check(overview.pills === overview.distinct && overview.pills === overview.total && overview.counted === overview.total, `overview: ${overview.pills} links, ${overview.distinct} programs, ${overview.counted} counted in the sections, ${overview.total} in the header`);
  check(overview.alike.length === 0, `overview: programs of a subject that read the same: ${overview.alike.join(", ")}`);
  check(overview.cut.length === 0, `overview: cut off at the edge of the sidebar: ${overview.cut.join(" | ")}`);
  check(overview.lines.join() === "1,1,1" && overview.overflowing === 0, `overview: the columns do not line up (${overview.lines}) or a cell overflows (${overview.overflowing})`);

  // Filters are links; the sidebar stays the same element and the toggle keeps the focus.
  await page.evaluate(() => { document.getElementById("sidebar").__same = true; });
  await step("filter: Master", () => page.click('#sidebar a.chip:has-text("Master")'), () => location.search === "?level=master" && document.querySelectorAll(".program-pill").length < 100);
  const masters = await page.evaluate(() => ({ pills: document.querySelectorAll(".program-pill").length, chip: Number(document.querySelector('#sidebar a.chip[data-state="with"] .chip-count').textContent), header: Number(document.querySelector(".summary .count").textContent), bachelors: [...document.querySelectorAll(".program-pill b")].filter((b) => /^B\.|Bachelor/.test(b.textContent)).length }));
  check(masters.pills === masters.chip && masters.pills === masters.header && masters.bachelors === 0, `filter: ${JSON.stringify(masters)}`);
  await step("filter: and dual", () => page.click('#sidebar a.chip:has-text("Dual")'), () => location.search === "?level=master&form=dual" && document.querySelectorAll(".program-pill").length > 0);
  check(await page.evaluate(() => [...document.querySelectorAll(".program-pill")].every((p) => p.textContent.includes("dual"))), "filter: a program that is not dual is listed");
  check(await page.evaluate(() => document.getElementById("sidebar").__same === true && document.activeElement?.textContent.includes("Dual")), "filter: the sidebar was rebuilt or the toggle lost the focus");
  // The search of the top bar narrows further and keeps the filters.
  await step("search keeps the filters", () => page.fill("#topsearch", "maschinen"), () => location.search.includes("q=maschinen") && location.search.includes("level=master") && location.search.includes("form=dual") && document.querySelectorAll(".subject").length === 1);
  await step("reset", () => page.click('#sidebar a:has-text("Zurücksetzen")'), () => location.search === "" && document.querySelectorAll(".program-pill").length > 100);

  // Jump to a faculty: in view, no history entry.
  const entries = await page.evaluate(() => history.length);
  await page.click('#sidebar .toc a:has-text("Fakultät 5")');
  await page.waitForTimeout(800);
  check(await page.evaluate(() => { const top = document.querySelector("section.faculty:nth-of-type(5)").getBoundingClientRect().top; return top >= 0 && top < innerHeight / 2 && location.hash === ""; }) && (await page.evaluate(() => history.length)) === entries, "jump: the faculty is not in view, or the jump left a history entry");

  // A program: the same frame, its views in the sidebar.
  await step("program page", () => page.click('.program-pill[href^="/programs/bachelor-informatik"]'), () => location.pathname.startsWith("/programs/bachelor-informatik") && document.querySelector('[data-walk="program-page"]'));
  check(JSON.stringify(await box(page, "#sidebar")) === JSON.stringify(frame), "program page: the sidebar moved");
  await step("program page: another view", () => page.click('#sidebar .toc a:has-text("Alle Module")'), () => location.pathname.endsWith("/modules") && document.querySelector("table.modules") && document.querySelector('#sidebar .toc a[aria-current="page"]')?.textContent === "Alle Module");
  await step("home", () => page.click('.rail a[href="/"]'), () => location.pathname === "/" && document.querySelector(".features"));
  check(JSON.stringify(await box(page, "#sidebar")) === JSON.stringify(frame), "home: the sidebar is not in the frame's place");
  await context.close();
}

// ---------- phone: the filters are a sheet ----------
{
  const { page, step, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, "/programs");
  check(await page.evaluate(() => document.querySelector("section.faculty").getBoundingClientRect().top < innerHeight * 0.4), "phone: the first faculty does not start on the first screen");
  await step("phone: the sheet opens", () => page.tap(".sheet-toggle"), () => document.querySelector(".sidebar.sheet.open") && document.querySelector(".sidebar.sheet").getBoundingClientRect().top < innerHeight - 200);
  await step("phone: a filter in the sheet", () => page.tap('#sidebar a.chip:has-text("Bachelor")'), () => location.search === "?level=bachelor" && document.querySelector(".sidebar.sheet.open"));
  await step("phone: the sheet closes", () => page.tap('#sidebar .sheet-only a'), () => !document.querySelector(".sidebar.sheet.open"));
  check(await page.evaluate(() => [...document.querySelectorAll(".faculty-head")].every((head) => head.scrollWidth <= head.clientWidth + 1)), "phone: a faculty's heading does not fit");
  // On a phone the matrix is one column: the programs of a subject stand under its name.
  const stacked = await page.evaluate(() => [...document.querySelectorAll(".subject")].filter((row) => { const name = row.querySelector(".subject-name").getBoundingClientRect(); return ![...row.querySelectorAll(".cell:not(.empty)")].every((cell) => cell.getBoundingClientRect().top >= name.bottom - 1 && cell.getBoundingClientRect().right <= innerWidth); }).length);
  check(stacked === 0, `phone: in ${stacked} rows the programs overlap the name of the subject or leave the screen`);
  await context.close();
}

// ---------- without JavaScript ----------
{
  const context = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 1300, height: 900 } });
  const page = await context.newPage();
  await page.goto(base + "/programs?level=master", { waitUntil: "domcontentloaded" });
  check((await page.getAttribute('#sidebar a.chip:has-text("Master")', "data-state")) === "with", "no JS: the filter of the URL is not shown as set");
  const before = await page.locator(".program-pill").count();
  await page.click('#sidebar a.chip:has-text("Doppelabschluss")');
  await page.waitForURL(/form=double/);
  check(page.url().includes("level=master") && (await page.locator(".program-pill").count()) < before, "no JS: a filter link lost the other filter, or did not filter");
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
