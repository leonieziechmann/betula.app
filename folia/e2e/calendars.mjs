// Calendars of one's own beside „Mein Studiengang" (owner, 2026-10-26; docs/folia/frontend.md,
// „Calendars of one's own"), on canary's data of 2026W: Lena's BWL B.Sc. (five modules of its first
// semester, a group of hers ticked) handed on by her link to someone with Informatik B.Sc.'s first
// semester planned.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node calendars.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// - Lena's link carries her ticked group; the offer it opens has „Als Kalender", which puts her
//   plan beside one's own (the plan's text: its `l`, `n` and `o` lines, nothing planned of it), with
//   „Rückgängig".
// - The calendar stands as a strip at first (more than eight Termine a week), its switch takes it
//   in full (outlined, its initial in the corner) and off; „Gemeinsam frei" in the head draws where
//   both are free; the module list has its part.
// - While looking the rows of „Kalender" stand with their „Voll · Spur · Aus", without „+ Kalender" and
//   without a menu; „Bearbeiten" brings both.
// - „Neuer Kalender" from the link pasted, with a name; its menu renames it and deletes it, and
//   „Rückgängig" brings it back where it stood. A calendar made by its name alone, without a link.
// - A phone: the head's „Kalender" with the number of calendars of one's own as its badge („Kalender, 1
//   weiterer") opens a sheet, in both states, its line saying what a calendar is for, with the rows,
//   „Gemeinsam frei", each calendar's menu and „+ Kalender", a finger's 44 px each; Esc closes the menu
//   first; a link that cannot be read makes an empty calendar and says so.
// History: 2026-11-17, the edit mode. Lena ticks her group in its card („Diese Gruppe anhaken"), not by
// the click on the hatched slot that took it into focus with the group ticked (that preselection went);
// „+ Kalender" and the calendars' menus are the mode's; „Neuer Kalender" takes a name alone.
// 2026-11-18: on a phone the line over the week became the sheet of the head's „Kalender" (owner: „Dann
// fehlt am Handy das Feature für weitere Kalender komplett bitte baue das wieder ein").
// Needs the snapshot whose current semester is WiSe 2026/27. Fails on a page load after takeover, a
// console error, or a step that does not show up.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const shots = process.env.SMOKE_SHOTS;
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
const planOf = (ids) => ids.map((id) => `m\t2026W\t${id}\t1790000000\t`).join("\n") + "\n";
const LENA = planOf(["11109", "13977", "12160", "12229", "13980"]);
const BWL = "program\t013-82-2018\nname\tBetriebswirtschaftslehre B.Sc.\ncaption\t\nstart\t2026W\n";
const FS1 = planOf(["12104", "12107", "12102", "11112"]);
const INFORMATIK = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n";

const open = async (options, plan, mine, path = "/studyplan") => {
  const context = await browser.newContext(options);
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  await context.addInitScript(([k1, k2, v1, v2]) => {
    if (localStorage.getItem(k1) === null) {
      localStorage.setItem(k1, v1);
      localStorage.setItem(k2, v2);
    }
  }, [PLAN, MINE, plan, mine]);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await page.evaluate(() => { window.__marker = 1; });
  return { page, context };
};
const until = async (page, name, fn, arg) => {
  try {
    await page.waitForFunction(fn, arg, { timeout: 10000 });
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
  }
};
const lines = (page, kinds) => page.evaluate(([key, kinds]) => (localStorage.getItem(key) ?? "").split("\n").filter((line) => kinds.includes(line.split("\t")[0])), [PLAN, kinds]);
const rows = (page) => page.evaluate(() => allOnPage(".sp-cals .sp-cal").map((row) => row.innerText.replace(/\s+/g, " ").trim()));
const week = (page) => page.evaluate(() => ({
  cal: allOnPage(".sp-week .slot.cal").length,
  strip: allOnPage(".sp-week .slot.strip").length,
  who: allOnPage(".sp-week .slot.cal .slot-who").length,
  bands: allOnPage(".sp-week .band.free").map((band) => band.textContent.trim()),
}));
const wide = { viewport: { width: 1440, height: 900 } };

