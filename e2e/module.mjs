// Checks a module in its two sizes, on the desktop and on a phone.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node module.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Desktop: preview → full page keeps the frame (sidebar exactly where the filter panel was), the
// sidebar jumps to sections without history entries, Esc goes back to the list at the row.
// Phone: a tap opens the module's page directly (never the preview), the page has the order of the
// preview (times and facts, then the description), back returns to the tapped row, and a shared
// link with a preview becomes the page.
// „Einplanen" and „Merken": one pair at the right end of the line of the badges, in the order of
// the Tab key, side by side or, where the line has no room for that, one over the other beside
// the badges (a line of their own only on a phone); „Einplanen" keeps its width when pressed; what
// the plan adds to it shows, and on the module's page the heading is as tall as the server's.
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const timings = {};
const check = (ok, message) => { if (!ok) problems.push(message); };

const open = async (options, path) => {
  const context = await browser.newContext(options);
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
    } catch {
      problems.push(`${name}: did not happen (${page.url()})`);
    }
    timings[name] = Date.now() - started;
    if (!(await page.evaluate(() => window.__marker === 1))) problems.push(`${name}: the page was loaded again`);
  };
  return { page, step, context };
};
const order = (page, root) => page.evaluate((selector) => [...document.querySelectorAll(`${selector} .section > .label`)].map((label) => label.firstChild.textContent), root);

// ---------- desktop ----------
{
  const { page, step, context } = await open({ viewport: { width: 1500, height: 900 } }, "/catalog?program=bachelor-informatik-2008");
  await page.evaluate(() => { document.querySelector(".rows").scrollTop = 1200; });
  await page.waitForTimeout(250); // the virtual list renders the rows of the new position on the next frame
  const id = await page.evaluate(() => { const rows = document.querySelector(".rows").getBoundingClientRect(); return [...document.querySelectorAll(".rows a.row")].find((r) => r.getBoundingClientRect().top > rows.top + 200).dataset.id; });
  await step("preview", () => page.click(`a.row[data-id="${id}"]`), () => Boolean(document.querySelector(".detail h2")));
  const previewOrder = await order(page, ".detail");
  const filters = await page.evaluate(() => { const r = document.getElementById("filters").getBoundingClientRect(); return [r.left, r.width, r.top]; });
  await step("full page", () => page.keyboard.press("f"), () => location.pathname.startsWith("/catalog/module/") && document.querySelector(".module-page h2") && document.getElementById("sidebar"));
  const sidebar = await page.evaluate(() => { const r = document.getElementById("sidebar").getBoundingClientRect(); return [r.left, r.width, r.top]; });
  check(JSON.stringify(sidebar) === JSON.stringify(filters), `the sidebar is not where the filter panel was: ${sidebar} instead of ${filters}`);
  check(JSON.stringify(await order(page, ".module-page")) === JSON.stringify(previewOrder), `the page and the preview differ in order: ${await order(page, ".module-page")} / ${previewOrder}`);
  const columns = await page.evaluate(() => { const [side, main] = [document.querySelector(".module-grid > aside"), document.querySelector(".module-grid > div")].map((el) => el.getBoundingClientRect()); return main.left < side.left && Math.abs(main.top - side.top) < 2; });
  check(columns, "wide page: the description is not on the left of the facts");

  // The sidebar lists the sections that exist, and jumps to them without a history entry.
  const toc = await page.evaluate(() => [...document.querySelectorAll(".toc a")].map((a) => [a.textContent, Boolean(document.querySelector(a.getAttribute("href")))]));
  check(toc.length >= 3 && toc.every(([, exists]) => exists), `the sidebar names sections that are not there: ${JSON.stringify(toc)}`);
  const entries = await page.evaluate(() => history.length);
  await page.click('.toc a[href="#studiengaenge"]');
  await page.waitForTimeout(700);
  const jumped = await page.evaluate(() => { const top = document.getElementById("studiengaenge").getBoundingClientRect().top; return top < innerHeight - 40 && location.hash === "" && top >= 0; });
  check(jumped && (await page.evaluate(() => history.length)) === entries, "the sidebar's jump did not show the section, or left a history entry");

  // The handle of the sidebar is the handle of the filter panel: one width for both.
  const edge = await page.locator('[data-action="resize-filters"]').boundingBox();
  await page.mouse.move(edge.x + 6, edge.y + 300);
  await page.mouse.down();
  await page.mouse.move(edge.x + 46, edge.y + 300, { steps: 5 });
  await page.mouse.up();
  check(Math.abs((await page.evaluate(() => document.getElementById("sidebar").getBoundingClientRect().width)) - (sidebar[1] + 40)) <= 2, "the sidebar's width does not follow its handle");

  await step("Esc goes back to the list", () => page.keyboard.press("Escape"), () => location.pathname === "/catalog" && location.search.includes("open=") && document.querySelector(".rows a.row"));
  await page.waitForTimeout(400);
  const back = await page.evaluate((id) => { const row = document.querySelector(`a.row[data-id="${id}"]`)?.getBoundingClientRect(); const rows = document.querySelector(".rows").getBoundingClientRect(); return row ? row.top >= rows.top && row.bottom <= rows.bottom : null; }, id);
  check(back === true, `back on the list the row of the module is ${back === null ? "not loaded" : "not in view"}`);
  check(Math.abs((await page.evaluate(() => document.getElementById("filters").getBoundingClientRect().width)) - (sidebar[1] + 40)) <= 2, "the filter panel does not have the width the sidebar was given");
  await page.evaluate(() => localStorage.removeItem("betula.filters.width"));
  await context.close();
}

