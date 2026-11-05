//! The merged records of a profile: a map from a record's key to its last write, tombstones
//! included, and which writes are in it.
//!
//! Merging keeps, per key, the write with the latest clock: commutative, associative and
//! idempotent, so devices that saw the same writes hold the same map, in whatever order and how
//! often the writes arrived (a state-based CRDT, an LWW map; its writes are the deltas that
//! travel). A deletion is a write of no value, a tombstone, so that an older write that arrives
//! late does not bring the record back.

use std::collections::BTreeMap;

use crate::hlc::Hlc;
use crate::schema;

/// One write of one record, as it travels between devices and through Phloem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Op {
    /// The writer's slot in the profile and its number for the write: a write counts once.
    pub dev: u16,
    pub seq: u32,
    pub key: String,
    /// `None`: deleted.
    pub val: Option<String>,
    pub hlc: Hlc,
    /// When this life of the record began: an update carries its record's; a record made anew
    /// its own clock. What belongs to a calendar or a collection counts only when it was made in
    /// the parent's present life (`schema`), and records that keep an order keep it by it.
    pub created: Hlc,
    /// The write it replaced on the writer's device: two writes on one base saw nothing of each
    /// other.
    pub base: Option<Hlc>,
}

/// What a device holds of one record: its last write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub val: Option<String>,
    pub hlc: Hlc,
    pub created: Hlc,
    pub base: Option<Hlc>,
}

impl Entry {
    fn of(op: &Op) -> Self {
        Entry { val: op.val.clone(), hlc: op.hlc, created: op.created, base: op.base }
    }
}

/// Two writes of one text that saw nothing of each other (a note typed on two devices at once):
/// the later one is kept, the other stays on the device to be looked at and taken over. Or a
/// module taken out of its semester on two devices at once (`other` empty): moved twice, it may
/// stand in two semesters now. Local: every device that sees both writes finds the same pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub key: String,
    pub kept: Option<String>,
    pub other: String,
    pub at: Hlc,
}

/// What a write did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// It was here already.
    Doubled,
    /// A later write of its record was here: it changes nothing.
    Stale,
    /// It is the record's last write now.
    Taken,
}

/// Which writes of each device a device holds: ranges of their numbers, so that a snapshot of a
/// profile with years of writes stays small.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seen(BTreeMap<u16, Vec<(u32, u32)>>);

impl Seen {
    pub fn contains(&self, dev: u16, seq: u32) -> bool {
        self.0.get(&dev).is_some_and(|ranges| {
            let at = ranges.partition_point(|(_, last)| *last < seq);
            ranges.get(at).is_some_and(|(first, _)| *first <= seq)
        })
    }

    /// Adds a write; `false` when it was there.
    pub fn insert(&mut self, dev: u16, seq: u32) -> bool {
        let ranges = self.0.entry(dev).or_default();
        let at = ranges.partition_point(|(_, last)| last.saturating_add(1) < seq);
        // `at` is the first range that ends at or after seq - 1: seq joins it, follows it or comes
        // before it.
        match ranges.get(at).copied() {
            Some((first, last)) if first <= seq && seq <= last => return false,
            Some((first, last)) if last.saturating_add(1) == seq => {
                let merged_last = match ranges.get(at + 1).copied() {
                    Some((next_first, next_last)) if next_first == seq.saturating_add(1) => {
                        ranges.remove(at + 1);
                        next_last
                    }
                    _ => seq,
                };
                if let Some(range) = ranges.get_mut(at) {
                    *range = (first, merged_last);
                }
            }
            Some((first, last)) if seq.saturating_add(1) == first => {
                if let Some(range) = ranges.get_mut(at) {
                    *range = (seq, last);
                }
            }
            _ => ranges.insert(at, (seq, seq)),
        }
        true
    }

