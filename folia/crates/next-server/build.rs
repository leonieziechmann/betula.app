//! Every UI crate's `style.css`, collected into one stylesheet in the order of the layers
//! (docs/folia-refactor.md §7.6): what the server's build script does in the plan.
use std::fmt::Write as _;

fn main() {
    let crates = ["design", "shell", "catalog-ui"];
    let mut out = String::from("@layer tokens, base, design, shell, widgets, features;\n");
    for name in crates {
        let path = format!("../{name}/style.css");
        println!("cargo:rerun-if-changed={path}");
        let css = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let _ = writeln!(out, "/* {name} */\n{css}");
    }
    let dir = std::env::var("OUT_DIR").unwrap_or_else(|_| ".".into());
    std::fs::write(format!("{dir}/folia.css"), out).unwrap_or_else(|e| panic!("{e}"));
}
