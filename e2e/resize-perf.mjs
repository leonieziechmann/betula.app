// Measures how smooth dragging the resize handles is: frame times while the pointer moves, on the
// pages where it matters (the catalog with one page of rows and with a long list, the program
// overview, a module's page).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node resize-perf.mjs [--no-fallback]
// `mode` says how the drag ended: "live" (the page followed the handle all the way) or "panel" (the
// page could not keep up and only the panel followed, see enhance.js). `--no-fallback` switches the
// fallback off, to see what laying out the page with every frame really costs.
// Prints numbers; it is a measuring tool, not a pass/fail check (frame times depend on the machine).
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const noFallback = process.argv.includes("--no-fallback");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });

const open = async (path, width = 1500) => {
  const page = await browser.newPage({ viewport: { width, height: 900 } });
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__btuApp === true, null, { timeout: 120000 });
  if (noFallback) await page.evaluate(() => { document.documentElement.dataset.resizeBudget = "100000"; });
  return page;
};

const drag = async (page, selector, dx) => {
  const handle = await page.locator(selector).boundingBox();
  const y = handle.y + 300;
  await page.mouse.move(handle.x + handle.width / 2, y);
  await page.mouse.down();
  await page.evaluate(() => {
    window.__frames = [];
    let last = performance.now();
    const tick = (now) => { window.__frames.push(now - last); last = now; window.__raf = requestAnimationFrame(tick); };
    window.__raf = requestAnimationFrame(tick);
  });
  const started = Date.now();
  for (let i = 1; i <= 60; i++) {
    await page.mouse.move(handle.x + handle.width / 2 + (dx * i) / 60, y);
  }
  const ms = Date.now() - started;
  const { frames, mode } = await page.evaluate(() => { cancelAnimationFrame(window.__raf); return { frames: window.__frames.slice(1), mode: document.documentElement.dataset.resizeMode }; });
  await page.mouse.up();
  await page.waitForTimeout(300);
  await page.evaluate(() => { localStorage.removeItem("btu.filters.width"); localStorage.removeItem("btu.preview.width"); document.documentElement.style.removeProperty("--w-filters"); document.documentElement.style.removeProperty("--preview-w"); });
  const sorted = [...frames].sort((a, b) => a - b);
  const at = (q) => Math.round(sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * q))] * 10) / 10;
  return { mode, ms, frames: frames.length, median: at(0.5), p95: at(0.95), worst: at(1), over33ms: frames.filter((f) => f > 33).length };
};

const result = {};

{
  const page = await open("/catalog?open=11112");
  result["catalog, 50 rows: filters"] = await drag(page, '[data-action="resize-filters"]', 120);
  result["catalog, 50 rows: preview"] = await drag(page, '[data-action="resize-preview"]', -160);
  for (let i = 0; i < 5; i++) {
    const before = await page.evaluate(() => document.querySelectorAll(".rows a.row").length);
    await page.evaluate(() => { const rows = document.querySelector(".rows"); rows.scrollTop = rows.scrollHeight; });
    await page.waitForFunction((n) => document.querySelectorAll(".rows a.row").length > n, before, { timeout: 8000 }).catch(() => {});
  }
  const rows = await page.evaluate(() => document.querySelectorAll(".rows a.row").length);
  result[`catalog, ${rows} rows: filters`] = await drag(page, '[data-action="resize-filters"]', 120);
  result[`catalog, ${rows} rows: preview`] = await drag(page, '[data-action="resize-preview"]', -160);
  await page.close();
}
for (const width of [1500, 1920]) {
  const page = await open("/programs", width);
  result[`programs, ${width}px wide`] = await drag(page, '[data-action="resize-filters"]', 120);
  await page.close();
}
{
  const page = await open("/catalog/module/11112");
  result["module page"] = await drag(page, '[data-action="resize-filters"]', 120);
  await page.close();
}

await browser.close();
console.log(JSON.stringify(result, null, 2));