// ---------- exam dates the BTU cannot mean (catalog/src/exam_reading.rs) ----------
// Analysis I (11103) lists QIS's placeholder „So 01:00–02:30, 27.12.2015" twice; 12000 has a
// deadline at 23:45–24:00. In the app's preview and in the server's page without JavaScript.
{
  const exams = (page) => page.evaluate(() => {
    const section = document.querySelector("#pruefungstermine");
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
  await page.waitForSelector("#preview #pruefungstermine", { timeout: 8000 }).catch(() => {});
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
// own. A plan in another semester and the finder's placeholder make the switch say more.
{
  const PLAN = "m\t2027S\t12330\t1790000000\t\np\t3\t2026W\t079-82-2008\t17\t1-1\t6\tfues\t\tFachübergreifendes Studium\n";
  const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };
  const pair = (page, root, body) => page.evaluate(([root, body]) => {
    const line = document.querySelector(`${root} .hero-line`);
    const [plan, mark, edge] = [line.querySelector(".plan-toggle"), line.querySelector(".mark-toggle"), document.querySelector(body)].map((el) => el.getBoundingClientRect());
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
      tab: Boolean(line.querySelector(".plan-toggle").compareDocumentPosition(line.querySelector(".mark-toggle")) & Node.DOCUMENT_POSITION_FOLLOWING),
      note: line.querySelector(".plan-toggle > small")?.checkVisibility() ? line.querySelector(".plan-toggle > small").textContent : null,
      label: line.querySelector(".plan-toggle > span").textContent,
      width: plan.width,
    };
  }, [root, body]);
  const sideBySide = (seen) => seen.together <= 0.5 && Math.abs(seen.gap - 6) <= 0.5;
  const stacked = (seen) => Math.abs(seen.under - 6) <= 0.5 && seen.aligned <= 0.5;
  // Beside the badges on a wide screen, never on a line of their own; under them on a phone.
  const expect = (where, seen, onPhone = false) => check((sideBySide(seen) || stacked(seen)) && seen.edge <= 0.5 && (onPhone ? seen.below : seen.beside) && seen.tab, `${where}: „Einplanen" and „Merken" are not one pair at the right end, in order: ${JSON.stringify(seen)}`);
  const preview = ["#preview", "#preview .dbody .section"];
  const onPage = [".module-page", ".module-grid > aside .section"];

  for (const width of [960, 1100, 1500]) {
    const { page, step, context } = await open({ viewport: { width, height: 900 } }, "/catalog?q=datenbanken&open=12330");
    await page.waitForSelector("#preview .plan-toggle", { timeout: 8000 }).catch(() => problems.push(`preview at ${width}px: no „Einplanen"`));
    const before = await pair(page, ...preview);
    expect(`preview at ${width}px`, before);
    check(stacked(before), `preview at ${width}px: the four badges leave no room for the pair side by side, and yet the two do not stand one over the other: ${JSON.stringify(before)}`);
    if (width === 1500) {
      await page.evaluate(() => document.activeElement?.blur());
      await step("„Einplanen“ plans", () => page.click("#preview .plan-toggle"), () => document.querySelector("#preview .plan-toggle").getAttribute("aria-pressed") === "true" && !document.querySelector("#preview .plan-toggle").hasAttribute("aria-busy"));
      const pressed = await pair(page, ...preview);
      check(pressed.label === "Eingeplant" && Math.abs(pressed.width - before.width) < 0.5, `„Einplanen“ changes its width when pressed: ${before.width} → ${JSON.stringify(pressed)}`);
      await step("and a second click takes it out", () => page.click("#preview .plan-toggle"), () => document.querySelector("#preview .plan-toggle").getAttribute("aria-pressed") === "false" && !localStorage.getItem("betula.studyplan.v1"));
    }
    await step("full page", () => page.click('#preview [data-action="fullscreen"]'), () => location.pathname === "/catalog/module/12330" && document.querySelector(".module-page .plan-toggle"));
    expect(`the module's page at ${width}px`, await pair(page, ...onPage));
    await context.close();
  }

  // With a plan: in the preview the switch says what the plan adds; on the page the sidebar says
  // it, and the heading keeps the server's height.
  const planned = async (options, path) => {
    const context = await browser.newContext(options);
    await context.addInitScript((plan) => localStorage.setItem("betula.studyplan.v1", plan), PLAN);
    const page = await context.newPage();
    page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
    await page.goto(base + path, { waitUntil: "domcontentloaded" });
    await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
    return { page, context };
  };
  const server = async (options, path) => {
    const context = await browser.newContext(options);
    await context.route("**/pkg/**", (route) => route.abort());
    const page = await context.newPage();
    await page.goto(base + path, { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    const hero = await page.evaluate(() => document.querySelector(".module-page > .hero").getBoundingClientRect().height);
    await context.close();
    return hero;
  };
  {
    const { page, context } = await planned({ viewport: { width: 1500, height: 900 } }, "/catalog?q=datenbanken&fill=p3&open=12330");
    await page.waitForFunction(() => document.querySelector("#preview .plan-toggle > small"), null, { timeout: 8000 }).catch(() => {});
    const seen = await pair(page, ...preview);
    expect("preview with a plan", seen);
    // (The finder hands its placeholder to the preview once it has one; the other semester shows now.)
    check(seen.label === "Einplanen" && Boolean(seen.note?.endsWith("geplant: SoSe 27")), `preview with a plan: the switch says ${JSON.stringify(seen)}`);
    await context.close();
  }
  for (const [where, options] of [["1000px", { viewport: { width: 1000, height: 900 } }], ["phone", phone]]) {
    const path = "/catalog/module/12330?plan=2026W&fill=p3";
    const { page, context } = await planned(options, path);
    await page.waitForFunction(() => document.querySelector(".sidebar .plan-toggle small")?.textContent.includes("geplant"), null, { timeout: 8000 }).catch(() => {});
    await page.evaluate(() => document.fonts.ready);
    const seen = await pair(page, ...onPage);
    expect(`the module's page with a plan (${where})`, seen, where === "phone");
    const side = await page.evaluate(() => document.querySelector(".sidebar .plan-toggle small")?.textContent);
    check(side === "WiSe 2026/27 · für „Fachübergreifendes Studium“ · geplant: SoSe 2027", `the module's page with a plan (${where}): the sidebar says ${side}`);
    check(where === "phone" ? seen.below && seen.note?.startsWith("für „Fach") : seen.note === null, `the module's page with a plan (${where}): the switch says ${JSON.stringify(seen)}`);
    const hero = await page.evaluate(() => document.querySelector(".module-page > .hero").getBoundingClientRect().height);
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
  await page.evaluate(() => { const list = document.querySelector(".vlist"); scrollTo(0, list.getBoundingClientRect().top + scrollY + 88 * 88); });
  await page.waitForFunction(() => document.querySelector('.vrow[data-i="88"] a.row'), null, { timeout: 8000 }).catch(() => problems.push("phone: the list did not render its 89th row"));
  const id = await page.evaluate(() => { const row = document.querySelector('.vrow[data-i="88"] a.row'); row.scrollIntoView({ block: "center" }); return row.dataset.id; });
  await page.waitForTimeout(300);
  await step("phone: a tap opens the module's page", () => page.tap(`a.row[data-id="${id}"]`), (id) => location.pathname === `/catalog/module/${id}` && document.querySelector(".module-page h2"), id);
  check(!(await page.evaluate(() => location.search.includes("open="))), "phone: the preview was not skipped");
  const phoneOrder = await order(page, ".module-page");
  check(phoneOrder[0] === "Termine" && phoneOrder.indexOf("Auf einen Blick") < phoneOrder.indexOf("Inhalte") , `phone: the page does not start with the times and facts: ${phoneOrder}`);
  check(await page.evaluate(() => { const side = document.getElementById("sidebar").getBoundingClientRect(); const article = document.querySelector(".module-page").getBoundingClientRect(); return side.top >= article.bottom - 1 && getComputedStyle(document.querySelector(".toc")).display === "none"; }), "phone: the sidebar is not a block of actions under the module");
  await step("phone: back returns to the list", () => page.click('[data-action="back"]'), () => location.pathname === "/catalog" && document.querySelector(".rows a.row"));
  await page.waitForTimeout(500);
  const seen = await page.evaluate((id) => { const row = document.querySelector(`a.row[data-id="${id}"]`)?.getBoundingClientRect(); return row ? row.top >= 0 && row.bottom <= innerHeight : null; }, id);
  check(seen === true, `phone: back on the list the tapped row is ${seen === null ? "not loaded" : "not in view"}`);

  // A shared link with a preview becomes the module's page on a phone.
  const shared = await context.newPage();
  await shared.goto(base + "/catalog?turnus=winter&open=11112", { waitUntil: "domcontentloaded" });
  await shared.waitForFunction(() => location.pathname === "/catalog/module/11112" && document.querySelector(".module-page h2"), null, { timeout: 120000 }).catch(() => problems.push("phone: a shared preview link did not become the module's page"));
  await context.close();
}

await browser.close();
console.log(JSON.stringify({ timings, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
