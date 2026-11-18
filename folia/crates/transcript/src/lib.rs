//! The BTU's Notenübersicht („Transcript of Records") read in the browser (owner, 2026-11-18: „Das
//! soll strikt im Browser laufen die datei darf das Endnutzer Gerät nicht verlassen und soll auch
//! nicht irgendwie persistent gespeichert werden. Nur einmal die module extrahieren+status und dann
//! datei vergessen").
//!
//! `read` takes the PDF as the BTU writes it (`pdf`: its objects, RC4 with the empty user
//! password, its fonts, the text and the rules of its pages) and reads its table (`table`): the
//! program the sentence above it names and since when, and every module with its status, its
//! credits, its attempt and its semester. Nothing else of the file is read: not the name, the
//! address, the date of birth, the grades or the code that verifies it.
//!
//! Which catalog module each row is, `matching` says from its titles; it runs where the catalog is
//! (`folia_pages::transcript`, in the data worker).
//!
//! In the browser it runs in a Web Worker of its own (`wasm`, `js/worker.js`), which the page
//! starts for one file and ends once it has answered: the file is read into the worker's memory and
//! nowhere else, and its memory goes with it. What it answers is `Transcript::lines`, read back on
//! the page by `Transcript::from_lines`. The crate has no dependencies, as the semantic search's.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic))]

#[cfg(any(test, feature = "fixture"))]
pub mod fixture;
pub mod matching;
pub mod pdf;
pub mod table;
#[cfg(all(feature = "worker", target_arch = "wasm32"))]
mod wasm;

pub use table::{Date, Entry, Status, Term, Transcript};

/// Why a file was not read as a Notenübersicht.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a PDF file.
    NotPdf,
    /// A PDF that opens only with a password, or is encrypted in a way not read here.
    Locked,
    /// A PDF, but no Notenübersicht of the BTU: no table of modules in it.
    NotTranscript,
}

impl Error {
    /// Its word in `Transcript::lines` and in what the worker answers.
    pub fn code(self) -> &'static str {
        match self {
            Error::NotPdf => "not-pdf",
            Error::Locked => "locked",
            Error::NotTranscript => "not-transcript",
        }
    }

    pub fn from_code(code: &str) -> Option<Error> {
        [Error::NotPdf, Error::Locked, Error::NotTranscript].into_iter().find(|error| error.code() == code)
    }
}

/// The Notenübersicht in the PDF `data`.
pub fn read(data: &[u8]) -> Result<Transcript, Error> {
    let document = pdf::Document::open(data).map_err(|refusal| match refusal {
        pdf::Refusal::NotPdf => Error::NotPdf,
        pdf::Refusal::Password | pdf::Refusal::Encryption => Error::Locked,
    })?;
    let pages: Vec<pdf::content::Page> = document.pages().iter().map(|page| pdf::content::read(&document, page)).collect();
    if pages.is_empty() {
        return Err(Error::NotPdf);
    }
    table::read(&pages).ok_or(Error::NotTranscript)
}
