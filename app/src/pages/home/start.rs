//! The way in for a first visit (owner, 2026-09-28: the sidebar made the start page confusing and
//! odd to look at, and a new visitor has to find their way — organically, without a pop-up or a
//! tour). Right under the first panel, three steps in the order a semester is planned: the program,
//! the modules, the Stundenplan. Each is one link, and each names the item of the navigation that
//! keeps it, with its icon and its name, so the navigation is learnt on the way.
//!
//! The steps follow what this browser has done (the app's alone, R9: on the server all of it is
//! empty, so the server's page is the one of a first visit): a step that is done wears a tick and
//! leads to what was done — the program set as „Mein Studiengang", the Merkliste, the
//! Stundenplan — and the first step not done yet is marked as the next one. Only words and colours
//! change, never a height, so nothing moves when the app takes over.
//!
//! The line under the heading sends whoever looks for one module into the search at the top
//! (`data-action="search"` in `enhance.js`; without JavaScript the link opens the catalog), and
//! the foot leads to the sections that explain Betula, for those who want to read first.

use catalog::url;
use leptos::prelude::*;

use crate::bookmarks::Bookmarks;
use crate::data::use_source;
use crate::i18n::{self, home::Step};
use crate::myprogram::{program_href, program_name, MineResolved, MyProgram};
use crate::studyplan::Studyplan;
use crate::ui::{Icon, Shortcut};

/// Which step comes next: the first one not done, none when all are.
fn next_step(done: [bool; 3]) -> Option<usize> {
    done.iter().position(|done| !done)
}

#[component]
pub fn StartPath() -> impl IntoView {
    let t = i18n::t();
    let home = &t.home;
    let source = use_source().ok();
    // „Mein Studiengang", while the stored program is in the snapshot (A.10), with the plan of its
    // stored Studienrichtung; the name as the app names it everywhere.
    let resolved = MineResolved::expect();
    let stored = MyProgram::expect();
    let program = Memo::new(move |_| {
        let program = resolved.and_then(MineResolved::exact)?;
        let (caption, direction) = stored.map(|mine| mine.with(|doc| (doc.caption.clone(), doc.direction.clone()))).unwrap_or_default();
        Some((program_href(source.as_ref(), &program, caption.as_deref(), direction.as_deref()), program_name(&program)))
    });
    let marked = Memo::new(move |_| Bookmarks::expect().map(Bookmarks::count).unwrap_or(0));
    let planned = Memo::new(move |_| Studyplan::expect().map(Studyplan::count).unwrap_or(0));
    let next = Memo::new(move |_| next_step([program.with(Option::is_some), marked.get() > 0, planned.get() > 0]));

    let steps = [
        StepView {
            step: &home.step_program,
            // The items of the navigation, as the rail and the bottom bar name them (`NavItems`);
            // the Merkliste and the Stundenplan are there with JavaScript only (R15).
            places: vec![("graduation-cap", t.app.study, false)],
            done: Signal::derive(move || program.with(Option::is_some)),
            href: Signal::derive(move || program.with(|program| program.as_ref().map_or_else(|| url::PROGRAMS.to_string(), |(href, _)| href.clone()))),
            state: Signal::derive(move || program.with(|program| program.as_ref().map(|(_, name)| name.clone()))),
        },
        StepView {
            step: &home.step_modules,
            places: vec![("layout-list", t.app.modules, false), ("bookmark", t.app.bookmarks, true)],
            done: Signal::derive(move || marked.get() > 0),
            href: Signal::derive(move || if marked.get() > 0 { url::BOOKMARKS.to_string() } else { url::CATALOG.to_string() }),
            state: Signal::derive(move || (marked.get() > 0).then(|| (home.marked)(marked.get()))),
        },
        StepView {
            step: &home.step_timetable,
            places: vec![("calendar-range", t.app.studyplan, true)],
            done: Signal::derive(move || planned.get() > 0),
            href: Signal::derive(|| url::STUDYPLAN.to_string()),
            state: Signal::derive(move || (planned.get() > 0).then(|| (home.planned)(planned.get()))),
        },
    ];

    view! {
        <section class="panel start-path" id="loslegen" aria-labelledby="loslegen-titel">
            <header class="block-head">
                <h2 id="loslegen-titel">{home.start_heading}</h2>
                <a class="ghost" href=t.path(url::CATALOG) data-action="search" title=home.search_now_title>
                    <Icon name="search"/>{home.search_now}<Shortcut keys=t.common.search_shortcut/>
                </a>
                <p>{home.start_lead}</p>
            </header>
            <ol class="start-steps" aria-label=home.steps_label>
                {steps.into_iter().enumerate().map(|(i, step)| step.view(i, next)).collect_view()}
            </ol>
            <p class="start-foot">
                <span>{home.learn_first}</span>
                {[("funktionen", home.abilities), ("fragen", home.questions), ("im-detail", t.home_detail.heading)].map(|(id, name)| view! {
                    <a href=format!("#{id}") data-action="jump">{name}<Icon name="chevron-right"/></a>
                })}
            </p>
        </section>
    }
}

/// One step as the page shows it.
struct StepView {
    step: &'static Step,
    /// Where the navigation keeps it: icon, name, and whether only the browser app has it.
    places: Vec<(&'static str, &'static str, bool)>,
    done: Signal<bool>,
    /// The app's path, without the language's prefix.
    href: Signal<String>,
    /// What stands in place of the way on once the step is done.
    state: Signal<Option<String>>,
}

impl StepView {
    fn view(self, i: usize, next: Memo<Option<usize>>) -> impl IntoView {
        let t = i18n::t();
        let StepView { step, places, done, href, state } = self;
        // Where every item is the app's, the line goes with them.
        let app_only = places.iter().all(|(_, _, app_only)| *app_only);
        view! {
            <li class="start-step" class:is-done=move || done.get() class:is-next=move || next.get() == Some(i)>
                <a href=move || t.path(&href.get()) aria-current=move || (next.get() == Some(i)).then_some("step")>
                    <span class="start-mark" aria-hidden="true">
                        {move || if done.get() { view! { <Icon name="check"/> }.into_any() } else { view! { <b class="num">{i + 1}</b> }.into_any() }}
                    </span>
                    <h3>{move || done.get().then_some(t.home.step_done).map(|done| view! { <span class="visually-hidden">{done}</span> })}{step.title}</h3>
                    <p>{step.text}</p>
                    <span class="start-go">
                        {move || match state.get() {
                            Some(state) => view! { <span class="start-state"><Icon name="check"/><span>{state}</span></span> }.into_any(),
                            None => view! { <span>{step.go}</span> }.into_any(),
                        }}
                        <Icon name="arrow-right"/>
                    </span>
                    <span class="start-where" class:js-only=app_only>
                        <span>{t.home.step_where}</span>
                        {places.into_iter().map(|(icon, name, app_only)| view! {
                            <span class="start-place" class:js-only=app_only><Icon name=icon/>{name}</span>
                        }).collect_view()}
                    </span>
                </a>
            </li>
        }
    }
}

#[cfg(test)]
mod tests {
    use super::next_step;

    #[test]
    fn the_next_step_is_the_first_not_done() {
        assert_eq!(next_step([false, false, false]), Some(0));
        assert_eq!(next_step([true, false, false]), Some(1));
        // Modules marked without a program chosen: the program is still the next step.
        assert_eq!(next_step([false, true, true]), Some(0));
        assert_eq!(next_step([true, true, false]), Some(2));
        assert_eq!(next_step([true, true, true]), None);
    }
}
