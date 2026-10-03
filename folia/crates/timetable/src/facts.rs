//! A semester's lecture period, breaks, A/B weeks and holidays, derived from its own data.
//!
//! No source states when a semester's lectures run. `v_semester` has the half-year (01.10.–31.03.),
//! QIS leaves the „Dauer" of hundreds of recurring rows empty, the Christmas break is in no column,
//! and „A" and „B" name weeks without saying which is which. The dates the rows do state say all
//! of it, counted per rhythm and range (`queries::semester_date_counts`):
//!
//! - the lecture period runs from the week most weekly rows begin in to the week most of them end
//!   in (2026W: 05.10.2026–31.01.2027);
//! - a break is a week inside it with next to no single dates (2026W: 0 and 5 against a median of
//!   70, so 21.12.–03.01.);
//! - the A weeks are the weeks the A rows begin in, counted from the period's first week, so the
//!   53-week year 2026 cannot flip them.
//!
//! Each rule needs enough rows and a clear majority, else it says nothing (`None`, no break), and
//! the page names what is unknown instead of guessing (R12). Holidays are Brandenburg's law
//! (`day::holidays`), not a reading of the data.

use std::collections::BTreeMap;

use folia_calendar::day::{self, Day, Holiday};
use folia_calendar::semester::SemesterKey;
use folia_model::labels::{Code, Rhythm};
use folia_model::rows::Semester;
use folia_model::rows_detail::DateCount;

use crate::clash::Weeks;

/// The share of the weekly rows that must begin in the first week, and end in the last one, for
/// those weeks to be the lecture period.
pub const LECTURE_MODE_SHARE: f64 = 0.4;
/// The fewest weekly rows with a range that make a lecture period.
pub const LECTURE_MIN_ROWS: i64 = 20;
/// A week is a break when it has fewer single dates than the median week divided by this.
pub const BREAK_DIVISOR: i64 = 5;
/// The fewest single dates of the median week for a quiet week to mean anything.
pub const BREAK_MIN_MEDIAN: i64 = 20;
/// The share of the A and B rows that must agree on which weeks are A weeks.
pub const AB_SHARE: f64 = 0.8;
/// The fewest A and B rows with a date that anchor the A weeks.
pub const AB_MIN_ROWS: i64 = 10;

/// The longest half-year `v_semester` may state before its own bounds are not believed: a
/// semester is half a year, and every loop over its weeks and years stays short.
const MAX_BOUNDS_DAYS: i64 = 366;

/// What the dates of one semester say about its weeks.
#[derive(Clone, Debug, PartialEq)]
pub struct SemesterFacts {
    pub key: SemesterKey,
    /// `v_semester`'s `starts_on..=ends_on`, else the half-year of the key.
    pub bounds: (Day, Day),
    /// Monday of the first lecture week ..= Sunday of the last; `None` when the data does not
    /// say. Always inside the weeks of `bounds`: the half-year may begin or end within the
    /// period's first or last week (2026S begins on a Wednesday), so a day of the period is not
    /// always a day of the semester.
    pub lecture: Option<(Day, Day)>,
    /// Weeks without lectures inside the period, Monday ..= Sunday, consecutive weeks merged.
    pub breaks: Vec<(Day, Day)>,
    /// Monday of the period's first A week; `None` when the A and B rows do not agree.
    pub a_week: Option<Day>,
    /// Brandenburg's public holidays inside `bounds`, in date order.
    pub holidays: Vec<(Day, Holiday)>,
}

