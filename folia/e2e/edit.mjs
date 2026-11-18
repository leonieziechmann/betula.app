// The Stundenplan looked at and edited (owner, 2026-11-17: „Ich glaube, das was hier die richtige Lösung
// wäre ist bei dem Kalender einen edit modus hinzuzufügen, weil man dadurch unnötiges ui ausblenden kann,
// und dann im edit modus auch Termine leicht anzeigen kann, die man ausgeblendet hat"; docs/folia/
// frontend.md, „The Stundenplan looked at and edited"), on canary's data of 2026W: Informatik B.Sc.'s
// first semester planned (EvS 12104, EEG 12107, PP 12102, MIT1 11112).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node edit.mjs [--only=look,status,enter,focus,edit,side,phone,en,command]
// - look: nothing on the page changes the plan — no `.slot-acts` on a slot, also when the pointer is on
//   it or it has the keyboard, no „+ Termin", no faint slot, a click on a free stretch opens nothing and
//   changes no address; a hatched group leads to its Termin (`termin=`), no slot preselects.
// - status: the status row while looking says the plan („… · 4 Module · 1 eigener Termin"), its overlap
//   („1 Überschneidung", a button that unfolds the list of overlaps) and „3 Wahlen offen · 2 Änderungen
//   offen ›", the quiet line into the mode, whose rows then stand open and have the keyboard; 46 px in
//   both states.
// - enter: „Bearbeiten" pressed (`aria-pressed`, `.sp-edit`, the bar, `edit=1`, the live region says
//   „Stundenplan bearbeiten: 3 Wahlen offen"), the plan stored byte for byte as before; the week and
//   three named slots (VL MIT1 Mo, VL EvS Fr, VL PP Fr) measured looking, editing and with a choice in
//   focus: equal to the tenth of a pixel. A reload keeps the mode; „Fertig", Esc, Back and the pressed
//   button each leave it (the week drawn without buttons again); the page is never loaded again.
// - focus: „Wählen ›" of „Offen" takes the choice into focus inside the mode (`edit=1&wahl=…`, the
//   focus bar in the status row in the bar's place), „Weiter" replaces the entry (no step of the
//   history), the focus's „Fertig" leads back into the mode; an old link `?wahl=e148370` is the mode
//   too, its „Fertig" `/studyplan?edit=1`.
// - edit: the bar „Stundenplan bearbeiten" with its figures („2 Wahlen offen · 2 ausgeblendet"); what the
//   plan leaves out stands faint, its link the Termin's; the slots' buttons — `eye` „Ausblenden" on a
//   Termin the week shows, `eye-off` „Einblenden" on a faint one, „✓"/„+" to tick („Diese Gruppe
//   nehmen", „Nur diese Gruppe", „Auch diese Gruppe"), the bin alone on a Termin of one's own, no „×"
//   anywhere; each answers in the store (`r`, `e`, `c` lines; the bin removes the `t` line, the keyboard comes
//   to its note's „Rückgängig", the note stands through a click elsewhere, and „Rückgängig" brings the
//   line back with the keyboard on the slot put back) and no other note comes. A series with a hidden date carries the crossed
//   eye only while editing. A choice decided while editing keeps its row of „Offen" in place, pale
//   („Mi 11:30 ✓"), until the mode is left. A click never moves what was clicked from under the pointer
//   (the eye clicked twice brings the plan back). A free stretch opens the form for that day; Esc closes
//   the form, the mode stays.
// - side: the sidebar's title „Stundenplan"; „Kalender" with „Mein Studiengang" (no lock) and Lena's
//   „Voll · Spur · Aus" in both states, „+ Kalender" and the calendars' „…" only while editing; no „Plan
//   leeren"; „Export" does not move.
// - phone: no „Anpassen"; „Bearbeiten" in the head; the dock exactly over the tab bar (its box), the
//   module list directly under the week while editing (after the Termine while looking), „Nach oben"
//   never; the amber line into the mode, a choice's sheet without the dock; Back leaves step by step.
// - en: `/en/studyplan` — the slot's bin, the card's „Delete", the overview's and the module's way back
//   stay under `/en/studyplan`, never `/en/en/…`.
// - command: „Befehle" (Strg + K) finds „Stundenplan bearbeiten", Enter enters the mode; in it „Bearbeiten
//   beenden" leaves it.
// Needs the snapshot whose current semester is WiSe 2026/27. Fails on a page load after takeover, a
// console error, or a step that does not show up.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const shots = process.env.SMOKE_SHOTS;
const only = (process.argv.find((arg) => arg.startsWith("--only=")) ?? "").slice("--only=".length);
const block = (name) => !only || only.split(",").includes(name);
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
const FS1 = ["12104", "12107", "12102", "11112"].map((id) => `m\t2026W\t${id}\t1790000000\t`).join("\n") + "\n";
const INFORMATIK = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n";
const ARBEIT = "t\t1\t0\t2026W\tw\t4\t13:00-17:00\t\t\t\tArbeit\tCafé Mosaik\t\n";
// The Tutorium of MIT1 hidden, one group of the Übung of EvS ticked, the Friday lecture of EvS hidden on
// 20.11., a Termin of one's own on Thursday.
const RICH = FS1 + "e\t2026W\t150132\nc\t2026W\t148369-aaf38\nh\t2026W\t148701-b7025\t2026-11-20\n" + ARBEIT;
// A Termin of one's own of EvS on Tuesday at 09:15 (over VL EEG), a change QIS moved and one whose Termin
// is gone.
const STATUS = FS1 + "t\t1\t0\t2026W\tw\t2\t09:15-10:45\t\t\t12104\t\t\t\tlecture\n"
  + "x\t0\t2026W\t150132-00001\tall\t\t\t\tHG/2.46\t2\t15:30-17:00\tHG/2.45\t11112\n"
  + "x\t0\t2026W\t999999-00001\tall\t\t\t\tZHG/HS.A\t3\t09:15-10:45\tHG/1.01\t12104\n";
