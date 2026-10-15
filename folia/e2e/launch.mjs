// Checks the launch screens of the installed app on iOS (folia/crates/shell/src/launch.rs): on an iPhone the
// head script names the pictures of its screen, upright, light and dark; on an iPad also turned;
// anywhere else none. Every picture it names is served, as a PNG exactly as large as the screen.
// And the manifest whose colours Android's splash screen and the strip under the gesture bar take
// (folia_app::MANIFEST_DARK): a page in the dark names the dark one, a theme chosen in Betula
// wins over the system's, and each is served in its colours. And the start of the installed app
// (`.launch`, folia_app::HEAD_SCRIPT and enhance.js): where Android and iOS leave their picture,
// once a session, flying to the logo on the screen or fading where there is none, taken away by a
// tap; nothing of it in a browser's tab or on a desktop. Needs no snapshot.
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

// The start of the installed app: an installed app (`display-mode: standalone`, which Chromium
// cannot be put in, so the page is told so; on iOS `navigator.standalone`) on a phone or a tablet.
async function start(device, { ios = false, installed = true, reduced = false, tap = false, path = "/" } = {}) {
  const context = await browser.newContext({ ...device, reducedMotion: reduced ? "reduce" : "no-preference" });
  await context.addInitScript(({ ios, installed }) => {
    if (ios) Object.defineProperty(Navigator.prototype, "standalone", { get: () => installed, configurable: true });
    const media = window.matchMedia.bind(window);
    window.matchMedia = (query) => (query === "(display-mode: standalone)" ? { matches: installed && !ios, media: query, addEventListener() {}, removeEventListener() {} } : media(query));
  }, { ios, installed });
  const page = await context.newPage();
  // The animations of the start go slowly, so that a moment of them can be held and measured.
  const cdp = await context.newCDPSession(page);
  await cdp.send("Animation.enable");
  await cdp.send("Animation.setPlaybackRate", { playbackRate: 0.1 });
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  const facts = await page.evaluate(() => {
    const o = document.querySelector("html > .launch");
    const box = (el) => el && (({ left, top, width, height }) => [left, top, width, height].map((v) => Math.round(v * 10) / 10))(el.getBoundingClientRect());
    return o && { ios: o.classList.contains("ios"), beside: o.nextElementSibling?.tagName, icon: box(o.querySelector(".launch-icon")), name: box(o.querySelector(".launch-name")), size: [innerWidth, innerHeight] };
  });
  if (facts) {
    if (tap) {
      await page.mouse.click(10, 10);
      await cdp.send("Animation.setPlaybackRate", { playbackRate: 1 });
      facts.gone = await page.waitForFunction(() => !document.querySelector("html > .launch"), null, { timeout: 1000 }).then(() => true, () => false);
    } else {
      await page.waitForFunction(() => document.querySelector("html > .launch")?.style.pointerEvents === "none", null, { timeout: 5000 }).catch(() => {});
      // Where the icon is at the end of its way (held there for a frame), and the logo it went to.
      await page.evaluate(() => new Promise((frame) => {
        for (const a of document.getAnimations().filter((a) => a.effect?.target?.closest?.(".launch"))) {
          a.pause();
          a.currentTime = 640;
        }
        requestAnimationFrame(() => requestAnimationFrame(frame));
      }));
      Object.assign(facts, await page.evaluate(() => {
        const box = ({ left, top, width, height }) => [left, top, width, height].map((v) => Math.round(v * 10) / 10);
        const logo = [...document.querySelectorAll(".logo")].map((el) => el.getBoundingClientRect()).find((r) => r.width > 0 && r.bottom > 0 && r.top < innerHeight);
        const own = document.getAnimations().filter((a) => a.effect?.target?.closest?.(".launch"));
        const moved = own.some((a) => a.effect.getKeyframes().some((k) => k.transform && k.transform !== "none"));
        const landed = box(document.querySelector("html > .launch .launch-icon").getBoundingClientRect());
        for (const a of own) a.play();
        return { landed, logo: logo && box(logo), moved };
      }));
      await cdp.send("Animation.setPlaybackRate", { playbackRate: 1 });
      facts.gone = await page.waitForFunction(() => !document.querySelector("html > .launch"), null, { timeout: 3000 }).then(() => true, () => false);
    }
    // Once a session: a page the app goes to (or loads again) is no start.
    await page.goto(base + "/en", { waitUntil: "domcontentloaded" });
    facts.again = await page.evaluate(() => !!document.querySelector("html > .launch"));
  }
  await context.close();
  return facts;
}
const pixel = { viewport: { width: 412, height: 839 }, screen: { width: 412, height: 915 }, deviceScaleFactor: 2.625, isMobile: true, hasTouch: true };
const iphone = { viewport: { width: 393, height: 793 }, screen: { width: 393, height: 852 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true };
const tab = { viewport: { width: 1280, height: 752 }, screen: { width: 1280, height: 800 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true };
const near = (a, b) => a && b && a.every((v, i) => Math.abs(v - b[i]) < 0.6);
const landed = (facts) => (facts.logo ? facts.moved && near(facts.landed, facts.logo) : !facts.moved);

// Android: the icon where Chrome's splash screen has it (128 px, its middle 16 px above the
// window's, 24 on a tablet), no name (Chrome's fades with its splash).
const android = await start(pixel);
check(android && !android.ios && android.beside === "BODY" && near(android.icon, [142, 339.5, 128, 128]) && !android.name, `Android's start: ${JSON.stringify(android)}`);
check(android && landed(android) && android.gone && !android.again, `Android's start goes to the logo once: ${JSON.stringify(android)}`);
const big = await start(tab);
check(big && near(big.icon, [576, 288, 128, 128]) && landed(big) && big.gone, `a tablet's start: ${JSON.stringify(big)}`);
// iOS: the launch screen of launch.rs, which stands on the whole screen, here under a status bar
// of 59 px: the icon in the screen's middle, the name's baseline 852 - 63.9 from its top.
const ios = await start(iphone, { ios: true });
check(ios && ios.ios && near(ios.icon, [132.5, 303, 128, 128]) && near(ios.name, [0, 715.3, 393, 16]), `iOS's start: ${JSON.stringify(ios)}`);
check(ios && landed(ios) && ios.gone && !ios.again, `iOS's start goes to the logo once: ${JSON.stringify(ios)}`);
// A phone's page without a logo on the screen, and reduced motion: the picture fades, nothing flies.
const catalog = await start(pixel, { path: "/catalog" });
check(catalog && !catalog.logo && !catalog.moved && catalog.gone, `a start on the catalog: ${JSON.stringify(catalog)}`);
const calm = await start(pixel, { reduced: true });
check(calm && !calm.moved && calm.gone, `a start with reduced motion: ${JSON.stringify(calm)}`);
// A tap takes it away at once.
const tapped = await start(pixel, { tap: true });
check(tapped && tapped.gone, `a tap on the start: ${JSON.stringify(tapped)}`);
// Nothing of it in a browser's tab, on a desktop, or on iOS in Safari.
check(!(await start(pixel, { installed: false })), "a start in Chrome's tab");
check(!(await start({ viewport: { width: 1300, height: 900 } })), "a start on a desktop");
check(!(await start(iphone, { ios: true, installed: false })), "a start in Safari");

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