impl SemesterFacts {
    /// The facts of `key` from its row counts. `semester` is its `v_semester` row when the
    /// snapshot has one (not for a semester without data, like the next summer).
    pub fn derive(key: SemesterKey, semester: Option<&Semester>, counts: &[DateCount]) -> Self {
        let bounds = semester.and_then(|s| stated_bounds(key, s)).unwrap_or_else(|| key.bounds());
        // A period outside the half-year is not one the semester's rows can mean. The period is
        // whole weeks and the half-year is not, so their weeks are compared: lectures from
        // Wednesday 01.04. make a period from Monday 30.03., which is still this summer's.
        let weeks = (bounds.0.monday(), bounds.1.monday().plus(6));
        let lecture = lecture_period(counts).filter(|(first, last)| weeks.0 <= *first && *last <= weeks.1);
        let breaks = lecture.map(|period| breaks(period, counts)).unwrap_or_default();
        let a_week = lecture.and_then(|(first, _)| a_week(first, counts));
        let (first_year, _, _) = bounds.0.ymd();
        let (last_year, _, _) = bounds.1.ymd();
        let holidays = (first_year..=last_year)
            .flat_map(day::holidays)
            .filter(|(day, _)| bounds.0 <= *day && *day <= bounds.1)
            .collect();
        Self { key, bounds, lecture, breaks, a_week, holidays }
    }

    /// Whether the week of `day` is an A or a B week (`Weeks::A`, `Weeks::B`): a week of the lecture
    /// period outside its breaks, counted from the first A week by the calendar, as the A and B rows
    /// are held (`occur`). `None` outside the period, in a break, and where the A weeks are unknown.
    pub fn ab_week(&self, day: Day) -> Option<Weeks> {
        let (first, last) = self.lecture?;
        if day < first || day > last || self.in_break(day) {
            return None;
        }
        let weeks = (day.monday().0 - self.a_week?.monday().0).div_euclid(7);
        Some(if weeks.rem_euclid(2) == 0 { Weeks::A } else { Weeks::B })
    }

    /// Whether `day` lies in a break.
    pub fn in_break(&self, day: Day) -> bool {
        self.breaks.iter().any(|(first, last)| *first <= day && day <= *last)
    }

    /// The public holiday on `day`, if it is one (`Holiday::name` names it). A semester's rows
    /// reach past its half-year (145750's „Block+SaSo" of 2026S runs to 03.10.), so a day outside
    /// `bounds` is looked up in the law of its year rather than taken as a working day.
    pub fn holiday(&self, day: Day) -> Option<Holiday> {
        let named = |holidays: &[(Day, Holiday)]| holidays.iter().find(|(date, _)| *date == day).map(|(_, holiday)| *holiday);
        if self.bounds.0 <= day && day <= self.bounds.1 {
            named(&self.holidays)
        } else {
            named(&day::holidays(day.ymd().0))
        }
    }
}

/// The half-year `v_semester` states for `key`, when it is that semester's row and a sane range.
fn stated_bounds(key: SemesterKey, semester: &Semester) -> Option<(Day, Day)> {
    if SemesterKey::parse(&semester.key) != Some(key) {
        return None;
    }
    let first = Day::parse(&semester.starts_on)?;
    let last = Day::parse(&semester.ends_on)?;
    let days = i64::from(last.0) - i64::from(first.0);
    (0..=MAX_BOUNDS_DAYS).contains(&days).then_some((first, last))
}

/// The weeks most weekly rows begin and end in, weighted by their rows (a group of the counts
/// holds every row of one range). On a tie the earlier start and the later end win: the wider
/// period assumes a date too many rather than one too few.
fn lecture_period(counts: &[DateCount]) -> Option<(Day, Day)> {
    let mut starts: BTreeMap<Day, i64> = BTreeMap::new();
    let mut ends: BTreeMap<Day, i64> = BTreeMap::new();
    let mut total = 0i64;
    for count in counts.iter().filter(|count| is_rhythm(count, Rhythm::Weekly)) {
        let (Some(first), Some(last)) =
            (Day::parse(&count.first_date), count.last_date.as_deref().and_then(Day::parse))
        else {
            continue;
        };
        if last < first {
            continue;
        }
        let rows = weight(count);
        add(&mut starts, first.monday(), rows);
        add(&mut ends, last.monday(), rows);
        total = total.saturating_add(rows);
    }
    let (first, first_rows) = heaviest(starts.iter())?;
    let (last, last_rows) = heaviest(ends.iter().rev())?;
    let clear = |rows: i64| rows as f64 >= LECTURE_MODE_SHARE * total as f64;
    (total >= LECTURE_MIN_ROWS && clear(first_rows) && clear(last_rows) && first < last).then(|| (first, last.plus(6)))
}

