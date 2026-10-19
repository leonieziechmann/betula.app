//! The choices among a module's events on its page, and its extras (folia/design/wahlangebote/
//! konzept.html; owner, 2026-10-19: „Passt so, nimm deine Vorschläge und bau es"; the choices are
//! Radix's, `folia_timetable::choices`): which options the page shows for whom, how, and the words
//! of the line over the week, of the list's headings and of a module's row in a list.
//!
//! - One event more than once, each for other programs (`ChoiceKind::Program`): for **Mein
//!   Studiengang** the options QIS lists it at, the others behind „Auch zeigen"; for **another
//!   program** the page stands for its options highlighted, the others faint; for **neither**, for
//!   a program QIS lists at none or at each, and always on the server (R9), every option open, each
//!   with whom QIS lists it for.
//! - In groups, or a topic to choose: every option open; where QIS lists the program at some,
//!   those first and the others fainter („Optionen, bei denen QIS Mein Studiengang nicht nennt,
//!   stehen blasser und zuletzt; wählbar bleiben sie").
//! - An option to choose is dashed in the week, and one of a choice of more than `WEEK_OPTIONS`
//!   stands in the list alone; an option taken is drawn as what everyone attends. An extra is
//!   dotted, „+" before its kind, and listed under „Zusätzlich, freiwillig".

use std::collections::BTreeSet;

use folia_calendar::kind::{kinds_of, EventKind};
use folia_design::format;
use folia_model::labels::{ChoiceBasis, ChoiceKind};
use folia_model::rows::{ChoiceSummary, Program};
use folia_model::rows_detail::EventDate;
use folia_timetable::choices::{Choice, Choices, Opt, Size};

use crate::courses::{program_name, Undecided};
use crate::i18n::module::{Noun, NounOf};
use crate::i18n::Texts;

/// The most options of a choice the week shows; a choice of more stands in the list alone
/// (owner, 2026-10-19, the concept's proposal: „bis zu vier gestrichelt in der Woche, mehr nur in
/// der Liste").
pub(crate) const WEEK_OPTIONS: usize = 4;

/// A choice of more than `FOLD_OPTIONS` options lists its first `LISTED_OPTIONS` and folds the
/// others away („9 weitere anzeigen").
pub(crate) const FOLD_OPTIONS: usize = 6;
pub(crate) const LISTED_OPTIONS: usize = 5;

/// How a module's page shows the options of one choice.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OptionView {
    /// Every option open; `about` says why where the page stands for a program.
    Open { about: Option<Undecided> },
    /// The options `own` QIS lists Mein Studiengang (`program`) at, the others behind „Auch
    /// zeigen"; as many as one takes are taken, more are open.
    Mine { program: String, own: BTreeSet<u32> },
    /// The same for another program the page stands for, the others faint.
    Highlight { program: String, own: BTreeSet<u32> },
    /// Every option open, those QIS lists the program at (`own`) first, the others fainter.
    Named { program: String, own: BTreeSet<u32> },
}

/// How an option shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OptState {
    /// Taken: as what everyone attends.
    Taken,
    /// One to choose among.
    Open,
    /// Another program's, or one QIS does not list the program at.
    Faint,
    /// Another program's, behind „Auch zeigen".
    Hidden,
}

impl OptionView {
    /// The view for the page's program (`page`) and Mein Studiengang (`mine`, the browser app's
    /// alone), as `CourseView::of` takes them: the page's program is the visitor's own where it
    /// is of the family of Mein Studiengang.
    pub(crate) fn of(choice: &Choice, page: Option<&Program>, mine: Option<&Program>) -> OptionView {
        let (program, is_mine) = match (page, mine) {
            (Some(page), Some(mine)) if page.family_key == mine.family_key => (mine, true),
            (Some(page), _) => (page, false),
            (None, Some(mine)) => (mine, true),
            (None, None) => return OptionView::Open { about: None },
        };
        let name = program_name(program);
        match (choice.kind, choice.listed_for(&program.family_key)) {
            (ChoiceKind::Program, Some(own)) if own.len() >= picks(choice) => match is_mine {
                true => OptionView::Mine { program: name, own },
                false => OptionView::Highlight { program: name, own },
            },
            (ChoiceKind::Program, None) if choice.listed_at_each(&program.family_key) => OptionView::Open { about: Some(Undecided::Every(name)) },
            (ChoiceKind::Program, None) => OptionView::Open { about: Some(Undecided::None(name)) },
            (_, Some(own)) => OptionView::Named { program: name, own },
            (_, None) => OptionView::Open { about: None },
        }
    }

