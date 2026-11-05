//! Which records a store's text holds, and the text a store gets back from the merged records.
//!
//! The key of a record is what the store itself tells lines apart by (`PlanDoc::restored`,
//! `Shelf::restored`): a module once per semester, planned or visited (`m` and `v` share a key,
//! so the two can never both stand); a placeholder by its pid; a Termin of one's own by its
//! number; a calendar by its number; a passed module once; a note once per module. Where a line
//! holds a set (the kinds a semester hides, the dates hidden of a Termin, the collections of a
//! module), each element is a record of its own, so that two devices adding to one set keep both.
//! A line of a tag this build does not know is one opaque record: kept, never written.
//!
//! `project` never folds two records into one: two collections of one name both stand, the later
//! one as „… (2)“, as „Aus Datei laden“ names a profile. It may leave records out (an eighth
//! calendar, the children of a calendar's earlier life), never merge them.

use std::collections::{BTreeMap, BTreeSet};

use crate::doc::{Doc, Entry};
use crate::hlc::Hlc;

/// A store's records: key to value (a line, or "1" for an element of a set).
pub type Records = BTreeMap<String, String>;

const ONE: &str = "1";

/// As `folia_plans::studyplan`: at most 8 calendars of one's own, numbered 1–99.
pub const MAX_CALENDARS: usize = 8;
pub const MAX_CALENDAR_ID: u32 = 99;
/// As `folia_routes::url::MAX_PID` and `folia_calendar::own::MAX_OWN_NUMBER`.
pub const MAX_PID: u32 = 9_999;
pub const MAX_OWN_NUMBER: u32 = 99_999;
/// Devices of one profile number what they make in classes of their own: device slot `s` takes
/// the numbers `n` with `n % SLOTS == s % SLOTS`, so two devices never make the same number.
pub const SLOTS: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Store {
    /// `betula.studyplan.v1`: the Stundenplan and „Mein Studium“ (`PlanDoc`).
    Plan,
    /// `betula.bookmarks.v1`: the Merkliste.
    Marks,
    /// `betula.collections.v1`: Sammlungen and notes.
    Shelf,
    /// `betula.myprogram.v1`: „Mein Studiengang“.
    Mine,
}

impl Store {
    pub const ALL: [Store; 4] = [Store::Plan, Store::Marks, Store::Shelf, Store::Mine];
}

/// Texts whose two concurrent writes are kept side by side (a note typed on two devices).
pub fn is_text(key: &str) -> bool {
    key.starts_with("C|n|")
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|l| l.trim_end_matches('\r')).filter(|l| !l.is_empty())
}

/// The records of a store's text.
pub fn records(store: Store, text: &str) -> Records {
    let mut r = Records::new();
    match store {
        Store::Plan => plan_records(text, &mut r),
        Store::Marks => {
            for line in lines(text) {
                if let Some((module, _)) = line.split_once('\t') {
                    r.entry(format!("B|{module}")).or_insert_with(|| line.to_string());
                }
            }
        }
        Store::Shelf => shelf_records(text, &mut r),
        Store::Mine => {
            for line in lines(text) {
                if let Some((key, _)) = line.split_once('\t') {
                    r.entry(format!("M|{key}")).or_insert_with(|| line.to_string());
                }
            }
        }
    }
    r
}

