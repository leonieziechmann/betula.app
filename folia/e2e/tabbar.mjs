// Checks the swipe along the phone's bottom bar (enhance.js; owner, 2026-09-30, the other way round
// the next day: „die ganze Leiste zu bewegen und den selector stehen zu lassen und erst wenn man los
// lässt geht das dann wieder zur original Location zurück"), with real touches
// (`Input.dispatchTouchEvent`, as phone.mjs): a swipe to the left goes one tab to the right, one to
// the right one tab to the left, from wherever on the bar it starts, each one step of the history
// and no page load; on the way the row of tabs follows the finger inside the bar and the mark of the
// current tab stays where it is (the bar's lens, over the tab's, in the same colour, with a copy of
// the row inside that lies over the row) while the page stands still; let go, the row and the lens
// glide as Web Animations, and the tab's own mark takes over once the tab is current; a long pull
// goes one tab and no further, a short slow one glides back, a short flick goes on; at either end
// nothing lies further and the tab stays; a finger catches the glide where it is and goes on from
// there, so two swipes in a row go two tabs, and at either end, where a quick swipe lets go with
// the lens past the last tab, the row goes on from there with the finger and never against it
// (owner, 2026-10-02: „Probleme, wenn man schnell swiped über den rand hinaus"); a tap right after
// a quick swipe is a tap; the hover a finger leaves on the tab it tapped lights nothing once a swipe
// has gone on to another tab (owner: „wird das davor angeklickt noch hervorgehoben"), while a mouse
// still lights the tab it points at; up or down, the bar scrolls the page. Then the site while the
// app is starting (owner, 2026-10-02: on a phone „friert das häufig ein", a tab tapped right after
// opening loaded the next page and started it all again): a tab tapped and a swipe are current at
// once, the page stays, and the app shows the last one's page once it runs; where the app does not
// start, a swipe loads the tab's page, as a tap would.
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
  // Safari leaves the hover of a tap on what was tapped until the next tap elsewhere, where
  // Chromium moves it with the finger: held on the element of `selector` as Safari holds it.
  const keepHover = async (selector) => {
    await cdp.send("DOM.enable");
    await cdp.send("CSS.enable");
    const { root } = await cdp.send("DOM.getDocument");
    const { nodeId } = await cdp.send("DOM.querySelector", { nodeId: root.nodeId, selector });
    await cdp.send("CSS.forcePseudoState", { nodeId, forcedPseudoClasses: ["hover"] });
  };
  // The finger held still is lifted: when it is, not as a flick.
  return { page, swipe, lift: () => touch("touchEnd", []), keepHover };
}

// The bar as it stands: the current tab, where the tabs' marks are and how far the row has moved,
// the bar's lens and the copy of the row in it.
const barState = () => {
  const bar = document.querySelector(".bottomnav");
  const tabs = [...bar.querySelectorAll(":scope > .nav")];
  const current = tabs.find((tab) => tab.getAttribute("aria-current") === "page");
  const lens = bar.querySelector(":scope > .bottomnav-lens");
  const copies = lens ? [...lens.querySelectorAll(".bottomnav-copy > .nav")] : [];
  const middle = (el) => { const r = el.getBoundingClientRect(); return r.left + r.width / 2; };
  const ind = current && getComputedStyle(current.querySelector(".ind"));
  const box = bar.getBoundingClientRect();
  return {
    current: current?.dataset.area ?? null,
    swipe: bar.dataset.swipe ?? null,
    styled: [bar, ...tabs].some((el) => el.getAttribute("style")),
    mids: tabs.map((tab) => Math.round(middle(tab.querySelector(".ind")))),
    // How far each tab stands from its place (the row moves as one).
    row: tabs.map((tab) => Math.round(new DOMMatrix(getComputedStyle(tab).transform).m41 * 10) / 10),
    bar: { top: Math.round(box.top), bottom: Math.round(box.bottom), mid: Math.round(box.top + box.height / 2) },
    // The lens shows while the bar is not at rest (where its middle stands in the window); the tab's
    // own mark the rest of the time.
    lens: lens && getComputedStyle(lens).visibility === "visible" ? { mid: middle(lens), color: getComputedStyle(lens).backgroundColor } : null,
    copies: copies.length,
    // How far a copy in the lens lies from its tab below at most (0: the copy lies over the row).
    off: copies.length === tabs.length ? Math.max(...copies.map((copy, i) => Math.abs(middle(copy.querySelector(".ind")) - middle(tabs[i].querySelector(".ind"))))) : copies.length ? Infinity : 0,
    own: ind ? ind.backgroundColor : null,
    // Each tab's own mark and the colour of its name (a tab neither current nor pointed at shows no
    // mark, and its name is light).
    marks: tabs.map((tab) => getComputedStyle(tab.querySelector(".ind")).backgroundColor),
    names: tabs.map((tab) => getComputedStyle(tab).color),
    near: tabs.map((tab) => Number(tab.style.getPropertyValue("--near") || 0)),
    anims: document.getAnimations().filter((anim) => anim.effect?.target && bar.contains(anim.effect.target)).length,
    left: [bar, ...tabs].filter((el) => el.getAttribute("style")).map((el) => `${el.dataset.area || "bar"}: "${el.getAttribute("style")}"`),
    path: location.pathname,
    history: history.length,
    content: Math.round(document.getElementById("content").getBoundingClientRect().left),
    loaded: window.__marker === 1,
  };
};
const AREAS = ["home", "catalog", "programs", "bookmarks", "studyplan"];
// Where a tab leads in the app on its first entry: „Studium" is „Mein Studium" (2026-10-04).
const FIRST = { programs: "/study" };

