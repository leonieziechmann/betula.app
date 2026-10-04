//! The sidebar of „Mein Studium", top to bottom: „Studiengang" (the picker of „Mein Studiengang"
//! and the ways to its Regelstudienplan, its electives and areas and to all programs), its study
//! direction where it has several plans, the Studienbeginn (assumed until one is stored), the way
//! to the Stundenplan, and how the plan reads; the storage hint last, as on the Stundenplan. On a
//! phone the sidebar is the sheet „Anpassen".
//!
//! What the visitor picks here is „Mein Studiengang"'s (`MyProgram`): it is stored at once, and
//! the page follows it. Each control reads a memo of its own (R5).

use folia_calendar::semester::{fachsemester, SemesterKey};
use folia_model::rows::Program;
use folia_pages::ask::ProgramsAsk;
use folia_routes::url::{self, ProgramTab, ProgramUrl};
use leptos::prelude::*;

use folia_design::combobox::{ComboItem, Combobox};
use folia_design::ui::Icon;
use folia_stores::myprogram::{po_of, program_name, ProgramPlans};

use super::{Plans, StudyCtx};
use crate::i18n;

/// How many semesters before the current one the Studienbeginn's select offers, and after it.
const STARTS_BEFORE: i32 = 16;
const STARTS_AFTER: i32 = 2;

#[component]
pub(super) fn StudySidebar(ctx: StudyCtx) -> impl IntoView {
    view! {
        <ProgramGroup ctx/>
        <DirectionGroup ctx/>
        <StartGroup ctx/>
        <WaysGroup ctx/>
        <LegendGroup ctx/>
        <div class="fgroup">
            <p class="hint storage-hint"><Icon name="shield-check"/><span>{i18n::t().study.storage_hint}</span></p>
        </div>
    }
}

/// Every program of the snapshot, as the pickers offer them: newest PO first.
pub(super) fn program_items(ctx: StudyCtx) -> (Memo<Vec<Program>>, Signal<Vec<ComboItem>>) {
    let all = Memo::new(move |before| match ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&ProgramsAsk {}))) {
        Some(now) => folia_pages::ask::unless_pending(now, before, Result::unwrap_or_default),
        None => Vec::new(),
    });
    let items = Signal::derive(move || {
        all.with(|all| {
            all.iter()
                .map(|program| ComboItem::new(program.id.clone(), program.name.clone(), format!("{} · PO {}", program.degree(), po_of(program)), i64::from(program.is_latest_po)))
                .collect::<Vec<_>>()
        })
    });
    (all, items)
}

/// What a pick of the picker does: the program is „Mein Studiengang", with its first plan (the
/// study direction is chosen below it); the Studienbeginn stays.
pub(super) fn keep_program(ctx: StudyCtx, all: Memo<Vec<Program>>) -> Callback<Option<String>> {
    Callback::new(move |id: Option<String>| {
        let (Some(id), Some(mine)) = (id, ctx.mine) else { return };
        let Some(program) = all.with_untracked(|all| all.iter().find(|program| program.id == id).cloned()) else { return };
        mine.set_program(&program.id, &program_name(&program), "", None);
    })
}

/// „Studiengang": the picker, and the ways to the program's pages and to all programs.
#[component]
fn ProgramGroup(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let (all, items) = program_items(ctx);
    let selected = Signal::derive(move || ctx.mine.and_then(|mine| mine.with(|doc| doc.program.clone())));
    // The program's pages, with the plan studied.
    let links = Memo::new(move |_| {
        ctx.state.with(|state| {
            let ready = state.ready()?;
            let plan = ProgramUrl::new(&ready.program.slug, ProgramTab::Plan).with_variant(ready.setup.shown.map_or(1, |shown| shown + 1)).path();
            Some((plan, url::program_path(&ready.program.slug, ProgramTab::Areas)))
        })
    });
    view! {
        <div class="fgroup first st-program">
            <p class="flabel label">{s.program}</p>
            <Combobox
                id="st-program"
                label=s.program
                placeholder=s.choose_program
                search_placeholder=s.search_program
                icon="graduation-cap"
                min_width=480.0
                items
                selected
                on_select=keep_program(ctx, all)
                clearable=false
            />
            <div class="st-ways">
                {move || links.get().map(|(plan, areas)| view! {
                    <a class="action" href=t.path(&plan)><Icon name="file-check-2"/><span>{s.to_plan}</span><Icon name="chevron-right"/></a>
                    <a class="action" href=t.path(&areas)><Icon name="layout-list"/><span>{s.to_areas}</span><Icon name="chevron-right"/></a>
                })}
                <a class="action" href=t.path(url::PROGRAMS)><Icon name="graduation-cap"/><span>{s.all_programs}</span><Icon name="chevron-right"/></a>
            </div>
        </div>
    }
}

