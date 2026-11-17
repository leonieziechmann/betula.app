//! A module's events as rows: the overview of a Termin names them in its segments, and the
//! Stundenplan's list of modules lists them under each module (owner, 2026-11-17: „dass bei den
//! Modulen die Kategorien vl, Übung, prak sind und man da die Übersicht auswählen kann, wo man alles
//! durchklicken kann"). One function says which events, in which order, under which word and
//! entered by which Termin (`module_events`), so that a row of the list and a segment of the
//! overview cannot differ; another says what a row tells of its event, as values (`summary`), which
//! the list words.
//!
//! An event is a row where it has a Termin; those for another place or program (the other town's
//! course, another program's option) are left out as the module's page leaves them out, unless it is
//! the one a view is about. Pure functions of a `Timetable`, as the rest of the crate.

use folia_calendar::day::clock;
use folia_calendar::kind::EventKind;
use folia_calendar::rowkey::RowKey;
use folia_calendar::select::HiddenBy;
use folia_locale::Locale;

use crate::choice;
use crate::clash::Weeks;
use crate::model::{Attendance, Event, Row, Timetable};
use crate::termin;
use crate::views::{hidden_days, type_text};

/// An event of a module as a row of the module list and a segment of the overview name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventRow {
    /// Into `Timetable::events`.
    pub event: usize,
    /// Its word: what QIS calls it (`views::type_text`), and of two of a kind what tells them apart
    /// („Übung · Mo").
    pub label: String,
    /// Its Termin the plan holds (a group ticked), else its first: where the overview opens at it.
    pub key: RowKey,
}

/// The events of module `id` that have a Termin, kind first (`kind_rank`), within a kind in the
/// timetable's order; those for another place or program (`HiddenBy::Town`, `Course`, `Program`) left out, unless it is `shown`, the one a view
/// is about. One event is a row too.
pub fn module_events(table: &Timetable, id: &str, shown: Option<usize>, locale: Locale) -> Vec<EventRow> {
    let theirs = |event: &Event| matches!(event.hidden, Some(HiddenBy::Town(_) | HiddenBy::Course(_) | HiddenBy::Program));
    let mut events: Vec<(usize, &Event, Vec<termin::Session>)> = table
        .events
        .iter()
        .enumerate()
        .filter(|(index, event)| event.modules.iter().any(|module| module == id) && (Some(*index) == shown || !theirs(event)))
        .map(|(index, event)| (index, event, termin::sessions(event)))
        .filter(|(_, _, sessions)| !sessions.is_empty())
        .collect();
    // Kind first, as the owner named them (2026-11-17: „die Kategorien vl, Übung, prak"): lectures,
    // exercises, seminars, practicals, projects, tutorials, then the rest; within a kind the
    // timetable's order.
    events.sort_by_key(|(_, event, _)| kind_rank(event));
    let kinds: Vec<String> = events.iter().map(|(_, event, _)| type_text(event, locale)).collect();
    // The weekday and the time of each event's first Termin, which tell two of a kind apart.
    let firsts: Vec<(Option<String>, Option<String>)> = events
        .iter()
        .map(|(_, event, sessions)| {
            let row = sessions.first().and_then(|session| event.rows.get(session.row));
            let day = row.and_then(Row::slot_day).map(|day| weekday(day, locale).to_string());
            let time = row.and_then(|row| Some(format!("{} {}", weekday(row.slot_day()?, locale), clock(row.from?))));
            (day, time)
        })
        .collect();
    let labels = tell_apart(&kinds, &firsts);
    events
        .iter()
        .zip(labels)
        .filter_map(|((index, event, sessions), label)| {
            // Its Termin the plan holds (a group ticked), else its first.
            let held = |session: &&termin::Session| event.rows.get(session.row).and_then(|row| row.option).is_some_and(|option| event.chosen.contains(&option));
            let key = sessions.iter().find(held).or(sessions.first())?.key;
            Some(EventRow { event: *index, label, key })
        })
        .collect()
}

