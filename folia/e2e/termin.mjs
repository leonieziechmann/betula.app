// A Termin in its card (owner, 2026-11-17: „wenn man auf einen termin klickt, dass dann infos kommen die
// halt nicht nur für den Termin sind. Das muss auch angepasst werden so dass das mehr wie bei google
// calendar"; docs/folia/frontend.md, „A Termin in its own view"), on canary's data of 2026W with
// Informatik B.Sc.'s first semester planned (EvS, EEG, PP, MIT1: the Programmierpraktikum 148370 in four
// groups, Mo, Mi, Do 11:30 and Fr 13:45; the lecture of EvS 148701 on Di 11:30 and Fr 09:15).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node termin.mjs [--only=card,hide,peel,place,keys,overview,edit,start,phone,lookup]
// - The Stundenplan while looking: its slots carry no buttons, a hatched group too leads to its Termin
//   (`termin=`). A click on a slot opens the card of that one Termin beside its slot (never over it,
//   never outside the window, its top at the slot's top or its foot 16 px over the window's edge), the
//   slot marked: the module's chip, the kind, the title, the series („wöchentlich · 15 Termine · noch
//   8"), the ticket, nothing that picks (no events, groups, „Reihe", day, „Zum Modul"), the tools
//   „Ändern" · the eye „Ausblenden" · „⋯" · „×", the one way on „Alle Termine von PP" with V.
// - A hatched group (C.3 b): „Eine von 4 Praktikumsgruppen · noch keine angehakt", „Diese Gruppe
//   anhaken" with the keyboard and „Vergleichen ›" into the choice in focus; Enter ticks it, the card
//   closes, the week has one solid group, the note „Prak PP Mo 11:30 angehakt" brings the four back
//   with „Rückgängig". Ticked (c): „Angehakt · 1 von 4 Praktikumsgruppen" and „Haken wegnehmen", which
//   keeps the card. „Ausblenden" of a group: „Diese Gruppe" · „Praktikum ganz", no single date.
// - „Ausblenden" of a firm lecture (a): „Nur am Fr …" · „Alle 15" · „Vorlesung ganz" (auch Di 11:30),
//   the first with the keyboard; each closes the card with its note („VL EvS am Fr … ausgeblendet",
//   „VL EvS Fr 09:15 ausgeblendet", „Vorlesung EvS ausgeblendet") and „Rückgängig" takes back its line;
//   a hidden date says so under the ticket with „Einblenden", which keeps the card.
// - Esc peels one layer every time: „Ändern", „Ausblenden", „⋯" and the note each back to the card, the
//   card's top and width kept; then Esc closes it. A click on another slot swaps the card in its place
//   (the address replaced, no step more), and Back leaves in one step.
// - Where it stands, for every slot of the week at 1440 × 900, again after the window shrinks to
//   1180 × 820, and opened by its address.
// - Keys: ← and → step the date (the card does not move), `?` shows „Im Termin" first, „⋯" → „Teilen"
//   copies the Termin's page; V fills the page with the overview („Alle Termine von …", `&full=1`, two
//   columns, its switches, the Termin's as a whole „Fr 09:15 im Stundenplan" between the event's and
//   the date's), whose „Zurück" and Esc lead to the plan, not to the card.
// - While editing (`edit=1`): what the card hides answers in the slot (faint) and no note comes; a
//   faint slot's card (d1) says „Ausgeblendet" / „Vorlesung ausgeblendet" with „Einblenden" and no eye;
//   a faint group beside a ticked one (d2): „Andere Gruppe gewählt · Mo 11:30", „Nur diese Gruppe",
//   „Auch diese".
// - The start page: a Termin of the day opens the same card in the middle of the page, its way on into
//   the Stundenplan's overview, its eye with the note and „Rückgängig" over the start page; „×" leads
//   back to `/`; `/?termin=…&full=1` still fills the start page.
// - A phone: the card a sheet as wide as the screen, its tools 44 px, its way on shown, the large action
//   its last line; the next date takes the history's entry, Back leaves the Termin; the Termin's own
//   page scrolls under a finger.
// - To look up: a line of the Praktikum's module page opens the Termin in the dialog over the page
//   (`?termin=<key>`), in the plan, its groups above the ticket measured against it, „Anpassen ›"
//   under it into the Stundenplan of its semester; a group picked changes the ticket under the
//   groups and nothing above them, a date picked by its dot in „Reihe" the ticket alone, the
//   lecture's segment the lecture's ticket; → its next date;
//   „Ganze Seite" is its own page (`/event/<key>/<day>`) in two columns, → there too; Back leads back
//   to the dialog over the module's page, Esc closes it.
//   Enterprise Ressource Planning's exercise (12612, outside the plan): „nicht im Plan", its second
//   group red against the plan's lecture, the Termin hatched among the plan's day, „In den
//   Kalender" a file of its own. On a phone the Termin is a sheet over its module's page.
// History: 2026-11-17, the card. Gone with the dialog of the Stundenplan (now the overview's): the
// events as segments, the groups' tiles, „Reihe", the day, „Im Stundenplan" in the card, two groups
// ticked from within it (the overview and the slots' „+" do that now), „Zum Modul" (the overview has
// it), „Ganze Seite" of the start page filling the start page (its way on is the Stundenplan's
// overview now). „Termine" (`view=dates`) is no longer the way to the Praktikum's key (its link left
// the sidebar on 2026-11-04): the week is. Before every key the held picture of the page before is
// waited out (← after → was dropped while it stood). Then: a card low in the window keeps its top edge
// and stands higher, as far as the card at its tallest needs (it was anchored by its foot 16 px over
// the window's edge before).
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
const only = (process.argv.find((arg) => arg.startsWith("--only=")) ?? "").slice("--only=".length);
const block = (name) => !only || only.split(",").includes(name);
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
const FS1 = ["m\t2026W\t12104\t1790000000\t", "m\t2026W\t12107\t1790000000\t", "m\t2026W\t12102\t1790000000\t", "m\t2026W\t11112\t1790000000\t", ""].join("\n");
const MINE_FS1 = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n";
const wide = { width: 1440, height: 900 };
const phone = { width: 390, height: 844, mobile: true };

