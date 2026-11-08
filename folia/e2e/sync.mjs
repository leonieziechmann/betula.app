// Checks Betula Sync (folia/crates/stores/src/sync, folia/crates/profile/src/sync.rs,
// docs/phloem/protocol.md) with three browsers of one profile against a running Phloem:
//   SMOKE_BASE_URL=http://127.0.0.1:8080 SMOKE_PHLOEM_URL=http://127.0.0.1:8200 node --import ./chromium.mjs sync.mjs
// Folia runs with FOLIA_SYNC=on (and FOLIA_SYNC_URL where Phloem answers on another port), Phloem
// with PHLOEM_ORIGINS naming Folia's origin.
// The start page's setup ends with Betula Sync where the site offers it: a fourth chip „Geräte"
// on the server's page and the app's, „Betula Sync aktivieren" after „Studium anlegen", the QR code
// and the link at once, a second browser that opens it holding the study, „Verbunden" and
// „Fertig", and the note after the setup saying the profile stays alike on its devices.
// The first browser switches Betula Sync on for a profile with marks and a note („Betula Sync
// aktivieren", „Aktivieren"), and the link for the next device opens by itself; the second opens
// that link and holds the same marks and note in a new profile; a mark set on one shows on the
// other within seconds; both mark while the second is offline, and once it is back both hold both;
// the recovery code brings the profile into a third browser; „Sync beenden" makes every browser
// private again with its copy. Privacy: no request or frame carries a module's number, the note
// or the profile key in the clear, nothing reaches another host than Folia's and Phloem's, and the
// link's part after # reaches no server. Fails on a console error or a step that does not show up.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const phloem = process.env.SMOKE_PHLOEM_URL || "";
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
const NOTE = "Altklausur mit Lena rechnen";
const SEED = ["11787", "12101", "13977"];

/// Every request and frame a browser sends, for the privacy check.
const sent = [];
// The keys of the active profile, in the page: `store("betula.bookmarks.v1")`, `syncKey("status")`.
const KEYS = () => {
  const active = () => (localStorage.getItem("betula.profiles.v1") || "").match(/^active\t(\d+)/m)?.[1] || "1";
  window.store = (key) => (active() === "1" ? key : key.replace("betula.", `betula.p${active()}.`));
  window.syncKey = (what) => `betula.sync.p${active()}.${what}`;
  window.marked = () => (localStorage.getItem(window.store("betula.bookmarks.v1")) || "").split("\n").filter(Boolean).map((l) => l.split("\t")[0]);
};
const device = async (name) => {
  const context = await browser.newContext();
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(KEYS);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error" && !/Failed to load resource|ERR_INTERNET_DISCONNECTED|WebSocket connection/.test(m.text())) problems.push(`${name} console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`${name} pageerror: ${String(e).slice(0, 300)}`));
  page.on("request", (r) => sent.push({ name, url: r.url(), body: r.postData() || "" }));
  page.on("websocket", (ws) => ws.on("framesent", (f) => sent.push({ name, url: ws.url(), body: String(f.payload) })));
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
    problems.push(`${name}: did not happen`);
  }
  timings[name] = Date.now() - started;
};
const marksOf = (page) => page.evaluate(() => marked().sort());
const shelfOf = (page) => page.evaluate(() => localStorage.getItem(store("betula.collections.v1")) || "");

/// Marks the first module of the catalog this browser has not marked; its number.
const markOne = async (page, name, stay = false, avoid = []) => {
  if (!stay) await go(page, "/catalog");
  const id = await page.evaluate((avoid) => {
    const rows = [...document.querySelectorAll(".row-wrap")].filter((w) => w.querySelector(":scope > .mark-toggle[aria-pressed=false]") && w.getBoundingClientRect().height > 0 && !avoid.includes(w.querySelector("a.row")?.dataset.id));
    return rows[0]?.querySelector("a.row")?.dataset.id || "";
  }, avoid);
  if (!id) {
    problems.push(`${name}: no module to mark on the catalog`);
    return "";
  }
  await page.click(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle >> visible=true`);
  await until(`${name} marks ${id}`, page, (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`)?.getAttribute("aria-pressed") === "true", id, 5000);
  return id;
};

