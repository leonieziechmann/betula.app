// Checks that the devices of a profile make its space anew when Phloem lost what it held (owner,
// 2026-11-11: „mach es im zweifelsfall so, dass es aus den clients rekonstruiert wird, falls es
// mal verloren geht"; docs/phloem/protocol.md, „When Phloem lost what it held"), against a
// Phloem the check may stop and start, so locally:
//   PHLOEM_LOSE=<stops Phloem and deletes its data directory> PHLOEM_START=<starts it again> \
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node --import ./chromium.mjs rebuild.mjs
// Folia runs with FOLIA_SYNC=on (FOLIA_SYNC_URL and FOLIA_SYNC_PARTS naming Phloem), Phloem with
// PHLOEM_ORIGINS naming Folia's origin. Never against a Phloem whose data counts. REBUILD_TRAIL=1
// prints, where something did not happen, what the devices asked Phloem and heard, in time.
// A switches Betula Sync on with three marked modules, B joins with the link, and A shares the
// whole Merkliste so that it stays current. Phloem loses its database; while it is gone, A marks
// one module and B another. Phloem comes back with a database of another epoch: A and B make the
// space anew, each in the slot it had, and both hold all five modules. A device that joins
// afterwards gets all of it, and the link of the shared list opens again.
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
if (!process.env.PHLOEM_LOSE || !process.env.PHLOEM_START) {
  console.error("rebuild: PHLOEM_LOSE and PHLOEM_START name the commands that stop Phloem with its data deleted and start it again");
  process.exit(2);
}
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const trail = [];
const t0 = Date.now();
const check = (ok, message) => { if (!ok) problems.push(message); };
const SEED = ["11787", "12101", "13977"];
const OFFLINE_A = "12104";
const OFFLINE_B = "12107";

