// The landing page in the browser app: the map of the programs is drawn from what the server laid
// out (nothing is computed in the browser), a pointer over a dot shows the program's relatives, a
// click on a dot opens the program without loading a page, and the head describes the page that
// is shown (not the one the visit started on). Needs radix serve-snapshot + folia running and a
// fresh `bash scripts/build-client.sh`.
//   node e2e/home.mjs [base-url]
import { chromium } from "playwright-core";

const base = process.argv[2] || "http://127.0.0.1:8080";
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge" });
const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
const errors = [];
page.on("pageerror", (error) => errors.push(String(error)));

// Without the app: the map is part of the server's HTML, its dots are links.
const html = await (await fetch(base + "/")).text();
check(/<svg[^>]*class="map map-wide"/.test(html) && /<svg[^>]*class="map map-tall"/.test(html), "server HTML has no map");
check((html.match(/class="map-dot /g) || []).length > 200, "server HTML: the dots of both sheets are missing");
check(/<a href="\/programs\/[^"]+\/plan"[^>]*class="map-dot /.test(html), "server HTML: dots are not links to programs");

await page.goto(base + "/", { waitUntil: "networkidle" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 });
await page.waitForSelector(".home .map-wide .map-dot");
// A page load would lose this (the method of spa.mjs).
await page.evaluate(() => { window.__marker = 1; });
check(await page.evaluate(() => typeof window.betulaMap === "string" && window.betulaMap.length > 1000), "boot.js did not hand the map to the app");
check(await page.evaluate(() => document.querySelectorAll(".map-wide .map-dot").length) > 100, "the app draws no dots");
check(await page.evaluate(() => document.querySelectorAll("head meta[name=description]").length) === 1, "the head has not exactly one description after the takeover");
check(await page.evaluate(() => document.querySelectorAll("head link[rel=canonical]").length) === 1, "the head has not exactly one canonical address after the takeover");

// A pointer over a dot: its relatives stand out and the line above the map names them.
const dot = page.locator(".map-wide .map-dot.bachelor").first();
const name = (await dot.getAttribute("aria-label")).split(" · ")[0];
await dot.hover();
await page.waitForSelector(".map-wide.has-hot .map-hot path");
const info = await page.textContent(".map-info");
check(info.includes(name), `the info line does not name the program under the pointer: ${info}`);
await page.mouse.move(5, 5);
await page.waitForFunction(() => !document.querySelector(".map.has-hot"));
check((await page.textContent(".map-info")).includes("Bachelor"), "the legend does not come back");

// A click on a dot opens the program in the app.
const href = await dot.getAttribute("href");
await dot.click();
await page.waitForFunction((path) => location.pathname === path, href);
await page.waitForSelector("[data-walk='program-page']");
check(await page.evaluate(() => window.__marker === 1), "a click on a dot loaded a page");
check(await page.evaluate((path) => document.querySelector("head link[rel=canonical]")?.href.endsWith(path), href), "the canonical address did not follow to the program");
check(await page.evaluate(() => document.querySelectorAll("head meta[name=description]").length) === 1, "two descriptions after a navigation");

// A phone gets the tall sheet.
await page.setViewportSize({ width: 390, height: 844 });
await page.goto(base + "/", { waitUntil: "networkidle" });
await page.waitForSelector(".home .map-tall .map-dot");
const shown = await page.evaluate(() => [".map-wide", ".map-tall"].map((s) => getComputedStyle(document.querySelector(s)).display));
check(shown[0] === "none" && shown[1] !== "none", `phone: the wrong sheet is shown (${shown})`);
check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), "phone: the page scrolls sideways");

check(errors.length === 0, `page errors: ${errors.join(" | ")}`);
await browser.close();
if (failures.length) {
  console.error(failures.map((f) => "FAIL " + f).join("\n"));
  process.exit(1);
}
console.log("home: ok");
