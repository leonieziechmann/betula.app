// Checks several modules at once on the Merkliste (folia/design/merken/oberflaeche.html, „Mehrere
// auswählen"; folia/crates/bookmarks/src/pick.rs), and the keys „Mein Studium" got with it.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node selection.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Desktop: Ctrl and a click, Shift and a click, a click while rows are selected, Esc; a mouse drawn
// over the rows; Ctrl A; Shift and ↑ ↓ with the focus following; the bar in the place of the heads
// of the columns, as tall, with the number and the credits; S with rows selected opens the menu of
// their collections (no note), one made there holds all of them, the dash where some are;
// „Einplanen" each where its own switch aims and all into one semester, with the note and its
// „Rückgängig"; „Teilen" copies the link of these modules; „Entfernen" dims them, „Rückgängig" brings
// them back; a row's right click and its menu; nothing selected is ever in a request or the address.
// „Mein Studium": Shift and ↑ ↓, Ctrl A.
// Phone: a finger held still on a card selects it, drawn on the next, the bar over the tab bar, a
// tap selects or lets go and opens nothing, a swipe stays a swipe, „Einplanen" is a sheet.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";
import { fileURLToPath } from "node:url";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
const KEY = "betula.bookmarks.v1";
const PLAN = "betula.studyplan.v1";
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };

const open = async (options, requests) => {
  const context = await browser.newContext({ ...options, permissions: ["clipboard-read", "clipboard-write"] });
  await context.addInitScript(ON_PAGE);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  if (requests) page.on("request", (r) => requests.push(r.url() + " " + (r.postData() || "")));
  const takeover = () => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  // Six modules of the catalog, marked a minute apart (the newest first), and the Merkliste.
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await takeover();
  await page.waitForFunction(() => allOnPage(".rows a.row").length >= 6, null, { timeout: 20000 });
  const ids = await page.evaluate(() => [...allOnPage(".rows a.row")].map((row) => row.dataset.id).slice(0, 6));
  await page.evaluate(([key, plan, ids, now]) => {
    localStorage.setItem(key, ids.map((id, i) => `${id}\t${now - i * 60}\n`).join(""));
    localStorage.removeItem(plan);
    localStorage.removeItem("betula.collections.v1");
  }, [KEY, PLAN, ids, Math.floor(Date.now() / 1000)]);
  await page.goto(base + "/bookmarks", { waitUntil: "domcontentloaded" });
  await takeover();
  await page.waitForFunction((n) => allOnPage("#rows a.row").length >= n, ids.length, { timeout: 20000 }).catch(() => problems.push("the Merkliste does not list the modules marked"));
  await page.waitForTimeout(400);
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
  const rows = await page.evaluate(() => [...allOnPage("#rows a.row")].map((row) => row.dataset.id));
  return { context, page, step, rows };
};
const selected = (page) => page.evaluate(() => [...allOnPage("#rows .row-wrap.is-selected a.row")].map((row) => row.dataset.id));
const same = (a, b) => JSON.stringify([...a].sort()) === JSON.stringify([...b].sort());
const row = (rows, i) => `.page-stage.front #rows a.row[data-id="${rows[i]}"]`;
const marks = (page) => page.evaluate((key) => (localStorage.getItem(key) || "").split("\n").filter(Boolean).map((line) => line.split("\t")[0]), KEY);
const planned = (page) => page.evaluate((key) => (localStorage.getItem(key) || "").split("\n").filter((line) => line.startsWith("m\t")).map((line) => line.split("\t").slice(1, 3).join(" ")), PLAN);