const KEYS = () => {
  const active = () => (localStorage.getItem("betula.profiles.v1") || "").match(/^active\t(\d+)/m)?.[1] || "1";
  window.store = (key) => (active() === "1" ? key : key.replace("betula.", `betula.p${active()}.`));
  window.syncKey = (what) => `betula.sync.p${active()}.${what}`;
  window.marked = () => (localStorage.getItem(window.store("betula.bookmarks.v1")) || "").split("\n").filter(Boolean).map((l) => l.split("\t")[0]);
  window.syncWord = () => localStorage.getItem(window.syncKey("status")) || "";
  /// What the device keeps of its sync: its slot and the epoch of Phloem's database it heard.
  window.held = () => Object.fromEntries((localStorage.getItem(window.syncKey("v1")) || "").split("\n").filter(Boolean).map((l) => l.split("\t")));
};
const device = async (name) => {
  const context = await browser.newContext();
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(KEYS);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error" && !/Failed to load resource|WebSocket connection|ERR_CONNECTION_REFUSED/.test(m.text())) problems.push(`${name} console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`${name} pageerror: ${String(e).slice(0, 300)}`));
  page.on("response", (r) => { if (/:8200|\/sync\/v1\//.test(r.url())) trail.push(`${((Date.now() - t0) / 1000).toFixed(1)}s ${name} ${r.request().method()} ${new URL(r.url()).pathname} ${r.status()}`); });
  page.on("websocket", (ws) => { trail.push(`${((Date.now() - t0) / 1000).toFixed(1)}s ${name} ws open`); ws.on("framereceived", (f) => { const t = String(f.payload).slice(0, 60); if (!t.includes('"entry"')) trail.push(`${((Date.now() - t0) / 1000).toFixed(1)}s ${name} <- ${t}`); }); ws.on("close", () => trail.push(`${((Date.now() - t0) / 1000).toFixed(1)}s ${name} ws closed`)); });
  return { context, page, name };
};
const app = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push(`the app never took over (${page.url()})`));
const go = async (page, path) => {
  await page.goto(base + path);
  await app(page);
};
const until = async (name, page, fn, arg, timeout = 10000) => {
  try {
    await page.waitForFunction(fn, arg, { timeout });
    return true;
  } catch {
    problems.push(`${name}: did not happen (${await page.evaluate(() => syncWord()).catch(() => "?")})`);
    return false;
  }
};
/// Marks a module on its page, as a visitor does.
const mark = async (who, id) => {
  await go(who.page, `/catalog/module/${id}`);
  await who.page.locator(".mark-toggle[aria-pressed=false] >> visible=true").first().click().catch(() => problems.push(`${who.name}: cannot mark ${id}`));
  await until(`${who.name} marks ${id}`, who.page, (id) => marked().includes(id), id, 5000);
};
/// A link for the next device, from the profile page's dialog.
const pairingLink = async (who) => {
  await go(who.page, "/profile");
  await who.page.locator("#pf-sync-connect >> visible=true").click().catch(() => problems.push(`${who.name}: no „Gerät verbinden“`));
  await until(`${who.name}: the link for the next device`, who.page, () => Boolean(onPage(".pf-pair-link code")?.textContent.includes("/sync#")), null, 15000);
  return (await who.page.locator(".pf-pair-link code").first().textContent().catch(() => "")).trim();
};

// ---- A switches Betula Sync on, B joins ----
const a = await device("A");
await go(a.page, "/bookmarks");
await a.page.evaluate((seed) => localStorage.setItem("betula.bookmarks.v1", seed.map((m, i) => `${m}\t${1790000000 - i}`).join("\n") + "\n"), SEED);
await go(a.page, "/profile");
await a.page.locator("#pf-sync-connect >> visible=true").click().catch(() => problems.push("A: no „Gerät verbinden“ on the profile page (is FOLIA_SYNC on?)"));
await a.page.getByRole("button", { name: /^Einschalten und Code zeigen$/ }).first().click().catch(() => problems.push("A: no „Einschalten und Code zeigen“ in the dialog"));
await until("A synced", a.page, () => syncWord().startsWith("online"), null, 20000);
await until("the link", a.page, () => Boolean(onPage(".pf-pair-link code")?.textContent.includes("/sync#")), null, 10000);
const link = (await a.page.locator(".pf-pair-link code").first().textContent().catch(() => "")).trim();
if (!link) {
  console.log(`rebuild: FAILED\n- ${problems.join("\n- ")}\n- A's profile page: ${(await a.page.locator(".pf-sync").first().textContent().catch(() => "")).slice(0, 400)}`);
  process.exit(1);
}
const b = await device("B");
await b.page.goto(link);
await app(b.page);
await until("B joined", b.page, (n) => marked().length === n, SEED.length, 30000);
await until("A heard the epoch", a.page, () => /^[A-Za-z0-9_-]{22}$/.test(held().epoch || ""), null, 10000);
await until("B heard the epoch", b.page, () => /^[A-Za-z0-9_-]{22}$/.test(held().epoch || ""), null, 10000);
const before = { a: await a.page.evaluate(() => held()), b: await b.page.evaluate(() => held()) };
check(before.a.slot !== before.b.slot, `A and B share a slot: ${before.a.slot}`);

// ---- A shares the whole Merkliste so that it stays current ----
await go(a.page, "/bookmarks");
await a.page.locator(".coll-nav .coll-item", { hasText: "Alle" }).first().hover();
await a.page.locator(".coll-nav .coll-item", { hasText: "Alle" }).first().locator(".coll-more").click().catch(() => problems.push("A: no menu of „Alle“"));
await a.page.locator(".view-menu [role=menuitem]", { hasText: "Teilen …" }).click().catch(() => problems.push("A: no „Teilen …“"));
await until("A: the dialog", a.page, () => Boolean(onPage(".sh-dialog[open] .sh-opt")), null);
await a.page.locator(".sh-dialog[open] .sh-opt", { hasText: "Bleibt aktuell" }).click();
await a.page.locator(".sh-dialog[open] .sh-opt", { hasText: "Alle mit dem Link" }).click();
await a.page.click("#sh-make");
await until("A: the link of the list", a.page, () => Boolean(onPage(".sh-link")?.textContent.includes("#live=")), null, 20000);
const listLink = (await a.page.locator(".sh-link").first().textContent().catch(() => "")).trim();
await a.page.click(".sh-dialog[open] footer .mini.primary").catch(() => {});

// ---- Phloem loses everything; meanwhile each device marks a module of its own ----
trail.push(`${((Date.now() - t0) / 1000).toFixed(1)}s LOSE`);
execSync(process.env.PHLOEM_LOSE, { stdio: "inherit" });
await until("A notices", a.page, () => !syncWord().startsWith("online"), null, 20000);
await until("B notices", b.page, () => !syncWord().startsWith("online"), null, 20000);
await mark(a, OFFLINE_A);
await mark(b, OFFLINE_B);
execSync(process.env.PHLOEM_START, { stdio: "inherit" });
trail.push(`${((Date.now() - t0) / 1000).toFixed(1)}s START`);

// ---- both make the space anew and end with all of it ----
const all = [...SEED, OFFLINE_A, OFFLINE_B].sort().join();
await until("A online again", a.page, () => syncWord().startsWith("online"), null, 60000);
await until("B online again", b.page, () => syncWord().startsWith("online"), null, 60000);
await until("A holds all five", a.page, (all) => [...marked()].sort().join() === all, all, 30000);
await until("B holds all five", b.page, (all) => [...marked()].sort().join() === all, all, 30000);
const after = { a: await a.page.evaluate(() => held()), b: await b.page.evaluate(() => held()) };
check(after.a.epoch && after.a.epoch !== before.a.epoch && after.a.epoch === after.b.epoch, `the epochs: ${JSON.stringify({ before, after })}`);
check(after.a.slot === before.a.slot && after.b.slot === before.b.slot, `the slots changed: ${JSON.stringify({ before, after })}`);
check(after.a.key === before.a.key, "A's key changed");

// ---- a device that joins afterwards gets all of it ----
const next = await pairingLink(a);
const c = await device("C");
await c.page.goto(next);
await app(c.page);
await until("C joined with all five", c.page, (all) => [...marked()].sort().join() === all, all, 30000);

// ---- the shared list opens again ----
const d = await device("D");
await d.page.goto(listLink);
await app(d.page);
await until("D: the shared list opens", d.page, () => Boolean(onPage("#list-follow") || onPage("#list-take")), null, 30000);
check(!(await d.page.locator(".offer[role=alert]").count()), `D: ${await d.page.locator(".offer[role=alert]").first().textContent().catch(() => "")}`);

// ---- the profile ends its sync: the service keeps nothing of the check ----
await go(a.page, "/profile");
await a.page.getByRole("button", { name: /^Sync beenden/ }).first().click().catch(() => problems.push("A: no „Sync beenden“"));
await a.page.getByRole("button", { name: /^Beenden$/ }).first().click().catch(() => problems.push("A: no „Beenden“ to confirm"));
await until("A: private again", a.page, () => !syncWord(), null, 15000);

await browser.close();
if (problems.length) {
  if (process.env.REBUILD_TRAIL) console.log(trail.join("\n"));
  console.log(`rebuild: ${problems.length} problem(s)\n- ${problems.join("\n- ")}`);
  process.exit(1);
}
console.log("rebuild: ok");