const LENA = "l\t1\t\t013-82-2018\tLena\n" + ["11109", "13977"].map((id) => `n\t1\t2026W\t${id}`).join("\n") + "\n";
// The three slots measured: VL MIT1 on Monday, VL EvS on Friday, VL PP on Friday — by their tooltips,
// which a choice in focus keeps (its slots are no links there).
const NAMED = ["Vorlesung · Mathematik IT-1 (Diskrete Mathematik) · Mo 13:45", "Vorlesung · Entwicklung von Softwaresystemen · Fr 09:15", "Vorlesung · Programmierpraktikum · Fr 11:30"];
const wide = { viewport: { width: 1440, height: 900 } };
const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };

const open = async (options, plan, path = "/studyplan") => {
  const context = await browser.newContext(options);
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  await context.addInitScript(([k1, k2, v1, v2]) => {
    if (localStorage.getItem(k1) === null) {
      localStorage.setItem(k1, v1);
      localStorage.setItem(k2, v2);
    }
  }, [PLAN, MINE, plan, INFORMATIK]);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await page.evaluate(() => { window.__marker = 1; });
  return { page, context };
};
const until = async (page, name, fn, arg) => {
  try {
    await page.waitForFunction(fn, arg, { timeout: 10000 });
    return true;
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
    return false;
  }
};
// The page at rest: no picture of the page before held (it comes a moment after the address changed,
// and a click while it is held can be lost), nothing busy, nothing settling, for 120 ms.
const rest = async (page) => {
  await page.evaluate(() => { window.__restSince = 0; }).catch(() => {});
  await page.waitForFunction(() => {
    const quiet = !document.querySelector(".pending-page.held, main.content[aria-busy]") && !document.documentElement.hasAttribute("data-settling");
    if (!quiet) { window.__restSince = 0; return false; }
    window.__restSince ||= performance.now();
    return performance.now() - window.__restSince > 120;
  }, null, { timeout: 5000 }).catch(() => {});
};
// A click that may find nothing: said, not thrown. It waits for the page at rest first.
const click = async (page, selector, name = selector) => {
  await rest(page);
  try {
    await page.click(selector, { timeout: 5000 });
    return true;
  } catch {
    problems.push(`${name}: nothing to click (${page.url()})`);
    return false;
  }
};
const tap = async (page, selector, name = selector) => {
  await rest(page);
  try {
    await page.tap(selector, { timeout: 5000 });
    return true;
  } catch {
    problems.push(`${name}: nothing to tap (${page.url()})`);
    return false;
  }
};
const stored = (page) => page.evaluate((key) => localStorage.getItem(key) ?? "", PLAN);
const lines = (text) => text.split("\n").filter(Boolean);
// What changed in the store between two texts: the lines gone and the lines added.
const diff = (before, after) => ({ gone: lines(before).filter((line) => !lines(after).includes(line)), added: lines(after).filter((line) => !lines(before).includes(line)) });
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const where = (page) => page.evaluate(() => location.pathname + location.search);
const marker = async (page, name) => check(await page.evaluate(() => window.__marker === 1), `${name}: the page loaded again`);
// The page's state of the mode: the address, `.sp-edit`, the toggle, the bar, the focus bar, the
// buttons of the week.
const mode = (page) => page.evaluate(() => ({
  path: location.pathname + location.search,
  edit: Boolean(onPage(".page-inner.sp.sp-edit")),
  pressed: onPage("#sp-edit-toggle")?.getAttribute("aria-pressed") ?? null,
  bar: Boolean(onPage(".sp-status .sp-editbar")),
  focus: Boolean(onPage(".sp-status .sp-focusbar")),
  acts: allOnPage(".sp-week .slot-acts").length,
}));
const looking = () => !onPage(".page-inner.sp.sp-edit") && !onPage(".sp-editbar") && allOnPage(".sp-week a.slot").length > 0 && allOnPage(".sp-week .slot-acts").length === 0 && !document.querySelector(".pending-page.held");
const editing = () => Boolean(onPage(".page-inner.sp.sp-edit")) && Boolean(onPage(".sp-status .sp-editbar")) && allOnPage(".sp-week .slot-acts").length > 0 && !document.querySelector(".pending-page.held");
// The week and the three named slots, to a tenth of a pixel.
const measure = (page) => page.evaluate((named) => {
  const box = (el) => { if (!el) return null; const r = el.getBoundingClientRect(); return [r.x, r.y, r.width, r.height].map((v) => Math.round(v * 10) / 10); };
  return { week: box(onPage(".sp-week")), slots: named.map((title) => box(allOnPage(".sp-week .slot").find((slot) => slot.getAttribute("title")?.startsWith(title)))) };
}, NAMED);
// The buttons beside a slot (they stand after it, not in it): icon and label.
const actsOf = (page, key) => page.evaluate((key) => {
  const slot = onPage(`.sp-week a.slot[href*="termin=${key}"]`);
  const acts = slot?.nextElementSibling?.matches(".slot-acts") ? [...slot.nextElementSibling.querySelectorAll(".slot-act")] : [];
  return acts.map((b) => `${b.dataset.icon}:${b.getAttribute("aria-label")}`);
}, key);
const act = (key, label) => `.sp-week a.slot[href*="termin=${key}"] + .slot-acts .slot-act[aria-label="${label}"]`;
// The point of a day's column at `minute` of the week grid.
const spot = (page, day, minute) => page.evaluate(([day, minute]) => {
  const week = allOnPage(".sp-week .week")[0];
  const col = week?.querySelectorAll(".col")[day - 1];
  if (!col) return null;
  const [first, last] = [Number(week.dataset.first), Number(week.dataset.last)];
  const y = () => { const rect = col.getBoundingClientRect(); return rect.top + rect.height * ((minute - first) / (last - first)) + 2; };
  if (y() < 120 || y() > innerHeight - 160) window.scrollBy(0, y() - innerHeight / 2);
  const rect = col.getBoundingClientRect();
  return { x: rect.left + rect.width * 0.5, y: y() };
}, [day, minute]);
const visible = (page, selector) => page.evaluate((selector) => allOnPage(selector).filter((el) => el.getClientRects().length && getComputedStyle(el).visibility !== "hidden").length, selector);