// ---------- Lena's link, her group ticked ----------
let link = "";
{
  const { page, context } = await open(wide, LENA, BWL);
  await until(page, "Lena's week", () => allOnPage(".sp-week a.slot.alt").length > 0);
  // Her group ticked in its card.
  await page.click(".sp-week a.slot.alt >> nth=0").catch(() => problems.push("Lena's hatched group: nothing to click"));
  await until(page, "its card", () => document.activeElement?.matches("dialog[open] .tm-big[data-action='tick']"));
  await page.keyboard.press("Enter");
  await until(page, "Lena's group ticked", ([key]) => (localStorage.getItem(key) ?? "").split("\n").some((line) => line.startsWith("c\t2026W\t")), [PLAN]);
  await page.keyboard.press("Escape");
  await until(page, "the card closed", () => !location.search.includes("termin=") && !document.querySelector("dialog[open]"));
  // „Teilen …": a private profile hands on the fixed copy for everybody, the link of today.
  await page.click("#sp-share");
  await until(page, "the dialog „Teilen“", () => Boolean(onPage(".sh-dialog[open] #sh-make")));
  await page.click("#sh-make");
  await until(page, "the link made", () => (onPage(".sh-dialog[open] .sh-link")?.textContent ?? "").includes("share="));
  const made = new URL(((await page.textContent(".sh-dialog[open] .sh-link")) ?? "").trim());
  link = made.pathname + made.search;
  check(link.startsWith("/studyplan?share="), `Lena's link: ${link}`);
  await context.close();
}

