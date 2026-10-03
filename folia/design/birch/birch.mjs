// Draws the birch around the app into folia/assets/birch — the crown along the top, the roots of the
// ground at the end of a page (docs/folia/frontend.md, „The birch“):
//   node design/birch/birch.mjs
// Every file is a mask: shape and nothing else. The stylesheet colours it (`--crown`, `--ground-root`),
// so one drawing serves both themes and the colour can change without drawing anew. The crown is a
// cut-out silhouette in two depths: what hangs behind is drawn at half strength, what hangs in front
// at full. It is drawn once per season, the same twigs every time — the tree changes, not its shape:
//   <season>-crown.svg  1200 × 64, repeats along the whole top, on every screen the same: the edge
//                        of the crown, dense at the top and hanging deeper and shallower in long waves
//                        (14 to 42 px, nothing below 46), with a long twig here and there. What stands
//                        in front of it (the mark, the search, 8–48 px) simply covers it; nothing hangs
//                        out below the search, where a tip would look like a crumb (owner, 2026-09-25:
//                        „durchgehend und auf allen Geräten“ — the first draft hung only where nothing
//                        stood and fell apart into clumps with a thin edge between them).
//   <season>-crown-head.svg  420 × 64, at the left end, before the tile: the mark, and over the
//                        title a clearing, where only the edge of the crown hangs in (down to 12 px)
//                        and the twigs at its sides lean away from it, so the name stands free as it
//                        did before (owner, 2026-09-25); its last part is the tile's own end, so the
//                        seam does not show. A phone has no title in its bar and shows the tile alone.
//   <season>-card-head.svg  300 × 64, the same at the left end of a link-preview card, which hangs
//                        the crown from its top at 1.5 times the size (folia/crates/server/src/cards.rs): the
//                        clearing just as wide as the card's wordmark („viel zu weit und links etwas
//                        daneben" was the site's head there, owner, 2026-09-26); not served.
//   <season>-og-head.svg  360 × 64, the same for the standard picture's larger wordmark
//                        (design/og/og.html).
//   spring-*-ck.svg      the catkins of April, coloured apart (`--crown-ck`).
//   roots.svg          1200 × 200, repeats along the ground, fading downwards.
//   litter.svg         1200 × 24, the leaves of October lying on the ground's edge.
// Deterministic: the same numbers draw the same tree.
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const out = fileURLToPath(new URL("../../app/assets/birch/", import.meta.url));
const H = 64;

// ---------- numbers, chance ----------
const n1 = (v) => { const r = Math.round(v * 10) / 10; return String(Object.is(r, -0) ? 0 : r); };
const n3 = (v) => { const r = Math.round(v * 1000) / 1000; return String(Object.is(r, -0) ? 0 : r); };
function rng(seed) {
  let a = seed >>> 0;
  return () => { a = (a + 0x6D2B79F5) >>> 0; let t = a; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}
const between = (r, a, b) => a + (b - a) * r();
const weighted = (r, pairs) => { let x = r() * pairs.reduce((s, p) => s + p[1], 0); for (const [v, w] of pairs) { if ((x -= w) <= 0) return v; } return pairs[pairs.length - 1][0]; };
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));

