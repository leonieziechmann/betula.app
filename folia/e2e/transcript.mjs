// Checks „Notenübersicht einlesen" in „Mein Studium" (folia/crates/planner/src/study/transcript.rs,
// folia/crates/transcript; owner, 2026-11-18: „Das soll strikt im Browser laufen die datei darf das
// Endnutzer Gerät nicht verlassen und soll auch nicht irgendwie persistent gespeichert werden. Nur
// einmal die module extrahieren+status und dann datei vergessen"), with the made-up Notenübersicht
// of Erika Mustermann (folia_transcript::fixture: Informatik B.Sc. since WiSe 2022/23, twelve
// modules, every status, one recognised without a semester), which it makes with
// `cargo run -p folia-transcript --features fixture --example fixture` (no real one is in the
// repository).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node transcript.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// 1 a fresh browser on its first visit: the box „Aus deiner Notenübersicht einrichten" beside the
//   one that fills from the plan opens the dialog at what it does (the same day: „ein Dialog …, bei
//   dem ganz kurz erklärt wird, dass die Module aus dem Dokument lokal ausgelesen werden, auch mit
//   einem link zur seite wo man das runterladen kann"): its three steps, the way to the BTU's
//   Online-Portal (btu.rs `ONLINE_PORTAL`, in a tab of its own), that the file stays on the device,
//   each part with its icon (2026-11-19: „weniger wie eine wall of text durch bessere visuelle
//   Hierarchie und Icons"), and „Notenübersicht auswählen" beside „Abbrechen", which opens the
//   browser's picker for one file; a file that is no PDF says so, with „Andere Datei wählen", which
//   opens the picker again.
// 2 the made-up file: the bar fills 24, 58, 86, 100 % with its steps ticked off one after the other
//   („PDF geöffnet", „12 Module gelesen", „Alle 12 im Katalog gefunden"), about two seconds in all
//   (`PACE`); then the dialog names the program, since when and that it becomes the study's, what
//   was read (the strip of marks: 9 passed, 2 not, 1 registered; 12 modules), each module with its
//   mark, its semester and „neu", the thesis registered in the summer under way now; „Studium
//   anlegen" sets up Informatik B.Sc. since WiSe 2022/23 with its modules: passed ones planned and
//   marked in their semesters, the recognised one in the Studienbeginn, the ones not passed planned
//   and not marked, the thesis in the current semester; the note says so, and „Rückgängig" takes all
//   of it back, the first visit again.
// 3 taken over again and read once more from the sidebar's „Notenübersicht einlesen": „12 schon so",
//   nothing changes. „Abbrechen" while a file is read closes the dialog, and nothing of that reading
//   comes back: the dialog opens anew at what it does.
// 4 nothing of it leaves the browser or stays in it: every request while reading is a GET of the
//   reader's two files, none carries a body; no worker is left once it has answered; nothing in
//   local or session storage holds the file or a title in the other language.
// 5 a phone: the dialog a sheet, the way to the portal in it, its button across the sheet at its
//   foot and „Abbrechen" thin above it (owner, 2026-11-19: „Notenübersicht auswählen auf die
//   komplette breite unten beim modal und darüber dünn abbrechen. Das gleiche bei übernehmen dann"),
//   so „Studium anlegen" too; with less motion asked for, the reading stands a quarter as long; the
//   overview's „Notenübersicht einlesen".
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
// What was read, or why nothing was: the dialog past its first view and past the bar.
const READ = "dialog[open] .st-transcript:not(.st-tr-intro):not(.st-tr-reading)";

const current = await fetch(base + "/api/status").then((r) => r.json()).then((status) => status?.snapshot?.current_semester).catch(() => null);
if (current !== "2026W") {
  console.log(JSON.stringify({ skipped: `the snapshot's current semester is ${current}, the checks need 2026W` }));
  process.exit(0);
}

