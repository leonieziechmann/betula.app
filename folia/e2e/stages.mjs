// Checks the tabs that stay built (folia/crates/shell/src/stage.rs; the concept
// folia/design/tabwechsel/konzept.html, „Tabs, die stehen bleiben"; owner, 2026-10-18: „Es ist nice
// zu sehen, dass die Seiten gleich da sind. Das muss auf jeden Fall rein").
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node stages.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Every tab's page stays built on a stage of its own in `main`, and a change of tab only swaps which
// stage is in front. On a phone (the bottom bar, taps) and on a wide screen (the rail, clicks):
// 1 a tab seen before comes in the first frame after the tap, without a skeleton and without a
//   picture held in front of it, as it was left: the same page (the very elements), scrolled as far
//   (the window on a phone, the page's scroll area on a wide screen), a module beside the list still
//   there;
// 2 one stage is in front, once a change has come to rest (on a phone the pages move, the Achse,
//   achse.mjs): the others are not drawn (`content-visibility: hidden`) and take no input (a link
//   in one takes no focus), and on a phone no room;
// 3 the document's title, its canonical address and its description are those of the page in
//   front, one of each;
// 4 a link inside a page that came back works (a filter of the catalog, a row);
// 5 the filters' sheet on a phone: a tap on the bar beside it closes it, and the tab stays; where
//   Back leaves the page with it open (an iPhone, no CloseWatcher), it goes with the page, and
//   nothing stays dimmed;
// 6 Back on a phone leads up to Start (nav.rs), which comes as it was left.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
let during = "";
const timings = {};

