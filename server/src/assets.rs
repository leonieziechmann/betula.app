//! The files of `app/assets` a browser gets as text — the stylesheet, the scripts, the SVGs — as
//! the build minified them (`build/main.rs`), and what ships altogether (`folia assets`).
//!
//! While working on them the server can read them from disk instead (`--live-assets`,
//! `api::minified`): an edit is there with the next reload, without a build.

use std::io::Write as _;
use std::path::Path;

/// A file of `app/assets` as the server serves it.
pub struct Asset {
    /// Where the file lies under `app/assets`, with `/` (`birch/roots.svg`).
    pub path: &'static str,
    /// The file minified.
    pub bytes: &'static [u8],
    /// The size of the file as it is written.
    pub source: usize,
}

/// A file of `app/assets` as the build minified it, by its path there.
macro_rules! minified {
    ($path:literal) => {
        Asset {
            path: $path,
            bytes: include_bytes!(concat!(env!("OUT_DIR"), "/assets/", $path)),
            // Only the length: the file as it is written is not part of the server.
            source: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../app/assets/", $path)).len(),
        }
    };
}

/// Every minified file the server serves or draws with. The build minifies every stylesheet,
/// script and SVG of `app/assets`; a file is part of the server when it is named here.
pub static MINIFIED: &[Asset] = &[
    minified!("app.css"),
    minified!("enhance.js"),
    minified!("boot.js"),
    minified!("sw.js"),
    minified!("sql-wasm.js"),
    minified!("favicon.svg"),
    // The birch served under `/assets/birch/` (`birch::file`)…
    minified!("birch/spring-crown.svg"),
    minified!("birch/spring-crown-ck.svg"),
    minified!("birch/spring-crown-head.svg"),
    minified!("birch/spring-crown-head-ck.svg"),
    minified!("birch/summer-crown.svg"),
    minified!("birch/summer-crown-head.svg"),
    minified!("birch/autumn-crown.svg"),
    minified!("birch/autumn-crown-head.svg"),
    minified!("birch/winter-crown.svg"),
    minified!("birch/winter-crown-head.svg"),
    minified!("birch/roots.svg"),
    minified!("birch/litter.svg"),
    // …and the heads the cards hang their crown from (`birch::card_crown`), which are not.
    minified!("birch/spring-card-head.svg"),
    minified!("birch/spring-card-head-ck.svg"),
    minified!("birch/summer-card-head.svg"),
    minified!("birch/autumn-card-head.svg"),
    minified!("birch/winter-card-head.svg"),
];

pub fn get(path: &str) -> Option<&'static Asset> {
    MINIFIED.iter().find(|asset| asset.path == path)
}

/// A minified file as text (they all are); empty for a path the build does not know.
pub fn text(path: &str) -> &'static str {
    get(path).and_then(|asset| std::str::from_utf8(asset.bytes).ok()).unwrap_or_default()
}

/// The type a browser is told for a minified file.
pub fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        _ => "image/svg+xml",
    }
}

/// `folia assets`: what ships, file by file — as written and as served, and what goes over the
/// wire (gzip, as the server compresses) — the minified files of the binary and the browser app
/// in `<site-root>/pkg`.
pub fn report(site_root: &Path) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    let gzip = |bytes: &[u8]| crate::cache::gzip(bytes).len();
    writeln!(out, "{:<34} {:>10} {:>10} {:>9}", "app/assets, minified by the build", "written", "served", "gzip")?;
    let (mut written, mut served, mut wire) = (0, 0, 0);
    for asset in MINIFIED {
        let compressed = gzip(asset.bytes);
        writeln!(out, "{:<34} {:>10} {:>10} {:>9}", asset.path, asset.source, asset.bytes.len(), compressed)?;
        (written, served, wire) = (written + asset.source, served + asset.bytes.len(), wire + compressed);
    }
    writeln!(out, "{:<34} {:>10} {:>10} {:>9}", "", written, served, wire)?;

    let pkg = site_root.join("pkg");
    let mut files: Vec<_> = std::fs::read_dir(&pkg).map(|entries| entries.flatten().map(|entry| entry.path()).collect()).unwrap_or_default();
    files.sort();
    writeln!(out)?;
    if files.is_empty() {
        writeln!(out, "no browser app in {} (scripts/build-client.sh)", pkg.display())?;
    } else {
        writeln!(out, "{:<34} {:>10} {:>10} {:>9}", format!("{}", pkg.display()), "", "served", "gzip")?;
        for file in files {
            let bytes = std::fs::read(&file)?;
            let name = file.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            writeln!(out, "{:<34} {:>10} {:>10} {:>9}", name, "", bytes.len(), gzip(&bytes))?;
            if let Some(names) = custom_section(&bytes, "name") {
                writeln!(out, "  of it {names} bytes of function names: a build with --dev, not one to ship (docs/frontend.md §3)")?;
            }
        }
    }
    Ok(())
}

