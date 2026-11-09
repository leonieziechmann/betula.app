//! Devices of one profile with their stores, as the browser holds them: every store reads its
//! text from the records (`project`) and writes it again as it writes it; a change is what the
//! store's own methods make of its state (`PlanDoc`, `SavedPlans`, `MineDoc`; the Merkliste and
//! the Sammlungen as `folia_stores` keeps them, in `marks` and `shelf`).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::rowkey::RowKey;
use folia_calendar::semester::SemesterKey;
use folia_plans::studyplan::{MineDoc, PlanDoc, SavedPlans};
use folia_sync::{project, Applied, Doc, Op, Store, Writer};

/// xorshift64*: the same run for the same seed.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
    pub fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }
}

pub const SEMESTERS: [&str; 3] = ["2026W", "2027S", "2027W"];
pub const MODULES: [&str; 12] = ["11109", "11787", "12101", "12102", "12104", "12107", "12160", "12229", "13977", "13980", "11112", "11903"];
pub const ROWS: [(u32, u32); 3] = [(301_512, 0xa1b2c), (301_517, 0x0f00d), (302_048, 0x77aa1)];
pub const DAYS: [&str; 3] = ["2026-11-04", "2026-11-11", "2026-11-18"];
pub const NAMES: [&str; 4] = ["Lena", "Max", "Sport", "Lerngruppe"];
pub const SHELVES: [&str; 5] = ["Für Lena", "für  lena", "Später", "Wichtig", "Prüfung"];
pub const WORDS: [&str; 10] = ["Altklausur", "lernen", "Gruppe", "Freitag", "wichtig", "Folien", "Tutorium", "mit", "Lena", "Abgabe"];

pub fn key(s: &str) -> SemesterKey {
    SemesterKey::parse(s).unwrap()
}

pub fn row(i: usize) -> RowKey {
    let (event, fp) = ROWS[i % ROWS.len()];
    RowKey { event, fp }
}

/// What a store does with a text it reads: keeps what it can and writes it again.
pub fn held(store: Store, text: &str) -> String {
    match store {
        Store::Plan => PlanDoc::restored(text).stored_without_program(),
        Store::Saved => SavedPlans::restored(text).stored(),
        Store::Mine => MineDoc::restored(text).stored(),
        Store::Marks => marks::Marks::restored(text).stored(),
        Store::Shelf => shelf::Shelf::restored(text).stored(),
    }
}

/// One device of a profile.
#[derive(Clone)]
pub struct Device {
    pub doc: Doc,
    pub writer: Writer,
}

impl Device {
    pub fn new(slot: u16) -> Self {
        let doc = Doc::default();
        Device { writer: Writer::new(slot, &doc), doc }
    }

    /// What a store of this device holds: the text of the records, as the store read it.
    pub fn text(&self, store: Store) -> String {
        held(store, &project(store, &self.doc))
    }

    pub fn plan_doc(&self) -> PlanDoc {
        PlanDoc::restored(&self.text(Store::Plan))
    }

    /// A change of one store's text, `f` doing to the text before what the store does, with the
    /// device's slot and the numbers the profile gave up, as the browser's leader sets them.
    pub fn edit(&mut self, store: Store, wall: u64, f: impl FnOnce(&str) -> String) -> Vec<Op> {
        use folia_plans::ids::{self, Kind};
        ids::set_slot(self.writer.dev());
        let retired = folia_sync::retired(&self.doc);
        for (kind, numbers) in [(Kind::Calendar, retired.calendars), (Kind::Own, retired.own), (Kind::Placeholder, retired.placeholders), (Kind::Collection, retired.collections)] {
            ids::set_retired(kind, numbers);
        }
        let before = self.text(store);
        let after = f(&before);
        ids::set_slot(0);
        ids::clear_retired();
        self.writer.changes(&mut self.doc, store, &before, &after, wall)
    }

