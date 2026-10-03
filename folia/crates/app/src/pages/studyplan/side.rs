//! The sidebar of the Stundenplan, „Anpassen" (owner's redesign of 2026-09-25), top to bottom:
//! „Studiengang" (the timetable's program, „Mein Studiengang" by default, and the way to its page),
//! „Importieren" (`import.rs`: a Fachsemester of its Regelstudienplan), then its view, what is
//! shown, the Standort and its calendar, and last „Plan": save the timetable under a name, load or
//! delete a saved one, hand it on by a link (`share.rs`), empty it. The storage hint under it is
//! `mod.rs`'s, the same on the server. On a phone the sidebar is a sheet, „Anpassen", and the
//! calendar is not in it: it stands under the Termine (`mod.rs`, owner 2026-09-27: among what is
//! shown nobody looks for it).
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

use folia_calendar::kind::{EventKind, KindSet};
use folia_calendar::select::{Town, TownChoice};
use folia_calendar::semester::SemesterKey;
use folia_model::rows::Program;
use folia_plans::studyplan::{MAX_SAVED_NAME, PlanDoc};
use folia_routes::url::{self, PlanView, ProgramTab, StudyplanUrl};
use folia_timetable::model::Timetable;
use folia_pages::ask::ProgramsAsk;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use crate::combobox::{ComboItem, Combobox};
use crate::format;
use crate::i18n;
use crate::myprogram::{po_of, program_name, MyProgram};
use crate::nav;
use crate::pending::Pending;
use crate::studyplan::{Saved, Studyplan};
use crate::ui::Icon;
use super::export::CalendarGroup;
use super::import::ImportGroup;
use super::share::ShareAction;
use super::{key_of, PlanCtx};

/// The id of „Plan leeren", where the focus returns from „Abbrechen" and „Rückgängig".
const CLEAR_ID: &str = "sp-clear";

/// The id of „Plan speichern", and of the name field it opens.
const SAVE_ID: &str = "sp-save";
const NAME_ID: &str = "sp-save-name";

/// The id of „Ersetzen" when loading a saved plan would replace an unsaved one.
const LOAD_ID: &str = "sp-load-yes";

#[component]
pub(super) fn PlanSidebar(ctx: PlanCtx) -> impl IntoView {
    let programs = Programs::new(ctx);
    // The Fachsemester last taken over in this visit, for the name „Plan speichern" suggests.
    let imported = RwSignal::new(None::<u8>);
    view! {
        <ProgramGroup ctx programs/>
        <ImportGroup ctx program=programs.shown imported/>
        <ViewGroup ctx/>
        <KindsGroup ctx/>
        <TownGroup ctx/>
        // On a phone the sidebar is the sheet „Anpassen", and the calendar stands under the Termine
        // instead (`SemesterView`).
        {move || (!ctx.phone.get()).then(|| view! { <CalendarGroup ctx/> })}
        <PlanGroup ctx program=programs.shown imported/>
    }
}

/// The address the page is going to (`pending`), else the one it is at: what the sidebar's
/// views mark, so a click marks its own link in the next frame (R21).
fn shown_url(ctx: PlanCtx) -> Memo<StudyplanUrl> {
    let going = Pending::expect();
    Memo::new(move |_| match going.and_then(|going| going.search_on(url::STUDYPLAN)) {
        Some(search) => StudyplanUrl::parse(&search),
        None => ctx.url.get(),
    })
}

// ---------- 1. Studiengang ----------

/// The programs of the snapshot, „Mein Studiengang"'s id, and the timetable's program.
#[derive(Clone, Copy)]
struct Programs {
    all: Memo<Vec<Program>>,
    mine: Memo<Option<String>>,
    shown: Memo<Option<Program>>,
}

