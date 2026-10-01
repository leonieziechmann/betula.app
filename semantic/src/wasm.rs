//! The exports of the Web Worker that searches in the browser (`js/worker.js`), without
//! wasm-bindgen: a handful of functions over the module's memory. Built with feature `worker`
//! (scripts/build-semantic.sh).
//!
//! ```js
//! const m = alloc(model.length);                  // the packed model (the browser's, 4 bit)
//! new Uint8Array(memory.buffer, m, model.length).set(model);
//! const i = alloc(index.length);                  // the index the server built
//! new Uint8Array(memory.buffer, i, index.length).set(index);
//! load_search(m, model.length, i, index.length);  // the rows; keeps the model's buffer
//! free(i, index.length);
//! const q = new TextEncoder().encode(query);      // as typed: no normalising here (see Search)
//! const t = alloc(q.length), rows = alloc(4 * k), scores = alloc(4 * k);
//! new Uint8Array(memory.buffer, t, q.length).set(q);
//! const n = search(t, q.length, k, rows, scores); // the best n rows and their scores
//! const id = id_of(row, len_at);                  // where the row's id is, its length at len_at
//! ```
//!
//! `load` and `embed` (any mode) are the PoC's (poc/semantic-search/demo), apart from the search.
//!
//! For an encoder elsewhere (the WebGPU experiment of poc/semantic-search) only the tokenizer is
//! needed: `load_tokenizer` with the start of the file, then `tokenize`.
//!
//! Radix runs the same module (in wazero, `internal/embed`) to compute the modules' vectors:
//! `load_encoder` with the server's model, then `embed_passage` for each module's passage.

use std::cell::RefCell;

use crate::{tokenizer_from_bytes, Mode, Model, Search, Tokenizer};

thread_local! {
    static MODEL: RefCell<Option<Model>> = const { RefCell::new(None) };
    static TOKENIZER: RefCell<Option<Tokenizer>> = const { RefCell::new(None) };
    static SEARCH: RefCell<Option<Search>> = const { RefCell::new(None) };
    static ENCODER: RefCell<Option<Model>> = const { RefCell::new(None) };
}

/// Room for `len` bytes, for JavaScript to write into.
#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len);
    let at = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    at
}

/// # Safety
/// `at` and `len` as `alloc` handed them out, and not given to `load`.
#[no_mangle]
pub unsafe extern "C" fn free(at: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(at, 0, len));
}

/// Takes over the packed model at `at` (from `alloc(len)`) and prepares it for `mode` (0: expand
/// rows per query, 1: f32, 2: int8; `Mode`). 0: loaded, 1: not a model.
///
/// # Safety
/// `at` and `len` as `alloc` handed them out, all `len` bytes written.
#[no_mangle]
pub unsafe extern "C" fn load(at: *mut u8, len: usize, mode: u32) -> i32 {
    let bytes = Vec::from_raw_parts(at, len, len);
    let mode = match mode {
        1 => Mode::F32,
        2 => Mode::Int8,
        _ => Mode::Expand,
    };
    match Model::from_bytes_with(bytes, mode) {
        Ok(model) => {
            MODEL.with(|m| *m.borrow_mut() = Some(model));
            0
        }
        Err(_) => 1,
    }
}

/// Floats in an embedding; 0 before `load`.
#[no_mangle]
pub extern "C" fn dims() -> usize {
    MODEL.with(|m| m.borrow().as_ref().map_or(0, Model::dims))
}

/// Embeds the UTF-8 text at `text` into the `dims()` floats at `out`. Returns the number of
/// tokens the text was cut into, -1 without a model or for text that is not UTF-8.
///
/// # Safety
/// `text` holds `len` bytes, `out` room for `dims()` floats.
#[no_mangle]
pub unsafe extern "C" fn embed(text: *const u8, len: usize, out: *mut f32) -> i32 {
    let Ok(text) = std::str::from_utf8(std::slice::from_raw_parts(text, len)) else { return -1 };
    MODEL.with(|m| {
        let model = m.borrow();
        let Some(model) = model.as_ref() else { return -1 };
        let ids = model.tokenizer().encode(text);
        let embedding = model.embed_ids(&ids);
        std::slice::from_raw_parts_mut(out, embedding.len()).copy_from_slice(&embedding);
        i32::try_from(ids.len()).unwrap_or(i32::MAX)
    })
}