// The made-up Notenübersicht, made by the crate that reads it; the portal as btu.rs has it.
const folia = fileURLToPath(new URL("..", import.meta.url));
const pdf = execFileSync("cargo", ["run", "-q", "-p", "folia-transcript", "--features", "fixture", "--example", "fixture"], { cwd: folia, maxBuffer: 1 << 24 });
check(pdf.subarray(0, 5).toString() === "%PDF-", "the fixture is no PDF");
const file = { name: "Notenuebersicht.pdf", mimeType: "application/pdf", buffer: pdf };
const portal = readFileSync(folia + "crates/planner/src/study/btu.rs", "utf8").match(/ONLINE_PORTAL: &str = "([^"]+)"/)?.[1];
check(portal?.startsWith("https://www.b-tu.de/"), `btu.rs's ONLINE_PORTAL: ${portal}`);

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
let during = "start";
const open = async (options) => {
  const context = await browser.newContext(options);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`console (${during}): ` + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push(`pageerror (${during}): ` + String(e).slice(0, 300)));
  const requests = [];
  page.on("request", (r) => requests.push({ method: r.method(), url: r.url(), body: r.postData() || "" }));
  await page.goto(base + "/study", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  return { context, page, requests };
};
const intro = (page) => page.waitForSelector("dialog[open] .st-tr-intro", { timeout: 20000 }).then(() => page.waitForTimeout(300)).catch(() => problems.push(`${during}: the dialog does not open at what it does`));
const outcome = (page) => page.waitForSelector(READ, { timeout: 30000 }).then(() => page.waitForTimeout(400)).catch(() => problems.push(`${during}: nothing read`));
const text = (page) => page.evaluate(() => document.querySelector("dialog[open] .st-transcript")?.innerText ?? "");
const stored = (page, key) => page.evaluate((key) => localStorage.getItem(key) || "", key);
const closed = (page) => page.waitForFunction(() => !document.querySelector("dialog[open]"), null, { timeout: 5000 }).catch(() => problems.push(`${during}: the dialog stays`));
const workers = (page) => page.workers().filter((worker) => worker.url().includes("transcript-worker")).length;
// The foot of the open dialog: its button `id` and „Abbrechen" (the foot's secondary button).
const foot = (page, id) => page.evaluate((id) => {
  const button = document.querySelector(`dialog[open] #${id}`);
  const cancel = button?.parentElement.querySelector(".btn.secondary");
  const sheet = document.querySelector("dialog[open]")?.getBoundingClientRect();
  if (!button || !cancel || !sheet) return null;
  const b = button.getBoundingClientRect(), c = cancel.getBoundingClientRect(), style = getComputedStyle(cancel);
  return {
    across: Math.round(b.width) >= Math.round(sheet.width) - 34,
    above: c.bottom <= b.top + 1 && Math.round(c.width) === Math.round(b.width),
    beside: Math.abs(c.top - b.top) < 2 && c.right <= b.left,
    thin: style.boxShadow === "none" && style.backgroundColor === "rgba(0, 0, 0, 0)" && c.height < b.height,
  };
}, id);

// Picks `file` with a button of the open dialog, which opens the browser's picker.
const choose = async (page, file, button = "#st-transcript-pick") => {
  const chooser = page.waitForEvent("filechooser", { timeout: 8000 }).catch(() => null);
  const clicked = await page.click(`dialog[open] ${button}`, { timeout: 8000 }).then(() => true, () => false);
  const picker = await chooser;
  if (!clicked || !picker) return problems.push(`${during}: ${button} opens no picker`);
  check(!picker.isMultiple(), `${during}: the picker takes several files`);
  await picker.setFiles(file);
};

