// Checks the filter panel of the catalog in the browser app, and the site's catalog without it.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node filters.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Walks: toggles (with → without → off, the panel is not rebuilt) → rows of toggles fill the width →
// program picker (typo-tolerant search, arrow keys, Enter, Esc, click outside, clear) → lecturer picker →
// credit slider → width of the panel (limits, localStorage) → the area picker and the list of a
// program (plan order, the semester at the row, the second page reached by scrolling, the note of
// a semester scrolling away, confirmed dates only) → nothing moves at the takeover (the site's
// catalog holds the panel's place) → the site's catalog without JavaScript.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
// What the check looks for is on the page in front (on-page.js): every tab's page is built behind
// it at the start (folia/crates/shell/src/stage.rs, `prebuild`), with parts of the same name.
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const newContext = browser.newContext.bind(browser);
browser.newContext = async (options) => { const context = await newContext(options); await context.addInitScript(ON_PAGE); return context; };
const page = await browser.newPage({ viewport: { width: 1500, height: 900 } });
const problems = [];
const timings = {};
page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));

const step = async (name, action, until, arg) => {
  const started = Date.now();
  await action();
  try {
    await page.waitForFunction(until, arg, { timeout: 8000 });
    // The page before stays as a picture in front of the new one while its answers come (pending.rs, `hold`).
    await page.waitForFunction(() => !onPage(".pending-page.held"), null, { timeout: 2000 });
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
  }
  timings[name] = Date.now() - started;
  if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
};
const check = (ok, message) => { if (!ok) problems.push(message); };
const param = (name) => page.evaluate((n) => new URL(location.href).searchParams.getAll(n).join("|"), name);
const count = () => page.evaluate(() => onPage(".count")?.textContent);

await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
await page.evaluate(() => { window.__marker = 1; onPage("#filters").__same = true; });

// ---- toggles: with → without → off; the panel is the same element throughout, the focus stays
const all = await count();
const talk = '#filters a.chip:has-text("Vortrag")';
await step("toggle: with", () => page.click(talk), () => new URL(location.href).searchParams.get("exam") === "presentation");
check((await page.getAttribute(talk, "data-state")) === "with" && (await page.getAttribute(talk, "aria-checked")) === "true", "toggle: the chip does not show 'with'");
const withTalk = await count();
await step("toggle: without", () => page.click(talk), () => new URL(location.href).searchParams.get("not-exam") === "presentation" && !location.search.includes("&exam=") && !location.search.includes("?exam="));
check((await page.getAttribute(talk, "data-state")) === "without", "toggle: the chip does not show 'without'");
check(await page.evaluate(() => [...allOnPage(".tag")].some((tag) => tag.textContent.includes("ohne Vortrag"))), "toggle: no tag 'ohne Vortrag' above the list");
const withoutTalk = await count();
check(all !== withTalk && withTalk !== withoutTalk && all !== withoutTalk, `toggle: counts did not change (${all}, ${withTalk}, ${withoutTalk})`);
check(await page.evaluate(() => document.activeElement?.textContent.includes("Vortrag")), "toggle: the chip lost the focus");
await step("toggle: space bar flips it off", () => page.keyboard.press(" "), () => !location.search.includes("exam"));
check((await page.getAttribute(talk, "data-state")) === "off", "toggle: the chip does not show 'off'");
check(await page.evaluate(() => onPage("#filters").__same === true), "the filter panel was rebuilt by a filter change");

// ---- every link of the panel follows the whole filter. (The first toggle once kept the link of
// the empty filter, so a click on it dropped everything else: a change of the filter that leaves
// a toggle's own state alone has to reach that toggle's link all the same.)
await step("two filters", async () => { await page.click('#filters a.chip:has-text("Vorlesung") >> visible=true'); await page.click('#filters a.chip:has-text("Klausur") >> visible=true'); }, () => location.search.includes("form=lecture") && location.search.includes("exam=written"));
const forgetful = await page.evaluate(() => [...allOnPage('#filters a[role="checkbox"], #filters a[role="radio"]')]
  .filter((a) => a.offsetParent && !(a.getAttribute("href").includes("form=lecture") && a.getAttribute("href").includes("exam=written")))
  .map((a) => `${a.textContent.trim()} → ${a.getAttribute("href")}`));
check(forgetful.length === 0, `links that drop the rest of the filter: ${forgetful.slice(0, 4).join(" | ")}`);
await step("the first toggle keeps the others", () => page.click('#filters a.chip:has-text("Winter") >> visible=true'), () => ["turnus=winter", "form=lecture", "exam=written"].every((part) => location.search.includes(part)));
await step("reset", () => page.click('#filters a:has-text("Zurücksetzen") >> visible=true'), () => location.search === "");

