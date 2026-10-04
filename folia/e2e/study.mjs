// Checks „Mein Studium" (folia/crates/planner/src/study, folia/crates/plans/src/study.rs), the first
// page of the Studium tab (owner, 2026-10-04).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node study.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// 1 a fresh browser: the Studium tab leads to it, it asks for the program and stores nothing by
//   being looked at; a pick shows the study, the Studienbeginn assumed until „Stimmt" stores it.
// 2 Informatik B.Sc. in its third semester (begun in WiSe 2025/26): the semesters before the
//   current one open to tick off; in the current one what is left over first, the summer's in the
//   summer; ticking off keeps the row where it is and takes the module out of the current
//   semester; „Alles bestanden"; ticked off in the current one, a row stays where it is too; ↓ to
//   the next winter, the focus with it, ↺ back; a module beside
//   the page, in full and back; „In den Stundenplan" once, with „Rückgängig"; the Stundenplan's tab
//   counts the current semester; a reload keeps all of it; nothing of it in a request.
// 3 a phone: the sidebar is a sheet, nothing scrolls sideways, rows as tall as a finger.
// 4 the Stundenplan: its import takes the current semester from „Mein Studium" too.
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
  return { page, step, context };
};
const stored = (page, key) => page.evaluate((key) => localStorage.getItem(key), key);
// The rows of a semester by its name („WiSe 2026/27"): their names and marks.
const rows = (page, semester) => page.evaluate((semester) => {
  const term = [...document.querySelectorAll(".st-term")].find((t) => t.querySelector(".st-sem")?.textContent === semester);
  return term ? [...term.querySelectorAll(".st-row")].map((row) => ({
    name: row.querySelector(".st-name")?.textContent,
    marks: [...row.querySelectorAll(".st-mark")].map((mark) => mark.textContent),
    done: row.querySelector(".st-tick")?.getAttribute("aria-checked") === "true",
  })) : null;
}, semester);
// A semester's panel, for real clicks.
const term = (page, semester) => page.locator(`.st-term:has(.st-sem:text-is("${semester}"))`);
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
  await step("Studium", () => page.click('.rail a[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-welcome"));
  const fresh = await page.evaluate(() => ({
    title: document.querySelector(".crumb h1")?.textContent,
    heading: document.querySelector(".st-welcome h1")?.textContent,
    tab: document.querySelector('.rail a[data-area="programs"]')?.getAttribute("aria-current"),
    all: document.querySelector('.st-welcome a[href="/programs"]') !== null,
    legend: document.querySelector(".st-legend") !== null,
  }));
  check(fresh.title === "Mein Studium" && fresh.heading === "Plane dein Studium" && fresh.tab === "page" && fresh.all && !fresh.legend, `the page without a program: ${JSON.stringify(fresh)}`);
  check((await stored(page, PLAN)) === null && (await stored(page, MINE)) === null, "looking at „Mein Studium“ stored something");
  // A pick in the picker: the study at once, the Studienbeginn assumed (this winter: a first semester).
  await step("pick a program", async () => {
    await page.click("#st-welcome-program");
    await page.fill("#st-welcome-program-search", "informatik b.sc. 2008");
    await page.keyboard.press("Enter");
  }, () => document.querySelector(".st-head h1")?.textContent?.startsWith("Informatik B.Sc."));
  const assumed = await page.evaluate(() => ({ when: document.querySelector(".st-when")?.textContent, asked: document.querySelector(".st-start .note-action") !== null, past: document.querySelector(".st-past") !== null, start: document.querySelector("#st-start")?.value }));
  check(assumed.when?.startsWith("1. Fachsemester · WiSe 2026/27") && assumed.asked && !assumed.past && assumed.start === "2026W", `the assumed Studienbeginn: ${JSON.stringify(assumed)}`);
  check(!(await stored(page, MINE))?.includes("start\t"), "an assumed Studienbeginn was stored");
  await step("Stimmt", () => page.click(".st-start .note-action button"), () => !document.querySelector(".st-start .note-action"));
  check((await stored(page, MINE))?.includes("start\t2026W"), "„Stimmt“ did not store the Studienbeginn");
  for (const request of requests) {
    if (/079-82-2008|start%09|2026W%0A/.test(request)) problems.push(`a request carries the study: ${request.slice(0, 160)}`);
  }
  await context.close();
}

// ---------- 2: Informatik B.Sc. in its third semester ----------
{
  const requests = [];
  const { page, step, context } = await open({ viewport: { width: 1440, height: 900 } }, "/", { requests });
  await page.evaluate(([key, text]) => localStorage.setItem(key, text), [MINE, THIRD]);
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  await step("Studium", () => page.click('.rail a[data-area="programs"]'), () => location.pathname === "/study" && document.querySelectorAll(".st-term").length > 4);
  const head = await page.evaluate(() => ({ when: document.querySelector(".st-when")?.textContent, progress: document.querySelector(".st-progress-text")?.textContent, open: document.querySelector(".st-past")?.open }));
  check(head.when === "3. Fachsemester · WiSe 2026/27" && head.progress === "0 von 180 LP" && head.open === true, `the head of the third semester: ${JSON.stringify(head)}`);
  // Nothing ticked off: what the first two semesters held comes first in the next one that offers it.
  const now = await rows(page, "WiSe 2026/27");
  const left = now.findIndex((row) => !row.marks.some((mark) => mark.startsWith("nachholen")));
  check(left > 3 && now.slice(left).every((row) => !row.marks.some((mark) => mark.startsWith("nachholen"))), `the current semester does not begin with what is left over: ${JSON.stringify(now.map((row) => row.name))}`);
  check(now.some((row) => row.name === "Entwicklung von Softwaresystemen" && row.marks.includes("nachholen · aus 1. FS")), "a module of the first winter is not due again in this one");
  const summer = await rows(page, "SoSe 2027");
  check(summer[0]?.marks.join() === "nachholen · aus 2. FS,nur im SoSe", `the summer does not begin with the summer's modules left over: ${JSON.stringify(summer[0])}`);
  check((await page.evaluate(() => document.querySelector(".st-term.is-now .st-now")?.textContent)) === "jetzt", "the current semester is not marked");

  // Ticking off keeps the row in its place; the module leaves the current semester.
  const firstWinter = await rows(page, "WiSe 2025/26");
  await step("tick off", () => term(page, "WiSe 2025/26").locator(".st-row .st-tick").first().click(), () => !document.querySelector(".st-term.is-now")?.textContent.includes("Entwicklung von Softwaresystemen"));
  const ticked = await rows(page, "WiSe 2025/26");
  check(ticked[0]?.name === firstWinter[0]?.name && ticked[0]?.done && ticked.length === firstWinter.length, `ticking off moved the row: ${JSON.stringify(ticked)}`);
  check((await page.evaluate(() => document.activeElement?.classList.contains("st-tick"))), "ticking off lost the focus");
  check((await stored(page, PLAN)) === "d\t2025W\t12104\n", `what was stored for a tick: ${JSON.stringify(await stored(page, PLAN))}`);
  await step("Alles bestanden", () => term(page, "SoSe 2026").locator(".st-all").click(), () => !document.querySelector(".page-inner.study")?.textContent.includes("nachholen · aus 2. FS"));
  check((await rows(page, "SoSe 2026")).every((row) => row.done), "„Alles bestanden“ left a row of the summer open");
  check((await page.evaluate(() => document.querySelector(".st-progress-text")?.textContent)) === "36 von 180 LP", "the progress does not count what was ticked off");
  // Ticked off in the current semester, a row stays where it is, the focus on it; ticked again,
  // it is open again.
  const nowOpen = await rows(page, "WiSe 2026/27");
  const tick = await page.evaluate(() => document.querySelector(".st-term.is-now .st-row .st-tick")?.id);
  await step("tick off now", () => page.click(`#${tick}`), (id) => document.getElementById(id)?.getAttribute("aria-checked") === "true", tick);
  const nowDone = await rows(page, "WiSe 2026/27");
  check(nowDone[0]?.done && nowDone.map((row) => row.name).join() === nowOpen.map((row) => row.name).join(), `ticking off in the current semester moved the row: ${JSON.stringify(nowDone.map((row) => row.name))}`);
  check((await page.evaluate(() => document.activeElement?.id)) === tick, "ticking off in the current semester lost the focus");
  await step("open again", () => page.click(`#${tick}`), (id) => document.getElementById(id)?.getAttribute("aria-checked") === "false", tick);
  check((await page.evaluate(() => document.querySelector(".st-progress-text")?.textContent)) === "36 von 180 LP", "opened again, the module still counts");

  // ↓ moves a module of the current winter to the next one (the summer does not offer it); the
  // focus goes with it; ↺ puts it back.
  const moved = await page.evaluate(() => {
    const row = [...document.querySelectorAll(".st-term.is-now .st-row")].find((row) => row.querySelector(".st-name")?.textContent === "Theoretische Informatik");
    return row?.querySelector('.st-act[aria-label^="Später"]')?.getAttribute("aria-label");
  });
  check(moved === "Später: WiSe 2027/28", `„später“ of a winter module says ${moved}`);
  await step("später", () => page.locator('.st-term.is-now .st-row:has(.st-name:text-is("Theoretische Informatik")) .st-act[aria-label^="Später"]').click(), () => document.activeElement?.id === "st-2027W-m-11787");
  check((await rows(page, "WiSe 2027/28"))[0]?.marks.includes("verschoben · laut Plan 3. FS"), "the moved module does not say where the plan has it");
  await step("zurück an seinen Platz", () => term(page, "WiSe 2027/28").locator('.st-act[aria-label^="Zurück"]').click(), () => [...document.querySelectorAll(".st-term.is-now .st-name")].some((name) => name.textContent === "Theoretische Informatik"));

  // A module beside the page, in full, and back.
  await step("a module beside it", () => page.locator('.st-term.is-now a.st-name:text-is("Theoretische Informatik")').click(), () => location.search === "?open=11787" && document.querySelector("#preview .hero .mono")?.textContent.trim() === "11787");
  await step("Vollbild", () => page.click('#preview [data-action="fullscreen"]'), () => location.search === "?open=11787&full=1" && document.querySelector(".module-page"));
  check((await page.getAttribute('.rail a[data-area="programs"]', "aria-current")) === "page", "the module in full left the Studium tab");
  await step("Zurück", () => page.click('[data-action="back"]'), () => location.search === "?open=11787" && document.querySelector(".st-term.is-now"));
  await step("close it", () => page.click('#preview [data-action="close-detail"]'), () => location.search === "" && !document.querySelector("#preview"));

  // „In den Stundenplan": the current semester into its timetable, once; „Rückgängig".
  const offered = await page.evaluate(() => document.querySelector("#st-take")?.textContent);
  check(/^In den Stundenplan \(\d+\)$/.test(offered ?? ""), `„In den Stundenplan“ says ${offered}`);
  await step("In den Stundenplan", () => page.click("#st-take"), () => document.querySelector(".st-take .note-action")?.textContent.startsWith("Übernommen:"));
  const taken = await page.evaluate(() => [...document.querySelectorAll(".st-term.is-now .st-row")].filter((row) => !row.querySelector(".st-tick[aria-checked='true']")).every((row) => row.querySelector(".st-mark.timetable")));
  check(taken, "a row of the current semester is not in the timetable after „In den Stundenplan“");
  const count = await page.evaluate(() => document.querySelector('.rail .nav[data-area="studyplan"] .nav-count')?.textContent);
  const planned = (await stored(page, PLAN)).split("\n").filter((line) => line.startsWith("m\t2026W\t")).length;
  check(Number(count) === planned && planned > 4, `the Stundenplan's tab counts ${count}, the current semester holds ${planned}`);
  await step("Rückgängig", () => page.click(".st-take .note-action button"), () => document.querySelector("#st-take"));
  check(!(await stored(page, PLAN)).includes("m\t2026W"), "„Rückgängig“ left modules in the timetable");

  // A reload keeps what was ticked off.
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(() => document.querySelector(".st-progress-text")?.textContent === "36 von 180 LP", null, { timeout: 8000 }).catch(() => problems.push("a reload lost what was ticked off"));
  for (const request of requests) {
    if (/12104|079-82-2008|11787%0A|d%092025W/.test(request.replace(/\/catalog\/module\/\d+|open=11787/g, ""))) problems.push(`a request carries the study: ${request.slice(0, 160)}`);
  }
  await context.close();
}

// ---------- 3: a phone ----------
{
  const { page, step, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 }, "/");
  await page.evaluate(([key, text]) => localStorage.setItem(key, text), [MINE, THIRD]);
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  await step("the bottom bar", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-head"));
  const phone = await page.evaluate(() => ({
    sideways: document.documentElement.scrollWidth - innerWidth,
    sheet: getComputedStyle(document.querySelector("#sidebar")).position,
    toggle: document.querySelector(".st-head .sheet-toggle")?.getBoundingClientRect().height,
    ticks: Math.min(...[...document.querySelectorAll(".st-term.is-now .st-tick")].map((tick) => tick.getBoundingClientRect().height)),
  }));
  check(phone.sideways <= 0 && phone.sheet === "fixed" && phone.toggle >= 40 && phone.ticks >= 44, `the phone: ${JSON.stringify(phone)}`);
  await step("Anpassen", () => page.tap(".st-head .sheet-toggle"), () => document.documentElement.classList.contains("sheet-open"));
  await context.close();
}

// ---------- 4: the Stundenplan takes the current semester from „Mein Studium" ----------
{
  const { page, step, context } = await open({ viewport: { width: 1440, height: 900 } }, "/");
  await page.evaluate(([key, text]) => localStorage.setItem(key, text), [MINE, THIRD]);
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  await step("Stundenplan", () => page.click('.rail a[data-area="studyplan"]'), () => location.pathname === "/studyplan" && document.querySelector(".sp-import"));
  await step("source „Mein Studium“", () => page.click('.sp-import .seg button:has-text("Mein Studium")'), () => document.querySelector('.sp-import .seg button[aria-checked="true"]')?.textContent === "Mein Studium" && document.querySelector("#sp-import-go:not([disabled])"));
  await step("Übernehmen", () => page.click("#sp-import-go"), () => document.querySelector(".sp-import .note-action")?.textContent.startsWith("Übernommen:"));
  const plan = await stored(page, PLAN);
  check(plan.includes("m\t2026W\t12104\t") && plan.includes("p\t"), `the current semester from „Mein Studium“ is not in the timetable: ${JSON.stringify(plan)}`);
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
