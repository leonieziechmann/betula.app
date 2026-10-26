//! The choices the plan leaves to the visitor, as „Offen" lists them and a choice in focus shows
//! them (folia/design/kalender/konzepte.html and handy.html; owner, 2026-10-26: „A mit Fokus und
//! Spur aus C", „nimm H1 mit Vergleichen"): which choices are open, the address of the one in
//! focus, its groups all in the week whether ticked or not, and what a group does to its day —
//! alone at the campus, the time before and after it, another campus, what it meets.
//!
//! A choice in focus is the groups of an event (`Attendance::OneOf`), ticked as many as one likes
//! (`Event::chosen`); a choice among a module's events (`Timetable::choices`) is listed, and its
//! module's panel decides it. Pure functions of a `Timetable`, as the rest of the crate.

use folia_calendar::select::HiddenBy;
use folia_model::labels::Campus;

use crate::model::{Attendance, Event, Timetable};
use crate::views::WeekItem;

/// The address of an event's choice (`wahl=`): `e` and the event's number in QIS.
pub fn code(event: &Event) -> String {
    format!("e{}", event.id)
}

/// The event of a choice by its address: one shown, whose rows are groups to tick (two or more).
pub fn find(table: &Timetable, code: &str) -> Option<usize> {
    let id = code.strip_prefix('e')?;
    table.events.iter().position(|event| event.id == id && event.hidden.is_none() && groups(event) >= 2)
}

/// How many groups an event's choice has; 0 for an event whose rows are all required.
pub fn groups(event: &Event) -> usize {
    match &event.attendance {
        Attendance::OneOf { options, .. } => options.len(),
        Attendance::All => 0,
    }
}

/// A choice „Offen" lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Open {
    /// The groups of an event, none ticked (`Event::unresolved`): the event, how many groups are
    /// shown, and whether none of them is free of the plan (`Timetable::blocked`).
    Groups { event: usize, options: usize, blocked: bool },
    /// A choice among a module's events still open: the choice (into `Timetable::choices`), its
    /// first event still to choose, how many one still takes of how many, and whether too few are
    /// free (`Timetable::blocked_choices`).
    Across { choice: usize, event: usize, take: usize, options: usize, blocked: bool },
}

impl Open {
    /// None of its options is free of the plan (too few of a choice that takes several).
    pub fn blocked(&self) -> bool {
        match self {
            Open::Groups { blocked, .. } | Open::Across { blocked, .. } => *blocked,
        }
    }

    /// The event it is named by: the groups' own, a choice's first option still to choose.
    pub fn event(&self) -> usize {
        match self {
            Open::Groups { event, .. } | Open::Across { event, .. } => *event,
        }
    }
}

/// The plan's open choices as „Offen" lists them: the groups of its events in the plan's order,
/// then the choices among a module's events in theirs; those of which nothing is free first.
pub fn open(table: &Timetable) -> Vec<Open> {
    let mut out: Vec<Open> = Vec::new();
    for (index, event) in table.events.iter().enumerate() {
        if event.hidden.is_some() || !event.unresolved() {
            continue;
        }
        out.push(Open::Groups { event: index, options: event.visible_options().len(), blocked: table.blocked.contains(&index) });
    }
    for (index, across) in table.choices.iter().enumerate() {
        let choosable = across.choosable(&table.events);
        let Some(event) = choosable.first().and_then(|option| across.options.get(*option)?.first().copied()) else { continue };
        out.push(Open::Across { choice: index, event, take: across.take(&table.events), options: choosable.len(), blocked: table.blocked_choices.contains(&index) });
    }
    // Stable: within each part the plan's order stays.
    out.sort_by_key(|open| !open.blocked());
    out
}

/// The choices of groups of the plan in its order, open or ticked: where „Weiter" goes from the one
/// in focus (the next one still open after it, else the first open one before it).
pub fn next_open(table: &Timetable, from: usize) -> Option<usize> {
    let open: Vec<usize> = open(table).iter().filter_map(|open| match open {
        Open::Groups { event, .. } => Some(*event),
        Open::Across { .. } => None,
    }).collect();
    open.iter().copied().find(|event| *event > from).or_else(|| open.iter().copied().find(|event| *event != from))
}