// ---- every toggle has its box, and the toggles of a row share the row's whole width
const layout = await page.evaluate(() => {
  const issues = [];
  for (const chips of allOnPage("#filters .chips")) {
    if (!chips.offsetParent) continue; // inside the closed „Weitere Filter"
    const frame = chips.getBoundingClientRect();
    const rows = new Map();
    for (const chip of chips.querySelectorAll(".chip")) {
      if (!chip.querySelector(".box")) issues.push(`no box: ${chip.textContent}`);
      const r = chip.getBoundingClientRect();
      const row = rows.get(Math.round(r.top)) || { left: Infinity, right: -Infinity };
      rows.set(Math.round(r.top), { left: Math.min(row.left, r.left), right: Math.max(row.right, r.right) });
    }
    for (const [top, row] of rows) {
      if (Math.abs(row.left - frame.left) > 1.5 || Math.abs(row.right - frame.right) > 1.5) issues.push(`row at ${top} spans ${Math.round(row.left)}–${Math.round(row.right)} of ${Math.round(frame.left)}–${Math.round(frame.right)}`);
    }
  }
  return issues;
});
layout.forEach((issue) => problems.push("toggles: " + issue));

// ---- virtual oversizing: a toggle also reacts just outside of what it shows, up to the middle of the gap
const beyond = await page.evaluate(() => {
  const chips = [...allOnPage("#filters .chips")].find((c) => c.querySelectorAll(".chip").length > 3);
  const [a, b] = chips.querySelectorAll(".chip");
  const ra = a.getBoundingClientRect(), rb = b.getBoundingClientRect();
  const owner = (x, y) => document.elementFromPoint(x, y)?.closest(".chip");
  return { above: owner(ra.left + 20, ra.top - 2) === a, leftOfGap: owner(ra.right + 1.5, ra.top + 10) === a, rightOfGap: owner(rb.left - 1.5, rb.top + 10) === b };
});
check(beyond.above && beyond.leftOfGap && beyond.rightOfGap, `oversizing: the area around a toggle is not its own (${JSON.stringify(beyond)})`);

// ---- the search field starts exactly where the list starts
const edges = await page.evaluate(() => [onPage(".search").getBoundingClientRect().left, onPage(".panel.list").getBoundingClientRect().left]);
check(Math.abs(edges[0] - edges[1]) < 0.01, `top bar: the search starts at ${edges[0]}, the list at ${edges[1]}`);

// ---- program picker
const pop = "#pick-program-list";
const active = () => page.evaluate(() => onPage(".combo-option.active .combo-label")?.textContent || null);
await step("picker opens", () => page.click("#pick-program >> visible=true"), () => document.activeElement?.id === "pick-program-search" && allOnPage("#pick-program-list .combo-option").length > 50);
check((await active()) === "Alle Studiengänge", `picker: starts at ${await active()} instead of the selection`);
const doubles = await page.evaluate(() => {
  const seen = new Set(), twice = [];
  for (const option of allOnPage("#pick-program-list .combo-option")) {
    if (seen.has(option.textContent)) twice.push(option.textContent);
    seen.add(option.textContent);
  }
  return twice;
});
check(doubles.length === 0, `picker: entries that cannot be told apart: ${doubles.slice(0, 3).join(" | ")}`);
// One shape for all: name, short degree, year of the PO; the form of study only where two would read the same.
const shapes = await page.evaluate(() => [...allOnPage("#pick-program-list .combo-option small")].map((el) => el.textContent).filter((text) => !/^[^·]+ · \d{4}( · [^·]+)?$/.test(text) || /SÄ|NF|PO/.test(text)));
check(shapes.length === 0, `picker: entries of another shape: ${shapes.slice(0, 3).join(" | ")}`);

// A typo, a second word, and the best match on top.
await step("picker: search with a typo", () => page.keyboard.type("infomatik bsc"), () => onPage("#pick-program-list .combo-option .combo-label")?.textContent === "Informatik");
check((await active()) === "Informatik", `picker: the first match is not marked (${await active()})`);
// The arrow keys move the mark; a resting mouse pointer over the list does not take it back.
await page.fill("#pick-program-search >> visible=true", "inf");
await page.waitForFunction(() => allOnPage("#pick-program-list .combo-option").length > 3);
const box = await page.locator(`${pop} .combo-option >> nth=0`).boundingBox();
await page.mouse.move(box.x + 40, box.y + box.height / 2);
const first = await active();
await page.keyboard.press("ArrowDown");
await page.keyboard.press("ArrowDown");
const third = await active();
const expectedThird = await page.evaluate(() => allOnPage("#pick-program-list .combo-option .combo-label")[2]?.textContent);
check(third === expectedThird && third !== null, `picker: two steps down mark "${third}", expected "${expectedThird}" (first was "${first}")`);
check((await page.getAttribute("#pick-program-search >> visible=true", "aria-activedescendant")) === "pick-program-option-2", "picker: aria-activedescendant does not follow");
await page.keyboard.press("ArrowUp");
await page.keyboard.press("ArrowUp");
await page.keyboard.press("ArrowUp");
const last = await page.evaluate(() => [...allOnPage("#pick-program-list .combo-option .combo-label")].pop()?.textContent);
check((await active()) === last, `picker: arrow up from the first entry does not wrap to the last (${await active()} / ${last})`);

