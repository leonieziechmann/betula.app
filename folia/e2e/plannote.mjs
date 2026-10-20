// Checks „Einplanen" as concept E has it (owner, 2026-10-18: „Also ich liebe Option E, das ist mit
// Abstand die eleganteste"; folia/crates/stores/src/studyplan/planning.rs, folia/crates/widgets/src/
// plannote.rs and plansetup.rs). Without a study set up: „Einplanen" opens one dialog — no program
// assumed, the module's programs open (three at most, compulsory first) with the search under them,
// the app's picker as wide as they are and staying so when it opens; „Einrichten und einplanen"
// waiting for a program; a program picked is a row as in „Mein Studium" whose picker has the
// module's programs first, and fills the Fachsemester to fit its Regelstudienplan; the button stores
// the study and plans the module, and the note says where. With a study: the note at the bottom, the
// semester between ‹ and › one control — a click on its right half later, on its left half earlier —
// moving the module at once (back into the past as well, to the first Fachsemester, „liegt
// zurück"), „Weitere Module auch nach …" once moved, which the next module's switch then plans into,
// ← and → as the arrows, „Rückgängig"; a module taken out says so, and „Rückgängig" puts it back. It
// waits for interaction: it stands while nothing is done, and goes with a click elsewhere, ×, Esc, a
// key not its own, another module opened, a long scroll — not a little one. A semester chosen for
// the next modules says so in the dialog, which clears it. On a phone: a row swiped to the right
// opens the dialog as a sheet, „Ohne Studium einplanen" plans and the session asks no more; the note
// stands over the bottom bar, as wide, under it and flat; a tap on the semester's half and a finger
// swiped over it move the module, the semester under the finger stays as the switch comes over it,
// a finger swipes the note down and away, and a long scroll, a tap elsewhere or a module opened puts
// it away; planned from a module's sheet, the note goes down with the sheet as it closes. Planned,
// the switch's arrow opens its menu: passed and back, moved, ⇧P and the keys, taken out. On a wide
// screen the note is in order: the check and the name at the left, the semester, „Rückgängig" and ×
// at the right, the switch a row over all of it lined up with them; on a small one nothing of it
// over anything else, the module named where there is room.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node plannote.mjs   (SMOKE_BROWSER_CHANNEL=msedge by default)
// Needs the synthetic catalog (docs/folia/frontend.md §4): Algorithmen und Datenstrukturen (11112,
// winter, in the 1st semester of Informatik B.Sc.'s plan) and Analysis I (11103, summer); its
// current semester is the one of the day it was made, and the session plans for the semester of the
// browser's day (until 15 November or 15 May the running one, then the next). Fails on a console
// error or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const SWITCH = '.detail .hero .plan-toggle[data-action="plan"]';
const STORE = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
const shot = (page, name) => (process.env.SHOTS ? page.screenshot({ path: `${process.env.SHOTS}/plannote-${name}.png` }) : null);

