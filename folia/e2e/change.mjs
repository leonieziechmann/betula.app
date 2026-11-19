// Changes of Termine of QIS and notes, step 2 of folia/design/kalender/termine.html (owner,
// 2026-11-03: „Passt, bau Schritt 2 jetzt in Folia"; docs/folia/frontend.md, „Changes of Termine of
// QIS"), from the card of one Termin (owner, 2026-11-17: „viele Professoren halten ihre daten nicht
// ordentlich aktuell"), on canary's data of 2026W: Informatik B.Sc.'s first semester planned (EvS, EEG,
// PP, MIT1).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node change.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// - VL EvS on Friday, from its slot of the week: the card's pencil „Ändern" opens the form in the
//   card's body, „Für" above Tag, Zeit and Raum, „Alle …" chosen, „QIS sagt: Fr 09:15–10:45 · ZHG/HS.C".
//   Another room and „Speichern": its `x` line, the ticket's pen at the room, „Von dir geändert, alle
//   Termine · QIS: ZHG/HS.C" with „Zurücksetzen", the slot's pen in the week; „Zurücksetzen" takes it
//   all back.
// - „Nur am …" on Thursday at 10:00: what it meets before „Speichern", the line of the one date,
//   the date in the week on Thursday with its pen.
// - The note and „Nachholtermin" are entries of the card's „⋯", not lines of its body. The note: its
//   words, „Speichern": its `b` line and a line of the card („Notiz: …") with its own „Bearbeiten",
//   the menu's entry now „Notiz bearbeiten".
// - „Nachholtermin": the form of a Termin of one's own of the module on the date shown, „Was" empty and
//   „Art" Vorlesung; „Anlegen": a `t` line of that date with its kind, the slot „VL EvS".
// - The share link carries a change, and the calendar made of it keeps it (its `x` line of
//   calendar 1).
// - „Offen" asks while editing: looking, none of its lines stands, and the quiet line counts the
//   changes beside the choices („3 Wahlen offen · 4 Änderungen offen ›"), leading into the mode, where
//   a change QIS says something else of now („QIS sagt jetzt ZHG/HS.C") leads to its card, where
//   „Meine behalten" settles it; one QIS took over („OK"); one of a Termin QIS moved („Übernehmen"
//   hangs it on the Termin QIS has now); one whose Termin is gone („Als eigenen Termin behalten"). A line
//   settled keeps its place, pale („erledigt"), until the mode is left; then the quiet line says the
//   choices alone.
// - A phone: a Termin QIS says something else of, its answers each a row of its own across the
//   sheet, „Meine behalten" first; „Ändern" in the sheet, nothing wider than the screen, „Speichern"
//   and „Abbrechen" one under the other.
// History: 2026-11-17, the card. „Ändern" is the card's pencil; the note and „Nachholtermin" moved
// into its „⋯"; „Nachholtermin" leaves „Was" empty and sets „Art" (the week says „VL EvS" by the kind);
// the lines of „Offen" stand in the edit mode. Later the same day: the quiet line counts the changes
// too, and a settled line stays pale in its place while editing (the check waited for it to go).
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
const dialog = "dialog[open] .tm-dialog";
const wide = { viewport: { width: 1440, height: 900 } };
// Betula's select (`combobox::Select`, since 2026-11-19 in the Termin's forms): its button, then
// the entry of that value in the list it opens, as a hand picks it.
const pick = async (page, button, value) => {
  await page.click(button);
  await page.click(`${button} ~ .combo-pop [role="option"][data-value="${value}"]`);
};