const context = await browser.newContext(phone);
const { page, swipe, lift, keepHover } = await open(context, "/");
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
await page.evaluate(() => { window.__marker = 1; });
// How far the row went with each move of the finger that has it, as the bar heard the move (before
// its own listener and after): the finger's px along the bar and the row's, and what the bar did
// before (a move that takes the gliding bar comes from "glide").
await page.evaluate(() => {
  const bar = document.querySelector(".bottomnav");
  const along = () => new DOMMatrix(getComputedStyle(bar.querySelector(":scope > .nav")).transform).m41;
  let x = null, before = null;
  window.__moves = [];
  addEventListener("pointerdown", (e) => { x = e.clientX; }, true);
  addEventListener("pointermove", () => { before = { row: along(), swipe: bar.dataset.swipe ?? null }; }, true);
  document.addEventListener("pointermove", (e) => {
    if (x !== null && before && bar.dataset.swipe === "drag") window.__moves.push({ finger: e.clientX - x, row: along() - before.row, from: before.swipe });
    x = e.clientX;
  });
});
const state = () => page.evaluate(barState);
// Once the row and the lens have glided and the tab's own mark has taken over: the current tab then.
const settled = async (what, area) => {
  await page.waitForFunction((area) => {
    const bar = document.querySelector(".bottomnav");
    return !bar.dataset.swipe && bar.querySelector(':scope > .nav[aria-current="page"]')?.dataset.area === area && !document.querySelector("#content[aria-busy]");
  }, area, { timeout: 8000 }).catch(() => {});
  const now = await state();
  check(now.current === area, `${what}: the current tab is ${now.current}, not ${area}`);
  check(!now.swipe && !now.lens && !now.copies && !now.anims, `${what}: the lens is still there (${now.swipe}, ${now.copies} copies, ${now.anims} animations)`);
  check(!now.styled && now.row.every((dx) => dx === 0), `${what}: the bar or a tab kept a style of the swipe (${now.left.join(", ")}; ${now.row})`);
  check(now.loaded, `${what}: the page was loaded again`);
  return now;
};

let now = await settled("start", "home");
const [first, second, middle] = now.mids;
const y = now.bar.mid;
const step = second - first;
check(step > 50, `the tabs stand ${step} px apart`);

