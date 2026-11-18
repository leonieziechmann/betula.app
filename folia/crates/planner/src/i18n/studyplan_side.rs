//! Texts of the Stundenplan's sidebar (`pages/studyplan/side.rs`): the plans saved before.

pub struct Texts {
    /// „Ersetzen": loading a saved plan over a plan no saved plan holds.
    pub replace: &'static str,
    pub cancel: &'static str,
    /// „„Für Lea“ löschen": the bin of a saved plan (its name), and its tooltip.
    pub delete_named: fn(&str) -> String,
    pub delete: &'static str,
    /// „Aktuellen Plan ersetzen?": loading a saved plan would lose the one shown.
    pub replace_current: &'static str,
    /// „„Für Lea“ geladen", with „Rückgängig": a saved plan took the semester's place.
    pub loaded: fn(&str) -> String,
}

pub const DE: Texts = Texts {
    replace: "Ersetzen",
    cancel: "Abbrechen",
    delete_named: |name| format!("„{name}“ löschen"),
    delete: "Löschen",
    replace_current: "Aktuellen Plan ersetzen?",
    loaded: |name| format!("„{name}“ geladen"),
};

pub const EN: Texts = Texts {
    replace: "Replace",
    cancel: "Cancel",
    delete_named: |name| format!("Delete “{name}”"),
    delete: "Delete",
    replace_current: "Replace the current plan?",
    loaded: |name| format!("“{name}” loaded"),
};
