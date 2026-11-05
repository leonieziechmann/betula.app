//! Betula Sync: how the stores of one profile merge between devices that change them at the
//! same time, online or offline. A prototype of `research/sync/entwurf.html`, not part of the app.
//!
//! - `hlc`: the clock every write carries.
//! - `doc`: the merged records, one last write per key (an LWW map with tombstones).
//! - `schema`: which records a store's text holds, and the text the records give back.
//! - `replica`: a device; a change becomes the writes of its difference, nothing else.
//! - `codec`: how many bytes a write, a batch and a snapshot take.

pub mod codec;
pub mod doc;
pub mod hlc;
pub mod replica;
pub mod schema;
