// Checks the swipe on a row of the catalog on a phone (app/src/swipe.rs; owner, 2026-09-30: „Nach
// links wischen merken nach rechts wischen planen. Mach das so, dass dann darunter freigelegt wird
// was die Aktion macht (also Icon und Text)"), with real touches (`Input.dispatchTouchEvent`, as
// tabbar.mjs): the card follows the finger inside its own place (the page never grows sideways)
// and uncovers at the side it leaves what the swipe does — „Merken" to the left, „Einplanen" with
// its semester to the right, „Entfernen" where the module is marked or planned already — quiet
// until the action is armed, then in the side's colour; let go armed, the ground says what was done
// while the card holds and glides back, and the module is marked (its bookmark, the Merkliste's
// count) or planned (the Stundenplan's count); a short slow pull glides back and does nothing, a
// short flick does it; neither a swipe nor its end is a tap or a step of the history, and a tap
// right after a swipe is a tap; up or down, a row scrolls the page. A mouse in a narrow window drags
// the card the same way; the layout of a wide screen has no swipe.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node swipe.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a console error or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };

// The catalog in the browser app, and a finger (Playwright itself only taps).
async function open(context, path) {
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 600)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await page.evaluate(() => { window.__marker = 1; });
  const cdp = await context.newCDPSession(page);
  // Each touch carries its time: the row measures a flick by the events' own times.
  let last = { x: 0, y: 0 }, at = 0;
  const touch = (type, points, time = Date.now() / 1000) => { at = time; return cdp.send("Input.dispatchTouchEvent", { type, touchPoints: points, timestamp: time }); };
  // Down at (x, y) (or on from where a finger is held), in `steps` moves `ms` apart by (dx, dy);
  // lifted `ms` after the last move unless `hold`.
  const swipe = async (x, y, dx, dy, { steps = 10, ms = 16, hold = false, from = false } = {}) => {
    if (from) ({ x, y } = last);
    else await touch("touchStart", [{ x, y }]);
    for (let i = 1; i <= steps; i++) {
      last = { x: x + (dx * i) / steps, y: y + (dy * i) / steps };
      await touch("touchMove", [last], at + ms / 1000);
      await page.waitForTimeout(ms);
    }
    if (!hold) await touch("touchEnd", [], at + ms / 1000);
  };
  // The finger held still is lifted: when it is, not as a flick.
  return { page, swipe, lift: () => touch("touchEnd", []) };
}

// The row at `i` of the list as it stands: the swipe's state, where the card is against its place,
// what the ground says and in which colour, the module's bookmark; and the page around it.
const rowState = (i) => {
  const wrap = document.querySelectorAll(".vrow .row-wrap")[i];
  const place = wrap.getBoundingClientRect(), card = wrap.querySelector(".row").getBoundingClientRect();
  const ground = wrap.querySelector(".swipe-ground");
  const shown = ground && [...ground.querySelectorAll(".swipe-act")].find((act) => getComputedStyle(act).visibility === "visible");
  const count = (area) => document.querySelector(`.bottomnav .nav[data-area="${area}"] .nav-count`)?.textContent ?? "";
  return {
    id: wrap.querySelector(".row").dataset.id,
    swipe: wrap.dataset.swipe ?? null,
    side: wrap.dataset.side ?? null,
    armed: "armed" in wrap.dataset,
    done: "done" in wrap.dataset,
    styled: wrap.hasAttribute("style"),
    dx: Math.round(card.left - place.left),
    said: shown ? shown.innerText.replace(/\s*\n\s*/g, " | ").trim() : null,
    ground: ground ? getComputedStyle(ground).backgroundColor : null,
    marked: wrap.querySelector(".mark-toggle")?.getAttribute("aria-pressed") === "true",
    marks: count("bookmarks"),
    planned: count("studyplan"),
    wide: document.scrollingElement.scrollWidth > innerWidth,
    scrollY: Math.round(scrollY),
    path: location.pathname + location.search,
    history: history.length,
    loaded: window.__marker === 1,
  };
};
// The colours the ground takes when armed: the inverted look of a marked module, the accent.
const colours = () => {
  const probe = document.createElement("i");
  document.body.append(probe);
  const of = (value) => { probe.style.color = value; return getComputedStyle(probe).color; };
  const out = { invert: of("var(--invert)"), accent: of("var(--accent)"), quiet: of("var(--panel-3)") };
  probe.remove();
  return out;
};

