//! Where the time of a catalog page goes: every query `pages::catalog` runs for the addresses of a
//! pages file (`betula-load discover`), timed by name.
//!
//! ```text
//! cargo run --release -p folia-pages --example loadtest_profile -- <catalog-*.db> <pages.tsv> [N]
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use folia_model::db::{Database, DbError, Rows, Value};
use folia_model::native::NativeDatabase;
use folia_pages as pages;
use folia_routes::url::CatalogUrl;

struct Timed<'a> {
    inner: &'a NativeDatabase,
    spent: RefCell<HashMap<&'static str, (u32, Duration)>>,
}

impl Database for Timed<'_> {
    fn query(&self, name: &'static str, sql: &str, params: &[Value]) -> Result<Rows, DbError> {
        let started = Instant::now();
        let rows = self.inner.query(name, sql, params);
        let mut spent = self.spent.borrow_mut();
        let entry = spent.entry(name).or_default();
        entry.0 += 1;
        entry.1 += started.elapsed();
        rows
    }
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(db), Some(pages)) = (args.first(), args.get(1)) else {
        eprintln!("usage: loadtest_profile <catalog-*.db> <pages.tsv> [N]");
        return std::process::ExitCode::FAILURE;
    };
    let limit = args.get(2).and_then(|n| n.parse::<usize>().ok()).unwrap_or(500);
    let Ok(native) = NativeDatabase::open(&PathBuf::from(db)) else {
        eprintln!("cannot open {db}");
        return std::process::ExitCode::FAILURE;
    };
    let timed = Timed { inner: &native, spent: RefCell::default() };
    let text = std::fs::read_to_string(pages).unwrap_or_default();
    let urls: Vec<&str> = text.lines().filter_map(|line| line.split('\t').next()).filter(|path| path.starts_with("/catalog?") || *path == "/catalog").take(limit).collect();

    let started = Instant::now();
    let mut failed = 0;
    let mut slowest: Vec<(Duration, &str)> = Vec::new();
    for url in &urls {
        let query = url.split_once('?').map(|(_, query)| query).unwrap_or("");
        let one = Instant::now();
        let parsed = CatalogUrl::parse(query);
        // What the page component asks for besides the list (folia/crates/catalog/src/catalog.rs): the
        // summary of the filter panel, the choices of its pickers, the meta row.
        if pages::catalog(&timed, &parsed, folia_locale::Locale::De).is_err() || pages::catalog_summary(&timed, &parsed.query).is_err() || pages::catalog_choices(&timed).is_err() || folia_query::meta(&timed).is_err() {
            failed += 1;
        }
        slowest.push((one.elapsed(), url));
    }
    let total = started.elapsed();
    let mut spent: Vec<(&'static str, (u32, Duration))> = timed.spent.borrow().iter().map(|(k, v)| (*k, *v)).collect();
    spent.sort_by_key(|b| std::cmp::Reverse(b.1 .1));
    println!("{} catalog pages, {failed} failed: {:.2} ms per page on average", urls.len(), total.as_secs_f64() * 1000.0 / urls.len().max(1) as f64);
    for (name, (n, time)) in spent {
        println!("  {name:<32} {n:>6} calls  {:>8.3} ms per call  {:>7.2} ms per page", time.as_secs_f64() * 1000.0 / n.max(1) as f64, time.as_secs_f64() * 1000.0 / urls.len().max(1) as f64);
    }
    slowest.sort_by_key(|b| std::cmp::Reverse(b.0));
    for (time, url) in slowest.iter().take(8) {
        println!("  slow: {:>7.2} ms  {url}", time.as_secs_f64() * 1000.0);
    }
    std::process::ExitCode::SUCCESS
}
