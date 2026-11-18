// The sheet of a choice in focus on a phone, and the way to a choice from the week (owner, 2026-10-26;
// docs/folia/frontend.md, „Offen and the choice in focus"), on canary's data of 2026W with Informatik
// B.Sc.'s first semester planned (Programmierpraktikum 148370 in four groups, Mo, Mi, Do 11:30 and
// Fr 13:45).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node choice-sheet.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// - A wide screen: a click on a hatched group of the week opens its card and ticks nothing; the card's
//   large action is „Diese Gruppe anhaken" and has the keyboard; its „Vergleichen ›" takes the choice
//   into focus inside the edit mode (`edit=1&wahl=e148370&vergleich=1`, „noch keine angehakt"), where a
//   click on a group ticks it („1 von 4 angehakt", one `c` line).
// - A phone: the amber line „3 Wahlen offen" leads into the mode with its rows open; the row of the
//   Praktikum opens the sheet („noch keine angehakt"), a tap on a tile ticks it; nothing under the head
//   says what is hidden.
// - A finger moves the sheet: up from its head, the tiles on their way to „Vergleichen" while it
//   touches (`.morph`, `--p` between half up and all up, a card for each group); let go all up, the
//   address says `vergleich=1`; down from all up, back to the tiles; „Vergleichen ›" and ‹ take the
//   same way; a quick move down from half up closes the choice, the mode stays.
// History: 2026-11-17, the edit mode and the card. A hatched group no longer takes its choice into focus
// with that group ticked (`data-pre` went: a click on a Termin shows that Termin); the focus is reached
// from the card's „Vergleichen ›" and from „Offen", and the group is ticked there. „Plan · Alle Termine"
// and the line of the overlaps as a `details` went from the phone's head.
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
const FS1 = ["12104", "12107", "12102", "11112"].map((id) => `m\t2026W\t${id}\t1790000000\t`).join("\n") + "\n";
const INFORMATIK = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n";

const open = async (options) => {
  const context = await browser.newContext(options);
  await context.addInitScript(ON_PAGE);
  // The sheet of the page in front: not its copy in the picture of the page the app holds in front
  // of the next one for a moment (`.pending-page.held`, folia/crates/shell/src/pending.rs).
  await context.addInitScript(() => { window.liveSheet = () => allOnPage("section.sp-pick").find((el) => !el.closest(".pending-page")) ?? null; });
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  await context.addInitScript(([plan, mine, planText, mineText]) => {
    if (localStorage.getItem(plan) === null) {
      localStorage.setItem(plan, planText);
      localStorage.setItem(mine, mineText);
    }
  }, [PLAN, MINE, FS1, INFORMATIK]);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + "/studyplan", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await page.evaluate(() => { window.__marker = 1; });
  return { page, context };
};
const until = async (page, name, fn, arg) => {
  try {
    await page.waitForFunction(fn, arg, { timeout: 10000 });
  } catch {
    problems.push(`${name}: did not happen (${page.url()})`);
  }
};
const ticks = (page) => page.evaluate((key) => (localStorage.getItem(key) ?? "").split("\n").filter((line) => line.startsWith("c\t2026W\t148370-")).length, PLAN);

// ---------- a wide screen: the card of a hatched group, its „Vergleichen ›" ----------
{
  const { page, context } = await open({ viewport: { width: 1440, height: 900 } });
  await until(page, "the week", () => document.querySelectorAll(".sp-week .slot").length > 0);
  const group = page.locator('.sp-week a.slot.alt[href$="termin=148370-eb017"]', { hasText: "Prak PP" });
  check(await group.count() === 1, "wide: no hatched group of the Praktikum to click");
  await group.click().catch(() => problems.push("wide: the hatched group could not be clicked"));
  await until(page, "wide: its card", () => location.search === "?termin=148370-eb017" && document.activeElement?.matches("dialog[open] .tm-big[data-action='tick']") && document.activeElement.textContent === "Diese Gruppe anhaken");
  check(await ticks(page) === 0, `wide: the click ticked a group: ${await ticks(page)}`);
  check(await page.evaluate(() => !document.querySelector(".sp-week [data-pre]")), "wide: a slot still preselects");
  await page.click("dialog[open] a.go2[data-action='compare']").catch(() => problems.push("wide: the card has no „Vergleichen ›“"));
  await until(page, "wide: the choice in focus", () => location.search === "?edit=1&wahl=e148370&vergleich=1" && document.querySelector(".sp-focusbar") && /noch keine angehakt/.test(document.querySelector(".sp-focusbar .what small")?.textContent ?? ""));
  await page.click('.sp-week button.slot.pick[title*="Mo 11:30"]').catch(() => problems.push("wide: no group of Monday in focus"));
  await until(page, "wide: the group clicked ticked", () => /1 von 4/.test(document.querySelector(".sp-focusbar .what small")?.textContent ?? ""));
  check(await ticks(page) === 1, `wide: not one tick kept: ${await ticks(page)}`);
  if (shots) await page.screenshot({ path: `${shots}/choice-wide-focus.png` });
  check(await page.evaluate(() => window.__marker === 1), "wide: the page loaded again");
  await context.close();
}

