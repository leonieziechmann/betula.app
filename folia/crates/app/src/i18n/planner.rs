//! Texts of the Stundenplan's store and its buttons on other pages (`studyplan.rs`): „Einplanen"
//! beside „Merken", and „Anderes Semester" in the sidebar of a module's page.

pub struct Texts {
    /// The switch's label: „Einplanen" while the module is not planned where it aims, „Eingeplant"
    /// once it is.
    pub plan: &'static str,
    pub planned: &'static str,
    /// „für „Anwendungsfach“": the placeholder of the Stundenplan the switch plans the module
    /// for; the value is its name.
    pub for_placeholder: fn(&str) -> String,
    /// „geplant: SoSe 2027": the other semesters the module is planned in, written already.
    pub planned_in: fn(&str) -> String,
    /// What a click does, as the switch's tooltip (its shortcut follows): take the module out of
    /// the semester (the value) …
    pub unplan_hint: fn(&str) -> String,
    /// … plan it there for a placeholder (semester, placeholder's name) …
    pub plan_for_hint: fn(&str, &str) -> String,
    /// … and plan it there.
    pub plan_hint: fn(&str) -> String,
    /// „Anderes Semester": the button that opens the list of semesters in the sidebar …
    pub other_semester: &'static str,
    /// … the list's name for screen readers („Analysis I: Semester"; the value is the module's
    /// title) …
    pub semesters_of: fn(&str) -> String,
    /// … „3. FS · WiSe 2027/28": a semester of it, with the student's semester of study where the
    /// start of the studies is known (the number and the semester, written already) …
    pub semester_of_study: fn(u8, &str) -> String,
    /// … and the mark at the end of a semester the module is planned in.
    pub planned_mark: &'static str,
    /// What a row swiped to the right uncovers on a module planned where it aims (`swipe.rs`):
    /// „Entfernen" …
    pub remove: &'static str,
    /// … with the line under it, the semester it takes the module out of (the value) …
    pub out_of: fn(&str) -> String,
    /// … and „Entfernt" once it is done (a module planned by the swipe says `planned`).
    pub removed: &'static str,
}

pub const DE: Texts = Texts {
    plan: "Einplanen",
    planned: "Eingeplant",
    for_placeholder: |name| format!("für „{name}“"),
    planned_in: |semesters| format!("geplant: {semesters}"),
    unplan_hint: |semester| format!("Eingeplant in {semester}. Noch einmal nimmt das Modul aus dem Plan"),
    plan_for_hint: |semester, name| format!("In {semester} einplanen, für „{name}“"),
    plan_hint: |semester| format!("In {semester} einplanen"),
    other_semester: "Anderes Semester",
    semesters_of: |title| format!("{title}: Semester"),
    semester_of_study: |n, semester| format!("{n}. FS · {semester}"),
    planned_mark: "geplant",
    remove: "Entfernen",
    out_of: |semester| format!("aus {semester}"),
    removed: "Entfernt",
};

pub const EN: Texts = Texts {
    plan: "Plan",
    planned: "Planned",
    for_placeholder: |name| format!("for \u{201c}{name}\u{201d}"),
    planned_in: |semesters| format!("planned: {semesters}"),
    unplan_hint: |semester| format!("Planned for {semester}. Once more removes the module from the plan"),
    plan_for_hint: |semester, name| format!("Plan for {semester}, for \u{201c}{name}\u{201d}"),
    plan_hint: |semester| format!("Plan for {semester}"),
    other_semester: "Another semester",
    semesters_of: |title| format!("{title}: semesters"),
    semester_of_study: |n, semester| format!("{} sem. · {semester}", folia_design::i18n::format::ordinal(i64::from(n))),
    planned_mark: "planned",
    remove: "Remove",
    out_of: |semester| format!("from {semester}"),
    removed: "Removed",
};
