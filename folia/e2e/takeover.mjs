// Nothing moves when the app takes the page over (owner, 2026-10-15: „Das Layout darf sich auf gar
// keinen Fall verschieben nach dem Laden"). The server's page, the picture of it that stands in
// front of the app while the app's first answers come (boot.js `takePicture`), and the app's page
// stand alike, box for box: the bar, the search, the page and its first parts, the rows of a list,
// the ground and the tab bar. On a phone (the picture once stood the body's children in a row: the
// ground a narrow strip at the right of the page for a moment) and on a wide screen; at the top of
// a page and scrolled down before the takeover (on a phone the window, on a wide screen the page's
// scroll area), step by step as a finger or a wheel does. A warm start, as in a browser that knows
// the site: the app's bundle is held back until the server's page is measured, and the app's call
// that takes the picture away until the picture is.
//   node takeover.mjs [base-url]      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a box that moved by half a pixel or more, a picture laid out otherwise than the body,
// a picture that stays, or an error on the page.
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };

const PROGRAM = "/programs/bachelor-informatik-2008";
// What must not move: the frame of every page, and the first parts of the pages measured. What
// only the app shows (a module's bookmark, „Einplanen") keeps its room on the server's page and is
// not measured; nor are the regions the server can only stand in for (a filter panel, the
// visitor's own pages) or draw otherwise for search engines (a study plan as a list).
const BOXES = [".crown", ".topbar", ".search", ".top-profile", ".rail", ".main", ".content", ".work", ".ground", ".ground-top", ".ground-foot", ".bottomnav", "h1", ".day-hero", ".ob", ".ob-chip", ".day-discover", ".home-hero", ".topic-head", ".intro-actions > *", ".summary", ".faccs", ".facc", ".prog-head", ".module-page > .hero", ".list-head", ".row"];
const ROWS = 12;
const SCENES = [
  { device: "phone", paths: ["/", "/about/overview", "/about/finding", "/catalog", "/programs", PROGRAM, "/catalog/module/11101", "/impressum"] },
  { device: "phone", paths: ["/", "/catalog"], scroll: 1500 },
  { device: "desktop", paths: ["/", "/about/overview", "/about/finding", "/catalog", PROGRAM, "/impressum"] },
  { device: "desktop", paths: ["/", "/catalog", "/impressum"], scroll: 600 },
];
const DEVICES = {
  phone: { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true },
  desktop: { viewport: { width: 1440, height: 900 } },
};

// The boxes of the page in `scope` (the body, or the picture: a second body after it), as far as
// they are in the window: what moves out of sight cannot be seen to move (the end of the catalog's
// list is the end of the whole catalog in the app, of a page of 50 rows on the server).
const measure = ([scope, selectors, rows]) => {
  const root = document.querySelector(scope);
  if (!root) return null;
  const boxes = {};
  for (const selector of selectors) {
    // Not of a page kept behind (folia/crates/shell/src/stage.rs): every tab's is built at the start.
    [...root.querySelectorAll(selector)].filter((el) => !el.closest("#content > .page-stage:not(.front)")).slice(0, selector === ".row" ? rows : 1).forEach((el, i) => {
      const r = el.getBoundingClientRect();
      const top = Math.max(r.top, 0), bottom = Math.min(r.bottom, innerHeight);
      if ((r.width || r.height) && bottom > top) boxes[`${selector} ${i}`] = [r.x, top, r.width, bottom - top].map((v) => Math.round(v * 100) / 100);
    });
  }
  const style = getComputedStyle(root);
  return { boxes, layout: `${style.display} ${style.flexDirection} ${style.gridTemplateColumns} ${style.gridTemplateRows}` };
};
const compare = (what, before, after) => {
  for (const [key, box] of Object.entries(before.boxes)) {
    const other = after.boxes[key];
    if (!other) continue;
    const moved = Math.max(...box.map((v, i) => Math.abs(v - other[i])));
    check(moved < 0.5, `${what}: ${key} stood at ${box.join(", ")} and stands at ${other.join(", ")}`);
  }
};
// Down to `y` step by step, a frame apart, so that what is drawn only near the screen is drawn on the way.
const scrollDown = (page, y, device) => page.evaluate(async ([y, phone]) => {
  const scroller = phone ? document.scrollingElement : document.querySelector("#content > .page-stage.front > .work.flowing");
  for (let at = 0; at <= y; at += 300) {
    scroller.scrollTop = Math.min(at, y);
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  }
  scroller.scrollTop = y;
}, [y, device === "phone"]);

for (const { device, paths, scroll } of SCENES) {
  // The service worker would answer for the bundle without the route that holds it back.
  const context = await browser.newContext({ ...DEVICES[device], serviceWorkers: "block" });
  await context.addInitScript(() => {
    let answered = null;
    Object.defineProperty(window, "betulaAnswered", {
      configurable: true,
      get() { const take = answered; return take && (() => { window.__takePicture = take; }); },
      set(value) { answered = value; },
    });
  });
  const warm = await context.newPage();
  await warm.goto(base + "/", { waitUntil: "load" });
  await warm.waitForFunction(() => window.__takePicture, null, { timeout: 180000 });
  await warm.evaluate(() => window.__takePicture());
  await warm.close();
  for (const path of paths) {
    const what = `${device} ${path}${scroll ? ` scrolled ${scroll}` : ""}`;
    const page = await context.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(String(error).slice(0, 300)));
    let release;
    const held = new Promise((resolve) => { release = resolve; });
    await page.route("**/pkg/folia_client.js*", async (route) => { await held; await route.continue(); });
    await page.goto(base + path, { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    if (scroll) await scrollDown(page, scroll, device);
    await page.waitForTimeout(400);
    const server = await page.evaluate(measure, ["body", BOXES, ROWS]);
    release();
    if (!(await page.waitForFunction(() => window.__takePicture, null, { timeout: 60000 }).then(() => true, () => false))) {
      check(false, `${what}: the app did not take over`);
      await page.close();
      continue;
    }
    await page.waitForTimeout(300);
    const picture = await page.evaluate(measure, [".takeover-picture", BOXES, ROWS]);
    await page.evaluate(() => window.__takePicture());
    const gone = await page.waitForFunction(() => window.__betulaApp === true && !document.querySelector(".takeover-picture"), null, { timeout: 10000 }).then(() => true, () => false);
    check(gone, `${what}: the picture of the server's page stayed after the takeover`);
    await page.waitForTimeout(600);
    const app = await page.evaluate(measure, ["body", BOXES, ROWS]);
    if (picture) {
      check(picture.layout === server.layout, `${what}: the picture is laid out as „${picture.layout}", the page was „${server.layout}"`);
      compare(`${what}, the picture`, server, picture);
    } else check(false, `${what}: no picture of the server's page while the app took over`);
    compare(`${what}, the app`, server, app);
    check(Object.keys(server.boxes).length > 6, `${what}: only ${Object.keys(server.boxes).length} boxes to compare`);
    check(errors.length === 0, `${what}: errors on the page: ${errors.join(" | ")}`);
    await page.close();
  }
  await context.close();
}

await browser.close();
if (problems.length) {
  console.log(JSON.stringify(problems, null, 2));
  process.exit(1);
}
console.log("takeover: ok");
