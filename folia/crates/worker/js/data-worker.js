// The data worker (docs/folia/folia-refactor.md §6.2): the catalog in sql.js and the worker's own
// bundle (`folia-worker`, without Leptos), whose `worker_answer` answers every question of the
// app's pages (`folia_pages::ask`) beside the page's thread. The page's thread has no copy of the
// catalog: it asks here and builds what the answers say. scripts/build-client.sh puts this file
// and the bundle into site/pkg; `boot.js` starts it.
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
//   {id, type: "start", schema}           find, fetch and open the catalog (`schema`: the build's
//                                         SCHEMA_VERSION)
//                                         → {etag} | {error}
//   {id, type: "ask", name, question}     a question by its name and its fields in JSON → {answer}:
//                                         the JSON of its Result<Answer, DataError>
//   {id, type: "query", sql, params}      a statement of boot.js's own (the semantic search's
//                                         vectors) → {columns, rows}
//   {type: "look"}                        the tab is in view again: is there a newer snapshot?
//   {type: "prepare"}                     a search is coming: the search's index of the snapshot
//                                         is made (`worker_prepare_search`), once per snapshot
// Messages to the page, without an id:
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

// The catalog asks for a pass (folia/crates/server/src/pass.rs): without one the server answers
// 401 with a challenge, which this worker solves and hands in (`earnPass`); the browser keeps the
// pass as a cookie for a week, and the download is asked again. A pass of another network (a phone
// that changed it) or of a server started since is no pass: then the same once more.
async function download() {
  for (let attempt = 0; ; attempt++) {
    const response = await fetch("/api/db");
    if (response.status === 401 && attempt < 2) {
      await earnPass(await response.json());
      continue;
    }
    if (!response.ok) throw new Error("GET /api/db: HTTP " + response.status);
    return response.blob();
  }
}

// A pass: the work the challenge asks for, handed in. A challenge the server did not take (older
// than ten minutes, of another network, from before a restart) comes back with a new one.
async function earnPass(asked) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const nonce = await solve(asked.seed, asked.difficulty);
    const response = await fetch("/api/pass", { method: "POST", body: new URLSearchParams({ challenge: asked.challenge, nonce: String(nonce) }) });
    if (response.ok) return;
    if (response.status !== 403) throw new Error("POST /api/pass: HTTP " + response.status);
    asked = await response.json();
  }
  throw new Error("POST /api/pass: no pass taken");
}

