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
//   the next Back brings the overview; the arrow at the head the same.
// 4 the catalog's filters: Back throws the sheet's draft away and writes nothing, „Module anzeigen"
//   applies it as one step, Back leads to the list before it.
// 5 a module's sheet closed by its × goes back a step instead of writing one: Forward brings it, Back
//   from the list does not.
// 6 a stack for each tab: Start under every tab; a change of tabs writes the history in place, the
//   tab left comes back as it was, Back leads up through the tab to Start; Forward does not lead into
//   what a change of tabs left over; the tab tapped again leads to its first page; Start is
//   the first entry, and its tab leads nowhere else; the profile lies on top of the tab it was
//   opened from, and a tab left from it keeps its stack without it.
// 6b the profile is a page of a while: the arrow and Back lead to the page under it as it was left
//   (a list with a filter, scrolled down), and so does the tab under it; from Start back to Start;
//   come to from outside, its arrow leads to Start.
// 7 a link to a page of another tab (a module's program) changes the tab, what lies above the page
//   under it.
// 8 a link from outside: while it is only read, Back leaves Betula; its first use (the arrow) lays
//   the list and Start under it.
// 9 the Stundenplan's sheets on a phone („Kalender", „Export", „Teilen …" over „Export", „+ Modul" over
//   „Kalender"): each closes with one Back and writes no entry, Forward opens nothing, the page stays;
//   what lay over another sheet goes back to it, the keyboard on the row that opened it.
// 10 „Module planen in „Mein Studium““ of the calendar (`data-lend`; owner, 2026-11-18): the page is
//   lent on top of the calendar's stack, the bar shows „Studium“; one Back (past the semester the link
//   opened) and the arrow lead to the calendar as it was left (`edit=1`), Forward brings the page
//   again; the semester's menu closes first, a semester turned to by › brings the overview first,
//   the workbench's sheet ended by a tap beside it is no Back, and after each the calendar; „Stundenplan“ tapped leads
//   there too; „Studium“ tapped again ends the loan as it opens the area as it starts, the calendar's
//   tab keeping its stack; a further entry („Alle Studiengänge“) ends it, Back then leads up
//   through „Mein Studium“ to Start. An iPhone (Back and the arrow) and a wide screen: Back is the
//   history, the calendar.
// An iPhone (no CloseWatcher): Back is the history; a dialog goes with the page it leaves.
// A wide screen: filter clicks closer than a second are one step, after a pause another; the
// module beside the list closed by its × goes back where the list was the step before, and so does
// the tab of the list; after two steps back at once a module is a step of its own with its address;
// after a reload, whose page keeps the entries it wrote, the × still goes back, and nothing loads
// the page anew.
// On a phone every new entry comes with a tap (`navigator.userActivation`): Chrome's Back skips one
// a page added without one.
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
  // Every new entry comes with a tap (`navigator.userActivation`): Chrome's Back skips an entry a
  // page added without one (its history manipulation intervention), which Playwright's own steps
  // through the history would never show.
  await context.addInitScript(() => {
    window.__untapped = [];
    const push = history.pushState.bind(history);
    history.pushState = (state, title, url) => {
      if (!navigator.userActivation.isActive) window.__untapped.push(String(url));
      return push(state, title, url);
    };
  });
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`console (${during}): ` + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push(`pageerror (${during}): ` + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  // The phone's steps are taps; the wide screen's clicks through the page are none.
  const taps = Boolean(options.hasTouch);
  const step = async (name, action, until, arg) => {
    during = name;
    await action();
    try {
      await page.waitForFunction(until, arg, { timeout: 8000 });
      // And for the router: until it takes the step (`main` busy) the links of the page before are there.
      await page.waitForFunction(() => !document.querySelector(".pending-page.held") && document.getElementById("content")?.getAttribute("aria-busy") !== "true", null, { timeout: 2000 });
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
    const untapped = await page.evaluate(() => window.__untapped.splice(0));
    if (taps && untapped.length) problems.push(`${name}: a new entry without a tap (${untapped.join(", ")})`);
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
  const row = await middle(page, ".st-pager > .st-slot[data-here] .st-row .lp");
  await step("a long press", async () => {
    await touch("touchStart", row.x, row.y);
    await page.waitForTimeout(700);
    await touch("touchEnd", 0, 0);
  }, () => document.querySelector(".st-selbar"));
  await step("Back lets go of the selection", () => back(page), new Function(`return !document.querySelector(".st-selbar") && (${resting("WiSe 2026/27").toString()})()`));
  await step("the next Back: the overview", () => back(page), resting(null));
  check((await where(page)).at === start.at, "the selection or the semester moved the history");
  // The arrow at the head is Back as well: the selection first, the semester staying; then the overview.
  await step("„Studium planen“ for the arrow", () => page.tap(".st-planbox-head"), resting("WiSe 2026/27"));
  const again = await middle(page, ".st-pager > .st-slot[data-here] .st-row .lp");
  await step("a long press for the arrow", async () => {
    await touch("touchStart", again.x, again.y);
    await page.waitForTimeout(700);
    await touch("touchEnd", 0, 0);
  }, () => document.querySelector(".st-selbar"));
  await step("the arrow lets go of the selection", () => page.tap(".top-back.on"), new Function(`return !document.querySelector(".st-selbar") && (${resting("WiSe 2026/27").toString()})()`));
  await step("the arrow again: the overview", () => page.tap(".top-back.on"), resting(null));
  check((await where(page)).at === start.at, "the arrow moved the history");

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
  // Closed as its × closes it (out of sight on a phone, the grabber its one decoration: owner,
  // 2026-10-27): a tap beside it.
  await page.waitForTimeout(400);
  await step("a tap beside it", () => page.touchscreen.tap(195, 150), () => !/[?&]open=/.test(location.search) && !document.querySelector(".detail.is-module:not(.is-gone)"));
  now = await where(page);
  check(now.at === opened.at - 1 && now.length === opened.length, `closing it did not go back a step: ${JSON.stringify(opened)} → ${JSON.stringify(now)}`);
  check(await page.evaluate(() => navigation.canGoForward), "after closing it Forward cannot bring the module");
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
  // The profile is a page of a while (owner, 2026-10-19): a step on top of the tab it was opened
  // from, and a tab left from it keeps its stack without it. Start is the entry under every tab and
  // leads nowhere else.
  await step("Back: the list", () => page.evaluate(() => history.back()), () => !/[?&]open=/.test(location.search) && !document.querySelector(".detail.is-module"));
  const list = await where(page);
  await step("the menu of the profiles", () => page.tap(".topbar .top-profile"), () => Boolean(document.querySelector(".profile-menu")));
  await step("the profile", () => page.tap('.profile-menu a[href="/profile"]'), () => location.pathname === "/profile");
  check((await stack()) === `${list.entries.slice(0, list.at + 1).join(" ")} /profile`, `the profile is not on top of „Module“: ${await stack()}`);
  await step("„Studium“ from the profile", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/study");
  await step("„Module“ without the profile", () => page.tap('.bottomnav a.nav[data-area="catalog"]'), (here) => location.pathname + location.search === here, list.here);
  check((await stack()) === list.entries.slice(0, list.at + 1).join(" "), `„Module“ came back with the profile on it: ${await stack()}`);
  await step("Start", () => page.tap('.bottomnav a.nav[data-area="home"]'), () => location.pathname === "/");
  check((await where(page)).at === 0, `Start is not the first entry: ${JSON.stringify(await where(page))}`);
});

// ---------- the profile, a page of a while ----------
// (owner, 2026-10-19: „dass die Profile page, als temporäre seite getreated werden soll wo man dann
// oben den zurück pfeil hat und genau wieder da landet, wo man davor war")
await part("the profile", async () => {
  const { page, step } = await open(PHONE, "/");
  const toProfile = (name) => step(name, async () => {
    await page.tap(".topbar .top-profile");
    await page.waitForSelector('.profile-menu a[href="/profile"]', { timeout: 3000 });
    await page.tap('.profile-menu a[href="/profile"]');
  }, () => location.pathname === "/profile" && document.querySelector(".top-back.on"));
  // A list two levels deep: the catalog, and the catalog with a filter applied, scrolled down.
  await step("„Module“", () => page.tap('.bottomnav a.nav[data-area="catalog"]'), () => location.pathname === "/catalog" && document.querySelector(".rows a.row"));
  await step("the filters", () => page.tap(".sheet-toggle"), () => document.getElementById("filters")?.classList.contains("open"));
  const shown = await page.$eval("#filters .show", (el) => el.textContent);
  await page.tap('#filters a.chip:has-text("Winter")');
  await page.waitForFunction((shown) => document.querySelector("#filters .show")?.textContent !== shown, shown, { timeout: 5000 }).catch(() => problems.push("a tap on Winter did not change the count in the sheet"));
  await step("„Module anzeigen“", () => page.tap("#filters .show"), () => /turnus=winter/.test(location.search) && !document.getElementById("filters").classList.contains("open"));
  await page.evaluate(() => window.scrollTo(0, 1500));
  await page.waitForTimeout(300);
  const list = { ...(await where(page)), y: await page.evaluate(() => Math.round(scrollY)) };
  check(list.at === 2 && list.y > 1000, `the list is not two levels deep and scrolled: ${JSON.stringify(list)}`);
  const scrolled = () => page.evaluate(() => Math.round(scrollY));
  // The arrow: back to the list as it was left, the same entry, scrolled as far.
  await toProfile("the profile over the list");
  check((await where(page)).at === list.at + 1, `the profile is not one step on top: ${JSON.stringify(await where(page))}`);
  await step("the arrow: the list", () => page.tap(".top-back.on"), (here) => location.pathname + location.search === here, list.here);
  check((await where(page)).at === list.at && Math.abs((await scrolled()) - list.y) < 4, `the arrow did not go back to the list as it was left: ${JSON.stringify(await where(page))}, scrolled ${await scrolled()}`);
  // Back the same.
  await toProfile("the profile again");
  await step("Back: the list", () => back(page), (here) => location.pathname + location.search === here, list.here);
  check((await where(page)).at === list.at && Math.abs((await scrolled()) - list.y) < 4, `Back did not go back to the list as it was left: ${JSON.stringify(await where(page))}, scrolled ${await scrolled()}`);
  // The tab under it: the tab as it was left, not its first page.
  await toProfile("the profile once more");
  await step("„Module“ from the profile: the list as it was left", () => page.tap('.bottomnav a.nav[data-area="catalog"]'), (here) => location.pathname + location.search === here, list.here);
  check((await where(page)).at === list.at, `„Module“ from the profile did not lead to the list as it was left: ${JSON.stringify(await where(page))}`);
  // From Start: back to Start.
  await step("Start", () => page.tap('.bottomnav a.nav[data-area="home"]'), () => location.pathname === "/");
  await toProfile("the profile from Start");
  await step("the arrow: Start", () => page.tap(".top-back.on"), () => location.pathname === "/");
  check((await where(page)).at === 0, `the arrow did not lead back to Start: ${JSON.stringify(await where(page))}`);
  // The page come to from outside: the arrow lays Start under it and leads there.
  const { page: outside, step: outsideStep } = await open(PHONE, "/profile");
  check(await outside.evaluate(() => Boolean(document.querySelector(".top-back.on")) && document.querySelector(".top-back").getAttribute("href") === "/"), "the profile come to from outside has no arrow to Start");
  await outsideStep("the arrow from outside: Start", () => outside.tap(".top-back.on"), () => location.pathname === "/");
});

// ---------- the Stundenplan's sheets on a phone, as Android has it ----------
// A sheet lies over the page: it has no entry of the history, and Back as Android has it (a close
// request) closes it — „Kalender", „Export", „Teilen …" over „Export", which goes back to „Export",
// and „+ Modul" of a calendar of one's own over „Kalender", which goes back to „Kalender"; each writes
// no entry, Forward opens nothing, the page stays /studyplan.
await part("the Stundenplan's sheets", async () => {
  const { page, step, seed } = await open(PHONE, "/");
  await seed([[MINE, THIRD], [PLAN, FILLED + "l\t1\t\t013-82-2018\tLena\nn\t1\t2026W\t11109\n"]]);
  await step("„Stundenplan“", () => page.tap('.bottomnav a.nav[data-area="studyplan"]'), () => location.pathname === "/studyplan" && document.querySelector("#content > .page-stage.front .sp-tool[data-action='calendars']"));
  const start = await where(page);
  const tool = (action) => `#content > .page-stage.front .sp-tool[data-action='${action}']`;
  const stays = async (name) => {
    const now = await where(page);
    check(now.here === "/studyplan" && now.length === start.length && now.at === start.at, `${name}: the history moved: ${JSON.stringify(now)}`);
    check(!(await page.evaluate(() => navigation.canGoForward)), `${name}: Forward could open something`);
    check(!(await over(page)), `${name}: something still lies over the page`);
  };
  await step("„Kalender“", () => page.tap(tool("calendars")), () => document.querySelector("dialog.sp-cal-sheet[open]"));
  check((await where(page)).length === start.length, "the calendars' sheet wrote an entry");
  await step("Back closes „Kalender“", () => back(page), () => !document.querySelector("dialog.sp-cal-sheet[open]"));
  await stays("„Kalender“");
  await step("„Export“", () => page.tap(tool("export")), () => document.querySelector("dialog.sp-export-sheet[open]"));
  await step("Back closes „Export“", () => back(page), () => !document.querySelector("dialog.sp-export-sheet[open]"));
  await stays("„Export“");
  await step("„Export“ again", () => page.tap(tool("export")), () => document.querySelector("dialog.sp-export-sheet[open] #sp-share"));
  await step("„Teilen …“ over it", () => page.tap("dialog.sp-export-sheet[open] #sp-share"), () => document.querySelector("dialog.sh-dialog[open]") && !document.querySelector("dialog.sp-export-sheet[open]"));
  check((await where(page)).length === start.length, "„Teilen“ wrote an entry");
  await step("Back: „Teilen“ goes, „Export“ comes back", () => back(page), () => !document.querySelector("dialog.sh-dialog[open]") && document.querySelector("dialog.sp-export-sheet[open]") && document.activeElement?.id === "sp-share");
  await step("Back closes „Export“ then", () => back(page), () => !document.querySelector("dialog[open]"));
  await stays("„Teilen“ over „Export“");
  await step("„Kalender“ for „Modul aufnehmen“", () => page.tap(tool("calendars")), () => document.querySelector("dialog.sp-cal-sheet[open] .sp-cal-more"));
  await page.tap("dialog.sp-cal-sheet[open] .sp-cal-more");
  await step("„Modul aufnehmen“", () => page.tap("dialog.sp-cal-sheet[open] .sp-cal-menu > button >> nth=0"), () => document.querySelector("dialog.sp-add-sheet[open] .sp-addp") && !document.querySelector("dialog.sp-cal-sheet[open]"));
  await step("Back: „+ Modul“ goes, „Kalender“ comes back", () => back(page), () => !document.querySelector("dialog.sp-add-sheet[open]") && document.querySelector("dialog.sp-cal-sheet[open]"));
  await step("Back closes „Kalender“ then", () => back(page), () => !document.querySelector("dialog[open]"));
  await stays("„+ Modul“ over „Kalender“");
});

// ---------- the calendar's way into „Mein Studium", a page lent ----------
// (owner, 2026-11-18: „wenn man vom Kalender auf Module planan in geht, dann kommt man mit back nicht
// wieder zum Kalender zurück. Mach das so, dass man so lange man da nichts weiteres in die back
// history macht, dass dann der Terminkalender das back ist.")
const lentParts = async (name, options, { android, iphone = false }) => part(name, async () => {
  const { page, step, seed } = await open(options, "/", { iphone });
  await seed([[MINE, THIRD], [PLAN, FILLED]]);
  const stack = async () => { const now = await where(page); return now.entries.slice(0, now.at + 1).join(" "); };
  const front = "#content > .page-stage.front";
  // The row of „Mein Studium“ rests at a semester, not the overview.
  const away = () => { const here = document.querySelector(".st-pager > .st-slot[data-here]"); return Boolean(here) && !here.classList.contains("is-overview") && !document.querySelector(".st-pager.is-gliding"); };
  const tapOrClick = (selector) => (options.hasTouch ? page.tap(selector) : page.click(selector));
  await step("„Stundenplan“", () => tapOrClick(`a.nav[data-area="studyplan"]:visible`), () => location.pathname === "/studyplan" && document.querySelector("#content > .page-stage.front a.sp-way"));
  await step("„Bearbeiten“", () => tapOrClick(`${front} .sp-edit-toggle`), () => /[?&]edit=1/.test(location.search) && document.querySelector("#content > .page-stage.front a.sp-way"));
  const calendar = await where(page);
  const toStudy = (label) => step(label, async () => {
    await page.locator(`${front} a.sp-way`).scrollIntoViewIfNeeded();
    await tapOrClick(`${front} a.sp-way`);
  }, () => location.pathname === "/study");
  const backAtCalendar = async (label, how) => {
    await step(label, how, (here) => location.pathname + location.search === here && document.querySelector("#content > .page-stage.front a.sp-way"), calendar.here);
    const now = await where(page);
    check(now.at === calendar.at && now.entries[now.at] === calendar.here, `${label}: not the calendar as it was left: ${JSON.stringify(calendar)} → ${JSON.stringify(now)}`);
  };
  await toStudy("„Module planen in „Mein Studium““");
  check((await stack()) === `${calendar.entries.slice(0, calendar.at + 1).join(" ")} /study?plan=1`, `the page is not lent on top of the calendar: ${await stack()}`);
  if (options.hasTouch) check(await page.evaluate(() => document.querySelector('.bottomnav a.nav[data-area="programs"]')?.getAttribute("aria-current") === "page"), "the bar does not show „Studium“ as current");
  if (!android) {
    // An iPhone and a wide screen: Back is the history.
    await backAtCalendar("Back: the calendar", () => page.evaluate(() => history.back()));
    if (iphone) {
      await toStudy("the way again, for the arrow");
      await backAtCalendar("the arrow: the calendar", () => page.tap(".top-back.on"));
    }
    return;
  }
  // One Back, past the semester the link itself opened.
  await page.waitForFunction(resting("WiSe 2026/27"), null, { timeout: 8000 }).catch(() => problems.push("the link did not open the current semester"));
  await backAtCalendar("one Back: the calendar", () => back(page));
  check(await page.evaluate(() => navigation.canGoForward), "Forward does not bring „Mein Studium“ again");
  // The arrow at the head the same.
  await toStudy("the way again, for the arrow");
  await page.waitForFunction(away, null, { timeout: 8000 }).catch(() => {});
  await backAtCalendar("the arrow: the calendar", () => page.tap(".top-back.on"));
  // What the visitor opens there closes first: the semester's menu, then Back leads to the calendar.
  await toStudy("the way, for a menu");
  await page.waitForFunction(away, null, { timeout: 8000 }).catch(() => {});
  await step("the semester's menu", () => page.tap(".st-pager > .st-slot[data-here] .st-sem-more"), () => document.documentElement.hasAttribute("data-layers") || document.querySelector("dialog:modal"));
  await step("Back closes the menu", () => back(page), () => !document.documentElement.hasAttribute("data-layers") && !document.querySelector("dialog:modal") && location.pathname === "/study");
  await backAtCalendar("Back after the menu: the calendar", () => back(page));
  // A semester turned to by ›: Back brings the overview, the Back after it the calendar.
  await toStudy("the way, for a turn");
  await page.waitForFunction(away, null, { timeout: 8000 }).catch(() => {});
  await step("› to the next semester", () => page.tap(".st-pager > .st-slot[data-here] .st-turn >> nth=1"), resting("SoSe 2027"));
  await step("Back: the overview", () => back(page), resting(null));
  await backAtCalendar("Back after the overview: the calendar", () => back(page));
  // The workbench's sheet closed by a tap beside it (a phone's workbench has no „Fertig“; the
  // tap beside it is its end, no Back): the Back after it leads to the calendar.
  await toStudy("the way, for the workbench");
  await page.waitForFunction(away, null, { timeout: 8000 }).catch(() => problems.push("the way did not open a semester for the workbench"));
  await step("„Module hinzufügen“", () => page.tap(".st-pager > .st-slot[data-here] :is(.st-add, .st-empty-add)"), () => document.querySelector(".st-wb-sheet"));
  await page.waitForTimeout(400);
  await step("a tap beside the workbench", () => page.touchscreen.tap(195, 60), () => !document.querySelector(".st-wb-sheet") && location.pathname === "/study");
  await backAtCalendar("Back after the workbench: the calendar", () => back(page));
  // „Stundenplan“ tapped from the page lent: the calendar as it was left.
  await toStudy("the way, for the tab");
  await backAtCalendar("„Stundenplan“: the calendar", () => page.tap('.bottomnav a.nav[data-area="studyplan"]'));
  // „Studium“ tapped again (Folia 1.105.0): the area opens as it starts, the loan ends; the
  // calendar's tab keeps its stack without the page lent.
  await toStudy("the way, for „Studium“ again");
  await step("„Studium“ tapped again", () => page.tap('.bottomnav a.nav[data-area="programs"]'), () => location.pathname === "/study" && !location.search && document.querySelector(".st-planbox"));
  check((await stack()) === "/ /study", `„Studium“ tapped again did not write its own stack: ${await stack()}`);
  await step("„Stundenplan“ after it", () => page.tap('.bottomnav a.nav[data-area="studyplan"]'), (here) => location.pathname + location.search === here, calendar.here);
  check((await stack()) === calendar.entries.slice(0, calendar.at + 1).join(" "), `the calendar did not come back as it was left: ${await stack()}`);
  // A further entry in „Mein Studium“ ends the loan: Back leads up through „Mein Studium“ to Start.
  await toStudy("the way, for a further step");
  await page.waitForFunction(away, null, { timeout: 8000 }).catch(() => {});
  // To the overview without a step of the history: a semester turned to by ›, then Back.
  const was = await page.$eval(".st-pager > .st-slot[data-here]", (here) => here.querySelector("h2")?.textContent);
  await step("› for the overview", () => page.tap(".st-pager > .st-slot[data-here] .st-turn >> nth=1"), (was) => {
    const here = document.querySelector(".st-pager > .st-slot[data-here]");
    return here && !document.querySelector(".st-pager.is-gliding") && here.querySelector("h2")?.textContent !== was;
  }, was);
  await step("Back: the overview, the loan kept", () => back(page), resting(null));
  check((await stack()) === `${calendar.entries.slice(0, calendar.at + 1).join(" ")} /study`, `the overview is no longer lent on top of the calendar: ${await stack()}`);
  await step("„Alle Studiengänge“", () => page.tap('.st-rest a[href="/programs"]'), () => location.pathname === "/programs");
  check((await stack()) === "/ /study /programs", `the further step did not end the loan: ${await stack()}`);
  await step("Back: „Mein Studium“", () => back(page), () => location.pathname === "/study");
  await step("Back: Start", () => back(page), () => location.pathname === "/");
  check((await where(page)).at === 0, `Back did not lead up to Start: ${JSON.stringify(await where(page))}`);
  await step("„Stundenplan“: as it was left", () => page.tap('.bottomnav a.nav[data-area="studyplan"]'), (here) => location.pathname + location.search === here, calendar.here);
});
await lentParts("lent, Android", PHONE, { android: true });
await lentParts("lent, iPhone", PHONE, { android: false, iphone: true });
await lentParts("lent, wide", { viewport: { width: 1280, height: 900 } }, { android: false });

// ---------- a link from outside ----------
await part("a link from outside", async () => {
  {
    const { page } = await open(PHONE, "/catalog/module/12104");
    during = "only read";
    check((await where(page)).at === 0, "the link from outside is not the first entry");
    // Back after the script has answered: it leaves the document, and a script still running in
    // it would be cut off with it („Execution context was destroyed").
    await page.evaluate(() => { setTimeout(() => history.back()); });
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
  // A tab whose page as it was left is the step before goes back there: the history does not grow.
  await step("„Studium“ in the rail", () => page.click('.rail a.nav[data-area="programs"]'), () => location.pathname === "/study");
  const away = await where(page);
  await step("„Module“ in the rail, the list the step before", () => page.click('.rail a.nav[data-area="catalog"]'), () => location.pathname === "/catalog" && document.querySelector(".vlist"));
  now = await where(page);
  check(now.at === away.at - 1 && now.length === away.length, `the tab wrote a step instead of going back to the list: ${JSON.stringify(away)} → ${JSON.stringify(now)}`);
  // Two steps back at once (the browser's menu of the history), which the router's own list of
  // steps does not see: a module opened then is a step of its own with its address all the same.
  await step("a module", () => page.click(".rows a.row >> nth=0"), () => /[?&]open=/.test(location.search) && document.querySelector("#preview h2"));
  const one = new URL(page.url()).searchParams.get("open");
  // Through the page: the module beside the list lies over a part of it at this width.
  await step("another", () => page.$eval(".rows a.row >> nth=1", (row) => row.click()), (one) => /[?&]open=/.test(location.search) && new URL(location.href).searchParams.get("open") !== one, one);
  const second = new URL(page.url()).searchParams.get("open");
  await step("two steps back at once", () => page.evaluate(() => history.go(-2)), () => !/[?&]open=/.test(location.search) && !document.querySelector("#preview h2"));
  const list = await where(page);
  await step("the second one again", () => page.$eval(`.rows a.row[data-id="${second}"]`, (row) => row.click()), (second) => new URL(location.href).searchParams.get("open") === second && document.querySelector("#preview h2"), second);
  now = await where(page);
  check(now.at === list.at + 1, `the module opened after two steps back is no step of its own: ${JSON.stringify(list)} → ${JSON.stringify(now)}`);
  // A reload keeps the entries the page wrote (they share its document): the × still goes back
  // where the list was the step before, and nothing loads the page anew.
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  await page.waitForFunction(() => document.querySelector("#preview h2"), null, { timeout: 20000 }).catch(() => problems.push("the module beside the list did not come back after the reload"));
  const reloaded = await where(page);
  await step("its × after a reload", () => page.click('#preview [data-action="close-detail"]'), () => !/[?&]open=/.test(location.search) && !document.querySelector("#preview h2"));
  now = await where(page);
  check(now.at === reloaded.at - 1 && now.length === reloaded.length, `after a reload the × did not go back where the list was: ${JSON.stringify(reloaded)} → ${JSON.stringify(now)}`);
});

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