/// The timetable as the week shows a choice in focus: its groups all there again, ticked or not,
/// each dashed as a group to choose (`WeekItem::alt`), whatever another group's tick hid. The
/// ticks themselves stay the plan's, which the page reads from the timetable it was given.
pub fn focus_table(table: &Timetable, event: usize) -> Timetable {
    let mut focus = table.clone();
    if let Some(of) = focus.events.get_mut(event) {
        of.chosen.clear();
        for row in &mut of.rows {
            if row.hidden == Some(HiddenBy::Choice) {
                row.hidden = None;
            }
        }
    }
    focus
}

/// A Termin of the plan beside a group on its weekday.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Beside {
    pub event: usize,
    pub row: usize,
    pub from: u16,
    pub to: u16,
}

/// What a group does to its weekday: the plan's Termine of that day that stand firm (no group of
/// an open choice, no extra, not its own event), what it meets, what comes before and after it
/// with the time between, and whether one of those is held at another campus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Around {
    pub day: u8,
    pub from: u16,
    pub to: u16,
    /// The day's firm Termine by start.
    pub day_items: Vec<Beside>,
    /// The events of those it overlaps.
    pub meets: Vec<usize>,
    /// The Termin before it that day and the minutes between, and the one after it.
    pub before: Option<(Beside, u16)>,
    pub after: Option<(Beside, u16)>,
    /// The one before or after it is held at another campus than the group.
    pub away: bool,
}

impl Around {
    /// The only Termin of its day: a way to the campus for it alone.
    pub fn alone(&self) -> bool {
        self.day_items.is_empty()
    }
}

/// What the group whose first Termin is the row `row` of `event` does to its weekday, against the
/// plan's Regelwoche `week` (`Timetable::regular_week`). `None` for a row without a weekday or a
/// time.
pub fn around(table: &Timetable, week: &[WeekItem], event: usize, row: usize) -> Option<Around> {
    let of = table.events.get(event)?.rows.get(row)?;
    let (day, from, to) = (of.slot_day()?, of.from?, of.to?);
    let mut day_items: Vec<Beside> = Vec::new();
    for item in week.iter().filter(|item| item.day == day && item.event != event && item.alt.is_none() && !item.extra) {
        let beside = Beside { event: item.event, row: item.row, from: item.from, to: item.to };
        if !day_items.iter().any(|seen| seen.event == beside.event && seen.from == beside.from && seen.to == beside.to) {
            day_items.push(beside);
        }
    }
    day_items.sort_by_key(|beside| (beside.from, beside.to, beside.event));
    let mut meets: Vec<usize> = Vec::new();
    for beside in day_items.iter().filter(|beside| beside.from < to && from < beside.to) {
        if !meets.contains(&beside.event) {
            meets.push(beside.event);
        }
    }
    let before = day_items.iter().filter(|beside| beside.to <= from).max_by_key(|beside| beside.to).map(|beside| (*beside, from - beside.to));
    let after = day_items.iter().filter(|beside| beside.from >= to).min_by_key(|beside| beside.from).map(|beside| (*beside, beside.from - to));
    let campus = |event: usize, row: usize| -> Option<Campus> { table.events.get(event)?.rows.get(row)?.date.campus.as_ref()?.known() };
    let own = of.date.campus.as_ref().and_then(|campus| campus.known());
    let away = own.is_some_and(|own| [before, after].iter().flatten().any(|(beside, _)| campus(beside.event, beside.row).is_some_and(|other| other != own)));
    Some(Around { day, from, to, day_items, meets, before, after, away })
}

#[cfg(test)]
mod tests {
    use folia_calendar::select::Selection;

    use super::*;
    use crate::model::tests::{table, teaching};