/// The weeks of the period with next to no single dates. Singles are what a lecturer adds to a
/// week; a week without them is one nobody teaches in, while the weekly rows run straight through
/// the break. The first and the last week are never breaks: a period starts and ends taught.
fn breaks(period: (Day, Day), counts: &[DateCount]) -> Vec<(Day, Day)> {
    let (start, end) = period;
    let weeks = usize::try_from((i64::from(end.0) - i64::from(start.0) + 1) / 7).unwrap_or(0);
    let mut singles = vec![0i64; weeks];
    for count in counts.iter().filter(|count| is_rhythm(count, Rhythm::Single)) {
        let Some(day) = Day::parse(&count.first_date).filter(|day| start <= *day && *day <= end) else {
            continue;
        };
        let week = usize::try_from((i64::from(day.monday().0) - i64::from(start.0)) / 7).ok();
        if let Some(slot) = week.and_then(|week| singles.get_mut(week)) {
            *slot = slot.saturating_add(weight(count));
        }
    }
    let mut sorted = singles.clone();
    sorted.sort_unstable();
    // The lower median: of two middle weeks the quieter one, so fewer weeks fall under its fifth.
    // In doubt a week counts as taught, which shows a date rather than hiding it.
    let Some(&median) = sorted.get(sorted.len().saturating_sub(1) / 2) else {
        return Vec::new();
    };
    if median < BREAK_MIN_MEDIAN {
        return Vec::new();
    }
    let last_week = singles.len().saturating_sub(1);
    let mut found: Vec<(Day, Day)> = Vec::new();
    let mut monday = start;
    for (week, rows) in singles.iter().enumerate() {
        // Fewer than a fifth of the median, without rounding the fifth.
        if week > 0 && week < last_week && rows.saturating_mul(BREAK_DIVISOR) < median {
            match found.last_mut() {
                Some(open) if open.1.plus(1) == monday => open.1 = monday.plus(6),
                _ => found.push((monday, monday.plus(6))),
            }
        }
        monday = monday.plus(7);
    }
    found
}

/// The Monday of the first A week, from the weeks the dated A and B rows begin in. A row that
/// begins in the period's first week (or any week an even number of weeks after it) puts the A
/// weeks there if it is an A row, and the B weeks if it is a B row.
fn a_week(start: Day, counts: &[DateCount]) -> Option<Day> {
    // Rows that put the first A week on the period's first week, and on its second.
    let (mut first, mut second) = (0i64, 0i64);
    for count in counts {
        let shift = match count.rhythm.as_ref().and_then(Code::known) {
            Some(Rhythm::WeekA) => 0,
            Some(Rhythm::WeekB) => 1,
            _ => continue,
        };
        let Some(day) = Day::parse(&count.first_date) else {
            continue;
        };
        let weeks = (i64::from(day.monday().0) - i64::from(start.0)).div_euclid(7);
        let rows = weight(count);
        if (weeks + shift).rem_euclid(2) == 0 {
            first = first.saturating_add(rows);
        } else {
            second = second.saturating_add(rows);
        }
    }
    let total = first.saturating_add(second);
    let (offset, agreeing) = if first >= second { (0, first) } else { (7, second) };
    (total >= AB_MIN_ROWS && agreeing as f64 >= AB_SHARE * total as f64).then(|| start.plus(offset))
}

/// The heaviest week of the iteration; of equal weights the first seen.
fn heaviest<'a>(weeks: impl Iterator<Item = (&'a Day, &'a i64)>) -> Option<(Day, i64)> {
    weeks.fold(None, |best, (day, rows)| match best {
        Some((_, top)) if top >= *rows => best,
        _ => Some((*day, *rows)),
    })
}

