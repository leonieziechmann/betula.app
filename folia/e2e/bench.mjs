// Checks the workbench of „Mein Studium" on a desktop (folia/crates/planner/src/study/bench.rs;
// folia/design/hinzufuegen/rechner.html, variant B; owner, 2026-11-09: „Ich glaube ich mag option B
// am besten"): the semester's card opens into what is taken and what can be added, under its one
// head. On Informatik B.Sc. in its third semester, the first two passed:
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node bench.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// 1 „Module hinzufügen" opens the card of WiSe 2026/27: the page stays (the sidebar, the overview,
//   the semester's head with ‹ ›), the head says what the semester holds over the bar of it
//   instead of „0 LP · Plan 30", the rows stay on the left (none yet), the right half has
//   „Hinzufügen" and × over „Laut Plan im 3. FS": a module of the plan with its credits as its
//   button, a choice dashed with its bar, „Aus dem 4. FS vorziehen", the ways.
// 2 „Plan übernehmen" plans the three Pflichtmodule of the 3. FS at once into the rows on the left,
//   the bar in the head counts them, the note says so with „Rückgängig".
// 3 The Anwendungsfach's card goes into its page: ‹ and „Übersicht ›" over it, its name, figure
//   and bar, the list in groups by the week, „Fach" with its five lists; „Passt in meine Woche"
//   keeps what fits, „Im WiSe angeboten" off shows what the winter does not offer.
// 4 „+ Einplanen" plans a module at once: it counts for the Anwendungsfach (its row on the left
//   says „für Anwendungsfach", the page's figure counts it), the store remembers the row.
// 5 A module both choices list (Mathematische Statistik), planned from Praktische Mathematik,
//   counts there; its row's menu, beside the open half, has „Zählt für ›": the Anwendungsfach;
//   „Anderer Bereich …" for Mathematik's Pflichtmodule as an exception, marked „Ausnahme", and
//   „Automatisch" back.
// 6 „Alle Bereiche" is a page of the tree, the FÜS its row: a module of it opens beside the page
//   over the half, its „Einplanen" aims at the semester for the FÜS; Esc closes the module, the
//   half stays.
// 7 „Im Katalog suchen ›": the catalog with the FÜS list and the winter, the workbench's bar at the
//   head of its list; a module planned from there counts for the FÜS; „Zurück zu WiSe 2026/27"
//   leads back to the half as it was.
// 8 › in the head turns the semester and the half with it (SoSe 2027, „Laut Plan im 4. FS"), ‹ back.
// 9 Esc takes one step back, then ends the mode; the semester holds what was planned. The
//   Gesamtplan's „+" opens the card in the semesters' view at its area's subgroup; another view
//   ends it. Nothing of it in a request.
// Needs a snapshot whose current semester is WiSe 2026/27. Fails on a page load after takeover, a
// console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };
let during = "";
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
const THIRD = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n";
const W = ["12104", "12107", "12102", "11112"];
const S = ["12101", "11903", "11113", "12111"];
const PASSED = [...W.map((id) => `m\t2025W\t${id}\t1\t\tplan`), ...S.map((id) => `m\t2026S\t${id}\t1\t\tplan`), ...W.map((id) => `d\t2025W\t${id}`), ...S.map((id) => `d\t2026S\t${id}`)].join("\n") + "\n";

const current = await fetch(base + "/api/status").then((r) => r.json()).then((status) => status?.snapshot?.current_semester).catch(() => null);
if (current !== "2026W") {
  console.log(JSON.stringify({ skipped: `the snapshot's current semester is ${current}, the checks need 2026W` }));
  await browser.close();
  process.exit(0);
}

