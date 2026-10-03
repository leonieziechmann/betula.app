//! What a page shows of an exam date whose entry at the BTU cannot mean what it says (owner
//! decisions of 2026-09-21). Radix and the snapshot keep the entry as it was read; this only
//! decides what a page shows in its place and why, so that the page can name the original next
//! to it: provenance names what was read, and a reading is never passed off as the source.
//!
//! The snapshot of 2026-09-21 has 1,078 exam dates with a time. 289 of them lie outside 06:00 to
//! 22:00, and every one follows one of two patterns of the lecture directory, neither a typo:
//!
//! - 262 × 01:00–02:30, on a Sunday or without a weekday: how QIS enters an exam whose date is
//!   not fixed („mündliche Prüfung, Termin nach Vereinbarung", „IKMZ e-Klausur"). 207 of them
//!   carry a date nine to eleven years before their semester (205 × 27.12.2015 in the WiSe
//!   2026/27), 54 none. Shifted by twelve hours they would read 13:00–14:30, a time nobody set.
//! - 27 × 23:45–24:00: the day a term paper or a take-home exam is due.
//!
//! So no time is ever shifted: the placeholder shows neither time nor weekday (nor its date where
//! that lies outside the semester), a deadline reads „bis 24:00", and whatever else looks wrong
//! keeps the source's value and is only marked.

use folia_model::rows::Semester;
use folia_model::rows_detail::EventDate;

/// An exam is held between 06:00 and 22:00 with near certainty (owner, 2026-09-21): minutes of
/// the day.
pub const DAY: (i64, i64) = (6 * 60, 22 * 60);

const MIDNIGHT: i64 = 24 * 60;

/// The times QIS enters for an exam without a fixed date.
const PLACEHOLDER: (&str, &str) = ("01:00", "02:30");

/// How far an exam may lie from its semester and still belong to it: one semester on either
/// side. Repeat exams run into the next semester (in the snapshot up to 170 days after the end:
/// a WiSe exam in September), none lies before the start.
const NEIGHBOUR_MONTHS: i64 = 6;

/// Weekday, times and dates of an exam date, as the source states them or as a page shows them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    /// 1 = Monday
    pub weekday: Option<i64>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
}

impl Slot {
    pub fn of(date: &EventDate) -> Self {
        Self {
            weekday: date.weekday,
            start_time: date.start_time.clone(),
            end_time: date.end_time.clone(),
            first_date: date.first_date.clone(),
            last_date: date.last_date.clone(),
        }
    }
}

/// Why a page shows an exam date otherwise than stated, or marks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// 01:00–02:30 on a Sunday or without a weekday: the lecture directory's entry for a date
    /// that is not fixed yet. Neither weekday nor time is shown.
    PlaceholderTime,
    /// The date of such a placeholder lies outside the semester (27.12.2015 in the WiSe
    /// 2026/27): not shown either.
    PlaceholderDate,
    /// Ends at 24:00, starting at 22:00 or later or not at all: the day something is due, shown
    /// as „bis 24:00". A reading, not a doubt, so the page does not mark it.
    Deadline,
    /// Starts before 06:00 or ends after 22:00, and is none of the above: shown as stated.
    UnusualTime,
    /// Ends before it starts: shown as stated.
    EndsBeforeStart,
    /// A date outside the semester and its two neighbours, not the placeholder's: shown as stated.
    DateOutsideSemester,
}

impl Reason {
    /// Whether a page marks the row for it: every reason but a deadline.
    pub fn is_marked(self) -> bool {
        self != Self::Deadline
    }
}

/// What a page shows of one exam date: the source's slot, the one shown, and why they differ or
/// the row is marked. Without reasons the two slots are the same.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExamReading {
    pub stated: Slot,
    pub shown: Slot,
    pub reasons: Vec<Reason>,
}

impl ExamReading {
    pub fn is_marked(&self) -> bool {
        self.reasons.iter().any(|reason| reason.is_marked())
    }

    pub fn has(&self, reason: Reason) -> bool {
        self.reasons.contains(&reason)
    }
}