    /// How option `option` of `choice` shows; `reveal`: „Auch zeigen" is on.
    pub(crate) fn state(&self, choice: &Choice, option: u32, reveal: bool) -> OptState {
        match self {
            OptionView::Open { .. } => OptState::Open,
            OptionView::Named { own, .. } if own.contains(&option) => OptState::Open,
            OptionView::Named { .. } => OptState::Faint,
            OptionView::Mine { own, .. } | OptionView::Highlight { own, .. } if own.contains(&option) => match own.len() <= picks(choice) {
                true => OptState::Taken,
                false => OptState::Open,
            },
            OptionView::Mine { .. } if !reveal => OptState::Hidden,
            OptionView::Mine { .. } | OptionView::Highlight { .. } => OptState::Faint,
        }
    }

    /// How many options one chooses among in this view: every one, or those of the program.
    fn among(&self, choice: &Choice) -> usize {
        match self {
            OptionView::Mine { own, .. } | OptionView::Highlight { own, .. } => own.len(),
            OptionView::Open { .. } | OptionView::Named { .. } => choice.options.len(),
        }
    }
}

/// How many options of a choice one takes.
fn picks(choice: &Choice) -> usize {
    usize::try_from(choice.pick).unwrap_or(usize::MAX)
}

/// The choices of the page's semester as they are shown: their views, in their order, and whether
/// „Auch zeigen" is on. `none` for a module without.
#[derive(Clone, Copy)]
pub(crate) struct Sight<'a> {
    pub choices: Option<&'a Choices>,
    pub views: &'a [OptionView],
    pub reveal: bool,
}

impl<'a> Sight<'a> {
    #[cfg(test)]
    pub(crate) fn none() -> Sight<'static> {
        Sight { choices: None, views: &[], reveal: false }
    }

    /// The choice of number `choice`, option `option`, and how it shows.
    pub(crate) fn of(&self, choice: u32, option: u32) -> Option<(usize, &'a Choice, &'a Opt, &'a OptionView, OptState)> {
        let choices = self.choices?;
        let at = choices.list.iter().position(|known| known.number == choice)?;
        let known = choices.list.get(at)?;
        let opt = known.options.iter().find(|known| known.number == option)?;
        let view = self.views.get(at)?;
        Some((at, known, opt, view, view.state(known, option, self.reveal)))
    }
}

/// What the options of a choice are called: groups a title names; the kind of their events
/// where each option is one event and all are one kind; else groups, offers or events.
pub(crate) fn noun_of(choice: &Choice, dates: &[EventDate]) -> NounOf {
    if choice.basis == Some(ChoiceBasis::Named) {
        return NounOf::Group;
    }
    let several = choice.options.iter().any(|option| option.events.len() > 1);
    let kinds: Vec<Option<EventKind>> =
        choice.options.iter().flat_map(|option| &option.events).map(|event| one_kind(dates.iter().find(|date| date.event_id == *event).and_then(|date| date.event_type.as_deref()))).collect();
    match kinds.split_first() {
        Some((Some(kind), rest)) if !several && rest.iter().all(|other| *other == Some(*kind)) => NounOf::Kind(*kind),
        _ => general_noun(choice.kind),
    }
}

/// The noun of a choice in a list's row (`ChoiceSummary`).
pub(crate) fn summary_noun(summary: &ChoiceSummary) -> NounOf {
    if summary.basis.known() == Some(ChoiceBasis::Named) {
        return NounOf::Group;
    }
    match one_kind(summary.event_type.as_deref()) {
        Some(kind) => NounOf::Kind(kind),
        None => general_noun(summary.kind.known().unwrap_or(ChoiceKind::Topic)),
    }
}

/// The noun of options of no one kind.
fn general_noun(kind: ChoiceKind) -> NounOf {
    match kind {
        ChoiceKind::Topic => NounOf::Offer,
        ChoiceKind::Group => NounOf::Group,
        ChoiceKind::Program => NounOf::Event,
    }
}

