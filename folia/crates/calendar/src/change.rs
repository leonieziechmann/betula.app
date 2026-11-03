//! A change of a Termin of QIS and a note at a Termin (folia/design/kalender/termine.html, its
//! second step; owner, 2026-11-03: „Passt, bau Schritt 2 jetzt in Folia"): what a student knows
//! better than QIS — the lecture in another room, on another day, an hour later — for one date,
//! from a date on or for all of them (question 4: „Beginn, Ende, Tag und Raum, für diesen Tag, ab
//! ihm oder alle; dazu eine Notiz. Titel und Art bleiben die des Moduls."), and a note only the
//! student sees.
//!
//! A change names its Termin by its key (`RowKey`) and keeps it: the Termin changed is the same
//! Termin, so its group stays ticked, its dates stay hidden and its calendar entry keeps its UID.
//! It keeps what QIS said when it was made (`Values`), so Betula sees whether QIS has moved on
//! since (question 5): QIS says what it said, and the change holds; it says what the student said,
//! and the change is done; or it says something else, and the change holds until the student
//! decides. QIS's weekday and start are part of the key: a Termin QIS moves to another day or start
//! is a new one for Betula, and the change hangs on the old one.

use serde::{Deserialize, Serialize};

use crate::day::Day;
use crate::rowkey::RowKey;

/// The most characters of a room a change names.
pub const MAX_ROOM: usize = 80;

/// The most characters of a note.
pub const MAX_NOTE: usize = 500;

/// The most characters of the id of the module whose Termin a change is (`url::is_module_id`
/// allows fewer).
const MAX_MODULE_ID: usize = 40;

/// For which of a Termin's dates a change holds (the form's „Für": „Nur am 04.11.", „Ab 04.11.",
/// „Alle 15").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Scope {
    /// Every date.
    All,
    /// The date and every one after it.
    From(Day),
    /// That date alone.
    On(Day),
}

impl Scope {
    /// The word its line keeps.
    pub fn code(self) -> &'static str {
        match self {
            Scope::All => "all",
            Scope::From(_) => "from",
            Scope::On(_) => "on",
        }
    }

    /// The scope of its word and, for one from a day or on a day, that day.
    pub fn of(code: &str, day: Option<Day>) -> Option<Scope> {
        match (code, day) {
            ("all", _) => Some(Scope::All),
            ("from", Some(day)) => Some(Scope::From(day)),
            ("on", Some(day)) => Some(Scope::On(day)),
            _ => None,
        }
    }

    /// The day it begins on or is held on; none for every date.
    pub fn day(self) -> Option<Day> {
        match self {
            Scope::All => None,
            Scope::From(day) | Scope::On(day) => Some(day),
        }
    }

    /// Whether it holds on `day`, a date of the Termin as QIS has it.
    pub fn holds(self, day: Day) -> bool {
        match self {
            Scope::All => true,
            Scope::From(from) => day >= from,
            Scope::On(on) => day == on,
        }
    }
}

/// A Termin's weekday (1 = Monday), time (minutes, `from < to`) and room as QIS says them; the
/// room is empty where QIS names none.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Values {
    pub weekday: u8,
    pub from: u16,
    pub to: u16,
    pub room: String,
}

impl Values {
    fn is_valid(&self) -> bool {
        (1..=7).contains(&self.weekday) && self.from < self.to && self.to <= 24 * 60 && self.room.chars().count() <= MAX_ROOM
    }
}

/// A change of a Termin of QIS: of its dates `scope` holds for, what it says instead of QIS (each
/// `None` keeps QIS's), what QIS said when it was made, and the module whose Termin it is (to name
/// it where QIS no longer has it).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Change {
    pub key: RowKey,
    pub scope: Scope,
    pub weekday: Option<u8>,
    /// Begin and end, in minutes.
    pub time: Option<(u16, u16)>,
    pub room: Option<String>,
    pub was: Values,
    pub module: String,
}

