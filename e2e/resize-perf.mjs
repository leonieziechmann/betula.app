// Measures how smooth dragging the two resize handles is: frame times while the pointer moves, with
// a long list loaded (the more rows, the more a relayout of the page costs).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node resize-perf.mjs
// Prints numbers; it is a measuring tool, not a pass/fail check (frame times depend on the machine).
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const page = await browser.newPage({ viewport: { width: 1500, height: 900 } });
await page.goto(base + "/catalog?open=11112", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__btuApp === true, null, { timeout: 120000 });

// A long list: five pages of the endless list.
for (let i = 0; i < 4; i++) {
  const before = await page.evaluate(() => document.querySelectorAll(".rows a.row").length);
  await page.evaluate(() => { const rows = document.querySelector(".rows"); rows.scrollTop = rows.scrollHeight; });
  await page.waitForFunction((n) => document.querySelectorAll(".rows a.row").length > n, before, { timeout: 8000 }).catch(() => {});
}
const rows = await page.evaluate(() => document.querySelectorAll(".rows a.row").length);

const drag = async (selector, dx) => {
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
  const moved = Date.now() - started;
  const frames = await page.evaluate(() => { cancelAnimationFrame(window.__raf); return window.__frames.slice(1); });
  await page.mouse.up();
  await page.waitForTimeout(300);
  const sorted = [...frames].sort((a, b) => a - b);
  return {
    moves: 60,
    ms: moved,
    frames: frames.length,
    median: Math.round(sorted[Math.floor(sorted.length / 2)] * 10) / 10,
    p95: Math.round(sorted[Math.floor(sorted.length * 0.95)] * 10) / 10,
    worst: Math.round(sorted.at(-1) * 10) / 10,
    over33ms: frames.filter((f) => f > 33).length,
  };
};

const result = { rows };
result.filters = await drag('[data-action="resize-filters"]', 120);
result.preview = await drag('[data-action="resize-preview"]', -160);
await page.evaluate(() => { localStorage.removeItem("btu.filters.width"); localStorage.removeItem("btu.preview.width"); });
await browser.close();
console.log(JSON.stringify(result, null, 2));
