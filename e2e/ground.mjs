// The ground at the end of every page on a wide screen, under a real wheel (docs/frontend.md „The
// birch"). The page's scrollbar is the only one: the window has none and no room to scroll while
// the page is not at its end, so the wheel over the header or the rail changes nothing. At the
// end the next turn of the wheel brings the ground up: the rail stays, the header stays, the view
// gets shorter by as much, the page's end goes up with the ground and the panel beside it is only
// cut off (its content does not move, and the wheel over it does not scroll it), 8 px between
// every panel and the ground. Upwards the ground leaves first, then the page scrolls. A short list
// brings it at once; a page that leaves its end (its scrollbar) and a new page send it away;
// Tab into it brings it up. A phone scrolls the ground with the page, and under a page shorter
// than the window the ground ends at the window's lower edge, not halfway up the screen.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node ground.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };

const PAGE = "#content #page-scroll, #content #rows, #content > .page, #content > .work > .page";
const facts = (PAGE) => {
  const box = (el) => el && el.getBoundingClientRect();
  const page = document.querySelector(PAGE);
  const side = document.querySelector(".sidebar, .filters");
  const body = document.querySelector(".sidebar > .body, .filters .body");
  const ground = box(document.querySelector(".ground"));
  return {
    state: document.documentElement.dataset.ground ?? null,
    inset: Math.round(scrollY),
    room: document.documentElement.scrollHeight - innerHeight,
    bar: getComputedStyle(document.documentElement).scrollbarWidth,
    rail: Math.round(box(document.querySelector(".rail")).top),
    topbar: Math.round(box(document.querySelector(".topbar")).top),
    groundTop: Math.round(ground.top),
    sideBottom: Math.round(box(side).bottom),
    sideScroll: Math.round(body.scrollTop),
    pageTop: Math.round(page.scrollTop),
    pageEnd: page.scrollHeight - page.clientHeight - page.scrollTop < 2,
    pageBottom: Math.round(box(page).bottom),
  };
};
const at = (page) => page.evaluate(facts, PAGE);
const wheel = async (page, x, y, dy, times) => {
  await page.mouse.move(x, y);
  for (let i = 0; i < times; i++) { await page.mouse.wheel(0, dy); await page.waitForTimeout(90); }
  await page.waitForTimeout(600);
};
const toEnd = (page) => page.evaluate((PAGE) => { const el = document.querySelector(PAGE); el.scrollTop = el.scrollHeight; }, PAGE);
const open = async (url, viewport = { width: 1440, height: 900 }) => {
  const context = await browser.newContext({ viewport });
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`${url}: console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`${url}: pageerror: ${String(e).slice(0, 300)}`));
  await page.goto(base + url, { waitUntil: "networkidle" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 30000 }).catch(() => problems.push(`${url}: the app did not take over`));
  await page.waitForTimeout(400);
  return { context, page };
};

// ---------- the start page: a long page ----------
{
  const { context, page } = await open("/");
  let f = await at(page);
  check(f.state === "mid" && f.room === 0 && f.bar === "none", `the page at its top: the window has room or a scrollbar: ${JSON.stringify(f)}`);
  await wheel(page, 700, 28, 100, 3);
  await wheel(page, 26, 400, 100, 3);
  f = await at(page);
  check(f.inset === 0 && f.pageTop === 0, `the wheel over the header or the rail moved something: ${JSON.stringify(f)}`);
  await toEnd(page);
  await page.waitForTimeout(200);
  f = await at(page);
  check(f.state === "end" && f.room === 208 && f.inset === 0, `the page at its end: the window has no room for the ground: ${JSON.stringify(f)}`);
  const sideBefore = f.sideBottom;
  await wheel(page, 900, 500, 100, 1);
  f = await at(page);
  check(f.state === "in" && f.inset > 0 && f.inset < 208, `one turn of the wheel at the end did not bring the ground part of the way: ${JSON.stringify(f)}`);
  await wheel(page, 900, 500, 100, 4);
  f = await at(page);
  check(f.inset === 208 && f.rail === 0 && f.topbar === 8, `the ground all the way: the rail or the header moved: ${JSON.stringify(f)}`);
  check(f.pageEnd && f.groundTop - f.pageBottom === 8 && f.groundTop - f.sideBottom === 8, `the ground all the way: the page's end or the panel is not 8 px above it: ${JSON.stringify(f)}`);
  check(sideBefore - f.sideBottom === 208 && f.sideScroll === 0, `the panel beside the page was not just cut off: ${JSON.stringify(f)} (bottom before ${sideBefore})`);
  await wheel(page, 180, 300, 100, 3);
  const still = await at(page);
  check(still.sideScroll === 0 && still.inset === 208, `the wheel over the cut panel scrolled something: ${JSON.stringify(still)}`);
  await wheel(page, 180, 300, -100, 1);
  f = await at(page);
  check(f.inset > 0 && f.inset < 208 && f.pageEnd, `upwards the ground did not leave first: ${JSON.stringify(f)}`);
  await wheel(page, 900, 500, -100, 4);
  f = await at(page);
  check(f.inset === 0 && f.state === "mid" && !f.pageEnd, `after the ground the page did not scroll back: ${JSON.stringify(f)}`);
  // The page leaves its end while the ground shows, by its scrollbar: the ground makes way. (Until
  // „Betula im Detail" followed them, the questions ended the page, and one opened did the same.)
  await toEnd(page);
  await wheel(page, 900, 500, 100, 5);
  await page.evaluate((PAGE) => { document.querySelector(PAGE).scrollTop -= 400; }, PAGE);
  await page.waitForTimeout(900);
  f = await at(page);
  check(f.inset === 0 && f.state !== "in", `the page left its end under the ground: the ground stayed: ${JSON.stringify(f)}`);
  // Tab into the ground: it comes up. On the way from the page's end „Nach oben", which stands
  // between the page and the ground (the page is far down, so it shows) and leaves the ground down.
  await page.evaluate((PAGE) => { const el = document.querySelector(PAGE); el.scrollTop = el.scrollHeight; const all = [...el.querySelectorAll("a[href], summary")]; all.at(-1).focus(); }, PAGE);
  await page.waitForTimeout(300);
  await page.keyboard.press("Tab");
  await page.waitForTimeout(300);
  const between = await page.evaluate(() => ({ at: document.activeElement?.id, inset: Math.round(scrollY) }));
  await page.keyboard.press("Tab");
  await page.waitForTimeout(900);
  f = await at(page);
  const focused = await page.evaluate(() => Boolean(document.activeElement?.closest(".ground")));
  check(between.at === "to-top" && between.inset === 0, `Tab from the page's end did not reach „Nach oben" first: ${JSON.stringify(between)}`);
  check(focused && f.inset === 208, `Tab into the ground did not bring it up: ${JSON.stringify(f)}`);
  // Leaving through the ground: the next page starts without it.
  await page.click('.ground-legal a[href="/impressum"]');
  await page.waitForTimeout(1000);
  f = await at(page);
  check(new URL(page.url()).pathname === "/impressum" && f.inset === 0, `the page opened from the ground starts with it: ${JSON.stringify(f)}`);
  await context.close();
}

