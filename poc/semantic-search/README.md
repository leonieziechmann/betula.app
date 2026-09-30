# Semantic search on the client — proof of concept

Not part of the app. The question: can a browser (a phone included) embed a search query with
`intfloat/multilingual-e5-small` itself, so that modules are found by meaning („coding lernen“ →
„Einführung in die Programmierung“), with everything it has to download under 20 MB?

**Yes: 18.1 MB (16.0 MB in brotli) + a 77 kB WASM runtime, 30–80 ms a query on a laptop,
150–470 ms with the CPU slowed down 4–6× (phones), at about the quality of the original model.**

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
   encoder, expanding each 4-bit row when it is used. Native and `wasm32` with `simd128`;
   `demo/e5.js` is the JavaScript side (no wasm-bindgen).

| Part | Bytes |
|---|---|
| 12 layers (72 matrices, 21.2 M weights, GPTQ 4 bit) | 11.95 MB |
| word embeddings (25,748 × 384, 4 bit) | 5.56 MB |
| tokenizer (pieces, scores, character table) | 0.32 MB |
| biases, LayerNorms (fp32), 128 positions (8 bit) | 0.30 MB |
| **model file** | **18.12 MB** (brotli 16.04 MB) |
| runtime `e5_mini.wasm` | 77 kB (brotli 25 kB) |
| index of the catalog (int8, 384 B a module) | ≈ 1.9 MB for 5,000 modules |

The documents are embedded once with the original model (`python/embed_catalog.py`: the server
or a build step would do that); the browser embeds only the query.

## Quality

Public benchmarks (the documents with the original model, the queries with each variant; MRR@10,
SciFact nDCG@10 as in MTEB). XQuAD is the same 240 paragraphs and 1,190 questions in both
languages, so it also measures German questions for English text and the other way round.

| Queries embedded by | XQuAD de | XQuAD en | en→de | de→en | GermanDPR | SciFact |
|---|---|---|---|---|---|---|
| original (250k pieces, fp32, 470 MB) | 94.9 | 96.7 | 90.6 | 90.1 | 73.3 | 68.1 |
| 25.7k pieces, fp32 | 94.4 | 96.6 | 90.3 | 89.7 | 72.3 | 66.6 |
| 32k pieces, fp32 | 94.5 | 96.6 | 90.4 | 90.0 | 72.8 | 67.1 |
| 25.7k pieces, 8 bit | 94.4 | 96.6 | 90.2 | 89.8 | 72.3 | 66.6 |
| 25.7k pieces, 4 bit, rounded | 93.5 | 96.0 | 89.7 | 89.3 | 71.6 | 64.7 |
| **25.7k pieces, 4 bit, GPTQ (the file)** | **94.1** | **96.7** | **90.5** | **89.6** | **72.5** | **66.4** |
| 25.7k pieces, 4 bit GPTQ, blocks of 64 (−0.7 MB) | 93.7 | 96.6 | 90.3 | 89.4 | 72.6 | 66.2 |

What is left of the loss is the trimming, and it falls on rare words: SciFact's claims are full
of scientific terms (−1.7). For Betula that is the one part to fix: `build_vocab.py --text`
keeps every piece the catalog's own texts need (`embed_catalog.py --export-text`), so a word
that stands in a module is always cut as the original model cuts it. **Not measured: the real
catalog** — reading the snapshot was not permitted in the session that built this, so the
evaluation is public data, and the demo runs on 30 made-up modules (`python/sample_catalog.py`).
On those, 8 of 10 test queries rank the intended module first:

| Query | First hit |
|---|---|
| coding lernen | Einführung in die Programmierung |
| wie berechne ich ob eine brücke hält | Stahlbetonbau (Technische Mechanik – Statik third) |
| solar power | Photovoltaik und Solarthermie |
| Hackerangriffe verhindern | IT-Sicherheit |
| wie schreibe ich meine bachelorarbeit | Wissenschaftliches Schreiben |
| KI | *nothing sensible*: an abbreviation of two letters carries too little |

Short abbreviations and exact titles or numbers stay the job of the existing text search
(`catalog::search`, `catalog::fuzzy`); the semantic one is for what a word match cannot find.

## Speed

The same file and WASM; the median of several runs; tokens including „query:“, `<s>`, `</s>`.

| | 7 tokens | 9 | 15 | 20 |
|---|---|---|---|---|
| Chromium, laptop CPU (4 cores, one used) | 32 ms | 37 ms | 53 ms | 76 ms |
| Chromium, CPU 4× slower | 144 ms | 180 ms | 245 ms | 313 ms |
| Chromium, CPU 6× slower | 197 ms | 233 ms | 335 ms | 466 ms |

Loading (fetching the file from a local server, instantiating, parsing): 0.2 s, 0.5 s and 0.7 s; the WASM memory is 21.5 MiB
(the file plus room for one query). Searching 5,000 int8 rows takes a few milliseconds in
plain JavaScript. The Rust output equals the Python model's: identical token ids for 617 test
texts (umlauts, ligatures, control characters, emoji, CJK, extra white space …), cosine
≥ 0.9999998 (`python/parity.py`, `demo/bench.mjs --check`).

Room left if it has to be faster: the weights expanded to f32 once (85 MB of memory, no
expanding per query), int8 activations with integer SIMD, a Web Worker so typing never waits.

## Run it

```bash
python -m venv .venv && . .venv/bin/activate
pip install torch --index-url https://download.pytorch.org/whl/cpu
pip install -r python/requirements.txt
cd python
python build_vocab.py --coverage 0.99 --out ../model/vocab.json            # ~1 min
#   for Betula: python embed_catalog.py catalog.db --export-text ../model/catalog.txt
#               and add --text ../model/catalog.txt
python pack.py --vocab ../model/vocab.json --weights gptq-q4 --embeddings q4 --out ../model/e5-de-en.bin   # ~3 min
python sample_catalog.py ../model/sample.db && python embed_catalog.py ../model/sample.db --out ../model   # or a real catalog.db
python evaluate.py --vocab ../model/vocab.json --variants gptq-q4/q4       # ~20 min the first time

cd ../runtime
cargo test --release && cargo build --release                               # native: target/release/embed
RUSTFLAGS="-C target-feature=+simd128" cargo build --release --lib --target wasm32-unknown-unknown
cd ../python && python parity.py ../model/e5-de-en.bin

cd .. && cp demo/index.html demo/e5.js runtime/target/wasm32-unknown-unknown/release/e5_mini.wasm model/
cd model && python -m http.server 8765                                     # http://127.0.0.1:8765
```

`model/` holds what is built and stays out of git (18 MB of weights).

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
| `demo/` | `index.html` (search as you type), `e5.js`, `bench.mjs` (Node), `browser-bench.mjs` (Chromium, throttled) |

The weights are derived from `intfloat/multilingual-e5-small` (MIT licence).
