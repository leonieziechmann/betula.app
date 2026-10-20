// Checks „Knopf mit Pfeil" (folia/design/tasten/konzept.html; owner, 2026-10-20: „Ja bitte setze
// deine Vorschläge um"): an action does the usual at once, its arrow beside it opens what one may
// choose; the key does the action, Shift with it opens the arrow.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node arrows.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// - A row's arrow beside its bookmark: hidden at rest, there while the row is pointed at; it opens
//   „Sammlung und Notiz" for the row, where a collection chosen marks the module. ⇧M at a row the
//   same; S and N do nothing any more; M with Caps Lock (no Shift) marks.
// - A module's „Merken" has its arrow unmarked as well; ⇧M opens it at the open module.
// - „Einplanen"'s arrow and ⇧P, the module not planned: „Einplanen in" with the semesters, „jetzt"
//   at the current one, each saying whether it offers the module, under the switch beside the
//   rail; Esc closes it, the focus back at the arrow. One chosen plans the module there and the
//   note says so; „Rückgängig" takes it out again. Planned, the same arrow opens the menu of what
//   is planned (plannote.mjs has its entries).
// - On a Mac Strg is ⌘: the search's key, the tooltip of the Merkliste's keys.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";
import { fileURLToPath } from "node:url";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
const KEY = "betula.bookmarks.v1";
const SHELF = "betula.collections.v1";
const PLAN = "betula.studyplan.v1";
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };

