// Checks „Weitergeben" of the Merkliste (folia/design/merken/oberflaeche.html, „Weitergeben und
// Ankommen"; the Merkliste's step 5).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node share.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// A collection's menu „Teilen": its head, „Link kopieren", „Notizen mitnehmen" off, „Als Text
// kopieren" with the number, title, LP and link of each module. Its link on another device: „Für
// Lenachen · 2 Module aus einem Link." with „Als Sammlung merken", „Nur merken", „Verwerfen"; kept
// as a collection without the note. Shared again with the notes: „Die Sammlung gibt es schon." with
// „Zu ‚…‘ hinzufügen" and „Anderer Name …", which refuses a name there is and keeps one that is not,
// the note arriving. The whole list's „Link für ein anderes Gerät": every collection and note on a
// third device at once. The selection's „Teilen": a link of modules alone. Nothing of the names,
// the notes or the modules goes into a request: they travel behind the #.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";
import { fileURLToPath } from "node:url";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
const KEY = "betula.bookmarks.v1";
const SHELF = "betula.collections.v1";
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const LENA = "Für Lenachen";
const NOTE = "Lenchen fragen, ob sie mitkommt";

const open = async () => {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, permissions: ["clipboard-read", "clipboard-write"] });
  await context.addInitScript(ON_PAGE);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  const requests = [];
  page.on("request", (r) => requests.push(r.url() + " " + (r.postData() || "")));
  const takeover = () => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  const go = async (path) => {
    await page.goto(base + path, { waitUntil: "domcontentloaded" });
    await takeover();
    await page.evaluate(() => { window.__marker = 1; });
  };
  const step = async (name, action, until, arg) => {
    const started = Date.now();
    await action();
    try {
      await page.waitForFunction(until, arg, { timeout: 8000 });
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    timings[name] = Date.now() - started;
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
  };
  const marks = () => page.evaluate((key) => (localStorage.getItem(key) || "").split("\n").filter(Boolean).map((line) => line.split("\t")[0]), KEY);
  const shelf = () => page.evaluate((key) => localStorage.getItem(key) || "", SHELF);
  return { context, page, go, step, requests, marks, shelf };
};
const clipboard = (page) => page.evaluate(() => navigator.clipboard.readText()).catch(() => "");

// ---------- the sender: four marked modules, two of them in „Für Lenachen", a note at one ----------
const sender = await open();
await sender.go("/catalog");
await sender.page.waitForFunction(() => allOnPage(".rows a.row").length >= 4, null, { timeout: 20000 });
const ids = await sender.page.evaluate(() => [...allOnPage(".rows a.row")].map((row) => row.dataset.id).slice(0, 4));
const now = Math.floor(Date.now() / 1000);
await sender.page.evaluate(([key, shelf, ids, now, lena, note]) => {
  localStorage.setItem(key, ids.map((id, i) => `${id}\t${now - i * 60}\n`).join(""));
  localStorage.setItem(shelf, `c\t1\t${lena}\nm\t${ids[0]}\t1\nm\t${ids[1]}\t1\nn\t${ids[0]}\t${note}\n`);
}, [KEY, "betula.collections.v1", ids, now, LENA, NOTE]);
sender.requests.length = 0;
await sender.go("/bookmarks");
await sender.page.waitForFunction(() => allOnPage("#rows a.row").length === 4, null, { timeout: 20000 }).catch(() => problems.push("the sender's Merkliste does not list four modules"));
const lenaMenu = async () => {
  await sender.page.click('.coll-nav a[href*="collection=1"] >> visible=true', { button: "right" });
  await sender.page.waitForSelector(".view-menu [role=menuitem] >> visible=true", { timeout: 4000 });
};
await lenaMenu();
const entries = await sender.page.evaluate(() => [...document.querySelectorAll(".view-menu .st-pop-item")].map((item) => item.querySelector(".st-pop-label > span")?.textContent));
check(entries.join("|").startsWith("Umbenennen|Teilen|"), `a collection's menu: ${entries}`);
await sender.step("Teilen", () => sender.page.click('.view-menu .st-pop-item:has-text("Teilen")'), () => document.querySelector(".view-menu.share-menu"));
const share = await sender.page.evaluate(() => ({
  head: document.querySelector(".share-menu .view-menu-head")?.textContent,
  items: [...document.querySelectorAll(".share-menu .st-pop-item")].map((item) => `${item.querySelector(".st-pop-label > span")?.textContent}${item.getAttribute("role") === "menuitemcheckbox" ? `:${item.getAttribute("aria-checked")}` : ""}`),
}));
check(share.head === `${LENA} · 2 Module` && share.items.join("|") === "Link kopieren|Notizen mitnehmen:false|Als Text kopieren", `the share menu: ${JSON.stringify(share)}`);
// The keys go through it, the switch among the entries.
await sender.page.waitForFunction(() => document.activeElement?.closest(".share-menu"), null, { timeout: 2000 }).catch(() => {});
await sender.step("↓ to the switch", () => sender.page.keyboard.press("ArrowDown"), () => document.activeElement?.getAttribute("role") === "menuitemcheckbox");
await sender.step("↓ past it", () => sender.page.keyboard.press("ArrowDown"), () => document.activeElement?.textContent.includes("Als Text kopieren"));
await sender.step("Als Text kopieren", () => sender.page.click('.share-menu .st-pop-item:has-text("Als Text kopieren")'), () => document.querySelector(".wait-note .mark-toast-said")?.textContent.includes("Liste kopiert") && !document.querySelector(".view-menu"));
const text = await clipboard(sender.page);
const lines = text.split("\n");
check(lines.length === 2 && lines.every((line) => /^\d{5} .+ · (\d+(,\d+)? LP · )?https?:\/\/.+\/catalog\/module\/\d{5}$/.test(line)) && [ids[0], ids[1]].every((id) => text.includes(`/catalog/module/${id}`)), `the collection as text: ${JSON.stringify(text)}`);
await lenaMenu();
await sender.page.click('.view-menu .st-pop-item:has-text("Teilen")');
await sender.step("Link kopieren", () => sender.page.click('.share-menu .st-pop-item:has-text("Link kopieren")'), () => document.querySelector(".wait-note .mark-toast-said")?.textContent.includes("Link kopiert"));
const plainLink = await clipboard(sender.page);
check(plainLink.startsWith(`${base}/bookmarks#m=`), `the collection's link: ${plainLink}`);

