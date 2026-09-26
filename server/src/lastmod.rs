//! When each page of the sitemap last said something new: its `<lastmod>`, so that a search engine
//! fetches again what changed (exam dates the BTU publishes in the middle of a semester) and not
//! the thousands of pages that did not.
//!
//! The snapshot has no date per page, but the warm-up (`warm`) renders every page of the sitemap
//! once per snapshot. Folia keeps, per page, a fingerprint of what it says (`fingerprint`) and
//! when that last changed: the `data_changed_at` of the snapshot it changed with (the time of
//! Radix's build that found the catalog changed, the same for every instance, so the two colours
//! of a site agree). A page seen for the first time counts as changed then. The record lives in
//! the data directory (`lastmod.json`), so a restart or a deploy leaves it as it is, and it is
//! written after each round of the warm-up. Without the warm-up (`--warm-cache off`) nothing is
//! recorded, and the sitemap names no dates: a stale date is worse than none.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use catalog::timetable::day::Day;
use serde::{Deserialize, Serialize};

/// The record's file in the data directory.
pub const FILE: &str = "lastmod.json";

/// The line of a module's page that says when its source was fetched.
const SOURCE: &str = "<p class=\"source\"";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Seen {
    /// What the page said (`fingerprint`).
    fingerprint: String,
    /// Since when: RFC 3339, as Radix writes `data_changed_at`.
    since: String,
}

pub struct Changes {
    file: PathBuf,
    pages: Mutex<HashMap<String, Seen>>,
    /// Counts the finished rounds, so that the sitemap is made anew after each (`api::sitemap`).
    rounds: AtomicU64,
}

