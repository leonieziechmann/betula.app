//! The way in for a first visit (owner, 2026-09-28: the sidebar made the start page confusing and
//! odd to look at, and a new visitor has to find their way — organically, without a pop-up or a
//! tour). Right under the first panel, three steps in the order a semester is planned: the program,
//! the modules, the Stundenplan. Each names the item of the navigation that keeps it, with its icon
//! and its name, so the navigation is learnt on the way.
//!
//! One click a step (owner, the same day: „Das sind hier 300 Schritte, um ans Ziel zu kommen. Ich
//! will dass man bei der Einführung immer nur so ein Click pro Step braucht", and whoever knows the
//! site goes straight on): each step has one button, and it does the step or leads to where it is
//! done. „Studiengang wählen" opens a picker of all programs in place, and a pick is „Mein
//! Studiengang" (`ProgramPick`, the first panel's button too); „Module deines Studiengangs" is the
//! catalog of that program; „Fachsemester übernehmen" the Stundenplan with its Regelstudienplan
//! ready to take over (`import=mine`).
//!
//! The steps follow what this browser has done (the app's alone, R9: on the server all of it is
//! empty, so the server's page is the one of a first visit): a step that is done wears a tick and
//! says what was done — the program, the Merkliste, the Stundenplan — in place of the navigation's
//! items, and the first step not done yet is the next one, its button in the accent. Only words
//! and colours change, never a height, so nothing moves when the app takes over.
//!
//! The line under the heading sends whoever looks for one module into the search at the top
//! (`data-action="search"` in `enhance.js`; without JavaScript the link opens the catalog), and
//! the foot leads to the sections that explain Betula, for those who want to read first.

use catalog::queries;
use catalog::url::{self, StudyplanUrl};
use leptos::prelude::*;

use crate::bookmarks::Bookmarks;
use crate::combobox::{ComboItem, Combobox};
use crate::data::use_source;
use crate::i18n::{self, home::Step};
use crate::myprogram::{program_href, program_name, po_of, MineResolved, MyProgram};
use crate::nav;
use crate::studyplan::Studyplan;
use crate::ui::{Icon, Shortcut};

/// The browser app (`csr`): only there is anything stored, and only there does a picker work.
const APP: bool = cfg!(feature = "csr");

/// Which step comes next: the first one not done, none when all are.
fn next_step(done: [bool; 3]) -> Option<usize> {
    done.iter().position(|done| !done)
}

