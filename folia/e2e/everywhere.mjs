// Checks the Merkliste from elsewhere (folia/design/merken/konzept.html, „Von überall"; its step 4).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node everywhere.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// The Merkliste's tab, right clicked: a menu of its views, each with how many it holds, which leads
// to one. The catalog's „Gemerkt": its views under the chip with their numbers, the address naming a
// collection by its number alone, the list that collection's modules. „Mein Studium": the
// Wahlpflicht says how many marked modules count in a choice and lists the marked one first, with
// the bookmark; „Module hinzufügen" has three tabs with short names (on a phone side by side), a
// marked module of the plan's tab says so with its collections, „Merkliste" lists the marked modules
// by collection and search, what is ticked in one tab is ticked in the other, „Einfügen" plans them;
// the Gesamtplan's cell lists the area's marked module first. The Stundenplan's „+ Modul" of „Mein
// Studiengang", its Merkliste: what the winter offers each with how it sits in the week, the
// summer's folded with „nur SoSe", a search, a collection, „2 gemerkte Module aufnehmen" (as visits)
// and „Rückgängig", Esc. Nothing of the marks, the names of the collections or what is typed is in a
// request.
// Needs a snapshot whose current semester is WiSe 2026/27. Fails on a page load after takeover, a
// console error, or a step that does not show up.
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
const MINE = "betula.myprogram.v1";
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
// Informatik B.Sc. in its third semester (WiSe 2026/27), the first two filled (as study.mjs has it).
const THIRD = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n";
const FILLED = [
  "m\t2025W\t12104\t1\t\tplan", "m\t2025W\t12107\t1\t\tplan", "m\t2025W\t12102\t1\t\tplan", "m\t2025W\t11112\t1\t\tplan",
  "m\t2026S\t12101\t1\t\tplan", "m\t2026S\t11903\t1\t\tplan", "m\t2026S\t11113\t1\t\tplan",
  "p\t1\t2025W\t079-82-2008\t17\t1-1\t6\tfues\t\tFachübergreifendes Studium",
  "p\t2\t2026S\t079-82-2008\t10\t2-2\t4\telective\t\tProseminar oder Praktikum",
].join("\n") + "\n";
// Names no page of the catalog has, so that a request that carries one is found.
const KANDIDATEN = "Kandidatenkiste";
const LENA = "Für Lenachen";

const current = await (async () => {
  const response = await fetch(base + "/api/status").then((r) => r.json()).catch(() => null);
  return response?.snapshot?.current_semester;
})();
if (current !== "2026W") {
  console.log(JSON.stringify({ skipped: `the snapshot's current semester is ${current}, the checks need 2026W` }));
  await browser.close();
  process.exit(0);
}

