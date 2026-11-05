//! What a person does in Betula, as the stores do it to their records, and a small random
//! source for the simulation.
#![allow(dead_code)]

use sync_merge_proto::schema::{Records, MAX_CALENDARS};

/// xorshift64*: enough for a simulation, the same run for the same seed.
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
        (self.next() % n as u64) as usize
    }
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

pub const SEMESTERS: [&str; 3] = ["2026W", "2027S", "2027W"];
pub const MODULES: [&str; 12] = ["11109", "11787", "12101", "12102", "12104", "12107", "12160", "12229", "13977", "13980", "11112", "11903"];
pub const KINDS: [&str; 4] = ["vl", "ue", "tut", "sem"];
pub const ROWS: [&str; 3] = ["301512-a1b2c", "301517-0f00d", "302048-77aa1"];
pub const DAYS: [&str; 3] = ["2026-11-04", "2026-11-11", "2026-11-18"];
pub const NAMES: [&str; 4] = ["Lena", "Max", "Sport", "Lerngruppe"];
pub const SHELVES: [&str; 5] = ["Für Lena", "für lena", "Später", "Wichtig", "Prüfung"];
pub const WORDS: [&str; 10] = ["Altklausur", "lernen", "Gruppe", "Freitag", "wichtig", "Folien", "Tutorium", "mit", "Lena", "Abgabe"];

// ---- the Stundenplan (`PlanDoc`) ----

pub fn mv(s: &str, module: &str) -> String {
    format!("P|mv|{s}|{module}")
}

pub fn plan(r: &mut Records, s: &str, module: &str, at: u64) {
    let key = mv(s, module);
    if !r.get(&key).is_some_and(|l| l.starts_with("m\t")) {
        r.insert(key, format!("m\t{s}\t{module}\t{at}\t\t"));
    }
}

pub fn visit(r: &mut Records, s: &str, module: &str, at: u64) {
    r.entry(mv(s, module)).or_insert_with(|| format!("v\t{s}\t{module}\t{at}"));
}

pub fn unplan(r: &mut Records, s: &str, module: &str) {
    r.remove(&mv(s, module));
}

/// `PlanDoc::move_to`: with when it was planned and what it fills.
pub fn move_to(r: &mut Records, from: &str, to: &str, module: &str) {
    let Some(line) = r.get(&mv(from, module)).filter(|l| l.starts_with("m\t")).cloned() else { return };
    if from == to || r.get(&mv(to, module)).is_some_and(|l| l.starts_with("m\t")) {
        return;
    }
    let f: Vec<&str> = line.split('\t').collect();
    let g = |i: usize| f.get(i).copied().unwrap_or("");
    r.remove(&mv(from, module));
    r.insert(mv(to, module), format!("m\t{to}\t{module}\t{}\t{}\t{}", g(3), g(4), g(5)));
}

pub fn planned(r: &Records) -> Vec<(String, String)> {
    r.iter()
        .filter(|(_, l)| l.starts_with("m\t"))
        .filter_map(|(_, l)| {
            let f: Vec<&str> = l.split('\t').collect();
            Some((f.get(1)?.to_string(), f.get(2)?.to_string()))
        })
        .collect()
}

pub fn toggle(r: &mut Records, key: String, on: bool) {
    if on {
        r.insert(key, "1".into());
    } else {
        r.remove(&key);
    }
}

pub fn pass(r: &mut Records, s: &str, module: &str) {
    r.insert(format!("P|d|{module}"), format!("d\t{s}\t{module}"));
}

pub fn new_placeholder(r: &mut Records, pid: u32, s: &str, ord: u32) {
    r.insert(format!("P|p|{pid}"), format!("p\t{pid}\t{s}\t079-82-2008\t{ord}\t1-1\t6\telective\t\tWahlpflichtmodul"));
}

/// Which placeholder a planned module counts for (`PlanDoc::set_fills`).
pub fn fill(r: &mut Records, s: &str, module: &str, pid: u32) {
    if let Some(line) = r.get(&mv(s, module)).filter(|l| l.starts_with("m\t")).cloned() {
        let mut f: Vec<String> = line.split('\t').map(str::to_string).collect();
        while f.len() < 6 {
            f.push(String::new());
        }
        f[4] = pid.to_string();
        r.insert(mv(s, module), f.join("\t"));
    }
}

