// Checks the Stundenplan on a phone (owner, 2026-09-27; app/src/pages/studyplan/week.rs
// `WeekCarousel`): „Woche" is a grid there too, as wide as the window; where the plan has Termine of
// A or B weeks only, a carousel of „A-Woche", „B-Woche" and „A/B" with its tabs under it (none in
// the head), which a finger swipes and carries while it moves, the page scrolling under any other
// move; each slot says where it is held and opens its module, and closed again the week is where
// it was; the list of the week's days is closed until its line opens it, counts the week shown and
// follows it; „Kalender" stands under the Termine in every view and not in the sheet „Anpassen",
// while a wide screen keeps it in the sidebar and the switch of the weeks in the head. A plan
// without A or B weeks has the grid alone.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node studyplan-phone.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Works on any snapshot: it plans modules of the snapshot's current semester that meet in A
// weeks, in B weeks and every week, found in the snapshot itself (`/api/db`, read with the app's
// own sql.js). Fails on a console error or a step that does not show up.
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const problems = [];
const notes = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const PLAN = "betula.studyplan.v1";

// ---- what to plan: from the snapshot the server serves
const initSqlJs = createRequire(import.meta.url)("../app/assets/sql-wasm.js");
const SQL = await initSqlJs({ locateFile: (file) => fileURLToPath(new URL(`../app/assets/${file}`, import.meta.url)) });
const db = new SQL.Database(new Uint8Array(await (await fetch(base + "/api/db")).arrayBuffer()));
const rows = (sql, params = []) => {
  const statement = db.prepare(sql);
  statement.bind(params);
  const out = [];
  while (statement.step()) out.push(statement.get());
  statement.free();
  return out;
};
const semester = rows("SELECT value FROM v_meta WHERE key = 'current_semester'")[0]?.[0];
// Modules whose every Termin of the semester lies on a weekday at a fixed time, in one rhythm.
const held = (rhythm, count) => rows(
  `SELECT module_id FROM v_module_schedule WHERE semester_key = ? GROUP BY module_id
   HAVING min(rhythm) = ? AND max(rhythm) = ? AND min(weekday) >= 1 AND max(weekday) <= 5 AND count(start_time) = count(*)
   ORDER BY module_id LIMIT ?`,
  [semester, rhythm, rhythm, count],
).map(([id]) => id);
const [inA, inB, weekly] = [held("week_a", 2), held("week_b", 1), held("weekly", 2)];
const planOf = (ids) => ids.map((id) => `m\t${semester}\t${id}\t1780000000\t`).join("\n") + "\n";

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };
const open = async (options, path, plan) => {
  const context = await browser.newContext(options);
  await context.addInitScript(([key, text]) => { if (localStorage.getItem(key) === null) localStorage.setItem(key, text); }, [PLAN, plan]);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  return { page, context };
};
const toCarousel = (page) => page.evaluate(() => { const c = document.querySelector(".sp-carousel"); scrollTo(0, c.getBoundingClientRect().top + scrollY - 100); });
// The page as a finger meets it: the week shown, the tabs, the line of the list and the list.
const state = (page) => page.evaluate(() => ({
  tab: document.querySelector(".sp-weektabs [aria-checked=true]")?.textContent ?? null,
  shown: document.querySelector(".sp-slide.is-current")?.getAttribute("aria-label") ?? null,
  path: location.pathname + location.search,
  scroll: Math.round(scrollY),
  counted: Number(/\d+/.exec(document.querySelector(".sp-days-toggle .num")?.textContent ?? "")?.[0] ?? -1),
  open: document.querySelector(".sp-days-toggle")?.getAttribute("aria-expanded") ?? null,
  listed: document.querySelectorAll(".sp-daylist .sp-dayrow").length,
}));