/// The size of a WASM file's custom section `wanted`, if it has one. Such a section is for
/// debuggers and profilers; the browser downloads it and runs nothing of it.
pub fn custom_section(wasm: &[u8], wanted: &str) -> Option<usize> {
    /// An unsigned LEB128 number at `at`, and where it ends.
    fn number(bytes: &[u8], mut at: usize) -> Option<(usize, usize)> {
        let (mut value, mut shift) = (0usize, 0u32);
        loop {
            let byte = *bytes.get(at)?;
            value |= usize::from(byte & 0x7f).checked_shl(shift)?;
            at += 1;
            if byte & 0x80 == 0 {
                return Some((value, at));
            }
            shift += 7;
        }
    }
    if wasm.get(..4) != Some(b"\0asm") {
        return None;
    }
    let mut at = 8;
    while at < wasm.len() {
        let id = *wasm.get(at)?;
        let (length, start) = number(wasm, at + 1)?;
        if id == 0 {
            let (name_length, name_start) = number(wasm, start)?;
            if wasm.get(name_start..name_start + name_length) == Some(wanted.as_bytes()) {
                return Some(length);
            }
        }
        at = start + length;
    }
    None
}

/// The minifier of the SVGs, here for its own tests (`build/main.rs` runs it).
#[cfg(test)]
#[path = "../build/svg.rs"]
mod svg;

#[cfg(test)]
mod tests {
    use super::*;
    use resvg::{tiny_skia, usvg};

    fn written(path: &str) -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/assets").join(path)).unwrap()
    }

    /// What the build made of every SVG of `app/assets` draws what the file draws, pixel for pixel
    /// at four times its size: `svg.rs` promises the same numbers, this is the renderer's word (the
    /// cards draw the crown with it; a browser reads the same grammar).
    #[test]
    fn every_minified_svg_draws_what_its_file_draws() {
        let draw = |svg: &str| {
            let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
            let size = tree.size().to_int_size();
            let mut pixmap = tiny_skia::Pixmap::new(size.width() * 4, size.height() * 4).unwrap();
            resvg::render(&tree, tiny_skia::Transform::from_scale(4.0, 4.0), &mut pixmap.as_mut());
            pixmap.take()
        };
        let built = Path::new(env!("OUT_DIR")).join("assets");
        let mut paths = vec!["favicon.svg".to_string()];
        for entry in std::fs::read_dir(built.join("birch")).unwrap() {
            paths.push(format!("birch/{}", entry.unwrap().file_name().to_string_lossy()));
        }
        assert!(paths.len() >= 23, "the favicon and the birch: {paths:?}");
        for path in paths {
            let (source, minified) = (written(&path), std::fs::read_to_string(built.join(&path)).unwrap());
            assert!(minified.len() < source.len(), "{path}");
            let (before, after) = (draw(&source), draw(&minified));
            let differ = before.iter().zip(&after).filter(|(a, b)| a != b).count();
            assert_eq!(differ, 0, "{path}: {differ} channels differ");
        }
        // What the server embeds is what the build made of the file as it is now.
        for asset in MINIFIED {
            assert_eq!(asset.source, written(asset.path).len(), "{}", asset.path);
            assert_eq!(asset.bytes, std::fs::read(built.join(asset.path)).unwrap(), "{}", asset.path);
        }
    }

    #[test]
    fn the_stylesheet_and_the_scripts_are_minified() {
        for path in ["app.css", "enhance.js", "boot.js", "sw.js", "sql-wasm.js"] {
            let asset = get(path).unwrap();
            assert!(asset.bytes.len() < asset.source, "{path}: {} of {}", asset.bytes.len(), asset.source);
            assert!(!text(path).contains("\n  "), "{path} is not indented any more");
        }
        // sql.js is a classic script: the global it defines is what boot.js calls.
        assert!(text("sql-wasm.js").contains("initSqlJs"), "{}", text("sql-wasm.js").get(..400).unwrap_or_default());
        // What the server writes in when it serves them (`api`) is still there to be written.
        assert!(text("boot.js").contains("__SCHEMA__") && text("sw.js").contains("__BUILD__"));
        // The stylesheet names every mask of the birch as the written one does (`tests.rs` serves
        // each of them).
        let masks = |css: &str| -> std::collections::BTreeSet<String> {
            css.split("/assets/birch/").skip(1).filter_map(|rest| rest.split(['"', ')']).next()).map(str::to_string).collect()
        };
        assert_eq!(masks(text("app.css")), masks(&written("app.css")));
    }

    #[test]
    fn a_wasm_file_tells_its_custom_sections() {
        // An empty module with a custom section of five bytes: its name's length and "name".
        let wasm = b"\0asm\x01\0\0\0\x00\x05\x04name";
        assert_eq!(custom_section(wasm, "name"), Some(5));
        assert_eq!(custom_section(wasm, "producers"), None);
        assert_eq!(custom_section(b"not wasm", "name"), None);
        assert_eq!(custom_section(b"\0asm\x01\0\0\0\x00\x7f", "name"), None, "a section longer than the file");
    }
}