const watch = (page) => {
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 600)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
};
// The semesters the plan holds the module in, from what the browser keeps.
const plannedIn = (page, id) => page.evaluate(([key, id]) => (localStorage.getItem(key) || "").split("\n").map((l) => l.split("\t")).filter((f) => f[0] === "m" && f[2] === id).map((f) => f[1]).sort(), [STORE, id]);
const waitPlanned = (page, id, list, what) => page.waitForFunction(([key, id, want]) => {
  const got = (localStorage.getItem(key) || "").split("\n").map((l) => l.split("\t")).filter((f) => f[0] === "m" && f[2] === id).map((f) => f[1]).sort();
  return JSON.stringify(got) === JSON.stringify([...want].sort());
}, [STORE, id, list], { timeout: 4000 }).catch(async () => problems.push(`${what}: planned in ${JSON.stringify(await plannedIn(page, id))}, not ${JSON.stringify(list)}`));
// What the note says: its semester, its line (all of it, and as shown), whether it can go back, its
// second row, where it stands and how it lies on the page.
const note = (page) => page.evaluate(() => {
  const el = document.querySelector(".plan-toast");
  if (!el) return null;
  // The note that waits for interaction around it (folia_design::waitnote).
  const frame = el.closest(".wait-note");
  const r = frame.getBoundingClientRect(), cs = getComputedStyle(frame);
  const words = el.querySelector(".plan-toast-now")?.getBoundingClientRect();
  return {
    at: el.querySelector(".plan-toast-now > b")?.textContent || null,
    line: el.querySelector(".plan-toast-now > small")?.textContent || null,
    shown: el.querySelector(".plan-toast-now > small")?.innerText || null,
    past: el.querySelector(".plan-toast-warn")?.textContent === "liegt zurück",
    said: el.querySelector(".plan-toast-said")?.textContent || null,
    back: el.querySelector(".plan-toast-step")?.disabled ?? null,
    keep: el.querySelector(".plan-toast-keep")?.getAttribute("aria-checked") ?? null,
    keepText: el.querySelector(".plan-toast-keep")?.textContent || null,
    title: el.querySelector(".plan-toast-title")?.checkVisibility() ? el.querySelector(".plan-toast-title").textContent : null,
    close: Boolean(frame.querySelector(".wait-note-close")?.checkVisibility()),
    box: { left: r.left, right: r.right, top: r.top, bottom: r.bottom, height: r.height },
    main: el.querySelector(".plan-toast-main")?.getBoundingClientRect().toJSON(),
    keepBox: el.querySelector(".plan-toast-keep")?.getBoundingClientRect().toJSON() ?? null,
    stepper: el.querySelector(".plan-toast-stepper")?.getBoundingClientRect().toJSON() ?? null,
    words: words ? { left: words.left, right: words.right, top: words.top, y: words.top + words.height / 2 } : null,
    // Where the parts that line up are: the check and the switch's icon, the name and the switch's
    // words, the × and the switch.
    lines: Object.fromEntries([["mark", ".plan-toast-mark"], ["name", ".plan-toast-title"], ["undo", ".plan-toast-undo"], ["close", ".wait-note-close .icon"], ["keepIcon", ".plan-toast-keep > .icon"], ["keepWords", ".plan-toast-keep > span:first-of-type"], ["keepSwitch", ".plan-toast-switch"]].map(([key, sel]) => { const e = frame.querySelector(sel); return [key, e?.checkVisibility() ? e.getBoundingClientRect().toJSON() : null]; })),
    z: Number(cs.zIndex),
    shadow: cs.boxShadow,
  };
});
const noteGone = (page, what, timeout = 2000) => page.waitForFunction(() => !document.querySelector(".plan-toast"), null, { timeout }).catch(() => problems.push(`${what}: the note stayed`));
const noteThere = (page, what) => page.waitForSelector(".plan-toast", { timeout: 2000 }).catch(() => problems.push(`${what}: no note`));
const rect = (el) => { if (!el) return null; const r = el.getBoundingClientRect(); return { left: r.left, right: r.right, top: r.top, height: r.height }; };
const dialog = (page) => page.evaluate(() => {
  const el = document.querySelector("dialog.plan-setup[open]");
  if (!el) return null;
  const r = el.getBoundingClientRect();
  const box = (e) => { if (!e) return null; const b = e.getBoundingClientRect(); return { left: b.left, right: b.right, top: b.top, height: b.height }; };
  return {
    say: el.querySelector(".st-study-say")?.textContent || "",
    offers: [...el.querySelectorAll(".plan-setup-offers > button")].map((b) => ({ name: b.querySelector("span")?.textContent, detail: b.querySelector("small")?.textContent, req: b.querySelector("small")?.classList.contains("req") })),
    search: Boolean(el.querySelector(".plan-setup-list #plan-setup-search")),
    offerBox: box(el.querySelector(".plan-setup-offers > button")),
    searchBox: box(el.querySelector("#plan-setup-search")),
    program: el.querySelector("#plan-setup-program .combo-value")?.firstChild?.textContent || null,
    rows: [...el.querySelectorAll(".st-srow")].map((row) => ({ value: row.querySelector(".st-fs-value")?.textContent || null, note: row.querySelector(".st-srow-note")?.textContent || "" })),
    warn: el.querySelector(".plan-setup-warn")?.textContent || null,
    go: !el.querySelector("#plan-setup-go")?.disabled,
    box: { left: r.left, right: r.right, top: r.top, bottom: r.bottom },
  };
});
const open = async (page, id) => {
  await page.goto(`${base}/catalog?open=${id}`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction((sel) => window.__betulaApp === true && document.querySelector(sel)?.checkVisibility(), SWITCH, { timeout: 120000 }).catch(() => problems.push(`${id}: the app or its switch never came`));
  await page.waitForTimeout(400);
};
const switchSays = (page) => page.evaluate((sel) => { const el = document.querySelector(sel); return { pressed: el.getAttribute("aria-pressed"), line: el.querySelector("small")?.textContent || "" }; }, SWITCH);
const near = (a, b, by = 1) => typeof a === "number" && typeof b === "number" && Math.abs(a - b) <= by;
// The dialog once it has come in (it scales in from a little smaller).
const settled = (page) => page.waitForFunction(() => { const el = document.querySelector("dialog.plan-setup[open]"); return el && el.getAnimations({ subtree: true }).every((a) => a.playState !== "running"); }, null, { timeout: 3000 }).catch(() => problems.push("the dialog never came to rest"));

// ---------- a wide screen ----------
{
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  watch(page);
  await open(page, "11112");
  const session = await page.evaluate(() => sessionStorage.getItem("betula.plan.semester"));
  check(/^20\d\d[WS]$/.test(session || ""), `the session's semester is not kept (${session})`);
  const sw = await switchSays(page);
  check(sw.pressed === "false" && /WiSe|SoSe/.test(sw.line), `the switch does not say where it plans (${JSON.stringify(sw)})`);

  // No study: the dialog, no program assumed, the module's programs open with the search.
  await page.click(SWITCH);
  await page.waitForSelector("dialog.plan-setup[open]", { timeout: 3000 }).catch(() => problems.push("no study: no dialog"));
  await page.waitForFunction(() => document.querySelectorAll("dialog.plan-setup .plan-setup-offers > button").length > 0, null, { timeout: 5000 }).catch(() => problems.push("no study: the module's programs never came"));
  await settled(page);
  let d = await dialog(page);
  check(d && d.say === "Was studierst du?", `no study: the sentence is ${d?.say}`);
  check(d && d.offers.length === 3 && d.offers.every((o) => o.req), `no study: not the three compulsory programs (${JSON.stringify(d?.offers)})`);
  check(d && d.offers[0]?.name?.startsWith("Informatik") && d.offers[0]?.detail === "Pflicht · 1. Sem.", `no study: Informatik first, with its semester (${JSON.stringify(d?.offers[0])})`);
  check(d && d.search, "no study: no search under the programs");
  check(d && !d.go, "no study: „Einrichten und einplanen“ before a program is chosen");
  check((await plannedIn(page, "11112")).length === 0, "no study: planned before the dialog's answer");
  // The search: the app's picker, as wide and as tall as the programs over it, and so when open
  // (owner, 2026-10-18: „Erstmal ist es zu weit links, dann wird es breiter wenn man rein klickt").
  check(d && near(d.searchBox?.left, d.offerBox?.left) && near(d.searchBox?.right, d.offerBox?.right) && near(d.searchBox?.height, d.offerBox?.height), `no study: the search is not as wide as the programs (${JSON.stringify([d?.searchBox, d?.offerBox])})`);
  await shot(page, "dialog-open");
  await page.click("#plan-setup-search");
  await page.waitForTimeout(300);
  const searching = await page.evaluate(() => ({ trigger: document.querySelector("#plan-setup-search").getBoundingClientRect().toJSON(), list: document.querySelector("dialog.plan-setup .combo-pop")?.getBoundingClientRect().toJSON() }));
  check(near(searching.trigger.left, d?.searchBox?.left) && near(searching.trigger.right, d?.searchBox?.right) && searching.list && near(searching.list.left, searching.trigger.left) && near(searching.list.right, searching.trigger.right), `no study: the search grows as it opens (${JSON.stringify([searching, d?.searchBox])})`);
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
  check(Boolean(await page.$("dialog.plan-setup[open]")), "no study: Esc in the search closed the dialog");
  // Informatik B.Sc.: the module of its 1st semester, a winter one, this winter: the 1st Fachsemester.
  await page.click("dialog.plan-setup .plan-setup-offers > button >> nth=0");
  await page.waitForTimeout(300);
  d = await dialog(page);
  check(d && d.program?.startsWith("Informatik"), `picked: the program is ${d?.program}`);
  check(d && /Informatik .* im 1\. Fachsemester\./.test(d.say), `picked: the sentence is ${d?.say}`);
  check(d && d.rows.some((row) => row.note.includes("wie im Regelstudienplan (1. Semester)")), `picked: the module not where the plan has it (${JSON.stringify(d?.rows)})`);
  check(d && d.go, "picked: „Einrichten und einplanen“ still waits");
  await shot(page, "dialog-picked");
  // Another program: its row's picker, the module's programs first (owner, the same day: „was ist
  // das für ein clusterfuck, wenn man schon ein Studiengangs ausgewählt hat und den doch nochmal
  // ändern möchte").
  await page.click("#plan-setup-program");
  await page.waitForTimeout(300);
  const change = await page.evaluate(() => ({ groups: [...document.querySelectorAll("dialog.plan-setup .combo-group")].map((g) => g.textContent), first: [...document.querySelectorAll("dialog.plan-setup .combo-option")].slice(0, 3).map((o) => o.querySelector(".combo-label")?.textContent) }));
  check(change.groups[0] === "Studiengänge mit diesem Modul" && change.first[0]?.startsWith("Informatik"), `another program: the module's programs are not first (${JSON.stringify(change)})`);
  await shot(page, "dialog-change");
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
  // The Fachsemester set to 3: the plan's place is behind the student, the module stays, a warning.
  await page.click('dialog.plan-setup button[aria-label="Ein Fachsemester mehr"]');
  await page.click('dialog.plan-setup button[aria-label="Ein Fachsemester mehr"]');
  await page.waitForTimeout(200);
  d = await dialog(page);
  check(d && /im 3\. Fachsemester/.test(d.say) && d.warn?.includes("Laut Regelstudienplan gehört es ins 1. Fachsemester"), `3rd: no warning of the plan's place behind (${JSON.stringify(d)})`);
  // Back into the past: allowed, with a warning.
  await page.click('dialog.plan-setup button[aria-label="Ein Semester früher"]');
  await page.waitForTimeout(200);
  d = await dialog(page);
  check(d && d.warn?.includes("liegt hinter dir.") && d.rows.some((row) => row.note.includes("liegt in der Vergangenheit")), `the past: no warning (${JSON.stringify(d)})`);
  await shot(page, "dialog-past");
  await page.click('dialog.plan-setup button[aria-label="Ein Semester später"]');
  await page.click('dialog.plan-setup button[aria-label="Ein Fachsemester weniger"]');
  await page.click('dialog.plan-setup button[aria-label="Ein Fachsemester weniger"]');
  await page.waitForTimeout(200);
  const target = await page.evaluate(() => document.querySelector("dialog.plan-setup .plan-setup-module .st-fs-value")?.textContent || "");
  // „Einrichten und einplanen": the study stored, the module planned, the note.
  await page.click("#plan-setup-go");
  await page.waitForFunction(() => !document.querySelector("dialog.plan-setup[open]"), null, { timeout: 2000 }).catch(() => problems.push("set up: the dialog stayed"));
  const mine = await page.evaluate((key) => localStorage.getItem(key) || "", MINE);
  check(/^program\t079-82-2008$/m.test(mine) && /^start\t20\d\d[WS]$/m.test(mine), `set up: „Mein Studiengang“ is ${JSON.stringify(mine)}`);
  await noteThere(page, "set up");
  let n = await note(page);
  const first = (await plannedIn(page, "11112"))[0];
  check(Boolean(first), "set up: the module was not planned");
  check(n && target.startsWith(n.at?.replace("WiSe ", "WiSe 20").replace("SoSe ", "SoSe 20").slice(0, 9) || "-"), `set up: the note says ${n?.at}, the dialog ${target}`);
  check(n && n.line?.startsWith("eingeplant · 1. FS"), `set up: the note's line is ${n?.line}`);
  check(n && n.keep === null, "set up: „Weitere Module auch nach …“ before a move");
  check(n && n.close, "set up: no × on a wide screen");
  // Where it stands once it has come in.
  await page.waitForFunction(() => document.querySelector(".wait-note")?.getAnimations().every((a) => a.playState !== "running"), null, { timeout: 2000 }).catch(() => problems.push("set up: the note never came to rest"));
  const once = await note(page);
  await shot(page, "note");

  // A click on the right half of the semester (owner, 2026-10-18: „wenn man auf der rechten Seite
  // drückt, dann nach rechts"): the module a semester later at once, „verschoben", the switch for
  // the next modules.
  await page.mouse.click(n.words.right - 4, n.words.y);
  await page.waitForTimeout(250);
  n = await note(page);
  const later = (await plannedIn(page, "11112"))[0];
  check(later && later !== first, `the right half: still planned in ${later}`);
  check(n && n.line?.startsWith("verschoben · 2. FS"), `the right half: the line is ${n?.line}`);
  check(n && n.keep === "false" && n.keepText?.includes("Weitere Module auch nach"), `the right half: no switch for the next modules (${JSON.stringify(n)})`);
  // A wide screen: one bar across the content, right of the filters, one line high, the module
  // named (owner, 2026-10-18: „über die ganze Breite rechts von der sidebar … etwas weniger hoch";
  // „nicht über die sidebar (z.b. filter) gehen, sondern nur über den content"). In order (the same
  // day: „Du musst halt noch mal gucken wie du die Elemente ein bisschen besser anordnest und das
  // ist halt nicht so durcheinander aussieht"): the check and the name at the left, the semester,
  // „Rückgängig" and × at the right; the switch a row over all of it, where the semester stays,
  // its icon over the check, its words where the name begins, its switch over the ×.
  const content = await page.evaluate(() => ({ side: document.querySelector(".work > .filters").getBoundingClientRect().right, left: document.querySelector(".work > .list").getBoundingClientRect().left }));
  check(once && near(once.box.left, content.left) && once.box.left >= content.side && near(once.box.right, 1432) && once.box.height <= 56, `the bar: not across the content right of the filters, one line high (${JSON.stringify([once?.box, content])})`);
  check(n && once && near(n.stepper.left, once.stepper.left) && near(n.stepper.top, once.stepper.top), `the bar: the semester moved as the switch came (${JSON.stringify([once?.stepper, n?.stepper])})`);
  const l = n?.lines;
  check(n && n.keepBox && n.keepBox.bottom <= n.stepper.top + 1 && near(n.keepBox.left, n.box.left) && near(n.keepBox.right, n.box.right), `the bar: the switch is not a row over all of it (${JSON.stringify([n?.box, n?.keepBox, n?.stepper])})`);
  check(l && near(l.keepIcon.left + l.keepIcon.width / 2, l.mark.left + l.mark.width / 2, 1) && near(l.keepWords.left, l.name.left, 1) && near(l.keepSwitch.right, l.close.right, 1), `the bar: the switch's row does not line up with the line under it (${JSON.stringify(l)})`);
  check(l && l.name.left - l.mark.right < 20 && n.box.right - l.close.right < 24 && l.undo.left > n.stepper.right && l.close.left > l.undo.right && n.stepper.left > l.name.left, `the bar: not the check and the name at the left, the semester, „Rückgängig“ and × at the right (${JSON.stringify([l, n?.stepper])})`);
  check(n && n.title === "Algorithmen und Datenstrukturen", `the bar: the module is not named (${n?.title})`);
  // Dark in both themes, light text on it, and off the dark page (owner, the same day: „dark modus
  // muss natürlich auch dafür funktionieren").
  for (const scheme of ["dark", "light"]) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.waitForTimeout(50);
    const look = await page.evaluate(() => {
      const lum = (s) => { const [r, g, b] = s.match(/[\d.]+/g).map(Number); return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255; };
      const bar = getComputedStyle(document.querySelector(".wait-note"));
      return { bar: lum(bar.backgroundColor), text: lum(bar.color), page: lum(getComputedStyle(document.body).backgroundColor) };
    });
    check(look.bar < 0.2 && look.text > 0.85 && Math.abs(look.bar - look.page) > 0.05, `${scheme}: the bar is not dark with light text, off the page (${JSON.stringify(look)})`);
    if (scheme === "dark") await shot(page, "note-dark");
  }
  // The left half: back; the right half again, where it was (the arrows stay where they are).
  const arrows = await page.evaluate(() => [...document.querySelectorAll(".plan-toast .plan-toast-step")].map((b) => Math.round(b.getBoundingClientRect().left)));
  await page.mouse.click(n.words.left + 4, n.words.y);
  await waitPlanned(page, "11112", [first], "the left half");
  const arrowsBack = await page.evaluate(() => [...document.querySelectorAll(".plan-toast .plan-toast-step")].map((b) => Math.round(b.getBoundingClientRect().left)));
  check(JSON.stringify(arrows) === JSON.stringify(arrowsBack), `the arrows moved with what the semester says (${JSON.stringify([arrows, arrowsBack])})`);
  await page.mouse.click(n.words.right - 4, n.words.y);
  await waitPlanned(page, "11112", [later], "the right half again");
  // ← (the key) back where it was; → again.
  await page.keyboard.press("ArrowLeft");
  await waitPlanned(page, "11112", [first], "←");
  await page.keyboard.press("ArrowRight");
  await waitPlanned(page, "11112", [later], "→");
  // The switch on: the session plans the next modules there; the next module's switch says so.
  await page.click(".plan-toast .plan-toast-keep");
  await page.waitForTimeout(200);
  check((await note(page))?.keep === "true", "keep: the switch did not turn on");
  check(await page.evaluate(() => sessionStorage.getItem("betula.plan.chosen")) === later, "keep: the session's semester is not kept");
  await shot(page, "note-keep");
  // The pointer on the note holds it; „Rückgängig" takes the module out again.
  await page.click(".plan-toast .plan-toast-undo");
  await noteGone(page, "undo");
  await waitPlanned(page, "11112", [], "undo");

  // Analysis I (summer): its switch plans into the semester chosen; P plans it, the note says so.
  await open(page, "11103");
  const chosen = await switchSays(page);
  check(chosen.line.length > 0, "chosen: the switch says nothing");
  await page.keyboard.press("p");
  await noteThere(page, "P");
  const analysis = await plannedIn(page, "11103");
  check(analysis.length === 1 && analysis[0] === later, `P: Analysis I planned in ${JSON.stringify(analysis)}, not the chosen ${later}`);
  n = await note(page);
  check(n && n.keep === "true", `P: the note does not show the chosen semester's switch on (${JSON.stringify(n)})`);
  // Off again, where it is seen.
  await page.click(".plan-toast .plan-toast-keep");
  await page.waitForTimeout(200);
  check(await page.evaluate(() => sessionStorage.getItem("betula.plan.chosen")) === null, "keep off: the session still plans into the chosen semester");
  // Back to the first Fachsemester and no further: „liegt zurück" before the current semester.
  for (let i = 0; i < 6; i++) {
    const back = await page.evaluate(() => document.querySelector(".plan-toast .plan-toast-step")?.disabled);
    if (back !== false) break;
    await page.click(".plan-toast .plan-toast-step >> nth=0");
    await page.waitForTimeout(150);
  }
  n = await note(page);
  check(n && n.back === true && n.line?.includes("1. FS"), `‹: not stopped at the first Fachsemester (${JSON.stringify(n)})`);
  check(n && n.past === (n.line?.includes("liegt zurück") ?? false), `‹: the past not said (${JSON.stringify(n)})`);
  // The left half at the first Fachsemester: nothing.
  const atFloor = await plannedIn(page, "11103");
  await page.mouse.click(n.words.left + 4, n.words.y);
  await page.waitForTimeout(250);
  check(JSON.stringify(await plannedIn(page, "11103")) === JSON.stringify(atFloor), "the left half: past the first Fachsemester");
  await shot(page, "note-past");
  // P on the module planned: out of it, the note says so; „Rückgängig" puts it back.
  const where = await plannedIn(page, "11103");
  await page.waitForTimeout(300);
  const pressedNow = (await switchSays(page)).pressed;
  if (pressedNow === "true") {
    await page.keyboard.press("p");
    await page.waitForTimeout(400);
    n = await note(page);
    check(n && n.said === "Analysis I entfernt", `out: the note says ${JSON.stringify(n)}`);
    await waitPlanned(page, "11103", [], "out");
    await page.click(".plan-toast .plan-toast-undo");
    await waitPlanned(page, "11103", where, "out, undone");
  }

  // Moving on puts it away (owner, 2026-10-18: „sobald erkannt wurde, dass damit nicht interagiert
  // wurde und der Nutzer weiter macht soll es ausgeblendet werden"): ×, Esc, a key that is not its
  // own, another module opened, a long scroll; a little scroll does not (the same day: „ein klein
  // wenig scrollen kein Problem").
  const planOne = async (id) => {
    await open(page, id);
    await page.keyboard.press("p");
    await noteThere(page, `${id}: P`);
    await page.waitForTimeout(600);
  };
  // It waits for interaction (owner, 2026-10-18: „Bau diese notification mal bitte als wait for
  // interaction notification"): it stands as long as nothing is done, and a click elsewhere puts
  // it away.
  await planOne("20210");
  await page.mouse.move(10, 10);
  await page.waitForTimeout(6000);
  check(Boolean(await note(page)), "it waits: the note went by itself");
  await page.click("#rows-top .count");
  await noteGone(page, "a click elsewhere");
  await planOne("20643");
  await page.click(".wait-note .wait-note-close");
  await noteGone(page, "×");
  await planOne("20809");
  await page.mouse.move(700, 400);
  await page.keyboard.press("Escape");
  await noteGone(page, "Esc");
  await planOne("20590");
  await page.mouse.move(700, 400);
  await page.keyboard.press("ArrowDown");
  await noteGone(page, "a key elsewhere");
  await planOne("20850");
  await page.click('.row[data-id="20613"]');
  await noteGone(page, "another module");
  check(page.url().includes("open=20613"), `another module: not opened (${page.url()})`);
  await planOne("20735");
  await page.mouse.move(500, 500);
  await page.mouse.wheel(0, 50);
  await page.waitForTimeout(500);
  check(Boolean(await note(page)), "a little scroll: the note went");
  await page.mouse.wheel(0, 600);
  await noteGone(page, "a long scroll");
  await context.close();
}

