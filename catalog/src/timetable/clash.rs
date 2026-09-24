//! Teaching clashes: planned Termine that meet on one day at one time.
//!
//! Two rows clash when they are held on a common day and their times overlap: a Tuesday lecture
//! in A weeks and a weekly one at the same hour meet on the A weeks' Tuesdays only, and a lecture
//! from October to November never meets one from December on. So the check compares the held days
//! of the rows (`occur`), not their weekdays; cancelled and skipped days are no meeting. A row
//! whose dates are only a pattern (no range and nothing to assume one from) meets another on its
//! weekday, unless an A week meets a B week: without its days, a pattern is taken to clash, the
//! safe direction.
//!
//! Only rows of events without a common planned module are compared: a module's own lecture and
//! Übung at one time are its business, and an event two planned modules share is attended once.
//! Touching ends (one ends at 11:30, the next begins at 11:30) do not clash, and a row with an
//! unclear time never does.
//!
//! An option of an open choice („1 von 4 wählen") that clashes is no conflict while another option
//! is free: the student takes that one. Only when every visible option clashes is the choice
//! blocked („0 von 4 Übungsterminen frei"), and then its clashes count.

use std::collections::{BTreeMap, BTreeSet};

use super::day::Day;
use super::model::{Event, Row};
use super::occur::{Every, Template};

/// A hard clash between two rows: `(event, row)` indices into the timetable, `a` of the event
/// that comes first. One per pair of events, weekday and start of the overlap.
#[derive(Clone, Debug, PartialEq)]
pub struct Clash {
    pub a: (usize, usize),
    pub b: (usize, usize),
    /// The first common held day. When both rows are only patterns, there is none: then a day of
    /// their weekday in the week of 1970-01-01, which only its weekday means, and `days` is 0.
    pub first: Day,
    /// The number of common held days („8 Termine überschneiden sich").
    pub days: usize,
}

/// Where two rows meet.
enum Meeting {
    /// Their common held days, in order.
    Days(Vec<Day>),
    /// Both are patterns of this weekday.
    Pattern(u8),
}

/// A visible row with a time.
struct Placed<'e> {
    event: usize,
    index: usize,
    row: &'e Row,
    from: u16,
    to: u16,
}

/// Two rows that meet, before the choices are weighed.
struct Found {
    a: (usize, usize),
    b: (usize, usize),
    /// The start of the overlap.
    from: u16,
    meeting: Meeting,
}

/// The hard clashes of the visible rows of `events`, and the open choices whose every visible
/// option clashes. A clash is dropped when one side is an option of an open choice that has
/// another visible option without any clash. The rest are reported once per pair of events,
/// weekday and start of the overlap, with the first common day and the number of common days, in
/// the order of the events.
pub fn clashes(events: &[Event]) -> (Vec<Clash>, Vec<usize>) {
    let placed: Vec<Placed<'_>> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| event.hidden.is_none())
        .flat_map(|(event, e)| {
            e.rows.iter().enumerate().filter_map(move |(index, row)| {
                if row.hidden.is_some() {
                    return None;
                }
                Some(Placed { event, index, row, from: row.from?, to: row.to? })
            })
        })
        .collect();

    let mut found: Vec<Found> = Vec::new();
    for (x, a) in placed.iter().enumerate() {
        for b in placed.iter().skip(x + 1) {
            if a.event == b.event || !(a.from < b.to && b.from < a.to) {
                continue;
            }
            let (Some(ea), Some(eb)) = (events.get(a.event), events.get(b.event)) else {
                continue;
            };
            if ea.modules.iter().any(|module| eb.modules.contains(module)) {
                continue;
            }
            if let Some(meeting) = meeting(a.row, b.row) {
                found.push(Found { a: (a.event, a.index), b: (b.event, b.index), from: a.from.max(b.from), meeting });
            }
        }
    }

    // The options that clash with anything, and for each open choice whether one of its visible
    // options is free.
    let option_of = |(event, index): (usize, usize)| events.get(event)?.rows.get(index)?.option.map(|o| (event, o));
    let clashing: BTreeSet<(usize, usize)> = found.iter().flat_map(|f| [f.a, f.b]).filter_map(option_of).collect();
    let open: BTreeMap<usize, bool> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| event.unresolved())
        .map(|(i, event)| (i, event.visible_options().into_iter().any(|option| !clashing.contains(&(i, option)))))
        .collect();
    let avoidable = |side: (usize, usize)| option_of(side).is_some_and(|(event, _)| open.get(&event) == Some(&true));
    let blocked = open.iter().filter(|(_, free)| !**free).map(|(event, _)| *event).collect();

    // Hard clashes by pair of events, weekday and start: the common days, and the pair of rows of
    // the earliest one.
    let mut groups: BTreeMap<(usize, usize, u8, u16), Group> = BTreeMap::new();
    for f in found.iter().filter(|f| !avoidable(f.a) && !avoidable(f.b)) {
        match &f.meeting {
            Meeting::Days(days) => {
                for day in days {
                    let group = groups.entry((f.a.0, f.b.0, day.weekday(), f.from)).or_insert_with(|| Group::new(f));
                    group.add(*day, f);
                }
            }
            Meeting::Pattern(weekday) => {
                groups.entry((f.a.0, f.b.0, *weekday, f.from)).or_insert_with(|| Group::new(f));
            }
        }
    }
    let clashes = groups
        .into_iter()
        .map(|((.., weekday, _), group)| Clash {
            a: group.a,
            b: group.b,
            first: group.days.first().copied().unwrap_or_else(|| pattern_day(weekday)),
            days: group.days.len(),
        })
        .collect();
    (clashes, blocked)
}

