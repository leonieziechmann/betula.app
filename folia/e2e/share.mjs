// Checks sharing a calendar (Betula Sync, step 3: folia/design/teilen/teilen.html,
// folia/crates/planner/src/studyplan/share_dialog.rs, docs/phloem/protocol.md) with four browsers
// against a running Phloem:
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node --import ./chromium.mjs share.mjs
// Folia runs with FOLIA_SYNC=on, FOLIA_SYNC_URL and FOLIA_SYNC_PARTS naming Phloem, Phloem with
// PHLOEM_ORIGINS naming Folia's origin. Against canary, SMOKE_COOKIES names a curl cookie jar that
// holds the cookie of closed testing (never the password); the profiles it syncs end their sync
// at the end („Sync beenden"), so the service keeps nothing of the check.
// Max (private) opens „Teilen …", takes „Bleibt aktuell" and „Eine Person" (each marked „mit
// Sync"), takes his Termine of his own along and names it „Lena"; „Profil speichern und Link
// erstellen" saves his profile with Betula Sync and makes a one-time link. Lena (private) opens it
// and follows it as a calendar, with Max's modules and his Termin „Arbeit"; a third browser opening
// the same link reads that it was opened already and gets nothing of the plan. Max plans another
// module: Lena's calendar has it after her page comes into view again. Lena saves her profile
// („Profil speichern"); Max plans one more, and Lena's open page has it without a reload (Phloem's
// `part`). Max stops sharing: Lena's calendar stays, „nicht mehr geteilt", and „Behalten" keeps it.
// A calendar app's address that stays current answers its calendar until Max stops sharing it, and
// a 404 after. Privacy: no request or frame carries a module's number, the Termin's name or the
// part's key in the clear, and a link's part after # reaches no server.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const SEMESTER = "2026W";
const MODULES = ["12104", "12107"];
const MORE = ["12102", "11112"];
const PLAN = MODULES.map((m) => `m\t${SEMESTER}\t${m}\t1790000000\t\n`).join("") + `t\t1\t0\t${SEMESTER}\tw\t5\t13:00-17:00\t\t\t\tArbeit\tCafé Mosaik\t\n`;

