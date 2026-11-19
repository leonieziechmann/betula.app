// Termine of one's own, step 1 of folia/design/kalender/termine.html (owner, 2026-10-27: „Passt
// alles, bau Schritt 1 jetzt in Folia"; docs/folia/frontend.md, „Termine of one's own"), with the
// calendar's two states (owner, 2026-11-17: „Ganz wichtig ist auch, dass man zu Modulen custom termine
// hinzufügen und auch editieren kann"), on canary's data of 2026W: Informatik B.Sc.'s first semester
// planned (EvS, EEG, PP, MIT1).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node own.mjs [--only=form,card,module,kind,alone,lena,phone,finger,start]
// - While looking nothing makes a Termin: no „+ Termin", a click on a free stretch opens nothing.
//   „Bearbeiten": the bar's „+ Termin" opens the form („Neuer Termin", „In „Mein Studiengang“, WiSe
//   2026/27"; „Was", „Gehört zu", „Art" above „Wann", „Tag", „Zeit"), the draft dashed in the week where
//   it will stand, „Passt in deine Woche"; „Anlegen": a `t` line, the slot of one's own with its mark
//   and, while editing, the bin alone (no eye), the list „Eigene Termine", the note with „Rückgängig".
// - One that meets the Friday lecture: „Trifft VL EvS (Fr 09:15). Anlegen geht trotzdem.", then both
//   slots red. A click on a free stretch while editing: the form with that day and time; Esc closes the
//   form and the mode stays. A drag on a free stretch draws its span in quarters of an hour over its
//   column (no slot changes its width), its times in a tag above it, in its day and the grid's hours,
//   upward as downward, a first move past the grid's end held, the minutes following a wheel's
//   scroll; it opens the form with it, which says no hint („Schneller: …" stands only where a button
//   opened it). Every way a drag ends without its up — Esc, a mouse let go elsewhere, lostpointercapture,
//   pointercancel, a second pointer, the window's blur, the page turning a phone's — leaves no span, no
//   hold of the pointer, nothing open, and the next click on a free stretch opens the form. The
//   slot's bin deletes it, „Rückgängig" brings it back. „Fertig": the sum counts them, the slots carry
//   no button.
// - Its card (C.3 e): „Eigener Termin" and its name, „Jede Woche · …, die Vorlesungszeit", „nicht in
//   Datei und festem Abo", the tools pencil „Bearbeiten" · eye „Nur am … ausblenden" · bin „Löschen",
//   no „⋯", no „Teilen". The eye skips the day and closes the card with its note and „Rückgängig"; on
//   that day the eye is crossed and „Am … einblenden" brings it back in the card; the pencil opens the
//   form (keys typed keep the field and the title), „Speichern" with its note; the bin closes the card
//   with „„Arbeit“ gelöscht" and „Rückgängig".
// - A module without Termine (Praktikum Betriebssysteme), while looking: „Termin eintragen", the form
//   with the module chosen, the slot in the module's tone; without a name it goes by the module („PBS"
//   in the week and the note, a row under its module in the list); its card with the module's title
//   and „von dir".
// - A Termin of a module with a kind: „+ Termin" under EvS while editing („Für EvS · WiSe 2026/27"),
//   „Art" Vorlesung: the slot „VL EvS", the `t` line's kind, the row „Vorlesung" under EvS, the card
//   „EvS Vorlesung · von dir"; „Art" back to „Termin": the slot „EvS", the line without its kind.
// - Termine of one's own alone, no module: the week has them, not „Noch keine Termine", and so has
//   „Termine".
// - Lena's calendar, while editing: „+ Termin" under its modules, its line of calendar 1, „Zählt für
//   „Gemeinsam frei“ mit Lena"; while looking it has no „+ Termin".
// - A phone: no round button and nothing for a held finger while looking; „Bearbeiten": the dock's
//   „+ Termin" opens the form as a sheet with the hint „Schneller: in der Woche gedrückt halten und
//   ziehen.", a finger held on a free stretch opens it for that day; a real finger as the owner defined
//   it (2026-11-18): the vibration as the hold completes and not before, 90 minutes standing, no
//   browser's menu, nothing open or stored before the lift; moved, it stretches while the page holds
//   still, its times in a tag; lifted, the form with exactly that, a lift without a move 90 minutes; a
//   pointercancel gives it up; in the last column the tag stays on the screen and off the finger; a
//   finger that moves at once scrolls.
// - A real finger (owner, 2026-11-18: „Ja das dragging funktioniert nicht dass ist richtig buggy."),
//   on a phone's carousel while editing: held and trembling 9 px and back, 90 minutes stand and the
//   lift gives them; held and lifted with a shift of 12 px within 30 ms, 90 minutes; drawn to 17:15,
//   rested 300 ms and lifted with a shift of 13 px, 17:15; a steady drag lifted in motion, what its
//   tag said last; resting 1 px past the line of 17:15 and trembling ±2 px, the end does not flip
//   (come from below) or steps once and never back (come from above); drifting 6 or 8 px sideways
//   while held, the hold completes, no week is carried and the lift gives 90 minutes; drifting 9 or
//   12 px, no hold, the week carried and back, no span, nothing open; nothing stored.
// - The start page: a Termin of one's own of today in the day, its card leading to the Stundenplan.
// History: 2026-11-17, the edit mode. The ways in are the mode's (the head's „+ Termin", the round
// button of a phone and a free stretch while looking went); the form has „Art" and puts „Gehört zu"
// first; the view of a Termin of one's own is its card (its rows „Im Stundenplan" went into the tools);
// a module's Termin is a row under its module, not under „Eigene Termine"; a name the student did not
// write is said without quotes („PBS angelegt"). 2026-11-18: a span drawn by a drag (owner: „drag
// placement um gleich eine Zeitspanne zu machen"); a finger held draws it before the form opens; the
// checks' finger stood still to the pixel and lifted without moving, and a real one, which trembles,
// drifts and shifts as it lifts, lost or changed the span (Folia 1.107.1).
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
const only = (process.argv.find((arg) => arg.startsWith("--only=")) ?? "").slice("--only=".length);
const block = (name) => !only || only.split(",").includes(name);
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
const FS1 = ["12104", "12107", "12102", "11112"].map((id) => `m\t2026W\t${id}\t1790000000\t`).join("\n") + "\n";
const INFORMATIK = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n";
const LENA = "l\t1\t\t013-82-2018\tLena\n" + ["11109", "13977"].map((id) => `n\t1\t2026W\t${id}`).join("\n") + "\n";
// A second calendar of one's own, to go to from Lena's.
const WG = "l\t2\t\t013-82-2018\tWG\nn\t2\t2026W\t13977\n";
const PRAK_BS = "12340";
const ARBEIT = "t\t1\t0\t2026W\tw\t4\t13:00-17:00\t\t\t\tArbeit\tCafé Mosaik\t\n";

