// Checks the phone layout of the catalog in the browser app: the filter sheet (the page behind it
// stands still while the sheet's list scrolls by itself; swiped down wherever its list is at the
// top, it closes; a short slow pull lets it snap back; a tap beside it and a mouse at its head
// close it too), what is picked in the sheet is a draft (the sheet counts, the list and the
// address follow once, when it closes, with one history entry), the picker staying open while
// the window shrinks (the on-screen keyboard), the area picker, and the virtual list (the page
// scrolls, the last rows come at its end, the height stays, rows do not overlap).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node phone.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a console error or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
const page = await context.newPage();
const cdp = await context.newCDPSession(page);
page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 600)); });
page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
await page.goto(base + "/catalog?program=bachelor-informatik-2008", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));

// A finger (Playwright itself only taps): down at (x, from), in `steps` moves `ms` apart to
// (x, to), up. The browser takes it as a real touch, its own scrolling included.
async function swipe(x, from, to, { steps = 10, ms = 16 } = {}) {
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y: from }] });
  for (let i = 1; i <= steps; i++) {
    await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x, y: from + ((to - from) * i) / steps }] });
    await page.waitForTimeout(ms);
  }
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await page.waitForTimeout(450); // a fling, a slide back or away
}
// The sheet slides in: a finger on it only counts once it stands still.
const settled = () => document.getElementById("filters").classList.contains("open") && document.documentElement.classList.contains("sheet-open") && getComputedStyle(document.getElementById("filters")).transform === "none";
const openSheet = async (what, { inPlace = false } = {}) => {
  // A step before that failed to close the sheet is reported there; the walk goes on.
  await page.evaluate(() => { if (document.getElementById("filters").classList.contains("open")) document.querySelector("#filters .sheet-close").click(); });
  await page.waitForTimeout(350);
  // A tap scrolls the button into view first; `inPlace` leaves the page where it is.
  if (inPlace) await page.evaluate(() => document.querySelector(".sheet-toggle").click());
  else await page.tap(".sheet-toggle");
  await page.waitForFunction(settled, null, { timeout: 5000 }).catch(() => problems.push(`${what}: the sheet did not open (or the page is not dimmed)`));
};
const isOpen = () => page.evaluate(settled);
const closed = (what) => page.waitForFunction(() => !document.getElementById("filters").classList.contains("open") && !document.documentElement.classList.contains("sheet-open"), null, { timeout: 5000 }).catch(() => problems.push(`${what}: the sheet did not close`));
const number = (text) => Number(String(text ?? "").replace(/\D/g, ""));
const pageAt = () => page.evaluate(() => Math.round(scrollY));
const body = () => page.evaluate(() => { const b = document.querySelector("#filters .body"); return { top: Math.round(b.scrollTop), max: b.scrollHeight - b.clientHeight }; });
// Where a finger can start inside the sheet's list: the middle of what is visible of it.
const bodyPoint = () => page.evaluate(() => { const r = document.querySelector("#filters .body").getBoundingClientRect(); return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2) }; });

