// Checks „Merken": marking modules and the list of marked modules.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node bookmarks.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Desktop: the button at the end of a row (in place, aligned, no navigation, the list is not
// rendered again), M on a row, in the preview and on the module's page, the same state wherever a
// module shows, the number at the rail, a reload keeps the marks; the list of marked modules
// (order of marking in sections by its time, numbers, the order at the heads of the columns with
// „Gemerkt" over the bookmarks, halves of the year, the search at the head searching the list and
// never the address, with its line into the whole catalog, preview, a mark taken away stays on the
// page dimmed, the menu of the whole list: copying it, the link for another device, emptying with
// „Rückgängig" in the note at the bottom); collections („Sammlungen", step 2): made in the sidebar,
// a view of the list with an address of its own, a name there never twice, a module put into one
// from its switch's arrow with a note in Markdown, at its row and under its switches, deleted and
// put back; the note after marking with its field; „Gemerkt" as a filter of the catalog; the rail's item as a tab; „Vollbild" of a
// module opened from the marked modules in place, and „Zurück" from it; another tab of the same
// browser; what the snapshot does not know; garbage in the storage.
// Privacy: no request ever carries a marked id, what is typed into the Merkliste's search, a
// collection's name or a note, and server HTML shows nothing marked.
// Phone: marking by touch, the filters and the orders in a sheet behind „Filter" that is a draft
// until „N Module anzeigen", a tap on a marked module opens it in the list's place and „Zurück" returns.
// Without the app: nothing of it shows without JavaScript, and with JavaScript the places are
// kept so that nothing moves at the takeover.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";
import { fileURLToPath } from "node:url";

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
// What a step looks for is on the page in front (on-page.js; the steps' selectors: what is seen).
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const open = async (options, path, requests) => {
  const context = await browser.newContext(options);
  await context.addInitScript(ON_PAGE);
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
      // The page before stays as a picture in front of the new one while its answers come (pending.rs, `hold`).
      await page.waitForFunction(() => !document.querySelector(".pending-page.held"), null, { timeout: 2000 });
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    timings[name] = Date.now() - started;
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
  };
  return { page, step, context };
};
const stored = (page) => page.evaluate((key) => (localStorage.getItem(key) || "").split("\n").filter(Boolean).map((line) => line.split("\t")[0]), KEY);
const badge = (page) => page.evaluate(() => onPage('.rail .nav[data-area="bookmarks"] .nav-count')?.textContent ?? null);
const pressed = (page, selector) => page.evaluate((s) => onPage(s)?.getAttribute("aria-pressed") ?? null, selector);
const listIds = (page) => page.evaluate(() => [...allOnPage(".rows a.row")].map((row) => row.dataset.id));
// Where the switch „Merken" of a module's heading stands: where the content under the heading
// ends, after „Einplanen" — at the right end of the line of the credits, or, where that line has no
// room for the two, under the badges on a line of their own (owner, 2026-10-20; module.mjs checks
// the three ways).
const placement = (page, root, body) => page.evaluate(([root, body]) => {
  const rect = (selector) => onPage(selector)?.getBoundingClientRect();
  // The switch with its arrow, once the module is marked (the pair).
  const [credits, mark, edge] = [`${root} .badges .badge.strong`, `${root} .mark-pair`, body].map(rect);
  const plan = rect(`${root} .plan-pair`) ?? mark;
  const middle = (r) => (r.top + r.bottom) / 2;
  const badges = [...allOnPage(`${root} .badges > .badge`)].map((badge) => badge.getBoundingClientRect());
  return {
    beside: Math.abs(middle(credits) - middle(plan)) <= 0.5 && Math.min(plan.left, mark.left) - Math.max(...badges.map((badge) => badge.right)) >= 6,
    below: Math.min(plan.top, mark.top) >= Math.max(...badges.map((badge) => badge.bottom)),
    edge: Math.abs(mark.right - edge.right),
    height: mark.height,
  };
}, [root, body]);

