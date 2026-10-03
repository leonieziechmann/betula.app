//! Texts of the Stundenplan's logic (`timetable/*`): the finder, clashes, exams, the calendar
//! feed, holidays.
//!
//! What QIS says stays as QIS says it, in every language: an event's type („Vorlesung/Übung"),
//! its title, rooms, people, a rhythm QIS writes in words of its own and the reason it gives for a
//! cancelled date. The words the timetable matches QIS's text with (`cancel`, `exams`, `kind`) are
//! no texts either: they read the data.

pub struct Texts {
    // The finder („Passt in meinen Stundenplan", `fit`): the note at a module of the catalog.
    /// A module whose rows have no time to compare: „keine festen Termine".
    pub no_fixed_dates: &'static str,
    /// A module with nothing to compare whose dates with a time are all of classes the visitor
    /// left out of the comparison: „nicht verglichen".
    pub not_compared: &'static str,
    /// A module whose only exam dates are retakes, one of them free: „nur
    /// Wiederholungsprüfung".
    pub retake_only: &'static str,
    /// Its first exam date overlaps one the plan holds, or its exam meets the first sitting
    /// („Erstermin") of a planned module: „Prüfung überschneidet sich mit Erstermin".
    pub exam_clashes_with_first: &'static str,
    /// The same for a hop between campuses that is too short: „Prüfung zu knapp am Erstermin".
    pub exam_close_to_first: &'static str,
    /// The town whose course of a module fits, where the other's does not (the town is a name):
    /// „nur in Senftenberg".
    pub only_in: fn(&str) -> String,
    /// Some but not all of a module's lectures are free (how many, of how many; at least 2):
    /// „Vorlesung 1 von 2 frei".
    pub lectures_free: fn(usize, usize) -> String,
    /// The same for its other events, Übungen, Seminare, Praktika …: „Übung 1 von 3 frei".
    pub exercises_free: fn(usize, usize) -> String,

    // Brandenburg's public holidays (`day::Holiday`), as the agenda and the calendar feed name
    // them.
    /// „Neujahr"
    pub new_year: &'static str,
    /// „Karfreitag"
    pub good_friday: &'static str,
    /// „Ostersonntag"
    pub easter_sunday: &'static str,
    /// „Ostermontag"
    pub easter_monday: &'static str,
    /// „Tag der Arbeit"
    pub labour_day: &'static str,
    /// „Christi Himmelfahrt"
    pub ascension_day: &'static str,
    /// „Pfingstsonntag"
    pub whit_sunday: &'static str,
    /// „Pfingstmontag"
    pub whit_monday: &'static str,
    /// „Tag der Deutschen Einheit"
    pub german_unity_day: &'static str,
    /// „Reformationstag"
    pub reformation_day: &'static str,
    /// „1. Weihnachtstag"
    pub christmas_day: &'static str,
    /// „2. Weihnachtstag"
    pub boxing_day: &'static str,
    /// Why a recurring date in the lecture break is not held, as the feed names it after the
    /// dates it drops: „vorlesungsfrei".
    pub lecture_break: &'static str,

