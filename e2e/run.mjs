// Playwright runner for the smoke walk. Usage:
//
//   cd e2e && npm install
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node run.mjs            (downloaded Chromium: npx playwright-core install chromium)
//   SMOKE_BROWSER_CHANNEL=msedge node run.mjs                     (an installed Edge or Chrome, no download)
//
// Options: SMOKE_MAX_PROGRAMS (quick local run), SMOKE_HEADED=1, SMOKE_OPTIONS (JSON merged into
// the walk's options, e.g. selectors). Exits 1 on any console error, page error, failed request,
// 5xx response, program page that did not render, or full page load during the walk.
import { chromium } from "playwright-core";
import { fileURLToPath } from "node:url";
import path from "node:path";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const options = JSON.parse(process.env.SMOKE_OPTIONS || "{}");
if (process.env.SMOKE_MAX_PROGRAMS) options.maxPrograms = Number(process.env.SMOKE_MAX_PROGRAMS);
const overview = options.overviewUrl || "/studiengaenge";

const browser = await chromium.launch({
  channel: process.env.SMOKE_BROWSER_CHANNEL || undefined,
  headless: !process.env.SMOKE_HEADED,
});
const page = await browser.newPage();

const seen = [];
page.on("console", (message) => {
  if (message.type() === "error") seen.push({ kind: "console.error", url: page.url(), text: message.text().slice(0, 600) });
});
page.on("pageerror", (error) => seen.push({ kind: "pageerror", url: page.url(), text: String(error).slice(0, 600) }));
page.on("requestfailed", (request) => {
  // Navigating away aborts in-flight requests; that is not a failure of the app.
  if (request.failure()?.errorText !== "net::ERR_ABORTED") {
    seen.push({ kind: "requestfailed", url: request.url(), text: request.failure()?.errorText || "" });
  }
});
page.on("response", (response) => {
  if (response.status() >= 500) seen.push({ kind: "http " + response.status(), url: response.url(), text: "" });
});

await page.addInitScript({ path: path.join(path.dirname(fileURLToPath(import.meta.url)), "smoke-walk.js") });
await page.goto(base + overview, { waitUntil: "networkidle" });
const report = await page.evaluate((walkOptions) => window.__smokeWalk(walkOptions), options);
await browser.close();

// The in-page trap and the runner see the same console errors; report each once.
const inPage = new Set(report.errors.map((e) => e.text));
const errors = [...report.errors, ...seen.filter((e) => !inPage.has(e.text))];
const ok = report.ok && errors.length === 0;

console.log(JSON.stringify({ ...report, ok, errors }, null, 2));
if (!ok) {
  console.error(`smoke walk FAILED: ${errors.length} error(s), ${report.notRendered.length} page(s) not rendered`);
  process.exit(1);
}
console.log(`smoke walk ok: ${report.visited} programs, ${report.tabs} tabs, ${report.chips} chips, ${report.variants} variants, ${report.jumps} fast jumps in ${report.seconds}s`);