const open = async (init) => {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await context.addInitScript(ON_PAGE);
  if (init) await context.addInitScript(init);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  const go = async (path) => {
    await page.goto(base + path, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
    await page.evaluate(() => { window.__marker = 1; });
  };
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
  const marks = () => page.evaluate((key) => (localStorage.getItem(key) || "").split("\n").filter(Boolean).map((line) => line.split("\t")[0]), KEY);
  const plannedIn = (id) => page.evaluate(([key, id]) => (localStorage.getItem(key) || "").split("\n").map((l) => l.split("\t")).filter((f) => f[0] === "m" && f[2] === id).map((f) => f[1]).sort(), [PLAN, id]);
  return { context, page, go, step, marks, plannedIn };
};

// ---------- the catalog: a row's arrow, ⇧M, no S and N, Caps Lock ----------
const { context, page, go, step, marks, plannedIn } = await open();
await go("/catalog");
await page.evaluate(([key, shelf, plan]) => {
  localStorage.removeItem(key);
  localStorage.setItem(shelf, "c\t1\tGrundlagen\n");
  localStorage.removeItem(plan);
  // A study begun in the winter 2025, so that planning asks nothing first.
  localStorage.setItem("betula.myprogram.v1", "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n");
}, [KEY, SHELF, PLAN]);
await go("/catalog");
await page.waitForFunction(() => allOnPage("#rows a.row").length >= 6, null, { timeout: 30000 }).catch(() => problems.push("the catalog lists no rows"));
await page.mouse.move(5, 890);
const ids = await page.evaluate(() => allOnPage("#rows a.row").slice(0, 6).map((row) => row.dataset.id));
const wrap = (id) => `.page-stage.front .row-wrap:has(a.row[data-id="${id}"])`;
const shown = (id) => page.evaluate((sel) => { const more = document.querySelector(`${sel} > .row-more`); return more ? getComputedStyle(more).opacity : "none"; }, wrap(id));
check((await shown(ids[0])) === "0", `a row's arrow at rest: ${await shown(ids[0])}`);
await page.hover(`${wrap(ids[0])} a.row`);
await page.waitForTimeout(200);
check((await shown(ids[0])) === "1", "a row's arrow does not show while the row is pointed at");
await step("the row's arrow opens „Sammlung und Notiz\"", () => page.click(`${wrap(ids[0])} > .row-more`), () => Boolean(document.activeElement?.closest?.(".collect-pop")));
await step("a collection chosen marks the module", () => page.click('.collect-pop .collect-item:has-text("Grundlagen")'), (id) => (localStorage.getItem("betula.bookmarks.v1") || "").includes(id), ids[0]);
await step("Esc closes it, the focus back at the arrow", () => page.keyboard.press("Escape"), () => !document.querySelector(".collect-pop") && document.activeElement?.classList.contains("row-more"));
// ⇧M at a row, S and N no more, M with Caps Lock.
await page.focus(`${wrap(ids[1])} a.row`);
await step("⇧M at a row opens „Sammlung und Notiz\"", () => page.keyboard.press("Shift+M"), () => Boolean(document.activeElement?.closest?.(".collect-pop")));
await page.keyboard.press("Escape");
await page.waitForFunction(() => !document.querySelector(".collect-pop"));
await page.focus(`${wrap(ids[1])} a.row`);
await page.keyboard.press("s");
await page.keyboard.press("n");
await page.waitForTimeout(400);
check(await page.evaluate(() => !document.querySelector(".collect-pop")), "S or N still opens „Sammlung und Notiz\"");
const before = await marks();
await page.focus(`${wrap(ids[2])} a.row`);
await step("M with Caps Lock marks", () => page.keyboard.press("M"), (n) => (localStorage.getItem("betula.bookmarks.v1") || "").split("\n").filter(Boolean).length === n, before.length + 1);
check(await page.evaluate(() => !document.querySelector(".collect-pop")), "M with Caps Lock opened the menu");

// ---------- a module: „Merken"'s arrow unmarked, ⇧M, „Einplanen"'s arrow and ⇧P ----------
await page.keyboard.press("Escape");
await step("a module beside the list", () => page.click(`${wrap(ids[3])} a.row`, { position: { x: 24, y: 12 } }), () => Boolean(onPage('.detail .hero [data-action="plan-more"]')));
const hero = await page.evaluate(() => {
  const visible = (sel) => { const el = onPage(sel); return Boolean(el) && getComputedStyle(el).visibility === "visible" && el.getBoundingClientRect().width > 0; };
  return { markMore: visible('.detail .hero [data-action="collect"]'), planMore: visible('.detail .hero [data-action="plan-more"]'), pressed: onPage('.detail .hero [data-action="mark"]')?.getAttribute("aria-pressed") };
});
check(hero.markMore && hero.planMore && hero.pressed === "false", `the module's arrows: ${JSON.stringify(hero)}`);
await page.evaluate(() => document.activeElement?.blur());
await step("⇧M at the module opens „Sammlung und Notiz\"", () => page.keyboard.press("Shift+M"), () => Boolean(document.activeElement?.closest?.(".collect-pop")));
await page.keyboard.press("Escape");
await page.waitForFunction(() => !document.querySelector(".collect-pop"));
await page.evaluate(() => document.activeElement?.blur());
await step("⇧P opens „Einplanen in\"", () => page.keyboard.press("Shift+P"), () => allOnPage(".plan-into [role=menuitemradio]").length >= 4 && Boolean(document.activeElement?.closest?.(".plan-into")));
const menu = await page.evaluate(() => ({
  head: document.querySelector(".plan-into .view-menu-head")?.textContent,
  now: [...document.querySelectorAll(".plan-into [role=menuitemradio]")].filter((item) => item.querySelector(".st-now")).length,
  notes: [...document.querySelectorAll(".plan-into [role=menuitemradio] .st-pop-note")].map((note) => note.textContent.trim()),
  checked: document.querySelectorAll('.plan-into [aria-checked="true"]').length,
  earlier: [...document.querySelectorAll(".plan-into [role=menuitem]")].some((item) => item.textContent.includes("Frühere Semester")),
}));
check(menu.head === "Einplanen in" && menu.now === 1 && menu.notes.every((note) => /^(angeboten|nicht angeboten)$/.test(note)) && menu.checked === 0 && menu.earlier, `„Einplanen in": ${JSON.stringify(menu)}`);
await step("Esc closes the menu, the focus back at the arrow", () => page.keyboard.press("Escape"), () => !document.querySelector(".plan-into") && document.activeElement?.dataset.action === "plan-more");
// The menu stands beside the rail, under the switch.
const placed = await page.evaluate(() => {
  document.querySelector('.detail .hero [data-action="plan-more"]').click();
  return new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(() => {
    const menu = document.querySelector(".plan-into")?.getBoundingClientRect();
    const pair = document.querySelector(".detail .hero .plan-pair")?.getBoundingClientRect();
    const rail = document.querySelector("aside.rail")?.getBoundingClientRect();
    done(menu && pair ? { right: Math.round(menu.right - pair.right), top: Math.round(menu.top - pair.bottom), clear: !rail || menu.left >= rail.right } : null);
  })));
});
check(placed && Math.abs(placed.right) <= 1 && placed.top >= 0 && placed.top <= 8 && placed.clear, `the menu at its switch: ${JSON.stringify(placed)}`);
await page.waitForFunction(() => document.activeElement?.closest?.(".plan-into"));
const second = await page.evaluate(() => document.querySelectorAll(".plan-into [role=menuitemradio]")[1]?.dataset.semester);
await step("a semester chosen plans the module there", () => page.click(`.plan-into [data-semester="${second}"]`), () => !document.querySelector(".plan-into") && /eingeplant/.test(document.querySelector(".planning-note")?.textContent || ""));
await page.waitForTimeout(300);
check(JSON.stringify(await plannedIn(ids[3])) === JSON.stringify([second]), `planned in: ${JSON.stringify(await plannedIn(ids[3]))} instead of ${second}`);
await step("„Rückgängig\" takes it out again", () => page.click(".planning-note .plan-toast-undo"), (id) => !(localStorage.getItem("betula.studyplan.v1") || "").split("\n").some((line) => line.split("\t")[0] === "m" && line.split("\t")[2] === id), ids[3]);
await page.waitForFunction(() => onPage('.detail .hero [data-action="plan"]')?.getAttribute("aria-pressed") === "false", null, { timeout: 4000 }).catch(() => problems.push("„Rückgängig\" leaves „Eingeplant\""));
await step("chosen again: planned there", async () => {
  await page.click('.detail .hero [data-action="plan-more"]');
  await page.waitForFunction(() => document.activeElement?.closest?.(".plan-into"));
  await page.click(`.plan-into [data-semester="${second}"]`);
}, () => onPage('.detail .hero [data-action="plan"]')?.getAttribute("aria-pressed") === "true");
await page.waitForTimeout(300);
// Planned, the arrow is the menu of what is planned.
await step("planned, the arrow opens the menu of what is planned", () => page.click('.detail .hero [data-action="plan-more"]'), () => Boolean(document.querySelector(".plan-menu.root #plan-menu-move")) && !document.querySelector(".plan-into"));
check((await page.evaluate(() => onPage('.detail .hero [data-action="plan-more"]')?.getAttribute("aria-label"))) === "Semester und bestanden", "the planned arrow's label is not „Semester und bestanden\"");
await page.keyboard.press("Escape");
await page.waitForFunction(() => !document.querySelector(".plan-menu"), null, { timeout: 2000 }).catch(() => problems.push("Esc leaves the menu of what is planned open"));
await page.keyboard.press("Escape");
await context.close();