const sent = [];
const KEYS = () => {
  const active = () => (localStorage.getItem("betula.profiles.v1") || "").match(/^active\t(\d+)/m)?.[1] || "1";
  window.store = (key) => (active() === "1" ? key : key.replace("betula.", `betula.p${active()}.`));
  window.plan = () => localStorage.getItem(window.store("betula.studyplan.v1")) || "";
  window.syncStatus = () => localStorage.getItem(`betula.sync.p${active()}.status`) || "";
};
/// The cookie of closed testing from a curl cookie jar, where the site has the gate.
const gate = process.env.SMOKE_COOKIES
  ? (await import("node:fs")).readFileSync(process.env.SMOKE_COOKIES, "utf8").split("\n").map((l) => l.replace(/^#HttpOnly_/, "").split("\t")).filter((f) => f.length >= 7 && f[5] === "betula_access").map((f) => ({ name: f[5], value: f[6], domain: f[0].replace(/^\./, ""), path: "/", secure: true, httpOnly: true }))
  : [];
/// A request to Phloem: on another port while working on the site, under /sync/v1/ of the site's host otherwise.
const toPhloem = (url) => url.includes(":8200") || url.includes("/sync/v1/");
const device = async (name) => {
  const context = await browser.newContext();
  if (gate.length) await context.addCookies(gate);
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(KEYS);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error" && !/Failed to load resource|WebSocket connection/.test(m.text())) problems.push(`${name} console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`${name} pageerror: ${String(e).slice(0, 300)}`));
  page.on("request", (r) => sent.push({ name, url: r.url(), body: r.postData() || "", headers: JSON.stringify(r.headers()) }));
  page.on("websocket", (ws) => ws.on("framesent", (f) => sent.push({ name, url: ws.url(), body: String(f.payload), headers: "" })));
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
    problems.push(`${name}: did not happen`);
    return false;
  }
};
const lines = (text, tag) => text.split("\n").filter((l) => l.startsWith(`${tag}\t`));
/// The page comes into view again, as after a switch of tabs.
const comeBack = (page) => page.evaluate(() => document.dispatchEvent(new Event("visibilitychange", { bubbles: true })));

// ---- Max: a plan with two modules and a Termin of his own, private ----
const max = await device("Max");
await go(max.page, "/studyplan");
await max.page.evaluate((plan) => localStorage.setItem("betula.studyplan.v1", plan), PLAN);
await go(max.page, "/studyplan");
await max.page.click("#sp-share");
await until("Max: the dialog opens", max.page, () => Boolean(onPage(".sh-dialog[open] .sh-opt")), null);
const opts = max.page.locator(".sh-dialog[open] .sh-opt");
check(await max.page.locator(".sh-dialog[open] .sh-opt[aria-checked=true]", { hasText: "Fester Stand" }).count() === 1, "Max: a private profile does not open with „Fester Stand“");
check(await max.page.locator(".sh-dialog[open] .sh-up").count() >= 1, "Max: no offer of Betula Sync under the fixed copy");
await opts.filter({ hasText: "Bleibt aktuell" }).click();
await opts.filter({ hasText: "Eine Person" }).click();
check(await max.page.locator(".sh-dialog[open] .sh-tag").count() === 2, "Max: the choices that need Betula Sync are not marked „mit Sync“");
check(await max.page.locator(".sh-dialog[open] .sh-up.need").count() === 1, "Max: no word on what saving the profile means");
await max.page.locator(".sh-dialog[open] .sh-sw", { hasText: "Eigene Termine" }).click();
await max.page.fill("#sh-who", "Lena");
const make = max.page.locator("#sh-make");
check((await make.textContent()).includes("Profil speichern und Link erstellen"), `Max: the button says „${await make.textContent()}“`);
await make.click();
await until("Max: the one-time link is made", max.page, () => Boolean(onPage(".sh-link")?.textContent.includes("#once=")), null, 30000);
const once = (await max.page.locator(".sh-link").first().textContent()).trim();
check(once.startsWith(base + "/studyplan#once="), `Max: the link is ${once}`);
// The link is there before the profile's connection to Phloem: over a real network (canary) the
// status still says "offline" for a moment.
await until("Max: the profile syncs after „Profil speichern“", max.page, () => syncStatus().startsWith("online"), null, 20000);
check(await max.page.locator(".sh-row", { hasText: "Lena" }).count() === 1, "Max: Lena is not in the list „Geteilt“");
const shareLine = await max.page.evaluate(() => plan().split("\n").find((l) => l.startsWith("s\t")) || "");
check(/^s\t\S{22}\t0\tp\t2026W\t\d+\tl1o\t\S{43}\t\S{64}\t\tLena$/.test(shareLine), `Max: the share's line is ${JSON.stringify(shareLine)}`);
const partKey = shareLine.split("\t")[7] || "none";

// ---- Lena (private) opens the one-time link and follows ----
const lena = await device("Lena");
await lena.page.goto(once);
await app(lena.page);
await until("Lena: the offer", lena.page, () => Boolean(onPage("#sp-live-follow")), null, 20000);
await lena.page.click("#sp-live-follow");
await until("Lena: a calendar that follows", lena.page, () => plan().split("\n").some((l) => l.startsWith("f\t")), null, 10000);
const lenaPlan = () => lena.page.evaluate(() => plan());
let text = await lenaPlan();
const calendar = (lines(text, "f")[0] || "").split("\t")[1] || "?";
check(lines(text, "n").filter((l) => l.startsWith(`n\t${calendar}\t`)).map((l) => l.split("\t")[3]).sort().join() === [...MODULES].sort().join(), `Lena: the calendar has other modules: ${lines(text, "n").join(" | ")}`);
check(lines(text, "t").some((l) => l.split("\t")[2] === calendar && l.includes("Arbeit")), "Lena: Max's Termin „Arbeit“ did not come along");
check(!lena.page.url().includes("#"), `Lena: the link stays in the address (${lena.page.url()})`);
await until("Lena: the row says „aktuell“", lena.page, () => [...document.querySelectorAll(".sp-cal .live")].some((e) => e.getBoundingClientRect().height > 0 || e.textContent.includes("aktuell")), null, 5000);

// ---- a third browser opens the same link: opened already, nothing of the plan ----
const other = await device("Other");
await other.page.goto(once);
await app(other.page);
await until("Other: „schon geöffnet“", other.page, () => (onPage(".offer[role=alert]")?.textContent || "").includes("schon geöffnet"), null, 20000);
check(!(await other.page.evaluate(() => plan())).includes("12104"), "Other: got something of the plan");

// ---- Max plans another module: Lena's page has it when it comes into view ----
await max.page.click(".sh-dialog[open] footer .mini.primary").catch(() => {});
await max.page.evaluate(([s, m]) => localStorage.setItem("betula.studyplan.v1", plan() + `m\t${s}\t${m}\t1790000100\t\n`), [SEMESTER, MORE[0]]);
await go(max.page, "/studyplan");
await max.page.waitForTimeout(4000);
await comeBack(lena.page);
await until("Lena: the module Max planned", lena.page, ([c, m]) => plan().includes(`n\t${c}\t2026W\t${m}`), [calendar, MORE[0]], 15000);

// ---- Lena saves her profile; Max plans once more, and her open page has it by itself ----
await lena.page.evaluate(() => [...document.querySelectorAll("button")].find((b) => b.textContent.trim() === "Profil speichern")?.click());
await until("Lena: synced", lena.page, () => syncStatus().startsWith("online"), null, 20000);
await lena.page.waitForTimeout(2000);
await max.page.evaluate(([s, m]) => localStorage.setItem("betula.studyplan.v1", plan() + `m\t${s}\t${m}\t1790000200\t\n`), [SEMESTER, MORE[1]]);
await go(max.page, "/studyplan");
await until("Lena: the next module by itself (Phloem's part)", lena.page, ([c, m]) => plan().includes(`n\t${c}\t2026W\t${m}`), [calendar, MORE[1]], 20000);

// ---- a calendar app's address that stays current ----
await max.page.click("#sp-share");
await until("Max: the dialog again", max.page, () => Boolean(onPage(".sh-dialog[open] .seg")), null);
await max.page.locator(".sh-dialog[open] .seg button", { hasText: "In Kalender-App" }).click();
check(await max.page.locator(".sh-dialog[open] .sh-opt[aria-checked=true]", { hasText: "Bleibt aktuell" }).count() === 1, "Max: a synced profile does not open with „Bleibt aktuell“");
await max.page.locator(".sh-dialog[open] .sh-sw", { hasText: "Eigene Termine" }).click();
await max.page.click("#sh-make");
await until("Max: the address is made", max.page, () => Boolean(onPage(".sh-link")?.textContent.includes("/calendar/s/")), null, 20000);
const feed = (await max.page.locator(".sh-link").first().textContent()).trim();
// A calendar service sends no cookie: the address alone must answer.
let answer = await fetch(feed);
let body = await answer.text();
check(answer.status === 200 && body.startsWith("BEGIN:VCALENDAR") && body.includes("Arbeit"), `the address answers ${answer.status}: ${body.split("\n").filter((l) => l.startsWith("SUMMARY")).slice(0, 8).join(" | ")} (share ${await max.page.evaluate(() => plan().split("\n").filter((l) => l.startsWith("s\t")).map((l) => l.split("\t").slice(2, 7).join(",")).join(" / "))})`);

// ---- Max stops sharing both ----
await max.page.locator(".sh-dialog[open] .sh-row", { hasText: "Lena" }).locator("button", { hasText: "Nicht mehr teilen" }).click();
await until("Max: Lena's share is gone", max.page, () => !plan().split("\n").some((l) => l.startsWith("s\t") && l.endsWith("\tLena")), null, 10000);
await until("Lena: „nicht mehr geteilt“", lena.page, () => plan().split("\n").some((l) => l.startsWith("f\t") && l.split("\t")[4] !== "0"), null, 20000);
check((await lenaPlan()).includes(`n\t${calendar}\t2026W\t${MODULES[0]}`), "Lena: the calendar lost what it showed");
const keep = await lena.page.evaluate(() => { const b = [...document.querySelectorAll("button")].find((b) => b.textContent.trim() === "Behalten"); b?.click(); return Boolean(b); });
if (!keep) await lena.page.screenshot({ path: process.env.SMOKE_SHOTS ? `${process.env.SMOKE_SHOTS}/lena-ended.png` : "/dev/null", fullPage: true }).catch(() => {});
check(keep, "Lena: no „Behalten“ on the page");
if (!(await until("Lena: „Behalten“ keeps the calendar and follows nothing", lena.page, () => !/(^|\n)f\t/.test(plan()) && /(^|\n)l\t/.test(plan()), null, 5000))) problems.push(`Lena's plan: ${JSON.stringify((await lenaPlan()).split("\n").filter((l) => /^[lf]\t/.test(l)))}`);
await max.page.locator(".sh-dialog[open] .sh-row", { hasText: "Kalender-App" }).locator("button", { hasText: "Nicht mehr teilen" }).click();
await until("Max: the app's share is gone", max.page, () => !plan().split("\n").some((l) => l.startsWith("s\t")), null, 10000);
answer = await fetch(feed);
check(answer.status === 404, `the address still answers ${answer.status} after „Nicht mehr teilen“`);

// ---- the profiles of the check end their sync: the service keeps nothing of it ----
for (const who of [max, lena]) {
  await go(who.page, "/profile");
  await who.page.getByRole("button", { name: /^Sync beenden/ }).first().click().catch(() => problems.push(`${who.name}: no „Sync beenden“`));
  await who.page.getByRole("button", { name: /^Beenden$/ }).first().click().catch(() => problems.push(`${who.name}: no „Beenden“ to confirm`));
  await until(`${who.name}: private again`, who.page, () => !syncStatus(), null, 15000);
}

// ---- privacy ----
for (const s of sent) {
  const leak = [...MODULES, ...MORE].find((m) => toPhloem(s.url) && (s.body.includes(m) || s.headers.includes(m))) || (s.body.includes("Arbeit") && toPhloem(s.url) && "Arbeit") || ((s.body.includes(partKey) || s.headers.includes(partKey)) && toPhloem(s.url) && "the part's key");
  if (leak) problems.push(`${s.name}: ${leak} in the clear to ${s.url}`);
  if (s.url.includes("#once=") || s.url.includes("#live=")) problems.push(`${s.name}: a link's part after # reached ${s.url}`);
}

await browser.close();
if (problems.length) {
  console.log(`share: ${problems.length} problem(s)\n- ${problems.join("\n- ")}`);
  process.exit(1);
}
console.log("share: ok");