await page.fill("#pick-program-search >> visible=true", "informatik b.sc");
await page.waitForFunction(() => onPage("#pick-program-list .combo-option .combo-label")?.textContent === "Informatik");
await step("picker: Enter takes the marked entry", () => page.keyboard.press("Enter"), () => new URL(location.href).searchParams.get("program")?.startsWith("bachelor-informatik") && !onPage(".combo-pop") && onPage(".rows .plan-sem"));
check(await page.evaluate(() => document.activeElement?.id === "pick-program"), "picker: the focus did not return to the button");
check((await page.textContent("#pick-program .combo-value >> visible=true")).startsWith("Informatik"), "picker: the button does not show the selection");
check(await page.evaluate(() => onPage("#filters").__same === true), "the filter panel was rebuilt by picking a program");

// Esc closes the picker and nothing else: the preview stays.
await step("open a preview", () => page.click("a.row >> visible=true >> nth=2"), () => location.search.includes("open=") && onPage(".detail h2"));
// In the filters, a mode of the keyboard, ↓ goes on to the next filter, and Alt + ↓ opens the picker
// (combobox.rs; folia/design/tastatur/konzept.html).
await step("picker opens by keyboard", async () => { await page.focus("#pick-program >> visible=true"); await page.keyboard.press("Alt+ArrowDown"); }, () => document.activeElement?.id === "pick-program-search");
check((await active()) === "Informatik", `picker: reopened at "${await active()}" instead of the selected program`);
await step("picker: Esc closes it", () => page.keyboard.press("Escape"), () => !onPage(".combo-pop") && document.activeElement?.id === "pick-program");
check((await param("open")) !== "", "picker: Esc also closed the module preview");
// Esc in the filters leads out into the list (a mode of the keyboard: folia/design/tastatur/
// konzept.html), the next one closes the module beside it.
await step("Esc leaves the filters for the list", () => page.keyboard.press("Escape"), () => Boolean(document.activeElement?.closest(".rows")) && location.search.includes("open="));
await page.keyboard.press("Escape");
await page.waitForFunction(() => !location.search.includes("open="));

await step("picker opens again", () => page.click("#pick-program >> visible=true"), () => Boolean(onPage(".combo-pop")));
await step("picker: a click outside closes it", () => page.click(".count-row >> visible=true"), () => !onPage(".combo-pop"));
await step("picker: a click on an entry", async () => { await page.click("#pick-program >> visible=true"); await page.fill("#pick-program-search >> visible=true", "architektur bachelor"); await page.click(`${pop} .combo-option >> nth=0`); }, () => new URL(location.href).searchParams.get("program")?.includes("architektur") && !onPage(".combo-pop"));
await step("picker: clear", () => page.click("#pick-program ~ .combo-clear >> visible=true"), () => !location.search.includes("program="));

