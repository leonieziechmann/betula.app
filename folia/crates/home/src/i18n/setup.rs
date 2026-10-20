//! Texts of the start page's setup (`home/setup.rs`, concept A „Einrichtung"): three steps on one
//! card, the program, the semester the study began in, and the modules of the Fachsemester that
//! follows from it. What a step says follows the usual case and refuses nothing (CLAUDE.md, „Betula
//! does not say no"): an unusual choice is said beside it.

pub struct Texts {
    /// The steps, as the chips at the top of the card name them.
    pub step_program: &'static str,
    pub step_start: &'static str,
    pub step_semester: &'static str,
    /// „Studium einrichten: Schritt 2 von 3", for screen readers.
    pub steps_label: &'static str,
    /// A step done, for screen readers before what it holds.
    pub done: &'static str,

    // Step 1
    pub program_title: &'static str,
    pub program_lead: &'static str,
    /// „148 Studiengänge in 6 Fakultäten, jeder ein Punkt".
    pub drawing: fn(usize, usize) -> String,
    /// The faculties of the drawing's key, by their number: a short name each.
    pub faculty: fn(&str) -> String,
    pub across_faculties: &'static str,
    pub look_around: &'static str,
    pub look_around_title: &'static str,
    pub take_profile: &'static str,
    pub take_profile_title: &'static str,

    // Step 2
    pub start_title: &'static str,
    pub start_lead: &'static str,
    /// „Ich fange gerade an", „jetzt im 3. Fachsemester".
    pub starting: &'static str,
    pub now_in: fn(u8) -> String,
    /// „3." and „FS" of the number at the right.
    pub fs_short: &'static str,
    /// What is unusual about a semester: „über der Regelstudienzeit (6 Sem.)", „vor der PO 2008".
    pub beyond_regular: fn(u8) -> String,
    pub before_po: fn(&str) -> String,
    pub other_fs: &'static str,
    pub go_on: &'static str,
    pub fs_range: &'static str,
    /// The custom field's semester, said under it: „Beginn im WiSe 2023/24".
    pub began_in: fn(&str) -> String,

    // Step 3
    /// „Dein 1. Fachsemester".
    pub semester_title: fn(u8) -> String,
    /// „Im WiSe 2026/27 laut Regelstudienplan: 5 Module, 32 LP. Abwählen, was du nicht belegst."
    pub semester_lead: fn(&str, usize, &str) -> String,
    pub semester_lead_empty: fn(&str) -> String,
    /// A row whose kind no source states.
    pub module: &'static str,
    pub placeholder: &'static str,
    /// „FS 3–4": the Fachsemester a row spans.
    pub span: fn(u8, u8) -> String,
    pub direction: &'static str,
    pub no_plan: &'static str,
    pub beyond_plan: fn(u8) -> String,
    pub past_switch: &'static str,
    /// What „Befehle" says of the switch while it is on (R25).
    pub switch_on: &'static str,
    /// „4 Module, 28 LP aus FS 1", „aus FS 1–2".
    pub past_detail: fn(usize, &str, u8) -> String,
    pub past_none: &'static str,
    pub without_modules: &'static str,
    pub create: &'static str,
    pub back: &'static str,
}

