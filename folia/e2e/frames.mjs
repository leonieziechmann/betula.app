// Checks the frames of the app (app.css, „Frames"; docs/folia/frontend.md R26; owner, 2026-11-19:
// „ein fehler, der wirklich immer und immer wieder in betula auftaucht sind abgeschnittene borders.
// Das hat zwei ursachen. Zum einen wie hier, wo der zindex nicht passt und dann zum anderen, weil der
// parent container die border abschneidet. Bitte löse das problem nachhaltig"): in the views of the
// app, its menus and its dialogs, on a wide screen and on a phone, every control the keyboard
// reaches is focused, and every frame that marks something at rest (an outline, the ring of a
// box-shadow) is looked at; each side of a frame has to be on the screen, pixel by pixel: the
// picture with the frame against the same picture with the frame taken away. A side that is not
// names what covers it (a neighbour painted after it: the layers) or what cuts it off (a parent
// that clips), or says that it does not stand out from what lies under it. A control the frame of
// which lies outside its edge is named too: the rule is inside.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node frames.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
//   FRAMES_ONLY=grid,phone-study   only the views whose names contain one of these
//   FRAMES_VERBOSE=1               every frame looked at, not only the ones cut
//   SMOKE_COOKIES=<curl cookie jar> the gate's cookie, for canary
// Fails on a console error, a view that does not come up, or a frame cut, covered or not seen.
import { chromium } from "playwright-core";
import { inflateSync } from "node:zlib";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const only = (process.env.FRAMES_ONLY || "").split(",").map((s) => s.trim()).filter(Boolean);
const verbose = !!process.env.FRAMES_VERBOSE;
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };

// ---- what a check of a frame finds

/// How many of the pixels along a side of a frame have to change when it is taken away for the side
/// to count as shown: a side cut by a pixel at a corner still counts, one cut along its length not.
const SHOWN = 0.9;
/// How far a pixel has to change (the largest of its three channels, 0–255) to count as the frame's:
/// little, as the edge of a panel is a line of 8 % (nothing else changes between the two pictures).
const CHANGED = 6;

/// A PNG of the screen as RGBA (Playwright's screenshots: 8 bits, RGB or RGBA, no interlace).
const decode = (buffer) => {
  let at = 8, width = 0, height = 0, depth = 0, type = 0;
  const parts = [];
  while (at < buffer.length) {
    const length = buffer.readUInt32BE(at);
    const kind = buffer.toString("latin1", at + 4, at + 8);
    const data = buffer.subarray(at + 8, at + 8 + length);
    if (kind === "IHDR") { width = data.readUInt32BE(0); height = data.readUInt32BE(4); depth = data[8]; type = data[9]; }
    else if (kind === "IDAT") parts.push(data);
    else if (kind === "IEND") break;
    at += 12 + length;
  }
  const channels = { 0: 1, 2: 3, 4: 2, 6: 4 }[type];
  if (depth !== 8 || !channels) throw new Error(`a screenshot of depth ${depth} and type ${type}`);
  const raw = inflateSync(Buffer.concat(parts));
  const stride = width * channels;
  const out = new Uint8Array(width * height * 3);
  let above = new Uint8Array(stride);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    const row = new Uint8Array(stride);
    for (let x = 0; x < stride; x++) {
      const a = x >= channels ? row[x - channels] : 0, b = above[x], c = x >= channels ? above[x - channels] : 0;
      let v = line[x];
      if (filter === 1) v += a;
      else if (filter === 2) v += b;
      else if (filter === 3) v += (a + b) >> 1;
      else if (filter === 4) { const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c); v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c; }
      row[x] = v & 255;
    }
    for (let x = 0; x < width; x++) {
      const o = (y * width + x) * 3;
      for (let k = 0; k < 3; k++) out[o + k] = row[x * channels + (channels >= 3 ? k : 0)];
    }
    above = row;
  }
  return { width, height, data: out };
};

// ---- in the page: the controls, their frames, taking a frame away, what covers or clips it

