// The traps (folia/crates/server/src/trap.rs) against a running Folia: the link nobody sees stands
// on the server's page and a browser never asks for it, nor for the address the data worker's
// script names, while a person uses the app; a program that follows the link is answered 429 and
// so is everything it asks for after; somebody at the same address gets the page that says so,
// presses „Weiter zu Betula" and is where they wanted to go, the app taking over; the address in
// the script and what a scanner looks for are traps too; another site cannot spring one for its
// visitors. Folia answers without Traefik here, so the check says where it comes from as Traefik
// would (`X-Forwarded-For`); the addresses are the documentation's, and each pause is ended again.
//   node trap.mjs [base-url]      (SMOKE_BROWSER_CHANNEL=msedge by default)
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const LINK = "/api/trap/modules";
const IN_SCRIPT = "/api/trap/catalog.db";
const RELEASE = "/api/trap/release";
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const page_of = { accept: "text/html,application/xhtml+xml,*/*;q=0.8" };
const ask = async (path, address, headers = {}, init = {}) => {
  const response = await fetch(base + path, { redirect: "manual", ...init, headers: { "x-forwarded-for": address, ...headers } });
  return { status: response.status, headers: response.headers, body: await response.text() };
};
const release = (address) => ask(RELEASE, address, {}, { method: "POST", body: new URLSearchParams({ next: "/" }) });
const paused = async () => (await (await fetch(base + "/api/status")).json()).paused_addresses;

const before = await paused();
check(typeof before === "number", "the server under test has no traps (FOLIA_TRAP_MINUTES=0?)");

// ---- the bait
const home = await ask("/", "203.0.113.40", page_of);
check(home.body.includes(`<a href="${LINK}" rel="nofollow" tabindex="-1" aria-hidden="true" hidden>`), "the server's page has no link nobody sees");
const script = await ask("/pkg/data-worker.js", "203.0.113.40");
check(script.body.includes(`fetch("${IN_SCRIPT}")`), "the data worker's script does not name the address nobody asks for");

// ---- a person: the link is nowhere to see or reach, and nothing of the app asks for a trap
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const takeover = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
{
  const context = await browser.newContext({ viewport: { width: 1400, height: 900 }, extraHTTPHeaders: { "x-forwarded-for": "203.0.113.40" }, javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  const link = await page.evaluate((href) => {
    const a = document.querySelector(`a[href="${href}"]`);
    if (!a) return null;
    const box = a.getBoundingClientRect();
    return { shown: box.width > 0 || box.height > 0 || a.offsetParent !== null, hidden: a.hidden, tab: a.tabIndex };
  }, LINK);
  check(link && !link.shown && link.hidden && link.tab === -1, `the link on the server's page: ${JSON.stringify(link)}`);
  // Tab through the whole page: the focus never lands on it.
  let reached = false;
  for (let i = 0; i < 60 && !reached; i++) {
    await page.keyboard.press("Tab");
    reached = await page.evaluate((href) => document.activeElement?.getAttribute?.("href") === href, LINK);
  }
  check(!reached, "Tab reaches the link nobody sees");
  await context.close();
}
{
  const context = await browser.newContext({ viewport: { width: 1400, height: 900 }, extraHTTPHeaders: { "x-forwarded-for": "203.0.113.40" } });
  const trapped = [];
  context.on("request", (request) => { if (new URL(request.url()).pathname.startsWith("/api/trap/")) trapped.push(request.url()); });
  const page = await context.newPage();
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await takeover(page);
  check(!(await page.evaluate(() => document.querySelector('a[href^="/api/trap/"]'))), "the browser app's page carries the link");
  await page.click('#filters a.chip:has-text("Winter")');
  await page.waitForFunction(() => location.search.includes("turnus=winter") && document.querySelector(".rows a.row"));
  await page.click(".rows a.row");
  await page.waitForTimeout(500);
  await page.goto(base + "/", { waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForTimeout(1500);
  check(trapped.length === 0, `a person's browser asked for ${trapped.join(", ")}`);
  check((await ask("/catalog", "203.0.113.40", page_of)).status === 200, "a person's address was paused");
  await context.close();
}

// ---- a program that follows the link
const sprung = await ask(LINK, "203.0.113.41");
check(sprung.status === 429 && Number(sprung.headers.get("retry-after")) > 0 && sprung.headers.get("cache-control") === "no-store", `the link followed: HTTP ${sprung.status}`);
check((await paused()) === before + 1, "the address that followed the link is not paused");
for (const path of ["/catalog", "/api/status", "/api/db", "/pkg/folia_client.js"]) {
  check((await ask(path, "203.0.113.41")).status === 429, `${path} answers the paused address`);
}
check((await ask("/catalog", "203.0.113.99", page_of)).status === 200, "another address is paused with it");
check((await ask("/assets/app.css", "203.0.113.41")).status === 200 && (await ask("/robots.txt", "203.0.113.41")).status === 200, "the page of a pause is without its stylesheet, or robots.txt closed");

// ---- somebody at that address: the page says so, the button leads on
{
  const context = await browser.newContext({ viewport: { width: 1400, height: 900 }, extraHTTPHeaders: { "x-forwarded-for": "203.0.113.41" } });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => { if (message.type() === "error" && /Content Security Policy/i.test(message.text())) errors.push(message.text()); });
  const answer = await page.goto(base + "/catalog?turnus=winter", { waitUntil: "load" });
  check(answer.status() === 429, `a page at a paused address: HTTP ${answer.status()}`);
  const said = await page.evaluate(() => ({ title: document.querySelector("h1")?.textContent, text: document.querySelector(".gate-panel")?.textContent, styled: getComputedStyle(document.querySelector(".gate-panel")).borderRadius !== "0px", focus: document.activeElement?.textContent }));
  check(said.title === "Kurze Pause" && /noch etwa \d+ Minuten/.test(said.text) && said.text.includes("WLAN der Uni"), `the page of a pause says ${JSON.stringify(said)}`);
  check(said.styled && said.focus === "Weiter zu Betula", `the page of a pause: styled ${said.styled}, the focus at „${said.focus}“`);
  if (process.env.TRAP_SHOT) await page.screenshot({ path: process.env.TRAP_SHOT });
  await Promise.all([page.waitForURL((url) => url.pathname === "/catalog" && url.search === "?turnus=winter"), page.click("button[type=submit]")]);
  await takeover(page);
  check(await page.evaluate(() => document.querySelectorAll(".rows a.row").length > 0), "after the button the catalog has no rows");
  check((await paused()) === before, "the button did not end the pause");
  check(errors.length === 0, `errors on the way: ${errors.join(" | ")}`);
  await context.close();
}
await browser.close();

// ---- the other baits, and who is never paused
for (const [address, path] of [["203.0.113.42", IN_SCRIPT], ["203.0.113.43", "/wp-login.php"], ["203.0.113.44", "/.env"]]) {
  check((await ask(path, address)).status === 429 && (await ask("/", address, page_of)).status === 429, `${path} is no trap`);
  check((await release(address)).status === 303 && (await ask("/", address, page_of)).status === 200, `${address} is still paused after the button`);
}
const elsewhere = await ask(LINK, "203.0.113.45", { "sec-fetch-site": "cross-site" });
check(elsewhere.status === 404 && (await ask("/", "203.0.113.45", page_of)).status === 200, "another site paused its visitor here");
const direct = await fetch(base + LINK);
await direct.text();
check(direct.status === 404 && (await paused()) === before, "the server asked directly was paused, or a pause is left");

if (problems.length) {
  console.error("FAIL\n  " + problems.join("\n  "));
  process.exit(1);
}
console.log("ok: the traps");