const open = async (path, viewport = { width: 1500, height: 1000 }, plan = FS1) => {
  const { mobile, ...size } = viewport;
  const context = await browser.newContext({ viewport: size, ...(mobile ? { isMobile: true, hasTouch: true } : {}) });
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  await context.addInitScript(([plan, mine, planText, mineText]) => { if (!localStorage.getItem(plan)) { localStorage.setItem(plan, planText); localStorage.setItem(mine, mineText); } }, [PLAN, MINE, plan, MINE_FS1]);
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
// A click that may find nothing: said, not thrown.
const click = async (page, selector, name = selector) => {
  try {
    await page.click(selector, { timeout: 5000 });
    return true;
  } catch {
    problems.push(`${name}: nothing to click (${page.url()})`);
    return false;
  }
};
// The held picture of the page before is gone (a key pressed while it stands is dropped).
const idle = (page) => page.waitForFunction(() => !document.querySelector(".pending-page.held"), null, { timeout: 2000 }).catch(() => {});
const stored = (page) => page.evaluate((key) => localStorage.getItem(key) ?? "", PLAN);
const lines = async (page, tag) => (await stored(page)).split("\n").filter((line) => line.startsWith(`${tag}\t`));
const here = (page) => page.evaluate(() => location.pathname + location.search);
// The slots of the week shown: [what, its key, its classes, its day (1 = Monday), its time].
const slots = (page) => page.evaluate(() => allOnPage(".sp-week .week").filter((week) => {
  const rect = week.getBoundingClientRect();
  return rect.left >= -1 && rect.right <= innerWidth + 1;
}).flatMap((week) => [...week.querySelectorAll("a.slot")].map((slot) => ({
  what: slot.querySelector(".l")?.textContent.trim().split(/\s+/).slice(0, 2).join(" ") ?? slot.textContent.trim().slice(0, 8),
  key: /termin=(\d+-[0-9a-f]{5})/.exec(slot.getAttribute("href") ?? "")?.[1] ?? "",
  href: slot.getAttribute("href") ?? "",
  classes: slot.className,
  day: [...week.querySelectorAll(".col")].indexOf(slot.closest(".col")) + 1,
  from: /(\d\d:\d\d)–/.exec(slot.textContent)?.[1] ?? "",
}))));
// The box of the slot of `key` in the week shown.
const slotBox = (page, key) => page.evaluate((key) => {
  const slot = allOnPage(`.sp-week a.slot[href*="termin=${key}"]`).find((slot) => {
    const rect = slot.getBoundingClientRect();
    return rect.width > 0 && rect.left >= 0 && rect.right <= innerWidth;
  });
  if (!slot) return null;
  const rect = slot.getBoundingClientRect();
  const col = slot.closest(".col")?.getBoundingClientRect();
  return { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, colLeft: col?.left ?? rect.left, colRight: col?.right ?? rect.right };
}, key);
// A click in the middle of a slot, as a hand makes it (over the dim of a card that stands, too).
const clickSlot = async (page, key) => {
  const box = await slotBox(page, key);
  if (!box) {
    problems.push(`the slot of ${key}: not in the week (${page.url()})`);
    return false;
  }
  await page.mouse.click((box.left + box.right) / 2, (box.top + box.bottom) / 2);
  return true;
};
// The card as it reads and where it stands.
const card = (page) => page.evaluate(() => {
  const dialog = onPage("dialog[open]");
  const panel = dialog?.querySelector(".tm-dialog.tm-card");
  const text = (selector) => panel?.querySelector(selector)?.textContent.replace(/\s+/g, " ").trim() ?? "";
  const rect = dialog?.getBoundingClientRect();
  const active = document.activeElement;
  const big = panel?.querySelector(".tm-act .tm-big");
  return {
    open: Boolean(panel),
    chip: text(".tm-mod"),
    kind: text(".tm-kind"),
    tags: [...(panel?.querySelectorAll(".tm-bar .np") ?? [])].map((tag) => tag.textContent.trim()),
    title: text(".tm-title"),
    when: text(".tm-when"),
    off: text(".tm-off"),
    date: text(".tk-date > b"),
    time: text(".tk-time"),
    room: text(".tk-room .plate"),
    tools: [...(panel?.querySelectorAll(".tm-tools [data-action]") ?? [])].map((tool) => tool.dataset.action),
    close: Boolean(panel?.querySelector(".tm-tools .tm-close")),
    eye: panel?.querySelector(".tm-tools [data-action='hide'] use")?.getAttribute("href")?.split("#")[1] ?? "",
    // What picks another Termin or tells of the module: the overview's, never the card's.
    picks: [".tm-evs", ".tm-pick", ".grp", ".sdots", ".rib", ".tm-sec", ".tm-src", "a.mrow", ".tm-full"].filter((selector) => panel?.querySelector(selector)),
    who: text(".who"),
    lines: text(".tm-lines"),
    group: text(".tm-group"),
    big: big ? [big.dataset.action, big.textContent.trim(), big.getAttribute("aria-pressed") ?? ""] : null,
    also: text(".tm-act .tm-also"),
    untick: text(".tm-act .tm-untick"),
    compare: panel?.querySelector(".tm-act a[data-action='compare']")?.getAttribute("href") ?? "",
    all: text(".tm-ways .tm-all > span"),
    allHref: panel?.querySelector(".tm-ways .tm-all")?.getAttribute("href") ?? "",
    focus: active === dialog ? "dialog" : active?.dataset?.action ?? active?.id ?? active?.tagName ?? "",
    box: rect ? { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom, width: rect.width } : null,
    placed: Boolean(dialog?.classList.contains("placed")),
    x: dialog?.style.getPropertyValue("--x") ?? "",
    address: location.pathname + location.search,
  };
});
// The card standing still: placed, not animating, the same box twice 100 ms apart.
const settled = async (page, name) => {
  try {
    await page.waitForFunction(() => {
      const dialog = onPage("dialog[open]");
      if (!dialog?.querySelector(".tm-card") || dialog.getAnimations({ subtree: true }).some((a) => a.playState === "running")) return false;
      const rect = JSON.stringify(dialog.getBoundingClientRect());
      const same = window.__cardRect === rect;
      window.__cardRect = rect;
      return same;
    }, null, { timeout: 10000, polling: 100 });
  } catch {
    problems.push(`${name}: the card did not stand still (${page.url()})`);
  }
};
// Where the card stands against its slot: never over it, nor over its day's column, inside the window
// and right of the rail, its top at the slot's top, or higher where the card at its tallest (its head
// and its tallest body) would not end 16 px over the window's edge from there — never lower than the
// slot's top, never above 16 px under the window's top; no card is anchored by its foot.
const stands = (page, key) => page.evaluate((key) => {
  const dialog = onPage("dialog[open]")?.getBoundingClientRect();
  const slot = allOnPage(`.sp-week a.slot[href*="termin=${key}"]`).find((slot) => slot.getBoundingClientRect().width > 0);
  const box = slot?.getBoundingClientRect();
  const col = slot?.closest(".col")?.getBoundingClientRect();
  const rail = document.querySelector(".rail")?.getBoundingClientRect().right ?? 0;
  if (!dialog || !box || !col) return { missing: true };
  const over = !(dialog.right <= box.left || dialog.left >= box.right || dialog.bottom <= box.top || dialog.top >= box.bottom);
  const overCol = !(dialog.right <= col.left + 1 || dialog.left >= col.right - 1);
  const inside = dialog.left >= rail && dialog.right <= innerWidth && dialog.top >= 0 && dialog.bottom <= innerHeight;
  const top = Math.abs(dialog.top - box.top) <= 2 || (dialog.top < box.top && dialog.top >= 14);
  return { over, overCol, inside, top, dialog: [Math.round(dialog.left), Math.round(dialog.top), Math.round(dialog.right), Math.round(dialog.bottom)], slot: [Math.round(box.left), Math.round(box.top), Math.round(box.right), Math.round(box.bottom)], window: [innerWidth, innerHeight] };
}, key);
const fine = (at) => !at.missing && !at.over && !at.overCol && at.inside && at.top;
// The note of the Stundenplan (or the start page) with „Rückgängig".
const note = (page) => page.evaluate(() => onPage(".own-note .plan-toast-said")?.textContent.replace(/\s+/g, " ").trim() ?? "");
const cardOpen = (page, name, key) => until(page, name, (key) => Boolean(onPage("dialog[open] .tm-dialog.tm-card .tk")) && new URLSearchParams(location.search).get("termin") === key, key);
// Gone, and the page at rest: the picture of the page before (`.pending-page.held`) comes a moment
// after the address changed, and a click while it is held is lost (3 to 5 in 30 clicks right after
// Esc, none once the page had rested) — so the next step waits until no picture is held, nothing is
// busy and nothing settles for 120 ms.
const cardGone = async (page, name) => {
  if (!(await until(page, name, () => !onPage("dialog[open] .tm-dialog") && !/[?&]termin=/.test(location.search)))) return false;
  await page.evaluate(() => { window.__restSince = 0; });
  return until(page, `${name}: the page at rest`, () => {
    const quiet = !document.querySelector(".pending-page.held, main.content[aria-busy]") && !document.documentElement.hasAttribute("data-settling");
    if (!quiet) { window.__restSince = 0; return false; }
    window.__restSince ||= performance.now();
    return performance.now() - window.__restSince > 120;
  });
};

// The keys of the week (FS1): the Praktikum's four groups, the lecture of EvS on Friday and Tuesday.
const keys = {};
{
  const { page, context } = await open("/studyplan", wide);
  await until(page, "the week", () => allOnPage(".sp-week a.slot").length > 0);
  const week = await slots(page);
  const find = (what, day, from) => week.find((slot) => slot.what === what && slot.day === day && slot.from === from)?.key ?? "";
  Object.assign(keys, {
    prakMo: find("Prak PP", 1, "11:30"), prakMi: find("Prak PP", 3, "11:30"), prakDo: find("Prak PP", 4, "11:30"), prakFr: find("Prak PP", 5, "13:45"),
    evsFr: find("VL EvS", 5, "09:15"), evsDi: find("VL EvS", 2, "11:30"), mitMo: find("VL MIT1", 1, "13:45"), ppFr: find("VL PP", 5, "11:30"),
  });
  check(Object.values(keys).every(Boolean) && keys.prakMo.startsWith("148370-") && keys.evsFr.startsWith("148701-"), `the week's Termine: ${JSON.stringify(keys)}`);
  await context.close();
}
const praktikum = keys.prakMo;

// ---------- the card while looking: a hatched group ----------
if (block("card")) {
  const { page, context } = await open("/studyplan", wide);
  await until(page, "the week", () => allOnPage(".sp-week a.slot").length > 0);
  // Looking: no slot carries a button, and the hatched groups lead to their Termin, not into a choice.
  const looking = await page.evaluate(() => ({
    acts: allOnPage(".sp-week .slot-acts, .sp-week .slot-act").length,
    bar: Boolean(onPage(".sp-editbar")),
    groups: allOnPage(".sp-week a.slot.alt").filter((slot) => slot.textContent.includes("Prak PP")).map((slot) => slot.getAttribute("href")),
  }));
  check(looking.acts === 0 && !looking.bar, `while looking no slot has a button: ${JSON.stringify(looking)}`);
  check(looking.groups.length === 4 && looking.groups.every((href) => /^\/studyplan\?termin=148370-[0-9a-f]{5}$/.test(href)), `a hatched group of the Praktikum leads to its Termin: ${JSON.stringify(looking.groups)}`);
  const steps = await page.evaluate(() => history.length);
  await clickSlot(page, praktikum);
  await cardOpen(page, "the card of the Praktikum's Monday group", praktikum);
  await settled(page, "the card of the Monday group");
  let v = await card(page);
  check(v.chip === "PP" && v.kind === "Praktikum" && v.title === "Programmierpraktikum", `the head: ${JSON.stringify(v)}`);
  check(v.picks.length === 0, `nothing in the card picks another Termin or tells of the module: ${JSON.stringify(v.picks)}`);
  check(/^wöchentlich · 15 Termine · noch \d+$/.test(v.when), `the series: „${v.when}"`);
  check(/^Mo \d\d\.\d\d\./.test(v.date) && /^11:30/.test(v.time) && v.room.includes("0.03"), `the ticket: ${JSON.stringify([v.date, v.time, v.room])}`);
  check(v.tools.join() === "change,hide,more" && v.close && v.eye === "eye", `the tools „Ändern" · the eye · „⋯" · „×": ${JSON.stringify([v.tools, v.close, v.eye])}`);
  check(!v.who, `QIS names nobody at the Termin, so nobody is named: „${v.who}"`);
  check(v.group === "Eine von 4 Praktikumsgruppen · noch keine angehakt" && JSON.stringify(v.big) === JSON.stringify(["tick", "Diese Gruppe anhaken", "false"]) && v.focus === "tick", `the group and its one large action, with the keyboard: ${JSON.stringify([v.group, v.big, v.focus])}`);
  check(v.compare === "/studyplan?edit=1&wahl=e148370&vergleich=1", `„Vergleichen ›": „${v.compare}"`);
  check(v.all === "Alle Termine von PP" && v.allHref.endsWith("&full=1"), `the one way on: ${JSON.stringify([v.all, v.allHref])}`);
  check(await page.evaluate((key) => allOnPage(`.sp-week a.slot[href*="termin=${key}"]`).some((slot) => slot.getAttribute("aria-current") === "true"), praktikum), "its slot is marked");
  const at = await stands(page, praktikum);
  check(v.placed && fine(at), `the card beside its slot: ${JSON.stringify(at)}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-card-group.png` });

  // Enter ticks it: the card closes, one group solid, the note brings the four back.
  await page.keyboard.press("Enter");
  await cardGone(page, "Enter: the card closes");
  await until(page, "the group ticked, the note", (key) => {
    const ticks = (localStorage.getItem("betula.studyplan.v1") ?? "").split("\n").filter((line) => line.startsWith("c\t"));
    const groups = allOnPage(".sp-week a.slot").filter((slot) => slot.textContent.includes("Prak PP"));
    return ticks.join() === `c\t2026W\t${key}` && groups.length === 1 && !groups[0].classList.contains("alt") && onPage(".own-note .plan-toast-said")?.textContent.trim() === "Prak PP Mo 11:30 angehakt";
  }, praktikum);
  check(await page.evaluate(() => history.length) === steps + 1, "the card and its tick took more than one step of the history");
  await click(page, ".own-note .plan-toast-undo", "„Rückgängig\" of the tick");
  await until(page, "„Rückgängig\": the four hatched again", () => !(localStorage.getItem("betula.studyplan.v1") ?? "").includes("\nc\t") && allOnPage(".sp-week a.slot.alt").filter((slot) => slot.textContent.includes("Prak PP")).length === 4);

  // Ticked by the large action, its card again: „Angehakt", „Haken wegnehmen" keeps the card.
  await clickSlot(page, praktikum);
  await cardOpen(page, "the card again", praktikum);
  await click(page, "dialog[open] .tm-act .tm-big[data-action='tick']", "„Diese Gruppe anhaken\"");
  await until(page, "ticked by its button", (key) => (localStorage.getItem("betula.studyplan.v1") ?? "").includes(`c\t2026W\t${key}\n`) && !onPage("dialog[open] .tm-card"), praktikum);
  await clickSlot(page, praktikum);
  await cardOpen(page, "the card of the ticked group", praktikum);
  await settled(page, "the card of the ticked group");
  v = await card(page);
  check(v.group === "Angehakt · 1 von 4 Praktikumsgruppen" && v.untick === "Haken wegnehmen" && !v.big && v.compare, `a ticked group (c): ${JSON.stringify([v.group, v.untick, v.big, v.compare])}`);
  // „Ausblenden" of a ticked group offers the date too.
  await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye of the ticked group");
  await until(page, "„Ausblenden\" of a ticked group", () => allOnPage("dialog[open] .tm-hide [data-action]").map((row) => row.dataset.action).join() === "hide-date,hide-termin,hide-event,hide-cancel");
  await page.keyboard.press("Escape");
  await until(page, "Esc: the card of the ticked group again", () => !onPage("dialog[open] .tm-hide") && Boolean(onPage("dialog[open] .tm-act .tm-untick")));
  const said = await note(page);
  await click(page, "dialog[open] .tm-act .tm-untick", "„Haken wegnehmen\"");
  await until(page, "„Haken wegnehmen\": the card stays, the large action back", (key) => !(localStorage.getItem("betula.studyplan.v1") ?? "").includes("\nc\t") && new URLSearchParams(location.search).get("termin") === key && onPage("dialog[open] .tm-act .tm-big")?.textContent.trim() === "Diese Gruppe anhaken", praktikum);
  await until(page, "the keyboard on the large action again", () => document.activeElement === onPage("dialog[open] .tm-act .tm-big"));
  check([said, ""].includes(await note(page)), `taking the tick away needs no note: „${await note(page)}"`);

  // „Ausblenden" of a hatched group: the group or the Praktikum, never one date the plan does not hold.
  await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye of the group");
  await until(page, "„Ausblenden\" of a group", () => allOnPage("dialog[open] .tm-hide [data-action]").map((row) => row.dataset.action).join() === "hide-termin,hide-event,hide-cancel");
  const reach = await page.evaluate(() => allOnPage("dialog[open] .tm-hide .tm-hide-row").map((row) => row.innerText.replace(/\s+/g, " ").trim()));
  check(reach[0] === "Diese Gruppe die anderen 3 bleiben zur Wahl" && /^Praktikum ganz alle 4 Gruppen$/.test(reach[1] ?? ""), `what „Ausblenden" of a group offers: ${JSON.stringify(reach)}`);
  await click(page, "dialog[open] .tm-hide [data-action='hide-termin']", "„Diese Gruppe\"");
  await cardGone(page, "„Diese Gruppe\": the card closes");
  await until(page, "the group hidden, its note", (key) => (localStorage.getItem("betula.studyplan.v1") ?? "").split("\n").some((line) => line.startsWith("r\t2026W\t") && line.includes(key)) && onPage(".own-note .plan-toast-said")?.textContent.trim() === "Prak PP Mo 11:30 ausgeblendet" && allOnPage(".sp-week a.slot").filter((slot) => slot.textContent.includes("Prak PP")).length === 3, praktikum);
  await click(page, ".own-note .plan-toast-undo", "„Rückgängig\" of the group hidden");
  await until(page, "the group back", () => !(localStorage.getItem("betula.studyplan.v1") ?? "").includes("\nr\t") && allOnPage(".sp-week a.slot").filter((slot) => slot.textContent.includes("Prak PP")).length === 4);
  check(await page.evaluate(() => window.__marker === 1), "the Stundenplan was loaded again");
  await context.close();
}

