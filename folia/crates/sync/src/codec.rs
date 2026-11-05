//! How writes and snapshots travel: a compact binary form behind a version byte (LEB128 numbers,
//! texts as their length and UTF-8), sealed by the browser before it leaves the device. Read back
//! with every length checked: what arrives opened only because its seal held, but a device of
//! another build may have written it, so nothing in it can make a reader panic or take more
//! memory than the bytes it came in.
//!
//! A batch: the version, the number of writes, each write as its device and number, a byte of
//! flags, its clock, its life and base when it has them (a life that is its own clock is left
//! out), its key and its value. A snapshot: the version, which writes it holds, then every record
//! with its clocks. The conflicts a device keeps are its own and travel nowhere; they have a form
//! here for the device's own storage.

use std::collections::BTreeMap;

use crate::doc::{Conflict, Doc, Entry, Op, Seen};
use crate::hlc::Hlc;

pub const VERSION: u8 = 1;

const VAL: u8 = 1;
const CREATED: u8 = 2;
const BASE: u8 = 4;
/// A write whose clock names another slot than its writer (never made by this build).
const OTHER: u8 = 8;
const KNOWN: u8 = VAL | CREATED | BASE | OTHER;

/// Why bytes could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// They end before what they say they hold.
    Short,
    /// Of a version this build cannot read.
    Version(u8),
    /// A text that is not UTF-8, a number too large for its place, flags or ranges this build
    /// does not know.
    Invalid,
    /// Bytes after the end.
    Trailing,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Short => write!(f, "ends too early"),
            Error::Version(v) => write!(f, "version {v}"),
            Error::Invalid => write!(f, "invalid"),
            Error::Trailing => write!(f, "bytes after the end"),
        }
    }
}

impl std::error::Error for Error {}

// ---- writing ----

fn varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn text(out: &mut Vec<u8>, s: &str) {
    varint(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

fn clock(out: &mut Vec<u8>, h: &Hlc) {
    varint(out, h.ms);
    varint(out, u64::from(h.n));
    varint(out, u64::from(h.dev));
}

fn op(out: &mut Vec<u8>, op: &Op) {
    varint(out, u64::from(op.dev));
    varint(out, u64::from(op.seq));
    let mut flags = 0;
    if op.val.is_some() {
        flags |= VAL;
    }
    if op.created != op.hlc {
        flags |= CREATED;
    }
    if op.base.is_some() {
        flags |= BASE;
    }
    if op.hlc.dev != op.dev {
        flags |= OTHER;
    }
    out.push(flags);
    varint(out, op.hlc.ms);
    varint(out, u64::from(op.hlc.n));
    if flags & OTHER != 0 {
        varint(out, u64::from(op.hlc.dev));
    }
    if flags & CREATED != 0 {
        clock(out, &op.created);
    }
    if let Some(base) = &op.base {
        clock(out, base);
    }
    text(out, &op.key);
    if let Some(val) = &op.val {
        text(out, val);
    }
}

/// The writes of one change, as one batch.
pub fn batch(ops: &[Op]) -> Vec<u8> {
    let mut out = vec![VERSION];
    varint(&mut out, ops.len() as u64);
    for o in ops {
        op(&mut out, o);
    }
    out
}

/// The writes of one change as batches of at most `max` bytes each, in their order; a write that
/// alone is larger is a batch of its own (the device sends a snapshot instead).
pub fn batches(ops: &[Op], max: usize) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut part: Vec<Op> = Vec::new();
    let mut size = 1 + 3;
    for o in ops {
        let mut one = Vec::new();
        op(&mut one, o);
        if !part.is_empty() && size + one.len() > max {
            out.push(batch(&part));
            part.clear();
            size = 1 + 3;
        }
        size += one.len();
        part.push(o.clone());
    }
    if !part.is_empty() {
        out.push(batch(&part));
    }
    out
}

/// Every record a device holds, tombstones included, with the writes they are made of: what a new
/// device starts from, and what lets the service forget the writes before it.
pub fn snapshot(doc: &Doc) -> Vec<u8> {
    let mut out = vec![VERSION];
    let devices: Vec<(u16, &[(u32, u32)])> = doc.seen.ranges().collect();
    varint(&mut out, devices.len() as u64);
    for (dev, ranges) in devices {
        varint(&mut out, u64::from(dev));
        varint(&mut out, ranges.len() as u64);
        for (first, last) in ranges {
            varint(&mut out, u64::from(*first));
            varint(&mut out, u64::from(last.saturating_sub(*first)));
        }
    }
    varint(&mut out, doc.entries.len() as u64);
    for (key, e) in &doc.entries {
        text(&mut out, key);
        let mut flags = 0;
        if e.val.is_some() {
            flags |= VAL;
        }
        if e.created != e.hlc {
            flags |= CREATED;
        }
        if e.base.is_some() {
            flags |= BASE;
        }
        out.push(flags);
        clock(&mut out, &e.hlc);
        if flags & CREATED != 0 {
            clock(&mut out, &e.created);
        }
        if let Some(base) = &e.base {
            clock(&mut out, base);
        }
        if let Some(val) = &e.val {
            text(&mut out, val);
        }
    }
    out
}

/// The conflicts a device keeps, for its own storage.
pub fn conflicts(conflicts: &[Conflict]) -> Vec<u8> {
    let mut out = vec![VERSION];
    varint(&mut out, conflicts.len() as u64);
    for c in conflicts {
        text(&mut out, &c.key);
        match &c.kept {
            Some(kept) => {
                out.push(VAL);
                text(&mut out, kept);
            }
            None => out.push(0),
        }
        text(&mut out, &c.other);
        clock(&mut out, &c.at);
    }
    out
}

// ---- reading ----

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, Error> {
        let mut r = Reader { bytes };
        match r.byte()? {
            VERSION => Ok(r),
            other => Err(Error::Version(other)),
        }
    }

    fn byte(&mut self) -> Result<u8, Error> {
        let (first, rest) = self.bytes.split_first().ok_or(Error::Short)?;
        self.bytes = rest;
        Ok(*first)
    }

    fn varint(&mut self) -> Result<u64, Error> {
        let mut v: u64 = 0;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            let part = u64::from(b & 0x7f);
            if shift == 63 && part > 1 {
                return Err(Error::Invalid);
            }
            v |= part << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(Error::Invalid)
    }

    fn u32(&mut self) -> Result<u32, Error> {
        u32::try_from(self.varint()?).map_err(|_| Error::Invalid)
    }

    fn u16(&mut self) -> Result<u16, Error> {
        u16::try_from(self.varint()?).map_err(|_| Error::Invalid)
    }

    /// A count of things that take at least one byte each: never more than the bytes left.
    fn count(&mut self) -> Result<usize, Error> {
        let n = usize::try_from(self.varint()?).map_err(|_| Error::Invalid)?;
        if n > self.bytes.len() {
            return Err(Error::Short);
        }
        Ok(n)
    }

    fn text(&mut self) -> Result<String, Error> {
        let len = self.count()?;
        let (s, rest) = self.bytes.split_at_checked(len).ok_or(Error::Short)?;
        self.bytes = rest;
        String::from_utf8(s.to_vec()).map_err(|_| Error::Invalid)
    }

    fn clock(&mut self) -> Result<Hlc, Error> {
        Ok(Hlc { ms: self.varint()?, n: self.u32()?, dev: self.u16()? })
    }

    fn flags(&mut self) -> Result<u8, Error> {
        let flags = self.byte()?;
        if flags & !KNOWN != 0 {
            return Err(Error::Invalid);
        }
        Ok(flags)
    }

    fn end(&self) -> Result<(), Error> {
        if self.bytes.is_empty() {
            Ok(())
        } else {
            Err(Error::Trailing)
        }
    }
}