// ---------- desktop ----------
{
  const requests = [];
  const { context, page, step, rows } = await open({ viewport: { width: 1500, height: 900 } }, requests);
  check(rows.length === 6, `the Merkliste lists ${rows.length} modules`);
  const heads = await page.evaluate(() => onPage(".list > .cols")?.getBoundingClientRect().height);

  // Ctrl and a click selects, nothing opens; the bar takes the place of the heads, as tall.
  await step("Ctrl and a click selects a row", () => page.click(row(rows, 0), { modifiers: ["Control"] }), (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`)?.classList.contains("is-selected"), rows[0]);
  check(!page.url().includes("open="), `Ctrl and a click opened the module: ${page.url()}`);
  const bar = await page.evaluate(() => { const bar = onPage(".list > .pick-bar"); return bar && { height: bar.getBoundingClientRect().height, cols: !!onPage(".list > .cols"), text: bar.innerText.replace(/\s+/g, " ").trim() }; });
  check(bar && !bar.cols && Math.abs(bar.height - heads) < 1, `the bar is not in the place of the heads, as tall: ${JSON.stringify(bar)} (heads ${heads})`);
  check(/^1 ausgewählt · \d+ LP\b/.test(bar?.text || ""), `the bar does not say how many and their credits: ${bar?.text}`);
  await step("Shift and a click takes in the rows up to it", () => page.click(row(rows, 2), { modifiers: ["Shift"] }), (ids) => [...allOnPage("#rows .row-wrap.is-selected a.row")].length === 3 && ids.every((id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`).classList.contains("is-selected")), rows.slice(0, 3));
  await step("a click while rows are selected selects", () => page.click(row(rows, 4)), (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`)?.classList.contains("is-selected"), rows[4]);
  check(!page.url().includes("open="), `a click while selecting opened the module: ${page.url()}`);
  await step("Ctrl and a click lets go", () => page.click(row(rows, 1), { modifiers: ["Control"] }), (id) => !onPage(`.row-wrap:has(a.row[data-id="${id}"])`)?.classList.contains("is-selected"), rows[1]);
  check(same(await selected(page), [rows[0], rows[2], rows[4]]), `selected: ${await selected(page)}`);
  // The look: tinted, hatched, lined.
  const look = await page.evaluate((id) => { const style = getComputedStyle(onPage(`.row-wrap:has(a.row[data-id="${id}"]) > a.row`)); return { image: style.backgroundImage, shadow: style.boxShadow }; }, rows[0]);
  check(look.image.includes("repeating-linear-gradient") && look.shadow.includes("inset"), `a selected row is not hatched and lined: ${JSON.stringify(look)}`);
  await step("Esc lets go", () => page.keyboard.press("Escape"), () => !allOnPage("#rows .row-wrap.is-selected").length && !onPage(".pick-bar") && !!onPage(".list > .cols"));

  // A mouse pressed on a row and drawn onto others selects them; the click it ends with opens nothing.
  const box = async (i) => page.locator(row(rows, i)).boundingBox();
  const [b1, b3] = [await box(1), await box(3)];
  await step("a mouse drawn over the rows selects them", async () => {
    await page.mouse.move(b1.x + 80, b1.y + b1.height / 2);
    await page.mouse.down();
    await page.mouse.move(b1.x + 80, b1.y + b1.height / 2 + 24, { steps: 4 });
    await page.mouse.move(b3.x + 80, b3.y + b3.height / 2, { steps: 6 });
    await page.mouse.up();
  }, (ids) => JSON.stringify([...allOnPage("#rows .row-wrap.is-selected a.row")].map((row) => row.dataset.id).sort()) === JSON.stringify([...ids].sort()), rows.slice(1, 4));
  await page.waitForTimeout(300);
  check(!page.url().includes("open="), `drawing opened a module: ${page.url()}`);
  check(await page.evaluate(() => !String(getSelection()).trim()), "drawing over the rows selected words");
  await page.keyboard.press("Escape");

  // Ctrl A: every row the list shows.
  await page.locator(row(rows, 0)).focus();
  await step("Ctrl A selects every row", () => page.keyboard.press("Control+a"), (n) => allOnPage("#rows .row-wrap.is-selected").length === n, rows.length);
  check(/^6 ausgewählt/.test(await page.evaluate(() => onPage(".pick-bar")?.innerText.trim())), "the bar does not count all of them");
  check(await page.evaluate(() => onPage("#pick-all")?.disabled === true), "„Alle auswählen\" is not done with all of them selected");
  await page.keyboard.press("Escape");
  // In the search field Ctrl A is the field's.
  await page.fill("#topsearch", "x");
  await page.focus("#topsearch");
  await page.keyboard.press("Control+a");
  check((await selected(page)).length === 0, "Ctrl A in the search field selected rows");
  await page.fill("#topsearch", "");
  await page.waitForTimeout(600);

  // Shift and the arrows: taken in, the focus along; back, the last let go.
  await page.locator(row(rows, 1)).focus();
  await step("Shift and ↓ take in the next rows", async () => { await page.keyboard.press("Shift+ArrowDown"); await page.keyboard.press("Shift+ArrowDown"); }, (ids) => JSON.stringify([...allOnPage("#rows .row-wrap.is-selected a.row")].map((row) => row.dataset.id).sort()) === JSON.stringify([...ids].sort()), rows.slice(1, 4));
  check(await page.evaluate((id) => document.activeElement?.dataset?.id === id, rows[3]), "the focus did not go along with Shift and ↓");
  await step("Shift and ↑ back lets go of the last", () => page.keyboard.press("Shift+ArrowUp"), (ids) => JSON.stringify([...allOnPage("#rows .row-wrap.is-selected a.row")].map((row) => row.dataset.id).sort()) === JSON.stringify([...ids].sort()), rows.slice(1, 3));

  // S with rows selected: the menu of their collections, no note; a collection made there holds
  // all of them.
  await step("S opens the menu for the rows selected", () => page.keyboard.press("s"), () => !!document.querySelector(".collect-pop .collect-field input"));
  check((await page.evaluate(() => document.querySelector(".collect-pop .collect-head")?.textContent)) === "2 Module", "the menu's head does not say how many");
  check(await page.evaluate(() => !document.querySelector(".collect-pop textarea, .collect-pop .collect-default")), "the menu for several has a note or a default");
  await page.keyboard.type("Wahlpflicht");
  await step("a collection made there holds all of them", () => page.keyboard.press("Enter"), (ids) => ids.every((id) => [...onPage(`.row-wrap:has(a.row[data-id="${id}"])`).querySelectorAll(".row-tag")].some((tag) => tag.textContent === "Wahlpflicht")), rows.slice(1, 3));
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
  check(same(await selected(page), rows.slice(1, 3)), "the menu let go of the rows selected");
  // One more selected: the dash where some are, a tap on it takes in the rest.
  await page.click(row(rows, 3), { modifiers: ["Control"] });
  await page.click(".page-stage.front #pick-collect");
  await page.waitForSelector(".collect-pop .collect-item");
  check((await page.evaluate(() => document.querySelector(".collect-pop .collect-item")?.getAttribute("aria-checked"))) === "mixed", "no dash where some of them are in a collection");
  await step("a tap on the dash takes in the rest", () => page.click(".collect-pop .collect-item"), (id) => [...onPage(`.row-wrap:has(a.row[data-id="${id}"])`).querySelectorAll(".row-tag")].some((tag) => tag.textContent === "Wahlpflicht"), rows[3]);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");

  // „Einplanen": each where its own switch aims, the note says where, „Rückgängig" takes all back.
  await page.click(row(rows, 0), { modifiers: ["Control"] });
  await page.click(row(rows, 1), { modifiers: ["Control"] });
  await page.click(".page-stage.front #pick-plan");
  await page.waitForSelector(".pick-menu .st-pop-item");
  const menu = await page.evaluate(() => [...document.querySelectorAll(".pick-menu .st-pop-item, .pick-menu .pick-menu-label")].map((item) => item.innerText.replace(/\s+/g, " ").trim()));
  check(/^Jedes in sein Semester/.test(menu[0] || "") && (menu.includes("ALLE IN") || menu.includes("Alle in")), `„Einplanen" does not offer each where it aims and all into one: ${JSON.stringify(menu)}`);
  check(menu.some((item) => /jetzt/.test(item)) && menu.some((item) => /angeboten/.test(item)), `the semesters do not say „jetzt" and whether they are offered: ${JSON.stringify(menu)}`);
  await step("„Jedes in sein Semester\" plans them", () => page.click(".pick-menu .st-pop-item >> nth=0"), () => /eingeplant/.test(document.querySelector(".mark-note")?.textContent || ""));
  await page.waitForTimeout(300);
  check((await planned(page)).length === 2, `not both planned: ${await planned(page)}`);
  check((await selected(page)).length === 0, "the rows stay selected after „Einplanen\"");
  await step("its „Rückgängig\" takes all of them back", () => page.click(".mark-note .plan-toast-undo"), () => !(localStorage.getItem("betula.studyplan.v1") || "").includes("\nm\t") && !(localStorage.getItem("betula.studyplan.v1") || "").startsWith("m\t"));
  // All into one semester: the second of „Alle in".
  await page.click(row(rows, 2), { modifiers: ["Control"] });
  await page.click(row(rows, 3), { modifiers: ["Control"] });
  await page.click(".page-stage.front #pick-plan");
  await page.waitForSelector(".pick-menu .st-pop-item");
  const target = await page.evaluate(() => document.querySelectorAll(".pick-menu .st-pop-item")[2]?.querySelector(".st-pop-label > span")?.firstChild?.textContent.trim());
  await step("all into one semester", () => page.click(".pick-menu .st-pop-item >> nth=2"), () => /2 eingeplant/.test(document.querySelector(".mark-note")?.textContent || ""));
  await page.waitForTimeout(300);
  const into = await planned(page);
  check(into.length === 2 && new Set(into.map((line) => line.split(" ")[0])).size === 1, `not both into ${target}: ${into}`);

  // „Teilen": the link of these modules, copied.
  await page.click(row(rows, 0), { modifiers: ["Control"] });
  await page.click(row(rows, 5), { modifiers: ["Control"] });
  await page.click(".page-stage.front #pick-share");
  await page.waitForSelector(".pick-menu .st-pop-item");
  await step("„Teilen\" copies the link", () => page.click(".pick-menu .st-pop-item >> nth=0"), () => /Link kopiert/.test(document.querySelector(".mark-note")?.textContent || ""));
  const link = await page.evaluate(() => navigator.clipboard.readText()).catch(() => "");
  check(/\/bookmarks#m=/.test(link), `the link copied is no link of the Merkliste: ${link.slice(0, 80)}`);

  // „Entfernen": dimmed, „Rückgängig" brings them back.
  await page.click(row(rows, 4), { modifiers: ["Control"] });
  await page.click(row(rows, 5), { modifiers: ["Control"] });
  await step("„Entfernen\" takes their marks away", () => page.click(".page-stage.front #pick-remove"), (ids) => ids.every((id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`).classList.contains("unmarked")) && /2 Module entfernt/.test(document.querySelector(".mark-note")?.textContent || ""), rows.slice(4, 6));
  check((await marks(page)).length === 4, `marks after „Entfernen": ${await marks(page)}`);
  await step("its „Rückgängig\" brings them back", () => page.click(".mark-note .plan-toast-undo"), () => !allOnPage("#rows .row-wrap.unmarked").length);
  check((await marks(page)).length === 6, `marks after „Rückgängig": ${await marks(page)}`);

  // A row's right click: its menu, „Auswählen" selects it.
  await step("a right click opens the row's menu", () => page.click(row(rows, 3), { button: "right" }), () => document.querySelectorAll(".pick-menu .st-pop-item").length === 4);
  const entries = await page.evaluate(() => [...document.querySelectorAll(".pick-menu .st-pop-item")].map((item) => item.innerText.replace(/\s+/g, " ").trim()));
  check(/^Sammlung und Notiz/.test(entries[0]) && /^Auswählen\b/.test(entries[1]) && /^(Einplanen|Aus dem Plan nehmen)/.test(entries[2]) && /^Von der Merkliste entfernen/.test(entries[3]), `the row's menu: ${JSON.stringify(entries)}`);
  await step("„Auswählen\" in it selects the row", () => page.click(".pick-menu .st-pop-item >> nth=1"), (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`)?.classList.contains("is-selected"), rows[3]);
  await page.keyboard.press("Escape");

  // Nothing of it leaves the browser: no request carries the marks or what is selected, none goes
  // elsewhere, and the address never says what is selected.
  const leaked = requests.filter((r) => (r.includes("bookmark") && !r.includes("/bookmarks")) || /[?&](ids|marks|marked|sel|selected)=/.test(r));
  check(leaked.length === 0, `requests that carry marks or the selection: ${leaked.slice(0, 3)}`);
  const beyond = requests.filter((r) => !r.startsWith(base));
  check(beyond.length === 0, `requests to somewhere else: ${beyond.slice(0, 3)}`);
  check(!/[?&](sel|selected|ids)=/.test(page.url()), `the address says what is selected: ${page.url()}`);
  await context.close();
}

// ---------- „Mein Studium": Shift and the arrows, Ctrl A ----------
{
  const context = await browser.newContext({ viewport: { width: 1500, height: 900 } });
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console (study): " + m.text().slice(0, 300)); });
  await page.goto(base + "/study", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  // A study begun in the winter 2025; the semester in view is the current one, whose key its box
  // carries (`st-sem-<key>`). Four modules planned there.
  await page.evaluate(() => localStorage.setItem("betula.myprogram.v1", "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n"));
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
  await page.waitForFunction(() => !!document.querySelector(".page-stage.front .st-card-sem[aria-labelledby]"), null, { timeout: 20000 }).catch(() => problems.push("„Mein Studium\" shows no semester"));
  const now = await page.evaluate(() => document.querySelector(".page-stage.front .st-card-sem")?.getAttribute("aria-labelledby")?.replace("st-sem-", ""));
  await page.evaluate(([plan, now]) => localStorage.setItem(plan, ["12104", "12107", "12102", "11112"].map((id) => `m\t${now}\t${id}\t1\t`).join("\n") + "\n"), [PLAN, now]);
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
  await page.waitForFunction(() => document.querySelectorAll(".page-stage.front .st-card-sem .st-row .st-open").length >= 4, null, { timeout: 20000 }).catch(() => problems.push("„Mein Studium\" does not show the semester's modules"));
  const keys = () => page.evaluate(() => [...document.querySelectorAll(".page-stage.front .st-card-sem .st-row")].filter((row) => row.classList.contains("is-selected")).map((row) => row.dataset.key));
  const order = await page.evaluate(() => [...document.querySelectorAll(".page-stage.front .st-card-sem .st-row")].map((row) => row.dataset.key));
  await page.locator(".page-stage.front .st-card-sem .st-row .st-open").first().focus();
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");
  check(same(await keys(), order.slice(0, 3)), `„Mein Studium": Shift and ↓ twice selected ${await keys()}`);
  check(await page.evaluate((key) => document.activeElement?.closest(".st-row")?.dataset.key === key, order[2]), "„Mein Studium\": the focus did not go along");
  await page.keyboard.press("Shift+ArrowUp");
  check(same(await keys(), order.slice(0, 2)), `„Mein Studium": Shift and ↑ selected ${await keys()}`);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+a");
  await page.waitForTimeout(200);
  check(same(await keys(), order), `„Mein Studium": Ctrl A selected ${await keys()}`);
  await context.close();
}

// ---------- phone ----------
{
  const { context, page, step, rows } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 });
  const cdp = await context.newCDPSession(page);
  const touch = (type, x, y) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x, y, id: 1 }] });
  const centre = async (i) => { const b = await page.locator(row(rows, i)).boundingBox(); return [b.x + b.width / 2 - 40, b.y + b.height / 2]; };
  // A finger held still on a card selects it; drawn on, the next.
  let [x, y] = await centre(0);
  await step("a finger held still selects the card", async () => { await touch("touchStart", x, y); await page.waitForTimeout(650); }, (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`)?.classList.contains("is-selected"), rows[0]);
  const [, y1] = await centre(1);
  await step("drawn on, the next card", async () => {
    for (let k = 1; k <= 8; k++) { await touch("touchMove", x, y + ((y1 - y) * k) / 8); await page.waitForTimeout(30); }
    await touch("touchEnd", x, y1);
  }, (ids) => ids.every((id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`)?.classList.contains("is-selected")), rows.slice(0, 2));
  await page.waitForTimeout(400);
  check(page.url() === base + "/bookmarks", `the hold opened something: ${page.url()}`);
  const bar = await page.evaluate(() => { const bar = onPage(".pick-bar"); if (!bar) return null; const r = bar.getBoundingClientRect(); return { position: getComputedStyle(bar).position, bottom: innerHeight - r.bottom, text: bar.innerText.replace(/\s+/g, " ").trim() }; });
  check(bar?.position === "fixed" && bar.bottom >= 0 && bar.bottom < 30 && /^2\b/.test(bar.text), `the bar is not over the tab bar: ${JSON.stringify(bar)}`);
  // A tap selects or lets go, and opens nothing.
  [x, y] = await centre(3);
  await step("a tap while selecting selects", () => page.touchscreen.tap(x, y), (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`)?.classList.contains("is-selected"), rows[3]);
  [x, y] = await centre(0);
  await step("a tap lets go", () => page.touchscreen.tap(x, y), (id) => !onPage(`.row-wrap:has(a.row[data-id="${id}"])`)?.classList.contains("is-selected"), rows[0]);
  check(page.url() === base + "/bookmarks", `a tap while selecting opened the module: ${page.url()}`);
  // A swipe stays a swipe.
  [x, y] = await centre(4);
  await step("a swipe to the left still takes the mark away", async () => {
    await touch("touchStart", x + 60, y);
    for (let k = 1; k <= 10; k++) { await touch("touchMove", x + 60 - 22 * k, y + 1); await page.waitForTimeout(16); }
    await touch("touchEnd", x - 160, y + 1);
  }, (id) => onPage(`.row-wrap:has(a.row[data-id="${id}"])`)?.classList.contains("unmarked"), rows[4]);
  check(!(await selected(page)).includes(rows[4]), "the swipe selected its card");
  // „Einplanen" is a sheet from below.
  await page.waitForTimeout(500);
  await step("„Einplanen\" is a sheet", () => page.tap(".page-stage.front #pick-plan"), () => { const menu = document.querySelector(".pick-menu"); return !!menu && Math.abs(menu.getBoundingClientRect().bottom - innerHeight) < 2; });
  await page.tap(".st-pop-shade");
  await page.waitForTimeout(300);
  // The list's ⋯ has „Alle auswählen", which the phone's bar has no room for: every marked card.
  await page.tap(".page-stage.front .pick-bar .st-act-close");
  await page.waitForTimeout(300);
  await page.tap(".page-stage.front .view-more");
  await page.waitForSelector(".view-menu .st-pop-item");
  check((await page.evaluate(() => document.querySelector(".view-menu .st-pop-item")?.textContent.trim())) === "Alle auswählen", "the list's ⋯ on a phone has no „Alle auswählen\" first");
  await step("„Alle auswählen\" in the list's ⋯", () => page.tap(".view-menu .st-pop-item >> nth=0"), (n) => allOnPage("#rows .row-wrap.is-selected").length === n, rows.length - 1);
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
