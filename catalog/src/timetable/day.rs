//! Calendar days, times of day, Easter and Brandenburg's holidays.
//!
//! The workspace has no date crate, and the Studienplan needs little of one: whole days (the
//! snapshot writes dates as `YYYY-MM-DD` and cancellations as `DD.MM.YYYY`), weekdays, ISO weeks
//! and times of day as minutes. A day is a count of days since 1970-01-01 in the proleptic
//! Gregorian calendar, converted with Howard Hinnant's `days_from_civil` and `civil_from_days`,
//! integers only, so the browser and the server count alike. There is no clock here: what „today"
//! is, the caller says.

use serde::{Deserialize, Serialize};

use crate::i18n::Locale;

/// A calendar day: days since 1970-01-01, proleptic Gregorian.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Day(pub i32);

impl Day {
    /// The day of a date, or `None` when the month or the day does not exist (leap years
    /// included).
    pub fn from_ymd(y: i32, m: u32, d: u32) -> Option<Day> {
        if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
            return None;
        }
        Some(from_civil(y, m, d))
    }

    /// Exactly `YYYY-MM-DD`, as the snapshot writes its dates.
    pub fn parse(iso: &str) -> Option<Day> {
        let [y1, y2, y3, y4, b'-', m1, m2, b'-', d1, d2] = *iso.as_bytes() else {
            return None;
        };
        let year = i32::try_from(number(&[y1, y2, y3, y4])?).ok()?;
        Day::from_ymd(year, number(&[m1, m2])?, number(&[d1, d2])?)
    }

    /// Exactly `DD.MM.YYYY`, as QIS writes a date in running text.
    pub fn parse_german(text: &str) -> Option<Day> {
        let [d1, d2, b'.', m1, m2, b'.', y1, y2, y3, y4] = *text.as_bytes() else {
            return None;
        };
        let year = i32::try_from(number(&[y1, y2, y3, y4])?).ok()?;
        Day::from_ymd(year, number(&[m1, m2])?, number(&[d1, d2])?)
    }

    /// Year, month (1–12) and day of the month (1–31).
    pub fn ymd(self) -> (i32, u32, u32) {
        civil_from_days(i64::from(self.0))
    }

    /// `2026-10-05`
    pub fn iso(self) -> String {
        let (y, m, d) = self.ymd();
        format!("{y:04}-{m:02}-{d:02}")
    }

    /// `20261005`, the form of an iCalendar date.
    pub fn compact(self) -> String {
        let (y, m, d) = self.ymd();
        format!("{y:04}{m:02}{d:02}")
    }

    /// `05.10.2026`
    pub fn german(self) -> String {
        let (y, m, d) = self.ymd();
        format!("{d:02}.{m:02}.{y:04}")
    }

    /// `05.10.`, for a date whose year the page already says.
    pub fn short(self) -> String {
        let (_, m, d) = self.ymd();
        format!("{d:02}.{m:02}.")
    }

    /// The date as a page in `locale` writes it: „05.10.2026", "5 Oct 2026".
    pub fn date(self, locale: Locale) -> String {
        let (y, m, d) = self.ymd();
        (locale.texts().common.date)(d, m, y)
    }

    /// The date without its year, for a page that already says it: „05.10.", "5 Oct".
    pub fn day_month(self, locale: Locale) -> String {
        let (_, m, d) = self.ymd();
        (locale.texts().common.day_month)(d, m)
    }

    /// 1 = Monday … 7 = Sunday, as in the event tables. 1970-01-01 was a Thursday.
    pub fn weekday(self) -> u8 {
        let index = (i64::from(self.0) + 3).rem_euclid(7);
        u8::try_from(index).unwrap_or(0) + 1
    }

    /// The Monday of the day's week.
    pub fn monday(self) -> Day {
        self.plus(1 - i32::from(self.weekday()))
    }

    /// The day `days` later (earlier when negative). Saturates at the ends of the range instead of
    /// wrapping; no real date comes near them.
    pub fn plus(self, days: i32) -> Day {
        Day(self.0.saturating_add(days))
    }

    /// ISO 8601 year and week: a week belongs to the year of its Thursday, so 2026-12-28 is in
    /// week 53 of 2026 and 2027-01-04 in week 1 of 2027.
    pub fn iso_week(self) -> (i32, u8) {
        let thursday = self.plus(4 - i32::from(self.weekday()));
        let (year, _, _) = thursday.ymd();
        let first = from_civil(year, 1, 1);
        let week = (i64::from(thursday.0) - i64::from(first.0)) / 7 + 1;
        (year, u8::try_from(week).unwrap_or(0))
    }
}