const inPage = () => {
  const registry = [];
  const id = (el) => { if (el.__frameId === undefined) { el.__frameId = registry.length; registry.push(el); } return el.__frameId; };
  const behind = (el) => el.closest("#content > .page-stage:not(.front)");
  const describe = (el) => {
    if (!el || el.nodeType !== 1) return String(el);
    const cls = [...el.classList].filter((c) => !/^(is-|rm-in|rm-out)/.test(c)).slice(0, 4).join(".");
    const label = (el.getAttribute("aria-label") || el.getAttribute("title") || el.textContent || "").trim().replace(/\s+/g, " ").slice(0, 32);
    return `${el.tagName.toLowerCase()}${el.id ? "#" + el.id : ""}${cls ? "." + cls : ""}${label ? ` „${label}"` : ""}`;
  };
  const shown = (el) => {
    if (behind(el) || el.closest("[inert]")) return false;
    if (!el.getClientRects().length) return false;
    const cs = getComputedStyle(el);
    if (cs.visibility !== "visible" || cs.display === "contents") return false;
    const r = el.getBoundingClientRect();
    if (r.width < 4 || r.height < 4) return false;
    // Visually hidden (a skip link until it is focused, a field read by a screen reader): no frame
    // of its own to show.
    for (let a = el; a && a !== document.body; a = a.parentElement) {
      const s = getComputedStyle(a);
      if (s.clipPath === "inset(50%)" || parseFloat(s.opacity) === 0) return false;
    }
    return true;
  };
  const ROLES = /^(button|link|menuitem|menuitemradio|menuitemcheckbox|option|tab|radio|checkbox|switch|gridcell|row|treeitem|slider|spinbutton|combobox|textbox|searchbox)$/;
  const focusable = (el) => {
    if (el.disabled) return false;
    const native = el.matches("a[href], button, input:not([type=hidden]), select, textarea, summary, [contenteditable=''], [contenteditable=true]");
    if (!native && !el.hasAttribute("tabindex")) return false;
    if (el.tabIndex >= 0) return true;
    // Not a stop of Tab: one the arrows of a region reach (enhance.js, `data-stop`; a list's rows, the
    // Gesamtplan's heads) or an item of a widget of its own (an option, a tab), as long as it is a
    // control and not a region that takes the focus to be scrolled. A picture behind the carousel's
    // front is none: its arrows bring it to the front first.
    return ROLES.test(el.getAttribute("role") || "") || el.matches("[data-stop] *, .row, .st-col-head");
  };
  const ringLayers = (shadow) => {
    if (!shadow || shadow === "none") return [];
    const layers = [];
    // Split at the commas between layers, not those inside a colour.
    let depth = 0, from = 0;
    for (let i = 0; i <= shadow.length; i++) {
      const ch = shadow[i];
      if (ch === "(") depth++;
      else if (ch === ")") depth--;
      else if ((ch === "," && depth === 0) || i === shadow.length) { layers.push(shadow.slice(from, i).trim()); from = i + 1; }
    }
    return layers.map((layer, at) => {
      const inset = /\binset\b/.test(layer);
      const lengths = (layer.replace(/(rgba?|color|oklch|oklab|color-mix|hsla?)\([^)]*\)/g, "").match(/-?[\d.]+px/g) || []).map(parseFloat);
      const [x = 0, y = 0, blur = 0, spread = 0] = lengths;
      const color = (layer.match(/(rgba?|color|oklch|oklab)\([^)]*\)/) || [""])[0];
      const clear = /rgba\([^)]*,\s*0\)$|^transparent$/.test(color) || /\/ 0\)$/.test(color);
      return { at, inset, x, y, blur, spread, clear, layer };
    }).filter((l) => l.x === 0 && l.y === 0 && l.blur === 0 && l.spread > 0 && !l.clear);
  };
  const frameOf = (el, pseudo) => {
    const cs = getComputedStyle(el, pseudo || null);
    if (pseudo && (cs.content === "none" || cs.content === "normal" || cs.display === "none")) return null;
    const outline = cs.outlineStyle !== "none" && parseFloat(cs.outlineWidth) > 0 && !/rgba\([^)]*,\s*0\)$|transparent/.test(cs.outlineColor)
      ? { width: parseFloat(cs.outlineWidth), offset: parseFloat(cs.outlineOffset), style: cs.outlineStyle, color: cs.outlineColor } : null;
    const rings = ringLayers(cs.boxShadow);
    return { outline, rings, shadow: cs.boxShadow, radius: Math.max(...["borderTopLeftRadius", "borderTopRightRadius", "borderBottomLeftRadius", "borderBottomRightRadius"].map((k) => parseFloat(cs[k]) || 0)) };
  };
  const key = (f) => f ? JSON.stringify([f.outline, f.rings.map((r) => r.layer)]) : "";
  // The box of an element or of its ::before or ::after (laid absolutely in a positioned element).
  const boxOf = (el, pseudo) => {
    const r = el.getBoundingClientRect();
    if (!pseudo) return { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
    const ps = getComputedStyle(el, pseudo), es = getComputedStyle(el);
    if (ps.position !== "absolute" || es.position === "static") return null;
    const cb = { left: r.left + parseFloat(es.borderLeftWidth), top: r.top + parseFloat(es.borderTopWidth), right: r.right - parseFloat(es.borderRightWidth), bottom: r.bottom - parseFloat(es.borderBottomWidth) };
    const px = (v) => (v === "auto" ? null : parseFloat(v));
    const [l, t, rr, b, w, h] = [px(ps.left), px(ps.top), px(ps.right), px(ps.bottom), parseFloat(ps.width), parseFloat(ps.height)];
    const left = l !== null ? cb.left + l : rr !== null ? cb.right - rr - w : cb.left;
    const top = t !== null ? cb.top + t : b !== null ? cb.bottom - b - h : cb.top;
    return { left, top, right: left + w, bottom: top + h };
  };
  // What might draw the frame of `el`: itself, its parents (a row ringed when its link is focused),
  // its first children (a tab's mark), what follows it (`~`), and their ::before and ::after.
  const candidates = (el) => {
    const list = [el];
    for (let a = el.parentElement, n = 0; a && n < 4; a = a.parentElement, n++) list.push(a);
    list.push(...[...el.querySelectorAll(":scope > *, :scope > * > *")].slice(0, 12));
    for (let s = el.nextElementSibling, n = 0; s && n < 3; s = s.nextElementSibling, n++) list.push(s);
    return list.flatMap((c) => [[c, ""], [c, "::before"], [c, "::after"]]);
  };
  // The check's own rules, in a stylesheet of its own (the page's Content-Security-Policy takes no
  // <style> but a constructed sheet): a frame taken away, and every element hit by the pointer
  // while it is asked what covers a frame.
  const OFF = "[data-frame-off]{outline-style:none!important}[data-frame-off~=shadow]{box-shadow:var(--frame-shadow-off)!important}[data-frame-off~=before]::before,[data-frame-off~=after]::after{outline-style:none!important;box-shadow:none!important}";
  const HIT = "*,*::before,*::after{pointer-events:auto!important}";
  const sheet = new CSSStyleSheet();
  sheet.replaceSync(OFF);
  const adopt = () => { if (!document.adoptedStyleSheets.includes(sheet)) document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet]; };
  const hitting = (inside) => { adopt(); sheet.replaceSync(OFF + HIT); try { return inside(); } finally { sheet.replaceSync(OFF); } };
  const clips = (a) => {
    const cs = getComputedStyle(a);
    return { x: cs.overflowX !== "visible", y: cs.overflowY !== "visible", paint: /paint|strict|content/.test(cs.contain), path: cs.clipPath !== "none" };
  };
  window.__frames = {
    describe: (n) => describe(registry[n]),
    controls(scope) {
      const root = scope ? document.querySelector(scope) : document;
      if (!root) return [];
      return [...root.querySelectorAll("*")].filter((el) => focusable(el) && shown(el)).map(id);
    },
    atRest(scope) {
      const root = scope ? document.querySelector(scope) : document.body;
      if (!root) return [];
      const found = [];
      for (const el of root.querySelectorAll("*")) {
        if (!shown(el) || el === document.activeElement) continue;
        for (const pseudo of ["", "::before", "::after"]) {
          const f = frameOf(el, pseudo);
          if (!f) continue;
          // An outline marks something, and so does a ring of 1.5 px or more; a ring of 1 px is
          // the edge of a panel or a field, which R26 lets lie around it.
          const marks = (f.outline && f.outline.width >= 1) || f.rings.some((ring) => ring.spread >= 1.5);
          if (marks) found.push({ el: id(el), pseudo });
        }
      }
      return found;
    },
    // Focuses a control as the keyboard does and says which frames came with the focus.
    focus(n) {
      const el = registry[n];
      if (!el || !el.isConnected) return { gone: true };
      // Still a stop of the keyboard (a picture of the carousel that went behind is none any more).
      if (!focusable(el) || !shown(el)) return { gone: true };
      const was = candidates(el).map(([c, p]) => ({ c, p, f: frameOf(c, p) }));
      el.focus();
      const visible = el.matches(":focus-visible");
      const holders = [];
      for (const { c, p, f: before } of was) {
        const f = frameOf(c, p);
        if (!f) continue;
        const outline = f.outline && JSON.stringify(f.outline) !== JSON.stringify(before?.outline || null);
        const rings = f.rings.filter((r) => !(before?.rings || []).some((b) => b.layer === r.layer));
        if (outline) holders.push({ el: id(c), pseudo: p, use: "outline" });
        else if (rings.length) holders.push({ el: id(c), pseudo: p, use: "rings", layers: rings.map((r) => r.layer) });
      }
      return { visible, focused: document.activeElement === el, holders };
    },
    blur() { document.activeElement?.blur?.(); },
    // The band of a frame on the screen: its outer and inner edge, the corners' radius, and how it
    // is drawn.
    band({ el: n, pseudo, use, layers }) {
      const el = registry[n];
      const f = frameOf(el, pseudo);
      const box = boxOf(el, pseudo);
      if (!f || !box) return null;
      if (use === "rings") f.outline = null;
      if (layers) f.rings = f.rings.filter((r) => layers.includes(r.layer));
      if (!f.outline && !f.rings.length) return null;
      // An inline element on several lines draws its frame per line: not judged by one box.
      if (!pseudo && el.getClientRects().length > 1 && getComputedStyle(el).display === "inline") return { lines: el.getClientRects().length };
      const grow = (b, d) => ({ left: b.left - d, top: b.top - d, right: b.right + d, bottom: b.bottom + d });
      let outer, inner, kind, inside;
      if (f.outline) {
        kind = "outline";
        outer = grow(box, f.outline.offset + f.outline.width);
        inner = grow(box, f.outline.offset);
        inside = f.outline.offset + f.outline.width <= 0.01;
      } else {
        const ring = f.rings.reduce((a, b) => (b.spread > a.spread ? b : a));
        kind = ring.inset ? "inset shadow" : "shadow";
        outer = ring.inset ? box : grow(box, ring.spread);
        inner = ring.inset ? grow(box, -ring.spread) : box;
        inside = ring.inset;
      }
      return { box, outer, inner, kind, inside, style: f.outline ? f.outline.style : "solid", radius: f.radius + Math.max(0, outer.right - box.right), width: f.outline ? f.outline.width : null, detail: f.outline ? `${f.outline.width}px ${f.outline.style}, offset ${f.outline.offset}px` : f.shadow.slice(0, 80) };
    },
    takeAway({ el: n, pseudo, use, layers }) {
      const el = registry[n];
      adopt();
      if (pseudo) { el.setAttribute("data-frame-off", pseudo.slice(2)); return; }
      const f = frameOf(el, "");
      if (use === "rings") f.outline = null;
      if (layers) f.rings = f.rings.filter((r) => layers.includes(r.layer));
      if (use === "outline") f.rings = [];
      const words = [];
      if (f.rings.length) {
        words.push("shadow");
        // Its other layers stay: only the rings go.
        const ringed = new Set(f.rings.map((r) => r.at));
        let depth = 0, from = 0;
        const layers = [];
        for (let i = 0; i <= f.shadow.length; i++) {
          const ch = f.shadow[i];
          if (ch === "(") depth++; else if (ch === ")") depth--;
          else if ((ch === "," && depth === 0) || i === f.shadow.length) { layers.push(f.shadow.slice(from, i).trim()); from = i + 1; }
        }
        const rest = layers.filter((_, at) => !ringed.has(at));
        el.style.setProperty("--frame-shadow-off", rest.length ? rest.join(", ") : "none");
      }
      if (f.outline) words.push("outline");
      el.setAttribute("data-frame-off", words.join(" ") || "outline");
    },
    putBack({ el: n }) {
      const el = registry[n];
      el.removeAttribute("data-frame-off");
      el.style.removeProperty("--frame-shadow-off");
    },
    // Why the frame of a holder is not seen at each of `points` ([x, y]): a parent that clips it,
    // what lies over it, the window, or nothing (it does not stand out from what lies under it).
    whyAll({ el: n }, points) {
      const el = registry[n];
      // The parents that clip, on the way up to the root as an element is laid out: an absolute one
      // is clipped by its containing block and what holds it, a fixed one by nothing above it.
      const clippers = [];
      let position = getComputedStyle(el).position;
      for (let a = el.parentElement; a; a = a.parentElement) {
        const as = getComputedStyle(a);
        if (position === "fixed" && as.transform === "none" && as.filter === "none" && !/paint|layout|strict|content/.test(as.contain)) continue;
        if (position === "absolute" && as.position === "static" && as.transform === "none") continue;
        position = as.position;
        const c = clips(a);
        if (!c.x && !c.y && !c.paint && !c.path) continue;
        const r = a.getBoundingClientRect();
        clippers.push({ a, c, r, pad: { left: r.left + parseFloat(as.borderLeftWidth), right: r.right - parseFloat(as.borderRightWidth), top: r.top + parseFloat(as.borderTopWidth), bottom: r.bottom - parseFloat(as.borderBottomWidth) } });
      }
      const related = (o) => o === el || el.contains(o) || o.contains(el);
      return hitting(() => points.map(([x, y]) => {
        if (x < 0 || y < 0 || x >= innerWidth || y >= innerHeight) return { by: "the window" };
        for (const { a, c, r, pad } of clippers) {
          const outX = x < pad.left || x >= pad.right, outY = y < pad.top || y >= pad.bottom;
          if (((c.x || c.paint) && outX) || ((c.y || c.paint) && outY) || (c.path && (x < r.left || x >= r.right || y < r.top || y >= r.bottom))) return { by: "clipped", what: describe(a) };
        }
        const top = document.elementFromPoint(x, y);
        const self = !!top && (top === el || el.contains(top));
        if (top && !related(top)) {
          let floating = false;
          for (let a = top; a && a !== document.body; a = a.parentElement) {
            if (a.contains(el)) break;
            if (/fixed|sticky/.test(getComputedStyle(a).position)) { floating = true; break; }
          }
          return { by: "covered", what: describe(top), floating, self };
        }
        return { by: "unseen", what: top ? describe(top) : "", self };
      }));
    },
    // Whether the element itself is under something (a head that sticks, a bar).
    under(n) {
      const el = registry[n];
      const r = el.getBoundingClientRect();
      const x = Math.min(Math.max(r.left + r.width / 2, 0), innerWidth - 1), y = Math.min(Math.max(r.top + r.height / 2, 0), innerHeight - 1);
      const top = hitting(() => document.elementFromPoint(x, y));
      return top && top !== el && !el.contains(top) && !top.contains(el) ? describe(top) : "";
    },
  };
};

