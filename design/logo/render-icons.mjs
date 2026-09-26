// Renders the pictures of the mark (mark.mjs) into app/assets and design/logo:
//   node design/logo/render-icons.mjs
// (Where there is no Edge: SMOKE_BROWSER_PATH=<chromium> node --import ./e2e/chromium.mjs design/logo/render-icons.mjs)
//
//   apple-touch-icon.png      180, full bleed: iOS rounds it itself and turns transparency black.
//   icon-192.png, icon-512.png   rounded (21 of 96), transparent corners: desktops and taskbars.
//   icon-maskable-512.png, icon-maskable-1024.png   full bleed, the glyph inside the safe circle:
//                             Android's launchers and the splash screen of the installed app, which
//                             draws the icon at about 220 dp (1024 keeps it sharp there).
//   icon-monochrome-512.png   white on transparent, the glyph as in the maskable one: Android's
//                             themed icons (Material You) tint it in the colours of the wallpaper.
//   favicon.ico               16, 32 and 48 px (the 16 and 32 grids for the first two).
//   favicon.svg               the 32 grid, for the tabs of browsers that read SVG.
//   design/logo/icon.svg, icon-32.svg, icon-16.svg   the mark on its grids, for reference.
// And it prints the paths the app and the server draw the mark with: paste them into
// app/src/ui.rs (`Mark`), server/src/mark.rs and design/og/og.html when the mark changes (then
// render og.png anew, see og.html).
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chromium } from "../../e2e/node_modules/playwright-core/index.mjs";
import { icon, MARK, MASKABLE } from "./mark.mjs";

const assets = fileURLToPath(new URL("../../app/assets/", import.meta.url));
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
  "icon-maskable-512.png": await png(512, icon({ k: MASKABLE })),
  "icon-maskable-1024.png": await png(1024, icon({ k: MASKABLE })),
  "icon-monochrome-512.png": await png(512, icon({ k: MASKABLE, mono: true })),
};
for (const [name, bytes] of Object.entries(files)) writeFileSync(assets + name, bytes);

// favicon.ico: a directory of PNG pictures (every browser since IE 11 reads those).
const frames = [[16, await png(16, icon({ grid: 16, radius: 3.5 }))], [32, await png(32, icon({ grid: 32, radius: 7 }))], [48, await png(48, icon({ radius: 21 }))]];
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

// The SVG pictures.
writeFileSync(assets + "favicon.svg", icon({ grid: 32, radius: 7 }) + "\n");
writeFileSync(here + "icon.svg", icon({ radius: 21 }) + "\n");
writeFileSync(here + "icon-32.svg", icon({ grid: 32, radius: 7 }) + "\n");
writeFileSync(here + "icon-16.svg", icon({ grid: 16, radius: 3.5 }) + "\n");

console.log(Object.keys(files).concat("favicon.ico", "favicon.svg").map((name) => "app/assets/" + name).join("\n"));
console.log("\nThe paths of the mark (app/src/ui.rs on the 32 grid, server/src/mark.rs on the 96 grid):");
for (const grid of [96, 32]) {
  const m = MARK[grid];
  console.log(`\n${grid}:\n  leaf  ${m.leaf}\n  marks ${m.marks}\n  cut   ${m.cut}\n  stem  ${m.stem.d} (width ${m.stem.width})`);
}