    /// A change of the Stundenplan as `PlanDoc` makes it.
    pub fn plan(&mut self, wall: u64, f: impl FnOnce(&mut PlanDoc)) -> Vec<Op> {
        self.edit(Store::Plan, wall, |text| {
            let mut doc = PlanDoc::restored(text);
            f(&mut doc);
            doc.stored_without_program()
        })
    }

    pub fn saved(&mut self, wall: u64, f: impl FnOnce(&mut SavedPlans)) -> Vec<Op> {
        self.edit(Store::Saved, wall, |text| {
            let mut doc = SavedPlans::restored(text);
            f(&mut doc);
            doc.stored()
        })
    }

    pub fn mine(&mut self, wall: u64, f: impl FnOnce(&mut MineDoc)) -> Vec<Op> {
        self.edit(Store::Mine, wall, |text| {
            let mut doc = MineDoc::restored(text);
            f(&mut doc);
            doc.stored()
        })
    }

    pub fn marks(&mut self, wall: u64, f: impl FnOnce(&mut marks::Marks)) -> Vec<Op> {
        self.edit(Store::Marks, wall, |text| {
            let mut doc = marks::Marks::restored(text);
            f(&mut doc);
            doc.stored()
        })
    }

    pub fn shelf(&mut self, wall: u64, f: impl FnOnce(&mut shelf::Shelf)) -> Vec<Op> {
        self.edit(Store::Shelf, wall, |text| {
            let mut doc = shelf::Shelf::restored(text);
            f(&mut doc);
            doc.stored()
        })
    }

    /// Writes of other devices, in any order and as often as they come; how many were new.
    pub fn receive(&mut self, ops: &[Op]) -> usize {
        ops.iter().filter(|op| self.writer.receive(&mut self.doc, op) != Applied::Doubled).count()
    }
}

/// Both see everything the other wrote: in another order, and once more for good measure.
pub fn meet(a: &mut Device, a_ops: &[Op], b: &mut Device, b_ops: &[Op]) {
    b.receive(a_ops);
    let mut reversed = b_ops.to_vec();
    reversed.reverse();
    a.receive(&reversed);
    a.receive(b_ops);
    b.receive(a_ops);
}

pub fn same(a: &Device, b: &Device) {
    assert_eq!(a.doc.entries, b.doc.entries, "the records differ");
    for store in Store::ALL {
        assert_eq!(a.text(store), b.text(store), "{store:?}");
    }
}

/// The Merkliste as `folia_stores::bookmarks` keeps it: `<module>\t<seconds>`, the newest first,
/// every module once.
pub mod marks {
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Marks(pub Vec<(String, u64)>);

    impl Marks {
        pub fn restored(text: &str) -> Self {
            let mut marks: Vec<(String, u64)> = Vec::new();
            for line in text.lines() {
                let (id, at) = line.split_once('\t').unwrap_or((line, ""));
                let id = id.trim();
                if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric()) && !marks.iter().any(|(m, _)| m == id) {
                    marks.push((id.to_string(), at.trim().parse().unwrap_or(0)));
                }
            }
            Marks(marks)
        }

        pub fn stored(&self) -> String {
            self.0.iter().map(|(id, at)| format!("{id}\t{at}\n")).collect()
        }

        pub fn mark(&mut self, id: &str, at: u64) {
            if !self.0.iter().any(|(m, _)| m == id) {
                self.0.insert(0, (id.to_string(), at));
            }
        }

        pub fn unmark(&mut self, id: &str) -> Option<(String, u64)> {
            let at = self.0.iter().position(|(m, _)| m == id)?;
            Some(self.0.remove(at))
        }

        /// „Rückgängig": where its time puts it.
        pub fn put_back(&mut self, mark: (String, u64)) {
            if !self.0.iter().any(|(m, _)| *m == mark.0) {
                let at = self.0.iter().position(|(_, kept)| *kept <= mark.1).unwrap_or(self.0.len());
                self.0.insert(at, mark);
            }
        }
    }
}