// ---- the row follows the finger inside the bar, the mark stays, and the page stands still
const colour = now.own;
await swipe(middle, y, -18, 3, { steps: 3, hold: true });
let held = await state();
check(held.swipe === "drag", `hold: the bar is not taken by the finger (${held.swipe})`);
check(held.row.every((dx) => Math.abs(dx + 10) <= 1), `hold: the row is ${held.row} px along, not -10 (18 less the slop)`);
check(held.lens && Math.abs(held.lens.mid - first) <= 1, `hold: the lens does not stay over the current tab's mark (${held.lens?.mid}, the mark at ${first})`);
check(held.lens?.color === colour, `hold: the lens is ${held.lens?.color}, the tab's own mark ${colour}`);
check(held.own === "rgba(0, 0, 0, 0)", `hold: the current tab shows its own mark as well (${held.own})`);
check(held.copies === held.mids.length && held.off <= 0.5, `hold: the copy in the lens does not lie over the row (${held.copies} copies, ${held.off} px off)`);
check(held.near[0] < 1 && held.near[0] > held.near[1] && held.near[1] > 0, `hold: the names turn at the wrong time (${held.near})`);
check(held.content === now.content && held.path === "/", `hold: the page moved (${now.content} → ${held.content}, ${held.path})`);
// One tab along: the tab on the right under the lens, its name dark.
await swipe(0, 0, -(step - 10), 0, { steps: 6, hold: true, from: true });
held = await state();
check(held.row.every((dx) => Math.abs(dx + step) <= 1), `hold: a tab along, the row is ${held.row[0]} px along, not ${-step}`);
check(held.lens && Math.abs(held.mids[1] - held.lens.mid) <= 1 && held.near[1] === 1 && held.near[0] === 0, `hold: a tab along, the next tab is not under the lens (${held.mids[1]}, the lens at ${held.lens?.mid}; ${held.near})`);
check(held.off <= 0.5, `hold: a tab along, the copy in the lens does not lie over the row (${held.off} px off)`);
// Far past the next tab: held back, never two tabs.
await swipe(0, 0, -150, 0, { steps: 8, hold: true, from: true });
held = await state();
check(held.row[0] < -step && held.row[0] > -step - 15, `pull: the row went ${held.row[0]} px, past the next tab by more than its room`);
check(held.lens && Math.abs(held.lens.mid - first) <= 1, `pull: the lens moved (${held.lens?.mid}, the mark at ${first})`);
check(held.content === now.content && held.path === "/", `pull: the page moved (${now.content} → ${held.content}, ${held.path})`);
await lift();
await page.waitForTimeout(40);
held = await state();
check(held.swipe === "glide" && held.anims === held.mids.length + 2, `let go: the row and the lens do not glide (${held.swipe}, ${held.anims} animations)`);
check(held.off <= 0.75, `let go: on the way the copy in the lens does not lie over the row (${held.off} px off)`);
now = await settled("swipe left", "catalog");
check(now.path.startsWith("/catalog"), `swipe left: the page is ${now.path}`);
check(now.own === colour, `swipe left: the tab's own mark is ${now.own}, not ${colour}`);

// ---- one tab to the left, from anywhere on the bar; a step of the history, as a tap is
const length = now.history;
await swipe(now.mids[4], y, 110, 2);
now = await settled("swipe right", "home");
check(now.path === "/", `swipe right: the page is ${now.path}`);
check(now.history === length + 1, `swipe right: the history grew by ${now.history - length}`);

// ---- at the left end nothing lies further: the row is held back, and the tab stays
await swipe(middle, y, 120, 0, { hold: true });
held = await state();
check(held.row[0] > 0 && held.row[0] < 15, `end: the row went ${held.row[0]} px where no tab is`);
check(held.lens && Math.abs(held.lens.mid - first) <= 1, `end: the lens moved (${held.lens?.mid}, the mark at ${first})`);
await lift();
now = await settled("end left", "home");

// ---- a short slow pull glides back, a short flick goes on
await swipe(middle, y, -26, 0, { steps: 10, ms: 50 });
now = await settled("slow pull", "home");
await swipe(middle, y, -30, 0, { steps: 3, ms: 8 });
now = await settled("flick", "catalog");