// ---------- the leaf of the silver birch ----------
// Blade 100 units long, hanging from its petiole at (0,0): the petiole runs to (0,P), the blade to
// (0,P+100). Rhombic to deltoid, widest low, a drawn-out tip, doubly serrate with the teeth
// leaning towards the tip. Three variants, so no two neighbours are the same leaf.
const SHAPES = {
  a: { W: 36, P: 34, bulge: .26, tipPow: 1.3, asym: .95, teeth: 10, amp: 3.6 },
  b: { W: 31, P: 38, bulge: .3, tipPow: 1.5, asym: .97, teeth: 11, amp: 3.3 },
  c: { W: 40, P: 30, bulge: .22, tipPow: 1.15, asym: .93, teeth: 9, amp: 3.8 },
};
const SHAPE_MIX = [["a", .45], ["b", .3], ["c", .25]];
const halfWidth = (o, t) => t <= 0 ? 0 : t < o.bulge ? Math.pow(Math.sin((t / o.bulge) * Math.PI / 2), 0.7) : Math.pow(Math.max(0, 1 - t) / (1 - o.bulge), o.tipPow);
function leafEdge(o, s) {
  const w = s > 0 ? o.W : o.W * o.asym;
  const pt = (t) => [s * halfWidth(o, t) * w, o.P + 100 * t];
  const normal = (t) => {
    const e = .002, a = pt(Math.max(0, t - e)), b = pt(Math.min(1, t + e));
    const dx = b[0] - a[0], dy = b[1] - a[1], L = Math.hypot(dx, dy) || 1;
    return s > 0 ? [dy / L, -dx / L] : [-dy / L, dx / L];
  };
  const pts = [], t0 = .13, t1 = .965, dt = (t1 - t0) / o.teeth;
  for (let i = 0; i <= 5; i++) pts.push(pt((i / 5) * t0));
  for (let k = 0; k < o.teeth; k++) {
    const a = t0 + k * dt, A = o.amp * (k % 2 ? .5 : 1) * (1 - .5 * k / o.teeth);
    pts.push(pt(a + .4 * dt));
    const tt = a + .78 * dt, p = pt(tt), q = normal(tt);
    pts.push([p[0] + q[0] * A, p[1] + q[1] * A + A * .3]);
    pts.push(pt(a + dt));
  }
  pts.push([0, o.P + 100]);
  return pts;
}
const pathOf = (pts, close = true) => pts.map((p, i) => (i ? "L" : "M") + n1(p[0]) + " " + n1(p[1])).join("") + (close ? "Z" : "");
const leafPath = (o) => { const R = leafEdge(o, 1), L = leafEdge(o, -1); return pathOf([...R, ...L.slice(1, -1).reverse()]); };

const DEFS = {
  "lf-a": `<g id="lf-a"><path d="M0 0Q3.6 17 0 37" fill="none" stroke="#000" stroke-width="6" stroke-linecap="round"/><path d="${leafPath(SHAPES.a)}"/></g>`,
  "lf-b": `<g id="lf-b"><path d="M0 0Q3.1 19 0 41" fill="none" stroke="#000" stroke-width="6" stroke-linecap="round"/><path d="${leafPath(SHAPES.b)}"/></g>`,
  "lf-c": `<g id="lf-c"><path d="M0 0Q4 15 0 33" fill="none" stroke="#000" stroke-width="6" stroke-linecap="round"/><path d="${leafPath(SHAPES.c)}"/></g>`,
  // male catkins: slim in summer, autumn and winter, long and a little bent when they flower in April
  ck: `<g id="ck"><path d="M0 0Q3 6 0 13" fill="none" stroke="#000" stroke-width="6" stroke-linecap="round"/><path d="M-8 17Q-8 11 0 11Q8 11 8 17L8 92Q8 101 0 101Q-8 101 -8 92Z"/></g>`,
  cks: `<g id="cks"><path d="M0 0Q2 5 0 10" fill="none" stroke="#000" stroke-width="6" stroke-linecap="round"/><path d="M-7 12Q0 7 7 12Q9 55 12 92Q12 101 6 100Q2 99 1 92Q-3 55 -7 12Z"/></g>`,
  // the winged nutlet of autumn
  sd: `<g id="sd"><ellipse cx="-9" cy="0" rx="9" ry="6" fill-opacity=".55"/><ellipse cx="9" cy="0" rx="9" ry="6" fill-opacity=".55"/><ellipse cx="0" cy="0" rx="3.4" ry="5.5"/></g>`,
};

