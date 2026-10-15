// Starts the browser app: starts the data worker (folia/crates/worker/js/data-worker.js), which finds,
// fetches and opens the catalog (kept in IndexedDB by the snapshot's ETag), loads the WASM bundle
// and lets it take the page over once the worker has answered for it. Until then, and whenever
// anything here fails, the server-rendered site keeps working as it is.
//
// The service worker (`/sw.js`) keeps the shell of the app — the page, the scripts, the styles,
// the bundle — so that the app starts without a network as well: the catalog itself is in
// IndexedDB (the data worker's), and the service worker never touches it.
const DB_NAME = "betula-catalog";
const STORE = "snapshots";
// The build of the page, as it linked this script (`?v=<build>`, `app::BuildId`). The bundle and
// sql.js are asked for with it too, so they come from the same build as the page and its
// stylesheet: a service worker of another build has nothing under these addresses.
const BUILD = new URL(import.meta.url).search;
// The schema of the catalog the queries of this build are written for: a snapshot's `PRAGMA
// user_version`, the number of Radix's last migration (`folia_model::SCHEMA_VERSION`, which the server
// writes in here). A copy of an older schema lacks columns they select, so it is never opened.
// A name and not a string: a minifier folds `Number("…")` into NaN before the server can write
// the number in (folia/crates/server/build/main.rs).
const SCHEMA = __SCHEMA__;
// The semantic search's model (folia/crates/server/src/semantic.rs), or null when the server has none: then the
// app has no semantic search. `url`: its address (`/models/e5-de-en-<hash>.bin`); `passage`: Radix's
// id of the passage model it was made for, or null when the server does not know it. A name, as SCHEMA.
const SEMANTIC_MODEL = __SEMANTIC_MODEL__;

// What this script says, in the page's language as its address says it (`folia_locale::Locale::split`;
// docs/folia/i18n.md). The first is the default, without a prefix. (Offline the service worker may
// answer with a page it kept in another language; the address is still the visitor's.)
const LANGUAGES = [
  {
    prefix: "",
    offline: "Offline – die Daten werden neu geladen, sobald du online bist",
  },
  {
    prefix: "/en",
    offline: "Offline – the data will be loaded again once you are online",
  },
];
const T = LANGUAGES.find((l) => l.prefix && (location.pathname === l.prefix || location.pathname.startsWith(l.prefix + "/"))) || LANGUAGES[0];

if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("/sw.js").catch((error) => console.info("[catalog] no service worker:", error));
}

// The status in the top bar says one thing only: why the app does not start, where the visitor
// needs to know (offline, with a copy of the catalog of an older schema). The loading of the data
// is not announced (owner, 2026-10-15: „Das Laden geht schnell genug, so dass man es nicht sehen
// muss"; on a phone its words took the search's room, „D…", and gave it back at the takeover).
function status(text) {
  const el = document.getElementById("db-status");
  if (!el) return;
  el.textContent = text;
  el.hidden = !text;
}