// ---------- a semester chosen for the next modules, and the dialog ----------
{
  // The session chose SoSe 2034 for the next modules (owner, 2026-10-18: a dialog at „SoSe 2034 ·
  // 16. FS" without a word why): the dialog says so, and „Aufheben" aims as without it.
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await context.addInitScript(() => { if (!sessionStorage.getItem("betula.plan.seen")) { sessionStorage.setItem("betula.plan.chosen", "2034S"); sessionStorage.setItem("betula.plan.seen", "1"); } });
  const page = await context.newPage();
  watch(page);
  await open(page, "11103");
  await page.click(SWITCH);
  await page.waitForSelector("dialog.plan-setup[open]", { timeout: 3000 }).catch(() => problems.push("chosen: no dialog"));
  await page.waitForTimeout(300);
  let row = await page.evaluate(() => ({ value: document.querySelector("dialog.plan-setup .plan-setup-module .st-fs-value")?.textContent, note: document.querySelector("dialog.plan-setup .plan-setup-module .st-srow-note")?.textContent }));
  check(row.value === "SoSe 2034" && row.note?.startsWith("für weitere Module gewählt"), `chosen: the dialog does not say why (${JSON.stringify(row)})`);
  await page.click("dialog.plan-setup .plan-setup-module .st-back");
  await page.waitForTimeout(200);
  row = await page.evaluate(() => ({ value: document.querySelector("dialog.plan-setup .plan-setup-module .st-fs-value")?.textContent, chosen: sessionStorage.getItem("betula.plan.chosen") }));
  check(/^SoSe 20\d\d$/.test(row.value || "") && row.value !== "SoSe 2034" && !row.chosen, `chosen: „Aufheben“ did not aim as without it (${JSON.stringify(row)})`);
  await page.click("dialog.plan-setup .plan-setup-cancel");
  await context.close();
}