fn add(weeks: &mut BTreeMap<Day, i64>, monday: Day, rows: i64) {
    let entry = weeks.entry(monday).or_default();
    *entry = entry.saturating_add(rows);
}

fn is_rhythm(count: &DateCount, rhythm: Rhythm) -> bool {
    count.rhythm.as_ref().is_some_and(|code| code.is(rhythm))
}

/// The rows a count stands for; a count below zero is no evidence.
fn weight(count: &DateCount) -> i64 {
    count.dates.max(0)
}

#[cfg(test)]
mod tests {
    use folia_locale::Locale;
    use folia_query as queries;

    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn winter() -> SemesterKey {
        SemesterKey::parse("2026W").unwrap()
    }

    fn count(rhythm: &str, first: &str, last: &str, dates: i64) -> DateCount {
        DateCount {
            rhythm: Some(Code::parse(rhythm)),
            first_date: first.to_string(),
            last_date: Some(last.to_string()),
            dates,
        }
    }

    /// A winter like 2026W: 30 weekly rows over the whole period and 5 that start a week late and
    /// end a week early; 40 single dates a week, none in the week of 21.12. and 3 in the next.
    fn winter_counts() -> Vec<DateCount> {
        let mut counts =
            vec![count("weekly", "2026-10-05", "2027-01-25", 30), count("weekly", "2026-10-12", "2027-01-18", 5)];
        let mut monday = d("2026-10-05");
        while monday <= d("2027-01-25") {
            let singles = match monday.iso().as_str() {
                "2026-12-21" => 0,
                "2026-12-28" => 3,
                _ => 40,
            };
            if singles > 0 {
                let tuesday = monday.plus(1).iso();
                counts.push(count("single", &tuesday, &tuesday, singles));
            }
            monday = monday.plus(7);
        }
        counts
    }

    #[test]
    fn the_period_breaks_and_a_weeks_of_a_winter() {
        let mut counts = winter_counts();
        // A rows begin in odd weeks of the period, B rows in even ones: the A weeks start at once.
        counts.push(count("week_a", "2026-10-06", "2027-01-26", 8));
        counts.push(count("week_a", "2026-10-20", "2027-01-26", 1));
        counts.push(count("week_b", "2026-10-13", "2027-01-19", 4));
        let facts = SemesterFacts::derive(winter(), None, &counts);
        assert_eq!(facts.bounds, (d("2026-10-01"), d("2027-03-31")));
        assert_eq!(facts.lecture, Some((d("2026-10-05"), d("2027-01-31"))));
        assert_eq!(facts.breaks, vec![(d("2026-12-21"), d("2027-01-03"))]);
        assert_eq!(facts.a_week, Some(d("2026-10-05")));
        assert!(facts.in_break(d("2026-12-21")) && facts.in_break(d("2027-01-03")));
        assert!(!facts.in_break(d("2026-12-20")) && !facts.in_break(d("2027-01-04")));
        // A and B weeks by the calendar from the first A week; none in the break or outside the
        // period.
        let ab = |day: &str| facts.ab_week(d(day));
        assert_eq!((ab("2026-10-05"), ab("2026-10-11"), ab("2026-10-13")), (Some(Weeks::A), Some(Weeks::A), Some(Weeks::B)));
        assert_eq!((ab("2026-12-22"), ab("2027-01-04"), ab("2027-02-01"), ab("2026-10-04")), (None, Some(Weeks::B), None, None));
        // The rows put the A weeks one week later: the anchor follows them.
        let mut later = winter_counts();
        later.push(count("week_a", "2026-10-13", "2027-01-19", 9));
        later.push(count("week_b", "2026-10-06", "2027-01-26", 3));
        assert_eq!(SemesterFacts::derive(winter(), None, &later).a_week, Some(d("2026-10-12")));
    }

