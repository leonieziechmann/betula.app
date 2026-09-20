// Checks „Merken": marking modules and the list of marked modules.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node bookmarks.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Desktop: the button at the end of a row (in place, aligned, no navigation, the list is not
// rendered again), M on a row, in the preview and on the module's page, the same state wherever a
// module shows, the number at the rail, a reload keeps the marks; the list of marked modules
// (order of marking, numbers, sorting, halves of the year, preview, a mark taken away stays on the
// page dimmed, emptying with „Rückgängig"); the rail's item as a tab; „Zurück" from a module that
// was opened from the marked modules; another tab of the same browser; what the snapshot does not
// know; garbage in the storage.
// Privacy: no request ever carries a marked id, and server HTML shows nothing marked.
// Phone: marking by touch, a tap on a marked module opens its page and „Zurück" returns.
// Without the app: nothing of it shows without JavaScript, and with JavaScript the places are
// kept so that nothing moves at the takeover.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
const KEY = "betula.bookmarks.v1";

const watch = (page, requests) => {
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  if (requests) page.on("request", (r) => requests.push(r.url() + " " + (r.postData() || "")));
};
const takeover = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
const open = async (options, path, requests) => {
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
const stored = (page) => page.evaluate((key) => (localStorage.getItem(key) || "").split("\n").filter(Boolean).map((line) => line.split("\t")[0]), KEY);
const badge = (page) => page.evaluate(() => document.querySelector(".rail .nav-count")?.textContent ?? null);
const pressed = (page, selector) => page.evaluate((s) => document.querySelector(s)?.getAttribute("aria-pressed") ?? null, selector);
const listIds = (page) => page.evaluate(() => [...document.querySelectorAll(".rows a.row")].map((row) => row.dataset.id));

// ---------- desktop ----------
{
  const requests = [];
  const { page, step, context } = await open({ viewport: { width: 1500, height: 900 } }, "/catalog", requests);
  check((await badge(page)) === null && (await stored(page)).length === 0, "a fresh browser already has something marked");

  // The button at the end of every row: inside the row, on its middle, clear of the last column,
  // its glyph ending where the content of the list ends (12px from the row's edge, like the head
  // of the list), and the column labels still stand over their columns.
  const geometry = await page.evaluate(() => {
    const rows = [...document.querySelectorAll(".rows .row-wrap")].slice(0, 12);
    const label = document.querySelector(".cols .c-events").getBoundingClientRect();
    const head = document.querySelector(".list-tools").getBoundingClientRect();
    return rows.map((wrap) => {
      const [row, mark, glyph, events] = [wrap.querySelector("a.row"), wrap.querySelector(":scope > .mark-toggle"), wrap.querySelector(":scope > .mark-toggle .icon"), wrap.querySelector(".events")].map((el) => el?.getBoundingClientRect());
      if (!mark) return { missing: true };
      return {
        inside: mark.left >= row.left && mark.right <= row.right && mark.top >= row.top && mark.bottom <= row.bottom,
        centred: Math.abs((mark.top + mark.bottom) / 2 - (row.top + row.bottom) / 2),
        clear: mark.left - events.right,
        edge: row.right - glyph.right,
        head: Math.abs(glyph.right - head.right),
        labelled: Math.abs(events.left - label.left),
        visible: getComputedStyle(wrap.querySelector(":scope > .mark-toggle")).visibility,
      };
    });
  });
  check(geometry.length === 12 && geometry.every((g) => !g.missing), "rows without a mark button");
  check(geometry.every((g) => g.inside && g.centred <= 0.5 && g.clear >= 8 && g.edge === 12 && g.visible === "visible"), `the mark is not in its place at the end of the row: ${JSON.stringify(geometry[0])}`);
  check(geometry.every((g) => g.head <= 4.5), `the marks do not end where the head of the list ends: ${geometry[0]?.head}`);
  check(geometry.every((g) => g.labelled <= 0.5), `the column labels no longer stand over their columns: ${geometry[0]?.labelled}`);

  // A click marks: no navigation, no preview, the row is the same element, the rail counts.
  const ids = (await listIds(page)).slice(0, 12);
  await page.evaluate(() => { document.querySelectorAll(".rows .row-wrap").forEach((wrap, i) => { wrap.__kept = i; }); });
  await step("a click marks the module", () => page.click(`.row-wrap:has(a.row[data-id="${ids[2]}"]) > .mark-toggle`), (id) => document.querySelector(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`).getAttribute("aria-pressed") === "true", ids[2]);
  check(page.url() === base + "/catalog", `marking changed the address: ${page.url()}`);
  check(JSON.stringify(await stored(page)) === JSON.stringify([ids[2]]), `the mark was not stored: ${await stored(page)}`);
  check((await badge(page)) === "1", `the rail does not count the mark: ${await badge(page)}`);
  check(await page.evaluate(() => [...document.querySelectorAll(".rows .row-wrap")].slice(0, 12).every((wrap, i) => wrap.__kept === i)), "marking rendered the list again");
  check(await page.evaluate((id) => getComputedStyle(document.querySelector(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle .icon`)).fill !== "none", ids[2]), "a marked module does not look marked");

  // A marked module keeps its strength while the pointer is on its row; an unmarked one is quiet
  // and comes forward under the pointer.
  const strength = async (id) => {
    await page.hover(`a.row[data-id="${id}"] .t`);
    await page.waitForTimeout(200);
    return page.evaluate((id) => { const mark = document.querySelector(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`); return [getComputedStyle(mark).color, getComputedStyle(document.body).color]; }, id);
  };
  const [markedColor, textColor] = await strength(ids[2]);
  const [quietColor] = await strength(ids[3]);
  check(markedColor === textColor && quietColor !== textColor, `under the pointer a marked module is ${markedColor}, an unmarked one ${quietColor}, the text ${textColor}`);
  await page.mouse.move(5, 5);

  // M marks the row the keyboard is on; again takes the mark away.
  await page.focus(`a.row[data-id="${ids[4]}"]`);
  await step("M marks the focused row", () => page.keyboard.press("m"), (id) => document.querySelector(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`).getAttribute("aria-pressed") === "true", ids[4]);
  await step("M again takes the mark away", () => page.keyboard.press("m"), (id) => document.querySelector(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`).getAttribute("aria-pressed") === "false", ids[4]);
  await page.keyboard.press("m");
  check(JSON.stringify(await stored(page)) === JSON.stringify([ids[4], ids[2]]), `the newest mark is not first: ${await stored(page)}`);
  check(await page.evaluate((id) => document.activeElement === document.querySelector(`a.row[data-id="${id}"]`), ids[4]), "marking by keyboard moved the focus");

  // In the preview: the switch under the heading, with M written on it; the row follows.
  await step("preview", () => page.click(`a.row[data-id="${ids[6]}"]`), () => Boolean(document.querySelector(".detail .mark-switch")));
  check((await pressed(page, ".detail .mark-switch")) === "false", "the preview of an unmarked module shows it marked");
  check(await page.evaluate(() => document.querySelector(".detail .mark-switch kbd")?.textContent === "M"), "the shortcut is not written on the switch");
  // In the line of the credits, at its right end: where the content under the heading ends.
  const placed = await page.evaluate(() => {
    const [credits, mark, body, last] = [".detail .badges .badge.strong", ".detail .mark-switch", ".detail .dbody .section", ".detail .badges .badge:last-of-type"].map((s) => document.querySelector(s).getBoundingClientRect());
    return { sameLine: Math.abs((credits.top + credits.bottom) / 2 - (mark.top + mark.bottom) / 2), edge: Math.abs(mark.right - body.right), afterBadges: mark.left - last.right, height: mark.height };
  });
  check(placed.sameLine <= 0.5 && placed.edge <= 0.5 && placed.afterBadges >= 6 && placed.height >= 32, `the switch is not at the right end of the line of the credits: ${JSON.stringify(placed)}`);
  await page.evaluate(() => document.activeElement?.blur());
  const widthBefore = await page.evaluate(() => document.querySelector(".detail .mark-switch").getBoundingClientRect().width);
  await step("M marks the previewed module", () => page.keyboard.press("m"), () => document.querySelector(".detail .mark-switch").getAttribute("aria-pressed") === "true");
  check((await pressed(page, `.row-wrap:has(a.row[data-id="${ids[6]}"]) > .mark-toggle`)) === "true", "the row does not follow the preview");
  check(await page.evaluate(() => document.querySelector(".detail .mark-switch").textContent.includes("Gemerkt")), "the switch does not say „Gemerkt“");
  check(Math.abs((await page.evaluate(() => document.querySelector(".detail .mark-switch").getBoundingClientRect().width)) - widthBefore) < 0.5, "the switch changes its width when pressed");
  check((await badge(page)) === "3", `three marks, the rail says ${await badge(page)}`);
  // The focused row wins over the preview: M acts where the keyboard is.
  await page.focus(`a.row[data-id="${ids[8]}"]`);
  await page.keyboard.press("m");
  check((await pressed(page, `.row-wrap:has(a.row[data-id="${ids[8]}"]) > .mark-toggle`)) === "true" && (await pressed(page, ".detail .mark-switch")) === "true", "M did not act on the focused row");
  await page.keyboard.press("m");

  // On the module's page: under the heading and among the actions of the sidebar, one state.
  await page.evaluate(() => document.activeElement?.blur());
  await step("full page", () => page.keyboard.press("f"), () => location.pathname.startsWith("/catalog/module/") && Boolean(document.querySelector(".module-page .mark-switch")));
  check((await pressed(page, ".module-page .mark-switch")) === "true" && (await pressed(page, ".sidebar .action.mark-toggle")) === "true", "the module's page does not know the mark");
  const onPage = await page.evaluate(() => { const [credits, mark, body] = [".module-page .badges .badge.strong", ".module-page .mark-switch", ".module-grid > aside .section"].map((s) => document.querySelector(s).getBoundingClientRect()); return [Math.abs((credits.top + credits.bottom) / 2 - (mark.top + mark.bottom) / 2), Math.abs(mark.right - body.right)]; });
  check(onPage[0] <= 0.5 && onPage[1] <= 0.5, `on the module's page the switch is not at the right end of the line of the credits: ${onPage}`);
  await step("the sidebar's action takes the mark away", () => page.click(".sidebar .action.mark-toggle"), () => document.querySelector(".module-page .mark-switch").getAttribute("aria-pressed") === "false" && document.querySelector(".sidebar .action.mark-toggle").textContent.includes("Merken"));
  await step("and M gives it back", () => page.keyboard.press("m"), () => document.querySelector(".sidebar .action.mark-toggle").getAttribute("aria-pressed") === "true");

  // A reload keeps the marks; server HTML knows nothing of them.
  const html = await (await context.request.get(page.url())).text();
  check(!html.includes('aria-pressed="true"') && !html.includes("nav-count"), "server HTML shows something marked");
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(() => document.querySelector(".module-page .mark-switch")?.getAttribute("aria-pressed") === "true", null, { timeout: 8000 }).catch(() => problems.push("a reload lost the mark"));
  await page.evaluate(() => { window.__marker = 1; });
  const marked = await stored(page);
  check(JSON.stringify(marked) === JSON.stringify([ids[6], ids[4], ids[2]]), `stored after all of this: ${marked}`);

  // ----- the list of marked modules -----
  await step("the rail leads to the marked modules", () => page.click('.rail .nav[data-area="bookmarks"]'), () => location.pathname === "/bookmarks" && document.querySelectorAll(".rows a.row").length === 3);
  check(JSON.stringify(await listIds(page)) === JSON.stringify(marked), `the order of marking, newest first: ${await listIds(page)}`);
  const numbers = await page.evaluate(() => ({
    count: document.querySelector(".list .count").textContent,
    label: document.querySelector(".list .count-label").textContent,
    side: [...document.querySelectorAll(".side-facts dd")].map((dd) => dd.textContent),
    credits: [...document.querySelectorAll(".rows a.row .lp")].map((lp) => parseFloat(lp.textContent.replace(",", "."))).filter((n) => !Number.isNaN(n)).reduce((a, b) => a + b, 0),
    current: document.querySelector('.rail .nav[data-area="bookmarks"]').getAttribute("aria-current"),
    title: document.title,
    frame: ["sidebar"].map((id) => { const r = document.getElementById(id).getBoundingClientRect(); return [r.left, r.top]; })[0],
  }));
  check(numbers.count === "3" && numbers.label.startsWith("gemerkte Module") && numbers.side[0] === "3 Module", `the numbers of the list: ${JSON.stringify(numbers)}`);
  check(numbers.label.includes(`${String(numbers.credits).replace(".", ",")} LP`) && numbers.side[1].startsWith(`${String(numbers.credits).replace(".", ",")} LP`), `the credits do not add up: ${JSON.stringify(numbers)}`);
  check(numbers.current === "page" && numbers.title.startsWith("Merkliste"), `rail or title: ${JSON.stringify(numbers)}`);

  // Sorting: the column links and the sidebar are one state, in the URL.
  await step("sort by credits", () => page.click(".cols a.c-lp"), () => location.search.includes("sort=ects") && !location.search.includes("desc"));
  const credits = () => page.evaluate(() => [...document.querySelectorAll(".rows a.row .lp")].map((lp) => parseFloat(lp.textContent.replace(",", ".")) || 0));
  let sorted = await credits();
  check(sorted.every((n, i) => i === 0 || sorted[i - 1] <= n), `not sorted by credits: ${sorted}`);
  check(await page.evaluate(() => document.querySelector('.toc a[aria-current="page"]')?.textContent.trim() === "Leistungspunkte"), "the sidebar does not show the order");
  await step("the other way round", () => page.click(".cols a.c-lp"), () => location.search.includes("desc=1"));
  sorted = await credits();
  check(sorted.every((n, i) => i === 0 || sorted[i - 1] >= n), `not sorted downwards: ${sorted}`);
  await step("back to the order of marking", () => page.click(".toc a:first-of-type"), () => location.search === "");
  check(JSON.stringify(await listIds(page)) === JSON.stringify(marked), "the order of marking did not come back");

  // Halves of the year: the numbers on the switch are the numbers of the lists.
  const seasons = await page.evaluate(() => [...document.querySelectorAll('.sidebar .seg a')].map((a) => [a.textContent.replace(/\d+/g, "").trim(), Number(a.querySelector(".num").textContent), a.getAttribute("aria-checked")]));
  check(JSON.stringify(seasons.map((s) => s[0])) === JSON.stringify(["Alle", "Winter", "Sommer"]) && seasons[0][1] === 3 && seasons[0][2] === "true", `the halves of the year: ${JSON.stringify(seasons)}`);
  for (const [index, name, code] of [[1, "Winter", "winter"], [2, "Sommer", "summer"]]) {
    await step(`only ${name}`, () => page.click(`.sidebar .seg a:nth-child(${index + 1})`), (code) => location.search.includes(`turnus=${code}`), code);
    const shown = await page.evaluate(() => ({ rows: document.querySelectorAll(".rows a.row").length, count: Number(document.querySelector(".list .count").textContent), tag: document.querySelector(".active-filters .tag")?.textContent ?? "", empty: Boolean(document.querySelector(".rows .state")) }));
    check(shown.rows === seasons[index][1] && shown.count === seasons[index][1] && shown.tag.includes(name), `${name}: ${JSON.stringify(shown)} instead of ${seasons[index][1]}`);
    check(shown.rows > 0 || shown.empty, `${name}: an empty list says nothing`);
  }
  await step("the tag takes the filter away", () => page.click(".active-filters .tag a"), () => location.search === "" && document.querySelectorAll(".rows a.row").length === 3);

  // The preview, as in the catalog; the list stays.
  await step("preview of a marked module", () => page.click(`a.row[data-id="${marked[1]}"]`), (id) => location.search === `?open=${id}` && Boolean(document.querySelector(".detail h2")), marked[1]);
  check((await pressed(page, ".detail .mark-switch")) === "true", "the preview of a marked module does not show it marked");
  // (Measured once the panel has arrived: it slides in.)
  await page.waitForFunction(() => document.querySelector(".work > .detail").getAnimations().every((animation) => animation.playState === "finished"), null, { timeout: 4000 }).catch(() => {});
  const floats = await page.evaluate(() => { const [list, detail] = [document.querySelector(".panel.list"), document.querySelector(".work > .detail")].map((el) => el.getBoundingClientRect()); return [detail.right - list.right, detail.top - list.top, detail.bottom - list.bottom, list.left - detail.left]; });
  check(floats.slice(0, 3).every((d) => Math.abs(d) <= 1) && floats[3] < -100, `the preview does not float at the right edge of the list: ${floats}`);

  // A mark taken away here stays on the page, dimmed; the numbers follow; one click undoes it.
  await page.evaluate(() => { document.querySelectorAll(".rows .row-wrap").forEach((wrap, i) => { wrap.__kept = i; }); });
  await step("a mark taken away stays on the page", () => page.click(".detail .mark-switch"), (id) => document.querySelector(`.row-wrap:has(a.row[data-id="${id}"])`).classList.contains("unmarked"), marked[1]);
  const after = await page.evaluate(() => ({ rows: document.querySelectorAll(".rows a.row").length, count: document.querySelector(".list .count").textContent, side: document.querySelector(".side-facts dd").textContent, kept: [...document.querySelectorAll(".rows .row-wrap")].every((wrap, i) => wrap.__kept === i), all: document.querySelector(".sidebar .seg a .num").textContent }));
  check(after.rows === 3 && after.count === "2" && after.side === "2 Module" && after.all === "2" && after.kept && (await badge(page)) === "2", `after taking a mark away: ${JSON.stringify(after)}`);
  // (The preview floats over the right end of the rows, so their buttons are reached once it is closed.)
  await step("Esc closes the preview", () => page.keyboard.press("Escape"), () => location.search === "" && !document.querySelector(".detail"));
  check(await page.evaluate((id) => document.querySelector(`.row-wrap:has(a.row[data-id="${id}"])`).classList.contains("unmarked"), marked[1]), "closing the preview took the unmarked module off the page");
  await step("one click gives it back", () => page.click(`.row-wrap:has(a.row[data-id="${marked[1]}"]) > .mark-toggle`), () => document.querySelectorAll(".rows .row-wrap.unmarked").length === 0 && document.querySelector(".list .count").textContent === "3");

  // „Liste kopieren" hands the marked modules to the clipboard as text.
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await step("copy the list", () => page.click('[data-action="copy-text"]'), () => document.querySelector('[data-action="copy-text"] span').textContent === "Kopiert");
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  check(copied.split("\n").length === 3 && marked.every((id) => copied.includes(id)) && /\tLP|LP$/m.test(copied.replace(/\d+(,\d+)? /g, "")), `copied: ${JSON.stringify(copied)}`);

  // A list travels to another device in the fragment of a link, which no browser sends anywhere:
  // the other device asks before it adds anything, and the answer takes the ids out of the address.
  const carried = await stored(page);
  await step("copy the link for another device", () => page.click('[data-action="copy-text"][data-absolute]'), () => document.querySelector('[data-action="copy-text"][data-absolute] [data-label]').textContent === "Kopiert");
  const link = await page.evaluate(() => navigator.clipboard.readText());
  check(link === `${base}/bookmarks#add=${carried.join(",")}`, `the link for another device: ${link}`);
  check(await page.evaluate(() => Boolean(document.querySelector('[data-action="copy-text"][data-absolute] small')?.textContent)), "copying the link lost the second line of its action");
  {
    const elsewhere = await browser.newContext({ viewport: { width: 1500, height: 900 } });
    const device = await elsewhere.newPage();
    const sent = [];
    watch(device, sent);
    await device.goto(link, { waitUntil: "domcontentloaded" });
    await takeover(device);
    await device.waitForSelector(".offer", { timeout: 8000 }).catch(() => problems.push("another device: the list from the link is not offered"));
    const offer = await device.evaluate(() => ({ text: document.querySelector(".offer")?.textContent ?? "", rows: document.querySelectorAll(".rows a.row").length, left: Math.abs(document.querySelector(".offer").getBoundingClientRect().left - document.querySelector(".list .count").getBoundingClientRect().left) }));
    check(offer.text.includes("3 Module aus einem Link") && offer.rows === 0 && (await stored(device)).length === 0, `another device: nothing may be added before the visitor says so: ${JSON.stringify(offer)}`);
    check(offer.left <= 0.5, `another device: the offer does not start where the head of the list starts: ${offer.left}`);
    const entries = await device.evaluate(() => history.length);
    await device.click("#offer-add");
    await device.waitForFunction(() => document.querySelectorAll(".rows a.row").length === 3 && location.hash === "" && !document.querySelector(".offer"), null, { timeout: 8000 }).catch(() => problems.push("another device: the list did not arrive, or the ids stayed in the address"));
    check(JSON.stringify(await stored(device)) === JSON.stringify(carried) && JSON.stringify(await listIds(device)) === JSON.stringify(carried), `another device: what arrived: ${await stored(device)}`);
    check((await device.evaluate(() => history.length)) === entries, "another device: answering left a history entry with the ids behind");
    // The same link again: nothing new. And one module more than the list has.
    await device.goto(link, { waitUntil: "domcontentloaded" });
    await takeover(device);
    await device.waitForFunction(() => document.querySelector(".offer")?.textContent.includes("schon auf deiner Merkliste") && !document.getElementById("offer-add"), null, { timeout: 8000 }).catch(() => problems.push("another device: the same link again offers to add what is there"));
    await device.click("#offer-dismiss");
    await device.waitForFunction(() => location.hash === "" && !document.querySelector(".offer"), null, { timeout: 8000 }).catch(() => problems.push("another device: „In Ordnung“ did not take the ids out of the address"));
    await device.goto(`${base}/bookmarks#add=${carried[0]},11101,<script>`, { waitUntil: "domcontentloaded" });
    await takeover(device);
    await device.waitForFunction(() => document.querySelector(".offer")?.textContent.includes("2 Module aus einem Link") && document.querySelector(".offer").textContent.includes("Eins davon"), null, { timeout: 8000 }).catch(() => problems.push("another device: a link with one new module is not counted right"));
    await device.click("#offer-dismiss");
    check((await stored(device)).length === 3, "another device: „Verwerfen“ added something");
    const told = sent.filter((r) => r.includes("add=") || carried.some((id) => r.includes(id)));
    check(told.length === 0, `another device: requests that carry the list: ${told.slice(0, 3)}`);
    await elsewhere.close();
  }

  // Emptying asks first and can be taken back.
  const beforeEmptying = await stored(page);
  await step("emptying asks first", () => page.click(".sidebar button.action:not(.mark-toggle)"), () => document.activeElement?.id === "clear-yes");
  check((await stored(page)).length === 3, "asking already emptied the list");
  await step("emptied", () => page.keyboard.press("Enter"), () => document.querySelector(".list .count").textContent === "0" && document.activeElement?.id === "clear-undo");
  check((await badge(page)) === null && (await page.evaluate((key) => localStorage.getItem(key), KEY)) === null && (await page.evaluate(() => document.querySelectorAll(".rows .row-wrap.unmarked").length)) === 3, "an emptied list leaves something behind, or takes the rows away");
  await step("„Rückgängig“ brings the marks back", () => page.click("#clear-undo"), () => document.querySelector(".list .count").textContent === "3" && document.querySelectorAll(".rows .row-wrap.unmarked").length === 0);
  check(beforeEmptying.length === 3 && JSON.stringify(await stored(page)) === JSON.stringify(beforeEmptying), `what came back: ${await stored(page)} instead of ${beforeEmptying}`);

  // The rail's item is a tab: it remembers how the list was left.
  await step("sorted, then away", () => page.click(".cols a.c-events"), () => location.search === "?sort=events");
  await step("to the catalog", () => page.click('.rail .nav[data-area="catalog"]'), () => location.pathname.startsWith("/catalog"));
  await step("the tab leads back to the list as it was left", () => page.click('.rail .nav[data-area="bookmarks"]'), () => location.pathname === "/bookmarks" && location.search === "?sort=events");

  // A module opened from the marked modules leads back to them.
  await step("preview, then full page", async () => { await page.click(`a.row[data-id="${marked[0]}"]`); await page.waitForSelector(".detail h2"); await page.evaluate(() => document.activeElement?.blur()); await page.keyboard.press("f"); }, (id) => location.pathname === `/catalog/module/${id}`, marked[0]);
  check(await page.evaluate(() => document.querySelector('[data-action="back"]').getAttribute("href").startsWith("/bookmarks")), "„Zurück“ of a module opened from the marked modules leads elsewhere");
  await step("Esc leads back to the marked modules", () => page.keyboard.press("Escape"), () => location.pathname === "/bookmarks" && location.search.includes("open=") && document.querySelectorAll(".rows a.row").length === 3);
  await page.keyboard.press("Escape");

  // Another tab of the same browser marks a module: this one follows.
  const other = await context.newPage();
  watch(other, requests);
  await other.goto(base + "/catalog?turnus=summer", { waitUntil: "domcontentloaded" });
  await takeover(other);
  const fresh = await other.evaluate((known) => [...document.querySelectorAll(".rows a.row")].map((row) => row.dataset.id).find((id) => !known.includes(id)), marked);
  await other.click(`.row-wrap:has(a.row[data-id="${fresh}"]) > .mark-toggle`);
  await page.waitForFunction((id) => document.querySelector(".rows a.row")?.dataset.id === id && document.querySelector(".list .count").textContent === "4", fresh, { timeout: 8000 }).catch(() => problems.push("a mark from another tab did not arrive"));
  check((await badge(page)) === "4", "the rail did not follow the other tab");
  await other.close();

  // What the snapshot does not know is named, and garbage in the storage is ignored.
  await page.evaluate(([key, id]) => localStorage.setItem(key, `99999999\t5\n<script>alert(1)</script>\t4\n../../etc\t3\n${id}\tyesterday\n${id}\t1\n`), [KEY, marked[0]]);
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(() => document.querySelectorAll(".rows .row-wrap").length === 2, null, { timeout: 8000 }).catch(() => problems.push("garbage in the storage: the list did not come up with two entries"));
  const unknown = await page.evaluate(() => ({ group: document.querySelector(".rows .sem")?.textContent, missing: document.querySelector(".rows .row.missing")?.textContent ?? "", count: document.querySelector(".list .count").textContent, markup: document.querySelector(".rows").innerHTML.includes("<script>") }));
  check(unknown.group === "Nicht im Modulkatalog" && unknown.missing.includes("99999999") && unknown.count === "2" && !unknown.markup, `what the snapshot does not know: ${JSON.stringify(unknown)}`);
  await page.click(".row-wrap:has(.row.missing) > .mark-toggle");
  check(JSON.stringify(await stored(page)) === JSON.stringify([marked[0]]), `the mark of an unknown module cannot be taken away: ${await stored(page)}`);

  // Privacy: whatever was marked, no request named it (the ids are five digits or more, the
  // pages of modules that were opened on purpose aside).
  const leaked = requests.filter((r) => r.includes("bookmark") && !r.includes("/bookmarks") || /[?&](ids|marks|marked)=/.test(r));
  check(leaked.length === 0, `requests that carry marks: ${leaked.slice(0, 3)}`);
  const beyond = requests.filter((r) => !r.startsWith(base));
  check(beyond.length === 0, `requests to somewhere else: ${beyond.slice(0, 3)}`);
  await context.close();
}

// ---------- phone ----------
{
  const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };
  const { page, step, context } = await open(phone, "/catalog");
  const ids = (await listIds(page)).slice(0, 6);
  const target = await page.evaluate((id) => { const wrap = document.querySelector(`.row-wrap:has(a.row[data-id="${id}"])`); const [row, mark, lp] = [wrap.querySelector("a.row"), wrap.querySelector(":scope > .mark-toggle"), wrap.querySelector(".lp")].map((el) => el.getBoundingClientRect()); return { size: [mark.width, mark.height], inside: mark.right <= row.right && mark.left >= lp.right, centred: Math.abs((mark.top + mark.bottom) / 2 - (row.top + row.bottom) / 2) }; }, ids[1]);
  check(target.size[0] >= 44 && target.size[1] >= 44 && target.inside && target.centred <= 0.5, `phone: the mark is no target for a finger: ${JSON.stringify(target)}`);
  await step("phone: a tap marks", () => page.tap(`.row-wrap:has(a.row[data-id="${ids[1]}"]) > .mark-toggle`), (id) => document.querySelector(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`).getAttribute("aria-pressed") === "true", ids[1]);
  check(page.url() === base + "/catalog", `phone: marking opened something: ${page.url()}`);
  await page.tap(`.row-wrap:has(a.row[data-id="${ids[3]}"]) > .mark-toggle`);
  check(await page.evaluate(() => document.querySelector(".bottomnav .nav-count")?.textContent === "2"), "phone: the bottom bar does not count");

  await step("phone: the bottom bar leads to the marked modules", () => page.tap('.bottomnav .nav[data-area="bookmarks"]'), () => location.pathname === "/bookmarks" && document.querySelectorAll(".rows a.row").length === 2);
  const layout = await page.evaluate(() => { const [list, side] = [document.querySelector(".panel.list"), document.getElementById("sidebar")].map((el) => el.getBoundingClientRect()); return { below: side.top >= list.bottom - 1, overflow: document.documentElement.scrollWidth > innerWidth }; });
  check(layout.below && !layout.overflow, `phone: the sidebar is not under the list, or the page is wider than the screen: ${JSON.stringify(layout)}`);
  await step("phone: a tap opens the module's page", () => page.tap(`a.row[data-id="${ids[1]}"]`), (id) => location.pathname === `/catalog/module/${id}` && Boolean(document.querySelector(".module-page .mark-switch")), ids[1]);
  const button = await page.evaluate(() => { const r = document.querySelector(".module-page .mark-switch").getBoundingClientRect(); return { height: r.height, pressed: document.querySelector(".module-page .mark-switch").getAttribute("aria-pressed"), seen: r.top < innerHeight }; });
  check(button.height >= 44 && button.pressed === "true" && button.seen, `phone: the switch on the module's page: ${JSON.stringify(button)}`);
  await step("phone: „Zurück“ returns to the marked modules", () => page.click('[data-action="back"]'), () => location.pathname === "/bookmarks" && document.querySelectorAll(".rows a.row").length === 2);
  await context.close();
}

// ---------- without the app ----------
{
  // No JavaScript: nothing of it shows, and the page of the list explains itself.
  const context = await browser.newContext({ viewport: { width: 1500, height: 900 }, javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  const plain = await page.evaluate(() => ({ marks: [...document.querySelectorAll(".mark-toggle")].filter((el) => el.getClientRects().length).length, rail: document.querySelector('.rail .nav[data-area="bookmarks"]')?.getClientRects().length ?? 0 }));
  check(plain.marks === 0 && plain.rail === 0, `without JavaScript something of „Merken“ shows: ${JSON.stringify(plain)}`);
  // (The logo's class is `mark`: what hides the buttons must not hide it.)
  check(await page.evaluate(() => { const logo = document.querySelector(".rail .logo svg"); return logo.getClientRects().length === 1 && getComputedStyle(logo).visibility === "visible"; }), "without JavaScript the logo is gone");
  const response = await page.goto(base + "/bookmarks", { waitUntil: "domcontentloaded" });
  check(response.status() === 200 && (await page.evaluate(() => document.querySelector(".rows .state")?.textContent.includes("JavaScript"))), "without JavaScript the page of the list does not explain itself");
  check((await page.evaluate(() => document.querySelector('meta[name="robots"]')?.content)) === "noindex", "the page of the list is offered to search engines");
  await page.goto(base + "/catalog/module/11101", { waitUntil: "domcontentloaded" });
  check((await page.evaluate(() => [...document.querySelectorAll(".mark-switch, .action.mark-toggle")].filter((el) => el.getClientRects().length).length)) === 0, "without JavaScript the module's page shows a dead switch");
  await context.close();

  // JavaScript, but the app never starts: the places are kept, the buttons are not shown, and
  // the heading is as tall as it is once the app runs (nothing moves at the takeover).
  const blocked = await browser.newContext({ viewport: { width: 1500, height: 900 } });
  await blocked.route("**/pkg/**", (route) => route.abort());
  const waiting = await blocked.newPage();
  await waiting.goto(base + "/catalog/module/11101", { waitUntil: "load" });
  const before = await waiting.evaluate(() => ({ hero: document.querySelector(".module-page > .hero").getBoundingClientRect().height, shown: [...document.querySelectorAll(".mark-toggle")].filter((el) => getComputedStyle(el).visibility !== "hidden").length, marks: document.querySelectorAll(".mark-toggle").length }));
  await blocked.close();
  const { page: running, context: runs } = await open({ viewport: { width: 1500, height: 900 } }, "/catalog/module/11101");
  const hero = await running.evaluate(() => document.querySelector(".module-page > .hero").getBoundingClientRect().height);
  check(before.marks === 2 && before.shown === 0 && Math.abs(before.hero - hero) < 0.5, `before the takeover: ${JSON.stringify(before)}, the heading is ${hero}px once the app runs`);
  await runs.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
