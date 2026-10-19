// Checks the profiles (folia/crates/stores/src/profiles.rs) and their page (folia/crates/profile).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node profile.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Desktop: the rail's way to the page, and no switch of the languages in the rail; a new profile is
// empty and the active one, its marks under its own key while the first profile's stay under the
// plain one; a switch in the sidebar brings a profile's marks back (the rail counts them),
// renaming, „Profil leeren" and deleting with „Rückgängig"; another tab of the same browser follows
// a switch, and the Merkliste open there shows the other profile's list; the menu of the profiles
// at the rail's way (a mouse resting on it: the profiles, „Neues Profil“, the way to the page and
// to the other language, a profile chosen; a right click: the focus on the active profile, a choice
// with the keyboard, Esc; „Neues Profil“'s field holds the menu, Esc leaves the field first); light, dark or the system's (with the switch of the rail); a profile kept as a
// file and made again of it, and a file that is none; „Mein Studiengang" of the active profile read
// by the script in <head>. Privacy: no request carries a profile's name or a mark, and none leaves
// the site. Phone: the way to the page a box at the right of the search, as high as it, and none on
// the start page's brand line; a finger held on it opens the menu (the release goes nowhere), a
// profile tapped there is the active one; a tap on the box opens the menu too, without a step, and
// „Neues Profil“ there makes the active profile; the menu's way leads to the page, on top of the
// list it was opened from, the profiles before the page, no sideways scrolling, and the arrow at
// the head back to the list as it was left, scrolled as far. English. Without the app: the page explains itself, is
// `noindex`, offers the other language, and the rail's way to it shows.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { fileURLToPath } from "node:url";
import { readFile, writeFile, mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
// What the check looks for is on the page in front (on-page.js): every tab's page is built behind
// it at the start (folia/crates/shell/src/stage.rs, `prebuild`), with parts of the same name.
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const newContext = browser.newContext.bind(browser);
browser.newContext = async (options) => { const context = await newContext(options); await context.addInitScript(ON_PAGE); return context; };
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
    await page.waitForFunction(() => !onPage(".pending-page.held"), null, { timeout: 2000 });
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
const badge = (page) => page.evaluate(() => onPage('.rail .nav[data-area="bookmarks"] .nav-count')?.textContent ?? null);
const listIds = (page) => page.evaluate(() => [...allOnPage(".rows a.row")].map((row) => row.dataset.id));
// The page's view of the active profile: its name, the profiles of the sidebar, the rows of what it keeps.
const shown = (page) => page.evaluate(() => ({
  name: onPage(".pf-head h2")?.textContent ?? null,
  profiles: [...allOnPage(".pf-item")].map((item) => [item.querySelector("b").textContent, item.getAttribute("aria-checked")]),
  rows: Object.fromEntries([...allOnPage(".pf-row")].map((row) => [row.querySelector(".pf-row-label").textContent, row.querySelector(".pf-row-value").textContent])),
  note: onPage(".pf-note p")?.textContent ?? null,
  rail: onPage(".rail .profile-btn")?.getAttribute("aria-label") ?? null,
}));
const toProfile = (step, page) => step("the rail leads to the profile page", () => page.click(".rail .profile-btn >> visible=true"), () => location.pathname === "/profile" && Boolean(onPage(".pf-head h2")));
const toCatalog = (step, page) => step("the rail leads back to the catalog", () => page.click('.rail .nav[data-area="catalog"] >> visible=true'), () => location.pathname === "/catalog" && allOnPage(".rows a.row").length > 5);
const mark = async (page, id) => {
  await page.click(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`);
  await page.waitForFunction((id) => onPage(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`)?.getAttribute("aria-pressed") === "true", id, { timeout: 8000 }).catch(() => problems.push(`the mark of ${id} was not made`));
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
  check(await page.evaluate(() => onPage(".rail .profile-btn").getAttribute("aria-current") === "page" && !onPage('.rail .nav[aria-current="page"]')), "the profile page is not the rail's current item, or a tab still is");
  check(await page.evaluate(() => onPage(".topbar h1")?.textContent === "Profil"), "the top bar does not name the page");
  check(await page.evaluate(() => onPage("#pf-delete").getAttribute("aria-disabled") === "true"), "the only profile can be deleted");
  check((await page.evaluate(() => allOnPage(".rail [data-language]").length)) === 0, "the rail still switches the language: it lives in the profile");

  // A new profile: empty, active, its own key.
  await step("„Neues Profil“ asks for a name", () => page.click("#pf-new >> visible=true"), () => document.activeElement?.id === "pf-new-name" && document.activeElement.placeholder === "Profil 2");
  await page.fill("#pf-new-name >> visible=true", `  ${NAME} `);
  await step("a new profile is the active one", () => page.press("#pf-new-name >> visible=true", "Enter"), (name) => onPage(".pf-head h2")?.textContent === name && !onPage('.rail .nav[data-area="bookmarks"] .nav-count'), NAME);
  now = await shown(page);
  check(JSON.stringify(now.profiles) === JSON.stringify([["Profil 1", "false"], [NAME, "true"]]) && now.rows["Merkliste"] === "0 Module" && now.note?.startsWith(`„${NAME}“ angelegt`), `after „Neues Profil“: ${JSON.stringify(now)}`);
  check(now.rail === `Profil: ${NAME}` && (await page.evaluate(() => onPage(".rail .profile-btn .avatar")?.textContent)) === "Z", `the rail's way to the page does not show the profile: ${now.rail}`);
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
  await step("a switch in the sidebar", () => page.click('.pf-item:has(b:text-is("Profil 1")) >> visible=true'), () => onPage(".pf-head h2")?.textContent === "Profil 1" && onPage('.rail .nav[data-area="bookmarks"] .nav-count')?.textContent === "2");
  check((await shown(page)).rows["Merkliste"] === "2 Module", "the first profile's marks are not back");
  await step("„Umbenennen“", () => page.click("#pf-rename >> visible=true"), () => document.activeElement?.id === "pf-name");
  await page.fill("#pf-name >> visible=true", "Informatik");
  await step("the new name", () => page.press("#pf-name >> visible=true", "Enter"), () => onPage(".pf-head h2")?.textContent === "Informatik" && document.activeElement?.id === "pf-rename");
  check(/^profile\t1\t0\tInformatik$/m.test(await kept(page, LIST)), "the name was not kept");

  // Another tab of the same browser: its Merkliste follows a switch made here.
  const { page: other } = await open(context, "/bookmarks");
  await other.waitForFunction((n) => allOnPage(".rows a.row").length === n, 2, { timeout: 8000 }).catch(() => problems.push("the other tab: the first profile's list did not come up"));
  // The visitor clicks in the tab in front (a tab behind gets no frames, which the switch waits for, R21).
  await page.bringToFront();
  await step("a switch to the second profile", () => page.click(`.pf-item:has(b:text-is("${NAME}"))`), () => !onPage('.rail .nav[data-area="bookmarks"] .nav-count')?.textContent?.startsWith("2"));
  await other.waitForFunction((id) => [...allOnPage(".rows a.row")].map((row) => row.dataset.id).join() === id, catalog[2], { timeout: 8000 }).catch(() => problems.push("the other tab's Merkliste did not follow the switch"));
  check((await other.evaluate(() => onPage(".rail .profile-btn")?.getAttribute("aria-label"))) === `Profil: ${NAME}`, "the other tab's rail did not follow the switch");
  await other.close();

  // „Profil leeren" and „Rückgängig".
  await step("„Profil leeren“ asks first", () => page.click("#pf-clear >> visible=true"), () => document.activeElement?.id === "pf-clear-yes");
  await step("the profile emptied", () => page.click("#pf-clear-yes >> visible=true"), () => onPage('.pf-row[href="/bookmarks"] .pf-row-value')?.textContent === "0 Module" && document.activeElement?.id === "pf-undo");
  check((await kept(page, "betula.p2.bookmarks.v1")) === null && ids(await kept(page, MARKS)).length === 2, "emptying took more or less than the profile's marks");
  await step("„Rückgängig“ puts it back", () => page.click("#pf-undo >> visible=true"), () => onPage('.pf-row[href="/bookmarks"] .pf-row-value')?.textContent === "1 Modul");

  // The menu of the profiles (owner, 2026-10-14: „wenn man beim pc über das profil in der nav bar
  // links hovert, kann man die profile direkt auswählen"): a mouse resting on the rail's way.
  const menu = () => page.evaluate(() => {
    const at = onPage(".profile-menu");
    return at && { items: [...at.querySelectorAll(".pm-item")].map((item) => `${item.querySelector("b").textContent} ${item.getAttribute("aria-checked")}`), add: at.querySelector(".pm-new-way")?.textContent ?? null, ways: [...at.querySelectorAll("a.tab-menu-way")].map((way) => way.getAttribute("href")), scrim: Boolean(onPage(".tab-scrim")), focus: document.activeElement?.closest(".pm-item")?.querySelector("b")?.textContent ?? null };
  });
  await page.mouse.move(700, 450);
  await page.hover(".rail .profile-btn >> visible=true");
  await page.waitForSelector(".profile-menu >> visible=true", { timeout: 3000 }).catch(() => problems.push("the menu of the profiles does not open where a mouse rests on the rail's way"));
  let offered = await menu();
  check(JSON.stringify(offered) === JSON.stringify({ items: ["Informatik false", `${NAME} true`], add: "Neues Profil", ways: ["/profile", "/en/profile"], scrim: false, focus: null }), `the menu of the profiles: ${JSON.stringify(offered)}`);
  await step("a profile chosen in the menu", () => page.click('.profile-menu .pm-item:has(b:text-is("Informatik")) >> visible=true'), () => !onPage(".profile-menu") && onPage(".pf-head h2")?.textContent === "Informatik" && onPage('.rail .nav[data-area="bookmarks"] .nav-count')?.textContent === "2");
  // A right click: the menu stays where the mouse goes, the active profile has the focus, and the
  // keyboard chooses another; Esc closes it, the focus back on the way.
  await page.click(".rail .profile-btn >> visible=true", { button: "right" });
  await page.mouse.move(700, 450);
  await page.waitForTimeout(500);
  offered = await menu();
  check(offered?.scrim && offered.focus === "Informatik", `a right click on the rail's way: ${JSON.stringify(offered)}`);
  await page.keyboard.press("Tab");
  await step("a profile chosen with the keyboard", () => page.keyboard.press("Enter"), (name) => !onPage(".profile-menu") && onPage(".pf-head h2")?.textContent === name && document.activeElement?.matches(".rail .profile-btn"), NAME);
  await page.click(".rail .profile-btn >> visible=true", { button: "right" });
  await page.waitForSelector(".profile-menu >> visible=true", { timeout: 3000 }).catch(() => problems.push("a right click on the rail's way opens no menu"));
  await page.keyboard.press("Escape");
  check(await page.evaluate(() => !onPage(".profile-menu") && document.activeElement?.matches(".rail .profile-btn")), "Esc does not close the menu of the profiles, or the focus is lost");
  // „Neues Profil“ in the menu (owner, 2026-10-19): its field holds a menu a mouse opened, which no
  // longer goes with the mouse; Esc leaves the field first, then the menu.
  await page.mouse.move(700, 450);
  await page.hover(".rail .profile-btn >> visible=true");
  await page.waitForSelector(".profile-menu .pm-new-way >> visible=true", { timeout: 3000 }).catch(() => problems.push("the menu of the profiles offers no „Neues Profil“"));
  await page.click(".profile-menu .pm-new-way >> visible=true");
  await page.mouse.move(700, 450);
  await page.waitForTimeout(500);
  const naming = await page.evaluate(() => ({ menu: Boolean(onPage(".profile-menu")), scrim: Boolean(onPage(".tab-scrim")), focus: document.activeElement?.id ?? null, placeholder: onPage("#pm-new-name")?.placeholder ?? null }));
  check(JSON.stringify(naming) === JSON.stringify({ menu: true, scrim: true, focus: "pm-new-name", placeholder: "Profil 3" }), `„Neues Profil“ in the menu: ${JSON.stringify(naming)}`);
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
  check(await page.evaluate(() => Boolean(onPage(".profile-menu")) && !onPage("#pm-new-name") && document.activeElement?.id === "pm-new"), "Esc in the menu's field does not leave the field alone, or closes the menu");
  await page.keyboard.press("Escape");
  check(await page.evaluate(() => !onPage(".profile-menu")), "the second Esc does not close the menu of the profiles");

  // Light, dark, the system's; and the switch of the rail.
  const theme = () => page.evaluate(() => ({ data: document.documentElement.dataset.theme ?? null, stored: localStorage.getItem("betula.theme"), checked: onPage('.pf-seg [aria-checked="true"][data-theme-value]')?.dataset.themeValue }));
  check(JSON.stringify(await theme()) === JSON.stringify({ data: null, stored: null, checked: "system" }), `the theme at first: ${JSON.stringify(await theme())}`);
  await page.click('[data-theme-value="dark"] >> visible=true');
  check(JSON.stringify(await theme()) === JSON.stringify({ data: "dark", stored: "dark", checked: "dark" }), `„Dunkel“: ${JSON.stringify(await theme())}`);
  check((await page.evaluate(() => onPage("meta[name=theme-color]").content)) === "#0a0c11", "the browser's chrome did not turn dark");
  const manifest = () => page.evaluate(() => onPage("link[rel=manifest]").getAttribute("href"));
  check((await manifest()) === "/manifest-dark.webmanifest", `the page names ${await manifest()} in the dark`);
  await page.click(".rail .theme-toggle >> visible=true");
  check(JSON.stringify(await theme()) === JSON.stringify({ data: "light", stored: "light", checked: "light" }), `the switch of the rail: ${JSON.stringify(await theme())}`);
  check((await manifest()) === "/manifest.webmanifest", `the page names ${await manifest()} in the light`);
  await page.click('[data-theme-value="system"] >> visible=true');
  check(JSON.stringify(await theme()) === JSON.stringify({ data: null, stored: null, checked: "system" }), `„System“: ${JSON.stringify(await theme())}`);

  // A profile as a file, and a profile made of it.
  const [download] = await Promise.all([page.waitForEvent("download"), page.click('button.action:has-text("Als Datei sichern") >> visible=true')]);
  const name = download.suggestedFilename();
  const file = JSON.parse(await readFile(await download.path(), "utf8"));
  check(/^betula-zweitstudium-\d{4}-\d{2}-\d{2}\.json$/.test(name), `the file's name: ${name}`);
  check(file.betula === "profile" && file.version === 1 && file.name === NAME && ids(file.stores?.[MARKS]).join() === catalog[2], `the file: ${JSON.stringify(file).slice(0, 300)}`);
  const folder = await mkdtemp(join(tmpdir(), "betula-profile-"));
  const backup = join(folder, name);
  await writeFile(backup, JSON.stringify(file));
  await page.setInputFiles('input[type="file"] >> visible=true', backup);
  await page.waitForFunction(() => onPage(".pf-offer")?.textContent.includes("1 gemerktes Modul"), null, { timeout: 8000 }).catch(() => problems.push("a file picked is not offered"));
  await step("a profile made of the file", () => page.click("#pf-take >> visible=true"), (name) => onPage(".pf-head h2")?.textContent === `${name} (2)` && onPage('.pf-row[href="/bookmarks"] .pf-row-value')?.textContent === "1 Modul", NAME);
  check(JSON.stringify(ids(await kept(page, "betula.p3.bookmarks.v1"))) === JSON.stringify([catalog[2]]), "the file's marks are not the new profile's");
  const garbage = join(folder, "not-a-backup.json");
  await writeFile(garbage, JSON.stringify({ betula: "bookmarks", version: 1 }));
  await page.setInputFiles('input[type="file"] >> visible=true', garbage);
  await page.waitForFunction(() => onPage(".pf-refused")?.textContent.includes("keine Sicherung"), null, { timeout: 8000 }).catch(() => problems.push("a file that is no backup is not refused"));

  // Deleting the active profile, and „Rückgängig".
  await step("„Löschen“ asks first", () => page.click("#pf-delete >> visible=true"), () => document.activeElement?.id === "pf-delete-yes");
  await step("the profile deleted", () => page.click("#pf-delete-yes >> visible=true"), (name) => onPage(".pf-head h2")?.textContent === name && allOnPage(".pf-item").length === 2, NAME);
  check((await kept(page, "betula.p3.bookmarks.v1")) === null && !/^profile\t3\t/m.test(await kept(page, LIST)), "the deleted profile left something behind");
  await step("„Rückgängig“ brings it back, active", () => page.click("#pf-undo >> visible=true"), (name) => onPage(".pf-head h2")?.textContent === `${name} (2)` && allOnPage(".pf-item").length === 3, NAME);
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
  await page.waitForFunction(() => onPage(".pf-head h2")?.textContent === "Profile 1", null, { timeout: 8000 }).catch(() => problems.push("English: the first profile is not named"));
  const english = await page.evaluate(() => ({ side: onPage("#sidebar .panel-head h2")?.textContent, top: onPage(".topbar h1")?.textContent, rows: [...allOnPage(".pf-row-label")].map((label) => label.textContent), language: [...allOnPage('.pf-seg a[data-language]')].map((a) => a.getAttribute("href")) }));
  check(english.side === "Profiles" && english.top === "Profile" && english.rows.join() === "Saved modules,Timetable,My studies,Saved plans" && english.language.join() === "/profile", `English: ${JSON.stringify(english)}`);
  await context.close();
}