pub const DE: Texts = Texts {
    step_program: "Studiengang",
    step_start: "Beginn",
    step_semester: "Semester",
    steps_label: "Schritte der Einrichtung",
    done: "Erledigt: ",

    program_title: "Was studierst du?",
    program_lead: "In drei Klicks zu deinem Stundenplan. Ohne Konto, alles bleibt in diesem Browser.",
    drawing: |programs, faculties| format!("{programs} Studiengänge in {faculties} Fakultäten, jeder ein Punkt"),
    faculty: |code| format!("Fakultät {code}"),
    across_faculties: "übergreifend",
    look_around: "Erst mal umschauen",
    look_around_title: "Die Einrichtung einklappen; sie bleibt oben erreichbar",
    take_profile: "Profil übernehmen",
    take_profile_title: "Schon eingerichtet? Ein Profil als Datei von einem anderen Gerät übernehmen",

    start_title: "Seit wann studierst du?",
    start_lead: "Daraus folgt dein Fachsemester.",
    starting: "Ich fange gerade an",
    now_in: |fs| format!("jetzt im {fs}. Fachsemester"),
    fs_short: "FS",
    beyond_regular: |semesters| format!("über der Regelstudienzeit ({semesters}\u{a0}Sem.)"),
    before_po: |year| format!("vor der PO {year}"),
    other_fs: "Anderes Fachsemester",
    go_on: "Weiter",
    fs_range: "Eine Zahl von 1 bis 30: so weit zählt Betula Fachsemester.",
    began_in: |semester| format!("Beginn im {semester}"),

    semester_title: |fs| format!("Dein {fs}. Fachsemester"),
    semester_lead: |semester, modules, credits| format!(
        "Im {semester} laut Regelstudienplan: {modules} {}, {credits}\u{a0}LP. Abwählen, was du nicht belegst.",
        if modules == 1 { "Modul" } else { "Module" }
    ),
    semester_lead_empty: |semester| format!("Im {semester}. Dein Stundenplan startet leer."),
    module: "Modul",
    placeholder: "als Platzhalter, später wählen",
    span: |from, to| if from == to { format!("FS\u{a0}{from}") } else { format!("FS\u{a0}{from}–{to}") },
    direction: "Studienrichtung",
    no_plan: "Für diesen Studiengang hat Betula keinen geprüften Regelstudienplan. Dein Stundenplan startet leer; Module planst du aus dem Katalog.",
    beyond_plan: |last| format!("Der Regelstudienplan endet mit dem {last}. Fachsemester. Dein Stundenplan startet leer."),
    past_switch: "Bisherige Module als bestanden markieren",
    switch_on: "an",
    past_detail: |modules, credits, last| match last {
        1 => format!("{modules} {}, {credits}\u{a0}LP aus FS\u{a0}1", if modules == 1 { "Modul" } else { "Module" }),
        last => format!("{modules} {}, {credits}\u{a0}LP aus FS\u{a0}1–{last}", if modules == 1 { "Modul" } else { "Module" }),
    },
    past_none: "Im 1. Fachsemester gibt es noch nichts zu markieren.",
    without_modules: "Ohne Module weiter",
    create: "Studium anlegen",
    back: "Zurück",
};

pub const EN: Texts = Texts {
    step_program: "Programme",
    step_start: "Start",
    step_semester: "Semester",
    steps_label: "Steps of the set-up",
    done: "Done: ",

    program_title: "What do you study?",
    program_lead: "Three clicks to your timetable. No account, everything stays in this browser.",
    drawing: |programs, faculties| format!("{programs} degree programmes in {faculties} faculties, one dot each"),
    faculty: |code| format!("Faculty {code}"),
    across_faculties: "across faculties",
    look_around: "Look around first",
    look_around_title: "Fold the set-up away; it stays within reach at the top",
    take_profile: "Bring a profile",
    take_profile_title: "Set up elsewhere already? Bring a profile from another device as a file",

    start_title: "When did you start?",
    start_lead: "That gives your semester of study.",
    starting: "I'm just starting",
    now_in: |fs| format!("now in semester {fs}"),
    fs_short: "sem.",
    beyond_regular: |semesters| format!("beyond the standard period ({semesters}\u{a0}sem.)"),
    before_po: |year| format!("before the {year} regulations"),
    other_fs: "Another semester",
    go_on: "Continue",
    fs_range: "A number from 1 to 30: Betula counts semesters that far.",
    began_in: |semester| format!("Started in {semester}"),

    semester_title: |fs| format!("Your semester {fs}"),
    semester_lead: |semester, modules, credits| format!(
        "In {semester} by the standard study plan: {modules} {}, {credits}\u{a0}ECTS. Untick what you do not take.",
        if modules == 1 { "module" } else { "modules" }
    ),
    semester_lead_empty: |semester| format!("In {semester}. Your timetable starts empty."),
    module: "Module",
    placeholder: "as a placeholder, choose later",
    span: |from, to| if from == to { format!("sem.\u{a0}{from}") } else { format!("sem.\u{a0}{from}–{to}") },
    direction: "Study direction",
    no_plan: "Betula has no checked standard study plan for this programme. Your timetable starts empty; you plan modules from the catalogue.",
    beyond_plan: |last| format!("The standard study plan ends with semester {last}. Your timetable starts empty."),
    past_switch: "Mark earlier modules as passed",
    switch_on: "on",
    past_detail: |modules, credits, last| match last {
        1 => format!("{modules} {}, {credits}\u{a0}ECTS of semester\u{a0}1", if modules == 1 { "module" } else { "modules" }),
        last => format!("{modules} {}, {credits}\u{a0}ECTS of semesters\u{a0}1–{last}", if modules == 1 { "module" } else { "modules" }),
    },
    past_none: "In the first semester there is nothing to mark yet.",
    without_modules: "Continue without modules",
    create: "Set up my studies",
    back: "Back",
};