/// „Studienrichtung": the program's plans by their labels, where it has two or more. A pick keeps
/// it with „Mein Studiengang" as the program's page does (`ProgramPlans::kept`).
#[component]
fn DirectionGroup(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let plans = Memo::new(move |_| ctx.state.with(|state| state.ready().map(|ready| ready.plans.clone()).unwrap_or_default()));
    let shown = Memo::new(move |_| ctx.state.with(|state| state.ready().and_then(|ready| ready.setup.shown)));
    let pick = move |ev: leptos::ev::Event| {
        let (Ok(index), Some(mine)) = (event_target_value(&ev).parse::<usize>(), ctx.mine) else { return };
        let kept = ctx.plans.with_untracked(|plans| match plans {
            Plans::Found(source) => {
                let (caption, direction) = ProgramPlans::new(&source.variants, source.supplements.clone()).kept(index);
                Some((source.program.clone(), caption, direction))
            }
            _ => None,
        });
        if let Some((program, caption, direction)) = kept {
            mine.set_program(&program.id, &program_name(&program), &caption, direction.as_deref());
        }
    };
    move || {
        (!plans.with(Vec::is_empty)).then(|| {
            view! {
                <div class="fgroup">
                    <p class="flabel label">{t.study.direction}</p>
                    <span class="select-wrap plain">
                        <select aria-label=t.study.direction prop:value=move || shown.get().map(|shown| shown.to_string()).unwrap_or_default() on:change=pick>
                            <For
                                each=move || plans.get()
                                key=|entry| entry.clone()
                                children=move |(index, label): (usize, String)| view! { <option value=index.to_string() selected=move || shown.get() == Some(index)>{label}</option> }
                            />
                        </select>
                        <Icon name="chevrons-up-down"/>
                    </span>
                </div>
            }
        })
    }
}

/// „Studienbeginn": the semesters a study may have begun in, each with the Fachsemester it makes
/// the current one. Until one is stored the page assumes one (`studyplan::intake_start`) and asks
/// whether it is right; „Stimmt" stores it.
#[component]
fn StartGroup(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let setup = Memo::new(move |_| ctx.state.with(|state| state.ready().map(|ready| ready.setup)));
    let options = Memo::new(move |_| {
        let Some(setup) = setup.get() else { return Vec::new() };
        let now = setup.now;
        (-STARTS_AFTER..=STARTS_BEFORE)
            .filter_map(|n| now.plus(-n))
            .map(|start| {
                let label = match fachsemester(now, start) {
                    Some(fs) => (s.start_entry)(&start.label(t.locale), fs),
                    None => start.label(t.locale),
                };
                (start.key(), label)
            })
            .collect::<Vec<_>>()
    });
    let chosen = Memo::new(move |_| setup.get().map(|setup| setup.start.key()));
    let assumed = Memo::new(move |_| setup.get().is_some_and(|setup| !setup.start_stored));
    let pick = move |ev: leptos::ev::Event| {
        if let (Some(start), Some(mine)) = (SemesterKey::parse(&event_target_value(&ev)), ctx.mine) {
            mine.set_start(Some(start));
        }
    };
    let confirm = move |_| {
        if let (Some(setup), Some(mine)) = (setup.get_untracked(), ctx.mine) {
            mine.set_start(Some(setup.start));
        }
    };
    move || {
        setup.get().is_some().then(|| {
            view! {
                <div class="fgroup st-start">
                    <p class="flabel label">{s.start}</p>
                    <span class="select-wrap plain">
                        <select id="st-start" aria-label=s.start prop:value=move || chosen.get().unwrap_or_default() on:change=pick>
                            <For
                                each=move || options.get()
                                key=|entry| entry.clone()
                                children=move |(key, label): (String, String)| {
                                    let value = key.clone();
                                    view! { <option value=key selected=move || chosen.get().as_deref() == Some(value.as_str())>{label}</option> }
                                }
                            />
                        </select>
                        <Icon name="chevrons-up-down"/>
                    </span>
                    {move || assumed.get().then(|| view! {
                        <p class="action note-action ask st-assumed">
                            <span>{s.start_assumed}</span>
                            <button class="mini primary hit" type="button" on:click=confirm>{s.start_confirm}</button>
                        </p>
                    })}
                </div>
            }
        })
    }
}

/// The way to the Stundenplan, the timetable of the current semester.
#[component]
fn WaysGroup(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let ready = Memo::new(move |_| ctx.state.with(|state| state.ready().is_some()));
    move || {
        ready.get().then(|| {
            view! {
                <div class="fgroup actions">
                    <a class="action" href=t.path(url::STUDYPLAN) rel="nofollow"><Icon name="calendar-range"/><span>{t.study.to_timetable}</span><Icon name="chevron-right"/></a>
                </div>
            }
        })
    }
}

/// How the plan reads: the marks a row may carry. With a study to read.
#[component]
fn LegendGroup(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let ready = Memo::new(move |_| ctx.state.with(|state| state.ready().is_some()));
    move || ready.get().then(|| view! {
        <div class="fgroup st-legend">
            <p class="flabel label">{s.legend}</p>
            <ul>
                <li><span class="st-mark overdue" aria-hidden="true"></span>{s.legend_overdue}</li>
                <li><span class="st-mark moved" aria-hidden="true"></span>{s.legend_moved}</li>
                <li><span class="st-mark turnus" aria-hidden="true"></span>{s.legend_turnus}</li>
                <li><span class="st-mark done" aria-hidden="true"></span>{s.legend_done}</li>
            </ul>
        </div>
    })
}
