// The pass in front of the catalog (folia/crates/server/src/pass.rs) in a real browser: a first visit
// is answered 401 with a challenge at `/api/db`, the data worker solves it by itself and hands it in
// (`/api/pass`, 204 and the cookie), the catalog comes, and the app takes the page over as always;
// nobody sees any of it. A visit after that asks for nothing. A browser that comes from another
// network with the catalog to download again earns a pass of that network the same way. Also
// what a client that runs no script gets: the challenge, and not a byte of the catalog.
// Folia answers without Traefik here, so the check says where it comes from as Traefik would
// (`X-Forwarded-For`).
//   node pass.mjs [base-url]      (SMOKE_BROWSER_CHANNEL=msedge by default)
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const takeover = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });

// ---- a client without a script: a challenge, no catalog
{
  const response = await fetch(base + "/api/db", { headers: { "x-forwarded-for": "203.0.113.9" } });
  const body = await response.text();
  check(response.status === 401, `/api/db without a pass: HTTP ${response.status}`);
  check(response.headers.get("www-authenticate") === "Betula-Pass" && response.headers.get("cache-control") === "no-store", "/api/db without a pass: not a challenge nobody keeps");
  const asked = JSON.parse(body);
  check(/^c1\.\d+\.\d+\.[0-9a-f]{24}\.[0-9a-f]{64}$/.test(asked.challenge) && asked.seed.length === 24 && asked.difficulty > 0, `the challenge: ${body}`);
  check(body.length < 400, `the answer without a pass is ${body.length} bytes`);
}

// ---- a first visit: 401, the work, 204, the catalog, the app
const context = await browser.newContext({ viewport: { width: 1400, height: 900 }, extraHTTPHeaders: { "x-forwarded-for": "203.0.113.20" } });
const seen = [];
context.on("response", (response) => {
  const path = new URL(response.url()).pathname;
  if (path === "/api/db" || path === "/api/pass") seen.push(`${response.request().method()} ${path} ${response.status()}`);
});
const page = await context.newPage();
const errors = [];
page.on("pageerror", (error) => errors.push(error.message));
const started = Date.now();
await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
await takeover(page);
const took = Date.now() - started;
check(seen.join(", ") === "GET /api/db 401, POST /api/pass 204, GET /api/db 200", `a first visit: ${seen.join(", ")}`);
const pass = (await context.cookies(base + "/api/db")).find((cookie) => cookie.name === "betula_pass");
check(pass && pass.httpOnly && pass.sameSite === "Strict" && pass.path === "/api/", `the pass: ${JSON.stringify(pass)}`);
check(pass && pass.expires * 1000 - Date.now() > 6.9 * 24 * 3600 * 1000, "the pass holds for a week");
check(!(await page.evaluate(() => document.cookie)).includes("betula_pass"), "the page's scripts can read the pass");
await page.click('#filters a.chip:has-text("Winter")');
await page.waitForFunction(() => location.search.includes("turnus=winter") && document.querySelector(".rows a.row"));

// ---- coming back: the catalog is kept, nothing is asked
seen.length = 0;
await page.reload({ waitUntil: "domcontentloaded" });
await takeover(page);
check(seen.length === 0, `coming back: ${seen.join(", ")}`);

// ---- another network, the catalog to download again (thrown away here, from IndexedDB and from the
// browser's cache, which would ask with If-None-Match and hear 304, which needs no pass): a pass of
// that network
await (await context.newCDPSession(page)).send("Network.clearBrowserCache");
await page.evaluate(() => new Promise((resolve) => {
  const request = indexedDB.open("betula-catalog", 1);
  request.onsuccess = () => {
    const tx = request.result.transaction("snapshots", "readwrite");
    tx.objectStore("snapshots").delete("catalog");
    tx.oncomplete = () => resolve();
  };
}));
await context.setExtraHTTPHeaders({ "x-forwarded-for": "198.51.100.30" });
seen.length = 0;
await page.reload({ waitUntil: "domcontentloaded" });
await takeover(page);
check(seen.join(", ") === "GET /api/db 401, POST /api/pass 204, GET /api/db 200", `from another network: ${seen.join(", ")}`);

check(errors.length === 0, `page errors: ${errors.join(" | ")}`);
await browser.close();
console.log(`first visit to the takeover: ${took} ms`);
if (problems.length) {
  console.error("FAIL\n  " + problems.join("\n  "));
  process.exit(1);
}
console.log("ok: the pass");