pub fn calendars(r: &Records) -> Vec<u32> {
    r.keys().filter_map(|k| k.strip_prefix("P|l|")?.parse().ok()).collect()
}

pub fn new_calendar(r: &mut Records, id: u32, name: &str) -> bool {
    if calendars(r).len() >= MAX_CALENDARS {
        return false;
    }
    r.insert(format!("P|l|{id}"), format!("l\t{id}\tstrip\t\t{name}"));
    true
}

pub fn rename_calendar(r: &mut Records, id: u32, name: &str) {
    if r.contains_key(&format!("P|l|{id}")) {
        r.insert(format!("P|l|{id}"), format!("l\t{id}\tstrip\t\t{name}"));
    }
}

/// A calendar goes with its modules and its Termine.
pub fn delete_calendar(r: &mut Records, id: u32) {
    r.remove(&format!("P|l|{id}"));
    let children = format!("P|n|{id}|");
    r.retain(|k, v| !k.starts_with(&children) && !(k.starts_with("P|t|") && v.split('\t').nth(2) == Some(&id.to_string())));
}

pub fn calendar_module(r: &mut Records, id: u32, s: &str, module: &str, on: bool) {
    if r.contains_key(&format!("P|l|{id}")) {
        toggle(r, format!("P|n|{id}|{s}|{module}"), on);
    }
}

pub fn own(r: &mut Records, number: u32, calendar: u32, s: &str, name: &str, from: &str) {
    r.insert(format!("P|t|{number}"), format!("t\t{number}\t{calendar}\t{s}\tw\t2\t{from}\t\t\t\t{name}\tBibliothek\t"));
}

pub fn owns(r: &Records) -> Vec<u32> {
    r.keys().filter_map(|k| k.strip_prefix("P|t|")?.parse().ok()).collect()
}

// ---- the Merkliste ----

pub fn mark(r: &mut Records, module: &str, secs: u64) {
    r.entry(format!("B|{module}")).or_insert_with(|| format!("{module}\t{secs}"));
}

pub fn unmark(r: &mut Records, module: &str) {
    r.remove(&format!("B|{module}"));
}

// ---- Sammlungen and notes ----

pub fn shelves(r: &Records) -> Vec<(u32, String)> {
    r.iter().filter_map(|(k, v)| Some((k.strip_prefix("C|c|")?.parse().ok()?, v.clone()))).collect()
}

/// A name is there once (`Shelf`): the store refuses one that is.
pub fn new_shelf(r: &mut Records, id: u32, name: &str) -> bool {
    if shelves(r).iter().any(|(_, n)| n.trim().to_lowercase() == name.trim().to_lowercase()) {
        return false;
    }
    r.insert(format!("C|c|{id}"), name.to_string());
    let order = r.get("C|order").cloned().unwrap_or_default();
    r.insert("C|order".into(), if order.is_empty() { id.to_string() } else { format!("{order},{id}") });
    true
}

pub fn delete_shelf(r: &mut Records, id: u32) {
    r.remove(&format!("C|c|{id}"));
    r.remove(&format!("C|d|{id}"));
    let tail = format!("|{id}");
    r.retain(|k, _| !(k.starts_with("C|m|") && k.ends_with(&tail)));
    if let Some(order) = r.get("C|order").cloned() {
        let rest: Vec<&str> = order.split(',').filter(|o| *o != id.to_string()).collect();
        r.insert("C|order".into(), rest.join(","));
    }
}

/// Two collections change places.
pub fn swap_shelves(r: &mut Records, i: usize, j: usize) {
    if let Some(order) = r.get("C|order").cloned() {
        let mut ids: Vec<&str> = order.split(',').collect();
        if i < ids.len() && j < ids.len() {
            ids.swap(i, j);
            let order = ids.join(",");
            r.insert("C|order".into(), order);
        }
    }
}

pub fn member(r: &mut Records, module: &str, id: u32, on: bool) {
    if r.contains_key(&format!("C|c|{id}")) {
        toggle(r, format!("C|m|{module}|{id}"), on);
    }
}

pub fn note(r: &mut Records, module: &str, text: Option<&str>) {
    match text {
        Some(t) => r.insert(format!("C|n|{module}"), t.to_string()),
        None => r.remove(&format!("C|n|{module}")),
    };
}

// ---- „Mein Studiengang" ----

pub fn mine(r: &mut Records, key: &str, value: &str) {
    r.insert(format!("M|{key}"), format!("{key}\t{value}"));
}
