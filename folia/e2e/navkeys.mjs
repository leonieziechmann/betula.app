// Checks the navigation's keys (enhance.js, „the navigation's keys"; owner, 2026-10-16: „wenn man
// ctrl/alt drückt sollen dann die shortcuts bei den nav items angezeigt werden. Wichtig ist, dass …
// so übliche shortcuts, wie ctrl+c/s/p usw. weiter funktionieren"): with Ctrl and Alt (Alt alone
// on Windows, ⌃⌥ on a Mac; the system is made up through `navigator.platform`) and a digit the app
// goes to the tab of the navigation, 1 the start page to 5 the Stundenplan, without a page load and
// from the search as well; held a moment, the keys show the digits at the rail's tabs, and let go
// they go; a key or the mouse with them, and nothing shows; Ctrl alone, Ctrl+C, S, P, Ctrl or Alt
// and a digit where they are the browser's, and AltGr (Ctrl and Alt on Windows) do nothing and
// keep their default; nothing while a modal dialog is open; where the app does not start, the keys
// load the tab's page.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node navkeys.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a console error or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
// Longer than the keys are held before the digits show (enhance.js, NAV_KEYS_AFTER).
const HELD_MS = 450;

// A page of the app on a wide screen, on the system `platform` names (undefined: the browser's).
const open = async (platform, path, options = {}) => {
  const context = await browser.newContext({ viewport: { width: 1400, height: 900 }, serviceWorkers: "block" });
  if (platform) {
    await context.addInitScript((platform) => {
      Object.defineProperty(Navigator.prototype, "platform", { get: () => platform });
      Object.defineProperty(Navigator.prototype, "userAgentData", { get: () => undefined });
    }, platform);
  }
  if (options.noApp) await context.route("**/pkg/folia_client.js*", (route) => route.fulfill({ contentType: "text/javascript", body: "export default () => Promise.reject(new Error('no app'));" }));
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error" && !(options.noApp && /no app/.test(m.text()))) problems.push(`${platform || "browser"}: console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => { if (!(options.noApp && /no app/.test(String(e)))) problems.push(`${platform || "browser"}: pageerror: ${String(e).slice(0, 300)}`); });
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  if (options.noApp) {
    await page.waitForFunction(() => window.__betulaStarting === false, null, { timeout: 30000 }).catch(() => problems.push("classic: the start of the app did not end"));
  } else {
    await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push(`${platform || "browser"}: the browser app never took over`));
  }
  // What became of each key after every listener of the page (registered last, so it hears last).
  await page.evaluate(() => {
    window.__marker = 1;
    window.__keys = [];
    for (const type of ["keydown", "keyup"]) addEventListener(type, (e) => window.__keys.push({ type, key: e.key, code: e.code, prevented: e.defaultPrevented }));
  });
  return { context, page };
};

const state = (page) => page.evaluate(() => ({
  path: location.pathname,
  loaded: window.__marker !== 1,
  shown: document.documentElement.hasAttribute("data-nav-keys"),
  digits: [...document.querySelectorAll(".rail .nav")].map((tab) => {
    const key = tab.querySelector(".nav-key");
    return key && getComputedStyle(key).display !== "none" ? key.textContent : "";
  }).join(""),
  bar: [...document.querySelectorAll(".bottomnav .nav-key")].some((key) => key.getClientRects().length > 0),
}));
const prevented = async (page, code, type = "keydown") => (await page.evaluate(() => window.__keys)).filter((k) => k.type === type && k.code === code).map((k) => k.prevented);
const clearKeys = (page) => page.evaluate(() => { window.__keys = []; });
// Holds `modifiers`, does `inside` while they are held, lets go of them.
const holding = async (page, modifiers, inside) => {
  for (const m of modifiers) await page.keyboard.down(m);
  try { await inside(); } finally { for (const m of [...modifiers].reverse()) await page.keyboard.up(m); }
};
const goes = async (page, label, modifiers, digit, path) => {
  await holding(page, modifiers, () => page.keyboard.press(`Digit${digit}`));
  const ok = await page.waitForFunction((path) => location.pathname === path, path, { timeout: 5000 }).then(() => true, () => false);
  const now = await state(page);
  check(ok, `${label}: ${modifiers.join("+")}+${digit} went to ${now.path}, not ${path}`);
  check(!now.loaded, `${label}: ${modifiers.join("+")}+${digit} loaded the page again`);
};
const stays = async (page, label, modifiers, digit) => {
  const before = (await state(page)).path;
  await clearKeys(page);
  await holding(page, modifiers, () => page.keyboard.press(`Digit${digit}`));
  await page.waitForTimeout(400);
  const now = await state(page);
  check(now.path === before, `${label}: ${modifiers.join("+")}+${digit} went to ${now.path}`);
  check((await prevented(page, `Digit${digit}`)).every((p) => !p), `${label}: ${modifiers.join("+")}+${digit} lost its default`);
};