// ---- the page behind the open sheet stands still, the sheet's list scrolls by itself
await page.evaluate(() => scrollTo(0, 600));
await page.waitForTimeout(300);
const behind = await pageAt();
check(behind > 300, `lock: the list is too short to tell (${behind})`);
await openSheet("lock", { inPlace: true });
check((await pageAt()) === behind, `lock: opening the sheet moved the page (${behind} → ${await pageAt()})`);
const sheetTop = await page.evaluate(() => Math.round(document.getElementById("filters").getBoundingClientRect().top));
check(sheetTop > 110, `lock: no room above the sheet to touch the dimmed page (${sheetTop})`);
await swipe(195, 12, sheetTop - 8); // down, on the dimmed page above the sheet
await swipe(195, sheetTop - 8, 12); // and up
check((await pageAt()) === behind && (await isOpen()), `lock: a swipe on the dimmed page moved the page (${behind} → ${await pageAt()}) or closed the sheet`);
const inside = await bodyPoint();
await swipe(inside.x, inside.y + 150, inside.y - 150); // up, inside the sheet
const scrolled = await body();
check(scrolled.top > 60, `lock: a swipe up in the sheet did not scroll its list (${scrolled.top})`);
for (let i = 0; i < 6; i++) await swipe(inside.x, inside.y + 150, inside.y - 200, { steps: 6, ms: 8 }); // flung past its end
const atEnd = await body();
check(atEnd.top >= atEnd.max - 2, `lock: the sheet's list did not reach its end (${atEnd.top} of ${atEnd.max})`);
check((await pageAt()) === behind, `lock: scrolling the sheet's list past its end moved the page (${behind} → ${await pageAt()})`);
// Down while its list is scrolled: the list scrolls back, the sheet stays.
await swipe(inside.x, inside.y - 100, inside.y + 100);
const back = await body();
check((await isOpen()) && back.top < atEnd.top, `lock: a swipe down in the scrolled list did not scroll it back (${atEnd.top} → ${back.top}) or closed the sheet`);
check((await pageAt()) === behind, "lock: scrolling the sheet's list back moved the page");

// ---- swiped down where its list is at the top, the sheet closes; a short slow pull snaps back
await page.evaluate(() => { document.querySelector("#filters .body").scrollTop = 0; });
await swipe(inside.x, inside.y, inside.y + 40, { steps: 20, ms: 30 });
check(await isOpen(), "swipe: a short slow pull closed the sheet (it should snap back)");
// The sheet follows the finger: halfway through a pull it stands lower.
await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: inside.x, y: inside.y }] });
for (let i = 1; i <= 6; i++) {
  await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: inside.x, y: inside.y + i * 20 }] });
  await page.waitForTimeout(16);
}
const following = await page.evaluate(() => getComputedStyle(document.getElementById("filters")).transform);
await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: inside.x, y: inside.y + 260 }] });
await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
check(following.includes("matrix"), `swipe: the sheet does not follow the finger (${following})`);
await closed("swipe: down from the middle of its list");
check((await pageAt()) === behind, `swipe: the page moved while the sheet was pulled down (${behind} → ${await pageAt()})`);
// A flick on its head, and one on the row of its buttons.
for (const [where, selector] of [["head", "#filters .panel-head h2"], ["buttons", "#filters .filter-actions"]]) {
  await openSheet(`swipe on the ${where}`);
  const box = await page.locator(selector).boundingBox();
  // Short of the distance that closes it by itself, so only the speed can. The page gets a touch
  // move per frame, and a headless browser draws few frames: two quick moves make a flick there.
  await swipe(box.x + 20, box.y + 8, Math.min(box.y + 128, 838), { steps: 2, ms: 0 });
  await closed(`swipe: a flick down on the ${where}`);
}

// ---- a mouse drags the sheet at its head (a narrow window on a desktop); a tap beside it closes it
await openSheet("mouse");
const head = await page.locator("#filters .panel-head").boundingBox();
await page.mouse.move(head.x + head.width / 2, head.y + 12);
await page.mouse.down();
await page.mouse.move(head.x + head.width / 2, head.y + 120, { steps: 6 });
const dragged = await page.evaluate(() => getComputedStyle(document.getElementById("filters")).transform);
await page.mouse.move(head.x + head.width / 2, head.y + 260, { steps: 6 });
await page.mouse.up();
check(dragged !== "none" && dragged.includes("matrix"), `mouse: the sheet does not follow the pointer (${dragged})`);
await closed("mouse: a drag down at its head");
await openSheet("tap beside");
await page.mouse.click(195, 60);
await closed("a tap beside it");

