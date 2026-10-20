//! Texts of the views of „Mein Studium" (`study/regular.rs`, `study/choose.rs`): the
//! Regelstudienplan and the Wahlpflicht with the student's stand in them, and the ways to them.

pub struct Texts {
    // The ways to the views.
    /// „Ansichten": the sidebar's group of the views.
    pub views: &'static str,
    /// The views by name: „Regelstudienplan", „Wahlpflicht".
    pub plan: &'static str,
    pub choices: &'static str,
    /// What the sidebar says beside a view: „jetzt 3. FS", „Soll · Ist", „72 LP".
    pub now_fs: fn(&str) -> String,
    pub plan_side: &'static str,
    /// „Studiengang": the sidebar's group of the program's own pages.
    pub program_group: &'static str,
    pub documents: &'static str,
    pub program_page: &'static str,
    /// „Alle Studiengänge" and what is there.
    pub all_programs: &'static str,
    pub all_programs_line: &'static str,
    /// The way to the Regelstudienplan on a phone: „Soll und Ist · 1.–2. FS: 52 von 60 LP
    /// bestanden", before any is over „Soll und Ist, Zeile für Zeile".
    pub plan_way: fn(&str, &str, &str) -> String,
    pub plan_way_plain: &'static str,
    /// The way to the Wahlpflicht: „Noch 72 LP zu wählen · Nebenfach, Fachstudium", „Alles
    /// gewählt", „Nichts zu wählen".
    pub choices_way: fn(&str, &str) -> String,
    pub all_chosen: &'static str,
    pub nothing_to_choose: &'static str,
    /// The way back on a phone, for screen readers: „Zur Übersicht".
    pub to_overview: &'static str,

    // The Regelstudienplan.
    /// „Informatik B.Sc. · PO 2008 · 180 LP" under the title on a phone; „Soll und Ist" on a
    /// desktop, with the study direction where there are several.
    pub plan_lead: &'static str,
    pub no_plan: &'static str,
    pub no_plan_hint: &'static str,
    /// „Mein Stand": the switch that shows the student's stand in the table.
    pub mine: &'static str,
    /// The columns: „Modul", „Art", the Fachsemester, „Bei dir".
    pub col_module: &'static str,
    pub col_kind: &'static str,
    pub col_fs: &'static str,
    pub col_mine: &'static str,
    pub col_credits: &'static str,
    /// The foot: „Laut Plan", „Bei dir".
    pub per_plan: &'static str,
    pub per_mine: &'static str,
    /// Under „Bei dir", a line each: „52 LP bestanden", „44 LP geplant".
    pub foot_passed: fn(&str) -> String,
    pub foot_planned: fn(&str) -> String,
    /// Where a row stands: „bestanden · WiSe 25/26", „geplant · SoSe 27", „wieder im WiSe
    /// 26/27", „nicht bestanden · SoSe 26", „offen".
    pub state_passed: fn(&str) -> String,
    pub state_planned: fn(&str) -> String,
    pub state_again: fn(&str) -> String,
    pub state_failed: fn(&str) -> String,
    pub state_open: &'static str,
    /// What fills a row's placeholder: „mit Proseminar".
    pub filled_by: fn(&str) -> String,
    /// What is taken of a row's choice: „8 von 12 LP gewählt".
    pub choice_taken: fn(&str, &str) -> String,
    /// „Einplanen", „Wählen": a row's action; the title of „Einplanen": „Ins SoSe 2027".
    pub plan_it: &'static str,
    pub choose: &'static str,
    pub plan_into: fn(&str) -> String,
    /// „„Theoretische Informatik“ ins WiSe 2026/27 eingeplant".
    pub planned_note: fn(&str, &str) -> String,
    /// The Fachsemester on a phone: „jetzt", the line under the head („Bis zum 2. FS 52 von 60
    /// LP bestanden"), a retake („Mathematik IT-1 wieder im WiSe 26/27"), a section's stand.
    pub so_far: fn(&str, &str, &str) -> String,
    pub retake_line: fn(&str, &str) -> String,
    pub retake_open: fn(&str) -> String,
    pub section_passed: fn(&str, &str) -> String,
    pub section_planned: fn(&str, &str) -> String,
    pub section_plain: fn(&str) -> String,
    /// The program's own page of the plan, where it comes from.
    pub plan_source: &'static str,
    /// „Fachsemester": the strip of a phone's sections, for screen readers.
    pub fs_strip: &'static str,