// ---- a quick swipe to the tab at an end of the row lets go with the lens past that tab, over
// nothing: a finger that catches the glide there takes the row on from where it is, held back, and
// never against itself (it jumped against the finger: „Probleme, wenn man schnell swiped über den
// rand hinaus")
const catchAtEnd = async (what, dx, area) => {
  await page.evaluate(() => { window.__moves = []; });
  await swipe(middle, y, dx, 0, { steps: 4 });
  await swipe(middle, y, Math.sign(dx) * 14, 0, { steps: 3, hold: true });
  const held = await state();
  check(held.swipe === "drag" && held.current === area, `${what}: the finger did not take the bar (${held.swipe}, ${held.current})`);
  await swipe(0, 0, Math.sign(dx) * 60, 0, { steps: 6, hold: true, from: true });
  await lift();
  const moves = await page.evaluate(() => window.__moves);
  check(moves.some((m) => m.from === "glide"), `${what}: the glide was over before the finger came`);
  const jumps = moves.filter((m) => Math.abs(m.row) > Math.abs(m.finger) + 1.5 || (Math.abs(m.row) > 1.5 && Math.sign(m.row) !== Math.sign(m.finger)));
  check(!jumps.length, `${what}: the row jumped (${jumps.map((m) => `the finger ${m.finger} px, the row ${m.row.toFixed(1)}`).join("; ")})`);
  return settled(what, area);
};
now = await catchAtEnd("catch at the left end", 110, "home");
await swipe(middle, y, -110, 0);
now = await settled("back to the catalog", "catalog");

// ---- along the whole bar, and at the right end no further
for (const area of AREAS.slice(2)) {
  await swipe(middle, y, -110, 0);
  now = await settled(`to ${area}`, area);
  check(now.path.startsWith(FIRST[area] ?? "/" + area), `to ${area}: the page is ${now.path}`);
}
await swipe(middle, y, -120, 0);
now = await settled("end right", "studyplan");

// ---- a finger catches the glide: the lens stays where it is just then, between the two tabs, and
// the swipe goes on from there, two tabs on (from the Stundenplan to the Merkliste, both empty in
// a new browser: nothing keeps the page from hearing the finger while the lens is on its way)
const [, , , fourth, fifth] = now.mids;
await swipe(middle, y, 60, 0, { steps: 4 });
await swipe(middle, y, 14, 0, { steps: 3, hold: true });
held = await state();
check(held.swipe === "drag" && held.anims === 0, `catch: the finger did not take the gliding bar (${held.swipe}, ${held.anims} animations)`);
check(held.current === "bookmarks", `catch: the tab the first swipe went to is not current (${held.current})`);
check(held.lens && held.lens.mid < fifth - 2 && held.lens.mid > fourth + 1, `catch: the lens jumped (${held.lens?.mid}, between ${fourth} and ${fifth})`);
check(held.off <= 0.5, `catch: the copy in the lens does not lie over the row (${held.off} px off)`);
await swipe(0, 0, 60, 0, { steps: 4, from: true });
now = await settled("caught", "programs");
await swipe(middle, y, -110, 0);
now = await settled("to the bookmarks", "bookmarks");
now = await catchAtEnd("catch at the right end", -110, "studyplan");

// ---- a tap right after a quick swipe is a tap (of the tab under the finger: the row moves, and a
// short swipe keeps Start there); two swipes in a row, the second while the first glides, go two
// tabs on
await swipe(middle, y, 30, 0, { steps: 3, ms: 8 });
await page.touchscreen.tap(first, y);
now = await settled("a tap after a swipe", "home");
await swipe(middle, y, -110, 0, { steps: 4 });
await swipe(middle, y, -110, 0, { steps: 4 });
now = await settled("two swipes", "programs");

// ---- a tab tapped and then swiped from keeps the hover the finger left on it (Safari's), which
// lights nothing once another tab is current („wenn man erst was anklickt und dann swiped wird das
// davor angeklickt noch hervorgehoben")
await page.touchscreen.tap(now.mids[1], y);
await page.waitForTimeout(300); // the marks of the two tabs fade (.15 s)
now = await settled("a tap", "catalog");
await keepHover('.bottomnav > .nav[data-area="catalog"]');
await swipe(middle, y, -110, 0);
now = await settled("a swipe from the tapped tab", "programs");
check(now.marks[1] === "rgba(0, 0, 0, 0)" && now.names[1] === now.names[4], `hover: the tab tapped before is still lit (its mark ${now.marks[1]}, its name ${now.names[1]}, another's ${now.names[4]})`);