impl Programs {
    fn new(ctx: PlanCtx) -> Self {
        let all = Memo::new(move |_| ctx.source.with_value(|source| source.as_ref().and_then(|source| source.now(&ProgramsAsk {}).ok())).unwrap_or_default());
        let asked = Memo::new(move |_| ctx.url.with(|url| url.import.clone()));
        let stored = Memo::new(move |_| ctx.plan.and_then(|plan| plan.with(|doc| doc.program.clone())));
        let mine = Memo::new(move |_| ctx.mine.and_then(|mine| mine.with(|doc| doc.program.clone())));
        let shown = Memo::new(move |_| {
            let (asked, stored, mine) = (asked.get(), stored.get(), mine.get());
            all.with(|all| shown_program(all, asked.as_deref(), stored.as_deref(), mine.as_deref()).cloned())
        });
        Programs { all, mine, shown }
    }
}

/// The timetable's program: the one the address names (`import=<slug>`, a program page's „In den
/// Stundenplan"; `import=mine`), else the one stored with the plan, else „Mein Studiengang"; each
/// only while the snapshot has it.
fn shown_program<'a>(all: &'a [Program], asked: Option<&str>, stored: Option<&str>, mine: Option<&str>) -> Option<&'a Program> {
    let by_id = |id: &str| all.iter().find(|program| program.id == id);
    let named = match asked {
        Some("mine") => mine.and_then(by_id),
        Some(slug) => all.iter().find(|program| program.slug == slug),
        None => None,
    };
    named.or_else(|| stored.and_then(by_id)).or_else(|| mine.and_then(by_id))
}

/// „Studiengang": the picker, „Mein Studiengang" first under its own heading, and the program's
/// page, where the whole study is planned. A pick is the timetable's program, stored with the plan;
/// it sets „Mein Studiengang" only while none is set.
#[component]
fn ProgramGroup(ctx: PlanCtx, programs: Programs) -> impl IntoView {
    let t = i18n::t();
    let Programs { all, mine, shown } = programs;
    let items = Signal::derive(move || {
        let mine = mine.get();
        all.with(|all| {
            let item = |program: &Program| ComboItem::new(program.id.clone(), program.name.clone(), format!("{} · PO {}", program.degree(), po_of(program)), i64::from(program.is_latest_po));
            let own = mine.as_deref().and_then(|id| all.iter().find(|program| program.id == id));
            own.map(|program| item(program).in_group(t.studyplan_side.mine)).into_iter().chain(all.iter().map(item)).collect::<Vec<_>>()
        })
    });
    let selected = Signal::derive(move || shown.with(|program| program.as_ref().map(|program| program.id.clone())));
    let pick = Callback::new(move |id: Option<String>| {
        let Some(id) = id else { return };
        let Some(program) = all.with_untracked(|all| all.iter().find(|program| program.id == id).cloned()) else { return };
        if let Some(plan) = ctx.plan {
            plan.update(|doc| doc.program = Some(program.id.clone()));
        }
        if let Some(mine) = ctx.mine.filter(|mine| untrack(|| mine.with(|doc| doc.program.is_none()))) {
            mine.set_program(&program.id, &program_name(&program), "", None);
        }
        // An address that named a program (`import=`) has done its part.
        let away = ctx.url.with_untracked(|url| url.import.is_some().then(|| url.without_import().path()));
        if let (Some(going), Some(away)) = (Pending::expect(), away) {
            going.go(&away, NavigateOptions { replace: true, ..Default::default() });
        }
    });
    // „Mein Plan" on the program's page, where the whole study is to be planned.
    let link = Memo::new(move |_| shown.with(|program| program.as_ref().map(|program| t.path(&url::program_path(&program.slug, ProgramTab::MyPlan)))));
    view! {
        <div class="fgroup first sp-program">
            <p class="flabel label">{t.studyplan_side.program}</p>
            <Combobox
                id="sp-program"
                label=t.studyplan_side.program
                placeholder=t.studyplan_side.choose_program
                search_placeholder=t.studyplan_side.search_program
                icon="graduation-cap"
                min_width=480.0
                items
                selected
                on_select=pick
                clearable=false
            />
            {move || link.get().map(|href| view! { <a class="sp-more" href=href>{t.studyplan_side.plan_studies}</a> })}
        </div>
    }
}