/// The one kind a type of QIS names, other than „Sonstiges".
fn one_kind(type_raw: Option<&str>) -> Option<EventKind> {
    let kinds: Vec<EventKind> = kinds_of(type_raw).iter().collect();
    match kinds.as_slice() {
        [kind] if *kind != EventKind::Other => Some(*kind),
        _ => None,
    }
}

/// The heading of a choice in the list, and its words in a slot's tooltip: „Eine der 3 Übungen",
/// „Übungen für 6 SWS" where the options differ in size. `among`: how many one chooses among.
pub(crate) fn heading(choice: &Choice, noun: Noun, among: usize, t: &'static Texts) -> String {
    match (choice.kind, choice.size(), choice.sws) {
        (ChoiceKind::Topic, Size::Differ, Some(sws)) => (t.module.choice_for_sws)(&format::number(sws, t.locale), noun),
        _ => (t.module.choice_of)(choice.pick, among, noun),
    }
}

/// The heading of `choice` in its view.
pub(crate) fn view_heading(choice: &Choice, view: &OptionView, dates: &[EventDate], t: &'static Texts) -> String {
    heading(choice, (t.module.noun)(noun_of(choice, dates)), view.among(choice), t)
}

/// „für Bauingenieurwesen, Maschinenbau": whom QIS lists an option for, where the options of its
/// choice are for other programs.
pub(crate) fn audience(choice: &Choice, option: &Opt, t: &'static Texts) -> Option<String> {
    (choice.split_by_program() && !option.listed.is_empty()).then(|| (t.module.choice_for)(&option.listed.join(", ")))
}

/// When an option is held, as the line over the week names it: „Di 07:30", „Mo, Mi 11:30" is
/// „Mo 11:30 und Mi 11:30"; the first three of its times, „ohne Termin" without any.
pub(crate) fn option_times(option: &Opt, dates: &[EventDate], t: &'static Texts) -> String {
    const SHOWN: usize = 3;
    let mut times: Vec<(i64, &str)> = Vec::new();
    for date in dates.iter().filter(|date| option.events.contains(&date.event_id)) {
        if let (Some(day), Some(start)) = (date.weekday, date.start_time.as_deref()) {
            if !times.contains(&(day, start)) {
                times.push((day, start));
            }
        }
    }
    times.sort_unstable();
    let mut named: Vec<String> = times.iter().take(SHOWN).filter_map(|(day, start)| format::time_slot(Some(*day), Some(start), None, t.locale)).collect();
    if named.is_empty() {
        return t.module.choice_no_time.to_string();
    }
    if times.len() > SHOWN {
        named.push("…".to_string());
    }
    and_list(&named, t)
}

/// The earliest weekday and start of an option, for the order of the list; `None` without a time.
pub(crate) fn option_start<'d>(option: &Opt, dates: &'d [EventDate]) -> Option<(i64, &'d str)> {
    dates.iter().filter(|date| option.events.contains(&date.event_id)).filter_map(|date| date.weekday.zip(date.start_time.as_deref())).min()
}

/// „A, B und C".
pub(crate) fn and_list(items: &[String], t: &'static Texts) -> String {
    match items.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{}{}{last}", rest.join(", "), t.module.courses_and),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

/// A title without the group its option is („Kontrapunkt/Tonsatz I (Gruppe 1)" is „Kontrapunkt/
/// Tonsatz I"): the label, „1. Gruppe" or „Gruppe1" for „Gruppe 1", in brackets or not, where the
/// title has it in any case of its letters.
pub(crate) fn without_group(title: &str, label: Option<&str>) -> String {
    let title = title.trim();
    let Some(label) = label.map(str::trim).filter(|label| !label.is_empty()) else { return title.to_string() };
    let number: String = label.chars().filter(char::is_ascii_digit).collect();
    let mut marks = vec![label.to_string()];
    if !number.is_empty() {
        marks.push(format!("{number}. Gruppe"));
        marks.push(format!("Gruppe{number}"));
    }
    for mark in &marks {
        for wrapped in [format!("({mark})"), format!("[{mark}]"), mark.clone()] {
            if let Some(at) = find_ascii_ci(title, &wrapped) {
                let (before, after) = (title.get(..at).unwrap_or_default(), title.get(at + wrapped.len()..).unwrap_or_default());
                let joined = format!("{} {}", before.trim_end(), after.trim_start());
                let tidy = joined.split_whitespace().collect::<Vec<_>>().join(" ");
                let tidy = tidy.trim_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '–' | ':' | ',' | '/'));
                if !tidy.is_empty() {
                    return tidy.to_string();
                }
            }
        }
    }
    title.to_string()
}