// ---- Linux (the browser's own system here): Ctrl and Alt
{
  const label = "linux";
  const { context, page } = await open("Linux x86_64", "/catalog");
  check((await state(page)).digits === "", `${label}: digits before any key: ${(await state(page)).digits}`);
  // Ctrl alone, or Alt alone, held: nothing shows.
  for (const alone of ["Control", "Alt"]) {
    await holding(page, [alone], () => page.waitForTimeout(HELD_MS));
    check(!(await state(page)).shown, `${label}: ${alone} alone shows the digits`);
  }
  // Ctrl and Alt held: the digits at the rail's five tabs, none in the hidden bar; let go, gone.
  await holding(page, ["Control", "Alt"], async () => {
    await page.waitForTimeout(HELD_MS);
    const held = await state(page);
    check(held.shown && held.digits === "12345", `${label}: Ctrl+Alt held shows ${held.shown ? held.digits : "nothing"}`);
    check(!held.bar, `${label}: the hidden bar shows digits`);
  });
  await clearKeys(page);
  check(!(await state(page)).shown, `${label}: the digits stay after the keys are let go`);
  // Quick, the keys show nothing.
  await holding(page, ["Control", "Alt"], () => page.waitForTimeout(80));
  await page.waitForTimeout(HELD_MS);
  check(!(await state(page)).shown, `${label}: a quick Ctrl+Alt shows the digits`);
  // Another key with them: nothing while they are held.
  await holding(page, ["Control", "Alt"], async () => {
    await page.keyboard.press("KeyX");
    await page.waitForTimeout(HELD_MS);
    check(!(await state(page)).shown, `${label}: Ctrl+Alt+X shows the digits`);
  });
  // The mouse with them: the digits go.
  await holding(page, ["Control", "Alt"], async () => {
    await page.waitForTimeout(HELD_MS);
    await page.mouse.move(700, 450);
    await page.mouse.down();
    await page.mouse.up();
    await page.waitForTimeout(50);
    check(!(await state(page)).shown, `${label}: the digits stay at a click`);
  });

  // Every tab, with the keys held through several digits; the digits stay meanwhile.
  await holding(page, ["Control", "Alt"], async () => {
    await page.waitForTimeout(HELD_MS);
    for (const [digit, path] of [["1", "/"], ["3", "/study"], ["4", "/bookmarks"], ["5", "/studyplan"]]) {
      await page.keyboard.press(`Digit${digit}`);
      const ok = await page.waitForFunction((path) => location.pathname === path, path, { timeout: 5000 }).then(() => true, () => false);
      check(ok, `${label}: Ctrl+Alt+${digit} (held) went to ${(await state(page)).path}, not ${path}`);
    }
    const held = await state(page);
    check(held.shown && held.digits === "12345", `${label}: the digits went on the way (${held.digits})`);
    check(!held.loaded, `${label}: a tab loaded the page again`);
  });
  await goes(page, label, ["Control", "Alt"], "2", "/catalog");
  // From the search, with a text typed: the keys type nothing into it.
  await page.click("#topsearch");
  await page.keyboard.type("abc");
  // The search follows what was typed first (140 ms later), then the keys go on from there.
  await page.waitForFunction(() => new URLSearchParams(location.search).get("q") === "abc", null, { timeout: 5000 }).catch(() => problems.push("linux: the search did not follow"));
  await goes(page, label, ["Control", "Alt"], "4", "/bookmarks");
  check(await page.evaluate(() => !document.querySelector("#topsearch")?.value.includes("4")), `${label}: Ctrl+Alt+4 typed into the search`);
  // A tab whose digit there is none of.
  await stays(page, label, ["Control", "Alt"], "8");
  // The browser's: Alt and a digit (Firefox's tabs on Linux), Ctrl and a digit (the tabs everywhere).
  await stays(page, label, ["Alt"], "2");
  await stays(page, label, ["Control"], "2");
  await stays(page, label, ["Control", "Alt", "Shift"], "2");
  // Ctrl+C, S, P keep what the browser does with them.
  await clearKeys(page);
  for (const key of ["KeyC", "KeyS", "KeyP"]) await page.keyboard.press(`Control+${key}`);
  for (const key of ["KeyC", "KeyS", "KeyP"]) check((await prevented(page, key)).length === 1 && (await prevented(page, key)).every((p) => !p), `${label}: Ctrl+${key} lost its default`);
  check(!(await state(page)).shown, `${label}: Ctrl+C, S, P show the digits`);
  // A modal dialog has the page to itself.
  await page.evaluate(() => {
    const dialog = Object.assign(document.createElement("dialog"), { id: "navkeys-dialog", textContent: "modal" });
    document.body.append(dialog);
    dialog.showModal();
  });
  await stays(page, label, ["Control", "Alt"], "1");
  await page.evaluate(() => { const d = document.getElementById("navkeys-dialog"); d.close(); d.remove(); });
  await context.close();
}

