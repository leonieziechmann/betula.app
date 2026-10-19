// The courses of a module held at two campuses (owner, 2026-10-19; docs/folia/frontend.md, „A module
// held at two campuses shows the course of the program"): the Programmierpraktikum (12102) of 2026W
// at the Zentralcampus for Informatik and in Sachsendorf for Medizininformatik.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node courses.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Needs a snapshot of schema 15 built by a Radix that read the courses (radix TestTracksOnSnapshot
// writes them into a copy of an older one).
// - Nobody's: every course in its colour, a heading for each, the line over the week naming both.
// - Mein Studiengang Medizininformatik: only Sachsendorf's Termine; „Auch zeigen" brings the
//   Zentralcampus's back faint, and hides them again.
// - The catalog filtered by Informatik while Mein Studiengang is Medizininformatik: the Zentralcampus
//   highlighted, Sachsendorf faint.
// - The Stundenplan of Medizininformatik with 12102 planned: the Zentralcampus's events hidden, and
//   the line „Abgeleitet: …" names the course.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const MINE = "betula.myprogram.v1";
const MEDINF = "program\t247-82-2016\nname\tMedizininformatik B.Sc. · PO 2016\ncaption\t\nstart\t2026W\n";
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
// What the Termine of the page in front show: the line over the week, the slots by their look, the
// headings and lines of the list.
const termine = (page) => page.evaluate(() => {
  const section = onPage("#termine");
  return {
    note: section?.querySelector(".note.courses")?.textContent.trim() ?? "",
    slots: [...(section?.querySelectorAll(".week .slot") ?? [])].map((slot) => slot.className),
    heads: [...(section?.querySelectorAll(".course-head") ?? [])].map((head) => [head.className, head.textContent.trim()]),
    faint: section?.querySelectorAll(".ev.faint").length ?? 0,
    lines: section?.querySelectorAll(".ev").length ?? 0,
  };
});

// ---------- nobody's: every course ----------
{
  const { page, context } = await open("/catalog/module/12102");
  await until(page, "the courses", () => Boolean(onPage("#termine .note.courses")));
  const t = await termine(page);
  check(t.note.includes("An zwei Standorten angeboten: Zentralcampus Cottbus und Cottbus-Sachsendorf"), `the line over the week: ${t.note}`);
  check(t.slots.some((c) => c.includes("course-1")) && t.slots.some((c) => c.includes("course-2")), `every course in its colour: ${JSON.stringify(t.slots)}`);
  check(t.heads.some(([c, text]) => c.includes("course-2") && text.startsWith("Cottbus-Sachsendorf · für Medizininformatik")), `a heading per course: ${JSON.stringify(t.heads)}`);
  check(t.faint === 0, "nothing is faint");
  if (shots) await page.screenshot({ path: `${shots}/courses-all.png`, fullPage: true });
  // The server's page says the same without the app.
  const html = await (await fetch(base + "/catalog/module/12102")).text();
  check(html.includes("An zwei Standorten angeboten") && html.includes("course-head"), "the server's page shows the courses");
  await context.close();
}

// ---------- Mein Studiengang: its course, the others behind „Auch zeigen" ----------
{
  const { page, context } = await open("/catalog/module/12102", MEDINF);
  await until(page, "Mein Studiengang's course", () => (onPage("#termine .note.courses")?.textContent ?? "").includes("Nur die Termine für Medizininformatik"));
  let t = await termine(page);
  check(t.note.includes("(Cottbus-Sachsendorf)") && t.note.includes("Auch angeboten: Zentralcampus Cottbus"), `the line: ${t.note}`);
  check(t.heads.length >= 1 && t.heads[0][1].startsWith("Cottbus-Sachsendorf") && !t.heads.some(([, text]) => text.startsWith("Zentralcampus")), `only Sachsendorf's heading: ${JSON.stringify(t.heads)}`);
  check(!t.slots.some((c) => c.includes("course-")), `no colours: ${JSON.stringify(t.slots)}`);
  const alone = t.slots.length;
  if (shots) await page.screenshot({ path: `${shots}/courses-mine.png`, fullPage: true });
  await page.click("#termine .courses-toggle >> visible=true");
  await until(page, "„Auch zeigen\"", () => onPage("#termine .slot.faint") !== null);
  t = await termine(page);
  check(t.slots.length > alone && t.slots.filter((c) => c.includes("faint")).length === t.slots.length - alone, `the others faint: ${JSON.stringify(t.slots)}`);
  if (shots) await page.screenshot({ path: `${shots}/courses-mine-shown.png`, fullPage: true });
  await page.click("#termine .courses-toggle >> visible=true");
  await until(page, "„Wieder ausblenden\"", () => onPage("#termine .slot.faint") === null);
  check(await page.evaluate(() => window.__marker === 1), "the page was loaded again");
  await context.close();
}

// ---------- another program: its course highlighted ----------
{
  const { page, context } = await open("/catalog?program=bachelor-informatik-2008&open=12102", MEDINF);
  await until(page, "Informatik's course", () => (onPage("#termine .note.courses")?.textContent ?? "").includes("Hervorgehoben: die Termine für Informatik"));
  const t = await termine(page);
  check(t.note.includes("(Zentralcampus Cottbus)") && t.note.includes("Blass: Cottbus-Sachsendorf"), `the line: ${t.note}`);
  check(t.slots.some((c) => c.includes("faint")) && t.slots.some((c) => !c.includes("faint")), `Sachsendorf faint: ${JSON.stringify(t.slots)}`);
  check(t.faint > 0 && t.faint < t.lines, `the list: ${t.faint} of ${t.lines} faint`);
  if (shots) await page.screenshot({ path: `${shots}/courses-highlight.png`, fullPage: true });
  await context.close();
}

// ---------- the Stundenplan of Medizininformatik ----------
{
  const { page, context } = await open("/catalog/module/12102", MEDINF);
  await until(page, "„Einplanen\"", () => Boolean(onPage(".hero-line .plan-toggle")));
  await page.click(".hero-line .plan-toggle >> visible=true");
  // The plan is written after the next frame: the button pressed and no longer busy.
  await until(page, "the module planned", () => {
    const button = onPage(".hero-line .plan-toggle");
    return button?.getAttribute("aria-pressed") === "true" && !button.hasAttribute("aria-busy");
  });
  await page.goto(base + "/studyplan?sem=2026W", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the Stundenplan: the browser app never took over"));
  await until(page, "the derived line", () => (onPage(".sp-derived")?.textContent ?? "").includes("in Cottbus-Sachsendorf laut Studiengang"));
  if (shots) await page.screenshot({ path: `${shots}/courses-studyplan.png`, fullPage: true });
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log("courses: ok");
