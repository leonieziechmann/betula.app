// Checks „Einplanen" as concept E has it (owner, 2026-10-18: „Also ich liebe Option E, das ist mit
// Abstand die eleganteste"; folia/crates/stores/src/studyplan/planning.rs, folia/crates/widgets/src/
// plannote.rs and plansetup.rs). Without a study set up: „Einplanen" opens one dialog — no program
// assumed, the module's programs open (three at most, compulsory first) with the search under them,
// „Einrichten und einplanen" waiting for a program; a program picked fills the Fachsemester to fit
// its Regelstudienplan; the button stores the study and plans the module, and the note says where.
// With a study: the note at the bottom, its ‹ › moving the module at once (back into the past as well,
// to the first Fachsemester, „liegt zurück"), „Weitere Module auch nach …" once moved, which the next
// module's switch then plans into, ← and → as the arrows, „Rückgängig"; a module taken out says so,
// and „Rückgängig" puts it back. On a phone: a row swiped to the right opens the dialog as a sheet,
// „Ohne Studium einplanen" plans and the session asks no more; the note stands over the bottom bar.
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
// What the note says: its semester, its line, whether it can go back, its second row.
const note = (page) => page.evaluate(() => {
  const el = document.querySelector(".plan-toast");
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return {
    at: el.querySelector(".plan-toast-at > b")?.textContent || null,
    line: el.querySelector(".plan-toast-at > small")?.textContent || null,
    past: Boolean(el.querySelector(".plan-toast-at .past")),
    said: el.querySelector(".plan-toast-said")?.textContent || null,
    back: el.querySelector(".plan-toast-step")?.disabled ?? null,
    keep: el.querySelector(".plan-toast-keep")?.getAttribute("aria-checked") ?? null,
    keepText: el.querySelector(".plan-toast-keep")?.textContent || null,
    title: el.querySelector(".plan-toast-title")?.checkVisibility() ? el.querySelector(".plan-toast-title").textContent : null,
    box: { left: r.left, right: r.right, top: r.top, bottom: r.bottom, height: r.height },
    main: el.querySelector(".plan-toast-main")?.getBoundingClientRect().toJSON(),
    keepBox: el.querySelector(".plan-toast-keep")?.getBoundingClientRect().toJSON() ?? null,
    colours: (() => { const cs = getComputedStyle(el); return { bg: cs.backgroundColor, fg: cs.color }; })(),
  };
});
const noteGone = (page, what, timeout = 2000) => page.waitForFunction(() => !document.querySelector(".plan-toast"), null, { timeout }).catch(() => problems.push(`${what}: the note stayed`));
const dialog = (page) => page.evaluate(() => {
  const el = document.querySelector("dialog.plan-setup[open]");
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return {
    say: el.querySelector(".st-study-say")?.textContent || "",
    offers: [...el.querySelectorAll(".plan-setup-offers > button")].map((b) => ({ name: b.querySelector("span")?.textContent, detail: b.querySelector("small")?.textContent, req: b.querySelector("small")?.classList.contains("req") })),
    search: Boolean(el.querySelector(".plan-setup-list .combo")),
    program: el.querySelector(".plan-setup-change > span")?.textContent || null,
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
  let d = await dialog(page);
  check(d && d.say === "Was studierst du?", `no study: the sentence is ${d?.say}`);
  check(d && d.offers.length === 3 && d.offers.every((o) => o.req), `no study: not the three compulsory programs (${JSON.stringify(d?.offers)})`);
  check(d && d.offers[0]?.name?.startsWith("Informatik") && d.offers[0]?.detail === "Pflicht · 1. Sem.", `no study: Informatik first, with its semester (${JSON.stringify(d?.offers[0])})`);
  check(d && d.search, "no study: no search under the programs");
  check(d && !d.go, "no study: „Einrichten und einplanen“ before a program is chosen");
  check((await plannedIn(page, "11112")).length === 0, "no study: planned before the dialog's answer");
  await shot(page, "dialog-open");
  // Informatik B.Sc.: the module of its 1st semester, a winter one, this winter: the 1st Fachsemester.
  await page.click("dialog.plan-setup .plan-setup-offers > button >> nth=0");
  await page.waitForTimeout(300);
  d = await dialog(page);
  check(d && d.program?.startsWith("Informatik"), `picked: the program is ${d?.program}`);
  check(d && /Informatik .* im 1\. Fachsemester\./.test(d.say), `picked: the sentence is ${d?.say}`);
  check(d && d.rows.some((row) => row.note.includes("wie im Regelstudienplan (1. Semester)")), `picked: the module not where the plan has it (${JSON.stringify(d?.rows)})`);
  check(d && d.go, "picked: „Einrichten und einplanen“ still waits");
  await shot(page, "dialog-picked");
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
  await page.waitForSelector(".plan-toast", { timeout: 2000 }).catch(() => problems.push("set up: no note"));
  let n = await note(page);
  const first = (await plannedIn(page, "11112"))[0];
  check(Boolean(first), "set up: the module was not planned");
  check(n && target.startsWith(n.at?.replace("WiSe ", "WiSe 20").replace("SoSe ", "SoSe 20").slice(0, 9) || "-"), `set up: the note says ${n?.at}, the dialog ${target}`);
  check(n && n.line?.startsWith("eingeplant · 1. FS"), `set up: the note's line is ${n?.line}`);
  check(n && n.keep === null, "set up: „Weitere Module auch nach …“ before a move");
  await shot(page, "note");

  // ›: the module a semester later at once, „verschoben", the switch for the next modules.
  await page.click(".plan-toast .plan-toast-step >> nth=1");
  await page.waitForTimeout(250);
  n = await note(page);
  const later = (await plannedIn(page, "11112"))[0];
  check(later && later !== first, `›: still planned in ${later}`);
  check(n && n.line?.startsWith("verschoben · 2. FS"), `›: the line is ${n?.line}`);
  check(n && n.keep === "false" && n.keepText?.includes("Weitere Module auch nach"), `›: no switch for the next modules (${JSON.stringify(n)})`);
  // A wide screen: one bar right of the rail as wide as the panels, one line high with the switch in
  // it, the module named (owner, 2026-10-18: „über die ganze Breite rechts von der sidebar … etwas
  // weniger hoch").
  check(n && Math.abs(n.box.left - 52) <= 1 && Math.abs(n.box.right - 1432) <= 1 && n.box.height <= 56, `the bar: not across the page right of the rail, one line high (${JSON.stringify(n?.box)})`);
  check(n && n.keepBox && Math.abs(n.keepBox.top + n.keepBox.height / 2 - (n.main.top + n.main.height / 2)) <= 2 && n.keepBox.left > n.main.right, `the bar: the switch is not in its line, right of the arrows (${JSON.stringify([n?.main, n?.keepBox])})`);
  check(n && n.title === "Algorithmen und Datenstrukturen", `the bar: the module is not named (${n?.title})`);
  // Dark in both themes, light text on it, and off the dark page (owner, the same day: „dark modus
  // muss natürlich auch dafür funktionieren").
  for (const scheme of ["dark", "light"]) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.waitForTimeout(50);
    const look = await page.evaluate(() => {
      const lum = (s) => { const [r, g, b] = s.match(/[\d.]+/g).map(Number); return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255; };
      const bar = getComputedStyle(document.querySelector(".plan-toast"));
      return { bar: lum(bar.backgroundColor), text: lum(bar.color), page: lum(getComputedStyle(document.body).backgroundColor) };
    });
    check(look.bar < 0.2 && look.text > 0.85 && Math.abs(look.bar - look.page) > 0.05, `${scheme}: the bar is not dark with light text, off the page (${JSON.stringify(look)})`);
    if (scheme === "dark") await shot(page, "note-dark");
  }
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
  await page.waitForSelector(".plan-toast", { timeout: 2000 }).catch(() => problems.push("P: no note"));
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
  // The note goes by itself.
  await page.mouse.move(10, 10);
  await noteGone(page, "its while", 9000);
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
  // A row swiped to the right: the dialog, a sheet from below.
  const swipe = async (index) => {
    const at = await page.evaluate((i) => { const r = document.querySelectorAll(".row-wrap")[i].getBoundingClientRect(); return { x: r.left + 40, y: r.top + r.height / 2 }; }, index);
    await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [at] });
    for (let i = 1; i <= 10; i++) {
      await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: at.x + i * 22, y: at.y }] });
      await page.waitForTimeout(16);
    }
    await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    await page.waitForTimeout(900);
  };
  const id0 = await page.evaluate(() => document.querySelectorAll(".row-wrap .row")[0].dataset.id);
  await swipe(0);
  await page.waitForSelector("dialog.plan-setup[open]", { timeout: 3000 }).catch(() => problems.push("phone: the swipe opened no dialog"));
  const d = await dialog(page);
  check(d && Math.abs(d.box.left) <= 1 && Math.abs(d.box.right - 390) <= 1 && Math.abs(d.box.bottom - 844) <= 1, `phone: the dialog is no sheet from below (${JSON.stringify(d?.box)})`);
  check((await plannedIn(page, id0)).length === 0, "phone: the swipe planned before the dialog's answer");
  await shot(page, "phone-dialog");
  await page.tap("dialog.plan-setup .plan-setup-without");
  await page.waitForTimeout(400);
  check((await plannedIn(page, id0)).length === 1, "phone: „Ohne Studium einplanen“ planned nothing");
  check(await page.evaluate(() => sessionStorage.getItem("betula.plan.asked")) === "1", "phone: the session will ask again");
  const n = await note(page);
  check(n && n.title === null, "phone: the module is named in the card");
  const bar = await page.evaluate(() => document.querySelector(".bottomnav")?.getBoundingClientRect().top);
  check(n && n.box.bottom <= bar && n.box.left >= 8 && n.box.right <= 382, `phone: the note is not over the bottom bar (${JSON.stringify([n?.box, bar])})`);
  await shot(page, "phone-note");
  // ›: the switch comes as a row over the arrows, which stay under the finger.
  const arrow = await page.evaluate(() => document.querySelectorAll(".plan-toast .plan-toast-step")[1]?.getBoundingClientRect().toJSON());
  await page.tap(".plan-toast .plan-toast-step >> nth=1");
  await page.waitForTimeout(350);
  const moved = await note(page);
  const arrowAfter = await page.evaluate(() => document.querySelectorAll(".plan-toast .plan-toast-step")[1]?.getBoundingClientRect().toJSON());
  check(moved?.keepBox && moved.keepBox.bottom <= moved.main.top + 1, `phone: the switch is not a row over the arrows (${JSON.stringify([moved?.keepBox, moved?.main])})`);
  check(arrow && arrowAfter && Math.abs(arrow.top - arrowAfter.top) <= 1, `phone: the arrow moved under the finger (${JSON.stringify([arrow, arrowAfter])})`);
  await shot(page, "phone-note-keep");
  // The next row: planned at once, no dialog.
  const id1 = await page.evaluate(() => document.querySelectorAll(".row-wrap .row")[1].dataset.id);
  await swipe(1);
  check(!(await page.$("dialog.plan-setup[open]")), "phone: the dialog asked again");
  check((await plannedIn(page, id1)).length === 1, "phone: the second swipe planned nothing");
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(`plannote: ${problems.length} problem(s)\n- ` + problems.join("\n- "));
  process.exit(1);
}
console.log("plannote: ok");
