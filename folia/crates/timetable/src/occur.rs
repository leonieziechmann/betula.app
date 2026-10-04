//! Every date of a row, from its rhythm, its range and the exceptions of its note.
//!
//! A date row of QIS says „Mo 15:30–17:00, wöchentlich, 12.10.2026 bis 25.01.2027" and, in another
//! column, which dates „fallen aus". The days follow from the rhythm and the range. Where QIS leaves
//! the range empty, the semester's lecture period stands in, and the row says so (`assumed`); where
//! there is no period either, or nothing anchors a four-week rhythm, the row keeps its pattern
//! (`template`) and names no day. Then the exceptions: a recurring day in the break or on a public
//! holiday is skipped (a single date never is: QIS lists that day on purpose, 147958 teaches on
//! Neujahr), a cancelled date is set apart with its reason, and a room note stays with its held
//! day. No date is invented: a note that names a replacement only says so (`cancel`).

use folia_calendar::cancel;
use folia_calendar::day::{minutes, Day, Holiday};
use folia_calendar::kind::fold;
use folia_locale::Locale;
use folia_model::labels::{Code, Rhythm};
use folia_model::rows_detail::{DateRow, EventDate};

use crate::facts::SemesterFacts;

/// The most dates walked for one row. No real row comes near it; a block that runs for years
/// (147307's, 2026 to 2028) stops here instead of filling a calendar.
pub const MAX_OCCURRENCES: usize = 400;

/// Why a recurring date is not held: it lies in a break, or it is a public holiday.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Skipped {
    Break,
    Holiday(Holiday),
}

impl Skipped {
    /// Why, in `locale`: „vorlesungsfrei", or the holiday's name („Reformationstag").
    pub fn text(self, locale: Locale) -> &'static str {
        match self {
            Skipped::Break => crate::i18n::texts(locale).lecture_break,
            Skipped::Holiday(holiday) => holiday.name(locale),
        }
    }
}

/// How often a recurring row meets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Every {
    Week,
    AWeek,
    BWeek,
    FourWeeks,
}

impl Every {
    /// The rhythm of a recurring row, or `None` for a single date, a block and anything unknown.
    /// Radix's `other` means QIS's „vierwöch." wherever the data has it; another `other` names no
    /// rhythm a date can be made of.
    pub fn of(date: &EventDate) -> Option<Every> {
        match date.rhythm.as_ref().and_then(Code::known)? {
            Rhythm::Weekly => Some(Every::Week),
            Rhythm::WeekA => Some(Every::AWeek),
            Rhythm::WeekB => Some(Every::BWeek),
            Rhythm::Other => date
                .rhythm_raw
                .as_deref()
                .map(fold)
                .filter(|raw| raw.contains("vierw") || raw.contains("4-w"))
                .map(|_| Every::FourWeeks),
            Rhythm::Single | Rhythm::Block => None,
        }
    }

    /// Days from one meeting to the next.
    fn step(self) -> i32 {
        match self {
            Every::Week => 7,
            Every::AWeek | Every::BWeek => 14,
            Every::FourWeeks => 28,
        }
    }
}

/// A recurring row that names no day: its weekday (1 = Monday) and rhythm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Template {
    pub weekday: u8,
    pub every: Every,
}

/// The dates of one row, each in exactly one of `days`, `cancelled` and `skipped`, each list in
/// date order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Occurrences {
    /// The days it is held.
    pub days: Vec<Day>,
    /// Days QIS cancels, with the reason as written and the date the reason names instead.
    pub cancelled: Vec<(Day, Option<String>, Option<Day>)>,
    /// Recurring days not held, and why: the break, or a holiday.
    pub skipped: Vec<(Day, Skipped)>,
    /// Room notes of held days („LV findet ersatzweise im ZHG HS A statt.").
    pub notes: Vec<(Day, String)>,
    /// The range is the lecture period's, because the row has none.
    pub assumed: bool,
    /// A recurring row without any concrete day: no range and nothing to assume one from.
    pub template: Option<Template>,
    /// A block without times: whole days.
    pub all_day: bool,
}

/// The row's times in minutes when both are clock times and the start comes first; 24:00 is
/// 1440. Anything else is „Zeit unklar": the row is listed, but never placed in a grid or a
/// clash.
pub fn times(date: &EventDate) -> Option<(u16, u16)> {
    let from = minutes(date.start_time.as_deref()?)?;
    let to = minutes(date.end_time.as_deref()?)?;
    (from < to).then_some((from, to))
}

/// Which days of a row the break and the holidays take.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Skip {
    /// Weekly, A/B and four-weekly rows: the break and the holidays.
    Recurring,
    /// Blocks: the holidays. A block in the break was set there on purpose.
    Holidays,
    /// Single dates: none.
    Never,
}

