// Checks the programs area: the overview by faculty, its sidebar, and the frame every page shares.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node programs.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Walks: the sidebar is in the same place on every page → overview (faculties in order, every program
// once, nothing cut off) → filters (links, the sidebar is not rebuilt, the search keeps them) → jump to a
// faculty without a history entry → program page (views in the sidebar, „Zurück" and Esc lead back to the
// program in the overview) → the rail's items are tabs that remember where their area was left → the
// program page itself (head, one study plan per study direction, matrix or list, a module beside it
// and the way back out of it, areas, all modules) → phone
// (filters in a sheet) → the overview without JavaScript.
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

  // „Zurück" (and Esc) lead to the overview, and the overview shows the program again.
  const inView = () => page.evaluate(() => { const pill = document.querySelector('.program-pill[data-id^="bachelor-informatik"]')?.getBoundingClientRect(); return Boolean(pill) && pill.top >= 0 && pill.bottom <= innerHeight; });
  check(await page.evaluate(() => [...document.querySelectorAll('[data-action="back"] kbd')].some((k) => k.textContent === "Esc")), "program page: no back link with its shortcut");
  await step("program page: Zurück", () => page.click('[data-action="back"]'), () => location.pathname === "/programs" && document.querySelectorAll(".program-pill").length > 100);
  await page.waitForTimeout(400);
  check(await inView(), "back on the overview the program is not in view");
  await step("program page again", () => page.click('.program-pill[data-id^="bachelor-informatik"]'), () => location.pathname.startsWith("/programs/bachelor-informatik"));
  await step("program page: Esc", () => page.keyboard.press("Escape"), () => location.pathname === "/programs");

  // The rail's items are tabs: each leads to where its area was left.
  const tab = (area) => page.getAttribute(`.rail a[data-area="${area}"]`, "href");
  await step("a filter in the overview", () => page.click('#sidebar a.chip:has-text("Master")'), () => location.search === "?level=master");
  await step("a program", () => page.click('.program-pill[data-id^="master-informatik"]'), () => location.pathname.startsWith("/programs/master-informatik"));
  const programPage = await page.evaluate(() => location.pathname);
  check((await tab("programs")) === "/programs?level=master", `tabs: on a program's page the own tab leads to ${await tab("programs")}, not to the overview as it was left`);
  await step("tab: Module", () => page.click('.rail a[data-area="catalog"]'), () => location.pathname === "/catalog" && document.querySelector(".rows a.row"));
  await step("a filter in the catalog", () => page.click('#filters a.chip:has-text("Winter")'), () => location.search.includes("turnus=winter"));
  check((await tab("programs")) === programPage, `tabs: the tab of the programs leads to ${await tab("programs")}, the area was left at ${programPage}`);
  await step("tab: Studium returns to the program", () => page.click('.rail a[data-area="programs"]'), (path) => location.pathname === path && document.querySelector('[data-walk="program-page"]'), programPage);
  check((await tab("catalog")) === "/catalog?turnus=winter", `tabs: the tab of the catalog leads to ${await tab("catalog")}`);
  // Coming from another area, „Zurück" leads up to the overview (not back to that other area).
  await step("Zurück after a change of tabs", () => page.click('[data-action="back"]'), () => location.pathname === "/programs" && location.search === "?level=master");
  await step("tab: Module returns to the filtered list", () => page.click('.rail a[data-area="catalog"]'), () => location.pathname === "/catalog" && location.search.includes("turnus=winter"));
  check((await tab("catalog")) === "/catalog", "tabs: on the list the own tab is not the plain list");

  await step("home", () => page.click('.rail a[href="/"]'), () => location.pathname === "/" && document.querySelector(".home .intro"));
  check(JSON.stringify(await box(page, "#sidebar")) === JSON.stringify(frame), "home: the sidebar is not in the frame's place");
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
      meta: document.querySelector(".prog-meta").textContent.replace(/\s+/g, " ").trim(),
    };
  });
  check(head.facts.join(" | ") === "6 Semester | 180 LP | 85 Module | 130 FÜS-Module", `head: the numbers read ${head.facts.join(" | ")}`);
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

  // A module opens in the panel on the right, which otherwise holds the numbers of the view.
  const numbers = await page.evaluate(() => ({
    panel: Boolean(document.querySelector("#preview .bars")),
    facts: document.querySelectorAll("#preview .facts .fact").length,
    over: (() => {
      const table = document.querySelector("table.matrix").getBoundingClientRect();
      const aside = document.querySelector("#preview").getBoundingClientRect();
      return table.right > aside.left + 1;
    })(),
  }));
  check(numbers.panel && numbers.facts >= 4, `aside: the numbers of the plan are not beside it (bars: ${numbers.panel}, facts: ${numbers.facts})`);
  check(!numbers.over, "aside: the panel lies over the table instead of beside it");
  await step("a module beside the plan", () => page.click('table.matrix tbody a[data-walk="module"]'), () => location.search.includes("open=") && document.querySelector("#preview .hero .mono"));
  const picked = await page.evaluate(() => ({
    id: document.querySelector("#preview .hero .mono").textContent.trim(),
    marked: document.querySelectorAll("table.matrix tbody tr.open").length,
    variant: location.search.includes("variant=2"),
  }));
  check(picked.marked === 1 && picked.variant, `aside: ${picked.marked} rows are marked as open, the study direction was kept: ${picked.variant}`);
  // The whole page, in place: the address stays the program's (with `full=1`), so the area,
  // its tab and the history do too; „Zurück" (and Esc) lead to the program with the module
  // beside it again, and the page is the module's own page.
  await step("the module as a whole page", () => page.click('#preview [data-action="fullscreen"]'), () => location.pathname.startsWith("/programs/bachelor-elektrotechnik") && location.search.includes("full=1") && document.querySelector(".module-page h2") && document.getElementById("sidebar"));
  check(await page.evaluate(() => document.querySelector('.rail a[data-area="programs"]')?.getAttribute("aria-current") === "page" && document.querySelector(".toc.jumps") !== null), "full page: the programs tab is not the current one, or the module's sidebar is missing");
  const entriesBefore = await page.evaluate(() => history.length);
  await step("Zurück leads to the program", () => page.click('[data-action="back"]'), () => location.pathname.startsWith("/programs/bachelor-elektrotechnik") && location.search.includes("open=") && !location.search.includes("full=") && document.querySelector("table.matrix"));
  check((await page.evaluate(() => history.length)) === entriesBefore, "back from the module page added a history entry instead of walking back");

  await step("close it again", () => page.click('#preview [data-action="close-detail"]'), () => !location.search.includes("open=") && Boolean(document.querySelector("#preview .bars")));
  // Esc must now leave the program, not walk back into the module that was just closed.
  await step("Esc leaves the program", () => page.keyboard.press("Escape"), () => location.pathname === "/programs" && document.querySelectorAll(".program-pill").length > 100);
  await step("back into the program", () => page.click('.program-pill[data-id^="bachelor-elektrotechnik"]'), () => location.pathname.startsWith("/programs/bachelor-elektrotechnik") && Boolean(document.querySelector("table.matrix")));
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
    ways: document.querySelectorAll("#preview .linklist .pre").length,
  }));
  check(Boolean(requirement.title) && requirement.marked === 1, `requirement: „${requirement.title}" is shown, ${requirement.marked} rows marked`);
  check(requirement.claims && requirement.ways >= 2, "requirement: the panel does not say that the plan names no module, or offers no way on");
  await step("closing the requirement", () => page.click('#preview [data-action="close-detail"]'), () => !location.search.includes("req=") && Boolean(document.querySelector("#preview .bars")));

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
  await step("closing the area too", () => page.click('#preview [data-action="close-detail"]'), () => !location.search.includes("area=") && Boolean(document.querySelector("#preview .facts")));

  // The sidebar picks an area as the table does, and brings it into view.
  await step("an area from the sidebar", () => page.click("#sidebar .toc.jumps a:nth-of-type(2)"), () => location.search.includes("area=") && document.querySelector('#sidebar .toc.jumps a[aria-current="true"]'));
  await page.waitForTimeout(600);
  check(
    await page.evaluate(() => { const row = document.querySelector("table.areas tr.group.open")?.getBoundingClientRect(); return Boolean(row) && row.top >= 0 && row.top < innerHeight; }),
    "areas: the area picked in the sidebar is not in view",
  );

  // All modules: the same columns, one line per module, its area beside it.
  await step("Alle Module", () => page.click('#sidebar .toc.views a:has-text("Alle Module")'), () => location.pathname.endsWith("/modules") && document.querySelector("table.modules tbody tr"));
  const modules = await page.evaluate(() => ({
    rows: document.querySelectorAll("table.modules tbody tr").length,
    tall: [...document.querySelectorAll("table.modules tbody tr")].filter((row) => row.getBoundingClientRect().height > 62).length,
    areas: [...document.querySelectorAll("table.modules td.c-area")].filter((cell) => cell.firstElementChild?.scrollWidth > cell.firstElementChild?.clientWidth + 1 && !cell.title).length,
    opens: document.querySelector('table.modules tbody a[data-walk="module"]')?.getAttribute("href")?.includes("open=") === true,
    wide: document.documentElement.scrollWidth <= innerWidth + 1,
  }));
  check(modules.rows > 50 && modules.tall === 0, `modules: ${modules.rows} rows, ${modules.tall} of them higher than two lines`);
  check(modules.areas === 0, "modules: a shortened area does not carry its full path");
  check(modules.opens, "modules: a row does not open its module beside the page");
  check(modules.wide, "modules: the page scrolls sideways");
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
  await step("back to the program", () => page.goBack(), () => location.pathname.startsWith("/programs/bachelor-informatik-2008") && location.search.includes("req="));
  // The FÜS by its name: its list, no area.
  const fues = await row("Fachübergreifendes Studium");
  check(/Fachübergreifenden Studium/.test(fues.note) && !fues.shown.startsWith("Vermutlich") && fues.beside.length === 0, `plan row: the FÜS row reads „${fues.note.slice(0, 80)}" and points at ${fues.shown}`);
  await context.close();
}

