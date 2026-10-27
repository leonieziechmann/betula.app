// A Termin in its own view (owner, 2026-10-26; docs/folia/frontend.md, „A Termin in its own view";
// folia/design/termin/konzepte.html, the fourth round, in the order of the fifth: what picks the
// Termin stands above everything it changes), on canary's data of 2026W with Informatik B.Sc.'s
// first semester planned (Programmierpraktikum 148370 in four groups, Mo, Mi, Do 11:30 and Fr
// 13:45, and its lecture).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node termin.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// - The Stundenplan: a dashed group of the week takes its choice into focus (`wahl=`), a line of
//   „Termine" and its address open the Termin beside the plan (`termin=`), its slot marked; the view
//   names the module and its title, the module's events as segments above the ticket (the
//   Praktikum marked), the date with its arrows, the time and the room; „Im Stundenplan" has the
//   four groups as boxes, none ticked; the day and the series are pictures, which pick nothing.
// - Two groups ticked, as many as one likes: both stand solid in the week, the others leave it, the
//   store keeps two ticks; the tick of one taken back leaves the other.
// - The arrow to the next date names it in the address; „Ausblenden" hides this date alone (the
//   store's `h` line, the pill „ausgeblendet"), „Zeigen" brings it back.
// - The event's switch hides the whole Praktikum and shows it again.
// - Esc closes it; „Zum Modul" opens the module beside the plan, pointing at the Termin.
// - The start page: a Termin of its day opens over the page (`/?termin=…&on=…`), and closing it
//   goes back to the page.
// - „Teilen" copies the Termin's own page (`/event/<key>/<day>`; a wide screen has no share sheet)
//   and says „Link kopiert"; V lets the Termin fill the plan's page („Ganze Seite", `&full=1`) in
//   two columns, still the view that adjusts, its next date the whole page too, and Esc brings it
//   back beside the plan on that day. Over the start page „Ganze Seite" fills the start page, and
//   „Zurück" brings it back over the page.
// - ← and → step to the date after and before as the arrows beside the date do, one date a press,
//   beside the plan, filling the page and on the Termin's own page; not while the keyboard is in
//   the search. `?` there shows the overview with „Im Termin" first.
// - A phone: the view is a sheet over the plan, with „Teilen" and without „Ganze Seite".
// - To look up: a line of the Praktikum's module page opens the Termin beside the page
//   (`?termin=<key>`), in the plan, its groups above the ticket measured against it, „Anpassen ›"
//   under it into the Stundenplan of its semester; a group picked changes the ticket under the
//   groups and nothing above them, the lecture's segment the lecture's ticket; → its next date;
//   „Ganze Seite" is its own page (`/event/<key>/<day>`) in two columns, → there too; Back leads back
//   beside the module's page, Esc closes it.
//   Enterprise Ressource Planning's exercise (12612, outside the plan): „nicht im Plan", its second
//   group red against the plan's lecture, the Termin hatched among the plan's day, „In den
//   Kalender" a file of its own. On a phone the Termin is a sheet over its module's page.
// Needs the snapshot whose current semester is WiSe 2026/27. Fails on a page load after takeover, a
// console error, or a step that does not show up.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const shots = process.env.SMOKE_SHOTS;
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
const FS1 = ["m\t2026W\t12104\t1790000000\t", "m\t2026W\t12107\t1790000000\t", "m\t2026W\t12102\t1790000000\t", "m\t2026W\t11112\t1790000000\t", ""].join("\n");
const MINE_FS1 = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n";

