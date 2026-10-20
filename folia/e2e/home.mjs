// The start page (concept A, owner 2026-10-20: „Ja setz Konzept A jetzt mal in die tat um"): on a
// first visit the setup over the photo of the season, three steps on one card — the program, the
// semester the study began in, the modules of the Fachsemester that follows —, and once a study is
// set up the visitor's day: the greeting, the day in a sentence, the clock of the semester and the
// next thing on the photo, under it the day, the week, the exams, the study, the Merkliste and the
// ways into the catalog. „Studium anlegen" plans the semester's modules and marks those of the
// semesters before as passed, and „Rückgängig" puts back what was; „Erst mal umschauen" shows the
// day without a study. The ground names who took the photo shown. On a phone the photo reaches up
// under the search, and once the page is scrolled a veil lies under the search. Nothing loads a
// page. („Über Betula", what the start page said until then, is `e2e/about.mjs`.)
// Needs radix serve-snapshot + folia running and a fresh `bash scripts/build-client.sh`.
//   node e2e/home.mjs [base-url]
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge" });
// What the check looks for is on the page in front (on-page.js).
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const errors = [];
const open = async (options, seed = {}) => {
  const context = await browser.newContext({ ...options, timezoneId: "Europe/Berlin" });
  await context.addInitScript(ON_PAGE);
  // What this browser keeps, once, before the first page reads it.
  await context.addInitScript((seed) => {
    if (sessionStorage.getItem("seeded")) return;
    for (const [key, value] of Object.entries(seed)) localStorage.setItem(key, value);
    sessionStorage.setItem("seeded", "1");
  }, seed);
  const page = await context.newPage();
  page.on("pageerror", (error) => errors.push(String(error).slice(0, 300)));
  await page.goto(base + "/", { waitUntil: "load" });
  await page.waitForFunction(() => window.__betulaApp === true && window.betulaAnswered === null, null, { timeout: 60000 });
  // A page load would lose this (the method of spa.mjs).
  await page.evaluate(() => { window.__marker = 1; });
  return { context, page };
};
const stored = (page) => page.evaluate(() => ({
  program: localStorage.getItem("betula.myprogram.v1") || "",
  plan: (localStorage.getItem("betula.studyplan.v1") || "").split("\n").filter(Boolean),
}));

// Without the app (R9): the setup's first step, whose „Studiengang wählen" leads to all programs,
// the day's skeleton beside it for a browser whose head script finds a program kept, the site and
// its search as structured data, who took each photo, and nothing of anybody's day.
const html = await (await fetch(base + "/")).text();
check(/<a (?=[^>]*\bid="ob-program")(?=[^>]*\bhref="\/programs")[^>]*class="combo-trigger"/.test(html), "server HTML: „Studiengang wählen\" of the setup does not lead to the programs");
check(html.includes('class="day-first-wrap"') && html.includes('class="day-sk-wrap"'), "server HTML: not the setup and the day's skeleton");
check(html.includes('"@type":"WebSite"') && (html.match(/class="photo-credit"/g) || []).length === 5, "server HTML: no WebSite data, or not who took each photo");
check(!html.includes("day-welcome") && !html.includes('class="day-ev'), "server HTML: something of a visitor's day");
check(html.includes('href="/about"') && html.includes('href="/impressum"') && html.includes('href="/datenschutz"'), "server HTML: „Über Betula\" or the legal pages are not linked");

