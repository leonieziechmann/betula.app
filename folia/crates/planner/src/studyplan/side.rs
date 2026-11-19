//! The sidebar of the Stundenplan, titled as the page is, „Stundenplan" (as „Mein Studium" titles
//! its own; owner's redesign of 2026-09-25), top to bottom: „Kalender" (`calendars.rs`: „Mein
//! Studiengang" and the calendars of one's own, in both states; while the plan is edited „+
//! Kalender" in its head and each calendar's „…"), „Export" (`export.rs`: the calendar file, the
//! subscription and the week as a picture), „Teilen …" (`ShareGroup`), the storage hint (`mod.rs`'s,
//! the same on the server), and while editing the plans saved before (`SavedPlans`), last, so that
//! nothing above them moves. „Plan leeren" went on 2026-11-17: the calendar no longer manages the
//! study's modules (owner: „man kann gerade noch module im Kalender verwalten, das ist aber keine
//! gute idee, weil man das ja in mein Studium macht"). A phone has no sidebar: its calendars and
//! „Export" with „Teilen …" are sheets its head opens (`calendars::CalendarSheet`, `ExportSheet`),
//! and the saved plans stand after the page.
//!
//! What the sidebar had besides went on 2026-11-04 (owner: „Komplett entfernen, weil überholt das
//! system" and „Später an einen anderen ort verlegen (jetzt einfach entfernen ich kümmer mich
//! später)"): „Studiengang" (the program is „Mein Studium"'s now), „Plan speichern" (the
//! calendars of one's own are there for several plans; the plans saved before stay to be loaded
//! and deleted), „Ansicht", „Zeigen" and „Standort"; and „Importieren" the same day („Das ist ja
//! auch in mein studium": a semester takes over its Fachsemester there, „Plan übernehmen").
//!
//! The groups stay where they are, whatever is planned and in both states (owner, 2026-09-25: „Da
//! sollte sich das Layout nicht viel shiften"): what the mode adds stands in places that are there
//! while looking, or after the last group.
//!
//! Every group is the app's alone (the server's sidebar is the storage hint). What changes the plan
//! is written after the next frame, once the control has answered (`Studyplan::update_after_paint`).
//! Each control reads a memo of its own (R5), and no closure reads a memo together with the one it
//! is derived from (R16).

use leptos::prelude::*;

use folia_design::format;
use crate::i18n;
use folia_stores::myprogram::MyProgram;
use folia_design::nav;
use folia_stores::studyplan::Saved;
use folia_plans::studyplan::PlanUndo;
use folia_design::waitnote::Mark;
use folia_design::modal::{Modal, ModalHead, ModalSize, Sheet};
use folia_design::ui::Icon;
use super::calendars::CalendarsGroup;
use super::export::CalendarGroup;
use super::note::PlanNote;
use super::share_dialog::{ShareOpen, ShareTab};
use super::{key_of, PlanCtx};

/// The id of „Teilen …".
const SHARE_ID: &str = "sp-share";

/// The id of „Ersetzen" when loading a saved plan would replace an unsaved one.
const LOAD_ID: &str = "sp-load-yes";

/// The question of „Ersetzen" with its answers, which a sheet of a phone brings into sight as it
/// unfolds (`nav::show_in_sheet`).
const LOAD_ASK_ID: &str = "sp-load-ask";

/// The sidebar's groups on a page wider than a phone; nothing on a phone, whose page has them in
/// their places (`mod.rs`: twice in one page would be `#sp-share` and `#sp-abo` twice).
#[component]
pub(super) fn PlanSidebar(ctx: PlanCtx) -> impl IntoView {
    move || {
        (!ctx.phone.get()).then(|| {
            view! {
                // The calendars beside the week first (`calendars.rs`): „Mein Studiengang" and
                // those of one's own.
                <CalendarsGroup ctx/>
                <CalendarGroup ctx/>
                <ShareGroup ctx/>
            }
        })
    }
}

/// „Teilen …": the dialog, with persons or into a calendar app (`share_dialog.rs`), in both states.
#[component]
pub(super) fn ShareGroup(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    view! {
        <div class="fgroup actions sp-share-group" data-section="" data-find="" role="group" aria-label=t.studyplan_keys.share>
            <button class="action" type="button" id=SHARE_ID data-command=t.studyplan_share_dialog.open data-command-group=t.studyplan.title on:click=move |_| ctx.sharing.set(Some(ShareOpen { calendar: 0, tab: ShareTab::People }))>
                <Icon name="share-2"/>
                <span><span>{t.studyplan_share_dialog.open}</span><small>{t.studyplan_share_dialog.open_hint}</small></span>
            </button>
        </div>
    }
}

