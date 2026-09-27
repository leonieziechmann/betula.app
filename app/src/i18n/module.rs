//! Texts of a module's page and its preview (`pages/module.rs`), and of a module shown in place
//! of a page (`local.rs`).

pub struct Texts {
    // ---- what search engines and link previews read ----
    /// The page's title, the name first: „Analysis I (11101) · Modul der BTU Cottbus-Senftenberg";
    /// the values are the module's title and number.
    pub title: fn(&str, &str) -> String,
    /// The page's description, from the module's contents or learning outcomes: „Analysis I
    /// (11101, 8 LP) an der BTU Cottbus-Senftenberg: <text>"; the values are the title, the
    /// number, the credits (written already) and the text.
    pub description: fn(&str, &str, &str, &str) -> String,
    /// The same for a module without either text, saying what the page tells instead.
    pub description_bare: fn(&str, &str, &str) -> String,
    /// The catalog as the breadcrumbs of the structured data name it.
    pub breadcrumb_catalog: &'static str,
    /// „Regelstudienplan Informatik (B.Sc.), PO 2008": the study plan that places the module in a
    /// semester, as structured data names it; the values are the program, its degree and its PO.
    pub plan_framework: fn(&str, &str, &str) -> String,
    /// „Prüfung Analysis I": an exam date as an event of the structured data; the value is the
    /// title of the event.
    pub exam_event: fn(&str) -> String,

    // ---- the page and the preview ----
    /// The heading of the page's sidebar.
    pub sidebar_title: &'static str,
    /// A module the snapshot does not know, at its own address and in place of a page.
    pub not_found: &'static str,
    pub not_found_hint: &'static str,
    /// The preview's name, for screen readers.
    pub preview: &'static str,
    /// The arrow back in the preview's head, for screen readers, and the tooltips of „Vollbild"
    /// and „Schließen" with their shortcuts.
    pub close_preview: &'static str,
    pub full_view_title: &'static str,
    pub close_preview_title: &'static str,

    // ---- the sidebar of the page ----
    /// „Auf dieser Seite": the jumps to the sections, their heading and name.
    pub on_this_page: &'static str,
    pub actions: &'static str,
    pub copy_link: &'static str,
    /// The link to the module's page at the BTU.
    pub original_at_btu: &'static str,

    // ---- the sections, as their headings and the sidebar's jumps name them ----
    /// „Termine": the week and the list of the module's teaching dates.
    pub dates: &'static str,
    pub exam_dates: &'static str,
    pub at_a_glance: &'static str,
    pub programs: &'static str,
    pub prerequisites: &'static str,
    pub contents: &'static str,
    pub learning_outcomes: &'static str,
    /// „Prüfungsleistung": what the exam asks for, in the words of the module description.
    pub assessment: &'static str,
    pub literature: &'static str,
    pub remarks: &'static str,
    /// „Literatur (12)": the folded list of titles, with how many there are.
    pub literature_count: fn(usize) -> String,

    // ---- the heading ----
    /// Before the modules that follow this one: while it is offered, and once it is replaced.
    pub successor: &'static str,
    pub replaced_by: &'static str,

    // ---- „Auf einen Blick" ----
    /// The labels of the key facts.
    pub exam: &'static str,
    pub grading: &'static str,
    pub duration: &'static str,
    pub places: &'static str,
    pub language: &'static str,
    pub location: &'static str,
    pub responsible: &'static str,
    pub teaching_forms: &'static str,
    /// The values of the facts the app words itself: graded or not, and the places …
    pub graded: &'static str,
    pub ungraded: &'static str,
    /// … „max. 30", limited without a number, not limited.
    pub places_max: fn(i64) -> String,
    pub limited: &'static str,
    pub unlimited: &'static str,

    // ---- „Voraussetzungen" ----
    /// What a linked module is to this one.
    pub mandatory: &'static str,
    pub recommended: &'static str,
    /// The folded texts of the module description.
    pub mandatory_verbatim: &'static str,
    pub recommended_verbatim: &'static str,

    // ---- the source, under the module ----
    pub source: &'static str,
    /// „ · abgerufen 19.09.2026"; the value is the date, written already.
    pub fetched: fn(&str) -> String,
    pub original: &'static str,