const open = async (path, viewport = { width: 1500, height: 1000 }) => {
  const context = await browser.newContext({ viewport });
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  await context.addInitScript(([plan, mine, planText, mineText]) => { if (!localStorage.getItem(plan)) { localStorage.setItem(plan, planText); localStorage.setItem(mine, mineText); } }, [PLAN, MINE, FS1, MINE_FS1]);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push(`${path}: the browser app never took over`));
  await page.evaluate(() => { window.__marker = 1; });
  return { page, context };
};
// The Praktikum's Termin as „Termine" leads to it (the Stundenplan's part finds it).
let praktikum = "";
const until = async (page, name, fn, arg) => {
  try {
    await page.waitForFunction(fn, arg, { timeout: 10000 });
    return true;
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
    return false;
  }
};
const stored = (page) => page.evaluate((key) => localStorage.getItem(key) ?? "", PLAN);
// The view as it reads: its head, the ticket, the groups and the switches.
const view = (page) => page.evaluate(() => {
  const panel = onPage("#preview.is-termin");
  const text = (selector) => panel?.querySelector(selector)?.textContent.replace(/\s+/g, " ").trim() ?? "";
  return {
    chip: text(".tm-mod"),
    kind: text(".tm-kind") || text(".tm-evs > [aria-checked='true']"),
    events: [...(panel?.querySelectorAll(".tm-evs > a") ?? [])].map((a) => a.textContent.trim()),
    // What picks the Termin above the ticket, and the pictures that pick nothing.
    above: (() => { const pick = panel?.querySelector(".tm-pick"), tk = panel?.querySelector(".tk"); return Boolean(pick && tk && pick.getBoundingClientRect().bottom <= tk.getBoundingClientRect().top); })(),
    pictures: (panel?.querySelectorAll(".rib a, .sdots a") ?? []).length === 0 && Boolean(panel?.querySelector(".rib, .sdots")),
    title: text(".tm-title"),
    date: text(".tk-date > b"),
    pill: text(".tk-date .pill"),
    time: text(".tk-time"),
    room: text(".tk-room .plate"),
    groups: [...(panel?.querySelectorAll(".gs.pick") ?? [])].map((tile) => [tile.querySelector("b")?.textContent.trim(), tile.getAttribute("aria-checked")]),
    right: text(".grp-h .r"),
    shown: panel?.querySelector(".tg")?.getAttribute("aria-checked") ?? "",
    date_button: text("[data-action='hide-date']"),
  };
});
// The Praktikum's slots in the week: [day column text, dashed].
const practicals = (page) => page.evaluate(() => [...document.querySelectorAll(".sp-body .week .slot")].filter((slot) => slot.textContent.includes("Prak PP")).map((slot) => slot.className.split(" ").includes("alt")));