// ---------- „Ausblenden" of a firm lecture, its reach and its notes ----------
if (block("hide")) {
  const { page, context } = await open("/studyplan", wide);
  await until(page, "the week", () => allOnPage(".sp-week a.slot").length > 0);
  const friday = keys.evsFr;
  const evs = () => page.evaluate(() => allOnPage(".sp-week a.slot").filter((slot) => slot.textContent.includes("VL EvS")).length);
  await clickSlot(page, friday);
  await cardOpen(page, "the card of VL EvS on Friday", friday);
  await settled(page, "the card of VL EvS");
  const first = await card(page);
  check(first.chip === "EvS" && first.kind === "Vorlesung" && first.title === "Entwicklung von Softwaresystemen" && !first.big && !first.group && first.tools.join() === "change,hide,more", `a firm lecture (a): ${JSON.stringify(first)}`);
  await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye");
  await until(page, "„Ausblenden\" in the card's body", () => Boolean(onPage("dialog[open] .tm-hide")));
  const offers = await page.evaluate(() => ({
    rows: allOnPage("dialog[open] .tm-hide [data-action]").map((row) => [row.dataset.action, row.innerText.replace(/\s+/g, " ").trim()]),
    focus: document.activeElement?.dataset?.action ?? "",
  }));
  check(offers.rows.map(([action]) => action).join() === "hide-date,hide-termin,hide-event,hide-cancel" && offers.focus === "hide-date", `what it offers, the first with the keyboard: ${JSON.stringify(offers)}`);
  check(/^Nur am Fr \d\d\.\d\d\. die anderen Termine bleiben$/.test(offers.rows[0]?.[1] ?? "") && offers.rows[1]?.[1] === "Alle 15 Fr 09:15 · wöchentlich" && offers.rows[2]?.[1] === "Vorlesung ganz auch Di 11:30" && offers.rows[3]?.[1] === "Abbrechen", `its reach: ${JSON.stringify(offers.rows)}`);
  const hidden = await card(page);
  check(hidden.box && Math.abs(hidden.box.top - first.box.top) < 1 && Math.abs(hidden.box.width - first.box.width) < 1, `„Ausblenden" keeps the card's top and width: ${JSON.stringify([first.box, hidden.box])}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-card-hide.png` });
  // Esc: the card; Esc: closed.
  await page.keyboard.press("Escape");
  await until(page, "Esc: back to the card", () => !onPage("dialog[open] .tm-hide") && Boolean(onPage("dialog[open] .tm-card .tk")));
  await page.keyboard.press("Escape");
  await cardGone(page, "Esc: the card closed");

  // „Nur am …": the date's line, its note, „Rückgängig".
  await clickSlot(page, friday);
  await cardOpen(page, "the card again", friday);
  const day = (await card(page)).date;
  await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye");
  await click(page, "dialog[open] .tm-hide [data-action='hide-date']", "„Nur am …\"");
  await cardGone(page, "„Nur am …\": the card closes");
  await until(page, "the date hidden, its note", ([key, day]) => (localStorage.getItem("betula.studyplan.v1") ?? "").split("\n").some((line) => line.startsWith(`h\t2026W\t${key}\t`)) && onPage(".own-note .plan-toast-said")?.textContent.trim() === `VL EvS am ${day} ausgeblendet`, [friday, day]);
  await click(page, ".own-note .plan-toast-undo", "„Rückgängig\" of the date");
  await until(page, "the date back", () => !(localStorage.getItem("betula.studyplan.v1") ?? "").includes("\nh\t"));
  // Hidden again: the card on that day says so under the ticket, „Einblenden" keeps the card.
  await clickSlot(page, friday);
  await cardOpen(page, "the card a third time", friday);
  await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye");
  await click(page, "dialog[open] .tm-hide [data-action='hide-date']", "„Nur am …\"");
  await cardGone(page, "the date hidden again");
  const line = (await lines(page, "h"))[0] ?? "";
  const date = line.split("\t")[3] ?? "";
  await page.evaluate(() => onPage(".own-note .wait-note-close, .own-note [aria-label='Schließen']")?.click());
  await clickSlot(page, friday);
  await cardOpen(page, "the card after the date was hidden", friday);
  // The card opened on another date names the hidden one in the series' line; the hidden date's own card.
  const series = await page.evaluate(() => ({ when: onPage("dialog[open] .tm-when")?.textContent.trim() ?? "", link: onPage("dialog[open] .tm-when a.tm-off-date")?.getAttribute("href") ?? "", on: new URLSearchParams(location.search).get("on") }));
  if (series.link) {
    check(series.when.includes("ausgeblendet: Fr") && series.link.includes(`on=${date}`), `the series names the hidden date: ${JSON.stringify(series)}`);
    await click(page, "dialog[open] .tm-when a.tm-off-date", "the hidden date in the series' line");
  } else {
    // The card opened on the hidden date itself.
    check(series.on === null || series.on === date, `the card after hiding a date: ${JSON.stringify(series)}`);
  }
  await until(page, "the hidden date's card", (date) => new URLSearchParams(location.search).get("on") === date || !location.search.includes("on="), date);
  await until(page, "„Am … ausgeblendet · Einblenden\" under the ticket", () => /^Am Fr \d\d\.\d\d\. ausgeblendet ?Einblenden$/.test(onPage("dialog[open] .tm-date-off")?.textContent.replace(/\s+/g, " ").trim() ?? "") && Boolean(onPage("dialog[open] .tm-date-off [data-action='show-date']")));
  check((await card(page)).tools.join() === "change,hide,more", "the tools do not depend on the date shown");
  await click(page, "dialog[open] .tm-date-off [data-action='show-date']", "„Einblenden\" of the date");
  await until(page, "„Einblenden\": the line gone, the card stays", (key) => !(localStorage.getItem("betula.studyplan.v1") ?? "").includes("\nh\t") && !onPage("dialog[open] .tm-date-off") && new URLSearchParams(location.search).get("termin") === key, friday);

  // „Alle 15": the Termin as a whole, its note, „Rückgängig".
  await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye");
  await click(page, "dialog[open] .tm-hide [data-action='hide-termin']", "„Alle 15\"");
  await cardGone(page, "„Alle 15\": the card closes");
  await until(page, "the Termin hidden, its note", (key) => (localStorage.getItem("betula.studyplan.v1") ?? "").split("\n").some((line) => line.startsWith("r\t2026W\t") && line.includes(key)) && onPage(".own-note .plan-toast-said")?.textContent.trim() === "VL EvS Fr 09:15 ausgeblendet" && !allOnPage(`.sp-week a.slot[href*="termin=${key}"]`).length, friday);
  check(await evs() === 1, "Tuesday's lecture went too");
  await click(page, ".own-note .plan-toast-undo", "„Rückgängig\" of the Termin");
  await until(page, "the Termin back", (key) => !(localStorage.getItem("betula.studyplan.v1") ?? "").includes("\nr\t") && allOnPage(`.sp-week a.slot[href*="termin=${key}"]`).length > 0, friday);

  // „Vorlesung ganz": the event, both of its Termine.
  await clickSlot(page, friday);
  await cardOpen(page, "the card once more", friday);
  await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye");
  await click(page, "dialog[open] .tm-hide [data-action='hide-event']", "„Vorlesung ganz\"");
  await cardGone(page, "„Vorlesung ganz\": the card closes");
  await until(page, "the lecture hidden, its note", () => (localStorage.getItem("betula.studyplan.v1") ?? "").includes("e\t2026W\t148701") && onPage(".own-note .plan-toast-said")?.textContent.trim() === "Vorlesung EvS ausgeblendet" && !allOnPage(".sp-week a.slot").some((slot) => slot.textContent.includes("VL EvS")));
  await click(page, ".own-note .plan-toast-undo", "„Rückgängig\" of the lecture");
  await until(page, "the lecture back", () => !(localStorage.getItem("betula.studyplan.v1") ?? "").includes("\ne\t") && allOnPage(".sp-week a.slot").filter((slot) => slot.textContent.includes("VL EvS")).length === 2);
  check((await stored(page)) === FS1, `what is stored after every „Rückgängig": ${JSON.stringify(await stored(page))}`);
  check(await page.evaluate(() => window.__marker === 1), "the Stundenplan was loaded again");
  await context.close();
}

