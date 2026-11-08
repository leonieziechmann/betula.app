//! „Anderer Bereich …" (owner, 2026-11-08: „Ich möchte dass es möglich ist jedes modul zu jedem
//! möglichen bereich zuzuordnen. Einfach aus dem grund, dass es bei uns an der uni häufig ist, dass
//! ausnahmen gemacht werden. Das einzige ist, ich möchte, dass es angezeigt wird, wenn man das
//! gemacht hat und dass das man das nicht ausversehen macht. Also wäre es wahrscheinlich sinnvoll
//! das in ein modal zu packen, dass man von dem aktuellen menü aufrufen kann."): the dialog behind
//! „Zählt für ›" of a module's menu, where it can count for any subgroup of any area — an area's
//! Pflichtmodule, any choice — or where the study puts it („Automatisch").
//!
//! Each subgroup says whether the module belongs there by the plan („passt") or would be an
//! exception („Ausnahme"), what it has of what it asks, and where the module counts now. Nothing
//! changes until „Zuordnen": picking an exception says so over the buttons, and the button says
//! „Als Ausnahme zuordnen". The module then carries the mark „Ausnahme" in its semester and in its
//! area (`study::Item::exception`), and „Rückgängig" of the note takes it back.

use folia_calendar::semester::SemesterKey;
use folia_plans::bench::GroupKind;
use leptos::prelude::*;

use super::bench::{count_for, counts_choices, Target};
use super::dialog::DialogHead;
use super::{n, StudyCtx};
use crate::i18n;

/// What is picked: the study's own choice, or a subgroup by its area and kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    Auto,
    Group(usize, GroupKind),
}

/// The subgroups of one area, under its name and colour.
#[derive(Clone, Debug, PartialEq)]
struct Section {
    name: String,
    tone: &'static str,
    targets: Vec<Target>,
}

#[component]
pub(super) fn CountDialog(ctx: StudyCtx, semester: SemesterKey, id: String) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let id = StoredValue::new(id);
    let counts = Memo::new(move |_| counts_choices(ctx, semester, &id.get_value(), true, t));
    let title = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let item = ready.study.semester(semester)?.items.iter().find(|item| item.module_id() == Some(id.get_value().as_str()))?;
            Some(match item.credits {
                Some(credits) => (item.name.clone(), format!("{} · {}", item.name, (t.study.credits)(&n(credits, t)))),
                None => (item.name.clone(), item.name.clone()),
            })
        })
        .flatten()
        .unwrap_or_else(|| (id.get_value(), id.get_value()))
    });
    let sections = Memo::new(move |_| {
        let Some(counts) = counts.get() else { return Vec::new() };
        ctx.with_ready(|ready| {
            let mut sections: Vec<(usize, Section)> = Vec::new();
            for target in counts.targets {
                match sections.iter_mut().find(|(area, _)| *area == target.area) {
                    Some((_, section)) => section.targets.push(target),
                    None => sections.push((target.area, Section { name: ready.area_name(Some(target.area), t), tone: ready.tone(Some(target.area)), targets: vec![target] })),
                }
            }
            sections.into_iter().map(|(_, section)| section).collect::<Vec<_>>()
        })
        .unwrap_or_default()
    });
    // Where it counts now, which the dialog opens with.
    let current = Memo::new(move |_| {
        counts.with(|counts| {
            let counts = counts.as_ref()?;
            let now = counts.targets.iter().find(|target| target.current).map(|target| Pick::Group(target.area, target.kind));
            Some(if counts.auto { Pick::Auto } else { now.unwrap_or(Pick::Auto) })
        })
        .unwrap_or(Pick::Auto)
    });
    let picked = RwSignal::new(None::<Pick>);
    let chosen = Signal::derive(move || picked.get().unwrap_or_else(|| current.get()));
    let target_of = move |pick: Pick| -> Option<Target> {
        let Pick::Group(area, kind) = pick else { return None };
        counts.with_untracked(|counts| counts.as_ref()?.targets.iter().find(|target| target.area == area && target.kind == kind).cloned())
    };
    let exception = Memo::new(move |_| match chosen.get() {
        Pick::Group(area, kind) => counts.with(|counts| counts.as_ref().and_then(|counts| counts.targets.iter().find(|target| target.area == area && target.kind == kind)).map(|target| (target.exception, target.name.clone()))),
        Pick::Auto => None,
    });
    let apply = move |_| {
        let pick = chosen.get_untracked();
        if pick != current.get_untracked() {
            let target = target_of(pick).map(|target| (target.area, target.kind, target.name, target.exception));
            count_for(ctx, semester, id.get_value(), title.get_untracked().0, target, t);
        }
        ctx.dialog.set(None);
    };

    // One way to pick: a radio and what it says, „jetzt" where the module counts now; the arrows go
    // from one to the next.
    let option = move |pick: Pick, name: String, hint: Option<String>, figure: Option<String>, mark: Option<(&'static str, &'static str)>, now: bool| {
        view! {
            <label class="st-cf-opt" class:is-picked=move || chosen.get() == pick>
                <input type="radio" name="st-cf" prop:checked=move || chosen.get() == pick on:change=move |_| picked.set(Some(pick))/>
                <span class="st-cf-name">
                    <b>{name}</b>
                    {hint.map(|hint| view! { <small>{hint}</small> })}
                </span>
                {now.then(|| view! { <span class="flag neutral">{b.count_now}</span> })}
                {mark.map(|(class, text)| view! { <span class=class>{text}</span> })}
                {figure.map(|figure| view! { <span class="st-cf-lp num">{figure}</span> })}
            </label>
        }
    };

    view! {
        <DialogHead ctx title=b.count_title sub=title.get_untracked().1/>
        <div class="st-dlg-body st-cf">
            <p class="st-cf-lead">{b.count_lead}</p>
            <div role="radiogroup" aria-labelledby="st-dialog-title">
                {option(Pick::Auto, b.count_auto.to_string(), Some(b.count_auto_hint.to_string()), None, None, false)}
                {move || sections.get().into_iter().map(|section| view! {
                    <section class="st-cf-area" style=format!("--c: {}", section.tone)>
                        <h3><span class="st-dot"></span>{section.name}</h3>
                        {section.targets.into_iter().map(|target| {
                            // Calm in the list; picked, the foot says what an exception means.
                            let mark = if target.exception { ("st-cf-mark", b.count_exception) } else { ("st-cf-mark fits", b.count_fits) };
                            let (have, asked) = target.figures;
                            let figure = (asked > 0.0).then(|| (b.count_figure)(&n(have, t), &n(asked, t)));
                            option(Pick::Group(target.area, target.kind), target.name, None, figure, Some(mark), target.current)
                        }).collect_view()}
                    </section>
                }).collect_view()}
            </div>
        </div>
        <footer class="st-dlg-foot">
            <p class="st-foot-state st-cf-warn" class:warn=move || exception.with(|exception| exception.as_ref().is_some_and(|(on, _)| *on)) aria-live="polite">
                {move || exception.get().filter(|(on, _)| *on).map(|(_, group)| (b.count_warn)(&title.get().0, &group))}
            </p>
            <button class="btn secondary" type="button" on:click=move |_| ctx.dialog.set(None)>{t.study.cancel}</button>
            <button class="btn primary" type="button" id="st-cf-go" on:click=apply>
                {move || if exception.with(|exception| exception.as_ref().is_some_and(|(on, _)| *on)) { b.count_apply_exception } else { b.count_apply }}
            </button>
        </footer>
    }
}