    // ---- „Termine" and „Prüfungstermine" ----
    /// „Termine aus dem WiSe 2026/27. Für das SoSe 2027 hat die BTU noch keine Termine zu diesem
    /// Modul veröffentlicht.": the Termine shown are of a semester before the next one; the values
    /// are the two semesters.
    pub dates_from: fn(&str, &str) -> String,
    /// No Termine at all: what the turnus says instead (winter, summer, every semester), and
    /// where it says nothing.
    pub no_dates_winter: &'static str,
    pub no_dates_summer: &'static str,
    pub no_dates_every: &'static str,
    pub no_dates: &'static str,
    /// A row without a time of the week: „Termin offen" for an exam whose date is QIS's
    /// placeholder, else „Zeit offen".
    pub date_open: &'static str,
    pub time_open: &'static str,
    /// „Mo bis 24:00", „bis 24:00": a deadline, an end without a start; the values are the
    /// weekday and the end.
    pub day_until: fn(&str, &str) -> String,
    pub until: fn(&str) -> String,
    /// „In QIS: So 01:00–02:30 · 27.12.2015": what the BTU wrote where the row shows something
    /// else; the value is what it wrote.
    pub in_qis: fn(&str) -> String,
    /// The same, with what is odd about it („In QIS: … · Uhrzeit ungewöhnlich").
    pub in_qis_odd: fn(&str, &str) -> String,
    /// „Uhrzeit ungewöhnlich, so steht es in QIS": what the row shows as stated is odd.
    pub as_in_qis: fn(&str) -> String,
    /// What is odd about an exam date, in the line under it …
    pub unusual_time: &'static str,
    pub ends_before_start: &'static str,
    pub date_outside_semester: &'static str,
    /// … and in the note under the exam dates, which explains QIS's placeholder at 01:00–02:30:
    /// with a date outside the semester (the value), and without.
    pub placeholder_date_note: fn(&str) -> String,
    pub placeholder_time_note: &'static str,
    /// The odd things in a sentence: „eine Uhrzeit außerhalb von 06:00–22:00" (the value is the
    /// hours), „ein Ende vor dem Beginn", „ein Datum weit außerhalb des WiSe 2026/27" (the
    /// semester) …
    pub odd_time: fn(&str) -> String,
    pub odd_end: &'static str,
    pub odd_date: fn(&str) -> String,
    /// … joined by „, " and, before the last, this …
    pub odd_or: &'static str,
    /// … and the sentence they make: „<Eine Uhrzeit …> steht so in QIS, ist für eine Prüfung aber
    /// ungewöhnlich, vermutlich ein Eingabefehler." The value's first letter is made a capital.
    pub odd_note: fn(&str) -> String,
    /// The label of a slot of the week whose row names no type.
    pub slot_date: &'static str,
    /// The small line of a slot of single dates: „1 Termin · 23.02.", „1 Termin", „3 Termine".
    pub one_date_on: fn(&str) -> String,
    pub dates_count: fn(usize) -> String,

    // ---- „Studiengänge" ----
    /// „12 Curricula", beside the heading.
    pub curricula: fn(usize) -> String,
    pub no_curriculum: &'static str,
    /// „Außerdem als fachübergreifendes Studium (FÜS) anrechenbar in 40 Studiengängen."
    pub fues: fn(usize) -> String,
    /// „3 weitere Nennungen gehören zu Studiengängen, die nicht im Katalog stehen."
    pub unresolved: fn(usize) -> String,
}

