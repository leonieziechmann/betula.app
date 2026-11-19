// The Stundenplan's module list: a module's events as rows, each with its eye (owner, 2026-11-17: „Man
// kanm das wahrscheinlich am besten umsetzen, in dem man das so Macht, dass bei den Modulen die
// Kategorien vl, Übung, prak sind und man da die Übersicht auswählen kann, wo man alles durchklicken
// kann"; „Denn man kann gerade noch module im Kalender verwalten, das ist aber keine gute idee, weil man
// das ja in mein Studium macht"; docs/folia/frontend.md, „The module list"), on canary's data of 2026W:
// Informatik B.Sc.'s first semester planned (EvS 12104, EEG 12107, PP 12102, MIT1 11112), and real
// modules the review of the list named (12196, 11714, 11515, 11474, 11777, 12244).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node categories.mjs [--only=rows,real,eye,hidden,cut,back,own,manage,calendar,empty]
// - rows: one line a module (its key in its tone, the title cut with „…"); a row per event that has a
//   Termin, kind first, with the summaries' exact texts of the example week (EvS „Vorlesung · Di 11:30
//   · Fr 09:15", „Übung · 4 Gruppen · offen", …, MIT1 „Tutorium · Di 15:30 · freiwillig"); the same rows
//   looking and editing, the summaries' left edge alike; the last line „Module planen in „Mein Studium“"
//   to `/study?plan=1`.
// - real: one slot said once (12196 „Mo 09:15"), the Bachelor-Arbeit's projects apart by their day, a
//   row without any date („ohne feste Zeit") leading to the module beside the plan (`open=…&row=…`), a
//   weekly Termin without a time adding nothing, groups („4 Gruppen · offen", „2 Gruppen · offen").
// - eye: while editing the eye is a switch (`role="switch"`, „Tutorium MIT1 zeigen"); a click hides
//   the event (the `e` line, the row pale „ausgeblendet", the crossed eye, its slot faint in the week),
//   keeps the keyboard on the eye and the eye in its place; a second click brings it back, the ticks untouched. Looking: no
//   button in the list, the crossed eye of a hidden row a link into the mode.
// - hidden: a lecture whose every Termin is hidden one by one (an `r` line) reads „ausgeblendet", its
//   eye crossed, one click shows it again; three groups hidden one by one: „Mo 15:30" with the sign „3
//   Gruppen ausgeblendet"; a date hidden: the sign „am Fr 20.11. ausgeblendet"; the foot „4 ausgeblendet
//   · Alle einblenden" (three Termine, one date) while editing alone, which clears what is hidden and
//   keeps the ticks, with „4 wieder eingeblendet · Rückgängig" bringing all of it back.
// - cut: at 1280 px no part of a summary is cut or broken inside, for the example week and the real
//   modules, looking and editing.
// - back: a row leads to the overview at that event (its segment chosen); „Zurück" leads to the plan
//   where it was — the module column scrolled as before, the row in its place and with the keyboard.
// - own: „+ Termin" of a module (while editing alone): the form „Für EvS · WiSe 2026/27" with „Art"; a
//   Termin of one's own of a module is a row under it with the mark „von dir", which opens its card; a
//   module QIS has nothing for: „keine Termine in QIS · Termin eintragen" in both states.
// - manage: nothing manages the study's modules — no „+ Modul", no „×", no credits, no number in the
//   list, looking or editing; the module's panel without „Entfernen", „Zählt für", „Auch geplant".
// - calendar: a calendar of one's own made by its name alone; its part says it by its name, „+ Modul"
//   while editing alone takes a module in (an `n` line, one line in the part, leading to `open=…&full=1`),
//   its bin (while editing alone) takes it out with „Rückgängig"; a Termin of one's own in it.
// - empty: nothing planned — „Noch keine Termine" with three ways („Zu „Mein Studium“", „Zum Katalog",
//   „+ Termin", which opens the form); while editing the week is drawn without the card; a phone the same.
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
const planOf = (ids) => ids.map((id) => `m\t2026W\t${id}\t1790000000\t`).join("\n") + "\n";
const FS1 = planOf(["12104", "12107", "12102", "11112"]);
const REAL = planOf(["12196", "11714", "12244", "11515", "11474", "11777"]);
const INFORMATIK = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n";
const PRAK_BS = "12340";
const wide = { viewport: { width: 1440, height: 900 } };
const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };

const open = async (options, plan, path = "/studyplan", mine = INFORMATIK) => {
  const context = await browser.newContext(options);
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  await context.addInitScript(([k1, k2, v1, v2]) => {
    if (localStorage.getItem(k1) === null) {
      // An empty plan is no key at all, as in a fresh browser.
      if (v1) localStorage.setItem(k1, v1);
      localStorage.setItem(k2, v2);
    }
  }, [PLAN, MINE, plan, mine]);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
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
// The page at rest: no picture of the page before held (it comes a moment after the address changed,
// and a click while it is held can be lost), nothing busy, nothing settling, for 120 ms.
const rest = async (page) => {
  await page.evaluate(() => { window.__restSince = 0; }).catch(() => {});
  await page.waitForFunction(() => {
    const quiet = !document.querySelector(".pending-page.held, main.content[aria-busy]") && !document.documentElement.hasAttribute("data-settling");
    if (!quiet) { window.__restSince = 0; return false; }
    window.__restSince ||= performance.now();
    return performance.now() - window.__restSince > 120;
  }, null, { timeout: 5000 }).catch(() => {});
};
// A click that may find nothing: said, not thrown. It waits for the page at rest first.
const click = async (page, selector, name = selector) => {
  await rest(page);
  try {
    await page.click(selector, { timeout: 5000 });
    return true;
  } catch {
    problems.push(`${name}: nothing to click (${page.url()})`);
    return false;
  }
};
const stored = (page) => page.evaluate((key) => localStorage.getItem(key) ?? "", PLAN);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const where = (page) => page.evaluate(() => location.pathname + location.search);
const marker = async (page, name) => check(await page.evaluate(() => window.__marker === 1), `${name}: the page loaded again`);
const settled = () => allOnPage(".sp-list .sp-mod").length > 0 && !document.querySelector(".pending-page.held");
const editing = () => Boolean(onPage(".page-inner.sp.sp-edit")) && Boolean(onPage(".sp-editbar")) && allOnPage(".sp-list .sp-cat-eye").length > 0 && !document.querySelector(".pending-page.held");
const looking = () => !onPage(".page-inner.sp.sp-edit") && allOnPage(".sp-list .sp-mod").length > 0 && allOnPage(".sp-list .sp-cat-eye").length === 0 && !document.querySelector(".pending-page.held");
// The list as the student reads it: per module its key, title and rows („word · summary"), the rows'
// links, and what stands in the eye's place.
const list = (page) => page.evaluate(() => allOnPage(".sp-list .sp-mod").map((mod) => ({
  id: mod.dataset.id,
  key: mod.querySelector(".sp-mod-row .sp-key")?.textContent.trim(),
  rows: [...mod.querySelectorAll(".sp-cat:not(.own)")].map((cat) => `${cat.querySelector(".sp-cat-row b")?.textContent.trim()} · ${cat.querySelector(".sp-cat-sum")?.textContent.replace(/\s+/g, " ").trim()}`),
  hrefs: [...mod.querySelectorAll(".sp-cat:not(.own) a.sp-cat-row")].map((a) => a.getAttribute("href")),
  none: mod.querySelector(".sp-cat-none")?.textContent.replace(/\s+/g, " ").trim() ?? null,
})));
const byId = (mods) => Object.fromEntries(mods.map((mod) => [mod.id, mod]));
// The eye of an event's row: what it is and says.
const eye = (page, event) => page.evaluate((event) => {
  const cat = onPage(`.sp-list .sp-cat[data-event="${event}"]`);
  const el = cat?.querySelector(".sp-cat-eye, .eye");
  return cat && {
    off: cat.classList.contains("off"),
    sum: cat.querySelector(".sp-cat-sum")?.textContent.replace(/\s+/g, " ").trim(),
    tag: el?.tagName.toLowerCase() ?? null,
    role: el?.getAttribute("role") ?? null,
    checked: el?.getAttribute("aria-checked") ?? null,
    label: el?.getAttribute("aria-label") ?? null,
    href: el?.getAttribute("href") ?? null,
    icon: el?.querySelector("use")?.getAttribute("href")?.split("#")[1] ?? null,
    sign: cat.querySelector(".sp-cat-hid")?.getAttribute("aria-label") ?? null,
    signShown: Boolean(cat.querySelector(".sp-cat-hid")) && getComputedStyle(cat.querySelector(".sp-cat-hid")).visibility !== "hidden",
  };
}, event);

// The example week: the rows and their exact words.
const EXAMPLE = {
  12104: ["Vorlesung · Di 11:30 · Fr 09:15", "Übung · 4 Gruppen · offen"],
  12107: ["Vorlesung · Di 07:30 A · Di 09:15", "Übung · Di 07:30 B"],
  12102: ["Vorlesung · Fr 11:30 A", "Praktikum · 4 Gruppen · offen"],
  11112: ["Vorlesung · Mo 13:45 · Di 13:45", "Übung · 3 Gruppen · offen", "Tutorium · Di 15:30 · freiwillig"],
};

// ---------- the rows of the example week ----------
if (block("rows")) {
  const { page, context } = await open(wide, FS1);
  await until(page, "the list while looking", looking);
  const mods = await list(page);
  check(same(mods.map((mod) => `${mod.id} ${mod.key}`), ["12104 EvS", "12107 EEG", "12102 PP", "11112 MIT1"]), `the modules: ${JSON.stringify(mods.map((mod) => mod.key))}`);
  for (const mod of mods) check(same(mod.rows, EXAMPLE[mod.id]), `the rows of ${mod.key}: ${JSON.stringify(mod.rows)}`);
  check(mods.every((mod) => mod.hrefs.every((href) => /^\/studyplan\?termin=\d+-[0-9a-f]{5}&full=1$/.test(href))), `a row does not lead to its overview: ${JSON.stringify(mods.map((mod) => mod.hrefs))}`);
  // One line a module: the title cut with „…", the row as high as one line.
  const lines = await page.evaluate(() => allOnPage(".sp-list .sp-mod-row").map((row) => {
    const b = row.querySelector("b");
    const style = getComputedStyle(b);
    return { title: b.textContent, lines: Math.round(b.getBoundingClientRect().height / parseFloat(style.lineHeight)), ellipsis: style.textOverflow, cut: b.scrollWidth > b.clientWidth + 0.5 };
  }));
  check(lines.every((line) => line.lines === 1 && line.ellipsis === "ellipsis"), `a module takes more than one line: ${JSON.stringify(lines)}`);
  check(lines.some((line) => line.cut), `no long title is cut with „…" at 1440 px: ${JSON.stringify(lines)}`);
  // The last line, to „Mein Studium".
  const way = await page.evaluate(() => { const a = onPage(".sp-list ~ a.sp-way, .sp-side a.sp-way"); return a && [a.textContent.trim(), a.getAttribute("href")]; });
  check(same(way, ["Module planen in „Mein Studium“", "/study?plan=1"]), `the last line: ${JSON.stringify(way)}`);
  // The same rows while editing, the summaries' left edge where it was.
  const left = () => page.evaluate(() => allOnPage(".sp-list .sp-cat-sum").map((sum) => Math.round(sum.getBoundingClientRect().left * 10) / 10));
  const before = await left();
  await click(page, "#sp-edit-toggle", "„Bearbeiten“");
  await until(page, "the list while editing", editing);
  const edited = await list(page);
  check(same(edited.map((mod) => mod.rows), mods.map((mod) => mod.rows)), `the rows while editing: ${JSON.stringify(edited.map((mod) => mod.rows))}`);
  check(same(await left(), before), `the summaries moved: ${JSON.stringify(before)} → ${JSON.stringify(await left())}`);
  if (shots) await page.screenshot({ path: `${shots}/categories-rows.png` });
  await marker(page, "the rows");
  await context.close();
}

// ---------- real modules ----------
if (block("real")) {
  const { page, context } = await open(wide, REAL);
  await until(page, "the real modules", () => allOnPage(".sp-list .sp-mod").length === 6);
  const mods = byId(await list(page));
  const want = {
    12196: ["Vorlesung · Mo 09:15", "Seminar · ohne feste Zeit"],
    11714: ["Projekt · Di 13:45 1 · Di 13:45", "Projekt · Do · Do 09:30", "Projekt · Di 13:45 2 · Di 13:45", "Projekt · Di 13:45 3 · Di 13:45"],
    11515: ["Seminar · ohne feste Zeit", "Kolloquium · ohne feste Zeit"],
    11474: ["Vorlesung · Do 13:45 B", "Übung · ohne feste Zeit"],
    11777: ["Vorlesung · Mi 07:30 · Mi 07:30", "Vorlesung · Mi 09:15 · Mi 09:15", "Vorlesung · Fr · Fr 08:00", "Praktikum · Mi · 4 Gruppen · offen", "Praktikum · Do · 2 Gruppen · offen"],
  };
  for (const [id, rows] of Object.entries(want)) check(same(mods[id]?.rows, rows), `the rows of ${id}: ${JSON.stringify(mods[id]?.rows)}`);
  // A row without any date: to the module beside the plan, at that Termin.
  check(same(mods[11515]?.hrefs, ["/studyplan?open=11515&row=148718-9a40f", "/studyplan?open=11515&row=148098-9a40f"]), `the undated rows of 11515: ${JSON.stringify(mods[11515]?.hrefs)}`);
  await click(page, '.sp-cat[data-event="148098"] a.sp-cat-row', "„Kolloquium · ohne feste Zeit“");
  await until(page, "the module beside the plan", () => location.search === "?open=11515&row=148098-9a40f" && Boolean(onPage(".work > .detail.aside#preview")));
  check(await page.evaluate(() => !/Diesen Termin hat dein Stundenplan nicht/.test(document.body.innerText)), "the undated row leads to „Diesen Termin hat dein Stundenplan nicht (mehr).“");
  await marker(page, "real modules");
  await context.close();
}

// ---------- the eye ----------
if (block("eye")) {
  const { page, context } = await open(wide, FS1 + "c\t2026W\t148369-aaf38\n", "/studyplan?edit=1");
  await until(page, "the list while editing", editing);
  const tut = "150132";
  let e = await eye(page, tut);
  check(e?.tag === "button" && e.role === "switch" && e.checked === "true" && e.label === "Tutorium MIT1 zeigen" && e.icon === "eye" && !e.off, `the eye of the Tutorium: ${JSON.stringify(e)}`);
  const before = await stored(page);
  const eyeBox = () => page.evaluate((tut) => { const r = onPage(`.sp-cat[data-event="${tut}"] .sp-cat-eye`)?.getBoundingClientRect(); return r && [r.x, r.y, r.width, r.height].map((v) => Math.round(v * 10) / 10); }, tut);
  const box0 = await eyeBox();
  await page.focus(`.sp-cat[data-event="${tut}"] .sp-cat-eye`);
  await page.keyboard.press("Enter");
  await until(page, "the eye hides the Tutorium", ([key, tut]) => (localStorage.getItem(key) ?? "").includes(`e\t2026W\t${tut}\n`) && onPage(`.sp-cat[data-event="${tut}"] .sp-cat-eye`)?.getAttribute("aria-checked") === "false", [PLAN, tut]);
  e = await eye(page, tut);
  check(e?.off && e.sum === "ausgeblendet" && e.icon === "eye-off", `the hidden row: ${JSON.stringify(e)}`);
  check(await page.evaluate((tut) => document.activeElement === onPage(`.sp-cat[data-event="${tut}"] .sp-cat-eye`), tut), "the keyboard left the eye");
  check(same(await eyeBox(), box0), `the eye moved: ${JSON.stringify(box0)} → ${JSON.stringify(await eyeBox())}`);
  check(await page.evaluate(() => allOnPage(".sp-week a.slot.faint").some((slot) => slot.getAttribute("href").includes("termin=150132-"))), "the Tutorium is not faint in the week");
  await page.keyboard.press("Enter");
  await until(page, "the eye shows it again", ([key, before]) => localStorage.getItem(key) === before, [PLAN, before]);
  check((await stored(page)).includes("c\t2026W\t148369-aaf38\n"), "the eye took the tick");
  // Looking: no button; a hidden row's crossed eye leads into the mode.
  await click(page, `.sp-cat[data-event="${tut}"] .sp-cat-eye`, "the eye");
  await until(page, "hidden again", ([key, tut]) => (localStorage.getItem(key) ?? "").includes(`e\t2026W\t${tut}\n`), [PLAN, tut]);
  await click(page, ".sp-editbar .sp-done", "„Fertig“");
  await until(page, "the list while looking", looking);
  check(await page.evaluate(() => allOnPage(".sp-list button").filter((b) => b.getClientRects().length).length) === 0, "looking: a button in the list");
  e = await eye(page, tut);
  check(e?.tag === "a" && e.href === "/studyplan?edit=1" && e.icon === "eye-off" && e.label === "Tutorium ausgeblendet: Stundenplan bearbeiten", `looking, the crossed eye: ${JSON.stringify(e)}`);
  const shown = await eye(page, "148303");
  check(shown?.tag === "i" && !shown.href && !shown.icon, `looking, a shown row's eye: ${JSON.stringify(shown)}`);
  await click(page, `.sp-cat[data-event="${tut}"] a.eye`, "the crossed eye while looking");
  await until(page, "the crossed eye leads into the mode", editing);
  await marker(page, "the eye");
  await context.close();
}

// ---------- what is hidden, and „Alle einblenden" ----------
if (block("hidden")) {
  // A lecture whose every Termin is hidden one by one.
  {
    const { page, context } = await open(wide, FS1 + "r\t2026W\t148008-8e4ea\n", "/studyplan?edit=1");
    await until(page, "the list while editing", editing);
    let e = await eye(page, "148008");
    check(e?.off && e.sum === "ausgeblendet" && e.checked === "false" && e.icon === "eye-off", `PP's lecture hidden Termin by Termin: ${JSON.stringify(e)}`);
    await click(page, '.sp-cat[data-event="148008"] .sp-cat-eye', "its eye");
    await until(page, "its eye shows it again", ([key]) => !(localStorage.getItem(key) ?? "").includes("r\t2026W\t148008-8e4ea") && onPage('.sp-cat[data-event="148008"] .sp-cat-eye')?.getAttribute("aria-checked") === "true", [PLAN]);
    e = await eye(page, "148008");
    check(e?.sum === "Fr 11:30 A" && !e.off, `shown again: ${JSON.stringify(e)}`);
    await context.close();
  }
  // Three groups hidden one by one, a date hidden, a tick: signs; „Alle einblenden" while editing.
  {
    const hidden = ["a4d12", "6bb8d", "f09d2"].map((fp) => `r\t2026W\t148369-${fp}\n`).join("") + "h\t2026W\t148701-b7025\t2026-11-20\n";
    const plan = FS1 + "c\t2026W\t148370-ef2fb\n" + hidden;
    const { page, context } = await open(wide, plan);
    await until(page, "the list while looking", looking);
    let ue = await eye(page, "148369");
    check(ue?.sum === "Mo 15:30" && ue.sign === "3 Gruppen ausgeblendet" && !ue.signShown && ue.tag === "a" && ue.label === "3 Gruppen ausgeblendet: Stundenplan bearbeiten", `looking, three groups hidden: ${JSON.stringify(ue)}`);
    check(!(await page.evaluate(() => onPage(".sp-list-foot"))), "looking: the foot „Alle einblenden“");
    await click(page, "#sp-edit-toggle", "„Bearbeiten“");
    await until(page, "the list while editing", editing);
    ue = await eye(page, "148369");
    const ve = await eye(page, "148701");
    check(ue?.sign === "3 Gruppen ausgeblendet" && ue.signShown && ue.checked === "true", `editing, three groups hidden: ${JSON.stringify(ue)}`);
    check(ve?.sign === "am Fr 20.11. ausgeblendet" && ve.signShown && ve.sum === "Di 11:30 · Fr 09:15", `editing, a date hidden: ${JSON.stringify(ve)}`);
    const foot = await page.evaluate(() => onPage(".sp-list-foot")?.textContent.replace(/\s+/g, " ").trim());
    check(foot === "4 ausgeblendet · Alle einblenden", `the foot: „${foot}"`);
    await click(page, "#sp-show-all", "„Alle einblenden“");
    await until(page, "„Alle einblenden“", ([key]) => !/^[rh]\t/m.test(localStorage.getItem(key) ?? "") && /4 wieder eingeblendet/.test(onPage(".plan-note .mark-toast-said")?.textContent ?? ""), [PLAN]);
    check((await stored(page)).includes("c\t2026W\t148370-ef2fb\n"), "„Alle einblenden“ took the tick");
    check(!(await page.evaluate(() => onPage(".sp-list-foot"))), "the foot after „Alle einblenden“");
    await click(page, ".plan-note .plan-toast-undo", "„Rückgängig“");
    // The same lines (the store writes them in its own order).
    await until(page, "„Rückgängig“ brings all of it back", ([key, plan]) => JSON.stringify((localStorage.getItem(key) ?? "").split("\n").filter(Boolean).sort()) === JSON.stringify(plan.split("\n").filter(Boolean).sort()), [PLAN, plan]);
    await marker(page, "what is hidden");
    await context.close();
  }
}

// ---------- no summary cut ----------
if (block("cut")) {
  for (const [name, plan] of [["the example week", FS1], ["the real modules", REAL]]) {
    for (const path of ["/studyplan", "/studyplan?edit=1"]) {
      const { page, context } = await open({ viewport: { width: 1280, height: 800 } }, plan, path);
      await until(page, `${name} at 1280`, settled);
      await page.waitForTimeout(300);
      const cut = await page.evaluate(() => allOnPage(".sp-list .sp-cat-sum .sp-cat-p").flatMap((part) => {
        const over = part.scrollWidth > part.clientWidth + 0.5 && getComputedStyle(part).overflow !== "visible";
        const broken = !part.classList.contains("words") && part.getClientRects().length > 1;
        const outside = part.getBoundingClientRect().right > part.closest(".sp-cat").getBoundingClientRect().right + 0.5;
        return over || broken || outside ? [`${part.textContent}${over ? " cut" : ""}${broken ? " broken" : ""}${outside ? " outside" : ""}`] : [];
      }));
      check(cut.length === 0, `${name}, ${path} at 1280 px: ${JSON.stringify(cut)}`);
      await context.close();
    }
  }
}

// ---------- a row to the overview, and back to the plan where it was ----------
if (block("back")) {
  const { page, context } = await open({ viewport: { width: 1280, height: 800 } }, REAL);
  await until(page, "the real modules", () => allOnPage(".sp-list .sp-mod").length === 6);
  // The column scrolled to a lower row: Medizinische Grundlagen's second Praktikum.
  const row = '.sp-cat[data-event="150585"] a.sp-cat-row';
  await page.evaluate((row) => onPage(row)?.scrollIntoView({ block: "center" }), row);
  await page.waitForTimeout(300);
  const at = () => page.evaluate((row) => ({ side: Math.round(onPage(".sp-side")?.scrollTop ?? 0), page: Math.round(scrollY), y: Math.round(onPage(row)?.getBoundingClientRect().top ?? -1) }), row);
  const before = await at();
  await click(page, row, "the row „Praktikum · Do“ of MeG");
  await until(page, "the overview at that event", () => location.search === "?termin=150585-1f122&full=1" && Boolean(onPage(".is-termin")) && !document.querySelector(".pending-page.held"));
  const seg = await page.evaluate(() => allOnPage(".tm-evs > a[aria-checked='true']").map((a) => a.textContent.trim()));
  check(same(seg, ["Praktikum · Do"]), `the overview's chosen segment: ${JSON.stringify(seg)}`);
  await click(page, "[data-action='back'] >> visible=true", "„Zurück“");
  await until(page, "back to the plan", () => location.search === "" && allOnPage(".sp-list .sp-mod").length === 6 && !document.querySelector(".pending-page.held"));
  await page.waitForTimeout(400);
  const after = await at();
  check(Math.abs(after.y - before.y) <= 2 && Math.abs(after.side - before.side) <= 2 && Math.abs(after.page - before.page) <= 2, `back, the plan is not where it was: ${JSON.stringify({ before, after })}`);
  check(await page.evaluate((row) => document.activeElement === onPage(row), row), "back, the row does not have the keyboard");
  await marker(page, "the way back");
  await context.close();
}

// ---------- Termine of one's own in the list ----------
if (block("own")) {
  const OWN = "t\t1\t0\t2026W\tw\t2\t16:00-17:30\t\t\t12104\t\t\t\tlecture\n";
  const { page, context } = await open(wide, FS1 + OWN + `m\t2026W\t${PRAK_BS}\t1790000000\t\n`);
  await until(page, "the list while looking", looking);
  // A module QIS has nothing for, in both states.
  const none = () => page.evaluate((id) => onPage(`.sp-mod[data-id="${id}"] .sp-cat-none`)?.textContent.replace(/\s+/g, " ").trim(), PRAK_BS);
  check((await none()) === "keine Termine in QIS · Termin eintragen", `looking, PBS: „${await none()}"`);
  // The Termin of one's own under its module, marked „von dir"; it opens its card.
  const own = await page.evaluate(() => {
    const cat = onPage('.sp-mod[data-id="12104"] .sp-cat.own');
    return cat && { word: cat.querySelector("b")?.textContent, sum: cat.querySelector(".sp-cat-sum")?.textContent.trim(), mark: cat.querySelector(".sp-key.own")?.getAttribute("aria-label"), href: cat.querySelector("a.sp-cat-row")?.getAttribute("href") };
  });
  check(same(own, { word: "Vorlesung", sum: "Di 16:00", mark: "von dir", href: "/studyplan?termin=4000000001-00000" }), `the Termin of one's own under EvS: ${JSON.stringify(own)}`);
  check(await page.evaluate(() => !onPage(".sp-list .sp-own .sp-row.own")), "the module's Termin under „Eigene Termine“");
  check(await page.evaluate(() => allOnPage(".sp-list .sp-own-add").filter((b) => b.getClientRects().length).length) === 0, "looking: a module's „+ Termin“");
  await click(page, '.sp-mod[data-id="12104"] .sp-cat.own a.sp-cat-row', "its row");
  await until(page, "its card", () => /von dir/.test(onPage("dialog[open] .tm-card .tm-bar")?.textContent ?? "") && onPage("dialog[open] .tm-card .tm-title")?.textContent.includes("Entwicklung von Softwaresystemen"));
  await rest(page);
  await page.keyboard.press("Escape");
  await until(page, "the card closed", () => !document.querySelector("dialog[open]") && location.search === "");
  // „+ Termin" of EvS, while editing.
  await click(page, "#sp-edit-toggle", "„Bearbeiten“");
  await until(page, "the list while editing", editing);
  check((await none()) === "keine Termine in QIS · Termin eintragen", `editing, PBS: „${await none()}"`);
  check((await page.getAttribute('.sp-mod[data-id="12104"] .sp-own-add', "aria-label").catch(() => null)) === "Termin für EvS eintragen", "EvS's „+ Termin“ is not named for it");
  await click(page, '.sp-mod[data-id="12104"] .sp-own-add', "„+ Termin“ of EvS");
  await until(page, "the form for EvS", () => onPage("dialog.own-form[open] #own-module")?.value === "12104" && Boolean(onPage("dialog.own-form[open] #own-kind")));
  const sub = await page.evaluate(() => onPage("dialog.own-form[open] .st-dlg-head p")?.textContent.trim());
  check(sub === "Für EvS · WiSe 2026/27", `the form's line: „${sub}"`);
  const labels = await page.evaluate(() => [...(onPage("dialog.own-form[open] .own-body")?.children ?? [])].map((field) => field.querySelector("label, .own-lb")?.textContent.trim() ?? ""));
  check(labels.indexOf("Art") === labels.indexOf("Gehört zu") + 1 && labels.indexOf("Art") > 0, `the form's „Art“ after „Gehört zu“: ${JSON.stringify(labels)}`);
  check((await page.evaluate(() => onPage("dialog.own-form[open] #own-kind")?.value)) === "", "the form's „Art“ is not „Termin“ at first");
  await rest(page);
  await page.keyboard.press("Escape");
  await until(page, "the form closed", () => !document.querySelector("dialog.own-form[open]"));
  await marker(page, "Termine of one's own");
  await context.close();
}

// ---------- nothing manages the study's modules ----------
if (block("manage")) {
  const { page, context } = await open(wide, FS1);
  for (const state of ["looking", "editing"]) {
    if (state === "editing") {
      await click(page, "#sp-edit-toggle", "„Bearbeiten“");
      await until(page, "the list while editing", editing);
    } else await until(page, "the list while looking", looking);
    const seen = await page.evaluate(() => ({
      add: allOnPage(".sp-list .sp-add-mod").length,
      remove: allOnPage(".sp-list .sp-remove, .sp-list [data-icon='x']").length,
      credits: allOnPage(".sp-list .lp").length + (/\bLP\b/.test(onPage(".sp-list")?.innerText ?? "") ? 1 : 0),
      numbers: allOnPage(".sp-list .mono").length + (/\b1\d{4}\b/.test(onPage(".sp-list")?.innerText ?? "") ? 1 : 0),
      areas: allOnPage(".sp-list .sp-area, .sp-list a.open-slot").length,
    }));
    check(same(seen, { add: 0, remove: 0, credits: 0, numbers: 0, areas: 0 }), `${state}: the list manages modules: ${JSON.stringify(seen)}`);
  }
  // The module's panel.
  await page.goto(base + "/studyplan?edit=1&open=12104", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
  await until(page, "the module's panel", () => Boolean(onPage(".work > .detail.aside#preview")) && /Entwicklung von Softwaresystemen/.test(onPage("#preview")?.textContent ?? ""));
  const panel = await page.evaluate(() => ({
    remove: allOnPage("#preview button, #preview a").some((el) => /^Entfernen/.test(el.textContent.trim())),
    counts: Boolean(onPage("#sp-counts-for")) || /Zählt für/.test(onPage("#preview")?.innerText ?? ""),
    also: Boolean(onPage(".sp-also")) || /Auch geplant/.test(onPage("#preview")?.innerText ?? ""),
  }));
  check(same(panel, { remove: false, counts: false, also: false }), `the module's panel manages it: ${JSON.stringify(panel)}`);
  await context.close();
}

// ---------- a calendar of one's own ----------
if (block("calendar")) {
  const { page, context } = await open(wide, FS1, "/studyplan?edit=1");
  await until(page, "the mode", editing);
  await click(page, ".sp-cals-head .sp-cal-add", "„+ Kalender“");
  await until(page, "the form „Neuer Kalender“", () => document.activeElement?.id === "sp-cal-new-name");
  await page.fill("#sp-cal-new-name", "Lena");
  await click(page, "form.sp-cal-new button[type=submit]", "„Anlegen“");
  await until(page, "the calendar by its name alone", ([key]) => (localStorage.getItem(key) ?? "").includes("l\t1\t\t\tLena\n") && onPage(".sp-cal-mods .sp-list-head h2")?.textContent.trim() === "Lena", [PLAN]);
  // „+ Modul" of the calendar: the catalog's search, an `n` line, one line in the part.
  await click(page, ".sp-cal-mods .sp-add-mod", "„+ Modul“ of „Lena“");
  await until(page, "„Modul in „Lena““", () => onPage(".sp-cal-mods .sp-addp-head b")?.textContent === "Modul in „Lena“" && onPage(".sp-cal-mods .sp-addp-says")?.textContent === "Module in „Lena“ zählen nirgends.");
  await page.fill(".sp-cal-mods .sp-addp .sp-marked-search input", "12105");
  await until(page, "the search finds EiP", () => Boolean(onPage('.sp-cal-mods .sp-addp li[data-id="12105"] .mini')));
  await click(page, '.sp-cal-mods .sp-addp li[data-id="12105"] .mini', "„Aufnehmen“");
  await until(page, "EiP in „Lena“", ([key]) => (localStorage.getItem(key) ?? "").includes("n\t1\t2026W\t12105") && Boolean(onPage('.sp-cal-mods a.sp-mod-row[href*="open=12105"]')), [PLAN]);
  const line = await page.evaluate(() => { const a = onPage('.sp-cal-mods a.sp-mod-row[href*="open=12105"]'); return a && { href: a.getAttribute("href"), text: a.innerText.replace(/\s+/g, " ").trim(), high: Math.round(a.getBoundingClientRect().height) }; });
  check(line?.href === "/studyplan?edit=1&open=12105&full=1" && line.text === "EiP Einführung in die Programmierung" && line.high < 48, `its line: ${JSON.stringify(line)}`);
  check(await page.evaluate(() => !(localStorage.getItem("betula.studyplan.v1") ?? "").match(/^[mv]\t2026W\t12105/m)), "a module of „Lena“ was planned or visited");
  // Looking: no „+ Modul", no bin; the line stays.
  await click(page, ".sp-editbar .sp-done", "„Fertig“");
  await until(page, "looking", looking);
  const still = await page.evaluate(() => ({ add: allOnPage(".sp-cal-mods .sp-add-mod").filter((b) => b.getClientRects().length).length, bin: allOnPage(".sp-cal-mods .sp-remove").filter((b) => b.getClientRects().length).length, line: Boolean(onPage('.sp-cal-mods a.sp-mod-row[href*="open=12105"]')) }));
  check(same(still, { add: 0, bin: 0, line: true }), `„Lena“ while looking: ${JSON.stringify(still)}`);
  await click(page, "#sp-edit-toggle", "„Bearbeiten“");
  await until(page, "the mode again", editing);
  // The bin, and „Rückgängig".
  const bin = await page.evaluate(() => onPage(".sp-cal-mods .sp-remove")?.querySelector("use")?.getAttribute("href")?.split("#")[1]);
  check(bin === "trash-2", `the calendar's module carries ${bin}, not the bin`);
  await click(page, ".sp-cal-mods .sp-remove", "the bin of EiP in „Lena“");
  await until(page, "EiP taken out of „Lena“", ([key]) => !(localStorage.getItem(key) ?? "").includes("n\t1\t2026W\t12105") && /aus „Lena“ genommen/.test(onPage(".wait-note")?.textContent ?? ""), [PLAN]);
  await click(page, "#plan-note-undo", "„Rückgängig“");
  await until(page, "EiP back in „Lena“", ([key]) => (localStorage.getItem(key) ?? "").includes("n\t1\t2026W\t12105"), [PLAN]);
  // A Termin of one's own in it.
  await click(page, ".sp-cal-mods .sp-own-add", "„+ Termin“ of „Lena“");
  await until(page, "the form in „Lena“", () => Boolean(onPage("dialog.own-form[open] #own-what")));
  await page.fill("dialog.own-form[open] #own-what", "Lerngruppe");
  await page.selectOption("dialog.own-form[open] #own-day", "3");
  await page.fill('dialog.own-form[open] .own-two input[type="time"] >> nth=0', "18:00");
  await page.fill('dialog.own-form[open] .own-two input[type="time"] >> nth=1', "19:30");
  await click(page, "dialog.own-form[open] .st-dlg-foot .primary", "„Anlegen“");
  await until(page, "the Termin in „Lena“", ([key]) => /^t\t\d+\t1\t2026W\tw\t3\t18:00-19:30\t.*\tLerngruppe\t/m.test(localStorage.getItem(key) ?? "") && Boolean(onPage(".sp-cal-mods .sp-own .sp-row.own")), [PLAN]);
  if (shots) await page.screenshot({ path: `${shots}/categories-calendar.png` });
  await marker(page, "a calendar of one's own");
  await context.close();
}

// ---------- the empty week ----------
if (block("empty")) {
  for (const [name, options] of [["wide", wide], ["phone", phone]]) {
    const { page, context } = await open(options, "", "/studyplan");
    await until(page, `${name}: the empty week`, () => onPage(".sp-empty .state-title")?.textContent === "Noch keine Termine");
    const ways = await page.evaluate(() => allOnPage(".sp-empty .state-actions > *").map((el) => [el.textContent.trim(), el.getAttribute("href")]));
    check(same(ways, [["Zu „Mein Studium“", "/study?plan=1"], ["Zum Katalog", "/catalog?program=bachelor-informatik-2008&fits=2026W"], ["Termin", null]]), `${name}: the empty week's ways: ${JSON.stringify(ways)}`);
    await click(page, ".sp-empty .state-actions button", "„+ Termin“ of the empty week");
    await until(page, `${name}: „+ Termin“ opens the form`, () => Boolean(document.querySelector("dialog.own-form[open]")));
    await rest(page);
    await page.keyboard.press("Escape");
    await until(page, `${name}: the form closed`, () => !document.querySelector("dialog.own-form[open]"));
    await page.goto(base + "/studyplan?edit=1", { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
    await until(page, `${name}: editing, the week without the card`, () => Boolean(onPage(".page-inner.sp.sp-edit")) && allOnPage(".sp-week .week, .sp-carousel .week").length > 0 && !onPage(".sp-empty"));
    check((await stored(page)) === "", `${name}: looking at the empty week stored something`);
    await context.close();
  }
}

await browser.close();
if (problems.length) {
  console.error(`categories: ${problems.length} problem(s)\n- ${problems.join("\n- ")}`);
  process.exit(1);
}
console.log("categories: ok");