// ---- lecturer picker: adds a person as a toggle (with → without → gone)
await page.click('#filters summary:has-text("Weitere Filter") >> visible=true');
await step("lecturer picker opens", () => page.click("#pick-lecturer >> visible=true"), () => allOnPage("#pick-lecturer-list .combo-option").length > 50);
const person = await page.evaluate(() => allOnPage("#pick-lecturer-list .combo-option .combo-label")[5]?.textContent || "");
const family = person.split(",")[0];
await page.fill("#pick-lecturer-search >> visible=true", family);
await page.waitForFunction((name) => onPage("#pick-lecturer-list .combo-option .combo-label")?.textContent.startsWith(name), family);
const picked = await page.evaluate(() => onPage("#pick-lecturer-list .combo-option .combo-label").textContent);
await step("lecturer: Enter adds the person", () => page.keyboard.press("Enter"), (name) => new URL(location.href).searchParams.get("lecturer") === name && onPage('.people .person[data-state="with"]'), picked);
const onlyFirst = await count();
// Academic titles: small, in the picker and under the chosen name.
await step("lecturer picker opens again", () => page.click("#pick-lecturer >> visible=true"), () => allOnPage("#pick-lecturer-list .combo-option").length > 50);
check(await page.evaluate(() => [...allOnPage("#pick-lecturer-list .combo-option small")].some((el) => el.textContent.startsWith("Prof."))), "lecturer: the picker shows no academic titles");
check(!(await page.evaluate((name) => [...allOnPage("#pick-lecturer-list .combo-label")].some((el) => el.textContent === name), picked)), "lecturer: a chosen person is offered again");
// A second wanted person is an alternative: the list grows (or stays), it never shrinks.
await page.fill("#pick-lecturer-search >> visible=true", "prof dr");
await page.waitForFunction(() => allOnPage("#pick-lecturer-list .combo-option").length > 3);
await step("lecturer: a second person, by a click", () => page.click('#pick-lecturer-list .combo-option:has(small:text-matches("^Prof\.")) >> visible=true >> nth=0'), () => new URL(location.href).searchParams.getAll("lecturer").length === 2 && allOnPage(".people .person").length === 2);
const either = await count();
const number = (text) => Number(String(text).replace(/\D/g, ""));
check(number(either) >= number(onlyFirst) && number(either) > 0, `lecturer: two wanted persons are not alternatives (${onlyFirst} → ${either})`);
check(await page.evaluate(() => [...allOnPage(".people .person-name small")].some((el) => el.textContent.startsWith("Prof."))), "lecturer: no title under the chosen name");
const second = await page.evaluate(() => allOnPage(".people .person-name b")[1].textContent);
await step("lecturer: × excludes", () => page.click(".people .person >> visible=true >> nth=1 >> a.cross"), (name) => new URL(location.href).searchParams.get("not-lecturer") === name && allOnPage('.people .person[data-state="without"]').length === 1, second);
await step("lecturer: + wants again", () => page.click(".people .person >> visible=true >> nth=1 >> a.plus"), () => new URL(location.href).searchParams.getAll("lecturer").length === 2 && !location.search.includes("not-lecturer"));
await step("lecturer: the button on the right removes", async () => { await page.click(".people .person >> visible=true >> nth=1 >> a.remove"); await page.click(".people .person >> visible=true >> nth=0 >> a.remove"); }, () => !location.search.includes("lecturer") && !onPage(".people"));

// ---- credits: drag the lower thumb, step the upper one with the keyboard
await page.evaluate(() => onPage("#filters .slider").scrollIntoView({ block: "center" }));
const track = await page.locator("#filters .slider >> visible=true").boundingBox();
const thumb = track.height; // the grabbed thumb is as high as the slider and larger than the knob it shows
const at = (credits) => track.x + thumb / 2 + ((track.width - thumb) * credits) / 30;
await page.mouse.move(at(0), track.y + track.height / 2);
await page.mouse.down();
await page.mouse.move(at(6), track.y + track.height / 2, { steps: 8 });
await step("slider: letting go filters", () => page.mouse.up(), () => new URL(location.href).searchParams.get("ects_min") === "6");
check((await page.inputValue('#filters input[name="ects_min"]')) === "6", "slider: the number field does not follow");
await page.focus("#filters .slider input >> visible=true >> nth=1");
// − and + change a knob, the arrows go on from it (folia/design/tastatur/konzept.html; owner,
// 2026-10-20: „Mach mal +- als inc und dec und die pfeiltasten für die navigation").
await step("slider: − on the upper thumb", () => page.keyboard.press("-"), () => new URL(location.href).searchParams.get("ects_max") === "29");
await page.keyboard.press("ArrowLeft");
check((await page.evaluate(() => document.activeElement === allOnPage("#filters .slider input")[0])) && new URL(page.url()).searchParams.get("ects_max") === "29", "slider: ← on the upper thumb changed it instead of going to the lower one");
// The upper thumb takes the lower one along where it passes it (the same concept: „Kommt ein Regler
// über den anderen, nimmt er ihn mit").
await page.mouse.move(at(29), track.y + track.height / 2);
await page.mouse.down();
await page.mouse.move(at(4), track.y + track.height / 2, { steps: 8 });
await step("slider: a thumb takes the other along", () => page.mouse.up(), () => { const p = new URL(location.href).searchParams; return p.get("ects_max") === "4" && p.get("ects_min") === "4"; });
await step("slider: typed numbers", async () => { await page.fill('#filters input[name="ects_max"] >> visible=true', "12"); await page.keyboard.press("Tab"); }, () => new URL(location.href).searchParams.get("ects_max") === "12");
check((await page.textContent("#filters .fgroup:has(.slider) .flabel span >> visible=true")) === "4–12 LP", "slider: the summary does not say 4–12 LP");

