//! Texts of the Stundenplan's modules (`pages/studyplan/modules.rs`): the list of the planned
//! modules beside the week, their placeholders and „Modul hinzufügen". What a placeholder is
//! called is the data contract's (`folia_plans::i18n`), the unit of credits the app's
//! (`common`).

pub struct Texts {
    /// The heading of the list („Module") and its name for screen readers („Module im
    /// Stundenplan").
    pub heading: &'static str,
    pub list: &'static str,
    /// A planned module the catalog has no title for, by its number: „Modul 12104".
    pub module_numbered: fn(&str) -> String,
    /// Under a module's title: the catalog does not know it („nicht im Modulkatalog"), or the
    /// semester has dates, the module none („keine Termine").
    pub not_in_catalog: &'static str,
    pub no_dates: &'static str,
    /// The placeholder of another semester a module counts for: „für „Anwendungsfach“".
    pub counts_for: fn(&str) -> String,
    /// The × at the end of a module's row: its tooltip, and its name with the module's title
    /// („„Analysis I“ aus dem Stundenplan nehmen").
    pub remove: &'static str,
    pub remove_named: fn(&str) -> String,
    /// A module just taken out, in its row's place until the plan changes otherwise: „aus dem
    /// Stundenplan genommen".
    pub removed: &'static str,
    /// A row of the Regelstudienplan over several Fachsemester: „5.–6. FS".
    pub fs_span: fn(u8, u8) -> String,
    /// A placeholder nothing counts for yet, under its name: „Platzhalter · Modul finden" (the
    /// words joined with „ · ", a span of Fachsemester between them).
    pub placeholder: &'static str,
    pub find_module: &'static str,
    /// What counts for a placeholder so far, while it takes more (the credits written already):
    /// „12 LP geplant".
    pub credits_planned: fn(&str) -> String,
    /// The box of a placeholder something counts for, to screen readers: „Bereich „Anwendungsfach“".
    pub area: fn(&str) -> String,
    /// The link of a box whose row takes more: „Weiteres Modul".
    pub another_module: &'static str,
    /// The last row of the list, to the catalog's modules that fit the week: „Modul hinzufügen".
    pub add_module: &'static str,
}

pub const DE: Texts = Texts {
    heading: "Module",
    list: "Module im Stundenplan",
    module_numbered: |id| format!("Modul {id}"),
    not_in_catalog: "nicht im Modulkatalog",
    no_dates: "keine Termine",
    counts_for: |name| format!("für „{name}“"),
    remove: "Aus dem Stundenplan nehmen",
    remove_named: |title| format!("„{title}“ aus dem Stundenplan nehmen"),
    removed: "aus dem Stundenplan genommen",
    fs_span: |first, last| format!("{first}.–{last}.\u{a0}FS"),
    placeholder: "Platzhalter",
    find_module: "Modul\u{a0}finden",
    credits_planned: |credits| format!("{credits}\u{a0}LP geplant"),
    area: |name| format!("Bereich „{name}“"),
    another_module: "Weiteres Modul",
    add_module: "Modul hinzufügen",
};

pub const EN: Texts = Texts {
    heading: "Modules",
    list: "Modules in the timetable",
    module_numbered: |id| format!("Module {id}"),
    not_in_catalog: "not in the module catalogue",
    no_dates: "no dates",
    counts_for: |name| format!("for “{name}”"),
    remove: "Remove from the timetable",
    remove_named: |title| format!("Remove “{title}” from the timetable"),
    removed: "removed from the timetable",
    fs_span: |first, last| format!("semesters\u{a0}{first}–{last}"),
    placeholder: "Placeholder",
    find_module: "Find\u{a0}a\u{a0}module",
    credits_planned: |credits| format!("{credits}\u{a0}CP planned"),
    area: |name| format!("Area “{name}”"),
    another_module: "Another module",
    add_module: "Add a module",
};