// ---- a frame on the screen, side by side

const sideNames = ["top", "right", "bottom", "left"];

/// Looks at one frame: a picture with it and one without, and per side how much of it changed.
const look = async (page, holder) => {
  const band = await page.evaluate((h) => window.__frames.band(h), holder);
  if (!band) return null;
  if (band.lines) return { lines: band.lines, offscreen: true };
  const vw = page.viewportSize().width, vh = page.viewportSize().height;
  const clip = {
    x: Math.max(0, Math.floor(band.outer.left) - 2),
    y: Math.max(0, Math.floor(band.outer.top) - 2),
  };
  clip.width = Math.min(vw, Math.ceil(band.outer.right) + 2) - clip.x;
  clip.height = Math.min(vh, Math.ceil(band.outer.bottom) + 2) - clip.y;
  if (clip.width < 4 || clip.height < 4) return { band, offscreen: true };
  const shot = async () => decode(await page.screenshot({ clip, animations: "disabled", caret: "hide" }));
  const withFrame = await shot();
  await page.evaluate((h) => window.__frames.takeAway(h), holder);
  const without = await shot();
  await page.evaluate((h) => window.__frames.putBack(h), holder);
  const at = (img, x, y) => {
    const ix = Math.round(x) - clip.x, iy = Math.round(y) - clip.y;
    if (ix < 0 || iy < 0 || ix >= img.width || iy >= img.height) return null;
    return (iy * img.width + ix) * 3;
  };
  const changed = (x, y) => {
    const a = at(withFrame, x, y), b = at(without, x, y);
    if (a === null || b === null) return false;
    return Math.max(Math.abs(withFrame.data[a] - without.data[b]), Math.abs(withFrame.data[a + 1] - without.data[b + 1]), Math.abs(withFrame.data[a + 2] - without.data[b + 2])) >= CHANGED;
  };
  const { outer, inner, box } = band;
  const corner = Math.ceil(band.radius) + 3;
  // Where the window ends the frame cannot be seen; that counts against it only where the element
  // itself is in the window whole, and its frame goes out over the window's edge.
  const whole = box.left >= 0 && box.top >= 0 && box.right <= vw && box.bottom <= vh;
  const inWindow = (x, y) => x >= 0 && y >= 0 && x < vw && y < vh;
  const colour = (img, x, y) => { const o = at(img, x, y); return o === null ? null : [img.data[o], img.data[o + 1], img.data[o + 2]]; };
  const seenColours = [];
  const sides = {};
  for (const side of sideNames) {
    const across = side === "top" ? [outer.top, inner.top] : side === "bottom" ? [inner.bottom, outer.bottom] : side === "left" ? [outer.left, inner.left] : [inner.right, outer.right];
    const along = side === "top" || side === "bottom" ? [outer.left + corner, outer.right - corner] : [outer.top + corner, outer.bottom - corner];
    const mid = (across[0] + across[1]) / 2;
    // A point of the element itself beside the frame: 2 px inside its edge or its frame, whichever
    // lies further in. Where the element is not seen there either (scrolled under a bar, out of its
    // box), its frame is not judged: what hides the element hides its frame too.
    const witness = side === "top" ? Math.max(box.top, inner.top) + 2 : side === "bottom" ? Math.min(box.bottom, inner.bottom) - 2 : side === "left" ? Math.max(box.left, inner.left) + 2 : Math.min(box.right, inner.right) - 2;
    let shown = 0, index = -1;
    const missing = [];
    for (let t = along[0]; t <= along[1]; t += 2) {
      index++;
      const horizontal = side === "top" || side === "bottom";
      const [x, y] = horizontal ? [t, mid] : [mid, t];
      if (!whole && !inWindow(x, y)) continue;
      let hit = false;
      for (let a = Math.floor(across[0]); a < Math.ceil(across[1]) && !hit; a++) {
        hit = horizontal ? changed(t, a) : changed(a, t);
        if (hit && seenColours.length < 400) seenColours.push(horizontal ? colour(withFrame, t, a) : colour(withFrame, a, t));
      }
      if (hit) shown++;
      else missing.push(horizontal ? [x, y, x, witness, colour(withFrame, x, y), index, colour(withFrame, x, witness)] : [x, y, witness, y, colour(withFrame, x, y), index, colour(withFrame, witness, y)]);
    }
    if (shown || missing.length) sides[side] = { shown, missing };
  }
  // The frame's colour, as it is seen where it is: the middle of each channel.
  const middle = (k) => { const v = seenColours.filter(Boolean).map((c) => c[k]).sort((a, b) => a - b); return v.length ? v[v.length >> 1] : null; };
  const frameColour = seenColours.length ? [middle(0), middle(1), middle(2)] : null;
  return { band, sides, frameColour };
};

