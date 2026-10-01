// node semantic/js/parity.mjs MODEL.bin INDEX.bin QUERIES.txt WASM… [--k N]
//
// What the browser's builds of the worker (semantic.simd.wasm, semantic.relaxed.wasm) make of
// each query, as the line `embed MODEL.bin --search INDEX.bin` writes natively: a hash of the
// embedding's bits, then the hits with the bits of their scores. Prints one block of lines per
// build; the check is that they are the native output exactly:
//
//   target/release/embed MODEL --search INDEX < QUERIES > native.txt
//   node semantic/js/parity.mjs MODEL INDEX QUERIES site/pkg/semantic.simd.wasm  > simd.txt
//   cmp native.txt simd.txt

import { readFile } from "node:fs/promises";

const args = process.argv.slice(2);
const k = args.includes("--k") ? Number(args[args.indexOf("--k") + 1]) : 10;
const [modelPath, indexPath, queriesPath, ...wasms] = args.filter((a, i) => a !== "--k" && args[i - 1] !== "--k");
const [model, index] = await Promise.all([readFile(modelPath), readFile(indexPath)]);
const queries = (await readFile(queriesPath, "utf8")).split("\n");
if (queries.at(-1) === "") queries.pop();

const fnv = (bytes) => bytes.reduce((h, b) => Math.imul(h ^ b, 0x01000193) >>> 0, 0x811c9dc5);
const hex = (n) => n.toString(16).padStart(8, "0");

for (const path of wasms) {
  const x = (await WebAssembly.instantiate(await readFile(path), {})).instance.exports;
  const put = (data) => { const at = x.alloc(data.length); new Uint8Array(x.memory.buffer, at, data.length).set(data); return at; };
  const at = put(index);
  if (x.load_search(put(model), model.length, at, index.length) < 0) throw new Error(`${path}: the model or the index did not load`);
  x.free(at, index.length);
  const encoder = new TextEncoder(), decoder = new TextDecoder();
  const out = x.alloc(4 * 384), rows = x.alloc(4 * k), scores = x.alloc(4 * k), len = x.alloc(4);
  const lines = [];
  for (const query of queries) {
    const text = encoder.encode(query);
    const t = put(text);
    x.embed_query(t, text.length, out);
    const embedding = new Uint8Array(x.memory.buffer.slice(out, out + 4 * 384));
    const n = x.search(t, text.length, k, rows, scores);
    x.free(t, text.length);
    const hits = [];
    for (let i = 0; i < n; i++) {
      const row = new Uint32Array(x.memory.buffer, rows, n)[i];
      const bits = new Uint32Array(x.memory.buffer, scores, n)[i];
      const idAt = x.id_of(row, len);
      const id = decoder.decode(new Uint8Array(x.memory.buffer, idAt, new Uint32Array(x.memory.buffer, len, 1)[0]));
      hits.push(`${id}:${hex(bits)}`);
    }
    lines.push(`${hex(fnv(embedding))}\t${hits.join(" ")}`);
  }
  process.stdout.write(lines.join("\n") + "\n");
}