    #[test]
    fn the_holidays_of_the_half_year() {
        let facts = SemesterFacts::derive(winter(), None, &[]);
        let names: Vec<(String, &str)> = facts.holidays.iter().map(|(day, holiday)| (day.iso(), holiday.name(Locale::De))).collect();
        assert_eq!(
            names,
            [
                ("2026-10-03".to_string(), "Tag der Deutschen Einheit"),
                ("2026-10-31".to_string(), "Reformationstag"),
                ("2026-12-25".to_string(), "1. Weihnachtstag"),
                ("2026-12-26".to_string(), "2. Weihnachtstag"),
                ("2027-01-01".to_string(), "Neujahr"),
                ("2027-03-26".to_string(), "Karfreitag"),
                ("2027-03-28".to_string(), "Ostersonntag"),
                ("2027-03-29".to_string(), "Ostermontag"),
            ]
        );
        let holiday = |day: &str, locale: Locale| facts.holiday(d(day)).map(|holiday| holiday.name(locale));
        assert_eq!(holiday("2026-10-31", Locale::De), Some("Reformationstag"));
        assert_eq!(holiday("2026-10-31", Locale::En), Some("Reformation Day"));
        assert_eq!(holiday("2026-11-01", Locale::De), None);
        // Neujahr 2026 and Christi Himmelfahrt 2027 lie outside the winter: not in its list, but a
        // row that reaches them is still told they are holidays.
        assert_eq!(holiday("2026-01-01", Locale::De), Some("Neujahr"));
        assert_eq!(holiday("2027-05-06", Locale::De), Some("Christi Himmelfahrt"));
        assert_eq!(holiday("2027-05-06", Locale::En), Some("Ascension Day"));
        assert_eq!(holiday("2027-05-07", Locale::De), None);
    }

    #[test]
    fn the_period_is_compared_with_the_weeks_of_the_half_year() {
        let summer = SemesterKey::parse("2026S").unwrap();
        // 2026S runs from Wednesday 01.04. to Wednesday 30.09. Lectures from the first day to the
        // last fill the first and the last week, so the period is those whole weeks.
        let wednesdays = [count("weekly", "2026-04-01", "2026-09-30", 30)];
        let facts = SemesterFacts::derive(summer, None, &wednesdays);
        assert_eq!(facts.bounds, (d("2026-04-01"), d("2026-09-30")));
        assert_eq!(facts.lecture, Some((d("2026-03-30"), d("2026-10-04"))));
        // A week earlier or a week later is another half-year's.
        let early = [count("weekly", "2026-03-27", "2026-07-15", 30)];
        assert_eq!(SemesterFacts::derive(summer, None, &early).lecture, None);
        let late = [count("weekly", "2026-04-01", "2026-10-05", 30)];
        assert_eq!(SemesterFacts::derive(summer, None, &late).lecture, None);
    }

    #[test]
    fn too_little_or_unclear_data_says_nothing() {
        // No data at all: the half-year and its holidays only.
        let empty = SemesterFacts::derive(winter(), None, &[]);
        assert_eq!((empty.lecture, empty.breaks.len(), empty.a_week), (None, 0, None));
        assert!(!empty.in_break(d("2026-12-24")));
        // Fewer than 20 weekly rows.
        let few = [count("weekly", "2026-10-05", "2027-01-25", 19)];
        assert_eq!(SemesterFacts::derive(winter(), None, &few).lecture, None);
        let enough = [count("weekly", "2026-10-05", "2027-01-25", 20)];
        assert_eq!(SemesterFacts::derive(winter(), None, &enough).lecture, Some((d("2026-10-05"), d("2027-01-31"))));
        // No first week with 40 % of the rows.
        let spread = [
            count("weekly", "2026-10-05", "2027-01-25", 13),
            count("weekly", "2026-10-12", "2027-01-25", 13),
            count("weekly", "2026-10-19", "2027-01-25", 14),
        ];
        assert_eq!(SemesterFacts::derive(winter(), None, &spread).lecture, None);
        // A last week before the first one is no period.
        let backwards =
            [count("weekly", "2026-12-07", "2026-12-07", 30), count("weekly", "2026-10-05", "2026-12-07", 1)];
        assert_eq!(SemesterFacts::derive(winter(), None, &backwards).lecture, None);
        // Rows of another half-year do not make this one's period.
        let summer = [count("weekly", "2026-04-13", "2026-07-20", 40)];
        assert_eq!(SemesterFacts::derive(winter(), None, &summer).lecture, None);
        // Only weekly rows count for the period, and a row without an end is none.
        let singles = [count("single", "2026-10-05", "2026-10-05", 50), count("block", "2026-10-05", "2027-01-25", 50)];
        assert_eq!(SemesterFacts::derive(winter(), None, &singles).lecture, None);
        let open = [DateCount {
            rhythm: Some(Code::parse("weekly")),
            first_date: "2026-10-05".into(),
            last_date: None,
            dates: 50,
        }];
        assert_eq!(SemesterFacts::derive(winter(), None, &open).lecture, None);
    }

