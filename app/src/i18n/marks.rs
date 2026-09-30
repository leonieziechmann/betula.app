//! Texts of „Merken" wherever a module can be marked (`bookmarks.rs`).

pub struct Texts {
    /// The switch's label: „Merken" while the module is not marked, „Gemerkt" once it is.
    pub save: &'static str,
    pub saved: &'static str,
    /// What the switch does, as its tooltip, with its shortcut: not marked yet …
    pub save_hint: &'static str,
    /// … and marked.
    pub saved_hint: &'static str,
    /// The icon alone at the end of a row, for screen readers: „Analysis I merken"; the value is
    /// the module's title.
    pub save_title: fn(&str) -> String,
    /// What a row swiped to the left uncovers on a marked module (`swipe.rs`): „Entfernen" …
    pub remove: &'static str,
    /// … with the line under it, where from …
    pub from_list: &'static str,
    /// … and „Entfernt" once it is done (a module marked by the swipe says `saved`).
    pub removed: &'static str,
}

pub const DE: Texts = Texts {
    save: "Merken",
    saved: "Gemerkt",
    save_hint: "Auf die Merkliste setzen (M)",
    saved_hint: "Gemerkt. Noch einmal nimmt das Modul von der Merkliste (M)",
    save_title: |title| format!("{title} merken"),
    remove: "Entfernen",
    from_list: "von der Merkliste",
    removed: "Entfernt",
};

pub const EN: Texts = Texts {
    save: "Save",
    saved: "Saved",
    save_hint: "Add to your saved modules (M)",
    saved_hint: "Saved. Once more removes the module from your saved modules (M)",
    save_title: |title| format!("Save {title}"),
    remove: "Remove",
    from_list: "from saved modules",
    removed: "Removed",
};
