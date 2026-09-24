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
//! An option of an open choice („1 von 4 wählen") that clashes is no conflict while the student
//! can take another one. The plan's open choices are weighed together: an option is free when it
//! meets no required Termin, and when every open choice can take a free option with no two picks
//! meeting, none of their clashes counts (two Übungen whose Friday slots meet leave each other
//! their other slots). A choice whose every option meets a required Termin is blocked („0 von 4
//! Übungsterminen frei"). When some choices cannot be placed together, those of them that cannot
//! move away from the others are blocked too. The clashes of blocked choices count, so each one
//! is named by a clash the page can show.

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

/// An option of an open choice: `(event, option)`.
type Pick = (usize, usize);

/// The most picks the search for free options tries per group of choices. A plan has a handful of
/// open choices with a few options each and needs far fewer; a pathological one falls back to
/// weighing each choice alone instead of stalling the page.
const MAX_STEPS: usize = 10_000;

/// The hard clashes of the visible rows of `events`, and the open choices that cannot take a free
/// option (see the module's text). A clash is dropped when one side is an option of an open
/// choice that is not blocked. The rest are reported once per pair of events, weekday and start
/// of the overlap, with the first common day and the number of common days, in the order of the
/// events.
pub fn clashes(events: &[Event]) -> (Vec<Clash>, Vec<usize>) {
    let Weighed { hard, blocked } = weigh(events);

    // Hard clashes by pair of events, weekday and start: the common days, and the pair of rows of
    // the earliest one.
    let mut groups: BTreeMap<(usize, usize, u8, u16), Group> = BTreeMap::new();
    for f in &hard {
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

/// Every row in a hard clash, as `(event, row)`. A reported `Clash` names only the rows of its
/// first common day; another row of the same pair of events, weekday and start is in the clash
/// too (148661's B-week Thursday behind the single on its first Thursday), and the Regelwoche and
/// a module page's overlay mark it.
pub fn hard_rows(events: &[Event]) -> BTreeSet<(usize, usize)> {
    weigh(events).hard.iter().flat_map(|f| [f.a, f.b]).collect()
}

/// The meetings that count, and the blocked choices by event index.
struct Weighed {
    hard: Vec<Found>,
    blocked: Vec<usize>,
}

/// Weighs the open choices of `events` against their meetings. An option is free when it meets
/// no required row: a visible row that belongs to no option, or a row of an event that is not an
/// open choice. A choice without a free option is blocked. The others are grouped by free options
/// that meet each other, and a group is settled when each of its choices can take a free option
/// with no two picks meeting. A group that cannot be (or whose search runs out of steps) is
/// weighed choice by choice: a choice moves away when it has an option that meets only options of
/// choices that move away (at first: settled ones, or nothing at all); the rest are blocked. A
/// meeting counts unless one side is an option of a choice that moves away.
fn weigh(events: &[Event]) -> Weighed {
    let found = meetings(events);
    let open: BTreeMap<usize, Vec<usize>> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| event.unresolved())
        .map(|(i, event)| (i, event.visible_options()))
        .collect();
    let pick_of = |(event, index): (usize, usize)| -> Option<Pick> {
        if !open.contains_key(&event) {
            return None;
        }
        events.get(event)?.rows.get(index)?.option.map(|option| (event, option))
    };

    // What each option of an open choice meets: `None` is a required row.
    let mut partners: BTreeMap<Pick, Vec<Option<Pick>>> = BTreeMap::new();
    for f in &found {
        let (a, b) = (pick_of(f.a), pick_of(f.b));
        if let Some(a) = a {
            partners.entry(a).or_default().push(b);
        }
        if let Some(b) = b {
            partners.entry(b).or_default().push(a);
        }
    }
    let meets: BTreeSet<(Pick, Pick)> =
        partners.iter().flat_map(|(pick, others)| others.iter().flatten().map(move |other| (*pick, *other))).collect();
    let free: BTreeMap<usize, Vec<usize>> = open
        .iter()
        .map(|(event, options)| {
            let free = options
                .iter()
                .copied()
                .filter(|option| !partners.get(&(*event, *option)).is_some_and(|p| p.iter().any(Option::is_none)))
                .collect();
            (*event, free)
        })
        .collect();

    // The choices with a free option, in groups linked by free options that meet.
    let (mut settled, mut failed) = (BTreeSet::new(), BTreeSet::new());
    let mut seen = BTreeSet::new();
    for (start, options) in &free {
        if options.is_empty() || !seen.insert(*start) {
            continue;
        }
        let mut group = vec![*start];
        let mut next = 0;
        while let Some(event) = group.get(next).copied() {
            next += 1;
            for option in free.get(&event).into_iter().flatten() {
                for other in partners.get(&(event, *option)).into_iter().flatten().flatten() {
                    let linked = free.get(&other.0).is_some_and(|options| options.contains(&other.1));
                    if linked && seen.insert(other.0) {
                        group.push(other.0);
                    }
                }
            }
        }
        // Fewest options first: a dead end shows early.
        let mut choices: Vec<(usize, &[usize])> =
            group.iter().filter_map(|event| Some((*event, free.get(event)?.as_slice()))).collect();
        choices.sort_by_key(|(event, options)| (options.len(), *event));
        let mut steps = MAX_STEPS;
        if place(&choices, &mut Vec::new(), &meets, &mut steps) {
            settled.extend(group);
        } else {
            failed.extend(group);
        }
    }

    // The choices that move away: the settled ones, then, while one is found, a failed one with
    // an option that meets only options of choices that move away.
    let mut away = settled;
    loop {
        let more: Vec<usize> = failed
            .iter()
            .copied()
            .filter(|event| !away.contains(event))
            .filter(|event| {
                open.get(event).into_iter().flatten().any(|option| {
                    partners
                        .get(&(*event, *option))
                        .into_iter()
                        .flatten()
                        .all(|other| matches!(other, Some((mover, _)) if away.contains(mover)))
                })
            })
            .collect();
        if more.is_empty() {
            break;
        }
        away.extend(more);
    }

    let blocked = open.keys().copied().filter(|event| !away.contains(event)).collect();
    let moves = |side: (usize, usize)| pick_of(side).is_some_and(|(event, _)| away.contains(&event));
    let hard = found.into_iter().filter(|f| !moves(f.a) && !moves(f.b)).collect();
    Weighed { hard, blocked }
}

/// Whether each choice can take one of its options with no two picks meeting: a depth-first
/// search that tries at most `steps` picks. Running out of steps counts as no.
fn place(
    choices: &[(usize, &[usize])],
    picked: &mut Vec<Pick>,
    meets: &BTreeSet<(Pick, Pick)>,
    steps: &mut usize,
) -> bool {
    let Some(((event, options), rest)) = choices.split_first() else {
        return true;
    };
    for option in options.iter() {
        let Some(left) = steps.checked_sub(1) else {
            return false;
        };
        *steps = left;
        let pick = (*event, *option);
        if picked.iter().any(|other| meets.contains(&(pick, *other))) {
            continue;
        }
        picked.push(pick);
        if place(rest, picked, meets, steps) {
            return true;
        }
        picked.pop();
    }
    false
}

/// Every two visible, timed rows that meet: of different events without a common planned module,
/// overlapping in time, on a common held day.
fn meetings(events: &[Event]) -> Vec<Found> {
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
    found
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

    /// An open choice of named groups: one Übung per slot, `(weekday, from, to)`.
    fn groups(module: &str, event: &str, slots: &[(i64, &str, &str)]) -> Vec<Fixture> {
        slots
            .iter()
            .zip(1i64..)
            .map(|((weekday, from, to), ord)| {
                teaching(module, event, ord, "Übung", *weekday, from, to).group(&format!("{ord}-Gruppe"))
            })
            .collect()
    }

    #[test]
    fn open_choices_are_weighed_together() {
        // B's Monday meets A's lecture and its Friday slots meet two of C's, but B on Friday 11:30
        // and C on Friday 07:30 meet nothing (Bauingenieurwesen's 150941 and 148590): no choice
        // is blocked, and no clash counts.
        let lecture = teaching("A", "1", 1, "Vorlesung", 1, "11:30", "13:00");
        let b = groups("B", "4", &[(5, "11:30", "13:00"), (5, "13:45", "15:15"), (1, "11:30", "13:00")]);
        let c = groups("C", "5", &[(5, "07:30", "09:00"), (5, "11:30", "13:00"), (5, "13:45", "15:15")]);
        let t =
            table(&[vec![lecture], b.clone(), c.clone()].concat(), &["A", "B", "C"], &no_sws(), &Selection::default());
        assert!(event(&t, "4").unresolved() && event(&t, "5").unresolved());
        assert_eq!((named(&t), blocked(&t)), (vec![], vec![]));
        assert!(hard_rows(&t.events).is_empty());
        // B's Friday slots alone meet only C's: B is not blocked by meetings no clash would name
        // (Physiotherapie's 150390).
        let t = table(&[b[..2].to_vec(), c].concat(), &["B", "C"], &no_sws(), &Selection::default());
        assert_eq!((named(&t), blocked(&t)), (vec![], vec![]));

        // Three choices that cannot be placed together (Wirtschaftsinformatik's 148936, 148130 and
        // 151699): B and C each have one slot free of A's lectures, and D's free slots are theirs.
        // All three are blocked, and their clashes count.
        let lectures = [
            teaching("A", "1", 1, "Vorlesung", 1, "11:30", "13:00"),
            teaching("A", "1", 2, "Vorlesung", 2, "13:45", "15:15"),
            teaching("A", "1", 3, "Vorlesung", 3, "15:30", "17:00"),
        ];
        let b = groups("B", "4", &[(1, "15:30", "17:00"), (3, "15:30", "17:00")]);
        let c = groups("C", "5", &[(1, "13:45", "15:15"), (2, "13:45", "15:15")]);
        let d = groups("D", "6", &[(1, "11:30", "13:00"), (1, "13:45", "15:15"), (1, "15:30", "17:00")]);
        let rows = [lectures.to_vec(), b, c, d].concat();
        let t = table(&rows, &["A", "B", "C", "D"], &no_sws(), &Selection::default());
        assert_eq!(blocked(&t), ["4", "5", "6"]);
        assert_eq!(
            named(&t),
            [
                clash(("1", 3), ("4", 2), "2026-10-07", 15),
                clash(("1", 2), ("5", 2), "2026-10-06", 15),
                clash(("1", 1), ("6", 1), "2026-10-05", 15),
                clash(("4", 1), ("6", 3), "2026-10-05", 15),
                clash(("5", 1), ("6", 2), "2026-10-05", 15)
            ]
        );
        // With A's Monday Termin hidden, D takes Monday 11:30 and nothing is left.
        let hide = Selection { hidden_rows: [lectures[0].key()].into(), ..Selection::default() };
        let t = table(&rows, &["A", "B", "C", "D"], &no_sws(), &hide);
        assert_eq!((named(&t), blocked(&t)), (vec![], vec![]));

        // A choice with no slot free of the lectures is blocked on its own; C's Monday slot meets
        // only B's, so C stays open (150585 beside 150349 in 216-82-2022).
        let lectures = [
            teaching("A", "1", 1, "Vorlesung", 1, "11:30", "13:00"),
            teaching("A", "1", 2, "Vorlesung", 2, "11:30", "13:00"),
        ];
        let b = groups("B", "4", &[(1, "12:15", "13:45"), (2, "12:15", "13:45")]);
        let c = groups("C", "5", &[(1, "13:00", "14:30"), (2, "11:30", "13:00")]);
        let t = table(&[lectures.to_vec(), b, c].concat(), &["A", "B", "C"], &no_sws(), &Selection::default());
        assert_eq!(blocked(&t), ["4"]);
        assert!(event(&t, "5").unresolved());
        assert_eq!(
            named(&t),
            [clash(("1", 1), ("4", 1), "2026-10-05", 15), clash(("1", 2), ("4", 2), "2026-10-06", 15)]
        );
    }

    #[test]
    fn hard_rows_hold_every_row_of_a_clash() {
        // 148661's case: a single on the first Thursday and the B weeks' Thursdays after it, at one
        // time, meet B's weekly Übung. The clash names the single; both rows are hard.
        let single =
            teaching("A", "1", 1, "Vorlesung", 4, "09:30", "11:00").rhythm("single").range("2026-10-08", "2026-10-08");
        let b_weeks =
            teaching("A", "1", 2, "Vorlesung", 4, "09:30", "11:00").rhythm("week_b").range("2026-10-15", "2027-01-21");
        let weekly = teaching("B", "2", 1, "Übung", 4, "09:00", "11:30");
        // C's Thursday slot meets all three, but C can take Friday: its rows are in no clash.
        let c = groups("C", "3", &[(4, "09:30", "11:00"), (5, "09:30", "11:00")]);
        let rows = [vec![single, b_weeks, weekly], c].concat();
        let t = table(&rows, &["A", "B", "C"], &no_sws(), &Selection::default());
        assert_eq!(named(&t), [clash(("1", 1), ("2", 1), "2026-10-08", 8)]);
        assert_eq!(hard_rows(&t.events), BTreeSet::from([(0, 0), (0, 1), (1, 0)]));
    }

    /// Plans of 2026W where open choices meet each other: those that can be placed together are
    /// not blocked, and a plan whose choices cannot be keeps them blocked.
    #[test]
    fn open_choices_of_real_plans() {
        let Some(db) = crate::tests::studyplan_db("open_choices_of_real_plans") else {
            return;
        };
        // Bauingenieurwesen 017-82-2022, FS1: Vermessung's Übung (150941) takes Friday 11:30 and
        // the Tutorium 148590 Friday 07:30; 150941's Monday meets 148750, which does not count.
        let bau = ids(&["11281", "13700", "11517", "11520", "11542"]);
        let t = planned(&db, "2026W", &bau, &Selection::default());
        let e150941 = t.events.iter().position(|e| e.id == "150941").unwrap();
        assert!(t.events[e150941].unresolved() && event(&t, "148590").unresolved());
        assert!(t.blocked.is_empty());
        assert!(t.clashes.iter().all(|c| c.a.0 != e150941 && c.b.0 != e150941));
        // Physiotherapie 901-84-2017, FS3: 150390's slots meet only other choices' slots.
        let t = planned(&db, "2026W", &ids(&["12099", "12100", "12112", "12113"]), &Selection::default());
        assert!(event(&t, "150390").unresolved() && t.blocked.is_empty());
        // 216-82-2022, FS1: 150585 has no slot free of 150349's required Termine; 150349's second
        // group meets only 150585's slot, so 150349 stays open.
        let t = planned(&db, "2026W", &ids(&["11107", "12761", "12537", "11777"]), &Selection::default());
        assert_eq!(blocked(&t), ["150585"]);
        assert!(event(&t, "150349").unresolved());
        // Wirtschaftsinformatik 021-82-2024, FS1: 148936 must take Mo 15:30 and 148130 Mo 13:45,
        // and 151699's Mo 11:30 meets 148935: the three cannot be placed together.
        let wi = ids(&["11109", "12160", "12229", "13977", "13980"]);
        let t = planned(&db, "2026W", &wi, &Selection::default());
        assert_eq!(blocked(&t), ["148936", "148130", "151699"]);
        assert_eq!(
            named(&t),
            [
                clash(("148934", 1), ("148130", 2), "2026-10-13", 14),
                clash(("148935", 1), ("151699", 1), "2026-10-05", 15),
                clash(("148936", 2), ("150199", 1), "2026-10-07", 15),
                clash(("148936", 1), ("151699", 3), "2026-10-05", 15),
                clash(("148130", 1), ("151699", 2), "2026-10-12", 14)
            ]
        );
        // 017-82-2017, FS5: 148661's single on 08.10. and its B-week Thursdays at 09:30 meet
        // 148041; the clash names the single, and both rows are hard.
        let t = planned(&db, "2026W", &ids(&["11540", "11538"]), &Selection::default());
        assert!(named(&t).contains(&clash(("148661", 3), ("148041", 1), "2026-10-08", 8)));
        let e148661 = t.events.iter().position(|e| e.id == "148661").unwrap();
        let hard: Vec<i64> = hard_rows(&t.events)
            .into_iter()
            .filter(|(e, _)| *e == e148661)
            .map(|(e, r)| t.events[e].rows[r].ord.unwrap())
            .collect();
        assert_eq!(hard, [3, 4]);
    }
}