// ---- Windows: Alt alone; Ctrl and Alt are AltGr there
{
  const label = "windows";
  const { context, page } = await open("Win32", "/catalog");
  await holding(page, ["Alt"], async () => {
    await page.waitForTimeout(HELD_MS);
    const held = await state(page);
    check(held.shown && held.digits === "12345", `${label}: Alt held shows ${held.shown ? held.digits : "nothing"}`);
  });
  // Let go after the digits showed: the Alt opens no menu of the browser (its keyup is taken).
  check((await prevented(page, "AltLeft", "keyup")).at(-1) === true, `${label}: the Alt let go after the digits keeps its default`);
  check(!(await state(page)).shown, `${label}: the digits stay after Alt is let go`);
  // A quick Alt is the browser's (its menu): its keyup keeps its default.
  await clearKeys(page);
  await page.keyboard.press("Alt");
  check((await prevented(page, "AltLeft", "keyup")).every((p) => !p), `${label}: a quick Alt lost its default`);
  await goes(page, label, ["Alt"], "1", "/");
  await goes(page, label, ["Alt"], "3", "/study");
  await stays(page, label, ["Control", "Alt"], "2");
  await stays(page, label, ["Control"], "2");
  await context.close();
}

// ---- a Mac: ⌃⌥ (⌥ and a digit types a character)
{
  const label = "mac";
  const { context, page } = await open("MacIntel", "/catalog");
  await goes(page, label, ["Control", "Alt"], "5", "/studyplan");
  await stays(page, label, ["Alt"], "1");
  await stays(page, label, ["Meta"], "1");
  await context.close();
}

// ---- where the app does not start: the keys load the tab's page
{
  const label = "classic";
  const { context, page } = await open("Linux x86_64", "/catalog", { noApp: true });
  await holding(page, ["Control", "Alt"], () => page.keyboard.press("Digit3"));
  await page.waitForURL((url) => url.pathname === "/programs", { timeout: 8000 }).catch(() => problems.push(`${label}: Ctrl+Alt+3 did not load „Studium" (${page.url()})`));
  await context.close();
}

await browser.close();

if (problems.length) {
  console.error(problems.join("\n"));
  console.error(`navkeys FAILED: ${problems.length} problem(s)`);
  process.exit(1);
}
console.log("navkeys ok");