    #[test]
    fn ties_widen_the_period() {
        let tied = [count("weekly", "2026-10-12", "2027-01-18", 10), count("weekly", "2026-10-05", "2027-01-25", 10)];
        assert_eq!(SemesterFacts::derive(winter(), None, &tied).lecture, Some((d("2026-10-05"), d("2027-01-31"))));
    }

    #[test]
    fn a_quiet_week_is_a_break_only_inside_a_busy_period() {
        let mut counts = winter_counts();
        // The first and the last week without singles: never breaks.
        counts.retain(|c| {
            let day = d(&c.first_date);
            c.rhythm.as_ref().is_some_and(|r| r.code() != "single")
                || !(day <= d("2026-10-11") || day >= d("2027-01-25"))
        });
        let facts = SemesterFacts::derive(winter(), None, &counts);
        assert_eq!(facts.breaks, vec![(d("2026-12-21"), d("2027-01-03"))]);
        // A median under 20 singles says nothing about quiet weeks.
        let sparse: Vec<DateCount> = winter_counts()
            .into_iter()
            .map(|c| {
                if c.rhythm.as_ref().is_some_and(|r| r.code() == "single") {
                    DateCount { dates: c.dates / 5, ..c }
                } else {
                    c
                }
            })
            .collect();
        assert!(SemesterFacts::derive(winter(), None, &sparse).breaks.is_empty());
        // A week with exactly a fifth of the median is taught: 8 × 5 = 40 is not below 40.
        let mut fifth = winter_counts();
        fifth.push(count("single", "2026-12-22", "2026-12-22", 8));
        assert_eq!(SemesterFacts::derive(winter(), None, &fifth).breaks, vec![(d("2026-12-28"), d("2027-01-03"))]);
    }

    #[test]
    fn a_weeks_need_many_rows_that_agree() {
        let with = |extra: &[DateCount]| {
            let mut counts = winter_counts();
            counts.extend_from_slice(extra);
            SemesterFacts::derive(winter(), None, &counts).a_week
        };
        assert_eq!(with(&[count("week_a", "2026-10-06", "2027-01-26", 9)]), None, "9 rows");
        assert_eq!(with(&[count("week_a", "2026-10-06", "2027-01-26", 10)]), Some(d("2026-10-05")));
        // 8 of 10 agree: enough. 7 of 10: not.
        assert_eq!(
            with(&[count("week_a", "2026-10-06", "2027-01-26", 8), count("week_b", "2026-10-06", "2027-01-26", 2)]),
            Some(d("2026-10-05"))
        );
        assert_eq!(
            with(&[count("week_a", "2026-10-06", "2027-01-26", 7), count("week_b", "2026-10-06", "2027-01-26", 3)]),
            None
        );
        // A row that begins before the period still counts its weeks from the period's start.
        assert_eq!(with(&[count("week_b", "2026-09-29", "2027-01-19", 10)]), Some(d("2026-10-05")));
        // Without a period there is nothing to count from.
        assert_eq!(
            SemesterFacts::derive(winter(), None, &[count("week_a", "2026-10-06", "2027-01-26", 50)]).a_week,
            None
        );
    }

