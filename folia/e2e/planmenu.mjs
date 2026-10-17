// Checks the menu of the semesters at „Einplanen" beside „Merken" (owner, 2026-10-17: „wenn man es
// lange gedrückt hält (auf dem handy) soll dabei sich die border aufladen und wrnn das voll ist soll
// ein dropdown mit so 3 bis 5 semestern kommen in die man das einplanen kann … Beim pc soll einfach
// on hover das dropdown kommen, oder wenn mann den shortcut lange gedrückt hält oder mit modifier";
// folia/crates/stores/src/studyplan/menu.rs, enhance.js). On a wide screen: a mouse resting on the
// switch opens it under the pair of switches, „Merken" uncovered, and leaving closes it; a semester
// taken plans there (one nothing is planned in yet included) and the menu goes; Shift+P opens it with
// the focus on the semester P plans into, the arrows and Enter take another, a digit too, Esc closes
// it with the preview still open; P pressed plans as ever, P held opens the menu and plans nothing;
// a right click opens it; a module offered in the summer alone says so of the winters. On a phone: a
// finger held fills the switch's border and then opens the menu across the screen, the finger's
// release takes nothing, a tap takes a semester; a tap plans as ever, a finger that moves opens
// nothing.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node planmenu.mjs   (SMOKE_BROWSER_CHANNEL=msedge by default)
// Needs a catalog with Datenbanken (12330, every semester) and Analysis I (11103, summer only), as
// the synthetic one has them (docs/folia/frontend.md §4); its current semester is the one of the day
// it was made. Fails on a console error or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const SWITCH = '.detail .hero .plan-toggle[data-action="plan"]';
const STORE = "betula.studyplan.v1";