fn plan_records(text: &str, r: &mut Records) {
    for line in lines(text) {
        let f: Vec<&str> = line.split('\t').collect();
        let g = |i: usize| f.get(i).copied().unwrap_or("");
        match g(0) {
            "m" => {
                // The first `m` of a semester and module counts, and it outranks a visit.
                let key = format!("P|mv|{}|{}", g(1), g(2));
                if !r.get(&key).is_some_and(|v| v.starts_with("m\t")) {
                    r.insert(key, line.to_string());
                }
            }
            "v" => {
                r.entry(format!("P|mv|{}|{}", g(1), g(2))).or_insert_with(|| line.to_string());
            }
            "p" => {
                r.entry(format!("P|p|{}", g(1))).or_insert_with(|| line.to_string());
            }
            "k" => {
                for kind in g(2).split(',').filter(|k| !k.is_empty()) {
                    r.insert(format!("P|k|{}|{kind}", g(1)), ONE.into());
                }
            }
            "r" => {
                r.insert(format!("P|r|{}|{}", g(1), g(2)), ONE.into());
            }
            "c" => {
                r.insert(format!("P|c|{}|{}", g(1), g(2)), ONE.into());
            }
            "h" => {
                for day in g(3).split(',').filter(|d| !d.is_empty()) {
                    r.insert(format!("P|h|{}|{}|{day}", g(1), g(2)), ONE.into());
                }
            }
            "a" => {
                // The last `a` of a semester counts.
                r.insert(format!("P|a|{}", g(1)), line.to_string());
            }
            "d" => {
                r.entry(format!("P|d|{}", g(2))).or_insert_with(|| line.to_string());
            }
            "l" => {
                r.entry(format!("P|l|{}", g(1))).or_insert_with(|| line.to_string());
            }
            "n" => {
                r.insert(format!("P|n|{}|{}|{}", g(1), g(2), g(3)), ONE.into());
            }
            "t" => {
                r.entry(format!("P|t|{}", g(1))).or_insert_with(|| line.to_string());
            }
            _ => {
                r.insert(format!("P|?|{line}"), ONE.into());
            }
        }
    }
}

fn shelf_records(text: &str, r: &mut Records) {
    let mut order: Vec<String> = Vec::new();
    for line in lines(text) {
        let f: Vec<&str> = line.splitn(3, '\t').collect();
        match f.as_slice() {
            ["c", id, name] => {
                let key = format!("C|c|{id}");
                if !r.contains_key(&key) {
                    r.insert(key, (*name).to_string());
                    order.push((*id).to_string());
                }
            }
            ["d", ids] => {
                for id in ids.split(',').filter(|i| !i.is_empty()) {
                    r.insert(format!("C|d|{id}"), ONE.into());
                }
            }
            ["m", module, ids] => {
                for id in ids.split(',').filter(|i| !i.is_empty()) {
                    r.insert(format!("C|m|{module}|{id}"), ONE.into());
                }
            }
            ["n", module, note] => {
                r.entry(format!("C|n|{module}")).or_insert_with(|| (*note).to_string());
            }
            _ => {}
        }
    }
    if !order.is_empty() {
        r.insert("C|order".into(), order.join(","));
    }
}

/// The text a store gets back from the merged records: the same on every device that holds the
/// same records.
pub fn project(store: Store, doc: &Doc) -> String {
    let out = match store {
        Store::Plan => plan_project(doc),
        Store::Marks => {
            let mut marks: Vec<(std::cmp::Reverse<u64>, &str, &str)> = doc
                .live_with("B|")
                .filter_map(|(k, e)| {
                    let line = e.val.as_deref()?;
                    let secs = line.split('\t').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    Some((std::cmp::Reverse(secs), &k[2..], line))
                })
                .collect();
            marks.sort();
            marks.into_iter().map(|(_, _, line)| line.to_string()).collect()
        }
        Store::Shelf => shelf_project(doc),
        Store::Mine => doc.live_with("M|").filter_map(|(_, e)| e.val.clone()).collect(),
    };
    if out.is_empty() {
        String::new()
    } else {
        out.join("\n") + "\n"
    }
}

#[derive(Default)]
struct Semester<'a> {
    kinds: Vec<&'a str>,
    rows: Vec<&'a str>,
    chosen: Vec<&'a str>,
    dates: BTreeMap<&'a str, Vec<&'a str>>,
    code: Option<&'a str>,
}

