// Checks Betula Sync (folia/crates/stores/src/sync, folia/crates/profile/src/sync.rs,
// docs/phloem/protocol.md) with three browsers of one profile against a running Phloem:
//   SMOKE_BASE_URL=http://127.0.0.1:8080 SMOKE_PHLOEM_URL=http://127.0.0.1:8200 node --import ./chromium.mjs sync.mjs
// Folia runs with FOLIA_SYNC=on (and FOLIA_SYNC_URL where Phloem answers on another port), Phloem
// with PHLOEM_ORIGINS naming Folia's origin.
// The start page's setup ends with Betula Sync where the site offers it: a fourth chip „Geräte"
// on the server's page and the app's, „Betula Sync aktivieren" after „Studium anlegen", the QR code
// and the link at once, a second browser that opens it holding the study, „Verbunden" and
// „Fertig", and the note after the setup saying the profile stays alike on its devices.
// On a phone the setup's last step names the address a laptop opens, /pair, which shows a QR code
// and four digits; the phone opens the code's link in a tab of its own (as its camera does), shows
// the same digits, „Ja, … verbinden", and the laptop holds the phone's study while the step says
// „Verbunden"; a private profile switches Betula Sync on on the way, and a code opens once.
// The first browser switches Betula Sync on for a profile with marks and a note („Gerät verbinden",
// „Einschalten und Code zeigen" in its dialog), and the link for the next device opens by itself;
// the second opens that link and holds the same marks and note in a new profile; a mark set on one
// shows on the other within seconds; both mark while the second is offline, and once it is back
// both hold both; the recovery code brings the profile into a third browser; a new computer shows
// a code that a phone opens (as the camera app does) and sends the profile to, the digits alike on
// both; „Sync beenden" makes every browser private again with its copy; the scanner of /sync (a
// camera that films a code) sends a private profile, switched on for it. Privacy: no request or
// frame carries a module's number, the note or the profile key in the clear, nothing reaches
// another host than Folia's and Phloem's, and no link's or code's part after # reaches a server. Fails on a console error or a step that does not show up.
import { fileURLToPath } from "node:url";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const phloem = process.env.SMOKE_PHLOEM_URL || "";
// The live way (WebRTC between the devices, live.mjs checks it) stays off here: Playwright's
// offline mode does not reach WebRTC, and this check is about the way through Phloem (a browser
// that is offline here must be offline for its changes too).
const browser = await chromium.launch({
  channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge",
  headless: !process.env.SMOKE_HEADED,
  args: ["--force-webrtc-ip-handling-policy=disable_non_proxied_udp"],
});
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
const device = async (name, from = browser, options = {}) => {
  const context = await from.newContext(options);
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

// ---- the first browser: marks and a note, then „Gerät verbinden" ----
const a = await device("A");
await go(a.page, "/bookmarks");
await a.page.evaluate(([seed, note]) => {
  localStorage.setItem("betula.bookmarks.v1", seed.map((m, i) => `${m}\t${1790000000 - i}`).join("\n") + "\n");
  localStorage.setItem("betula.collections.v1", `n\t${seed[0]}\t${note}\n`);
}, [SEED, NOTE]);
await go(a.page, "/profile");
const activate = a.page.locator("#pf-sync-connect >> visible=true");
check((await activate.textContent().catch(() => "")).trim() === "Gerät verbinden", "A: no „Gerät verbinden“ on the profile page (is FOLIA_SYNC on?)");
await activate.click().catch(() => {});
await a.page.getByRole("button", { name: /^Einschalten und Code zeigen$/ }).first().click().catch(() => problems.push("A: no „Einschalten und Code zeigen“ in the dialog"));
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
await until("A sees B online", a.page, () => /^online 1( |$)/.test(localStorage.getItem("betula.sync.p1.status") || ""), null, 10000);

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
await a.page.click("#pf-sync-code >> visible=true").catch(() => problems.push("A: no „Wiederherstellungscode“"));
await until("the recovery code", a.page, () => /^[0-9A-Z]{4}(-[0-9A-Z]{4}){12}$/.test(onPage(".pf-code code")?.textContent.trim() || ""), null, 5000);
const code = (await a.page.locator(".pf-code code").first().textContent().catch(() => "")).trim();
const c = await device("C");
await go(c.page, "/profile");
await c.page.click("#pf-sync-code-way >> visible=true").catch(() => problems.push("C: no „Wiederherstellungscode eingeben“ on a new device"));
await c.page.getByLabel(/Wiederherstellungscode/).first().fill(code).catch(() => problems.push("C: no field for the recovery code"));
await c.page.getByRole("button", { name: /Profil holen/ }).first().click().catch(() => problems.push("C: no „Profil holen“"));
await until("C recovered", c.page, (n) => marked().length >= n, SEED.length, 20000);
check(JSON.stringify(await marksOf(c.page)) === JSON.stringify(await marksOf(a.page)), `C: other marks than A (${await marksOf(c.page)})`);

// ---- „Profil vom Handy holen": the computer shows a code, the phone sends a profile to it ----
// (folia/design/profil/konzepte.html, the second step.) F, a new computer, asks; B, which holds A's
// synced profile, opens the code's link as the camera app does, sees the same digits and sends it.
const asking = async (who) => {
  await go(who.page, "/profile");
  await who.page.click("#pf-sync-get >> visible=true").catch(() => problems.push(`${who.name}: no „Profil vom Handy holen“ on a new device`));
  await until(`${who.name}: the code to scan`, who.page, () => Boolean(onPage(".pf-get")?.dataset.link?.includes("/pair#")) && allOnPage(".pf-get .pf-digits span").length === 4, null, 10000);
  return who.page.evaluate(() => ({ link: onPage(".pf-get").dataset.link, digits: [...allOnPage(".pf-get .pf-digits span")].map((d) => d.textContent).join("") }));
};
const giving = (page) => page.evaluate(() => ({ title: onPage(".pf-give h2")?.textContent ?? null, digits: [...allOnPage(".pf-give .pf-digits span")].map((d) => d.textContent).join(""), chosen: onPage(".pf-give-item.on b")?.textContent ?? null }));
const f = await device("F");
const asked = await asking(f);
check(new URL(asked.link).origin === base && /^\d{4}$/.test(asked.digits), `F: the code ${JSON.stringify(asked)}`);
await b.page.goto(asked.link);
await app(b.page);
await until("B: the question for F's code", b.page, () => allOnPage(".pf-give .pf-digits span").length === 4, null, 10000);
check(!b.page.url().includes("#"), `B: the code is still in the address (${b.page.url()})`);
const question = await giving(b.page);
check(question.title === "Ein Rechner möchte ein Profil" && question.digits === asked.digits, `B: the question ${JSON.stringify(question)} for the digits ${asked.digits}`);
await b.page.click("#pf-give-yes >> visible=true").catch(() => problems.push("B: no „Ja, … verbinden“"));
await until("F holds A's profile", f.page, (n) => marked().length >= n, SEED.length, 30000);
await until("F says the profile came", f.page, () => Boolean(onPage(".pf-connect-body .pf-joined")), null, 5000);
check(JSON.stringify(await marksOf(f.page)) === JSON.stringify(await marksOf(b.page)), `F: other marks than B (${await marksOf(f.page)} / ${await marksOf(b.page)})`);
check((await f.page.evaluate(() => (localStorage.getItem("betula.profiles.v1") || "").split("\n").filter((l) => l.startsWith("profile\t")).length)) <= 1, "F: the profile came beside the empty one instead of into it");
await until("B: „Verbunden“", b.page, () => Boolean(onPage(".pf-give .pf-joined")), null, 15000);


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

// ---- on a phone: the laptop opens /pair, and the phone scans its code ----
const pairCodes = [];
{
  const PHONE = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };
  const g = await device("G", browser, PHONE);
  await go(g.page, "/");
  await g.page.click("#ob-program >> visible=true").catch(() => problems.push("G: no picker of the program"));
  await g.page.waitForSelector("#ob-program-search >> visible=true").catch(() => {});
  await g.page.keyboard.type("Informatik");
  await until("G: Informatik in the picker", g.page, () => /^Informatik/.test(onPage("#ob-program-option-0")?.textContent.trim() ?? ""), null, 10000);
  await g.page.keyboard.press("Enter");
  await until("G: step 2", g.page, () => onPage(".ob")?.dataset.step === "2" && onPage(".ob-sems") && !onPage(".ob-sems").classList.contains("wait"), null, 5000);
  await g.page.click(".ob-next >> visible=true").catch(() => {});
  await until("G: step 3", g.page, () => onPage(".ob")?.dataset.step === "3" && allOnPage(".ob-mod").length > 1, null, 5000);
  await g.page.click(".ob-create >> visible=true").catch(() => problems.push("G: no „Studium anlegen“"));
  await until("G: step 4 names the laptop", g.page, () => onPage(".ob")?.dataset.step === "4" && onPage(".ob-title")?.textContent === "Auch am Laptop?", null, 5000);
  await g.page.click(".ob-activate >> visible=true").catch(() => problems.push("G: no „Betula Sync aktivieren“"));
  await until("G: the address for the laptop", g.page, () => (onPage(".ob-address code")?.textContent ?? "").endsWith("/pair") && !onPage(".ob .pf-qr"), null, 15000);
  const h = await device("H");
  await go(h.page, "/pair");
  await until("H: the code for the phone", h.page, () => (onPage(".pf-asking .pf-get")?.dataset.link ?? "").includes("/pair#") && allOnPage(".pf-asking .pf-digits span").length === 4, null, 10000);
  const code = await h.page.evaluate(() => onPage(".pf-asking .pf-get")?.dataset.link ?? "");
  const digits = await h.page.evaluate(() => [...allOnPage(".pf-asking .pf-digits span")].map((d) => d.textContent).join(""));
  pairCodes.push(code);
  check(!h.page.url().includes("#"), `H: the address carries a key (${h.page.url()})`);
  const scanned = await g.context.newPage();
  await scanned.goto(code);
  await app(scanned);
  await until("G: the question of the code", scanned, (digits) => onPage(".pf-give h2")?.textContent === "Ein Rechner möchte ein Profil" && [...allOnPage(".pf-give .pf-digits span")].map((d) => d.textContent).join("") === digits, digits, 10000);
  check(!scanned.url().includes("#"), `G: the code's key stays in the address (${scanned.url()})`);
  await scanned.click("#pf-give-yes >> visible=true").catch(() => problems.push("G: no „Ja, … verbinden“"));
  await until("G: given", scanned, () => /Verbunden: Der Rechner hat/.test(onPage(".pf-give .pf-joined")?.textContent ?? ""), null, 20000);
  await until("H holds G's study", h.page, () => /^program\t079-82-2008$/m.test(localStorage.getItem("betula.myprogram.v1") || ""), null, 20000);
  await until("H says it is here", h.page, () => /ist jetzt auch auf diesem Gerät/.test(onPage(".pf-pair-page")?.textContent ?? ""), null, 5000);
  const plans = await Promise.all([g.page, h.page].map((page) => page.evaluate(() => (localStorage.getItem("betula.studyplan.v1") || "").split("\n").filter((line) => /^[mp]\t/.test(line)).sort().join("\n"))));
  check(plans[0] && plans[0] === plans[1], `H: another plan than G's (${plans[1].split("\n").length} / ${plans[0].split("\n").length} lines)`);
  await until("G: „Verbunden“ in step 4", g.page, () => onPage(".ob-title")?.textContent === "Verbunden", null, 15000);

  // A private profile (A's, ended above) switches Betula Sync on on the way.
  const f = await device("F");
  await go(f.page, "/pair");
  await until("F: the code", f.page, () => (onPage(".pf-asking .pf-get")?.dataset.link ?? "").includes("/pair#"), null, 10000);
  const second = await f.page.evaluate(() => onPage(".pf-asking .pf-get")?.dataset.link ?? "");
  pairCodes.push(second);
  const fromA = await a.context.newPage();
  await fromA.goto(second);
  await app(fromA);
  await until("A: the question says the profile is switched on for it", fromA, () => Boolean(onPage(".pf-give-item.on small")?.textContent.includes("eingeschaltet")), null, 10000);
  await fromA.click("#pf-give-yes >> visible=true").catch(() => problems.push("A: no „Ja, … verbinden“"));
  await until("F holds A's marks", f.page, (n) => marked().length >= n, SEED.length, 25000);
  // The code is spent: opened again, it opens nothing.
  const again = await g.context.newPage();
  await again.goto(second);
  await app(again);
  await again.click("#pf-give-yes >> visible=true").catch(() => problems.push("G: no „Ja, … verbinden“ for a code spent"));
  await until("a code opens once", again, () => /öffnet nichts/.test(onPage(".pf-pair-page .pf-refused")?.textContent ?? ""), null, 10000);
  if (problems.some((p) => p.startsWith("a code opens once"))) problems.push(`a code spent says: ${await again.evaluate(() => onPage(".pf-pair-page")?.textContent ?? "")}`);
}