    /// The writes of another holder, joined into these.
    pub fn join(&mut self, other: &Seen) {
        for (dev, theirs) in &other.0 {
            let ours = self.0.entry(*dev).or_default();
            let mut all: Vec<(u32, u32)> = ours.iter().chain(theirs).copied().collect();
            all.sort_unstable();
            let mut merged: Vec<(u32, u32)> = Vec::with_capacity(all.len());
            for (first, last) in all {
                match merged.last_mut() {
                    Some((_, end)) if first <= end.saturating_add(1) => *end = (*end).max(last),
                    _ => merged.push((first, last)),
                }
            }
            *ours = merged;
        }
    }

    /// The highest number of a device's writes held: where its next write continues after a
    /// device was set up again from a snapshot.
    pub fn top(&self, dev: u16) -> u32 {
        self.0.get(&dev).and_then(|ranges| ranges.last()).map_or(0, |(_, last)| *last)
    }

    pub fn ranges(&self) -> impl Iterator<Item = (u16, &[(u32, u32)])> {
        self.0.iter().map(|(dev, ranges)| (*dev, ranges.as_slice()))
    }

    /// Sets a device's ranges as read from a snapshot (`codec`): sorted and apart, else nothing.
    pub fn set(&mut self, dev: u16, ranges: Vec<(u32, u32)>) -> bool {
        let ordered = ranges.iter().all(|(first, last)| first <= last) && ranges.windows(2).all(|pair| matches!(pair, [(_, a), (b, _)] if a.saturating_add(1) < *b));
        if ordered {
            self.0.insert(dev, ranges);
        }
        ordered
    }

