// The Content-Security-Policy (folia/crates/server/src/csp.rs, docs/folia/frontend.md
// „Content-Security-Policy"): every page carries it, and no page the app takes over breaks it.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node csp.mjs           (SMOKE_BROWSER_CHANNEL=msedge by default)
// Opens the start page, the catalog with a module, the program overview and a program, the
// Merkliste, the Stundenplan, „Mein Studium", the profile, Betula Sync's „Gerät verbinden" (/sync),
// the pages about Betula, the legal pages and a page in English; each until the app has taken it
// over and a moment longer. Then the start page of a browser that keeps „Mein Studiengang", twice:
// the second time the server's page puts in the day the app kept (`folia_home::home::AHEAD_SCRIPT`)
// before the app starts. With SMOKE_ACCESS_PASSWORD set the site is behind the gate of closed
// testing: its login page is checked first, then the password opens the rest.
// Fails on a page without the header, on any violation the browser reports
// (`securitypolicyviolation`), on a console error, and on a page the app does not take over.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const password = process.env.SMOKE_ACCESS_PASSWORD || "";
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const seen = [];

// Every violation, from the first script on: the event reaches the document, whoever was refused.
const WATCH = () => {
  window.__violations = [];
  document.addEventListener("securitypolicyviolation", (e) => {
    window.__violations.push(`${e.effectiveDirective} refused ${e.blockedURI || "inline"}${e.sample ? ` (${e.sample.slice(0, 60)})` : ""} at ${e.sourceFile || "-"}:${e.lineNumber}`);
  });
};

const open = async (seed = {}) => {
  const context = await browser.newContext({ timezoneId: "Europe/Berlin" });
  await context.addInitScript(WATCH);
  await context.addInitScript((seed) => {
    if (sessionStorage.getItem("seeded")) return;
    for (const [key, value] of Object.entries(seed)) localStorage.setItem(key, value);
    sessionStorage.setItem("seeded", "1");
  }, seed);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`${page.url()} console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`${page.url()} pageerror: ${String(e).slice(0, 300)}`));
  return { context, page };
};

/// Opens `path` and checks its header and what the browser refused; with `app` until the app took
/// the page over and a second longer (its workers, its first requests, the WebSocket of Betula Sync).
const visit = async (page, path, app = true) => {
  const response = await page.goto(base + path, { waitUntil: "load" });
  const policy = response?.headers()["content-security-policy"] || "";
  if (!policy.includes("default-src 'self'")) problems.push(`${path}: no Content-Security-Policy (HTTP ${response?.status()})`);
  if (app) {
    // The first page of a browser downloads the catalog (44 MB) before the app starts.
    await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push(`${path} (visit ${seen.length + 1}): the app never took over`));
    await page.waitForTimeout(1000);
  }
  const violations = await page.evaluate(() => window.__violations || []);
  for (const violation of violations) problems.push(`${path}: ${violation}`);
  seen.push(`${path} ${violations.length ? "✗" : "✓"}`);
  return page;
};

const { context, page } = await open();
if (password) {
  await visit(page, "/access", false);
  await page.fill("#password", password);
  await Promise.all([page.waitForNavigation(), page.click("button[type=submit]")]);
}
await visit(page, "/");
await visit(page, "/catalog");
const module = await page.evaluate(() => document.querySelector("a.row[data-id]")?.dataset.id || "");
if (module) await visit(page, `/catalog/module/${module}`);
else problems.push("/catalog: no module to open");
await visit(page, "/programs");
const program = await page.evaluate(() => [...document.querySelectorAll("a[href^='/programs/']")].map((a) => a.getAttribute("href")).find((href) => /^\/programs\/[^/?]+\/plan$/.test(href)) || "");
if (program) await visit(page, program);
else problems.push("/programs: no program to open");
for (const path of ["/bookmarks", "/studyplan", "/study", "/profile", "/sync", "/about/overview", "/imprint", "/privacy", "/en/catalog"]) await visit(page, path);
const cookies = await context.cookies();
await context.close();

// A browser that keeps „Mein Studiengang": its start page shows the day, and keeps it for the next.
const mine = await open({ "betula.myprogram.v1": "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n" });
await mine.context.addCookies(cookies);
await visit(mine.page, "/");
await mine.page.waitForFunction(() => Object.keys(localStorage).some((key) => key.startsWith("betula.today.v2.")), null, { timeout: 30000 }).catch(() => problems.push("/ with a program: the start page keeps no day"));
await visit(mine.page, "/");
await mine.context.close();
await browser.close();

console.log(seen.join("\n"));
if (problems.length) {
  console.error(`\n${problems.length} problems:\n${[...new Set(problems)].join("\n")}`);
  process.exit(1);
}
console.log("\ncsp: every page carries the policy and broke none of it");
