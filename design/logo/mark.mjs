// The mark of Betula, drawn from numbers: a birch leaf, white, with the four rows of the first mark
// on it as the black marks of birch bark, on the green of the birch leaf (design/logo/logo.html).
// Every picture of the mark comes from here: `render-icons.mjs` makes the icons of the site from
// it and prints the paths the app and the server draw with (`app/src/ui.rs` on the 32 grid,
// `server/src/mark.rs` on the 96 grid for the link-preview cards and the launch screens of iOS;
// `design/og/og.html` carries the 96 grid too). Change the mark here, then paste.
//
// The leaf is three cubic curves on its right, mirrored to the left; the rows are horizontal bands
// that enter the leaf from its edges, from the left and the right in turn, as the bars of the first
// mark entered their square. They are cut to the leaf by computing where they meet its edge, so
// the pictures need no clip paths and no masks: the app draws the mark inline more than once on a
// page, and ids in inline SVG would collide.

// ---------- the leaf on the 96 grid ----------
// Its right half, from the tip down to the base, as cubic Bézier segments [p0, p1, p2, p3].
const RIGHT = [
  [[48, 14.7], [49.6, 23.6], [59.9, 37], [67.6, 49.2]],
  [[67.6, 49.2], [71.7, 55.3], [71.5, 60.4], [67.6, 63.1]],
  [[67.6, 63.1], [61.7, 65.9], [53.9, 67], [48, 68.2]],
];
// The rows: top, height, the side they enter from, and where they end inside the leaf.
const ROWS_96 = [
  { y: 31.3, h: 4.7, side: -1, end: 48.9 },
  { y: 40.2, h: 4.7, side: 1, end: 50.4 },
  { y: 49.1, h: 4.7, side: -1, end: 38.8 },
  { y: 58, h: 4.7, side: 1, end: 42.7 },
];
const STEM_96 = { d: "M48 65.9Q48.8 72.6 45.5 78.2", width: 3.6 };

// The small cuts: the same leaf, scaled; the rows on whole pixels (heavier than scaled, as the bars
// of the first mark were at small sizes), the stem a little stronger. 16 px keeps three rows.
const SMALL = {
  32: { scale: 1 / 3, rows: [{ y: 10, h: 2, side: -1, end: 16.3 }, { y: 13, h: 2, side: 1, end: 16.8 }, { y: 16, h: 2, side: -1, end: 12.9 }, { y: 19, h: 2, side: 1, end: 14.2 }], stem: { d: "M16 22Q16.27 24.2 15.17 26.07", width: 1.5 } },
  16: { scale: 1 / 6, rows: [{ y: 5, h: 1, side: -1, end: 8.2 }, { y: 7, h: 1, side: 1, end: 8 }, { y: 9, h: 1, side: -1, end: 6.4 }], stem: { d: "M8 11Q8.13 12.1 7.58 13.03", width: 1 } },
};

// ---------- colours ----------
// The green of the birch leaf: the owner's tone (oklch .6867 .0996 149) at the top, deeper towards
// the bottom (oklch .47 .085 155). A vertical gradient: every row of a picture is one colour, which
// keeps the PNGs small. The leaf is white, the marks are the ink of the app.
export const GREEN_TOP = "#6dac78";
export const GREEN_BOTTOM = "#2d6945";
export const WHITE = "#ffffff";
export const INK = "#10151f";

// ---------- geometry ----------
const n = (v) => { const r = Math.round(v * 100) / 100; return String(Object.is(r, -0) ? 0 : r); };
const lerp = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
// de Casteljau: the part of a cubic between t0 and t1.
function split(c, t) {
  const [p0, p1, p2, p3] = c;
  const a = lerp(p0, p1, t), b = lerp(p1, p2, t), d = lerp(p2, p3, t), e = lerp(a, b, t), f = lerp(b, d, t), g = lerp(e, f, t);
  return [[p0, a, e, g], [g, f, d, p3]];
}
function part(c, t0, t1) {
  const [, right] = split(c, t0);
  return split(right, (t1 - t0) / (1 - t0))[0];
}
const at = (c, t) => split(c, t)[0][3];
// Every segment of the right side falls monotonically in y: find t for a y by bisection.
function tAt(c, y) {
  let lo = 0, hi = 1;
  for (let i = 0; i < 60; i++) { const mid = (lo + hi) / 2; if (at(c, mid)[1] < y) lo = mid; else hi = mid; }
  return (lo + hi) / 2;
}
// The left side: the right one mirrored at the axis of the leaf.
const mirrored = (curves) => curves.map((c) => c.map(([x, y]) => [2 * curves[0][0][0] - x, y]));
// The edge of one side between two heights, as curves (it may cross a joint).
function edgeBetween(curves, y0, y1) {
  const out = [];
  for (const c of curves) {
    const top = c[0][1], bottom = c[3][1];
    if (bottom <= y0 || top >= y1) continue;
    const t0 = y0 > top ? tAt(c, y0) : 0, t1 = y1 < bottom ? tAt(c, y1) : 1;
    out.push(part(c, t0, t1));
  }
  return out;
}
const curveTo = (c) => `C${n(c[1][0])} ${n(c[1][1])} ${n(c[2][0])} ${n(c[2][1])} ${n(c[3][0])} ${n(c[3][1])}`;
const reverse = (c) => [c[3], c[2], c[1], c[0]];

