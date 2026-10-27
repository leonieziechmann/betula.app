//! A Termin of one's own (folia/design/kalender/termine.html, its first step; owner, 2026-10-27:
//! „Passt alles, bau Schritt 1 jetzt in Folia"): what a student adds to a calendar beside what QIS
//! lists — a job, sport, a Lerngruppe, the Termin of a module QIS has none for (a fifth of the
//! modules of 2026W with an event have no Termin with a day and a time). It recurs every week, in
//! the A or in the B weeks, through the lecture period or a period of its own, or it is held on
//! dates of its own; it has a time, a name and, where it has them, a place and the module it
//! belongs to. It is firm, as a module's Termin is: where it meets one, both are a clash, and a
//! choice of groups takes it as one more Termin of the plan.
//!
//! It is keyed like a Termin of QIS (`RowKey`), by an event number QIS never gives (`OWN_EVENT_BASE`
//! and above; the snapshot's `veranstid`s have six digits) and no fingerprint, so its key stays when
//! it is changed: the week, the day, its own view and „Nur am … ausblenden"
//! (`Selection::hidden_dates`) take it as a Termin, and what names it keeps naming it.

use serde::{Deserialize, Serialize};

use crate::day::Day;
use crate::rowkey::RowKey;

/// The event number of the Termin of one's own numbered 0; the first one is one more. Above every
/// `veranstid` QIS gives, and below `u32::MAX` by more than the numbers there are.
pub const OWN_EVENT_BASE: u32 = 4_000_000_000;

/// The largest number of a Termin of one's own: five digits keep its line short.
pub const MAX_OWN_NUMBER: u32 = 99_999;

/// The most dates of one on dates of its own: a block seminar, the Termine of a term's project.
pub const MAX_OWN_DATES: usize = 30;

/// The most characters of its name and of its place.
pub const MAX_OWN_TEXT: usize = 80;

/// How a Termin of one's own recurs: every week, every other week in the A or in the B weeks
/// (`SemesterFacts::ab_week`), or not at all, on dates of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum OwnRhythm {
    Week,
    AWeek,
    BWeek,
    Dates,
}

impl OwnRhythm {
    pub const ALL: [Self; 4] = [Self::Week, Self::AWeek, Self::BWeek, Self::Dates];

    /// The letter its line keeps.
    pub fn code(self) -> &'static str {
        match self {
            Self::Week => "w",
            Self::AWeek => "a",
            Self::BWeek => "b",
            Self::Dates => "d",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|rhythm| rhythm.code() == code)
    }

    /// It recurs on a weekday.
    pub fn recurs(self) -> bool {
        self != Self::Dates
    }
}

/// A Termin of one's own, of one semester and one calendar (the plan's, or one of one's own; the
/// plan keeps which, `folia_plans::studyplan`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OwnTermin {
    /// 1..=`MAX_OWN_NUMBER`, one per Termin of the plan's text, whatever its calendar: its key.
    pub number: u32,
    pub rhythm: OwnRhythm,
    /// The weekday it recurs on, 1 = Monday … 7 = Sunday; 0 for one on dates of its own.
    pub weekday: u8,
    /// The dates of one on dates of its own, in order and each once; empty for one that recurs.
    pub dates: Vec<Day>,
    /// Minutes since midnight, `from < to <= 1440`.
    pub from: u16,
    pub to: u16,
    /// The first and the last day of one that recurs, where they are not the lecture period's.
    pub period: Option<(Day, Day)>,
    /// The module it belongs to („Gehört zu"), by id, where it belongs to one: it takes the
    /// module's tone and stands with it.
    pub module: Option<String>,
    /// At most `MAX_OWN_TEXT` characters; empty where it has none (a page names it „Eigener
    /// Termin" then: nothing about it is required, owner 2026-10-18: „Betula soll nie sagen. Nein").
    pub name: String,
    /// Where it is held, at most `MAX_OWN_TEXT` characters; empty where it is not said.
    pub place: String,
    /// The days it is not held on („Nur am 30.10. ausblenden"), in order and each once, at most
    /// `MAX_OWN_DATES`: the timetable takes them out of its days, as a hidden date of a Termin of
    /// QIS (`Selection::hidden_dates`), and its view still names them.
    pub skip: Vec<Day>,
}

impl OwnTermin {
    /// Its key: what the week, the day and its view name it by.
    pub fn key(&self) -> RowKey {
        RowKey { event: OWN_EVENT_BASE + self.number, fp: 0 }
    }

    /// Whether it is one: its number in range, its time a time, a weekday where it recurs and a
    /// date where it does not, a period that ends after it begins, no text too long, and the days
    /// it skips in order.
    pub fn is_valid(&self) -> bool {
        let number = (1..=MAX_OWN_NUMBER).contains(&self.number);
        let time = self.from < self.to && self.to <= 24 * 60;
        let when = match self.rhythm {
            OwnRhythm::Dates => self.weekday == 0 && !self.dates.is_empty() && self.dates.len() <= MAX_OWN_DATES && self.dates.windows(2).all(|pair| pair.first() < pair.get(1)),
            _ => (1..=7).contains(&self.weekday) && self.dates.is_empty(),
        };
        let period = self.period.is_none_or(|(first, last)| self.rhythm.recurs() && first <= last);
        let texts = self.name.chars().count() <= MAX_OWN_TEXT && self.place.chars().count() <= MAX_OWN_TEXT;
        let skip = self.skip.len() <= MAX_OWN_DATES && self.skip.windows(2).all(|pair| pair.first() < pair.get(1));
        number && time && when && period && texts && skip
    }
}