const watch = (page) => {
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 600)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
};
// The semesters the plan holds the module in, from what the browser keeps.
const plannedIn = (page, id) => page.evaluate(([key, id]) => (localStorage.getItem(key) || "").split("\n").map((l) => l.split("\t")).filter((f) => f[0] === "m" && f[2] === id).map((f) => f[1]), [STORE, id]);
const waitPlanned = (page, id, list, what) => page.waitForFunction(([key, id, want]) => {
  const got = (localStorage.getItem(key) || "").split("\n").map((l) => l.split("\t")).filter((f) => f[0] === "m" && f[2] === id).map((f) => f[1]).sort();
  return JSON.stringify(got) === JSON.stringify([...want].sort());
}, [STORE, id, list], { timeout: 4000 }).catch(async () => problems.push(`${what}: planned in ${JSON.stringify(await plannedIn(page, id))}, not ${JSON.stringify(list)}`));
const menuGone = (page, what) => page.waitForFunction(() => !document.querySelector(".plan-menu"), null, { timeout: 2000 }).catch(() => problems.push(`${what}: the menu stayed`));
const menu = (page) => page.evaluate(() => {
  const el = document.querySelector(".plan-menu");
  if (!el) return null;
  const box = (e) => { const r = e.getBoundingClientRect(); return { left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width }; };
  return {
    box: box(el),
    shade: Boolean(document.querySelector(".plan-menu-shade")),
    keys: el.classList.contains("by-keys"),
    entries: [...el.querySelectorAll(".plan-menu-item")].map((e) => {
      const label = e.querySelector(".plan-menu-label");
      return { text: label.textContent, whole: label.scrollWidth <= label.clientWidth, note: e.querySelector(".plan-menu-note")?.textContent || "", checked: e.getAttribute("aria-checked") === "true", n: e.dataset.n };
    }),
    focus: document.activeElement?.closest?.(".plan-menu-item")?.dataset.n || null,
    mark: box(document.querySelector(".detail .hero .mark-toggle")),
    pair: box(document.querySelector(".detail .hero .switches")),
    expanded: document.querySelector('.detail .hero .plan-toggle[data-action="plan"]').getAttribute("aria-expanded"),
  };
});
const overlap = (a, b) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
const open = async (page, id) => {
  await page.goto(`${base}/catalog?open=${id}`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction((sel) => window.__betulaApp === true && document.querySelector(sel)?.checkVisibility(), SWITCH, { timeout: 120000 }).catch(() => problems.push(`${id}: the app or its switch never came`));
  await page.waitForTimeout(400);
};

// ---------- a wide screen ----------
{
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  watch(page);
  await open(page, "12330");

  // A mouse resting on the switch: the menu under the pair, „Merken" uncovered, five semesters.
  await page.hover(SWITCH);
  await page.waitForSelector(".plan-menu", { timeout: 2000 }).catch(() => problems.push("hover: no menu"));
  let m = await menu(page);
  if (m) {
    check(m.entries.length === 5, `hover: ${m.entries.length} semesters, not 5`);
    check(m.entries[0]?.text.includes("WiSe 2026/27") || m.entries[0]?.text.includes("SoSe 2027"), `hover: the first semester is ${m.entries[0]?.text}`);
    check(!m.shade, "hover: a shade over the page");
    check(!overlap(m.box, m.mark), "hover: the menu covers „Merken“");
    check(Math.abs(m.box.right - m.pair.right) <= 1 && m.box.top >= m.pair.bottom, `hover: not under the pair at its right edge (${JSON.stringify([m.box, m.pair])})`);
    check(m.expanded === "true", "hover: the switch does not say its menu is open");
    check(m.entries.every((e) => !e.checked), "hover: a semester checked in an empty plan");
  }
  await page.screenshot({ path: process.env.SHOTS ? `${process.env.SHOTS}/planmenu-hover.png` : undefined });
  // Away: it goes.
  await page.mouse.move(700, 450);
  await page.mouse.move(40, 450);
  await menuGone(page, "hover, the mouse gone");

  // A semester further on, nothing planned there yet: planned there, and the menu goes.
  await page.hover(SWITCH);
  await page.waitForSelector(".plan-menu", { timeout: 2000 });
  const third = (await menu(page))?.entries[2]?.text || "";
  await page.click(".plan-menu-item >> nth=2");
  await page.waitForFunction(() => document.querySelector('.plan-menu-item[data-n="3"]')?.getAttribute("aria-checked") === "true", null, { timeout: 1000 }).catch(() => problems.push("taken: the semester did not flip in the menu"));
  await menuGone(page, "taken");
  const later = await plannedIn(page, "12330");
  check(later.length === 1, `taken: planned in ${JSON.stringify(later)}`);
  const label = await page.evaluate((sel) => document.querySelector(sel).textContent, SWITCH);
  check(label.includes("Einplanen") && label.includes("geplant"), `taken (${third}): the switch says ${JSON.stringify(label)}`);

  // Shift+P: the focus on the semester P plans into; ↓ and Enter take the next one.
  await page.mouse.move(40, 450);
  await page.locator(".detail .hero h2").click();
  await page.keyboard.press("Shift+P");
  await page.waitForSelector(".plan-menu", { timeout: 2000 }).catch(() => problems.push("Shift+P: no menu"));
  m = await menu(page);
  check(m?.keys && m?.focus === "1", `Shift+P: the focus is not on the first semester (${m?.focus})`);
  await page.keyboard.press("ArrowDown");
  check((await menu(page))?.focus === "2", "Shift+P: ↓ did not go to the second semester");
  await page.keyboard.press("Enter");
  await menuGone(page, "Shift+P, Enter");
  const two = await plannedIn(page, "12330");
  check(two.length === 2, `Shift+P, Enter: planned in ${JSON.stringify(two)}`);
  check(await page.evaluate((sel) => document.activeElement === document.querySelector(sel), SWITCH), "Shift+P, Enter: the focus did not come back to the switch");
  // A digit takes its semester.
  await page.keyboard.press("Shift+P");
  await page.waitForSelector(".plan-menu", { timeout: 2000 });
  await page.keyboard.press("5");
  await menuGone(page, "Shift+P, 5");
  check((await plannedIn(page, "12330")).length === 3, "Shift+P, 5: the fifth semester was not planned");
  // Esc closes the menu, and only the menu.
  await page.keyboard.press("Shift+P");
  await page.waitForSelector(".plan-menu", { timeout: 2000 });
  await page.keyboard.press("Escape");
  await menuGone(page, "Esc");
  check(await page.evaluate(() => location.search.includes("open=12330")), "Esc: the preview closed with the menu");

  // P pressed plans as ever (into the semester it aims at); held, it opens the menu and plans nothing.
  // (An empty plan: the page keeps the plan it read, so it is read anew.)
  await page.evaluate((key) => localStorage.removeItem(key), STORE);
  await open(page, "12330");
  await page.locator(".detail .hero h2").click();
  await page.keyboard.press("p");
  await waitPlanned(page, "12330", ["2026W"], "P");
  await page.keyboard.down("p");
  await page.waitForTimeout(700);
  check(Boolean(await menu(page)), "P held: no menu");
  await page.keyboard.up("p");
  await page.waitForTimeout(300);
  check(Boolean(await menu(page)), "P held: the menu went when P was let go");
  check(JSON.stringify(await plannedIn(page, "12330")) === '["2026W"]', "P held: the plan changed");
  await page.keyboard.press("Escape");
  await menuGone(page, "P held, Esc");

  // A right click: the menu, a click beside it closes it.
  await page.click(SWITCH, { button: "right" });
  await page.waitForSelector(".plan-menu", { timeout: 2000 }).catch(() => problems.push("right click: no menu"));
  check((await menu(page))?.shade, "right click: no shade to click beside the menu");
  await page.mouse.click(40, 450);
  await menuGone(page, "right click, a click beside it");
  check(await page.evaluate(() => location.search.includes("open=12330")), "right click: the click beside the menu went somewhere");

  // A module of the summers: the winters say it is not offered there.
  await open(page, "11103");
  await page.keyboard.press("Shift+P");
  await page.waitForSelector(".plan-menu", { timeout: 2000 });
  m = await menu(page);
  const winters = m?.entries.filter((e) => e.text.includes("WiSe")) || [];
  check(winters.length >= 2 && winters.every((e) => e.note === "nicht angeboten"), `a summer module: the winters say ${JSON.stringify(winters)}`);
  check(m?.entries.filter((e) => e.text.includes("SoSe")).every((e) => e.note === ""), "a summer module: a summer says something");
  check(m?.entries.every((e) => e.whole), `a summer module: a semester is cut off beside its mark (${JSON.stringify(m?.entries.map((e) => e.text))})`);
  await page.keyboard.press("Escape");
  await context.close();
}

// ---------- a phone ----------
{
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
  const page = await context.newPage();
  watch(page);
  const cdp = await context.newCDPSession(page);
  await open(page, "12330");
  await page.waitForTimeout(600); // the sheet has come up
  const at = await page.evaluate((sel) => { const r = document.querySelector(sel).getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; }, SWITCH);
  const ring = () => page.evaluate((sel) => {
    const el = document.querySelector(sel);
    const rect = el.querySelector(".plan-charge rect");
    return { charge: el.getAttribute("data-charge"), offset: parseFloat(getComputedStyle(rect).strokeDashoffset), shown: getComputedStyle(el.querySelector(".plan-charge")).opacity };
  }, SWITCH);

  // Held: the border fills, then the menu.
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [at] });
  await page.waitForTimeout(260);
  const filling = await ring();
  check(filling.charge === "on" && filling.offset > 5 && filling.offset < 95 && filling.shown === "1", `held: the border is not filling (${JSON.stringify(filling)})`);
  check(!(await menu(page)), "held: the menu came before the border was full");
  await page.screenshot({ path: process.env.SHOTS ? `${process.env.SHOTS}/planmenu-charging.png` : undefined });
  await page.waitForTimeout(400);
  const full = await ring();
  check(full.charge === "full" && full.offset < 1, `held: the border is not full (${JSON.stringify(full)})`);
  let m = await menu(page);
  check(Boolean(m), "held: no menu");
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await page.waitForTimeout(450);
  m = await menu(page);
  check(Boolean(m), "held: the menu went with the finger");
  if (m) {
    check(m.entries.length === 5, `held: ${m.entries.length} semesters`);
    check(Math.abs(m.box.left - 8) <= 1 && Math.abs(m.box.right - 382) <= 1, `held: the menu is not across the screen 8px off its edges (${JSON.stringify(m.box)})`);
    check(m.shade, "held: no shade");
  }
  check((await plannedIn(page, "12330")).length === 0, "held: the finger's release planned the module");
  check((await ring()).charge === null, "held: the border stayed");
  await page.screenshot({ path: process.env.SHOTS ? `${process.env.SHOTS}/planmenu-phone.png` : undefined });
  // A tap takes a semester.
  await page.tap(".plan-menu-item >> nth=1");
  await menuGone(page, "phone, a semester tapped");
  check((await plannedIn(page, "12330")).length === 1, "phone: the semester tapped was not planned");

  // A tap on the switch plans as ever; a finger that moves opens nothing.
  await page.evaluate((key) => localStorage.removeItem(key), STORE);
  await open(page, "12330");
  await page.waitForTimeout(600);
  await page.touchscreen.tap(at.x, at.y);
  await waitPlanned(page, "12330", ["2026W"], "phone, a tap");
  check(!(await menu(page)), "phone, a tap: a menu");
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [at] });
  for (let i = 1; i <= 4; i++) {
    await page.waitForTimeout(30);
    await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: at.x, y: at.y + i * 6 }] });
  }
  await page.waitForTimeout(600);
  check(!(await menu(page)), "phone, a finger that moves: a menu");
  check((await ring()).charge === null, "phone, a finger that moves: the border fills");
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
