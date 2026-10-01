// Starts the browser app: opens the local copy of the catalog (sql.js, cached in IndexedDB by
// the snapshot's ETag), loads the WASM bundle and lets it take the page over. Until then, and
// whenever anything here fails, the server-rendered site keeps working as it is.
//
// The service worker (`/sw.js`) keeps the shell of the app — the page, the scripts, the styles,
// the bundle — so that the app starts without a network as well: the catalog itself is here in
// IndexedDB, and the worker never touches it.
const DB_NAME = "betula-catalog";
const STORE = "snapshots";
// The build of the page, as it linked this script (`?v=<build>`, `app::BuildId`). The bundle and
// sql.js are asked for with it too, so they come from the same build as the page and its
// stylesheet: a service worker of another build has nothing under these addresses.
const BUILD = new URL(import.meta.url).search;
// The schema of the catalog the queries of this build are written for: a snapshot's `PRAGMA
// user_version`, the number of Radix's last migration (`catalog::SCHEMA_VERSION`, which the server
// writes in here). A copy of an older schema lacks columns they select, so it is never opened.
// A name and not a string: a minifier folds `Number("…")` into NaN before the server can write
// the number in (server/build/main.rs).
const SCHEMA = __SCHEMA__;
// The semantic search's model (server/src/semantic.rs), or null when the server has none: then the
// app has no semantic search. `url`: its address (`/models/e5-de-en-<hash>.bin`); `passage`: Radix's
// id of the passage model it was made for, or null when the server does not know it. A name, as SCHEMA.
const SEMANTIC_MODEL = __SEMANTIC_MODEL__;

// What this script says, in the page's language as its address says it (`catalog::Locale::split`;
// docs/i18n.md). The first is the default, without a prefix. (Offline the service worker may
// answer with a page it kept in another language; the address is still the visitor's.)
const LANGUAGES = [
  {
    prefix: "",
    loading: "Daten werden geladen …",
    loadingShare: (percent) => `Daten werden geladen … ${percent} %`,
    offline: "Offline – die Daten werden neu geladen, sobald du online bist",
  },
  {
    prefix: "/en",
    loading: "Loading the data …",
    loadingShare: (percent) => `Loading the data … ${percent} %`,
    offline: "Offline – the data will be loaded again once you are online",
  },
];
const T = LANGUAGES.find((l) => l.prefix && (location.pathname === l.prefix || location.pathname.startsWith(l.prefix + "/"))) || LANGUAGES[0];

if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("/sw.js").catch((error) => console.info("[catalog] no service worker:", error));
}

let statusState = null;
function status(text, state) {
  statusState = text ? { text, state: state || "" } : null;
  applyStatus();
}
function applyStatus() {
  const el = document.getElementById("db-status");
  if (!el) return;
  const text = statusState ? statusState.text : "";
  if (el.textContent !== text) el.textContent = text;
  el.hidden = !text;
  el.dataset.state = statusState ? statusState.state : "";
}
// The app re-renders the top bar; keep the status visible across that.
new MutationObserver(applyStatus).observe(document.documentElement, { childList: true, subtree: true });

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

async function download(total) {
  const response = await fetch("/api/db");
  if (!response.ok) throw new Error("GET /api/db: HTTP " + response.status);
  const reader = response.body.getReader();
  const chunks = [];
  let received = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    received += value.length;
    if (total) status(T.loadingShare(Math.min(99, Math.round((received / total) * 100))));
  }
  const bytes = new Uint8Array(received);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  return bytes;
}

// The schema of a copy of the catalog, from the header of the SQLite file, without opening it
// (`user_version`: four bytes at offset 60, big-endian). 0 for what is not a SQLite file.
function schemaOf(bytes) {
  if (bytes.length < 100 || new TextDecoder().decode(bytes.subarray(0, 15)) !== "SQLite format 3") return 0;
  return new DataView(bytes.buffer, bytes.byteOffset, 100).getInt32(60);
}

function loadScript(src) {
  return new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = src;
    script.onload = resolve;
    script.onerror = () => reject(new Error("cannot load " + src));
    document.head.appendChild(script);
  });
}