/// Where `needle` (ASCII) stands in `text`, its letters in any case.
fn find_ascii_ci(text: &str, needle: &str) -> Option<usize> {
    let (text_bytes, needle_bytes) = (text.as_bytes(), needle.as_bytes());
    if needle_bytes.is_empty() || needle_bytes.len() > text_bytes.len() {
        return None;
    }
    (0..=text_bytes.len() - needle_bytes.len())
        .find(|at| text.is_char_boundary(*at) && text.is_char_boundary(at + needle_bytes.len()) && text_bytes.get(*at..at + needle_bytes.len()).is_some_and(|window| window.eq_ignore_ascii_case(needle_bytes)))
}

/// The line over the week of one choice, in its view, without the button of „Auch zeigen".
/// `invite`: where the page stands for no program, it says that Mein Studiengang would show only
/// its options (not where the line of the courses says so already).
pub(crate) fn note_text(choice: &Choice, view: &OptionView, dates: &[EventDate], invite: bool, t: &'static Texts) -> String {
    let words = &t.module;
    let noun = (words.noun)(noun_of(choice, dates));
    let count = choice.options.len();
    // The times of some options, in the order of the week.
    let times = |pick: &dyn Fn(&Opt) -> bool| -> String {
        let mut picked: Vec<&Opt> = choice.options.iter().filter(|option| pick(option)).collect();
        picked.sort_by_key(|option| (option_start(option, dates).is_none(), option_start(option, dates)));
        let parts: Vec<String> = picked.into_iter().map(|option| option_times(option, dates, t)).collect();
        and_list(&parts, t)
    };
    let named = |program: &str, own: &BTreeSet<u32>| (words.choice_named)(program, own.len());
    let what = match choice.kind {
        ChoiceKind::Program => (words.choice_program)(count, noun),
        ChoiceKind::Group if choice.basis == Some(ChoiceBasis::Named) => {
            // The titles of the first option's events, without the group they are in.
            let first = choice.options.first();
            let mut titles: Vec<String> = Vec::new();
            for event in first.map(|option| option.events.as_slice()).unwrap_or_default() {
                if let Some(date) = dates.iter().find(|date| date.event_id == *event) {
                    let title = without_group(&date.event_title, first.and_then(|option| option.label.as_deref()));
                    if !titles.contains(&title) {
                        titles.push(title);
                    }
                }
            }
            (words.choice_groups)(&and_list(&titles, t), count, titles.len())
        }
        ChoiceKind::Group => (words.choice_twice)(count, noun),
        ChoiceKind::Topic => match (choice.size(), choice.sws) {
            (Size::Each(each), _) => (words.choice_topic)(choice.pick, count, noun, Some(&format::number(each, t.locale))),
            (Size::Differ, Some(sws)) => (words.choice_topic_sws)(noun, &format::number(sws, t.locale)),
            _ => (words.choice_topic)(choice.pick, count, noun, None),
        },
    };
    match view {
        OptionView::Open { about: None } if choice.kind == ChoiceKind::Program && invite => format!("{what} {}", words.choice_setup),
        OptionView::Open { about: None } => what,
        OptionView::Open { about: Some(Undecided::None(program)) } => format!("{what} {}", (words.choice_none)(program)),
        OptionView::Open { about: Some(Undecided::Every(program)) } => format!("{what} {}", (words.choice_every)(program)),
        OptionView::Named { program, own } => format!("{what} {}", named(program, own)),
        OptionView::Mine { program, own } => {
            let mine = times(&|option| own.contains(&option.number));
            let others = times(&|option| !own.contains(&option.number));
            let first = match own.len() <= picks(choice) {
                true => (words.choice_mine)(noun, program, &mine),
                false => (words.choice_mine_some)(program, own.len(), count, choice.pick, noun, &mine),
            };
            format!("{first} {}", (words.choice_also)(&others))
        }
        OptionView::Highlight { program, own } => {
            let theirs = times(&|option| own.contains(&option.number));
            let first = match own.len() <= picks(choice) {
                true => (words.choice_highlight)(noun, program, &theirs),
                false => (words.choice_highlight_some)(program, own.len(), count, choice.pick, noun, &theirs),
            };
            format!("{first} {}", words.choice_faint)
        }
    }
}

