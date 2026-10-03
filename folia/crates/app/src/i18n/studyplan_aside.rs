//! Texts of what stands beside the Stundenplan (`pages/studyplan/aside.rs`): a planned module, its
//! Termine and what the plan shows of them, where it stands in the plan, and its notes. What QIS
//! says (an event's type and title, rooms, groups, a rhythm in its own words) stands as it says
//! it; weekdays, dates and semesters are the data contract's (`folia_locale`), and so are the
//! words of what avoids an exam warning (`plans`) and of an exam („Prüfung", „Uhrzeit offen",
//! „bis 24:00", `timetable`).

use folia_calendar::day::Day;
use folia_calendar::kind::EventKind;
use folia_locale::Locale;

use super::studyplan_week::days;

pub struct Texts {
    // ---- the panel ----
    /// The panel's name for screen readers: „Modul im Plan".
    pub panel: &'static str,
    /// The link to the module's whole page: its tooltip with its key („Als ganze Seite öffnen
    /// (F)") and its text („Modul ansehen").
    pub full_title: &'static str,
    pub full: &'static str,
    /// The tooltip of „Schließen", with its key: „Schließen (Esc)".
    pub close_title: &'static str,
    /// Under the title of a module the catalog does not know: „Nicht im Modulkatalog."
    pub not_in_catalog: &'static str,
    /// Instead of the Termine of a module planned in no semester: „Nicht im Studienplan."
    pub not_planned: &'static str,

    // ---- where the module stands ----
    /// The select of the placeholder the module counts for: its name („Zählt für"), the choice
    /// of none („Für keinen Platzhalter") and a placeholder by its name („Für „Anwendungsfach“").
    pub counts_for: &'static str,
    pub no_placeholder: &'static str,
    pub for_placeholder: fn(&str) -> String,
    /// Takes the module out of the plan: „Entfernen".
    pub remove: &'static str,
    /// Before the other semesters the module is planned in, which follow as links: „Auch
    /// geplant: ".
    pub also_planned: &'static str,

    // ---- a semester without the module's dates (the semester's name written already) ----
    /// A semester that is over: „WiSe 2025/26 ist vorbei; vergangene Termine fehlen im
    /// Datenstand." …
    pub past: fn(&str) -> String,
    /// … one whose dates are not out yet: „SoSe 2027: noch keine Termine veröffentlicht." …
    pub unpublished: fn(&str) -> String,
    /// … one with dates, none of the module's, and what its description says of its season
    /// (`said_summer`, `said_winter`, or nothing): „Keine Termine im WiSe 2026/27 (laut
    /// Beschreibung im Sommer)."
    pub no_dates: fn(&str, &str) -> String,
    pub said_summer: &'static str,
    pub said_winter: &'static str,

    // ---- an event or an exam, its choice and its Termine ----
    /// The eye of an event („Veranstaltung ausblenden", „… einblenden") and of an exam
    /// („Prüfung ausblenden", „… einblenden").
    pub hide_event: &'static str,
    pub show_event: &'static str,
    pub hide_exam: &'static str,
    pub show_exam: &'static str,
    /// The link to the event's page in QIS: „In QIS".
    pub in_qis: &'static str,
    /// An event's choice, of how many options: „1 von 4 wählen" …
    pub choose_one_of: fn(usize) -> String,
    /// … the button and tooltip that open it again once it is made („Wahl aufheben") …
    pub clear_choice: &'static str,
    /// … and the tooltip of an option's line („Diesen wählen").
    pub choose_this: &'static str,
    /// An option QIS gives no group name for, by its place: „Gruppe 2".
    pub group: fn(usize) -> String,
    /// An event or exam QIS lists without any date: „Termine nicht angegeben".
    pub undated: &'static str,
    /// The eye of a Termin: „Termin ausblenden", „Termin einblenden".
    pub hide_date: &'static str,
    pub show_date: &'static str,
    /// Beside a Termin in a hard clash: „Überschneidung".
    pub clash: &'static str,

    // ---- why an event is hidden, where that is decided elsewhere ----
    /// Its kinds switched off in the sidebar, as the chips' plurals (`plural`): „Vorlesungen und
    /// Übungen ausgeblendet", „Ausgeblendet" without a kind.
    pub kinds_hidden: fn(&[&str]) -> String,
    pub plural: fn(EventKind) -> &'static str,
    /// The course of the other town, named by the town it is held in: „Standort Senftenberg".
    pub location: fn(&str) -> String,

