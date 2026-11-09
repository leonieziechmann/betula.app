//! „Umfang …" of a placeholder (owner, 2026-11-09: „Überlege dir mal wie man das so bei wp
//! bereichen wie bei der informatik macht, wo man selbst die gewichtung wählen kann, die man
//! machen möchte. Also wie man das richtig darstellt"): the dialog behind the entry of a
//! placeholder's menu that sets how many credits it keeps in its semester (`Placeholder::size`).
//!
//! A placeholder is a place for modules: it stands until they hold what it asks, and says how far
//! they are (`study::Item::fills`). What it asks is what its row says; a row of a range („Komplex
//! Praktische Informatik, 10–24") asks its least until the student says more, for the plan leaves
//! the rest to them. So the weighting is the student's, set where it is planned: a stepper for the
//! placeholder, with what the plan says and what is in it already, and under it the group the row
//! is part of (`choices::Group::weights`) — every member with what its modules hold and what its
//! placeholders keep, the least marked, the sum against what the group asks — moving as the
//! stepper turns. Nothing is refused (CLAUDE.md, „Betula does not say no"): below the least or
//! above the most it says what that means. „Wie im Plan" gives the row's own figure back.

use folia_calendar::semester::SemesterKey;
use folia_design::ui::Icon;
use folia_plans::bench as model;
use folia_plans::choices::Weight;
use folia_plans::study::{self, Subject};
use leptos::prelude::*;

use super::dialog::DialogHead;
use super::{n, StudyCtx};
use crate::i18n::{self, Texts};

/// The most a placeholder may be set to here: more than any row asks.
const MOST: f64 = 99.0;

/// The placeholder as the dialog shows it.
#[derive(Clone, Debug, PartialEq)]
struct Place {
    name: String,
    tone: &'static str,
    /// What its row says: the least and the most.
    least: f64,
    most: f64,
    /// What it asks now, and what its modules hold.
    asks: f64,
    held: f64,
    /// The plan's caption and `ord` of its row.
    caption: String,
    ord: i64,
}

/// A member of the group as the dialog draws it.
#[derive(Clone, Debug, PartialEq)]
struct Member {
    name: String,
    weight: Weight,
    /// The member the placeholder is of.
    own: bool,
}

/// The group the row is part of, with the placeholder turned to the stepper's value.
#[derive(Clone, Debug, PartialEq)]
struct Weighing {
    label: String,
    members: Vec<Member>,
    extras: f64,
    target: f64,
}

#[component]
pub(super) fn SizeDialog(ctx: StudyCtx, semester: SemesterKey, pid: u32) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_bench;
    let place = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let item = ready.study.semester(semester)?.items.iter().find(|item| matches!(item.subject, Subject::Row { pid: Some(of), .. } if of == pid))?;
            let Subject::Row { caption, ord, credits, .. } = &item.subject else { return None };
            let (least, most) = credits.as_deref().and_then(study::credits_range).unwrap_or((0.0, 0.0));
            Some(Place { name: item.name.clone(), tone: ready.tone(item.area), least, most, asks: item.credits.unwrap_or(least), held: item.filled, caption: caption.clone(), ord: *ord })
        })
        .flatten()
    });
    let Some(first) = place.get_untracked() else { return ().into_any() };
    let value = RwSignal::new(first.asks.max(1.0));
    let set = move |to: f64| value.set(to.round().clamp(1.0, MOST));
    let row = StoredValue::new((first.caption.clone(), first.ord));
    let name = StoredValue::new(first.name.clone());
    let weighing = Memo::new(move |_| row.with_value(|(caption, ord)| weighing_of(ctx, semester, pid, caption, *ord, value.get())));
    let ranged = first.most > first.least + 0.01;
    let plan_text = if ranged { format!("{}–{}", n(first.least, t), n(first.most, t)) } else { n(first.least, t) };
    // What the row says, set again: none for a row of one figure, its least for a range.
    let own = first.least.max(1.0);
    let warn = Memo::new(move |_| {
        let at = value.get();
        if first.least > 0.0 && at + 0.01 < first.least {
            Some((b.size_below)(&n(first.least, t)))
        } else if first.most > 0.0 && at > first.most + 0.01 {
            Some((b.size_above)(&n(first.most, t)))
        } else {
            None
        }
    });
    let apply = move |_| {
        let to = value.get_untracked();
        if (to - first.asks).abs() > 0.01 {
            // What the row says is no setting of its own.
            let size = ((to - own).abs() > 0.01).then_some(to);
            let label = semester.label(t.locale);
            let note = match size {
                Some(size) => (b.size_note)(&name.get_value(), &n(size, t), &label),
                None => (b.size_note_plan)(&name.get_value(), &label),
            };
            ctx.change_noted(Some(note), move |doc, _| {
                doc.set_size(pid, size);
            }, || {});
        }
        ctx.dialog.set(None);
    };
    let sub = format!("{} · {}", first.name, semester.label(t.locale));
    let tone = first.tone;

    view! {
        <DialogHead ctx title=b.size_title sub tone/>
        <div class="st-dlg-body st-sz">
            <p class="st-cf-lead">{(b.size_lead)(&semester.label(t.locale))}</p>
            <div class="st-sz-step">
                <button class="icon-btn hit" type="button" id="st-sz-down" aria-label=b.size_down title=b.size_down on:click=move |_| set(value.get_untracked() - 1.0)>
                    <Icon name="minus"/>
                </button>
                <label class="st-sz-num">
                    <input
                        type="number"
                        id="st-sz-value"
                        min="1"
                        max="99"
                        step="1"
                        inputmode="numeric"
                        aria-label=b.size_field
                        prop:value=move || n(value.get(), t)
                        on:change=move |ev| {
                            if let Ok(to) = event_target_value(&ev).replace(',', ".").trim().parse::<f64>() {
                                set(to);
                            }
                        }
                    />
                    <span>{t.common.credits_unit}</span>
                </label>
                <button class="icon-btn hit" type="button" id="st-sz-up" aria-label=b.size_up title=b.size_up on:click=move |_| set(value.get_untracked() + 1.0)>
                    <Icon name="plus"/>
                </button>
                <span class="st-sz-plan">
                    <small>{(b.size_plan)(&plan_text)}</small>
                    {(first.held > 0.0).then(|| view! { <small>{(b.size_held)(&n(first.held, t))}</small> })}
                </span>
                <button class="st-link st-sz-reset" type="button" hidden=move || (value.get() - own).abs() < 0.01 on:click=move |_| set(own)>{b.size_reset}</button>
            </div>
            <p class="st-sz-warn" aria-live="polite">{move || warn.get()}</p>
            {move || weighing.get().map(|weighing| weights_view(weighing, tone, t))}
        </div>
        <footer class="st-dlg-foot">
            <button class="btn secondary" type="button" on:click=move |_| ctx.dialog.set(None)>{t.study.cancel}</button>
            <button class="btn primary" type="button" id="st-sz-go" on:click=apply>{b.size_apply}</button>
        </footer>
    }
    .into_any()
}

