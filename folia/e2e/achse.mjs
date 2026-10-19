// A change of tab on a phone moves the two pages on an axis (folia/design/tabwechsel/konzept.html,
// „Die Bewegung am Handy: Achse"; enhance.js, the Achse): the page that goes slides away to the side
// of its tab and fades out, the one that comes slides in from the side of its tab and fades in a
// moment later. On a phone with a study set up and everything built:
// 1 a tap: Start (scrolled down) → Module. The page that goes leaves to the left, the one that comes
//   from the right, and only once the first has faded; no frame jumps; at rest within 0.8 s, nothing
//   left of the layers, the window where the page in front was left. In the first frame of the
//   change the page that goes stands where it stood in the window, in the last the page that comes
//   where it stands at rest;
// 2 Back from a tab's root to Start comes as a tap on „Start" does: from the left, scrolled as left;
// 3 a tap during the change back to the page that goes: it comes back from where it is, the other
//   goes back to its side, no jump;
// 4 three taps in a row: no jump, the last tab in front at rest;
// 5 a finger along the bar takes the pages with it: up to four tenths of its way the page goes,
//   after that the one of the tab beside comes; let go past a third, it comes to rest in front;
//   let go short, the page comes back and nothing changes;
// 6 the ground at the end of a page and the dots of „Mein Studium" move with their page, where
//   they stood;
// 7 with less motion a fade, nothing slides; on a wide screen nothing moves; a tab whose page does
//   not stand yet comes as before, without moving.
// Fails on a console error, a page error, or a page load after the takeover.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
let during = "";
const timings = {};

// „Mein Studium" with modules planned, and three modules kept, so that every tab has a page.
const MINE = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n";
const PLAN = ["m\t2025W\t12104\t1\t\tplan", "m\t2025W\t12107\t1\t\tplan", "m\t2026S\t11113\t1\t\tplan"].join("\n") + "\n";
const KEPT = "12104\n12107\n11113\n";
const TABS = ["catalog", "programs", "bookmarks", "studyplan"];
const PHONE = { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true };

