//! Texts of a study program's page (`pages/program.rs`). The names of its views are the labels of
//! `folia_routes::url::ProgramTab`.

pub struct Texts {
    /// The heading of the sidebar.
    pub program: &'static str,
    pub not_found_title: &'static str,
    pub not_found_hint: &'static str,
    /// What link previews say of the page: „Informatik (B.Sc., PO 2008) an der BTU …: 42 Module,
    /// Regelstudienplan, …" (name, degree, PO, modules) …
    pub seo_description: fn(&str, &str, &str, i64) -> String,
    /// … and on the plan of one study track (name, degree, PO, track, modules).
    pub seo_description_track: fn(&str, &str, &str, &str, i64) -> String,
    /// The name of the view of the plan of one study track: „Regelstudienplan Produktionstechnik".
    pub plan_of: fn(&str) -> String,
    /// The name of the view of the areas in the page's title („Wahlpflicht und Bereiche").
    pub areas_title: &'static str,
    /// „Regelstudienplan": the plan of the regulations, as a word.
    pub plan: &'static str,
    /// „Bereich": an area of the program's module tree, as a word.
    pub area: &'static str,

    // The sidebar.
    /// The link to the catalog narrowed down to what is picked on the page.
    pub in_catalog: &'static str,
    pub views_label: &'static str,
    pub views: &'static str,
    /// How the plan is drawn: its heading, its name for screen readers, and why the matrix is not
    /// there.
    pub shape: &'static str,
    pub shape_label: &'static str,
    pub matrix: &'static str,
    pub list: &'static str,
    pub too_narrow: &'static str,
    pub areas_label: &'static str,
    pub areas: &'static str,
    pub related: &'static str,
    /// „Passender Master": the program of the other degree level.
    pub counterpart: fn(&str) -> String,
    /// „ (aktuell)" after the newest PO of the program.
    pub current_po: &'static str,
    pub actions: &'static str,
    /// Takes the plan shown into the Stundenplan.
    pub to_studyplan: &'static str,
    pub documents: &'static str,
    /// The program's page in the BTU's directory.
    pub at_btu: &'static str,

    // The head of the page.
    pub crumbs: &'static str,
    /// The facts of the head: how long, how big, what for („6 Semester", „180 LP"); the number is
    /// written in front of them.
    pub semesters_title: &'static str,
    pub semesters_after: fn(&str) -> &'static str,
    pub credits_title: &'static str,
    /// „42 Module", „12 FÜS-Module": what follows the number.
    pub modules_after: fn(i64) -> &'static str,
    pub fues_modules_after: fn(i64) -> &'static str,
    /// „Prüfungsordnung 2008": what stands before the version of the PO.
    pub regulations: &'static str,
    pub current: &'static str,
    pub older: &'static str,

    // The study plan.
    pub no_plan: &'static str,
    pub no_plan_hint: &'static str,
    /// Where the plan comes from: „Aus der Prüfungs- und Studienordnung übernommen und geprüft am
    /// 19.09.2026." (the date written already, if known) …
    pub taken_from: fn(Option<&str>) -> String,
    /// … and where it stands there: „ Dort auf Seite 9, unter „Studienrichtung A"." (the pages,
    /// the heading if the document prints one).
    pub found_at: fn(&str, Option<&str>) -> String,
    /// „Seite 9", „Seiten 9–11": the pages in `found_at`.
    pub page: &'static str,
    pub pages: &'static str,
    pub track: &'static str,
    /// The heads of the matrix.
    pub module: &'static str,
    pub kind: &'static str,
    pub credits_per_semester: &'static str,
    pub credits_differ: &'static str,
    pub no_semester_title: &'static str,
    pub total: &'static str,
    /// The notes under the matrix.
    pub stated_note: &'static str,
    pub spread_note: &'static str,
    pub differs_note: &'static str,
    /// A row the plan places in no semester: „Semester 5-6" as the plan writes it, or nothing.
    pub semester_as_written: fn(&str) -> String,
    pub no_semester: &'static str,
    /// The group of rows the plan names no area for.
    pub no_area: &'static str,
    /// „17 Module" at the head of a group.
    pub modules: fn(usize) -> String,
    /// A row that names no module: what its link opens.
    pub row_title: &'static str,

