// „Nach oben" (docs/frontend.md „Back to the top"): a button in the corner of what scrolls, out of
// sight until the page is more than a screen down, that takes the page back to its top. On a wide
// screen: the catalog's whole list (virtual) far down → the button in the corner of the list → a
// click → the list at its top with its first row, `page` gone from the address, the button gone,
// no page loaded; the wheel over the button turns the list under it; a wheel stops the way up; with
// a module beside the list the button stands left of it; a keyboard goes on from the start of the
// page; another page starts without it and Back to the list far down brings it again; a long page
// with the ground in (the button above it, the ground going back down); less motion: up at once.
// On a phone: the window, above the bottom bar, a tap; far down before the app takes over, the
// app's button is there after it. The classic site (the app kept away) has it too; without
// JavaScript it is not there.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node top.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };

const PAGE = "#content > .work.flowing, #content #page-scroll, #content #rows, #content > .page, #content > .work > .page";
const open = async (url, { viewport = { width: 1440, height: 900 }, app = true, ...options } = {}) => {
  const context = await browser.newContext({ viewport, ...options });
  // The classic site: the bundle never comes, so the app never takes over (its failed fetch is no problem here).
  if (!app) await context.route(/\/pkg\//, (route) => route.abort());
  const page = await context.newPage();
  if (app) {
    page.on("console", (m) => { if (m.type() === "error") problems.push(`${url}: console: ${m.text().slice(0, 300)}`); });
    page.on("pageerror", (e) => problems.push(`${url}: pageerror: ${String(e).slice(0, 300)}`));
  }
  await page.goto(base + url, { waitUntil: "networkidle" });
  if (app && options.javaScriptEnabled !== false) await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 }).catch(() => problems.push(`${url}: the app did not take over`));
  await page.waitForTimeout(400);
  if (options.javaScriptEnabled !== false) await page.evaluate(() => { window.__marker = 1; });
  return { context, page };
};
// The button: whether enhance.js shows it, whether it can be seen and clicked, and where it is.
const button = (page) => page.evaluate(() => {
  const b = document.getElementById("to-top");
  if (!b) return null;
  const r = b.getBoundingClientRect();
  const s = getComputedStyle(b);
  return { shown: b.hasAttribute("data-shown"), seen: s.display !== "none" && s.visibility === "visible" && s.opacity === "1", left: Math.round(r.left), right: Math.round(r.right), top: Math.round(r.top), bottom: Math.round(r.bottom), width: Math.round(r.width) };
});
// What scrolls: the page on a wide screen, the window on a phone; the first row the list renders.
const where = (page) => page.evaluate((PAGE) => {
  const phone = matchMedia("(max-width: 900px)").matches;
  const el = phone ? document.scrollingElement : document.querySelector(PAGE);
  const rows = [...document.querySelectorAll(".vrow")].map((row) => Number(row.dataset.i));
  return { top: Math.round(el.scrollTop), screen: el.clientHeight, end: el.scrollHeight - el.clientHeight - el.scrollTop < 2, first: rows.length ? Math.min(...rows) : null, search: location.search, inset: Math.round(scrollY), ground: document.documentElement.dataset.ground ?? null, same: window.__marker === 1 };
}, PAGE);
const scrollTo = (page, fraction) => page.evaluate(([PAGE, fraction]) => {
  const phone = matchMedia("(max-width: 900px)").matches;
  const el = phone ? document.scrollingElement : document.querySelector(PAGE);
  el.scrollTop = typeof fraction === "string" ? el.clientHeight * Number(fraction) : (el.scrollHeight - el.clientHeight) * fraction;
}, [PAGE, fraction]);
const wheel = async (page, x, y, dy, times) => {
  await page.mouse.move(x, y);
  for (let i = 0; i < times; i++) { await page.mouse.wheel(0, dy); await page.waitForTimeout(90); }
  await page.waitForTimeout(600);
};

