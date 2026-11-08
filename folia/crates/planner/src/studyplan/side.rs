//! The sidebar of the Stundenplan, „Anpassen" (owner's redesign of 2026-09-25), top to bottom:
//! „Kalender" (`calendars.rs`: „Mein Studiengang" and the calendars of one's own), on a phone
//! „Plan · Alle Termine" (which a wide page has over the week), „Export" (`export.rs`: the calendar
//! file, the subscription and the week as a picture), and last „Plan": load or delete a saved
//! plan, hand it on by a link (`share.rs`), empty it. The storage hint under it is `mod.rs`'s, the same on the server. On a
//! phone the sidebar is a sheet, „Anpassen", and „Export" is not in it: it stands under the
//! Termine (`mod.rs`, owner 2026-09-27: among what is shown nobody looks for it).
//!
//! What the sidebar had besides went on 2026-11-04 (owner: „Komplett entfernen, weil überholt das
//! system" and „Später an einen anderen ort verlegen (jetzt einfach entfernen ich kümmer mich
//! später)"): „Studiengang" (the program is „Mein Studium"'s now), „Plan speichern" (the
//! calendars of one's own are there for several plans; the plans saved before stay to be loaded
//! and deleted), „Ansicht", „Zeigen" and „Standort"; and „Importieren" the same day („Das ist ja
//! auch in mein studium": a semester takes over its Fachsemester there, „Plan übernehmen").
//!
//! The groups stay where they are, whatever is planned (owner, 2026-09-25: „Da sollte sich das
//! Layout nicht viel shiften"): with nothing to show or to do a group says so or greys its control
//! out, and what is hidden is listed in the plan's quiet line, not here (`head::Overlaps`).
//!
//! Every group is the app's alone (the server's sidebar is the storage hint). What is shown of the
//! timetable changes at once, in the click (the timetable is worked out again in Rust, no query,
//! R21); what changes the planned modules is written after the next frame, once the control has
//! answered (`Studyplan::update_after_paint`). Each control reads a memo of its own (R5), and no
//! closure reads a memo together with the one it is derived from (R16).

use folia_routes::url::{self, StudyplanUrl};
use leptos::prelude::*;

use folia_design::format;
use crate::i18n;
use folia_stores::myprogram::MyProgram;
use folia_design::nav;
use folia_shell::pending::Pending;
use folia_stores::studyplan::Saved;
use folia_design::ui::Icon;
use super::calendars::CalendarsGroup;
use super::export::CalendarGroup;
use super::share_dialog::{ShareOpen, ShareTab};
use super::week::AllSwitch;
use super::{key_of, PlanCtx};

/// The id of „Plan leeren", where the focus returns from „Abbrechen" and „Rückgängig".
const CLEAR_ID: &str = "sp-clear";

/// The id of „Ersetzen" when loading a saved plan would replace an unsaved one.
const LOAD_ID: &str = "sp-load-yes";

#[component]
pub(super) fn PlanSidebar(ctx: PlanCtx) -> impl IntoView {
    view! {
        // The calendars beside the week first (`calendars.rs`): „Mein Studiengang" and those of one's
        // own, and „Neuer Kalender"; on a phone in „Anpassen" too.
        <CalendarsGroup ctx/>
        // On a phone „Plan · Alle Termine", which a wide page has in the head over the week.
        {move || ctx.phone.get().then(|| view! { <SessionsGroup ctx/> })}
        // On a phone the sidebar is the sheet „Anpassen", and „Export" stands under the Termine
        // instead (`SemesterView`).
        {move || (!ctx.phone.get()).then(|| view! { <CalendarGroup ctx/> })}
        <PlanGroup ctx/>
    }
}

/// The address the page is going to (`pending`), else the one it is at: what the sidebar's
/// switches follow, so a click shows in the next frame (R21).
fn shown_url(ctx: PlanCtx) -> Memo<StudyplanUrl> {
    let going = Pending::expect();
    Memo::new(move |_| match going.and_then(|going| going.search_on(url::STUDYPLAN)) {
        Some(search) => StudyplanUrl::parse(&search),
        None => ctx.url.get(),
    })
}

// ---------- on a phone: Plan · Alle Termine ----------

/// On a phone „Plan · Alle Termine", which a wide page has in the head over the week (owner,
/// 2026-10-26: „Die können in das Anpassen Feld verschoben werden. Das ist keine Aktion, die man
/// häufig macht"): greyed out where it would show nothing, in another view or with nothing planned.
#[component]
fn SessionsGroup(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let shown = shown_url(ctx);
    let weekly = Memo::new(move |_| shown.with(|shown| shown.view.is_weekly()));
    let planned = Memo::new(move |_| ctx.wanted.with(|wanted| !wanted.1.is_empty()));
    let off = Signal::derive(move || !(weekly.get() && planned.get()));
    view! {
        <div class="fgroup">
            <p class="flabel label">{t.studyplan_week.sessions_switch}</p>
            <AllSwitch all=ctx.all wide=true off/>
        </div>
    }
}

// ---------- Plan ----------

