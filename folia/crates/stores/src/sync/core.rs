//! One synced profile on this device, without the browser: its key and device id, its slot, its
//! records and writer, the batches waiting for Phloem's `stored`, and which entries it holds. The
//! browser part (`browser`) seals and opens, talks to Phloem and keeps all of it in localStorage
//! (`Core::held`, `Core::records`, `Core::waiting` are the three texts); everything here is plain
//! data, tested natively.

use std::collections::BTreeSet;

use folia_sync::{codec, Doc, Hlc, Op, Store, Writer};

use super::codes::{b64, unb64, KEY};

/// The bytes of a device id.
pub const DEVICE: usize = 16;

/// A batch's plain bytes at most: what Phloem takes (64 KiB sealed) less the seal, with room. A
/// change with one record larger than that (a saved plan of a whole semester at the caps) goes out
/// as a snapshot instead.
pub const MAX_BATCH: usize = 60 * 1024;

/// A batch of one change, waiting for `stored`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    /// The device's own id of it (`push`'s `id`): sent again after a reconnect, Phloem keeps it once.
    pub id: String,
    pub batch: Vec<u8>,
}

/// A change a store made before the device had its slot (joining): written once it has.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Raw {
    store: Store,
    before: String,
    after: String,
    wall: u64,
}

#[derive(Clone, Debug)]
pub struct Core {
    pub key: [u8; KEY],
    pub device: [u8; DEVICE],
    /// The device's place in the profile, 0 until Phloem gave one.
    pub slot: u16,
    /// Every entry up to this number is held.
    pub nr: u64,
    /// Entries above `nr` held already (own ones `stored`, others' that came early).
    above: BTreeSet<u64>,
    pub doc: Doc,
    writer: Option<Writer>,
    /// The writer's last number and clock, kept with what waits: a number is never given twice,
    /// also when the records were kept less recently.
    seq: u32,
    last: Hlc,
    pub outbox: Vec<Pending>,
    raw: Vec<Raw>,
    /// The stores whose records changed since they were last given their text.
    pub touched: BTreeSet<Store>,
    /// A change too large for a batch went into the records: the next thing to send is a snapshot.
    pub snapshot_due: bool,
    /// The records of the profile's devices (`super::devices`) changed since they were last kept.
    pub devices_touched: bool,
}

impl Core {
    pub fn new(key: [u8; KEY], device: [u8; DEVICE]) -> Self {
        Core { key, device, slot: 0, nr: 0, above: BTreeSet::new(), doc: Doc::default(), writer: None, seq: 0, last: Hlc::default(), outbox: Vec::new(), raw: Vec::new(), touched: BTreeSet::new(), snapshot_due: false, devices_touched: false }
    }

    /// The slot Phloem gave (on making the space, or in its welcome): the writer goes on after
    /// what the records hold of it, and the changes made before it are written.
    pub fn set_slot(&mut self, slot: u16, mut id: impl FnMut() -> String) {
        if slot == 0 {
            return;
        }
        if self.slot != slot || self.writer.is_none() {
            let (seq, last) = if self.slot == slot { (self.seq, self.last) } else { (0, self.last) };
            self.slot = slot;
            self.writer = Some(Writer::resume(slot, &self.doc, seq, last));
        }
        for raw in std::mem::take(&mut self.raw) {
            self.change(raw.store, &raw.before, &raw.after, raw.wall, &mut id);
        }
    }

    pub fn slot(&self) -> u16 {
        self.slot
    }

    /// A store changed from `before` to `after` (both as the store writes its text): its writes go
    /// into the records and wait to be sent. `false` when it wrote nothing.
    pub fn change(&mut self, store: Store, before: &str, after: &str, wall: u64, mut id: impl FnMut() -> String) -> bool {
        let Some(writer) = self.writer.as_mut() else {
            if before != after {
                self.raw.push(Raw { store, before: before.to_string(), after: after.to_string(), wall });
            }
            return before != after;
        };
        let ops = writer.changes(&mut self.doc, store, before, after, wall);
        self.seq = writer.seq();
        self.last = writer.last();
        if ops.is_empty() {
            return false;
        }
        self.touched.insert(store);
        for batch in codec::batches(&ops, MAX_BATCH) {
            if batch.len() > MAX_BATCH {
                self.snapshot_due = true;
            } else {
                self.outbox.push(Pending { id: id(), batch });
            }
        }
        true
    }

