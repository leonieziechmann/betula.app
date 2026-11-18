// Checks the tab of the area one is in, tapped again (folia/crates/shell/src/tabs.rs `TabAgain`,
// chrome.rs `NavItems`; owner, 2026-11-18: „wenn man schon auf einen tab ist und dann nochmal darauf
// klickt, dass sich dann die standard Ansicht davon öffnet. Also bei mein Studium zum Beispiel die
// mein Studium page. Beim Modulkatalog für Standard Filter von Studiengang, bei der landing Page etc.
// Und ganz noch oben gescrollt"):
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node again.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Behind the gate of closed testing (canary): SMOKE_COOKIES=<curl's cookie jar of /access>.
// A wide screen, „Mein Studiengang" Informatik B.Sc.:
// 1 the catalog filtered and far down: „Module" leads to the catalog of „Mein Studiengang", at its
//   top; the tab's link says so; from another area the tab still leads where the catalog was left.
// 2 there, far down (`page=` in the address): „Module" glides to the top, `page` goes, no step of
//   the history; Ctrl+Alt+2, the tab's keys, the same.
// 3 a module's own page, reached from a row far down: „Module" leads to the list at its top, not to
//   the module's row as „Zurück" would.
// 4 a program's page: „Studium" leads to „Mein Studium"; another semester in focus: the current one
//   again; the workbench open: it ends, the address stays `/study` (also where the Regelstudienplan
//   opened it, whose way back it is otherwise).
// 5 the Merkliste ordered: `/bookmarks`; the Stundenplan in another view: `/studyplan`.
// 6 the start page far down: „Start" glides it to its top.
// A phone (its tabs are stacks, folia/crates/shell/src/nav.rs):
// 7 the catalog filtered, a module's own page over it: „Module" makes the tab's stack the catalog of
//   „Mein Studiengang", Start under it, the window at its top; Back leads to Start.
// 8 there, far down: „Module" glides the window up, the history stays as it is.
// 9 „Studium" from a program's page: „Mein Studium" right above Start; the row turned to a semester:
//   back to the overview.
// 10 Start far down: „Start" takes it to its top.
// Fails on a page load after takeover, a console error, a new entry of a phone's history without a
// tap, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
let during = "";
const PLAN = "betula.studyplan.v1";
const MINE = "betula.myprogram.v1";
const THIRD = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n";
const W = ["12104", "12107", "12102", "11112"];
const S = ["12101", "11903", "11113", "12111"];
const PASSED = [...W.map((id) => `m\t2025W\t${id}\t1\t\tplan`), ...S.map((id) => `m\t2026S\t${id}\t1\t\tplan`), ...W.map((id) => `d\t2025W\t${id}`), ...S.map((id) => `d\t2026S\t${id}`)].join("\n") + "\n";
const HOME = "/catalog?program=bachelor-informatik-2008";
const PAGE = "#content > .page-stage.front > .work.flowing";