// ---------- twigs ----------
function catmull(ctrl, per = 10) {
  const pts = [];
  for (let i = 0; i < ctrl.length - 1; i++) {
    const p0 = ctrl[Math.max(0, i - 1)], p1 = ctrl[i], p2 = ctrl[i + 1], p3 = ctrl[Math.min(ctrl.length - 1, i + 2)];
    for (let k = 0; k < per; k++) {
      const t = k / per, t2 = t * t, t3 = t2 * t;
      pts.push([0, 1].map((j) => .5 * ((2 * p1[j]) + (-p0[j] + p2[j]) * t + (2 * p0[j] - 5 * p1[j] + 4 * p2[j] - p3[j]) * t2 + (-p0[j] + 3 * p1[j] - 3 * p2[j] + p3[j]) * t3)));
    }
  }
  pts.push(ctrl[ctrl.length - 1]);
  return pts;
}
const lengthOf = (pts) => pts.reduce((s, p, i) => i ? s + Math.hypot(p[0] - pts[i - 1][0], p[1] - pts[i - 1][1]) : 0, 0);
function at(pts, d) {
  let acc = 0;
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1], b = pts[i], L = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (acc + L >= d || i === pts.length - 1) {
      const f = L ? clamp((d - acc) / L, 0, 1) : 0;
      return { x: a[0] + (b[0] - a[0]) * f, y: a[1] + (b[1] - a[1]) * f, dir: Math.atan2(b[1] - a[1], b[0] - a[0]) * 180 / Math.PI };
    }
    acc += L;
  }
  const p = pts[pts.length - 1];
  return { x: p[0], y: p[1], dir: 90 };
}
function taper(pts, w0, w1) {
  const n = pts.length, left = [], right = [];
  for (let i = 0; i < n; i++) {
    const p = pts[i], a = pts[Math.max(0, i - 1)], b = pts[Math.min(n - 1, i + 1)];
    let dx = b[0] - a[0], dy = b[1] - a[1]; const L = Math.hypot(dx, dy) || 1; dx /= L; dy /= L;
    const w = (w0 + (w1 - w0) * (i / (n - 1))) / 2;
    left.push([p[0] - dy * w, p[1] + dx * w]); right.push([p[0] + dy * w, p[1] - dx * w]);
  }
  return pathOf([...left, ...right.reverse()]);
}
// A twig that hangs: gravity pulls it towards `target`, and a birch twig bends a little at every node.
function grow(r, { x, y, angle, length, target = 90, droop = .04, wiggle = .03, step = 2, kink = 0 }) {
  let a = angle * Math.PI / 180; const t = target * Math.PI / 180; const pts = [[x, y]];
  let side = 1, nextKink = between(r, 6, 10);
  for (let d = 0; d < length; d += step) {
    a += (t - a) * droop + (r() - .5) * wiggle;
    if (kink && d >= nextKink) { a += side * kink; side = -side; nextKink += between(r, 6, 10); }
    x += Math.cos(a) * step; y += Math.sin(a) * step; pts.push([x, y]);
  }
  return pts;
}

