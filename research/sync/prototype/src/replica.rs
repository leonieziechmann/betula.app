//! One device of a profile: its clock, its merged records, and the way a change becomes writes.

use crate::doc::{Doc, Op};
use crate::hlc::Clock;
use crate::schema::{self, Records, Store, SLOTS};

pub struct Replica {
    pub dev: u16,
    /// The device's place in its profile (1–16), handed out by the service when it joined.
    pub slot: u32,
    pub clock: Clock,
    pub doc: Doc,
    seq: u32,
}

impl Replica {
    pub fn new(dev: u16, slot: u32) -> Self {
        Replica { dev, slot, clock: Clock::new(dev), doc: Doc::default(), seq: 0 }
    }

    /// The text a store of this device holds: what its records make.
    pub fn text(&self, store: Store) -> String {
        schema::project(store, &self.doc)
    }

    /// The records of that text, as the store reads them.
    pub fn view(&self, store: Store) -> Records {
        schema::records(store, &self.text(store))
    }

    /// A change the person makes in one store, `f` doing to its records what the store does to
    /// its text. The writes are exactly the difference between the store before and after the
    /// change: nothing that was only read, normalized or left out by the store ever becomes a
    /// write, so a build that drops a line it cannot read never deletes it on other devices.
    pub fn change(&mut self, store: Store, wall: u64, f: impl FnOnce(&mut Records)) -> Vec<Op> {
        let before = self.view(store);
        let mut after = before.clone();
        f(&mut after);
        let mut ops = Vec::new();
        for (key, val) in schema::diff(&before, &after) {
            let hlc = self.clock.tick(wall);
            let current = self.doc.entries.get(&key);
            // A record the store showed keeps its life; one it did not show begins a new one.
            let created = match current {
                Some(e) if val.is_some() && before.contains_key(&key) => e.created,
                _ => hlc,
            };
            let base = current.map(|e| e.hlc);
            self.seq += 1;
            let op = Op { dev: self.dev, seq: self.seq, key, val, hlc, created, base };
            self.doc.apply(&op, schema::is_text(&op.key));
            ops.push(op);
        }
        ops
    }

    /// Writes from another device (directly or through the service), in any order, any number
    /// of times. Returns how many were new.
    pub fn receive(&mut self, ops: &[Op]) -> usize {
        let mut fresh = 0;
        for op in ops {
            self.clock.observe(op.hlc);
            if self.doc.apply(op, schema::is_text(&op.key)) {
                fresh += 1;
            }
        }
        fresh
    }

    /// A number for something new under `prefix` (a placeholder, a calendar, a Termin of one's
    /// own, a collection) from this device's own class, the smallest one free; `reuse` takes one
    /// whose record was deleted (calendars: there are only 99 numbers).
    pub fn free_id(&self, prefix: &str, max: u32, reuse: bool) -> Option<u32> {
        let class = self.slot % SLOTS;
        let first = if class == 0 { SLOTS } else { class };
        (first..=max).step_by(SLOTS as usize).find(|id| match self.doc.entries.get(&format!("{prefix}{id}")) {
            None => true,
            Some(e) => reuse && e.val.is_none(),
        })
    }
}
