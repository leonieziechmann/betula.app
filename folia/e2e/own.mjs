// Termine of one's own, step 1 of folia/design/kalender/termine.html (owner, 2026-10-27: „Passt
// alles, bau Schritt 1 jetzt in Folia"; docs/folia/frontend.md, „Termine of one's own"), on canary's
// data of 2026W: Informatik B.Sc.'s first semester planned (EvS, EEG, PP, MIT1).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node own.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// - „+ Termin" in the week's head: the form („Neuer Termin", „In „Mein Studiengang“, WiSe
//   2026/27"), „Wann" above the day, the draft dashed in the week where it will stand, „Passt in
//   deine Woche"; „Anlegen": a `t` line, the slot of one's own with its mark, the head's count, the
//   list „Eigene Termine", the note with „Rückgängig".
// - One that meets the Friday lecture: „Trifft VL EvS (Fr 09:15). Anlegen geht trotzdem.", then
//   both slots red.
// - A click on a free stretch of a day: the form with that day and time.
// - Its view: „Eigener Termin", no „Teilen", „Im Stundenplan · nicht in Datei und Abo", „Jede
//   Woche · …, die Vorlesungszeit"; „Nur am … ausblenden" skips the day, „Bearbeiten" changes its
//   place (keys typed keep the field and the title), „Löschen" closes the view with „Rückgängig".
// - A module without Termine (Praktikum Betriebssysteme): „Termin eintragen", the form with the
//   module chosen, the slot in the module's tone; without a name it goes by the module („PBS" in
//   the week, the list and the note; its view with the module's title, „Von dir angelegt").
// - Termine of one's own alone, no module: the week has them, not „Noch keine Termine", and so
//   has „Termine".
// - Lena's calendar: „+ Termin" under its modules, its line of calendar 1, „Zählt für „Gemeinsam
//   frei“ mit Lena".
// - A phone: the round button over the bar opens the form as a sheet; a finger held on a free
//   stretch of the week opens it for that day.
// - The start page: a Termin of one's own of today in the day, its view leading to the Stundenplan.
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
const LENA = "l\t1\t\t013-82-2018\tLena\n" + ["11109", "13977"].map((id) => `n\t1\t2026W\t${id}`).join("\n") + "\n";
const PRAK_BS = "12340";

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
const own = (page) => page.evaluate((key) => (localStorage.getItem(key) ?? "").split("\n").filter((line) => line.startsWith("t\t")), PLAN);
const text = (page, selector) => page.evaluate((selector) => onPage(selector)?.innerText.replace(/\s+/g, " ").trim() ?? null, selector);
const form = "dialog.own-form[open]";
const wide = { viewport: { width: 1440, height: 900 } };
// The form's fields, as a hand fills them.
const fill = async (page, { name, day, from, to, place }) => {
  if (name !== undefined) await page.fill(`${form} #own-what`, name);
  if (day !== undefined) await page.selectOption(`${form} #own-day`, String(day));
  if (from !== undefined) await page.fill(`${form} .own-two input[type="time"] >> nth=0`, from);
  if (to !== undefined) await page.fill(`${form} .own-two input[type="time"] >> nth=1`, to);
  if (place !== undefined) await page.fill(`${form} #own-where`, place);
};
// The point of a day's column at `minute` of the week grid shown (of a phone's carousel, the week on
// the screen), scrolled into the middle of the screen when it is not on it.
const spot = (page, day, minute) => page.evaluate(([day, minute]) => {
  const weeks = allOnPage(".sp-week .week");
  const week = weeks.find((week) => {
    const rect = week.getBoundingClientRect();
    return rect.left >= -1 && rect.right <= innerWidth + 1;
  }) ?? weeks[0];
  const col = week?.querySelectorAll(".col")[day - 1];
  if (!col) return null;
  const [first, last] = [Number(week.dataset.first), Number(week.dataset.last)];
  const y = () => {
    const rect = col.getBoundingClientRect();
    return rect.top + rect.height * ((minute - first) / (last - first)) + 2;
  };
  if (y() < 120 || y() > innerHeight - 160) window.scrollBy(0, y() - innerHeight / 2);
  const rect = col.getBoundingClientRect();
  return { x: rect.left + rect.width * 0.5, y: y() };
}, [day, minute]);

