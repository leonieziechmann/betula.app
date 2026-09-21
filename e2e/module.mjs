// Checks a module in its two sizes, on the desktop and on a phone.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node module.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Desktop: preview → full page keeps the frame (sidebar exactly where the filter panel was), the
// sidebar jumps to sections without history entries, Esc goes back to the list at the row.
// Phone: a tap opens the module's page directly (never the preview), the page has the order of the
// preview (times and facts, then the description), back returns to the tapped row, and a shared
// link with a preview becomes the page.
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