impl Change {
    /// What can be a change: a Termin of QIS (no key of one's own), a weekday, a time and a room
    /// that can be, at least one of them, what QIS said that can be, and a module's id.
    pub fn is_valid(&self) -> bool {
        let weekday = self.weekday.is_none_or(|day| (1..=7).contains(&day));
        let time = self.time.is_none_or(|(from, to)| from < to && to <= 24 * 60);
        let room = self.room.as_ref().is_none_or(|room| !room.trim().is_empty() && room.chars().count() <= MAX_ROOM && room.trim() == room);
        let something = self.weekday.is_some() || self.time.is_some() || self.room.is_some();
        let module = !self.module.is_empty() && self.module.len() <= MAX_MODULE_ID && self.module.bytes().all(|byte| byte.is_ascii_graphic());
        self.key.event > 0 && !crate::own::is_own(self.key) && weekday && time && room && something && self.was.is_valid() && module
    }

    /// What a date of the Termin says with the change, QIS saying `qis`.
    pub fn applied(&self, qis: &Values) -> Values {
        let (from, to) = self.time.unwrap_or((qis.from, qis.to));
        Values { weekday: self.weekday.unwrap_or(qis.weekday), from, to, room: self.room.clone().unwrap_or_else(|| qis.room.clone()) }
    }

    /// What it says of the fields it changes, of which QIS says `qis` now: the same as QIS
    /// (`Agrees`: QIS has taken it over), what QIS said when it was made (`Holds`), or of one of
    /// them something else (`Differs`: the student decides). QIS's weekday and start are part of
    /// the key, so of an existing Termin they are what they were.
    pub fn against(&self, qis: &Values) -> Against {
        let mine = self.applied(qis);
        let fields = [
            (self.weekday.is_some(), qis.weekday == mine.weekday, qis.weekday == self.was.weekday),
            (self.time.is_some(), qis.from == mine.from, qis.from == self.was.from),
            (self.time.is_some(), qis.to == mine.to, qis.to == self.was.to),
            (self.room.is_some(), qis.room == mine.room, qis.room == self.was.room),
        ];
        let changed: Vec<(bool, bool)> = fields.iter().filter(|(changed, ..)| *changed).map(|(_, agrees, holds)| (*agrees, *holds)).collect();
        if changed.iter().all(|(agrees, _)| *agrees) {
            Against::Agrees
        } else if changed.iter().all(|(agrees, holds)| *agrees || *holds) {
            Against::Holds
        } else {
            Against::Differs
        }
    }

    /// The day a date `day` of the Termin moves to: by as many days as its weekday changes, within
    /// its week.
    pub fn moved(&self, day: Day, qis_weekday: u8) -> Day {
        match self.weekday {
            Some(weekday) => day.plus(i32::from(weekday) - i32::from(qis_weekday)),
            None => day,
        }
    }
}

/// How a change stands against what QIS says now (`Change::against`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Against {
    /// QIS says what it said when the change was made: the change holds.
    Holds,
    /// QIS says what the change says: it is done.
    Agrees,
    /// QIS says something else of a field the change changes: it holds until the student
    /// decides („QIS übernehmen", „Meine behalten").
    Differs,
}

/// A note as a line keeps it: on one line, without control characters, trimmed, at most
/// `MAX_NOTE` characters.
pub fn note_text(text: &str) -> String {
    let flat: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    flat.trim().chars().take(MAX_NOTE).collect::<String>().trim_end().to_string()
}

