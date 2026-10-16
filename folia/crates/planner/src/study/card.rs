//! „Dein Studium" as a card (owner, 2026-10-16, concept A of folia/design/studiengang/konzepte.html:
//! „option a mit karte finde ich auch am besten"): what is studied, a row a value — the program, its
//! study direction, the Fachsemester — each changed where it stands, in „Studiengang wechseln"
//! (`side.rs`) and on the first visit (`setup.rs`). Over the rows the sentence they make („Du
//! studierst Informatik B.Sc. im 9. Fachsemester."); a row that differs from what is kept says in its
//! line under the value what it was, with „Zurück". The Fachsemester is asked for, not the
//! Studienbeginn („so ein ding von ich bin in informatik 9. semester"): − and + count it, and the
//! Studienbeginn kept is the semester that makes the current one that Fachsemester
//! (`study::start_of_fs`; below the first, a study that begins after the current semester).
//!
//! Nothing moves while the card is used (owner, the same day: „Wenn man eine aktion macht, wie
//! semester +/- darf nicht unterbrochen werden durch layout shifts"): the sentence is one line,
//! every row is as tall whatever it says (its line one line, cut with „…"), and what follows the
//! card keeps its height (app.css „Dein Studium"). The one move is the study direction's: its row
//! is there only where the program has study directions, unfolding as such a program is picked and
//! folding away for one without (owner, the same day: „wenn ein studiengang keine ausrichtung hat,
//! dann sollte das nicht eingeblendet sein, ansonsten ist das verwirrend. Mach mal ne nette
//! animation, wenn sich das ausklappt"); − and + never move it.

use std::collections::BTreeSet;

use folia_calendar::semester::SemesterKey;
use folia_plans::study;
use leptos::prelude::*;

use folia_design::combobox::{ComboItem, Combobox, Select};

use crate::i18n;

/// How many semesters after the current one a study may begin (− below the first Fachsemester),
/// and the last Fachsemester + counts to.
const BEGINS_AFTER: i32 = 2;
const MOST_FS: i32 = 30;

/// What is kept, which a changed row says it was („Studiengang wechseln"; the first visit has
/// nothing kept).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Kept {
    pub program: String,
    /// „Informatik B.Sc. · PO 2008".
    pub name: String,
    pub direction: usize,
    pub direction_label: Option<String>,
    pub start: SemesterKey,
}

/// The ids of a card's controls.
#[derive(Clone, Copy, Debug)]
pub(super) struct Ids {
    pub program: &'static str,
    pub direction: &'static str,
    pub less: &'static str,
    pub more: &'static str,
}

/// A part of the card that „Zurück" brings back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
    Program,
    Direction,
    Start,
}

/// Where `start` puts `now`, as − and + count: its Fachsemester, the semesters of `leave` not
/// counted; for a study that begins after `now` 0 the semester after it, -1 the one after that.
pub(super) fn step_of(start: SemesterKey, now: SemesterKey, leave: &BTreeSet<SemesterKey>) -> i32 {
    if start > now {
        return 1 - (i32::from(start.index()) - i32::from(now.index()));
    }
    let mut n = 0;
    let mut at = start;
    while at <= now {
        if !leave.contains(&at) {
            n += 1;
        }
        match at.plus(1) {
            Some(next) => at = next,
            None => break,
        }
    }
    n
}

/// The Studienbeginn of a step (`step_of` the other way round).
pub(super) fn start_of_step(step: i32, now: SemesterKey, leave: &BTreeSet<SemesterKey>) -> Option<SemesterKey> {
    match u8::try_from(step) {
        Ok(fs) if fs >= 1 => study::start_of_fs(now, fs, leave),
        _ => now.plus(1 - step),
    }
}

/// How many semesters of `leave` lie between `start` and `now`.
fn leave_between(start: SemesterKey, now: SemesterKey, leave: &BTreeSet<SemesterKey>) -> usize {
    leave.range(start..=now).count()
}

