// Closed testing (folia/crates/server/src/access.rs) in a real browser: a page leads to the login page, a wrong
// password stays there and says so, the right one leads to where the visitor wanted to go, and
// behind the gate the browser app takes over as always (its fetches carry the cookie; the manifest,
// which browsers fetch without cookies, stays open). Also the login page itself: one left edge,
// centred, nothing scrolling sideways on a phone, and the form working without JavaScript.
// Needs a folia with the gate on; the password comes from the environment, never from this file:
//   FOLIA_ACCESS_PASSWORD=… folia --access-gate --addr 127.0.0.1:8086
//   GATE_PASSWORD=… node e2e/gate.mjs [base-url]
import { chromium } from "playwright-core";

const base = process.argv[2] || "http://127.0.0.1:8086";
const password = process.env.GATE_PASSWORD;
if (!password) {
  console.error("GATE_PASSWORD is not set (the password the server under test was started with)");
  process.exit(2);
}
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

// Without a cookie: nothing but the login page and what it needs. (The body is read to its end:
// an unread one keeps its connection, and with it this process, alive.)
const status = async (path) => {
  const response = await fetch(base + path);
  await response.arrayBuffer();
  return response.status;
};
for (const path of ["/api/db", "/api/status", "/sitemap.xml", "/pkg/folia_client.js", "/assets/boot.js", "/assets/launch/100x100.png"]) {
  check((await status(path)) === 401, `${path} answers without the password`);
}
// What a home screen reads: the manifest and its icons, and the launch screens of iOS.
for (const path of ["/assets/app.css", "/assets/inter-latin.woff2", "/favicon.ico", "/manifest.webmanifest", "/assets/icon-maskable-1024.png", "/assets/icon-monochrome-512.png", "/assets/launch/1179x2556.png"]) {
  check((await status(path)) === 200, `${path} is not open`);
}
check((await status("/healthz")) !== 401, "/healthz is behind the gate (a supervisor has no password)");
check(/Disallow: \/\n/.test(await (await fetch(base + "/robots.txt")).text()), "robots.txt does not turn crawlers away");

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge" });
const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
const page = await context.newPage();
const errors = [];
const refused = [];
page.on("pageerror", (error) => errors.push(String(error)));
page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
page.on("response", (response) => { if (response.status() === 401) refused.push(new URL(response.url()).pathname); });

const wanted = "/catalog?q=mathe";
await page.goto(base + wanted, { waitUntil: "networkidle" });
check(new URL(page.url()).pathname === "/access", `a page did not lead to the login page: ${page.url()}`);
check(new URL(page.url()).searchParams.get("next") === wanted, "the login page does not know where the visitor wanted to go");
check((await page.title()).includes("Betula"), "the login page has no title");
check(await page.evaluate(() => document.activeElement?.id === "password"), "the password field does not have the focus");

// One column: mark and panel share the left edge, field and button fill the panel, the column
// stands in the middle.
const box = (selector) => page.evaluate((s) => { const r = document.querySelector(s).getBoundingClientRect(); return { left: r.left, right: r.right, top: r.top, bottom: r.bottom }; }, selector);
const layout = async (label) => {
  const [mark, panel, field, button, heading] = [await box(".gate-brand .logo"), await box(".gate-panel"), await box("#password"), await box(".gate-panel .button"), await box(".gate-panel h1")];
  const width = await page.evaluate(() => window.innerWidth);
  check(Math.abs(mark.left - panel.left) < 0.5, `${label}: mark and panel do not share the left edge (${mark.left} / ${panel.left})`);
  check(Math.abs(field.left - heading.left) < 0.5 && Math.abs(field.left - button.left) < 0.5 && Math.abs(field.right - button.right) < 0.5, `${label}: heading, field and button are not on one axis`);
  check(Math.abs(panel.left - (width - panel.right)) < 1, `${label}: the column is not centred (${panel.left} / ${width - panel.right})`);
  check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), `${label}: the page scrolls sideways`);
  return { mark, panel };
};
await layout("desktop");

