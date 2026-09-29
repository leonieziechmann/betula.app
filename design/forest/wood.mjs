// The birch wood behind the start page (docs/frontend.md, „The wood"): one birch, and a wood of
// them walking outwards from the edges of the column. Shared by the prototype (forest.src.html,
// where build.mjs writes it into the page) and forest.mjs, which draws the masks the site uses.
// Deterministic: the same numbers draw the same wood.

// ---------- numbers ----------
export function rng(seed) { let a = seed >>> 0; return () => { a = (a + 0x6D2B79F5) >>> 0; let t = a; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; }
export const B = (r, a, b) => a + (b - a) * r();
// Numbers in tenths; the masks of the site in whole pixels (setPrecision(1)), which is what makes
// them small enough to send.
let precision = 10;
export const setPrecision = (p) => { precision = p; };
export const f = (v) => Math.round(v * precision) / precision;
export const poly = (pts) => "M" + pts.map((p) => f(p[0]) + " " + f(p[1])).join("L") + "Z";
export const line = (pts) => "M" + pts.map((p) => f(p[0]) + " " + f(p[1])).join("L");
export function quad(p0, c, p1, n) { const o = []; for (let i = 0; i <= n; i++) { const t = i / n, u = 1 - t; o.push([u * u * p0[0] + 2 * u * t * c[0] + t * t * p1[0], u * u * p0[1] + 2 * u * t * c[1] + t * t * p1[1]]); } return o; }
export function tapered(pts, w0, w1) {
  const L = [], R = [], n = pts.length;
  for (let i = 0; i < n; i++) {
    const a = pts[Math.max(0, i - 1)], b = pts[Math.min(n - 1, i + 1)];
    const dx = b[0] - a[0], dy = b[1] - a[1], l = Math.hypot(dx, dy) || 1, nx = -dy / l, ny = dx / l, w = (w0 + (w1 - w0) * i / (n - 1)) / 2;
    L.push([pts[i][0] + nx * w, pts[i][1] + ny * w]); R.push([pts[i][0] - nx * w, pts[i][1] - ny * w]);
  }
  return poly([...L, ...R.reverse()]);
}
// A birch leaf, pointed: from its stalk at (x, y) along the angle.
export function leaf(x, y, ang, len, wid, plain) {
  const c = Math.cos(ang), s = Math.sin(ang), P = (u, v) => [x + c * u - s * v, y + s * u + c * v];
  if (plain) return poly([P(0, 0), P(len * .42, wid * .5), P(len, 0), P(len * .42, -wid * .5)]);
  return poly([P(0, 0), P(len * .18, wid * .3), P(len * .42, wid * .5), P(len, 0), P(len * .42, -wid * .5), P(len * .18, -wid * .3)]);
}
// A mark of the bark, a lens across the trunk.
export function lens(cx, cy, w, th, tilt) {
  const c = Math.cos(tilt), s = Math.sin(tilt), P = (u, v) => [cx + c * u - s * v, cy + s * u + c * v];
  const a = P(-w / 2, 0), b = P(w / 2, 0), t = P(0, -th), d = P(0, th * .7);
  return `M${f(a[0])} ${f(a[1])}Q${f(t[0])} ${f(t[1])} ${f(b[0])} ${f(b[1])}Q${f(d[0])} ${f(d[1])} ${f(a[0])} ${f(a[1])}Z`;
}
export const bird = (x, y, s, dir) => { // sitting on (x, y), looking along dir
  const P = (u, v) => [x + dir * u * s, y + v * s];
  return poly([P(-9, -1), P(-5, -4), P(-1, -6), P(3, -9), P(6, -9.5), P(8.5, -8), P(11, -7.6), P(8.6, -6.6), P(7.5, -4.5), P(5, -1.5), P(1, 0), P(-3, .3), P(-11, 2.5)]);
};
export const mushroom = (x, y, s) => {
  const cap = quad([x - 7 * s, y - 7 * s], [x, y - 17 * s], [x + 7 * s, y - 7 * s], 8);
  return poly([[x - 1.6 * s, y + 2], [x - 1.8 * s, y - 7 * s], ...cap, [x + 1.8 * s, y - 7 * s], [x + 1.6 * s, y + 2]]);
};

// ---------- one birch ----------
// o: x, gy (foot), top (where the trunk ends), s (size), season, form (organic: grown; plain:
// straight, a few straight limbs with small crowns; block: the trunk of the first panel, a bar with
// its marks), crown (a crown of its own at the trunk's top, in the form's shape)
export const ellipse = (cx, cy, rx, ry) => `M${f(cx - rx)} ${f(cy)}a${f(rx)} ${f(ry)} 0 1 0 ${f(2 * rx)} 0a${f(rx)} ${f(ry)} 0 1 0 ${f(-2 * rx)} 0Z`;
export function birch(r, o) {
  const s = o.s, form = o.form || "organic", block = form === "block", plain = form === "plain", organic = !block && !plain, crowned = !!o.crown;
  const w = block ? s * (10 + 4 * Math.floor(B(r, 0, 4))) : s * B(r, 10, 21);
  const lean = organic ? B(r, -.018, .018) : plain ? B(r, -.006, .006) : 0, amp = organic ? B(r, 1.5, 6) * s : 0, fq = B(r, .5, 1.3), ph = B(r, 0, 6.28);
  const hTot = o.gy - o.top, taper = block ? 0 : crowned ? (plain ? .45 : .7) : plain ? .3 : .5, flare = block ? 0 : plain ? .25 : .6;
  const cx = (t) => o.x + lean * hTot * t + amp * (Math.sin(t * Math.PI * fq + ph) - Math.sin(ph));
  const hw = (t) => w / 2 * (1 - taper * t) * (1 + flare * Math.max(0, 1 - t * hTot / (w * 1.8)) ** 2);
  const yAt = (t) => o.gy - hTot * t;
  let trunk;
  if (block) {
    const x0 = o.x - w / 2, x1 = o.x + w / 2, rad = Math.min(5 * s, w / 2), t0 = o.top;
    trunk = `M${f(x0)} ${f(o.gy + 10)}L${f(x0)} ${f(t0 + rad)}Q${f(x0)} ${f(t0)} ${f(x0 + rad)} ${f(t0)}L${f(x1 - rad)} ${f(t0)}Q${f(x1)} ${f(t0)} ${f(x1)} ${f(t0 + rad)}L${f(x1)} ${f(o.gy + 10)}Z`;
  } else {
    const R = [], L = [];
    const seg = o.seg ?? 48;
    for (let i = 0; i <= seg; i++) { const t = i / seg; R.push([cx(t) + hw(t), yAt(t)]); L.push([cx(t) - hw(t), yAt(t)]); }
    trunk = poly([[cx(0) + hw(0), o.gy + 10], ...R, ...L.reverse(), [cx(0) - hw(0), o.gy + 10]]);
  }
  const marks = [];
  for (let y = o.gy - 3 * s; y > o.top + 6; y -= (o.markStep ?? 1) * B(r, block ? 7 : 5, block ? 22 : 24) * s * (!block && y > o.gy - 40 * s ? .45 : 1)) {
    const t = (o.gy - y) / hTot, c = cx(t), h = hw(t);
    let sw = 2 * h * B(r, .3, .78); const room = h - 1.3 * s; if (sw / 2 > room) sw = room * 2; if (sw < 2) continue;
    const off = B(r, -(room - sw / 2), room - sw / 2), th = s * B(r, block ? 1.1 : .9, block ? 2.4 : 2.4) * (!block && t * hTot < 50 * s ? 1.7 : 1), tilt = block ? 0 : B(r, -.12, .12);
    marks.push(lens(c + off, y, sw, th, tilt));
    if (!block && !o.lean && r() < .3 && sw > 5) marks.push(lens(c + off + B(r, -2, 2) * s, y + th + 1.6 * s, sw * B(r, .45, .8), th * .65, tilt));
  }
  const limbs = [], twigs = [], leaves = [], catkins = [], perches = [], crowns = [];
  const leafD = { spring: .45, summer: 1, autumn: .6, winter: 0 }[o.season] * (o.density ?? 1);
  const leafL = (o.season === "spring" ? 3.6 : 5.2) * s, leafW = (o.season === "spring" ? 2.3 : 3.3) * s;
  const hangTwig = (p, sx, len) => {
    const end = [p[0] + sx * B(r, 1, 7) * s, p[1] + len], ctl = [p[0] + sx * B(r, 3, 9) * s, p[1] + len * .3];
    const pts = quad(p, ctl, end, 8);
    twigs.push(o.lean ? `M${f(p[0])} ${f(p[1])}Q${f(ctl[0])} ${f(ctl[1])} ${f(end[0])} ${f(end[1])}` : line(pts));
    for (let i = 1; i <= 8; i++) if (r() < leafD) { const q = pts[i], side = i % 2 ? 1 : -1; leaves.push(leaf(q[0], q[1], Math.PI / 2 + side * B(r, .35, 1.0), leafL * B(r, .8, 1.2), leafW * B(r, .8, 1.15), o.lean)); }
    if (o.season === "spring" && r() < .6) catkins.push(tapered(quad(end, [end[0], end[1] + 4 * s], [end[0] + sx * s, end[1] + 9 * s], 4), 2.2 * s, 1.1 * s));
  };
  // limbs: organic ones rise and then hang (the weeping birch); plain ones are straight and end in
  // a small crown; the block has none. Scars above them on the bark.
  const nb = crowned || block ? 0 : plain ? Math.round(B(r, 2, 4)) : Math.round(B(r, 3, 7));
  let side = r() < .5 ? 1 : -1;
  for (let k = 0; k < nb; k++, side = -side) {
    const t = B(r, .14, .88), y = yAt(t), c = cx(t), h = hw(t);
    if (y < 60) continue;
    marks.push(poly([[c + side * h * .1, y - 5 * s], [c + side * h * .82, y - 1.2 * s], [c + side * h * .72, y + 2.4 * s]]));
    const P0 = [c + side * h * .7, y];
    if (plain) {
      const L = s * B(r, 30, 80), a = B(r, .6, 1.05), E = [P0[0] + side * L * Math.cos(a), P0[1] - L * Math.sin(a)];
      const pts = quad(P0, [(P0[0] + E[0]) / 2, (P0[1] + E[1]) / 2], E, 6);
      limbs.push(tapered(pts, s * B(r, 2.2, 3.2), .9 * s));
      if (o.birds && k === 1) perches.push([pts[3], side]);
      if (leafD > 0) { const cr = s * B(r, 8, 15) * (o.season === "spring" ? .75 : 1); crowns.push(ellipse(E[0], E[1] - cr * .3, cr, cr * .8)); }
      continue;
    }
    const L = s * B(r, 36, 125), a = B(r, .55, 1.1);
    const pts = quad(P0, [P0[0] + side * L * .45, P0[1] - L * Math.sin(a) * .85], [P0[0] + side * L, P0[1] - L * B(r, .02, .35)], o.lean ? 8 : 14);
    limbs.push(tapered(pts, s * B(r, 2, 3.4), .5 * s));
    if (o.birds && k === 1) perches.push([pts[Math.round((pts.length - 1) * .57)], side]);
    const n = pts.length - 1;
    for (let j = Math.round(n * .3); j <= n; j += o.lean ? 2 : 2) hangTwig(pts[j], side, s * B(r, 12, 56) * (.55 + j / n));
  }
  if (crowned) {
    const ccx = cx(1), rx = o.rx, ry = o.ry, ccy = o.top + ry * .25;
    if (organic) {
      // a lacy cloud, limbs inside, a fringe hanging below
      for (let k = 0; k < 6; k++) { const side = k % 2 ? 1 : -1, t = B(r, .55, .95), p0 = [cx(t), yAt(t)], e = [ccx + side * rx * B(r, .3, .8), ccy - ry * B(r, -.1, .6)]; limbs.push(tapered(quad(p0, [p0[0] + side * rx * .15, (p0[1] + e[1]) / 2], e, 10), s * B(r, 2, 3.2), .6 * s)); }
      const n = Math.round(rx * ry * .085 * leafD);
      for (let i = 0; i < n; i++) {
        const a = r() * Math.PI * 2, d = Math.sqrt(r()), x = ccx + Math.cos(a) * d * rx, y = ccy + Math.sin(a) * d * ry * (Math.sin(a) > 0 ? 1.15 : 1);
        leaves.push(leaf(x, y, Math.PI / 2 + B(r, -1.1, 1.1), leafL * B(r, .8, 1.2), leafW));
      }
      for (let i = 0; i < 26; i++) { const a = B(r, .15, Math.PI - .15), p = [ccx + Math.cos(a) * rx * B(r, .5, 1), ccy + Math.sin(a) * ry * B(r, .5, .95)]; hangTwig(p, Math.cos(a) > 0 ? 1 : -1, s * B(r, 14, 40)); }
      if (leafD === 0) for (let i = 0; i < 40; i++) { const a = r() * Math.PI * 2, d = Math.sqrt(r()), p = [ccx + Math.cos(a) * d * rx, ccy + Math.sin(a) * d * ry]; twigs.push(line(quad([cx(.8), yAt(.8)], [(cx(.8) + p[0]) / 2, p[1] + 10], p, 6))); }
    } else if (leafD > 0) {
      const k = o.season === "spring" ? .8 : o.season === "autumn" ? .9 : 1, RX = rx * .8 * k, RY = ry * k;
      if (plain) { // an oval, narrower towards its top
        const pts = []; for (let i = 0; i < 48; i++) { const a = i / 48 * Math.PI * 2, c = Math.cos(a); pts.push([ccx + RX * Math.sin(a) * (.72 + .28 * (1 - c) / 2), ccy - RY * c * (c > 0 ? 1.08 : 1)]); }
        crowns.push(poly(pts));
      } else { // a rounded bar
        const x0 = ccx - RX * .8, x1 = ccx + RX * .8, y0 = ccy - RY, y1 = ccy + RY * .8, q = RX * .8;
        crowns.push(`M${f(x0)} ${f(y1 - q)}L${f(x0)} ${f(y0 + q)}A${f(q)} ${f(q)} 0 0 1 ${f(x1)} ${f(y0 + q)}L${f(x1)} ${f(y1 - q)}A${f(q)} ${f(q)} 0 0 1 ${f(x0)} ${f(y1 - q)}Z`);
      }
    } else if (plain) {
      for (let i = 0; i < 5; i++) { const a = -Math.PI / 2 + (i - 2) * .32, L = ry * B(r, .7, 1.1), p0 = [cx(.85), yAt(.85)]; limbs.push(tapered([p0, [p0[0] + Math.cos(a) * L * .5, p0[1] + Math.sin(a) * L * .5], [p0[0] + Math.cos(a) * L, p0[1] + Math.sin(a) * L]], 2 * s, .6 * s)); }
    }
  }
  return { trunk, marks, limbs, twigs, leaves, catkins, perches, crowns };
}

// ---------- the wood beside the column ----------
// Trees walk from the column's edge outwards, each side from its own seed, so the ones near the
// column are the same at every width; a wider screen only adds trees further out.
export function wood(o) {
  const out = { trunks: [], marks: [], limbs: [], twigs: [], leaves: [], catkins: [], crowns: [], ground: [], extra: [], snow: [], birds: [] };
  for (const [edge, dir, far, seed] of o.sides) {
    const r = rng(seed * 7919 + o.layer * 104729);
    let x = edge + dir * B(r, 8, 22) * o.s, n = 0;
    while (dir < 0 ? x > far - 40 : x < far + 40) {
      const crown = o.lacy || o.crown, gy = o.gy + B(r, -3, 3) * o.s;
      const top = o.lacy ? o.gy - B(r, .26, .4) * o.H * o.s : o.crown ? gy - B(r, o.crownLo, o.crownHi) * o.H : -60;
      const t = birch(r, { x, gy, top, s: o.s * B(r, .8, 1.12), season: o.season, form: o.lacy ? "organic" : o.form, crown, rx: B(r, o.crown ? 30 : 40, o.crown ? 50 : 75) * o.s, ry: B(r, o.crown ? 70 : 55, o.crown ? 120 : 95) * o.s, birds: o.birds && n === 1, density: o.density, seg: o.seg, markStep: o.markStep, lean: o.lean });
      out.trunks.push(t.trunk); out.marks.push(...t.marks); out.limbs.push(...t.limbs); out.twigs.push(...t.twigs); out.leaves.push(...t.leaves); out.catkins.push(...t.catkins); out.crowns.push(...t.crowns);
      for (const [p, d] of t.perches) out.birds.push(bird(p[0], p[1], o.s * .9, d));
      x += dir * B(r, o.gapMin, o.gapMax) * o.s; n++;
    }
    // ground: grass as a row of blades, a mushroom or two
    const g = []; const x0 = dir < 0 ? far - 60 : edge - 30, x1 = dir < 0 ? edge + 30 : far + 60, ph = B(r, 0, 6);
    g.push([x0, o.H + 4]);
    for (let gx = x0; gx < x1; gx += B(r, 1.6, 4.2) * o.s * (o.lean ? 1.8 : 1)) {
      const by = o.gy + 2 * Math.sin(gx / 70 + ph) * o.s;
      g.push([gx, by]); const h = o.form === "block" && !o.lacy ? 0 : B(r, 3, r() < .12 ? 20 : 11) * o.s; g.push([gx + B(r, -2.5, 2.5) * o.s + 1, by - h]); g.push([gx + 2 * o.s, by]);
    }
    g.push([x1, o.H + 4]); out.ground.push(poly(g));
    if (o.mushrooms) for (let k = 0; k < 2; k++) { const mx = dir < 0 ? B(r, Math.max(far, edge - 260), edge - 20) : B(r, edge + 20, Math.min(far, edge + 260)); out.extra.push(mushroom(mx, o.gy + 2 * Math.sin(mx / 70 + ph) * o.s, o.s * B(r, .7, 1.1))); }
    // the season in the air
    const span = Math.abs(far - edge), top = o.lacy ? o.H * .45 : 70;
    if (o.season === "autumn" && o.fall) for (let k = 0; k < span * .04; k++) out.leaves.push(leaf(edge + dir * B(r, 10, span), B(r, top, o.gy - 10), B(r, 0, 6.28), 5 * o.s, 3.4 * o.s));
    if (o.season === "winter" && o.fall) for (let k = 0; k < span * .09; k++) { const cx = edge + dir * B(r, 6, span), cy = B(r, top, o.gy), rr = B(r, .7, 1.9) * o.s; out.snow.push(`M${f(cx - rr)} ${f(cy)}a${f(rr)} ${f(rr)} 0 1 0 ${f(2 * rr)} 0a${f(rr)} ${f(rr)} 0 1 0 ${f(-2 * rr)} 0Z`); }
  }
  return out;
}