// What the bar shows from the file picked to what was read: each change of its share, its steps
// and the file's line, with the milliseconds since it was watched; `end` when what was read came
// after it (not what the dialog showed before the file was picked).
const watch = (page) => page.evaluate((read) => new Promise((done) => {
  const seen = [];
  const t0 = performance.now();
  const tick = () => {
    const at = Math.round(performance.now() - t0);
    const bar = document.querySelector("dialog[open] .st-tr-bar[role=progressbar]");
    if (bar) {
      const state = {
        share: Number(bar.getAttribute("aria-valuenow")),
        stages: [...document.querySelectorAll("dialog[open] .st-tr-stage")].map((li) => ["done", "now", "next"].find((s) => li.classList.contains(s)) + " " + li.innerText.trim()),
        file: document.querySelector("dialog[open] .st-tr-file")?.innerText.replace(/\s+/g, " ").trim(),
        done: bar.classList.contains("done"),
      };
      if (JSON.stringify(state) !== JSON.stringify(seen.at(-1)?.state)) seen.push({ at, state });
    }
    if (seen.length && !bar && document.querySelector(read)) return done({ seen, end: at });
    if (at > 20000) return done({ seen, end: null });
    setTimeout(tick, 20);
  };
  tick();
}), READ);
const took = (watched) => (watched.end != null && watched.seen.length ? watched.end - watched.seen[0].at : null);