// ---------- looking: nothing changes the plan ----------
if (block("look")) {
  const { page, context } = await open(wide, FS1);
  await until(page, "the week while looking", looking);
  const before = await stored(page);
  // No button on a slot, also with the pointer on it and with the keyboard in it.
  const firm = ".sp-week a.slot[href*=\"termin=148303-bcd64\"]";
  const hatched = '.sp-week a.slot.alt[href*="termin=148370-eb017"]';
  await page.hover(firm);
  await page.waitForTimeout(250);
  check(await page.evaluate(() => allOnPage(".sp-week .slot-acts, .sp-week .slot-act").length) === 0, "looking: a slot under the pointer has buttons");
  await page.focus(hatched);
  await page.waitForTimeout(250);
  check(await page.evaluate(() => allOnPage(".sp-week .slot-acts, .sp-week .slot-act").length) === 0, "looking: a slot with the keyboard has buttons");
  check(await visible(page, "[data-action='own-add'], .sp-editbar, .sp-editdock, .sp-cal-add, .sp-cal-more, .sp-cat-eye, .sp-own-add") === 0, "looking: a control of the mode is shown");
  check(await page.evaluate(() => allOnPage(".sp-week .slot.faint").length) === 0, "looking: a faint slot");
  // A hatched group leads to its Termin; nothing preselects.
  const href = await page.getAttribute(hatched, "href");
  check(href === "/studyplan?termin=148370-eb017" && !(await page.evaluate(() => allOnPage(".sp-week [data-pre]").length)), `looking: the hatched group leads to ${href}`);
  // A free stretch (Monday 09:00): nothing.
  const at = await spot(page, 1, 9 * 60);
  if (at) await page.mouse.click(at.x, at.y);
  await page.waitForTimeout(500);
  check(await page.evaluate(() => !document.querySelector("dialog[open]")) && (await where(page)) === "/studyplan", `looking: a free stretch opened something (${await where(page)})`);
  check((await stored(page)) === before, "looking: the plan changed");
  await marker(page, "looking");
  await context.close();
}

// ---------- the status row: one way into the mode ----------
if (block("status")) {
  const { page, context } = await open(wide, STATUS);
  await until(page, "the status row", () => Boolean(onPage(".sp-status .sp-look .sp-open-way")) && Boolean(onPage(".sp-status .sp-clash-fig")));
  const row = await page.evaluate(() => ({
    sum: onPage(".sp-look .sp-sum")?.textContent.trim(),
    clash: onPage(".sp-look .sp-clash-fig")?.textContent.trim(),
    expanded: onPage(".sp-look .sp-clash-fig")?.getAttribute("aria-expanded"),
    way: onPage(".sp-look .sp-open-way")?.textContent.replace(/\s+/g, " ").trim(),
    href: onPage(".sp-look .sp-open-way")?.getAttribute("href"),
    name: onPage(".sp-look .sp-open-way")?.getAttribute("aria-label"),
    height: Math.round(onPage(".sp-status")?.getBoundingClientRect().height ?? 0),
    overlaps: Boolean(onPage("#sp-overlaps")),
  }));
  check(row.sum === "Informatik B.Sc. · 4 Module · 1 eigener Termin", `the sum line: ${JSON.stringify(row)}`);
  check(row.clash === "1 Überschneidung" && row.expanded === "false" && !row.overlaps, `the overlap's figure: ${JSON.stringify(row)}`);
  check(row.way === "3 Wahlen offen · 2 Änderungen offen" && row.href === "/studyplan?edit=1" && row.name === "3 Wahlen offen · 2 Änderungen offen: Stundenplan bearbeiten", `the quiet line: ${JSON.stringify(row)}`);
  check(row.height === 46, `the status row's height looking: ${row.height}`);
  // The overlap's figure unfolds what overlaps, and changes nothing else.
  await click(page, ".sp-look .sp-clash-fig", "„1 Überschneidung“");
  await until(page, "the overlaps unfolded", () => onPage(".sp-look .sp-clash-fig")?.getAttribute("aria-expanded") === "true" && /Di 09:15–10:45 · Vorlesung Elektrische/.test(onPage("#sp-overlaps")?.innerText ?? ""));
  check((await where(page)) === "/studyplan", `the overlap's figure changed the address: ${await where(page)}`);
  // The quiet line: into the mode, the rows of „Offen" open and the first of them has the keyboard.
  const measured = await measure(page);
  await click(page, ".sp-look .sp-open-way", "the quiet line");
  await until(page, "the mode by the quiet line", editing);
  await until(page, "the first row of „Offen“ has the keyboard", () => document.activeElement?.matches(".sp-open-row") && document.activeElement.textContent.includes("Übung EvS"));
  const bar = await page.evaluate(() => ({
    figs: onPage(".sp-editbar .eb-open-fig")?.textContent.trim(),
    clash: onPage(".sp-editbar .sp-clash-fig")?.textContent.trim(),
    rows: allOnPage(".sp-open-row").filter((row) => row.getClientRects().length).map((row) => row.querySelector("b")?.textContent),
    height: Math.round(onPage(".sp-status")?.getBoundingClientRect().height ?? 0),
  }));
  check(bar.figs === "3 Wahlen offen · 2 Änderungen offen" && bar.clash === "1 Überschneidung" && bar.height === 46, `the bar's figures: ${JSON.stringify(bar)}`);
  check(bar.rows.length === 5 && bar.rows[0] === "Übung EvS", `„Offen“ in the mode: ${JSON.stringify(bar.rows)}`);
  check(same(await measure(page), measured), `the quiet line moved the week: ${JSON.stringify(measured)} → ${JSON.stringify(await measure(page))}`);
  if (shots) await page.screenshot({ path: `${shots}/edit-status.png` });
  await marker(page, "the status row");
  await context.close();
}

