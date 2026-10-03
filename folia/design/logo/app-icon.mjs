// The icon of the installed app, drawn from numbers: a birch leaf, white, with the four bars of the
// mark on it as the black marks of birch bark, on the green of the birch leaf (design/logo/logo.html).
// The site keeps its mark (the square of bark in the tab, the rail, the link previews); the leaf is
// what a home screen shows. The mark's bars run out to the edges of its square, and a launcher that
// cuts a circle out of it leaves a white disc with stubs (owner, 2026-09-26: „sieht bei einer Kugel
// mit Material You wirklich nach nichts aus"); the leaf stands whole in the middle of any shape, and
// in one colour for Android's themed icons.
//
// Every picture of the icon comes from here: `render-icons.mjs` makes the icons of the manifest and
// of iOS from it and prints the paths the server draws the launch screens of iOS with
// (`folia/crates/server/src/logo.rs`). Change the icon here, then paste.
//
// The leaf is three cubic curves on its right, mirrored to the left; the bars are horizontal bands
// that enter the leaf from its edges, from the left and the right in turn, as they enter the mark's
// square. They are cut to the leaf by computing where they meet its edge, so the pictures need no
// clip paths and no masks.

// ---------- the leaf on the 96 grid ----------
// Its right half, from the tip down to the base, as cubic Bézier segments [p0, p1, p2, p3].
const RIGHT = [
  [[48, 14.7], [49.6, 23.6], [59.9, 37], [67.6, 49.2]],
  [[67.6, 49.2], [71.7, 55.3], [71.5, 60.4], [67.6, 63.1]],
  [[67.6, 63.1], [61.7, 65.9], [53.9, 67], [48, 68.2]],
];
// The bars: top, height, the side they enter from, and where they end inside the leaf. Long,
// middle, short, longest, as in the mark.
const BARS = [
  { y: 31.3, h: 4.7, side: -1, end: 48.9 },
  { y: 40.2, h: 4.7, side: 1, end: 50.4 },
  { y: 49.1, h: 4.7, side: -1, end: 38.8 },
  { y: 58, h: 4.7, side: 1, end: 42.7 },
];
const STEM = { d: "M48 65.9Q48.8 72.6 45.5 78.2", width: 3.6 };

// ---------- colours ----------
// The green of the birch leaf: the owner's tone (oklch .6867 .0996 149) at the top, deeper towards
// the bottom (oklch .47 .085 155). A vertical gradient: every row of a picture is one colour, which
// keeps the PNGs small. The leaf is white, its marks are the ink of the app.
export const GREEN_TOP = "#6dac78";
export const GREEN_BOTTOM = "#2d6945";
export const WHITE = "#ffffff";
export const INK = "#10151f";

// ---------- geometry ----------
const n = (v) => { const r = Math.round(v * 100) / 100; return String(Object.is(r, -0) ? 0 : r); };
const lerp = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
// de Casteljau: a cubic split at t, and the part of it between t0 and t1.
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
// Every segment of a side runs monotonically in y: find t for a y by bisection.
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
    out.push(part(c, y0 > top ? tAt(c, y0) : 0, y1 < bottom ? tAt(c, y1) : 1));
  }
  return out;
}
const curveTo = (c) => `C${n(c[1][0])} ${n(c[1][1])} ${n(c[2][0])} ${n(c[2][1])} ${n(c[3][0])} ${n(c[3][1])}`;
const reverse = (c) => [c[3], c[2], c[1], c[0]];