// IndexedDB, where the map of the programs is kept beside the catalog (the data worker keeps the
// catalog in the same store).
function idb() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}
async function idbGet(key) {
  const db = await idb();
  return new Promise((resolve, reject) => {
    const request = db.transaction(STORE).objectStore(STORE).get(key);
    request.onsuccess = () => resolve(request.result || null);
    request.onerror = () => reject(request.error);
  });
}
async function idbPut(key, value) {
  const db = await idb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, "readwrite");
    tx.objectStore(STORE).put(value, key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

// The app's bundle, compiled as it loads. (The data worker loads a bundle of its own, `folia-worker`.)
let bundle = null;
async function compileBundle() {
  const url = "/pkg/folia_client_bg.wasm" + BUILD;
  try {
    return await WebAssembly.compileStreaming(fetch(url));
  } catch {
    // A server that does not say `application/wasm`, or a browser that cannot compile as it loads.
    const response = await fetch(url);
    if (!response.ok) throw new Error("GET " + url + ": HTTP " + response.status);
    return WebAssembly.compile(await response.arrayBuffer());
  }
}

// The map of the programs on the landing page. The server lays it out once per snapshot; the app
// only draws it. Kept next to the catalog, so the landing page has it offline too. Without it
// the app simply shows no map.
async function loadProgramMap() {
  try {
    const response = await fetch("/api/map.json");
    if (response.ok) {
      const text = await response.text();
      idbPut("map", text).catch(() => {});
      return text;
    }
  } catch {}
  return idbGet("map").catch(() => null);
}

// The semantic search (folia/crates/semantic/README.md): the model (15 MB) runs in a Web Worker of its own,
// with the index built from the modules' vectors in the catalog (`v_module_vector`, Radix's), which
// the data worker reads.
// It is loaded when it is first wanted: as a search field takes the focus (in the browser's idle
// time, so that it is there by the time the words are), or when a search asks it. Loaded at every
// start once the browser was idle (until 2026-10-02), it took a phone's main thread at a moment
// nobody could see coming, and the model's 15 MB on every start, from those who never search too.
// With a mouse it is still loaded once the browser is idle. The model comes with a low priority
// and is kept by the service worker apart from the shell of a build, so it is downloaded once per
// model, not once per deploy.
//
// `window.betulaSemantic`, for the app:
//   ready             a promise: {rows, build, ms} once the search can answer; null when this
//                     browser has none (no model on the server, no vectors in the catalog yet,
//                     vectors of another passage model than the query model was made for, data
//                     saving, a device with little memory, or loading failed). Asking for it
//                     starts the loading.
//   search(query, k)  a promise: {hits: [{id, score}], ms}, best first; null when a newer query
//                     took its place, or when there is no semantic search
function startSemantic() {
  const none = () => {
    window.betulaSemantic = { ready: Promise.resolve(null), search: async () => null };
    return null;
  };
  const connection = navigator.connection;
  if (!SEMANTIC_MODEL || !("Worker" in window) || (connection && connection.saveData) || (navigator.deviceMemory && navigator.deviceMemory < 2)) return none();
  let semantic = null;
  let ready = null;
  const begin = () => (ready ??= load().catch((error) => {
    console.info("[semantic] not loaded:", error);
    if (semantic) semantic.terminate();
    semantic = null;
    return null;
  }));
  const soon = () => {
    if (ready) return;
    if ("requestIdleCallback" in window) requestIdleCallback(begin, { timeout: 3000 });
    else setTimeout(begin, 500);
  };
  document.addEventListener("focusin", (e) => { if (e.target.matches?.('input[type="search"], form[data-live-search] input')) soon(); });
  if (matchMedia("(hover: hover) and (pointer: fine)").matches) soon();
  async function load() {
    // A query is only comparable with passages of the model it was made for. The local copy of the
    // catalog may be older than the server's model (it is replaced at the next start), or Radix may
    // still be computing the vectors of a new passage model: then no search, rather than a wrong one.
    const made = (await window.betulaData.query("SELECT value FROM meta WHERE key = 'semantic_model'")).rows;
    const vectorsOf = made.length ? made[0][0] : null;
    if (SEMANTIC_MODEL.passage && vectorsOf !== SEMANTIC_MODEL.passage) {
      console.info(`[semantic] the catalog's vectors are of the passage model ${vectorsOf}, the query model is for ${SEMANTIC_MODEL.passage}`);
      return null;
    }
    const { Semantic, indexFromVectors } = await import("/pkg/semantic.js" + BUILD);
    const rows = (await window.betulaData.query("SELECT module_id, scale, vector FROM v_module_vector ORDER BY module_id")).rows;
    // Radix has not computed the vectors of this snapshot yet: nothing to find, nothing to load.
    if (!rows.length) return null;
    semantic = new Semantic({ model: SEMANTIC_MODEL.url, index: indexFromVectors(rows) });
    return semantic.ready;
  }
  window.betulaSemantic = {
    get ready() {
      return begin();
    },
    async search(query, k) {
      if (!(await begin()) || !semantic) return null;
      return semantic.search(query, k);
    },
  };
}

// The data worker (folia/crates/worker/js/data-worker.js, docs/folia/folia-refactor.md §6.2): the
// catalog and the app's bundle beside the page's thread, which answer every question of the app's
// pages; the page builds what they say. A Web Worker, not the service worker: a browser stops an
// idle service worker after some seconds, and each start would open the catalog again.
//
// `window.betulaData`, for the app (folia/crates/client/src/worker.rs) and for this script:
//   ask(name, lane, question)  a promise: the JSON of the answer to the question `name` with the
//                              fields `question` (JSON); null when the worker failed
//   query(sql)                 a promise: {columns, rows} of a statement of this script's own
//   etag                       the ETag of the snapshot the worker answers from
function startData() {
  const worker = new Worker("/pkg/data-worker.js" + BUILD);
  const asked = new Map();
  let next = 0;
  let failed = null;
  const send = (message) => {
    if (failed) return Promise.reject(failed);
    const id = next++;
    worker.postMessage({ ...message, id });
    return new Promise((resolve, reject) => asked.set(id, { resolve, reject }));
  };
  const fail = (error) => {
    if (failed) return;
    failed = error;
    console.error("[data] the worker failed:", error);
    worker.terminate();
    for (const { reject } of asked.values()) reject(error);
    asked.clear();
  };
  worker.onmessage = ({ data }) => {
    // What the worker says by itself: a newer snapshot in use.
    if (data.event === "snapshot") {
      window.betulaData.etag = data.etag;
      return window.betulaSnapshot?.();
    }
    const question = asked.get(data.id);
    asked.delete(data.id);
    if (data.error) {
      const error = new Error(data.error);
      if (data.notice === "offline") error.notice = T.offline;
      question?.reject(error);
    } else question?.resolve(data);
  };
  worker.onerror = (event) => fail(new Error(event.message || "the data worker failed"));
  // Back in view: the worker looks for a newer snapshot (and every few minutes by itself).
  document.addEventListener("visibilitychange", () => { if (document.visibilityState === "visible" && !failed) worker.postMessage({ type: "look" }); });
  window.betulaData = {
    // Every question is answered, in turn. (A newer question of a kind taking the place of a
    // waiting one, „newest wins", would drop the question of another part of the page that asks
    // the same kind at the same time; the steps of what is typed are dropped before they ask,
    // `Pending::typed`.)
    ask(name, lane, question) {
      return send({ type: "ask", name, question }).then(({ answer }) => answer ?? null, () => null);
    },
    query(sql) {
      return send({ type: "query", sql, params: [] });
    },
  };
  // The worker loads its own bundle meanwhile: the download does not wait for the page's. Without
  // a catalog (offline on a first visit, an older schema) the site stays a website.
  return send({ type: "start", schema: SCHEMA }).then(({ etag }) => {
    window.betulaData.etag = etag;
  });
}

// The server's page as a picture in front of the app while the app's first answers are on their
// way (`betulaAnswered`, called by the app): the app replaces the page at once, and its first frame
// would otherwise show its regions empty for the moment the data worker takes. The picture is a
// copy of the body itself, a second <body> after it, so that every rule of app.css that lays out
// the body and its children lays out the copy as it laid out the page, on every screen. (Copied
// into a <div> that took four of the body's properties, the children stood in a row on a phone,
// where the body is a column: the ground a narrow strip at the right for a moment; owner,
// 2026-10-15: „Das Layout darf sich auf gar keinen Fall verschieben nach dem Laden".) It lies where
// the body lies (the window keeps its scroll, and what is sticky stays where it stuck), with every
// scroll position inside and every animation where it was; without ids (nothing finds it instead
// of the app's own), out of reach of clicks, keys and screen readers, and gone once the app has
// its answers. `lazy`: the page's boxes that are laid out only near the screen (`lazyBoxes`), with
// what they stand as.
function takePicture(lazy) {
  const body = document.body;
  const picture = body.cloneNode(true);
  picture.classList.add("takeover-picture");
  picture.setAttribute("aria-hidden", "true");
  picture.inert = true;
  for (const el of [picture, ...picture.querySelectorAll("[id]")]) el.removeAttribute("id");
  const originals = [...body.querySelectorAll("*")];
  const scrolled = originals.map((el) => [el.scrollTop, el.scrollLeft]);
  // A copy starts its CSS animations anew (a module that slid in would slide in again, the
  // carousel's bar would start over): each one takes the start of the one it copies.
  const moving = document.getAnimations().filter((a) => a.animationName && a.startTime !== null && a.timeline === document.timeline && body.contains(a.effect?.target));
  const key = (animation, index) => `${index} ${animation.effect.pseudoElement || ""} ${animation.animationName}`;
  const order = moving.length || lazy.length ? new Map(originals.map((el, i) => [el, i]).concat([[body, -1]])) : null;
  const started = new Map(moving.map((a) => [key(a, order.get(a.effect.target)), a.startTime]));
  const copies = picture.querySelectorAll("*");
  for (const [el, stands] of lazy) copies[order.get(el)]?.style.setProperty("contain-intrinsic-block-size", stands.size);
  document.documentElement.appendChild(picture);
  scrolled.forEach(([top, left], i) => {
    if ((top || left) && copies[i]) {
      copies[i].scrollTop = top;
      copies[i].scrollLeft = left;
    }
  });
  if (moving.length) {
    const copied = new Map([...copies].map((el, i) => [el, i]).concat([[picture, -1]]));
    for (const animation of picture.getAnimations({ subtree: true })) {
      const start = started.get(key(animation, copied.get(animation.effect?.target)));
      if (start !== undefined) animation.startTime = start;
    }
  }
  // The app may scroll the window past the page (on a phone the catalog's `?open=` goes to the
  // module's row in the whole list): a picture that no longer covers the window would only lay
  // what stands fixed in it (the bar's ground, the tab bar) over the app's own, so it goes at once.
  const uncovered = () => {
    if (picture.getBoundingClientRect().bottom >= innerHeight - 1) return;
    picture.remove();
    removeEventListener("scroll", uncovered);
  };
  addEventListener("scroll", uncovered, { passive: true });
  return picture;
}

// The boxes app.css lays out only near the screen (`content-visibility: auto`: the start page's
// panels, the rows of a list, the weeks of „Termine"), found by the rules that say so. Off the
// screen such a box stands as large as it was when it was last drawn, or as the rule guesses where
// it never was; a box made anew (the picture's copy, the app's own) has drawn nothing yet and
// takes the guess, and everything below it moves (the start page scrolled 3000 px down before the
// takeover: a heading in the window moved by 671 px). So the new box is told what its original
// stands as (`contain-intrinsic-block-size`, which counts only while the box is not drawn).
function lazyBoxes(root) {
  const selectors = [];
  const look = (rules) => {
    for (const rule of rules) {
      if (rule.selectorText && rule.style?.contentVisibility === "auto") selectors.push(rule.selectorText);
      if (rule.cssRules) look(rule.cssRules);
    }
  };
  for (const sheet of document.styleSheets) {
    try {
      look(sheet.cssRules);
    } catch {}
  }
  if (!selectors.length) return [];
  return [...root.querySelectorAll(selectors.join(", "))].filter((el) => getComputedStyle(el).contentVisibility === "auto");
}
// What such a box stands as: its height inside padding and border, and what it is (its element and
// classes), by which the app's box of the same place is known.
function lazyStands(el) {
  const style = getComputedStyle(el);
  const inner = el.getBoundingClientRect().height - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom) - parseFloat(style.borderTopWidth) - parseFloat(style.borderBottomWidth);
  return { tag: el.tagName, classes: [...el.classList], size: `auto ${Math.max(0, inner)}px` };
}

