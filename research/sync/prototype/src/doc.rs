//! The replicated profile: a map from a record's key to its last write, tombstones included.
//!
//! Every write is an `Op` stamped with a hybrid logical clock. Merging keeps, per key, the write
//! with the latest clock: commutative, associative and idempotent, so devices that saw the same
//! writes hold the same map, in whatever order and how often the writes arrived (a state-based
//! CRDT, an LWW map; its writes are the deltas that travel). A deletion is a write of no value
//! (a tombstone), so that an older write that arrives late does not bring the record back.

use std::collections::{BTreeMap, BTreeSet};

use crate::hlc::Hlc;

/// One write of one record, as it travels between devices and to the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Op {
    /// Who wrote it, and the writer's number for it: a write is applied once.
    pub dev: u16,
    pub seq: u32,
    pub key: String,
    /// `None`: deleted.
    pub val: Option<String>,
    pub hlc: Hlc,
    /// When this life of the record began: an update carries its record's, a record made anew
    /// (or again after a deletion) its own clock. Children count only when made after their
    /// parent's life began (`schema`).
    pub created: Hlc,
    /// The write it replaced on the writer's device, if any: two writes of one base saw nothing
    /// of each other.
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

/// Two writes of one text that saw nothing of each other: the later one is kept, the other one
/// stays on the device to be looked at („die andere Fassung“). Local, never synced: every device
/// that sees both writes records the same pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub key: String,
    pub kept: Option<String>,
    pub other: String,
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub entries: BTreeMap<String, Entry>,
    seen: BTreeMap<u16, BTreeSet<u32>>,
    pub conflicts: Vec<Conflict>,
    /// Writes that arrived after a later write of the same record (and lost), and writes that
    /// arrived a second time: what the merge had to sort out.
    pub stale: usize,
    pub doubled: usize,
}

impl Doc {
    /// Whether this write was applied here already.
    pub fn has(&self, op: &Op) -> bool {
        self.seen.get(&op.dev).is_some_and(|s| s.contains(&op.seq))
    }

    /// Applies a write; `text` keeps the losing one of two concurrent writes as a conflict.
    /// `false` when it was applied before.
    pub fn apply(&mut self, op: &Op, text: bool) -> bool {
        if !self.seen.entry(op.dev).or_default().insert(op.seq) {
            self.doubled += 1;
            return false;
        }
        let incoming = Entry { val: op.val.clone(), hlc: op.hlc, created: op.created, base: op.base };
        match self.entries.get(&op.key) {
            Some(current) if current.hlc >= op.hlc => {
                self.stale += 1;
                if text && siblings(current, &incoming) {
                    if let Some(other) = incoming.val.clone() {
                        self.conflicts.push(Conflict { key: op.key.clone(), kept: current.val.clone(), other });
                    }
                }
            }
            current => {
                if let Some(current) = current {
                    if text && siblings(current, &incoming) {
                        if let Some(other) = current.val.clone() {
                            self.conflicts.push(Conflict { key: op.key.clone(), kept: incoming.val.clone(), other });
                        }
                    }
                }
                self.entries.insert(op.key.clone(), incoming);
            }
        }
        true
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

    /// How many writes of each device this device has applied.
    pub fn applied(&self) -> usize {
        self.seen.values().map(BTreeSet::len).sum()
    }
}

/// Two writes made on the same base: neither saw the other.
fn siblings(a: &Entry, b: &Entry) -> bool {
    a.hlc != b.hlc && a.base == b.base
}
