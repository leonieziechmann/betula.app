//! The UI bundle as a binary, for wasm-split-cli (it looks for `main`). `start` stays the entry.
fn main() {
    let _ = folia_app::start as fn(web_sys::Worker);
}
