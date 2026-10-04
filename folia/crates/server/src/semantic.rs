//! The browser's model of the semantic search (folia/crates/semantic/README.md): the packed encoder of the
//! queries (`e5-de-en.bin`, 15 MB), which the browser loads after the app has started and keeps in
//! a cache of its own (`folia/assets/boot.js`, `folia/assets/sw.js`). The modules' vectors are in the
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
    /// Radix's id of the passage model this query model was made for (`--semantic-passage-model`),
    /// when it is known: the browser compares it with the snapshot's `semantic_model`.
    pub passage: Option<String>,
}

fn invalid(message: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

impl Model {
    /// Reads the packed model at `path` (`FOLIA_SEMANTIC_MODEL`); a file that is not one
    /// (`E5Q1`, folia/crates/semantic/src/lib.rs) is refused, and so is one named by a sha256 (the server's
    /// model store, deploy/models.lock) that is not the one of its content: a damaged file.
    /// `passage` is Radix's id of the passage model it belongs to: 16 hex digits.
    pub fn load(path: &Path, passage: Option<&str>) -> std::io::Result<Model> {
        if let Some(passage) = passage {
            if passage.len() != 16 || !passage.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                return Err(invalid(format!("the passage model '{passage}' is not Radix's id of a model (16 hex digits)")));
            }
        }
        let bytes = std::fs::read(path)?;
        if !bytes.starts_with(b"E5Q1") {
            return Err(invalid("not a packed model of the semantic search (E5Q1)".into()));
        }
        let sum = hex(&Sha256::digest(&bytes));
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if name.len() == 64 && name.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) && sum != name {
            return Err(invalid(format!("the content's sha256 is {sum}, not the one its name says: a damaged file")));
        }
        Ok(Model { passage: passage.map(str::to_string), ..Model::with_sum(Bytes::from(bytes), &sum) })
    }

    pub fn of(bytes: Bytes) -> Model {
        let sum = hex(&Sha256::digest(&bytes));
        Model::with_sum(bytes, &sum)
    }

    fn with_sum(bytes: Bytes, sum: &str) -> Model {
        let hash = sum.get(..16).unwrap_or(sum);
        Model { path: format!("{PREFIX}e5-de-en-{hash}.bin"), etag: format!("\"{hash}\""), body: Kept::new(bytes), passage: None }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