/// The cookie of closed testing from a curl cookie jar, where the site has the gate.
const gate = process.env.SMOKE_COOKIES
  ? (await import("node:fs")).readFileSync(process.env.SMOKE_COOKIES, "utf8").split("\n").map((l) => l.replace(/^#HttpOnly_/, "").split("\t")).filter((f) => f.length >= 7 && f[5] === "betula_access").map((f) => ({ name: f[5], value: f[6], domain: f[0].replace(/^\./, ""), path: "/", secure: true, httpOnly: true }))
  : [];

const takeover = (page) => page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
const open = async (options) => {
  const context = await browser.newContext({ locale: "de-DE", ...options });
  if (gate.length) await context.addCookies(gate);
  // A phone's every new entry comes with a tap (Chrome's Back skips one a page added without).
  await context.addInitScript(() => {
    window.__untapped = [];
    const push = history.pushState.bind(history);
    history.pushState = (state, title, url) => {
      if (!navigator.userActivation.isActive) window.__untapped.push(String(url));
      return push(state, title, url);
    };
  });
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`console (${during}): ` + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push(`pageerror (${during}): ` + String(e).slice(0, 300)));
  await page.goto(base + "/", { waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate((entries) => { for (const [key, text] of entries) localStorage.setItem(key, text); }, [[MINE, THIRD], [PLAN, PASSED]]);
  await page.reload({ waitUntil: "domcontentloaded" });
  await takeover(page);
  await page.evaluate(() => { window.__marker = 1; });
  const taps = Boolean(options.hasTouch);
  const step = async (name, action, until, arg) => {
    during = name;
    await action();
    try {
      await page.waitForFunction(until, arg, { timeout: 8000 });
      await page.waitForFunction(() => !document.querySelector(".pending-page.held") && document.getElementById("content")?.getAttribute("aria-busy") !== "true", null, { timeout: 2000 });
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
    const untapped = await page.evaluate(() => window.__untapped.splice(0));
    if (taps && untapped.length) problems.push(`${name}: a new entry without a tap (${untapped.join(", ")})`);
  };
  return { context, page, step };
};
// Where the page is: the address, how far its scroll area (a phone: the window) is down, the history.
const where = (page) => page.evaluate((selector) => ({
  here: location.pathname + location.search,
  down: Math.round(window.matchMedia("(max-width: 900px)").matches ? scrollY : document.querySelector(selector)?.scrollTop ?? -1),
  entries: navigation.entries().map((entry) => { const url = new URL(entry.url); return url.pathname + url.search; }),
  at: navigation.currentEntry.index,
}), PAGE);
// The page in front at its top, once it stands still.
const atTop = (page) => page.waitForFunction((selector) => {
  const down = window.matchMedia("(max-width: 900px)").matches ? scrollY : document.querySelector(selector)?.scrollTop;
  return down === 0;
}, PAGE, { timeout: 4000 }).then(() => true, () => false);
const part = async (name, run) => {
  try { await run(); } catch (error) { problems.push(`${name} (${during}): ${String(error).split("\n")[0]}`); }
};

// ---------- a wide screen ----------
await part("wide", async () => {
  const { context, page, step } = await open({ viewport: { width: 1440, height: 900 } });
  const tab = (area) => page.click(`.rail .nav[data-area="${area}"]`);
  const down = (y) => page.$eval(PAGE, (el, y) => { el.scrollTop = y; }, y).then(() => page.waitForTimeout(500));
  // 1
  await step("the catalog filtered", () => page.click('.rail .nav[data-area="catalog"]').then(() => page.waitForTimeout(300)).then(() => page.evaluate(() => {
    const a = Object.assign(document.createElement("a"), { href: "/catalog?turnus=winter", textContent: "winter", id: "x-winter" });
    document.querySelector("#content > .page-stage.front").append(a);
    a.click();
    a.remove();
  })), () => location.search === "?turnus=winter" && document.querySelectorAll("#content > .page-stage.front .rows a.row").length > 5);
  await down(3000);
  check((await page.getAttribute('.rail .nav[data-area="catalog"]', "href")) === HOME, `the catalog's tab in the catalog does not lead to the catalog of „Mein Studiengang“: ${await page.getAttribute('.rail .nav[data-area="catalog"]', "href")}`);
  await step("Module again: the catalog of „Mein Studiengang“", () => tab("catalog"), (home) => location.pathname + location.search === home && document.querySelector("#content > .page-stage.front .rows a.row"), HOME);
  check(await atTop(page), `Module again: the list is not at its top (${JSON.stringify(await where(page))})`);
  await step("Studium", () => tab("programs"), () => location.pathname === "/study");
  check((await page.getAttribute('.rail .nav[data-area="catalog"]', "href")) === HOME, "from another area the catalog's tab does not lead where the catalog was left");
  await step("Module", () => tab("catalog"), (home) => location.pathname + location.search === home, HOME);
  // 2
  await down(4000);
  await page.waitForFunction(() => /[?&]page=\d/.test(location.search), null, { timeout: 4000 }).catch(() => problems.push("far down the list the address names no page"));
  const before = await where(page);
  await step("Module again where the app is", () => tab("catalog"), (home) => location.pathname + location.search === home, HOME);
  const mid = await where(page);
  check(mid.down > 0, `Module again where the app is: the list is at its top at once, it does not glide (${mid.down})`);
  check(await atTop(page), `Module again where the app is: the list does not come to its top (${JSON.stringify(await where(page))})`);
  const after = await where(page);
  check(after.at === before.at && after.entries.length === before.entries.length, `Module again where the app is: a step of the history (${before.at}/${before.entries.length} → ${after.at}/${after.entries.length})`);
  await down(3000);
  await page.keyboard.down("Control");
  await page.keyboard.down("Alt");
  await page.keyboard.press("2");
  await page.keyboard.up("Alt");
  await page.keyboard.up("Control");
  check(await atTop(page), "Ctrl+Alt+2 on the catalog: the list does not come to its top");
  // 3
  await down(2500);
  const row = await page.evaluate(() => [...document.querySelectorAll("#content > .page-stage.front .rows a.row[data-id]")].find((a) => a.getBoundingClientRect().top > 400)?.dataset.id);
  await step("a module's own page far down", () => page.evaluate((id) => {
    const a = Object.assign(document.createElement("a"), { href: `/catalog/module/${id}`, textContent: id });
    document.querySelector("#content > .page-stage.front").append(a);
    a.click();
  }, row), (id) => location.pathname === `/catalog/module/${id}` && document.querySelector("#content > .page-stage.front h1, #content > .page-stage.front h2"), row);
  await step("Module again from the module's page", () => tab("catalog"), (home) => location.pathname + location.search === home && document.querySelector("#content > .page-stage.front .rows a.row"), HOME);
  await page.waitForTimeout(400);
  check(await atTop(page), `Module again from the module's page: the list is not at its top, the module's row shown again? (${JSON.stringify(await where(page))})`);
  // 4
  await step("a program's page", () => page.evaluate(() => {
    const a = Object.assign(document.createElement("a"), { href: "/programs/bachelor-informatik-2008/plan", textContent: "program" });
    document.querySelector("#content > .page-stage.front").append(a);
    a.click();
  }), () => location.pathname === "/programs/bachelor-informatik-2008/plan");
  await step("Studium again: „Mein Studium“", () => tab("programs"), () => location.pathname === "/study" && location.search === "" && document.querySelector(".st-card-sem h2"));
  const now = await page.textContent(".st-card-sem h2");
  await step("another semester", () => page.click(".st-card-sem .st-sem-pick").then(() => page.click(".st-pop.root [role=menuitem] >> nth=1")), (now) => document.querySelector(".st-card-sem h2")?.textContent !== now, now);
  await step("Studium again: the current semester", () => tab("programs"), (now) => document.querySelector(".st-card-sem h2")?.textContent === now, now);
  if (await page.$(".st-card-sem .st-empty-add")) {
    await step("the workbench", () => page.click(".st-card-sem .st-empty-add"), () => document.querySelector(".st-wb-half"));
    await step("Studium again: the workbench ends", () => tab("programs"), () => !document.querySelector(".st-wb-half") && location.pathname === "/study" && location.search === "");
    await step("the Regelstudienplan", () => page.click('#sidebar a:has-text("Regelstudienplan")'), () => location.search === "?view=plan");
    await step("its „Wählen“: the workbench", () => page.click('button.st-addb[title^="Wählen"] >> nth=0'), () => document.querySelector(".st-wb-half") && location.search === "");
    await step("Studium again: the workbench ends, not back to the Regelstudienplan", () => tab("programs"), () => !document.querySelector(".st-wb-half"));
    await page.waitForTimeout(600);
    check((await where(page)).here === "/study", `the workbench ended by the tab led to ${(await where(page)).here}`);
  } else {
    console.log(JSON.stringify({ skipped: "the workbench: the current semester has no „Module hinzufügen“ (the checks need a snapshot of WiSe 2026/27)" }));
  }
  // 5
  await step("the Merkliste ordered", () => page.goto(base + "/bookmarks?sort=title").then(() => takeover(page)).then(() => page.evaluate(() => { window.__marker = 1; })), () => location.search === "?sort=title");
  await step("Merkliste again", () => tab("bookmarks"), () => location.pathname === "/bookmarks" && location.search === "");
  await step("the Stundenplan's Termine", () => tab("studyplan").then(() => page.waitForTimeout(300)).then(() => page.evaluate(() => {
    const a = Object.assign(document.createElement("a"), { href: "/studyplan?view=dates", textContent: "dates" });
    document.querySelector("#content > .page-stage.front").append(a);
    a.click();
  })), () => location.search === "?view=dates");
  await step("Stundenplan again", () => tab("studyplan"), () => location.pathname === "/studyplan" && location.search === "");
  // 6
  await step("Start", () => tab("home"), () => location.pathname === "/");
  await down(800);
  check((await where(page)).down > 100, "the start page did not scroll");
  await step("Start again", () => tab("home"), () => location.pathname === "/");
  check(await atTop(page), `Start again: the start page is not at its top (${(await where(page)).down})`);
  await context.close();
});

// ---------- a phone ----------
await part("phone", async () => {
  const { context, page, step } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 });
  const tab = (area) => page.tap(`.bottomnav a.nav[data-area="${area}"]`);
  const link = (href) => page.evaluate((href) => {
    const a = Object.assign(document.createElement("a"), { href, textContent: href, id: "x-link" });
    a.style.cssText = "position:fixed;top:200px;left:10px;z-index:99999;background:#fff;padding:20px";
    document.body.append(a);
  }, href).then(() => page.tap("#x-link")).then(() => page.evaluate(() => document.getElementById("x-link")?.remove()));
  // 7
  await step("Module", () => tab("catalog"), (home) => location.pathname + location.search === home, HOME);
  await step("the catalog filtered", () => link("/catalog?turnus=winter"), () => location.search === "?turnus=winter" && document.querySelector("#content > .page-stage.front .rows a.row"));
  await page.evaluate(() => window.scrollTo(0, 2500));
  await page.waitForTimeout(500);
  const row = await page.evaluate(() => [...document.querySelectorAll("#content > .page-stage.front .rows a.row[data-id]")].find((a) => { const r = a.getBoundingClientRect(); return r.top > 150 && r.bottom < 700; })?.dataset.id);
  await step("a module's own page", () => link(`/catalog/module/${row}`), (id) => location.pathname === `/catalog/module/${id}`, row);
  await page.evaluate(() => window.scrollTo(0, 400));
  await page.waitForTimeout(300);
  await step("Module again", () => tab("catalog"), (home) => location.pathname + location.search === home && document.querySelector("#content > .page-stage.front .rows a.row"), HOME);
  const again = await where(page);
  check(again.at === 1 && again.entries[0] === "/" && again.entries[1] === HOME, `Module again: the tab's stack is not the catalog of „Mein Studiengang“ above Start: ${JSON.stringify(again)}`);
  check(await atTop(page), `Module again: the window is not at its top (${again.down})`);
  // 8
  await page.evaluate(() => window.scrollTo(0, 3000));
  await page.waitForTimeout(600);
  const far = await where(page);
  await step("Module again where the app is", () => tab("catalog"), (home) => location.pathname + location.search === home, HOME);
  check(await atTop(page), `Module again where the app is: the window does not come to its top (${(await where(page)).down})`);
  const still = await where(page);
  check(still.at === far.at && still.entries.join(" ") === far.entries.join(" "), `Module again where the app is: the history changed (${JSON.stringify(far)} → ${JSON.stringify(still)})`);
  await step("Back: Start", () => page.evaluate(() => history.back()), () => location.pathname === "/");
  // 9
  await step("Studium", () => tab("programs"), () => location.pathname === "/study" && document.querySelector(".st-planbox"));
  await step("all programs", () => page.tap('.st-rest a[href="/programs"]'), () => location.pathname === "/programs");
  await step("a program", () => link("/programs/bachelor-informatik-2008/plan"), () => location.pathname === "/programs/bachelor-informatik-2008/plan");
  await step("Studium again", () => tab("programs"), () => location.pathname === "/study" && location.search === "" && document.querySelector(".st-planbox"));
  const study = await where(page);
  check(study.at === 1 && study.entries[0] === "/" && study.entries[1] === "/study", `Studium again: „Mein Studium“ is not right above Start: ${JSON.stringify(study)}`);
  const resting = (overview) => page.waitForFunction((overview) => {
    const here = document.querySelector(".st-pager > .st-slot[data-here]");
    return here && !document.querySelector(".st-pager.is-gliding") && here.classList.contains("is-overview") === overview;
  }, overview, { timeout: 4000 }).then(() => true, () => false);
  await page.tap(".st-planbox-head");
  check(await resting(false), "„Studium planen“ did not turn the row to a semester");
  await tab("programs");
  check(await resting(true), "Studium again: the row did not come back to the overview");
  // 10
  await step("Start", () => tab("home"), () => location.pathname === "/");
  await page.evaluate(() => window.scrollTo(0, 900));
  await page.waitForTimeout(400);
  await step("Start again", () => tab("home"), () => location.pathname === "/");
  check(await atTop(page), `Start again: the start page is not at its top (${(await where(page)).down})`);
  await context.close();
});

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
if (problems.length) {
  console.error(`again FAILED: ${problems.length} problem(s)`);
  process.exit(1);
}