    // The panels beside the page.
    pub picked_not_found_title: &'static str,
    pub picked_not_found_hint: &'static str,
    pub to_program: &'static str,
    pub preview: &'static str,
    pub module_not_found_title: &'static str,
    pub module_not_found_hint: &'static str,
    pub close_preview: &'static str,
    /// „Schließen (Esc)".
    pub close_title: &'static str,
    /// The panel of a row of the plan.
    pub row: &'static str,
    /// The way into the catalog for a FÜS row, a single module, a row the name of which points at
    /// areas, and any other row.
    pub fues_in_catalog: &'static str,
    pub search_catalog: &'static str,
    pub fitting_in_catalog: &'static str,
    pub electives_in_catalog: &'static str,
    /// „Vermutlich " in front of the area the name of the row points at.
    pub probably: &'static str,
    pub whole_area: &'static str,
    pub fitting_areas: &'static str,
    pub also_possible: &'static str,
    pub program_areas: &'static str,
    pub all_areas: &'static str,
    /// Rows the regulation sums together: the heading …
    pub together: &'static str,
    /// … and what it says: how many other rows, the line's label, the credits together, the
    /// semesters (`choice_one`, `choice_many`), the least and the most each row allows (the
    /// numbers written already).
    pub choice_hint: fn(usize, &str, &str, &str, &str, &str) -> String,
    pub choice_one: fn(i64) -> String,
    pub choice_many: fn(i64, i64) -> String,
    /// What the plan states about a row that names no module of the catalog.
    pub note_one_module: &'static str,
    pub note_fues: &'static str,
    pub note_ambiguous: &'static str,
    pub note_fitting: &'static str,
    pub note_none: &'static str,
    /// The panel of an area.
    pub areas_within: &'static str,
    pub modules_heading: &'static str,

    // The tables of modules.
    pub number: &'static str,
    pub turnus: &'static str,
    pub semester_short: &'static str,
    pub no_areas_title: &'static str,
    pub no_areas_hint: &'static str,
    pub areas_intro: &'static str,

    // „Mein Plan".
    pub my_plan_hint: &'static str,
    pub to_timetable: &'static str,
    pub all_modules: &'static str,
}