    // ---- the module's notes ----
    /// Exams on one day whose place is open: how many, the day (written already): „Ort offen: 2
    /// Prüfungen am Fr 12.03.2027".
    pub room_open: fn(usize, &str) -> String,
    /// A module taught in both towns while no Standort is chosen: „Standort wählen: Cottbus oder
    /// Senftenberg".
    pub choose_location: &'static str,
    /// Two exams at once (the day and time, then the modules, joined with „ · "): „Prüfungen
    /// gleichzeitig: Mo 08.02.2027 11:00 · …" …
    pub same_time: fn(&str) -> String,
    /// … or a hop between campuses too short: the minutes, from where, to where, then the day and
    /// the two sittings (`ends`, `starts`): „0 min von Zentralcampus nach Senftenberg: …" …
    pub tight: fn(u16, &str, &str, &str) -> String,
    /// … the module whose sitting ends, and when: „Kraftwerkstechnik I bis 10:00" …
    pub ends: fn(&str, &str) -> String,
    /// … the module whose sitting starts, and when: „Gentechnik ab 10:00".
    pub starts: fn(&str, &str) -> String,

    // ---- a Termin's two lines ----
    /// Its time where there is no clear one: the whole day („ganztags"), a time QIS gives that
    /// cannot be read („Zeit unklar"), none („Zeit offen").
    pub all_day: &'static str,
    pub time_unclear: &'static str,
    pub time_open: &'static str,
    /// Which weeks, where it is not „A", „B" or „A/B": once („einmalig"), every fourth week
    /// („4-wöchentlich"), the days of a block („Block"), a rhythm QIS names in no words of its own
    /// („nach Absprache"), none („Rhythmus offen").
    pub once: &'static str,
    pub every_four_weeks: &'static str,
    pub block: &'static str,
    pub by_arrangement: &'static str,
    pub rhythm_open: &'static str,
    /// How many dates it has: „1 Termin", „14 Termine".
    pub sessions: fn(usize) -> String,
    /// How many of them are cancelled: „1 fällt aus", „2 fallen aus"; a single date „fällt aus".
    pub cancelled: fn(usize) -> String,
    pub cancelled_once: &'static str,
    /// Its range where QIS states none, and the lecture period stands in: „Vorlesungszeit".
    pub lecture_period: &'static str,
    /// The days of a block or a window, the month said once where both share it: „08.–19.02.",
    /// „28.01.–05.02.".
    pub span: fn(Day, Day) -> String,
    /// An exam sitting whose date QIS leaves open: „Termin offen".
    pub date_open: &'static str,
}

pub const DE: Texts = Texts {
    panel: "Modul im Plan",
    full_title: "Als ganze Seite öffnen (F)",
    full: "Modul ansehen",
    close_title: "Schließen (Esc)",
    not_in_catalog: "Nicht im Modulkatalog.",
    not_planned: "Nicht im Studienplan.",

    counts_for: "Zählt für",
    no_placeholder: "Für keinen Platzhalter",
    for_placeholder: |name| format!("Für „{name}“"),
    remove: "Entfernen",
    also_planned: "Auch geplant: ",

    past: |semester| format!("{semester} ist vorbei; vergangene Termine fehlen im Datenstand."),
    unpublished: |semester| format!("{semester}: noch keine Termine veröffentlicht."),
    no_dates: |semester, said| format!("Keine Termine im {semester}{said}."),
    said_summer: " (laut Beschreibung im Sommer)",
    said_winter: " (laut Beschreibung im Winter)",

    hide_event: "Veranstaltung ausblenden",
    show_event: "Veranstaltung einblenden",
    hide_exam: "Prüfung ausblenden",
    show_exam: "Prüfung einblenden",
    in_qis: "In QIS",
    choose_one_of: |n| format!("1 von {n} wählen"),
    clear_choice: "Wahl aufheben",
    choose_this: "Diesen wählen",
    group: |n| format!("Gruppe {n}"),
    undated: "Termine nicht angegeben",
    hide_date: "Termin ausblenden",
    show_date: "Termin einblenden",
    clash: "Überschneidung",

    kinds_hidden: |kinds| match kinds {
        [] => "Ausgeblendet".to_string(),
        [one] => format!("{one} ausgeblendet"),
        [rest @ .., last] => format!("{} und {last} ausgeblendet", rest.join(", ")),
    },
    plural: |kind| match kind {
        EventKind::Lecture => "Vorlesungen",
        EventKind::Exercise => "Übungen",
        EventKind::Seminar => "Seminare",
        EventKind::Practical => "Praktika",
        EventKind::Project => "Projekte",
        EventKind::Tutorial => "Tutorien",
        EventKind::Consultation => "Konsultationen",
        EventKind::Excursion => "Exkursionen",
        EventKind::SelfStudy => "Selbststudium",
        EventKind::Paper => "Hausarbeiten",
        EventKind::Other => "Sonstige",
        EventKind::Exam => "Prüfungen",
    },
    location: |town| format!("Standort {town}"),

    room_open: |n, day| format!("Ort offen: {n} Prüfungen am {day}"),
    choose_location: "Standort wählen: Cottbus oder Senftenberg",
    same_time: |what| format!("Prüfungen gleichzeitig: {what}"),
    tight: |gap, from, to, what| format!("{gap} min von {from} nach {to}: {what}"),
    ends: |module, time| format!("{module} bis {time}"),
    starts: |module, time| format!("{module} ab {time}"),

    all_day: "ganztags",
    time_unclear: "Zeit unklar",
    time_open: "Zeit offen",
    once: "einmalig",
    every_four_weeks: "4-wöchentlich",
    block: "Block",
    by_arrangement: "nach Absprache",
    rhythm_open: "Rhythmus offen",
    sessions: |n| if n == 1 { "1 Termin".to_string() } else { format!("{n} Termine") },
    cancelled: |n| if n == 1 { "1 fällt aus".to_string() } else { format!("{n} fallen aus") },
    cancelled_once: "fällt aus",
    lecture_period: "Vorlesungszeit",
    span: |first, last| days(first, last, false, Locale::De),
    date_open: "Termin offen",
};

