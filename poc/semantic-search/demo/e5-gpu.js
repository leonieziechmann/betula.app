// The encoder on the GPU (WebGPU), the WASM runtime as the cold fallback.
//
// `createEmbedder` is what an app calls once at start: it sets up everything the GPU needs —
// weights uploaded as they are in the file (4 bit, expanded in the shaders), every buffer,
// pipeline and bind group for a fixed number of tokens (64) — and runs one query so the
// shaders are compiled. A query then costs a tokenisation (WASM, only the tokenizer loaded),
// two small buffer writes, one command buffer of ~110 dispatches and the read-back of 384
// floats. Without WebGPU, or once the GPU fails (device lost, an error), the same calls go to
// the WASM runtime (`e5.js`), which is only loaded then.
//
// The arithmetic is that of `runtime/` in expand mode (f32, the same GELU), so the embeddings
// agree with it to float rounding (`demo/gpu-check.mjs`).

import { E5 } from "./e5.js";

/** Tokens per query on the GPU: longer queries are cut off, `</s>` put at the end. */
export const TOKENS = 64;
const PAD = 1, EOS = 2;

/** An embedder: `backend` is "webgpu" or "wasm"; `embed(text)` gives {vector, tokens}.
 * @param {{wasm: BufferSource, model: () => Promise<Uint8Array>, gpu?: boolean}} options
 *   wasm: the runtime's bytes (for the tokenizer, and the fallback); model: fetches the packed
 *   model — called once for the GPU and again only if it has to fall back. */
export async function createEmbedder({ wasm, model, gpu = true }) {
  const embedder = new Embedder(wasm, model);
  if (gpu) {
    try {
      embedder.gpu = await E5Gpu.create(wasm, await model());
      embedder.backend = "webgpu";
      return embedder;
    } catch (e) {
      embedder.reason = e.message;
    }
  }
  await embedder.fallBack();
  return embedder;
}

class Embedder {
  constructor(wasm, model) {
    this.wasm = wasm;
    this.model = model;
    this.gpu = null;
    this.cpu = null;
    this.backend = "wasm";
    this.reason = "";
  }

  async fallBack() {
    this.gpu?.destroy();
    this.gpu = null;
    this.backend = "wasm";
    this.cpu ??= await E5.create(this.wasm, await this.model(), "int8");
  }

  async embed(text) {
    if (this.gpu) {
      try {
        return await this.gpu.embed(text);
      } catch (e) {
        this.reason = e.message;
        await this.fallBack();
      }
    }
    return this.cpu.embed(text);
  }
}

// ---------------------------------------------------------------------------------------------
// The packed file (python/pack.py)

function readModel(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let at = 0;
  const u8 = () => bytes[at++];
  const u32 = () => ((at += 4), view.getUint32(at - 4, true));
  const take = (n) => ((at += n), bytes.subarray(at - n, at));
  if (new TextDecoder().decode(take(4)) !== "E5Q1") throw new Error("not a packed e5 model");
  const [hidden, layers, heads, intermediate, positions, vocab] = [u32(), u32(), u32(), u32(), u32(), u32()];
  for (let i = 0; i < vocab; i++) { const len = u8(); at += len + 4; }
  for (let i = u32(); i > 0; i--) at += 4 + 1 + bytes[at + 4];
  const tokenizerEnd = at;

  const f32s = (n) => new Float32Array(take(4 * n).slice().buffer);
  const tensor = () => {
    const kind = u8(), rows = u32(), cols = u32(), count = rows * cols;
    if (kind === 0) return { kind: "f32", rows, cols, data: f32s(count) };
    if (kind !== 1 && kind !== 2) throw new Error(`tensor kind ${kind} is not supported on the GPU`);
    const block = u32();
    const levels = kind === 1 ? f32s(16) : null;
    const scales = halves(take(2 * (count / block)));
    const codes = take(kind === 1 ? count / 2 : count);
    return { kind: kind === 1 ? "q4" : "q8", rows, cols, block, levels, scales, codes };
  };
  const linear = () => ({ weight: tensor(), bias: tensor().data });
  const norm = () => ({ weight: tensor().data, bias: tensor().data });
  const model = { hidden, heads, intermediate, positions, vocab, tokenizerEnd };
  model.words = tensor();
  model.position = tensor();
  model.norm = norm();
  model.layers = Array.from({ length: layers }, () => ({
    query: linear(), key: linear(), value: linear(), output: linear(), attentionNorm: norm(),
    intermediate: linear(), out: linear(), outputNorm: norm(),
  }));
  if (at !== bytes.length) throw new Error("the model file has bytes left over");
  return model;
}

