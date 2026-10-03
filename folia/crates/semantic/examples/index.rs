//! The index of a snapshot's modules, built as the server would build it:
//!
//!     cargo run -p folia-semantic --release --example index -- MODEL.bin catalog.db index.bin [--mode f32|int8|expand] [--limit N]
//!
//! MODEL.bin is best the server's model (q8, 512 positions; folia/crates/semantic/README.md): a module's
//! description runs to several hundred tokens. Prints how long it took.

use std::time::Instant;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [model, db, out, ..] = args.as_slice() else {
        return Err("usage: index MODEL.bin catalog.db index.bin [--mode f32|int8|expand] [--limit N]".into());
    };
    let option = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let mode = match option("--mode").map(String::as_str) {
        None | Some("f32") => folia_semantic::Mode::F32,
        Some("int8") => folia_semantic::Mode::Int8,
        Some("expand") => folia_semantic::Mode::Expand,
        Some(other) => return Err(format!("--mode {other}")),
    };
    let limit: usize = option("--limit").map_or(Ok(usize::MAX), |n| n.parse().map_err(|e| format!("--limit: {e}")))?;

    let started = Instant::now();
    let model = folia_semantic::Model::from_bytes_with(std::fs::read(model).map_err(|e| format!("{model}: {e}"))?, mode)?;
    eprintln!("model: {} positions, loaded in {:.1} s", model.positions(), started.elapsed().as_secs_f64());

    let db = rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
    let mut statement = db
        .prepare("SELECT CAST(id AS TEXT), coalesce(title_de, title, ''), title_en, contents, learning_outcomes FROM v_module ORDER BY id")
        .map_err(|e| e.to_string())?;
    let documents: Vec<(String, String)> = statement
        .query_map([], |row| {
            let (id, de, en, contents, outcomes): (String, String, Option<String>, Option<String>, Option<String>) =
                (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?);
            Ok((id, folia_semantic::module_text(&de, en.as_deref(), contents.as_deref(), outcomes.as_deref())))
        })
        .map_err(|e| e.to_string())?
        .take(limit)
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let started = Instant::now();
    let index = folia_semantic::Index::build(&model, &documents, threads)?;
    let seconds = started.elapsed().as_secs_f64();
    let bytes = index.to_bytes()?;
    std::fs::write(out, &bytes).map_err(|e| format!("{out}: {e}"))?;
    eprintln!(
        "{} modules on {threads} threads in {seconds:.1} s ({:.0} ms a module and thread) → {out} ({:.1} MB)",
        index.len(),
        seconds * 1000.0 * threads as f64 / index.len().max(1) as f64,
        bytes.len() as f64 / 1e6
    );
    Ok(())
}