    /// A record outside the stores (`super::devices`): written with the device's clock, waiting to
    /// be sent. `false` where it is so already, or before the device has its slot.
    pub fn put(&mut self, key: &str, val: Option<String>, wall: u64, mut id: impl FnMut() -> String) -> bool {
        let Some(writer) = self.writer.as_mut() else { return false };
        let Some(op) = writer.put(&mut self.doc, key, val, wall) else { return false };
        self.seq = writer.seq();
        self.last = writer.last();
        self.devices_touched |= key.starts_with(super::devices::PREFIX);
        for batch in codec::batches(&[op], MAX_BATCH) {
            self.outbox.push(Pending { id: id(), batch });
        }
        true
    }

    /// The records of the profile's devices, `(key, value)`.
    pub fn device_records(&self) -> Vec<(String, String)> {
        self.doc.live_with(super::devices::PREFIX).filter_map(|(key, e)| Some((key.to_string(), e.val.clone()?))).collect()
    }

    /// Writes of another device (or this one's, back from Phloem): into the records. How many
    /// were new.
    pub fn apply(&mut self, ops: &[Op]) -> usize {
        let mut fresh = 0;
        for op in ops {
            let applied = match self.writer.as_mut() {
                Some(writer) => writer.receive(&mut self.doc, op),
                None => {
                    self.last = self.last.max(op.hlc);
                    self.doc.apply(op)
                }
            };
            if applied == folia_sync::Applied::Taken {
                fresh += 1;
                if let Some(store) = Store::of_record(&op.key) {
                    self.touched.insert(store);
                }
                self.devices_touched |= op.key.starts_with(super::devices::PREFIX);
            }
        }
        fresh
    }

    /// A snapshot of Phloem's (what a device sent as of entry `upto`): joined into the records.
    pub fn join(&mut self, upto: u64, bytes: &[u8]) -> Result<(), codec::Error> {
        let (entries, seen) = codec::read_snapshot(bytes)?;
        for key in self.doc.join(entries, &seen) {
            if let Some(store) = Store::of_record(&key) {
                self.touched.insert(store);
            }
            self.devices_touched |= key.starts_with(super::devices::PREFIX);
        }
        if let Some(writer) = &self.writer {
            // Its clocks count as seen: the writer goes on after them.
            let doc = &self.doc;
            self.writer = Some(Writer::resume(writer.dev(), doc, self.seq, self.last.max(writer.last())));
        }
        self.hold(upto);
        self.nr = self.nr.max(upto);
        self.advance();
        Ok(())
    }

    /// Entry `nr` is held (it came, or it is this device's own, `stored`).
    pub fn hold(&mut self, nr: u64) {
        if nr > self.nr {
            self.above.insert(nr);
        }
        self.advance();
    }

    fn advance(&mut self) {
        self.above.retain(|n| *n > self.nr);
        while self.above.remove(&(self.nr + 1)) {
            self.nr += 1;
        }
    }

    /// The welcome held every entry up to `head`.
    pub fn held_up_to(&mut self, head: u64) {
        self.nr = self.nr.max(head);
        self.advance();
    }

    /// Phloem kept batch `id` as entry `nr`.
    pub fn stored(&mut self, id: &str, nr: u64) {
        self.outbox.retain(|p| p.id != id);
        self.hold(nr);
    }

    /// Every record with the writes they are made of, plain: what `snapshot` seals.
    pub fn snapshot(&self) -> Vec<u8> {
        codec::snapshot(&self.doc)
    }

    /// The projection of a store, as its text.
    pub fn text(&self, store: Store) -> String {
        folia_sync::project(store, &self.doc)
    }

    // ---- what the browser keeps of it ----