// ---------- Esc peels one layer every time; another slot swaps the card ----------
if (block("peel")) {
  const { page, context } = await open("/studyplan", wide);
  await until(page, "the week", () => allOnPage(".sp-week a.slot").length > 0);
  const friday = keys.evsFr;
  const steps = await page.evaluate(() => history.length);
  await clickSlot(page, friday);
  await cardOpen(page, "the card of VL EvS", friday);
  await settled(page, "the card of VL EvS");
  const first = await card(page);
  // „Ändern" takes the card's body, the card keeps its top and width; Esc: the card.
  await click(page, "dialog[open] .tm-tools [data-action='change']", "„Ändern\"");
  await until(page, "„Ändern\" in the card", () => Boolean(onPage("dialog[open] .tm-card #tm-change")));
  const changing = await card(page);
  check(Math.abs(changing.box.top - first.box.top) < 1 && Math.abs(changing.box.width - first.box.width) < 1, `„Ändern" keeps the card's top and width: ${JSON.stringify([first.box, changing.box])}`);
  // Typing in „Raum", then Esc: one layer, not the card.
  await page.click("dialog[open] #chg-room");
  await page.keyboard.type("X");
  await page.keyboard.press("Escape");
  await until(page, "Esc from „Ändern\": the card", () => !onPage("dialog[open] #tm-change") && Boolean(onPage("dialog[open] .tm-card .tk")));
  check(!(await stored(page)).includes("\nx\t"), "Esc saved the change");
  // „⋯": a menu over the body (the card grows by nothing); Esc: the card.
  await click(page, "dialog[open] .tm-tools [data-action='more']", "„⋯\"");
  await until(page, "„⋯\" as a menu", () => allOnPage("dialog[open] .tm-menu[role='menu'] [role='menuitem']").length === 4);
  const menu = await page.evaluate(() => ({ rows: allOnPage("dialog[open] .tm-menu [role='menuitem']").map((row) => row.dataset.action ?? row.textContent.trim()), expanded: onPage("dialog[open] [data-action='more']")?.getAttribute("aria-expanded") }));
  check(JSON.stringify(menu.rows) === JSON.stringify(["note", "makeup", "share-link", "In QIS öffnen ↗"]) && menu.expanded === "true", `the menu: ${JSON.stringify(menu)}`);
  const withMenu = await card(page);
  check(Math.abs(withMenu.box.bottom - first.box.bottom) < 1, `the menu does not make the card taller: ${JSON.stringify([first.box, withMenu.box])}`);
  await page.keyboard.press("Escape");
  await until(page, "Esc from the menu: the card", () => !onPage("dialog[open] .tm-menu") && Boolean(onPage("dialog[open] .tm-card .tk")));
  // The note from the menu; Esc: the card.
  await click(page, "dialog[open] .tm-tools [data-action='more']", "„⋯\"");
  await click(page, "dialog[open] .tm-menu [data-action='note']", "„Notiz hinzufügen\"");
  await until(page, "the note's field, with the keyboard", () => Boolean(onPage("dialog[open] #tm-note-field")) && document.activeElement === onPage("dialog[open] #tm-note-field"));
  await page.keyboard.press("Escape");
  await until(page, "Esc from the note: the card", () => !onPage("dialog[open] #tm-note-field") && Boolean(onPage("dialog[open] .tm-card .tk")));
  // „Ausblenden"; Esc: the card; Esc: closed.
  await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye");
  await until(page, "„Ausblenden\"", () => Boolean(onPage("dialog[open] .tm-hide")));
  await page.keyboard.press("Escape");
  await until(page, "Esc from „Ausblenden\": the card", () => !onPage("dialog[open] .tm-hide") && Boolean(onPage("dialog[open] .tm-card .tk")));
  // The keyboard comes back to the eye in the frame after the layer goes.
  await until(page, "the keyboard back on the eye", () => document.activeElement === onPage("dialog[open] .tm-tools [data-action='hide']"));

  // Another slot while the card stands: its card in the place of this one, no step more; Back leaves.
  await clickSlot(page, keys.mitMo);
  await cardOpen(page, "the other slot's card", keys.mitMo);
  await settled(page, "the other slot's card");
  const swapped = await card(page);
  check(swapped.chip === "MIT1" && swapped.x !== first.x && await page.evaluate(() => history.length) === steps + 1, `the card swapped in place: ${JSON.stringify([first.x, swapped.x, swapped.chip, steps, await page.evaluate(() => history.length)])}`);
  check(await page.evaluate((key) => allOnPage(`.sp-week a.slot[href*="termin=${key}"]`).some((slot) => slot.getAttribute("aria-current") === "true"), keys.mitMo), "the other slot is marked");
  check(fine(await stands(page, keys.mitMo)), `the swapped card beside its slot: ${JSON.stringify(await stands(page, keys.mitMo))}`);
  await page.goBack();
  await cardGone(page, "Back leaves the card in one step");
  await until(page, "the plan's address", () => location.pathname + location.search === "/studyplan");
  // Back once more is not the card again.
  await page.keyboard.press("Escape");
  check(await page.evaluate(() => !onPage("dialog[open]")), "Esc on the plan opened something");
  check(await page.evaluate(() => window.__marker === 1), "the Stundenplan was loaded again");
  await context.close();
}