pub const DE: Texts = Texts {
    program: "Studiengang",
    not_found_title: "Studiengang nicht gefunden",
    not_found_hint: "Diesen Studiengang oder diese Ansicht gibt es nicht (mehr).",
    seo_description: |name, degree, po, modules| {
        format!("{name} ({degree}, PO {po}) an der BTU Cottbus-Senftenberg: {modules} Module, Regelstudienplan, Wahlpflichtbereiche und Ordnungen.")
    },
    seo_description_track: |name, degree, po, track, modules| {
        format!("{name} ({degree}, PO {po}) an der BTU Cottbus-Senftenberg, Studienrichtung {track}: Regelstudienplan, {modules} Module, Wahlpflichtbereiche und Ordnungen.")
    },
    plan_of: |track| format!("Regelstudienplan {track}"),
    areas_title: "Wahlpflicht und Bereiche",
    plan: "Regelstudienplan",
    area: "Bereich",

    in_catalog: "Im Modulkatalog",
    views_label: "Ansichten des Studiengangs",
    views: "Ansichten",
    shape: "Darstellung",
    shape_label: "Darstellung des Regelstudienplans",
    matrix: "Matrix",
    list: "Liste",
    too_narrow: "Für die Matrix ist die Seite gerade zu schmal. Mit mehr Platz kommt sie wieder.",
    areas_label: "Bereiche des Studiengangs",
    areas: "Bereiche",
    related: "Verwandt",
    counterpart: |level| format!("Passender {level}"),
    current_po: " (aktuell)",
    actions: "Aktionen",
    to_studyplan: "In den Stundenplan",
    documents: "Ordnungen & Dokumente",
    at_btu: "Im Verzeichnis der BTU",

    crumbs: "Pfad",
    semesters_title: "Fachsemester laut Regelstudienplan",
    semesters_after: |_| " Semester",
    credits_title: "Leistungspunkte laut Regelstudienplan",
    modules_after: |_| " Module",
    fues_modules_after: |_| " FÜS-Module",
    regulations: "Prüfungsordnung ",
    current: "aktuell",
    older: "ältere Prüfungsordnung",

    no_plan: "Für diesen Studiengang liegt kein geprüfter Regelstudienplan vor",
    no_plan_hint: "Die Module findest du unter „Wahlpflicht & Bereiche“ und im Modulkatalog. Fachsemester werden nur angezeigt, wenn ein Regelstudienplan sie nennt.",
    taken_from: |date| match date {
        Some(date) => format!("Aus der Prüfungs- und Studienordnung übernommen und geprüft am {date}."),
        None => "Aus der Prüfungs- und Studienordnung übernommen und geprüft.".to_string(),
    },
    found_at: |pages, heading| match heading {
        Some(heading) => format!(" Dort auf {pages}, unter „{heading}“."),
        None => format!(" Dort auf {pages}."),
    },
    page: "Seite",
    pages: "Seiten",
    track: "Studienrichtung",
    module: "Modul",
    kind: "Art",
    credits_per_semester: "Leistungspunkte im Semester",
    credits_differ: "Die LP dieses Plans weichen vom Modulkatalog ab",
    no_semester_title: "Der Plan ordnet dieses Modul keinem Semester zu",
    total: "Summe",
    stated_note: "Die Summen sind die der Prüfungsordnung; Zeilen mit einer Spanne („10–24“) gehen in der Summe ihres Semesters auf. ",
    spread_note: "Module über mehrere Semester stehen über ihrem Zeitraum und sind in keiner Semestersumme enthalten. ",
    differs_note: "Markierte LP weichen vom Modulkatalog ab.",
    semester_as_written: |span| format!("Semester {span}"),
    no_semester: "Ohne Semesterangabe im Plan",
    no_area: "Ohne Bereich im Plan",
    modules: |n| format!("{n} Module"),
    row_title: "Was der Plan zu dieser Zeile sagt",

    picked_not_found_title: "Nicht gefunden",
    picked_not_found_hint: "Diesen Bereich oder diese Zeile des Regelstudienplans gibt es nicht (mehr).",
    to_program: "Zum Studiengang",
    preview: "Modulvorschau",
    module_not_found_title: "Modul nicht gefunden",
    module_not_found_hint: "Dieses Modul steht nicht (mehr) im Modulkatalog der BTU.",
    close_preview: "Vorschau schließen",
    close_title: "Schließen (Esc)",
    row: "Zeile des Regelstudienplans",
    fues_in_catalog: "FÜS-Module im Katalog",
    search_catalog: "Im Katalog suchen",
    fitting_in_catalog: "Passende Module im Katalog",
    electives_in_catalog: "Wahlpflichtmodule im Katalog",
    probably: "Vermutlich ",
    whole_area: "Diesen Bereich ganz ansehen",
    fitting_areas: "Passende Bereiche",
    also_possible: "Kommt auch in Frage",
    program_areas: "Bereiche des Studiengangs",
    all_areas: "alle Bereiche",
    together: "Zusammen zu belegen",
    choice_hint: |others, label, together, span, least, most| {
        let rows = if others == 1 { "einer weiteren Zeile".to_string() } else { format!("{others} weiteren Zeilen") };
        format!(
            "Diese Zeile nennt eine Spanne, keine feste Zahl. Die Prüfungsordnung weist sie mit {rows} zusammen aus — „{label}“: {together} LP in {span}. Einzeln lassen diese Zeilen {least} bis {most} LP zu; wie die {together} LP auf sie aufgeteilt werden, ist die Wahl der Studierenden."
        )
    },
    choice_one: |semester| format!("Semester {semester}"),
    choice_many: |from, to| format!("den Semestern {from} bis {to}"),
    note_one_module: "Der Plan nennt hier ein einzelnes Modul, das der Modulkatalog unter diesem Namen nicht führt — es kann anders heißen oder nicht mehr angeboten werden.",
    note_fues: "Der Plan verlangt hier ein Modul aus dem Fachübergreifenden Studium. Angerechnet wird, was in der FÜS-Liste dieses Studiengangs steht.",
    note_ambiguous: "Der Plan nennt für diese Zeile kein einzelnes Modul. Die Bereiche oben sind aus ihrem Namen abgeleitet und passen gleich gut.",
    note_fitting: "Der Plan nennt für diese Zeile kein einzelnes Modul. Der Bereich oben ist aus ihrem Namen abgeleitet, nicht aus dem Plan.",
    note_none: "Der Plan nennt für diese Zeile kein einzelnes Modul. Welche Module in Frage kommen, steht in den Bereichen dieses Studiengangs.",
    areas_within: "Bereiche darin",
    modules_heading: "Module",

    number: "Nr.",
    turnus: "Turnus",
    semester_short: "Sem.",
    no_areas_title: "Keine Bereiche bekannt",
    no_areas_hint: "Das Vorlesungsverzeichnis gliedert diesen Studiengang nicht in Bereiche.",
    areas_intro: "Wie das Vorlesungsverzeichnis diesen Studiengang gliedert. Ein Modul kann in mehreren Bereichen stehen.",

    my_plan_hint: "Hier planst du bald dein ganzes Studium, Semester für Semester.",
    to_timetable: "Zum Stundenplan",
    all_modules: "Alle Module des Studiengangs im Katalog",
};