// ---------- another device: the collection arrives ----------
const other = await open();
await other.page.goto(plainLink, { waitUntil: "domcontentloaded" });
await other.page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
await other.page.evaluate(() => { window.__marker = 1; });
await other.page.waitForSelector(".offer >> visible=true", { timeout: 8000 }).catch(() => problems.push("another device: the collection is not offered"));
const offered = await other.page.evaluate(() => ({ text: onPage(".offer p")?.textContent.trim(), buttons: [...onPage(".offer").querySelectorAll("button")].map((b) => b.textContent.trim()) }));
check(offered.text === `${LENA} · 2 Module aus einem Link.` && offered.buttons.join("|") === "Als Sammlung merken|Nur merken|Verwerfen", `another device, the collection: ${JSON.stringify(offered)}`);
check((await other.marks()).length === 0, "another device: something was kept before the visitor said so");
await other.step("Als Sammlung merken", () => other.page.click("#offer-add"), () => !onPage(".offer") && location.hash === "" && allOnPage("#rows a.row").length === 2);
const kept = await other.shelf();
check(kept.includes(`c\t1\t${LENA}\n`) && kept.includes(`m\t${ids[0]}\t1`) && kept.includes(`m\t${ids[1]}\t1`) && !kept.includes(NOTE), `another device kept: ${JSON.stringify(kept)}`);

// Shared again, with the notes: the collection is there, „Anderer Name …".
await sender.go("/bookmarks?collection=1");
await lenaMenu();
await sender.page.click('.view-menu .st-pop-item:has-text("Teilen")');
await sender.step("Notizen mitnehmen", () => sender.page.click(".share-menu .share-switch"), () => document.querySelector(".share-menu .share-switch")?.getAttribute("aria-checked") === "true");
await sender.step("the link with the notes", () => sender.page.click('.share-menu .st-pop-item:has-text("Link kopieren")'), () => document.querySelector(".wait-note .mark-toast-said")?.textContent.includes("Link kopiert"));
const noteLink = await clipboard(sender.page);
check(noteLink !== plainLink && noteLink.length > plainLink.length, "the link with the notes is not longer");
await other.page.goto(noteLink, { waitUntil: "domcontentloaded" });
await other.page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
await other.page.evaluate(() => { window.__marker = 1; });
await other.page.waitForSelector(".offer >> visible=true", { timeout: 8000 }).catch(() => problems.push("another device: the collection is not offered again"));
const again = await other.page.evaluate(() => ({ text: onPage(".offer p")?.textContent.trim(), buttons: [...onPage(".offer").querySelectorAll("button")].map((b) => b.textContent.trim()) }));
check(again.text === `${LENA} · 2 Module aus einem Link. Die Sammlung gibt es schon.` && again.buttons.join("|") === `Zu ‚${LENA}‘ hinzufügen|Anderer Name …|Verwerfen`, `another device, the collection there: ${JSON.stringify(again)}`);
await other.step("Anderer Name …", () => other.page.click("#offer-other"), () => document.activeElement?.id === "offer-name");
await other.page.keyboard.type(LENA.toLowerCase());
await other.step("a name there is", () => other.page.keyboard.press("Enter"), () => onPage(".offer-name small")?.textContent === "Gibt es schon" && onPage(".offer"));
await other.page.fill("#offer-name", "Von Lenachen");
await other.step("a name there is not", () => other.page.keyboard.press("Enter"), () => !onPage(".offer") && location.hash === "");
const named = await other.shelf();
check(named.includes("c\t2\tVon Lenachen\n") && named.includes(`m\t${ids[0]}\t1,2`) && named.includes(`n\t${ids[0]}\t${NOTE}`), `another device, under another name: ${JSON.stringify(named)}`);

