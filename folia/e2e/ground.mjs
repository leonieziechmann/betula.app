// The ground at the end of a page of text without a sidebar — the start page, the pages about Betula —,
// and of no page of the app (docs/folia/frontend.md „The birch", „One scroll area"; owner,
// 2026-11-03: „Bloß in der APP Ui ist das ein wenig wild"). On a wide screen every page is one
// scroll area right of the rail under the header: the start page and the ground after it scroll
// natively as one, the window never scrolls, and the wheel over the rail moves nothing, nor over
// the header of a list's page; a page of panels runs under the header, and the wheel over it turns
// the page. The wood behind the start page stands on the ground's edge in every frame; a page about
// Betula ends in its ground as well. A page of the app ends 8 px above the window's edge, as what is
// pinned beside it (a sidebar, the filters, a module) does, with no round corners drawn, and the
// wood stays on the window's edge. A page of panels runs under the bar, the frost behind the bar
// once it is scrolled, its sidebar pinned below the bar, and another page of the same tab starts
// without the frost; a list's page does not run under it. Tab into the ground brings it into view.
// A phone scrolls the start page's ground with the page; a page of the app ends 12 px above the
// bottom bar, and one shorter than the window does not scroll.
import { chromium } from "playwright-core";
import { fileURLToPath } from "node:url";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };

