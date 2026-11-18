//! What Betula announces by itself: the general dates of a semester, read from the catalog.
//!
//! The lectures begin, they pause over Christmas, the exam periods begin, the next semester's
//! dates are there: none of it is written anywhere a program could read it, and all of it follows
//! from the dates the catalog holds (`timetable::facts`: the lecture period and its breaks from
//! the teaching rows, the exam periods from the exam rows; the semester the catalog shows from
//! Radix's `current_semester`, which moves on once the next schedule is published). So the notices
//! here are what the start page already says of a semester („Weihnachtspause bis …"), turned into
//! moments: each with the day and hour it is worth telling.
//!
//! The server hands them out as a feed (`GET /api/announcements`), which Xylem, the service of the
//! notifications, reads and tells to the browsers that listen (docs/xylem/protocol.md section 7).
//! A notice is known by its key: read again with another time, it is the same notice, moved.
//! Where the data does not say (a semester without a lecture period, too few exam dates yet),
//! there is no notice: nothing is guessed (R12).

use folia_calendar::day::Day;
use folia_calendar::semester::SemesterKey;
use folia_model::db::{Database, DbError};
use folia_query as queries;
use folia_timetable::facts::{self, SemesterFacts};

/// How long a notice stays named after its day: Xylem tells one that is late only while it is
/// told for, and forgets nothing it has; the feed just need not carry a whole semester's past.
pub const KEEP_DAYS: i32 = 3;

/// When the day's notices are told: before the first lecture (07:30).
const MORNING: u16 = 7 * 60;
/// When a break is announced: the Friday afternoon before it.
const AFTERNOON: u16 = 15 * 60;

/// One thing to announce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub semester: SemesterKey,
    pub what: What,
    /// When it is told: a day and the minutes of it, in Berlin. `None`: it is so from now on and
    /// has no hour of its own (a semester's dates being there); whoever reads it first tells it.
    pub at: Option<(Day, u16)>,
    /// How long it is told, in hours: a device that was off hears it within them, nobody after.
    pub hours: u16,
}

/// What a notice says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    /// The semester's first lectures are today.
    LecturesBegin,
    /// The lectures pause from Monday, for `weeks` weeks, and resume on `resume`: the `nr`-th
    /// break of the lecture period (1 is the first). `christmas`: the one over Christmas.
    Break { nr: u8, weeks: u8, resume: Day, christmas: bool },
    /// The `nr`-th exam period of the semester begins today; its last exams are on `last`.
    Exams { nr: u8, last: Day },
    /// The semester's dates are published, and it has not begun: time to plan it.
    Dates,
}

impl Notice {
    /// What the notice is known by, the same every time it is read: `lectures-2026w-begin`,
    /// `breaks-2026w-1`, `exams-2026w-2`, `dates-2027s`.
    pub fn key(&self) -> String {
        let semester = self.semester.key().to_lowercase();
        match self.what {
            What::LecturesBegin => format!("lectures-{semester}-begin"),
            What::Break { nr, .. } => format!("breaks-{semester}-{nr}"),
            What::Exams { nr, .. } => format!("exams-{semester}-{nr}"),
            What::Dates => format!("dates-{semester}"),
        }
    }

    /// The topic a browser listens to for it (docs/xylem/protocol.md section 1).
    pub fn topic(&self) -> &'static str {
        match self.what {
            What::LecturesBegin | What::Dates => "semester",
            What::Break { .. } => "breaks",
            What::Exams { .. } => "exams",
        }
    }
}

/// What the catalog says of one semester.
#[derive(Clone, Debug, PartialEq)]
pub struct Evidence {
    pub facts: SemesterFacts,
    /// `facts::exam_periods`.
    pub exams: Vec<(Day, Day)>,
}

