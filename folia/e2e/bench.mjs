// Checks the workbench of „Mein Studium" (folia/crates/planner/src/study/bench.rs; folia/design/
// hinzufuegen/konzepte.html, third round; owner, 2026-11-03: „Passt so, bau das jetzt in Folia"),
// its ten steps on Informatik B.Sc. in its third semester, the first two passed:
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node bench.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// 1 „Module hinzufügen" opens the mode for WiSe 2026/27: its head over everything it changes
//   („Hinzufügen zu", the semester with ‹ ›, what it holds over the bar of it, ×), the page's
//   sidebar aside, the areas of the overview as a tree, each with its subgroups unfolded, under
//   „Laut Plan im 3. FS", which is picked; a module of the plan with its credits as its button, a
//   choice dashed with its bar; the semester empty.
// 2 „Plan übernehmen" plans the three Pflichtmodule of the 3. FS at once, the semester shows them
//   with „Pflicht", the note says so with „Rückgängig".
// 3 Nebenfach picks the Anwendungsfach, the subgroup due in the 3. FS; the list in groups by the
//   week, its head with its figure and bar, „Fach" with its five lists as a chip; „Passt in meine
//   Woche" keeps what fits, „Im WiSe angeboten" off shows what the winter does not offer.
// 4 „+ Einplanen" plans a module at once: it counts for the Anwendungsfach (the semester says „für
//   Anwendungsfach", the Nebenfach and the subgroup count it), the store remembers the row.
// 5 A module both choices list (Mathematische Statistik), planned from Praktische Mathematik,
//   counts there; its menu's „Zählt für ›" lets it count for the Anwendungsfach.
// 6 The FÜS: a row opens its module beside the page over the semester, whose „Einplanen" aims at
//   the workbench's semester for the FÜS; Esc closes the module, the mode stays.
// 7 „Im Katalog suchen ›": the catalog with the FÜS list and the winter, the workbench's bar at the
//   head of its list; a module planned from there counts for the FÜS; „Zurück zur Werkbank" leads
//   back to the mode as it was.
// 8 Esc ends the mode (× does too); the semester holds what was planned. Nothing of it in a request.
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
const pane = () => page.evaluate(() => ({
  title: document.querySelector(".st-wb-pt h3")?.textContent,
  crumb: document.querySelector(".st-wb-crumb")?.textContent,
  groups: [...document.querySelectorAll(".st-wb-pane .st-wb-g")].map((g) => g.firstChild?.textContent.trim()),
  rows: [...document.querySelectorAll(".st-wb-pane .st-wb-row")].map((row) => ({ name: row.querySelector(".st-wb-go b")?.textContent, act: row.querySelector(".st-wb-plus")?.textContent.trim(), fit: row.querySelector(".st-wb-fit")?.className, choice: row.classList.contains("choice") })),
}));
const semester = () => page.evaluate(() => [...document.querySelectorAll(".st-wb-sem .st-wb-sr")].map((row) => `${row.querySelector(".st-wb-go b")?.textContent}|${row.querySelector(".st-wb-tag")?.textContent ?? ""}`));
const area = (name) => page.evaluate((name) => [...document.querySelectorAll(".st-wb-areas .st-wb-a")].find((a) => a.querySelector(".nm > span:last-child")?.textContent === name)?.querySelector("small")?.textContent, name);

