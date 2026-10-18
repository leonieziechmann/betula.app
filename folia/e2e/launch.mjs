// Checks the launch screens of the installed app on iOS (folia/crates/shell/src/launch.rs): on an iPhone the
// head script names the pictures of its screen, upright, light and dark; on an iPad also turned;
// anywhere else none. Every picture it names is served, as a PNG exactly as large as the screen.
// And the manifest whose colours Android's splash screen and the strip under the gesture bar take
// (folia_app::MANIFEST_DARK): a page in the dark names the dark one, a theme chosen in Betula
// wins over the system's, and each is served in its colours. And the start of the installed app
// (`.launch`, folia_app::HEAD_SCRIPT and enhance.js): where Android and iOS leave their picture
// (Chrome: Android's own splash, 240 px in the middle of the screen; Brave: Chromium's, 128 px,
// placed by Chromium's version; iOS: the server's picture), with the wordmark at the bottom, once
// a session, flying to the logo on the screen or fading where there is none, taken away by a
// tap; the status bar it is placed under, measured at a tap; nothing of it in a browser's tab or
// on a desktop. Needs no snapshot.
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
// `chrome`: the brands of Google Chrome (an app it installs is a WebAPK); `brave`: Brave's
// `navigator.brave`; `version`: the Chromium the user agent names; `top`: the status bar measured
// at an earlier start.
async function start(device, { ios = false, installed = true, reduced = false, tap = false, path = "/", chrome = false, brave = false, version = 0, top = null, dark = false } = {}) {
  const userAgent = version ? `Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${version}.0.0.0 Mobile Safari/537.36` : undefined;
  const context = await browser.newContext({ ...device, userAgent, colorScheme: dark ? "dark" : "light", reducedMotion: reduced ? "reduce" : "no-preference" });
  await context.addInitScript(({ ios, installed, chrome, brave, top }) => {
    if (ios) Object.defineProperty(Navigator.prototype, "standalone", { get: () => installed, configurable: true });
    if (chrome) Object.defineProperty(Navigator.prototype, "userAgentData", { get: () => ({ brands: [{ brand: "Google Chrome", version: "155" }, { brand: "Chromium", version: "155" }], mobile: true }), configurable: true });
    if (brave) Object.defineProperty(Navigator.prototype, "brave", { get: () => ({ isBrave: () => Promise.resolve(true) }), configurable: true });
    if (top && !sessionStorage.getItem("betula.launched")) localStorage.setItem("betula.launch.top", top);
    const media = window.matchMedia.bind(window);
    window.matchMedia = (query) => (query === "(display-mode: standalone)" ? { matches: installed && !ios, media: query, addEventListener() {}, removeEventListener() {} } : media(query));
  }, { ios, installed, chrome, brave, top });
  const page = await context.newPage();
  // The animations of the start go slowly, so that a moment of them can be held and measured.
  const cdp = await context.newCDPSession(page);
  await cdp.send("Animation.enable");
  await cdp.send("Animation.setPlaybackRate", { playbackRate: 0.1 });
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  const facts = await page.evaluate(() => {
    const o = document.querySelector("html > .launch");
    const box = (el) => el && (({ left, top, width, height }) => [left, top, width, height].map((v) => Math.round(v * 10) / 10))(el.getBoundingClientRect());
    const leaf = o && getComputedStyle(o.querySelector(".launch-icon i"));
    return o && {
      ios: o.classList.contains("ios"), sys: o.classList.contains("sys"), beside: o.nextElementSibling?.tagName,
      icon: box(o.querySelector(".launch-icon")), name: box(o.querySelector(".launch-name")), mark: box(o.querySelector(".launch-name .wordmark")),
      leaf: leaf.transform, colours: [getComputedStyle(o.querySelector(".launch-icon b")).backgroundColor, leaf.backgroundColor], size: [innerWidth, innerHeight],
    };
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

// The wordmark as the page sets it (`.wordmark.small`, 22 px), centred, its baseline at the box's
// top + 19.
const wordmark = (facts, baseline) => near(facts.name, [0, baseline - 19, facts.size[0], 22]) && Math.abs(facts.mark[2] - 68.3) < 0.6 && near([facts.mark[0] * 2 + facts.mark[2]], [facts.size[0]]);
// The leaf on its grid of 96, scaled by `k` around the middle of the icon `size` wide.
const leaf = (facts, size, k) => {
  const [s, a] = [size * k / 96, size * (1 - k) / 2];
  const got = facts.leaf.match(/-?[\d.]+(e-?\d+)?/g)?.map(Number) || [];
  return [s, 0, 0, s, a, a + 6.5 * s].every((v, i) => Math.abs(v - got[i]) < 0.01);
};
// The phone of the screen recordings (2026-10-18): 915 px high, the page 839 under a status bar of
// 52 and over a gesture bar of 24.
// Chrome: Android's own splash, the launcher's icon round, 240 px in the middle of the screen, ink
// and paper in the dark too, its leaf as large as the icon's; the wordmark at the bottom.
const chrome = await start(pixel, { chrome: true, dark: true, version: 155 });
check(chrome && chrome.sys && chrome.beside === "BODY" && near(chrome.icon, [86, 285.5, 240, 240]) && leaf(chrome, 240, 1) && chrome.colours.join() === "rgb(16, 21, 31),rgb(244, 245, 247)", `Chrome's start: ${JSON.stringify(chrome)}`);
check(chrome && wordmark(chrome, 915 - 68.625 - 52) && landed(chrome) && chrome.gone && !chrome.again, `Chrome's start, its wordmark, once: ${JSON.stringify(chrome)}`);
// Brave on Chromium 154: Chromium's splash on the whole screen, the round picture of 128 px as it
// is (the leaf at .873), its middle 16 px above the screen's; the wordmark on its name's baseline,
// 54.5 above the screen's bottom.
const brave = await start(pixel, { brave: true, version: 154 });
check(brave && !brave.ios && !brave.sys && near(brave.icon, [142, 325.5, 128, 128]) && leaf(brave, 128, 0.873) && wordmark(brave, 915 - 54.5 - 52), `Brave's start (154): ${JSON.stringify(brave)}`);
check(brave && landed(brave) && brave.gone && !brave.again, `Brave's start goes to the logo once: ${JSON.stringify(brave)}`);
// From Chromium 155 on its splash lies between the bars, the gesture area of 32 px counted.
const later = await start(pixel, { brave: true, version: 155 });
check(later && near(later.icon, [142, 335.5, 128, 128]) && wordmark(later, 915 - 32 - 54.5 - 52), `Brave's start (155): ${JSON.stringify(later)}`);
// A status bar measured at an earlier start places it; one the bars leave no room for does not.
const measured = await start(pixel, { brave: true, version: 154, top: "p40" });
check(measured && near(measured.icon, [142, 337.5, 128, 128]), `a start under a status bar of 40: ${JSON.stringify(measured)}`);
const wrong = await start(pixel, { brave: true, version: 154, top: "p90" });
check(wrong && near(wrong.icon, [142, 325.5, 128, 128]), `a start with a status bar higher than the bars: ${JSON.stringify(wrong)}`);
const big = await start(tab);
check(big && near(big.icon, [576, 288, 128, 128]) && landed(big) && big.gone, `a tablet's start: ${JSON.stringify(big)}`);
// iOS: the launch screen of launch.rs, which stands on the whole screen, here under a status bar
// of 59 px: the icon in the screen's middle, the name's baseline 852 - 63.9 from its top.
const ios = await start(iphone, { ios: true });
check(ios && ios.ios && near(ios.icon, [132.5, 303, 128, 128]) && leaf(ios, 128, 1) && wordmark(ios, 852 - 63.9 - 59), `iOS's start: ${JSON.stringify(ios)}`);
check(ios && landed(ios) && ios.gone && !ios.again, `iOS's start goes to the logo once: ${JSON.stringify(ios)}`);
// A phone's page without a logo on the screen, and reduced motion: the picture fades, nothing flies.
const catalog = await start(pixel, { path: "/catalog" });
check(catalog && !catalog.logo && !catalog.moved && catalog.gone, `a start on the catalog: ${JSON.stringify(catalog)}`);
const calm = await start(pixel, { reduced: true });
check(calm && !calm.moved && calm.gone, `a start with reduced motion: ${JSON.stringify(calm)}`);
// A tap takes it away at once.
const tapped = await start(pixel, { tap: true });
check(tapped && tapped.gone, `a tap on the start: ${JSON.stringify(tapped)}`);
// The status bar over the installed app's page, measured at its first touch: a finger's place on
// the screen less its place in the page, kept where the bars around the page leave room for it.
async function measure(device, screenY, clientY, installed = true) {
  const context = await browser.newContext(device);
  await context.addInitScript((installed) => {
    const media = window.matchMedia.bind(window);
    window.matchMedia = (query) => (query === "(display-mode: standalone)" ? { matches: installed, media: query, addEventListener() {}, removeEventListener() {} } : media(query));
  }, installed);
  const page = await context.newPage();
  await page.goto(base + "/", { waitUntil: "load" });
  const kept = await page.evaluate(([screenY, clientY]) => {
    document.body.dispatchEvent(new PointerEvent("pointerdown", { pointerType: "touch", bubbles: true, screenY, clientY }));
    return localStorage.getItem("betula.launch.top");
  }, [screenY, clientY]);
  await context.close();
  return kept;
}
check((await measure(pixel, 352, 300)) === "p52", "a touch under a status bar of 52 is kept");
check((await measure(pixel, 300, 300)) === null, "a touch that says the page begins at the screen's top is not kept");
check((await measure(pixel, 400, 300)) === null, "a status bar higher than the bars is not kept");
check((await measure(pixel, 352, 300, false)) === null, "a touch in a browser's tab is not kept");
// Nothing of it in a browser's tab, on a desktop, or on iOS in Safari.
check(!(await start(pixel, { installed: false })), "a start in Chrome's tab");
check(!(await start({ viewport: { width: 1300, height: 900 } })), "a start on a desktop");
check(!(await start(iphone, { ios: true, installed: false })), "a start in Safari");

await browser.close();
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
