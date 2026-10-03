//! Texts of the head of the Stundenplan (`pages/studyplan/head.rs`): the line under its heading,
//! the marked modules, the empty week, the exams that collide, the line of overlaps, what is hidden
//! and what is derived. (The heading itself is the page's name, `studyplan::Texts::title`.)

pub struct Texts {
    // ---- the head ----
    /// „1 Platzhalter", „2 Platzhalter": the placeholders open in the semester, in the line under
    /// the heading and in what the import adds.
    pub placeholders: fn(usize) -> String,
    /// „WiSe 2025/26 ist vorbei; vergangene Termine fehlen im Datenstand.": a semester before the
    /// current one (its name).
    pub past: fn(&str) -> String,
    /// „SoSe 2027: noch keine Termine veröffentlicht."
    pub unpublished: fn(&str) -> String,

    // ---- the Merkliste ----
    /// „Aus der Merkliste (3)": the marked modules the semester could still take, closed until
    /// asked.
    pub from_bookmarks: fn(usize) -> String,
    /// „Einplanen": a marked module's button, and „Eingeplant" right after its click.
    pub plan: &'static str,
    pub planned: &'static str,
    /// „3 gemerkte Module übernehmen": the empty week's button that plans them all at once …
    pub take_marked: fn(usize) -> String,
    /// … and its note, with „Rückgängig": „3 gemerkte Module übernommen".
    pub marked_taken: fn(usize) -> String,
    /// „Noch keine Termine": in the middle of the week with nothing planned …
    pub no_dates: &'static str,
    /// … and „Zum Katalog", the way to the catalog's modules that fit the week.
    pub to_catalog: &'static str,

    // ---- the exams that collide ----
    /// A module the catalog has no title for: „Modul 12104".
    pub module_numbered: fn(&str) -> String,
    /// „Zentralcampus": the campus as a hop between two exams names it (Sachsendorf and
    /// Senftenberg are names).
    pub central_campus: &'static str,
    /// Two exams at once (the modules' titles): „Prüfung A und Prüfung B überschneiden sich" …
    pub exams_clash: fn(&str, &str) -> String,
    /// … or too close for the way between their campuses (the titles, the minutes between, the
    /// campuses): „Prüfung A und Prüfung B: nur 0 min zwischen Zentralcampus und Senftenberg".
    pub exams_tight: fn(&str, &str, u16, &str, &str) -> String,
    /// Their times after the day: „beide 11:00–13:00" …
    pub both_at: fn(&str) -> String,
    /// … „08:00–10:00 und 10:00–12:00".
    pub each_at: fn(&str, &str) -> String,

    // ---- overlaps and open choices ----
    /// The parts of the one quiet line: „1 Überschneidung pro Woche", „2 Überschneidungen pro
    /// Woche" …
    pub clashes_per_week: fn(usize) -> String,
    /// … „2 Überschneidungen an einzelnen Tagen" alone …
    pub clashes_on_days: fn(usize) -> String,
    /// … and after those per week only „1 an einzelnen Tagen" …
    pub more_on_days: fn(usize) -> String,
    /// … „3 Wahlen offen" …
    pub choices_open: fn(usize) -> String,
    /// … „Standort offen" …
    pub town_open: &'static str,
    /// … „2 ausgeblendet".
    pub hidden: fn(usize) -> String,
    /// After an overlap's times, the week it is in: „A-Woche", „B-Woche".
    pub week_a: &'static str,
    pub week_b: &'static str,
    /// „0 von 4 Terminen frei: Übung · Mathematik IT-1": a choice none of whose options is free
    /// (how many options there are, the event's type as QIS writes it, the title); the neutral
    /// „Terminen", never glued to the type.
    pub blocked: fn(usize, &str, &str) -> String,
    /// „1 von 3 wählen: Übung · Mathematik IT-1": a choice still open.
    pub choose_one: fn(usize, &str, &str) -> String,
    /// „Standort wählen: Entwicklung von Softwaresystemen · Cottbus oder Senftenberg": the modules
    /// taught in both towns, their titles joined with „, ".
    pub choose_town: fn(&str) -> String,

    // ---- what is hidden ----
    /// „Prüfung Mathematik IT-1": an exam by its module's title.
    pub exam_of: fn(&str) -> String,
    /// „nur Mo 15:30": the one option a choice was narrowed to, by when it meets.
    pub only: fn(&str) -> String,
    /// „Einblenden": takes one line back …
    pub unhide: &'static str,
    /// … „Alle einblenden": every line.
    pub unhide_all: &'static str,

    // ---- what is derived ----
    /// „Abgeleitet: Vorlesungszeit …, Standort Cottbus.": the parts joined with „, ".
    pub derived: fn(&str) -> String,
    /// „Vorlesungszeit 05.10.2026–31.01.2027": the first and the last day, written already.
    pub lecture_period: fn(&str, &str) -> String,
    /// „vorlesungsfrei 21.12.–03.01.": the breaks, written already.
    pub lecture_break: fn(&str) -> String,
    /// „Übungsgruppen": parallel slots of which the SWS need one.
    pub groups_derived: &'static str,
    /// „Standort Cottbus": the town the plan's other modules are taught in.
    pub town_derived: fn(&str) -> String,
}

