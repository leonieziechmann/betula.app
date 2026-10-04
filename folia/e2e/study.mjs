// Checks „Mein Studium" (folia/crates/planner/src/study, folia/crates/plans/src/study.rs), the first
// page of the Studium tab (owner, mockup of 2026-10-04).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node study.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// 1 a fresh browser: the Studium tab leads to it; it asks to set the study up once and stores
//   nothing by being looked at; the program, a Studienbeginn of last winter and „1. und 2. FS nach
//   Regelstudienplan füllen" set it up: the overview, the first two semesters filled.
// 2 Informatik B.Sc. in its third semester, the first two filled and nothing ticked off: one hint
//   for what is not ticked off; ticking off counts and keeps the row in its place; „Module
//   hinzufügen" puts the plan's third semester in; the Stundenplan's tab counts it; a module moved
//   by its menu and taken back, removed and taken back; an area's dialog; a module beside the page,
//   in full and back; the Gesamtplan, a grey line of the plan added with a click, the view kept; a
//   reload keeps all of it; nothing of it in a request.
// 3 a phone: one semester as a card as wide as the page, turned by ‹ › and a swipe, the last page
//   adds a semester; the areas as a sheet; the sidebar as a sheet („Anpassen"); nothing scrolls
//   sideways; boxes as tall as a finger.
// 4 the Stundenplan: its import from „Mein Studium" takes the Wiederholer.
// 5 without the app: one page for everybody, for no index; the server's tab is the overview.
// Needs a snapshot whose current semester is WiSe 2026/27. Fails on a page load after takeover, a
// console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
const THIRD = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n";
// The first two semesters as „nach Regelstudienplan füllen" leaves them.
const FILLED = [
  "m\t2025W\t12104\t1\t\tplan", "m\t2025W\t12107\t1\t\tplan", "m\t2025W\t12102\t1\t\tplan", "m\t2025W\t11112\t1\t\tplan",
  "m\t2026S\t12101\t1\t\tplan", "m\t2026S\t11903\t1\t\tplan", "m\t2026S\t11113\t1\t\tplan",
  "p\t1\t2025W\t079-82-2008\t17\t1-1\t6\tfues\t\tFachübergreifendes Studium",
  "p\t2\t2026S\t079-82-2008\t10\t2-2\t4\telective\t\tProseminar oder Praktikum",
].join("\n") + "\n";