const requests = [];
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, locale: "de-DE" });
const page = await context.newPage();
page.on("console", (m) => { if (m.type() === "error") problems.push(`console (${during}): ` + m.text().slice(0, 300)); });
page.on("pageerror", (e) => problems.push(`pageerror (${during}): ` + String(e).slice(0, 300)));
page.on("request", (r) => requests.push(r.url() + " " + (r.postData() || "")));
const takeover = () => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
await page.goto(base + "/", { waitUntil: "domcontentloaded" });
await takeover();
await page.evaluate((entries) => { for (const [key, text] of entries) localStorage.setItem(key, text); }, [[MINE, THIRD], [PLAN, PASSED]]);
await page.goto(base + "/study", { waitUntil: "domcontentloaded" });
await takeover();
await page.evaluate(() => { window.__marker = 1; });
const step = async (name, action, until, arg) => {
  const started = Date.now();
  during = name;
  await action();
  try {
    await page.waitForFunction(until, arg, { timeout: 8000 });
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
  }
  timings[name] = Date.now() - started;
  if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
};
const lines = async (prefix) => ((await page.evaluate((key) => localStorage.getItem(key), PLAN)) || "").split("\n").filter((line) => line.startsWith(prefix));
// What the half shows: the page's name in its path (none at the root), its groups, its rows.
const half = () => page.evaluate(() => ({
  title: document.querySelector(".st-wb-half .st-wb-pt > b")?.textContent ?? null,
  top: document.querySelector(".st-wb-half .st-wb-half-t")?.textContent ?? null,
  crumbs: [...document.querySelectorAll(".st-wb-half .st-wb-crumbs > li")].map((crumb) => crumb.textContent).join(" › "),
  groups: [...document.querySelectorAll(".st-wb-half .st-wb-g")].map((g) => g.firstChild?.textContent.trim()),
  rows: [...document.querySelectorAll(".st-wb-half .st-wb-row")].map((row) => ({ name: row.querySelector(".st-wb-go b")?.textContent, act: row.querySelector(".st-wb-plus")?.textContent.trim(), choice: row.classList.contains("choice") })),
}));
// The semester's rows on the left: the name, and what it counts for where that is a choice.
const semester = () => page.evaluate(() => [...document.querySelectorAll(".st-card-sem .st-row")].map((row) => `${row.querySelector(".st-name")?.textContent}|${row.querySelector(".st-for")?.textContent ?? ""}`));
const pageFigure = () => page.evaluate(() => document.querySelector(".st-wb-half .st-wb-pt > .st-wb-fig")?.textContent);

