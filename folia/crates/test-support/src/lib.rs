//! What the tests of Folia's crates share: the catalog snapshot they run against, the pinned one
//! of the Studienplan's checks, and direct SQL to compare with. A dev-dependency only.
//!
//! The snapshot is found through `FOLIA_TEST_SNAPSHOT` (a path to a `catalog-*.db`), else
//! through `snapshot/current.json` at the top of the repository as `radix export` writes it.
//! Without one the tests fail: a green run must mean that every query ran against real data.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use folia_model::db::{Database, Value};
use folia_model::native::NativeDatabase;
use folia_model::rows::meta;
use folia_model::SCHEMA_VERSION;

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
/// pinned to. The checks read it as Radix's later migrations leave it (`migrated`): what they add
/// is empty, the series of single dates of schema 13 among it.
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
    let migrated_db = |path: &Path| NativeDatabase::open(&migrated(path)).unwrap_or_else(|e| panic!("{e}"));
    if let Some(path) = std::env::var("FOLIA_STUDYPLAN_SNAPSHOT").ok().filter(|p| !p.is_empty()) {
        let db = NativeDatabase::open(&PathBuf::from(&path))
            .unwrap_or_else(|e| panic!("FOLIA_STUDYPLAN_SNAPSHOT: {e}"));
        let digest = meta(&db).unwrap().content_digest;
        assert_eq!(
            digest.as_deref(),
            Some(STUDYPLAN_DIGEST),
            "FOLIA_STUDYPLAN_SNAPSHOT={path} is not the snapshot the Studienplan's checks were pinned to"
        );
        return Some(migrated_db(Path::new(&path)));
    }
    let db = open();
    let digest = meta(&db).unwrap().content_digest;
    if digest.as_deref() == Some(STUDYPLAN_DIGEST) {
        return Some(migrated_db(&snapshot_path()));
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

/// The snapshot at `path` as the queries read it: the file itself where its schema is the queries'
/// (`SCHEMA_VERSION`) or newer, else a copy that Radix's newer migrations
/// (`radix/internal/catalogdb/migrations`) were applied to, one transaction each, as Radix applies
/// them to an older database before it builds it again. No build follows here: what they add is
/// empty, as in such a database, and the data, its digest included, stays the file's. The pinned
/// snapshot of the Studienplan's checks is older than the queries (schema 9); this lets them read
/// it. The copy is made once per file and schema, in the system's temporary directory, and found
/// there again.
pub fn migrated(path: &Path) -> PathBuf {
    let db = NativeDatabase::open(path).unwrap_or_else(|e| panic!("{e}"));
    let version = db.schema_version().unwrap();
    if version >= SCHEMA_VERSION {
        return path.to_path_buf();
    }
    let digest = meta(&db).unwrap().content_digest.unwrap_or_else(|| "none".to_string());
    let dir = std::env::temp_dir().join("folia-migrated-snapshots");
    let copy = dir.join(format!("{}-schema-{SCHEMA_VERSION}.db", digest.get(..16).unwrap_or(&digest)));
    if copy.exists() {
        return copy;
    }
    std::fs::create_dir_all(&dir).unwrap();
    // Made beside the copy under a name of its own and renamed when done: tests in other threads
    // and processes may ask for it at the same moment, and none may open a copy half made.
    static MADE: AtomicUsize = AtomicUsize::new(0);
    let making = dir.join(format!("{}-{}.making", std::process::id(), MADE.fetch_add(1, Ordering::Relaxed)));
    std::fs::copy(path, &making).unwrap();
    let mut conn = rusqlite::Connection::open(&making).unwrap();
    for (number, file) in radix_migrations().into_iter().filter(|(number, _)| *number > version) {
        apply(&mut conn, number, &file);
    }
    drop(conn);
    // Another test may have renamed its copy there first (Windows refuses to replace a file that is
    // open); the two are the same.
    if let Err(e) = std::fs::rename(&making, &copy) {
        let _ = std::fs::remove_file(&making);
        assert!(copy.exists(), "{}: {e}", copy.display());
    }
    copy
}

/// Radix's migrations (`radix/internal/catalogdb/migrations/NNNN_name.sql`) by number.
fn radix_migrations() -> Vec<(i64, PathBuf)> {
    let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../radix/internal/catalogdb/migrations");
    let mut found: Vec<(i64, PathBuf)> = std::fs::read_dir(&migrations)
        .unwrap_or_else(|e| panic!("{}: {e}", migrations.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let number = path.file_name()?.to_str()?.strip_suffix(".sql")?.split_once('_')?.0.parse::<i64>().ok()?;
            Some((number, path))
        })
        .collect();
    found.sort();
    found
}

/// Applies one migration of Radix as Radix does: its SQL and its number, in one transaction.
fn apply(conn: &mut rusqlite::Connection, number: i64, file: &Path) {
    let sql = std::fs::read_to_string(file).unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute_batch(&sql).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    tx.pragma_update(None, "user_version", number).unwrap();
    tx.commit().unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A snapshot of schema 9, as the pinned one is (Radix's first nine migrations and a digest of
    /// its own), is read through a copy that the later ones were applied to: the queries' schema,
    /// the columns they select there and empty, the digest the file's. Asked again, the same copy;
    /// a snapshot of the queries' schema is itself.
    #[test]
    fn an_older_snapshot_is_read_as_the_migrations_leave_it() {
        let unique = format!("{:x}{:08x}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let dir = std::env::temp_dir().join(format!("folia-migrated-test-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("catalog-old.db");
        let mut conn = rusqlite::Connection::open(&old).unwrap();
        for (number, file) in radix_migrations().into_iter().filter(|(number, _)| *number <= 9) {
            apply(&mut conn, number, &file);
        }
        let digest = format!("{unique:0<16}-schema-9");
        conn.execute("INSERT INTO meta (key, value) VALUES ('content_digest', ?1)", [&digest]).unwrap();
        drop(conn);

        let copy = migrated(&old);
        assert_ne!(copy, old);
        let db = NativeDatabase::open(&copy).unwrap();
        assert_eq!(db.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(meta(&db).unwrap().content_digest.as_deref(), Some(digest.as_str()));
        assert_eq!(scalar(&db, "SELECT COUNT(*) FROM v_module_schedule WHERE series IS NOT NULL OR series_weeks IS NOT NULL"), 0);
        assert_eq!(NativeDatabase::open(&old).unwrap().schema_version().unwrap(), 9, "the file stays as it is");
        assert_eq!(migrated(&old), copy);
        assert_eq!(migrated(&copy), copy);
        drop(db);
        let _ = std::fs::remove_file(&copy);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
