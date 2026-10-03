// Checks that the browser app never opens a local copy of the catalog of an older schema than the
// one its build reads (`folia_model::SCHEMA_VERSION`, which the server writes into boot.js). After a
// release that changed the schema, a returning visitor's copy lacks columns the new queries
// select (2026-09-23: „no such column: source_pages" on a study plan). Such a copy is planted in
// IndexedDB: the server's own, its header saying one schema less (`user_version`, four bytes at
// offset 60), which is all that tells a copy from before the last migration. Then:
//   1. without a network the app does not start on it: the page stays the one the service worker
//      kept, and the status says why, where an error page used to be;
//   2. with the network the app starts only on the server's copy, which is downloaded first, as
//      on a first visit ("Daten werden geladen …"), and kept;
//   3. a copy of the build's own schema under an older ETag still starts the app at once, and the
//      server's copy replaces it in the background for the next start, as before;
//   4. a copy the browser cannot read any more (its file gone, 2026-10-02: the app then never
//      started again) is replaced as an older one is, and the app starts on the server's copy.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node schema.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// The server must serve a snapshot of the build's schema.
import { chromium } from "playwright-core";
import { readFileSync } from "node:fs";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
// A study plan, as in the report; the synthetic snapshot has this one too.
const path = "/programs/bachelor-informatik-2008/plan";
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// What the server has, and the schema the build reads: `folia_model::SCHEMA_VERSION` of this checkout,
// which the server writes into boot.js (minified there, under a name of the minifier's).
const server = (await (await fetch(base + "/api/status")).json()).snapshot;
const schema = Number(/pub const SCHEMA_VERSION: i64 = (\d+);/.exec(readFileSync(new URL("../catalog/src/db.rs", import.meta.url), "utf8"))?.[1]);
if (!(schema > 0) || !(server?.schema_version >= schema)) {
  console.log(JSON.stringify({ problems: [`boot.js reads schema ${schema}, the server's snapshot is of schema ${server?.schema_version}: this check needs a server whose snapshot the app can open`] }, null, 2));
  process.exit(1);
}

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
try {
  const context = await browser.newContext({ viewport: { width: 1300, height: 900 } });
  // In every document, before its scripts: each text the status pill shows (boot.js empties it
  // once the app runs), and the copy of the catalog as boot.js keeps it in IndexedDB.
  await context.addInitScript(() => {
    window.__statuses = [];
    new MutationObserver(() => {
      const text = document.getElementById("db-status")?.textContent;
      if (text && window.__statuses.at(-1) !== text) window.__statuses.push(text);
    }).observe(document, { childList: true, subtree: true, characterData: true });
    const store = (mode, act) => new Promise((resolve, reject) => {
      const open = indexedDB.open("betula-catalog", 1);
      open.onupgradeneeded = () => open.result.createObjectStore("snapshots");
      open.onerror = () => reject(open.error);
      open.onsuccess = () => {
        const tx = open.result.transaction("snapshots", mode);
        const request = act(tx.objectStore("snapshots"));
        tx.oncomplete = () => { open.result.close(); resolve(request.result); };
        tx.onerror = () => reject(tx.error);
      };
    });
    // The copy (a Blob under `catalog`, boot.js): its ETag and the schema its header names.
    window.__copy = async () => {
      const copy = await store("readonly", (snapshots) => snapshots.get("catalog"));
      return copy ? { etag: copy.etag, schema: new DataView(await copy.blob.slice(0, 100).arrayBuffer()).getInt32(60) } : null;
    };
    // The copy there is, kept again under `etag` with `schema` in its header.
    window.__plant = async ([etag, schema]) => {
      const { blob } = await store("readonly", (snapshots) => snapshots.get("catalog"));
      const head = await blob.slice(0, 100).arrayBuffer();
      new DataView(head).setInt32(60, schema);
      await store("readwrite", (snapshots) => snapshots.put({ etag, blob: new Blob([head, blob.slice(100)]) }, "catalog"));
    };
    // A copy that cannot be read: what IndexedDB hands back once the file of its Blob is gone
    // (NotFoundError) is beyond a check to bring about, so here the record has no Blob.
    window.__break = (etag) => store("readwrite", (snapshots) => snapshots.put({ etag, blob: null }, "catalog"));
  });
  const page = await context.newPage();
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  // Playwright's waitForFunction takes the promise of an async function for a truthy answer, so a
  // check that has to wait for IndexedDB or the worker is polled here.
  const until = async (test, arg, ms) => {
    for (const end = Date.now() + ms; Date.now() < end; await pause(250)) {
      if (await page.evaluate(test, arg).catch(() => false)) return true;
    }
    return false;
  };
  const started = (ms) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: ms }).then(() => true, () => false);
  const statuses = () => page.evaluate(() => window.__statuses);
  const loading = (texts) => texts.some((text) => text.startsWith("Daten werden geladen"));
  const errorPage = () => page.evaluate(() => Boolean(document.querySelector(".state-error, .fatal")));

  // A first visit keeps the server's copy; once the worker is in charge, the page is seen again
  // and kept for a visit without a network.
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  check(await started(120000), "first visit: the browser app never took over");
  const installed = await until(async () => {
    const registration = await navigator.serviceWorker.getRegistration();
    return Boolean(registration && registration.active && navigator.serviceWorker.controller);
  }, null, 120000);
  check(installed, "first visit: the service worker did not take over");
  await page.reload({ waitUntil: "domcontentloaded" });
  check(await started(60000), "second visit: the app did not take over");
  const first = await page.evaluate(() => window.__copy());
  check(first?.etag === server.etag && first?.schema === server.schema_version, `first visit: kept ${JSON.stringify(first)}, the server has ${server.etag} of schema ${server.schema_version}`);

  // 1. An older copy without a network: no app, no error page; the kept page and a word why.
  await page.evaluate((args) => window.__plant(args), ['"before-the-release"', schema - 1]);
  await context.setOffline(true);
  await page.reload({ waitUntil: "domcontentloaded" }).catch((error) => problems.push("offline: the page did not load (" + String(error).slice(0, 120) + ")"));
  const said = await until(() => {
    const pill = document.getElementById("db-status");
    return Boolean(pill && !pill.hidden && pill.textContent.startsWith("Offline"));
  }, null, 30000);
  check(said, `offline, an older copy: the status does not say why the app does not start (${JSON.stringify(await statuses())})`);
  check(await page.evaluate(() => window.__betulaApp !== true), "offline, an older copy: the app started on it");
  check(!(await errorPage()), "offline, an older copy: an error page");
  check(await page.evaluate((path) => location.pathname === path && Boolean(document.querySelector("table.ptable")), path), "offline, an older copy: not the plan the worker kept");
  await context.setOffline(false);

  // 2. The same copy with the network: replaced before the app starts.
  await page.reload({ waitUntil: "domcontentloaded" });
  check(await started(120000), "online, an older copy: the app did not start");
  const opened = await page.evaluate(() => window.betulaDb?.etag);
  check(opened === server.etag, `online, an older copy: the app opened ${opened}, not the server's copy ${server.etag}`);
  const replacing = await statuses();
  check(loading(replacing), `online, an older copy: the status never said that the data is loading (${JSON.stringify(replacing)})`);
  const replaced = await page.evaluate(() => window.__copy());
  check(replaced?.etag === server.etag && replaced?.schema === server.schema_version, `online, an older copy: kept ${JSON.stringify(replaced)}`);
  const drawn = await page.waitForFunction(() => document.querySelector('#plan table.ptable tbody a[data-walk="module"]'), null, { timeout: 30000 }).then(() => true, () => false);
  check(drawn && !(await errorPage()), "online, an older copy: the app does not show the plan");

  // 3. A copy of the build's schema under an older ETag: the app starts on it at once, and the
  // server's copy follows in the background.
  await page.evaluate((args) => window.__plant(args), ['"an-older-export"', schema]);
  await page.reload({ waitUntil: "domcontentloaded" });
  check(await started(60000), "the build's schema, an older ETag: the app did not start");
  check((await page.evaluate(() => window.betulaDb?.etag)) === '"an-older-export"', "the build's schema, an older ETag: the app did not start on the copy it had");
  check(!loading(await statuses()), "the build's schema, an older ETag: the app waited for a download");
  const followed = await until((etag) => window.__copy().then((copy) => copy?.etag === etag), server.etag, 60000);
  check(followed, "the build's schema, an older ETag: the server's copy did not replace it in the background");

  // 4. A copy that cannot be read: replaced before the app starts.
  await page.evaluate((etag) => window.__break(etag), '"unreadable"');
  await page.reload({ waitUntil: "domcontentloaded" });
  check(await started(120000), "an unreadable copy: the app did not start");
  check((await page.evaluate(() => window.betulaDb?.etag)) === server.etag, "an unreadable copy: the app did not start on the server's copy");
  const mended = await page.evaluate(() => window.__copy());
  check(mended?.etag === server.etag && mended?.schema === server.schema_version, `an unreadable copy: kept ${JSON.stringify(mended)}`);
} catch (error) {
  problems.push(String(error).slice(0, 300));
} finally {
  await browser.close();
}
console.log(JSON.stringify({ schema, server: { etag: server.etag, schema: server.schema_version }, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