const open = async (options, { built = true } = {}) => {
  const context = await browser.newContext({ locale: "de-DE", ...options });
  await context.addInitScript(([mine, plan, kept]) => {
    sessionStorage.setItem("betula.plan.asked", "1");
    if (!localStorage.getItem("betula.myprogram.v1")) {
      localStorage.setItem("betula.myprogram.v1", mine);
      localStorage.setItem("betula.studyplan.v1", plan);
      localStorage.setItem("betula.bookmarks.v1", kept);
    }
    // Each frame: the pose of every stage on its way (x px, how clear), which is in front, the
    // window's scroll.
    window.__frames = [];
    window.__filming = false;
    const look = (now) => {
      if (window.__filming) {
        const moving = {};
        for (const el of document.querySelectorAll("#content > .page-stage.moving")) {
          const cs = getComputedStyle(el), m = new DOMMatrix(cs.transform === "none" ? undefined : cs.transform);
          moving[el.dataset.stage] = { x: m.m41, o: Number(cs.opacity) };
        }
        window.__frames.push({ t: now, front: document.querySelector("#content > .page-stage.front")?.dataset.stage, y: scrollY, moving });
      }
      requestAnimationFrame(look);
    };
    requestAnimationFrame(look);
  }, [MINE, PLAN, KEPT]);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`console (${during}): ` + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push(`pageerror (${during}): ` + String(e).slice(0, 300)));
  await page.goto(base + "/", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  if (built) {
    await page.waitForFunction((tabs) => tabs.every((name) => { const s = document.querySelector(`#content > .page-stage[data-stage="${name}"]`); return s && s.childElementCount > 0 && s.getAttribute("aria-busy") !== "true"; }), TABS, { timeout: 60000 }).catch(() => problems.push("not every tab was built"));
    await page.waitForTimeout(800);
  }
  // After enhance.js has started a change: its animations held where `__hold` says (0 its start).
  await page.evaluate(() => {
    window.__marker = 1;
    addEventListener("betula:stage", () => {
      if (window.__hold === undefined) return;
      window.__held = document.getAnimations().filter((a) => a.effect?.target?.classList?.contains("page-stage"));
      for (const a of window.__held) { a.pause(); a.currentTime = window.__hold; }
    });
  });
  return { context, page };
};
const reloaded = async (page) => !(await page.evaluate(() => window.__marker === 1));
const point = async (page, area) => { const box = await page.locator(`.bottomnav > .nav[data-area="${area}"]`).boundingBox(); return { x: box.x + box.width / 2, y: box.y + box.height / 2, w: box.width }; };
const tap = async (page, area) => { const p = await point(page, area); await page.touchscreen.tap(p.x, p.y); };
const film = (page) => page.evaluate(() => { window.__frames = []; window.__filming = true; });
const cut = (page) => page.evaluate(() => { window.__filming = false; return window.__frames; });
const atRest = (page) => page.waitForFunction(() => !document.querySelector("#content > .page-stage.moving"), null, { timeout: 3000 }).then(() => true, () => false);
const rest = (page) => page.evaluate(() => ({ moving: document.querySelectorAll("#content > .page-stage.moving").length, room: document.getElementById("content").style.minHeight, cls: document.documentElement.classList.contains("stages-moving"), y: Math.round(scrollY), front: document.querySelector("#content > .page-stage.front")?.dataset.stage }));
// A jump: a stage's pose changing from one frame to the next further than its springs move it in
// the time between them (the pages move on the compositor; a frame of the page's thread late shows
// them that much further on), and further than a frame's worth at least: 900 px and 25 times its
// clearness a second, 12 px and a third of its clearness a frame. Sideways only where it can be seen
// (a page not yet come or gone already goes to the side of its tab unseen).
const jumps = (frames) => {
  let x = 0, o = 0;
  for (let i = 1; i < frames.length; i++) {
    const dt = Math.max(1 / 60, (frames[i].t - frames[i - 1].t) / 1000);
    for (const [name, pose] of Object.entries(frames[i].moving)) {
      const was = frames[i - 1].moving[name];
      if (!was) continue;
      if (Math.max(pose.o, was.o) > 0.02) x = Math.max(x, Math.abs(pose.x - was.x) - Math.max(12, 900 * dt) + 12);
      o = Math.max(o, Math.abs(pose.o - was.o) - Math.max(0.35, 25 * dt) + 0.35);
    }
  }
  return { x, o };
};
const poses = (frames, name) => frames.map((f) => f.moving[name]).filter(Boolean);
// The time from the first frame of a change until it is at rest (ms).
const lasted = (frames) => {
  const moving = frames.filter((f) => Object.keys(f.moving).length);
  return moving.length ? Math.round(moving[moving.length - 1].t - moving[0].t) : 0;
};
// Every frame smooth: no jump (`jumps`).
const smooth = (frames) => {
  const j = jumps(frames);
  check(j.x <= 12 && j.o <= 0.35, `${during}: a jump between two frames (${j.x.toFixed(1)} px, ${j.o.toFixed(2)} beyond what the springs move)`);
};

