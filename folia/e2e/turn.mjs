// The browsers of deploy/test/turn.sh (run that, not this): A and B, each in a home behind a NAT
// that gives every destination a port of its own, so that no direct way between them exists.
// They are started in their network namespaces by the wrappers BT_BROWSER_A and BT_BROWSER_B;
// Folia is SMOKE_BASE_URL (127.0.0.1:8080 of each home, forwarded through its NAT to Folia).
// A switches Betula Sync on, B joins with the link. The first attempt (STUN alone) fails, the
// next one with TURN opens: both say „über Betula weitergeleitet". With Phloem's entries to B
// held back, a mark of A still arrives, through the relay alone.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
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
  window.syncWord = () => localStorage.getItem(window.syncKey("status")) || "";
};
const launch = (path) => chromium.launch({ executablePath: path, headless: true });
const device = async (name, path) => {
  const browser = await launch(path);
  const context = await browser.newContext();
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(KEYS);
  const page = await context.newPage();
  page.on("pageerror", (e) => problems.push(`${name} pageerror: ${String(e).slice(0, 300)}`));
  page.on("console", (m) => { if (m.type() === "error") problems.push(`${name} console: ${m.text().slice(0, 300)}`); });
  return { browser, context, page, name };
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

const a = await device("A", process.env.BT_BROWSER_A);
await go(a.page, "/bookmarks");
await a.page.evaluate((seed) => localStorage.setItem("betula.bookmarks.v1", seed.map((m, i) => `${m}\t${1790000000 - i}`).join("\n") + "\n"), SEED);
await go(a.page, "/profile");
await a.page.getByRole("button", { name: /Betula Sync aktivieren/ }).first().click().catch(() => problems.push("A: no „Betula Sync aktivieren“"));
await a.page.getByRole("button", { name: /^Aktivieren$/ }).first().click().catch(() => problems.push("A: no „Aktivieren“"));
await until("A synced", a.page, () => syncWord().startsWith("online"), null, 20000);
await until("the link", a.page, () => Boolean(onPage(".pf-pair-link code")?.textContent.includes("/sync#")), null, 10000);
const link = (await a.page.locator(".pf-pair-link code").first().textContent().catch(() => "")).trim();
if (!link) {
  console.error("turn: FAILED\n" + problems.map((p) => "  - " + p).join("\n") + `\n  - A's profile page: ${(await a.page.locator(".pf-sync").first().textContent().catch(() => "")).slice(0, 400)}`);
  process.exit(1);
}

const b = await device("B", process.env.BT_BROWSER_B);
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
await until("B joined", b.page, (n) => marked().length === n, SEED.length, 30000);

// No direct way: the first attempt (STUN alone) fails within 8 s, the one with TURN opens.
await until("A relayed", a.page, () => / relayed 1 /.test(syncWord() + " "), null, 60000);
await until("B relayed", b.page, () => / relayed 1 /.test(syncWord() + " "), null, 10000);
check(!/ direct [1-9]/.test(await a.page.evaluate(() => syncWord())), "A found a direct way through the NATs: the check proves nothing about TURN then");
await until("A's profile page says so", a.page, () => /über Betula weitergeleitet/.test(onPage(".pf-sync-state")?.textContent || ""), null, 5000);

/// Marks a module A has not marked, on the catalog A shows; its number.
const markOnA = async () => {
  const id = await a.page.evaluate(() => {
    const rows = [...document.querySelectorAll(".row-wrap")].filter((w) => w.querySelector(":scope > .mark-toggle[aria-pressed=false]") && w.getBoundingClientRect().height > 0);
    return rows[0]?.querySelector("a.row")?.dataset.id || "";
  });
  check(id, "A: no module to mark on the catalog");
  if (id) await a.page.click(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle >> visible=true`);
  return id;
};

// Within the app (no page load: the connection stays), a mark of A reaches B through the relay
// alone, with Phloem's entries to B held back.
await a.page.click('.rail a[data-area="catalog"] >> visible=true').catch(() => problems.push("A: no way to the catalog in the rail"));
await until("A on the catalog", a.page, () => document.querySelectorAll(".row-wrap .mark-toggle").length > 3, null, 10000);
holding = true;
const first = await markOnA();
if (first) await until("A's mark on B, through the relay", b.page, (id) => marked().includes(id), first, 5000);
check(held > 0, "B: Phloem's entry was not held back (the check proves nothing then)");

// A's page loads again: it remembers that B needs TURN and takes it at once, without the 8 s of
// a direct attempt first.
await go(a.page, "/catalog");
await until("A relayed again, at once", a.page, () => / relayed 1 /.test(syncWord() + " "), null, 7500);
const second = await markOnA();
if (second) await until("A's second mark on B, through the relay", b.page, (id) => marked().includes(id), second, 5000);
holding = false;

await a.browser.close();
await b.browser.close();
console.log(JSON.stringify({ timings, held }));
if (problems.length) {
  console.error("turn: FAILED\n" + problems.map((p) => "  - " + p).join("\n"));
  process.exit(1);
}
console.log("turn: ok");
