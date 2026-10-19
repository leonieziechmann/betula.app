// Checks the programs area: the overview by faculty, its sidebar, and the frame every page shares.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node programs.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Walks: the sidebar is in the same place on every page → overview (faculties in order as accordions,
// closed until opened, every program once, nothing cut off) → filters (links, the sidebar is not
// rebuilt, the search keeps them, what is left is open) → jump to a faculty without a history entry,
// which opens it → program page (views in the sidebar, „Zurück" and Esc lead back to the program in
// the overview, its faculty open) → the rail's items are tabs that remember where their area was left, and
// „Studium" leads to „Mein Studium" first → the program page itself (head, one study plan per study
// direction, matrix or list, a module beside it and the way back out of it, areas, all modules in the
// catalog, and „Mein Plan" going on to „Mein Studium") → the plan at
// every window width (no column runs into another, no row taller than its name, the list where the
// matrix has no room) → phone (the plan's rows as tall as their names, filters in a sheet, a subject
// a row that folds open) → the overview without JavaScript.
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
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await page.evaluate(() => { window.__marker = 1; });
  const step = async (name, action, until, arg) => {
    const started = Date.now();
    await action();
    try {
      await page.waitForFunction(until, arg, { timeout: 8000 });
      // The page before stays as a picture in front of the new one while its answers come (pending.rs, `hold`).
      await page.waitForFunction(() => !document.querySelector(".pending-page.held"), null, { timeout: 2000 });
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    timings[name] = Date.now() - started;
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
  };
  return { page, step, context };
};
const box = (page, selector) => page.evaluate((s) => { const r = document.querySelector(s)?.getBoundingClientRect(); return r ? [Math.round(r.left), Math.round(r.top), Math.round(r.width)] : null; }, selector);
// The rows of the plan that are taller than their names. A row is as tall as its name: a column
// that gave way („Art", „Bereich", 0 px wide) keeps its cells, and they take no line of their own.
// In 0 px a cell that wraps stands a letter to a line — „Grundstudium" made every row of Informatik
// B.Sc. 252 px high on a phone (2026-10-14). A row is 20–21 px taller than its name's text (padding
// and rule; all 4,563 rows of the plans on a phone and at 1200 px), one line more is 19.6 px.
const tallRows = (page) => page.evaluate(() => [...document.querySelectorAll("table.planlist tr:not(.group), table.matrix tbody tr:not(.group)")].flatMap((row) => {
  const name = row.querySelector(".c-name");
  const range = document.createRange();
  range.selectNodeContents(name);
  const height = row.getBoundingClientRect().height;
  const text = range.getBoundingClientRect().height;
  return height > text + 30 ? [`„${name.textContent.trim().slice(0, 24)}" is ${Math.round(height)} px high, its name ${Math.round(text)} px`] : [];
}));
// A program of the overview on a wide page: a line of the matrix in its faculty's accordion.
const LINE = ".fx-desk a.prog-line";

