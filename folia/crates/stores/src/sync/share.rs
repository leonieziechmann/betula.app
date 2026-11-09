//! Calendars shared that stay current, and one-time links (Betula Sync, step 3;
//! folia/design/teilen/teilen.html, owner 2026-11-08): the links they travel as and what opening
//! one finds. The browser half (`browser`) seals the parts and the parcels, sends them to Phloem,
//! renews the parts of a profile's shares and follows the shares of others.
//!
//! A link's part after `#` never reaches a server: `/studyplan#live=<id ‖ key>` names a part and
//! the key that opens it, `/studyplan#once=<id ‖ key>` a parcel that opens once within 7 days. The
//! Merkliste's links are the same under `/bookmarks` (a list or one collection of it).

use folia_plans::shares::{is_b64, LINK_BYTES};

/// How long a one-time link waits at most (owner, 2026-11-08: „einmal link ist gut mit 7 tage max
/// time"), in seconds.
pub const ONCE_TTL: u64 = 7 * 24 * 3600;

/// The fragment's names.
const LIVE: &str = "live=";
const ONCE: &str = "once=";

/// What the part of a link after `#` names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opening {
    /// A part that stays current: its id and key, as one text (`folia_plans::shares::split_link`).
    Live(String),
    /// A one-time link's parcel: its id and key, as one text.
    Once(String),
}

/// What the fragment of a page's address names (with or without its `#`); `None` for anything
/// else.
pub fn opening(fragment: &str) -> Option<Opening> {
    let fragment = fragment.trim().trim_start_matches('#');
    if let Some(link) = fragment.strip_prefix(LIVE).filter(|link| is_b64(link, LINK_BYTES)) {
        return Some(Opening::Live(link.to_string()));
    }
    fragment.strip_prefix(ONCE).filter(|link| is_b64(link, LINK_BYTES)).map(|link| Opening::Once(link.to_string()))
}

/// The path of the Stundenplan that opens a part that stays current.
pub fn live_path(link: &str) -> String {
    format!("/studyplan#{LIVE}{link}")
}

/// The path of the Stundenplan that opens a one-time link.
pub fn once_path(parcel: &str) -> String {
    format!("/studyplan#{ONCE}{parcel}")
}

/// The path of the Merkliste that opens a list that stays current.
pub fn list_live_path(link: &str) -> String {
    format!("/bookmarks#{LIVE}{link}")
}

/// The path of the Merkliste that opens a one-time link of a list.
pub fn list_once_path(parcel: &str) -> String {
    format!("/bookmarks#{ONCE}{parcel}")
}

/// Why sharing, or opening what was shared, did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareProblem {
    /// The profile does not sync: what stays current, and a one-time link, need Betula Sync.
    NotSynced,
    /// A one-time link was opened already.
    Taken,
    /// A one-time link waited 7 days and nobody opened it.
    Expired,
    /// A one-time link was taken back before anybody opened it.
    Withdrawn,
    /// A link that opens nothing: mistyped, or what it named is forgotten.
    Unknown,
    /// What the link named is not shared any more.
    Ended,
    /// The profile shares as many calendars as it may (32).
    TooMany,
    /// The service takes nothing new now (its budget), or too much at once.
    Busy,
    /// The service could not be reached.
    Unreachable,
    /// This browser cannot (no Web Crypto).
    Unsupported,
}

/// What a parcel's missing says (`GET /sync/v1/pair`, 404 with the code of what became of it).
pub fn missing(code: &str) -> ShareProblem {
    match code {
        "taken" => ShareProblem::Taken,
        "expired" => ShareProblem::Expired,
        "withdrawn" => ShareProblem::Withdrawn,
        _ => ShareProblem::Unknown,
    }
}

/// A short, stable hash of a part's code: what the device last put of a share, to know whether
/// the calendar changed since (FNV-1a, 64 bits).
pub fn fingerprint(code: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in code.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// What this device put of each share of a profile, and the parts it deleted: kept beside the
/// profile's records (`betula.sync.p<id>.parts`), never synced. A share whose code is the one put
/// is not put again; a share gone from the records has its part deleted once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Put {
    /// Part id → the fingerprint of the code put and its version.
    pub parts: std::collections::BTreeMap<String, (String, u64)>,
    /// Part ids deleted.
    pub deleted: std::collections::BTreeSet<String>,
}

impl Put {
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (id, (print, rev)) in &self.parts {
            out.push_str(&format!("p\t{id}\t{print}\t{rev}\n"));
        }
        for id in &self.deleted {
            out.push_str(&format!("d\t{id}\n"));
        }
        out
    }

    pub fn read(text: &str) -> Put {
        let mut put = Put::default();
        for line in text.lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            match fields.as_slice() {
                ["p", id, print, rev] => {
                    put.parts.insert((*id).to_string(), ((*print).to_string(), rev.parse().unwrap_or(0)));
                }
                ["d", id] => {
                    put.deleted.insert((*id).to_string());
                }
                _ => {}
            }
        }
        put
    }

    /// Forgets the parts deleted long enough not to come up any more: at most 256 are kept.
    pub fn trim(&mut self) {
        while self.deleted.len() > 256 {
            let first = self.deleted.iter().next().cloned();
            if let Some(first) = first {
                self.deleted.remove(&first);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link() -> String {
        folia_plans::shares::encode(&[7u8; LINK_BYTES])
    }

    #[test]
    fn a_fragment_names_a_part_or_a_parcel() {
        assert_eq!(opening(&format!("#live={}", link())), Some(Opening::Live(link())));
        assert_eq!(opening(&format!("once={}", link())), Some(Opening::Once(link())));
        for wrong in ["", "#", "#live=", "#live=abc", "#share=x", &format!("#live={}x", link()), &format!("#other={}", link())] {
            assert_eq!(opening(wrong), None, "{wrong}");
        }
        assert_eq!(live_path(&link()), format!("/studyplan#live={}", link()));
        assert_eq!(opening(&once_path(&link()).replace("/studyplan", "")), Some(Opening::Once(link())));
        assert_eq!(list_live_path(&link()), format!("/bookmarks#live={}", link()));
        assert_eq!(opening(&list_once_path(&link()).replace("/bookmarks", "")), Some(Opening::Once(link())));
    }

    #[test]
    fn what_was_put_is_kept_and_read_back() {
        let mut put = Put::default();
        put.parts.insert("cA".into(), (fingerprint("abc"), 3));
        put.deleted.insert("cQ".into());
        assert_eq!(Put::read(&put.text()), put);
        assert_ne!(fingerprint("abc"), fingerprint("abd"));
        assert_eq!(fingerprint("abc").len(), 16);
        for n in 0..300 {
            put.deleted.insert(format!("x{n:03}"));
        }
        put.trim();
        assert_eq!(put.deleted.len(), 256);
    }

    #[test]
    fn a_parcel_says_what_became_of_it() {
        assert_eq!(missing("taken"), ShareProblem::Taken);
        assert_eq!(missing("expired"), ShareProblem::Expired);
        assert_eq!(missing("withdrawn"), ShareProblem::Withdrawn);
        assert_eq!(missing("not_found"), ShareProblem::Unknown);
    }
}