const open = async (options) => {
  const context = await browser.newContext(options);
  await context.addInitScript(ON_PAGE);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  const requests = [];
  page.on("request", (r) => requests.push(r.url() + " " + (r.postData() || "")));
  const takeover = () => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  const go = async (path) => {
    await page.goto(base + path, { waitUntil: "domcontentloaded" });
    await takeover();
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
  return { context, page, go, step, requests };
};

// What the test marks: a module of the Wahlpflicht's „Anwendungsfach" (not the first of its list),
// „Theoretische Informatik" of the plan's third semester, and four of the catalog's.
const { context, page, go, step, requests } = await open({ viewport: { width: 1440, height: 900 } });
await go("/catalog?q=Theoretische%20Informatik");
await page.waitForFunction(() => [...allOnPage(".rows a.row")].some((row) => row.querySelector(".t b")?.textContent.trim() === "Theoretische Informatik"), null, { timeout: 20000 }).catch(() => problems.push("the catalog does not find „Theoretische Informatik“"));
const ti = await page.evaluate(() => [...allOnPage(".rows a.row")].find((row) => row.querySelector(".t b")?.textContent.trim() === "Theoretische Informatik")?.dataset.id);
await page.evaluate(([plan, mine, third, filled]) => { localStorage.setItem(mine, third); localStorage.setItem(plan, filled); }, [PLAN, MINE, THIRD, FILLED]);
await go("/study?view=areas");
await page.click('.st-sl:has-text("Anwendungsfach")').catch(() => problems.push("the Wahlpflicht has no „Anwendungsfach“"));
await page.waitForFunction(() => document.querySelectorAll(".st-wtable tbody tr:not(.st-more-row):not(.taken)").length >= 3, null, { timeout: 8000 }).catch(() => problems.push("„Anwendungsfach“ lists fewer than three modules"));
const listed = await page.evaluate(() => [...document.querySelectorAll(".st-wtable tbody tr:not(.st-more-row):not(.taken) td.id")].map((td) => td.textContent.trim()));
const wp = listed[listed.length - 1];
await go("/catalog");
await page.waitForFunction(() => allOnPage(".rows a.row").length >= 8, null, { timeout: 20000 });
const others = await page.evaluate((not) => [...allOnPage(".rows a.row")].map((row) => row.dataset.id).filter((id) => !not.includes(id)).slice(0, 4), [ti, wp, ...listed]);
const marked = [wp, ti, ...others];
const now = Math.floor(Date.now() / 1000);
await page.evaluate(([key, shelf, marked, now, kandidaten, lena]) => {
  localStorage.setItem(key, marked.map((id, i) => `${id}\t${now - i * 60}\n`).join(""));
  // „Kandidaten" (1) holds the two of the study, „Für Lena" (2) one of the catalog's.
  localStorage.setItem(shelf, `c\t1\t${kandidaten}\nc\t2\t${lena}\nm\t${marked[0]}\t1\nm\t${marked[1]}\t1\nm\t${marked[2]}\t2\n`);
}, [KEY, SHELF, marked, now, KANDIDATEN, LENA]);
check(ti && wp && others.length === 4, `the modules to mark: ${JSON.stringify({ ti, wp, others })}`);
requests.length = 0;

// ---------- the Merkliste's tab ----------
await go("/catalog");
const way = (text) => `.tab-menu a.tab-menu-way:has-text("${text}")`;
await step("the tab's menu", () => page.click('.rail a[data-area="bookmarks"]', { button: "right" }), () => document.querySelectorAll(".tab-menu .tab-menu-way").length === 4);
const menu = await page.evaluate(() => [...document.querySelectorAll(".tab-menu .tab-menu-way")].map((a) => `${a.innerText.replace(/\s+/g, " ").trim()} ${a.getAttribute("href")}`));
check(JSON.stringify(menu) === JSON.stringify(["Alle gemerkten 6 /bookmarks", "Ohne Sammlung 3 /bookmarks?collection=none", `${KANDIDATEN} 2 /bookmarks?collection=1`, `${LENA} 1 /bookmarks?collection=2`]), `the tab's menu: ${JSON.stringify(menu)}`);
check(await page.evaluate(() => location.pathname === "/catalog"), "the right click went somewhere");
await step("a collection by the tab", () => page.click(way(KANDIDATEN)), () => location.pathname === "/bookmarks" && location.search === "?collection=1" && allOnPage("#rows a.row").length === 2 && !document.querySelector(".tab-menu"));
check(same(await page.evaluate(() => [...allOnPage("#rows a.row")].map((row) => row.dataset.id)), [wp, ti]), "the collection by the tab lists other modules");
await page.keyboard.press("Escape");

// ---------- the catalog's „Gemerkt" per collection ----------
await go("/catalog?marked=only");
await page.waitForFunction(() => allOnPage(".rows a.row").length === 6 && onPage(".marked-views"), null, { timeout: 20000 }).catch(() => problems.push("„Gemerkt“ does not list the six marked modules with its views"));
const views = await page.evaluate(() => [...onPage(".marked-views").querySelectorAll("a")].map((a) => `${a.innerText.replace(/\s+/g, " ").trim()}${a.getAttribute("aria-checked") === "true" ? "*" : ""}`));
check(JSON.stringify(views) === JSON.stringify(["Alle 6*", "Ohne Sammlung 3", `${KANDIDATEN} 2`, `${LENA} 1`]), `the views of „Gemerkt“: ${JSON.stringify(views)}`);
await step(`„Gemerkt“: ${LENA}`, () => page.click(`.page-stage.front .marked-views a:has-text("${LENA}")`), (id) => new URLSearchParams(location.search).get("collection") === "2" && allOnPage(".rows a.row").length === 1 && onPage(".rows a.row").dataset.id === id, marked[2]);
await step("„Gemerkt“: Ohne Sammlung", () => page.click('.page-stage.front .marked-views a:has-text("Ohne Sammlung")'), () => new URLSearchParams(location.search).get("collection") === "none" && allOnPage(".rows a.row").length === 3);
await step("„Gemerkt“ off", () => page.click('.page-stage.front .marked-views a:has-text("Alle")'), () => !new URLSearchParams(location.search).has("collection") && allOnPage(".rows a.row").length === 6);
// „Ohne gemerkte" about a collection: all but what it holds, the tag naming it.
await go("/catalog?marked=none&collection=2");
await page.waitForFunction((id) => allOnPage(".rows a.row").length > 0 && ![...allOnPage(".rows a.row")].some((row) => row.dataset.id === id), marked[2], { timeout: 20000 }).catch(() => problems.push("„nicht in Für Lena“ lists what the collection holds"));
const without = await page.evaluate((ids) => ({ tag: [...allOnPage(".tag")].map((tag) => tag.textContent.trim()).find((text) => text.startsWith("Merkliste")), others: [...allOnPage(".rows a.row")].filter((row) => ids.includes(row.dataset.id)).length }), [marked[0], marked[1], marked[3]]);
check(without.tag === `Merkliste nicht in ‚${LENA}‘` && without.others >= 1, `„nicht in Für Lena“: ${JSON.stringify(without)}`);

// ---------- „Mein Studium" ----------
// The Wahlpflicht: the choice says how many marked modules count in it; the marked one first, marked.
await go("/study?view=areas");
await step("the Wahlpflicht", () => page.click('.st-sl:has-text("Anwendungsfach")'), () => document.querySelector('.st-sl[aria-current="true"]')?.textContent.includes("Anwendungsfach") && document.querySelector(".st-wtable tbody tr"));
const choice = await page.evaluate(() => ({
  // The area the choice is in: the head over it in the list of the choices.
  area: (() => {
    let name = null;
    for (const el of document.querySelectorAll(".st-split-l > *")) {
      if (el.classList.contains("st-sl-h")) name = el.querySelector("span:not(.st-dot)")?.textContent;
      if (el.classList.contains("st-sl") && el.getAttribute("aria-current") === "true") return name;
    }
    return null;
  })(),
  count: document.querySelector('.st-sl[aria-current="true"] .st-sl-mk')?.textContent.trim(),
  first: document.querySelector(".st-wtable tbody tr:not(.taken) td.id")?.textContent.trim(),
  sign: Boolean(document.querySelector(".st-wtable tbody tr:not(.taken) td.nm .st-mk")),
  signs: document.querySelectorAll(".st-wtable td.nm .st-mk").length,
}));
check(/^(1 gemerktes zählt|\d+ gemerkte zählen) hier$/.test(choice.count || "") && choice.first === wp && choice.sign && choice.signs >= 1 && choice.area, `the Wahlpflicht and the marked module: ${JSON.stringify(choice)} (${wp})`);

// „Module hinzufügen": three tabs; the plan's „Theoretische Informatik" marked, in its collection.
await go("/study");
await step("Module hinzufügen", () => page.click(".st-card-sem .st-empty-add"), () => document.querySelector(".st-dialog[open] .st-choices"));
const tabs = await page.evaluate(() => [...document.querySelectorAll(".st-dialog[open] .st-tabs button")].map((b) => b.textContent + (b.getAttribute("aria-pressed") === "true" ? "*" : "")));
check(tabs.join("|") === "Regelstudienplan*|Merkliste|Katalog", `the tabs of „Module hinzufügen“: ${tabs}`);
const tiRow = await page.evaluate(() => {
  const row = [...document.querySelectorAll(".st-dialog[open] .st-choice-row")].find((li) => li.querySelector(".st-choice-name")?.textContent === "Theoretische Informatik");
  return row && { marked: row.querySelector(".st-marked")?.textContent, on: row.querySelector(".st-tick")?.getAttribute("aria-checked") };
});
check(tiRow?.marked === `gemerkt: ${KANDIDATEN}` && tiRow?.on === "true", `„Theoretische Informatik“ in the plan's tab: ${JSON.stringify(tiRow)}`);
await step("Merkliste", () => page.click('.st-dialog[open] .st-tabs button:has-text("Merkliste")'), () => document.querySelectorAll(".st-dialog[open] .st-choice-row").length === 6);
const chips = await page.evaluate(() => [...document.querySelectorAll(".st-dialog[open] .st-marked-chips button")].map((b) => `${b.firstChild.textContent} ${b.querySelector(".n")?.textContent}${b.getAttribute("aria-pressed") === "true" ? "*" : ""}`));
check(JSON.stringify(chips) === JSON.stringify(["Alle 6*", `${KANDIDATEN} 2`, `${LENA} 1`]), `the collections in „Merkliste“: ${JSON.stringify(chips)}`);
// What is ticked in one tab is ticked in the other.
const tiThere = await page.evaluate(() => [...document.querySelectorAll(".st-dialog[open] .st-choice-row")].find((li) => li.querySelector(".st-choice-name")?.textContent === "Theoretische Informatik")?.querySelector(".st-tick")?.getAttribute("aria-checked"));
check(tiThere === "true", `„Theoretische Informatik“ is not ticked in „Merkliste“ as it is in the plan's tab: ${tiThere}`);
await step(`the collection ${KANDIDATEN}`, () => page.click(`.st-dialog[open] .st-marked-chips button:has-text("${KANDIDATEN}")`), () => document.querySelectorAll(".st-dialog[open] .st-choice-row").length === 2);
const wpTitle = await page.evaluate((ti) => [...document.querySelectorAll(".st-dialog[open] .st-choice-name")].map((name) => name.textContent).find((name) => name !== "Theoretische Informatik"), ti);
const typed = wpTitle.split(/\s+/).find((word) => word.length >= 5) || wpTitle;
await step("the search of „Merkliste“", () => page.fill("#st-marked-search", typed.slice(0, 5)), (title) => { const names = [...document.querySelectorAll(".st-dialog[open] .st-choice-name")].map((name) => name.textContent); return names.includes(title) && !names.includes("Theoretische Informatik"); }, wpTitle);
await step("ticked", () => page.click(`.st-dialog[open] .st-choice-row:has(.st-choice-name:text-is("${wpTitle}")) .st-tick`), (title) => document.querySelector(`.st-dialog[open] .st-choice-row .st-tick[aria-label="${title} auswählen"]`)?.getAttribute("aria-checked") === "true", wpTitle);
await step("nothing found", () => page.fill("#st-marked-search", "zzzqqq"), () => document.querySelector(".st-dialog[open] .st-pick p.st-sub")?.textContent === "Nichts gefunden.");
await page.fill("#st-marked-search", "");
await step("Einfügen", () => page.click("#st-insert"), ([key, wp, ti]) => !document.querySelector(".st-dialog[open]") && (localStorage.getItem(key) || "").includes(`m\t2026W\t${wp}\t`) && (localStorage.getItem(key) || "").includes(`m\t2026W\t${ti}\t`), [PLAN, wp, ti]);

// The Gesamtplan's cell of the area of the module of the Wahlpflicht in SoSe 2027 (it is planned
// into WiSe 2026/27 now, so it is no done one there): the marked module first in its list, marked.
await go("/study");
await step("Gesamtplan", () => page.click('#sidebar .st-vl:has-text("Gesamtplan")'), () => document.querySelector(".st-grid .st-cell-add"));
const found = await page.evaluate((area) => {
  const grid = document.querySelector(".st-grid");
  const column = [...grid.querySelectorAll(":scope > .st-col-head")].findIndex((head) => head.querySelector(".st-col-name")?.textContent.startsWith("SoSe 27"));
  const head = [...grid.querySelectorAll(":scope > .st-row-head")].find((head) => head.querySelector(".st-card-name")?.textContent === area);
  if (column < 0 || !head) return false;
  let cell = head;
  for (let i = 0; i <= column; i++) cell = cell.nextElementSibling;
  cell?.querySelector(".st-cell-add")?.setAttribute("data-check", "cell");
  return Boolean(cell?.querySelector(".st-cell-add"));
}, choice.area);
check(found, `no cell of ${choice.area} in SoSe 27`);
await step("the cell", () => page.click('[data-check="cell"]'), () => document.querySelector(".st-dialog[open] .st-choice-row"));
await page.click('.st-dialog[open] .st-pick > button.st-link[aria-expanded="false"]', { timeout: 1500 }).catch(() => {});
await page.waitForTimeout(300);
const inCell = await page.evaluate((title) => {
  const row = [...document.querySelectorAll(".st-dialog[open] .st-choice-row")].find((li) => li.querySelector(".st-choice-name")?.textContent === title);
  return row && { first: row.parentElement.firstElementChild === row, marked: row.querySelector(".st-marked")?.textContent };
}, wpTitle);
check(inCell?.first && inCell?.marked === `gemerkt: ${KANDIDATEN}`, `the cell does not list the marked module first, marked: ${JSON.stringify(inCell)}`);
await page.keyboard.press("Escape");

const leaks = requests.filter((r) => marked.some((id) => r.includes(id)) || r.includes(encodeURIComponent(KANDIDATEN)) || r.includes(KANDIDATEN) || r.includes("Lenachen") || r.includes(typed.slice(0, 5)) || r.includes("zzzqqq"));
check(leaks.length === 0, `the marks, a collection or the search went into a request: ${leaks.slice(0, 3).join(" | ")}`);
await context.close();

// ---------- the Stundenplan's „+ Modul", its Merkliste ----------
// The first semester's four modules planned into WiSe 2026/27; marked: Analysis I and Theoretische
// Informatik, both of the winter, in „Kandidatenkiste", and Algorithmieren und Programmieren, a
// summer's, in „Für Lenachen". What „+ Modul" of „Mein Studiengang" takes in is a visit (a `v`
// line), never a module of the study (owner, 2026-10-26).
{
  const { context, page, go, step, requests } = await open({ viewport: { width: 1440, height: 900 } });
  await go("/catalog");
  const fs1 = ["12104", "12107", "12102", "11112"].map((id) => `m\t2026W\t${id}\t1790000000\t`).join("\n") + "\n";
  const [analysis, theory, programming] = ["11103", ti, "12101"];
  await page.evaluate(([key, shelf, plan, fs1, ids, now, kandidaten, lena]) => {
    localStorage.setItem(key, ids.map((id, i) => `${id}\t${now - i * 60}\n`).join(""));
    localStorage.setItem(shelf, `c\t1\t${kandidaten}\nc\t2\t${lena}\nm\t${ids[0]}\t1\nm\t${ids[1]}\t1\nm\t${ids[2]}\t2\n`);
    localStorage.setItem(plan, fs1);
  }, [KEY, SHELF, PLAN, fs1, [analysis, theory, programming], now, KANDIDATEN, LENA]);
  requests.length = 0;
  await go("/studyplan");
  await page.waitForSelector(".sp-list .sp-add-mod", { timeout: 20000 }).catch(() => problems.push("„Mein Studiengang“ has no „+ Modul“"));
  const lis = (selector = ".sp-addp-list li") => page.evaluate((selector) => [...document.querySelectorAll(`.sp-addp ${selector}`)].map((li) => ({ id: li.dataset.id, fit: li.querySelector(".sp-fit")?.textContent ?? null, kind: li.querySelector(".sp-fit")?.className ?? null })), selector);
  await step("+ Modul", () => page.click(".sp-list .sp-add-mod"), () => document.querySelector('.sp-addp [role="tab"][aria-selected="true"]')?.textContent === "Merkliste (3)" && document.querySelectorAll(".sp-addp .sp-addp-list li").length === 2 && document.querySelector(".sp-addp-says")?.textContent === "Zählt in „Mein Studium“ als Besuch, ohne LP.");
  const chips = await page.evaluate(() => [...document.querySelectorAll(".sp-addp .sp-marked-chips button")].map((b) => `${b.firstChild.textContent} ${b.querySelector(".n")?.textContent}${b.getAttribute("aria-pressed") === "true" ? "*" : ""}`));
  check(JSON.stringify(chips) === JSON.stringify(["Alle 3*", `${KANDIDATEN} 2`, `${LENA} 1`]), `the collections of „+ Modul“: ${JSON.stringify(chips)}`);
  // How each sits in the week, once the panel is open: „passt", or what it overlaps with.
  await page.waitForFunction((id) => document.querySelector(`.sp-addp li[data-id="${id}"] .sp-fit`), analysis, { timeout: 8000 }).catch(() => {});
  const offered = await lis();
  timings.fits = offered.map((li) => `${li.id}: ${li.fit}`).join(" | ");
  check(same(offered.map((li) => li.id), [analysis, theory]), `what the winter offers: ${JSON.stringify(offered)}`);
  check(offered.some((li) => /sp-fit (ok|warn)/.test(li.kind || "") && /^(passt|Überschneidet sich mit: .+)$/.test(li.fit || "")), `no module says how it sits in the week: ${JSON.stringify(offered)}`);
  // What the winter does not offer, folded, with why.
  const fold = await page.evaluate(() => document.querySelector(".sp-addp .sp-marked-fold")?.textContent);
  check(fold === "Im WiSe 2026/27 nicht angeboten · 1", `the fold: ${fold}`);
  await step("what the winter does not offer", () => page.click(".sp-addp .sp-marked-fold"), (id) => document.querySelector(`.sp-addp li[data-id="${id}"] .sp-fit`)?.textContent === "nur SoSe", programming);
  // The search, then a collection, then all of it at once.
  await step("the search of the Merkliste", () => page.fill(".sp-addp .sp-marked-search input", "theoret"), (id) => [...document.querySelectorAll(".sp-addp li")].map((li) => li.dataset.id).join() === id, theory);
  await page.fill(".sp-addp .sp-marked-search input", "");
  await step(`the collection ${KANDIDATEN}`, () => page.click(`.sp-addp .sp-marked-chips button:has-text("${KANDIDATEN}")`), () => document.querySelectorAll(".sp-addp li").length === 2 && !document.querySelector(".sp-addp .sp-marked-fold"));
  check((await page.evaluate(() => document.querySelector(".sp-addp .sp-take")?.textContent)) === "2 gemerkte Module aufnehmen", "no „2 gemerkte Module aufnehmen“ for the collection");
  await step("aufnehmen", () => page.click(".sp-addp .sp-take"), ([key, ids]) => {
    const text = localStorage.getItem(key) || "";
    return ids.every((id) => text.includes(`v\t2026W\t${id}\t`) && !text.includes(`m\t2026W\t${id}\t`))
      && document.querySelector(".sp-addp .sp-marked-note")?.textContent.includes("2 gemerkte Module in „Mein Studiengang“ aufgenommen")
      && document.querySelector('.sp-addp [role="tab"][aria-selected="true"]')?.textContent === "Merkliste (1)"
      && document.querySelectorAll(".sp-list .sp-row.visit").length === 2;
  }, [PLAN, [analysis, theory]]);
  await step("Rückgängig the take", () => page.click(".sp-addp .sp-marked-note .mini"), ([key, ids]) => ids.every((id) => !(localStorage.getItem(key) || "").includes(`\t${id}\t`)) && document.querySelector('.sp-addp [role="tab"][aria-selected="true"]')?.textContent === "Merkliste (3)" && !document.querySelector(".sp-list .sp-row.visit"), [PLAN, [analysis, theory]]);
  await step("Esc closes „+ Modul“", async () => { await page.focus(".sp-addp .sp-marked-search input"); await page.keyboard.press("Escape"); }, () => !document.querySelector(".sp-addp") && document.querySelector(".sp-list .sp-add-mod")?.getAttribute("aria-expanded") === "false");
  const leaks = requests.filter((r) => [analysis, theory, programming].some((id) => r.includes(id)) || r.includes(KANDIDATEN) || r.includes(encodeURIComponent(KANDIDATEN)) || r.includes("Lenachen") || r.includes("theoret"));
  check(leaks.length === 0, `the Stundenplan's Merkliste went into a request: ${leaks.slice(0, 3).join(" | ")}`);
  await context.close();
}

// ---------- a phone: the three tabs side by side ----------
{
  const { context, page, go, step } = await open({ viewport: { width: 412, height: 915 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2.625 });
  await go("/catalog");
  await page.evaluate(([key, plan, mine, third, filled, ids, now]) => {
    localStorage.setItem(key, ids.map((id, i) => `${id}\t${now - i * 60}\n`).join(""));
    localStorage.setItem(mine, third);
    localStorage.setItem(plan, filled);
  }, [KEY, PLAN, MINE, THIRD, FILLED, marked, now]);
  await go("/study");
  await page.waitForTimeout(600);
  await step("Module hinzufügen on a phone", () => page.tap(".st-page.is-current .st-empty-add, .st-empty-add >> visible=true"), () => document.querySelector(".st-dialog[open] .st-tabs"));
  const line = await page.evaluate(() => {
    const tabs = document.querySelector(".st-dialog[open] .st-tabs");
    const buttons = [...tabs.querySelectorAll("button")].map((b) => b.getBoundingClientRect());
    return { n: buttons.length, tops: new Set(buttons.map((b) => Math.round(b.top))).size, inside: tabs.getBoundingClientRect().right <= document.querySelector(".st-dialog[open]").getBoundingClientRect().right + 0.5, wraps: [...tabs.querySelectorAll("button")].some((b) => b.scrollWidth > b.clientWidth + 1) };
  });
  check(line.n === 3 && line.tops === 1 && line.inside && !line.wraps, `the three tabs on a phone: ${JSON.stringify(line)}`);
  await step("Merkliste on a phone", () => page.tap('.st-dialog[open] .st-tabs button:has-text("Merkliste")'), () => document.querySelectorAll(".st-dialog[open] .st-choice-row").length === 6);
  await context.close();
}

function same(a, b) {
  return JSON.stringify([...a].sort()) === JSON.stringify([...b].sort());
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