/// The words of the rows: each its kind, of two of a kind the weekday of their first Termin
/// („Übung · Mo"), where that is the same too its time („Übung · Mo 09:15"), where that is the same
/// too a number in their order („Übung · Mo 09:15 2").
pub fn tell_apart(kinds: &[String], firsts: &[(Option<String>, Option<String>)]) -> Vec<String> {
    // How often a word stands among the words.
    let count = |labels: &[String], label: &String| labels.iter().filter(|other| *other == label).count();
    let by_day: Vec<String> = kinds
        .iter()
        .zip(firsts)
        .map(|(kind, (day, _))| match day {
            Some(day) if count(kinds, kind) > 1 => format!("{kind} · {day}"),
            _ => kind.clone(),
        })
        .collect();
    let by_time: Vec<String> = kinds
        .iter()
        .zip(firsts)
        .zip(&by_day)
        .map(|((kind, (_, time)), label)| match time {
            Some(time) if label != kind && count(&by_day, label) > 1 => format!("{kind} · {time}"),
            _ => label.clone(),
        })
        .collect();
    by_time
        .iter()
        .enumerate()
        .map(|(at, label)| match count(&by_time, label) > 1 {
            true => format!("{label} {}", by_time.iter().take(at + 1).filter(|other| *other == label).count()),
            false => label.clone(),
        })
        .collect()
}

/// Why the plan shows nothing of an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Out {
    /// The student hid it (an `e` line): its eye brings it back.
    Hidden,
    /// A kind switched off for the whole semester hides it (a `k` line of before 2026-11-04).
    Kind,
    /// Another option of a choice among the module's events was taken: not hidden, not chosen.
    Unchosen,
}

/// What a row says of its event, as values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    /// Its weekly Termine the plan holds, in the week's order: weekday, start, weeks. Of an event of
    /// groups the ticked ones, and what is required beside them; none while its choice is open.
    pub weekly: Vec<(u8, u16, Weeks)>,
    /// The single dates of the Termine it holds that do not recur.
    pub dates: usize,
    /// Of an event of groups: how many are ticked, of how many (`choice::groups`).
    pub groups: Option<(usize, usize)>,
    /// It is an option of a choice among the module's events that is still open.
    pub option_open: bool,
    /// An extra beside the module's compulsory part, attended by whoever wants to.
    pub extra: bool,
    pub out: Option<Out>,
    /// Termine (a series as one) and single dates of it the student hid while it shows: what
    /// `views::hidden_count` counts of this event.
    pub hidden: usize,
}

/// What a row says of `event`: the Termine the plan holds of it, its groups, whether it is out of
/// the plan and why, and what of it is hidden.
pub fn summary(event: &Event) -> Summary {
    let out = event.hidden.map(|hidden| match hidden {
        HiddenBy::Event => Out::Hidden,
        HiddenBy::Kinds => Out::Kind,
        _ => Out::Unchosen,
    });
    let mut weekly: Vec<(u8, u16, Weeks)> = Vec::new();
    let mut days = Vec::new();
    let mut hidden = 0;
    for session in termin::sessions(event) {
        let rows: Vec<&Row> = termin::termin_rows(event, session.row).into_iter().filter_map(|index| event.rows.get(index)).collect();
        let Some(first) = event.rows.get(session.row) else { continue };
        // Held: its Termin is not hidden, and it is required or an option ticked.
        let held = first.hidden.is_none() && first.option.is_none_or(|option| event.chosen.contains(&option));
        if out.is_none() {
            if !rows.is_empty() && rows.iter().all(|row| row.hidden == Some(HiddenBy::Row)) {
                hidden += 1;
            } else {
                hidden += hidden_days(rows.iter().copied()).len();
            }
        }
        if !held {
            continue;
        }
        match (Weeks::of(first.every), first.slot_day(), first.from) {
            (Some(weeks), Some(day), Some(from)) => weekly.push((day, from, weeks)),
            _ => days.extend(rows.iter().flat_map(|row| row.occ.days.iter().copied())),
        }
    }
    days.sort_unstable();
    days.dedup();
    let groups = match &event.attendance {
        Attendance::OneOf { .. } if event.across.is_none() => Some((event.chosen.len(), choice::groups(event))),
        _ => None,
    };
    Summary { weekly, dates: days.len(), groups, option_open: event.across.is_some_and(|alt| alt.open), extra: event.extra, out, hidden }
}