/// The Sammlungen as `folia_stores::collections::Shelf` keeps them.
pub mod shelf {
    use super::*;

    pub const MAX_COLLECTIONS: usize = 64;

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Shelf {
        pub collections: Vec<(u32, String)>,
        pub standard: BTreeSet<u32>,
        pub recent: Vec<u32>,
        pub members: BTreeMap<String, BTreeSet<u32>>,
        pub notes: BTreeMap<String, String>,
        /// The shares (`s`) and the lists followed (`f`), as the store keeps them: lines of their
        /// own, kept whole here (`folia_stores::collections` checks their fields).
        pub shared: Vec<String>,
    }

    pub fn clean_name(name: &str) -> String {
        let words: Vec<&str> = name.split(|c: char| c.is_whitespace() || c.is_control()).filter(|word| !word.is_empty()).collect();
        words.join(" ").chars().take(60).collect()
    }

    fn name_key(name: &str) -> String {
        clean_name(name).to_lowercase()
    }

    impl Shelf {
        pub fn find(&self, name: &str) -> Option<u32> {
            self.collections.iter().find(|(_, n)| name_key(n) == name_key(name)).map(|(id, _)| *id)
        }

        fn has(&self, id: u32) -> bool {
            self.collections.iter().any(|(c, _)| *c == id)
        }

        pub fn create(&mut self, name: &str) -> Option<u32> {
            let name = clean_name(name);
            if name.is_empty() || self.find(&name).is_some() || self.collections.len() >= MAX_COLLECTIONS {
                return None;
            }
            let largest = self.collections.iter().map(|(id, _)| *id).chain(self.recent.iter().copied()).max().unwrap_or(0);
            let id = folia_plans::ids::after(folia_plans::ids::Kind::Collection, largest, u32::MAX, |id| !self.has(id) && !self.recent.contains(&id))?;
            self.collections.push((id, name));
            Some(id)
        }

        pub fn delete(&mut self, id: u32) {
            self.collections.retain(|(c, _)| *c != id);
            self.standard.remove(&id);
            self.recent.retain(|r| *r != id);
            for ids in self.members.values_mut() {
                ids.remove(&id);
            }
            self.members.retain(|_, ids| !ids.is_empty());
        }

        pub fn member(&mut self, module: &str, id: u32, on: bool) {
            if !self.has(id) {
                return;
            }
            if on {
                self.members.entry(module.to_string()).or_default().insert(id);
                self.recent.retain(|r| *r != id);
                self.recent.insert(0, id);
                self.recent.truncate(MAX_COLLECTIONS);
            } else if let Some(ids) = self.members.get_mut(module) {
                ids.remove(&id);
                if ids.is_empty() {
                    self.members.remove(module);
                }
            }
        }

        pub fn standard(&mut self, id: u32, on: bool) {
            if on && self.has(id) {
                self.standard.insert(id);
            } else {
                self.standard.remove(&id);
            }
        }

        pub fn note(&mut self, module: &str, text: Option<&str>) {
            match text.map(str::trim).filter(|t| !t.is_empty()) {
                Some(t) => {
                    self.notes.insert(module.to_string(), t.to_string());
                }
                None => {
                    self.notes.remove(module);
                }
            }
        }

        pub fn rename(&mut self, id: u32, name: &str) {
            let name = clean_name(name);
            if name.is_empty() || self.find(&name).is_some_and(|other| other != id) {
                return;
            }
            if let Some((_, n)) = self.collections.iter_mut().find(|(c, _)| *c == id) {
                *n = name;
            }
        }

        /// Two collections change places.
        pub fn swap(&mut self, i: usize, j: usize) {
            if i < self.collections.len() && j < self.collections.len() {
                self.collections.swap(i, j);
            }
        }

