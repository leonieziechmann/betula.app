// Checks the keyboard of the catalog and of what is global (enhance.js, „the keyboard";
// folia/design/tastatur/konzept.html; docs/folia/frontend.md R24): ↓ from nowhere goes into the
// list at the open module; F leads into the filters, which wear the ring and „Esc zum Verlassen"
// and light the section the keyboard is in; Tab goes from section to section, opens „Weitere
// Filter" on its way and stops at either end without leaving; the arrows go from filter to filter
// (↓ in the same column, → in reading order); + − Entf set a chip to with, without and off, and
// the bar at the foot names them; typing opens the bubble „Finden", which marks what it found,
// forgives a typo where nothing matches as typed, sets a chip with Enter (with) and Shift + Enter
// (without) and goes back with Esc; at a knob of the credits the arrows go on and keep its value,
// + − Bild ↓ Ende change it, one knob takes the other along, a digit opens a field for the number;
// ← → at an option group choose; Esc leads out into the list; Tab goes round the content (the
// heads, the list, the module, its text, the ground) and never into the filters; V and Enter on
// the open row open the module in full; Ctrl + F6 goes from region to region; ? opens the overview
// of the keys, marked where the keyboard was, whose switch turns the single keys off (F does
// nothing then, and its hint is gone); the rail is one stop of Tab whose ↓ goes from tab to tab;
// Esc in the search leads into the list; a combination is written with „+" on a PC and as signs on
// a Mac; on the Merkliste ↓ goes from row to row, F into its sidebar as a mode, Esc back to the row,
// Entf takes a row off with „Rückgängig“ and ? marks „In der Merkliste“; a chosen lecturer is one
// stop whose ← → Enter and Entf choose and take it out, the picker's list stays whole in the area;
// on „Mein Studium" the rows of a semester are a stop, F leads into its views, ← → at the
// semester's name turn it, Entf takes a row out, Shift + F10 opens its menu, „Befehle" offers what
// it does, and the Gesamtplan and the Regelstudienplan are tables the arrows go through.
// On the synthetic catalog
// (docs/folia/frontend.md §4) as on the real one.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node tastatur.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a console error or a step that does not come out as it should.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const MOMENT_MS = 450;

// A page of the app on a wide screen, on the system `platform` names (undefined: the browser's).
const open = async (path, platform) => {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, serviceWorkers: "block" });
  await context.addInitScript({ path: new URL("./on-page.js", import.meta.url).pathname });
  if (platform) {
    await context.addInitScript((platform) => {
      Object.defineProperty(Navigator.prototype, "platform", { get: () => platform });
      Object.defineProperty(Navigator.prototype, "userAgentData", { get: () => undefined });
    }, platform);
  }
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`pageerror: ${String(e).slice(0, 300)}`));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await page.waitForTimeout(800);
  return { context, page };
};

// Where the keyboard is: the element, its region, its section and what is lit.
const at = (page) => page.evaluate(() => {
  const a = document.activeElement;
  const section = a?.closest?.("[data-section]");
  return {
    tag: a?.tagName.toLowerCase(),
    id: a?.id || "",
    cls: a?.className || "",
    role: a?.getAttribute("role") || "",
    label: (a?.getAttribute("aria-label") || a?.textContent || "").trim().replace(/\s+/g, " ").slice(0, 60),
    region: a?.closest?.("[data-region]")?.dataset.region || "",
    section: section ? section.getAttribute("aria-labelledby") || section.getAttribute("aria-label") || section.className : "",
    lit: document.querySelector("[data-kb-current]")?.getAttribute("aria-labelledby") || document.querySelector("[data-kb-current]")?.getAttribute("aria-label") || "",
    value: a?.value ?? "",
  };
});
const search = (page) => new URL(page.url()).search;
const params = (page) => new URLSearchParams(search(page));
const press = async (page, key, wait = 60) => { await page.keyboard.press(key); await page.waitForTimeout(wait); };
// Presses `key` until `until(at)` holds, at most `most` times; the places on the way.
const pressUntil = async (page, key, until, most = 40) => {
  const way = [];
  for (let i = 0; i < most; i++) {
    await press(page, key);
    const now = await at(page);
    way.push(now);
    if (until(now)) return { now, way };
  }
  return { now: null, way };
};
const url = (page, test, timeout = 5000) => page.waitForFunction(test, null, { timeout }).then(() => true, () => false);

