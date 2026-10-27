// The start page (concept A, owner 2026-10-20: „Ja setz Konzept A jetzt mal in die tat um"): on a
// first visit the setup over the photo of the season, three steps on one card — the program, the
// semester the study began in, the modules of the Fachsemester that follows —, and once a study is
// set up the visitor's day: the greeting, the day in a sentence, the clock of the semester and the
// next thing on the photo, under it the day, the week, the exams, the study, the Merkliste and the
// ways into the catalog. „Studium anlegen" plans the semester's modules and marks those of the
// semesters before as passed, and „Rückgängig" puts back what was; „Erst mal umschauen" shows the
// day without a study. The app keeps the photo of the days ahead, which the server's page then
// shows before the app has started. On a wide screen the page runs under the bar as on a phone,
// the frost behind the bar once it is scrolled. The ground names who took the photo shown. On a phone the photo reaches up under the
// search without the crown's leaves, which slide down the whole way once the photo has left the
// bar, in 150 ms, on a quick swipe as well; once a page is scrolled the frost lies under the
// search, on the start page as on the catalog's. Nothing loads a page. („Über Betula", what the
// start page said until then, is `e2e/about.mjs`.)
// Needs radix serve-snapshot + folia running and a fresh `bash scripts/build-client.sh`.
//   node e2e/home.mjs [base-url]
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge" });
// What the check looks for is on the page in front (on-page.js).
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const errors = [];
const open = async (options, seed = {}) => {
  const context = await browser.newContext({ ...options, timezoneId: "Europe/Berlin" });
  await context.addInitScript(ON_PAGE);
  // What this browser keeps, once, before the first page reads it.
  await context.addInitScript((seed) => {
    if (sessionStorage.getItem("seeded")) return;
    for (const [key, value] of Object.entries(seed)) localStorage.setItem(key, value);
    sessionStorage.setItem("seeded", "1");
  }, seed);
  const page = await context.newPage();
  page.on("pageerror", (error) => errors.push(String(error).slice(0, 300)));
  await page.goto(base + "/", { waitUntil: "load" });
  await page.waitForFunction(() => window.__betulaApp === true && window.betulaAnswered === null, null, { timeout: 60000 });
  // A page load would lose this (the method of spa.mjs).
  await page.evaluate(() => { window.__marker = 1; });
  return { context, page };
};
const stored = (page) => page.evaluate(() => ({
  program: localStorage.getItem("betula.myprogram.v1") || "",
  plan: (localStorage.getItem("betula.studyplan.v1") || "").split("\n").filter(Boolean),
}));

// Without the app (R9): the setup's first step, whose „Studiengang wählen" leads to all programs,
// the day's skeleton beside it for a browser whose head script finds a program kept, the site and
// its search as structured data, who took each photo, and nothing of anybody's day.
const html = await (await fetch(base + "/")).text();
check(/<a (?=[^>]*\bid="ob-program")(?=[^>]*\bhref="\/programs")[^>]*class="combo-trigger"/.test(html), "server HTML: „Studiengang wählen\" of the setup does not lead to the programs");
check(html.includes('class="day-first-wrap"') && html.includes('class="day-sk-wrap"'), "server HTML: not the setup and the day's skeleton");
// (Who took each photo stands in the page's ground and in the app's, which a phone shows.)
check(html.includes('"@type":"WebSite"') && (html.match(/class="photo-credit"/g) || []).length === 10, "server HTML: no WebSite data, or not who took each photo");
check(!html.includes("day-welcome") && !html.includes('class="day-ev'), "server HTML: something of a visitor's day");
check(html.includes('href="/about"') && html.includes('href="/impressum"') && html.includes('href="/datenschutz"'), "server HTML: „Über Betula\" or the legal pages are not linked");