// ---------- where the card stands ----------
if (block("place")) {
  const { page, context } = await open("/studyplan", wide);
  await until(page, "the week", () => allOnPage(".sp-week a.slot").length > 0);
  const week = await slots(page);
  const wrong = [];
  for (const slot of week) {
    if (await page.evaluate(() => Boolean(onPage("dialog[open]")))) {
      await page.keyboard.press("Escape");
      // Gone and the page at rest: a click in the instant the card closes can be lost (the step back
      // that closes it lands after the click — the shell's way of closing what stands beside a page).
      await cardGone(page, `Esc on the card before ${slot.what} ${slot.day} ${slot.from}`);
    }
    await clickSlot(page, slot.key);
    if (!(await cardOpen(page, `the card of ${slot.what} ${slot.day} ${slot.from}`, slot.key))) continue;
    await settled(page, `the card of ${slot.what} ${slot.day} ${slot.from}`);
    const at = await stands(page, slot.key);
    if (!fine(at)) wrong.push({ slot: `${slot.what} ${slot.day} ${slot.from}`, ...at });
  }
  check(week.length === 21 && wrong.length === 0, `every card of the week (${week.length}) beside its slot, inside the window: ${JSON.stringify(wrong)}`);
  // A narrower window: the card of the last slot stands beside its column again.
  const last = week.at(-1);
  await page.setViewportSize({ width: 1180, height: 820 });
  await page.waitForTimeout(300);
  await settled(page, "the card after the window shrank");
  const narrower = await stands(page, last.key);
  check(fine(narrower), `after the window shrank: ${JSON.stringify(narrower)}`);
  await context.close();
  // Opened by its address (a reload): beside its slot all the same, once the week is drawn.
  const again = await open(`/studyplan?termin=${keys.prakFr}`, wide);
  await cardOpen(again.page, "the card by its address", keys.prakFr);
  // Beside its slot once the week has drawn it, however late: the card asks every 100 ms until the
  // slot it marks is there (before, it was placed at 150, 400 and 1000 ms only and stayed in the
  // middle of the page column when the week came later, 1 run in 9 here). Waited for up to 10 s.
  let byAddress = { missing: true };
  for (let i = 0; i < 50; i++) {
    await settled(again.page, "the card by its address");
    byAddress = await stands(again.page, keys.prakFr);
    if (fine(byAddress)) break;
    await again.page.waitForTimeout(200);
  }
  check(fine(byAddress), `the card by its address beside its slot: ${JSON.stringify(byAddress)}`);
  await again.context.close();
}

// ---------- keys, „Teilen", the overview ----------
if (block("keys")) {
  const { page, context } = await open("/studyplan", wide);
  await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin: base });
  await until(page, "the week", () => allOnPage(".sp-week a.slot").length > 0);
  await clickSlot(page, praktikum);
  await cardOpen(page, "the card of the Praktikum", praktikum);
  await settled(page, "the card of the Praktikum");
  const first = await card(page);
  const on = (page) => page.evaluate(() => new URLSearchParams(location.search).get("on"));
  const shift = (day, days) => { const d = new Date(`${day}T12:00:00Z`); d.setUTCDate(d.getUTCDate() + days); return d.toISOString().slice(0, 10); };
  // → and ←: the date after and back, one a press; the card does not move.
  await page.evaluate(() => document.activeElement?.blur());
  const allHref = first.allHref;
  const shown = new URLSearchParams(allHref.split("?")[1]).get("on") ?? "";
  await idle(page);
  await page.keyboard.press("ArrowRight");
  await until(page, "→: the next date", (next) => new URLSearchParams(location.search).get("on") === next && onPage("dialog[open] .tk-date > b")?.textContent.includes(`${next.slice(8, 10)}.${next.slice(5, 7)}.`), shift(shown, 7));
  await idle(page);
  await page.keyboard.press("ArrowLeft");
  await until(page, "←: the date before", (day) => (new URLSearchParams(location.search).get("on") ?? day) === day && onPage("dialog[open] .tk-date > b")?.textContent.includes(`${day.slice(8, 10)}.${day.slice(5, 7)}.`), shown);
  await idle(page);
  const moved = await card(page);
  check(Math.abs(moved.box.left - first.box.left) < 1 && Math.abs(moved.box.top - first.box.top) < 1, `the date's arrows do not move the card: ${JSON.stringify([first.box, moved.box])}`);
  // `?` from within the card: „Im Termin" first, over it.
  await page.focus("dialog[open] .tk-date a.pg >> nth=-1");
  await page.keyboard.press("?");
  await until(page, "the keys of a Termin", () => document.querySelector(".keys-dialog[open] .keys-group.here h3")?.textContent.startsWith("Im Termin"));
  await page.keyboard.press("Escape");
  await until(page, "the keys' overview closed, the card still there", () => !document.querySelector(".keys-dialog[open]") && Boolean(onPage("dialog[open] .tm-card .tk")));
  // „⋯" → „Teilen": the Termin's own page on the day shown, copied, and said so.
  const day = (await on(page)) ?? shown;
  await click(page, "dialog[open] .tm-tools [data-action='more']", "„⋯\"");
  await click(page, "dialog[open] .tm-menu .tm-share", "„Teilen\"");
  await until(page, "„Teilen\" says it copied", () => /kopiert/i.test(onPage("dialog[open] .tm-menu .tm-share")?.textContent ?? ""));
  const copied = await page.evaluate(() => navigator.clipboard.readText()).catch((error) => String(error));
  check(copied === `${base}/event/${praktikum}/${day}`, `„Teilen" copies the Termin's page: „${copied}"`);
  await page.keyboard.press("Escape");
  await until(page, "the menu closed", () => !onPage("dialog[open] .tm-menu") && Boolean(onPage("dialog[open] .tm-card")));
  // V: the overview fills the plan's page, in two columns, with its switches.
  await page.evaluate(() => document.activeElement?.blur());
  await idle(page);
  await page.keyboard.press("v");
  await until(page, "V: the overview", (day) => new URLSearchParams(location.search).get("full") === "1" && (new URLSearchParams(location.search).get("on") ?? day) === day && Boolean(onPage(".is-termin.tm-filling .tk")) && !onPage("dialog[open] .tm-card"), day);
  const overview = await page.evaluate(() => {
    const body = onPage(".is-termin.tm-filling .dbody.tm");
    return { columns: body ? getComputedStyle(body).gridTemplateColumns.split(" ").length : 0, switches: allOnPage(".is-termin.tm-filling .tm-sec.cal [data-action]").map((el) => el.dataset.action).join(), events: allOnPage(".is-termin.tm-filling .tm-evs > a").map((a) => a.textContent.trim()).join(" | "), groups: allOnPage(".is-termin.tm-filling .tm-pick .grp:not(.tm-series) .gs").length, back: onPage(".is-termin.tm-filling .hero-top > a.at-head")?.getAttribute("href") ?? "" };
  });
  check(overview.columns === 2 && overview.events === "Vorlesung | Praktikum" && overview.groups === 4 && overview.back === "/studyplan", `the overview: ${JSON.stringify(overview)}`);
  check(overview.switches === "show-event,tick,hide-date,change,note,makeup", `the overview's switches: ${overview.switches}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-overview.png` });
  // Esc: the plan, not the card.
  await page.keyboard.press("Escape");
  await until(page, "Esc from the overview: the plan", () => location.pathname + location.search === "/studyplan" && !onPage("dialog[open]") && !onPage(".is-termin.tm-filling"));
  check(await page.evaluate(() => window.__marker === 1), "the Stundenplan was loaded again");
  await context.close();
}
if (block("overview")) {
  // The lecture's card „Alle Termine von EvS ›": the switch of the Termin as a whole between the
  // event's and the date's; „Zurück" leads to the plan as it was.
  const friday = keys.evsFr;
  const { page, context } = await open("/studyplan", wide);
  await until(page, "the week", () => allOnPage(".sp-week a.slot").length > 0);
  await clickSlot(page, friday);
  await cardOpen(page, "the card of VL EvS", friday);
  check((await card(page)).all === "Alle Termine von EvS", "the way on names the module");
  await click(page, "dialog[open] .tm-ways a.tm-all", "„Alle Termine von EvS\"");
  await until(page, "the overview of EvS", () => /[?&]full=1/.test(location.search) && Boolean(onPage(".is-termin.tm-filling .tk")));
  const switches = await page.evaluate(() => allOnPage(".is-termin.tm-filling .tm-sec.cal .crow").filter((row) => row.querySelector("[data-action]")).map((row) => [row.querySelector("[data-action]").dataset.action, row.querySelector("b")?.textContent.trim()]));
  check(switches.map(([action]) => action).join() === "show-event,show-termin,hide-date,change,note,makeup" && switches[1]?.[1] === "Fr 09:15 im Stundenplan", `the Termin's switch as a whole: ${JSON.stringify(switches)}`);
  await click(page, ".is-termin.tm-filling [data-action='show-termin']", "the Termin's switch");
  await until(page, "the Termin off", (key) => onPage(".is-termin.tm-filling [data-action='show-termin']")?.getAttribute("aria-checked") === "false" && (localStorage.getItem("betula.studyplan.v1") ?? "").split("\n").some((line) => line.startsWith("r\t2026W\t") && line.includes(key)) && !onPage(".is-termin.tm-filling [data-action='hide-date']"), friday);
  await click(page, ".is-termin.tm-filling [data-action='show-termin']", "the Termin's switch again");
  await until(page, "the Termin on again", () => onPage(".is-termin.tm-filling [data-action='show-termin']")?.getAttribute("aria-checked") === "true" && !(localStorage.getItem("betula.studyplan.v1") ?? "").includes("\nr\t"));
  await click(page, ".is-termin.tm-filling .hero-top > a.at-head", "„Zurück\"");
  await until(page, "„Zurück\": the plan, no card", () => location.pathname + location.search === "/studyplan" && !onPage("dialog[open]") && !onPage(".is-termin.tm-filling"));
  // Back from the plan does not bring the card back.
  check(await page.evaluate(() => window.__marker === 1), "the Stundenplan was loaded again");
  await context.close();
}