// 1: the mode.
await step("to now", () => page.click(".st-card-sem .st-sem-pick").then(() => page.click('.st-pop.root [role=menuitem]:has-text("WiSe 26/27")')), () => document.querySelector(".st-card-sem h2")?.textContent === "WiSe 2026/27");
await step("Module hinzufügen", () => page.click(".st-card-sem .st-empty-add"), () => document.querySelector(".st-wb-bar em")?.textContent === "WiSe 2026/27" && document.querySelector('.st-wb-a.top[aria-current="true"]') && document.querySelector("#st-wb-takeover"));
const shape = await page.evaluate(() => {
  const rect = (selector) => document.querySelector(selector)?.getBoundingClientRect();
  const [bar, areas, middle, sem] = [".st-wb-bar", ".st-wb-areas", ".st-wb-pane", ".st-wb-sem"].map(rect);
  return {
    above: [areas, middle, sem].every((box) => box && bar && box.top >= bar.bottom - 1),
    across: bar && sem ? Math.round(bar.right - sem.right) : null,
    order: areas && middle && sem ? areas.right <= middle.left && middle.right <= sem.left : false,
    sidebar: getComputedStyle(document.querySelector("#sidebar")).display,
    bar: [...document.querySelectorAll(".st-wb-bar *")].map((el) => el.getAttribute("class") ?? "").join(" "),
    areas: [...document.querySelectorAll(".st-wb-areas .st-wb-a .nm > span:last-child")].map((name) => name.textContent).join("|"),
    empty: document.querySelector(".st-wb-sem .st-wb-rule")?.textContent,
  };
});
check(shape.above && shape.across === 0 && shape.order && shape.sidebar === "none", `the workbench's columns are not under its bar, side by side: ${JSON.stringify(shape)}`);
check(shape.bar.includes("st-wb-sem-pick") && shape.bar.includes("st-wb-hfig") && shape.bar.includes("st-wb-hbar") && shape.bar.includes("st-wb-done"), `the head of the mode: ${shape.bar}`);
const head = await page.evaluate(() => ({
  title: document.querySelector(".st-wb-bar .st-wb-bar-t")?.textContent,
  figure: document.querySelector(".st-wb-bar .st-wb-hfig")?.textContent,
  bar: document.querySelector(".st-wb-bar .st-wb-hbar")?.getAttribute("aria-label"),
  close: document.querySelector(".st-wb-bar .st-wb-done")?.getAttribute("aria-label"),
  words: document.querySelector(".st-wb-bar .st-wb-done")?.textContent.trim(),
  subs: [...document.querySelectorAll(".st-wb-areas .st-wb-subs")].map((subs) => subs.querySelectorAll(".st-wb-s").length).join(","),
  wahlpflicht: [...document.querySelectorAll(".st-wb-areas .st-wb-s")].some((sub) => sub.textContent.startsWith("Wahlpflicht")),
}));
check(head.title === "Hinzufügen zu" && head.figure === "0 von 30 LP" && head.bar === "0 von 30 LP" && head.close === "Planen beenden" && head.words === "" && head.subs === "2,2,4" && !head.wahlpflicht, `the head and the tree: ${JSON.stringify(head)}`);
check(shape.areas === "Laut Plan im 3. FS|Informatik|Mathematik|Nebenfach|FÜS|Fachstudium|Bachelor-Arbeit|Gemerkt|Im Katalog suchen", `the areas: ${shape.areas}`);
check(shape.empty === "Noch nichts belegt.", `the semester begins empty: ${shape.empty}`);
const plan = await pane();
// One level of headings: „Aus dem 4. FS vorziehen" alone, the rows say what they are.
const nextHead = await page.evaluate(() => document.querySelector(".st-wb-pane #st-wb-next")?.textContent);
check(plan.groups.length === 0 && nextHead === "Aus dem 4. FS vorziehen", `the headings of „Laut Plan“: ${JSON.stringify({ groups: plan.groups, next: nextHead })}`);
check(plan.rows.filter((row) => /^\d+\s*LP$/.test(row.act ?? "")).slice(0, 3).map((row) => row.name).join("|") === "Theoretische Informatik|Softwarepraktikum|Mathematik IT-3 (Analysis)", `the Pflicht of the 3. FS: ${JSON.stringify(plan.rows)}`);
check(plan.rows.some((row) => row.name === "Fachübergreifendes Studium" && row.choice), "the FÜS of the 1. FS is not open from before");
const balanced = await page.evaluate(() => {
  const row = document.querySelector(".st-wb-pane .st-wb-row:not(.choice):not(.place)");
  const box = row?.getBoundingClientRect();
  const button = row?.querySelector(":scope > .st-wb-plus")?.getBoundingClientRect();
  const card = document.querySelector(".st-wb-pane .st-wb-row.choice");
  return { middle: box && button ? Math.abs((box.top + box.bottom) / 2 - (button.top + button.bottom) / 2) : -1, dashed: card ? getComputedStyle(card).outlineStyle : "", bar: Boolean(card?.querySelector(".st-wb-fill")) };
});
check(balanced.middle >= 0 && balanced.middle <= 2 && balanced.dashed === "dashed" && balanced.bar, `a module and a choice in the middle: ${JSON.stringify(balanced)}`);

// 2: „Plan übernehmen": the Pflicht into the semester on the right, out of the middle, a part of
// the head's bar each.
await step("Plan übernehmen", () => page.click("#st-wb-takeover"), () => document.querySelectorAll(".st-wb-sem .st-wb-sr").length === 3 && document.querySelector(".study-note")?.textContent.includes("3 Pflichtmodule in WiSe 2026/27 eingeplant") && document.querySelectorAll(".st-wb-hbar > .planned").length === 3 && document.querySelector(".st-wb-bar .st-wb-hfig")?.textContent === "24 von 30 LP" && !document.querySelector("#st-wb-takeover"));
check(JSON.stringify(await semester()) === JSON.stringify(["Theoretische Informatik|Pflicht", "Softwarepraktikum|Pflicht", "Mathematik IT-3 (Analysis)|Pflicht"]), `the semester after „Plan übernehmen“: ${JSON.stringify(await semester())}`);
check((await lines("m\t2026W\t")).every((line) => line.split("\t")[5] === "plan"), "what „Plan übernehmen“ planned is not the plan's");