/// What is wrong with a frame, in words: per side how much of it is not seen where the element is,
/// and what covers it there, what cuts it off, or that it does not stand out.
const judge = async (page, holder, seen) => {
  if (!seen || seen.offscreen) return [];
  const faults = [];
  for (const side of sideNames) {
    const s = seen.sides[side];
    if (!s || !s.missing.length) continue;
    const asked = s.missing;
    const told = await page.evaluate(([h, points]) => {
      const frame = window.__frames.whyAll(h, points.map(([x, y]) => [x, y]));
      const element = window.__frames.whyAll(h, points.map(([, , wx, wy]) => [wx, wy]));
      return frame.map((f, i) => ({ frame: f, element: element[i] }));
    }, [holder, asked]);
    // Where the frame meets a line of its own colour (the accent's bar of a row chosen), it goes on
    // in it and is seen: the picture shows its colour there, and the element beside it another one.
    // Where the element is of the frame's colour itself (a frame in the accent on the accent), it is
    // lost in it.
    const near = (c) => c && seen.frameColour && Math.max(...c.map((v, k) => Math.abs(v - seen.frameColour[k]))) <= 24;
    told.forEach((t, i) => { if (t.frame.by === "unseen" && near(asked[i][4]) && !near(asked[i][6])) t.frame.by = "merged"; });
    // A bar or a button floating over the page covers the element as well: no fault of the frame.
    const floating = told.filter((t) => t.element.self && t.frame.floating);
    if (floating.length) notes.push(`${await page.evaluate((h) => window.__frames.describe(h.el), holder)}: its ${side} side under ${floating[0].frame.what}`);
    const isLost = (t) => t.element.self && t.frame.by !== "merged" && !t.frame.floating;
    const lost = told.filter(isLost);
    const judged = s.shown + lost.length;
    if (!judged) continue;
    if (/dashed|dotted/.test(seen.band.style)) {
      // Its gaps are its look: what fails is a stretch longer than a gap and a dash (12 px).
      let run = 0, longest = 0, last = -2;
      told.forEach((t, i) => { if (!isLost(t)) return; const at = asked[i][5]; run = at === last + 1 ? run + 1 : 1; last = at; longest = Math.max(longest, run); });
      if (longest * 2 <= 12) continue;
    } else if (s.shown / judged >= SHOWN) continue;
    const causes = new Map();
    for (const t of lost) {
      const text = t.frame.what ? `${t.frame.by} by ${t.frame.what}` : t.frame.by;
      causes.set(text, (causes.get(text) || 0) + 1);
    }
    const named = [...causes.entries()].sort((a, b) => b[1] - a[1]).slice(0, 3).map(([text]) => text);
    faults.push(`${side} ${Math.round((s.shown / judged) * 100)}% (${named.join("; ")})`);
  }
  return faults;
};