const PAGE = "#content > .page-stage.front > .work.flowing";
// The ground that shows: the page's own on a wide screen, the one after the view on a phone.
const facts = (PAGE) => {
  const box = (el) => el && el.getBoundingClientRect();
  const visible = () => [...allOnPage(".ground")].find((g) => getComputedStyle(g).display !== "none");
  const page = onPage(PAGE);
  const side = onPage(".sidebar, .filters");
  const body = onPage(".sidebar > .body, .filters .body");
  const ground = box(visible());
  const inner = page.querySelector(":scope > .page > .page-inner");
  return {
    inset: Math.round(scrollY),
    room: document.documentElement.scrollHeight - innerHeight,
    bar: getComputedStyle(document.documentElement).scrollbarWidth,
    rail: Math.round(box(onPage(".rail")).top),
    topbar: Math.round(box(onPage(".topbar")).top),
    groundTop: ground ? Math.round(ground.top) : null,
    // The foot of the wood behind the page (none on a phone).
    woodBottom: getComputedStyle(onPage(".wood")).display === "none" ? null : Math.round(box(onPage(".wood")).bottom),
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
    viewHeight: Math.round(box(onPage(".main")).height),
  };
};
const at = (page) => page.evaluate(facts, PAGE);
// The wood goes with the ground in every frame, and stays on the window's edge where there is none:
// the frames in which it did not are counted.
const watchWood = (page) => page.evaluate(() => {
  const visible = () => [...allOnPage(".ground")].find((g) => getComputedStyle(g).display !== "none");
  window.__woodOff = 0;
  const tick = () => {
    const wood = onPage(".wood").getBoundingClientRect().bottom;
    const ground = visible()?.getBoundingClientRect().top ?? innerHeight;
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
const toEnd = (page) => page.evaluate((PAGE) => { const el = onPage(PAGE); el.scrollTop = el.scrollHeight; }, PAGE);
// Whether the area stands still as with the ground up (`data-rest`), which no page without a ground
// may: its panels would end with round corners above a ground that is not there.
const rests = (page) => page.evaluate((PAGE) => onPage(PAGE).hasAttribute("data-rest") || [...onPage(PAGE).querySelectorAll(".ground-cap")].some((cap) => getComputedStyle(cap).display !== "none"), PAGE);
const open = async (url, viewport = { width: 1440, height: 900 }) => {
  const context = await browser.newContext({ viewport });
  // What a step looks for is on the page in front (on-page.js; the steps' selectors: what is seen).
  await context.addInitScript({ path: fileURLToPath(new URL("on-page.js", import.meta.url)) });
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
  await wheel(page, 26, 400, 100, 3);
  f = await at(page);
  check(f.inset === 0 && f.pageTop === 0, `the wheel over the rail moved something: ${JSON.stringify(f)}`);
  // The page runs under the header: the wheel over the header and the room around it, where the
  // page shows, turns it, every notch (the browser's own wheel did once, from the top); the window
  // stays.
  await wheel(page, 700, 28, 100, 1);
  await wheel(page, 700, 4, 100, 1);
  f = await at(page);
  check(f.inset === 0 && f.pageTop === 200, `the wheel over the header did not turn the page under it, notch by notch: ${JSON.stringify(f)}`);
  // Just short of the ground; then the wheel brings it as the end of the page, the wood on it.
  await page.evaluate((PAGE) => { const el = onPage(PAGE); el.scrollTop = el.scrollHeight - el.clientHeight - 300; }, PAGE);
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
  await page.evaluate((PAGE) => { const el = onPage(PAGE); el.scrollTop = el.scrollHeight - el.clientHeight - 400; const all = [...el.querySelectorAll(":scope > .page a[href], :scope > .page summary")]; all.at(-1).focus(); }, PAGE);
  await page.waitForTimeout(300);
  await page.keyboard.press("Tab");
  await page.waitForTimeout(900);
  const focused = await page.evaluate(() => { const el = document.activeElement; const r = el?.getBoundingClientRect(); return { inGround: Boolean(el?.closest(".ground")), top: r && Math.round(r.top), bottom: r && Math.round(r.bottom), inset: Math.round(scrollY) }; });
  check(focused.inGround && focused.top >= 56 && focused.bottom <= 893 && focused.inset === 0, `Tab into the ground did not bring it into view: ${JSON.stringify(focused)}`);
  // Leaving through the ground: the next page starts at its top.
  await page.click('.work.flowing > .ground .ground-legal a[href="/impressum"] >> visible=true');
  await page.waitForTimeout(1000);
  f = await at(page);
  check(new URL(page.url()).pathname === "/impressum" && f.pageTop === 0 && f.groundTop === null, `the page opened from the ground does not start at its top, or has a ground: ${JSON.stringify(f)}`);
  await context.close();
}

// ---------- a page about Betula: its ground at its end, on a wide screen and on a phone ----------
for (const viewport of [{ width: 1440, height: 900 }, { width: 390, height: 844 }]) {
  const { context, page } = await open("/about/finding", viewport);
  await toEnd(page);
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await page.waitForTimeout(600);
  const f = await at(page);
  const credit = await page.evaluate(() => allOnPage(".photo-credit").filter((line) => line.checkVisibility()).length);
  check(f.groundTop !== null && f.groundTop < viewport.height && credit === 0, `a page about Betula at ${viewport.width} px: its ground does not show at its end, or names a photo: ${JSON.stringify({ ...f, credit })}`);
  await context.close();
}

// ---------- a framed page: the sidebar pinned beside the page ----------
// The privacy notice, long enough to run under the bar.
{
  const { context, page } = await open("/datenschutz");
  let f = await at(page);
  check(f.sideBottom === 892 && f.groundTop === null, `a framed page: the sidebar does not end 8 px above the window's edge, or there is a ground: ${JSON.stringify(f)}`);
  // The page runs under the bar, the frost behind the bar once it is scrolled (owner, 2026-10-26:
  // „Mach das mal so wie bei der mobile version"); the sidebar stays pinned below the bar, and at
  // the top the page stands where it stood.
  const bar = () => page.evaluate((PAGE) => {
    const frost = document.querySelector("body > .frost").getBoundingClientRect(), side = onPage(".sidebar").getBoundingClientRect(), inner = onPage(PAGE).querySelector(":scope > .page");
    return {
      frost: getComputedStyle(document.querySelector("body > .frost")).opacity,
      side: Math.round(side.top),
      page: Math.round(inner.getBoundingClientRect().top),
      // The frost over what scrolls alone: from half the room after the sidebar to the window's
      // edge (the page's scrollbar left clear), not over the sidebar.
      across: Math.round(frost.left) === Math.round(side.right) + 4 && Math.round(frost.right) === innerWidth - (onPage(PAGE).offsetWidth - onPage(PAGE).clientWidth),
    };
  }, PAGE);
  const still = await bar();
  await page.evaluate((PAGE) => { onPage(PAGE).scrollTop = 200; }, PAGE);
  await page.waitForTimeout(400);
  const under = await bar();
  check(still.frost === "0" && still.side === 56 && still.page >= 55 && still.page <= 57 && under.frost === "1" && under.side === 56 && under.page < 0 && under.across, `a framed page: it does not run under the bar with the frost behind it over its column, or the sidebar moves: ${JSON.stringify([still, under])}`);
  await page.evaluate((PAGE) => { onPage(PAGE).scrollTop = 0; }, PAGE);
  await page.waitForTimeout(300);
  await watchWood(page);
  await toEnd(page);
  await wheel(page, 900, 500, 100, 2);
  f = await at(page);
  check(f.pageEnd && f.groundTop === null && f.contentBottom === 892 && f.areaBottom === 893, `a framed page: its end is not 8 px above the window's edge: ${JSON.stringify(f)}`);
  check(f.sideBottom === 892, `a framed page: the sidebar is not pinned 8 px above the window's edge: ${JSON.stringify(f)}`);
  check(f.woodOff === 0 && f.woodBottom === 900, `a framed page: the wood does not stay on the window's edge: ${JSON.stringify(f)}`);
  check(!(await rests(page)), "a framed page: it stands still as with a ground up");
  await context.close();
}

// ---------- another page of the same tab: it starts without the frost of the one before ----------
// A page of the same tab comes in an area of its own, at its top, with no scroll to say so: the
// frost goes with the page that was scrolled.
{
  const { context, page } = await open("/catalog/module/12104");
  await page.evaluate((PAGE) => { onPage(PAGE).scrollTop = 300; }, PAGE);
  await page.waitForTimeout(400);
  const before = await page.evaluate(() => getComputedStyle(document.querySelector("body > .frost")).opacity);
  await page.evaluate(() => { history.pushState(null, "", "/catalog/module/12107"); dispatchEvent(new PopStateEvent("popstate", { state: null })); });
  await page.waitForFunction(() => location.pathname.endsWith("/12107") && document.querySelector(".module-page"), null, { timeout: 8000 }).catch(() => problems.push("another module of the same tab did not come"));
  await page.waitForTimeout(600);
  const after = await page.evaluate((PAGE) => ({ frost: getComputedStyle(document.querySelector("body > .frost")).opacity, top: Math.round(onPage(PAGE).scrollTop) }), PAGE);
  check(before === "1" && after.top === 0 && after.frost === "0", `another page of the same tab starts under the frost of the page before: ${JSON.stringify({ before, after })}`);
  await context.close();
}

// ---------- the catalog: one scroll area (docs/folia/frontend.md „One scroll area") ----------
// The page scrolls natively in it; the window never scrolls. A short list is pinned beside the
// pinned filters, both ending 8 px above the window's edge, and the wheel moves neither its rows
// nor the window; the filters scroll to their end in their panel. The whole list flows: its end
// comes 8 px above the window's edge. The wood stays on the window's edge, there is no ground.
{
  const { context, page } = await open("/catalog?q=datenbank");
  const listBottom = () => page.evaluate(() => Math.round(onPage(".panel.list").getBoundingClientRect().bottom));
  let f = await at(page);
  check(f.room === 0 && f.groundTop === null && f.sideBottom === 892, `the catalog: the window has room, or there is a ground, or the filters do not end 8 px above the window's edge: ${JSON.stringify(f)}`);
  check(await page.evaluate(() => onPage(".panel.list").classList.contains("short")), "a short list is not pinned");
  const rows = await page.evaluate(() => Math.round(onPage("#rows .row").getBoundingClientRect().top));
  await watchWood(page);
  await wheel(page, 900, 400, 100, 4);
  f = await at(page);
  const after = await page.evaluate(() => Math.round(onPage("#rows .row").getBoundingClientRect().top));
  // A list's page does not run under the bar (its head stays at the top): no frost behind the bar.
  check(await page.evaluate(() => getComputedStyle(document.querySelector("body > .frost")).opacity) === "0", "the catalog: the frost lies behind the bar of a list's page, under which nothing runs");
  await wheel(page, 700, 28, 100, 2);
  check((await at(page)).pageTop === f.pageTop, "the catalog: the wheel over the header of a list's page moved it");
  check(f.inset === 0 && f.groundTop === null && rows === after && f.sideBottom === 892 && f.sideBodyBottom === 892 && await listBottom() === 892, `a short list: the rows moved, or it or the filters do not end 8 px above the window's edge: ${JSON.stringify(f)} (rows ${rows} → ${after})`);
  check(f.woodOff === 0 && f.woodBottom === 900, `a short list: the wood did not stay on the window's edge in every frame: ${JSON.stringify(f)}`);
  check(f.sideEnd === false, `a short list: the filters are not longer than their panel, so this tells nothing: ${JSON.stringify(f)}`);
  check(!(await rests(page)), "a short list: it stands still as with a ground up");
  await wheel(page, 150, 500, 100, 40);
  f = await at(page);
  check(f.sideEnd && f.sideBodyBottom === 892, `the filters do not scroll to their end in their panel: ${JSON.stringify(f)}`);
  await page.goto(base + "/catalog", { waitUntil: "networkidle" });
  await page.waitForTimeout(600);
  await toEnd(page);
  await page.waitForTimeout(600);
  await toEnd(page);
  await wheel(page, 900, 500, 100, 5);
  f = await at(page);
  const end = await listBottom();
  check(f.pageEnd && f.groundTop === null && end === 892 && f.sideBodyBottom === 892, `the whole list: its end is not 8 px above the window's edge: ${JSON.stringify(f)} (list ${end})`);
  check(f.woodBottom === 900, `the catalog: the wood does not stay on the window's edge: ${JSON.stringify(f)}`);
  check(!(await rests(page)), "the whole list: it stands still as with a ground up");
  await context.close();
}

// ---------- a phone: the ground follows the page ----------
{
  const { context, page } = await open("/", { width: 390, height: 844 });
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await page.waitForTimeout(500);
  const f = await page.evaluate(() => ({ ground: Math.round(onPage("body > .ground").getBoundingClientRect().bottom), inner: innerHeight, position: getComputedStyle(onPage("body > .ground")).position }));
  check(f.position === "relative" && Math.abs(f.ground - f.inner) <= 1, `a phone: the ground is not at the end of the page: ${JSON.stringify(f)}`);
  await context.close();
}

// ---------- a phone, another page: no ground, its end 12 px above the bottom bar ----------
{
  const { context, page } = await open("/programs", { width: 390, height: 844 });
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await page.waitForTimeout(500);
  const f = await page.evaluate(() => ({
    ground: getComputedStyle(onPage("body > .ground")).display,
    end: Math.round(onPage("#content > .page-stage.front > .work > .page").lastElementChild.getBoundingClientRect().bottom),
    bar: Math.round(onPage(".bottomnav").getBoundingClientRect().top),
  }));
  check(f.ground === "none" && f.bar - f.end >= 12 && f.bar - f.end <= 24, `a phone: the program overview has a ground, or its end is not just above the bottom bar: ${JSON.stringify(f)}`);
  await context.close();
}

// ---------- a phone, a short page: nothing to scroll ----------
// A new visitor's Stundenplan is empty and shorter than the window: with no ground after it, the
// window has nothing to scroll to.
{
  const { context, page } = await open("/studyplan", { width: 390, height: 844 });
  const f = await page.evaluate(() => ({
    ground: getComputedStyle(onPage("body > .ground")).display,
    room: document.documentElement.scrollHeight - innerHeight,
    needs: Math.round(onPage("#content").getBoundingClientRect().bottom),
    inner: innerHeight,
  }));
  check(f.needs < f.inner, `a phone: the empty Stundenplan is not shorter than the window any more, so it tells nothing: ${JSON.stringify(f)}`);
  check(f.ground === "none" && f.room === 0, `a phone, a short page: there is a ground, or the window scrolls into nothing: ${JSON.stringify(f)}`);
  await context.close();
}

await browser.close();
if (problems.length) {
  console.log(JSON.stringify(problems, null, 2));
  process.exit(1);
}
console.log("ground: ok");
