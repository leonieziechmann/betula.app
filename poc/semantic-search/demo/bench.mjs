// node bench.mjs WASM MODEL [--check embeddings.jsonl]
//
// How long a query takes in V8's WebAssembly (the engine of Chrome, Edge and Node), from the
// text to the unit vector, for queries of different lengths; the median of 20 runs each. With
// --check, the embeddings are compared with those of the native binary (`embed MODEL < texts`).

import { readFile } from "node:fs/promises";
import { E5 } from "./e5.js";

const [wasmPath, modelPath, flag, checkPath] = process.argv.slice(2);
const queries = [
  "query: Statik",
  "query: maschinelles lernen",
  "query: Einführung in die Programmierung für Ingenieure",
  "query: Welche Module behandeln erneuerbare Energien und Klimaschutz im Bauwesen?",
  "query: I am looking for a course about the history of architecture and urban planning in the 20th century",
];

let started = performance.now();
const [wasm, model] = await Promise.all([readFile(wasmPath), readFile(modelPath)]);
const read = performance.now() - started;
started = performance.now();
const e5 = await E5.create(wasm, model);
console.log(`read ${(read).toFixed(0)} ms, instantiate + load ${(performance.now() - started).toFixed(0)} ms, ` +
  `memory ${(e5.x.memory.buffer.byteLength / 2 ** 20).toFixed(1)} MiB`);

for (const q of queries) {
  e5.embed(q); // warm-up
  const times = [];
  let tokens = 0;
  for (let i = 0; i < 20; i++) {
    const t = performance.now();
    tokens = e5.embed(q).tokens;
    times.push(performance.now() - t);
  }
  times.sort((a, b) => a - b);
  console.log(`${times[10].toFixed(1).padStart(6)} ms  ${String(tokens).padStart(3)} tokens  ${q}`);
}

if (flag === "--check") {
  const lines = (await readFile(checkPath, "utf8")).trim().split("\n").map((l) => JSON.parse(l));
  const texts = (await readFile(checkPath.replace(/\.jsonl$/, ".txt"), "utf8")).split("\n").slice(0, lines.length);
  let worst = 1;
  texts.forEach((text, i) => {
    const { vector } = e5.embed(text);
    let dot = 0;
    vector.forEach((v, j) => (dot += v * lines[i].embedding[j]));
    worst = Math.min(worst, dot);
  });
  console.log(`${texts.length} texts: cosine to the native embeddings at least ${worst.toFixed(7)}`);
}
