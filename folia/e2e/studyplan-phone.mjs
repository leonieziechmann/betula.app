// Checks the Stundenplan on a phone (owner, 2026-09-27; folia/crates/planner/src/studyplan/week.rs
// `WeekCarousel`): „Woche" is a grid there too, as wide as the window; where the plan has Termine of
// A or B weeks only, a carousel of „A-Woche", „B-Woche" and „A/B", switched in the head's toolbar
// („A · B · A/B", owner 2026-10-26 and 2026-11-18), which a finger swipes and carries while it
// moves, the page scrolling under any other move; each slot says where it is held and opens its
// card, and closed again the week is where it was; the list of the week's days is closed until
// its line opens it, counts the week shown and follows it; „Bearbeiten" brings the dock in the tab
// bar's place and the buttons of the day list's rows, Back takes both away; the head is two rows of
// one grid, as tall in both states; „Export" and „Teilen …" are a sheet the head's „Export" opens in
// every view, while a wide screen keeps them in the sidebar and the switch of the weeks and
// „Bearbeiten" in the head. A plan without A or B weeks has the grid alone,
// and no switch of the weeks.
// History: 2026-11-17, the edit mode. The sheet „Anpassen" and „Plan · Alle Termine" in it went (the
// mode shows what is hidden); a slot opens its card (`termin=`), not its module (`open=`).
// 2026-11-18: the head tidied into two rows (owner: „der header ist ja eine absolute Katastrophe"),
// „Export" a sheet of the head (owner: „Export sollte am handy wahrscheinlich auch ein modal werden").
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
const initSqlJs = createRequire(import.meta.url)("../assets/sql-wasm.js");
const SQL = await initSqlJs({ locateFile: (file) => fileURLToPath(new URL(`../assets/${file}`, import.meta.url)) });
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
  tab: document.querySelector(".sp-head .sp-weekpick [aria-checked=true]")?.textContent ?? null,
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
    tabs: [...document.querySelectorAll(".sp-head .sp-weekpick button")].map((b) => [b.textContent, b.getAttribute("aria-label")]),
    // Two rows of one grid: the title over the switch (their left ends one line), „Bearbeiten" over
    // „Kalender · Export" (their right ends one line), the switch and the tools in one row.
    beside: (() => {
      const r = (sel) => document.querySelector(sel)?.getBoundingClientRect();
      const [title, weeks, edit, tools] = [r(".sp-head h1"), r(".sp-head .sp-weekpick"), r(".sp-head .sp-edit-toggle"), r(".sp-head .sp-tools")];
      return Boolean(title && weeks && edit && tools) && Math.abs(title.left - weeks.left) < 1 && Math.abs(edit.right - tools.right) < 1 && Math.abs(weeks.top + weeks.height / 2 - (tools.top + tools.height / 2)) < 2 && weeks.top >= title.bottom && tools.top >= edit.bottom - 1;
    })(),
    word: [...document.querySelectorAll(".sp-head .sp-weekpick *")].some((e) => e.children.length === 0 && /Woche/.test(e.textContent)),
    named: document.querySelector(".sp-head .sp-weekpick [role=radiogroup]")?.getAttribute("aria-label") ?? null,
    tools: [...document.querySelectorAll(".sp-head .sp-tools > button")].map((b) => [b.dataset.action, b.textContent.trim(), Math.round(b.getBoundingClientRect().height)]),
    sum: Boolean(document.querySelector(".sp-sum")),
    head: Math.round(document.querySelector(".sp-head").getBoundingClientRect().height),
    all: Boolean(document.querySelector(".sp-all, .sheet-toggle")),
    band: (() => { const r = document.querySelector(".sp-slide.is-current .week").getBoundingClientRect(); return [Math.round(r.left), Math.round(r.right), innerWidth]; })(),
  }));
  check(JSON.stringify(first.weeks) === JSON.stringify([["A-Woche", true], ["B-Woche", true], ["A/B", false]]), `phone: not three weeks with A/B shown, the others out of reach: ${JSON.stringify(first.weeks)}`);
  check(JSON.stringify(first.tabs) === JSON.stringify([["A", "A-Woche"], ["B", "B-Woche"], ["A/B", "A- und B-Woche"]]) && first.beside && !first.all, `phone: the head is not two rows of one grid, „A · B · A/B" under the title and „Kalender · Export" under „Bearbeiten", or „Anpassen" or „Plan · Alle Termine" still: ${JSON.stringify(first)}`);
  check(!first.word && first.named === "Woche", `phone: the switch of the weeks has the word „Woche" or no name: ${JSON.stringify({ word: first.word, named: first.named })}`);
  check(JSON.stringify(first.tools) === JSON.stringify([["calendars", "Kalender", 44], ["export", "Export", 44]]) && !first.sum, `phone: the toolbar's „Kalender" and „Export" (44 px), and no line of numbers under the head: ${JSON.stringify({ tools: first.tools, sum: first.sum })}`);
  check(first.band[0] === 0 && first.band[1] === first.band[2], `phone: the week does not take the window's whole width: ${JSON.stringify(first.band)}`);
  // Each slot tall enough for it says where it is held.
  const rooms = await page.evaluate(() => [...document.querySelectorAll(".sp-slide.is-current .slot")].filter((slot) => slot.querySelector("small.p") && slot.getBoundingClientRect().height > 44).map((slot) => getComputedStyle(slot.querySelector("small.p")).display));
  check(rooms.length > 0 && rooms.every((display) => display !== "none"), `phone: a slot hides where it is held: ${JSON.stringify(rooms)}`);

  // ---- the list of days: closed, its line counts the week shown
  let s = await state(page);
  const all = s.counted;
  check(s.tab === "A/B" && s.open === "false" && s.listed === 0 && all > 0, `phone: the list of days is not closed under a counting line: ${JSON.stringify(s)}`);
  // „Bearbeiten": the dock in the tab bar's place, the day list's rows with their buttons; Back leaves.
  await page.tap(".sp-head .sp-edit-toggle");
  await page.waitForFunction(() => location.search === "?edit=1" && document.querySelector(".sp-editdock") && !document.querySelector(".sp-editdock").getAnimations().length, null, { timeout: 8000 }).catch(() => problems.push("phone: „Bearbeiten“ brings no dock"));
  const editingHead = await page.evaluate(() => Math.round(document.querySelector(".sp-head").getBoundingClientRect().height));
  check(editingHead === first.head, `phone: the head is not as tall while editing: ${first.head} → ${editingHead}`);
  const dock = await page.evaluate(() => { const [a, b] = [document.querySelector(".sp-editdock")?.getBoundingClientRect(), document.querySelector(".bottomnav")?.getBoundingClientRect()]; return Boolean(a && b) && ["x", "y", "width", "height"].every((k) => Math.abs(a[k] - b[k]) < 0.5); });
  check(dock, "phone: the dock does not stand in the tab bar's place");
  await page.tap(".sp-days-toggle");
  await page.waitForTimeout(300);
  const acts = await page.evaluate(() => document.querySelectorAll(".sp-daylist .sp-dayrow .slot-act").length);
  check(acts > 0, "phone: the day list's rows have no buttons while editing");
  await page.tap(".sp-days-toggle");
  await page.waitForTimeout(300);
  await page.goBack();
  await page.waitForFunction(() => location.search === "" && !document.querySelector(".sp-editdock"), null, { timeout: 8000 }).catch(() => problems.push("phone: Back does not leave the mode"));
  await page.waitForTimeout(400);
  s = await state(page);
  check(s.open === "false" && s.listed === 0 && (await page.evaluate(() => document.querySelectorAll(".sp-dayrow .slot-act").length)) === 0, `phone: back from the mode, the list of days is not closed: ${JSON.stringify(s)}`);
  await toCarousel(page);
  await page.waitForTimeout(300);

  // ---- a swipe to the right: the week before, carried by the finger, opening nothing
  const box = await page.locator(".sp-carousel").boundingBox();
  const y = box.y + box.height * 0.85;
  await swipe(80, y, 240, y + 6, async () => {
    const carried = await page.evaluate(() => ({ dragging: document.querySelector(".sp-carousel").classList.contains("dragging"), drag: parseFloat(getComputedStyle(document.querySelector(".sp-slide.is-current")).getPropertyValue("--drag")) }));
    check(carried.dragging && carried.drag > 100, `phone: the finger does not carry the weeks: ${JSON.stringify(carried)}`);
  });
  s = await state(page);
  check(s.tab === "B" && s.shown === "B-Woche" && s.path === "/studyplan", `phone: a swipe to the right does not bring the B-Woche alone: ${JSON.stringify(s)}`);
  check(s.counted > 0 && s.counted < all, `phone: the line does not count the B-Woche (${s.counted} of ${all})`);
  await swipe(80, y, 240, y);
  await swipe(80, y, 240, y);
  s = await state(page);
  check(s.tab === "A", `phone: two more swipes do not end at the A-Woche: ${JSON.stringify(s)}`);
  await swipe(200, y, 170, y);
  s = await state(page);
  check(s.tab === "A", `phone: a short move changes the week: ${JSON.stringify(s)}`);
  // Up and down the page scrolls.
  const before = s.scroll;
  await swipe(200, y, 210, y - 200);
  s = await state(page);
  check(s.tab === "A" && s.scroll > before + 50, `phone: a finger moving up does not scroll the page, or changes the week (${before} → ${JSON.stringify(s)})`);
  await toCarousel(page);

  // ---- the head's switch, and the list following the week
  await page.tap(".sp-head .sp-weekpick button:nth-of-type(3)");
  await page.waitForTimeout(600);
  await page.tap(".sp-days-toggle");
  await page.waitForTimeout(300);
  s = await state(page);
  check(s.tab === "A/B" && s.open === "true" && s.listed === all, `phone: the switch's A/B and the line do not show the list of all ${all} Termine: ${JSON.stringify(s)}`);
  check((await page.evaluate(() => document.querySelectorAll(".sp-daylist .sp-dayrow .slot-act").length)) === 0, "phone: the day list's rows have buttons while looking");
  await page.tap(".sp-head .sp-weekpick button:nth-of-type(1)");
  await page.waitForTimeout(600);
  s = await state(page);
  check(s.tab === "A" && s.shown === "A-Woche" && s.listed === s.counted && s.counted < all, `phone: the week and the open list do not follow the A-Woche: ${JSON.stringify(s)}`);
  await page.tap(".sp-head .sp-weekpick button:nth-of-type(3)");
  await page.waitForTimeout(600);
  await page.tap(".sp-days-toggle");
  await page.waitForTimeout(300);
  s = await state(page);
  check(s.open === "false" && s.listed === 0, `phone: the line does not close the list: ${JSON.stringify(s)}`);

  // ---- a slot opens its card; closed again, the week is in view as it was
  await toCarousel(page);
  await page.waitForTimeout(300);
  const slot = page.locator(".sp-slide.is-current a.slot").first();
  const href = await slot.getAttribute("href");
  await slot.tap();
  await page.waitForFunction(() => location.search.includes("termin=") && document.querySelector("dialog[open][data-sheet] .tm-card"), null, { timeout: 5000 }).catch(() => {});
  s = await state(page);
  check(s.path === href && href.startsWith("/studyplan?termin="), `phone: a tap on a slot does not open its card: ${s.path} instead of ${href}`);
  await page.goBack();
  await page.waitForFunction(() => !location.search.includes("termin=") && !document.querySelector("dialog[open]"), null, { timeout: 5000 }).catch(() => {});
  await page.waitForTimeout(600);
  const back = await page.evaluate(() => { const r = document.querySelector(".sp-carousel").getBoundingClientRect(); return { top: Math.round(r.top), height: innerHeight }; });
  s = await state(page);
  check(s.tab === "A/B" && back.top >= 0 && back.top < back.height / 2, `phone: back from the card the week is not in view as it was: ${JSON.stringify({ s, back })}`);

  // ---- „Export" and „Teilen …": a sheet the head's „Export" opens, in every view, no sidebar
  for (const view of ["week", "dates", "exams"]) {
    if (view !== "week") {
      await page.goto(base + `/studyplan?view=${view}`, { waitUntil: "domcontentloaded" });
      await page.waitForFunction(() => window.__betulaApp === true && document.querySelector(".sp-view"), null, { timeout: 30000 }).catch(() => {});
      await page.waitForTimeout(500);
    }
    const before = await page.evaluate(() => ({ page: Boolean(document.querySelector("#sp-abo, #sp-share")), sheet: Boolean(document.querySelector(".sidebar #sp-abo, .sidebar #sp-share")) }));
    await page.tap(".sp-head .sp-tool[data-action='export']");
    await page.waitForFunction(() => document.querySelector("dialog.sp-export-sheet[open] #sp-abo"), null, { timeout: 5000 }).catch(() => {});
    const where = await page.evaluate(() => {
      const sheet = document.querySelector("dialog.sp-export-sheet[open]");
      return { abo: Boolean(sheet?.querySelector("#sp-abo")), share: Boolean(sheet?.querySelector("#sp-share")), file: Boolean(sheet?.querySelector("a.action[download]")), picture: [...(sheet?.querySelectorAll("button.action") ?? [])].some((b) => /Als Bild speichern/.test(b.textContent)), expanded: document.querySelector(".sp-head .sp-tool[data-action='export']")?.getAttribute("aria-expanded") };
    });
    check(!before.page && !before.sheet && where.abo && where.share && where.file && where.picture && where.expanded === "true", `phone (${view}): „Export" is not a sheet of the head with the file, the picture, „Abonnieren" and „Teilen …": ${JSON.stringify({ before, where })}`);
    if (view === "week") {
      // „Abonnieren" opens „Teilen", which takes the sheet's place.
      await page.tap("dialog.sp-export-sheet[open] #sp-abo");
      await page.waitForFunction(() => document.querySelector("dialog.sh-dialog[open]") && !document.querySelector("dialog.sp-export-sheet[open]"), null, { timeout: 5000 }).catch(() => problems.push("phone: „Abonnieren“ in the sheet does not open „Teilen“ in its place"));
    }
    await page.keyboard.press("Escape");
    await page.waitForFunction(() => !document.querySelector("dialog[open]"), null, { timeout: 5000 }).catch(() => problems.push(`phone (${view}): Esc leaves a dialog open`));
  }
  await context.close();

  // ---- a wide screen keeps „Export" in the sidebar, the switch of the weeks and „Bearbeiten" in the head
  const wide = await open({ viewport: { width: 1440, height: 900 } }, "/studyplan", planOf([...weekly, ...inA, ...inB]));
  await wide.page.waitForSelector(".sp-week .slot", { timeout: 8000 }).catch(() => problems.push("wide: the week shows no slot"));
  const seen = await wide.page.evaluate(() => ({ sidebar: Boolean(document.querySelector(".sidebar #sp-abo")), export: Boolean(document.querySelector(".sp-export, .sp-tools")), head: Boolean(document.querySelector(".sp-head .sp-weeks")), all: Boolean(document.querySelector(".sp-head .sp-edit-toggle")) && !document.querySelector(".sp-all"), carousel: Boolean(document.querySelector(".sp-carousel")) }));
  check(seen.sidebar && !seen.export && seen.head && seen.all && !seen.carousel, `wide: not the sidebar's „Export", the head's switch and „Bearbeiten": ${JSON.stringify(seen)}`);
  await wide.context.close();
}

// ---- a plan without A or B weeks: the grid alone
if (semester && weekly.length > 0) {
  const { page, context } = await open(phone, "/studyplan", planOf(weekly));
  await page.waitForSelector(".sp-carousel .slot", { timeout: 8000 }).catch(() => problems.push("phone, every week: the week shows no slot"));
  const alone = await page.evaluate(() => ({ slides: document.querySelectorAll(".sp-slide").length, head: Boolean(document.querySelector(".sp-head .sp-weeks")) }));
  check(alone.slides === 1 && !alone.head, `phone, every week: not the grid alone: ${JSON.stringify(alone)}`);
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ semester, planned: { inA, inB, weekly }, problems, notes }, null, 2));
process.exit(problems.length ? 1 : 0);
