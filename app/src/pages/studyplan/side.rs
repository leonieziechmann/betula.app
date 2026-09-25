//! The sidebar of the Stundenplan, „Anpassen" (owner's redesign of 2026-09-25), top to bottom:
//! „Studiengang" (the timetable's program, „Mein Studiengang" by default, and the way to its page),
//! „Importieren" (`import.rs`: a Fachsemester of its Regelstudienplan), then beside a planned
//! timetable its view, what is shown, the Standort, what is hidden and its calendar, and last
//! „Plan": save the timetable under a name, load or delete a saved one, empty it. The storage hint
//! under it is `mod.rs`'s, the same on the server.
//!
//! Every group is the app's alone (the server's sidebar is the storage hint). What is shown of the
//! timetable changes at once, in the click (the timetable is worked out again in Rust, no query,
//! R21); what changes the planned modules is written after the next frame, once the control has
//! answered (`Studyplan::update_after_paint`). Each control reads a memo of its own (R5), and no
//! closure reads a memo together with the one it is derived from (R16).

use std::collections::{BTreeMap, BTreeSet};

use catalog::labels::Rhythm;
use catalog::queries;
use catalog::rows::Program;
use catalog::studyplan::{PlanDoc, MAX_SAVED_NAME};
use catalog::timetable::day::{clock, Day};
use catalog::timetable::kind::{EventKind, KindSet};
use catalog::timetable::model::{Row, Timetable};
use catalog::timetable::rowkey::RowKey;
use catalog::timetable::select::{Town, TownChoice};
use catalog::timetable::semester::SemesterKey;
use catalog::url::{self, PlanView, StudyplanUrl};
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use super::export::CalendarGroup;
use super::head::{kind_word, weekday_name};
use super::import::ImportGroup;
use super::{key_of, PlanCtx};
use crate::combobox::{ComboItem, Combobox};
use crate::format;
use crate::myprogram::{po_of, program_href, program_name, MyProgram};
use crate::nav;
use crate::pending::Pending;
use crate::studyplan::{Saved, Studyplan};
use crate::ui::Icon;

/// The note „Rückgängig" answers after „Plan leeren" (`PlanCtx::undo`, which the import shares).
const CLEARED: &str = "Plan geleert";

/// The id of „Plan leeren", where the focus returns from „Abbrechen" and „Rückgängig".
const CLEAR_ID: &str = "sp-clear";

/// The id of „Plan speichern", and of the name field it opens.
const SAVE_ID: &str = "sp-save";
const NAME_ID: &str = "sp-save-name";

/// The id of „Ersetzen" when loading a saved plan would replace an unsaved one.
const LOAD_ID: &str = "sp-load-yes";

