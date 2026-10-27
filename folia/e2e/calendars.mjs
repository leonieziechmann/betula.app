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
// - „Neuer Kalender" from the link pasted, with a name; its menu renames it and deletes it, and
//   „Rückgängig" brings it back where it stood.
// - A phone: one line over the week, „Kalender" and how each stands, that opens into the rows.
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
  await until(page, "Lena's week", () => allOnPage(".sp-week a.slot.alt[data-pre]").length > 0);
  await page.click(".sp-week a.slot.alt[data-pre] >> nth=0");
  await until(page, "Lena's group ticked", () => /1 von \d/.test(onPage(".sp-focusbar .what small")?.textContent ?? ""));
  await page.keyboard.press("Escape");
  await until(page, "the focus closed", () => !location.search.includes("wahl="));
  link = (await page.getAttribute('[data-action="copy-text"][data-text*="share="]', "data-text")) ?? "";
  check(link.startsWith("/studyplan?share="), `Lena's link: ${link}`);
  await context.close();
}

// ---------- „Als Kalender" from the offer ----------
{
  const { page, context } = await open(wide, FS1, INFORMATIK, link);
  await until(page, "the offer", () => Boolean(onPage("#sp-share-calendar")));
  await page.click("#sp-share-calendar");
  await until(page, "the calendar made", () => !location.search.includes("share=") && /Als Kalender angelegt/.test(onPage(".sp-fold .offer")?.textContent ?? ""));
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
  await until(page, "deleted", () => allOnPage(".sp-cals .sp-cal").length === 2 && /gelöscht/.test(onPage(".sp-cals .sp-cal-note")?.textContent ?? ""));
  await page.click(".sp-cals .sp-cal-note button");
  await until(page, "brought back", () => allOnPage(".sp-cals .sp-cal").length === 3);
  r = await rows(page);
  check(r[1].startsWith("Lerngruppe") && r[2].startsWith("Lena"), `back where it stood: ${JSON.stringify(r)}`);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- a phone: the line over the week ----------
{
  const plan = FS1 + "l\t1\t\t013-82-2018\tLena\n" + ["11109", "13977", "12160", "12229", "13980"].map((id) => `n\t1\t2026W\t${id}`).join("\n") + "\n";
  const { page, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, plan, INFORMATIK);
  await until(page, "the phone's line", () => Boolean(onPage(".sp-cal-line > summary")));
  const said = await page.evaluate(() => onPage(".sp-cal-line > summary")?.innerText.replace(/\s+/g, " ").trim() ?? "");
  check(said === "Kalender Mein Studiengang, Lena als Spur", `the phone's line: „${said}"`);
  await page.tap(".sp-cal-line > summary");
  await page.waitForTimeout(400);
  const words = await page.evaluate(() => allOnPage(".sp-cal-line .sp-cal-seg button").map((b) => b.textContent.trim()));
  check(JSON.stringify(words) === JSON.stringify(["Voll", "Spur", "Aus"]) && (await page.evaluate(() => Boolean(onPage(".sp-cal-line .sp-cal-free")))), `the rows with their words and „Gemeinsam frei": ${JSON.stringify(words)}`);
  if (shots) await page.screenshot({ path: `${shots}/calendars-phone.png` });
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
