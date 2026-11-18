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
//   one that fills from the plan; a file that is no PDF says so, with „Andere Datei wählen".
// 2 the made-up file: its dialog names the program, since when and that it becomes the study's,
//   what was read (12 modules: 9 passed, 2 not, 1 registered), each module with its mark, its
//   semester and „neu", the thesis registered in the summer under way now; „Studium anlegen" sets
//   up Informatik B.Sc. since WiSe 2022/23 with its modules: passed ones planned and marked in their
//   semesters, the recognised one in the Studienbeginn, the ones not passed planned and not marked,
//   the thesis in the current semester; the note says so, and „Rückgängig" takes all of it back,
//   the first visit again.
// 3 taken over again and read once more from the sidebar's „Notenübersicht einlesen": „12 schon so",
//   nothing changes.
// 4 nothing of it leaves the browser or stays in it: every request while reading is a GET of the
//   reader's two files, none carries a body; no worker is left once it has answered; nothing in
//   local or session storage holds the file or a title in the other language.
// 5 a phone: the overview's „Notenübersicht einlesen", the dialog a sheet.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";

const current = await fetch(base + "/api/status").then((r) => r.json()).then((status) => status?.snapshot?.current_semester).catch(() => null);
if (current !== "2026W") {
  console.log(JSON.stringify({ skipped: `the snapshot's current semester is ${current}, the checks need 2026W` }));
  process.exit(0);
}

// The made-up Notenübersicht, made by the crate that reads it.
const folia = fileURLToPath(new URL("..", import.meta.url));
const pdf = execFileSync("cargo", ["run", "-q", "-p", "folia-transcript", "--features", "fixture", "--example", "fixture"], { cwd: folia, maxBuffer: 1 << 24 });
check(pdf.subarray(0, 5).toString() === "%PDF-", "the fixture is no PDF");
const file = { name: "Notenuebersicht.pdf", mimeType: "application/pdf", buffer: pdf };

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
const dialog = (page) => page.waitForSelector("dialog[open] .st-transcript", { timeout: 30000 }).then(() => page.waitForTimeout(300)).catch(() => problems.push(`${during}: no dialog`));
const text = (page) => page.evaluate(() => document.querySelector("dialog[open] .st-transcript")?.innerText ?? "");
const stored = (page, key) => page.evaluate((key) => localStorage.getItem(key) || "", key);
const closed = (page) => page.waitForFunction(() => !document.querySelector("dialog[open]"), null, { timeout: 5000 }).catch(() => problems.push(`${during}: the dialog stays`));

// ---------- 1, 2, 4: a fresh browser ----------
{
  const { page, requests } = await open({ viewport: { width: 1440, height: 900 } });
  during = "first visit";
  await page.waitForSelector(".st-setup", { timeout: 20000 }).catch(() => problems.push("no first visit"));
  const box = await page.evaluate(() => {
    const box = document.querySelector("#st-transcript-setup");
    const fill = document.querySelector("#st-fill");
    const r = box?.getBoundingClientRect(), f = fill?.getBoundingClientRect();
    return box && fill && { text: box.innerText, width: Math.round(r.width) === Math.round(f.width), below: r.top > f.top };
  });
  check(box && box.text.includes("Aus deiner Notenübersicht einrichten") && box.width && box.below, `the box on the first visit: ${JSON.stringify(box)}`);

  during = "no pdf";
  await page.setInputFiles(".st-setup input[type=file]", { name: "x.pdf", mimeType: "application/pdf", buffer: Buffer.from("no pdf at all") });
  await dialog(page);
  const refused = await text(page);
  check(refused.includes("Das ist keine PDF-Datei"), `a file that is no PDF: ${refused}`);
  check(!!(await page.$("dialog[open] #st-transcript-again")), "no „Andere Datei wählen“");
  await page.keyboard.press("Escape");
  await closed(page);

  during = "read";
  const before = requests.length;
  await page.setInputFiles(".st-setup input[type=file]", file);
  await dialog(page);
  const read = await text(page);
  for (const part of ["Informatik B.Sc.", "seit WiSe 2022/23", "Wird dein Studiengang", "12 Module gelesen · 9 bestanden · 2 nicht bestanden · 1 angemeldet", "12 neu", "Kommt in dein Studium"]) {
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
  check(await page.$eval("dialog[open] #st-transcript-apply", (b) => b.textContent.trim()), "no „Studium anlegen“");
  const asked = requests.slice(before);
  check(asked.every((r) => r.method === "GET" && !r.body), `a request with a body while reading: ${JSON.stringify(asked.filter((r) => r.body).map((r) => r.url))}`);
  const reader = asked.filter((r) => r.url.includes("/pkg/transcript")).map((r) => new URL(r.url).pathname).sort();
  check(JSON.stringify(reader) === JSON.stringify(["/pkg/transcript-worker.js", "/pkg/transcript.wasm"]), `the reader's files: ${JSON.stringify(reader)}`);
  check(asked.every((r) => r.url.startsWith(base + "/pkg/transcript") || r.url.startsWith(base + "/assets/")), `another request while reading: ${JSON.stringify(asked.map((r) => r.url))}`);
  check(!page.workers().some((worker) => worker.url().includes("transcript-worker")), "the reader's worker is still there");

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
  await page.setInputFiles(".st-setup input[type=file]", file);
  await dialog(page);
  await page.click("dialog[open] #st-transcript-apply");
  await closed(page);
  await page.waitForSelector("#st-transcript", { timeout: 8000 }).catch(() => problems.push("no „Notenübersicht einlesen“ in the sidebar"));
  const sidebar = await page.evaluate(() => document.querySelector("#st-transcript")?.innerText);
  check((sidebar || "").startsWith("Notenübersicht einlesen"), `the sidebar's way: ${sidebar}`);
  const planBefore = await stored(page, PLAN);
  await page.setInputFiles(".st-ways input[type=file]", file);
  await dialog(page);
  const again = await text(page);
  check(again.includes("Dein Studiengang") && again.includes("12 schon so") && !again.includes("neu ·"), `read again: ${again.slice(0, 300)}`);
  check(await page.$eval("dialog[open] #st-transcript-apply", (b) => b.textContent.trim() === "Übernehmen"), "„Übernehmen“ the second time");
  await page.click("dialog[open] .st-dlg-foot .btn.secondary");
  await closed(page);
  check((await stored(page, PLAN)) === planBefore, "reading again changed the plan");
  await page.context().close();
}

// ---------- 5: a phone ----------
{
  during = "phone";
  const { page } = await open({ viewport: { width: 393, height: 852 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 });
  await page.waitForSelector(".st-setup", { timeout: 20000 }).catch(() => problems.push("no first visit on a phone"));
  await page.setInputFiles(".st-setup input[type=file]", file);
  await dialog(page);
  const sheet = await page.evaluate(() => { const d = document.querySelector("dialog[open]"); const r = d?.getBoundingClientRect(); return r && { bottom: Math.round(r.bottom), width: Math.round(r.width) }; });
  check(sheet && sheet.width === 393 && sheet.bottom >= 851, `the dialog on a phone: ${JSON.stringify(sheet)}`);
  await page.click("dialog[open] #st-transcript-apply");
  await closed(page);
  await page.waitForSelector("#st-transcript-phone", { state: "attached", timeout: 8000 }).catch(() => problems.push("no „Notenübersicht einlesen“ on a phone's overview"));
  await page.context().close();
}

await browser.close();
console.log(JSON.stringify({ problems }, null, 1));
process.exit(problems.length ? 1 : 0);