const context = await browser.newContext(phone);
const { page, swipe, lift } = await open(context, "/catalog");
await page.waitForSelector(".vrow .row-wrap.swipes", { timeout: 20000 }).catch(() => problems.push("the catalog's rows cannot be swiped"));
const state = (i = 1) => page.evaluate(rowState, i);
const colour = await page.evaluate(colours);
// Once the card has glided back: the row at rest, nothing of the swipe left.
const rested = async (what, i = 1) => {
  await page.waitForFunction((i) => !document.querySelectorAll(".vrow .row-wrap")[i]?.dataset.swipe, i, { timeout: 5000 }).catch(() => {});
  await page.waitForTimeout(80);
  const now = await state(i);
  check(!now.swipe && !now.styled && now.said === null && now.dx === 0, `${what}: the row is not at rest (${now.swipe}, style ${now.styled}, ground ${now.said}, card ${now.dx} px)`);
  check(now.path === "/catalog" && now.loaded, `${what}: the page changed (${now.path}, loaded again: ${!now.loaded})`);
  return now;
};
const box = await page.evaluate(() => { const r = document.querySelectorAll(".vrow .row-wrap")[1].getBoundingClientRect(); return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2), width: r.width }; });
let now = await state();
const start = now;
check(!now.marked && now.marks === "" && now.planned === "", `start: something is marked or planned already (${now.marked}, ${now.marks}, ${now.planned})`);

// ---- to the left: the card follows the finger, the ground says „Merken", quiet until armed
await swipe(box.x + 60, box.y, -70, 2, { steps: 6, hold: true });
now = await state();
check(now.swipe === "drag" && now.side === "mark", `left: the row is not taken by the finger (${now.swipe}, ${now.side})`);
check(Math.abs(now.dx + 60) <= 1, `left: the card went ${now.dx} px, not -60 (70 less the slop)`);
check(now.said === "Merken", `left: the ground says ${JSON.stringify(now.said)}, not „Merken"`);
check(!now.armed && now.ground === colour.quiet, `left: armed before its time (${now.armed}, ${now.ground})`);
check(!now.wide && now.scrollY === start.scrollY, `left: the page moved (${now.wide ? "wider than the window" : ""} ${start.scrollY} → ${now.scrollY})`);
await swipe(0, 0, -90, 0, { steps: 6, hold: true, from: true });
await page.waitForTimeout(250); // the ground's colour comes in .16 s
now = await state();
check(now.armed && now.ground === colour.invert, `left: not armed at ${now.dx} px (${now.armed}, ${now.ground}, not ${colour.invert})`);
check(!now.wide, "left: the card pushed the page wider than the window");
await lift();
await page.waitForTimeout(120);
now = await state();
check(now.done && now.said === "Gemerkt", `left: the ground does not say it was done (${now.done}, ${JSON.stringify(now.said)})`);
check(now.marked && now.marks === "1", `left: the module is not marked (${now.marked}, count ${now.marks})`);
now = await rested("left");
check(now.marked && now.history === start.history, `left: the mark went, or the history grew (${now.marked}, ${start.history} → ${now.history})`);
check(await page.evaluate((id) => (localStorage.getItem("betula.bookmarks.v1") || "").startsWith(id + "\t"), now.id), "left: the mark is not stored");

// ---- to the right: „Einplanen" with the semester it plans into
await swipe(box.x - 60, box.y, 80, -2, { steps: 6, hold: true });
now = await state();
check(now.side === "plan" && /^Einplanen \| WiSe \d{4}\/\d{2}$/.test(now.said ?? ""), `right: the ground says ${JSON.stringify(now.said)}, not „Einplanen | WiSe …" (${now.side})`);
await swipe(0, 0, 80, 0, { steps: 5, hold: true, from: true });
await page.waitForTimeout(250);
now = await state();
check(now.armed && now.ground === colour.accent, `right: not armed in the accent at ${now.dx} px (${now.ground})`);
const semester = (now.said ?? "").split(" | ")[1];
await lift();
await page.waitForTimeout(120);
now = await state();
check(now.said === `Eingeplant | ${semester}`, `right: the ground says ${JSON.stringify(now.said)} once done`);
now = await rested("right");
check(now.planned === "1" && now.marked, `right: the Stundenplan counts ${JSON.stringify(now.planned)}, the mark ${now.marked}`);
check(await page.evaluate((id) => (localStorage.getItem("betula.studyplan.v1") || "").includes(`\t${id}\t`), now.id), "right: the plan does not hold the module");

// ---- again, each way: now the swipe takes away, and says from where
await swipe(box.x + 60, box.y, -170, 0, { hold: true });
now = await state();
check(now.said === "Entfernen | von der Merkliste" && now.armed, `left again: the ground says ${JSON.stringify(now.said)} (${now.armed})`);
await lift();
now = await rested("left again");
check(!now.marked && now.marks === "", `left again: the module is still marked (${now.marked}, ${now.marks})`);
await swipe(box.x - 60, box.y, 170, 0, { hold: true });
now = await state();
check(now.said === `Entfernen | aus ${semester}` && now.armed, `right again: the ground says ${JSON.stringify(now.said)} (${now.armed})`);
await lift();
now = await rested("right again");
check(now.planned === "", `right again: the Stundenplan still counts ${now.planned}`);