/// The number of the Termin of one's own an event number names, where it names one.
pub fn own_number(event: u32) -> Option<u32> {
    event.checked_sub(OWN_EVENT_BASE).filter(|number| (1..=MAX_OWN_NUMBER).contains(number))
}

/// Whether a key names a Termin of one's own.
pub fn is_own(key: RowKey) -> bool {
    own_number(key.event).is_some()
}

/// Whether an event id (`Event::id`, the text of a `veranstid`) is a Termin of one's own.
pub fn is_own_event(id: &str) -> bool {
    id.parse::<u32>().ok().and_then(own_number).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn job() -> OwnTermin {
        OwnTermin {
            number: 3,
            rhythm: OwnRhythm::Week,
            weekday: 5,
            dates: Vec::new(),
            from: 13 * 60,
            to: 17 * 60,
            period: None,
            module: None,
            name: "Arbeit".to_string(),
            place: "Café Mosaik".to_string(),
            skip: Vec::new(),
        }
    }

    #[test]
    fn its_key_is_a_termins_that_qis_never_gives() {
        let key = job().key();
        assert_eq!(key.text(), "4000000003-00000");
        assert_eq!(RowKey::parse(&key.text()), Some(key));
        assert!(is_own(key) && is_own_event("4000000003"));
        assert_eq!(own_number(key.event), Some(3));
        assert!(!is_own(RowKey { event: 148_701, fp: 0xb7025 }) && !is_own_event("148701") && !is_own_event("4000000000"));
        assert_eq!(own_number(OWN_EVENT_BASE + MAX_OWN_NUMBER), Some(MAX_OWN_NUMBER));
        assert_eq!(own_number(OWN_EVENT_BASE + MAX_OWN_NUMBER + 1), None);
        // The packed key of a subscription code holds it.
        assert_eq!(RowKey::unpack(key.packed()), Some(key));
    }

    #[test]
    fn a_termin_is_valid_where_it_can_be() {
        assert!(job().is_valid());
        let weekend = OwnTermin { weekday: 7, ..job() };
        assert!(weekend.is_valid(), "a Sunday is a day too");
        let till_midnight = OwnTermin { from: 22 * 60, to: 24 * 60, ..job() };
        assert!(till_midnight.is_valid());
        let unnamed = OwnTermin { name: String::new(), place: String::new(), ..job() };
        assert!(unnamed.is_valid(), "nothing about it is required");
        let dates = OwnTermin { rhythm: OwnRhythm::Dates, weekday: 0, dates: vec![d("2026-11-12"), d("2026-11-19")], ..job() };
        assert!(dates.is_valid());
        let a_week = OwnTermin { rhythm: OwnRhythm::AWeek, period: Some((d("2026-10-15"), d("2026-12-18"))), ..job() };
        assert!(a_week.is_valid());
        let skipping = OwnTermin { skip: vec![d("2026-10-30"), d("2026-11-06")], ..job() };
        assert!(skipping.is_valid());

        for broken in [
            OwnTermin { number: 0, ..job() },
            OwnTermin { number: MAX_OWN_NUMBER + 1, ..job() },
            OwnTermin { from: 17 * 60, to: 13 * 60, ..job() },
            OwnTermin { to: 24 * 60 + 1, ..job() },
            OwnTermin { weekday: 0, ..job() },
            OwnTermin { weekday: 8, ..job() },
            OwnTermin { dates: vec![d("2026-11-12")], ..job() },
            OwnTermin { rhythm: OwnRhythm::Dates, weekday: 0, ..job() },
            OwnTermin { rhythm: OwnRhythm::Dates, weekday: 0, dates: vec![d("2026-11-19"), d("2026-11-12")], ..job() },
            OwnTermin { rhythm: OwnRhythm::Dates, weekday: 0, dates: vec![d("2026-11-12")], period: Some((d("2026-11-01"), d("2026-11-30"))), ..job() },
            OwnTermin { period: Some((d("2026-12-18"), d("2026-10-15"))), ..job() },
            OwnTermin { name: "x".repeat(MAX_OWN_TEXT + 1), ..job() },
            OwnTermin { place: "x".repeat(MAX_OWN_TEXT + 1), ..job() },
            OwnTermin { skip: vec![d("2026-11-06"), d("2026-10-30")], ..job() },
            OwnTermin { skip: vec![d("2026-10-30"); 2], ..job() },
        ] {
            assert!(!broken.is_valid(), "{broken:?}");
        }
    }

    #[test]
    fn rhythms_by_their_letters() {
        for rhythm in OwnRhythm::ALL {
            assert_eq!(OwnRhythm::from_code(rhythm.code()), Some(rhythm));
        }
        assert_eq!(OwnRhythm::from_code("x"), None);
        assert!(OwnRhythm::BWeek.recurs() && !OwnRhythm::Dates.recurs());
    }
}
