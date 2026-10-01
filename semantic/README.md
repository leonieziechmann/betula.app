# folia-semantic — semantic search

Finds modules by what they are about, not by the words of their title: „coding lernen“ →
„Einführung in die Programmierung“, „wie baue ich eine brücke“ → „Brückenbau“, „renewable
energy“ → „Erneuerbare Energien“. German and English, also across the two.

The model is `intfloat/multilingual-e5-small` (MIT), made small in `poc/semantic-search/`: the
vocabulary cut from 250,002 pieces to the 27,625 German, English and the catalog's texts need,
the weights quantised. That directory has the measurements (quality against the original on
public benchmarks and on the catalog, speed, size); this crate is what came of it. WebGPU was
tried there too and was no faster than WASM on a phone, so the browser runs WASM.

No dependencies. The same code runs on the server and, as WASM in a Web Worker, in the browser.

**The server and the browser give the same results for the same input, to the bit:** the same
hits in the same order with the same scores, for the same query, `k`, model file and index file.
`semantic::Search` is that search on both sides (the browser's 4-bit model in `Mode::Int8`).
Its arithmetic is defined bit for bit and computed so by every build: integer sums where they
are exact whatever the order, floats added in one fixed order (`tensor::tile_scalar`, which
the SIMD and relaxed-SIMD kernels reproduce), an `exp` of its own instead of the platform's,
and no Unicode normalising on one side only (the worker hands the query over as typed).
`js/parity.mjs` checks it (below).

## How the parts fit

```
snapshot ──► server: Index::build(model_server, module_text(…) for each module)  ──► index.bin
                     (once per snapshot, in the background)                          1.9 MB

browser: Web Worker ── Search { model_browser, index }.search(query, k) ──► [{id, score}]
         (js/worker.js, semantic.*.wasm; the page talks to it through js/semantic.js)
server:  the same Search, natively (for pages rendered on the server, an API) ──► the same hits
```

E5 embeds what is searched for and what is searched differently: `embed_query` and
`embed_passage` add the prefixes it was trained with. Documents are embedded once, on the server;
a browser embeds only the query.

| | file | positions | size | used for |
|---|---|---|---|---|
| browser model | 4-bit GPTQ, embeddings 4 bit | 128 | 18.5 MB (16.4 brotli) | queries, in the Web Worker |
| server model | 8 bit | 512 | 34.6 MB | the modules' texts (several hundred tokens), only for building the index |
| index | int8, 384 values + scale + id a module | — | 1.9 MB for 4,938 modules | what a query is compared with |

Both models come out of `poc/semantic-search/python/pack.py` with the same vocabulary, so a
browser's queries and the server's documents are embedded by the same model up to rounding.

```bash
cd poc/semantic-search/python     # setup: poc/semantic-search/README.md, "Run it"
python embed_catalog.py catalog.db --export-text ../model/catalog.txt
python build_vocab.py --coverage 0.99 --text ../model/catalog.txt --out ../model/vocab.json
python pack.py --vocab ../model/vocab.json --weights gptq-q4 --embeddings q4 --out ../model/e5-de-en.bin                  # browser
python pack.py --vocab ../model/vocab.json --weights q8 --embeddings q8 --positions 512 --out ../model/e5-de-en-server.bin  # server
```

The vocabulary keeps every piece the catalog's texts of that day need; a module added later is
still cut into pieces the model knows, just not always into the original's.

## On the server

```rust
// Once per snapshot, in the background: the index, with the server model.
let model = semantic::Model::from_bytes_with(std::fs::read(server_model)?, semantic::Mode::F32)?;
let documents: Vec<(String, String)> = modules.map(|m| (m.id, semantic::module_text(&m.title_de, m.title_en.as_deref(), m.contents.as_deref(), m.learning_outcomes.as_deref()))).collect();
let index = semantic::Index::build(&model, &documents, threads)?.to_bytes()?;   // served to browsers as it is

// Searching: the browser's model and these index bytes, as the worker does.
let search = semantic::Search::new(std::fs::read(browser_model)?, &index)?;
let hits = search.search("coding lernen", 20);
```

Only the index is built with the server model; its bytes are what both sides search, so how
they were computed does not matter for the equality. Queries must go through `Search` with the
browser's model: another model or `Model` in another mode embeds them differently.

`examples/index.rs` does this for a snapshot file:
`cargo run -p folia-semantic --release --example index -- e5-de-en-server.bin catalog.db index.bin`.

**Cost:** a module's text runs to 512 tokens, about 1.2 s of one processor (f32 mode; int8 is
slower natively, it is built for WASM SIMD): **INDEX_TIME** for the catalog on 4 threads.
That is a background job per snapshot, like the brotli copy of the snapshot (`server/src/snapshot.rs`). Most modules do not change from one
snapshot to the next, so keeping each module's embedding under a hash of its text would leave a
few seconds per snapshot (not built yet). `Mode::F32` holds the server model as floats: 102 MB.

## In the browser

`scripts/build-semantic.sh` builds the Web Worker into `site/pkg/`: `semantic.simd.wasm` and
`semantic.relaxed.wasm` (relaxed SIMD, Chrome/Edge/Firefox; the worker picks one by feature
test), `semantic-worker.js` and `semantic.js`. A page:

```js
import { Semantic } from "/pkg/semantic.js";
const semantic = new Semantic({ model: "/api/semantic/model", index: "/api/semantic/index" });
await semantic.ready;                        // model and index loaded in the worker
const found = await semantic.search("coding lernen", 20);
if (found) for (const { id, score } of found.hits) { /* id: the module's id */ }
```

Everything happens in the worker: loading (18.5 MB, then 21 MB of int8 weights made from it),
the query (15–55 ms on a laptop, the main thread untouched: its longest pause during searches
was 9 ms in the test below), the search over the index (a few ms). Typing fast does not pile up
work: while a query runs only the newest waits, the ones it replaced resolve to `null`. The
worker's memory is 47 MiB (the WASM memory with model, int8 weights and index).

**Why a Web Worker, not the service worker:** the service worker is the right place to *keep*
the model and the index (Cache Storage, so the search works offline like the rest of the app,
`app/assets/sw.js`), not to *compute*: a browser stops an idle service worker after some
seconds (Chrome: 30), and each start would load the model again (0.25 s on a laptop, more on a phone, and 47 MiB). A
dedicated worker lives as long as the page and keeps the model loaded between queries.

## Not done yet

- the server's routes for the model and the index (`/api/semantic/…`, names above are
  placeholders), building the index when a snapshot activates, and where the model files come
  from (35 + 18.5 MB: not in git; a release artifact, or built in the deploy);
- the search in the app's UI (`app/`), next to the existing text search, which stays the
  answer for titles, numbers and abbreviations;
- `sw.js` keeping model and index;
- Unicode composition (NFC): neither side composes „e“ + U+0301 into „é“ (a query typed so is
  cut differently from one with „é“, on both sides alike). Keyboards and the catalog write the
  composed form; composing would have to happen in Rust, for both.

## Checks

- `cargo test -p folia-semantic`; with `SEMANTIC_TEST_MODEL=path/to/e5-de-en.bin` also the
  model (the repository has none).
- `poc/semantic-search/python/parity.py`: the tokenizer and the encoder against Hugging Face
  (identical token ids for 617 texts, cosine ≥ 0.9999998).
- `js/parity.mjs`: the server's and the browser's search to the bit — the native
  `target/release/embed MODEL --search INDEX < queries` against the WASM builds over the same
  queries (PARITY_RESULT).
