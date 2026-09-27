//! Texts of the pickers (`combobox.rs`). What a picker picks, its button's text and the
//! placeholder of its search field are its caller's.

pub struct Texts {
    /// The popup's line when the search finds no entry.
    pub nothing_found: &'static str,
    /// „23 weitere – tippe, um sie zu finden": the entries beyond those rendered.
    pub more: fn(usize) -> String,
    /// The keys at the foot of the popup, each word after its key: „↑↓ wählen, Enter übernehmen,
    /// Esc schließen".
    pub key_move: &'static str,
    pub key_take: &'static str,
    pub key_close: &'static str,
    /// The button beside the selection that clears it, for screen readers: „Studiengang: Auswahl
    /// aufheben"; the value is what is being picked.
    pub clear: fn(&str) -> String,
}

pub const DE: Texts = Texts {
    nothing_found: "Nichts gefunden",
    more: |n| format!("{n} weitere – tippe, um sie zu finden"),
    key_move: "wählen",
    key_take: "übernehmen",
    key_close: "schließen",
    clear: |label| format!("{label}: Auswahl aufheben"),
};

pub const EN: Texts = Texts {
    nothing_found: "Nothing found",
    more: |n| format!("{n} more – type to find them"),
    key_move: "select",
    key_take: "choose",
    key_close: "close",
    clear: |label| format!("{label}: clear the selection"),
};