/// „Export" on a phone: a sheet the head's „Export" opens (owner, 2026-11-18: „Ja und Export sollte am
/// handy wahrscheinlich auch ein modal werden oder so"), with what the sidebar's groups hold —
/// „.ics herunterladen", „Als Bild speichern", „Abonnieren" with what it unfolds — and „Teilen …".
/// Until then they stood in a section under the Termine. „Abonnieren" and „Teilen …" open the dialog
/// „Teilen", which takes the sheet's place: one sheet at a time. Closing that dialog — its ×, Esc,
/// Back as Android has it — brings the sheet back with the keyboard on the row that opened it; the
/// sheet's own way out then leaves it. Opened from a wide screen's sidebar the dialog closes as ever.
#[component]
pub(super) fn ExportSheet(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let open = Signal::derive(move || ctx.sheet.get() == Some(super::head::PhoneSheet::Export));
    let close = Callback::new(move |()| {
        if open.get_untracked() {
            ctx.sheet.set(None);
        }
    });
    // The id of the row that opened „Teilen" from the sheet, while the dialog stands in its place.
    let from = StoredValue::new(None::<&'static str>);
    Effect::new(move |_| match ctx.sharing.get() {
        Some(asked) if open.get_untracked() => {
            from.set_value(Some(if asked.tab == ShareTab::App { super::export::ABO_ID } else { SHARE_ID }));
            ctx.sheet.set(None);
        }
        None => {
            let Some(row) = from.get_value() else { return };
            from.set_value(None);
            if !ctx.phone.get_untracked() {
                return;
            }
            ctx.sheet.set(Some(super::head::PhoneSheet::Export));
            // After the dialog has put the keyboard on its first button.
            nav::after_paint(move || request_animation_frame(move || nav::focus_by_id(row)));
        }
        Some(_) => {}
    });
    let sub = move || (t.studyplan_head.export_sub)(&ctx.key.get().label(t.locale));
    view! {
        <Modal open on_close=close size=ModalSize::M sheet=Sheet::Auto class="sp-export-sheet" labelled_by="sp-export-sheet-title">
            <ModalHead id="sp-export-sheet-title" title=t.studyplan_export.calendar on_close=close>
                <p>{sub}</p>
            </ModalHead>
            <div class="st-dlg-body sp-export">
                <CalendarGroup ctx/>
                <ShareGroup ctx/>
            </div>
            // What is done in the sheet (a new address copied) is said at its foot (`note.rs`).
            <PlanNote ctx shown=open/>
        </Modal>
    }
}

// ---------- the plans saved before ----------

/// The plans saved before, while the plan is edited and where there are any, last in the sidebar (on
/// a phone after the page): a click loads one into the semester shown, asking first where that would
/// lose a timetable no saved plan holds; the bin deletes one. The saved plan the timetable holds is
/// marked. „Plan speichern" went on 2026-11-04 (owner: „das sind jetzt hat man ja mehrere
/// kallender"); loading still replaces the semester of the study (`put_semester`), kept as it was.
#[component]
pub(super) fn SavedPlans(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.studyplan_side;
    let saved = Saved::open();
    // Whether the semester shown holds a timetable, and the saved plan it is.
    let held = Memo::new(move |_| {
        let (url, current) = (ctx.url.get(), ctx.current.get());
        let town = ctx.mine.map(MyProgram::town).unwrap_or_default();
        let Some(plan) = ctx.plan else { return (false, None) };
        plan.with(|doc| {
            let key = key_of(&url, current, doc, ctx.today);
            let any = !doc.week_in(key).is_empty() || !doc.placeholders_in(key).is_empty();
            let marked = saved.with(|saved| saved.plans.iter().find(|p| p.town == town && doc.holds_saved(key, &p.doc)).map(|p| p.name.clone()));
            (any, marked)
        })
    });
    let any = Memo::new(move |_| held.with(|held| held.0));
    let marked = Memo::new(move |_| held.with(|held| held.1.clone()));
    let entries = Memo::new(move |_| saved.with(|saved| saved.plans.iter().map(|p| (p.name.clone(), p.modules())).collect::<Vec<_>>()));
    let shown = Memo::new(move |_| ctx.edit.get() && entries.with(|entries| !entries.is_empty()));

    // Loading: at once where nothing unsaved is lost, else after „Ersetzen". The row is marked in
    // the next frame (`said`); the plan follows after it (R21).
    let asking = RwSignal::new(None::<String>);
    let said = RwSignal::new(None::<String>);
    let mark = Memo::new(move |_| said.get().or_else(|| marked.get()));
    // A load replaces the semester's modules and Termine of the study: it answers with the page's
    // note and „Rückgängig" (as every change of the page that takes something away), which puts back
    // the semester and the Standort as they were — „Ersetzen" had none, and what it replaced was
    // gone; a note that lasts (`note.rs`).
    let load_now = move |which: String| {
        let Some(chosen) = saved.with_untracked(|saved| saved.get(&which).cloned()) else { return };
        asking.set(None);
        said.set(Some(which.clone()));
        ctx.undo.set(None);
        let (key, plan, mine) = (ctx.key.get_untracked(), ctx.plan, ctx.mine);
        let town = mine.map(|mine| untrack(|| mine.town()));
        nav::after_paint(move || {
            if let Some(mine) = mine {
                mine.set_town(chosen.town);
            }
            if let Some(plan) = plan {
                let done = plan.update(|doc| {
                    let before = doc.clone();
                    doc.put_semester(key, &chosen.doc);
                    PlanUndo::of(before, doc.clone())
                });
                ctx.say_undoable("check", Mark::Done, (s.loaded)(&which), move || {
                    if let (Some(mine), Some(town)) = (mine, town) {
                        mine.set_town(town);
                    }
                    let done = done.clone();
                    plan.update_after_paint(move |doc| done.restore(doc));
                });
            }
            let _ = said.try_set(None);
        });
    };
    let load = move |which: String| {
        if any.get_untracked() && marked.with_untracked(Option::is_none) {
            asking.set(Some(which));
            request_animation_frame(|| {
                nav::focus_by_id(LOAD_ID);
                nav::show_in_sheet(LOAD_ASK_ID);
            });
        } else {
            load_now(which);
        }
    };
    let delete = move |which: String| {
        if asking.with_untracked(|asking| asking.as_deref() == Some(which.as_str())) {
            asking.set(None);
        }
        let gone = saved.update(|saved| {
            let at = saved.plans.iter().position(|plan| plan.name == which)?;
            let plan = saved.plans.get(at).cloned()?;
            saved.remove(&which);
            Some((at, plan))
        });
        // The page's note says so with „Rückgängig", which puts it back at its place (`note.rs`; a
        // note that lasts: until 2026-11-19 a line under the saved plans).
        let Some((at, plan)) = gone else { return };
        let name = plan.name.clone();
        ctx.say_undoable("trash-2", Mark::Out, (t.studyplan_calendars.removed)(&name), move || {
            let plan = plan.clone();
            saved.update(|saved| {
                if saved.get(&plan.name).is_none() {
                    let at = at.min(saved.plans.len());
                    saved.plans.insert(at, plan);
                }
            });
        });
    };

    move || {
        shown.get().then(|| {
            view! {
                <div class="fgroup actions sp-plans" data-section="" data-find="" role="group" aria-label=t.studyplan_edit.saved_plans>
                    <p class="flabel label">{t.studyplan_edit.saved_plans}</p>
                    <For
                        each=move || entries.get()
                        key=|entry| entry.clone()
                        children=move |(which, count): (String, usize)| {
                            let here = {
                                let which = which.clone();
                                Memo::new(move |_| mark.with(|mark| mark.as_deref() == Some(which.as_str())))
                            };
                            let asked = {
                                let which = which.clone();
                                Memo::new(move |_| asking.with(|asking| asking.as_deref() == Some(which.as_str())))
                            };
                            let (one, two, three) = (which.clone(), which.clone(), which.clone());
                            view! {
                                <div class="sp-saved">
                                    <button class="action" type="button" aria-current=move || here.get().then_some("true") on:click=move |_| load(one.clone())>
                                        <span>{which.clone()}</span>
                                        <small class="num">{format::modules(i64::try_from(count).unwrap_or(i64::MAX), t.locale)}</small>
                                    </button>
                                    // It is deleted, not hidden: the bin (owner, 2026-11-17: „da wo es
                                    // entfernt eine Mülltonne").
                                    <button class="icon-btn" type="button" aria-label=(s.delete_named)(&which) title=s.delete on:click=move |_| delete(two.clone())>
                                        <Icon name="trash-2"/>
                                    </button>
                                </div>
                                {move || {
                                    let three = three.clone();
                                    asked.get().then(|| view! {
                                        <p class="action note-action ask" id=LOAD_ASK_ID>
                                            <span>{s.replace_current}</span>
                                            <button class="mini danger hit" type="button" id=LOAD_ID on:click=move |_| load_now(three.clone())>{s.replace}</button>
                                            <button class="mini hit" type="button" on:click=move |_| asking.set(None)>{s.cancel}</button>
                                        </p>
                                    })
                                }}
                            }
                        }
                    />
                </div>
            }
        })
    }
}