// A wrong password: the form again, with the reason and the way back kept.
await page.fill("#password", password + "-falsch");
await page.click(".gate-panel .button");
await page.waitForSelector(".gate-problem[role=alert]");
check(new URL(page.url()).pathname === "/access", "a wrong password left the login page");
check(await page.getAttribute("#password", "aria-invalid") === "true", "the field is not marked after a wrong password");
check(await page.inputValue("input[name=next]") === wanted, "a wrong password lost the way back");
check((await context.cookies()).length === 0, "a wrong password set a cookie");
await layout("after a wrong password");
// The refusal is a 401, which the browser notes in its console; nothing else may be there.
check(errors.every((text) => text.includes("401")), `errors on the login page: ${errors.join(" | ")}`);
refused.length = 0;
errors.length = 0;

// The right one: on to the wanted page, with the fade between the pages, and the browser app takes
// over behind the gate. The stylesheet answers late here, as over a real network (its 304 comes
// after the parser has reached <body>): the page must still come with its transition, which it
// only does when the opt-in is in the head itself (app::VIEW_TRANSITION_STYLE). Without, Chromium
// drops it and says "ViewTransition opt-in disabled" in the console.
await context.addInitScript(() => addEventListener("pagereveal", (event) => { window.__revealedWithTransition = !!event.viewTransition; }));
await context.route((url) => url.pathname === "/assets/app.css", async (route) => { await new Promise((resolve) => setTimeout(resolve, 150)); await route.continue(); });
await page.fill("#password", password);
await Promise.all([page.waitForURL((url) => url.pathname === "/catalog"), page.click(".gate-panel .button")]);
await page.waitForFunction(() => window.__revealedWithTransition !== undefined);
check(await page.evaluate(() => window.__revealedWithTransition), "the page after the login came without the fade");
// (Waits for what is still held back: the service worker fetches the stylesheet too.)
await context.unrouteAll({ behavior: "wait" });
check(page.url() === base + wanted, `the login led to ${page.url()}`);
const [cookie] = await context.cookies();
check(cookie?.name === "betula_access" && cookie.httpOnly && cookie.sameSite === "Lax", `the cookie is not what it should be: ${JSON.stringify(cookie)}`);
check(cookie && Math.abs(cookie.expires - Date.now() / 1000 - 90 * 86400) < 600, "the visit does not last 90 days");
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 });
await page.waitForLoadState("networkidle");
check(refused.length === 0, `behind the gate something was still refused: ${refused}`);
check(errors.length === 0, `errors in the browser: ${errors.join(" | ")}`);
await page.goto(base + "/access", { waitUntil: "networkidle" });
check(new URL(page.url()).pathname === "/", "who is in is still asked for the password");
await context.close();

// A phone, and no JavaScript: the same form, nothing sideways, the login works.
const phone = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, javaScriptEnabled: false });
const small = await phone.newPage();
await small.goto(base + "/programs", { waitUntil: "networkidle" });
const box2 = (selector) => small.locator(selector).boundingBox();
const [mark, panel] = [await box2(".gate-brand .logo"), await box2(".gate-panel")];
check(Math.abs(mark.x - panel.x) < 0.5 && Math.abs(panel.x - (390 - panel.x - panel.width)) < 1, "phone: the column is not centred on one left edge");
check(mark.y > 40 && panel.y + panel.height < 844, `phone: the column does not stand free on the screen (${mark.y} … ${panel.y + panel.height})`);
check((await small.locator("#password").boundingBox()).height >= 44 && (await small.locator(".gate-panel .button").boundingBox()).height >= 44, "phone: field or button are too small to hit");
await small.fill("#password", password);
await Promise.all([small.waitForURL((url) => url.pathname === "/programs"), small.click(".gate-panel .button")]);
check((await small.title()).includes("Studieng"), `phone without JavaScript: the login led to "${await small.title()}"`);
await phone.close();
await browser.close();

if (failures.length) {
  console.error("gate FAILED:\n- " + failures.join("\n- "));
  process.exit(1);
}
console.log("gate ok: turned away, wrong password, login, takeover behind the gate, phone without JavaScript");
