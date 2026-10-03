// Checks the data worker (docs/folia/folia-refactor.md §6.2): the page's thread holds no catalog
// and asks the worker; the app takes the page over without a blank moment; two tabs share one
// download; and, where the check may give Radix a newer snapshot (`WORKER_SNAPSHOT_DIR`, the
// directory `radix serve-snapshot` serves, with Folia polling it often: FOLIA_SNAPSHOT_POLL=2), an
// open page shows the new data in place, without a reload.
//   node worker.mjs [base-url]      (SMOKE_BROWSER_CHANNEL=msedge by default)
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
// A run that stopped before it put the pointer back leaves the server on its snapshot for a
// while: the check starts once the server has the one the pointer names.
const dir = process.env.WORKER_SNAPSHOT_DIR;
if (dir) {
  const named = JSON.parse(readFileSync(`${dir}/current.json`, "utf8")).etag;
  for (let waited = 0; (await (await fetch(base + "/api/status")).json()).snapshot.etag !== named; waited += 1000) {
    if (waited > 600000) throw new Error(`the server does not have the snapshot ${named}`);
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
}
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const timings = {};
const takeover = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });

// ---- one tab: no catalog on the page's thread, no blank moment at the takeover
const context = await browser.newContext({ viewport: { width: 1400, height: 900 } });
let downloads = 0;
context.on("request", (request) => { if (new URL(request.url()).pathname === "/api/db") downloads++; });
const page = await context.newPage();
const errors = [];
page.on("pageerror", (error) => errors.push(error.message));
const started = Date.now();
await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
// From the first frame to the takeover the page is never without rows: the server's, then the
// picture of them (boot.js), then the app's.
const blank = await page.evaluate(() => new Promise((resolve) => {
  let empty = 0;
  const look = () => {
    const rows = [...document.querySelectorAll(".rows a.row")].filter((row) => row.getBoundingClientRect().height > 0).length;
    if (!rows) empty++;
    if (window.__betulaApp === true) return resolve(empty);
    requestAnimationFrame(look);
  };
  look();
}));
timings.takeover = Date.now() - started;
check(blank === 0, `in ${blank} frames before the takeover no row of the list was to be seen`);
const page_thread = await page.evaluate(() => ({ sql: typeof window.initSqlJs, db: typeof window.betulaDb, data: typeof window.betulaData?.ask }));
check(page_thread.sql === "undefined" && page_thread.db === "undefined" && page_thread.data === "function", `the page's thread holds the catalog: ${JSON.stringify(page_thread)}`);
check(!(await page.evaluate(() => document.querySelector(".takeover-picture"))), "the picture of the server's page stayed after the takeover");
// A click is answered by the worker: a filter, then a module beside the list.
let t = Date.now();
await page.click('#filters a.chip:has-text("Winter")');
await page.waitForFunction(() => location.search.includes("turnus=winter") && document.querySelector(".rows a.row"));
timings["a filter"] = Date.now() - t;
t = Date.now();
await page.click(".rows a.row");
await page.waitForFunction(() => document.querySelector(".detail h2"));
timings["a module beside the list"] = Date.now() - t;

// ---- a second tab: the catalog the first one kept, no second download
const before = downloads;
const second = await context.newPage();
await second.goto(base + "/bookmarks", { waitUntil: "domcontentloaded" });
await takeover(second);
check(downloads === before, `the second tab downloaded the catalog again (${downloads - before} times)`);
check(downloads === 1, `the catalog was downloaded ${downloads} times`);

// ---- a newer snapshot, shown in place
if (dir) {
  const pointer = JSON.parse(readFileSync(`${dir}/current.json`, "utf8"));
  const initSqlJs = createRequire(import.meta.url)("../assets/sql-wasm.js");
  const SQL = await initSqlJs({ locateFile: (file) => fileURLToPath(new URL(`../assets/${file}`, import.meta.url)) });
  const db = new SQL.Database(readFileSync(`${dir}/${pointer.file}`));
  db.run("UPDATE meta SET value = '2031-01-02T03:04:05Z' WHERE key = 'data_changed_at'");
  const bytes = Buffer.from(db.export());
  db.close();
  const hash = createHash("sha256").update(bytes).digest("hex").slice(0, 32);
  const file = `catalog-${hash.slice(0, 16)}.db`;
  writeFileSync(`${dir}/${file}`, bytes);
  writeFileSync(`${dir}/current.json`, JSON.stringify({ file, etag: `"${hash}"`, bytes: bytes.length, exported_at: new Date().toISOString() }) + "\n");
  try {
    // Folia fetches it within its poll, after the brotli of the snapshot before is made (minutes in
    // a debug build, snapshot.rs `sync`); the worker looks when the tab comes back into view.
    for (let waited = 0; !(await (await page.request.get(base + "/api/status")).json()).snapshot.data_changed_at.startsWith("2031"); waited += 1000) {
      if (waited > 600000) throw new Error("the server did not take the newer snapshot");
      await page.waitForTimeout(1000);
    }
    const scrolled = await page.evaluate(() => { const list = document.querySelector("#catalog-scroll, .work.flowing"); list.scrollTop = 400; return list.scrollTop; });
    t = Date.now();
    await page.evaluate(() => document.dispatchEvent(new Event("visibilitychange")));
    await page.waitForFunction(() => document.querySelector(".ground")?.textContent.includes("02.01.2031"), null, { timeout: 60000 });
    timings["a newer snapshot in place"] = Date.now() - t;
    check((await page.evaluate(() => document.querySelector("#catalog-scroll, .work.flowing").scrollTop)) === scrolled, "the list did not stay where it was when the new data came");
    check(await page.evaluate(() => Boolean(document.querySelector(".detail h2") && location.search.includes("turnus=winter"))), "the page did not stay the page it was");
    // The other tab, told by the first one's worker.
    await second.waitForFunction(() => document.querySelector(".ground")?.textContent.includes("02.01.2031"), null, { timeout: 30000 }).catch(() => problems.push("the other tab did not get the newer snapshot"));
    check(downloads === 2, `the newer snapshot was downloaded ${downloads - 1} times`);
  } finally {
    writeFileSync(`${dir}/current.json`, JSON.stringify(pointer) + "\n");
  }
}
check(errors.length === 0, `errors on the page: ${errors.join(" | ")}`);
await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
