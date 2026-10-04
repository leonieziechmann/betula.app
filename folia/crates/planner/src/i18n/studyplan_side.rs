//! Texts of the Stundenplan's sidebar (`pages/studyplan/side.rs`): the program, the view, what is
//! shown, the Standort, and the plan as a whole.

pub struct Texts {
    // ---- Studiengang ----
    /// „Studiengang": the group's heading and the picker's name.
    pub program: &'static str,
    /// „Mein Studiengang": the heading of the picker's first entry.
    pub mine: &'static str,
    /// The picker's text while nothing is picked, and its search field's.
    pub choose_program: &'static str,
    pub search_program: &'static str,
    /// „Studium planen →": the way to „Mein Studium", where the whole study is planned.
    pub plan_studies: &'static str,

    // ---- Ansicht, Zeigen, Standort ----
    /// „Ansicht": Woche · Termine · Prüfungen.
    pub view: &'static str,
    /// „Zeigen": a chip per kind of the semester …
    pub show: &'static str,
    /// … the tooltip of a chip whose kind is hidden, and of one that is shown …
    pub unhide: &'static str,
    pub hide: &'static str,
    /// … the line without any Termin …
    pub no_dates: &'static str,
    /// … and under the chips, where an event counts for several kinds (their labels joined with
    /// „/", how many): „„Vorlesung/Übung“ zählt als beides."
    pub combined: fn(&str, usize) -> String,
    /// „Standort": Cottbus · Senftenberg · „Beide".
    pub town: &'static str,
    pub both_towns: &'static str,

    // ---- Plan ----
    /// „Plan": the group's heading.
    pub plan: &'static str,
    /// The names „Plan speichern" suggests: the program with the Fachsemester last taken over
    /// („Informatik 1. FS") …
    pub name_fs: fn(&str, u8) -> String,
    /// … or „Plan 3" without a program.
    pub name_numbered: fn(usize) -> String,
    pub save_plan: &'static str,
    /// The tooltip of „Plan speichern" with nothing to save.
    pub nothing_to_save: &'static str,
    /// The name field's name, for screen readers.
    pub plan_name: &'static str,
    /// „Speichern", and „Ersetzen" where a saved plan has the name already (and for loading one
    /// over a plan no saved plan holds).
    pub save: &'static str,
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
    program: "Studiengang",
    mine: "Mein Studiengang",
    choose_program: "Studiengang wählen",
    search_program: "Studiengang suchen",
    plan_studies: "Studium planen →",

    view: "Ansicht",
    show: "Zeigen",
    unhide: "Einblenden",
    hide: "Ausblenden",
    no_dates: "Keine Termine im Plan.",
    combined: |labels, kinds| match kinds {
        2 => format!("„{labels}“ zählt als beides."),
        _ => format!("„{labels}“ zählt als jede dieser Arten."),
    },
    town: "Standort",
    both_towns: "Beide",

    plan: "Plan",
    name_fs: |program, fs| format!("{program} {fs}. FS"),
    name_numbered: |n| format!("Plan {n}"),
    save_plan: "Plan speichern",
    nothing_to_save: "Nichts zu speichern",
    plan_name: "Name des Plans",
    save: "Speichern",
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
    program: "Degree programme",
    mine: "My programme",
    choose_program: "Choose a degree programme",
    search_program: "Search degree programmes",
    plan_studies: "Plan your studies →",

    view: "View",
    show: "Show",
    unhide: "Show",
    hide: "Hide",
    no_dates: "No dates in the plan.",
    combined: |labels, kinds| match kinds {
        2 => format!("“{labels}” counts as both."),
        _ => format!("“{labels}” counts as each of these kinds."),
    },
    town: "Location",
    both_towns: "Both",

    plan: "Plan",
    name_fs: |program, fs| format!("{program} semester {fs}"),
    name_numbered: |n| format!("Plan {n}"),
    save_plan: "Save plan",
    nothing_to_save: "Nothing to save",
    plan_name: "Name of the plan",
    save: "Save",
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
