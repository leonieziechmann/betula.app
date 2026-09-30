//! The model for JavaScript, without wasm-bindgen: a handful of functions over the module's
//! memory (`demo/search.js` shows the calls).
//!
//! ```js
//! const at = alloc(bytes.length);                 // the packed model
//! new Uint8Array(memory.buffer, at, bytes.length).set(bytes);
//! load(at, bytes.length);                         // 0: loaded; the model keeps the buffer
//! const text = new TextEncoder().encode("query: " + query.normalize("NFC"));
//! const t = alloc(text.length), out = alloc(4 * dims());
//! new Uint8Array(memory.buffer, t, text.length).set(text);
//! const tokens = embed(t, text.length, out);      // the embedding is at `out`
//! free(t, text.length);
//! ```

use std::cell::RefCell;

use crate::Model;

thread_local! {
    static MODEL: RefCell<Option<Model>> = const { RefCell::new(None) };
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

/// Takes over the packed model at `at` (from `alloc(len)`). 0: loaded, 1: not a model.
///
/// # Safety
/// `at` and `len` as `alloc` handed them out, all `len` bytes written.
#[no_mangle]
pub unsafe extern "C" fn load(at: *mut u8, len: usize) -> i32 {
    let bytes = Vec::from_raw_parts(at, len, len);
    match Model::from_bytes(bytes) {
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