function build() {
  const right = RIGHT, left = mirrored(RIGHT), tip = RIGHT[0][0];
  // The whole leaf.
  const leaf = `M${n(tip[0])} ${n(tip[1])}${right.map(curveTo).join("")}${[...left].reverse().map((c) => curveTo(reverse(c))).join("")}Z`;
  // Each bar, cut to the leaf: in along its top, the edge down, out along its bottom.
  const band = (bar) => {
    const curves = edgeBetween(bar.side > 0 ? right : left, bar.y, bar.y + bar.h);
    return `M${n(bar.end)} ${n(bar.y)}H${n(curves[0][0][0])}${curves.map(curveTo).join("")}H${n(bar.end)}Z`;
  };
  const marks = BARS.map(band).join("");
  // The leaf with the bars cut out, as one outline (for one colour: the monochrome icon).
  const walk = (curves, bars, down) => {
    // `down`: the right side, from the tip to the base; else the left side, from the base up.
    const ends = [curves[0][0][1], curves[curves.length - 1][3][1]];
    const along = (from, to) => {
      const cs = edgeBetween(curves, Math.min(from, to), Math.max(from, to));
      return (down ? cs : [...cs].reverse().map(reverse)).map(curveTo).join("");
    };
    let d = "", y = down ? ends[0] : ends[1];
    for (const bar of [...bars].sort((a, b) => (down ? a.y - b.y : b.y - a.y))) {
      const near = down ? bar.y : bar.y + bar.h, far = down ? bar.y + bar.h : bar.y;
      const edge = edgeBetween(curves, bar.y, bar.y + bar.h);
      const out = down ? edge[edge.length - 1][3] : edge[0][0];
      d += `${along(y, near)}H${n(bar.end)}V${n(far)}H${n(out[0])}`;
      y = far;
    }
    return d + along(y, down ? ends[1] : ends[0]);
  };
  const cut = `M${n(tip[0])} ${n(tip[1])}${walk(right, BARS.filter((bar) => bar.side > 0), true)}${walk(left, BARS.filter((bar) => bar.side < 0), false)}Z`;
  return { leaf, marks, cut, stem: STEM };
}

export const ICON = build();

// ---------- pictures ----------
// How large the glyph is in the full-bleed maskable and monochrome icons: a launcher shows the
// middle 87 % of them (Chromium pads the web's maskable icon so that its safe circle, 80 %, lands
// on Android's, 66 of 108 dp; the mask shows 72 dp), so the glyph is drawn smaller by that much
// and looks as large on a phone as in the plain icons. Its far ends (the tip, the stem) stay at
// 30 % of the width from the middle, well inside the safe circle of 40 %.
export const MASKABLE = 0.873;

// The glyph (leaf, marks, stem), scaled by `k` around the middle. `mono`: one colour, white, the
// bars cut out (the monochrome icon of Android's themed icons).
export function glyph({ k = 1, mono = false } = {}) {
  const t = k === 1 ? "" : ` transform="translate(${n(48 - 48 * k)} ${n(48 - 48 * k)}) scale(${k})"`;
  const stem = `<path d="${ICON.stem.d}" fill="none" stroke="${WHITE}" stroke-width="${ICON.stem.width}" stroke-linecap="round"/>`;
  if (mono) return `<g${t}>${stem}<path d="${ICON.cut}" fill="${WHITE}"/></g>`;
  return `<g${t}>${stem}<path d="${ICON.leaf}" fill="${WHITE}"/><path d="${ICON.marks}" fill="${INK}"/></g>`;
}
// The green ground: a square, rounded by `radius` (0: full bleed).
export function ground({ radius = 0, id = "g" } = {}) {
  return `<defs><linearGradient id="${id}" x1="0" y1="0" x2="0" y2="96" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="${GREEN_TOP}"/><stop offset="1" stop-color="${GREEN_BOTTOM}"/></linearGradient></defs>`
    + `<rect width="96" height="96"${radius ? ` rx="${radius}"` : ""} fill="url(#${id})"/>`;
}
// A whole icon as an SVG document on the 96 grid.
export function icon({ radius = 0, k = 1, mono = false, size } = {}) {
  const dims = size ? ` width="${size}" height="${size}"` : "";
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96"${dims}>${mono ? "" : ground({ radius })}${glyph({ k, mono })}</svg>`;
}