/// The reading of an exam date of `v_module_exam`. `semester` is the one the date is listed
/// under; without it the dates are not judged.
pub fn read(date: &EventDate, semester: Option<&Semester>) -> ExamReading {
    let stated = Slot::of(date);
    let mut shown = stated.clone();
    let mut reasons = Vec::new();
    let outside = semester.is_some_and(|semester| {
        [&date.first_date, &date.last_date].into_iter().flatten().any(|day| belongs(semester, day) == Some(false))
    });

    if is_placeholder(date) {
        reasons.push(Reason::PlaceholderTime);
        (shown.weekday, shown.start_time, shown.end_time) = (None, None, None);
        if outside {
            reasons.push(Reason::PlaceholderDate);
            (shown.first_date, shown.last_date) = (None, None);
        }
        return ExamReading { stated, shown, reasons };
    }

    let start = date.start_time.as_deref().and_then(minutes);
    let end = date.end_time.as_deref().and_then(minutes);
    let (day_starts, day_ends) = DAY;
    if end == Some(MIDNIGHT) && start.is_none_or(|start| (day_ends..MIDNIGHT).contains(&start)) {
        reasons.push(Reason::Deadline);
        shown.start_time = None;
    } else {
        if [start, end].into_iter().flatten().any(|time| !(day_starts..=day_ends).contains(&time)) {
            reasons.push(Reason::UnusualTime);
        }
        if start.zip(end).is_some_and(|(start, end)| end < start) {
            reasons.push(Reason::EndsBeforeStart);
        }
    }
    if outside {
        reasons.push(Reason::DateOutsideSemester);
    }
    ExamReading { stated, shown, reasons }
}

fn is_placeholder(date: &EventDate) -> bool {
    matches!(date.weekday, Some(7) | None)
        && date.start_time.as_deref() == Some(PLACEHOLDER.0)
        && date.end_time.as_deref() == Some(PLACEHOLDER.1)
}

/// `11:00` as minutes of the day; `24:00` (the end of a deadline) and later count on. `None` for
/// what is not written `HH:MM`, which is then not judged.
fn minutes(time: &str) -> Option<i64> {
    let (hours, mins) = time.split_once(':')?;
    let digits = |text: &str| (text.len() == 2 && text.bytes().all(|byte| byte.is_ascii_digit())).then(|| text.parse::<i64>().ok()).flatten();
    let (hours, mins) = (digits(hours)?, digits(mins)?);
    (mins < 60).then_some(hours * 60 + mins)
}

/// `2026-09-14` as a count of months, for comparing with the semester's.
fn month(date: &str) -> Option<i64> {
    let mut parts = date.split('-');
    let (year, month) = (parts.next()?, parts.next()?);
    let (year, month): (i64, i64) = (year.parse().ok().filter(|_| year.len() == 4)?, month.parse().ok()?);
    (1..=12).contains(&month).then_some(year * 12 + month - 1)
}

/// Whether a date lies in the semester or one of its two neighbours, by month; `None` when a
/// date is not written `YYYY-MM-DD`.
fn belongs(semester: &Semester, date: &str) -> Option<bool> {
    let (starts, ends, at) = (month(&semester.starts_on)?, month(&semester.ends_on)?, month(date)?);
    Some((starts - NEIGHBOUR_MONTHS..=ends + NEIGHBOUR_MONTHS).contains(&at))
}

#[cfg(test)]
mod tests {
    use folia_model::labels::Code;

    use super::*;

    fn winter() -> Semester {
        Semester {
            key: "2026W".into(),
            season: Code::parse("winter"),
            year: 2026,
            label: "WiSe 2026/27".into(),
            starts_on: "2026-10-01".into(),
            ends_on: "2027-03-31".into(),
            is_current: false,
            teaching_events: 0,
            exam_events: 0,
        }
    }

    fn summer() -> Semester {
        Semester { key: "2026S".into(), season: Code::parse("summer"), label: "SoSe 2026".into(), starts_on: "2026-04-01".into(), ends_on: "2026-09-30".into(), ..winter() }
    }