// ---- the scanner of /sync ----
// A camera that films F2's code: H holds a private profile with marks,
// which is switched on for it and goes to F2.
const f2 = await device("F2");
const asked2 = await asking(f2);
const frame = await f2.page.evaluate(async () => {
  const svg = onPage(".pf-get .pf-qr");
  const [w, h] = [640, 480];
  const image = new Image();
  image.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(new XMLSerializer().serializeToString(svg).replace("<svg", '<svg xmlns="http://www.w3.org/2000/svg" width="300" height="300"'));
  await image.decode();
  const canvas = Object.assign(document.createElement("canvas"), { width: w, height: h });
  const context = canvas.getContext("2d");
  context.fillStyle = "#d8d8d8";
  context.fillRect(0, 0, w, h);
  context.fillStyle = "#ffffff";
  context.fillRect(150, 70, 340, 340);
  context.drawImage(image, 170, 90, 300, 300);
  const rgba = context.getImageData(0, 0, w, h).data;
  const luma = [];
  for (let i = 0; i < rgba.length; i += 4) luma.push(Math.round(rgba[i] * 0.299 + rgba[i + 1] * 0.587 + rgba[i + 2] * 0.114));
  return luma;
});
const folder = await mkdtemp(join(tmpdir(), "betula-scan-"));
const video = join(folder, "code.y4m");
const plane = Buffer.from(frame);
const chroma = Buffer.alloc((640 * 480) / 4, 128);
await writeFile(video, Buffer.concat([Buffer.from("YUV4MPEG2 W640 H480 F10:1 Ip A1:1 C420jpeg\n"), ...Array.from({ length: 10 }, () => Buffer.concat([Buffer.from("FRAME\n"), plane, chroma, chroma]))]));
const filming = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED, args: ["--use-fake-ui-for-media-stream", "--use-fake-device-for-media-stream", `--use-file-for-fake-video-capture=${video}`] });
const h = await device("H", filming, { permissions: ["camera"] });
await go(h.page, "/catalog");
await h.page.evaluate((seed) => localStorage.setItem("betula.bookmarks.v1", seed.map((m, i) => `${m}\t${1790000000 - i}`).join("\n") + "\n"), SEED.slice(0, 2));
await go(h.page, "/sync");
await until("H: the scanner reads F2's code", h.page, () => allOnPage(".pf-give .pf-digits span").length === 4, null, 20000);
const question2 = await giving(h.page);
check(question2.digits === asked2.digits && question2.chosen === "Profil 1" && Boolean(await h.page.evaluate(() => onPage(".pf-give-item small")?.textContent.includes("eingeschaltet"))), `H: the question ${JSON.stringify(question2)} for the digits ${asked2.digits}`);
check(await h.page.evaluate(() => onPage(".pf-cam") === null), "H: the camera is still on after the code was read");
await h.page.click("#pf-give-yes >> visible=true").catch(() => problems.push("H: no „Ja, … verbinden“"));
await until("H switched on", h.page, () => (localStorage.getItem("betula.sync.p1.status") || "").startsWith("online"), null, 15000);
await until("F2 holds H's profile", f2.page, (n) => marked().length === n, 2, 30000);
check(JSON.stringify(await marksOf(f2.page)) === JSON.stringify(await marksOf(h.page)), `F2: other marks than H (${await marksOf(f2.page)} / ${await marksOf(h.page)})`);
await filming.close();