// ---- a short slow pull glides back and does nothing; a short flick does it, and the row's
// bookmark tapped right after it (the card still aside) is a tap: it takes the mark away again
await swipe(box.x + 60, box.y, -50, 0, { steps: 10, ms: 50 });
now = await rested("slow pull");
check(!now.marked, "slow pull: the module was marked");
await swipe(box.x + 60, box.y, -80, 0, { steps: 3, ms: 8 });
await page.waitForTimeout(60);
now = await state();
check(now.marked && now.done, `flick: the module was not marked (${now.marked}, ${now.done})`);
const mark = await page.evaluate(() => { const r = document.querySelectorAll(".vrow .row-wrap")[1].querySelector(".mark-toggle").getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
await page.touchscreen.tap(mark.x, mark.y);
now = await rested("a tap after a flick");
check(!now.marked && now.marks === "", `a tap after a flick: the bookmark did not take the mark away (${now.marked}, ${now.marks})`);

// ---- up the row scrolls the page, and nothing of a swipe starts
const top = now.scrollY;
await swipe(box.x, box.y + 20, 6, -300, { steps: 12 });
await page.waitForTimeout(600);
const scrolled = await page.evaluate(() => [...document.querySelectorAll(".vrow .row-wrap")].filter((wrap) => wrap.dataset.swipe).length);
check((await page.evaluate(() => scrollY)) > top + 100, `scroll: a finger up a row did not scroll the page (${top} → ${await page.evaluate(() => scrollY)})`);
check(scrolled === 0, `scroll: ${scrolled} row(s) took the finger`);
await page.evaluate(() => window.scrollTo(0, 0));
await page.waitForTimeout(300);

// ---- a tap on another row right after a swipe opens that module, as a tap does (R21, no page load)
await swipe(box.x + 60, box.y, -160, 0, { steps: 4 });
const other = await page.evaluate(() => { const row = document.querySelectorAll(".vrow .row-wrap")[3].querySelector(".row"); const r = row.getBoundingClientRect(); return { id: row.dataset.id, x: r.left + 60, y: r.top + r.height / 2 }; });
await page.touchscreen.tap(other.x, other.y);
await page.waitForURL((url) => url.pathname === `/catalog/module/${other.id}`, { timeout: 8000 }).catch(() => problems.push(`tap after a swipe: the module ${other.id} did not open (${page.url()})`));
check(await page.evaluate(() => window.__marker === 1), "tap after a swipe: the page was loaded again");
await context.close();

// ---- a mouse in a narrow window drags the card the same way; the drag is no click
const narrow = await browser.newContext({ viewport: { width: 700, height: 900 } });
{
  const page = await narrow.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("narrow console: " + m.text().slice(0, 600)); });
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("narrow: the browser app never took over"));
  const r = await page.evaluate(() => { const r = document.querySelectorAll(".vrow .row-wrap")[1].getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; });
  await page.mouse.move(r.x, r.y);
  await page.mouse.down();
  for (let i = 1; i <= 10; i++) { await page.mouse.move(r.x - i * 20, r.y); await page.waitForTimeout(16); }
  const held = await page.evaluate(rowState, 1);
  check(held.swipe === "drag" && held.said === "Merken" && held.armed, `narrow: the mouse does not drag the card (${held.swipe}, ${JSON.stringify(held.said)}, ${held.armed})`);
  await page.mouse.up();
  await page.waitForTimeout(900);
  const after = await page.evaluate(rowState, 1);
  check(after.marked && after.path === "/catalog" && !after.swipe, `narrow: the drag did not mark the module, or it was a click (${after.marked}, ${after.path}, ${after.swipe})`);
}
await narrow.close();

// ---- the layout of a wide screen has no swipe: a drag is none
const wide = await browser.newContext({ viewport: { width: 1440, height: 900 } });
{
  const page = await wide.newPage();
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("wide: the browser app never took over"));
  const r = await page.evaluate(() => { const r = document.querySelectorAll(".vrow .row-wrap")[1].getBoundingClientRect(); return { x: r.left + 200, y: r.top + r.height / 2 }; });
  await page.mouse.move(r.x, r.y);
  await page.mouse.down();
  for (let i = 1; i <= 8; i++) { await page.mouse.move(r.x - i * 20, r.y); await page.waitForTimeout(16); }
  const held = await page.evaluate(() => document.querySelectorAll(".vrow .row-wrap")[1].dataset.swipe ?? null);
  await page.mouse.up();
  check(held === null, `wide: the row took the mouse (${held})`);
}
await wide.close();
await browser.close();

if (problems.length) {
  console.error(problems.join("\n"));
  console.error(`swipe FAILED: ${problems.length} problem(s)`);
  process.exit(1);
}
console.log("swipe ok");