// ---- width of the panel: dragged, limited, remembered outside the URL
const width = () => page.evaluate(() => Math.round(onPage("#filters").getBoundingClientRect().width));
const drag = async (dx) => {
  const edge = await page.locator('[data-action="resize-filters"] >> visible=true').boundingBox();
  await page.mouse.move(edge.x + edge.width / 2, edge.y + 200);
  await page.mouse.down();
  await page.mouse.move(edge.x + edge.width / 2 + dx, edge.y + 200, { steps: 6 });
  await page.mouse.up();
};
const startWidth = await width();
// The page follows the handle live (list and search field move with it). Only where a page cannot keep
// up, the rest of the drag moves the panel alone and the page follows when the handle is let go; the
// test forces that with a budget no frame can meet, and the live drag with one every frame meets:
// whether a page keeps up depends on the machine (a slow one without a GPU falls back after a few
// frames; `resize-perf.mjs` measures it), what either mode does does not.
{
  const state = () => page.evaluate(() => ({
    panel: Math.round(onPage("#filters").getBoundingClientRect().width),
    list: Math.round(onPage(".panel.list").getBoundingClientRect().width),
    search: Math.round(onPage(".search").getBoundingClientRect().left),
    variable: document.documentElement.style.getPropertyValue("--w-filters"),
    mode: document.documentElement.dataset.resizeMode || null,
  }));
  const dragBy = async (dx) => {
    const edge = await page.locator('[data-action="resize-filters"] >> visible=true').boundingBox();
    await page.mouse.move(edge.x + edge.width / 2, edge.y + 200);
    await page.mouse.down();
    await page.mouse.move(edge.x + edge.width / 2 + dx, edge.y + 200, { steps: 12 });
    await page.waitForTimeout(150);
    const during = await state();
    await page.mouse.up();
    await page.waitForTimeout(150);
    return { during, after: await state() };
  };
  const before = await state();
  await page.evaluate(() => { document.documentElement.dataset.resizeBudget = "100000"; });
  const live = await dragBy(60);
  await page.evaluate(() => { delete document.documentElement.dataset.resizeBudget; });
  check(live.during.mode === "live" && Math.abs(live.during.panel - (before.panel + 60)) <= 2 && live.during.list === before.list - (live.during.panel - before.panel) && live.during.search === before.search + (live.during.panel - before.panel),
    `resize: the page does not follow the handle live: ${JSON.stringify(live.during)}, before ${JSON.stringify(before)}`);
  check(live.after.mode === null && live.after.variable === live.after.panel + "px", `resize: after letting go ${JSON.stringify(live.after)}`);
  await page.dblclick('[data-action="resize-filters"] >> visible=true');

  await page.evaluate(() => { document.documentElement.dataset.resizeBudget = "0.001"; });
  const forced = await dragBy(60);
  check(forced.during.mode === "panel" && Math.abs(forced.during.panel - (before.panel + 60)) <= 2 && forced.during.list > before.list - 60, `resize: the fallback did not take over: ${JSON.stringify(forced.during)}`);
  check(forced.after.mode === null && forced.after.variable === forced.after.panel + "px" && forced.after.list === before.list - (forced.after.panel - before.panel) && Math.abs(forced.after.panel - (before.panel + 60)) <= 2,
    `resize: after the fallback the page did not catch up: ${JSON.stringify(forced.after)}`);
  await page.evaluate(() => { delete document.documentElement.dataset.resizeBudget; });
  await page.dblclick('[data-action="resize-filters"] >> visible=true');
}
// The zone that takes the pointer is much wider than the grip, and none of it lies over the panel (its scrollbar).
const zone = await page.evaluate(() => { const z = onPage('[data-action="resize-filters"]').getBoundingClientRect(); return { width: z.width, over: onPage("#filters").getBoundingClientRect().right - z.left }; });
check(zone.width >= 16 && zone.over <= 0.5, `resize: the handle's zone is ${zone.width}px wide and ${zone.over}px over the filter panel`);
await drag(60);
check(Math.abs((await width()) - (startWidth + 60)) <= 2, `resize: expected about ${startWidth + 60}px, got ${await width()}px`);
check(Number(await page.evaluate(() => localStorage.getItem("betula.filters.width"))) === (await width()), "resize: the width is not in localStorage");
await drag(600);
check((await width()) === 440, `resize: the upper limit is 440px, got ${await width()}px`);
await drag(-900);
check((await width()) === 232, `resize: the lower limit is 232px, got ${await width()}px`);
check(!(await page.evaluate(() => location.search)).includes("232"), "resize: the width leaked into the URL");
await page.dblclick('[data-action="resize-filters"] >> visible=true');
check((await width()) === startWidth && (await page.evaluate(() => localStorage.getItem("betula.filters.width"))) === null, `resize: a double click does not reset (${await width()}px)`);