// ---- the views

const results = [];
/// What is seen on the way but is no frame's fault: a control focused under a bar or another layer.
const notes = [];
const report = (view, control, holder, seen, faults, outsideNote) => {
  const entry = { view, control, holder, kind: seen?.band?.kind, detail: seen?.band?.detail, inside: seen?.band?.inside, faults: faults || [], note: outsideNote || "" };
  results.push(entry);
  if (faults?.length) problems.push(`${view}: ${control}${holder !== control ? ` (its frame on ${holder})` : ""} — ${entry.kind}, ${entry.detail}: ${faults.join(", ")}`);
  else if (verbose) console.log(`  ok ${view}: ${control} (${entry.kind}${entry.inside ? ", inside" : ", OUTSIDE"})`);
};

/// Every frame of a view: those at rest, then each control's as it is focused.
const walk = async (page, view, scope) => {
  const t0 = Date.now();
  let frames = 0;
  // A key first, so that the focus that follows is the keyboard's (`:focus-visible`), as Tab's.
  await page.keyboard.press("Shift");
  const resting = await page.evaluate((s) => window.__frames.atRest(s), scope);
  for (const holder of resting) {
    const seen = await look(page, holder);
    if (!seen || seen.offscreen) continue;
    const faults = await judge(page, holder, seen);
    frames++;
    // An outline marks something (a box-shadow at rest may be a lift or a gap around a badge).
    if (seen.band.kind === "outline" && !seen.band.inside && !faults.length) faults.push("drawn outside its edge (the rule is inside)");
    const name = await page.evaluate((h) => window.__frames.describe(h.el) + h.pseudo, holder);
    report(view, `${name} (at rest)`, `${name} (at rest)`, seen, faults);
  }
  const controls = await page.evaluate((s) => window.__frames.controls(s), scope);
  let unseen = 0;
  for (const n of controls) {
    const focus = await page.evaluate((n) => window.__frames.focus(n), n);
    if (focus.gone || !focus.focused) continue;
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    const name = await page.evaluate((n) => window.__frames.describe(n), n);
    if (!focus.holders.length) {
      if (focus.visible) unseen++;
      if (verbose) console.log(`  · ${view}: ${name}: no frame comes with the focus`);
      continue;
    }
    // Transitions of a frame (a box-shadow that eases in) end before it is looked at.
    await page.evaluate(() => Promise.race([Promise.all(document.getAnimations().map((a) => a.finished.catch(() => {}))), new Promise((r) => setTimeout(r, 1200))]));
    for (const holder of focus.holders) {
      const seen = await look(page, holder);
      if (!seen || seen.offscreen) continue;
      frames++;
      const faults = await judge(page, holder, seen);
      const under = await page.evaluate((n) => window.__frames.under(n), n);
      if (under) notes.push(`${view}: ${name} is under ${under} as it is focused`);
      if (!seen.band.inside && !faults.length) faults.push("drawn outside its edge (the rule is inside)");
      const holderName = await page.evaluate((h) => window.__frames.describe(h.el) + h.pseudo, holder);
      report(view, name, holderName, seen, faults);
    }
  }
  await page.evaluate(() => window.__frames.blur());
  console.log(`${view}: ${frames} frames of ${controls.length} controls and ${resting.length} at rest in ${((Date.now() - t0) / 1000).toFixed(1)} s${unseen ? `, ${unseen} focused without a frame` : ""}`);
};

