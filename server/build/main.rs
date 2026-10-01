//! The files of `app/assets` a browser gets as text — the stylesheet, the scripts, the SVGs —
//! minified into `OUT_DIR/assets`, under the same paths, where the server embeds them from
//! (`src/assets.rs`). Every other file there (pictures, the font, sql.js's WASM) is compressed
//! already and embedded as it is. docs/frontend.md, „What ships".
//!
//! - **The stylesheet** with lightningcss, for the browsers that read it as it is (no `targets`:
//!   nothing is lowered or prefixed, only written shorter).
//! - **The scripts** with oxc: comments and whitespace out, names shortened, what is constant
//!   folded, in no newer syntax than the scripts are written in (`SYNTAX`). A classic script's
//!   top-level names are globals other scripts use (sql.js's `initSqlJs`), so it is read as a
//!   script and they stay; only what a module alone can be (`import.meta`, a top-level `await`:
//!   `boot.js`) is read as a module.
//! - **The SVGs** by `svg.rs`: the same drawing, number for number, written shorter.
//!
//! The server writes into two scripts (`__BUILD__` into `sw.js`, `__SCHEMA__` and
//! `__SEMANTIC_MODEL__` into `boot.js`): a
//! placeholder the minifier folded away would ship a script that no longer knows its build, so
//! the build fails instead.
//!
//! A file that does not parse fails the build too, with the minifier's message: what the server
//! would otherwise ship is a stylesheet or a script no browser reads either.

use std::path::{Path, PathBuf};

#[path = "svg.rs"]
mod svg;

type Error = Box<dyn std::error::Error>;

fn main() -> Result<(), Error> {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or("CARGO_MANIFEST_DIR is not set")?);
    let assets = manifest.join("..").join("app").join("assets");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
    // A directory: cargo looks at every file below it, new ones included.
    println!("cargo:rerun-if-changed={}", assets.display());

    let mut files = Vec::new();
    collect(&assets, "", &mut files)?;
    for path in files {
        let source = std::fs::read_to_string(assets.join(&path)).map_err(|e| format!("app/assets/{path}: {e}"))?;
        let minified = minify(&path, &source).map_err(|e| format!("app/assets/{path}: {e}"))?;
        for placeholder in PLACEHOLDERS.iter().filter(|placeholder| source.contains(*placeholder)) {
            if !minified.contains(placeholder) {
                return Err(format!("app/assets/{path}: the minifier folded away {placeholder}, which the server writes into it; write it where no minifier can evaluate it, as a name of its own (boot.js: `const SCHEMA = __SCHEMA__;`)").into());
            }
        }
        let target = out.join("assets").join(&path);
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&target, &minified)?;
    }
    Ok(())
}

/// The files below `dir` a browser gets as text, as paths under `app/assets` with `/`.
fn collect(dir: &Path, prefix: &str, files: &mut Vec<String>) -> Result<(), Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if entry.file_type()?.is_dir() {
            collect(&entry.path(), &format!("{path}/"), files)?;
        } else if [".css", ".js", ".svg"].iter().any(|kind| name.ends_with(kind)) {
            files.push(path);
        }
    }
    Ok(())
}

fn minify(path: &str, source: &str) -> Result<String, Error> {
    if path.ends_with(".css") {
        stylesheet(source)
    } else if path.ends_with(".js") {
        script(source)
    } else {
        Ok(svg::minify(source))
    }
}

fn stylesheet(source: &str) -> Result<String, Error> {
    use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
    use lightningcss::visitor::Visit;
    let mut sheet = StyleSheet::parse(source, ParserOptions::default()).map_err(|e| e.to_string())?;
    sheet.minify(MinifyOptions::default()).map_err(|e| e.to_string())?;
    let Ok(()) = sheet.visit(&mut TimelineApart);
    let printed = sheet.to_css(PrinterOptions { minify: true, ..PrinterOptions::default() }).map_err(|e| e.to_string())?;
    Ok(printed.code)
}

/// Takes an animation's timeline out of the `animation` shorthand again, into an
/// `animation-timeline` after it. lightningcss folds `animation: wood-rise linear both;
/// animation-timeline: --page` into `animation:linear both wood-rise --page`, as its data says
/// Chrome reads that since 115; Chrome 141 does not, and drops the whole declaration (the wood of
/// a page stopped moving with its scroll, e2e/ground.mjs). The spec has the shorthand only reset
/// the timeline, never set it.
struct TimelineApart;

impl<'i> lightningcss::visitor::Visitor<'i> for TimelineApart {
    type Error = std::convert::Infallible;

    fn visit_types(&self) -> lightningcss::visitor::VisitTypes {
        lightningcss::visitor::VisitTypes::PROPERTIES
    }

    fn visit_declaration_block(&mut self, block: &mut lightningcss::declaration::DeclarationBlock<'i>) -> Result<(), Self::Error> {
        use lightningcss::properties::animation::AnimationTimeline;
        use lightningcss::properties::Property;
        for declarations in [&mut block.declarations, &mut block.important_declarations] {
            let mut apart = Vec::with_capacity(declarations.len());
            for property in declarations.drain(..) {
                match property {
                    Property::Animation(mut animations, prefix) if animations.iter().any(|animation| animation.timeline != AnimationTimeline::Auto) => {
                        let timelines = animations.iter_mut().map(|animation| std::mem::replace(&mut animation.timeline, AnimationTimeline::Auto)).collect();
                        apart.push(Property::Animation(animations, prefix));
                        apart.push(Property::AnimationTimeline(timelines));
                    }
                    property => apart.push(property),
                }
            }
            *declarations = apart;
        }
        Ok(())
    }
}

fn script(source: &str) -> Result<String, Error> {
    use oxc_allocator::Allocator;
    use oxc_codegen::{Codegen, CodegenOptions};
    use oxc_compat::EngineTargets;
    use oxc_minifier::{CompressOptions, Minifier, MinifierOptions};
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::script()).parse();
    if !parsed.diagnostics.is_empty() {
        let module = Parser::new(&allocator, source, SourceType::mjs()).parse();
        if !module.diagnostics.is_empty() {
            let errors: Vec<String> = module.diagnostics.iter().map(ToString::to_string).collect();
            return Err(errors.join("; ").into());
        }
        parsed = module;
    }
    let mut program = parsed.program;
    let compress = CompressOptions { target: EngineTargets::from_target(SYNTAX)?, ..CompressOptions::smallest() };
    let minified = Minifier::new(MinifierOptions { compress: Some(compress), ..MinifierOptions::default() }).minify(&allocator, &mut program);
    Ok(Codegen::new().with_options(CodegenOptions::minify()).with_scoping(minified.scoping).build(&program).code)
}

/// The newest syntax the minifier may write: what the classic scripts are written in themselves
/// (`?.` and `??` in `enhance.js`). Left to itself it writes the newest there is, and turned
/// `a = a || b` into `a ||= b` (ES2021) in a script every browser loads.
const SYNTAX: &str = "es2020";

/// What the server writes into its scripts when it serves them (`src/api.rs`: the build into the
/// service worker, the schema and the semantic search's model into `boot.js`).
const PLACEHOLDERS: [&str; 3] = ["__BUILD__", "__SCHEMA__", "__SEMANTIC_MODEL__"];