// The work: the smallest number from 0 for which SHA-256 of the seed followed by the number in
// decimal digits begins with `difficulty` zero bits (about 2^difficulty tries; 18 by default, a
// tenth of a second on a laptop, half a second on a phone). SHA-256 of one block, in place: the
// seed (24 hex digits) and the number are ASCII and together shorter than 56 bytes. In slices,
// so that a question of the page is answered in between (`breathe`).
const K256 = new Int32Array([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);
const SLICE = 50000;
async function solve(seed, difficulty) {
  if (seed.length > 35) throw new Error("the pass's seed is too long for one block");
  for (let from = 0; ; from += SLICE) {
    const nonce = solveSlice(seed, difficulty, from, from + SLICE);
    if (nonce >= 0) return nonce;
    await breathe();
  }
}

// The numbers from `from` to `to` (without it) tried: the first that does it, or -1. A function of
// its own and not async: the browser compiles a loop inside an async function only so far, and
// the work was fifty times slower there (2026-11-18, Chromium).
const POW_BLOCK = new Uint8Array(64);
const POW_W = new Int32Array(64);
function solveSlice(seed, difficulty, from, to) {
  const block = POW_BLOCK;
  for (let i = 0; i < seed.length; i++) block[i] = seed.charCodeAt(i);
  for (let nonce = from; nonce < to; nonce++) {
    const digits = String(nonce);
    for (let i = 0; i < digits.length; i++) block[seed.length + i] = digits.charCodeAt(i);
    hash(seed.length + digits.length);
    const zeros = POW_HASH[0] ? Math.clz32(POW_HASH[0]) : 32 + Math.clz32(POW_HASH[1]);
    if (zeros >= difficulty) return nonce;
  }
  return -1;
}

// The first 64 bits of the SHA-256 of the block's first `length` bytes, into `POW_HASH`. (Counting
// their zero bits in here made the browser give up compiling it, 2026-11-18.)
const POW_HASH = new Int32Array(2);
function hash(length) {
  const block = POW_BLOCK;
  const w = POW_W;
  block[length] = 0x80;
  block.fill(0, length + 1, 62);
  block[62] = (length * 8) >>> 8;
  block[63] = (length * 8) & 0xff;
  for (let i = 0; i < 16; i++) w[i] = (block[4 * i] << 24) | (block[4 * i + 1] << 16) | (block[4 * i + 2] << 8) | block[4 * i + 3];
  for (let i = 16; i < 64; i++) {
    const x = w[i - 15], y = w[i - 2];
    w[i] = ((((x >>> 7) | (x << 25)) ^ ((x >>> 18) | (x << 14)) ^ (x >>> 3)) + w[i - 7] + (((y >>> 17) | (y << 15)) ^ ((y >>> 19) | (y << 13)) ^ (y >>> 10)) + w[i - 16]) | 0;
  }
  let a = 0x6a09e667 | 0, b = 0xbb67ae85 | 0, c = 0x3c6ef372 | 0, d = 0xa54ff53a | 0;
  let e = 0x510e527f | 0, f = 0x9b05688c | 0, g = 0x1f83d9ab | 0, h = 0x5be0cd19 | 0;
  for (let i = 0; i < 64; i++) {
    const t1 = (h + (((e >>> 6) | (e << 26)) ^ ((e >>> 11) | (e << 21)) ^ ((e >>> 25) | (e << 7))) + ((e & f) ^ (~e & g)) + K256[i] + w[i]) | 0;
    const t2 = ((((a >>> 2) | (a << 30)) ^ ((a >>> 13) | (a << 19)) ^ ((a >>> 22) | (a << 10))) + ((a & b) ^ (a & c) ^ (b & c))) | 0;
    h = g; g = f; f = e; e = (d + t1) | 0; d = c; c = b; b = a; a = (t1 + t2) | 0;
  }
  POW_HASH[0] = (a + 0x6a09e667) | 0;
  POW_HASH[1] = (b + 0xbb67ae85) | 0;
}

// A moment for the messages waiting: a task of its own. Not `setTimeout`, which a browser holds
// back to once a second where it thinks nobody looks (headless, a tab in the background).
const breather = new MessageChannel();
const breaths = [];
breather.port1.onmessage = () => breaths.shift()?.();
function breathe() {
  return new Promise((resolve) => {
    breaths.push(resolve);
    breather.port2.postMessage(0);
  });
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
  if (prepareWanted) prepare();
}

// The search's index of the snapshot in use (`folia_search::prepare`: the names of every module,
// a quarter of a second of a laptop, a phone's second), made once the page says a search is
// coming, so that the first search does not wait for it; once per snapshot, and only when no
// question has come for a moment (`PREPARE_QUIET_MS`): the questions of a page that is being
// built go first. Asked before the catalog is open, it is made once it is and things are quiet.
const PREPARE_QUIET_MS = 300;
let prepareWanted = false;
let preparedFor = null;
let prepareTimer = 0;
function prepare() {
  prepareWanted = true;
  clearTimeout(prepareTimer);
  if (!app || !current || preparedFor === current.etag) return;
  const etag = current.etag;
  prepareTimer = setTimeout(() => {
    queue = queue.then(() => {
      if (current?.etag !== etag || preparedFor === etag) return;
      preparedFor = etag;
      try {
        app.worker_prepare_search();
      } catch (error) {
        console.warn("[catalog] the search's index", error);
      }
    });
  }, PREPARE_QUIET_MS);
}

async function start(build) {
  schema = build;
  importScripts("/assets/sql-wasm.js" + BUILD);
  const [sql, bundle] = await Promise.all([
    self.initSqlJs({ locateFile: (file) => "/assets/" + file + BUILD }),
    import("/pkg/folia_worker.js" + BUILD).then(async (bundle) => {
      await bundle.default({ module_or_path: "/pkg/folia_worker_bg.wasm" + BUILD });
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
    if (!server) throw new Error("the local copy of the catalog is older than this build, and the server is not reachable");
    kept = null;
  }
  if (!kept) {
    if (!server) throw new Error("no local copy of the catalog and the server is not reachable");
    if (!serverFits) throw new Error(`the server's catalog is of schema ${server.schema_version}, this build reads ${schema}`);
    kept = await oneAtATime(async () => {
      // Another tab may have kept it meanwhile.
      const theirs = await idbGet(KEPT).catch(() => null);
      if (theirs?.etag === server.etag) return theirs;
      const fresh = { etag: server.etag, blob: await download() };
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
  if (prepareWanted) prepare();
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
      const fresh = { etag: server.etag, blob: await download() };
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
  start({ schema }) {
    return start(schema);
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
  if (data.type === "prepare") {
    prepare();
    return;
  }
  // A question puts the index off until things are quiet again (a search makes it itself).
  if (prepareTimer && preparedFor !== current?.etag) prepare();
  queue = queue.then(async () => {
    try {
      const handler = handlers[data.type];
      if (!handler) throw new Error(`unknown message ${data.type}`);
      if (data.type !== "start" && !app) throw new Error("the catalog is not open");
      self.postMessage({ id: data.id, ...(await handler(data)) });
    } catch (e) {
      self.postMessage({ id: data.id, error: e instanceof Error ? e.message : String(e) });
    }
  });
};