/// The writes of a batch.
pub fn read_batch(bytes: &[u8]) -> Result<Vec<Op>, Error> {
    let mut r = Reader::new(bytes)?;
    let n = r.count()?;
    let mut ops = Vec::new();
    for _ in 0..n {
        let dev = r.u16()?;
        let seq = r.u32()?;
        let flags = r.flags()?;
        let ms = r.varint()?;
        let count = r.u32()?;
        let hlc_dev = if flags & OTHER != 0 { r.u16()? } else { dev };
        let hlc = Hlc { ms, n: count, dev: hlc_dev };
        let created = if flags & CREATED != 0 { r.clock()? } else { hlc };
        let base = if flags & BASE != 0 { Some(r.clock()?) } else { None };
        let key = r.text()?;
        let val = if flags & VAL != 0 { Some(r.text()?) } else { None };
        ops.push(Op { dev, seq, key, val, hlc, created, base });
    }
    r.end()?;
    Ok(ops)
}

/// The records of a snapshot and the writes they are made of.
pub fn read_snapshot(bytes: &[u8]) -> Result<(BTreeMap<String, Entry>, Seen), Error> {
    let mut r = Reader::new(bytes)?;
    let mut seen = Seen::default();
    let devices = r.count()?;
    for _ in 0..devices {
        let dev = r.u16()?;
        let n = r.count()?;
        let mut ranges = Vec::new();
        for _ in 0..n {
            let first = r.u32()?;
            let last = first.checked_add(r.u32()?).ok_or(Error::Invalid)?;
            ranges.push((first, last));
        }
        if seen.ranges().any(|(d, _)| d == dev) || !seen.set(dev, ranges) {
            return Err(Error::Invalid);
        }
    }
    let n = r.count()?;
    let mut entries = BTreeMap::new();
    for _ in 0..n {
        let key = r.text()?;
        let flags = r.flags()?;
        if flags & OTHER != 0 {
            return Err(Error::Invalid);
        }
        let hlc = r.clock()?;
        let created = if flags & CREATED != 0 { r.clock()? } else { hlc };
        let base = if flags & BASE != 0 { Some(r.clock()?) } else { None };
        let val = if flags & VAL != 0 { Some(r.text()?) } else { None };
        if entries.insert(key, Entry { val, hlc, created, base }).is_some() {
            return Err(Error::Invalid);
        }
    }
    r.end()?;
    Ok((entries, seen))
}

