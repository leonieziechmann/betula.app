//! The sidebar of the Studienplan, „Anpassen" (design A.3): which semester and which view, what is
//! shown of the semester (kinds, Standort, what was hidden or chosen), its calendar, „Mein
//! Studiengang", and the plan as a whole. The storage hint under it is `mod.rs`'s, the same on the
//! server.
//!
//! Every group is the app's alone (the server's sidebar is the storage hint). The groups of one
//! semester stand only beside one semester; the Übersicht and an empty plan have the semesters, Mein
//! Studiengang and the plan. What is shown of a semester changes at once, in the click (the
//! timetable is worked out again in Rust, no query, R21); what changes the planned modules is
//! written after the next frame, once the control has answered (`Studyplan::update_after_paint`).
//! Each control reads a memo of its own (R5), and no closure reads a memo together with the one it
//! is derived from (R16).

use std::collections::{BTreeMap, BTreeSet};

use catalog::queries;
use catalog::rows::Program;
use catalog::studyplan::PlanDoc;
use catalog::timetable::day::{clock, Day};
use catalog::timetable::kind::{EventKind, KindSet};
use catalog::timetable::model::{Row, Timetable};
use catalog::timetable::rowkey::RowKey;
use catalog::timetable::select::{Town, TownChoice};
use catalog::timetable::semester::SemesterKey;
use catalog::url::{self, PlanView, StudyplanUrl};
use catalog::labels::Rhythm;
use leptos::prelude::*;

use super::export::CalendarGroup;
use super::head::{kind_word, semester_href, weekday_name};
use super::{key_of, PlanCtx};
use crate::combobox::{ComboItem, Combobox};
use crate::myprogram::{po_of, program_href, program_name, MineResolved, MyProgram};
use crate::nav;
use crate::pending::Pending;
use crate::studyplan::Studyplan;
use crate::ui::Icon;

/// The note „Rückgängig" answers after „Plan leeren" (`PlanCtx::undo`, which the import shares).
const CLEARED: &str = "Plan geleert";

/// How many semesters before the current one „Studienbeginn" offers.
const STARTS_BEFORE: i32 = 12;

/// The id of the „Studienbeginn" select.
const START_ID: &str = "sp-start";

/// The id of „Plan leeren", where the focus returns from „Abbrechen" and „Rückgängig".
const CLEAR_ID: &str = "sp-clear";

#[component]
pub(super) fn PlanSidebar(ctx: PlanCtx) -> impl IntoView {
    let empty = Memo::new(move |_| ctx.plan.is_none_or(Studyplan::is_empty));
    let overview = Memo::new(move |_| ctx.url.with(|url| url.view == PlanView::Overview));
    view! {
        {move || (!empty.get()).then(|| view! { <SemesterGroup ctx/> })}
        {move || {
            (!empty.get() && !overview.get()).then(|| view! {
                <ViewGroup ctx/>
                <KindsGroup ctx/>
                <TownGroup ctx/>
                <HiddenGroup ctx/>
                <CalendarGroup ctx/>
            })
        }}
        <MineGroup ctx first=empty/>
        <PlanGroup ctx/>
    }
}

/// The address the page is going to (`pending`), else the one it is at: what the sidebar's
/// semesters and views mark, so a click marks its own link in the next frame (R21).
fn shown_url(ctx: PlanCtx) -> Memo<StudyplanUrl> {
    let going = Pending::expect();
    Memo::new(move |_| match going.and_then(|going| going.search_on(url::STUDYPLAN)) {
        Some(search) => StudyplanUrl::parse(&search),
        None => ctx.url.get(),
    })
}

// ---------- 1. Semester ----------

/// The semesters the toc lists, in order: those the plan holds, the current one and the one
/// shown; each with whether it is the current one („jetzt") and whether it is past (muted).
fn toc_semesters(held: &[SemesterKey], current: Option<SemesterKey>, shown: SemesterKey) -> Vec<(SemesterKey, bool, bool)> {
    let all: BTreeSet<SemesterKey> = held.iter().copied().chain(current).chain([shown]).collect();
    all.into_iter().map(|key| (key, current == Some(key), current.is_some_and(|current| key < current))).collect()
}