// ---------- a small wide screen ----------
// The note takes the room it has, however narrow the window and wide the sidebar (owner,
// 2026-10-18: „Auf der pc Version bei sehr kleinen Bildschirmen, gibt es das problem, dass die
// notification bar so alles übereinander zeichnet. Bau die mal so, dass das alles responsive je
// nach verfügbaren platz ist"): nothing of it over anything else or out of it; the semester where
// it was as the switch comes, a row over the line; the module named where there is room;
// „entfernt" whole.
{
  // Its parts as they are seen (inside what clips them), what lies over what, and where they are.
  const layout = (page) => page.evaluate(() => {
    const note = document.querySelector(".wait-note");
    if (!note) return null;
    const nb = note.getBoundingClientRect();
    const seen = (el) => {
      const b = el.getBoundingClientRect();
      let r = { left: b.left, right: b.right, top: b.top, bottom: b.bottom };
      for (let up = el.parentElement; up && up !== note; up = up.parentElement) {
        if (getComputedStyle(up).overflowX === "visible") continue;
        const c = up.getBoundingClientRect();
        r = { left: Math.max(r.left, c.left), right: Math.min(r.right, c.right), top: Math.max(r.top, c.top), bottom: Math.min(r.bottom, c.bottom) };
      }
      return r;
    };
    const parts = [".plan-toast-mark", ".plan-toast-title", ".plan-toast-step", ".plan-toast-now > b", ".plan-toast-now > small", ".plan-toast-said-name", ".plan-toast-said-what", ".plan-toast-keep > .icon", ".plan-toast-keep > span:first-of-type", ".plan-toast-switch", ".plan-toast-sep", ".plan-toast-undo", ".wait-note-close"]
      .flatMap((sel) => [...note.querySelectorAll(sel)].filter((el) => el.checkVisibility()).map((el, i) => ({ name: `${sel}${i ? "#" + i : ""}`, r: seen(el) })));
    const hits = [];
    for (let i = 0; i < parts.length; i++) for (let j = i + 1; j < parts.length; j++) {
      const a = parts[i].r, b = parts[j].r;
      if (Math.min(a.right, b.right) - Math.max(a.left, b.left) > 1 && Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 1) hits.push(`${parts[i].name} × ${parts[j].name}`);
    }
    const box = (sel) => { const el = note.querySelector(sel); return el?.checkVisibility() ? el.getBoundingClientRect().toJSON() : null; };
    const what = note.querySelector(".plan-toast-said-what");
    return {
      width: nb.width,
      hits,
      out: parts.filter((p) => p.r.left < nb.left - 1 || p.r.right > nb.right + 1 || p.r.top < nb.top - 1 || p.r.bottom > nb.bottom + 1).map((p) => p.name),
      stepper: box(".plan-toast-stepper"),
      keep: box(".plan-toast-keep"),
      named: Boolean(box(".plan-toast-title")),
      removed: what ? { whole: what.scrollWidth <= what.clientWidth + 1 && seen(what).right >= what.getBoundingClientRect().right - 1, text: note.querySelector(".plan-toast-said").textContent } : null,
    };
  });
  // The window, the sidebar, and whether the note has room for the module's name.
  for (const [width, side, named] of [[1024, 440, false], [1100, 272, true], [1366, 272, true]]) {
    const at = `${width}/${side}`;
    const context = await browser.newContext({ viewport: { width, height: 760 } });
    await context.addInitScript((side) => { localStorage.setItem("betula.filters.width", String(side)); sessionStorage.setItem("betula.plan.asked", "1"); }, side);
    const page = await context.newPage();
    watch(page);
    await open(page, "11112");
    await page.keyboard.press("p");
    await noteThere(page, `${at}: P`);
    await page.waitForTimeout(400);
    const planned = await layout(page);
    await page.keyboard.press("ArrowRight");
    await page.waitForTimeout(400);
    const moved = await layout(page);
    for (const [what, l] of [["planned", planned], ["moved", moved]]) {
      check(l && !l.hits.length && !l.out.length, `${at} ${what}: parts of the note over each other or out of it (${JSON.stringify(l && { hits: l.hits, out: l.out, width: l.width })})`);
    }
    check(planned && moved && near(planned.stepper?.left, moved.stepper?.left) && near(planned.stepper?.top, moved.stepper?.top), `${at}: the semester moved as the switch came (${JSON.stringify([planned?.stepper, moved?.stepper])})`);
    check(moved?.keep && moved.keep.bottom <= moved.stepper.top + 1, `${at}: the switch is not a row over the line (${JSON.stringify([moved?.keep, moved?.stepper])})`);
    check(planned?.named === named, `${at}: the module ${named ? "not " : ""}named (a note ${Math.round(planned?.width)} px wide)`);
    await shot(page, `narrow-${width}-${side}`);
    // P on the module planned takes it out: „entfernt" whole.
    await page.keyboard.press("p");
    await page.waitForTimeout(500);
    const removed = await layout(page);
    check(removed?.removed?.whole && removed.removed.text.endsWith("entfernt") && !removed.hits.length && !removed.out.length, `${at}: „entfernt" not whole (${JSON.stringify(removed && { removed: removed.removed, hits: removed.hits, out: removed.out })})`);
    await context.close();
  }
}