#[component]
pub(super) fn StudyCard(
    ids: Ids,
    /// The programs to pick from („Mein Studiengang" first, `side::program_items`).
    #[prop(into)]
    items: Signal<Vec<ComboItem>>,
    #[prop(into)] program: Signal<Option<String>>,
    on_program: Callback<Option<String>>,
    /// „Informatik B.Sc." of the program picked, for the sentence.
    #[prop(into)]
    title: Signal<Option<String>>,
    /// Under the program: its plan's length, „Regelstudienplan: 6 Semester, 180 LP".
    #[prop(into)]
    plan_line: Signal<Option<String>>,
    /// The study directions of the program picked (none where it has one plan), `None` while its
    /// plans are on their way; and the one shown.
    #[prop(into)]
    directions: Signal<Option<Vec<ComboItem>>>,
    #[prop(into)] direction: Signal<usize>,
    on_direction: Callback<usize>,
    /// The Studienbeginn; `None` while there is none yet (the first visit, before a program).
    #[prop(into)]
    start: Signal<Option<SemesterKey>>,
    on_start: Callback<SemesterKey>,
    #[prop(into)] now: Signal<SemesterKey>,
    #[prop(into)] leave: Signal<BTreeSet<SemesterKey>>,
    #[prop(into)] kept: Signal<Option<Kept>>,
    /// „Zurück" of a changed row.
    on_back: Callback<Part>,
) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let step = Memo::new(move |_| start.get().map(|start| leave.with(|leave| step_of(start, now.get(), leave))).unwrap_or(1));
    let program_changed = Memo::new(move |_| kept.with(|kept| kept.as_ref().is_some_and(|kept| program.with(|program| program.as_deref() != Some(kept.program.as_str())))));
    // The study direction's row: whether it is unfolded, its directions and the one picked. While
    // the plans of a program picked are on their way it stays as it is (it neither folds nor
    // unfolds in between), and folding away it keeps what it said.
    let fold = Memo::new(move |before: Option<&(bool, Vec<ComboItem>, usize)>| match directions.get() {
        None => before.cloned().unwrap_or_default(),
        Some(items) if items.is_empty() => before.map_or_else(Default::default, |(_, items, at)| (false, items.clone(), *at)),
        Some(items) => (true, items, direction.get()),
    });
    let unfolded = Memo::new(move |_| fold.with(|(open, ..)| *open));
    let direction_changed = Memo::new(move |_| {
        !program_changed.get() && fold.with(|(open, _, at)| *open && kept.with(|kept| kept.as_ref().is_some_and(|kept| kept.direction != *at)))
    });
    let start_changed = Memo::new(move |_| kept.with(|kept| kept.as_ref().is_some_and(|kept| start.get() != Some(kept.start))));

    // The sentence the rows make.
    let say = move || {
        let Some(title) = title.get() else { return s.say_none.to_string() };
        let (now, switching) = (now.get(), program_changed.get());
        match start.get() {
            Some(start) if start > now => {
                let begins = start.label(t.locale);
                if switching { (s.say_switch_begins)(&title, &begins) } else { (s.say_begins)(&title, &begins) }
            }
            Some(start) => match leave.with(|leave| study::fs_of(start, now, leave)) {
                Some(fs) if switching => (s.say_switch)(&title, fs),
                Some(fs) => (s.say_studies)(&title, fs),
                None => (s.say_leave)(&title),
            },
            None => (s.say_studies)(&title, 1),
        }
    };
    // A row's line: what it was, with „Zurück", where it changed; else what it says besides.
    let note = move |changed: bool, was: Option<String>, part: Part, plain: Option<String>| -> AnyView {
        match (changed, was) {
            (true, Some(was)) => view! {
                <span>{(s.was)(&was)}</span>
                <button class="st-back" type="button" on:click=move |_| on_back.run(part)>{t.common.back}</button>
            }
            .into_any(),
            _ => view! { <span>{plain}</span> }.into_any(),
        }
    };
    let program_note = move || {
        let was = kept.with(|kept| kept.as_ref().map(|kept| kept.name.clone()));
        note(program_changed.get(), was, Part::Program, plan_line.get())
    };
    let direction_note = move || {
        let was = kept.with(|kept| kept.as_ref().and_then(|kept| kept.direction_label.clone()));
        let count = fold.with(|(_, items, _)| items.len());
        note(direction_changed.get(), was, Part::Direction, (count > 1).then(|| (s.directions_line)(count)))
    };
    let fs_value = move || match start.get() {
        Some(start) if start > now.get() => (s.begins_in)(&start.label(t.locale)),
        Some(start) => leave.with(|leave| study::fs_of(start, now.get(), leave)).map_or_else(|| s.leave.to_string(), |fs| (s.fs_long)(fs)),
        None => (s.fs_long)(1),
    };
    let fs_note = move || {
        let plain = start.get().filter(|start| *start <= now.get()).map(|start| {
            let since = (s.since)(&start.label(t.locale));
            match leave.with(|leave| leave_between(start, now.get(), leave)) {
                0 => since,
                n => format!("{since} · {}", (s.leave_line)(n)),
            }
        });
        let was = kept.with(|kept| kept.as_ref().map(|kept| {
            let fs = leave.with(|leave| study::fs_of(kept.start, now.get(), leave));
            fs.map_or_else(|| (s.begins_in)(&kept.start.label(t.locale)), |fs| (s.fs_long)(fs))
        }));
        // What it was, after where it starts now: both stay on the line.
        let was = was.map(|was| match &plain {
            Some(plain) => format!("{was} · {plain}"),
            None => was,
        });
        note(start_changed.get(), was, Part::Start, plain)
    };
    let set_step = move |by: i32| {
        let next = (step.get_untracked() + by).clamp(1 - BEGINS_AFTER, MOST_FS);
        if let Some(start) = leave.with_untracked(|leave| start_of_step(next, now.get_untracked(), leave)) {
            on_start.run(start);
        }
    };

    view! {
        <section class="st-study" aria-label=s.card>
            <p class="st-study-say" aria-live="polite">{say}</p>
            <div class="st-srow" class:changed=move || program_changed.get()>
                <span class="label">{s.program}</span>
                <Combobox
                    id=ids.program
                    label=s.program
                    placeholder=s.choose_program
                    search_placeholder=s.search_program
                    min_width=420.0
                    items
                    selected=program
                    on_select=on_program
                    clearable=false
                />
                <p class="st-srow-note">{program_note}</p>
            </div>
            // Folded away, out of reach of Tab and of a reader, where the program has one plan.
            <div class="st-fold" class:open=move || unfolded.get() inert=move || !unfolded.get()>
                <div class="st-fold-in">
                    <div class="st-srow" class:changed=move || direction_changed.get()>
                        <span class="label">{s.direction}</span>
                        <Select
                            id=ids.direction
                            label=s.direction
                            items=Signal::derive(move || fold.with(|(_, items, _)| items.clone()))
                            selected=Signal::derive(move || fold.with(|(_, _, at)| at.to_string()))
                            on_select=Callback::new(move |value: String| on_direction.run(value.parse().unwrap_or(0)))
                        />
                        <p class="st-srow-note">{direction_note}</p>
                    </div>
                </div>
            </div>
            <div class="st-srow" class:changed=move || start_changed.get() data-start=move || start.get().map(SemesterKey::key)>
                <span class="label">{s.fs_row}</span>
                <div class="st-fs-line">
                    <span class="st-fs-value">{fs_value}</span>
                    <span class="st-stepper" role="group" aria-label=s.fs_row>
                        <button type="button" id=ids.less aria-label=s.fs_less title=s.fs_less disabled={move || step.get() <= 1 - BEGINS_AFTER} on:click=move |_| set_step(-1)>"−"</button>
                        <button type="button" id=ids.more aria-label=s.fs_more title=s.fs_more disabled={move || step.get() >= MOST_FS} on:click=move |_| set_step(1)>"+"</button>
                    </span>
                </div>
                <p class="st-srow-note">{fs_note}</p>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use folia_calendar::semester::SemesterKey;

    use super::{start_of_step, step_of};

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    #[test]
    fn a_step_is_a_fachsemester_and_below_the_first_a_start_to_come() {
        let (now, none) = (key("2026W"), BTreeSet::new());
        assert_eq!(step_of(key("2022W"), now, &none), 9);
        assert_eq!(step_of(now, now, &none), 1);
        assert_eq!((step_of(key("2027S"), now, &none), step_of(key("2027W"), now, &none)), (0, -1));
        for step in -1..=20 {
            let start = start_of_step(step, now, &none).unwrap();
            assert_eq!(step_of(start, now, &none), step, "{step}");
        }
        // A semester of leave is no Fachsemester: one step back passes it.
        let leave: BTreeSet<SemesterKey> = [key("2026S")].into_iter().collect();
        assert_eq!(start_of_step(2, now, &leave), Some(key("2025W")));
        assert_eq!(step_of(key("2025W"), now, &leave), 2);
    }
}
