//! Calendar semesters and Fachsemester.
//!
//! A Studienplan is keyed by the calendar semester (`2026W`), because events, exams, subscription
//! codes and Radix's retention all are. The Fachsemester („1. FS") is only a label: it follows from
//! the Studienbeginn, so changing the start relabels a plan and moves nothing.

use folia_locale::Locale;
use folia_model::labels::Season;
use serde::{Deserialize, Serialize};

use crate::day::{from_civil, Day};

/// The first and the last year a key may name. Keys come from URLs and from storage, so they are
/// bounded like everything read from there; this century is plenty for a catalog of the BTU.
const YEARS: (u16, u16) = (2000, 2099);

/// The longest a study takes, in semesters: a Fachsemester past it is refused, not counted.
const MAX_FACHSEMESTER: u8 = 30;

/// A calendar semester: the summer of `year`, or the winter that starts in `year`. Ordered
/// chronologically (2026S < 2026W < 2027S).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SemesterKey {
    pub year: u16,
    pub winter: bool,
}

impl SemesterKey {
    /// A key of the years 2000–2099.
    pub fn new(year: u16, winter: bool) -> Option<Self> {
        (YEARS.0..=YEARS.1).contains(&year).then_some(Self { year, winter })
    }

    /// `2026W` or `2026S` as Radix writes `semester.key`; trimmed, either case.
    pub fn parse(key: &str) -> Option<Self> {
        let [y1, y2, y3, y4, season] = *key.trim().as_bytes() else {
            return None;
        };
        let winter = match season.to_ascii_uppercase() {
            b'W' => true,
            b'S' => false,
            _ => return None,
        };
        let year = [y1, y2, y3, y4].iter().try_fold(0u16, |n, b| {
            b.is_ascii_digit().then(|| n * 10 + u16::from(b - b'0'))
        })?;
        Self::new(year, winter)
    }

    /// `2026W`
    pub fn key(self) -> String {
        format!("{}{}", self.year, if self.winter { 'W' } else { 'S' })
    }

    /// „WiSe 2026/27", „SoSe 2027" (as Radix writes `semester.label`), "Winter 2026/27".
    pub fn label(self, locale: Locale) -> String {
        let common = locale.texts();
        let next = (self.year % 100 + 1) % 100;
        if self.winter {
            (common.winter_semester)(self.year, next)
        } else {
            (common.summer_semester)(self.year)
        }
    }

    /// „WiSe 26/27", „SoSe 27", "WS 26/27", where space is short.
    pub fn short(self, locale: Locale) -> String {
        let common = locale.texts();
        if self.winter {
            (common.winter_semester_short)(self.year % 100, (self.year % 100 + 1) % 100)
        } else {
            (common.summer_semester_short)(self.year % 100)
        }
    }

    /// `year * 2 + 1` for a winter, `year * 2` for a summer: 2026S 4052, 2026W 4053, 2027S 4054.
    /// Consecutive semesters have consecutive numbers, which makes Fachsemester a subtraction and
    /// the subscription code's semester one small number.
    pub fn index(self) -> u16 {
        // The fields are public and deserializable, so a key outside the constructor's years must
        // not overflow; it saturates to a number `from_index` refuses.
        u16::try_from(u32::from(self.year) * 2 + u32::from(self.winter)).unwrap_or(u16::MAX)
    }

    /// The key of an `index()`, or `None` outside 2000–2099.
    pub fn from_index(n: u16) -> Option<Self> {
        Self::new(n / 2, n % 2 == 1)
    }

    /// The semester `n` semesters later (earlier when negative), or `None` past the years a key
    /// may name.
    pub fn plus(self, n: i32) -> Option<Self> {
        let index = i32::from(self.index()).checked_add(n)?;
        Self::from_index(u16::try_from(index).ok()?)
    }

    pub fn season(self) -> Season {
        if self.winter {
            Season::Winter
        } else {
            Season::Summer
        }
    }

    /// The half-year the semester stands for: 01.04.–30.09. for a summer, 01.10.–31.03. for a
    /// winter. Not the lecture period, which the data says (`facts`).
    pub fn bounds(self) -> (Day, Day) {
        let year = i32::from(self.year);
        if self.winter {
            (from_civil(year, 10, 1), from_civil(year + 1, 3, 31))
        } else {
            (from_civil(year, 4, 1), from_civil(year, 9, 30))
        }
    }
}

/// The Fachsemester `at` is for a study started in `start`: 1 for the first semester. `None`
/// before the start and past 30 semesters.
pub fn fachsemester(at: SemesterKey, start: SemesterKey) -> Option<u8> {
    let n = i32::from(at.index()) - i32::from(start.index()) + 1;
    u8::try_from(n).ok().filter(|n| (1..=MAX_FACHSEMESTER).contains(n))
}