/// Minutes since midnight of `HH:MM`: `"09:15"` → 555. `"24:00"` → 1440 is allowed, because the
/// snapshot ends events there; anything else past 23:59, and anything not zero-padded, is `None`.
pub fn minutes(hhmm: &str) -> Option<u16> {
    let [h1, h2, b':', m1, m2] = *hhmm.as_bytes() else {
        return None;
    };
    let hours = number(&[h1, h2])?;
    let mins = number(&[m1, m2])?;
    match (hours, mins) {
        (0..=23, 0..=59) => u16::try_from(hours * 60 + mins).ok(),
        (24, 0) => Some(1440),
        _ => None,
    }
}

/// `HH:MM` of minutes since midnight: 555 → `"09:15"`, 1440 → `"24:00"`.
pub fn clock(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// How far `Europe/Berlin` is ahead of UTC at a time of `day` (minutes since its midnight):
/// `+02:00` in summer time, from 02:00 on the last Sunday of March to 03:00 on the last Sunday of
/// October, else `+01:00`. The hour October has twice counts as summer time. What a date and a
/// time need to name one moment for a reader outside the calendar (the structured data of a page).
pub fn berlin_offset(day: Day, minutes: u16) -> &'static str {
    let (year, _, _) = day.ymd();
    let last_sunday = |month| Day::from_ymd(year, month, 31).map(|end| end.plus(-(i32::from(end.weekday()) % 7)));
    let (Some(spring), Some(autumn)) = (last_sunday(3), last_sunday(10)) else { return "+01:00" };
    let summer = (day > spring || (day == spring && minutes >= 2 * 60)) && (day < autumn || (day == autumn && minutes < 3 * 60));
    if summer {
        "+02:00"
    } else {
        "+01:00"
    }
}

/// Easter Sunday of a Gregorian year (the anonymous computus of Meeus, Jones and Butcher).
pub fn easter_sunday(year: i32) -> Day {
    let y = i64::from(year);
    let a = y.rem_euclid(19);
    let b = y.div_euclid(100);
    let c = y.rem_euclid(100);
    let d = b.div_euclid(4);
    let e = b.rem_euclid(4);
    let f = (b + 8).div_euclid(25);
    let g = (b - f + 1).div_euclid(3);
    let h = (19 * a + b - d - g + 15).rem_euclid(30);
    let i = c.div_euclid(4);
    let k = c.rem_euclid(4);
    let l = (32 + 2 * e + 2 * i - h - k).rem_euclid(7);
    let m = (a + 11 * h + 22 * l).div_euclid(451);
    let n = h + l - 7 * m + 114;
    // March or April and 1–31 by construction; the fallbacks are never taken.
    let month = u32::try_from(n.div_euclid(31)).unwrap_or(4);
    let day = u32::try_from(n.rem_euclid(31) + 1).unwrap_or(1);
    from_civil(year, month, day)
}

/// Brandenburg (FTG Bbg): Neujahr, Karfreitag, Ostersonntag, Ostermontag, Tag der Arbeit, Christi
/// Himmelfahrt, Pfingstsonntag, Pfingstmontag, Tag der Deutschen Einheit, Reformationstag,
/// 1. und 2. Weihnachtstag. Law, not a guess. In date order.
pub fn holidays(year: i32) -> Vec<(Day, &'static str)> {
    let easter = easter_sunday(year);
    let mut days = vec![
        (from_civil(year, 1, 1), "Neujahr"),
        (easter.plus(-2), "Karfreitag"),
        (easter, "Ostersonntag"),
        (easter.plus(1), "Ostermontag"),
        (from_civil(year, 5, 1), "Tag der Arbeit"),
        (easter.plus(39), "Christi Himmelfahrt"),
        (easter.plus(49), "Pfingstsonntag"),
        (easter.plus(50), "Pfingstmontag"),
        (from_civil(year, 10, 3), "Tag der Deutschen Einheit"),
        (from_civil(year, 10, 31), "Reformationstag"),
        (from_civil(year, 12, 25), "1. Weihnachtstag"),
        (from_civil(year, 12, 26), "2. Weihnachtstag"),
    ];
    // Christi Himmelfahrt can fall on the 1st of May (it did in 2008), so sort rather than trust
    // the order above. The sort is stable: on such a day Tag der Arbeit comes first.
    days.sort_by_key(|(day, _)| *day);
    days
}