    // The Wahlpflicht.
    /// „Noch 72 LP zu wählen – nach deinen Bereichen, mit dem, was das Semester anbietet".
    pub choices_lead: fn(&str) -> String,
    pub choices_done_lead: &'static str,
    pub choices_none: &'static str,
    pub choices_none_hint: &'static str,
    /// „Angeboten im" before the semesters; „Alle".
    pub offered_in: &'static str,
    pub all: &'static str,
    /// What the plan says of a choice: „12 LP laut Plan im 3. und 4. FS.", a choice among
    /// several lists („Aus einer der 5 Listen."), the sum it shares („Zusammen mit Praktische
    /// Informatik und Angewandte und Technische Informatik 44 LP.").
    pub rule: fn(&str, &str) -> String,
    pub one_of: fn(usize) -> String,
    pub together: fn(&str, &str) -> String,
    pub and: &'static str,
    /// „Aus dem FÜS-Katalog der BTU." and the way there.
    pub fues_rule: &'static str,
    pub fues_catalog: &'static str,
    /// A choice whose list the tree does not have.
    pub no_list: &'static str,
    pub search_catalog: &'static str,
    /// „8 von 12 LP gewählt", „Erledigt im SoSe 2026".
    pub taken: fn(&str, &str) -> String,
    pub done_in: fn(&str) -> String,
    /// What is taken of it, as a heading.
    pub yours: &'static str,
    /// The table: „Nr.", „Modul", „LP", „Turnus".
    pub col_id: &'static str,
    pub col_turnus: &'static str,
    /// „Jedes Semester", „Unregelmäßig": a turnus without one half of the year.
    pub every_semester: &'static str,
    pub turnus_unknown: &'static str,
    /// A module's stand in the list: „geplant · SoSe 27", „bestanden".
    pub chip_planned: fn(&str) -> String,
    pub chip_passed: &'static str,
    /// „In SoSe 27": the button that plans it.
    pub into: fn(&str) -> String,
    /// „16 weitere, nicht im SoSe angeboten", „alle zeigen".
    pub more_unoffered: fn(usize, &str) -> String,
    pub show_all: &'static str,
    /// How many marked modules a choice's lists hold: „3 gemerkte zählen hier"; and what marks one.
    pub marked_here: fn(usize) -> String,
    pub marked: &'static str,
    /// „17 Module".
    pub modules: fn(usize) -> String,
    /// „laut Plan im 4. FS".
    pub plan_says: fn(&str) -> String,
    /// The lists of a choice, for screen readers.
    pub lists: &'static str,
    /// The areas, for screen readers.
    pub areas: &'static str,
}