// ---------- phone ----------
{
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
  const { page, step } = await open(context, "/");
  await page.evaluate((name) => localStorage.setItem("betula.profiles.v1", `active\t1\nprofile\t1\t0\t\nprofile\t2\t1760443200\t${name}\n`), NAME);
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  // The way to the profile page at the head of every page (owner, 2026-10-14: „für die mobile
  // Version noch ein Konzept wie man an sein Profil rankommt"): a box at the right of the search,
  // as high as it; the start page's brand line holds nothing else any more.
  const way = await page.evaluate(() => {
    const box = onPage(".topbar .top-profile")?.getBoundingClientRect();
    const search = onPage(".topbar .search").getBoundingClientRect();
    return box && { size: `${Math.round(box.width)}x${Math.round(box.height)}`, right: Math.round(innerWidth - box.right), gap: Math.round(box.left - search.right), search: Math.round(search.height), brand: allOnPage(".brand-phone a, .brand-phone button").length };
  });
  check(JSON.stringify(way) === JSON.stringify({ size: "48x48", right: 12, gap: 8, search: 48, brand: 0 }), `phone: the way to the profile page at the head: ${JSON.stringify(way)}`);
  // A finger held on it: the menu of the profiles (owner: „Beim Handy long hold"), and its release
  // goes nowhere.
  const cdp = await context.newCDPSession(page);
  const at = await page.evaluate(() => { const box = onPage(".topbar .top-profile").getBoundingClientRect(); return { x: box.left + box.width / 2, y: box.top + box.height / 2 }; });
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [at] });
  await page.waitForTimeout(650);
  const held = await page.evaluate(() => ({ items: [...allOnPage(".profile-menu .pm-item")].map((item) => `${item.querySelector("b").textContent} ${item.getAttribute("aria-checked")}`), ways: [...allOnPage(".profile-menu a.tab-menu-way")].map((way) => way.getAttribute("href")) }));
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await page.waitForTimeout(500);
  check(JSON.stringify(held) === JSON.stringify({ items: ["Profil 1 true", `${NAME} false`], ways: ["/profile", "/en"] }), `phone: a finger held on the way: ${JSON.stringify(held)}`);
  check(await page.evaluate(() => location.pathname === "/" && Boolean(onPage(".profile-menu"))), "phone: the release of the finger went somewhere, or the menu went");
  await step("phone: a profile tapped in the menu", () => page.tap(`.profile-menu .pm-item:has(b:text-is("${NAME}"))`), (name) => !onPage(".profile-menu") && onPage(".topbar .top-profile")?.getAttribute("aria-label") === `Profil: ${name}` && onPage(".topbar .top-profile .avatar")?.textContent === "Z", NAME);
  // A tap on the box is the menu as well (owner, 2026-10-19: „die häufigste Aktion wird sein, dass
  // man das nur das Profil wechseln will"), and no step: the page stays where it is.
  await step("phone: „Module“", () => page.tap('.bottomnav a.nav[data-area="catalog"] >> visible=true'), () => location.pathname === "/catalog" && allOnPage(".rows a.row").length > 5);
  await page.evaluate(() => window.scrollTo(0, 900));
  await page.waitForTimeout(300);
  const left = await page.evaluate(() => ({ here: location.pathname + location.search, y: Math.round(scrollY), at: navigation.currentEntry.index }));
  await step("phone: a tap on the box opens the menu", () => page.tap(".topbar .top-profile >> visible=true"), () => Boolean(onPage(".profile-menu .pm-new-way")) && onPage(".topbar .top-profile")?.getAttribute("aria-expanded") === "true");
  const tapped = await page.evaluate(() => ({ here: location.pathname + location.search, at: navigation.currentEntry.index, scrim: Boolean(onPage(".tab-scrim")), ways: [...allOnPage(".profile-menu a.tab-menu-way")].map((way) => way.getAttribute("href")) }));
  check(tapped.here === left.here && tapped.at === left.at && tapped.scrim && JSON.stringify(tapped.ways) === JSON.stringify(["/profile", "/en/catalog"]), `phone: the tap on the box: ${JSON.stringify(tapped)}`);
  // „Neues Profil“ in it: a name, and the new profile, empty, is the active one; the page stays.
  await step("phone: „Neues Profil“ asks for a name", () => page.tap(".profile-menu .pm-new-way >> visible=true"), () => document.activeElement?.id === "pm-new-name" && document.activeElement.placeholder === "Profil 3");
  await page.fill("#pm-new-name >> visible=true", "Mobil");
  await step("phone: the new profile is the active one", () => page.press("#pm-new-name >> visible=true", "Enter"), () => !onPage(".profile-menu") && onPage(".topbar .top-profile")?.getAttribute("aria-label") === "Profil: Mobil" && onPage(".topbar .top-profile .avatar")?.textContent === "M");
  check(/^active\t3$/m.test(await kept(page, LIST)) && /^profile\t3\t\d{10}\tMobil$/m.test(await kept(page, LIST)), `phone: the profile made in the menu: ${JSON.stringify(await kept(page, LIST))}`);
  check((await page.evaluate(() => location.pathname + location.search)) === left.here, "phone: a profile made in the menu led somewhere");
  // The page is a page of a while (owner, 2026-10-19: „dass die Profile page, als temporäre seite
  // getreated werden soll wo man dann oben den zurück pfeil hat und genau wieder da landet, wo man
  // davor war"): the menu's way leads there, the arrow at the head back to the list as it was left.
  await page.evaluate((y) => window.scrollTo(0, y), left.y);
  await step("phone: the box, then the menu's way to the page", async () => { await page.tap(".topbar .top-profile >> visible=true"); await page.waitForSelector('.profile-menu a[href="/profile"] >> visible=true', { timeout: 3000 }); await page.tap('.profile-menu a[href="/profile"] >> visible=true'); }, () => location.pathname === "/profile" && onPage(".pf-head h2")?.textContent === "Mobil" && Boolean(onPage(".top-back.on")));
  const layout = await page.evaluate(() => { const [side, head] = [onPage("#sidebar"), onPage(".pf-head")].map((el) => el.getBoundingClientRect()); return { first: side.bottom <= head.top + 1, overflow: document.documentElement.scrollWidth > innerWidth, theme: Boolean(onPage('[data-theme-value="dark"]')?.getClientRects().length), away: onPage(".topbar .top-profile")?.inert ?? null, at: navigation.currentEntry.index }; });
  check(layout.first && !layout.overflow && layout.theme && layout.away === true && layout.at === left.at + 1, `phone: the profile page (the profiles first, no sideways scrolling, the theme to choose, the arrow in the box's place, one step on top): ${JSON.stringify(layout)}`);
  await step("phone: the arrow leads back to where the visitor was", () => page.tap(".top-back.on >> visible=true"), (here) => location.pathname + location.search === here && allOnPage(".rows a.row").length > 5, left.here);
  await page.waitForTimeout(300);
  const back = await page.evaluate(() => ({ y: Math.round(scrollY), at: navigation.currentEntry.index, forward: navigation.canGoForward }));
  check(Math.abs(back.y - left.y) < 4 && back.at === left.at && back.forward, `phone: the arrow did not go back to the list as it was left (scrolled ${left.y}, at ${left.at}): ${JSON.stringify(back)}`);
  await context.close();
}

// ---------- without the app ----------
{
  const context = await browser.newContext({ viewport: { width: 1500, height: 900 }, javaScriptEnabled: false });
  const page = await context.newPage();
  const response = await page.goto(base + "/profile", { waitUntil: "domcontentloaded" });
  const plain = await page.evaluate(() => ({ explained: onPage(".state")?.textContent.includes("JavaScript") ?? false, robots: onPage('meta[name="robots"]')?.content ?? "", way: onPage(".rail .profile-btn")?.getClientRects().length ?? 0, english: onPage('.state a[data-language="en"]')?.getAttribute("href") ?? null }));
  check(response.status() === 200 && plain.explained && plain.robots.startsWith("noindex") && plain.way === 1 && plain.english === "/en/profile", `without JavaScript: ${JSON.stringify(plain)}`);
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
