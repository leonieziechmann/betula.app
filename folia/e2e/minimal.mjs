// The minimal version's check (docs/folia-refactor.md §11.1): drives folia-next in Chromium and
// prints what it measured. Fails where the architecture does not hold: the shell remounted, a
// skeleton over a page the server finished, the new snapshot not shown in place.
//   node folia/e2e/minimal.mjs [http://127.0.0.1:8090]
import { chromium } from "../../e2e/node_modules/playwright-core/index.mjs";

const BASE = process.argv[2] || "http://127.0.0.1:8090";
const browser = await chromium.launch({ executablePath: process.env.SMOKE_BROWSER_PATH || undefined });
const out = {};
const fail = (message) => { console.error("FAIL:", message); process.exitCode = 1; };

async function visit(context, path, label) {
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) fail(`${label}: console: ${m.text()}`); });
  page.on("response", (r) => { if (r.status() >= 400 && !r.url().endsWith("/favicon.ico")) fail(`${label}: ${r.status()} ${r.url()}`); });
  page.on("pageerror", (e) => fail(`${label}: ${e.message}`));
  await page.goto(BASE + path);
  await page.waitForFunction(() => window.__foliaReady !== undefined, null, { timeout: 120000 });
  const times = await page.evaluate(() => ({
    firstPaint: Math.round(performance.getEntriesByName("first-contentful-paint")[0]?.startTime ?? -1),
    bundle: Math.round(window.__foliaBundle),
    ready: Math.round(window.__foliaReady),
  }));
  out[label] = times;
  return page;
}

const context = await browser.newContext();

// 0. The takeover moves nothing: the site's page and the app's render of it, pixel for pixel.
{
  const view = { viewport: { width: 1280, height: 900 } };
  const site = await browser.newPage(view);
  await site.route("**/pkg/**", (route) => route.abort());
  await site.goto(BASE + "/catalog/module/11103");
  await site.waitForLoadState("networkidle").catch(() => {});
  const before = await site.screenshot();
  await site.close();
  const app = await browser.newPage(view);
  await app.goto(BASE + "/catalog/module/11103");
  await app.waitForFunction(() => window.__foliaReady !== undefined, null, { timeout: 120000 });
  await app.waitForTimeout(300);
  const after = await app.screenshot();
  await app.close();
  out["takeover: the same pixels"] = before.equals(after);
  if (!before.equals(after)) {
    const fs = await import("node:fs");
    fs.writeFileSync("/tmp/takeover-site.png", before);
    fs.writeFileSync("/tmp/takeover-app.png", after);
  }
}
// 1. A first visit: the site's page paints, the snapshot downloads in the worker, the app takes over.
const page = await visit(context, "/catalog", "first visit /catalog");
const rows = await page.locator("a.row").count();
if (rows !== 50) fail(`the list has ${rows} rows after the takeover, not 50`);
if (await page.locator(".ds-skeleton").count()) fail("a skeleton over the finished list after the takeover");

// 2. The shell stays: mark its elements, navigate, look again.
await page.evaluate(() => { for (const s of [".rail", ".topbar", ".sh-ground", ".crown"]) document.querySelector(s).__kept = true; });
const kept = () => page.evaluate(() => [".rail", ".topbar", ".sh-ground", ".crown"].every((s) => document.querySelector(s)?.__kept === true));
let skeletons = 0;
await page.exposeFunction("__sawSkeleton", () => { skeletons += 1; });
await page.evaluate(() => new MutationObserver(() => { if (document.querySelector(".ds-skeleton")) window.__sawSkeleton(); }).observe(document.body, { childList: true, subtree: true }));
const title = await page.locator("a.row .t b").first().innerText();
let t0 = Date.now();
await page.locator("a.row").first().click();
await page.waitForFunction((t) => document.querySelector(".cat-module h2")?.textContent === t, title);
out["module after click (ms)"] = Date.now() - t0;
if (!(await kept())) fail("the shell was mounted anew by a navigation");
if (page.url().endsWith("/catalog")) fail("no navigation");
await page.goBack();
await page.waitForSelector("a.row");
if (!(await kept())) fail("the shell was mounted anew going back");
const firstRow = await page.locator("a.row").first().getAttribute("data-id");
t0 = Date.now();
await page.locator(".cat-pager a").last().click();
await page.waitForFunction((id) => location.search === "?page=2" && document.querySelector("a.row")?.dataset.id !== id && document.querySelectorAll("a.row").length === 50, firstRow);
out["next page after click (ms)"] = Date.now() - t0;

// 3. New data at once: the server says there is a new snapshot; the page shows it in place.
const before = await page.evaluate(() => window.__foliaTimings.length);
await page.request.get(BASE + "/spike/next-snapshot");
await page.evaluate(() => window.__foliaWorker.postMessage({ type: "visible" }));
t0 = Date.now();
await page.waitForFunction((n) => window.__foliaTimings.length > n && window.__foliaTimings.slice(n).some(([ask]) => ask.startsWith("Catalog { page: 2")), before, { timeout: 120000 });
out["requests before the switch"] = before;
out["new snapshot shown (ms)"] = Date.now() - t0;
if ((await page.locator("a.row").count()) !== 50) fail("the list is not there after the switch");
out["skeletons seen"] = skeletons;
out["requests: [ask, worker ms, total ms]"] = await page.evaluate(() => window.__foliaTimings.map(([a, w, t]) => [a, Math.round(w * 10) / 10, Math.round(t * 10) / 10]));
await page.close();

// 4. A returning visit: the snapshot is in the worker's Cache Storage.
(await visit(context, "/catalog/module/11103", "returning visit, a module")).close();
// 5. A route of the app alone: the shell paints from the server before any WASM.
const app = await context.newPage();
await app.goto(BASE + "/bookmarks", { waitUntil: "commit" });
const html = await (await app.request.get(BASE + "/bookmarks")).text();
if (!html.includes('class="rail sh-rail"')) fail("the app document has no shell");
await app.waitForFunction(() => window.__foliaReady !== undefined, null, { timeout: 60000 });
out["app route /bookmarks"] = await app.evaluate(() => ({ firstPaint: Math.round(performance.getEntriesByName("first-contentful-paint")[0]?.startTime ?? -1), ready: Math.round(window.__foliaReady) }));
await app.close();

// 6. A phone's CPU (four times slower): a click into a module.
const slow = await visit(context, "/catalog?page=3", "returning visit, CPU ×4");
const cdp = await slow.context().newCDPSession(slow);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
const slowTitle = await slow.locator("a.row .t b").nth(5).innerText();
t0 = Date.now();
await slow.locator("a.row").nth(5).click();
await slow.waitForFunction((t) => document.querySelector(".cat-module h2")?.textContent === t, slowTitle);
out["CPU ×4: module after click (ms)"] = Date.now() - t0;
out["CPU ×4: last request [ask, worker ms, total ms]"] = await slow.evaluate(() => window.__foliaTimings.at(-1));

console.log(JSON.stringify(out, null, 2));
await browser.close();