// ---------- a Mac: ⌘ for Strg ----------
const mac = await open(() => {
  Object.defineProperty(Navigator.prototype, "platform", { get: () => "MacIntel" });
  Object.defineProperty(Navigator.prototype, "userAgentData", { get: () => undefined });
});
await mac.go("/catalog");
const keys = await mac.page.evaluate(() => ({ mac: document.documentElement.classList.contains("mac"), search: [...document.querySelectorAll(".page-stage.front .search kbd, .topbar kbd")].map((kbd) => kbd.innerText.trim()).filter(Boolean) }));
check(keys.mac && keys.search.includes("⌘K") && !keys.search.includes("Strg K"), `a Mac's search key: ${JSON.stringify(keys)}`);
await mac.page.evaluate((key) => localStorage.setItem(key, `11103\t${Math.floor(Date.now() / 1000)}\n`), KEY);
await mac.go("/bookmarks");
await mac.page.waitForFunction(() => document.querySelector(".page-stage.front .keys"), null, { timeout: 30000 }).catch(() => problems.push("the Merkliste shows no keys"));
const tip = await mac.page.evaluate(() => document.querySelector(".page-stage.front .keys")?.title || "");
check(tip.includes("⌘ und Klick") && !tip.includes("Strg"), `a Mac's tooltip of the keys: ${tip}`);
await mac.context.close();

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