const open = async (options, plan, path = "/studyplan") => {
  const context = await browser.newContext(options);
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  // A phone that writes down when it vibrates (as haptics.mjs has it).
  await context.addInitScript(() => {
    window.__felt = [];
    Object.defineProperty(Navigator.prototype, "vibrate", { configurable: true, value(pattern) { window.__felt.push(performance.now()); return true; } });
  });
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
    return true;
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
    return false;
  }
};
// A click that may find nothing: said, not thrown.
const click = async (page, selector, name = selector) => {
  try {
    await page.click(selector, { timeout: 5000 });
    return true;
  } catch {
    problems.push(`${name}: nothing to click (${page.url()})`);
    return false;
  }
};
const own = (page) => page.evaluate((key) => (localStorage.getItem(key) ?? "").split("\n").filter((line) => line.startsWith("t\t")), PLAN);
const text = (page, selector) => page.evaluate((selector) => onPage(selector)?.innerText.replace(/\s+/g, " ").trim() ?? null, selector);
const note = (page) => page.evaluate(() => onPage(".own-note .plan-toast-said")?.textContent.replace(/\s+/g, " ").trim() ?? "");
const form = "dialog.own-form[open]";
// Betula's select (`combobox::Select`, since 2026-11-19 in the Termin's forms): its button, then
// the entry of that value in the list it opens, as a hand picks it.
const pick = async (page, button, value) => {
  await page.click(button);
  await page.click(`${button} ~ .combo-pop [role="option"][data-value="${value}"]`);
};
const wide = { viewport: { width: 1440, height: 900 } };
// The form's fields, as a hand fills them.
const fill = async (page, { name, day, from, to, place }) => {
  if (name !== undefined) await page.fill(`${form} #own-what`, name);
  if (day !== undefined) await pick(page, `${form} #own-day`, String(day));
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
// A finger held on a point (a phone).
const hold = async (page, at) => {
  await page.evaluate(([x, y]) => {
    document.elementFromPoint(x, y)?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, pointerType: "touch", clientX: x, clientY: y, isPrimary: true }));
  }, [at.x, at.y]);
  await page.waitForTimeout(700);
  await page.evaluate(([x, y]) => {
    document.elementFromPoint(x, y)?.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerType: "touch", clientX: x, clientY: y, isPrimary: true }));
  }, [at.x, at.y]);
};
// The slot of `name` in the week and the buttons beside it (the `.slot-acts` after it), by their sign.
const slotOf = (page, name) => page.evaluate((name) => {
  const slot = allOnPage(".sp-week a.slot.own:not(.draft)").find((slot) => slot.textContent.includes(name));
  if (!slot) return null;
  const acts = slot.nextElementSibling?.matches(".slot-acts") ? [...slot.nextElementSibling.querySelectorAll(".slot-act")] : [];
  return { href: slot.getAttribute("href"), classes: slot.className, label: slot.querySelector(".l")?.textContent.trim(), acts: acts.map((act) => `${act.dataset.icon} ${act.getAttribute("aria-label")}`) };
}, name);
// The card of a Termin of one's own as it reads.
const card = (page) => page.evaluate(() => {
  const panel = onPage("dialog[open] .tm-dialog.tm-card");
  const text = (selector) => panel?.querySelector(selector)?.textContent.replace(/\s+/g, " ").trim() ?? "";
  return {
    open: Boolean(panel),
    chip: text(".tm-mod"),
    kind: text(".tm-kind"),
    mark: text(".tm-own"),
    title: text(".tm-title"),
    when: text(".tm-when"),
    tools: [...(panel?.querySelectorAll(".tm-tools [data-action]") ?? [])].map((tool) => `${tool.dataset.action} ${tool.querySelector("use")?.getAttribute("href")?.split("#")[1] ?? ""}`),
    toolLabels: [...(panel?.querySelectorAll(".tm-tools [data-action]") ?? [])].map((tool) => tool.getAttribute("aria-label")),
    share: Boolean(panel?.querySelector(".tm-share")),
    feed: text(".tm-own-feed"),
    date: text(".tk-date > b"),
    way: panel?.querySelector(".tm-ways a.tm-way")?.getAttribute("href") ?? "",
  };
});
const visible = (page, selector) => page.evaluate((selector) => allOnPage(selector).filter((el) => el.getClientRects().length).length, selector);