pub const DE: Texts = Texts {
    placeholders: |n| format!("{n} Platzhalter"),
    past: |semester| format!("{semester} ist vorbei; vergangene Termine fehlen im Datenstand."),
    unpublished: |semester| format!("{semester}: noch keine Termine veröffentlicht."),

    from_bookmarks: |n| format!("Aus der Merkliste ({n})"),
    plan: "Einplanen",
    planned: "Eingeplant",
    take_marked: |n| match n {
        1 => "1 gemerktes Modul übernehmen".to_string(),
        n => format!("{n} gemerkte Module übernehmen"),
    },
    marked_taken: |n| match n {
        1 => "1 gemerktes Modul übernommen".to_string(),
        n => format!("{n} gemerkte Module übernommen"),
    },
    no_dates: "Noch keine Termine",
    to_catalog: "Zum Katalog",

    module_numbered: |id| format!("Modul {id}"),
    central_campus: "Zentralcampus",
    exams_clash: |a, b| format!("Prüfung {a} und Prüfung {b} überschneiden sich"),
    exams_tight: |a, b, gap, from, to| format!("Prüfung {a} und Prüfung {b}: nur {gap} min zwischen {from} und {to}"),
    both_at: |span| format!("beide {span}"),
    each_at: |a, b| format!("{a} und {b}"),

    clashes_per_week: |n| match n {
        1 => "1 Überschneidung pro Woche".to_string(),
        n => format!("{n} Überschneidungen pro Woche"),
    },
    clashes_on_days: |n| match n {
        1 => "1 Überschneidung an einzelnen Tagen".to_string(),
        n => format!("{n} Überschneidungen an einzelnen Tagen"),
    },
    more_on_days: |n| format!("{n} an einzelnen Tagen"),
    choices_open: |n| match n {
        1 => "1 Wahl offen".to_string(),
        n => format!("{n} Wahlen offen"),
    },
    town_open: "Standort offen",
    hidden: |n| format!("{n} ausgeblendet"),
    week_a: "A-Woche",
    week_b: "B-Woche",
    blocked: |options, kind, title| format!("0 von {options} Terminen frei: {kind} · {title}"),
    choose_one: |options, kind, title| format!("1 von {options} wählen: {kind} · {title}"),
    choose_town: |modules| format!("Standort wählen: {modules} · Cottbus oder Senftenberg"),

    exam_of: |title| format!("Prüfung {title}"),
    only: |when| format!("nur {when}"),
    unhide: "Einblenden",
    unhide_all: "Alle einblenden",

    derived: |parts| format!("Abgeleitet: {parts}."),
    lecture_period: |first, last| format!("Vorlesungszeit {first}–{last}"),
    lecture_break: |breaks| format!("vorlesungsfrei {breaks}"),
    groups_derived: "Übungsgruppen",
    town_derived: |town| format!("Standort {town}"),
};

pub const EN: Texts = Texts {
    placeholders: |n| match n {
        1 => "1 placeholder".to_string(),
        n => format!("{n} placeholders"),
    },
    past: |semester| format!("{semester} is over; past dates are missing from the data."),
    unpublished: |semester| format!("{semester}: no dates published yet."),

    from_bookmarks: |n| format!("From saved modules ({n})"),
    plan: "Plan",
    planned: "Planned",
    take_marked: |n| match n {
        1 => "Add 1 saved module".to_string(),
        n => format!("Add {n} saved modules"),
    },
    marked_taken: |n| match n {
        1 => "1 saved module added".to_string(),
        n => format!("{n} saved modules added"),
    },
    no_dates: "No dates yet",
    to_catalog: "To the catalogue",

    module_numbered: |id| format!("Module {id}"),
    central_campus: "Central Campus",
    exams_clash: |a, b| format!("The exams of {a} and {b} clash"),
    exams_tight: |a, b, gap, from, to| format!("The exams of {a} and {b}: only {gap} min between {from} and {to}"),
    both_at: |span| format!("both {span}"),
    each_at: |a, b| format!("{a} and {b}"),

    clashes_per_week: |n| match n {
        1 => "1 clash per week".to_string(),
        n => format!("{n} clashes per week"),
    },
    clashes_on_days: |n| match n {
        1 => "1 clash on single days".to_string(),
        n => format!("{n} clashes on single days"),
    },
    more_on_days: |n| format!("{n} on single days"),
    choices_open: |n| match n {
        1 => "1 choice open".to_string(),
        n => format!("{n} choices open"),
    },
    town_open: "Location open",
    hidden: |n| format!("{n} hidden"),
    week_a: "week A",
    week_b: "week B",
    blocked: |options, kind, title| format!("0 of {options} sessions free: {kind} · {title}"),
    choose_one: |options, kind, title| format!("Choose 1 of {options}: {kind} · {title}"),
    choose_town: |modules| format!("Choose a location: {modules} · Cottbus or Senftenberg"),

    exam_of: |title| format!("Exam {title}"),
    only: |when| format!("only {when}"),
    unhide: "Show",
    unhide_all: "Show all",

    derived: |parts| format!("Derived: {parts}."),
    lecture_period: |first, last| format!("lecture period {first}–{last}"),
    lecture_break: |breaks| format!("lecture break {breaks}"),
    groups_derived: "exercise groups",
    town_derived: |town| format!("location {town}"),
};