// ---------- a first visit ----------
{
  const { context, page } = await open({ viewport: { width: 1440, height: 900 } });
  const setup = () => page.evaluate(() => ({
    card: Boolean(onPage(".ob")),
    step: onPage(".ob")?.dataset.step,
    title: onPage(".ob-title")?.textContent,
    chips: [...allOnPage(".ob-chip")].map((chip) => chip.textContent.replace(/^\d/, "").trim()),
    brand: Boolean(onPage(".day-brand .wordmark")?.checkVisibility()),
    mine: document.documentElement.classList.contains("mine"),
  }));
  const first = await setup();
  check(first.card && first.step === "1" && first.title === "Was studierst du?" && first.chips.join() === "Studiengang,Beginn,Semester" && first.brand && !first.mine, `the setup does not begin with the program: ${JSON.stringify(first)}`);
  // The photo of the season under it, and in the ground the name of who took it, alone.
  const photo = await page.evaluate(() => {
    const season = document.documentElement.dataset.season;
    const shown = [...allOnPage(".photo-credit")].filter((credit) => credit.checkVisibility()).map((credit) => credit.dataset.photo);
    return { season, image: getComputedStyle(onPage(".day-photo")).backgroundImage, shown };
  });
  check(photo.image.includes(`/assets/season/${photo.season}.webp`) && photo.shown.join() === photo.season, `the photo is not the season's, or the ground names another: ${JSON.stringify(photo)}`);

  // Step 1: the programs in the app's picker; a pick goes on to step 2 at once.
  await page.click("#ob-program >> visible=true");
  await page.waitForSelector("#ob-program-search >> visible=true");
  await page.keyboard.type("Informatik");
  // The picker lists the programs once the data worker has answered for them (`ProgramsAsk`), and
  // Enter takes nothing from a list still empty: Enter once its first entry is the program typed
  // (on the cloud's debug build an Enter right after the typing came first in four runs of five).
  await page.waitForFunction(() => /^Informatik/.test(onPage("#ob-program-option-0")?.textContent.trim() ?? ""), null, { timeout: 10000 }).catch(() => failures.push("the picker does not list Informatik first"));
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => onPage(".ob")?.dataset.step === "2", null, { timeout: 5000 }).catch(() => failures.push("a pick does not lead to the second step"));
  // Step 2: the six semesters back from the current one, the first the usual start, and a field
  // for another Fachsemester, which says what it takes instead of refusing in silence.
  const semesters = await page.evaluate(() => ({
    rows: [...allOnPage(".ob-sem")].map((row) => row.querySelector(".ob-fs")?.textContent.replace(/\s+/g, "")),
    field: Boolean(onPage("#ob-fs")),
    done: onPage(".ob-chip.done > span:last-child")?.textContent.trim(),
  }));
  check(semesters.rows.join() === "1.FS,2.FS,3.FS,4.FS,5.FS,6.FS" && semesters.field && /^Informatik/.test(semesters.done || ""), `the second step is not the semesters: ${JSON.stringify(semesters)}`);
  await page.fill("#ob-fs", "40");
  await page.keyboard.press("Enter");
  check(await page.isVisible(".ob-note.warn >> visible=true"), "a Fachsemester out of range is not said");
  await page.fill("#ob-fs", "3");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => onPage(".ob")?.dataset.step === "3", null, { timeout: 5000 }).catch(() => failures.push("the custom Fachsemester does not lead to the third step"));
  // Step 3: the modules of the third Fachsemester, all ticked, and the switch for the semesters
  // before it on; a row unticked stays out.
  const modules = await page.evaluate(() => ({
    title: onPage(".ob-title")?.textContent,
    rows: allOnPage(".ob-mod").length,
    ticked: allOnPage('.ob-mod[aria-checked="true"]').length,
    past: onPage(".ob-switch")?.getAttribute("aria-checked"),
  }));
  check(modules.title === "Dein 3. Fachsemester" && modules.rows > 1 && modules.ticked === modules.rows && modules.past === "true", `the third step is not the semester's modules: ${JSON.stringify(modules)}`);
  await page.click(".ob-mod >> visible=true >> nth=0");
  check(await page.evaluate(() => onPage(".ob-mod")?.getAttribute("aria-checked") === "false"), "a row does not untick");
  await page.click(".ob-create >> visible=true");
  await page.waitForSelector(".day-welcome >> visible=true", { timeout: 5000 }).catch(() => failures.push("„Studium anlegen\" leads nowhere"));
  const made = await stored(page);
  const planned = made.plan.filter((line) => line.startsWith("m\t") || line.startsWith("p\t"));
  const passed = made.plan.filter((line) => line.startsWith("d\t") || line.startsWith("q\t"));
  check(/^program\t079-82-2008$/m.test(made.program) && /^start\t\d{4}[WS]$/m.test(made.program), `the study is not kept: ${made.program}`);
  check(passed.length > 0 && planned.length >= modules.rows - 1 + passed.length, `the semesters before are not passed, or the semester not planned: ${JSON.stringify(made.plan)}`);
  // The day's parts come with their answers (the semester's facts, the Stundenplan's Termine).
  await page.waitForFunction(() => allOnPage(".day-clock-row > i").length > 5 && allOnPage(".day-head h2").length >= 4, null, { timeout: 15000 }).catch(() => {});
  const day = await page.evaluate(() => ({
    hero: Boolean(onPage(".day-hero:not(.day-first) h1")),
    clock: allOnPage(".day-clock-row > i").length,
    panels: [...allOnPage(".day-head h2")].map((head) => head.textContent),
    mine: document.documentElement.classList.contains("mine"),
    marker: window.__marker === 1 && location.pathname === "/",
  }));
  check(day.hero && day.clock > 5 && day.panels.length >= 4 && day.mine && day.marker, `the day does not follow the setup: ${JSON.stringify(day)}`);
  // The panels under the photo in two columns that end on one line, whatever the day holds (owner,
  // 2026-10-27: „dass gerne mal riesige Lücken in dem UI sind … so dass das relativ uniform ist").
  const ends = await page.evaluate(() => [...allOnPage(".day-cols > .day-col")].map((col) => Math.round(col.lastElementChild?.getBoundingClientRect().bottom ?? -1)));
  check(ends.length === 2 && ends[0] > 0 && ends[0] === ends[1], `wide screen: the columns of panels under the photo do not end on one line: ${ends}`);
  // The day at once (owner, 2026-10-26: „so dass eine person die nur wissen will was als nächstes
  // ansteht das sofort sieht"): the app keeps the photo of the days ahead, and the server's page
  // puts it in before the app has started, as the app's page shows it but for what goes by the
  // minute; in another language, where it was not made, the skeleton stays.
  await page.waitForFunction(() => Boolean(localStorage.getItem("betula.today.v1")), null, { timeout: 15000 }).catch(() => failures.push("the start page does not keep the days ahead"));
  // The photo as it reads but for what goes by the minute (`data-count`), and for Leptos's marks.
  const bare = (hero) => {
    if (!hero) return null;
    const copy = hero.cloneNode(true);
    copy.querySelectorAll("[data-count]").forEach((part) => (part.dataset.count === "fill" ? part.removeAttribute("style") : (part.textContent = "")));
    return copy.outerHTML.replace(/<!---->/g, "");
  };
  const app = await page.evaluate(`(${bare})(onPage(".day-hero:not(.day-first)"))`);
  const alone = await context.newPage();
  await alone.route("**/assets/boot.js*", (route) => route.abort());
  for (const [path, lang] of [["/", "de"], ["/en", "en"]]) {
    await alone.goto(base + path, { waitUntil: "load" });
    const server = await alone.evaluate(`({ hero: (${bare})(document.querySelector(".day-sk-wrap .day-hero:not(.day-hero-sk)")), shown: Boolean(document.querySelector(".day-sk-wrap .day-hero")?.checkVisibility()), skeleton: Boolean(document.querySelector(".day-hero-sk")), line: document.querySelector(".day-sk-wrap [data-count=line]")?.textContent ?? null, hidden: Boolean(document.querySelector(".day-sk-wrap .day-hero:not(.day-hero-sk)")?.closest('[aria-hidden="true"]')) })`);
    if (lang === "de") check(server.hero === app && server.shown && !server.skeleton && server.line !== "" && !server.hidden, `the server's page does not put in the app's day at once, or hides it from screen readers: ${JSON.stringify({ server, app })}`);
    else check(server.skeleton && !server.hero, `the day in another language than it was made in: ${JSON.stringify(server)}`);
  }
  await alone.close();
  // On a wide screen the start page runs under the bar as on a phone, and the frost lies behind the
  // bar once it is scrolled (owner, 2026-10-26: „Mach das mal so wie bei der mobile version");
  // at its top the frost is away and the page stands where it stood. Its photo as on a phone
  // (owner, the same day: „Mach das mal ordentlich … die animation für die Blätter", „den hero
  // banner beim Pc einfach auch bis ganz nach oben"): up to the window's edge behind the bar, its
  // greeting where it stood, the bar's words light and no leaves over it; scrolled, the blur
  // alone; past it the leaves and the frost's colour.
  const under = async (y) => {
    await page.evaluate((y) => document.querySelector("#content > .page-stage.front > .work.flowing").scrollTo(0, y), y);
    // The frost follows in the next frame and fades in a fifth of a second: a frame can be slow
    // on a busy machine.
    await page.waitForFunction((down) => document.documentElement.hasAttribute("data-scrolled") === down, y > 4, { timeout: 3000 }).catch(() => {});
    await page.waitForTimeout(400);
    return page.evaluate(() => {
      const frost = document.querySelector("body > .frost");
      return {
        frost: getComputedStyle(frost).opacity,
        tint: getComputedStyle(frost, "::after").opacity,
        leaves: getComputedStyle(document.querySelector("body > .crown")).getPropertyValue("--leaves").trim(),
        word: getComputedStyle(document.querySelector(".topbar .crumb.brand .wordmark")).color,
        hero: Math.round(document.querySelector("#content > .page-stage.front .day-hero:not(.day-first)").getBoundingClientRect().top),
        // The photo and the frost across the whole width, from the rail to the window's edge.
        across: [document.querySelector("#content > .page-stage.front .day-hero:not(.day-first)"), frost].map((el) => { const r = el.getBoundingClientRect(); return [Math.round(r.left), Math.round(r.right)].join(); }).every((x) => x === `${Math.round(document.querySelector(".rail").getBoundingClientRect().right)},${innerWidth}`),
        greet: Math.round(document.querySelector("#content > .page-stage.front .day-hero:not(.day-first) .day-greet").getBoundingClientRect().top),
      };
    });
  };
  const bars = [await under(120), await under(560), await under(0)];
  const [onPhoto, past, top] = bars;
  check(onPhoto.frost === "1" && onPhoto.hero < 0 && top.frost === "0" && top.hero === 0 && top.greet >= 56 + 24, `wide screen: the start page does not run under the bar with the frost behind it, or its photo does not reach up to the window's edge with its greeting below the bar: ${JSON.stringify(bars)}`);
  check(top.across && onPhoto.across, `wide screen: the photo or the frost does not reach from the rail to the window's edge: ${JSON.stringify(bars)}`);
  check(onPhoto.tint === "0" && onPhoto.leaves === "0" && onPhoto.word === "rgb(255, 255, 255)" && top.leaves === "0" && top.word === "rgb(255, 255, 255)", `wide screen: over the photo the bar is not the photo's (the blur alone, no leaves, its words light): ${JSON.stringify(bars)}`);
  check(past.frost === "1" && past.tint === "1" && past.leaves === "1" && past.word !== "rgb(255, 255, 255)", `wide screen: past the photo the leaves and the frost's colour did not come, or the bar's words stayed light: ${JSON.stringify(bars)}`);
  // Whatever the system's setting on motion (owner, 2026-10-26: „das mit weniger bewegung … sollte
  // nicht dazu gehören"), a jump far past the photo, as Bild ↓ makes it, brings the leaves in 150 ms
  // at the quickest: on their way after 60 ms, there after 400 („nicht sofort da …
  // vielleicht so 100-200ms").
  await page.emulateMedia({ reducedMotion: "reduce" });
  const jump = await page.evaluate(async () => {
    const area = document.querySelector("#content > .page-stage.front > .work.flowing");
    const crown = document.querySelector("body > .crown");
    const leaves = () => Number(getComputedStyle(crown).getPropertyValue("--leaves"));
    area.scrollTo(0, 0);
    await new Promise((resolve) => setTimeout(resolve, 400));
    const t0 = performance.now();
    area.scrollTo(0, 900);
    const seen = [leaves()];
    for (const ms of [60, 400]) {
      await new Promise((resolve) => setTimeout(resolve, Math.max(0, ms - (performance.now() - t0))));
      seen.push(leaves());
    }
    return seen;
  });
  await page.emulateMedia({ reducedMotion: null });
  check(jump[0] === 0 && jump[1] > 0 && jump[1] < 1 && jump[2] === 1, `wide screen: the leaves do not slide in on a jump past the photo, where less motion is wanted as well: ${JSON.stringify(jump)}`);
  await page.evaluate(() => document.querySelector("#content > .page-stage.front > .work.flowing").scrollTo(0, 0));
  await page.waitForTimeout(400);
  // „Rückgängig": the setup again, and what this browser kept before.
  await page.click(".day-welcome button:has-text('Rückgängig') >> visible=true");
  await page.waitForSelector(".ob >> visible=true", { timeout: 5000 }).catch(() => failures.push("„Rückgängig\" does not bring the setup back"));
  const undone = await stored(page);
  check(!/^program\t/m.test(undone.program) && undone.plan.length === 0 && !(await page.evaluate(() => document.documentElement.classList.contains("mine"))), `„Rückgängig\" leaves something: ${JSON.stringify(undone)}`);
  // „Erst mal umschauen": the day without a study, and the way back to the setup on it.
  await page.click("button.ob-link >> visible=true");
  await page.waitForSelector(".day-hero:not(.day-first) >> visible=true", { timeout: 5000 }).catch(() => failures.push("„Erst mal umschauen\" does not show the day"));
  check(await page.evaluate(() => Boolean(onPage(".day-next .btn")) && allOnPage(".day-ev").length === 0), "a day without a study shows Termine, or no way back to the setup");
  await page.click(".day-next .btn >> visible=true");
  await page.waitForSelector(".ob >> visible=true", { timeout: 5000 }).catch(() => failures.push("„Studium einrichten\" does not bring the setup back"));
  check(await page.evaluate(() => window.__marker === 1 && location.pathname === "/"), "the setup loaded a page or left the start page");
  await context.close();
}

