// Checks Betula Sync where no WebSocket comes about (folia/crates/stores/src/sync/browser/poll.rs,
// docs/phloem/protocol.md §4 „The socket over HTTPS", Phloem 0.4.0; owner, 2026-11-10: „wenn eine
// methode nicht funktioniert muss dann halt ausgewichen werden auf eine andere die grantiert
// läuft"), with two browsers of one profile against a running Folia and Phloem:
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node --import ./chromium.mjs sync-https.mjs
// Twice: once the browser refuses every WebSocket at once (as a policy or an extension does), once
// its WebSocket never opens (a proxy that takes the upgrade and then holds it). Each time A
// switches Betula Sync on („Gerät verbinden", „Einschalten und Code zeigen") and shows the link,
// B opens it and holds A's marks, a mark set on either shows on the other, A's profile page lists
// both devices, and every frame went as a request to /sync/v1/poll. „Sync beenden" at the end.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
const SEED = ["11787", "12101", "13977"];
const KEYS = () => {
  const active = () => (localStorage.getItem("betula.profiles.v1") || "").match(/^active\t(\d+)/m)?.[1] || "1";
  window.store = (key) => (active() === "1" ? key : key.replace("betula.", `betula.p${active()}.`));
  window.syncKey = (what) => `betula.sync.p${active()}.${what}`;
  window.marked = () => (localStorage.getItem(window.store("betula.bookmarks.v1")) || "").split("\n").filter(Boolean).map((l) => l.split("\t")[0]);
};
// No WebSocket: refused at once, or one that never opens.
const BLOCK = {
  refused: () => {
    window.WebSocket = function WebSocket() { throw new DOMException("blocked", "SecurityError"); };
  },
  silent: () => {
    window.WebSocket = class extends EventTarget {
      constructor(url) { super(); this.url = url; this.readyState = 0; }
      send() { throw new DOMException("not open", "InvalidStateError"); }
      close() { if (this.readyState === 3) return; this.readyState = 3; const e = new CloseEvent("close", { code: 1006, wasClean: false }); this.onclose?.(e); this.dispatchEvent(e); }
    };
  },
};

const device = async (name, mode) => {
  const context = await browser.newContext();
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(KEYS);
  await context.addInitScript(BLOCK[mode]);
  const page = await context.newPage();
  const seen = { poll: 0, sockets: 0 };
  page.on("console", (m) => { if (m.type() === "error" && !/Failed to load resource|WebSocket/.test(m.text())) problems.push(`${mode} ${name} console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`${mode} ${name} pageerror: ${String(e).slice(0, 300)}`));
  page.on("request", (r) => { if (new URL(r.url()).pathname === "/sync/v1/poll") seen.poll++; });
  page.on("websocket", () => { seen.sockets++; });
  return { context, page, name, seen };
};
const app = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push(`the app never took over (${page.url()})`));
const go = async (page, path) => {
  await page.goto(base + path);
  await app(page);
};
const until = async (name, page, fn, arg, timeout = 10000) => {
  const started = Date.now();
  try {
    await page.waitForFunction(fn, arg, { timeout });
  } catch {
    problems.push(`${name}: did not happen (${await page.evaluate(() => localStorage.getItem(syncKey("status"))).catch(() => "?")})`);
  }
  timings[name] = Date.now() - started;
};
const markOne = async (page, name, avoid = []) => {
  await go(page, "/catalog");
  const id = await page.evaluate((avoid) => {
    const rows = [...document.querySelectorAll(".row-wrap")].filter((w) => w.querySelector(":scope > .mark-toggle[aria-pressed=false]") && w.getBoundingClientRect().height > 0 && !avoid.includes(w.querySelector("a.row")?.dataset.id));
    return rows[0]?.querySelector("a.row")?.dataset.id || "";
  }, avoid);
  if (!id) {
    problems.push(`${name}: no module to mark on the catalog`);
    return "";
  }
  await page.click(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle >> visible=true`);
  return id;
};

for (const mode of ["refused", "silent"]) {
  const a = await device("A", mode);
  await go(a.page, "/bookmarks");
  await a.page.evaluate((seed) => localStorage.setItem("betula.bookmarks.v1", seed.map((m, i) => `${m}\t${1790000000 - i}`).join("\n") + "\n"), SEED);
  await go(a.page, "/profile");
  await a.page.locator("#pf-sync-connect >> visible=true").click().catch(() => problems.push(`${mode} A: no „Gerät verbinden“`));
  await a.page.getByRole("button", { name: /^(Einschalten und Code zeigen|Turn on and show the code)$/ }).first().click().catch(() => problems.push(`${mode} A: no „Einschalten und Code zeigen“`));
  // The link comes once the profile is at Phloem: after the WebSocket failed twice, over HTTPS.
  await until(`${mode}: A's link`, a.page, () => Boolean(onPage(".pf-pair-link code")?.textContent.includes("/sync#")), null, 45000);
  const link = (await a.page.locator(".pf-pair-link code").first().textContent().catch(() => "")).trim();
  check(/^online/.test(await a.page.evaluate(() => localStorage.getItem(syncKey("status")) || "")), `${mode}: A shows the link while not online`);
  const b = await device("B", mode);
  if (link) {
    await b.page.goto(link);
    await app(b.page);
    await until(`${mode}: B holds A's marks`, b.page, (n) => marked().length === n, SEED.length, 45000);
    await until(`${mode}: B online`, b.page, () => /^online/.test(localStorage.getItem(syncKey("status")) || ""), null, 20000);
    const fromB = await markOne(b.page, `${mode} B`);
    if (fromB) await until(`${mode}: B's mark on A`, a.page, (id) => marked().includes(id), fromB, 20000);
    const fromA = await markOne(a.page, `${mode} A`, [fromB]);
    if (fromA) await until(`${mode}: A's mark on B`, b.page, (id) => marked().includes(id), fromA, 20000);
    await go(a.page, "/profile");
    await until(`${mode}: A lists both devices`, a.page, () => allOnPage(".pf-device").length === 2, null, 15000);
  }
  for (const d of [a, b]) {
    check(d.seen.poll > 0, `${mode} ${d.name}: no request to /sync/v1/poll`);
    check(d.seen.sockets === 0 || mode === "silent", `${mode} ${d.name}: a WebSocket opened although the browser refuses them`);
  }
  // „Sync beenden": the space goes; both are private again.
  await go(a.page, "/profile");
  await a.page.getByRole("button", { name: /^(Sync beenden|End Sync)/ }).first().click().catch(() => problems.push(`${mode} A: no „Sync beenden“`));
  await a.page.getByRole("button", { name: /^(Beenden|End)$/ }).first().click().catch(() => problems.push(`${mode} A: no „Beenden“ to confirm`));
  await until(`${mode}: A private again`, a.page, () => !localStorage.getItem(syncKey("v1")), null, 15000);
  await a.context.close();
  await b.context.close();
}

await browser.close();
if (problems.length) {
  console.error("sync-https: FAILED\n" + problems.map((p) => "  - " + p).join("\n"));
  process.exit(1);
}
console.log("sync-https: ok " + JSON.stringify(timings));