// ---------- the arrow beside „Eingeplant" (owner, 2026-10-20) ----------
// Planned, the switch has an arrow of its own, as „Gemerkt" has; its menu marks the module as
// passed and takes that back, moves it to another semester, opens the Stundenplan and takes it
// out, each with the note and „Rückgängig"; ⇧P opens it, the arrow keys go through it and into the
// semesters, Esc goes back step by step to the arrow.
{
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  const page = await context.newPage();
  watch(page);
  await open(page, "11112");
  const ARROW = ".detail .hero .plan-more";
  // Not planned, the arrow is there as well (folia/design/tasten/konzept.html, „Knopf mit Pfeil"):
  // „Einplanen in …" (planinto.rs), e2e/arrows.mjs walks it.
  check((await page.getAttribute(ARROW, "aria-label").catch(() => null)) === "In ein anderes Semester", "not planned: the arrow is not the one of „Einplanen in …“");
  await page.click(SWITCH);
  await page.waitForSelector(ARROW, { state: "visible", timeout: 3000 }).catch(() => problems.push("planned: no arrow"));
  await page.waitForFunction((key) => (localStorage.getItem(key) || "").includes("\t11112\t"), STORE, { timeout: 4000 }).catch(() => {});
  const [at] = await plannedIn(page, "11112");
  const passedIn = () => page.evaluate((key) => (localStorage.getItem(key) || "").split("\n").map((l) => l.split("\t")).filter((f) => f[0] === "d" && f[2] === "11112").map((f) => f[1]), STORE);
  const menu = () => page.evaluate(() => ({
    entries: [...document.querySelectorAll(".plan-menu.root [role=menuitem]")].map((e) => e.textContent.trim()),
    focus: document.activeElement?.id || document.activeElement?.className || null,
    sub: [...document.querySelectorAll(".plan-menu.sub [role=menuitem]")].map((e) => e.textContent.trim()),
  }));

  // „Als bestanden markieren": the note says so, the switch's small line too, „Rückgängig" takes it back.
  await page.click(ARROW);
  await page.waitForSelector(".plan-menu.root", { timeout: 2000 }).catch(() => problems.push("the arrow opens no menu"));
  let seen = await menu();
  check(JSON.stringify(seen.entries) === JSON.stringify(["Als bestanden markieren", "Verschieben nach", "Im Stundenplan öffnen", "Entfernen"]) && seen.focus === "plan-menu-passed", `the menu: ${JSON.stringify(seen)}`);
  await page.click("#plan-menu-passed");
  await page.waitForFunction(() => !document.querySelector(".plan-menu"), null, { timeout: 2000 }).catch(() => problems.push("„Als bestanden markieren“ leaves the menu open"));
  await page.waitForFunction((sel) => document.querySelector(sel)?.querySelector("small")?.textContent.endsWith("bestanden"), SWITCH, { timeout: 3000 }).catch(() => {});
  check(JSON.stringify(await passedIn()) === JSON.stringify([at]), `passed: ${JSON.stringify(await passedIn())}, not ${at}`);
  check((await switchSays(page)).line.endsWith("· bestanden") || (await switchSays(page)).line.endsWith("bestanden"), `passed: the switch says ${JSON.stringify(await switchSays(page))}`);
  check((await note(page))?.said?.includes("als bestanden markiert"), `passed: the note says ${JSON.stringify((await note(page))?.said)}`);
  await page.click(".plan-toast-undo");
  await page.waitForFunction((key) => !(localStorage.getItem(key) || "").split("\n").some((l) => l.startsWith("d\t")), STORE, { timeout: 3000 }).catch(() => problems.push("„Rückgängig“ leaves the module passed"));

  // „Verschieben nach ›": the semesters beside the menu, „jetzt" at the current one; the last moves it there.
  await page.click(ARROW);
  await page.hover("#plan-menu-move");
  await page.waitForSelector(".plan-menu.sub", { timeout: 2000 }).catch(() => problems.push("„Verschieben nach“ opens no semesters"));
  seen = await menu();
  const plus = (key, n) => { let [y, w] = [Number(key.slice(0, 4)), key[4] === "W"]; for (let i = 0; i < n; i++) [y, w] = w ? [y + 1, false] : [y, true]; return `${y}${w ? "W" : "S"}`; };
  // Without the one it is in; whether each offers it (a winter module).
  const short = (key) => (key.endsWith("W") ? `WiSe ${key.slice(2, 4)}/` : `SoSe ${key.slice(2, 4)}`);
  check(seen.sub.length >= 4 && !seen.sub.some((label) => label.startsWith(short(at))) && seen.sub.every((label) => label.endsWith(label.startsWith("WiSe") ? "angeboten" : "nicht angeboten")), `the semesters: ${JSON.stringify(seen.sub)}`);
  const boxes = await page.evaluate(() => [".plan-menu.root", ".plan-menu.sub"].map((s) => document.querySelector(s).getBoundingClientRect().toJSON()));
  check(boxes[1].left >= boxes[0].right - 4 || boxes[1].right <= boxes[0].left + 4, `the semesters do not stand beside the menu: ${JSON.stringify(boxes)}`);
  await page.click(".plan-menu.sub [role=menuitem]:last-child");
  const later = plus(at, 3);
  await waitPlanned(page, "11112", [later], "moved with the menu");
  await noteThere(page, "moved with the menu");
  check((await switchSays(page)).pressed === "true", `moved: the switch is not pressed where it is now (${JSON.stringify(await switchSays(page))})`);
  check((await note(page))?.line?.includes("verschoben"), `moved: the note says ${JSON.stringify(await note(page))}`);

  // ⇧P opens it, the keys go through it and into the semesters, Esc back step by step.
  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press("Shift+P");
  await page.waitForSelector(".plan-menu.root", { timeout: 2000 }).catch(() => problems.push("⇧P opens no menu"));
  await page.waitForTimeout(100);
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowRight");
  await page.waitForTimeout(100);
  seen = await menu();
  check(seen.sub.length > 0 && (await page.evaluate(() => Boolean(document.activeElement?.closest(".plan-menu.sub")))), `→ does not go into the semesters: ${JSON.stringify(seen)}`);
  await page.keyboard.press("Escape");
  check((await page.evaluate(() => document.activeElement?.id)) === "plan-menu-move" && !(await page.$(".plan-menu.sub")), "Esc does not go back from the semesters to „Verschieben nach“");
  await page.keyboard.press("Escape");
  check(!(await page.$(".plan-menu")) && (await page.evaluate(() => document.activeElement?.classList.contains("plan-more"))), "Esc does not close the menu back to the arrow");
  check(JSON.stringify(await plannedIn(page, "11112")) === JSON.stringify([later]), "the keys changed the plan");

  // „Entfernen": out, the note says so, the arrow goes.
  await page.click(ARROW);
  await page.click("#plan-menu-remove");
  await waitPlanned(page, "11112", [], "taken out with the menu");
  check(await page.evaluate((sel) => document.activeElement === document.querySelector(sel), SWITCH), "taken out: the focus is not on the switch");
  check((await note(page))?.said?.includes("entfernt"), `taken out: the note says ${JSON.stringify(await note(page))}`);
  await page.waitForFunction((sel) => document.querySelector(sel)?.getAttribute("aria-label") === "In ein anderes Semester", ARROW, { timeout: 2000 }).catch(() => problems.push("taken out: the arrow is not the one of „Einplanen in …“ again"));
  await context.close();
}

