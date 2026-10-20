//! The semester clock under the greeting (concept A, the third round; owner, 2026-10-20: „Ich
//! fande die Wochen Elemente davor schon viel besser. Mach mal nur die aktuelle woche breiter mit
//! der in week progress bar und den rest gleich groß. Ja und den Prüfungszeitraum halt wie am
//! anfang als gestrichelte bar, die dann erst in der Prüfungszeit sich expandiert und wieder eine
//! week preview wird"): a piece per week of the lecture period, all of one size and filled once
//! they are over; the running week wider, filling up to the hour; the weeks of a break hollow; the
//! rest of the semester after the lectures one hatched bar, which opens into its weeks only once
//! the lectures are over, a dot over each week with an exam of the visitor's. After the semester
//! all of it is full.
//!
//! Worked out from the semester's facts (`SemesterFacts`: the lecture period, its breaks and A
//! weeks, as the dates of the rows say them) and the visitor's exams; without a lecture period the
//! data does not say, there is no clock (R12).

use folia_calendar::day::Day;
use folia_timetable::clash::Weeks;
use folia_timetable::facts::SemesterFacts;

/// Where the semester stands on a day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The semester has begun, its lectures have not (the first days of October).
    Before,
    /// The lecture period, its breaks excepted.
    Lectures,
    /// A break of the lecture period (Christmas).
    Break,
    /// After the lectures to the semester's end: the time of the exams.
    Exams,
    /// After the semester's end, while the catalog's current semester is still this one.
    Over,
}

/// The phase of `day` in the semester of `facts`. Without a lecture period the data does not say,
/// the semester's own days count as its lectures.
pub fn phase(facts: &SemesterFacts, day: Day) -> Phase {
    let (first, last) = facts.bounds;
    if day > last {
        return Phase::Over;
    }
    match facts.lecture {
        Some((start, _)) if day < start => Phase::Before,
        Some((_, end)) if day > end => Phase::Exams,
        Some(_) if facts.in_break(day) => Phase::Break,
        Some(_) => Phase::Lectures,
        None if day < first => Phase::Before,
        None => Phase::Lectures,
    }
}

/// What a piece of the clock is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A week of lectures.
    Lecture,
    /// A week of a break.
    Pause,
    /// A week after the lectures, while that time runs.
    ExamWeek,
    /// The time after the lectures as one bar, before and after it runs.
    Exams,
}

/// One piece of the clock: a week, or the time of the exams.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub kind: Kind,
    /// Its first and last day: Monday and Sunday of a week.
    pub from: Day,
    pub to: Day,
    /// Over.
    pub done: bool,
    /// The running week: how much of it is gone, 0–1, to the hour.
    pub fill: Option<f64>,
    /// An exam of the visitor's lies in it.
    pub exam: bool,
}

/// The clock of a day.
#[derive(Clone, Debug, PartialEq)]
pub struct Clock {
    pub phase: Phase,
    pub pieces: Vec<Piece>,
    /// The weeks of lectures (a break's not counted): the one running or the last one begun, and
    /// how many there are („Woche 3 von 15").
    pub week: usize,
    pub weeks: usize,
    /// The week of `day` is an A or a B week.
    pub ab: Option<Weeks>,
    /// The lecture period, Monday of its first week to Sunday of its last.
    pub lectures: (Day, Day),
    /// The break `day` lies in: its first and last day, and whether it is Christmas's.
    pub pause: Option<(Day, Day, bool)>,
    /// The semester's last day.
    pub end: Day,
    /// The visitor's exams of the semester, by day: all of them, and those from `day` on.
    pub exams: Vec<Day>,
    pub exams_left: Vec<Day>,
}

/// The clock of `day` at `minute` (since midnight) in the semester of `facts`, with the days of the
/// visitor's exams. `None` where the data names no lecture period.
pub fn clock(facts: &SemesterFacts, day: Day, minute: u16, exams: &[Day]) -> Option<Clock> {
    let (start, end_of_lectures) = facts.lecture?;
    let end = facts.bounds.1;
    let mut exams: Vec<Day> = exams.to_vec();
    exams.sort_unstable();
    exams.dedup();
    let part = f64::from(minute.min(1440)) / 1440.0;
    let week_of = |kind: Kind, from: Day, to: Day| {
        let running = from <= day && day <= to;
        Piece {
            kind,
            from,
            to,
            done: day > to,
            fill: running.then(|| ((f64::from(day.0 - from.0) + part) / 7.0).clamp(0.0, 1.0)),
            exam: exams.iter().any(|exam| from <= *exam && *exam <= to),
        }
    };
    let mut pieces = Vec::new();
    let mut monday = start.monday();
    while monday <= end_of_lectures && pieces.len() < 40 {
        let kind = if facts.in_break(monday) { Kind::Pause } else { Kind::Lecture };
        pieces.push(week_of(kind, monday, monday.plus(6)));
        monday = monday.plus(7);
    }
    let lectures_done = monday;
    let phase = phase(facts, day);
    if lectures_done <= end {
        if phase == Phase::Exams {
            let mut monday = lectures_done;
            while monday <= end && pieces.len() < 80 {
                pieces.push(week_of(Kind::ExamWeek, monday, monday.plus(6)));
                monday = monday.plus(7);
            }
        } else {
            pieces.push(Piece { kind: Kind::Exams, from: lectures_done, to: end, done: day > end, fill: None, exam: false });
        }
    }
    let lecture_weeks: Vec<&Piece> = pieces.iter().filter(|piece| piece.kind == Kind::Lecture).collect();
    let weeks = lecture_weeks.len();
    let week = lecture_weeks.iter().filter(|piece| piece.from <= day).count();
    let pause = facts.breaks.iter().find(|(first, last)| *first <= day && day <= *last).map(|(first, last)| {
        let christmas = |month: u32, date: u32| (first.0..=last.0).any(|n| matches!(Day(n).ymd(), (_, m, d) if m == month && d == date));
        (*first, *last, christmas(12, 24) || christmas(1, 1))
    });
    let exams_left = exams.iter().copied().filter(|exam| *exam >= day).collect();
    Some(Clock { phase, pieces, week, weeks, ab: facts.ab_week(day), lectures: (start, end_of_lectures), pause, end, exams, exams_left })
}