// 3: Nebenfach, its Anwendungsfach, the switches.
// The groups once the week of the semester is checked (`pages::fit`, asked as the mode opens).
await step("Nebenfach", () => page.click('.st-wb-areas .st-wb-a:has-text("Nebenfach")'), () => document.querySelector(".st-wb-pt h3")?.textContent === "Anwendungsfach" && document.querySelector('.st-wb-s[aria-current="true"]')?.textContent.startsWith("Anwendungsfach") && document.querySelector(".st-wb-pane .st-wb-g")?.textContent.startsWith("passen in deine Woche"));
const anw = await pane();
const anwHead = await page.evaluate(() => ({ figure: document.querySelector(".st-wb-ph .st-wb-fig")?.textContent, bar: Boolean(document.querySelector(".st-wb-ph > .st-wb-fill")), chips: [...document.querySelectorAll(".st-wb-ctl .st-wb-chips > *")].map((chip) => chip.textContent.trim()).join("|") }));
check(anw.crumb === "Nebenfach ›" && anw.groups[0] === "passen in deine Woche" && anwHead.figure === "0 / 12 LP" && anwHead.bar && anwHead.chips === "Alle 5 Fächer|Im WiSe angeboten|Passt in meine Woche", `the Anwendungsfach: ${JSON.stringify({ crumb: anw.crumb, groups: anw.groups, ...anwHead })}`);
const lists = await page.evaluate(() => document.querySelector("#st-wb-fach")?.textContent.trim());
check(lists === "Alle 5 Fächer", `„Fach“: ${lists}`);
const all = anw.rows.length;
await step("Passt in meine Woche", () => page.click('.st-wb-ctl button:has-text("Passt in meine Woche")'), (all) => document.querySelectorAll(".st-wb-pane .st-wb-row").length < all && ![...document.querySelectorAll(".st-wb-pane .st-wb-g")].some((g) => g.textContent.startsWith("treffen deine Woche")), all);
await step("Im WiSe angeboten, off", () => page.click('.st-wb-ctl button:has-text("Im WiSe angeboten")').then(() => page.click('.st-wb-ctl button:has-text("Passt in meine Woche")')), () => [...document.querySelectorAll(".st-wb-pane .st-wb-g")].some((g) => g.textContent.startsWith("nicht im WiSe angeboten")));
await page.click('.st-wb-ctl button:has-text("Im WiSe angeboten")');

// 4: „+ Einplanen" counts for the Anwendungsfach.
const vwl = "Einführung in die Volkswirtschaftslehre";
await step("+ Einplanen", () => page.click(`.st-wb-pane .st-wb-row:has(.st-wb-go b:text-is("${vwl}")) .st-wb-plus`), (vwl) => [...document.querySelectorAll(".st-wb-sem .st-wb-sr")].some((row) => row.textContent.includes(vwl) && row.textContent.includes("für Anwendungsfach")), vwl);
const vwlLine = (await lines("m\t2026W\t11947\t"))[0]?.split("\t");
check(vwlLine?.[6] === "079-82-2008" && vwlLine?.[7] && vwlLine?.length === 9, `the store does not remember the Anwendungsfach: ${JSON.stringify(vwlLine)}`);
check((await area("Nebenfach")) === "6 / 18 LP" && (await page.textContent('.st-wb-s[aria-current="true"] small')) === "6 / 12 LP", `what the Nebenfach counts: ${await area("Nebenfach")}`);
await step("its row planned", () => Promise.resolve(), (vwl) => document.querySelector(`.st-wb-pane .st-wb-row.is-done .st-wb-plus.done`)?.closest(".st-wb-row")?.textContent.includes(vwl), vwl);

