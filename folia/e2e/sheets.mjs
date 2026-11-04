// Checks the sheets of a phone (owner, 2026-10-27: „Ein modal beim handy ist dieses swipebare menü,
// dass sich von unten öffnet … zwei einrastpunkte … Im kleinen modus soll man nicht scrollen können
// nur im full modus … je nach swipe acceleration und speed in den mid und schließen wechseln …
// Animationen sollen physikalischen Simulationen entsprechen"; folia/assets/enhance.js „the sheets
// of a phone", folia/crates/design/src/modal.rs): a dialog comes up from below in one move to half
// the screen, its × out of sight and its grabber its one decoration; half up a finger takes the
// sheet and not its content, and let go slowly it comes back; a tap on its grabber takes it all up
// and back; all up its content scrolls, over a module's sheet too; from all up a swipe that speeds
// up as it lets go throws it away, one as long and as quick on average that slows down comes to
// rest half up; Esc closes it and a copy of it glides away. Three ways to stand (owner, 2026-11-03:
// „dass es entweder fit ist … zwischen 40% bis 70% vom screen … kein full screen. Dann die Option
// mit 50% und Full screen. Und dann noch die Option für sofort full screen"): the keys' dialog half
// up and all up, its top half up at half the screen; a menu and the dialog „Sammeln" fit, as tall
// as they hold between 40 and 70 % of the screen, one stop that a pull comes back to and a tap on
// the grabber leaves; „Sammeln" (`auto`) half up and all up once it holds more than fits, and
// fitting again once it holds less; „Tasten" naming its keys; „Plan einrichten" all up at once in one move, a pull bringing it back there,
// thrown away from there. A menu swiped down closes, and closed by its choice a copy of it glides
// away. The menus of a tab held and of the profiles are sheets of `auto` as well (owner,
// 2026-11-04): fitting, half up with many ways, thrown away by a finger.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node sheets.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a console error or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
const page = await context.newPage();
const cdp = await context.newCDPSession(page);
page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 600)); });
page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
const HALF = Math.round(844 * 0.5);
// What a sheet that fits may be tall (owner, 2026-11-03: „zwischen 40% bis 70% vom screen").
const FIT = [Math.floor(844 * 0.4), Math.ceil(844 * 0.7)];