pub const DE: Texts = Texts {
    views: "Ansichten",
    plan: "Regelstudienplan",
    choices: "Wahlpflicht",
    now_fs: |fs| format!("jetzt {fs}"),
    plan_side: "Soll · Ist",
    program_group: "Studiengang",
    documents: "Ordnungen & Dokumente",
    program_page: "Seite des Studiengangs",
    all_programs: "Alle Studiengänge",
    all_programs_line: "Nach Fakultät, deiner obenauf",
    plan_way: |span, passed, all| format!("Soll und Ist · {span}: {passed} von {all}\u{a0}LP bestanden"),
    plan_way_plain: "Soll und Ist, Zeile für Zeile",
    choices_way: |credits, areas| if areas.is_empty() { format!("Noch {credits}\u{a0}LP zu wählen") } else { format!("Noch {credits}\u{a0}LP zu wählen · {areas}") },
    all_chosen: "Alles gewählt",
    nothing_to_choose: "Nichts zu wählen",
    to_overview: "Zur Übersicht",

    plan_lead: "Soll und Ist",
    no_plan: "Kein geprüfter Regelstudienplan",
    no_plan_hint: "Für diesen Studiengang hat Betula keinen geprüften Regelstudienplan. Deine Semester planst du trotzdem – aus dem Katalog.",
    mine: "Mein Stand",
    col_module: "Modul",
    col_kind: "Art",
    col_fs: "Fachsemester",
    col_mine: "Bei dir",
    col_credits: "LP",
    per_plan: "Laut Plan",
    per_mine: "Bei dir",
    foot_passed: |credits| format!("{credits} bestanden"),
    foot_planned: |credits| format!("{credits} geplant"),
    state_passed: |semester| format!("bestanden · {semester}"),
    state_planned: |semester| format!("geplant · {semester}"),
    state_again: |semester| format!("wieder im {semester}"),
    state_failed: |semester| format!("nicht bestanden · {semester}"),
    state_open: "offen",
    filled_by: |names| format!("mit {names}"),
    choice_taken: |taken, all| format!("{taken} von {all}\u{a0}LP gewählt"),
    plan_it: "Einplanen",
    choose: "Wählen",
    plan_into: |semester| format!("Ins {semester} einplanen"),
    planned_note: |name, semester| format!("„{name}“ ins {semester} eingeplant"),
    so_far: |span, passed, all| format!("{span}: {passed} von {all}\u{a0}LP bestanden"),
    retake_line: |name, semester| format!("{name} wieder im {semester}"),
    retake_open: |name| format!("{name} noch nicht wieder eingeplant"),
    section_passed: |passed, all| format!("{passed} von {all} bestanden"),
    section_planned: |planned, all| format!("{planned} von {all} geplant"),
    section_plain: |all| format!("{all}\u{a0}LP"),
    plan_source: "Der Plan beim Studiengang",
    fs_strip: "Fachsemester",

    choices_lead: |credits| format!("Noch {credits}\u{a0}LP zu wählen – nach deinen Bereichen, mit dem, was das Semester anbietet"),
    choices_done_lead: "Alles gewählt. Tauschen kannst du trotzdem.",
    choices_none: "Nichts zu wählen",
    choices_none_hint: "Dieser Regelstudienplan nennt für jede Zeile ein Modul.",
    offered_in: "Angeboten im",
    all: "Alle",
    rule: |credits, span| format!("{credits}\u{a0}LP laut Plan im {span}."),
    one_of: |n| format!("Aus einer der {n} Listen."),
    together: |names, credits| format!("Zusammen mit {names} {credits}\u{a0}LP."),
    and: "und",
    fues_rule: "Aus dem FÜS-Katalog der BTU: Module außerhalb deines Fachs.",
    fues_catalog: "FÜS-Module im Katalog",
    no_list: "Welche Module hier zählen, sagt der Modulbaum des Studiengangs nicht.",
    search_catalog: "Im Katalog suchen",
    taken: |taken, all| format!("{taken} von {all}\u{a0}LP gewählt"),
    done_in: |semester| format!("Erledigt im {semester}"),
    yours: "Deine Wahl",
    col_id: "Nr.",
    col_turnus: "Turnus",
    every_semester: "jedes Sem.",
    turnus_unknown: "–",
    chip_planned: |semester| format!("geplant · {semester}"),
    chip_passed: "bestanden",
    into: |semester| format!("In {semester}"),
    more_unoffered: |n, season| if n == 1 { format!("1 weiteres, nicht im {season} angeboten") } else { format!("{n} weitere, nicht im {season} angeboten") },
    show_all: "alle zeigen",
    marked_here: |n| if n == 1 { "1 gemerktes zählt hier".to_string() } else { format!("{n} gemerkte zählen hier") },
    marked: "Gemerkt",
    modules: |n| if n == 1 { "1 Modul".to_string() } else { format!("{n} Module") },
    plan_says: |span| format!("laut Plan im {span}"),
    lists: "Listen",
    areas: "Bereiche",
};