    // The calendar a visitor downloads or subscribes to (`export`): what it says around the data.
    /// The calendar's name with the semester's label: „Studienplan WiSe 2026/27"; with none
    /// (empty), „Studienplan".
    pub feed_name: fn(&str) -> String,
    /// What the calendar says of itself: „Betula (inoffiziell) · Termine laut QIS" …
    pub feed_about: &'static str,
    /// … and after a comma the date of its data: „Stand 23.09.2026" (the date is written already)
    /// …
    pub feed_as_of: fn(&str) -> String,
    /// … and after „ · " that the semester has no dates yet: „Noch keine Termine veröffentlicht" …
    pub feed_unpublished: &'static str,
    /// … or that it holds only its earliest entries: „nur die ersten 5000 Termine".
    pub feed_first_only: fn(usize) -> String,
    /// The last line of every entry's description: „Quelle: QIS".
    pub feed_source: &'static str,
    /// The rooms of an entry as QIS names them (how many, the names joined with „ / "): „Raum:
    /// …", „Räume: … / …".
    pub feed_rooms: fn(usize, &str) -> String,
    /// Who teaches (the names joined with „ / "): „Lehrende: Bleicher / Freymann".
    pub feed_lecturers: fn(&str) -> String,
    /// The title of an entry that is one option of a choice not made yet, after the slot's short
    /// text (how many options): „Ü AuP · 1 von 3".
    pub feed_one_of: fn(&str, usize) -> String,
    /// A room note QIS gives for the entry's date: „Hinweis: Raumwechsel".
    pub feed_note: fn(&str) -> String,
    /// The group QIS names: „Gruppe: 1-Gruppe".
    pub feed_group: fn(&str) -> String,
    /// A range QIS does not state, taken from the lecture period (first and last day, written
    /// already): „Zeitraum in QIS nicht angegeben; angenommen: Vorlesungszeit
    /// 05.10.2026–31.01.2027".
    pub feed_assumed: fn(&str, &str) -> String,
    /// An option of a choice not made yet (how many options, what they are, below): „Eine von 3
    /// Übungsgruppen; in Betula wählen".
    pub feed_choose: fn(usize, &str) -> String,
    /// What the options of a choice are, in `feed_choose`: Übungen („Übungsgruppen") …
    pub exercise_groups: &'static str,
    /// … Praktika („Praktikumsgruppen") …
    pub practical_groups: &'static str,
    /// … Seminare („Seminargruppen") …
    pub seminar_groups: &'static str,
    /// … Tutorien („Tutoriumsgruppen") …
    pub tutorial_groups: &'static str,
    /// … Projekte („Projektgruppen") …
    pub project_groups: &'static str,
    /// … or anything else („Gruppen").
    pub groups: &'static str,
    /// A module of an entry by its number and, where the catalog knows it, its title: „Modul
    /// 12104 Entwicklung von Softwaresystemen", „Modul 12104".
    pub feed_module: fn(&str, Option<&str>) -> String,
    /// The rhythm of a Termin in its description, before its range (a rhythm QIS writes in words
    /// of its own is shown as it writes it): weekly („wöchentlich") …
    pub weekly: &'static str,
    /// … every other week, in A weeks („14-täglich (A-Woche)") …
    pub fortnightly_a: &'static str,
    /// … in B weeks („14-täglich (B-Woche)") …
    pub fortnightly_b: &'static str,
    /// … every four weeks („vierwöchentlich") …
    pub every_four_weeks: &'static str,
    /// … once („Einzeltermin") …
    pub single_date: &'static str,
    /// … on the days of a block („Blockveranstaltung").
    pub block_course: &'static str,
    /// The dates a Termin drops, each with why (written already): „Entfällt: 22.12., 29.12.
    /// (vorlesungsfrei); 31.10. (Reformationstag); 05.11. (laut QIS)".
    pub feed_dropped: fn(&str) -> String,
    /// Why QIS cancels a date where it gives no reason: „laut QIS".
    pub cancelled_by_qis: &'static str,
    /// What an exam entry is, before its module's short name („Prüfung EvS") and in its
    /// description: a sitting, or a day without a time („Prüfung") …
    pub exam: &'static str,
    /// … something due that day („Abgabe") …
    pub submission: &'static str,
    /// … several days („Prüfungszeitraum").
    pub exam_period: &'static str,
    /// What the whole day of a submission leaves open, in parentheses after what it is: „bis
    /// 24:00".
    pub by_midnight: &'static str,
    /// The same for an exam day without a time: „Uhrzeit offen".
    pub time_open: &'static str,
    /// After „ · " in the title of a retake's entry: „Wdh.".
    pub retake: &'static str,
    /// After „ · " in the title of a later sitting's entry: „2. Termin".
    pub second_sitting: &'static str,
    /// What is odd about an exam date shown as QIS states it, joined with „, ": a start before
    /// 06:00 or an end after 22:00 („Uhrzeit ungewöhnlich") …
    pub unusual_time: &'static str,
    /// … an end before the start („Ende vor Beginn") …
    pub ends_before_start: &'static str,
    /// … a date outside the semester („Datum außerhalb des Semesters") …
    pub outside_semester: &'static str,
    /// … in one line: „Laut QIS: Uhrzeit ungewöhnlich, Ende vor Beginn".
    pub qis_states: fn(&str) -> String,
}