    /// An exam date: weekday, start, end, first and last day.
    fn exam(weekday: Option<i64>, start: Option<&str>, end: Option<&str>, first: Option<&str>, last: Option<&str>) -> EventDate {
        EventDate {
            semester_key: "2026W".into(),
            semester_label: "WiSe 2026/27".into(),
            event_id: "148663".into(),
            event_number: Some("130932".into()),
            event_title: "Analysis I".into(),
            event_type: None,
            group_name: None,
            weekday,
            start_time: start.map(Into::into),
            end_time: end.map(Into::into),
            rhythm: None,
            rhythm_raw: None,
            first_date: first.map(Into::into),
            last_date: last.map(Into::into),
            room: None,
            campus: None,
            instructor: None,
            comment: None,
            source_url: None,
            room_short: None,
        }
    }

    fn on(day: &str, start: &str, end: &str) -> EventDate {
        exam(None, Some(start), Some(end), Some(day), Some(day))
    }

    #[test]
    fn the_placeholder_shows_no_time_and_no_date_from_another_year() {
        // Analysis I (11103) in the WiSe 2026/27, twice, as QIS lists it.
        let date = exam(Some(7), Some("01:00"), Some("02:30"), Some("2015-12-27"), Some("2015-12-27"));
        let reading = read(&date, Some(&winter()));
        assert_eq!(reading.reasons, [Reason::PlaceholderTime, Reason::PlaceholderDate]);
        assert_eq!(reading.shown, Slot::default());
        assert_eq!(reading.stated, Slot::of(&date), "the original stays with the reading");
        assert!(reading.is_marked());

        // Without a date (rhythm A/B, „Termine nach Vereinbarung"): only the time goes.
        let reading = read(&exam(Some(7), Some("01:00"), Some("02:30"), None, None), Some(&winter()));
        assert_eq!(reading.reasons, [Reason::PlaceholderTime]);
        assert_eq!(reading.shown, Slot::default());

        // A block of oral exams inside the semester keeps its days, not the time.
        let block = exam(None, Some("01:00"), Some("02:30"), Some("2027-02-08"), Some("2027-02-19"));
        let reading = read(&block, Some(&winter()));
        assert_eq!(reading.reasons, [Reason::PlaceholderTime]);
        assert_eq!(reading.shown, Slot { first_date: Some("2027-02-08".into()), last_date: Some("2027-02-19".into()), ..Slot::default() });

        // Without a semester the date is not judged.
        let reading = read(&exam(Some(7), Some("01:00"), Some("02:30"), Some("2015-12-27"), None), None);
        assert_eq!(reading.reasons, [Reason::PlaceholderTime]);
        assert_eq!(reading.shown.first_date.as_deref(), Some("2015-12-27"));
    }

    #[test]
    fn only_the_exact_placeholder_is_one() {
        // Another weekday, or other times: odd, shown as stated, marked. Never shifted.
        for date in [
            exam(Some(1), Some("01:00"), Some("02:30"), None, None),
            exam(Some(7), Some("01:00"), Some("03:00"), None, None),
            exam(Some(7), Some("01:30"), Some("02:30"), None, None),
            on("2027-02-14", "02:00", "04:00"),
        ] {
            let reading = read(&date, Some(&winter()));
            assert_eq!(reading.reasons, [Reason::UnusualTime], "{date:?}");
            assert_eq!(reading.shown, reading.stated);
        }
    }

    #[test]
    fn a_deadline_at_midnight_reads_bis_24_00() {
        let reading = read(&exam(Some(1), Some("23:45"), Some("24:00"), Some("2026-09-21"), Some("2026-09-21")), Some(&summer()));
        assert_eq!(reading.reasons, [Reason::Deadline]);
        assert!(!reading.is_marked());
        assert_eq!(reading.shown.start_time, None);
        assert_eq!(reading.shown.end_time.as_deref(), Some("24:00"));
        assert_eq!((reading.shown.weekday, reading.shown.first_date.as_deref()), (Some(1), Some("2026-09-21")));
        assert_eq!(read(&on("2027-03-21", "22:00", "24:00"), Some(&winter())).reasons, [Reason::Deadline]);
        assert_eq!(read(&exam(None, None, Some("24:00"), None, None), Some(&winter())).reasons, [Reason::Deadline]);

        // A whole day until midnight is no deadline: its start matters. Odd, so kept and marked.
        let day = read(&on("2027-02-15", "08:00", "24:00"), Some(&winter()));
        assert_eq!(day.reasons, [Reason::UnusualTime]);
        assert_eq!(day.shown, day.stated);
    }

