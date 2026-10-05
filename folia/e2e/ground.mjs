// The ground at the end of every page (docs/folia/frontend.md „The birch", „One scroll area"). On a wide
// screen every page is one scroll area right of the rail under the header: the page and the ground
// after it scroll natively as one, the window never scrolls, and the wheel over the header or the
// rail moves nothing. What stands beside the page (a sidebar, the filters, a module) is pinned: the
// ground slides over its lower end, and once the area stands still its content ends 8 px above the
// ground, as the page's end does, and the panel ends there with round corners, which the ground
// draws; the first scroll takes them away. The wood behind every page stands on the ground's edge
// in every frame. Tab into the ground brings it into view. A phone scrolls the ground with the page, and
// under a page shorter than the window the ground lies just past the window's lower edge.
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };

const PAGE = "#content > .work.flowing";
// The ground that shows: the page's own on a wide screen, the one after the view on a phone.
const facts = (PAGE) => {
  const box = (el) => el && el.getBoundingClientRect();
  const visible = () => [...document.querySelectorAll(".ground")].find((g) => getComputedStyle(g).display !== "none");
  const page = document.querySelector(PAGE);
  const side = document.querySelector(".sidebar, .filters");
  const body = document.querySelector(".sidebar > .body, .filters .body");
  const ground = box(visible());
  const inner = page.querySelector(":scope > .page > .page-inner");
  return {
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
    // Since `watchWood`: in how many frames the wood did not stand on the ground.
    woodOff: window.__woodOff ?? null,
    pageTop: Math.round(page.scrollTop),
    pageEnd: page.scrollHeight - page.clientHeight - page.scrollTop < 2,
    // Where the page ends (a page of panels: their ring with it), and where the area ends.
    contentBottom: inner ? Math.round(box(inner.parentElement).bottom) : null,
    areaBottom: Math.round(box(page).bottom),
    viewHeight: Math.round(box(document.querySelector(".main")).height),
  };
};
const at = (page) => page.evaluate(facts, PAGE);
// The wood goes with the ground in every frame: the frames in which it did not are counted.
const watchWood = (page) => page.evaluate(() => {
  const visible = () => [...document.querySelectorAll(".ground")].find((g) => getComputedStyle(g).display !== "none");
  window.__woodOff = 0;
  const tick = () => {
    const wood = document.querySelector(".wood").getBoundingClientRect().bottom;
    const ground = visible().getBoundingClientRect().top;
    if (Math.abs(wood - Math.min(ground, innerHeight)) > 0.5) window.__woodOff++;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
});
const wheel = async (page, x, y, dy, times) => {
  await page.mouse.move(x, y);
  for (let i = 0; i < times; i++) { await page.mouse.wheel(0, dy); await page.waitForTimeout(90); }
  await page.waitForTimeout(600);
};
const toEnd = (page) => page.evaluate((PAGE) => { const el = document.querySelector(PAGE); el.scrollTop = el.scrollHeight; }, PAGE);
// The round corners the ground draws for each panel it cut off, once the area stands still with it
// up (`data-rest`): each 1 px wider than its panel on both sides, its foot 8 px above the ground;
// none for a panel that is not there. What is off is listed.
const corners = (page) => page.evaluate((PAGE) => {
  const area = document.querySelector(PAGE);
  const ground = area.querySelector(":scope > .ground").getBoundingClientRect();
  const off = [];
  for (const [name, of] of [["cap-side", ":scope > .filters, :scope > .sidebar"], ["cap-list", ":scope > .list.short"], ["cap-preview", ":scope > .detail"]]) {
    const cap = area.querySelector(`:scope > .ground > .${name}`), panel = area.querySelector(of);
    const shown = Boolean(cap) && getComputedStyle(cap).display !== "none";
    if (!panel) { if (shown) off.push(`${name} without its panel`); continue; }
    if (!shown) { off.push(`no ${name}`); continue; }
    const a = cap.getBoundingClientRect(), b = panel.getBoundingClientRect();
    const at = [a.left, a.right, a.bottom].map(Math.round), wanted = [Math.round(b.left) - 1, Math.round(b.right) + 1, Math.round(ground.top) - 8];
    if (at.join() !== wanted.join()) off.push(`${name} at ${at}, not ${wanted}`);
  }
  return { rest: area.hasAttribute("data-rest"), off };
}, PAGE);
// A scroll of the area takes the round corners away at once, before the ground has moved far
// (owner, 2026-09-30: drawn along with the ground they looked off): whether they are still there
// two frames later.
const nudge = (page) => page.evaluate((PAGE) => new Promise((done) => {
  const area = document.querySelector(PAGE);
  area.scrollTop -= 40;
  requestAnimationFrame(() => requestAnimationFrame(() => done(area.hasAttribute("data-rest") || [...area.querySelectorAll(":scope > .ground > .ground-cap")].some((cap) => getComputedStyle(cap).display !== "none"))));
}), PAGE);
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
  check(f.room === 0 && f.groundTop >= 900 && f.areaBottom === 893 && f.sideBottom === null, `the page at its top: the window has room, or the ground shows: ${JSON.stringify(f)}`);
  await wheel(page, 700, 28, 100, 3);
  await wheel(page, 26, 400, 100, 3);
  f = await at(page);
  check(f.inset === 0 && f.pageTop === 0, `the wheel over the header or the rail moved something: ${JSON.stringify(f)}`);
  // Just short of the ground; then the wheel brings it as the end of the page, the wood on it.
  await page.evaluate((PAGE) => { const el = document.querySelector(PAGE); el.scrollTop = el.scrollHeight - el.clientHeight - 300; }, PAGE);
  await page.waitForTimeout(300);
  await watchWood(page);
  const view = f.viewHeight;
  await wheel(page, 900, 500, 100, 5);
  f = await at(page);
  check(f.pageEnd && f.inset === 0 && f.groundTop === 692 && f.groundTop - f.contentBottom === 8, `the page's end: the ground is not its end, 8 px under the page: ${JSON.stringify(f)}`);
  check(f.woodOff === 0 && f.woodBottom === f.groundTop, `the wood did not stand on the ground in every frame: ${JSON.stringify(f)}`);
  check(f.viewHeight === view && f.rail === 0 && f.topbar === 8, `the ground came: the view, the rail or the header changed: ${JSON.stringify(f)}`);
  await wheel(page, 900, 500, -100, 3);
  f = await at(page);
  check(!f.pageEnd && f.groundTop > 692, `upwards the page and the ground did not go back together: ${JSON.stringify(f)}`);
  // Tab from the page's last link into the ground: it comes into view.
  await page.evaluate((PAGE) => { const el = document.querySelector(PAGE); el.scrollTop = el.scrollHeight - el.clientHeight - 400; const all = [...el.querySelectorAll(":scope > .page a[href], :scope > .page summary")]; all.at(-1).focus(); }, PAGE);
  await page.waitForTimeout(300);
  await page.keyboard.press("Tab");
  await page.waitForTimeout(900);
  const focused = await page.evaluate(() => { const el = document.activeElement; const r = el?.getBoundingClientRect(); return { inGround: Boolean(el?.closest(".ground")), top: r && Math.round(r.top), bottom: r && Math.round(r.bottom), inset: Math.round(scrollY) }; });
  check(focused.inGround && focused.top >= 56 && focused.bottom <= 893 && focused.inset === 0, `Tab into the ground did not bring it into view: ${JSON.stringify(focused)}`);
  // Leaving through the ground: the next page starts at its top.
  await page.click('.work.flowing > .ground .ground-legal a[href="/impressum"]');
  await page.waitForTimeout(1000);
  f = await at(page);
  check(new URL(page.url()).pathname === "/impressum" && f.pageTop === 0 && f.groundTop >= 900, `the page opened from the ground does not start at its top: ${JSON.stringify(f)}`);
  await context.close();
}