#[component]
pub(super) fn PlanSidebar(ctx: PlanCtx) -> impl IntoView {
    let empty = Memo::new(move |_| ctx.plan.is_none_or(Studyplan::is_empty));
    let programs = Programs::new(ctx);
    // The Fachsemester last taken over in this visit, for the name „Plan speichern" suggests.
    let imported = RwSignal::new(None::<u8>);
    view! {
        <ProgramGroup ctx programs/>
        <ImportGroup ctx program=programs.shown imported/>
        {move || {
            (!empty.get()).then(|| view! {
                <ViewGroup ctx/>
                <KindsGroup ctx/>
                <TownGroup ctx/>
                <HiddenGroup ctx/>
                <CalendarGroup ctx/>
            })
        }}
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
        let all = Memo::new(move |_| ctx.source.with_value(|source| source.as_ref().and_then(|source| source.run(|db| queries::programs(db)).ok())).unwrap_or_default());
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
    let Programs { all, mine, shown } = programs;
    let items = Signal::derive(move || {
        let mine = mine.get();
        all.with(|all| {
            let item = |program: &Program| ComboItem::new(program.id.clone(), program.name.clone(), format!("{} · PO {}", program.degree(), po_of(program)), i64::from(program.is_latest_po));
            let own = mine.as_deref().and_then(|id| all.iter().find(|program| program.id == id));
            own.map(|program| item(program).in_group("Mein Studiengang")).into_iter().chain(all.iter().map(item)).collect::<Vec<_>>()
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
    // The program's page with the plan of the stored Studienrichtung where it is „Mein
    // Studiengang".
    let link = Memo::new(move |_| {
        let named = ctx.mine.map(|mine| mine.with(|doc| (doc.program.clone(), doc.caption.clone(), doc.direction.clone())));
        shown.with(|program| {
            let program = program.as_ref()?;
            let (caption, direction) = match named {
                Some((Some(id), caption, direction)) if id == program.id => (caption, direction),
                _ => (None, None),
            };
            Some(ctx.source.with_value(|source| program_href(source.as_ref(), program, caption.as_deref(), direction.as_deref())))
        })
    });
    view! {
        <div class="fgroup first sp-program">
            <p class="flabel label">"Studiengang"</p>
            <Combobox
                id="sp-program"
                label="Studiengang"
                placeholder="Studiengang wählen"
                search_placeholder="Studiengang suchen"
                icon="graduation-cap"
                min_width=480.0
                items
                selected
                on_select=pick
                clearable=false
            />
            {move || link.get().map(|href| view! { <a class="sp-more" href=href>"Studium planen →"</a> })}
        </div>
    }
}

// ---------- 2. Ansicht ----------

/// „Ansicht": Woche · Termine · Prüfungen, the module beside the plan staying.
#[component]
fn ViewGroup(ctx: PlanCtx) -> impl IntoView {
    let shown = shown_url(ctx);
    let link = move |view: PlanView| {
        let checked = Memo::new(move |_| shown.with(|shown| shown.view == view));
        view! {
            <a
                href=move || ctx.url.with(|url| url.with_view(view).path())
                role="radio"
                draggable="false"
                aria-checked=move || if checked.get() { "true" } else { "false" }
            >
                {view.label()}
            </a>
        }
    };
    view! {
        <div class="fgroup">
            <p class="flabel label">"Ansicht"</p>
            <div class="seg" role="radiogroup" aria-label="Ansicht">
                {[PlanView::Week, PlanView::Dates, PlanView::Exams].into_iter().map(link).collect_view()}
            </div>
        </div>
    }
}

// ---------- 3. Zeigen ----------

/// Under the chips, where an event counts for two kinds: „„Vorlesung/Übung“ zählt als beides."
fn combined_hint(table: &Timetable) -> Option<String> {
    let kinds = table.events.iter().map(|event| event.kinds.known()).find(|kinds| kinds.iter().count() >= 2)?;
    let labels: Vec<&str> = kinds.iter().map(EventKind::label).collect();
    let as_what = if labels.len() == 2 { "beides" } else { "jede dieser Arten" };
    Some(format!("„{}“ zählt als {as_what}.", labels.join("/")))
}

/// „Zeigen": a chip per kind of the semester with its number of events; a click hides or shows
/// the kind (an event goes only when all its kinds are hidden).
#[component]
fn KindsGroup(ctx: PlanCtx) -> impl IntoView {
    let kinds = Memo::new(move |_| ctx.table.with(|table| table.as_ref().map(Timetable::kinds_present).unwrap_or_default()));
    let hidden = Memo::new(move |_| ctx.selection.with(|(_, selection)| selection.hidden_kinds));
    let combined = Memo::new(move |_| ctx.table.with(|table| table.as_ref().and_then(combined_hint)));
    let any = Memo::new(move |_| kinds.with(|kinds| !kinds.is_empty()));
    move || {
        any.get().then(|| {
            view! {
                <div class="fgroup">
                    <p class="flabel label">"Zeigen"</p>
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
                                        title=move || if off.get() { "Einblenden" } else { "Ausblenden" }
                                        on:click=toggle
                                    >
                                        <span class="chip-label">{kind.label()}</span>
                                        <span class="chip-count num">{count}</span>
                                    </button>
                                }
                            }
                        />
                    </div>
                    {move || combined.get().map(|text| view! { <p class="hint">{text}</p> })}
                </div>
            }
        })
    }
}

// ---------- 4. Standort ----------

/// „Standort", for modules taught in both towns: Cottbus · Senftenberg · Beide. With nothing
/// stored the derived town is checked (the line „Abgeleitet: …" says so); a click stores the
/// choice in „Mein Studiengang".
#[component]
fn TownGroup(ctx: PlanCtx) -> impl IntoView {
    let tracks = Memo::new(move |_| ctx.table.with(|table| table.as_ref().is_some_and(|table| !table.tracks.is_empty())));
    let derived = Memo::new(move |_| ctx.table.with(|table| table.as_ref().filter(|table| table.town_derived).and_then(|table| table.town)));
    let choice = Memo::new(move |_| ctx.selection.with(|(_, selection)| selection.town));
    let shown = Memo::new(move |_| tracks.get() || choice.get() != TownChoice::Derive);
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
                {label}
            </button>
        }
    };
    move || {
        shown.get().then(|| {
            view! {
                <div class="fgroup">
                    <p class="flabel label">"Standort"</p>
                    <div class="seg" role="radiogroup" aria-label="Standort">
                        {button("Cottbus", TownChoice::Only(Town::Cottbus))}
                        {button("Senftenberg", TownChoice::Only(Town::Senftenberg))}
                        {button("Beide", TownChoice::Both)}
                    </div>
                </div>
            }
        })
    }
}

// ---------- 5. Ausgeblendet ----------

/// What „Einblenden" takes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Unhide {
    Event(u32),
    Row(RowKey),
    Choice(u32),
}

