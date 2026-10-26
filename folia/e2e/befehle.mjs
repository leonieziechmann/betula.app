// Checks „Befehle“ (folia/crates/shell/src/commands.rs; folia/design/tastatur/konzept.html,
// „Befehle“; docs/folia/frontend.md R25): the button beside the search field with its keys opens the
// palette, as Strg + K does, and its field has the keyboard; without a module open it offers „Gehe
// zu" (the tabs with their keys, the search field with /, the filters with F) and „Hilfe“, and says
// what typing finds besides; with a module open first what can be done with it („Für „…“":
// Merken M, Sammlung und Notiz, Vollbild V, Einplanen P), and Enter on „Merken“ marks it and says
// „Geht auch mit M“; ↓ and ↑ choose; typing finds a filter by its group and name, folded („prufung
// klaus"), whose Enter sets it „mit“ and again off, and Shift + Enter „ohne“; a module by its number
// opens beside the list with Enter and its page with Shift + Enter; on a module's page its sections
// are found; the settings are found and say what holds now; „Filter“ leads into the filters and
// „Suchfeld“ into the search; what finds nothing says so; Strg + K in the palette closes it, Esc as
// well, and the keyboard is back where it was; with the single keys off the commands name no single
// key; on the Merkliste its sidebar is „Gehe zu“ with F, a row's commands come first and „Neue
// Sammlung“ is found and opens the field for its name. On the synthetic catalog
// (docs/folia/frontend.md §4) as on the real one.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node befehle.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a console error or a step that does not come out as it should.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };

const open = async (path) => {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, serviceWorkers: "block", locale: "de-DE" });
  await context.addInitScript({ path: new URL("./on-page.js", import.meta.url).pathname });
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`pageerror: ${String(e).slice(0, 300)}`));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await page.waitForTimeout(800);
  return { context, page };
};

// The palette as it stands: its groups with their rows, the row chosen, what is told.
const palette = (page) => page.evaluate(() => {
  const d = document.querySelector("dialog.cmd-dialog[open]");
  if (!d) return null;
  const input = d.querySelector("input");
  return {
    focus: document.activeElement === input,
    groups: [...d.querySelectorAll(".cmd-group")].map((g) => ({
      heading: g.querySelector(".cmd-heading")?.textContent || "",
      rows: [...g.querySelectorAll(".cmd-opt")].map((o) => ({
        label: o.querySelector(".cmd-label")?.textContent || "",
        sub: o.querySelector(".cmd-sub")?.textContent || "",
        keys: (o.querySelector(".cmd-keys")?.innerText || "").replace(/\s+/g, ""),
        on: o.getAttribute("aria-selected") === "true",
      })),
    })),
    active: document.getElementById(input.getAttribute("aria-activedescendant") || "")?.querySelector(".cmd-label")?.textContent || "",
    finds: !!d.querySelector(".cmd-finds"),
    none: d.querySelector(".cmd-none")?.textContent || "",
    status: d.querySelector("[role=status]")?.textContent || "",
  };
});
const rows = (p) => (p ? p.groups.flatMap((g) => g.rows.map((r) => ({ ...r, heading: g.heading }))) : []);
const row = (p, label) => rows(p).find((r) => r.label === label);
const press = async (page, key, wait = 120) => { await page.keyboard.press(key); await page.waitForTimeout(wait); };
const openPalette = async (page) => { await press(page, "Control+k", 250); return palette(page); };
// Types into the field and waits for the rows to rest (the modules come a moment later).
const typeIn = async (page, text, until) => {
  await page.fill("dialog.cmd-dialog input", text);
  if (until) await page.waitForFunction(until, null, { timeout: 5000 }).catch(() => {});
  else await page.waitForTimeout(500);
  return palette(page);
};
const where = (page) => page.evaluate(() => ({ id: document.activeElement?.id || "", region: document.activeElement?.closest?.("[data-region]")?.dataset.region || "", cls: document.activeElement?.className || "" }));

