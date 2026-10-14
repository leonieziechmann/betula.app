// Checks the profiles (folia/crates/stores/src/profiles.rs) and their page (folia/crates/profile).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node profile.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Desktop: the rail's way to the page; a new profile is empty and the active one, its marks under
// its own key while the first profile's stay under the plain one; a switch in the sidebar brings
// a profile's marks back (the rail counts them), renaming, „Profil leeren" and deleting with
// „Rückgängig"; another tab of the same browser follows a switch, and the Merkliste open there
// shows the other profile's list; light, dark or the system's (with the switch of the rail); a
// profile kept as a file and made again of it, and a file that is none; „Mein Studiengang" of the
// active profile read by the script in <head>. Privacy: no request carries a profile's name or a
// mark, and none leaves the site. Phone: the way to the page on the start page's brand line, the
// profiles before the page, no sideways scrolling. English. Without the app: the page explains
// itself and is `noindex`, and nothing of it shows.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { readFile, writeFile, mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
const MARKS = "betula.bookmarks.v1";
const LIST = "betula.profiles.v1";
const NAME = "Zweitstudium";

const watch = (page, requests) => {
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  if (requests) page.on("request", (r) => requests.push(r.url() + " " + (r.postData() || "")));
};
const takeover = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
const stepper = (page) => async (name, action, until, arg) => {
  const started = Date.now();
  await action();
  try {
    await page.waitForFunction(until, arg, { timeout: 8000 });
    await page.waitForFunction(() => !document.querySelector(".pending-page.held"), null, { timeout: 2000 });
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
  }
  timings[name] = Date.now() - started;
  if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
};
const open = async (context, path, requests) => {
  const page = await context.newPage();
  watch(page, requests);
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  return { page, step: stepper(page) };
};
const kept = (page, key) => page.evaluate((key) => localStorage.getItem(key), key);
const ids = (text) => (text || "").split("\n").filter(Boolean).map((line) => line.split("\t")[0]);
const badge = (page) => page.evaluate(() => document.querySelector('.rail .nav[data-area="bookmarks"] .nav-count')?.textContent ?? null);
const listIds = (page) => page.evaluate(() => [...document.querySelectorAll(".rows a.row")].map((row) => row.dataset.id));
// The page's view of the active profile: its name, the profiles of the sidebar, the rows of what it keeps.
const shown = (page) => page.evaluate(() => ({
  name: document.querySelector(".pf-head h2")?.textContent ?? null,
  profiles: [...document.querySelectorAll(".pf-item")].map((item) => [item.querySelector("b").textContent, item.getAttribute("aria-checked")]),
  rows: Object.fromEntries([...document.querySelectorAll(".pf-row")].map((row) => [row.querySelector(".pf-row-label").textContent, row.querySelector(".pf-row-value").textContent])),
  note: document.querySelector(".pf-note p")?.textContent ?? null,
  rail: document.querySelector(".rail .profile-btn")?.getAttribute("title") ?? null,
}));
const toProfile = (step, page) => step("the rail leads to the profile page", () => page.click(".rail .profile-btn"), () => location.pathname === "/profile" && Boolean(document.querySelector(".pf-head h2")));
const toCatalog = (step, page) => step("the rail leads back to the catalog", () => page.click('.rail .nav[data-area="catalog"]'), () => location.pathname === "/catalog" && document.querySelectorAll(".rows a.row").length > 5);
const mark = async (page, id) => {
  await page.click(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`);
  await page.waitForFunction((id) => document.querySelector(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`)?.getAttribute("aria-pressed") === "true", id, { timeout: 8000 }).catch(() => problems.push(`the mark of ${id} was not made`));
};

