// The Web Worker of the semantic search: the model, the index and every query live here, so a
// search never blocks the page (embedding a query takes 15–60 ms on a laptop, several times that
// on a phone). A page talks to it through `semantic.js`; scripts/build-semantic.sh puts both, and
// the two builds of the WASM (semantic/src/wasm.rs), into site/pkg.
//
// Messages (each answer carries the `id` of its request):
//   {id, type: "init", model, index}   URL of the packed model; the index's URL or bytes → {rows, ms}
//   {id, type: "index", index}         another index (a new snapshot), URL or bytes → {rows, ms}
//   {id, type: "search", query, k}     → {hits: [{id, score}], ms}
//   any failure                        → {error}
//
// A plain (not module) worker: no imports, so it runs in every browser that has workers.

// A module with one relaxed-SIMD instruction (i32x4.relaxed_dot_i8x16_i7x16_add_s): valid where
// the engine has relaxed SIMD (Chrome 114+, Firefox 120+), not in Safari.
const RELAXED_PROBE = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // \0asm, version 1
  0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7b, // type: () -> v128
  0x03, 0x02, 0x01, 0x00, // one function of that type
  0x0a, 0x3d, 0x01, 0x3b, 0x00, // code: one body of 59 bytes, no locals
  ...[0, 1, 2].flatMap(() => [0xfd, 0x0c, ...new Array(16).fill(0)]), // v128.const 0, three times
  0xfd, 0x93, 0x02, 0x0b, // i32x4.relaxed_dot_i8x16_i7x16_add_s, end
]);
const BUILD = WebAssembly.validate(RELAXED_PROBE) ? "relaxed" : "simd";

let x = null; // the module's exports
const encoder = new TextEncoder(), decoder = new TextDecoder();

/** The bytes at a URL, or the bytes themselves (an index the page built: `indexFromVectors`). */
async function bytes(url, init) {
  if (url instanceof Uint8Array) return url;
  const response = await fetch(url, init);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return new Uint8Array(await response.arrayBuffer());
}

/** Copies `data` into the module's memory; returns where it is. */
function put(data) {
  const at = x.alloc(data.length);
  new Uint8Array(x.memory.buffer, at, data.length).set(data);
  return at;
}

async function loadIndex(url) {
  const index = await bytes(url);
  const at = put(index);
  const rows = x.load_index(at, index.length);
  x.free(at, index.length);
  if (rows < 0) throw new Error("not an index");
  return rows;
}

const handlers = {
  async init({ model, index }) {
    // The WASM of this worker's build (its `?v=<build>`); the model, 15 MB, after what the page
    // needs (`priority`, where the browser has it).
    const [wasm, modelBytes] = await Promise.all([
      bytes(new URL(`semantic.${BUILD}.wasm${self.location.search}`, self.location.href)),
      bytes(model, { priority: "low" }),
    ]);
    const indexBytes = await bytes(index);
    x = (await WebAssembly.instantiate(wasm, {})).instance.exports;
    // The model stays in the module's memory (18.5 MB, and 21 MB of int8 weights made from it).
    const at = put(indexBytes);
    const rows = x.load_search(put(modelBytes), modelBytes.length, at, indexBytes.length);
    x.free(at, indexBytes.length);
    if (rows < 0) throw new Error(`${model}: not a packed model, or the index is none`);
    return { rows, build: BUILD };
  },

  async index({ index }) {
    return { rows: await loadIndex(index) };
  },

  search({ query, k = 20 }) {
    if (!x) throw new Error("search before init");
    // As typed, not normalised here: the server gets the same string and must find the same.
    const text = encoder.encode(String(query));
    const t = put(text), rows = x.alloc(4 * k), scores = x.alloc(4 * k), len = x.alloc(4);
    try {
      const n = x.search(t, text.length, k, rows, scores);
      if (n < 0) throw new Error("no model or index");
      // Views only after the call: the memory may have grown, and its buffer been replaced.
      const rowView = new Uint32Array(x.memory.buffer, rows, n), scoreView = new Float32Array(x.memory.buffer, scores, n);
      const hits = [];
      for (let i = 0; i < n; i++) {
        const at = x.id_of(rowView[i], len);
        const length = new Uint32Array(x.memory.buffer, len, 1)[0];
        hits.push({ id: decoder.decode(new Uint8Array(x.memory.buffer, at, length)), score: scoreView[i] });
      }
      return { hits };
    } finally {
      x.free(t, text.length);
      x.free(rows, 4 * k);
      x.free(scores, 4 * k);
      x.free(len, 4);
    }
  },
};

// One message at a time, in order: an `index` never interleaves with a search.
let queue = Promise.resolve();
self.onmessage = ({ data }) => {
  queue = queue.then(async () => {
    const started = performance.now();
    try {
      const handler = handlers[data.type];
      if (!handler) throw new Error(`unknown message ${data.type}`);
      const answer = await handler(data);
      self.postMessage({ id: data.id, ...answer, ms: performance.now() - started });
    } catch (e) {
      self.postMessage({ id: data.id, error: e instanceof Error ? e.message : String(e) });
    }
  });
};