// ---- the list of a program: in plan order with the semester at every row (no headings between
// the semesters: the semester filter is for that), and the areas of its tree as a picker
await step("program list", async () => { await page.click('#filters a:has-text("Zurücksetzen") >> visible=true'); await page.click("#pick-program >> visible=true"); await page.fill("#pick-program-search >> visible=true", "informatik b.sc"); await page.keyboard.press("Enter"); }, () => onPage(".rows .plan-sem") && !onPage(".rows .sem") && onPage("#pick-area"));
const semesters = await page.evaluate(() => [...allOnPage(".rows .vrow")].slice(0, 12).map((row) => row.querySelector(".plan-sem")?.textContent ?? null));
check(semesters[0] === "1. Semester" && semesters.every((s) => s === null || /^\d+\. Semester$/.test(s)), `program list: the first rows say ${semesters.join(" | ")}`);
await step("scroll into the second page", () => page.evaluate(() => { onPage("#catalog-scroll").scrollTop = 60 * 58 + 120; }), () => /[?&]page=2\b/.test(location.search) && onPage('.vrow[data-i="60"] a.row'));
// A semester lists what the plan places there and, for the plan's requirement rows („Wahlpflicht…"),
// the modules that can be chosen; the note above the list says so and where they come from.
await step("semester 3", () => page.click('#filters .seg[aria-label="Fachsemester"] a:has-text("3") >> visible=true'), () => new URL(location.href).searchParams.get("semester") === "3" && onPage(".plan-note"));
const semester3 = await page.evaluate(() => ({
  count: Number(onPage(".count").textContent.replace(/\D/g, "")),
  placed: [...allOnPage(".rows .vrow .plan-sem")].filter((s) => s.textContent === "3. Semester").length,
  rows: allOnPage(".rows .vrow").length,
  note: onPage(".plan-note")?.textContent || "",
  items: allOnPage(".plan-note li").length,
  links: allOnPage(".plan-note a").length,
  first: onPage(".rows .vrow")?.dataset.i,
  top: onPage("#catalog-scroll").scrollTop,
  search: location.search,
}));
check(semester3.first === "0" && semester3.top === 0 && !semester3.search.includes("page="), `semester: the list of the new filter does not start at the top (row ${semester3.first} at ${semester3.top}px, ${semester3.search})`);
check(semester3.count > semester3.placed && semester3.placed > 0, `semester: ${semester3.count} modules listed, ${semester3.placed} of them placed in the semester by the plan`);
check(/Regelstudienplan sieht im 3\. Semester/.test(semester3.note) && /abgeleitet/.test(semester3.note), `semester: the note does not say what the plan asks for (${semester3.note.slice(0, 120)})`);
// A line a row of the plan, how much and from where, each thing once (owner, 2026-09-23).
check(semester3.items > 0 && /≥\s6\sLP/.test(semester3.note) && !/vermutlich|Käme dem Namen|gleich gut passend/.test(semester3.note), `semester: the note is not a line a row (${semester3.note.slice(0, 200)})`);
// The note scrolls away with the rows; the heads of the columns stay at the top, under the list's
// head, which stays at the top of the page's scroll area.
const scrolled = await page.evaluate(async () => {
  const area = onPage("#catalog-scroll"), rows = onPage(".rows"), note = onPage(".plan-note"), head = onPage(".rows > .cols");
  area.scrollTop = 400;
  await new Promise((resolve) => setTimeout(resolve, 50));
  const top = onPage("#rows-top").getBoundingClientRect().bottom;
  const at = { inRows: note.parentElement === rows, note: note.getBoundingClientRect().bottom - top, head: head.getBoundingClientRect().top - top, search: location.search };
  area.scrollTop = 0;
  return at;
});
check(scrolled.inRows && scrolled.note < 0 && Math.abs(scrolled.head) < 1 && !scrolled.search.includes("page="), `semester: the note stays or the heads of the columns go (${JSON.stringify(scrolled)})`);
// „Termine: bestätigt" keeps the rows with published teaching events, none that says „noch keine" (owner, 2026-09-23).
await step("events: confirmed only", () => page.click('#filters a.chip:has-text("Bestätigt") >> visible=true'), () => new URL(location.href).searchParams.get("events") === "yes" && [...allOnPage(".tag")].some((tag) => tag.textContent.includes("Termine")));
const confirmed = await page.evaluate(() => ({
  count: Number(onPage(".count").textContent.replace(/\D/g, "")),
  rows: allOnPage(".rows .vrow").length,
  none: allOnPage(".rows .vrow .events.none").length,
  semester: new URL(location.href).searchParams.get("semester"),
}));
check(confirmed.rows > 0 && confirmed.none === 0 && confirmed.count <= semester3.count && confirmed.semester === "3", `events: ${confirmed.none} of ${confirmed.rows} rows without dates, ${confirmed.count} of ${semester3.count} listed (${JSON.stringify(confirmed)})`);
await step("events: the tag takes it out again", () => page.click('.tag:has(em:text("Termine")) a >> visible=true'), () => !location.search.includes("events="));
check(await page.evaluate(() => onPage("#filters").__same === true), "the filter panel was rebuilt by choosing a semester");
await step("all semesters again", () => page.click('#filters .seg[aria-label="Fachsemester"] a:has-text("Alle") >> visible=true'), () => !location.search.includes("semester=") && !onPage(".plan-note"));
await step("area picker opens", () => page.click("#pick-area >> visible=true"), () => document.activeElement?.id === "pick-area-search" && allOnPage("#pick-area-list .combo-option").length > 3);
// Only the areas a student chooses from are offered (the fixed complexes are not), named without
// „Wahlpflichtmodule", in sections (folia_plans::areas::catalog_areas): the own electives first,
// without a heading, then the Nebenfach — never a phase of the studies or „Komplex" as a heading,
// never a heading twice, never a heading over a single area.
const areaNames = await page.evaluate(() => [...allOnPage("#pick-area-list .combo-option .combo-label")].map((el) => el.textContent).filter((name) => name !== "Alle Bereiche"));
const areaGroups = await page.evaluate(() => [...allOnPage("#pick-area-list .combo-group:not(.combo-rule)")].map((el) => el.textContent));
const areaSections = await page.evaluate(() => {
  const sections = [{ heading: null, names: [] }];
  for (const item of allOnPage("#pick-area-list li")) {
    if (item.classList.contains("combo-group")) sections.push({ heading: item.textContent, names: [] });
    else if (item.classList.contains("combo-option")) sections.at(-1).names.push(item.querySelector(".combo-label").textContent);
  }
  return sections.map((section) => ({ ...section, names: section.names.filter((name) => name !== "Alle Bereiche") }));
});
check(!areaNames.some((name) => /^Komplex (Informatik|Mathematik)$|^Fachstudium$/.test(name)) && areaNames.includes("Praktische Informatik") && areaNames.includes("Physik"), `area picker: offers ${areaNames.join(" | ")}`);
check(!areaNames.some((name) => /wahlpflicht/i.test(name)) && areaNames.includes("Praktische Mathematik"), `area picker: a name still starts with „Wahlpflichtmodule“ (${areaNames.join(" | ")})`);
check(areaGroups.join(" | ") === "Nebenfach", `area picker: the headings are ${areaGroups.join(" | ")}, not the Nebenfach alone`);
check(areaSections[0].names.join(" | ") === "Proseminar oder Praktikum | Grundlagen der Informatik | Praktische Informatik | Angewandte und Technische Informatik | Seminar oder Praktikum aus der Informatik", `area picker: the own electives are ${areaSections[0].names.join(" | ")}`);
check(areaSections[1]?.names.join(" | ") === "Praktische Mathematik | Mathematik | Physik | Maschinenbau / Elektrotechnik | Wirtschaftswissenschaften | Bauingenieurwesen", `area picker: the Nebenfach holds ${areaSections[1]?.names.join(" | ")}`);
// A heading once: the sections come from the tree, not from the path („Maschinenbau / Elektrotechnik" is one area, not one below „Maschinenbau").
check(new Set(areaGroups).size === areaGroups.length && !areaGroups.some((heading) => /^(Komplex |Grundstudium|Fachstudium|Maschinenbau$)/.test(heading)) && areaNames.includes("Maschinenbau / Elektrotechnik"), `area picker: a heading repeats, splits a name or only structures (${areaGroups.join(" | ")})`);
check(areaSections.slice(1).every((section) => section.names.length >= 2), `area picker: a heading stands over a single area (${areaSections.map((section) => `${section.heading}: ${section.names.length}`).join(", ")})`);
// Typing keeps each section together: a heading does not come twice in the results either.
await page.fill("#pick-area-search >> visible=true", "mat");
await page.waitForFunction(() => allOnPage("#pick-area-list .combo-option").length >= 2);
const searchGroups = await page.evaluate(() => [...allOnPage("#pick-area-list .combo-group:not(.combo-rule)")].map((el) => el.textContent));
check(new Set(searchGroups).size === searchGroups.length, `area picker: while searching a heading comes twice (${searchGroups.join(" | ")})`);
await page.fill("#pick-area-search >> visible=true", "");
const areaName = areaNames.find((name) => name.split(" ").length >= 2);
check(Boolean(areaName), "area picker: no elective area is offered");
// An entry is the name and the number of modules, nothing else: the path of the tree pushed the names out.
const areaDetails = await page.evaluate(() => [...allOnPage("#pick-area-list .combo-option small")].map((el) => el.textContent));
check(areaDetails.length > 3 && areaDetails.every((text) => /^\d+ Modul(e)?$/.test(text)), `area picker: an entry says more than its name and count (${areaDetails.find((text) => !/^\d+ Modul(e)?$/.test(text))})`);
await page.fill("#pick-area-search >> visible=true", areaName.split(" ").slice(0, 2).join(" "));
await page.waitForFunction((name) => onPage("#pick-area-list .combo-option .combo-label")?.textContent === name, areaName);
await step("area: Enter filters the list", () => page.keyboard.press("Enter"), (name) => /[?&]area=\d+/.test(location.search) && [...allOnPage(".tag")].some((tag) => tag.textContent.includes(name)) && !location.search.includes("page="), areaName);
const inArea = await count();
check(number(inArea) > 0 && number(inArea) < number(all), `area: ${inArea} modules in „${areaName}" (of ${all})`);
check(await page.evaluate(() => onPage("#filters").__same === true), "the filter panel was rebuilt by picking an area");
await step("area: the tag takes it out again", () => page.click('.tag:has(em:text("Bereich")) a >> visible=true'), () => !location.search.includes("area="));