// ---------- a framed page: the sidebar pinned beside the page ----------
{
  const { context, page } = await open("/programs");
  let f = await at(page);
  check(f.sideBottom === 892 && f.groundTop >= 900, `a framed page: the sidebar does not end 8 px above the window's edge, or the ground shows: ${JSON.stringify(f)}`);
  await watchWood(page);
  await toEnd(page);
  await wheel(page, 900, 500, 100, 2);
  f = await at(page);
  check(f.pageEnd && f.groundTop === 692 && f.groundTop - f.contentBottom === 8, `a framed page: its end is not 8 px above the ground: ${JSON.stringify(f)}`);
  check(f.sideBottom === 892 && f.groundTop - f.sideBodyBottom === 8, `a framed page: the sidebar is not pinned, or its content does not end 8 px above the ground: ${JSON.stringify(f)}`);
  check(f.woodOff === 0 && f.woodBottom === f.groundTop, `a framed page: the wood does not stand on the ground: ${JSON.stringify(f)}`);
  const round = await corners(page);
  check(round.rest && !round.off.length, `a framed page: the sidebar does not end above the ground with round corners: ${JSON.stringify(round)}`);
  check(!(await nudge(page)), "a framed page: the sidebar's round corners stay while the area scrolls");
  await context.close();
}