/// „Studiengang wählen": in the browser app, while this browser keeps no program, a picker of all
/// programs (the catalog's `Combobox`), and a pick is „Mein Studiengang" at once — the page stays
/// and the way in goes on. Once a program is kept, and without JavaScript, the way to all programs,
/// „Alle Studiengänge" (owner, 2026-09-28). The server's link carries both words and the stylesheet
/// shows the ones the app will show (`html.js`, and `html.mine` where the head script found a
/// program), so they stay when the app takes over.
#[component]
pub fn ProgramPick(
    /// The id of the button; the picker's popup derives the ids of its parts from it.
    id: &'static str,
    /// The classes of the link (`btn secondary`, `start-btn`); a picker is styled by its place.
    class: &'static str,
) -> impl IntoView {
    let t = i18n::t();
    if !APP {
        return view! {
            <a id=id class=format!("{class} program-pick") href=t.path(url::PROGRAMS)>
                <Icon name="graduation-cap"/>
                <span class="if-choose">{t.home.choose_program}</span>
                <span class="if-all">{t.home.all_programs}</span>
            </a>
        }
        .into_any();
    }
    let mine = MyProgram::expect();
    let source = use_source().ok();
    let kept = Memo::new(move |_| mine.is_some_and(|mine| mine.with(|doc| doc.program.is_some())));
    // Asked of the catalog only once the picker opens: a memo computes when it is first read.
    let programs = Memo::new(move |_| source.as_ref().and_then(|source| source.run(|db| queries::programs(db)).ok()).unwrap_or_default());
    let items = Signal::derive(move || {
        programs.with(|all| {
            all.iter()
                .map(|program| ComboItem::new(program.id.clone(), program.name.clone(), format!("{} · PO {}", program.degree(), po_of(program)), i64::from(program.is_latest_po)))
                .collect::<Vec<_>>()
        })
    });
    let pick = Callback::new(move |picked: Option<String>| {
        let Some(picked) = picked else { return };
        let Some(program) = programs.with_untracked(|all| all.iter().find(|program| program.id == picked).cloned()) else { return };
        if let Some(mine) = mine {
            mine.set_program(&program.id, &program_name(&program), "", None);
        }
        // The picker gives way to „Alle Studiengänge", under the same id: the focus goes there.
        request_animation_frame(move || nav::focus_by_id(id));
    });
    (move || {
        if kept.get() {
            view! { <a id=id class=class href=t.path(url::PROGRAMS)><Icon name="graduation-cap"/>{t.home.all_programs}</a> }.into_any()
        } else {
            view! {
                <div class="pick-combo">
                    <Combobox
                        id=id
                        label=t.catalog.program
                        placeholder=t.home.choose_program
                        search_placeholder=t.catalog.search_program
                        icon="graduation-cap"
                        min_width=420.0
                        items
                        selected=Signal::derive(|| None::<String>)
                        on_select=pick
                        clearable=false
                    />
                </div>
            }
            .into_any()
        }
    })
    .into_any()
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
    let has_program = Memo::new(move |_| program.with(Option::is_some));
    let marked = Memo::new(move |_| Bookmarks::expect().map(Bookmarks::count).unwrap_or(0));
    let planned = Memo::new(move |_| Studyplan::expect().map(Studyplan::count).unwrap_or(0));
    let next = Memo::new(move |_| next_step([has_program.get(), marked.get() > 0, planned.get() > 0]));
    // The catalog as a way in: the program's while it is kept and known (`MineResolved::catalog_href`).
    let catalog = Memo::new(move |_| resolved.map_or_else(|| url::CATALOG.to_string(), MineResolved::catalog_href));
    let import = StudyplanUrl { import: Some("mine".to_string()), ..Default::default() }.path();

    let steps = [
        StepView {
            step: &home.step_program,
            // The items of the navigation, as the rail and the bottom bar name them (`NavItems`);
            // the Merkliste and the Stundenplan are there with JavaScript only (R15).
            places: vec![("graduation-cap", t.app.study, false)],
            done: has_program.into(),
            state: Signal::derive(move || program.get().map(|(href, name)| (name, Some(href)))),
            action: view! { <ProgramPick id="start-program" class="start-btn"/> }.into_any(),
        },
        StepView {
            step: &home.step_modules,
            places: vec![("layout-list", t.app.modules, false), ("bookmark", t.app.bookmarks, true)],
            done: Signal::derive(move || marked.get() > 0),
            state: Signal::derive(move || (marked.get() > 0).then(|| ((home.marked)(marked.get()), Some(url::BOOKMARKS.to_string())))),
            action: view! {
                <a class="start-btn" href=move || t.path(&catalog.get())>
                    <Icon name="layout-list"/>{move || if has_program.get() { home.mine_modules } else { home.to_catalog }}
                </a>
            }
            .into_any(),
        },
        StepView {
            step: &home.step_timetable,
            places: vec![("calendar-range", t.app.studyplan, true)],
            done: Signal::derive(move || planned.get() > 0),
            state: Signal::derive(move || (planned.get() > 0).then(|| ((home.planned)(planned.get()), None))),
            // With a program and nothing planned yet, its Regelstudienplan waits in the Stundenplan
            // to be taken over; else the Stundenplan as it is.
            action: {
                let take = Memo::new(move |_| has_program.get() && planned.get() == 0);
                view! {
                    <a class="start-btn" href=move || t.path(if take.get() { import.as_str() } else { url::STUDYPLAN })>
                        <Icon name="calendar-range"/>{move || if take.get() { home.take_semester } else { home.to_timetable }}
                    </a>
                }
                .into_any()
            },
        },
    ];

    view! {
        <section class="panel start-path" id="loslegen" aria-labelledby="loslegen-titel">
            <super::Branch side=super::Side::Left shape=1 at=36/>
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
    /// What was done, and where it leads (where the button does not lead there already): it
    /// stands in place of the navigation's items once the step is done.
    state: Signal<Option<(String, Option<String>)>>,
    /// The step's one button.
    action: AnyView,
}

impl StepView {
    fn view(self, i: usize, next: Memo<Option<usize>>) -> impl IntoView {
        let t = i18n::t();
        let StepView { step, places, done, state, action } = self;
        // Where every item is the app's, the line goes with them.
        let app_only = places.iter().all(|(_, _, app_only)| *app_only);
        let places = StoredValue::new(places);
        view! {
            <li class="start-step" class:is-done=move || done.get() class:is-next=move || next.get() == Some(i) aria-current=move || (next.get() == Some(i)).then_some("step")>
                <span class="start-mark" aria-hidden="true">
                    {move || if done.get() { view! { <Icon name="check"/> }.into_any() } else { view! { <b class="num">{i + 1}</b> }.into_any() }}
                </span>
                <h3>{move || done.get().then_some(t.home.step_done).map(|done| view! { <span class="visually-hidden">{done}</span> })}{step.title}</h3>
                <p>{step.text}</p>
                <div class="start-act">{action}</div>
                {move || match state.get() {
                    Some((text, Some(href))) => view! {
                        <p class="start-where start-state"><a href=t.path(&href)><Icon name="check"/><span>{text}</span><Icon name="chevron-right"/></a></p>
                    }
                    .into_any(),
                    Some((text, None)) => view! { <p class="start-where start-state"><span><Icon name="check"/><span>{text}</span></span></p> }.into_any(),
                    None => view! {
                        <p class="start-where" class:js-only=app_only>
                            <span>{t.home.step_where}</span>
                            {places.with_value(|places| places.iter().map(|&(icon, name, app_only)| view! {
                                <span class="start-place" class:js-only=app_only><Icon name=icon/>{name}</span>
                            }).collect_view())}
                        </p>
                    }
                    .into_any(),
                }}
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
