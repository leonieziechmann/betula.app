// Checks the launch screens of the installed app on iOS (folia/crates/shell/src/launch.rs): on an iPhone the
// head script names the pictures of its screen, upright, light and dark; on an iPad also turned;
// anywhere else none. Every picture it names is served, as a PNG exactly as large as the screen.
// And the manifest whose colours Android's splash screen and the strip under the gesture bar take
// (folia_app::MANIFEST_DARK): a page in the dark names the dark one, a theme chosen in Betula
// wins over the system's, and each is served in its colours. Needs no snapshot.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node launch.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };

// What iOS would take from the page: the startup images the head names.
async function named(device, ios) {
  const context = await browser.newContext(device);
  // `navigator.standalone` is what tells iOS apart; Chromium has none.
  if (ios) await context.addInitScript(() => Object.defineProperty(Navigator.prototype, "standalone", { get: () => false, configurable: true }));
  const page = await context.newPage();
  await page.goto(base + "/", { waitUntil: "domcontentloaded" });
  const links = await page.evaluate(() => [...document.querySelectorAll('link[rel="apple-touch-startup-image"]')].map((link) => ({ href: link.getAttribute("href"), media: link.media })));
  for (const link of links) {
    const response = await context.request.get(base + link.href);
    const body = await response.body();
    const [, width, height] = link.href.match(/\/(\d+)x(\d+)(-dark)?\.png$/) || [];
    const size = body.length > 24 ? [body.readUInt32BE(16), body.readUInt32BE(20)].join("x") : "";
    check(response.status() === 200 && response.headers()["content-type"] === "image/png", `${link.href}: ${response.status()} ${response.headers()["content-type"]}`);
    check(size === `${width}x${height}`, `${link.href} is ${size}`);
  }
  await context.close();
  return links;
}

const phone = await named({ viewport: { width: 393, height: 700 }, screen: { width: 393, height: 852 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true }, true);
check(
  JSON.stringify(phone.map((link) => link.href)) === JSON.stringify(["/assets/launch/1179x2556.png", "/assets/launch/1179x2556-dark.png"]),
  `an iPhone 16 names ${JSON.stringify(phone)}`,
);
check(phone.every((link) => link.media.includes("(orientation: portrait)")) && phone[1]?.media.includes("(prefers-color-scheme: dark)"), `media: ${JSON.stringify(phone)}`);

const tablet = await named({ viewport: { width: 1180, height: 760 }, screen: { width: 820, height: 1180 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true }, true);
check(
  JSON.stringify(tablet.map((link) => link.href).sort()) === JSON.stringify(["/assets/launch/1640x2360-dark.png", "/assets/launch/1640x2360.png", "/assets/launch/2360x1640-dark.png", "/assets/launch/2360x1640.png"]),
  `an iPad Air names ${JSON.stringify(tablet)}`,
);

const desktop = await named({ viewport: { width: 1300, height: 900 } }, false);
check(desktop.length === 0, `a desktop browser names ${JSON.stringify(desktop)}`);

// The manifest a page names, in the system's scheme or in a theme chosen in Betula, and its colours.
async function manifest(colorScheme, path, chosen) {
  const context = await browser.newContext({ colorScheme, viewport: { width: 393, height: 852 }, isMobile: true, hasTouch: true });
  if (chosen) await context.addInitScript((theme) => localStorage.setItem("betula.theme", theme), chosen);
  const page = await context.newPage();
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  const href = await page.evaluate(() => document.querySelector("link[rel=manifest]").getAttribute("href"));
  const response = await context.request.get(base + href);
  const json = response.ok() ? await response.json() : {};
  await context.close();
  return { href, status: response.status(), id: json.id, theme: json.theme_color, background: json.background_color };
}
for (const [colorScheme, path, chosen, href, colour] of [
  ["light", "/", null, "/manifest.webmanifest", "#f1f2f4"],
  ["dark", "/", null, "/manifest-dark.webmanifest", "#0a0c11"],
  ["dark", "/en/catalog", null, "/en/manifest-dark.webmanifest", "#0a0c11"],
  ["light", "/", "dark", "/manifest-dark.webmanifest", "#0a0c11"],
  ["dark", "/", "light", "/manifest.webmanifest", "#f1f2f4"],
]) {
  const got = await manifest(colorScheme, path, chosen);
  const id = path.startsWith("/en") ? "/en" : "/";
  check(
    JSON.stringify(got) === JSON.stringify({ href, status: 200, id, theme: colour, background: colour }),
    `${path} in the ${colorScheme} system${chosen ? `, ${chosen} chosen,` : ""} names ${JSON.stringify(got)}`,
  );
}

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