    /// `betula.sync.p<id>.v1`: the key, the device and its slot.
    pub fn held(&self) -> String {
        format!("key\t{}\ndevice\t{}\nslot\t{}\n", b64(&self.key), b64(&self.device), self.slot)
    }

    /// `….doc`: which entries are held, the records and the conflicts. Kept now and then.
    pub fn records(&self) -> String {
        format!("nr\t{}\nsnapshot\t{}\nconflicts\t{}\n", self.nr, b64(&codec::snapshot(&self.doc)), b64(&codec::conflicts(&self.doc.conflicts)))
    }

    /// `….out`: the writer's last number and clock, the batches waiting, the changes before the
    /// slot. Kept with every change.
    pub fn waiting(&self) -> String {
        let mut out = format!("seq\t{}\nlast\t{}.{}.{}\n", self.seq, self.last.ms, self.last.n, self.last.dev);
        for p in &self.outbox {
            out.push_str(&format!("batch\t{}\t{}\n", p.id, b64(&p.batch)));
        }
        for r in &self.raw {
            out.push_str(&format!("change\t{}\t{}\t{}\t{}\n", r.store.storage_key(), r.wall, b64(r.before.as_bytes()), b64(r.after.as_bytes())));
        }
        if self.snapshot_due {
            out.push_str("snapshot\t1\n");
        }
        out
    }

    /// A profile as the browser kept it; `None` where its key or device cannot be read (nothing to
    /// sync). What else fails to read is left out: the records come again from Phloem.
    pub fn restore(held: &str, records: &str, waiting: &str) -> Option<Core> {
        let field = |text: &str, name: &str| text.lines().find_map(|line| line.strip_prefix(name)?.strip_prefix('\t').map(str::to_string));
        let key: [u8; KEY] = unb64(&field(held, "key")?)?.try_into().ok()?;
        let device: [u8; DEVICE] = unb64(&field(held, "device")?)?.try_into().ok()?;
        let mut core = Core::new(key, device);
        let slot = field(held, "slot").and_then(|s| s.parse().ok()).unwrap_or(0);
        let snapshot = field(records, "snapshot").and_then(|s| unb64(&s)).and_then(|bytes| codec::read_snapshot(&bytes).ok());
        if let Some((entries, seen)) = snapshot {
            core.doc.entries = entries;
            core.doc.seen = seen;
            core.nr = field(records, "nr").and_then(|n| n.parse().ok()).unwrap_or(0);
            core.doc.conflicts = field(records, "conflicts").and_then(|s| unb64(&s)).and_then(|bytes| codec::read_conflicts(&bytes).ok()).unwrap_or_default();
        }
        core.seq = field(waiting, "seq").and_then(|n| n.parse().ok()).unwrap_or(0);
        core.last = field(waiting, "last").and_then(|l| {
            let mut parts = l.split('.');
            Some(Hlc { ms: parts.next()?.parse().ok()?, n: parts.next()?.parse().ok()?, dev: parts.next()?.parse().ok()? })
        }).unwrap_or_default();
        for line in waiting.lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            match fields.as_slice() {
                ["batch", id, batch] => {
                    // What waits went into the records before: they hold it again.
                    let Some(batch) = unb64(batch) else { continue };
                    if let Ok(ops) = codec::read_batch(&batch) {
                        for op in &ops {
                            core.last = core.last.max(op.hlc);
                            core.doc.apply(op);
                        }
                        core.outbox.push(Pending { id: (*id).to_string(), batch });
                    }
                }
                ["change", store, wall, before, after] => {
                    let (Some(store), Ok(wall)) = (Store::of_storage_key(store), wall.parse()) else { continue };
                    let text = |b: &str| unb64(b).and_then(|bytes| String::from_utf8(bytes).ok());
                    if let (Some(before), Some(after)) = (text(before), text(after)) {
                        core.raw.push(Raw { store, before, after, wall });
                    }
                }
                ["snapshot", "1"] => core.snapshot_due = true,
                _ => {}
            }
        }
        if slot != 0 {
            // The slot's writer; the changes before it wait for the welcome's word on the slot.
            core.slot = slot;
            core.writer = Some(Writer::resume(slot, &core.doc, core.seq, core.last));
        }
        Some(core)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> impl FnMut() -> String {
        let mut n = 0;
        move || {
            n += 1;
            format!("b{n}")
        }
    }