/// The notices of `today`: those of every semester the catalog has dates of, from `KEEP_DAYS`
/// days back on, in the order they are told (those without an hour last). `current` is the
/// semester the catalog shows (`meta.current_semester`).
pub fn notices(today: Day, current: Option<SemesterKey>, semesters: &[Evidence]) -> Vec<Notice> {
    let mut out = Vec::new();
    for Evidence { facts, exams } in semesters {
        let semester = facts.key;
        if let Some((first, _)) = facts.lecture {
            if let Some(day) = first_lecture_day(facts, first) {
                out.push(Notice { semester, what: What::LecturesBegin, at: Some((day, MORNING)), hours: 12 });
            }
            for (nr, (from, to)) in (1u8..).zip(&facts.breaks) {
                let christmas = |month: u32, date: u32| (from.0..=to.0).any(|n| matches!(Day(n).ymd(), (_, m, d) if m == month && d == date));
                let weeks = u8::try_from((i64::from(to.0) - i64::from(from.0) + 1) / 7).unwrap_or(u8::MAX);
                out.push(Notice {
                    semester,
                    what: What::Break { nr, weeks, resume: to.plus(1), christmas: christmas(12, 24) || christmas(1, 1) },
                    // A break is whole weeks from a Monday: told on the Friday before, for the
                    // weekend, until the Monday morning.
                    at: Some((from.plus(-3), AFTERNOON)),
                    hours: 60,
                });
            }
        }
        for (nr, (from, to)) in (1u8..).zip(exams) {
            // A period is whole weeks to a Sunday; its last exams are on the Saturday.
            out.push(Notice { semester, what: What::Exams { nr, last: to.plus(-1) }, at: Some((*from, MORNING)), hours: 12 });
        }
    }
    if let Some(semester) = current {
        let begins = semesters.iter().find(|evidence| evidence.facts.key == semester).map_or(semester.bounds().0, |evidence| evidence.facts.bounds.0);
        if begins > today {
            out.push(Notice { semester, what: What::Dates, at: None, hours: 72 });
        }
    }
    out.retain(|notice| notice.at.is_none_or(|(day, _)| day.0 >= today.0.saturating_sub(KEEP_DAYS)));
    out.sort_by_key(|notice| (notice.at.is_none(), notice.at, notice.key()));
    out
}

/// The day the first lectures are held: the period's first day inside the half-year that is a
/// working day (2026S begins on a Wednesday; a first Monday may be Easter Monday).
fn first_lecture_day(facts: &SemesterFacts, first: Day) -> Option<Day> {
    let from = first.max(facts.bounds.0);
    (0..7).map(|days| from.plus(days)).find(|day| day.weekday() <= 5 && facts.holiday(*day).is_none())
}