// ---------- leaves on twigs ----------
// A leaf seen a little from the side is narrower; some show their other half.
const turn = (r) => (r() < .28 ? -1 : 1) * (r() < .35 ? between(r, .55, .8) : between(r, .88, 1));
// A short shoot: one to three leaves from one node, fanned, the first the largest.
function shoot(r, node, side, o) {
  const n = weighted(r, o.cluster ?? [[1, .5], [2, .36], [3, .14]]);
  const base = -side * between(r, o.tilt[0], o.tilt[1]) + (node.dir - 90) * (o.follow ?? .3);
  const L = between(r, o.len[0], o.len[1]) * (o.scale ?? 1);
  const fan = [0, side * between(r, 24, 40), -side * between(r, 30, 50)], size = [1, between(r, .72, .88), between(r, .56, .7)];
  const leaves = [];
  for (let k = 0; k < n; k++) leaves.push({ x: node.x, y: node.y, rot: clamp(base + fan[k] + between(r, -5, 5), -80, 80), len: L * size[k], sx: turn(r), shape: weighted(r, SHAPE_MIX) });
  return leaves;
}
// Does the leaf stay above the floor (the lowest y the crown may reach at x)?
function fits(l, floor) {
  const dir = (90 + l.rot) * Math.PI / 180, total = l.len * 1.34, halfW = l.len * .3 * Math.abs(l.sx ?? 1);
  for (const f of [.3, .45, .6, .75, .9, 1]) {
    const x = l.x + Math.cos(dir) * total * f, y = l.y + Math.sin(dir) * total * f;
    const hw = f < .6 ? halfW : halfW * (1 - f) / .4;
    if (y > floor(x) || y > floor(x - hw) || y > floor(x + hw)) return false;
  }
  return true;
}
// Everything drawn is an item with its horizontal extent, so a repeating tile can wrap it around.
const leafItem = (l) => ({ x0: l.x - l.len * 1.4, x1: l.x + l.len * 1.4, svg: (dx = 0) => `<use href="#lf-${l.shape}" transform="translate(${n1(l.x + dx)} ${n1(l.y)}) rotate(${n1(l.rot)}) scale(${n3(l.len / 100 * (l.sx ?? 1))} ${n3(l.len / 100)})"/>`, use: `lf-${l.shape}` });
const catkinItem = (c) => ({ x0: c.x - c.len, x1: c.x + c.len, svg: (dx = 0) => `<use href="#${c.sym}" transform="translate(${n1(c.x + dx)} ${n1(c.y)}) rotate(${n1(c.rot)}) scale(${n3(c.w / 16)} ${n3(c.len / 101)})"/>`, use: c.sym });
const seedItem = (x, y, rot, s) => ({ x0: x - 5, x1: x + 5, svg: (dx = 0) => `<use href="#sd" transform="translate(${n1(x + dx)} ${n1(y)}) rotate(${n1(rot)}) scale(${n3(s)})"/>`, use: "sd" });
const twigItem = (pts, w0, w1) => ({ x0: Math.min(...pts.map((p) => p[0])) - 2, x1: Math.max(...pts.map((p) => p[0])) + 2, svg: (dx = 0) => `<path d="${taper(pts.map(([x, y]) => [x + dx, y]), w0, w1)}"/>` });

// A hanging twig with side twigs, short shoots of leaves and catkins at its tip.
function twigSystem(r, s, layer, depth = 0) {
  const pts = grow(r, s);
  layer.twigs.push(twigItem(pts, s.w0, s.w1));
  const total = lengthOf(pts);
  let side = r() < .5 ? 1 : -1;
  for (let d = s.first ?? between(r, 3, 7); d < total - 1; d += between(r, s.gap[0], s.gap[1])) {
    const node = at(pts, d), t = d / total;
    if (depth < (s.depth ?? 1) && t < .8 && r() < (s.branch ?? 0)) {
      twigSystem(r, { ...s, x: node.x, y: node.y, angle: node.dir + side * between(r, 28, 60), length: Math.max(6, (total - d) * between(r, .35, .75)), w0: s.w0 + (s.w1 - s.w0) * t, w1: Math.max(.45, s.w1 * .85), first: between(r, 2, 5), kink: s.kink ?? .12 }, layer, depth + 1);
    } else if (!s.bare) {
      for (const l of shoot(r, node, side, s)) if (fits(l, s.floor)) layer.leaves.push(leafItem(l));
    }
    side = -side;
  }
  if (s.ck && r() < s.ck.p) {
    const tip = pts[pts.length - 1], dirTip = at(pts, total).dir;
    const k = 1 + Math.floor(r() * (s.ck.max ?? 2.4));
    for (let i = 0; i < k; i++) {
      const c = { x: tip[0] + between(r, -1, 1), y: tip[1] - between(r, 0, 2.5), rot: clamp(dirTip - 90, -30, 30) * .5 + between(r, -14, 14), len: between(r, s.ck.len[0], s.ck.len[1]), w: between(r, s.ck.w[0], s.ck.w[1]), sym: s.ck.sym };
      if (c.y + c.len * .95 <= s.floor(c.x) + (s.ck.slack ?? 1)) (s.ck.apart ? layer.apart : layer.catkins).push(catkinItem(c));
    }
  }
}