// ---------- the catalog ----------
{
  const { context, page } = await open("/catalog");

  // The button in the search field, with its name and its keys.
  const button = await page.evaluate(() => {
    const b = document.querySelector(".topbar > .cmd-btn");
    const field = document.querySelector(".topbar > .search")?.getBoundingClientRect();
    const box = b?.getBoundingClientRect();
    return b && { shown: b.getClientRects().length > 0, text: b.innerText.replace(/\s+/g, ""), beside: !!field && box.left >= field.right && Math.abs(box.height - field.height) < 1, field: field && Math.round(field.width) };
  });
  // Beside the search, as tall as it, the search as long as it was before (owner, 2026-10-26).
  check(button?.shown && /^BefehleStrg\+K$/.test(button.text) && button.beside && button.field === 760, `the button beside the search field: ${JSON.stringify(button)}`);
  check(await page.evaluate(() => [...document.querySelectorAll(".search kbd")].some((k) => !k.closest(".cmd-btn") && k.textContent === "/" && k.getClientRects().length)), "the search field does not name /");

  // A click opens it, the field has the keyboard; nothing open: „Gehe zu“ and „Hilfe“.
  await page.click(".topbar > .cmd-btn");
  await page.waitForTimeout(250);
  let p = await palette(page);
  check(p?.focus, `the button opened no palette, or its field has no keyboard (${JSON.stringify(p)})`);
  check(p && p.groups.map((g) => g.heading).join("|") === "Gehe zu|Hilfe", `nothing open: the groups are ${JSON.stringify(p?.groups.map((g) => g.heading))}`);
  for (const [label, keys] of [["Start", "1"], ["Module", "2"], ["Merkliste", "4"], ["Suchfeld", "/"], ["Filter", "F"], ["Alle Tasten zeigen", "?"]]) {
    const r = row(p, label);
    check(r && r.keys === keys, `„${label}“ not offered with its keys ${keys}: ${JSON.stringify(r)}`);
  }
  check(p?.finds && !row(p, "Prüfung: Klausur"), "without a text: no hint what typing finds, or the filters shown already");
  check(p?.active === "Start", `the first row is not chosen: ${p?.active}`);
  await press(page, "Escape", 250);
  check(!(await palette(page)), "Esc: the palette stays open");
  check((await where(page)).cls.includes("cmd-btn"), `Esc: the keyboard is not back at the button (${JSON.stringify(await where(page))})`);

  // A module open beside the list: its commands first, and „Merken“ marks it.
  await page.locator(".rows a.row").nth(1).click();
  await page.waitForSelector(".detail h2", { timeout: 5000 }).catch(() => problems.push("a row opened no module"));
  const title = await page.evaluate(() => document.querySelector(".detail h2")?.textContent || "");
  p = await openPalette(page);
  const first = p?.groups[0];
  check(first?.heading === `Für „${title}“`, `the first group is not the module's: ${first?.heading} (${title})`);
  const own = (first?.rows || []).map((r) => `${r.label}:${r.keys}`).join("|");
  for (const want of ["Merken:M", "Sammlung und Notiz:Shift+M", "Vollbild:V", "Einplanen:P"]) check(own.includes(want), `the module's commands lack ${want}: ${own}`);
  check(p?.active === "Merken", `with a module open the first row is not „Merken“: ${p?.active}`);
  await press(page, "ArrowDown");
  await press(page, "ArrowDown");
  await press(page, "ArrowUp");
  check((await palette(page))?.active === "Sammlung und Notiz", `↓ ↓ ↑: ${(await palette(page))?.active}`);
  await press(page, "ArrowUp");
  await press(page, "Enter", 100);
  await page.waitForFunction(() => document.querySelector(".detail .mark-toggle.mark-switch")?.getAttribute("aria-pressed") === "true", null, { timeout: 3000 }).catch(() => {});
  const marked = await page.evaluate(() => ({ pressed: document.querySelector(".detail .mark-toggle.mark-switch")?.getAttribute("aria-pressed"), taught: document.querySelector(".cmd-taught.on")?.textContent || "" }));
  check(!(await palette(page)) && marked.pressed === "true", `Enter on „Merken“: not marked (${JSON.stringify(marked)})`);
  check(marked.taught === "Geht auch mit M", `after „Merken“: no „Geht auch mit M“ (${marked.taught})`);
  p = await openPalette(page);
  check(row(p, "Nicht mehr merken")?.keys === "M", `marked, the command is not „Nicht mehr merken“: ${JSON.stringify(rows(p).slice(0, 3))}`);

  // A filter, folded and by its group: Enter „mit“, again off; Shift + Enter „ohne“.
  p = await typeIn(page, "prufung klaus");
  check(p?.groups[0]?.heading === "Filter" && p?.active === "Prüfung: Klausur", `„prufung klaus“: ${JSON.stringify(p?.groups[0])}`);
  await press(page, "Enter", 700);
  check(new URL(page.url()).searchParams.get("exam") === "written", `Enter on „Prüfung: Klausur“: ${page.url()}`);
  p = await openPalette(page);
  p = await typeIn(page, "klausur");
  check(row(p, "Prüfung: Klausur")?.sub === "mit", `„Prüfung: Klausur“ does not say „mit“: ${JSON.stringify(row(p, "Prüfung: Klausur"))}`);
  await press(page, "Enter", 700);
  check(!new URL(page.url()).searchParams.get("exam"), `Enter on „Prüfung: Klausur“ again: still on (${page.url()})`);
  p = await openPalette(page);
  p = await typeIn(page, "englisch");
  check(p?.active === "Sprache: Englisch", `„englisch“: ${p?.active}`);
  await press(page, "Shift+Enter", 700);
  const english = await page.evaluate(() => [...document.querySelectorAll("#filters a.chip")].find((c) => c.textContent.trim() === "Englisch")?.dataset.state);
  check(english === "without", `Shift + Enter on „Sprache: Englisch“: the chip is ${english}`);

  // A module by its number: beside the list, and with Shift + Enter its page.
  const other = await page.evaluate(() => {
    const r = [...document.querySelectorAll(".rows a.row")].find((r) => r.getAttribute("aria-current") !== "true");
    return r && { id: r.dataset.id, title: r.querySelector("b")?.textContent.trim() || "" };
  });
  p = await openPalette(page);
  p = await typeIn(page, other?.id || "", () => !!document.querySelector("dialog.cmd-dialog[open] .cmd-group:last-child .cmd-sub"));
  const found = rows(p).find((r) => r.heading === "Module");
  check(found && found.sub === other?.id && found.label === other?.title, `module ${other?.id}: ${JSON.stringify(found)} (${JSON.stringify(rows(p).map((r) => r.heading + ":" + r.label))})`);
  const index = rows(p).findIndex((r) => r.heading === "Module");
  for (let i = 0; i < index; i++) await press(page, "ArrowDown", 40);
  await press(page, "Enter", 900);
  check(new URL(page.url()).searchParams.get("open") === other?.id && (await page.evaluate(() => document.querySelector(".detail h2")?.textContent)) === other?.title, `Enter on the module: ${page.url()}`);
  p = await openPalette(page);
  p = await typeIn(page, other?.id || "", () => !!document.querySelector("dialog.cmd-dialog[open] .cmd-group:last-child .cmd-sub"));
  const at = rows(p).findIndex((r) => r.heading === "Module");
  for (let i = 0; i < at; i++) await press(page, "ArrowDown", 40);
  await press(page, "Shift+Enter", 1200);
  check(new URL(page.url()).pathname === `/catalog/module/${other?.id}`, `Shift + Enter on the module: ${page.url()}`);

  // On the module's page: its sections, found by typing.
  p = await openPalette(page);
  check(p?.groups[0]?.heading === `Für „${other?.title}“` && row(p, "Link kopieren"), `the module's page: ${JSON.stringify(p?.groups[0])}`);
  p = await typeIn(page, "termine");
  check(rows(p).some((r) => r.heading === "Auf dieser Seite" && r.label === "Termine"), `„termine“ on the module's page: ${JSON.stringify(rows(p).map((r) => r.heading + ":" + r.label))}`);
  await press(page, "Escape", 200);
  await press(page, "Escape", 900);

  // The settings: found, and what holds now.
  p = await openPalette(page);
  p = await typeIn(page, "dunkel");
  check(p?.active === "Dunkel" && p?.groups[0]?.heading === "Einstellungen", `„dunkel“: ${JSON.stringify(p?.groups[0])}`);
  await press(page, "Enter", 300);
  check((await page.evaluate(() => document.documentElement.dataset.theme)) === "dark", "„Dunkel“: not dark");
  p = await openPalette(page);
  p = await typeIn(page, "dunkel");
  check(row(p, "Dunkel")?.sub === "jetzt", `dark: „Dunkel“ does not say „jetzt“ (${JSON.stringify(row(p, "Dunkel"))})`);
  p = await typeIn(page, "system");
  await press(page, "Enter", 300);
  check(!(await page.evaluate(() => document.documentElement.dataset.theme)), "„wie das System“: a theme of its own stays");

  // „Filter“ leads into the filters, „Suchfeld“ into the search.
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
  await page.waitForTimeout(600);
  p = await openPalette(page);
  p = await typeIn(page, "filter");
  check(p?.active === "Filter", `„filter“: the first row is ${p?.active}`);
  await press(page, "Enter", 400);
  check((await where(page)).region === "side", `„Filter“: the keyboard is not in the filters (${JSON.stringify(await where(page))})`);
  p = await openPalette(page);
  p = await typeIn(page, "suchfeld");
  await press(page, "Enter", 300);
  check((await where(page)).id === "topsearch", `„Suchfeld“: the keyboard is not in the search (${JSON.stringify(await where(page))})`);

  // Nothing found says so; Strg + K closes it again.
  p = await openPalette(page);
  p = await typeIn(page, "xqzjwv", () => /^Nichts/.test(document.querySelector("dialog.cmd-dialog[open] .cmd-none")?.textContent || ""));
  check(p && rows(p).length === 0 && /^Nichts gefunden/.test(p.none) && p.status === "0 Treffer", `„xqzjwv“: ${JSON.stringify(p)}`);
  await press(page, "Control+k", 250);
  check(!(await palette(page)), "Strg + K in the palette: it stays open");

  // The single keys off: the commands name none, Strg + K still opens them.
  p = await openPalette(page);
  p = await typeIn(page, "einzelne tasten");
  await press(page, "Enter", 300);
  check((await page.evaluate(() => document.documentElement.dataset.letters)) === "off", "„Einzelne Tasten ausschalten“: still on");
  p = await openPalette(page);
  check(p && row(p, "Filter")?.keys === "" && row(p, "Suchfeld")?.keys === "" && row(p, "Module")?.keys === "2", `the single keys off: ${JSON.stringify(rows(p).filter((r) => r.heading === "Gehe zu"))}`);
  p = await typeIn(page, "einzelne tasten");
  check(p?.active === "Einzelne Tasten (F, ?, M …) einschalten", `the single keys off: the setting is ${p?.active}`);
  await press(page, "Enter", 300);
  check(!(await page.evaluate(() => document.documentElement.dataset.letters)), "„Einzelne Tasten einschalten“: still off");
  await context.close();
}