/// A room as a change keeps it: on one line, trimmed, at most `MAX_ROOM` characters; `None` for
/// none.
pub fn room_text(text: &str) -> Option<String> {
    let flat: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let room: String = flat.trim().chars().take(MAX_ROOM).collect::<String>().trim_end().to_string();
    (!room.is_empty()).then_some(room)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn evs() -> Change {
        Change {
            key: RowKey { event: 148_701, fp: 0xb7025 },
            scope: Scope::All,
            weekday: None,
            time: None,
            room: Some("ZHG/HS.A".into()),
            was: Values { weekday: 5, from: 555, to: 645, room: "ZHG/HS.C".into() },
            module: "12104".into(),
        }
    }

    #[test]
    fn a_change_holds_for_its_dates() {
        assert!(Scope::All.holds(d("2026-10-09")));
        assert!(Scope::From(d("2026-11-06")).holds(d("2026-11-06")) && Scope::From(d("2026-11-06")).holds(d("2026-12-04")) && !Scope::From(d("2026-11-06")).holds(d("2026-10-30")));
        assert!(Scope::On(d("2026-11-06")).holds(d("2026-11-06")) && !Scope::On(d("2026-11-06")).holds(d("2026-11-13")));
        for scope in [Scope::All, Scope::From(d("2026-11-06")), Scope::On(d("2026-11-06"))] {
            assert_eq!(Scope::of(scope.code(), scope.day()), Some(scope));
        }
        assert_eq!(Scope::of("from", None), None);
        assert_eq!(Scope::of("ab", Some(d("2026-11-06"))), None);
    }

    #[test]
    fn a_change_is_one_of_a_termin_of_qis_that_changes_something() {
        assert!(evs().is_valid());
        assert!(!Change { room: None, ..evs() }.is_valid(), "nothing changed");
        assert!(!Change { room: Some(" ".into()), ..evs() }.is_valid());
        assert!(!Change { room: Some("x".repeat(MAX_ROOM + 1)), ..evs() }.is_valid());
        assert!(!Change { weekday: Some(8), ..evs() }.is_valid());
        assert!(!Change { time: Some((600, 600)), ..evs() }.is_valid());
        assert!(Change { time: Some((600, 1440)), ..evs() }.is_valid(), "up to midnight");
        assert!(!Change { key: RowKey { event: crate::own::OWN_EVENT_BASE + 1, fp: 0 }, ..evs() }.is_valid(), "a Termin of one's own changes itself");
        assert!(!Change { module: String::new(), ..evs() }.is_valid());
        assert!(!Change { was: Values { weekday: 0, ..evs().was }, ..evs() }.is_valid());
    }

    #[test]
    fn a_change_says_what_qis_says_now() {
        let change = evs();
        let qis = change.was.clone();
        assert_eq!(change.applied(&qis), Values { room: "ZHG/HS.A".into(), ..qis.clone() });
        assert_eq!(change.against(&qis), Against::Holds);
        assert_eq!(change.against(&Values { room: "ZHG/HS.A".into(), ..qis.clone() }), Against::Agrees, "QIS took it over");
        assert_eq!(change.against(&Values { room: "ZHG/HS.B".into(), ..qis.clone() }), Against::Differs);
        // A field the change leaves is QIS's, whatever QIS says of it.
        assert_eq!(change.against(&Values { to: 660, ..qis.clone() }), Against::Holds);
        let later = Change { time: Some((600, 690)), room: None, ..evs() };
        assert_eq!(later.against(&Values { to: 700, ..qis.clone() }), Against::Differs, "QIS ends at another time now");
        assert_eq!(later.against(&qis), Against::Holds);
    }

    #[test]
    fn a_date_moves_within_its_week() {
        let moved = Change { weekday: Some(4), ..evs() };
        assert_eq!(moved.moved(d("2026-11-06"), 5), d("2026-11-05"), "Friday to Thursday");
        assert_eq!(Change { weekday: Some(1), ..evs() }.moved(d("2026-11-06"), 5), d("2026-11-02"));
        assert_eq!(evs().moved(d("2026-11-06"), 5), d("2026-11-06"));
    }

    #[test]
    fn notes_and_rooms_keep_to_one_line() {
        assert_eq!(note_text("  Folien\tvorher\nlesen  "), "Folien vorher lesen");
        assert_eq!(note_text(&"ä".repeat(MAX_NOTE + 10)).chars().count(), MAX_NOTE);
        assert_eq!(room_text(" ZHG/HS.A\n"), Some("ZHG/HS.A".into()));
        assert_eq!(room_text("  "), None);
    }
}