// ---------- the seasons ----------
const SEASONS = {
  spring: { bare: false, scale: .64, ck: { p: .55, len: [15, 20], w: [2.6, 3.1], sym: "cks", max: 1.8, apart: true }, fall: false, seeds: 0 },
  summer: { bare: false, scale: 1, ck: { p: .32, len: [8.5, 11], w: [2.1, 2.5], sym: "ck" }, fall: false, seeds: 0 },
  autumn: { bare: false, scale: 1, ck: { p: .45, len: [9.5, 12.5], w: [2.3, 2.7], sym: "ck" }, fall: true, seeds: 5 },
  winter: { bare: true, scale: 1, ck: { p: .66, len: [9.5, 12.5], w: [2.3, 2.7], sym: "ck" }, fall: false, seeds: 2 },
};
const layers = () => ({ back: { twigs: [], leaves: [], catkins: [], apart: [] }, front: { twigs: [], leaves: [], catkins: [], apart: [] } });

// Twigs coming in from above the edge, deeper where the piece allows (`reach`).
function strands(r, S, season, L, { from, to, every, reach, floor, frontShare = .4, angles = () => [60, 120] }) {
  for (let x = from; x < to; x += between(r, every[0], every[1])) {
    const deep = reach(x, r), y0 = between(r, -26, -8), isFront = r() < frontShare;
    const layer = isFront ? L.front : L.back;
    const [a0, a1] = angles(x);
    twigSystem(r, {
      x, y: y0, angle: between(r, a0, a1), length: Math.max(8, deep - y0 - 4), droop: .07, wiggle: .05, kink: .1,
      w0: isFront ? 1.7 : 1.3, w1: .5, gap: isFront ? [5, 9] : [4.5, 8], len: isFront ? [10, 16] : [9, 14], tilt: [15, 52], follow: .45,
      floor, scale: S.scale, branch: S.bare ? .4 : .22, depth: S.bare ? 2 : 1, bare: S.bare,
      ck: (isFront || season === "spring") && deep > 18 ? S.ck : null,
    }, layer);
  }
}
// The mass of the crown right at the edge, closed all along (smaller leaves stand closer); in
// winter a mass of fine twigs instead.
function mass(r, S, L, { from, to, floor }) {
  if (S.bare) {
    for (let x = from; x < to; x += between(r, 2.5, 5.5)) {
      const isFront = r() < .4;
      twigSystem(r, { x, y: between(r, -10, -2), angle: between(r, 55, 125), length: between(r, 6, 13), droop: .08, wiggle: .1, kink: .15, w0: .9, w1: .4, gap: [4, 7], len: [0, 0], tilt: [0, 0], floor, bare: true, branch: .3, depth: 1, ck: isFront ? { ...S.ck, p: .1 } : null }, isFront ? L.front : L.back);
    }
    return;
  }
  for (let x = from; x < to; x += between(r, 5, 10) * S.scale) {
    const isFront = r() < .36;
    const l = { x, y: between(r, -12, 0), rot: between(r, -55, 55), len: between(r, 10, 17) * S.scale, sx: turn(r), shape: weighted(r, SHAPE_MIX) };
    if (fits(l, floor)) (isFront ? L.front : L.back).leaves.push(leafItem(l));
  }
}

