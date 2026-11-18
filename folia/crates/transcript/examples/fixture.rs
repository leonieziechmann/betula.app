//! Writes the made-up Notenübersicht of the checks (`folia_transcript::fixture::example`) to
//! stdout: `cargo run -p folia-transcript --features fixture --example fixture >
//! folia/e2e/fixtures/notenuebersicht.pdf`.

use std::io::Write;

fn main() {
    let pdf = folia_transcript::fixture::example();
    if let Err(error) = std::io::stdout().write_all(&pdf) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
