// The choices among a module's events and its extras (owner, 2026-10-19; docs/folia/frontend.md, „A
// module's choices and extras"; folia/design/wahlangebote/konzept.html), on canary's catalog of 2026W:
// Höhere Mathematik T3 (11206) with three exercises at the Zentralcampus, each for other programs, and
// an Aufbaukurs beside them; Musiktheorie (13151) in three groups; Berufspraktische Profilierung
// (12672), one of 14 projects.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node choices.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Needs a snapshot built by a Radix that read the choices, schema 17 or newer (radix
// TestChoicesOnSnapshot writes them into a copy of an older one).
// - Nobody's: the line over the week, the exercises dashed, the Aufbaukurs dotted, the list's headings;
//   the server's page says the same.
// - Mein Studiengang Bauingenieurwesen: its exercise taken, the others behind „Auch zeigen", which
//   brings them back faint under „Für andere Studiengänge".
// - The groups, and the catalogue of projects: no week, five in the list and the rest folded away.
// - The catalog's row: „14 Projekte zur Wahl".
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const MINE = "betula.myprogram.v1";
const BAU = "program\t017-82-2022\nname\tBauingenieurwesen B.Sc. · PO 2022\ncaption\t\nstart\t2026W\n";
const shots = process.env.SMOKE_SHOTS;

const open = async (path, mine) => {
  const context = await browser.newContext({ viewport: { width: 1500, height: 1000 } });
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  if (mine) await context.addInitScript(([key, value]) => { if (!localStorage.getItem(key)) localStorage.setItem(key, value); }, [MINE, mine]);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push(`${path}: the browser app never took over`));
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
// What the Termine of the page in front show: the lines over the week, the slots by their look, the
// headings of the list, its lines and how many of them are faint.
const termine = (page) => page.evaluate(() => {
  const section = onPage("#termine");
  return {
    notes: [...(section?.querySelectorAll(".note.choices") ?? [])].map((note) => note.textContent.trim()),
    slots: [...(section?.querySelectorAll(".week .slot") ?? [])].map((slot) => [slot.className, slot.textContent.trim()]),
    heads: [...(section?.querySelectorAll(".choice-head") ?? [])].map((head) => head.textContent.trim()),
    faint: section?.querySelectorAll(".ev.faint").length ?? 0,
    lines: section?.querySelectorAll(".ev").length ?? 0,
    more: section?.querySelector(".evmore > summary")?.textContent.trim() ?? "",
  };
});
const alt = (slots) => slots.filter(([c]) => c.split(" ").includes("alt")).length;

// ---------- nobody's: every exercise open ----------
{
  const { page, context } = await open("/catalog/module/11206");
  await until(page, "the line of the choice", () => Boolean(onPage("#termine .note.choices")));
  const t = await termine(page);
  check(t.notes.some((n) => n.startsWith("Die Übung gibt es dreimal, je für andere Studiengänge. Du besuchst eine.")), `the line over the week: ${JSON.stringify(t.notes)}`);
  check(t.notes.some((n) => n.startsWith("Zusätzlich, freiwillig: Aufbaukurs Höhere Mathematik - T3")), `the extra's line: ${JSON.stringify(t.notes)}`);
  check(alt(t.slots) === 3, `three exercises dashed: ${JSON.stringify(t.slots)}`);
  check(t.slots.some(([c, text]) => c.includes("extra") && text.startsWith("+")), `the Aufbaukurs dotted: ${JSON.stringify(t.slots)}`);
  check(["Pflicht", "Eine der 3 Übungen", "Zusätzlich, freiwillig"].every((h) => t.heads.includes(h)), `the list's headings: ${JSON.stringify(t.heads)}`);
  if (shots) await page.screenshot({ path: `${shots}/choices-all.png`, fullPage: true });
  // The server's page says the same without the app.
  const html = await (await fetch(base + "/catalog/module/11206")).text();
  check(html.includes("je für andere Studiengänge") && html.includes("choice-head") && html.includes("Zusätzlich, freiwillig"), "the server's page shows the choice");
  await context.close();
}

// ---------- Mein Studiengang: its exercise, the others behind „Auch zeigen" ----------
{
  const { page, context } = await open("/catalog/module/11206", BAU);
  await until(page, "Mein Studiengang's exercise", () => [...document.querySelectorAll("#termine .note.choices")].some((n) => n.textContent.includes("Die Übung für Bauingenieurwesen")));
  let t = await termine(page);
  check(t.notes.some((n) => n.includes("Di 07:30") && n.includes("Auch angeboten, für andere Studiengänge: Mi 13:45 und Do 11:30")), `the line: ${JSON.stringify(t.notes)}`);
  check(alt(t.slots) === 0, `its exercise taken, the others left out: ${JSON.stringify(t.slots)}`);
  check(!t.heads.includes("Eine der 3 Übungen") && t.heads.includes("Pflicht"), `its exercise among what everyone attends: ${JSON.stringify(t.heads)}`);
  const alone = t.slots.length;
  if (shots) await page.screenshot({ path: `${shots}/choices-mine.png`, fullPage: true });
  await page.click("#termine .note.choices .courses-toggle >> visible=true");
  await until(page, "„Auch zeigen\"", () => document.querySelectorAll("#termine .week .slot.faint").length >= 2);
  t = await termine(page);
  check(t.slots.length === alone + 2, `the other two back: ${JSON.stringify(t.slots)}`);
  check(t.heads.includes("Für andere Studiengänge") && t.faint >= 2, `the list: ${JSON.stringify(t.heads)}, ${t.faint} faint`);
  if (shots) await page.screenshot({ path: `${shots}/choices-mine-shown.png`, fullPage: true });
  await page.click("#termine .note.choices .courses-toggle >> visible=true");
  await until(page, "„Wieder ausblenden\"", () => document.querySelectorAll("#termine .week .slot.faint").length === 0);
  check(await page.evaluate(() => window.__marker === 1), "the page was loaded again");
  await context.close();
}

// ---------- groups, and a catalogue of projects ----------
{
  const { page, context } = await open("/catalog/module/13151");
  await until(page, "the groups", () => (onPage("#termine .note.choices")?.textContent ?? "").includes("gibt es in 3 Gruppen"));
  const t = await termine(page);
  check(alt(t.slots) === 6 && t.heads.includes("Eine der 3 Gruppen"), `six events of three groups dashed: ${JSON.stringify(t.slots)} ${JSON.stringify(t.heads)}`);
  await context.close();
}
{
  const { page, context } = await open("/catalog/module/12672");
  await until(page, "the projects", () => (onPage("#termine .note.choices")?.textContent ?? "").includes("eines der 14 Projekte"));
  const t = await termine(page);
  check(t.slots.length === 0, `no week: ${JSON.stringify(t.slots)}`);
  check(t.heads.includes("Eines der 14 Projekte") && t.more === "9 weitere anzeigen", `five listed, nine folded: ${JSON.stringify(t.heads)} „${t.more}"`);
  if (shots) await page.screenshot({ path: `${shots}/choices-projects.png`, fullPage: true });
  await context.close();
}

// ---------- the catalog's row ----------
{
  const { page, context } = await open("/catalog?q=Berufspraktische%20Profilierung");
  await until(page, "the row's choice", () => [...document.querySelectorAll(".row .row-offer")].some((offer) => offer.textContent.trim() === "14 Projekte zur Wahl"));
  if (shots) await page.screenshot({ path: `${shots}/choices-catalog.png` });
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log("choices: ok");
