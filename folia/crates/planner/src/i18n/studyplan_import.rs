//! Texts of taking a Regelstudienplan over into the Stundenplan (`pages/studyplan/import.rs`).

pub struct Texts {
    /// „Importieren": the group's heading.
    pub import: &'static str,
    /// The source to import from, for screen readers: „Quelle" …
    pub source: &'static str,
    /// … „Regelstudienplan" …
    pub standard_plan: &'static str,
    /// … and „Mein Studium" (its Wiederholer: what its semesters hold is this timetable already),
    /// with what it takes over as its tooltip and the line under it …
    pub my_studies: &'static str,
    pub my_studies_hint: &'static str,
    /// … where no program is kept, or the study begins after the semester shown …
    pub no_studies: &'static str,
    pub study_not_yet: &'static str,
    /// … and the way there.
    pub to_studies: &'static str,
    /// Without a program, and for a program without a plan.
    pub choose_program_first: &'static str,
    pub no_plan: &'static str,
    /// The names of the selects, for screen readers: the plan where a program has several, and
    /// the Fachsemester …
    pub plan: &'static str,
    pub fachsemester: &'static str,
    /// … whose entries read „1. FS".
    pub fs: fn(u8) -> String,
    /// „Übernehmen".
    pub take: &'static str,
    /// What the import would add, under the button: „4 Module · 1 Platzhalter" (the parts written
    /// already, `studyplan_head::placeholders` among them), or nothing …
    pub nothing_to_take: &'static str,
    /// … because the timetable holds all of it already.
    pub already: &'static str,
    /// „Übernommen: 4 Module, 1 Platzhalter": how the note after „Übernehmen" begins, which the
    /// note of a plan taken over from a link shares after its own beginning …
    pub imported: &'static str,
    /// … and „nichts" where nothing was taken over.
    pub nothing: &'static str,
}

pub const DE: Texts = Texts {
    import: "Importieren",
    source: "Quelle",
    standard_plan: "Regelstudienplan",
    my_studies: "Mein Studium",
    my_studies_hint: "Deine Wiederholer aus „Mein Studium“: was du nicht bestanden und noch nicht wieder eingeplant hast. Was du dort planst, steht schon in diesem Stundenplan.",
    no_studies: "Wähle zuerst in „Mein Studium“ deinen Studiengang.",
    study_not_yet: "„Mein Studium“ beginnt erst nach diesem Semester.",
    to_studies: "Zu „Mein Studium“ →",
    choose_program_first: "Erst einen Studiengang wählen.",
    no_plan: "Kein Regelstudienplan für diesen Studiengang.",
    plan: "Plan",
    fachsemester: "Fachsemester",
    fs: |n| format!("{n}. FS"),
    take: "Übernehmen",
    nothing_to_take: "Nichts zu übernehmen",
    already: "Schon im Plan",
    imported: "Übernommen: ",
    nothing: "nichts",
};

pub const EN: Texts = Texts {
    import: "Import",
    source: "Source",
    standard_plan: "Standard study plan",
    my_studies: "My studies",
    my_studies_hint: "Your retakes from \u{201c}My studies\u{201d}: what you have not passed and not planned again. What you plan there is in this timetable already.",
    no_studies: "Choose your degree programme in \u{201c}My studies\u{201d} first.",
    study_not_yet: "\u{201c}My studies\u{201d} begins after this semester.",
    to_studies: "To \u{201c}My studies\u{201d} →",
    choose_program_first: "Choose a degree programme first.",
    no_plan: "No standard study plan for this degree programme.",
    plan: "Plan",
    fachsemester: "Semester",
    fs: |n| format!("Semester {n}"),
    take: "Import",
    nothing_to_take: "Nothing to import",
    already: "Already in the plan",
    imported: "Imported: ",
    nothing: "nothing",
};
