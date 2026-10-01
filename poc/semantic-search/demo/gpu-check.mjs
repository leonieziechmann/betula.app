// node gpu-check.mjs URL [CHROMIUM]
//
// The WebGPU encoder (e5-gpu.js) against the WASM runtime in expand mode (the same arithmetic)
// in Chromium: the cosine of their embeddings for the demo's queries and catalog titles, and
// how long a query takes on each. URL serves e5.js, e5-gpu.js, e5_mini.simd.wasm, e5-de-en.bin
// and index.json. Without a GPU, Chromium runs WebGPU on SwiftShader (on the CPU): right for
// the numbers, meaningless for the speed. Needs playwright-core.

import { chromium } from "playwright-core";

const [url, executablePath = "/opt/pw-browsers/chromium-1194/chrome-linux/chrome"] = process.argv.slice(2);
const browser = await chromium.launch({ executablePath, args: ["--enable-unsafe-webgpu"] });
const page = await browser.newPage();
page.on("console", (m) => console.log("console:", m.text()));
await page.goto(new URL("index.json", url).href);

const result = await page.evaluate(async () => {
  const { E5Gpu, TOKENS } = await import("./e5-gpu.js");
  const { E5 } = await import("./e5.js");
  const get = async (u) => new Uint8Array(await (await fetch(u)).arrayBuffer());
  const [wasm, model, modules] = await Promise.all([get("e5_mini.simd.wasm"), get("e5-de-en.bin"), fetch("index.json").then((r) => r.json())]);
  const adapter = await navigator.gpu.requestAdapter();
  let t = performance.now();
  const gpu = await E5Gpu.create(wasm, model);
  const setup = performance.now() - t;
  const cpu = await E5.create(wasm, model, "expand");

  const queries = ["query: Statik", "query: maschinelles lernen", "query: Einführung in die Programmierung für Ingenieure",
    "query: Welche Module behandeln erneuerbare Energien und Klimaschutz im Bauwesen?",
    "query: I am looking for a course about the history of architecture and urban planning in the 20th century"];
  const awkward = ["query: ÄÖÜ äöü ß ẞ", "query: ﬁnance ﬂow", "query:    lots   of\tspace\n", "query: 😀 emoji 🚀", "query: 数据库",
    "query: x", "query: " + "sehr ".repeat(80) + "lang"];
  const texts = [...queries, ...awkward, ...modules.slice(0, 40).map((m) => "query: " + m.title)];
  let worst = 1, compared = 0, cut = 0;
  for (const text of texts) {
    const g = await gpu.embed(text);
    const c = cpu.embed(text);
    if (c.tokens > TOKENS) { cut++; continue; }
    let dot = 0;
    for (let i = 0; i < g.vector.length; i++) dot += g.vector[i] * c.vector[i];
    worst = Math.min(worst, dot);
    compared++;
  }
  const time = async (f) => {
    const times = [];
    for (let i = 0; i < 5; i++) { const s = performance.now(); await f(); times.push(performance.now() - s); }
    return times.sort((a, b) => a - b)[2];
  };
  const speed = [];
  for (const q of queries) {
    speed.push({ q, tokens: cpu.embed(q).tokens, gpu: await time(() => gpu.embed(q)), wasm: await time(() => cpu.embed(q)) });
  }
  return { adapter: adapter.info?.architecture, setup, compared, cut, worst, speed };
});

console.log(`adapter ${result.adapter}; set-up (upload, pipelines, one warm-up query) ${result.setup.toFixed(0)} ms`);
console.log(`${result.compared} texts: cosine GPU ↔ WASM expand at least ${result.worst.toFixed(7)}; ${result.cut} longer than the GPU's tokens not compared`);
for (const s of result.speed) {
  console.log(`  GPU ${s.gpu.toFixed(1).padStart(6)} ms   WASM expand ${s.wasm.toFixed(1).padStart(6)} ms   ${String(s.tokens).padStart(3)} tokens  ${s.q}`);
}
await browser.close();