/// The conflicts a device kept.
pub fn read_conflicts(bytes: &[u8]) -> Result<Vec<Conflict>, Error> {
    let mut r = Reader::new(bytes)?;
    let n = r.count()?;
    let mut out = Vec::new();
    for _ in 0..n {
        let key = r.text()?;
        let kept = match r.byte()? {
            0 => None,
            VAL => Some(r.text()?),
            _ => return Err(Error::Invalid),
        };
        let other = r.text()?;
        let at = r.clock()?;
        out.push(Conflict { key, kept, other, at });
    }
    r.end()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: u64, n: u32, dev: u16) -> Hlc {
        Hlc { ms, n, dev }
    }

    fn ops() -> Vec<Op> {
        vec![
            Op { dev: 1, seq: 1, key: "P|mv|2026W|11787".into(), val: Some("m\t2026W\t11787\t1762300000\t".into()), hlc: at(1_762_300_000_123, 0, 1), created: at(1_762_300_000_123, 0, 1), base: None },
            Op { dev: 1, seq: 2, key: "C|n|11787".into(), val: Some("n\t11787\tAltklausur – mit Lena 🌿".into()), hlc: at(1_762_300_000_123, 1, 1), created: at(1_762_200_000_000, 4, 3), base: Some(at(1_762_250_000_000, 0, 2)) },
            Op { dev: 16, seq: u32::MAX, key: "B|12101".into(), val: None, hlc: at(u64::MAX, u32::MAX, 16), created: at(0, 0, 0), base: Some(at(1, 2, 3)) },
            Op { dev: 2, seq: 7, key: "M|start".into(), val: Some(String::new()), hlc: at(5, 0, 9), created: at(5, 0, 9), base: None },
        ]
    }

    #[test]
    fn a_batch_comes_back_as_it_went() {
        let ops = ops();
        assert_eq!(read_batch(&batch(&ops)), Ok(ops.clone()));
        assert_eq!(read_batch(&batch(&[])), Ok(Vec::new()));
        let parts = batches(&ops, 60);
        assert!(parts.len() > 1);
        let again: Vec<Op> = parts.iter().flat_map(|p| read_batch(p).unwrap()).collect();
        assert_eq!(again, ops);
        assert_eq!(batches(&ops, 1 << 20).len(), 1);
    }

    #[test]
    fn a_snapshot_comes_back_as_it_went() {
        let mut doc = Doc::default();
        for o in ops() {
            doc.apply(&o);
        }
        doc.seen.insert(3, 40);
        let (entries, seen) = read_snapshot(&snapshot(&doc)).unwrap();
        assert_eq!(entries, doc.entries);
        assert_eq!(seen, doc.seen);
        assert_eq!(read_snapshot(&snapshot(&Doc::default())), Ok((BTreeMap::new(), Seen::default())));
    }

    #[test]
    fn conflicts_come_back_as_they_went() {
        let kept = vec![
            Conflict { key: "C|n|11787".into(), kept: Some("eins".into()), other: "zwei".into(), at: at(9, 1, 2) },
            Conflict { key: "P|b|0|2026W|301512-a1b2c".into(), kept: None, other: "drei".into(), at: at(10, 0, 1) },
        ];
        assert_eq!(read_conflicts(&conflicts(&kept)), Ok(kept));
    }

    #[test]
    fn broken_bytes_are_refused_and_never_panic() {
        let whole = batch(&ops());
        for cut in 0..whole.len() {
            assert!(read_batch(&whole[..cut]).is_err(), "cut at {cut}");
        }
        let mut longer = whole.clone();
        longer.push(0);
        assert_eq!(read_batch(&longer), Err(Error::Trailing));
        assert_eq!(read_batch(&[2, 0]), Err(Error::Version(2)));
        // A count far beyond the bytes, a text longer than what is left, a number of eleven bytes.
        assert_eq!(read_batch(&[1, 0xff, 0xff, 0xff, 0xff, 0x0f]), Err(Error::Short));
        assert!(read_snapshot(&[1, 0, 1, 0xff, 0xff, 0x03, b'a']).is_err());
        assert_eq!(read_batch(&[1, 1, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01]), Err(Error::Invalid));
        // Ranges out of order, or a device twice.
        assert_eq!(read_snapshot(&[1, 1, 1, 2, 5, 0, 1, 0, 0]), Err(Error::Invalid));
        assert_eq!(read_snapshot(&[1, 2, 1, 1, 1, 0, 1, 1, 3, 0, 0]), Err(Error::Invalid));
        // Every byte flipped, one at a time: an error or a value, never a panic.
        let snap = {
            let mut doc = Doc::default();
            for o in ops() {
                doc.apply(&o);
            }
            snapshot(&doc)
        };
        for bytes in [whole, snap] {
            for i in 0..bytes.len() {
                for bit in 0..8 {
                    let mut b = bytes.clone();
                    b[i] ^= 1 << bit;
                    let _ = read_batch(&b);
                    let _ = read_snapshot(&b);
                    let _ = read_conflicts(&b);
                }
            }
        }
    }
}
