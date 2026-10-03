//! Texts of the Stundenplan's week and its days (`pages/studyplan/week.rs`): the Regelwoche as a
//! grid and a phone's list of days, the buttons of its slots, and the agenda „Termine". What QIS
//! says (an event's type, its title, rooms, a rhythm in its own words, the reason for a cancelled
//! date) stands as it says it; weekdays, dates and holidays are the data contract's
//! (`folia_locale`).

use folia_calendar::day::Day;
use folia_locale::Locale;

pub struct Texts {
    // ---- the Regelwoche ----
    /// A phone's line that opens the list of the week's days, before how many rows it holds:
    /// „Termine als Liste".
    pub days_toggle: &'static str,
    /// The weeks the Regelwoche shows, as the head's switch and a phone's carousel name them: the A
    /// weeks („A-Woche") …
    pub tab_a: &'static str,
    /// … the B weeks („B-Woche") …
    pub tab_b: &'static str,
    /// … and both („A/B").
    pub tab_ab: &'static str,
    /// The name of the switch of the weeks for screen readers: „Woche".
    pub week: &'static str,
    /// What a week of a phone's carousel is to a screen reader („Woche", a role: lower case in
    /// English), what the carousel is („Karussell"), and its name („Wochen").
    pub week_role: &'static str,
    pub carousel_role: &'static str,
    pub weeks: &'static str,
    /// The heading of what has no fixed time, under the week: „Ohne feste Zeit".
    pub no_fixed_time: &'static str,
    /// „Plan · Alle Termine": the plan's Regelwoche, or every Termin of its modules; the switch's
    /// name for screen readers: „Termine".
    pub plan: &'static str,
    pub all_sessions: &'static str,
    pub sessions_switch: &'static str,

    // ---- a slot and its line ----
    /// The end of a slot's tooltip: the other slots it overlaps, by their labels (joined with
    /// „, "): „überschneidet sich mit VL EvS".
    pub clashes_with: fn(&str) -> String,
    /// Why „Alle Termine" shows a slot faint, first in its tooltip and its line: its kinds switched
    /// off (the kinds' labels joined with „/"): „„Tutorium“ ausgeblendet" …
    pub kinds_hidden: fn(&str) -> String,
    /// … another option of its choice taken („andere Gruppe gewählt") …
    pub other_group_chosen: &'static str,
    /// … its event or Termin hidden („ausgeblendet").
    pub hidden: &'static str,
    /// The buttons of a slot (their names and tooltips): „✓" takes an option of a choice („Diese
    /// Gruppe nehmen") …
    pub take_group: &'static str,
    /// … „×" leaves it out („Nicht diese Gruppe") …
    pub not_this_group: &'static str,
    /// … the „✓" of the option taken takes the choice back („Gruppe abwählen") …
    pub drop_group: &'static str,
    /// … „×" hides one Termin of an event of several slots („Termin ausblenden") …
    pub hide_session: &'static str,
    /// … or the whole event, named by its type as QIS writes it: „Vorlesung ausblenden" …
    pub hide_event: fn(&str) -> String,
    /// … a faint slot's „✓" shows a kind switched off again („„Übung“ wieder einblenden") …
    pub show_kind: fn(&str) -> String,
    /// … or its Termin („Wieder einblenden").
    pub show_again: &'static str,
    /// An option of a choice not made yet, of how many: „1 von 4".
    pub one_of: fn(usize) -> String,
    /// How many single dates a slot or a Termin gathers: „1 Termin", „3 Termine".
    pub sessions: fn(usize) -> String,
    /// The weeks a Termin meets in, in words: A weeks („A-Woche"), B weeks („B-Woche"), every
    /// fourth week („4-wöch.").
    pub in_week_a: &'static str,
    pub in_week_b: &'static str,
    pub every_four_weeks: &'static str,
    /// The part of the lecture period a Termin meets in (the date written already): „bis 24.11." …
    pub until: fn(&str) -> String,
    /// … „ab 07.12.".
    pub from: fn(&str) -> String,

    // ---- the agenda ----
    /// The head of a week: its number and its days (written already): „KW 41 · 05.–11.10.2026".
    pub week_head: fn(u8, &str) -> String,
    /// The days from the first to the last, the month (and with `year` the year) said once where
    /// they are the same: „05.–11.10.2026", „26.10.–01.11.2026", „28.12.2026–03.01.2027".
    pub days: fn(Day, Day, bool) -> String,
    /// Weeks of a break with nothing in them, as one line (their days written already):
    /// „21.12.–03.01. vorlesungsfrei".
    pub break_line: fn(&str) -> String,
    /// After the head of a week with nothing in it: „keine Termine".
    pub no_dates: &'static str,
    /// The time of a date that takes the whole day („ganztägig") or has no time yet („Zeit
    /// offen").
    pub all_day: &'static str,
    pub time_open: &'static str,
    /// A date that does not take place, before the reason QIS gives: „fällt aus".
    pub cancelled: &'static str,
    /// The time of an exam that is something due (the time written already): „bis 24:00".
    pub due_by: fn(&str) -> String,
    /// An exam whose date QIS leaves open: „Termin offen".
    pub date_open: &'static str,
    /// An event QIS lists without any date: „ohne Termine".
    pub without_dates: &'static str,
    /// The heading of what the agenda cannot place on a day: „Ohne Datum".
    pub no_date: &'static str,
}

