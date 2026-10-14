//! Texts of the overview of the study programs (`pages/programs.rs`).

pub struct Texts {
    /// The title of the overview in the browser's tab.
    pub title: &'static str,
    /// The overview as link previews name it, and what they say of it: „Alle 148 Studiengänge …".
    pub seo_title: &'static str,
    pub seo_description: fn(usize) -> String,
    /// The heading of the sidebar, and the button that opens it as a sheet on a phone.
    pub filters: &'static str,
    /// The headings of the sidebar's groups: degree, form of study, the study plan, faculties.
    pub level: &'static str,
    pub form: &'static str,
    pub data: &'static str,
    /// „Mit Regelstudienplan": only programs with a checked standard study plan.
    pub with_plan: &'static str,
    pub faculties: &'static str,
    /// Under the faculties: on what grounds a program is given one.
    pub faculties_hint: &'static str,
    /// The button at the end of the sheet on a phone: „148 Studiengänge anzeigen"; the number is
    /// written already.
    pub show_programs: fn(u64, &str) -> String,
    /// What the number of the head counts, with no filter …
    pub count_all: &'static str,
    /// … with a filter: „von 148 Studiengängen" …
    pub count_of: fn(usize) -> String,
    /// … and with a search: „von 148 Studiengängen passen zu „Informatik"".
    pub count_of_matching: fn(usize, &str) -> String,
    /// Nothing is left of the filters and the search.
    pub none_title: &'static str,
    pub none_hint: &'static str,
    pub show_all: &'static str,
    /// The head's line that leads to the visitor's own program: „Mein Studiengang: Informatik
    /// B.Sc. · PO 2008", its prefix, and its title.
    pub mine_prefix: &'static str,
    pub mine_title: fn(&str) -> String,
    /// The same line for a program gone from the snapshot: its prefix, what its title says, and
    /// where it leads („Zur PO 2023", the newest PO of the program).
    pub gone_prefix: &'static str,
    pub gone: fn(&str) -> String,
    pub to_po: fn(&str) -> String,
    /// „Fakultät 1": a numbered faculty in the sidebar and at the head of its section.
    pub faculty: fn(&str) -> String,
    /// The section of the programs no faculty could be derived for: its short name, its name, and
    /// why.
    pub unassigned_short: &'static str,
    pub unassigned: &'static str,
    pub unassigned_hint: &'static str,
    /// „12 Studiengänge" at the head of a faculty: what follows the number.
    pub programs_after: fn(usize) -> &'static str,
    /// The first column of the matrix, and the last (doctoral programs and programs without a
    /// degree; the other two are named as the degrees are).
    pub subject: &'static str,
    pub stage_other: &'static str,
    /// A program's tooltip says whether it has a checked standard study plan.
    pub plan_checked: &'static str,
    pub plan_unchecked: &'static str,
    /// „17 Module" in a program's tooltip.
    pub modules: fn(i64) -> String,
    /// The head of a faculty: what it has, by degree („12 Bachelor · 11 Master · 3 Lehramt · 1
    /// weitere").
    pub sum_bachelor: fn(usize) -> String,
    pub sum_master: fn(usize) -> String,
    pub sum_teaching: fn(usize) -> String,
    pub sum_other: fn(usize) -> String,
    /// What a program's validated plan says of it: „6 Semester", „6 Sem.", „180 LP"; „kein
    /// geprüfter Plan" where there is none.
    pub semesters: fn(&str) -> String,
    pub semesters_short: fn(&str) -> String,
    pub credits: fn(&str) -> String,
    pub no_plan: &'static str,
    /// A subject of several programs: „5 Angebote", „13 Fächerkombinationen".
    pub offers: fn(usize) -> String,
    pub combinations: fn(usize) -> String,
    /// The visitor's own program at the head (the app's): „Dein Studiengang" and the master or
    /// bachelor after it („Danach", „Davor"), with their ways: „Mein Studium", „Regelstudienplan".
    pub yours: &'static str,
    pub after: &'static str,
    pub before: &'static str,
    pub to_study: &'static str,
    pub to_plan: &'static str,
    pub in_study: &'static str,
    /// The mark of the visitor's own program in the list.
    pub mine_mark: &'static str,
}