// ---- nothing moves at the takeover: the site's catalog has no filter panel (docs/folia/
// folia-refactor.md §4.1), but its place, with the panel's bars, so the list stands where the app's does.
const boxes = (p) => p.evaluate(() => {
  const box = (el) => { const r = el?.getBoundingClientRect(); return r ? `${r.left.toFixed(0)},${r.top.toFixed(0)},${r.width.toFixed(0)}` : "none"; };
  return { panel: box(onPage("#filters, .filters.sk-standin")), list: box(onPage(".panel.list")) };
});
for (const path of ["/catalog", "/catalog?turnus=winter"]) {
  const server = await browser.newContext({ viewport: { width: 1500, height: 900 } });
  await server.route("**/pkg/**", (route) => route.abort());
  const before = await server.newPage();
  await before.goto(base + path, { waitUntil: "load" });
  await before.evaluate(() => document.fonts.ready);
  const was = await boxes(before);
  const standin = await before.locator(".filters.sk-standin").count();
  await server.close();
  const app = await browser.newContext({ viewport: { width: 1500, height: 900 } });
  const after = await app.newPage();
  await after.goto(base + path, { waitUntil: "domcontentloaded" });
  await after.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await after.evaluate(() => document.fonts.ready);
  const is = await boxes(after);
  check(standin === 1, `takeover (${path}): the site's page has no stand-in for the filter panel`);
  check(was.panel === is.panel && was.list === is.list, `takeover (${path}): panel and list stood at ${JSON.stringify(was)} and stand at ${JSON.stringify(is)}`);
  await app.close();
}