const takeover = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
const open = async (options, path, { requests } = {}) => {
  const context = await browser.newContext(options);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  if (requests) page.on("request", (r) => requests.push(r.url() + " " + (r.postData() || "")));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  const step = async (name, action, until, arg) => {
    const started = Date.now();
    await action();
    try {
      await page.waitForFunction(until, arg, { timeout: 8000 });
      await page.waitForFunction(() => !document.querySelector(".pending-page.held"), null, { timeout: 2000 });
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    timings[name] = Date.now() - started;
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
  };
  // The store as given, then the page anew.
  const seed = async (entries) => {
    await page.evaluate((entries) => { for (const [key, text] of entries) localStorage.setItem(key, text); }, entries);
    await page.reload({ waitUntil: "domcontentloaded" });
    await takeover(page);
    await page.evaluate(() => { window.__marker = 1; });
  };
  return { page, step, seed, context };
};
const stored = (page, key) => page.evaluate((key) => localStorage.getItem(key), key);
const lines = async (page, prefix) => ((await stored(page, PLAN)) || "").split("\n").filter((line) => line.startsWith(prefix));
// The rows of the semester in focus: their names, whether ticked off, their marks.
const rows = (page) => page.evaluate(() => [...document.querySelectorAll(".st-card-sem .st-row")].map((row) => ({
  name: row.querySelector(".st-name")?.textContent,
  done: row.querySelector(".st-tick")?.getAttribute("aria-checked") === "true",
  chips: [...row.querySelectorAll(".st-chip")].map((chip) => chip.textContent),
})));
const focused = (page) => page.evaluate(() => document.querySelector(".st-card-sem h2")?.textContent);
const headline = (page) => page.evaluate(() => document.querySelector(".st-headline")?.textContent);
const current = await (async () => {
  const response = await fetch(base + "/api/status").then((r) => r.json()).catch(() => null);
  return response?.snapshot?.current_semester;
})();
if (current !== "2026W") {
  console.log(JSON.stringify({ skipped: `the snapshot's current semester is ${current}, the checks need 2026W` }));
  await browser.close();
  process.exit(0);
}

// ---------- 1: a fresh browser ----------
{
  const requests = [];
  const { page, step, context } = await open({ viewport: { width: 1440, height: 900 } }, "/catalog", { requests });
  check((await page.getAttribute('.rail a[data-area="programs"]', "href")) === "/study", "the Studium tab does not lead to „Mein Studium“");
  await step("Studium", () => page.click('.rail a[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-setup"));
  const fresh = await page.evaluate(() => ({
    title: document.querySelector(".crumb h1")?.textContent,
    heading: document.querySelector(".st-setup h1")?.textContent,
    tab: document.querySelector('.rail a[data-area="programs"]')?.getAttribute("aria-current"),
    all: document.querySelector('.st-setup a[href="/programs"]') !== null,
    create: document.querySelector("#st-create")?.disabled,
    legend: document.querySelector(".st-legend") !== null,
  }));
  check(fresh.title === "Mein Studium" && fresh.heading === "Richte dein Studium einmal ein" && fresh.tab === "page" && fresh.all && fresh.create === true && !fresh.legend, `the page without a program: ${JSON.stringify(fresh)}`);
  check((await stored(page, PLAN)) === null && (await stored(page, MINE)) === null, "looking at „Mein Studium“ stored something");
  // The program: a winter intake starts in the current winter, which has nothing before it.
  await step("pick a program", async () => {
    await page.click("#st-program");
    await page.fill("#st-program-search", "informatik b.sc. 2008");
    // The list follows what is typed in the next frame.
    await page.waitForFunction(() => document.querySelector("#st-program-list [role=option]")?.textContent.startsWith("Informatik"), null, { timeout: 5000 }).catch(() => {});
    await page.keyboard.press("Enter");
  }, () => document.querySelector("#st-start")?.value === "2026W" && !document.querySelector("#st-create")?.disabled);
  check(!(await page.$("#st-fill")), "a first semester is offered to be filled from the plan");
  check((await stored(page, MINE)) === null, "a pick in the setup was stored before „Studium anlegen“");
  await step("Studienbeginn WiSe 2025/26", () => page.selectOption("#st-start", "2025W"), () => document.querySelector("#st-fill")?.textContent.includes("1. und 2. FS nach Regelstudienplan füllen"));
  await step("fill from the plan", () => page.click("#st-fill"), () => document.querySelector("#st-fill")?.getAttribute("aria-checked") === "true");
  await step("Studium anlegen", () => page.click("#st-create"), () => document.querySelector(".st-over") && document.querySelector(".st-card-sem"));
  check((await stored(page, MINE)) === THIRD, `what „Studium anlegen“ stored: ${JSON.stringify(await stored(page, MINE))}`);
  const filled = await lines(page, "m\t");
  check(filled.length >= 7 && filled.every((line) => /^m\t(2025W|2026S)\t/.test(line)), `the first two semesters are not filled from the plan: ${JSON.stringify(filled)}`);
  check((await focused(page)) === "WiSe 2026/27" && (await rows(page)).length === 0, "the current semester does not begin empty");
  for (const request of requests) {
    if (/079-82-2008|start%09|2025W%0A|12104/.test(request.replace(/\/catalog\/module\/\d+/g, ""))) problems.push(`a request carries the study: ${request.slice(0, 160)}`);
  }
  await context.close();
}

// ---------- 2: Informatik B.Sc. in its third semester ----------
{
  const requests = [];
  const { page, step, seed, context } = await open({ viewport: { width: 1440, height: 900 } }, "/", { requests });
  await seed([[MINE, THIRD], [PLAN, FILLED]]);
  await step("Studium", () => page.click('.rail a[data-area="programs"]'), () => location.pathname === "/study" && document.querySelectorAll(".st-strip .st-tab").length === 7);
  const over = await page.evaluate(() => ({
    headline: document.querySelector(".st-headline")?.textContent,
    cards: [...document.querySelectorAll(".st-card .st-card-name")].map((name) => name.textContent),
    hints: [...document.querySelectorAll(".st-hints li")].map((hint) => hint.textContent),
    now: document.querySelector(".st-tab.is-now .st-now")?.textContent,
  }));
  check(over.headline === "0 von 180 LP bestanden" && over.cards.slice(0, 5).join() === "Informatik,Mathematik,Nebenfach,FÜS,Fachstudium" && over.cards.length === 6 && over.now === "jetzt", `the overview of the third semester: ${JSON.stringify(over)}`);
  check(over.hints.length === 1 && over.hints[0].startsWith("9 Einträge früherer Semester sind nicht abgehakt"), `the hints after filling: ${JSON.stringify(over.hints)}`);

  // The first winter: nothing ticked off is a Wiederholer; ticking off counts and keeps the place.
  await step("Abhaken", () => page.click(".st-hints .st-link"), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2025/26");
  const before = await rows(page);
  check(before.length === 5 && before.every((row) => !row.done && row.chips.some((chip) => chip.startsWith("nicht bestanden"))), `the first winter: ${JSON.stringify(before)}`);
  await step("tick off", () => page.click(".st-card-sem .st-row .st-tick >> nth=0"), () => document.querySelector(".st-headline")?.textContent === "8 von 180 LP bestanden");
  const after = await rows(page);
  check(after[0]?.name === before[0]?.name && after[0]?.done && after.length === before.length, `ticking off moved the row: ${JSON.stringify(after.map((row) => row.name))}`);
  check((await page.evaluate(() => document.activeElement?.classList.contains("st-tick"))), "ticking off lost the focus");
  check((await lines(page, "d\t")).join() === "d\t2025W\t12104", `what was stored for a tick: ${JSON.stringify(await lines(page, "d\t"))}`);

  // The current semester: „Module hinzufügen" offers the plan's third semester, its modules ticked.
  await step("to now", () => page.click(".st-strip .st-tab.is-now"), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
  await step("Module hinzufügen", () => page.click(".st-card-sem .st-add"), () => document.querySelector(".st-dialog[open] .st-choices") && document.querySelector("#st-plan-fs")?.value === "3");
  const offered = await page.evaluate(() => [...document.querySelectorAll(".st-dialog[open] .st-choice-row")].map((row) => ({ name: row.querySelector(".st-choice-name")?.textContent, on: row.querySelector(".st-tick")?.getAttribute("aria-checked") })));
  check(offered.some((row) => row.name === "Theoretische Informatik" && row.on === "true") && offered.some((row) => row.name === "Anwendungsfach" && row.on === "false"), `what „Module hinzufügen“ offers: ${JSON.stringify(offered)}`);
  const ticked = offered.filter((row) => row.on === "true").length;
  await step("Einfügen", () => page.click("#st-insert"), (n) => !document.querySelector(".st-dialog[open]") && document.querySelectorAll(".st-card-sem .st-row").length === n, ticked);
  const planned = await lines(page, "m\t2026W\t");
  const count = await page.evaluate(() => document.querySelector('.rail .nav[data-area="studyplan"] .nav-count')?.textContent);
  check(Number(count) === planned.length && planned.length === ticked && ticked >= 2, `the Stundenplan's tab counts ${count}, the current semester holds ${planned.length} of ${ticked} ticked`);
  check(planned.some((line) => line.startsWith("m\t2026W\t11787\t")), "Theoretische Informatik is not in the current semester");

  // A module's menu: moved to the next winter, and back; removed, and back.
  const menu = '.st-card-sem .st-row:has(.st-name:text-is("Theoretische Informatik")) .st-more';
  await step("the menu", () => page.click(menu), () => document.querySelector(".st-dialog[open] .st-move-chip"));
  const chips = await page.evaluate(() => [...document.querySelectorAll(".st-move-chip")].map((chip) => chip.textContent));
  check(chips.includes("SoSe 27") && chips.includes("WiSe 27/28angeboten"), `where the menu moves to: ${JSON.stringify(chips)}`);
  await step("verschieben", () => page.click('.st-move-chip:has-text("WiSe 27/28")'), () => document.querySelector(".st-undo")?.textContent.includes("ins WiSe 2027/28 verschoben") && document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2027/28");
  check((await lines(page, "m\t2027W\t11787\t")).length === 1 && (await lines(page, "m\t2026W\t11787\t")).length === 0, "the module did not move");
  await step("Rückgängig", () => page.click(".st-undo .mini"), () => !document.querySelector(".st-undo"));
  await page.waitForTimeout(300);
  check((await lines(page, "m\t2026W\t11787\t")).length === 1, "„Rückgängig“ did not bring the module back");
  await step("to now again", () => page.click(".st-strip .st-tab.is-now"), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
  await step("the menu again", () => page.click(menu), () => document.querySelector("#st-menu-remove"));
  await step("Entfernen", () => page.click("#st-menu-remove"), () => document.querySelector(".st-undo")?.textContent.includes("„Theoretische Informatik“ entfernt"));
  check((await lines(page, "m\t2026W\t11787\t")).length === 0, "„Entfernen“ left the module");
  await step("Rückgängig again", () => page.click(".st-undo .mini"), () => [...document.querySelectorAll(".st-card-sem .st-name")].some((name) => name.textContent === "Theoretische Informatik"));

  // An area: what it asks and what counts there.
  await step("an area", () => page.click(".st-card >> nth=0"), () => document.querySelector(".st-dialog[open] .st-area-dlg"));
  const area = await page.evaluate(() => ({ title: document.querySelector("#st-dialog-title")?.textContent, sub: document.querySelector(".st-dlg-head p")?.textContent, mine: document.querySelectorAll(".st-area-items li").length }));
  check(area.title === "Komplex Informatik" && area.sub?.startsWith("Verlangt 66") && area.mine >= 2, `the area's dialog: ${JSON.stringify(area)}`);
  await step("close it", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-dialog[open]"));

  // A module beside the page, in full, and back.
  await step("a module beside it", () => page.click('.st-card-sem a.st-name:text-is("Theoretische Informatik")'), () => location.search === "?open=11787" && document.querySelector("#preview .hero .mono")?.textContent.trim() === "11787");
  await step("Vollbild", () => page.click('#preview [data-action="fullscreen"]'), () => location.search === "?open=11787&full=1" && document.querySelector(".module-page"));
  check((await page.getAttribute('.rail a[data-area="programs"]', "aria-current")) === "page", "the module in full left the Studium tab");
  await step("Zurück", () => page.click('[data-action="back"]'), () => location.search === "?open=11787" && document.querySelector(".st-card-sem"));
  await step("close the module", () => page.click('#preview [data-action="close-detail"]'), () => location.search === "" && !document.querySelector("#preview"));

  // The Gesamtplan: a grey line of the plan, added with a click; the view kept.
  await step("Gesamtplan", () => page.click('.st-views button:has-text("Gesamtplan")'), () => document.querySelector(".st-grid") && document.querySelectorAll(".st-row-head").length === 6);
  const line = await page.evaluate(() => [...document.querySelectorAll(".st-entry.line")].map((entry) => entry.getAttribute("aria-label")).find((label) => label?.startsWith("Betriebssysteme I")));
  check(line?.includes("laut Regelstudienplan im 4. FS"), `the plan's line of Betriebssysteme I: ${line}`);
  await step("add a grey line", () => page.click('.st-entry.line[aria-label^="Betriebssysteme I"]'), () => document.querySelector(".st-undo")?.textContent.includes("hinzugefügt"));
  check((await lines(page, "m\t2027S\t")).length === 1, `the grey line did not plan the module: ${JSON.stringify(await lines(page, "m\t2027S\t"))}`);
  check((await stored(page, "betula.study.view")) === "grid", "the view is not remembered");

  // A reload keeps all of it.
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(() => document.querySelector(".st-grid") && document.querySelector(".st-headline")?.textContent === "8 von 180 LP bestanden", null, { timeout: 8000 }).catch(() => problems.push("a reload lost the view or what was ticked off"));
  await page.evaluate(() => localStorage.removeItem("betula.study.view"));
  for (const request of requests) {
    if (/12104|079-82-2008|11787%0A|d%092025W|start%09/.test(request.replace(/\/catalog\/module\/\d+|open=11787/g, ""))) problems.push(`a request carries the study: ${request.slice(0, 160)}`);
  }
  await context.close();
}

// ---------- 3: a phone ----------
{
  const { page, step, seed, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 }, "/");
  await seed([[MINE, THIRD], [PLAN, FILLED]]);
  await step("the bottom bar", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-card-sem"));
  const phone = await page.evaluate(() => {
    const card = document.querySelector(".st-card-sem").getBoundingClientRect();
    const over = document.querySelector(".st-over").getBoundingClientRect();
    return {
      sideways: document.documentElement.scrollWidth - innerWidth,
      sheet: getComputedStyle(document.querySelector("#sidebar")).position,
      card: Math.round(card.width),
      right: Math.round(card.left + card.width - (over.left + over.width)),
      strip: document.querySelector(".st-strip")?.getBoundingClientRect().height ?? 0,
      turns: Math.min(...[...document.querySelectorAll(".st-turn")].map((button) => button.getBoundingClientRect().height)),
      grid: document.querySelector(".st-views")?.getBoundingClientRect().height ?? 0,
    };
  });
  check(phone.sideways <= 0 && phone.sheet === "fixed" && phone.card >= 360 && phone.right === 0 && phone.strip === 0 && phone.turns >= 44 && phone.grid === 0, `the phone: ${JSON.stringify(phone)}`);
  await step("‹", () => page.tap(".st-card-sem .st-turn >> nth=0"), () => document.querySelector(".st-card-sem h2")?.textContent === "SoSe 2026");
  const ticks = await page.evaluate(() => Math.min(...[...document.querySelectorAll(".st-card-sem .st-tick")].map((tick) => tick.getBoundingClientRect().height)));
  check(ticks >= 44, `a box on a phone is ${ticks} px tall`);
  // A swipe to the left: the next semester.
  await step("swipe", () => page.evaluate(() => {
    const card = document.querySelector(".st-card-sem");
    const box = card.getBoundingClientRect();
    const fire = (type, x) => card.dispatchEvent(new PointerEvent(type, { bubbles: true, pointerType: "touch", isPrimary: true, clientX: x, clientY: box.top + 120 }));
    fire("pointerdown", box.left + box.width - 40);
    fire("pointerup", box.left + 40);
  }), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
  // To the end: one semester more.
  for (let i = 0; i < 10 && !(await page.$(".st-new")); i++) await page.tap(".st-card-sem .st-turn >> nth=1");
  check((await page.evaluate(() => document.querySelector(".st-new h2")?.textContent)) === "Neues Semester", "the last page does not add a semester");
  await step("anfügen", () => page.tap("#st-append"), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2028/29");
  check((await stored(page, MINE))?.includes("until\t2028W"), "the semester added was not stored");
  await step("Bereiche", () => page.tap(".st-kicker .st-link"), () => document.querySelector(".st-dialog[open] .st-area-list"));
  await page.waitForTimeout(500);
  const sheet = await page.evaluate(() => { const box = document.querySelector(".st-dialog[open]").getBoundingClientRect(); return { bottom: Math.round(innerHeight - box.bottom), width: Math.round(box.width) }; });
  check(sheet.bottom === 0 && sheet.width === 390, `the areas are no sheet from below: ${JSON.stringify(sheet)}`);
  await step("close the areas", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-dialog[open]"));
  // The sidebar, a sheet: „Mein Studiengang" and its change.
  await step("Anpassen", () => page.tap(".st-sheet .sheet-toggle"), () => document.documentElement.classList.contains("sheet-open") && document.querySelector("#sidebar .st-mine-card .st-link"));
  await context.close();
}

// ---------- 4: the Stundenplan takes the Wiederholer from „Mein Studium" ----------
{
  const { page, step, seed, context } = await open({ viewport: { width: 1440, height: 900 } }, "/");
  await seed([[MINE, THIRD], [PLAN, FILLED]]);
  await step("Stundenplan", () => page.click('.rail a[data-area="studyplan"]'), () => location.pathname === "/studyplan" && document.querySelector(".sp-import"));
  await step("source „Mein Studium“", () => page.click('.sp-import .seg button:has-text("Mein Studium")'), () => document.querySelector('.sp-import .seg button[aria-checked="true"]')?.textContent === "Mein Studium" && document.querySelector("#sp-import-go:not([disabled])"));
  await step("Übernehmen", () => page.click("#sp-import-go"), () => document.querySelector(".sp-import .note-action")?.textContent.startsWith("Übernommen:"));
  const now = await lines(page, "m\t2026W\t");
  check(now.length === 7 && now.some((line) => line.startsWith("m\t2026W\t12104\t")) && (await lines(page, "p\t")).length === 4, `the Wiederholer are not in the timetable: ${JSON.stringify(await stored(page, PLAN))}`);
  await context.close();
}

// ---------- 5: without the app ----------
{
  const html = await fetch(base + "/study").then((r) => r.text());
  check(html.includes('content="noindex') && html.includes("<title>Mein Studium · Betula</title>"), "the server's page of „Mein Studium“ is not one page for no index");
  check(/<a [^>]*data-area="programs" href="\/programs"/.test(html), "the server's Studium tab does not lead to the overview");
  const old = await fetch(base + "/programs/bachelor-informatik-2008/my-plan").then((r) => r.text());
  check(old.includes('href="/study"') && old.includes('content="noindex'), "„Mein Plan“ does not lead on to „Mein Studium“");
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