/// The calendar semester of Fachsemester `n` (1–30) of a study started in `start`.
pub fn of_fachsemester(start: SemesterKey, n: u8) -> Option<SemesterKey> {
    if !(1..=MAX_FACHSEMESTER).contains(&n) {
        return None;
    }
    start.plus(i32::from(n) - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    #[test]
    fn keys_parse_and_print() {
        assert_eq!(key("2026W"), SemesterKey { year: 2026, winter: true });
        assert_eq!(key(" 2026w "), key("2026W"));
        assert_eq!(key("2027s").key(), "2027S");
        for bad in ["2026X", "26W", "20260W", "1999W", "2100S", "", "W2026", "2026 W"] {
            assert_eq!(SemesterKey::parse(bad), None, "{bad:?}");
        }
        assert_eq!(SemesterKey::new(1999, true), None);
        assert_eq!(SemesterKey::new(2099, true).map(|key| key.label(Locale::De)).as_deref(), Some("WiSe 2099/00"));
    }

    #[test]
    fn labels_as_radix_writes_them() {
        assert_eq!(key("2026W").label(Locale::De), "WiSe 2026/27");
        assert_eq!(key("2027S").label(Locale::De), "SoSe 2027");
        assert_eq!(key("2026W").short(Locale::De), "WiSe 26/27");
        assert_eq!(key("2027S").short(Locale::De), "SoSe 27");
        assert_eq!((key("2026W").label(Locale::En), key("2027S").short(Locale::En)), ("Winter 2026/27".to_string(), "SS 27".to_string()));
        assert_eq!(key("2026W").season(), Season::Winter);
        assert_eq!(key("2026S").season(), Season::Summer);
    }

    #[test]
    fn index_order_and_steps() {
        assert_eq!(key("2026S").index(), 4052);
        assert_eq!(key("2026W").index(), 4053);
        assert_eq!(key("2027S").index(), 4054);
        assert_eq!(SemesterKey::from_index(4053), Some(key("2026W")));
        assert_eq!(SemesterKey::from_index(0), None);
        assert!(key("2026S") < key("2026W") && key("2026W") < key("2027S"));
        assert_eq!(key("2026W").plus(1), Some(key("2027S")));
        assert_eq!(key("2026W").plus(-1), Some(key("2026S")));
        assert_eq!(key("2026W").plus(4), Some(key("2028W")));
        assert_eq!(key("2000S").plus(-1), None);
        assert_eq!(key("2099W").plus(1), None);
        assert_eq!(key("2026W").plus(i32::MAX), None);
        assert_eq!(key("2026W").plus(i32::MIN), None);
    }

    #[test]
    fn bounds_are_the_half_year() {
        let (first, last) = key("2026W").bounds();
        assert_eq!((first.iso(), last.iso()), ("2026-10-01".to_string(), "2027-03-31".to_string()));
        let (first, last) = key("2026S").bounds();
        assert_eq!((first.iso(), last.iso()), ("2026-04-01".to_string(), "2026-09-30".to_string()));
    }

    #[test]
    fn fachsemester_follows_the_start() {
        // Winter start 2026W: FS1 2026W, FS2 2027S, FS3 2027W.
        let start = key("2026W");
        assert_eq!(fachsemester(key("2026W"), start), Some(1));
        assert_eq!(fachsemester(key("2027S"), start), Some(2));
        assert_eq!(fachsemester(key("2027W"), start), Some(3));
        assert_eq!(fachsemester(key("2026S"), start), None);
        assert_eq!(of_fachsemester(start, 1), Some(start));
        assert_eq!(of_fachsemester(start, 3), Some(key("2027W")));
        // Summer start 2027S (the four current masters with a summer intake): no special case.
        let summer = key("2027S");
        assert_eq!(fachsemester(key("2027S"), summer), Some(1));
        assert_eq!(fachsemester(key("2027W"), summer), Some(2));
        assert_eq!(of_fachsemester(summer, 2), Some(key("2027W")));
        // 1–30 only.
        assert_eq!(of_fachsemester(start, 0), None);
        assert_eq!(of_fachsemester(start, 31), None);
        assert_eq!(fachsemester(start.plus(29).unwrap(), start), Some(30));
        assert_eq!(fachsemester(start.plus(30).unwrap(), start), None);
        for n in 1..=30 {
            assert_eq!(fachsemester(of_fachsemester(start, n).unwrap(), start), Some(n));
        }
    }
}
