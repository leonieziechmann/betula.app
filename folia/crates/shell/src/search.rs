//! What is typed into the search at the head on a page that searches itself and keeps the text out
//! of its address: the Merkliste (folia/design/merken/oberflaeche.html, „Suchen, ordnen, filtern").
//!
//! The search at the head belongs to the page it is on (`chrome::TopBar`): the catalog's text is
//! part of the catalog's address, the programs' of the overview's. What is typed on the Merkliste
//! searches the marked modules and, with them, the visitor's own notes and collections, so it may
//! name what only a note of the visitor's says: it never goes into a URL, which is requested from
//! the server and ends up in its logs (R20). It lives here, for the session of the tab, and the
//! page reads it as it reads its address.

use leptos::prelude::*;

/// The text of the Merkliste's search, shared through context. Empty on the server.
#[derive(Clone, Copy)]
pub struct LocalSearch(RwSignal<String>);

impl LocalSearch {
    pub fn provide() -> Self {
        let search = LocalSearch(RwSignal::new(String::new()));
        provide_context(search);
        search
    }

    pub fn expect() -> Option<Self> {
        use_context::<Self>()
    }

    /// The text as typed. Tracked.
    pub fn text(self) -> String {
        self.0.get()
    }

    pub fn text_untracked(self) -> String {
        self.0.get_untracked()
    }

    /// What is typed now; the same text again changes nothing (a page that reads it asks once).
    pub fn set(self, text: String) {
        if self.0.with_untracked(|now| *now != text) {
            self.0.set(text);
        }
    }
}
