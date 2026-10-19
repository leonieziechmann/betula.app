// A change of tab on a phone moves the pages on an axis (folia/design/tabwechsel/konzept.html,
// „Die Bewegung am Handy: Achse" and „Schnell hintereinander"; enhance.js, the Achse): the page that
// comes slides in from the side of its tab and gets clear over the page that goes, which slides away
// to the side of its own tab under it. What the eye sees of each page is how clear it is, of what
// the pages over it leave (each layer is opaque); every frame shows a whole screen of pages, never
// the empty window. On a phone with a study set up and everything built:
// 1 a tap: Start (scrolled down) → Module. Module comes from the right, on top, seen from the first
//   frames of the change (half of it within 120 ms of the tap), the tab's mark on „Module" in the
//   frame the pages start; Start goes to the left under it; no frame jumps and none is empty; at rest
//   within 0.8 s, nothing left of the layers, the window where the page in front was left. In the
//   first frame of the change the page that goes stands where it stood in the window, in the last
//   the page that comes where it stands at rest;
// 2 Back from a tab's root to Start comes as a tap on „Start" does: from the left, scrolled as left;
// 3 a tap during the change back to the page that goes: it comes back from where it is, the other
//   goes back to its side, no jump;
// 4 three taps in a row, and four quick ones: no jump, no empty frame, the last tab in front at rest;
// 5 a finger along the bar takes the pages with it: the page of the tab beside comes over the one
//   in front as the finger goes; let go past a third, it comes to rest in front; let go short, the
//   page comes back and nothing changes. A quick swipe (a finger's moves every 8 ms) is seen as a
//   motion: the page that comes takes 0.15 s at least from a tenth of it seen to nine tenths;
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
    // Each frame: the pose of every stage on its way (x px, how clear, how high it lies), which is
    // in front, the bar's current tab, the window's scroll.
    window.__frames = [];
    window.__filming = false;
    const look = (now) => {
      if (window.__filming) {
        const moving = {};
        for (const el of document.querySelectorAll("#content > .page-stage.moving")) {
          const cs = getComputedStyle(el), m = new DOMMatrix(cs.transform === "none" ? undefined : cs.transform);
          moving[el.dataset.stage] = { x: m.m41, o: Number(cs.opacity), z: Number(cs.zIndex) || 0 };
        }
        const tab = document.querySelector('.bottomnav > .nav[aria-current="page"]')?.dataset.area;
        window.__frames.push({ t: now, front: document.querySelector("#content > .page-stage.front")?.dataset.stage, tab, y: scrollY, moving });
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
// What the eye sees of each page on its way: as clear as it is, of what the pages over it leave
// (`seen`), and how much of the screen the pages cover (`covered`; each is opaque, so 1 where one
// of them is all clear).
const weights = (frame) => {
  const list = Object.entries(frame.moving).sort((a, b) => b[1].z - a[1].z), seen = {};
  let left = 1;
  for (const [name, pose] of list) { seen[name] = pose.o * left; left *= 1 - pose.o; }
  return { seen, covered: list.length ? 1 - left : 1 };
};
// A jump: a stage's pose changing from one frame to the next further than its springs move it in
// the time between them (the pages move on the compositor; a frame of the page's thread late shows
// them that much further on), and further than a frame's worth at least: 900 px and 25 times its
// clearness a second, 12 px and a third of its clearness a frame. How much of it is seen, which
// changes nowhere a page goes on top (its clearness does); sideways only where it can be seen (a
// page not yet come or gone already goes to the side of its tab unseen).
const jumps = (frames) => {
  let x = 0, o = 0;
  for (let i = 1; i < frames.length; i++) {
    const dt = Math.max(1 / 60, (frames[i].t - frames[i - 1].t) / 1000), now = weights(frames[i]).seen, was = weights(frames[i - 1]).seen;
    for (const [name, pose] of Object.entries(frames[i].moving)) {
      const before = frames[i - 1].moving[name];
      if (!before) continue;
      if (Math.max(now[name], was[name]) > 0.02) x = Math.max(x, Math.abs(pose.x - before.x) - Math.max(12, 900 * dt) + 12);
      o = Math.max(o, Math.abs(now[name] - was[name]) - Math.max(0.35, 25 * dt) + 0.35);
    }
  }
  return { x, o };
};
// A stage's pose in each frame it moves, with how much of it is seen (`w`).
const poses = (frames, name) => frames.filter((f) => f.moving[name]).map((f) => ({ ...f.moving[name], w: weights(f).seen[name], t: f.t }));
// The time from the first frame of a change until it is at rest (ms).
const lasted = (frames) => {
  const moving = frames.filter((f) => Object.keys(f.moving).length);
  return moving.length ? Math.round(moving[moving.length - 1].t - moving[0].t) : 0;
};
// Every frame smooth: no jump (`jumps`), and none empty: the pages on their way cover the screen.
const smooth = (frames) => {
  const j = jumps(frames);
  check(j.x <= 12 && j.o <= 0.35, `${during}: a jump between two frames (${j.x.toFixed(1)} px, ${j.o.toFixed(2)} beyond what the springs move)`);
  const empty = frames.filter((f) => Object.keys(f.moving).length && weights(f).covered < 0.98);
  check(empty.length === 0, `${during}: ${empty.length} frames with the window showing between the pages (${empty.slice(0, 3).map((f) => weights(f).covered.toFixed(2)).join(", ")})`);
};
// A finger's touches on the bar, sent on time as a phone sends them (not each after the page took
// the one before): along it from the middle of the tab of `area`, `dx` px in `ms`, a move every 8 ms.
const swipe = async (page, area, dx, ms) => {
  const cdp = await page.context().newCDPSession(page), p = await point(page, area);
  const touch = (type, x) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x, y: p.y, id: 1, radiusX: 6, radiusY: 6, force: 1 }] });
  await touch("touchStart", p.x);
  const sent = [], t0 = Date.now(), n = Math.max(2, Math.round(ms / 8));
  for (let k = 1; k <= n; k++) {
    const wait = t0 + (k * ms) / n - Date.now();
    if (wait > 0) await new Promise((r) => setTimeout(r, wait));
    const f = k / n;
    sent.push(touch("touchMove", p.x + dx * (f < 0.5 ? 2 * f * f : 1 - 2 * (1 - f) * (1 - f))));
  }
  sent.push(touch("touchEnd", p.x + dx));
  await Promise.all(sent);
  await cdp.detach();
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
  check(Math.min(...home.map((p) => p.x)) <= -30 && home[home.length - 1].w < 0.02, `${during}: Start did not go to the left under Module`);
  check(catalog[0].x >= 30 && catalog[0].w < 0.35 && Math.abs(catalog[catalog.length - 1].x) < 2 && catalog[catalog.length - 1].w > 0.99, `${during}: Module did not come from the right`);
  // Module on top from the start, and seen at once: half of it within 120 ms of the tap.
  const both = frames.filter((f) => f.moving.home && f.moving.catalog);
  check(both.every((f) => f.moving.catalog.z > f.moving.home.z), `${during}: Module not over Start`);
  const begun = catalog.find((p) => p.w > 0.02 || Math.abs(p.x - 36) > 0.5), half = catalog.find((p) => p.w >= 0.5);
  timings[`${during}: half of Module seen after the first frame of the change (ms)`] = begun && half ? Math.round(half.t - begun.t) : null;
  check(begun && half && half.t - begun.t <= 120, `${during}: half of Module seen ${begun && half ? Math.round(half.t - begun.t) : "never"} ms after the change began`);
  // The tab's mark on „Module" in the frame the pages start, not after the app has the click.
  const start = frames.find((f) => f.moving.catalog && (weights(f).seen.catalog > 0.02 || f.moving.home.x < -0.5));
  check(start?.tab === "catalog", `${during}: the mark on ${start?.tab} as the pages start`);
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
  check(back.length > 3 && back[0].x <= -30 && back[0].w < 0.35, `${during}: Start did not come from the left as a tap on it does`);
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
  check(turned.some((p) => p.w < 0.9) && turned[turned.length - 1].w > 0.99, `${during}: Start did not go and come back`);

  during = "phone: three taps in a row";
  await film(page);
  for (const area of ["catalog", "programs", "bookmarks"]) { await tap(page, area); await page.waitForTimeout(70); }
  check(await atRest(page), `${during}: the pages did not come to rest`);
  frames = await cut(page);
  smooth(frames);
  now = await rest(page);
  check(now.front === "bookmarks" && now.moving === 0 && !now.room, `${during}: not at rest on the Merkliste: ${JSON.stringify(now)}`);

  during = "phone: four quick taps, back and forth";
  await film(page);
  for (const area of ["studyplan", "bookmarks", "studyplan", "programs"]) { await tap(page, area); await page.waitForTimeout(40); }
  check(await atRest(page), `${during}: the pages did not come to rest`);
  frames = await cut(page);
  smooth(frames);
  now = await rest(page);
  check(now.front === "programs" && now.moving === 0 && !now.room, `${during}: not at rest on „Mein Studium": ${JSON.stringify(now)}`);
  await tap(page, "bookmarks");
  await atRest(page);

  during = "phone: a finger along the bar, Merkliste → Stundenplan";
  const from = await point(page, "bookmarks"), to = await point(page, "studyplan"), step = to.x - from.x;
  const along = async (share) => { await page.mouse.move(from.x - step * share, from.y); await page.waitForTimeout(40); return page.evaluate(() => Object.fromEntries([...document.querySelectorAll("#content > .page-stage.moving")].map((el) => { const cs = getComputedStyle(el), m = new DOMMatrix(cs.transform === "none" ? undefined : cs.transform); return [el.dataset.stage, { x: m.m41, o: Number(cs.opacity), z: Number(cs.zIndex) || 0 }]; }))); };
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  for (const share of [0.1, 0.2]) await along(share);
  await page.waitForTimeout(120);
  const third = await along(0.35);
  check(third.bookmarks && third.bookmarks.x < -5 && third.studyplan && third.studyplan.o > 0.1 && third.studyplan.o < 0.6 && third.studyplan.x > 15 && third.studyplan.z > third.bookmarks.z, `${during}: a third of the way: ${JSON.stringify(third)}`);
  for (const share of [0.5, 0.65, 0.8]) await along(share);
  await page.waitForTimeout(200);
  const most = await along(0.95);
  check(most.studyplan && most.studyplan.o > 0.6 && most.studyplan.x >= 0 && most.studyplan.x < 25 && most.studyplan.z > (most.bookmarks?.z ?? 0), `${during}: most of the way: ${JSON.stringify(most)}`);
  await film(page);
  await page.mouse.up();
  check(await atRest(page), `${during}: the pages did not come to rest`);
  frames = await cut(page);
  smooth(frames);
  now = await rest(page);
  check(now.front === "studyplan" && now.moving === 0, `${during}: Stundenplan is not in front at rest: ${JSON.stringify(now)}`);

  during = "phone: a quick swipe along the bar, Stundenplan → Merkliste";
  await film(page);
  await swipe(page, "studyplan", 150, 70);
  check(await atRest(page), `${during}: the pages did not come to rest`);
  frames = await cut(page);
  smooth(frames);
  now = await rest(page);
  check(now.front === "bookmarks" && now.moving === 0, `${during}: the Merkliste is not in front at rest: ${JSON.stringify(now)}`);
  const came = poses(frames, "bookmarks"), tenth = came.find((p) => p.w >= 0.1), nine = came.find((p) => p.w >= 0.9);
  timings[`${during}: a tenth to nine tenths of the page seen (ms)`] = tenth && nine ? Math.round(nine.t - tenth.t) : null;
  check(tenth && nine && nine.t - tenth.t >= 150, `${during}: the page came in ${tenth && nine ? Math.round(nine.t - tenth.t) : "no"} ms, too quick to be seen as a motion`);
  await swipe(page, "bookmarks", -150, 70);
  await atRest(page);

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
