//! What writes take on the wire: a compact binary form (varints, clocks as milliseconds since
//! 2026), and a batch sealed with XChaCha20-Poly1305 behind a version byte. Only the lengths are
//! computed here: the prototype shows sizes, not a format.

use crate::doc::{Doc, Op};
use crate::hlc::Hlc;

/// A version byte, the 24-byte nonce and the 16-byte tag of XChaCha20-Poly1305.
pub const SEAL: usize = 1 + 24 + 16;

/// 2026-01-01T00:00:00Z in milliseconds.
pub const EPOCH_MS: u64 = 1_767_225_600_000;

fn varint(mut v: u64) -> usize {
    let mut n = 1;
    while v >= 0x80 {
        v >>= 7;
        n += 1;
    }
    n
}

fn clock(h: &Hlc) -> usize {
    varint(h.ms.saturating_sub(EPOCH_MS)) + varint(u64::from(h.n))
}

fn other_clock(h: &Hlc) -> usize {
    clock(h) + varint(u64::from(h.dev))
}

/// One write: who and which, its clock, its life and base (a flag when it has none or they are
/// its own clock), its key and its value.
pub fn op_bytes(op: &Op) -> usize {
    let created = if op.created == op.hlc { 1 } else { 1 + other_clock(&op.created) };
    let base = op.base.as_ref().map_or(1, |b| 1 + other_clock(b));
    let val = op.val.as_ref().map_or(1, |v| varint(v.len() as u64 + 1) + v.len());
    varint(u64::from(op.dev)) + varint(u64::from(op.seq)) + clock(&op.hlc) + created + base + varint(op.key.len() as u64) + op.key.len() + val
}

/// The writes of one change, sealed.
pub fn batch_bytes(ops: &[Op]) -> usize {
    SEAL + varint(ops.len() as u64) + ops.iter().map(op_bytes).sum::<usize>()
}

/// Every record of a profile with its clocks, tombstones included, sealed: what a new device
/// downloads first, and what lets the service forget the writes before it.
pub fn snapshot_bytes(doc: &Doc) -> usize {
    SEAL + doc
        .entries
        .iter()
        .map(|(k, e)| {
            let val = e.val.as_ref().map_or(1, |v| varint(v.len() as u64 + 1) + v.len());
            let base = e.base.as_ref().map_or(1, |b| 1 + other_clock(b));
            varint(k.len() as u64) + k.len() + val + other_clock(&e.hlc) + other_clock(&e.created) + base
        })
        .sum::<usize>()
}
