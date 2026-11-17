// Checks the vibrations of the app (owner, 2026-11-17: „ich möchte dass in der ganzen app vibrations
// richtig umgesetzt werden und die Benutzung unterstreichen und satisfying macht"; enhance.js
// „vibrations", folia/crates/design/src/haptic.rs), on a phone whose `navigator.vibrate` writes down
// what it is asked to play: one vocabulary, each kind its own pattern — „Merken" firm, a mark taken
// away light, „Einplanen" as done (or as a warning where the semester does not offer the module),
// ‹ › a step, „Rückgängig" a tap, a switch a tick and a segment already chosen nothing; a row swiped
// ticks as its action is armed and as it is no longer, and what it then does is felt as its switch;
// a swipe along the bottom bar ticks as it passes a third of the way; a tab held is felt; the switch
// in the menu of the profiles turns all of it off (and turning it on is felt), kept for the next
// page. On an iPhone, which has no Vibration API, a label for a native switch lies over the buttons
// that tick there (`html.tap-ticks`): a tap on it is the button's, once. A wide screen offers no
// switch.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node haptics.mjs   (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a console error or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };
const PATTERNS = { tick: "6", tap: "10", hold: "16", success: "10,60,16", warning: "16,80,10", error: "20,50,20,50,20" };
const named = Object.fromEntries(Object.entries(PATTERNS).map(([name, pattern]) => [pattern, name]));

async function open(context, path) {
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 600)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  const cdp = await context.newCDPSession(page);
  let at = 0;
  const touch = (type, points, time = Date.now() / 1000) => { at = time; return cdp.send("Input.dispatchTouchEvent", { type, touchPoints: points, timestamp: time }); };
  const swipe = async (x, y, dx, dy, { steps = 10, ms = 16 } = {}) => {
    await touch("touchStart", [{ x, y }]);
    for (let i = 1; i <= steps; i++) {
      await touch("touchMove", [{ x: x + (dx * i) / steps, y: y + (dy * i) / steps }], at + ms / 1000);
      await page.waitForTimeout(ms);
    }
    await touch("touchEnd", [], at + ms / 1000);
  };
  const hold = async (x, y, ms) => {
    await touch("touchStart", [{ x, y }]);
    await page.waitForTimeout(ms);
    await touch("touchEnd", []);
  };
  return { page, swipe, hold };
}

// A module of the catalog's list opened on a phone: its sheet, all up (a tap on its grabber), so
// that its switches are on the screen.
const ROWS = "#content > .page-stage.front .vrow .row-wrap";
const MARK = ".detail.is-module .hero .mark-switch[data-action=mark]", PLAN = '.detail.is-module .hero .plan-toggle[data-action="plan"]';
async function openModule(page, i) {
  const id = await page.evaluate(([rows, i]) => document.querySelectorAll(rows)[i].querySelector(".row").dataset.id, [ROWS, i]);
  await page.tap(`${ROWS} .row[data-id="${id}"]`);
  await page.waitForFunction((sel) => document.querySelector(sel)?.checkVisibility(), PLAN, { timeout: 10000 }).catch(() => problems.push("no module's sheet"));
  await page.waitForTimeout(800);
  const top = await page.evaluate(() => { const r = document.querySelector(".detail.is-module").getBoundingClientRect(); return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + 8) }; });
  await page.touchscreen.tap(top.x, top.y);
  await page.waitForFunction(() => document.querySelector(".detail.is-module")?.classList.contains("is-full") && document.querySelector(".detail.is-module").getAnimations().every((a) => a.playState !== "running"), null, { timeout: 5000 }).catch(() => problems.push("the module's sheet never came all up"));
  await page.waitForTimeout(200);
  return id;
}

// What was played since the last look, by name.
const felt = async (page, wait = 400) => {
  await page.waitForTimeout(wait);
  return page.evaluate(() => window.__felt.splice(0)).then((all) => all.map((pattern) => named[pattern] ?? pattern));
};
const same = (got, want) => JSON.stringify(got) === JSON.stringify(want);