// ---------- a phone: the sheet ----------
{
  const { page, context } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
  const cdp = await context.newCDPSession(page);
  const touch = (type, x, y) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x, y }] });
  // A finger from (x, y0) to (x, y1) in `n` moves, 16 ms apart; `halfway` while it moves, and with
  // `rest` it rests before it lets go, so that the sheet goes to the stop it is nearest (a finger
  // let go moving goes on as far as it went: enhance.js `goalOf`).
  const swipe = async (x, y0, y1, n, { halfway, rest } = {}) => {
    await touch("touchStart", x, y0);
    for (let i = 1; i <= n; i++) {
      await touch("touchMove", x, y0 + ((y1 - y0) * i) / n);
      await page.waitForTimeout(16);
      if (halfway && i === Math.floor(n / 2)) await halfway();
    }
    if (rest) await page.waitForTimeout(150);
    await touch("touchEnd");
  };
  const sheet = () => page.evaluate(() => {
    const el = liveSheet();
    const shown = (node) => node.getClientRects().length > 0 && getComputedStyle(node).visibility !== "hidden";
    return el && {
      full: el.classList.contains("full"),
      morph: el.classList.contains("morph"),
      p: Number(el.style.getPropertyValue("--p") || -1),
      top: Math.round(el.getBoundingClientRect().top),
      cards: el.querySelectorAll(".sp-morph > .mc").length,
      tiles: [...el.querySelectorAll(".sp-pick-body > .grp .gs.pick")].filter(shown).map((tile) => tile.getAttribute("aria-checked")),
      lines: [...el.querySelectorAll(".sp-pick-body > .sp-pick-list .sp-cmp-row")].filter(shown).length,
      say: el.querySelector(".grp-h .r")?.textContent ?? "",
    };
  });
  const head = () => page.evaluate(() => ({ y: liveSheet()?.getBoundingClientRect().top ?? 600 }));
  await until(page, "phone: the week", () => document.querySelectorAll(".sp-carousel .slot").length > 0);
  check(await page.evaluate(() => !document.querySelector(".sp-all")), "phone: „Plan · Alle Termine\" still there");

  // The amber line: into the mode, its rows open; the Praktikum's row: its sheet; a tap on a tile.
  await page.tap("a.sp-open-line").catch(() => problems.push("phone: no amber line to tap"));
  await until(page, "phone: the mode with the rows open", () => location.search === "?edit=1" && document.querySelectorAll("details.sp-open-line[open] .sp-open-row").length === 3);
  await page.tap('details.sp-open-line .sp-open-row[href$="wahl=e148370"]').catch(() => problems.push("phone: no row of the Praktikum"));
  await until(page, "phone: the sheet", () => location.search === "?edit=1&wahl=e148370" && liveSheet() && /noch keine angehakt/.test(liveSheet().querySelector(".grp-h .r")?.textContent ?? ""));
  await page.tap("section.sp-pick .sp-pick-body > .grp .gs.pick >> nth=0").catch(() => problems.push("phone: no tile to tap"));
  await until(page, "phone: the tile tapped ticked", () => liveSheet()?.querySelector(".grp-h .r")?.textContent.includes("1 von 4"));
  let s = await sheet();
  check(s?.tiles.filter((on) => on === "true").length === 1 && !s.full, `phone: not one tile ticked, half up: ${JSON.stringify(s)}`);
  check(await page.evaluate(() => !/ausgeblendet/.test(document.querySelector(".sp-sum-row, .sp-overlaps")?.textContent ?? "")), "phone: the line under the head says what is hidden");
  if (shots) await page.screenshot({ path: `${shots}/choice-phone-ticked.png` });

  // Up from its head: the tiles on their way while the finger moves; let go past the middle, all up.
  let box = await head();
  await swipe(195, box.y + 30, box.y + 30 - box.y * 0.7, 16, {
    rest: true,
    halfway: async () => {
      s = await sheet();
      check(s?.morph && s.p > 0.15 && s.p < 0.95 && s.cards === 4, `phone: the tiles are not on their way while the finger holds the sheet: ${JSON.stringify(s)}`);
      if (shots) await page.screenshot({ path: `${shots}/choice-phone-on-the-way.png` });
    },
  });
  await until(page, "phone: all up", () => location.search.includes("vergleich=1") && liveSheet()?.matches(".full:not(.morph)"));
  s = await sheet();
  check(s?.full && !s.morph && s.lines === 4 && s.top < 60, `phone: not „Vergleichen" all up: ${JSON.stringify(s)}`);
  if (shots) await page.screenshot({ path: `${shots}/choice-phone-full.png` });
  await page.waitForTimeout(500);

  // Down from its head, let go nearer half up than all up: back to the tiles.
  box = await head();
  await swipe(195, box.y + 30, box.y + 330, 16, { rest: true });
  await until(page, "phone: half up again", () => !location.search.includes("vergleich=1") && liveSheet()?.matches(":not(.full):not(.morph)"));
  s = await sheet();
  check(s && !s.full && s.tiles.length === 4 && s.lines === 0, `phone: not the tiles half up: ${JSON.stringify(s)}`);

  // „Vergleichen ›" and ‹ the same way.
  await page.tap("section.sp-pick .grp .go2 >> visible=true");
  await until(page, "phone: „Vergleichen ›\"", () => location.search.includes("vergleich=1") && liveSheet()?.matches(".full:not(.morph)"));
  await page.waitForTimeout(500);
  await page.tap("section.sp-pick .sp-pick-head .to-tiles >> visible=true");
  await until(page, "phone: ‹", () => !location.search.includes("vergleich=1") && liveSheet()?.matches(":not(.full):not(.morph)"));
  await page.waitForTimeout(500);

  // A quick move down from half up: away, the choice closed, its tick kept.
  box = await head();
  await swipe(195, box.y + 30, box.y + 250, 5);
  await until(page, "phone: the sheet away, the mode stays", () => location.search === "?edit=1" && !liveSheet());
  check(await ticks(page) === 1, `phone: the tick not kept: ${await ticks(page)}`);
  check(await page.evaluate(() => window.__marker === 1), "phone: the page loaded again");
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