pub const DE: Texts = Texts {
    no_fixed_dates: "keine festen Termine",
    not_compared: "nicht verglichen",
    retake_only: "nur Wiederholungsprüfung",
    exam_clashes_with_first: "Prüfung überschneidet sich mit Erstermin",
    exam_close_to_first: "Prüfung zu knapp am Erstermin",
    only_in: |town| format!("nur in {town}"),
    lectures_free: |free, all| format!("Vorlesung {free} von {all} frei"),
    exercises_free: |free, all| format!("Übung {free} von {all} frei"),

    new_year: "Neujahr",
    good_friday: "Karfreitag",
    easter_sunday: "Ostersonntag",
    easter_monday: "Ostermontag",
    labour_day: "Tag der Arbeit",
    ascension_day: "Christi Himmelfahrt",
    whit_sunday: "Pfingstsonntag",
    whit_monday: "Pfingstmontag",
    german_unity_day: "Tag der Deutschen Einheit",
    reformation_day: "Reformationstag",
    christmas_day: "1. Weihnachtstag",
    boxing_day: "2. Weihnachtstag",
    lecture_break: "vorlesungsfrei",

    feed_name: |label| match label.is_empty() {
        true => "Studienplan".to_string(),
        false => format!("Studienplan {label}"),
    },
    feed_about: "Betula (inoffiziell) · Termine laut QIS",
    feed_as_of: |date| format!("Stand {date}"),
    feed_unpublished: "Noch keine Termine veröffentlicht",
    feed_first_only: |n| format!("nur die ersten {n} Termine"),
    feed_source: "Quelle: QIS",
    feed_rooms: |n, rooms| if n > 1 { format!("Räume: {rooms}") } else { format!("Raum: {rooms}") },
    feed_lecturers: |names| format!("Lehrende: {names}"),
    feed_one_of: |label, n| format!("{label} · 1 von {n}"),
    feed_note: |note| format!("Hinweis: {note}"),
    feed_group: |group| format!("Gruppe: {group}"),
    feed_assumed: |first, last| format!("Zeitraum in QIS nicht angegeben; angenommen: Vorlesungszeit {first}–{last}"),
    feed_choose: |n, groups| format!("Eine von {n} {groups}; in Betula wählen"),
    exercise_groups: "Übungsgruppen",
    practical_groups: "Praktikumsgruppen",
    seminar_groups: "Seminargruppen",
    tutorial_groups: "Tutoriumsgruppen",
    project_groups: "Projektgruppen",
    groups: "Gruppen",
    feed_module: |id, title| match title {
        Some(title) => format!("Modul {id} {title}"),
        None => format!("Modul {id}"),
    },
    weekly: "wöchentlich",
    fortnightly_a: "14-täglich (A-Woche)",
    fortnightly_b: "14-täglich (B-Woche)",
    every_four_weeks: "vierwöchentlich",
    single_date: "Einzeltermin",
    block_course: "Blockveranstaltung",
    feed_dropped: |dates| format!("Entfällt: {dates}"),
    cancelled_by_qis: "laut QIS",
    exam: "Prüfung",
    submission: "Abgabe",
    exam_period: "Prüfungszeitraum",
    by_midnight: "bis 24:00",
    time_open: "Uhrzeit offen",
    retake: "Wdh.",
    second_sitting: "2. Termin",
    unusual_time: "Uhrzeit ungewöhnlich",
    ends_before_start: "Ende vor Beginn",
    outside_semester: "Datum außerhalb des Semesters",
    qis_states: |odd| format!("Laut QIS: {odd}"),
};

pub const EN: Texts = Texts {
    no_fixed_dates: "no fixed dates",
    not_compared: "not compared",
    retake_only: "retake exam only",
    exam_clashes_with_first: "exam clashes with a first sitting",
    exam_close_to_first: "exam too close to a first sitting",
    only_in: |town| format!("only in {town}"),
    lectures_free: |free, all| format!("{free} of {all} lectures free"),
    exercises_free: |free, all| format!("{free} of {all} exercises free"),

    new_year: "New Year's Day",
    good_friday: "Good Friday",
    easter_sunday: "Easter Sunday",
    easter_monday: "Easter Monday",
    labour_day: "Labour Day",
    ascension_day: "Ascension Day",
    whit_sunday: "Whit Sunday",
    whit_monday: "Whit Monday",
    german_unity_day: "Day of German Unity",
    reformation_day: "Reformation Day",
    christmas_day: "Christmas Day",
    boxing_day: "Boxing Day",
    lecture_break: "no lectures",

    feed_name: |label| match label.is_empty() {
        true => "Timetable".to_string(),
        false => format!("Timetable {label}"),
    },
    feed_about: "Betula (unofficial) · dates according to QIS",
    feed_as_of: |date| format!("as of {date}"),
    feed_unpublished: "No dates published yet",
    feed_first_only: |n| format!("only the first {} dates", super::Locale::En.thousands(u64::try_from(n).unwrap_or(u64::MAX))),
    feed_source: "Source: QIS",
    feed_rooms: |n, rooms| if n > 1 { format!("Rooms: {rooms}") } else { format!("Room: {rooms}") },
    feed_lecturers: |names| format!("Lecturers: {names}"),
    feed_one_of: |label, n| format!("{label} · 1 of {n}"),
    feed_note: |note| format!("Note: {note}"),
    feed_group: |group| format!("Group: {group}"),
    feed_assumed: |first, last| format!("Period not stated in QIS; assumed: lecture period {first}–{last}"),
    feed_choose: |n, groups| format!("One of {n} {groups}; choose in Betula"),
    exercise_groups: "exercise groups",
    practical_groups: "practical groups",
    seminar_groups: "seminar groups",
    tutorial_groups: "tutorial groups",
    project_groups: "project groups",
    groups: "groups",
    feed_module: |id, title| match title {
        Some(title) => format!("Module {id} {title}"),
        None => format!("Module {id}"),
    },
    weekly: "weekly",
    fortnightly_a: "fortnightly (week A)",
    fortnightly_b: "fortnightly (week B)",
    every_four_weeks: "every four weeks",
    single_date: "single date",
    block_course: "block course",
    feed_dropped: |dates| format!("Cancelled: {dates}"),
    cancelled_by_qis: "according to QIS",
    exam: "Exam",
    submission: "Submission",
    exam_period: "Exam period",
    by_midnight: "by 24:00",
    time_open: "time to be announced",
    retake: "Retake",
    second_sitting: "2nd sitting",
    unusual_time: "unusual time",
    ends_before_start: "ends before it begins",
    outside_semester: "date outside the semester",
    qis_states: |odd| format!("According to QIS: {odd}"),
};
