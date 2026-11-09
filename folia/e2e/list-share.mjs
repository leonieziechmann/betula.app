// Checks sharing the Merkliste (Betula Sync: folia/crates/bookmarks/src/share.rs, decision 6 of
// folia/design/teilen/teilen.html, docs/phloem/protocol.md) with three browsers against a running
// Phloem:
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node --import ./chromium.mjs list-share.mjs
// Folia runs with FOLIA_SYNC=on, FOLIA_SYNC_URL and FOLIA_SYNC_PARTS naming Phloem, Phloem with
// PHLOEM_ORIGINS naming Folia's origin. Against canary, SMOKE_COOKIES names a curl cookie jar that
// holds the cookie of closed testing (never the password); the profiles it syncs end their sync
// at the end („Sync beenden"), so the service keeps nothing of the check.
// Max (private) marks three modules, two of them in his collection „Für Lena", one with a note. The
// collection's menu has „Teilen …": he takes „Bleibt aktuell" and „Eine Person" (each marked „mit
// Sync"), the notes along, names it „Lena", and „Profil speichern und Link erstellen" saves his
// profile with Betula Sync and makes a one-time link. Lena (private) opens it and follows it as a
// list: it stands under „Geteilt mit dir" beside her collections, its rows Max's two modules, his
// note under one. A third browser opening the same link reads that it was opened already. Max puts
// another module in: Lena's list has it when her page comes into view again. Lena saves her profile
// („Profil speichern"); Max puts one more in, and Lena's open page has it without a reload
// (Phloem's `part`). „Alle übernehmen" marks the modules for her, as her collection „Für Lena" with
// Max's note. Max stops sharing: Lena's list keeps its last state, „wird nicht mehr geteilt", and
// „Löschen" takes it away. The whole Merkliste as a fixed copy for everybody is the link of today
// (`#m=`). Privacy: no request or frame carries a module's number, the note or the part's key in
// the clear, and a link's part after # reaches no server.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const SHARED = ["12104", "12107"];
const ALONE = "11101";
const MORE = ["12102", "11112"];
const NOTE = "Lena fragt nach dem Skript";
const MARKS = [...SHARED, ALONE].map((m, i) => `${m}\t${1790000000 - i}`).join("\n") + "\n";
const SHELF = `c\t1\tFür Lena\nm\t${SHARED[0]}\t1\nm\t${SHARED[1]}\t1\nn\t${SHARED[0]}\t${NOTE}\n`;