// ---------- a phone that keeps a study ----------
{
  const seed = {
    "betula.myprogram.v1": "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2026W\n",
  };
  const { context, page } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true }, seed);
  const top = await page.evaluate(() => {
    const hero = onPage(".day-hero").getBoundingClientRect(), bar = onPage(".topbar .search").getBoundingClientRect();
    return { under: hero.top <= 0 && hero.bottom > bar.bottom, wide: Math.round(hero.width) === innerWidth };
  });
  check(top.under && top.wide, `phone: the photo does not reach up under the search: ${JSON.stringify(top)}`);
  // One column: the panels in the order they are read, „Mein Studium" and the Merkliste last,
  // whichever column a wide screen gives them.
  await page.waitForFunction(() => allOnPage(".day-cols .day-head h2").length >= 3, null, { timeout: 15000 }).catch(() => {});
  const read = await page.evaluate(() => [...allOnPage(".day-cols .day-head h2")].sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top).map((head) => head.textContent));
  check(read.slice(-2).join(" · ") === "Mein Studium · Merkliste", `phone: the panels under the photo are not in the order they are read: ${read.join(" · ")}`);
  // The bar: how far the leaves have come down (`--leaves`, 0 to 1) and where they stand, the frost
  // under them and its colour, and the crown's box whatever they do.
  const bar = () => page.evaluate(() => {
    const crown = document.querySelector("body > .crown"), frost = document.querySelector("body > .frost");
    return {
      frost: getComputedStyle(frost).opacity,
      tint: getComputedStyle(frost, "::after").opacity,
      leaves: getComputedStyle(crown).getPropertyValue("--leaves").trim(),
      at: getComputedStyle(crown, "::before").translate,
      height: Math.round(crown.getBoundingClientRect().height),
    };
  });
  const settle = () => page.waitForTimeout(800);
  // The window as far down as the photo's end lies `past` px above the bar's.
  const pastPhoto = (past) => page.evaluate((past) => {
    const hero = document.querySelector("#content > .page-stage.front .day-hero"), crown = document.querySelector("body > .crown");
    window.scrollTo(0, hero.getBoundingClientRect().bottom + scrollY - crown.getBoundingClientRect().bottom + past);
  }, past);
  const still = await bar();
  check(still.frost === "0" && still.leaves === "0" && still.tint === "0" && still.at === "0px -100%", `phone: the photo has leaves or a frost over it at the top: ${JSON.stringify(still)}`);
  // A little down, the photo still under the search: the frost without its colour, no leaves yet.
  await page.evaluate(() => window.scrollTo(0, 200));
  await settle();
  const down = await bar();
  check(down.frost === "1" && down.leaves === "0" && down.tint === "0" && down.height === still.height, `phone: no frost over the photo once scrolled, or leaves over it: ${JSON.stringify(down)}`);
  // A breaking point, not the scroll (owner, 2026-10-26: „dass die Animation nicht mit dem scrollen
  // mitläuft, sondern dass es einen breaking point gibt, wo die dann umschaltet"): the photo's end
  // 8 px short of the bar's, no leaves yet; 4 px past it, the leaves and the frost's colour the
  // whole way.
  await pastPhoto(-8);
  await settle();
  const short = await bar();
  await pastPhoto(4);
  await settle();
  const over = await bar();
  check(short.leaves === "0" && short.tint === "0" && over.leaves === "1" && over.tint === "1" && over.height === still.height, `phone: the leaves do not switch where the photo leaves the bar: ${JSON.stringify([short, over])}`);
  // Past the photo: the leaves of every page, the frost in its colour.
  await pastPhoto(120);
  await settle();
  const past = await bar();
  check(past.frost === "1" && past.leaves === "1" && past.tint === "1" && past.at === "0px 0%" && past.height === still.height, `phone: the leaves do not come once the photo has left the bar: ${JSON.stringify(past)}`);
  await page.evaluate(() => window.scrollTo(0, 0));
  await settle();
  const back = await bar();
  check(back.frost === "0" && back.leaves === "0", `phone: the frost or the leaves stay at the top: ${JSON.stringify(back)}`);
  // A quick swipe from the top far past the photo: the leaves come no faster than their most (the
  // whole way in 150 ms), on their way after 80 ms, there after 600.
  const swipe = await page.evaluate(async () => {
    const crown = document.querySelector("body > .crown");
    const leaves = () => Number(getComputedStyle(crown).getPropertyValue("--leaves"));
    const t0 = performance.now();
    window.scrollTo(0, 2400);
    const seen = [];
    for (const ms of [80, 600]) {
      await new Promise((resolve) => setTimeout(resolve, Math.max(0, ms - (performance.now() - t0))));
      seen.push(leaves());
    }
    return seen;
  });
  check(swipe[0] > 0 && swipe[0] < 0.75 && swipe[1] === 1, `phone: the leaves pop in on a quick swipe, or do not come: ${JSON.stringify(swipe)}`);
  await page.evaluate(() => window.scrollTo(0, 0));
  await settle();
  check(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "phone: the start page scrolls sideways");
  // The ground a phone shows (the app's, under every page) names who took the photo shown.
  const credit = await page.evaluate(() => [...document.querySelectorAll("body > .ground .photo-credit")].filter((line) => line.checkVisibility()).map((line) => line.dataset.photo));
  check(credit.length === 1, `phone: the ground does not name who took the photo: ${JSON.stringify(credit)}`);
  // Not under another page.
  await page.click('.bottomnav a[data-area="catalog"]');
  await page.waitForFunction(() => location.pathname === "/catalog", null, { timeout: 10000 }).catch(() => {});
  await page.waitForTimeout(500);
  check(await page.evaluate(() => [...document.querySelectorAll(".photo-credit")].every((line) => !line.checkVisibility())), "phone: the photo's credit stays under another page");
  // Another page: the leaves at its top, the frost in its colour once it is scrolled.
  await settle();
  const catalog = await bar();
  await page.evaluate(() => window.scrollTo(0, 600));
  await settle();
  const list = await bar();
  check(catalog.leaves === "1" && catalog.at === "0px 0%" && catalog.frost === "0" && list.frost === "1" && list.leaves === "1" && list.tint === "1", `phone: the catalog's bar is not the leaves with the frost under them: ${JSON.stringify([catalog, list])}`);
  await context.close();
}

check(errors.length === 0, `page errors: ${errors.join(" | ")}`);
await browser.close();
if (failures.length) {
  console.error(failures.map((f) => "FAIL " + f).join("\n"));
  process.exit(1);
}
console.log("home: ok");
