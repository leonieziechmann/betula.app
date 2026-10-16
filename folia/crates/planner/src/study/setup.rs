//! The first visit of „Mein Studium" (owner, 2026-10-04: the program is set once): the study as the
//! card of „Studiengang wechseln", empty (owner, 2026-10-16, concept A „Karte"; `card.rs`) — the
//! program, its study direction and the Fachsemester — and whether the semesters before the current
//! one are filled from the Regelstudienplan, so that only ticking off is left (`study::fill_past`).
//! „Studium anlegen" stores it all in „Mein Studiengang" at once. A program kept from elsewhere
//! (the program's page, the Stundenplan) is the one offered; so is its Studienbeginn. As in the
//! card nothing moves while it is used: the box that fills the semesters before is there in the
//! first Fachsemester too, where there are none and it says so.

use folia_calendar::semester::SemesterKey;
use folia_plans::study;
use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_stores::myprogram::program_name;

use super::card::{Ids, Part, StudyCard};
use super::side::{direction_options, kept_plan, plan_line, plans_of, program_items, title_of};
use super::{assumed_start, chosen, input_of, now_secs, rows_of, Setup as Asked, State, StudyCtx};
use crate::i18n;

#[component]
pub(super) fn Setup(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let kept = ctx.mine.map(|mine| untrack(move || mine.get())).unwrap_or_default();
    let (all, items) = program_items(ctx);
    let program = RwSignal::new(kept.program.clone());
    let source = plans_of(ctx, program.into());
    // What the visitor picked; until then what is kept, or assumed.
    let start = RwSignal::new(kept.start);
    let direction = RwSignal::new(None::<usize>);
    let fill = RwSignal::new(false);
    let gone = Memo::new(move |_| {
        ctx.state.with(|state| match state {
            State::Gone(name) => Some(name.clone()),
            _ => None,
        })
    });
    let pick_program = Callback::new(move |id: Option<String>| {
        if id.is_some() && id != program.get_untracked() {
            program.set(id);
            direction.set(None);
        }
    });
    // The plan picked, else the one kept with the program, else the first.
    let plan_index = Memo::new(move |_| {
        let picked = direction.get();
        source.with(|source| {
            let source = source.as_ref()?.as_ref()?;
            Some(picked.unwrap_or_else(|| {
                let kept = ctx.mine.map(|mine| untrack(move || mine.get())).unwrap_or_default();
                let same = kept.program.as_deref() == Some(source.program.id.as_str());
                if same { chosen(source, kept.caption.as_deref(), kept.direction.as_deref()).0.unwrap_or(0) } else { 0 }
            }))
        })
    });
    let leave = Memo::new(move |_| ctx.mine.map(|mine| mine.with(|doc| doc.leave.clone())).unwrap_or_default());
    let start_now = Memo::new(move |_| {
        let now = ctx.now.get();
        start.get().or_else(|| {
            source.with(|source| {
                let source = source.as_ref()?.as_ref()?;
                let index = plan_index.get().unwrap_or(0);
                let (_, core, _) = chosen(source, Some(&kept_plan(source, index).0), kept_plan(source, index).1.as_deref());
                Some(assumed_start(source, core, now))
            })
        })
    });
    // The Fachsemester before the current one, which the plan can fill.
    let before = Memo::new(move |_| {
        let (start, now) = (start_now.get()?, ctx.now.get());
        let has_plan = source.with(|source| source.as_ref().and_then(Option::as_ref).is_some_and(|source| !source.variants.is_empty()));
        let fs = study::fs_of(start, now, &leave.get())?;
        (has_plan && fs > 1).then(|| (t.study.fs_span)(1, fs - 1))
    });
    let title = Memo::new(move |_| program.with(|program| all.with(|all| title_of(all, program.as_deref()))));
    let line = Memo::new(move |_| source.with(|source| source.as_ref().and_then(Option::as_ref).map(|source| plan_line(source, plan_index.get().unwrap_or(0), t))));
    let directions = Memo::new(move |_| source.with(|source| source.as_ref().and_then(|source| source.as_ref().map(direction_options)).unwrap_or_default()));
    let ready = Memo::new(move |_| source.with(|source| matches!(source, Some(Some(_)))) && start_now.with(Option::is_some));

    let create = move |_| {
        let (Some(mine), Some(Some(source)), Some(start)) = (ctx.mine, source.get_untracked(), start_now.get_untracked()) else { return };
        let index = plan_index.get_untracked().unwrap_or(0);
        let (caption, page) = kept_plan(&source, index);
        mine.set_study(&source.program.id, &program_name(&source.program), &caption, page.as_deref(), start);
        // „bisherige Semester nach Regelstudienplan füllen".
        if fill.get_untracked() && before.get_untracked().is_some() {
            let (Some(plan), now, leave) = (ctx.plan, ctx.now.get_untracked(), leave.get_untracked()) else { return };
            let rows = ctx.rows.with_untracked(|rows| rows_of(&source, rows));
            plan.update_after_paint(move |doc| {
                let (shown, core, page) = chosen(&source, Some(&caption), page.as_deref());
                let setup = Asked { shown, core, page, start, start_stored: true, now, leave, until: None };
                let before = doc.clone();
                let input = input_of(&source, &setup, &before, &rows);
                study::fill_past(doc, &input, now_secs());
            });
        }
    };

    view! {
        <section class="panel st-setup" aria-labelledby="st-setup-title">
            <header class="st-setup-head">
                <span class="st-setup-icon" aria-hidden="true"><Icon name="graduation-cap"/></span>
                <div>
                    <h1 id="st-setup-title">{s.setup_title}</h1>
                    <p>{s.setup_lead}</p>
                    {move || gone.get().map(|name| view! { <p class="st-gone"><Icon name="triangle-alert"/><span>{(s.gone)(&name)}</span></p> })}
                </div>
            </header>
            <StudyCard
                ids=Ids { program: "st-program", direction: "st-direction", less: "st-fs-less", more: "st-fs-more" }
                items
                program=program
                on_program=pick_program
                title=title
                plan_line=line
                directions=directions
                direction=Signal::derive(move || plan_index.get().unwrap_or(0))
                on_direction=Callback::new(move |index: usize| direction.set(Some(index)))
                start=start_now
                on_start=Callback::new(move |key: SemesterKey| start.set(Some(key)))
                now=ctx.now
                leave=leave
                kept=Signal::derive(|| None)
                on_back=Callback::new(|_: Part| {})
            />
            // Whether the semesters before the current one are filled from the plan: in the first
            // Fachsemester there are none, and the box says so.
            <button
                class="st-check big st-fill"
                type="button"
                role="checkbox"
                id="st-fill"
                aria-checked=move || if fill.get() && before.with(Option::is_some) { "true" } else { "false" }
                disabled=move || before.with(Option::is_none)
                on:click=move |_| fill.update(|fill| *fill = !*fill)
            >
                <span class="st-box" class:with=move || fill.get() && before.with(Option::is_some) aria-hidden="true"><Icon name="check"/></span>
                {move || match before.get() {
                    Some(span) => view! { <span><b>{(s.fill_past)(&span)}</b><span>{s.fill_past_hint}</span></span> }.into_any(),
                    None => view! { <span><b>{s.fill_later}</b><span>{s.fill_later_hint}</span></span> }.into_any(),
                }}
            </button>
            <footer class="st-setup-foot">
                <p class="hint"><Icon name="shield-check"/><span>{s.stays_here}</span></p>
                <a class="ghost" href=t.path(folia_routes::url::PROGRAMS)>{s.all_programs}</a>
                <button class="btn primary" type="button" id="st-create" disabled=move || !ready.get() on:click=create>{s.create}<Icon name="arrow-right"/></button>
            </footer>
        </section>
    }
}
