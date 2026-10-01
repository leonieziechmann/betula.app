//! The browser's model of the semantic search (semantic/README.md): the packed encoder of the
//! queries (`e5-de-en.bin`, 15 MB), which the browser loads after the app has started and keeps in
//! a cache of its own (`app/assets/boot.js`, `app/assets/sw.js`). The modules' vectors are in the
//! snapshot; the model is the one file of the search that is not.
//!
//! Its address names its content (`/models/e5-de-en-<hash>.bin`): it changes only when the model
//! does, not with every build, so a browser downloads it once per model and keeps it for good.

use std::path::Path;

use axum::body::Bytes;
use sha2::{Digest, Sha256};

use crate::encoding::Kept;

/// Where the model is served, before its name.
pub const PREFIX: &str = "/models/";

/// The model as it is served: read once at the start and kept in memory, with its compressed
/// forms (`Kept`, brotli made once per process by the warm-up, `warm::files`).
pub struct Model {
    /// Its address: `/models/e5-de-en-<the first 16 hex digits of its SHA-256>.bin`.
    pub path: String,
    pub etag: String,
    pub body: Kept,
}

impl Model {
    /// Reads the packed model at `path` (`FOLIA_SEMANTIC_MODEL`); a file that is not one
    /// (`E5Q1`, semantic/src/lib.rs) is refused.
    pub fn load(path: &Path) -> std::io::Result<Model> {
        let bytes = std::fs::read(path)?;
        if !bytes.starts_with(b"E5Q1") {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "not a packed model of the semantic search (E5Q1)"));
        }
        Ok(Model::of(Bytes::from(bytes)))
    }

    pub fn of(bytes: Bytes) -> Model {
        let hash: String = Sha256::digest(&bytes).iter().take(8).map(|b| format!("{b:02x}")).collect();
        Model { path: format!("{PREFIX}e5-de-en-{hash}.bin"), etag: format!("\"{hash}\""), body: Kept::new(bytes) }
    }
}