// ---------- entering and leaving ----------
if (block("enter")) {
  const { page, context } = await open(wide, FS1);
  await until(page, "the week while looking", looking);
  const before = await stored(page);
  const looked = await measure(page);
  check(looked.week && looked.slots.every(Boolean), `the three named slots: ${JSON.stringify(looked)}`);
  check(await page.evaluate(() => onPage("#sp-edit-toggle")?.textContent.trim() === "Bearbeiten" && onPage("#sp-edit-toggle")?.getAttribute("aria-pressed") === "false"), "„Bearbeiten“ in the head, not pressed");
  // The button: pressed in the frame after the click.
  const frame = page.evaluate(() => new Promise((done) => {
    onPage("#sp-edit-toggle").addEventListener("click", () => requestAnimationFrame(() => requestAnimationFrame(() => done({ pressed: onPage("#sp-edit-toggle")?.getAttribute("aria-pressed"), edit: Boolean(onPage(".page-inner.sp.sp-edit")) }))), { once: true });
  }));
  await click(page, "#sp-edit-toggle", "„Bearbeiten“");
  const next = await frame;
  check(next.pressed === "true" && next.edit, `„Bearbeiten“ two frames after the click: ${JSON.stringify(next)}`);
  await until(page, "the mode", editing);
  let m = await mode(page);
  check(m.path === "/studyplan?edit=1" && m.pressed === "true" && m.bar && !m.focus, `the mode: ${JSON.stringify(m)}`);
  const said = await page.evaluate(() => allOnPage("p.visually-hidden[role=status]").map((p) => p.textContent.trim()).filter(Boolean));
  check(said.includes("Stundenplan bearbeiten: 3 Wahlen offen"), `the live region on entering: ${JSON.stringify(said)}`);
  check(same(await measure(page), looked), `editing moved the week: ${JSON.stringify(looked)} → ${JSON.stringify(await measure(page))}`);
  check((await stored(page)) === before, "entering the mode changed the plan");
  // A reload keeps it.
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the reload: the app never took over"));
  await until(page, "the mode after a reload", editing);
  await page.evaluate(() => { window.__marker = 1; });
  check((await where(page)) === "/studyplan?edit=1", `the reload: ${await where(page)}`);
  // „Fertig".
  await click(page, ".sp-editbar .sp-done", "„Fertig“");
  await until(page, "„Fertig“ leaves", looking);
  m = await mode(page);
  check(m.path === "/studyplan" && m.pressed === "false", `„Fertig“: ${JSON.stringify(m)}`);
  check(same(await measure(page), looked), "after „Fertig“ the week is not where it was");
  // Esc.
  await click(page, "#sp-edit-toggle", "„Bearbeiten“");
  await until(page, "the mode again", editing);
  await rest(page);
  await page.keyboard.press("Escape");
  await until(page, "Esc leaves", looking);
  check((await where(page)) === "/studyplan", `Esc: ${await where(page)}`);
  // Back.
  await click(page, "#sp-edit-toggle", "„Bearbeiten“");
  await until(page, "the mode again", editing);
  await page.goBack();
  await until(page, "Back leaves", looking);
  check((await where(page)) === "/studyplan", `Back: ${await where(page)}`);
  // The pressed button.
  await click(page, "#sp-edit-toggle", "„Bearbeiten“");
  await until(page, "the mode again", editing);
  await click(page, "#sp-edit-toggle", "the pressed „Bearbeiten“");
  await until(page, "the pressed button leaves", looking);
  m = await mode(page);
  check(m.path === "/studyplan" && m.pressed === "false", `the pressed button: ${JSON.stringify(m)}`);
  check((await stored(page)) === before, "entering and leaving changed the plan");
  await marker(page, "entering and leaving");
  await context.close();
}

// ---------- the focus inside the mode ----------
if (block("focus")) {
  const { page, context } = await open(wide, FS1, "/studyplan?edit=1");
  await until(page, "the mode", editing);
  const editingBox = await measure(page);
  await click(page, '.sp-side .sp-open-row[href$="wahl=e148369"]', "„Wählen ›“ of Übung EvS");
  await until(page, "the choice in focus", () => location.search === "?edit=1&wahl=e148369" && Boolean(onPage(".sp-status .sp-focusbar")) && !onPage(".sp-editbar") && !document.querySelector(".pending-page.held"));
  check(await page.evaluate(() => Boolean(onPage(".page-inner.sp.sp-edit")) && onPage("#sp-edit-toggle")?.getAttribute("aria-pressed") === "true"), "the focus is not the mode");
  check(same(await measure(page), editingBox), `the focus moved the week: ${JSON.stringify(editingBox)} → ${JSON.stringify(await measure(page))}`);
  const entries = await page.evaluate(() => history.length);
  await click(page, '.sp-focusbar a.btn[data-replace]:has-text("Weiter")', "„Weiter“");
  await until(page, "„Weiter“", () => location.search === "?edit=1&wahl=e148370" && /Praktikum PP/.test(onPage(".sp-focusbar")?.textContent ?? ""));
  check((await page.evaluate(() => history.length)) === entries, "„Weiter“ added a step to the history");
  await click(page, ".sp-focusbar .sp-done", "the focus's „Fertig“");
  await until(page, "back in the mode", () => location.search === "?edit=1" && Boolean(onPage(".sp-status .sp-editbar")) && !onPage(".sp-focusbar"));
  check(same(await measure(page), editingBox), "back from the focus the week moved");
  await marker(page, "the focus");
  await context.close();
  // An old link to a choice: the mode, its „Fertig" the mode without the choice.
  const old = await open(wide, FS1, "/studyplan?wahl=e148370");
  await until(old.page, "the old link's focus", () => Boolean(onPage(".sp-status .sp-focusbar")) && Boolean(onPage(".page-inner.sp.sp-edit")) && /Praktikum PP/.test(onPage(".sp-focusbar")?.textContent ?? ""));
  check((await old.page.getAttribute(".sp-focusbar .sp-done", "href")) === "/studyplan?edit=1", `the old link's „Fertig“: ${await old.page.getAttribute(".sp-focusbar .sp-done", "href")}`);
  await click(old.page, ".sp-focusbar .sp-done", "the old link's „Fertig“");
  await until(old.page, "the old link's „Fertig“ leads into the mode", () => location.search === "?edit=1" && Boolean(onPage(".sp-status .sp-editbar")));
  await old.context.close();
}