    /// 12104's lecture Tue 09:15 and its exercise in three groups: Mon 11:30, Wed 11:30 and Tue
    /// 09:15 under the lecture; 12107's lecture Wed 13:45.
    fn plan(selection: &Selection) -> Timetable {
        let rows = vec![
            teaching("12104", "1", 1, "Vorlesung", 2, "09:15", "10:45"),
            teaching("12104", "2", 1, "Übung", 1, "11:30", "13:00").group("1-Gruppe"),
            teaching("12104", "2", 2, "Übung", 3, "11:30", "13:00").group("2-Gruppe"),
            teaching("12104", "2", 3, "Übung", 2, "09:15", "10:45").group("3-Gruppe"),
            teaching("12107", "3", 1, "Vorlesung", 3, "13:45", "15:15"),
        ];
        table(&rows, &["12104", "12107"], &[], selection)
    }

    fn exercise(table: &Timetable) -> usize {
        table.events.iter().position(|event| event.id == "2").unwrap()
    }

    #[test]
    fn an_exercise_in_groups_is_open_until_one_is_ticked() {
        let open_plan = plan(&Selection::default());
        let e = exercise(&open_plan);
        assert_eq!(code(&open_plan.events[e]), "e2");
        assert_eq!(find(&open_plan, "e2"), Some(e));
        assert_eq!(find(&open_plan, "e1"), None, "a lecture has no groups");
        assert_eq!(find(&open_plan, "e9"), None);
        let listed = open(&open_plan);
        assert_eq!(listed.len(), 1);
        assert!(matches!(listed[0], Open::Groups { event, options: 3, .. } if event == e));
        // Ticked, the choice is no longer open, and the address still finds it.
        let key = open_plan.events[e].rows[0].key.unwrap();
        let ticked = plan(&Selection { chosen_rows: [key].into_iter().collect(), ..Default::default() });
        assert!(open(&ticked).is_empty());
        assert_eq!(find(&ticked, "e2"), Some(e));
    }

    #[test]
    fn the_week_of_a_choice_in_focus_shows_every_group() {
        let open_plan = plan(&Selection::default());
        let e = exercise(&open_plan);
        let key = open_plan.events[e].rows[0].key.unwrap();
        let ticked = plan(&Selection { chosen_rows: [key].into_iter().collect(), ..Default::default() });
        let groups = |table: &Timetable| table.regular_week().iter().filter(|item| item.event == e).count();
        assert_eq!(groups(&ticked), 1, "the plan shows the group ticked");
        let focus = focus_table(&ticked, e);
        assert_eq!(groups(&focus), 3);
        assert!(focus.regular_week().iter().filter(|item| item.event == e).all(|item| item.alt.is_some()));
        // The ticks stay the plan's.
        assert!(ticked.events[e].chosen.contains(&0));
    }

    #[test]
    fn a_group_says_what_it_does_to_its_day() {
        let table = plan(&Selection::default());
        let e = exercise(&table);
        let week = table.regular_week();
        let row_of = |day: u8| table.events[e].rows.iter().position(|row| row.slot_day() == Some(day)).unwrap();
        // Monday 11:30: alone that day.
        let monday = around(&table, &week, e, row_of(1)).unwrap();
        assert!(monday.alone() && monday.meets.is_empty() && monday.before.is_none() && monday.after.is_none());
        // Wednesday 11:30: 45 minutes before the lecture of 12107.
        let wednesday = around(&table, &week, e, row_of(3)).unwrap();
        let lecture = table.events.iter().position(|event| event.id == "3").unwrap();
        assert_eq!(wednesday.after.map(|(beside, gap)| (beside.event, gap)), Some((lecture, 45)));
        assert!(!wednesday.away);
        // Tuesday 09:15: under the lecture of its own module.
        let tuesday = around(&table, &week, e, row_of(2)).unwrap();
        let own = table.events.iter().position(|event| event.id == "1").unwrap();
        assert_eq!(tuesday.meets, vec![own]);
    }

    #[test]
    fn next_goes_to_the_next_open_choice() {
        let table = plan(&Selection::default());
        let e = exercise(&table);
        assert_eq!(next_open(&table, e), None, "the only choice");
    }
}