/// Reads the tokenizer of the packed model whose first `len` bytes (at least up to the tensors)
/// are at `at`; the bytes stay JavaScript's to `free`. 0: loaded, 1: not a model.
///
/// # Safety
/// `at` holds `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn load_tokenizer(at: *const u8, len: usize) -> i32 {
    match tokenizer_from_bytes(std::slice::from_raw_parts(at, len)) {
        Ok(tokenizer) => {
            TOKENIZER.with(|t| *t.borrow_mut() = Some(tokenizer));
            0
        }
        Err(_) => 1,
    }
}

/// Cuts the UTF-8 text at `text` into token ids (`<s>` … `</s>`), the first `room` of them into
/// `out`. Returns how many there are (possibly more than `room`), -1 without a tokenizer (from
/// `load_tokenizer` or `load`) or for text that is not UTF-8.
///
/// # Safety
/// `text` holds `len` bytes, `out` room for `room` u32.
#[no_mangle]
pub unsafe extern "C" fn tokenize(text: *const u8, len: usize, out: *mut u32, room: usize) -> i32 {
    let Ok(text) = std::str::from_utf8(std::slice::from_raw_parts(text, len)) else { return -1 };
    let ids = TOKENIZER
        .with(|t| t.borrow().as_ref().map(|t| t.encode(text)))
        .or_else(|| MODEL.with(|m| m.borrow().as_ref().map(|m| m.tokenizer().encode(text))));
    let Some(ids) = ids else { return -1 };
    let n = ids.len().min(room);
    std::slice::from_raw_parts_mut(out, n).copy_from_slice(ids.get(..n).unwrap_or_default());
    i32::try_from(ids.len()).unwrap_or(i32::MAX)
}

/// The search (`Search`): the packed model at `model` (from `alloc(model_len)`, taken over like
/// `load` takes it) and the index at `index` (stays JavaScript's to `free`). Returns the index's
/// rows, -1 for a file that is not what it should be.
///
/// # Safety
/// `model` and `model_len` as `alloc` handed them out, all bytes written; `index` holds
/// `index_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn load_search(model: *mut u8, model_len: usize, index: *const u8, index_len: usize) -> i32 {
    let model = Vec::from_raw_parts(model, model_len, model_len);
    match Search::new(model, std::slice::from_raw_parts(index, index_len)) {
        Ok(search) => {
            let rows = i32::try_from(search.index().len()).unwrap_or(i32::MAX);
            SEARCH.with(|s| *s.borrow_mut() = Some(search));
            rows
        }
        Err(_) => -1,
    }
}

/// Another index for the search (`Search::set_index`); the bytes stay JavaScript's to `free`.
/// Returns its rows, -1 without a search or for what is not an index.
///
/// # Safety
/// `at` holds `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn load_index(at: *const u8, len: usize) -> i32 {
    SEARCH.with(|s| {
        let mut search = s.borrow_mut();
        let Some(search) = search.as_mut() else { return -1 };
        match search.set_index(std::slice::from_raw_parts(at, len)) {
            Ok(()) => i32::try_from(search.index().len()).unwrap_or(i32::MAX),
            Err(_) => -1,
        }
    })
}

/// `Search::search` for the UTF-8 query at `text` (without „query: “): the `k` best rows of the
/// index, best first, to `rows`, their scores to `scores`. Returns how many there are (k at
/// most, fewer for a small index), -1 without a search or for text that is not UTF-8.
///
/// # Safety
/// `text` holds `len` bytes, `rows` and `scores` room for `k` values each.
#[no_mangle]
pub unsafe extern "C" fn search(text: *const u8, len: usize, k: usize, rows: *mut u32, scores: *mut f32) -> i32 {
    let Ok(text) = std::str::from_utf8(std::slice::from_raw_parts(text, len)) else { return -1 };
    SEARCH.with(|s| {
        let search = s.borrow();
        let Some(search) = search.as_ref() else { return -1 };
        let hits = search.search(text, k);
        let rows = std::slice::from_raw_parts_mut(rows, hits.len());
        let scores = std::slice::from_raw_parts_mut(scores, hits.len());
        for ((row, score), hit) in rows.iter_mut().zip(scores.iter_mut()).zip(&hits) {
            *row = u32::try_from(hit.row).unwrap_or(u32::MAX);
            *score = hit.score;
        }
        i32::try_from(hits.len()).unwrap_or(i32::MAX)
    })
}