// ---------- the mode's „+ Termin", the form, the week ----------
if (block("form")) {
  const { page, context } = await open(wide, FS1);
  await until(page, "the week", () => allOnPage(".sp-week .slot").length > 0 && Boolean(onPage("#sp-edit-toggle")));
  // Looking: no „+ Termin", and a free stretch is no way in.
  check(await visible(page, "[data-action='own-add'], .sp-own-head, .sp-editbar") === 0, "while looking there is a „+ Termin\"");
  let at = await spot(page, 3, 16 * 60);
  await page.mouse.click(at.x, at.y);
  await page.waitForTimeout(400);
  check(await page.evaluate(() => !onPage("dialog.own-form[open]")), "a click on a free stretch while looking opened the form");
  await click(page, "#sp-edit-toggle", "„Bearbeiten\"");
  await until(page, "the mode", () => /[?&]edit=1/.test(location.search) && Boolean(onPage(".sp-editbar [data-action='own-add']")));
  const clashes = await page.evaluate(() => allOnPage(".sp-week .slot.clash").length);
  await click(page, ".sp-editbar [data-action='own-add']", "the bar's „+ Termin\"");
  await until(page, "the form", (form) => Boolean(onPage(form)) && onPage(`${form} #own-form-title`)?.textContent === "Neuer Termin", form);
  // Opened by a button, one quiet line by „Wann" says that drawing in the week is quicker.
  check((await text(page, `${form} .own-fld.wide .own-quicker`)) === "Schneller: die Zeitspanne direkt in der Woche ziehen.", `the hint of the gesture: ${await text(page, `${form} .own-quicker`)}`);
  check((await text(page, `${form} .st-dlg-head p`)) === "In „Mein Studiengang“, WiSe 2026/27", `the form's line: ${await text(page, `${form} .st-dlg-head p`)}`);
  const order = await page.evaluate((form) => [...onPage(`${form} .own-body`).children].map((field) => field.querySelector("label, .own-lb")?.textContent ?? field.className), form);
  check(JSON.stringify(order.slice(0, 6)) === JSON.stringify(["Was", "Gehört zu", "Art", "Wann", "Tag", "Zeit"]), `„Gehört zu" and „Art" first, „Wann" above what it changes: ${JSON.stringify(order)}`);
  check(await page.evaluate((form) => onPage(`${form} #own-kind`)?.dataset.value === "" && onPage(`${form} #own-kind .combo-value`)?.textContent === "Termin", form), "„Art\" is „Termin\" to begin with");
  await fill(page, { name: "Arbeit", day: 4, from: "13:00", to: "17:00", place: "Café Mosaik" });
  await until(page, "the draft in the week, and that it fits", (form) => {
    const draft = allOnPage(".sp-week .slot.draft");
    const cols = [...(onPage(".sp-week .week")?.querySelectorAll(".col") ?? [])];
    return draft.length === 1 && draft[0].textContent.includes("Arbeit") && cols.indexOf(draft[0].closest(".col")) === 3 && onPage(`${form} .own-fit.ok`)?.textContent === "Passt in deine Woche";
  }, form);
  if (shots) await page.screenshot({ path: `${shots}/own-form.png` });
  await click(page, `dialog.own-form .st-dlg-foot .primary`, "„Anlegen\"");
  await until(page, "„Anlegen“: the line, the slot, the list, the note", ([key]) => {
    const text = localStorage.getItem(key) ?? "";
    const slot = allOnPage(".sp-week .slot.own:not(.draft)").find((slot) => slot.textContent.includes("Arbeit"));
    return text.includes("t\t1\t0\t2026W\tw\t4\t13:00-17:00\t\t\t\tArbeit\tCafé Mosaik\t\n") && !onPage("dialog.own-form[open]")
      && slot?.querySelector(".slot-own") && slot.classList.contains("t-own") && !allOnPage(".sp-week .slot.draft").length
      && onPage(".sp-own .sp-row.own")?.innerText.replace(/\s+/g, " ").includes("Arbeit Do 13:00 Café Mosaik")
      && onPage(".own-note .plan-toast-said")?.textContent.includes("„Arbeit“ angelegt");
  }, [PLAN]);
  // While editing a slot of one's own carries the bin, and no eye: it is deleted, not hidden.
  check(JSON.stringify((await slotOf(page, "Arbeit"))?.acts) === JSON.stringify(["trash-2 Löschen"]), `the slot's buttons while editing: ${JSON.stringify(await slotOf(page, "Arbeit"))}`);

  // One that meets the Friday lecture: the form says so, and both are red.
  await click(page, ".sp-editbar [data-action='own-add']", "the bar's „+ Termin\"");
  await until(page, "the form again", (form) => Boolean(onPage(form)), form);
  await fill(page, { name: "HiWi", day: 5, from: "09:00", to: "10:00" });
  await until(page, "„Trifft VL EvS“", (form) => onPage(`${form} .own-fit.warn`)?.textContent === "Trifft VL EvS (Fr 09:15). Anlegen geht trotzdem.", form);
  await click(page, `dialog.own-form .st-dlg-foot .primary`, "„Anlegen\"");
  await until(page, "both red", () => {
    const slots = allOnPage(".sp-week .slot.clash");
    return slots.some((slot) => slot.textContent.includes("HiWi")) && slots.some((slot) => slot.textContent.includes("EvS") && !slot.classList.contains("own"));
  });
  check((await page.evaluate(() => allOnPage(".sp-week .slot.clash").length)) === clashes + 2, "the clash counted otherwise");

  // A click on a free stretch of Wednesday at 16:00 while editing: the form with that day and time.
  at = await spot(page, 3, 16 * 60);
  await page.mouse.click(at.x, at.y);
  await until(page, "the form of the free stretch", (form) => onPage(`${form} #own-day`)?.dataset.value === "3" && onPage(`${form} .own-two input`)?.value === "16:00" && onPage(`${form} #own-form-title`)?.textContent === "Neuer Termin", form);
  check(await page.evaluate((form) => !onPage(`${form} .own-quicker`), form), "the form of a free stretch says that drawing is quicker");
  await page.keyboard.press("Escape");
  await until(page, "Esc closes the form, the mode stays", () => !onPage("dialog.own-form[open]") && /[?&]edit=1/.test(location.search) && Boolean(onPage(".sp-editbar")));

  // A drag on a free stretch draws the span in quarters of an hour, its times written in it and in a
  // tag above it; letting go opens the form with them, the draft standing where the span was (owner,
  // 2026-11-18: „drag placement um gleich eine Zeitspanne zu machen"). Wednesday from 16:05 down to 17:20.
  const sketch = () => page.evaluate(() => ({
    span: allOnPage(".sp-week .sp-sketch").map((el) => el.textContent.replace(/\s+/g, " ").trim()).join(),
    tag: allOnPage(".sp-week .sp-sketch-tag").map((el) => el.textContent.trim()).join(),
    drafts: allOnPage(".sp-week .slot.draft").length,
    drawing: Boolean(onPage(".sp-week.drawing")),
    held: Boolean(onPage(".sp-week")?.hasPointerCapture(1)),
  }));
  const none = (s) => !s.span && !s.tag && !s.drafts && !s.drawing && !s.held;
  const [down, low] = [await spot(page, 3, 16 * 60 + 5), await spot(page, 3, 17 * 60 + 20)];
  await page.mouse.move(down.x, down.y);
  await page.mouse.down();
  await page.mouse.move(down.x, low.y, { steps: 8 });
  await until(page, "the span drawn while dragging, its times in a tag above it", () => {
    const span = onPage(".sp-week .sp-sketch"), tag = onPage(".sp-week .sp-sketch-tag");
    if (!span || !tag) return false;
    const [s, t] = [span.getBoundingClientRect(), tag.getBoundingClientRect()];
    return span.textContent.replace(/\s+/g, " ").trim() === "16:00 – 17:30" && tag.textContent.trim() === "16:00–17:30" && t.bottom <= s.top && !allOnPage(".sp-week .slot.draft").length;
  });
  // Out of its column and below the grid: the day stays, the end stops at the grid's last hour.
  await page.mouse.move(down.x - 300, low.y + 2000, { steps: 4 });
  await until(page, "the span keeps its day and the grid's hours", () => {
    const week = onPage(".sp-week .week");
    const last = Number(week?.dataset.last);
    const col = week.querySelectorAll(".col")[2].getBoundingClientRect(), span = onPage(".sp-week .sp-sketch")?.getBoundingClientRect();
    return span && Math.abs(span.left - col.left) < 1.5 && onPage(".sp-week .sp-sketch-tag")?.textContent === `16:00–${String(Math.floor(last / 60)).padStart(2, "0")}:${String(last % 60).padStart(2, "0")}`;
  });
  await page.mouse.move(down.x, low.y, { steps: 4 });
  if (shots) await page.screenshot({ path: `${shots}/own-drag.png` });
  await page.mouse.up();
  await until(page, "the form of the span drawn, the tag gone", (form) => onPage(`${form} #own-day`)?.dataset.value === "3" && allOnPage(`${form} .own-two input`).map((input) => input.value).join() === "16:00,17:30" && allOnPage(".sp-week .slot.draft").length === 1 && !onPage(".sp-sketch, .sp-sketch-tag"), form);
  check(await page.evaluate((form) => !onPage(`${form} .own-quicker`), form), "the form of a span drawn says that drawing is quicker");
  await page.keyboard.press("Escape");
  await until(page, "Esc closes the form of the span", () => !onPage("dialog.own-form[open]") && allOnPage(".sp-week .slot.draft").length === 0 && /[?&]edit=1/.test(location.search));
  // Upward as downward, the quarter pressed in it: from 17:20 up to 16:10 is 16:00–17:30; Esc while
  // drawing gives the span up, and letting go then opens nothing (the mode stays).
  await page.mouse.move(low.x, low.y);
  await page.mouse.down();
  await page.mouse.move(low.x, (await spot(page, 3, 16 * 60 + 10)).y, { steps: 8 });
  await until(page, "the span drawn upward", () => onPage(".sp-week .sp-sketch-tag")?.textContent === "16:00–17:30");
  await page.keyboard.press("Escape");
  await until(page, "Esc gives the span up", () => !onPage(".sp-week .sp-sketch"));
  await page.mouse.up();
  await page.waitForTimeout(400);
  check(await page.evaluate(() => !onPage("dialog.own-form[open]") && /[?&]edit=1/.test(location.search)), "after Esc a drag opened the form or left the mode");

  // A plain click on a free stretch opens the form at its time, whatever ended the drag before: no
  // click is swallowed.
  const clickOpens = async (name) => {
    const at = await spot(page, 3, 16 * 60);
    await page.mouse.click(at.x, at.y);
    await until(page, `${name}: a click on a free stretch then opens the form`, (form) => onPage(`${form} .own-two input`)?.value === "16:00", form);
    await page.keyboard.press("Escape");
    await until(page, `${name}: the form closed`, () => !onPage("dialog.own-form[open]"));
  };
  // Every way a drag can end without its pointer let go over the week leaves nothing: no span, no
  // hold of the pointer. Each begun at Wednesday 16:05 and drawn down to 17:20.
  const begin = async () => {
    const [from, to] = [await spot(page, 3, 16 * 60 + 5), await spot(page, 3, 17 * 60 + 20)];
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move(from.x, to.y, { steps: 6 });
    await page.waitForFunction(() => Boolean(onPage(".sp-week .sp-sketch")), null, { timeout: 5000 }).catch(() => {});
  };
  const ended = async (name, end, after = async () => page.mouse.up()) => {
    await begin();
    await end();
    await page.waitForTimeout(250);
    const now = await sketch();
    check(none(now), `${name}: the drag left something behind: ${JSON.stringify(now)}`);
    await after();
    await page.waitForTimeout(450);
    const later = await sketch();
    check(none(later) && !(await page.evaluate(() => Boolean(onPage("dialog.own-form[open]")))), `${name}: after it something was drawn or opened: ${JSON.stringify(later)}`);
    await clickOpens(name);
  };
  // Let go where the week never hears it (a menu, another window), and the mouse comes back with its
  // button up: nothing follows it.
  await ended("let go elsewhere", () => page.evaluate(() => {
    const r = onPage(".sp-week .week .col").getBoundingClientRect();
    onPage(".sp-week").dispatchEvent(new PointerEvent("pointermove", { pointerId: 1, pointerType: "mouse", buttons: 0, bubbles: true, cancelable: true, clientX: r.left + 20, clientY: r.top + 300 }));
  }), async () => {
    await page.mouse.up();
    const [b, c] = [await spot(page, 4, 16 * 60), await spot(page, 4, 18 * 60)];
    await page.mouse.move(b.x, b.y, { steps: 3 });
    await page.mouse.move(c.x, c.y, { steps: 5 });
  });
  // (The browser says it at the pointer's next event.)
  await ended("the browser takes the pointer (lostpointercapture)", async () => {
    await page.evaluate(() => onPage(".sp-week").releasePointerCapture(1));
    const to = await spot(page, 3, 17 * 60 + 20);
    await page.mouse.move(to.x + 2, to.y + 1);
  });
  await ended("the browser takes the pointer (pointercancel)", () => page.evaluate(() => onPage(".sp-week").dispatchEvent(new PointerEvent("pointercancel", { pointerId: 1, bubbles: true }))));
  await ended("a second pointer comes down", () => page.evaluate(() => {
    const r = onPage(".sp-week .week .col").getBoundingClientRect();
    onPage(".sp-week").dispatchEvent(new PointerEvent("pointerdown", { pointerId: 7, pointerType: "touch", bubbles: true, clientX: r.left + 4, clientY: r.top + 4 }));
  }));
  await ended("the window loses the focus", () => page.evaluate(() => window.dispatchEvent(new Event("blur"))));
  await ended("the page turns a phone's", () => page.setViewportSize({ width: 800, height: 900 }), async () => {
    await page.mouse.up();
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.waitForFunction(() => Boolean(onPage(".sp-week .week .col")), null, { timeout: 5000 }).catch(() => {});
  });
  // A first move already past the grid's end is held, not lost: Friday from 17:05 flicked down.
  const flick = await spot(page, 5, 17 * 60 + 5);
  await page.mouse.move(flick.x, flick.y);
  await page.mouse.down();
  await page.mouse.move(flick.x, flick.y + 900);
  await page.mouse.up();
  await until(page, "a flick past the grid's end draws to its last hour", (form) => onPage(`${form} #own-day`)?.dataset.value === "5" && allOnPage(`${form} .own-two input`).map((input) => input.value).join() === "17:00,19:00", form);
  await page.keyboard.press("Escape");
  await until(page, "the flick's form closed", () => !onPage("dialog.own-form[open]"));
  // The wheel scrolls the page while the mouse draws: the minutes follow the column where it is now.
  {
    const from = await spot(page, 3, 16 * 60 + 5);
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move(from.x, from.y + 20, { steps: 3 });
    await page.mouse.wheel(0, 120);
    await page.waitForTimeout(300);
    const to = await page.evaluate(() => {
      const week = onPage(".sp-week .week");
      const col = week.querySelectorAll(".col")[2].getBoundingClientRect();
      const [first, last] = [Number(week.dataset.first), Number(week.dataset.last)];
      return col.top + col.height * ((17 * 60 + 20 - first) / (last - first)) + 2;
    });
    await page.mouse.move(from.x, to, { steps: 4 });
    await until(page, "a wheel while drawing shifts no time", () => onPage(".sp-week .sp-sketch-tag")?.textContent === "16:00–17:30");
    await page.keyboard.press("Escape");
    await page.mouse.up();
    await page.evaluate(() => window.scrollTo(0, 0));
  }
  // Drawn across slots, the span lies over them: no slot of the week changes its width (Tuesday
  // from 07:05 down to 12:50).
  {
    const widths = () => page.evaluate(() => allOnPage(".sp-week .week a.slot").map((slot) => Math.round(slot.getBoundingClientRect().width * 10) / 10).join());
    const before = await widths();
    const [from, to] = [await spot(page, 2, 7 * 60 + 5), await spot(page, 2, 12 * 60 + 50)];
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move(from.x, to.y, { steps: 8 });
    await until(page, "the span across Tuesday's slots", () => onPage(".sp-week .sp-sketch-tag")?.textContent === "07:00–13:00");
    const during = await widths();
    check(during === before, `drawn across slots, the slots changed their widths: ${before} → ${during}`);
    await page.keyboard.press("Escape");
    await page.mouse.up();
    await page.waitForTimeout(300);
  }

  // The slot's bin: gone, with „Rückgängig".
  const hiwi = await page.evaluate(() => {
    const slot = allOnPage(".sp-week a.slot.own").find((slot) => slot.textContent.includes("HiWi"));
    slot?.nextElementSibling?.querySelector(".slot-act[data-icon='trash-2']")?.click();
    return Boolean(slot);
  });
  check(hiwi, "the slot of „HiWi\" and its bin");
  await until(page, "the bin: the line gone, the note", ([key]) => !(localStorage.getItem(key) ?? "").includes("\tHiWi\t") && !allOnPage(".sp-week a.slot.own").some((slot) => slot.textContent.includes("HiWi")) && onPage(".own-note .plan-toast-said")?.textContent.includes("„HiWi“ gelöscht"), [PLAN]);
  await click(page, ".own-note .plan-toast-undo", "„Rückgängig\" of the bin");
  await until(page, "„Rückgängig“: „HiWi“ back", ([key]) => (localStorage.getItem(key) ?? "").includes("\tHiWi\t") && allOnPage(".sp-week a.slot.own").some((slot) => slot.textContent.includes("HiWi")), [PLAN]);

  // „Fertig": the head counts them, and no slot carries a button.
  await click(page, ".sp-editbar .sp-done", "„Fertig\"");
  await until(page, "looking again: the head counts them, no slot has a button", () => !/[?&]edit=1/.test(location.search) && !onPage(".sp-editbar") && onPage(".sp-sum")?.textContent.endsWith("2 eigene Termine") && allOnPage(".sp-week .slot-acts").length === 0);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- its card: pencil, eye, bin ----------
if (block("card")) {
  const { page, context } = await open(wide, FS1 + ARBEIT);
  await until(page, "the week", () => allOnPage(".sp-week a.slot.own").some((slot) => slot.textContent.includes("Arbeit")));
  check(await page.evaluate(() => onPage(".sp-sum")?.textContent.endsWith("1 eigener Termin")), `the head: ${await text(page, ".sp-sum")}`);
  await click(page, '.sp-week a.slot.own:has-text("Arbeit")', "the slot of „Arbeit\"");
  await until(page, "its card", () => onPage("dialog[open] .tm-card .tm-title")?.textContent === "Arbeit");
  let v = await card(page);
  check(v.chip === "Eigener Termin" && !v.kind && !v.mark && !v.share, `its head: ${JSON.stringify(v)}`);
  check(/^Jede Woche · \d\d\.\d\d\.–\d\d\.\d\d\., die Vorlesungszeit$/.test(v.when) && v.feed === "nicht in Datei und festem Abo", `its lines: ${JSON.stringify(v)}`);
  check(JSON.stringify(v.tools) === JSON.stringify(["own-edit pencil", "hide-date eye", "own-remove trash-2"]) && v.toolLabels[0] === "Bearbeiten" && /^Nur am Do \d\d\.\d\d\. ausblenden$/.test(v.toolLabels[1]) && v.toolLabels[2] === "Löschen", `the pencil, the eye, the bin: ${JSON.stringify([v.tools, v.toolLabels])}`);
  if (shots) await page.screenshot({ path: `${shots}/own-card.png` });
  // The eye skips the day, the card closes, the note brings it back.
  const day = v.date;
  await click(page, "dialog[open] .tm-tools [data-action='hide-date']", "the eye");
  await until(page, "the day skipped", ([key, day]) => /^t\t1\t0\t2026W\tw\t4\t13:00-17:00\t\t\t\tArbeit\tCafé Mosaik\t\d{4}-\d\d-\d\d$/m.test(localStorage.getItem(key) ?? "") && !onPage("dialog[open] .tm-card") && !location.search.includes("termin=") && onPage(".own-note .plan-toast-said")?.textContent.trim() === `„Arbeit“ am ${day} ausgeblendet`, [PLAN, day]);
  await click(page, ".own-note .plan-toast-undo", "„Rückgängig\" of the day");
  await until(page, "the day held again", ([key, line]) => (localStorage.getItem(key) ?? "").includes(line), [PLAN, ARBEIT]);
  // Skipped again: on that day the eye is crossed, and „Am … einblenden" keeps the card.
  await click(page, '.sp-week a.slot.own:has-text("Arbeit")', "the slot of „Arbeit\"");
  await until(page, "its card again", () => Boolean(onPage("dialog[open] .tm-card [data-action='hide-date']")));
  await click(page, "dialog[open] .tm-tools [data-action='hide-date']", "the eye");
  await until(page, "the day skipped again", ([key]) => /\tArbeit\tCafé Mosaik\t\d{4}-\d\d-\d\d$/m.test(localStorage.getItem(key) ?? "") && !onPage("dialog[open] .tm-card"), [PLAN]);
  await click(page, '.sp-week a.slot.own:has-text("Arbeit")', "the slot of „Arbeit\"");
  await until(page, "its card on another day", () => Boolean(onPage("dialog[open] .tm-card .tk")));
  if (!(await page.evaluate(() => onPage("dialog[open] .tm-tools [data-action='show-date']")))) {
    await page.evaluate(() => document.activeElement?.blur());
    await page.waitForFunction(() => !document.querySelector(".pending-page.held"), null, { timeout: 2000 }).catch(() => {});
    await page.keyboard.press("ArrowLeft");
  }
  await until(page, "on the skipped day the crossed eye", (day) => onPage("dialog[open] .tk-date > b")?.textContent.trim() === day && onPage("dialog[open] .tm-tools [data-action='show-date'] use")?.getAttribute("href")?.endsWith("#eye-off") && onPage("dialog[open] .tm-tools [data-action='show-date']")?.getAttribute("aria-label") === `Am ${day} einblenden`, day);
  await click(page, "dialog[open] .tm-tools [data-action='show-date']", "„Am … einblenden\"");
  await until(page, "the day held again, the card stays", ([key, line]) => (localStorage.getItem(key) ?? "").includes(line) && Boolean(onPage("dialog[open] .tm-card [data-action='hide-date']")), [PLAN, ARBEIT]);
  // The pencil: the form with its fields, „Speichern".
  await click(page, "dialog[open] .tm-tools [data-action='own-edit']", "the pencil");
  await until(page, "its form", (form) => onPage(`${form} #own-form-title`)?.textContent === "„Arbeit“ bearbeiten" && onPage(`${form} #own-where`)?.value === "Café Mosaik" && onPage(`${form} #own-day`)?.dataset.value === "4", form);
  // Keys typed one by one keep the field and the title: the form is built once as it opens.
  await page.click(`${form} #own-what`);
  await page.keyboard.press("End");
  await page.keyboard.type("!", { delay: 30 });
  check(await page.evaluate((form) => document.activeElement?.id === "own-what" && onPage(`${form} #own-form-title`)?.textContent === "„Arbeit“ bearbeiten" && onPage(`${form} #own-what`)?.value === "Arbeit!", form), "a key typed moved the focus or the title");
  await page.keyboard.press("Backspace");
  await fill(page, { place: "Mensa" });
  await click(page, `dialog.own-form .st-dlg-foot .primary`, "„Speichern\"");
  await until(page, "„Speichern“", ([key]) => (localStorage.getItem(key) ?? "").includes("t\t1\t0\t2026W\tw\t4\t13:00-17:00\t\t\t\tArbeit\tMensa\t\n") && onPage(".own-note .plan-toast-said")?.textContent.includes("„Arbeit“ gespeichert"), [PLAN]);
  // The bin: the card closes, the note brings it back.
  if (!(await page.evaluate(() => onPage("dialog[open] .tm-card")))) await click(page, '.sp-week a.slot.own:has-text("Arbeit")', "the slot of „Arbeit\"");
  await until(page, "its card for the bin", () => Boolean(onPage("dialog[open] .tm-card [data-action='own-remove']")));
  await click(page, "dialog[open] .tm-tools [data-action='own-remove']", "the bin");
  await until(page, "„Löschen“", ([key]) => !(localStorage.getItem(key) ?? "").includes("\tArbeit\t") && !location.search.includes("termin=") && !onPage("dialog[open] .tm-card") && onPage(".own-note .plan-toast-said")?.textContent.includes("„Arbeit“ gelöscht"), [PLAN]);
  await click(page, ".own-note .plan-toast-undo", "„Rückgängig\" of the bin");
  await until(page, "„Rückgängig“", ([key]) => (localStorage.getItem(key) ?? "").includes("\tArbeit\tMensa\t") && allOnPage(".sp-week .slot.own").some((slot) => slot.textContent.includes("Arbeit")), [PLAN]);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- a module without Termine: „Termin eintragen", while looking ----------
if (block("module")) {
  const { page, context } = await open(wide, FS1 + `m\t2026W\t${PRAK_BS}\t1790000000\t\n`);
  await until(page, "„Termin eintragen“", (id) => /^keine Termine in QIS/.test(onPage(`.sp-list .sp-mod[data-id='${id}'] .sp-cat-none`)?.textContent.trim() ?? "") && Boolean(onPage(`.sp-list .sp-mod[data-id='${id}'] .sp-cat-none .sp-enter`)), PRAK_BS);
  await click(page, `.sp-list .sp-mod[data-id='${PRAK_BS}'] .sp-cat-none .sp-enter`, "„Termin eintragen\"");
  await until(page, "the form with the module", ([form, id]) => onPage(`${form} #own-module`)?.dataset.value === id, [form, PRAK_BS]);
  check(!(await page.evaluate(() => /[?&]edit=1/.test(location.search))), "„Termin eintragen\" entered the mode");
  await fill(page, { day: 4, from: "13:45", to: "15:15" });
  await click(page, `dialog.own-form .st-dlg-foot .primary`, "„Anlegen\"");
  // Without a name it goes by its module: „PBS" in the week, under its module, and in the note.
  await until(page, "the module's Termin in its tone, by its module's name", ([key, id]) => {
    const line = (localStorage.getItem(key) ?? "").split("\n").find((line) => line.startsWith("t\t"));
    const slot = allOnPage(".sp-week .slot.own").find((slot) => slot.title.includes("von dir"));
    return line === `t\t1\t0\t2026W\tw\t4\t13:45-15:15\t\t\t${id}\t\t\t` && slot && !slot.classList.contains("t-own") && /^PBS/.test(slot.textContent)
      && onPage(`.sp-list .sp-mod[data-id='${id}'] .sp-cat.own`)?.innerText.replace(/\s+/g, " ").includes("Do 13:45") && onPage(".own-note .plan-toast-said")?.textContent.trim() === "PBS angelegt";
  }, [PLAN, PRAK_BS]);
  check(!(await page.evaluate((id) => onPage(`.sp-list .sp-mod[data-id='${id}'] .sp-cat-none .sp-enter`), PRAK_BS)), "„Termin eintragen\" stays once the module has a Termin");
  if (shots) await page.screenshot({ path: `${shots}/own-module.png` });
  // Its card: the module's chip and title, „von dir".
  await click(page, '.sp-week .slot.own:has-text("PBS")', "the slot of „PBS\"");
  await until(page, "its card by its module", () => Boolean(onPage("dialog[open] .tm-card .tk")));
  const v = await card(page);
  check(v.title === "Praktikum Betriebssysteme" && v.chip === "PBS" && v.mark === "von dir" && v.feed === "nicht in Datei und festem Abo", `its card: ${JSON.stringify(v)}`);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- a Termin of a module, with its kind ----------
if (block("kind")) {
  const { page, context } = await open(wide, FS1, "/studyplan?edit=1");
  await until(page, "„+ Termin\" under EvS", () => Boolean(onPage(".sp-list .sp-mod[data-id='12104'] .sp-own-add")));
  await click(page, ".sp-list .sp-mod[data-id='12104'] .sp-own-add", "„+ Termin\" under EvS");
  await until(page, "the form for EvS", (form) => onPage(`${form} #own-module`)?.dataset.value === "12104" && onPage(`${form} .st-dlg-head p`)?.textContent === "Für EvS · WiSe 2026/27", form);
  await pick(page, `${form} #own-kind`, "lecture");
  await fill(page, { day: 2, from: "16:00", to: "17:30" });
  await until(page, "the draft „VL EvS\"", () => allOnPage(".sp-week .slot.draft").some((slot) => slot.textContent.includes("VL EvS")));
  await click(page, `dialog.own-form .st-dlg-foot .primary`, "„Anlegen\"");
  await until(page, "„VL EvS\": the line with its kind, the slot, the row under EvS, the note", ([key]) => {
    const lines = (localStorage.getItem(key) ?? "").split("\n").filter((line) => line.startsWith("t\t"));
    const row = onPage(".sp-list .sp-mod[data-id='12104'] .sp-cat.own .sp-cat-row");
    return lines.join() === "t\t1\t0\t2026W\tw\t2\t16:00-17:30\t\t\t12104\t\t\t\tlecture" && allOnPage(".sp-week a.slot.own").some((slot) => slot.querySelector(".l")?.textContent.replace(/\s+/g, " ").trim() === "VL EvS")
      && row?.querySelector("b")?.textContent === "Vorlesung" && row.querySelector(".sp-cat-sum")?.textContent.includes("Di 16:00") && onPage(".own-note .plan-toast-said")?.textContent.trim() === "VL EvS angelegt";
  }, [PLAN]);
  check(await page.evaluate(() => !onPage(".sp-list .sp-own .sp-row.own")), "a module's Termin under „Eigene Termine\"");
  // Its card: „EvS Vorlesung · von dir", the module's title.
  await click(page, '.sp-week a.slot.own:has-text("VL EvS")', "the slot of „VL EvS\"");
  await until(page, "its card", () => Boolean(onPage("dialog[open] .tm-card .tk")));
  const v = await card(page);
  check(v.chip === "EvS" && v.kind === "Vorlesung" && v.mark === "von dir" && v.title === "Entwicklung von Softwaresystemen", `its card: ${JSON.stringify(v)}`);
  // „Art" back to „Termin": the slot says the module, the line loses its kind.
  await click(page, "dialog[open] .tm-tools [data-action='own-edit']", "the pencil");
  await until(page, "its form", (form) => onPage(`${form} #own-kind`)?.dataset.value === "lecture", form);
  await pick(page, `${form} #own-kind`, "");
  await click(page, `dialog.own-form .st-dlg-foot .primary`, "„Speichern\"");
  await until(page, "without a kind", ([key]) => (localStorage.getItem(key) ?? "").split("\n").filter((line) => line.startsWith("t\t")).join() === "t\t1\t0\t2026W\tw\t2\t16:00-17:30\t\t\t12104\t\t\t" && allOnPage(".sp-week a.slot.own").some((slot) => slot.querySelector(".l")?.textContent.trim() === "EvS"), [PLAN]);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- Termine of one's own alone: no „Noch keine Termine" ----------
if (block("alone")) {
  const { page, context } = await open(wide, "t\t1\t0\t2026W\tw\t2\t17:00-19:00\t\t\t\tChor\tAula\t\n");
  await until(page, "the week of one's own alone", () => allOnPage(".sp-week .slot.own").some((slot) => slot.textContent.includes("Chor")) && !onPage(".sp-empty") && onPage(".sp-sum")?.textContent.endsWith("1 eigener Termin"));
  await page.goto(base + "/studyplan?view=dates", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await until(page, "„Termine“ lists it with its mark", () => allOnPage(".agenda-item.own").some((item) => item.textContent.includes("Chor") && item.querySelector(".ag-own") && item.title.includes("von dir")) && !onPage(".sp-empty"));
  await click(page, '.agenda-item.own:has-text("Chor") >> nth=0', "„Chor\" in „Termine\"");
  await until(page, "its card from „Termine“", () => onPage("dialog[open] .tm-card .tm-title")?.textContent === "Chor");
  await context.close();
}

// ---------- Lena's calendar ----------
if (block("lena")) {
  const { page, context } = await open(wide, FS1 + LENA + WG);
  await until(page, "Lena's part", () => Boolean(onPage(".sp-cal-mods")));
  check(await visible(page, ".sp-cal-mods .sp-own-add") === 0, "Lena's „+ Termin\" while looking");
  await click(page, "#sp-edit-toggle", "„Bearbeiten\"");
  await until(page, "Lena's „+ Termin\" while editing", () => Boolean(onPage(".sp-cal-mods .sp-own-add")));
  await click(page, ".sp-cal-mods .sp-own-add", "Lena's „+ Termin\"");
  await until(page, "the form for Lena", (form) => onPage(`${form} #own-cal`)?.dataset.value === "1" && onPage(`${form} .st-dlg-head p`)?.textContent === "In „Lena“, WiSe 2026/27", form);
  // The line under the title follows „Kalender".
  await pick(page, `${form} #own-cal`, "0");
  await until(page, "the line follows „Kalender“", (form) => onPage(`${form} .st-dlg-head p`)?.textContent === "In „Mein Studiengang“, WiSe 2026/27", form);
  await pick(page, `${form} #own-cal`, "1");
  await fill(page, { name: "Arbeit", day: 3, from: "13:00", to: "17:00" });
  await until(page, "what it counts for", (form) => onPage(`${form} .own-fit.ok`)?.textContent === "Zählt für „Gemeinsam frei“ mit Lena", form);
  // From one calendar of one's own to another the line names the other one, as the line under the
  // title does (it kept Lena's name: a closure that read what `calendar` makes before it, R16).
  await pick(page, `${form} #own-cal`, "2");
  await until(page, "the line follows „Kalender“ from Lena to the WG", (form) => onPage(`${form} .own-fit.ok`)?.textContent === "Zählt für „Gemeinsam frei“ mit WG" && onPage(`${form} .st-dlg-head p`)?.textContent === "In „WG“, WiSe 2026/27", form);
  await pick(page, `${form} #own-cal`, "1");
  await until(page, "and back to Lena", (form) => onPage(`${form} .own-fit.ok`)?.textContent === "Zählt für „Gemeinsam frei“ mit Lena", form);
  await click(page, `dialog.own-form .st-dlg-foot .primary`, "„Anlegen\"");
  await until(page, "in Lena's calendar", ([key]) => (localStorage.getItem(key) ?? "").includes("t\t1\t1\t2026W\tw\t3\t13:00-17:00\t\t\t\tArbeit\t\t\n") && onPage(".sp-cal-mods .sp-own button.sp-row.own")?.textContent.includes("Arbeit"), [PLAN]);
  check(!(await page.evaluate(() => onPage(".sp-list .sp-own .sp-row.own"))), "Lena's Termin in „Mein Studiengang“'s list");
  // Its row opens its form.
  await click(page, ".sp-cal-mods .sp-own button.sp-row.own", "its row");
  await until(page, "its form from the row", (form) => onPage(`${form} #own-form-title`)?.textContent === "„Arbeit“ bearbeiten", form);
  await page.keyboard.press("Escape");
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- a phone: the dock's „+ Termin", a finger held, only while editing ----------
if (block("phone")) {
  const { page, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, FS1);
  await until(page, "the week", () => allOnPage(".sp-week .slot").length > 0 && Boolean(onPage(".sp-head .sp-edit-toggle")));
  check(await visible(page, ".sp-own-fab, .sp-editdock, [data-action='own-add']") === 0, "a „+ Termin\" while looking");
  // A finger held on Wednesday at 16:00 while looking: nothing.
  let at = await spot(page, 3, 16 * 60);
  await hold(page, at);
  await page.waitForTimeout(300);
  check(await page.evaluate(() => !onPage("dialog.own-form[open]")), "a finger held while looking opened the form");
  await page.tap(".sp-head .sp-edit-toggle");
  await until(page, "the dock's „+ Termin\"", () => {
    const add = onPage(".sp-editdock [data-action='own-add']")?.getBoundingClientRect();
    return /[?&]edit=1/.test(location.search) && add && add.bottom <= innerHeight && add.right <= innerWidth && add.height >= 40;
  });
  await page.tap(".sp-editdock [data-action='own-add']");
  await until(page, "the form as a sheet", (form) => Boolean(onPage(form)) && onPage(`${form}`)?.hasAttribute("data-sheet"), form);
  check((await text(page, `${form} .own-quicker`)) === "Schneller: in der Woche gedrückt halten und ziehen.", `the phone's hint of the gesture: ${await text(page, `${form} .own-quicker`)}`);
  if (shots) await page.screenshot({ path: `${shots}/own-phone-form.png` });
  await page.keyboard.press("Escape");
  await until(page, "the sheet closed, the mode stays", () => !onPage("dialog.own-form[open]") && /[?&]edit=1/.test(location.search));
  // A finger held on Wednesday at 16:00 while editing.
  at = await spot(page, 3, 16 * 60);
  const target = await page.evaluate(([x, y]) => document.elementFromPoint(x, y)?.className ?? "", [at.x, at.y]);
  await hold(page, at);
  await until(page, `a finger held opens the form for Wednesday (on ${target})`, (form) => onPage(`${form} #own-day`)?.dataset.value === "3" && onPage(`${form} .own-two input`)?.value === "16:00", form);
  await page.keyboard.press("Escape");
  await until(page, "the form closed", () => !onPage("dialog.own-form[open]"));
  // A real finger, as the owner defined it (2026-11-18: „Das soll so sein, dass man lange gedrückt
  // hält, dann bekommt man ein vib feedback, und danach kann man draggen oder einfach los lassen. Und
  // erst veim los lassen wird der termin erstellt. Beim draggen in dem bereich und wenn man direkt los
  // lässt dann ein 90min termin."): held, the phone vibrates as the hold completes and not before, the
  // span of 90 minutes stands, nothing is open and nothing stored; moved, it stretches with its times in
  // a tag outside it and the page does not scroll; lifted, the form opens with exactly what stood.
  const cdp = await context.newCDPSession(page);
  const touch = (type, x, y) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x: Math.round(x), y: Math.round(y), id: 1 }] });
  const drawnNow = () => page.evaluate(() => ({
    span: allOnPage(".sp-sketch").map((el) => el.textContent.replace(/\s+/g, " ").trim()).join(),
    tag: allOnPage(".sp-sketch-tag").map((el) => el.textContent.trim()).join(),
    form: Boolean(onPage("dialog.own-form[open]")),
    felt: window.__felt.length,
    scroll: scrollY,
  }));
  const stored = await own(page);
  // Each spot may scroll its point into view: both are taken once the page holds still for them.
  await spot(page, 3, 17 * 60 + 5);
  const from = await spot(page, 3, 16 * 60 + 5);
  const further = await spot(page, 3, 17 * 60 + 5);
  const before = await page.evaluate(() => { window.__felt = []; return scrollY; });
  await touch("touchStart", from.x, from.y);
  await page.waitForTimeout(300);
  const early = await drawnNow();
  check(early.felt === 0 && !early.span && !early.form, `before the hold completes nothing is felt or drawn: ${JSON.stringify(early)}`);
  await page.waitForTimeout(400);
  const held = await drawnNow();
  check(held.felt === 1 && held.span === "16:00 – 17:30" && held.tag === "16:00–17:30" && !held.form, `the hold completed: felt once, 90 minutes stand, nothing open: ${JSON.stringify(held)}`);
  // The long press's menu of the browser does not come over a finger held on the week.
  check(await page.evaluate(([x, y]) => !document.elementFromPoint(x, y).dispatchEvent(new PointerEvent("contextmenu", { bubbles: true, cancelable: true, pointerType: "touch", clientX: x, clientY: y })), [from.x, from.y]), "a finger held on the week brings the browser's menu");
  for (let i = 1; i <= 8; i++) {
    await touch("touchMove", from.x, from.y + ((further.y - from.y) * i) / 8);
    await page.waitForTimeout(30);
  }
  const drawn = await drawnNow();
  check(drawn.tag === "16:00–17:15" && drawn.scroll === before && !drawn.form && drawn.felt === 1, `the finger stretches the span and the page holds still: ${JSON.stringify({ ...drawn, before })}`);
  check(JSON.stringify(await own(page)) === JSON.stringify(stored), "a Termin was stored before the finger lifted");
  if (shots) await page.screenshot({ path: `${shots}/own-phone-drag.png` });
  // The finger rests before it lifts: a lift within 80 ms of the span's last change is taken as
  // the shift of lifting (the block „a real finger" below), and this finger's last move was now.
  await page.waitForTimeout(250);
  await touch("touchEnd");
  await until(page, "the form of the span a finger drew, the tag gone", (form) => onPage(`${form} #own-day`)?.dataset.value === "3" && allOnPage(`${form} .own-two input`).map((input) => input.value).join() === "16:00,17:15" && !onPage(".sp-sketch-tag"), form);
  check(JSON.stringify(await own(page)) === JSON.stringify(stored), "the lift stored a Termin before „Anlegen\"");
  await page.keyboard.press("Escape");
  await until(page, "the form closed again", () => !onPage("dialog.own-form[open]"));
  // Held and let go without a move: 90 minutes.
  {
    const at = await spot(page, 3, 16 * 60 + 5);
    await touch("touchStart", at.x, at.y);
    await page.waitForTimeout(700);
    await touch("touchEnd");
    await until(page, "a hold let go at once gives 90 minutes", (form) => allOnPage(`${form} .own-two input`).map((input) => input.value).join() === "16:00,17:30", form);
    await page.keyboard.press("Escape");
    await until(page, "its form closed", () => !onPage("dialog.own-form[open]"));
  }
  // The browser takes the finger's pointer while it draws: the span is given up, nothing opens.
  {
    const at = await spot(page, 3, 16 * 60 + 5);
    await touch("touchStart", at.x, at.y);
    await page.waitForTimeout(700);
    await touch("touchMove", at.x, at.y + 30);
    await page.evaluate(() => onPage(".sp-carousel").dispatchEvent(new PointerEvent("pointercancel", { pointerId: 1, pointerType: "touch", bubbles: true })));
    await page.waitForTimeout(200);
    const gone = await drawnNow();
    await touch("touchEnd");
    await page.waitForTimeout(500);
    check(!gone.span && !gone.tag && !(await page.evaluate(() => Boolean(onPage("dialog.own-form[open]")))), `a cancelled finger left a span or opened the form: ${JSON.stringify(gone)}`);
  }
  // In the week's last column the tag stays on the screen, and never under the finger.
  {
    const at = await spot(page, 5, 16 * 60 + 5);
    if (await page.evaluate(([x, y]) => !document.elementFromPoint(x, y)?.closest(".slot, a, button"), [at.x, at.y])) {
      await touch("touchStart", at.x, at.y);
      await page.waitForTimeout(700);
      for (let i = 1; i <= 4; i++) await touch("touchMove", at.x, at.y + 12 * i);
      const tag = await page.evaluate(() => { const r = onPage(".sp-sketch-tag")?.getBoundingClientRect(); return r && { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: innerWidth }; });
      check(tag && tag.left >= 0 && tag.right <= tag.width && !(tag.top <= at.y + 48 && tag.bottom >= at.y + 48 && tag.left <= at.x && tag.right >= at.x), `the tag of Friday's span leaves the screen or lies under the finger: ${JSON.stringify({ tag, at })}`);
      await touch("touchEnd");
      await page.waitForTimeout(400);
      await page.keyboard.press("Escape");
      await page.waitForTimeout(300);
    } else problems.push("Friday 16:05 is no free stretch here");
  }
  const quick = await spot(page, 3, 16 * 60 + 5);
  const still = await page.evaluate(() => scrollY);
  await touch("touchStart", quick.x, quick.y);
  for (let i = 1; i <= 8; i++) {
    await touch("touchMove", quick.x, quick.y - 25 * i);
    await page.waitForTimeout(16);
  }
  await touch("touchEnd");
  await page.waitForTimeout(800);
  const moved = await page.evaluate(() => ({ scroll: scrollY, draft: allOnPage(".sp-week .slot.draft, .sp-sketch").length, form: Boolean(onPage("dialog.own-form[open]")) }));
  check(moved.scroll > still + 40 && moved.draft === 0 && !moved.form, `a finger moving at once scrolls and draws nothing: ${JSON.stringify({ ...moved, still })}`);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- a real finger: it trembles, drifts and shifts as it lifts ----------
// (owner, 2026-11-18, of a real Android phone: „Ja das dragging funktioniert nicht dass ist richtig
// buggy."). On the phone's carousel of A, B and A/B, while editing; a finger's millimetre is some 6 px.
if (block("finger")) {
  const { page, context } = await open({ viewport: { width: 412, height: 915 }, isMobile: true, hasTouch: true }, FS1, "/studyplan?edit=1");
  await until(page, "the week while editing", () => allOnPage(".sp-week .slot").length > 0 && /[?&]edit=1/.test(location.search) && Boolean(onPage(".sp-slide.is-current")));
  const cdp = await context.newCDPSession(page);
  // `at` (seconds): the event's own time, which the page reads as its `timeStamp`. Where the times
  // between a finger's last moves and its lift decide what the lift gives, the check says them
  // itself: how long a call takes here (some 50 ms, more on a busy machine) is not the finger's.
  const touch = (type, x, y, at) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x: Math.round(x), y: Math.round(y), id: 1 }], ...(at === undefined ? {} : { timestamp: at }) });
  const clock = () => Date.now() / 1000 + 0.02;
  // The shift of a lifting finger: its moves and the lift `ms` apart on the finger's clock.
  const lift = async (moves, ms = 15) => {
    const from = clock();
    for (const [i, [x, y]] of moves.entries()) await touch("touchMove", x, y, from + (i * ms) / 1000);
    await touch("touchEnd", 0, 0, from + (moves.length * ms) / 1000);
  };
  const now = () => page.evaluate(() => ({
    tag: onPage(".sp-sketch-tag")?.textContent ?? null,
    form: allOnPage("dialog.own-form[open] .own-two input").map((input) => input.value).join() || null,
    carried: getComputedStyle(onPage(".sp-slide.is-current") ?? document.body).getPropertyValue("--drag").trim(),
    weeks: location.search,
  }));
  const formIs = (name, times) => until(page, name, ([form, times]) => allOnPage(`${form} .own-two input`).map((input) => input.value).join() === times && !onPage(".sp-sketch-tag"), [form, times]);
  const close = async () => {
    if (await page.evaluate(() => Boolean(onPage("dialog.own-form[open]")))) await page.keyboard.press("Escape");
    await until(page, "the form closed, the mode stays", () => !onPage("dialog.own-form[open]") && /[?&]edit=1/.test(location.search));
  };
  // Both points taken once the page holds still for them.
  const points = async (...minutes) => {
    for (const minute of minutes) await spot(page, 3, minute);
    await page.waitForTimeout(200);
    const out = [];
    for (const minute of minutes) out.push(await spot(page, 3, minute));
    return out;
  };
  const stored = await own(page);
  // Held, then the finger trembles 9 px (1.5 mm) and back: the 90 minutes stand, and the lift gives them.
  {
    const [at] = await points(16 * 60 + 5);
    await touch("touchStart", at.x, at.y);
    await page.waitForTimeout(700);
    await touch("touchMove", at.x + 1, at.y + 9);
    await page.waitForTimeout(40);
    const trembled = await now();
    await touch("touchMove", at.x, at.y + 1);
    await page.waitForTimeout(40);
    await touch("touchEnd");
    check(trembled.tag === "16:00–17:30", `a finger trembling 9 px drew another span than 90 minutes: ${JSON.stringify(trembled)}`);
    await formIs("a trembling finger lifted gives 90 minutes", "16:00,17:30");
    await close();
  }
  // Held, then lifted with a lifting finger's shift of 12 px within 30 ms: 90 minutes.
  {
    const [at] = await points(16 * 60 + 5);
    await touch("touchStart", at.x, at.y);
    await page.waitForTimeout(700);
    await lift([[at.x + 2, at.y - 6], [at.x + 3, at.y - 12]]);
    await formIs("a hold lifted with a shift gives 90 minutes", "16:00,17:30");
    await close();
  }
  // Drawn on purpose to 17:15, the finger rests 300 ms and lifts with a shift of 13 px: 17:15.
  {
    const [at, to] = await points(16 * 60 + 5, 17 * 60 + 10);
    await touch("touchStart", at.x, at.y);
    await page.waitForTimeout(700);
    for (let i = 1; i <= 10; i++) {
      await touch("touchMove", at.x, at.y + ((to.y - at.y) * i) / 10);
      await page.waitForTimeout(25);
    }
    await page.waitForTimeout(300);
    const rested = await now();
    check(rested.tag === "16:00–17:15", `the span drawn to 17:15: ${JSON.stringify(rested)}`);
    await lift([[at.x + 2, to.y + 7], [at.x + 3, to.y + 13]]);
    await formIs("a span rested on and lifted with a shift gives what stood", "16:00,17:15");
    await close();
  }
  // A steady drag lifted in motion: the form says the tag's last span.
  {
    const [at] = await points(16 * 60 + 5);
    await touch("touchStart", at.x, at.y);
    await page.waitForTimeout(700);
    // 25 ms a move on the finger's clock, the lift 10 ms after the last: no span stands for 150 ms.
    let last = null;
    const from = clock();
    for (let i = 1; i <= 12; i++) {
      await touch("touchMove", at.x, at.y + 7 * i, from + (i * 25) / 1000);
      last = (await now()).tag;
    }
    await touch("touchEnd", 0, 0, from + (12 * 25 + 10) / 1000);
    const times = last?.replace("–", ",") ?? "a tag";
    check(Boolean(last) && last !== "16:00–17:30", `a steady drag drew no span of its own: ${last}`);
    await formIs(`a drag lifted in motion gives what its tag said (${last})`, times);
    await close();
  }
  // A finger resting 1 px past the line of 17:15 and trembling ±2 px, so that it crosses the line
  // each time. Come back up from 17:45, so that 17:30 stands: the end does not flip. Come down from
  // above, so that 17:15 stands: the tremble's 3 px past the line step it once at most, and it
  // never steps back.
  for (const below of [true, false]) {
    const [at, line, far] = await points(16 * 60 + 5, 17 * 60 + 15, 17 * 60 + 40);
    // 1 px past the line (`spot` gives its minute 2 px down).
    const y = line.y - 1;
    const via = below ? far.y : y - 12;
    await touch("touchStart", at.x, at.y);
    await page.waitForTimeout(700);
    for (let i = 1; i <= 8; i++) {
      await touch("touchMove", at.x, at.y + ((via - at.y) * i) / 8);
      await page.waitForTimeout(25);
    }
    for (let i = 1; i <= 4; i++) {
      await touch("touchMove", at.x, via + ((y - via) * i) / 4);
      await page.waitForTimeout(25);
    }
    await page.waitForTimeout(100);
    const tags = [(await now()).tag];
    for (let i = 0; i < 12; i++) {
      await touch("touchMove", at.x + (i % 2), y + (i % 2 ? 2 : -2));
      await page.waitForTimeout(30);
      tags.push((await now()).tag);
    }
    const steps = tags.filter((tag, i) => i === 0 || tag !== tags[i - 1]);
    if (below) check(JSON.stringify(steps) === JSON.stringify(["16:00–17:30"]), `a resting finger flipped the end: ${JSON.stringify(steps)}`);
    else check(steps[0] === "16:00–17:15" && steps.length <= 2 && steps.every((tag) => /^16:00–17:(15|30)$/.test(tag)), `a resting finger come from above flipped the end back and forth: ${JSON.stringify(steps)}`);
    // It rests before it lifts, so that its last tremble is no shift of lifting.
    await page.waitForTimeout(250);
    await touch("touchEnd");
    await formIs("a resting finger lifted gives what stood", steps.at(-1).replace("–", ","));
    await close();
  }
  // Drifting 6 px, and the hold's whole 8 px, sideways while held: the hold completes, no week is
  // carried, the lift gives 90 minutes (the element the finger came down on held its pointer until
  // then; its loss of it is not the week's).
  for (const drift of [6, 8]) {
    const [at] = await points(16 * 60 + 5);
    const weeks = (await now()).weeks;
    await touch("touchStart", at.x, at.y);
    await page.waitForTimeout(150);
    await touch("touchMove", at.x + drift, at.y + 1);
    await page.waitForTimeout(550);
    const held = await now();
    check(held.tag === "16:00–17:30" && held.carried === "0px", `a finger drifting ${drift} px while held: ${JSON.stringify(held)}`);
    await touch("touchEnd");
    await formIs(`a hold drifted ${drift} px gives 90 minutes as it lifts`, "16:00,17:30");
    check((await now()).weeks === weeks, `a hold drifted ${drift} px turned the week`);
    await close();
  }
  // Drifting 9 and 12 px sideways while held: no hold, the week carried and back, no span, nothing open.
  for (const drift of [9, 12]) {
    const [at] = await points(16 * 60 + 5);
    const weeks = (await now()).weeks;
    await touch("touchStart", at.x, at.y);
    await page.waitForTimeout(150);
    await touch("touchMove", at.x + drift, at.y + 1);
    await page.waitForTimeout(550);
    const drifted = await now();
    check(drifted.tag === null && drifted.carried !== "0px", `a finger drifting ${drift} px while held: the hold and the carousel both had it, or neither: ${JSON.stringify(drifted)}`);
    await touch("touchEnd");
    await page.waitForTimeout(600);
    const after = await now();
    check(after.tag === null && after.form === null && after.carried === "0px" && after.weeks === weeks, `a finger drifted ${drift} px and lifted: ${JSON.stringify(after)}`);
    await close();
  }
  check(JSON.stringify(await own(page)) === JSON.stringify(stored), "a Termin was stored without „Anlegen\"");
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

// ---------- the start page: today's Termin of one's own ----------
if (block("start")) {
  const now = new Date();
  const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
  const line = `t\t1\t0\t2026W\td\t${today}\t23:00-23:30\t\t\t\tAbendlauf\tPark\t\n`;
  const { page, context } = await open(wide, FS1 + line, "/");
  await until(page, "today's Termin of one's own", () => allOnPage(".day-ev").some((entry) => entry.textContent.includes("Abendlauf")));
  await click(page, '.day-ev:has-text("Abendlauf")', "„Abendlauf\"");
  await until(page, "its card over the start page", () => onPage("dialog[open] .tm-card .tm-title")?.textContent === "Abendlauf" && Boolean(onPage("dialog[open] .tm-ways a.tm-way")));
  const v = await card(page);
  check(v.way.startsWith("/studyplan?") && !v.tools.some((tool) => tool.startsWith("own-remove")), `from the start page it leads to the Stundenplan: ${JSON.stringify(v)}`);
  check(await page.evaluate(() => window.__marker === 1), "the page loaded again");
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(`own: ${problems.length} problem(s)\n- ${problems.join("\n- ")}`);
  process.exit(1);
}
console.log("own: ok");