// ---------- a wide screen: the catalog's whole list ----------
{
  const { context, page } = await open("/catalog");
  let b = await button(page);
  check(b && !b.shown && !b.seen, `the list at its top: the button shows: ${JSON.stringify(b)}`);
  await scrollTo(page, "0.8");
  await page.waitForTimeout(400);
  b = await button(page);
  check(!b.shown && !b.seen, `less than a screen down: the button shows: ${JSON.stringify(b)}`);
  await scrollTo(page, 0.6);
  await page.waitForTimeout(700);
  b = await button(page);
  let w = await where(page);
  // The list runs on under the edge of its scroll area: its corner is where the area cuts it off.
  const panel = await page.evaluate(() => { const r = document.querySelector(".panel.list").getBoundingClientRect(), area = document.querySelector("#content > .work.flowing")?.getBoundingClientRect(); return { right: Math.round(r.right), bottom: Math.round(Math.min(r.bottom, area ? area.bottom - 1 : Infinity)) }; });
  check(b.shown && b.seen, `far down the list: the button does not show: ${JSON.stringify(b)} ${JSON.stringify(w)}`);
  check(Math.abs(panel.right - b.right - 16) <= 1 && Math.abs(panel.bottom - b.bottom - 16) <= 1 && b.width === 40, `the button is not in the corner of the list: ${JSON.stringify(b)} ${JSON.stringify(panel)}`);
  check(/[?&]page=\d/.test(w.search) && w.first > 100, `far down the list: the list did not follow: ${JSON.stringify(w)}`);
  // The wheel over the button turns the list under it.
  const before = w.top;
  await wheel(page, (b.left + b.right) / 2, (b.top + b.bottom) / 2, 100, 3);
  w = await where(page);
  check(w.top > before + 150, `the wheel over the button did not turn the list: ${before} → ${JSON.stringify(w)}`);
  await page.click("#to-top");
  await page.waitForTimeout(900);
  w = await where(page);
  b = await button(page);
  check(w.top === 0 && w.first === 0 && !/[?&]page=/.test(w.search) && w.same, `after „Nach oben" the list is not at its top: ${JSON.stringify(w)}`);
  check(!b.shown && !b.seen, `at the top the button stays: ${JSON.stringify(b)}`);
  // A wheel on the way up stops it where it is.
  await scrollTo(page, 0.4);
  await page.waitForTimeout(600);
  await page.click("#to-top");
  await wheel(page, 700, 500, 100, 1);
  w = await where(page);
  check(w.top > 0, `a wheel did not stop the way up: ${JSON.stringify(w)}`);
  // A module beside the list: the button stands left of it, and the module stays.
  await scrollTo(page, 0.5);
  await page.waitForTimeout(700);
  await page.click(".vrow a.row >> nth=4");
  await page.waitForFunction(() => /[?&]open=/.test(location.search) && document.querySelector(".work > .detail h2"), null, { timeout: 8000 }).catch(() => problems.push("the module did not open beside the list"));
  await page.waitForTimeout(500);
  b = await button(page);
  const preview = await page.evaluate(() => Math.round(document.querySelector(".work > .detail").getBoundingClientRect().left));
  check(b.shown && b.seen && Math.abs(preview - b.right - 16) <= 1, `with a module beside the list the button is not left of it: ${JSON.stringify(b)}, the module from ${preview}`);
  await page.click("#to-top");
  await page.waitForTimeout(900);
  w = await where(page);
  check(w.top === 0 && w.first === 0 && /[?&]open=/.test(w.search) && w.same, `with a module beside the list: not at the top, or the module went: ${JSON.stringify(w)}`);
  // The keyboard: Enter on the button, and Tab goes on from the start of the page.
  await scrollTo(page, 0.5);
  await page.waitForTimeout(700);
  await page.keyboard.press("Shift");
  await page.evaluate(() => document.getElementById("to-top").focus());
  const visible = await page.evaluate(() => document.getElementById("to-top").matches(":focus-visible"));
  await page.keyboard.press("Enter");
  await page.waitForTimeout(900);
  w = await where(page);
  const focus = await page.evaluate(() => document.activeElement?.id);
  await page.keyboard.press("Tab");
  const next = await page.evaluate(() => Boolean(document.activeElement?.closest("#filters")));
  check(visible && w.top === 0 && focus === "content" && next, `Enter on the button: the page is not at its top, or the keyboard does not go on from its start: ${JSON.stringify({ visible, focus, next, ...w })}`);
  // Another page starts without it (it replaces the list without a scroll); back at the list,
  // which comes back at the module's row far down, it is there again.
  await scrollTo(page, 0.5);
  await page.waitForTimeout(700);
  await page.click(".vrow a.row >> nth=4");
  await page.waitForFunction(() => /[?&]open=/.test(location.search) && document.querySelector('.work > .detail [data-action="fullscreen"]'), null, { timeout: 8000 }).catch(() => problems.push("the module did not open beside the list again"));
  await page.keyboard.press("f");
  await page.waitForFunction(() => location.pathname.startsWith("/catalog/module/") && document.querySelector(".module-page"), null, { timeout: 8000 }).catch(() => problems.push("F did not open the module's page"));
  await page.waitForTimeout(500);
  b = await button(page);
  check(!b.shown && !b.seen, `the module's page starts with the button of the list: ${JSON.stringify(b)}`);
  await page.goBack();
  await page.waitForFunction(() => location.pathname === "/catalog" && document.querySelector(".vrow"), null, { timeout: 8000 }).catch(() => problems.push("Back did not lead to the list"));
  await page.waitForTimeout(900);
  b = await button(page);
  w = await where(page);
  check(b.shown && b.seen && w.top > w.screen, `back at the list far down the button is not there: ${JSON.stringify(b)} ${JSON.stringify(w)}`);
  await context.close();
}