// ---------- 2. Ansicht ----------

/// „Ansicht": Woche · Termine · Prüfungen, the module beside the plan staying.
#[component]
fn ViewGroup(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let shown = shown_url(ctx);
    let link = move |view: PlanView| {
        let checked = Memo::new(move |_| shown.with(|shown| shown.view == view));
        view! {
            <a
                href=move || t.path(&ctx.url.with(|url| url.with_view(view).path()))
                role="radio"
                draggable="false"
                aria-checked=move || if checked.get() { "true" } else { "false" }
            >
                <span class="seg-label">{view.label(t.locale)}</span>
            </a>
        }
    };
    view! {
        <div class="fgroup">
            <p class="flabel label">{t.studyplan_side.view}</p>
            <div class="seg" role="radiogroup" aria-label=t.studyplan_side.view>
                {[PlanView::Week, PlanView::Dates, PlanView::Exams].into_iter().map(link).collect_view()}
            </div>
        </div>
    }
}

// ---------- 3. Zeigen ----------

/// Under the chips, where an event counts for two kinds: „„Vorlesung/Übung“ zählt als beides."
fn combined_hint(table: &Timetable, t: &i18n::Texts) -> Option<String> {
    let kinds = table.events.iter().map(|event| event.kinds.known()).find(|kinds| kinds.iter().count() >= 2)?;
    let labels: Vec<&str> = kinds.iter().map(|kind| kind.label(t.locale)).collect();
    Some((t.studyplan_side.combined)(&labels.join("/"), labels.len()))
}

/// „Zeigen": a chip per kind of the semester with its number of events; a click hides or shows
/// the kind (an event goes only when all its kinds are hidden). Without any Termin a line says so.
#[component]
fn KindsGroup(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let kinds = Memo::new(move |_| ctx.table.with(|table| table.as_ref().map(Timetable::kinds_present).unwrap_or_default()));
    let hidden = Memo::new(move |_| ctx.selection.with(|(_, selection)| selection.hidden_kinds));
    let combined = Memo::new(move |_| ctx.table.with(|table| table.as_ref().and_then(|table| combined_hint(table, t))));
    let any = Memo::new(move |_| kinds.with(|kinds| !kinds.is_empty()));
    view! {
        <div class="fgroup">
            <p class="flabel label">{t.studyplan_side.show}</p>
            {move || match any.get() {
                false => view! { <p class="hint sp-none">{t.studyplan_side.no_dates}</p> }.into_any(),
                true => view! {
                    <div class="chips">
                        <For
                            each=move || kinds.get()
                            key=|entry| *entry
                            children=move |(kind, count): (EventKind, usize)| {
                                let off = Memo::new(move |_| hidden.with(|hidden: &KindSet| hidden.contains(kind)));
                                let toggle = move |_| {
                                    let (key, hide) = (ctx.key.get_untracked(), !off.get_untracked());
                                    if let Some(plan) = ctx.plan {
                                        plan.update(|doc| doc.set_kind(key, kind, hide));
                                    }
                                };
                                view! {
                                    <button
                                        class="chip"
                                        type="button"
                                        aria-pressed=move || if off.get() { "false" } else { "true" }
                                        data-state=move || if off.get() { "without" } else { "off" }
                                        title=move || if off.get() { t.studyplan_side.unhide } else { t.studyplan_side.hide }
                                        on:click=toggle
                                    >
                                        <span class="chip-label">{kind.label(t.locale)}</span>
                                        <span class="chip-count num">{count}</span>
                                    </button>
                                }
                            }
                        />
                    </div>
                    {move || combined.get().map(|text| view! { <p class="hint">{text}</p> })}
                }
                .into_any(),
            }}
        </div>
    }
}

// ---------- 4. Standort ----------