function build(grid) {
  const s = grid === 96 ? 1 : SMALL[grid].scale;
  const right = RIGHT.map((c) => c.map(([x, y]) => [x * s, y * s]));
  const left = mirrored(right);
  const rows = grid === 96 ? ROWS_96 : SMALL[grid].rows;
  const stem = grid === 96 ? STEM_96 : SMALL[grid].stem;
  const tip = right[0][0];
  // The whole leaf.
  const leaf = `M${n(tip[0])} ${n(tip[1])}${right.map(curveTo).join("")}${[...left].reverse().map((c) => curveTo(reverse(c))).join("")}Z`;
  // Each row, cut to the leaf: in along its top, the edge down, out along its bottom.
  const band = (r) => {
    const curves = edgeBetween(r.side > 0 ? right : left, r.y, r.y + r.h);
    const first = curves[0][0];
    return `M${n(r.end)} ${n(r.y)}H${n(first[0])}${curves.map(curveTo).join("")}H${n(r.end)}Z`;
  };
  const marks = rows.map(band).join("");
  // The leaf with the rows cut out, as one outline (for one colour: the monochrome icon).
  const walk = (curves, rowsOnSide, down) => {
    // `down`: the right side, from the tip to the base; else the left side, from the base up.
    const ends = [curves[0][0][1], curves[curves.length - 1][3][1]];
    const along = (from, to) => {
      const cs = edgeBetween(curves, Math.min(from, to), Math.max(from, to));
      return (down ? cs : [...cs].reverse().map(reverse)).map(curveTo).join("");
    };
    let d = "", y = down ? ends[0] : ends[1];
    for (const r of [...rowsOnSide].sort((a, b) => (down ? a.y - b.y : b.y - a.y))) {
      const near = down ? r.y : r.y + r.h, far = down ? r.y + r.h : r.y;
      const edge = edgeBetween(curves, r.y, r.y + r.h);
      const out = down ? edge[edge.length - 1][3] : edge[0][0];
      d += `${along(y, near)}H${n(r.end)}V${n(far)}H${n(out[0])}`;
      y = far;
    }
    return d + along(y, down ? ends[1] : ends[0]);
  };
  const cut = `M${n(tip[0])} ${n(tip[1])}${walk(right, rows.filter((r) => r.side > 0), true)}${walk(left, rows.filter((r) => r.side < 0), false)}Z`;
  return { leaf, marks, cut, stem };
}

export const MARK = { 96: build(96), 32: build(32), 16: build(16) };

// ---------- pictures ----------
// How large the glyph is in the full-bleed maskable and monochrome icons: a launcher shows the
// middle 87 % of them (Chromium pads the web's maskable icon so that its safe circle, 80 %, lands
// on Android's, 66 of 108 dp; the mask shows 72 dp), so the glyph is drawn smaller by that much
// and looks as large on a phone as in the plain icons. Its far ends (the tip, the stem) stay at
// 30 % of the width from the middle, well inside the safe circle of 40 %.
export const MASKABLE = 0.873;

// The glyph (leaf, marks, stem) on a grid, scaled by `k` around its middle. `mono`: one colour,
// white, the rows cut out (the monochrome icon of Android's themed icons).
export function glyph({ grid = 96, k = 1, mono = false } = {}) {
  const m = MARK[grid], c = grid / 2;
  const t = k === 1 ? "" : ` transform="translate(${n(c - c * k)} ${n(c - c * k)}) scale(${k})"`;
  const stem = `<path d="${m.stem.d}" fill="none" stroke="${WHITE}" stroke-width="${m.stem.width}" stroke-linecap="round"/>`;
  if (mono) return `<g${t}>${stem}<path d="${m.cut}" fill="${WHITE}"/></g>`;
  return `<g${t}>${stem}<path d="${m.leaf}" fill="${WHITE}"/><path d="${m.marks}" fill="${INK}"/></g>`;
}
// The green ground: a square, rounded by `radius` (0: full bleed).
export function ground({ grid = 96, radius = 0, id = "g" } = {}) {
  return `<defs><linearGradient id="${id}" x1="0" y1="0" x2="0" y2="${grid}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="${GREEN_TOP}"/><stop offset="1" stop-color="${GREEN_BOTTOM}"/></linearGradient></defs>`
    + `<rect width="${grid}" height="${grid}"${radius ? ` rx="${radius}"` : ""} fill="url(#${id})"/>`;
}
// A whole icon as an SVG document.
export function icon({ grid = 96, radius = 0, k = 1, mono = false, size } = {}) {
  const dims = size ? ` width="${size}" height="${size}"` : "";
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${grid} ${grid}"${dims}>${mono ? "" : ground({ grid, radius })}${glyph({ grid, k, mono })}</svg>`;
}
