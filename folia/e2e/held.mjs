// Checks what every tab of the navigation holds besides its page (owner, 2026-11-18: „alle Einträge
// in der navbar eine rclick und long press action/context menu. Also am pc immer context menu und
// beim handy kann beides sein je nachdem was besser passt"; folia/crates/shell/src/chrome.rs
// `TabMenu`, enhance.js „a tab held"):
// - on a wide screen every tab of the rail, right clicked, has a menu beside it and the browser's
//   own menu never comes: „Start" the pages about Betula, „Module" all modules, those of „Mein
//   Studiengang" and the marked ones, „Studium" the views of „Mein Studium" and all programs, the
//   Merkliste all of it and the catalog's marked modules (without a collection too), the
//   Stundenplan „Woche", „Termine", „Prüfungen" and „Bearbeiten"; a way leads there, Esc closes;
// - on a phone a finger held on a tab of the bar: the same menus above the bar, the catalog's list
//   with its filters open instead of a menu; the finger's release goes nowhere.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node held.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a console error, a menu that is not as it should be, or a way that does not lead there.
import { chromium } from "playwright-core";
import { fileURLToPath } from "node:url";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
// Informatik B.Sc. as „Mein Studiengang", two modules marked, no collection.
const MINE = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n";
const now = Math.floor(Date.now() / 1000);
const MARKED = `12104\t${now}\n12107\t${now - 60}\n`;
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };

const open = async (options) => {
  const context = await browser.newContext(options);
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(([mine, marked]) => {
    if (sessionStorage.getItem("held.seeded")) return;
    sessionStorage.setItem("held.seeded", "1");
    localStorage.setItem("betula.myprogram.v1", mine);
    localStorage.setItem("betula.bookmarks.v1", marked);
  }, [MINE, MARKED]);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  return page;
};
const ready = (page, path) => page.goto(base + path, { waitUntil: "domcontentloaded" }).then(() => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 })).then(() => page.waitForTimeout(600));
const ways = (page) => page.evaluate(() => [...document.querySelectorAll(".tab-menu .tab-menu-way")].map((a) => `${a.innerText.replace(/\s+/g, " ").trim()} ${a.getAttribute("href")}`));
const head = (page) => page.evaluate(() => document.querySelector(".tab-menu .tab-menu-head")?.textContent ?? null);
const closed = (page, what) => page.waitForFunction(() => !document.querySelector(".tab-menu"), null, { timeout: 2000 }).catch(() => problems.push(`${what}: the menu did not close`));

const ABOUT = ["Überblick /about/overview", "Module finden /about/finding", "Studium planen /about/planning", "Deine Daten /about/data", "Technik /about/technology", "Häufige Fragen /about/questions"];
const CATALOG = ["Alle Module /catalog", "Module meines Studiengangs /catalog?program=bachelor-informatik-2008", "Gemerkte Module 2 /catalog?marked=only"];
const STUDY = ["Regelstudienplan /study?view=plan", "Wahlpflicht /study?view=areas", "Ordnungen & Dokumente /study?view=documents", "Alle Studiengänge /programs"];
const BOOKMARKS = ["Alle gemerkten 2 /bookmarks", "Im Katalog zeigen /catalog?marked=only"];
const STUDYPLAN = ["Woche /studyplan", "Termine /studyplan?view=dates", "Prüfungen /studyplan?view=exams", "Bearbeiten /studyplan?edit=1"];
const same = (got, want, what) => check(JSON.stringify(got) === JSON.stringify(want), `${what}: ${JSON.stringify(got)}, not ${JSON.stringify(want)}`);