// ---- privacy ----
const secrets = SEED.concat([live, awayA, awayB, NOTE, "Fassung von A", "Fassung von B"].filter(Boolean));
const clear = sent.filter((s) => (phloem && s.url.startsWith(phloem.replace(/^http/, "ws"))) || s.url.includes("/sync/v1/")).filter((s) => secrets.some((secret) => s.body.includes(secret) || s.url.includes(secret)));
check(clear.length === 0, `to Phloem in the clear: ${clear.slice(0, 3).map((s) => `${s.name} ${s.url} ${s.body.slice(0, 120)}`).join(" | ")}`);
const keyLeaks = sent.filter((s) => s.body.includes(key) || s.url.includes(key) || (code && s.body.includes(code)));
check(keyLeaks.length === 0, `the profile key left a browser: ${keyLeaks.slice(0, 2).map((s) => s.url).join(" ")}`);
const codes = [link, ...pairCodes, asked.link, asked2.link].map((l) => l.split("#")[1] || "");
for (const fragment of codes) {
  check(!sent.some((s) => fragment && (s.url.includes(fragment) || s.body.includes(fragment))), "a link's part after # reached a server");
}
// A code's public key (after its id) goes nowhere either: only the id reaches Phloem.
const requestKeys = codes.slice(1).map((part) => part.slice(22));
check(!sent.some((s) => requestKeys.some((key) => key && (s.url.includes(key) || s.body.includes(key)))), "a code's part after # (its public key) reached a server");
const allowed = new Set([new URL(base).host].concat(phloem ? [new URL(phloem).host] : []));
const foreign = [...new Set(sent.map((s) => new URL(s.url).host))].filter((h) => !allowed.has(h));
check(foreign.length === 0, `requests to other hosts: ${foreign.join(", ")}`);

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
