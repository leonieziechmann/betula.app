// Checks the filter panel of the catalog in the browser app, and its plain version without JavaScript.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node filters.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Walks: toggles (with → without → off, the panel is not rebuilt) → rows of toggles fill the width →
// program picker (typo-tolerant search, arrow keys, Enter, Esc, click outside, clear) → lecturer picker →
// credit slider → width of the panel (limits, localStorage) → group header across a page border →
// the same panel without JavaScript.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
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
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
  }
  timings[name] = Date.now() - started;
  if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
};
const check = (ok, message) => { if (!ok) problems.push(message); };
const param = (name) => page.evaluate((n) => new URL(location.href).searchParams.getAll(n).join("|"), name);
const count = () => page.evaluate(() => document.querySelector(".count")?.textContent);

await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
await page.evaluate(() => { window.__marker = 1; document.getElementById("filters").__same = true; });

// ---- toggles: with → without → off; the panel is the same element throughout, the focus stays
const all = await count();
const talk = '#filters a.chip:has-text("Vortrag")';
await step("toggle: with", () => page.click(talk), () => new URL(location.href).searchParams.get("exam") === "presentation");
check((await page.getAttribute(talk, "data-state")) === "with" && (await page.getAttribute(talk, "aria-checked")) === "true", "toggle: the chip does not show 'with'");
const withTalk = await count();
await step("toggle: without", () => page.click(talk), () => new URL(location.href).searchParams.get("not-exam") === "presentation" && !location.search.includes("&exam=") && !location.search.includes("?exam="));
check((await page.getAttribute(talk, "data-state")) === "without", "toggle: the chip does not show 'without'");
check(await page.evaluate(() => [...document.querySelectorAll(".tag")].some((tag) => tag.textContent.includes("ohne Vortrag"))), "toggle: no tag 'ohne Vortrag' above the list");
const withoutTalk = await count();
check(all !== withTalk && withTalk !== withoutTalk && all !== withoutTalk, `toggle: counts did not change (${all}, ${withTalk}, ${withoutTalk})`);
check(await page.evaluate(() => document.activeElement?.textContent.includes("Vortrag")), "toggle: the chip lost the focus");
await step("toggle: space bar flips it off", () => page.keyboard.press(" "), () => !location.search.includes("exam"));
check((await page.getAttribute(talk, "data-state")) === "off", "toggle: the chip does not show 'off'");
check(await page.evaluate(() => document.getElementById("filters").__same === true), "the filter panel was rebuilt by a filter change");

// ---- every link of the panel follows the whole filter. (The first toggle once kept the link of
// the empty filter, so a click on it dropped everything else: a change of the filter that leaves
// a toggle's own state alone has to reach that toggle's link all the same.)
await step("two filters", async () => { await page.click('#filters a.chip:has-text("Vorlesung")'); await page.click('#filters a.chip:has-text("Klausur")'); }, () => location.search.includes("form=lecture") && location.search.includes("exam=written"));
const forgetful = await page.evaluate(() => [...document.querySelectorAll('#filters a[role="checkbox"], #filters a[role="radio"]')]
  .filter((a) => a.offsetParent && !(a.getAttribute("href").includes("form=lecture") && a.getAttribute("href").includes("exam=written")))
  .map((a) => `${a.textContent.trim()} → ${a.getAttribute("href")}`));
check(forgetful.length === 0, `links that drop the rest of the filter: ${forgetful.slice(0, 4).join(" | ")}`);
await step("the first toggle keeps the others", () => page.click('#filters a.chip:has-text("Winter")'), () => ["turnus=winter", "form=lecture", "exam=written"].every((part) => location.search.includes(part)));
await step("reset", () => page.click('#filters a:has-text("Zurücksetzen")'), () => location.search === "");

