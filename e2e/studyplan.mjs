// Checks the Studienplan (app/src/pages/studyplan, app/src/studyplan.rs, app/src/myprogram.rs).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node studyplan.mjs [--only=<n>]      (SMOKE_BROWSER_CHANNEL=msedge by default)
// The blocks of the design (G.3), each a function, `--only=3` runs one of them:
//   1 the desktop walk, 2 the overlay and the finder, 4 the phone, 6 privacy — with their features;
//   3 persistence: the plan and „Mein Studiengang" live in this browser, survive a reload, follow
//     another tab, read garbage as an empty plan, and never reach the server's HTML; the plan is an
//     area of its own (its tab, the module beside it and in full, „Zurück");
//   5 without the app: the explanation, no „Plan" in the rail, and on a phone the app's frame;
//   8 the week's labels: a cut first word ends a label, a module's own slots say their kinds.
// Needs the snapshot whose current semester is WiSe 2026/27 (the plans below are of that
// semester); another snapshot skips the blocks with a note. Prints {timings, problems} (block 7)
// and fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const only = Number((process.argv.find((arg) => arg.startsWith("--only=")) || "").slice("--only=".length)) || null;
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const notes = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";

// Informatik B.Sc., first semester, as the import writes it (design B.2): four modules, one
// placeholder, one hidden event, one choice, one hidden kind.
const FS1 = [
  "m\t2026W\t12104\t1790000000\t",
  "m\t2026W\t12107\t1790000000\t",
  "m\t2026W\t12102\t1790000000\t",
  "m\t2026W\t11112\t1790000000\t",
  "p\t1\t2026W\t079-82-2008\t17\t1-1\t6\tfues\t\tFachübergreifendes Studium",
  "k\t2026W\ttutorial",
  "e\t2026W\t149408",
  "c\t2026W\t148369-a4d12",
  "",
].join("\n");
const MINE_FS1 = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n";
// What of the plan must never leave the browser: ids of planned modules, of hidden and chosen
// events, the program's id.
const SECRETS = ["12104", "12107", "149408", "148369", "079-82-2008"];