pub const DE: Texts = Texts {
    title: |title, id| format!("{title} ({id}) · Modul der BTU Cottbus-Senftenberg"),
    description: |title, id, credits, text| format!("{title} ({id}, {credits}) an der BTU Cottbus-Senftenberg: {text}"),
    description_bare: |title, id, credits| format!("{title} (Modul {id}, {credits}) an der BTU Cottbus-Senftenberg: Turnus, Prüfung, Voraussetzungen und Studiengänge."),
    breadcrumb_catalog: "Modulkatalog",
    plan_framework: |program, degree, po| format!("Regelstudienplan {program} ({degree}), PO {po}"),
    exam_event: |title| format!("Prüfung {title}"),

    sidebar_title: "Modul",
    not_found: "Modul nicht gefunden",
    not_found_hint: "Dieses Modul steht nicht (mehr) im Modulkatalog der BTU.",
    preview: "Modulvorschau",
    close_preview: "Vorschau schließen",
    full_view_title: "Als ganze Seite öffnen (F)",
    close_preview_title: "Vorschau schließen (Esc)",

    on_this_page: "Auf dieser Seite",
    actions: "Aktionen",
    copy_link: "Link kopieren",
    original_at_btu: "Original bei der BTU",

    dates: "Termine",
    exam_dates: "Prüfungstermine",
    at_a_glance: "Auf einen Blick",
    programs: "Studiengänge",
    prerequisites: "Voraussetzungen",
    contents: "Inhalte",
    learning_outcomes: "Lernziele",
    assessment: "Prüfungsleistung",
    literature: "Literatur",
    remarks: "Bemerkungen",
    literature_count: |n| format!("Literatur ({n})"),

    successor: "Nachfolgemodul: ",
    replaced_by: "Wird abgelöst durch: ",

    exam: "Prüfung",
    grading: "Benotung",
    duration: "Dauer",
    places: "Plätze",
    language: "Sprache",
    location: "Standort",
    responsible: "Verantwortlich",
    teaching_forms: "Lehrformen",
    graded: "benotet",
    ungraded: "unbenotet",
    places_max: |n| format!("max. {n}"),
    limited: "begrenzt",
    unlimited: "unbegrenzt",

    mandatory: "zwingend",
    recommended: "empfohlen",
    mandatory_verbatim: "Zwingend, im Wortlaut",
    recommended_verbatim: "Empfohlen, im Wortlaut",

    source: "Quelle: Modulbeschreibung der BTU",
    fetched: |date| format!(" · abgerufen {date}"),
    original: "Original",

    dates_from: |semester, next| format!("Termine aus dem {semester}. Für das {next} hat die BTU noch keine Termine zu diesem Modul veröffentlicht."),
    no_dates_winter: "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul im Wintersemester angeboten.",
    no_dates_summer: "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul im Sommersemester angeboten.",
    no_dates_every: "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul jedes Semester angeboten.",
    no_dates: "Zu diesem Modul sind keine Termine veröffentlicht.",
    date_open: "Termin offen",
    time_open: "Zeit offen",
    day_until: |day, end| format!("{day} bis {end}"),
    until: |end| format!("bis {end}"),
    in_qis: |stated| format!("In QIS: {stated}"),
    in_qis_odd: |stated, odd| format!("In QIS: {stated} · {odd}"),
    as_in_qis: |odd| format!("{odd}, so steht es in QIS"),
    unusual_time: "Uhrzeit ungewöhnlich",
    ends_before_start: "Ende vor Beginn",
    date_outside_semester: "Datum außerhalb des Semesters",
    placeholder_date_note: |semester| {
        format!("01:00–02:30 ist in QIS ein Platzhalter für eine Prüfung ohne festen Termin (oft „nach Vereinbarung“), und das Datum dazu passt nicht ins {semester}. Betula zeigt beides nicht; was die BTU angibt, steht in der Zeile.")
    },
    placeholder_time_note: "01:00–02:30 ist in QIS ein Platzhalter für eine Prüfung ohne feste Uhrzeit (oft „nach Vereinbarung“). Betula zeigt ihn nicht als Uhrzeit; was die BTU angibt, steht in der Zeile.",
    odd_time: |hours| format!("eine Uhrzeit außerhalb von {hours}"),
    odd_end: "ein Ende vor dem Beginn",
    odd_date: |semester| format!("ein Datum weit außerhalb des {semester}"),
    odd_or: " oder ",
    odd_note: |list| format!("{list} steht so in QIS, ist für eine Prüfung aber ungewöhnlich, vermutlich ein Eingabefehler."),
    slot_date: "Termin",
    one_date_on: |day| format!("1 Termin · {day}"),
    dates_count: |n| if n == 1 { "1 Termin".to_string() } else { format!("{n} Termine") },

    curricula: |n| format!("{n} Curricula"),
    no_curriculum: "Das Modul gehört zu keinem Curriculum eines Studiengangs im Katalog.",
    fues: |n| format!("Außerdem als fachübergreifendes Studium (FÜS) anrechenbar in {n} Studiengängen."),
    unresolved: |n| format!("{n} weitere Nennungen gehören zu Studiengängen, die nicht im Katalog stehen."),
};

