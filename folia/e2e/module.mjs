// Checks a module in its two sizes, on the desktop and on a phone.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node module.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Desktop: preview → full page keeps the frame (sidebar exactly where the filter panel was), the
// sidebar jumps to sections without history entries, Esc goes back to the list at the row.
// Phone: a tap opens the module's page directly (never the preview), the page has the order of the
// preview (times and facts, then the description), back returns to the tapped row, and a shared
// link with a preview becomes the page.
// A shared link with a preview of a module far down the whole list, loaded directly, on both: the
// page answers all the while, the list renders a few screens of rows, the module is beside the
// list (a sheet on a phone) with its row in view and marked.
// „Einplanen" and „Merken": one pair at the right end of the line of the badges, in the order of
// the Tab key, side by side or, where the line has no room for that, one over the other beside
// the badges (a line of their own only on a phone); „Einplanen" keeps its width when pressed; what
// the plan adds to it shows, and on the module's page the heading is as tall as the server's.
// On a phone (owner, 2026-10-17): the Termine under the week folded, a slot tapped unfolds them and
// marks its line in sight (the sheet half up going all up), the facts two side by side,
// „Studiengänge" last with five curricula and „Alle … anzeigen".
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
// What the check looks for is on the page in front (on-page.js): every tab's page is built behind
// it at the start (folia/crates/shell/src/stage.rs, `prebuild`), with parts of the same name.
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const newContext = browser.newContext.bind(browser);
browser.newContext = async (options) => { const context = await newContext(options); await context.addInitScript(ON_PAGE); return context; };
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };

