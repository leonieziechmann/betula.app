//! R24 (docs/folia-refactor.md §10): a crate uses only crates of lower layers, and of its own
//! layer those `layers.toml` lists under `within`; features never use features; every crate of
//! the workspace has a layer and is named `folia-<crate>`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;
use std::process::Command;

#[test]
fn every_crate_keeps_to_its_layer() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let layers: toml::Table = std::fs::read_to_string(format!("{root}/layers.toml")).unwrap().parse().unwrap();
    let order: Vec<&str> = layers["order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    let rank = |crate_name: &str| -> Option<(usize, &str)> {
        let layer = layers["crates"].get(crate_name)?.as_str()?;
        Some((order.iter().position(|l| *l == layer).unwrap(), layer))
    };
    let within = |crate_name: &str, dep: &str| -> bool {
        layers.get("within").and_then(|w| w.get(crate_name)).and_then(|v| v.as_array()).is_some_and(|list| list.iter().any(|v| v.as_str() == Some(dep)))
    };

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo).args(["metadata", "--format-version", "1", "--no-deps"]).current_dir(root).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let members: Vec<&str> = metadata["workspace_members"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();

    let mut wrong = BTreeMap::new();
    for package in metadata["packages"].as_array().unwrap() {
        if !members.contains(&package["id"].as_str().unwrap()) {
            continue;
        }
        let name = package["name"].as_str().unwrap();
        if !name.starts_with("folia-") {
            wrong.insert(name.to_string(), "is not named folia-<crate>".to_string());
        }
        let Some((own, layer)) = rank(name) else {
            wrong.insert(name.to_string(), "has no layer in layers.toml".to_string());
            continue;
        };
        for dep in package["dependencies"].as_array().unwrap() {
            let dep_name = dep["name"].as_str().unwrap();
            if dep["kind"].as_str() == Some("dev") {
                continue;
            }
            let Some((theirs, their_layer)) = rank(dep_name) else { continue };
            let allowed = theirs < own || (theirs == own && layer != "features" && within(name, dep_name));
            if !allowed {
                wrong.insert(format!("{name} → {dep_name}"), format!("{layer} may not use {their_layer}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