pub const EN: Texts = Texts {
    panel: "Module in the plan",
    full_title: "Open as a full page (F)",
    full: "View module",
    close_title: "Close (Esc)",
    not_in_catalog: "Not in the module catalogue.",
    not_planned: "Not in the timetable.",

    counts_for: "Counts for",
    no_placeholder: "For no placeholder",
    for_placeholder: |name| format!("For “{name}”"),
    remove: "Remove",
    also_planned: "Also planned: ",

    past: |semester| format!("{semester} is over; past dates are missing from the data."),
    unpublished: |semester| format!("{semester}: no dates published yet."),
    no_dates: |semester, said| format!("No dates in {semester}{said}."),
    said_summer: " (in summer, according to the description)",
    said_winter: " (in winter, according to the description)",

    hide_event: "Hide course",
    show_event: "Show course",
    hide_exam: "Hide exam",
    show_exam: "Show exam",
    in_qis: "View in QIS",
    choose_one_of: |n| format!("Choose 1 of {n}"),
    clear_choice: "Clear choice",
    choose_this: "Choose this one",
    group: |n| format!("Group {n}"),
    undated: "Dates not stated",
    hide_date: "Hide this date",
    show_date: "Show this date",
    clash: "Clash",

    kinds_hidden: |kinds| {
        let text = match kinds {
            [] => "hidden".to_string(),
            [one] => format!("{one} hidden"),
            [rest @ .., last] => format!("{} and {last} hidden", rest.join(", ")),
        };
        capitalized(&text)
    },
    plural: |kind| match kind {
        EventKind::Lecture => "lectures",
        EventKind::Exercise => "exercises",
        EventKind::Seminar => "seminars",
        EventKind::Practical => "practicals",
        EventKind::Project => "projects",
        EventKind::Tutorial => "tutorials",
        EventKind::Consultation => "consultations",
        EventKind::Excursion => "excursions",
        EventKind::SelfStudy => "self-study",
        EventKind::Paper => "term papers",
        EventKind::Other => "other events",
        EventKind::Exam => "exams",
    },
    location: |town| format!("Location: {town}"),

    room_open: |n, day| if n == 1 { format!("Room TBA: 1 exam on {day}") } else { format!("Room TBA: {n} exams on {day}") },
    choose_location: "Choose a location: Cottbus or Senftenberg",
    same_time: |what| format!("Exams at the same time: {what}"),
    tight: |gap, from, to, what| format!("{gap} min from {from} to {to}: {what}"),
    ends: |module, time| format!("{module} until {time}"),
    starts: |module, time| format!("{module} from {time}"),

    all_day: "all day",
    time_unclear: "time unclear",
    time_open: "time TBA",
    once: "once",
    every_four_weeks: "every 4 weeks",
    block: "block",
    by_arrangement: "by arrangement",
    rhythm_open: "rhythm TBA",
    sessions: |n| if n == 1 { "1 session".to_string() } else { format!("{n} sessions") },
    cancelled: |n| format!("{n} cancelled"),
    cancelled_once: "cancelled",
    lecture_period: "lecture period",
    span: |first, last| days(first, last, false, Locale::En),
    date_open: "date TBA",
};

/// `text` with its first letter in upper case: English writes the kinds in a sentence in lower
/// case, but its first word.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
