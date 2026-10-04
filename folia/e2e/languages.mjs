// Checks the browser app in another language (docs/folia/i18n.md): on `/en/…` it takes over like on the
// German pages, every step stays under `/en` and inside the app (no page load), every link it
// writes leads to an English page or to what has no language, and the switch in the rail leads to
// the same page in German, as a page load of its own. Then the language a browser opens the site in
// (`app::languages::language_script`): its own on the first visit, kept in `localStorage`, the
// kept one on every later visit, the switch's once it was used, the default for a browser that
// speaks none of the site's languages. (An automated browser is left where it is, so the checks
// of the German pages run as they are; here the browser says it is none.)
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node languages.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a page load after takeover, a console error, a link out of English, or a step that does
// not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const page = await browser.newPage({ viewport: { width: 1500, height: 900 } });
const problems = [];
page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));

// The links of the page that lead to a German page: paths of the site outside `/en`, but for
// the switch and the alternates, which name their language (`hreflang`), and files.
const outOfEnglish = () => page.evaluate(() => {
  const english = (path) => path === "/en" || /^\/en[/?#]/.test(path);
  const files = /^\/(assets|pkg|api|favicon|apple-touch-icon|sw\.js)/;
  return [...document.querySelectorAll("a[href], form[action]")]
    .filter((el) => !el.hasAttribute("hreflang"))
    .map((el) => el.getAttribute("href") ?? el.getAttribute("action"))
    .filter((address) => address.startsWith("/") && !address.startsWith("//") && !english(address) && !files.test(address));
});

const step = async (name, action, until, arg = null) => {
  await action();
  try {
    await page.waitForFunction(until, arg, { timeout: 8000 });
    // The page before stays as a picture in front of the new one while its answers come (pending.rs, `hold`).
    await page.waitForFunction(() => !document.querySelector(".pending-page.held"), null, { timeout: 2000 });
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
    return;
  }
  if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
  const out = await outOfEnglish();
  if (out.length) problems.push(`${name}: ${out.length} links out of English, e.g. ${[...new Set(out)].slice(0, 5).join(" ")}`);
  if (!(await page.evaluate(() => document.documentElement.lang === "en"))) problems.push(`${name}: the document is not in English`);
};

await page.goto(base + "/en/catalog", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
await page.evaluate(() => { window.__marker = 1; });
await step("the catalogue in English", async () => {}, () => document.querySelector(".skip-link")?.textContent === "Skip to content" && document.querySelectorAll(".rows a.row").length > 5);

await step("a filter", () => page.click('#filters a.chip:has-text("Winter")'), () => location.pathname === "/en/catalog" && location.search.includes("turnus=winter"));
await step("a preview", () => page.click("a.row >> nth=2"), () => location.pathname === "/en/catalog" && location.search.includes("open=") && document.querySelector(".detail h2"));
await step("F opens the module's page", () => page.keyboard.press("f"), () => location.pathname.startsWith("/en/catalog/module/") && document.querySelector(".module-page"));
await step("Esc goes back to the list", () => page.keyboard.press("Escape"), () => location.pathname === "/en/catalog" && document.querySelector(".rows"));
await step("my studies", () => page.click('.rail a.nav[data-area="programs"]'), () => location.pathname === "/en/study" && document.querySelector(".st-setup h1")?.textContent === "Set up your studies once");
await step("the programmes", () => page.click('#sidebar a[href="/en/programs"]'), () => location.pathname === "/en/programs" && document.querySelector(".page"));
await step("a programme", () => page.click('.page a[href^="/en/programs/"] >> nth=0'), () => /^\/en\/programs\/[^/]+/.test(location.pathname));
await step("its electives and areas", () => page.click('a[href*="/areas"] >> nth=0'), () => location.pathname.endsWith("/areas"));
await step("the saved modules", () => page.click('.rail a.nav[data-area="bookmarks"]'), () => location.pathname === "/en/bookmarks");
await step("the timetable", () => page.click('.rail a.nav[data-area="studyplan"]'), () => location.pathname === "/en/studyplan");
await step("the home page", () => page.click('.rail a.nav[data-area="home"]'), () => location.pathname === "/en");
await step("the search of the top bar", () => page.fill("#topsearch", "mathe"), () => location.pathname === "/en/catalog" && location.search.includes("q=mathe"));

// The switch: the same page in German, loaded anew.
const english = new URL(page.url());
await page.click('.rail .languages a[hreflang="de"]');
await page.waitForFunction(() => document.documentElement.lang === "de" && window.__betulaApp === true, null, { timeout: 60000 }).catch(() => problems.push("the switch did not lead to a German page"));
const german = new URL(page.url());
if (german.pathname !== english.pathname.replace(/^\/en/, "") || german.search !== english.search) problems.push(`the switch led from ${english.pathname}${english.search} to ${german.pathname}${german.search}`);
if (await page.evaluate(() => window.__marker === 1)) problems.push("the switch stayed in the app of the other language");

// The language of a visit.
const visitor = async (locale) => {
  const context = await browser.newContext({ locale, viewport: { width: 1500, height: 900 } });
  await context.addInitScript(() => Object.defineProperty(Navigator.prototype, "webdriver", { get: () => false, configurable: true }));
  const tab = await context.newPage();
  tab.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  return { context, tab };
};
const arrive = async (tab, path, expected, name) => {
  // The page may be replaced while it loads: that is the point.
  await tab.goto(base + path, { waitUntil: "commit" }).catch(() => {});
  await tab.waitForFunction((want) => location.pathname + location.search === want, expected, { timeout: 15000 }).catch(() => {});
  await tab.waitForLoadState("domcontentloaded").catch(() => {});
  const at = await tab.evaluate(() => location.pathname + location.search);
  if (at !== expected) problems.push(`${name}: ${path} ended at ${at}, not ${expected}`);
  return tab.evaluate(() => localStorage.getItem("betula.language"));
};
{
  const { context, tab } = await visitor("en-US");
  let kept = await arrive(tab, "/catalog?turnus=winter", "/en/catalog?turnus=winter", "an English browser's first visit");
  if (kept !== "en") problems.push(`the first visit kept ${kept}, not en`);
  await arrive(tab, "/programs", "/en/programs", "the next visit of the English browser");
  await tab.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the app did not take over on /en/programs"));
  await tab.click('.rail .languages a[hreflang="de"]');
  await tab.waitForFunction(() => location.pathname === "/programs" && document.documentElement.lang === "de", null, { timeout: 15000 }).catch(() => problems.push(`the switch led to ${tab.url()}`));
  kept = await tab.evaluate(() => localStorage.getItem("betula.language"));
  if (kept !== "de") problems.push(`the switch kept ${kept}, not de`);
  await arrive(tab, "/en/catalog", "/catalog", "a visit after the switch to German");
  await context.close();
}
{
  const { context, tab } = await visitor("de-DE");
  const kept = await arrive(tab, "/en", "/", "a German browser's first visit");
  if (kept !== "de") problems.push(`the German browser kept ${kept}, not de`);
  await context.close();
}
{
  const { context, tab } = await visitor("fr-FR");
  const kept = await arrive(tab, "/en/programs", "/programs", "a browser in none of the site's languages");
  if (kept !== "de") problems.push(`the French browser kept ${kept}, not the default`);
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log("languages: ok");