pub const DE: Texts = Texts {
    title: "Studiengänge der BTU Cottbus-Senftenberg: Regelstudienpläne und Module",
    seo_title: "Studiengänge der BTU Cottbus-Senftenberg",
    seo_description: |total| {
        format!("Alle {total} Studiengänge der BTU Cottbus-Senftenberg nach Fakultät: Bachelor, Master, dual und Lehramt, jeweils mit Regelstudienplan, Wahlpflichtbereichen, Modulen und Ordnungen.")
    },
    filters: "Filter",
    level: "Abschluss",
    form: "Studienform",
    data: "Daten",
    with_plan: "Mit Regelstudienplan",
    faculties: "Fakultäten",
    faculties_hint: "Die BTU nennt zu einem Studiengang keine Fakultät. Zugeordnet ist die Fakultät des Abschlussmoduls, sonst die, die den größten Teil des Curriculums anbietet.",
    show_programs: |_, written| format!("{written} Studiengänge anzeigen"),
    count_all: "Studiengänge in ihrer aktuellen Prüfungsordnung",
    count_of: |total| format!("von {total} Studiengängen"),
    count_of_matching: |total, text| format!("von {total} Studiengängen passen zu „{text}“"),
    none_title: "Kein Studiengang gefunden",
    none_hint: "Nimm einen Filter zurück oder suche nach einem Teil des Namens.",
    show_all: "Alle Studiengänge zeigen",
    mine_prefix: "Mein Studiengang: ",
    mine_title: |name| format!("Mein Studiengang: {name}"),
    gone_prefix: "Nicht mehr im Katalog: ",
    gone: |name| format!("Dein Studiengang {name} ist nicht mehr im Katalog."),
    to_po: |po| format!("Zur PO {po}"),
    faculty: |code| format!("Fakultät {code}"),
    unassigned_short: "Ohne Zuordnung",
    unassigned: "Fakultätsübergreifend oder nicht eindeutig zuzuordnen",
    unassigned_hint: "Für diese Studiengänge lässt sich aus den Daten keine Fakultät eindeutig ableiten, zum Beispiel weil mehrere Fakultäten sie gemeinsam tragen.",
    programs_after: |n| if n == 1 { " Studiengang" } else { " Studiengänge" },
    subject: "Fach",
    stage_other: "Weitere",
    plan_checked: "geprüfter Regelstudienplan",
    plan_unchecked: "noch kein geprüfter Regelstudienplan",
    modules: |n| format!("{n} Module"),
    sum_bachelor: |n| format!("{n} Bachelor"),
    sum_master: |n| format!("{n} Master"),
    sum_teaching: |n| format!("{n} Lehramt"),
    sum_other: |n| format!("{n} weitere"),
    semesters: |n| format!("{n}\u{a0}Semester"),
    semesters_short: |n| format!("{n}\u{a0}Sem."),
    credits: |n| format!("{n}\u{a0}LP"),
    no_plan: "kein geprüfter Plan",
    offers: |n| format!("{n} Angebote"),
    combinations: |n| format!("{n} Fächerkombinationen"),
    yours: "Dein Studiengang",
    after: "Danach",
    before: "Davor",
    to_study: "Mein Studium",
    to_plan: "Regelstudienplan",
    in_study: "in Mein Studium",
    mine_mark: "Dein Studiengang",
};

pub const EN: Texts = Texts {
    title: "Degree programmes of BTU Cottbus-Senftenberg: standard study plans and modules",
    seo_title: "Degree programmes of BTU Cottbus-Senftenberg",
    seo_description: |total| {
        format!("All {total} degree programmes of BTU Cottbus-Senftenberg by faculty: Bachelor, Master, dual and teacher training, each with its standard study plan, elective areas, modules and regulations.")
    },
    filters: "Filters",
    level: "Degree",
    form: "Form of study",
    data: "Data",
    with_plan: "With standard study plan",
    faculties: "Faculties",
    faculties_hint: "BTU does not name a faculty for a degree programme. The one given is the faculty of the thesis module, otherwise the one that offers most of the curriculum.",
    show_programs: |n, written| if n == 1 { "Show 1 degree programme".to_string() } else { format!("Show {written} degree programmes") },
    count_all: "degree programmes in their current examination regulations",
    count_of: |total| format!("of {total} degree programmes"),
    count_of_matching: |total, text| format!("of {total} degree programmes match \"{text}\""),
    none_title: "No degree programme found",
    none_hint: "Remove a filter or search for part of the name.",
    show_all: "Show all degree programmes",
    mine_prefix: "My programme: ",
    mine_title: |name| format!("My programme: {name}"),
    gone_prefix: "No longer in the catalogue: ",
    gone: |name| format!("Your degree programme {name} is no longer in the catalogue."),
    to_po: |po| format!("To PO {po}"),
    faculty: |code| format!("Faculty {code}"),
    unassigned_short: "Unassigned",
    unassigned: "Across faculties or not clearly assignable",
    unassigned_hint: "No single faculty can be derived from the data for these degree programmes, for example because several faculties run them together.",
    programs_after: |n| if n == 1 { " degree programme" } else { " degree programmes" },
    subject: "Subject",
    stage_other: "Other",
    plan_checked: "checked standard study plan",
    plan_unchecked: "no checked standard study plan yet",
    modules: |n| if n == 1 { "1 module".to_string() } else { format!("{n} modules") },
    sum_bachelor: |n| format!("{n} Bachelor"),
    sum_master: |n| format!("{n} Master"),
    sum_teaching: |n| format!("{n} teacher training"),
    sum_other: |n| format!("{n} other"),
    semesters: |n| if n == "1" { "1\u{a0}semester".to_string() } else { format!("{n}\u{a0}semesters") },
    semesters_short: |n| format!("{n}\u{a0}sem."),
    credits: |n| format!("{n}\u{a0}CP"),
    no_plan: "no checked plan",
    offers: |n| format!("{n} options"),
    combinations: |n| format!("{n} subject combinations"),
    yours: "Your programme",
    after: "After it",
    before: "Before it",
    to_study: "My studies",
    to_plan: "Standard study plan",
    in_study: "in My studies",
    mine_mark: "Your programme",
};