// ---- the first browser: marks and a note, then „Betula Sync aktivieren" ----
const a = await device("A");
await go(a.page, "/bookmarks");
await a.page.evaluate(([seed, note]) => {
  localStorage.setItem("betula.bookmarks.v1", seed.map((m, i) => `${m}\t${1790000000 - i}`).join("\n") + "\n");
  localStorage.setItem("betula.collections.v1", `n\t${seed[0]}\t${note}\n`);
}, [SEED, NOTE]);
await go(a.page, "/profile");
const activate = a.page.getByRole("button", { name: /Betula Sync aktivieren/ }).first();
check(await activate.isVisible().catch(() => false), "A: no „Betula Sync aktivieren“ on the profile page (is FOLIA_SYNC on?)");
await activate.click().catch(() => {});
await a.page.getByRole("button", { name: /^Aktivieren$/ }).first().click().catch(() => problems.push("A: no „Aktivieren“ in the explanation"));
await until("A synced", a.page, () => (localStorage.getItem("betula.sync.p1.status") || "").startsWith("online"), null, 15000);
await until("the link for the next device", a.page, () => Boolean(onPage(".pf-pair-link code")?.textContent.includes("/sync#")), null, 10000);
const link = (await a.page.locator(".pf-pair-link code").first().textContent().catch(() => "")).trim();
const key = await a.page.evaluate(() => (localStorage.getItem("betula.sync.p1.v1") || "").split("\n").find((l) => l.startsWith("key\t"))?.slice(4) || "");
check(key.length === 43, `A: no profile key kept (${key.length})`);
check(link.startsWith(base + "/sync#"), `A: the link is ${link}`);

// ---- the second browser opens the link ----
const b = await device("B");
await b.page.goto(link);
await app(b.page);
await until("B joined", b.page, (n) => marked().length === n, SEED.length, 20000);
check(!b.page.url().includes("#"), `B: the link's key is still in the address (${b.page.url()})`);
check(JSON.stringify(await marksOf(b.page)) === JSON.stringify(await marksOf(a.page)), `B: other marks than A (${await marksOf(b.page)} / ${await marksOf(a.page)})`);
check((await shelfOf(b.page)).includes(NOTE), "B: the note did not come");
await until("A sees B online", a.page, () => (localStorage.getItem("betula.sync.p1.status") || "") === "online 1", null, 10000);

// ---- live: a mark set on B shows on A ----
const live = await markOne(b.page, "B");
if (live) await until("B's mark on A", a.page, (id) => (localStorage.getItem("betula.bookmarks.v1") || "").includes(`${id}\t`), live, 5000);

// ---- offline: both mark while B is away; back online, both hold both ----
await go(b.page, "/catalog");
await b.context.setOffline(true);
await until("B offline", b.page, () => (localStorage.getItem(syncKey("status")) || "") === "offline", null, 60000);
const awayB = await markOne(b.page, "B offline", true);
const awayA = await markOne(a.page, "A while B is away", false, [awayB]);
check(awayA !== awayB, "A and B marked the same module");
await b.context.setOffline(false);
if (awayA) await until("A's mark on B after offline", b.page, (id) => marked().includes(id), awayA, 40000);
if (awayB) await until("B's offline mark on A", a.page, (id) => (localStorage.getItem("betula.bookmarks.v1") || "").includes(`${id}\t`), awayB, 40000);
check(JSON.stringify(await marksOf(b.page)) === JSON.stringify(await marksOf(a.page)), `after offline: A ${await marksOf(a.page)}, B ${await marksOf(b.page)}`);

// ---- the recovery code brings it into a third browser ----
await go(a.page, "/profile");
await a.page.getByRole("button", { name: /Wiederherstellungscode zeigen/ }).first().click().catch(() => problems.push("A: no „Wiederherstellungscode zeigen“"));
await until("the recovery code", a.page, () => /^[0-9A-Z]{4}(-[0-9A-Z]{4}){12}$/.test(onPage(".pf-code code")?.textContent.trim() || ""), null, 5000);
const code = (await a.page.locator(".pf-code code").first().textContent().catch(() => "")).trim();
const c = await device("C");
await go(c.page, "/profile");
await c.page.getByLabel(/Wiederherstellungscode/).first().fill(code).catch(() => problems.push("C: no field for the recovery code"));
await c.page.getByRole("button", { name: /Profil holen/ }).first().click().catch(() => problems.push("C: no „Profil holen“"));
await until("C recovered", c.page, (n) => marked().length >= n, SEED.length, 20000);
check(JSON.stringify(await marksOf(c.page)) === JSON.stringify(await marksOf(a.page)), `C: other marks than A (${await marksOf(c.page)})`);

