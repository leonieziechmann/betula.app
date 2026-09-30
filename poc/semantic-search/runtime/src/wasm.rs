//! The model for JavaScript, without wasm-bindgen: a handful of functions over the module's
//! memory (`demo/e5.js` wraps them).
//!
//! ```js
//! const at = alloc(bytes.length);                 // the packed model
//! new Uint8Array(memory.buffer, at, bytes.length).set(bytes);
//! load(at, bytes.length, 2);                      // 0: loaded (mode 2: int8); keeps the buffer
//! const text = new TextEncoder().encode("query: " + query.normalize("NFC"));
//! const t = alloc(text.length), out = alloc(4 * dims());
//! new Uint8Array(memory.buffer, t, text.length).set(text);
//! const tokens = embed(t, text.length, out);      // the embedding is at `out`
//! free(t, text.length);
//! ```
//!
//! For an encoder on the GPU (`demo/e5-gpu.js`) only the tokenizer is needed: `load_tokenizer`
//! with the start of the file (up to the tensors), then `tokenize` hands out the token ids.

use std::cell::RefCell;

use crate::{tokenizer_from_bytes, Mode, Model, Tokenizer};

thread_local! {
    static MODEL: RefCell<Option<Model>> = const { RefCell::new(None) };
    static TOKENIZER: RefCell<Option<Tokenizer>> = const { RefCell::new(None) };
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