    #[test]
    fn the_stated_half_year_is_used_when_it_is_this_semesters() {
        let semester = |key: &str, starts: &str, ends: &str| Semester {
            key: key.into(),
            season: Code::parse("winter"),
            year: 2026,
            label: "WiSe 2026/27".into(),
            starts_on: starts.into(),
            ends_on: ends.into(),
            is_current: true,
            teaching_events: 0,
            exam_events: 0,
        };
        let stated = semester("2026W", "2026-10-01", "2027-03-15");
        assert_eq!(SemesterFacts::derive(winter(), Some(&stated), &[]).bounds, (d("2026-10-01"), d("2027-03-15")));
        // Another semester's row, an unreadable date, a range backwards or of years: the key's half-year.
        for other in [
            semester("2026S", "2026-04-01", "2026-09-30"),
            semester("2026W", "01.10.2026", "2027-03-31"),
            semester("2026W", "2027-03-31", "2026-10-01"),
            semester("2026W", "2026-10-01", "2030-03-31"),
        ] {
            assert_eq!(SemesterFacts::derive(winter(), Some(&other), &[]).bounds, winter().bounds(), "{other:?}");
        }
        // The period must lie inside the stated half-year.
        let short = semester("2026W", "2026-10-01", "2027-01-15");
        assert_eq!(SemesterFacts::derive(winter(), Some(&short), &winter_counts()).lecture, None);
    }

    /// The pinned snapshot's two semesters as the design quotes them; on any snapshot, what holds
    /// by construction.
    #[test]
    fn semester_facts_of_the_winter() {
        let pinned = folia_test_support::studyplan_db("semester_facts_of_the_winter");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(folia_test_support::open);
        let semesters = queries::semesters(&db).unwrap();
        assert!(!semesters.is_empty());
        let mut derived = BTreeMap::new();
        for semester in &semesters {
            let key = SemesterKey::parse(&semester.key).unwrap();
            let counts = queries::semester_date_counts(&db, &semester.key).unwrap();
            let facts = SemesterFacts::derive(key, Some(semester), &counts);
            let (first, last) = facts.bounds;
            assert_eq!((first.iso(), last.iso()), (semester.starts_on.clone(), semester.ends_on.clone()));
            match facts.lecture {
                Some((start, end)) => {
                    // Whole weeks against a half-year that may begin and end mid-week.
                    let inside = first.monday() <= start && end <= last.monday().plus(6);
                    assert!(inside, "{}: lecture inside the weeks of the bounds", semester.key);
                    assert_eq!((start.weekday(), end.weekday()), (1, 7));
                    for (from, to) in &facts.breaks {
                        assert!(start < *from && *to < end && from < to, "{}: break inside the period", semester.key);
                        assert_eq!((from.weekday(), to.weekday()), (1, 7));
                    }
                    if let Some(a) = facts.a_week {
                        assert!(start <= a && a <= end && a.weekday() == 1);
                    }
                }
                None => assert!(facts.breaks.is_empty() && facts.a_week.is_none()),
            }
            assert!(facts.holidays.iter().all(|(day, _)| first <= *day && *day <= last));
            assert!(facts.holidays.windows(2).all(|pair| pair[0].0 <= pair[1].0));
            derived.insert(semester.key.clone(), facts);
        }
        if !is_pinned {
            return;
        }
        let winter = &derived["2026W"];
        assert_eq!(winter.lecture, Some((d("2026-10-05"), d("2027-01-31"))));
        assert_eq!(winter.breaks, vec![(d("2026-12-21"), d("2027-01-03"))]);
        assert_eq!(winter.a_week, Some(d("2026-10-05")));
        let summer = &derived["2026S"];
        assert_eq!(summer.lecture, Some((d("2026-04-13"), d("2026-07-26"))));
        assert!(summer.breaks.is_empty(), "too few singles in the summer for a break");
        assert_eq!(summer.a_week, None, "6 A/B rows in the summer");
    }
}