pub const DE: Texts = Texts {
    days_toggle: "Termine als Liste",
    tab_a: "A-Woche",
    tab_b: "B-Woche",
    tab_ab: "A/B",
    week: "Woche",
    week_role: "Woche",
    carousel_role: "Karussell",
    weeks: "Wochen",
    no_fixed_time: "Ohne feste Zeit",
    plan: "Plan",
    all_sessions: "Alle Termine",
    sessions_switch: "Termine",

    clashes_with: |others| format!("überschneidet sich mit {others}"),
    kinds_hidden: |kinds| format!("„{kinds}“ ausgeblendet"),
    other_group_chosen: "andere Gruppe gewählt",
    hidden: "ausgeblendet",
    take_group: "Diese Gruppe nehmen",
    not_this_group: "Nicht diese Gruppe",
    drop_group: "Gruppe abwählen",
    hide_session: "Termin ausblenden",
    hide_event: |kind| format!("{kind} ausblenden"),
    show_kind: |kind| format!("„{kind}“ wieder einblenden"),
    show_again: "Wieder einblenden",
    one_of: |n| format!("1 von {n}"),
    sessions: |n| if n == 1 { "1 Termin".to_string() } else { format!("{n} Termine") },
    in_week_a: "A-Woche",
    in_week_b: "B-Woche",
    every_four_weeks: "4-wöch.",
    until: |day| format!("bis {day}"),
    from: |day| format!("ab {day}"),

    week_head: |week, days| format!("KW {week} · {days}"),
    days: |first, last, year| days(first, last, year, Locale::De),
    break_line: |days| format!("{days} vorlesungsfrei"),
    no_dates: "keine Termine",
    all_day: "ganztägig",
    time_open: "Zeit offen",
    cancelled: "fällt aus",
    due_by: |time| format!("bis {time}"),
    date_open: "Termin offen",
    without_dates: "ohne Termine",
    no_date: "Ohne Datum",
};

pub const EN: Texts = Texts {
    days_toggle: "Sessions as a list",
    tab_a: "Week A",
    tab_b: "Week B",
    tab_ab: "A/B",
    week: "Week",
    week_role: "week",
    carousel_role: "carousel",
    weeks: "Weeks",
    no_fixed_time: "No fixed time",
    plan: "Plan",
    all_sessions: "All sessions",
    sessions_switch: "Sessions",

    clashes_with: |others| format!("clashes with {others}"),
    kinds_hidden: |kinds| format!("“{kinds}” hidden"),
    other_group_chosen: "another group chosen",
    hidden: "hidden",
    take_group: "Take this group",
    not_this_group: "Not this group",
    drop_group: "Deselect this group",
    hide_session: "Hide this session",
    hide_event: |kind| format!("Hide {kind}"),
    show_kind: |kind| format!("Show “{kind}” again"),
    show_again: "Show again",
    one_of: |n| format!("1 of {n}"),
    sessions: |n| if n == 1 { "1 session".to_string() } else { format!("{n} sessions") },
    in_week_a: "week A",
    in_week_b: "week B",
    every_four_weeks: "4-weekly",
    until: |day| format!("until {day}"),
    from: |day| format!("from {day}"),

    week_head: |week, days| format!("Week {week} · {days}"),
    days: |first, last, year| days(first, last, year, Locale::En),
    break_line: |days| format!("{days} · no lectures"),
    no_dates: "no dates",
    all_day: "all day",
    time_open: "time TBA",
    cancelled: "cancelled",
    due_by: |time| format!("by {time}"),
    date_open: "date TBA",
    without_dates: "no dates",
    no_date: "No date",
};

/// The days from `first` to `last` as `locale` writes a span of them, the month (and with `year`
/// the year) said once where both days share it: „08.–19.02.2027", „25.02.–05.03.2027",
/// „28.12.2026–08.01.2027"; "8–19 Feb 2027", "25 Feb–5 Mar 2027", "28 Dec 2026–8 Jan 2027".
/// Without `year`: „08.–19.02.", „28.01.–05.02."; "8–19 Feb", "28 Jan–5 Feb". The spans of the
/// other groups of the Stundenplan are this one.
pub fn days(first: Day, last: Day, year: bool, locale: Locale) -> String {
    let ((y1, m1, d1), (y2, m2, _)) = (first.ymd(), last.ymd());
    let whole = |day: Day| if year { day.date(locale) } else { day.day_month(locale) };
    if (y1, m1) == (y2, m2) {
        // Only the first day's number: German writes it before the month („08.–"), English
        // before the dash ("8–").
        let day = match locale {
            Locale::De => format!("{d1:02}."),
            Locale::En => d1.to_string(),
        };
        format!("{day}–{}", whole(last))
    } else if y1 == y2 || !year {
        format!("{}–{}", first.day_month(locale), whole(last))
    } else {
        format!("{}–{}", first.date(locale), last.date(locale))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_of_days_says_its_month_and_year_once() {
        let d = |iso: &str| Day::parse(iso).unwrap();
        let spans = |locale: Locale, year: bool| -> Vec<String> {
            [("2027-02-08", "2027-02-19"), ("2027-02-25", "2027-03-05"), ("2026-12-28", "2027-01-08")].iter().map(|(a, b)| days(d(a), d(b), year, locale)).collect()
        };
        assert_eq!(spans(Locale::De, true), ["08.–19.02.2027", "25.02.–05.03.2027", "28.12.2026–08.01.2027"]);
        assert_eq!(spans(Locale::En, true), ["8–19 Feb 2027", "25 Feb–5 Mar 2027", "28 Dec 2026–8 Jan 2027"]);
        assert_eq!(spans(Locale::De, false), ["08.–19.02.", "25.02.–05.03.", "28.12.–08.01."]);
        assert_eq!(spans(Locale::En, false), ["8–19 Feb", "25 Feb–5 Mar", "28 Dec–8 Jan"]);
    }
}