// 5: a module both choices list, planned from Praktische Mathematik; „Zählt für ›".
const stat = "Mathematische Statistik";
await step("Praktische Mathematik", () => page.click('.st-wb-subs .st-wb-s:has-text("Praktische Mathematik")'), () => document.querySelector(".st-wb-pt h3")?.textContent.endsWith("Praktische Mathematik"));
await page.click('.st-wb-ctl button:has-text("Im WiSe angeboten")');
await step("+ Einplanen, for Praktische Mathematik", () => page.click(`.st-wb-pane .st-wb-row:has(.st-wb-go b:text-is("${stat}")) .st-wb-plus`), (stat) => [...document.querySelectorAll(".st-wb-sem .st-wb-sr")].some((row) => row.textContent.includes(stat) && row.textContent.includes("für Praktische Mathematik")), stat);
await page.click('.st-wb-ctl button:has-text("Im WiSe angeboten")');
await step("Fertig, for its menu", () => page.click(".st-wb-done"), () => !document.querySelector(".st-wb"));
await step("its menu", () => page.click(`.st-card-sem .st-row:has(.st-name:text-is("${stat}")) .st-status`), () => document.querySelector(".st-pop.root #st-menu-counts"));
await step("Zählt für ›", () => page.click(".st-pop.root #st-menu-counts"), () => document.querySelector(".st-pop.sub") && [...document.querySelectorAll(".st-pop.sub [role=menuitem]")].some((item) => item.textContent.includes("Anwendungsfach")));
await step("Zählt für Anwendungsfach", () => page.click('.st-pop.sub [role=menuitem]:has-text("Anwendungsfach")'), () => document.querySelector(".study-note")?.textContent.includes("zählt für Anwendungsfach"));
await step("Module hinzufügen, again", () => page.click(".st-card-sem .st-add"), () => document.querySelector(".st-wb-sem .st-wb-sr"));
check((await semester()).includes(`${stat}|für Anwendungsfach`), `„Zählt für“ did not move it: ${JSON.stringify(await semester())}`);

// 6: the FÜS, a module beside the page.
await step("FÜS", () => page.click('.st-wb-areas .st-wb-a:has-text("FÜS")'), () => document.querySelector(".st-wb-pt h3")?.textContent === "Fachübergreifendes Studium" && document.querySelector(".st-wb-pane .st-wb-row"));
await step("a module beside the page", () => page.click(".st-wb-pane .st-wb-row a.st-wb-go >> nth=0"), () => document.querySelector("#preview .plan-toggle[data-action=plan]"));
const aim = await page.evaluate(() => document.querySelector("#preview .plan-toggle[data-action=plan]")?.textContent.replace(/\s+/g, " "));
check(aim?.includes("WiSe 26/27") && aim.includes("für „Fachübergreifendes Studium“"), `„Einplanen“ beside the page does not aim at the workbench: ${aim}`);
await step("Esc, the module", () => page.keyboard.press("Escape"), () => !document.querySelector("#preview") && document.querySelector(".st-wb"));

// 7: the catalog with the workbench as its target.
await step("Im Katalog suchen", () => page.click(".st-wb-foot a.st-wb-a.way"), () => location.pathname === "/catalog" && document.querySelector(".bench-bar")?.textContent.includes("für Fachübergreifendes Studium") && document.querySelector(".rows a.row"));
const query = await page.evaluate(() => Object.fromEntries(new URLSearchParams(location.search)));
check(Object.values(query).join(" ").includes("fues") || location.search.includes("fues"), `the catalog is not the FÜS list: ${JSON.stringify(query)}`);
const fuesId = await page.getAttribute(".rows a.row >> nth=0", "data-id");
await step("its module", () => page.click(".rows a.row >> nth=0"), () => document.querySelector(".detail .hero .plan-toggle[data-action=plan]"));
await step("Einplanen, for the FÜS", () => page.click(".detail .hero .plan-toggle[data-action=plan]"), ([key, id]) => (localStorage.getItem(key) || "").split("\n").some((line) => line.startsWith(`m\t2026W\t${id}\t`) && line.split("\t").length === 9), [PLAN, fuesId]);
await step("Zurück zur Werkbank", () => page.click(".bench-bar a"), () => location.pathname === "/study" && document.querySelector(".st-wb-pt h3")?.textContent === "Fachübergreifendes Studium");
check((await semester()).some((row) => row.endsWith("|für FÜS")), `the catalog's module does not count for the FÜS: ${JSON.stringify(await semester())}`);

// 8: Esc ends the mode.
await step("Esc, the mode", () => page.keyboard.press("Escape"), () => !document.querySelector(".st-wb") && document.querySelector(".st-card-sem .st-row"));
const held = await page.evaluate(() => document.querySelectorAll(".st-card-sem .st-row").length);
check(held === 6, `the semester holds ${held} modules, not 6`);
const leaks = requests.filter((r) => r.includes("11947") && !r.includes("/pkg/") && !r.includes("/assets/"));
check(leaks.length === 0, `the plan went into a request: ${leaks.slice(0, 3).join(" | ")}`);

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
