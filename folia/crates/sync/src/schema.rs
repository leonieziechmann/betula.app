//! Which records a store's text holds, and the text a store gets back from the merged records.
//!
//! The key of a record is what the store itself tells lines apart by (`PlanDoc::restored`,
//! `SavedPlans`, `MineDoc`, the Merkliste's marks, `Shelf::restored`): a module once per semester,
//! planned or visited (`m` and `v` share a key, so the two never both stand, and `m` outranks
//! `v` as it does in the reader); a placeholder by its pid; a Termin of one's own by its number; a
//! calendar by its number; a passed module once; a change of a Termin by its Termin, scope and
//! day; a note once per Termin or module; a saved plan by its name; a field of „Mein Studiengang"
//! by its name. Where a line holds a set (the kinds a semester hides, the dates hidden of a
//! Termin, a module's collections, the collections new marks go into), each element is a record of
//! its own, so two devices adding to one set keep both. A line of a tag this build does not know,
//! or one whose fields could not be a key, is one opaque record: kept as it is, never split.
//!
//! `project` writes the records back in the order the store writes its text: what the store keeps
//! in order (modules within a semester, passed modules, Termine of one's own, calendars and their
//! modules, saved plans, lines of a newer build) by when each record was made, which is the order
//! of the text where the records came from and the end for what is made later, as the stores add
//! it. It never folds two records into one: two collections of one name both stand, the later one
//! as „… (2)", as „Aus Datei laden" names a profile. It leaves out what the store would not keep
//! (a ninth calendar, what belongs to an earlier life of a calendar or a collection).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use crate::doc::{Doc, Entry};
use crate::hlc::Hlc;

/// As `folia_plans::studyplan`: at most 8 calendars of one's own.
pub const MAX_CALENDARS: usize = 8;

/// A collection's name, at most (`folia_stores::collections::MAX_NAME`).
const MAX_SHELF_NAME: usize = 60;

/// The collections a shelf holds, at most (`folia_stores::collections::MAX_COLLECTIONS`).
const MAX_SHELF: usize = 64;

/// The fields of „Mein Studiengang", in the order `MineDoc` writes them.
const MINE_KEYS: [&str; 8] = ["program", "name", "caption", "direction", "start", "leave", "until", "town"];

const ONE: &str = "1";

/// The five stores of a profile (`folia_stores::profiles::KEYS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Store {
    /// The Merkliste.
    Marks,
    /// The Stundenplan with „Mein Studium" (`PlanDoc`).
    Plan,
    /// The saved plans (`SavedPlans`).
    Saved,
    /// „Mein Studiengang" (`MineDoc`).
    Mine,
    /// Sammlungen and notes (`Shelf`).
    Shelf,
}

impl Store {
    pub const ALL: [Store; 5] = [Store::Marks, Store::Plan, Store::Saved, Store::Mine, Store::Shelf];

    /// The store's key in the first profile (another profile's has its number after `betula.`).
    pub fn storage_key(self) -> &'static str {
        match self {
            Store::Marks => "betula.bookmarks.v1",
            Store::Plan => "betula.studyplan.v1",
            Store::Saved => "betula.studyplan.saved.v1",
            Store::Mine => "betula.myprogram.v1",
            Store::Shelf => "betula.collections.v1",
        }
    }

    pub fn of_storage_key(key: &str) -> Option<Store> {
        Store::ALL.into_iter().find(|store| store.storage_key() == key)
    }

    /// What its records' keys begin with.
    pub fn prefix(self) -> &'static str {
        match self {
            Store::Marks => "B|",
            Store::Plan => "P|",
            Store::Saved => "S|",
            Store::Mine => "M|",
            Store::Shelf => "C|",
        }
    }

    /// The store a record belongs to.
    pub fn of_record(key: &str) -> Option<Store> {
        Store::ALL.into_iter().find(|store| key.starts_with(store.prefix()))
    }
}