/// Where an event's rows stand among a module's: the place of its first kind in the kinds' order
/// (`EventKind::ALL`: Vorlesung, Übung, Seminar, Praktikum, Projekt, Tutorium, the rest), an event
/// of no kind Betula knows last.
pub fn kind_rank(event: &Event) -> usize {
    let first = event.kinds.iter().next();
    first.and_then(|kind| EventKind::ALL.iter().position(|other| *other == kind)).unwrap_or(EventKind::ALL.len())
}

fn weekday(day: u8, locale: Locale) -> &'static str {
    locale.texts().weekday_short(i64::from(day)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use folia_calendar::select::Selection;

    use super::*;
    use crate::model::tests::{choice_row, choosing, d, table, teaching, Fixture};

    const DE: Locale = Locale::De;

    fn labels(rows: &[EventRow]) -> Vec<&str> {
        rows.iter().map(|row| row.label.as_str()).collect()
    }

    /// EvS of the example week: a lecture on Tuesday and Friday, an exercise of four groups.
    fn evs() -> Vec<Fixture> {
        vec![
            teaching("12104", "148701", 1, "Vorlesung", 2, "11:30", "13:00"),
            teaching("12104", "148701", 2, "Vorlesung", 5, "09:15", "10:45"),
            teaching("12104", "148369", 1, "Übung", 1, "15:30", "17:00").group("1-Gruppe"),
            teaching("12104", "148369", 2, "Übung", 1, "17:30", "19:00").group("2-Gruppe"),
            teaching("12104", "148369", 3, "Übung", 2, "15:30", "17:00").group("3-Gruppe"),
            teaching("12104", "148369", 4, "Übung", 2, "17:30", "19:00").group("4-Gruppe"),
        ]
    }

    fn of(t: &Timetable, id: &str) -> Event {
        t.events.iter().find(|event| event.id == id).cloned().unwrap()
    }

    #[test]
    fn a_modules_rows_are_its_events_with_a_termin() {
        // One title for both, as QIS gives it: the order is the kinds', not the titles'.
        let rows: Vec<Fixture> = evs().into_iter().map(|row| row.title("Einführung in die Softwaretechnik")).collect();
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let found = module_events(&t, "12104", None, DE);
        assert_eq!(labels(&found), ["Vorlesung", "Übung"]);
        assert!(found.iter().all(|row| t.events[row.event].modules == ["12104"]));
        assert!(module_events(&t, "11112", None, DE).is_empty());
    }

    #[test]
    fn a_modules_rows_come_kind_first() {
        // By their titles („{kind} {id}") the practical and the tutorial would come first.
        let rows = [
            teaching("A", "11", 1, "Übung", 1, "09:15", "10:45"),
            teaching("A", "13", 1, "Tutorium", 1, "11:30", "13:00"),
            teaching("A", "10", 1, "Vorlesung", 2, "09:15", "10:45"),
            teaching("A", "12", 1, "Praktikum", 3, "13:45", "15:15"),
            teaching("A", "14", 1, "Übung", 4, "09:15", "10:45"),
        ];
        let t = table(&rows, &["A"], &[], &Selection::default());
        let found = module_events(&t, "A", None, DE);
        assert_eq!(labels(&found), ["Vorlesung", "Übung · Mo", "Übung · Do", "Praktikum", "Tutorium"]);
    }

    #[test]
    fn a_module_of_one_event_has_its_row() {
        let rows = [teaching("A", "10", 1, "Vorlesung", 1, "09:15", "10:45")];
        let t = table(&rows, &["A"], &[], &Selection::default());
        let found = module_events(&t, "A", None, DE);
        assert_eq!(found, [EventRow { event: 0, label: "Vorlesung".into(), key: rows[0].key() }]);
    }

    #[test]
    fn two_of_a_kind_are_told_apart() {
        let kinds = |kinds: &[&str]| kinds.iter().map(|kind| kind.to_string()).collect::<Vec<_>>();
        let first = |day: &str, time: &str| (Some(day.to_string()), Some(format!("{day} {time}")));
        assert_eq!(tell_apart(&kinds(&["Vorlesung", "Übung"]), &[first("Mo", "09:15"), first("Di", "11:30")]), ["Vorlesung", "Übung"]);
        assert_eq!(tell_apart(&kinds(&["Vorlesung", "Übung", "Übung"]), &[first("Mo", "09:15"), first("Di", "11:30"), first("Mi", "11:30")]), ["Vorlesung", "Übung · Di", "Übung · Mi"]);
        assert_eq!(tell_apart(&kinds(&["Übung", "Übung"]), &[first("Mo", "09:15"), first("Mo", "13:45")]), ["Übung · Mo 09:15", "Übung · Mo 13:45"]);
        assert_eq!(tell_apart(&kinds(&["Übung", "Übung"]), &[first("Mo", "09:15"), first("Mo", "09:15")]), ["Übung · Mo 09:15 1", "Übung · Mo 09:15 2"]);
        assert_eq!(tell_apart(&kinds(&["Projekt", "Seminar", "Projekt"]), &[(None, None), (None, None), (None, None)]), ["Projekt 1", "Seminar", "Projekt 2"]);
        // In a timetable: two exercises on Monday and Wednesday.
        let rows = [teaching("A", "10", 1, "Übung", 1, "09:15", "10:45"), teaching("A", "11", 1, "Übung", 3, "09:15", "10:45")];
        let t = table(&rows, &["A"], &[], &Selection::default());
        assert_eq!(labels(&module_events(&t, "A", None, DE)), ["Übung · Mo", "Übung · Mi"]);
    }

    #[test]
    fn another_places_event_is_no_row_unless_it_is_the_one_shown() {
        use crate::model::tests::listed_at;
        // The program is listed at the second option: the first is another program's.
        let rows = [teaching("H", "61", 1, "Übung", 2, "07:30", "09:00").option(1), teaching("H", "62", 1, "Übung", 3, "13:45", "15:15").option(2)];
        let choices = [choice_row("H", "program", 1, 1, "61"), choice_row("H", "program", 1, 2, "62")];
        let t = choosing(&rows, &["H"], &choices, &[listed_at("H", 2)], &Selection::default());
        assert_eq!(t.events[0].hidden, Some(HiddenBy::Program));
        assert_eq!(module_events(&t, "H", None, DE).iter().map(|row| row.event).collect::<Vec<_>>(), [1]);
        assert_eq!(module_events(&t, "H", Some(0), DE).iter().map(|row| row.event).collect::<Vec<_>>(), [0, 1]);
    }

    #[test]
    fn an_event_without_a_termin_is_no_row() {
        // QIS lists the exercise without a date row.
        let mut bare = teaching("A", "11", 1, "Übung", 2, "09:15", "10:45").undated();
        bare.0.ord = None;
        let rows = [teaching("A", "10", 1, "Vorlesung", 1, "09:15", "10:45"), bare];
        let t = table(&rows, &["A"], &[], &Selection::default());
        assert!(t.events.iter().any(|event| event.id == "11" && event.rows.is_empty()));
        assert_eq!(labels(&module_events(&t, "A", None, DE)), ["Vorlesung"]);
    }

    #[test]
    fn a_row_opens_at_the_termin_the_plan_holds() {
        let rows = evs();
        let open = table(&rows, &["12104"], &[], &Selection::default());
        let exercise = |t: &Timetable| module_events(t, "12104", None, DE).into_iter().find(|row| row.label == "Übung").unwrap().key;
        // Open: its first group, Monday 15:30.
        assert_eq!(exercise(&open), rows[2].key());
        // The Tuesday group ticked: that one.
        let ticked = table(&rows, &["12104"], &[], &Selection { chosen_rows: [rows[4].key()].into(), ..Selection::default() });
        assert_eq!(exercise(&ticked), rows[4].key());
    }

    #[test]
    fn a_lecture_says_its_termine() {
        let rows = evs();
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let lecture = summary(&of(&t, "148701"));
        assert_eq!(lecture, Summary { weekly: vec![(2, 690, Weeks::All), (5, 555, Weeks::All)], ..Summary::default() });
        // Every other week: the week's letter.
        let a = [teaching("A", "10", 1, "Vorlesung", 2, "07:30", "09:00").rhythm("week_a")];
        let t = table(&a, &["A"], &[], &Selection::default());
        assert_eq!(summary(&t.events[0]).weekly, [(2, 450, Weeks::A)]);
    }

    #[test]
    fn groups_open_and_ticked() {
        let rows = evs();
        let open = table(&rows, &["12104"], &[], &Selection::default());
        assert_eq!(summary(&of(&open, "148369")), Summary { groups: Some((0, 4)), ..Summary::default() });
        let ticked = table(&rows, &["12104"], &[], &Selection { chosen_rows: [rows[2].key()].into(), ..Selection::default() });
        assert_eq!(summary(&of(&ticked, "148369")), Summary { weekly: vec![(1, 930, Weeks::All)], groups: Some((1, 4)), ..Summary::default() });
    }

    #[test]
    fn an_extra_says_so() {
        let rows = [teaching("A", "10", 1, "Tutorium", 2, "15:30", "17:00").extra()];
        let t = table(&rows, &["A"], &[], &Selection::default());
        assert!(summary(&t.events[0]).extra);
    }

    #[test]
    fn an_event_hidden_whole_and_one_hidden_in_part() {
        let rows = [teaching("A", "10", 1, "Vorlesung", 1, "13:45", "15:15"), teaching("A", "10", 2, "Vorlesung", 2, "13:45", "15:15"), teaching("A", "11", 1, "Tutorium", 2, "15:30", "17:00")];
        let selection = Selection { hidden_events: [11].into(), hidden_rows: [rows[1].key()].into(), hidden_dates: [(rows[0].key(), d("2026-11-23"))].into(), ..Selection::default() };
        let t = table(&rows, &["A"], &[], &selection);
        // The Tuesday hidden and a Monday: one Termin and one date, the Monday still held.
        assert_eq!(summary(&of(&t, "10")), Summary { weekly: vec![(1, 825, Weeks::All)], hidden: 2, ..Summary::default() });
        assert_eq!(summary(&of(&t, "11")).out, Some(Out::Hidden));
        // A kind switched off.
        use folia_calendar::kind::{EventKind, KindSet};
        let kinds = table(&rows, &["A"], &[], &Selection { hidden_kinds: KindSet::default().with(EventKind::Tutorial), ..Selection::default() });
        assert_eq!(summary(&of(&kinds, "11")).out, Some(Out::Kind));
    }

    #[test]
    fn single_dates_alone_are_counted() {
        let single = |ord: i64, on: &str| teaching("A", "10", ord, "Übung", i64::from(d(on).weekday()), "13:45", "15:15").rhythm("single").range(on, on);
        let rows = [single(1, "2026-10-15"), single(2, "2026-11-12"), single(3, "2026-12-10")];
        let t = table(&rows, &["A"], &[], &Selection::default());
        assert_eq!(summary(&t.events[0]), Summary { dates: 3, ..Summary::default() });
    }

    #[test]
    fn an_event_without_a_time() {
        let mut row = teaching("A", "10", 1, "Seminar", 3, "09:15", "10:45");
        (row.0.date.start_time, row.0.date.end_time) = (None, None);
        let t = table(&[row], &["A"], &[], &Selection::default());
        let said = summary(&t.events[0]);
        assert!(said.weekly.is_empty() && said.out.is_none() && said.groups.is_none(), "{said:?}");
    }

    #[test]
    fn an_option_another_was_taken_over() {
        let rows = [
            teaching("H", "60", 1, "Vorlesung", 1, "15:30", "17:00"),
            teaching("H", "61", 1, "Übung", 2, "07:30", "09:00").option(1),
            teaching("H", "62", 1, "Übung", 3, "13:45", "15:15").option(2),
        ];
        let choices = [choice_row("H", "group", 1, 1, "61"), choice_row("H", "group", 1, 2, "62")];
        let open = choosing(&rows, &["H"], &choices, &[], &Selection::default());
        assert!(summary(&of(&open, "61")).option_open && summary(&of(&open, "61")).weekly == [(2, 450, Weeks::All)]);
        let taken = choosing(&rows, &["H"], &choices, &[], &Selection { chosen_rows: [rows[2].key()].into(), ..Selection::default() });
        assert_eq!(summary(&of(&taken, "61")).out, Some(Out::Unchosen));
        assert!(!summary(&of(&taken, "62")).option_open);
        // An option taken over is a row all the same: the overview has its segment.
        assert_eq!(labels(&module_events(&taken, "H", None, DE)).len(), 3);
    }
}