/// „Semester": the Übersicht, then each semester by its label alone (the head has its numbers).
#[component]
fn SemesterGroup(ctx: PlanCtx) -> impl IntoView {
    let shown = shown_url(ctx);
    let overview = Memo::new(move |_| shown.with(|shown| shown.view == PlanView::Overview));
    // The semester marked: the one of the address being shown, worked out with the plan (B.4).
    let marked = Memo::new(move |_| {
        let (shown, current) = (shown.get(), ctx.current.get());
        if shown.view == PlanView::Overview {
            return None;
        }
        Some(ctx.plan.map_or_else(|| key_of(&shown, current, &PlanDoc::default(), ctx.today), |plan| plan.with(|doc| key_of(&shown, current, doc, ctx.today))))
    });
    // The semester of the address being shown is listed at once too, so ‹ › to a semester the
    // plan does not hold mark it in the next frame. `shown` alone, never with `ctx.url` it is
    // derived from (R16).
    let semesters = Memo::new(move |_| {
        let (shown, current) = (shown.get(), ctx.current.get());
        let (held, key) = match ctx.plan {
            Some(plan) => plan.with(|doc| (doc.semesters(), key_of(&shown, current, doc, ctx.today))),
            None => (Vec::new(), key_of(&shown, current, &PlanDoc::default(), ctx.today)),
        };
        toc_semesters(&held, current, key)
    });
    let view = Memo::new(move |_| ctx.url.with(|url| url.view));
    let overview_href = StudyplanUrl { view: PlanView::Overview, ..Default::default() }.path();
    view! {
        <nav class="toc fgroup first" aria-label="Semester">
            <p class="flabel label">"Semester"</p>
            <a href=overview_href aria-current=move || overview.get().then_some("page")>"Übersicht"</a>
            <For
                each=move || semesters.get()
                key=|entry| *entry
                children=move |(key, now, past): (SemesterKey, bool, bool)| {
                    let here = Memo::new(move |_| marked.get() == Some(key));
                    view! {
                        <a class:past=past href=move || semester_href(view.get(), Some(key)).unwrap_or_default() aria-current=move || here.get().then_some("page")>
                            {key.label()}
                            {now.then(|| view! { <small class="num">"jetzt"</small> })}
                        </a>
                    }
                }
            />
        </nav>
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

// ---------- 7. Mein Studiengang ----------

/// The semesters „Studienbeginn" offers, in order: the current one, the twelve before it and the
/// next, and a stored one outside them.
fn start_options(current: Option<SemesterKey>, start: Option<SemesterKey>) -> Vec<SemesterKey> {
    let around = current.into_iter().flat_map(|current| (-STARTS_BEFORE..=1).filter_map(move |n| current.plus(n)));
    let all: BTreeSet<SemesterKey> = around.chain(start).collect();
    all.into_iter().collect()
}

/// „Mein Studiengang" as the sidebar shows it.
#[derive(Clone, Debug, PartialEq)]
enum Mine {
    /// None stored: the picker.
    Unset,
    /// Stored and in the snapshot: a link to its page.
    Set(String),
    /// Stored but gone from the snapshot (A.10): its stored name, and the family's newest PO.
    Gone(String, Option<Box<Program>>),
}

/// „Mein Studiengang": the program (a link to its page, or the picker while none is set) and
/// „Studienbeginn", which the Fachsemester follow.
#[component]
fn MineGroup(ctx: PlanCtx, #[prop(into)] first: Signal<bool>) -> impl IntoView {
    let mine = ctx.mine;
    let resolved = MineResolved::expect();
    let stored = Memo::new(move |_| mine.and_then(|mine| mine.with(|doc| doc.program.clone())));
    let name = Memo::new(move |_| mine.and_then(|mine| mine.with(|doc| doc.name.clone().or_else(|| doc.program.clone()))).unwrap_or_default());
    let caption = Memo::new(move |_| mine.and_then(|mine| mine.with(|doc| doc.caption.clone())));
    let direction = Memo::new(move |_| mine.and_then(|mine| mine.with(|doc| doc.direction.clone())));
    let info = Memo::new(move |_| resolved.and_then(|resolved| resolved.0.get()));
    let state = Memo::new(move |_| match (stored.with(Option::is_some), info.get()) {
        (false, _) => Mine::Unset,
        (true, Some(info)) if info.exact => Mine::Set(program_name(&info.program)),
        (true, Some(info)) => Mine::Gone(name.get(), info.latest.map(Box::new)),
        (true, None) => Mine::Gone(name.get(), None),
    });
    let link = Memo::new(move |_| {
        let (info, caption, direction) = (info.get()?, caption.get(), direction.get());
        Some(ctx.source.with_value(|source| program_href(source.as_ref(), &info.program, caption.as_deref(), direction.as_deref())))
    });
    let take_latest = move |latest: &Program| {
        let (id, name) = (latest.id.clone(), program_name(latest));
        move |_| {
            if let Some(mine) = mine {
                let caption = caption.get_untracked().unwrap_or_default();
                mine.set_program(&id, &name, &caption, None);
            }
        }
    };

    view! {
        <div class="fgroup" class:first=move || first.get()>
            <p class="flabel label">"Mein Studiengang"</p>
            {move || match state.get() {
                Mine::Unset => view! { <ProgramPicker ctx/> }.into_any(),
                Mine::Set(name) => view! {
                    <a class="action" href=move || link.get().unwrap_or_default()><Icon name="graduation-cap"/><span>{name}</span></a>
                }
                .into_any(),
                Mine::Gone(name, latest) => view! {
                    <p class="hint">{format!("Dein Studiengang {name} ist nicht mehr im Katalog.")}</p>
                    {latest.map(|latest| {
                        let label = format!("PO {} übernehmen", po_of(&latest));
                        view! { <p class="action note-action"><button class="mini hit" type="button" on:click=take_latest(&latest)>{label}</button></p> }
                    })}
                }
                .into_any(),
            }}
            <StartSelect ctx/>
        </div>
    }
}

/// The programs to pick from while „Mein Studiengang" is not set: name, degree and PO, the newest
/// PO of a program first among equals. Picking one stores it.
#[component]
fn ProgramPicker(ctx: PlanCtx) -> impl IntoView {
    let programs = Memo::new(move |_| ctx.source.with_value(|source| source.as_ref().and_then(|source| source.run(|db| queries::programs(db)).ok())).unwrap_or_default());
    let items = Signal::derive(move || {
        programs.with(|programs| {
            programs
                .iter()
                .map(|program| ComboItem::new(program.id.clone(), program.name.clone(), format!("{} · {}", program.degree(), po_of(program)), i64::from(program.is_latest_po)))
                .collect::<Vec<_>>()
        })
    });
    let pick = Callback::new(move |id: Option<String>| {
        let Some(id) = id else { return };
        let Some(program) = programs.with_untracked(|programs| programs.iter().find(|program| program.id == id).cloned()) else { return };
        // The picker makes way for the program's link once it is stored. The focus leaves it
        // first, for the field that comes next: a picker taken away while it holds the focus
        // hears its own blur after it is gone.
        nav::focus_by_id(START_ID);
        if let Some(mine) = ctx.mine {
            mine.set_program(&program.id, &program_name(&program), "", None);
        }
    });
    view! {
        <Combobox
            id="sp-program"
            label="Mein Studiengang"
            placeholder="Studiengang wählen"
            search_placeholder="Studiengang suchen"
            icon="graduation-cap"
            min_width=480.0
            items
            selected=Signal::derive(|| None::<String>)
            on_select=pick
            clearable=false
        />
    }
}

/// „Studienbeginn": the current semester, the twelve before it and the next one („—" for none).
#[component]
fn StartSelect(ctx: PlanCtx) -> impl IntoView {
    let start = Memo::new(move |_| ctx.mine.and_then(MyProgram::start));
    let options = Memo::new(move |_| start_options(ctx.current.get(), start.get()));
    let change = move |ev: leptos::ev::Event| {
        if let Some(mine) = ctx.mine {
            mine.set_start(SemesterKey::parse(&event_target_value(&ev)));
        }
    };
    view! {
        <label class="field">
            <span>"Studienbeginn"</span>
            <span class="select-wrap plain">
                <select id=START_ID prop:value=move || start.get().map(SemesterKey::key).unwrap_or_default() on:change=change>
                    <option value="" selected=move || start.with(Option::is_none)>"—"</option>
                    <For
                        each=move || options.get()
                        key=|key| *key
                        children=move |key: SemesterKey| {
                            view! { <option value=key.key() selected=move || start.get() == Some(key)>{key.label()}</option> }
                        }
                    />
                </select>
                <Icon name="chevrons-up-down"/>
            </span>
        </label>
    }
}

// ---------- 8. Plan ----------

/// „Plan": take a Regelstudienplan over, or empty the plan („Wirklich leeren?"), which „Rückgängig"
/// takes back. An empty plan has its own way to begin in the page, so only the note stays here.
#[component]
fn PlanGroup(ctx: PlanCtx) -> impl IntoView {
    let empty = Memo::new(move |_| ctx.plan.is_none_or(Studyplan::is_empty));
    let cleared = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().is_some_and(|(note, _)| note == CLEARED)));
    let confirming = RwSignal::new(false);
    let restoring = RwSignal::new(false);
    let import = StudyplanUrl { view: PlanView::Overview, import: Some("mine".to_string()), ..Default::default() }.path();

    let ask = move |_| {
        confirming.set(true);
        request_animation_frame(|| nav::focus_by_id("sp-clear-yes"));
    };
    // The plan is emptied after the next frame (the views and the calendar let go of it then);
    // the note answers at once and keeps what was there.
    let clear = move |_| {
        confirming.set(false);
        let Some(plan) = ctx.plan else { return };
        ctx.undo.set(Some((CLEARED.to_string(), plan.with_untracked(Clone::clone))));
        plan.update_after_paint(|doc| {
            *doc = PlanDoc { extra: std::mem::take(&mut doc.extra), ..PlanDoc::default() };
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
    // „Rückgängig".
    let shown = Memo::new(move |_| !empty.get() || cleared.get());
    move || {
        shown.get().then(|| {
            let import = import.clone();
            view! {
                <div class="fgroup actions">
                    <p class="flabel label">"Plan"</p>
                    {move || {
                        let import = import.clone();
                        (!empty.get()).then(|| view! { <a class="action" href=import><Icon name="download"/><span>"Regelstudienplan übernehmen"</span></a> })
                    }}
                    {move || match (cleared.get(), confirming.get()) {
                        (true, _) => view! {
                            <p class="action note-action">
                                <Icon name="check"/>
                                <span>{CLEARED}</span>
                                <button class="mini hit" type="button" id="sp-clear-undo" aria-busy=move || restoring.get().then_some("true") on:click=undo>"Rückgängig"</button>
                            </p>
                        }
                        .into_any(),
                        (false, true) => view! {
                            <p class="action note-action ask">
                                <span>"Wirklich leeren?"</span>
                                <button class="mini danger hit" type="button" id="sp-clear-yes" on:click=clear>"Leeren"</button>
                                <button class="mini hit" type="button" on:click=cancel>"Abbrechen"</button>
                            </p>
                        }
                        .into_any(),
                        (false, false) => view! {
                            <button class="action" type="button" id=CLEAR_ID on:click=ask><Icon name="trash-2"/><span>"Plan leeren"</span></button>
                        }
                        .into_any(),
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

    #[test]
    fn the_toc_lists_the_plans_semesters_the_current_and_the_shown_one() {
        let now = Some(key("2026W"));
        assert_eq!(
            toc_semesters(&[key("2027S"), key("2026S"), key("2027S")], now, key("2026W")),
            vec![(key("2026S"), false, true), (key("2026W"), true, false), (key("2027S"), false, false)]
        );
        // A semester reached with ‹ › stands there while it is shown.
        assert_eq!(toc_semesters(&[], now, key("2028S")), vec![(key("2026W"), true, false), (key("2028S"), false, false)]);
        assert_eq!(toc_semesters(&[], None, key("2026W")), vec![(key("2026W"), false, false)]);
    }

    #[test]
    fn studienbeginn_offers_the_last_years_and_the_next_semester() {
        let options = start_options(Some(key("2026W")), None);
        assert_eq!((options.first().copied(), options.last().copied(), options.len()), (Some(key("2020W")), Some(key("2027S")), 14));
        // A start stored long ago stays choosable.
        assert_eq!(start_options(Some(key("2026W")), Some(key("2015W"))).first().copied(), Some(key("2015W")));
        assert_eq!(start_options(None, Some(key("2026W"))), vec![key("2026W")]);
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