const open = async (options, path) => {
  const context = await browser.newContext({ locale: "de-DE", ...options });
  // „Einplanen" and the rest stay out of the way: nothing here plans.
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`console (${during}): ` + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push(`pageerror (${during}): ` + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await film(page);
  return { context, page };
};

// The frames after the next tap or click, or from now on (`now`): which stage is in front in each,
// and whether a skeleton or a picture held in front of the page is there. The frames from the click
// on, which changes the tab (a finger's tap is clicked a moment after it lifted, and a frame may come
// between), each timed from the finger's or the button's release. Nothing after 3 s without one: no
// frames. And the marker a page load takes.
const film = (page) => page.evaluate(() => {
    window.__marker = 1;
    window.__film = (now) => new Promise((done) => {
      const frames = [];
      let at = now ? performance.now() : null, clicked = Boolean(now);
      const start = (e) => { if (at === null) at = e.timeStamp; };
      if (!now) {
        addEventListener("pointerup", start, { capture: true, once: true });
        addEventListener("click", (e) => { start(e); clicked = true; }, { capture: true, once: true });
      }
      const until = performance.now() + 3000;
      const look = (now) => {
        if (!clicked && now > until) return done(frames);
        if (clicked) {
          frames.push({
            ms: Math.round(now - at),
            front: document.querySelector("#content > .page-stage.front")?.dataset.stage ?? null,
            skeleton: Boolean(document.querySelector("#content > .pending-page:not(.held)")),
            held: Boolean(document.querySelector("#content > .pending-page.held")),
            y: Math.round(scrollY),
          });
          if (now - at > 700) return done(frames);
        }
        requestAnimationFrame(look);
      };
      requestAnimationFrame(look);
    });
  });

// The stages as they are: which is in front, and what the others are.
const stages = (page) => page.evaluate(() => [...document.querySelectorAll("#content > .page-stage")].map((stage) => ({
  name: stage.dataset.stage,
  front: stage.classList.contains("front"),
  // A link in a stage behind takes no focus.
  focus: (() => {
    const link = stage.classList.contains("front") ? null : stage.querySelector("a[href]");
    if (!link) return false;
    const was = document.activeElement;
    link.focus({ preventScroll: true });
    const took = document.activeElement === link;
    if (took) link.blur();
    was?.focus?.({ preventScroll: true });
    return took;
  })(),
  hidden: getComputedStyle(stage).contentVisibility === "hidden",
  height: Math.round(stage.getBoundingClientRect().height),
  built: stage.childElementCount > 0,
})));
// The document's title, canonical address (its path) and descriptions.
const head = (page) => page.evaluate(() => ({
  title: document.title,
  canonical: [...document.querySelectorAll("link[rel=canonical]")].map((link) => new URL(link.href).pathname),
  descriptions: document.querySelectorAll("meta[name=description]").length,
}));
const reloaded = async (page) => !(await page.evaluate(() => window.__marker === 1));

// One stage in front, the others built or empty, not drawn, without input (and on a phone without
// room); the head the front page's. On a phone once the pages have come to rest.
const oneInFront = async (page, name, { phone, title, canonical }) => {
  await page.waitForFunction(() => !document.querySelector("#content > .page-stage.moving"), null, { timeout: 3000 }).catch(() => problems.push(`${during}: the pages did not come to rest`));
  const all = await stages(page);
  const front = all.filter((stage) => stage.front);
  check(front.length === 1 && front[0].name === name, `${during}: in front ${front.map((s) => s.name).join(", ") || "nothing"}, not ${name}`);
  for (const stage of all.filter((stage) => !stage.front)) {
    check(!stage.focus && stage.hidden, `${during}: the stage ${stage.name} behind ${stage.focus ? "takes focus " : ""}${stage.hidden ? "" : "is drawn"}`);
    if (phone) check(stage.height === 0, `${during}: the stage ${stage.name} behind takes ${stage.height} px`);
  }
  const now = await head(page);
  if (title) check(title.test(now.title), `${during}: the title is „${now.title}"`);
  check(now.canonical.length === 1 && (!canonical || now.canonical[0] === canonical), `${during}: canonical ${JSON.stringify(now.canonical)}, not one ${canonical ?? ""}`);
  check(now.descriptions === 1, `${during}: ${now.descriptions} descriptions`);
  return now;
};

// A tab (`area`) of the bottom bar or the rail, tapped or clicked: the frames after it.
const tab = async (page, area, phone) => {
  const selector = phone ? `.bottomnav > .nav[data-area="${area}"]` : `.rail nav > .nav[data-area="${area}"]`;
  const box = await page.locator(selector).first().boundingBox();
  // Filming before the finger or the button goes: input can overtake a script still waiting its turn.
  await page.evaluate(() => { window.__filming = window.__film(); });
  if (phone) await page.touchscreen.tap(box.x + box.width / 2, box.y + box.height / 2);
  else await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  return page.evaluate(() => window.__filming);
};
// A tab seen before: there in the first frame, nothing in front of it, and it stays.
const swapped = (frames, name) => {
  const first = frames[0];
  timings[during] = first?.ms;
  check(first && first.front === name, `${during}: the first frame after the tap (${first?.ms} ms) shows ${first?.front}, not ${name}`);
  check(first && first.ms <= 80, `${during}: the first frame came after ${first?.ms} ms`);
  check(frames.every((frame) => frame.front === name), `${during}: ${name} left the front again (${frames.map((f) => f.front).join(" ")})`);
  const covered = frames.filter((frame) => frame.skeleton || frame.held);
  check(covered.length === 0, `${during}: a skeleton or a held picture in ${covered.length} frames`);
};
const settle = (page) => page.waitForFunction(() => !document.querySelector("#content > .pending-page") && document.getElementById("content")?.getAttribute("aria-busy") !== "true", null, { timeout: 10000 }).catch(() => problems.push(`${during}: the page did not settle`));
// The element a stage's page starts with: the same one when the stage comes back.
const remember = (page, name, key) => page.evaluate(([name, key]) => { window[key] = document.querySelector(`#content > .page-stage[data-stage="${name}"] > :not(template)`); return Boolean(window[key]); }, [name, key]);
const same = (page, name, key) => page.evaluate(([name, key]) => document.querySelector(`#content > .page-stage[data-stage="${name}"] > :not(template)`) === window[key], [name, key]);

// The tabs built at the start (stage.rs, `prebuild`).
// A study set up, so that „Studium" is „Mein Studium" with its semesters, the most it shows.
const MINE = ["betula.myprogram.v1", "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n"];
const PLAN = ["betula.studyplan.v1", ["m\t2025W\t12104\t1\t\tplan", "m\t2025W\t12107\t1\t\tplan", "m\t2025W\t12102\t1\t\tplan", "m\t2025W\t11112\t1\t\tplan", "m\t2026S\t11113\t1\t\tplan", "m\t2026W\t12101\t1\t\tplan", "m\t2026W\t11903\t1\t\tplan"].join("\n") + "\n"];
const TABS = ["home", "catalog", "programs", "bookmarks", "studyplan"];
// Until every tab's stage has its page and its answers (no stage `aria-busy`).
const allBuilt = (page, timeout) => page.waitForFunction((tabs) => tabs.every((name) => {
  const stage = document.querySelector(`#content > .page-stage[data-stage="${name}"]`);
  return stage && stage.childElementCount > 0 && stage.getAttribute("aria-busy") !== "true";
}), TABS, { timeout }).then(() => true, () => false);
// ---- a phone
{
  const phone = true;
  const { context, page } = await open({ viewport: { width: 412, height: 915 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 }, "/");
  during = "phone: Start";
  await settle(page);
  // What follows is the app running: the start over, every tab built (`prebuild`).
  check(await allBuilt(page, 90000), `${during}: not every tab was built (${JSON.stringify(await stages(page))})`);
  check(await remember(page, "home", "__home"), `${during}: no page on the stage of Start`);
  await oneInFront(page, "home", { phone, canonical: "/" });

  during = "phone: the catalog the first time";
  await tab(page, "catalog", phone);
  await page.waitForFunction(() => document.querySelectorAll("#content > .page-stage.front .rows a.row").length > 5, null, { timeout: 15000 }).catch(() => problems.push(`${during}: no rows`));
  await settle(page);
  check(await remember(page, "catalog", "__catalog"), `${during}: no page on the catalog's stage`);
  const catalogHead = await oneInFront(page, "catalog", { phone, title: /Modulkatalog/, canonical: "/catalog" });
  await page.evaluate(() => scrollTo(0, 1400));
  await page.waitForTimeout(400);
  const down = await page.evaluate(() => Math.round(scrollY));
  check(down > 1000, `${during}: the list did not scroll (${down})`);

  during = "phone: the Merkliste the first time";
  await tab(page, "bookmarks", phone);
  await page.waitForFunction(() => document.querySelector("#content > .page-stage.front")?.dataset.stage === "bookmarks", null, { timeout: 10000 }).catch(() => problems.push(`${during}: did not come`));
  await settle(page);
  await oneInFront(page, "bookmarks", { phone, title: /Merkliste/, canonical: "/bookmarks" });
  check(await page.evaluate(() => scrollY) < 2, `${during}: the Merkliste is not at its top (${await page.evaluate(() => scrollY)})`);

  during = "phone: the catalog again";
  swapped(await tab(page, "catalog", phone), "catalog");
  check(await same(page, "catalog", "__catalog"), `${during}: the page was built anew`);
  const back = await page.evaluate(() => Math.round(scrollY));
  check(Math.abs(back - down) <= 2, `${during}: scrolled to ${back}, left at ${down}`);
  check((await oneInFront(page, "catalog", { phone, canonical: "/catalog" })).title === catalogHead.title, `${during}: another title than when it was left`);

  during = "phone: a filter on the catalog that came back";
  await page.evaluate(() => scrollTo(0, 0));
  await page.waitForTimeout(200);
  await page.tap("#content > .page-stage.front .sheet-toggle");
  await page.waitForFunction(() => document.querySelector("#content > .page-stage.front #filters")?.classList.contains("open") && document.documentElement.classList.contains("sheet-open"), null, { timeout: 5000 }).catch(() => problems.push(`${during}: the sheet did not open`));
  await page.waitForTimeout(400);

  during = "phone: a tap on the bar beside the open sheet";
  // The bar lies under the dimmed page: a tap on it is a tap beside the sheet, which closes it.
  const bar = await page.locator('.bottomnav > .nav[data-area="bookmarks"]').boundingBox();
  await page.touchscreen.tap(bar.x + bar.width / 2, bar.y + bar.height / 2);
  await page.waitForTimeout(600);
  check(await page.evaluate(() => document.querySelector("#content > .page-stage.front")?.dataset.stage === "catalog" && !document.documentElement.classList.contains("sheet-open")), `${during}: the sheet did not close, or the tab changed`);

  during = "phone: Back as an iPhone has it, the sheet open";
  // Safari has no CloseWatcher: Back is a step of the history, and the sheet goes with the page,
  // which goes behind.
  await page.evaluate(() => { window.__closeWatcher = window.CloseWatcher; delete window.CloseWatcher; });
  await page.tap("#content > .page-stage.front .sheet-toggle");
  await page.waitForFunction(() => document.querySelector("#content > .page-stage.front #filters")?.classList.contains("open"), null, { timeout: 5000 }).catch(() => problems.push(`${during}: the sheet did not open`));
  await page.waitForTimeout(400);
  await page.evaluate(() => history.back());
  await page.waitForFunction(() => document.querySelector("#content > .page-stage.front")?.dataset.stage === "home", null, { timeout: 5000 }).catch(() => problems.push(`${during}: Start did not come (${page.url()})`));
  await page.waitForTimeout(300);
  check(!(await page.evaluate(() => document.documentElement.classList.contains("sheet-open"))), `${during}: Start is dimmed`);
  check(!(await page.evaluate(() => document.querySelector('#content > .page-stage[data-stage="catalog"] #filters')?.classList.contains("open"))), `${during}: the sheet stayed open behind`);
  await page.evaluate(() => { window.CloseWatcher = window.__closeWatcher; });
  swapped(await tab(page, "catalog", phone), "catalog");
  await page.waitForTimeout(300);
  check(!(await page.evaluate(() => document.documentElement.classList.contains("sheet-open") || document.querySelector("#content > .page-stage.front #filters")?.classList.contains("open"))), `${during}: the sheet is open again`);

  during = "phone: a row of the catalog that came back";
  await page.tap("#content > .page-stage.front .rows a.row >> nth=1");
  await page.waitForFunction(() => location.search.includes("open=") && document.querySelector("#content > .page-stage.front .detail.is-module h2"), null, { timeout: 10000 }).catch(() => problems.push(`${during}: the module did not open (${page.url()})`));
  await page.waitForTimeout(500);
  await page.keyboard.press("Escape");
  await page.waitForFunction(() => !location.search.includes("open="), null, { timeout: 5000 }).catch(() => problems.push(`${during}: the module did not close`));
  await page.waitForTimeout(400);

  during = "phone: Back to Start";
  const frames = page.evaluate(() => { const film = window.__film(true); history.back(); return film; });
  const backFrames = await frames;
  await page.waitForFunction(() => document.querySelector("#content > .page-stage.front")?.dataset.stage === "home", null, { timeout: 5000 }).catch(() => problems.push(`${during}: Start did not come (${page.url()})`));
  check(backFrames.every((frame) => !frame.skeleton && !frame.held), `${during}: a skeleton or a held picture came`);
  check(await same(page, "home", "__home"), `${during}: Start was built anew`);
  check(new URL(page.url()).pathname === "/", `${during}: at ${page.url()}`);
  await oneInFront(page, "home", { phone, canonical: "/" });
  if (await reloaded(page)) problems.push("phone: the page was loaded again");
  await context.close();
}

// ---- a phone, the start over: every tab built behind the first page (stage.rs, `prebuild`)
for (const slow of [1, 4]) {
  const phone = true;
  const tag = slow > 1 ? ` (CPU ${slow}× slower)` : "";
  const { context, page } = await open({ viewport: { width: 412, height: 915 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 }, "/");
  await page.evaluate((entries) => { for (const [key, text] of entries) localStorage.setItem(key, text); }, [MINE, PLAN]);
  if (slow > 1) await (await context.newCDPSession(page)).send("Emulation.setCPUThrottlingRate", { rate: slow });
  // The long tasks from the load on: what a tap during the start may fall into.
  await page.addInitScript(() => {
    window.__long = [];
    new PerformanceObserver((list) => { for (const entry of list.getEntries()) window.__long.push([Math.round(entry.startTime), Math.round(entry.duration)]); }).observe({ type: "longtask", buffered: true });
  });
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  const tookOver = await page.evaluate(() => performance.now());
  await film(page);
  during = `phone, the start${tag}: every tab built`;
  check(await allBuilt(page, 90000), `${during}: not every tab was built (${JSON.stringify(await stages(page))})`);
  const start = await page.evaluate((tookOver) => ({ after: Math.round(performance.now() - tookOver), long: window.__long.filter(([at]) => at >= tookOver).map(([, ms]) => ms) }), tookOver);
  timings[`${during}: built after (ms)`] = start.after;
  timings[`${during}: its longest task (ms)`] = Math.max(0, ...start.long);
  timings[`${during}: its long tasks (ms)`] = start.long.reduce((sum, ms) => sum + ms, 0);
  await oneInFront(page, "home", { phone, canonical: "/" });
  // Each tab the first time: in the first frame after the tap, nothing in front of it, at its top
  // although the page before was scrolled.
  let homeAt = null;
  for (const name of ["bookmarks", "studyplan", "catalog", "programs", "home"]) {
    during = `phone, the start over${tag}: ${name} ${name === "home" ? "again" : "the first time"}`;
    await page.evaluate(() => scrollTo(0, 500));
    await page.waitForTimeout(300);
    homeAt ??= await page.evaluate(() => Math.round(scrollY));
    if (name !== "home") check(await remember(page, name, `__${name}`), `${during}: no page on its stage`);
    const frames = await tab(page, name, phone);
    if (slow === 1) swapped(frames, name);
    else check(frames[0]?.front === name && frames.every((frame) => !frame.skeleton && !frame.held), `${during}: ${JSON.stringify(frames.slice(0, 3))}`);
    timings[during] = frames[0]?.ms;
    // Start was left scrolled; the others open at their top, although the page before was scrolled.
    const y = await page.evaluate(() => Math.round(scrollY));
    if (name === "home") check(Math.abs(y - homeAt) <= 2, `${during}: scrolled to ${y}, left at ${homeAt}`);
    else check(y < 2, `${during}: not at its top (${y})`);
    if (name !== "home") check(await same(page, name, `__${name}`), `${during}: the page was built anew`);
    await settle(page);
    await oneInFront(page, name, { phone });
  }
  during = `phone, the start over${tag}: „Mein Studium" at its overview`;
  await tab(page, "programs", phone);
  await page.waitForTimeout(400);
  check(await page.evaluate(() => Boolean(document.querySelector("#content > .page-stage.front .st-pager > .st-slot[data-here].is-overview"))), `${during}: the row of the semesters does not rest at the overview`);
  if (await reloaded(page)) problems.push(`${during}: the page was loaded again`);
  await context.close();
}

// ---- a phone: a tab tapped while the tabs are still built
{
  const phone = true;
  const { context, page } = await open({ viewport: { width: 412, height: 915 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 }, "/");
  during = "phone, the start: „Studium“ tapped at once";
  await tab(page, "programs", phone);
  await page.waitForFunction(() => document.querySelector("#content > .page-stage.front")?.dataset.stage === "programs" && document.querySelector("#content > .page-stage.front .st-setup, #content > .page-stage.front .st-pager"), null, { timeout: 15000 }).catch(() => problems.push(`${during}: „Mein Studium“ did not come`));
  await settle(page);
  check(await remember(page, "programs", "__programs"), `${during}: no page on its stage`);
  during = "phone, the start: the other tabs built meanwhile";
  check(await allBuilt(page, 90000), `${during}: not every tab was built (${JSON.stringify(await stages(page))})`);
  check(await same(page, "programs", "__programs"), `${during}: „Mein Studium“ was built anew`);
  await oneInFront(page, "programs", { phone });
  if (await reloaded(page)) problems.push(`${during}: the page was loaded again`);
  await context.close();
}

// ---- a wide screen
{
  const phone = false;
  const { context, page } = await open({ viewport: { width: 1280, height: 900 } }, "/catalog");
  during = "wide: the catalog";
  await page.waitForFunction(() => document.querySelectorAll("#content > .page-stage.front .rows a.row").length > 5, null, { timeout: 15000 }).catch(() => problems.push(`${during}: no rows`));
  await settle(page);
  check(await allBuilt(page, 90000), `${during}: not every tab was built (${JSON.stringify(await stages(page))})`);
  check(await remember(page, "catalog", "__catalog"), `${during}: no page on the catalog's stage`);
  await page.click("#content > .page-stage.front .rows a.row >> nth=2");
  await page.waitForFunction(() => location.search.includes("open=") && document.querySelector("#content > .page-stage.front .work > .detail h2"), null, { timeout: 10000 }).catch(() => problems.push(`${during}: the module beside the list did not open`));
  await settle(page);
  const scroller = "#content > .page-stage.front .work.flowing";
  await page.$eval(scroller, (el) => { el.scrollTop = 1400; });
  await page.waitForTimeout(400);
  const down = await page.$eval(scroller, (el) => Math.round(el.scrollTop));
  check(down > 1000, `${during}: the list did not scroll (${down})`);
  const leftAt = new URL(page.url()).search;
  const catalogHead = await oneInFront(page, "catalog", { phone });

  during = "wide: the Merkliste the first time";
  await tab(page, "bookmarks", phone);
  await page.waitForFunction(() => document.querySelector("#content > .page-stage.front")?.dataset.stage === "bookmarks", null, { timeout: 10000 }).catch(() => problems.push(`${during}: did not come`));
  await settle(page);
  await oneInFront(page, "bookmarks", { phone, title: /Merkliste/, canonical: "/bookmarks" });

  during = "wide: the catalog again";
  swapped(await tab(page, "catalog", phone), "catalog");
  check(await same(page, "catalog", "__catalog"), `${during}: the page was built anew`);
  check(Math.abs((await page.$eval(scroller, (el) => Math.round(el.scrollTop))) - down) <= 2, `${during}: the list is not scrolled as it was left`);
  check(await page.evaluate(() => Boolean(document.querySelector("#content > .page-stage.front .work > .detail h2"))), `${during}: the module beside the list is gone`);
  await page.waitForTimeout(300);
  check(new URL(page.url()).search === leftAt, `${during}: at ${page.url()}, left at ${leftAt}`);
  const now = await oneInFront(page, "catalog", { phone });
  check(now.title === catalogHead.title && now.canonical[0] === catalogHead.canonical[0], `${during}: the title or the canonical address is not the one it was left with`);

  during = "wide: a filter on the catalog that came back";
  await page.click('#content > .page-stage.front #filters a.chip:has-text("Winter")');
  await page.waitForFunction(() => location.search.includes("turnus=winter") && document.querySelectorAll("#content > .page-stage.front .rows a.row").length > 5, null, { timeout: 10000 }).catch(() => problems.push(`${during}: the filter did not apply (${page.url()})`));
  if (await reloaded(page)) problems.push("wide: the page was loaded again");
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
if (problems.length) {
  console.error(`stages FAILED: ${problems.length} problem(s)`);
  process.exit(1);
}
console.log("stages ok");
