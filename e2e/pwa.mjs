// Checks that the app starts without a network: the service worker keeps the shell, IndexedDB
// the catalog. A first visit with the network installs both; then the network is cut and pages
// are loaded afresh (a full load, not a step of the app) — the catalog with a filter, a module,
// a program — and the app has to take over and show them from what it kept.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node pwa.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const context = await browser.newContext({ viewport: { width: 1300, height: 900 } });
const page = await context.newPage();
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));

await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
// The worker is there and has the shell, the bundle included.
await page.waitForFunction(async () => {
  const registration = await navigator.serviceWorker.getRegistration();
  if (!registration || !registration.active) return false;
  const names = await caches.keys();
  for (const name of names.filter((name) => name.startsWith("betula-shell-"))) {
    const cache = await caches.open(name);
    if ((await cache.match("/pkg/folia_client_bg.wasm")) && (await cache.match("/assets/sql-wasm.wasm")) && (await cache.match("/"))) return true;
  }
  return false;
}, null, { timeout: 60000 }).catch(() => problems.push("the service worker did not cache the shell"));
check(!(await page.evaluate(() => document.getElementById("db-status")?.getClientRects().length > 0)), "the status pill is still shown once the app runs");

// A page seen with the network is kept; the rest comes from the shell.
await page.goto(base + "/programs/bachelor-informatik-2008/plan", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 });

await context.setOffline(true);
const offline = async (path, ready, name) => {
  try {
    await page.goto(base + path, { waitUntil: "domcontentloaded", timeout: 20000 });
  } catch (error) {
    problems.push(`offline: ${name} did not load at all (${String(error).slice(0, 120)})`);
    return;
  }
  await page.waitForFunction(ready, null, { timeout: 60000 }).catch(() => problems.push(`offline: ${name} did not show up`));
  // The kept document brings the stylesheet, once, and the page is styled by it.
  const sheets = await page.evaluate(() => [...document.querySelectorAll("link[rel=stylesheet]")].map((link) => Boolean(link.sheet?.cssRules.length)));
  check(sheets.length === 1 && sheets[0], `offline: ${name} has ${sheets.length} stylesheets (${sheets})`);
};
await offline("/catalog?turnus=winter", () => window.__betulaApp === true && document.querySelectorAll(".rows a.row").length > 5 && location.search.includes("turnus=winter"), "the filtered catalog");
await offline("/catalog/module/11101", () => window.__betulaApp === true && document.querySelector(".module-page h2"), "a module page never loaded before");
await offline("/programs/bachelor-informatik-2008/plan", () => window.__betulaApp === true && document.querySelector("table.matrix"), "the program page seen before");
// And the app keeps working across a step: a module beside the program, out of the local catalog.
await page.click('table.matrix tbody a[data-walk="module"]');
await page.waitForFunction(() => location.search.includes("open=") && document.querySelector("#preview .hero h2"), null, { timeout: 8000 }).catch(() => problems.push("offline: a module did not open beside the program"));
await context.setOffline(false);

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