/// One line of „Ausgeblendet": „Tutorium Mathematik IT-1 · Di 15:30".
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct HiddenLine {
    text: String,
    unhide: Unhide,
}

/// When a Termin meets, in a word: „Di 15:30"; a single date with its day („Di 23.02. 11:45").
fn row_when(row: &Row) -> String {
    let weekday = row.date.weekday.and_then(|day| u8::try_from(day).ok()).filter(|day| (1..=7).contains(day));
    let single = row.date.rhythm.as_ref().is_some_and(|rhythm| rhythm.is(Rhythm::Single));
    let date = row.date.first_date.as_deref().and_then(Day::parse).map(Day::short).filter(|_| single || weekday.is_none());
    let parts: Vec<String> = [weekday.map(|day| weekday_name(day).to_string()), date, row.from.map(clock)].into_iter().flatten().collect();
    parts.join(" ")
}

/// The lines of „Ausgeblendet" (A.3): each hidden event, each hidden Termin and each made choice
/// of the semester's timetable, exams included, in the timetable's order. `events` and `rows` are
/// what the semester hides; a choice is the timetable's own (`Event::chosen`). Titles are the
/// modules' (`titles`).
fn hidden_lines(table: &Timetable, events: &BTreeSet<u32>, rows: &BTreeSet<RowKey>, titles: &BTreeMap<String, String>) -> Vec<HiddenLine> {
    let title = |modules: &[String], fallback: &str| modules.first().and_then(|module| titles.get(module)).cloned().unwrap_or_else(|| fallback.to_string());
    let mut lines = Vec::new();
    let mut seen = BTreeSet::new();
    for event in &table.events {
        let name = format!("{} {}", kind_word(event), title(&event.modules, &event.title));
        let id = event.id.parse::<u32>().ok();
        if let Some(id) = id.filter(|id| events.contains(id)) {
            lines.push(HiddenLine { text: name.clone(), unhide: Unhide::Event(id) });
        }
        for row in &event.rows {
            if let Some(key) = row.key.filter(|key| rows.contains(key) && seen.insert(*key)) {
                lines.push(HiddenLine { text: format!("{name} · {}", row_when(row)), unhide: Unhide::Row(key) });
            }
        }
        if let (Some(option), Some(id)) = (event.chosen, id) {
            let when = event.rows.iter().find(|row| row.option == Some(option)).map(row_when).unwrap_or_default();
            lines.push(HiddenLine { text: format!("{name} · nur {when}"), unhide: Unhide::Choice(id) });
        }
    }
    for exam in &table.exams {
        let name = format!("Prüfung {}", title(&exam.modules, &exam.title));
        if let Some(id) = exam.event_id.parse::<u32>().ok().filter(|id| events.contains(id)) {
            lines.push(HiddenLine { text: name.clone(), unhide: Unhide::Event(id) });
        }
        for row in &exam.rows {
            if let Some(key) = row.key.filter(|key| rows.contains(key) && seen.insert(*key)) {
                let day = row.date.first_date.as_deref().and_then(Day::parse).map(Day::short).unwrap_or_default();
                lines.push(HiddenLine { text: format!("{name} · {day}"), unhide: Unhide::Row(key) });
            }
        }
    }
    lines
}