// ---------- desktop ----------
{
  const requests = [];
  const context = await browser.newContext({ viewport: { width: 1500, height: 900 }, acceptDownloads: true });
  const { page, step } = await open(context, "/catalog", requests);
  check((await kept(page, LIST)) === null, "a fresh browser keeps a list of profiles");
  const catalog = (await listIds(page)).slice(0, 6);
  await mark(page, catalog[0]);
  await mark(page, catalog[1]);
  check((await badge(page)) === "2", `the rail does not count the marks: ${await badge(page)}`);

  // The first profile: unnamed, the marks of the browser.
  await toProfile(step, page);
  let now = await shown(page);
  check(now.name === "Profil 1" && JSON.stringify(now.profiles) === JSON.stringify([["Profil 1", "true"]]) && now.rows["Merkliste"] === "2 Module", `the first profile: ${JSON.stringify(now)}`);
  check(await page.evaluate(() => document.querySelector(".rail .profile-btn").getAttribute("aria-current") === "page" && !document.querySelector('.rail .nav[aria-current="page"]')), "the profile page is not the rail's current item, or a tab still is");
  check(await page.evaluate(() => document.querySelector(".topbar h1")?.textContent === "Profil"), "the top bar does not name the page");
  check(await page.evaluate(() => document.getElementById("pf-delete").getAttribute("aria-disabled") === "true"), "the only profile can be deleted");

  // A new profile: empty, active, its own key.
  await step("„Neues Profil“ asks for a name", () => page.click("#pf-new"), () => document.activeElement?.id === "pf-new-name" && document.activeElement.placeholder === "Profil 2");
  await page.fill("#pf-new-name", `  ${NAME} `);
  await step("a new profile is the active one", () => page.press("#pf-new-name", "Enter"), (name) => document.querySelector(".pf-head h2")?.textContent === name && !document.querySelector('.rail .nav[data-area="bookmarks"] .nav-count'), NAME);
  now = await shown(page);
  check(JSON.stringify(now.profiles) === JSON.stringify([["Profil 1", "false"], [NAME, "true"]]) && now.rows["Merkliste"] === "0 Module" && now.note?.startsWith(`„${NAME}“ angelegt`), `after „Neues Profil“: ${JSON.stringify(now)}`);
  check(now.rail === `Profil: ${NAME}` && (await page.evaluate(() => document.querySelector(".rail .profile-btn .avatar")?.textContent)) === "Z", `the rail's way to the page does not show the profile: ${now.rail}`);
  const list = await kept(page, LIST);
  check(/^active\t2$/m.test(list) && new RegExp(`^profile\\t2\\t\\d{10}\\t${NAME}$`, "m").test(list) && /^profile\t1\t0\t$/m.test(list), `the list of profiles as kept: ${JSON.stringify(list)}`);
  check(ids(await kept(page, MARKS)).length === 2 && (await kept(page, "betula.p2.bookmarks.v1")) === null, "the first profile's marks moved, or the new one has some");

  // A mark in the new profile goes under its key alone.
  await toCatalog(step, page);
  await mark(page, catalog[2]);
  check(JSON.stringify(ids(await kept(page, "betula.p2.bookmarks.v1"))) === JSON.stringify([catalog[2]]) && ids(await kept(page, MARKS)).length === 2, "a mark of the second profile is not under its own key alone");
  check((await badge(page)) === "1", "the rail counts another profile's marks");

  // Back to the first profile: its marks come back; a name.
  await toProfile(step, page);
  await step("a switch in the sidebar", () => page.click('.pf-item:has(b:text-is("Profil 1"))'), () => document.querySelector(".pf-head h2")?.textContent === "Profil 1" && document.querySelector('.rail .nav[data-area="bookmarks"] .nav-count')?.textContent === "2");
  check((await shown(page)).rows["Merkliste"] === "2 Module", "the first profile's marks are not back");
  await step("„Umbenennen“", () => page.click("#pf-rename"), () => document.activeElement?.id === "pf-name");
  await page.fill("#pf-name", "Informatik");
  await step("the new name", () => page.press("#pf-name", "Enter"), () => document.querySelector(".pf-head h2")?.textContent === "Informatik" && document.activeElement?.id === "pf-rename");
  check(/^profile\t1\t0\tInformatik$/m.test(await kept(page, LIST)), "the name was not kept");

  // Another tab of the same browser: its Merkliste follows a switch made here.
  const { page: other } = await open(context, "/bookmarks");
  await other.waitForFunction((n) => document.querySelectorAll(".rows a.row").length === n, 2, { timeout: 8000 }).catch(() => problems.push("the other tab: the first profile's list did not come up"));
  // The visitor clicks in the tab in front (a tab behind gets no frames, which the switch waits for, R21).
  await page.bringToFront();
  await step("a switch to the second profile", () => page.click(`.pf-item:has(b:text-is("${NAME}"))`), () => !document.querySelector('.rail .nav[data-area="bookmarks"] .nav-count')?.textContent?.startsWith("2"));
  await other.waitForFunction((id) => [...document.querySelectorAll(".rows a.row")].map((row) => row.dataset.id).join() === id, catalog[2], { timeout: 8000 }).catch(() => problems.push("the other tab's Merkliste did not follow the switch"));
  check((await other.evaluate(() => document.querySelector(".rail .profile-btn")?.getAttribute("title"))) === `Profil: ${NAME}`, "the other tab's rail did not follow the switch");
  await other.close();

  // „Profil leeren" and „Rückgängig".
  await step("„Profil leeren“ asks first", () => page.click("#pf-clear"), () => document.activeElement?.id === "pf-clear-yes");
  await step("the profile emptied", () => page.click("#pf-clear-yes"), () => document.querySelector('.pf-row[href="/bookmarks"] .pf-row-value')?.textContent === "0 Module" && document.activeElement?.id === "pf-undo");
  check((await kept(page, "betula.p2.bookmarks.v1")) === null && ids(await kept(page, MARKS)).length === 2, "emptying took more or less than the profile's marks");
  await step("„Rückgängig“ puts it back", () => page.click("#pf-undo"), () => document.querySelector('.pf-row[href="/bookmarks"] .pf-row-value')?.textContent === "1 Modul");

  // Light, dark, the system's; and the switch of the rail.
  const theme = () => page.evaluate(() => ({ data: document.documentElement.dataset.theme ?? null, stored: localStorage.getItem("betula.theme"), checked: document.querySelector('.pf-seg [aria-checked="true"][data-theme-value]')?.dataset.themeValue }));
  check(JSON.stringify(await theme()) === JSON.stringify({ data: null, stored: null, checked: "system" }), `the theme at first: ${JSON.stringify(await theme())}`);
  await page.click('[data-theme-value="dark"]');
  check(JSON.stringify(await theme()) === JSON.stringify({ data: "dark", stored: "dark", checked: "dark" }), `„Dunkel“: ${JSON.stringify(await theme())}`);
  check((await page.evaluate(() => document.querySelector("meta[name=theme-color]").content)) === "#0a0c11", "the browser's chrome did not turn dark");
  await page.click(".rail .theme-toggle");
  check(JSON.stringify(await theme()) === JSON.stringify({ data: "light", stored: "light", checked: "light" }), `the switch of the rail: ${JSON.stringify(await theme())}`);
  await page.click('[data-theme-value="system"]');
  check(JSON.stringify(await theme()) === JSON.stringify({ data: null, stored: null, checked: "system" }), `„System“: ${JSON.stringify(await theme())}`);

  // A profile as a file, and a profile made of it.
  const [download] = await Promise.all([page.waitForEvent("download"), page.click('button.action:has-text("Als Datei sichern")')]);
  const name = download.suggestedFilename();
  const file = JSON.parse(await readFile(await download.path(), "utf8"));
  check(/^betula-zweitstudium-\d{4}-\d{2}-\d{2}\.json$/.test(name), `the file's name: ${name}`);
  check(file.betula === "profile" && file.version === 1 && file.name === NAME && ids(file.stores?.[MARKS]).join() === catalog[2], `the file: ${JSON.stringify(file).slice(0, 300)}`);
  const folder = await mkdtemp(join(tmpdir(), "betula-profile-"));
  const backup = join(folder, name);
  await writeFile(backup, JSON.stringify(file));
  await page.setInputFiles('input[type="file"]', backup);
  await page.waitForFunction(() => document.querySelector(".pf-offer")?.textContent.includes("1 gemerktes Modul"), null, { timeout: 8000 }).catch(() => problems.push("a file picked is not offered"));
  await step("a profile made of the file", () => page.click("#pf-take"), (name) => document.querySelector(".pf-head h2")?.textContent === `${name} (2)` && document.querySelector('.pf-row[href="/bookmarks"] .pf-row-value')?.textContent === "1 Modul", NAME);
  check(JSON.stringify(ids(await kept(page, "betula.p3.bookmarks.v1"))) === JSON.stringify([catalog[2]]), "the file's marks are not the new profile's");
  const garbage = join(folder, "not-a-backup.json");
  await writeFile(garbage, JSON.stringify({ betula: "bookmarks", version: 1 }));
  await page.setInputFiles('input[type="file"]', garbage);
  await page.waitForFunction(() => document.querySelector(".pf-refused")?.textContent.includes("keine Sicherung"), null, { timeout: 8000 }).catch(() => problems.push("a file that is no backup is not refused"));

  // Deleting the active profile, and „Rückgängig".
  await step("„Löschen“ asks first", () => page.click("#pf-delete"), () => document.activeElement?.id === "pf-delete-yes");
  await step("the profile deleted", () => page.click("#pf-delete-yes"), (name) => document.querySelector(".pf-head h2")?.textContent === name && document.querySelectorAll(".pf-item").length === 2, NAME);
  check((await kept(page, "betula.p3.bookmarks.v1")) === null && !/^profile\t3\t/m.test(await kept(page, LIST)), "the deleted profile left something behind");
  await step("„Rückgängig“ brings it back, active", () => page.click("#pf-undo"), (name) => document.querySelector(".pf-head h2")?.textContent === `${name} (2)` && document.querySelectorAll(".pf-item").length === 3, NAME);
  check(JSON.stringify(ids(await kept(page, "betula.p3.bookmarks.v1"))) === JSON.stringify([catalog[2]]), "„Rückgängig“ did not bring the deleted profile's marks back");

  // „Mein Studiengang" of the active profile, read before the first paint (`html.mine`).
  await page.evaluate(() => localStorage.setItem("betula.p3.myprogram.v1", "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\n"));
  await page.goto(base + "/programs", { waitUntil: "domcontentloaded" });
  check(await page.evaluate(() => document.documentElement.classList.contains("mine")), "the script in <head> does not read the active profile's program");
  await page.evaluate((list) => localStorage.setItem("betula.profiles.v1", list.replace(/^active\t\d+$/m, "active\t2")), await kept(page, LIST));
  await page.goto(base + "/programs", { waitUntil: "domcontentloaded" });
  check(!(await page.evaluate(() => document.documentElement.classList.contains("mine"))), "the script in <head> reads another profile's program");

  // Privacy: no request names a profile or a mark, and none leaves the site.
  const leaked = requests.filter((r) => r.includes(NAME) || r.includes("Informatik") && !r.includes("/programs") || /[?&](profile|marks?)=/.test(r));
  check(leaked.length === 0, `requests that carry a profile: ${leaked.slice(0, 3)}`);
  // (The file „Als Datei sichern" hands over is the page's own, a `blob:` address.)
  const beyond = requests.filter((r) => !r.startsWith(base) && !r.startsWith("blob:"));
  check(beyond.length === 0, `requests to somewhere else: ${beyond.slice(0, 3)}`);
  await context.close();
}

