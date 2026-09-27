//! Texts of the Stundenplan's exams (`pages/studyplan/exams.rs`): the sittings by date, the
//! warnings above them, what has no date. Weekdays and dates are the data contract's, and so are
//! the words of what avoids a warning (`catalog::i18n::plans`) and of a later sitting („2.
//! Termin", `catalog::i18n::timetable`).

use catalog::timetable::day::Day;
use catalog::Locale;

use super::studyplan_week::days;

pub struct Texts {
    /// Where the data could have exams and has none: „Keine Prüfungstermine im Datenstand."
    pub none: &'static str,
    /// A planned module the catalog has no title for, by its number: „Modul 12104".
    pub module_numbered: fn(&str) -> String,
    /// When a sitting is where that is no sitting with a time: something due (the time written
    /// already, „bis 24:00") …
    pub due_by: fn(&str) -> String,
    /// … a window of several days („nach Absprache") …
    pub by_arrangement: &'static str,
    /// … a day without a time („Zeit offen") …
    pub time_open: &'static str,
    /// … no date at all („Termin offen").
    pub date_open: &'static str,
    /// Where a dated sitting is, when neither a room nor a campus is known: „Ort offen".
    pub room_open: &'static str,
    /// The days of a window, the month and year said once where they are the same:
    /// „08.–19.02.2027", „25.02.–05.03.2027", „28.12.2026–08.01.2027".
    pub span: fn(Day, Day) -> String,
    /// Why a sitting is hidden where its own eye did not hide it: the kind „Prüfung" switched off
    /// („Prüfungen ausgeblendet") …
    pub exams_hidden: &'static str,
    /// … or the Standort, named by the town the exam is held in: „Standort Senftenberg".
    pub location: fn(&str) -> String,
    /// A sitting's eye (its name and tooltip): it hides the sitting („Prüfungstermin
    /// ausblenden") …
    pub hide: &'static str,
    /// … shows it again („Prüfungstermin einblenden") …
    pub show: &'static str,
    /// … shows every exam again („Prüfungen einblenden") …
    pub show_exams: &'static str,
    /// … or, where it can do nothing and nothing else says why, names what it is
    /// („Prüfungstermin").
    pub sitting: &'static str,
    /// The heading of the sittings without a date: „Ohne festen Termin".
    pub undated: &'static str,
    /// The planned modules the data has no exam for, by title (joined with „, "): „Keine Prüfung
    /// im Datenstand: …".
    pub without: fn(&str) -> String,
    /// A warning above the later sitting of a pair: two exams at once (the modules joined with
    /// „ · "): „Prüfungen gleichzeitig: Mathematik W-1 · ERP" …
    pub same_time: fn(&str) -> String,
    /// … or a hop between campuses too short: the minutes, from where, to where, then the two
    /// sittings (`ends`, `starts`, joined with „ · "): „0 min von Zentralcampus nach Senftenberg:
    /// …" …
    pub tight: fn(u16, &str, &str, &str) -> String,
    /// … the module whose sitting ends, and when: „Kraftwerkstechnik I bis 10:00" …
    pub ends: fn(&str, &str) -> String,
    /// … the module whose sitting starts, and when: „Gentechnik ab 10:00".
    pub starts: fn(&str, &str) -> String,
}

pub const DE: Texts = Texts {
    none: "Keine Prüfungstermine im Datenstand.",
    module_numbered: |id| format!("Modul {id}"),
    due_by: |time| format!("bis {time}"),
    by_arrangement: "nach Absprache",
    time_open: "Zeit offen",
    date_open: "Termin offen",
    room_open: "Ort offen",
    span: |first, last| days(first, last, true, Locale::De),
    exams_hidden: "Prüfungen ausgeblendet",
    location: |town| format!("Standort {town}"),
    hide: "Prüfungstermin ausblenden",
    show: "Prüfungstermin einblenden",
    show_exams: "Prüfungen einblenden",
    sitting: "Prüfungstermin",
    undated: "Ohne festen Termin",
    without: |titles| format!("Keine Prüfung im Datenstand: {titles}"),
    same_time: |modules| format!("Prüfungen gleichzeitig: {modules}"),
    tight: |gap, from, to, sittings| format!("{gap} min von {from} nach {to}: {sittings}"),
    ends: |module, time| format!("{module} bis {time}"),
    starts: |module, time| format!("{module} ab {time}"),
};

pub const EN: Texts = Texts {
    none: "No exam dates in the data.",
    module_numbered: |id| format!("Module {id}"),
    due_by: |time| format!("by {time}"),
    by_arrangement: "by arrangement",
    time_open: "time TBA",
    date_open: "date TBA",
    room_open: "room TBA",
    span: |first, last| days(first, last, true, Locale::En),
    exams_hidden: "Exams hidden",
    location: |town| format!("Location: {town}"),
    hide: "Hide this exam date",
    show: "Show this exam date",
    show_exams: "Show exams",
    sitting: "Exam date",
    undated: "No fixed date",
    without: |titles| format!("No exam in the data: {titles}"),
    same_time: |modules| format!("Exams at the same time: {modules}"),
    tight: |gap, from, to, sittings| format!("{gap} min from {from} to {to}: {sittings}"),
    ends: |module, time| format!("{module} until {time}"),
    starts: |module, time| format!("{module} from {time}"),
};