/// Every date of `row` in its semester, by its rhythm:
///
/// - weekly, A and B: each day of the weekday from the first date to the last, every 7 or 14
///   days. Without a range the lecture period's, A and B rows in their weeks (`assumed`); without
///   a period, or for A and B without the anchor, the `template` only.
/// - four-weekly: every 28 days from the first day of the weekday; without a range the
///   `template`, since nothing says which four weeks.
/// - single: the first date.
/// - block: every day of the range, Saturday and Sunday only for „Block+SaSo".
/// - none, or a rhythm this build does not know: the date when first and last are one day.
///
/// A recurring row needs its weekday; without one it has no date. Another `other` has none either
/// (the page lists it „ohne feste Zeit").
pub fn occurrences(row: &DateRow, facts: &SemesterFacts) -> Occurrences {
    let date = &row.date;
    let weekday = date.weekday.and_then(|w| u8::try_from(w).ok()).filter(|w| (1..=7).contains(w));
    let first = date.first_date.as_deref().and_then(Day::parse);
    let range = first.zip(date.last_date.as_deref().and_then(Day::parse));
    let mut occ = Occurrences::default();
    let (candidates, skip) = match Every::of(date) {
        Some(every) => {
            let Some(weekday) = weekday else {
                return occ;
            };
            match range {
                Some((first, last)) => (walk(next(first, weekday), last, every.step()), Skip::Recurring),
                None => match assumed(every, weekday, facts) {
                    Some(days) => {
                        occ.assumed = true;
                        (days, Skip::Recurring)
                    }
                    None => {
                        occ.template = Some(Template { weekday, every });
                        return occ;
                    }
                },
            }
        }
        None => match date.rhythm.as_ref().and_then(Code::known) {
            Some(Rhythm::Other) => return occ,
            // A single date is its first date, whatever the last one says.
            Some(Rhythm::Single) => (first.into_iter().collect(), Skip::Never),
            Some(Rhythm::Block) => {
                let weekends = date.rhythm_raw.as_deref().is_some_and(|raw| fold(raw).contains("saso"));
                // Unclear times count as none: a whole day shows the block rather than losing it.
                occ.all_day = times(date).is_none();
                let days = range.map(|(first, last)| walk(first, last, 1)).unwrap_or_default();
                (days.into_iter().filter(|day| weekends || day.weekday() <= 5).collect(), Skip::Holidays)
            }
            // No rhythm, or a code this build does not know: only a range of one day is a date.
            _ => {
                (range.filter(|(first, last)| first == last).map(|(first, _)| first).into_iter().collect(), Skip::Never)
            }
        },
    };
    let notes = row.cancelled_dates.as_deref().map(cancel::parse).unwrap_or_default();
    for day in candidates {
        if skip == Skip::Recurring && facts.in_break(day) {
            occ.skipped.push((day, Skipped::Break));
            continue;
        }
        if skip != Skip::Never {
            if let Some(holiday) = facts.holiday(day) {
                occ.skipped.push((day, Skipped::Holiday(holiday)));
                continue;
            }
        }
        let on_day: Vec<&cancel::CancelNote> = notes.iter().filter(|note| note.day == day).collect();
        if let Some(note) = on_day.iter().find(|note| note.cancels) {
            occ.cancelled.push((day, note.note.clone(), note.moved_to));
            continue;
        }
        occ.notes.extend(on_day.iter().filter_map(|note| note.note.clone()).map(|text| (day, text)));
        occ.days.push(day);
    }
    occ
}

/// The days of a recurring row without a range: the lecture period's days of its weekday, for
/// A and B rows those of their weeks. `None` when the period, or for A and B the anchor, is
/// unknown, and always for a four-weekly row, which nothing anchors. The period is whole weeks,
/// the half-year need not be (2026S begins on a Wednesday): no day before or after the half-year
/// is assumed.
fn assumed(every: Every, weekday: u8, facts: &SemesterFacts) -> Option<Vec<Day>> {
    let (period_start, period_end) = facts.lecture?;
    let (start, end) = (period_start.max(facts.bounds.0), period_end.min(facts.bounds.1));
    let base = start.monday();
    let monday = match every {
        Every::Week => base,
        Every::AWeek | Every::BWeek => {
            let a_week = facts.a_week?.monday();
            let own = if every == Every::BWeek { a_week.plus(7) } else { a_week };
            // The period's first week of that parity.
            base.plus(own.0.saturating_sub(base.0).rem_euclid(14))
        }
        Every::FourWeeks => return None,
    };
    let first = monday.plus(i32::from(weekday) - 1);
    Some(walk(first, end, every.step()).into_iter().filter(|day| *day >= start).collect())
}

/// The first day on or after `day` that falls on `weekday` (1 = Monday).
fn next(day: Day, weekday: u8) -> Day {
    day.plus(i32::from((weekday + 7 - day.weekday()) % 7))
}

