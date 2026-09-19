// Checks the browser app: after it has taken over, nothing loads a page again.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node spa.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Walks: catalog → preview (floats, list keeps its width) → resize by dragging → filter → Esc →
// preview → F (full page) → Esc (back) → programs search.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const page = await browser.newPage({ viewport: { width: 1500, height: 900 } });
const problems = [];
const timings = {};
page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));

const step = async (name, action, until) => {
  const started = Date.now();
  await action();
  try {
    await page.waitForFunction(until, null, { timeout: 8000 });
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
  }
  timings[name] = Date.now() - started;
  if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
};

const started = Date.now();
await page.goto(base + "/catalog?program=bachelor-informatik-2008", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__btuApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
timings.takeover = Date.now() - started;
await page.evaluate(() => { window.__marker = 1; });

const count = () => page.evaluate(() => document.querySelector(".count")?.textContent);
const before = await count();

const listWidth = () => page.evaluate(() => Math.round(document.querySelector(".panel.list").getBoundingClientRect().width));
const previewWidth = () => page.evaluate(() => Math.round(document.querySelector(".work > .detail")?.getBoundingClientRect().width || 0));
const widthBefore = await listWidth();
await page.evaluate(() => { document.querySelector(".rows").scrollTop = 300; });
await step("open preview", () => page.click("a.row >> nth=6"), () => location.search.includes("open=") && document.querySelector(".detail h2"));
const kept = await page.evaluate(() => document.querySelector(".rows").scrollTop);
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
const stored = await page.evaluate(() => localStorage.getItem("btu.preview.width"));
if (Number(stored) !== dragged) problems.push(`resize: localStorage has ${stored}, the preview is ${dragged}px`);
if ((await page.evaluate(() => location.search)).includes(String(dragged))) problems.push("resize: the width leaked into the URL");

await step("filter", () => page.click('label.chip:has(input[name="turnus"][value="winter"])'), () => location.search.includes("turnus=winter") && document.querySelector(".tag"));
if ((await count()) === before) problems.push("filter: the count did not change");
if (!(await page.evaluate(() => Boolean(document.querySelector(".detail h2"))))) problems.push("filter: the preview closed");

await step("escape closes the preview", () => page.keyboard.press("Escape"), () => !location.search.includes("open=") && !document.querySelector(".detail"));
await step("open preview again", () => page.click("a.row >> nth=1"), () => Boolean(document.querySelector(".detail h2")));
if ((await previewWidth()) !== dragged) problems.push(`the preview forgot its width: ${await previewWidth()}px instead of ${dragged}px`);
await step("F opens the full page", () => page.keyboard.press("f"), () => location.pathname.startsWith("/catalog/module/") && document.querySelector(".module-page"));
if (!(await page.evaluate(() => [...document.querySelectorAll(".module-page .hero-top kbd")].some((k) => k.textContent === "Esc")))) problems.push("the module page does not show the shortcut Esc");
await step("Esc goes back to the list", () => page.keyboard.press("Escape"), () => location.pathname === "/catalog" && location.search.includes("open=") && document.querySelector(".rows"));
await step("Esc closes the preview", () => page.keyboard.press("Escape"), () => !location.search.includes("open="));
await step("programs", () => page.click('.rail a[href="/programs"]'), () => location.pathname === "/programs" && document.querySelectorAll(".card-link").length > 50);
await step("program search", () => page.fill("#topsearch", "informatik"), () => location.search.includes("q=informatik") && document.querySelectorAll(".card-link").length < 30 && document.querySelectorAll(".card-link").length > 0);

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