// ---------- while editing: what hides answers in the slot; the faint slots' cards ----------
if (block("edit")) {
  const friday = keys.evsFr;
  const { page, context } = await open("/studyplan?edit=1", wide);
  await until(page, "the week while editing", () => allOnPage(".sp-week a.slot").length > 0 && Boolean(onPage(".sp-editbar")));
  await clickSlot(page, friday);
  await cardOpen(page, "the card while editing", friday);
  check((await card(page)).tools.join() === "change,hide,more", "the card's tools while editing");
  await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye");
  await click(page, "dialog[open] .tm-hide [data-action='hide-termin']", "„Alle 15\"");
  await cardGone(page, "„Alle 15\" while editing: the card closes");
  await until(page, "the slot faint", (key) => allOnPage(`.sp-week a.slot.faint[href*="termin=${key}"]`).length === 1, friday);
  await page.waitForTimeout(300);
  check(await page.evaluate(() => !onPage(".own-note")), "while editing the slot answers, and no note comes");
  check(/^\/studyplan\?edit=1$/.test(await here(page)), `the mode stays: ${await here(page)}`);
  // d1: the faint slot's card says what hid it, has nothing to hide, and „Einblenden" has the keyboard.
  await clickSlot(page, friday);
  await cardOpen(page, "the faint slot's card", friday);
  await settled(page, "the faint slot's card");
  let v = await card(page);
  check(v.off === "Ausgeblendet" && !v.tools.includes("hide") && v.tools.includes("more") && JSON.stringify(v.big) === JSON.stringify(["show-termin", "Einblenden", ""]) && v.focus === "show-termin", `a faint slot's card (d1): ${JSON.stringify([v.off, v.tools, v.big, v.focus])}`);
  check(fine(await stands(page, friday)), `the faint slot's card beside it: ${JSON.stringify(await stands(page, friday))}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-card-faint.png` });
  await page.keyboard.press("Enter");
  await cardGone(page, "„Einblenden\": the card closes");
  await until(page, "the slot full again", (key) => !(localStorage.getItem("betula.studyplan.v1") ?? "").includes("\nr\t") && allOnPage(`.sp-week a.slot.faint[href*="termin=${key}"]`).length === 0 && allOnPage(`.sp-week a.slot[href*="termin=${key}"]`).length === 1, friday);
  // The whole lecture: „Vorlesung ausgeblendet", „Vorlesung einblenden".
  await clickSlot(page, friday);
  await cardOpen(page, "the card again", friday);
  await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye");
  await click(page, "dialog[open] .tm-hide [data-action='hide-event']", "„Vorlesung ganz\"");
  await cardGone(page, "„Vorlesung ganz\" while editing");
  await until(page, "both faint", () => allOnPage(".sp-week a.slot.faint").filter((slot) => slot.textContent.includes("VL EvS")).length === 2);
  await clickSlot(page, keys.evsDi);
  await cardOpen(page, "the faint lecture's card", keys.evsDi);
  v = await card(page);
  check(v.off === "Vorlesung ausgeblendet" && JSON.stringify(v.big) === JSON.stringify(["show-event", "Vorlesung einblenden", ""]), `a lecture hidden as a whole: ${JSON.stringify([v.off, v.big])}`);
  await click(page, "dialog[open] .tm-act .tm-big", "„Vorlesung einblenden\"");
  await until(page, "the lecture back", () => !(localStorage.getItem("betula.studyplan.v1") ?? "").includes("\ne\t") && !allOnPage(".sp-week a.slot.faint").some((slot) => slot.textContent.includes("VL EvS")));
  check(await page.evaluate(() => window.__marker === 1), "the Stundenplan was loaded again");
  await context.close();
}
if (block("edit")) {
  // d2: a group faint beside the one ticked.
  const { page, context } = await open("/studyplan?edit=1", wide, FS1 + `c\t2026W\t${praktikum}\n`);
  await until(page, "the week while editing", () => allOnPage(".sp-week a.slot.faint").length > 0);
  await clickSlot(page, keys.prakMi);
  await cardOpen(page, "the faint group's card", keys.prakMi);
  await settled(page, "the faint group's card");
  let v = await card(page);
  check(v.off === "Andere Gruppe gewählt · Mo 11:30" && JSON.stringify(v.big) === JSON.stringify(["tick-only", "Nur diese Gruppe", ""]) && v.also === "Auch diese" && v.compare && !v.tools.includes("hide"), `a faint group beside the ticked one (d2): ${JSON.stringify([v.off, v.big, v.also, v.compare, v.tools])}`);
  await click(page, "dialog[open] .tm-act .tm-also", "„Auch diese\"");
  await cardGone(page, "„Auch diese\": the card closes");
  await until(page, "two ticked", ([mo, mi]) => {
    const ticks = (localStorage.getItem("betula.studyplan.v1") ?? "").split("\n").filter((line) => line.startsWith("c\t")).sort();
    return ticks.join() === [`c\t2026W\t${mo}`, `c\t2026W\t${mi}`].sort().join();
  }, [praktikum, keys.prakMi]);
  check(await page.evaluate(() => !onPage(".own-note")), "while editing a tick needs no note");
  await clickSlot(page, keys.prakDo);
  await cardOpen(page, "another faint group", keys.prakDo);
  await click(page, "dialog[open] .tm-act .tm-big[data-action='tick-only']", "„Nur diese Gruppe\"");
  await cardGone(page, "„Nur diese Gruppe\": the card closes");
  await until(page, "only Thursday's group ticked", (key) => (localStorage.getItem("betula.studyplan.v1") ?? "").split("\n").filter((line) => line.startsWith("c\t")).join() === `c\t2026W\t${key}`, keys.prakDo);
  check(await page.evaluate(() => window.__marker === 1), "the Stundenplan was loaded again");
  await context.close();
}