// ---- the picker on a phone: the keyboard shrinks the viewport, the popup stays
await openSheet("picker");
await page.tap("#pick-area");
await page.waitForFunction(() => document.activeElement?.id === "pick-area-search" && document.querySelector("#pick-area-list"), null, { timeout: 5000 }).catch(() => problems.push("picker: did not open on the phone"));
await page.setViewportSize({ width: 390, height: 500 }); // the on-screen keyboard
await page.evaluate(() => document.getElementById("pick-area-search")?.scrollIntoView());
await page.waitForTimeout(400);
check(await page.evaluate(() => Boolean(document.querySelector("#pick-area-list")) && document.querySelector("#pick-area").closest(".combo").hasAttribute("data-open")), "picker: the popup closed when the viewport shrank (the keyboard)");
await page.keyboard.type("praktische");
await page.waitForFunction(() => document.querySelector("#pick-area-list .combo-option .combo-label")?.textContent.includes("Praktische"), null, { timeout: 5000 }).catch(() => problems.push("picker: typing did not narrow the list"));
// Enter takes the first entry; the sheet then counts as many modules as the entry says, and the
// list holds them once the sheet is closed.
const picked = await page.evaluate(() => {
  const entry = document.querySelector("#pick-area-list .combo-option");
  return { name: entry?.querySelector(".combo-label")?.textContent, count: Number(entry?.querySelector("small")?.textContent.replace(/\D/g, "")) };
});
await page.keyboard.press("Enter");
await page.waitForFunction((n) => Number(document.querySelector("#filters .show")?.textContent.replace(/\D/g, "")) === n, picked.count, { timeout: 5000 }).catch(() => problems.push(`picker: the sheet does not count the ${picked.count} modules of „${picked.name}"`));
check(!(await page.evaluate(() => /[?&]area=/.test(location.search))), "picker: the address changed before the sheet was closed");
await page.setViewportSize({ width: 390, height: 844 });
await page.tap("#filters .filter-actions .show");
await closed("picker: its button");
await page.waitForFunction(() => /[?&]area=\d+/.test(location.search), null, { timeout: 5000 }).catch(() => problems.push("picker: closing the sheet did not pick the area"));
// The list before stays until the list of the area has come from the data worker.
await page.waitForFunction((n) => Number(document.querySelector(".count")?.textContent.replace(/\D/g, "")) === n, picked.count, { timeout: 5000 }).catch(() => {});
const inArea = await page.evaluate(() => Number(document.querySelector(".count").textContent.replace(/\D/g, "")));
check(picked.count > 0 && inArea === picked.count, `area: ${inArea} modules in „${picked.name}", the picker said ${picked.count}`);

