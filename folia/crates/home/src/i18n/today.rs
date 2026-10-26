//! Texts of the start page's day (`home/today.rs`, concept A „Heute"), in the order of the page:
//! the greeting over the photo with the date and a sentence about the day, the semester clock under
//! it, the card of what comes next beside it, then the panels: today, the next day, the week, the
//! exams, the study, the Merkliste, the ways into the catalog. The sentences name modules by their
//! few letters („VL EvS"), as the week of the Stundenplan does, so that they keep to two lines.

pub struct Texts {
    pub good_morning: &'static str,
    pub good_day: &'static str,
    pub good_evening: &'static str,
    pub good_night: &'static str,
    /// January … December, written out: „Dienstag, 20. Oktober".
    pub months: [&'static str; 12],
    /// „Dienstag, 20. Oktober · KW 43": the weekday, the day of the month and the month, the week.
    pub long_date: fn(&str, u32, &str, u8) -> String,
    /// „Mo 21.12.": a weekday's short name and a date without its year.
    pub weekday_date: fn(&str, &str) -> String,

    // The sentence about the day.
    /// „Gerade läuft VL EvS in ZHG/HS.C, noch 20 Minuten." (kind, module, room or none, minutes).
    pub running: fn(&str, &str, Option<&str>, u16) -> String,
    /// „Heute 6 Termine, der erste um 07:30 in ZHG/HS.B."
    pub first_today: fn(usize, &str, Option<&str>) -> String,
    /// „Noch 3 Termine, als Nächstes um 13:45 in LG1A/HS.2."
    pub still_today: fn(usize, &str, Option<&str>) -> String,
    /// „Heute ist geschafft." and what follows it: „Morgen 4 Termine, ab 07:30 in ZHG/HS.B."
    pub day_done: &'static str,
    pub then_day: fn(&str, usize, &str, Option<&str>) -> String,
    /// „Heute keine Termine.", „Wochenende."
    pub nothing_today: &'static str,
    pub weekend: &'static str,
    /// „Vorlesungsfrei bis 03.01. Weiter am Mo 04.01. um 13:45."
    pub pause_until: fn(&str) -> String,
    pub back_on: fn(&str, &str) -> String,
    /// „Prüfungszeit. In 3 Tagen: EEG, Do 11.02. um 11:00." (when, module, day, time or none)
    pub exam_time: &'static str,
    pub next_exam: fn(&str, &str, &str, Option<&str>) -> String,
    pub exams_written: &'static str,
    pub no_exams: &'static str,
    /// „Die Vorlesungen beginnen am Mo 05.10."
    pub lectures_begin: fn(&str) -> String,
    /// After the semester: what is left to do.
    pub holidays: &'static str,
    /// Nothing planned in the current semester: „Dein Stundenplan für das WiSe 2026/27 ist noch leer."
    pub empty_plan: fn(&str) -> String,
    /// Without a study set up.
    pub guest: &'static str,
    /// „heute", „morgen", „in 3 Tagen" (sentence case where it begins one: `days_upper`).
    pub days: fn(i32) -> String,
    pub days_upper: fn(i32) -> String,
    /// „Morgen", „Am Donnerstag": the next day with Termine.
    pub tomorrow: &'static str,
    pub on_weekday: fn(&str) -> String,

    // The semester clock.
    /// „Woche 3 von 15", „ · A-Woche".
    pub week_of: fn(usize, usize) -> String,
    pub ab_week: fn(&str) -> String,
    /// „Prüfungen 11.02.–12.03.", „Prüfung am 11.02.", „Vorlesungen bis 31.01."
    pub exams_span: fn(&str, &str) -> String,
    pub exam_on: fn(&str) -> String,
    pub lectures_until: fn(&str) -> String,
    /// „Weihnachtspause bis 03.01.", „Vorlesungsfrei bis 03.01."
    pub christmas_until: fn(&str) -> String,
    pub free_until: fn(&str) -> String,
    /// „11 von 15 Wochen geschafft".
    pub weeks_done: fn(usize, usize) -> String,
    pub exam_period: &'static str,
    /// „noch 3 Prüfungen bis 12.03.", „alle geschrieben", „Semester bis 31.03."
    pub exams_left: fn(usize, &str) -> String,
    pub all_written: &'static str,
    pub semester_until: fn(&str) -> String,
    /// „Vorlesungen ab Mo 05.10.", „15 Wochen Vorlesung".
    pub lectures_from: fn(&str) -> String,
    pub lecture_weeks: fn(usize) -> String,
    /// „WiSe 2026/27 geschafft", „SoSe 2027 ab 01.04."
    pub semester_done: fn(&str) -> String,
    pub next_from: fn(&str, &str) -> String,
    /// What the clock says, for those who cannot see it: „Semester: Woche 3 von 15, Prüfungen …".
    pub clock_label: fn(&str, &str) -> String,
    /// A piece's tooltip: „KW 43 · 19.10.–25.10.", and what it is besides.
    pub piece: fn(u8, &str, &str) -> String,
    pub piece_pause: &'static str,
    pub piece_exams: &'static str,
    pub piece_exam: &'static str,

    // The card beside the greeting.
    pub now: &'static str,
    pub next: &'static str,
    pub next_exam_head: &'static str,
    pub your_study: &'static str,
    /// „noch 20 min", „in 25 min", „in 1 h 05 min".
    pub minutes_left: fn(u16) -> String,
    pub in_minutes: fn(u16) -> String,
    /// „bis 13:00".
    pub until: fn(&str) -> String,
    /// „Danach 13:45 · VL MIT1 · LG1A/HS.2".
    pub after: fn(&str, &str, Option<&str>) -> String,
    /// „zugleich Ü MIT1".
    pub at_once: fn(&str) -> String,
    /// „4 Termine, bis 17:00".
    pub termine_until: fn(usize, &str) -> String,
    /// „Tag", „Tage" after the number of days to an exam.
    pub day_word: fn(i32) -> &'static str,
    pub today_word: &'static str,
    /// „von 180 LP".
    pub of_credits: fn(&str) -> String,
    pub nothing_passed: &'static str,
    /// „Nach dem 1. FS".
    pub after_fs: fn(u8) -> String,
    pub plan_empty_card: &'static str,
    pub not_set_up: &'static str,
    pub set_up_study: &'static str,
    pub module: &'static str,
    /// The card's way to its Termin in its own view: „Termin ansehen".
    pub termin: &'static str,
    pub timetable: &'static str,
    pub all_exams: &'static str,
    pub enter_results: &'static str,
    pub take_semester: &'static str,

    // The panels.
    pub today: &'static str,
    /// „Di 20.10. · 6 Termine", „ · 2 Gruppen offen".
    pub count_termine: fn(usize) -> String,
    pub open_groups: fn(usize) -> String,
    pub past: &'static str,
    /// „läuft · noch 20 min".
    pub running_chip: fn(u16) -> String,
    pub cancelled: &'static str,
    pub group_open: &'static str,
    pub choose_group: &'static str,
    /// „zugleich" before what overlaps.
    pub overlaps: &'static str,
    pub no_lectures: &'static str,
    pub pause_text: fn(&str, &str) -> String,
    pub exams_text: fn(&str) -> String,
    pub holidays_head: &'static str,
    pub holidays_text: fn(&str) -> String,
    pub weekend_text: &'static str,
    pub nothing_planned_text: &'static str,
    pub before_text: fn(&str) -> String,
    /// „Ab Montag" over the next day's Termine after a break.
    pub from_weekday: fn(&str) -> String,
    pub this_week: &'static str,
    /// „Woche ab 04.01.": the week shown in a break, the one the lectures go on in.
    pub week_from: fn(&str) -> String,
    /// „KW 43 · A-Woche".
    pub week_sub: fn(u8, Option<&str>) -> String,
    pub week_link: &'static str,
    pub exams: &'static str,
    /// An exam among the Termine of a day: „Prüfung · Mathematik IT-1".
    pub exam: &'static str,
    pub your_exams: &'static str,
    /// „3 im WiSe 2026/27".
    pub exams_in: fn(usize, &str) -> String,
    pub all: &'static str,
    pub my_study: &'static str,
    /// „Informatik B.Sc. · 3. FS".
    pub study_sub: fn(&str, Option<u8>) -> String,
    pub open: &'static str,
    /// „0 von 180 LP bestanden".
    pub passed_of: fn(&str) -> String,
    pub passed: &'static str,
    pub planned: &'static str,
    pub left: &'static str,
    pub credits: &'static str,
    pub bookmarks: &'static str,
    /// „3 Module".
    pub modules: fn(usize) -> String,
    pub no_bookmarks: &'static str,
    pub discover: &'static str,
    pub discover_sub: &'static str,
    pub all_programs: &'static str,
    /// „6 Fakultäten".
    pub faculties: fn(usize) -> String,
    pub what_is_betula: &'static str,
    pub without_setup: &'static str,
    pub without_setup_sub: &'static str,

    // After „Studium anlegen".
    /// „Eingerichtet: Informatik B.Sc. · 1. FS seit WiSe 2026/27".
    pub set_up: fn(&str, u8, &str) -> String,
    /// „5 Module im Stundenplan · 28 LP aus früheren Semestern als bestanden markiert".
    pub set_up_detail: fn(usize, usize, Option<&str>) -> String,
    pub stays_here: &'static str,
    pub undo: &'static str,
    pub to_timetable: &'static str,
    pub close: &'static str,

    /// The photo's credit in the ground: „Foto: John Price, Unsplash".
    pub photo_by: &'static str,
    /// The page, for screen readers: „Dein Tag".
    pub your_day: &'static str,
}

const MONTHS_DE: [&str; 12] = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"];
const MONTHS_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// A sentence ended once: a date such as „03.01." ends it with its own point.
fn stop(sentence: String) -> String {
    if sentence.ends_with('.') { sentence } else { sentence + "." }
}

/// „ in ZHG/HS.C" where a room is known.
fn in_room_de(room: Option<&str>) -> String {
    room.map(|room| format!(" in {room}")).unwrap_or_default()
}

fn in_room_en(room: Option<&str>) -> String {
    room.map(|room| format!(" in {room}")).unwrap_or_default()
}

/// „1 h 05 min", „25 min".
fn duration(minutes: u16) -> String {
    match minutes {
        0..=59 => format!("{minutes}\u{a0}min"),
        _ => format!("{}\u{a0}h {:02}\u{a0}min", minutes / 60, minutes % 60),
    }
}

pub const DE: Texts = Texts {
    good_morning: "Guten Morgen",
    good_day: "Guten Tag",
    good_evening: "Guten Abend",
    good_night: "Gute Nacht",
    months: MONTHS_DE,
    long_date: |weekday, day, month, week| format!("{weekday}, {day}. {month} · KW {week}"),
    weekday_date: |weekday, date| format!("{weekday} {date}"),

    running: |kind, module, room, minutes| format!("Gerade läuft {kind} {module}{}, noch {minutes}\u{a0}{}.", in_room_de(room), if minutes == 1 { "Minute" } else { "Minuten" }),
    first_today: |n, time, room| match n {
        1 => format!("Heute ein Termin, um {time}{}.", in_room_de(room)),
        n => format!("Heute {n} Termine, der erste um {time}{}.", in_room_de(room)),
    },
    still_today: |n, time, room| match n {
        1 => format!("Noch ein Termin, um {time}{}.", in_room_de(room)),
        n => format!("Noch {n} Termine, als Nächstes um {time}{}.", in_room_de(room)),
    },
    day_done: "Heute ist geschafft.",
    then_day: |when, n, time, room| match n {
        1 => format!("{when} ein Termin, um {time}{}.", in_room_de(room)),
        n => format!("{when} {n} Termine, ab {time}{}.", in_room_de(room)),
    },
    nothing_today: "Heute keine Termine.",
    weekend: "Wochenende.",
    pause_until: |date| stop(format!("Vorlesungsfrei bis {date}")),
    back_on: |day, time| format!("Weiter am {day} um {time}."),
    exam_time: "Prüfungszeit.",
    next_exam: |when, module, day, time| match time {
        Some(time) => format!("{when}: {module}, {day} um {time}."),
        None => stop(format!("{when}: {module}, {day}")),
    },
    exams_written: "Alle Prüfungen im Stundenplan sind geschrieben.",
    no_exams: "Im Stundenplan steht keine Prüfung.",
    lectures_begin: |day| stop(format!("Die Vorlesungen beginnen am {day}")),
    holidays: "Semesterferien: Prüfungen eintragen, das nächste Semester planen.",
    empty_plan: |semester| format!("Dein Stundenplan für das {semester} ist noch leer."),
    guest: "Richte dein Studium ein, dann steht hier dein Tag: was gerade läuft, was als Nächstes kommt, wann deine Prüfungen sind.",
    days: |n| match n {
        0 => "heute".to_string(),
        1 => "morgen".to_string(),
        n => format!("in {n} Tagen"),
    },
    days_upper: |n| match n {
        0 => "Heute".to_string(),
        1 => "Morgen".to_string(),
        n => format!("In {n} Tagen"),
    },
    tomorrow: "Morgen",
    on_weekday: |weekday| format!("Am {weekday}"),

    week_of: |n, total| format!("Woche {n} von {total}"),
    ab_week: |week| format!("{week}-Woche"),
    exams_span: |first, last| format!("Prüfungen {first}–{last}"),
    exam_on: |day| format!("Prüfung am {day}"),
    lectures_until: |day| format!("Vorlesungen bis {day}"),
    christmas_until: |day| format!("Weihnachtspause bis {day}"),
    free_until: |day| format!("Vorlesungsfrei bis {day}"),
    weeks_done: |n, total| format!("{n} von {total} Wochen geschafft"),
    exam_period: "Prüfungszeit",
    exams_left: |n, day| match n {
        1 => format!("noch eine Prüfung, am {day}"),
        n => format!("noch {n} Prüfungen bis {day}"),
    },
    all_written: "alle geschrieben",
    semester_until: |day| format!("Semester bis {day}"),
    lectures_from: |day| format!("Vorlesungen ab {day}"),
    lecture_weeks: |n| format!("{n} Wochen Vorlesung"),
    semester_done: |semester| format!("{semester} geschafft"),
    next_from: |semester, day| format!("{semester} ab {day}"),
    clock_label: |left, right| format!("Semester: {left}, {right}"),
    piece: |week, from, to| format!("KW {week} · {from}–{to}"),
    piece_pause: "vorlesungsfrei",
    piece_exams: "Prüfungszeit",
    piece_exam: "mit Prüfung",

    now: "Jetzt",
    next: "Als Nächstes",
    next_exam_head: "Nächste Prüfung",
    your_study: "Dein Studium",
    minutes_left: |minutes| format!("noch {}", duration(minutes)),
    in_minutes: |minutes| format!("in {}", duration(minutes)),
    until: |time| format!("bis {time}"),
    after: |time, what, room| match room {
        Some(room) => format!("Danach {time} · {what} · {room}"),
        None => format!("Danach {time} · {what}"),
    },
    at_once: |what| format!("zugleich {what}"),
    termine_until: |n, time| match n {
        1 => format!("ein Termin, bis {time}"),
        n => format!("{n} Termine, bis {time}"),
    },
    day_word: |n| if n == 1 { "Tag" } else { "Tage" },
    today_word: "heute",
    of_credits: |credits| format!("von {credits}\u{a0}LP"),
    nothing_passed: "Noch nichts als bestanden eingetragen.",
    after_fs: |fs| format!("nach dem {fs}.\u{a0}FS"),
    plan_empty_card: "Der Stundenplan ist leer: das Fachsemester aus dem Regelstudienplan übernehmen.",
    not_set_up: "Noch nicht eingerichtet: Studiengang und Beginn, und der Tag steht hier.",
    set_up_study: "Studium einrichten",
    module: "Modul ansehen",
    termin: "Termin ansehen",
    timetable: "Stundenplan",
    all_exams: "Alle Prüfungen",
    enter_results: "Prüfungen eintragen",
    take_semester: "Fachsemester übernehmen",

    today: "Heute",
    count_termine: |n| match n {
        0 => "keine Termine".to_string(),
        1 => "1 Termin".to_string(),
        n => format!("{n} Termine"),
    },
    open_groups: |n| match n {
        1 => "1 Gruppe offen".to_string(),
        n => format!("{n} Gruppen offen"),
    },
    past: "vorbei",
    running_chip: |minutes| format!("läuft · noch {}", duration(minutes)),
    cancelled: "fällt aus",
    group_open: "Gruppe offen",
    choose_group: "wählen",
    overlaps: "zugleich",
    no_lectures: "Heute keine Vorlesungen",
    pause_text: |from, until| format!("{} Danach geht es mit der Woche weiter, wie sie war.", stop(format!("Vorlesungsfrei vom {from} bis {until}"))),
    exams_text: |end| format!("{} Bis zum Semesterende stehen deine Prüfungen an.", stop(format!("Die Vorlesungszeit endete am {end}"))),
    holidays_head: "Semesterferien",
    holidays_text: |semester| format!("Die Termine des {semester} zeigt Betula, sobald die BTU sie in QIS stellt."),
    weekend_text: "Am Wochenende hat dein Stundenplan nichts.",
    nothing_planned_text: "Heute steht nichts in deinem Stundenplan.",
    before_text: |day| stop(format!("Die Vorlesungen beginnen am {day}")),
    from_weekday: |weekday| format!("Ab {weekday}"),
    this_week: "Diese Woche",
    week_from: |monday| format!("Woche ab {monday}"),
    week_sub: |week, ab| match ab {
        Some(ab) => format!("KW {week} · {ab}"),
        None => format!("KW {week}"),
    },
    week_link: "Woche",
    exams: "Prüfungen",
    exam: "Prüfung",
    your_exams: "Deine Prüfungen",
    exams_in: |n, semester| format!("{n} im {semester}"),
    all: "Alle",
    my_study: "Mein Studium",
    study_sub: |program, fs| match fs {
        Some(fs) => format!("{program} · {fs}.\u{a0}FS"),
        None => program.to_string(),
    },
    open: "Öffnen",
    passed_of: |credits| format!("von {credits}\u{a0}LP bestanden"),
    passed: "bestanden",
    planned: "geplant",
    left: "offen",
    credits: "LP",
    bookmarks: "Merkliste",
    modules: |n| if n == 1 { "1 Modul".to_string() } else { format!("{n} Module") },
    no_bookmarks: "Noch nichts gemerkt: Das Lesezeichen an einem Modul setzt es hierher.",
    discover: "Entdecken",
    discover_sub: "Einstiege in den Katalog",
    all_programs: "Alle Studiengänge",
    faculties: |n| format!("{n} Fakultäten"),
    what_is_betula: "Was ist Betula?",
    without_setup: "Ohne Einrichtung",
    without_setup_sub: "Suchen und stöbern geht sofort",

    set_up: |program, fs, start| format!("Eingerichtet: {program} · {fs}.\u{a0}FS seit {start}"),
    set_up_detail: |modules, placeholders, passed| {
        let planned = match (modules, placeholders) {
            (0, 0) => "Stundenplan ohne Module".to_string(),
            (m, 0) => format!("{m} {} im Stundenplan", if m == 1 { "Modul" } else { "Module" }),
            (0, p) => format!("{p} Platzhalter im Stundenplan"),
            (m, p) => format!("{m} {} und {p} Platzhalter im Stundenplan", if m == 1 { "Modul" } else { "Module" }),
        };
        match passed {
            Some(credits) => format!("{planned} · {credits}\u{a0}LP aus früheren Semestern als bestanden markiert."),
            None => format!("{planned}."),
        }
    },
    stays_here: "Alles bleibt in diesem Browser.",
    undo: "Rückgängig",
    to_timetable: "Zum Stundenplan",
    close: "Schließen",

    photo_by: "Foto",
    your_day: "Dein Tag",
};

pub const EN: Texts = Texts {
    good_morning: "Good morning",
    good_day: "Good afternoon",
    good_evening: "Good evening",
    good_night: "Good night",
    months: MONTHS_EN,
    long_date: |weekday, day, month, week| format!("{weekday}, {day} {month} · Week {week}"),
    weekday_date: |weekday, date| format!("{weekday} {date}"),

    running: |kind, module, room, minutes| format!("{kind} {module} is on{}, {minutes} more {}.", in_room_en(room), if minutes == 1 { "minute" } else { "minutes" }),
    first_today: |n, time, room| match n {
        1 => format!("One date today, at {time}{}.", in_room_en(room)),
        n => format!("{n} dates today, the first at {time}{}.", in_room_en(room)),
    },
    still_today: |n, time, room| match n {
        1 => format!("One more date, at {time}{}.", in_room_en(room)),
        n => format!("{n} more dates, the next at {time}{}.", in_room_en(room)),
    },
    day_done: "Today is done.",
    then_day: |when, n, time, room| match n {
        1 => format!("{when} one date, at {time}{}.", in_room_en(room)),
        n => format!("{when} {n} dates, from {time}{}.", in_room_en(room)),
    },
    nothing_today: "Nothing on today.",
    weekend: "Weekend.",
    pause_until: |date| stop(format!("No lectures until {date}")),
    back_on: |day, time| format!("Back on {day} at {time}."),
    exam_time: "Exam period.",
    next_exam: |when, module, day, time| match time {
        Some(time) => format!("{when}: {module}, {day} at {time}."),
        None => stop(format!("{when}: {module}, {day}")),
    },
    exams_written: "Every exam in your timetable is done.",
    no_exams: "Your timetable has no exam.",
    lectures_begin: |day| stop(format!("Lectures begin on {day}")),
    holidays: "Semester break: enter your results, plan the next semester.",
    empty_plan: |semester| format!("Your timetable for {semester} is still empty."),
    guest: "Set up your studies, and your day stands here: what is on, what comes next, when your exams are.",
    days: |n| match n {
        0 => "today".to_string(),
        1 => "tomorrow".to_string(),
        n => format!("in {n} days"),
    },
    days_upper: |n| match n {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        n => format!("In {n} days"),
    },
    tomorrow: "Tomorrow",
    on_weekday: |weekday| format!("On {weekday}"),

    week_of: |n, total| format!("Week {n} of {total}"),
    ab_week: |week| format!("{week} week"),
    exams_span: |first, last| format!("Exams {first}–{last}"),
    exam_on: |day| format!("Exam on {day}"),
    lectures_until: |day| format!("Lectures until {day}"),
    christmas_until: |day| format!("Christmas break until {day}"),
    free_until: |day| format!("No lectures until {day}"),
    weeks_done: |n, total| format!("{n} of {total} weeks done"),
    exam_period: "Exam period",
    exams_left: |n, day| match n {
        1 => format!("one exam left, on {day}"),
        n => format!("{n} exams left, until {day}"),
    },
    all_written: "all done",
    semester_until: |day| format!("semester until {day}"),
    lectures_from: |day| format!("Lectures from {day}"),
    lecture_weeks: |n| format!("{n} weeks of lectures"),
    semester_done: |semester| format!("{semester} done"),
    next_from: |semester, day| format!("{semester} from {day}"),
    clock_label: |left, right| format!("Semester: {left}, {right}"),
    piece: |week, from, to| format!("Week {week} · {from}–{to}"),
    piece_pause: "no lectures",
    piece_exams: "exam period",
    piece_exam: "with an exam",

    now: "Now",
    next: "Next",
    next_exam_head: "Next exam",
    your_study: "Your studies",
    minutes_left: |minutes| format!("{} left", duration(minutes)),
    in_minutes: |minutes| format!("in {}", duration(minutes)),
    until: |time| format!("until {time}"),
    after: |time, what, room| match room {
        Some(room) => format!("Then {time} · {what} · {room}"),
        None => format!("Then {time} · {what}"),
    },
    at_once: |what| format!("at the same time as {what}"),
    termine_until: |n, time| match n {
        1 => format!("one date, until {time}"),
        n => format!("{n} dates, until {time}"),
    },
    day_word: |n| if n == 1 { "day" } else { "days" },
    today_word: "today",
    of_credits: |credits| format!("of {credits}\u{a0}ECTS"),
    nothing_passed: "Nothing entered as passed yet.",
    after_fs: |fs| format!("after semester {fs}"),
    plan_empty_card: "Your timetable is empty: take over the semester of the standard study plan.",
    not_set_up: "Not set up yet: your programme and when you began, and your day stands here.",
    set_up_study: "Set up your studies",
    module: "View module",
    termin: "View session",
    timetable: "Timetable",
    all_exams: "All exams",
    enter_results: "Enter your results",
    take_semester: "Take over a semester",

    today: "Today",
    count_termine: |n| match n {
        0 => "no dates".to_string(),
        1 => "1 date".to_string(),
        n => format!("{n} dates"),
    },
    open_groups: |n| match n {
        1 => "1 group to choose".to_string(),
        n => format!("{n} groups to choose"),
    },
    past: "over",
    running_chip: |minutes| format!("on · {} left", duration(minutes)),
    cancelled: "cancelled",
    group_open: "group to choose",
    choose_group: "choose",
    overlaps: "at the same time",
    no_lectures: "No lectures today",
    pause_text: |from, until| format!("{} Then the week goes on as it was.", stop(format!("No lectures from {from} to {until}"))),
    exams_text: |end| format!("{} Your exams come until the end of the semester.", stop(format!("The lecture period ended on {end}"))),
    holidays_head: "Semester break",
    holidays_text: |semester| format!("Betula shows the dates of {semester} as soon as BTU puts them into QIS."),
    weekend_text: "Your timetable has nothing at the weekend.",
    nothing_planned_text: "Nothing in your timetable today.",
    before_text: |day| stop(format!("Lectures begin on {day}")),
    from_weekday: |weekday| format!("From {weekday}"),
    this_week: "This week",
    week_from: |monday| format!("Week of {monday}"),
    week_sub: |week, ab| match ab {
        Some(ab) => format!("Week {week} · {ab}"),
        None => format!("Week {week}"),
    },
    week_link: "Week",
    exams: "Exams",
    exam: "Exam",
    your_exams: "Your exams",
    exams_in: |n, semester| format!("{n} in {semester}"),
    all: "All",
    my_study: "My studies",
    study_sub: |program, fs| match fs {
        Some(fs) => format!("{program} · semester {fs}"),
        None => program.to_string(),
    },
    open: "Open",
    passed_of: |credits| format!("of {credits}\u{a0}ECTS passed"),
    passed: "passed",
    planned: "planned",
    left: "open",
    credits: "ECTS",
    bookmarks: "Saved",
    modules: |n| if n == 1 { "1 module".to_string() } else { format!("{n} modules") },
    no_bookmarks: "Nothing saved yet: the bookmark on a module puts it here.",
    discover: "Discover",
    discover_sub: "Ways into the catalogue",
    all_programs: "All degree programmes",
    faculties: |n| format!("{n} faculties"),
    what_is_betula: "What is Betula?",
    without_setup: "Without setting up",
    without_setup_sub: "Searching and browsing work right away",

    set_up: |program, fs, start| format!("Set up: {program} · semester {fs} since {start}"),
    set_up_detail: |modules, placeholders, passed| {
        let planned = match (modules, placeholders) {
            (0, 0) => "Timetable without modules".to_string(),
            (m, 0) => format!("{m} {} in your timetable", if m == 1 { "module" } else { "modules" }),
            (0, p) => format!("{p} {} in your timetable", if p == 1 { "placeholder" } else { "placeholders" }),
            (m, p) => format!("{m} {} and {p} {} in your timetable", if m == 1 { "module" } else { "modules" }, if p == 1 { "placeholder" } else { "placeholders" }),
        };
        match passed {
            Some(credits) => format!("{planned} · {credits}\u{a0}ECTS of earlier semesters marked as passed."),
            None => format!("{planned}."),
        }
    },
    stays_here: "Everything stays in this browser.",
    undo: "Undo",
    to_timetable: "To the timetable",
    close: "Close",

    photo_by: "Photo",
    your_day: "Your day",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sentences_read_well() {
        assert_eq!((DE.running)("VL", "EvS", Some("ZHG/HS.C"), 20), "Gerade läuft VL EvS in ZHG/HS.C, noch 20\u{a0}Minuten.");
        assert_eq!((DE.first_today)(6, "07:30", Some("ZHG/HS.B")), "Heute 6 Termine, der erste um 07:30 in ZHG/HS.B.");
        assert_eq!((DE.then_day)("Morgen", 4, "07:30", None), "Morgen 4 Termine, ab 07:30.");
        assert_eq!((DE.in_minutes)(65), "in 1\u{a0}h 05\u{a0}min");
        assert_eq!((DE.long_date)("Dienstag", 20, "Oktober", 43), "Dienstag, 20. Oktober · KW 43");
        assert_eq!((EN.long_date)("Tuesday", 20, "October", 43), "Tuesday, 20 October · Week 43");
        assert_eq!((DE.set_up_detail)(5, 1, Some("28")), "5 Module und 1 Platzhalter im Stundenplan · 28\u{a0}LP aus früheren Semestern als bestanden markiert.");
        assert_eq!((DE.pause_until)("03.01."), "Vorlesungsfrei bis 03.01.", "a date ends the sentence with its own point");
        assert_eq!((DE.exams_text)("29.01."), "Die Vorlesungszeit endete am 29.01. Bis zum Semesterende stehen deine Prüfungen an.");
        assert_eq!((EN.pause_until)("3 January"), "No lectures until 3 January.");
    }
}