const watch = (page, requests) => {
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  if (requests) page.on("request", (r) => requests.push(r.url() + " " + (r.postData() || "")));
};
const takeover = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
// A page of a fresh browser, once the app has taken over.
const open = async (options, path, { requests } = {}) => {
  const context = await browser.newContext(options);
  const page = await context.newPage();
  watch(page, requests);
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  const step = async (name, action, until, arg) => {
    const started = Date.now();
    await action();
    try {
      await page.waitForFunction(until, arg, { timeout: 8000 });
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    timings[name] = Date.now() - started;
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
  };
  return { page, step, context };
};
const stored = (page, key) => page.evaluate((key) => localStorage.getItem(key), key);
// An empty plan is the empty week, with „Noch keine Termine" in its middle (owner, 2026-09-25).
const emptyPlan = () => document.querySelector(".sp-body .sp-empty .state-title")?.textContent === "Noch keine Termine";
const aPlan = () => Boolean(document.querySelector(".sp-body .sp-fold")) && !document.querySelector(".sp-body .state, .sp-body .sp-empty");

// ---------- 3: persistence ----------
async function persistence() {
  const requests = [];
  const { page, step, context } = await open({ viewport: { width: 1500, height: 900 } }, "/studyplan", { requests });

  // A fresh browser: nothing planned, the empty week with the way to the catalog's modules that
  // fit it (nothing marked, so nothing to take over), and the plan's tab current.
  await page.waitForFunction(emptyPlan, null, { timeout: 8000 }).catch(() => problems.push("a fresh browser does not show the empty plan"));
  const fresh = await page.evaluate(() => ({
    links: [...document.querySelectorAll(".sp-body .sp-empty .btn")].map((a) => [a.textContent.trim(), a.getAttribute("href")]),
    week: Boolean(document.querySelector(".sp-body .week.fit")),
    tab: document.querySelector('.rail .nav[data-area="studyplan"]')?.getAttribute("aria-current"),
    title: document.querySelector(".crumb h1")?.textContent,
    frame: document.querySelector(".sidebar .panel-head h2")?.textContent,
    hint: document.querySelector(".sidebar .storage-hint")?.textContent ?? "",
    count: Boolean(document.querySelector('.nav[data-area="studyplan"] .nav-count')),
  }));
  check(JSON.stringify(fresh.links) === JSON.stringify([["Zum Katalog", "/catalog?fits=2026W"]]), `the empty plan's ways on: ${JSON.stringify(fresh.links)}`);
  check(fresh.week && fresh.tab === "page" && fresh.title === "Stundenplan" && fresh.frame === "Anpassen" && !fresh.count, `the empty plan: ${JSON.stringify(fresh)}`);
  check(fresh.hint.includes("nur in diesem Browser"), "the sidebar does not say where the plan lives");
  check((await stored(page, PLAN)) === null && (await stored(page, MINE)) === null, "a fresh browser stored something by looking at the plan");

  // The plan of the first semester, stored as the import writes it: shown, and kept by a reload
  // of the bare address — nothing of it rewritten or lost.
  await page.evaluate(([plan, mine, planText, mineText]) => { localStorage.setItem(plan, planText); localStorage.setItem(mine, mineText); }, [PLAN, MINE, FS1, MINE_FS1]);
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(aPlan, null, { timeout: 8000 }).catch(() => problems.push("a stored plan does not show after a reload"));
  await page.evaluate(() => { window.__marker = 1; });
  check((await stored(page, PLAN)) === FS1 && (await stored(page, MINE)) === MINE_FS1, "a reload changed what is stored");
  // The rail's „Plan" counts the planned modules, as the Merkliste counts the marked ones.
  const count = () => page.evaluate(() => { const count = document.querySelector('.rail .nav[data-area="studyplan"] .nav-count'); return count ? [count.textContent, count.getAttribute("aria-label")] : null; });
  check(JSON.stringify(await count()) === JSON.stringify(["4", "4 geplant"]), `the rail's „Plan“ counts ${JSON.stringify(await count())} instead of 4 modules`);
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(aPlan, null, { timeout: 8000 }).catch(() => problems.push("a second reload lost the plan"));
  await page.evaluate(() => { window.__marker = 1; });
  check((await stored(page, PLAN)) === FS1, "a second reload changed the plan");

  // Another tab of the same browser follows the storage: emptied there, empty here, and back.
  const other = await context.newPage();
  watch(other, requests);
  await other.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await takeover(other);
  await other.evaluate((key) => localStorage.removeItem(key), PLAN);
  await page.waitForFunction(emptyPlan, null, { timeout: 8000 }).catch(() => problems.push("another tab emptied the plan, this one did not follow"));
  check((await count()) === null, "an empty plan still has a count in the rail");
  await other.evaluate(([key, text]) => localStorage.setItem(key, text), [PLAN, FS1]);
  await page.waitForFunction(aPlan, null, { timeout: 8000 }).catch(() => problems.push("another tab stored a plan, this one did not follow"));
  check(await page.evaluate(() => window.__marker === 1), "following another tab loaded the page again");
  await other.close();

  // The module beside the plan, and „Modul ansehen" in its place: the address and the tab stay the
  // plan's, the catalog's tab does not hear of the module, and Esc returns through the history.
  const catalogTab = () => page.getAttribute('.rail .nav[data-area="catalog"]', "href");
  const catalogBefore = await catalogTab();
  await step("the module beside the plan", () => page.goto(base + "/studyplan?sem=2026W&open=12104").then(() => takeover(page)).then(() => page.evaluate(() => { window.__marker = 1; })), () => Boolean(document.querySelector(".work > .detail.aside#preview")));
  await step("„Modul ansehen“ fills the plan's place", () => page.click('#preview a[href*="full=1"]'), () => location.pathname === "/studyplan" && location.search.includes("open=12104") && location.search.includes("full=1") && Boolean(document.querySelector(".module-page h2")));
  check(await page.evaluate(() => document.querySelector('.rail .nav[data-area="studyplan"]')?.getAttribute("aria-current") === "page"), "in full: the plan's tab is not the current one");
  check((await catalogTab()) === catalogBefore, `in full: the catalog's tab leads to ${await catalogTab()} instead of ${catalogBefore}`);
  check(await page.evaluate(() => { const back = document.querySelector('[data-action="back"]')?.getAttribute("href") ?? ""; return back.startsWith("/studyplan?") && back.includes("open=12104") && !back.includes("full="); }), "„Zurück“ of the module in full does not lead to the plan with the module beside it");
  const entries = await page.evaluate(() => history.length);
  await step("Esc returns to the plan", () => page.keyboard.press("Escape"), () => location.pathname === "/studyplan" && !location.search.includes("full=") && Boolean(document.querySelector(".work > .detail.aside#preview")));
  check((await page.evaluate(() => history.length)) === entries, "back from the module in full added a history entry instead of walking back");
  await step("Esc closes the module beside the plan", () => page.keyboard.press("Escape"), () => !location.search.includes("open=") && !document.querySelector(".work > .detail"));
  // The rail's „Plan" remembers where the plan was left.
  await step("to the catalog", () => page.click('.rail .nav[data-area="catalog"]'), () => location.pathname.startsWith("/catalog"));
  await step("the tab leads back to the plan as it was left", () => page.click('.rail .nav[data-area="studyplan"]'), () => location.pathname === "/studyplan" && location.search === "?sem=2026W");

  // Garbage in both keys: an empty plan, no error.
  await page.evaluate(([plan, mine]) => {
    localStorage.setItem(plan, "<script>alert(1)</script>\nm\t2026X\t12104\t0\t\nm\t2026W\t../../etc\t0\t\ne\t2026W\t0\nc\t2026W\t148369-zzzzz\n\u0000");
    localStorage.setItem(mine, "program\t<img src=x>\nstart\tsoon\ntown\tberlin\n");
  }, [PLAN, MINE]);
  await page.goto(base + "/studyplan", { waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(emptyPlan, null, { timeout: 8000 }).catch(() => problems.push("garbage in the storage: the page does not show an empty plan"));
  check(!(await page.evaluate(() => document.body.innerHTML.includes("<script>alert") || document.body.innerHTML.includes("<img src=x"))), "garbage in the storage reached the page");

  // The server's HTML knows nothing of any of it, and is the same for every address of the plan.
  const html = await (await context.request.get(base + "/studyplan")).text();
  const again = await (await context.request.get(base + "/studyplan?sem=2026W&view=dates&open=12104&row=148369-aaf38")).text();
  check(!html.includes("12104") && !html.includes("148369") && !html.includes("nav-count"), "the server's HTML of the plan names a module or an event, or counts them");
  check(html === again, "the server's HTML of the plan depends on the address");

  // No request of the session carried the plan (the module's own page aside, opened on purpose).
  const leaked = requests.filter((r) => SECRETS.some((secret) => r.includes(secret)) && !r.includes("/studyplan?"));
  check(leaked.length === 0, `requests that carry the plan: ${leaked.slice(0, 3)}`);
  const beyond = requests.filter((r) => !r.startsWith(base));
  check(beyond.length === 0, `requests to somewhere else: ${beyond.slice(0, 3)}`);
  await context.close();
}

// ---------- 5: without the app ----------
async function withoutTheApp() {
  const context = await browser.newContext({ viewport: { width: 1500, height: 900 }, javaScriptEnabled: false });
  const page = await context.newPage();
  const response = await page.goto(base + "/studyplan?sem=2026W&open=12104", { waitUntil: "domcontentloaded" });
  const plain = await page.evaluate(() => ({
    title: document.querySelector(".sp-body .state-title")?.textContent ?? "",
    h1: document.querySelectorAll("h1").length,
    heading: document.querySelector(".crumb h1")?.textContent,
    rail: document.querySelector('.rail .nav[data-area="studyplan"]')?.getClientRects().length ?? -1,
    bottom: document.querySelector('.bottomnav .nav[data-area="studyplan"]')?.getClientRects().length ?? -1,
    robots: document.querySelector('meta[name="robots"]')?.content ?? "",
    aside: Boolean(document.querySelector(".detail")),
    hint: document.querySelector(".sidebar .storage-hint")?.textContent ?? "",
  }));
  check(response.status() === 200 && plain.title === "Dein Stundenplan erscheint, sobald die App geladen ist." && plain.h1 === 1 && plain.heading === "Stundenplan", `without JavaScript the plan does not explain itself: ${JSON.stringify(plain)}`);
  check(plain.rail === 0 && plain.bottom === 0, `without JavaScript the rail offers the plan: ${JSON.stringify(plain)}`);
  check(plain.robots.startsWith("noindex") && !plain.aside && plain.hint.includes("nur in diesem Browser"), `without JavaScript: ${JSON.stringify(plain)}`);
  // A phone: the frame is the app's, its sidebar a closed sheet, so nothing of the page vanishes
  // when the app takes over (R15); the explanation says where the plan lives.
  await page.setViewportSize({ width: 390, height: 844 });
  const phone = await page.evaluate(() => {
    const sidebar = document.querySelector(".sidebar");
    const state = document.querySelector(".sp-body .state").getBoundingClientRect();
    return { sheet: sidebar.classList.contains("sheet"), closed: sidebar.getBoundingClientRect().top >= innerHeight, state: state.height > 0 && state.bottom <= innerHeight, wide: document.documentElement.scrollWidth > innerWidth };
  });
  check(phone.sheet && phone.closed && phone.state && !phone.wide, `a phone without JavaScript: ${JSON.stringify(phone)}`);
  await context.close();
}

// ---------- 8: the week's labels ----------
// A slot's label breaks between words only; a word wider than its slot ends in „…", and a first
// word that had to be cut ends the label (review 2026-09-25: „Mathe…" over „IT-1"). A module's
// own slots say their kinds in few letters. On a module's page beside the plan and in the Woche,
// on a desktop and on a phone.
async function labels() {
  // Per label: its words as shown, a cut one marked with „…"; the labels where a cut word is
  // followed by a word still shown.
  const probe = () => [...document.querySelectorAll(".week .slot > .l")].flatMap((label) => {
    const bottom = label.getBoundingClientRect().bottom;
    const words = [...label.querySelectorAll(":scope > .w")].filter((word) => word.getBoundingClientRect().top < bottom - 1);
    const cut = (word) => { const text = word.firstElementChild ?? word; return text.scrollWidth > text.clientWidth + 0.5; };
    const bad = words.slice(0, -1).some(cut);
    return bad ? [words.map((word) => (cut(word) ? word.textContent.slice(0, 6) + "…" : word.textContent)).join(" / ")] : [];
  });
  const own = () => [...document.querySelectorAll(".week .slot:not(.planned) > .l")].map((label) => label.textContent).filter((text) => !/^(VL|Ü|Sem|Prak|Proj|Tut|Kons|Exk|Selbst|HA|Sonst)(\/(VL|Ü|Sem|Prak|Proj|Tut|Kons|Exk|Selbst|HA|Sonst))*$/.test(text));
  for (const [width, height] of [[1500, 900], [390, 844]]) {
    const context = await browser.newContext({ viewport: { width, height }, isMobile: width < 600, hasTouch: width < 600 });
    await context.addInitScript(([plan, mine, planText, mineText]) => { if (!localStorage.getItem(plan)) { localStorage.setItem(plan, planText); localStorage.setItem(mine, mineText); } }, [PLAN, MINE, FS1, MINE_FS1]);
    const page = await context.newPage();
    watch(page);
    await page.goto(base + "/catalog/module/12102", { waitUntil: "domcontentloaded" });
    await takeover(page);
    await page.waitForFunction(() => Boolean(document.querySelector(".week .slot.planned")), null, { timeout: 8000 }).catch(() => problems.push(`${width}: the module's week shows no planned module beside it`));
    const moduleWeek = await page.evaluate(probe);
    check(moduleWeek.length === 0, `${width}: a module's week goes on under a cut word: ${JSON.stringify(moduleWeek)}`);
    const words = await page.evaluate(own);
    check(words.length === 0, `${width}: a module's own slots say more than their kinds: ${JSON.stringify(words)}`);
    if (width > 900) {
      await page.goto(base + "/studyplan?sem=2026W", { waitUntil: "domcontentloaded" });
      await takeover(page);
      await page.waitForFunction(() => Boolean(document.querySelector(".week .slot")), null, { timeout: 8000 }).catch(() => problems.push(`${width}: the Woche shows no slot`));
      const woche = await page.evaluate(probe);
      check(woche.length === 0, `${width}: the Woche goes on under a cut word: ${JSON.stringify(woche)}`);
    }
    await context.close();
  }
}

const blocks = { 3: persistence, 5: withoutTheApp, 8: labels };
const status = await (await fetch(base + "/api/status")).json().catch(() => null);
const semester = status?.snapshot?.current_semester;
if (semester !== "2026W") {
  notes.push(`skipped: the snapshot's current semester is ${semester}, the plans of these checks are of 2026W`);
} else {
  for (const [n, block] of Object.entries(blocks)) {
    if (only && Number(n) !== only) continue;
    const started = Date.now();
    await block();
    timings[`block ${n}`] = Date.now() - started;
  }
}

await browser.close();
console.log(JSON.stringify({ timings, problems, notes }, null, 2));
process.exit(problems.length ? 1 : 0);