// ---- what is picked in the sheet is a draft: the sheet counts at once, the list and the address
// wait until the sheet closes (the list is built once, not with every tap)
await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true && document.querySelector(".vlist"), null, { timeout: 120000 });
const state = () => page.evaluate(() => ({ list: document.querySelector(".count")?.textContent, show: document.querySelector("#filters .show")?.textContent, search: location.search, same: document.querySelector(".vlist")?.__same === true, entries: history.length }));
await page.evaluate(() => { document.querySelector(".vlist").__same = true; });
const start = await state();
await openSheet("draft");
for (const label of ["Winter", "Vorlesung"]) {
  const chip = `#filters a.chip:has-text("${label}")`;
  const shown = (await state()).show;
  await page.tap(chip);
  await page.waitForFunction((shown) => document.querySelector("#filters .show")?.textContent !== shown, shown, { timeout: 5000 }).catch(() => problems.push(`draft: a tap on ${label} did not change the count in the sheet`));
  check((await page.getAttribute(chip, "data-state")) === "with", `draft: ${label} does not show as picked`);
}
const drafted = await state();
check(drafted.search === start.search, `draft: a tap in the sheet changed the address (${drafted.search})`);
check(drafted.list === start.list && drafted.same, "draft: the list behind the sheet was rebuilt while the sheet was open");
check(await isOpen(), "draft: a tap closed the sheet");
check(number(drafted.show) > 0 && number(drafted.show) < number(start.show), `draft: the sheet's count did not narrow (${start.show} → ${drafted.show})`);
await page.tap("#filters .show");
await closed("draft: its button");
await page.waitForFunction(() => /turnus=winter/.test(location.search) && /form=lecture/.test(location.search), null, { timeout: 5000 }).catch(() => problems.push(`draft: the button did not apply the draft (${page.url()})`));
await page.waitForFunction((n) => Number(document.querySelector(".count")?.textContent.replace(/\D/g, "")) === n, number(drafted.show), { timeout: 5000 }).catch(() => problems.push(`draft: the list does not hold what the sheet said (${drafted.show})`));
check((await state()).entries === start.entries + 1, `draft: the sheet left ${(await state()).entries - start.entries} history entries (one is right)`);
// A swipe down applies what was picked as well.
await openSheet("draft by swipe");
const before = (await state()).show;
await page.tap('#filters a.chip:has-text("Übung")');
await page.waitForFunction((shown) => document.querySelector("#filters .show")?.textContent !== shown, before, { timeout: 5000 }).catch(() => problems.push("draft: a tap on Übung did not change the count in the sheet"));
const swiped = (await state()).show;
const top = await page.locator("#filters .panel-head h2").boundingBox();
await swipe(top.x + 20, top.y + 8, top.y + 200, { steps: 6, ms: 12 });
await closed("draft: a swipe");
await page.waitForFunction(() => /form=[^&]*exercise/.test(location.search), null, { timeout: 5000 }).catch(() => problems.push(`draft: the swipe did not apply the draft (${page.url()})`));
await page.waitForFunction((n) => Number(document.querySelector(".count")?.textContent.replace(/\D/g, "")) === n, number(swiped), { timeout: 5000 }).catch(() => problems.push(`draft: after the swipe the list does not hold what the sheet said (${swiped})`));
// „Zurücksetzen" in the sheet empties the draft; the list follows when the sheet goes.
await openSheet("reset");
await page.tap("#filters .panel-head a.ghost");
await page.waitForFunction((n) => Number(document.querySelector("#filters .show")?.textContent.replace(/\D/g, "")) === n, number(start.show), { timeout: 5000 }).catch(() => problems.push("reset: the sheet does not count the whole catalog"));
check(/turnus=winter/.test((await state()).search), "reset: the address changed before the sheet was closed");
await page.tap("#filters .sheet-close");
await closed("reset: its close button");
await page.waitForFunction(() => location.search === "", null, { timeout: 5000 }).catch(() => problems.push(`reset: closing the sheet did not show the whole catalog (${page.url()})`));
// Back closes the sheet, as in an app (the sheet is a step of its own in the history), and the
// list follows what was picked, like with any other way of closing it.
const beforeBack = await state();
await openSheet("back");
await page.tap('#filters a.chip:has-text("Seminar")');
await page.waitForFunction((shown) => document.querySelector("#filters .show")?.textContent !== shown, beforeBack.show, { timeout: 5000 }).catch(() => problems.push("back: a tap on Seminar did not change the count in the sheet"));
const backCount = (await state()).show;
await page.evaluate(() => history.back());
await closed("back: Back");
await page.waitForFunction(() => /form=seminar/.test(location.search), null, { timeout: 5000 }).catch(() => problems.push(`back: Back left the page or dropped what was picked (${page.url()})`));
await page.waitForFunction((n) => Number(document.querySelector(".count")?.textContent.replace(/\D/g, "")) === n, number(backCount), { timeout: 5000 }).catch(() => problems.push(`back: the list does not hold what the sheet said (${backCount})`));
check((await state()).entries === beforeBack.entries + 1, `back: the sheet left ${(await state()).entries - beforeBack.entries} history entries (one is right)`);
// From there Back walks the lists the sheet applied, one step each.
await page.evaluate(() => history.back());
await page.waitForFunction(() => location.search === "", null, { timeout: 5000 }).catch(() => problems.push(`back: did not return to the list before the sheet (${page.url()})`));
await page.evaluate(() => history.back());
await page.waitForFunction(() => /form=[^&]*exercise/.test(location.search), null, { timeout: 5000 }).catch(() => problems.push(`back: did not return to the list two sheets before (${page.url()})`));