const sent = [];
const KEYS = () => {
  const active = () => (localStorage.getItem("betula.profiles.v1") || "").match(/^active\t(\d+)/m)?.[1] || "1";
  window.store = (key) => (active() === "1" ? key : key.replace("betula.", `betula.p${active()}.`));
  window.marked = () => (localStorage.getItem(window.store("betula.bookmarks.v1")) || "").split("\n").filter(Boolean).map((l) => l.split("\t")[0]);
  window.shelf = () => localStorage.getItem(window.store("betula.collections.v1")) || "";
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
/// The modules a list followed holds, by the rows the page shows.
const rows = (page) => page.evaluate(() => allOnPage("#rows a.row").map((a) => a.dataset.id));
/// Puts a module into Max's collection: marked, and in „Für Lena".
const putIn = async (page, module) => {
  await page.evaluate((m) => {
    localStorage.setItem(store("betula.bookmarks.v1"), `${m}\t${Math.floor(Date.now() / 1000)}\n` + (localStorage.getItem(store("betula.bookmarks.v1")) || ""));
    localStorage.setItem(store("betula.collections.v1"), shelf() + `m\t${m}\t1\n`);
  }, module);
};

// ---- Max: three marked modules, two of them in „Für Lena", private ----
const max = await device("Max");
await go(max.page, "/bookmarks");
await max.page.evaluate(([marks, shelf]) => { localStorage.setItem("betula.bookmarks.v1", marks); localStorage.setItem("betula.collections.v1", shelf); }, [MARKS, SHELF]);
await go(max.page, "/bookmarks?collection=1");
// The collection's menu in the sidebar: „Teilen …".
await max.page.locator(".coll-nav .coll-item", { hasText: "Für Lena" }).hover();
await max.page.locator(".coll-nav .coll-item", { hasText: "Für Lena" }).locator(".coll-more").click();
await max.page.locator(".view-menu [role=menuitem]", { hasText: "Teilen …" }).click();
await until("Max: the dialog opens", max.page, () => Boolean(onPage(".sh-dialog[open] .sh-opt")), null);
check((await max.page.locator(".sh-dialog[open] h2").first().textContent()).includes("„Für Lena“"), "Max: the dialog does not name the collection");
const opts = max.page.locator(".sh-dialog[open] .sh-opt");
check(await max.page.locator(".sh-dialog[open] .sh-opt[aria-checked=true]", { hasText: "Fester Stand" }).count() === 1, "Max: a private profile does not open with „Fester Stand“");
check(await max.page.locator(".sh-dialog[open] .sh-up").count() >= 1, "Max: no offer of Betula Sync under the fixed copy");
await opts.filter({ hasText: "Bleibt aktuell" }).click();
await opts.filter({ hasText: "Eine Person" }).click();
check(await max.page.locator(".sh-dialog[open] .sh-tag").count() === 2, "Max: the choices that need Betula Sync are not marked „mit Sync“");
check(await max.page.locator(".sh-dialog[open] .sh-up.need").count() === 1, "Max: no word on what saving the profile means");
await max.page.click("#sh-notes");
await max.page.fill("#sh-who", "Lena");
const make = max.page.locator("#sh-make");
check((await make.textContent()).includes("Profil speichern und Link erstellen"), `Max: the button says „${await make.textContent()}“`);
await make.click();
await until("Max: the one-time link is made", max.page, () => Boolean(onPage(".sh-link")?.textContent.includes("#once=")), null, 30000);
const once = (await max.page.locator(".sh-link").first().textContent()).trim();
check(once.startsWith(base + "/bookmarks#once="), `Max: the link is ${once}`);
await until("Max: the profile syncs after „Profil speichern“", max.page, () => syncStatus().startsWith("online"), null, 20000);
check(await max.page.locator(".sh-row", { hasText: "Lena" }).count() === 1, "Max: Lena is not in the list „Geteilt“");
const shareLine = await max.page.evaluate(() => shelf().split("\n").find((l) => l.startsWith("s\t")) || "");
check(/^s\t\S{22}\t1\t\d+\tlon\t\S{43}\t\S{64}\tLena$/.test(shareLine), `Max: the share's line is ${JSON.stringify(shareLine)}`);
const partKey = shareLine.split("\t")[5] || "none";

// ---- Lena (private) opens the one-time link and follows it as a list ----
const lena = await device("Lena");
await lena.page.goto(once);
await app(lena.page);
await until("Lena: the offer", lena.page, () => Boolean(onPage("#list-follow")), null, 20000);
check((await lena.page.locator(".offer", { hasText: "Für Lena" }).count()) === 1, "Lena: the offer does not name the list");
await lena.page.click("#list-follow");
await until("Lena: a list that follows", lena.page, () => shelf().split("\n").some((l) => l.startsWith("f\t")), null, 10000);
const followed = (lines(await lena.page.evaluate(() => shelf()), "f")[0] || "").split("\t")[1] || "?";
await until("Lena: the list's view", lena.page, (id) => location.search.includes(`collection=${id}`), followed, 10000);
check(!lena.page.url().includes("#"), `Lena: the link stays in the address (${lena.page.url()})`);
await until("Lena: Max's two modules", lena.page, (want) => allOnPage("#rows a.row").map((a) => a.dataset.id).sort().join() === want, [...SHARED].sort().join(), 15000);
check((await lena.page.evaluate(() => marked())).length === 0, "Lena: following marked something");
await until("Lena: Max's note at its row", lena.page, (note) => allOnPage(".row-note").some((n) => n.textContent.includes(note)), NOTE, 5000);
check(await lena.page.locator(".coll-nav .flabel", { hasText: "Geteilt mit dir" }).count() === 1, "Lena: no „Geteilt mit dir“ in the sidebar");
check(await lena.page.locator(".coll-nav .coll-item.followed", { hasText: "Für Lena" }).count() === 1, "Lena: the list is not under „Geteilt mit dir“");
check(await lena.page.locator(".fl-head", { hasText: "bleibt aktuell" }).count() === 1, "Lena: no head over the list followed");

// ---- a third browser opens the same link: opened already, nothing of the list ----
const other = await device("Other");
await other.page.goto(once);
await app(other.page);
await until("Other: „schon geöffnet“", other.page, () => (onPage(".offer[role=alert]")?.textContent || "").includes("schon geöffnet"), null, 20000);
check(!(await other.page.evaluate(() => shelf())).includes("f\t"), "Other: got something of the list");

// ---- Max puts another module in: Lena's page has it when it comes into view ----
await max.page.click(".sh-dialog[open] footer .mini.primary").catch(() => {});
await putIn(max.page, MORE[0]);
await go(max.page, "/bookmarks?collection=1");
await max.page.waitForTimeout(4000);
await comeBack(lena.page);
await until("Lena: the module Max put in", lena.page, (m) => allOnPage("#rows a.row").some((a) => a.dataset.id === m), MORE[0], 15000);

// ---- Lena saves her profile; Max puts one more in, and her open page has it by itself ----
await lena.page.click("#fl-save").catch(() => problems.push("Lena: no „Profil speichern“ over the list"));
await until("Lena: synced", lena.page, () => syncStatus().startsWith("online"), null, 20000);
await lena.page.waitForTimeout(2000);
await putIn(max.page, MORE[1]);
await go(max.page, "/bookmarks?collection=1");
await until("Lena: the next module by itself (Phloem's part)", lena.page, (m) => allOnPage("#rows a.row").some((a) => a.dataset.id === m), MORE[1], 20000);

// ---- „Alle übernehmen": marked, as her collection „Für Lena", with Max's note ----
await lena.page.click("#fl-take");
await until("Lena: the modules are marked", lena.page, (want) => want.every((m) => marked().includes(m)), [...SHARED, ...MORE], 5000);
const lenaShelf = await lena.page.evaluate(() => shelf());
const mine = (lines(lenaShelf, "c").find((l) => l.endsWith("\tFür Lena")) || "").split("\t")[1];
check(mine && lines(lenaShelf, "m").filter((l) => l.split("\t")[2].split(",").includes(mine)).length === 4, `Lena: no collection „Für Lena“ with the four modules: ${JSON.stringify(lines(lenaShelf, "c").concat(lines(lenaShelf, "m")))}`);
check(lines(lenaShelf, "n").some((l) => l.includes(NOTE)), "Lena: Max's note did not come along");
check(!(await rows(lena.page)).includes(ALONE), "Lena: a module outside the collection came along");

// ---- Max stops sharing: Lena's list keeps its last state, and „Löschen" takes it away ----
await max.page.locator(".coll-nav .coll-item", { hasText: "Für Lena" }).hover();
await max.page.locator(".coll-nav .coll-item", { hasText: "Für Lena" }).locator(".coll-more").click();
await max.page.locator(".view-menu [role=menuitem]", { hasText: "Teilen …" }).click();
await until("Max: the dialog again", max.page, () => Boolean(onPage(".sh-dialog[open] .sh-row")), null);
check(await max.page.locator(".sh-dialog[open] .sh-opt[aria-checked=true]", { hasText: "Bleibt aktuell" }).count() === 1, "Max: a synced profile does not open with „Bleibt aktuell“");
await max.page.locator(".sh-dialog[open] .sh-row", { hasText: "Lena" }).locator("button", { hasText: "Nicht mehr teilen" }).click();
await until("Max: Lena's share is gone", max.page, () => !shelf().split("\n").some((l) => l.startsWith("s\t")), null, 10000);
await until("Lena: „nicht mehr geteilt“", lena.page, () => shelf().split("\n").some((l) => l.startsWith("f\t") && l.split("\t")[4] !== "0"), null, 20000);
await until("Lena: the head asks what to do with it", lena.page, () => Boolean(onPage(".fl-head.warn #fl-delete")), null, 5000);
const last = await rows(lena.page);
check(last.length === 4, `Lena: the list lost its last state (${last.join(",")}; ${JSON.stringify(lines(await lena.page.evaluate(() => shelf()), "f").map((l) => l.split("\t").slice(0, 7).join(" ")))})`);
await lena.page.click("#fl-delete").catch(() => problems.push("Lena: no „Löschen“"));
await until("Lena: „Löschen“ takes the list away", lena.page, () => !/(^|\n)f\t/.test(shelf()) && !location.search.includes("collection="), null, 5000);
check((await lena.page.evaluate(() => marked())).length === 4, "Lena: „Löschen“ took her marks too");

// ---- the whole Merkliste as a fixed copy for everybody: the link of today ----
await max.page.click(".sh-dialog[open] footer .mini").catch(() => {});
await go(max.page, "/bookmarks");
await max.page.locator(".coll-nav .coll-item", { hasText: "Alle" }).first().hover();
await max.page.locator(".coll-nav .coll-item", { hasText: "Alle" }).first().locator(".coll-more").click();
await max.page.locator(".view-menu [role=menuitem]", { hasText: "Teilen …" }).click();
await until("Max: the dialog of the whole list", max.page, () => (onPage(".sh-dialog[open] h2")?.textContent || "").includes("Merkliste teilen"), null);
await max.page.locator(".sh-dialog[open] .sh-opt", { hasText: "Fester Stand" }).click();
await max.page.locator(".sh-dialog[open] .sh-opt", { hasText: "Alle mit dem Link" }).click();
await max.page.click("#sh-make");
await until("Max: the fixed link", max.page, () => Boolean(onPage(".sh-link")?.textContent.includes("/bookmarks#m=")), null, 5000);
check(await max.page.locator(".sh-dialog[open] .sh-row").count() === 0, "Max: a fixed copy for everybody is listed as a share");

// ---- the profiles of the check end their sync: the service keeps nothing of it ----
await max.page.click(".sh-dialog[open] footer .mini.primary").catch(() => {});
for (const who of [max, lena]) {
  await go(who.page, "/profile");
  await who.page.getByRole("button", { name: /^Sync beenden/ }).first().click().catch(() => problems.push(`${who.name}: no „Sync beenden“`));
  await who.page.getByRole("button", { name: /^Beenden$/ }).first().click().catch(() => problems.push(`${who.name}: no „Beenden“ to confirm`));
  await until(`${who.name}: private again`, who.page, () => !syncStatus(), null, 15000);
}

// ---- privacy ----
for (const s of sent) {
  const leak = [...SHARED, ALONE, ...MORE].find((m) => toPhloem(s.url) && (s.body.includes(m) || s.headers.includes(m))) || (s.body.includes("Skript") && toPhloem(s.url) && "the note") || ((s.body.includes(partKey) || s.headers.includes(partKey)) && toPhloem(s.url) && "the part's key");
  if (leak) problems.push(`${s.name}: ${leak} in the clear to ${s.url}`);
  if (s.url.includes("#once=") || s.url.includes("#live=") || s.url.includes("#m=")) problems.push(`${s.name}: a link's part after # reached ${s.url}`);
}

await browser.close();
if (problems.length) {
  console.log(`list-share: ${problems.length} problem(s)\n- ${problems.join("\n- ")}`);
  process.exit(1);
}
console.log("list-share: ok");
