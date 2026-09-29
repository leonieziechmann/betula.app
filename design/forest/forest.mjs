// Draws the wood behind the start page into app/assets/birch (docs/frontend.md, „The wood"):
//   node design/forest/forest.mjs
// Direction E of the prototype (owner, 2026-09-29: „Option E, organisch und ohne Kronen"): a
// Scherenschnitt in the grey of the background, a few steps darker, in two layers — thin trees
// further back, standing a little higher, and the wood in front. Per season and layer one mask,
// <season>-wood-back.svg and <season>-wood-front.svg, 3080 × 1600: the column of the page
// (1280 px) empty in the middle, 900 px of wood on either side of it, the ground along the
// bottom. The stylesheet puts its middle under the column's middle and its bottom on the window's,
// and colours it (`--wood-back`, `--wood-front`); what the window does not reach is cut off. The
// trees walk outwards from the column's edges, so the ones next to it are the same on every screen.
// Their path data is as short as it gets (path.mjs), and each goes out as <name>.svg.br too, brotli
// at quality 11, which the server sends to browsers that take it (`birch::brotli`).
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants } from "node:zlib";
import { compact } from "./path.mjs";
import { setPrecision, wood } from "./wood.mjs";

const out = fileURLToPath(new URL("../../app/assets/birch/", import.meta.url));
const SIDE = 900, COLUMN = 1280, W = 2 * SIDE + COLUMN, H = 1600;
setPrecision(1);
const sides = [[SIDE, -1, 0, 11], [SIDE + COLUMN, 1, W, 23]];
const LAYERS = {
  back: { layer: 8, s: .6, gy: H - 70, gapMin: 40, gapMax: 100, density: .15, seg: 12, markStep: 4, lean: true },
  front: { layer: 9, s: 1, gy: H - 14, gapMin: 55, gapMax: 140, density: .8, birds: true, mushrooms: true, seg: 16, markStep: 1.5, lean: true },
};

for (const season of ["spring", "summer", "autumn", "winter"]) {
  for (const [name, layer] of Object.entries(LAYERS)) {
    const p = wood({ W, H, season, form: "organic", sides, ...layer });
    const j = (a) => a.join("");
    // The trunks with their marks cut out (even-odd); everything else is filled; the twigs are lines.
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">`
      + `<path fill-rule="evenodd" d="${compact(j(p.trunks) + j(p.marks))}"/>`
      + `<path d="${compact(j(p.limbs) + j(p.leaves) + j(p.catkins) + j(p.birds) + j(p.extra) + j(p.ground))}"/>`
      + `<path fill="none" stroke="#000" stroke-width="${layer.s < 1 ? .7 : 1}" stroke-linecap="round" d="${compact(j(p.twigs))}"/>`
      + `</svg>\n`;
    writeFileSync(out + `${season}-wood-${name}.svg`, svg);
    // Brotli at its best, once, here: the server hands it to browsers that take `br` as it is.
    writeFileSync(out + `${season}-wood-${name}.svg.br`, brotliCompressSync(svg, { params: { [constants.BROTLI_PARAM_QUALITY]: 11, [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT, [constants.BROTLI_PARAM_SIZE_HINT]: svg.length } }));
  }
}
