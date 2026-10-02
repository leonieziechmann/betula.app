// The data worker (docs/folia-refactor.md §6.2), step 1: sql.js in the worker. It keeps the
// snapshot in Cache Storage by its ETag, opens it, answers the page's questions through the Rust
// half (folia_worker), and looks for a new snapshot when it starts, when the page says it is
// visible again, and every POLL milliseconds; a new one is used at once (owner, 2026-10-02).
// Classic worker: importScripts works in every browser with workers.
const BUILD = self.location.search;
importScripts("/assets/sql-wasm.js" + BUILD, "/pkg/folia_worker.js" + BUILD);
const POLL = 5 * 60 * 1000;
const STORE = "folia-next-snapshot";

let SQL = null;
let current = null; // { etag, db }
let ready = null;

const say = (text) => self.postMessage({ type: "status", text });

async function status() {
  const response = await fetch("/api/status", { cache: "no-store" });
  if (!response.ok) throw new Error("/api/status: " + response.status);
  return (await response.json()).snapshot;
}

async function download(server) {
  const response = await fetch("/api/db", { cache: "no-store" });
  if (!response.ok) throw new Error("/api/db: " + response.status);
  const reader = response.body.getReader();
  const chunks = [];
  let received = 0, shown = -1;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    received += value.length;
    const percent = Math.min(99, Math.round((received / server.bytes) * 100));
    if (percent !== shown && !current) { shown = percent; say(`Daten werden geladen … ${percent} %`); }
  }
  const bytes = new Uint8Array(received);
  let at = 0;
  for (const chunk of chunks) { bytes.set(chunk, at); at += chunk.length; }
  return bytes;
}

async function kept() {
  const cache = await caches.open(STORE);
  const keys = await cache.keys();
  if (!keys.length) return null;
  const response = await cache.match(keys[0]);
  return { etag: decodeURIComponent(new URL(keys[0].url).searchParams.get("etag")), bytes: new Uint8Array(await response.arrayBuffer()) };
}

async function keep(etag, bytes) {
  const cache = await caches.open(STORE);
  for (const key of await cache.keys()) await cache.delete(key);
  await cache.put("/snapshot?etag=" + encodeURIComponent(etag), new Response(bytes));
}

function open(etag, bytes) {
  const db = new SQL.Database(bytes);
  const before = current;
  current = { etag, db };
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
  if (before) before.db.close();
  self.postMessage({ type: "snapshot", etag });
}

// Looks for a newer snapshot; a new one is downloaded beside the open one and used at once.
async function check() {
  let server;
  try { server = await status(); } catch { return; }
  if (current && server.etag === current.etag) return;
  const bytes = await download(server);
  await keep(server.etag, bytes).catch(() => {});
  open(server.etag, bytes);
  say("");
}

async function start() {
  const [, sql, copy] = await Promise.all([
    wasm_bindgen({ module_or_path: "/pkg/folia_worker_bg.wasm" + BUILD }),
    initSqlJs({ locateFile: (file) => "/assets/" + file + BUILD }),
    kept().catch(() => null),
  ]);
  SQL = sql;
  if (copy) open(copy.etag, copy.bytes);
  if (!current) await check();
  else check().catch((error) => console.warn("[worker] update failed", error));
  setInterval(() => check().catch(() => {}), POLL);
}
ready = start();

self.onmessage = async ({ data }) => {
  if (data && data.type === "visible") { check().catch(() => {}); return; }
  await ready;
  const reply = wasm_bindgen.answer(new Uint8Array(data), current.etag);
  self.postMessage(reply, [reply.buffer]);
};