// A finger (Playwright itself only taps): down at (x, from), through the moves `by` (px each, one
// every `ms`), up.
async function finger(x, from, by, { ms = 16, hold = 0 } = {}) {
  let y = from;
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
  for (const step of by) {
    y += step;
    await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x, y }] });
    await page.waitForTimeout(ms);
  }
  if (hold) await page.waitForTimeout(hold);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
}
const evenly = (px, steps) => Array.from({ length: steps }, () => px / steps);
// Where the dialog is: its top, all up or not, how far its content is scrolled; null closed.
const dialogAt = (selector) => page.evaluate((selector) => {
  const dialog = document.querySelector(selector);
  if (!dialog?.open) return null;
  const body = dialog.querySelector(".st-dlg-body");
  const r = dialog.getBoundingClientRect();
  return { top: Math.round(r.top), bottom: Math.round(r.bottom), height: Math.round(r.height), way: dialog.dataset.sheet, as: dialog.dataset.sheetAs, full: dialog.classList.contains("is-full"), scrolled: Math.round(body?.scrollTop ?? -1), moving: dialog.getAnimations().some((a) => a.playState === "running") };
}, selector);
const resting = (selector) => page.waitForFunction((selector) => { const d = document.querySelector(selector); return !d?.open || !d.getAnimations().some((a) => a.playState === "running"); }, selector, { timeout: 4000 }).catch(() => problems.push(`${selector}: it does not come to rest`));
// Its top frame by frame for `ms`.
const watch = (selector, ms) => page.evaluate(([selector, ms]) => {
  window.__tops = [];
  const t0 = performance.now();
  const tick = () => {
    const el = document.querySelector(selector);
    if (el?.open !== false && el) window.__tops.push(Math.round(el.getBoundingClientRect().top));
    if (performance.now() - t0 < ms) requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}, [selector, ms]);
const tops = () => page.evaluate(() => window.__tops);

// Six modules marked, for the Merkliste's menu below.
await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true && document.querySelectorAll(".rows a.row").length >= 6, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
await page.evaluate(() => {
  const now = Math.floor(Date.now() / 1000);
  localStorage.setItem("betula.bookmarks.v1", [...document.querySelectorAll(".rows a.row")].slice(0, 6).map((row, i) => `${row.dataset.id}\t${now - i * 60}\n`).join(""));
});
await page.waitForTimeout(500);

// ---- a dialog: half up, from below in one move, the grabber its one decoration
const KEYS = "dialog.keys-dialog";
await watch(KEYS, 900);
await page.keyboard.press("?");
await page.waitForFunction((selector) => document.querySelector(selector)?.open, KEYS, { timeout: 5000 }).catch(() => problems.push("the keys' dialog did not open"));
await resting(KEYS);
let at = await dialogAt(KEYS);
check(at && !at.full && Math.abs(at.top - HALF) < 3, `a dialog does not come up half up: ${JSON.stringify(at)}`);
const rise = (await tops()).filter((top) => top > 0);
check(rise.length > 4 && rise[0] > HALF + 150 && rise.filter((top) => top > HALF + 4 && top < rise[0] - 4).length >= 3 && rise.every((top, i) => i === 0 || top <= rise[i - 1] + 3), `a dialog does not come up from below in one move: ${rise.join(" ")}`);
// (Opened by a key, its × has the keyboard's focus, and shows: away from it, it is out of sight.)
const frame = await page.evaluate((selector) => {
  const dialog = document.querySelector(selector), grip = getComputedStyle(dialog, "::before"), close = dialog.querySelector(".dlg-close");
  if (close?.matches(":focus-visible")) dialog.focus();
  return { grip: `${grip.width}×${grip.height}`, close: Boolean(close?.getAttribute("aria-label")) && getComputedStyle(close).clipPath === "inset(50%)", radius: getComputedStyle(dialog).borderTopLeftRadius, wide: Math.round(dialog.getBoundingClientRect().width) };
}, KEYS);
check(frame.grip === "40px×4px" && frame.close && frame.radius === "18px" && frame.wide === 390, `a dialog's frame is not the sheets' (its grabber, its × out of sight for screen readers): ${JSON.stringify(frame)}`);
// „Tasten" names its keys on a phone too (a phone hides keys elsewhere): each row's keys shown.
const caps = await page.evaluate((selector) => [...document.querySelectorAll(`${selector} .keys-row dt`)].map((dt) => [...dt.querySelectorAll("kbd")].some((kbd) => kbd.getClientRects().length > 0)), KEYS);
check(caps.length > 5 && caps.every(Boolean), `„Tasten“ shows no keys on a phone: ${caps.filter(Boolean).length} of ${caps.length} rows`);

// ---- half up a finger takes the sheet, not its content; let go slowly it comes back
await finger(195, HALF + 140, evenly(-80, 10), { ms: 24, hold: 120 });
await page.waitForTimeout(50);
await resting(KEYS);
at = await dialogAt(KEYS);
check(at && !at.full && Math.abs(at.top - HALF) < 3 && at.scrolled === 0, `half up, a short slow pull scrolled its content or did not come back: ${JSON.stringify(at)}`);
// ---- a tap on the grabber: all up, and back
for (const want of [true, false, true]) {
  const top = (await dialogAt(KEYS)).top;
  await page.touchscreen.tap(195, top + 8);
  await resting(KEYS);
  at = await dialogAt(KEYS);
  check(at?.full === want && (want ? at.top <= 12 : Math.abs(at.top - HALF) < 3), `a tap on the grabber did not take it ${want ? "all up" : "half up"}: ${JSON.stringify(at)}`);
}
// ---- all up its content scrolls
await finger(195, 600, evenly(-300, 10), { ms: 16 });
await page.waitForTimeout(500);
at = await dialogAt(KEYS);
check(at?.full && at.scrolled > 100, `all up, its content does not scroll: ${JSON.stringify(at)}`);
await page.evaluate((selector) => { document.querySelector(`${selector} .st-dlg-body`).scrollTop = 0; }, KEYS);
// ---- from all up: as far and as quick on average, slowing down it comes to rest half up, speeding
// up it goes away (the speed as the finger lets go, not its mean)
const slowing = [80, 60, 50, 40, 30, 20, 10, 6, 4, 4];
await finger(195, 240, slowing, { ms: 20 });
await page.waitForTimeout(80);
await resting(KEYS);
at = await dialogAt(KEYS);
check(at && !at.full && Math.abs(at.top - HALF) < 3, `a swipe down from all up slowing down as it let go did not come to rest half up: ${JSON.stringify(at)}`);
await page.touchscreen.tap(195, at.top + 8);
await resting(KEYS);
await finger(195, 240, [...slowing].reverse(), { ms: 20 });
await page.waitForFunction((selector) => !document.querySelector(selector)?.open, KEYS, { timeout: 3000 }).catch(() => problems.push("a swipe down from all up speeding up as it let go did not throw the dialog away"));

// ---- over a module's sheet a dialog is a sheet of its own: all up its content scrolls; Esc
// closes it, and a copy of it glides away
const row = await page.evaluate(() => { const r = [...document.querySelectorAll(".rows a.row")].find((a) => a.getBoundingClientRect().top > 220).getBoundingClientRect(); return { x: r.left + 60, y: r.top + 20 }; });
await page.touchscreen.tap(row.x, row.y);
await page.waitForFunction(() => document.querySelector(".detail.is-module h2"), null, { timeout: 8000 }).catch(() => problems.push("a module did not come up"));
await page.waitForTimeout(600);
await page.keyboard.press("?");
await page.waitForFunction((selector) => document.querySelector(selector)?.open, KEYS, { timeout: 5000 }).catch(() => problems.push("the keys' dialog did not open over the module"));
await resting(KEYS);
// Thrown away before, it dims the page behind it again.
const dim = await page.evaluate((selector) => getComputedStyle(document.querySelector(selector), "::backdrop").opacity, KEYS);
check(dim === "1", `a dialog thrown away and opened again does not dim the page behind it (${dim})`);
await page.touchscreen.tap(195, (await dialogAt(KEYS)).top + 8);
await resting(KEYS);
await finger(195, 600, evenly(-300, 10), { ms: 16 });
await page.waitForTimeout(500);
at = await dialogAt(KEYS);
check(at?.full && at.scrolled > 100, `over a module's sheet, a dialog all up does not scroll its content: ${JSON.stringify(at)}`);
await page.keyboard.press("Escape");
const copy = await page.waitForFunction(() => document.querySelector(".sheet-ghost .keys-groups") ? Math.round(document.querySelector(".sheet-ghost").getBoundingClientRect().top) : false, null, { timeout: 1000, polling: "raf" }).then((h) => h.jsonValue()).catch(() => null);
check(copy !== null, "Esc: no copy of the dialog glided away");
await page.waitForFunction(() => !document.querySelector(".sheet-ghost") && !document.querySelector("dialog.keys-dialog[open]"), null, { timeout: 3000 }).catch(() => problems.push("Esc: the dialog or its copy stayed"));
check(await page.evaluate(() => Boolean(document.querySelector(".detail.is-module h2")) && !document.querySelector(".detail.is-module").classList.contains("is-gone")), "Esc on the dialog took the module's sheet along");

// ---- a menu: a sheet of one stop, as tall as what it holds
await page.goto(base + "/bookmarks", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true && document.querySelectorAll("#rows a.row, .rows a.row").length >= 6, null, { timeout: 120000 }).catch(() => problems.push("the Merkliste does not list the modules marked"));
await page.waitForTimeout(500);
const MENU = ".view-menu";
const menuAt = () => page.evaluate((selector) => {
  const menu = document.querySelector(`${selector}:not(.sheet-ghost)`);
  if (!menu) return null;
  const r = menu.getBoundingClientRect();
  return { top: Math.round(r.top), bottom: Math.round(r.bottom), height: Math.round(r.height), full: menu.classList.contains("is-full"), sheet: menu.hasAttribute("data-sheet"), grip: getComputedStyle(menu, "::before").width, moving: menu.getAnimations().some((a) => a.playState === "running") };
}, MENU);
await page.tap(".page-stage.front .view-more");
await page.waitForFunction((selector) => document.querySelector(selector), MENU, { timeout: 5000 }).catch(() => problems.push("the Merkliste's ⋯ did not open its menu"));
await page.waitForFunction((selector) => !document.querySelector(selector)?.getAnimations().some((a) => a.playState === "running"), MENU, { timeout: 3000 }).catch(() => {});
const menu = await menuAt();
check(menu?.sheet && menu.full && menu.bottom === 844 && menu.height >= FIT[0] && menu.height <= FIT[1] && menu.grip === "40px", `a menu is no sheet of one stop that fits (40–70 % of the screen): ${JSON.stringify(menu)}`);
await finger(195, menu.top + 30, evenly(160, 8), { ms: 16 });
await page.waitForFunction((selector) => !document.querySelector(selector), MENU, { timeout: 3000 }).catch(() => problems.push("a menu swiped down did not close"));
await page.tap(".page-stage.front .view-more");
await page.waitForFunction((selector) => document.querySelector(`${selector} .st-pop-item`), MENU, { timeout: 5000 }).catch(() => problems.push("the menu did not open again"));
await page.waitForTimeout(400);
await page.tap(`${MENU} .st-pop-item >> nth=0`);
const gone = await page.waitForFunction(() => document.querySelector(".sheet-ghost.view-menu") ? true : false, null, { timeout: 1000, polling: "raf" }).then(() => true).catch(() => false);
check(gone, "a menu closed by its choice left no copy gliding away");
await page.waitForFunction(() => !document.querySelector(".sheet-ghost") && !document.querySelector(".view-menu"), null, { timeout: 3000 }).catch(() => problems.push("the menu or its copy stayed"));

// ---- a dialog of `auto` that fits: „Sammeln" (folia_widgets::collect, `Sheet::Auto`), holding
// little: as tall as it holds between 40 and 70 % of the screen, at its foot; one stop, which a
// short pull comes back to and a tap on the grabber leaves be
const COLLECT = "dialog.collect-sheet";
await page.goto(base + "/catalog/module/11103", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true && document.querySelector(".page-stage.front .mark-more"), null, { timeout: 120000 }).catch(() => problems.push("the module's page has no „Sammeln“"));
await page.waitForTimeout(500);
await page.tap(".page-stage.front .mark-more");
await page.waitForFunction((selector) => document.querySelector(selector)?.open, COLLECT, { timeout: 5000 }).catch(() => problems.push("„Sammeln“ did not open"));
await resting(COLLECT);
at = await dialogAt(COLLECT);
check(at?.way === "auto" && at.as === "fit" && at.full && at.bottom === 844 && at.height >= FIT[0] && at.height <= FIT[1], `a dialog of auto holding little does not fit, as tall as it holds between 40 and 70 % at the screen's foot: ${JSON.stringify(at)}`);
const fitTop = at?.top ?? 0;
await page.touchscreen.tap(195, fitTop + 8);
await resting(COLLECT);
at = await dialogAt(COLLECT);
check(at && Math.abs(at.top - fitTop) < 3, `a tap on the grabber moved a dialog that fits: ${JSON.stringify(at)}`);
await finger(195, fitTop + 60, evenly(70, 10), { ms: 24, hold: 120 });
await resting(COLLECT);
at = await dialogAt(COLLECT);
check(at && Math.abs(at.top - fitTop) < 3 && at.full, `a dialog that fits did not come back from a short pull: ${JSON.stringify(at)}`);
// ---- what it holds grows past what fits (owner, 2026-11-03: „ein Auto Modus, der zwischen fit und
// Half entscheidet für dynamischen content"): half up and all up now — all up as it was, a pull
// takes it half up; shrunk again it fits again, back at its height
await page.evaluate((selector) => { document.querySelector(`${selector} .st-dlg-body`).style.paddingBottom = "520px"; }, COLLECT);
await page.waitForTimeout(150);
await resting(COLLECT);
at = await dialogAt(COLLECT);
check(at?.as === "half" && at.full && at.bottom === 844 && at.height > FIT[1], `a dialog of auto holding more than fits does not stand half up and all up, all up as it was: ${JSON.stringify(at)}`);
await finger(195, (at?.top ?? 200) + 40, [30, 40, 50, 50, 40, 30, 20, 10], { ms: 24 });
await page.waitForTimeout(80);
await resting(COLLECT);
at = await dialogAt(COLLECT);
check(at?.as === "half" && !at.full && Math.abs(at.top - HALF) < 3, `a dialog of auto holding more than fits did not come to rest half up from a pull: ${JSON.stringify(at)}`);
await page.evaluate((selector) => { document.querySelector(`${selector} .st-dlg-body`).style.removeProperty("padding-bottom"); }, COLLECT);
await page.waitForTimeout(150);
await resting(COLLECT);
at = await dialogAt(COLLECT);
check(at?.as === "fit" && at.full && Math.abs(at.top - fitTop) < 3, `a dialog of auto holding little again does not fit again at its height: ${JSON.stringify(at)}`);
await finger(195, fitTop + 60, evenly(260, 8), { ms: 16 });
await page.waitForFunction((selector) => !document.querySelector(selector)?.open, COLLECT, { timeout: 3000 }).catch(() => problems.push("a dialog that fits did not go when swiped down"));

// ---- all up at once: „Plan einrichten" (folia_widgets::plansetup, `Sheet::Full`; „Einplanen" on
// the module's page, no study set up), from below in one move to all up, never half up; a short
// pull comes back all up, a tap on the grabber leaves it, a swipe down throws it away
const SETUP = "dialog.plan-setup";
await page.waitForTimeout(400);
await watch(SETUP, 1200);
await page.tap(".page-stage.front .plan-toggle");
await page.waitForFunction((selector) => document.querySelector(selector)?.open, SETUP, { timeout: 5000 }).catch(() => problems.push("„Einplanen“ did not open „Plan einrichten“"));
await resting(SETUP);
at = await dialogAt(SETUP);
check(at?.way === "full" && at.full && at.top <= 12 && at.bottom === 844, `a dialog all up at once is not all up: ${JSON.stringify(at)}`);
const up = (await tops()).filter((top) => top > 0);
check(up.length > 4 && up[0] > HALF + 150 && up.at(-1) <= 12 && !up.some((top, i) => i > 0 && Math.abs(top - HALF) < 6 && Math.abs(up[i - 1] - top) < 1), `a dialog all up at once does not come up in one move: ${up.join(" ")}`);
await page.touchscreen.tap(195, (at?.top ?? 10) + 8);
await resting(SETUP);
at = await dialogAt(SETUP);
check(at?.full && at.top <= 12, `a tap on the grabber moved a dialog all up at once: ${JSON.stringify(at)}`);
await finger(195, 120, evenly(90, 10), { ms: 24, hold: 120 });
await resting(SETUP);
at = await dialogAt(SETUP);
check(at?.full && at.top <= 12, `a dialog all up at once did not come back all up from a short pull: ${JSON.stringify(at)}`);
await finger(195, 120, [40, 60, 80, 100, 120], { ms: 16 });
await page.waitForFunction((selector) => !document.querySelector(selector)?.open, SETUP, { timeout: 3000 }).catch(() => problems.push("a dialog all up at once did not go when thrown down"));

// ---- the menu of a tab held and the menu of the profiles (folia/crates/shell/src/chrome.rs
// `TabMenu`, `ProfileSwitcher`; owner, 2026-11-04: „guck mal bitte durch, wo das überall sinn ergibt
// das system zu verwenden"): sheets of `auto` from below, over the bar too. The Merkliste's tab held
// with two collections: its ways fit, a tap on the dim closes it. With sixteen it stands half up
// and all up, all up its ways scroll, and a way taken leaves a copy gliding away. The profiles'
// from the box at the head: it fits, and a swipe down throws it away.
const TAB = ".tab-menu:not(.sheet-ghost)";
const tabMenuAt = (selector) => page.evaluate((selector) => {
  const menu = document.querySelector(selector);
  if (!menu) return null;
  const r = menu.getBoundingClientRect();
  return { top: Math.round(r.top), bottom: Math.round(r.bottom), height: Math.round(r.height), way: menu.dataset.sheet, as: menu.dataset.sheetAs, full: menu.classList.contains("is-full"), grip: getComputedStyle(menu, "::before").width, wide: Math.round(r.width), scrolled: Math.round(menu.scrollTop), ways: menu.querySelectorAll(".tab-menu-way").length };
}, selector);
const menuResting = (selector) => page.waitForFunction((selector) => { const m = document.querySelector(selector); return m && !m.getAnimations().some((a) => a.playState === "running"); }, selector, { timeout: 4000 }).catch(() => problems.push(`${selector}: it does not come to rest`));
const shelf = (n) => Array.from({ length: n }, (_, i) => `c\t${i + 1}\tSammlung ${i + 1}\n`).join("");
const holdTab = async (area) => {
  const box = await page.evaluate((area) => { const r = document.querySelector(`.bottomnav > .nav[data-area="${area}"]`).getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; }, area);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: box.x, y: box.y }] });
  await page.waitForTimeout(600);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
};
for (const [n, half] of [[2, false], [16, true]]) {
  await page.evaluate((text) => localStorage.setItem("betula.collections.v1", text), shelf(n));
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true && document.querySelector('.bottomnav > .nav[data-area="bookmarks"]'), null, { timeout: 120000 }).catch(() => problems.push("the catalog has no tab bar"));
  await page.waitForTimeout(600);
  await holdTab("bookmarks");
  await page.waitForFunction((selector) => document.querySelector(selector), TAB, { timeout: 5000 }).catch(() => problems.push(`the Merkliste's tab held (${n} collections) opened no menu`));
  await menuResting(TAB);
  const tab = await tabMenuAt(TAB);
  if (!half) {
    check(tab?.way === "auto" && tab.as === "fit" && tab.full && tab.bottom === 844 && tab.wide === 390 && tab.height >= FIT[0] && tab.height <= FIT[1] && tab.grip === "40px" && tab.ways === n + 2, `the menu of a tab held does not fit as a sheet from below (40–70 % of the screen): ${JSON.stringify(tab)}`);
    await page.touchscreen.tap(195, 120);
    await page.waitForFunction((selector) => !document.querySelector(selector), TAB, { timeout: 3000 }).catch(() => problems.push("the menu of a tab held: a tap on the dim did not close it"));
    check(await page.evaluate(() => location.pathname === "/catalog"), "the menu of a tab held: the tap on the dim went somewhere");
  } else {
    check(tab?.way === "auto" && tab.as === "half" && !tab.full && Math.abs(tab.top - HALF) < 3 && tab.ways === n + 2, `the menu of a tab held with many ways does not come up half up: ${JSON.stringify(tab)}`);
    await page.touchscreen.tap(195, (tab?.top ?? HALF) + 8);
    await menuResting(TAB);
    let all = await tabMenuAt(TAB);
    check(all?.full && all.top <= 12, `a tap on the grabber did not take the menu of a tab held all up: ${JSON.stringify(all)}`);
    await finger(195, 640, evenly(-300, 10), { ms: 16 });
    await page.waitForTimeout(500);
    all = await tabMenuAt(TAB);
    check(all?.full && all.scrolled > 100, `all up, the ways of the menu of a tab held do not scroll: ${JSON.stringify(all)}`);
    await page.tap(`${TAB} .tab-menu-way >> nth=-1`);
    const glided = await page.waitForFunction(() => Boolean(document.querySelector(".sheet-ghost.tab-menu")), null, { timeout: 1000, polling: "raf" }).then(() => true).catch(() => false);
    check(glided, "the menu of a tab held, closed by its way, left no copy gliding away");
    await page.waitForFunction((n) => location.pathname === "/bookmarks" && location.search === `?collection=${n}` && !document.querySelector(".tab-menu"), n, { timeout: 8000 }).catch(() => problems.push("the menu of a tab held: its last way did not lead to its collection, or the menu or its copy stayed"));
  }
}
const PROFILES = ".profile-menu:not(.sheet-ghost)";
await page.waitForTimeout(400);
await page.tap(".topbar .top-profile >> visible=true");
await page.waitForFunction((selector) => document.querySelector(`${selector} .pm-new-way`), PROFILES, { timeout: 5000 }).catch(() => problems.push("a tap on the box at the head opened no menu of the profiles"));
await menuResting(PROFILES);
const profiles = await tabMenuAt(PROFILES);
check(profiles?.way === "auto" && profiles.as === "fit" && profiles.full && profiles.bottom === 844 && profiles.wide === 390 && profiles.height >= FIT[0] && profiles.height <= FIT[1] && profiles.grip === "40px", `the menu of the profiles does not fit as a sheet from below (40–70 % of the screen): ${JSON.stringify(profiles)}`);
await finger(195, (profiles?.top ?? 500) + 30, evenly(220, 8), { ms: 16 });
await page.waitForFunction((selector) => !document.querySelector(selector), PROFILES, { timeout: 3000 }).catch(() => problems.push("the menu of the profiles swiped down did not close"));

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