// ---------- desktop ----------
{
  const requests = [];
  const { page, step, context } = await open({ viewport: { width: 1500, height: 900 } }, "/catalog", requests);
  check((await badge(page)) === null && (await stored(page)).length === 0, "a fresh browser already has something marked");

  // The button at the end of every row: inside the row, on its middle, clear of the last column,
  // its glyph ending where the content of the list ends (12px from the row's edge, like the head
  // of the list), and the column labels still stand over their columns.
  const geometry = await page.evaluate(() => {
    const rows = [...allOnPage(".rows .row-wrap")].slice(0, 12);
    const label = onPage(".cols .c-events").getBoundingClientRect();
    const head = onPage(".list-tools").getBoundingClientRect();
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
  await page.evaluate(() => { allOnPage(".rows .row-wrap").forEach((wrap, i) => { wrap.__kept = i; }); });
  await step("a click marks the module", () => page.click(`.row-wrap:has(a.row[data-id="${ids[2]}"]) > .mark-toggle >> visible=true`), (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`).getAttribute("aria-pressed") === "true", ids[2]);
  check(page.url() === base + "/catalog", `marking changed the address: ${page.url()}`);
  check(JSON.stringify(await stored(page)) === JSON.stringify([ids[2]]), `the mark was not stored: ${await stored(page)}`);
  check((await badge(page)) === "1", `the rail does not count the mark: ${await badge(page)}`);
  check(await page.evaluate(() => [...allOnPage(".rows .row-wrap")].slice(0, 12).every((wrap, i) => wrap.__kept === i)), "marking rendered the list again");
  check(await page.evaluate((id) => getComputedStyle(onPage(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle .icon`)).fill !== "none", ids[2]), "a marked module does not look marked");

  // A marked module keeps its strength while the pointer is on its row; an unmarked one is quiet
  // and comes forward under the pointer.
  const strength = async (id) => {
    await page.hover(`a.row[data-id="${id}"] .t >> visible=true`);
    await page.waitForTimeout(200);
    return page.evaluate((id) => { const mark = onPage(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`); return [getComputedStyle(mark).color, getComputedStyle(document.body).color]; }, id);
  };
  const [markedColor, textColor] = await strength(ids[2]);
  const [quietColor] = await strength(ids[3]);
  check(markedColor === textColor && quietColor !== textColor, `under the pointer a marked module is ${markedColor}, an unmarked one ${quietColor}, the text ${textColor}`);
  await page.mouse.move(5, 5);

  // M marks the row the keyboard is on; again takes the mark away.
  await page.focus(`a.row[data-id="${ids[4]}"] >> visible=true`);
  await step("M marks the focused row", () => page.keyboard.press("m"), (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`).getAttribute("aria-pressed") === "true", ids[4]);
  await step("M again takes the mark away", () => page.keyboard.press("m"), (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`).getAttribute("aria-pressed") === "false", ids[4]);
  await page.keyboard.press("m");
  check(JSON.stringify(await stored(page)) === JSON.stringify([ids[4], ids[2]]), `the newest mark is not first: ${await stored(page)}`);
  check(await page.evaluate((id) => document.activeElement === onPage(`a.row[data-id="${id}"]`), ids[4]), "marking by keyboard moved the focus");
  // The note after marking: where the module went, as a field that opens the menu („Sammlung"
  // while there is none).
  check(await page.evaluate(() => document.querySelector(".wait-note .mark-toast-field")?.textContent.includes("Sammlung")), "the note after marking has no field for the collection");

  // In the preview: the switch under the heading, with M written on it; the row follows.
  await step("preview", () => page.click(`a.row[data-id="${ids[6]}"] >> visible=true`), () => Boolean(onPage(".detail .mark-toggle.mark-switch")));
  check((await pressed(page, ".detail .mark-toggle.mark-switch")) === "false", "the preview of an unmarked module shows it marked");
  check(await page.evaluate(() => onPage(".detail .mark-toggle.mark-switch kbd")?.textContent === "M"), "the shortcut is not written on the switch");
  // At the right end of the line of the credits, or under the badges where it has no room.
  const placed = await placement(page, ".detail", ".detail .dbody .section");
  check((placed.beside || placed.below) && placed.edge <= 0.5 && placed.height >= 32, `the switch is not at the right end of the line of the credits nor under it: ${JSON.stringify(placed)}`);
  await page.evaluate(() => document.activeElement?.blur());
  // The label keeps its width (the switch itself makes room for its arrow where it has half the line).
  const widthBefore = await page.evaluate(() => onPage(".detail .mark-toggle.mark-switch > span").getBoundingClientRect().width);
  await step("M marks the previewed module", () => page.keyboard.press("m"), () => onPage(".detail .mark-toggle.mark-switch").getAttribute("aria-pressed") === "true");
  check((await pressed(page, `.row-wrap:has(a.row[data-id="${ids[6]}"]) > .mark-toggle`)) === "true", "the row does not follow the preview");
  check(await page.evaluate(() => onPage(".detail .mark-toggle.mark-switch").textContent.includes("Gemerkt")), "the switch does not say „Gemerkt“");
  check(Math.abs((await page.evaluate(() => onPage(".detail .mark-toggle.mark-switch > span").getBoundingClientRect().width)) - widthBefore) < 0.5, "the switch's label changes its width when pressed");
  check((await badge(page)) === "3", `three marks, the rail says ${await badge(page)}`);
  // The focused row wins over the preview: M acts where the keyboard is.
  await page.focus(`a.row[data-id="${ids[8]}"] >> visible=true`);
  await page.keyboard.press("m");
  check((await pressed(page, `.row-wrap:has(a.row[data-id="${ids[8]}"]) > .mark-toggle`)) === "true" && (await pressed(page, ".detail .mark-toggle.mark-switch")) === "true", "M did not act on the focused row");
  await page.keyboard.press("m");

  // On the module's page: under the heading and among the actions of the sidebar, one state.
  await page.evaluate(() => document.activeElement?.blur());
  await step("full page", () => page.keyboard.press("v"), () => location.pathname.startsWith("/catalog/module/") && Boolean(onPage(".module-page .mark-toggle.mark-switch")));
  check((await pressed(page, ".module-page .mark-toggle.mark-switch")) === "true" && (await pressed(page, ".sidebar .action.mark-toggle")) === "true", "the module's page does not know the mark");
  const onPage = await placement(page, ".module-page", ".module-grid > aside .section");
  check((onPage.beside || onPage.below) && onPage.edge <= 0.5, `on the module's page the switch is not at the right end of the line of the credits nor under it: ${JSON.stringify(onPage)}`);
  await step("the sidebar's action takes the mark away", () => page.click(".sidebar .action.mark-toggle >> visible=true"), () => onPage(".module-page .mark-toggle.mark-switch").getAttribute("aria-pressed") === "false" && onPage(".sidebar .action.mark-toggle").textContent.includes("Merken"));
  await step("and M gives it back", () => page.keyboard.press("m"), () => onPage(".sidebar .action.mark-toggle").getAttribute("aria-pressed") === "true");

  // A reload keeps the marks; server HTML knows nothing of them.
  const html = await (await context.request.get(page.url())).text();
  check(!html.includes('aria-pressed="true"') && !html.includes("nav-count"), "server HTML shows something marked");
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(() => onPage(".module-page .mark-toggle.mark-switch")?.getAttribute("aria-pressed") === "true", null, { timeout: 8000 }).catch(() => problems.push("a reload lost the mark"));
  await page.evaluate(() => { window.__marker = 1; });
  const marked = await stored(page);
  check(JSON.stringify(marked) === JSON.stringify([ids[6], ids[4], ids[2]]), `stored after all of this: ${marked}`);

  // ----- the list of marked modules -----
  await step("the rail leads to the marked modules", () => page.click('.rail .nav[data-area="bookmarks"] >> visible=true'), () => location.pathname === "/bookmarks" && allOnPage(".rows a.row").length === 3);
  check(JSON.stringify(await listIds(page)) === JSON.stringify(marked), `the order of marking, newest first: ${await listIds(page)}`);
  const numbers = await page.evaluate(() => ({
    count: onPage(".list .count").textContent,
    label: onPage(".list .count-label").textContent,
    sections: [...allOnPage(".rows .sem")].map((sem) => sem.textContent),
    credits: [...allOnPage(".rows a.row .lp")].map((lp) => parseFloat(lp.textContent.replace(",", "."))).filter((n) => !Number.isNaN(n)).reduce((a, b) => a + b, 0),
    current: onPage('.rail .nav[data-area="bookmarks"]').getAttribute("aria-current"),
    title: document.title,
    frame: ["sidebar"].map((id) => { const r = onPage(`#${id}`).getBoundingClientRect(); return [r.left, r.top]; })[0],
  }));
  check(numbers.count === "3" && numbers.label.startsWith("gemerkte Module"), `the numbers of the list: ${JSON.stringify(numbers)}`);
  check(numbers.label.includes(`${String(numbers.credits).replace(".", ",")} LP`), `the credits do not add up: ${JSON.stringify(numbers)}`);
  // Marked just now: one section, „Heute", with its number.
  check(JSON.stringify(numbers.sections) === JSON.stringify(["Heute 3"]), `the sections by the time of marking: ${JSON.stringify(numbers.sections)}`);
  check(numbers.current === "page" && numbers.title.startsWith("Merkliste"), `rail or title: ${JSON.stringify(numbers)}`);

  // Sorting: the heads of the columns are the order, in the URL; the phone's list of the orders
  // (hidden here) says the same.
  await step("sort by credits", () => page.click(".cols a.c-lp >> visible=true"), () => location.search.includes("sort=ects") && !location.search.includes("desc"));
  const credits = () => page.evaluate(() => [...allOnPage(".rows a.row .lp")].map((lp) => parseFloat(lp.textContent.replace(",", ".")) || 0));
  let sorted = await credits();
  check(sorted.every((n, i) => i === 0 || sorted[i - 1] <= n), `not sorted by credits: ${sorted}`);
  check(await page.evaluate(() => onPage('.cols a.c-lp[aria-current="true"]')?.textContent.includes("↑") && onPage('.order-list a[aria-current="page"]')?.textContent.startsWith("Leistungspunkte") && !allOnPage(".rows .sem").length), "the heads or the orders do not show the order, or an order without sections has some");
  await step("the other way round", () => page.click(".cols a.c-lp >> visible=true"), () => location.search.includes("desc=1"));
  sorted = await credits();
  check(sorted.every((n, i) => i === 0 || sorted[i - 1] >= n), `not sorted downwards: ${sorted}`);
  await step("„Gemerkt“ is the order of marking", () => page.click(".cols a.c-marked >> visible=true"), () => location.search === "" && onPage(".cols a.c-marked")?.textContent.includes("↓"));
  check(JSON.stringify(await listIds(page)) === JSON.stringify(marked), "the order of marking did not come back");
  await step("„Gemerkt“ again: the oldest first", () => page.click(".cols a.c-marked >> visible=true"), () => location.search === "?desc=1" && onPage(".cols a.c-marked")?.textContent.includes("↑"));
  check(JSON.stringify(await listIds(page)) === JSON.stringify([...marked].reverse()), `the oldest first: ${await listIds(page)}`);
  await step("and back", () => page.click(".cols a.c-marked >> visible=true"), () => location.search === "");
  await step("„Turnus“ orders by the offer, in sections", () => page.click(".cols a.c-turnus >> visible=true"), () => location.search === "?sort=offer" && allOnPage(".rows .sem").length > 0);
  check(await page.evaluate(() => [...allOnPage(".rows .sem")].every((sem) => /^(Winter|Sommer|Jedes Semester|Unregelmäßig|Ohne Angabe) \d+$/.test(sem.textContent))), `the sections of the offer: ${await page.evaluate(() => [...allOnPage(".rows .sem")].map((sem) => sem.textContent))}`);
  await step("back to the order of marking", () => page.click(".cols a.c-marked >> visible=true"), () => location.search === "");

  // The search at the head searches the list, and never the address (R20): what is typed may
  // name what only a note says. Under what it found, the same text in the whole catalog.
  const word = await page.evaluate((id) => onPage(`a.row[data-id="${id}"] .t b`).textContent.split(/[\s(),:–-]+/).filter((w) => w.length >= 4).sort((a, b) => b.length - a.length)[0], marked[1]);
  await step("the search finds among the marked modules", () => page.fill("#topsearch", word), (id) => Boolean(onPage(`.rows a.row[data-id="${id}"] mark`)) && /· \d[\d.]* Module?$/.test(onPage(".rows .catalog-line")?.textContent ?? ""), marked[1]);
  const found = await page.evaluate(() => ({ search: location.search, label: onPage(".list .count-label").textContent, line: onPage(".rows .catalog-line").textContent, href: onPage(".rows .catalog-line").getAttribute("href"), sections: allOnPage(".rows .sem").length, marks: allOnPage(".rows a.row mark").length }));
  check(found.search === "" && found.label.startsWith("von 3 gemerkten") && found.line.includes(`‚${word}‘ im ganzen Katalog`) && found.href.startsWith("/catalog?q=") && found.sections === 0 && found.marks > 0, `the search on the Merkliste: ${JSON.stringify(found)}`);
  await step("nothing found, and the way into the catalog stays", () => page.fill("#topsearch", "zzqqzz"), () => allOnPage(".rows a.row").length === 0 && onPage(".rows .state-title")?.textContent.includes("Nichts Gemerktes") && Boolean(onPage(".rows .catalog-line")));
  await step("the search emptied: the whole list", () => page.fill("#topsearch", ""), () => allOnPage(".rows a.row").length === 3 && !onPage(".rows .catalog-line"));
  check(!requests.some((r) => r.includes(encodeURIComponent(word)) || r.includes(word) || r.includes("zzqqzz")), "a request carried what was typed into the Merkliste's search");

  // Halves of the year: the numbers on the switch are the numbers of the lists. The switch is
  // there while the marked modules are not all offered alike.
  const seasons = await page.evaluate(() => [...allOnPage('.sidebar .seg a')].map((a) => [a.textContent.replace(/\d+/g, "").trim(), Number(a.querySelector(".num").textContent), a.getAttribute("aria-checked")]));
  const turnus = await page.evaluate(() => [...new Set([...allOnPage(".rows a.row .turnus")].map((t) => t.getAttribute("title")))]);
  check(seasons.length ? JSON.stringify(seasons.map((s) => s[0])) === JSON.stringify(["Alle", "Winter", "Sommer"]) && seasons[0][1] === 3 && seasons[0][2] === "true" : turnus.length === 1, `the halves of the year: ${JSON.stringify(seasons)} for ${JSON.stringify(turnus)}`);
  for (const [index, name, code] of seasons.length ? [[1, "Winter", "winter"], [2, "Sommer", "summer"]] : []) {
    await step(`only ${name}`, () => page.click(`.sidebar .seg a:nth-child(${index + 1}) >> visible=true`), (code) => location.search.includes(`turnus=${code}`), code);
    const shown = await page.evaluate(() => ({ rows: allOnPage(".rows a.row").length, count: Number(onPage(".list .count").textContent), tag: onPage(".active-filters .tag")?.textContent ?? "", empty: Boolean(onPage(".rows .state")) }));
    check(shown.rows === seasons[index][1] && shown.count === seasons[index][1] && shown.tag.includes(name), `${name}: ${JSON.stringify(shown)} instead of ${seasons[index][1]}`);
    check(shown.rows > 0 || shown.empty, `${name}: an empty list says nothing`);
  }
  if (seasons.length) await step("the tag takes the filter away", () => page.click(".active-filters .tag a >> visible=true"), () => location.search === "" && allOnPage(".rows a.row").length === 3);

  // The preview, as in the catalog; the list stays.
  await step("preview of a marked module", () => page.click(`a.row[data-id="${marked[1]}"] >> visible=true`), (id) => location.search === `?open=${id}` && Boolean(onPage(".detail h2")), marked[1]);
  check((await pressed(page, ".detail .mark-toggle.mark-switch")) === "true", "the preview of a marked module does not show it marked");
  // (Measured once the panel has arrived: it slides in.)
  await page.waitForFunction(() => onPage(".work > .detail").getAnimations().every((animation) => animation.playState === "finished"), null, { timeout: 4000 }).catch(() => {});
  const floats = await page.evaluate(() => { const [list, detail] = [onPage(".panel.list"), onPage(".work > .detail")].map((el) => el.getBoundingClientRect()); return [detail.right - list.right, detail.top - list.top, detail.bottom - list.bottom, list.left - detail.left]; });
  check(floats.slice(0, 3).every((d) => Math.abs(d) <= 1) && floats[3] < -100, `the preview does not float at the right edge of the list: ${floats}`);

  // A mark taken away here stays on the page, dimmed; the numbers follow; one click undoes it.
  await page.evaluate(() => { allOnPage(".rows .row-wrap").forEach((wrap, i) => { wrap.__kept = i; }); });
  await step("a mark taken away stays on the page", () => page.click(".detail .mark-toggle.mark-switch >> visible=true"), (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`).classList.contains("unmarked"), marked[1]);
  const after = await page.evaluate(() => ({ rows: allOnPage(".rows a.row").length, count: onPage(".list .count").textContent, kept: [...allOnPage(".rows .row-wrap")].every((wrap, i) => wrap.__kept === i), all: onPage(".sidebar .seg a .num")?.textContent ?? "2" }));
  check(after.rows === 3 && after.count === "2" && after.all === "2" && after.kept && (await badge(page)) === "2", `after taking a mark away: ${JSON.stringify(after)}`);
  // (The preview floats over the right end of the rows, so their buttons are reached once it is closed.)
  await step("Esc closes the preview", () => page.keyboard.press("Escape"), () => location.search === "" && !onPage(".detail"));
  check(await page.evaluate((id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`).classList.contains("unmarked"), marked[1]), "closing the preview took the unmarked module off the page");
  await step("one click gives it back", () => page.click(`.row-wrap:has(a.row[data-id="${marked[1]}"]) > .mark-toggle >> visible=true`), () => allOnPage(".rows .row-wrap.unmarked").length === 0 && onPage(".list .count").textContent === "3");

  // The menu of the whole list (its ⋯ in the sidebar, a right click on „Alle"): „Als Text kopieren"
  // hands the marked modules to the clipboard as text, and the note at the bottom says so.
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  const listMenu = async () => {
    await page.click('.coll-nav > div.coll-item:first-of-type > a >> visible=true', { button: "right" });
    await page.waitForSelector(".view-menu [role=menuitem] >> visible=true", { timeout: 4000 });
  };
  await step("copy the list", async () => { await listMenu(); await page.click('.view-menu .st-pop-item:has-text("Als Text kopieren")'); }, () => document.querySelector(".wait-note .mark-toast-said")?.textContent.includes("Liste kopiert") && !document.querySelector(".view-menu"));
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  // Each module a line of a message: its number, title, credits and the link to its page.
  check(copied.split("\n").length === 3 && marked.every((id) => copied.includes(`/catalog/module/${id}`)) && copied.split("\n").every((line) => /^\d{5} .+ · (\d+(,\d+)? LP · )?https?:\/\/\S+\/catalog\/module\/\d{5}$/.test(line)), `copied: ${JSON.stringify(copied)}`);

  // A list travels to another device in the fragment of a link, which no browser sends anywhere:
  // the other device asks before it adds anything, and the answer takes the ids out of the address.
  const carried = await stored(page);
  await listMenu();
  check(await page.evaluate(() => Boolean(document.querySelector('.view-menu .st-pop-item:has(small)')?.textContent.includes("Link für ein anderes Gerät"))), "the link for another device lost its line in the menu");
  await step("copy the link for another device", () => page.click('.view-menu .st-pop-item:has-text("Link für ein anderes Gerät")'), () => document.querySelector(".wait-note .mark-toast-said")?.textContent.includes("Link kopiert"));
  const link = await page.evaluate(() => navigator.clipboard.readText());
  // The list as a code (pack/): characters an address carries as they are, the last two check the
  // rest, and the last one is a letter or a digit. Shorter than the ids one by one.
  const code = link.slice(`${base}/bookmarks#m=`.length);
  check(link.startsWith(`${base}/bookmarks#m=`) && /^[A-Za-z0-9._~-]*[A-Za-z0-9]$/.test(code) && code.length < carried.join(",").length, `the link for another device: ${link}`);
  {
    const elsewhere = await browser.newContext({ viewport: { width: 1500, height: 900 } });
    await elsewhere.addInitScript(ON_PAGE);
    const device = await elsewhere.newPage();
    const sent = [];
    watch(device, sent);
    await device.goto(link, { waitUntil: "domcontentloaded" });
    await takeover(device);
    await device.waitForSelector(".offer >> visible=true", { timeout: 8000 }).catch(() => problems.push("another device: the list from the link is not offered"));
    const offer = await device.evaluate(() => ({ text: onPage(".offer")?.textContent ?? "", rows: allOnPage(".rows a.row").length, left: Math.abs(onPage(".offer").getBoundingClientRect().left - onPage(".list .count").getBoundingClientRect().left) }));
    check(offer.text.includes("3 Module aus einem Link") && offer.rows === 0 && (await stored(device)).length === 0, `another device: nothing may be added before the visitor says so: ${JSON.stringify(offer)}`);
    check(offer.left <= 0.5, `another device: the offer does not start where the head of the list starts: ${offer.left}`);
    const entries = await device.evaluate(() => history.length);
    await device.click("#offer-add >> visible=true");
    await device.waitForFunction(() => allOnPage(".rows a.row").length === 3 && location.hash === "" && !onPage(".offer"), null, { timeout: 8000 }).catch(() => problems.push("another device: the list did not arrive, or the ids stayed in the address"));
    // The same modules; not in the same order, since the link carries them as a set.
    const same = (ids) => JSON.stringify([...ids].sort()) === JSON.stringify([...carried].sort());
    check(same(await stored(device)) && same(await listIds(device)), `another device: what arrived: ${await stored(device)}`);
    check((await device.evaluate(() => history.length)) === entries, "another device: answering left a history entry with the ids behind");
    // The same link again: nothing new. And one module more than the list has.
    await device.goto(link, { waitUntil: "domcontentloaded" });
    await takeover(device);
    await device.waitForFunction(() => onPage(".offer")?.textContent.includes("schon auf deiner Merkliste") && !document.getElementById("offer-add"), null, { timeout: 8000 }).catch(() => problems.push("another device: the same link again offers to add what is there"));
    await device.click("#offer-dismiss >> visible=true");
    await device.waitForFunction(() => location.hash === "" && !onPage(".offer"), null, { timeout: 8000 }).catch(() => problems.push("another device: „In Ordnung“ did not take the ids out of the address"));
    // One module of the link that the list lacks (the list here loses one, a page load later).
    await device.evaluate(([key, id]) => localStorage.setItem(key, (localStorage.getItem(key) || "").split("\n").filter((line) => line && line.split("\t")[0] !== id).map((line) => `${line}\n`).join("")), [KEY, carried[0]]);
    await device.goto(`${base}/`, { waitUntil: "domcontentloaded" });
    await device.goto(link, { waitUntil: "domcontentloaded" });
    await takeover(device);
    await device.waitForFunction(() => onPage(".offer")?.textContent.includes("3 Module aus einem Link") && onPage(".offer").textContent.includes("Eins davon"), null, { timeout: 8000 }).catch(() => problems.push("another device: a link with one new module is not counted right"));
    await device.click("#offer-dismiss >> visible=true");
    check((await stored(device)).length === 2, "another device: „Verwerfen“ added something");
    // A character typed wrong, two neighbours swapped: caught for certain by the check characters,
    // named, and nothing of the list offered.
    const at = [...code].findIndex((c, i) => i + 1 < code.length && c !== code[i + 1]);
    const swapped = `${base}/bookmarks#m=${code.slice(0, at)}${code[at + 1]}${code[at]}${code.slice(at + 2)}`;
    for (const broken of [link.replace(/.$/, (c) => (c === "A" ? "B" : "A")), swapped]) {
      await device.goto(broken, { waitUntil: "domcontentloaded" });
      await takeover(device);
      await device.waitForFunction(() => onPage(".offer")?.textContent.includes("beschädigt") && !document.getElementById("offer-add"), null, { timeout: 8000 }).catch(() => problems.push(`another device: a broken link is not named: ${broken}`));
      await device.click("#offer-dismiss >> visible=true");
      await device.waitForFunction(() => location.hash === "" && !onPage(".offer"), null, { timeout: 8000 }).catch(() => problems.push("another device: „In Ordnung“ did not take a broken link out of the address"));
    }
    check((await stored(device)).length === 2, "another device: a broken link added something");
    const told = sent.filter((r) => r.includes(code) || carried.some((id) => r.includes(id)));
    check(told.length === 0, `another device: requests that carry the list: ${told.slice(0, 3)}`);
    await elsewhere.close();
  }

  // Emptying asks first (in the menu, the answer has the focus) and can be taken back in the note.
  const beforeEmptying = await stored(page);
  await step("emptying asks first", async () => { await listMenu(); await page.click('.view-menu .st-pop-item:has-text("Merkliste leeren")'); }, () => document.activeElement?.id === "clear-yes");
  check((await stored(page)).length === 3, "asking already emptied the list");
  await step("emptied", () => page.keyboard.press("Enter"), () => onPage(".list .count").textContent === "0" && document.querySelector(".wait-note .mark-toast-said")?.textContent.includes("Merkliste geleert"));
  check((await badge(page)) === null && (await page.evaluate((key) => localStorage.getItem(key), KEY)) === null && (await page.evaluate(() => allOnPage(".rows .row-wrap.unmarked").length)) === 3, "an emptied list leaves something behind, or takes the rows away");
  await step("„Rückgängig“ brings the marks back", () => page.click(".wait-note .plan-toast-undo >> visible=true"), () => onPage(".list .count").textContent === "3" && allOnPage(".rows .row-wrap.unmarked").length === 0);
  check(beforeEmptying.length === 3 && JSON.stringify(await stored(page)) === JSON.stringify(beforeEmptying), `what came back: ${await stored(page)} instead of ${beforeEmptying}`);

  // Collections (step 2, „Ordnen"; folia/design/merken/oberflaeche.html): made in the sidebar,
  // where a name typed that is there opens that one; a view of the list with an address of its own.
  const COLLECTIONS = "betula.collections.v1";
  await step("a new collection", async () => { await page.click(".coll-nav .coll-item.add >> visible=true"); await page.waitForSelector(".coll-nav .coll-item.input input:focus"); await page.keyboard.type("Für Lena"); await page.keyboard.press("Enter"); }, () => /[?&]collection=\d+/.test(location.search) && onPage(".rows .state-title")?.textContent === "‚Für Lena‘ ist leer");
  const lena = await page.evaluate(() => new URLSearchParams(location.search).get("collection"));
  await step("a name there is never a second one", async () => { await page.click(".coll-nav .coll-item.add >> visible=true"); await page.waitForSelector(".coll-nav .coll-item.input input:focus"); await page.keyboard.type("für lena "); }, () => onPage(".coll-taken")?.textContent.includes("Gibt es schon"));
  await page.keyboard.press("Escape");
  await step("back to all of the list", () => page.click('.coll-nav > div.coll-item:first-of-type > a >> visible=true'), () => !location.search.includes("collection=") && allOnPage(".rows a.row").length === 3);
  // The arrow of the module's switch opens the menu of its collections and its note, the field
  // with the focus; each choice holds at once.
  await step("the preview", () => page.click(`a.row[data-id="${marked[0]}"] >> visible=true`), () => Boolean(onPage('.detail [data-action="collect"]')));
  await step("its arrow opens the menu", () => page.click('.detail .hero [data-action="collect"] >> visible=true'), () => Boolean(document.activeElement?.closest?.(".collect-field")));
  await step("into the collection", () => page.click('.collect-pop .collect-item:has-text("Für Lena")'), (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"]) .row-tag`)?.textContent === "Für Lena", marked[0]);
  await page.click(".collect-pop .collect-note");
  await page.keyboard.type("Lena **fragen**");
  await page.waitForFunction((id) => onPage(`.row-wrap:has(a.row[data-id="${id}"]) .row-note strong`)?.textContent === "fragen", marked[0], { timeout: 4000 }).catch(() => problems.push("the note does not show at its row"));
  await page.keyboard.press("Escape");
  check(await page.evaluate(() => !document.querySelector(".collect-pop")), "Esc does not close the menu");
  check(await page.evaluate(() => onPage(".detail .module-note strong")?.textContent === "fragen" && onPage('.detail [data-action="mark"] small')?.textContent === "Für Lena"), "the module does not show its collection and its note");
  const shelf = await page.evaluate((key) => localStorage.getItem(key) || "", COLLECTIONS);
  check(shelf.includes("\tFür Lena\n") && shelf.includes(`m\t${marked[0]}\t${lena}\n`) && shelf.includes(`n\t${marked[0]}\tLena **fragen**`), `what the store of the collections keeps: ${JSON.stringify(shelf)}`);
  await page.keyboard.press("Escape");
  // Its view: its own numbers, its rows without its own name.
  await step("the collection's view", () => page.click(`.coll-nav a[href*="collection=${lena}"] >> visible=true`), () => allOnPage(".rows a.row").length === 1 && onPage(".list .count-label").textContent.includes("in Für Lena") && !onPage(".rows .row-tag"));
  // The search at the head finds what the visitor keeps: the note.
  await step("the search finds the note", async () => { await page.click(".coll-nav > div.coll-item:first-of-type > a >> visible=true"); await page.fill("#topsearch", "fragen"); }, (id) => allOnPage(".rows a.row").length === 1 && onPage(".rows a.row").dataset.id === id, marked[0]);
  await page.fill("#topsearch", "");
  // Deleting asks nothing: the modules stay marked, and the note after it puts it back.
  await step("deleting the collection", async () => { await page.click(`.coll-nav a[href*="collection=${lena}"] >> visible=true`, { button: "right" }); await page.click('.view-menu .st-pop-item:has-text("Löschen")'); }, () => !onPage(".coll-nav a[href*='collection=']") && document.querySelector(".wait-note .mark-toast-said")?.textContent.includes("‚Für Lena‘ gelöscht · 1 bleibt gemerkt"));
  check((await stored(page)).length === 3, "deleting the collection took marks away");
  await step("„Rückgängig“ puts it back", () => page.click(".wait-note .plan-toast-undo >> visible=true"), (id) => Boolean(onPage(".coll-nav a[href*='collection=']")) && onPage(`.row-wrap:has(a.row[data-id="${id}"]) .row-tag`)?.textContent === "Für Lena", marked[0]);
  // Its key asks first (owner, 2026-10-20: „einen Delete-Shortcut …, wo man dann aber nochmal
  // gefragt wird"): Entf at the collection in the sidebar, the question where its menu would stand,
  // „Löschen" with the focus; Esc keeps it, and the focus goes back to the collection.
  const lenaLink = `.coll-nav a[href*="collection=${lena}"] >> visible=true`;
  await page.focus(lenaLink);
  await step("Entf asks", () => page.keyboard.press("Delete"), () => document.querySelector(".view-menu.ask .view-menu-ask")?.textContent.startsWith("‚Für Lena‘ löschen?") && document.activeElement?.id === "delete-yes");
  check(await page.evaluate(() => document.querySelector(".view-menu.ask .view-menu-ask small")?.textContent === "Das Modul darin bleibt gemerkt."), "the question does not say what stays");
  await step("Esc keeps it", () => page.keyboard.press("Escape"), (lena) => !document.querySelector(".view-menu") && document.activeElement?.matches?.(`.coll-nav a[href*="collection=${lena}"]`), lena);
  // Its menu shows the key beside „Löschen", and the key asks there as well; F2 there renames.
  await step("its menu shows the key", () => page.click(lenaLink, { button: "right" }), () => [...document.querySelectorAll(".view-menu .st-pop-item")].some((item) => item.textContent.includes("Löschen") && item.querySelector("kbd")?.textContent === "Entf") && Boolean(document.activeElement?.closest?.(".view-menu")));
  await step("Entf in the menu asks", () => page.keyboard.press("Delete"), () => document.activeElement?.id === "delete-yes" && Boolean(document.querySelector(".view-menu.ask")));
  await step("„Abbrechen“ keeps it", () => page.click('.view-menu.ask button:has-text("Abbrechen")'), (lena) => !document.querySelector(".view-menu") && Boolean(onPage(`.coll-nav a[href*="collection=${lena}"]`)), lena);
  await step("F2 in the menu renames", async () => { await page.click(lenaLink, { button: "right" }); await page.waitForFunction(() => document.activeElement?.closest?.(".view-menu")); await page.keyboard.press("F2"); }, () => document.activeElement?.matches?.(".coll-nav .coll-item.input input") && document.activeElement.value === "Für Lena");
  await page.keyboard.press("Escape");
  // Entf, then Enter: gone as by the click, the note after it the same; the focus on the view shown.
  await page.focus(lenaLink);
  await step("Entf and Enter delete it", async () => { await page.keyboard.press("Delete"); await page.waitForFunction(() => document.activeElement?.id === "delete-yes", null, { timeout: 4000 }); await page.keyboard.press("Enter"); }, () => !onPage(".coll-nav a[href*='collection=']") && document.querySelector(".wait-note .mark-toast-said")?.textContent.includes("‚Für Lena‘ gelöscht · 1 bleibt gemerkt") && document.activeElement?.matches?.(".coll-nav a[aria-current=page]"));
  check((await stored(page)).length === 3, "deleting by its key took marks away");
  await step("„Rückgängig“ puts it back again", () => page.click(".wait-note .plan-toast-undo >> visible=true"), (id) => Boolean(onPage(".coll-nav a[href*='collection=']")) && onPage(`.row-wrap:has(a.row[data-id="${id}"]) .row-tag`)?.textContent === "Für Lena", marked[0]);
  // No key in a menu reaches the module behind it: M there marked the module open beside the list.
  const opened = !(await page.evaluate(() => Boolean(onPage('.detail [data-action="mark"]'))));
  if (opened) await page.click(`a.row[data-id="${marked[0]}"] >> visible=true`, { position: { x: 24, y: 12 } });
  await page.waitForFunction(() => Boolean(onPage('.detail [data-action="mark"]')), null, { timeout: 4000 }).catch(() => problems.push("no module open beside the list"));
  const markedBefore = (await stored(page)).length;
  await page.click(lenaLink, { button: "right" });
  await page.waitForFunction(() => document.activeElement?.closest?.(".view-menu"));
  await page.keyboard.press("m");
  await page.waitForTimeout(300);
  check((await stored(page)).length === markedBefore && (await page.evaluate(() => Boolean(document.querySelector(".view-menu")))), "M in a collection's menu marked the module behind it, or closed the menu");
  await page.keyboard.press("Escape");
  // The module opened for it closes again (Esc, with the menu gone and the focus back at its
  // collection): the sidebar is a mode of the keyboard (docs/folia/frontend.md R24), which the first
  // Esc leaves into the list, and the next closes the module.
  if (opened) {
    await page.waitForFunction(() => !document.querySelector(".view-menu") && !document.activeElement?.closest?.(".view-menu"));
    await page.keyboard.press("Escape");
    await page.waitForFunction(() => !document.activeElement?.closest?.("#sidebar"), null, { timeout: 2000 }).catch(() => problems.push("Esc in the sidebar did not lead out of it"));
    await page.keyboard.press("Escape");
    await page.waitForFunction(() => !onPage('.detail [data-action="mark"]'), null, { timeout: 4000 }).catch(() => problems.push("the module opened for the check does not close"));
  }
  // The menu never stands under the rail (owner, 2026-10-20), the sidebar at its narrowest.
  await page.evaluate(() => document.documentElement.style.setProperty("--w-filters", "232px"));
  await step("its ⋯ in a narrow sidebar", async () => { await page.hover(lenaLink); await page.click(`.coll-nav .coll-item:has(> a[href*="collection=${lena}"]) > .coll-more >> visible=true`); }, () => {
    const menu = document.querySelector(".view-menu")?.getBoundingClientRect();
    const rail = document.querySelector("aside.rail").getBoundingClientRect();
    return Boolean(menu) && menu.left >= rail.right + 8 - 0.5 && Boolean(document.activeElement?.closest?.(".view-menu"));
  });
  await page.keyboard.press("Escape");
  await page.evaluate(() => document.documentElement.style.removeProperty("--w-filters"));
  check(!requests.some((r) => r.includes("Lena") || r.includes("fragen") || r.includes(encodeURIComponent("Für"))), "a request carried a collection's name or a note");

  // The rail's item is a tab: it remembers how the list was left.
  await step("sorted, then away", () => page.click(".cols a.c-events >> visible=true"), () => location.search === "?sort=events");
  await step("to the catalog", () => page.click('.rail .nav[data-area="catalog"] >> visible=true'), () => location.pathname.startsWith("/catalog"));
  await step("the tab leads back to the list as it was left", () => page.click('.rail .nav[data-area="bookmarks"] >> visible=true'), () => location.pathname === "/bookmarks" && location.search === "?sort=events");

  // A module opened from the marked modules stays among them (owner, 2026-09-24): „Vollbild" fills
  // the list's place, the address stays the list's, the tab stays the current one, and the
  // catalog's tab does not hear of the module. „Zurück" and Esc lead back to the list with the
  // module beside it, through the history.
  const catalogTab = () => page.getAttribute('.rail .nav[data-area="catalog"] >> visible=true', "href");
  const catalogBefore = await catalogTab();
  await step("preview, then full page", async () => { await page.click(`a.row[data-id="${marked[0]}"] >> visible=true`); await page.waitForSelector(".detail h2 >> visible=true"); await page.evaluate(() => document.activeElement?.blur()); await page.keyboard.press("v"); }, (id) => location.pathname === "/bookmarks" && location.search.includes(`open=${id}&full=1`) && Boolean(onPage(".module-page h2")) && Boolean(onPage("#sidebar .toc.jumps")), marked[0]);
  check(await page.evaluate(() => onPage('.rail .nav[data-area="bookmarks"]')?.getAttribute("aria-current") === "page"), "full page: the tab of the marked modules is not the current one");
  check((await catalogTab()) === catalogBefore, `full page: the catalog's tab leads to ${await catalogTab()} instead of ${catalogBefore}`);
  check(await page.evaluate(() => { const back = onPage('[data-action="back"]').getAttribute("href"); return back.startsWith("/bookmarks?") && back.includes("open=") && !back.includes("full="); }), "„Zurück“ of a module opened from the marked modules leads elsewhere");
  check((await pressed(page, ".module-page .mark-toggle.mark-switch")) === "true", "full page: the module's page does not know the mark");
  const entries = await page.evaluate(() => history.length);
  await step("Esc leads back to the marked modules", () => page.keyboard.press("Escape"), () => location.pathname === "/bookmarks" && location.search.includes("open=") && !location.search.includes("full=") && allOnPage(".rows a.row").length === 3 && Boolean(onPage(".detail h2")));
  check((await page.evaluate(() => history.length)) === entries, "back from the full page added a history entry instead of walking back");
  await page.keyboard.press("Escape");

  // Another tab of the same browser marks a module: this one follows.
  const other = await context.newPage();
  watch(other, requests);
  await other.goto(base + "/catalog?turnus=summer", { waitUntil: "domcontentloaded" });
  await takeover(other);
  const fresh = await other.evaluate((known) => [...allOnPage(".rows a.row")].map((row) => row.dataset.id).find((id) => !known.includes(id)), marked);
  await other.click(`.row-wrap:has(a.row[data-id="${fresh}"]) > .mark-toggle >> visible=true`);
  await page.waitForFunction((id) => onPage(".rows a.row")?.dataset.id === id && onPage(".list .count").textContent === "4", fresh, { timeout: 8000 }).catch(() => problems.push("a mark from another tab did not arrive"));
  check((await badge(page)) === "4", "the rail did not follow the other tab");
  await other.close();

  // What the snapshot does not know is named, and garbage in the storage is ignored.
  await page.evaluate(([key, id]) => localStorage.setItem(key, `99999999\t5\n<script>alert(1)</script>\t4\n../../etc\t3\n${id}\tyesterday\n${id}\t1\n`), [KEY, marked[0]]);
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(() => allOnPage(".rows .row-wrap").length === 2, null, { timeout: 8000 }).catch(() => problems.push("garbage in the storage: the list did not come up with two entries"));
  const unknown = await page.evaluate(() => ({ group: [...allOnPage(".rows .sem")].map((sem) => sem.textContent).find((text) => text.startsWith("Nicht")), missing: onPage(".rows .row.missing")?.textContent ?? "", count: onPage(".list .count").textContent, markup: onPage(".rows").innerHTML.includes("<script>") }));
  check(unknown.group === "Nicht im Modulkatalog" && unknown.missing.includes("99999999") && unknown.count === "2" && !unknown.markup, `what the snapshot does not know: ${JSON.stringify(unknown)}`);
  await page.click(".row-wrap:has(.row.missing) > .mark-toggle >> visible=true");
  check(JSON.stringify(await stored(page)) === JSON.stringify([marked[0]]), `the mark of an unknown module cannot be taken away: ${await stored(page)}`);

  // Privacy: whatever was marked, no request named it (the ids are five digits or more, the
  // pages of modules that were opened on purpose aside).
  const leaked = requests.filter((r) => r.includes("bookmark") && !r.includes("/bookmarks") || /[?&](ids|marks|marked)=/.test(r));
  check(leaked.length === 0, `requests that carry marks: ${leaked.slice(0, 3)}`);
  const beyond = requests.filter((r) => !r.startsWith(base));
  check(beyond.length === 0, `requests to somewhere else: ${beyond.slice(0, 3)}`);
  await context.close();
}

// ---------- „Gemerkt" as a filter of the catalog ----------
{
  const { page, step, context } = await open({ viewport: { width: 1500, height: 900 } }, "/catalog?turnus=winter");
  const ids = (await listIds(page)).slice(0, 8);
  const marked = [ids[1], ids[3], ids[5]];
  for (const id of marked) await page.click(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle >> visible=true`);
  await page.waitForFunction((n) => onPage('.rail .nav[data-area="bookmarks"] .nav-count')?.textContent === String(n), marked.length, { timeout: 8000 }).catch(() => problems.push("filter: the marks were not made"));

  const chip = '.filters .chip:has(.chip-label:text-is("Gemerkt"))';
  await step("only the marked ones", () => page.click(chip), () => location.search.includes("marked=only") && allOnPage(".rows a.row").length === 3);
  const shown = await listIds(page);
  check(JSON.stringify([...shown].sort()) === JSON.stringify([...marked].sort()), `only the marked ones: ${shown} instead of ${marked}`);
  check((await page.evaluate(() => onPage(".list .count").textContent)) === "3", "the header counts something else than the list");
  check(await page.evaluate(() => [...allOnPage(".active-filters .tag")].some((tag) => tag.textContent.includes("nur gemerkte"))), "the filter is not among the tags above the list");
  // The other filters still hold, and a mark made now changes this list.
  check(await page.evaluate(() => location.search.includes("turnus=winter")), "turning the filter on dropped the rest of the filter");
  // A mark taken away here takes the module out of this list at once.
  await step("a mark taken away leaves the list", () => page.click(`.row-wrap:has(a.row[data-id="${shown[0]}"]) > .mark-toggle >> visible=true`), () => allOnPage(".rows a.row").length === 2);

  await step("without the marked ones", () => page.click(chip), () => location.search.includes("marked=none"));
  const rest = await listIds(page);
  const still = marked.filter((id) => id !== shown[0]);
  check(rest.length > 0 && !rest.some((id) => still.includes(id)), `„ohne Gemerkte" still shows marked modules: ${rest.slice(0, 5)}`);
  check(rest.includes(shown[0]), "the module whose mark was taken away is missing from „ohne Gemerkte“");
  await step("and a mark made here leaves this one", () => page.click(`.row-wrap:has(a.row[data-id="${shown[0]}"]) > .mark-toggle >> visible=true`), (id) => !onPage(`a.row[data-id="${id}"]`), shown[0]);
  await step("the tag takes the filter off", () => page.click('.active-filters .tag:has-text("gemerkte") a >> visible=true'), () => !location.search.includes("marked="));

  // Nothing marked matches the rest of the filter: the app says so, and says nothing else.
  await page.goto(base + "/catalog?q=zzzzzz&marked=only", { waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.waitForFunction(() => onPage(".rows .state-title")?.textContent === "Keine gemerkten Module in dieser Liste", null, { timeout: 8000 }).catch(() => problems.push("filter: an empty list does not say that nothing marked matches"));
  const stored_ids = await stored(page);
  check(stored_ids.length === 3, `filter: the marks changed while filtering: ${stored_ids}`);
  await context.close();

  // Shared without the app: the page says that it cannot know, instead of „nothing found".
  const plain = await browser.newContext({ viewport: { width: 1500, height: 900 }, javaScriptEnabled: false });
  const flat = await plain.newPage();
  await flat.goto(base + "/catalog?marked=only", { waitUntil: "domcontentloaded" });
  const said = await flat.evaluate(() => ({ title: document.querySelector(".rows .state-title")?.textContent ?? "", rows: document.querySelectorAll(".rows a.row").length, chip: [...document.querySelectorAll(".chip-label")].filter((el) => el.textContent === "Gemerkt" && el.getClientRects().length).length }));
  check(said.title.includes("kennt nur dein Browser") && said.rows === 0 && said.chip === 0, `without the app a shared „marked=only" link: ${JSON.stringify(said)}`);
  await plain.close();
}

// ---------- phone ----------
{
  const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };
  const { page, step, context } = await open(phone, "/catalog");
  const ids = (await listIds(page)).slice(0, 6);
  const target = await page.evaluate((id) => { const wrap = onPage(`.row-wrap:has(a.row[data-id="${id}"])`); const [row, mark, lp] = [wrap.querySelector("a.row"), wrap.querySelector(":scope > .mark-toggle"), wrap.querySelector(".lp")].map((el) => el.getBoundingClientRect()); return { size: [mark.width, mark.height], inside: mark.right <= row.right && mark.left >= lp.right, centred: Math.abs((mark.top + mark.bottom) / 2 - (row.top + row.bottom) / 2) }; }, ids[1]);
  check(target.size[0] >= 44 && target.size[1] >= 44 && target.inside && target.centred <= 0.5, `phone: the mark is no target for a finger: ${JSON.stringify(target)}`);
  await step("phone: a tap marks", () => page.tap(`.row-wrap:has(a.row[data-id="${ids[1]}"]) > .mark-toggle >> visible=true`), (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"]) > .mark-toggle`).getAttribute("aria-pressed") === "true", ids[1]);
  check(page.url() === base + "/catalog", `phone: marking opened something: ${page.url()}`);
  await page.tap(`.row-wrap:has(a.row[data-id="${ids[3]}"]) > .mark-toggle >> visible=true`);
  check(await page.evaluate(() => onPage('.bottomnav .nav[data-area="bookmarks"] .nav-count')?.textContent === "2"), "phone: the bottom bar does not count");

  // The note after marking, on a phone: its field opens the menu as a sheet from below, a draft
  // until „Fertig" (let go, nothing of it stays and the note waits); „Fertig" keeps it and ends
  // the note too (owner, 2026-10-19: „Wenn man bei dem Modal auf fertig klickt, dann ist der Prozess
  // done und die Notification kann auch weg").
  const SHELF = "betula.collections.v1";
  const inSheet = async () => { await page.tap(".collect-sheet .collect-field input"); await page.keyboard.type("Für Lena"); await page.keyboard.press("Enter"); };
  await step("phone: the note's field opens the sheet", () => page.tap(".wait-note .mark-toast-field >> visible=true"), () => document.querySelector("dialog.collect-sheet")?.open === true && !document.activeElement?.closest?.(".collect-field"));
  await inSheet();
  await page.waitForFunction(() => document.querySelector(".collect-sheet .collect-item.on")?.textContent.includes("Für Lena"), null, { timeout: 4000 }).catch(() => problems.push("phone: the sheet does not make the collection"));
  check((await page.evaluate((key) => localStorage.getItem(key), SHELF)) === null, "phone: the sheet's draft was kept before „Fertig“");
  await page.evaluate(() => document.querySelector("dialog.collect-sheet")?.close());
  await page.waitForTimeout(400);
  check((await page.evaluate((key) => localStorage.getItem(key), SHELF)) === null && (await page.evaluate(() => Boolean(document.querySelector(".wait-note.mark-note:not(.leaving)")))), "phone: a sheet let go kept something, or took the note along");
  await step("phone: „Fertig“ keeps it and ends the note", async () => { await page.tap(".wait-note .mark-toast-field >> visible=true"); await inSheet(); await page.tap("#collect-done"); }, (key) => (localStorage.getItem(key) || "").includes("\tFür Lena\n") && !document.querySelector("dialog.collect-sheet") && !document.querySelector(".wait-note.mark-note:not(.leaving)"), SHELF);

  await step("phone: the bottom bar leads to the marked modules", () => page.tap('.bottomnav .nav[data-area="bookmarks"] >> visible=true'), () => location.pathname === "/bookmarks" && allOnPage(".rows a.row").length === 2);
  // The collections are chips over the list, and a row names its own.
  check(await page.evaluate(() => [...allOnPage(".coll-chips .coll-chip")].map((chip) => chip.textContent).includes("Für Lena1") && Boolean(onPage(".rows .row-tag"))), "phone: the collections are no chips over the list, or the row does not name its own");
  // The filters and the orders are a sheet behind „Filter" (folia/design/merken/oberflaeche.html),
  // closed until it is opened, and a draft: the list follows only „2 Module anzeigen".
  const layout = await page.evaluate(() => ({ closed: onPage("#sidebar").getBoundingClientRect().top >= innerHeight - 1, toggle: Boolean(onPage(".list-head .sheet-toggle")?.getClientRects().length), overflow: document.documentElement.scrollWidth > innerWidth }));
  check(layout.closed && layout.toggle && !layout.overflow, `phone: the sheet of the filters is not closed behind „Filter", or the page is wider than the screen: ${JSON.stringify(layout)}`);
  // (The page in front's: the catalog's slides away under it for half a second after the tab.)
  await step("phone: „Filter“ opens the sheet", () => page.tap("#content > .page-stage.front .list-head .sheet-toggle"), () => onPage("#sidebar").classList.contains("open") && Boolean(onPage("#sidebar .order-list")?.getClientRects().length));
  await page.waitForTimeout(400);
  await step("phone: an order in the sheet is a draft", () => page.tap('#sidebar .order-list a:has-text("Leistungspunkte") >> visible=true'), () => location.search === "" && onPage('#sidebar .order-list a[aria-current="page"]')?.textContent.startsWith("Leistungspunkte") && onPage("#sidebar .filter-actions .show")?.textContent === "2 Module anzeigen");
  await step("phone: „2 Module anzeigen“ applies it", () => page.tap("#sidebar .filter-actions .show >> visible=true"), () => location.search === "?sort=ects" && !onPage("#sidebar").classList.contains("open"));
  await step("phone: „Filter“ counts the order", () => page.waitForTimeout(400), () => onPage(".list-head .sheet-toggle em")?.textContent === "1");
  await step("phone: the sheet again", () => page.tap(".list-head .sheet-toggle >> visible=true"), () => onPage("#sidebar").classList.contains("open"));
  await page.waitForTimeout(400);
  await step("phone: another order, then the sheet closed with its ×", async () => { await page.tap('#sidebar .order-list a:has-text("Zuletzt gemerkt") >> visible=true'); await page.tap("#sidebar .sheet-close >> visible=true"); }, () => !onPage("#sidebar").classList.contains("open") && location.search === "?sort=ects");
  check(await page.evaluate(() => onPage('#sidebar .order-list a[aria-current="page"]')?.textContent.startsWith("Leistungspunkte")), "phone: a sheet closed with its × kept its draft");
  // The module comes up as a sheet over the list (2026-10-06): the address and the tab stay the
  // list's, and the list stays under it.
  await step("phone: a tap opens the module's sheet over the list", () => page.tap(`a.row[data-id="${ids[1]}"] >> visible=true`), (id) => location.pathname === "/bookmarks" && location.search === `?sort=ects&open=${id}` && Boolean(onPage(".detail.is-module .mark-toggle")) && allOnPage(".rows a.row").length === 2, ids[1]);
  check(await page.evaluate(() => onPage('.bottomnav .nav[data-area="bookmarks"]')?.getAttribute("aria-current") === "page" && !onPage('.bottomnav .nav[data-area="catalog"]').getAttribute("href").startsWith("/catalog/module/")), "phone: the module is not the marked modules', or the catalog's tab heard of it");
  await page.waitForTimeout(500);
  const button = await page.evaluate(() => { const mark = onPage(".detail.is-module .mark-toggle"); const r = mark.getBoundingClientRect(); return { height: r.height, pressed: mark.getAttribute("aria-pressed"), seen: r.top < innerHeight && r.bottom <= innerHeight }; });
  check(button.height >= 44 && button.pressed === "true" && button.seen, `phone: the switch in the module's sheet: ${JSON.stringify(button)}`);
  await step("phone: closing it returns to the marked modules", () => page.tap('.detail.is-module [data-action="close-detail"] >> visible=true'), () => location.pathname === "/bookmarks" && location.search === "?sort=ects" && !onPage(".detail.is-module") && allOnPage(".rows a.row").length === 2);
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
  // A route of the app (docs/folia/folia-refactor.md §4.2): its skeleton is the app's, and without
  // JavaScript the <noscript> says what the list needs.
  check(response.status() === 200 && (await page.evaluate(() => document.querySelector(".state")?.textContent.includes("JavaScript") && !document.querySelector(".app-standin")?.getClientRects().length)), "without JavaScript the page of the list does not explain itself");
  check((await page.evaluate(() => document.querySelector('meta[name="robots"]')?.content ?? "")).startsWith("noindex"), "the page of the list is offered to search engines");
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