/// Texts of which two concurrent writes are both kept (a note typed on two devices at once).
pub fn is_text(key: &str) -> bool {
    key.starts_with("C|n|") || key.starts_with("P|b|")
}

/// A store's records in the order of its text: every key once, with its value (a line, or "1"
/// for an element of a set).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Records {
    order: Vec<String>,
    map: BTreeMap<String, String>,
}

impl Records {
    /// The first of a key counts.
    fn first(&mut self, key: String, val: String) {
        if !self.map.contains_key(&key) {
            self.order.push(key.clone());
            self.map.insert(key, val);
        }
    }

    /// The last of a key counts, in the place of its first.
    fn last(&mut self, key: String, val: String) {
        if !self.map.contains_key(&key) {
            self.order.push(key.clone());
        }
        self.map.insert(key, val);
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// In the order of the text.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.order.iter().filter_map(|key| Some((key.as_str(), self.map.get(key)?.as_str())))
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|l| l.trim_end_matches('\r')).filter(|l| !l.trim().is_empty())
}

/// A field that can stand in a key: not empty, no separator of keys, no tab.
fn keyable(field: &str) -> bool {
    !field.is_empty() && !field.contains('|')
}

/// The records of a store's text.
pub fn records(store: Store, text: &str) -> Records {
    let mut r = Records::default();
    match store {
        Store::Marks => {
            for line in lines(text) {
                match line.split_once('\t').map(|(module, _)| module.trim()) {
                    Some(module) if keyable(module) => r.first(format!("B|{module}"), line.to_string()),
                    _ => r.first(format!("B|?|{line}"), ONE.to_string()),
                }
            }
        }
        Store::Plan => plan_records(text, &mut r),
        Store::Saved => saved_records(text, &mut r),
        Store::Mine => {
            for line in lines(text) {
                match line.split_once('\t').map(|(key, _)| key.trim()) {
                    Some(key) if keyable(key) => r.first(format!("M|{key}"), line.to_string()),
                    _ => r.first(format!("M|?|{line}"), ONE.to_string()),
                }
            }
        }
        Store::Shelf => shelf_records(text, &mut r),
    }
    r
}

fn plan_records(text: &str, r: &mut Records) {
    for line in lines(text) {
        let f: Vec<&str> = line.split('\t').map(str::trim).collect();
        let g = |i: usize| f.get(i).copied().unwrap_or_default();
        let all = |fields: &[usize]| fields.iter().all(|i| keyable(g(*i)));
        let opaque = || (format!("P|?|{line}"), ONE.to_string());
        match g(0) {
            "g" => r.first("P|g".into(), line.to_string()),
            "m" if all(&[1, 2]) => {
                // The first `m` of a semester and module counts, and it outranks a visit.
                let key = format!("P|mv|{}|{}", g(1), g(2));
                if !r.get(&key).is_some_and(|v| v.starts_with("m\t")) {
                    r.last(key, line.to_string());
                }
            }
            "v" if all(&[1, 2]) => r.first(format!("P|mv|{}|{}", g(1), g(2)), line.to_string()),
            "p" if all(&[1]) => r.first(format!("P|p|{}", g(1)), line.to_string()),
            "k" if all(&[1]) => {
                for kind in g(2).split(',').map(str::trim).filter(|k| keyable(k)) {
                    r.last(format!("P|k|{}|{kind}", g(1)), ONE.into());
                }
            }
            "e" if all(&[1, 2]) => r.last(format!("P|e|{}|{}", g(1), g(2)), ONE.into()),
            "r" if all(&[1, 2]) => r.last(format!("P|r|{}|{}", g(1), g(2)), ONE.into()),
            "c" if all(&[1, 2]) => r.last(format!("P|c|{}|{}", g(1), g(2)), ONE.into()),
            "h" if all(&[1, 2]) => {
                for day in g(3).split(',').map(str::trim).filter(|d| keyable(d)) {
                    r.last(format!("P|h|{}|{}|{day}", g(1), g(2)), ONE.into());
                }
            }
            "x" if all(&[1, 2, 3, 4]) && !g(5).contains('|') => r.last(format!("P|x|{}|{}|{}|{}|{}", g(1), g(2), g(3), g(4), g(5)), line.to_string()),
            "b" if all(&[1, 2, 3]) => r.last(format!("P|b|{}|{}|{}", g(1), g(2), g(3)), line.to_string()),
            "a" if all(&[1]) => r.last(format!("P|a|{}", g(1)), line.to_string()),
            "d" if all(&[2]) => r.first(format!("P|d|{}", g(2)), line.to_string()),
            "q" if all(&[2, 3]) => r.first(format!("P|q|{}|{}", g(2), g(3)), line.to_string()),
            "l" if all(&[1]) => r.first(format!("P|l|{}", g(1)), line.to_string()),
            "n" if all(&[1, 2, 3]) => r.last(format!("P|n|{}|{}|{}", g(1), g(2), g(3)), ONE.into()),
            "o" if all(&[1, 2, 3]) => r.last(format!("P|o|{}|{}|{}", g(1), g(2), g(3)), ONE.into()),
            "t" if all(&[1]) => r.first(format!("P|t|{}", g(1)), line.to_string()),
            _ => {
                let (key, val) = opaque();
                r.first(key, val);
            }
        }
    }
}

