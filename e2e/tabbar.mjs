// Checks the swipe along the phone's bottom bar (enhance.js; owner, 2026-09-30: „wenn man nach
// links swiped soll ein tab nach links gehen und beim rechts swipe eine tab nach rechts"), with
// real touches (`Input.dispatchTouchEvent`, as phone.mjs): a swipe to the right goes one tab to the
// right, one to the left one tab to the left, from wherever on the bar it starts, each one step of
// the history and no page load; the mark of the current tab follows the finger on the way (the
// bar's own, over the tab's, in the same colour) while the page stands still, and the tab's own
// mark takes over once the tab is current; a long pull goes one tab and no further, a short slow
// one glides back, a short flick goes on; at either end nothing lies further and the tab stays;
// two swipes in a row go two tabs, a tap right after a quick swipe is a tap; up or down, the bar
// scrolls the page. Then the site before the app takes over: a swipe loads the tab's page, as a
// tap would.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node tabbar.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a console error or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };

// A page on a phone, with a finger (Playwright itself only taps).
async function open(context, path) {
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 600)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  const cdp = await context.newCDPSession(page);
  // Each touch carries its time (s since the epoch): the page measures a flick by the events' own
  // times, which the round trips of the protocol would otherwise stretch.
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

// The bar as it stands: the current tab, where the tabs' marks are, the bar's own mark.
const barState = () => {
  const bar = document.querySelector(".bottomnav");
  const tabs = [...bar.querySelectorAll(":scope > .nav")];
  const current = tabs.find((tab) => tab.getAttribute("aria-current") === "page");
  const own = getComputedStyle(bar, "::before");
  const ind = current && getComputedStyle(current.querySelector(".ind"));
  const box = bar.getBoundingClientRect();
  return {
    current: current?.dataset.area ?? null,
    swipe: bar.dataset.swipe ?? null,
    styled: [bar, ...tabs].some((el) => el.getAttribute("style")),
    mids: tabs.map((tab) => { const r = tab.querySelector(".ind").getBoundingClientRect(); return Math.round(r.left + r.width / 2); }),
    bar: { top: Math.round(box.top), bottom: Math.round(box.bottom), mid: Math.round(box.top + box.height / 2) },
    // The bar's mark shows while it moves (where it stands in the window); the tab's own the rest
    // of the time.
    mark: own.content === "none" ? null : { dx: new DOMMatrix(own.transform).m41, left: box.left + parseFloat(own.left), width: parseFloat(own.width), color: own.backgroundColor },
    own: ind ? ind.backgroundColor : null,
    on: tabs.map((tab) => Number(tab.style.getPropertyValue("--on") || 0)),
    near: tabs.map((tab) => Number(tab.style.getPropertyValue("--near") || 0)),
    left: [bar, ...tabs].filter((el) => el.getAttribute("style")).map((el) => `${el.dataset.area || "bar"}: "${el.getAttribute("style")}"`),
    path: location.pathname,
    history: history.length,
    content: Math.round(document.getElementById("content").getBoundingClientRect().left),
    loaded: window.__marker === 1,
  };
};
const AREAS = ["home", "catalog", "programs", "bookmarks", "studyplan"];

const context = await browser.newContext(phone);
const { page, swipe, lift } = await open(context, "/");
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
await page.evaluate(() => { window.__marker = 1; });
const state = () => page.evaluate(barState);
// Once the mark has glided and the tab's own has taken over: the current tab then.
const settled = async (what, area) => {
  await page.waitForFunction((area) => {
    const bar = document.querySelector(".bottomnav");
    return !bar.dataset.swipe && bar.querySelector(':scope > .nav[aria-current="page"]')?.dataset.area === area && !document.querySelector("#content[aria-busy]");
  }, area, { timeout: 8000 }).catch(() => {});
  const now = await state();
  check(now.current === area, `${what}: the current tab is ${now.current}, not ${area}`);
  check(!now.swipe && !now.mark, `${what}: the bar's mark is still there (${now.swipe})`);
  check(!now.styled, `${what}: the bar or a tab kept a style of the swipe (${now.left.join(", ")})`);
  check(now.loaded, `${what}: the page was loaded again`);
  return now;
};

let now = await settled("start", "home");
const [first, , middle] = now.mids;
const y = now.bar.mid;
const step = now.mids[1] - now.mids[0];
check(step > 50, `the tabs stand ${step} px apart`);

