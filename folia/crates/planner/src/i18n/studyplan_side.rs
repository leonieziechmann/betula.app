//! Texts of the Stundenplan's sidebar (`pages/studyplan/side.rs`): the plan as a whole.

pub struct Texts {
    /// „Plan": the group's heading.
    pub plan: &'static str,
    /// „Ersetzen": loading a saved plan over a plan no saved plan holds.
    pub replace: &'static str,
    pub cancel: &'static str,
    /// „„Für Lea“ löschen": × of a saved plan (its name), and its tooltip.
    pub delete_named: fn(&str) -> String,
    pub delete: &'static str,
    /// „Aktuellen Plan ersetzen?": loading a saved plan would lose the one shown.
    pub replace_current: &'static str,
    /// „Plan leeren", the question it asks, its answer, and the tooltip with nothing planned.
    pub clear_plan: &'static str,
    pub really_clear: &'static str,
    pub clear: &'static str,
    pub nothing_planned: &'static str,
    /// „Plan geleert": the note „Rückgängig" answers. It is the whole note (`PlanCtx::undo`), so it
    /// must differ from the other notes' beginnings (`studyplan_import::imported`,
    /// `studyplan_share::taken`).
    pub cleared: &'static str,
}

pub const DE: Texts = Texts {
    plan: "Plan",
    replace: "Ersetzen",
    cancel: "Abbrechen",
    delete_named: |name| format!("„{name}“ löschen"),
    delete: "Löschen",
    replace_current: "Aktuellen Plan ersetzen?",
    clear_plan: "Plan leeren",
    really_clear: "Wirklich leeren?",
    clear: "Leeren",
    nothing_planned: "Nichts geplant",
    cleared: "Plan geleert",
};

pub const EN: Texts = Texts {
    plan: "Plan",
    replace: "Replace",
    cancel: "Cancel",
    delete_named: |name| format!("Delete “{name}”"),
    delete: "Delete",
    replace_current: "Replace the current plan?",
    clear_plan: "Clear plan",
    really_clear: "Really clear it?",
    clear: "Clear",
    nothing_planned: "Nothing planned",
    cleared: "Plan cleared",
};