// ---------- 1, 2, 4: a fresh browser ----------
{
  const { page, requests } = await open({ viewport: { width: 1440, height: 900 }, reducedMotion: "no-preference" });
  during = "first visit";
  await page.waitForSelector(".st-setup", { timeout: 20000 }).catch(() => problems.push("no first visit"));
  const box = await page.evaluate(() => {
    const box = document.querySelector("#st-transcript-setup");
    const fill = document.querySelector("#st-fill");
    const r = box?.getBoundingClientRect(), f = fill?.getBoundingClientRect();
    return box && fill && { text: box.innerText, width: Math.round(r.width) === Math.round(f.width), below: r.top > f.top };
  });
  check(box && box.text.includes("Aus deiner Notenübersicht einrichten") && box.width && box.below, `the box on the first visit: ${JSON.stringify(box)}`);
  await page.click("#st-transcript-setup");
  await intro(page);
  const told = await page.evaluate(() => document.querySelector("dialog[open]")?.innerText ?? "");
  for (const part of ["Notenübersicht einlesen", "Dein Studium in einem Schritt", "Aus der PDF direkt in dein Studium", "Notenübersicht herunterladen", "Zum Online-Portal", "Hier auswählen", "Prüfen und übernehmen", "Die Datei bleibt bei dir", "Nicht hochgeladen, nicht gespeichert", "Keine Noten", "Notenübersicht auswählen"]) {
    check(told.includes(part), `the dialog's first view lacks „${part}“: ${told.slice(0, 500)}`);
  }
  const icons = await page.evaluate(() => {
    const names = (selector) => [...document.querySelectorAll(`dialog[open] ${selector} use`)].map((use) => use.getAttribute("href").split("#")[1]);
    return { flow: names(".st-tr-flow"), steps: names(".st-tr-steps .st-tr-n"), points: names(".st-tr-stays li") };
  });
  check(JSON.stringify(icons) === JSON.stringify({ flow: ["file-text", "arrow-right", "graduation-cap"], steps: ["download", "file-input", "list-checks"], points: ["monitor-smartphone", "cloud-off", "eye-off"] }), `the icons of the first view: ${JSON.stringify(icons)}`);
  const desk = await foot(page, "st-transcript-pick");
  check(desk?.beside && !desk.thin, `on a desktop „Abbrechen“ and „Notenübersicht auswählen“ side by side: ${JSON.stringify(desk)}`);
  const way = await page.evaluate(() => { const a = document.querySelector("dialog[open] #st-transcript-portal"); return a && { href: a.href, target: a.target, rel: a.rel }; });
  check(way && way.href === portal && way.target === "_blank" && way.rel.includes("noopener"), `the way to the Online-Portal: ${JSON.stringify(way)}`);
  check(!(await page.$("dialog[open] .st-tr-bar")), "a bar before a file is picked");

  during = "no pdf";
  await choose(page, { name: "x.pdf", mimeType: "application/pdf", buffer: Buffer.from("no pdf at all") });
  await outcome(page);
  const refused = await text(page);
  check(refused.includes("Das ist keine PDF-Datei"), `a file that is no PDF: ${refused}`);

  during = "read";
  const before = requests.length;
  const watching = watch(page);
  // „Andere Datei wählen" opens the picker again.
  await choose(page, file, "#st-transcript-again");
  const watched = await watching;
  await outcome(page);
  const shares = watched.seen.map(({ state }) => state.share).filter((share, i, all) => i === 0 || share !== all[i - 1]);
  check(JSON.stringify(shares) === "[24,58,86,100]", `the bar went ${JSON.stringify(shares)}`);
  const at = (share) => watched.seen.find(({ state }) => state.share === share)?.state;
  const marks = (share) => at(share)?.stages.map((stage) => stage.split(" ")[0]).join(" ");
  check(marks(24) === "now next next" && marks(58) === "done now next" && marks(86) === "done done now", `the steps one after the other: ${JSON.stringify([24, 58, 86].map(marks))}`);
  const end = watched.seen.at(-1)?.state;
  check(JSON.stringify(end?.stages) === JSON.stringify(["done PDF geöffnet", "done 12 Module gelesen", "done Alle 12 im Katalog gefunden"]), `the steps at the end: ${JSON.stringify(end?.stages)}`);
  check(end?.done && (end?.file || "").startsWith("Deine Notenübersicht ist gelesen") && (at(24)?.file || "").startsWith("Deine Notenübersicht wird gelesen"), `the file's line: ${at(24)?.file} → ${end?.file}`);
  const reading = took(watched);
  check(reading != null && reading >= 1900 && reading < 10000, `reading took ${reading} ms (about two seconds: PACE)`);
  const read = await text(page);
  const tally = await page.evaluate(() => [...document.querySelectorAll("dialog[open] .st-tr-tally li")].map((li) => li.innerText.replace(/\s+/g, " ").trim()));
  check(JSON.stringify(tally) === JSON.stringify(["9 bestanden", "2 nicht bestanden", "1 angemeldet"]), `what was read as a strip: ${JSON.stringify(tally)}`);
  for (const part of ["Informatik B.Sc.", "seit WiSe 2022/23", "Wird dein Studiengang", "12 Module gelesen · 12 neu", "Kommt in dein Studium"]) {
    check(read.toLowerCase().includes(part.toLowerCase()), `the dialog lacks „${part}“: ${read.slice(0, 400)}`);
  }
  check(!read.includes("Nicht im Katalog gefunden"), "a module of the made-up file not found");
  const rows = await page.evaluate(() => [...document.querySelectorAll("dialog[open] .st-transcript .st-area-row")].map((row) => ({
    name: row.querySelector(".st-area-nm b")?.textContent,
    line: row.querySelector(".st-area-nm small")?.textContent,
    mark: row.querySelector(".st-glyph")?.getAttribute("class") ?? "",
  })));
  check(rows.length === 12, `${rows.length} rows`);
  const row = (name) => rows.find((row) => row.name === name) || {};
  check(row("Theoretische Informatik").line === "WiSe 24/25 · neu" && row("Theoretische Informatik").mark.includes("passed"), `Theoretische Informatik: ${JSON.stringify(row("Theoretische Informatik"))}`);
  check(row("Mathematik IT-2 (Lineare Algebra)").mark.includes("failed"), `Lineare Algebra: ${JSON.stringify(row("Mathematik IT-2 (Lineare Algebra)"))}`);
  check(row("Bachelor-Arbeit").line === "WiSe 26/27 · neu · angemeldet im SoSe 26", `the thesis: ${JSON.stringify(row("Bachelor-Arbeit"))}`);
  check(row("Sicherheitsengineering").line === "WiSe 22/23 · neu · anerkannt", `the recognised module: ${JSON.stringify(row("Sicherheitsengineering"))}`);
  check((row("Einführung Information Retrieval").line || "").includes("endgültig nicht bestanden"), `finally failed: ${JSON.stringify(row("Einführung Information Retrieval"))}`);
  check(row("Graph Theory").name === "Graph Theory", "the module taught in English is not Graph Theory (14085)");
  check(await page.$eval("dialog[open] #st-transcript-apply", (b) => b.textContent.trim()).catch(() => ""), "no „Studium anlegen“");
  const asked = requests.slice(before);
  check(asked.every((r) => r.method === "GET" && !r.body), `a request with a body while reading: ${JSON.stringify(asked.filter((r) => r.body).map((r) => r.url))}`);
  const reader = asked.filter((r) => r.url.includes("/pkg/transcript")).map((r) => new URL(r.url).pathname).sort();
  check(JSON.stringify(reader) === JSON.stringify(["/pkg/transcript-worker.js", "/pkg/transcript.wasm"]), `the reader's files: ${JSON.stringify(reader)}`);
  check(asked.every((r) => r.url.startsWith(base + "/pkg/transcript") || r.url.startsWith(base + "/assets/")), `another request while reading: ${JSON.stringify(asked.map((r) => r.url))}`);
  check(!workers(page), "the reader's worker is still there");

  during = "take over";
  await page.click("dialog[open] #st-transcript-apply");
  await closed(page);
  await page.waitForSelector(".study-note", { timeout: 8000 }).catch(() => problems.push("no note"));
  const note = await page.evaluate(() => document.querySelector(".study-note .mark-toast-said")?.textContent);
  check(note === "12 Module aus der Notenübersicht übernommen", `the note: ${note}`);
  const mine = await stored(page, MINE);
  check(mine.includes("program\t079-82-2008") && mine.includes("start\t2022W"), `Mein Studiengang: ${JSON.stringify(mine)}`);
  const plan = await stored(page, PLAN);
  const has = (line) => plan.split("\n").some((stored) => stored === line || stored.startsWith(line + "\t"));
  for (const line of ["m\t2024W\t11787", "d\t2024W\t11787", "m\t2023S\t11113", "m\t2026W\t12333", "d\t2022W\t11684", "m\t2025S\t14460", "d\t2025W\t14085"]) {
    check(has(line), `the plan lacks ${JSON.stringify(line)}: ${JSON.stringify(plan)}`);
  }
  for (const line of ["d\t2023S\t11113", "d\t2026W\t12333", "d\t2025S\t14460"]) check(!has(line), `the plan has ${JSON.stringify(line)}`);
  await page.waitForSelector(".st-mine", { timeout: 8000 }).catch(() => problems.push("no study after it"));
  const card = await page.evaluate(() => document.querySelector(".st-mine")?.innerText);
  check(/9\. Fachsemester/.test(card || "") && (card || "").includes("WiSe 22/23"), `the program's card: ${card}`);
  // Nothing of the file and no title of the other language in the browser's storage.
  const kept = await page.evaluate(() => [localStorage, sessionStorage].flatMap((store) => Object.keys(store).map((key) => key + "=" + store.getItem(key))).join("\n"));
  for (const word of ["%PDF", "Theoretical Computer Science", "Mustermann", "Notenübersicht"]) check(!kept.includes(word), `the browser keeps „${word}“`);

  during = "undo";
  await page.click(".study-note .plan-toast-undo", { timeout: 5000 }).catch(() => problems.push("no „Rückgängig“"));
  await page.waitForSelector(".st-setup", { timeout: 8000 }).catch(() => problems.push("„Rückgängig“ does not bring the first visit back"));
  check(!(await stored(page, MINE)).includes("program\t"), `Mein Studiengang after „Rückgängig“: ${JSON.stringify(await stored(page, MINE))}`);
  check(!(await stored(page, PLAN)).includes("d\t"), `the plan after „Rückgängig“: ${JSON.stringify(await stored(page, PLAN))}`);

  // ---------- 3: once more ----------
  during = "again";
  await page.click("#st-transcript-setup");
  await intro(page);
  await choose(page, file);
  await outcome(page);
  await page.click("dialog[open] #st-transcript-apply");
  await closed(page);
  await page.waitForSelector("#st-transcript", { timeout: 8000 }).catch(() => problems.push("no „Notenübersicht einlesen“ in the sidebar"));
  const sidebar = await page.evaluate(() => document.querySelector("#st-transcript")?.innerText);
  check((sidebar || "").startsWith("Notenübersicht einlesen"), `the sidebar's way: ${sidebar}`);
  const planBefore = await stored(page, PLAN);
  await page.click("#st-transcript");
  await intro(page);
  await choose(page, file);
  await outcome(page);
  const again = await text(page);
  check(again.includes("Dein Studiengang") && again.includes("12 schon so") && !again.includes("neu ·"), `read again: ${again.slice(0, 300)}`);
  check(await page.$eval("dialog[open] #st-transcript-apply", (b) => b.textContent.trim() === "Übernehmen").catch(() => false), "„Übernehmen“ the second time");
  await page.click("dialog[open] .st-dlg-foot .btn.secondary");
  await closed(page);
  check((await stored(page, PLAN)) === planBefore, "reading again changed the plan");

  during = "cancelled";
  await page.click("#st-transcript");
  await intro(page);
  await choose(page, file);
  await page.waitForSelector("dialog[open] .st-tr-bar[aria-valuenow='58']", { timeout: 8000 }).catch(() => problems.push("the bar never stands at 58 %"));
  await page.click("dialog[open] .st-dlg-foot .btn.secondary");
  await closed(page);
  await page.waitForTimeout(2500);
  check(!(await page.$("dialog[open]")), "a reading cancelled comes back");
  check((await stored(page, PLAN)) === planBefore, "a reading cancelled changed the plan");
  check(!workers(page), "the reader's worker is still there after „Abbrechen“");
  await page.click("#st-transcript");
  await intro(page);
  await page.keyboard.press("Escape");
  await closed(page);
  await page.context().close();
}