/** f16 (little endian) to f32. */
function halves(bytes) {
  const out = new Float32Array(bytes.length / 2);
  for (let i = 0; i < out.length; i++) {
    const h = bytes[2 * i] | (bytes[2 * i + 1] << 8);
    const exponent = (h >> 10) & 31, fraction = h & 1023, sign = h & 0x8000 ? -1 : 1;
    out[i] = exponent === 0 ? sign * 2 ** -14 * (fraction / 1024)
      : exponent === 31 ? (fraction ? NaN : sign * Infinity)
      : sign * 2 ** (exponent - 15) * (1 + fraction / 1024);
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// Shaders. Buffers: `codes` (u32: every 4- and 8-bit code, as in the file), `params` (f32: scales,
// levels, biases, LayerNorms), activations T × H in f32; `info.n` the tokens of this query.

const shaders = ({ H, T, D }) => {
  const reduce = (size) => /* wgsl */ `
var<workgroup> red: array<f32, ${size}>;
fn total(li: u32, v: f32) -> f32 {
  red[li] = v;
  workgroupBarrier();
  for (var s = ${size / 2}u; s > 0u; s >>= 1u) {
    if (li < s) { red[li] += red[li + s]; }
    workgroupBarrier();
  }
  let r = red[0];
  workgroupBarrier();
  return r;
}
fn largest(li: u32, v: f32) -> f32 {
  red[li] = v;
  workgroupBarrier();
  for (var s = ${size / 2}u; s > 0u; s >>= 1u) {
    if (li < s) { red[li] = max(red[li], red[li + s]); }
    workgroupBarrier();
  }
  let r = red[0];
  workgroupBarrier();
  return r;
}`;
  const PER = H / 128;
  const meta = "struct Meta { n: u32 }";

  // Token + position embedding, then LayerNorm: a workgroup a token (all T, padding included).
  const embed = /* wgsl */ `
struct Args { wc: u32, ws: u32, wl: u32, wb: u32, pc: u32, ps: u32, pb: u32, nw: u32, nb: u32 }
@group(0) @binding(0) var<uniform> a: Args;
@group(0) @binding(1) var<storage, read> ids: array<u32>;
@group(0) @binding(2) var<storage, read> codes: array<u32>;
@group(0) @binding(3) var<storage, read> params: array<f32>;
@group(0) @binding(4) var<storage, read_write> x: array<f32>;
${reduce(128)}
@compute @workgroup_size(128)
fn main(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_index) li: u32) {
  let t = wg.x;
  let id = ids[t];
  var v: array<f32, ${PER}>;
  var sum = 0.0;
  for (var r = 0u; r < ${PER}u; r++) {
    let i = li + r * 128u;
    let wi = id * ${H}u + i;
    let code = (codes[a.wc + wi / 8u] >> (4u * (wi % 8u))) & 15u;
    let word = params[a.wl + code] * params[a.ws + wi / a.wb];
    let pi = t * ${H}u + i;
    let byte = (codes[a.pc + pi / 4u] >> (8u * (pi % 4u))) & 255u;
    let position = f32(bitcast<i32>(byte << 24u) >> 24u) * params[a.ps + pi / a.pb];
    v[r] = word + position;
    sum += v[r];
  }
  let mean = total(li, sum) / ${H}.0;
  var sq = 0.0;
  for (var r = 0u; r < ${PER}u; r++) { let d = v[r] - mean; sq += d * d; }
  let inverse = 1.0 / sqrt(total(li, sq) / ${H}.0 + 1e-12);
  for (var r = 0u; r < ${PER}u; r++) {
    let i = li + r * 128u;
    x[t * ${H}u + i] = (v[r] - mean) * inverse * params[a.nw + i] + params[a.nb + i];
  }
}`;

  // y = x · Wᵀ + b (then GELU, or + residual), W in 4 bits, blocks of 32 along K. A workgroup
  // makes 16 tokens × 64 outputs; per block of K it expands the 64 × 32 weights into shared
  // memory once for all 16 tokens. Tiles past the query's tokens return at once.
  const matmul = (gelu, residual) => /* wgsl */ `
struct Args { k: u32, n: u32, codes: u32, scales: u32, levels: u32, bias: u32 }
${meta}
@group(0) @binding(0) var<uniform> a: Args;
@group(0) @binding(1) var<uniform> info: Meta;
@group(0) @binding(2) var<storage, read> codes: array<u32>;
@group(0) @binding(3) var<storage, read> params: array<f32>;
@group(0) @binding(4) var<storage, read> x: array<f32>;
${residual ? "@group(0) @binding(5) var<storage, read> residual: array<f32>;" : ""}
@group(0) @binding(6) var<storage, read_write> y: array<f32>;
var<workgroup> xs: array<f32, 512>;     // [16 tokens][32]
var<workgroup> ws: array<vec4f, 512>;   // [32][16 × 4 outputs]
var<workgroup> table: array<f32, 16>;

fn erf(x0: f32) -> f32 {  // runtime/src/tensor.rs erf: Eigen's rational approximation
  let x = clamp(x0, -4.0, 4.0);
  let x2 = x * x;
  var p = -2.7261423e-10;
  p = p * x2 + 2.7706814e-8;
  p = p * x2 - 2.101024e-6;
  p = p * x2 - 5.6925066e-5;
  p = p * x2 - 7.3499063e-4;
  p = p * x2 - 2.9546001e-3;
  p = p * x2 - 1.6096033e-2;
  var q = -1.4566072e-5;
  q = q * x2 - 2.1337406e-4;
  q = q * x2 - 1.682827e-3;
  q = q * x2 - 7.3733292e-3;
  q = q * x2 - 1.426474e-2;
  return p * x / q;
}

@compute @workgroup_size(256)
fn main(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_index) li: u32) {
  let t0 = wg.y * 16u;
  if (t0 >= info.n) { return; }
  let n0 = wg.x * 64u;
  if (li < 16u) { table[li] = params[a.levels + li]; }
  let tm = li / 16u;     // this thread's token in the tile
  let g = li % 16u;      // and its 4 outputs
  let row = li / 4u;     // the weight row it expands
  let part = li % 4u;    // 8 of that row's 32 codes
  let blocks = a.k / 32u;
  var acc = vec4f(0.0);
  workgroupBarrier();
  for (var b = 0u; b < blocks; b++) {
    for (var i = li; i < 512u; i += 256u) { xs[i] = x[(t0 + i / 32u) * a.k + b * 32u + i % 32u]; }
    let r = n0 + row;
    let scale = params[a.scales + r * blocks + b];
    let packed = codes[a.codes + (r * a.k + b * 32u) / 8u + part];
    for (var j = 0u; j < 8u; j++) {
      ws[(part * 8u + j) * 16u + row / 4u][row % 4u] = table[(packed >> (4u * j)) & 15u] * scale;
    }
    workgroupBarrier();
    for (var c = 0u; c < 32u; c++) { acc += xs[tm * 32u + c] * ws[c * 16u + g]; }
    workgroupBarrier();
  }
  let t = t0 + tm;
  for (var j = 0u; j < 4u; j++) {
    let col = n0 + g * 4u + j;
    var v = acc[j] + params[a.bias + col];
    ${gelu ? "v = 0.5 * v * (1.0 + erf(v * 0.70710678));" : ""}
    ${residual ? "v += residual[t * a.n + col];" : ""}
    y[t * a.n + col] = v;
  }
}`;

  // LayerNorm in place, a workgroup a token.
  const norm = /* wgsl */ `
struct Args { w: u32, b: u32 }
${meta}
@group(0) @binding(0) var<uniform> a: Args;
@group(0) @binding(1) var<uniform> info: Meta;
@group(0) @binding(2) var<storage, read> params: array<f32>;
@group(0) @binding(3) var<storage, read_write> x: array<f32>;
${reduce(128)}
@compute @workgroup_size(128)
fn main(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_index) li: u32) {
  let t = wg.x;
  if (t >= info.n) { return; }
  var v: array<f32, ${PER}>;
  var sum = 0.0;
  for (var r = 0u; r < ${PER}u; r++) { v[r] = x[t * ${H}u + li + r * 128u]; sum += v[r]; }
  let mean = total(li, sum) / ${H}.0;
  var sq = 0.0;
  for (var r = 0u; r < ${PER}u; r++) { let d = v[r] - mean; sq += d * d; }
  let inverse = 1.0 / sqrt(total(li, sq) / ${H}.0 + 1e-12);
  for (var r = 0u; r < ${PER}u; r++) {
    let i = li + r * 128u;
    x[t * ${H}u + i] = (v[r] - mean) * inverse * params[a.w + i] + params[a.b + i];
  }
}`;

  // Attention of one token in one head over the query's tokens: a thread a key.
  const attention = /* wgsl */ `
${meta}
@group(0) @binding(0) var<uniform> info: Meta;
@group(0) @binding(1) var<storage, read> q: array<f32>;
@group(0) @binding(2) var<storage, read> k: array<f32>;
@group(0) @binding(3) var<storage, read> v: array<f32>;
@group(0) @binding(4) var<storage, read_write> context: array<f32>;
${reduce(T)}
var<workgroup> p: array<f32, ${T}>;
@compute @workgroup_size(${T})
fn main(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_index) li: u32) {
  let t = wg.x;
  let n = info.n;
  if (t >= n) { return; }
  let head = wg.y * ${D}u;
  var s = -3.0e38;
  if (li < n) {
    s = 0.0;
    for (var c = 0u; c < ${D}u; c++) { s += q[t * ${H}u + head + c] * k[li * ${H}u + head + c]; }
    s *= ${1 / Math.sqrt(D)};
  }
  let e = select(0.0, exp(s - largest(li, s)), li < n);
  p[li] = e / total(li, e);
  workgroupBarrier();
  if (li < ${D}u) {
    var acc = 0.0;
    for (var j = 0u; j < n; j++) { acc += p[j] * v[j * ${H}u + head + li]; }
    context[t * ${H}u + head + li] = acc;
  }
}`;

  // The sum over the query's tokens, scaled to length 1.
  const pool = /* wgsl */ `
${meta}
@group(0) @binding(0) var<uniform> info: Meta;
@group(0) @binding(1) var<storage, read> x: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
${reduce(128)}
@compute @workgroup_size(128)
fn main(@builtin(local_invocation_index) li: u32) {
  var s: array<f32, ${PER}>;
  var sq = 0.0;
  for (var r = 0u; r < ${PER}u; r++) {
    for (var t = 0u; t < info.n; t++) { s[r] += x[t * ${H}u + li + r * 128u]; }
    sq += s[r] * s[r];
  }
  let length = max(sqrt(total(li, sq)), 1e-12);
  for (var r = 0u; r < ${PER}u; r++) { out[li + r * 128u] = s[r] / length; }
}`;

  return { embed, matmul, norm, attention, pool };
};

// ---------------------------------------------------------------------------------------------

export class E5Gpu {
  /** Everything the GPU needs, set up once; throws where there is no usable WebGPU.
   * @param {BufferSource} wasm the runtime (for the tokenizer), @param {Uint8Array} bytes the packed model */
  static async create(wasm, bytes, { tokens = TOKENS } = {}) {
    if (!globalThis.navigator?.gpu) throw new Error("this browser has no WebGPU");
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
    if (!adapter) throw new Error("no WebGPU adapter");
    const device = await adapter.requestDevice();
    const m = readModel(bytes);
    const H = m.hidden, I = m.intermediate, T = tokens, D = H / m.heads;
    if (H % 128 || I % 64 || T % 16 || T > 256 || (T & (T - 1)) || D > T || T > m.positions) {
      throw new Error(`these dimensions do not fit the shaders (hidden ${H}, tokens ${T})`);
    }

    // The tokenizer, in WASM: only the start of the file.
    const { instance } = await WebAssembly.instantiate(wasm, {});
    const x = instance.exports;
    const at = x.alloc(m.tokenizerEnd);
    new Uint8Array(x.memory.buffer, at, m.tokenizerEnd).set(bytes.subarray(0, m.tokenizerEnd));
    if (x.load_tokenizer(at, m.tokenizerEnd) !== 0) throw new Error("no tokenizer in the model file");
    x.free(at, m.tokenizerEnd);

    // Weights: codes and floats, each into one buffer.
    const codeParts = [], paramParts = [];
    let codeWords = 0, paramCount = 0;
    const addCodes = (u8) => {
      const padded = new Uint8Array(Math.ceil(u8.length / 4) * 4);
      padded.set(u8);
      codeParts.push(padded);
      return (codeWords += padded.length / 4) - padded.length / 4;
    };
    const addParams = (f32) => {
      paramParts.push(f32);
      return (paramCount += f32.length) - f32.length;
    };
    const q4 = (w, k, n) => {
      if (w.kind !== "q4" || w.block !== 32 || w.cols !== k || w.rows !== n) throw new Error("a matrix the shaders cannot read");
      return { codes: addCodes(w.codes), scales: addParams(w.scales), levels: addParams(w.levels) };
    };
    if (m.words.kind !== "q4" || m.position.kind !== "q8") throw new Error("embeddings the shaders cannot read");

    // Uniform arguments, one 256-byte slot per dispatch.
    const args = [];
    const arg = (values) => (args.push(values), (args.length - 1) * 256);

    const embedArgs = arg([addCodes(m.words.codes), addParams(m.words.scales), addParams(m.words.levels), m.words.block,
      addCodes(m.position.codes), addParams(m.position.scales), m.position.block, addParams(m.norm.weight), addParams(m.norm.bias)]);
    const layers = m.layers.map((l) => {
      const mat = (lin, k, n) => {
        const w = q4(lin.weight, k, n);
        return arg([k, n, w.codes, w.scales, w.levels, addParams(lin.bias)]);
      };
      const norm = (n) => arg([addParams(n.weight), addParams(n.bias)]);
      return {
        query: mat(l.query, H, H), key: mat(l.key, H, H), value: mat(l.value, H, H), output: mat(l.output, H, H),
        attentionNorm: norm(l.attentionNorm), intermediate: mat(l.intermediate, H, I), out: mat(l.out, I, H),
        outputNorm: norm(l.outputNorm),
      };
    });

    const storage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST;
    const upload = (data, usage) => {
      const buffer = device.createBuffer({ size: Math.max(16, Math.ceil(data.byteLength / 4) * 4), usage });
      device.queue.writeBuffer(buffer, 0, data);
      return buffer;
    };
    const concat = (parts, Type, length) => {
      const out = new Type(length);
      let o = 0;
      for (const p of parts) out.set(p, o), (o += p.length);
      return out;
    };
    const codes = upload(concat(codeParts.map((p) => new Uint32Array(p.buffer)), Uint32Array, codeWords), storage);
    const params = upload(concat(paramParts, Float32Array, paramCount), storage);
    const argData = new Uint32Array(args.length * 64);
    args.forEach((values, i) => argData.set(values, i * 64));
    const argBuffer = upload(argData, GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST);
    const meta = device.createBuffer({ size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
    const ids = device.createBuffer({ size: 4 * T, usage: storage });
    const activation = (width) => device.createBuffer({ size: 4 * T * width, usage: GPUBufferUsage.STORAGE });
    const [xb, qb, kb, vb, ctx, att] = [H, H, H, H, H, H].map(activation);
    const inner = activation(I);
    const out = device.createBuffer({ size: 4 * H, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC });
    const read = device.createBuffer({ size: 4 * H, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });

    // Pipelines and the dispatches of one query, with their bind groups.
    const source = shaders({ H, T, D });
    device.pushErrorScope("validation");
    const pipeline = async (code) => {
      const module = device.createShaderModule({ code });
      const errors = (await module.getCompilationInfo()).messages.filter((m) => m.type === "error");
      if (errors.length) throw new Error(`WGSL: ${errors.map((e) => `${e.lineNum}:${e.linePos} ${e.message}`).join("; ")}`);
      return device.createComputePipelineAsync({ layout: "auto", compute: { module, entryPoint: "main" } });
    };
    const [pEmbed, pLinear, pGelu, pResidual, pNorm, pAttention, pPool] = await Promise.all([
      pipeline(source.embed), pipeline(source.matmul(false, false)), pipeline(source.matmul(true, false)),
      pipeline(source.matmul(false, true)), pipeline(source.norm), pipeline(source.attention), pipeline(source.pool),
    ]);
    const bind = (p, entries) => device.createBindGroup({
      layout: p.getBindGroupLayout(0),
      entries: entries.map(([binding, buffer, offset]) => ({ binding, resource: offset === undefined ? { buffer } : { buffer, offset, size: 64 } })),
    });
    const dispatches = [[pEmbed, bind(pEmbed, [[0, argBuffer, embedArgs], [1, ids], [2, codes], [3, params], [4, xb]]), T, 1]];
    const linear = (p, slot, input, output, residual) => [p, bind(p, [[0, argBuffer, slot], [1, meta], [2, codes], [3, params], [4, input],
      ...(residual ? [[5, residual]] : []), [6, output]]), args[slot / 256][1] / 64, T / 16];
    const norm = (slot, buffer) => [pNorm, bind(pNorm, [[0, argBuffer, slot], [1, meta], [2, params], [3, buffer]]), T, 1];
    for (const l of layers) {
      dispatches.push(
        linear(pLinear, l.query, xb, qb), linear(pLinear, l.key, xb, kb), linear(pLinear, l.value, xb, vb),
        [pAttention, bind(pAttention, [[0, meta], [1, qb], [2, kb], [3, vb], [4, ctx]]), T, m.heads],
        linear(pResidual, l.output, ctx, att, xb), norm(l.attentionNorm, att),
        linear(pGelu, l.intermediate, att, inner), linear(pResidual, l.out, inner, xb, att), norm(l.outputNorm, xb),
      );
    }
    dispatches.push([pPool, bind(pPool, [[0, meta], [1, xb], [2, out]]), 1, 1]);
    const error = await device.popErrorScope();
    if (error) throw new Error(`WebGPU: ${error.message}`);

    const e5 = new E5Gpu({ device, x, dims: H, tokens: T, ids, meta, out, read, dispatches });
    await e5.embed("query: warm-up"); // compiles and runs every pipeline once
    return e5;
  }

  constructor(state) {
    Object.assign(this, state);
    this.encoder = new TextEncoder();
    this.queue = Promise.resolve();
    this.lost = null;
    this.device.lost.then((info) => (this.lost = info.message || "the GPU device was lost"));
    this.room = this.x.alloc(4 * 512);
  }

  /** The token ids of `text` (prefix included), cut to the GPU's tokens. */
  tokenize(text) {
    const utf8 = this.encoder.encode(text.normalize("NFC"));
    const at = this.x.alloc(utf8.length);
    new Uint8Array(this.x.memory.buffer, at, utf8.length).set(utf8);
    const count = this.x.tokenize(at, utf8.length, this.room, 512);
    this.x.free(at, utf8.length);
    if (count < 0) throw new Error("tokenizing failed");
    const ids = Array.from(new Uint32Array(this.x.memory.buffer, this.room, Math.min(count, 512)));
    if (ids.length > this.tokens) ids.splice(this.tokens - 1, Infinity, EOS);
    return ids;
  }

  /** The embedding of `text` (prefix included: "query: …"), and how many tokens it was. */
  embed(text) {
    const run = this.queue.then(() => this.#run(this.tokenize(text)));
    this.queue = run.catch(() => {});
    return run;
  }

  async #run(tokenIds) {
    if (this.lost) throw new Error(this.lost);
    const { device } = this;
    const padded = new Uint32Array(this.tokens).fill(PAD);
    padded.set(tokenIds);
    device.queue.writeBuffer(this.ids, 0, padded);
    device.queue.writeBuffer(this.meta, 0, new Uint32Array([tokenIds.length, 0, 0, 0]));
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginComputePass();
    for (const [pipeline, group, gx, gy] of this.dispatches) {
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, group);
      pass.dispatchWorkgroups(gx, gy);
    }
    pass.end();
    encoder.copyBufferToBuffer(this.out, 0, this.read, 0, 4 * this.dims);
    device.queue.submit([encoder.finish()]);
    await this.read.mapAsync(GPUMapMode.READ);
    const vector = new Float32Array(this.read.getMappedRange().slice(0));
    this.read.unmap();
    return { vector, tokens: tokenIds.length };
  }

  destroy() {
    this.device.destroy();
  }
}
