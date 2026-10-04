// Checks the browser app: after it has taken over, nothing loads a page again.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node spa.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Walks: catalog → preview (floats, list keeps its width) → resize by dragging → filter → Esc →
// preview → F (full page) → Esc (back) → virtual list (the scrollbar has the whole length, only a
// slice is rendered, the URL follows the position) → arrow keys and Enter → programs search.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const page = await browser.newPage({ viewport: { width: 1500, height: 900 } });
const problems = [];
const timings = {};
page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));

const step = async (name, action, until, arg = null) => {
  const started = Date.now();
  await action();
  try {
    await page.waitForFunction(until, arg, { timeout: 8000 });
    // The page before stays as a picture in front of the new one while its answers come (pending.rs, `hold`).
    await page.waitForFunction(() => !document.querySelector(".pending-page.held"), null, { timeout: 2000 });
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
  }
  timings[name] = Date.now() - started;
  if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
};

const started = Date.now();
await page.goto(base + "/catalog?program=bachelor-informatik-2008", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
timings.takeover = Date.now() - started;
await page.evaluate(() => { window.__marker = 1; });

const count = () => page.evaluate(() => document.querySelector(".count")?.textContent);
const before = await count();

const listWidth = () => page.evaluate(() => Math.round(document.querySelector(".panel.list").getBoundingClientRect().width));
const previewWidth = () => page.evaluate(() => Math.round(document.querySelector(".work > .detail")?.getBoundingClientRect().width || 0));
const widthBefore = await listWidth();
await page.evaluate(() => { document.getElementById("catalog-scroll").scrollTop = 300; });
await step("open preview", () => page.click("a.row >> nth=6"), () => location.search.includes("open=") && document.querySelector(".detail h2"));
const kept = await page.evaluate(() => document.getElementById("catalog-scroll").scrollTop);
if (kept < 250) problems.push(`open preview: the list scrolled back to ${kept}`);
if ((await listWidth()) !== widthBefore) problems.push(`open preview: the list was resized from ${widthBefore} to ${await listWidth()}`);
for (const hint of ["F", "Esc"]) {
  if (!(await page.evaluate((key) => [...document.querySelectorAll(".detail .hero-top kbd")].some((k) => k.textContent === key), hint))) problems.push(`the preview does not show the shortcut ${hint}`);
}

// Drag the left edge 150 px to the left: wider, remembered in localStorage, not in the URL.
const startWidth = await previewWidth();
const handle = await page.locator('[data-action="resize-preview"]').boundingBox();
await page.mouse.move(handle.x + 4, handle.y + handle.height / 2);
await page.mouse.down();
await page.mouse.move(handle.x - 146, handle.y + handle.height / 2, { steps: 6 });
await page.mouse.up();
const dragged = await previewWidth();
if (Math.abs(dragged - (startWidth + 150)) > 3) problems.push(`resize: expected about ${startWidth + 150}px, got ${dragged}px`);
const stored = await page.evaluate(() => localStorage.getItem("betula.preview.width"));
if (Number(stored) !== dragged) problems.push(`resize: localStorage has ${stored}, the preview is ${dragged}px`);
if ((await page.evaluate(() => location.search)).includes(String(dragged))) problems.push("resize: the width leaked into the URL");

await step("filter", () => page.click('#filters a.chip:has-text("Winter")'), () => location.search.includes("turnus=winter") && document.querySelector(".tag"));
if ((await count()) === before) problems.push("filter: the count did not change");
if (!(await page.evaluate(() => Boolean(document.querySelector(".detail h2"))))) problems.push("filter: the preview closed");

await step("escape closes the preview", () => page.keyboard.press("Escape"), () => !location.search.includes("open=") && !document.querySelector(".detail"));
await step("open preview again", () => page.click("a.row >> nth=1"), () => Boolean(document.querySelector(".detail h2")));
if ((await previewWidth()) !== dragged) problems.push(`the preview forgot its width: ${await previewWidth()}px instead of ${dragged}px`);
await step("F opens the full page", () => page.keyboard.press("f"), () => location.pathname.startsWith("/catalog/module/") && document.querySelector(".module-page"));
if (!(await page.evaluate(() => [...document.querySelectorAll(".module-page .hero-top kbd")].some((k) => k.textContent === "Esc")))) problems.push("the module page does not show the shortcut Esc");
await step("Esc goes back to the list", () => page.keyboard.press("Escape"), () => location.pathname === "/catalog" && location.search.includes("open=") && document.querySelector(".rows"));
await step("Esc closes the preview", () => page.keyboard.press("Escape"), () => !location.search.includes("open="));
// The virtual list: as long as the whole list from the start, only what is on screen rendered,
// the last rows there as soon as the list is scrolled to its end, and `page` in the URL follows.
await step("reset the filters", () => page.click('#filters a:has-text("Zurücksetzen")'), () => location.pathname === "/catalog" && !location.search.includes("turnus") && document.querySelectorAll(".rows a.row").length > 10);
const total = await page.evaluate(() => Number(document.querySelector(".count").textContent.replace(/\D/g, "")));
const rowCount = () => page.evaluate(() => document.querySelectorAll(".rows a.row").length);
const lastRendered = () => page.evaluate(() => Math.max(-1, ...[...document.querySelectorAll(".vrow")].map((row) => Number(row.dataset.i))));
const listHeight = () => page.evaluate(() => document.getElementById("catalog-scroll").scrollHeight);
const heightAtStart = await listHeight();
if ((await rowCount()) >= total || (await rowCount()) > 60) problems.push(`virtual list: ${await rowCount()} of ${total} rows are rendered`);
if (heightAtStart < total * 40) problems.push(`virtual list: the list is ${heightAtStart}px tall for ${total} rows`);
await step("scrolling to the end renders the last rows", () => page.evaluate(() => { const rows = document.getElementById("catalog-scroll"); rows.scrollTop = rows.scrollHeight; }), (total) => Math.max(-1, ...[...document.querySelectorAll(".vrow")].map((row) => Number(row.dataset.i))) === total - 1, total);
if ((await rowCount()) > 60) problems.push(`virtual list: ${await rowCount()} rows rendered at the end of the list`);
await step("the URL follows the position", () => page.evaluate(() => { const rows = document.getElementById("catalog-scroll"); rows.scrollTop = rows.scrollHeight; }), (last) => new RegExp(`[?&]page=${last}\\b`).test(location.search), Math.ceil(total / 50));
const heightAtEnd = await listHeight();
if (Math.abs(heightAtEnd - heightAtStart) > heightAtStart * 0.05) problems.push(`virtual list: the scrollbar changed its length from ${heightAtStart}px to ${heightAtEnd}px`);
await step("and back to the top", () => page.evaluate(() => { document.getElementById("catalog-scroll").scrollTop = 0; }), () => !/[?&]page=/.test(location.search) && document.querySelector('.vrow[data-i="0"]'));
const historyBefore = await page.evaluate(() => history.length);

// Keyboard: click a row, two rows down with the arrow keys, Enter opens that one.
const ids = await page.evaluate(() => [...document.querySelectorAll(".rows a.row")].slice(0, 4).map((row) => new URL(row.href).searchParams.get("open")));
await step("click a row", () => page.click("a.row >> nth=0"), () => location.search.includes("open="));
await page.keyboard.press("ArrowDown");
await page.keyboard.press("ArrowDown");
if ((await page.evaluate(() => new URL(location.href).searchParams.get("open"))) !== ids[0]) problems.push("arrow keys: moving the selection already opened another module");
// The clicked row's preview is open already: wait for another one.
await step("Enter opens the selected row", () => page.keyboard.press("Enter"), (clicked) => new URL(location.href).searchParams.get("open") !== clicked && Boolean(document.querySelector(".detail h2")), ids[0]);
const opened = await page.evaluate(() => new URL(location.href).searchParams.get("open"));
if (opened !== ids[2]) problems.push(`arrow keys: expected module ${ids[2]}, the preview shows ${opened}`);
if ((await page.evaluate(() => history.length)) > historyBefore + 2) problems.push("scrolling added history entries");
await page.keyboard.press("Escape");

await step("Studium", () => page.click('.rail a[data-area="programs"]'), () => location.pathname === "/study" && document.querySelector(".st-welcome"));
await step("programs", () => page.click('#sidebar a[href="/programs"]'), () => location.pathname === "/programs" && document.querySelectorAll(".program-pill").length > 50);
await step("program search", () => page.fill("#topsearch", "informatik"), () => location.search.includes("q=informatik") && document.querySelectorAll(".program-pill").length < 30 && document.querySelectorAll(".program-pill").length > 0);

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