// The parts of a page that are scrolled down (on a wide screen the page itself in its scroll area,
// a panel's list), each by where it stands: the element and first class of every part from the
// body down to it, and which of the parts standing there it is. The app builds its page anew, at
// the top of each part: it takes the scroll of the server's part of the same place (on a wide
// screen a page scrolled before the takeover went back to its top).
const placeOf = (el, root) => {
  let place = "";
  for (let e = el; e && e !== root; e = e.parentElement) place = `>${e.tagName}.${e.classList[0] || ""}` + place;
  return place;
};
const kindOf = (el) => el.tagName.toLowerCase() + (el.classList[0] ? "." + CSS.escape(el.classList[0]) : "");
const partsAt = (root, kind, place) => [...root.querySelectorAll(kind)].filter((el) => placeOf(el, root) === place);
function scrolledParts(root) {
  return [...root.querySelectorAll("*")].filter((el) => el.scrollTop > 0).map((el) => {
    const kind = kindOf(el), place = placeOf(el, root);
    return { kind, place, nth: partsAt(root, kind, place).indexOf(el), top: el.scrollTop };
  });
}
function carryScroll(parts, root) {
  for (const { kind, place, nth, top } of parts) {
    const el = partsAt(root, kind, place)[nth];
    if (el && el.scrollTop !== top) el.scrollTop = top;
  }
}

