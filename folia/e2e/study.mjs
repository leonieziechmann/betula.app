// Checks „Mein Studium" (folia/crates/planner/src/study, folia/crates/plans/src/study.rs), the first
// page of the Studium tab (owner, mockup of 2026-10-04).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node study.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// 1 a fresh browser: the Studium tab leads to it; it asks to set the study up once and stores
//   nothing by being looked at; the program, a Studienbeginn of last winter and „1. und 2. FS nach
//   Regelstudienplan füllen" set it up: the overview, the first two semesters filled.
// 2 Informatik B.Sc. in its third semester, the first two filled and nothing marked as passed: the
//   overview says nothing of it, only the credits; the sidebar has the program as one card, which
//   opens „Studiengang wechseln", no way to the Stundenplan, the legend in two parts; the box in
//   the head selects the first semester's rows; the boxes show while rows are selected and go with
//   „×"; a row's menu marks it as passed (counted, the row in its place, the focus back on its ⋯);
//   a drag over the boxes selects three, the bar marks them and „Rückgängig" takes it back;
//   „Module hinzufügen" puts the plan's third semester in; the Stundenplan's tab counts it; what
//   fits the third, a winter, and the fourth, a summer: nothing of a Fachsemester after it, nothing
//   of the other half of the year; a module moved by its menu's „Verschieben nach ›" and taken back,
//   removed and taken back; two rows selected and dragged onto a semester of the strip, and back;
//   a right click opens a row's menu, the semester's ⋯ selects all of it, Escape lets go; an
//   area's dialog; a module beside the page by its name and by its row, in full and back; the
//   Gesamtplan, a module's menu, a grey line of the plan added with a click, the view kept; a
//   reload keeps all of it; nothing of it in a request.
// 3 a phone: the overview first (owner, 2026-10-05), the bar with its legend, the program's card
//   and its ways, the box „Studium planen" with a column per semester, the way to all programs, and
//   no sidebar; the box leads to the semesters (`?plan=1`), its column to that one, and drawn to the
//   left as well; there one semester as a card as wide as the page, which ‹ › turn by a glide and a
//   finger turns as it goes (the next semester beside it), a short pull leaves it, the last page
//   adds a semester; a long press selects a row, the bar over the tab bar; a row's menu as a sheet
//   without „Modul ansehen", „Verschieben nach" in its place; „Übersicht" and Back lead back; the
//   areas as a sheet, the program's card opens „Studiengang wechseln"; nothing scrolls sideways;
//   boxes and ⋯ as tall as a finger.
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
// The step under way, for what the page says while it is.
let during = "";
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
  page.on("console", (m) => { if (m.type() === "error") problems.push(`console (${during}): ` + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push(`pageerror (${during}): ` + String(e).slice(0, 300)));
  if (requests) page.on("request", (r) => requests.push(r.url() + " " + (r.postData() || "")));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  const step = async (name, action, until, arg) => {
    const started = Date.now();
    during = name;
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
// The rows of the semester in focus: their names, whether passed, whether selected, their marks.
const rows = (page) => page.evaluate(() => [...document.querySelectorAll(".st-card-sem .st-row")].map((row) => ({
  name: row.querySelector(".st-name")?.textContent,
  done: row.classList.contains("is-passed") && !!row.querySelector(".st-passed"),
  on: row.querySelector(".st-sel")?.getAttribute("aria-checked") === "true",
  chips: [...row.querySelectorAll(".st-chip")].map((chip) => chip.textContent),
})));
// The middle of an element on the screen.
const middle = (page, selector) => page.$eval(selector, (el) => { const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
const selected = (page) => page.evaluate(() => document.querySelector(".st-selbar-count")?.textContent ?? "");
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
    words: document.querySelector(".st-over")?.textContent ?? "",
    now: document.querySelector(".st-tab.is-now .st-now")?.textContent,
  }));
  check(over.headline === "0 von 180 LP bestanden" && over.cards.slice(0, 5).join() === "Informatik,Mathematik,Nebenfach,FÜS,Fachstudium" && over.cards.length === 6 && over.now === "jetzt", `the overview of the third semester: ${JSON.stringify(over)}`);
  check(!/nicht (als )?bestanden|Wiederhol|Einplanen/.test(over.words), `the overview says more than the credits: ${over.words}`);

  // The sidebar: the program as one card, no way to the Stundenplan (the tab bar has it), the
  // parts of the bar apart from the marks of a module.
  const side = await page.evaluate(() => ({
    labels: [...document.querySelectorAll("#sidebar .flabel")].map((label) => label.textContent),
    mine: [...document.querySelectorAll("#sidebar #st-mine .st-mine-name, #sidebar #st-mine .st-mine-line")].map((line) => line.textContent),
    timetable: document.querySelector('#sidebar a[href="/studyplan"]') !== null,
    legend: [...document.querySelectorAll("#sidebar .st-legend ul")].map((list) => list.textContent),
  }));
  check(side.labels.join("|") === "Fortschrittsbalken|Zeichen an Modulen" && side.mine.join("|") === "Informatik B.Sc.|PO 2008|3. Fachsemester · seit WiSe 25/26" && !side.timetable, `the sidebar: ${JSON.stringify(side)}`);
  check(side.legend.join("|") === "BestandenGeplantOffenÜber Bedarf|BestandenWiederholungNicht angeboten", `the legend: ${JSON.stringify(side.legend)}`);
  await step("the program's card", () => page.click("#st-mine"), () => document.querySelector(".st-dialog[open] #st-switch-program"));
  await step("not changed", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-dialog[open]"));

  // The first winter: its rows not passed (Wiederholer), all selected by the box in the head.
  await step("the first winter", () => page.click('.st-strip .st-tab:has-text("WiSe 25/26")'), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2025/26");
  await step("select all", () => page.click(".st-card-sem .st-sel.all"), () => document.querySelector(".st-selbar"));
  const before = await rows(page);
  check(before.length === 5 && before.every((row) => !row.done && row.on && row.chips.some((chip) => chip.startsWith("nicht bestanden"))), `the first winter: ${JSON.stringify(before)}`);
  check((await selected(page)).startsWith("5 ausgewählt"), `the bar: ${await selected(page)}`);
  await step("let go", () => page.click(".st-selbar .st-act-close"), () => !document.querySelector(".st-selbar") && !document.querySelector(".st-row.is-selected"));
  await page.mouse.move(5, 5);
  await page.waitForTimeout(400);
  const hidden = await page.$eval(".st-card-sem .st-row .st-sel .st-box", (box) => getComputedStyle(box).opacity);
  check(hidden === "0", `a row's box shows with nothing selected: opacity ${hidden}`);

  // A row's menu: „Als bestanden markieren" counts, keeps the row in its place, gives the focus back.
  await step("the row's menu", () => page.click(".st-card-sem .st-row >> nth=0 >> .st-more"), () => document.querySelector(".st-pop.root #st-menu-passed"));
  const entries = await page.evaluate(() => [...document.querySelectorAll(".st-pop.root [role^=menuitem]")].map((item) => item.textContent.trim()));
  // Not the module: a click on the row opens it (owner, 2026-10-05).
  check(entries.join("|") === "Als bestanden markieren|Verschieben nach|Entfernen", `the row's menu: ${JSON.stringify(entries)}`);
  await step("Als bestanden markieren", () => page.click("#st-menu-passed"), () => !document.querySelector(".st-pop") && document.querySelector(".st-headline")?.textContent === "8 von 180 LP bestanden");
  const after = await rows(page);
  check(after[0]?.name === before[0]?.name && after[0]?.done && after.length === before.length, `marking moved the row: ${JSON.stringify(after.map((row) => row.name))}`);
  check((await page.evaluate(() => document.activeElement?.classList.contains("st-more"))), "the menu did not give the focus back to its ⋯");
  check((await lines(page, "d\t")).join() === "d\t2025W\t12104", `what was stored for passed: ${JSON.stringify(await lines(page, "d\t"))}`);
  check((await page.evaluate(() => document.querySelector(".st-undo")?.textContent ?? "")).includes("als bestanden markiert"), "no note to take the mark back");

  // A drag over the boxes selects the rows it passes; the bar marks them; „Rückgängig" takes it back.
  const from = await middle(page, ".st-card-sem .st-row >> nth=1 >> .st-sel");
  const to = await middle(page, ".st-card-sem .st-row >> nth=3 >> .st-sel");
  await step("draw over three boxes", async () => {
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move(to.x, (from.y + to.y) / 2, { steps: 4 });
    await page.mouse.move(to.x, to.y, { steps: 4 });
    await page.mouse.up();
  }, () => document.querySelectorAll(".st-card-sem .st-row.is-selected").length === 3 && document.querySelector(".st-selbar-count")?.textContent.startsWith("3 ausgewählt"));
  check((await rows(page)).map((row) => row.on).join() === "false,true,true,true,false", `the drawn selection: ${JSON.stringify((await rows(page)).map((row) => row.on))}`);
  await step("the bar marks them", () => page.click("#st-sel-passed"), () => !document.querySelector(".st-selbar") && document.querySelector(".st-undo")?.textContent.includes("3 Einträge als bestanden markiert") && document.querySelector(".st-headline")?.textContent !== "8 von 180 LP bestanden");
  check((await lines(page, "d\t")).length === 4, `the bar's mark: ${JSON.stringify(await lines(page, "d\t"))}`);
  await step("Rückgängig the mark", () => page.click(".st-undo .mini"), () => document.querySelector(".st-headline")?.textContent === "8 von 180 LP bestanden");

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

  // „Passt in dieses Semester": of its Fachsemester and the ones before, what the semester offers;
  // the Wiederholer of the first winter in a winter, those of the summer in a summer.
  const fits = () => page.evaluate(() => ({
    names: [...document.querySelectorAll(".st-fits .st-fit-name")].map((name) => name.textContent),
    lines: [...document.querySelectorAll(".st-fits .st-fit .st-sub")].map((line) => line.textContent),
    warn: document.querySelectorAll(".st-fits .st-chip.warn").length,
  }));
  const winter = await fits();
  check(winter.names.includes("Programmierpraktikum") && !winter.names.includes("Digitaltechnik") && winter.warn === 0 && !winter.lines.some((line) => /im ([4-9]|1\d)\. FS/.test(line)), `what fits the third semester, a winter: ${JSON.stringify(winter)}`);
  await step("SoSe 27", () => page.click('.st-strip .st-tab:has-text("SoSe 27")'), () => document.querySelector(".st-card-sem h2")?.textContent === "SoSe 2027");
  const summer = await fits();
  check(summer.names.includes("Digitaltechnik") && !summer.names.includes("Programmierpraktikum") && summer.warn === 0 && !summer.lines.some((line) => /im ([5-9]|1\d)\. FS/.test(line)), `what fits the fourth semester, a summer: ${JSON.stringify(summer)}`);
  await step("back to now", () => page.click(".st-strip .st-tab.is-now"), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");

  // A module's menu: „Verschieben nach ›" to the next winter, and back; removed, and back.
  const menu = '.st-card-sem .st-row:has(.st-name:text-is("Theoretische Informatik")) .st-more';
  await step("the menu", () => page.click(menu), () => document.querySelector(".st-pop.root #st-menu-move"));
  await step("Verschieben nach", () => page.click("#st-menu-move"), () => document.querySelector(".st-pop.sub [role=menuitem]"));
  const targets = await page.evaluate(() => [...document.querySelectorAll(".st-pop.sub [role=menuitem]")].map((item) => item.textContent.trim()));
  check(targets.some((target) => target.startsWith("SoSe 27 · 4. FS")) && targets.some((target) => target.startsWith("WiSe 27/28 · 5. FS") && target.endsWith("angeboten")), `where the menu moves to: ${JSON.stringify(targets)}`);
  await step("verschieben", () => page.click('.st-pop.sub [role=menuitem]:has-text("WiSe 27/28")'), () => document.querySelector(".st-undo")?.textContent.includes("ins WiSe 2027/28 verschoben") && !document.querySelector(".st-pop"));
  check((await focused(page)) === "WiSe 2026/27", "moving took the focus to the other semester");
  check((await lines(page, "m\t2027W\t11787\t")).length === 1 && (await lines(page, "m\t2026W\t11787\t")).length === 0, "the module did not move");
  await step("Rückgängig", () => page.click(".st-undo .mini"), () => !document.querySelector(".st-undo"));
  await page.waitForTimeout(300);
  check((await lines(page, "m\t2026W\t11787\t")).length === 1, "„Rückgängig“ did not bring the module back");
  // The keys: Enter opens the menu, the arrows go through it and into its submenu and back,
  // Escape closes it and gives the focus back to its ⋯.
  // A click on the row opens the module: the menu does not offer it again.
  await step("the menu by the keys", async () => {
    await page.focus(menu);
    await page.keyboard.press("Enter");
  }, () => document.activeElement?.id === "st-menu-passed" && !document.querySelector("#st-menu-view"));
  await step("down to Verschieben nach", () => page.keyboard.press("ArrowDown"), () => document.activeElement?.id === "st-menu-move");
  await step("into the submenu", () => page.keyboard.press("ArrowRight"), () => document.activeElement?.closest(".st-pop.sub"));
  await step("back out of it", () => page.keyboard.press("ArrowLeft"), () => !document.querySelector(".st-pop.sub") && document.activeElement?.id === "st-menu-move");
  await step("Escape gives the focus back", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-pop") && document.activeElement?.classList.contains("st-more"));
  await step("the menu again", () => page.click(menu), () => document.querySelector("#st-menu-remove"));
  await step("Entfernen", () => page.click("#st-menu-remove"), () => document.querySelector(".st-undo")?.textContent.includes("„Theoretische Informatik“ entfernt"));
  check((await lines(page, "m\t2026W\t11787\t")).length === 0, "„Entfernen“ left the module");
  await step("Rückgängig again", () => page.click(".st-undo .mini"), () => [...document.querySelectorAll(".st-card-sem .st-name")].some((name) => name.textContent === "Theoretische Informatik"));

  // Two rows selected by their boxes, dragged onto a semester of the strip: both move, and back.
  await step("select two", async () => {
    await page.click(".st-card-sem .st-row >> nth=0 >> .st-sel");
    await page.click(".st-card-sem .st-row >> nth=1 >> .st-sel");
  }, () => document.querySelectorAll(".st-card-sem .st-row.is-selected").length === 2);
  const two = (await rows(page)).filter((row) => row.on).map((row) => row.name);
  await step("drag them onto SoSe 27", () => page.dragAndDrop(".st-card-sem .st-row >> nth=0 >> .st-area.st-pc", '.st-strip .st-tab:has-text("SoSe 27")'), () => document.querySelector(".st-undo")?.textContent.includes("2 Einträge ins SoSe 2027 verschoben"));
  const left = (await rows(page)).map((row) => row.name);
  check(two.length === 2 && two.every((name) => !left.includes(name)) && (await lines(page, "m\t2027S\t")).length === 2, `the two dragged: ${JSON.stringify({ two, left })}`);
  await step("Rückgängig the drag", () => page.click(".st-undo .mini"), (n) => document.querySelectorAll(".st-card-sem .st-row").length === n, left.length + 2);

  // A right click opens a row's menu where the pointer is; the semester's ⋯ selects all of it;
  // Escape lets go.
  await step("a right click", () => page.click(".st-card-sem .st-row >> nth=1 >> .st-area.st-pc", { button: "right" }), () => document.querySelector(".st-pop.root #st-menu-passed"));
  await step("Escape closes it", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-pop"));
  await step("the semester's menu", () => page.click(".st-card-sem .st-sem-more"), () => document.querySelector(".st-pop.root #st-menu-select"));
  const semester = await page.evaluate(() => [...document.querySelectorAll(".st-pop.root [role^=menuitem]")].map((item) => item.id));
  check(semester.join() === "st-menu-timetable,st-menu-select,st-menu-leave", `the semester's menu: ${JSON.stringify(semester)}`);
  await step("Alle auswählen", () => page.click("#st-menu-select"), (n) => document.querySelectorAll(".st-card-sem .st-row.is-selected").length === n, ticked);
  await step("Escape lets go", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-selbar"));

  // An area: what it asks and what counts there.
  await step("an area", () => page.click(".st-card >> nth=0"), () => document.querySelector(".st-dialog[open] .st-area-dlg"));
  const area = await page.evaluate(() => ({ title: document.querySelector("#st-dialog-title")?.textContent, sub: document.querySelector(".st-dlg-head p")?.textContent, mine: document.querySelectorAll(".st-area-items li").length }));
  check(area.title === "Komplex Informatik" && area.sub?.startsWith("Verlangt 66") && area.mine >= 2, `the area's dialog: ${JSON.stringify(area)}`);
  await step("close it", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-dialog[open]"));

  // A module beside the page, in full, and back.
  await step("a module beside it", () => page.click('.st-card-sem a.st-name:text-is("Theoretische Informatik")'), () => location.search === "?open=11787" && document.querySelector("#preview .hero .mono")?.textContent.trim() === "11787");
  // A click on a row beside its name opens its module too.
  const other = await page.evaluate(() => [...document.querySelectorAll(".st-card-sem .st-row")].findIndex((row) => row.querySelector("a.st-name") && row.querySelector("a.st-name").textContent !== "Theoretische Informatik"));
  await step("another by its row", () => page.click(`.st-card-sem .st-row >> nth=${other} >> .st-area.st-pc`), () => /^\?open=\d+$/.test(location.search) && location.search !== "?open=11787");
  await step("Theoretische Informatik again", () => page.click('.st-card-sem a.st-name:text-is("Theoretische Informatik")'), () => location.search === "?open=11787" && document.querySelector("#preview .hero .mono")?.textContent.trim() === "11787");
  await step("Vollbild", () => page.click('#preview [data-action="fullscreen"]'), () => location.search === "?open=11787&full=1" && document.querySelector(".module-page"));
  check((await page.getAttribute('.rail a[data-area="programs"]', "aria-current")) === "page", "the module in full left the Studium tab");
  await step("Zurück", () => page.click('[data-action="back"]'), () => location.search === "?open=11787" && document.querySelector(".st-card-sem"));
  await step("close the module", () => page.click('#preview [data-action="close-detail"]'), () => location.search === "" && !document.querySelector("#preview"));

  // The Gesamtplan: a grey line of the plan, added with a click; the view kept.
  await step("Gesamtplan", () => page.click('.st-views button:has-text("Gesamtplan")'), () => document.querySelector(".st-grid") && document.querySelectorAll(".st-row-head").length === 6);
  const line = await page.evaluate(() => [...document.querySelectorAll(".st-entry.line")].map((entry) => entry.getAttribute("aria-label")).find((label) => label?.startsWith("Betriebssysteme I")));
  check(line?.includes("laut Regelstudienplan im 4. FS"), `the plan's line of Betriebssysteme I: ${line}`);
  await step("a module's menu in the Gesamtplan", () => page.click(".st-entry:not(.line) >> nth=0"), () => document.querySelector(".st-pop.root #st-menu-remove"));
  await step("closed by a click beside it", () => page.click(".st-pop-shade", { position: { x: 5, y: 5 } }), () => !document.querySelector(".st-pop"));
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
  await step("the bottom bar", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-planbox"));
  const landing = await page.evaluate(() => ({
    sideways: document.documentElement.scrollWidth - innerWidth,
    parts: [...document.querySelectorAll(".st-phone-page > *")].map((part) => [...part.classList].find((name) => name !== "panel")),
    legend: [...document.querySelectorAll(".st-progress .st-legend-n li")].map((li) => li.textContent.replace(/[\d,]+/g, "").trim()),
    mine: [...document.querySelectorAll(".st-phone-page #st-mine > span:not(.visually-hidden)")].map((part) => part.textContent.replace(/\s+/g, " ").trim()).join(" | "),
    ways: [...document.querySelectorAll(".st-mine-panel .st-ways a")].map((a) => a.textContent.trim()),
    columns: [...document.querySelectorAll(".st-chart .st-col")].map((col) => col.textContent.trim() + (col.classList.contains("is-now") ? "*" : "")).join(" "),
    line: document.querySelector(".st-planbox-line")?.textContent.replace(/\u00a0/g, " "),
    rest: [...document.querySelectorAll(".st-rest a")].map((a) => a.textContent.trim()),
    sidebar: document.querySelector("#sidebar #st-mine") !== null || document.querySelector(".sheet-toggle") !== null,
    cards: document.querySelectorAll(".st-card-sem").length,
  }));
  check(landing.sideways <= 0 && landing.parts.join() === "st-over,st-mine-panel,st-planbox-wrap,st-rest" && landing.legend.join() === "bestanden,geplant,offen"
    && landing.mine === "Informatik B.Sc. | PO 2008 | 3. Fachsemester · seit WiSe 25/26" && landing.ways.join() === "Regelstudienplan,Wahlpflicht & Bereiche"
    && landing.columns === "1 2 3* 4 5 6" && landing.line === "Jetzt WiSe 2026/27 · 0 von 30 LP" && landing.rest.join() === "Alle Studiengänge ansehen" && !landing.sidebar && landing.cards === 0, `the phone's overview: ${JSON.stringify(landing)}`);
  await step("Studium planen", () => page.tap(".st-planbox-head"), () => location.search === "?plan=1" && document.querySelector(".st-pager .st-card-sem h2")?.textContent === "WiSe 2026/27" && document.querySelector(".st-plan-page.st-enter-right"));
  const phone = await page.evaluate(() => ({
    sideways: document.documentElement.scrollWidth - innerWidth,
    card: Math.round(document.querySelector(".st-card-sem").getBoundingClientRect().width),
    cards: document.querySelectorAll(".st-card-sem").length,
    strip: document.querySelector(".st-strip")?.getBoundingClientRect().height ?? 0,
    turns: Math.min(...[...document.querySelectorAll(".st-turn")].map((button) => button.getBoundingClientRect().height)),
    grid: document.querySelector(".st-views")?.getBoundingClientRect().height ?? 0,
    back: document.querySelector(".st-back")?.getAttribute("href"),
    marks: document.querySelectorAll(".st-plan-legend .st-legend-marks li").length,
  }));
  check(phone.sideways <= 0 && phone.card >= 360 && phone.cards === 1 && phone.strip === 0 && phone.turns >= 44 && phone.grid === 0 && phone.back === "/study" && phone.marks === 3, `the phone's semesters: ${JSON.stringify(phone)}`);
  // ‹ plays the glide: the semester before stands beside the card while it goes. The cards are
  // still again once one card is left, in its place.
  const still = (name) => new Function(`return !document.querySelector(".st-pager[data-phase]") && document.querySelectorAll(".st-card-sem").length === 1 && document.querySelector(".st-card-sem h2")?.textContent === ${JSON.stringify(name)};`);
  let gliding = false;
  await step("‹", async () => {
    await page.tap(".st-card-sem .st-turn >> nth=0");
    gliding = await page.waitForFunction(() => document.querySelector(".st-pager[data-phase=glide]") && document.querySelectorAll(".st-card-sem").length > 1, null, { timeout: 3000 }).then(() => true, () => false);
  }, still("SoSe 2026"));
  check(gliding, "‹ turns the card without a glide");
  const mores = await page.evaluate(() => Math.min(...[...document.querySelectorAll(".st-card-sem .st-more")].map((more) => more.getBoundingClientRect().height)));
  check(mores >= 44, `a row's ⋯ on a phone is ${mores} px tall`);
  // A long press selects a row: the boxes come in, the bar over the tab bar.
  await step("a long press", async () => {
    const at = await middle(page, ".st-card-sem .st-row >> nth=0 >> .st-what");
    await page.evaluate(({ x, y }) => {
      const body = document.querySelector(".st-card-sem .st-row .st-body");
      body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, pointerType: "touch", isPrimary: true, clientX: x, clientY: y }));
    }, at);
    await page.waitForTimeout(650);
    await page.evaluate(({ x, y }) => {
      const body = document.querySelector(".st-card-sem .st-row .st-body");
      body.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerType: "touch", isPrimary: true, clientX: x, clientY: y }));
    }, at);
  }, () => document.querySelector(".st-selbar") && document.querySelectorAll(".st-card-sem .st-row.is-selected").length === 1);
  await page.waitForTimeout(400);
  const bar = await page.evaluate(() => {
    const box = document.querySelector(".st-selbar").getBoundingClientRect();
    const nav = document.querySelector(".bottomnav").getBoundingClientRect();
    const sel = document.querySelector(".st-card-sem .st-row .st-sel").getBoundingClientRect();
    const top = document.elementFromPoint(nav.left + nav.width / 2, nav.top + nav.height / 2);
    return { fixed: getComputedStyle(document.querySelector(".st-selbar")).position, off: [box.left - nav.left, box.right - nav.right, box.top - nav.top, box.bottom - nav.bottom].map(Math.round).join(), covers: Boolean(top?.closest(".st-selbar")), sel: Math.round(Math.min(sel.width, sel.height)) };
  });
  check(bar.fixed === "fixed" && bar.off === "0,0,0,0" && bar.covers && bar.sel >= 40, `the phone's bar is not over the tab bar: ${JSON.stringify(bar)}`);
  await step("a tap selects another", () => page.tap(".st-card-sem .st-row >> nth=1 >> .st-what"), () => document.querySelectorAll(".st-card-sem .st-row.is-selected").length === 2);
  await step("let go on the phone", () => page.tap(".st-selbar .st-act-close"), () => !document.querySelector(".st-selbar"));
  // A row's menu: a sheet, without the module (a tap on the row opens it); „Verschieben nach"
  // takes its place, and gives it back.
  await step("a row's menu as a sheet", () => page.tap(".st-card-sem .st-row >> nth=0 >> .st-more"), () => document.querySelector(".st-pop.root #st-menu-move"));
  await page.waitForTimeout(400);
  const menuSheet = await page.evaluate(() => { const box = document.querySelector(".st-pop.root").getBoundingClientRect(); return { bottom: Math.round(innerHeight - box.bottom), width: Math.round(box.width), entries: [...document.querySelectorAll(".st-pop.root [role^=menuitem]")].map((item) => item.id).join() }; });
  check(menuSheet.bottom === 0 && menuSheet.width === 390 && menuSheet.entries === "st-menu-passed,st-menu-move,st-menu-remove", `a row's menu on a phone: ${JSON.stringify(menuSheet)}`);
  await step("Verschieben nach on a phone", () => page.tap("#st-menu-move"), () => document.querySelector(".st-pop.sub .st-pop-back") && getComputedStyle(document.querySelector(".st-pop.root")).display === "none");
  await step("back to the menu", () => page.tap(".st-pop.sub .st-pop-back"), () => !document.querySelector(".st-pop.sub") && getComputedStyle(document.querySelector(".st-pop.root")).display !== "none");
  await step("close the menu", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-pop"));
  // A finger drawn sideways over `selector` by `by` px (moves a frame apart), then lifted: what the
  // cards did the frame before it went.
  const swipe = (selector, by) => page.evaluate(({ selector, by }) => new Promise((done) => {
    const element = document.querySelector(selector);
    const box = element.getBoundingClientRect();
    const y = box.top + Math.min(120, box.height / 2);
    const from = by < 0 ? box.right - 40 : box.left + 40;
    const fire = (type, x) => element.dispatchEvent(new PointerEvent(type, { bubbles: true, cancelable: true, pointerId: 3, pointerType: "touch", isPrimary: true, buttons: type === "pointerup" ? 0 : 1, clientX: x, clientY: y }));
    fire("pointerdown", from);
    const steps = 8;
    let i = 0;
    let seen = null;
    const next = () => {
      i++;
      fire("pointermove", from + (by * i) / steps);
      if (i === steps) seen = { phase: element.dataset.phase, cards: document.querySelectorAll(".st-card-sem").length, pull: element.style.getPropertyValue("--pull") };
      if (i < steps) requestAnimationFrame(next);
      else { fire("pointerup", from + by); done(seen); }
    };
    requestAnimationFrame(next);
  }), { selector, by });
  // To the left: the cards follow the finger, the next semester beside them, and it comes.
  let held = null;
  await step("swipe", async () => { held = await swipe(".st-pager", -220); }, still("WiSe 2026/27"));
  check(held?.phase === "drag" && held.cards > 1 && parseFloat(held.pull) < -150, `the cards do not follow the finger: ${JSON.stringify(held)}`);
  // A short pull: back where it was.
  await step("a short swipe", () => swipe(".st-pager", -40), still("WiSe 2026/27"));
  // To the end: one semester more.
  for (let i = 0; i < 10 && !(await page.$(".st-slot[data-slot='0'] .st-new")); i++) {
    await page.tap(".st-slot[data-slot='0'] .st-turn >> nth=1");
    await page.waitForFunction(() => !document.querySelector(".st-pager[data-phase]") && document.querySelectorAll(".st-slot").length === 1, null, { timeout: 3000 }).catch(() => {});
  }
  check((await page.evaluate(() => document.querySelector(".st-new h2")?.textContent)) === "Neues Semester", "the last page does not add a semester");
  await step("anfügen", () => page.tap("#st-append"), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2028/29");
  check((await stored(page, MINE))?.includes("until\t2028W"), "the semester added was not stored");
  // „Übersicht": back through the history (no entry more), the overview from the left.
  const entries = await page.evaluate(() => history.length);
  await step("Übersicht", () => page.tap(".st-back"), () => location.search === "" && document.querySelector(".st-planbox") && document.querySelector(".st-phone-page.st-enter-left"));
  check((await page.evaluate(() => history.length)) === entries, "„Übersicht“ added a step to the history");
  await step("Bereiche", () => page.tap(".st-progress-top .st-link"), () => document.querySelector(".st-dialog[open] .st-area-list"));
  await page.waitForTimeout(500);
  const sheet = await page.evaluate(() => { const box = document.querySelector(".st-dialog[open]").getBoundingClientRect(); return { bottom: Math.round(innerHeight - box.bottom), width: Math.round(box.width) }; });
  check(sheet.bottom === 0 && sheet.width === 390, `the areas are no sheet from below: ${JSON.stringify(sheet)}`);
  await step("close the areas", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-dialog[open]"));
  await step("the program's card", () => page.tap("#st-mine"), () => document.querySelector(".st-dialog[open] #st-switch-program"));
  await step("close it", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-dialog[open]"));
  // A column leads to its semester; Back to the overview.
  await step("a column", () => page.tap(".st-chart .st-col >> nth=1"), () => location.search === "?plan=1" && document.querySelector(".st-card-sem h2")?.textContent === "SoSe 2026");
  await step("Back", () => page.goBack(), () => location.search === "" && document.querySelector(".st-planbox"));
  // The box drawn to the left leads to the current semester; a short pull leaves it.
  await step("the box drawn a little", () => swipe(".st-planbox-wrap", -40), () => location.search === "" && !document.querySelector(".st-planbox-wrap[data-phase]"));
  await step("the box drawn to the left", () => swipe(".st-planbox-wrap", -200), () => location.search === "?plan=1" && document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
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