// ---- the virtual list on a phone: the page scrolls, the last rows come when it is scrolled down
await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true && document.querySelector(".vlist"), null, { timeout: 120000 });
const total = await page.evaluate(() => Number(document.querySelector(".count").textContent.replace(/\D/g, "")));
const tall = await page.evaluate(() => document.documentElement.scrollHeight);
await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
await page.waitForFunction((total) => Math.max(-1, ...[...document.querySelectorAll(".vrow")].map((r) => Number(r.dataset.i))) === total - 1, total, { timeout: 8000 }).catch(() => problems.push("phone list: the last row did not render at the end"));
const after = await page.evaluate(() => document.documentElement.scrollHeight);
check(Math.abs(after - tall) < tall * 0.05, `phone list: the page changed its height from ${tall} to ${after}`);
const overlap = await page.evaluate(() => {
  const rows = [...document.querySelectorAll(".vrow")].map((r) => r.getBoundingClientRect()).sort((a, b) => a.top - b.top);
  return rows.some((r, i) => i > 0 && r.top < rows[i - 1].bottom - 1);
});
check(!overlap, "phone list: rendered rows overlap");
check(await page.evaluate(() => /[?&]page=\d+/.test(location.search)), "phone list: the URL does not follow the position");

// ---- a module is a sheet from below (owner, 2026-10-06: „wenn man wie bei der Übersicht nach
// bereichen so ein menu bekommt, dass sich dann von unten öffnet … erstmal bis zur hälfte … nach
// unten swipen oder es nach oben um es zu schließen oder den vollen bereich zu verwenden. Achte
// dabei darauf, dass man auch noch scrollen können muss in dem fenster"): half the screen, all up
// by a finger, its content scrolling there, down to half and away; the list stays where it was.
await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true && document.querySelector(".rows a.row"), null, { timeout: 120000 });
await page.waitForTimeout(800);
await page.evaluate(() => scrollTo(0, 400));
await page.waitForTimeout(300);
const sheetOf = () => page.evaluate(() => {
  const sheet = document.querySelector(".detail.is-module");
  if (!sheet || sheet.classList.contains("is-gone")) return null;
  const box = sheet.getBoundingClientRect();
  return { top: Math.round(box.top), left: Math.round(box.left), right: Math.round(box.right), full: sheet.classList.contains("is-full"), scrolled: Math.round(sheet.querySelector(":scope > .scroll")?.scrollTop ?? -1), title: Boolean(sheet.querySelector("h2")) };
});
const listAt = () => page.evaluate(() => ({ y: Math.round(scrollY), search: location.search, entries: history.length }));
const listed = await listAt();
const rowAt = await page.evaluate(() => { const row = [...document.querySelectorAll(".rows a.row")].find((a) => a.getBoundingClientRect().top > 220); const box = row.getBoundingClientRect(); return { x: box.left + 60, y: box.top + 20 }; });
await page.touchscreen.tap(rowAt.x, rowAt.y);
await page.waitForFunction(() => location.search.includes("open=") && document.querySelector(".detail.is-module h2"), null, { timeout: 8000 }).catch(() => problems.push("sheet: a tap on a row did not bring the module's sheet"));
await page.waitForTimeout(500);
let sheet = await sheetOf();
let list = await listAt();
check(sheet && !sheet.full && Math.abs(sheet.top - 844 * 0.48) < 4 && sheet.left === 0 && sheet.right === 390 && sheet.title, `sheet: the module does not come up to half the screen: ${JSON.stringify(sheet)}`);
check(list.y === listed.y && list.entries === listed.entries + 1, `sheet: the list moved under it, or the history took more than a step: ${JSON.stringify([listed, list])}`);
await swipe(200, sheet.top + 80, sheet.top - 220, { steps: 10, ms: 24 });
sheet = await sheetOf();
check(sheet?.full && sheet.top < 30, `sheet: a finger up does not take it all up: ${JSON.stringify(sheet)}`);
await swipe(200, 640, 260, { steps: 10, ms: 20 });
await page.waitForTimeout(600);
sheet = await sheetOf();
check(sheet?.full && sheet.scrolled > 100, `sheet: all up, its content does not scroll: ${JSON.stringify(sheet)}`);
await swipe(200, 300, 420, { steps: 8, ms: 20 });
await page.waitForTimeout(600);
const scrolledBack = await sheetOf();
check(scrolledBack?.full && scrolledBack.scrolled < sheet.scrolled, `sheet: a finger down while its content is scrolled moved the sheet, not the content: ${JSON.stringify([sheet, scrolledBack])}`);
await page.evaluate(() => { document.querySelector(".detail.is-module > .scroll").scrollTop = 0; });
await page.waitForTimeout(200);
await swipe(200, 120, 380, { steps: 12, ms: 30 });
sheet = await sheetOf();
check(sheet && !sheet.full && Math.abs(sheet.top - 844 * 0.48) < 4, `sheet: drawn down a third from all up, it does not stay half up: ${JSON.stringify(sheet)}`);
await swipe(200, sheet.top + 80, sheet.top + 420, { steps: 12, ms: 30 });
await page.waitForFunction(() => !location.search.includes("open=") && !document.querySelector(".detail.is-module"), null, { timeout: 4000 }).catch(() => problems.push("sheet: drawn down from half up, it did not close"));
list = await listAt();
check(list.y === listed.y && list.entries === listed.entries + 1 && list.search === listed.search, `sheet: closing it did not go back to the list as it was: ${JSON.stringify([listed, list])}`);
// A flick up takes it all up; a tap beside it, its × and Back close it.
await page.touchscreen.tap(rowAt.x, rowAt.y);
await page.waitForFunction(() => document.querySelector(".detail.is-module h2"), null, { timeout: 8000 }).catch(() => problems.push("sheet: it did not come again"));
await page.waitForTimeout(500);
await swipe(200, 600, 400, { steps: 4, ms: 12 });
check((await sheetOf())?.full, "sheet: a flick up does not take it all up");
await swipe(200, 200, 360, { steps: 4, ms: 12 });
check((await sheetOf())?.full === false, "sheet: a flick down from all up does not take it to half");
await page.touchscreen.tap(200, 150);
await page.waitForFunction(() => !location.search.includes("open=") && !document.querySelector(".detail.is-module"), null, { timeout: 4000 }).catch(() => problems.push("sheet: a tap beside it did not close it"));
await page.touchscreen.tap(rowAt.x, rowAt.y);
await page.waitForFunction(() => document.querySelector('.detail.is-module [data-action="close-detail"]'), null, { timeout: 8000 });
await page.waitForTimeout(500);
await page.tap('.detail.is-module [data-action="close-detail"]');
await page.waitForFunction(() => !location.search.includes("open=") && !document.querySelector(".detail.is-module"), null, { timeout: 4000 }).catch(() => problems.push("sheet: its × did not close it"));
await page.touchscreen.tap(rowAt.x, rowAt.y);
await page.waitForFunction(() => document.querySelector(".detail.is-module h2"), null, { timeout: 8000 });
await page.evaluate(() => history.back());
await page.waitForFunction(() => !location.search.includes("open=") && !document.querySelector(".detail.is-module"), null, { timeout: 4000 }).catch(() => problems.push("sheet: Back did not close it"));
check((await listAt()).y === listed.y, "sheet: the list did not stay where it was");
// „Vollbild": the module's own page.
await page.touchscreen.tap(rowAt.x, rowAt.y);
await page.waitForFunction(() => document.querySelector('.detail.is-module [data-action="fullscreen"]'), null, { timeout: 8000 });
await page.waitForTimeout(500);
await page.tap('.detail.is-module [data-action="fullscreen"]');
await page.waitForFunction(() => location.pathname.startsWith("/catalog/module/") && document.querySelector(".module-page h2"), null, { timeout: 8000 }).catch(() => problems.push("sheet: „Vollbild“ did not lead to the module's page"));