// ---------- desktop ----------
{
  const { context, page } = await open("/catalog?open=12330");
  // ↓ with the keyboard nowhere: into the list, at the open module.
  await press(page, "ArrowDown");
  let now = await at(page);
  check(now.cls.includes("row") && now.role === "option" && /Datenbanken/.test(now.label), `↓ from nowhere: not at the open row (${JSON.stringify(now)})`);
  const position = await page.evaluate(() => { const a = document.activeElement; return [a.getAttribute("aria-posinset"), a.getAttribute("aria-setsize"), a.closest('[role="listbox"]')?.getAttribute("aria-label")]; });
  check(position[0] && position[1] && position[2], `the row has no place in a list box (${JSON.stringify(position)})`);

  // F: into the filters, at the program picker; the ring, the tag, the lit section.
  await press(page, "f", 150);
  now = await at(page);
  check(now.region === "side" && now.id === "pick-program", `F: not at the program picker (${JSON.stringify(now)})`);
  const look = await page.evaluate(() => {
    const filters = onPage("#filters");
    const tag = document.querySelector(".mode-tag[data-on]");
    const style = getComputedStyle(filters);
    return { kb: document.documentElement.hasAttribute("data-kb"), ring: `${style.outlineStyle} ${style.outlineWidth}`, tag: tag && getComputedStyle(tag).display !== "none" ? tag.textContent : "", lit: !!filters.querySelector("[data-kb-current]") };
  });
  check(look.kb && look.ring === "solid 2px", `F: the filters wear no ring (${JSON.stringify(look)})`);
  check(/Esc/.test(look.tag) && /Verlassen/.test(look.tag), `F: no „Esc zum Verlassen" (${JSON.stringify(look)})`);
  check(look.lit, "F: the program's section is not lit");

  // Tab: section by section, „Weitere Filter" open on the way, and never out of the filters.
  let { now: end, way } = await pressUntil(page, "Tab", (n) => n.region !== "side" || /location/.test(n.section));
  check(end && end.region === "side", `Tab: the filters were left, or „Standort" never came (${JSON.stringify(way.map((w) => w.section))})`);
  check(way.every((w) => w.region === "side"), "Tab: left the filters on the way");
  check(new Set(way.map((w) => w.section)).size === way.length, `Tab: a section twice (${way.map((w) => w.section).join(", ")})`);
  check(way.some((w) => w.tag === "summary") && way.some((w) => /lecturers/.test(w.section)), `Tab: „Weitere Filter" did not open on the way (${way.map((w) => w.section).join(", ")})`);
  await press(page, "Tab");
  await press(page, "Tab");
  now = await at(page);
  const bump = await page.evaluate(() => onPage("#filters").getAttribute("data-bump"));
  check(now.region === "side" && bump === "end", `Tab at the end: left the filters or no nudge (${JSON.stringify(now)}, ${bump})`);
  // At the top „Zurücksetzen", the head's, before the first section.
  ({ way } = await pressUntil(page, "Shift+Tab", (n) => n.label === "Zurücksetzen", 30));
  await press(page, "Shift+Tab");
  now = await at(page);
  check(now.label === "Zurücksetzen" && way.every((w) => w.region === "side"), `Shift+Tab at the top: left the filters (${JSON.stringify(now)})`);
  await press(page, "Tab");

  // The arrows: ↓ in the same column, → in reading order, the section lit with them.
  ({ now } = await pressUntil(page, "Tab", (n) => /fl-exam/.test(n.section), 8));
  check(now && /Klausur/.test(now.label), `Tab: „Prüfung" not entered at „Klausur" (${JSON.stringify(now)})`);
  await press(page, "ArrowDown");
  now = await at(page);
  check(/Hausarbeit/.test(now.label) && now.lit === "fl-exam", `↓ from „Klausur": not at „Hausarbeit" (${JSON.stringify(now)})`);
  await press(page, "ArrowRight");
  now = await at(page);
  check(/Vortrag/.test(now.label), `→ from „Hausarbeit": not at „Vortrag" (${JSON.stringify(now)})`);
  const bar = await page.evaluate(() => { const b = onPage("#filters .key-bar"); return b && getComputedStyle(b).display !== "none" ? b.textContent : ""; });
  check(/mit/.test(bar) && /ohne/.test(bar) && /aus/.test(bar), `the bar names no + − Entf at a chip (${bar})`);

  // + − Entf at a chip.
  await press(page, "+");
  check(await url(page, () => new URLSearchParams(location.search).get("exam") === "presentation"), `+: „Vortrag" not wanted (${search(page)})`);
  await press(page, "-");
  check(await url(page, () => new URLSearchParams(location.search).get("not-exam") === "presentation" && !new URLSearchParams(location.search).get("exam")), `−: „Vortrag" not left out (${search(page)})`);
  await press(page, "Delete");
  check(await url(page, () => !location.search.includes("presentation")), `Entf: „Vortrag" not off (${search(page)})`);

  // Finding: the bubble, the marks, Enter, a typo, Shift + Enter, Esc back.
  await page.keyboard.type("engl");
  await page.waitForTimeout(MOMENT_MS);
  const found = await page.evaluate(() => ({
    on: !!onPage("#filters .find-bubble[data-on]"),
    note: onPage("#filters .find-note")?.textContent || "",
    marks: CSS.highlights?.get("find-hit")?.size || 0,
    target: onPage("[data-find-target]")?.textContent.trim() || "",
  }));
  check(found.on && found.note === "1/1" && /Englisch/.test(found.target), `typing „engl": no bubble or no target (${JSON.stringify(found)})`);
  check(found.marks > 0 || typeof Highlight !== "function", `typing „engl": nothing marked (${JSON.stringify(found)})`);
  await press(page, "Enter", 200);
  now = await at(page);
  check(await url(page, () => new URLSearchParams(location.search).get("lang") === "en"), `Enter in the bubble: „Englisch" not wanted (${search(page)})`);
  check(/Englisch/.test(now.label), `Enter in the bubble: the keyboard not at „Englisch" (${JSON.stringify(now)})`);
  await page.keyboard.type("vorlseung");
  await page.waitForTimeout(MOMENT_MS);
  const like = await page.evaluate(() => onPage("#filters .find-note")?.textContent || "");
  check(/ähnlich/.test(like), `a typo („vorlseung") not forgiven (${like})`);
  await press(page, "Shift+Enter", 200);
  check(await url(page, () => new URLSearchParams(location.search).get("not-form") === "lecture" || /not-form=lecture/.test(location.search)), `Shift+Enter in the bubble: „Vorlesung" not left out (${search(page)})`);
  const origin = await at(page);
  await page.keyboard.type("kla");
  await page.waitForTimeout(MOMENT_MS);
  await press(page, "Escape", 150);
  now = await at(page);
  check(now.label === origin.label && now.region === "side" && !(await page.evaluate(() => !!onPage(".find-bubble[data-on]"))), `Esc in the bubble: not back where the typing began (${JSON.stringify([origin, now])})`);

  // The knobs of the credits.
  ({ now } = await pressUntil(page, "Tab", (n) => n.id === "credits-low", 10));
  if (!now) ({ now } = await pressUntil(page, "Shift+Tab", (n) => n.id === "credits-low", 12));
  check(now && now.id === "credits-low", "Tab: the credits not entered at the lower knob");
  await press(page, "+", 200);
  check(await url(page, () => new URLSearchParams(location.search).get("ects_min") === "1"), `+ at the lower knob: not 1 (${search(page)})`);
  await press(page, "ArrowRight");
  now = await at(page);
  check(now.id === "credits-high", `→ from the lower knob: not at the upper one (${JSON.stringify(now)})`);
  await press(page, "PageDown", 200);
  check(await url(page, () => new URLSearchParams(location.search).get("ects_max") === "25"), `Bild ↓ at the upper knob: not 25 (${search(page)})`);
  await press(page, "ArrowLeft");
  await press(page, "End", 200);
  check(await url(page, () => { const p = new URLSearchParams(location.search); return p.get("ects_min") === "30" && !p.get("ects_max"); }), `Ende at the lower knob: the upper one not taken along (${search(page)})`);
  await press(page, "Home", 200);
  await press(page, "1", 150);
  const field = await page.evaluate(() => { const f = onPage(".lp-edit input"); return f && document.activeElement === f ? f.getAttribute("aria-label") : ""; });
  check(/Zahl/.test(field), `a digit at a knob: no field for the number (${field})`);
  await page.keyboard.type("2");
  await press(page, "Enter", 300);
  now = await at(page);
  check(await url(page, () => new URLSearchParams(location.search).get("ects_min") === "12") && now.id === "credits-low", `12 Enter: not 12, or not back at the knob (${search(page)}, ${JSON.stringify(now)})`);
  const before = (await at(page)).value;
  await press(page, "ArrowDown");
  now = await at(page);
  check(now.id !== "credits-low" && (await page.evaluate(() => onPage("#credits-low").value)) === before, `↓ at a knob: changed it instead of going on (${JSON.stringify(now)})`);

  // An option group: ← → choose.
  ({ now } = await pressUntil(page, "Tab", (n) => /duration/.test(n.section), 12));
  check(now && now.role === "radio", `Tab: „Dauer" not entered at its choice (${JSON.stringify(now)})`);
  await press(page, "ArrowRight", 200);
  check(await url(page, () => new URLSearchParams(location.search).get("duration") === "1"), `→ at „Dauer": nothing chosen (${search(page)})`);
  const radioBar = await page.evaluate(() => onPage("#filters .key-bar")?.textContent || "");
  check(/wählen/.test(radioBar), `the bar names no ← → at an option group (${radioBar})`);

  // Esc: out into the list.
  await page.goto(base + "/catalog?open=12330");
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
  await page.waitForTimeout(800);
  await press(page, "f", 150);
  await press(page, "Tab");
  await press(page, "Escape", 150);
  now = await at(page);
  check(now.region === "content" && now.role === "option", `Esc in the filters: not in the list (${JSON.stringify(now)})`);
  const ringGone = await page.evaluate(() => getComputedStyle(onPage("#filters")).outlineStyle);
  check(ringGone === "none", "Esc: the ring stays on the filters");

  // Tab round the content: never into the filters, and round to where it began.
  const first = now.label;
  ({ now, way } = await pressUntil(page, "Tab", (n) => n.label === first, 40));
  check(now && way.every((w) => w.region === "content"), `Tab round the content: left it or did not come round (${JSON.stringify(way.map((w) => `${w.region}:${w.label.slice(0, 16)}`))})`);
  check(way.some((w) => /Vollbild/.test(w.label)) && way.some((w) => /Modul|Module|LP/.test(w.label) && w.tag === "a"), `Tab round the content: missed the module or the heads (${JSON.stringify(way.map((w) => w.label.slice(0, 16)))})`);

  // Enter on the open row and V: the module in full.
  await press(page, "Enter", 300);
  check(await url(page, () => location.pathname.endsWith("/catalog/module/12330")), `Enter on the open row: not in full (${page.url()})`);
  await page.goBack();
  await page.waitForTimeout(MOMENT_MS);
  await press(page, "ArrowDown");
  await press(page, "v", 300);
  check(await url(page, () => location.pathname.endsWith("/catalog/module/12330")), `V: not in full (${page.url()})`);
  await page.goBack();
  await page.waitForTimeout(MOMENT_MS);

  // Ctrl + F6: region by region, and round.
  await press(page, "ArrowDown");
  const regions = [];
  for (let i = 0; i < 4; i++) { await press(page, "Control+F6", 120); regions.push((await at(page)).region); }
  check(regions.join(",") === "nav,search,side,content", `Ctrl+F6: ${regions.join(",")}`);

  // The rail: one stop of Tab, ↓ from tab to tab, Tab out to the search; Esc there into the list.
  await press(page, "Control+F6");
  now = await at(page);
  check(now.region === "nav" && /Module/.test(now.label), `Ctrl+F6: not at the current tab (${JSON.stringify(now)})`);
  await press(page, "ArrowDown");
  now = await at(page);
  check(now.region === "nav" && /Studium/.test(now.label), `↓ in the rail: not at the next tab (${JSON.stringify(now)})`);
  await press(page, "Tab");
  now = await at(page);
  check(now.id === "topsearch", `Tab out of the rail: not at the search (${JSON.stringify(now)})`);
  await press(page, "Shift+Tab");
  now = await at(page);
  check(now.region === "nav", `Shift+Tab from the search: not in the rail (${JSON.stringify(now)})`);
  await press(page, "Tab");
  await press(page, "Escape", 150);
  now = await at(page);
  check(now.role === "option", `Esc in the search: not in the list (${JSON.stringify(now)})`);

  // ?: the overview, marked where the keyboard was; its switch turns the single keys off.
  await press(page, "?", 300);
  const overview = await page.evaluate(() => {
    const d = document.querySelector("dialog.keys-dialog[open]");
    return d ? { here: d.querySelector(".keys-group.here h3")?.textContent || "", groups: d.querySelectorAll(".keys-group").length } : null;
  });
  check(overview && /Liste/.test(overview.here) && overview.groups === 4, `?: no overview, or not marked at the list (${JSON.stringify(overview)})`);
  await page.click('dialog.keys-dialog [data-letters-value="off"]');
  await page.waitForTimeout(150);
  const off = await page.evaluate(() => [document.documentElement.dataset.letters, localStorage.getItem("betula.letters.v1")]);
  check(off[0] === "off" && off[1] === "off", `the switch: single keys not off (${off})`);
  await press(page, "Escape", 200);
  check(!(await page.evaluate(() => !!document.querySelector("dialog.keys-dialog[open]"))), "Esc: the overview stays open");
  const back = await at(page);
  await press(page, "f", 150);
  now = await at(page);
  check(now.region === back.region && now.label === back.label, `F with the single keys off: did something (${JSON.stringify(now)})`);
  const hint = await page.evaluate(() => [...onPage("#filters .panel-title").querySelectorAll("kbd")].some((k) => k.getClientRects().length));
  check(!hint, "the single keys off: „F\" still at the head of the filters");
  await page.evaluate(() => { localStorage.removeItem("betula.letters.v1"); delete document.documentElement.dataset.letters; });
  await context.close();
}