/// `first`, then every `step` days up to `last`, at most `MAX_OCCURRENCES` of them.
fn walk(first: Day, last: Day, step: i32) -> Vec<Day> {
    std::iter::successors(Some(first), |day| Some(day.plus(step)))
        .take(MAX_OCCURRENCES)
        .take_while(|day| *day <= last)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use folia_calendar::day;
    use folia_calendar::semester::SemesterKey;
    use folia_query as queries;

    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn isos(days: &[Day]) -> Vec<String> {
        days.iter().map(|day| day.iso()).collect()
    }

    /// 2026W as the data says it: lectures 05.10.2026–31.01.2027, the break 21.12.–03.01., A weeks
    /// from the first week, Brandenburg's holidays.
    fn winter() -> SemesterFacts {
        let key = SemesterKey::parse("2026W").unwrap();
        SemesterFacts {
            lecture: Some((d("2026-10-05"), d("2027-01-31"))),
            breaks: vec![(d("2026-12-21"), d("2027-01-03"))],
            a_week: Some(d("2026-10-05")),
            ..SemesterFacts::derive(key, None, &[])
        }
    }

    /// The same winter with nothing the data could say: no period, no anchor.
    fn unknown() -> SemesterFacts {
        SemesterFacts::derive(SemesterKey::parse("2026W").unwrap(), None, &[])
    }

    /// 2026S without data: the half-year 01.04.–30.09., a Wednesday to a Wednesday.
    fn summer() -> SemesterFacts {
        SemesterFacts::derive(SemesterKey::parse("2026S").unwrap(), None, &[])
    }

    #[derive(Clone, Copy)]
    struct Fixture {
        rhythm: Option<&'static str>,
        raw: Option<&'static str>,
        weekday: Option<i64>,
        time: Option<(&'static str, &'static str)>,
        range: Option<(&'static str, &'static str)>,
        cancelled: Option<&'static str>,
    }

    const WEEKLY: Fixture = Fixture {
        rhythm: Some("weekly"),
        raw: Some("A/B"),
        weekday: Some(1),
        time: Some(("15:30", "17:00")),
        range: Some(("2026-10-05", "2027-01-25")),
        cancelled: None,
    };

    fn row(f: Fixture) -> DateRow {
        DateRow {
            module_id: "12104".into(),
            ord: Some(1),
            cancelled_dates: f.cancelled.map(Into::into),
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: "148369".into(),
                event_number: None,
                event_title: "Entwicklung von Softwaresystemen".into(),
                event_type: Some("Übung".into()),
                group_name: Some("[unbenannt]".into()),
                weekday: f.weekday,
                start_time: f.time.map(|(from, _)| from.into()),
                end_time: f.time.map(|(_, to)| to.into()),
                rhythm: f.rhythm.map(Code::parse),
                rhythm_raw: f.raw.map(Into::into),
                first_date: f.range.map(|(first, _)| first.into()),
                last_date: f.range.map(|(_, last)| last.into()),
                room: None,
                campus: None,
                instructor: None,
                comment: None,
                source_url: None,
                room_short: None,
            },
        }
    }

    fn occ(f: Fixture, facts: &SemesterFacts) -> Occurrences {
        occurrences(&row(f), facts)
    }

    /// Why a date in the break is not held, in German.
    const BREAK_NOTE: &str = crate::i18n::DE.lecture_break;

    /// The skipped days and why, in German.
    fn skipped(occ: &Occurrences) -> Vec<(String, &'static str)> {
        occ.skipped.iter().map(|(day, why)| (day.iso(), why.text(Locale::De))).collect()
    }

    #[test]
    fn weekly_rows_meet_on_their_weekday_inside_the_range() {
        let o = occ(WEEKLY, &winter());
        assert_eq!(o.days.len(), 15);
        assert_eq!((o.days.first().copied(), o.days.last().copied()), (Some(d("2026-10-05")), Some(d("2027-01-25"))));
        assert_eq!(skipped(&o), [("2026-12-21".into(), BREAK_NOTE), ("2026-12-28".into(), BREAK_NOTE)]);
        assert!(!o.assumed && o.template.is_none() && !o.all_day);
        // A range that begins or ends on another weekday holds the row's weekday inside it.
        let o = occ(Fixture { weekday: Some(3), range: Some(("2026-10-05", "2026-10-27")), ..WEEKLY }, &winter());
        assert_eq!(isos(&o.days), ["2026-10-07", "2026-10-14", "2026-10-21"]);
        let o = occ(Fixture { weekday: Some(3), range: Some(("2026-10-08", "2026-10-13")), ..WEEKLY }, &winter());
        assert!(o.days.is_empty() && o.template.is_none(), "no Wednesday in the range");
        // Without a weekday a recurring row has no date at all.
        let o = occ(Fixture { weekday: None, ..WEEKLY }, &winter());
        assert_eq!(o, Occurrences::default());
        let o = occ(Fixture { weekday: Some(0), range: None, ..WEEKLY }, &winter());
        assert_eq!(o, Occurrences::default());
    }

    #[test]
    fn a_and_b_rows_with_a_range_count_from_their_first_date() {
        let a = occ(
            Fixture {
                rhythm: Some("week_a"),
                raw: Some("A"),
                weekday: Some(2),
                range: Some(("2026-10-06", "2027-01-26")),
                ..WEEKLY
            },
            &winter(),
        );
        assert_eq!(
            isos(&a.days),
            [
                "2026-10-06",
                "2026-10-20",
                "2026-11-03",
                "2026-11-17",
                "2026-12-01",
                "2026-12-15",
                "2027-01-12",
                "2027-01-26"
            ]
        );
        assert_eq!(skipped(&a), [("2026-12-29".into(), BREAK_NOTE)]);
        // The range, not the anchor, decides: a B row whose range starts in an A week keeps it.
        let odd = occ(
            Fixture {
                rhythm: Some("week_b"),
                raw: Some("B"),
                weekday: Some(2),
                range: Some(("2026-10-06", "2026-11-03")),
                ..WEEKLY
            },
            &winter(),
        );
        assert_eq!(isos(&odd.days), ["2026-10-06", "2026-10-20", "2026-11-03"]);
    }

    #[test]
    fn a_and_b_rows_without_a_range_take_the_a_weeks_when_they_are_anchored() {
        let a = Fixture { rhythm: Some("week_a"), raw: Some("A"), weekday: Some(4), range: None, ..WEEKLY };
        let b = Fixture { rhythm: Some("week_b"), raw: Some("B"), ..a };
        let (a_days, b_days) = (occ(a, &winter()), occ(b, &winter()));
        assert!(a_days.assumed && b_days.assumed);
        assert_eq!(a_days.days.first().copied(), Some(d("2026-10-08")));
        assert_eq!(b_days.days.first().copied(), Some(d("2026-10-15")));
        let (a_set, b_set): (BTreeSet<Day>, BTreeSet<Day>) =
            (a_days.days.iter().copied().collect(), b_days.days.iter().copied().collect());
        assert!(a_set.is_disjoint(&b_set));
        // Together they are the weekly row's Thursdays, the break (24.12., 31.12.) skipped.
        let weekly = occ(Fixture { weekday: Some(4), range: None, ..WEEKLY }, &winter());
        assert_eq!(a_set.union(&b_set).copied().collect::<Vec<_>>(), weekly.days);
        assert_eq!(skipped(&a_days), [("2026-12-31".into(), BREAK_NOTE)]);
        assert_eq!(skipped(&b_days), [("2026-12-24".into(), BREAK_NOTE)]);
        // A weeks that start in the period's second week: the first week is a B week.
        let shifted = SemesterFacts { a_week: Some(d("2026-10-12")), ..winter() };
        assert_eq!(occ(b, &shifted).days.first().copied(), Some(d("2026-10-08")));
        assert_eq!(occ(a, &shifted).days.first().copied(), Some(d("2026-10-15")));
        // Not anchored: the pattern only.
        let loose = SemesterFacts { a_week: None, ..winter() };
        let o = occ(a, &loose);
        assert_eq!(o.template, Some(Template { weekday: 4, every: Every::AWeek }));
        assert!(o.days.is_empty() && !o.assumed);
        assert_eq!(occ(b, &unknown()).template, Some(Template { weekday: 4, every: Every::BWeek }));
    }

    #[test]
    fn a_row_without_a_range_assumes_the_lecture_period() {
        let wednesday = Fixture { weekday: Some(3), range: None, ..WEEKLY };
        let o = occ(wednesday, &winter());
        assert!(o.assumed && o.template.is_none());
        assert_eq!(o.days.len(), 15);
        assert!(o.days.iter().all(|day| day.weekday() == 3));
        assert_eq!((o.days.first().copied(), o.days.last().copied()), (Some(d("2026-10-07")), Some(d("2027-01-27"))));
        assert_eq!(skipped(&o), [("2026-12-23".into(), BREAK_NOTE), ("2026-12-30".into(), BREAK_NOTE)]);
        // A Sunday row runs to the period's last day.
        let sunday = occ(Fixture { weekday: Some(7), range: None, ..WEEKLY }, &winter());
        assert_eq!(sunday.days.last().copied(), Some(d("2027-01-31")));
        // No period: the pattern only, and no date is assumed.
        let o = occ(wednesday, &unknown());
        assert_eq!(o.template, Some(Template { weekday: 3, every: Every::Week }));
        assert!(o.days.is_empty() && o.skipped.is_empty() && !o.assumed);
    }

    #[test]
    fn assumed_dates_stay_inside_the_half_year() {
        // Lectures from Wednesday 01.04. to Wednesday 30.09.: the period is whole weeks, from
        // Monday 30.03. to Sunday 04.10., and its first two and last four days are not 2026S's.
        let facts = SemesterFacts { lecture: Some((d("2026-03-30"), d("2026-10-04"))), ..summer() };
        let dated = |weekday: i64| {
            let o = occ(Fixture { weekday: Some(weekday), range: None, ..WEEKLY }, &facts);
            assert!(o.assumed);
            let mut all = o.days.clone();
            all.extend(o.skipped.iter().map(|(day, _)| *day));
            all.sort_unstable();
            (all.first().map(|day| day.iso()), all.last().map(|day| day.iso()))
        };
        assert_eq!(dated(2), (Some("2026-04-07".into()), Some("2026-09-29".into())), "not Tuesday 31.03.");
        assert_eq!(dated(3), (Some("2026-04-01".into()), Some("2026-09-30".into())));
        assert_eq!(dated(4), (Some("2026-04-02".into()), Some("2026-09-24".into())), "not Thursday 01.10.");
        // Ostermontag is the summer's first Monday.
        let monday = occ(Fixture { weekday: Some(1), range: None, ..WEEKLY }, &facts);
        assert_eq!(skipped(&monday), [("2026-04-06".into(), "Ostermontag"), ("2026-05-25".into(), "Pfingstmontag")]);
        assert_eq!(monday.days.first().copied(), Some(d("2026-04-13")));
    }

    #[test]
    fn four_weekly_rows_and_other_rhythms() {
        let vierw = Fixture {
            rhythm: Some("other"),
            raw: Some("vierwöch."),
            range: Some(("2026-10-12", "2026-12-07")),
            ..WEEKLY
        };
        assert_eq!(isos(&occ(vierw, &winter()).days), ["2026-10-12", "2026-11-09", "2026-12-07"]);
        // In the break, the four weeks skip that meeting.
        let o = occ(Fixture { range: Some(("2026-11-30", "2027-01-25")), ..vierw }, &winter());
        assert_eq!(isos(&o.days), ["2026-11-30", "2027-01-25"]);
        assert_eq!(skipped(&o), [("2026-12-28".into(), BREAK_NOTE)]);
        // Without a range nothing anchors four weeks, not even the lecture period.
        let o = occ(Fixture { weekday: Some(4), range: None, ..vierw }, &winter());
        assert_eq!(o.template, Some(Template { weekday: 4, every: Every::FourWeeks }));
        assert!(o.days.is_empty() && !o.assumed);
        assert_eq!(Every::of(&row(Fixture { raw: Some("4-wöchentlich"), ..vierw }).date), Some(Every::FourWeeks));
        // Another `other` is no rhythm: no day, no pattern.
        assert_eq!(occ(Fixture { raw: Some("nach Absprache"), ..vierw }, &winter()), Occurrences::default());
        assert_eq!(occ(Fixture { raw: None, ..vierw }, &winter()), Occurrences::default());
    }

    #[test]
    fn blocks_are_every_day_and_weekends_only_with_saso() {
        let block = Fixture {
            rhythm: Some("block"),
            raw: Some("Block"),
            weekday: None,
            time: Some(("09:15", "17:00")),
            range: Some(("2026-11-13", "2026-11-16")),
            cancelled: None,
        };
        let o = occ(block, &winter());
        assert_eq!(isos(&o.days), ["2026-11-13", "2026-11-16"]);
        assert!(!o.all_day);
        let o = occ(Fixture { raw: Some("Block+SaSo"), ..block }, &winter());
        assert_eq!(isos(&o.days), ["2026-11-13", "2026-11-14", "2026-11-15", "2026-11-16"]);
        // Without times, whole days; unclear times count as none.
        assert!(occ(Fixture { time: None, ..block }, &winter()).all_day);
        assert!(occ(Fixture { time: Some(("17:00", "09:15")), ..block }, &winter()).all_day);
        // A block in the break was set there on purpose; a holiday is skipped.
        let o = occ(Fixture { raw: Some("Block+SaSo"), range: Some(("2026-12-30", "2027-01-02")), ..block }, &winter());
        assert_eq!(isos(&o.days), ["2026-12-30", "2026-12-31", "2027-01-02"]);
        assert_eq!(skipped(&o), [("2027-01-01".into(), "Neujahr")]);
        // Without a range a block has no day.
        assert!(occ(Fixture { range: None, ..block }, &winter()).days.is_empty());
    }

    #[test]
    fn holidays_skip_recurring_dates_and_never_single_ones() {
        // Tag der Deutschen Einheit and Reformationstag 2026 are Saturdays.
        let saturday = Fixture { weekday: Some(6), range: Some(("2026-10-03", "2026-11-07")), ..WEEKLY };
        let o = occ(saturday, &winter());
        assert_eq!(isos(&o.days), ["2026-10-10", "2026-10-17", "2026-10-24", "2026-11-07"]);
        assert_eq!(
            skipped(&o),
            [("2026-10-03".into(), "Tag der Deutschen Einheit"), ("2026-10-31".into(), "Reformationstag")]
        );
        // A holiday in the break counts as the break.
        let friday = occ(Fixture { weekday: Some(5), ..WEEKLY }, &winter());
        assert!(friday.skipped.contains(&(d("2027-01-01"), Skipped::Break)));
        assert_eq!(skipped(&o).iter().map(|(_, why)| *why).collect::<Vec<_>>(), ["Tag der Deutschen Einheit", "Reformationstag"]);
        let english: Vec<&str> = o.skipped.iter().map(|(_, why)| why.text(Locale::En)).collect();
        assert_eq!(english, ["Day of German Unity", "Reformation Day"]);
        assert_eq!(Skipped::Break.text(Locale::En), "no lectures");
        // A single date on a holiday and in the break is held (147958 on Neujahr).
        let single = Fixture {
            rhythm: Some("single"),
            raw: Some("Einzel"),
            weekday: Some(5),
            range: Some(("2027-01-01", "2027-01-01")),
            ..WEEKLY
        };
        let o = occ(single, &winter());
        assert_eq!(isos(&o.days), ["2027-01-01"]);
        assert!(o.skipped.is_empty());
        // Outside the lecture period too.
        assert_eq!(
            isos(&occ(Fixture { range: Some(("2027-02-23", "2027-02-23")), ..single }, &winter()).days),
            ["2027-02-23"]
        );
        assert!(occ(Fixture { range: None, ..single }, &winter()).days.is_empty());
    }

    #[test]
    fn holidays_past_the_half_year_are_skipped_too() {
        // 145750's „Block+SaSo" of 2026S runs from 28.09. into the winter's first days.
        let block = Fixture {
            rhythm: Some("block"),
            raw: Some("Block+SaSo"),
            weekday: None,
            time: Some(("08:30", "20:00")),
            range: Some(("2026-09-28", "2026-10-03")),
            cancelled: None,
        };
        let o = occ(block, &summer());
        assert_eq!(isos(&o.days), ["2026-09-28", "2026-09-29", "2026-09-30", "2026-10-01", "2026-10-02"]);
        assert_eq!(skipped(&o), [("2026-10-03".into(), "Tag der Deutschen Einheit")]);
        // A weekly row that runs on past the half-year the same.
        let saturday = Fixture { weekday: Some(6), range: Some(("2026-09-26", "2026-10-10")), ..WEEKLY };
        let o = occ(saturday, &summer());
        assert_eq!(isos(&o.days), ["2026-09-26", "2026-10-10"]);
        assert_eq!(skipped(&o), [("2026-10-03".into(), "Tag der Deutschen Einheit")]);
    }

    #[test]
    fn cancellations_and_room_notes() {
        let noted = |cancelled: &'static str| occ(Fixture { cancelled: Some(cancelled), ..WEEKLY }, &winter());
        let o = noted("12.10.2026: 19.10.2026: Raumwechsel");
        assert_eq!(o.cancelled, [(d("2026-10-12"), None, None)]);
        assert_eq!(o.notes, [(d("2026-10-19"), "Raumwechsel".to_string())]);
        assert!(o.days.contains(&d("2026-10-19")) && !o.days.contains(&d("2026-10-12")));
        assert_eq!(o.days.len(), 14);
        let o = noted("07.12.2026: takes place on 10.12.2026");
        assert_eq!(
            o.cancelled,
            [(d("2026-12-07"), Some("takes place on 10.12.2026".to_string()), Some(d("2026-12-10")))]
        );
        assert!(!o.days.contains(&d("2026-12-10")), "a named replacement is not a date of the row");
        // A cancellation in the break stays a break; one of a day the row does not meet is nothing.
        let o = noted("21.12.2026: 28.12.2026: 06.10.2026:");
        assert!(o.cancelled.is_empty());
        assert_eq!(o.skipped.len(), 2);
        assert_eq!(o.days.len(), 15);
        // Cancelled wins over a room note of the same day.
        let o = noted("12.10.2026: Raumwechsel 12.10.2026: entfällt");
        assert_eq!(o.cancelled, [(d("2026-10-12"), Some("entfällt".to_string()), None)]);
        assert!(o.notes.is_empty());
        // Single dates are cancelled like any other.
        let single = Fixture {
            rhythm: Some("single"),
            range: Some(("2026-10-12", "2026-10-12")),
            cancelled: Some("12.10.2026:"),
            ..WEEKLY
        };
        let o = occ(single, &winter());
        assert!(o.days.is_empty() && o.cancelled.len() == 1);
    }

    #[test]
    fn rows_without_a_rhythm_are_dated_only_by_one_day() {
        let none = Fixture { rhythm: None, raw: None, range: Some(("2026-11-02", "2026-11-02")), ..WEEKLY };
        assert_eq!(isos(&occ(none, &winter()).days), ["2026-11-02"]);
        assert!(occ(Fixture { range: Some(("2026-11-02", "2026-11-09")), ..none }, &winter()).days.is_empty());
        assert!(occ(Fixture { range: None, ..none }, &winter()).days.is_empty());
        // A rhythm this build does not know is read the same way.
        let newer = Fixture { rhythm: Some("biweekly"), ..none };
        assert_eq!(isos(&occ(newer, &winter()).days), ["2026-11-02"]);
    }

    #[test]
    fn times_and_24_00() {
        let late = row(Fixture { time: Some(("22:00", "24:00")), ..WEEKLY });
        assert_eq!(times(&late.date), Some((1320, 1440)));
        assert_eq!(occurrences(&late, &winter()).days.len(), 15);
        assert_eq!(times(&row(Fixture { time: Some(("11:30", "11:30")), ..WEEKLY }).date), None);
        assert_eq!(times(&row(Fixture { time: Some(("13:00", "11:30")), ..WEEKLY }).date), None);
        assert_eq!(times(&row(Fixture { time: Some(("9:15", "11:30")), ..WEEKLY }).date), None);
        assert_eq!(times(&row(Fixture { time: None, ..WEEKLY }).date), None);
        // A block that ends at midnight has times.
        let block = Fixture {
            rhythm: Some("block"),
            raw: Some("Block"),
            weekday: None,
            time: Some(("18:00", "24:00")),
            ..WEEKLY
        };
        assert!(!occ(block, &winter()).all_day);
    }

    #[test]
    fn a_range_of_years_stops_at_the_guard() {
        let block = Fixture {
            rhythm: Some("block"),
            raw: Some("Block"),
            weekday: None,
            time: None,
            range: Some(("2026-06-01", "2028-06-07")),
            cancelled: None,
        };
        let o = occ(block, &winter());
        assert!(o.days.len() + o.skipped.len() <= MAX_OCCURRENCES);
        assert!(o.days.len() > 250);
        // Its holidays past the winter are skipped like those inside it.
        assert!(skipped(&o).contains(&("2026-12-25".into(), "1. Weihnachtstag")));
        assert!(skipped(&o).contains(&("2027-05-06".into(), "Christi Himmelfahrt")));
        assert!(o.days.iter().all(|day| !is_holiday(*day)));
        let o = occ(Fixture { range: Some(("2026-10-05", "2099-12-31")), ..WEEKLY }, &winter());
        assert_eq!(o.days.len() + o.skipped.len(), MAX_OCCURRENCES);
        let o = occ(Fixture { range: Some(("2027-01-25", "2026-10-05")), ..WEEKLY }, &winter());
        assert_eq!(o, Occurrences::default(), "a range backwards has no date");
    }

    /// Whether the law makes `day` a public holiday, asked without any semester's facts.
    fn is_holiday(day: Day) -> bool {
        day::holidays(day.ymd().0).iter().any(|(holiday, _)| *holiday == day)
    }

    /// What holds for every row: each date in one list, lists in order, the guard, patterns
    /// without days, assumed days inside the period and the half-year, no recurring or block day
    /// held on a holiday, and the weekday of recurring rows.
    fn invariants(row: &DateRow, o: &Occurrences, facts: &SemesterFacts) {
        let what = format!("{}/{:?}", row.date.event_id, row.ord);
        let mut all: Vec<Day> = o.days.clone();
        all.extend(o.cancelled.iter().map(|c| c.0));
        all.extend(o.skipped.iter().map(|s| s.0));
        let distinct: BTreeSet<Day> = all.iter().copied().collect();
        assert_eq!(distinct.len(), all.len(), "{what}: a date in two lists");
        assert!(all.len() <= MAX_OCCURRENCES, "{what}");
        assert!(o.days.windows(2).all(|w| w[0] < w[1]), "{what}");
        assert!(o.cancelled.windows(2).all(|w| w[0].0 < w[1].0), "{what}");
        assert!(o.skipped.windows(2).all(|w| w[0].0 < w[1].0), "{what}");
        assert!(o.notes.iter().all(|(day, _)| o.days.contains(day)), "{what}: a note of a day not held");
        if o.template.is_some() {
            assert!(all.is_empty() && !o.assumed, "{what}");
        }
        if o.assumed {
            let (start, end) = facts.lecture.unwrap();
            let (first, last) = facts.bounds;
            assert!(all.iter().all(|day| start <= *day && *day <= end), "{what}");
            assert!(all.iter().all(|day| first <= *day && *day <= last), "{what}: assumed outside the half-year");
        }
        let is_block = row.date.rhythm.as_ref().is_some_and(|r| r.is(Rhythm::Block));
        if o.all_day {
            assert!(is_block, "{what}");
        }
        if Every::of(&row.date).is_some() || is_block {
            assert!(o.days.iter().all(|day| !is_holiday(*day)), "{what}: held on a public holiday");
        }
        if Every::of(&row.date).is_some() {
            let weekday = row.date.weekday.and_then(|w| u8::try_from(w).ok());
            assert!(all.iter().all(|day| Some(day.weekday()) == weekday), "{what}");
        }
        if row.date.rhythm.as_ref().is_some_and(|r| r.is(Rhythm::Single)) {
            assert!(o.skipped.is_empty(), "{what}: a single date skipped");
            assert_eq!(all.len(), usize::from(row.date.first_date.is_some()), "{what}");
        }
    }

    /// The rows the design quotes, on the pinned snapshot; the invariants on every dated row of
    /// every semester of any snapshot.
    #[test]
    fn occurrences_follow_rhythm_and_exceptions() {
        let pinned = folia_test_support::studyplan_db("occurrences_follow_rhythm_and_exceptions");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(folia_test_support::open);
        let mut semesters = std::collections::BTreeMap::new();
        for semester in queries::semesters(&db).unwrap() {
            let key = SemesterKey::parse(&semester.key).unwrap();
            let counts = queries::semester_date_counts(&db, &semester.key).unwrap();
            let facts = SemesterFacts::derive(key, Some(&semester), &counts);
            let rows = queries::semester_schedule(&db, &semester.key).unwrap();
            for row in &rows {
                invariants(row, &occurrences(row, &facts), &facts);
            }
            semesters.insert(semester.key.clone(), (facts, rows));
        }
        if !is_pinned {
            return;
        }
        let of_in = |semester: &str, event: &str, ord: i64| {
            let (facts, rows) = &semesters[semester];
            let row = rows.iter().find(|row| row.date.event_id == event && row.ord == Some(ord)).unwrap();
            occurrences(row, facts)
        };
        let of = |event: &str, ord: i64| of_in("2026W", event, ord);
        // Mathematik IT-1, Mondays: 17 in the range, the two of the break skipped.
        let o = of("148303", 1);
        assert_eq!(o.days.len(), 15);
        assert_eq!(skipped(&o), [("2026-12-21".into(), BREAK_NOTE), ("2026-12-28".into(), BREAK_NOTE)]);
        // Elektrische und elektronische Grundlagen: lecture in A weeks, Übung in B weeks, Tue 07:30.
        let a = of("148134", 1);
        assert_eq!(
            isos(&a.days),
            [
                "2026-10-06",
                "2026-10-20",
                "2026-11-03",
                "2026-11-17",
                "2026-12-01",
                "2026-12-15",
                "2027-01-12",
                "2027-01-26"
            ]
        );
        let b = of("148135", 1);
        assert_eq!(
            isos(&b.days),
            ["2026-10-13", "2026-10-27", "2026-11-10", "2026-11-24", "2026-12-08", "2027-01-05", "2027-01-19"]
        );
        assert!(a.days.iter().all(|day| !b.days.contains(day)));
        // Fundamentals of Engine Technology: 17.12. cancelled, 10.12. named but not invented.
        let o = of("147988", 2);
        assert_eq!(
            o.cancelled,
            [(d("2026-12-17"), Some("takes place on 10.12.2026".to_string()), Some(d("2026-12-10")))]
        );
        assert!(!o.days.contains(&d("2026-12-10")) && !o.days.contains(&d("2026-12-17")));
        // A room note keeps the date.
        let o = of("149709", 1);
        assert!(o.days.contains(&d("2026-10-19")) && o.cancelled.is_empty());
        assert_eq!(o.notes, [(d("2026-10-19"), "LV findet ersatzweise im ZHG HS A statt.".to_string())]);
        // Risikomanagement, Block+SaSo: a Saturday and a Sunday, with times.
        let o = of("148181", 1);
        assert_eq!(isos(&o.days), ["2026-11-14", "2026-11-15"]);
        assert!(!o.all_day);
        // 12229's single date after the lecture period stays a single date.
        assert_eq!(isos(&of("148019", 1).days), ["2027-02-23"]);
        // 147958 teaches on Neujahr.
        assert_eq!(isos(&of("147958", 3).days), ["2027-01-01"]);
        // No range in QIS: the Wednesdays of the lecture period, assumed.
        let o = of("151406", 1);
        assert!(o.assumed);
        assert_eq!(o.days.len(), 15);
        assert!(o.days.iter().all(|day| day.weekday() == 3));
        // „vierwöch.": with a range every four weeks, without one the pattern only.
        assert_eq!(isos(&of("148293", 1).days), ["2026-10-12", "2026-11-09", "2026-12-07"]);
        assert_eq!(isos(&of("148293", 2).days), ["2027-01-11"]);
        assert_eq!(of("150397", 5).template, Some(Template { weekday: 4, every: Every::FourWeeks }));
        // A summer block that runs into the winter skips the winter's holiday.
        let o = of_in("2026S", "145750", 1);
        assert_eq!(isos(&o.days), ["2026-09-28", "2026-09-29", "2026-09-30", "2026-10-01", "2026-10-02"]);
        assert_eq!(skipped(&o), [("2026-10-03".into(), "Tag der Deutschen Einheit")]);
    }
}
