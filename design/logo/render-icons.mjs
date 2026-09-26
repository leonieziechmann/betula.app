// Renders the raster icons of Betula into app/assets:
//   node design/logo/render-icons.mjs
// (Where there is no Edge: SMOKE_BROWSER_PATH=<chromium> node --import ./e2e/chromium.mjs design/logo/render-icons.mjs)
//
// The site's own picture is the mark, from its grids (logo.html):
//   favicon.ico               32 and 48 px with the hairline of favicon.svg.
// What a home screen shows is the icon of the installed app, the leaf (app-icon.mjs):
//   apple-touch-icon.png      180, full bleed: iOS rounds it itself and turns transparency black.
//   icon-192.png, icon-512.png   rounded (21 of 96), transparent corners: desktops and taskbars.
//   icon-maskable-512.png, icon-maskable-1024.png   full bleed, the glyph inside the safe circle:
//                             Android's launchers and the splash screen of the installed app, which
//                             draws the icon at about 220 dp (1024 keeps it sharp there).
//   icon-monochrome-512.png   white on transparent, the glyph as in the maskable one: Android's
//                             themed icons (Material You) tint it in the colours of the wallpaper.
//   design/logo/app-icon.svg  the icon on its grid, for reference.
// And it prints the paths the server draws the launch screens of iOS with: paste them into
// server/src/logo.rs when the icon changes.
// Uses the installed Edge/Chrome like the other scripts (SMOKE_BROWSER_CHANNEL, default msedge).
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chromium } from "../../e2e/node_modules/playwright-core/index.mjs";
import { icon, ICON, MASKABLE } from "./app-icon.mjs";

const assets = fileURLToPath(new URL("../../app/assets/", import.meta.url));
const here = fileURLToPath(new URL("./", import.meta.url));
const BARK = "#ffffff";
const INK = "#10151f";

// The mark on a grid: [size, bar height, [y, length, side]...]; lengths from the left or right edge.
const GRIDS = {
  48: { bar: 4, bars: [[10, 20, "l"], [18, 14, "r"], [26, 8, "l"], [34, 22, "r"]] },
  32: { bar: 3, bars: [[7, 13, "l"], [12, 9, "r"], [17, 5, "l"], [22, 15, "r"]] },
};

function mark(grid, radius) {
  const { bar, bars } = GRIDS[grid];
  const rects = bars.map(([y, length, side]) => `<rect x="${side === "l" ? 0 : grid - length}" y="${y}" width="${length}" height="${bar}" fill="${INK}"/>`).join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${grid} ${grid}"><clipPath id="c"><rect width="${grid}" height="${grid}" rx="${radius}"/></clipPath>`
    + `<rect x=".5" y=".5" width="${grid - 1}" height="${grid - 1}" rx="${radius - 0.5}" fill="${BARK}" stroke="${INK}" stroke-opacity=".2"/><g clip-path="url(#c)">${rects}</g></svg>`;
}

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
  "icon-maskable-512.png": await png(512, icon({ k: MASKABLE })),
  "icon-maskable-1024.png": await png(1024, icon({ k: MASKABLE })),
  "icon-monochrome-512.png": await png(512, icon({ k: MASKABLE, mono: true })),
};
for (const [name, bytes] of Object.entries(files)) writeFileSync(assets + name, bytes);

// favicon.ico: a directory of PNG pictures (every browser since IE 11 reads those).
const frames = [[32, await png(32, mark(32, 7))], [48, await png(48, mark(48, 10.5))]];
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
await browser.close();

writeFileSync(here + "app-icon.svg", icon({ radius: 21 }) + "\n");

console.log(Object.keys(files).concat("favicon.ico").map((name) => "app/assets/" + name).concat("design/logo/app-icon.svg").join("\n"));
console.log(`\nThe paths of the icon (server/src/logo.rs, the 96 grid):\n  leaf  ${ICON.leaf}\n  marks ${ICON.marks}\n  stem  ${ICON.stem.d} (width ${ICON.stem.width})`);
