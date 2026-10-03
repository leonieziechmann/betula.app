//! What the tests of Folia's crates share: the catalog snapshot they run against, the pinned one
//! of the Studienplan's checks, and direct SQL to compare with. A dev-dependency only.
//!
//! The snapshot is found through `FOLIA_TEST_SNAPSHOT` (a path to a `catalog-*.db`), else
//! through `snapshot/current.json` at the top of the repository as `radix export` writes it.
//! Without one the tests fail: a green run must mean that every query ran against real data.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::io::Write;
use std::path::PathBuf;

use folia_model::db::{Database, Value};
use folia_model::native::NativeDatabase;
use folia_model::rows::meta;

pub fn snapshot_path() -> PathBuf {
    if let Ok(path) = std::env::var("FOLIA_TEST_SNAPSHOT") {
        return PathBuf::from(path);
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../snapshot");
    let pointer = std::fs::read_to_string(dir.join("current.json")).unwrap_or_else(|e| {
        panic!(
            "no catalog snapshot for the tests ({e}). Run `radix export`, or set FOLIA_TEST_SNAPSHOT \
             to a catalog-*.db (docs/radix/operations.md)."
        )
    });
    // {"file":"catalog-<hash>.db", …}; not worth a JSON dependency.
    let file = pointer
        .split("\"file\"")
        .nth(1)
        .and_then(|rest| rest.split('"').nth(1))
        .unwrap_or_else(|| panic!("snapshot/current.json names no file: {pointer}"));
    dir.join(file)
}

pub fn open() -> NativeDatabase {
    let path = snapshot_path();
    NativeDatabase::open(&path).unwrap_or_else(|e| panic!("{e}"))
}

/// `content_digest` of the snapshot the Studienplan's pinned checks were taken from
/// (`catalog-41bcde83e1bbcaab.db`: the data of 2026-09-23, built and exported by the Radix of
/// schema 9, so with the short names of rooms and modules; docs/folia/frontend.md §4 says how to make
/// it again): the event-level expectations of `timetable`, `studyplan` and `pages` hold for this
/// one only. The data is that of `catalog-abca4baa1d8f8d8e.db` (schema 8), which they were first
/// pinned to.
pub const STUDYPLAN_DIGEST: &str = "8700613779415164c03d36c53966137e574a6e2b7ef2bf40542ad05c8f29b68b";

/// The snapshot for the pinned checks of the Studienplan named `test`, or `None` when there is
/// none to pin against; the test then asserts only what holds on any snapshot.
///
/// `FOLIA_STUDYPLAN_SNAPSHOT` names the pinned file. It stays in the main checkout's
/// `target/studyplan-snapshot/`, apart from the newer ones of `snapshot/`, so Radix's refetch every
/// three days never silences these checks for whoever sets the variable. A file with another digest fails the test: the pinned file was replaced, and
/// a green run would claim checks that did not run. Without the variable, the tests' own snapshot
/// serves when it is the pinned one; else the skip is said on stderr, once per test.
pub fn studyplan_db(test: &str) -> Option<NativeDatabase> {
    if let Some(path) = std::env::var("FOLIA_STUDYPLAN_SNAPSHOT").ok().filter(|p| !p.is_empty()) {
        let db = NativeDatabase::open(&PathBuf::from(&path))
            .unwrap_or_else(|e| panic!("FOLIA_STUDYPLAN_SNAPSHOT: {e}"));
        let digest = meta(&db).unwrap().content_digest;
        assert_eq!(
            digest.as_deref(),
            Some(STUDYPLAN_DIGEST),
            "FOLIA_STUDYPLAN_SNAPSHOT={path} is not the snapshot the Studienplan's checks were pinned to"
        );
        return Some(db);
    }
    let db = open();
    let digest = meta(&db).unwrap().content_digest;
    if digest.as_deref() == Some(STUDYPLAN_DIGEST) {
        return Some(db);
    }
    let d = digest.as_deref().unwrap_or("none");
    // Straight to the handle, not `eprintln!`: libtest swallows the macro's output of a test that
    // passes, and a skip nobody sees is the silent green run this function exists to prevent.
    let _ = writeln!(
        std::io::stderr(),
        "studyplan: pinned checks of {test} skipped: snapshot digest {d}, pinned {STUDYPLAN_DIGEST}; \
         set FOLIA_STUDYPLAN_SNAPSHOT=…/target/studyplan-snapshot/catalog-41bcde83e1bbcaab.db"
    );
    None
}

/// Direct SQL on the snapshot, to compare the query layer against.
pub fn scalar(db: &dyn Database, sql: &str) -> i64 {
    let rows = db.query("test", sql, &[]).unwrap_or_else(|e| panic!("{sql}: {e}"));
    match rows.rows.first().and_then(|r| r.first()) {
        Some(Value::Integer(n)) => *n,
        other => panic!("{sql}: expected one integer, got {other:?}"),
    }
}

pub fn column(db: &dyn Database, sql: &str) -> Vec<String> {
    let rows = db.query("test", sql, &[]).unwrap_or_else(|e| panic!("{sql}: {e}"));
    rows.rows
        .iter()
        .filter_map(|r| match r.first() {
            Some(Value::Text(s)) => Some(s.clone()),
            _ => None,
        })
        .collect()
}