// ---------- the catalog: a short list and the whole list ----------
{
  const { context, page } = await open("/catalog?q=datenbank");
  let f = await at(page);
  check(f.state === "end", `a short list is at its end: ${JSON.stringify(f)}`);
  const rows = await page.evaluate(() => Math.round(document.querySelector("#rows .row").getBoundingClientRect().top));
  await wheel(page, 900, 400, 100, 4);
  f = await at(page);
  const after = await page.evaluate(() => Math.round(document.querySelector("#rows .row").getBoundingClientRect().top));
  check(f.inset === 208 && rows === after && f.groundTop - f.sideBottom === 8, `a short list: the ground did not come, or the rows moved: ${JSON.stringify(f)} (rows ${rows} → ${after})`);
  await page.goto(base + "/catalog", { waitUntil: "networkidle" });
  await page.waitForTimeout(600);
  await toEnd(page);
  await page.waitForTimeout(600);
  await toEnd(page);
  await wheel(page, 900, 500, 100, 5);
  f = await at(page);
  check(f.inset === 208 && f.pageEnd && f.groundTop - f.pageBottom >= 8, `the whole list: its end did not go up with the ground: ${JSON.stringify(f)}`);
  await context.close();
}

// ---------- a phone: the ground follows the page ----------
{
  const { context, page } = await open("/", { width: 390, height: 844 });
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await page.waitForTimeout(500);
  const f = await page.evaluate(() => ({ state: document.documentElement.dataset.ground ?? null, ground: Math.round(document.querySelector(".ground").getBoundingClientRect().bottom), inner: innerHeight, position: getComputedStyle(document.querySelector(".ground")).position }));
  check(f.state === null && f.position === "relative" && Math.abs(f.ground - f.inner) <= 1, `a phone: the ground is not at the end of the page: ${JSON.stringify(f)}`);
  await context.close();
}

// ---------- a phone, a short page: the ground at the window's lower edge ----------
// A new visitor's Stundenplan is empty and shorter than the window: the room is left above the
// ground, which does not float halfway up the screen.
{
  const { context, page } = await open("/studyplan", { width: 390, height: 844 });
  const f = await page.evaluate(() => {
    const el = document.querySelector(".ground");
    const ground = el.getBoundingClientRect();
    // How far down the page and the ground right after it would reach.
    const needs = document.querySelector("#content").getBoundingClientRect().bottom + parseFloat(getComputedStyle(el).marginTop) + ground.height;
    return { room: document.documentElement.scrollHeight - innerHeight, needs: Math.round(needs), ground: Math.round(ground.bottom), inner: innerHeight };
  });
  check(f.needs < f.inner, `a phone: the empty Stundenplan is not shorter than the window any more, so it tells nothing: ${JSON.stringify(f)}`);
  check(f.room === 0 && Math.abs(f.ground - f.inner) <= 1, `a phone, a short page: the ground does not end at the window's lower edge: ${JSON.stringify(f)}`);
  await context.close();
}

await browser.close();
if (problems.length) {
  console.log(JSON.stringify(problems, null, 2));
  process.exit(1);
}
console.log("ground: ok");