/// A saved plan is one record: its `plan` line and the lines of its plan, by its name.
fn saved_records(text: &str, r: &mut Records) {
    let mut block: Option<(String, String)> = None;
    let finish = |block: Option<(String, String)>, r: &mut Records| {
        if let Some((name, body)) = block {
            r.first(format!("S|{name}"), body);
        }
    };
    for line in lines(text) {
        let mut fields = line.split('\t');
        if fields.next().map(str::trim) == Some("plan") {
            finish(block.take(), r);
            let name = fields.next().unwrap_or_default().trim().to_string();
            block = Some((name, line.to_string()));
        } else if let Some((_, body)) = block.as_mut() {
            body.push('\n');
            body.push_str(line);
        }
    }
    finish(block, r);
}

fn shelf_records(text: &str, r: &mut Records) {
    let mut order: Vec<String> = Vec::new();
    for line in lines(text) {
        let f: Vec<&str> = line.splitn(3, '\t').collect();
        match f.as_slice() {
            ["c", id, name] if keyable(id.trim()) => {
                let key = format!("C|c|{}", id.trim());
                if !r.contains(&key) {
                    order.push(id.trim().to_string());
                }
                r.first(key, (*name).to_string());
            }
            ["d", ids] => {
                for id in ids.split(',').map(str::trim).filter(|i| keyable(i)) {
                    r.last(format!("C|d|{id}"), ONE.into());
                }
            }
            ["r", _] => r.first("C|r".into(), line.to_string()),
            ["m", module, ids] if keyable(module.trim()) => {
                for id in ids.split(',').map(str::trim).filter(|i| keyable(i)) {
                    r.last(format!("C|m|{}|{id}", module.trim()), ONE.into());
                }
            }
            ["n", module, _] if keyable(module.trim()) => r.first(format!("C|n|{}", module.trim()), line.to_string()),
            _ => r.first(format!("C|?|{line}"), ONE.into()),
        }
    }
    if !order.is_empty() {
        r.last("C|order".into(), order.join(","));
    }
}

