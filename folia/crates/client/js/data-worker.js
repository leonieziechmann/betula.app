// The data worker (docs/folia/folia-refactor.md §6.2): the catalog in sql.js and the app's bundle,
// whose `worker_answer` (folia/crates/client/src/worker.rs) answers every question of the app's
// pages (`folia_pages::ask`) beside the page's thread. The page's thread has no copy of the
// catalog: it asks here and builds what the answers say. scripts/build-client.sh puts this file
// into site/pkg; `boot.js` starts it.
//
// The snapshot is the worker's too: it finds the copy kept in IndexedDB (by the snapshot's ETag),
// asks the server what it has (`/api/status`), downloads a new one (one tab at a time: a Web Lock),
// keeps it and opens it. While the tab is open it looks again when the page says the tab came back
// into view, and every few minutes; a newer snapshot is downloaded in the background, opened beside
// the one in use, and then answers every question from then on (owner, 2026-10-02: „sofort rein
// mit den neuen daten"): the page hears of it (`snapshot`) and asks its questions again, and so do
// the other tabs, told by a BroadcastChannel, which open the copy this one kept. A snapshot of
// another schema than this build reads waits for the next page load, as before; one of an older
// schema is never opened.
//
// Messages from the page (each answer carries the `id` of its request):
//   {id, type: "start", schema, bundle}   find, fetch and open the catalog (`schema`: the build's
//                                         SCHEMA_VERSION; `bundle`: the compiled module, or none)
//                                         → {etag} | {error, notice?}
//   {id, type: "ask", name, question}     a question by its name and its fields in JSON → {answer}:
//                                         the JSON of its Result<Answer, DataError>
//   {id, type: "query", sql, params}      a statement of boot.js's own (the semantic search's
//                                         vectors) → {columns, rows}
//   {type: "look"}                        the tab is in view again: is there a newer snapshot?
// Messages to the page, without an id:
//   {event: "progress", share}            the first download, `share` per cent of it
//   {event: "snapshot", etag}             a newer snapshot answers from now on
//
// A classic worker: sql.js is a classic script (`importScripts`), the bundle a module, which comes
// by `import()`. Both of the build of this worker's address (`?v=<build>`), as the page has them.
const BUILD = self.location.search;
const DB_NAME = "betula-catalog";
const STORE = "snapshots";
// The catalog as it came, as a Blob, kept under `catalog`: IndexedDB keeps a Blob as a file of its
// own and hands it back without reading it. A copy under the old name (`current`, an ArrayBuffer
// of a build of before 2026-10-02) is taken over once and then deleted.
const KEPT = "catalog";
const KEPT_BEFORE = "current";
// How often an open tab asks the server for a newer snapshot (Radix exports at most a few a day).
const LOOK_EVERY_MS = 5 * 60 * 1000;

let app = null;
let SQL = null;
let schema = 0;
let current = null; // { etag, db }
const channel = "BroadcastChannel" in self ? new BroadcastChannel("betula-catalog") : null;

// ---- IndexedDB, as boot.js kept it
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
async function idbDelete(key) {
  const db = await idb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, "readwrite");
    tx.objectStore(STORE).delete(key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

// What the server has now; null offline.
async function serverSnapshot() {
  try {
    const response = await fetch("/api/status", { cache: "no-store" });
    if (response.ok) return (await response.json()).snapshot;
  } catch {}
  return null;
}

async function download(total, progress) {
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
    if (total && progress) progress(Math.min(99, Math.round((received / total) * 100)));
  }
  return new Blob(chunks);
}

// The schema of a copy, from the header of the SQLite file, without opening it (`user_version`:
// four bytes at offset 60, big-endian). 0 for what is not a SQLite file, and for a copy the
// browser can no longer read (its file gone): either is replaced as an older one is.
async function schemaOf(blob) {
  let bytes;
  try {
    bytes = new Uint8Array(await blob.slice(0, 100).arrayBuffer());
  } catch {
    return 0;
  }
  if (bytes.length < 100 || new TextDecoder().decode(bytes.subarray(0, 15)) !== "SQLite format 3") return 0;
  return new DataView(bytes.buffer, 0, 100).getInt32(60);
}

// The kept copy, the one of the old name taken over.
async function keptCopy() {
  let kept = await idbGet(KEPT);
  if (!kept) {
    const before = await idbGet(KEPT_BEFORE).catch(() => null);
    if (before?.bytes) {
      kept = { etag: before.etag, blob: new Blob([before.bytes]) };
      idbPut(KEPT, kept).then(() => idbDelete(KEPT_BEFORE)).catch(() => {});
    }
  } else {
    idbDelete(KEPT_BEFORE).catch(() => {});
  }
  return kept;
}

// One download for all tabs: the lock is held while one runs, and the others find its copy kept.
function oneAtATime(job) {
  return self.navigator.locks ? self.navigator.locks.request("betula-catalog-download", job) : job();
}

function failure(message, notice) {
  const error = new Error(message);
  error.notice = notice;
  return error;
}