        pub fn stored(&self) -> String {
            let list = |ids: &mut dyn Iterator<Item = u32>| ids.map(|id| id.to_string()).collect::<Vec<_>>().join(",");
            let mut out = String::new();
            for (id, name) in &self.collections {
                out.push_str(&format!("c\t{id}\t{name}\n"));
            }
            if !self.standard.is_empty() {
                out.push_str(&format!("d\t{}\n", list(&mut self.standard.iter().copied())));
            }
            if !self.recent.is_empty() {
                out.push_str(&format!("r\t{}\n", list(&mut self.recent.iter().copied())));
            }
            for (module, ids) in &self.members {
                out.push_str(&format!("m\t{module}\t{}\n", list(&mut ids.iter().copied())));
            }
            for (module, note) in &self.notes {
                let escaped = note.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t");
                out.push_str(&format!("n\t{module}\t{escaped}\n"));
            }
            for line in self.shared.iter().filter(|l| l.starts_with("s\t")).chain(self.shared.iter().filter(|l| l.starts_with("f\t"))) {
                out.push_str(line);
                out.push('\n');
            }
            out
        }

        pub fn restored(text: &str) -> Self {
            let mut shelf = Shelf::default();
            let numbers = |field: &str| -> BTreeSet<u32> { field.split(',').filter_map(|n| n.trim().parse::<u32>().ok()).collect() };
            let lines: Vec<Vec<&str>> = text.split('\n').map(|line| line.trim_end_matches('\r').splitn(3, '\t').collect()).collect();
            for fields in &lines {
                if let ["c", id, name] = fields.as_slice() {
                    let (Ok(id), name) = (id.trim().parse::<u32>(), clean_name(name)) else { continue };
                    if id > 0 && !name.is_empty() && !shelf.has(id) && shelf.find(&name).is_none() && shelf.collections.len() < MAX_COLLECTIONS {
                        shelf.collections.push((id, name));
                    }
                }
            }
            let mut standard_seen = false;
            for fields in &lines {
                match fields.as_slice() {
                    ["d", ids] if !standard_seen => {
                        standard_seen = true;
                        shelf.standard = numbers(ids).into_iter().filter(|id| shelf.has(*id)).collect();
                    }
                    ["r", ids] if shelf.recent.is_empty() => {
                        let mut seen = BTreeSet::new();
                        shelf.recent = ids.split(',').filter_map(|n| n.trim().parse::<u32>().ok()).filter(|id| shelf.has(*id) && seen.insert(*id)).collect();
                    }
                    ["s", ..] | ["f", ..] => {
                        // A share of a collection that is gone goes with it; a list followed takes
                        // no collection's number.
                        let line = fields.join("\t");
                        let number = line.split('\t').nth(if fields[0] == "s" { 2 } else { 1 }).and_then(|n| n.parse::<u32>().ok());
                        let fine = match (fields[0], number) {
                            ("s", Some(collection)) => collection == 0 || shelf.has(collection),
                            ("f", Some(id)) => id > 0 && !shelf.has(id),
                            _ => false,
                        };
                        if fine && !shelf.shared.contains(&line) {
                            shelf.shared.push(line);
                        }
                    }
                    ["m", module, ids] if !module.trim().is_empty() && !shelf.members.contains_key(module.trim()) => {
                        let ids: BTreeSet<u32> = numbers(ids).into_iter().filter(|id| shelf.has(*id)).collect();
                        if !ids.is_empty() {
                            shelf.members.insert(module.trim().to_string(), ids);
                        }
                    }
                    ["n", module, note] if !module.trim().is_empty() && !shelf.notes.contains_key(module.trim()) => {
                        let mut text = String::new();
                        let mut chars = note.chars();
                        while let Some(c) = chars.next() {
                            if c == '\\' {
                                match chars.next() {
                                    Some('n') => text.push('\n'),
                                    Some('t') => text.push('\t'),
                                    Some(other) => text.push(other),
                                    None => {}
                                }
                            } else {
                                text.push(c);
                            }
                        }
                        if !text.trim().is_empty() {
                            shelf.notes.insert(module.trim().to_string(), text.trim().to_string());
                        }
                    }
                    _ => {}
                }
            }
            shelf
        }
    }
}