// 1: the card opens.
await step("to now", () => page.click(".st-card-sem .st-sem-pick").then(() => page.click('.st-pop.root [role=menuitem]:has-text("WiSe 26/27")')), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
await step("Module hinzufügen", () => page.click(".st-card-sem .st-empty-add"), () => document.querySelector(".st-card-sem.split > .st-wb-half #st-wb-takeover") && document.querySelector(".st-card-sem .st-sem-head .st-sem-bar"));
// Measured once the half has come in (it slides in from the right).
await page.waitForFunction(() => document.querySelector(".st-wb-half")?.getAnimations().length === 0, null, { timeout: 3000 }).catch(() => problems.push("the half never stands still"));
const shape = await page.evaluate(() => {
  const rect = (selector) => document.querySelector(selector)?.getBoundingClientRect();
  const [head, table, sheet, card, over] = [".st-card-sem > .st-sem-head", ".st-card-sem > .st-table", ".st-card-sem > .st-wb-half", ".st-card-sem", ".st-over"].map(rect);
  return {
    under: Boolean(head && table && sheet) && table.top >= head.bottom - 1 && sheet.top >= head.bottom - 1,
    beside: Boolean(table && sheet) && table.right <= sheet.left + 1 && Math.abs(sheet.right - card.right) <= 1,
    // The head's ring stands 1 px out on either side, as the sticky head of every list's.
    head: Boolean(head && card) && Math.abs(head.left - card.left) <= 1 && Math.abs(head.right - card.right) <= 1,
    sidebar: getComputedStyle(document.querySelector("#sidebar")).display,
    overview: Boolean(over && over.height > 0),
    turns: document.querySelectorAll(".st-card-sem > .st-sem-head .st-turn").length,
    sub: Boolean(document.querySelector(".st-card-sem .st-sem-sub")),
    pressed: document.querySelector(".st-card-sem .st-empty-add")?.getAttribute("aria-pressed"),
    empty: Boolean(document.querySelector(".st-card-sem .st-empty")),
  };
});
check(shape.under && shape.beside && shape.head && shape.sidebar !== "none" && shape.overview && shape.turns === 2 && !shape.sub && shape.pressed === "true" && shape.empty, `the card is not opened under its head, the page around it: ${JSON.stringify(shape)}`);
const head = await page.evaluate(() => ({
  figure: document.querySelector(".st-sem-bar .st-wb-hfig")?.textContent,
  bar: document.querySelector(".st-sem-bar .st-wb-hbar")?.getAttribute("aria-label"),
  title: document.querySelector(".st-wb-half .st-wb-half-t")?.textContent,
  close: document.querySelector(".st-wb-half-h .st-wb-done")?.getAttribute("aria-label"),
  esc: document.querySelectorAll('.st-card-sem [data-action="close-focus"]').length,
  label: document.querySelector(".st-wb-half")?.getAttribute("aria-label"),
}));
check(head.figure === "0 von 30 LP" && head.bar === "0 von 30 LP" && head.title === "Hinzufügen" && head.close === "Planen beenden" && head.esc === 1 && head.label === "Hinzufügen zu WiSe 2026/27", `the head and the half's top: ${JSON.stringify(head)}`);
const plan = await half();
const root = await page.evaluate(() => ({
  heading: document.querySelector(".st-wb-half .st-wb-sk")?.textContent,
  next: document.querySelector(".st-wb-half #st-wb-next .st-wb-sk")?.textContent,
  ways: [...document.querySelectorAll(".st-wb-half .st-wb-ways .st-wb-a .nm > span:last-child")].map((way) => way.firstChild?.textContent.trim()).join("|"),
}));
check(root.heading === "Laut Plan im 3. FS" && root.next === "Aus dem 4. FS vorziehen" && root.ways === "Alle Bereiche|Gemerkt|Im Katalog suchen" && plan.title === null, `the root of the half: ${JSON.stringify(root)}`);
check(plan.rows.filter((row) => /^\d+\s*LP$/.test(row.act ?? "")).slice(0, 3).map((row) => row.name).join("|") === "Theoretische Informatik|Softwarepraktikum|Mathematik IT-3 (Analysis)", `the Pflicht of the 3. FS: ${JSON.stringify(plan.rows)}`);
check(plan.rows.some((row) => row.name === "Fachübergreifendes Studium" && row.choice), "the FÜS of the 1. FS is not open from before");
const balanced = await page.evaluate(() => {
  const row = document.querySelector(".st-wb-half .st-wb-row:not(.choice):not(.place)");
  const box = row?.getBoundingClientRect();
  const button = row?.querySelector(":scope > .st-wb-plus")?.getBoundingClientRect();
  const card = document.querySelector(".st-wb-half .st-wb-row.choice");
  return { middle: box && button ? Math.abs((box.top + box.bottom) / 2 - (button.top + button.bottom) / 2) : -1, dashed: card ? getComputedStyle(card).outlineStyle : "", bar: Boolean(card?.querySelector(".st-wb-fill")) };
});
check(balanced.middle >= 0 && balanced.middle <= 2 && balanced.dashed === "dashed" && balanced.bar, `a module and a choice in the half: ${JSON.stringify(balanced)}`);

// 2: „Plan übernehmen": the Pflicht into the rows on the left, a part of the head's bar each.
await step("Plan übernehmen", () => page.click("#st-wb-takeover"), () => document.querySelectorAll(".st-card-sem .st-row").length === 3 && document.querySelector(".study-note")?.textContent.includes("3 Pflichtmodule in WiSe 2026/27 eingeplant") && document.querySelectorAll(".st-sem-bar .st-wb-hbar > .planned").length === 3 && document.querySelector(".st-sem-bar .st-wb-hfig")?.textContent === "24 von 30 LP" && !document.querySelector("#st-wb-takeover"));
check(JSON.stringify(await semester()) === JSON.stringify(["Theoretische Informatik|", "Softwarepraktikum|", "Mathematik IT-3 (Analysis)|"]), `the semester after „Plan übernehmen“: ${JSON.stringify(await semester())}`);
check((await lines("m\t2026W\t")).every((line) => line.split("\t")[5] === "plan"), "what „Plan übernehmen“ planned is not the plan's");

// 3: the Anwendungsfach's page, its switches. The groups once the week of the semester is checked
// (`pages::fit`, asked as the half opens).
await step("Anwendungsfach ›", () => page.click('.st-wb-half .st-wb-row.choice:has-text("Anwendungsfach") .st-wb-go'), () => document.querySelector(".st-wb-half .st-wb-pt > b")?.textContent === "Anwendungsfach" && document.querySelector(".st-wb-half .st-wb-g")?.textContent.startsWith("passen in deine Woche"));
const anw = await half();
const anwHead = await page.evaluate(() => ({
  figure: document.querySelector(".st-wb-half .st-wb-pt > .st-wb-fig")?.textContent,
  bar: Boolean(document.querySelector(".st-wb-half .st-wb-pth > .st-wb-fill")),
  chips: [...document.querySelectorAll(".st-wb-half .st-wb-ctl .st-wb-chips > *")].map((chip) => chip.textContent.trim()).join("|"),
  way: document.querySelector(".st-wb-half .st-wb-page")?.getAttribute("data-way"),
  esc: document.querySelector('.st-card-sem [data-action="close-focus"]')?.classList.contains("st-wb-back"),
}));
check(anw.crumbs === "Übersicht" && anw.groups[0] === "passen in deine Woche" && anwHead.figure === "0 / 12 LP" && anwHead.bar && anwHead.chips === "Alle 5 Fächer|Im WiSe angeboten|Passt in meine Woche" && anwHead.way === "in" && anwHead.esc, `the Anwendungsfach: ${JSON.stringify({ crumbs: anw.crumbs, groups: anw.groups, ...anwHead })}`);
const lists = await page.evaluate(() => document.querySelector("#st-wb-fach")?.textContent.trim());
check(lists === "Alle 5 Fächer", `„Fach“: ${lists}`);
const all = anw.rows.length;
await step("Passt in meine Woche", () => page.click('.st-wb-half .st-wb-ctl button:has-text("Passt in meine Woche")'), (all) => document.querySelectorAll(".st-wb-half .st-wb-row").length < all && ![...document.querySelectorAll(".st-wb-half .st-wb-g")].some((g) => g.textContent.startsWith("treffen deine Woche")), all);
await step("Im WiSe angeboten, off", () => page.click('.st-wb-half .st-wb-ctl button:has-text("Im WiSe angeboten")').then(() => page.click('.st-wb-half .st-wb-ctl button:has-text("Passt in meine Woche")')), () => [...document.querySelectorAll(".st-wb-half .st-wb-g")].some((g) => g.textContent.startsWith("nicht im WiSe angeboten")));
await page.click('.st-wb-half .st-wb-ctl button:has-text("Im WiSe angeboten")');

// 4: „+ Einplanen" counts for the Anwendungsfach.
const vwl = "Einführung in die Volkswirtschaftslehre";
await step("+ Einplanen", () => page.click(`.st-wb-half .st-wb-row:has(.st-wb-go b:text-is("${vwl}")) .st-wb-plus`), (vwl) => [...document.querySelectorAll(".st-card-sem .st-row")].some((row) => row.querySelector(".st-name")?.textContent === vwl && row.querySelector(".st-for")?.textContent === "für Anwendungsfach"), vwl);
const vwlLine = (await lines("m\t2026W\t11947\t"))[0]?.split("\t");
check(vwlLine?.[6] === "079-82-2008" && vwlLine?.[7] && vwlLine?.length === 9, `the store does not remember the Anwendungsfach: ${JSON.stringify(vwlLine)}`);
check((await pageFigure()) === "6 / 12 LP", `what the Anwendungsfach counts: ${await pageFigure()}`);
await step("its row planned", () => Promise.resolve(), (vwl) => document.querySelector(`.st-wb-half .st-wb-row.is-done .st-wb-plus.done`)?.closest(".st-wb-row")?.textContent.includes(vwl), vwl);

// 5: a module both choices list, planned from Praktische Mathematik; its menu beside the half.
const stat = "Mathematische Statistik";
await step("‹, the root", () => page.click(".st-wb-half .st-wb-back"), () => document.querySelector(".st-wb-half .st-wb-half-t") && document.querySelector(".st-wb-half .st-wb-page")?.getAttribute("data-way") === "back");
await step("Praktische Mathematik ›", () => page.click('.st-wb-half .st-wb-row.choice:has-text("Praktische Mathematik") .st-wb-go'), () => document.querySelector(".st-wb-half .st-wb-pt > b")?.textContent.endsWith("Praktische Mathematik"));
await page.click('.st-wb-half .st-wb-ctl button:has-text("Im WiSe angeboten")');
await step("+ Einplanen, for Praktische Mathematik", () => page.click(`.st-wb-half .st-wb-row:has(.st-wb-go b:text-is("${stat}")) .st-wb-plus`), (stat) => [...document.querySelectorAll(".st-card-sem .st-row")].some((row) => row.querySelector(".st-name")?.textContent === stat && row.querySelector(".st-for")?.textContent === "für Praktische Mathematik"), stat);
await page.click('.st-wb-half .st-wb-ctl button:has-text("Im WiSe angeboten")');
await step("its menu, the half open", () => page.click(`.st-card-sem .st-row:has(.st-name:text-is("${stat}")) .st-status`), () => document.querySelector(".st-pop.root #st-menu-counts") && document.querySelector(".st-wb-half"));
await step("Zählt für ›", () => page.click(".st-pop.root #st-menu-counts"), () => document.querySelector(".st-pop.sub") && [...document.querySelectorAll(".st-pop.sub [role=menuitem]")].some((item) => item.textContent.includes("Anwendungsfach")));
await step("Zählt für Anwendungsfach", () => page.click('.st-pop.sub [role=menuitem]:has-text("Anwendungsfach")'), (stat) => document.querySelector(".study-note")?.textContent.includes("zählt für Anwendungsfach") && [...document.querySelectorAll(".st-card-sem .st-row")].some((row) => row.querySelector(".st-name")?.textContent === stat && row.querySelector(".st-for")?.textContent === "für Anwendungsfach"), stat);

// 5b: „Zählt für ›" › „Anderer Bereich …" (owner, 2026-11-08: „jedes modul zu jedem möglichen
// bereich zuzuordnen … dass es angezeigt wird, wenn man das gemacht hat und dass das man das nicht
// ausversehen macht"): the dialog of every subgroup; Mathematik's Pflichtmodule are an exception,
// said before „Als Ausnahme zuordnen"; the row then says „Ausnahme" and counts for Mathematik;
// „Automatisch" takes it back to where it fits best, the Nebenfach, without the mark.
const counts = async (name) => {
  await step(`${name}: its menu`, () => page.click(`.st-card-sem .st-row:has(.st-name:text-is("${stat}")) .st-status`), () => document.querySelector(".st-pop.root #st-menu-counts"));
  await step(`${name}: Zählt für ›`, () => page.click(".st-pop.root #st-menu-counts"), () => document.querySelector(".st-pop.sub #st-menu-auto") && document.querySelector(".st-pop.sub #st-menu-other"));
};
await counts("an exception");
await step("Anderer Bereich …", () => page.click(".st-pop.sub #st-menu-other"), () => document.querySelectorAll(".st-cf .st-cf-opt").length > 8 && document.querySelector(".st-cf-area h3")?.textContent === "Informatik" && document.querySelector('.st-cf-opt input:checked')?.closest(".st-cf-opt")?.textContent.includes("Anwendungsfach"));
await step("Mathematik's Pflichtmodule", () => page.click('.st-cf-area:has(h3:text-is("Mathematik")) .st-cf-opt'), () => document.querySelector(".st-cf-warn.warn")?.textContent.includes("Ausnahme") && document.querySelector("#st-cf-go")?.textContent.trim() === "Als Ausnahme zuordnen");
await step("Als Ausnahme zuordnen", () => page.click("#st-cf-go"), (stat) => {
  const counted = [...document.querySelectorAll(".st-card-sem .st-row")].some((row) => row.querySelector(".st-name")?.textContent === stat && row.querySelector(".st-exception") && row.querySelector(".st-area")?.textContent === "Mathematik");
  return !document.querySelector(".st-cf") && document.querySelector(".study-note")?.textContent.includes("zählt als Ausnahme für") && counted;
}, stat);
await counts("back");
await step("Automatisch", () => page.click(".st-pop.sub #st-menu-auto"), (stat) => [...document.querySelectorAll(".st-card-sem .st-row")].some((row) => row.querySelector(".st-name")?.textContent === stat && !row.querySelector(".st-exception") && row.querySelector(".st-area")?.textContent === "Nebenfach"), stat);
check(Boolean(await page.$(".st-card-sem.split > .st-wb-half")), "a row's menu closed the half");

// 6: „Alle Bereiche", the FÜS, a module beside the page.
await step("‹, the root again", () => page.click(".st-wb-half .st-wb-back"), () => document.querySelector(".st-wb-half .st-wb-half-t"));
await step("Alle Bereiche ›", () => page.click("#st-wb-areas"), () => document.querySelector(".st-wb-half .st-wb-pt > b")?.textContent === "Alle Bereiche" && document.querySelectorAll(".st-wb-half .st-wb-ta").length >= 5);
const tree = await page.evaluate(() => [...document.querySelectorAll(".st-wb-half .st-wb-ta")].map((area) => area.querySelector(".st-wb-tn > span:not(.st-dot)")?.textContent + ":" + area.querySelectorAll(".st-wb-ts").length).join("|"));
check(tree === "Informatik:2|Mathematik:0|Nebenfach:2|FÜS:0|Fachstudium:4|Bachelor-Arbeit:0", `the tree of the areas: ${tree}`);
await step("FÜS ›", () => page.click('.st-wb-half .st-wb-ta:has(.st-wb-tn:has-text("FÜS")) button.st-wb-th'), () => document.querySelector(".st-wb-half .st-wb-pt > b")?.textContent === "Fachübergreifendes Studium" && document.querySelector(".st-wb-half .st-wb-row") && document.querySelector(".st-wb-half .st-wb-crumbs")?.textContent.includes("Alle Bereiche"));
await step("a module beside the page", () => page.click(".st-wb-half .st-wb-row a.st-wb-go >> nth=0"), () => document.querySelector("#preview .plan-toggle[data-action=plan]"));
const aim = await page.evaluate(() => document.querySelector("#preview .plan-toggle[data-action=plan]")?.textContent.replace(/\s+/g, " "));
check(aim?.includes("WiSe 26/27") && aim.includes("für „Fachübergreifendes Studium“"), `„Einplanen“ beside the page does not aim at the workbench: ${aim}`);
await step("Esc, the module", () => page.keyboard.press("Escape"), () => !document.querySelector("#preview") && document.querySelector(".st-wb-half .st-wb-pt > b")?.textContent === "Fachübergreifendes Studium");

// 7: the catalog with the workbench as its target.
await step("Im Katalog suchen", () => page.click(".st-wb-half .st-wb-foot a.st-wb-a.way"), () => location.pathname === "/catalog" && document.querySelector(".bench-bar")?.textContent.includes("für Fachübergreifendes Studium") && document.querySelector(".rows a.row"));
const query = await page.evaluate(() => Object.fromEntries(new URLSearchParams(location.search)));
check(Object.values(query).join(" ").includes("fues") || (await page.evaluate(() => location.search)).includes("fues"), `the catalog is not the FÜS list: ${JSON.stringify(query)}`);
const back = await page.evaluate(() => document.querySelector(".bench-bar a")?.textContent.replace(/\s+/g, " ").trim());
check(back?.startsWith("Zurück zu WiSe 2026/27"), `the way back of the catalog: ${back}`);
const fuesId = await page.getAttribute(".rows a.row >> nth=0", "data-id");
await step("its module", () => page.click(".rows a.row >> nth=0"), () => document.querySelector(".detail .hero .plan-toggle[data-action=plan]"));
await step("Einplanen, for the FÜS", () => page.click(".detail .hero .plan-toggle[data-action=plan]"), ([key, id]) => (localStorage.getItem(key) || "").split("\n").some((line) => line.startsWith(`m\t2026W\t${id}\t`) && line.split("\t").length === 9), [PLAN, fuesId]);
await step("Zurück zu WiSe 2026/27", () => page.click(".bench-bar a"), () => location.pathname === "/study" && document.querySelector(".st-wb-half .st-wb-pt > b")?.textContent === "Fachübergreifendes Studium");
check((await semester()).some((row) => row.endsWith("|für FÜS")), `the catalog's module does not count for the FÜS: ${JSON.stringify(await semester())}`);

// 8: › in the head turns the semester and the half with it.
await step("back to the root", () => page.click('.st-wb-half .st-wb-crumbs button:has-text("Übersicht")'), () => document.querySelector(".st-wb-half .st-wb-half-t"));
await step("› SoSe 2027", () => page.click(".st-card-sem > .st-sem-head .st-turn >> nth=1"), () => document.querySelector(".st-card-sem h2")?.textContent === "SoSe 2027" && document.querySelector(".st-wb-half")?.getAttribute("aria-label") === "Hinzufügen zu SoSe 2027" && document.querySelector(".st-wb-half .st-wb-sk")?.textContent === "Laut Plan im 4. FS" && document.querySelector(".st-sem-bar .st-wb-hfig")?.textContent === "0 von 30 LP");
await step("‹ WiSe 2026/27", () => page.click(".st-card-sem > .st-sem-head .st-turn >> nth=0"), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27" && document.querySelector(".st-wb-half")?.getAttribute("aria-label") === "Hinzufügen zu WiSe 2026/27");

// 9: Esc a step back, then the mode; the Gesamtplan's way in, another view's way out.
await step("Alle Bereiche, to step back from", () => page.click("#st-wb-areas"), () => document.querySelector(".st-wb-half .st-wb-pt > b")?.textContent === "Alle Bereiche");
await step("Esc, a step back", () => page.keyboard.press("Escape"), () => document.querySelector(".st-wb-half .st-wb-half-t") && document.querySelector('.st-card-sem [data-action="close-focus"]')?.classList.contains("st-wb-done"));
await step("Esc, the mode", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-wb-half") && !document.querySelector(".st-card-sem.split") && document.querySelector(".st-card-sem .st-sem-sub") && !document.querySelector(".st-card-sem .st-for"));
const held = await page.evaluate(() => document.querySelectorAll(".st-card-sem .st-row").length);
check(held === 6, `the semester holds ${held} modules, not 6`);
await step("Gesamtplan", () => page.click('#sidebar .st-vl:has-text("Gesamtplan")'), () => document.querySelector(".st-cell-add"));
await step("+ of the Nebenfach in WiSe 2026/27", () => page.click('.st-cell-add[aria-label="Modul einplanen: Nebenfach, WiSe 2026/27"]', { force: true }), () => document.querySelector(".st-card-sem.split .st-wb-half .st-wb-pt > b")?.textContent === "Anwendungsfach" && document.querySelector('#sidebar .st-vl[aria-current="page"]')?.textContent.startsWith("Semester") && document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
await step("another view ends it", () => page.click('#sidebar .st-vl:has-text("Gesamtplan")'), () => !document.querySelector(".st-wb-half") && document.querySelector(".st-cell-add"));
const leaks = requests.filter((r) => r.includes("11947") && !r.includes("/pkg/") && !r.includes("/assets/"));
check(leaks.length === 0, `the plan went into a request: ${leaks.slice(0, 3).join(" | ")}`);

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