// ---------- English ----------
{
  const context = await browser.newContext({ viewport: { width: 1500, height: 900 } });
  const { page } = await open(context, "/en/profile");
  await page.waitForFunction(() => document.querySelector(".pf-head h2")?.textContent === "Profile 1", null, { timeout: 8000 }).catch(() => problems.push("English: the first profile is not named"));
  const english = await page.evaluate(() => ({ side: document.querySelector("#sidebar .panel-head h2")?.textContent, top: document.querySelector(".topbar h1")?.textContent, rows: [...document.querySelectorAll(".pf-row-label")].map((label) => label.textContent), language: [...document.querySelectorAll('.pf-seg a[data-language]')].map((a) => a.getAttribute("href")) }));
  check(english.side === "Profiles" && english.top === "Profile" && english.rows.join() === "Saved modules,Timetable,My studies,Saved plans" && english.language.join() === "/profile", `English: ${JSON.stringify(english)}`);
  await context.close();
}

// ---------- phone ----------
{
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
  const { page, step } = await open(context, "/");
  const way = await page.evaluate(() => { const button = document.querySelector(".brand-phone .profile-btn"); const r = button?.getBoundingClientRect(); return r && { width: r.width, height: r.height, before: button.nextElementSibling?.classList.contains("languages") }; });
  check(way && way.width >= 44 && way.height >= 44 && way.before, `phone: the way to the profile page on the brand line: ${JSON.stringify(way)}`);
  await step("phone: the brand line leads to the profile page", () => page.tap(".brand-phone .profile-btn"), () => location.pathname === "/profile" && Boolean(document.querySelector(".pf-head h2")));
  const layout = await page.evaluate(() => { const [side, head] = [document.getElementById("sidebar"), document.querySelector(".pf-head")].map((el) => el.getBoundingClientRect()); return { first: side.bottom <= head.top + 1, overflow: document.documentElement.scrollWidth > innerWidth, theme: Boolean(document.querySelector('[data-theme-value="dark"]')?.getClientRects().length) }; });
  check(layout.first && !layout.overflow && layout.theme, `phone: the profiles before the page, no sideways scrolling, the theme to choose: ${JSON.stringify(layout)}`);
  await context.close();
}

// ---------- without the app ----------
{
  const context = await browser.newContext({ viewport: { width: 1500, height: 900 }, javaScriptEnabled: false });
  const page = await context.newPage();
  const response = await page.goto(base + "/profile", { waitUntil: "domcontentloaded" });
  const plain = await page.evaluate(() => ({ explained: document.querySelector(".state")?.textContent.includes("JavaScript") ?? false, robots: document.querySelector('meta[name="robots"]')?.content ?? "", way: document.querySelector(".rail .profile-btn")?.getClientRects().length ?? 0 }));
  check(response.status() === 200 && plain.explained && plain.robots.startsWith("noindex") && plain.way === 0, `without JavaScript: ${JSON.stringify(plain)}`);
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