// ---------- „+ Termin", the form, the week ----------
{
  const { page, context } = await open(wide, FS1);
  await until(page, "the week", () => allOnPage(".sp-week .slot").length > 0 && Boolean(onPage(".sp-head .sp-own-head")));
  const clashes = await page.evaluate(() => allOnPage(".sp-week .slot.clash").length);
  await page.click(".sp-head .sp-own-head");
  await until(page, "the form", (form) => Boolean(onPage(form)) && onPage(`${form} #own-form-title`)?.textContent === "Neuer Termin", form);
  check((await text(page, `${form} .st-dlg-head p`)) === "In „Mein Studiengang“, WiSe 2026/27", `the form's line: ${await text(page, `${form} .st-dlg-head p`)}`);
  const order = await page.evaluate((form) => [...onPage(`${form} .own-body`).children].map((field) => field.querySelector("label, .own-lb")?.textContent ?? field.className), form);
  check(JSON.stringify(order.slice(0, 4)) === JSON.stringify(["Was", "Wann", "Tag", "Zeit"]), `„Wann“ above what it changes: ${JSON.stringify(order)}`);
  await fill(page, { name: "Arbeit", day: 4, from: "13:00", to: "17:00", place: "Café Mosaik" });
  await until(page, "the draft in the week, and that it fits", (form) => {
    const draft = allOnPage(".sp-week .slot.draft");
    const cols = [...(onPage(".sp-week .week")?.querySelectorAll(".col") ?? [])];
    return draft.length === 1 && draft[0].textContent.includes("Arbeit") && cols.indexOf(draft[0].closest(".col")) === 3 && onPage(`${form} .own-fit.ok`)?.textContent === "Passt in deine Woche";
  }, form);
  if (shots) await page.screenshot({ path: `${shots}/own-form.png` });
  await page.click(`dialog.own-form .st-dlg-foot .primary`);
  await until(page, "„Anlegen“: the line, the slot, the head, the list, the note", ([key]) => {
    const text = localStorage.getItem(key) ?? "";
    const slot = allOnPage(".sp-week .slot.own:not(.draft)").find((slot) => slot.textContent.includes("Arbeit"));
    return text.includes("t\t1\t0\t2026W\tw\t4\t13:00-17:00\t\t\t\tArbeit\tCafé Mosaik\t\n") && !onPage("dialog.own-form[open]")
      && slot?.querySelector(".slot-own") && slot.classList.contains("t-own") && !allOnPage(".sp-week .slot.draft").length
      && onPage(".sp-sum")?.textContent.endsWith("1 eigener Termin")
      && onPage(".sp-own .sp-row.own")?.innerText.replace(/\s+/g, " ").includes("Arbeit Do 13:00 Café Mosaik")
      && onPage(".own-note")?.textContent.includes("„Arbeit“ angelegt");
  }, [PLAN]);
  check(await page.evaluate(() => allOnPage(".sp-week .slot.own .slot-acts").length === 0), "a slot of one's own has buttons");

  // One that meets the Friday lecture: the form says so, and both are red.
  await page.click(".sp-head .sp-own-head");
  await until(page, "the form again", (form) => Boolean(onPage(form)), form);
  await fill(page, { name: "HiWi", day: 5, from: "09:00", to: "10:00" });
  await until(page, "„Trifft VL EvS“", (form) => onPage(`${form} .own-fit.warn`)?.textContent === "Trifft VL EvS (Fr 09:15). Anlegen geht trotzdem.", form);
  await page.click(`dialog.own-form .st-dlg-foot .primary`);
  await until(page, "both red", () => {
    const slots = allOnPage(".sp-week .slot.clash");
    return slots.some((slot) => slot.textContent.includes("HiWi")) && slots.some((slot) => slot.textContent.includes("EvS") && !slot.classList.contains("own"));
  });
  check((await page.evaluate(() => allOnPage(".sp-week .slot.clash").length)) === clashes + 2, "the clash counted otherwise");

  // A click on a free stretch of Wednesday at 16:00: the form with that day and time.
  const at = await spot(page, 3, 16 * 60);
  await page.mouse.click(at.x, at.y);
  await until(page, "the form of the free stretch", (form) => onPage(`${form} #own-day`)?.value === "3" && onPage(`${form} .own-two input`)?.value === "16:00" && onPage(`${form} #own-form-title`)?.textContent === "Neuer Termin", form);
  await page.keyboard.press("Escape");
  await until(page, "Esc closes the form", () => !onPage("dialog.own-form[open]"));

  // Its view: what it is and what it changes.
  await page.click('.sp-week .slot.own:has-text("Arbeit")');
  await until(page, "its view", () => onPage(".is-termin .tm-title")?.textContent === "Arbeit" && Boolean(onPage(".is-termin .tm-sec.own")));
  const view = await page.evaluate(() => ({
    chip: onPage(".is-termin .tm-mod")?.textContent,
    share: Boolean(onPage(".is-termin .tm-share")),
    head: onPage(".is-termin .tm-sec.own > h4")?.innerText.replace(/\s+/g, " "),
    rows: allOnPage(".is-termin .tm-sec.own .crow").map((row) => row.innerText.replace(/\s+/g, " ").trim()),
  }));
  check(view.chip === "Eigener Termin" && !view.share, `its head: ${JSON.stringify(view)}`);
  check(/^Im Stundenplan ?nicht in Datei und Abo$/i.test(view.head ?? ""), `its section's head: ${view.head}`);
  check(/^Jede Woche \d\d\.\d\d\.–\d\d\.\d\d\., die Vorlesungszeit Bearbeiten$/.test(view.rows[0] ?? "") && view.rows[1]?.startsWith("Nur am Do ") && view.rows[2] === "Löschen mit „Rückgängig“ Löschen", `its rows: ${JSON.stringify(view.rows)}`);
  if (shots) await page.screenshot({ path: `${shots}/own-view.png` });
  // „Nur am … ausblenden" skips the day.
  await page.click('.is-termin .tm-sec.own [data-action="hide-date"]');
  await until(page, "the day skipped", ([key]) => /^t\t1\t0\t2026W\tw\t4\t13:00-17:00\t\t\t\tArbeit\tCafé Mosaik\t\d{4}-\d\d-\d\d$/m.test(localStorage.getItem(key) ?? "") && onPage('.is-termin .tm-sec.own [data-action="hide-date"]')?.getAttribute("aria-pressed") === "true", [PLAN]);
  await page.click('.is-termin .tm-sec.own [data-action="hide-date"]');
  await until(page, "the day held again", ([key]) => (localStorage.getItem(key) ?? "").includes("t\t1\t0\t2026W\tw\t4\t13:00-17:00\t\t\t\tArbeit\tCafé Mosaik\t\n"), [PLAN]);
  // „Bearbeiten": the form with its fields, „Speichern".
  await page.click('.is-termin [data-action="own-edit"]');
  await until(page, "its form", (form) => onPage(`${form} #own-form-title`)?.textContent === "„Arbeit“ bearbeiten" && onPage(`${form} #own-where`)?.value === "Café Mosaik" && onPage(`${form} #own-day`)?.value === "4", form);
  // Keys typed one by one keep the field and the title: the form is built once as it opens.
  await page.click(`${form} #own-what`);
  await page.keyboard.press("End");
  await page.keyboard.type("!", { delay: 30 });
  check(await page.evaluate((form) => document.activeElement?.id === "own-what" && onPage(`${form} #own-form-title`)?.textContent === "„Arbeit“ bearbeiten" && onPage(`${form} #own-what`)?.value === "Arbeit!", form), "a key typed moved the focus or the title");
  await page.keyboard.press("Backspace");
  await fill(page, { place: "Mensa" });
  await page.click(`dialog.own-form .st-dlg-foot .primary`);
  await until(page, "„Speichern“", ([key]) => (localStorage.getItem(key) ?? "").includes("t\t1\t0\t2026W\tw\t4\t13:00-17:00\t\t\t\tArbeit\tMensa\t\n") && onPage(".own-note")?.textContent.includes("„Arbeit“ gespeichert"), [PLAN]);
  // „Löschen": the view closes, the note brings it back.
  await page.click('.is-termin [data-action="own-remove"]');
  await until(page, "„Löschen“", ([key]) => !(localStorage.getItem(key) ?? "").includes("\tArbeit\t") && !location.search.includes("termin=") && onPage(".own-note")?.textContent.includes("„Arbeit“ gelöscht"), [PLAN]);
  await page.click(".own-note .plan-toast-undo");
  await until(page, "„Rückgängig“", ([key]) => (localStorage.getItem(key) ?? "").includes("\tArbeit\tMensa\t") && allOnPage(".sp-week .slot.own").some((slot) => slot.textContent.includes("Arbeit")), [PLAN]);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- a module without Termine: „Termin eintragen" ----------
{
  const { page, context } = await open(wide, FS1 + `m\t2026W\t${PRAK_BS}\t1790000000\t\n`);
  await until(page, "„Termin eintragen“", () => Boolean(onPage(".sp-list .sp-row-wrap.has-enter .sp-enter")));
  await page.click(".sp-list .sp-row-wrap.has-enter .sp-enter");
  await until(page, "the form with the module", ([form, id]) => onPage(`${form} #own-module`)?.value === id, [form, PRAK_BS]);
  await fill(page, { day: 4, from: "13:45", to: "15:15" });
  await page.click(`dialog.own-form .st-dlg-foot .primary`);
  // Without a name it goes by its module: „PBS" in the week, the list and the note.
  await until(page, "the module's Termin in its tone, by its module's name", ([key, id]) => {
    const line = (localStorage.getItem(key) ?? "").split("\n").find((line) => line.startsWith("t\t"));
    const slot = allOnPage(".sp-week .slot.own").find((slot) => slot.title.includes("von dir"));
    return line === `t\t1\t0\t2026W\tw\t4\t13:45-15:15\t\t\t${id}\t\t\t` && slot && !slot.classList.contains("t-own") && /^PBS/.test(slot.textContent)
      && onPage(".sp-own .sp-row.own")?.innerText.replace(/\s+/g, " ").startsWith("PBS Do 13:45") && onPage(".own-note")?.textContent.includes("„PBS“ angelegt");
  }, [PLAN, PRAK_BS]);
  if (shots) await page.screenshot({ path: `${shots}/own-module.png` });
  // Its view: the module's chip and title, its source.
  await page.click('.sp-week .slot.own:has-text("PBS")');
  await until(page, "its view by its module", () => onPage(".is-termin .tm-title")?.textContent === "Praktikum Betriebssysteme" && onPage(".is-termin .tm-mod")?.textContent === "PBS" && /^Von dir angelegt$/.test(onPage(".is-termin .tm-src")?.innerText.trim() ?? ""));
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- Termine of one's own alone: no „Noch keine Termine" ----------
{
  const { page, context } = await open(wide, "t\t1\t0\t2026W\tw\t2\t17:00-19:00\t\t\t\tChor\tAula\t\n");
  await until(page, "the week of one's own alone", () => allOnPage(".sp-week .slot.own").some((slot) => slot.textContent.includes("Chor")) && !onPage(".sp-empty") && onPage(".sp-sum")?.textContent.endsWith("1 eigener Termin"));
  await page.goto(base + "/studyplan?view=dates", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await until(page, "„Termine“ lists it with its mark", () => allOnPage(".agenda-item.own").some((item) => item.textContent.includes("Chor") && item.querySelector(".ag-own") && item.title.includes("von dir")) && !onPage(".sp-empty"));
  await page.click('.agenda-item.own:has-text("Chor") >> nth=0');
  await until(page, "its view from „Termine“", () => onPage(".is-termin .tm-title")?.textContent === "Chor");
  await context.close();
}

// ---------- Lena's calendar ----------
{
  const { page, context } = await open(wide, FS1 + LENA);
  await until(page, "Lena's part", () => Boolean(onPage(".sp-cal-mods .sp-own-add")));
  await page.click(".sp-cal-mods .sp-own-add");
  await until(page, "the form for Lena", (form) => onPage(`${form} #own-cal`)?.value === "1" && onPage(`${form} .st-dlg-head p`)?.textContent === "In „Lena“, WiSe 2026/27", form);
  // The line under the title follows „Kalender".
  await page.selectOption(`${form} #own-cal`, "0");
  await until(page, "the line follows „Kalender“", (form) => onPage(`${form} .st-dlg-head p`)?.textContent === "In „Mein Studiengang“, WiSe 2026/27", form);
  await page.selectOption(`${form} #own-cal`, "1");
  await fill(page, { name: "Arbeit", day: 3, from: "13:00", to: "17:00" });
  await until(page, "what it counts for", (form) => onPage(`${form} .own-fit.ok`)?.textContent === "Zählt für „Gemeinsam frei“ mit Lena", form);
  await page.click(`dialog.own-form .st-dlg-foot .primary`);
  await until(page, "in Lena's calendar", ([key]) => (localStorage.getItem(key) ?? "").includes("t\t1\t1\t2026W\tw\t3\t13:00-17:00\t\t\t\tArbeit\t\t\n") && onPage(".sp-cal-mods .sp-own button.sp-row.own")?.textContent.includes("Arbeit"), [PLAN]);
  check(!(await page.evaluate(() => onPage(".sp-list .sp-own .sp-row.own"))), "Lena's Termin in „Mein Studiengang“'s list");
  // Its row opens its form.
  await page.click(".sp-cal-mods .sp-own button.sp-row.own");
  await until(page, "its form from the row", (form) => onPage(`${form} #own-form-title`)?.textContent === "„Arbeit“ bearbeiten", form);
  await page.keyboard.press("Escape");
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- a phone: the round button, a finger held ----------
{
  const { page, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, FS1);
  await until(page, "the round button", () => {
    const fab = onPage(".sp-own-fab")?.getBoundingClientRect();
    return fab && fab.bottom <= innerHeight && fab.right <= innerWidth && fab.width > 40;
  });
  await page.tap(".sp-own-fab");
  await until(page, "the form as a sheet", (form) => Boolean(onPage(form)), form);
  if (shots) await page.screenshot({ path: `${shots}/own-phone-form.png` });
  await page.keyboard.press("Escape");
  await until(page, "the sheet closed", () => !onPage("dialog.own-form[open]"));
  // A finger held on Wednesday at 16:00.
  const at = await spot(page, 3, 16 * 60);
  const target = await page.evaluate(([x, y]) => document.elementFromPoint(x, y)?.className ?? "", [at.x, at.y]);
  await page.evaluate(([x, y]) => {
    const element = document.elementFromPoint(x, y);
    element.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, pointerType: "touch", clientX: x, clientY: y, isPrimary: true }));
  }, [at.x, at.y]);
  await page.waitForTimeout(700);
  await page.evaluate(([x, y]) => {
    const element = document.elementFromPoint(x, y);
    element?.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerType: "touch", clientX: x, clientY: y, isPrimary: true }));
  }, [at.x, at.y]);
  await until(page, `a finger held opens the form for Wednesday (on ${target})`, (form) => onPage(`${form} #own-day`)?.value === "3" && onPage(`${form} .own-two input`)?.value === "16:00", form);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- the start page: today's Termin of one's own ----------
{
  const now = new Date();
  const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  const line = `t\t1\t0\t2026W\td\t${today}\t23:00-23:30\t\t\t\tAbendlauf\tPark\t\n`;
  const { page, context } = await open(wide, FS1 + line, "/");
  await until(page, "today's Termin of one's own", () => allOnPage(".day-ev").some((entry) => entry.textContent.includes("Abendlauf")));
  await page.click('.day-ev:has-text("Abendlauf")');
  await until(page, "its view over the start page", () => onPage(".is-termin .tm-title")?.textContent === "Abendlauf" && Boolean(onPage(".is-termin .tm-sec.own a.sw")));
  check((await page.getAttribute(".is-termin .tm-sec.own a.sw", "href"))?.includes("/studyplan?") && !(await page.evaluate(() => onPage('.is-termin [data-action="own-remove"]'))), "from the start page it leads to the Stundenplan");
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(`own: ${problems.length} problem(s)\n- ${problems.join("\n- ")}`);
  process.exit(1);
}
console.log("own: ok");