// ---------- the whole list for another device: every collection and note ----------
await sender.go("/bookmarks");
await sender.page.click('.coll-nav > div.coll-item:first-of-type > a >> visible=true', { button: "right" });
await sender.page.waitForSelector(".view-menu [role=menuitem] >> visible=true", { timeout: 4000 });
await sender.step("Link für ein anderes Gerät", () => sender.page.click('.view-menu .st-pop-item:has-text("Link für ein anderes Gerät")'), () => document.querySelector(".wait-note .mark-toast-said")?.textContent.includes("Link kopiert"));
const deviceLink = await clipboard(sender.page);
const third = await open();
await third.page.goto(deviceLink, { waitUntil: "domcontentloaded" });
await third.page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
await third.page.evaluate(() => { window.__marker = 1; });
await third.page.waitForSelector(".offer >> visible=true", { timeout: 8000 }).catch(() => problems.push("a third device: the list is not offered"));
const whole = await third.page.evaluate(() => ({ text: onPage(".offer p")?.textContent.trim(), buttons: [...onPage(".offer").querySelectorAll("button")].map((b) => b.textContent.trim()) }));
check(whole.text === "4 Module in 1 Sammlung aus einem Link. 4 davon neu." && whole.buttons.join("|") === "Hinzufügen|Verwerfen", `a third device: ${JSON.stringify(whole)}`);
await third.step("Hinzufügen", () => third.page.click("#offer-add"), () => !onPage(".offer") && allOnPage("#rows a.row").length === 4);
const everything = await third.shelf();
check(everything.includes(`c\t1\t${LENA}\n`) && everything.includes(`n\t${ids[0]}\t${NOTE}`) && (await third.marks()).length === 4, `a third device kept: ${JSON.stringify(everything)}`);

// ---------- the selection's „Teilen": modules alone ----------
await sender.page.click(`.page-stage.front #rows a.row[data-id="${ids[2]}"]`, { modifiers: ["Control"] });
await sender.page.click(`.page-stage.front #rows a.row[data-id="${ids[3]}"]`, { modifiers: ["Control"] });
await sender.page.click(".page-stage.front #pick-share");
await sender.page.waitForSelector(".pick-menu .st-pop-item");
const pickShare = await sender.page.evaluate(() => [...document.querySelectorAll(".pick-menu .st-pop-item")].map((item) => `${item.querySelector(".st-pop-label > span")?.textContent}${item.getAttribute("role") === "menuitemcheckbox" ? `:${item.getAttribute("aria-checked")}` : ""}`));
check(pickShare.join("|") === "Link kopieren|Notizen mitnehmen:false|Als Text kopieren", `the selection's „Teilen“ (a page loaded anew starts with the notes at home): ${pickShare}`);
await sender.step("the selection's link", () => sender.page.click('.pick-menu .st-pop-item:has-text("Link kopieren")'), () => /Link kopiert/.test(document.querySelector(".mark-note")?.textContent || ""));
const pickLink = await clipboard(sender.page);
const fourth = await open();
await fourth.page.goto(pickLink, { waitUntil: "domcontentloaded" });
await fourth.page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
await fourth.page.waitForSelector(".offer >> visible=true", { timeout: 8000 }).catch(() => problems.push("a fourth device: the modules are not offered"));
const modules = await fourth.page.evaluate(() => ({ text: onPage(".offer p")?.textContent.trim(), buttons: [...onPage(".offer").querySelectorAll("button")].map((b) => b.textContent.trim()) }));
check(modules.text === "2 Module aus einem Link. Auf deine Merkliste setzen?" && modules.buttons.join("|") === "Hinzufügen|Verwerfen", `a fourth device: ${JSON.stringify(modules)}`);

// Nothing of it in a request: it travels behind the #.
for (const [who, { requests }] of [["sender", sender], ["other", other], ["third", third], ["fourth", fourth]]) {
  const leaks = requests.filter((r) => r.includes("Lenachen") || r.includes(encodeURIComponent(LENA)) || r.includes("Lenchen") || r.includes("#m=") || ids.some((id) => r.includes(`m=${id}`)));
  check(leaks.length === 0, `${who}: what was shared went into a request: ${leaks.slice(0, 2).join(" | ")}`);
}

for (const { context } of [sender, other, third, fourth]) await context.close();
await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