const open = async (options, plan, path = "/studyplan") => {
  const context = await browser.newContext(options);
  // The check's own clock, a Monday morning of the lecture period, running on from there: the date
  // moved „nur am …" to Thursday 10:00 lies ahead whenever the check runs. With the machine's clock it
  // was past on a Thursday after 11:30 and until the week's end, and the card then showed the date
  // after it, without „Von dir geändert" (seen on Thursday, 2026-11-19).
  await context.clock.install({ time: new Date("2026-11-16T08:00:00") });
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
const lines = (page, tag) => page.evaluate(([key, tag]) => (localStorage.getItem(key) ?? "").split("\n").filter((line) => line.startsWith(`${tag}\t`)), [PLAN, tag]);
const text = (page, selector) => page.evaluate((selector) => onPage(selector)?.innerText.replace(/\s+/g, " ").trim() ?? null, selector);
// The slot of `name` in the week's column of `day` (1 = Monday) that begins at `from`.
const slotOf = (page, day, name, from) => page.evaluate(([day, name, from]) => {
  const col = onPage(".sp-week .week")?.querySelectorAll(".col")[day - 1];
  const slot = [...(col?.querySelectorAll("a.slot") ?? [])].find((slot) => slot.textContent.includes(name) && slot.textContent.includes(from));
  return slot ? { href: slot.getAttribute("href"), edited: slot.classList.contains("edited"), pen: Boolean(slot.querySelector(".slot-pen")), conflict: slot.classList.contains("conflict") } : null;
}, [day, name, from]);
const keyOf = (href) => /termin=(\d+-[0-9a-f]{5})/.exec(href ?? "")?.[1] ?? "";

// ---------- „Ändern" for every date, „Zurücksetzen" ----------
let friday = "";
let tuesday = "";
{
  const { page, context } = await open(wide, FS1);
  await until(page, "the week", () => allOnPage(".sp-week a.slot").length > 0);
  const slot = await slotOf(page, 5, "EvS", "09:15");
  friday = keyOf(slot?.href);
  tuesday = keyOf((await slotOf(page, 2, "EvS", "11:30"))?.href);
  check(friday.startsWith("148701-") && tuesday.startsWith("148701-") && friday !== tuesday, `VL EvS's two Termine: ${friday}, ${tuesday}`);
  await page.click(`.sp-week a.slot[href*="termin=${friday}"] >> nth=0`);
  await until(page, "the Termin's card", (dialog) => Boolean(onPage(`${dialog}.tm-card .tk`)), dialog);
  // „Ändern" is the card's pencil; the note and „Nachholtermin" are not in its body.
  const tools = await page.evaluate((dialog) => ({
    pencil: onPage(`${dialog} .tm-tools [data-action="change"] use`)?.getAttribute("href")?.split("#")[1] ?? "",
    label: onPage(`${dialog} .tm-tools [data-action="change"]`)?.getAttribute("aria-label") ?? "",
    body: allOnPage(`${dialog} .dbody [data-action="note"], ${dialog} .dbody [data-action="makeup"]`).length,
  }), dialog);
  check(tools.pencil === "pencil" && tools.label === "Ändern" && tools.body === 0, `the card's tools: ${JSON.stringify(tools)}`);
  await page.click(`${dialog} .tm-tools [data-action="change"]`);
  await until(page, "„Ändern“ in the card", (dialog) => Boolean(onPage(`${dialog}.tm-card .dbody #tm-change`)), dialog);
  const form = await page.evaluate((dialog) => {
    const seg = [...(onPage(`${dialog} #tm-change .chg-seg`)?.children ?? [])];
    const day = onPage(`${dialog} #chg-day`);
    return {
      seg: seg.map((button) => [button.textContent.trim(), button.getAttribute("aria-checked")]),
      above: Boolean(seg[0] && day && seg[0].getBoundingClientRect().bottom <= day.getBoundingClientRect().top),
      qis: onPage(`${dialog} .chg-qis`)?.textContent.trim(),
      room: onPage(`${dialog} #chg-room`)?.value,
    };
  }, dialog);
  check(form.seg.length === 3 && /^Nur am /.test(form.seg[0][0]) && /^Ab /.test(form.seg[1][0]) && /^Alle \d+$/.test(form.seg[2][0]) && form.seg[2][1] === "true", `„Für“: ${JSON.stringify(form.seg)}`);
  check(form.above, "„Für“ stands above what it changes");
  check(form.qis === "QIS sagt: Fr 09:15–10:45 · ZHG/HS.C" && form.room === "ZHG/HS.C", `what QIS says: ${JSON.stringify(form)}`);
  await page.fill(`${dialog} #chg-room`, "ZHG/HS.A");
  await until(page, "what the change meets", (dialog) => Boolean(onPage(`${dialog} #tm-change .own-fit`)), dialog);
  if (shots) await page.screenshot({ path: `${shots}/change-form.png` });
  await page.click(`${dialog} [data-action="change-save"]`);
  await until(page, "„Speichern“: the line, the ticket, the view", ([dialog, key, plan]) => {
    const text = localStorage.getItem(plan) ?? "";
    return text.includes(`x\t0\t2026W\t${key}\tall\t\t\t\tZHG/HS.A\t5\t09:15-10:45\tZHG/HS.C\t12104\n`) && !onPage(`${dialog} #tm-change`) && Boolean(onPage(`${dialog} .tk-room .tk-pen`)) && /ZHG\/HS\.A/.test(onPage(`${dialog} .tk-room`)?.textContent ?? "");
  }, [dialog, friday, PLAN]);
  const said = await text(page, `${dialog} .tm-changed`);
  check(said === "Von dir geändert, alle Termine · QIS: ZHG/HS.C Zurücksetzen", `the changed line: „${said}“`);
  const marked = await slotOf(page, 5, "EvS", "09:15");
  check(marked?.edited && marked?.pen && !marked?.conflict, `the slot's pen: ${JSON.stringify(marked)}`);
  if (shots) await page.screenshot({ path: `${shots}/change-saved.png` });
  await page.click(`${dialog} [data-action="change-reset"]`);
  await until(page, "„Zurücksetzen“", ([dialog, plan]) => !(localStorage.getItem(plan) ?? "").includes("\nx\t") && !onPage(`${dialog} .tm-changed`) && !onPage(`${dialog} .tk-pen`), [dialog, PLAN]);
  check(!(await slotOf(page, 5, "EvS", "09:15"))?.edited, "the slot without its pen");

  // ---------- „Nur am …" on Thursday at 10:00 ----------
  await page.click(`${dialog} [data-action="change"]`);
  await until(page, "„Ändern“ again", (dialog) => Boolean(onPage(`${dialog} #tm-change`)), dialog);
  await page.click(`${dialog} #tm-change .chg-seg > button >> nth=0`);
  await pick(page, `${dialog} #chg-day`, "4");
  await page.fill(`${dialog} #tm-change .chg-two input[type="time"] >> nth=0`, "10:00");
  await page.fill(`${dialog} #tm-change .chg-two input[type="time"] >> nth=1`, "11:30");
  await until(page, "what one date on Thursday meets", (dialog) => Boolean(onPage(`${dialog} #tm-change .own-fit`)), dialog);
  const fit = await text(page, `${dialog} #tm-change .own-fit`);
  check(/^(Passt in deine Woche|Trifft .+)$/.test(fit ?? ""), `its fit: „${fit}“`);
  await page.click(`${dialog} [data-action="change-save"]`);
  await until(page, "the line of one date", ([plan, key]) => (localStorage.getItem(plan) ?? "").split("\n").some((line) => line.startsWith(`x\t0\t2026W\t${key}\ton\t`) && line.endsWith("\t4\t10:00-11:30\t\t5\t09:15-10:45\tZHG/HS.C\t12104")), [PLAN, friday]);
  await until(page, "the date on Thursday in the week", () => {
    const col = onPage(".sp-week .week")?.querySelectorAll(".col")[3];
    return [...(col?.querySelectorAll("a.slot.edited") ?? [])].some((slot) => slot.textContent.includes("EvS") && Boolean(slot.querySelector(".slot-pen")));
  });
  await until(page, "the view says „nur am …“", (dialog) => /^Von dir geändert, nur am /.test(onPage(`${dialog} .tm-changed`)?.textContent ?? ""), dialog);
  await page.click(`${dialog} [data-action="change-reset"]`);
  await until(page, "the one date back", ([plan]) => !(localStorage.getItem(plan) ?? "").includes("\nx\t"), [PLAN]);

  // ---------- the note ----------
  await page.click(`${dialog} .tm-tools [data-action="more"]`);
  await page.click(`${dialog} .tm-menu [data-action="note"]`);
  await until(page, "the note's field", (dialog) => Boolean(onPage(`${dialog} #tm-note-field`)), dialog);
  await page.fill(`${dialog} #tm-note-field`, "Folien vorher lesen");
  await page.click(`${dialog} [data-action="note-save"]`);
  await until(page, "the note", ([dialog, plan, key]) => (localStorage.getItem(plan) ?? "").includes(`b\t0\t2026W\t${key}\tFolien vorher lesen\n`) && onPage(`${dialog} .tm-note-line .tm-note-text`)?.textContent === "Folien vorher lesen" && Boolean(onPage(`${dialog} .tm-note-line [data-action="note-edit"]`)), [dialog, PLAN, friday]);
  await page.click(`${dialog} .tm-tools [data-action="more"]`);
  check((await text(page, `${dialog} .tm-menu [data-action="note"]`)) === "Notiz bearbeiten", `the menu's entry of a note that exists: „${await text(page, `${dialog} .tm-menu [data-action="note"]`)}“`);
  await page.keyboard.press("Escape");
  await until(page, "the menu closed", (dialog) => !onPage(`${dialog} .tm-menu`) && Boolean(onPage(dialog)), dialog);
  const shown = await text(page, `${dialog} .tk-date > b`);

  // ---------- „Nachholtermin" ----------
  await page.click(`${dialog} .tm-tools [data-action="more"]`);
  await page.click(`${dialog} .tm-menu [data-action="makeup"]`);
  await until(page, "the form of a Termin of one's own", () => Boolean(onPage("dialog.own-form[open]")));
  const made = await page.evaluate(() => ({ what: onPage("dialog.own-form[open] #own-what")?.value, kind: onPage("dialog.own-form[open] #own-kind")?.dataset.value, module: onPage("dialog.own-form[open] #own-module")?.dataset.value, title: onPage("dialog.own-form[open] #own-form-title")?.textContent }));
  check(made.what === "" && made.kind === "lecture" && made.module === "12104" && made.title === "Neuer Termin", `the make-up's form: ${JSON.stringify(made)}`);
  await page.click("dialog.own-form .st-dlg-foot .primary");
  await until(page, "the make-up made: a date of its own with its kind, „VL EvS“ in the week", ([plan, day]) => {
    const line = (localStorage.getItem(plan) ?? "").split("\n").find((line) => line.startsWith("t\t")) ?? "";
    const [, , , , rhythm, date, time, , , module, name, , , kind] = line.split("\t");
    return rhythm === "d" && `${date.slice(8, 10)}.${date.slice(5, 7)}.` === day.slice(3) && time === "09:15-10:45" && module === "12104" && name === "" && kind === "lecture"
      && allOnPage(".sp-week a.slot.own").some((slot) => slot.querySelector(".l")?.textContent.replace(/\s+/g, " ").trim() === "VL EvS");
  }, [PLAN, shown ?? ""]);
  await context.close();
}

// ---------- the share link carries a change; the calendar made of it keeps it ----------
{
  const room = `x\t0\t2026W\t${friday}\tall\t\t\t\tZHG/HS.A\t5\t09:15-10:45\tZHG/HS.C\t12104\n`;
  const { page, context } = await open(wide, FS1 + room);
  await until(page, "the week with the change", () => allOnPage(".sp-week a.slot.edited").length > 0);
  // „Teilen …": a private profile hands on the fixed copy for everybody, the link of today.
  await page.click("#sp-share");
  await until(page, "the dialog „Teilen“", () => Boolean(onPage(".sh-dialog[open] #sh-make")));
  await page.click("#sh-make");
  await until(page, "the link made", () => (onPage(".sh-dialog[open] .sh-link")?.textContent ?? "").includes("share="));
  const made = new URL(((await page.textContent(".sh-dialog[open] .sh-link")) ?? "").trim());
  const link = made.pathname + made.search;
  check(link.startsWith("/studyplan?share="), `the link: ${link}`);
  await context.close();
  const other = await open(wide, FS1, link);
  await until(other.page, "the offer", () => Boolean(onPage("#sp-share-calendar")));
  await other.page.click("#sp-share-calendar");
  await until(other.page, "the calendar with the change", ([plan, key]) => (localStorage.getItem(plan) ?? "").split("\n").some((line) => line.startsWith(`x\t1\t2026W\t${key}\tall\t`) && line.includes("\tZHG/HS.A\t")), [PLAN, friday]);
  await other.context.close();
}

// ---------- „Offen" ----------
{
  // QIS says ZHG/HS.C of the Friday, the change was made while it said ZHG/HS.B; of the Tuesday it
  // says what the change says; a Termin of the Tutorium of MIT1 QIS gave another key; one of an
  // event QIS has no more.
  const seeded = [
    `x\t0\t2026W\t${friday}\tall\t\t\t\tZHG/HS.A\t5\t09:15-10:45\tZHG/HS.B\t12104`,
    `x\t0\t2026W\t${tuesday}\tall\t\t\t\tZHG/HS.C\t2\t11:30-13:00\tZHG/HS.B\t12104`,
    "x\t0\t2026W\t150132-00001\tall\t\t\t\tHG/2.46\t2\t15:30-17:00\tHG/2.45\t11112",
    "x\t0\t2026W\t999999-00001\tall\t\t\t\tZHG/HS.A\t3\t09:15-10:45\tHG/1.01\t12104",
  ].join("\n") + "\n";
  const { page, context } = await open(wide, FS1 + seeded);
  // Looking, none of the lines stands; the way into the mode does.
  await until(page, "the week while looking", () => allOnPage(".sp-week a.slot").length > 0 && Boolean(onPage(".sp-status .sp-open-way")));
  check(await page.evaluate(() => allOnPage(".sp-open-row.chg").filter((row) => row.getClientRects().length).length === 0), "a line of „Offen\" while looking");
  const way = await page.evaluate(() => onPage(".sp-status .sp-open-way")?.textContent.replace(/\s+/g, " ").trim());
  check(way === "3 Wahlen offen · 4 Änderungen offen", `the quiet line: „${way}"`);
  await page.click(".sp-status .sp-open-way");
  await until(page, "the mode", () => /[?&]edit=1/.test(location.search) && Boolean(onPage(".sp-editbar")));
  const rows = () => page.evaluate(() => allOnPage(".sp-side .sp-open-row.chg").map((row) => row.innerText.replace(/\s+/g, " ").trim()));
  await until(page, "four lines of changes", () => allOnPage(".sp-side .sp-open-row.chg").length === 4);
  const four = await rows();
  check(four.some((row) => /^VL EvS: QIS sagt jetzt ZHG\/HS\.C Fr 09:15 · du: ZHG\/HS\.A Ansehen$/.test(row)), `QIS says something else: ${JSON.stringify(four)}`);
  check(four.some((row) => /^VL EvS: QIS sagt jetzt dasselbe wie du .* OK$/.test(row)), `QIS took it over: ${JSON.stringify(four)}`);
  check(four.some((row) => /QIS hat den Termin verschoben jetzt Di 15:30 · deine Änderung hing an Di 15:30 Übernehmen Verwerfen$/.test(row)), `moved: ${JSON.stringify(four)}`);
  check(four.some((row) => /den Termin gibt es nicht mehr .* Als eigenen Termin behalten Verwerfen$/.test(row)), `gone: ${JSON.stringify(four)}`);
  if (shots) await page.screenshot({ path: `${shots}/change-open.png` });
  // „OK": done, its line pale in its place; „Übernehmen": the change hangs on the Termin QIS has now.
  const place = () => page.evaluate(() => allOnPage(".sp-side .sp-open-row").map((row) => ({ text: row.innerText.replace(/\s+/g, " ").trim(), done: row.classList.contains("done"), y: Math.round(row.getBoundingClientRect().top) })));
  const okBefore = (await place()).find((row) => /QIS sagt jetzt dasselbe wie du/.test(row.text));
  await page.click('.sp-side .sp-open-row.chg [data-action="change-ok"]');
  await until(page, "„OK“", ([plan, key]) => !(localStorage.getItem(plan) ?? "").includes(`\t${key}\t`), [PLAN, tuesday]);
  await until(page, "the settled line pale", () => allOnPage(".sp-side .sp-open-row.done").length === 1);
  const okAfter = (await place()).find((row) => /QIS sagt jetzt dasselbe wie du/.test(row.text));
  check(okAfter?.done && okAfter.text === "VL EvS: QIS sagt jetzt dasselbe wie du erledigt" && okAfter.y === okBefore?.y, `the settled line: ${JSON.stringify({ okBefore, okAfter })}`);
  await page.click('.sp-side .sp-open-row.chg [data-action="change-take-new"]');
  await until(page, "„Übernehmen“", ([plan]) => {
    const text = localStorage.getItem(plan) ?? "";
    return !text.includes("150132-00001") && text.split("\n").some((line) => /^x\t0\t2026W\t150132-[0-9a-f]{5}\tall\t\t\t\tHG\/2\.46\t2\t15:30-17:00\tHG\/2\.45\t11112$/.test(line));
  }, [PLAN]);
  // „Als eigenen Termin behalten": a Termin of one's own in its stead, with „Rückgängig".
  await page.click('.sp-side .sp-open-row.chg [data-action="change-keep-own"]');
  await until(page, "„Als eigenen Termin behalten“", ([plan]) => {
    const text = localStorage.getItem(plan) ?? "";
    return !text.includes("999999-00001") && text.split("\n").some((line) => line.startsWith("t\t") && line.includes("\tZHG/HS.A\t"));
  }, [PLAN]);
  // The one QIS says something else of leads to its Termin, where „Meine behalten" settles it.
  await page.click(`.sp-side a.sp-open-row.chg[href*="termin=${friday}"]`);
  await until(page, "its Termin, comparing", (dialog) => Boolean(onPage(`${dialog} .tm-differs`)), dialog);
  const cmp = await page.evaluate((dialog) => [...(onPage(`${dialog} .tm-cmp`)?.children ?? [])].map((part) => [...part.children].map((el) => el.textContent.trim()).join(" ")), dialog);
  check(JSON.stringify(cmp) === JSON.stringify(["QIS jetzt ZHG/HS.C", "Deine Änderung ZHG/HS.A"]), `the comparison: ${JSON.stringify(cmp)}`);
  await page.click(`${dialog} [data-action="change-keep"]`);
  await until(page, "„Meine behalten“", ([plan, key]) => (localStorage.getItem(plan) ?? "").includes(`x\t0\t2026W\t${key}\tall\t\t\t\tZHG/HS.A\t5\t09:15-10:45\tZHG/HS.C\t12104\n`) && !allOnPage(".sp-side .sp-open-row.chg").length, [PLAN, friday]);
  // All four settled: pale while editing; the mode left, the quiet line says the choices alone.
  check(await page.evaluate(() => allOnPage(".sp-side .sp-open-row.done").length) === 4, `the settled lines while editing: ${JSON.stringify(await place())}`);
  await until(page, "the page at rest", () => !document.querySelector(".pending-page.held"));
  await page.keyboard.press("Escape");
  await until(page, "the card closed", () => !document.querySelector("dialog[open]") && !location.search.includes("termin="));
  await page.click(".sp-editbar .sp-done");
  await until(page, "the mode left", ([]) => !onPage(".sp-editbar") && onPage(".sp-status .sp-open-way")?.textContent.replace(/\s+/g, " ").trim() === "3 Wahlen offen", []);
  await context.close();
}

// ---------- a phone ----------
{
  // A change QIS says something else of now, as under „Offen".
  const differs = `x\t0\t2026W\t${friday}\tall\t\t\t\tZHG/HS.A\t5\t09:15-10:45\tZHG/HS.B\t12104\n`;
  const { page, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, FS1 + differs, `/studyplan?termin=${friday}`);
  await until(page, "the Termin's sheet", (dialog) => Boolean(onPage(`${dialog} .tk`)), dialog);
  // In the sheet each answer a row of its own across it, the one suggested first (owner, 2026-11-17:
  // „Beim modal sollte das unbedingt jeweils eine Zeile sein").
  const rows = (selector) => page.evaluate((selector) => {
    const box = onPage(selector);
    if (!box) return null;
    const width = Math.round(box.getBoundingClientRect().width);
    return { width, rows: [...box.querySelectorAll(":scope > button")].map((button) => { const r = button.getBoundingClientRect(); return { text: button.textContent.trim(), top: Math.round(r.top), bottom: Math.round(r.bottom), width: Math.round(r.width), height: Math.round(r.height) }; }).sort((a, b) => a.top - b.top) };
  }, selector);
  const stacked = (got, first) => Boolean(got) && got.rows.length === 2 && got.rows[0].text === first && got.rows.every((row) => row.width >= got.width - 1 && row.height >= 44) && got.rows[1].top >= got.rows[0].bottom;
  const answers = await rows(`${dialog} .tm-cmp-acts`);
  check(stacked(answers, "Meine behalten"), `the Termin QIS says something else of: not each answer a row of its own, „Meine behalten“ first: ${JSON.stringify(answers)}`);
  await page.click(`${dialog} [data-action="change"]`);
  await until(page, "„Ändern“ in the sheet", (dialog) => Boolean(onPage(`${dialog} #tm-change`)), dialog);
  const wider = await page.evaluate((dialog) => allOnPage(`${dialog} #tm-change *`).filter((el) => el.getBoundingClientRect().right > innerWidth + 1).map((el) => el.className || el.tagName), dialog);
  check(wider.length === 0, `nothing wider than the screen: ${JSON.stringify(wider.slice(0, 5))}`);
  const form = await rows(`${dialog} #tm-change .chg-foot`);
  check(stacked(form, "Speichern"), `„Ändern“ in the sheet: not each button a row of its own, „Speichern“ first: ${JSON.stringify(form)}`);
  if (shots) await page.screenshot({ path: `${shots}/change-phone.png` });
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log("change: ok");