async function openCopy(kept) {
  const db = new SQL.Database(new Uint8Array(await kept.blob.arrayBuffer()));
  return { etag: kept.etag, db };
}

// The snapshot in use from now on; the answers kept for the one before go with it.
function use(next) {
  const before = current;
  current = next;
  app?.worker_forget();
  before?.db.close();
}

async function start(build, compiled) {
  schema = build;
  importScripts("/assets/sql-wasm.js" + BUILD);
  const [sql, bundle] = await Promise.all([
    self.initSqlJs({ locateFile: (file) => "/assets/" + file + BUILD }),
    import("/pkg/folia_client.js" + BUILD).then(async (bundle) => {
      await bundle.default({ module_or_path: compiled ?? "/pkg/folia_client_bg.wasm" + BUILD });
      return bundle;
    }),
  ]);
  SQL = sql;
  const server = await serverSnapshot();
  // A copy of an older schema than this build reads is never opened: the queries of what came
  // since would fail on it („no such column"). An older one on the server is not even fetched
  // (Radix has not exported the new schema yet); without another, the site stays a website.
  const serverFits = server && server.schema_version >= schema;
  let kept = await keptCopy();
  if (kept && (await schemaOf(kept.blob)) < schema) {
    if (!server) throw failure("the local copy of the catalog is older than this build, and the server is not reachable", "offline");
    kept = null;
  }
  if (!kept) {
    if (!server) throw failure("no local copy of the catalog and the server is not reachable");
    if (!serverFits) throw failure(`the server's catalog is of schema ${server.schema_version}, this build reads ${schema}`);
    kept = await oneAtATime(async () => {
      // Another tab may have kept it meanwhile.
      const theirs = await idbGet(KEPT).catch(() => null);
      if (theirs?.etag === server.etag) return theirs;
      self.postMessage({ event: "progress", share: 0 });
      const fresh = { etag: server.etag, blob: await download(server.bytes, (share) => self.postMessage({ event: "progress", share })) };
      await idbPut(KEPT, fresh);
      return fresh;
    });
  }
  use(await openCopy(kept));
  self.betulaDb = {
    query(sql, params) {
      const statement = current.db.prepare(sql);
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
  app = bundle;
  // A kept copy older than the server's is used now, and the newer one comes in the background.
  if (serverFits && server.etag !== current.etag) setTimeout(look, 0);
  setInterval(look, LOOK_EVERY_MS);
  return { etag: current.etag };
}

// Is there a newer snapshot? Downloaded (once for all tabs), kept, opened beside the one in use,
// then used, and the page and the other tabs told.
let looking = false;
async function look() {
  if (looking || !current) return;
  looking = true;
  try {
    const server = await serverSnapshot();
    if (!server || server.etag === current.etag || server.schema_version !== schema) return;
    const kept = await oneAtATime(async () => {
      const theirs = await idbGet(KEPT).catch(() => null);
      if (theirs?.etag === server.etag) return theirs;
      const fresh = { etag: server.etag, blob: await download(0) };
      if ((await schemaOf(fresh.blob)) !== schema) return null;
      await idbPut(KEPT, fresh);
      return fresh;
    });
    if (!kept || kept.etag === current.etag) return;
    use(await openCopy(kept));
    self.postMessage({ event: "snapshot", etag: current.etag });
    channel?.postMessage({ etag: current.etag });
  } catch (error) {
    console.warn("[catalog] update failed", error);
  } finally {
    looking = false;
  }
}

// Another tab kept a newer snapshot: open its copy.
if (channel) {
  channel.onmessage = async ({ data }) => {
    if (!current || !data?.etag || data.etag === current.etag) return;
    try {
      const kept = await idbGet(KEPT);
      if (kept?.etag !== data.etag || (await schemaOf(kept.blob)) !== schema) return;
      use(await openCopy(kept));
      self.postMessage({ event: "snapshot", etag: current.etag });
    } catch (error) {
      console.warn("[catalog] the snapshot of another tab cannot be opened", error);
    }
  };
}

const handlers = {
  start({ schema, bundle }) {
    return start(schema, bundle);
  },
  ask({ name, question }) {
    return { answer: app.worker_answer(name, question) };
  },
  query({ sql, params }) {
    return self.betulaDb.query(sql, params ?? []);
  },
};

// One message at a time, in order: nothing is asked before the catalog is open.
let queue = Promise.resolve();
self.onmessage = ({ data }) => {
  if (data.type === "look") {
    look();
    return;
  }
  queue = queue.then(async () => {
    try {
      const handler = handlers[data.type];
      if (!handler) throw new Error(`unknown message ${data.type}`);
      if (data.type !== "start" && !app) throw new Error("the catalog is not open");
      self.postMessage({ id: data.id, ...(await handler(data)) });
    } catch (e) {
      self.postMessage({ id: data.id, error: e instanceof Error ? e.message : String(e), notice: e?.notice });
    }
  });
};
