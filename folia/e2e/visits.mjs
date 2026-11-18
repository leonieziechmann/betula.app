// Modules only visited and „+ Modul" in every calendar (owner, 2026-10-26; folia/design/kalender/
// konzepte.html, „+ Modul in einem Kalender" and „Ein Besuchsmodul in „Mein Studium“";
// docs/folia/frontend.md, „Visits"), on canary's data of 2026W: Informatik B.Sc.'s first semester
// planned (EvS, EEG, PP, MIT1), Einführung in die Programmierung (EiP, 12105) taken in to visit.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node visits.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// - A visit (seeded: a `v` line, as „Mein Studium" writes it) in the week: ringed in its tone, giving
//   way where it meets the plan (its slots `gives`, no red clash; in the focus of its own choice too),
//   the head counting it apart („1 Besuch"), the quiet line counting only the choices („4 Wahlen
//   offen"), „Offen" in the mode saying where it gives way („EiP trifft VL MIT1 · Di 13:45 · der Besuch
//   gibt nach"), the list with it as one line with „Besuch" and its rows, and nothing to take it out.
// - „Mein Studium": the visit with its name, „nur besuchen · zählt nicht" and „–"; its menu
//   („Im Kalender öffnen", „Nur besuchen" ticked, „Entfernen"); without the tick it counts (an `m`
//   line), the planned module's menu makes it a visit again, „Entfernen" with „Rückgängig".
// - A calendar of one's own, while editing: its „…" has „Modul aufnehmen", the panel „Modul in
//   „Lena“", what it takes in is an `n` line and a row of its part, whose bin takes it out with
//   „Rückgängig". While looking the part has neither „+ Modul" nor the bin.
// - A phone: a calendar's „+ Modul" is a sheet of its own, in view, closed by Esc to the page.
// History: 2026-11-17, the edit mode. „+ Modul" of „Mein Studiengang" went (visits are made in „Mein
// Studium"), and with it the list's „×" of a visit and its credits; the first block seeds the visit
// and keeps what the week, „Offen" and the head say of it. A calendar's „+ Modul" is the mode's. „Mein
// Studium" says of a visit „nur besuchen · zählt nicht" (was „… · aus dem Kalender").
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
const FS1 = ["12104", "12107", "12102", "11112"].map((id) => `m\t2026W\t${id}\t1790000000\t`).join("\n") + "\n";
const INFORMATIK = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n";
const EIP = "12105";
const EIP_TITLE = "Einführung in die Programmierung";
const VISIT = `v\t2026W\t${EIP}\t1790000100\n`;
const LENA = "l\t1\t\t013-82-2018\tLena\n" + ["11109", "13977"].map((id) => `n\t1\t2026W\t${id}`).join("\n") + "\n";