// ---------- the catalog: one scroll area (docs/folia/frontend.md „One scroll area") ----------
// The page and the ground after it scroll natively as one; the window never scrolls. A short list is
// pinned beside the pinned filters: the ground slides over their lower ends and their rows stay,
// 8 px between the ground and what stands above it once the area stands still; the filters then
// scroll to their end above it. The whole list flows: its end comes 8 px above the ground. The wood
// stands on the ground in every frame.
{
  const { context, page } = await open("/catalog?q=datenbank");
  const listBottom = () => page.evaluate(() => Math.round(document.querySelector(".panel.list").getBoundingClientRect().bottom));
  let f = await at(page);
  check(f.room === 0 && f.groundTop >= 900 && f.sideBottom === 892, `the catalog: the window has room, or the ground shows, or the filters do not end 8 px above the window's edge: ${JSON.stringify(f)}`);
  check(await page.evaluate(() => document.querySelector(".panel.list").classList.contains("short")), "a short list is not pinned");
  const rows = await page.evaluate(() => Math.round(document.querySelector("#rows .row").getBoundingClientRect().top));
  await watchWood(page);
  await wheel(page, 900, 400, 100, 4);
  f = await at(page);
  const after = await page.evaluate(() => Math.round(document.querySelector("#rows .row").getBoundingClientRect().top));
  check(f.inset === 0 && f.groundTop === 692 && rows === after && f.sideBottom === 892 && f.groundTop - f.sideBodyBottom === 8, `a short list: the ground did not come as the end of the page, or the rows moved: ${JSON.stringify(f)} (rows ${rows} → ${after})`);
  check(f.woodOff === 0 && f.woodBottom === f.groundTop, `a short list: the wood did not stand on the ground in every frame: ${JSON.stringify(f)}`);
  check(f.sideEnd === false, `a short list: the filters are not longer than their panel, so this tells nothing: ${JSON.stringify(f)}`);
  let round = await corners(page);
  check(round.rest && !round.off.length, `a short list: the filters and the list do not end above the ground with round corners: ${JSON.stringify(round)}`);
  await wheel(page, 150, 500, 100, 40);
  f = await at(page);
  check(f.groundTop === 692 && f.sideEnd && f.groundTop - f.sideBodyBottom === 8, `the ground in: the filters do not scroll to their end above it: ${JSON.stringify(f)}`);
  // The filters scrolled, not the area: the round corners stay.
  round = await corners(page);
  check(round.rest && !round.off.length, `the ground in: the filters' scroll took the round corners away: ${JSON.stringify(round)}`);
  await page.goto(base + "/catalog", { waitUntil: "networkidle" });
  await page.waitForTimeout(600);
  await toEnd(page);
  await page.waitForTimeout(600);
  await toEnd(page);
  await wheel(page, 900, 500, 100, 5);
  f = await at(page);
  const end = await listBottom();
  check(f.pageEnd && f.groundTop === 692 && f.groundTop - end === 8 && f.groundTop - f.sideBodyBottom === 8, `the whole list: its end is not 8 px above the ground: ${JSON.stringify(f)} (list ${end})`);
  check(f.woodBottom === f.groundTop, `the catalog: the wood does not stand on the ground: ${JSON.stringify(f)}`);
  round = await corners(page);
  check(round.rest && !round.off.length, `the whole list: the filters do not end above the ground with round corners: ${JSON.stringify(round)}`);
  check(!(await nudge(page)), "the whole list: the filters' round corners stay while the area scrolls");
  await context.close();
}

// ---------- a phone: the ground follows the page ----------
{
  const { context, page } = await open("/", { width: 390, height: 844 });
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await page.waitForTimeout(500);
  const f = await page.evaluate(() => ({ ground: Math.round(document.querySelector("body > .ground").getBoundingClientRect().bottom), inner: innerHeight, position: getComputedStyle(document.querySelector("body > .ground")).position }));
  check(f.position === "relative" && Math.abs(f.ground - f.inner) <= 1, `a phone: the ground is not at the end of the page: ${JSON.stringify(f)}`);
  await context.close();
}

// ---------- a phone, a short page: the ground just past the window's lower edge ----------
// A new visitor's Stundenplan is empty and shorter than the window: the room is left above the
// ground, which lies just past the window's lower edge, to be scrolled to (owner, 2026-10-05: „der
// footer … mindestens genau außerhalb vom bild …, so dass man rein scrollen muss").
{
  const { context, page } = await open("/studyplan", { width: 390, height: 844 });
  const f = await page.evaluate(() => {
    const el = document.querySelector("body > .ground");
    const ground = el.getBoundingClientRect();
    // How far down the page and the ground right after it would reach.
    const needs = document.querySelector("#content").getBoundingClientRect().bottom + parseFloat(getComputedStyle(el).marginTop) + ground.height;
    return { room: document.documentElement.scrollHeight - innerHeight, needs: Math.round(needs), top: Math.round(ground.top), bottom: Math.round(ground.bottom), inner: innerHeight };
  });
  check(f.needs < f.inner, `a phone: the empty Stundenplan is not shorter than the window any more, so it tells nothing: ${JSON.stringify(f)}`);
  check(f.top >= f.inner && f.top - f.inner <= 16 && f.room === f.bottom - f.inner, `a phone, a short page: the ground does not lie just past the window's lower edge: ${JSON.stringify(f)}`);
  await context.close();
}

await browser.close();
if (problems.length) {
  console.log(JSON.stringify(problems, null, 2));
  process.exit(1);
}
console.log("ground: ok");