// ---- a browser with a study, marks and a plan, as the student has them

const MINE = "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\nstart\t2025W\n";
const W = ["12104", "12107", "12102", "11112"];
const S = ["12101", "11903", "11113", "12111"];
const NOW = ["12330", "11103", "12109", "11905"];
const PLAN = [...W.map((id) => `m\t2025W\t${id}\t1\t\tplan`), ...S.map((id) => `m\t2026S\t${id}\t1\t\tplan`), ...NOW.map((id, i) => `m\t2026W\t${id}\t${i + 1}\t\t`), ...W.map((id) => `d\t2025W\t${id}`), ...S.map((id) => `d\t2026S\t${id}`)].join("\n") + "\n";
const now = Math.floor(Date.now() / 1000);
const MARKS = ["12330", "11103", "12104", "11903", "12109"].map((id, i) => `${id}\t${now - i * 3600}\n`).join("");

/// The cookie of closed testing from a curl cookie jar, where the site has the gate (canary).
const gate = process.env.SMOKE_COOKIES
  ? (await import("node:fs")).readFileSync(process.env.SMOKE_COOKIES, "utf8").split("\n").map((l) => l.replace(/^#HttpOnly_/, "").split("\t")).filter((f) => f.length >= 7 && f[5] === "betula_access").map((f) => ({ name: f[5], value: f[6], domain: f[0].replace(/^\./, ""), path: "/", secure: true, httpOnly: true }))
  : [];

const open = async (phone) => {
  const context = await browser.newContext(phone
    ? { viewport: { width: 390, height: 844 }, deviceScaleFactor: 1, isMobile: true, hasTouch: true, serviceWorkers: "block", locale: "de-DE" }
    : { viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1, serviceWorkers: "block", locale: "de-DE" });
  if (gate.length) await context.addCookies(gate);
  await context.addInitScript({ path: new URL("./on-page.js", import.meta.url).pathname });
  await context.addInitScript(inPage);
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push(`console: ${m.text().slice(0, 300)}`); });
  page.on("pageerror", (e) => problems.push(`pageerror: ${String(e).slice(0, 300)}`));
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
  await page.evaluate(([mine, plan, marks]) => {
    localStorage.setItem("betula.myprogram.v1", mine);
    localStorage.setItem("betula.studyplan.v1", plan);
    localStorage.setItem("betula.bookmarks.v1", marks);
    localStorage.setItem("betula.collections.v1", "c\t1\tGrundlagen\n");
    // The pictures of „Über Betula" hold still (they go on by themselves), as e2e/about.mjs has them.
    localStorage.setItem("betula.showcase", "paused");
  }, [MINE, PLAN, MARKS]);
  return { context, page };
};

