//! A module's menu in „Mein Studium" (owner, 2026-10-04: adding and removing was „eine absolute
//! Krise"): everything one item of a semester can do, in one place — tick it off, move it to
//! another semester (each says whether it offers the module), open the module, take it out (which
//! „Rückgängig" takes back) — and what Betula knows of it: the turnus where the semester does not
//! offer it, what it asks for of other modules and where those stand, its exam and language.

use folia_calendar::semester::SemesterKey;
use folia_plans::study::{self, Subject, When};
use leptos::prelude::*;

use folia_design::format;
use folia_design::ui::Icon;

use super::dialog::DialogHead;
use super::focus::{find_href, move_to};
use super::picker::needs_text;
use super::{item_of, module_href, n, StudyCtx};
use crate::i18n::{self, Texts};

/// What the menu shows of an item.
#[derive(Clone, Debug, PartialEq)]
struct Info {
    item: study::Item,
    sub: String,
    tone: &'static str,
    unoffered: Option<String>,
    needs: Option<(String, bool)>,
    exam: Option<String>,
    language: Option<&'static str>,
    /// The semesters it can move to: the semester, its label, whether it offers it.
    targets: Vec<(SemesterKey, String, bool)>,
    find: Option<String>,
}

impl Info {
    fn of(ctx: StudyCtx, semester: SemesterKey, key: &str, t: &Texts) -> Option<Self> {
        let s = &t.study;
        let doc = ctx.plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
        let needs = ctx.needs.get();
        let row = ctx.rows.with(|rows| key.strip_prefix("m:").and_then(|id| rows.iter().find(|row| row.id == id).cloned()));
        ctx.with_ready(|ready| {
            let item = item_of(ready, semester, key)?;
            let mut parts = Vec::new();
            if let Some(credits) = item.credits {
                parts.push((s.credits)(&n(credits, t)));
            }
            parts.push(ready.area_name(item.area, t));
            parts.push(if item.passed { (s.passed_in)(&semester.label(t.locale)) } else if item.failed { (s.failed_in)(&semester.label(t.locale)) } else { (s.planned_in)(&semester.label(t.locale)) });
            let turnus = row.as_ref().map(|row| format::turnus(row.turnus_season.as_ref(), row.turnus_parity.as_ref(), t.locale));
            let unoffered = (item.unoffered && !item.passed).then(|| (s.unoffered_note)(&turnus.clone().map(|turnus| (s.every)(&turnus)).unwrap_or_default(), &semester.label(t.locale)));
            let targets = ready
                .study
                .semesters
                .iter()
                .filter(|other| other.key != semester && (other.when != When::Past || item.passed || item.failed))
                .map(|other| (other.key, other.key.short(t.locale), item.offer.offered(other.key)))
                .collect();
            let find = match &item.subject {
                Subject::Row { caption, ord, .. } => find_href(ctx, caption, *ord, t),
                Subject::Module { .. } => None,
            };
            Some(Info {
                sub: parts.join(" · "),
                tone: ready.tone(item.area),
                unoffered,
                needs: item.module_id().and_then(|id| needs_text(&doc, &needs, id, &item.name, semester, ready.study.now, t)),
                exam: row.as_ref().and_then(|row| row.exam_form.as_ref()).map(|form| format::exam_short(form, t.locale)),
                language: row.as_ref().and_then(|row| format::languages(row.teaches_german, row.teaches_english)),
                targets,
                find,
                item,
            })
        })
        .flatten()
    }
}

#[component]
pub(super) fn ItemMenu(ctx: StudyCtx, semester: SemesterKey, key: String) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let info = {
        let key = key.clone();
        Memo::new(move |_| Info::of(ctx, semester, &key, t))
    };
    let toggle = move |_| {
        let Some(info) = info.get_untracked() else { return };
        let next = !info.item.passed;
        ctx.change(move |doc, _| {
            study::set_passed(doc, semester, &info.item, next);
        }, || {});
    };
    let go = {
        let key = key.clone();
        move |to: SemesterKey| {
            move_to(ctx, semester, to, &key, t);
            ctx.focus.set(Some(to));
            ctx.dialog.set(None);
        }
    };
    let remove = move |_| {
        let Some(info) = info.get_untracked() else { return };
        let note = (s.removed)(&info.item.name);
        ctx.change_noted(Some(note), move |doc, _| study::remove(doc, semester, &info.item), || {});
        ctx.dialog.set(None);
    };
    move || {
        let Some(info) = info.get() else { return ().into_any() };
        let passed = info.item.passed;
        let view_way = match info.item.module_id() {
            Some(id) => {
                let href = module_href(ctx, id, t);
                Some(view! { <a class="btn secondary" href=href data-noscroll="" on:click=move |_| ctx.dialog.set(None)>{s.view_module}<Icon name="arrow-up-right"/></a> }.into_any())
            }
            None => info.find.clone().map(|href| view! { <a class="btn secondary" href=t.path(&href) rel="nofollow"><Icon name="search"/>{s.choose_module}</a> }.into_any()),
        };
        let go = go.clone();
        view! {
            <DialogHead ctx title=info.item.name.clone() sub=info.sub.clone() tone=info.tone/>
            <div class="st-dlg-body st-menu">
                {info.unoffered.map(|text| view! { <p class="st-note warn"><Icon name="triangle-alert"/><span>{text}</span></p> })}
                {(info.needs.is_some() || info.exam.is_some() || info.language.is_some()).then(|| view! {
                    <dl class="st-facts">
                        {info.needs.map(|(text, warn)| view! { <dt>{s.needs_label}</dt><dd class:warn=warn>{text}</dd> })}
                        {info.exam.map(|exam| view! { <dt>{s.exam}</dt><dd>{exam}</dd> })}
                        {info.language.map(|language| view! { <dt>{s.language}</dt><dd>{language}</dd> })}
                    </dl>
                })}
                <button class="st-check big" type="button" role="checkbox" id="st-menu-passed" aria-checked=if passed { "true" } else { "false" } on:click=toggle>
                    <span class="st-box" class:with=passed aria-hidden="true"><Icon name="check"/></span>{s.mark_passed}
                </button>
                {(!info.targets.is_empty()).then(|| view! {
                    <div class="st-move">
                        <p class="label">{s.move_to}</p>
                        <div class="st-move-chips">
                            {info.targets.into_iter().map(|(to, label, offered)| {
                                let go = go.clone();
                                view! {
                                    <button type="button" class="st-move-chip" class:off=!offered on:click=move |_| go(to)>
                                        {label}
                                        {if offered { view! { <span class="ok">{s.offered}</span> }.into_any() } else { view! { <Icon name="triangle-alert"/> }.into_any() }}
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    </div>
                })}
                <div class="st-menu-ways">
                    {view_way}
                    <button class="btn secondary danger" type="button" id="st-menu-remove" on:click=remove><Icon name="trash-2"/>{s.remove}</button>
                </div>
                <p class="st-sub">{s.remove_note}</p>
            </div>
        }
        .into_any()
    }
}
