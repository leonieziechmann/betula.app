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
    if (total) status(`Daten werden geladen … ${Math.min(99, Math.round((received / total) * 100))} %`);
  }
  const bytes = new Uint8Array(received);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  return bytes;
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

  let current = await idbGet("current"); // { etag, bytes }
  if (!current) {
    if (!server) throw new Error("no local copy of the catalog and the server is not reachable");
    status("Daten werden geladen …");
    current = { etag: server.etag, bytes: await download(server.bytes) };
    await idbPut("current", current);
    if (navigator.storage && navigator.storage.persist) navigator.storage.persist().catch(() => {});
  } else if (server && server.etag !== current.etag) {
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
} catch (error) {
  // Not fatal: the site stays a classic website.
  console.info("[catalog] browser app not started:", error);
  status("");
}