// ---- a note typed on both at once: the later stands, the other is a hint on both ----
const typeNote = async (page, text) => {
  await page.click(`a.row[data-id="${SEED[0]}"] >> visible=true`);
  await page.click('.detail .hero [data-action="collect"] >> visible=true');
  await page.click(".collect-pop .collect-note");
  await page.keyboard.press("Control+A");
  await page.keyboard.type(text, { delay: 20 });
  await page.keyboard.press("Escape");
};
await go(a.page, "/bookmarks");
await go(b.page, "/bookmarks");
await b.context.setOffline(true);
await until("B offline again", b.page, () => (localStorage.getItem(syncKey("status")) || "") === "offline", null, 10000);
await typeNote(b.page, "Fassung von B");
await typeNote(a.page, "Fassung von A");
await b.context.setOffline(false);
await until("A's note on B", b.page, () => (localStorage.getItem(store("betula.collections.v1")) || "").includes("Fassung von A"), null, 40000);
await until("B's note came to A, and lost", a.page, () => (localStorage.getItem("betula.sync.p1.doc") || "").includes("conflicts\t") && !(localStorage.getItem("betula.collections.v1") || "").includes("Fassung von B"), null, 10000);
await go(a.page, "/profile");
await until("A's profile names the hint", a.page, () => Boolean(onPage(".pf-sync-conflicts")), null, 8000);
await b.page.click(`a.row[data-id="${SEED[0]}"] >> visible=true`).catch(() => {});
await b.page.click('.detail .hero [data-action="collect"] >> visible=true').catch(() => {});
await until("B's hint shows its own version", b.page, () => (onPage(".sync-hint-other")?.textContent || "").includes("Fassung von B"), null, 8000);
await b.page.keyboard.press("Escape").catch(() => {});

// ---- „Sync beenden": every browser private again, with its copy ----
await a.page.getByRole("button", { name: /^Sync beenden/ }).first().click().catch(() => problems.push("A: no „Sync beenden“"));
await a.page.getByRole("button", { name: /^Beenden$/ }).first().click().catch(() => problems.push("A: no „Beenden“ to confirm"));
await until("A private", a.page, () => !localStorage.getItem("betula.sync.p1.v1"), null, 10000);
await until("B ended", b.page, () => Object.keys(localStorage).some((k) => /^betula\.sync\.p\d+\.ended$/.test(k)), null, 10000);
check((await marksOf(b.page)).length >= SEED.length, "B: the copy is gone after the end");