// ---------- the chosen lecturers ----------
// A chosen person is one stop (owner, 2026-10-26: „mit up and down durch die Personen … mit enter
// oder den pfeiltasten die selection ändern und halt del zum entfernen"): ↓ from the picker goes to
// the first person and on to the next, never into its switch's other half or its bin; → makes it
// „ohne", Enter „mit" again, the whole row is lit and the bar names ← → and Entf; Entf takes it out
// and the keyboard goes on to the person before, the last one back to the picker. The picker's list
// stands inside what shows the filters, with its search field, and the tag „Esc zum Verlassen" is
// gone while it is open; the ring lies inside the panel's edge.
{
  const { context, page } = await open("/catalog");
  await page.evaluate(() => { const more = onPage("#filters details.more"); if (more) more.open = true; });
  await page.click("#pick-lecturer >> visible=true");
  await page.waitForFunction(() => allOnPage("#pick-lecturer-list .combo-option").length > 3);
  const people = await page.evaluate(() => allOnPage("#pick-lecturer-list .combo-option .combo-label").slice(0, 2).map((el) => el.textContent));
  await press(page, "Escape", 200);
  await page.goto(`${base}/catalog?${people.map((name) => `lecturer=${encodeURIComponent(name)}`).join("&")}`, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true && allOnPage(".people .person").length === 2, null, { timeout: 60000 }).catch(() => problems.push("the lecturers: the two persons are not there"));
  await page.waitForTimeout(400);
  const here = () => page.evaluate(() => {
    const a = document.activeElement;
    const person = a?.closest?.(".person");
    return { id: a?.id || "", label: a?.getAttribute("aria-label") || "", checked: a?.getAttribute("aria-checked") || "", person: person?.querySelector(".person-name b")?.textContent || "", lit: person ? getComputedStyle(person).boxShadow : "", bar: onPage("#filters > .key-bar[data-on]")?.textContent || "" };
  });
  await press(page, "f", 250);
  const ring = await page.evaluate(() => getComputedStyle(onPage("#filters")).outlineOffset);
  check(ring === "-2px", `the filters' ring does not lie inside the panel's edge (${ring})`);
  await pressUntil(page, "Tab", (now) => /lecturers/.test(now.section));
  let now = await here();
  check(now.id === "pick-lecturer", `the lecturers: Tab did not come to the picker (${JSON.stringify(now)})`);
  await press(page, "ArrowDown", 120);
  now = await here();
  check(now.person === people[0] && now.checked === "true", `the lecturers: ↓ from the picker is not at the first person (${JSON.stringify(now)})`);
  check(/inset/.test(now.lit) && /entfernen/.test(now.bar), `the lecturers: the person is not lit, or the bar does not name Entf (${JSON.stringify(now)})`);
  await press(page, "ArrowDown", 120);
  now = await here();
  check(now.person === people[1] && now.checked === "true", `the lecturers: ↓ did not go to the next person (${JSON.stringify(now)})`);
  await press(page, "ArrowRight", 600);
  now = await here();
  const without = await page.evaluate(() => new URL(location.href).searchParams.getAll("not-lecturer"));
  check(without.join() === people[1] && now.person === people[1] && now.label === "ohne" && now.checked === "true", `the lecturers: → did not make the person „ohne“ with the keyboard on it (${JSON.stringify({ without, now })})`);
  await press(page, "Enter", 600);
  now = await here();
  const wanted = await page.evaluate(() => new URL(location.href).searchParams.getAll("lecturer"));
  check(wanted.length === 2 && now.person === people[1] && now.label === "mit", `the lecturers: Enter did not make the person „mit“ again (${JSON.stringify({ wanted, now })})`);
  await press(page, "ArrowUp", 120);
  check((await here()).person === people[0], "the lecturers: ↑ did not go back to the first person");
  await press(page, "ArrowDown", 120);
  await press(page, "Delete", 700);
  now = await here();
  const left = await page.evaluate(() => new URL(location.href).searchParams.getAll("lecturer"));
  check(left.join() === people[0] && now.person === people[0], `the lecturers: Entf did not take the last person out with the keyboard going to the one before (${JSON.stringify({ left, now })})`);
  await press(page, "Delete", 700);
  now = await here();
  check(now.id === "pick-lecturer" && !(await page.evaluate(() => location.search.includes("lecturer"))), `the lecturers: Entf at the last person did not lead back to the picker (${JSON.stringify(now)})`);
  // The picker's list, opened at the foot of the filters: inside what shows them, the tag gone.
  await press(page, "Enter", 500);
  const pop = await page.evaluate(() => {
    const list = onPage(".combo[data-open] .combo-pop")?.getBoundingClientRect();
    const search = onPage(".combo[data-open] .combo-search")?.getBoundingClientRect();
    const area = onPage(".work.flowing")?.getBoundingClientRect();
    const tag = document.querySelector(".mode-tag[data-on]");
    return list && area && { inside: list.top >= area.top && list.bottom <= area.bottom, search: !!search && search.top >= area.top, tag: !!tag && tag.getClientRects().length > 0 };
  });
  check(pop && pop.inside && pop.search && !pop.tag, `the lecturers' picker: its list is cut off by the area or under the tag (${JSON.stringify(pop)})`);
  await press(page, "Escape", 200);
  await context.close();
}