// One file: the back at half strength, the front at full; `wrap` repeats what crosses the edges.
function mask(width, height, L, { wrap = false, part = "leaves", extra = [] } = {}) {
  const used = new Set(), draw = (items) => {
    let svg = "";
    for (const item of items) {
      if (item.use) used.add(item.use);
      svg += item.svg(0);
      if (wrap && item.x1 > width) svg += item.svg(-width);
      if (wrap && item.x0 < 0) svg += item.svg(width);
    }
    return svg;
  };
  const body = part === "catkins"
    ? draw([...L.back.apart, ...L.front.apart])
    : `<g opacity=".5">${draw(L.back.twigs)}${draw(L.back.leaves)}${draw(L.back.catkins)}</g>${draw(L.front.twigs)}${draw(L.front.leaves)}${draw(L.front.catkins)}${draw(extra)}`;
  const defs = [...used].map((id) => DEFS[id]).join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">${defs ? `<defs>${defs}</defs>` : ""}${body}</svg>\n`;
}

mkdirSync(out, { recursive: true });
const written = [];
const write = (name, svg) => { writeFileSync(out + name, svg); written.push(`${name} ${(svg.length / 1024).toFixed(1)} KB`); };

// A head: what hangs at the left end before the tile. Before the clearing the mark stands in front
// of the crown; over the clearing (`clear`: from, to) only the crown's edge hangs in, and the twigs
// at its sides lean away from it, so a name stands free; after it the crown is dense again. Then
// what hangs just before the tile's start in an endless row of tiles (the tile's end and what
// crosses its left edge), so that head and tile meet without a seam at `width`. The head's own
// twigs end well before that (`seam`): its picture is cut at `width`, the tile's picture begins there.
function head(S, season, L, extra, { seed, width, clear: [from, to], seam }) {
  const rh = rng(seed), Hd = layers();
  const floorHead = (x) => x < from - 12 ? 46 : x < from ? 28 : x < to ? 12 : x < to + 16 ? 30 : 46;
  const lean = (x) => x < from ? [95, 135] : x >= to ? [45, 85] : [60, 120];
  strands(rh, S, season, Hd, { from: -6, to: from - 2, every: [10, 16], reach: (x, r) => between(r, 26, 42), floor: floorHead, angles: lean });
  strands(rh, S, season, Hd, { from: to, to: seam, every: [9, 15], reach: (x, r) => between(r, 26, 42) * clamp((x - to + 10) / 36, .55, 1), floor: floorHead, angles: lean });
  mass(rh, S, Hd, { from: -6, to: from, floor: () => 18 });
  mass(rh, S, Hd, { from: to, to: seam, floor: () => 18 });
  // Over the clearing the edge only: smaller leaves higher up, a fine twig here and there.
  for (let x = from - 2; x < to + 2; x += between(rh, 4, 8) * S.scale) {
    const isFront = rh() < .36;
    if (S.bare) {
      twigSystem(rh, { x, y: between(rh, -8, -2), angle: between(rh, 55, 125), length: between(rh, 5, 10), droop: .08, wiggle: .1, kink: .15, w0: .9, w1: .4, gap: [4, 7], len: [0, 0], tilt: [0, 0], floor: () => 12, bare: true, branch: .3, depth: 1 }, isFront ? Hd.front : Hd.back);
      continue;
    }
    const l = { x, y: between(rh, -10, -3), rot: between(rh, -60, 60), len: between(rh, 9, 13) * S.scale, sx: turn(rh), shape: weighted(rh, SHAPE_MIX) };
    if (fits(l, () => 12)) (isFront ? Hd.front : Hd.back).leaves.push(leafItem(l));
  }
  const shift = (item, dx) => ({ ...item, x0: item.x0 + dx, x1: item.x1 + dx, svg: (d = 0) => item.svg(d + dx) });
  for (const side of ["back", "front"]) for (const part of ["twigs", "leaves", "catkins", "apart"]) for (const item of L[side][part]) {
    if (item.x1 > CW - (width - seam)) Hd[side][part].push(shift(item, width - CW));
    if (item.x0 < 0) Hd[side][part].push(shift(item, width));
  }
  return { Hd, extra: extra.filter((item) => item.x1 > CW - (width - seam)).map((item) => shift(item, width - CW)) };
}