// ---------- desktop ----------
{
  const { page, step, context } = await open({ viewport: { width: 1500, height: 900 } }, "/catalog");
  // The frame: on every page a sidebar exactly where the catalog has its filter panel.
  const frame = await box(page, "#filters");
  // „Studium" is „Mein Studium" first (owner, 2026-10-04); the overview is one link away.
  await step("Studium", () => page.click('.rail a[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-setup"));
  await step("programs", () => page.click('#sidebar a[href="/programs"]'), (line) => location.pathname === "/programs" && document.querySelectorAll(line).length > 100, LINE);
  check(JSON.stringify(await box(page, "#sidebar")) === JSON.stringify(frame), `programs: the sidebar is at ${await box(page, "#sidebar")}, the filter panel was at ${frame}`);
  // The faculties wait to be opened (owner, 2026-10-14: the accordions); opened, their matrices.
  check(await page.evaluate(() => document.querySelectorAll("details.facc").length > 5 && document.querySelectorAll("details.facc[open]").length === 0), "overview: a faculty is open without being opened");
  await page.evaluate(() => document.querySelectorAll("details.facc").forEach((faculty) => { faculty.open = true; }));

  const overview = await page.evaluate((line) => {
    const sections = [...document.querySelectorAll("details.facc")];
    const pills = [...document.querySelectorAll(line)];
    return {
      codes: sections.map((s) => s.querySelector(".facc-tag").textContent),
      counted: sections.map((s) => Number(s.querySelector(".facc-n").textContent.replace(/\D/g, ""))).reduce((a, b) => a + b, 0),
      pills: pills.length,
      distinct: new Set(pills.map((p) => p.getAttribute("href"))).size,
      // On a phone the same programs, each once.
      phone: new Set([...document.querySelectorAll(".fx-phone a[data-id]")].map((p) => p.getAttribute("href"))).size,
      total: Number(document.querySelector(".summary .count").textContent.replace(/\D/g, "")),
      // A form of study says only what differs from its program („dual, Praxis"); its full name is its label.
      alike: [...document.querySelectorAll(".fx-desk tbody tr")].filter((row) => { const names = [...row.querySelectorAll("a.prog-line")].map((p) => p.getAttribute("aria-label")); return new Set(names).size !== names.length; }).map((row) => row.querySelector(".fx-name").textContent),
      // The matrix: every column starts on one vertical line, in all faculties.
      lines: ["bachelor", "master"].map((stage) => new Set([...document.querySelectorAll(`.fx-desk td[data-stage="${stage}"]`)].map((cell) => Math.round(cell.getBoundingClientRect().left))).size),
      overflowing: [...document.querySelectorAll(".fx-desk td")].filter((cell) => cell.scrollWidth > cell.clientWidth + 1).length,
      cut: [...document.querySelectorAll("#sidebar .toc a, #sidebar .hint, #sidebar .chip")].filter((el) => el.getBoundingClientRect().right > document.getElementById("sidebar").getBoundingClientRect().right + 0.5).map((el) => el.textContent.slice(0, 30)),
    };
  }, LINE);
  await page.evaluate(() => document.querySelectorAll("details.facc").forEach((faculty) => { faculty.open = false; }));
  check(overview.codes.slice(0, 6).join() === "Fakultät 1,Fakultät 2,Fakultät 3,Fakultät 4,Fakultät 5,Fakultät 6" && overview.codes.at(-1) === "Ohne Zuordnung", `overview: sections are ${overview.codes.join(" | ")}`);
  check(overview.pills === overview.distinct && overview.pills === overview.total && overview.counted === overview.total && overview.phone === overview.total, `overview: ${overview.pills} links, ${overview.distinct} programs, ${overview.phone} on a phone, ${overview.counted} counted in the faculties, ${overview.total} in the header`);
  check(overview.alike.length === 0, `overview: programs of a subject that read the same: ${overview.alike.join(", ")}`);
  check(overview.cut.length === 0, `overview: cut off at the edge of the sidebar: ${overview.cut.join(" | ")}`);
  check(overview.lines.join() === "1,1" && overview.overflowing === 0, `overview: the columns do not line up (${overview.lines}) or a cell overflows (${overview.overflowing})`);

  // Filters are links; the sidebar stays the same element and the toggle keeps the focus.
  await page.evaluate(() => { document.getElementById("sidebar").__same = true; });
  await step("filter: Master", () => page.click('#sidebar a.chip:has-text("Master")'), (line) => location.search === "?level=master" && document.querySelectorAll(line).length < 100, LINE);
  const masters = await page.evaluate((line) => ({ pills: document.querySelectorAll(line).length, chip: Number(document.querySelector('#sidebar a.chip[data-state="with"] .chip-count').textContent), header: Number(document.querySelector(".summary .count").textContent), bachelors: [...document.querySelectorAll(`${line} b`)].filter((b) => /^B\.|Bachelor/.test(b.textContent)).length, closed: document.querySelectorAll("details.facc:not([open])").length }), LINE);
  check(masters.pills === masters.chip && masters.pills === masters.header && masters.bachelors === 0 && masters.closed === 0, `filter: ${JSON.stringify(masters)}`);
  await step("filter: and dual", () => page.click('#sidebar a.chip:has-text("Dual")'), (line) => location.search === "?level=master&form=dual" && document.querySelectorAll(line).length > 0, LINE);
  check(await page.evaluate((line) => [...document.querySelectorAll(line)].every((p) => p.getAttribute("aria-label").includes("dual")), LINE), "filter: a program that is not dual is listed");
  check(await page.evaluate(() => document.getElementById("sidebar").__same === true && document.activeElement?.textContent.includes("Dual")), "filter: the sidebar was rebuilt or the toggle lost the focus");
  // The search of the top bar narrows further and keeps the filters.
  await step("search keeps the filters", () => page.fill("#topsearch", "maschinen"), () => location.search.includes("q=maschinen") && location.search.includes("level=master") && location.search.includes("form=dual") && document.querySelectorAll(".fx-desk tbody tr").length === 1);
  await step("reset", () => page.click('#sidebar a:has-text("Zurücksetzen")'), (line) => location.search === "" && document.querySelectorAll(line).length > 100 && document.querySelectorAll("details.facc[open]").length === 0, LINE);

  // Jump to a faculty: open and in view, no history entry.
  const entries = await page.evaluate(() => history.length);
  await page.click('#sidebar .toc a:has-text("Fakultät 5")');
  await page.waitForTimeout(800);
  check(await page.evaluate(() => { const faculty = document.querySelector("details.facc:nth-of-type(5)"); const top = faculty.getBoundingClientRect().top; return faculty.open && top >= 0 && top < innerHeight / 2 && location.hash === ""; }) && (await page.evaluate(() => history.length)) === entries, "jump: the faculty is not open and in view, or the jump left a history entry");

  // A program: the same frame, its views in the sidebar.
  await step("open Fakultät 1", () => page.click("details.facc:nth-of-type(1) > summary"), () => document.querySelector("details.facc:nth-of-type(1)").open);
  await step("program page", () => page.click(`${LINE}[href^="/programs/bachelor-informatik"]`), () => location.pathname.startsWith("/programs/bachelor-informatik") && document.querySelector('[data-walk="program-page"]'));
  check(JSON.stringify(await box(page, "#sidebar")) === JSON.stringify(frame), "program page: the sidebar moved");
  await step("program page: another view", () => page.click('#sidebar .toc.views a:has-text("Wahlpflicht & Bereiche")'), () => location.pathname.endsWith("/areas") && document.querySelector("table.areas") && document.querySelector('#sidebar .toc a[aria-current="page"]')?.textContent === "Wahlpflicht & Bereiche");

  // „Zurück" (and Esc) lead to the overview, and the overview shows the program again.
  const inView = () => page.evaluate((line) => { const pill = document.querySelector(`${line}[data-id^="bachelor-informatik"]`)?.getBoundingClientRect(); return Boolean(pill) && pill.height > 0 && pill.top >= 0 && pill.bottom <= innerHeight; }, LINE);
  check(await page.evaluate(() => [...document.querySelectorAll('[data-action="back"] kbd')].some((k) => k.textContent === "Esc")), "program page: no back link with its shortcut");
  await step("program page: Zurück", () => page.click('[data-action="back"]'), (line) => location.pathname === "/programs" && document.querySelectorAll(line).length > 100, LINE);
  await page.waitForTimeout(400);
  check(await inView(), "back on the overview the program is not in view (its faculty open)");
  await step("program page again", () => page.click(`${LINE}[data-id^="bachelor-informatik"]`), () => location.pathname.startsWith("/programs/bachelor-informatik"));
  await step("program page: Esc", () => page.keyboard.press("Escape"), () => location.pathname === "/programs");

  // The rail's items are tabs: each leads to where its area was left.
  const tab = (area) => page.getAttribute(`.rail a[data-area="${area}"]`, "href");
  await step("a filter in the overview", () => page.click('#sidebar a.chip:has-text("Master")'), () => location.search === "?level=master");
  await step("a program", () => page.click(`${LINE}[data-id^="master-informatik"]`), () => location.pathname.startsWith("/programs/master-informatik"));
  const programPage = await page.evaluate(() => location.pathname);
  check((await tab("programs")) === "/study", `tabs: on a program's page the own tab leads to ${await tab("programs")}, not up to „Mein Studium"`);
  await step("tab: Module", () => page.click('.rail a[data-area="catalog"]'), () => location.pathname === "/catalog" && document.querySelector(".rows a.row"));
  await step("a filter in the catalog", () => page.click('#filters a.chip:has-text("Winter")'), () => location.search.includes("turnus=winter"));
  check((await tab("programs")) === programPage, `tabs: the tab of the programs leads to ${await tab("programs")}, the area was left at ${programPage}`);
  await step("tab: Studium returns to the program", () => page.click('.rail a[data-area="programs"]'), (path) => location.pathname === path && document.querySelector('[data-walk="program-page"]'), programPage);
  check((await tab("catalog")) === "/catalog?turnus=winter", `tabs: the tab of the catalog leads to ${await tab("catalog")}`);
  // Coming from another area, „Zurück" leads up to the overview (not back to that other area).
  await step("Zurück after a change of tabs", () => page.click('[data-action="back"]'), () => location.pathname === "/programs" && location.search === "?level=master");
  await step("tab: Module returns to the filtered list", () => page.click('.rail a[data-area="catalog"]'), () => location.pathname === "/catalog" && location.search.includes("turnus=winter"));
  check((await tab("catalog")) === "/catalog", "tabs: on the list the own tab is not the plain list");

  await step("home", () => page.click('.rail a[href="/"]'), () => location.pathname === "/" && document.querySelector(".home .home-hero"));
  check((await box(page, "#content > .page-stage.front #sidebar")) === null, "home: a sidebar beside the start page (owner, 2026-09-28: none)");
  await context.close();
}

// ---------- a program: its head, its study plans, one table in every view ----------
{
  const { page, step, context } = await open({ viewport: { width: 1500, height: 900 } }, "/programs/bachelor-elektrotechnik-2022");
  // The head: the numbers stand on the title's line and stay inside the panel.
  const head = await page.evaluate(() => {
    const panel = document.querySelector(".prog-head").getBoundingClientRect();
    const facts = [...document.querySelectorAll(".prog-facts .pfact")];
    return {
      facts: facts.map((f) => f.textContent.replace(/\s+/g, " ").trim()),
      out: facts.filter((f) => f.getBoundingClientRect().right > panel.right - 20).length,
      crumbs: [...document.querySelectorAll(".crumbs a")].map((a) => a.getAttribute("href")),
      meta: document.querySelector(".prog-meta").innerText.replace(/\s+/g, " ").trim(),
    };
  });
  check(head.facts.join(" | ") === "6 Semester | 180 LP | 85 Module | 131 FÜS-Module", `head: the numbers read ${head.facts.join(" | ")}`);
  check(head.facts.every((fact) => / /.test(fact)), "head: a number and its label are written without a space between them");
  check(head.out === 0, "head: a number stands outside its panel");
  check(head.crumbs.join() === "/programs,/programs?level=bachelor", `head: the path leads to ${head.crumbs.join(" ")}`);
  check(/B\.Sc\..*Prüfungsordnung 2022.*aktuell/.test(head.meta), `head: the line under the title reads ${head.meta}`);

  // The study plan: one plan per study direction, never both in one table (each is 180 LP, and
  // the first semester of one direction is 30 LP, not the 60 of both).
  const plan = await page.evaluate(() => {
    const names = [...document.querySelectorAll("table.matrix tbody tr .c-name")].map((cell) => cell.textContent.trim());
    const widths = new Set([...document.querySelectorAll("table.matrix thead tr:last-child th")].map((th) => Math.round(th.getBoundingClientRect().width)));
    const scroll = document.querySelector(".plan-block .table-scroll");
    return {
      chips: [...document.querySelectorAll('[data-walk="plan-variant"]')].map((chip) => chip.getAttribute("data-state")),
      rows: names.length,
      credits: [...document.querySelectorAll("table.matrix tbody td.lp")].reduce((sum, td) => sum + Number(td.textContent.replace(",", ".").replace("*", "").trim() || 0), 0),
      sums: [...document.querySelectorAll("table.matrix tfoot td.lp")].map((td) => Number(td.textContent.trim() || 0)),
      widths: [...widths],
      cut: scroll.scrollWidth > scroll.clientWidth + 1,
    };
  });
  check(plan.chips.join() === "with,off", `plan: the study directions are ${plan.chips.join()}`);
  // Both study directions are 180 LP each: 360 would mean they are drawn as one plan again.
  check(plan.credits === 180, `plan: the plan adds up to ${plan.credits} LP, not to the 180 of one study direction`);
  check(plan.sums[0] === 30, `plan: the first semester adds up to ${plan.sums[0]} LP, not to 30`);
  check(plan.widths.length === 1 && !plan.cut, `plan: the semester columns are ${plan.widths.join("/")} px wide, cut off: ${plan.cut}`);

  const first = await page.evaluate(() => document.querySelector("table.matrix tbody .c-name").textContent.trim());
  await step("plan: the other study direction", () => page.click(".chip-links a:nth-child(2)"), (before) => location.search === "?variant=2" && document.querySelector("table.matrix tbody .c-name")?.textContent.trim() !== before, first);
  check(await page.evaluate(() => document.querySelectorAll('[data-walk="plan-variant"][data-state="with"]').length === 1), "plan: two study directions are shown as chosen");

  // Owner, 2026-09-23: on a 13-inch screen a third column left the plan too little room. With
  // nothing picked nothing stands beside the plan; what is picked opens in a panel floating over
  // the page, as the catalog's preview.
  check(!(await page.evaluate(() => Boolean(document.querySelector("#preview")))), "aside: with nothing picked a panel is beside the plan");
  await step("a module beside the plan", () => page.click('table.matrix tbody a[data-walk="module"]'), () => location.search.includes("open=") && document.querySelector("#preview .hero .mono"));
  const picked = await page.evaluate(() => ({
    id: document.querySelector("#preview .hero .mono").textContent.trim(),
    marked: document.querySelectorAll("table.matrix tbody tr.open").length,
    variant: location.search.includes("variant=2"),
    // Over the page, docked to its right edge: pinned in the page's scroll area (app.css „one
    // scroll area"), the page as wide as before.
    floats: ["absolute", "sticky"].includes(getComputedStyle(document.querySelector("#preview")).position) && document.querySelector("#preview").getBoundingClientRect().left < document.querySelector(".framed > .page").getBoundingClientRect().right - 100,
  }));
  check(picked.marked === 1 && picked.variant, `aside: ${picked.marked} rows are marked as open, the study direction was kept: ${picked.variant}`);
  check(picked.floats, "aside: the panel is a column of the page instead of floating over it");
  // The whole page, in place: the address stays the program's (with `full=1`), so the area,
  // its tab and the history do too; „Zurück" (and Esc) lead to the program with the module
  // beside it again, and the page is the module's own page.
  await step("the module as a whole page", () => page.click('#preview [data-action="fullscreen"]'), () => location.pathname.startsWith("/programs/bachelor-elektrotechnik") && location.search.includes("full=1") && document.querySelector(".module-page h2") && document.getElementById("sidebar"));
  check(await page.evaluate(() => document.querySelector('.rail a[data-area="programs"]')?.getAttribute("aria-current") === "page" && document.querySelector(".toc.jumps") !== null), "full page: the programs tab is not the current one, or the module's sidebar is missing");
  const entriesBefore = await page.evaluate(() => history.length);
  await step("Zurück leads to the program", () => page.click('[data-action="back"]'), () => location.pathname.startsWith("/programs/bachelor-elektrotechnik") && location.search.includes("open=") && !location.search.includes("full=") && document.querySelector("table.matrix"));
  check((await page.evaluate(() => history.length)) === entriesBefore, "back from the module page added a history entry instead of walking back");

  await step("close it again", () => page.click('#preview [data-action="close-detail"]'), () => !location.search.includes("open=") && !document.querySelector("#preview"));
  // Esc must now leave the program, not walk back into the module that was just closed.
  await step("Esc leaves the program", () => page.keyboard.press("Escape"), (line) => location.pathname === "/programs" && document.querySelectorAll(line).length > 100, LINE);
  await step("back into the program", () => page.click(`${LINE}[data-id^="bachelor-elektrotechnik"]`), () => location.pathname.startsWith("/programs/bachelor-elektrotechnik") && Boolean(document.querySelector("table.matrix")));
  // The module seen in full screen out of a program is none of the catalog's business: its tab
  // still leads to the list as it was left, and that list does not reveal the module.
  const catalogTab = await page.getAttribute('.rail a[data-area="catalog"]', "href");
  check(!catalogTab.startsWith("/catalog/module/"), `tabs: after a module out of a program the catalog leads to ${catalogTab}`);

  // A row of the plan that names no module says what the plan states about it, and where the
  // modules that can be chosen are listed.
  await step("a requirement of the plan", () => page.click('table.matrix tbody a[data-walk="plan-row"]'), () => location.search.includes("req=") && document.querySelector('#preview [aria-label="Zeile des Regelstudienplans"], #preview .note'));
  const requirement = await page.evaluate(() => ({
    title: document.querySelector("#preview .hero h2")?.textContent.trim(),
    marked: document.querySelectorAll("table.matrix tbody tr.open").length,
    claims: Boolean(document.querySelector("#preview .note")),
    catalogFirst: document.querySelector("#preview .dbody > *")?.matches('a[data-walk="catalog"]') ?? false,
    ways: document.querySelectorAll("#preview .dbody a[href]").length,
  }));
  check(Boolean(requirement.title) && requirement.marked === 1, `requirement: „${requirement.title}" is shown, ${requirement.marked} rows marked`);
  // Owner, 2026-09-23: the way into the catalog is the first thing of the panel, what can be
  // chosen follows under it.
  check(requirement.claims && requirement.catalogFirst && requirement.ways >= 2, "requirement: the panel does not say that the plan names no module, does not lead into the catalog first, or offers no way on");
  await step("closing the requirement", () => page.click('#preview [data-action="close-detail"]'), () => !location.search.includes("req=") && !document.querySelector("#preview"));

  // How the plan is drawn is personal: it stays in this browser, not in the URL (R9, R13).
  await step("plan: as a list", () => page.click("#sidebar .seg button:nth-of-type(2)"), () => document.querySelector("table.planlist") && !document.querySelector("table.matrix"));
  check(await page.evaluate(() => localStorage.getItem("betula.plan.shape") === "list" && !location.search.includes("shape")), "plan: the chosen drawing is not remembered, or it stands in the URL");
  await step("plan: as a matrix again", () => page.click("#sidebar .seg button:nth-of-type(1)"), () => Boolean(document.querySelector("table.matrix")));

  // The areas: one table, a group of rows per area, and the sidebar leads to each of them.
  await step("Bereiche", () => page.click('#sidebar .toc.views a:has-text("Wahlpflicht")'), () => location.pathname.endsWith("/areas") && document.querySelectorAll("table.areas tr.group").length > 3);
  const areas = await page.evaluate(() => {
    const left = (selector) => new Set([...document.querySelectorAll(selector)].map((cell) => Math.round(cell.getBoundingClientRect().left))).size;
    return {
      groups: document.querySelectorAll("table.areas tr.group").length,
      jumps: document.querySelectorAll("#sidebar .toc.jumps a").length,
      lines: ["c-id", "c-name", "c-lp", "c-sem"].map((column) => left(`table.areas tbody td.${column}, table.areas tbody th.${column}`)),
      tall: [...document.querySelectorAll("table.areas tbody tr:not(.group)")].filter((row) => row.getBoundingClientRect().height > 62).length,
      cut: [...document.querySelectorAll("#sidebar .toc.jumps a")].filter((a) => a.scrollWidth > a.clientWidth + 1).length,
    };
  });
  check(areas.groups === areas.jumps, `areas: ${areas.groups} areas in the table, ${areas.jumps} in the sidebar`);
  check(areas.lines.join() === "1,1,1,1", `areas: the columns do not line up (${areas.lines})`);
  check(areas.tall === 0, `areas: ${areas.tall} rows are higher than two lines`);
  check(areas.cut === 0, "areas: an entry of the sidebar is cut off");

  // An area says beside the page what it holds; a module picked from there keeps the area, so
  // closing it comes back to the list.
  await step("an area beside the page", () => page.click('table.areas tr.group a[data-walk="area"]'), () => location.search.includes("area=") && document.querySelector("#preview .linklist .pre"));
  const picked_area = await page.evaluate(() => ({
    modules: document.querySelectorAll('#preview .linklist a[data-walk="module"]').length,
    counted: Number(document.querySelector("#preview .badge.strong").textContent.replace(/\D/g, "")),
    marked: document.querySelectorAll("table.areas tr.group.open").length,
  }));
  check(picked_area.modules === picked_area.counted && picked_area.modules > 0, `area: the panel lists ${picked_area.modules} of ${picked_area.counted} modules`);
  check(picked_area.marked === 1, `area: ${picked_area.marked} areas are marked as picked`);
  await step("a module out of the area", () => page.click('#preview .linklist a[data-walk="module"]'), () => location.search.includes("area=") && location.search.includes("open=") && document.querySelector("#preview .hero .mono"));
  await step("closing it returns to the area", () => page.click('#preview [data-action="close-detail"]'), () => !location.search.includes("open=") && location.search.includes("area=") && document.querySelector("#preview .linklist .pre"));
  await step("closing the area too", () => page.click('#preview [data-action="close-detail"]'), () => !location.search.includes("area=") && !document.querySelector("#preview"));

  // The sidebar picks an area as the table does, and brings it into view.
  await step("an area from the sidebar", () => page.click("#sidebar .toc.jumps a:nth-of-type(2)"), () => location.search.includes("area=") && document.querySelector('#sidebar .toc.jumps a[aria-current="true"]'));
  await page.waitForTimeout(600);
  check(
    await page.evaluate(() => { const row = document.querySelector("table.areas tr.group.open")?.getBoundingClientRect(); return Boolean(row) && row.top >= 0 && row.top < innerHeight; }),
    "areas: the area picked in the sidebar is not in view",
  );

  // The views are the plan and the areas: „Mein Plan" is „Mein Studium" now (owner, 2026-10-04).
  const views = await page.evaluate(() => [...document.querySelectorAll("#sidebar .toc.views a")].map((a) => a.textContent));
  check(views.join() === "Regelstudienplan,Wahlpflicht & Bereiche", `views: ${views.join(" | ")}`);
  // All modules of the program are the catalog's (owner, 2026-09-25): the sidebar's way into the
  // catalog, with nothing picked, lists as many modules as the head counts.
  await step("closing the area", () => page.click('#preview [data-action="close-detail"]'), () => !location.search.includes("area=") && !document.querySelector("#preview"));
  const all = await page.evaluate(() => document.querySelector('#sidebar a[data-walk="catalog"]')?.getAttribute("href"));
  check(all === "/catalog?program=bachelor-elektrotechnik-2022", `the program's modules are at ${all}`);
  await step("all modules in the catalog", () => page.click('#sidebar a[data-walk="catalog"]'), () => location.pathname === "/catalog" && new URLSearchParams(location.search).get("program") === "bachelor-elektrotechnik-2022" && document.querySelector(".rows a.row"));
  const counted = Number(head.facts.find((fact) => /^\S+ Module$/.test(fact))?.replace(/\D/g, ""));
  const listed = await page.evaluate(() => Number(document.querySelector(".count")?.textContent.replace(/\D/g, "")));
  check(listed === counted && listed > 50, `all modules: the catalog lists ${listed} modules of the program, its head counts ${counted}`);
  // The old address of „Mein Plan" goes on to „Mein Studium", in the same history entry.
  await step("Mein Plan goes on to Mein Studium", () => page.evaluate(() => { const a = document.createElement("a"); a.href = "/programs/bachelor-elektrotechnik-2022/my-plan"; document.body.append(a); a.click(); a.remove(); }), () => location.pathname === "/study" && document.querySelector(".st-setup, .st-over"));
  await context.close();
}

// ---------- a row of the plan finds the area it names (Informatik B.Sc.) ----------
{
  const { page, step, context } = await open({ viewport: { width: 1500, height: 900 } }, "/programs/bachelor-informatik-2008");
  // The panel of the first row that reads exactly `name`: the area shown with its modules, and
  // the areas named beside it.
  const row = async (name) => {
    await step(`plan row „${name}"`, () => page.evaluate((name) => [...document.querySelectorAll('table.matrix tbody a[data-walk="plan-row"]')].find((a) => a.textContent.replace(/\s+/g, " ").trim().startsWith(name + " ") || a.textContent.replace(/\s+/g, " ").trim() === name)?.click(), name), (name) => location.search.includes("req=") && document.querySelector("#preview .hero h2")?.textContent.trim() === name, name);
    return page.evaluate(() => {
      const sections = [...document.querySelectorAll("#preview .section")];
      const labelled = (text) => sections.find((section) => section.querySelector(".label")?.textContent.startsWith(text));
      return {
        shown: [...(labelled("Vermutlich")?.querySelector(".label")?.childNodes ?? [])].filter((node) => node.nodeType === Node.TEXT_NODE).map((node) => node.textContent).join("").trim(),
        beside: [...(labelled("Passende Bereiche") ?? labelled("Kommt auch in Frage"))?.querySelectorAll(".pre b") ?? []].map((b) => b.textContent),
        note: document.querySelector("#preview .note")?.textContent ?? "",
      };
    });
  };
  // Owner, 2026-09-21: „Komplex Praktische Informatik" is Praktische Informatik, and nothing else.
  const praktisch = await row("Komplex Praktische Informatik");
  check(praktisch.shown === "Vermutlich Praktische Informatik" && praktisch.beside.length === 0, `plan row: „Komplex Praktische Informatik" points at ${praktisch.shown} and ${praktisch.beside.join(" | ") || "nothing else"}`);
  // „Anwendungsfach" is one of the Nebenfächer — not Praktische Mathematik, which has a row of its own.
  const anwendung = await row("Anwendungsfach");
  check([...anwendung.beside].sort().join(" | ") === "Bauingenieurwesen | Maschinenbau / Elektrotechnik | Mathematik | Physik | Wirtschaftswissenschaften", `plan row: „Anwendungsfach" points at ${anwendung.beside.join(" | ")}`);
  // Owner, 2026-09-23: an area opened out of a row is the same kind of panel — the catalog first —
  // and says how one got there („Anwendungsfach / Mathematik"); closing it returns to the row.
  await step("an area out of the row", () => page.evaluate(() => [...document.querySelectorAll('#preview .linklist a[data-walk="area"]')].find((a) => a.querySelector("b")?.textContent === "Mathematik")?.click()), () => location.search.includes("area=") && location.search.includes("req=") && document.querySelector("#preview .trail"));
  const within = await page.evaluate(() => ({
    trail: document.querySelector("#preview .trail")?.textContent.replace(/\s+/g, " ").trim(),
    catalogFirst: document.querySelector("#preview .dbody > *")?.matches('a[data-walk="catalog"]') ?? false,
  }));
  check(within.trail === "Anwendungsfach / Mathematik" && within.catalogFirst, `area out of a row: the path reads „${within.trail}", the catalog first: ${within.catalogFirst}`);
  await step("closing it returns to the row", () => page.click('#preview [data-action="close-detail"]'), () => !location.search.includes("area=") && location.search.includes("req=") && document.querySelector("#preview .hero h2")?.textContent.trim() === "Anwendungsfach");
  // A row that lists three complexes means each of them.
  const listed = await row("Wahlpflicht: Komplex Grundlagen der Informatik / Komplex Praktische Informatik / Komplex Angewandte und Technische Informatik");
  check(listed.beside.join(" | ") === "Grundlagen der Informatik | Praktische Informatik | Angewandte und Technische Informatik", `plan row: the listing row points at ${listed.beside.join(" | ")}`);
  // The way into the catalog is the first thing of the sidebar and takes along what is picked:
  // the row „Anwendungsfach" opens the catalog with its five areas.
  await row("Anwendungsfach");
  const jump = await page.evaluate(() => {
    const first = document.querySelector("#sidebar a, #sidebar button");
    return { first: first?.textContent.replace(/\s+/g, " ").trim(), href: document.querySelector('#sidebar a[data-walk="catalog"]')?.getAttribute("href") };
  });
  check(jump.first?.startsWith("Im Modulkatalog"), `sidebar: the first entry is „${jump.first}", not the way into the catalog`);
  const jumpAreas = new URL(jump.href ?? "/", base).searchParams.get("area")?.split(",") ?? [];
  check(jumpAreas.length === 5 && new URL(jump.href, base).searchParams.get("program") === "bachelor-informatik-2008", `sidebar: the catalog link of „Anwendungsfach" is ${jump.href}`);
  await step("into the catalog with the row's areas", () => page.click('#sidebar a[data-walk="catalog"]'), () => location.pathname === "/catalog" && document.querySelectorAll('.tag:has(em)').length >= 5 && document.querySelector("#pick-area")?.textContent.includes("5 Bereiche"));
  const tags = await page.evaluate(() => [...document.querySelectorAll(".tag")].filter((tag) => tag.querySelector("em")?.textContent === "Bereich").map((tag) => tag.textContent.replace("Bereich", "").trim()).sort());
  check(tags.join(" | ").includes("Physik") && tags.length === 5, `catalog: the areas of the row are ${tags.join(" | ")}`);
  // The address changes at once, the page a moment later (the catalog stays up until it is drawn).
  await step("back to the program", () => page.goBack(), () => location.pathname.startsWith("/programs/bachelor-informatik-2008") && location.search.includes("req=") && document.querySelector('table.matrix tbody a[data-walk="plan-row"]'));
  // The FÜS by its name: its list, no area.
  const fues = await row("Fachübergreifendes Studium");
  check(/Fachübergreifenden Studium/.test(fues.note) && !fues.shown.startsWith("Vermutlich") && fues.beside.length === 0, `plan row: the FÜS row reads „${fues.note.slice(0, 80)}" and points at ${fues.shown}`);
  await context.close();
}

// ---------- the plan at every width: no column runs into another ----------
// Owner, 2026-09-21: at 1200 px „Art" lay over „Modul" and the names stood a word to a line. The page
// shares the window with the sidebar and the panel on the right: „Art" gives way where the names would
// get narrower than 200 px, and where even that is not enough for the matrix, the plan is the list —
// never a matrix that scrolls sideways; the switch says so, and with room the matrix comes back without
// the choice being touched. At every width every cell keeps to its column, the semesters are equally
// wide, the credits of the list stand on one line with the sums of their semesters, and no row is
// taller than its name (up to 1200 px „Bereich" has given way, at 1000 px „Art" of the eight semesters).
{
  const fits = (page, chosen, semesters) => page.evaluate(([chosen, semesters]) => {
    // On the page in front: the other tabs' pages stay built behind it (stage.rs).
    const front = document.querySelector("#content > .page-stage.front");
    const table = front.querySelector("table.matrix, table.planlist");
    const panel = front.querySelector(".plan-block");
    const scroll = panel.querySelector(".table-scroll");
    // The names at their minimum and every semester (app.css, `matrix_min_width` in program.rs).
    const room = panel.getBoundingClientRect().width + 1 >= 200 + semesters * Math.max(48, 144 / semesters);
    const matrix = table.classList.contains("matrix");
    const found = [];
    if (matrix && scroll.scrollWidth > scroll.clientWidth + 1) found.push("the matrix scrolls sideways");
    if (chosen === "matrix" && matrix !== room) found.push(room ? "the list is drawn although the matrix fits" : "the matrix is drawn without room for it");
    const checked = front.querySelector('#sidebar .seg [aria-checked="true"]')?.textContent;
    if (checked !== (matrix ? "Matrix" : "Liste")) found.push(`the switch says „${checked}"`);
    const blocked = front.querySelector('#sidebar .seg button[aria-disabled="true"]')?.textContent;
    if ((blocked === "Matrix") === room || Boolean(front.querySelector("#sidebar .seg + .hint")) === room) found.push(`the switch blocks „${blocked ?? "nothing"}" and ${room ? "says" : "does not say"} why`);
    const pad = (cell, side) => parseFloat(getComputedStyle(cell)[side]) || 0;
    for (const cell of table.querySelectorAll("th, td")) {
      const box = cell.getBoundingClientRect();
      // A column that gave way is 0 px wide; the heading of a group cuts its own text.
      if (box.width < 1 || cell.closest("tr.group")) continue;
      const range = document.createRange();
      range.selectNodeContents(cell);
      const out = [...range.getClientRects()].some((r) => r.width > 0 && (r.left < box.left + pad(cell, "paddingLeft") - 0.5 || r.right > box.right - pad(cell, "paddingRight") + 0.5));
      if (out) found.push(`„${cell.textContent.trim().slice(0, 24)}" runs out of its column`);
    }
    const name = table.querySelector("thead .c-name").getBoundingClientRect().width;
    if (name < 199.5) found.push(`the names are ${Math.round(name)} px wide`);
    if (new Set([...table.querySelectorAll("thead th.c-sem")].map((th) => Math.round(th.getBoundingClientRect().width))).size > 1) found.push("the semesters are not equally wide");
    if (new Set([...table.querySelectorAll("td.c-lp")].map((td) => Math.round(td.getBoundingClientRect().right))).size > 1) found.push("the credits do not stand on one line");
    return found;
  }, [chosen, semesters]);
  // Six semesters in both drawings, and the matrices of two and of eight.
  for (const [slug, semesters, shapes] of [["bachelor-informatik-2008", 6, ["matrix", "list"]], ["master-forensic-sciences-and-engineering-2011", 2, ["matrix"]], ["bachelor-pflegewissenschaft-2025-84", 8, ["matrix"]]]) {
    const { page, context } = await open({ viewport: { width: 1800, height: 900 } }, `/programs/${slug}/plan`);
    for (const shape of shapes) {
      if (shape === "list") await page.click("#sidebar .seg button:nth-of-type(2)");
      await page.waitForSelector(shape === "list" ? "table.planlist" : "table.matrix");
      // From narrow to wide: the matrix has to come back by itself.
      for (const width of [1000, 1100, 1200, 1250, 1300, 1350, 1400, 1450, 1500, 1600, 1800]) {
        await page.setViewportSize({ width, height: 900 });
        // The plan's panel is measured when the browser lays out the new width.
        await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
        const found = [...(await fits(page, shape, semesters)), ...(await tallRows(page))];
        check(found.length === 0, `plan (${slug}, ${shape}) at ${width} px: ${found.slice(0, 3).join("; ")}`);
      }
      if (shape === "matrix") check(await page.evaluate(() => localStorage.getItem("betula.plan.shape") === null), `plan (${slug}): a narrow window changed the chosen drawing`);
    }
    await context.close();
  }
}

// ---------- a program on a phone: the plan is a list, the page does not scroll sideways ----------
{
  const { page, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, "/programs/bachelor-elektrotechnik-2022");
  const phone = await page.evaluate(() => {
    return {
      wide: document.documentElement.scrollWidth <= innerWidth + 1,
      list: Boolean(document.querySelector("table.planlist")) && !document.querySelector("table.matrix") && !document.querySelector('#sidebar [aria-label="Darstellung des Regelstudienplans"]'),
      // No box above the page (owner, 2026-10-15): the views end the head, the sidebar (the way
      // into the catalog, what is related) ends the page, after the regulations and the module tree.
      views: (() => {
        const views = document.querySelector(".prog-head .toc.views");
        const head = document.querySelector(".prog-head").getBoundingClientRect();
        return Boolean(views?.offsetHeight) && views.getBoundingClientRect().bottom <= head.bottom && !document.querySelector("#sidebar .toc.views")?.offsetHeight
          && [...views.querySelectorAll("a")].map((a) => a.textContent).join() === "Regelstudienplan,Wahlpflicht & Bereiche";
      })(),
      end: (() => {
        const side = document.querySelector("#sidebar").getBoundingClientRect();
        const docs = document.querySelector("#dokumente");
        return side.top >= document.querySelector(".page").getBoundingClientRect().bottom - 1 && document.querySelector("#sidebar .catalog-jump a")?.offsetHeight > 0
          && [...docs.querySelectorAll(".docrow")].at(-1)?.getAttribute("href")?.includes("qisserver") && docs.getBoundingClientRect().bottom <= side.top;
      })(),
      // Nothing to do with the program any more: it is chosen in „Mein Studium".
      actions: [...document.querySelectorAll("#sidebar .action, #sidebar button")].map((a) => a.textContent.trim()).filter((text) => /Studiengang setzen|Stundenplan|Studium planen|Ordnungen|Verzeichnis/.test(text)),
      // „Zurück" at the head, left of the search, and the way to the profile page gone meanwhile.
      back: (() => {
        const back = document.querySelector(".topbar .top-back.on");
        const search = document.querySelector(".topbar .search").getBoundingClientRect();
        const profile = document.querySelector(".topbar .top-profile");
        const box = back?.getBoundingClientRect();
        return Boolean(box) && Math.round(box.width) === 48 && box.right < search.left && back.getAttribute("href") === "/programs" && profile.inert && getComputedStyle(profile).opacity === "0"
          && !document.querySelector('.prog-head [data-action="back"]')?.offsetHeight && Math.round(innerWidth - search.right) === 12;
      })(),
      facts: [...document.querySelectorAll(".prog-facts .pfact")].every((f) => f.getBoundingClientRect().right <= innerWidth),
      // The numbers one row, each its figure over its word; the line under the name one line, „PO"
      // for „Prüfungsordnung" (owner, 2026-10-15: „wie goofy der header … aussieht").
      head: (() => {
        const tops = (els) => new Set([...els].map((el) => Math.round(el.getBoundingClientRect().top)));
        const meta = document.querySelector(".prog-meta");
        return { facts: tops(document.querySelectorAll(".prog-facts .pfact")).size, meta: tops(meta.children).size, text: meta.innerText.replace(/\s+/g, " ").trim(), abbr: meta.querySelector(".po-abbr")?.offsetWidth > 0 && meta.querySelector(".po-word")?.getBoundingClientRect().width <= 1 };
      })(),
    };
  });
  check(phone.wide, "phone: the program page scrolls sideways");
  check(phone.list, "phone: the plan is not the list, or a switch to the matrix is offered");
  check(phone.views, "phone: the views of the program do not end its head");
  check(phone.end, "phone: the regulations with the module tree, and after them the way into the catalog, do not end the page");
  check(phone.actions.length === 0, `phone: the sidebar still offers ${phone.actions.join(", ")}`);
  check(phone.back, "phone: „Zurück\" is not at the head left of the search, or the way to the profile page did not give way");
  check(phone.facts, "phone: a number of the head stands outside the screen");
  // „Prüfungsordnung" is still there for a screen reader, „PO" is what is seen.
  check(phone.head.facts === 1 && phone.head.meta === 1 && phone.head.abbr && /^B\.Sc\. Prüfungsordnung PO 2022 aktuell$/.test(phone.head.text), `phone: the head wraps: ${JSON.stringify(phone.head)}`);
  // A module comes up as a sheet over the page (owner, 2026-10-06), in the program's area (the
  // address keeps `open=`), one history entry; closed, the program is there as it was, a step back.
  // The page came from outside: its first use lays the way up under it, up to Start
  // (folia/crates/shell/src/nav.rs, `seed`).
  // Clicked through the DOM: the sticky top bar covers the first rows on a phone.
  const stack = () => page.evaluate(() => ({ entries: navigation.entries().map((entry) => { const url = new URL(entry.url); return url.pathname + url.search; }), at: navigation.currentEntry.index }));
  await page.evaluate(() => document.querySelector('table.planlist tbody a[data-walk="module"]')?.click());
  await page.waitForFunction(() => location.pathname.startsWith("/programs/bachelor-elektrotechnik") && location.search.includes("open=") && document.querySelector(".detail.is-module h2") && document.querySelector("table.planlist"), null, { timeout: 8000 }).catch(() => problems.push("phone: a module did not come up as a sheet over the program"));
  const opened = await stack();
  check(opened.entries.slice(0, opened.at).join(" ") === "/ /study /programs /programs/bachelor-elektrotechnik-2022" && opened.entries[opened.at].includes("open=") && opened.at === opened.entries.length - 1,
    `phone: opening a module took more than one history entry, or the first use did not lay the way up under the program (${JSON.stringify(opened)})`);
  await page.waitForTimeout(400);
  await page.tap('.detail.is-module [data-action="close-detail"]');
  await page.waitForFunction(() => !location.search.includes("open=") && !document.querySelector(".detail.is-module") && document.querySelector("table.planlist"), null, { timeout: 8000 }).catch(() => problems.push("phone: closing the module did not leave the program"));
  const closed = await stack();
  check(closed.at === opened.at - 1 && closed.entries.length === opened.entries.length, `phone: closing the module added a step to the history (${JSON.stringify(closed)})`);
  // An area is the page on a phone, and a module picked from it comes up over it.
  await page.evaluate(() => [...document.querySelectorAll(".prog-head .toc.views a")].find((a) => a.getAttribute("href")?.endsWith("/areas"))?.click());
  await page.waitForFunction(() => location.pathname.endsWith("/areas") && document.querySelector('table.areas tr.group a[data-walk="area"]'), null, { timeout: 8000 }).catch(() => problems.push("phone: the areas did not open"));
  await page.evaluate(() => document.querySelector('table.areas tr.group a[data-walk="area"]')?.click());
  await page.waitForFunction(() => location.search.includes("area=") && document.querySelector("#preview .linklist .pre") && !document.querySelector("table.areas"), null, { timeout: 8000 }).catch(() => problems.push("phone: an area did not become the page"));
  // The area has its own way back, to the program, and it is the box at the head as well (owner,
  // 2026-10-15: „mach den zurückbutton mal überall nach oben"): the area's link steps aside.
  await page.waitForFunction(() => {
    const back = document.querySelector(".top-back.on")?.getAttribute("href") ?? "";
    return back.startsWith("/programs/bachelor-elektrotechnik-2022") && !back.includes("area=") && document.querySelector(".top-profile").inert && !document.querySelector('#preview .hero-top [data-action="back"]')?.offsetHeight;
  }, null, { timeout: 2000 }).catch(() => problems.push("phone: an area as the page does not have its way back to the program at the head"));
  await page.evaluate(() => document.querySelector('#preview .linklist a[data-walk="module"]')?.click());
  await page.waitForFunction(() => location.search.includes("area=") && location.search.includes("open=") && document.querySelector(".detail.is-module h2") && document.querySelector(".picked-page .linklist .pre"), null, { timeout: 8000 }).catch(() => problems.push("phone: a module out of an area did not come up over it"));
  await page.waitForTimeout(400);
  await page.tap('.detail.is-module [data-action="close-detail"]');
  await page.waitForFunction(() => location.search.includes("area=") && !location.search.includes("open=") && !document.querySelector(".detail.is-module") && document.querySelector("#preview .linklist .pre"), null, { timeout: 8000 }).catch(() => problems.push("phone: closing the module did not leave the area"));
  await context.close();
}

// ---------- the plan on a phone: every row as tall as its name ----------
// „Art" and „Bereich" have given way, and Informatik B.Sc. names „Grundstudium" or „Fachstudium" in
// every row: the rows are one line high, two or four where the name takes them (37, 56, 95 px).
{
  const { page, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, "/programs/bachelor-informatik-2008/plan");
  const widths = await page.evaluate(() => [...document.querySelectorAll("table.planlist thead th")].map((th) => Math.round(th.getBoundingClientRect().width)));
  check(widths.length === 4 && widths[1] === 0 && widths[2] === 0, `phone: „Art" and „Bereich" have not given way (${widths.join(", ")} px)`);
  const tall = await tallRows(page);
  check(tall.length === 0, `phone: ${tall.length} rows of the plan are taller than their names: ${tall.slice(0, 3).join("; ")}`);
  await context.close();
}

// ---------- phone: the filters are a sheet ----------
{
  const { page, step, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, "/programs");
  check(await page.evaluate(() => document.querySelector("details.facc").getBoundingClientRect().top < innerHeight * 0.4), "phone: the first faculty does not start on the first screen");
  // The way back at the head, left of the search, up to „Mein Studium" (owner, 2026-10-17), and
  // the way to the profile page gone meanwhile.
  check(await page.evaluate(() => {
    const back = document.querySelector(".topbar .top-back.on");
    const search = document.querySelector(".topbar .search").getBoundingClientRect();
    const profile = document.querySelector(".topbar .top-profile");
    return Math.round(back?.getBoundingClientRect().width ?? 0) === 48 && back.getAttribute("href") === "/study" && back.getBoundingClientRect().right < search.left
      && profile.inert && getComputedStyle(profile).opacity === "0";
  }), "phone: the overview has no way back to „Mein Studium“ at the head, or the way to the profile page did not give way");
  await step("phone: the sheet opens", () => page.tap(".sheet-toggle"), () => document.querySelector(".sidebar.sheet.open") && document.querySelector(".sidebar.sheet").getBoundingClientRect().top < innerHeight - 200);
  await step("phone: a filter in the sheet", () => page.tap('#sidebar a.chip:has-text("Bachelor")'), () => location.search === "?level=bachelor" && document.querySelector(".sidebar.sheet.open"));
  await step("phone: the sheet closes", () => page.tap('#sidebar .sheet-only a'), () => !document.querySelector(".sidebar.sheet.open"));
  check(await page.evaluate(() => [...document.querySelectorAll(".facc-h")].every((head) => head.scrollWidth <= head.clientWidth + 1)), "phone: a faculty's heading does not fit");
  // On a phone a subject is a row (owner, 2026-10-14: „Variante 1 mit ausklappen"): one program a
  // link, several a row that folds open and shows them by degree; nothing leaves the screen.
  const phoneList = await page.evaluate(() => ({
    desk: [...document.querySelectorAll(".fx-desk")].some((table) => table.getBoundingClientRect().height > 0),
    wide: document.documentElement.scrollWidth <= innerWidth + 1,
  }));
  check(!phoneList.desk && phoneList.wide, `phone: the matrix shows (${phoneList.desk}) or the page scrolls sideways (${!phoneList.wide})`);
  // The filter of the sheet left every faculty with something in it open: this one is closed first.
  await page.evaluate(() => { document.querySelector("details.facc:nth-of-type(1)").open = false; });
  await step("phone: a faculty opens", () => page.tap("details.facc:nth-of-type(1) > summary"), () => document.querySelector("details.facc:nth-of-type(1)").open);
  await step("phone: a subject folds open", () => page.tap("details.facc:nth-of-type(1) details.sj-w > summary"), () => { const subject = document.querySelector("details.facc:nth-of-type(1) details.sj-w"); return subject.open && subject.querySelector(".of-lvl") && [...subject.querySelectorAll("a.of")].every((a) => a.getBoundingClientRect().right <= innerWidth + 1); });
  await context.close();
}

// ---------- without JavaScript ----------
{
  const context = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 1300, height: 900 } });
  const page = await context.newPage();
  // The site's overview (docs/folia/folia-refactor.md §4.1): the programs by faculty with their
  // jumps, no filters; an address with filters (a link from before) still lists what they hold.
  await page.goto(base + "/programs", { waitUntil: "domcontentloaded" });
  check((await page.locator("#sidebar a.chip").count()) === 0, "no JS: the site's overview has filters");
  check((await page.locator("#sidebar .jumps a").count()) > 3, "no JS: the overview has no jumps to its faculties");
  check((await page.getAttribute('.rail a[data-area="catalog"]', "href")) === "/catalog", "no JS: a tab is not the plain link to its area");
  const all = await page.locator(LINE).count();
  // The accordions open without JavaScript.
  await page.click("details.facc:nth-of-type(1) > summary");
  check(await page.evaluate(() => document.querySelector("details.facc:nth-of-type(1)").open && document.querySelector("details.facc:nth-of-type(1) a.prog-line").getBoundingClientRect().height > 0), "no JS: a faculty does not open");
  await page.goto(base + "/programs?level=master", { waitUntil: "domcontentloaded" });
  const masters = await page.locator(LINE).count();
  check(masters > 0 && masters < all, `no JS: an address with a filter does not list what it holds (${masters} of ${all})`);
  // The server's page draws the plan as a list, and only as a list.
  await page.goto(base + "/programs/bachelor-informatik-2008", { waitUntil: "domcontentloaded" });
  check((await page.locator("table.planlist").count()) === 1 && (await page.locator("table.matrix").count()) === 0, "no JS: the server's plan is not the list");
  await context.close();
  // On a phone the overview's way back leads to „Mein Studium", which needs the browser app: without
  // JavaScript it is not there, and the way to the profile page is.
  const phone = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
  const narrow = await phone.newPage();
  await narrow.goto(base + "/programs", { waitUntil: "domcontentloaded" });
  check(await narrow.evaluate(() => !document.querySelector(".topbar .top-back")?.getBoundingClientRect().width && !document.querySelector(".topbar .top-profile").inert && Math.round(document.querySelector(".topbar .top-profile").getBoundingClientRect().width) === 48 && getComputedStyle(document.querySelector(".topbar .top-profile")).opacity === "1"),
    "no JS, phone: the overview has a way back to „Mein Studium“, or no way to the profile page");
  await phone.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