/// The extras of a module's Termine as the line over the week names them: their titles, a title
/// that is the module's own by its type and times („Tutorium Mo 09:15"); `None` without any.
pub(crate) fn extras_text(dates: &[EventDate], module_title: &str, t: &'static Texts) -> Option<String> {
    let mut named: Vec<String> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for date in dates.iter().filter(|date| date.extra) {
        if seen.contains(&date.event_id.as_str()) {
            continue;
        }
        seen.push(&date.event_id);
        let title = date.event_title.trim();
        let name = match title.eq_ignore_ascii_case(module_title.trim()) || title.is_empty() {
            true => {
                let option = Opt { number: 0, events: vec![date.event_id.clone()], label: None, sws: None, families: BTreeSet::new(), listed: Vec::new() };
                let kind = date.event_type.as_deref().map(str::trim).filter(|kind| !kind.is_empty()).unwrap_or(t.module.slot_date);
                format!("{kind} {}", option_times(&option, dates, t))
            }
            false => title.to_string(),
        };
        if !named.contains(&name) {
            named.push(name);
        }
    }
    (!named.is_empty()).then(|| (t.module.choice_extra_note)(&and_list(&named, t)))
}

/// What a module's row in a list says of its courses and its choices (`CatalogRow::courses`,
/// `choices`): „2 Standorte · Übung je Studiengang", „14 Projekte zur Wahl", „3 Gruppen"; `None`
/// for a module with neither.
pub fn row_offer(courses: u32, choices: &[ChoiceSummary], t: &'static Texts) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if courses >= 2 {
        parts.push((t.catalog.row_courses)(usize::try_from(courses).unwrap_or(2)));
    }
    for choice in choices {
        let noun = (t.module.noun)(summary_noun(choice));
        let options = usize::try_from(choice.options).unwrap_or(0);
        parts.push(match choice.kind.known().unwrap_or(ChoiceKind::Topic) {
            ChoiceKind::Program => (t.catalog.row_choice_program)(noun),
            ChoiceKind::Group => (t.catalog.row_choice_group)(options),
            ChoiceKind::Topic => (t.catalog.row_choice_topic)(options, noun),
        });
    }
    parts.dedup();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use folia_model::labels::Code;
    use folia_model::rows_detail::{ChoiceFamily, ChoiceOption};

    use super::*;
    use crate::i18n::{DE, EN};

    pub(crate) fn date(event: &str, title: &str, kind: &str, weekday: i64, start: &str) -> EventDate {
        EventDate {
            semester_key: "2026W".into(),
            semester_label: "WiSe 2026/27".into(),
            event_id: event.into(),
            event_number: None,
            event_title: title.into(),
            event_type: Some(kind.into()),
            group_name: None,
            weekday: Some(weekday),
            start_time: Some(start.into()),
            end_time: None,
            rhythm: None,
            rhythm_raw: None,
            first_date: None,
            last_date: None,
            room: None,
            campus: None,
            instructor: None,
            comment: None,
            source_url: None,
            room_short: None,
            series: None,
            track: None,
            choice: None,
            option: None,
            extra: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn row(kind: &str, basis: &str, sws: Option<f64>, pick: u32, option: u32, event: &str, label: Option<&str>, event_sws: Option<f64>) -> ChoiceOption {
        ChoiceOption {
            module_id: "11206".into(),
            semester_key: "2026W".into(),
            choice: 1,
            course: None,
            kind: Code::parse(kind),
            basis: Code::parse(basis),
            sws,
            pick,
            option,
            event_id: event.into(),
            label: label.map(str::to_string),
            event_sws,
        }
    }

    fn family(option: u32, family: &str, name: &str) -> ChoiceFamily {
        ChoiceFamily {
            module_id: "11206".into(),
            semester_key: "2026W".into(),
            choice: 1,
            option,
            family_key: family.into(),
            program_name: name.into(),
            is_latest_po: true,
        }
    }

    fn program(family: &str, name: &str) -> Program {
        Program {
            id: format!("{family}-2022"),
            slug: name.to_lowercase(),
            name: name.into(),
            degree_level: Code::parse("bachelor"),
            study_variant: None,
            degree_label: Some("B.Sc.".into()),
            degree_raw: "Bachelor".into(),
            degree_display: Some("B.Sc.".into()),
            po_version: "2022".into(),
            po_year: Some(2022),
            family_key: family.into(),
            name_key: name.to_lowercase(),
            is_latest_po: true,
            source_url: String::new(),
            has_plan: false,
            plan_status: None,
            curricular_modules: 0,
            fues_modules: 0,
            documents: 0,
        }
    }

    /// Höhere Mathematik T3 at the Zentralcampus: three exercises, each for other programs.
    fn hm_t3() -> (Choices, Vec<EventDate>) {
        let rows = [
            row("program", "form", Some(2.0), 1, 1, "148245", None, Some(2.0)),
            row("program", "form", Some(2.0), 1, 2, "148246", None, Some(2.0)),
            row("program", "form", Some(2.0), 1, 3, "148932", None, Some(2.0)),
        ];
        let families = [
            family(1, "048-82", "Elektrotechnik"),
            family(1, "774-82", "Umweltingenieurwesen"),
            family(2, "017-82", "Bauingenieurwesen"),
            family(3, "104-82", "Maschinenbau"),
            family(3, "774-82", "Umweltingenieurwesen"),
        ];
        let dates = vec![
            date("148060", "Höhere Mathematik - T3", "Vorlesung", 1, "15:30"),
            date("148245", "Höhere Mathematik - T3", "Übung", 4, "11:30"),
            date("148246", "Höhere Mathematik - T3", "Übung", 2, "07:30"),
            date("148932", "Höhere Mathematik - T3", "Übung", 3, "13:45"),
            EventDate { extra: true, ..date("150190", "Aufbaukurs Höhere Mathematik - T3", "Übung", 1, "09:15") },
        ];
        (Choices::of(&rows, &families, "2026W").unwrap(), dates)
    }

    #[test]
    fn whom_the_exercises_of_hm_t3_are_for() {
        let (choices, dates) = hm_t3();
        let choice = &choices.list[0];
        let (bi, ui, inf) = (program("017-82", "Bauingenieurwesen"), program("774-82", "Umweltingenieurwesen"), program("079-82", "Informatik"));

        // Nobody: every exercise open, each with whom QIS lists it for.
        let open = OptionView::of(choice, None, None);
        assert_eq!(open, OptionView::Open { about: None });
        assert_eq!((1..=3).map(|n| open.state(choice, n, false)).collect::<Vec<_>>(), [OptState::Open; 3]);
        assert_eq!(
            note_text(choice, &open, &dates, true, &DE),
            "Die Übung gibt es dreimal, je für andere Studiengänge. Du besuchst eine. Mit deinem Studiengang in „Mein Studium“ zeigt Betula nur, was QIS für ihn nennt."
        );
        assert_eq!(note_text(choice, &open, &dates, false, &DE), "Die Übung gibt es dreimal, je für andere Studiengänge. Du besuchst eine.");
        assert_eq!(view_heading(choice, &open, &dates, &DE), "Eine der 3 Übungen");
        assert_eq!(view_heading(choice, &open, &dates, &EN), "One of the 3 exercise classes");
        assert_eq!(audience(choice, &choice.options[0], &DE).as_deref(), Some("für Elektrotechnik, Umweltingenieurwesen"));

        // Bauingenieurwesen as Mein Studiengang: its exercise taken, the others behind „Auch zeigen".
        let mine = OptionView::of(choice, None, Some(&bi));
        assert_eq!(mine, OptionView::Mine { program: "Bauingenieurwesen · B.Sc.".into(), own: BTreeSet::from([2]) });
        assert_eq!([1, 2, 3].map(|n| mine.state(choice, n, false)), [OptState::Hidden, OptState::Taken, OptState::Hidden]);
        assert_eq!(mine.state(choice, 1, true), OptState::Faint);
        assert_eq!(
            note_text(choice, &mine, &dates, true, &DE),
            "Die Übung für Bauingenieurwesen · B.Sc.: Di 07:30. Auch angeboten, für andere Studiengänge: Mi 13:45 und Do 11:30."
        );
        // Umweltingenieurwesen: two to choose between.
        let two = OptionView::of(choice, None, Some(&ui));
        assert_eq!([1, 2, 3].map(|n| two.state(choice, n, false)), [OptState::Open, OptState::Hidden, OptState::Open]);
        assert_eq!(view_heading(choice, &two, &dates, &DE), "Eine der 2 Übungen");
        assert_eq!(
            note_text(choice, &two, &dates, true, &DE),
            "Für Umweltingenieurwesen · B.Sc. nennt QIS zwei der 3 Übungen (Mi 13:45 und Do 11:30); du besuchst eine. Auch angeboten, für andere Studiengänge: Di 07:30."
        );
        // Another program the page stands for: highlighted, the others faint.
        let other = OptionView::of(choice, Some(&bi), Some(&inf));
        assert_eq!([1, 2, 3].map(|n| other.state(choice, n, false)), [OptState::Faint, OptState::Taken, OptState::Faint]);
        assert_eq!(note_text(choice, &other, &dates, true, &DE), "Hervorgehoben: die Übung für Bauingenieurwesen · B.Sc. (Di 07:30). Blass: die für andere Studiengänge.");
        // A program QIS lists at none: an open choice that says so.
        let none = OptionView::of(choice, None, Some(&inf));
        assert_eq!(none, OptionView::Open { about: Some(Undecided::None("Informatik · B.Sc.".into())) });
        assert!(note_text(choice, &none, &dates, true, &DE).ends_with("Du besuchst eine. Für Informatik · B.Sc. nennt QIS keinen dieser Termine."));

        assert_eq!(extras_text(&dates, "Höhere Mathematik - T3", &DE).as_deref(), Some("Zusätzlich, freiwillig: Aufbaukurs Höhere Mathematik - T3."));
        assert_eq!(extras_text(&dates[..4], "Höhere Mathematik - T3", &DE), None);
    }

    #[test]
    fn groups_topics_and_their_words() {
        // Musiktheorie: Kontrapunkt and Gehörbildung in three groups.
        let rows = [
            row("group", "named", None, 1, 1, "149750", Some("Gruppe 1"), Some(2.0)),
            row("group", "named", None, 1, 1, "149753", Some("Gruppe 1"), Some(1.0)),
            row("group", "named", None, 1, 2, "149752", Some("Gruppe 2"), Some(2.0)),
            row("group", "named", None, 1, 2, "149755", Some("Gruppe 2"), Some(1.0)),
            row("group", "named", None, 1, 3, "150378", Some("Gruppe 3"), Some(2.0)),
            row("group", "named", None, 1, 3, "150377", Some("Gruppe 3"), Some(1.0)),
        ];
        let dates = vec![
            date("149750", "Kontrapunkt/Tonsatz I (Gruppe 1)", "Seminar", 5, "10:30"),
            date("149753", "Gehörbildung I (Gruppe 1)", "Übung", 5, "12:00"),
            date("149752", "Kontrapunkt/Tonsatz I (Gruppe 2)", "Seminar", 5, "13:30"),
            date("149755", "Gehörbildung I (Gruppe 2)", "Übung", 5, "15:00"),
            date("150378", "Kontrapunkt/Tonsatz I (Gruppe 3)", "Seminar", 5, "16:00"),
            date("150377", "Gehörbildung I (Gruppe 3)", "Übung", 5, "17:30"),
        ];
        let choices = Choices::of(&rows, &[], "2026W").unwrap();
        let choice = &choices.list[0];
        let open = OptionView::of(choice, None, None);
        assert_eq!(note_text(choice, &open, &dates, true, &DE), "Kontrapunkt/Tonsatz I und Gehörbildung I gibt es in 3 Gruppen. Du besuchst eine, beide in derselben Gruppe.");
        assert_eq!(note_text(choice, &open, &dates, true, &EN), "Kontrapunkt/Tonsatz I and Gehörbildung I are held in 3 groups. You attend one, both in the same group.");
        assert_eq!(view_heading(choice, &open, &dates, &DE), "Eine der 3 Gruppen");
        assert_eq!(audience(choice, &choice.options[0], &DE), None);
        assert_eq!(option_times(&choice.options[0], &dates, &DE), "Fr 10:30 und Fr 12:00");

        // A catalogue of projects, QIS naming the program at some.
        let projects: Vec<ChoiceOption> = (1..=14).map(|n| row("topic", "module", Some(4.0), 1, n, &format!("p{n}"), None, Some(4.0))).collect();
        let families: Vec<ChoiceFamily> = [2, 3, 5].into_iter().map(|n| family(n, "208-84", "Soziale Arbeit")).collect();
        let dates: Vec<EventDate> = (1..=14).map(|n| date(&format!("p{n}"), &format!("Projekt {n}"), "Projekt", 1, "11:00")).collect();
        let choices = Choices::of(&projects, &families, "2026W").unwrap();
        let choice = &choices.list[0];
        let open = OptionView::of(choice, None, None);
        assert_eq!(note_text(choice, &open, &dates, true, &DE), "Wahlangebot: Du belegst eines der 14 Projekte (je 4 SWS).");
        assert_eq!(view_heading(choice, &open, &dates, &DE), "Eines der 14 Projekte");
        let social = program("208-84", "Soziale Arbeit");
        let named = OptionView::of(choice, None, Some(&social));
        assert_eq!([1, 2].map(|n| named.state(choice, n, false)), [OptState::Faint, OptState::Open]);
        assert_eq!(
            note_text(choice, &named, &dates, true, &DE),
            "Wahlangebot: Du belegst eines der 14 Projekte (je 4 SWS). QIS nennt Soziale Arbeit · B.Sc. bei 3 davon, sie stehen zuerst."
        );

        // Courses of differing sizes for 6 SWS.
        let courses = [
            row("topic", "form", Some(6.0), 2, 1, "e1", None, Some(4.0)),
            row("topic", "form", Some(6.0), 2, 2, "e2", None, Some(4.0)),
            row("topic", "form", Some(6.0), 2, 3, "e3", None, Some(2.0)),
        ];
        let dates: Vec<EventDate> = ["e1", "e2", "e3"].into_iter().map(|id| date(id, "English for Academic Purposes", "Übung", 2, "09:15")).collect();
        let choice = &Choices::of(&courses, &[], "2026W").unwrap().list[0];
        let open = OptionView::of(choice, None, None);
        assert_eq!(note_text(choice, &open, &dates, true, &DE), "Wahlangebot: Du belegst Übungen für 6 SWS.");
        assert_eq!(view_heading(choice, &open, &dates, &DE), "Übungen für 6 SWS");
        assert_eq!(view_heading(choice, &open, &dates, &EN), "Exercise classes for 6 SWS");
    }

    #[test]
    fn a_title_without_its_group() {
        assert_eq!(without_group("Kontrapunkt/Tonsatz I (Gruppe 1)", Some("Gruppe 1")), "Kontrapunkt/Tonsatz I");
        assert_eq!(without_group("Lehrprobenseminar 2. Gruppe", Some("Gruppe 2")), "Lehrprobenseminar");
        assert_eq!(without_group("Englisch – B-Kurs", Some("B-Kurs")), "Englisch");
        assert_eq!(without_group("Gehörbildung I [gruppe 3]", Some("Gruppe 3")), "Gehörbildung I");
        assert_eq!(without_group("Gruppe 1", Some("Gruppe 1")), "Gruppe 1");
        assert_eq!(without_group("Übung Öko (Gruppe 1)", None), "Übung Öko (Gruppe 1)");
    }

    #[test]
    fn a_row_names_the_courses_and_the_choices() {
        let summary = |kind: &str, basis: &str, options: u32, event_type: Option<&str>| ChoiceSummary {
            kind: Code::parse(kind),
            basis: Code::parse(basis),
            options,
            pick: 1,
            event_type: event_type.map(str::to_string),
        };
        assert_eq!(row_offer(0, &[summary("topic", "module", 14, Some("Projekt"))], &DE).as_deref(), Some("14 Projekte zur Wahl"));
        assert_eq!(row_offer(2, &[summary("program", "form", 3, Some("Übung"))], &DE).as_deref(), Some("2 Standorte · Übung je Studiengang"));
        assert_eq!(row_offer(0, &[summary("group", "named", 3, None)], &DE).as_deref(), Some("3 Gruppen"));
        assert_eq!(row_offer(0, &[summary("topic", "module", 4, Some("Seminar/Übung"))], &DE).as_deref(), Some("4 Angebote zur Wahl"));
        assert_eq!(row_offer(0, &[summary("topic", "module", 14, Some("Projekt"))], &EN).as_deref(), Some("14 projects to choose from"));
        assert_eq!(row_offer(0, &[], &DE), None);
        assert_eq!(row_offer(1, &[], &DE), None);
    }
}
