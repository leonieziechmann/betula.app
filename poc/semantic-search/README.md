# Semantic search on the client — proof of concept

Not part of the app. The question: can a browser (a phone included) embed a search query with
`intfloat/multilingual-e5-small` itself, so that modules are found by meaning („coding lernen“ →
„Einführung in die Programmierung“), with everything it has to download under 20 MB?

**Yes: 18.5 MB (16.4 MB in brotli) + a 90 kB WASM runtime, 14–55 ms a query on a laptop's CPU,
at about the quality of the original model — measured on the real module catalog as well.
A WebGPU encoder computes the same embeddings; its speed on real GPUs is still to be measured.**

## How

The original is 118 M parameters, 470 MB in fp32. 96 M of them are the embedding matrix: one
row for each of XLM-R's 250,002 pieces, for 100 languages.

1. **Only the pieces German and English need** (`python/build_vocab.py`). The frequent words
   of both languages (wordfreq: 634k German, 321k English words, in lower case, capitalised,
   in capitals, after a hyphen or bracket) are cut the way the original tokenizer cuts them; the
   pieces are ranked by the frequency of the words that use them. A unigram tokenizer left with
   a subset of its pieces cuts a word exactly as before as long as the pieces of that cut are
   still there, so coverage is measurable: **25,748 pieces cut 99 % of German and English text
   (by word frequency) exactly as the full vocabulary does** (`--coverage 0.99`). A word
   with a missing piece is cut into other, smaller pieces — still a meaningful input, just not
   the one the model saw in training. 32k pieces cost 1.4 MB more and gain at most half a point;
   ranking rare words higher (`--alpha 0.5`, 28k pieces for the same 99 %) gains nothing.
   **Plus every piece the catalog's texts need** (`--text`, from `embed_catalog.py
   --export-text`): 27,625 pieces, 0.4 MB more, and every word that stands in a module is cut
   exactly as the original model cuts it.
2. **4 bits per weight** (`python/common.py`, `python/gptq.py`): blocks of 32 weights share
   an fp16 scale (4.5 bits a weight). Rounding to the nearest level costs up to 2 points;
   **GPTQ** — quantising the input dimensions one after the other and moving each one's error
   onto those not yet quantised, weighted by calibration text (STS-B, SQuAD-train questions,
   news, keyword queries; none of it from the evaluation) — makes up for it completely: 4 bits
   end up as good as 8. The format and the runtime stay the same.
3. **One file** (`python/pack.py`, layout in its docstring): pieces and scores, the
   normaliser's character table, 128 positions, the weights.
4. **A runtime in Rust** (`runtime/`, no dependencies, the clippy rules of the app's crates):
   the tokenizer (Viterbi over the pieces, as Hugging Face's `tokenizers` does it) and the BERT
   encoder. Native and `wasm32`, in two builds: `simd128`, and `simd128` + `relaxed-simd`
   (Chrome, Edge, Firefox; `demo/e5.js` picks one by feature detection, Safari gets the first).
   `demo/e5.js` is the JavaScript side (no wasm-bindgen). Three ways to keep the matrices
   (`Mode`, `runtime/src/tensor.rs`):
   - **expand**: the 4-bit rows as stored, each expanded to f32 when it is used (21.5 MiB);
   - **f32**: expanded once at load (102 MiB) — hardly faster, WASM SIMD has no FMA and the
     multiply-adds, not the expanding, are the work;
   - **int8** (the default): expanded once to one byte a weight (43 MiB); each token's input
     is quantised to int8 per block of 32, the products are integer dot products (as
     llama.cpp does it), 4 weight rows × 2 tokens at a time so that each load is used 8 times.
     With relaxed SIMD one instruction does 16 multiply-adds (`i32x4.relaxed_dot_i8x16_i7x16_add`);
     its second operand must not have the top bit set (x86 reads it unsigned), so the weights
     are kept as 0…15 and 8 × the block's input sum is subtracted afterwards.
   The GELU uses a rational approximation of erf (Eigen's, no `exp`).
5. **WebGPU** (`demo/e5-gpu.js`), with the WASM runtime as the cold fallback. `createEmbedder`
   runs once at app start: it uploads the weights as the file has them (4-bit codes and their
   scales; the shaders expand them, 16 MB of GPU memory), creates every buffer, pipeline and bind
   group for a fixed 64 tokens, and runs one query so the shaders are compiled. A query is then
   the tokenizer (WASM — only the tokenizer is loaded, `load_tokenizer` / `tokenize`), two small
   buffer writes, one command buffer of 110 dispatches (per layer: 3 projections, attention,
   output + residual, LayerNorm, feed-forward with GELU, output + residual, LayerNorm) and the
   read-back of 384 floats. The matrix kernel makes 16 tokens × 64 outputs a workgroup, expanding
   each 64 × 32 block of weights into shared memory once for all 16; tiles past the query's
   tokens return at once, so a 10-token query computes one tile row, not four. Queries longer
   than 64 tokens are cut off (`</s>` at the end). Without WebGPU, or when the GPU fails later (device
   lost, an error), the same `embed` goes to the WASM runtime, which only then loads the model.

| Part | Bytes |
|---|---|
| 12 layers (72 matrices, 21.2 M weights, GPTQ 4 bit) | 11.95 MB |
| word embeddings (27,625 × 384, 4 bit) | 5.97 MB |
| tokenizer (pieces, scores, character table) | 0.33 MB |
| biases, LayerNorms (fp32), 128 positions (8 bit) | 0.30 MB |
| **model file** | **18.55 MB** (brotli 16.42 MB) |
| runtime `e5_mini.wasm` | 90 kB (simd) / 93 kB (relaxed) |
| WebGPU encoder `e5-gpu.js` | 23 kB |
| index of the catalog (int8, 384 B a module) | 1.9 MB for 4,938 modules |

The documents are embedded once with the original model (`python/embed_catalog.py`: the server
or a build step would do that); the browser embeds only the query.

## Quality

Public benchmarks and the real catalog (the documents with the original model, the queries with
each variant; MRR@10, SciFact nDCG@10 as in MTEB). XQuAD is the same 240 paragraphs and 1,190
questions in both languages, so it also measures German questions for English text and the
other way round. **Betula** is the catalog of https://betula.app/api/db: 1,000 modules each,
queried by their German or English title, the corpus their contents and learning outcomes
(titles left out; descriptions of at least 200 characters) — hard, since a title often says
little, but the same for every variant.

| Queries embedded by | XQuAD de | XQuAD en | en→de | de→en | GermanDPR | SciFact | Betula de | Betula en |
|---|---|---|---|---|---|---|---|---|
| original (250k pieces, fp32, 470 MB) | 94.9 | 96.7 | 90.6 | 90.1 | 73.3 | 68.1 | 37.1 | 19.3 |
| 25.7k pieces, fp32 | 94.4 | 96.6 | 90.3 | 89.7 | 72.3 | 66.6 | | |
| 32k pieces, fp32 | 94.5 | 96.6 | 90.4 | 90.0 | 72.8 | 67.1 | | |
| 25.7k pieces, 8 bit | 94.4 | 96.6 | 90.2 | 89.8 | 72.3 | 66.6 | | |
| 25.7k pieces, 4 bit, rounded | 93.5 | 96.0 | 89.7 | 89.3 | 71.6 | 64.7 | | |
| 25.7k pieces, 4 bit, GPTQ | 94.1 | 96.7 | 90.5 | 89.6 | 72.5 | 66.4 | | |
| 25.7k pieces, 4 bit GPTQ, blocks of 64 (−0.7 MB) | 93.7 | 96.6 | 90.3 | 89.4 | 72.6 | 66.2 | | |
| 27.6k (+ catalog) pieces, 4 bit GPTQ, Rust expand | 94.3 | 96.5 | 90.5 | 89.6 | 72.2 | 67.2 | 36.9 | 18.1 |
| **27.6k (+ catalog) pieces, 4 bit GPTQ, Rust int8 (the demo)** | **94.2** | **96.5** | **90.4** | **89.5** | **72.3** | **67.1** | **36.9** | **18.0** |

The Rust rows are the WASM runtime's arithmetic run natively (`evaluate.py --variants
rust:expand rust:int8`); int8 activations cost nothing measurable (cosine to expand ≥ 0.9999).
What is left of the loss is the 4 bits and the trimming, which falls on rare words: SciFact's
claims are full of scientific terms. With the catalog's pieces kept, the Betula queries are
tokenised exactly as by the original; what they lose (−0.2 German, −1.3 English) is the 4 bits.

Not measured: whether it finds what students search for better than the existing text search.
Short abbreviations and exact titles or numbers stay the job of `catalog::search` /
`catalog::fuzzy`; the semantic one is for what a word match cannot find. Some queries on the
real catalog (first hits):

| Query | First hits |
|---|---|
| wie baue ich eine brücke | Brückenbau |
| renewable energy | Renewable Resources Management, Erneuerbare Energien |
| Datenbanken | Datenbanken |
| coding lernen | Grundlagen und Verfahren zur Datencodierung, Einführung in die Programmierung |

## Speed

The same file; median of 20 runs in Node 22 (V8, as Chrome), one core of a laptop CPU; tokens
including „query:“, `<s>`, `</s>`.

| Mode | 7 tokens | 9 | 13 | 23 | 25 | memory |
|---|---|---|---|---|---|---|
| expand | 29 ms | 34 ms | 43 ms | 68 ms | 73 ms | 21.5 MiB |
| f32 | 22 ms | | 42 ms | 70 ms | | 102 MiB |
| int8, SIMD (Safari) | 14 ms | 23 ms | 31 ms | 54 ms | 59 ms | 43 MiB |
| **int8, relaxed SIMD** | **13.5 ms** | **20 ms** | **27 ms** | **39 ms** | **53 ms** | 43 MiB |

Measured earlier for expand in Chromium with the CPU slowed down 4× / 6× (a stand-in for
phones): 144–313 ms / 197–466 ms, i.e. about 4.5–6× the laptop's time; int8 should scale the
same way. Comparing a query with the 4,938 modules takes 4–10 ms in plain JavaScript. The Rust
output equals the Python model's: identical token ids for 617 test texts (umlauts, ligatures,
control characters, emoji, CJK, extra white space …), cosine ≥ 0.9999998 in expand mode
(`python/parity.py`, `demo/bench.mjs --check`); WASM equals native.

**WebGPU:** the arithmetic is the WASM runtime's expand mode in f32, and the embeddings agree
with it to float rounding (cosine ≥ 0.9999997 for 51 queries, catalog titles and awkward input,
`demo/gpu-check.mjs`). Its speed is **not measured**: the machine this was built on has no GPU,
and Chromium's software WebGPU (SwiftShader, on the CPU) takes about 2 s a query, which says
nothing about a GPU. The work is small for one (≈ 0.7 GFLOP for a query of up to 16 tokens, 16 MB of
weights read once); what is left is the fixed cost of 110 dispatches and a read-back, typically
a few milliseconds. To be measured on real laptops and phones, with the demo (it shows the
backend and the time per query; `?backend=wasm` for the comparison).

Room left: a Web Worker so typing never waits; a phone-sized benchmark on real devices.

## Demo

`demo/index.html` searches as you type. The same page with the real catalog was published as a
claude.ai artifact (model and index as base64 text there, 27 MB to download, since that host
serves no binary files).

## Run it

```bash
python -m venv .venv && . .venv/bin/activate
pip install torch --index-url https://download.pytorch.org/whl/cpu
pip install -r python/requirements.txt
cd python
curl -o ../model/catalog.db https://betula.app/api/db
python embed_catalog.py ../model/catalog.db --export-text ../model/catalog.txt
python build_vocab.py --coverage 0.99 --text ../model/catalog.txt --out ../model/vocab.json   # ~1 min
python pack.py --vocab ../model/vocab.json --weights gptq-q4 --embeddings q4 --out ../model/e5-de-en.bin   # ~3 min
python embed_catalog.py ../model/catalog.db --out ../model                  # index.bin, index.json
#   no catalog at hand: python sample_catalog.py ../model/sample.db (30 made-up modules)

