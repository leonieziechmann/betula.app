// Checks the live way of Betula Sync (step 2: folia/crates/stores/src/sync/live.rs and
// browser/rtc.rs, docs/phloem/protocol.md section 4) with two browsers of one profile against a
// running Phloem:
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node --import ./chromium.mjs live.mjs
// Folia runs with FOLIA_SYNC=on (FOLIA_SYNC_URL where Phloem answers on another port), Phloem with
// PHLOEM_ORIGINS naming Folia's origin; TURN is not needed: both browsers are in one network.
// Chromium hides the addresses of its candidates behind mDNS names, which two browsers in one
// container cannot resolve, so the check launches it with WebRtcHideLocalIpsWithMdns off.
// A switches Betula Sync on („Gerät verbinden“) and B joins with the link; both say „direkt verbunden" within seconds.
// Then the entries Phloem hands to B are held back: a mark A sets still shows on B, over the
// direct connection alone, and A's profile page says so. Privacy: the signals that set the
// connection up (offers, answers, candidates with the devices' addresses) reach Phloem sealed,
// never in the clear. When A goes, B's word drops back to Phloem alone.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({
  channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge",
  headless: !process.env.SMOKE_HEADED,
  args: ["--disable-features=WebRtcHideLocalIpsWithMdns"],
});
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
const SEED = ["11787", "12101", "13977"];

/// Every frame a browser sends, for the privacy check.
const sent = [];
const KEYS = () => {
  const active = () => (localStorage.getItem("betula.profiles.v1") || "").match(/^active\t(\d+)/m)?.[1] || "1";
  window.store = (key) => (active() === "1" ? key : key.replace("betula.", `betula.p${active()}.`));
  window.syncKey = (what) => `betula.sync.p${active()}.${what}`;
  window.marked = () => (localStorage.getItem(window.store("betula.bookmarks.v1")) || "").split("\n").filter(Boolean).map((l) => l.split("\t")[0]);
  window.syncWord = () => localStorage.getItem(window.syncKey("status")) || "";
};
const device = async (name) => {
  const context = await browser.newContext();
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(KEYS);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error" && !/Failed to load resource|WebSocket connection/.test(m.text())) problems.push(`${name} console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`${name} pageerror: ${String(e).slice(0, 300)}`));
  page.on("websocket", (ws) => ws.on("framesent", (f) => sent.push({ name, body: String(f.payload) })));
  return { context, page, name };
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
    problems.push(`${name}: did not happen (${await page.evaluate(() => syncWord()).catch(() => "?")})`);
  }
  timings[name] = Date.now() - started;
};

// ---- A switches Betula Sync on ----
const a = await device("A");
await go(a.page, "/bookmarks");
await a.page.evaluate((seed) => localStorage.setItem("betula.bookmarks.v1", seed.map((m, i) => `${m}\t${1790000000 - i}`).join("\n") + "\n"), SEED);
await go(a.page, "/profile");
await a.page.locator("#pf-sync-connect >> visible=true").click().catch(() => problems.push("A: no „Gerät verbinden“ on the profile page (is FOLIA_SYNC on?)"));
await a.page.getByRole("button", { name: /^Einschalten und Code zeigen$/ }).first().click().catch(() => problems.push("A: no „Einschalten und Code zeigen“ in the dialog"));
await until("A synced", a.page, () => syncWord().startsWith("online"), null, 15000);
await until("the link for the next device", a.page, () => Boolean(onPage(".pf-pair-link code")?.textContent.includes("/sync#")), null, 10000);
const link = (await a.page.locator(".pf-pair-link code").first().textContent().catch(() => "")).trim();

// ---- B joins; Phloem's entries to B can be held back from here on ----
const b = await device("B");
let holding = false;
let held = 0;
await b.page.routeWebSocket(/\/sync\/v1\/ws/, (ws) => {
  const server = ws.connectToServer();
  server.onMessage((m) => {
    if (holding && String(m).startsWith('{"t":"entry"')) {
      held += 1;
      return;
    }
    ws.send(m);
  });
  ws.onMessage((m) => server.send(m));
});
await b.page.goto(link);
await app(b.page);
await until("B joined", b.page, (n) => marked().length === n, SEED.length, 20000);

// ---- both connect directly ----
await until("A connected directly", a.page, () => / direct 1 /.test(syncWord() + " "), null, 30000);
await until("B connected directly", b.page, () => / direct 1 /.test(syncWord() + " "), null, 30000);
await until("A's profile page says so", a.page, () => /direkt verbunden/.test(onPage(".pf-sync-state")?.textContent || ""), null, 5000);

// ---- a mark of A reaches B over the direct connection alone ----
holding = true;
await go(a.page, "/catalog");
const id = await a.page.evaluate((seed) => {
  const rows = [...document.querySelectorAll(".row-wrap")].filter((w) => w.querySelector(":scope > .mark-toggle[aria-pressed=false]") && w.getBoundingClientRect().height > 0 && !seed.includes(w.querySelector("a.row")?.dataset.id));
  return rows[0]?.querySelector("a.row")?.dataset.id || "";
}, SEED);
check(id, "A: no module to mark on the catalog");
if (id) {
  await a.page.click(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle >> visible=true`);
  await until("A's mark on B, directly", b.page, (id) => marked().includes(id), id, 5000);
  check(held > 0, "B: Phloem's entry was not held back (the check proves nothing then)");
}
holding = false;

// ---- privacy: the signals reach Phloem sealed ----
const signals = sent.filter((s) => s.body.startsWith('{"t":"signal"'));
check(signals.length > 0, "no signal went through Phloem");
for (const s of signals) {
  const clear = ["v=0", "a=fingerprint", "ice-ufrag", "candidate:", "typ host", "127.0.0.1", "sdp"].find((w) => s.body.includes(w));
  if (clear) problems.push(`${s.name}: a signal carries ${clear} in the clear`);
}

// ---- A goes: B is on Phloem alone ----
await a.context.close();
await until("B drops back to Phloem", b.page, () => / direct 0 /.test(syncWord() + " "), null, 30000);

await browser.close();
console.log(JSON.stringify({ timings, held, signals: signals.length }));
if (problems.length) {
  console.error("live: FAILED\n" + problems.map((p) => "  - " + p).join("\n"));
  process.exit(1);
}
console.log("live: ok");
