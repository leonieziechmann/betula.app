// Checks the search of the catalog (docs/frontend.md „The search of the catalog“), on the server's
// page without JavaScript and in the browser app:
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node search.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// A typo is corrected and the head of the list says so; what the other filters leave out is said
// under the list, one click away; the matches come by relevance, „Modul“ orders them by title, and
// typing orders them by relevance again. Fails on a page load after takeover, a console error, or a
// step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];

// The server's page, without JavaScript: the same list, the same lines.
const plain = await browser.newPage({ javaScriptEnabled: false });
await plain.goto(base + "/catalog?q=algoritmen");
const corrected = (await plain.locator(".search-line").textContent().catch(() => "")) || "";
// The word a typo was taken for, written as it was typed (lower case here).
if (!corrected.includes("algoritmen") || !corrected.includes("algorithmen")) problems.push(`server: the corrected typo is not said („${corrected}“)`);
if (!(await plain.locator(".rows a.row").count())) problems.push("server: no module for the corrected typo");
await plain.goto(base + "/catalog?q=python&program=bachelor-informatik-2008");
const outside = (await plain.locator(".list-note").textContent().catch(() => "")) || "";
if (!outside.includes("außerhalb deiner Filter")) problems.push(`server: nothing is said of the matches outside the filters („${outside}“)`);
await plain.close();

const page = await browser.newPage({ viewport: { width: 1500, height: 900 } });
page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
const timings = {};
const step = async (name, action, until, arg = null) => {
  const started = Date.now();
  await action();
  try {
    await page.waitForFunction(until, arg, { timeout: 8000 });
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
  }
  timings[name] = Date.now() - started;
  if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
};

await page.goto(base + "/catalog?program=bachelor-informatik-2008", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
await page.evaluate(() => { window.__marker = 1; });

await step("search inside a program", () => page.fill("#topsearch", "python"),
  () => location.search.includes("q=python") && /Filter/.test(document.querySelector(".list-note")?.textContent || ""));
await step("the matches outside the filters", () => page.click(".list-note a"),
  () => location.search.includes("q=python") && !location.search.includes("program=") && document.querySelectorAll(".rows a.row").length > 0);
await step("a typo is corrected", () => page.fill("#topsearch", "algoritmen"),
  () => (document.querySelector(".search-line")?.textContent || "").includes("algorithmen") && document.querySelectorAll(".rows a.row").length > 0);
await step("the best match first", () => page.fill("#topsearch", "informatik"),
  () => !document.querySelector(".search-line") && /(^|\s)Informatik(\s|$)/.test(document.querySelector(".rows a.row b")?.textContent || ""));
await step("„Modul“ orders by title", () => page.click('.cols a:has-text("Modul")'), () => location.search.includes("sort=title"));
const titles = await page.evaluate(() => [...document.querySelectorAll(".rows a.row b")].slice(0, 12).map((b) => b.textContent.toLowerCase()));
if (titles.some((title, i) => i > 0 && titles[i - 1] > title)) problems.push(`„Modul“: not by title: ${titles.join(" | ")}`);
await step("typing orders by relevance again", () => page.fill("#topsearch", "informatik 1"),
  () => location.search.includes("q=informatik") && !location.search.includes("sort=") && document.querySelectorAll(".rows a.row").length > 0);

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
