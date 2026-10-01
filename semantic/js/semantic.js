// The semantic search for a page: the work happens in a Web Worker (worker.js), this is its
// face. Typing fast never piles up work: while a query runs, only the newest one waits, and the
// ones it replaced resolve to null.
//
//   import { Semantic, indexFromVectors } from "/pkg/semantic.js";
//   // The modules' vectors are in the snapshot (v_module_vector, computed by Radix):
//   const rows = db.exec("SELECT module_id, scale, vector FROM v_module_vector ORDER BY module_id")[0]?.values ?? [];
//   const semantic = new Semantic({ model: "/pkg/e5-de-en.bin", index: indexFromVectors(rows) });
//   await semantic.ready;                       // {rows, build, ms}: loaded (a second or two)
//   const found = await semantic.search("coding lernen", 20);
//   if (found) for (const { id, score } of found.hits) …   // null: a newer query took its place

export class Semantic {
  /** @param {{model: string, index: string | Uint8Array, worker?: string | URL}} urls the packed
   * model, the index (a URL, or its bytes: `indexFromVectors`), and the worker script (beside this
   * file unless given, asked for with this file's query: the build it came with, `?v=<build>`,
   * which the worker passes on to its WASM) */
  constructor({ model, index, worker = new URL("semantic-worker.js" + new URL(import.meta.url).search, import.meta.url) }) {
    this.worker = new Worker(worker);
    this.next = 0;
    this.pending = new Map();
    this.waiting = null; // the newest query, while another one runs
    this.running = false;
    this.worker.onmessage = ({ data }) => {
      const pending = this.pending.get(data.id);
      this.pending.delete(data.id);
      if (data.error) pending?.reject(new Error(data.error));
      else pending?.resolve(data);
    };
    this.worker.onerror = (event) => {
      for (const { reject } of this.pending.values()) reject(new Error(event.message || "the search worker failed"));
      this.pending.clear();
    };
    this.ready = this.#ask({ type: "init", model: String(new URL(model, location.href)), index: where(index) });
  }

  /** The `k` modules closest to `query`, best first: {hits: [{id, score}], ms}; null when a newer
   * query replaced this one before it ran. */
  search(query, k = 20) {
    if (this.waiting) this.waiting.resolve(null);
    return new Promise((resolve, reject) => {
      this.waiting = { query, k, resolve, reject };
      this.#pump();
    });
  }

  /** Loads another index (a new snapshot) into the worker: a URL or bytes. */
  setIndex(index) {
    return this.#ask({ type: "index", index: where(index) });
  }

  terminate() {
    this.worker.terminate();
  }

  async #pump() {
    if (this.running || !this.waiting) return;
    const { query, k, resolve, reject } = this.waiting;
    this.waiting = null;
    this.running = true;
    try {
      await this.ready;
      resolve(await this.#ask({ type: "search", query, k }));
    } catch (e) {
      reject(e);
    } finally {
      this.running = false;
      this.#pump();
    }
  }

  #ask(message) {
    const id = this.next++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...message, id });
    });
  }
}

/** An index (bytes) as an absolute URL, so the worker resolves it as the page does. */
function where(index) {
  return index instanceof Uint8Array ? index : String(new URL(index, location.href));
}

/**
 * The index of the semantic search (semantic/src/index.rs, „E5I3“) from the rows of
 * `SELECT module_id, scale, vector FROM v_module_vector ORDER BY module_id` as sql.js returns them
 * (the vector a Uint8Array, packed as Radix published it: the crate reads it). The same bytes as
 * `semantic::Index::push_codes` of the same rows makes, so the browser and the server search the
 * same index. Without rows (a snapshot whose vectors Radix has not computed yet) the index is empty
 * and every search finds nothing, as on the server.
 * @param {Array<[string, number, Uint8Array]>} rows
 * @param {number} dims the values of a vector, for an index without rows
 */
export function indexFromVectors(rows, dims = 384) {
  const encoder = new TextEncoder();
  const ids = rows.map(([id]) => encoder.encode(String(id)));
  const bytes = rows.length ? rows[0][2].length : dims / 2; // two values a byte
  const size = 12 + ids.reduce((n, id) => n + 2 + id.length, 0) + rows.length * (4 + bytes);
  const out = new Uint8Array(size);
  const view = new DataView(out.buffer);
  out.set([0x45, 0x35, 0x49, 0x33]); // "E5I3"
  view.setUint32(4, rows.length, true);
  view.setUint32(8, 2 * bytes, true);
  let at = 12;
  for (const id of ids) {
    view.setUint16(at, id.length, true);
    out.set(id, at + 2);
    at += 2 + id.length;
  }
  // The scale as the f32 Radix stored (REAL in SQLite holds it exactly).
  for (const [, scale] of rows) {
    view.setFloat32(at, scale, true);
    at += 4;
  }
  for (const [id, , vector] of rows) {
    if (vector.length !== bytes) throw new Error(`the vector of ${id} has ${vector.length} bytes, not ${bytes}`);
    out.set(vector, at);
    at += bytes;
  }
  return out;
}