// ---------- editing: the bar, the faint slots, the buttons ----------
if (block("edit")) {
  const { page, context } = await open(wide, RICH);
  await until(page, "the week while looking", looking);
  // The series with a hidden date: its mark only while editing.
  check(await page.evaluate(() => allOnPage(".sp-week .slot-hid").filter((i) => i.getClientRects().length).length) === 0, "looking: a series is marked for its hidden date");
  await click(page, "#sp-edit-toggle", "„Bearbeiten“");
  await until(page, "the mode", editing);
  const bar = await page.evaluate(() => ({ title: onPage(".sp-editbar .eb-text > b")?.textContent, figs: onPage(".sp-editbar .figs")?.innerText.replace(/\s+/g, " ").trim(), add: Boolean(onPage(".sp-editbar [data-action='own-add']")), done: onPage(".sp-editbar .sp-done")?.innerText.replace(/\s+/g, " ").trim() }));
  check(bar.title === "Stundenplan bearbeiten" && bar.figs === "2 Wahlen offen · 2 ausgeblendet" && bar.add && bar.done === "Fertig Esc", `the bar: ${JSON.stringify(bar)}`);
  const marked = await page.evaluate(() => allOnPage(".sp-week .slot-hid").map((i) => i.closest(".slot")?.getAttribute("href")));
  check(same(marked, ["/studyplan?edit=1&termin=148701-b7025"]), `the series with a hidden date: ${JSON.stringify(marked)}`);
  // Faint: the Tutorium hidden, the groups of the Übung of EvS not ticked; each leads to its Termin.
  const faint = await page.evaluate(() => allOnPage(".sp-week a.slot.faint").map((slot) => slot.getAttribute("href")).sort());
  check(same(faint, ["/studyplan?edit=1&termin=148369-6bb8d", "/studyplan?edit=1&termin=148369-a4d12", "/studyplan?edit=1&termin=148369-f09d2", "/studyplan?edit=1&termin=150132-6bb8d"]), `the faint slots: ${JSON.stringify(faint)}`);
  // The slots' buttons.
  const seen = {};
  for (const key of ["148303-bcd64", "148008-8e4ea", "148370-eb017", "148369-aaf38", "148369-a4d12", "150132-6bb8d", "4000000001-00000"]) seen[key] = await actsOf(page, key);
  const want = {
    "148303-bcd64": ["eye:Ausblenden"],
    "148008-8e4ea": ["eye:Vorlesung ausblenden"],
    "148370-eb017": ["check:Diese Gruppe nehmen", "eye:Diese Gruppe ausblenden"],
    "148369-aaf38": ["check:Gruppe abwählen", "eye:Diese Gruppe ausblenden"],
    "148369-a4d12": ["check:Nur diese Gruppe", "plus:Auch diese Gruppe"],
    "150132-6bb8d": ["eye-off:Einblenden"],
    "4000000001-00000": ["trash-2:Löschen"],
  };
  for (const [key, acts] of Object.entries(want)) check(same(seen[key], acts), `the buttons of ${key}: ${JSON.stringify(seen[key])}, not ${JSON.stringify(acts)}`);
  check(await page.evaluate(() => document.querySelectorAll(".slot-act[data-icon='x']").length) === 0, "a slot has the „×“");
  if (shots) await page.screenshot({ path: `${shots}/edit-buttons.png` });
  // Each action answers in the store, and no note comes but the bin's.
  const does = async (key, label, want, name) => {
    const before = await stored(page);
    const selector = act(key, label);
    await page.focus(selector).catch(() => {});
    if (!(await click(page, selector, name))) return;
    await until(page, `${name}: the store`, ([key, before, want]) => {
      const now = (localStorage.getItem(key) ?? "").split("\n").filter(Boolean);
      const was = before.split("\n").filter(Boolean);
      const gone = was.filter((line) => !now.includes(line)), added = now.filter((line) => !was.includes(line));
      return JSON.stringify(gone.sort()) === JSON.stringify([...want.gone].sort()) && JSON.stringify(added.sort()) === JSON.stringify([...want.added].sort());
    }, [PLAN, before, want]);
    const d = diff(before, await stored(page));
    check(same(d.gone.sort(), [...want.gone].sort()) && same(d.added.sort(), [...want.added].sort()), `${name}: ${JSON.stringify(d)}`);
  };
  const noNote = async (name) => check(await page.evaluate(() => !allOnPage(".plan-toast-said").some((n) => n.getClientRects().length)), `${name}: a note while editing`);
  // The eye on a Termin the week shows: in the same place its crossed eye, a second click brings it back.
  const eye = act("148303-bcd64", "Ausblenden");
  await page.focus(eye);
  const box = await page.locator(eye).boundingBox();
  const centre = box && { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  const plan0 = await stored(page);
  await does("148303-bcd64", "Ausblenden", { gone: [], added: ["r\t2026W\t148303-bcd64"] }, "the eye of VL MIT1 Mo");
  await noNote("the eye");
  if (centre) {
    await page.mouse.move(centre.x, centre.y);
    await page.waitForTimeout(250);
    const under = await page.evaluate(([x, y]) => { const el = document.elementFromPoint(x, y)?.closest(".slot-act"); const slot = el?.parentElement?.previousElementSibling; return el ? `${el.dataset.icon}:${el.getAttribute("aria-label")}@${slot?.getAttribute("href")}` : null; }, [centre.x, centre.y]);
    check(under === "eye-off:Einblenden@/studyplan?edit=1&termin=148303-bcd64", `under the pointer after the eye: ${under}`);
    await page.mouse.click(centre.x, centre.y);
    await until(page, "the second click in the same place brings it back", ([key, plan]) => localStorage.getItem(key) === plan, [PLAN, plan0]);
  }
  await does("150132-6bb8d", "Einblenden", { gone: ["e\t2026W\t150132"], added: [] }, "the crossed eye of the Tutorium");
  await does("148369-a4d12", "Auch diese Gruppe", { gone: [], added: ["c\t2026W\t148369-a4d12"] }, "„Auch diese Gruppe“");
  await does("148369-6bb8d", "Nur diese Gruppe", { gone: ["c\t2026W\t148369-aaf38", "c\t2026W\t148369-a4d12"], added: ["c\t2026W\t148369-6bb8d"] }, "„Nur diese Gruppe“");
  await does("148370-eb017", "Diese Gruppe ausblenden", { gone: [], added: ["r\t2026W\t148370-eb017"] }, "„Diese Gruppe ausblenden“");
  await does("148008-8e4ea", "Vorlesung ausblenden", { gone: [], added: ["e\t2026W\t148008"] }, "„Vorlesung ausblenden“");
  await noNote("the slots' buttons");
  // A choice decided while editing: its row of „Offen" stays in its place, pale, until the mode is left.
  const offen = () => page.evaluate(() => allOnPage(".sp-side .sp-open-row").map((row) => ({ done: row.classList.contains("done"), text: row.innerText.replace(/\s+/g, " ").trim(), y: Math.round(row.getBoundingClientRect().top) })));
  const rows0 = await offen();
  await does("148370-ef2fb", "Diese Gruppe nehmen", { gone: [], added: ["c\t2026W\t148370-ef2fb"] }, "„Diese Gruppe nehmen“");
  await until(page, "the decided row pale", () => allOnPage(".sp-side .sp-open-row.done").length === 1);
  const rows1 = await offen();
  const pp0 = rows0.find((row) => row.text.startsWith("Praktikum PP")), pp1 = rows1.find((row) => row.text.startsWith("Praktikum PP"));
  check(pp0 && pp1 && pp1.done && pp1.text === "Praktikum PP Mi 11:30 ✓" && pp1.y === pp0.y && rows1.length === rows0.length, `the decided row of „Offen“: ${JSON.stringify({ rows0, rows1 })}`);
  // The bin: the `t` line goes, the note's „Rückgängig" brings it back.
  const bin = act("4000000001-00000", "Löschen");
  await page.focus(bin).catch(() => {});
  await click(page, bin, "the bin of „Arbeit“");
  await until(page, "the bin", ([key]) => !(localStorage.getItem(key) ?? "").includes("\tArbeit\t") && /„Arbeit“ gelöscht/.test(onPage(".own-note .plan-toast-said")?.textContent ?? ""), [PLAN]);
  // The keyboard, whose bin went with the Termin, is on „Rückgängig"; and a deletion's note stays
  // until it is put away: a click on a blank part of the sidebar leaves it standing (it took the
  // „Rückgängig" with it once, and a slow double click on a bin lost the Termin).
  await until(page, "the keyboard on „Rückgängig“ after the bin", () => document.activeElement?.id === "own-note-undo");
  const blank = await page.evaluate(() => { const at = { x: 200, y: innerHeight - 140 }; return document.elementFromPoint(at.x, at.y)?.closest("a, button, input, select, label, dialog, .wait-note") ? null : at; });
  check(Boolean(blank), "a blank part of the sidebar to click");
  if (blank) await page.mouse.click(blank.x, blank.y);
  await page.waitForTimeout(700);
  check(await page.evaluate(() => Boolean(onPage(".own-note .plan-toast-undo") ?? document.querySelector(".own-note .plan-toast-undo"))), "a deletion's note stands after a click elsewhere");
  await click(page, ".own-note .plan-toast-undo", "„Rückgängig“ of the bin");
  await until(page, "„Rückgängig“ of the bin", ([key, line]) => (localStorage.getItem(key) ?? "").includes(line) && allOnPage(".sp-week a.slot.own").length === 1, [PLAN, ARBEIT]);
  await until(page, "the keyboard on the slot put back", () => document.activeElement?.matches?.(".sp-week a.slot.own") === true);
  // A free stretch: the form of a new Termin, of that day; Esc closes the form and the mode stays. The
  // week takes no click for half a second after a slot's bin (a second click meant for the bin would
  // land on what moved into its place), so the free stretch waits that out.
  await page.waitForTimeout(600);
  const at = await spot(page, 1, 9 * 60);
  if (at) await page.mouse.click(at.x, at.y);
  await until(page, "a free stretch opens the form", () => Boolean(document.querySelector("dialog.own-form[open]")) && document.querySelector("dialog.own-form[open] #own-day")?.value === "1");
  await rest(page);
  await page.keyboard.press("Escape");
  await until(page, "Esc closes the form alone", () => !document.querySelector("dialog.own-form[open]") && location.search === "?edit=1" && Boolean(onPage(".sp-editbar")));
  // Leaving tidies „Offen": entered again, the decided row is gone.
  await click(page, ".sp-editbar .sp-done", "„Fertig“");
  await until(page, "„Fertig“", looking);
  check(await page.evaluate(() => allOnPage(".sp-week .slot-hid").filter((i) => i.getClientRects().length).length) === 0, "looking again: a series is marked for its hidden date");
  await click(page, "#sp-edit-toggle", "„Bearbeiten“");
  await until(page, "the mode again", editing);
  const rows2 = await offen();
  check(!rows2.some((row) => row.done || row.text.startsWith("Praktikum PP")), `the mode left and entered again, „Offen“: ${JSON.stringify(rows2)}`);
  await marker(page, "editing");
  await context.close();
}

// ---------- the sidebar ----------
if (block("side")) {
  const { page, context } = await open(wide, FS1 + LENA);
  await until(page, "the sidebar while looking", () => allOnPage(".sp-cals .sp-cal").length === 2 && Boolean(onPage(".sidebar #sp-abo")));
  const side = () => page.evaluate(() => {
    const shown = (selector) => allOnPage(selector).filter((el) => el.getClientRects().length).length;
    return {
      title: onPage(".sidebar .panel-head h2")?.textContent.trim(),
      group: onPage(".sp-cals-head .flabel")?.textContent.trim(),
      rows: allOnPage(".sp-cals .sp-cal").map((row) => row.innerText.replace(/\s+/g, " ").trim()),
      lock: shown(".sp-cal .lock"),
      seg: allOnPage(".sp-cal:not(.mine) .sp-cal-seg button").map((b) => `${b.getAttribute("aria-label")}${b.getAttribute("aria-checked") === "true" ? " ✓" : ""}`),
      add: shown(".sp-cals-head .sp-cal-add"),
      more: shown(".sp-cal .sp-cal-more"),
      clear: Boolean(document.querySelector("#sp-clear")) || allOnPage(".sidebar button, .sidebar a").some((el) => /Plan leeren/.test(el.textContent)),
      abo: Math.round(onPage(".sidebar #sp-abo")?.getBoundingClientRect().top ?? -1),
    };
  });
  const look = await side();
  check(look.title === "Stundenplan" && look.group === "Kalender" && look.rows[0]?.startsWith("Mein Studiengang") && look.lock === 0 && same(look.seg, ["voll in der Woche, neben deinen Terminen ✓", "als Spur am Rand jedes Tages", "nicht in der Woche"]), `the sidebar while looking: ${JSON.stringify(look)}`);
  check(look.add === 0 && look.more === 0 && !look.clear, `looking: „+ Kalender“, a menu or „Plan leeren“: ${JSON.stringify(look)}`);
  await click(page, "#sp-edit-toggle", "„Bearbeiten“");
  await until(page, "the mode", editing);
  const edit = await side();
  check(edit.title === "Stundenplan" && same(edit.rows, look.rows) && same(edit.seg, look.seg) && edit.lock === 0, `the sidebar while editing: ${JSON.stringify(edit)}`);
  check(edit.add === 1 && edit.more === 1 && !edit.clear, `editing: „+ Kalender“ and one „…“, no „Plan leeren“: ${JSON.stringify(edit)}`);
  check(edit.abo === look.abo && look.abo > 0, `„Export“ moved: ${look.abo} → ${edit.abo}`);
  await marker(page, "the sidebar");
  await context.close();
}

// ---------- a phone ----------
if (block("phone")) {
  const { page, context } = await open(phone, FS1);
  await until(page, "the phone's week", () => allOnPage(".sp-carousel a.slot").length > 0);
  const boxes = () => page.evaluate(() => {
    // In the page (what scrolls), and on the screen (what stands fixed: the dock, the tab bar).
    const box = (selector, fixed) => { const el = onPage(selector); if (!el || !el.getClientRects().length) return null; const r = el.getBoundingClientRect(); return [r.x, r.y + (fixed ? 0 : scrollY), r.width, r.height].map((v) => Math.round(v * 10) / 10); };
    return { dock: box(".sp-editdock", true), nav: box(".bottomnav", true), carousel: box(".sp-carousel"), list: box(".sp-list"), days: box(".sp-days-toggle") };
  });
  const head = await page.evaluate(() => ({ sheet: Boolean(onPage(".sp-head .sheet-toggle, .sheet-toggle")), toggle: onPage(".sp-head .sp-edit-toggle")?.textContent.trim(), line: onPage("a.sp-open-line")?.getAttribute("href") }));
  check(!head.sheet && head.toggle === "Bearbeiten" && head.line === "/studyplan?edit=1", `the phone's head while looking: ${JSON.stringify(head)}`);
  let b = await boxes();
  check(!b.dock && b.list && b.days && b.list[1] > b.days[1], `the phone while looking: the list after the Termine, no dock: ${JSON.stringify(b)}`);
  await tap(page, ".sp-head .sp-edit-toggle", "„Bearbeiten“");
  await until(page, "the phone's mode", () => Boolean(onPage(".page-inner.sp.sp-edit")) && Boolean(onPage(".sp-editdock")) && !document.querySelector(".pending-page.held"));
  // The dock slides up into the tab bar's place: once it rests (no animation, no transform), its box is the tab bar's to half a pixel.
  await until(page, "the dock at rest over the tab bar", () => {
    const [dock, nav] = [onPage(".sp-editdock")?.getBoundingClientRect(), onPage(".bottomnav")?.getBoundingClientRect()];
    return dock && nav && !onPage(".sp-editdock").getAnimations().length && getComputedStyle(onPage(".sp-editdock")).transform === "none" && ["x", "y", "width", "height"].every((k) => Math.abs(dock[k] - nav[k]) < 0.5);
  });
  b = await boxes();
  check(b.dock && b.nav && b.dock.every((v, i) => Math.abs(v - b.nav[i]) < 0.5), `the dock over the tab bar: ${JSON.stringify(b)}`);
  check(b.list && b.carousel && b.list[1] - (b.carousel[1] + b.carousel[3]) >= 0 && b.list[1] - (b.carousel[1] + b.carousel[3]) <= 40 && b.list[1] < b.days[1], `the list directly under the week while editing: ${JSON.stringify(b)}`);
  const dock = await page.evaluate(() => onPage(".sp-editdock")?.innerText.replace(/\s+/g, " ").trim());
  check(dock === "Bearbeiten 3 Wahlen offen Termin Fertig", `the dock: „${dock}"`);
  // „Nach oben" never, also far down the page.
  await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
  await page.waitForTimeout(400);
  check(await visible(page, ".to-top") === 0, "„Nach oben“ while editing on a phone");
  await page.evaluate(() => scrollTo(0, 0));
  // A choice: its sheet, no dock; Back to the mode, Back out of it.
  await tap(page, "details.sp-open-line > summary", "the amber line");
  await until(page, "the rows of „Offen“", () => allOnPage("details.sp-open-line[open] .sp-open-row").length === 3);
  await tap(page, 'details.sp-open-line .sp-open-row[href$="wahl=e148370"]', "„Praktikum PP“");
  await until(page, "the choice's sheet", () => location.search === "?edit=1&wahl=e148370" && allOnPage("section.sp-pick").some((el) => !el.closest(".pending-page")) && !allOnPage(".sp-editdock").some((el) => el.getClientRects().length));
  await page.goBack();
  await until(page, "Back: the mode", () => location.search === "?edit=1" && allOnPage(".sp-editdock").some((el) => el.getClientRects().length));
  await page.goBack();
  await until(page, "Back: looking", () => location.search === "" && !onPage(".page-inner.sp.sp-edit") && !onPage(".sp-editdock"));
  if (shots) await page.screenshot({ path: `${shots}/edit-phone.png` });
  await marker(page, "a phone");
  await context.close();
}

// ---------- English: the ways back stay under /en/ ----------
if (block("en")) {
  const { page, context } = await open(wide, FS1 + ARBEIT, "/en/studyplan?edit=1");
  await until(page, "the English mode", () => Boolean(onPage(".sp-editbar")) && allOnPage(".sp-week a.slot.own").length === 1);
  const own = '.sp-week a.slot.own + .slot-acts .slot-act[data-icon="trash-2"]';
  check((await page.getAttribute(own, "aria-label").catch(() => null)) === "Delete", "the English bin is not „Delete“");
  await page.focus(own).catch(() => {});
  await click(page, own, "the English bin");
  await until(page, "the bin in English", ([key]) => !(localStorage.getItem(key) ?? "").includes("\tArbeit\t") && Boolean(onPage(".own-note .plan-toast-undo")), [PLAN]);
  check((await where(page)) === "/en/studyplan?edit=1", `the bin led to ${await where(page)}`);
  await click(page, ".own-note .plan-toast-undo", "„Undo“");
  await until(page, "„Undo“", () => allOnPage(".sp-week a.slot.own").length === 1);
  // The card's „Delete".
  await click(page, ".sp-week a.slot.own", "the Termin of one's own");
  await until(page, "its card", () => Boolean(onPage("dialog[open] .tm-card [data-action='own-remove']")));
  await click(page, "dialog[open] .tm-card [data-action='own-remove']", "the card's „Delete“");
  await until(page, "the card's „Delete“", ([key]) => !(localStorage.getItem(key) ?? "").includes("\tArbeit\t") && !document.querySelector("dialog[open]"), [PLAN]);
  check((await where(page)) === "/en/studyplan?edit=1", `the card's „Delete“ led to ${await where(page)}`);
  // The overview's „Back", and the module's in full.
  await page.goto(base + "/en/studyplan?termin=148303-bcd64&full=1", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
  await until(page, "the English overview", () => Boolean(onPage(".is-termin [data-action='back'], .is-termin a.back, [data-action='back']")));
  await click(page, "[data-action='back'] >> visible=true", "the overview's „Back“");
  await until(page, "the overview's way back", () => location.pathname === "/en/studyplan" && !location.search.includes("full="));
  await page.goto(base + "/en/studyplan?open=11112&full=1", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
  await until(page, "the English module in full", () => Boolean(onPage(".module-page h2")) && Boolean(onPage("[data-action='back']")));
  const back = await page.getAttribute("[data-action='back'] >> visible=true", "href").catch(() => null);
  check(back?.startsWith("/en/studyplan") && !back.includes("/en/en"), `the module's way back: ${back}`);
  await click(page, "[data-action='back'] >> visible=true", "the module's „Back“");
  await until(page, "the module's way back", () => location.pathname === "/en/studyplan" && !location.search.includes("full="));
  check(!(await where(page)).includes("/en/en"), `English: ${await where(page)}`);
  await context.close();
}

// ---------- „Befehle" ----------
if (block("command")) {
  const { page, context } = await open(wide, FS1);
  await until(page, "the week while looking", looking);
  const run = async (typed, label) => {
    await page.keyboard.press("Control+k");
    if (!(await until(page, `the palette for „${label}“`, () => Boolean(document.querySelector("dialog.cmd-dialog[open] input"))))) return;
    await page.fill("dialog.cmd-dialog[open] input", typed);
    await until(page, `„${label}“ chosen`, (label) => {
      const input = document.querySelector("dialog.cmd-dialog[open] input");
      return document.getElementById(input?.getAttribute("aria-activedescendant") ?? "")?.querySelector(".cmd-label")?.textContent === label;
    }, label);
    await page.keyboard.press("Enter");
  };
  await run("bearbeiten", "Stundenplan bearbeiten");
  await until(page, "the command enters the mode", editing);
  check((await where(page)) === "/studyplan?edit=1", `the command: ${await where(page)}`);
  await run("beenden", "Bearbeiten beenden");
  await until(page, "the command leaves the mode", looking);
  await marker(page, "„Befehle“");
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(`edit: ${problems.length} problem(s)\n- ${problems.join("\n- ")}`);
  process.exit(1);
}
console.log("edit: ok");
