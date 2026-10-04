//! The first visit of „Mein Studium" (owner, 2026-10-04: the program is set once): the program, the
//! Studienbeginn and the study direction, and how the semesters before the current one begin —
//! empty, or filled from the Regelstudienplan, so that only ticking off is left (`study::fill_past`).
//! „Studium anlegen" stores it all in „Mein Studiengang" at once. A program kept from elsewhere
//! (the program's page, the Stundenplan) is the one offered; so is its Studienbeginn.

use folia_calendar::semester::SemesterKey;
use folia_plans::study;
use leptos::prelude::*;

use folia_design::combobox::Combobox;
use folia_design::ui::Icon;
use folia_stores::myprogram::program_name;

use super::side::{direction_options, kept_plan, plans_of, program_items, start_options, PlainSelect};
use super::{assumed_start, chosen, input_of, now_secs, rows_of, Setup as Asked, State, StudyCtx};
use crate::i18n;

#[component]
pub(super) fn Setup(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let kept = ctx.mine.map(|mine| untrack(move || mine.get())).unwrap_or_default();
    let (_, items) = program_items(ctx);
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
    let options = Memo::new(move |_| start_options(ctx.now.get(), &leave.get(), t));
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
            <div class="st-fields">
                <label class="st-field wide">
                    <span class="label">{s.program}</span>
                    <Combobox
                        id="st-program"
                        label=s.program
                        placeholder=s.choose_program
                        search_placeholder=s.search_program
                        icon="graduation-cap"
                        min_width=420.0
                        items
                        selected=Signal::derive(move || program.get())
                        on_select=pick_program
                        clearable=false
                    />
                </label>
                <label class="st-field">
                    <span class="label">{s.start}</span>
                    <PlainSelect
                        id="st-start"
                        label=s.start
                        options=Signal::derive(move || options.get())
                        chosen=Signal::derive(move || start_now.get().map(SemesterKey::key).unwrap_or_default())
                        pick=Callback::new(move |value: String| start.set(SemesterKey::parse(&value)))
                        disabled=Signal::derive(move || program.with(Option::is_none))
                    />
                </label>
                <label class="st-field">
                    <span class="label">{s.direction}</span>
                    {move || if directions.with(Vec::is_empty) {
                        view! { <span class="select-wrap plain"><select id="st-direction" aria-label=s.direction disabled=true><option>{s.no_direction}</option></select><Icon name="chevrons-up-down"/></span> }.into_any()
                    } else {
                        view! {
                            <PlainSelect
                                id="st-direction"
                                label=s.direction
                                options=Signal::derive(move || directions.get())
                                chosen=Signal::derive(move || plan_index.get().unwrap_or(0).to_string())
                                pick=Callback::new(move |value: String| direction.set(value.parse().ok()))
                            />
                        }.into_any()
                    }}
                </label>
            </div>
            {move || before.get().map(|span| view! {
                <fieldset class="st-choice">
                    <legend>{s.past_legend}</legend>
                    <button type="button" role="radio" aria-checked=move || if fill.get() { "false" } else { "true" } on:click=move |_| fill.set(false)>
                        <span class="st-radio" aria-hidden="true"></span>
                        <span><b>{s.start_empty}</b><span>{s.start_empty_hint}</span></span>
                    </button>
                    <button type="button" role="radio" id="st-fill" aria-checked=move || if fill.get() { "true" } else { "false" } on:click=move |_| fill.set(true)>
                        <span class="st-radio" aria-hidden="true"></span>
                        <span><b>{(s.fill_past)(&span)}</b><span>{s.fill_past_hint}</span></span>
                    </button>
                </fieldset>
            })}
            <footer class="st-setup-foot">
                <p class="hint"><Icon name="shield-check"/><span>{s.stays_here}</span></p>
                <a class="ghost" href=t.path(folia_routes::url::PROGRAMS)>{s.all_programs}</a>
                <button class="btn primary" type="button" id="st-create" disabled=move || !ready.get() on:click=create>{s.create}<Icon name="arrow-right"/></button>
            </footer>
        </section>
    }
}
