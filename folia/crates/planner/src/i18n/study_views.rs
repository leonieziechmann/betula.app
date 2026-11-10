//! Texts of the views of „Mein Studium" (`study/regular.rs`, `study/choose.rs`,
//! `study/documents.rs`): the Regelstudienplan and the Wahlpflicht with the student's stand in
//! them, the program's regulations, and the ways to them.

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
    /// The way to the regulations on a phone: „PO 2008 - 2. SÄ 2024 · 4 Dokumente".
    pub documents_way: fn(&str, usize) -> String,

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
    /// „Noch 72 LP zu wählen".
    pub choices_lead: fn(&str) -> String,
    pub choices_done_lead: &'static str,
    pub choices_none: &'static str,
    pub choices_none_hint: &'static str,
    /// What is held of what is asked: „6 / 10–24 LP".
    pub figure: fn(&str, &str) -> String,
    /// What a choice asks: „6 LP aus „Praktische Mathematik“.", „12 LP aus einer der 5 Listen.",
    /// „4 LP.", the FÜS „6 LP aus dem FÜS-Katalog der BTU …"; the sum it shares („Zusammen mit
    /// Praktische Informatik und Angewandte und Technische Informatik 44 LP.").
    pub rule_list: fn(&str, &str) -> String,
    pub rule_one_of: fn(&str, usize) -> String,
    pub rule_plain: fn(&str) -> String,
    pub fues_rule: fn(&str) -> String,
    pub together: fn(&str, &str) -> String,
    pub and: &'static str,
    /// What a group asks (the Komplexe): „50 LP aus diesen drei, aus jedem 10 bis 24." (the sum,
    /// how many, the least and the most of each), and where its extras give one of them more
    /// („Einer darf bis 30: Die 6 LP von „Wahlpflicht“ im 4. FS, ein Modul aus einem der drei,
    /// zählen nur bei einem."). A number as a word: „drei".
    pub group_rule: fn(&str, &str, &str, &str) -> String,
    pub group_extra: fn(&str, &str, &str, &str, &str) -> String,
    pub count_word: fn(usize) -> String,
    /// The hatched stretch of a group's members: open to each („24 bis 30 LP: nur für einen von
    /// ihnen …"), kept by one, and two over the most (the others, whether several, the one
    /// keeping it, the most, the room's end).
    pub room_open: fn(&str, &str) -> String,
    pub room_kept: fn(&str, &str, &str) -> String,
    pub room_over: fn(&str, bool, &str, &str, &str) -> String,
    /// The way to the FÜS's catalog.
    pub fues_catalog: &'static str,
    /// A choice whose list the tree does not have.
    pub no_list: &'static str,
    pub search_catalog: &'static str,
    /// „17 Module", „5 Listen · 59 Module", „im FÜS-Katalog", „1 gemerkt".
    pub modules: fn(usize) -> String,
    pub lists_count: fn(usize, usize) -> String,
    pub fues_count: &'static str,
    pub marked_count: fn(usize) -> String,
    /// A module of the list that counts for another choice: „zählt für Anwendungsfach".
    pub counts_for: fn(&str) -> String,
    /// A placeholder of the choice no module fills: „Platzhalter", „im WiSe 2026/27, noch ohne
    /// Modul".
    pub place: &'static str,
    pub place_line: fn(&str) -> String,
    /// What marks a module on the Merkliste.
    pub marked: &'static str,
    /// The lists of a choice, for screen readers.
    pub lists: &'static str,
    /// The areas, for screen readers.
    pub areas: &'static str,

    // „Ordnungen & Dokumente".
    /// The regulations, for screen readers.
    pub documents_list: &'static str,
    /// Under the regulation the Regelstudienplan was read from: „Darin der Regelstudienplan",
    /// „Seite 7", „Seiten 6, 7".
    pub plan_in: &'static str,
    pub plan_pages: fn(&str) -> String,
    /// A program without a regulation Betula knows.
    pub no_documents: &'static str,
    pub no_documents_hint: &'static str,
    /// „Mehr zum Studiengang", and its ways: the module tree at the BTU, the program's modules in
    /// the catalog („109 Module im Katalog"), its page for everybody.
    pub more: &'static str,
    pub module_tree: &'static str,
    pub module_tree_line: &'static str,
    pub program_modules: &'static str,
    pub program_modules_line: fn(i64) -> String,
    pub program_page_line: &'static str,
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
    documents_way: |po, n| match n {
        0 => format!("PO {po}"),
        1 => format!("PO {po} · 1 Dokument"),
        n => format!("PO {po} · {n} Dokumente"),
    },

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

    choices_lead: |credits| format!("Noch {credits}\u{a0}LP zu wählen"),
    choices_done_lead: "Alles gewählt. Tauschen kannst du trotzdem.",
    choices_none: "Nichts zu wählen",
    choices_none_hint: "Dieser Regelstudienplan nennt für jede Zeile ein Modul.",
    figure: |taken, range| format!("{taken} / {range}\u{a0}LP"),
    rule_list: |credits, list| format!("{credits}\u{a0}LP aus „{list}“."),
    rule_one_of: |credits, n| format!("{credits}\u{a0}LP aus einer der {n} Listen."),
    rule_plain: |credits| format!("{credits}\u{a0}LP."),
    fues_rule: |credits| format!("{credits}\u{a0}LP aus dem FÜS-Katalog der BTU: Module außerhalb deines Fachs."),
    together: |names, credits| format!("Zusammen mit {names} {credits}\u{a0}LP."),
    and: "und",
    group_rule: |target, n, least, most| format!("{target}\u{a0}LP aus diesen {n}, aus jedem {least} bis {most}."),
    group_extra: |top, credits, names, span, n| format!("Einer darf bis {top}: Die {credits}\u{a0}LP von „{names}“ im {span}, ein Modul aus einem der {n}, zählen nur bei einem."),
    count_word: |n| match n {
        2 => "beiden".to_string(),
        3 => "drei".to_string(),
        4 => "vier".to_string(),
        5 => "fünf".to_string(),
        6 => "sechs".to_string(),
        n => n.to_string(),
    },
    room_open: |most, top| format!("{most} bis {top}\u{a0}LP: nur für einen von ihnen. Geht einer über {most}, gehört die Stelle ihm."),
    room_kept: |name, most, top| format!("{most} bis {top}\u{a0}LP hat {name}; bei den anderen endet der Balken bei {most}."),
    room_over: |names, several, kept, most, top| format!("Auch {names} {} mehr als {most}\u{a0}LP. Nur einer darf bis {top}, hier {kept}; bei {names} zählt nur bis {most}.", if several { "haben" } else { "hat" }),
    fues_catalog: "FÜS-Module im Katalog",
    no_list: "Welche Module hier zählen, sagt der Modulbaum des Studiengangs nicht.",
    search_catalog: "Im Katalog suchen",
    modules: |n| if n == 1 { "1 Modul".to_string() } else { format!("{n} Module") },
    lists_count: |lists, modules| format!("{lists} Listen · {modules} Module"),
    fues_count: "im FÜS-Katalog",
    marked_count: |n| format!("{n} gemerkt"),
    counts_for: |name| format!("zählt für {name}"),
    place: "Platzhalter",
    place_line: |semester| format!("im {semester}, noch ohne Modul"),
    marked: "Gemerkt",
    lists: "Listen",
    areas: "Bereiche",

    documents_list: "Ordnungen",
    plan_in: "Darin der Regelstudienplan",
    plan_pages: |pages| if pages.contains([',', '–', '-']) { format!("Seiten {pages}") } else { format!("Seite {pages}") },
    no_documents: "Keine Ordnung bekannt",
    no_documents_hint: "Für diesen Studiengang kennt Betula keine Ordnung. Der Modulbaum der BTU nennt sie, sobald sie dort verlinkt ist.",
    more: "Mehr zum Studiengang",
    module_tree: "Modulbaum des Studiengangs",
    module_tree_line: "Im Vorlesungsverzeichnis der BTU",
    program_modules: "Module des Studiengangs",
    program_modules_line: |n| if n == 1 { "1 Modul im Katalog".to_string() } else { format!("{n} Module im Katalog") },
    program_page_line: "Plan und Bereiche, wie alle sie sehen",
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
    documents_way: |po, n| match n {
        0 => format!("PO {po}"),
        1 => format!("PO {po} · 1 document"),
        n => format!("PO {po} · {n} documents"),
    },

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

    choices_lead: |credits| format!("{credits}\u{a0}CP left to choose"),
    choices_done_lead: "All chosen. You can still swap.",
    choices_none: "Nothing to choose",
    choices_none_hint: "This standard study plan names a module for every row.",
    figure: |taken, range| format!("{taken} / {range}\u{a0}CP"),
    rule_list: |credits, list| format!("{credits}\u{a0}CP from \u{201c}{list}\u{201d}."),
    rule_one_of: |credits, n| format!("{credits}\u{a0}CP from one of the {n} lists."),
    rule_plain: |credits| format!("{credits}\u{a0}CP."),
    fues_rule: |credits| format!("{credits}\u{a0}CP from BTU's FÜS catalogue: modules outside your subject."),
    together: |names, credits| format!("Together with {names}: {credits}\u{a0}CP."),
    and: "and",
    group_rule: |target, n, least, most| format!("{target}\u{a0}CP from these {n}, {least} to {most} from each."),
    group_extra: |top, credits, names, span, n| format!("One of them may go to {top}: the {credits}\u{a0}CP of \u{201c}{names}\u{201d} in {span}, a module of one of the {n}, count for one alone."),
    count_word: |n| match n {
        2 => "two".to_string(),
        3 => "three".to_string(),
        4 => "four".to_string(),
        5 => "five".to_string(),
        6 => "six".to_string(),
        n => n.to_string(),
    },
    room_open: |most, top| format!("{most} to {top}\u{a0}CP: for one of them alone. The one that goes over {most} has it."),
    room_kept: |name, most, top| format!("{name} has {most} to {top}\u{a0}CP; the others' bars end at {most}."),
    room_over: |names, several, kept, most, top| format!("{names} {} more than {most}\u{a0}CP as well. Only one may go to {top}, here {kept}; of {names} only {most} count.", if several { "have" } else { "has" }),
    fues_catalog: "FÜS modules in the catalogue",
    no_list: "The programme's module tree does not say which modules count here.",
    search_catalog: "Search the catalogue",
    modules: |n| if n == 1 { "1 module".to_string() } else { format!("{n} modules") },
    lists_count: |lists, modules| format!("{lists} lists · {modules} modules"),
    fues_count: "in the FÜS catalogue",
    marked_count: |n| format!("{n} saved"),
    counts_for: |name| format!("counts for {name}"),
    place: "Placeholder",
    place_line: |semester| format!("in {semester}, no module yet"),
    marked: "Saved",
    lists: "Lists",
    areas: "Areas",

    documents_list: "Regulations",
    plan_in: "The standard study plan in it",
    plan_pages: |pages| if pages.contains([',', '–', '-']) { format!("pages {pages}") } else { format!("page {pages}") },
    no_documents: "No regulations known",
    no_documents_hint: "Betula knows no regulations of this programme. BTU's module tree names them once they are linked there.",
    more: "More on the programme",
    module_tree: "Module tree of the programme",
    module_tree_line: "In BTU's course catalogue",
    program_modules: "Modules of the programme",
    program_modules_line: |n| if n == 1 { "1 module in the catalogue".to_string() } else { format!("{n} modules in the catalogue") },
    program_page_line: "Plan and areas as everybody sees them",
};