// ---------- a program on a phone: the plan is a list, the page does not scroll sideways ----------
{
  const { page, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, "/programs/bachelor-elektrotechnik-2022");
  const phone = await page.evaluate(() => {
    return {
      wide: document.documentElement.scrollWidth <= innerWidth + 1,
      list: Boolean(document.querySelector("table.planlist")) && !document.querySelector("table.matrix") && !document.querySelector('#sidebar [aria-label="Darstellung des Regelstudienplans"]'),
      views: document.querySelector("#sidebar .toc.views").getBoundingClientRect().top < document.querySelector(".prog-head").getBoundingClientRect().top,
      facts: [...document.querySelectorAll(".prog-facts .pfact")].every((f) => f.getBoundingClientRect().right <= innerWidth),
    };
  });
  check(phone.wide, "phone: the program page scrolls sideways");
  check(phone.list, "phone: the plan is not the list, or a switch to the matrix is offered");
  check(phone.views, "phone: the views of the program are not above the page");
  check(phone.facts, "phone: a number of the head stands outside the screen");
  // A module has no room beside the page here: one tap, and it is the page, in the program's
  // area (the address keeps `open=`), and „Zurück" is the program again.
  // Clicked through the DOM: the sticky top bar covers the first rows on a phone.
  const entries = await page.evaluate(() => history.length);
  await page.evaluate(() => document.querySelector('table.planlist tbody a[data-walk="module"]')?.click());
  await page.waitForFunction(() => location.pathname.startsWith("/programs/bachelor-elektrotechnik") && location.search.includes("open=") && document.querySelector(".module-page h2") && !document.querySelector("table.planlist"), null, { timeout: 8000 }).catch(() => problems.push("phone: a module did not become the page"));
  check((await page.evaluate(() => history.length)) === entries + 1, "phone: opening a module took more than one history entry");
  await page.evaluate(() => document.querySelector('[data-action="back"]')?.click());
  await page.waitForFunction(() => !location.search.includes("open=") && document.querySelector("table.planlist"), null, { timeout: 8000 }).catch(() => problems.push("phone: Zurück did not lead back to the program"));
  // An area, too, is the page on a phone, and a module picked from it leads back to it.
  await page.evaluate(() => [...document.querySelectorAll("#sidebar .toc.views a")].find((a) => a.getAttribute("href")?.endsWith("/areas"))?.click());
  await page.waitForFunction(() => location.pathname.endsWith("/areas") && document.querySelector('table.areas tr.group a[data-walk="area"]'), null, { timeout: 8000 }).catch(() => problems.push("phone: the areas did not open"));
  await page.evaluate(() => document.querySelector('table.areas tr.group a[data-walk="area"]')?.click());
  await page.waitForFunction(() => location.search.includes("area=") && document.querySelector("#preview .linklist .pre") && !document.querySelector("table.areas"), null, { timeout: 8000 }).catch(() => problems.push("phone: an area did not become the page"));
  await page.evaluate(() => document.querySelector('#preview .linklist a[data-walk="module"]')?.click());
  await page.waitForFunction(() => location.search.includes("area=") && location.search.includes("open=") && document.querySelector(".module-page h2"), null, { timeout: 8000 }).catch(() => problems.push("phone: a module out of an area did not become the page"));
  await page.evaluate(() => document.querySelector('[data-action="back"]')?.click());
  await page.waitForFunction(() => location.search.includes("area=") && !location.search.includes("open=") && document.querySelector("#preview .linklist .pre"), null, { timeout: 8000 }).catch(() => problems.push("phone: Zurück from the module did not lead back to the area"));
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
  check((await page.getAttribute('.rail a[data-area="catalog"]', "href")) === "/catalog", "no JS: a tab is not the plain link to its area");
  const before = await page.locator(".program-pill").count();
  await page.click('#sidebar a.chip:has-text("Doppelabschluss")');
  await page.waitForURL(/form=double/);
  check(page.url().includes("level=master") && (await page.locator(".program-pill").count()) < before, "no JS: a filter link lost the other filter, or did not filter");
  // The server's page draws the plan as a list, and only as a list.
  await page.goto(base + "/programs/bachelor-informatik-2008", { waitUntil: "domcontentloaded" });
  check((await page.locator("table.planlist").count()) === 1 && (await page.locator("table.matrix").count()) === 0, "no JS: the server's plan is not the list");
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
