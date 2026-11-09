// Checks „Mein Studium" (folia/crates/planner/src/study, folia/crates/plans/src/study.rs), the first
// page of the Studium tab (owner, mockup of 2026-10-04).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node study.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// 1 a fresh browser: the Studium tab leads to it; it asks to set the study up once and stores
//   nothing by being looked at; the program, its third Fachsemester (+ twice; nothing of the card
//   moves meanwhile) and „1. und 2. FS nach
//   Regelstudienplan füllen" set it up: the overview, the first two semesters filled.
// 2 Informatik B.Sc. in its third semester, the first two filled and nothing marked as passed: the
//   overview says nothing of it, only the credits; the sidebar has the program as one card, which
//   opens „Studiengang wechseln", no way to the Stundenplan, the legend in two parts (the
//   statuses of a module the second); the semester at the top of its box, no strip of the
//   semesters and no switch of the views over it (owner, 2026-10-19), its name the list of every
//   semester with what each holds; its rows the catalog's, each with its status where the catalog
//   has the bookmark, no box to tick, the name no link that underlines; the semester's ⋯ selects
//   the first semester's rows, highlighted and hatched, and „×" lets go; a row's menu at its
//   status marks it as passed (counted, the row in its place, the focus back on its status); a
//   press drawn over three rows selects them, the bar marks them and „Rückgängig" takes it back;
//   „Module hinzufügen" opens the semester's card (the rows on the left, „Laut Plan im 3. FS" and
//   the ways on the right, the sidebar and the overview where they were), whose „Plan übernehmen"
//   puts the plan's third semester in and × ends it; the Stundenplan's tab counts it; the
//   semester's box a list as the catalog's, nothing beside it (owner, 2026-10-20), its head and the
//   heads of its columns at the top when scrolled down, the box as tall as the area shows; a module
//   moved by its menu's „Verschieben nach ›" and taken back,
//   removed and taken back; two rows selected by Ctrl and a click, moved by the bar, and back; a
//   right click opens a row's menu, the semester's ⋯ selects all of it, Escape lets go; an area's
//   dialog; a module beside the page by its name and by its row, in full and back; the
//   Gesamtplan as a roadmap (the band of the study years, Informatik's first semester by its
//   abbreviations, the pieces as wide as their credits, the Bachelor-Arbeit's bar not before its
//   semester, the line of today and its flag, a click on Mathematik's name unfolding it into its
//   lanes, no dialog, kept, growing in with the focus on its head, the one button unfolding all and
//   then folding all, shrinking them away, „Bisher zuklappen" and back), each
//   cell as tall as what it lists, a module's menu,
//   the plan's open Betriebssysteme I dashed and planned
//   with a click, the view kept, in a narrower window the plan scrolled sideways to the semester
//   before the current one with the areas' names at the left, the plan's head at the top when
//   scrolled down and going along sideways with the areas, each head over its column; a reload
//   keeps all of it; nothing of it in a request.
// 2b a place filled (folia/design/hinzufuegen/konzepte.html, „Was heute schiefgeht"): the same study,
//   „Anwendungsfach" alone put into WiSe 2026/27 by the workbench's „Vormerken"; „Modul wählen" in
//   its menu and at its row opens the workbench at its subgroup, whose „Im Katalog suchen" leads to
//   the catalog with the place (`fill=`) and the bar of the workbench, whose „Einplanen" aims at it
//   and its semester; planned, Allgemeine BWL I fills it — „Zurück zu WiSe 2026/27" shows it planned,
//   the semester and Nebenfach count 6 LP, not 12, the place full stands no more and the module
//   says „für „Anwendungsfach“"; taken out again, the place stands again, and the Wahlpflicht's
//   „+ In WiSe 26/27" fills it the same way.
// 2c the Gesamtplan reworked (owner, 2026-11-09): the Fachstudium's head unfolds into its Komplexe
//   (10–24, the room to 30) and its Seminar, the Nebenfach's into its two choices, Praktische
//   Mathematik's 8 of 6 LP no „über Bedarf"; a subsection into its lanes; a module dragged onto
//   another subsection that lists it counts there („Rückgängig"), onto one that does not it moves
//   in its own and the row says what Strg does, with Strg it counts there as an exception;
//   „Vollbild" at the head's right end, the rail and the search staying, Esc out of it; „Als Bild
//   speichern" a PNG of the day.
// 2d a place as a place for modules (owner, 2026-11-09: „dass die verschwinden, wenn die voll
//   erfüllt sind … Dann soll der platzhalter azeigen, wie viel progress da schon eingeplant ist"):
//   the Anwendungsfach set to 12 LP with a module of 6 in it stands with its bar, „6 von 12 LP
//   eingeplant", and counts what is left; „Umfang …" in its menu, turned to 6 („Wie im Plan"
//   then gone), takes the place away and the eleventh field with it; „Rückgängig". A place of the
//   Komplex Praktische Informatik in WiSe 2027/28: its dialog weighs the three Komplexe, „Zusammen
//   10 von 50 LP", 4 more turn it to 14, stored and shown.
// 3 a phone: the overview first (owner, 2026-10-05), the bar with its legend, the program's card
//   and its ways, the box „Studium planen" with a column per semester, the way to all programs, and
//   no sidebar; one row of pages the browser scrolls (a real touch, CDP; owner, 2026-10-06): the
//   semesters that are over, the overview, the current one and the ones to come, the dots of all of
//   them over the tab bar on the overview too; the box leads to the current semester (no step of the
//   history and no address of its own: the entry keeps it, folia/crates/shell/src/nav.rs),
//   its column to that one, and the whole overview drawn to the left by a finger to the current
//   one, to the right to the last that is over, a short pull back; the way back a square box left
//   of the search on either side; the semesters pages in the catalog's look, no box, their head
//   under the search, a card per module as the catalog's (its padding, its title, its number and
//   area under its name, its credits in LP, its status where the bookmark is; owner, 2026-10-19),
//   as far apart as from the screen's edge: no ground under the row, on none of its pages (owner,
//   2026-10-19), the Regelstudienplan and all programs with theirs, the dots fixed over
//   the tab bar on the page's frosted ground, no box and no legend, an empty semester a mark and
//   „Module hinzufügen" and nothing of the plan's open rows under it (owner, 2026-10-20), which
//   opens the workbench: the area a button over the list, the bar of the mode over the tab bar,
//   „Plan übernehmen" with its note over that bar and „Rückgängig", the semester's sheet with what
//   it holds and its × , „Fertig"; ‹ › scroll smoothly, past the overview
//   from the current semester to the one before; a finger moves the row and a short pull goes
//   back, a semester swiped to from the end of another, which ends above the dots, shows from its
//   head, and one scrolled while the row moves stays as scrolled, up or down, with nothing of the
//   page that goes to scroll into, the last page adds a
//   semester; a long press selects a row, highlighted and no box to tick, the bar over the tab
//   bar; a row's menu at its status as a sheet without
//   „Modul ansehen", „Verschieben nach" in its place; the way back, Back and a finger towards the
//   overview lead back to it (Back as a close request, as Android has it); a tap anywhere on the
//   credits opens the areas
//   as a sheet a finger drags down to close it (a short pull: it slides back), the program's card
//   opens „Studiengang wechseln" without a lead, − and + move nothing of it, under the card a line
//   at the foot of the body, its picker of the program 8 px off the sheet's edges, the screen's
//   keyboard up (simulated: the part of the page shown shorter) the sheet that part without its
//   foot, the picker at the top of its body, gliding there and scrolling up frame by frame, all as
//   it was with the keyboard gone; nothing scrolls sideways; a row's status as tall as a finger.
// 4 (until 2026-11-04 the Stundenplan's import from „Mein Studium", which went with „Importieren").
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
// Six modules planned into SoSe 2027, none beyond what its area asks.
const LATER = ["12202", "12204", "12214", "11213", "11102", "11104"].map((id, i) => `m\t2027S\t${id}\t${i + 1}\t\t`).join("\n") + "\n";