// ---- the setup's last step: the study on the phone as well ----
{
  const html = await (await fetch(base + "/")).text();
  const serverChips = [...html.matchAll(/class="ob-chip(?: [^"]*)?"[^>]*>.*?<span>([^<]*)(?:<!>)?<\/span>/g)].map((m) => m[1]);
  check(serverChips.join() === "Studiengang,Beginn,Semester,Geräte", `the server's setup does not name four steps: ${serverChips.join()}`);
  const d = await device("D");
  await go(d.page, "/");
  const chips = () => d.page.evaluate(() => [...allOnPage(".ob-chip")].map((chip) => chip.textContent.replace(/^\d/, "").trim()).join());
  check((await chips()) === "Studiengang,Beginn,Semester,Geräte", `D: the setup does not name four steps: ${await chips()}`);
  await d.page.click("#ob-program >> visible=true").catch(() => problems.push("D: no picker of the program"));
  await d.page.waitForSelector("#ob-program-search >> visible=true").catch(() => {});
  await d.page.keyboard.type("Informatik");
  await until("D: Informatik in the picker", d.page, () => /^Informatik/.test(onPage("#ob-program-option-0")?.textContent.trim() ?? ""), null, 10000);
  await d.page.keyboard.press("Enter");
  await until("D: step 2", d.page, () => onPage(".ob")?.dataset.step === "2" && onPage(".ob-sems") && !onPage(".ob-sems").classList.contains("wait"), null, 5000);
  await d.page.click(".ob-next >> visible=true").catch(() => {});
  await until("D: step 3", d.page, () => onPage(".ob")?.dataset.step === "3" && allOnPage(".ob-mod").length > 1, null, 5000);
  await d.page.click(".ob-create >> visible=true").catch(() => problems.push("D: no „Studium anlegen“"));
  await until("D: step 4 after „Studium anlegen“", d.page, () => onPage(".ob")?.dataset.step === "4" && onPage(".ob-title")?.textContent === "Auch auf dem Handy?", null, 5000);
  const four = await d.page.evaluate(() => ({
    kept: /^program\t079-82-2008$/m.test(localStorage.getItem("betula.myprogram.v1") || ""),
    welcome: Boolean(onPage(".day-welcome")),
    done: [...allOnPage(".ob-chip.done > span:last-child")].map((chip) => chip.textContent.trim()),
    points: allOnPage(".ob-points li").length,
    later: Boolean(onPage(".ob-later")),
  }));
  check(four.kept && !four.welcome && four.done.length === 3 && /^\d+ Module$/.test(four.done[2]) && four.points === 3 && four.later, `D: step 4 does not follow the study set up: ${JSON.stringify(four)}`);
  await d.page.getByRole("button", { name: /Betula Sync aktivieren/ }).first().click().catch(() => problems.push("D: no „Betula Sync aktivieren“ in step 4"));
  await until("D synced from the setup", d.page, () => (localStorage.getItem("betula.sync.p1.status") || "").startsWith("online"), null, 15000);
  await until("D: the link for the phone in step 4", d.page, () => Boolean(onPage(".ob .pf-pair-link code")?.textContent.includes("/sync#")) && Boolean(onPage(".ob .pf-qr")) && onPage(".ob-title")?.textContent === "Jetzt dein Handy verbinden", null, 10000);
  const phoneLink = (await d.page.locator(".ob .pf-pair-link code").first().textContent().catch(() => "")).trim();
  const e = await device("E");
  await e.page.goto(phoneLink);
  await app(e.page);
  await until("E holds D's study", e.page, () => /^program\t079-82-2008$/m.test(localStorage.getItem("betula.myprogram.v1") || ""), null, 20000);
  const plans = await Promise.all([d.page, e.page].map((page) => page.evaluate(() => (localStorage.getItem("betula.studyplan.v1") || "").split("\n").filter((line) => /^[mp]\t/.test(line)).sort().join("\n"))));
  check(plans[0] && plans[0] === plans[1], `E: another plan than D's (${plans[1].split("\n").length} / ${plans[0].split("\n").length} lines)`);
  await until("D: „Verbunden“ in step 4", d.page, () => onPage(".ob-title")?.textContent === "Verbunden" && Boolean(onPage(".ob-note.ok")), null, 10000);
  await d.page.click(".ob-finish >> visible=true").catch(() => problems.push("D: no „Fertig“"));
  await until("D: the day with its note after „Fertig“", d.page, () => !onPage(".ob") && /Betula Sync hält es auf deinen Geräten gleich\./.test(onPage(".day-welcome")?.textContent ?? ""), null, 8000);
}

// ---- privacy ----
const secrets = SEED.concat([live, awayA, awayB, NOTE, "Fassung von A", "Fassung von B"].filter(Boolean));
const clear = sent.filter((s) => (phloem && s.url.startsWith(phloem.replace(/^http/, "ws"))) || s.url.includes("/sync/v1/")).filter((s) => secrets.some((secret) => s.body.includes(secret) || s.url.includes(secret)));
check(clear.length === 0, `to Phloem in the clear: ${clear.slice(0, 3).map((s) => `${s.name} ${s.url} ${s.body.slice(0, 120)}`).join(" | ")}`);
const keyLeaks = sent.filter((s) => s.body.includes(key) || s.url.includes(key) || (code && s.body.includes(code)));
check(keyLeaks.length === 0, `the profile key left a browser: ${keyLeaks.slice(0, 2).map((s) => s.url).join(" ")}`);
const fragment = link.split("#")[1] || "";
check(!sent.some((s) => fragment && (s.url.includes(fragment) || s.body.includes(fragment))), "the link's part after # reached a server");
const allowed = new Set([new URL(base).host].concat(phloem ? [new URL(phloem).host] : []));
const foreign = [...new Set(sent.map((s) => new URL(s.url).host))].filter((h) => !allowed.has(h));
check(foreign.length === 0, `requests to other hosts: ${foreign.join(", ")}`);

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