const open = async (options, plan, path = "/studyplan") => {
  const context = await browser.newContext(options);
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  await context.addInitScript(([k1, k2, v1, v2]) => {
    if (localStorage.getItem(k1) === null) {
      localStorage.setItem(k1, v1);
      localStorage.setItem(k2, v2);
    }
  }, [PLAN, MINE, plan, INFORMATIK]);
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
const stored = (page) => page.evaluate((key) => (localStorage.getItem(key) ?? "").split("\n").filter((line) => line.includes(`\t12105`)), PLAN);
const text = (page, selector) => page.evaluate((selector) => onPage(selector)?.innerText.replace(/\s+/g, " ").trim() ?? null, selector);
const wide = { viewport: { width: 1440, height: 900 } };

// ---------- the visit in the week ----------
{
  const { page, context } = await open(wide, FS1 + VISIT);
  // The week: ringed, giving way, no clash.
  await until(page, "the visit in the week", () => allOnPage(".sp-week .slot.visit").length > 0 && allOnPage(".sp-week .slot.gives").length > 0);
  const week = await page.evaluate(() => ({
    gives: allOnPage(".sp-week .slot.gives").map((slot) => slot.title),
    clashes: allOnPage(".sp-week .slot.clash").length,
  }));
  check(week.gives.every((title) => /Besuch, zählt nicht/.test(title) && /der Besuch gibt nach/.test(title)), `the slots that give way: ${JSON.stringify(week.gives)}`);
  check(week.clashes === 0, `the visit made a clash: ${JSON.stringify(week)}`);
  // The head counts it apart; the quiet line counts the choices alone.
  check((await text(page, ".sp-sum")) === "Informatik B.Sc. · 4 Module · 1 Besuch", `the head: ${await text(page, ".sp-sum")}`);
  check((await text(page, ".sp-status .sp-open-way")) === "4 Wahlen offen", `the quiet line: ${await text(page, ".sp-status .sp-open-way")}`);
  // The list: one line with „Besuch", its rows, nothing that takes it out, no credits.
  const row = await page.evaluate((id) => {
    const mod = onPage(`.sp-list .sp-mod[data-id="${id}"]`);
    return mod && { line: mod.querySelector(".sp-mod-row")?.innerText.replace(/\s+/g, " ").trim(), visit: mod.querySelector(".sp-mod-row")?.classList.contains("visit"), rows: mod.querySelectorAll(".sp-cat").length, out: mod.querySelectorAll(".sp-remove, .lp, .mono").length };
  }, EIP);
  check(row?.line === `EiP ${EIP_TITLE} Besuch` && row.visit && row.rows > 0 && row.out === 0, `the list's visit: ${JSON.stringify(row)}`);
  // „Offen" in the mode says where it gives way.
  await page.click(".sp-status .sp-open-way");
  await until(page, "the mode", () => Boolean(onPage(".sp-editbar")) && allOnPage(".sp-side .sp-open-row.gives").length > 0);
  const lines = await page.evaluate(() => allOnPage(".sp-side .sp-open-row.gives").map((row) => row.innerText.replace(/\s+/g, " ").trim()));
  check(lines.includes("EiP trifft VL MIT1 Di 13:45 · der Besuch gibt nach Ansehen"), `„Offen“: ${JSON.stringify(lines)}`);
  check(await page.evaluate((id) => onPage(`.sp-list .sp-mod[data-id="${id}"]`)?.querySelectorAll(".sp-remove").length === 0, EIP), "editing: the visit can be taken out of the list");
  if (shots) await page.screenshot({ path: `${shots}/visits-week.png` });
  // The visit's own choice in focus: its groups that meet the plan give way, yellow, never red.
  await page.click('.sp-side .sp-open-row:has-text("Übung EiP") >> visible=true');
  await until(page, "the visit's choice in focus", () => location.search.includes("wahl=") && allOnPage(".sp-week .slot.pick").length > 1);
  const focus = await page.evaluate(() => ({ picks: allOnPage(".sp-week .slot.pick").length, red: allOnPage(".sp-week .slot.pick.clash").length, yellow: allOnPage(".sp-week .slot.pick.gives").length }));
  check(focus.red === 0 && focus.yellow > 0, `the visit's groups in focus: ${JSON.stringify(focus)}`);
  if (shots) await page.screenshot({ path: `${shots}/visits-focus.png` });
  await page.keyboard.press("Escape");
  await until(page, "the focus closed", () => !location.search.includes("wahl="));
  check((await stored(page)).length === 1, `the plan's lines of EiP: ${JSON.stringify(await stored(page))}`);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- „Mein Studium": the visit and its menu ----------
{
  const { page, context } = await open(wide, FS1 + VISIT, "/study");
  const visit = `.st-visit[data-key="v:${EIP}"]`;
  await until(page, "the visit with its name", ([selector, title]) => onPage(`${selector} .st-name`)?.textContent === title, [visit, EIP_TITLE]);
  const said = await text(page, visit);
  check(said?.startsWith(`${EIP_TITLE} ${EIP} nur besuchen · zählt nicht`) && said.includes("–"), `the visit's row: „${said}"`);
  const menu = () => page.evaluate(() => allOnPage(".st-pop [role^=menuitem]").map((item) => `${item.id}${item.getAttribute("aria-checked") === "true" ? "✓" : ""}`));
  await page.click(`${visit} .st-status`);
  await until(page, "the visit's menu", () => Boolean(onPage("#st-menu-visit")));
  const entries = await menu();
  check(JSON.stringify(entries) === JSON.stringify(["st-menu-calendar", "st-menu-visit✓", "st-menu-remove"]), `the visit's menu: ${JSON.stringify(entries)}`);
  check((await page.getAttribute("#st-menu-calendar", "href"))?.includes(`open=${EIP}`), "„Im Kalender öffnen“ does not lead to it");
  // Without the tick it counts; the planned module's menu makes it a visit again.
  await page.click("#st-menu-visit");
  await until(page, "it counts", ([key, id]) => (localStorage.getItem(key) ?? "").includes(`m\t2026W\t${id}\t`) && !(localStorage.getItem(key) ?? "").includes(`v\t2026W\t${id}\t`) && Boolean(onPage(`.st-row[data-key="m:${id}"]`)), [PLAN, EIP]);
  await page.click(`.st-row[data-key="m:${EIP}"] .st-status`);
  await until(page, "the planned module's „Nur besuchen“", () => onPage("#st-menu-visit")?.getAttribute("aria-checked") === "false");
  await page.click("#st-menu-visit");
  await until(page, "a visit again", ([key, id, selector]) => (localStorage.getItem(key) ?? "").includes(`v\t2026W\t${id}\t`) && !(localStorage.getItem(key) ?? "").includes(`m\t2026W\t${id}\t`) && Boolean(onPage(selector)), [PLAN, EIP, visit]);
  if (shots) await page.screenshot({ path: `${shots}/visits-study.png` });
  // „Entfernen", and „Rückgängig".
  await page.click(`${visit} .st-status`);
  await until(page, "the visit's menu again", () => Boolean(onPage("#st-menu-remove")));
  await page.click("#st-menu-remove");
  await until(page, "the visit removed", ([key, id]) => !(localStorage.getItem(key) ?? "").includes(`\t${id}\t`) && Boolean(onPage(".study-note")), [PLAN, EIP]);
  await page.click(".study-note .plan-toast-undo", { timeout: 5000 }).catch(() => {});
  await until(page, "the visit back", ([key, id]) => (localStorage.getItem(key) ?? "").includes(`v\t2026W\t${id}\t`), [PLAN, EIP]);
  check((await stored(page)).length === 1, `the plan's lines of EiP: ${JSON.stringify(await stored(page))}`);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- a calendar of one's own ----------
{
  const { page, context } = await open(wide, FS1 + LENA);
  await until(page, "Lena's part while looking", () => Boolean(onPage(".sp-cal-mods .sp-mod-row")));
  check(await page.evaluate(() => allOnPage(".sp-cal-mods .sp-add-mod, .sp-cal-mods .sp-remove, .sp-cal .sp-cal-more").filter((el) => el.getClientRects().length).length === 0), "looking: „+ Modul“, a bin or a menu in Lena's part");
  await page.click("#sp-edit-toggle");
  await until(page, "Lena's part while editing", () => Boolean(onPage(".sp-cal-mods .sp-add-mod")) && Boolean(onPage(".sp-cal:not(.mine) .sp-cal-more")));
  await page.click(".sp-cal:not(.mine) .sp-cal-more >> nth=0");
  await page.click('.sp-cal-menu button:has-text("Modul aufnehmen")');
  await until(page, "„Modul in „Lena““", () => onPage(".sp-cal-mods .sp-addp-head b")?.textContent === "Modul in „Lena“" && onPage(".sp-cal-mods .sp-addp-says")?.textContent === "Module in „Lena“ zählen nirgends." && !onPage(".sp-cal-mods .sp-addp a.sp-way"));
  await page.fill(".sp-cal-mods .sp-addp .sp-marked-search input", EIP);
  await until(page, "the search in Lena's", (id) => Boolean(onPage(`.sp-cal-mods .sp-addp li[data-id="${id}"] .sp-fit`)), EIP);
  await page.click(`.sp-cal-mods .sp-addp li[data-id="${EIP}"] .mini`);
  await until(page, "taken into Lena's calendar", ([key, id, title]) => {
    const text = localStorage.getItem(key) ?? "";
    return text.includes(`n\t1\t2026W\t${id}`) && !text.includes(`v\t2026W\t${id}`) && !text.includes(`m\t2026W\t${id}`)
      && allOnPage(".sp-cal-mods .sp-row").some((row) => row.textContent.includes(title));
  }, [PLAN, EIP, EIP_TITLE]);
  // Its row's bin takes it out, „Rückgängig" brings it back.
  check(await page.evaluate((title) => [...document.querySelectorAll(".sp-cal-mods .sp-row-wrap")].find((wrap) => wrap.textContent.includes(title))?.querySelector(".sp-remove use")?.getAttribute("href")?.endsWith("#trash-2"), EIP_TITLE), "the calendar's module has no bin");
  await page.click(`.sp-cal-mods .sp-row-wrap:has-text("${EIP_TITLE}") .sp-remove`);
  await until(page, "taken out of Lena's", ([key, id, title]) => !(localStorage.getItem(key) ?? "").includes(`n\t1\t2026W\t${id}`) && onPage(".sp-cal-mods .sp-marked-note")?.textContent.includes(`„${title}“ aus „Lena“ genommen`), [PLAN, EIP, EIP_TITLE]);
  await page.click(".sp-cal-mods .sp-marked-note .mini");
  await until(page, "back in Lena's", ([key, id]) => (localStorage.getItem(key) ?? "").includes(`n\t1\t2026W\t${id}`), [PLAN, EIP]);
  if (shots) await page.screenshot({ path: `${shots}/visits-calendar.png` });
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- a phone: a calendar's „+ Modul" in view ----------
{
  const { page, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, FS1 + VISIT + LENA, "/studyplan?edit=1");
  await until(page, "the phone's calendar part", () => Boolean(onPage(".sp-cal-mods .sp-add-mod")));
  await page.tap(".sp-cal-mods .sp-add-mod >> visible=true");
  // A sheet of its own, named for the calendar, its search in view; Esc closes it to the page.
  await until(page, "„+ Modul“ a sheet on a phone", () => {
    const panel = document.querySelector("dialog.sp-add-sheet[open] .sp-addp")?.getBoundingClientRect();
    return panel && panel.top >= 0 && panel.top < innerHeight - 80 && panel.right <= innerWidth + 0.5 && document.querySelector("#sp-add-sheet-title")?.textContent === "Modul in „Lena“" && !onPage(".sp-cal-mods .sp-addp");
  });
  if (shots) await page.screenshot({ path: `${shots}/visits-phone.png` });
  await page.keyboard.press("Escape");
  await until(page, "Esc closes „+ Modul“ to the page", () => !document.querySelector("dialog[open]") && /[?&]edit=1/.test(location.search));
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
