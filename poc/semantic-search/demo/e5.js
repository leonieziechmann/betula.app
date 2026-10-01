// The WASM model for JavaScript (browser and Node): load the packed model once, then embed
// queries. `semantic/src/wasm.rs` (feature `worker`) has the exports this wraps.

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

/** Which build of the runtime this engine runs: "relaxed" (relaxed SIMD) or "simd" (SIMD). */
export const build = WebAssembly.validate(RELAXED_PROBE) ? "relaxed" : "simd";

export class E5 {
  /** @param {BufferSource} wasm the module's bytes, @param {Uint8Array} model the packed model,
   * @param {"expand"|"f32"|"int8"} mode how the matrices are kept (semantic/src/tensor.rs `Mode`):
   * int8 is fastest (21 MB more memory), expand smallest, f32 in between but 85 MB more. */
  static async create(wasm, model, mode = "int8") {
    const { instance } = await WebAssembly.instantiate(wasm, {});
    const e5 = new E5(instance.exports);
    const at = e5.x.alloc(model.length);
    new Uint8Array(e5.x.memory.buffer, at, model.length).set(model);
    if (e5.x.load(at, model.length, { expand: 0, f32: 1, int8: 2 }[mode]) !== 0) throw new Error("not a packed e5 model");
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

/** Parses an index file (semantic/src/index.rs): "E5I3", u32 rows, u32 dims, rows × (u16 length,
 * id), rows × f32 scales, rows × dims / 2 bytes of 4-bit values (nibble = value + 8, the first in
 * the low nibble). The ids are skipped: a row is a module of index.json. */
export function readIndex(buffer) {
  const view = new DataView(buffer);
  const magic = new TextDecoder().decode(new Uint8Array(buffer, 0, 4));
  if (magic !== "E5I3") throw new Error("not an index");
  const rows = view.getUint32(4, true), dims = view.getUint32(8, true);
  let at = 12;
  for (let r = 0; r < rows; r++) at += 2 + view.getUint16(at, true);
  const scales = new Float32Array(buffer.slice(at, at + 4 * rows));
  const packed = new Uint8Array(buffer, at + 4 * rows, rows * dims / 2);
  const codes = new Int8Array(rows * dims);
  for (let i = 0; i < packed.length; i++) {
    codes[2 * i] = (packed[i] & 15) - 8;
    codes[2 * i + 1] = (packed[i] >> 4) - 8;
  }
  return { rows, dims, scales, codes };
}
