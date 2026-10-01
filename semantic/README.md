# folia-semantic — semantic search

Finds modules by what they are about, not by the words of their title: „coding lernen“ →
„Einführung in die Programmierung“, „wie baue ich eine brücke“ → „Brückenbau“, „renewable
energy“ → „Erneuerbare Energien“. German and English, also across the two. It is there for what
the exact search does not answer: the catalog's text filter and a fuzzy search over titles,
numbers and abbreviations stay the answer for those, and leave most queries students type with
fewer than three modules (measured below).

The model is `intfloat/multilingual-e5-small` (MIT), made small in `poc/semantic-search/`: the
vocabulary cut from 250,002 pieces to the 27,625 German, English and the catalog's texts need
(12,000 for the browser's model, which reads queries only), the weights quantised. The browser's model is in addition fine-tuned on the queries students type
(below). `poc/semantic-search/` has the measurements and the scripts; this crate is what came of
them. WebGPU was tried there too and was no faster than WASM on a phone, so the browser runs WASM.

No dependencies. The same code runs on the web server, as WASM in a Web Worker in the browser,
and as WASM in Radix (in wazero), which computes the modules' vectors with it.

**The server and the browser give the same results for the same input, to the bit:** the same
hits in the same order with the same scores, for the same query, `k`, model file and vectors.
`semantic::Search` is that search on both sides (the browser's 4-bit model in `Mode::Int8`).
Its arithmetic is defined bit for bit and computed so by every build: integer sums where they
are exact whatever the order, floats added in one fixed order (`tensor::tile_scalar`, which
the SIMD and relaxed-SIMD kernels reproduce), an `exp` of its own instead of the platform's,
and no Unicode normalising on one side only (the worker hands the query over as typed).
`js/parity.mjs` checks it (below). The same holds for the modules' vectors: Radix runs this crate
as WASM in `Mode::Int8`, and its vectors are the bits the crate computes natively.

## How the parts fit

```
Radix (each cycle, after the export; docs/schema-v2.md, „Semantic search“):
  module text ──Gemini──▶ summary (DE, EN, search terms; Radix-internal, never published)
  titles + summary + description ──semantic.wasm in wazero (Model::embed_passage, quantize)──▶ 4-bit vector
  next build ──▶ v_module_vector in the snapshot (schema 11; 4,938 vectors, about 1 MB)

Folia: snapshot ──▶ semantic::Index (Index::push_codes; js/semantic.js indexFromVectors)
  browser: Web Worker ── Search { the fine-tuned query model, index }.search(query, k) ──▶ [{id, score}]
  server:  the same Search, natively ──▶ the same hits
```

E5 embeds what is searched for and what is searched differently: `embed_query` and
`embed_passage` add the prefixes it was trained with. A module's passage (Radix,
`internal/semantic.Passage`) is its titles, then Gemini's German and English summary of it and
search terms, then its contents and learning outcomes: students search with other words than a
description uses, and the summary brings theirs in. Without a Gemini key it is the module's text
alone (`module_text`).

| | file | positions | size | used for |
|---|---|---|---|---|
| browser model | fine-tuned for queries (`finetune.py`), 12,000 pieces, 4-bit GPTQ, embeddings 4 bit | 128 | 15.0 MB (13.4 brotli) | queries, in the Web Worker and on the server |
| server model | the original, 8 bit | 512 | 34.6 MB | the modules' passages, in Radix (`RADIX_EMBED_MODEL`) |
| vectors | 384 values of 4 bits + scale a module, 192 bytes | — | about 1 MB for 4,938 modules | what a query is compared with, in the snapshot |

The query side is fine-tuned, the passage side is not: the modules' vectors stay the original
model's, so the passages need no training data, and the browser's model learns where students'
queries belong among them. Both models come out of `poc/semantic-search/python/pack.py`; the
browser's with a vocabulary of its own, the pieces the queries and the modules' titles need
(`query_text.py`; a piece both vocabularies keep is the same piece, so the two models still read
text alike).

```bash
cd poc/semantic-search/python     # setup: poc/semantic-search/README.md, "Run it"
python embed_catalog.py catalog.db --export-text ../model/catalog.txt
python build_vocab.py --coverage 0.99 --text ../model/catalog.txt --out ../model/vocab.json
python finetune.py catalog.db ../model/ft                                                                                   # query side, 15 min
python query_text.py catalog.db ../model/query-text.txt
python build_vocab.py --size 12000 --text ../model/query-text.txt --out ../model/vocab-query.json
python pack.py --model ../model/ft --vocab ../model/vocab-query.json --weights gptq-q4 --embeddings q4 --out ../model/e5-de-en.bin  # browser
python pack.py --vocab ../model/vocab.json --weights q8 --embeddings q8 --positions 512 --out ../model/e5-de-en-server.bin    # Radix
```

The vocabulary keeps every piece the catalog's texts of that day need; a module added later is
still cut into pieces the model knows, just not always into the original's.

## Quality

Measured on the catalog of 2026-09-30 with `poc/semantic-search/python/evaluate_search.py` and its
data (`poc/semantic-search/data`, written by Claude):

- **open queries**: 344 queries students would type, written without seeing the catalog (200
  realistic ones; 144 by personas: international students in English, first-semester and
  undecided ones, advanced and part-time students), with the modules the variants found graded
  0 (not relevant), 1 (partly), 2 (a good answer): 11,316 judgments. Measured: the share of
  relevant modules among the first 10.
- **known-item**: for 600 sampled modules, 5 queries each for exactly what the module offers
  (keywords; German and English paraphrases without the title's words; a goal; a situation
  without any term of the subject: „mein quadrocopter wackelt ständig …“). Measured: is the module
  among the first 10.

What counts is the queries the exact search does not answer (fewer than 3 modules by the
catalog's LIKE and by every word in the titles): 82 % of the open queries, 95 % of the known-item
ones. On those, measured as deployed — the packed 4-bit query model, Radix's vectors, the search
of this crate (`target/release/embed --search`, the browser's bits):

| | relevant of the first 10 | the module in the first 10 |
|---|---|---|
| before (original model, the description alone) | 56.9 % | 61.8 % |
| fine-tuned query model | 70.7 % | 76.3 % |
| summaries in the passage | 63.9 % | 76.7 % |
| both, the vectors in 8 bits | 72.6 % | 81.7 % |
| both, the vectors in 4 bits | 70.2 % | 80.8 % |
| **both, 4-bit vectors, the query model's vocabulary 12,000 (this crate, Radix)** | **69.2 %** | **80.1 %** |

The vectors are 4 bits a value (`index.rs`): half of the 2 MB 8 bits take in the snapshot, for
2.4 points relevant and 0.9 found. The browser's model keeps 12,000 pieces instead of 27,625:
3.1 MB less to download (13.4 instead of 16.5 MB with brotli; the file 15.0 instead of 18.5) for
1.0 point relevant and 0.7 found. The situation queries, the hardest: 27.7 % → 45.7 % in the first 10. For all queries, including those the exact search answers: 62.1 % → 74.7 % relevant
(the realistic ones), 60.8 % → 71.6 % (the personas'). Of the judgments' holes (modules nobody
graded, counted as not relevant), 1–5 % of the first 10; the numbers are a little low for it.

The same in Python with the original model in f32 (the variants below were measured so): 58.5 %,
71.8 %, 65.5 %, 73.4 % relevant; 62.8 %, 76.0 %, 77.1 %, 82.1 % found. Packing and int8 cost about
a point.

Tried and left out:
- a second vector per module (the summary alone, the higher of the two counts): +1 point, the
  vectors twice the size;
- adding a word match of titles and abbreviations to the score: +0.6 points at best — the exact
  search does that better; of the summaries' search terms +1.5, but those stay in Radix;
- lowering modules that are near every query („hubs“, by their mean similarity to their 10
  nearest training queries): worse (−3 points, with 11 % of its hits not judged). Few modules are
  hubs: the first 10 of 200 queries hold 1,506 different modules, none wrong more than four times;
- the description in chunks of 60 words, the best chunk counting: 61.1 % relevant, 64.7 % found
  (Python; +2–3 points), superseded by the summaries;
- 3-bit vectors: 5.9 points relevant below 4 bits (Python);
- a smaller vocabulary of the query model by word frequency alone, without the pieces of the
  queries and titles: 20,000 pieces 79.7 % found, 14,000 78.3 %, 10,000 75.3 %, 6,000 67.2 %
  (80.8 % with 27,625; 80.0 % with 14,000 and 80.1 % with 12,000 that keep the queries' and
  titles' pieces). The queries' pieces are those of Claude's training queries, which wrote the
  test queries too; the open queries, written without the catalog, lose 1 point all the same.

The vectors make the snapshot larger: 7.5 → 8.3 MB with gzip -9 (packed values hardly
compress; with 8 bits it would be 9.3 MB). That is the index itself, which the browser would
otherwise download on its own.

The fine-tuning (`poc/semantic-search/python/finetune.py`): 21,245 queries of 4,330 modules (five
a module text, written by Claude from the description, three German and two English, keywords to
sentences), the
600 modules of the known-item set left out, so its numbers are for modules the model never saw;
the softmax over all modules of the catalog, two epochs, 15 minutes on four cores. A model trained
on the passages without summaries finds them as well with summaries (and the other way round), so
it does not depend on Gemini.

Not measured: Gemini's summaries. Those above are Claude's, written with the prompt
`internal/gemini` sends; `gemini-3.5-flash-lite` may write them differently. The queries and the
judgments are a language model's too: two independent judgings of 736 pairs agree in 96 %, but
real queries of students are the better test.

## On the server

```rust
// The index of the semantic search, from the snapshot's vectors (computed by Radix).
let mut index = semantic::Index::new(384);
for v in catalog::queries::module_vectors(&db)? {
    index.push_codes(v.module_id, v.scale, &v.vector)?;   // packed, as Radix published it
}
let search = semantic::Search::new(std::fs::read(browser_model)?, &index.to_bytes()?)?;
let hits = search.search("coding lernen", 20);   // the browser's hits, to the bit
```

A query takes the server 14–41 ms (7–25 tokens, one thread, SSE2), as in the browser. Queries
must go through `Search` with the browser's model: another model or `Model` in another mode
embeds them differently.

`Index::build` (and `examples/index.rs`) embeds passages natively, for experiments; the deployed
vectors are Radix's.

## In the browser

`scripts/build-semantic.sh` builds the Web Worker into `site/pkg/`: `semantic.simd.wasm` and
`semantic.relaxed.wasm` (relaxed SIMD, Chrome/Edge/Firefox; the worker picks one by feature
test), `semantic-worker.js` and `semantic.js` — and Radix's copy of the SIMD build,
`internal/embed/semantic.wasm`, committed, so that Radix builds with Go alone (build and commit it
again with every change of `semantic/src`). (`scripts/build-client.sh` runs it, and Nix builds the same as `.#folia-semantic`, part of the
image.) The API of a page:

```js
import { Semantic, indexFromVectors } from "/pkg/semantic.js";
// The vectors are in the local copy of the snapshot: no extra download.
const rows = db.exec("SELECT module_id, scale, vector FROM v_module_vector ORDER BY module_id")[0]?.values ?? [];
const semantic = new Semantic({ model: "/models/e5-de-en-<hash>.bin", index: indexFromVectors(rows) });
await semantic.ready;                        // model and index loaded in the worker
const found = await semantic.search("coding lernen", 20);
if (found) for (const { id, score } of found.hits) { /* id: the module's id */ }
```

Everything happens in the worker: loading (15.0 MB, then the int8 weights made from it; 21 MB with the larger vocabulary),
the query (15–55 ms in Chromium on the container this was built in, the main thread untouched: its longest pause during searches
was 9 ms in the test below), the search over the index (a few ms). Typing fast does not pile up
work: while a query runs only the newest waits, the ones it replaced resolve to `null`. The
worker's memory is 47 MiB (the WASM memory with model, int8 weights and index).

**Why a Web Worker, not the service worker:** the service worker is the right place to *keep*
the model (Cache Storage, so the search works offline like the rest of the app,
`app/assets/sw.js`), not to *compute*: a browser stops an idle service worker after some
seconds (Chrome: 30), and each start would load the model again (0.25–0.3 s here, more on a phone, and 47 MiB). A
dedicated worker lives as long as the page and keeps the model loaded between queries.

## In the app

Everything is in place and loaded; no component uses it yet.

- **Folia** serves the model given by `FOLIA_SEMANTIC_MODEL` (`server/src/semantic.rs`) at
  `/models/e5-de-en-<the first 16 hex digits of its SHA-256>.bin`, kept for good (`immutable`):
  the address changes with the model, not with a build. It is read at the start and kept in
  memory with its brotli form (13.6 MB, made in 1.8 s by the warm-up, `warm::files`, which also
  compresses the worker's files). The server writes the address into `boot.js`
  (`SEMANTIC_MODEL`, `null` without a model) and names it in `/api/status` (`semantic_model`).
- **`boot.js`** starts the search only after `app.start()`, and then in `requestIdleCallback`
  (or 1.5 s later): it imports `/pkg/semantic.js?v=<build>`, reads the vectors from the local
  catalog, builds the index (1 MB; 20–35 ms of the main thread on a laptop) and hands it to the
  worker, which fetches its WASM (with the build's `?v=`) and the model (`priority: "low"`). Not
  at all without a model, without vectors in the snapshot, with data saving on
  (`navigator.connection.saveData`), or on a device with less than 2 GB (`navigator.deviceMemory`).
  Measured in Chromium against the real snapshot: the app runs at 2.3 s, the search is ready
  0.7 s later (the model from the network), 0.16 s on a second visit; under 4× CPU throttling
  the main thread's long tasks after the app's start are the same with and without it.
- **`sw.js`** keeps the model in a cache of its own (`betula-models`), which no build drops: a
  deploy does not download it again, a new model replaces the old one. The worker's other files
  are kept with the shell of their build, as the bundle is.
- **For the app's code:** `window.betulaSemantic` (`ready`, `search(query, k)`; `boot.js` says
  what they answer), and in Rust `app::data::Semantic` in the context of the browser app
  (`SemanticSearch::ready`, `SemanticSearch::search` → `SemanticHit`s), none on the server.

## In a deploy

The two models are not in git (35 + 15 MB). `deploy/models.lock` pins them by their sha256, as a
pair; the server keeps them in its model store, `/var/lib/betula/models`, one file per model named
by its sha256, shared by every instance and every release and mounted read-only into Radix and
Folia (`deploy/README.md` section 13). `deploy/ship-models.sh` uploads what the store lacks, from
`models/` in the repository's root; `deploy/ship.sh` runs it before every deploy, and
`vps/50-app.sh` gives the instance the models only when both are in the store, intact (else it
runs without the semantic search). The browser offers the search only when the snapshot's vectors
are of the passage model its query model was made for (`meta.semantic_model`,
`FOLIA_SEMANTIC_PASSAGE_MODEL`; `docs/schema-v2.md`, „Semantic search“).

## Not done yet

- the search in the app's UI (`app/`): the semantic hits when the exact search finds few;
- a schema-11 snapshot for Folia's tests (`catalog::tests` and the pinned digests);
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
  queries. Result: **6,455 queries** (the 4,938 module titles, 1,500 module descriptions, and
  awkward input: empty, white space only, emoji, CJK, ligatures, a decomposed accent, 1,000
  tokens), each with the bits of its embedding and its 10 hits with the bits of their scores:
  **identical** for the server (x86_64 with SSE2, and the plain Rust code of other platforms),
  WASM SIMD (Safari) and WASM relaxed SIMD (Chrome, Edge, Firefox).
- `cargo test -p folia-semantic`: `tiles_are_the_definition_to_the_bit` checks the build's SIMD
  kernel against the definition (`tile_scalar`) bit for bit.
- Radix's vectors: `RADIX_TEST_EMBED_MODEL=e5-de-en-server.bin go test ./internal/embed` compares
  `semantic.wasm` in wazero, in Radix's process and in worker processes, with the native
  `embed MODEL --passages` (17 passages: long ones, emoji, CJK): identical. In `Mode::Int8` the
  passages' vectors have a cosine of 0.9999 (median; ≥ 0.9998) with the original model's in f32,
  as close as `Mode::F32` (0.99994) and 1.4 times faster. Radix embeds a 512-token passage in 5 s
  on one processor (wazero; natively 1.7 s): the catalog once in about 6 processor-hours (the
  passages average 1.5 s natively), then what changed. The encoder runs in processes of its own
  (`radix embed-worker`, about 170 MB each): wazero's machine code cannot be preempted, and in
  Radix's process every garbage collection would wait for a passage — measured, its HTTP server
  answered one request in 40 s; with worker processes 757, the slowest in 11 ms.
