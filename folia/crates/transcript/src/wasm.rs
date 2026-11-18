//! The exports of the Web Worker that reads a Notenübersicht in the browser (`js/worker.js`),
//! without wasm-bindgen, as the semantic search's: three functions over the module's memory.
//! Built with feature `worker` (scripts/build-client.sh).
//!
//! ```js
//! const at = alloc(pdf.length);                    // room for the file
//! new Uint8Array(memory.buffer, at, pdf.length).set(pdf);
//! const length = read(at, pdf.length);             // takes the bytes over; the answer's length
//! const lines = new TextDecoder().decode(new Uint8Array(memory.buffer, answer(), length));
//! ```
//!
//! The answer is `Transcript::lines`, or `error <code>` (`Error::code`). The worker reads one file
//! and is ended once it has answered: what the module held of the file goes with its memory.

use std::cell::RefCell;

thread_local! {
    static ANSWER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Room for `len` bytes, for JavaScript to write the file into.
#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len);
    let at = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    at
}

/// Reads the file at `at` (from `alloc(len)`, all `len` bytes written), which it takes over and
/// gives back; the length of the answer, which `answer` points to.
///
/// # Safety
/// `at` and `len` as `alloc` handed them out, all `len` bytes written.
#[no_mangle]
pub unsafe extern "C" fn read(at: *mut u8, len: usize) -> usize {
    let pdf = Vec::from_raw_parts(at, len, len);
    let text = match crate::read(&pdf) {
        Ok(transcript) => transcript.lines(),
        Err(error) => format!("error\t{}\n", error.code()),
    };
    drop(pdf);
    ANSWER.with(|answer| {
        *answer.borrow_mut() = text.into_bytes();
        answer.borrow().len()
    })
}

/// Where the last answer is.
#[no_mangle]
pub extern "C" fn answer() -> *const u8 {
    ANSWER.with(|answer| answer.borrow().as_ptr())
}