/// The rows and days of one reported clash.
struct Group {
    a: (usize, usize),
    b: (usize, usize),
    days: BTreeSet<Day>,
}

impl Group {
    fn new(found: &Found) -> Self {
        Group { a: found.a, b: found.b, days: BTreeSet::new() }
    }

    /// Adds a common day; the rows of the earliest day stand for the clash.
    fn add(&mut self, day: Day, found: &Found) {
        if self.days.first().is_none_or(|first| day < *first) {
            (self.a, self.b) = (found.a, found.b);
        }
        self.days.insert(day);
    }
}

/// The first day two rows are both held, and on how many days: a merge of their held days. A
/// pattern meets a row's held days of its weekday, and another pattern of that weekday with no day
/// at all (`(a day of that weekday in 1970, 0)`), unless one is an A week and the other a B week.
/// Times are not compared.
pub fn shared(a: &Row, b: &Row) -> Option<(Day, usize)> {
    match meeting(a, b)? {
        Meeting::Days(days) => Some((*days.first()?, days.len())),
        Meeting::Pattern(weekday) => Some((pattern_day(weekday), 0)),
    }
}

fn meeting(a: &Row, b: &Row) -> Option<Meeting> {
    let days = match (a.occ.template, b.occ.template) {
        (None, None) => common(&a.occ.days, &b.occ.days),
        (Some(pattern), None) => on_pattern(pattern, b),
        (None, Some(pattern)) => on_pattern(pattern, a),
        (Some(p), Some(q)) => {
            return (p.weekday == q.weekday && compatible(p.every, Some(q.every)))
                .then_some(Meeting::Pattern(p.weekday));
        }
    };
    (!days.is_empty()).then_some(Meeting::Days(days))
}

/// The days in both sorted lists.
fn common(a: &[Day], b: &[Day]) -> Vec<Day> {
    let (mut a, mut b) = (a.iter().peekable(), b.iter().peekable());
    let mut both = Vec::new();
    while let (Some(x), Some(y)) = (a.peek(), b.peek()) {
        match x.cmp(y) {
            std::cmp::Ordering::Less => {
                a.next();
            }
            std::cmp::Ordering::Greater => {
                b.next();
            }
            std::cmp::Ordering::Equal => {
                both.push(**x);
                a.next();
                b.next();
            }
        }
    }
    both
}

/// The held days of `row` a pattern may meet: those of its weekday, unless the rhythms are A and
/// B weeks.
fn on_pattern(pattern: Template, row: &Row) -> Vec<Day> {
    if !compatible(pattern.every, Every::of(&row.date)) {
        return Vec::new();
    }
    row.occ.days.iter().copied().filter(|day| day.weekday() == pattern.weekday).collect()
}

/// Whether two rhythms can meet: everything but an A week and a B week.
fn compatible(a: Every, b: Option<Every>) -> bool {
    !matches!((a, b), (Every::AWeek, Some(Every::BWeek)) | (Every::BWeek, Some(Every::AWeek)))
}

