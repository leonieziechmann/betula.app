//! `embed MODEL.bin [--mode expand|f32|int8] [--bench N]`: one text per line on stdin, one JSON
//! line per text on stdout with its ids and its embedding — what `python/parity.py` compares with
//! the Python model. With `--bench N` every text is embedded N times and only the times are printed.
//!
//! `embed MODEL.bin --search INDEX.bin [--k N]`: one query per line, one line per query with what
//! `semantic::Search` makes of it — a hash of the embedding's bits, then the hits with the bits of
//! their scores — the line `js/parity.mjs` writes for the browser's builds and compares.

use std::io::{BufRead, Write};
use std::time::Instant;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args
        .first()
        .ok_or("usage: embed MODEL.bin [--mode expand|f32|int8] [--bench N] < texts")?;
    let option = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    let bench: Option<usize> = option("--bench")
        .map(|n| n.parse().map_err(|e| format!("--bench: {e}")))
        .transpose()?;
    let mode = match option("--mode").map(String::as_str) {
        None | Some("expand") => semantic::Mode::Expand,
        Some("f32") => semantic::Mode::F32,
        Some("int8") => semantic::Mode::Int8,
        Some(other) => return Err(format!("--mode {other}: expand, f32 or int8")),
    };
    if let Some(index) = option("--search") {
        let k: usize = option("--k").map_or(Ok(10), |k| k.parse().map_err(|e| format!("--k: {e}")))?;
        let index = std::fs::read(index).map_err(|e| format!("{index}: {e}"))?;
        let search = semantic::Search::new(std::fs::read(path).map_err(|e| format!("{path}: {e}"))?, &index)?;
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        for line in std::io::stdin().lock().lines() {
            let query = line.map_err(|e| e.to_string())?;
            let hits: Vec<String> = search.search(&query, k).iter().map(|h| format!("{}:{:08x}", h.id, h.score.to_bits())).collect();
            writeln!(out, "{:08x}\t{}", fnv(&search.model().embed_query(&query)), hits.join(" ")).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let started = Instant::now();
    let model = semantic::Model::from_bytes_with(
        std::fs::read(path).map_err(|e| format!("{path}: {e}"))?,
        mode,
    )?;
    eprintln!(
        "loaded {path} in {:.0} ms, {} pieces",
        started.elapsed().as_secs_f64() * 1000.0,
        model.tokenizer().len()
    );

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in std::io::stdin().lock().lines() {
        let text = line.map_err(|e| e.to_string())?;
        let ids = model.tokenizer().encode(&text);
        match bench {
            Some(times) => {
                let started = Instant::now();
                for _ in 0..times {
                    std::hint::black_box(model.embed_ids(&ids));
                }
                let ms = started.elapsed().as_secs_f64() * 1000.0 / times.max(1) as f64;
                writeln!(out, "{ms:.2} ms\t{} tokens\t{text}", ids.len())
                    .map_err(|e| e.to_string())?;
            }
            None => {
                let embedding = model.embed_ids(&ids);
                let ids: Vec<String> = ids.iter().map(u32::to_string).collect();
                let values: Vec<String> = embedding.iter().map(|v| format!("{v:.7}")).collect();
                writeln!(
                    out,
                    "{{\"ids\":[{}],\"embedding\":[{}]}}",
                    ids.join(","),
                    values.join(",")
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// FNV-1a (32 bit) over the floats' bytes, little endian: equal only for equal bits.
fn fnv(values: &[f32]) -> u32 {
    values.iter().flat_map(|v| v.to_bits().to_le_bytes()).fold(0x811c_9dc5, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}