    #[test]
    fn plausible_times_are_kept_and_the_edges_are_inside() {
        for (start, end) in [("09:00", "11:00"), ("06:00", "22:00"), ("07:00", "13:00"), ("20:00", "21:30"), ("11:00", "11:00")] {
            let reading = read(&on("2027-02-15", start, end), Some(&winter()));
            assert_eq!(reading.reasons, [], "{start}–{end}");
            assert_eq!(reading.shown, reading.stated);
            assert!(!reading.is_marked());
        }
        // A start alone, or no time at all.
        assert_eq!(read(&exam(Some(3), Some("10:00"), None, Some("2027-02-17"), None), Some(&winter())).reasons, []);
        assert_eq!(read(&exam(None, None, None, Some("2027-02-17"), None), Some(&winter())).reasons, []);
        assert_eq!(read(&exam(None, None, None, None, None), Some(&winter())).reasons, []);
    }

    #[test]
    fn odd_times_are_marked_not_changed() {
        let marked = |start: &str, end: &str| {
            let reading = read(&on("2027-02-15", start, end), Some(&winter()));
            assert_eq!(reading.shown, reading.stated, "{start}–{end}");
            assert!(reading.is_marked(), "{start}–{end}");
            reading.reasons
        };
        assert_eq!(marked("05:59", "08:00"), [Reason::UnusualTime]);
        assert_eq!(marked("20:00", "22:01"), [Reason::UnusualTime]);
        assert_eq!(marked("13:00", "11:00"), [Reason::EndsBeforeStart]);
        assert_eq!(marked("23:00", "01:00"), [Reason::UnusualTime, Reason::EndsBeforeStart]);
        assert_eq!(marked("18:00", "25:30"), [Reason::UnusualTime]);
        // What is not written HH:MM is not judged.
        let reading = read(&on("2027-02-15", "9 Uhr", "11"), Some(&winter()));
        assert_eq!(reading.reasons, []);
    }

    #[test]
    fn a_date_belongs_to_its_semester_and_both_neighbours() {
        let reasons = |day: &str, semester: &Semester| read(&on(day, "09:00", "11:00"), Some(semester)).reasons;
        // Repeat exams in the next semester: all in the snapshot.
        assert_eq!(reasons("2027-09-14", &winter()), []);
        assert_eq!(reasons("2027-09-30", &winter()), []);
        assert_eq!(reasons("2027-04-01", &winter()), []);
        assert_eq!(reasons("2026-10-01", &summer()), []);
        assert_eq!(reasons("2027-03-19", &summer()), []);
        assert_eq!(reasons("2026-04-01", &winter()), []);
        // Beyond them: marked, and shown as stated.
        assert_eq!(reasons("2027-10-01", &winter()), [Reason::DateOutsideSemester]);
        assert_eq!(reasons("2026-03-31", &winter()), [Reason::DateOutsideSemester]);
        assert_eq!(reasons("2015-12-27", &winter()), [Reason::DateOutsideSemester]);
        assert_eq!(reasons("2027-04-01", &summer()), [Reason::DateOutsideSemester]);
        let reading = read(&on("2015-06-01", "09:00", "11:00"), Some(&winter()));
        assert_eq!(reading.shown, reading.stated);
        // A period counts by both ends; what is no date is not judged.
        assert_eq!(read(&exam(None, None, None, Some("2027-03-01"), Some("2027-10-02")), Some(&winter())).reasons, [Reason::DateOutsideSemester]);
        assert_eq!(read(&exam(None, None, None, Some("demnächst"), None), Some(&winter())).reasons, []);
        assert_eq!(read(&exam(None, None, None, Some("27.12.2015"), None), Some(&winter())).reasons, []);
        // Both at once: an odd time on a date from another year.
        assert_eq!(read(&on("2015-12-27", "03:00", "04:00"), Some(&winter())).reasons, [Reason::UnusualTime, Reason::DateOutsideSemester]);
    }
}