// ---- a tab held (owner, 2026-10-06: „wenn man die buttons in der nav bar lange gedrückt hält,
// dass dann eine special aktion kommt. Bei mein Studium währe das dann eine auswahl von
// Regelstudienplan, Wahlpflicht, Alle Studiengänge … Bei dem katalog könnte sich dann gleich der
// katalog mit offenem Filter öffnen"): „Studium"'s menu, the catalog with its filters.
await page.evaluate(() => localStorage.setItem("betula.myprogram.v1", "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n"));
await page.goto(base + "/study", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true && document.querySelector(".st-pager"), null, { timeout: 120000 });
await page.waitForTimeout(800);
const hold = async (area, ms = 600) => {
  const at = await page.evaluate((area) => { const box = document.querySelector(`.bottomnav > .nav[data-area="${area}"]`).getBoundingClientRect(); return { x: box.left + box.width / 2, y: box.top + box.height / 2 }; }, area);
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: at.x, y: at.y }] });
  await page.waitForTimeout(ms);
  const menu = await page.evaluate(() => [...document.querySelectorAll(".tab-menu .tab-menu-way")].map((a) => `${a.textContent.trim()} ${a.getAttribute("href")}`));
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await page.waitForTimeout(500);
  return menu;
};
const ways = await hold("programs");
check(ways.join("|") === "Regelstudienplan /programs/bachelor-informatik-2008/plan|Wahlpflicht & Bereiche /programs/bachelor-informatik-2008/areas|Alle Studiengänge /programs", `a tab held: „Studium“ does not offer its ways: ${JSON.stringify(ways)}`);
check(await page.evaluate(() => location.pathname === "/study" && Boolean(document.querySelector(".tab-menu"))), "a tab held: the finger's release went somewhere, or the menu went");
await page.touchscreen.tap(200, 200);
await page.waitForFunction(() => !document.querySelector(".tab-menu"), null, { timeout: 2000 }).catch(() => problems.push("a tab held: a tap beside the menu did not close it"));
check(await page.evaluate(() => location.pathname === "/study"), "a tab held: the tap beside the menu went somewhere");
await hold("programs");
await page.tap(".tab-menu .tab-menu-way >> nth=1");
await page.waitForFunction(() => location.pathname === "/programs/bachelor-informatik-2008/areas" && !document.querySelector(".tab-menu"), null, { timeout: 8000 }).catch(() => problems.push("a tab held: „Wahlpflicht & Bereiche“ did not lead there"));
// A short tap is the tab as ever; the catalog held opens its list with the filters.
await page.touchscreen.tap(...(await page.evaluate(() => { const box = document.querySelector('.bottomnav > .nav[data-area="programs"]').getBoundingClientRect(); return [box.left + box.width / 2, box.top + box.height / 2]; })));
await page.waitForFunction(() => location.pathname === "/study" && !document.querySelector(".tab-menu"), null, { timeout: 8000 }).catch(() => problems.push("a tab tapped: „Studium“ did not lead to „Mein Studium“"));
await hold("catalog");
await page.waitForFunction(() => location.pathname === "/catalog" && document.getElementById("filters")?.classList.contains("open"), null, { timeout: 8000 }).catch(() => problems.push("a tab held: the catalog did not open with its filters"));

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