const go = async (page, path, ready, settings = {}) => {
  await page.evaluate((settings) => { for (const [k, v] of Object.entries(settings)) localStorage.setItem(k, v); }, settings);
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 });
  const ok = await page.waitForFunction(ready, null, { timeout: 30000 }).then(() => true, () => false);
  await page.waitForTimeout(600);
  return ok;
};

const views = [
  { name: "catalog", path: "/catalog?program=bachelor-informatik-2008", ready: () => allOnPage(".rows a.row").length > 5 },
  { name: "catalog-module", path: "/catalog?open=12330", ready: () => !!onPage(".detail .hero") },
  { name: "module-page", path: "/catalog/module/12330", ready: () => !!onPage(".hero") },
  { name: "programs", path: "/programs", ready: () => allOnPage("a[href*='/programs/']").length > 5 },
  { name: "program-plan", path: "/programs/bachelor-informatik-2008/plan", ready: () => !!onPage(".content, main") && allOnPage("a, button").length > 20 },
  { name: "program-areas", path: "/programs/bachelor-informatik-2008/areas", ready: () => allOnPage("a, button").length > 20 },
  { name: "bookmarks", path: "/bookmarks", ready: () => allOnPage(".rows a.row, .rows .row").length >= 3 },
  { name: "study", path: "/study", settings: { "betula.study.view": "semester" }, ready: () => allOnPage(".st-row").length >= 3 },
  { name: "grid", path: "/study", settings: { "betula.study.view": "grid" }, ready: () => !!onPage(".st-rm .st-col-head"),
    then: async (page) => { await page.click(".st-rm-all >> visible=true").catch(() => {}); await page.waitForTimeout(500); } },
  { name: "study-plan", path: "/study?view=plan", ready: () => allOnPage("button, a").length > 20 },
  { name: "study-areas", path: "/study?view=areas", ready: () => allOnPage("button, a").length > 20 },
  { name: "study-documents", path: "/study?view=documents", ready: () => allOnPage("a").length > 10 },
  { name: "studyplan", path: "/studyplan", ready: () => allOnPage(".slot").length > 0 },
  { name: "studyplan-edit", path: "/studyplan?edit=1", ready: () => allOnPage(".slot").length > 0 },
  { name: "home", path: "/", ready: () => allOnPage("a, button").length > 10 },
  { name: "profile", path: "/profile", ready: () => allOnPage("button").length > 5 },
  { name: "about", path: "/about/overview", ready: () => allOnPage("a").length > 10 },
  { name: "commands", path: "/catalog", ready: () => allOnPage(".rows a.row").length > 5, scope: ".cmd-dialog[open]",
    then: async (page) => { await page.keyboard.press("Control+k"); await page.waitForTimeout(400); } },
  { name: "row-menu", path: "/study", settings: { "betula.study.view": "semester" }, ready: () => allOnPage(".st-row").length >= 3, scope: ".st-pop",
    then: async (page) => { await page.click(".st-card-sem .st-row .st-status >> visible=true").catch(() => {}); await page.waitForTimeout(400); } },
  { name: "termin", path: "/studyplan", ready: () => allOnPage(".slot").length > 0, scope: ".tm-card",
    then: async (page) => { await page.click("#content .page-stage.front .slot >> visible=true").catch(() => {}); await page.waitForTimeout(500); } },
];
const phoneViews = [
  { name: "phone-catalog", path: "/catalog", ready: () => allOnPage(".rows a.row").length > 5 },
  { name: "phone-study", path: "/study", settings: { "betula.study.view": "semester" }, ready: () => allOnPage("button").length > 5 },
  { name: "phone-grid", path: "/study", settings: { "betula.study.view": "grid" }, ready: () => allOnPage("button").length > 5 },
  { name: "phone-studyplan", path: "/studyplan", ready: () => allOnPage(".slot, .sp-daylist a").length > 0 },
  { name: "phone-home", path: "/", ready: () => allOnPage("a, button").length > 10 },
];

for (const [phone, list] of [[false, views], [true, phoneViews]]) {
  const chosen = list.filter((v) => !only.length || only.some((o) => v.name.includes(o)));
  if (!chosen.length) continue;
  const { context, page } = await open(phone);
  for (const view of chosen) {
    const ok = await go(page, view.path, view.ready, view.settings);
    check(ok, `${view.name}: ${view.path} did not come up`);
    if (!ok) continue;
    if (view.then) await view.then(page);
    await walk(page, view.name, view.scope);
    await page.keyboard.press("Escape").catch(() => {});
  }
  await context.close();
}

await browser.close();

const cut = results.filter((r) => r.faults.length);
console.log(`\n${results.length} frames looked at, ${cut.length} not whole`);
if (notes.length) console.log(`${notes.length} controls under something as they are focused${verbose ? ":\n" + notes.join("\n") : " (FRAMES_VERBOSE=1 names them)"}`);
if (problems.length) {
  console.error(problems.join("\n"));
  console.error(`frames FAILED: ${problems.length} problem(s)`);
  process.exit(1);
}
console.log("frames ok");