// ---------- a wide screen: a long page and the ground ----------
{
  const { context, page } = await open("/");
  await scrollTo(page, 1);
  await page.waitForTimeout(400);
  await wheel(page, 900, 500, 100, 5);
  let w = await where(page);
  const b = await button(page);
  const ground = await page.evaluate(() => Math.round(document.querySelector(".ground").getBoundingClientRect().top));
  check(w.ground === "in" && w.inset === 208 && b.shown && b.seen && b.bottom <= ground - 16, `the ground in: the button is not above it: ${JSON.stringify(b)}, the ground from ${ground}, ${JSON.stringify(w)}`);
  await page.click("#to-top");
  await page.waitForTimeout(1200);
  w = await where(page);
  check(w.top === 0 && w.inset === 0 && w.ground === "mid" && w.same, `after „Nach oben" the page is not at its top, or the ground stayed: ${JSON.stringify(w)}`);
  await context.close();
}

// ---------- less motion: up at once ----------
{
  const { context, page } = await open("/catalog", { reducedMotion: "reduce" });
  await scrollTo(page, 0.5);
  await page.waitForTimeout(700);
  const top = await page.evaluate((PAGE) => { document.getElementById("to-top").click(); return document.querySelector(PAGE).scrollTop; }, PAGE);
  check(top === 0, `less motion: the list is not at its top at once (${top})`);
  await context.close();
}

// ---------- a phone: the window, above the bottom bar ----------
{
  const { context, page } = await open("/catalog", { viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  await scrollTo(page, 0.5);
  await page.waitForTimeout(700);
  let b = await button(page);
  const bar = await page.evaluate(() => { const r = document.querySelector(".bottomnav").getBoundingClientRect(); return { top: Math.round(r.top), right: Math.round(r.right) }; });
  check(b.shown && b.seen && b.width === 44 && bar.top - b.bottom === 12 && bar.right === b.right, `a phone: the button is not above the bottom bar: ${JSON.stringify(b)} ${JSON.stringify(bar)}`);
  await page.tap("#to-top");
  await page.waitForTimeout(900);
  const w = await where(page);
  b = await button(page);
  check(w.top === 0 && w.first === 0 && !/[?&]page=/.test(w.search) && !b.shown && w.same, `a phone: after „Nach oben" the list is not at its top: ${JSON.stringify(w)} ${JSON.stringify(b)}`);
  await context.close();
}
// Far down before the app takes over: the app brings a button of its own, and nothing scrolls.
{
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  let open;
  const scrolled = new Promise((resolve) => { open = resolve; });
  await context.route(/\/pkg\//, async (route) => { await scrolled; await route.continue(); });
  const page = await context.newPage();
  await page.goto(base + "/catalog?page=2", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => document.getElementById("to-top") && document.querySelectorAll(".rows a.row").length > 20, null, { timeout: 30000 });
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await page.waitForTimeout(400);
  const before = await button(page);
  open();
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 }).catch(() => problems.push("far down before the takeover: the app did not take over"));
  await page.waitForTimeout(900);
  const w = await where(page);
  const b = await button(page);
  check(before.shown && (w.top <= w.screen || (b.shown && b.seen)), `far down before the takeover: the app's button is not there: ${JSON.stringify(before)} → ${JSON.stringify(b)} ${JSON.stringify(w)}`);
  await context.close();
}

// ---------- the classic site, and without JavaScript ----------
{
  const { context, page } = await open("/catalog?page=2", { app: false });
  const app = await page.evaluate(() => window.__betulaApp === true);
  await scrollTo(page, 0.9);
  await page.waitForTimeout(500);
  const b = await button(page);
  check(!app && b.shown && b.seen, `the classic site: the button does not show far down: ${JSON.stringify(b)}`);
  await page.click("#to-top");
  await page.waitForTimeout(900);
  const w = await where(page);
  check(w.top === 0, `the classic site: after „Nach oben" the list is not at its top: ${JSON.stringify(w)}`);
  await context.close();
}
{
  const { context, page } = await open("/catalog?page=2", { javaScriptEnabled: false, app: false });
  const shown = await page.evaluate(() => { const b = document.getElementById("to-top"); return b ? getComputedStyle(b).display : "none"; });
  check(shown === "none", `without JavaScript the button is there (${shown})`);
  await context.close();
}

await browser.close();
if (problems.length) {
  console.log(JSON.stringify(problems, null, 2));
  process.exit(1);
}
console.log("top: ok");
