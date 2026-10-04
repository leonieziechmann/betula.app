//! The layers of the workspace hold (`folia/layers.toml`, docs/folia/folia-refactor.md §7.2, R24):
//! what `cargo metadata` says every crate depends on, against the table of layers.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

struct Layer {
    name: String,
    crates: Vec<String>,
    apart: bool,
    no_sql: bool,
    tests: bool,
}

/// The few shapes `layers.toml` uses: `[[layer]]`, `name = "…"`, `crates = […]` (over several
/// lines too) and the flags.
fn layers(text: &str) -> Vec<Layer> {
    let mut layers: Vec<Layer> = Vec::new();
    let mut in_crates = false;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
        if line == "[[layer]]" {
            layers.push(Layer { name: String::new(), crates: Vec::new(), apart: false, no_sql: false, tests: false });
            continue;
        }
        let layer = layers.last_mut().expect("a key before the first [[layer]]");
        let quoted = |part: &str| part.split('"').skip(1).step_by(2).map(str::to_string).collect::<Vec<_>>();
        if in_crates || line.starts_with("crates") {
            layer.crates.extend(quoted(line.split_once('=').map_or(line, |(_, value)| value)));
            in_crates = !line.ends_with(']');
        } else if let Some((key, value)) = line.split_once('=') {
            let (key, value) = (key.trim(), value.trim());
            match key {
                "name" => layer.name = quoted(value).concat(),
                "apart" => layer.apart = value == "true",
                "no_sql" => layer.no_sql = value == "true",
                "tests" => layer.tests = value == "true",
                _ => panic!("layers.toml: unknown key {key}"),
            }
        }
    }
    layers
}

/// Every crate of the workspace with the workspace crates it depends on (normal, build and dev).
fn dependencies(workspace: &Path) -> BTreeMap<String, BTreeSet<(String, bool)>> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo).args(["metadata", "--no-deps", "--format-version", "1", "--offline"]).current_dir(workspace).output().expect("cargo metadata");
    assert!(output.status.success(), "cargo metadata: {}", String::from_utf8_lossy(&output.stderr));
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).expect("the metadata as JSON");
    let packages = metadata["packages"].as_array().expect("packages");
    let names: BTreeSet<String> = packages.iter().map(|package| package["name"].as_str().expect("a name").to_string()).collect();
    packages
        .iter()
        .map(|package| {
            let uses = package["dependencies"]
                .as_array()
                .expect("dependencies")
                .iter()
                .filter_map(|dependency| {
                    let name = dependency["name"].as_str()?.to_string();
                    let dev = dependency["kind"].as_str() == Some("dev");
                    names.contains(&name).then_some((name, dev))
                })
                .collect();
            (package["name"].as_str().expect("a name").to_string(), uses)
        })
        .collect()
}

#[test]
fn every_crate_uses_only_what_its_layer_may() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let table = layers(&std::fs::read_to_string(workspace.join("layers.toml")).expect("folia/layers.toml"));
    // Where each crate stands: its layer and its place in that layer.
    let mut place: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for (level, layer) in table.iter().enumerate() {
        for (index, name) in layer.crates.iter().enumerate() {
            assert!(place.insert(name, (level, index)).is_none(), "{name} is in two layers");
        }
    }
    let dependencies = dependencies(&workspace);
    let mut wrong = Vec::new();
    for name in dependencies.keys() {
        if !place.contains_key(name.as_str()) {
            wrong.push(format!("{name} is in no layer of layers.toml"));
        }
    }
    for (name, uses) in &dependencies {
        let Some(&(level, index)) = place.get(name.as_str()) else { continue };
        let layer = &table[level];
        for (used, dev) in uses {
            let Some(&(used_level, used_index)) = place.get(used.as_str()) else { continue };
            let used_layer = &table[used_level];
            if *dev && used_layer.tests {
                continue;
            }
            if used_level > level || (used_level == level && (layer.apart || used_index > index)) {
                wrong.push(format!("{name} ({}) uses {used} ({})", layer.name, used_layer.name));
            }
            if layer.no_sql && used == "folia-query" {
                wrong.push(format!("{name} ({}) names the SQL of folia-query", layer.name));
            }
        }
    }
    assert!(wrong.is_empty(), "the layers do not hold:\n{}", wrong.join("\n"));
}

#[test]
fn the_table_reads_as_written() {
    let table = layers("[[layer]]\nname = \"a\"\napart = true\ncrates = [\"x\",\n  \"y\"]\n\n[[layer]]\nname = \"b\"\ncrates = [\"z\"]\n");
    assert_eq!(table.iter().map(|layer| (layer.name.as_str(), layer.crates.clone(), layer.apart)).collect::<Vec<_>>(), vec![("a", vec!["x".to_string(), "y".to_string()], true), ("b", vec!["z".to_string()], false)]);
}