pub const EN: Texts = Texts {
    program: "Degree programme",
    not_found_title: "Degree programme not found",
    not_found_hint: "This degree programme or this view does not exist (any more).",
    seo_description: |name, degree, po, modules| {
        format!("{name} ({degree}, PO {po}) at BTU Cottbus-Senftenberg: {modules} modules, standard study plan, elective areas and regulations.")
    },
    seo_description_track: |name, degree, po, track, modules| {
        format!("{name} ({degree}, PO {po}) at BTU Cottbus-Senftenberg, study track {track}: standard study plan, {modules} modules, elective areas and regulations.")
    },
    plan_of: |track| format!("Standard study plan {track}"),
    areas_title: "Electives and areas",
    plan: "Standard study plan",
    area: "Area",

    in_catalog: "In the module catalogue",
    views_label: "Views of the degree programme",
    views: "Views",
    shape: "Layout",
    shape_label: "Layout of the standard study plan",
    matrix: "Matrix",
    list: "List",
    too_narrow: "The page is too narrow for the matrix right now. With more room it comes back.",
    areas_label: "Areas of the degree programme",
    areas: "Areas",
    related: "Related",
    counterpart: |level| format!("Matching {level}"),
    current_po: " (current)",
    actions: "Actions",
    to_studyplan: "Add to timetable",
    documents: "Regulations & documents",
    at_btu: "In BTU's directory",

    crumbs: "Breadcrumb",
    semesters_title: "Semesters according to the standard study plan",
    semesters_after: |n| if n == "1" { " semester" } else { " semesters" },
    credits_title: "Credit points according to the standard study plan",
    modules_after: |n| if n == 1 { " module" } else { " modules" },
    fues_modules_after: |n| if n == 1 { " FÜS module" } else { " FÜS modules" },
    regulations: "Examination regulations ",
    current: "current",
    older: "older examination regulations",

    no_plan: "There is no checked standard study plan for this degree programme",
    no_plan_hint: "You will find the modules under \"Electives & areas\" and in the module catalogue. Semesters are only shown where a standard study plan names them.",
    taken_from: |date| match date {
        Some(date) => format!("Taken from the examination and study regulations and checked on {date}."),
        None => "Taken from the examination and study regulations and checked.".to_string(),
    },
    found_at: |pages, heading| match heading {
        Some(heading) => format!(" There on {pages}, under \"{heading}\"."),
        None => format!(" There on {pages}."),
    },
    page: "page",
    pages: "pages",
    track: "Study track",
    module: "Module",
    kind: "Kind",
    credits_per_semester: "Credit points per semester",
    credits_differ: "The CP of this plan differ from the module catalogue",
    no_semester_title: "The plan places this module in no semester",
    total: "Total",
    stated_note: "The totals are those of the examination regulations; rows with a range (\"10–24\") are part of the total of their semester. ",
    spread_note: "Modules over several semesters stand across their period and are part of no semester's total. ",
    differs_note: "Marked CP differ from the module catalogue.",
    semester_as_written: |span| format!("Semester {span}"),
    no_semester: "No semester stated in the plan",
    no_area: "No area in the plan",
    modules: |n| if n == 1 { "1 module".to_string() } else { format!("{n} modules") },
    row_title: "What the plan says about this row",

    picked_not_found_title: "Not found",
    picked_not_found_hint: "This area or this row of the standard study plan does not exist (any more).",
    to_program: "To the degree programme",
    preview: "Module preview",
    module_not_found_title: "Module not found",
    module_not_found_hint: "This module is not in BTU's module catalogue (any more).",
    close_preview: "Close preview",
    close_title: "Close (Esc)",
    row: "Row of the standard study plan",
    fues_in_catalog: "FÜS modules in the catalogue",
    search_catalog: "Search the catalogue",
    fitting_in_catalog: "Matching modules in the catalogue",
    electives_in_catalog: "Compulsory elective modules in the catalogue",
    probably: "Probably ",
    whole_area: "See the whole area",
    fitting_areas: "Matching areas",
    also_possible: "Also possible",
    program_areas: "Areas of the degree programme",
    all_areas: "all areas",
    together: "To be taken together",
    choice_hint: |others, label, together, span, least, most| {
        let rows = if others == 1 { "one more row".to_string() } else { format!("{others} more rows") };
        format!(
            "This row states a range, not a fixed number. The examination regulations show it together with {rows} — \"{label}\": {together} CP in {span}. On their own these rows allow {least} to {most} CP; how the {together} CP are split among them is the students' choice."
        )
    },
    choice_one: |semester| format!("semester {semester}"),
    choice_many: |from, to| format!("semesters {from} to {to}"),
    note_one_module: "The plan names a single module here that the module catalogue does not list under this name — it may be called something else or no longer be offered.",
    note_fues: "The plan asks for a module from the interdisciplinary studies here. What counts is what the FÜS list of this degree programme names.",
    note_ambiguous: "The plan names no single module for this row. The areas above are derived from its name and fit equally well.",
    note_fitting: "The plan names no single module for this row. The area above is derived from its name, not from the plan.",
    note_none: "The plan names no single module for this row. Which modules come into question is listed in the areas of this degree programme.",
    areas_within: "Areas within",
    modules_heading: "Modules",

    number: "No.",
    turnus: "Offered",
    semester_short: "Sem.",
    no_areas_title: "No areas known",
    no_areas_hint: "The course directory does not divide this degree programme into areas.",
    areas_intro: "How the course directory divides this degree programme. A module can stand in several areas.",

    my_plan_hint: "Soon you will plan your whole studies here, semester by semester.",
    to_timetable: "To the timetable",
    all_modules: "All modules of the degree programme in the catalogue",
};