// ---- an Android phone: `navigator.vibrate` writes down what it plays
const android = await browser.newContext(phone);
await android.addInitScript(() => {
  window.__felt = [];
  Object.defineProperty(Navigator.prototype, "vibrate", { configurable: true, value(pattern) { window.__felt.push([].concat(pattern).join(",")); return true; } });
  // „Einplanen" plans at once: the question for a study is put aside (plannote.mjs checks it).
  sessionStorage.setItem("betula.plan.asked", "1");
});
{
  const { page, swipe, hold } = await open(android, "/catalog");
  await page.waitForSelector(`${ROWS}.swipes`, { timeout: 20000 }).catch(() => problems.push("the catalog's rows cannot be swiped"));
  const can = await page.evaluate(() => ({ haptics: document.documentElement.classList.contains("haptics"), ticks: document.documentElement.classList.contains("tap-ticks"), engine: typeof window.betulaHaptic }));
  check(can.haptics && !can.ticks && can.engine === "function", `android: the device is not one that vibrates (${JSON.stringify(can)})`);
  await felt(page, 0);

  // A row swiped to the left past its point: a tick as it is armed, and „Merken" firmly.
  const box = await page.evaluate((rows) => { const r = document.querySelectorAll(rows)[1].getBoundingClientRect(); return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2) }; }, ROWS);
  await swipe(box.x + 80, box.y, -200, 0, { steps: 12 });
  let got = await felt(page, 700);
  check(same(got, ["tick", "tap"]), `swipe left: felt ${JSON.stringify(got)}, not a tick and a tap`);
  const marked = await page.evaluate((rows) => document.querySelectorAll(rows)[1].querySelector(".mark-toggle")?.getAttribute("aria-pressed"), ROWS);
  check(marked === "true", `swipe left: the module is not marked (${marked})`);
  // Past the point and back before letting go: armed, no longer, nothing done.
  const cdpSwipe = async () => {
    const cdp = await android.newCDPSession(page);
    const t0 = Date.now() / 1000;
    const send = (type, points, dt) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: points, timestamp: t0 + dt });
    await send("touchStart", [{ x: box.x + 80, y: box.y }], 0);
    let dt = 0;
    for (const dx of [-20, -60, -120, -200, -200, -120, -60, -20, -10, -10]) { dt += 0.03; await send("touchMove", [{ x: box.x + 80 + dx, y: box.y }], dt); await page.waitForTimeout(30); }
    await page.waitForTimeout(150);
    await send("touchEnd", [], dt + 0.2);
  };
  await cdpSwipe();
  got = await felt(page, 700);
  check(same(got, ["tick", "tick"]), `swipe there and back: felt ${JSON.stringify(got)}, not two ticks`);

  // Along the bottom bar: a tick as the next tab is a third of the way there.
  const bar = await page.evaluate(() => { const r = document.querySelector(".bottomnav").getBoundingClientRect(); return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2) }; });
  await swipe(bar.x, bar.y, -140, 0, { steps: 14 });
  got = await felt(page, 900);
  check(got.length >= 1 && got[0] === "tick" && got.every((kind) => kind === "tick"), `the bar swiped: felt ${JSON.stringify(got)}, not a tick`);
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
  await felt(page, 300);

  // A tab held: its menu, felt.
  const tab = await page.evaluate(() => { const r = document.querySelector('.bottomnav > .nav[data-hold]')?.getBoundingClientRect(); return r && { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2) }; });
  if (tab) {
    await hold(tab.x, tab.y, 650);
    got = await felt(page, 300);
    check(got[0] === "hold", `a tab held: felt ${JSON.stringify(got)}, not a hold`);
    await page.keyboard.press("Escape");
    await page.mouse.click(5, 300).catch(() => {});
    await felt(page, 300);
  } else problems.push("no tab of the bar holds anything");

  // A module's switches: „Merken" firm, taken away light, „Einplanen" done or a warning, ‹ › a
  // step, „Rückgängig" a tap.
  await openModule(page, 3);
  got = await felt(page, 300);
  check(got.every((kind) => kind === "tick"), `a module opened: felt ${JSON.stringify(got)}`);
  await page.tap(MARK);
  got = await felt(page);
  check(same(got, ["tap"]), `„Merken“: felt ${JSON.stringify(got)}, not a tap`);
  await page.tap(MARK);
  got = await felt(page);
  check(same(got, ["tick"]), `a mark taken away: felt ${JSON.stringify(got)}, not a tick`);
  await page.tap(PLAN);
  got = await felt(page);
  check(got.length === 1 && (got[0] === "success" || got[0] === "warning"), `„Einplanen“: felt ${JSON.stringify(got)}, not done`);
  await page.waitForSelector(".plan-toast-stepper >> visible=true", { timeout: 3000 }).catch(() => problems.push("„Einplanen“: no note"));
  const step = await page.evaluate(() => { const r = document.querySelector(".plan-toast-stepper").getBoundingClientRect(); return { x: Math.round(r.right - 12), y: Math.round(r.top + r.height / 2) }; });
  await page.touchscreen.tap(step.x, step.y);
  got = await felt(page);
  check(got.length === 1 && (got[0] === "tick" || got[0] === "warning"), `› in the note: felt ${JSON.stringify(got)}, not a step`);
  await page.tap(".plan-toast-undo");
  got = await felt(page);
  check(same(got, ["tap"]), `„Rückgängig“: felt ${JSON.stringify(got)}, not one tap`);

  // The menu of the profiles: a segment already chosen is nothing, another a tick; the switch for
  // vibrations turns them off, and on again with what it feels like.
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
  await page.tap(".topbar .top-profile").catch(() => problems.push("the phone's profile box cannot be tapped"));
  await page.waitForSelector(".profile-menu button.pm-haptics >> visible=true", { timeout: 3000 }).catch(() => problems.push("the menu of the profiles has no switch for vibrations"));
  await felt(page, 200);
  const checkedTheme = await page.evaluate(() => document.querySelector('.profile-menu [data-action="theme-set"][aria-checked="true"]')?.dataset.themeValue);
  await page.tap(`.profile-menu [data-action="theme-set"][data-theme-value="${checkedTheme}"]`);
  got = await felt(page);
  check(same(got, []), `the theme already chosen: felt ${JSON.stringify(got)}`);
  const other = checkedTheme === "dark" ? "light" : "dark";
  await page.tap(`.profile-menu [data-action="theme-set"][data-theme-value="${other}"]`);
  got = await felt(page);
  check(same(got, ["tick"]), `another theme: felt ${JSON.stringify(got)}, not a tick`);
  await page.tap(`.profile-menu [data-action="theme-set"][data-theme-value="${checkedTheme}"]`);
  await felt(page, 200);
  await page.tap(".profile-menu button.pm-haptics");
  got = await felt(page);
  const off = await page.evaluate(() => ({ kept: localStorage.getItem("betula.haptics.v1"), checked: document.querySelector(".profile-menu button.pm-haptics").getAttribute("aria-checked") }));
  check(off.kept === "off" && off.checked === "false", `vibrations switched off: kept ${off.kept}, the switch says ${off.checked}`);
  check(got.length <= 1, `switched off: felt ${JSON.stringify(got)}`);
  await page.evaluate(() => window.betulaHaptic("success"));
  got = await felt(page, 100);
  check(same(got, []), `off, and still felt ${JSON.stringify(got)}`);
  // Kept for the next page.
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
  await page.waitForSelector(ROWS, { timeout: 20000 }).catch(() => {});
  await openModule(page, 4);
  await felt(page, 0);
  await page.tap(MARK);
  got = await felt(page);
  check(same(got, []), `off on the next page: „Merken“ felt ${JSON.stringify(got)}`);
  await page.tap(MARK);
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
  await page.tap(".topbar .top-profile").catch(() => {});
  await page.waitForSelector(".profile-menu button.pm-haptics >> visible=true", { timeout: 3000 }).catch(() => {});
  await page.tap(".profile-menu button.pm-haptics");
  got = await felt(page);
  const on = await page.evaluate(() => localStorage.getItem("betula.haptics.v1"));
  check(on === null && same(got, ["success"]), `switched on: kept ${on}, felt ${JSON.stringify(got)}, not a success`);
  await page.close();
}
await android.close();