// ---------- a phone ----------
{
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
  const page = await context.newPage();
  watch(page);
  const cdp = await context.newCDPSession(page);
  await page.goto(`${base}/catalog`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true && document.querySelector(".row-wrap .row"), null, { timeout: 120000 }).catch(() => problems.push("phone: the list never came"));
  await page.waitForTimeout(800);
  // A finger from `at`, `dx` and `dy` in `steps` moves 16 ms apart.
  const drag = async (at, dx, dy, steps = 8) => {
    await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [at] });
    for (let i = 1; i <= steps; i++) {
      await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: at.x + (dx * i) / steps, y: at.y + (dy * i) / steps }] });
      await page.waitForTimeout(16);
    }
    await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  };
  // A row swiped to the right.
  const swipe = async (index) => {
    const at = await page.evaluate((i) => { const r = document.querySelectorAll(".row-wrap")[i].getBoundingClientRect(); return { x: r.left + 40, y: r.top + r.height / 2 }; }, index);
    await drag(at, 220, 0, 10);
    await page.waitForTimeout(900);
  };
  const id0 = await page.evaluate(() => document.querySelectorAll(".row-wrap .row")[0].dataset.id);
  await swipe(0);
  await page.waitForSelector("dialog.plan-setup[open]", { timeout: 3000 }).catch(() => problems.push("phone: the swipe opened no dialog"));
  await page.waitForFunction(() => document.querySelectorAll("dialog.plan-setup .plan-setup-offers > button").length > 0, null, { timeout: 5000 }).catch(() => problems.push("phone: the module's programs never came"));
  await settled(page);
  const d = await dialog(page);
  check(d && Math.abs(d.box.left) <= 1 && Math.abs(d.box.right - 390) <= 1 && Math.abs(d.box.bottom - 844) <= 1, `phone: the dialog is no sheet from below (${JSON.stringify(d?.box)})`);
  check(d && near(d.searchBox?.left, d.offerBox?.left) && near(d.searchBox?.right, d.offerBox?.right) && near(d.searchBox?.height, d.offerBox?.height), `phone: the search is not as wide as the programs (${JSON.stringify([d?.searchBox, d?.offerBox])})`);
  await page.tap("#plan-setup-search");
  await page.waitForTimeout(400);
  const searching = await page.evaluate(() => ({ trigger: document.querySelector("#plan-setup-search").getBoundingClientRect().toJSON(), list: document.querySelector("dialog.plan-setup .combo-pop")?.getBoundingClientRect().toJSON() }));
  check(near(searching.trigger.left, d?.searchBox?.left) && near(searching.trigger.right, d?.searchBox?.right) && searching.list && near(searching.list.left, searching.trigger.left) && near(searching.list.right, searching.trigger.right), `phone: the search grows as it opens (${JSON.stringify([searching, d?.searchBox])})`);
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
  check((await plannedIn(page, id0)).length === 0, "phone: the swipe planned before the dialog's answer");
  await shot(page, "phone-dialog");
  await page.tap("dialog.plan-setup .plan-setup-without");
  await page.waitForTimeout(400);
  check((await plannedIn(page, id0)).length === 1, "phone: „Ohne Studium einplanen“ planned nothing");
  check(await page.evaluate(() => sessionStorage.getItem("betula.plan.asked")) === "1", "phone: the session will ask again");
  let n = await note(page);
  check(n && n.title === null && !n.close, "phone: the module is named in the card, or it has ×");
  // As wide as the bottom bar, over it, under it in the order of what lies on what and flat (owner,
  // 2026-10-18: „keinen schatten auf die navbar werfen").
  const bar = await page.evaluate(() => { const el = document.querySelector(".bottomnav"); const r = el.getBoundingClientRect(); return { left: r.left, right: r.right, top: r.top, z: Number(getComputedStyle(el).zIndex) }; });
  check(n && n.box.bottom <= bar.top && near(n.box.left, bar.left) && near(n.box.right, bar.right), `phone: the note is not over the bottom bar, as wide (${JSON.stringify([n?.box, bar])})`);
  check(n && n.z < bar.z && n.shadow === "none", `phone: the note lies on the bottom bar or casts a shadow (${JSON.stringify([n?.z, n?.shadow, bar.z])})`);
  await shot(page, "phone-note");
  // A tap on the right half of the semester: the module later; the switch comes as a row over the
  // semester, which stays under the finger; between the arrows the warning, not what happened
  // (owner: „Der Rest kann optional ausgeblendet werden").
  const plannedFirst = (await plannedIn(page, id0))[0];
  const wordsBefore = n.words;
  await page.touchscreen.tap(n.words.right - 6, n.words.y);
  await page.waitForTimeout(350);
  n = await note(page);
  const plannedLater = (await plannedIn(page, id0))[0];
  check(plannedLater && plannedLater !== plannedFirst, `phone, the right half: still planned in ${plannedLater}`);
  check(n?.keepBox && n.keepBox.bottom <= n.main.top + 1, `phone: the switch is not a row over the semester (${JSON.stringify([n?.keepBox, n?.main])})`);
  check(n && near(n.words.top, wordsBefore.top), `phone: the semester moved under the finger (${JSON.stringify([wordsBefore, n?.words])})`);
  check(n && n.shown === "kein Angebot", `phone: between the arrows ${JSON.stringify(n?.shown)}, not the warning alone`);
  await shot(page, "phone-note-keep");
  // A finger swiped over the semester: to the left the next one, to the right back; a short one
  // nothing.
  const mid = { x: (n.stepper.left + n.stepper.right) / 2, y: n.stepper.top + n.stepper.height / 2 };
  await drag(mid, -80, 0);
  await page.waitForTimeout(350);
  const swipedOn = (await plannedIn(page, id0))[0];
  check(swipedOn && swipedOn > plannedLater, `phone, swiped to the left: planned in ${swipedOn}, not after ${plannedLater}`);
  await drag(mid, 80, 0);
  await waitPlanned(page, id0, [plannedLater], "phone, swiped to the right");
  await drag(mid, -20, 0, 4);
  await page.waitForTimeout(300);
  check(JSON.stringify(await plannedIn(page, id0)) === JSON.stringify([plannedLater]), "phone: a short swipe moved the module");
  // Down and away; what it did stays.
  const top = await note(page);
  await drag({ x: (top.box.left + top.box.right) / 2, y: top.box.top + 20 }, 0, 70, 6);
  await noteGone(page, "phone, swiped down");
  check(JSON.stringify(await plannedIn(page, id0)) === JSON.stringify([plannedLater]), "phone: swiped down, the plan changed");
  // The next row: planned at once, no dialog; a little scroll leaves the note, a long one puts it
  // away.
  const id1 = await page.evaluate(() => document.querySelectorAll(".row-wrap .row")[1].dataset.id);
  await swipe(1);
  check(!(await page.$("dialog.plan-setup[open]")), "phone: the dialog asked again");
  check((await plannedIn(page, id1)).length === 1, "phone: the second swipe planned nothing");
  await noteThere(page, "phone, the second row");
  await drag({ x: 200, y: 400 }, 0, -40, 6);
  await page.waitForTimeout(500);
  check(Boolean(await note(page)), "phone: a little scroll put the note away");
  await drag({ x: 200, y: 500 }, 0, -300, 12);
  await noteGone(page, "phone, a long scroll");
  // A tap elsewhere puts it away; so does a module opened.
  await page.evaluate(() => scrollTo(0, 0));
  await page.waitForTimeout(400);
  await swipe(4);
  await noteThere(page, "phone, the fifth row");
  await page.waitForTimeout(500);
  await page.tap("#rows-top .count");
  await noteGone(page, "phone, a tap elsewhere");
  await swipe(2);
  await noteThere(page, "phone, the third row");
  await page.waitForTimeout(500);
  const id3 = await page.evaluate(() => document.querySelectorAll(".row-wrap .row")[3].dataset.id);
  await page.tap(`.row-wrap .row[data-id="${id3}"]`);
  await noteGone(page, "phone, a module opened");
  // Planned from the module's sheet, the note over the sheet; the sheet closed, the note goes down
  // with it and out, never up first (owner, 2026-10-18: „wenn man da die Modul Ansicht schließt
  // sollte die notification auch mit eher Animation nach unten raus geschlossen werden").
  await page.waitForFunction(() => document.querySelector(".detail.is-module .hero .plan-toggle[data-action=plan]")?.checkVisibility(), null, { timeout: 10000 }).catch(() => problems.push("phone: no module's sheet"));
  await page.waitForTimeout(800);
  await page.tap(".detail.is-module .hero .plan-toggle[data-action=plan]");
  await noteThere(page, "phone, planned from the sheet");
  await page.waitForTimeout(700);
  await page.evaluate(() => { window.__tops = []; const tick = () => { const n = document.querySelector(".wait-note"); window.__tops.push(n ? n.getBoundingClientRect().top : null); if (window.__tops.length < 60) requestAnimationFrame(tick); }; requestAnimationFrame(tick); });
  await page.tap(".detail.is-module [data-action=close-detail]");
  await noteGone(page, "phone, the sheet closed");
  await page.waitForTimeout(1100);
  const tops = (await page.evaluate(() => window.__tops)).filter((top) => top !== null);
  const up = tops.reduce((most, top, i) => (i ? Math.max(most, tops[i - 1] - top) : 0), 0);
  check(tops.length > 2 && up <= 1 && tops[tops.length - 1] > tops[0], `phone: the note does not go down with the sheet (${JSON.stringify(tops.map(Math.round))})`);
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(`plannote: ${problems.length} problem(s)\n- ` + problems.join("\n- "));
  process.exit(1);
}
console.log("plannote: ok");
