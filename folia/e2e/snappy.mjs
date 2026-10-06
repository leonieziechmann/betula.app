// Checks that a click answers in the next frame (folia/crates/shell/src/pending.rs): what was clicked shows its
// new state at once, what still has to be computed stands there as a skeleton, and the result
// follows and replaces it.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node snappy.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Records every frame after an interaction: the first one must come quickly and show the
// feedback (the tab of the rail, the toggle, the row, the view of a program, „Einplanen" with
// „Passt in meinen Stundenplan" on), a skeleton where the page, the list or the preview is being
// built (the first change of each kind has no measured duration yet, so it always gets one), and
// a later one the result without a skeleton. Then the
// same on a phone and with the CPU slowed down four times, where the skeletons are what bridges
// the wait. Fails on a page load after takeover, a console error, or a frame that does not show
// what it should.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };

// The first frame after an interaction has to come within this many ms (a frame and the work the
// click itself does), whatever the page then takes: on this machine, and four times slower.
const FEEDBACK_MS = { 1: 80, 4: 200 };

async function open(context, path) {
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await page.waitForTimeout(500);
  await page.evaluate(() => {
    window.__marker = 1;
    // Frames after the next click, tap, key or `window.__go()`: when each came, whether `feedback`
    // and `result` held in it, and whether a skeleton was there.
    window.__watch = (feedbackSrc, resultSrc) => new Promise((resolve) => {
      const feedback = new Function("return (" + feedbackSrc + ")()");
      const result = new Function("return (" + resultSrc + ")()");
      const frames = [];
      let at = null, left = null;
      const mark = (e) => { if (at === null) at = e?.timeStamp ?? performance.now(); };
      for (const type of ["click", "keydown"]) addEventListener(type, mark, { capture: true, once: true });
      // Back and Forward: the step begins when the browser has moved (its event comes a moment
      // after `history.back()`), which is the first frame with the new address.
      window.__back = () => { left = location.href; history.back(); };
      const skeleton = () => Boolean(document.querySelector(".pending-page, .rows-pending, .sk-detail"));
      const tick = () => {
        const now = performance.now();
        if (at === null && left !== null && location.href !== left) at = now;
        if (at !== null) {
          const safe = (f) => { try { return Boolean(f()); } catch { return false; } };
          frames.push({ ms: Math.round(now - at), feedback: safe(feedback), result: safe(result), skeleton: skeleton(), busy: document.querySelector("main")?.getAttribute("aria-busy") === "true" });
          const last = frames[frames.length - 1];
          if ((last.result && !last.skeleton && !last.busy) || now - at > 15000) return resolve(frames);
        }
        requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
  });
  return page;
}

// One interaction: `act` clicks (or taps, or presses), `feedback` must hold in the first frame
// after it, `result` in the last. `skeleton`: a skeleton has to be in the first frame (`true`),
// or may be (`null`).
async function watch(page, name, act, { feedback, result, skeleton = null }, slow = 1) {
  const frames = page.evaluate(([f, r]) => window.__watch(f, r), [feedback.toString(), result.toString()]);
  await page.waitForTimeout(30);
  await act();
  const seen = await frames;
  const first = seen[0];
  const last = seen[seen.length - 1];
  timings[name] = { feedback: first?.ms, result: last?.ms, frames: seen.length };
  check(first && first.ms <= FEEDBACK_MS[slow], `${name}: the first frame came after ${first?.ms} ms`);
  check(first?.feedback, `${name}: the first frame does not show what was clicked`);
  if (skeleton === true) check(first?.skeleton || first?.result, `${name}: neither a skeleton nor the result in the first frame`);
  check(last?.result && !last.skeleton && !last.busy, `${name}: the result did not come, or a skeleton stayed (${JSON.stringify(last)})`);
  check(await page.evaluate(() => window.__marker === 1), `${name}: the page was loaded again`);
  await page.waitForTimeout(250);
}

const box = async (page, selector) => {
  const found = await page.locator(selector).first().boundingBox();
  if (!found) throw new Error("not on the page: " + selector);
  return { x: found.x + Math.min(found.width / 2, 40), y: found.y + found.height / 2 };
};
const click = (page, selector) => async () => { const at = await box(page, selector); await page.mouse.click(at.x, at.y); };
const tap = (page, selector) => async () => { const at = await box(page, selector); await page.touchscreen.tap(at.x, at.y); };

for (const slow of [1, 4]) {
  const tag = slow > 1 ? ` (CPU ${slow}× slower)` : "";
  // ---- desktop
  const context = await browser.newContext({ viewport: { width: 1500, height: 900 } });
  const page = await open(context, "/catalog");
  if (slow > 1) await (await context.newCDPSession(page)).send("Emulation.setCPUThrottlingRate", { rate: slow });
  const winter = '#filters a.chip:has-text("Winter")';
  await watch(page, "a toggle" + tag, click(page, winter), {
    feedback: () => document.querySelector('#filters a.chip[data-state="with"]')?.textContent.includes("Winter") && document.querySelector(".tag")?.textContent.includes("Winter"),
    result: () => location.search.includes("turnus=winter") && document.querySelectorAll(".rows a.row").length > 5,
    skeleton: true,
  }, slow);
  check(await page.evaluate(() => !document.querySelector(".list[data-pending]")), "a toggle: the list still waits");
  await watch(page, "a row" + tag, click(page, ".rows a.row >> nth=2"), {
    feedback: () => document.querySelectorAll('.rows a.row[aria-current="true"]').length === 1,
    result: () => location.search.includes("open=") && document.querySelector(".detail h2"),
    skeleton: true,
  }, slow);
  await watch(page, "Esc closes the preview" + tag, () => page.keyboard.press("Escape"), {
    feedback: () => !document.querySelector(".work > .detail"),
    result: () => !location.search.includes("open=") && !document.querySelector(".work > .detail"),
  }, slow);
  // „Studium" is „Mein Studium" first (owner, 2026-10-04), the overview one link away.
  await watch(page, "the rail: Studium" + tag, click(page, '.rail a.nav[data-area="programs"]'), {
    feedback: () => document.querySelector('.rail a.nav[data-area="programs"]')?.getAttribute("aria-current") === "page" && document.querySelector(".crumb h1")?.textContent === "Mein Studium",
    result: () => location.pathname === "/study" && document.querySelector(".st-setup"),
    skeleton: true,
  }, slow);
  await watch(page, "all programs" + tag, click(page, '#sidebar a[href="/programs"]'), {
    feedback: () => document.querySelector(".crumb h1")?.textContent === "Studiengänge",
    result: () => location.pathname === "/programs" && document.querySelectorAll(".program-pill").length > 50,
    skeleton: true,
  }, slow);
  await watch(page, "a program" + tag, click(page, '.program-pill[href^="/programs/bachelor-informatik"]'), {
    feedback: () => document.querySelector(".pending-page") || document.querySelector(".prog-head"),
    result: () => location.pathname.startsWith("/programs/bachelor-informatik") && document.querySelector(".prog-head h1"),
    skeleton: true,
  }, slow);
  // „Wahlpflicht & Bereiche": the tab in the first frame, then the table of the program's areas.
  await watch(page, "a view of the program" + tag, click(page, '.sidebar a[data-walk="tab"][href$="/areas"]'), {
    feedback: () => document.querySelector('.sidebar a[data-walk="tab"][aria-current="page"]')?.getAttribute("href").endsWith("/areas"),
    result: () => location.pathname.endsWith("/areas") && document.querySelector("table.areas tr.group"),
    skeleton: true,
  }, slow);
  // Another view of the program, the second one: a skeleton only where the first one was slow.
  await watch(page, "Back" + tag, () => page.evaluate(() => window.__back()), {
    feedback: () => document.querySelector('.sidebar a[data-walk="tab"][aria-current="page"]')?.getAttribute("href").endsWith("/plan"),
    result: () => location.pathname.endsWith("/plan") && (document.querySelector("table.matrix") || document.querySelector("table.planlist")),
  }, slow);
  await watch(page, "the rail: start" + tag, click(page, '.rail a.nav[data-area="home"]'), {
    feedback: () => document.querySelector('.rail a.nav[data-area="home"]')?.getAttribute("aria-current") === "page",
    result: () => location.pathname === "/" && document.querySelector(".home-hero h1"),
    skeleton: true,
  }, slow);
  // The Stundenplan: the tab and the title in the first frame, the frame of the plan as its
  // skeleton, then the plan (an empty one: this browser has planned nothing).
  await watch(page, "the rail: Stundenplan" + tag, click(page, '.rail a.nav[data-area="studyplan"]'), {
    feedback: () => document.querySelector('.rail a.nav[data-area="studyplan"]')?.getAttribute("aria-current") === "page" && document.querySelector(".crumb h1")?.textContent === "Stundenplan",
    result: () => location.pathname === "/studyplan" && document.querySelector(".sp-body .state-actions"),
    skeleton: true,
  }, slow);
  // The second time the start page's queries are answered from memory, and its kind has a
  // measured duration: a skeleton only if that was long.
  await watch(page, "the rail: the catalog as it was left" + tag, click(page, '.rail a.nav[data-area="catalog"]'), {
    feedback: () => document.querySelector('.rail a.nav[data-area="catalog"]')?.getAttribute("aria-current") === "page",
    result: () => location.pathname === "/catalog" && location.search.includes("turnus=winter") && document.querySelectorAll(".rows a.row").length > 5,
    skeleton: true,
  }, slow);
  // Nothing stays waiting when two clicks come before the first one has reached the page.
  await watch(page, "two toggles at once" + tag, async () => { await click(page, '#filters a.chip:has-text("Vorlesung")')(); await click(page, '#filters a.chip:has-text("Seminar")')(); }, {
    feedback: () => document.querySelector('#filters a.chip[data-state="with"]'),
    result: () => location.search.includes("form=lecture") && location.search.includes("seminar") && document.querySelectorAll(".rows a.row").length > 0 && !document.querySelector(".list[data-pending]"),
  }, slow);
  // „Passt in meinen Stundenplan": the chip and its tag in the first frame, then what fits the
  // plan (an empty one here), however long the first check of the semester takes.
  await watch(page, "the finder" + tag, click(page, '#filters a.chip:has-text("Passt in meinen Stundenplan")'), {
    feedback: () => document.querySelector('#filters a.chip[data-state="with"]')?.textContent.includes("Passt in meinen Stundenplan") && [...document.querySelectorAll(".tag")].some((t) => t.textContent.includes("Passt in")),
    result: () => location.search.includes("fits=") && document.querySelectorAll(".rows a.row").length > 2 && !document.querySelector(".list[data-pending]"),
  }, slow);
  // „Einplanen" in the preview with the finder on: the button in the first frame; the plan is
  // written after it, and the planned module leaves the list. What stays is what still fits
  // beside it: the lectures of the synthetic snapshot are spread over the week, so a planned
  // module clashes with a few of them only.
  await click(page, ".rows a.row >> nth=2")();
  await page.waitForFunction(() => document.querySelector("#preview .plan-toggle.mark-switch") && location.search.includes("open="), null, { timeout: 15000 }).catch(() => problems.push("the finder: no preview"));
  await page.evaluate(() => { window.__planned = new URL(location.href).searchParams.get("open"); });
  await page.waitForTimeout(250);
  await watch(page, "Einplanen with the finder on" + tag, click(page, "#preview .plan-toggle.mark-switch"), {
    feedback: () => document.querySelector("#preview .plan-toggle.mark-switch")?.getAttribute("aria-pressed") === "true",
    result: () => (localStorage.getItem("betula.studyplan.v1") || "").includes(window.__planned) && !document.querySelector(`.rows a.row[data-id="${window.__planned}"]`) && document.querySelectorAll(".rows a.row").length > 0,
  }, slow);
  check((await page.evaluate(() => document.querySelectorAll(".pending-page, .rows-pending, .sk-detail").length)) === 0, "a skeleton stayed after the last step" + tag);
  await context.close();

  // ---- phone
  const phone = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 });
  const small = await open(phone, "/catalog");
  if (slow > 1) await (await phone.newCDPSession(small)).send("Emulation.setCPUThrottlingRate", { rate: slow });
  await watch(small, "phone: the bottom bar" + tag, tap(small, '.bottomnav a.nav[data-area="programs"]'), {
    feedback: () => document.querySelector('.bottomnav a.nav[data-area="programs"]')?.getAttribute("aria-current") === "page",
    result: () => location.pathname === "/study" && document.querySelector(".st-setup"),
    skeleton: true,
  }, slow);
  await watch(small, "phone: back to the catalog" + tag, tap(small, '.bottomnav a.nav[data-area="catalog"]'), {
    feedback: () => document.querySelector('.bottomnav a.nav[data-area="catalog"]')?.getAttribute("aria-current") === "page",
    result: () => location.pathname === "/catalog" && document.querySelectorAll(".rows a.row").length > 5,
  }, slow);
  // A module: its sheet over the list (2026-10-06), the row marked at once.
  await watch(small, "phone: a module" + tag, tap(small, ".rows a.row >> nth=1"), {
    feedback: () => document.querySelectorAll('.rows a.row[aria-current="true"]').length === 1 || document.querySelector(".detail.is-module"),
    result: () => location.search.includes("open=") && document.querySelector(".detail.is-module h2"),
    skeleton: true,
  }, slow);
  await small.evaluate(() => history.back());
  await small.waitForFunction(() => !location.search.includes("open=") && !document.querySelector(".detail.is-module"), null, { timeout: 8000 }).catch(() => problems.push("phone: Back did not close the module's sheet"));
  await watch(small, "phone: the bottom bar: Stundenplan" + tag, tap(small, '.bottomnav a.nav[data-area="studyplan"]'), {
    feedback: () => document.querySelector('.bottomnav a.nav[data-area="studyplan"]')?.getAttribute("aria-current") === "page",
    result: () => location.pathname === "/studyplan" && document.querySelector(".sp-body .state-actions"),
    skeleton: true,
  }, slow);
  await phone.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