/// „Standort", for modules taught in both towns: Cottbus · Senftenberg · Beide. With nothing
/// stored the derived town is checked (the line „Abgeleitet: …" says so); a click stores the
/// choice in „Mein Studiengang". It stays where it is while no planned module is taught in both
/// towns (the choice then waits for one).
#[component]
fn TownGroup(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let derived = Memo::new(move |_| ctx.table.with(|table| table.as_ref().filter(|table| table.town_derived).and_then(|table| table.town)));
    let choice = Memo::new(move |_| ctx.selection.with(|(_, selection)| selection.town));
    let checked = Memo::new(move |_| match choice.get() {
        TownChoice::Derive => derived.get().map(TownChoice::Only),
        stored => Some(stored),
    });
    let button = move |label: &'static str, town: TownChoice| {
        let on = Memo::new(move |_| checked.get() == Some(town));
        view! {
            <button
                type="button"
                role="radio"
                aria-checked=move || if on.get() { "true" } else { "false" }
                on:click=move |_| {
                    if let Some(mine) = ctx.mine {
                        mine.set_town(town);
                    }
                }
            >
                <span class="seg-label">{label}</span>
            </button>
        }
    };
    view! {
        <div class="fgroup">
            <p class="flabel label">{t.studyplan_side.town}</p>
            <div class="seg hug" role="radiogroup" aria-label=t.studyplan_side.town>
                {button(Town::Cottbus.label(), TownChoice::Only(Town::Cottbus))}
                {button(Town::Senftenberg.label(), TownChoice::Only(Town::Senftenberg))}
                {button(t.studyplan_side.both_towns, TownChoice::Both)}
            </div>
        </div>
    }
}

// ---------- 5. Plan ----------

/// The name „Plan speichern" suggests: the saved plan the timetable holds (saving again replaces
/// it), else the program with the Fachsemester last taken over („Informatik 1. FS") or the
/// semester, else „Plan n".
fn default_name(program: Option<&Program>, imported: Option<u8>, semester: SemesterKey, held: Option<String>, count: usize, t: &i18n::Texts) -> String {
    if let Some(held) = held {
        return held;
    }
    match (program, imported) {
        (Some(program), Some(fs)) => (t.studyplan_side.name_fs)(&program.name, fs),
        (Some(program), None) => format!("{} {}", program.name, semester.short(t.locale)),
        (None, _) => (t.studyplan_side.name_numbered)(count + 1),
    }
}

