// The ground at the end of every page on a wide screen, under a real wheel (docs/frontend.md „The
// birch"). The page's scrollbar is the only one: the window has none and no room to scroll while
// the page is not at its end, so the wheel over the header or the rail changes nothing. At the
// end the next turn of the wheel brings the ground up: the rail stays, the header stays; while the
// window scrolls the panels are only cut off above the ground, and once it stands still the view
// gets shorter by as much, once (not in every frame: that was laggy), the page's end shows above
// the ground and the panel beside it ends above it too, 8 px between every panel and the ground,
// and scrolls to its own end (its content does not move when the ground comes). Upwards the ground
// leaves first, then the page scrolls. A short list
// brings it at once; a page that leaves its end (its scrollbar) and a new page send it away;
// Tab into it brings it up. A phone scrolls the ground with the page, and under a page shorter
// than the window the ground ends at the window's lower edge, not halfway up the screen. The wood
// behind every page stands on the ground's edge as it comes up.
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
    // The foot of the wood behind the page (none on a phone).
    woodBottom: getComputedStyle(document.querySelector(".wood")).display === "none" ? null : Math.round(box(document.querySelector(".wood")).bottom),
    // The start page has no panel beside it (owner, 2026-09-28).
    sideBottom: side ? Math.round(box(side).bottom) : null,
    sideScroll: body ? Math.round(body.scrollTop) : null,
    sideEnd: body ? body.scrollHeight - body.clientHeight - body.scrollTop < 2 : null,
    sideBodyBottom: body ? Math.round(box(body).bottom) : null,
    // Since `watchView`: how often the view changed its size, and how often while the window moved;
    // in how many frames the wood did not stand on the ground; the most steps the page took to its
    // end after the view got shorter.
    viewResized: window.__viewResized ?? null,
    viewWhileMoving: window.__viewWhileMoving ?? null,
    woodOff: window.__woodOff ?? null,
    glideSteps: window.__glideSteps ?? null,
    pageTop: Math.round(page.scrollTop),
    pageEnd: page.scrollHeight - page.clientHeight - page.scrollTop < 2,
    pageBottom: Math.round(box(page).bottom),
  };
};
const at = (page) => page.evaluate(facts, PAGE);
// The view is laid out for the ground once the window stands still, not in every frame while it
// moves (that was laggy): every change of the view's size is counted, and those that came less
// than 100 ms after the window last moved. The wood goes with the ground in every frame, the view
// or not. And once the view got shorter, a page at its end glides to it again, it does not jump.
const watchView = (page) => page.evaluate((PAGE) => {
  window.__viewResized = 0;
  window.__viewWhileMoving = 0;
  window.__woodOff = 0;
  window.__glideSteps = 0;
  let movedAt = -Infinity;
  addEventListener("scroll", () => { movedAt = performance.now(); }, { passive: true });
  let height = document.querySelector(".main").getBoundingClientRect().height;
  let shrunkAt = -Infinity;
  let steps = 0;
  new ResizeObserver(([entry]) => {
    const now = entry.borderBoxSize[0].blockSize;
    if (Math.abs(now - height) < 0.5) return; // a ResizeObserver reports the size it finds at once
    window.__viewResized++;
    if (performance.now() - movedAt < 100) window.__viewWhileMoving++;
    if (now < height) { shrunkAt = performance.now(); steps = 0; }
    height = now;
  }).observe(document.querySelector(".main"));
  document.addEventListener("scroll", (e) => {
    if (e.target !== document.querySelector(PAGE) || performance.now() - shrunkAt > 600) return;
    window.__glideSteps = Math.max(window.__glideSteps, ++steps);
  }, { capture: true, passive: true });
  const tick = () => {
    const wood = document.querySelector(".wood").getBoundingClientRect().bottom;
    const ground = document.querySelector(".ground").getBoundingClientRect().top;
    if (Math.abs(wood - ground) > 0.5) window.__woodOff++;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}, PAGE);
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

// ---------- the start page: a long page, and nothing beside it ----------
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
  await watchView(page);
  await wheel(page, 900, 500, 100, 1);
  f = await at(page);
  check(f.state === "in" && f.inset > 0 && f.inset < 208, `one turn of the wheel at the end did not bring the ground part of the way: ${JSON.stringify(f)}`);
  check(f.woodBottom === f.groundTop, `the wood does not stand on the ground coming up: ${JSON.stringify(f)}`);
  check(f.pageEnd && f.groundTop - f.pageBottom === 8, `part of the way: the view is not laid out for the ground once the window stands still: ${JSON.stringify(f)}`);
  await wheel(page, 900, 500, 100, 4);
  f = await at(page);
  check(f.inset === 208 && f.rail === 0 && f.topbar === 8, `the ground all the way: the rail or the header moved: ${JSON.stringify(f)}`);
  check(f.viewResized > 0 && f.viewWhileMoving === 0, `the view was laid out while the ground moved, not once the window stood still: ${JSON.stringify(f)}`);
  check(f.woodOff === 0, `the wood did not stand on the ground in every frame while it moved: ${JSON.stringify(f)}`);
  check(f.glideSteps >= 4, `the page jumped to its end above the ground instead of gliding there: ${JSON.stringify(f)}`);
  check(f.pageEnd && f.groundTop - f.pageBottom === 8 && f.sideBottom === null, `the ground all the way: the page's end is not 8 px above it, or a panel stands beside the page: ${JSON.stringify(f)}`);
  check(f.woodBottom === f.groundTop, `the ground all the way: the wood does not stand on it: ${JSON.stringify(f)}`);
  await wheel(page, 180, 300, 100, 3);
  const still = await at(page);
  check(still.pageEnd && still.inset === 208, `the wheel over the page at its end moved the page or the ground: ${JSON.stringify(still)}`);
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
  // The filters beside the list, the ground in: the wheel over them scrolls them to their end, and
  // their end shows above the ground (owner, 2026-09-29: with the ground in, the panel did not scroll
  // far enough to see all of it).
  check(f.sideEnd === false, `a short list: the filters are not longer than their panel, so this tells nothing: ${JSON.stringify(f)}`);
  await wheel(page, 150, 500, 100, 40);
  f = await at(page);
  check(f.inset === 208 && f.sideEnd && f.sideBodyBottom <= f.sideBottom && f.groundTop - f.sideBottom === 8, `the ground in: the filters do not scroll to their end above it: ${JSON.stringify(f)}`);
  await page.goto(base + "/catalog", { waitUntil: "networkidle" });
  await page.waitForTimeout(600);
  await toEnd(page);
  await page.waitForTimeout(600);
  await toEnd(page);
  await wheel(page, 900, 500, 100, 5);
  f = await at(page);
  check(f.inset === 208 && f.pageEnd && f.groundTop - f.pageBottom >= 8, `the whole list: its end did not go up with the ground: ${JSON.stringify(f)}`);
  check(f.woodBottom === f.groundTop, `the catalog: the wood does not stand on the ground: ${JSON.stringify(f)}`);
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