// ---- the mark follows the finger, and only the mark moves: the page stands still
const colour = now.own;
await swipe(middle, y, 18, 3, { steps: 3, hold: true });
let held = await state();
check(held.swipe === "drag", `hold: the bar is not taken by the finger (${held.swipe})`);
check(held.mark && Math.abs(held.mark.dx - 10) <= 1, `hold: the mark is ${held.mark?.dx} px along, not 10 (18 less the slop)`);
check(held.mark && Math.abs(held.mark.left + held.mark.width / 2 - first) <= 1, `hold: the mark does not start over the current tab (${held.mark?.left} + ${held.mark?.width / 2}, the tab at ${first})`);
check(held.mark?.color === colour, `hold: the moving mark is ${held.mark?.color}, the tab's own ${colour}`);
check(held.own === "rgba(0, 0, 0, 0)", `hold: the current tab shows its own mark as well (${held.own})`);
check(held.on[0] === 1 && held.on[1] === 0 && held.near[0] > held.near[1], `hold: the icons and names turn at the wrong time (${held.on} / ${held.near})`);
check(held.content === now.content && held.path === "/", `hold: the page moved (${now.content} → ${held.content}, ${held.path})`);
// One tab along: the mark over the next tab, its icon light and its name dark.
await swipe(0, 0, step - 10, 0, { steps: 6, hold: true, from: true });
held = await state();
check(held.mark && Math.abs(held.mark.dx - step) <= 1, `hold: a tab along, the mark is ${held.mark?.dx} px along, not ${step}`);
check(held.on[0] === 0 && held.on[1] === 1 && held.near[1] === 1, `hold: a tab along, the icons and names are ${held.on} / ${held.near}`);
// Far past the next tab: held back, never two tabs.
await swipe(0, 0, 150, 0, { steps: 8, hold: true, from: true });
held = await state();
check(held.mark && held.mark.dx > step && held.mark.dx < step + 15, `pull: the mark went ${held.mark?.dx} px, past the next tab by more than its room`);
check(held.content === now.content && held.path === "/", `pull: the page moved (${now.content} → ${held.content}, ${held.path})`);
await lift();
now = await settled("swipe right", "catalog");
check(now.path.startsWith("/catalog"), `swipe right: the page is ${now.path}`);
check(now.own === colour, `swipe right: the tab's own mark is ${now.own}, not ${colour}`);

// ---- one tab to the left, from anywhere on the bar; a step of the history, as a tap is
const length = now.history;
await swipe(now.mids[4], y, -110, 2);
now = await settled("swipe left", "home");
check(now.path === "/", `swipe left: the page is ${now.path}`);
check(now.history === length + 1, `swipe left: the history grew by ${now.history - length}`);

// ---- at the left end nothing lies further: held back, and the tab stays
await swipe(middle, y, -120, 0, { hold: true });
held = await state();
check(held.mark && held.mark.dx < 0 && held.mark.dx > -15, `end: the mark went ${held.mark?.dx} px where no tab is`);
check(held.mark && held.mark.left + held.mark.dx >= 0, "end: the mark left the bar");
await lift();
now = await settled("end left", "home");

// ---- a short slow pull glides back, a short flick goes on
await swipe(middle, y, 26, 0, { steps: 10, ms: 50 });
now = await settled("slow pull", "home");
await swipe(middle, y, 30, 0, { steps: 3, ms: 8 });
now = await settled("flick", "catalog");

// ---- along the whole bar, and at the right end no further
for (const area of AREAS.slice(2)) {
  await swipe(middle, y, 110, 0);
  now = await settled(`to ${area}`, area);
  check(now.path.startsWith("/" + area), `to ${area}: the page is ${now.path}`);
}
await swipe(middle, y, 120, 0);
now = await settled("end right", "studyplan");
for (const area of AREAS.slice(0, 4).reverse()) {
  await swipe(middle, y, -110, 0);
  now = await settled(`back to ${area}`, area);
}

// ---- two swipes in a row, the second while the mark still glides, go two tabs on; a tap right
// after a swipe is a tap
await swipe(middle, y, 110, 0, { steps: 4 });
await swipe(middle, y, 110, 0, { steps: 4 });
now = await settled("two swipes", "programs");
await swipe(middle, y, -110, 0, { steps: 4 });
await page.touchscreen.tap(first, y);
now = await settled("a tap after a swipe", "home");

// ---- up the bar scrolls the page, the tab stays
await swipe(middle, y, 110, 0);
now = await settled("to the catalog", "catalog");
const top = await page.evaluate(() => scrollY);
await swipe(middle, y + 20, 4, -300, { steps: 12 });
await page.waitForTimeout(600);
now = await settled("scroll", "catalog");
check((await page.evaluate(() => scrollY)) > top + 100, `scroll: a finger up the bar did not scroll the page (${top} → ${await page.evaluate(() => scrollY)})`);
await context.close();

// ---- before the app takes over (its bundle never comes): the swipe loads the tab's page
const classic = await browser.newContext({ ...phone, serviceWorkers: "block" });
await classic.route("**/pkg/folia_client.js*", (route) => route.fulfill({ contentType: "text/javascript", body: "export default () => new Promise(() => {});" }));
{
  const { page, swipe } = await open(classic, "/programs");
  await page.waitForLoadState("load");
  const before = await page.evaluate(barState);
  check(before.current === "programs", `classic: the current tab is ${before.current}`);
  await swipe(before.mids[2], before.bar.mid, -110, 0);
  await page.waitForURL((url) => url.pathname.startsWith("/catalog"), { timeout: 8000 }).catch(() => problems.push(`classic: the swipe did not load the catalog (${page.url()})`));
  await page.waitForLoadState("load");
  const after = await page.evaluate(barState);
  check(after.current === "catalog" && !after.swipe, `classic: the catalog's page has ${after.current} current (${after.swipe})`);
  check(await page.evaluate(() => window.__betulaApp !== true), "classic: the app took over after all");
}
await browser.close();

if (problems.length) {
  console.error(problems.join("\n"));
  console.error(`tabbar FAILED: ${problems.length} problem(s)`);
  process.exit(1);
}
console.log("tabbar ok");