// ---- a phone ----
{
  const { context, page } = await open(PHONE);

  during = "phone: a tap, Start → Module";
  await page.evaluate(() => scrollTo(0, 1200));
  await page.waitForTimeout(400);
  await film(page);
  await tap(page, "catalog");
  check(await atRest(page), `${during}: the pages did not come to rest`);
  let frames = await cut(page);
  smooth(frames);
  const home = poses(frames, "home"), catalog = poses(frames, "catalog");
  check(home.length > 3 && catalog.length > 3, `${during}: the pages did not move (${home.length}, ${catalog.length} frames)`);
  check(Math.min(...home.map((p) => p.x)) <= -30 && home[home.length - 1].o < 0.05, `${during}: Start did not go to the left and fade`);
  check(catalog[0].x >= 30 && catalog[0].o < 0.05 && Math.abs(catalog[catalog.length - 1].x) < 2, `${during}: Module did not come from the right`);
  // Module comes once Start has faded: while Start is still clear, Module is not.
  const both = frames.filter((f) => f.moving.home && f.moving.catalog);
  check(both.every((f) => f.moving.home.o < 0.5 || f.moving.catalog.o < 0.1), `${during}: Module came before Start was gone`);
  timings[`${during}: until at rest (ms)`] = lasted(frames);
  check(lasted(frames) < 800, `${during}: ${lasted(frames)} ms until at rest`);
  let now = await rest(page);
  check(now.front === "catalog" && now.moving === 0 && !now.room && !now.cls && now.y === 0, `${during}: not at rest as it should be: ${JSON.stringify(now)}`);

  during = "phone: Back from Module to Start";
  await film(page);
  await page.evaluate(() => history.back());
  await page.waitForFunction(() => document.querySelector("#content > .page-stage.front")?.dataset.stage === "home", null, { timeout: 5000 }).catch(() => problems.push(`${during}: Start did not come (${page.url()})`));
  check(await atRest(page), `${during}: the pages did not come to rest`);
  frames = await cut(page);
  smooth(frames);
  const back = poses(frames, "home");
  check(back.length > 3 && back[0].x <= -30 && back[0].o < 0.05, `${during}: Start did not come from the left as a tap on it does`);
  now = await rest(page);
  check(now.front === "home" && now.y === 1200, `${during}: Start is not where it was left: ${JSON.stringify(now)}`);

  during = "phone: a tap during the change, back to the page that goes";
  await film(page);
  await tap(page, "catalog");
  await page.waitForTimeout(140);
  await tap(page, "home");
  check(await atRest(page), `${during}: the pages did not come to rest`);
  frames = await cut(page);
  smooth(frames);
  now = await rest(page);
  check(now.front === "home" && now.y === 1200 && now.moving === 0, `${during}: Start is not back where it was: ${JSON.stringify(now)}`);
  const turned = poses(frames, "home");
  check(turned.some((p) => p.o < 0.9) && turned[turned.length - 1].o > 0.99, `${during}: Start did not go and come back`);

  during = "phone: three taps in a row";
  await film(page);
  for (const area of ["catalog", "programs", "bookmarks"]) { await tap(page, area); await page.waitForTimeout(70); }
  check(await atRest(page), `${during}: the pages did not come to rest`);
  frames = await cut(page);
  smooth(frames);
  now = await rest(page);
  check(now.front === "bookmarks" && now.moving === 0 && !now.room, `${during}: not at rest on the Merkliste: ${JSON.stringify(now)}`);

  during = "phone: a finger along the bar, Merkliste → Stundenplan";
  const from = await point(page, "bookmarks"), to = await point(page, "studyplan"), step = to.x - from.x;
  const along = async (share) => { await page.mouse.move(from.x - step * share, from.y); await page.waitForTimeout(40); return page.evaluate(() => Object.fromEntries([...document.querySelectorAll("#content > .page-stage.moving")].map((el) => { const cs = getComputedStyle(el), m = new DOMMatrix(cs.transform === "none" ? undefined : cs.transform); return [el.dataset.stage, { x: m.m41, o: Number(cs.opacity) }]; }))); };
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  for (const share of [0.1, 0.2]) await along(share);
  const third = await along(0.35);
  check(third.bookmarks && third.bookmarks.x < -10 && third.bookmarks.o < 0.75 && (!third.studyplan || third.studyplan.o < 0.05), `${during}: a third of the way: ${JSON.stringify(third)}`);
  for (const share of [0.5, 0.65, 0.8]) await along(share);
  const most = await along(0.95);
  check(most.studyplan && most.studyplan.o > 0.5 && most.studyplan.x > 0 && most.studyplan.x < 25 && most.bookmarks?.o < 0.05, `${during}: most of the way: ${JSON.stringify(most)}`);
  await film(page);
  await page.mouse.up();
  check(await atRest(page), `${during}: the pages did not come to rest`);
  frames = await cut(page);
  smooth(frames);
  now = await rest(page);
  check(now.front === "studyplan" && now.moving === 0, `${during}: Stundenplan is not in front at rest: ${JSON.stringify(now)}`);

  during = "phone: a finger along the bar let go short";
  const here = await point(page, "studyplan"), left = await point(page, "bookmarks"), way = here.x - left.x;
  await page.mouse.move(here.x, here.y);
  await page.mouse.down();
  for (const share of [0.08, 0.15, 0.22]) { await page.mouse.move(here.x + way * share, here.y); await page.waitForTimeout(60); }
  const short = await page.evaluate(() => [...document.querySelectorAll("#content > .page-stage.moving")].map((el) => el.dataset.stage));
  check(short.includes("studyplan"), `${during}: the page did not follow the finger (${short.join(", ")})`);
  await page.waitForTimeout(150);
  await page.mouse.up();
  check(await atRest(page), `${during}: the pages did not come to rest`);
  now = await rest(page);
  check(now.front === "studyplan" && now.moving === 0, `${during}: the tab changed: ${JSON.stringify(now)}`);

  during = "phone: the ground and the dots move with their page";
  await tap(page, "bookmarks");
  await atRest(page);
  await page.evaluate(() => scrollTo(0, 1e6));
  await page.waitForTimeout(400);
  const ground = await page.evaluate(() => Math.round(document.querySelector("body > .ground").getBoundingClientRect().top));
  await page.evaluate(() => { window.__hold = 0; });
  await tap(page, "home");
  await page.waitForFunction(() => (window.__held || []).length > 0, null, { timeout: 3000 }).catch(() => {});
  const held = await page.evaluate(() => Math.round(document.querySelector('#content > .page-stage.moving[data-stage="bookmarks"] > .work > .ground')?.getBoundingClientRect().top ?? -1));
  check(Math.abs(held - ground) <= 1, `${during}: the Merkliste's ground stood at ${ground} px, in its first frame on the way at ${held} px`);
  await page.evaluate(() => { for (const a of window.__held || []) a.play(); window.__held = []; window.__hold = undefined; });
  await atRest(page);
  await tap(page, "programs");
  await atRest(page);
  const dock = await page.evaluate(() => Math.round(document.querySelector("#content > .page-stage.front .st-dock")?.getBoundingClientRect().bottom ?? -1));
  await page.evaluate(() => { window.__hold = 0; });
  await tap(page, "bookmarks");
  await page.waitForFunction(() => (window.__held || []).length > 0, null, { timeout: 3000 }).catch(() => {});
  const dockHeld = await page.evaluate(() => Math.round(document.querySelector('#content > .page-stage.moving[data-stage="programs"] .st-dock')?.getBoundingClientRect().bottom ?? -1));
  check(dock > 0 && Math.abs(dockHeld - dock) <= 1, `${during}: the dots of „Mein Studium" stood at ${dock} px, on the way at ${dockHeld} px`);
  await page.evaluate(() => { for (const a of window.__held || []) a.play(); window.__held = []; window.__hold = undefined; });
  check(await atRest(page), `${during}: the pages did not come to rest`);

  if (await reloaded(page)) problems.push("phone: the page was loaded again");
  await context.close();
}