/// The text a store gets back from the merged records: the same on every device that holds the
/// same records, and read by the store as what it wrote.
pub fn project(store: Store, doc: &Doc) -> String {
    let out = match store {
        Store::Marks => {
            let mut marks: Vec<(Reverse<u64>, Hlc, &str)> = Vec::new();
            for (key, e) in doc.live_with("B|") {
                let Some(line) = e.val.as_deref() else { continue };
                if let Some(opaque) = key.strip_prefix("B|?|") {
                    marks.push((Reverse(0), e.created, opaque));
                } else {
                    let secs = line.split('\t').nth(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
                    marks.push((Reverse(secs), e.created, line));
                }
            }
            marks.sort();
            marks.into_iter().map(|(_, _, line)| line.to_string()).collect()
        }
        Store::Plan => plan_project(doc),
        Store::Saved => {
            let mut saved: Vec<(Hlc, &str)> = doc.live_with("S|").filter_map(|(_, e)| Some((e.created, e.val.as_deref()?))).collect();
            saved.sort();
            saved.into_iter().map(|(_, block)| block.to_string()).collect()
        }
        Store::Mine => {
            let mut fields: Vec<(usize, Hlc, &str)> = Vec::new();
            for (key, e) in doc.live_with("M|") {
                let Some(line) = e.val.as_deref() else { continue };
                let name = key.strip_prefix("M|").unwrap_or_default();
                let place = MINE_KEYS.iter().position(|k| *k == name).unwrap_or(MINE_KEYS.len());
                let line = name.strip_prefix("?|").unwrap_or(line);
                fields.push((place, e.created, line));
            }
            fields.sort();
            fields.into_iter().map(|(_, _, line)| line.to_string()).collect()
        }
        Store::Shelf => shelf_project(doc),
    };
    if out.is_empty() {
        String::new()
    } else {
        out.join("\n") + "\n"
    }
}

/// The part of a key after its prefix, split at its separators.
fn parts<'a>(key: &'a str, prefix: &str) -> Vec<&'a str> {
    key.strip_prefix(prefix).map(|rest| rest.split('|').collect()).unwrap_or_default()
}

#[derive(Default)]
struct Semester<'a> {
    kinds: Vec<&'a str>,
    events: Vec<&'a str>,
    rows: Vec<&'a str>,
    chosen: Vec<&'a str>,
    dates: BTreeMap<&'a str, Vec<&'a str>>,
    changes: Vec<&'a str>,
    notes: Vec<&'a str>,
    code: Option<&'a str>,
}