// ---------- the start page ----------
if (block("start")) {
  const { page, context } = await open("/", wide);
  await until(page, "a Termin of the day", () => Boolean(onPage(".day-ev[href*='termin='], .day-wk-col a[href*='termin=']")));
  const href = await page.evaluate(() => onPage(".day-ev[href*='termin=']")?.getAttribute("href") ?? onPage(".day-wk-col a[href*='termin=']")?.getAttribute("href") ?? "");
  check(/^\/\?termin=\d+-[0-9a-f]{5}&on=\d{4}-\d\d-\d\d$/.test(href), `its link: „${href}"`);
  const params = new URLSearchParams(href.split("?")[1]);
  await page.click(`[href="${href}"] >> visible=true`);
  await until(page, "the card over the start page", () => Boolean(onPage("dialog[open] .tm-dialog.tm-card .tk")));
  await settled(page, "the card over the start page");
  const v = await card(page);
  const middle = await page.evaluate(() => {
    const rail = document.querySelector(".rail")?.getBoundingClientRect().right ?? 0;
    const box = onPage("dialog[open]").getBoundingClientRect();
    return { centre: Math.abs((box.left + box.right) / 2 - (rail + innerWidth) / 2), top: Math.abs(box.top - innerHeight * 0.12) };
  });
  check(v.picks.length === 0 && middle.centre <= 2 && middle.top <= 2, `the card in the middle of the page: ${JSON.stringify([v.picks, middle])}`);
  check(v.allHref === `/studyplan?termin=${params.get("termin")}&on=${params.get("on")}&full=1`, `its way on is the Stundenplan's overview: „${v.allHref}"`);
  if (shots) await page.screenshot({ path: `${shots}/termin-home.png` });
  // „×": the start page again.
  await click(page, "dialog[open] .tm-card .dlg-close", "„×\"");
  await until(page, "the start page again", () => !onPage("dialog[open] .tm-dialog") && location.search === "");
  // Its eye: the note over the start page, „Rückgängig".
  await page.click(`[href="${href}"] >> visible=true`);
  await until(page, "the card again", () => Boolean(onPage("dialog[open] .tm-dialog.tm-card .tk")));
  if ((await card(page)).tools.includes("hide")) {
    await click(page, "dialog[open] .tm-tools [data-action='hide']", "the eye on the start page");
    await until(page, "„Ausblenden\" on the start page", () => allOnPage("dialog[open] .tm-hide .tm-hide-row").length > 0);
    await click(page, "dialog[open] .tm-hide .tm-hide-row >> nth=0", "the first reach");
    await until(page, "the note over the start page", () => !onPage("dialog[open] .tm-dialog") && /ausgeblendet$/.test(onPage(".own-note .plan-toast-said")?.textContent.trim() ?? "") && /\n[hre]\t/.test(localStorage.getItem("betula.studyplan.v1") ?? ""));
    await click(page, ".own-note .plan-toast-undo", "„Rückgängig\" on the start page");
    await until(page, "„Rückgängig\" on the start page", (plan) => localStorage.getItem("betula.studyplan.v1") === plan, FS1);
  } else problems.push(`the start page's first Termin has no eye: ${JSON.stringify(await card(page))}`);
  check(await page.evaluate(() => window.__marker === 1), "the start page was loaded again");
  // The way on: the Stundenplan's overview.
  if (!(await page.evaluate(() => Boolean(onPage("dialog[open] .tm-card"))))) await page.click(`[href="${href}"] >> visible=true`);
  await until(page, "the card a third time", () => Boolean(onPage("dialog[open] .tm-dialog.tm-card .tk")));
  await click(page, "dialog[open] .tm-ways a.tm-all", "„Alle Termine von …\"");
  await until(page, "the Stundenplan's overview", () => location.pathname === "/studyplan" && /[?&]full=1/.test(location.search) && Boolean(onPage(".is-termin.tm-filling .tk")));
  await context.close();
  // `/?termin=…&full=1` keeps answering: the start page filled.
  const filled = await open(`/?termin=${params.get("termin")}&on=${params.get("on")}&full=1`, wide);
  await until(filled.page, "the start page filled by its address", () => Boolean(onPage(".day-termin.filling .is-termin.tm-filling .tk")));
  await filled.context.close();
}

// ---------- a phone ----------
if (block("phone")) {
  const { page, context } = await open("/studyplan", phone);
  await until(page, "the week", () => allOnPage(".sp-week a.slot").length > 0);
  // A slot of the week in view (the carousel holds the weeks beside it too): the Praktikum's Monday.
  const tapped = await page.evaluate(() => {
    const inView = (el) => { const r = el.getBoundingClientRect(); return r.width > 0 && r.left >= 0 && r.right <= innerWidth; };
    const slot = allOnPage(".sp-week a.slot.alt").find((slot) => inView(slot) && slot.textContent.includes("Prak PP"));
    slot?.click();
    return slot?.getAttribute("href") ?? "";
  });
  check(tapped.includes("termin=148370-"), `a group of the Praktikum in view: „${tapped}"`);
  await until(page, "the sheet", () => {
    const sheet = onPage("dialog[open][data-sheet]");
    return Boolean(sheet?.querySelector(".tm-card .tk")) && getComputedStyle(sheet).position === "fixed" && Math.round(sheet.getBoundingClientRect().width) === innerWidth;
  });
  await page.waitForFunction(() => !onPage("dialog[open]")?.getAnimations({ subtree: true }).some((a) => a.playState === "running"), null, { timeout: 5000 }).catch(() => {});
  const sheet = await page.evaluate(() => {
    const size = (selector) => { const r = onPage(selector)?.getBoundingClientRect(); return r ? [Math.round(r.width), Math.round(r.height)] : null; };
    const body = onPage("dialog[open] .tm-card .dbody");
    const big = onPage("dialog[open] .tm-act .tm-big")?.getBoundingClientRect();
    const lowest = Math.max(...[...(body?.querySelectorAll("*") ?? [])].filter((el) => el.getClientRects().length && !el.closest(".tm-act")).map((el) => el.getBoundingClientRect().bottom));
    return { hide: size("dialog[open] [data-action='hide']"), more: size("dialog[open] [data-action='more']"), all: Boolean(onPage("dialog[open] .tm-all")?.getClientRects().length), last: big ? big.bottom >= lowest - 1 : false, wider: allOnPage("dialog[open] .tm-card *").filter((el) => el.getBoundingClientRect().right > innerWidth + 1).length };
  });
  check(sheet.hide?.[0] >= 44 && sheet.hide?.[1] >= 44 && sheet.more?.[0] >= 44 && sheet.more?.[1] >= 44, `the tools 44 px: ${JSON.stringify(sheet)}`);
  check(sheet.all && sheet.last && sheet.wider === 0, `the sheet: its way on shown, the large action last, nothing wider than the screen: ${JSON.stringify(sheet)}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-phone.png` });
  // The next date takes the history's entry; Back leaves the Termin.
  const steps = await page.evaluate(() => history.length);
  const was = await here(page);
  await page.evaluate(() => onPage("dialog[open] .tm-card .tk-date a.pg[data-step='next']")?.click());
  await until(page, "the next date in the sheet", (was) => location.pathname + location.search !== was && /[?&]on=\d{4}-\d\d-\d\d/.test(location.search), was);
  check(await page.evaluate(() => history.length) === steps, "the next date took a step of the history");
  await page.goBack();
  await until(page, "Back leaves the Termin", () => !onPage("dialog[open] .tm-dialog") && !/termin=/.test(location.search));
  check(await page.evaluate(() => window.__marker === 1), "the Stundenplan was loaded again");
  await context.close();
}