// How deep the crown hangs at x: long waves that fit the tile a whole number of times, so the
// edges meet; between 14 and 42 px.
const CW = 1200, HEAD = 420;
const WAVES = [[2, 10, 1.3], [5, 6, 4.1], [11, 4, 2.2]];
const reachAt = (x) => clamp(25 + WAVES.reduce((sum, [k, a, phase]) => sum + a * Math.sin(2 * Math.PI * k * x / CW + phase), 0), 14, 42);
for (const [season, S] of Object.entries(SEASONS)) {
  const r = rng(101), L = layers(), floor = () => 46;
  strands(r, S, season, L, { from: 0, to: CW, every: [9, 17], reach: (x, r) => reachAt(x) * between(r, .72, 1.06), floor });
  mass(r, S, L, { from: 0, to: CW, floor: () => 18 });
  // Now and then a long twig, as a birch lets them hang.
  const rl = rng(202);
  for (let x = between(rl, 20, 90); x < CW; x += between(rl, 130, 210)) {
    strands(rl, S, season, L, { from: x, to: x + 1, every: [2, 2], reach: () => between(rl, 38, 44), floor, frontShare: .6 });
  }
  const extra = [];
  const rs = rng(505);
  if (S.fall) for (let i = 0; i < 5; i++) extra.push(leafItem({ x: between(rs, 30, CW - 30), y: between(rs, 16, 26), rot: between(rs, -160, 160), len: between(rs, 11, 13), sx: between(rs, .7, .9) * (rs() < .5 ? -1 : 1), shape: "a" }));
  for (let i = 0; i < S.seeds * 2; i++) extra.push(seedItem(between(rs, 10, CW - 10), between(rs, 22, 40), between(rs, 0, 360), between(rs, .22, .28)));
  write(`${season}-crown.svg`, mask(CW, H, L, { wrap: true, extra }));
  if (season === "spring") write("spring-crown-ck.svg", mask(CW, H, L, { wrap: true, part: "catkins" }));

  // The site's head: the mark (0–48), the clearing over the title (60–300), its far side (300–372).
  const site = head(S, season, L, extra, { seed: 303, width: HEAD, clear: [60, 300], seam: 372 });
  write(`${season}-crown-head.svg`, mask(HEAD, H, site.Hd, { extra: site.extra }));
  if (season === "spring") write("spring-crown-head-ck.svg", mask(HEAD, H, site.Hd, { part: "catkins" }));
  // The link-preview cards (folia/crates/server/src/cards.rs) hang the crown from the top of their face at 1.5
  // times the size, the logo 64 px in: the clearing over the wordmark alone, whose ink runs from 91
  // to 179 in the mask's units; the mark stands in front of the crown before it, as on the site.
  const card = head(S, season, L, extra, { seed: 404, width: 300, clear: [87, 183], seam: 252 });
  write(`${season}-card-head.svg`, mask(300, H, card.Hd, { extra: card.extra }));
  if (season === "spring") write("spring-card-head-ck.svg", mask(300, H, card.Hd, { part: "catkins" }));
  // The standard picture (design/og/og.html), the same way around its larger logo: the wordmark's
  // ink from 110 to 238.
  const og = head(S, season, L, extra, { seed: 808, width: 360, clear: [106, 242], seam: 312 });
  write(`${season}-og-head.svg`, mask(360, H, og.Hd, { extra: og.extra }));
  if (season === "spring") write("spring-og-head-ck.svg", mask(360, H, og.Hd, { part: "catkins" }));
}

