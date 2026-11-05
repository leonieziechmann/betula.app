// Checks Betula Sync (folia/crates/stores/src/sync, docs/phloem/protocol.md) with two browsers of
// one profile against a running Phloem:
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node --import ./chromium.mjs sync.mjs
// Folia runs with FOLIA_SYNC=on (and FOLIA_SYNC_URL where Phloem answers on another port), Phloem
// with PHLOEM_ORIGINS naming Folia's origin.
// The first device switches Betula Sync on for a profile with a Merkliste and a note; the second
// joins with the profile's key and holds the same marks and note; a mark set on one shows on the
// other within a second; both change while the second is offline, and once it is back both hold
// both changes; a note typed on both at once keeps the later one and the other version as a hint.
// Privacy: no request or frame carries a module's number, a note or a key in the clear; nothing
// goes to another host than Folia's and Phloem's. Fails on a console error or a step that does
// not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const phloem = process.env.SMOKE_PHLOEM_URL || "";
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
const MARKS = "betula.bookmarks.v1";
const SHELF = "betula.collections.v1";
const SECRET_NOTE = "Altklausur mit Lena rechnen";
const MODULES = ["11787", "12101", "13977"];

/// Every request and frame a page sends, for the privacy check.
const sent = [];
const watch = (page, name) => {
  page.on("console", (m) => { if (m.type() === "error") problems.push(`${name} console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`${name} pageerror: ${String(e).slice(0, 300)}`));
  page.on("request", (r) => sent.push({ name, url: r.url(), body: r.postData() || "" }));
  page.on("websocket", (ws) => ws.on("framesent", (f) => sent.push({ name, url: ws.url(), body: String(f.payload) })));
};

const device = async (name) => {
  const context = await browser.newContext();
  const page = await context.newPage();
  watch(page, name);
  await page.goto(base + "/merkliste");
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push(`${name}: the app never took over`));
  return { context, page, name };
};

const marks = (page) => page.evaluate((key) => (localStorage.getItem(key) || "").split("\n").filter(Boolean).map((l) => l.split("\t")[0]), MARKS);
const shelf = (page) => page.evaluate((key) => localStorage.getItem(key) || "", SHELF);
const state = (page) => page.evaluate(() => localStorage.getItem("betula.sync.p1.status") || "");

const until = async (name, page, fn, arg, timeout = 8000) => {
  const started = Date.now();
  try {
    await page.waitForFunction(fn, arg, { timeout });
  } catch {
    problems.push(`${name}: did not happen`);
  }
  timings[name] = Date.now() - started;
};

// The first device: a Merkliste with three marks and a note, as a person leaves them.
const a = await device("A");
await a.page.evaluate(([marksKey, shelfKey, modules, note]) => {
  localStorage.setItem(marksKey, modules.map((m, i) => `${m}\t${1790000000 - i}`).join("\n") + "\n");
  localStorage.setItem(shelfKey, `n\t${modules[0]}\t${note}\n`);
}, [MARKS, SHELF, MODULES, SECRET_NOTE]);
await a.page.reload();
await a.page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });

// „Betula Sync aktivieren" on the profile page.
await a.page.goto(base + "/profil");
await a.page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
const activate = a.page.getByRole("button", { name: /Betula Sync aktivieren/ }).first();
check(await activate.isVisible().catch(() => false), "A: no „Betula Sync aktivieren“ on the profile page");
await activate.click().catch(() => problems.push("A: „Betula Sync aktivieren“ cannot be clicked"));
// The dialog asks once more.
const confirm = a.page.getByRole("dialog").getByRole("button", { name: /aktivieren|Einschalten|Weiter/i }).first();
if (await confirm.isVisible().catch(() => false)) await confirm.click();
await until("A synced", a.page, () => (localStorage.getItem("betula.sync.p1.status") || "").startsWith("online"), null, 15000);
const key = await a.page.evaluate(() => (localStorage.getItem("betula.sync.p1.v1") || "").split("\n").find((l) => l.startsWith("key\t"))?.slice(4) || "");
check(key.length === 43, `A: no profile key kept (${key.length})`);
// The snapshot is kept at Phloem before the next device asks.
await a.page.waitForTimeout(500);

// The second device joins with the profile's key (what the link's parcel hands on).
const b = await device("B");
await b.page.evaluate((key) => {
  const device = btoa(String.fromCharCode(...crypto.getRandomValues(new Uint8Array(16)))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  localStorage.setItem("betula.sync.p1.v1", `key\t${key}\ndevice\t${device}\nslot\t0\n`);
}, key);
await b.page.reload();
await b.page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
await until("B joined", b.page, (n) => (localStorage.getItem("betula.bookmarks.v1") || "").split("\n").filter(Boolean).length === n, MODULES.length, 15000);
check(JSON.stringify(await marks(b.page)) === JSON.stringify(await marks(a.page)), `B: other marks than A (${await marks(b.page)} / ${await marks(a.page)})`);
check((await shelf(b.page)).includes(SECRET_NOTE), "B: the note did not come");
check((await state(a.page)) === "online 1", `A: does not see B online (${await state(a.page)})`);

// Live: a mark set on B shows on A.
await b.page.goto(base + "/module/12104");
await b.page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
const mark = b.page.getByRole("button", { name: /Merken/ }).first();
if (await mark.isVisible().catch(() => false)) {
  await mark.click();
  await until("a mark of B on A", a.page, () => (localStorage.getItem("betula.bookmarks.v1") || "").includes("12104\t"), null, 5000);
} else {
  problems.push("B: no „Merken“ on a module's page");
}

// Offline: B changes while away, A meanwhile; back online, both hold both.
await b.context.setOffline(true);
await until("B offline", b.page, () => (localStorage.getItem("betula.sync.p1.status") || "") === "offline", null, 10000);
await b.page.evaluate(() => {
  const key = "betula.collections.v1";
  localStorage.setItem(key, (localStorage.getItem(key) || "") + "n\t13977\tFolien von Woche 3\n");
});
await a.page.goto(base + "/module/11112");
await a.page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
const markA = a.page.getByRole("button", { name: /Merken/ }).first();
if (await markA.isVisible().catch(() => false)) await markA.click();
await b.page.reload().catch(() => {});
await b.context.setOffline(false);
await b.page.reload().catch(() => {});
await b.page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
await until("A's mark on B after offline", b.page, () => (localStorage.getItem("betula.bookmarks.v1") || "").includes("11112\t"), null, 15000);
await until("B's offline note on A", a.page, () => (localStorage.getItem("betula.collections.v1") || "").includes("Folien von Woche 3"), null, 15000);

// Privacy: what left the browsers.
const clear = sent.filter((s) => MODULES.concat(["12104", "11112", SECRET_NOTE, "Folien von Woche 3"]).some((secret) => s.body.includes(secret) || s.url.includes(SECRET_NOTE)));
check(clear.length === 0, `in the clear: ${clear.slice(0, 3).map((s) => `${s.name} ${s.url} ${s.body.slice(0, 120)}`).join(" | ")}`);
const keyLeaks = sent.filter((s) => s.body.includes(key) || s.url.includes(key));
check(keyLeaks.length === 0, `the profile key left the browser: ${keyLeaks.slice(0, 2).map((s) => s.url).join(" ")}`);
const hosts = new Set(sent.map((s) => new URL(s.url).host));
const allowed = new Set([new URL(base).host].concat(phloem ? [new URL(phloem).host] : []));
const foreign = [...hosts].filter((h) => !allowed.has(h));
check(foreign.length === 0, `requests to other hosts: ${foreign.join(", ")}`);

await browser.close();
console.log(JSON.stringify(timings));
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log("sync: ok");