// ---------- a wide screen: the rail, right clicked ----------
{
  const page = await open({ viewport: { width: 1280, height: 860 } });
  await ready(page, "/catalog");
  // Whether the browser's own menu would come: the right click's default left alone.
  await page.evaluate(() => { window.__nativeMenus = 0; document.addEventListener("contextmenu", (e) => { setTimeout(() => { if (!e.defaultPrevented) window.__nativeMenus++; }); }); });
  const rightClick = async (area) => {
    await page.click(`.rail a.nav[data-area="${area}"]`, { button: "right" });
    await page.waitForSelector(".tab-menu .tab-menu-way", { timeout: 3000 }).catch(() => problems.push(`the rail: „${area}“ right clicked has no menu`));
    // Where it comes to stand, once it has come.
    await page.waitForFunction(() => !document.querySelector(".tab-menu")?.getAnimations().some((a) => a.playState === "running"), null, { timeout: 2000 }).catch(() => {});
  };
  for (const [area, want, name] of [["home", ABOUT, "Über Betula"], ["catalog", CATALOG, null], ["programs", STUDY, "Informatik B.Sc. · PO 2008"], ["bookmarks", BOOKMARKS, null], ["studyplan", STUDYPLAN, null]]) {
    await rightClick(area);
    same(await ways(page), want, `the rail: „${area}“'s menu`);
    check((await head(page)) === name, `the rail: „${area}“'s menu is headed ${JSON.stringify(await head(page))}`);
    // Beside the rail, at its tab, whole on the screen.
    const at = await page.evaluate((area) => { const menu = document.querySelector(".tab-menu")?.getBoundingClientRect(); if (!menu) return null; const tab = document.querySelector(`.rail a.nav[data-area="${area}"]`).getBoundingClientRect(); return { left: menu.left - tab.right, top: Math.round(menu.top - tab.top), bottom: menu.bottom <= innerHeight }; }, area);
    check(at && at.left >= 0 && at.left <= 24 && at.top === 0 && at.bottom, `the rail: „${area}“'s menu does not stand beside its tab: ${JSON.stringify(at)}`);
    check(await page.evaluate(() => location.pathname === "/catalog"), `the rail: „${area}“ right clicked went somewhere`);
    await page.keyboard.press("Escape");
    await closed(page, `the rail: „${area}“, Esc`);
  }
  await page.waitForTimeout(100);
  check((await page.evaluate(() => window.__nativeMenus)) === 0, "the rail: a right click on a tab left the browser its own menu");
  // A way leads there: „Prüfungen" of the Stundenplan, „Technik" of the start page, the catalog's
  // marked modules.
  await rightClick("studyplan");
  await page.click('.tab-menu a.tab-menu-way:has-text("Prüfungen")');
  await page.waitForFunction(() => location.pathname === "/studyplan" && location.search === "?view=exams" && !document.querySelector(".tab-menu"), null, { timeout: 8000 }).catch(() => problems.push("the rail: „Prüfungen“ did not lead to the exams"));
  await rightClick("studyplan");
  await page.click('.tab-menu a.tab-menu-way:has-text("Bearbeiten")');
  await page.waitForFunction(() => location.pathname === "/studyplan" && new URLSearchParams(location.search).get("edit") === "1", null, { timeout: 8000 }).catch(() => problems.push("the rail: „Bearbeiten“ did not edit the plan"));
  await rightClick("home");
  await page.click('.tab-menu a.tab-menu-way:has-text("Technik")');
  await page.waitForFunction(() => location.pathname === "/about/technology", null, { timeout: 8000 }).catch(() => problems.push("the rail: „Technik“ did not lead to its page"));
  await ready(page, "/bookmarks");
  await rightClick("catalog");
  await page.click('.tab-menu a.tab-menu-way:has-text("Gemerkte Module")');
  await page.waitForFunction(() => location.pathname === "/catalog" && location.search === "?marked=only" && allOnPage("#rows a.row").length === 2, null, { timeout: 8000 }).catch(() => problems.push("the rail: „Gemerkte Module“ did not list the two marked modules"));
  // The menu key on a tab with the focus, as a right click.
  await page.focus('.rail a.nav[data-area="studyplan"]');
  await page.keyboard.press("Shift+F10");
  await page.waitForSelector(".tab-menu .tab-menu-way", { timeout: 3000 }).then(() => check(true)).catch(() => problems.push("the rail: Shift+F10 on a tab opened no menu"));
  await page.keyboard.press("Escape");
  await page.context().close();
}

// ---------- a phone: the bar, held ----------
{
  const page = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
  const cdp = await page.context().newCDPSession(page);
  await ready(page, "/bookmarks");
  const hold = async (area, ms = 650) => {
    const at = await page.evaluate((area) => { const box = document.querySelector(`.bottomnav > .nav[data-area="${area}"]`).getBoundingClientRect(); return { x: box.left + box.width / 2, y: box.top + box.height / 2 }; }, area);
    await cdp.send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x: at.x, y: at.y }] });
    await page.waitForTimeout(ms);
    const got = await ways(page);
    await cdp.send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
    await page.waitForTimeout(500);
    return got;
  };
  for (const [area, want] of [["home", ABOUT], ["programs", STUDY], ["bookmarks", BOOKMARKS], ["studyplan", STUDYPLAN]]) {
    same(await hold(area), want, `the bar: „${area}“ held`);
    const at = await page.evaluate(() => { const menu = document.querySelector(".tab-menu")?.getBoundingClientRect(), bar = document.querySelector(".bottomnav")?.getBoundingClientRect(); return menu && { above: menu.bottom <= bar.top, left: menu.left >= 0, right: menu.right <= innerWidth }; });
    check(at && at.above && at.left && at.right, `the bar: „${area}“'s menu does not stand above the bar, on the screen: ${JSON.stringify(at)}`);
    check(await page.evaluate(() => location.pathname === "/bookmarks" && Boolean(document.querySelector(".tab-menu"))), `the bar: „${area}“ held, its release went somewhere or the menu went`);
    await page.touchscreen.tap(200, 300);
    await closed(page, `the bar: „${area}“, a tap beside it`);
    check(await page.evaluate(() => location.pathname === "/bookmarks"), `the bar: „${area}“, the tap beside the menu went somewhere`);
  }
  // A way taken with a finger.
  await hold("studyplan");
  await page.tap('.tab-menu a.tab-menu-way:has-text("Termine")');
  await page.waitForFunction(() => location.pathname === "/studyplan" && location.search === "?view=dates" && !document.querySelector(".tab-menu"), null, { timeout: 8000 }).catch(() => problems.push("the bar: „Termine“ did not lead to the dates"));
  // The catalog held: its list with the filters, no menu.
  same(await hold("catalog"), [], "the bar: the catalog held has a menu");
  await page.waitForFunction(() => location.pathname === "/catalog" && document.getElementById("filters")?.classList.contains("open"), null, { timeout: 8000 }).catch(() => problems.push("the bar: the catalog held did not open with its filters"));
  await page.context().close();
}

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
