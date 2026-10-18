// Checks Back (folia/crates/shell/src/nav.rs; the concept folia/design/zurueck/konzept.html; owner,
// 2026-10-18: „am handy ist das ja eher wie eine App … dass man zurück zum main screen kommt und
// nicht die Seiten rückwärts abzuspielen"; „auf gar keinen Fall darf Android darunter leiden").
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node back.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// A phone as Android has it: Back is a close request to the newest CloseWatcher where something
// lies over the page (Escape is one in Chromium), else a step back in the history (`back`).
// 1 a dialog of „Mein Studium" closes with Back and writes no entry; Forward opens nothing.
// 2 the row of „Mein Studium": the box „Studium planen" turns it to the current semester without a
//   step of the history; the arrow at the head and Back bring the overview; turned three times by
//   ‹ ›, one Back; the entry keeps the semester across a reload.
// 3 the selection first: a long press selects a row, Back lets go of it and the semester stays,
//   the next Back brings the overview.
// 4 the catalog's filters: Back throws the sheet's draft away and writes nothing, „Module anzeigen"
//   applies it as one step, Back leads to the list before it.
// 5 a module's sheet closed by its × goes back a step instead of writing one: Forward brings it, Back
//   from the list does not.
// 6 a stack for each tab: Start under every tab; a change of tabs writes the history in place, the
//   tab left comes back as it was, Back leads up through the tab to Start; Forward does not lead into
//   what a change of tabs left over; the tab tapped again leads to its first page.
// 7 a link to a page of another tab (a module's program) changes the tab, what lies above the page
//   under it.
// 8 a link from outside: while it is only read, Back leaves Betula; its first use (the arrow) lays
//   the list and Start under it.
// An iPhone (no CloseWatcher): Back is the history; a dialog goes with the page it leaves.
// A wide screen: filter clicks closer than a second are one step, after a pause another; the
// module beside the list closed by its × goes back where the list was the step before.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
let during = "";
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
const THIRD = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n";
// The first semesters as „nach Regelstudienplan füllen" leaves them, two modules of the second
// planned into the current one.
const FILLED = [
  "m\t2025W\t12104\t1\t\tplan", "m\t2025W\t12107\t1\t\tplan", "m\t2025W\t12102\t1\t\tplan", "m\t2025W\t11112\t1\t\tplan",
  "m\t2026S\t11113\t1\t\tplan", "m\t2026W\t12101\t1\t\tplan", "m\t2026W\t11903\t1\t\tplan",
].join("\n") + "\n";
const PHONE = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 };

const takeover = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
const open = async (options, path, { iphone = false } = {}) => {
  const context = await browser.newContext(options);
  // An iPhone: Safari has no CloseWatcher.
  if (iphone) await context.addInitScript(() => { delete window.CloseWatcher; });
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`console (${during}): ` + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push(`pageerror (${during}): ` + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  const step = async (name, action, until, arg) => {
    during = name;
    await action();
    try {
      await page.waitForFunction(until, arg, { timeout: 8000 });
      await page.waitForFunction(() => !document.querySelector(".pending-page.held"), null, { timeout: 2000 });
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
  };
  const seed = async (entries) => {
    await page.evaluate((entries) => { for (const [key, text] of entries) localStorage.setItem(key, text); }, entries);
    await page.reload({ waitUntil: "domcontentloaded" });
    await takeover(page);
    await page.evaluate(() => { window.__marker = 1; });
  };
  const cdp = await context.newCDPSession(page);
  const touch = (type, x, y) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x: Math.round(x), y: Math.round(y), id: 1 }] });
  return { page, step, seed, touch, context };
};
// Where the history is: the entries' addresses and the current one (the Navigation API).
const where = (page) => page.evaluate(() => {
  const entries = navigation.entries().map((entry) => { const url = new URL(entry.url); return url.pathname + url.search; });
  return { entries, at: navigation.currentEntry.index, length: entries.length, here: location.pathname + location.search };
});
// What lies over the page (nav.rs marks it on the root; the filters' sheet; a modal dialog).
const over = (page) => page.evaluate(() => {
  const root = document.documentElement;
  return root.hasAttribute("data-layers") || root.hasAttribute("data-state") || root.classList.contains("sheet-open") || !!document.querySelector("dialog:modal");
});
// Back as Android has it: a close request where something lies over the page, else the history.
const back = async (page) => {
  if (await over(page)) await page.keyboard.press("Escape");
  else await page.evaluate(() => history.back());
};
const middle = (page, selector) => page.$eval(selector, (el) => { const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + Math.min(r.height / 2, 20) }; });
// The row of „Mein Studium" rests at the semester `name` (`null`: the overview).
const resting = (name) => new Function(`
  const here = document.querySelector(".st-pager > .st-slot[data-here]");
  if (!here || document.querySelector(".st-pager.is-gliding")) return false;
  return ${name === null ? `here.classList.contains("is-overview")` : `here.querySelector("h2")?.textContent === ${JSON.stringify(name)}`};
`);