// ---------- the Stundenplan ----------
{
  // A dashed group of the week takes its choice into focus (docs/folia/frontend.md, „Offen and the
  // choice in focus"); a line of „Termine" opens the Termin, and so does its address.
  const { page: first, context: weekly } = await open("/studyplan");
  await until(first, "the week", () => document.querySelectorAll(".sp-body .week .slot").length > 0);
  const groups = await first.evaluate(() => [...document.querySelectorAll(".sp-body .week a.slot.alt")].filter((slot) => slot.textContent.includes("Prak PP")).map((a) => a.getAttribute("href")));
  check(groups.length === 4 && groups.every((href) => href === "/studyplan?wahl=e148370"), `a dashed group of the Praktikum takes its choice into focus: ${JSON.stringify(groups)}`);
  await first.click('a[href="/studyplan?view=dates"]');
  await until(first, "„Termine\"", () => document.querySelector(".sp-body [href*='termin=148370-']"));
  const href = await first.evaluate(() => document.querySelector(".sp-body [href*='termin=148370-']")?.getAttribute("href") ?? "");
  const key = new URLSearchParams(href.split("?")[1] ?? "").get("termin") ?? "";
  check(key.startsWith("148370-"), `a line of the Praktikum leads to its Termin: „${href}"`);
  praktikum = key;
  await weekly.close();
  const { page, context } = await open(`/studyplan?termin=${key}`);
  await until(page, "the Termin beside the plan", () => Boolean(onPage("#preview.is-termin .tk")));
  let v = await view(page);
  check(v.chip === "PP" && v.kind === "Praktikum" && v.title === "Programmierpraktikum", `the head: ${JSON.stringify(v)}`);
  check(v.events.join(" | ") === "Vorlesung | Praktikum" && v.above && v.pictures, `the module's events above the ticket, the day and the series pictures: ${JSON.stringify(v)}`);
  check(/^(Mo|Di|Mi|Do|Fr) \d\d\.\d\d\./.test(v.date) && /^\d\d:\d\d/.test(v.time) && v.room.includes("0.03"), `the ticket: ${JSON.stringify(v)}`);
  check(v.groups.length === 4 && v.groups.every(([, on]) => on === "false") && v.right === "noch keine", `four groups, none ticked: ${JSON.stringify(v)}`);
  check(await page.evaluate(() => Boolean(document.querySelector(".sp-body .week .slot.cur, .sp-body .week [aria-current='true']"))), "its slot is marked");
  check((await practicals(page)).filter(Boolean).length === 4, `the four groups dashed: ${JSON.stringify(await practicals(page))}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-open.png` });

  // Two ticked: both solid, the others gone; one taken back leaves the other.
  await page.click("#preview.is-termin .gs.pick >> nth=0");
  await until(page, "the first tick", () => onPage("#preview.is-termin .gs.pick")?.getAttribute("aria-checked") === "true");
  await page.click("#preview.is-termin .gs.pick >> nth=1");
  await until(page, "the second tick", () => [...document.querySelectorAll("#preview.is-termin .gs.pick")].filter((tile) => tile.getAttribute("aria-checked") === "true").length === 2);
  v = await view(page);
  check(v.right === "2 von 4", `„2 von 4": ${JSON.stringify(v)}`);
  let week = await practicals(page);
  check(week.length === 2 && week.every((dashed) => !dashed), `two groups solid in the week: ${JSON.stringify(week)}`);
  check((await stored(page)).split("\n").filter((line) => line.startsWith("c\t2026W\t148370-")).length === 2, `two ticks kept: ${JSON.stringify(await stored(page))}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-ticked.png` });
  await page.click("#preview.is-termin .gs.pick >> nth=1");
  await until(page, "one tick taken back", () => [...document.querySelectorAll("#preview.is-termin .gs.pick")].filter((tile) => tile.getAttribute("aria-checked") === "true").length === 1);
  week = await practicals(page);
  check(week.length === 1 && !week[0], `one group left: ${JSON.stringify(week)}`);

  // The next date, then this date alone hidden and shown again.
  const before = new URL(page.url()).searchParams.get("on");
  await page.click("#preview.is-termin .tk-date a.pg >> nth=-1");
  await until(page, "the next date", (was) => new URL(location.href).searchParams.get("on") && new URL(location.href).searchParams.get("on") !== was, before);
  await page.click("#preview.is-termin [data-action='hide-date']");
  await until(page, "the date hidden", () => onPage("#preview.is-termin [data-action='hide-date']")?.getAttribute("aria-pressed") === "true");
  check((await stored(page)).split("\n").some((line) => line.startsWith("h\t2026W\t148370-")), `the date kept as hidden: ${JSON.stringify(await stored(page))}`);
  check((await view(page)).pill === "ausgeblendet", `the pill: ${JSON.stringify(await view(page))}`);
  await page.click("#preview.is-termin [data-action='hide-date']");
  await until(page, "the date shown again", () => onPage("#preview.is-termin [data-action='hide-date']")?.getAttribute("aria-pressed") === "false");
  check(!(await stored(page)).includes("\nh\t"), `no date hidden any more: ${JSON.stringify(await stored(page))}`);

  // The whole Praktikum off and on again.
  await page.click("#preview.is-termin .tg");
  await until(page, "the Praktikum hidden", () => ![...document.querySelectorAll(".sp-body .week .slot")].some((slot) => slot.textContent.includes("Prak PP")));
  await page.click("#preview.is-termin .tg");
  await until(page, "the Praktikum back", () => [...document.querySelectorAll(".sp-body .week .slot")].some((slot) => slot.textContent.includes("Prak PP")));

  // „Zum Modul": the module beside the plan, pointing at the Termin; Esc closes it.
  await page.click("#preview.is-termin a.mrow");
  await until(page, "the module beside the plan", () => Boolean(onPage("#preview.is-module:not(.is-termin)")) && /[?&]open=12102/.test(location.search) && /[?&]row=148370-/.test(location.search));
  await page.goBack();
  await until(page, "back at the Termin", () => Boolean(onPage("#preview.is-termin .tk")));
  await page.keyboard.press("Escape");
  await until(page, "Esc closes it", () => !onPage("#preview.is-termin") && !/termin=/.test(location.search));
  check(await page.evaluate(() => window.__marker === 1), "the Stundenplan was loaded again");
  await context.close();
}