pub const EN: Texts = Texts {
    title: |title, id| format!("{title} ({id}) · Module of BTU Cottbus-Senftenberg"),
    description: |title, id, credits, text| format!("{title} ({id}, {credits}) at BTU Cottbus-Senftenberg: {text}"),
    description_bare: |title, id, credits| format!("{title} (module {id}, {credits}) at BTU Cottbus-Senftenberg: when it is offered, exam, prerequisites and degree programmes."),
    breadcrumb_catalog: "Module catalogue",
    plan_framework: |program, degree, po| format!("Standard study plan {program} ({degree}), PO {po}"),
    exam_event: |title| format!("Exam {title}"),

    sidebar_title: "Module",
    not_found: "Module not found",
    not_found_hint: "This module is not (or no longer) in BTU's module catalogue.",
    preview: "Module preview",
    close_preview: "Close preview",
    full_view_title: "Open as a full page (F)",
    close_preview_title: "Close preview (Esc)",

    on_this_page: "On this page",
    actions: "Actions",
    copy_link: "Copy link",
    original_at_btu: "Original at BTU",

    dates: "Dates",
    exam_dates: "Exam dates",
    at_a_glance: "At a glance",
    programs: "Degree programmes",
    prerequisites: "Prerequisites",
    contents: "Contents",
    learning_outcomes: "Learning outcomes",
    assessment: "Assessment",
    literature: "Literature",
    remarks: "Remarks",
    literature_count: |n| format!("Literature ({n})"),

    successor: "Successor module: ",
    replaced_by: "Replaced by: ",

    exam: "Exam",
    grading: "Grading",
    duration: "Duration",
    places: "Places",
    language: "Language",
    location: "Location",
    responsible: "Responsible",
    teaching_forms: "Teaching forms",
    graded: "graded",
    ungraded: "ungraded",
    places_max: |n| format!("max. {n}"),
    limited: "limited",
    unlimited: "unlimited",

    mandatory: "mandatory",
    recommended: "recommended",
    mandatory_verbatim: "Mandatory, as written",
    recommended_verbatim: "Recommended, as written",

    source: "Source: BTU's module description",
    fetched: |date| format!(" · retrieved {date}"),
    original: "Original",

    dates_from: |semester, next| format!("Dates from {semester}. BTU has not published any dates for this module for {next} yet."),
    no_dates_winter: "No dates published yet. According to the module description, the module is offered in the winter semester.",
    no_dates_summer: "No dates published yet. According to the module description, the module is offered in the summer semester.",
    no_dates_every: "No dates published yet. According to the module description, the module is offered every semester.",
    no_dates: "No dates have been published for this module.",
    date_open: "Date TBA",
    time_open: "Time TBA",
    day_until: |day, end| format!("{day} until {end}"),
    until: |end| format!("until {end}"),
    in_qis: |stated| format!("In QIS: {stated}"),
    in_qis_odd: |stated, odd| format!("In QIS: {stated} · {odd}"),
    as_in_qis: |odd| format!("{odd}, as stated in QIS"),
    unusual_time: "Unusual time",
    ends_before_start: "End before start",
    date_outside_semester: "Date outside the semester",
    placeholder_date_note: |semester| {
        format!("In QIS, 01:00–02:30 is a placeholder for an exam without a fixed date (often \u{201c}by arrangement\u{201d}), and the date given with it does not fall within {semester}. Betula shows neither; what BTU states is in the row.")
    },
    placeholder_time_note: "In QIS, 01:00–02:30 is a placeholder for an exam without a fixed time (often \u{201c}by arrangement\u{201d}). Betula does not show it as a time; what BTU states is in the row.",
    odd_time: |hours| format!("a time outside {hours}"),
    odd_end: "an end before the start",
    odd_date: |semester| format!("a date far outside {semester}"),
    odd_or: " or ",
    odd_note: |list| format!("{list} is what QIS states, but it is unusual for an exam, probably an input error."),
    slot_date: "Session",
    one_date_on: |day| format!("1 date · {day}"),
    dates_count: |n| if n == 1 { "1 date".to_string() } else { format!("{n} dates") },

    curricula: |n| if n == 1 { "1 curriculum".to_string() } else { format!("{n} curricula") },
    no_curriculum: "The module is in no curriculum of a degree programme in the catalogue.",
    fues: |n| {
        if n == 1 {
            "Also creditable as interdisciplinary studies (FÜS) in 1 degree programme.".to_string()
        } else {
            format!("Also creditable as interdisciplinary studies (FÜS) in {n} degree programmes.")
        }
    },
    unresolved: |n| {
        if n == 1 {
            "1 further mention belongs to a degree programme that is not in the catalogue.".to_string()
        } else {
            format!("{n} further mentions belong to degree programmes that are not in the catalogue.")
        }
    },
};