// ---------- a first visit ----------
{
  const { context, page } = await open({ viewport: { width: 1440, height: 900 } });
  const setup = () => page.evaluate(() => ({
    card: Boolean(onPage(".ob")),
    step: onPage(".ob")?.dataset.step,
    title: onPage(".ob-title")?.textContent,
    chips: [...allOnPage(".ob-chip")].map((chip) => chip.textContent.replace(/^\d/, "").trim()),
    brand: Boolean(onPage(".day-brand .wordmark")?.checkVisibility()),
    mine: document.documentElement.classList.contains("mine"),
  }));
  const first = await setup();
  check(first.card && first.step === "1" && first.title === "Was studierst du?" && first.chips.join() === "Studiengang,Beginn,Semester" && first.brand && !first.mine, `the setup does not begin with the program: ${JSON.stringify(first)}`);
  // The photo of the season under it, and in the ground the name of who took it, alone.
  const photo = await page.evaluate(() => {
    const season = document.documentElement.dataset.season;
    const shown = [...allOnPage(".photo-credit")].filter((credit) => credit.checkVisibility()).map((credit) => credit.dataset.photo);
    return { season, image: getComputedStyle(onPage(".day-photo")).backgroundImage, shown };
  });
  check(photo.image.includes(`/assets/season/${photo.season}.webp`) && photo.shown.join() === photo.season, `the photo is not the season's, or the ground names another: ${JSON.stringify(photo)}`);

  // Step 1: the programs in the app's picker; a pick goes on to step 2 at once.
  await page.click("#ob-program >> visible=true");
  await page.waitForSelector("#ob-program-search >> visible=true");
  await page.keyboard.type("Informatik");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => onPage(".ob")?.dataset.step === "2", null, { timeout: 5000 }).catch(() => failures.push("a pick does not lead to the second step"));
  // Step 2: the six semesters back from the current one, the first the usual start, and a field
  // for another Fachsemester, which says what it takes instead of refusing in silence.
  const semesters = await page.evaluate(() => ({
    rows: [...allOnPage(".ob-sem")].map((row) => row.querySelector(".ob-fs")?.textContent.replace(/\s+/g, "")),
    field: Boolean(onPage("#ob-fs")),
    done: onPage(".ob-chip.done > span:last-child")?.textContent.trim(),
  }));
  check(semesters.rows.join() === "1.FS,2.FS,3.FS,4.FS,5.FS,6.FS" && semesters.field && /^Informatik/.test(semesters.done || ""), `the second step is not the semesters: ${JSON.stringify(semesters)}`);
  await page.fill("#ob-fs", "40");
  await page.keyboard.press("Enter");
  check(await page.isVisible(".ob-note.warn >> visible=true"), "a Fachsemester out of range is not said");
  await page.fill("#ob-fs", "3");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => onPage(".ob")?.dataset.step === "3", null, { timeout: 5000 }).catch(() => failures.push("the custom Fachsemester does not lead to the third step"));
  // Step 3: the modules of the third Fachsemester, all ticked, and the switch for the semesters
  // before it on; a row unticked stays out.
  const modules = await page.evaluate(() => ({
    title: onPage(".ob-title")?.textContent,
    rows: allOnPage(".ob-mod").length,
    ticked: allOnPage('.ob-mod[aria-checked="true"]').length,
    past: onPage(".ob-switch")?.getAttribute("aria-checked"),
  }));
  check(modules.title === "Dein 3. Fachsemester" && modules.rows > 1 && modules.ticked === modules.rows && modules.past === "true", `the third step is not the semester's modules: ${JSON.stringify(modules)}`);
  await page.click(".ob-mod >> visible=true >> nth=0");
  check(await page.evaluate(() => onPage(".ob-mod")?.getAttribute("aria-checked") === "false"), "a row does not untick");
  await page.click(".ob-create >> visible=true");
  await page.waitForSelector(".day-welcome >> visible=true", { timeout: 5000 }).catch(() => failures.push("„Studium anlegen\" leads nowhere"));
  const made = await stored(page);
  const planned = made.plan.filter((line) => line.startsWith("m\t") || line.startsWith("p\t"));
  const passed = made.plan.filter((line) => line.startsWith("d\t") || line.startsWith("q\t"));
  check(/^program\t079-82-2008$/m.test(made.program) && /^start\t\d{4}[WS]$/m.test(made.program), `the study is not kept: ${made.program}`);
  check(passed.length > 0 && planned.length >= modules.rows - 1 + passed.length, `the semesters before are not passed, or the semester not planned: ${JSON.stringify(made.plan)}`);
  // The day's parts come with their answers (the semester's facts, the Stundenplan's Termine).
  await page.waitForFunction(() => allOnPage(".day-clock-row > i").length > 5 && allOnPage(".day-head h2").length >= 4, null, { timeout: 15000 }).catch(() => {});
  const day = await page.evaluate(() => ({
    hero: Boolean(onPage(".day-hero:not(.day-first) h1")),
    clock: allOnPage(".day-clock-row > i").length,
    panels: [...allOnPage(".day-head h2")].map((head) => head.textContent),
    mine: document.documentElement.classList.contains("mine"),
    marker: window.__marker === 1 && location.pathname === "/",
  }));
  check(day.hero && day.clock > 5 && day.panels.length >= 4 && day.mine && day.marker, `the day does not follow the setup: ${JSON.stringify(day)}`);
  // „Rückgängig": the setup again, and what this browser kept before.
  await page.click(".day-welcome button:has-text('Rückgängig') >> visible=true");
  await page.waitForSelector(".ob >> visible=true", { timeout: 5000 }).catch(() => failures.push("„Rückgängig\" does not bring the setup back"));
  const undone = await stored(page);
  check(!/^program\t/m.test(undone.program) && undone.plan.length === 0 && !(await page.evaluate(() => document.documentElement.classList.contains("mine"))), `„Rückgängig\" leaves something: ${JSON.stringify(undone)}`);
  // „Erst mal umschauen": the day without a study, and the way back to the setup on it.
  await page.click("button.ob-link >> visible=true");
  await page.waitForSelector(".day-hero:not(.day-first) >> visible=true", { timeout: 5000 }).catch(() => failures.push("„Erst mal umschauen\" does not show the day"));
  check(await page.evaluate(() => Boolean(onPage(".day-next .btn")) && allOnPage(".day-ev").length === 0), "a day without a study shows Termine, or no way back to the setup");
  await page.click(".day-next .btn >> visible=true");
  await page.waitForSelector(".ob >> visible=true", { timeout: 5000 }).catch(() => failures.push("„Studium einrichten\" does not bring the setup back"));
  check(await page.evaluate(() => window.__marker === 1 && location.pathname === "/"), "the setup loaded a page or left the start page");
  await context.close();
}