// ---------- the start page ----------
{
  const { page, context } = await open("/");
  await until(page, "a Termin of the day", () => Boolean(onPage(".day-ev[href*='termin='], .day-wk-col a[href*='termin=']")));
  const href = await page.evaluate(() => onPage(".day-ev[href*='termin=']")?.getAttribute("href") ?? onPage(".day-wk-col a[href*='termin=']")?.getAttribute("href") ?? "");
  check(/^\/\?termin=\d+-[0-9a-f]{5}&on=\d{4}-\d\d-\d\d$/.test(href), `its link: „${href}"`);
  await page.click(`[href="${href}"] >> visible=true`);
  await until(page, "the Termin over the start page", () => Boolean(onPage(".day-termin #preview.is-termin .tk")));
  if (shots) await page.screenshot({ path: `${shots}/termin-home.png` });
  await page.click(".day-termin [data-action='close-detail']");
  await until(page, "the start page again", () => !onPage(".day-termin #preview") && location.search === "");
  check(await page.evaluate(() => window.__marker === 1), "the start page was loaded again");
  await context.close();
}

// ---------- „Teilen" and „Ganze Seite" ----------
{
  const { page, context } = await open(`/studyplan?termin=${praktikum}&on=2026-11-02`);
  await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin: base });
  await until(page, "the Termin beside the plan", () => Boolean(onPage("#preview.is-termin .tk")));
  const full = await page.evaluate(() => onPage("#preview.is-termin .tm-full")?.getAttribute("href") ?? "");
  check(full === `/studyplan?termin=${praktikum}&on=2026-11-02&full=1`, `„Ganze Seite": „${full}"`);
  // ← and →: the date after and back, one a press; in the search → moves in the text.
  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press("ArrowRight");
  await until(page, "→: the next date", () => /[?&]on=2026-11-09/.test(location.search) && Boolean(onPage("#preview.is-termin .tk-date > b")?.textContent.includes("09.11.")));
  await page.keyboard.press("ArrowLeft");
  await until(page, "←: the date before", () => /[?&]on=2026-11-02/.test(location.search) && Boolean(onPage("#preview.is-termin .tk-date > b")?.textContent.includes("02.11.")));
  await page.focus("#topsearch");
  await page.keyboard.press("ArrowRight");
  await page.waitForTimeout(300);
  check(/[?&]on=2026-11-02/.test(await page.evaluate(() => location.search)), "→ in the search stays in the search");
  // `?` from within the Termin: the overview, „Im Termin" first.
  await page.focus("#preview.is-termin .tk-date a.pg >> nth=-1");
  await page.keyboard.press("?");
  await until(page, "the keys of a Termin", () => document.querySelector(".keys-dialog[open] .keys-group.here h3")?.textContent.startsWith("Im Termin"));
  await page.keyboard.press("Escape");
  await until(page, "the overview closed", () => !document.querySelector(".keys-dialog[open]") && Boolean(onPage("#preview.is-termin .tk")));
  await page.evaluate(() => document.activeElement?.blur());
  // „Teilen": the Termin's own page on the day shown, copied, and said so.
  await page.click("#preview.is-termin .tm-share");
  await until(page, '„Link kopiert"', () => onPage("#preview.is-termin .tm-share")?.textContent.includes("Link kopiert"));
  const copied = await page.evaluate(() => navigator.clipboard.readText()).catch((error) => String(error));
  check(copied === `${base}/event/${praktikum}/2026-11-02`, `„Teilen" copies the Termin's page: „${copied}"`);
  // V: the Termin fills the plan's page, in two columns, „Im Stundenplan" still there.
  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press("v");
  await until(page, "the Termin filling the page", (href) => location.pathname + location.search === href && Boolean(onPage(".is-termin.tm-filling .tk")), full);
  const columns = await page.evaluate(() => {
    const body = onPage(".is-termin.tm-filling .dbody.tm");
    return { columns: body ? getComputedStyle(body).gridTemplateColumns.split(" ").length : 0, adjust: Boolean(body?.querySelector(".tm-sec.cal .gs.pick")), back: onPage(".is-termin.tm-filling .hero-top > a.at-head")?.getAttribute("href") ?? "" };
  });
  check(columns.columns === 2 && columns.adjust && columns.back === `/studyplan?termin=${praktikum}&on=2026-11-02`, `the whole page: ${JSON.stringify(columns)}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-full.png` });
  // The next date: still the whole page; Esc: beside the plan again, on that day.
  await page.click(".is-termin.tm-filling .tk-date a.pg >> nth=-1");
  await until(page, "the next date, filling the page", () => /[?&]on=2026-11-09/.test(location.search) && /[?&]full=1/.test(location.search) && Boolean(onPage(".is-termin.tm-filling .tk")));
  await page.keyboard.press("Escape");
  await until(page, "Esc: beside the plan again", () => !/full=/.test(location.search) && /[?&]on=2026-11-09/.test(location.search) && Boolean(onPage("#preview.is-termin .tk")));
  check(await page.evaluate(() => window.__marker === 1), "the Stundenplan was loaded again");
  await context.close();
}
{
  // Over the start page: „Ganze Seite" fills it, „Zurück" brings the Termin back over it.
  const { page, context } = await open("/");
  await until(page, "a Termin of the day", () => Boolean(onPage(".day-ev[href*='termin='], .day-wk-col a[href*='termin=']")));
  const href = await page.evaluate(() => onPage(".day-ev[href*='termin=']")?.getAttribute("href") ?? onPage(".day-wk-col a[href*='termin=']")?.getAttribute("href") ?? "");
  await page.click(`[href="${href}"] >> visible=true`);
  await until(page, "the Termin over the start page", () => Boolean(onPage(".day-termin #preview.is-termin .tk")));
  await page.click(".day-termin .tm-full");
  await until(page, "the start page filled", () => /[?&]full=1/.test(location.search) && Boolean(onPage(".day-termin.filling .is-termin.tm-filling .tk")) && !onPage(".day-hero")?.offsetParent);
  if (shots) await page.screenshot({ path: `${shots}/termin-home-full.png` });
  await page.click(".day-termin.filling .hero-top > a.at-head");
  await until(page, "the Termin over the start page again", (href) => location.pathname + location.search === href && Boolean(onPage(".day-termin #preview.is-termin .tk")) && Boolean(onPage(".day-hero")?.offsetParent), href);
  check(await page.evaluate(() => window.__marker === 1), "the start page was loaded again");
  await context.close();
}