/// The day of a date the caller knows to exist (a month's first day, a fixed holiday). Saturates
/// outside the range of `Day` instead of wrapping.
pub(crate) fn from_civil(y: i32, m: u32, d: u32) -> Day {
    let z = days_from_civil(i64::from(y), i64::from(m), i64::from(d));
    Day(i32::try_from(z).unwrap_or(if z < 0 { i32::MIN } else { i32::MAX }))
}

/// Howard Hinnant's `days_from_civil`: days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Howard Hinnant's `civil_from_days`, the inverse.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    // A `Day` spans about ±5.8 million years, so the year always fits; month and day fit by
    // construction.
    (i32::try_from(y).unwrap_or(0), u32::try_from(m).unwrap_or(1), u32::try_from(d).unwrap_or(1))
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// The decimal value of a few ASCII digits, or `None` for anything else.
fn number(digits: &[u8]) -> Option<u32> {
    digits.iter().try_fold(0u32, |n, b| {
        if b.is_ascii_digit() {
            n.checked_mul(10)?.checked_add(u32::from(b - b'0'))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(y: i32, m: u32, d: u32) -> Day {
        Day::from_ymd(y, m, d).unwrap()
    }

    #[test]
    fn hinnant_round_trips_every_day_from_1990_to_2100() {
        let mut current = day(1990, 1, 1);
        let last = day(2100, 12, 31);
        let mut count = 0;
        while current <= last {
            let (y, m, d) = current.ymd();
            assert_eq!(Day::from_ymd(y, m, d), Some(current));
            assert_eq!(Day::parse(&current.iso()), Some(current));
            assert_eq!(Day::parse_german(&current.german()), Some(current));
            let next = current.plus(1);
            let (ny, nm, nd) = next.ymd();
            // The next day is the next date: the day after, or the first of the next month or year.
            assert!(
                (ny, nm, nd) == (y, m, d + 1) || (ny, nm, nd) == (y, m + 1, 1) || (ny, nm, nd) == (y + 1, 1, 1),
                "{} → {}",
                current.iso(),
                next.iso()
            );
            current = next;
            count += 1;
        }
        // 111 years, 27 of them leap years (2000 is one, 2100 is not).
        assert_eq!(count, 111 * 365 + 27);
    }

    #[test]
    fn weekdays_and_weeks() {
        assert_eq!(Day(0).iso(), "1970-01-01");
        assert_eq!(Day(0).weekday(), 4, "1970-01-01 was a Thursday");
        assert_eq!(Day(-1).weekday(), 3, "1969-12-31 was a Wednesday");
        assert_eq!(day(2026, 10, 5).weekday(), 1);
        assert_eq!(day(2026, 10, 11).weekday(), 7);
        assert_eq!(day(2026, 10, 8).monday(), day(2026, 10, 5));
        assert_eq!(day(2026, 10, 11).monday(), day(2026, 10, 5));
        assert_eq!(day(2026, 10, 5).monday(), day(2026, 10, 5));
        assert_eq!(day(2026, 12, 28).iso_week(), (2026, 53));
        assert_eq!(day(2027, 1, 3).iso_week(), (2026, 53));
        assert_eq!(day(2027, 1, 4).iso_week(), (2027, 1));
        assert_eq!(day(2026, 10, 5).iso_week(), (2026, 41));
        assert_eq!(day(2024, 12, 30).iso_week(), (2025, 1));
    }

    #[test]
    fn parsing_is_exact() {
        assert_eq!(Day::parse("2026-10-05"), Some(day(2026, 10, 5)));
        assert_eq!(Day::parse("2028-02-29"), Some(day(2028, 2, 29)));
        for bad in ["2027-02-29", "2026-13-01", "2026-00-10", "2026-10-00", "2026-10-5", " 2026-10-05", "2026/10/05", ""] {
            assert_eq!(Day::parse(bad), None, "{bad:?}");
        }
        assert_eq!(Day::parse_german("05.10.2026"), Some(day(2026, 10, 5)));
        for bad in ["5.10.2026", "05.10.26", "31.11.2026", "05.10.2026:", "2026-10-05"] {
            assert_eq!(Day::parse_german(bad), None, "{bad:?}");
        }
        assert_eq!(Day::from_ymd(1900, 2, 29), None);
        assert_eq!(Day::from_ymd(2000, 2, 29).map(Day::iso).as_deref(), Some("2000-02-29"));
    }

    #[test]
    fn formats() {
        let d = day(2026, 10, 5);
        assert_eq!(d.iso(), "2026-10-05");
        assert_eq!(d.compact(), "20261005");
        assert_eq!(d.german(), "05.10.2026");
        assert_eq!(d.short(), "05.10.");
    }

    #[test]
    fn easter_and_brandenburgs_holidays() {
        assert_eq!(easter_sunday(2026), day(2026, 4, 5));
        assert_eq!(easter_sunday(2027), day(2027, 3, 28));
        assert_eq!(easter_sunday(2028), day(2028, 4, 16));
        assert_eq!(easter_sunday(2008), day(2008, 3, 23));
        let h2027 = holidays(2027);
        assert_eq!(h2027.len(), 12);
        for (date, name) in [
            (day(2027, 3, 26), "Karfreitag"),
            (day(2027, 3, 29), "Ostermontag"),
            (day(2027, 5, 6), "Christi Himmelfahrt"),
            (day(2027, 5, 17), "Pfingstmontag"),
            (day(2027, 10, 31), "Reformationstag"),
        ] {
            assert!(h2027.contains(&(date, name)), "{} {name}", date.iso());
        }
        assert!(h2027.windows(2).all(|w| w[0].0 < w[1].0));
        // 2008: Christi Himmelfahrt on the 1st of May, the day of Tag der Arbeit.
        let h2008 = holidays(2008);
        assert!(h2008.windows(2).all(|w| w[0].0 <= w[1].0));
        assert!(h2008.contains(&(day(2008, 5, 1), "Christi Himmelfahrt")));
        // Far outside any semester, still no panic.
        let _ = holidays(i32::MAX);
        let _ = holidays(i32::MIN);
    }

    #[test]
    fn times_of_day() {
        assert_eq!(minutes("09:15"), Some(555));
        assert_eq!(minutes("00:00"), Some(0));
        assert_eq!(minutes("23:59"), Some(1439));
        assert_eq!(minutes("24:00"), Some(1440));
        for bad in ["24:01", "7:5", "07:5", "7:05", "25:00", "12:60", "12.30", "", "ab:cd"] {
            assert_eq!(minutes(bad), None, "{bad:?}");
        }
        assert_eq!(clock(555), "09:15");
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(1440), "24:00");
    }

    #[test]
    fn berlin_is_an_hour_ahead_in_winter_and_two_in_summer() {
        let at = |y, m, d, hhmm| berlin_offset(day(y, m, d), minutes(hhmm).unwrap());
        // 2026: summer time from 29 March, 02:00, to 25 October, 03:00.
        assert_eq!(at(2026, 3, 29, "01:59"), "+01:00");
        assert_eq!(at(2026, 3, 29, "03:00"), "+02:00");
        assert_eq!(at(2026, 7, 1, "12:00"), "+02:00");
        assert_eq!(at(2026, 10, 25, "02:30"), "+02:00");
        assert_eq!(at(2026, 10, 25, "03:00"), "+01:00");
        assert_eq!(at(2027, 2, 15, "09:00"), "+01:00");
        // 2027: the last Sunday of October is its last day.
        assert_eq!(at(2027, 3, 27, "10:00"), "+01:00");
        assert_eq!(at(2027, 3, 28, "10:00"), "+02:00");
        assert_eq!(at(2027, 10, 31, "02:59"), "+02:00");
        assert_eq!(at(2027, 10, 31, "03:00"), "+01:00");
    }

    #[test]
    fn the_ends_of_the_range_do_not_panic() {
        assert_eq!(Day(i32::MAX).plus(1), Day(i32::MAX));
        assert_eq!(Day(i32::MIN).plus(-1), Day(i32::MIN));
        for d in [Day(i32::MAX), Day(i32::MIN)] {
            let _ = (d.ymd(), d.weekday(), d.monday(), d.iso_week(), d.iso(), berlin_offset(d, 0));
        }
    }
}