fn plan_project(doc: &Doc) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // The calendars, in the order they were made, at most eight; what belongs to one counts
    // only from the beginning of its present life on.
    let mut calendars: Vec<(Hlc, u32, &str)> = doc
        .live_with("P|l|")
        .filter_map(|(k, e)| Some((e.created, k[4..].parse().ok()?, e.val.as_deref()?)))
        .collect();
    calendars.sort();
    calendars.truncate(MAX_CALENDARS);
    let born: BTreeMap<u32, Hlc> = calendars.iter().map(|(c, id, _)| (*id, *c)).collect();
    let belongs = |calendar: u32, e: &Entry| born.get(&calendar).is_some_and(|b| e.created >= *b);

    let mut planned: Vec<(&str, u64, &str, &str)> = doc
        .live_with("P|mv|")
        .filter_map(|(k, e)| {
            let (semester, module) = k[5..].split_once('|')?;
            let line = e.val.as_deref()?;
            let at = line.split('\t').nth(3).and_then(|a| a.parse().ok()).unwrap_or(0);
            Some((semester, at, module, line))
        })
        .collect();
    planned.sort();
    out.extend(planned.iter().filter(|p| p.3.starts_with("m\t")).map(|p| p.3.to_string()));
    out.extend(planned.iter().filter(|p| p.3.starts_with("v\t")).map(|p| p.3.to_string()));

    let mut placeholders: Vec<(u32, &str)> = doc.live_with("P|p|").filter_map(|(k, e)| Some((k[4..].parse().ok()?, e.val.as_deref()?))).collect();
    placeholders.sort();
    out.extend(placeholders.into_iter().map(|(_, line)| line.to_string()));

    let mut passed: Vec<(Hlc, &str)> = doc.live_with("P|d|").filter_map(|(_, e)| Some((e.created, e.val.as_deref()?))).collect();
    passed.sort();
    out.extend(passed.into_iter().map(|(_, line)| line.to_string()));

    let mut semesters: BTreeMap<&str, Semester> = BTreeMap::new();
    for (k, _) in doc.live_with("P|k|") {
        if let Some((s, kind)) = k[4..].split_once('|') {
            semesters.entry(s).or_default().kinds.push(kind);
        }
    }
    for (k, _) in doc.live_with("P|r|") {
        if let Some((s, row)) = k[4..].split_once('|') {
            semesters.entry(s).or_default().rows.push(row);
        }
    }
    for (k, _) in doc.live_with("P|c|") {
        if let Some((s, row)) = k[4..].split_once('|') {
            semesters.entry(s).or_default().chosen.push(row);
        }
    }
    for (k, _) in doc.live_with("P|h|") {
        let mut parts = k[4..].splitn(3, '|');
        if let (Some(s), Some(row), Some(day)) = (parts.next(), parts.next(), parts.next()) {
            semesters.entry(s).or_default().dates.entry(row).or_default().push(day);
        }
    }
    for (k, e) in doc.live_with("P|a|") {
        semesters.entry(&k[4..]).or_default().code = e.val.as_deref();
    }
    for (s, sem) in &semesters {
        if !sem.kinds.is_empty() {
            out.push(format!("k\t{s}\t{}", sem.kinds.join(",")));
        }
        out.extend(sem.rows.iter().map(|row| format!("r\t{s}\t{row}")));
        out.extend(sem.chosen.iter().map(|row| format!("c\t{s}\t{row}")));
        out.extend(sem.dates.iter().map(|(row, days)| format!("h\t{s}\t{row}\t{}", days.join(","))));
        out.extend(sem.code.map(str::to_string));
    }

    let mut own: Vec<(Hlc, u32, &str, &Entry)> = doc
        .live_with("P|t|")
        .filter_map(|(_, e)| {
            let line = e.val.as_deref()?;
            Some((e.created, line.split('\t').nth(2)?.parse().ok()?, line, e))
        })
        .collect();
    own.sort_by_key(|(created, ..)| *created);
    out.extend(own.iter().filter(|o| o.1 == 0).map(|o| o.2.to_string()));
    for (_, id, line) in &calendars {
        out.push((*line).to_string());
        let mut modules: Vec<(Hlc, String)> = doc
            .live_with(&format!("P|n|{id}|"))
            .filter(|(_, e)| belongs(*id, e))
            .filter_map(|(k, e)| {
                let rest = k.splitn(4, '|').nth(3)?;
                let (s, module) = rest.split_once('|')?;
                Some((e.created, format!("n\t{id}\t{s}\t{module}")))
            })
            .collect();
        modules.sort();
        out.extend(modules.into_iter().map(|(_, line)| line));
        out.extend(own.iter().filter(|o| o.1 == *id && belongs(*id, o.3)).map(|o| o.2.to_string()));
    }

    out.extend(doc.live_with("P|?|").map(|(k, _)| k[4..].to_string()));
    out
}