/// „Ausgeblendet (n)", while anything is: a line each with „Einblenden", and „Alle einblenden"
/// (events, Termine and choices of the semester; hidden kinds keep their chips).
#[component]
fn HiddenGroup(ctx: PlanCtx) -> impl IntoView {
    let hides = Memo::new(move |_| ctx.selection.with(|(_, selection)| (selection.hidden_events.clone(), selection.hidden_rows.clone())));
    let titles = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(|data| data.titles()).unwrap_or_default()));
    let lines = Memo::new(move |_| {
        hides.with(|(events, rows)| {
            titles.with(|titles| ctx.table.with(|table| table.as_ref().map(|table| hidden_lines(table, events, rows, titles)).unwrap_or_default()))
        })
    });
    let any = Memo::new(move |_| lines.with(|lines| !lines.is_empty()));
    let unhide = move |what: Unhide| {
        let key = ctx.key.get_untracked();
        if let Some(plan) = ctx.plan {
            plan.update(|doc| match what {
                Unhide::Event(id) => doc.set_event(key, id, false),
                Unhide::Row(row) => doc.set_row(key, row, false),
                Unhide::Choice(id) => doc.choose(key, id, None),
            });
        }
    };
    let show_all = move |_| {
        let key = ctx.key.get_untracked();
        if let Some(plan) = ctx.plan {
            plan.update(|doc| doc.show_all(key));
        }
    };
    move || {
        any.get().then(|| {
            view! {
                <details class="fgroup sp-hidden" open=true>
                    <summary class="label">"Ausgeblendet ("{move || lines.with(Vec::len)}")"</summary>
                    <For
                        each=move || lines.get()
                        key=|line| line.clone()
                        children=move |line: HiddenLine| {
                            let what = line.unhide;
                            // The line's text across the sidebar, its button under it (`.ask`), the
                            // same for a short line as for a long one.
                            view! {
                                <p class="action note-action ask">
                                    <span>{line.text}</span>
                                    <button class="mini hit" type="button" on:click=move |_| unhide(what)>"Einblenden"</button>
                                </p>
                            }
                        }
                    />
                    <p class="action note-action"><button class="mini hit" type="button" on:click=show_all>"Alle einblenden"</button></p>
                </details>
            }
        })
    }
}

// ---------- 6. Plan ----------

/// The name „Plan speichern" suggests: the saved plan the timetable holds (saving again replaces
/// it), else the program with the Fachsemester last taken over („Informatik 1. FS") or the
/// semester, else „Plan n".
fn default_name(program: Option<&Program>, imported: Option<u8>, semester: SemesterKey, held: Option<String>, count: usize) -> String {
    if let Some(held) = held {
        return held;
    }
    match (program, imported) {
        (Some(program), Some(fs)) => format!("{} {fs}. FS", program.name),
        (Some(program), None) => format!("{} {}", program.name, semester.short()),
        (None, _) => format!("Plan {}", count + 1),
    }
}