/// „Plan": „Plan speichern" (a name, the same name replaces), the saved plans (a click loads one
/// into the semester shown, asking first where that would lose a timetable no saved plan holds;
/// × deletes one), „Link zum Teilen kopieren" (`share`), and „Plan leeren" („Wirklich leeren?"),
/// which „Rückgängig" takes back. The saved plan the timetable holds is marked.
#[component]
fn PlanGroup(ctx: PlanCtx, program: Memo<Option<Program>>, imported: RwSignal<Option<u8>>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.studyplan_side;
    let saved = Saved::open();
    let empty = Memo::new(move |_| ctx.plan.is_none_or(Studyplan::is_empty));
    // Whether the semester shown holds a timetable, and the saved plan it is.
    let held = Memo::new(move |_| {
        let (url, current) = (ctx.url.get(), ctx.current.get());
        let town = ctx.mine.map(MyProgram::town).unwrap_or_default();
        let Some(plan) = ctx.plan else { return (false, None) };
        plan.with(|doc| {
            let key = key_of(&url, current, doc, ctx.today);
            let any = !doc.modules_in(key).is_empty() || !doc.placeholders_in(key).is_empty();
            let marked = saved.with(|saved| saved.plans.iter().find(|p| p.town == town && doc.holds_saved(key, &p.doc)).map(|p| p.name.clone()));
            (any, marked)
        })
    });
    let any = Memo::new(move |_| held.with(|held| held.0));
    let marked = Memo::new(move |_| held.with(|held| held.1.clone()));
    let entries = Memo::new(move |_| saved.with(|saved| saved.plans.iter().map(|p| (p.name.clone(), p.modules())).collect::<Vec<_>>()));
    let cleared = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().is_some_and(|(note, _)| note == s.cleared)));

    // „Plan speichern".
    let naming = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let taken = Memo::new(move |_| name.with(|name| saved.with(|saved| saved.get(name.trim()).is_some())));
    let start_saving = move |_| {
        let count = entries.with_untracked(Vec::len);
        let suggested = program.with_untracked(|program| default_name(program.as_ref(), imported.get_untracked(), ctx.key.get_untracked(), marked.get_untracked(), count, t));
        name.set(suggested);
        naming.set(true);
        request_animation_frame(|| nav::focus_by_id(NAME_ID));
    };
    let stop_saving = move || {
        naming.set(false);
        request_animation_frame(|| nav::focus_by_id(SAVE_ID));
    };
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(plan) = ctx.plan else { return };
        let (key, town) = (ctx.key.get_untracked(), ctx.mine.map(|mine| untrack(|| mine.town())).unwrap_or_default());
        let doc = plan.with_untracked(|doc| doc.semester_only(key));
        let text = name.get_untracked();
        if saved.update(|saved| saved.save(&text, town, doc)) {
            stop_saving();
        }
    };

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

    // „Plan leeren": emptied after the next frame (the views and the calendar let go of it then);
    // the note answers at once and keeps what was there. The program stays the timetable's.
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
        plan.update_after_paint(|doc| {
            *doc = PlanDoc { program: doc.program.take(), extra: std::mem::take(&mut doc.extra), ..PlanDoc::default() };
        });
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
    // nothing to save or to empty, its actions are greyed out.
    view! {
        <div class="fgroup actions">
            <p class="flabel label">{s.plan}</p>
            {move || match (any.get(), naming.get()) {
                (false, _) => view! {
                    <button class="action" type="button" aria-disabled="true" title=s.nothing_to_save><Icon name="bookmark"/><span>{s.save_plan}</span></button>
                }
                .into_any(),
                (true, false) => view! {
                    <button class="action" type="button" id=SAVE_ID on:click=start_saving><Icon name="bookmark"/><span>{s.save_plan}</span></button>
                }
                .into_any(),
                (true, true) => view! {
                    <form class="sp-save" on:submit=save>
                        <input
                            id=NAME_ID
                            type="text"
                            maxlength=MAX_SAVED_NAME.to_string()
                            aria-label=s.plan_name
                            autocomplete="off"
                            prop:value=move || name.get()
                            on:input=move |ev| name.set(event_target_value(&ev))
                            on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                if ev.key() == "Escape" {
                                    stop_saving();
                                }
                            }
                        />
                        <button class="mini hit" type="submit">{move || if taken.get() { s.replace } else { s.save }}</button>
                        <button class="mini hit" type="button" on:click=move |_| stop_saving()>{s.cancel}</button>
                    </form>
                }
                .into_any(),
            }}
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
            <ShareAction ctx/>
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use folia_calendar::kind::{kinds_of, Class};
    use folia_model::labels::Code;
    use folia_timetable::model::{Attendance, Event, Row};

    use crate::i18n::{DE, EN};
    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn program(id: &str, slug: &str, name: &str) -> Program {
        Program {
            id: id.into(),
            slug: slug.into(),
            name: name.into(),
            degree_level: Code::parse("bachelor"),
            study_variant: None,
            degree_label: Some("B.Sc.".into()),
            degree_raw: "Bachelor".into(),
            degree_display: Some("B.Sc.".into()),
            po_version: "2008".into(),
            po_year: Some(2008),
            family_key: "079-82".into(),
            name_key: name.to_lowercase(),
            is_latest_po: true,
            source_url: String::new(),
            has_plan: true,
            plan_status: None,
            curricular_modules: 0,
            fues_modules: 0,
            documents: 0,
        }
    }

    #[test]
    fn the_timetables_program_is_the_addresss_the_plans_or_mine() {
        let all = [program("079-82-2008", "bachelor-informatik-2008", "Informatik"), program("048-82-2022", "bachelor-elektrotechnik-2022", "Elektrotechnik")];
        let id = |found: Option<&Program>| found.map(|program| program.id.clone());
        let (inf, et) = (Some("079-82-2008"), Some("048-82-2022"));
        assert_eq!(id(shown_program(&all, None, None, et)), Some("048-82-2022".into()));
        assert_eq!(id(shown_program(&all, None, inf, et)), Some("079-82-2008".into()));
        assert_eq!(id(shown_program(&all, Some("bachelor-elektrotechnik-2022"), inf, None)), Some("048-82-2022".into()));
        assert_eq!(id(shown_program(&all, Some("mine"), inf, et)), Some("048-82-2022".into()));
        // What the snapshot no longer has falls through; nothing at all is no program.
        assert_eq!(id(shown_program(&all, Some("gone"), Some("999-99-1999"), inf)), Some("079-82-2008".into()));
        assert_eq!(id(shown_program(&all, None, None, None)), None);
    }

    #[test]
    fn plan_speichern_suggests_a_name() {
        let inf = program("079-82-2008", "bachelor-informatik-2008", "Informatik");
        let w = key("2026W");
        assert_eq!(default_name(Some(&inf), Some(1), w, None, 0, &DE), "Informatik 1. FS");
        assert_eq!(default_name(Some(&inf), None, w, None, 0, &DE), "Informatik WiSe 26/27");
        assert_eq!(default_name(None, None, w, None, 2, &DE), "Plan 3");
        assert_eq!(default_name(Some(&inf), Some(1), w, Some("Für Lea".into()), 0, &DE), "Für Lea");
        assert_eq!((default_name(Some(&inf), Some(1), w, None, 0, &EN), default_name(Some(&inf), None, w, None, 0, &EN)), ("Informatik semester 1".to_string(), "Informatik WS 26/27".to_string()));
    }

    fn event(id: &str, type_raw: &str, module: &str, rows: Vec<Row>, chosen: Option<usize>) -> Event {
        let options = rows.iter().filter_map(|row| row.option).map(|option| vec![option]).collect::<Vec<_>>();
        Event {
            id: id.into(),
            number: None,
            title: format!("Veranstaltung {id}"),
            type_raw: Some(type_raw.into()),
            kinds: kinds_of(Some(type_raw)),
            class: Class::Other,
            modules: vec![module.into()],
            tone: 1,
            attendance: if options.is_empty() { Attendance::All } else { Attendance::OneOf { options, basis: folia_timetable::model::Basis::Groups } },
            chosen,
            rows,
            hidden: None,
            source_url: None,
        }
    }

    #[test]
    fn a_type_of_two_kinds_is_explained_once() {
        let mut table = Timetable {
            key: key("2026W"),
            facts: folia_timetable::facts::SemesterFacts::derive(key("2026W"), None, &[]),
            modules: Vec::new(),
            events: vec![event("1", "Vorlesung", "1", Vec::new(), None)],
            exams: Vec::new(),
            tracks: BTreeSet::new(),
            town: None,
            town_derived: false,
            clashes: Vec::new(),
            blocked: Vec::new(),
            exam_warnings: Vec::new(),
            place_unknown: Vec::new(),
            without_dates: Vec::new(),
        };
        assert_eq!(combined_hint(&table, &DE), None);
        table.events.push(event("2", "Vorlesung/Übung", "1", Vec::new(), None));
        assert_eq!(combined_hint(&table, &DE).as_deref(), Some("„Vorlesung/Übung“ zählt als beides."));
        assert_eq!(combined_hint(&table, &EN).as_deref(), Some("“Lecture/Exercise” counts as both."));
    }
}