// ---------- 5: a phone ----------
{
  during = "phone";
  const { page } = await open({ viewport: { width: 393, height: 852 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2, reducedMotion: "reduce" });
  await page.waitForSelector(".st-setup", { timeout: 20000 }).catch(() => problems.push("no first visit on a phone"));
  await page.click("#st-transcript-setup");
  await intro(page);
  const sheet = await page.evaluate(() => { const d = document.querySelector("dialog[open]"); const r = d?.getBoundingClientRect(); return r && { bottom: Math.round(r.bottom), width: Math.round(r.width) }; });
  check(sheet && sheet.width === 393 && sheet.bottom >= 851, `the dialog on a phone: ${JSON.stringify(sheet)}`);
  check(await page.isVisible("dialog[open] #st-transcript-portal"), "no way to the Online-Portal on a phone");
  const first = await foot(page, "st-transcript-pick");
  check(first?.across && first.above && first.thin, `on a phone „Notenübersicht auswählen“ across the foot, „Abbrechen“ thin above it: ${JSON.stringify(first)}`);
  const watching = watch(page);
  await choose(page, file);
  const watched = await watching;
  const quick = took(watched);
  check(quick != null && quick >= 450 && quick < 1500 && watched.seen.at(-1)?.state.share === 100, `with less motion, reading took ${quick} ms to ${watched.seen.at(-1)?.state.share} %`);
  await outcome(page);
  const last = await foot(page, "st-transcript-apply");
  check(last?.across && last.above && last.thin, `on a phone „Studium anlegen“ across the foot, „Abbrechen“ thin above it: ${JSON.stringify(last)}`);
  await page.click("dialog[open] #st-transcript-apply");
  await closed(page);
  await page.waitForSelector("#st-transcript-phone", { state: "attached", timeout: 8000 }).catch(() => problems.push("no „Notenübersicht einlesen“ on a phone's overview"));
  await page.context().close();
}

await browser.close();
console.log(JSON.stringify({ problems }, null, 1));
process.exit(problems.length ? 1 : 0);