// ---------- the Merkliste ----------
// The same regions on the Merkliste (folia/design/tastatur/konzept.html, „Die anderen Seiten"): its
// rows a stop whose ↓ goes from row to row, F into the sidebar (the collections and the filters) as
// a mode with its ring, ↓ from collection to collection, Esc back to the row, Entf taking a row off
// with „Rückgängig", ? with „In der Merkliste" marked.
{
  const { context, page } = await open("/catalog");
  // Four modules marked with M, from the first row down.
  await page.locator(".rows a.row >> visible=true").first().focus();
  for (let i = 0; i < 4; i++) { await press(page, "m", 120); await press(page, "ArrowDown", 80); }
  await page.click('.rail .nav[data-area="bookmarks"] >> visible=true');
  await url(page, () => location.pathname === "/bookmarks" && document.querySelectorAll("#rows a.row").length >= 4);
  await page.waitForTimeout(400);
  const titles = await page.evaluate(() => allOnPage("#rows a.row").map((r) => r.dataset.id));
  await page.locator("#rows a.row >> visible=true").first().focus();
  await press(page, "ArrowDown", 150);
  let now = await page.evaluate(() => ({ id: document.activeElement?.dataset?.id || "", region: document.activeElement?.closest?.("[data-region]")?.dataset.region || "" }));
  check(now.id === titles[1] && now.region === "content", `the Merkliste: ↓ did not go to the second row (${JSON.stringify(now)}, ${JSON.stringify(titles)})`);
  // F: into the sidebar, a mode with its ring and its tag; ↓ from collection to collection.
  await press(page, "f", 250);
  now = await at(page);
  const ring = await page.evaluate(() => ({ kb: document.documentElement.hasAttribute("data-kb"), tag: document.querySelector(".mode-tag[data-on]")?.textContent || "", outline: getComputedStyle(onPage("#sidebar")).outlineStyle }));
  check(now.region === "side" && ring.kb && /Esc/.test(ring.tag) && ring.outline === "solid", `the Merkliste: F did not lead into the sidebar as a mode (${JSON.stringify({ now, ring })})`);
  // Esc: back to the row the keyboard left.
  await press(page, "Escape", 250);
  now = await page.evaluate(() => ({ id: document.activeElement?.dataset?.id || "", region: document.activeElement?.closest?.("[data-region]")?.dataset.region || "" }));
  check(now.id === titles[1] && now.region === "content", `the Merkliste: Esc did not lead back to the row (${JSON.stringify(now)})`);
  // Entf: the row off the Merkliste, dimmed in its place, the note with „Rückgängig".
  await press(page, "Delete", 400);
  const removed = await page.evaluate((id) => ({ pressed: onPage(`#rows .row-wrap:has(a.row[data-id="${id}"]) [data-action="mark"]`)?.getAttribute("aria-pressed") || "", note: document.querySelector(".wait-note")?.textContent || "", row: !!onPage(`#rows a.row[data-id="${id}"]`) }), titles[1]);
  check(removed.pressed === "false" && /Rückgängig/.test(removed.note) && removed.row, `the Merkliste: Entf did not take the row off with „Rückgängig“ (${JSON.stringify(removed)})`);
  // ?: the overview with „In der Merkliste" marked where the keyboard was.
  await press(page, "?", 300);
  const overview = await page.evaluate(() => document.querySelector("dialog.keys-dialog[open] .keys-group.here h3")?.textContent || "");
  check(/In der Merkliste/.test(overview), `the Merkliste: ? did not mark „In der Merkliste“ (${overview})`);
  await press(page, "Escape", 200);
  await context.close();
}

