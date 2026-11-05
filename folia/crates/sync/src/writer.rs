//! What a device writes: the records a change of a store made differ, each with the device's
//! clock and its next number.

use std::collections::BTreeMap;

use crate::doc::{Applied, Doc, Op};
use crate::hlc::{Clock, Hlc};
use crate::schema::{self, Records, Store};

/// The writes of one device of a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Writer {
    clock: Clock,
    seq: u32,
}

impl Writer {
    /// The writer of the device in slot `dev`, going on after what `doc` holds: its writes are
    /// numbered after the last one of the slot there, and its clock is after every write there.
    pub fn new(dev: u16, doc: &Doc) -> Self {
        let mut clock = Clock::new(dev);
        for entry in doc.entries.values() {
            clock.observe(entry.hlc);
        }
        Writer { clock, seq: doc.seen.top(dev) }
    }

    /// The same, going on after a write numbered `seq` and a clock at `last` that this device
    /// gave before (what it keeps beside its records, for records it kept less often than its
    /// writes): a number is never given twice, and no clock goes back.
    pub fn resume(dev: u16, doc: &Doc, seq: u32, last: Hlc) -> Self {
        let mut writer = Writer::new(dev, doc);
        writer.seq = writer.seq.max(seq);
        writer.clock.observe(last);
        writer
    }

    pub fn dev(&self) -> u16 {
        self.clock.dev()
    }

    /// The number of the device's last write.
    pub fn seq(&self) -> u32 {
        self.seq
    }

    /// The latest time the clock gave or saw.
    pub fn last(&self) -> Hlc {
        self.clock.last()
    }

    /// A write of another device came: applied, and the next write comes after it.
    pub fn receive(&mut self, doc: &mut Doc, op: &Op) -> Applied {
        self.clock.observe(op.hlc);
        doc.apply(op)
    }

    /// The writes of one change of a store, from its text before the change to its text after it,
    /// both as this build writes them: the records that differ, in the order of the text after
    /// it, then the ones gone. Applied to `doc` as they are made. What the store only read, left
    /// out or wrote the same never becomes a write, and neither does a record `doc` holds as the
    /// store wants it already.
    pub fn changes(&mut self, doc: &mut Doc, store: Store, before: &str, after: &str, wall: u64) -> Vec<Op> {
        let before = schema::records(store, before);
        let after = schema::records(store, after);
        let groups: Vec<(&str, Option<String>)> = after.iter().map(|(key, val)| (key, group(key, val))).collect();
        let place: BTreeMap<&str, usize> = groups.iter().enumerate().map(|(at, (key, _))| (*key, at)).collect();
        let mut ops = Vec::new();
        for (key, val) in schema::diff(&before, &after) {
            let current = doc.entries.get(&key).cloned();
            // A deletion of what is gone already writes nothing; anything else is written, also
            // what `doc` holds the same: it may be of a life the store cannot show.
            if val.is_none() && current.as_ref().is_none_or(|e| e.val.is_none()) {
                continue;
            }
            let hlc = self.clock.tick(wall);
            let created = match (&current, &val) {
                // An update of a record the store showed: the same life, unless it went to another
                // group (a visit planned, a Termin into another calendar), whose end it joins.
                (Some(e), Some(v)) if e.val.is_some() && before.contains(&key) && e.val.as_deref().map(|old| group(&key, old)) == Some(group(&key, v)) => e.created,
                // Put back where it stood („Rückgängig"): it keeps its place and the life it had,
                // where its old place fits the order of the text and the calendar it belongs to
                // still lives that life. A calendar or a collection that comes back begins a new
                // life: one made anew under a number used before looks the same, and what was
                // added to the old one must not appear in it.
                (Some(e), Some(v)) if e.val.is_none() && !lives(&key) && place.get(key.as_str()).is_some_and(|at| fits(doc, &before, &groups, *at, e.created)) && parent_lives(doc, &key, v, e.created) => e.created,
                // A deletion ends the life it had, so that it can come back in its place.
                (Some(e), None) => e.created,
                _ => hlc,
            };
            let base = current.map(|e| e.hlc);
            self.seq = self.seq.saturating_add(1);
            let op = Op { dev: self.dev(), seq: self.seq, key, val, hlc, created, base };
            doc.apply(&op);
            ops.push(op);
        }
        ops
    }
}

/// The group of records whose order the store keeps by when each was made: modules planned (and
/// visits) of one semester, passed modules, finished rows, Termine of one's own of one calendar,
/// the calendars, the modules of one calendar, the saved plans, the lines of a newer build.
fn group(key: &str, val: &str) -> Option<String> {
    let field = |i: usize| val.split('\t').nth(i).map(str::trim).unwrap_or_default();
    if let Some(rest) = key.strip_prefix("P|mv|") {
        let semester = rest.split('|').next().unwrap_or_default();
        return Some(format!("P|{}|{semester}", field(0)));
    }
    if key.starts_with("P|t|") {
        return Some(format!("P|t|{}", field(2)));
    }
    if let Some(rest) = key.strip_prefix("P|n|") {
        return Some(format!("P|n|{}", rest.split('|').next().unwrap_or_default()));
    }
    ["P|d|", "P|q|", "P|l|", "P|?|", "S|"].iter().find(|prefix| key.starts_with(*prefix)).map(|prefix| (*prefix).to_string())
}

/// Whether a record put back with the clock it was made at (`created`) stands where the text has
/// it: after the records of its group before it there, which `doc` holds already (also those just
/// written), and before the ones after it that the store showed. A record planned anew after
/// others came does not fit its old place and is made now, at the end, where the store put it.
fn fits(doc: &Doc, before: &Records, groups: &[(&str, Option<String>)], at: usize, created: Hlc) -> bool {
    let Some((_, Some(g))) = groups.get(at) else { return false };
    let (earlier, later) = groups.split_at(at);
    let of_group = |(_, other): &&(&str, Option<String>)| other.as_ref() == Some(g);
    earlier.iter().filter(of_group).all(|(key, _)| doc.live(key).is_none_or(|e| e.created < created))
        && later.iter().skip(1).filter(of_group).all(|(key, _)| !before.contains(key) || doc.live(key).is_none_or(|e| e.created > created))
}

/// What other records belong to by its life: a calendar of one's own, a collection.
fn lives(key: &str) -> bool {
    key.starts_with("P|l|") || key.starts_with("C|c|")
}

/// Whether a record of a calendar put back with the life it had still belongs to the calendar:
/// the calendar lives the life the record was made in. The plan's own records belong to none.
fn parent_lives(doc: &Doc, key: &str, val: &str, created: Hlc) -> bool {
    let calendar = if key.starts_with("P|t|") {
        val.split('\t').nth(2).map(str::trim).unwrap_or_default()
    } else if let Some(rest) = key.strip_prefix("P|n|") {
        rest.split('|').next().unwrap_or_default()
    } else {
        return true;
    };
    calendar == "0" || doc.live(&format!("P|l|{calendar}")).is_some_and(|l| l.created <= created)
}