async function openDatabase() {
  // What the server has now; offline this fails and the cached copy is used as it is.
  let server = null;
  try {
    const response = await fetch("/api/status", { cache: "no-store" });
    if (response.ok) server = (await response.json()).snapshot;
  } catch {}
  // A copy of an older schema than this build reads is never opened: the queries of what came
  // since would fail on it („no such column"). An older one on the server is not even fetched
  // (Radix has not exported the new schema yet); without another, the site stays a classic website.
  const serverFits = server && server.schema_version >= SCHEMA;

  let current = await idbGet("current"); // { etag, bytes }
  if (current && schemaOf(current.bytes) < SCHEMA) {
    // Replaced before the app starts, as on a first visit. Offline there is nothing to replace
    // it with: the app does not start, and the page stays the one the service worker kept.
    if (!server) {
      const error = new Error("the local copy of the catalog is older than this build, and the server is not reachable");
      error.notice = T.offline;
      throw error;
    }
    current = null;
  }
  if (!current) {
    if (!server) throw new Error("no local copy of the catalog and the server is not reachable");
    if (!serverFits) throw new Error(`the server's catalog is of schema ${server.schema_version}, this build reads ${SCHEMA}`);
    status(T.loading);
    current = { etag: server.etag, bytes: await download(server.bytes) };
    await idbPut("current", current);
    if (navigator.storage && navigator.storage.persist) navigator.storage.persist().catch(() => {});
  } else if (serverFits && server.etag !== current.etag) {
    // Work with the copy we have; fetch the new one in the background for the next start.
    download(0)
      .then((bytes) => idbPut("current", { etag: server.etag, bytes }))
      .catch((error) => console.warn("[catalog] update failed", error));
  }

  await loadScript("/assets/sql-wasm.js" + BUILD);
  const SQL = await window.initSqlJs({ locateFile: (file) => "/assets/" + file + BUILD });
  const db = new SQL.Database(current.bytes);
  window.betulaDb = {
    etag: current.etag,
    query(sql, params) {
      const statement = db.prepare(sql);
      try {
        statement.bind(params);
        const columns = statement.getColumnNames();
        const rows = [];
        while (statement.step()) rows.push(statement.get());
        return { columns, rows };
      } finally {
        statement.free();
      }
    },
  };
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

// The semantic search (semantic/README.md): the model (15 MB) runs in a Web Worker of its own,
// with the index built from the modules' vectors in the local catalog (`v_module_vector`, Radix's).
// It is loaded only once the app runs and the browser is idle, so it never holds up the page, the
// catalog or the app; the model comes with a low priority and is kept by the service worker apart
// from the shell of a build, so it is downloaded once per model, not once per deploy.
//
// `window.betulaSemantic`, for the app:
//   ready             a promise: {rows, build, ms} once the search can answer; null when this
//                     browser has none (no model on the server, no vectors in the catalog yet,
//                     vectors of another passage model than the query model was made for, data
//                     saving, a device with little memory, or loading failed)
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
  const ready = new Promise((resolve) => {
    const start = () => resolve(load());
    if ("requestIdleCallback" in window) requestIdleCallback(start, { timeout: 5000 });
    else setTimeout(start, 1500);
  }).catch((error) => {
    console.info("[semantic] not loaded:", error);
    if (semantic) semantic.terminate();
    semantic = null;
    return null;
  });
  async function load() {
    // A query is only comparable with passages of the model it was made for. The local copy of the
    // catalog may be older than the server's model (it is replaced at the next start), or Radix may
    // still be computing the vectors of a new passage model: then no search, rather than a wrong one.
    const made = window.betulaDb.query("SELECT value FROM meta WHERE key = 'semantic_model'", []).rows;
    const vectorsOf = made.length ? made[0][0] : null;
    if (SEMANTIC_MODEL.passage && vectorsOf !== SEMANTIC_MODEL.passage) {
      console.info(`[semantic] the catalog's vectors are of the passage model ${vectorsOf}, the query model is for ${SEMANTIC_MODEL.passage}`);
      return null;
    }
    const { Semantic, indexFromVectors } = await import("/pkg/semantic.js" + BUILD);
    const rows = window.betulaDb.query("SELECT module_id, scale, vector FROM v_module_vector ORDER BY module_id", []).rows;
    // Radix has not computed the vectors of this snapshot yet: nothing to find, nothing to load.
    if (!rows.length) return null;
    semantic = new Semantic({ model: SEMANTIC_MODEL.url, index: indexFromVectors(rows) });
    return semantic.ready;
  }
  window.betulaSemantic = {
    ready,
    async search(query, k) {
      if (!(await ready) || !semantic) return null;
      return semantic.search(query, k);
    },
  };
  return ready;
}

try {
  const [app, , programMap] = await Promise.all([
    import("/pkg/folia_client.js" + BUILD).then(async (module) => { await module.default("/pkg/folia_client_bg.wasm" + BUILD); return module; }),
    openDatabase(),
    loadProgramMap(),
  ]);
  window.betulaMap = programMap || null;
  window.__betulaApp = true;
  document.documentElement.classList.add("app");
  app.start();
  // Once the app runs there is nothing to say: it simply works.
  status("");
  startSemantic();
} catch (error) {
  // Not fatal: the site stays a classic website. The pill says nothing, unless the visitor needs
  // to know why (`notice`).
  console.info("[catalog] browser app not started:", error);
  status(error.notice || "");
}
