// The WASM model for JavaScript (browser and Node): load the packed model once, then embed
// queries. `runtime/src/wasm.rs` has the exports this wraps.

export class E5 {
  /** @param {BufferSource} wasm the module's bytes, @param {Uint8Array} model the packed model */
  static async create(wasm, model) {
    const { instance } = await WebAssembly.instantiate(wasm, {});
    const e5 = new E5(instance.exports);
    const at = e5.x.alloc(model.length);
    new Uint8Array(e5.x.memory.buffer, at, model.length).set(model);
    if (e5.x.load(at, model.length) !== 0) throw new Error("not a packed e5 model");
    e5.dims = e5.x.dims();
    e5.out = e5.x.alloc(4 * e5.dims);
    return e5;
  }

  constructor(exports) {
    this.x = exports;
    this.encoder = new TextEncoder();
  }

  /** The embedding of `text` (prefix included: "query: …"), and how many tokens it was. */
  embed(text) {
    const utf8 = this.encoder.encode(text.normalize("NFC"));
    const at = this.x.alloc(utf8.length);
    new Uint8Array(this.x.memory.buffer, at, utf8.length).set(utf8);
    const tokens = this.x.embed(at, utf8.length, this.out);
    this.x.free(at, utf8.length);
    if (tokens < 0) throw new Error("embedding failed");
    // The memory may have grown (and its buffer been replaced) during the call.
    return { vector: new Float32Array(this.x.memory.buffer, this.out, this.dims).slice(), tokens };
  }
}

/** The documents most similar to `query` (a unit vector): an index of int8 rows with one scale
 * each (`python/embed_catalog.py`). Returns [score, row] pairs, best first. */
export function nearest(index, query, k = 10) {
  const { rows, dims, codes, scales } = index;
  const scores = new Float32Array(rows);
  for (let r = 0; r < rows; r++) {
    let sum = 0;
    const base = r * dims;
    for (let i = 0; i < dims; i++) sum += codes[base + i] * query[i];
    scores[r] = sum * scales[r];
  }
  const order = Array.from(scores.keys()).sort((a, b) => scores[b] - scores[a]).slice(0, k);
  return order.map((r) => [scores[r], r]);
}

/** Parses an index file: "E5I1", u32 rows, u32 dims, rows × f32 scales, rows × dims int8. */
export function readIndex(buffer) {
  const view = new DataView(buffer);
  const magic = new TextDecoder().decode(new Uint8Array(buffer, 0, 4));
  if (magic !== "E5I1") throw new Error("not an index");
  const rows = view.getUint32(4, true), dims = view.getUint32(8, true);
  const scales = new Float32Array(buffer.slice(12, 12 + 4 * rows));
  const codes = new Int8Array(buffer, 12 + 4 * rows, rows * dims);
  return { rows, dims, scales, codes };
}