// ---------- the Merkliste ----------
// Its sidebar under „Gehe zu" with F, „Neue Sammlung" found by typing and run, a row's commands
// while the keyboard is on it, and Entf named at „Entfernen" of the selection.
{
  const { context, page } = await open("/catalog");
  await page.locator(".rows a.row >> visible=true").first().focus();
  for (let i = 0; i < 2; i++) { await press(page, "m", 120); await press(page, "ArrowDown", 80); }
  await page.click('.rail .nav[data-area="bookmarks"] >> visible=true');
  await page.waitForFunction(() => location.pathname === "/bookmarks" && document.querySelectorAll("#rows a.row").length >= 2, null, { timeout: 8000 }).catch(() => problems.push("the Merkliste: no rows"));
  await page.waitForTimeout(400);
  await page.locator("#rows a.row >> visible=true").first().focus();
  const title = await page.evaluate(() => document.activeElement?.closest(".row-wrap")?.dataset.commands || "");
  let p = await openPalette(page);
  check(row(p, "Sammlungen und Filter")?.keys === "F", `the Merkliste: its sidebar is not offered with F (${JSON.stringify(rows(p).filter((r) => r.heading === "Gehe zu"))})`);
  check(p?.groups[0]?.heading === title && row(p, "Nicht mehr merken")?.keys === "M", `the Merkliste: the row's commands do not come first (${JSON.stringify(p?.groups[0])}, ${title})`);
  p = await typeIn(page, "neue samm");
  check(p?.active === "Neue Sammlung", `the Merkliste: „neue samm“ finds ${p?.active}`);
  await press(page, "Enter", 400);
  check(await page.evaluate(() => !!document.querySelector(".coll-nav input")), "the Merkliste: „Neue Sammlung“ opened no field for the name");
  await press(page, "Escape", 200);
  await context.close();
}

await browser.close();
console.log(problems.length ? JSON.stringify({ problems }, null, 2) : "befehle: ok");
process.exit(problems.length ? 1 : 0);