const takeover = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
// Where the parts of the study's card are (owner, 2026-10-16: „Wenn man eine aktion macht, wie
// semester +/- darf nicht unterbrochen werden durch layout shifts").
// Until nothing of `selector` moves: none of its animations running (a dialog gliding, a picker's
// button drawing out, its list unrolling) and its scrolling body still for four frames.
const still = (page, selector) => page.evaluate((selector) => new Promise((done) => {
  let last = null, frames = 0;
  const started = performance.now();
  const tick = () => {
    const element = document.querySelector(selector);
    const moving = element?.getAnimations({ subtree: true }).some((animation) => animation.playState === "running");
    const at = [...(element?.querySelectorAll(".st-dlg-body") ?? [])].map((body) => body.scrollTop).join() + "|" + scrollY;
    frames = !moving && at === last ? frames + 1 : 0;
    last = at;
    if (frames >= 4 || performance.now() - started > 3000) done(); else requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}), selector);
// The screen's keyboard as a browser has it (`folia_design::keyboard`): the part of the page shown
// shrinks by `kb` px while the page stays; 0, it has gone. Then until the dialog has glided and its
// body scrolled.
const screenKeyboard = async (page, kb) => {
  await page.evaluate((kb) => {
    const viewport = window.visualViewport;
    if (kb) {
      const height = document.documentElement.clientHeight - kb;
      Object.defineProperty(viewport, "height", { get: () => height, configurable: true });
    } else {
      delete viewport.height;
    }
    viewport.dispatchEvent(new Event("resize"));
  }, kb);
  await still(page, ".st-dialog[open]");
};
const cardAt = (page, scope) => page.evaluate((scope) => [...document.querySelectorAll(`${scope} .st-study-say, ${scope} .st-srow, ${scope} .st-stepper button, ${scope} #st-fill, ${scope} #st-create, ${scope} .st-effect, ${scope} .st-dlg-foot .btn`)].map((el) => { const r = el.getBoundingClientRect(); return [r.left, r.top, r.width, r.height].map(Math.round).join(); }).join(" "), scope);
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
// The note's „Rückgängig": the click takes the note away with it, which Playwright, still at the
// button after the click, can take for a button gone before it; its retries then wait for a button
// that is no more. The step's `until` says whether the change was taken back.
const undo = (page, selector = ".study-note .plan-toast-undo") => page.click(selector, { timeout: 5000 }).catch(() => {});
// The rows of the semester in focus: their names, whether passed, whether selected, their marks.
const rows = (page) => page.evaluate(() => [...document.querySelectorAll(".st-card-sem .st-row")].map((row) => ({
  name: row.querySelector(".st-name")?.firstChild?.textContent,
  done: row.classList.contains("is-passed") && !!row.querySelector(".st-status .st-glyph.passed"),
  on: row.classList.contains("is-selected"),
  chips: [...row.querySelectorAll(".t small > .flag, .t small > .st-quiet")].map((chip) => chip.textContent),
})));
// The semester `label` („WiSe 25/26") by the list at the name of the one shown.
const pick = async (page, label) => {
  await page.click(".st-card-sem .st-sem-pick");
  await page.click(`.st-pop.root [role=menuitem]:has-text("${label}")`);
};
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
  // No program, no study direction to choose: its row is folded away, out of reach of Tab (owner,
  // 2026-10-16: „wenn ein studiengang keine ausrichtung hat, dann sollte das nicht eingeblendet sein").
  const fold = await page.$eval(".st-setup .st-fold", (el) => ({ height: el.getBoundingClientRect().height, inert: el.inert }));
  check(fold.height === 0 && fold.inert, `the study direction's row without a program: ${JSON.stringify(fold)}`);
  check((await stored(page, PLAN)) === null && (await stored(page, MINE)) === null, "looking at „Mein Studium“ stored something");
  // The program: a winter intake starts in the current winter, which has nothing before it.
  await step("pick a program", async () => {
    await page.click("#st-program");
    await page.fill("#st-program-search", "informatik b.sc. 2008");
    // The list follows what is typed in the next frame.
    await page.waitForFunction(() => document.querySelector("#st-program-list [role=option]")?.textContent.startsWith("Informatik"), null, { timeout: 5000 }).catch(() => {});
    await page.keyboard.press("Enter");
  }, () => document.querySelector(".st-setup .st-srow[data-start]")?.dataset.start === "2026W" && !document.querySelector("#st-create")?.disabled);
  check(await page.$eval("#st-fill", (box) => box.disabled), "a first semester is offered to be filled from the plan");
  check((await stored(page, MINE)) === null, "a pick in the setup was stored before „Studium anlegen“");
  // The Fachsemester, not the Studienbeginn (owner, 2026-10-16: „ich bin in informatik 9. semester"):
  // two up is the third, begun in the WiSe 2025/26; nothing of the card moves meanwhile.
  const setupAt = await cardAt(page, ".st-setup");
  await step("3. Fachsemester", async () => { await page.click("#st-fs-more"); await page.click("#st-fs-more"); }, () => document.querySelector(".st-setup .st-srow[data-start]")?.dataset.start === "2025W" && document.querySelector("#st-fill")?.textContent.includes("1. und 2. FS nach Regelstudienplan füllen"));
  check((await cardAt(page, ".st-setup")) === setupAt, `+ moved the card: ${setupAt} → ${await cardAt(page, ".st-setup")}`);
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
  await step("Studium", () => page.click('.rail a[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
  const over = await page.evaluate(() => ({
    headline: document.querySelector(".st-headline")?.textContent,
    cards: [...document.querySelectorAll(".st-card .st-card-name")].map((name) => name.textContent),
    words: document.querySelector(".st-over")?.textContent ?? "",
    now: document.querySelector(".st-card-sem .st-sem-name .st-now")?.textContent,
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
  check(side.labels.join("|") === "Ansichten|Studiengang|Fortschrittsbalken|Zeichen an Modulen" && side.mine.join("|") === "Informatik B.Sc.|PO 2008|3. Fachsemester · seit WiSe 25/26" && !side.timetable, `the sidebar: ${JSON.stringify(side)}`);
  check(side.legend.join("|") === "BestandenGeplantOffenÜber Bedarf|GeplantLäuftBestandenNicht bestanden", `the legend: ${JSON.stringify(side.legend)}`);
  await step("the program's card", () => page.click("#st-mine"), () => document.querySelector(".st-dialog[open] #st-switch-program"));
  await step("not changed", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-dialog[open]"));

  // The semester at the top of its box (owner, 2026-10-19: „Die aktuelle Auswahl … kann dann nach
  // ganz oben in die box wandern"): no strip of the semesters over it, no switch of the views.
  const top = await page.evaluate(() => ({
    strip: document.querySelectorAll(".st-strip, .st-tab, .st-sems .st-sems-head, .st-sems .st-views").length,
    head: Math.round(document.querySelector(".st-sems .st-sem-head").getBoundingClientRect().top - document.querySelector(".st-sems").getBoundingClientRect().top),
  }));
  check(top.strip === 0 && top.head <= 16, `the semester is not at the top of its box: ${JSON.stringify(top)}`);
  // Its name opens the list of every semester (the same day: „auf das semester selbst klicken
  // können, um eine selection … zu bekommen"): what each holds, „jetzt", the one shown, one more.
  await step("the list of the semesters", () => page.click(".st-card-sem .st-sem-pick"), () => document.activeElement?.matches(".st-pop.root [aria-current=true]"));
  const list = await page.evaluate(() => ({
    entries: [...document.querySelectorAll(".st-pop.root [role=menuitem]")].map((item) => item.querySelector(".st-pop-label").textContent.trim() + "|" + (item.querySelector(".st-pop-note")?.textContent ?? "")),
    shown: document.querySelector(".st-pop.root [aria-current=true] .st-pop-label")?.textContent.trim(),
    focus: document.activeElement?.getAttribute("aria-current"),
  }));
  check(list.entries.length === 7 && list.entries[0] === "WiSe 25/26 · 1. FS|0 von 32 bestanden" && list.entries[2] === "WiSe 26/27 · 3. FS jetzt|noch leer · Plan 30" && list.entries[6] === "Semester anfügen|" && list.shown === "WiSe 26/27 · 3. FS jetzt" && list.focus === "true", `the list of the semesters: ${JSON.stringify(list)}`);
  await step("closed by Escape", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-pop") && document.activeElement?.classList.contains("st-sem-pick"));

  // The first winter: its rows not passed (Wiederholer), all selected by the semester's ⋯.
  await step("the first winter", () => pick(page, "WiSe 25/26"), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2025/26");
  // Its rows as the catalog's (owner, 2026-10-19: „1 zu 1 der Style vom Modulkatalog"): the
  // catalog's row with its columns, the status where the catalog has the bookmark, no box to
  // tick, a name that is no link of its own that underlines.
  await page.hover(".st-card-sem .st-row >> nth=0 >> .st-name");
  const looks = await page.evaluate(() => {
    const row = document.querySelector(".st-card-sem .st-row");
    const box = row.querySelector(":scope > .row");
    return {
      catalog: Boolean(box && row.classList.contains("row-wrap") && box.querySelector(":scope > .t > b") && box.querySelector(":scope > .t > small > .mono") && box.querySelector(":scope > .lp small") && box.querySelector(":scope > .turnus")),
      cols: [...document.querySelectorAll(".st-card-sem .cols > span")].map((span) => span.textContent).join(","),
      status: Boolean(row.querySelector(":scope > .st-status .st-glyph.failed")),
      boxes: document.querySelectorAll(".st-card-sem .st-sel, .st-card-sem .st-row .st-box, .st-card-sem .st-more").length,
      underline: getComputedStyle(row.querySelector(".st-name")).textDecorationLine,
      pointer: getComputedStyle(box).cursor + "," + getComputedStyle(row.querySelector(".st-name")).cursor,
    };
  });
  check(looks.catalog && looks.cols === "Modul,Verantwortlich,Prüfung,LP,Turnus,Spr.,Termine" && looks.status && looks.boxes === 0 && looks.underline === "none", `the rows are not the catalog's: ${JSON.stringify(looks)}`);
  await step("select all", async () => { await page.click(".st-card-sem .st-sem-more"); await page.click("#st-menu-select"); }, () => document.querySelector(".st-selbar"));
  const before = await rows(page);
  check(before.length === 5 && before.every((row) => !row.done && row.on && row.chips.some((chip) => chip.startsWith("nicht bestanden"))), `the first winter: ${JSON.stringify(before)}`);
  check((await selected(page)).startsWith("5 ausgewählt"), `the bar: ${await selected(page)}`);
  // Highlighted and hatched (the same day: „per hervorheben/vlt texture").
  const lit = await page.evaluate(() => getComputedStyle(document.querySelector(".st-card-sem .st-row.is-selected > .row")).backgroundImage);
  check(lit.includes("repeating-linear-gradient"), `a selected row is not hatched: ${lit}`);
  await step("let go", () => page.click(".st-selbar .st-act-close"), () => !document.querySelector(".st-selbar") && !document.querySelector(".st-row.is-selected"));

  // A row's menu at its status: „Als bestanden markieren" counts, keeps the row in its place, gives
  // the focus back.
  await step("the row's menu", () => page.click(".st-card-sem .st-row >> nth=0 >> .st-status"), () => document.querySelector(".st-pop.root #st-menu-passed"));
  const entries = await page.evaluate(() => [...document.querySelectorAll(".st-pop.root [role^=menuitem]")].map((item) => item.textContent.trim()));
  // Not the module: a click on the row opens it (owner, 2026-10-05).
  check(entries.join("|") === "Als bestanden markieren|Verschieben nach|Nur besuchenzählt nicht, ohne LP|Auswählen|Entfernen", `the row's menu: ${JSON.stringify(entries)}`);
  await step("Als bestanden markieren", () => page.click("#st-menu-passed"), () => !document.querySelector(".st-pop") && document.querySelector(".st-headline")?.textContent === "8 von 180 LP bestanden");
  const after = await rows(page);
  check(after[0]?.name === before[0]?.name && after[0]?.done && after.length === before.length, `marking moved the row: ${JSON.stringify(after.map((row) => row.name))}`);
  check((await page.evaluate(() => document.activeElement?.classList.contains("st-status"))), "the menu did not give the focus back to the row's status");
  check((await lines(page, "d\t")).join() === "d\t2025W\t12104", `what was stored for passed: ${JSON.stringify(await lines(page, "d\t"))}`);
  check((await page.evaluate(() => document.querySelector(".study-note")?.textContent ?? "")).includes("als bestanden markiert"), "no note to take the mark back");

  // A press drawn over three rows selects them; the bar marks them; „Rückgängig" takes it back.
  const from = await middle(page, ".st-card-sem .st-row >> nth=1 >> .lp");
  const to = await middle(page, ".st-card-sem .st-row >> nth=3 >> .lp");
  await step("draw over three rows", async () => {
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move(to.x, (from.y + to.y) / 2, { steps: 4 });
    await page.mouse.move(to.x, to.y, { steps: 4 });
    await page.mouse.up();
  }, () => document.querySelectorAll(".st-card-sem .st-row.is-selected").length === 3 && document.querySelector(".st-selbar-count")?.textContent.startsWith("3 ausgewählt"));
  check((await rows(page)).map((row) => row.on).join() === "false,true,true,true,false", `the drawn selection: ${JSON.stringify((await rows(page)).map((row) => row.on))}`);
  await step("the bar marks them", () => page.click("#st-sel-passed"), () => !document.querySelector(".st-selbar") && document.querySelector(".study-note")?.textContent.includes("3 Einträge als bestanden markiert") && document.querySelector(".st-headline")?.textContent !== "8 von 180 LP bestanden");
  check((await lines(page, "d\t")).length === 4, `the bar's mark: ${JSON.stringify(await lines(page, "d\t"))}`);
  await step("Rückgängig the mark", () => undo(page), () => document.querySelector(".st-headline")?.textContent === "8 von 180 LP bestanden");

  // The current semester: „Module hinzufügen" opens the workbench (folia/design/hinzufuegen/
  // rechner.html, variant B): the semester's card opens under its head, the page around it as it
  // was (the sidebar, the overview); the rows stay on the left (none yet), the right half has
  // „Laut Plan im 3. FS" with its Pflicht and „Plan übernehmen", the Anwendungsfach dashed, and
  // the ways into „Alle Bereiche", „Gemerkt" and the catalog; „Plan übernehmen" plans the Pflicht
  // at once into the rows on the left, × ends the mode.
  await step("to now", () => pick(page, "WiSe 26/27"), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
  const empty = await page.evaluate(() => ({ words: document.querySelector(".st-card-sem .st-empty p")?.textContent, icon: Boolean(document.querySelector(".st-card-sem .st-empty-icon .icon")), rows: document.querySelectorAll(".st-card-sem .st-addrow").length }));
  check(empty.words === "Noch keine Module" && empty.icon && empty.rows === 0, `an empty semester is no mark with „Module hinzufügen“ under it: ${JSON.stringify(empty)}`);
  await step("Module hinzufügen", () => page.click(".st-card-sem .st-empty-add"), () => document.querySelector(".st-card-sem.split .st-wb-half")?.getAttribute("aria-label") === "Hinzufügen zu WiSe 2026/27" && document.querySelector(".st-wb-half .st-wb-sk")?.textContent === "Laut Plan im 3. FS" && document.querySelector("#st-wb-takeover"));
  const bench = await page.evaluate(() => ({
    ways: [...document.querySelectorAll(".st-wb-half .st-wb-ways .st-wb-a .nm > span:last-child")].map((name) => name.firstChild?.textContent.trim()).join("|"),
    rows: [...document.querySelectorAll(".st-wb-half .st-wb-row")].map((row) => ({ name: row.querySelector(".st-wb-go b")?.textContent, act: row.querySelector(".st-wb-plus")?.textContent.trim(), choice: row.classList.contains("choice") })),
    semester: Boolean(document.querySelector(".st-card-sem.split .st-empty")),
    sidebar: getComputedStyle(document.querySelector("#sidebar")).display,
    overview: Boolean(document.querySelector(".st-over")),
  }));
  check(bench.ways === "Alle Bereiche|Gemerkt|Im Katalog suchen", `the workbench's ways: ${bench.ways}`);
  // A module's credits are its button („+ 8 LP"), a choice a dashed card that goes into its page.
  check(bench.rows.some((row) => row.name === "Theoretische Informatik" && /^8\s*LP$/.test(row.act ?? "")) && bench.rows.some((row) => row.name === "Anwendungsfach" && row.choice), `what the workbench offers: ${JSON.stringify(bench.rows)}`);
  check(bench.semester && bench.sidebar !== "none" && bench.overview, `the page around the opened semester: ${JSON.stringify(bench)}`);
  const ticked = Number((await page.textContent("#st-wb-takeover")).match(/\d+/)?.[0] || 0);
  await step("Plan übernehmen", () => page.click("#st-wb-takeover"), (n) => document.querySelectorAll(".st-card-sem.split .st-row").length === n && document.querySelector(".study-note"), ticked);
  await step("×", () => page.click(".st-wb-half .st-wb-done"), (n) => !document.querySelector(".st-wb-half") && document.querySelectorAll(".st-card-sem .st-row").length === n, ticked);
  const planned = await lines(page, "m\t2026W\t");
  const count = await page.evaluate(() => document.querySelector('.rail .nav[data-area="studyplan"] .nav-count')?.textContent);
  check(Number(count) === planned.length && planned.length === ticked && ticked >= 2, `the Stundenplan's tab counts ${count}, the current semester holds ${planned.length} of ${ticked} ticked`);
  check(planned.some((line) => line.startsWith("m\t2026W\t11787\t")), "Theoretische Informatik is not in the current semester");

  // The semester's box is a list as the catalog's (owner, 2026-10-20: „dass die box wo die module
  // drin sind vom design angepasst werden an die anderen seiten"): nothing beside it („Passt in
  // dieses Semester" went), its head as tall as the list's and its name as large as the count, the
  // heads of the columns across it, right under the head; scrolled down, its head stays at the top
  // with the heads of the columns under it, and the box is as tall as the area shows („dass die
  // untere box … sticky ist und dann wenn man ein wenig gescrolled hat, dass es auch eine volle
  // liste ist").
  const box = () => page.evaluate(() => {
    const area = document.querySelector("#content > .page-stage.front > .work.flowing");
    const list = area.querySelector(".st-sems"), head = list.querySelector(".st-sem-head"), cols = list.querySelector(".st-table > .cols");
    const [a, l, h, c] = [area, list, head, cols].map((el) => el.getBoundingClientRect());
    return {
      beside: document.querySelectorAll(".st-fits, .st-sems aside").length,
      head: Math.round(h.height),
      name: getComputedStyle(head.querySelector("h2")).fontSize,
      across: [c.left - l.left, l.right - c.right].map(Math.round).join(),
      under: Math.round(c.top - h.bottom),
      top: Math.round(h.top - a.top),
      tall: l.height >= innerHeight - 64 - 1,
    };
  });
  const look = await box();
  check(look.beside === 0 && look.head === 61 && look.name === "22px" && look.across === "0,0" && look.under === 0 && look.tall, `the semester's box is not the catalog's list: ${JSON.stringify(look)}`);
  await page.evaluate(() => { document.querySelector("#content > .page-stage.front > .work.flowing").scrollTop = 1000; });
  await page.waitForTimeout(300);
  const scrolled = await box();
  check(Math.abs(scrolled.top) <= 1 && scrolled.under === 0 && scrolled.tall, `scrolled down, the semester's box is not the whole list: ${JSON.stringify(scrolled)}`);
  await page.evaluate(() => { document.querySelector("#content > .page-stage.front > .work.flowing").scrollTop = 0; });

  // A module's menu: „Verschieben nach ›" to the next winter, and back; removed, and back.
  const menu = '.st-card-sem .st-row:has(.st-name:text-is("Theoretische Informatik")) .st-status';
  await step("the menu", () => page.click(menu), () => document.querySelector(".st-pop.root #st-menu-move"));
  await step("Verschieben nach", () => page.click("#st-menu-move"), () => document.querySelector(".st-pop.sub [role=menuitem]"));
  const targets = await page.evaluate(() => [...document.querySelectorAll(".st-pop.sub [role=menuitem]")].map((item) => item.textContent.trim()));
  check(targets.some((target) => target.startsWith("SoSe 27 · 4. FS")) && targets.some((target) => target.startsWith("WiSe 27/28 · 5. FS") && target.endsWith("angeboten")), `where the menu moves to: ${JSON.stringify(targets)}`);
  await step("verschieben", () => page.click('.st-pop.sub [role=menuitem]:has-text("WiSe 27/28")'), () => document.querySelector(".study-note")?.textContent.includes("ins WiSe 2027/28 verschoben") && !document.querySelector(".st-pop"));
  check((await focused(page)) === "WiSe 2026/27", "moving took the focus to the other semester");
  check((await lines(page, "m\t2027W\t11787\t")).length === 1 && (await lines(page, "m\t2026W\t11787\t")).length === 0, "the module did not move");
  await step("Rückgängig", () => undo(page), () => !document.querySelector(".study-note"));
  await page.waitForTimeout(300);
  check((await lines(page, "m\t2026W\t11787\t")).length === 1, "„Rückgängig“ did not bring the module back");
  // The keys: Enter opens the menu, the arrows go through it and into its submenu and back,
  // Escape closes it and gives the focus back to the row's status.
  // A click on the row opens the module: the menu does not offer it again.
  await step("the menu by the keys", async () => {
    await page.focus(menu);
    await page.keyboard.press("Enter");
  }, () => document.activeElement?.id === "st-menu-passed" && !document.querySelector("#st-menu-view"));
  await step("down to Verschieben nach", () => page.keyboard.press("ArrowDown"), () => document.activeElement?.id === "st-menu-move");
  await step("into the submenu", () => page.keyboard.press("ArrowRight"), () => document.activeElement?.closest(".st-pop.sub"));
  await step("back out of it", () => page.keyboard.press("ArrowLeft"), () => !document.querySelector(".st-pop.sub") && document.activeElement?.id === "st-menu-move");
  await step("Escape gives the focus back", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-pop") && document.activeElement?.classList.contains("st-status"));
  await step("the menu again", () => page.click(menu), () => document.querySelector("#st-menu-remove"));
  await step("Entfernen", () => page.click("#st-menu-remove"), () => document.querySelector(".study-note")?.textContent.includes("„Theoretische Informatik“ entfernt"));
  check((await lines(page, "m\t2026W\t11787\t")).length === 0, "„Entfernen“ left the module");
  await step("Rückgängig again", () => undo(page), () => [...document.querySelectorAll(".st-card-sem .st-name")].some((name) => name.textContent === "Theoretische Informatik"));

  // Two rows selected by Ctrl and a click, moved by the bar to SoSe 27: both move, and back.
  await step("select two", async () => {
    await page.click(".st-card-sem .st-row >> nth=0 >> .row", { modifiers: ["Control"] });
    await page.click(".st-card-sem .st-row >> nth=1 >> .row", { modifiers: ["Control"] });
  }, () => document.querySelectorAll(".st-card-sem .st-row.is-selected").length === 2 && location.search === "");
  const two = (await rows(page)).filter((row) => row.on).map((row) => row.name);
  await step("move them to SoSe 27", async () => { await page.click("#st-sel-move"); await page.click('.st-pop.root [role=menuitem]:has-text("SoSe 27")'); }, () => document.querySelector(".study-note")?.textContent.includes("2 Einträge ins SoSe 2027 verschoben"));
  const left = (await rows(page)).map((row) => row.name);
  check(two.length === 2 && two.every((name) => !left.includes(name)) && (await lines(page, "m\t2027S\t")).length === 2, `the two moved: ${JSON.stringify({ two, left })}`);
  await step("Rückgängig the move", () => undo(page), (n) => document.querySelectorAll(".st-card-sem .st-row").length === n, left.length + 2);

  // A right click opens a row's menu where the pointer is; the semester's ⋯ selects all of it;
  // Escape lets go.
  await step("a right click", () => page.click(".st-card-sem .st-row >> nth=1 >> .row", { button: "right" }), () => document.querySelector(".st-pop.root #st-menu-passed"));
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
  const other = await page.evaluate(() => [...document.querySelectorAll(".st-card-sem .st-row")].findIndex((row) => row.querySelector("a.st-name") && row.querySelector("a.st-name").textContent !== "Theoretische Informatik"));
  // A click on a row beside its name (at its number) opens its module too: the whole box is its
  // button (owner, 2026-10-19). Not at its credits: with the box across the page (owner,
  // 2026-10-20) they lie under the module beside it, as the catalog's do.
  const number = await middle(page, `.st-card-sem .st-row >> nth=${other} >> .mono`);
  await step("another by its row", () => page.mouse.click(number.x, number.y), () => /^\?open=\d+$/.test(location.search) && location.search !== "?open=11787");
  await step("Theoretische Informatik again", () => page.click('.st-card-sem a.st-name:text-is("Theoretische Informatik")'), () => location.search === "?open=11787" && document.querySelector("#preview .hero .mono")?.textContent.trim() === "11787");
  await step("Vollbild", () => page.click('#preview [data-action="fullscreen"]'), () => location.search === "?open=11787&full=1" && document.querySelector(".module-page"));
  check((await page.getAttribute('.rail a[data-area="programs"]', "aria-current")) === "page", "the module in full left the Studium tab");
  await step("Zurück", () => page.click('[data-action="back"]'), () => location.search === "?open=11787" && document.querySelector(".st-card-sem"));
  await step("close the module", () => page.click('#preview [data-action="close-detail"]'), () => location.search === "" && !document.querySelector("#preview"));

  // The Gesamtplan as a roadmap (folia/design/gesamtplan/konzepte.html, A; owner, 2026-11-03):
  // an area's bar of pieces named by their abbreviations, none where the area has nothing, the
  // band of the study years, the line of today; a click anywhere on an area's head unfolds a lane
  // per module, kept, the one button folds them (owner, 2026-11-09); „Bisher zuklappen"; a
  // module's menu; the plan's open module dashed and planned with a click; the view kept.
  await step("Gesamtplan", () => page.click('#sidebar .st-vl:has-text("Gesamtplan")'), () => document.querySelector(".st-rm-grid") && document.querySelectorAll(".st-row-head").length === 6 && document.querySelector(".st-rm-seg .st-rm-p"));
  // A cell as tall as what it lists (the Regelstudienplan's pills once took its class, and the
  // cells shrank to a pill, their modules over the semesters' heads).
  const cut = await page.evaluate(() => [...document.querySelectorAll(".st-grid .st-rm-cell")].filter((cell) => {
    const box = cell.getBoundingClientRect();
    return [...cell.children].some((child) => { const r = child.getBoundingClientRect(); return r.height > 0 && (r.top < box.top - 1 || r.bottom > box.bottom + 1); });
  }).length);
  check(cut === 0, `${cut} cells of the Gesamtplan do not hold what they list`);
  const roadmap = await page.evaluate(() => {
    const grid = document.querySelector(".st-rm-grid");
    const rows = [...grid.querySelectorAll(":scope > .st-row-head")];
    const cells = (row) => {
      const out = [];
      for (let cell = row?.nextElementSibling; cell?.classList.contains("st-rm-cell"); cell = cell.nextElementSibling) out.push(cell);
      return out;
    };
    const row = (name) => rows.find((head) => head.querySelector(".st-card-name")?.textContent === name);
    const first = cells(row("Informatik"))[0];
    const pieces = [...(first?.querySelectorAll(".st-rm-seg .st-rm-p") ?? [])];
    const grow = (label) => Number(pieces.find((piece) => piece.textContent.trim() === label)?.style.flexGrow ?? 0);
    return {
      bands: [...document.querySelectorAll(".st-rm-heads .st-rm-band")].map((band) => band.textContent),
      first: pieces.map((piece) => piece.textContent.trim()),
      // A piece's share of its semester: Entwicklung von Softwaresystemen has 8 LP, the
      // Programmierpraktikum 4.
      wider: grow("EvS") === 8 && grow("PP") === 4,
      thesis: cells(row("Bachelor-Arbeit")).map((cell) => (cell.querySelector(".st-rm-seg") ? 1 : 0)).join(""),
      today: grid.querySelectorAll(".st-rm-cell.rm-now").length === 6 && /^heute, \d\d\.\d\d\.$/.test(grid.querySelector(".st-rm-flag")?.textContent ?? ""),
    };
  });
  check(roadmap.bands.join(" | ") === "1. Studienjahr · 2025/26 | 2. Studienjahr · 2026/27 | 3. Studienjahr · 2027/28", `the band of the study years: ${roadmap.bands.join(" | ")}`);
  check(roadmap.first.join() === "EvS,EEG,PP" && roadmap.wider, `Informatik's first semester: ${roadmap.first.join()} (EvS wider than PP: ${roadmap.wider})`);
  check(/^00/.test(roadmap.thesis) && roadmap.thesis.includes("1"), `the Bachelor-Arbeit's bar is not broken before its semester: ${roadmap.thesis}`);
  check(roadmap.today, "the line of today does not run through the current semester, or its flag is missing");
  // The whole head is the toggle (owner, 2026-11-09: „dass man den kompletten bereich als toogle für
  // das aus und einklappen verwenden kann. Der modal mit dem progress ist komplett redundant"): a
  // click on the area's name unfolds it, no dialog.
  await step("Mathematik's head", () => page.click('.st-row-head:has(.st-card-name:text-is("Mathematik")) .st-card-name'), () => document.querySelector('.st-row-head.is-open .st-card-name')?.textContent === "Mathematik" && [...document.querySelectorAll(".st-rm-lane-name")].some((lane) => lane.textContent === "Mathematik IT-1"));
  check(!(await page.evaluate(() => document.querySelector(".st-dialog[open]"))), "a click on an area's head opened a dialog");
  check((await stored(page, "betula.study.open")) === "Mathematik", `what is unfolded is not kept: ${JSON.stringify(await stored(page, "betula.study.open"))}`);
  // Unfolding and folding move (owner, 2026-11-05: „animationen für das auf und zuklappen.
  // unauffällig undd snappy einfach so, dass das leicher zu verstehen ist"): Mathematik's lanes grew
  // in, the other areas stayed as they were and ▸ kept the focus; folded, they shrink away before
  // they go.
  const grew = await page.evaluate(() => ({
    in: document.querySelectorAll(".st-rm-grid > .st-rm-lane.rm-in").length === document.querySelectorAll(".st-rm-grid > .st-rm-lane").length,
    focus: document.activeElement?.matches(".st-rm-area[aria-expanded=true]"),
  }));
  check(grew.in && grew.focus, `Mathematik's head: its lanes did not grow in, or the head lost the focus: ${JSON.stringify(grew)}`);
  await page.evaluate(() => {
    window.__shrunk = false;
    const seen = new MutationObserver(() => { if (document.querySelector(".st-rm-lane.rm-out")) { window.__shrunk = true; seen.disconnect(); } });
    seen.observe(document.querySelector(".st-rm-grid"), { subtree: true, attributes: true, attributeFilter: ["class"] });
  });
  // One button for all of it (owner, 2026-11-09: „alles auf/zuklappen ein knopf … so dass man halt
  // da im zweifel zwei mal drauf drücken muss"): with one area unfolded it unfolds all, then folds
  // all.
  check((await page.textContent(".st-rm-all"))?.trim() === "Alle aufklappen", `the one button with Mathematik unfolded: ${await page.textContent(".st-rm-all")}`);
  await step("Alle aufklappen", () => page.click(".st-rm-all"), () => document.querySelector(".st-rm-all")?.textContent.trim() === "Alle zuklappen" && [...document.querySelectorAll(".st-rm-grid > .st-row-head")].every((head) => head.classList.contains("is-open")));
  await page.evaluate(() => {
    window.__shrunk = false;
    const seen = new MutationObserver(() => { if (document.querySelector(".st-rm-lane.rm-out")) { window.__shrunk = true; seen.disconnect(); } });
    seen.observe(document.querySelector(".st-rm-grid"), { subtree: true, attributes: true, attributeFilter: ["class"] });
  });
  await step("Alle zuklappen", () => page.click(".st-rm-all"), () => !document.querySelector(".st-rm-lane") && document.querySelector(".st-rm-all")?.textContent.trim() === "Alle aufklappen");
  check((await stored(page, "betula.study.open")) === null, "„Alle zuklappen“ kept something unfolded");
  check(await page.evaluate(() => window.__shrunk), "„Alle zuklappen“: the lanes went at once, without shrinking away");
  await step("Bisher zuklappen", () => page.click('.st-sems-head button:has-text("Bisher zuklappen")'), () => document.querySelector("div.st-col-head.past") && document.querySelector(".st-rm-cell.past .st-rm-p.stub"));
  await step("Bisher aufklappen", () => page.click('.st-sems-head button:has-text("Bisher aufklappen")'), () => !document.querySelector("div.st-col-head.past"));
  // Betriebssysteme I, open in the plan: a dashed piece by its abbreviation. Its semester holds
  // more than fits beside each other here (the Wiederholer of the SoSe 26 too): „4 offen" stands
  // for them, and Informatik's lanes have each.
  const line = await page.evaluate(() => [...document.querySelectorAll(".st-rm-seg .st-rm-p.open")].map((piece) => piece.getAttribute("aria-label")).find((label) => label?.includes("Betriebssysteme I")));
  check(line?.startsWith("BS1 – ") && line.includes("laut Regelstudienplan im 4. FS"), `the plan's open Betriebssysteme I: ${line}`);
  const full = await page.evaluate(() => [...document.querySelectorAll(".st-rm-cell[data-full] .st-rm-sum")].map((sum) => sum.textContent.trim()));
  check(full.length > 0 && full.every((sum) => /\d+ (Module|offen)/.test(sum)), `a semester too full for its labels: ${JSON.stringify(full)}`);
  await step("a module's menu in the Gesamtplan", () => page.click(".st-rm-cell:not([data-full]) .st-rm-seg .st-rm-p:not(.open) >> nth=0"), () => document.querySelector(".st-pop.root #st-menu-remove"));
  // Beside it on the page, over the sidebar: since the plan's head stays at the top (Folia 1.71.2),
  // the area is masked as the semester's is, a layer of its own under the rail and the bar, and a
  // click on them reaches them, as it does from a semester's menu.
  await step("closed by a click beside it", () => page.click(".st-pop-shade", { position: { x: 200, y: 850 } }), () => !document.querySelector(".st-pop"));
  // Informatik unfolds into its subsections, its Pflichtmodule and „Proseminar oder Praktikum“; its
  // Pflichtmodule into their lanes.
  await step("Informatik's head", () => page.click('.st-row-head:has(.st-card-name:text-is("Informatik")) .st-card-name'), () => document.querySelector('.st-rm-sub .st-card-name')?.textContent === "Pflichtmodule");
  await step("its Pflichtmodule", () => page.click('.st-rm-sub:has(.st-card-name:text-is("Pflichtmodule")) .st-rm-area'), () => document.querySelector('.st-rm-bar.open[aria-label*="Betriebssysteme I"]'));
  await step("plan the open Betriebssysteme I", () => page.click('.st-rm-bar.open[aria-label*="Betriebssysteme I"]'), () => document.querySelector(".study-note")?.textContent.includes("hinzugefügt"));
  check((await lines(page, "m\t2027S\t")).length === 1, `the open module was not planned: ${JSON.stringify(await lines(page, "m\t2027S\t"))}`);
  check((await stored(page, "betula.study.view")) === "grid", "the view is not remembered");
  // A module dragged over the plan (owner, 2026-11-04: „soll einfach das gesamte semester die
  // dropstelle sein und light gehighlighted werden mit der stelle, wo es final landen soll dann
  // särker"): BS1 of its lane held over the head of WiSe 2027/28 lights that semester from its head
  // down, Informatik's cell there and BS1's lane strongly; let go there, it moves; „Rückgängig".
  const bs1 = (await lines(page, "m\t2027S\t"))[0]?.split("\t")[2];
  const dragFrom = await page.locator('.st-rm-bar[aria-label^="BS1,"]').boundingBox();
  const dragTo = await page.locator('.st-col-head[data-sem="2027W"]').boundingBox();
  if (dragFrom && dragTo) {
    await page.mouse.move(dragFrom.x + 20, dragFrom.y + dragFrom.height / 2);
    await page.mouse.down();
    await page.mouse.move(dragFrom.x + 40, dragFrom.y + dragFrom.height / 2, { steps: 3 });
    await page.mouse.move(dragTo.x + dragTo.width / 2, dragTo.y + dragTo.height / 2, { steps: 10 });
    await page.waitForFunction(() => document.querySelector('.st-col-head[data-sem="2027W"]')?.classList.contains("rm-drop"), null, { timeout: 3000 }).catch(() => {});
  }
  const aimed = await page.evaluate(() => ({
    column: [...document.querySelectorAll('[data-sem="2027W"]')].every((cell) => cell.classList.contains("rm-drop")),
    others: document.querySelectorAll('.rm-drop:not([data-sem="2027W"])').length,
    lands: [...document.querySelectorAll(".rm-drop-here")].map((cell) => {
      let head = cell.previousElementSibling;
      while (head && !head.matches(".st-row-head, .st-rm-lane")) head = head.previousElementSibling;
      return `${cell.dataset.sem} ${head?.querySelector(".st-card-name, .st-rm-lane-name")?.textContent}`;
    }),
  }));
  check(aimed.column && aimed.others === 0 && aimed.lands.join(" | ") === "2027W Informatik | 2027W Pflichtmodule | 2027W Betriebssysteme I", `a module dragged over the head of WiSe 2027/28: ${JSON.stringify(aimed)}`);
  await step("let go over the semester's head", () => page.mouse.up(), (id) => (localStorage.getItem("betula.studyplan.v1") || "").split("\n").some((line) => line.startsWith(`m\t2027W\t${id}\t`)) && !document.querySelector(".rm-drop, .rm-drop-here"), bs1);
  await step("Rückgängig, the module back", () => page.click(".study-note .plan-toast-undo"), (id) => (localStorage.getItem("betula.studyplan.v1") || "").split("\n").some((line) => line.startsWith(`m\t2027S\t${id}\t`)), bs1);
  // The plan's head stays at the top (owner, 2026-11-04: „Ich möchte, dass da der header sticky
  // ist von dem Plan. Also ähnlich wie bei dem Semester plan."): scrolled down, the title, the band
  // of the study years and the semesters' heads at the top of the area, the areas under it; in a
  // window too narrow for every semester, the areas scrolled sideways under it, the heads go along,
  // each over its column.
  await page.setViewportSize({ width: 1100, height: 800 });
  // The current semester in view (owner, 2026-11-04: „dass immer das aktuelle semester passend
  // gezeigt wird"): too narrow for every semester, the plan scrolls sideways by itself to the one
  // before WiSe 2026/27, first after the areas' names, which stay at the left; the heads with it.
  await page.waitForFunction(() => document.querySelector(".st-rm-top-scroll")?.scrollLeft > 0, null, { timeout: 3000 }).catch(() => {});
  const shown = await page.evaluate(() => {
    const box = document.querySelector(".st-grid-scroll:not(.st-rm-top-scroll)").getBoundingClientRect().left;
    const left = (selector) => Math.round(document.querySelector(selector).getBoundingClientRect().left - box);
    return { left: [...document.querySelectorAll(".st-grid-scroll")].map((scroller) => scroller.scrollLeft), now: left(".st-col-head.is-now"), name: left(".st-rm-grid > .st-row-head") };
  });
  check(shown.left.join() === "140,140" && shown.now === 352 && shown.name === 0, `the current semester in view: ${JSON.stringify(shown)}`);
  const stuck = await page.evaluate(() => new Promise((done) => {
    const area = document.querySelector("#content > .page-stage.front > .work.flowing");
    const body = document.querySelector(".st-grid-scroll:not(.st-rm-top-scroll)");
    area.scrollTop = area.scrollHeight;
    body.scrollLeft = 120;
    requestAnimationFrame(() => requestAnimationFrame(() => {
      const at = (el) => Math.round(el.getBoundingClientRect().top - area.getBoundingClientRect().top);
      const out = {
        top: at(document.querySelector(".st-rm-top")),
        panel: at(document.getElementById("st-grid")),
        heads: document.querySelector(".st-rm-top-scroll").scrollLeft,
        body: body.scrollLeft,
        now: [".st-col-head.is-now", ".st-rm-cell.is-now"].map((selector) => Math.round(document.querySelector(selector).getBoundingClientRect().left)),
      };
      area.scrollTop = 0;
      body.scrollLeft = 0;
      done(out);
    }));
  }));
  await page.setViewportSize({ width: 1440, height: 900 });
  check(stuck.panel < -50 && Math.abs(stuck.top + 1) <= 1 && stuck.body === 120 && stuck.heads === 120 && stuck.now[0] === stuck.now[1], `the plan's head at the top: ${JSON.stringify(stuck)}`);

  // A reload keeps all of it.
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(() => document.querySelector(".st-grid") && document.querySelector(".st-headline")?.textContent === "8 von 180 LP bestanden", null, { timeout: 8000 }).catch(() => problems.push("a reload lost the view or what was ticked off"));
  await page.evaluate(() => localStorage.removeItem("betula.study.view"));
  // The reload was a page load of its own; from here on none.
  await page.evaluate(() => { window.__marker = 1; });

  // The views (owner, 2026-10-14, concept A „Ansichten"): the Regelstudienplan and the Wahlpflicht
  // are views of „Mein Studium", named in its sidebar, each with the student's stand in it.
  const views = await page.evaluate(() => [...document.querySelectorAll("#sidebar .st-vl")].map((a) => a.querySelector("span").textContent + (a.getAttribute("aria-current") === "page" ? "*" : "")));
  check(views.join() === "Semester,Gesamtplan*,Regelstudienplan,Wahlpflicht", `the views: ${views}`);
  await step("Regelstudienplan", () => page.click('#sidebar .st-vl:has-text("Regelstudienplan")'), () => location.search === "?view=plan" && document.querySelectorAll(".st-mx tbody tr.st-rsp-row").length === 23);
  const regular = await page.evaluate(() => {
    const row = (name) => [...document.querySelectorAll(".st-mx tbody tr.st-rsp-row")].find((tr) => tr.querySelector(".st-rsp-name")?.textContent === name);
    return {
      current: document.querySelector('#sidebar .st-vl[aria-current="page"] span')?.textContent,
      groups: [...document.querySelectorAll(".st-mx tr.g .st-g b")].map((b) => b.textContent),
      columns: [...document.querySelectorAll(".st-mx thead th.fs b")].map((b) => b.textContent).join(""),
      os: row("Betriebssysteme I")?.querySelector(".st-minec")?.textContent.trim(),
      passed: document.querySelectorAll(".st-mx .st-rsp-c.is-passed").length,
      choose: [...document.querySelectorAll(".st-mx button.st-addb")].filter((button) => button.textContent.startsWith("Wählen")).length,
      // The overview, its bar and the legend of the bar stay with the semesters (owner, 2026-11-04).
      overview: document.querySelectorAll(".st-over, .st-bar, .st-legend-parts").length,
    };
  });
  check(regular.current === "Regelstudienplan" && regular.groups.slice(0, 2).join() === "Komplex Informatik,Komplex Mathematik" && regular.columns === "123456" && regular.os === "SoSe 27" && regular.passed > 0 && regular.choose > 3 && regular.overview === 0, `the Regelstudienplan: ${JSON.stringify(regular)}`);
  await step("a row planned from the plan", () => page.click('.st-mx tr.st-rsp-row:has(.st-rsp-name:text-is("Digitaltechnik-Praktikum")) button.st-addb'), () => document.querySelector(".study-note")?.textContent.includes("„Digitaltechnik-Praktikum“ ins SoSe 2027 eingeplant"));
  await page.waitForFunction(() => [...document.querySelectorAll(".st-mx tbody tr.st-rsp-row")].find((tr) => tr.querySelector(".st-rsp-name")?.textContent === "Digitaltechnik-Praktikum")?.querySelector(".st-minec")?.textContent.trim() === "SoSe 27", null, { timeout: 4000 }).catch(() => problems.push("the row planned from the plan does not say where it is planned"));
  check((await lines(page, "m\t2027S\t12214")).length === 1, "„Einplanen\" of the plan did not plan the module");
  await step("Rückgängig", () => undo(page, ".study-note .plan-toast-undo"), () => !document.querySelector(".study-note"));
  await page.waitForFunction(() => document.querySelector(".st-mx") !== null, null, { timeout: 4000 });
  // The store follows the click after its frame (R21).
  await page.waitForFunction(([key, prefix]) => !(localStorage.getItem(key) || "").split("\n").some((line) => line.startsWith(prefix)), [PLAN, "m\t2027S\t12214"], { timeout: 4000 }).catch(() => {});
  check((await lines(page, "m\t2027S\t12214")).length === 0, "„Rückgängig\" did not take the module out again");
  // „Wählen ›" opens the workbench at the choice, for the semester of its first Fachsemester: on a
  // desktop the semesters' view first, its card opened at the choice's page
  // (folia/design/hinzufuegen/rechner.html, variant B); × leads back to the view it came from.
  await step("Wählen: the workbench", () => page.click('.st-mx tr.st-rsp-row:has-text("Anwendungsfach") button.st-addb'), () => location.search === "" && document.querySelector(".st-card-sem.split .st-wb-half .st-wb-pt > b")?.textContent === "Anwendungsfach" && document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
  await step("×, the Regelstudienplan again", () => page.click(".st-wb-half .st-wb-done"), () => !document.querySelector(".st-wb-half") && location.search === "?view=plan" && document.querySelector(".st-mx"));
  await step("the Wahlpflicht", () => page.click('#sidebar .st-vl:has-text("Wahlpflicht")'), () => location.search === "?view=areas" && document.querySelector(".st-sl"));
  await step("its Anwendungsfach", () => page.click('.st-sl:has-text("Anwendungsfach")'), () => document.querySelector('.st-sl[aria-current="true"]')?.textContent.includes("Anwendungsfach"));
  // To look up how much a choice asks and what there is (folia/design/wahlpflicht, second round):
  // no semester and no button to plan, the Komplexe one entry of 50 with the three under it, the
  // Wahlpflicht im 4. FS no entry of its own.
  const choice = await page.evaluate(() => ({
    current: document.querySelector('#sidebar .st-vl[aria-current="page"] span')?.textContent,
    lists: [...document.querySelectorAll(".st-wtabs button > span")].map((span) => span.childNodes[0].textContent),
    rows: document.querySelectorAll(".st-wrows .st-wrow").length,
    figure: document.querySelector('.st-sl[aria-current="true"] .st-wfig')?.textContent,
    buttons: document.querySelectorAll(".st-split-r button.st-addb, .st-split-r .st-views, .st-split-r .btn").length,
    entries: [...document.querySelectorAll(".st-sl .st-sl-t > b")].map((b) => b.textContent),
    komplexe: [...document.querySelectorAll(".st-sl-kid > span:first-child")].map((span) => span.textContent),
    overview: document.querySelectorAll(".st-over, .st-bar, .st-legend-parts").length,
  }));
  check(choice.current === "Wahlpflicht" && choice.lists.length === 5 && choice.lists[0] === "Mathematik" && choice.rows > 3 && /^\d+ \/ 12\u00a0LP$/.test(choice.figure || "") && choice.buttons === 0 && choice.overview === 0
    && choice.entries.includes("Komplexe des Fachstudiums") && !choice.entries.some((name) => name.startsWith("Wahlpflicht")) && choice.komplexe.join() === "Grundlagen der Informatik,Praktische Informatik,Angewandte und Technische Informatik", `the Wahlpflicht: ${JSON.stringify(choice)}`);
  // A module of the list opens beside the page; its „Einplanen" plans for the choice.
  const figured = parseFloat(choice.figure);
  const SIDE = '.detail .hero .plan-toggle[data-action="plan"]';
  const offered = await page.evaluate(() => { const row = document.querySelector(".st-wrows .st-wrow:not(.has)"); return { id: row?.querySelector(".st-wrow-id")?.textContent, credits: parseFloat(row?.querySelector(".st-wrow-lp")?.textContent) }; });
  await step("a module of its list beside the page", () => page.click(".st-wrows .st-wrow:not(.has) >> nth=0"), (selector) => document.querySelector(selector)?.checkVisibility(), SIDE);
  const aimedChoice = await page.evaluate((selector) => document.querySelector(selector)?.textContent.replace(/\s+/g, " "), SIDE);
  check(aimedChoice?.includes("für „Anwendungsfach“"), `the sidebar does not plan for the choice: ${aimedChoice}`);
  await step("Einplanen, for the choice", () => page.click(SIDE), ([key, id]) => (localStorage.getItem(key) || "").split("\n").some((line) => line.startsWith("m\t") && line.split("\t")[2] === id), [PLAN, offered.id]);
  await page.waitForFunction((after) => document.querySelector('.st-sl[aria-current="true"] .st-wfig')?.textContent.startsWith(`${after} / 12`), String(figured + offered.credits).replace(".", ","), { timeout: 4000 }).catch(() => problems.push("the choice does not count the module planned out of its list"));
  // The Komplexe: no tabs over their modules, all three's under their names; a Komplex picked on
  // the left, its own (owner, 2026-11-05: „Mach mal die auswahl in dem fachbereich oben weg das ist
  // redundant").
  const sections = () => [...document.querySelectorAll(".st-split-r .st-wsub > span:first-child")].map((head) => head.textContent).join();
  await step("the Komplexe", () => page.click('.st-sl:has-text("Komplexe des Fachstudiums")'), (all) => !document.querySelector(".st-split-r .st-wtabs") && new Function(`return (${all})`)()() === "Grundlagen der Informatik,Praktische Informatik,Angewandte und Technische Informatik", sections.toString());
  await step("a Komplex of them", () => page.click('.st-sl-kid:has-text("Praktische Informatik")'), (all) => new Function(`return (${all})`)()() === "Praktische Informatik" && document.querySelector('.st-sl-kid[aria-current="true"]')?.textContent.includes("Praktische Informatik"), sections.toString());
  await step("all of them again", () => page.click('.st-sl:has-text("Komplexe des Fachstudiums")'), (all) => new Function(`return (${all})`)()().split(",").length === 3 && !document.querySelector('.st-sl-kid[aria-current="true"]'), sections.toString());
  await step("back to the semesters", () => page.click('#sidebar .st-vl:has-text("Gesamtplan")'), () => location.search === "" && document.querySelector(".st-grid") && document.querySelector(".st-over .st-bar") && document.querySelector("#sidebar .st-legend-parts"));
  for (const request of requests) {
    if (/12104|079-82-2008|11787%0A|d%092025W|start%09/.test(request.replace(/\/catalog\/module\/\d+|open=11787/g, ""))) problems.push(`a request carries the study: ${request.slice(0, 160)}`);
  }
  await context.close();
}

// ---------- 2b: a place filled ----------
{
  const { page, step, seed, context } = await open({ viewport: { width: 1440, height: 900 } }, "/");
  await seed([[MINE, THIRD], [PLAN, FILLED]]);
  await step("Studium, for a place", () => page.click('.rail a[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
  // What the semester and Nebenfach count.
  const counts = () => page.evaluate(() => ({
    semester: document.querySelector(".st-card-sem .st-sem-sub b")?.textContent.replace(/ /g, " "),
    nebenfach: [...document.querySelectorAll(".st-card")].find((card) => card.querySelector(".st-card-name")?.textContent === "Nebenfach")?.querySelector(".st-card-figure")?.textContent,
  }));
  // „Anwendungsfach" alone into WiSe 2026/27, as a place of 6 LP: „Vormerken" in its subgroup.
  await step("Module hinzufügen, for a place", () => page.click(".st-card-sem .st-empty-add"), () => document.querySelector(".st-wb-half #st-wb-takeover"));
  await step("Anwendungsfach in the workbench", () => page.click('.st-wb-half .st-wb-row.choice:has-text("Anwendungsfach") .st-wb-go'), () => document.querySelector(".st-wb-half .st-wb-pt > b")?.textContent === "Anwendungsfach");
  await step("the place put in", () => page.click('.st-wb-half .st-wb-foot button:has-text("Vormerken")'), () => document.querySelector(".study-note")?.textContent.includes("Anwendungsfach in WiSe 2026/27 vorgemerkt"));
  await step("×, for a place", () => page.click(".st-wb-half .st-wb-done"), () => !document.querySelector(".st-wb-half") && document.querySelectorAll(".st-card-sem .st-row").length === 1);
  // The store follows the click after its frame (R21).
  const place = (key) => (localStorage.getItem(key) || "").split("\n").map((line) => line.split("\t")).find((fields) => fields[0] === "p" && fields[2] === "2026W" && fields[9] === "Anwendungsfach")?.[1];
  await page.waitForFunction(place, PLAN, { timeout: 4000 }).catch(() => {});
  const pid = await page.evaluate(place, PLAN);
  check(!!pid, `the place was not stored: ${JSON.stringify(await lines(page, "p\t"))}`);
  const open6 = await counts();
  check(open6.semester === "6 LP" && open6.nebenfach === "6/18", `the place alone: ${JSON.stringify(open6)}`);
  // In the Gesamtplan the place is a piece of its own, dashed, as tall as the others (until Folia
  // 1.70.1 it took the catalog's list row's class, and with it its grid and height); its lane names
  // it and says what it is.
  await step("the place in the Gesamtplan", () => page.click('#sidebar .st-vl:has-text("Gesamtplan")'), () => [...document.querySelectorAll(".st-rm-seg .st-rm-p.is-ph")].some((piece) => piece.textContent.trim() === "Anwendungsfach"));
  const inPlan = await page.evaluate(() => {
    const piece = [...document.querySelectorAll(".st-rm-seg .st-rm-p.is-ph")].find((piece) => piece.textContent.trim() === "Anwendungsfach");
    const box = piece?.getBoundingClientRect();
    const cell = piece?.closest(".st-rm-cell").getBoundingClientRect();
    return box && { height: Math.round(box.height), inside: box.top >= cell.top && box.bottom <= cell.bottom };
  });
  check(inPlan?.height === 28 && inPlan.inside, `the place in the Gesamtplan: ${JSON.stringify(inPlan)}`);
  await step("Nebenfach's head", () => page.click('.st-row-head:has(.st-card-name:text-is("Nebenfach")) .st-card-name'), () => document.querySelector('.st-rm-sub .st-card-name')?.textContent === "Anwendungsfach");
  await step("its Anwendungsfach, the place", () => page.click('.st-rm-sub:has(.st-card-name:text-is("Anwendungsfach")) .st-rm-area'), () => {
    const lane = [...document.querySelectorAll(".st-rm-lane")].find((lane) => lane.querySelector(".st-rm-lane-name")?.textContent === "Anwendungsfach");
    let bar = null;
    for (let cell = lane?.nextElementSibling; !bar && cell?.classList.contains("st-rm-lc"); cell = cell.nextElementSibling) bar = cell.querySelector(".st-rm-bar.is-ph");
    return bar?.textContent.trim() === "Platzhalter";
  });
  // The rows of one name are one lane, their credits added up (owner, 2026-11-04: „warum gibt es
  // hier zwei mal anwendungsfach als ein eintrag mit jeweils 6 punkten. Das sollte einmal mit 12
  // sein"): the place of the 3rd FS and what the plan has open of the 4th.
  const anwendungsfach = await page.evaluate(() => [...document.querySelectorAll(".st-rm-lane")].filter((lane) => lane.querySelector(".st-rm-lane-name")?.textContent === "Anwendungsfach").map((lane) => lane.querySelector(".st-rm-lane-lp")?.textContent));
  check(anwendungsfach.join() === "12", `the lanes of the Anwendungsfach: ${JSON.stringify(anwendungsfach)}`);
  // A placeholder put in leads into its module choice (owner, 2026-11-04: „wenn man einen
  // platzhalter slot erstellt auch gleich in die modulauswahl dafür geschmissen werden"): the
  // plan's open Anwendungsfach, a click, the workbench of its semester at its subgroup: on a
  // desktop the semester's card opened in the semesters' view, × back to the Gesamtplan.
  await step("the plan's open Anwendungsfach", () => page.click('.st-rm-bar.open[aria-label^="Anwendungsfach –"]'), () => document.querySelector(".st-card-sem.split .st-wb-half .st-wb-pt > b")?.textContent === "Anwendungsfach" && document.querySelector(".st-card-sem h2")?.textContent === "SoSe 2027" && document.querySelector(".study-note")?.textContent.includes("Anwendungsfach in SoSe 2027 vorgemerkt"));
  await step("Rückgängig, in its module choice", () => page.click(".study-note .plan-toast-undo"), (key) => !(localStorage.getItem(key) || "").split("\n").some((line) => line.startsWith("p\t") && line.split("\t")[2] === "2027S"), PLAN);
  await step("×, the Gesamtplan again", () => page.click(".st-wb-half .st-wb-done"), () => !document.querySelector(".st-wb-half") && document.querySelector(".st-rm-grid"));
  // Not all of it unfolded: the one button unfolds all first, then folds all.
  await step("Alle aufklappen, for the place", () => page.click(".st-rm-all"), () => document.querySelector(".st-rm-all")?.textContent.trim() === "Alle zuklappen");
  await step("Alle zuklappen, for the place", () => page.click(".st-rm-all"), () => !document.querySelector(".st-rm-lane"));
  // Its semester's head shows it as the semester (the workbench's × left SoSe 2027 in focus).
  await step("back to the semester", () => page.click('.st-col-head[data-sem="2026W"]'), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
  // „Modul wählen", in its menu and at the row, opens the workbench at the place's subgroup
  // (folia/design/hinzufuegen/konzepte.html, third round: „„Modul wählen“ an einem Platzhalter
  // öffnet die Werkbank in seiner Untergruppe"), whose „Im Katalog suchen" names the place (`fill=`).
  const atAnwendungsfach = () => document.querySelector(".st-wb-half .st-wb-pt > b")?.textContent === "Anwendungsfach" && document.querySelector(".st-wb-half")?.getAttribute("aria-label") === "Hinzufügen zu WiSe 2026/27";
  await step("the place's menu", () => page.click(".st-card-sem .st-row .st-status"), () => document.querySelector(".st-pop.root #st-menu-view"));
  await step("its „Modul wählen“", () => page.click(".st-pop.root #st-menu-view"), atAnwendungsfach);
  await step("×, from the menu's way", () => page.click(".st-wb-half .st-wb-done"), () => !document.querySelector(".st-wb-half") && document.querySelector(".st-card-sem .st-row .st-find"));
  await step("Modul wählen", () => page.click(".st-card-sem .st-row .st-find"), atAnwendungsfach);
  const find = await page.getAttribute(".st-wb-half .st-wb-foot a.st-wb-a.way", "href");
  check(find?.endsWith(`&fill=p${pid}`), `„Im Katalog suchen“ does not name the place: ${find}`);
  await step("Im Katalog suchen", () => page.click(".st-wb-half .st-wb-foot a.st-wb-a.way"), (pid) => location.pathname === "/catalog" && new URLSearchParams(location.search).get("fill") === `p${pid}` && document.querySelector(".bench-bar")?.textContent.includes("für Anwendungsfach") && document.querySelector('.row[data-id="12160"]'), pid);
  // „Einplanen" aims at the place and its semester, and plans the module to fill it.
  const SWITCH = '.detail .hero .plan-toggle[data-action="plan"]';
  await step("Allgemeine BWL I beside the list", () => page.click('.row[data-id="12160"]'), (selector) => document.querySelector(selector)?.checkVisibility(), SWITCH);
  const aim = await page.evaluate((selector) => document.querySelector(selector)?.textContent.replace(/\s+/g, " "), SWITCH);
  check(aim?.includes("WiSe 26/27") && aim.includes("für „Anwendungsfach“"), `„Einplanen“ does not aim at the place: ${aim}`);
  await step("Einplanen, for the place", () => page.click(SWITCH), ([key, pid]) => (localStorage.getItem(key) || "").split("\n").some((line) => line.startsWith("m\t2026W\t12160\t") && line.split("\t")[4] === pid), [PLAN, pid]);
  // Back in „Mein Studium" („Zurück zu WiSe 2026/27", then ×): one module of 6 LP for the
  // place, counted once (it was 12 and 12/18: folia/design/hinzufuegen/konzepte.html, „Was heute
  // schiefgeht"); the place, full, stands no more, and the module carries its name (owner,
  // 2026-11-09: „wenn man ein anwendungsfach eplant hat, dann muss man den platzhalter nicht mehr
  // sehen"; until Folia 1.87.0 it stood beside it, „gefüllt mit …").
  await step("Zurück zu WiSe 2026/27", () => page.click(".bench-bar a"), () => location.pathname === "/study" && document.querySelector(".st-wb-half .st-wb-row.is-done")?.textContent.includes("Allgemeine Betriebswirtschaftslehre I"));
  // Beside the open half the module says once what it fills: its flag, not the half's „für …" too.
  const once = await page.evaluate(() => {
    const row = document.querySelector('.st-card-sem.split .st-row[data-key="m:12160"]');
    return row && { flag: [...row.querySelectorAll(".flag")].some((flag) => flag.textContent === "für „Anwendungsfach“"), also: Boolean(row.querySelector(".st-for")) };
  });
  check(once?.flag && !once.also, `the module beside the half says twice what it fills: ${JSON.stringify(once)}`);
  await step("the semester, its place filled", () => page.click(".st-wb-half .st-wb-done"), () => !document.querySelector(".st-wb-half") && document.querySelectorAll(".st-card-sem .st-row").length === 1);
  const filled = await counts();
  check(filled.semester === "6 LP" && filled.nebenfach === "6/18", `one module for the place counts twice: ${JSON.stringify(filled)}`);
  const placed = await rows(page);
  check(!placed.some((row) => row.name === "Anwendungsfach") && placed.some((row) => row.name?.startsWith("Allgemeine Betriebswirtschaftslehre I") && row.chips.includes("für „Anwendungsfach“")), `the place filled still stands, or its module does not carry it: ${JSON.stringify(placed)}`);
  check(!(await page.$(".st-card-sem .st-find")), "a filled place still offers „Modul wählen“");
  // The module out again, the place open; the Wahlpflicht's module, beside the page, fills it as
  // well.
  await step("the module's menu, at the place", () => page.click('.st-card-sem .st-row[data-key="m:12160"] .st-status'), () => document.querySelector(".st-pop.root #st-menu-remove"));
  await step("the module taken out", () => page.click("#st-menu-remove"), () => document.querySelectorAll(".st-card-sem .st-row").length === 1 && document.querySelector(".st-card-sem .st-find"));
  await step("the Wahlpflicht, for a place", () => page.click('#sidebar .st-vl:has-text("Wahlpflicht")'), () => location.search === "?view=areas" && document.querySelector(".st-sl"));
  await step("Anwendungsfach, its lists", () => page.click('.st-sl:has-text("Anwendungsfach")'), () => document.querySelector('.st-sl[aria-current="true"]')?.textContent.includes("Anwendungsfach") && document.querySelector(".st-wrows .st-wrow:not(.has)"));
  const chosen = await page.evaluate(() => {
    const row = document.querySelector(".st-wrows .st-wrow:not(.has)");
    return { id: row?.querySelector(".st-wrow-id")?.textContent, credits: row?.querySelector(".st-wrow-lp")?.textContent.replace(/\s*LP$/, "") };
  });
  // The catalog stays built behind its tab: the switch beside „Mein Studium" is the one shown. Passed
  // as a function, not as text made a function in the page, which its Content-Security-Policy
  // refuses (no 'unsafe-eval').
  const shown = (selector) => [...document.querySelectorAll(selector)].find((el) => el.checkVisibility());
  await step("the Wahlpflicht's module beside the page", () => page.click(".st-wrows .st-wrow:not(.has) >> nth=0"), shown, SWITCH);
  const forPlace = await page.evaluate((selector) => [...document.querySelectorAll(selector)].find((el) => el.checkVisibility())?.textContent.replace(/\s+/g, " "), SWITCH);
  check(forPlace?.includes("WiSe 26/27") && forPlace.includes("für „Anwendungsfach“"), `the Wahlpflicht's module does not aim at the place: ${forPlace}`);
  await step("Einplanen, for the Wahlpflicht's place", () => page.click(`${SWITCH} >> visible=true`), ([key, id, pid]) => (localStorage.getItem(key) || "").split("\n").some((line) => line.startsWith(`m\t2026W\t${id}\t`) && line.split("\t")[4] === pid), [PLAN, chosen.id, pid]);
  await page.waitForFunction((credits) => [...document.querySelectorAll(".st-sl-h")].find((head) => head.textContent.includes("Nebenfach"))?.querySelector("small")?.textContent === `${credits} / 18\u00a0LP`, chosen.credits, { timeout: 4000 })
    .catch(async () => problems.push(`the Wahlpflicht's module counts beside its place: Nebenfach ${await page.evaluate(() => [...document.querySelectorAll(".st-sl-h")].map((head) => head.textContent).join(" | "))}, the module ${chosen.credits} LP`));
  await context.close();
}

// ---------- 2c: the Gesamtplan reworked (owner, 2026-11-09) ----------
{
  const { page, step, seed, context } = await open({ viewport: { width: 1600, height: 1000 }, acceptDownloads: true }, "/");
  // SoSe 2027 as in 3 (Lineare Algebra und analytische Geometrie II, 8 LP, for Praktische
  // Mathematik's 6; Analysis II for the Anwendungsfach), three modules of the Komplex Praktische
  // Informatik in WiSe 2027/28 and two of Grundlagen der Informatik in SoSe 2028.
  const KOMPLEX = ["14012", "13569", "12330"].map((id, i) => `m\t2027W\t${id}\t${i + 1}\t\t`).concat(["12329", "12350"].map((id, i) => `m\t2028S\t${id}\t${i + 1}\t\t`)).join("\n") + "\n";
  await seed([[MINE, THIRD], [PLAN, FILLED + LATER + KOMPLEX], ["betula.study.view", "grid"]]);
  await step("the Gesamtplan", () => page.click('.rail a[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-rm-grid .st-rm-p"));
  // Subsections (owner: „subsections für bereiche … Orientiere dich daran, wie die wp bereiche das
  // darstellen mit den requirements, also min 10lp max 24lp mit einem der bereiche mit 30 erlaubten
  // lp"): the Fachstudium unfolds into its Komplexe, each with what it holds against its 10–24 and
  // the Wahlpflicht's bar (its least marked, the room to 30 hatched), and the Seminar; its
  // „Wahlpflicht: Komplex …" is none of them. The Nebenfach into its two choices: Praktische
  // Mathematik holds 8 of its 6, which is no „über Bedarf" (owner: „weil ein modul 8 statt 6 lp
  // bringt. Das darf dann aber nicht als zu viel angezeigt werden").
  await step("Fachstudium's head", () => page.click('.st-row-head:has(.st-card-name:text-is("Fachstudium")) .st-card-name'), () => document.querySelectorAll(".st-rm-sub").length === 4);
  await step("Nebenfach's head", () => page.click('.st-row-head:has(.st-card-name:text-is("Nebenfach")) .st-card-name'), () => document.querySelectorAll(".st-rm-sub").length === 6);
  const subs = await page.evaluate(() => [...document.querySelectorAll(".st-rm-sub")].map((head) => ({
    name: head.querySelector(".st-card-name")?.textContent,
    figure: head.querySelector(".st-card-lp")?.textContent.trim(),
    least: !!head.querySelector(".st-wbar > .lt"),
    room: !!head.querySelector(".st-wbar > .zn"),
  })));
  const said = subs.map((sub) => `${sub.name} ${sub.figure}`).join(" | ");
  check(said === "Anwendungsfach 8/12 | Praktische Mathematik 8/6 | Grundlagen der Informatik 16/10–24 | Praktische Informatik 18/10–24 | Angewandte und Technische Informatik 0/10–24 | Seminar oder Praktikum 0/4", `the subsections: ${said}`);
  check(subs.slice(2, 5).every((sub) => sub.least && sub.room), `the Komplexe's bars: ${JSON.stringify(subs.slice(2, 5))}`);
  const over = await page.evaluate(() => [...document.querySelectorAll(".st-rm-p.over")].map((piece) => piece.getAttribute("aria-label")));
  check(over.length === 0, `8 LP where 6 were asked marked as beyond need: ${JSON.stringify(over)}`);
  // A subsection unfolds into its lanes.
  await step("Praktische Informatik's head", () => page.click('.st-rm-sub:has(.st-card-name:text-is("Praktische Informatik")) .st-rm-area'), () => [...document.querySelectorAll(".st-rm-lane.deep .st-rm-lane-name")].map((lane) => lane.textContent).includes("Datenbanken"));
  // A module dragged onto another subsection whose list holds it counts there, with „Rückgängig":
  // Lineare Algebra II from Praktische Mathematik to the Anwendungsfach, in its semester.
  const drag = async (from, to, ctrl) => {
    const a = await page.locator(from).first().boundingBox();
    const b = await page.locator(to).first().boundingBox();
    if (!a || !b) return problems.push(`nothing to drag from ${from} to ${to}`);
    await page.mouse.move(a.x + 10, a.y + a.height / 2);
    await page.mouse.down();
    await page.mouse.move(a.x + 30, a.y + a.height / 2, { steps: 3 });
    if (ctrl) await page.keyboard.down("Control");
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 10 });
    await page.waitForTimeout(150);
  };
  const row = (name) => page.evaluate((name) => [...document.querySelectorAll(".st-rm-sub")].find((head) => head.querySelector(".st-card-name")?.textContent === name)?.nextElementSibling?.dataset.row, name);
  const [anwendung, pm, seminar, praktisch] = [await row("Anwendungsfach"), await row("Praktische Mathematik"), await row("Seminar oder Praktikum"), await row("Praktische Informatik")];
  await drag(`[data-row="${pm}"] .st-rm-p[aria-label^="LAAG2,"]`, `[data-sem="2027S"][data-row="${anwendung}"]`, false);
  check(await page.evaluate((row) => document.querySelector(`.rm-drop-here[data-sem="2027S"][data-row="${row}"]`) !== null, anwendung), "the Anwendungsfach's cell is not where Lineare Algebra II lands");
  await step("let go over the Anwendungsfach", () => page.mouse.up(), () => document.querySelector(".study-note")?.textContent.includes("zählt für Anwendungsfach"));
  check((await lines(page, "m\t2027S\t11102\t"))[0]?.split("\t").length > 6, `Lineare Algebra II does not count for the Anwendungsfach: ${JSON.stringify(await lines(page, "m\t2027S\t11102\t"))}`);
  await step("Rückgängig, Praktische Mathematik's again", () => undo(page), () => !document.querySelector(".study-note"));
  // Onto the Seminar, whose list does not hold it, Datenbanken does not count: the row says what
  // Strg does, and the module moves in its own; with Strg it counts there as an exception.
  await drag(`[data-row="${praktisch}"] .st-rm-p[aria-label^="DB,"]`, `[data-sem="2028S"][data-row="${seminar}"]`, false);
  const refused = await page.evaluate((row) => ({ hint: document.querySelector(`[data-sem="2028S"][data-row="${row}"] .st-rm-hint`)?.textContent, no: !!document.querySelector(`.rm-drop-no[data-sem="2028S"][data-row="${row}"]`) }), seminar);
  check(refused.no && refused.hint === "Strg: als Ausnahme", `the Seminar's row under Datenbanken: ${JSON.stringify(refused)}`);
  await step("let go over the Seminar", () => page.mouse.up(), () => document.querySelector(".study-note")?.textContent.includes("ins SoSe 2028 verschoben"));
  check((await lines(page, "m\t2028S\t12330\t"))[0]?.split("\t").length <= 6, "Datenbanken counts for the Seminar without Strg");
  await step("Rückgängig, Datenbanken back", () => undo(page), () => !document.querySelector(".study-note"));
  await page.waitForFunction(() => document.querySelector('.st-rm-p[aria-label^="DB,"]'), null, { timeout: 4000 }).catch(() => {});
  await drag(`[data-row="${praktisch}"] .st-rm-p[aria-label^="DB,"]`, `[data-sem="2027W"][data-row="${seminar}"]`, true);
  await step("with Strg, as an exception", async () => { await page.mouse.up(); await page.keyboard.up("Control"); }, () => document.querySelector(".study-note")?.textContent.includes("zählt als Ausnahme für Seminar oder Praktikum"));
  await step("Rückgängig, the exception", () => undo(page), () => !document.querySelector(".study-note"));
  // „Vollbild" at the head's right end (owner: „einen Fullscreen knopf oben rechts haben mit dem
  // klassischen … dass die nav bar noch funktioniert und oben auch noch die suchleiste ist"): the
  // plan fills the page, the sidebar and the overview aside, the rail and the search there; Esc
  // ends it.
  const corner = await page.evaluate(() => {
    const head = document.querySelector(".st-rm-top .st-sems-head").getBoundingClientRect();
    const button = document.querySelector(".st-rm-full").getBoundingClientRect();
    return Math.round(head.right - button.right);
  });
  check(corner <= 8, `„Vollbild“ is not at the head's right end: ${corner}`);
  await step("Vollbild", () => page.click(".st-rm-full"), () => document.querySelector(".st-grid-panel.is-full") && !document.querySelector(".st-over") && getComputedStyle(document.getElementById("sidebar")).display === "none");
  const filled = await page.evaluate(() => {
    const panel = document.querySelector(".st-grid-panel").getBoundingClientRect();
    const rail = document.querySelector(".rail")?.getBoundingClientRect();
    const search = document.querySelector('.topbar input[type="search"], .search input')?.getBoundingClientRect();
    return { left: Math.round(panel.left - (rail?.right ?? 0)), wide: Math.round(panel.width), rail: (rail?.width ?? 0) > 0, search: (search?.width ?? 0) > 0, label: document.querySelector(".st-rm-full")?.getAttribute("aria-label") };
  });
  check(filled.left <= 16 && filled.wide > 1400 && filled.rail && filled.search && filled.label === "Vollbild beenden", `the plan in full screen: ${JSON.stringify(filled)}`);
  await step("Esc, out of it", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-grid-panel.is-full") && document.querySelector(".st-over") && getComputedStyle(document.getElementById("sidebar")).display !== "none");
  // „Als Bild speichern": the plan as it is shown, a PNG of the day.
  const [download] = await Promise.all([page.waitForEvent("download", { timeout: 8000 }).catch(() => null), page.click(".st-rm-save")]);
  const name = download?.suggestedFilename() ?? "";
  const bytes = download ? await download.createReadStream().then((stream) => new Promise((done) => { const parts = []; stream.on("data", (part) => parts.push(part)); stream.on("end", () => done(Buffer.concat(parts))); })) : Buffer.alloc(0);
  check(/^gesamtplan-\d{4}-\d\d-\d\d\.png$/.test(name) && bytes.subarray(1, 4).toString() === "PNG" && bytes.length > 20000, `„Als Bild speichern“: ${name}, ${bytes.length} bytes`);
  await context.close();
}

// ---------- 2d: a place for modules, its size, the weighting of the Komplexe ----------
{
  const { page, step, seed, context } = await open({ viewport: { width: 1440, height: 900 } }, "/");
  // The Anwendungsfach of WiSe 2026/27 set to 12 LP, Allgemeine BWL I (6 LP) in it; a place of the
  // Komplex Praktische Informatik in WiSe 2027/28.
  const PLACES = "p\t9\t2026W\t079-82-2008\t15\t3-3\t6\telective\t\tAnwendungsfach\t12\nm\t2026W\t12160\t2\t9\t\n"
    + "p\t8\t2027W\t079-82-2008\t20\t5-6\t10–24\telective\t\tKomplex Praktische Informatik\n";
  await seed([[MINE, THIRD], [PLAN, FILLED + PLACES]]);
  await step("Studium, for a place in part", () => page.click('.rail a[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector('.st-card-sem .st-row[data-key="r::15"] .st-ph-fill'));
  const part = await page.evaluate(() => ({
    fill: document.querySelector('.st-card-sem .st-row[data-key="r::15"] .st-ph-fill small')?.textContent.replace(/\u00a0/g, " "),
    lp: document.querySelector('.st-card-sem .st-row[data-key="r::15"] .lp')?.textContent,
    find: !!document.querySelector('.st-card-sem .st-row[data-key="r::15"] .st-find'),
    semester: document.querySelector(".st-card-sem .st-sem-sub b")?.textContent.replace(/\u00a0/g, " "),
  }));
  check(part.fill === "6 von 12 LP eingeplant" && part.lp === "12LP" && part.find && part.semester === "12 LP", `a place filled in part: ${JSON.stringify(part)}`);
  // „Umfang …" in its menu: 12, turned down to what the row says, 6.
  await step("the place's menu, Umfang", () => page.click('.st-card-sem .st-row[data-key="r::15"] .st-status'), () => document.querySelector(".st-pop.root #st-menu-size"));
  await step("Umfang …", () => page.click(".st-pop.root #st-menu-size"), () => document.querySelector("#st-sz-value")?.value === "12" && !document.querySelector(".st-sz-reset").hidden && !document.querySelector(".st-sz-w"));
  await step("Wie im Plan", () => page.click(".st-sz-reset"), () => document.querySelector("#st-sz-value")?.value === "6" && document.querySelector(".st-sz-reset").hidden);
  await step("Übernehmen: the place full", () => page.click("#st-sz-go"), (key) => !document.querySelector('.st-card-sem .st-row[data-key="r::15"]')
    && document.querySelector(".study-note")?.textContent.includes("Anwendungsfach im WiSe 2026/27 wieder wie im Plan")
    && (localStorage.getItem(key) || "").split("\n").some((line) => line.startsWith("p\t9\t") && line.endsWith("\tAnwendungsfach")), PLAN);
  await step("Rückgängig, the place in part again", () => undo(page), () => document.querySelector('.st-card-sem .st-row[data-key="r::15"] .st-ph-fill'));
  // The module taken out: the place stands as it was, nothing in it.
  await step("the module's menu", () => page.click('.st-card-sem .st-row[data-key="m:12160"] .st-status'), () => document.querySelector(".st-pop.root #st-menu-remove"));
  await step("the module taken out", () => page.click("#st-menu-remove"), () => !document.querySelector('.st-card-sem .st-row[data-key="m:12160"]') && document.querySelector('.st-card-sem .st-row[data-key="r::15"] .st-find') && !document.querySelector(".st-ph-fill"));
  // The Komplex Praktische Informatik of WiSe 2027/28: its size among the three Komplexe.
  await pick(page, "WiSe 27/28");
  await step("WiSe 2027/28, its Komplex", () => page.click('.st-card-sem .st-row[data-key="r::20"] .st-status'), () => document.querySelector(".st-pop.root #st-menu-size"));
  await step("Umfang of the Komplex", () => page.click(".st-pop.root #st-menu-size"), () => document.querySelectorAll(".st-sz-m").length === 3 && document.querySelector(".st-sz-sum b")?.textContent.replace(/\u00a0/g, " ") === "Zusammen 10 von 50 LP");
  const weights = await page.evaluate(() => [...document.querySelectorAll(".st-sz-m")].map((member) => `${member.querySelector("b")?.textContent}${member.classList.contains("is-own") ? "*" : ""}`).join(", "));
  check(weights === "Grundlagen der Informatik, Praktische Informatik*, Angewandte und Technische Informatik", `the Komplexe weighed: ${weights}`);
  for (let i = 0; i < 4; i += 1) await page.click("#st-sz-up");
  await page.waitForFunction(() => document.querySelector(".st-sz-sum b")?.textContent.replace(/\u00a0/g, " ") === "Zusammen 14 von 50 LP", null, { timeout: 4000 }).catch(() => problems.push("the weighting does not follow the stepper"));
  await step("Übernehmen, 14 LP", () => page.click("#st-sz-go"), (key) => document.querySelector('.st-card-sem .st-row[data-key="r::20"] .lp')?.textContent === "14LP"
    && (localStorage.getItem(key) || "").split("\n").some((line) => line.startsWith("p\t8\t") && line.endsWith("\tKomplex Praktische Informatik\t14")), PLAN);
  await context.close();
}

// ---------- 3: a phone ----------
{
  const { page, step, seed, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 }, "/");
  // SoSe 2027 longer than the screen, to be scrolled to its end and swiped from there (below).
  await seed([[MINE, THIRD], [PLAN, FILLED + LATER]]);
  await step("the bottom bar", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-planbox"));
  const landing = await page.evaluate(() => ({
    sideways: document.documentElement.scrollWidth - innerWidth,
    parts: [...document.querySelectorAll(".st-phone-page > *")].map((part) => [...part.classList].find((name) => name !== "panel")),
    legend: [...document.querySelectorAll(".st-progress .st-legend-n li")].map((li) => li.textContent.replace(/[\d,]+/g, "").trim()),
    mine: [...document.querySelectorAll(".st-phone-page #st-mine > span:not(.visually-hidden)")].map((part) => part.textContent.replace(/\s+/g, " ").trim()).join(" | "),
    ways: [...document.querySelectorAll(".st-mine-panel .st-ways a")].map((a) => a.querySelector("span")?.childNodes[0]?.textContent.trim()),
    columns: [...document.querySelectorAll(".st-chart .st-col")].map((col) => col.textContent.trim() + (col.classList.contains("is-now") ? "*" : "")).join(" "),
    line: document.querySelector(".st-planbox-line")?.textContent.replace(/\u00a0/g, " "),
    rest: [...document.querySelectorAll(".st-rest a")].map((a) => a.querySelector("span")?.childNodes[0]?.textContent.trim()),
    sidebar: document.querySelector("#sidebar #st-mine") !== null || document.querySelector(".sheet-toggle") !== null,
    // One row of pages (owner, 2026-10-06: „die landing page für mein Studium mit auf den pager"):
    // the two semesters that are over, the overview (O) in view and the only one to be used (!),
    // the current one and the ones to come, the page that adds one; its dots, the overview's in
    // view; no way back at the head.
    pages: [...document.querySelectorAll(".st-pager > .st-slot")].map((slot) => (slot.classList.contains("is-overview") ? "O" : "S") + (slot.hasAttribute("data-here") && !slot.inert ? "!" : "") + (slot.inert ? "" : "+")).join(" "),
    dots: [...document.querySelectorAll(".st-dock .st-dots span")].map((dot) => (dot.classList.contains("overview") ? "O" : "S") + (dot.classList.contains("here") ? "!" : "") + (dot.classList.contains("now") ? "*" : "")).join(" "),
    back: (() => { const back = document.querySelector(".top-back"); return back ? `${getComputedStyle(back).opacity}${back.inert ? " inert" : ""}` : "none"; })(),
  }));
  check(landing.sideways <= 0 && landing.parts.join() === "st-over,st-mine-panel,st-planbox,st-rest" && landing.legend.join() === "bestanden,geplant,offen"
    && landing.mine === "Informatik B.Sc. | PO 2008 | 3. Fachsemester · seit WiSe 25/26" && landing.ways.join() === "Regelstudienplan,Wahlpflicht"
    && landing.columns === "1 2 3* 4 5 6" && landing.line === "Jetzt WiSe 2026/27 · 0 von 30 LP" && landing.rest.join() === "Alle Studiengänge" && !landing.sidebar
    && landing.pages === "S S O!+ S S S S S" && landing.dots === "S S O! S* S S S S" && landing.back === "0 inert", `the phone's overview: ${JSON.stringify(landing)}`);
  // No ground under the row, the overview's page included (owner, 2026-10-19: „Ich würde den footer
  // bei den semester ansichten einfach raus nehmen. Der hat da eh keinen Mehrwert"); every other
  // page keeps it.
  const grounded = () => page.evaluate(() => getComputedStyle(document.querySelector("body > .ground")).display !== "none");
  check(!(await grounded()), "the phone's overview has the ground under its row");
  // A finger on the screen, as a phone's touch comes (through the browser's own input, so that the
  // browser scrolls what it scrolls): down near the side of `selector` it comes from, drawn sideways
  // by `by` px in `steps` moves 16 ms apart, held `hold` ms, lifted. How far the row `row` moved
  // under it before it went.
  const cdp = await context.newCDPSession(page);
  const touch = (type, x, y) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x: Math.round(x), y: Math.round(y), id: 1 }] });
  const swipe = async (selector, by, { steps = 10, hold = 0, row = selector } = {}) => {
    const box = await page.$eval(selector, (el) => { const r = el.getBoundingClientRect(); return { left: r.left, right: r.right, top: r.top, height: r.height }; });
    // On the part of it that shows, under the bar at the top.
    const y = Math.min(Math.max(box.top + Math.min(120, box.height / 2), 160), 600);
    const from = by < 0 ? box.right - 30 : box.left + 30;
    const start = await page.$eval(row, (el) => el.scrollLeft);
    await touch("touchStart", from, y);
    for (let i = 1; i <= steps; i++) {
      await page.waitForTimeout(16);
      await touch("touchMove", from + (by * i) / steps, y);
    }
    if (hold) await page.waitForTimeout(hold);
    const moved = (await page.$eval(row, (el) => el.scrollLeft)) - start;
    await touch("touchEnd", 0, 0);
    return moved;
  };
  // How long a tap's glide takes (owner, 2026-10-06: „die Animationen müssen more snappy sein"): the
  // row's place each frame from `watch()` on (where its first page is drawn: a glide draws the
  // pages of a row that does not scroll meanwhile), and the time from its first move to where it is
  // within a px of where it came to rest.
  const watch = () => page.evaluate(() => {
    window.__xs = [];
    const row = document.querySelector(".st-pager");
    const from = performance.now();
    const tick = (t) => { window.__xs.push([t, -row.firstElementChild.getBoundingClientRect().left]); if (t - from < 1500) requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
  });
  const glided = () => page.evaluate(async () => {
    await new Promise((resolve) => setTimeout(resolve, 600));
    const xs = window.__xs;
    const rest = xs[xs.length - 1][1];
    const begin = xs.find(([, x]) => x !== xs[0][1])?.[0];
    const end = xs.find(([t, x]) => t >= begin && Math.abs(x - rest) < 1)?.[0];
    return begin === undefined || end === undefined ? -1 : Math.round(end - begin);
  });
  // The row rests at the semester `name` (`null`: the overview): its page in view, the only one to
  // be used, the row exactly at it (a step is a page's place, the page and its share of the space);
  // the address is the overview's, and its entry keeps the semester (none at the overview).
  const resting = (name) => new Function(`
    const row = document.querySelector(".st-pager");
    const here = document.querySelector(".st-pager > .st-slot[data-here]");
    if (!row || !here || here.inert || document.querySelectorAll(".st-pager > .st-slot:not([inert])").length !== 1) return false;
    const at = [...row.children].indexOf(here) * here.getBoundingClientRect().width;
    if (Math.abs(row.scrollLeft - at) >= 2) return false;
    return location.search === "" && ${name === null ? `here.classList.contains("is-overview") && history.state?.["betula.semester"] === undefined` : `here.querySelector("h2")?.textContent === ${JSON.stringify(name)} && history.state?.["betula.semester"] !== undefined`};
  `);
  const atOverview = resting(null);
  const HERE = ".st-pager > .st-slot:not([inert])";
  await watch();
  await step("Studium planen", () => page.tap(".st-planbox-head"), () => history.state?.["betula.semester"] === "2026W" && document.querySelector(".st-pager .st-card-sem"));
  let glide = await glided();
  check(glide > 60 && glide <= 280, `the box's way to the current semester is no quick glide: ${glide} ms`);
  await page.waitForFunction(resting("WiSe 2026/27"), null, { timeout: 4000 }).catch(() => problems.push("the semesters do not open at the current one"));
  // The way back has slid in.
  await page.waitForFunction(() => document.querySelector(".top-back.on")?.getAnimations().length === 0, null, { timeout: 3000 }).catch(() => problems.push("the way back never comes to rest"));
  const phone = await page.evaluate((HERE) => ({
    sideways: document.documentElement.scrollWidth - innerWidth,
    card: Math.round(document.querySelector(`${HERE} .st-card-sem`).getBoundingClientRect().width),
    slots: document.querySelectorAll(".st-pager > .st-slot").length,
    snap: getComputedStyle(document.querySelector(".st-pager")).scrollSnapType,
    strip: document.querySelector(".st-strip")?.getBoundingClientRect().height ?? 0,
    turns: Math.min(...[...document.querySelectorAll(`${HERE} .st-turn`)].map((button) => button.getBoundingClientRect().height)),
    grid: document.querySelector(".st-views")?.getBoundingClientRect().height ?? 0,
    // The way back: a square box as the search is, left of it at the page's margin (owner,
    // 2026-10-05: „oben links neben der search bar im gleichen style eine quadratische box").
    back: (() => {
      const back = document.querySelector(".top-back.on");
      if (!back || back.inert) return null;
      const box = back.getBoundingClientRect();
      const search = document.querySelector(".topbar .search").getBoundingClientRect();
      const looks = getComputedStyle(back).boxShadow === getComputedStyle(document.querySelector(".topbar .search")).boxShadow;
      return `${back.getAttribute("href")} ${Math.round(box.left)},${Math.round(box.width)}x${Math.round(box.height)} ${Math.round(search.left - box.right)} ${Math.round(search.height)} ${looks}`;
    })(),
    // No box around the semester, its head right under the search.
    box: (() => { const card = getComputedStyle(document.querySelector(`${HERE} .st-card-sem`)); return card.boxShadow === "none" && card.backgroundColor === "rgba(0, 0, 0, 0)"; })(),
    head: Math.round(document.querySelector(`${HERE} .st-sem-head h2`).getBoundingClientRect().top - document.querySelector(".topbar").getBoundingClientRect().bottom),
    ground: getComputedStyle(document.querySelector("body > .ground")).display,
    // The card in view from the screen's edges, and from the pages before (the overview) and after
    // it (owner, 2026-10-05: „dass die Abstände zwischen den boxen genau der Abstand zum rand ist").
    apart: (() => {
      const here = document.querySelector(".st-pager > .st-slot[data-here]");
      const card = here.firstElementChild.getBoundingClientRect();
      const before = here.previousElementSibling.firstElementChild.getBoundingClientRect();
      const after = here.nextElementSibling.firstElementChild.getBoundingClientRect();
      return [card.left, innerWidth - card.right, card.left - before.right, after.left - card.right].map(Math.round).join();
    })(),
  }), HERE);
  check(phone.sideways <= 0 && phone.card >= 360 && phone.slots === 8 && phone.snap.startsWith("x mandatory") && phone.strip === 0 && phone.turns >= 44 && phone.grid === 0
    && phone.back === "/study 12,48x48 8 48 true" && phone.box && phone.head >= 0 && phone.head <= 16 && phone.ground === "none" && phone.apart === "12,12,12,12", `the phone's semesters: ${JSON.stringify(phone)}`);
  // Over the tab bar, where they stay: the dots, the current semester's ringed and the card's
  // filled (owner, 2026-10-05: „fest über der nav bar"), in no box and with no legend, on the
  // page's ground frosted over what passes under it, from the screen's foot and edges to a little
  // above them (the same day: „einfach nur den grauen Hintergrund mit blur zum content").
  const dock = async () => page.evaluate(() => {
    const ground = document.querySelector(".st-dock");
    const style = getComputedStyle(ground);
    const box = ground.getBoundingClientRect();
    const nav = document.querySelector(".bottomnav").getBoundingClientRect();
    const row = ground.querySelector(".st-dots").getBoundingClientRect();
    const dots = [...ground.querySelectorAll(".st-dots span")];
    return {
      position: style.position,
      edges: [box.left, innerWidth - box.right, innerHeight - box.bottom].map(Math.round).join(),
      above: Math.round(nav.top - box.top),
      gap: Math.round(nav.top - row.bottom),
      frosted: style.backdropFilter.includes("blur") && style.boxShadow === "none" && style.borderRadius === "0px",
      legend: ground.querySelectorAll(".st-legend, .st-legend-marks, li").length,
      dots: dots.length, here: dots.findIndex((dot) => dot.classList.contains("here")), now: dots.findIndex((dot) => dot.classList.contains("now")), overview: dots.findIndex((dot) => dot.classList.contains("overview")),
    };
  });
  await page.waitForFunction(() => document.querySelector(".st-dock")?.getAnimations().length === 0, null, { timeout: 3000 }).catch(() => problems.push("the dots never stand still"));
  let docked = await dock();
  check(docked.position === "fixed" && docked.edges === "0,0,0" && docked.above >= 30 && docked.gap >= 4 && docked.gap <= 16 && docked.frosted && docked.legend === 0
    && docked.dots === 8 && docked.here === 3 && docked.now === 3 && docked.overview === 2, `the dots are not over the tab bar on the frosted ground: ${JSON.stringify(docked)}`);
  await page.evaluate(() => window.scrollBy(0, 200));
  await page.waitForTimeout(200);
  check(JSON.stringify(await dock()) === JSON.stringify(docked), "the dots move with the page");
  await page.evaluate(() => window.scrollTo(0, 0));
  // An empty semester: a mark and „Module hinzufügen" (owner, 2026-10-05: „keine Module sollte mit
  // icon und gleich modul hinzufügen sein"); nothing of what the plan has open under it (owner,
  // 2026-10-20: „Ja, auf dem Handy auch raus"): that is „Module hinzufügen"'s.
  const empty = await page.evaluate((HERE) => {
    const card = document.querySelector(`${HERE} .st-card-sem`);
    return {
      icon: Boolean(card.querySelector(".st-empty .st-empty-icon .icon")),
      words: card.querySelector(".st-empty p")?.textContent,
      add: card.querySelector(".st-empty .st-empty-add")?.textContent.trim(),
      addrow: card.querySelectorAll(".st-addrow").length,
      rows: card.querySelectorAll(".row").length,
      after: card.lastElementChild?.matches(".st-table"),
    };
  }, HERE);
  check(empty.icon && empty.words === "Noch keine Module" && empty.add === "Module hinzufügen" && empty.addrow === 0 && empty.rows === 0 && empty.after, `an empty semester on a phone: ${JSON.stringify(empty)}`);
  // „Module hinzufügen" puts the plan's third semester in; the note of the change stands over the
  // dots, and its „Rückgängig" takes it back.
  // The workbench on a phone (bench.rs `BenchSheet`; folia/design/hinzufuegen/handy.html, variant
  // B and its fifth round): a sheet out of „Module hinzufügen", half up under the semester, over
  // the tab bar, the page kept behind a veil that takes the touches (owner, 2026-11-04: „Das kann
  // einfach gesperrt sein, so lange der [offen] ist"); its head names the semester, the figure of
  // what it holds over the bar of it, and no button („der btw nur für verwirrung sorgt"). „Plan
  // übernehmen" puts the Pflicht into the semester over it and leaves the sheet, a part of the bar
  // more; „Rückgängig" takes it back; a choice goes into its list, the sheet all up; a module of
  // the list opens as its sheet over it; Esc at the root lets it go, the page as it was.
  await step("Module hinzufügen on a phone", () => page.tap(`${HERE} .st-empty-add`), () => document.querySelector(".st-wb-sheet[data-sheet] #st-wb-takeover") && !document.querySelector(".st-wb"));
  await page.waitForTimeout(800);
  const benchSheet = await page.evaluate((HERE) => {
    const box = document.querySelector(".st-wb-sheet").getBoundingClientRect();
    const tabs = document.querySelector(".bottomnav")?.getBoundingClientRect();
    const card = document.querySelector(`${HERE} .st-card-sem`)?.getBoundingClientRect();
    const onTabs = tabs ? document.elementFromPoint(tabs.left + tabs.width / 2, tabs.top + tabs.height / 2) : null;
    const onCard = card ? document.elementFromPoint(card.left + 20, card.top + 20) : null;
    return {
      top: Math.round((box.top / innerHeight) * 100),
      covers: Boolean(onTabs?.closest(".st-wb-sheet")),
      veiled: Boolean(onCard?.matches(".page-sheet-shade")),
      card: card ? Math.round((card.top / innerHeight) * 100) : -1,
      title: document.querySelector(".st-wb-sheet-h h2")?.textContent,
      figure: document.querySelector(".st-wb-sheet-h .st-wb-hfig")?.textContent,
      bar: document.querySelector(".st-wb-sheet-h .st-wb-hbar")?.getAttribute("aria-label"),
      pieces: document.querySelectorAll(".st-wb-hbar > :is(.planned, .passed, .place)").length,
      button: Boolean(document.querySelector(".st-wb-sheet-h button, .st-wb-sheet-h .btn")),
    };
  }, HERE);
  check(benchSheet.top >= 40 && benchSheet.top <= 60 && benchSheet.covers && benchSheet.veiled && benchSheet.card >= 0 && benchSheet.card < 40
    && benchSheet.title === "Hinzufügen zu WiSe 2026/27" && benchSheet.figure === "0 von 30 LP" && benchSheet.bar === "0 von 30 LP" && benchSheet.pieces === 0 && !benchSheet.button, `the workbench's sheet on a phone: ${JSON.stringify(benchSheet)}`);
  // A module of the sheet (the fifth round: „wir müssen an dem konzept arbeiten, wie module
  // dargestellt werden … unbalanciert"): its name and one line under it, its credits as the button
  // in the middle of the row's right end; a choice dashed, with its bar.
  const balanced = await page.evaluate(() => {
    const row = document.querySelector(".st-wb-sheet .st-wb-row:not(.choice):not(.place)");
    const plus = row?.querySelector(":scope > .st-wb-plus");
    const box = row?.getBoundingClientRect();
    const button = plus?.getBoundingClientRect();
    const choice = document.querySelector(".st-wb-sheet .st-wb-row.choice");
    return {
      words: plus?.textContent.trim(),
      middle: box && button ? Math.abs((box.top + box.bottom) / 2 - (button.top + button.bottom) / 2) : -1,
      right: box && button ? Math.round(box.right - button.right) : -1,
      lines: row ? row.querySelectorAll(".st-wb-go > small").length : -1,
      dashed: choice ? getComputedStyle(choice).outlineStyle : "",
      bar: Boolean(choice?.querySelector(".st-wb-fill")),
      figure: choice?.querySelector(".st-wb-fig")?.textContent,
    };
  });
  check(/^8\s*LP$/.test(balanced.words ?? "") && balanced.middle >= 0 && balanced.middle <= 2 && balanced.right <= 1 && balanced.lines === 1 && balanced.dashed === "dashed" && balanced.bar && /^\d+ \/ 12 LP$/.test(balanced.figure ?? ""), `a module and a choice in the sheet: ${JSON.stringify(balanced)}`);
  // What „Plan übernehmen" takes: the Pflicht of the 3. FS that stands nowhere yet (Softwarepraktikum
  // and Mathematik IT-3 are planned into SoSe 2027 here, and stay there).
  const taking = Number((await page.textContent("#st-wb-takeover")).match(/\d+/)?.[0] || 0);
  check(taking === 1, `„Plan übernehmen“ takes ${taking} modules, not Theoretische Informatik alone`);
  await step("Plan übernehmen on a phone", () => page.tap("#st-wb-takeover"), ([HERE, n]) => document.querySelectorAll(`${HERE} .st-row`).length === n && document.querySelector(".study-note") && document.querySelector(".st-wb-hfig")?.textContent === "8 von 30 LP" && document.querySelectorAll(".st-wb-hbar > .planned").length === 1 && !document.querySelector("#st-wb-takeover") && document.querySelector(".st-wb-sheet .st-wb-act .st-wb-fit.ok"), [HERE, taking]);
  check(!(await page.evaluate(() => [...document.querySelectorAll(".st-wb-sheet .st-wb-row .st-wb-go b")].some((name) => name.textContent === "Theoretische Informatik"))), "what is planned still stands in the sheet");
  // The note of the change waits for interaction (owner, 2026-11-04: „stelle den auch mal auf diese
  // wait notification um"): at the screen's bottom over the sheet, no timer.
  await page.waitForTimeout(400);
  const noted = await page.evaluate(() => {
    const note = document.querySelector(".study-note");
    const box = note?.getBoundingClientRect();
    const hit = box ? document.elementFromPoint(box.left + 30, box.top + box.height / 2) : null;
    return { over: Boolean(hit?.closest(".study-note")), bottom: box ? Math.round(innerHeight - box.bottom) : -1, text: note?.textContent ?? "" };
  });
  check(noted.over && noted.bottom >= 0 && noted.bottom <= 24 && noted.text.includes("Rückgängig"), `the note of „Plan übernehmen“ over the sheet: ${JSON.stringify(noted)}`);
  await page.waitForTimeout(1500);
  check(await page.evaluate(() => Boolean(document.querySelector(".study-note:not(.leaving)"))), "the note of a change goes by itself");
  await step("Rückgängig on a phone", () => page.tap(".study-note .plan-toast-undo"), (HERE) => document.querySelectorAll(`${HERE} .st-row`).length === 0 && document.querySelector(".st-wb-hfig")?.textContent === "0 von 30 LP" && !document.querySelector(".st-wb-hbar > .planned") && document.querySelector("#st-wb-takeover") && !document.querySelector(".study-note"), HERE);
  // A row with › goes into a page of the sheet (owner, 2026-11-04: „ein vertiefendes menu … mit >
  // in das quasi rein geht und das nicht ausklappt. Der modal soll offen bleiben. Oben einen Pfad
  // haben und auch einen back button"): the sheet stays, all up, its head as it was; over the page
  // ‹ and „Übersicht ›", the page's name under them; the page has the choice's switches and list.
  await step("a choice goes into its page", () => page.tap('.st-wb-sheet .st-wb-row.choice:has-text("Anwendungsfach") .st-wb-go'), () => document.querySelector(".st-wb-sheet .st-wb-path .st-wb-back") && document.querySelector(".st-wb-sheet .st-wb-page .st-wb-ctl") && document.querySelector(".st-wb-sheet .st-wb-page .st-wb-row a.st-wb-go") && !document.querySelector("#st-wb-takeover"));
  await page.waitForFunction(() => document.querySelector(".st-wb-sheet")?.classList.contains("is-full") && document.querySelector(".st-wb-sheet").getAnimations().length === 0, null, { timeout: 4000 }).catch(() => problems.push("the sheet does not go all up for a page of it"));
  const into = await page.evaluate(() => ({
    crumbs: [...document.querySelectorAll(".st-wb-crumbs > li")].map((crumb) => crumb.textContent).join(" › "),
    name: document.querySelector(".st-wb-pt > b")?.textContent,
    title: document.querySelector(".st-wb-sheet-h h2")?.textContent,
    way: document.querySelector(".st-wb-page")?.getAttribute("data-way"),
    figure: document.querySelector(".st-wb-pt > .st-wb-fig")?.textContent,
    bar: Boolean(document.querySelector(".st-wb-pth > .st-wb-fill")),
    chips: [...document.querySelectorAll(".st-wb-page .st-wb-chips > .st-wb-chip")].map((chip) => (chip.querySelector(":scope > span") ?? chip).textContent.trim()).join("|"),
    fach: document.querySelectorAll("#st-wb-fach > option").length,
  }));
  check(into.crumbs === "Übersicht" && into.name === "Anwendungsfach" && into.title === "Hinzufügen zu WiSe 2026/27" && into.way === "in" && /^\d+ \/ 12 LP$/.test(into.figure ?? "") && into.bar
    && into.chips === "Alle 5 Fächer|Im WiSe angeboten|Passt in meine Woche" && into.fach === 6, `a choice's page in the sheet: ${JSON.stringify(into)}`);
  // „Fach" is a chip over the phone's own list (a `<select>`): a Fach picked, its modules alone.
  const all = await page.evaluate(() => document.querySelectorAll(".st-wb-page .st-wb-row").length);
  await step("a Fach picked", () => page.selectOption("#st-wb-fach", { index: 1 }), (all) => document.querySelector(".st-wb-chip.pick.on") && document.querySelectorAll(".st-wb-page .st-wb-row").length < all && document.querySelectorAll(".st-wb-page .st-wb-row").length > 0, all);
  await step("all Fächer again", () => page.selectOption("#st-wb-fach", "all"), (all) => !document.querySelector(".st-wb-chip.pick.on") && document.querySelectorAll(".st-wb-page .st-wb-row").length === all, all);
  await step("a module of the list", () => page.tap(".st-wb-sheet .st-wb-page .st-wb-row a.st-wb-go >> nth=0"), () => document.querySelector(".detail.is-module h2") && document.querySelector(".st-wb-sheet"));
  const over = await page.evaluate(() => Number(getComputedStyle(document.querySelector(".detail.is-module")).zIndex) > Number(getComputedStyle(document.querySelector(".st-wb-sheet")).zIndex));
  check(over, "the module's sheet is not over the workbench's");
  await step("the module closed, the page of the sheet stays", () => page.keyboard.press("Escape"), () => !document.querySelector(".detail.is-module") && document.querySelector(".st-wb-sheet .st-wb-path") && document.querySelector(".st-wb-pt > b")?.textContent === "Anwendungsfach");
  // „Vormerken" at the foot of the list puts the placeholder into the semester behind the sheet.
  await step("Vormerken on a phone", () => page.tap('.st-wb-sheet .st-wb-foot button:has-text("Vormerken")'), (HERE) => document.querySelector(".study-note")?.textContent.includes("Anwendungsfach in WiSe 2026/27 vorgemerkt") && document.querySelector(`${HERE} .st-row.is-place`), HERE);
  await step("Esc a step back", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-wb-sheet .st-wb-path") && document.querySelector(".st-wb-sheet #st-wb-takeover") && document.querySelector(".st-wb-page")?.getAttribute("data-way") === "back");
  // „Alle Bereiche" is a page of the tree, unfolded (owner, 2026-11-04: „die container, dass die da
  // als ausgeklappter baum sind"): each area with its subgroups, the Fachstudium's „Wahlpflicht" of
  // the three Komplexe not among them („keinen kombinierten bereich aus mehreren wp dingen"); a
  // subgroup goes into its list at once; the path has each step, and a step of it is a way back.
  await step("Alle Bereiche goes into the tree", () => page.tap("#st-wb-areas"), () => document.querySelector(".st-wb-pt > b")?.textContent === "Alle Bereiche" && document.querySelectorAll(".st-wb-page .st-wb-ta").length === 6);
  const tree = await page.evaluate(() => Object.fromEntries([...document.querySelectorAll(".st-wb-page .st-wb-ta")].map((area) => [
    area.querySelector(".st-wb-th .st-wb-tn > span:not(.st-dot)")?.textContent,
    [...area.querySelectorAll(".st-wb-ts .st-wb-tn > span")].map((sub) => sub.textContent),
  ])));
  check(JSON.stringify(tree.Nebenfach) === JSON.stringify(["Anwendungsfach", "Praktische Mathematik"]) && tree.Fachstudium?.length === 4 && !tree.Fachstudium.some((name) => name.startsWith("Wahlpflicht")) && tree.Mathematik?.length === 0,
    `the tree of „Alle Bereiche“: ${JSON.stringify(tree)}`);
  await step("a subgroup into its list", () => page.tap('.st-wb-page .st-wb-ta .st-wb-ts:has-text("Praktische Mathematik")'), () => document.querySelectorAll(".st-wb-crumbs > li").length === 2 && document.querySelector(".st-wb-crumbs > li:nth-child(2)")?.textContent === "Alle Bereiche" && document.querySelector(".st-wb-pt > b")?.textContent.endsWith("Praktische Mathematik"));
  await step("„Übersicht“ of the path", () => page.tap(".st-wb-crumbs > li:first-child button"), () => !document.querySelector(".st-wb-sheet .st-wb-path") && document.querySelector(".st-wb-sheet #st-wb-takeover"));
  await step("Gemerkt goes into its page", () => page.tap("#st-wb-marked"), () => document.querySelector(".st-wb-pt > b")?.textContent === "Gemerkt");
  await step("‹ a step back", () => page.tap(".st-wb-back"), () => !document.querySelector(".st-wb-sheet .st-wb-path") && document.querySelector("#st-wb-marked"));
  await step("Esc at the root ends it, the placeholder in", () => page.keyboard.press("Escape"), (HERE) => !document.querySelector(".st-wb-sheet") && document.querySelector(`${HERE} .st-row.is-place`), HERE);
  // A placeholder stands apart from a module (owner, 2026-11-04: „Das design von platzhaltern muss
  // sich von dem der Module abheben"): its edge dashed, no card's shadow, „Modul wählen ›" under its
  // line; a tap on its box opens the sheet at its choice's list, all up.
  const placeRow = await page.evaluate((HERE) => {
    const row = document.querySelector(`${HERE} .st-row.is-place > .row`);
    const module = document.querySelector(`${HERE} .st-row:not(.is-place) > .row`);
    const style = row ? getComputedStyle(row) : null;
    return { edge: style?.outlineStyle, shadow: style?.boxShadow, ground: style?.backgroundColor, choose: row?.querySelector(".st-find")?.textContent, module: Boolean(module) };
  }, HERE);
  check(placeRow.edge === "dashed" && placeRow.shadow === "none" && placeRow.ground === "rgba(0, 0, 0, 0)" && placeRow.choose === "Modul wählen", `a placeholder on a phone: ${JSON.stringify(placeRow)}`);
  await step("a tap on the placeholder", async () => {
    const box = await page.locator(`${HERE} .st-row.is-place > .row`).boundingBox({ timeout: 4000 }).catch(() => null);
    if (box) await page.touchscreen.tap(box.x + box.width * 0.4, box.y + 14);
  }, () => document.querySelector(".st-wb-sheet[data-sheet-start='full'] .st-wb-path") && document.querySelector(".st-wb-pt > b")?.textContent === "Anwendungsfach" && document.querySelector(".st-wb-sheet .st-wb-page .st-wb-ctl"));
  await page.waitForFunction(() => document.querySelector(".st-wb-sheet")?.classList.contains("is-full") && document.querySelector(".st-wb-sheet").getAnimations().length === 0, null, { timeout: 4000 }).catch(() => problems.push("the sheet does not come all up at a placeholder's choice"));
  await step("Back to the root, then out", async () => { await page.keyboard.press("Escape"); await page.waitForTimeout(300); await page.keyboard.press("Escape"); }, () => !document.querySelector(".st-wb-sheet"));
  await step("the placeholder's menu", () => page.tap(`${HERE} .st-row.is-place .st-status`, { timeout: 4000 }).catch(() => {}), () => document.querySelector("#st-menu-remove"));
  await step("the placeholder taken out", () => page.tap("#st-menu-remove", { timeout: 4000 }).catch(() => {}), (HERE) => !document.querySelector(`${HERE} .st-row`), HERE);
  // ‹ scrolls the row to the semester before, smoothly, as a finger would: past the overview, which
  // lies between the current semester and the one before.
  await watch();
  await step("‹", () => page.tap(`${HERE} .st-turn >> nth=0`), resting("SoSe 2026"));
  const between = await page.evaluate(() => { const step = document.querySelector(".st-pager > .st-slot").getBoundingClientRect().width; return window.__xs.filter(([, x]) => x % step > 4 && x % step < step - 4).length; });
  check(between >= 3, `‹ turns the card without a glide: ${between} frames between two cards`);
  glide = await glided();
  check(glide > 60 && glide <= 320, `‹ past the overview is no quick glide: ${glide} ms for two pages`);
  check((await page.evaluate(() => history.state?.["betula.semester"])) === "2026S", "‹ past the overview stopped there");
  check((await dock()).here === 1, "the dots do not follow the card");
  // A semester's rows: the catalog's cards (owner, 2026-10-05: „die rows pro modul sollen ähnlich
  // aussehen"; 2026-10-19: „Gestalte das so, das der aufbau komplett gleich ist. Also padding,
  // margin, Titel, LP, und format von den Metadaten"): its name, under it its number and its area,
  // its credits in LP, as far from the card's edges as the catalog's, its status where the
  // catalog's card has the bookmark.
  const cards = await page.evaluate((HERE) => [...document.querySelectorAll(`${HERE} .st-row`)].map((row) => {
    const box = row.querySelector(":scope > .row");
    const style = getComputedStyle(box);
    const name = row.querySelector(".t > b").getBoundingClientRect();
    const meta = row.querySelector(".t > small").getBoundingClientRect();
    const card = box.getBoundingClientRect();
    const status = row.querySelector(":scope > .st-status").getBoundingClientRect();
    return {
      card: style.backgroundColor !== "rgba(0, 0, 0, 0)" && style.boxShadow !== "none" && parseFloat(style.borderTopLeftRadius) >= 8,
      place: row.classList.contains("is-place") && style.outlineStyle === "dashed" && style.boxShadow === "none",
      padding: style.padding,
      title: getComputedStyle(row.querySelector(".t > b")).fontSize,
      under: meta.top >= name.bottom - 1,
      line: [row.querySelector(".t small > .mono")?.textContent, row.querySelector(".t small > .kind")?.textContent].filter(Boolean).join(" "),
      lp: row.querySelector(".lp").textContent.trim(),
      status: [Math.round(card.right - status.right), Math.round(status.height)].join(),
    };
  }), HERE);
  check(cards.length === 4 && cards.every((card, at) => (at === 3 ? card.place && !card.card : card.card) && card.under && /LP$/.test(card.lp) && card.padding === "12px 52px 12px 18px" && card.title === "15.5px" && card.status === "1,44") && cards[0].line === "12101 Informatik" && cards[0].lp === "10LP", `a semester's rows are not the catalog's cards: ${JSON.stringify(cards)}`);
  // A long press selects a row: highlighted, no box to tick, the bar over the tab bar.
  await step("a long press", async () => {
    const at = await middle(page, `${HERE} .st-row >> nth=0 >> .lp`);
    const fire = (type) => page.evaluate(({ x, y, type, HERE }) => {
      const body = document.querySelector(`${HERE} .st-row .row`);
      body.dispatchEvent(new PointerEvent(type, { bubbles: true, pointerType: "touch", isPrimary: true, clientX: x, clientY: y }));
    }, { ...at, type, HERE });
    await fire("pointerdown");
    await page.waitForTimeout(650);
    await fire("pointerup");
  }, () => document.querySelector(".st-selbar") && document.querySelectorAll(".st-pager > .st-slot:not([inert]) .st-row.is-selected").length === 1);
  await page.waitForTimeout(400);
  const bar = await page.evaluate((HERE) => {
    const box = document.querySelector(".st-selbar").getBoundingClientRect();
    const nav = document.querySelector(".bottomnav").getBoundingClientRect();
    const top = document.elementFromPoint(nav.left + nav.width / 2, nav.top + nav.height / 2);
    return {
      fixed: getComputedStyle(document.querySelector(".st-selbar")).position,
      off: [box.left - nav.left, box.right - nav.right, box.top - nav.top, box.bottom - nav.bottom].map(Math.round).join(),
      covers: Boolean(top?.closest(".st-selbar")),
      boxes: document.querySelectorAll(`${HERE} .st-sel, ${HERE} .st-row .st-box`).length,
      lit: getComputedStyle(document.querySelector(`${HERE} .st-row.is-selected > .row`)).backgroundImage.includes("repeating-linear-gradient"),
      still: getComputedStyle(document.querySelector(".st-pager")).touchAction,
    };
  }, HERE);
  check(bar.fixed === "fixed" && bar.off === "0,0,0,0" && bar.covers && bar.boxes === 0 && bar.lit && bar.still === "pan-y", `the phone's bar is not over the tab bar: ${JSON.stringify(bar)}`);
  await step("a tap selects another", () => page.tap(`${HERE} .st-row >> nth=1 >> .row`), () => document.querySelectorAll(".st-pager > .st-slot:not([inert]) .st-row.is-selected").length === 2 && location.search === "");
  await step("let go on the phone", () => page.tap(".st-selbar .st-act-close"), () => !document.querySelector(".st-selbar"));
  // A row's menu: a sheet, without the module (a tap on the row opens it); „Verschieben nach"
  // takes its place, and gives it back.
  await step("a row's menu as a sheet", () => page.tap(`${HERE} .st-row >> nth=0 >> .st-status`), () => document.querySelector(".st-pop.root #st-menu-move"));
  await page.waitForTimeout(400);
  const menuSheet = await page.evaluate(() => { const box = document.querySelector(".st-pop.root").getBoundingClientRect(); return { bottom: Math.round(innerHeight - box.bottom), width: Math.round(box.width), entries: [...document.querySelectorAll(".st-pop.root [role^=menuitem]")].map((item) => item.id).join() }; });
  check(menuSheet.bottom === 0 && menuSheet.width === 390 && menuSheet.entries === "st-menu-passed,st-menu-move,st-menu-visit,st-menu-select,st-menu-remove", `a row's menu on a phone: ${JSON.stringify(menuSheet)}`);
  await step("Verschieben nach on a phone", () => page.tap("#st-menu-move"), () => document.querySelector(".st-pop.sub .st-pop-back") && getComputedStyle(document.querySelector(".st-pop.root")).display === "none");
  await step("back to the menu", () => page.tap(".st-pop.sub .st-pop-back"), () => !document.querySelector(".st-pop.sub") && getComputedStyle(document.querySelector(".st-pop.root")).display !== "none");
  await step("close the menu", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-pop"));
  // A finger to the left on the last semester that is over brings the overview, back through the
  // history (the semesters were come to from it).
  let moved = 0;
  let entries = await page.evaluate(() => history.length);
  await step("a semester that is over drawn to the left", async () => { moved = await swipe(".st-pager", -220); }, atOverview);
  check(moved > 150 && (await page.evaluate(() => history.length)) === entries, `the overview does not follow the finger: ${moved} px for 220`);
  check((await dock()).here === 2, "the dots do not follow the finger to the overview");
  // From the overview to the current semester, and on to the next: the row follows the finger, and
  // let go it rests there.
  await step("the overview drawn to the left", async () => { moved = await swipe(".st-progress", -220, { row: ".st-pager" }); }, resting("WiSe 2026/27"));
  await step("swipe", async () => { moved = await swipe(".st-pager", -220); }, resting("SoSe 2027"));
  check(moved > 150, `the cards do not follow the finger: ${moved} px for 220`);
  check((await dock()).here === 4, "the dots do not follow the finger");
  // A short pull, held: back where it was.
  await step("a short swipe", async () => { moved = await swipe(".st-pager", -50, { steps: 12, hold: 150 }); }, resting("SoSe 2027"));
  check(moved > 10, `a short pull does not move the cards: ${moved}`);
  // The window scrolled down to the end of a semester (owner, 2026-10-19: „wenn man nach unten
  // scrolled zum footer und dann weiter swiped dann gibt es ganz komisches behavior"): no ground
  // there, the page's end above the dots, nothing of it under them; the next one comes from its head.
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await page.waitForTimeout(200);
  const end = await page.evaluate(() => ({
    ground: getComputedStyle(document.querySelector("body > .ground")).display,
    room: document.documentElement.scrollHeight - innerHeight,
    above: Math.round(document.querySelector(".st-dock").getBoundingClientRect().top - document.querySelector(".st-pager > .st-slot[data-here]").firstElementChild.getBoundingClientRect().bottom),
  }));
  check(end.ground === "none" && end.above >= 0 && (end.above <= 2 || end.room <= 0), `the end of a semester on a phone: ${JSON.stringify(end)}`);
  const scrolled = await page.evaluate(() => Math.round(document.querySelector(".st-pager > .st-slot[data-here]").getBoundingClientRect().top - document.querySelector(".topbar").getBoundingClientRect().bottom));
  await step("swipe from the end of a semester", () => swipe(".st-pager", 220), resting("WiSe 2026/27"));
  const head = await page.evaluate(() => Math.round(document.querySelector(".st-pager > .st-slot[data-here] h2").getBoundingClientRect().top - document.querySelector(".topbar").getBoundingClientRect().bottom));
  check(scrolled < -40 && head >= 0 && head <= 16 && (await page.evaluate(() => scrollY)) === 0, `the semester that came does not show from its head: ${scrolled} before, ${head} after`);
  // To the end: one semester more.
  const restsAt = () => page.evaluate(() => [...document.querySelector(".st-pager").children].indexOf(document.querySelector(".st-pager > .st-slot[data-here]")));
  for (let i = 0; i < 10 && !(await page.$(".st-pager > .st-slot[data-here]:not([inert]) .st-new")); i++) {
    const at = await restsAt();
    await page.tap(`${HERE} .st-turn >> nth=1`);
    await page.waitForFunction((at) => {
      const pager = document.querySelector(".st-pager");
      const here = document.querySelector(".st-pager > .st-slot[data-here]");
      const index = [...pager.children].indexOf(here);
      return index === at + 1 && !here.inert && Math.abs(pager.scrollLeft - index * here.getBoundingClientRect().width) < 2;
    }, at, { timeout: 3000 }).catch(() => problems.push(`› did not turn from the card ${at}`));
  }
  check((await page.evaluate(() => document.querySelector(".st-pager > .st-slot[data-here] .st-new h2")?.textContent)) === "Neues Semester", "the last page does not add a semester");
  await step("anfügen", () => page.tap("#st-append"), resting("WiSe 2028/29"));
  check((await stored(page, MINE))?.includes("until\t2028W"), "the semester added was not stored");
  // The way back at the head: the semester is the page's state, no step of the history; the
  // overview slides in.
  entries = await page.evaluate(() => history.length);
  await watch();
  await step("the way back", () => page.tap(".top-back.on"), atOverview);
  glide = await glided();
  check(glide > 60 && glide <= 360, `the way back is no quick glide: ${glide} ms`);
  check((await page.evaluate(() => history.length)) === entries, "the way back added a step to the history");
  await page.waitForFunction(() => document.querySelector(".top-back")?.inert && getComputedStyle(document.querySelector(".top-back")).opacity === "0", null, { timeout: 2000 }).catch(() => problems.push("the way back stays on the overview"));
  // The credits one place to tap (owner, 2026-10-06: „die progress bar komplett zu einer
  // clickfläche werden"): their headline, the bar, the legend and „Bereiche ›" open the areas.
  // The whole box, not only the bar (owner, the same day: „nicht nur der ganze balken, sondern die
  // ganze progress box muss klickbar sein"): its parts, and 6 px inside each of its edges.
  const box = await page.$eval(".st-progress", (el) => { const r = el.getBoundingClientRect(); return { left: r.left, top: r.top, right: r.right, bottom: r.bottom }; });
  const spots = { headline: await middle(page, ".st-progress .st-headline"), bar: await middle(page, ".st-progress .st-bar"), legend: await middle(page, ".st-progress .st-legend-n li:nth-child(2)"),
    "left edge": { x: box.left + 6, y: (box.top + box.bottom) / 2 }, "right edge": { x: box.right - 6, y: (box.top + box.bottom) / 2 },
    "top edge": { x: (box.left + box.right) / 2, y: box.top + 6 }, "bottom edge": { x: (box.left + box.right) / 2, y: box.bottom - 6 }, corner: { x: box.left + 14, y: box.bottom - 14 } };
  for (const [part, at] of Object.entries(spots)) {
    await step(`the credits: ${part}`, () => page.touchscreen.tap(at.x, at.y), () => document.querySelector(".st-dialog[open] .st-area-list"));
    await step("close the areas", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-dialog[open]"));
  }
  await step("Bereiche", () => page.tap(".st-progress-top .st-link"), () => document.querySelector(".st-dialog[open] .st-area-list"));
  await page.waitForFunction(() => !document.querySelector(".st-dialog[open]")?.getAnimations().some((a) => a.playState === "running"), null, { timeout: 3000 }).catch(() => {});
  const sheet = await page.evaluate(() => { const box = document.querySelector(".st-dialog[open]").getBoundingClientRect(); return { bottom: Math.round(innerHeight - box.bottom), width: Math.round(box.width), top: Math.round(box.top) }; });
  // A sheet of a phone (owner, 2026-10-27): half up, as tall as the screen all up.
  check(Math.abs(sheet.top - 844 * 0.5) < 4 && sheet.bottom === -(844 - 10 - (844 - sheet.top)) && sheet.width === 390, `the areas are no sheet from below, half up: ${JSON.stringify(sheet)}`);
  // A finger drags the sheet down (owner, 2026-10-05: „dass man die einfach wieder runter sliden
  // kann"): it follows the finger; a short pull, let go, slides back; drawn far enough, it closes.
  const drag = async (by, { steps = 8, hold = 0 } = {}) => {
    await touch("touchStart", 195, sheet.top + 24);
    for (let i = 1; i <= steps; i++) { await page.waitForTimeout(16); await touch("touchMove", 195, sheet.top + 24 + (by * i) / steps); }
    if (hold) await page.waitForTimeout(hold);
    const top = await page.evaluate(() => Math.round(document.querySelector(".st-dialog[open]")?.getBoundingClientRect().top ?? -1));
    await touch("touchEnd", 0, 0);
    return top;
  };
  const pulled = await drag(60, { hold: 150 });
  await page.waitForTimeout(600);
  const after = await page.evaluate(() => Math.round(document.querySelector(".st-dialog[open]")?.getBoundingClientRect().top ?? -1));
  check(pulled >= sheet.top + 40 && after === sheet.top, `a short pull does not move the sheet and back: ${sheet.top} → ${pulled} → ${after}`);
  await step("the areas dragged down", () => drag(320), () => !document.querySelector(".st-dialog[open]"));
  await step("the program's card", () => page.tap("#st-mine"), () => document.querySelector(".st-dialog[open] #st-switch-program"));
  check((await page.evaluate(() => document.querySelector(".st-dialog[open] .st-dlg-head p"))) === null, "„Studiengang wechseln“ still has its lead");
  // − and + count the Fachsemester, and nothing of the sheet moves while they do (measured once it
  // has come up).
  await page.waitForFunction(() => document.querySelector(".st-dialog[open]")?.getAnimations().every((animation) => animation.playState !== "running"));
  const sheetAt = await cardAt(page, ".st-dialog[open]");
  await step("+ and −", async () => { await page.tap("#st-switch-more"); await page.tap("#st-switch-more"); await page.tap("#st-switch-less"); }, () => document.querySelector(".st-dialog[open] .st-study-say")?.textContent === "Du studierst Informatik B.Sc. im 4. Fachsemester." && document.querySelector(".st-foot-state")?.textContent === "1 Änderung");
  check((await cardAt(page, ".st-dialog[open]")) === sheetAt, `− and + moved the sheet: ${sheetAt} → ${await cardAt(page, ".st-dialog[open]")}`);
  // Under the card, what nothing but the Fachsemester changed means: a line at the foot of the body,
  // without a box (owner, 2026-10-17: „die änderungsbox … sieht akward aus. Mach die mal weg und
  // den Text nach unten bündig").
  const note = await page.evaluate(() => {
    const [note, body] = [".st-effect-note", ".st-dlg-body"].map((sel) => document.querySelector(`.st-dialog[open] ${sel}`));
    return { text: note?.textContent, box: note && getComputedStyle(note).borderTopStyle, below: note && Math.round(body.getBoundingClientRect().bottom - note.getBoundingClientRect().bottom) };
  });
  check(note.text?.startsWith("Nur der Studienbeginn ändert sich") && note.box === "none" && note.below >= 10 && note.below <= 24, `the note under the card is not a line at the foot of the body: ${JSON.stringify(note)}`);
  // The program's picker across the sheet, 8 px off its edges, its button and its list one box
  // (owner, 2026-10-17: „Eingabeboxen/comboboxen immer von ganz links nach ganz rechts mit 8pt
  // margin zum Rand in der Mobile Version"). The screen's keyboard up, the sheet is the part of the
  // page shown, without its foot, the picker at the top of its body and its list above the keyboard
  // („das modal kann ruhig full screen werden, wenn man das Keyboard öffnet. (Aber nur bei
  // onscreen keyboard)"); the keyboard gone, all as it was.
  const sheetNow = await cardAt(page, ".st-dialog[open]");
  await step("the program's picker", () => page.tap("#st-switch-program"), () => document.activeElement?.id === "st-switch-program-search");
  await still(page, ".st-dialog[open]");
  const across = await page.evaluate(() => ["#st-switch-program", ".st-dialog[open] .combo-pop"].map((sel) => { const r = document.querySelector(sel).getBoundingClientRect(); return `${Math.round(r.left)}|${Math.round(innerWidth - r.right)}`; }));
  check(across.every((edges) => edges === "8|8") && !(await page.evaluate(() => document.documentElement.hasAttribute("data-keyboard"))), `the program's picker is not across the sheet, or a field alone is a keyboard: ${across}`);
  // It glides there and its body scrolls, frame by frame: nothing jumps (owner, the same day, after
  // a try on her phone: „Animation bitte. Aktuell springt das noch komisch").
  await page.evaluate(() => {
    window.__rise = [];
    const tick = () => {
      const dialog = document.querySelector(".st-dialog[open]");
      if (dialog) window.__rise.push([Math.round(dialog.getBoundingClientRect().top), dialog.querySelector(".st-dlg-body").scrollTop]);
      if (window.__rise.length < 90) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
  await screenKeyboard(page, 336);
  const rise = await page.evaluate(() => window.__rise);
  const [firstTop, lastTop, lastScroll] = [rise[0][0], rise[rise.length - 1][0], rise[rise.length - 1][1]];
  const ways = [new Set(rise.map(([top]) => top).filter((top) => top > lastTop + 2 && top < firstTop - 2)).size, new Set(rise.map(([, scroll]) => scroll).filter((scroll) => scroll > 2 && scroll < lastScroll - 2)).size];
  // (All up at once since 2026-11-04, its top has but the 10 px under the safe area to go.)
  check((ways[0] >= 3 || firstTop - lastTop <= 12) && ways[1] >= 3, `the sheet jumps over the keyboard (${ways} frames on the way): ${rise.map((frame) => frame.join("/")).join(" ")}`);
  const typing = await page.evaluate(() => {
    const at = (sel) => document.querySelector(sel).getBoundingClientRect();
    const [dialog, body, label, pop] = [".st-dialog[open]", ".st-dialog[open] .st-dlg-body", ".st-dialog[open] .st-srow:has(.combo[data-open]) > .label", ".st-dialog[open] .combo-pop"].map(at);
    return { shown: innerHeight - 336, dialog: [dialog.top, dialog.bottom].map(Math.round), foot: getComputedStyle(document.querySelector(".st-dialog[open] .st-dlg-foot")).display, label: Math.round(label.top - body.top), pop: Math.round(pop.bottom) };
  });
  check(typing.dialog[0] === 0 && typing.dialog[1] === typing.shown && typing.foot === "none" && typing.label === 8 && typing.pop <= typing.shown - 7, `the keyboard up, the sheet is not the part shown with the picker at its top: ${JSON.stringify(typing)}`);
  await step("the keyboard gone", async () => { await page.keyboard.press("Escape"); await screenKeyboard(page, 0); }, () => !document.documentElement.hasAttribute("data-keyboard") && !document.querySelector(".st-dialog[open] .combo[data-open]"));
  await still(page, ".st-dialog[open]");
  check((await cardAt(page, ".st-dialog[open]")) === sheetNow, `the keyboard gone, the sheet is not as it was: ${sheetNow} → ${await cardAt(page, ".st-dialog[open]")}`);
  await step("close it", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-dialog[open]"));
  // A column leads to its semester, one that is over to the left of the overview; Back to the
  // overview: on Android a close request to the semester's watcher (Escape in Chromium), no step of
  // the history.
  await step("a column", () => page.tap(".st-chart .st-col >> nth=1"), resting("SoSe 2026"));
  entries = await page.evaluate(() => history.length);
  await step("Back", () => page.keyboard.press("Escape"), atOverview);
  check((await page.evaluate(() => history.length)) === entries, "Back from a semester moved through the history");
  // The whole overview drawn (owner, 2026-10-05: „dass man die komplette page nach links sliden
  // kann"), from its credits: a short pull, held, leaves it; drawn on to the right, the last
  // semester that is over comes (owner, 2026-10-06: „so dass man nach rechts swipen muss, um die
  // zu sehen"), with the way back at the head.
  const home = await page.evaluate(() => document.querySelector(".st-pager").scrollLeft);
  await step("the overview drawn a little", async () => { moved = await swipe(".st-progress", -50, { steps: 12, hold: 150, row: ".st-pager" }); }, (home) => location.search === "" && Math.abs(document.querySelector(".st-pager")?.scrollLeft - home) < 2, home);
  check(moved > 10, `a finger does not draw the overview: ${moved}`);
  await step("the overview drawn to the right", async () => { moved = await swipe(".st-progress", 260, { row: ".st-pager" }); }, resting("SoSe 2026"));
  check(moved < -180 && (await page.$(".top-back.on")) !== null, `the overview drawn to the right did not lead to the semester before with the way back: ${moved}`);
  // To the first: a finger to the right on it goes nowhere further; the way back leads to the
  // overview.
  await step("‹ to the first", () => page.tap(`${HERE} .st-turn >> nth=0`), resting("WiSe 2025/26"));
  await step("the first semester drawn to the right", async () => { moved = await swipe(".st-pager", 160); }, resting("WiSe 2025/26"));
  // The window scrolled while the row moves (owner, 2026-10-19: „wenn man vertikal am scrollen ist
  // während man swiped. Dann springt das am [Ende vom] swipen wieder zum Anfang"): the page that came
  // stays as far down in itself as it was scrolled, nothing moves as the row rests. The window goes
  // down halfway through the finger's way and again as it lets go, as a fling of it would; from the
  // first semester to the second, both longer than the screen. (Scrolled past the end of the page
  // that came, which the row as tall as the page it left allows, the window goes back to its end.)
  const headOf = () => page.evaluate(() => {
    const mid = [...document.querySelectorAll(".st-pager > .st-slot")].find((slot) => { const box = slot.getBoundingClientRect(); return box.left < innerWidth / 2 && box.right > innerWidth / 2; });
    return { name: mid?.querySelector("h2")?.textContent, top: Math.round(mid.querySelector("h2").getBoundingClientRect().top - document.querySelector(".topbar").getBoundingClientRect().bottom), end: document.documentElement.scrollHeight - innerHeight - scrollY < 2 };
  });
  const sideways = await page.$eval(".st-pager", (el) => { const box = el.getBoundingClientRect(); return { x: box.right - 30, y: Math.min(Math.max(box.top + 120, 160), 600) }; });
  await touch("touchStart", sideways.x, sideways.y);
  for (let i = 1; i <= 10; i++) {
    await page.waitForTimeout(16);
    await touch("touchMove", sideways.x - 22 * i, sideways.y);
    if (i === 5) await page.evaluate(() => window.scrollBy(0, 10));
  }
  await touch("touchEnd", 0, 0);
  await page.evaluate(() => window.scrollBy(0, 10));
  const swiping = await headOf();
  await page.waitForFunction(resting("SoSe 2026"), null, { timeout: 4000 }).catch(() => problems.push("the swipe while the window scrolls does not rest at SoSe 2026"));
  await page.waitForTimeout(200);
  const rested = await headOf();
  check(swiping.name === "SoSe 2026" && swiping.top < -10 && rested.name === "SoSe 2026" && (Math.abs(rested.top - swiping.top) <= 2 || (rested.end && rested.top > swiping.top && rested.top < 0)), `a semester scrolled while the row moves does not stay as scrolled: ${JSON.stringify({ swiping, rested })}`);
  await page.evaluate(() => window.scrollTo(0, 0));
  await step("back to the first", async () => { moved = await swipe(".st-pager", 220); }, resting("WiSe 2025/26"));
  // From the end of a semester, up while the row moves (owner, 2026-10-19: „wenn man bei einem
  // langen semester ganz nach unten scrolled, dann zur seite swiped und dann nach oben scrolled, wird
  // der vertikale space von der gerade verschwinden seite noch als scrollbar angesehen, bis sie
  // verschwindet und dann springt es wieder zurück"): the window scrolls the page that comes, from
  // its head, with nothing of the page that goes above it, and nothing moves as the row rests.
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await page.waitForTimeout(200);
  const deep = await page.evaluate(() => scrollY);
  await touch("touchStart", sideways.x, sideways.y);
  for (let i = 1; i <= 10; i++) {
    await page.waitForTimeout(16);
    await touch("touchMove", sideways.x - 22 * i, sideways.y);
    if (i === 5) await page.evaluate(() => window.scrollBy(0, -100));
  }
  await touch("touchEnd", 0, 0);
  await page.evaluate(() => window.scrollBy(0, -100));
  const upSwiping = await headOf();
  await page.waitForFunction(resting("SoSe 2026"), null, { timeout: 4000 }).catch(() => problems.push("the swipe up from the end of a semester does not rest at SoSe 2026"));
  await page.waitForTimeout(200);
  const upRested = await headOf();
  check(deep > 100 && upSwiping.name === "SoSe 2026" && upSwiping.top >= 0 && upSwiping.top <= 16 && upRested.name === "SoSe 2026" && upRested.top === upSwiping.top, `scrolled up while the row leaves the end of a semester: ${JSON.stringify({ deep, upSwiping, upRested })}`);
  await step("back to the first again", async () => { moved = await swipe(".st-pager", 220); }, resting("WiSe 2025/26"));
  entries = await page.evaluate(() => history.length);
  await step("the way back from the first", () => page.tap(".top-back.on"), atOverview);
  check((await page.evaluate(() => history.length)) === entries, "the way back from a semester that is over added a step to the history");
  // The way back from the moment a finger draws the row off the overview (owner, 2026-10-06: „der
  // sollte eigentlich sofort eingeblendet werden, wenn man weg swiped, von der main page"), while
  // the finger still holds it; gone again once the row is back.
  const progress = await page.$eval(".st-progress", (el) => { const r = el.getBoundingClientRect(); return { right: r.right, y: Math.min(Math.max(r.top + r.height / 2, 160), 600) }; });
  await touch("touchStart", progress.right - 30, progress.y);
  for (let i = 1; i <= 8; i++) { await page.waitForTimeout(16); await touch("touchMove", progress.right - 30 - 6 * i, progress.y); }
  await page.waitForTimeout(200);
  const backWhileHeld = await page.evaluate(() => Boolean(document.querySelector(".top-back.on")));
  await touch("touchMove", progress.right - 30, progress.y);
  await touch("touchEnd", 0, 0);
  check(backWhileHeld, "the way back does not show while a finger draws the row off the overview");
  await page.waitForFunction(atOverview, null, { timeout: 3000 }).catch(() => problems.push("the overview drawn a little did not come back"));
  await page.waitForFunction(() => !document.querySelector(".top-back.on"), null, { timeout: 2000 }).catch(() => problems.push("the way back stays at the overview after a finger let it go"));
  // And it goes as soon (owner, 2026-10-18: „das einblenden und ausblenden sollte gleich schnell
  // gehen … verschwindet aber bei der home page, erst wenn ich aufgehört habe zu swipen"): from the
  // semester next to the overview, with the finger's first move back into the overview, while the
  // finger still holds the row; let go short of half way, the row goes back to the semester, and
  // the way back with it.
  await step("the overview drawn to the right again", async () => { moved = await swipe(".st-progress", 260, { row: ".st-pager" }); }, resting("SoSe 2026"));
  const pager = await page.$eval(".st-pager", (el) => { const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: Math.min(Math.max(r.top + 120, 160), 600) }; });
  await touch("touchStart", pager.x, pager.y);
  for (let i = 1; i <= 8; i++) { await page.waitForTimeout(16); await touch("touchMove", pager.x - 6 * i, pager.y); }
  await page.waitForTimeout(150);
  const backGoneWhileHeld = await page.evaluate(() => !document.querySelector(".top-back.on") && document.querySelector(".topbar .top-profile")?.inert === false);
  await touch("touchEnd", 0, 0);
  check(backGoneWhileHeld, "the way back stays while a finger draws the row back into the overview");
  await page.waitForFunction(new Function(`return (${resting("SoSe 2026").toString()})() && document.querySelector(".top-back.on")`), null, { timeout: 3000 }).catch(() => problems.push("let go short of half way, the row did not go back to the semester with the way back"));
  await step("the way back from the semester before", () => page.tap(".top-back.on"), atOverview);
  // The tab of „Studium" and the way back, tapped while the browser still flings the row to the
  // next page: the row back at the overview at once (owner, 2026-10-06: „funktioniert der button in
  // der nav bar auch erst, wenn die seite sich nicht mehr bewegt … Wenn man den anklickt muss es
  // sofort wieder an die standard position gehen"): from the click to rest at most 450 ms (a glide
  // of 360 ms at most; waiting for the browser's fling and snap took longer than that alone), and
  // nothing of the browser's moves after it.
  const fling = async () => {
    await touch("touchStart", 360, progress.y);
    for (let i = 1; i <= 6; i++) { await page.waitForTimeout(12); await touch("touchMove", 360 - 50 * i, progress.y); }
    await touch("touchEnd", 0, 0);
    await page.waitForTimeout(60);
  };
  const untilHome = () => page.evaluate(() => new Promise((resolve) => {
    let clicked = null;
    addEventListener("click", () => { clicked ??= performance.now(); }, { capture: true, once: true });
    const row = document.querySelector(".st-pager");
    const overview = row.querySelector(".st-slot.is-overview");
    const start = performance.now();
    let rest = null;
    const tick = (t) => {
      const there = Math.abs(overview.getBoundingClientRect().left - 6) < 1 && !row.classList.contains("is-gliding");
      if (there && rest === null) rest = t;
      if (!there) rest = null;
      if (t - start < 2500) requestAnimationFrame(tick);
      else resolve(clicked === null || rest === null ? -1 : Math.round(rest - clicked));
    };
    requestAnimationFrame(tick);
  }));
  const tabAt = await middle(page, '.bottomnav > .nav[data-area="programs"]');
  for (const [what, at] of [["the tab of „Studium“", async () => tabAt], ["the way back", () => middle(page, ".top-back.on")]]) {
    const timed = untilHome();
    await fling();
    const spot = await at();
    await page.touchscreen.tap(spot.x, spot.y);
    const ms = await timed;
    check(ms >= 0 && ms <= 450, `${what} tapped while the row flings: not back at the overview at once (${ms} ms)`);
    check(await page.evaluate(new Function(`return (${atOverview.toString()})()`)), `${what} tapped while the row flings: the row did not stay at the overview`);
  }
  // A module of a semester comes up as a sheet over the row (owner, 2026-10-06), and goes again;
  // the row stays at its semester.
  await step("a column again", () => page.tap(".st-chart .st-col >> nth=0"), resting("WiSe 2025/26"));
  const name = await middle(page, `${HERE} a.st-name`);
  await step("a module of the semester", () => page.touchscreen.tap(name.x, name.y), () => location.search.startsWith("?open=") && document.querySelector(".detail.is-module h2"));
  await page.waitForTimeout(500);
  const up = await page.evaluate(() => Math.round(document.querySelector(".detail.is-module").getBoundingClientRect().top));
  check(Math.abs(up - 844 * 0.5) < 4, `the module's sheet does not come up to half the screen: ${up}`);
  await touch("touchStart", 195, up + 80);
  for (let i = 1; i <= 12; i++) { await page.waitForTimeout(30); await touch("touchMove", 195, up + 80 + 30 * i); }
  await page.waitForTimeout(150);
  await touch("touchEnd", 0, 0);
  await page.waitForFunction(new Function(`return location.search === "" && !document.querySelector(".detail.is-module") && (${resting("WiSe 2025/26").toString()})()`), null, { timeout: 4000 }).catch(() => problems.push("the module's sheet drawn down did not leave the semester as it was"));

  // The views on a phone: pages of their own behind the overview, the way back at the head.
  const go = (href) => page.evaluate((href) => { const a = document.createElement("a"); a.href = href; document.body.append(a); a.click(); a.remove(); }, href);
  await step("the overview", () => go("/study"), () => location.search === "" && document.querySelector(".st-pager"));
  await step("the Regelstudienplan on a phone", () => page.tap('.st-mine-panel .st-ways a[href="/study?view=plan"]'), () => location.search === "?view=plan" && document.querySelectorAll(".st-rsp-phone .st-fs-sec").length >= 5 && document.querySelector(".top-back.on") && document.querySelector(".topbar .top-profile").inert);
  const sections = await page.evaluate(() => ({
    strip: [...document.querySelectorAll(".st-fsidx a")].map((a) => a.textContent.trim()),
    cards: document.querySelectorAll(".st-pcard").length,
    wide: document.documentElement.scrollWidth <= innerWidth + 1,
  }));
  check(sections.strip[2] === "3. FS · jetzt" && sections.cards === 23 && sections.wide, `the Regelstudienplan on a phone: ${JSON.stringify(sections)}`);
  // No page of the app has a ground (owner, 2026-11-03).
  check(!(await grounded()), "the Regelstudienplan on a phone has a ground");
  // While the way back is at the head, the way to the profile page gives way to it (owner,
  // 2026-10-15), and it comes back with the overview.
  await step("the way back", () => page.tap(".top-back.on"), () => location.search === "" && document.querySelector(".st-pager") && !document.querySelector(".top-back.on") && !document.querySelector(".topbar .top-profile").inert);
  // The Wahlpflicht on a phone: the choices as cards, each a page of its own behind them, whose way
  // back leads to them; a module of it opens as the sheet, „Einplanen" there for the choice.
  await step("the Wahlpflicht on a phone", () => page.tap('.st-mine-panel .st-ways a[href="/study?view=areas"]'), () => location.search === "?view=areas" && document.querySelectorAll(".st-wp-phone .st-wcard").length > 3);
  check(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), "the Wahlpflicht on a phone scrolls sideways");
  await step("a choice's page on a phone", () => page.tap('.st-wcard:has-text("Anwendungsfach")'), () => /^\?view=areas&choice=\d+$/.test(location.search) && document.querySelector(".st-wp-phone .st-wph h1")?.textContent === "Anwendungsfach" && document.querySelectorAll(".st-wcard-m").length > 0 && document.querySelector(".top-back.on"));
  check(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), "a choice's page on a phone scrolls sideways");
  await step("its module as the sheet", () => page.tap(".st-wcard-m:not(.has) >> nth=0"), () => /&open=\d+/.test(location.search) && document.querySelector('.detail.is-module .plan-toggle[data-action="plan"]')?.textContent.includes("für „Anwendungsfach“"));
  await step("the sheet closed", () => page.goBack(), () => /^\?view=areas&choice=\d+$/.test(location.search) && !document.querySelector(".detail.is-module"));
  await step("the way back to the choices", () => page.tap(".top-back.on"), () => location.search === "?view=areas" && document.querySelectorAll(".st-wp-phone .st-wcard").length > 3);
  // A choice once more: the steps back are in the history ahead of here no more, as the ones after
  // count it.
  await step("a choice's page once more", () => page.tap('.st-wcard:has-text("Praktische Mathematik")'), () => /^\?view=areas&choice=\d+$/.test(location.search) && document.querySelector(".st-wp-phone .st-wph h1")?.textContent === "Praktische Mathematik");
  // „Alle Studiengänge" has its way back at the head too, up to „Mein Studium" (owner, 2026-10-17:
  // „wenn man auf alle Studiengänge geht, dann fehlt da ein zurück button"): back through the
  // history, which does not grow, and the way to the profile page gives way meanwhile.
  await step("the overview once more", () => go("/study"), () => location.search === "" && document.querySelector(".st-pager"));
  const before = await page.evaluate(() => history.length);
  // The way back and the way to the profile page change places as one row that glides (owner,
  // 2026-10-17: „Mach das mal smooth"): by `transform` and `opacity` alone, which the browser runs
  // on a thread of its own while the page is being built, every box as wide as before.
  const watchRow = () => page.evaluate(() => {
    const row = (window.__row = { moved: new Set(), widths: new Set() });
    const end = performance.now() + 700;
    const tick = () => {
      for (const el of document.querySelectorAll(".topbar > .top-back, .topbar > .search, .topbar > .top-profile")) {
        for (const animation of el.getAnimations()) row.moved.add(`${el.classList[0]}:${animation.transitionProperty}`);
        row.widths.add(`${el.classList[0]}:${el.offsetWidth}`);
      }
      if (performance.now() < end) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
  const rowMoved = async (what) => {
    await page.waitForTimeout(750);
    const row = await page.evaluate(() => ({ moved: [...window.__row.moved].sort().join(), widths: [...window.__row.widths].sort().join() }));
    check(row.moved === "search:transform,top-back:opacity,top-back:transform,top-profile:opacity,top-profile:transform" && row.widths === "search:310,top-back:48,top-profile:48", `${what}: the row at the head does not glide by transform and opacity alone: ${JSON.stringify(row)}`);
  };
  await watchRow();
  await step("all programs on a phone", () => page.tap('.st-rest a[href="/programs"]'), () => location.pathname === "/programs" && document.querySelector(".faccs") && document.querySelector(".top-back.on")?.getAttribute("href") === "/study" && document.querySelector(".topbar .top-profile").inert);
  await rowMoved("the way back coming");
  // The row stays built behind (folia_design::stage); the page in front has no ground either.
  check(!(await grounded()), "all programs on a phone have a ground after „Mein Studium\"");
  await watchRow();
  await step("the way back from all programs", () => page.tap(".top-back.on"), () => location.pathname === "/study" && location.search === "" && document.querySelector(".st-pager") && !document.querySelector(".top-back.on") && !document.querySelector(".topbar .top-profile").inert);
  await rowMoved("the way back going");
  check(!(await grounded()), "back at the row, a ground shows under it");
  check((await page.evaluate(() => history.length)) === before + 1, "the way back from all programs added a step to the history");
  await context.close();
}

// ---------- 3b: the catalog with the workbench as its target, on a phone ----------
{
  const { page, step, seed, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 }, "/study?plan=1");
  await seed([[MINE, THIRD], [PLAN, FILLED]]);
  const HERE = ".st-pager > .st-slot:not([inert])";
  await page.waitForTimeout(600);
  await step("Module hinzufügen, for the catalog", () => page.tap(`${HERE} .st-empty-add`), () => document.querySelector(".st-wb-sheet[data-sheet] #st-wb-takeover"));
  await page.waitForTimeout(600);
  // „Im Katalog suchen" leads into the tab „Module" with the semester as its target; on a phone the
  // tab bar gives way to the bar of the target (owner, 2026-11-04: „wenn unten die navbar mit
  // ausgetauscht wird mit einer interaction bar, wo man das dann abbrechen kann und wieder zurück zu
  // mein studium kommt"), and „Abbrechen" leads back to the sheet as it was.
  // At the root's end, below what the sheet shows half up.
  await step("Im Katalog suchen on a phone", () => page.locator(".st-wb-sheet a.st-wb-a.way").dispatchEvent("click"), () => location.pathname === "/catalog" && document.querySelector(".bench-dock"));
  await page.waitForTimeout(400);
  const benchDock = await page.evaluate(() => {
    const box = document.querySelector(".bench-dock").getBoundingClientRect();
    const nav = document.querySelector(".bottomnav").getBoundingClientRect();
    const hit = document.elementFromPoint(nav.left + nav.width / 2, nav.top + nav.height / 2);
    const top = document.querySelector(".bench-bar");
    return { title: document.querySelector(".bench-dock-w > b")?.textContent, go: document.querySelector(".bench-dock-go")?.textContent, covers: Boolean(hit?.closest(".bench-dock")), same: Math.abs(box.top - nav.top) < 1 && Math.abs(box.height - nav.height) < 1, top: top ? getComputedStyle(top).display : "gone" };
  });
  check(benchDock.title === "Hinzufügen zu WiSe 2026/27" && benchDock.go === "Abbrechen" && benchDock.covers && benchDock.same && benchDock.top === "none", `the catalog's bar on a phone: ${JSON.stringify(benchDock)}`);
  await step("Abbrechen", () => page.tap(".bench-dock-go"), () => location.pathname === "/study" && document.querySelector(".st-wb-sheet #st-wb-takeover") && !document.querySelector(".bench-dock"));
  await page.waitForTimeout(400);
  check((await page.evaluate((HERE) => document.querySelector(`${HERE} .st-sem-name`)?.textContent ?? "", HERE)).startsWith("WiSe 2026/27"), "„Abbrechen“ does not lead back to the semester the workbench plans");
  // The sheet has no button to end it: a tap above it does, as a throw and Back.
  await step("a tap above the sheet after the catalog", () => page.touchscreen.tap(195, 120), () => !document.querySelector(".st-wb-sheet") && !document.querySelector(".bench-dock"));
  await context.close();
}

// ---------- 5: without the app ----------
{
  const html = await fetch(base + "/study").then((r) => r.text());
  check(html.includes('content="noindex') && html.includes("<title>Mein Studium · Betula</title>"), "the server's page of „Mein Studium“ is not one page for no index");
  check(/<a [^>]*data-area="programs"[^>]* href="\/programs"/.test(html), "the server's Studium tab does not lead to the overview");
  const old = await fetch(base + "/programs/bachelor-informatik-2008/my-plan").then((r) => r.text());
  check(old.includes('href="/study"') && old.includes('content="noindex'), "„Mein Plan“ does not lead on to „Mein Studium“");
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
