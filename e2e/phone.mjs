// Checks the phone layout of the catalog in the browser app: the filter sheet closed by a drag
// at its head and by a tap beside it (and snapping back after a short drag), the picker staying
// open while the window shrinks (the on-screen keyboard), the area picker, and the virtual list
// (the page scrolls, the last rows come at its end, the height stays, rows do not overlap).
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node phone.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a console error or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const context = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
const page = await context.newPage();
page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 600)); });
page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
await page.goto(base + "/catalog?program=bachelor-informatik-2008", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));

// ---- the sheet: opened by the button, dimmed page behind it, closed by a drag at its head
await page.tap(".sheet-toggle");
// The sheet slides in: a finger on its head only counts once it stands still.
const settled = () => document.getElementById("filters").classList.contains("open") && getComputedStyle(document.getElementById("filters")).transform === "none";
await page.waitForFunction(() => document.getElementById("filters").classList.contains("open") && document.documentElement.classList.contains("sheet-open"), null, { timeout: 5000 }).catch(() => problems.push("sheet: did not open (or the page is not dimmed)"));
await page.waitForFunction(settled, null, { timeout: 5000 }).catch(() => problems.push("sheet: it did not come to rest"));
const head = await page.locator("#filters .panel-head").boundingBox();
await page.mouse.move(head.x + head.width / 2, head.y + 12);
await page.mouse.down();
await page.mouse.move(head.x + head.width / 2, head.y + 120, { steps: 6 });
const during = await page.evaluate(() => getComputedStyle(document.getElementById("filters")).transform);
await page.mouse.move(head.x + head.width / 2, head.y + 260, { steps: 6 });
await page.mouse.up();
check(during !== "none" && during.includes("matrix"), `sheet: it does not follow the finger (${during})`);
await page.waitForFunction(() => !document.getElementById("filters").classList.contains("open") && !document.documentElement.classList.contains("sheet-open"), null, { timeout: 5000 }).catch(() => problems.push("sheet: a drag down did not close it"));
// A short drag lets it snap back.
await page.tap(".sheet-toggle");
await page.waitForFunction(settled);
const head2 = await page.locator("#filters .panel-head").boundingBox();
await page.mouse.move(head2.x + 100, head2.y + 12);
await page.mouse.down();
await page.mouse.move(head2.x + 100, head2.y + 30, { steps: 4 });
await page.waitForTimeout(400);
await page.mouse.up();
await page.waitForTimeout(400);
check(await page.evaluate(() => document.getElementById("filters").classList.contains("open") && getComputedStyle(document.getElementById("filters")).transform === "none"), "sheet: a short drag did not snap back");
// A tap on the dimmed page closes it.
await page.mouse.click(195, 60);
await page.waitForFunction(() => !document.getElementById("filters").classList.contains("open") && !document.documentElement.classList.contains("sheet-open"), null, { timeout: 5000 }).catch(() => problems.push("sheet: a tap beside it did not close it"));

// ---- the picker on a phone: the keyboard shrinks the viewport, the popup stays
await page.tap(".sheet-toggle");
await page.waitForFunction(() => document.getElementById("filters").classList.contains("open"));
await page.tap("#pick-area");
await page.waitForFunction(() => document.activeElement?.id === "pick-area-search" && document.querySelector("#pick-area-list"), null, { timeout: 5000 }).catch(() => problems.push("picker: did not open on the phone"));
await page.setViewportSize({ width: 390, height: 500 }); // the on-screen keyboard
await page.evaluate(() => document.getElementById("pick-area-search")?.scrollIntoView());
await page.waitForTimeout(400);
check(await page.evaluate(() => Boolean(document.querySelector("#pick-area-list")) && document.querySelector("#pick-area").closest(".combo").hasAttribute("data-open")), "picker: the popup closed when the viewport shrank (the keyboard)");
await page.keyboard.type("praktische");
await page.waitForFunction(() => document.querySelector("#pick-area-list .combo-option .combo-label")?.textContent.includes("Praktische"), null, { timeout: 5000 }).catch(() => problems.push("picker: typing did not narrow the list"));
// Enter takes the first entry; the list then holds as many modules as the entry says.
const picked = await page.evaluate(() => {
  const entry = document.querySelector("#pick-area-list .combo-option");
  return { name: entry?.querySelector(".combo-label")?.textContent, count: Number(entry?.querySelector("small")?.textContent.replace(/\D/g, "")) };
});
await page.keyboard.press("Enter");
await page.waitForFunction(() => /[?&]area=\d+/.test(location.search), null, { timeout: 5000 }).catch(() => problems.push("picker: Enter did not pick the area"));
await page.setViewportSize({ width: 390, height: 844 });
await page.tap('#filters .filter-actions .show');
await page.waitForFunction(() => !document.getElementById("filters").classList.contains("open"));
const inArea = await page.evaluate(() => Number(document.querySelector(".count").textContent.replace(/\D/g, "")));
check(picked.count > 0 && inArea === picked.count, `area: ${inArea} modules in „${picked.name}", the picker said ${picked.count}`);

// ---- the virtual list on a phone: the page scrolls, the last rows come when it is scrolled down
await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true && document.querySelector(".vlist"), null, { timeout: 120000 });
const total = await page.evaluate(() => Number(document.querySelector(".count").textContent.replace(/\D/g, "")));
const before = await page.evaluate(() => document.documentElement.scrollHeight);
await page.evaluate(() => scrollTo(0, document.documentElement.scrollHeight));
await page.waitForFunction((total) => Math.max(-1, ...[...document.querySelectorAll(".vrow")].map((r) => Number(r.dataset.i))) === total - 1, total, { timeout: 8000 }).catch(() => problems.push("phone list: the last row did not render at the end"));
const after = await page.evaluate(() => document.documentElement.scrollHeight);
check(Math.abs(after - before) < before * 0.05, `phone list: the page changed its height from ${before} to ${after}`);
const overlap = await page.evaluate(() => {
  const rows = [...document.querySelectorAll(".vrow")].map((r) => r.getBoundingClientRect()).sort((a, b) => a.top - b.top);
  return rows.some((r, i) => i > 0 && r.top < rows[i - 1].bottom - 1);
});
check(!overlap, "phone list: rendered rows overlap");
check(await page.evaluate(() => /[?&]page=\d+/.test(location.search)), "phone list: the URL does not follow the position");

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