// ---------- a phone ----------
{
  const { page, context } = await open("/studyplan", { width: 390, height: 844 });
  await until(page, "the week", () => document.querySelectorAll(".sp-body .week .slot").length > 0);
  // A slot of the week in view (the carousel holds the weeks beside it too).
  const tapped = await page.evaluate(() => {
    const inView = (el) => { const r = el.getBoundingClientRect(); return r.width > 0 && r.left >= 0 && r.right <= innerWidth; };
    const slot = allOnPage(".sp-body [href*='termin=']").find(inView);
    slot?.click();
    return Boolean(slot);
  });
  check(tapped, "a Termin of the week in view");
  await until(page, "the sheet", () => {
    const sheet = onPage("#preview.is-termin");
    return Boolean(sheet) && getComputedStyle(sheet).position === "fixed";
  });
  // „Teilen" there; „Ganze Seite" not: all up the sheet is the whole screen already.
  const tools = await page.evaluate(() => ({ share: Boolean(onPage("#preview.is-termin .tm-share")?.getClientRects().length), full: Boolean(onPage("#preview.is-termin .tm-full")?.getClientRects().length) }));
  check(tools.share && !tools.full, `the sheet's head: ${JSON.stringify(tools)}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-phone.png` });
  await context.close();
}

// ---------- to look up: beside the module's page, and its own page ----------
// The view as it reads, beside a page or as one: the head, the groups' dots, the foot, the ways out.
const lookUp = (page, where) => page.evaluate((where) => {
  const panel = onPage(where);
  const text = (selector) => panel?.querySelector(selector)?.textContent.replace(/\s+/g, " ").trim() ?? "";
  return {
    title: text(".tm-title"),
    out: text(".np"),
    dots: [...(panel?.querySelectorAll(".gs") ?? [])].map((tile) => tile.querySelector("i.st")?.className.replace("st ", "") ?? ""),
    foot: text(".grp-f > span"),
    adjust: panel?.querySelector(".grp-f a.go2")?.getAttribute("href") ?? "",
    full: panel?.querySelector(".tm-full")?.getAttribute("href") ?? "",
    file: panel?.querySelector("a.tm-big")?.getAttribute("download") ?? "",
    blob: (panel?.querySelector("a.tm-big")?.getAttribute("href") ?? "").startsWith("blob:"),
    day: [...(panel?.querySelectorAll(".rb") ?? [])].map((bar) => bar.className),
    // The head, the segments and where the groups stand: what a pick must leave as it is.
    head: text(".hero"),
    events: [...(panel?.querySelectorAll(".tm-evs > a") ?? [])].map((a) => (a.getAttribute("aria-checked") === "true" ? "*" : "") + a.textContent.trim()),
    groupsAt: Math.round(panel?.querySelector(".tm-pick .grp")?.getBoundingClientRect().top ?? -1),
    ticketAt: Math.round(panel?.querySelector(".tk")?.getBoundingClientRect().top ?? -1),
    when: text(".tk-date > b") + " " + text(".tk-time"),
  };
}, where);
// A line of a module's Termine (they stand folded) as the visitor follows it.
const follow = async (page, prefix) => {
  const href = await page.evaluate((prefix) => onPage(`a.ev[href^="${prefix}"]`)?.getAttribute("href") ?? "", prefix);
  check(href.startsWith(prefix) && await page.evaluate((href) => onPage(`a.ev[href="${href}"]`)?.getAttribute("rel") === "nofollow", href), `a Termin of the module's page opens beside it: „${href}"`);
  await page.evaluate((href) => onPage(`a.ev[href="${href}"]`)?.click(), href);
  return href;
};
{
  // The Praktikum, in the plan: beside its module's page; „Ganze Seite" its own page; Back.
  const { page, context } = await open("/catalog/module/12102");
  await until(page, "the module's Termine", () => Boolean(onPage("a.ev[href^='/catalog/module/12102?termin=148370-']")));
  const href = await follow(page, "/catalog/module/12102?termin=148370-");
  await until(page, "the Termin beside the module's page", (href) => location.pathname + location.search === href && Boolean(onPage("#preview.is-termin .tk")) && Boolean(onPage("#preview.is-termin .grp-f")), href);
  let v = await lookUp(page, "#preview.is-termin");
  check(v.title === "Programmierpraktikum" && !v.out && !v.file, `in the plan: ${JSON.stringify(v)}`);
  check(v.dots.length === 4 && v.dots.includes("ok") && v.dots.every((dot) => dot === "ok" || dot === "no"), `the groups measured against the plan: ${JSON.stringify(v)}`);
  check(v.foot === "Noch keine in deinem Stundenplan" && /^\/studyplan\?sem=2026W&termin=148370-[0-9a-f]{5}&on=\d{4}-\d\d-\d\d$/.test(v.adjust), `„Anpassen ›": ${JSON.stringify(v)}`);
  check(v.day.length > 1, `the plan's day around it: ${JSON.stringify(v)}`);
  check(/^\/event\/148370-[0-9a-f]{5}\/\d{4}-\d\d-\d\d$/.test(v.full), `„Ganze Seite" is its own page: ${JSON.stringify(v)}`);
  check(v.events.join(" | ") === "Vorlesung | *Praktikum" && v.groupsAt >= 0 && v.groupsAt < v.ticketAt, `the events and the groups above the ticket: ${JSON.stringify(v)}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-beside-module.png` });
  // A group picked (owner, 2026-10-27: „Wenn man solche Buttons hat, dann ändern die nur den Inhalt
  // unter ihnen"): the ticket under the groups is the group's, nothing above them changes.
  const second = await page.evaluate(() => onPage("#preview.is-termin .tm-pick .gs:not(.this)")?.getAttribute("href") ?? "");
  await page.click(`#preview.is-termin .tm-pick .gs[href="${second}"]`);
  await until(page, "the second group", (second) => location.pathname + location.search === second && onPage("#preview.is-termin .tm-pick .gs.this")?.getAttribute("href") === second, second);
  const picked = await lookUp(page, "#preview.is-termin");
  check(picked.head === v.head && picked.events.join() === v.events.join() && picked.groupsAt === v.groupsAt && picked.when !== v.when, `a group picked changes the ticket alone: ${JSON.stringify([v, picked])}`);
  // The lecture's segment: the lecture's ticket under the head, which stays.
  await page.click("#preview.is-termin .tm-evs > a:not([aria-checked='true'])");
  await until(page, "the lecture", () => onPage("#preview.is-termin .tm-evs > a[aria-checked='true']")?.textContent.trim() === "Vorlesung" && !onPage("#preview.is-termin .tm-pick .grp"));
  const lecture = await lookUp(page, "#preview.is-termin");
  check(lecture.head === v.head && lecture.when !== v.when, `the lecture's segment changes what stands under it: ${JSON.stringify(lecture)}`);
  await page.goBack();
  await page.goBack();
  await until(page, "back on the first group", (href) => location.pathname + location.search === href, href);
  // → beside the page: the date after, in the module page's address.
  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press("ArrowRight");
  await until(page, "→ beside the module's page", () => location.pathname === "/catalog/module/12102" && /[?&]on=\d{4}-\d\d-\d\d/.test(location.search) && Boolean(onPage("#preview.is-termin .tk")));
  await page.goBack();
  await until(page, "back to the first date", (href) => location.pathname + location.search === href, href);
  // „Ganze Seite": the Termin's own page, in two columns.
  await page.click("#preview.is-termin .tm-full");
  await until(page, "the Termin's own page", () => location.pathname.startsWith("/event/148370-") && Boolean(onPage(".is-termin.tm-page .tk")));
  v = await lookUp(page, ".is-termin.tm-page");
  check(v.title === "Programmierpraktikum" && v.foot === "Noch keine in deinem Stundenplan", `its own page: ${JSON.stringify(v)}`);
  check(await page.evaluate(() => getComputedStyle(onPage(".is-termin.tm-page .dbody.tm")).gridTemplateColumns.split(" ").length === 2), "its own page in two columns");
  check(await page.evaluate(() => /Programmierpraktikum/.test(document.title)), `the title: ${await page.title()}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-page.png` });
  // → on its own page: the date after, the day in its address.
  const shownDay = await page.evaluate(() => onPage(".is-termin.tm-page .tk-date > b")?.textContent ?? "");
  const own = await page.evaluate(() => location.pathname);
  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press("ArrowRight");
  await until(page, "→ on the Termin's page", ([own, day]) => location.pathname !== own && /^\/event\/148370-[0-9a-f]{5}\/\d{4}-\d\d-\d\d$/.test(location.pathname) && onPage(".is-termin.tm-page .tk-date > b")?.textContent !== day, [own, shownDay]);
  await page.goBack();
  await until(page, "back on its first date", (own) => location.pathname === own, own);
  // „Anpassen ›": the Termin in the Stundenplan; Back, Back: beside the module's page again.
  await page.click(".is-termin.tm-page .grp-f a.go2");
  await until(page, "the Termin in the Stundenplan", () => location.pathname === "/studyplan" && Boolean(onPage("#preview.is-termin .tk")) && Boolean(onPage("#preview.is-termin .gs.pick")));
  await page.goBack();
  await until(page, "back on the Termin's page", () => Boolean(onPage(".is-termin.tm-page .tk")));
  await page.goBack();
  await until(page, "back beside the module's page", (href) => location.pathname + location.search === href && Boolean(onPage("#preview.is-termin .tk")), href);
  // Esc: the module's page alone.
  await page.keyboard.press("Escape");
  await until(page, "Esc: the module's page alone", () => location.pathname === "/catalog/module/12102" && !/termin=/.test(location.search) && !onPage("#preview.is-termin"));
  check(await page.evaluate(() => window.__marker === 1), "the module page was loaded again");
  await context.close();
}
{
  // Enterprise Ressource Planning's exercise, outside the plan: beside its module's page.
  const { page, context } = await open("/catalog/module/12612");
  await until(page, "the module's Termine", () => Boolean(onPage("a.ev[href^='/catalog/module/12612?termin=150294-']")));
  await follow(page, "/catalog/module/12612?termin=150294-");
  await until(page, "the Termin beside the module's page", () => /[?&]termin=150294-/.test(location.search) && Boolean(onPage("#preview.is-termin .tk")));
  await until(page, "the calendar file", () => (onPage("#preview.is-termin a.tm-big")?.getAttribute("href") ?? "").startsWith("blob:"));
  const v = await lookUp(page, "#preview.is-termin");
  check(v.out === "nicht im Plan" && !v.foot && !v.adjust, `outside the plan: ${JSON.stringify(v)}`);
  check(v.dots.length === 3 && v.dots.filter((dot) => dot === "no").length === 1, `one group meets the plan: ${JSON.stringify(v)}`);
  check(v.day.some((bar) => bar.includes(" me") && bar.includes(" out")) && v.day.length > 1, `hatched among the plan's day: ${JSON.stringify(v)}`);
  check(v.file.endsWith(".ics") && v.blob, `„In den Kalender": ${JSON.stringify(v)}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-beside-out.png` });
  await context.close();
}
{
  // A phone: the Termin a sheet over its module's page.
  const { page, context } = await open("/catalog/module/12102", { width: 390, height: 844 });
  await until(page, "the module's Termine", () => Boolean(onPage("a.ev[href^='/catalog/module/12102?termin=148370-']")));
  await follow(page, "/catalog/module/12102?termin=148370-");
  await until(page, "the sheet over the module's page", () => {
    const sheet = onPage("#preview.is-termin");
    return Boolean(sheet?.querySelector(".tk")) && getComputedStyle(sheet).position === "fixed";
  });
  if (shots) await page.screenshot({ path: `${shots}/termin-module-phone.png` });
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log("termin: ok");