// ---- without JavaScript: the site's catalog, a page of the list at a time, no filters
const plain = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 1300, height: 900 } });
const still = await plain.newPage();
await still.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
const total = await still.locator(".rows a.row").count();
check((await still.locator("#filters, .filters select, .filters a.chip").count()) === 0, "no JS: the site's catalog has a filter panel");
check((await still.locator(".filters.sk-standin").evaluate((el) => el.getClientRects().length)) === 0, "no JS: the stand-in of the filter panel shows");
// What needs JavaScript (shortcut hints, drag handles, the slider, the theme switch) is not shown.
await still.goto(base + "/catalog?open=11101", { waitUntil: "domcontentloaded" });
const leftovers = await still.evaluate(() => [...allOnPage("kbd, .resizer, .slider, .scale, .theme-toggle, .keys, .load-more, .fit-chip")].filter((el) => el.getClientRects().length > 0).map((el) => el.tagName + "." + el.className));
check(leftovers.length === 0, `no JS: still visible: ${leftovers.join(", ")}`);
// The preview is the app's: the server's page ignores `open` and every row leads to the module's own page.
check((await still.locator(".detail").count()) === 0, "no JS: the server's page shows a preview for a shared link");
check(/^\/catalog\/module\/[A-Za-z0-9_-]+$/.test(await still.getAttribute(".rows a.row", "href")), `no JS: a row leads to ${await still.getAttribute(".rows a.row", "href")}, not to the module's page`);
await still.click(".rows a.row");
await still.waitForURL(/\/catalog\/module\//);
check((await still.locator(".module-page h2").count()) === 1, "no JS: the row's link did not open the module's page");
// An address with a filter (a link from elsewhere) still lists what it holds.
await still.goto(base + "/catalog?turnus=winter", { waitUntil: "domcontentloaded" });
check((await still.locator(".rows a.row").count()) > 0 && total > 0, "no JS: a filtered address lists nothing");

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
