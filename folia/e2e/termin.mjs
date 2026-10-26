// A Termin in its own view (owner, 2026-10-26; docs/folia/frontend.md, „A Termin in its own view";
// folia/design/termin/konzepte.html, the fourth round), on canary's data of 2026W with Informatik
// B.Sc.'s first semester planned (Programmierpraktikum 148370 in four groups, Mo, Mi, Do 11:30 and
// Fr 13:45).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node termin.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// - The Stundenplan: a slot of the week opens its Termin beside the plan (`termin=`), its slot marked;
//   the view names the module, the kind and the title, the date with its arrows, the time and the
//   room; „Im Stundenplan" has the four groups as boxes, none ticked.
// - Two groups ticked, as many as one likes: both stand solid in the week, the others leave it, the
//   store keeps two ticks; the tick of one taken back leaves the other.
// - The arrow to the next date names it in the address; „Ausblenden" hides this date alone (the
//   store's `h` line, the pill „ausgeblendet"), „Zeigen" brings it back.
// - The event's switch hides the whole Praktikum and shows it again.
// - Esc closes it; „Zum Modul" opens the module beside the plan, pointing at the Termin.
// - The start page: a Termin of its day opens over the page (`/?termin=…&on=…`), and closing it
//   goes back to the page.
// - A phone: the view is a sheet over the plan.
// - The Termin's own page to look up (`/event/<key>`): from a line of the Praktikum's module page, in
//   the plan, its groups measured against it, „Anpassen ›" into the Stundenplan of its semester;
//   Back leaves it. Enterprise Ressource Planning's exercise (12612, outside the plan): „nicht im
//   Plan", its second group red against the plan's lecture, the Termin hatched among the plan's
//   day, „In den Kalender" a file of its own.
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
    kind: text(".tm-kind"),
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
  const { page, context } = await open("/studyplan");
  await until(page, "the week", () => document.querySelectorAll(".sp-body .week .slot").length > 0);
  const href = await page.evaluate(() => [...document.querySelectorAll(".sp-body .week [href*='termin=148370-']")].map((a) => a.getAttribute("href"))[0] ?? "");
  check(href.startsWith("/studyplan?termin=148370-"), `a slot of the Praktikum leads to its Termin: „${href}"`);
  await page.click(`.sp-body .week [href="${href}"]`);
  await until(page, "the Termin beside the plan", () => Boolean(onPage("#preview.is-termin .tk")));
  let v = await view(page);
  check(v.chip === "PP" && v.kind === "Praktikum" && v.title === "Programmierpraktikum", `the head: ${JSON.stringify(v)}`);
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
  if (shots) await page.screenshot({ path: `${shots}/termin-phone.png` });
  await context.close();
}

// ---------- the Termin's own page ----------
// The page as it reads: the head, the groups' dots, the foot, the ways out.
const lookUp = (page) => page.evaluate(() => {
  const panel = onPage(".is-termin.tm-page");
  const text = (selector) => panel?.querySelector(selector)?.textContent.replace(/\s+/g, " ").trim() ?? "";
  return {
    title: text(".tm-title"),
    out: text(".np"),
    dots: [...(panel?.querySelectorAll(".gs") ?? [])].map((tile) => tile.querySelector("i.st")?.className.replace("st ", "") ?? ""),
    foot: text(".grp-f > span"),
    adjust: panel?.querySelector(".grp-f a.go2")?.getAttribute("href") ?? "",
    file: panel?.querySelector("a.tm-big")?.getAttribute("download") ?? "",
    blob: (panel?.querySelector("a.tm-big")?.getAttribute("href") ?? "").startsWith("blob:"),
    day: [...(panel?.querySelectorAll(".rb") ?? [])].map((bar) => bar.className),
  };
});
// A line of a module's Termine (they stand folded) as the visitor follows it.
const follow = async (page, prefix) => {
  const href = await page.evaluate((prefix) => onPage(`a.ev[href^="${prefix}"]`)?.getAttribute("href") ?? "", prefix);
  check(href.startsWith(prefix) && await page.evaluate((href) => onPage(`a.ev[href="${href}"]`)?.getAttribute("rel") === "nofollow", href), `a Termin of the module page leads to its page: „${href}"`);
  await page.evaluate((href) => onPage(`a.ev[href="${href}"]`)?.click(), href);
  return href;
};
{
  const { page, context } = await open("/catalog/module/12102");
  await until(page, "the module's Termine", () => Boolean(onPage("a.ev[href^='/event/148370-']")));
  const href = await follow(page, "/event/148370-");
  await until(page, "the Termin's page", (href) => location.pathname === href && Boolean(onPage(".is-termin.tm-page .tk")), href);
  let v = await lookUp(page);
  check(v.title === "Programmierpraktikum" && !v.out && !v.file, `in the plan: ${JSON.stringify(v)}`);
  check(v.dots.length === 4 && v.dots.includes("ok") && v.dots.every((dot) => dot === "ok" || dot === "no"), `the groups measured against the plan: ${JSON.stringify(v)}`);
  check(v.foot === "Noch keine in deinem Stundenplan" && /^\/studyplan\?sem=2026W&termin=148370-[0-9a-f]{5}&on=\d{4}-\d\d-\d\d$/.test(v.adjust), `„Anpassen ›": ${JSON.stringify(v)}`);
  check(v.day.length > 1, `the plan's day around it: ${JSON.stringify(v)}`);
  check(await page.evaluate(() => /Programmierpraktikum/.test(document.title)), `the title: ${await page.title()}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-page.png` });
  await page.click(".is-termin.tm-page .grp-f a.go2");
  await until(page, "the Termin in the Stundenplan", () => location.pathname === "/studyplan" && Boolean(onPage("#preview.is-termin .tk")) && Boolean(onPage("#preview.is-termin .gs.pick")));
  await page.goBack();
  await until(page, "back on the Termin's page", () => Boolean(onPage(".is-termin.tm-page .tk")));
  await page.goBack();
  await until(page, "back on the module page", () => location.pathname === "/catalog/module/12102");
  check(await page.evaluate(() => window.__marker === 1), "the module page was loaded again");
  await context.close();
}
{
  const { page, context } = await open("/catalog/module/12612");
  await until(page, "the module's Termine", () => Boolean(onPage("a.ev[href^='/event/150294-']")));
  await follow(page, "/event/150294-");
  await until(page, "the Termin's page", () => location.pathname.startsWith("/event/150294-") && Boolean(onPage(".is-termin.tm-page .tk")));
  await until(page, "the calendar file", () => (onPage(".is-termin.tm-page a.tm-big")?.getAttribute("href") ?? "").startsWith("blob:"));
  const v = await lookUp(page);
  check(v.out === "nicht im Plan" && !v.foot && !v.adjust, `outside the plan: ${JSON.stringify(v)}`);
  check(v.dots.length === 3 && v.dots.filter((dot) => dot === "no").length === 1, `one group meets the plan: ${JSON.stringify(v)}`);
  check(v.day.some((bar) => bar.includes(" me") && bar.includes(" out")) && v.day.length > 1, `hatched among the plan's day: ${JSON.stringify(v)}`);
  check(v.file.endsWith(".ics") && v.blob, `„In den Kalender": ${JSON.stringify(v)}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-page-out.png` });
  await context.close();
}

await browser.close();
if (problems.length) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log("termin: ok");