/// The notices of `today` from a snapshot.
pub fn announcements(db: &dyn Database, today: Day) -> Result<Vec<Notice>, DbError> {
    let current = queries::meta(db)?.current_semester.as_deref().and_then(SemesterKey::parse);
    let mut evidence = Vec::new();
    for semester in queries::semesters(db)? {
        let Some(key) = SemesterKey::parse(&semester.key) else { continue };
        let facts = SemesterFacts::derive(key, Some(&semester), &queries::semester_date_counts(db, &semester.key)?);
        let exams = facts::exam_periods(facts.bounds, &queries::semester_exam_counts(db, &semester.key)?);
        evidence.push(Evidence { facts, exams });
    }
    Ok(notices(today, current, &evidence))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    /// A semester with the period, the breaks and the exam periods given (its half-year and its
    /// holidays are its key's).
    fn semester(name: &str, lecture: Option<(&str, &str)>, breaks: &[(&str, &str)], exams: &[(&str, &str)]) -> Evidence {
        let pairs = |list: &[(&str, &str)]| list.iter().map(|(first, last)| (d(first), d(last))).collect::<Vec<_>>();
        let mut facts = SemesterFacts::derive(key(name), None, &[]);
        facts.lecture = lecture.map(|(first, last)| (d(first), d(last)));
        facts.breaks = pairs(breaks);
        Evidence { facts, exams: pairs(exams) }
    }

    /// The winter of 2026/27 as the catalog of 2026-11-18 says it.
    fn winter() -> Evidence {
        semester(
            "2026W",
            Some(("2026-10-05", "2027-01-31")),
            &[("2026-12-21", "2027-01-03")],
            &[("2027-02-08", "2027-02-21"), ("2027-03-08", "2027-03-21")],
        )
    }

    fn told(notices: &[Notice]) -> Vec<String> {
        notices
            .iter()
            .map(|notice| match notice.at {
                Some((day, minutes)) => format!("{} {} {:02}:{:02}", notice.key(), day.iso(), minutes / 60, minutes % 60),
                None => notice.key(),
            })
            .collect()
    }

    #[test]
    fn a_winter_in_november() {
        let list = notices(d("2026-11-18"), Some(key("2026W")), &[winter()]);
        assert_eq!(told(&list), ["breaks-2026w-1 2026-12-18 15:00", "exams-2026w-1 2027-02-08 07:00", "exams-2026w-2 2027-03-08 07:00"]);
        let [pause, first, second] = list.as_slice() else { panic!("{list:?}") };
        assert_eq!(pause.what, What::Break { nr: 1, weeks: 2, resume: d("2027-01-04"), christmas: true });
        assert_eq!((pause.topic(), pause.hours), ("breaks", 60));
        // Friday 15:00 and sixty hours: told until Monday 03:00, the night before the break's first morning.
        assert_eq!((d("2026-12-18").weekday(), d("2026-12-21").weekday()), (5, 1));
        assert_eq!(first.what, What::Exams { nr: 1, last: d("2027-02-20") });
        assert_eq!(second.what, What::Exams { nr: 2, last: d("2027-03-20") });
        assert_eq!((first.topic(), first.hours, first.semester), ("exams", 12, key("2026W")));
    }

    #[test]
    fn a_notice_stays_named_three_days() {
        let begin = "lectures-2026w-begin 2026-10-05 07:00";
        for (today, named) in [("2026-09-01", true), ("2026-10-05", true), ("2026-10-08", true), ("2026-10-09", false)] {
            let list = told(&notices(d(today), Some(key("2026W")), &[winter()]));
            assert_eq!(list.iter().any(|line| line == begin), named, "{today}: {list:?}");
        }
        // Long after everything: nothing, and nothing of a semester the catalog says nothing of.
        assert!(notices(d("2027-03-30"), Some(key("2026W")), &[winter()]).is_empty());
        assert!(notices(d("2026-11-18"), Some(key("2026W")), &[semester("2026W", None, &[("2026-12-21", "2027-01-03")], &[])]).is_empty());
        assert!(notices(d("2026-11-18"), None, &[]).is_empty());
    }

    #[test]
    fn the_first_lecture_day_is_a_working_day_of_the_semester() {
        let begins = |evidence: Evidence| {
            let list = notices(d("2026-01-01"), None, &[evidence]);
            list.iter().find(|notice| notice.what == What::LecturesBegin).and_then(|notice| notice.at).map(|(day, _)| day.iso())
        };
        // The summer of 2026 begins on Wednesday 01.04., in the week its lecture period is counted from.
        assert_eq!(begins(semester("2026S", Some(("2026-03-30", "2026-07-12")), &[], &[])).as_deref(), Some("2026-04-01"));
        // A period from Easter Monday begins on the Tuesday.
        assert_eq!(begins(semester("2026S", Some(("2026-04-06", "2026-07-12")), &[], &[])).as_deref(), Some("2026-04-07"));
        assert_eq!(begins(semester("2026S", Some(("2026-04-13", "2026-07-26")), &[], &[])).as_deref(), Some("2026-04-13"));
    }

    #[test]
    fn the_next_semesters_dates_are_news_until_it_begins() {
        // The catalog moved on to the summer while the winter still runs.
        let both = [winter(), semester("2027S", Some(("2027-04-12", "2027-07-25")), &[], &[])];
        let list = notices(d("2027-02-20"), Some(key("2027S")), &both);
        assert_eq!(told(&list), ["exams-2026w-2 2027-03-08 07:00", "lectures-2027s-begin 2027-04-12 07:00", "dates-2027s"]);
        let dates = list.last().unwrap();
        assert_eq!((dates.what, dates.at, dates.topic(), dates.hours, dates.semester), (What::Dates, None, "semester", 72, key("2027S")));
        // Until the day before it begins, and without any row of it in the catalog as well.
        assert_eq!(told(&notices(d("2027-03-31"), Some(key("2027S")), &[])), ["dates-2027s"]);
        assert!(notices(d("2027-04-01"), Some(key("2027S")), &[]).is_empty());
        // The semester that runs is no news.
        assert!(!told(&notices(d("2026-11-18"), Some(key("2026W")), &[winter()])).iter().any(|line| line.starts_with("dates")));
    }

    /// Against the catalog at hand: whatever it says, the notices keep their shape, and every
    /// sitting is counted once.
    #[test]
    fn the_notices_of_the_catalog_at_hand() {
        let db = folia_test_support::open();
        let semesters = queries::semesters(&db).unwrap();
        assert!(!semesters.is_empty());
        let mut earliest = Day::parse("2099-01-01").unwrap();
        for semester in &semesters {
            let counts = queries::semester_exam_counts(&db, &semester.key).unwrap();
            let sittings = folia_test_support::scalar(
                &db,
                &format!(
                    "SELECT COUNT(DISTINCT event_id || '/' || COALESCE(ord, 0)) FROM v_module_exam WHERE semester_key = '{}' \
                     AND first_date IS NOT NULL AND (start_time IS NULL OR (start_time >= '06:00' AND start_time < '22:00'))",
                    semester.key
                ),
            );
            assert_eq!(counts.iter().map(|count| count.dates).sum::<i64>(), sittings, "{}", semester.key);
            assert!(counts.iter().all(|count| count.rhythm.is_none() && count.dates > 0));
            let bounds = key(&semester.key).bounds();
            let periods = facts::exam_periods(bounds, &counts);
            for (first, last) in &periods {
                assert_eq!((first.weekday(), last.weekday()), (1, 7), "{}", semester.key);
                assert!(bounds.0.monday() <= *first && *last <= bounds.1.monday().plus(6), "{}: an exam period outside the half-year", semester.key);
            }
            assert!(periods.windows(2).all(|pair| pair[0].1.plus(1) < pair[1].0), "{}: periods in order, apart", semester.key);
            earliest = earliest.min(bounds.0);
        }
        // From a day before all of it, every notice is named.
        let all = announcements(&db, earliest.plus(-30)).unwrap();
        let mut keys: Vec<String> = all.iter().map(Notice::key).collect();
        assert!(keys.iter().all(|key| key.len() <= 64 && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')), "{keys:?}");
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), all.len(), "a key twice: {:?}", told(&all));
        for notice in &all {
            match (notice.what, notice.at) {
                (What::LecturesBegin, Some((day, minutes))) => assert!(day.weekday() <= 5 && minutes == 7 * 60),
                (What::Break { weeks, resume, .. }, Some((day, minutes))) => {
                    assert!(day.weekday() == 5 && minutes == 15 * 60 && resume.weekday() == 1 && weeks >= 1, "{notice:?}");
                    assert_eq!(resume.0 - day.0, 3 + 7 * i32::from(weeks), "{notice:?}");
                }
                (What::Exams { last, .. }, Some((day, _))) => assert!(day.weekday() == 1 && last.weekday() == 6 && last > day, "{notice:?}"),
                (What::Dates, None) => {}
                other => panic!("{other:?}"),
            }
        }
        assert!(all.windows(2).all(|pair| (pair[0].at.is_none(), pair[0].at) <= (pair[1].at.is_none(), pair[1].at)), "in the order they are told");
        println!("the catalog at hand names: {:#?}", told(&all));
    }

    #[test]
    fn a_week_without_lectures_that_is_not_christmas() {
        let list = notices(d("2026-05-01"), None, &[semester("2026S", Some(("2026-04-13", "2026-07-26")), &[("2026-05-25", "2026-05-31"), ("2026-06-15", "2026-06-28")], &[])]);
        assert_eq!(told(&list), ["breaks-2026s-1 2026-05-22 15:00", "breaks-2026s-2 2026-06-12 15:00"]);
        assert_eq!(list[0].what, What::Break { nr: 1, weeks: 1, resume: d("2026-06-01"), christmas: false });
        assert_eq!(list[1].what, What::Break { nr: 2, weeks: 2, resume: d("2026-06-29"), christmas: false });
    }
}