    /// The marks of a Merkliste's text.
    fn marks(ids: &[&str]) -> String {
        ids.iter().enumerate().map(|(i, id)| format!("{id}\t{}\n", 1_790_000_000 - i as u64)).collect()
    }

    #[test]
    fn changes_wait_until_stored_and_entries_count_in_order() {
        let mut a = Core::new([1; KEY], [2; DEVICE]);
        let mut next = ids();
        // Before the slot, changes wait as they are.
        assert!(a.change(Store::Marks, "", &marks(&["11787"]), 1_000, &mut next));
        assert!(a.outbox.is_empty());
        a.set_slot(1, &mut next);
        assert_eq!(a.outbox.len(), 1);
        assert!(a.change(Store::Marks, &marks(&["11787"]), &marks(&["12101", "11787"]), 2_000, &mut next));
        assert!(!a.change(Store::Marks, &marks(&["12101", "11787"]), &marks(&["12101", "11787"]), 3_000, &mut next));
        assert_eq!(a.outbox.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["b1", "b2"]);
        // Phloem keeps them as 3 and 5; 4 came from another device later.
        a.held_up_to(2);
        a.stored("b1", 3);
        a.stored("b2", 5);
        assert_eq!((a.nr, a.outbox.len()), (3, 0));
        a.hold(4);
        assert_eq!(a.nr, 5);
        // Another device reads them in.
        let mut b = Core::new([1; KEY], [3; DEVICE]);
        b.set_slot(2, &mut next);
        b.join(5, &a.snapshot()).unwrap();
        assert_eq!(b.text(Store::Marks), a.text(Store::Marks));
        assert_eq!(b.nr, 5);
        assert!(b.touched.contains(&Store::Marks));
    }

    #[test]
    fn what_the_browser_keeps_comes_back_whole() {
        let mut a = Core::new([7; KEY], [8; DEVICE]);
        let mut next = ids();
        a.set_slot(3, &mut next);
        a.change(Store::Mine, "", "start\t2026W\n", 1_000, &mut next);
        a.held_up_to(4);
        let kept = (a.held(), a.records(), a.waiting());
        let back = Core::restore(&kept.0, &kept.1, &kept.2).unwrap();
        assert_eq!((back.key, back.device, back.slot, back.nr), (a.key, a.device, 3, 4));
        assert_eq!(back.doc, a.doc);
        assert_eq!(back.outbox, a.outbox);
        // The records were kept before the change: what waits brings it back, and numbers go on.
        let older = Core::new([7; KEY], [8; DEVICE]).records();
        let mut back = Core::restore(&kept.0, &older, &kept.2).unwrap();
        assert_eq!(back.text(Store::Mine), "start\t2026W\n");
        back.change(Store::Mine, "start\t2026W\n", "start\t2027S\n", 2_000, &mut next);
        let seqs: Vec<u32> = back.outbox.iter().flat_map(|p| codec::read_batch(&p.batch).unwrap()).map(|op| op.seq).collect();
        assert_eq!(seqs, [1, 2], "a number is never given twice");
        assert!(Core::restore("key\tnope\n", "", "").is_none());
    }

    #[test]
    fn a_change_too_large_for_a_batch_asks_for_a_snapshot() {
        let mut a = Core::new([1; KEY], [2; DEVICE]);
        let mut next = ids();
        a.set_slot(1, &mut next);
        let line = format!("program\t079-82-2008\nname\t{}\n", "x".repeat(MAX_BATCH + 10));
        a.change(Store::Mine, "", &line, 1_000, &mut next);
        assert!(a.snapshot_due);
        assert!(a.outbox.iter().all(|p| p.batch.len() <= MAX_BATCH));
    }
}