cd ../runtime
cargo test --release && cargo build --release                               # native: target/release/embed
CARGO_TARGET_DIR=target/simd RUSTFLAGS="-C target-feature=+simd128" \
  cargo build --release --lib --target wasm32-unknown-unknown
CARGO_TARGET_DIR=target/relaxed RUSTFLAGS="-C target-feature=+simd128,+relaxed-simd" \
  cargo build --release --lib --target wasm32-unknown-unknown
cd ../python
python parity.py ../model/e5-de-en.bin
python evaluate.py --vocab ../model/vocab.json --catalog ../model/catalog.db \
  --packed ../model/e5-de-en.bin --embed ../runtime/target/release/embed \
  --variants gptq-q4/q4 rust:int8                                           # ~20 min the first time

cd ..
cp demo/index.html demo/e5.js demo/e5-gpu.js model/
cp runtime/target/simd/wasm32-unknown-unknown/release/e5_mini.wasm model/e5_mini.simd.wasm
cp runtime/target/relaxed/wasm32-unknown-unknown/release/e5_mini.wasm model/e5_mini.relaxed.wasm
node demo/bench.mjs model/e5_mini.relaxed.wasm model/e5-de-en.bin --mode int8
node demo/gpu-check.mjs http://127.0.0.1:8765/                             # with the server below running; needs playwright-core
cd model && python -m http.server 8765                                     # http://127.0.0.1:8765
```

`model/` holds what is built and stays out of git (18 MB of weights, the catalog).

## Files

| | |
|---|---|
| `python/build_vocab.py` | the pieces to keep |
| `python/common.py` | tokenizers, quantisation (4 bit with a searched scale, NF4, 8 bit), the model with the quantised weights put back in |
| `python/gptq.py` | GPTQ and its calibration text |
| `python/pack.py` | the model file |
| `python/evaluate.py` | the table above |
| `python/parity.py` | Rust against Python |
| `python/embed_catalog.py`, `python/sample_catalog.py` | the index of a catalog; a made-up one |
| `runtime/` | tokenizer and encoder in Rust, `src/bin/embed.rs` a command line, `src/wasm.rs` the exports |
| `demo/` | `index.html` (search as you type), `e5.js` (WASM), `e5-gpu.js` (WebGPU, falls back to WASM), `bench.mjs` (Node), `browser-bench.mjs` (Chromium, throttled), `gpu-check.mjs` (WebGPU against WASM) |

The weights are derived from `intfloat/multilingual-e5-small` (MIT licence).