/// A day of `weekday` (1 = Monday) in the week of 1970-01-01: the `first` of a clash of two
/// patterns, which has no date.
fn pattern_day(weekday: u8) -> Day {
    Day(0).monday().plus(i32::from(weekday) - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::Code;
    use crate::rows_detail::ModuleSws;
    use crate::timetable::model::tests::{d, event, ids, planned, table, teaching, Fixture};
    use crate::timetable::model::{Attendance, Timetable};
    use crate::timetable::select::{HiddenBy, Selection};
    use crate::timetable::semester::SemesterKey;

    fn no_sws() -> Vec<ModuleSws> {
        Vec::new()
    }

    /// The clashes as `(event a, row a, event b, row b, first day, days)`, by event id and `ord`.
    fn named(t: &Timetable) -> Vec<(String, i64, String, i64, String, usize)> {
        t.clashes
            .iter()
            .map(|c| {
                let (ea, eb) = (&t.events[c.a.0], &t.events[c.b.0]);
                let (ra, rb) = (&ea.rows[c.a.1], &eb.rows[c.b.1]);
                (ea.id.clone(), ra.ord.unwrap(), eb.id.clone(), rb.ord.unwrap(), c.first.iso(), c.days)
            })
            .collect()
    }

    fn blocked(t: &Timetable) -> Vec<&str> {
        t.blocked.iter().map(|i| t.events[*i].id.as_str()).collect()
    }

    fn clash(a: (&str, i64), b: (&str, i64), first: &str, days: usize) -> (String, i64, String, i64, String, usize) {
        (a.0.into(), a.1, b.0.into(), b.1, first.into(), days)
    }

    #[test]
    fn touching_ends_do_not_clash_and_an_overlap_does() {
        let at = |from: &str, to: &str| {
            let rows = [
                teaching("A", "1", 1, "Vorlesung", 2, "09:15", "10:45"),
                teaching("B", "2", 1, "Vorlesung", 2, from, to),
            ];
            named(&table(&rows, &["A", "B"], &no_sws(), &Selection::default()))
        };
        assert_eq!(at("10:45", "12:15"), []);
        assert_eq!(at("07:45", "09:15"), []);
        assert_eq!(at("10:44", "12:15"), [clash(("1", 1), ("2", 1), "2026-10-06", 15)]);
        assert_eq!(at("09:30", "10:00"), [clash(("1", 1), ("2", 1), "2026-10-06", 15)]);
        // Another weekday, or an unclear time, never clashes.
        let rows = [
            teaching("A", "1", 1, "Vorlesung", 2, "09:15", "10:45"),
            teaching("B", "2", 1, "Vorlesung", 3, "09:15", "10:45"),
            teaching("B", "3", 1, "Vorlesung", 2, "09:15", "08:00"),
        ];
        assert!(table(&rows, &["A", "B"], &no_sws(), &Selection::default()).clashes.is_empty());
    }

    #[test]
    fn a_module_never_clashes_with_itself_nor_with_a_shared_event() {
        let rows = [
            teaching("A", "1", 1, "Vorlesung", 2, "09:15", "10:45"),
            teaching("A", "2", 1, "Übung", 2, "09:15", "10:45"),
            // Event 3 belongs to A and B: it is attended once, beside A's own events.
            teaching("A", "3", 1, "Seminar", 2, "09:15", "10:45"),
            teaching("B", "3", 1, "Seminar", 2, "09:15", "10:45"),
        ];
        assert!(table(&rows, &["A", "B"], &no_sws(), &Selection::default()).clashes.is_empty());
        // B's own event meets A's two; the shared one meets none.
        let mut more = rows.to_vec();
        more.push(teaching("B", "4", 1, "Übung", 2, "10:00", "11:30"));
        let t = table(&more, &["A", "B"], &no_sws(), &Selection::default());
        assert_eq!(
            named(&t),
            [clash(("1", 1), ("4", 1), "2026-10-06", 15), clash(("2", 1), ("4", 1), "2026-10-06", 15)]
        );
    }

    #[test]
    fn rows_meet_only_on_common_held_days() {
        let a =
            teaching("A", "1", 1, "Vorlesung", 2, "07:30", "09:00").rhythm("week_a").range("2026-10-06", "2027-01-26");
        let b = teaching("B", "2", 1, "Übung", 2, "07:30", "09:00").rhythm("week_b").range("2026-10-13", "2027-01-19");
        let weekly = teaching("C", "3", 1, "Vorlesung", 2, "07:30", "09:00");
        let t = table(&[a.clone(), b.clone(), weekly], &["A", "B", "C"], &no_sws(), &Selection::default());
        // A and B weeks never meet; the weekly row meets both, on 8 and on 7 Tuesdays.
        assert_eq!(named(&t), [clash(("1", 1), ("3", 1), "2026-10-06", 8), clash(("2", 1), ("3", 1), "2026-10-13", 7)]);
        // A cancelled day is no meeting; the break is none either.
        let mut cancelled = teaching("C", "3", 1, "Vorlesung", 2, "07:30", "09:00");
        cancelled.0.cancelled_dates = Some("06.10.2026: 20.10.2026: Raumwechsel".into());
        let t = table(&[a.clone(), cancelled], &["A", "C"], &no_sws(), &Selection::default());
        assert_eq!(named(&t), [clash(("1", 1), ("3", 1), "2026-10-20", 7)]);
        // One range after the other.
        let early = teaching("C", "3", 1, "Vorlesung", 2, "07:30", "09:00").range("2026-10-06", "2026-10-13");
        let late = teaching("D", "4", 1, "Vorlesung", 2, "07:30", "09:00").range("2026-10-20", "2027-01-26");
        assert!(table(&[early, late], &["C", "D"], &no_sws(), &Selection::default()).clashes.is_empty());
        let rows = [&t.events[0].rows[0], &t.events[1].rows[0]];
        assert_eq!(shared(rows[0], rows[1]), Some((d("2026-10-20"), 7)));
        assert_eq!(shared(rows[1], rows[0]), Some((d("2026-10-20"), 7)));
    }

    /// A timetable of 2026W without a lecture period: rows without a range are patterns.
    fn unanchored(rows: &[Fixture], modules: &[&str]) -> Timetable {
        let facts = crate::timetable::facts::SemesterFacts::derive(SemesterKey::parse("2026W").unwrap(), None, &[]);
        let schedule: Vec<_> = rows.iter().map(|row| row.0.clone()).collect();
        let modules = ids(modules);
        let input = crate::timetable::model::Input {
            key: facts.key,
            semester: None,
            facts: &facts,
            modules: &modules,
            schedule: &schedule,
            exams: &[],
            sws: &[],
        };
        let t = Timetable::build(&input, &Selection::default());
        crate::timetable::model::tests::invariants(&t);
        t
    }

    #[test]
    fn patterns_meet_on_their_weekday_unless_a_meets_b() {
        let a = teaching("A", "1", 1, "Vorlesung", 4, "09:15", "10:45").rhythm("week_a").undated();
        let b = teaching("B", "2", 1, "Vorlesung", 4, "09:15", "10:45").rhythm("week_b").undated();
        let weekly = teaching("C", "3", 1, "Vorlesung", 4, "10:00", "11:30").undated();
        let dated = teaching("D", "4", 1, "Vorlesung", 4, "09:15", "10:45");
        let t = unanchored(&[a, b, weekly, dated.clone()], &["A", "B", "C", "D"]);
        assert!(t.events[..3].iter().all(|e| e.rows[0].occ.template.is_some()));
        let thursday = Day(-3).plus(3);
        assert_eq!(thursday.weekday(), 4);
        // A × B: none. A × C and B × C: patterns, no day. Each × D: D's 17 Thursdays (without a
        // lecture period there is no break).
        assert_eq!(
            named(&t),
            [
                clash(("1", 1), ("3", 1), &thursday.iso(), 0),
                clash(("1", 1), ("4", 1), "2026-10-08", 17),
                clash(("2", 1), ("3", 1), &thursday.iso(), 0),
                clash(("2", 1), ("4", 1), "2026-10-08", 17),
                clash(("3", 1), ("4", 1), "2026-10-08", 17)
            ]
        );
        assert_eq!(t.clashes[0].first, thursday);
        // A dated B row does not meet an A pattern.
        let b_dated = dated.rhythm("week_b").range("2026-10-15", "2027-01-28");
        let a = teaching("A", "1", 1, "Vorlesung", 4, "09:15", "10:45").rhythm("week_a").undated();
        assert!(unanchored(&[a, b_dated], &["A", "D"]).clashes.is_empty());
        // Patterns of two weekdays never meet.
        let monday = teaching("A", "1", 1, "Vorlesung", 1, "09:15", "10:45").undated();
        let tuesday = teaching("B", "2", 1, "Vorlesung", 2, "09:15", "10:45").undated();
        assert!(unanchored(&[monday, tuesday], &["A", "B"]).clashes.is_empty());
    }

    #[test]
    fn a_clash_is_one_per_pair_of_events_weekday_and_time() {
        // A's Monday slot with two ends (two rows of one key, 147849's case) meets B's block once;
        // the block, Monday to Friday, meets A's Wednesday row on its Wednesday.
        let rows = [
            teaching("A", "1", 1, "Vorlesung", 1, "09:15", "10:45").range("2026-10-05", "2026-11-30"),
            teaching("A", "1", 2, "Vorlesung", 1, "09:15", "11:30").range("2026-10-05", "2026-11-30"),
            teaching("A", "1", 3, "Vorlesung", 3, "09:15", "10:45"),
            teaching("B", "2", 1, "Blockseminar", 1, "09:00", "12:00")
                .rhythm("block")
                .range("2026-11-30", "2026-12-04"),
        ];
        let t = table(&rows, &["A", "B"], &no_sws(), &Selection::default());
        assert_eq!(named(&t), [clash(("1", 1), ("2", 1), "2026-11-30", 1), clash(("1", 3), ("2", 1), "2026-12-02", 1)]);
        // A block meeting a weekly row twice a week of two weeks: one clash per weekday.
        let rows = [
            teaching("A", "1", 1, "Vorlesung", 2, "13:45", "15:15"),
            teaching("B", "2", 1, "Blockseminar", 1, "13:00", "16:00")
                .rhythm("block")
                .range("2026-11-02", "2026-11-13"),
            teaching("A", "1", 2, "Vorlesung", 4, "13:45", "15:15"),
        ];
        let t = table(&rows, &["A", "B"], &no_sws(), &Selection::default());
        let expected = [clash(("1", 1), ("2", 1), "2026-11-03", 2), clash(("1", 2), ("2", 1), "2026-11-05", 2)];
        assert_eq!(named(&t), expected);
    }

    #[test]
    fn an_open_choice_clashes_only_when_no_option_is_free() {
        let two = [ModuleSws { module_id: "B".into(), form: Code::parse("exercise"), sws: 2.0 }];
        let lecture = teaching("A", "1", 1, "Vorlesung", 1, "15:30", "17:00");
        let options =
            [teaching("B", "2", 1, "Übung", 1, "15:30", "17:00"), teaching("B", "2", 2, "Übung", 2, "15:30", "17:00")];
        let rows = [lecture.clone(), options[0].clone(), options[1].clone()];
        let t = table(&rows, &["A", "B"], &two, &Selection::default());
        assert!(matches!(event(&t, "2").attendance, Attendance::OneOf { .. }) && event(&t, "2").unresolved());
        assert_eq!((named(&t), blocked(&t)), (vec![], vec![]), "Tuesday is free");
        // Both options taken: blocked, and each clash counts.
        let mut full = rows.to_vec();
        full.push(teaching("A", "3", 1, "Seminar", 2, "16:00", "17:30"));
        let t = table(&full, &["A", "B"], &two, &Selection::default());
        assert_eq!(blocked(&t), ["2"]);
        // A's „Seminar 3" comes before its „Vorlesung 1" by title.
        assert_eq!(
            named(&t),
            [clash(("3", 1), ("2", 2), "2026-10-06", 15), clash(("1", 1), ("2", 1), "2026-10-05", 15)]
        );
        // Tuesday hidden by its eye button: one visible option, no longer open, and its clash is hard.
        let hide = Selection { hidden_rows: [options[1].key()].into(), ..Selection::default() };
        let t = table(&rows, &["A", "B"], &two, &hide);
        assert!(!event(&t, "2").unresolved());
        assert_eq!((named(&t), blocked(&t)), (vec![clash(("1", 1), ("2", 1), "2026-10-05", 15)], vec![]));
        // Monday chosen: the same.
        let chosen = Selection { chosen_rows: [options[0].key()].into(), ..Selection::default() };
        let t = table(&rows, &["A", "B"], &two, &chosen);
        assert_eq!(event(&t, "2").rows[1].hidden, Some(HiddenBy::Choice));
        assert_eq!(named(&t), [clash(("1", 1), ("2", 1), "2026-10-05", 15)]);
        // Tuesday chosen: nothing.
        let chosen = Selection { chosen_rows: [options[1].key()].into(), ..Selection::default() };
        assert!(table(&rows, &["A", "B"], &two, &chosen).clashes.is_empty());
        // A hidden event clashes with nothing: Monday is free again.
        let hidden = Selection { hidden_events: [1].into(), ..Selection::default() };
        let t = table(&full, &["A", "B"], &two, &hidden);
        assert_eq!((named(&t), blocked(&t)), (vec![], vec![]));
    }

    fn fs1() -> Vec<String> {
        ids(&["12104", "12107", "12102", "11112"])
    }

    /// Informatik B.Sc.'s first semester: 12102's second offering at Sachsendorf (149408, 148455)
    /// meets 12107's Tuesday; hiding it leaves no clash (H.2).
    #[test]
    fn informatik_first_semester_clashes() {
        let Some(db) = crate::tests::studyplan_db("informatik_first_semester_clashes") else {
            return;
        };
        let t = planned(&db, "2026W", &fs1(), &Selection::default());
        // 12107's lecture (A weeks at 07:30, weekly at 09:15) and Übung (B weeks at 07:30) against
        // the Sachsendorf lecture (Tuesdays at 07:30) and lab (Tuesdays at 09:15); 12102's events
        // by title, the lab first.
        assert_eq!(
            named(&t),
            [
                clash(("148134", 2), ("148455", 1), "2026-10-06", 15),
                clash(("148134", 1), ("149408", 1), "2026-10-06", 8),
                clash(("148135", 1), ("149408", 1), "2026-10-13", 7)
            ]
        );
        assert!(t.blocked.is_empty());
        let open: Vec<&str> = t.events.iter().filter(|e| e.unresolved()).map(|e| e.id.as_str()).collect();
        assert_eq!(open, ["148369", "148370", "148304"]);
        // 148369's Tuesday 15:30 meets 150132 (Tutorium Mathematik IT-1), but three other
        // Übungen of it are free: no clash.
        let (e148369, e150132) = (event(&t, "148369"), event(&t, "150132"));
        let tuesday = e148369.rows.iter().find(|r| r.date.weekday == Some(2) && r.from == Some(930)).unwrap();
        assert!(shared(tuesday, &e150132.rows[0]).is_some());
        assert_eq!((tuesday.from, e150132.rows[0].from), (Some(930), Some(930)));

        let hide = Selection { hidden_events: [149408, 148455].into(), ..Selection::default() };
        let t = planned(&db, "2026W", &fs1(), &hide);
        assert!(t.clashes.is_empty() && t.blocked.is_empty());
    }

    /// Its third semester as the import places it: 12202's cohort offering „Softwarepraktikum
    /// Medizininformatik" (149333, Projekt, Wednesdays at Sachsendorf, three required slots of a
    /// 4-SWS module) meets Theoretische Informatik's Übung; hiding it is the way out.
    #[test]
    fn informatik_third_semester_clashes() {
        let Some(db) = crate::tests::studyplan_db("informatik_third_semester_clashes") else {
            return;
        };
        let fs3 = ids(&["11787", "12202", "11213"]);
        let t = planned(&db, "2026W", &fs3, &Selection::default());
        assert_eq!(event(&t, "149333").attendance, Attendance::All);
        assert_eq!(named(&t), [clash(("148315", 1), ("149333", 1), "2026-10-07", 15)]);
        let e148230 = event(&t, "148230");
        assert!(e148230.unresolved());
        assert_eq!(e148230.visible_options().len(), 2);
        assert!(t.blocked.is_empty());
        assert_eq!(t.exam_warnings, []);
        let hide = Selection { hidden_events: [149333].into(), ..Selection::default() };
        let t = planned(&db, "2026W", &fs3, &hide);
        assert!(t.clashes.is_empty() && t.blocked.is_empty());
    }
}