pub const EN: Texts = Texts {
    views: "Views",
    plan: "Standard study plan",
    choices: "Electives",
    now_fs: |fs| format!("now {fs}"),
    plan_side: "Plan · yours",
    program_group: "Degree programme",
    documents: "Regulations & documents",
    program_page: "Programme page",
    all_programs: "All degree programmes",
    all_programs_line: "By faculty, yours on top",
    plan_way: |span, passed, all| format!("Plan and progress · {span}: {passed} of {all}\u{a0}CP passed"),
    plan_way_plain: "Plan and progress, row by row",
    choices_way: |credits, areas| if areas.is_empty() { format!("{credits}\u{a0}CP left to choose") } else { format!("{credits}\u{a0}CP left to choose · {areas}") },
    all_chosen: "All chosen",
    nothing_to_choose: "Nothing to choose",
    to_overview: "To the overview",

    plan_lead: "Plan and progress",
    no_plan: "No checked standard study plan",
    no_plan_hint: "Betula has no checked standard study plan for this programme. You can still plan your semesters – from the catalogue.",
    mine: "My progress",
    col_module: "Module",
    col_kind: "Kind",
    col_fs: "Semester",
    col_mine: "Yours",
    col_credits: "CP",
    per_plan: "Per plan",
    per_mine: "Yours",
    foot_passed: |credits| format!("{credits} passed"),
    foot_planned: |credits| format!("{credits} planned"),
    state_passed: |semester| format!("passed · {semester}"),
    state_planned: |semester| format!("planned · {semester}"),
    state_again: |semester| format!("again in {semester}"),
    state_failed: |semester| format!("not passed · {semester}"),
    state_open: "open",
    filled_by: |names| format!("with {names}"),
    choice_taken: |taken, all| format!("{taken} of {all}\u{a0}CP chosen"),
    plan_it: "Plan it",
    choose: "Choose",
    plan_into: |semester| format!("Plan into {semester}"),
    planned_note: |name, semester| format!("\u{201c}{name}\u{201d} planned into {semester}"),
    so_far: |span, passed, all| format!("{span}: {passed} of {all}\u{a0}CP passed"),
    retake_line: |name, semester| format!("{name} again in {semester}"),
    retake_open: |name| format!("{name} not planned again yet"),
    section_passed: |passed, all| format!("{passed} of {all} passed"),
    section_planned: |planned, all| format!("{planned} of {all} planned"),
    section_plain: |all| format!("{all}\u{a0}CP"),
    plan_source: "The plan on the programme's page",
    fs_strip: "Semesters",

    choices_lead: |credits| format!("{credits}\u{a0}CP left to choose – by your areas, with what the semester offers"),
    choices_done_lead: "All chosen. You can still swap.",
    choices_none: "Nothing to choose",
    choices_none_hint: "This standard study plan names a module for every row.",
    offered_in: "Offered in",
    all: "All",
    rule: |credits, span| format!("{credits}\u{a0}CP per plan in {span}."),
    one_of: |n| format!("From one of the {n} lists."),
    together: |names, credits| format!("Together with {names}: {credits}\u{a0}CP."),
    and: "and",
    fues_rule: "From BTU's FÜS catalogue: modules outside your subject.",
    fues_catalog: "FÜS modules in the catalogue",
    no_list: "The programme's module tree does not say which modules count here.",
    search_catalog: "Search the catalogue",
    taken: |taken, all| format!("{taken} of {all}\u{a0}CP chosen"),
    done_in: |semester| format!("Done in {semester}"),
    yours: "Your choice",
    col_id: "No.",
    col_turnus: "Offered",
    every_semester: "every sem.",
    turnus_unknown: "–",
    chip_planned: |semester| format!("planned · {semester}"),
    chip_passed: "passed",
    into: |semester| format!("In {semester}"),
    more_unoffered: |n, season| if n == 1 { format!("1 more, not offered in {season}") } else { format!("{n} more, not offered in {season}") },
    show_all: "show all",
    marked_here: |n| format!("{n} saved count{} here", if n == 1 { "s" } else { "" }),
    marked: "Saved",
    modules: |n| if n == 1 { "1 module".to_string() } else { format!("{n} modules") },
    plan_says: |span| format!("per plan in {span}"),
    lists: "Lists",
    areas: "Areas",
};