if (!semester || inA.length === 0 || inB.length === 0 || weekly.length === 0) {
  notes.push(`skipped the carousel: the snapshot's semester ${semester} has no modules held in A weeks (${inA}), B weeks (${inB}) and every week (${weekly}) alone`);
} else {
  const { page, context } = await open(phone, "/studyplan", planOf([...weekly, ...inA, ...inB]));
  const cdp = await context.newCDPSession(page);
  const touch = (type, x, y) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x, y }] });
  // A finger: down, `steps` moves to (x1, y1), `during` while it still touches, up.
  const swipe = async (x0, y0, x1, y1, during) => {
    await touch("touchStart", x0, y0);
    for (let i = 1; i <= 10; i++) {
      await touch("touchMove", x0 + ((x1 - x0) * i) / 10, y0 + ((y1 - y0) * i) / 10);
      await page.waitForTimeout(16);
    }
    if (during) await during();
    await touch("touchEnd");
    await page.waitForTimeout(700);
  };
  await page.waitForSelector(".sp-carousel .sp-slide.is-current .slot", { timeout: 8000 }).catch(() => problems.push("phone: the week shows no slot"));
  await toCarousel(page);
  await page.waitForTimeout(300);

  // ---- the week: three of them, A/B shown, as wide as the window
  const first = await page.evaluate(() => ({
    weeks: [...document.querySelectorAll(".sp-slide")].map((slide) => [slide.getAttribute("aria-label"), slide.inert]),
    tabs: [...document.querySelectorAll(".sp-weektabs button")].map((b) => b.textContent),
    head: Boolean(document.querySelector(".sp-head .sp-weeks")),
    band: (() => { const r = document.querySelector(".sp-slide.is-current .week").getBoundingClientRect(); return [Math.round(r.left), Math.round(r.right), innerWidth]; })(),
  }));
  check(JSON.stringify(first.weeks) === JSON.stringify([["A-Woche", true], ["B-Woche", true], ["A/B", false]]), `phone: not three weeks with A/B shown, the others out of reach: ${JSON.stringify(first.weeks)}`);
  check(JSON.stringify(first.tabs) === JSON.stringify(["A-Woche", "B-Woche", "A/B"]) && !first.head, `phone: the tabs under the week are not the switch of the weeks, or the head has one too: ${JSON.stringify(first)}`);
  check(first.band[0] === 0 && first.band[1] === first.band[2], `phone: the week does not take the window's whole width: ${JSON.stringify(first.band)}`);
  // Each slot tall enough for it says where it is held.
  const rooms = await page.evaluate(() => [...document.querySelectorAll(".sp-slide.is-current .slot")].filter((slot) => slot.querySelector("small.p") && slot.getBoundingClientRect().height > 44).map((slot) => getComputedStyle(slot.querySelector("small.p")).display));
  check(rooms.length > 0 && rooms.every((display) => display !== "none"), `phone: a slot hides where it is held: ${JSON.stringify(rooms)}`);

  // ---- the list of days: closed, its line counts the week shown
  let s = await state(page);
  const all = s.counted;
  check(s.tab === "A/B" && s.open === "false" && s.listed === 0 && all > 0, `phone: the list of days is not closed under a counting line: ${JSON.stringify(s)}`);

  // ---- a swipe to the right: the week before, carried by the finger, opening nothing
  const box = await page.locator(".sp-carousel").boundingBox();
  const y = box.y + box.height * 0.85;
  await swipe(80, y, 240, y + 6, async () => {
    const carried = await page.evaluate(() => ({ dragging: document.querySelector(".sp-carousel").classList.contains("dragging"), drag: parseFloat(getComputedStyle(document.querySelector(".sp-slide.is-current")).getPropertyValue("--drag")) }));
    check(carried.dragging && carried.drag > 100, `phone: the finger does not carry the weeks: ${JSON.stringify(carried)}`);
  });
  s = await state(page);
  check(s.tab === "B-Woche" && s.shown === "B-Woche" && s.path === "/studyplan", `phone: a swipe to the right does not bring the B-Woche alone: ${JSON.stringify(s)}`);
  check(s.counted > 0 && s.counted < all, `phone: the line does not count the B-Woche (${s.counted} of ${all})`);
  await swipe(80, y, 240, y);
  await swipe(80, y, 240, y);
  s = await state(page);
  check(s.tab === "A-Woche", `phone: two more swipes do not end at the A-Woche: ${JSON.stringify(s)}`);
  await swipe(200, y, 170, y);
  s = await state(page);
  check(s.tab === "A-Woche", `phone: a short move changes the week: ${JSON.stringify(s)}`);
  // Up and down the page scrolls.
  const before = s.scroll;
  await swipe(200, y, 210, y - 200);
  s = await state(page);
  check(s.tab === "A-Woche" && s.scroll > before + 50, `phone: a finger moving up does not scroll the page, or changes the week (${before} → ${JSON.stringify(s)})`);
  await toCarousel(page);

  // ---- the tabs, and the list following the week
  await page.tap(".sp-weektabs button:nth-of-type(3)");
  await page.waitForTimeout(600);
  await page.tap(".sp-days-toggle");
  await page.waitForTimeout(300);
  s = await state(page);
  check(s.tab === "A/B" && s.open === "true" && s.listed === all, `phone: the tab A/B and the line do not show the list of all ${all} Termine: ${JSON.stringify(s)}`);
  await page.tap(".sp-weektabs button:nth-of-type(1)");
  await page.waitForTimeout(600);
  s = await state(page);
  check(s.tab === "A-Woche" && s.listed === s.counted && s.counted < all, `phone: the open list does not follow the A-Woche: ${JSON.stringify(s)}`);
  await page.tap(".sp-weektabs button:nth-of-type(3)");
  await page.waitForTimeout(600);
  await page.tap(".sp-days-toggle");
  await page.waitForTimeout(300);
  s = await state(page);
  check(s.open === "false" && s.listed === 0, `phone: the line does not close the list: ${JSON.stringify(s)}`);

  // ---- a slot opens its module; closed again, the week is in view as it was
  await toCarousel(page);
  await page.waitForTimeout(300);
  const slot = page.locator(".sp-slide.is-current a.slot").first();
  const href = await slot.getAttribute("href");
  await slot.tap();
  await page.waitForFunction(() => location.search.includes("open="), null, { timeout: 5000 }).catch(() => {});
  s = await state(page);
  check(s.path === href, `phone: a tap on a slot does not open its module: ${s.path} instead of ${href}`);
  await page.goBack();
  await page.waitForFunction(() => !location.search.includes("open="), null, { timeout: 5000 }).catch(() => {});
  await page.waitForTimeout(600);
  const back = await page.evaluate(() => { const r = document.querySelector(".sp-carousel").getBoundingClientRect(); return { top: Math.round(r.top), height: innerHeight }; });
  s = await state(page);
  check(s.tab === "A/B" && back.top >= 0 && back.top < back.height / 2, `phone: back from the module the week is not in view as it was: ${JSON.stringify({ s, back })}`);

  // ---- „Kalender": under the Termine in every view, not in the sheet
  for (const view of ["week", "dates", "exams"]) {
    if (view !== "week") {
      await page.goto(base + `/studyplan?view=${view}`, { waitUntil: "domcontentloaded" });
      await page.waitForFunction(() => window.__betulaApp === true && document.querySelector(".sp-view"), null, { timeout: 30000 }).catch(() => {});
      await page.waitForTimeout(500);
    }
    const where = await page.evaluate(() => ({ export: Boolean(document.querySelector(".sp-view + .sp-export #sp-abo")), sheet: Boolean(document.querySelector(".sidebar #sp-abo")) }));
    check(where.export && !where.sheet, `phone (${view}): „Kalender" is not under the Termine alone: ${JSON.stringify(where)}`);
  }
  await context.close();

  // ---- a wide screen keeps „Kalender" in the sidebar and the switch of the weeks in the head
  const wide = await open({ viewport: { width: 1440, height: 900 } }, "/studyplan", planOf([...weekly, ...inA, ...inB]));
  await wide.page.waitForSelector(".sp-week .slot", { timeout: 8000 }).catch(() => problems.push("wide: the week shows no slot"));
  const seen = await wide.page.evaluate(() => ({ sidebar: Boolean(document.querySelector(".sidebar #sp-abo")), export: Boolean(document.querySelector(".sp-export")), head: Boolean(document.querySelector(".sp-head .sp-weeks")), carousel: Boolean(document.querySelector(".sp-carousel")) }));
  check(seen.sidebar && !seen.export && seen.head && !seen.carousel, `wide: not the sidebar's „Kalender" and the head's switch: ${JSON.stringify(seen)}`);
  await wide.context.close();
}

// ---- a plan without A or B weeks: the grid alone
if (semester && weekly.length > 0) {
  const { page, context } = await open(phone, "/studyplan", planOf(weekly));
  await page.waitForSelector(".sp-carousel .slot", { timeout: 8000 }).catch(() => problems.push("phone, every week: the week shows no slot"));
  const alone = await page.evaluate(() => ({ slides: document.querySelectorAll(".sp-slide").length, tabs: Boolean(document.querySelector(".sp-weektabs")), head: Boolean(document.querySelector(".sp-head .sp-weeks")) }));
  check(alone.slides === 1 && !alone.tabs && !alone.head, `phone, every week: not the grid alone: ${JSON.stringify(alone)}`);
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ semester, planned: { inA, inB, weekly }, problems, notes }, null, 2));
process.exit(problems.length ? 1 : 0);
