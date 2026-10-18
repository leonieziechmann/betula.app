// Renders the pictures of Betula's icon into folia/assets:
//   node design/logo/render-icons.mjs
// (Where there is no Edge: SMOKE_BROWSER_PATH=<chromium> node --import ./e2e/chromium.mjs design/logo/render-icons.mjs)
//
// All of them are the icon of app-icon.mjs, the paper leaf on ink (in the dark: ink on paper):
//   apple-touch-icon.png      180, full bleed: iOS rounds it itself and turns transparency black.
//   icon-192.png, icon-512.png   rounded (21 of 96), transparent corners: desktops and taskbars.
//   icon-maskable-334.png     full bleed, the glyph inside the safe circle: Android's launchers, in
//                             both manifests. Chrome asks for 83 dp there (334 px up to a density
//                             of 4.02).
//   icon-splash-512.png       the splash screen of the light theme, which Chromium draws at 128 dp
//                             (from a density of 2.61 on more than 334 px, so it takes this one and
//                             never the launcher's): the maskable one cut round, transparent outside
//                             the circle (owner, 2026-10-18: „wenn das Icon schon am Anfang rund
//                             ist … im Zweifelsfall mach halt das Icon Bild rund"). Named maskable,
//                             so that Chromium takes it before the plain ones; a browser that
//                             installs a shortcut (Brave) draws it as it is, round, and a mask of
//                             Android's shows only its middle, which is the full-bleed one's.
//   icon-splash-dark-512.png  turned over, for the splash screen of the dark theme: only the
//                             dark manifest names it (folia_app::MANIFEST_DARK). The launcher's
//                             icon is the same in both manifests, so a theme that changes never
//                             changes the app's icon on the home screen (Chrome would ask the
//                             visitor about it each time); only the splash screen turns over.
//   icon-monochrome-512.png   white on transparent, the glyph as in the maskable ones: Android's
//                             themed icons (Material You) tint it in the colours of the wallpaper.
//   favicon.ico               32 and 48 px, rounded like favicon.svg, for what cannot read the SVG.
//   favicon.svg               the tab's: rounded, and turned over where the browser is dark.
//   design/logo/app-icon.svg  the icon on its grid, for reference.
// And it prints the path and the offset the page draws the icon with (`Mark`), and the server with
// it (launch screens of iOS, link-preview cards): paste them into folia/crates/design/src/ui.rs
// when the icon changes.
// Uses the installed Edge/Chrome like the other scripts (SMOKE_BROWSER_CHANNEL, default msedge).
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chromium } from "../../e2e/node_modules/playwright-core/index.mjs";
import { DY, ICON, MASKABLE, favicon, icon } from "./app-icon.mjs";

const assets = fileURLToPath(new URL("../../assets/", import.meta.url));
const here = fileURLToPath(new URL("./", import.meta.url));

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge" });
const page = await browser.newPage({ deviceScaleFactor: 1 });

async function png(size, markup) {
  await page.setViewportSize({ width: size, height: size });
  await page.setContent(`<!doctype html><html><body style="margin:0;background:transparent"><div style="width:${size}px;height:${size}px">${markup.replace("<svg ", `<svg width="${size}" height="${size}" style="display:block" `)}</div></body></html>`);
  return page.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: size, height: size } });
}

const files = {
  "apple-touch-icon.png": await png(180, icon()),
  "icon-192.png": await png(192, icon({ radius: 21 })),
  "icon-512.png": await png(512, icon({ radius: 21 })),
  "icon-maskable-334.png": await png(334, icon({ k: MASKABLE })),
  "icon-splash-512.png": await png(512, icon({ radius: 48, k: MASKABLE })),
  "icon-splash-dark-512.png": await png(512, icon({ radius: 48, k: MASKABLE, dark: true })),
  "icon-monochrome-512.png": await png(512, icon({ k: MASKABLE, mono: true })),
};
for (const [name, bytes] of Object.entries(files)) writeFileSync(assets + name, bytes);
writeFileSync(assets + "favicon.svg", favicon());

// favicon.ico: a directory of PNG pictures (every browser since IE 11 reads those).
const frames = [[32, await png(32, icon({ radius: 21 }))], [48, await png(48, icon({ radius: 21 }))]];
const header = Buffer.alloc(6 + 16 * frames.length);
header.writeUInt16LE(0, 0);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(frames.length, 4);
let offset = header.length;
frames.forEach(([size, bytes], i) => {
  const at = 6 + 16 * i;
  header.writeUInt8(size, at);
  header.writeUInt8(size, at + 1);
  header.writeUInt8(0, at + 2);
  header.writeUInt8(0, at + 3);
  header.writeUInt16LE(1, at + 4);
  header.writeUInt16LE(32, at + 6);
  header.writeUInt32LE(bytes.length, at + 8);
  header.writeUInt32LE(offset, at + 12);
  offset += bytes.length;
});
writeFileSync(assets + "favicon.ico", Buffer.concat([header, ...frames.map(([, bytes]) => bytes)]));

writeFileSync(here + "app-icon.svg", icon({ radius: 21 }) + "\n");
await browser.close();

console.log(`// folia/crates/design/src/ui.rs
pub const LEAF: &str = "${ICON.cut}";
pub const LEAF_DY: f32 = ${DY};`);