const open = async (options, path) => {
  const context = await browser.newContext(options);
  // „Einplanen" plans at once: the question for a study not set up is put aside for the session
  // (plannote.mjs checks the question).
  await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  await page.evaluate(() => { window.__marker = 1; });
  const step = async (name, action, until, arg) => {
    const started = Date.now();
    await action();
    try {
      await page.waitForFunction(until, arg, { timeout: 8000 });
      // The page before stays as a picture in front of the new one while its answers come (pending.rs, `hold`).
      await page.waitForFunction(() => !onPage(".pending-page.held"), null, { timeout: 2000 });
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    timings[name] = Date.now() - started;
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
  };
  return { page, step, context };
};
const order = (page, root) => page.evaluate((selector) => [...allOnPage(`${selector} .section > .label`)].map((label) => label.firstChild.textContent), root);

// ---------- desktop ----------
{
  const { page, step, context } = await open({ viewport: { width: 1500, height: 900 } }, "/catalog?program=bachelor-informatik-2008");
  await page.evaluate(() => { onPage(".rows").scrollTop = 1200; });
  await page.waitForTimeout(250); // the virtual list renders the rows of the new position on the next frame
  const id = await page.evaluate(() => { const rows = onPage(".rows").getBoundingClientRect(); return [...allOnPage(".rows a.row")].find((r) => r.getBoundingClientRect().top > rows.top + 200).dataset.id; });
  await step("preview", () => page.click(`a.row[data-id="${id}"]`), () => Boolean(onPage(".detail h2")));
  const previewOrder = await order(page, ".detail");
  const filters = await page.evaluate(() => { const r = onPage("#filters").getBoundingClientRect(); return [r.left, r.width, r.top]; });
  await step("full page", () => page.keyboard.press("f"), () => location.pathname.startsWith("/catalog/module/") && onPage(".module-page h2") && onPage("#sidebar"));
  const sidebar = await page.evaluate(() => { const r = onPage("#sidebar").getBoundingClientRect(); return [r.left, r.width, r.top]; });
  check(JSON.stringify(sidebar) === JSON.stringify(filters), `the sidebar is not where the filter panel was: ${sidebar} instead of ${filters}`);
  check(JSON.stringify(await order(page, ".module-page")) === JSON.stringify(previewOrder), `the page and the preview differ in order: ${await order(page, ".module-page")} / ${previewOrder}`);
  const columns = await page.evaluate(() => { const [side, main] = [onPage(".module-grid > aside"), onPage(".module-grid > div")].map((el) => el.getBoundingClientRect()); return main.left < side.left && Math.abs(main.top - side.top) < 2; });
  check(columns, "wide page: the description is not on the left of the facts");

  // The sidebar lists the sections that exist, and jumps to them without a history entry.
  const toc = await page.evaluate(() => [...allOnPage(".toc a")].map((a) => [a.textContent, Boolean(onPage(a.getAttribute("href")))]));
  check(toc.length >= 3 && toc.every(([, exists]) => exists), `the sidebar names sections that are not there: ${JSON.stringify(toc)}`);
  const entries = await page.evaluate(() => history.length);
  await page.click('.toc a[href="#studiengaenge"] >> visible=true');
  await page.waitForTimeout(700);
  const jumped = await page.evaluate(() => { const top = onPage("#studiengaenge").getBoundingClientRect().top; return top < innerHeight - 40 && location.hash === "" && top >= 0; });
  check(jumped && (await page.evaluate(() => history.length)) === entries, "the sidebar's jump did not show the section, or left a history entry");

  // The handle of the sidebar is the handle of the filter panel: one width for both.
  const edge = await page.locator('[data-action="resize-filters"] >> visible=true').boundingBox();
  await page.mouse.move(edge.x + 6, edge.y + 300);
  await page.mouse.down();
  await page.mouse.move(edge.x + 46, edge.y + 300, { steps: 5 });
  await page.mouse.up();
  check(Math.abs((await page.evaluate(() => onPage("#sidebar").getBoundingClientRect().width)) - (sidebar[1] + 40)) <= 2, "the sidebar's width does not follow its handle");

  await step("Esc goes back to the list", () => page.keyboard.press("Escape"), () => location.pathname === "/catalog" && location.search.includes("open=") && onPage(".rows a.row"));
  await page.waitForTimeout(400);
  const back = await page.evaluate((id) => { const row = onPage(`a.row[data-id="${id}"]`)?.getBoundingClientRect(); const rows = onPage(".rows").getBoundingClientRect(); return row ? row.top >= rows.top && row.bottom <= rows.bottom : null; }, id);
  check(back === true, `back on the list the row of the module is ${back === null ? "not loaded" : "not in view"}`);
  check(Math.abs((await page.evaluate(() => onPage("#filters").getBoundingClientRect().width)) - (sidebar[1] + 40)) <= 2, "the filter panel does not have the width the sidebar was given");
  await page.evaluate(() => localStorage.removeItem("betula.filters.width"));
  await context.close();
}

// ---------- exam dates the BTU cannot mean (folia/crates/timetable/src/exam_reading.rs) ----------
// Analysis I (11103) lists QIS's placeholder „So 01:00–02:30, 27.12.2015" twice; 12000 has a
// deadline at 23:45–24:00. In the app's preview and in the server's page without JavaScript.
{
  const exams = (page) => page.evaluate(() => {
    const section = onPage("#pruefungstermine");
    return section && {
      when: [...section.querySelectorAll(".ev .when")].map((w) => [w.textContent, w.getAttribute("title")]),
      odd: [...section.querySelectorAll(".ev .odd")].map((o) => o.textContent),
      note: section.querySelector(".note")?.textContent ?? "",
    };
  });
  const expect = (where, seen) => {
    if (!seen) return problems.push(`exams (${where}): 11103 shows no exam dates`);
    check(seen.when.every(([when]) => !when.includes("01:00")), `exams (${where}): the placeholder is shown as a time: ${JSON.stringify(seen.when)}`);
    check(seen.odd.length === 0 || seen.note.includes("Platzhalter"), `exams (${where}): marked rows without the note: ${JSON.stringify(seen)}`);
    check(seen.odd.some((line) => line.includes("In QIS: So 01:00–02:30 · 27.12.2015")) && seen.when.every(([when]) => when === "Termin offen"), `exams (${where}): 11103 does not read as „Termin offen" with the original beside it: ${JSON.stringify(seen)}`);
  };
  const { page, context } = await open({ viewport: { width: 1500, height: 900 } }, "/catalog?q=analysis&open=11103");
  await page.waitForSelector("#preview #pruefungstermine >> visible=true", { timeout: 8000 }).catch(() => {});
  expect("preview", await exams(page));
  await context.close();

  const plain = await browser.newContext({ viewport: { width: 1500, height: 900 }, javaScriptEnabled: false });
  const server = await plain.newPage();
  await server.goto(base + "/catalog/module/11103");
  expect("without JavaScript", await exams(server));
  await server.goto(base + "/catalog/module/12000");
  const deadline = await exams(server);
  check(deadline?.when.some(([when, title]) => when === "So bis 24:00" && title === "In QIS: So 23:45–24:00") && deadline.odd.length === 0, `exams: the deadline of 12000 does not read „So bis 24:00", unmarked: ${JSON.stringify(deadline)}`);
  await plain.close();
}

// ---------- „Einplanen" beside „Merken" ----------
// Datenbanken (12330) has four badges, which leave the preview no room for the pair side by side:
// there the two stand one over the other beside them, and on a phone they take a line of their
// own and fill it, half each (owner, 2026-09-27). A plan in another semester and the finder's
// placeholder make the switch say more.
{
  const PLAN = "m\t2027S\t12330\t1790000000\t\np\t3\t2026W\t079-82-2008\t17\t1-1\t6\tfues\t\tFachübergreifendes Studium\n";
  const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };
  const pair = (page, root, body) => page.evaluate(([root, body]) => {
    const line = onPage(`${root} .hero-line`);
    // Each switch with its arrow (folia/design/tasten/konzept.html, „Knopf mit Pfeil"): the pair of
    // „Einplanen" (`PlanButton`) and that of „Merken" (`MarkButton`).
    const [plan, mark, edge, toggle] = [line.querySelector(".plan-pair") ?? line.querySelector(".plan-toggle"), line.querySelector(".mark-pair:not(.plan-pair)") ?? line.querySelector(".mark-toggle"), onPage(body), line.querySelector(".plan-toggle")].map((el) => el.getBoundingClientRect());
    const badges = [...line.querySelectorAll(".badges > .badge")].map((el) => el.getBoundingClientRect());
    const middle = (r) => (r.top + r.bottom) / 2;
    return {
      together: Math.abs(middle(plan) - middle(mark)),
      gap: mark.left - plan.right,
      // One over the other: „Merken" 6px under „Einplanen", their right edges in line.
      under: mark.top - plan.bottom,
      aligned: Math.abs(mark.right - plan.right),
      edge: Math.abs(mark.right - edge.right),
      // Right of every badge, „Einplanen" on the middle of the credits; or under all of them.
      beside: plan.left >= Math.max(...badges.map((b) => b.right)) + 6 && Math.abs(middle(plan) - middle(badges[0])) <= 0.5,
      below: plan.top >= Math.max(...badges.map((b) => b.bottom)),
      // On a line of their own the two fill it: „Einplanen" starts where the badges start, and
      // both are as wide.
      fill: Math.abs(plan.left - badges[0].left),
      halves: Math.abs(plan.width - mark.width),
      tab: Boolean(line.querySelector(".plan-toggle").compareDocumentPosition(line.querySelector(".mark-toggle")) & Node.DOCUMENT_POSITION_FOLLOWING),
      note: line.querySelector(".plan-toggle > small")?.checkVisibility() ? line.querySelector(".plan-toggle > small").textContent : null,
      label: line.querySelector(".plan-toggle > span").textContent,
      width: toggle.width,
    };
  }, [root, body]);
  const sideBySide = (seen) => seen.together <= 0.5 && Math.abs(seen.gap - 6) <= 0.5;
  const stacked = (seen) => Math.abs(seen.under - 6) <= 0.5 && seen.aligned <= 0.5;
  // Beside the badges on a wide screen, never on a line of their own; under them on a phone,
  // filling that line.
  const filled = (seen) => seen.below && sideBySide(seen) && seen.fill <= 0.5 && seen.halves <= 0.5;
  const expect = (where, seen, onPhone = false) => check((sideBySide(seen) || stacked(seen)) && seen.edge <= 0.5 && (onPhone ? filled(seen) : seen.beside) && seen.tab, `${where}: „Einplanen" and „Merken" are not one pair at the right end${onPhone ? ", filling their line" : ""}, in order: ${JSON.stringify(seen)}`);
  const preview = ["#preview", "#preview .dbody .section"];
  const onPage = [".module-page", ".module-grid > aside .section"];

  for (const width of [960, 1100, 1500]) {
    const { page, step, context } = await open({ viewport: { width, height: 900 } }, "/catalog?q=datenbanken&open=12330");
    await page.waitForSelector("#preview .plan-toggle >> visible=true", { timeout: 8000 }).catch(() => problems.push(`preview at ${width}px: no „Einplanen"`));
    const before = await pair(page, ...preview);
    expect(`preview at ${width}px`, before);
    check(stacked(before), `preview at ${width}px: the four badges leave no room for the pair side by side, and yet the two do not stand one over the other: ${JSON.stringify(before)}`);
    if (width === 1500) {
      await page.evaluate(() => document.activeElement?.blur());
      await step("„Einplanen“ plans", () => page.click("#preview .plan-toggle >> visible=true"), () => onPage("#preview .plan-toggle").getAttribute("aria-pressed") === "true" && !onPage("#preview .plan-toggle").hasAttribute("aria-busy"));
      const pressed = await pair(page, ...preview);
      check(pressed.label === "Eingeplant" && Math.abs(pressed.width - before.width) < 0.5, `„Einplanen“ changes its width when pressed: ${before.width} → ${JSON.stringify(pressed)}`);
      await step("and a second click takes it out", () => page.click("#preview .plan-toggle >> visible=true"), () => onPage("#preview .plan-toggle").getAttribute("aria-pressed") === "false" && !localStorage.getItem("betula.studyplan.v1"));
    }
    await step("full page", () => page.click('#preview [data-action="fullscreen"] >> visible=true'), () => location.pathname === "/catalog/module/12330" && onPage(".module-page .plan-toggle"));
    expect(`the module's page at ${width}px`, await pair(page, ...onPage));
    await context.close();
  }

  // With a plan: in the preview the switch says what the plan adds; on the page the sidebar says
  // it, and the heading keeps the server's height.
  const planned = async (options, path) => {
    const context = await browser.newContext(options);
    // „Einplanen" plans at once: the question for a study not set up is put aside for the session
    // (plannote.mjs checks the question).
    await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
    await context.addInitScript((plan) => localStorage.setItem("betula.studyplan.v1", plan), PLAN);
    const page = await context.newPage();
    page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
    await page.goto(base + path, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
    return { page, context };
  };
  const server = async (options, path) => {
    const context = await browser.newContext(options);
    // „Einplanen" plans at once: the question for a study not set up is put aside for the session
    // (plannote.mjs checks the question).
    await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
    await context.route("**/pkg/**", (route) => route.abort());
    const page = await context.newPage();
    await page.goto(base + path, { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    const hero = await page.evaluate(() => onPage(".module-page > .hero").getBoundingClientRect().height);
    await context.close();
    return hero;
  };
  {
    const { page, context } = await planned({ viewport: { width: 1500, height: 900 } }, "/catalog?q=datenbanken&fill=p3&open=12330");
    await page.waitForFunction(() => onPage("#preview .plan-toggle > small"), null, { timeout: 8000 }).catch(() => {});
    const seen = await pair(page, ...preview);
    expect("preview with a plan", seen);
    // (The finder hands its placeholder to the preview once it has one; the other semester shows now.)
    check(seen.label === "Einplanen" && Boolean(seen.note?.endsWith("geplant: SoSe 27")), `preview with a plan: the switch says ${JSON.stringify(seen)}`);
    await context.close();
  }
  for (const [where, options] of [["1000px", { viewport: { width: 1000, height: 900 } }], ["phone", phone]]) {
    const path = "/catalog/module/12330?plan=2026W&fill=p3";
    const { page, context } = await planned(options, path);
    await page.waitForFunction(() => onPage(".sidebar .plan-toggle small")?.textContent.includes("geplant"), null, { timeout: 8000 }).catch(() => {});
    await page.evaluate(() => document.fonts.ready);
    const seen = await pair(page, ...onPage);
    expect(`the module's page with a plan (${where})`, seen, where === "phone");
    const side = await page.evaluate(() => onPage(".sidebar .plan-toggle small")?.textContent);
    check(side === "WiSe 2026/27 · für „Fachübergreifendes Studium“ · geplant: SoSe 2027", `the module's page with a plan (${where}): the sidebar says ${side}`);
    // The switch's small line, on a phone its second line: where it plans first (the semester the
    // page means), then the placeholder.
    check(where === "phone" ? seen.below && seen.note?.startsWith("WiSe 26/27 · für „Fach") : seen.note === null, `the module's page with a plan (${where}): the switch says ${JSON.stringify(seen)}`);
    const hero = await page.evaluate(() => onPage(".module-page > .hero").getBoundingClientRect().height);
    const before = await server(options, path);
    check(Math.abs(hero - before) < 0.5, `the module's page with a plan (${where}): the heading is ${before}px before the takeover and ${hero}px after`);
    await context.close();
  }
}

// ---------- phone ----------
{
  const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };
  const { page, step, context } = await open(phone, "/catalog");
  // Deep in the virtual list (the 89th row, on its second page), so that coming back has
  // something to prove.
  await page.evaluate(() => { const list = onPage(".vlist"); scrollTo(0, list.getBoundingClientRect().top + scrollY + 88 * 88); });
  await page.waitForFunction(() => onPage('.vrow[data-i="88"] a.row'), null, { timeout: 8000 }).catch(() => problems.push("phone: the list did not render its 89th row"));
  const id = await page.evaluate(() => { const row = onPage('.vrow[data-i="88"] a.row'); row.scrollIntoView({ block: "center" }); return row.dataset.id; });
  await page.waitForTimeout(300);
  // The module comes up as a sheet over the list (owner, 2026-10-06), in the order of its page.
  await step("phone: a tap opens the module's sheet", () => page.tap(`a.row[data-id="${id}"]`), (id) => location.pathname === "/catalog" && location.search.includes(`open=${id}`) && onPage(".detail.is-module h2"), id);
  const sheetOrder = await order(page, ".detail.is-module");
  check(sheetOrder[0] === "Termine" && sheetOrder.indexOf("Auf einen Blick") < sheetOrder.indexOf("Inhalte"), `phone: the sheet does not start with the times and facts: ${sheetOrder}`);
  // No „Vollbild" on the sheet (owner, 2026-10-16: „Vollbild als knopf ergibt keinen sinn, der muss
  // raus"): all up it is the whole screen. The module's page, where a link leads (a shared one, the
  // search), has its sidebar as a block of actions under it.
  await page.waitForTimeout(400);
  check(await page.evaluate(() => getComputedStyle(onPage('.detail.is-module [data-action="fullscreen"]')).display === "none"), "phone: the module's sheet shows „Vollbild“");
  await step("phone: a link to the module's page", () => page.evaluate((id) => { const a = Object.assign(document.createElement("a"), { href: `/catalog/module/${id}` }); document.body.append(a); a.click(); a.remove(); }, id), (id) => location.pathname === `/catalog/module/${id}` && onPage(".module-page h2"), id);
  const phoneOrder = await order(page, ".module-page");
  check(phoneOrder[0] === "Termine" && phoneOrder.indexOf("Auf einen Blick") < phoneOrder.indexOf("Inhalte") , `phone: the page does not start with the times and facts: ${phoneOrder}`);
  check(await page.evaluate(() => { const side = onPage("#sidebar").getBoundingClientRect(); const article = onPage(".module-page").getBoundingClientRect(); return side.top >= article.bottom - 1 && getComputedStyle(onPage(".toc")).display === "none"; }), "phone: the sidebar is not a block of actions under the module");
  // „Zurück" is the box at the head, left of the search (owner, 2026-10-15: „mach den
  // zurückbutton mal überall nach oben neben die suche"); the page's own link steps aside and the
  // way to the profile page gives way.
  check(await page.evaluate(() => Boolean(onPage(".topbar .top-back.on")) && !onPage('.module-page [data-action="back"]')?.offsetHeight && onPage(".topbar .top-profile").inert), "phone: the module's page has no „Zurück\" at the head");
  await step("phone: back returns to the list with the sheet", () => page.tap(".top-back.on >> visible=true"), (id) => location.pathname === "/catalog" && location.search.includes(`open=${id}`) && onPage(".detail.is-module h2") && !onPage(".top-back.on"), id);
  await page.waitForTimeout(500);
  await step("phone: the sheet closed", () => page.tap('.detail.is-module [data-action="close-detail"] >> visible=true'), () => !location.search.includes("open=") && !onPage(".detail.is-module") && onPage(".rows a.row"));
  await page.waitForTimeout(500);
  const seen = await page.evaluate((id) => { const row = onPage(`a.row[data-id="${id}"]`)?.getBoundingClientRect(); return row ? row.top >= 0 && row.bottom <= innerHeight : null; }, id);
  check(seen === true, `phone: back on the list the tapped row is ${seen === null ? "not loaded" : "not in view"}`);

  // A shared link with a preview is the list with the module's sheet on a phone.
  const shared = await context.newPage();
  await shared.goto(base + "/catalog?turnus=winter&open=11112", { waitUntil: "domcontentloaded" });
  await shared.waitForFunction(() => location.pathname === "/catalog" && location.search.includes("open=11112") && onPage(".detail.is-module h2"), null, { timeout: 120000 }).catch(() => problems.push("phone: a shared preview link did not bring the module's sheet"));
  await context.close();
}

// ---------- a shared preview link far down the whole list, loaded directly ----------
// The list starts at the module's row, far below the server's first page, while the takeover's
// picture of that page still stands. It rendered every row from the first one down to there
// (`/catalog?open=11787`: 2,986 of them), asked for all their pages and built and measured them all
// again with each answer: on a wide screen the tab froze for minutes, on a phone for seconds
// (2026-10-17). The page has to answer all the while, the list holds a few screens of rows at
// most, and the module stands beside the list (a sheet on a phone) with its row in view and marked.
{
  // A module near the end of the unfiltered list, as the server's pages list it.
  const pages = Number((await (await fetch(base + "/catalog")).text()).match(/class="pager".*?<span class="num">[^<]*?(\d+)<\/span>/s)?.[1] || 1);
  const far = await (await fetch(`${base}/catalog?page=${Math.ceil(pages * 0.9)}`)).text();
  const id = [...far.matchAll(/<a [^>]*>/g)].map(([a]) => a).filter((a) => a.includes('class="row"')).map((a) => a.match(/data-id="([^"]+)"/)?.[1])[25];
  check(id, `far down: no module on page ${Math.ceil(pages * 0.9)} of ${pages} of the catalog`);
  const devices = { desktop: { viewport: { width: 1440, height: 900 } }, phone: { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true } };
  for (const [device, options] of id ? Object.entries(devices) : []) {
    const context = await browser.newContext(options);
    // „Einplanen" plans at once: the question for a study not set up is put aside for the session
    // (plannote.mjs checks the question).
    await context.addInitScript(() => sessionStorage.setItem("betula.plan.asked", "1"));
    const page = await context.newPage();
    page.on("console", (m) => { if (m.type() === "error") problems.push(`far down, ${device}: console: ${m.text().slice(0, 300)}`); });
    page.on("pageerror", (e) => problems.push(`far down, ${device}: pageerror: ${String(e).slice(0, 300)}`));
    // The most rows the list has held at once, from the first moment on.
    await page.addInitScript(() => {
      window.__mostRows = 0;
      new MutationObserver(() => { window.__mostRows = Math.max(window.__mostRows, onPage("#rows-virtual")?.querySelectorAll(":scope > .vrow").length || 0); }).observe(document, { childList: true, subtree: true });
    });
    await page.goto(`${base}/catalog?open=${id}`, { waitUntil: "domcontentloaded" });
    // Where the page is, asked every quarter of a second; `null` when it does not answer within 3 s.
    const state = () => Promise.race([
      page.evaluate((id) => {
        const row = onPage(`#rows-virtual a.row[data-id="${id}"]`);
        const phone = matchMedia("(max-width: 900px)").matches;
        const box = row?.getBoundingClientRect();
        // In view: on a wide screen in the list's part of the scroll area, under the heads of its
        // columns; on a phone beginning in the window between the bar and the sheet (the row
        // stands in the middle of the window, as when coming back to the list, where the sheet,
        // half the screen high, begins).
        const edge = (selector, side) => onPage(selector)?.getBoundingClientRect()[side];
        const area = onPage("#catalog-scroll");
        const top = phone ? edge(".topbar", "bottom") ?? 0 : Math.max(edge("#catalog-scroll", "top") ?? 0, edge("#rows-head", "bottom") ?? 0);
        const bottom = phone ? Math.min(innerHeight, edge(".detail.is-module", "top") ?? innerHeight) : (edge("#catalog-scroll", "top") ?? 0) + (area?.clientHeight ?? 0);
        return {
          app: window.__betulaApp === true,
          preview: Boolean(onPage(".detail.is-module h2")),
          marked: row?.getAttribute("aria-current") === "true",
          seen: Boolean(box) && box.top >= top - 1 && (phone ? box.top < bottom : box.bottom <= bottom + 1),
          where: box && { top: Math.round(box.top), bottom: Math.round(box.bottom), from: Math.round(top), to: Math.round(bottom) },
          rows: window.__mostRows,
        };
      }, id),
      new Promise((resolve) => setTimeout(() => resolve(null), 3000)),
    ]);
    const started = Date.now();
    let now = await state();
    let after = 0;
    // Until the module and its row are there, then a few seconds more.
    while (now && Date.now() - started < 120000 && after < 12) {
      if (now.app && now.preview && now.marked) after++;
      await page.waitForTimeout(250);
      now = await state();
    }
    timings[`far down, ${device}`] = Date.now() - started;
    if (!now) problems.push(`far down, ${device}: /catalog?open=${id} did not answer for 3 s after ${Date.now() - started - 3000} ms`);
    else {
      check(now.app, `far down, ${device}: the browser app never took over`);
      check(now.preview, `far down, ${device}: /catalog?open=${id} did not bring the module ${device === "phone" ? "as a sheet" : "beside the list"}`);
      check(now.marked && now.seen, `far down, ${device}: the module's row is ${now.marked ? `not in view (${JSON.stringify(now.where)})` : "not there or not marked"}`);
      // A screen of rows and ten above and below it, not every row from the first one.
      check(now.rows <= 120, `far down, ${device}: the list held ${now.rows} rows at once`);
    }
    await context.close();
  }
}

// ---------- the height of a phone spent on what is read (owner, 2026-10-17) ----------
// The Termine under the week are folded, and a slot tapped unfolds them and marks its line, in
// sight, without a history entry; the sheet half up goes all up for it. The facts stand two side
// by side. „Studiengänge" is the last section, with five curricula and „Alle … anzeigen".
{
  const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };
  const { page, step, context } = await open(phone, "/catalog?events=yes");
  await page.waitForSelector(".rows a.row >> visible=true", { timeout: 8000 }).catch(() => problems.push("termine: no module with Termine"));
  // A module whose Termine have a week, as the server's page says.
  const id = await page.evaluate(async () => {
    for (const row of [...allOnPage(".rows a.row")].slice(0, 24)) {
      const html = await (await fetch(`/catalog/module/${row.dataset.id}`)).text();
      if (html.includes('class="evfold"')) return row.dataset.id;
    }
    return null;
  });
  if (!id) problems.push("termine: none of the first modules with Termine has a week");
  else {
    const sheet = ".detail.is-module";
    await step("termine: the module's sheet", () => page.evaluate((id) => onPage(`a.row[data-id="${id}"]`)?.click(), id), (id) => location.search.includes(`open=${id}`) && onPage(".detail.is-module .week"), id);
    await page.waitForTimeout(700);
    const folded = await page.evaluate((sheet) => {
      const fold = onPage(`${sheet} details.evfold`);
      const facts = [...allOnPage(`${sheet} .fact`)].map((fact) => Math.round(fact.getBoundingClientRect().top));
      const labels = [...allOnPage(`${sheet} .section > .label`)].map((label) => label.firstChild.textContent);
      return { open: fold?.open, summary: fold?.querySelector("summary").textContent, facts: facts.length, rows: new Set(facts).size, last: labels[labels.length - 1], labels };
    }, sheet);
    check(folded.open === false && /^Termine als Liste\(\d+\)$/.test(folded.summary), `termine: the list under the week is not folded: ${JSON.stringify(folded)}`);
    check(folded.rows < folded.facts, `termine: no two facts stand side by side on a phone: ${JSON.stringify(folded)}`);
    check(!folded.labels.includes("Studiengänge") || folded.last === "Studiengänge", `termine: „Studiengänge" is not the last section: ${folded.labels}`);
    const entries = await page.evaluate(() => history.length);
    // A slot tapped where the sheet shows it, half up; the sheet goes all up, the list unfolds,
    // the slot and its line are marked and the line is in sight.
    const target = await page.evaluate((sheet) => {
      const root = onPage(sheet);
      const slot = root.querySelector('a.slot[data-action="termin"]');
      // The slot into the part of the screen the sheet half up shows (its content scrolled there).
      const over = slot.getBoundingClientRect().top + 24 - innerHeight;
      if (over > 0) root.querySelector(":scope > .scroll").scrollTop += over;
      const r = slot.getBoundingClientRect();
      return { x: r.left + r.width / 2, y: Math.min(r.top + 12, innerHeight - 4), half: !root.classList.contains("is-full") };
    }, sheet);
    await page.touchscreen.tap(target.x, target.y);
    await page.waitForTimeout(900);
    const shown = await page.evaluate((sheet) => {
      const root = onPage(sheet);
      const slot = root.querySelector('a.slot[aria-current="true"]');
      const lines = (slot?.dataset.lines || "").split(" ").filter(Boolean).map((id) => root.querySelector(`#${id}`));
      const line = lines[0]?.getBoundingClientRect();
      return {
        open: root.querySelector("details.evfold").open, full: root.classList.contains("is-full"), picked: lines.length > 0 && lines.every((el) => el?.classList.contains("picked")),
        seen: Boolean(line) && line.top >= 0 && line.bottom <= innerHeight, hash: location.hash, others: root.querySelectorAll(".ev.picked").length - lines.length,
      };
    }, sheet);
    check(shown.open && shown.picked && shown.seen && shown.others === 0 && shown.hash === "", `termine: a tapped slot did not show its line, marked and in sight: ${JSON.stringify(shown)}`);
    check(!target.half || shown.full, "termine: the sheet half up did not go all up for the line");
    check((await page.evaluate(() => history.length)) === entries, "termine: a tapped slot left a history entry");
    // Its line folds it again, and the marks go.
    await page.tap(`${sheet} details.evfold > summary`);
    await page.waitForTimeout(500);
    const shut = await page.evaluate((sheet) => ({ open: onPage(`${sheet} details.evfold`).open, marks: allOnPage(`${sheet} .ev.picked, ${sheet} .slot[aria-current="true"]`).length }), sheet);
    check(!shut.open && shut.marks === 0, `termine: the list's line did not fold it: ${JSON.stringify(shut)}`);
  }
  // Algorithmen und Datenstrukturen belongs to more than five curricula.
  await step("programs: a module of many", () => page.evaluate(() => { const a = Object.assign(document.createElement("a"), { href: "/catalog?events=yes&open=11112" }); document.body.append(a); a.click(); a.remove(); }), () => location.search.includes("open=11112") && onPage(".detail.is-module .linklist.programs"));
  const programs = () => page.evaluate(() => {
    const list = onPage(".detail.is-module .linklist.programs");
    const more = onPage(".detail.is-module .runout-more");
    return { all: list.children.length, shown: [...list.children].filter((a) => a.offsetHeight > 0).length, more: more && more.offsetHeight > 0 ? more.textContent : null };
  });
  const cut = await programs();
  check(cut.all > 5 && cut.shown === 5 && cut.more === `Alle ${cut.all} anzeigen`, `programs: not five of them and „Alle … anzeigen": ${JSON.stringify(cut)}`);
  await page.evaluate(() => onPage(".detail.is-module .runout-more")?.click());
  await page.waitForTimeout(500);
  const all = await programs();
  check(all.shown === all.all && all.more === null, `programs: „Alle … anzeigen" did not show them all: ${JSON.stringify(all)}`);
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