// ---------- „Mein Studium" ----------
// The same regions on „Mein Studium" (folia/design/tastatur/konzept.html, „Die anderen Seiten"): the
// rows of the semester a stop whose ↓ goes from row to row and on to „Module hinzufügen", their
// status no stop of Tab; F into the sidebar as a mode, at the view shown, ↓ from view to view,
// typing finds a view and Enter shows it, Esc back to the row; ← → at the semester's name turn it
// and keep the keyboard there; the space bar selects, Shift + ↓ takes in the next, Esc lets go;
// Shift + F10 opens the row's menu; Entf takes the row out with „Rückgängig", the keyboard on the
// next row; ? marks „In „Mein Studium“"; „Befehle" at a row offers its menu, passed and „Entfernen",
// and the views under „Gehe zu"; the Gesamtplan is one stop that Tab enters at the current
// semester, whose arrows go from cell to cell as the eye sees them and whose ? marks „Im
// Gesamtplan"; the Regelstudienplan's ↓ goes from module to module and → to „Einplanen".
{
  const { context, page } = await open("/catalog");
  // Six modules of the catalog planned into the current semester of Informatik B.Sc. in its third.
  await page.waitForFunction(() => allOnPage(".rows a.row").length >= 6, null, { timeout: 20000 });
  const ids = await page.evaluate(() => allOnPage(".rows a.row").map((row) => row.dataset.id).slice(0, 6));
  await page.evaluate((ids) => {
    localStorage.setItem("betula.myprogram.v1", "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n");
    localStorage.setItem("betula.studyplan.v1", ids.map((id, i) => `m\t2026W\t${id}\t${i + 1}\t\t`).join("\n") + "\n");
    localStorage.setItem("betula.study.view", "list");
  }, ids);
  await page.goto(base + "/study", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true && allOnPage(".st-card-sem .st-row .st-open").length >= 6, null, { timeout: 60000 }).catch(() => problems.push("„Mein Studium“: the six rows are not there"));
  await page.waitForTimeout(500);
  const where = () => page.evaluate(() => {
    const a = document.activeElement;
    return { cls: a?.className || "", key: a?.closest?.(".st-row")?.dataset.key || "", region: a?.closest?.("[data-region]")?.dataset.region || "", label: (a?.getAttribute("aria-label") || a?.textContent || "").trim().replace(/\s+/g, " ").slice(0, 60), current: a?.getAttribute("aria-current") || "" };
  });
  const keys = await page.evaluate(() => allOnPage(".st-card-sem .st-row[data-key]").map((row) => row.dataset.key));
  // ↓ from nowhere: into the rows, then from row to row; the status is no stop.
  await press(page, "ArrowDown", 120);
  let now = await where();
  check(now.key === keys[0] && /st-open/.test(now.cls) && now.region === "content", `„Mein Studium“: ↓ from nowhere is not at the first row (${JSON.stringify(now)})`);
  await press(page, "ArrowDown", 120);
  now = await where();
  check(now.key === keys[1] && /st-open/.test(now.cls), `„Mein Studium“: ↓ did not go to the second row (${JSON.stringify(now)})`);
  const stops = await page.evaluate(() => allOnPage(".st-card-sem .st-status").filter((b) => b.tabIndex >= 0).length);
  check(stops === 0, `„Mein Studium“: the rows' status is a stop of Tab (${stops})`);
  await press(page, "Tab", 120);
  now = await where();
  check(!/st-open|st-status/.test(now.cls), `„Mein Studium“: Tab stayed among the rows (${JSON.stringify(now)})`);
  await press(page, "Shift+Tab", 120);
  now = await where();
  check(now.key === keys[1], `„Mein Studium“: Shift + Tab did not come back to the row the keyboard left (${JSON.stringify(now)})`);
  // F: into the sidebar as a mode, at the view shown; ↓ to the next view; Esc back to the row.
  await press(page, "f", 250);
  now = await where();
  const mode = await page.evaluate(() => ({ tag: document.querySelector(".mode-tag[data-on]")?.textContent || "", outline: getComputedStyle(onPage("#sidebar")).outlineStyle }));
  check(now.region === "side" && now.current === "page" && /st-vl/.test(now.cls) && /Esc/.test(mode.tag) && mode.outline === "solid", `„Mein Studium“: F did not lead into the sidebar as a mode at the view shown (${JSON.stringify({ now, mode })})`);
  await press(page, "ArrowDown", 120);
  now = await where();
  check(/st-vl/.test(now.cls) && /Gesamtplan/.test(now.label), `„Mein Studium“: ↓ in the sidebar did not go to the next view (${JSON.stringify(now)})`);
  await press(page, "Escape", 250);
  now = await where();
  check(now.key === keys[1] && now.region === "content", `„Mein Studium“: Esc did not lead back to the row (${JSON.stringify(now)})`);
  // ← → at the semester's name.
  await page.focus(".st-card-sem .st-sem-pick");
  const named = () => page.evaluate(() => ({ name: onPage(".st-card-sem .st-sem-pick")?.textContent.trim() || "", focus: document.activeElement?.classList.contains("st-sem-pick") || false }));
  const first = await named();
  await press(page, "ArrowLeft", 400);
  const before = await named();
  check(before.name && before.name !== first.name && before.focus, `„Mein Studium“: ← at the semester's name did not turn to the one before with the keyboard there (${JSON.stringify({ first, before })})`);
  await press(page, "ArrowRight", 400);
  const back = await named();
  check(back.name === first.name && back.focus, `„Mein Studium“: → did not turn back (${JSON.stringify(back)})`);
  // The space bar selects, Shift + ↓ takes in the next, Esc lets go.
  await page.focus(`.st-card-sem .st-row[data-key="${keys[0]}"] .st-open`);
  await press(page, " ", 150);
  await press(page, "Shift+ArrowDown", 150);
  const selected = await page.evaluate(() => allOnPage(".st-card-sem .st-row.is-selected").map((row) => row.dataset.key));
  check(selected.join() === keys.slice(0, 2).join(), `„Mein Studium“: the space bar and Shift + ↓ did not select the two rows (${selected})`);
  await press(page, "Escape", 200);
  check((await page.evaluate(() => allOnPage(".st-card-sem .st-row.is-selected").length)) === 0, "„Mein Studium“: Esc did not let go of the rows selected");
  // Shift + F10: the row's menu, the keyboard in it; Esc closes it and gives the keyboard back.
  await page.focus(`.st-card-sem .st-row[data-key="${keys[2]}"] .st-open`);
  await press(page, "Shift+F10", 300);
  const menu = await page.evaluate(() => ({ open: !!document.querySelector(".st-pop.root"), inside: !!document.activeElement?.closest?.(".st-pop"), remove: !!document.querySelector(".st-pop.root #st-menu-remove, .st-pop.root [data-id=\"st-menu-remove\"]") || /Entfernen/.test(document.querySelector(".st-pop.root")?.textContent || "") }));
  check(menu.open && menu.inside && menu.remove, `„Mein Studium“: Shift + F10 did not open the row's menu with the keyboard in it (${JSON.stringify(menu)})`);
  await press(page, "Escape", 250);
  now = await where();
  check(now.key === keys[2], `„Mein Studium“: Esc out of the menu did not give the keyboard back to the row (${JSON.stringify(now)})`);
  // „Befehle" at the row: its menu, passed and „Entfernen"; the views under „Gehe zu".
  await press(page, "Control+k", 500);
  const offered = await page.evaluate(() => [...document.querySelectorAll(".cmd-dialog[open] .cmd-group")].map((g) => `${g.querySelector(".cmd-heading")?.textContent.trim()}: ${[...g.querySelectorAll(".cmd-label")].map((l) => l.textContent.trim()).join(" | ")}`));
  const rowGroup = offered.find((g) => g.startsWith("Für „")) || "";
  const go = offered.find((g) => g.startsWith("Gehe zu")) || "";
  check(/Menü öffnen/.test(rowGroup) && /Als bestanden markieren/.test(rowGroup) && /Entfernen/.test(rowGroup) && /Gesamtplan/.test(go) && /Regelstudienplan/.test(go) && /Ansichten und Studiengang/.test(go), `„Mein Studium“: „Befehle“ at a row does not offer what it should (${JSON.stringify(offered)})`);
  await press(page, "Escape", 250);
  // Entf: the row out with „Rückgängig", the keyboard on the next row.
  await page.focus(`.st-card-sem .st-row[data-key="${keys[2]}"] .st-open`);
  await press(page, "Delete", 500);
  const gone = await page.evaluate((key) => ({ row: !!onPage(`.st-card-sem .st-row[data-key="${key}"]`), note: document.querySelector(".study-note")?.textContent || "", at: document.activeElement?.closest?.(".st-row")?.dataset.key || "" }), keys[2]);
  check(!gone.row && /Rückgängig/.test(gone.note) && gone.at === keys[3], `„Mein Studium“: Entf did not take the row out with „Rückgängig“ and the keyboard on the next row (${JSON.stringify(gone)})`);
  // ?: „In „Mein Studium“" marked where the keyboard was.
  await press(page, "?", 300);
  const overview = await page.evaluate(() => document.querySelector("dialog.keys-dialog[open] .keys-group.here h3")?.textContent || "");
  check(/Mein Studium/.test(overview), `„Mein Studium“: ? did not mark its group (${overview})`);
  await press(page, "Escape", 250);
  // Typing in the sidebar finds a view, Enter shows it: the Gesamtplan.
  await press(page, "f", 250);
  for (const key of "gesamt") await press(page, key, 40);
  await page.waitForTimeout(200);
  const found = await page.evaluate(() => ({ bubble: !!document.querySelector(".find-bubble[data-on]"), target: document.querySelector("[data-find-target]")?.textContent.trim() || "", bar: onPage("#sidebar > .key-bar[data-on]")?.textContent || "" }));
  check(found.bubble && /Gesamtplan/.test(found.target) && /öffnen/.test(found.bar), `„Mein Studium“: typing in the sidebar did not find the Gesamtplan (${JSON.stringify(found)})`);
  await press(page, "Enter", 600);
  const grid = await url(page, () => !!document.querySelector(".st-rm .st-col-head"), 8000);
  check(grid, "„Mein Studium“: Enter at the view found did not show the Gesamtplan");
  // The Gesamtplan: Tab enters at the current semester; ↓ goes into its rows, ← → along them.
  await press(page, "Escape", 250);
  const enter = await pressUntil(page, "Tab", (now) => /st-col-head/.test(now.cls) || /st-rm-p|st-cell-add|st-rm-area/.test(now.cls), 30);
  now = await page.evaluate(() => ({ cls: document.activeElement?.className || "", home: document.activeElement?.hasAttribute("data-kb-home") || false }));
  check(enter.now && /st-col-head/.test(now.cls) && now.home, `„Mein Studium“: Tab did not enter the Gesamtplan at the current semester (${JSON.stringify(now)})`);
  const box = () => page.evaluate(() => { const r = document.activeElement.getBoundingClientRect(); return { cls: document.activeElement.className, x: r.left + r.width / 2, y: r.top + r.height / 2, plan: !!document.activeElement.closest(".st-rm") }; });
  const head = await box();
  await press(page, "ArrowDown", 150);
  const below = await box();
  check(below.plan && below.y > head.y && !/st-col-head/.test(below.cls), `„Mein Studium“: ↓ in the Gesamtplan did not go below the head (${JSON.stringify({ head, below })})`);
  await press(page, "ArrowLeft", 150);
  const left = await box();
  check(left.plan && left.x < below.x && Math.abs(left.y - below.y) < 30, `„Mein Studium“: ← in the Gesamtplan did not go left in the row (${JSON.stringify({ below, left })})`);
  await press(page, "?", 300);
  const planHelp = await page.evaluate(() => document.querySelector("dialog.keys-dialog[open] .keys-group.here h3")?.textContent || "");
  check(/Gesamtplan/.test(planHelp), `„Mein Studium“: ? in the Gesamtplan did not mark its group (${planHelp})`);
  await press(page, "Escape", 250);
  // The Regelstudienplan: ↓ from module to module, → to „Einplanen" in the row.
  await page.goto(base + "/study?view=plan", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true && allOnPage(".st-mx a.st-rsp-name").length >= 2, null, { timeout: 60000 }).catch(() => problems.push("„Mein Studium“: the Regelstudienplan has no modules"));
  // A row with „Einplanen", and the name after it.
  const rsp = await page.evaluate(() => {
    const names = allOnPage(".st-mx a.st-rsp-name");
    const at = names.findIndex((a) => a.closest("tr")?.querySelector(".st-addb"));
    return { at, name: names[at]?.textContent.trim() || "", next: names[at + 1]?.textContent.trim() || "" };
  });
  await page.locator(".st-mx a.st-rsp-name >> visible=true").nth(Math.max(0, rsp.at)).focus();
  await press(page, "ArrowDown", 150);
  now = await where();
  check(/st-rsp-name/.test(now.cls) && now.label === rsp.next, `„Mein Studium“: ↓ in the Regelstudienplan did not go to the next module (${JSON.stringify({ now, rsp })})`);
  await press(page, "ArrowUp", 150);
  await press(page, "ArrowRight", 150);
  now = await where();
  const row = await page.evaluate(() => document.activeElement?.closest("tr")?.querySelector(".st-rsp-name")?.textContent.trim() || "");
  check(/st-addb/.test(now.cls) && row === rsp.name, `„Mein Studium“: → in the Regelstudienplan did not go to „Einplanen“ in the row (${JSON.stringify({ now, row, rsp })})`);
  await context.close();
}

// ---------- the keys as each system writes them ----------
for (const [platform, said] of [["Linux x86_64", /^Strg\+K$/], ["MacIntel", /^⌘K$/]]) {
  const { context, page } = await open("/catalog", platform);
  const shown = await page.evaluate(() => [...document.querySelectorAll(".cmd-btn .k-pc, .cmd-btn .k-mac")].filter((k) => k.getClientRects().length).map((k) => k.textContent.replace(/\s+/g, "")).join("|"));
  check(said.test(shown), `${platform}: „Befehle“ names ${shown}`);
  await context.close();
}

await browser.close();
console.log(problems.length ? JSON.stringify({ problems }, null, 2) : "tastatur: ok");
process.exit(problems.length ? 1 : 0);