const part = async (name, run) => {
  try { await run(); } catch (error) { problems.push(`${name} (${during}): ${String(error).split("\n")[0]}`); }
};

// ---------- a phone, as Android has it ----------
await part("Android", async () => {
  const { page, step, seed, touch } = await open(PHONE, "/");
  await seed([[MINE, THIRD], [PLAN, FILLED]]);
  await step("„Studium“", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-planbox"));
  const start = await where(page);

  // 1 a dialog
  await step("„Studiengang wechseln“", () => page.tap("#st-mine"), () => document.querySelector(".st-dialog[open]"));
  check((await where(page)).length === start.length, "the dialog wrote an entry");
  await step("Back closes the dialog", () => back(page), () => !document.querySelector(".st-dialog[open]") && location.pathname === "/study");
  const afterDialog = await where(page);
  check(afterDialog.at === start.at && afterDialog.length === start.length, `Back on the dialog moved the history: ${JSON.stringify(afterDialog)}`);
  check(!(await page.evaluate(() => navigation.canGoForward)), "after the dialog Forward could open something");

  // 2 the row
  await step("„Studium planen“", () => page.tap(".st-planbox-head"), resting("WiSe 2026/27"));
  let now = await where(page);
  check(now.here === "/study" && now.length === start.length && now.at === start.at, `the semester wrote a step of the history or its address: ${JSON.stringify(now)}`);
  check((await page.evaluate(() => history.state?.["betula.semester"])) === "2026W", "the entry does not keep the semester");
  check(await page.evaluate(() => document.documentElement.hasAttribute("data-state")), "the semester is no state Back undoes");
  await step("the arrow at the head", () => page.tap(".top-back.on"), resting(null));
  check((await where(page)).at === start.at, "the arrow left the overview's entry");
  check((await page.evaluate(() => history.state?.["betula.semester"])) === undefined, "the overview's entry still keeps a semester");
  // Three turns by ‹ ›, then one Back.
  await step("„Studium planen“ again", () => page.tap(".st-planbox-head"), resting("WiSe 2026/27"));
  for (const name of ["SoSe 2027", "WiSe 2027/28", "SoSe 2028"]) {
    await step(`› to ${name}`, () => page.tap(".st-pager > .st-slot[data-here] .st-turn >> nth=1"), resting(name));
  }
  now = await where(page);
  check(now.length === start.length && now.at === start.at, `turning the row wrote steps of the history: ${JSON.stringify(now)}`);
  await step("one Back after three turns", () => back(page), resting(null));
  check((await where(page)).at === start.at, "Back after the turns left the overview");
  // A reload keeps the semester the entry names.
  await step("„Studium planen“, the third time", () => page.tap(".st-planbox-head"), resting("WiSe 2026/27"));
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  await page.waitForFunction(resting("WiSe 2026/27"), null, { timeout: 8000 }).catch(() => problems.push("a reload did not keep the semester"));

  // 3 the selection first
  const row = await middle(page, ".st-pager > .st-slot[data-here] .st-row .st-what");
  await step("a long press", async () => {
    await touch("touchStart", row.x, row.y);
    await page.waitForTimeout(700);
    await touch("touchEnd", 0, 0);
  }, () => document.querySelector(".st-selbar"));
  await step("Back lets go of the selection", () => back(page), new Function(`return !document.querySelector(".st-selbar") && (${resting("WiSe 2026/27").toString()})()`));
  await step("the next Back: the overview", () => back(page), resting(null));
  check((await where(page)).at === start.at, "the selection or the semester moved the history");

  // 4 the catalog's filters
  await step("„Module“", () => page.tap('.bottomnav a.nav[data-area="catalog"]'), () => location.pathname === "/catalog" && document.querySelector(".vlist"));
  const list = await where(page);
  const sheet = () => page.evaluate(() => document.getElementById("filters")?.classList.contains("open"));
  const pick = async (label) => {
    const shown = await page.$eval("#filters .show", (el) => el.textContent);
    await page.tap(`#filters a.chip:has-text("${label}")`);
    await page.waitForFunction((shown) => document.querySelector("#filters .show")?.textContent !== shown, shown, { timeout: 5000 }).catch(() => problems.push(`a tap on ${label} did not change the count in the sheet`));
  };
  await step("the filters", () => page.tap(".sheet-toggle"), () => document.getElementById("filters")?.classList.contains("open"));
  check((await where(page)).length === list.length, "the filters' sheet wrote an entry");
  await pick("Winter");
  await step("Back throws the draft away", () => back(page), () => !document.getElementById("filters").classList.contains("open"));
  now = await where(page);
  check(now.here === list.here && now.length === list.length && now.at === list.at, `Back on the sheet applied the draft or moved the history: ${JSON.stringify(now)}`);
  await step("the filters again", () => page.tap(".sheet-toggle"), () => document.getElementById("filters")?.classList.contains("open"));
  check(!(await page.$eval('#filters a.chip:has-text("Winter")', (el) => el.dataset.state === "with")), "the sheet opened with the draft thrown away");
  await pick("Winter");
  await step("„Module anzeigen“", () => page.tap("#filters .show"), () => /turnus=winter/.test(location.search) && !document.getElementById("filters").classList.contains("open"));
  now = await where(page);
  check(now.length === list.length + 1 && now.at === list.at + 1, `„Module anzeigen“ is not one step: ${JSON.stringify(now)}`);
  await step("Back to the list before", () => back(page), () => location.pathname === "/catalog" && !/turnus/.test(location.search));
  check(!(await sheet()), "Back opened the sheet");

  // 5 a module's sheet
  await step("a module", () => page.tap(".rows a.row >> nth=0"), () => /[?&]open=/.test(location.search) && document.querySelector(".detail.is-module h2"));
  const opened = await where(page);
  await step("its ×", () => page.tap('.detail.is-module [data-action="close-detail"]'), () => !/[?&]open=/.test(location.search) && !document.querySelector(".detail.is-module:not(.is-gone)"));
  now = await where(page);
  check(now.at === opened.at - 1 && now.length === opened.length, `the × did not go back a step: ${JSON.stringify(opened)} → ${JSON.stringify(now)}`);
  check(await page.evaluate(() => navigation.canGoForward), "after the × Forward cannot bring the module");
});

// ---------- a stack for each tab ----------
await part("tabs", async () => {
  const { page, step, seed } = await open(PHONE, "/");
  await seed([[MINE, THIRD], [PLAN, FILLED]]);
  const stack = async () => { const now = await where(page); return now.entries.slice(0, now.at + 1).join(" "); };
  await step("„Studium“", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-planbox"));
  check((await stack()) === "/ /study", `Start is not under „Studium“: ${await stack()}`);
  await step("„Alle Studiengänge“", () => page.tap('.st-rest a[href="/programs"]'), () => location.pathname === "/programs");
  await step("„Module“", () => page.tap('.bottomnav a.nav[data-area="catalog"]'), () => location.pathname === "/catalog" && document.querySelector(".vlist"));
  const catalog = await where(page);
  check(catalog.at === 1 && catalog.entries[0] === "/", `the change of tabs did not write „Module“ above Start: ${JSON.stringify(catalog)}`);
  await step("Forward into what is left over", () => page.evaluate(() => history.forward()), () => location.pathname === "/catalog");
  await page.waitForTimeout(400);
  check((await where(page)).here.startsWith("/catalog"), "Forward led into what the change of tabs left over");
  await step("„Studium“ again", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/programs");
  check((await stack()) === "/ /study /programs", `„Studium“ did not come back as it was left: ${await stack()}`);
  await step("Back: „Mein Studium“", () => page.evaluate(() => history.back()), () => location.pathname === "/study" && document.querySelector(".st-planbox"));
  await step("Back: Start", () => page.evaluate(() => history.back()), () => location.pathname === "/");
  check((await where(page)).at === 0, "Start is not the first entry");
  await step("Forward", () => page.evaluate(() => history.forward()), () => location.pathname === "/study");
  await step("Forward", () => page.evaluate(() => history.forward()), () => location.pathname === "/programs");
  await step("the tab tapped again", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-planbox"));
  check((await where(page)).at === 1, "the tab tapped again did not lead back to its first page");
  // 7 a link to a page of another tab
  await step("„Module“ once more", () => page.tap('.bottomnav a.nav[data-area="catalog"]'), () => location.pathname === "/catalog" && document.querySelector(".rows a.row"));
  await step("a module", () => page.tap(".rows a.row >> nth=0"), () => /[?&]open=/.test(location.search) && document.querySelector('.detail.is-module a[href^="/programs/"]'));
  const program = await page.$eval('.detail.is-module a[href^="/programs/"]', (a) => new URL(a.href).pathname);
  await step("its program", () => page.tap('.detail.is-module a[href^="/programs/"] >> nth=0'), (program) => location.pathname === program, program);
  check((await stack()) === `/ /study /programs ${program}`, `the program did not come with „Mein Studium“ and „Alle Studiengänge“ under it: ${await stack()}`);
  check(await page.evaluate(() => document.querySelector('.bottomnav a.nav[data-area="programs"]')?.getAttribute("aria-current") === "page"), "the program is not in the tab „Studium“");
  await step("Back: „Alle Studiengänge“", () => page.evaluate(() => history.back()), () => location.pathname === "/programs");
  await step("Back: „Mein Studium“", () => page.evaluate(() => history.back()), () => location.pathname === "/study");
  await step("Back: Start", () => page.evaluate(() => history.back()), () => location.pathname === "/");
  await step("„Module“: the module as it was left", () => page.tap('.bottomnav a.nav[data-area="catalog"]'), () => /[?&]open=/.test(location.search));
});

// ---------- a link from outside ----------
await part("a link from outside", async () => {
  {
    const { page } = await open(PHONE, "/catalog/module/12104");
    during = "only read";
    check((await where(page)).at === 0, "the link from outside is not the first entry");
    await page.evaluate(() => history.back());
    await page.waitForFunction(() => location.href === "about:blank", null, { timeout: 5000 }).catch(() => problems.push(`Back on a link only read did not leave Betula (${page.url()})`));
  }
  const { page, step } = await open(PHONE, "/catalog/module/12104");
  await step("the arrow, its first use", () => page.tap(".top-back.on"), () => location.pathname === "/catalog");
  const now = await where(page);
  check(now.entries.slice(0, 3).map((url) => url.split("?")[0]).join(" ") === "/ /catalog /catalog/module/12104" && now.at === 1, `the first use did not lay the list and Start under the page: ${JSON.stringify(now)}`);
  await step("Back: Start", () => page.evaluate(() => history.back()), () => location.pathname === "/");
});

// ---------- an iPhone: no CloseWatcher, Back is the history ----------
await part("iPhone", async () => {
  const { page, step, seed } = await open(PHONE, "/", { iphone: true });
  await seed([[MINE, THIRD], [PLAN, FILLED]]);
  check(await page.evaluate(() => typeof window.CloseWatcher === "undefined"), "the iPhone has a CloseWatcher");
  await step("„Studium“", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-planbox"));
  await step("„Alle Studiengänge“", () => page.tap('.st-rest a[href="/programs"]'), () => location.pathname === "/programs");
  await step("back to „Mein Studium“", () => page.tap(".top-back.on"), () => location.pathname === "/study" && document.querySelector(".st-planbox"));
  await step("forward to „Alle Studiengänge“", () => page.evaluate(() => history.forward()), () => location.pathname === "/programs");
  await step("back again", () => page.evaluate(() => history.back()), () => location.pathname === "/study" && document.querySelector("#st-mine"));
  await step("„Studiengang wechseln“", () => page.tap("#st-mine"), () => document.querySelector(".st-dialog[open]"));
  await step("the swipe from the edge leaves the page, the dialog goes with it", () => page.evaluate(() => history.back()), () => location.pathname !== "/study" && !document.querySelector(".st-dialog[open]"));
});

// ---------- a wide screen ----------
await part("wide", async () => {
  const { page, step } = await open({ viewport: { width: 1280, height: 900 } }, "/catalog");
  await page.waitForFunction(() => document.querySelector(".vlist") && document.querySelector("#filters a.chip"), null, { timeout: 20000 });
  const first = await where(page);
  // Two filters closer together than a second: one step; after a pause another.
  await step("two filters at once", async () => {
    await page.click('#filters a.chip:has-text("Winter")');
    await page.waitForTimeout(250);
    await page.click('#filters a.chip:has-text("Vorlesung")');
  }, () => /turnus=winter/.test(location.search) && /form=lecture/.test(location.search));
  let now = await where(page);
  check(now.length === first.length + 1, `two quick filters are not one step: ${JSON.stringify(now)}`);
  await page.waitForTimeout(1300);
  await step("a filter after a pause", () => page.click('#filters a.chip:has-text("Übung")'), () => /exercise/.test(location.search));
  now = await where(page);
  check(now.length === first.length + 2, `a filter after a pause is no step of its own: ${JSON.stringify(now)}`);
  await step("Back: the two filters", () => page.evaluate(() => history.back()), () => /turnus=winter/.test(location.search) && !/exercise/.test(location.search));
  await step("Back: none", () => page.evaluate(() => history.back()), () => !/turnus/.test(location.search));
  // The module beside the list, closed by its ×: back where the list was the step before.
  await step("a module beside the list", () => page.click(".rows a.row >> nth=1"), () => /[?&]open=/.test(location.search) && document.querySelector("#preview h2"));
  const opened = await where(page);
  await step("its ×", () => page.click('#preview [data-action="close-detail"]'), () => !/[?&]open=/.test(location.search));
  now = await where(page);
  check(now.at === opened.at - 1, `the × wrote a step instead of going back: ${JSON.stringify(opened)} → ${JSON.stringify(now)}`);
});

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