// ---------- „Als Kalender" from the offer ----------
{
  const { page, context } = await open(wide, FS1, INFORMATIK, link);
  await until(page, "the offer", () => Boolean(onPage("#sp-share-calendar")));
  await page.click("#sp-share-calendar");
  await until(page, "the calendar made, the page's note says so", () => !location.search.includes("share=") && /Als Kalender angelegt/.test(onPage(".wait-note")?.textContent ?? "") && !onPage(".sp-fold .offer"));
  const stored = await lines(page, ["l", "n", "o", "m"]);
  check(stored.filter((line) => line.startsWith("l\t1\t")).length === 1 && stored.filter((line) => line.startsWith("n\t1\t2026W\t")).length === 5 && stored.filter((line) => line.startsWith("o\t1\t2026W\t")).length === 1, `the calendar's lines: ${JSON.stringify(stored)}`);
  check(stored.filter((line) => line.startsWith("m\t")).length === 4, `nothing of it planned: ${JSON.stringify(stored)}`);
  // Its row says „Termine kommen …" until the Termine of Lena's Studiengang are there.
  await until(page, "the calendar's Termine", () => /5 Module/.test(allOnPage(".sp-cals .sp-cal")[1]?.innerText ?? ""));
  let r = await rows(page);
  check(r.length === 2 && r[0].startsWith("Mein Studiengang") && r[1].startsWith("Kalender 1 5 Module"), `the list: ${JSON.stringify(r)}`);

  // A strip at first, then in full, then off; „Gemeinsam frei" where both are free.
  await until(page, "the strip", () => allOnPage(".sp-week .slot.strip").length > 0);
  let w = await week(page);
  check(w.strip > 8 && w.cal === 0, `a strip at first: ${JSON.stringify(w)}`);
  await page.click(".sp-cal:not(.mine) .sp-cal-seg button >> nth=0");
  await until(page, "in full", () => allOnPage(".sp-week .slot.cal").length > 0);
  w = await week(page);
  check(w.cal > w.strip && w.who === w.cal && w.strip === 0, `in full with its initial: ${JSON.stringify(w)}`);
  check(await page.evaluate(() => Boolean(onPage(".sp-head .sp-together"))), "no „Gemeinsam frei\" in the head");
  await page.click(".sp-head .sp-together");
  await until(page, "„Gemeinsam frei\"", () => allOnPage(".sp-week .band.free").length > 0);
  w = await week(page);
  check(w.bands.length > 0 && w.bands.every((label) => label === "beide frei"), `where both are free: ${JSON.stringify(w)}`);
  const part = await page.evaluate(() => onPage(".sp-side .sp-cal-mods")?.innerText.replace(/\s+/g, " ").trim() ?? "");
  check(/MW1/.test(part) && /aus einem Link · mit den Gruppen/.test(part), `the module list's part: „${part}"`);
  if (shots) await page.screenshot({ path: `${shots}/calendars-full.png` });
  await page.click(".sp-cal:not(.mine) .sp-cal-seg button >> nth=2");
  await until(page, "off", () => allOnPage(".sp-week .slot.cal, .sp-week .slot.strip, .sp-week .band").length === 0);

  // Looking: no „+ Kalender", no menu; the mode brings them.
  const shown = (selector) => page.evaluate((selector) => allOnPage(selector).filter((el) => el.getClientRects().length).length, selector);
  check(await shown(".sp-cals .sp-cal-add") === 0 && await shown(".sp-cal .sp-cal-more") === 0, "looking: „+ Kalender“ or a calendar's menu");
  await page.click("#sp-edit-toggle");
  await until(page, "the mode", () => Boolean(onPage(".sp-editbar")) && Boolean(onPage(".sp-cals .sp-cal-add")) && Boolean(onPage(".sp-cal:not(.mine) .sp-cal-more")));

  // „Neuer Kalender" from the link, named; renamed, deleted and brought back.
  await page.click(".sp-cal-add");
  await page.fill(".sp-cal-new input[inputmode=url]", `https://betula.app${link}`);
  await until(page, "what the link holds", () => /5 Module, mit 1 Gruppe/.test(onPage(".sp-cal-holds")?.textContent ?? ""));
  await page.fill(".sp-cal-new input:not([inputmode])", "Lena");
  await page.click(".sp-cal-new button[type=submit]");
  await until(page, "the second calendar", () => allOnPage(".sp-cals .sp-cal").length === 3);
  await page.click(".sp-cal:not(.mine) .sp-cal-more >> nth=0");
  await page.click('.sp-cal-menu button:has-text("Umbenennen")');
  await page.fill(".sp-cal-rename input", "Lerngruppe");
  await page.keyboard.press("Enter");
  await until(page, "renamed", () => allOnPage(".sp-cals .sp-cal b").some((b) => b.textContent === "Lerngruppe"));
  await page.click(".sp-cal:not(.mine) .sp-cal-more >> nth=0");
  await page.click('.sp-cal-menu button:has-text("Löschen")');
  // The page's note says so, in the place of the offer's „Als Kalender angelegt: Lena" (note.rs;
  // until 2026-11-19 a line under the calendars), and stays for a click elsewhere.
  await until(page, "deleted", () => allOnPage(".sp-cals .sp-cal").length === 2 && /„Lerngruppe“ gelöscht/.test(onPage(".wait-note")?.textContent ?? "") && !onPage(".sp-cals .sp-cal-note"));
  await page.waitForTimeout(500);
  await page.click(".sp-cals .flabel");
  await page.waitForTimeout(400);
  check(await page.evaluate(() => /„Lerngruppe“ gelöscht/.test(onPage(".wait-note:not(.leaving)")?.textContent ?? "")), "a click elsewhere took the note of the calendar deleted and its „Rückgängig“ with it");
  await page.click("#plan-note-undo");
  await until(page, "brought back", () => allOnPage(".sp-cals .sp-cal").length === 3);
  r = await rows(page);
  check(r[1].startsWith("Lerngruppe") && r[2].startsWith("Lena"), `back where it stood: ${JSON.stringify(r)}`);
  // A calendar by its name alone: no link, „Anlegen" never disabled.
  await page.click(".sp-cal-add");
  await until(page, "the form again", () => document.activeElement?.id === "sp-cal-new-name");
  check(await page.evaluate(() => !onPage(".sp-cal-new button[type=submit]")?.disabled), "„Anlegen“ is disabled without a link");
  await page.fill("#sp-cal-new-name", "WG");
  await page.click(".sp-cal-new button[type=submit]");
  await until(page, "a calendar by its name alone", ([key]) => /^l\t3\t\t\tWG$/m.test(localStorage.getItem(key) ?? "") && allOnPage(".sp-cals .sp-cal").length === 4, [PLAN]);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- a phone: the sheet „Kalender" of the head ----------
{
  const plan = FS1 + "l\t1\t\t013-82-2018\tLena\n" + ["11109", "13977", "12160", "12229", "13980"].map((id) => `n\t1\t2026W\t${id}`).join("\n") + "\n";
  const { page, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, plan, INFORMATIK);
  await until(page, "the head's „Kalender“", () => Boolean(onPage(".sp-head .sp-tool[data-action='calendars']")));
  check(await page.evaluate(() => !document.querySelector(".sp-cal-line, .sp-cal-newrow:not(dialog *)")), "a phone still has the line over the week or „+ Kalender“ in the list");
  // The head's „Kalender" says that a calendar of one's own exists: its number as a badge at the
  // sign, in its name „Kalender, 1 weiterer".
  const badge = await page.evaluate(() => { const b = onPage(".sp-head .sp-tool[data-action='calendars']"); return { name: b?.getAttribute("aria-label"), badge: b?.querySelector(".sp-badge")?.textContent }; });
  check(badge.name === "Kalender, 1 weiterer" && badge.badge === "1", `the head's „Kalender“ does not say that a calendar exists: ${JSON.stringify(badge)}`);
  // While looking: it is opened on purpose, so it has everything.
  await page.tap("#content > .page-stage.front .sp-tool[data-action='calendars']");
  await until(page, "the sheet", () => Boolean(document.querySelector("dialog.sp-cal-sheet[open] .sp-cal:not(.mine)")));
  const words = await page.evaluate(() => [...document.querySelectorAll("dialog.sp-cal-sheet[open] .sp-cal-seg button")].map((b) => b.textContent.trim()));
  const has = await page.evaluate(() => ({ mine: Boolean(document.querySelector("dialog.sp-cal-sheet[open] .sp-cal.mine")), free: Boolean(document.querySelector("dialog.sp-cal-sheet[open] .sp-cal-free")), more: Boolean(document.querySelector("dialog.sp-cal-sheet[open] .sp-cal-more")), add: Boolean(document.querySelector("dialog.sp-cal-sheet[open] .sp-cal-add")), title: document.querySelector("dialog.sp-cal-sheet[open] h2")?.textContent.trim() }));
  check(JSON.stringify(words) === JSON.stringify(["Voll", "Spur", "Aus"]) && has.mine && has.free && has.more && has.add && has.title === "Kalender", `the sheet: the rows with their words, „Gemeinsam frei“, the menu and „+ Kalender“, looking: ${JSON.stringify({ words, has })}`);
  // What a calendar is for, in one sentence under the title; a finger's 44 px for „…" and the switch.
  const sub = await page.evaluate(() => document.querySelector("dialog.sp-cal-sheet[open] .st-dlg-head p")?.textContent.trim());
  check(sub === "Dein Studium und weitere Kalender daneben – von Freunden, für ein Nebenfach oder aus einem Link.", `the sheet's line: ${sub}`);
  const sizes = await page.evaluate(() => [...document.querySelectorAll("dialog.sp-cal-sheet[open] :is(.sp-cal-more, .sp-cal-seg button)")].map((b) => Math.round(b.getBoundingClientRect().height)));
  check(sizes.length >= 4 && sizes.every((h) => h >= 44), `targets of the sheet under 44 px: ${JSON.stringify(sizes)}`);
  // Its menu: a question's answers each a row (the app's rule for sheets).
  await page.tap("dialog.sp-cal-sheet[open] .sp-cal-more");
  await until(page, "the menu", () => Boolean(document.querySelector("dialog.sp-cal-sheet[open] .sp-cal-menu")));
  const menu = await page.evaluate(() => [...document.querySelectorAll("dialog.sp-cal-sheet[open] .sp-cal-menu > button")].map((b) => b.textContent.trim()));
  check(menu.includes("Umbenennen") && menu.includes("Löschen") && menu.includes("Modul aufnehmen"), `the menu: ${JSON.stringify(menu)}`);
  // Esc closes the menu first, the sheet stays.
  await page.keyboard.press("Escape");
  await until(page, "the menu closed, the sheet open", () => !document.querySelector("dialog.sp-cal-sheet[open] .sp-cal-menu") && Boolean(document.querySelector("dialog.sp-cal-sheet[open]")));
  // „+ Kalender" by a name alone.
  await page.tap("dialog.sp-cal-sheet[open] .sp-cal-add");
  await until(page, "the form", () => Boolean(document.querySelector("dialog.sp-cal-sheet[open] #sp-cal-new-name")));
  await page.fill("#sp-cal-new-name", "WG");
  await page.tap("dialog.sp-cal-sheet[open] .sp-cal-new button[type=submit]");
  await until(page, "a calendar by its name in the sheet", ([key]) => /^l\t2\t\t\tWG$/m.test(localStorage.getItem(key) ?? "") && document.querySelectorAll("dialog.sp-cal-sheet[open] .sp-cal:not(.mine)").length === 2, [PLAN]);
  check(await page.evaluate(() => onPage(".sp-head .sp-tool[data-action='calendars'] .sp-badge")?.textContent === "2"), "the badge does not count the new calendar");
  // A link that cannot be read: the calendar is made, empty, and the note says so (Betula does not
  // say no, but it does not keep quiet either).
  await page.tap("dialog.sp-cal-sheet[open] .sp-cal-add");
  await until(page, "the form once more", () => Boolean(document.querySelector("dialog.sp-cal-sheet[open] #sp-cal-new-name")));
  const fields = await page.evaluate(() => [...document.querySelectorAll("dialog.sp-cal-sheet[open] .sp-cal-new input")].map((i) => Math.round(i.getBoundingClientRect().height)));
  check(fields.length === 2 && fields.every((h) => h >= 44), `the fields of „+ Kalender“ under 44 px: ${JSON.stringify(fields)}`);
  await page.fill("#sp-cal-new-name", "Max");
  await page.fill("dialog.sp-cal-sheet[open] .sp-cal-new input >> nth=1", "hello world");
  await page.tap("dialog.sp-cal-sheet[open] .sp-cal-new button[type=submit]");
  await until(page, "an empty calendar from a link not read, and the note says so", ([key]) => /^l\t3\t\t\tMax$/m.test(localStorage.getItem(key) ?? "") && document.querySelector("dialog.sp-cal-sheet[open] .wait-note .mark-toast-said")?.textContent.trim() === "Link nicht lesbar – leerer Kalender angelegt: Max" && (() => { const box = document.querySelector("dialog.sp-cal-sheet[open] .wait-note").getBoundingClientRect(); return box.top >= 0 && box.bottom <= innerHeight; })(), [PLAN]);
  if (shots) await page.screenshot({ path: `${shots}/calendars-phone.png` });
  await page.keyboard.press("Escape");
  await until(page, "the sheet closed", () => !document.querySelector("dialog.sp-cal-sheet[open]"));
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