// ---------- a phone: the Termin's own page scrolls under a finger ----------
if (block("lookup")) {
  // Its box is no scroller of its own (owner, 2026-11-03: „ich kann aber auf der Termin Page immer
  // noch nicht scrollen"): a swipe moves the page.
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
  await context.addInitScript(ON_PAGE);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  await page.goto(`${base}/event/${praktikum}/2026-11-02`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the Termin's page: the browser app never took over"));
  await until(page, "the Termin's page", () => Boolean(onPage(".is-termin.tm-page .tk")));
  const cdp = await context.newCDPSession(page);
  let y = 650;
  await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: 200, y }] });
  for (let i = 0; i < 15; i++) {
    y -= 20;
    await cdp.send("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [{ x: 200, y }] });
    await new Promise((resolve) => setTimeout(resolve, 16));
  }
  await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  await until(page, "a swipe scrolls the Termin's page", () => scrollY > 100);
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
    groupsAt: Math.round(panel?.querySelector(".tm-pick .grp:not(.tm-series)")?.getBoundingClientRect().top ?? -1),
    seriesAt: Math.round(panel?.querySelector(".tm-pick .sdots")?.getBoundingClientRect().top ?? -1),
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
if (block("lookup")) {
  // The Praktikum, in the plan: beside its module's page; „Ganze Seite" its own page; Back.
  const { page, context } = await open("/catalog/module/12102");
  await until(page, "the module's Termine", () => Boolean(onPage("a.ev[href^='/catalog/module/12102?termin=148370-']")));
  const href = await follow(page, "/catalog/module/12102?termin=148370-");
  await until(page, "the Termin over the module's page", (href) => location.pathname + location.search === href && Boolean(onPage("dialog[open] .tm-dialog .tk")) && Boolean(onPage("dialog[open] .tm-dialog .grp-f")) && !onPage("dialog[open]").getAnimations().length, href);
  let v = await lookUp(page, "dialog[open] .tm-dialog");
  check(v.title === "Programmierpraktikum" && !v.out && !v.file, `in the plan: ${JSON.stringify(v)}`);
  check(v.dots.length === 4 && v.dots.includes("ok") && v.dots.every((dot) => dot === "ok" || dot === "no"), `the groups measured against the plan: ${JSON.stringify(v)}`);
  check(v.foot === "Noch keine in deinem Stundenplan" && /^\/studyplan\?sem=2026W&termin=148370-[0-9a-f]{5}&on=\d{4}-\d\d-\d\d$/.test(v.adjust), `„Anpassen ›": ${JSON.stringify(v)}`);
  check(v.day.length > 1, `the plan's day around it: ${JSON.stringify(v)}`);
  check(/^\/event\/148370-[0-9a-f]{5}\/\d{4}-\d\d-\d\d$/.test(v.full), `„Ganze Seite" is its own page: ${JSON.stringify(v)}`);
  check(v.events.join(" | ") === "Vorlesung | *Praktikum" && v.groupsAt >= 0 && v.groupsAt < v.seriesAt && v.seriesAt < v.ticketAt, `the events, the groups and „Reihe" above the ticket: ${JSON.stringify(v)}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-beside-module.png` });
  // A group picked (owner, 2026-10-27: „Wenn man solche Buttons hat, dann ändern die nur den Inhalt
  // unter ihnen"): the ticket under the groups is the group's, nothing above them changes.
  const second = await page.evaluate(() => onPage("dialog[open] .tm-dialog .tm-pick .gs:not(.this)")?.getAttribute("href") ?? "");
  await page.click(`dialog[open] .tm-dialog .tm-pick .gs[href="${second}"]`);
  await until(page, "the second group", (second) => location.pathname + location.search === second && onPage("dialog[open] .tm-dialog .tm-pick .gs.this")?.getAttribute("href") === second, second);
  const picked = await lookUp(page, "dialog[open] .tm-dialog");
  check(picked.head === v.head && picked.events.join() === v.events.join() && picked.groupsAt === v.groupsAt && picked.when !== v.when, `a group picked changes the ticket alone: ${JSON.stringify([v, picked])}`);
  // A date picked by its dot (owner, 2026-11-03: „bau als Nächstes die Reihe als Auswahl über dem
  // Ticket"): the ticket under „Reihe" is that date's, nothing above it moves.
  await page.goBack();
  await until(page, "back on the first group", (href) => location.pathname + location.search === href, href);
  const nextDot = await page.evaluate(() => {
    const dots = [...(onPage("dialog[open] .tm-dialog .tm-pick .sdots")?.querySelectorAll("a.sdl") ?? [])];
    return dots[dots.findIndex((a) => a.getAttribute("aria-current") === "true") + 1]?.getAttribute("href") ?? "";
  });
  await page.click(`dialog[open] .tm-dialog .tm-pick .sdots a.sdl[href="${nextDot}"]`);
  await until(page, "the date after by its dot", (next) => location.pathname + location.search === next && onPage("dialog[open] .tm-dialog .sdots a.sdl[aria-current='true']")?.getAttribute("href") === next, nextDot);
  const dated = await lookUp(page, "dialog[open] .tm-dialog");
  check(dated.head === v.head && dated.events.join() === v.events.join() && dated.groupsAt === v.groupsAt && dated.seriesAt === v.seriesAt && dated.when !== v.when, `a date picked changes the ticket alone: ${JSON.stringify([v, dated])}`);
  await page.goBack();
  await until(page, "back on the first date", (href) => location.pathname + location.search === href, href);
  await page.click(`dialog[open] .tm-dialog .tm-pick .gs[href="${second}"]`);
  await until(page, "the second group again", (second) => location.pathname + location.search === second, second);
  // The lecture's segment: the lecture's ticket under the head, which stays.
  await page.click("dialog[open] .tm-dialog .tm-evs > a:not([aria-checked='true'])");
  await until(page, "the lecture", () => onPage("dialog[open] .tm-dialog .tm-evs > a[aria-checked='true']")?.textContent.trim() === "Vorlesung" && !onPage("dialog[open] .tm-dialog .tm-pick .grp:not(.tm-series)"));
  const lecture = await lookUp(page, "dialog[open] .tm-dialog");
  check(lecture.head === v.head && lecture.when !== v.when, `the lecture's segment changes what stands under it: ${JSON.stringify(lecture)}`);
  await page.goBack();
  await page.goBack();
  await until(page, "back on the first group", (href) => location.pathname + location.search === href, href);
  // → beside the page: the date after, in the module page's address.
  await page.evaluate(() => document.activeElement?.blur());
  await page.keyboard.press("ArrowRight");
  await until(page, "→ beside the module's page", () => location.pathname === "/catalog/module/12102" && /[?&]on=\d{4}-\d\d-\d\d/.test(location.search) && Boolean(onPage("dialog[open] .tm-dialog .tk")));
  await page.goBack();
  await until(page, "back to the first date", (href) => location.pathname + location.search === href, href);
  // „Ganze Seite": the Termin's own page, in two columns.
  await page.click("dialog[open] .tm-dialog .tm-full");
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
  await until(page, "the Termin in the Stundenplan", () => location.pathname === "/studyplan" && Boolean(onPage("dialog[open] .tm-dialog .tk")) && Boolean(onPage("dialog[open] .tm-dialog [data-action='tick']")));
  await page.goBack();
  await until(page, "back on the Termin's page", () => Boolean(onPage(".is-termin.tm-page .tk")));
  await page.goBack();
  await until(page, "back beside the module's page", (href) => location.pathname + location.search === href && Boolean(onPage("dialog[open] .tm-dialog .tk")), href);
  // Esc: the module's page alone.
  await page.keyboard.press("Escape");
  await until(page, "Esc: the module's page alone", () => location.pathname === "/catalog/module/12102" && !/termin=/.test(location.search) && !onPage("dialog[open] .tm-dialog"));
  check(await page.evaluate(() => window.__marker === 1), "the module page was loaded again");
  await context.close();
}
if (block("lookup")) {
  // Enterprise Ressource Planning's exercise, outside the plan: beside its module's page.
  const { page, context } = await open("/catalog/module/12612");
  await until(page, "the module's Termine", () => Boolean(onPage("a.ev[href^='/catalog/module/12612?termin=150294-']")));
  await follow(page, "/catalog/module/12612?termin=150294-");
  await until(page, "the Termin beside the module's page", () => /[?&]termin=150294-/.test(location.search) && Boolean(onPage("dialog[open] .tm-dialog .tk")));
  await until(page, "the calendar file", () => (onPage("dialog[open] .tm-dialog a.tm-big")?.getAttribute("href") ?? "").startsWith("blob:"));
  const v = await lookUp(page, "dialog[open] .tm-dialog");
  check(v.out === "nicht im Plan" && !v.foot && !v.adjust, `outside the plan: ${JSON.stringify(v)}`);
  check(v.dots.length === 3 && v.dots.filter((dot) => dot === "no").length === 1, `one group meets the plan: ${JSON.stringify(v)}`);
  check(v.day.some((bar) => bar.includes(" me") && bar.includes(" out")) && v.day.length > 1, `hatched among the plan's day: ${JSON.stringify(v)}`);
  check(v.file.endsWith(".ics") && v.blob, `„In den Kalender": ${JSON.stringify(v)}`);
  if (shots) await page.screenshot({ path: `${shots}/termin-beside-out.png` });
  await context.close();
}
if (block("lookup")) {
  // A phone: the Termin a sheet over its module's page.
  const { page, context } = await open("/catalog/module/12102", { width: 390, height: 844 });
  await until(page, "the module's Termine", () => Boolean(onPage("a.ev[href^='/catalog/module/12102?termin=148370-']")));
  await follow(page, "/catalog/module/12102?termin=148370-");
  await until(page, "the sheet over the module's page", () => {
    const sheet = onPage("dialog[open][data-sheet]");
    return Boolean(sheet?.querySelector(".tm-dialog .tk")) && getComputedStyle(sheet).position === "fixed" && Math.round(sheet.getBoundingClientRect().width) === innerWidth;
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