#[cfg(test)]
mod tests {
    use folia_calendar::semester::SemesterKey;

    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    /// The winter of 2026/27 as its dates say it: lectures from 05.10.2026 to 31.01.2027, the break
    /// from 21.12. to 03.01., A weeks from the first.
    fn winter() -> SemesterFacts {
        SemesterFacts {
            key: SemesterKey::parse("2026W").unwrap(),
            bounds: (d("2026-10-01"), d("2027-03-31")),
            lecture: Some((d("2026-10-05"), d("2027-01-31"))),
            breaks: vec![(d("2026-12-21"), d("2027-01-03"))],
            a_week: Some(d("2026-10-05")),
            holidays: Vec::new(),
        }
    }

    const EXAMS: [&str; 3] = ["2027-02-11", "2027-03-10", "2027-03-12"];

    fn exams() -> Vec<Day> {
        EXAMS.iter().map(|day| d(day)).collect()
    }

    /// The prototype's Tuesday at 12:40: the third week of fifteen, an A week, a third of it gone;
    /// two weeks before it filled, the two of the break hollow, the exams one bar ahead.
    #[test]
    fn a_tuesday_in_october() {
        let clock = clock(&winter(), d("2026-10-20"), 12 * 60 + 40, &exams()).unwrap();
        assert_eq!((clock.phase, clock.week, clock.weeks, clock.ab), (Phase::Lectures, 3, 15, Some(Weeks::A)));
        let kinds: Vec<Kind> = clock.pieces.iter().map(|piece| piece.kind).collect();
        assert_eq!(kinds.len(), 18, "17 weeks of lectures and the bar");
        assert_eq!((kinds[11], kinds[12], kinds[17]), (Kind::Pause, Kind::Pause, Kind::Exams));
        assert_eq!(clock.pieces.iter().filter(|piece| piece.done).count(), 2);
        let fill = clock.pieces[2].fill.unwrap();
        assert!((fill - (1.0 + 12.0 / 24.0 + 40.0 / 1440.0) / 7.0).abs() < 1e-9, "{fill}");
        assert!(clock.pieces.iter().all(|piece| !piece.exam), "the bar carries no dots");
        assert_eq!((clock.exams.len(), clock.exams_left.len()), (3, 3));
    }

    /// In the break the clock says which; the weeks before it are over.
    #[test]
    fn christmas() {
        let clock = clock(&winter(), d("2026-12-23"), 600, &exams()).unwrap();
        assert_eq!((clock.phase, clock.pause.map(|(_, last, christmas)| (last, christmas))), (Phase::Break, Some((d("2027-01-03"), true))));
        assert_eq!(clock.pieces.iter().filter(|piece| piece.kind == Kind::Lecture && piece.done).count(), 11);
        assert_eq!(clock.ab, None, "no A or B week in a break");
    }

    /// Once the lectures are over, the bar opens into its weeks, a dot over each with an exam.
    #[test]
    fn the_exams_open_into_weeks() {
        let clock = clock(&winter(), d("2027-02-08"), 9 * 60, &exams()).unwrap();
        assert_eq!(clock.phase, Phase::Exams);
        let weeks: Vec<&Piece> = clock.pieces.iter().filter(|piece| piece.kind == Kind::ExamWeek).collect();
        assert_eq!(weeks.len(), 9, "01.02. to 31.03.");
        assert!(weeks[1].fill.is_some() && weeks[1].exam, "the week of 08.02. runs and holds the exam of 11.02.");
        assert_eq!(weeks.iter().filter(|week| week.exam).count(), 2, "10.03. and 12.03. share a week");
        assert!(clock.pieces.iter().filter(|piece| piece.kind == Kind::Lecture).all(|piece| piece.done));
        assert_eq!(clock.exams_left.len(), 3);
    }

    /// After the semester everything is full; before its lectures nothing is.
    #[test]
    fn before_and_after() {
        let over = clock(&winter(), d("2027-04-01"), 600, &exams()).unwrap();
        assert_eq!(over.phase, Phase::Over);
        assert!(over.pieces.iter().all(|piece| piece.done) && over.pieces.last().map(|piece| piece.kind) == Some(Kind::Exams));
        assert!(over.exams_left.is_empty());
        let before = clock(&winter(), d("2026-10-02"), 600, &[]).unwrap();
        assert_eq!((before.phase, before.week), (Phase::Before, 0));
        assert!(before.pieces.iter().all(|piece| !piece.done && piece.fill.is_none()));
        let unknown = SemesterFacts { lecture: None, breaks: Vec::new(), ..winter() };
        assert!(clock(&unknown, d("2026-10-20"), 0, &[]).is_none(), "no lecture period, no clock");
        assert_eq!(phase(&unknown, d("2026-10-20")), Phase::Lectures);
    }
}