/// „Plan": „Plan speichern" (a name, the same name replaces), the saved plans (a click loads one
/// into the semester shown, asking first where that would lose a timetable no saved plan holds;
/// × deletes one), and „Plan leeren" („Wirklich leeren?"), which „Rückgängig" takes back. The
/// saved plan the timetable holds is marked.
#[component]
fn PlanGroup(ctx: PlanCtx, program: Memo<Option<Program>>, imported: RwSignal<Option<u8>>) -> impl IntoView {
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
    let cleared = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().is_some_and(|(note, _)| note == CLEARED)));

    // „Plan speichern".
    let naming = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let taken = Memo::new(move |_| name.with(|name| saved.with(|saved| saved.get(name.trim()).is_some())));
    let start_saving = move |_| {
        let count = entries.with_untracked(Vec::len);
        let suggested = program.with_untracked(|program| default_name(program.as_ref(), imported.get_untracked(), ctx.key.get_untracked(), marked.get_untracked(), count));
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
        ctx.undo.set(Some((CLEARED.to_string(), plan.with_untracked(Clone::clone))));
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

    // The group stays while its note does, so emptying the plan under it keeps the focus on
    // „Rückgängig"; saved plans keep it for an empty timetable.
    let shown = Memo::new(move |_| !empty.get() || cleared.get() || entries.with(|entries| !entries.is_empty()));
    move || {
        shown.get().then(|| {
            view! {
                <div class="fgroup actions">
                    <p class="flabel label">"Plan"</p>
                    {move || match (any.get(), naming.get()) {
                        (false, _) => ().into_any(),
                        (true, false) => view! {
                            <button class="action" type="button" id=SAVE_ID on:click=start_saving><Icon name="bookmark"/><span>"Plan speichern"</span></button>
                        }
                        .into_any(),
                        (true, true) => view! {
                            <form class="sp-save" on:submit=save>
                                <input
                                    id=NAME_ID
                                    type="text"
                                    maxlength=MAX_SAVED_NAME.to_string()
                                    aria-label="Name des Plans"
                                    autocomplete="off"
                                    prop:value=move || name.get()
                                    on:input=move |ev| name.set(event_target_value(&ev))
                                    on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                        if ev.key() == "Escape" {
                                            stop_saving();
                                        }
                                    }
                                />
                                <button class="mini hit" type="submit">{move || if taken.get() { "Ersetzen" } else { "Speichern" }}</button>
                                <button class="mini hit" type="button" on:click=move |_| stop_saving()>"Abbrechen"</button>
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
                                        <small class="num">{format::modules(i64::try_from(count).unwrap_or(i64::MAX))}</small>
                                    </button>
                                    <button class="icon-btn" type="button" aria-label=format!("„{which}“ löschen") title="Löschen" on:click=move |_| delete(two.clone())>
                                        <Icon name="x"/>
                                    </button>
                                </div>
                                {move || {
                                    let three = three.clone();
                                    asked.get().then(|| view! {
                                        <p class="action note-action ask">
                                            <span>"Aktuellen Plan ersetzen?"</span>
                                            <button class="mini danger hit" type="button" id=LOAD_ID on:click=move |_| load_now(three.clone())>"Ersetzen"</button>
                                            <button class="mini hit" type="button" on:click=move |_| asking.set(None)>"Abbrechen"</button>
                                        </p>
                                    })
                                }}
                            }
                        }
                    />
                    {move || match (cleared.get(), confirming.get(), empty.get()) {
                        (true, _, _) => view! {
                            <p class="action note-action">
                                <Icon name="check"/>
                                <span>{CLEARED}</span>
                                <button class="mini hit" type="button" id="sp-clear-undo" aria-busy=move || restoring.get().then_some("true") on:click=undo>"Rückgängig"</button>
                            </p>
                        }
                        .into_any(),
                        (false, true, _) => view! {
                            <p class="action note-action ask">
                                <span>"Wirklich leeren?"</span>
                                <button class="mini danger hit" type="button" id="sp-clear-yes" on:click=clear>"Leeren"</button>
                                <button class="mini hit" type="button" on:click=cancel>"Abbrechen"</button>
                            </p>
                        }
                        .into_any(),
                        (false, false, false) => view! {
                            <button class="action" type="button" id=CLEAR_ID on:click=ask><Icon name="trash-2"/><span>"Plan leeren"</span></button>
                        }
                        .into_any(),
                        (false, false, true) => ().into_any(),
                    }}
                </div>
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use catalog::labels::Code;
    use catalog::rows_detail::EventDate;
    use catalog::timetable::kind::{kinds_of, Class};
    use catalog::timetable::model::{Attendance, Event};
    use catalog::timetable::occur::Occurrences;

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
        assert_eq!(default_name(Some(&inf), Some(1), w, None, 0), "Informatik 1. FS");
        assert_eq!(default_name(Some(&inf), None, w, None, 0), "Informatik WiSe 26/27");
        assert_eq!(default_name(None, None, w, None, 2), "Plan 3");
        assert_eq!(default_name(Some(&inf), Some(1), w, Some("Für Lea".into()), 0), "Für Lea");
    }

    fn row(event: &str, fp: u32, weekday: i64, from: &str, rhythm: &str, first: &str, option: Option<usize>) -> Row {
        Row {
            key: Some(RowKey { event: event.parse().unwrap(), fp }),
            ord: Some(1),
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: event.into(),
                event_number: None,
                event_title: String::new(),
                event_type: None,
                group_name: None,
                weekday: Some(weekday),
                start_time: Some(from.into()),
                end_time: None,
                rhythm: Some(Code::parse(rhythm)),
                rhythm_raw: None,
                first_date: Some(first.into()),
                last_date: Some(first.into()),
                room: None,
                campus: None,
                instructor: None,
                comment: None,
                source_url: None,
            },
            cancelled_dates: None,
            occ: Occurrences::default(),
            from: catalog::timetable::day::minutes(from),
            to: None,
            option,
            hidden: None,
        }
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
            attendance: if options.is_empty() { Attendance::All } else { Attendance::OneOf { options, basis: catalog::timetable::model::Basis::Groups } },
            chosen,
            rows,
            hidden: None,
            source_url: None,
        }
    }

    #[test]
    fn what_is_hidden_is_named_as_the_plan_names_it() {
        let mut table = Timetable {
            key: key("2026W"),
            facts: catalog::timetable::facts::SemesterFacts::derive(key("2026W"), None, &[]),
            modules: vec!["11112".into(), "12104".into()],
            events: vec![
                event("150132", "Tutorium", "11112", vec![row("150132", 7, 2, "15:30", "weekly", "2026-10-13", None)], None),
                event(
                    "148369",
                    "Übung",
                    "12104",
                    vec![row("148369", 1, 1, "15:30", "weekly", "2026-10-12", Some(0)), row("148369", 2, 2, "11:30", "weekly", "2026-10-13", Some(1))],
                    Some(0),
                ),
                event("148019", "Übung", "12104", vec![row("148019", 3, 2, "11:45", "single", "2027-02-23", None)], None),
            ],
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
        let titles: BTreeMap<String, String> =
            [("11112", "Mathematik IT-1"), ("12104", "Entwicklung von Softwaresystemen")].iter().map(|(id, title)| (id.to_string(), title.to_string())).collect();
        let events: BTreeSet<u32> = [150132].into_iter().collect();
        let rows: BTreeSet<RowKey> = [RowKey { event: 150132, fp: 7 }, RowKey { event: 148019, fp: 3 }].into_iter().collect();
        let lines = hidden_lines(&table, &events, &rows, &titles);
        let texts: Vec<(&str, Unhide)> = lines.iter().map(|line| (line.text.as_str(), line.unhide)).collect();
        assert_eq!(
            texts,
            vec![
                ("Tutorium Mathematik IT-1", Unhide::Event(150132)),
                ("Tutorium Mathematik IT-1 · Di 15:30", Unhide::Row(RowKey { event: 150132, fp: 7 })),
                ("Übung Entwicklung von Softwaresystemen · nur Mo 15:30", Unhide::Choice(148369)),
                ("Übung Entwicklung von Softwaresystemen · Di 23.02. 11:45", Unhide::Row(RowKey { event: 148019, fp: 3 })),
            ]
        );
        // Nothing hidden, nothing chosen: no line.
        table.events.iter_mut().for_each(|event| event.chosen = None);
        assert!(hidden_lines(&table, &BTreeSet::new(), &BTreeSet::new(), &titles).is_empty());
    }

    #[test]
    fn a_type_of_two_kinds_is_explained_once() {
        let mut table = Timetable {
            key: key("2026W"),
            facts: catalog::timetable::facts::SemesterFacts::derive(key("2026W"), None, &[]),
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
        assert_eq!(combined_hint(&table), None);
        table.events.push(event("2", "Vorlesung/Übung", "1", Vec::new(), None));
        assert_eq!(combined_hint(&table).as_deref(), Some("„Vorlesung/Übung“ zählt als beides."));
    }
}