impl Changes {
    /// The record kept in `data_dir`; empty where there is none or it cannot be read.
    pub fn load(data_dir: &Path) -> Self {
        let file = data_dir.join(FILE);
        let pages = std::fs::read(&file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        Self { file, pages: Mutex::new(pages), rounds: AtomicU64::new(0) }
    }

    /// Notes what the page at `path` says now (its HTML), as of `since` (the `data_changed_at` of the
    /// snapshot it was rendered from). True when that is new; a `since` that names no day is not
    /// taken.
    pub fn note(&self, path: &str, html: &[u8], since: &str) -> bool {
        let (Some(fingerprint), Some(_)) = (fingerprint(html), since.get(..10).and_then(Day::parse)) else { return false };
        let Ok(mut pages) = self.pages.lock() else { return false };
        match pages.get(path) {
            Some(seen) if seen.fingerprint == fingerprint => false,
            _ => {
                pages.insert(path.to_string(), Seen { fingerprint, since: since.to_string() });
                true
            }
        }
    }

    /// When the page at `path` last changed, if it was ever seen.
    pub fn since(&self, path: &str) -> Option<String> {
        self.pages.lock().ok()?.get(path).map(|seen| seen.since.clone())
    }

    /// How many rounds have finished since the start.
    pub fn rounds(&self) -> u64 {
        self.rounds.load(Ordering::Relaxed)
    }

    /// Ends a round of the warm-up over `paths`, the whole sitemap: forgets the pages that left it,
    /// lets the sitemap be made anew and writes the record (whole or not at all: a new file is
    /// renamed over the old one).
    pub fn finish(&self, paths: &[String]) -> std::io::Result<()> {
        let json = {
            let Ok(mut pages) = self.pages.lock() else { return Ok(()) };
            let listed: HashSet<&str> = paths.iter().map(String::as_str).collect();
            pages.retain(|path, _| listed.contains(path.as_str()));
            serde_json::to_vec(&*pages).map_err(std::io::Error::other)?
        };
        self.rounds.fetch_add(1, Ordering::Relaxed);
        let written = self.file.with_extension("json.new");
        std::fs::write(&written, json)?;
        std::fs::rename(&written, &self.file)
    }
}

/// What a page says, to tell whether it changed: the inside of its `<main>` (FNV-1a, as hex),
/// without what changes although the page says nothing new: the build in the addresses of its
/// icons and files (`?v=<build>`, new with every start) and the line of when its source was fetched
/// (`<p class="source">`, new whenever Radix fetches the module description again). `None` for
/// what has no `<main>`.
pub fn fingerprint(html: &[u8]) -> Option<String> {
    let html = std::str::from_utf8(html).ok()?;
    let start = html.find("<main")?;
    let main = html.get(start..)?;
    let mut rest = main.get(..main.find("</main>")?)?;
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut feed = |text: &str| {
        for byte in text.bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    loop {
        let Some(at) = [rest.find("?v="), rest.find(SOURCE)].into_iter().flatten().min() else {
            feed(rest);
            break;
        };
        let (before, from) = rest.split_at_checked(at)?;
        feed(before);
        let skip = match from.strip_prefix("?v=") {
            // To the end of the build: the end of the address or of its query.
            Some(build) => "?v=".len() + build.find(|c: char| matches!(c, '"' | '\'' | '#' | '&' | '>') || c.is_whitespace()).unwrap_or(build.len()),
            None => from.find("</p>").map_or(from.len(), |end| end + "</p>".len()),
        };
        rest = from.get(skip..)?;
    }
    Some(format!("{hash:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module's page as the server writes it, with this build and these dates.
    fn page(build: &str, fetched: &str, exam: &str) -> String {
        format!(
            "<!DOCTYPE html><html><head><link rel=\"stylesheet\" href=\"/assets/app.css?v={build}\"><script nonce=\"{build}\"></script></head>\
             <body><main id=\"content\" class=\"content\"><h2>Analysis I</h2><use href=\"/assets/icons.svg?v={build}#info\"/>\
             <div id=\"pruefungstermine\"><time datetime=\"{exam}\">{exam}</time></div>\
             <p class=\"source\"><use href=\"/assets/icons.svg?v={build}#shield-check\"/>Quelle: Modulbeschreibung der BTU · abgerufen {fetched}<a href=\"x\">Original</a></p></main>\
             <footer>{build}</footer></body></html>"
        )
    }

    #[test]
    fn a_page_changes_with_what_it_says_not_with_the_build_or_the_fetch() {
        let first = fingerprint(page("0.2.2-1", "21.09.2026", "2027-02-15").as_bytes());
        assert!(first.is_some());
        // Another build, another fetch of the same description: the same page.
        assert_eq!(fingerprint(page("0.2.3-7", "26.09.2026", "2027-02-15").as_bytes()), first);
        // Another exam date: a new page.
        assert_ne!(fingerprint(page("0.2.2-1", "21.09.2026", "2027-02-16").as_bytes()), first);
        assert_eq!(fingerprint(b"<html><body>no main</body></html>"), None);
    }

    #[test]
    fn the_record_keeps_the_time_of_the_last_change_across_starts() {
        let dir = std::env::temp_dir().join(format!("folia-lastmod-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let paths = vec!["/catalog/module/11101".to_string(), "/catalog/module/11102".to_string()];
        let changes = Changes::load(&dir);
        assert!(changes.note(&paths[0], page("a", "21.09.2026", "2027-02-15").as_bytes(), "2026-09-21T10:00:00Z"));
        assert!(changes.note(&paths[1], page("a", "21.09.2026", "2027-03-01").as_bytes(), "2026-09-21T10:00:00Z"));
        // Not a day: not taken.
        assert!(!changes.note("/catalog/module/99999", page("a", "21.09.2026", "2027-03-01").as_bytes(), "unknown"));
        changes.finish(&paths).unwrap();
        assert_eq!(changes.rounds(), 1);

        // After a restart, with a newer snapshot: the page that says the same keeps its date,
        // the one with a new exam date gets the snapshot's.
        let again = Changes::load(&dir);
        assert!(!again.note(&paths[0], page("b", "26.09.2026", "2027-02-15").as_bytes(), "2026-09-26T08:00:00Z"));
        assert!(again.note(&paths[1], page("b", "26.09.2026", "2027-03-02").as_bytes(), "2026-09-26T08:00:00Z"));
        assert_eq!(again.since(&paths[0]).as_deref(), Some("2026-09-21T10:00:00Z"));
        assert_eq!(again.since(&paths[1]).as_deref(), Some("2026-09-26T08:00:00Z"));
        // A page that left the sitemap is forgotten.
        again.finish(&paths[..1]).unwrap();
        assert_eq!((Changes::load(&dir).since(&paths[0]).is_some(), Changes::load(&dir).since(&paths[1])), (true, None));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