    /// How many writes are held.
    pub fn count(&self) -> u64 {
        self.0.values().flatten().map(|(first, last)| u64::from(last.saturating_sub(*first)) + 1).sum()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Doc {
    pub entries: BTreeMap<String, Entry>,
    pub seen: Seen,
    /// Local: two writes of one text that saw nothing of each other, newest last.
    pub conflicts: Vec<Conflict>,
}

impl Doc {
    /// Whether this write was applied here already.
    pub fn has(&self, op: &Op) -> bool {
        self.seen.contains(op.dev, op.seq)
    }

    /// Applies a write, from this device or another, as often as it comes.
    pub fn apply(&mut self, op: &Op) -> Applied {
        if !self.seen.insert(op.dev, op.seq) {
            return Applied::Doubled;
        }
        let text = schema::is_text(&op.key);
        let moves = schema::is_move(&op.key);
        let incoming = Entry::of(op);
        let current = self.entries.get(&op.key).cloned();
        match current {
            Some(current) if current.hlc >= incoming.hlc => {
                if siblings(&current, &incoming) {
                    match (&incoming.val, text, moves) {
                        (Some(other), true, _) => self.remember(Conflict { key: op.key.clone(), kept: current.val, other: other.clone(), at: incoming.hlc }),
                        (None, _, true) if current.val.is_none() => self.remember(Conflict { key: op.key.clone(), kept: None, other: String::new(), at: current.hlc }),
                        _ => {}
                    }
                }
                Applied::Stale
            }
            current => {
                if let Some(current) = current {
                    if siblings(&current, &incoming) {
                        match (current.val, text, moves) {
                            (Some(other), true, _) => self.remember(Conflict { key: op.key.clone(), kept: incoming.val.clone(), other, at: current.hlc }),
                            (None, _, true) if incoming.val.is_none() => self.remember(Conflict { key: op.key.clone(), kept: None, other: String::new(), at: incoming.hlc }),
                            _ => {}
                        }
                    }
                }
                self.entries.insert(op.key.clone(), incoming);
                Applied::Taken
            }
        }
    }

    /// Joins every record of a snapshot, as `apply` would join their writes: the later write of
    /// each record wins, so a snapshot that is older than what this device holds changes nothing
    /// of the newer records. Returns the keys that changed.
    pub fn join(&mut self, entries: BTreeMap<String, Entry>, seen: &Seen) -> Vec<String> {
        let mut changed = Vec::new();
        for (key, entry) in entries {
            let newer = self.entries.get(&key).is_none_or(|current| current.hlc < entry.hlc);
            if newer {
                self.entries.insert(key.clone(), entry);
                changed.push(key);
            }
        }
        self.seen.join(seen);
        changed
    }

    /// The live record of a key: `None` when there is none or it was deleted.
    pub fn live(&self, key: &str) -> Option<&Entry> {
        self.entries.get(key).filter(|e| e.val.is_some())
    }

    /// The live records whose keys start with `prefix`, in the order of their keys.
    pub fn live_with<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = (&'a str, &'a Entry)> + 'a {
        self.entries
            .range(prefix.to_string()..)
            .take_while(move |(k, _)| k.starts_with(prefix))
            .filter(|(_, e)| e.val.is_some())
            .map(|(k, e)| (k.as_str(), e))
    }

    /// Tombstones older than `before` are forgotten: what was deleted 13 months ago no longer
    /// has to outweigh a write that old (a device away that long is set up anew).
    pub fn forget_deleted(&mut self, before: u64) {
        self.entries.retain(|_, e| e.val.is_some() || e.hlc.ms >= before);
    }

    fn remember(&mut self, conflict: Conflict) {
        if !self.conflicts.iter().any(|c| c.key == conflict.key && c.at == conflict.at) {
            self.conflicts.push(conflict);
        }
        // A note typed back and forth on two devices for an hour leaves a few versions, not
        // hundreds.
        let excess = self.conflicts.len().saturating_sub(50);
        self.conflicts.drain(..excess);
    }
}

/// Two writes made on the same base: neither saw the other.
fn siblings(a: &Entry, b: &Entry) -> bool {
    a.hlc != b.hlc && a.base == b.base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seen_keeps_ranges_and_counts_each_write_once() {
        let mut seen = Seen::default();
        for seq in [5, 1, 2, 3, 9, 4, 7, 8, 6] {
            assert!(seen.insert(1, seq));
        }
        assert!(!seen.insert(1, 4));
        assert_eq!(seen.ranges().collect::<Vec<_>>(), vec![(1, &[(1, 9)][..])]);
        assert!(seen.insert(1, 20) && seen.insert(1, 11));
        assert_eq!(seen.ranges().next().map(|(_, r)| r.to_vec()), Some(vec![(1, 9), (11, 11), (20, 20)]));
        assert!(seen.insert(1, 10));
        assert_eq!(seen.ranges().next().map(|(_, r)| r.to_vec()), Some(vec![(1, 11), (20, 20)]));
        assert!(seen.contains(1, 11) && !seen.contains(1, 12) && !seen.contains(2, 1));
        assert_eq!((seen.top(1), seen.count()), (20, 12));
    }

    #[test]
    fn seen_joins_ranges_without_counting_them_out() {
        let mut a = Seen::default();
        let mut b = Seen::default();
        assert!(a.set(1, vec![(1, 5), (9, 9)]) && a.set(2, vec![(1, 1)]));
        assert!(b.set(1, vec![(6, 7), (11, u32::MAX)]) && b.set(3, vec![(4, 4)]));
        a.join(&b);
        let ranges: Vec<(u16, Vec<(u32, u32)>)> = a.ranges().map(|(dev, r)| (dev, r.to_vec())).collect();
        assert_eq!(ranges, vec![(1, vec![(1, 7), (9, 9), (11, u32::MAX)]), (2, vec![(1, 1)]), (3, vec![(4, 4)])]);
        assert!(a.contains(1, 4_000_000_000) && !a.contains(1, 8) && !a.contains(1, 10));
        assert_eq!(a.top(1), u32::MAX);
    }

    #[test]
    fn seen_survives_any_order() {
        let mut state = 12345u64;
        for _ in 0..200 {
            let mut seen = Seen::default();
            let mut all = std::collections::BTreeSet::new();
            for _ in 0..60 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let seq = (state % 80) as u32 + 1;
                assert_eq!(seen.insert(3, seq), all.insert(seq));
            }
            for seq in 0..90 {
                assert_eq!(seen.contains(3, seq), all.contains(&seq), "{seq}");
            }
            let ranges = seen.ranges().next().map(|(_, r)| r.to_vec()).unwrap_or_default();
            assert!(ranges.windows(2).all(|w| w[0].1 + 1 < w[1].0), "apart: {ranges:?}");
        }
    }
}