fn shelf_project(doc: &Doc) -> Vec<String> {
    let mut made: Vec<(Hlc, u32, &str)> = doc
        .live_with("C|c|")
        .filter_map(|(k, e)| Some((e.created, k[4..].parse().ok()?, e.val.as_deref()?)))
        .collect();
    made.sort();
    // A name is there once (case and the spaces at its ends do not count): of two collections
    // made on two devices under one name, the later one stands as „Für Lena (2)“.
    let mut taken = BTreeSet::new();
    let mut shown: BTreeMap<u32, String> = BTreeMap::new();
    for (_, id, name) in &made {
        let mut n = 1;
        let mut candidate = (*name).to_string();
        while !taken.insert(candidate.trim().to_lowercase()) {
            n += 1;
            candidate = format!("{name} ({n})");
        }
        shown.insert(*id, candidate);
    }
    let order: Vec<u32> = doc
        .live("C|order")
        .and_then(|e| e.val.as_deref())
        .map(|v| v.split(',').filter_map(|id| id.parse().ok()).collect())
        .unwrap_or_default();
    let place = |id: u32| order.iter().position(|o| *o == id).unwrap_or(usize::MAX);
    let mut shelf = made.clone();
    shelf.sort_by_key(|(created, id, _)| (place(*id), *created));
    let born: BTreeMap<u32, Hlc> = made.iter().map(|(c, id, _)| (*id, *c)).collect();
    let belongs = |id: u32, e: &Entry| born.get(&id).is_some_and(|b| e.created >= *b);
    let at = |id: &u32| shelf.iter().position(|(_, s, _)| s == id).unwrap_or(usize::MAX);

    let mut out: Vec<String> = shelf.iter().map(|(_, id, _)| format!("c\t{id}\t{}", shown[id])).collect();
    let mut standard: Vec<u32> = doc
        .live_with("C|d|")
        .filter_map(|(k, e)| Some((k[4..].parse::<u32>().ok()?, e)))
        .filter(|(id, e)| belongs(*id, e))
        .map(|(id, _)| id)
        .collect();
    standard.sort_by_key(at);
    if !standard.is_empty() {
        out.push(format!("d\t{}", join(&standard)));
    }
    let mut members: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for (k, e) in doc.live_with("C|m|") {
        if let Some((module, id)) = k[4..].split_once('|') {
            if let Ok(id) = id.parse::<u32>() {
                if belongs(id, e) {
                    members.entry(module).or_default().push(id);
                }
            }
        }
    }
    for (module, mut ids) in members {
        ids.sort_by_key(at);
        out.push(format!("m\t{module}\t{}", join(&ids)));
    }
    out.extend(doc.live_with("C|n|").filter_map(|(k, e)| Some(format!("n\t{}\t{}", &k[4..], e.val.as_deref()?))));
    out
}

fn join(ids: &[u32]) -> String {
    ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

/// What a change made of a store's records: the records written, and the ones gone (`None`).
/// The order of the collections counts only when two of them changed places; a collection made
/// or deleted moves nothing.
pub fn diff(before: &Records, after: &Records) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for (k, v) in after {
        if k != "C|order" && before.get(k) != Some(v) {
            out.push((k.clone(), Some(v.clone())));
        }
    }
    for k in before.keys() {
        if k != "C|order" && !after.contains_key(k) {
            out.push((k.clone(), None));
        }
    }
    if let (Some(b), Some(a)) = (before.get("C|order"), after.get("C|order")) {
        let b: Vec<&str> = b.split(',').collect();
        let a: Vec<&str> = a.split(',').collect();
        let kept_b: Vec<&&str> = b.iter().filter(|id| a.contains(id)).collect();
        let kept_a: Vec<&&str> = a.iter().filter(|id| b.contains(id)).collect();
        if kept_b != kept_a {
            out.push(("C|order".into(), Some(a.join(","))));
        }
    }
    out
}
