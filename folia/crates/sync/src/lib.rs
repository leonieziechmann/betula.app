//! Betula Sync: how the five stores of a profile merge between devices that change them at the
//! same time, online, offline or directly (research/sync/entwurf.html; the prototype it comes from
//! is research/sync/prototype, the service docs/phloem/protocol.md).
//!
//! Every line of a store's text is a record under what the store itself tells lines apart by
//! (`schema`): a module once per semester, planned or visited; a placeholder by its pid; a note
//! once per module. A set within a line is a record per element. A change of a store is written
//! as the records it made differ (`Writer::changes`), never as what the store only read: a build
//! that drops a line it cannot read never deletes it on other devices. Every write carries a
//! hybrid logical clock (`hlc`), and of two writes of one record the later one wins (`doc`, an LWW
//! map with tombstones: devices that saw the same writes hold the same records, in whatever order
//! and however often the writes came). The records give every store its text back (`project`),
//! in the order the store writes it, so the store reads the same thing it wrote. `codec` is how
//! writes and snapshots travel.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

pub mod codec;
pub mod doc;
pub mod hlc;
pub mod schema;
pub mod writer;

pub use doc::{Applied, Conflict, Doc, Entry, Op, Seen};
pub use hlc::{Clock, Hlc};
pub use schema::{project, records, Store};
pub use writer::Writer;