// ---- an iPhone: no Vibration API, a switch that ticks when the finger toggles it
const iphone = await browser.newContext(phone);
await iphone.addInitScript(() => {
  delete Navigator.prototype.vibrate;
  Object.defineProperty(HTMLInputElement.prototype, "switch", { configurable: true, get() { return this.hasAttribute("switch"); } });
  window.__switched = 0;
  document.addEventListener("change", (e) => { if (e.target.matches?.(".tap-tick > input[switch]")) window.__switched++; }, true);
});
{
  const { page } = await open(iphone, "/catalog");
  await page.waitForSelector(ROWS, { timeout: 20000 }).catch(() => {});
  const can = await page.evaluate(() => ({ haptics: document.documentElement.classList.contains("haptics"), ticks: document.documentElement.classList.contains("tap-ticks") }));
  check(can.haptics && can.ticks, `iphone: no switch ticks (${JSON.stringify(can)})`);
  await openModule(page, 2);
  // What a finger at the middle of „Merken" lands on: the label for the switch, as large as it.
  const under = await page.evaluate((sel) => {
    const button = document.querySelector(sel), r = button.getBoundingClientRect(), label = button.querySelector(".tap-tick");
    const hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
    const l = label?.getBoundingClientRect();
    return { hit: hit?.className ?? hit?.tagName, covers: Boolean(l) && Math.abs(l.width - r.width) < 1 && Math.abs(l.height - r.height) < 1, place: getComputedStyle(button).position };
  }, MARK);
  check(under.hit === "tap-tick" && under.covers, `iphone: a tap on „Merken“ does not land on its switch's label (${JSON.stringify(under)})`);
  await page.tap(MARK);
  await page.waitForTimeout(400);
  let state = await page.evaluate((sel) => ({ pressed: document.querySelector(sel).getAttribute("aria-pressed"), switched: window.__switched, path: location.pathname }), MARK);
  check(state.pressed === "true" && state.switched === 1, `iphone: a tap on „Merken“ did not mark once and toggle its switch (${JSON.stringify(state)})`);
  await page.tap(MARK);
  await page.waitForTimeout(400);
  state = await page.evaluate((sel) => ({ pressed: document.querySelector(sel).getAttribute("aria-pressed"), switched: window.__switched }), MARK);
  check(state.pressed === "false" && state.switched === 2, `iphone: a second tap did not take the mark away once (${JSON.stringify(state)})`);
  // „Einplanen" the same: once.
  await page.tap(PLAN);
  await page.waitForSelector(".plan-toast-stepper, dialog.plan-setup[open]", { timeout: 3000 }).catch(() => problems.push("iphone: „Einplanen“ did nothing"));
  await page.close();
}
await iphone.close();

// ---- a wide screen: nothing to vibrate, no switch
const wide = await browser.newContext({ viewport: { width: 1400, height: 900 } });
{
  const page = await wide.newPage();
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
  const can = await page.evaluate(() => ({ haptics: document.documentElement.classList.contains("haptics"), ticks: document.documentElement.classList.contains("tap-ticks") }));
  check(!can.haptics && !can.ticks, `wide: offers vibrations (${JSON.stringify(can)})`);
  await page.close();
}
await wide.close();

await browser.close();
if (problems.length) {
  console.error(`haptics: ${problems.length} problem(s)\n- ` + problems.join("\n- "));
  process.exit(1);
}
console.log("haptics: ok");