/// „Plan": the plans saved before (a click loads one into the semester shown, asking first where
/// that would lose a timetable no saved plan holds; × deletes one; „Plan speichern" went on
/// 2026-11-04, owner: „das sind jetzt hat man ja mehrere kallender"), „Link zum Teilen kopieren"
/// (`share`), and „Plan leeren" („Wirklich leeren?": the semester shown), which „Rückgängig" takes
/// back. The saved plan the timetable holds is marked.
#[component]
fn PlanGroup(ctx: PlanCtx) -> impl IntoView {
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
    // „Plan leeren" empties the semester shown: with nothing in it there is nothing to empty.
    let empty = Memo::new(move |_| !any.get());
    let marked = Memo::new(move |_| held.with(|held| held.1.clone()));
    let entries = Memo::new(move |_| saved.with(|saved| saved.plans.iter().map(|p| (p.name.clone(), p.modules())).collect::<Vec<_>>()));
    let cleared = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().is_some_and(|(note, _)| note == s.cleared)));

    // Loading: at once where nothing unsaved is lost, else after „Ersetzen". The row is marked in
    // the next frame (`said`); the plan follows after it (R21).
    let asking = RwSignal::new(None::<String>);
    let said = RwSignal::new(None::<String>);
    let mark = Memo::new(move |_| said.get().or_else(|| marked.get()));
    let load_now = move |which: String| {
        let Some(chosen) = saved.with_untracked(|saved| saved.get(&which).cloned()) else { return };
        asking.set(None);
        said.set(Some(which));
        ctx.undo.set(None);
        let (key, plan, mine) = (ctx.key.get_untracked(), ctx.plan, ctx.mine);
        nav::after_paint(move || {
            if let Some(mine) = mine {
                mine.set_town(chosen.town);
            }
            if let Some(plan) = plan {
                plan.update(|doc| doc.put_semester(key, &chosen.doc));
            }
            let _ = said.try_set(None);
        });
    };
    let load = move |which: String| {
        if any.get_untracked() && marked.with_untracked(Option::is_none) {
            asking.set(Some(which));
            request_animation_frame(|| nav::focus_by_id(LOAD_ID));
        } else {
            load_now(which);
        }
    };
    let delete = move |which: String| {
        if asking.with_untracked(|asking| asking.as_deref() == Some(which.as_str())) {
            asking.set(None);
        }
        saved.update(|saved| saved.remove(&which));
    };

    // „Plan leeren": the timetable of the semester shown, emptied after the next frame (the views
    // and the calendar let go of it then); the note answers at once and keeps what was there. The
    // program stays the timetable's, and the other semesters and what was passed stay „Mein
    // Studium"'s (`PlanDoc::clear_semester`).
    let confirming = RwSignal::new(false);
    let restoring = RwSignal::new(false);
    let ask = move |_| {
        confirming.set(true);
        request_animation_frame(|| nav::focus_by_id("sp-clear-yes"));
    };
    let clear = move |_| {
        confirming.set(false);
        let Some(plan) = ctx.plan else { return };
        ctx.undo.set(Some((s.cleared.to_string(), plan.with_untracked(Clone::clone))));
        let key = ctx.key.get_untracked();
        plan.update_after_paint(move |doc| doc.clear_semester(key));
        request_animation_frame(|| nav::focus_by_id("sp-clear-undo"));
    };
    let undo = move |_| {
        if restoring.get_untracked() {
            return;
        }
        let (Some(plan), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        restoring.set(true);
        let note = ctx.undo;
        plan.update_after_paint(move |doc| {
            *doc = before;
            note.set(None);
            restoring.set(false);
            // „Rückgängig" makes way for „Plan leeren", which takes the focus it had.
            request_animation_frame(|| nav::focus_by_id(CLEAR_ID));
        });
    };
    let cancel = move |_| {
        confirming.set(false);
        request_animation_frame(|| nav::focus_by_id(CLEAR_ID));
    };

    // The group stays whatever is planned (owner, 2026-09-25: the sidebar holds still): with
    // nothing to empty, „Plan leeren" is greyed out.
    view! {
        <div class="fgroup actions">
            <p class="flabel label">{s.plan}</p>
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
                            <button class="icon-btn" type="button" aria-label=(s.delete_named)(&which) title=s.delete on:click=move |_| delete(two.clone())>
                                <Icon name="x"/>
                            </button>
                        </div>
                        {move || {
                            let three = three.clone();
                            asked.get().then(|| view! {
                                <p class="action note-action ask">
                                    <span>{s.replace_current}</span>
                                    <button class="mini danger hit" type="button" id=LOAD_ID on:click=move |_| load_now(three.clone())>{s.replace}</button>
                                    <button class="mini hit" type="button" on:click=move |_| asking.set(None)>{s.cancel}</button>
                                </p>
                            })
                        }}
                    }
                }
            />
            // „Teilen …": the dialog, with persons or into a calendar app (`share_dialog.rs`).
            <button class="action" type="button" id="sp-share" on:click=move |_| ctx.sharing.set(Some(ShareOpen { calendar: 0, tab: ShareTab::People }))>
                <Icon name="share-2"/>
                <span><span>{t.studyplan_share_dialog.open}</span><small>{t.studyplan_share_dialog.open_hint}</small></span>
            </button>
            {move || match (cleared.get(), confirming.get(), empty.get()) {
                (true, _, _) => view! {
                    <p class="action note-action">
                        <Icon name="check"/>
                        <span>{s.cleared}</span>
                        <button class="mini hit" type="button" id="sp-clear-undo" aria-busy=move || restoring.get().then_some("true") on:click=undo>{t.common.undo}</button>
                    </p>
                }
                .into_any(),
                (false, true, _) => view! {
                    <p class="action note-action ask">
                        <span>{s.really_clear}</span>
                        <button class="mini danger hit" type="button" id="sp-clear-yes" on:click=clear>{s.clear}</button>
                        <button class="mini hit" type="button" on:click=cancel>{s.cancel}</button>
                    </p>
                }
                .into_any(),
                (false, false, false) => view! {
                    <button class="action" type="button" id=CLEAR_ID on:click=ask><Icon name="trash-2"/><span>{s.clear_plan}</span></button>
                }
                .into_any(),
                (false, false, true) => view! {
                    <button class="action" type="button" aria-disabled="true" title=s.nothing_planned><Icon name="trash-2"/><span>{s.clear_plan}</span></button>
                }
                .into_any(),
            }}
        </div>
    }
}