/// As `PlanDoc::stored`: `g`; `m` by semester, then in planning order; `v` the same; `p` by pid;
/// `d` and `q` in the order they were marked; per semester `k`, `e`, `r`, `c`, `h`, `x`, `b` and
/// `a`; the plan's own Termine; per calendar `l`, its `n`, `o`, `t` and `x`; the rest in the order
/// it came.
fn plan_project(doc: &Doc) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    // The calendars, in the order they were made, at most eight; what belongs to one counts only
    // from the beginning of its present life on.
    let mut calendars: Vec<(Hlc, u32, &str)> = doc
        .live_with("P|l|")
        .filter_map(|(key, e)| Some((e.created, key.strip_prefix("P|l|")?.parse().ok()?, e.val.as_deref()?)))
        .collect();
    calendars.sort();
    calendars.truncate(MAX_CALENDARS);
    let born: BTreeMap<u32, Hlc> = calendars.iter().map(|(created, id, _)| (*id, *created)).collect();
    let belongs = |calendar: u32, e: &Entry| born.get(&calendar).is_some_and(|b| e.created >= *b);

    if let Some(line) = doc.live("P|g").and_then(|e| e.val.as_deref()) {
        out.push(line.to_string());
    }

    // A plan row has one placeholder in a semester (`PlanDoc::add_placeholder`): of two made on
    // two devices at once, the one of the smaller pid stands, and what counted for the other
    // counts for it. A module counts for no placeholder that is gone.
    let mut placeholders: Vec<(u32, &str)> = doc.live_with("P|p|").filter_map(|(key, e)| Some((key.strip_prefix("P|p|")?.parse().ok()?, e.val.as_deref()?))).collect();
    placeholders.sort();
    let mut rows: BTreeMap<(&str, &str, &str), u32> = BTreeMap::new();
    let mut instead: BTreeMap<u32, u32> = BTreeMap::new();
    placeholders.retain(|(pid, line)| {
        let f: Vec<&str> = line.split('\t').map(str::trim).collect();
        let field = |i: usize| f.get(i).copied().unwrap_or_default();
        match rows.get(&(field(2), field(3), field(4))) {
            Some(first) => {
                instead.insert(*pid, *first);
                false
            }
            None => {
                rows.insert((field(2), field(3), field(4)), *pid);
                true
            }
        }
    });
    let standing: BTreeSet<u32> = placeholders.iter().map(|(pid, _)| *pid).collect();
    let fills = |line: &str| -> String {
        let mut f: Vec<String> = line.split('\t').map(str::to_string).collect();
        if let Some(field) = f.get_mut(4) {
            if let Ok(pid) = field.trim().parse::<u32>() {
                let pid = instead.get(&pid).copied().unwrap_or(pid);
                *field = if standing.contains(&pid) { pid.to_string() } else { String::new() };
            }
        }
        f.join("\t")
    };

    let mut planned: Vec<(&str, Hlc, &str)> = doc
        .live_with("P|mv|")
        .filter_map(|(key, e)| Some((*parts(key, "P|mv|").first()?, e.created, e.val.as_deref()?)))
        .collect();
    planned.sort();
    out.extend(planned.iter().filter(|p| p.2.starts_with("m\t")).map(|p| fills(p.2)));
    out.extend(planned.iter().filter(|p| p.2.starts_with("v\t")).map(|p| p.2.to_string()));
    out.extend(placeholders.into_iter().map(|(_, line)| line.to_string()));

    for prefix in ["P|d|", "P|q|"] {
        let mut done: Vec<(Hlc, &str)> = doc.live_with(prefix).filter_map(|(_, e)| Some((e.created, e.val.as_deref()?))).collect();
        done.sort();
        out.extend(done.into_iter().map(|(_, line)| line.to_string()));
    }

    let mut semesters: BTreeMap<&str, Semester> = BTreeMap::new();
    for (key, _) in doc.live_with("P|k|") {
        if let [s, kind] = parts(key, "P|k|").as_slice() {
            semesters.entry(s).or_default().kinds.push(kind);
        }
    }
    for (prefix, pick) in [("P|e|", 0), ("P|r|", 1), ("P|c|", 2)] {
        for (key, _) in doc.live_with(prefix) {
            if let [s, item] = parts(key, prefix).as_slice() {
                let semester = semesters.entry(s).or_default();
                match pick {
                    0 => semester.events.push(item),
                    1 => semester.rows.push(item),
                    _ => semester.chosen.push(item),
                }
            }
        }
    }
    for (key, _) in doc.live_with("P|h|") {
        if let [s, row, day] = parts(key, "P|h|").as_slice() {
            semesters.entry(s).or_default().dates.entry(row).or_default().push(day);
        }
    }
    let mut calendar_changes: BTreeMap<u32, Vec<&str>> = BTreeMap::new();
    for (key, e) in doc.live_with("P|x|") {
        let Some(line) = e.val.as_deref() else { continue };
        if let [calendar, s, ..] = parts(key, "P|x|").as_slice() {
            match calendar.parse::<u32>() {
                Ok(0) => semesters.entry(s).or_default().changes.push(line),
                Ok(id) if belongs(id, e) => calendar_changes.entry(id).or_default().push(line),
                _ => {}
            }
        }
    }
    let mut later: Vec<(Hlc, String)> = Vec::new();
    for (key, e) in doc.live_with("P|b|") {
        let Some(line) = e.val.as_deref() else { continue };
        if let [calendar, s, ..] = parts(key, "P|b|").as_slice() {
            if *calendar == "0" {
                semesters.entry(s).or_default().notes.push(line);
            } else {
                // A note of a calendar's Termin is a line the plan keeps as it came.
                later.push((e.created, line.to_string()));
            }
        }
    }
    for (key, e) in doc.live_with("P|a|") {
        if let Some(s) = key.strip_prefix("P|a|") {
            semesters.entry(s).or_default().code = e.val.as_deref();
        }
    }
    for (s, semester) in &semesters {
        if !semester.kinds.is_empty() {
            out.push(format!("k\t{s}\t{}", semester.kinds.join(",")));
        }
        out.extend(semester.events.iter().map(|event| format!("e\t{s}\t{event}")));
        out.extend(semester.rows.iter().map(|row| format!("r\t{s}\t{row}")));
        out.extend(semester.chosen.iter().map(|row| format!("c\t{s}\t{row}")));
        out.extend(semester.dates.iter().map(|(row, days)| format!("h\t{s}\t{row}\t{}", days.join(","))));
        out.extend(semester.changes.iter().map(|line| line.to_string()));
        out.extend(semester.notes.iter().map(|line| line.to_string()));
        out.extend(semester.code.map(str::to_string));
    }

    let mut own: Vec<(Hlc, u32, &str, &Entry)> = doc
        .live_with("P|t|")
        .filter_map(|(_, e)| {
            let line = e.val.as_deref()?;
            Some((e.created, line.split('\t').nth(2)?.trim().parse().ok()?, line, e))
        })
        .collect();
    own.sort_by_key(|(created, ..)| *created);
    out.extend(own.iter().filter(|o| o.1 == 0).map(|o| o.2.to_string()));

    for (_, id, line) in &calendars {
        out.push((*line).to_string());
        let mut modules: Vec<(Hlc, String)> = doc
            .live_with(&format!("P|n|{id}|"))
            .filter(|(_, e)| belongs(*id, e))
            .filter_map(|(key, e)| match parts(key, "P|n|").as_slice() {
                [_, s, module] => Some((e.created, format!("n\t{id}\t{s}\t{module}"))),
                _ => None,
            })
            .collect();
        modules.sort();
        out.extend(modules.into_iter().map(|(_, line)| line));
        for (key, e) in doc.live_with(&format!("P|o|{id}|")) {
            if let ([_, s, row], true) = (parts(key, "P|o|").as_slice(), belongs(*id, e)) {
                out.push(format!("o\t{id}\t{s}\t{row}"));
            }
        }
        out.extend(own.iter().filter(|o| o.1 == *id && belongs(*id, o.3)).map(|o| o.2.to_string()));
        out.extend(calendar_changes.get(id).into_iter().flatten().map(|line| line.to_string()));
    }

    later.extend(doc.live_with("P|?|").filter_map(|(key, e)| Some((e.created, key.strip_prefix("P|?|")?.to_string()))));
    later.sort();
    out.extend(later.into_iter().map(|(_, line)| line));
    out
}