// The app is on its way: a link followed meanwhile waits for it (enhance.js, `betulaStarted`).
window.__betulaStarting = true;
try {
  const [app, , programMap] = await Promise.all([
    import("/pkg/folia_client.js" + BUILD).then(async (module) => {
      bundle = await compileBundle();
      await module.default({ module_or_path: bundle });
      return module;
    }),
    // The data worker finds, fetches and opens the catalog before the app takes the page over:
    // the app's first questions go there.
    startData(),
    loadProgramMap(),
  ]);
  window.betulaSnapshot = () => app.snapshot_changed();
  window.betulaMap = programMap || null;
  const lazy = lazyBoxes(document.body).map((el) => [el, lazyStands(el)]);
  const scrolled = scrolledParts(document.body);
  // While the picture stands, the window's scroll is the picture's: the browser must not move it
  // to keep in place what the app builds under it (scroll anchoring), nor while the app's page
  // takes the sizes and the scroll of the server's (below).
  const root = document.documentElement;
  root.style.setProperty("overflow-anchor", "none");
  const picture = takePicture(lazy);
  // The app has taken the page over once its first answers are in and the picture goes: from then
  // on it is the app (`__betulaApp`, which enhance.js and the checks go by).
  window.betulaAnswered = () => {
    window.betulaAnswered = null;
    // The app's page stands as the server's did: its boxes that are laid out only near the screen
    // as large, box for box as long as they are the same, and its parts scrolled as far. The
    // picture stays two frames more, while the app follows the scroll (a virtual list renders the
    // rows there).
    const own = lazyBoxes(document.body);
    for (let i = 0; i < own.length && i < lazy.length; i++) {
      const was = lazy[i][1];
      if (own[i].tagName !== was.tag || !was.classes.every((name) => own[i].classList.contains(name))) break;
      own[i].style.setProperty("contain-intrinsic-block-size", was.size);
    }
    carryScroll(scrolled, document.body);
    requestAnimationFrame(() => requestAnimationFrame(() => {
      picture.remove();
      requestAnimationFrame(() => requestAnimationFrame(() => root.style.removeProperty("overflow-anchor")));
      window.__betulaApp = true;
      window.__betulaStarting = false;
      // Where a link followed meanwhile leads, now within the app (once what it set off has run).
      setTimeout(() => window.betulaStarted?.(true), 0);
    }));
  };
  document.documentElement.classList.add("app");
  app.start();
  startSemantic();
} catch (error) {
  window.__betulaStarting = false;
  document.querySelector(".takeover-picture")?.remove();
  document.documentElement.style.removeProperty("overflow-anchor");
  // Not fatal: the site stays a classic website. The pill says nothing, unless the visitor needs
  // to know why (`notice`). A link followed meanwhile loads its page.
  console.info("[catalog] browser app not started:", error);
  status(error.notice || "");
  window.betulaStarted?.(false);
}