/// The group of the row `caption`/`ord` with placeholder `pid` asking `value`: none where the row
/// is part of no group the regulation sums over.
fn weighing_of(ctx: StudyCtx, s: SemesterKey, pid: u32, caption: &str, ord: i64, value: f64) -> Option<Weighing> {
    let model = super::bench::model_of(ctx, s)?;
    let view = &model.view;
    let choice = view.lines.iter().find(|line| line.caption == caption && line.ord == ord)?.choice?;
    let group = view.group_of(choice)?;
    let (weights, extras) = ctx.with_ready(|ready| group.weights(view, &ready.study, Some((pid, value))))?;
    let members = weights
        .into_iter()
        .map(|weight| Member { name: view.choices.get(weight.choice).map(|choice| model::short_name(&choice.name)).unwrap_or_default(), own: weight.choice == choice, weight })
        .collect();
    Some(Weighing { label: group.label.clone(), members, extras, target: group.target(&view.choices) })
}

/// The weighting: each member's bar — what its modules hold, what its placeholders keep, its least
/// marked — and the sum against what the group asks.
fn weights_view(weighing: Weighing, tone: &'static str, t: &'static Texts) -> impl IntoView {
    let b = &t.study_bench;
    let total: f64 = weighing.members.iter().map(|member| member.weight.total()).sum::<f64>() + weighing.extras;
    let state = if total + 0.01 < weighing.target {
        ("open", (b.weights_open)(&n(weighing.target - total, t)))
    } else if total > weighing.target + 0.01 {
        ("over", (b.weights_over)(&n(total - weighing.target, t)))
    } else {
        ("done", b.weights_done.to_string())
    };
    view! {
        <section class="st-sz-w" style=format!("--c: {tone}")>
            <h3>{(b.weights_title)(&weighing.label)}</h3>
            <p class="st-cf-lead">{b.weights_lead}</p>
            <ul class="st-sz-ms">
                {weighing.members.into_iter().map(|member| {
                    let stand = member.weight.stand;
                    let (least, most) = (stand.least, stand.most.max(stand.least));
                    let total = member.weight.total();
                    let whole = most.max(total).max(0.01);
                    let at = move |v: f64| format!("{:.2}%", (v / whole * 100.0).clamp(0.0, 100.0));
                    let asked = if most > least + 0.01 { format!("{}–{}", n(least, t), n(most, t)) } else { n(least, t) };
                    let short = least - total;
                    view! {
                        <li class="st-sz-m" class:is-own=member.own>
                            <span class="st-sz-mh"><b>{member.name}</b><span class="num">{(b.weights_figure)(&n(total, t), &asked)}</span></span>
                            <span class="st-wb-fill" aria-hidden="true">
                                <i class="passed" style=format!("width: {}", at(stand.passed))></i>
                                <i class="planned" style=format!("width: {}", at(stand.planned))></i>
                                <i class="kept" style=format!("width: {}", at(member.weight.reserved))></i>
                                {(most > least + 0.01).then(|| view! { <b class="need" style=format!("left: {}", at(least))></b> })}
                            </span>
                            {(short > 0.01).then(|| view! { <small class="st-sz-short">{(b.weights_short)(&n(short, t))}</small> })}
                        </li>
                    }
                }).collect_view()}
            </ul>
            {(weighing.extras > 0.01).then(|| view! { <p class="st-sz-extra">{(b.weights_extras)(&n(weighing.extras, t))}</p> })}
            <p class=format!("st-sz-sum {}", state.0)>
                <b>{(b.weights_sum)(&n(total, t), &n(weighing.target, t))}</b>
                <span>{state.1}</span>
            </p>
            <p class="st-sz-legend" aria-hidden="true">
                <span><i class="planned"></i>{b.weights_modules}</span>
                <span><i class="kept"></i>{b.weights_kept}</span>
            </p>
        </section>
    }
}