/// A collection's name as the shelf keeps it (`folia_stores::collections::clean_name`): its words
/// with one space between them, at most `MAX_SHELF_NAME` characters.
fn shelf_name(name: &str) -> String {
    let words: Vec<&str> = name.split(|c: char| c.is_whitespace() || c.is_control()).filter(|word| !word.is_empty()).collect();
    words.join(" ").chars().take(MAX_SHELF_NAME).collect()
}

/// As `Shelf::stored`: the collections in their order, `d`, `r`, `m` by module, `n` by module; the
/// numbers of a set in their order.
fn shelf_project(doc: &Doc) -> Vec<String> {
    let mut made: Vec<(Hlc, u32, &str)> = doc
        .live_with("C|c|")
        .filter_map(|(key, e)| Some((e.created, key.strip_prefix("C|c|")?.parse().ok()?, e.val.as_deref()?)))
        .collect();
    made.sort();
    // A name is there once (`Shelf::find`: neither case nor spaces count): of two collections
    // made on two devices under one name, the later one stands as „Für Lena (2)".
    let mut taken = BTreeSet::new();
    let mut shown: BTreeMap<u32, String> = BTreeMap::new();
    for (_, id, name) in &made {
        let name = shelf_name(name);
        let mut n = 1;
        let mut candidate = name.clone();
        while !taken.insert(candidate.to_lowercase()) {
            n += 1;
            let suffix = format!(" ({n})");
            let room = MAX_SHELF_NAME.saturating_sub(suffix.chars().count());
            candidate = shelf_name(&format!("{}{suffix}", name.chars().take(room).collect::<String>()));
        }
        shown.insert(*id, candidate);
    }
    let order: Vec<u32> = doc
        .live("C|order")
        .and_then(|e| e.val.as_deref())
        .map(|v| v.split(',').filter_map(|id| id.trim().parse().ok()).collect())
        .unwrap_or_default();
    let place = |id: u32| order.iter().position(|o| *o == id).unwrap_or(usize::MAX);
    let mut shelf = made.clone();
    shelf.sort_by_key(|(created, id, _)| (place(*id), *created));
    // The shelf reads the first 64; what names the others, or a collection that is gone, it
    // leaves out, and so does the text.
    shelf.truncate(MAX_SHELF);
    let born: BTreeMap<u32, Hlc> = shelf.iter().map(|(created, id, _)| (*id, *created)).collect();
    let belongs = |id: u32, e: &Entry| born.get(&id).is_some_and(|b| e.created >= *b);
    let join = |ids: &[u32]| ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");

    let mut out: Vec<String> = shelf.iter().filter_map(|(_, id, _)| Some(format!("c\t{id}\t{}", shown.get(id)?))).collect();
    let mut standard: Vec<u32> = doc
        .live_with("C|d|")
        .filter_map(|(key, e)| Some((key.strip_prefix("C|d|")?.parse::<u32>().ok()?, e)))
        .filter(|(id, e)| belongs(*id, e))
        .map(|(id, _)| id)
        .collect();
    standard.sort_unstable();
    if !standard.is_empty() {
        out.push(format!("d\t{}", join(&standard)));
    }
    if let Some((_, ids)) = doc.live("C|r").and_then(|e| e.val.as_deref()).and_then(|line| line.split_once('\t')) {
        let mut seen = BTreeSet::new();
        let recent: Vec<u32> = ids.split(',').filter_map(|id| id.trim().parse().ok()).filter(|id| born.contains_key(id) && seen.insert(*id)).collect();
        if !recent.is_empty() {
            out.push(format!("r\t{}", join(&recent)));
        }
    }
    let mut members: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for (key, e) in doc.live_with("C|m|") {
        if let [module, id] = parts(key, "C|m|").as_slice() {
            if let Ok(id) = id.parse::<u32>() {
                if belongs(id, e) {
                    members.entry(module).or_default().push(id);
                }
            }
        }
    }
    for (module, mut ids) in members {
        ids.sort_unstable();
        out.push(format!("m\t{module}\t{}", join(&ids)));
    }
    out.extend(doc.live_with("C|n|").filter_map(|(_, e)| e.val.clone()));
    let mut rest: Vec<(Hlc, &str)> = doc.live_with("C|?|").filter_map(|(key, e)| Some((e.created, key.strip_prefix("C|?|")?))).collect();
    rest.sort();
    out.extend(rest.into_iter().map(|(_, line)| line.to_string()));
    out
}

/// What a change made of a store's records: the records written, in the order of the text after
/// it, and the ones gone. The order of the collections counts only when two of them changed
/// places; a collection made or deleted moves nothing.
pub fn diff(before: &Records, after: &Records) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for (key, val) in after.iter() {
        if key != "C|order" && before.get(key) != Some(val) {
            out.push((key.to_string(), Some(val.to_string())));
        }
    }
    for (key, _) in before.iter() {
        if key != "C|order" && !after.contains(key) {
            out.push((key.to_string(), None));
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