/// Where the id of row `row` of the index is (UTF-8, its length written to `len`); null for a
/// row the index does not have. Valid until the next `load_index`.
///
/// # Safety
/// `len` has room for a usize.
#[no_mangle]
pub unsafe extern "C" fn id_of(row: usize, len: *mut usize) -> *const u8 {
    SEARCH.with(|s| match s.borrow().as_ref().and_then(|search| search.index().id(row)) {
        Some(id) => {
            *len = id.len();
            id.as_ptr()
        }
        None => std::ptr::null(),
    })
}

/// The query's embedding as `Search::search` computes it (`Model::embed_query`) into the
/// `dims` floats at `out`; for checking that the server computes the same bits. 0, or -1 without
/// a search or for text that is not UTF-8.
///
/// # Safety
/// `text` holds `len` bytes, `out` room for the model's dims floats.
#[no_mangle]
pub unsafe extern "C" fn embed_query(text: *const u8, len: usize, out: *mut f32) -> i32 {
    let Ok(text) = std::str::from_utf8(std::slice::from_raw_parts(text, len)) else { return -1 };
    SEARCH.with(|s| {
        let search = s.borrow();
        let Some(search) = search.as_ref() else { return -1 };
        let embedding = search.model().embed_query(text);
        std::slice::from_raw_parts_mut(out, embedding.len()).copy_from_slice(&embedding);
        0
    })
}

/// Radix's encoder (`internal/embed`, which runs this module in wazero): takes over the packed
/// model at `at` (from `alloc(len)`; the server's, 8 bit and 512 positions) for passages, in
/// `Mode::Int8` — the arithmetic every build computes alike, so the vectors Radix publishes are
/// the bits this crate computes natively. Returns the dims, -1 if the bytes are not a model.
///
/// # Safety
/// `at` and `len` as `alloc` handed them out, all `len` bytes written.
#[no_mangle]
pub unsafe extern "C" fn load_encoder(at: *mut u8, len: usize) -> i32 {
    let bytes = Vec::from_raw_parts(at, len, len);
    match Model::from_bytes_with(bytes, Mode::Int8) {
        Ok(model) => {
            let dims = i32::try_from(model.dims()).unwrap_or(-1);
            ENCODER.with(|e| *e.borrow_mut() = Some(model));
            dims
        }
        Err(_) => -1,
    }
}

/// The vector of a module's passage (`Model::embed_passage`: „passage: “ added, cut off at the
/// model's positions) as Radix publishes it (`quantize`): dims / 2 bytes of packed values at
/// `packed`, their scale at `scale`. Returns the tokens of the passage before the cut, -1 without
/// `load_encoder` or for text that is not UTF-8.
///
/// # Safety
/// `text` holds `len` bytes, `packed` room for dims / 2 bytes, `scale` for one f32.
#[no_mangle]
pub unsafe extern "C" fn embed_passage(text: *const u8, len: usize, packed: *mut u8, scale: *mut f32) -> i32 {
    let Ok(text) = std::str::from_utf8(std::slice::from_raw_parts(text, len)) else { return -1 };
    ENCODER.with(|e| {
        let encoder = e.borrow();
        let Some(model) = encoder.as_ref() else { return -1 };
        // `embed_passage`, with the tokens counted on the way.
        let ids = model.tokenizer().encode(&format!("passage: {text}"));
        let (s, p) = crate::quantize(&model.embed_ids(&ids));
        std::slice::from_raw_parts_mut(packed, p.len()).copy_from_slice(&p);
        *scale = s;
        i32::try_from(ids.len()).unwrap_or(i32::MAX)
    })
}