// ---- less motion: a fade ----
{
  const { context, page } = await open({ ...PHONE, reducedMotion: "reduce" });
  during = "phone, less motion";
  await film(page);
  await tap(page, "catalog");
  check(await atRest(page), `${during}: the pages did not come to rest`);
  const frames = await cut(page);
  const all = Object.values(frames.flatMap((f) => Object.values(f.moving)));
  check(all.length > 3 && all.every((p) => Math.abs(p.x) < 0.5) && all.some((p) => p.o < 0.5), `${during}: not a fade (${all.length} frames, x up to ${Math.max(0, ...all.map((p) => Math.abs(p.x))).toFixed(1)})`);
  await context.close();
}

// ---- a tab whose page does not stand yet ----
{
  during = "phone, a tab tapped before its page stands";
  const { context, page } = await open(PHONE, { built: false });
  const name = await page.evaluate((tabs) => tabs.find((t) => { const s = document.querySelector(`#content > .page-stage[data-stage="${t}"]`); return !s?.childElementCount || s.getAttribute("aria-busy") === "true"; }) ?? null, ["programs", "catalog", "home"]);
  if (name) {
    await film(page);
    await tap(page, name);
    await page.waitForFunction((n) => document.querySelector("#content > .page-stage.front")?.dataset.stage === n, name, { timeout: 15000 }).catch(() => problems.push(`${during}: ${name} did not come`));
    await page.waitForTimeout(300);
    const frames = await cut(page);
    check(frames.every((f) => !f.moving[name]), `${during}: ${name} moved`);
  }
  await context.close();
}

// ---- a wide screen ----
{
  during = "wide";
  const { context, page } = await open({ viewport: { width: 1280, height: 900 } });
  await film(page);
  await page.locator('.rail .nav[data-area="catalog"]').first().click();
  await page.waitForFunction(() => document.querySelector("#content > .page-stage.front")?.dataset.stage === "catalog", null, { timeout: 5000 }).catch(() => problems.push(`${during}: Module did not come`));
  await page.waitForTimeout(300);
  const frames = await cut(page);
  check(frames.every((f) => !Object.keys(f.moving).length), `${during}: a page moved`);
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
if (problems.length) {
  console.error(`achse FAILED: ${problems.length} problem(s)`);
  process.exit(1);
}
console.log("achse ok");