// ---------- a phone that keeps a study ----------
{
  const seed = {
    "betula.myprogram.v1": "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n",
  };
  const { context, page } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, seed);
  const top = await page.evaluate(() => {
    const hero = onPage(".day-hero").getBoundingClientRect(), bar = onPage(".topbar .search").getBoundingClientRect();
    return { under: hero.top <= 0 && hero.bottom > bar.bottom, wide: Math.round(hero.width) === innerWidth, veil: document.documentElement.hasAttribute("data-veil") };
  });
  check(top.under && top.wide && !top.veil, `phone: the photo does not reach up under the search: ${JSON.stringify(top)}`);
  const crown = () => page.evaluate(() => ({ veil: document.documentElement.hasAttribute("data-veil"), opacity: getComputedStyle(document.querySelector(".crown"), "::before").opacity, height: Math.round(document.querySelector(".crown").getBoundingClientRect().height) }));
  const still = await crown();
  await page.evaluate(() => window.scrollTo(0, 400));
  await page.waitForTimeout(500);
  const scrolled = await crown();
  check(scrolled.veil && scrolled.opacity === "1" && scrolled.height === still.height, `phone: no veil under the search once scrolled, or the bar changed its box: ${JSON.stringify([still, scrolled])}`);
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.waitForTimeout(500);
  check(!(await crown()).veil, "phone: the veil stays at the top");
  check(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "phone: the start page scrolls sideways");
  await context.close();
}

check(errors.length === 0, `page errors: ${errors.join(" | ")}`);
await browser.close();
if (failures.length) {
  console.error(failures.map((f) => "FAIL " + f).join("\n"));
  process.exit(1);
}
console.log("home: ok");