// ---- every toggle has its box, and the toggles of a row share the row's whole width
const layout = await page.evaluate(() => {
  const issues = [];
  for (const chips of document.querySelectorAll("#filters .chips")) {
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
  const chips = [...document.querySelectorAll("#filters .chips")].find((c) => c.querySelectorAll(".chip").length > 3);
  const [a, b] = chips.querySelectorAll(".chip");
  const ra = a.getBoundingClientRect(), rb = b.getBoundingClientRect();
  const owner = (x, y) => document.elementFromPoint(x, y)?.closest(".chip");
  return { above: owner(ra.left + 20, ra.top - 2) === a, leftOfGap: owner(ra.right + 1.5, ra.top + 10) === a, rightOfGap: owner(rb.left - 1.5, rb.top + 10) === b };
});
check(beyond.above && beyond.leftOfGap && beyond.rightOfGap, `oversizing: the area around a toggle is not its own (${JSON.stringify(beyond)})`);

// ---- the search field starts exactly where the list starts
const edges = await page.evaluate(() => [document.querySelector(".search").getBoundingClientRect().left, document.querySelector(".panel.list").getBoundingClientRect().left]);
check(Math.abs(edges[0] - edges[1]) < 0.01, `top bar: the search starts at ${edges[0]}, the list at ${edges[1]}`);

// ---- program picker
const pop = "#pick-program-list";
const active = () => page.evaluate(() => document.querySelector(".combo-option.active .combo-label")?.textContent || null);
await step("picker opens", () => page.click("#pick-program"), () => document.activeElement?.id === "pick-program-search" && document.querySelectorAll("#pick-program-list .combo-option").length > 50);
check((await active()) === "Alle Studiengänge", `picker: starts at ${await active()} instead of the selection`);
const doubles = await page.evaluate(() => {
  const seen = new Set(), twice = [];
  for (const option of document.querySelectorAll("#pick-program-list .combo-option")) {
    if (seen.has(option.textContent)) twice.push(option.textContent);
    seen.add(option.textContent);
  }
  return twice;
});
check(doubles.length === 0, `picker: entries that cannot be told apart: ${doubles.slice(0, 3).join(" | ")}`);
// One shape for all: name, short degree, year of the PO; the form of study only where two would read the same.
const shapes = await page.evaluate(() => [...document.querySelectorAll("#pick-program-list .combo-option small")].map((el) => el.textContent).filter((text) => !/^[^·]+ · \d{4}( · [^·]+)?$/.test(text) || /SÄ|NF|PO/.test(text)));
check(shapes.length === 0, `picker: entries of another shape: ${shapes.slice(0, 3).join(" | ")}`);

// A typo, a second word, and the best match on top.
await step("picker: search with a typo", () => page.keyboard.type("infomatik bsc"), () => document.querySelector("#pick-program-list .combo-option .combo-label")?.textContent === "Informatik");
check((await active()) === "Informatik", `picker: the first match is not marked (${await active()})`);
// The arrow keys move the mark; a resting mouse pointer over the list does not take it back.
await page.fill("#pick-program-search", "inf");
await page.waitForFunction(() => document.querySelectorAll("#pick-program-list .combo-option").length > 3);
const box = await page.locator(`${pop} .combo-option >> nth=0`).boundingBox();
await page.mouse.move(box.x + 40, box.y + box.height / 2);
const first = await active();
await page.keyboard.press("ArrowDown");
await page.keyboard.press("ArrowDown");
const third = await active();
const expectedThird = await page.evaluate(() => document.querySelectorAll("#pick-program-list .combo-option .combo-label")[2]?.textContent);
check(third === expectedThird && third !== null, `picker: two steps down mark "${third}", expected "${expectedThird}" (first was "${first}")`);
check((await page.getAttribute("#pick-program-search", "aria-activedescendant")) === "pick-program-option-2", "picker: aria-activedescendant does not follow");
await page.keyboard.press("ArrowUp");
await page.keyboard.press("ArrowUp");
await page.keyboard.press("ArrowUp");
const last = await page.evaluate(() => [...document.querySelectorAll("#pick-program-list .combo-option .combo-label")].pop()?.textContent);
check((await active()) === last, `picker: arrow up from the first entry does not wrap to the last (${await active()} / ${last})`);

await page.fill("#pick-program-search", "informatik b.sc");
await page.waitForFunction(() => document.querySelector("#pick-program-list .combo-option .combo-label")?.textContent === "Informatik");
await step("picker: Enter takes the marked entry", () => page.keyboard.press("Enter"), () => new URL(location.href).searchParams.get("program")?.startsWith("bachelor-informatik") && !document.querySelector(".combo-pop") && document.querySelector(".sem"));
check(await page.evaluate(() => document.activeElement?.id === "pick-program"), "picker: the focus did not return to the button");
check((await page.textContent("#pick-program .combo-value")).startsWith("Informatik"), "picker: the button does not show the selection");
check(await page.evaluate(() => document.getElementById("filters").__same === true), "the filter panel was rebuilt by picking a program");

// Esc closes the picker and nothing else: the preview stays.
await step("open a preview", () => page.click("a.row >> nth=2"), () => location.search.includes("open=") && document.querySelector(".detail h2"));
await step("picker opens by keyboard", async () => { await page.focus("#pick-program"); await page.keyboard.press("ArrowDown"); }, () => document.activeElement?.id === "pick-program-search");
check((await active()) === "Informatik", `picker: reopened at "${await active()}" instead of the selected program`);
await step("picker: Esc closes it", () => page.keyboard.press("Escape"), () => !document.querySelector(".combo-pop") && document.activeElement?.id === "pick-program");
check((await param("open")) !== "", "picker: Esc also closed the module preview");
await page.keyboard.press("Escape");
await page.waitForFunction(() => !location.search.includes("open="));

await step("picker opens again", () => page.click("#pick-program"), () => Boolean(document.querySelector(".combo-pop")));
await step("picker: a click outside closes it", () => page.click(".count-row"), () => !document.querySelector(".combo-pop"));
await step("picker: a click on an entry", async () => { await page.click("#pick-program"); await page.fill("#pick-program-search", "architektur bachelor"); await page.click(`${pop} .combo-option >> nth=0`); }, () => new URL(location.href).searchParams.get("program")?.includes("architektur") && !document.querySelector(".combo-pop"));
await step("picker: clear", () => page.click("#pick-program ~ .combo-clear"), () => !location.search.includes("program="));

// ---- lecturer picker: adds a person as a toggle (with → without → gone)
await page.click('#filters summary:has-text("Weitere Filter")');
await step("lecturer picker opens", () => page.click("#pick-lecturer"), () => document.querySelectorAll("#pick-lecturer-list .combo-option").length > 50);
const person = await page.evaluate(() => document.querySelectorAll("#pick-lecturer-list .combo-option .combo-label")[5]?.textContent || "");
const family = person.split(",")[0];
await page.fill("#pick-lecturer-search", family);
await page.waitForFunction((name) => document.querySelector("#pick-lecturer-list .combo-option .combo-label")?.textContent.startsWith(name), family);
const picked = await page.evaluate(() => document.querySelector("#pick-lecturer-list .combo-option .combo-label").textContent);
await step("lecturer: Enter adds the person", () => page.keyboard.press("Enter"), (name) => new URL(location.href).searchParams.get("lecturer") === name && document.querySelector('.people .person[data-state="with"]'), picked);
const onlyFirst = await count();
// Academic titles: small, in the picker and under the chosen name.
await step("lecturer picker opens again", () => page.click("#pick-lecturer"), () => document.querySelectorAll("#pick-lecturer-list .combo-option").length > 50);
check(await page.evaluate(() => [...document.querySelectorAll("#pick-lecturer-list .combo-option small")].some((el) => el.textContent.startsWith("Prof."))), "lecturer: the picker shows no academic titles");
check(!(await page.evaluate((name) => [...document.querySelectorAll("#pick-lecturer-list .combo-label")].some((el) => el.textContent === name), picked)), "lecturer: a chosen person is offered again");
// A second wanted person is an alternative: the list grows (or stays), it never shrinks.
await page.fill("#pick-lecturer-search", "prof dr");
await page.waitForFunction(() => document.querySelectorAll("#pick-lecturer-list .combo-option").length > 3);
await step("lecturer: a second person, by a click", () => page.click('#pick-lecturer-list .combo-option:has(small:text-matches("^Prof\.")) >> nth=0'), () => new URL(location.href).searchParams.getAll("lecturer").length === 2 && document.querySelectorAll(".people .person").length === 2);
const either = await count();
const number = (text) => Number(String(text).replace(/\D/g, ""));
check(number(either) >= number(onlyFirst) && number(either) > 0, `lecturer: two wanted persons are not alternatives (${onlyFirst} → ${either})`);
check(await page.evaluate(() => [...document.querySelectorAll(".people .person-name small")].some((el) => el.textContent.startsWith("Prof."))), "lecturer: no title under the chosen name");
const second = await page.evaluate(() => document.querySelectorAll(".people .person-name b")[1].textContent);
await step("lecturer: × excludes", () => page.click(".people .person >> nth=1 >> a.cross"), (name) => new URL(location.href).searchParams.get("not-lecturer") === name && document.querySelectorAll('.people .person[data-state="without"]').length === 1, second);
await step("lecturer: + wants again", () => page.click(".people .person >> nth=1 >> a.plus"), () => new URL(location.href).searchParams.getAll("lecturer").length === 2 && !location.search.includes("not-lecturer"));
await step("lecturer: the button on the right removes", async () => { await page.click(".people .person >> nth=1 >> a.remove"); await page.click(".people .person >> nth=0 >> a.remove"); }, () => !location.search.includes("lecturer") && !document.querySelector(".people"));

// ---- credits: drag the lower thumb, step the upper one with the keyboard
await page.evaluate(() => document.querySelector("#filters .slider").scrollIntoView({ block: "center" }));
const track = await page.locator("#filters .slider").boundingBox();
const thumb = track.height; // the grabbed thumb is as high as the slider and larger than the knob it shows
const at = (credits) => track.x + thumb / 2 + ((track.width - thumb) * credits) / 30;
await page.mouse.move(at(0), track.y + track.height / 2);
await page.mouse.down();
await page.mouse.move(at(6), track.y + track.height / 2, { steps: 8 });
await step("slider: letting go filters", () => page.mouse.up(), () => new URL(location.href).searchParams.get("ects_min") === "6");
check((await page.inputValue('#filters input[name="ects_min"]')) === "6", "slider: the number field does not follow");
await page.focus("#filters .slider input >> nth=1");
await step("slider: arrow key on the upper thumb", () => page.keyboard.press("ArrowLeft"), () => new URL(location.href).searchParams.get("ects_max") === "29");
// The upper thumb cannot pass the lower one.
await page.mouse.move(at(29), track.y + track.height / 2);
await page.mouse.down();
await page.mouse.move(at(2), track.y + track.height / 2, { steps: 8 });
await step("slider: thumbs do not cross", () => page.mouse.up(), () => new URL(location.href).searchParams.get("ects_max") === "6");
await step("slider: typed numbers", async () => { await page.fill('#filters input[name="ects_max"]', "12"); await page.keyboard.press("Tab"); }, () => new URL(location.href).searchParams.get("ects_max") === "12");
check((await page.textContent("#filters .fgroup:has(.slider) .flabel span")) === "6–12 LP", "slider: the summary does not say 6–12 LP");

// ---- width of the panel: dragged, limited, remembered outside the URL
const width = () => page.evaluate(() => Math.round(document.getElementById("filters").getBoundingClientRect().width));
const drag = async (dx) => {
  const edge = await page.locator('[data-action="resize-filters"]').boundingBox();
  await page.mouse.move(edge.x + edge.width / 2, edge.y + 200);
  await page.mouse.down();
  await page.mouse.move(edge.x + edge.width / 2 + dx, edge.y + 200, { steps: 6 });
  await page.mouse.up();
};
const startWidth = await width();
// The page follows the handle live (list and search field move with it). Only where a page cannot keep
// up, the rest of the drag moves the panel alone and the page follows when the handle is let go; the
// test forces that with a budget no frame can meet.
{
  const state = () => page.evaluate(() => ({
    panel: Math.round(document.getElementById("filters").getBoundingClientRect().width),
    list: Math.round(document.querySelector(".panel.list").getBoundingClientRect().width),
    search: Math.round(document.querySelector(".search").getBoundingClientRect().left),
    variable: document.documentElement.style.getPropertyValue("--w-filters"),
    mode: document.documentElement.dataset.resizeMode || null,
  }));
  const dragBy = async (dx) => {
    const edge = await page.locator('[data-action="resize-filters"]').boundingBox();
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
  const live = await dragBy(60);
  check(live.during.mode === "live" && Math.abs(live.during.panel - (before.panel + 60)) <= 2 && live.during.list === before.list - (live.during.panel - before.panel) && live.during.search === before.search + (live.during.panel - before.panel),
    `resize: the page does not follow the handle live: ${JSON.stringify(live.during)}, before ${JSON.stringify(before)}`);
  check(live.after.mode === null && live.after.variable === live.after.panel + "px", `resize: after letting go ${JSON.stringify(live.after)}`);
  await page.dblclick('[data-action="resize-filters"]');

  await page.evaluate(() => { document.documentElement.dataset.resizeBudget = "0.001"; });
  const forced = await dragBy(60);
  check(forced.during.mode === "panel" && Math.abs(forced.during.panel - (before.panel + 60)) <= 2 && forced.during.list > before.list - 60, `resize: the fallback did not take over: ${JSON.stringify(forced.during)}`);
  check(forced.after.mode === null && forced.after.variable === forced.after.panel + "px" && forced.after.list === before.list - (forced.after.panel - before.panel) && Math.abs(forced.after.panel - (before.panel + 60)) <= 2,
    `resize: after the fallback the page did not catch up: ${JSON.stringify(forced.after)}`);
  await page.evaluate(() => { delete document.documentElement.dataset.resizeBudget; });
  await page.dblclick('[data-action="resize-filters"]');
}
// The zone that takes the pointer is much wider than the grip, and none of it lies over the panel (its scrollbar).
const zone = await page.evaluate(() => { const z = document.querySelector('[data-action="resize-filters"]').getBoundingClientRect(); return { width: z.width, over: document.getElementById("filters").getBoundingClientRect().right - z.left }; });
check(zone.width >= 16 && zone.over <= 0.5, `resize: the handle's zone is ${zone.width}px wide and ${zone.over}px over the filter panel`);
await drag(60);
check(Math.abs((await width()) - (startWidth + 60)) <= 2, `resize: expected about ${startWidth + 60}px, got ${await width()}px`);
check(Number(await page.evaluate(() => localStorage.getItem("betula.filters.width"))) === (await width()), "resize: the width is not in localStorage");
await drag(600);
check((await width()) === 440, `resize: the upper limit is 440px, got ${await width()}px`);
await drag(-900);
check((await width()) === 232, `resize: the lower limit is 232px, got ${await width()}px`);
check(!(await page.evaluate(() => location.search)).includes("232"), "resize: the width leaked into the URL");
await page.dblclick('[data-action="resize-filters"]');
check((await width()) === startWidth && (await page.evaluate(() => localStorage.getItem("betula.filters.width"))) === null, `resize: a double click does not reset (${await width()}px)`);

// ---- a group header stays while its group runs across the border between two pages
await step("program list", async () => { await page.click('#filters a:has-text("Zurücksetzen")'); await page.click("#pick-program"); await page.fill("#pick-program-search", "informatik b.sc"); await page.keyboard.press("Enter"); }, () => document.querySelector(".sem") && document.querySelectorAll(".rows a.row").length === 50);
await step("scroll into the second page", () => page.evaluate(() => { const rows = document.querySelector(".rows"); rows.scrollTop = rows.scrollHeight; }), () => document.querySelectorAll(".rows a.row").length > 50);
const header = await page.evaluate(async () => {
  const rows = document.querySelector(".rows");
  const second = rows.querySelector('a.row[data-page="2"]');
  rows.scrollTop = second.offsetTop + 200;
  await new Promise((r) => setTimeout(r, 150));
  const top = rows.getBoundingClientRect().top;
  const stuck = [...rows.querySelectorAll(".sem")].filter((h) => Math.abs(h.getBoundingClientRect().top - top) < 3).pop();
  // The group of the first visible row: the last header before it in the document.
  const visible = [...rows.querySelectorAll("a.row")].find((r) => r.getBoundingClientRect().bottom > top + 40);
  let expected = null;
  for (const el of rows.querySelectorAll(".sem, a.row")) {
    if (el === visible) break;
    if (el.classList.contains("sem")) expected = el.textContent;
  }
  return { stuck: stuck?.textContent || null, expected };
});
check(header.stuck !== null && header.stuck === header.expected, `group header on page two: shows "${header.stuck}", expected "${header.expected}"`);

// ---- without JavaScript: the same panel as links and plain fields
const plain = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 1300, height: 900 } });
const still = await plain.newPage();
await still.goto(base + "/catalog?exam=written&not-exam=presentation", { waitUntil: "domcontentloaded" });
check((await still.getAttribute('#filters a.chip:has-text("Vortrag")', "data-state")) === "without", "no JS: the excluded chip is not shown as excluded");
check((await still.locator('#filters select[name="program"] option').count()) > 50, "no JS: no plain program select");
// What needs JavaScript (shortcut hints, drag handles, the slider, the theme switch) is not shown.
await still.goto(base + "/catalog?open=11101", { waitUntil: "domcontentloaded" });
const leftovers = await still.evaluate(() => [...document.querySelectorAll("kbd, .resizer, .slider, .scale, .theme-toggle, .keys, .load-more")].filter((el) => el.getClientRects().length > 0).map((el) => el.tagName + "." + el.className));
check(leftovers.length === 0, `no JS: still visible: ${leftovers.join(", ")}`);
check((await still.locator(".detail h2").count()) === 1, "no JS: the shared link with a preview does not show the module");
await still.goto(base + "/catalog?exam=written&not-exam=presentation", { waitUntil: "domcontentloaded" });
await still.click('#filters a.chip:has-text("Winter")');
await still.waitForURL(/turnus=winter/);
check((await still.getAttribute('#filters a.chip:has-text("Winter")', "data-state")) === "with" && still.url().includes("not-exam=presentation"), "no JS: a toggle link lost the rest of the filter");
await still.selectOption('#filters select[name="program"]', { index: 5 });
await still.waitForTimeout(600); // the view transition of the page load holds clicks back for a moment
await still.click('#filters button[type="submit"]');
await still.waitForURL(/program=/);
check(still.url().includes("turnus=winter") && still.url().includes("not-exam=presentation"), `no JS: the form lost what the links had set (${still.url()})`);

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
