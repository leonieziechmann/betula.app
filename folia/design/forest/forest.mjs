// Draws the wood behind the start page into folia/assets/birch (docs/folia/frontend.md, „The wood"):
//   node design/forest/forest.mjs
// Direction E of the prototype (owner, 2026-09-29: „Option E, organisch und ohne Kronen"): a
// Scherenschnitt in the grey of the background, a few steps darker, in two layers — thin trees
// further back, standing a little higher, and the wood in front. Per season and layer one mask,
// <season>-wood-back.svg and <season>-wood-front.svg, 3400 × 1600: the column of the page (1600 px) in
// the middle, with wood under it too, and 900 px of wood on either side of it, the ground along the
// bottom. The stylesheet puts its middle under the column's middle and its bottom on the window's,
// and colours it (`--wood-back`, `--wood-front`); what the window does not reach is cut off. The
// trees walk outwards from the column's edges, so the ones next to it are the same on every screen.
// Their path data is as short as it gets (path.mjs), and each goes out as <name>.svg.br too, brotli
// at quality 11, which the server sends to browsers that take it (`birch::brotli`).
// The trunks fade out in the top 160 px (a window taller than the wood sees them end softly): a
// gradient in the mask itself, not one laid over it by the stylesheet, which the browser draws anew
// with every restyle of the page — and dragging a panel's edge restyles all of it in every frame.
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants } from "node:zlib";
import { compact } from "./path.mjs";
import { setPrecision, wood } from "./wood.mjs";

const out = fileURLToPath(new URL("../../app/assets/birch/", import.meta.url));
const SIDE = 900, COLUMN = 1600, W = 2 * SIDE + COLUMN, H = 1600, FADE = 160;
setPrecision(1);
// Outwards from either edge of the column, and under it from its left edge to a little before its
// right one: the panels hide that part, but where they leave a gap (between two of them, or under
// a column shorter than the one beside it) the wood and its ground go on instead of stopping.
const sides = [[SIDE, -1, 0, 11], [SIDE + COLUMN, 1, W, 23], [SIDE, 1, SIDE + COLUMN - 80, 37]];
const LAYERS = {
  back: { layer: 8, s: .6, gy: H - 70, gapMin: 40, gapMax: 100, density: .15, seg: 12, markStep: 4, lean: true },
  front: { layer: 9, s: 1, gy: H - 14, gapMin: 55, gapMax: 140, density: .8, birds: true, mushrooms: true, seg: 16, markStep: 1.5, lean: true },
};

for (const season of ["spring", "summer", "autumn", "winter"]) {
  for (const [name, layer] of Object.entries(LAYERS)) {
    const p = wood({ W, H, season, form: "organic", sides, ...layer });
    const j = (a) => a.join("");
    // The trunks with their marks cut out (even-odd); everything else is filled; the twigs are lines.
    // All of it in the fade: clear at the top, whole from FADE down.
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">`
      + `<defs><linearGradient id="f" x2="0" y2="${FADE}" gradientUnits="userSpaceOnUse"><stop stop-opacity="0"/><stop offset="1"/></linearGradient></defs>`
      + `<path fill="url(#f)" fill-rule="evenodd" d="${compact(j(p.trunks) + j(p.marks))}"/>`
      + `<path fill="url(#f)" d="${compact(j(p.limbs) + j(p.leaves) + j(p.catkins) + j(p.birds) + j(p.extra) + j(p.ground))}"/>`
      + `<path fill="none" stroke="url(#f)" stroke-width="${layer.s < 1 ? .7 : 1}" stroke-linecap="round" d="${compact(j(p.twigs))}"/>`
      + `</svg>\n`;
    writeFileSync(out + `${season}-wood-${name}.svg`, svg);
    // Brotli at its best, once, here: the server hands it to browsers that take `br` as it is.
    writeFileSync(out + `${season}-wood-${name}.svg.br`, brotliCompressSync(svg, { params: { [constants.BROTLI_PARAM_QUALITY]: 11, [constants.BROTLI_PARAM_MODE]: constants.BROTLI_MODE_TEXT, [constants.BROTLI_PARAM_SIZE_HINT]: svg.length } }));
  }
}