// ---------- the ground ----------
// Roots as calm lines: a curve with a tip, forking once or twice; a few shallow laterals run just
// under the edge (the birch roots flat and wide). The tile repeats; the roots fade downwards.
{
  const W = 1200, RH = 200, r = rng(606), items = [];
  const line = (pts, width, opacity) => ({ x0: Math.min(...pts.map((p) => p[0])) - 3, x1: Math.max(...pts.map((p) => p[0])) + 3, svg: (dx = 0) => `<path d="${pathOf(pts.map(([x, y]) => [x + dx, y]), false)}" fill="none" stroke="#000" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round"${opacity < 1 ? ` stroke-opacity="${opacity}"` : ""}/>` });
  const tip = (x, y, radius) => ({ x0: x - 3, x1: x + 3, svg: (dx = 0) => `<circle cx="${n1(x + dx)}" cy="${n1(y)}" r="${radius}"/>` });
  for (let x = between(r, 0, 80); x < W; x += between(r, 90, 170)) {
    const dir = r() < .5 ? -1 : 1, len = between(r, 60, 140);
    const p = catmull([[x, 0], [x + dir * len * .35, between(r, 6, 12)], [x + dir * len * .7, between(r, 10, 20)], [x + dir * len, between(r, 14, 28)]], 8), e = p[p.length - 1];
    items.push(line(p, 1.1, .72), tip(e[0], e[1], 1.4));
  }
  for (let x = between(r, 4, 22); x < W; x += between(r, 22, 44)) {
    const len = between(r, 60, 150), drift = between(r, -.45, .45), ctrl = [[x, 0]];
    for (let i = 1; i <= 4; i++) ctrl.push([x + drift * len * i / 4 + between(r, -5, 5), len * i / 4]);
    const main = catmull(ctrl, 8), end = main[main.length - 1];
    items.push(line(main, 1.6, 1), tip(end[0], end[1], 1.9));
    const forks = Math.floor(between(r, 1, 3.6));
    for (let k = 0; k < forks; k++) {
      const node = at(main, lengthOf(main) * between(r, .2, .72)), side = r() < .5 ? -1 : 1;
      const flen = len * between(r, .22, .42), a = (node.dir + side * between(r, 28, 58)) * Math.PI / 180;
      const e = [node.x + Math.cos(a) * flen, node.y + Math.sin(a) * flen];
      const m = [(node.x + e[0]) / 2 - Math.sin(a) * side * between(r, 1, 4), (node.y + e[1]) / 2 + between(r, -2, 2)];
      items.push(line(catmull([[node.x, node.y], m, e], 6), 1.1, .72), tip(e[0], e[1], 1.4));
    }
  }
  let svg = "";
  for (const item of items) svg += item.svg(0) + (item.x1 > W ? item.svg(-W) : "") + (item.x0 < 0 ? item.svg(W) : "");
  write("roots.svg", `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${RH}" viewBox="0 0 ${W} ${RH}"><defs><linearGradient id="f" x1="0" y1="0" x2="0" y2="${RH}" gradientUnits="userSpaceOnUse"><stop offset=".35" stop-color="#fff"/><stop offset=".95" stop-color="#fff" stop-opacity="0"/></linearGradient><mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="${W}" height="${RH}"><rect width="${W}" height="${RH}" fill="url(#f)"/></mask></defs><g mask="url(#m)">${svg}</g></svg>\n`);
}
// The leaves of October on the ground's edge: a few drifts, lying (the ground's edge at y 12).
{
  const W = 1200, r = rng(707), items = [];
  for (const drift of [140, 470, 760, 1060]) {
    for (let i = 0; i < 3; i++) {
      items.push(leafItem({ x: drift + between(r, -26, 26), y: 9 + between(r, -.5, .5), rot: (r() < .5 ? -1 : 1) * between(r, 80, 96), len: between(r, 11, 15), sx: between(r, .45, .62) * (r() < .5 ? -1 : 1), shape: weighted(r, SHAPE_MIX) }));
    }
  }
  const used = new Set(items.map((i) => i.use));
  write("litter.svg", `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="24" viewBox="0 0 ${W} 24"><defs>${[...used].map((id) => DEFS[id]).join("")}</defs>${items.map((i) => i.svg(0)).join("")}</svg>\n`);
}

console.log(written.join("\n"));