// ---- up the bar scrolls the page, the tab stays
await swipe(middle, y, 110, 0);
now = await settled("to the catalog", "catalog");
const top = await page.evaluate(() => scrollY);
await swipe(middle, y + 20, 4, -300, { steps: 12 });
await page.waitForTimeout(600);
now = await settled("scroll", "catalog");
check((await page.evaluate(() => scrollY)) > top + 100, `scroll: a finger up the bar did not scroll the page (${top} → ${await page.evaluate(() => scrollY)})`);
await context.close();

// ---- a mouse (a narrow window on a desktop) still lights the tab it points at
const desk = await browser.newContext({ viewport: phone.viewport, serviceWorkers: "block" });
await desk.route("**/pkg/folia_client.js*", (route) => route.fulfill({ contentType: "text/javascript", body: "export default () => new Promise(() => {});" }));
{
  const { page } = await open(desk, "/");
  await page.waitForLoadState("load");
  await page.hover('.bottomnav > .nav[data-area="programs"]');
  await page.waitForTimeout(300);
  const pointed = await page.evaluate(barState);
  check(pointed.marks[2] !== "rgba(0, 0, 0, 0)" && pointed.names[2] !== pointed.names[4], `mouse: the tab pointed at is not lit (its mark ${pointed.marks[2]}, its name ${pointed.names[2]})`);
}
await desk.close();

// ---- while the app is starting (its bundle comes 3 s late): a tab tapped and a swipe wait for it,
// each current at once, and the app shows the last one's page; nothing loads the page again
const starting = await browser.newContext({ ...phone, serviceWorkers: "block" });
await starting.route("**/pkg/folia_client_bg.wasm*", async (route) => { await new Promise((resolve) => setTimeout(resolve, 3000)); await route.continue(); });
{
  const { page, swipe } = await open(starting, "/programs");
  await page.evaluate(() => { window.__marker = 1; });
  const before = await page.evaluate(barState);
  check(await page.evaluate(() => window.__betulaApp !== true), "starting: the app was there before its bundle");
  await page.touchscreen.tap(before.mids[3], before.bar.mid);
  let now = await page.evaluate(barState);
  check(now.current === "bookmarks" && now.loaded, `starting: the tab tapped is not current at once (${now.current}, the page kept: ${now.loaded})`);
  await swipe(before.mids[2], before.bar.mid, -110, 0);
  await page.waitForFunction(() => document.querySelector('.bottomnav > .nav[aria-current="page"]')?.dataset.area === "studyplan", null, { timeout: 2000 }).catch(() => {});
  now = await page.evaluate(barState);
  check(now.current === "studyplan" && now.loaded, `starting: the tab swiped to is not current (${now.current}, the page kept: ${now.loaded})`);
  await page.waitForFunction(() => window.__betulaApp === true && location.pathname === "/studyplan" && !document.querySelector("#content[aria-busy]"), null, { timeout: 30000 }).catch(() => problems.push(`starting: the app did not show the Stundenplan (${page.url()})`));
  now = await page.evaluate(barState);
  check(now.current === "studyplan" && now.loaded, `starting: the app went to ${now.path} with ${now.current} current (the page kept: ${now.loaded})`);
}
await starting.close();

// ---- where the app does not start (its bundle fails): the swipe loads the tab's page, at once
const classic = await browser.newContext({ ...phone, serviceWorkers: "block" });
await classic.route("**/pkg/folia_client.js*", (route) => route.fulfill({ contentType: "text/javascript", body: "export default () => Promise.reject(new Error('no app'));" }));
{
  const { page, swipe } = await open(classic, "/programs");
  await page.waitForLoadState("load");
  await page.waitForFunction(() => window.__betulaStarting === false, null, { timeout: 30000 }).catch(() => problems.push("classic: the start of the app did not end"));
  const before = await page.evaluate(barState);
  check(before.current === "programs", `classic: the current tab is ${before.current}`);
  await swipe(before.mids[2], before.bar.mid, 110, 0);
  await page.waitForURL((url) => url.pathname.startsWith("/catalog"), { timeout: 4000 }).catch(() => problems.push(`classic: the swipe did not load the catalog (${page.url()})`));
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
