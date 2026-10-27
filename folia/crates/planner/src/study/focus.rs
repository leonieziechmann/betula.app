//! One semester in focus (the mockup's concept A, owner 2026-10-04: „am sichersten … und am besten
//! zwischen Handy und PC"): on a desktop the semester at the top of its box, the box a list as the
//! catalog's and the Merkliste's are (owner, 2026-10-20: „dass die box wo die module drin sind vom
//! design angepasst werden an die anderen seiten"; app.css, „Mein Studium"): its head as theirs, the
//! heads of the columns across it, both staying at the top, the box as tall as the page shows, so
//! that it is the whole list once the overview has scrolled away („dass die untere box … sticky ist
//! und dann wenn man ein wenig gescrolled hat, dass es auch eine volle liste ist"). Nothing beside
//! it: „Passt in dieses Semester" went (the same day: „Das möchte ich alles über Module hinzufügen
//! später handeln"). On a phone the semester is a page of its own, which ‹ › in its head or a swipe
//! turn (`pager.rs`). Its head has ‹ › and its name, which a click turns into the list of every
//! semester to go to directly (owner, 2026-10-19: „Dazu soll man aber auch auf das semester selbst
//! klicken können, um eine selection (maybe combobox) zu bekommen"; `menu.rs`). Until then a desktop
//! had a strip of every semester over it and a line with the switch of the views over that (the
//! same day: „Das muss raus. Es verbraucht zu [viel] Platz und bietet wenig Mehrwert"): the sidebar
//! switches the views. After the last semester comes a page that adds one (owner: „Wenn man bis zum
//! Schluss ist, soll eine Page kommen, wo man dann ein neues Semester hinzufügen kann"), as a
//! semester of leave if the student says so.
//!
//! A row is a module as the catalog's list shows it (owner, 2026-10-19: „Gestalte das so, das der
//! aufbau komplett gleich ist. Also padding, margin, Titel, LP, und format von den Metadaten", and
//! on a desktop „1 zu 1 der Style vom Modulkatalog …, so dass man einfach versteht, dass es sich um
//! ein Modul handelt"): the catalog's row and its columns (`folia_widgets::list::Facts`), its area
//! where the catalog puts a module's kind, its marks as the catalog's, and at its end, where the
//! catalog has the bookmark, its status — planned, under way, passed, not passed („Die merken flag
//! soll getauscht werden mit einem Status symbol"), the button of the row's menu (`menu.rs`; a right
//! click opens it too). The whole box is the module's button („Es soll die gesamte modulbox als
//! button funktionieren"): a click anywhere on it opens the module beside the page, its name no link
//! of its own that underlines; a row of the plan that names no module leads to its modules in the
//! catalog („Modul wählen"). Marking a module as passed is done once and not by the way (owner,
//! 2026-10-04: „Das Abhaken von dem Modul ist viel zu einfach"): it is in the menu.
//!
//! Rows are selected for what is done to several at once (owner, 2026-10-04: „eine Multiselection
//! für Bearbeitung"), highlighted and hatched, with no box to tick (owner, 2026-10-19: „Die
//! selection soll nicht mehr per checkbox umgesetzt werden, sondern per hervorheben/vlt texture
//! funktionieren"): on a desktop Ctrl or ⌘ and a click selects a row, Shift and a click the rows up
//! to it, a press drawn from one row over others every row it passes, „Auswählen" in a row's menu
//! the row; on a phone a long press selects a row, and the finger, held on, every row it is drawn
//! over. While rows are selected a click on one selects it or lets it go, and a bar marks them as
//! passed, moves them, takes them out: over the rows on a desktop, in the tab bar's place on a phone
//! (owner, 2026-10-05: „Mach das mal so, dass die die navbar überdeckt"). Under the rows one more,
//! „Module hinzufügen"; an empty semester is a mark, „Noch keine Module" and „Module hinzufügen"
//! right under it (owner, 2026-10-05: „keine Module sollte mit icon und gleich modul hinzufügen
//! sein"). Nothing of what the plan still has open follows them, on a phone neither (owner,
//! 2026-10-20: „Ja, auf dem Handy auch raus"; until then a row of the catalog each with a „+"): that
//! is „Module hinzufügen"'s. The semester's own ⋯ in its head holds the rest: the Stundenplan, all
//! of its rows selected, a semester of leave.

use std::collections::BTreeSet;
use std::time::Duration;

use folia_calendar::semester::SemesterKey;
use folia_model::labels::Season;
use folia_plans::study::{self, Item, Pick, Subject, When};
use folia_plans::variants;
use folia_routes::url::CatalogUrl;
use folia_widgets::list::Facts;
use leptos::ev::{KeyboardEvent, MouseEvent, PointerEvent, TouchEvent};
use leptos::prelude::*;

use folia_design::ui::Icon;

use super::dom;
use super::menu::{self, Act, MenuFor};
use super::pager::Pages;
use super::picker::needs_text;
use super::{item_of, module_href, n, now_secs, Dialog, Plans, Ready, Selection, StudyCtx};
use crate::i18n::{self, Texts};

#[component]
pub(super) fn Focus(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let focused = Memo::new(move |_| ctx.focused());
    // The page that adds a semester: one after the last.
    let end = Memo::new(move |_| ctx.with_ready(|ready| ready.study.semesters.last().and_then(|last| last.key.plus(1))).flatten());
    // What is selected belongs to the semester in focus: turning to another lets go of it.
    Effect::new(move |_| {
        let now = focused.get();
        if ctx.selection.with_untracked(|selection| selection.semester.is_some() && selection.semester != now) {
            ctx.selection.set(Selection::default());
        }
    });
    view! {
        <section class="panel list st-sems" aria-label=s.semesters>
            {move || {
                let semester = focused.get()?;
                Some(if Some(semester) == end.get() {
                    view! { <NewSemester ctx/> }.into_any()
                } else {
                    view! { <SemesterCard ctx semester/> }.into_any()
                })
            }}
            // The list's bottom edge where the scroll area cuts it off (app.css, „one scroll area").
            <i class="list-cap" aria-hidden="true"></i>
        </section>
    }
}

/// What a semester holds as the list of the semesters says it (`menu.rs`): „24 von 32 bestanden" of
/// one that is over, „32 von 30 LP" of the others (a warning where it is more than the plan puts
/// in), „noch leer · Plan 30"; with the class of how it reads.
pub(super) fn figure(ready: &Ready, key: SemesterKey, t: &Texts) -> Option<(String, &'static str)> {
    let s = &t.study;
    let semester = ready.study.semester(key)?;
    let planned = semester.planned;
    Some(match semester.when {
        When::Past => (format!("{} {}", n(semester.passed, t), (s.of_passed)(&n(semester.credits, t))), "ok"),
        _ if semester.items.is_empty() => (planned.map_or_else(|| s.still_empty.to_string(), |plan| format!("{} · {}", s.still_empty, (s.plan_n)(&n(plan, t)))), ""),
        _ => {
            let heavy = planned.is_some_and(|plan| semester.credits > plan + 0.5);
            let figure = planned.map_or_else(|| (s.credits)(&n(semester.credits, t)), |plan| format!("{} {}", n(semester.credits, t), (s.of_planned)(&n(plan, t))));
            (figure, if heavy { "warn" } else { "" })
        }
    })
}

/// What the head of a semester says.
#[derive(Clone, Debug, PartialEq)]
struct Head {
    label: String,
    now: bool,
    beyond: bool,
    sub: String,
    figure: Option<(String, String, bool)>,
    previous: Option<SemesterKey>,
    next: Option<SemesterKey>,
    keys: Vec<String>,
    past: bool,
}

impl Head {
    fn of(ready: &Ready, key: SemesterKey, t: &Texts) -> Option<Self> {
        let s = &t.study;
        let study = &ready.study;
        let at = study.semesters.iter().position(|semester| semester.key == key)?;
        let semester = study.semesters.get(at)?;
        let sub = match semester.fs {
            Some(fs) => (s.fs_long)(fs),
            None if semester.leave => s.leave.to_string(),
            None => String::new(),
        };
        let figure = match semester.when {
            When::Past => Some(((s.head_past)(&n(semester.passed, t), &n(semester.credits, t)), String::new(), false)),
            _ => {
                let heavy = semester.planned.is_some_and(|plan| semester.credits > plan + 0.5);
                let mut rest = Vec::new();
                if let Some(plan) = semester.planned {
                    rest.push((s.plan_n)(&n(plan, t)));
                    if heavy {
                        rest.push((s.more)(&n(semester.credits - plan, t)));
                    }
                }
                (semester.credits > 0.0 || semester.planned.is_some()).then(|| ((s.credits)(&n(semester.credits, t)), rest.join(" · "), heavy))
            }
        };
        Some(Head {
            label: key.label(t.locale),
            now: semester.when == When::Now,
            beyond: semester.beyond,
            sub,
            figure,
            previous: at.checked_sub(1).and_then(|before| study.semesters.get(before)).map(|semester| semester.key),
            next: study.semesters.get(at + 1).map(|semester| semester.key).or_else(|| key.plus(1)),
            keys: semester.items.iter().map(Item::key).collect(),
            past: semester.when == When::Past,
        })
    }
}

/// Turns to semester `to`: on a phone the row of pages scrolls there (`pager::Pages`), else at once.
fn turn(ctx: StudyCtx, pages: Option<Pages>, to: Option<SemesterKey>) {
    match (to, pages) {
        (Some(to), Some(pages)) => pages.turn_to(to),
        (Some(to), None) => ctx.focus.set(Some(to)),
        (None, _) => {}
    }
}

/// The name of the semester in a head: a button that opens the list of every semester, to go to
/// one directly (`MenuFor::Pick`). `id` is the heading's, which the page names itself by.
#[component]
fn SemesterPick(ctx: StudyCtx, semester: SemesterKey, id: String, label: String) -> impl IntoView {
    let t = i18n::t();
    let open = Memo::new(move |_| ctx.menu.with(|menu| menu.as_ref().is_some_and(|menu| menu.what == MenuFor::Pick(semester))));
    view! {
        <h2 id=id>
            <button
                class="st-sem-pick"
                type="button"
                aria-haspopup="menu"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                title=t.study.choose_semester
                on:click=move |ev: MouseEvent| menu::open_under_button(ctx, MenuFor::Pick(semester), &ev)
            >
                <span>{label}</span>
                <Icon name="chevron-down"/>
            </button>
        </h2>
    }
}

/// The semester in focus: its head with ‹ ›, its name and its ⋯, the heads of the columns (or the
/// bar of the rows selected), its rows, and „Module hinzufügen" as the last row.
#[component]
pub(super) fn SemesterCard(ctx: StudyCtx, semester: SemesterKey) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let head = Memo::new(move |_| ctx.with_ready(|ready| Head::of(ready, semester, t)).flatten());
    let keys = Memo::new(move |_| head.with(|head| head.as_ref().map(|head| head.keys.clone()).unwrap_or_default()));
    // The rows selected here, in their order. A row that goes is let go of.
    let chosen = Memo::new(move |_| {
        ctx.selection.with(|selection| match selection.semester == Some(semester) {
            true => keys.with(|keys| keys.iter().filter(|key| selection.keys.contains(*key)).cloned().collect::<Vec<_>>()),
            false => Vec::new(),
        })
    });
    Effect::new(move |_| {
        let keys = keys.get();
        ctx.selection.maybe_update(|selection| {
            let before = selection.keys.len();
            if selection.semester == Some(semester) {
                selection.keys.retain(|key| keys.contains(key));
            }
            selection.keys.len() != before
        });
    });
    let selecting = Memo::new(move |_| !chosen.with(Vec::is_empty));
    // The modules only visited in it, after its rows (`VisitRow`), by their numbers.
    let visits = Memo::new(move |_| ctx.with_ready(|ready| ready.study.semester(semester).map(|semester| semester.visits.iter().map(|visit| visit.id.clone()).collect::<Vec<_>>())).flatten().unwrap_or_default());
    let empty = Memo::new(move |_| keys.with(Vec::is_empty) && visits.with(Vec::is_empty));
    let add = move || ctx.open(Dialog::Add { semester, catalog: false, chosen: Vec::new() });
    let pages = use_context::<Pages>();
    let go = move |to: Option<SemesterKey>| turn(ctx, pages, to);
    let c = &t.catalog;
    // A press on a row, and the selection it draws over the others (`Draw`).
    let draw = Draw::new();
    // While a finger draws a selection the page does not scroll under it: the listener is the
    // list's own (the app is built without Leptos's delegation of events), so the browser lets it
    // cancel the move.
    let touch_move = move |ev: TouchEvent| {
        if draw.painting() && ev.cancelable() {
            ev.prevent_default();
        }
    };
    view! {
        <article class="st-card-sem" class:is-now=move || head.with(|head| head.as_ref().is_some_and(|head| head.now)) aria-labelledby=format!("st-sem-{}", semester.key())>
            {move || head.get().map(|head| view! {
                <header class="st-sem-head">
                    <button class="icon-btn st-turn hit" type="button" aria-label=s.previous disabled=head.previous.is_none() on:click=move |_| go(head.previous)><Icon name="chevron-left"/></button>
                    <div class="st-sem-title">
                        <div class="st-sem-name">
                            <SemesterPick ctx semester id=format!("st-sem-{}", semester.key()) label=head.label.clone()/>
                            {head.now.then(|| view! { <span class="st-now">{s.now}</span> })}
                            {head.beyond.then(|| view! { <span class="st-badge warn">{s.beyond}</span> })}
                            <span class="st-sem-fs st-pc">{head.sub.clone()}</span>
                        </div>
                        <p class="st-sem-sub">
                            <span class="st-phone">{head.sub.clone()}{head.figure.is_some().then_some(" · ")}</span>
                            {head.figure.clone().map(|(figure, rest, heavy)| view! {
                                <b class:heavy=heavy class:ok=head.past>{figure}</b>
                                {(!rest.is_empty()).then(|| format!(" · {rest}"))}
                            })}
                        </p>
                    </div>
                    <button
                        class="icon-btn st-sem-more hit"
                        type="button"
                        aria-haspopup="menu"
                        aria-label=(s.more_about)(&head.label)
                        title=(s.more_about)(&head.label)
                        on:click=move |ev: MouseEvent| menu::open_at_button(ctx, MenuFor::Semester(semester), &ev)
                    >
                        <Icon name="ellipsis"/>
                    </button>
                    <button class="icon-btn st-turn hit" type="button" aria-label=s.next disabled=head.next.is_none() on:click=move |_| go(head.next)><Icon name="chevron-right"/></button>
                </header>
            })}
            <div class="st-table" class:is-selecting=selecting>
                // The heads of the catalog's columns, and in their place the bar of the rows
                // selected: as tall, so that no row moves when it comes.
                {move || (!empty.get()).then(|| if selecting.get() {
                    view! { <SelectionBar ctx semester keys chosen/> }.into_any()
                } else {
                    view! {
                        <div class="cols label st-cols">
                            <span>{c.col_module}</span>
                            <span class="c-resp">{c.col_responsible}</span>
                            <span class="c-exam">{c.exam}</span>
                            <span class="c-lp">{t.common.credits_unit}</span>
                            <span class="c-turnus">{c.turnus}</span>
                            <span class="c-lang">{c.col_language}</span>
                            <span class="c-events">{c.dates}</span>
                        </div>
                    }
                    .into_any()
                })}
                <ul
                    class="st-rows"
                    on:pointermove=move |ev: PointerEvent| draw.moved(ctx, semester, keys, &ev)
                    on:pointerup=move |_| draw.end()
                    on:pointercancel=move |_| draw.end()
                    on:touchmove=touch_move
                >
                    <For each=move || keys.get() key=|key| key.clone() children=move |key: String| view! { <ItemRow ctx semester key order=keys draw/> }/>
                    <For each=move || visits.get() key=|id| id.clone() children=move |id: String| view! { <VisitRow ctx semester id/> }/>
                    {move || if empty.get() {
                        view! {
                            <li class="st-empty">
                                <span class="st-empty-icon" aria-hidden="true"><Icon name="layout-list"/></span>
                                <p>{s.no_modules}</p>
                                <button class="btn primary st-empty-add" type="button" on:click=move |_| add()>
                                    <Icon name="plus"/><span>{s.add_modules}</span>
                                </button>
                            </li>
                        }
                        .into_any()
                    } else {
                        view! {
                            <li class="st-addrow">
                                <button class="st-add" type="button" on:click=move |_| add()>
                                    <span class="st-add-icon" aria-hidden="true"><Icon name="plus"/></span>
                                    <span>{s.add_modules}</span>
                                </button>
                            </li>
                        }
                        .into_any()
                    }}
                </ul>
            </div>
        </article>
    }
}

/// The bar of the rows selected (owner, 2026-10-04: „Ich finde die Optionen als bestanden
/// markieren und Löschen als sinnvoll"): how many and their credits, „Alle auswählen" while some
/// are not; „Als bestanden markieren" (or taking that back where all of them are), „Verschieben
/// nach", „Entfernen"; and letting go of them. Over the rows on a desktop, in the tab bar's place on
/// a phone.
#[component]
fn SelectionBar(ctx: StudyCtx, semester: SemesterKey, keys: Memo<Vec<String>>, chosen: Memo<Vec<String>>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    // How many, their credits, whether all of them are passed.
    let summary = Memo::new(move |_| {
        let chosen = chosen.get();
        ctx.with_ready(|ready| {
            let items: Vec<Item> = chosen.iter().filter_map(|key| item_of(ready, semester, key)).collect();
            let credits: f64 = items.iter().filter_map(|item| item.credits).sum();
            (items.len(), credits, !items.is_empty() && items.iter().all(|item| item.passed))
        })
        .unwrap_or((0, 0.0, false))
    });
    let all_passed = Memo::new(move |_| summary.get().2);
    let some_left = Memo::new(move |_| chosen.with(Vec::len) < keys.with(Vec::len));
    let passed = move |_| menu::act(ctx, Act::Passed { semester, keys: chosen.get_untracked(), passed: !all_passed.get_untracked() }, t);
    let remove = move |_| menu::act(ctx, Act::Remove { semester, keys: chosen.get_untracked() }, t);
    view! {
        // Its words in full where the table has room for them, short where it has not (a narrow
        // window, a phone).
        <div class="st-selbar" role="toolbar" aria-label=s.selection>
            <span class="st-selbar-count" aria-live="polite">
                {move || (s.n_selected)(summary.get().0)}
                <span class="st-long">{move || format!(" · {}", (s.credits)(&n(summary.get().1, t)))}</span>
            </span>
            <button class="st-act st-pc" type="button" id="st-sel-all" title=s.select_all aria-label=s.select_all disabled=move || !some_left.get() on:click=move |_| menu::act(ctx, Act::SelectAll(semester), t)>
                <Icon name="list-checks"/>
                <span class="st-long">{s.select_all}</span>
            </button>
            <span class="st-grow"></span>
            <button class="st-act" type="button" id="st-sel-passed" on:click=passed>
                {move || if all_passed.get() { view! { <Icon name="rotate-ccw"/> }.into_any() } else { view! { <Icon name="circle-check-big"/> }.into_any() }}
                <span class="st-long">{move || if all_passed.get() { s.unmark_passed } else { s.mark_passed }}</span>
                <span class="st-short">{move || if all_passed.get() { s.unmark_short } else { s.act_passed }}</span>
            </button>
            <button class="st-act" type="button" id="st-sel-move" aria-haspopup="menu" on:click=move |ev: MouseEvent| menu::open_at_button(ctx, MenuFor::Move(semester), &ev)>
                <Icon name="arrow-right-left"/>
                <span class="st-long">{s.move_to}</span>
                <span class="st-short">{s.act_move}</span>
                <Icon name="chevron-down" class="st-pc"/>
            </button>
            <button class="st-act danger" type="button" id="st-sel-remove" on:click=remove>
                <Icon name="trash-2"/>
                <span>{s.remove}</span>
            </button>
            <button class="icon-btn st-act-close hit" type="button" aria-label=s.clear_selection title=s.clear_selection on:click=move |_| ctx.selection.set(Selection::default())>
                <Icon name="x"/>
            </button>
        </div>
    }
}

/// Where a module of a semester stands (owner, 2026-10-19: „Status symbol (In
/// progress/bestanden/nicht bestanden)"): planned for a semester to come, under way in the current
/// one, passed, not passed in one that is over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Status {
    Planned,
    Running,
    Passed,
    Failed,
}

impl Status {
    fn of(item: &Item, when: When) -> Self {
        match (item.passed, item.failed, when) {
            (true, ..) => Status::Passed,
            (_, true, _) | (_, _, When::Past) => Status::Failed,
            (_, _, When::Now) => Status::Running,
            (_, _, When::Later) => Status::Planned,
        }
    }

    /// Its class (app.css `.st-glyph`).
    pub fn class(self) -> &'static str {
        match self {
            Status::Planned => "planned",
            Status::Running => "running",
            Status::Passed => "passed",
            Status::Failed => "failed",
        }
    }

    /// Its word: „Geplant", „Läuft", „Bestanden", „Nicht bestanden".
    pub fn word(self, t: &Texts) -> &'static str {
        let s = &t.study;
        match self {
            Status::Planned => s.status_planned,
            Status::Running => s.status_running,
            Status::Passed => s.status_passed,
            Status::Failed => s.status_failed,
        }
    }
}

/// The mark of a status: a ring, a ring half filled, a check on green, a cross (app.css).
#[component]
pub(super) fn Glyph(#[prop(into)] status: Signal<Status>) -> impl IntoView {
    view! {
        <span class=move || format!("st-glyph {}", status.get().class()) aria-hidden="true">
            <Icon name="check" class="g-passed"/>
            <Icon name="x" class="g-failed"/>
        </span>
    }
}

/// What a row shows besides its name.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    item: Item,
    status: Status,
    area: String,
    tone: &'static str,
    credits: Option<String>,
    /// The marks, as the catalog's (`.flag`): their class and words.
    flags: Vec<(&'static str, String)>,
    /// A module not passed here and not planned again: where „Einplanen" puts it.
    again: Option<SemesterKey>,
    /// The catalog of a row's modules.
    find: Option<String>,
    /// The module as the catalog's list says it: its columns.
    facts: Facts,
}

/// The season of a semester as the marks say it: „WiSe", „SoSe".
fn season_word(s: SemesterKey, t: &Texts) -> &'static str {
    if s.winter { t.format.winter_short } else { t.format.summer_short }
}

impl Row {
    fn of(ctx: StudyCtx, ready: &Ready, semester: SemesterKey, item: Item, t: &Texts) -> Self {
        let s = &t.study;
        let study = &ready.study;
        let mut flags = Vec::new();
        if let Some(before) = item.retake {
            let from = ready.fs_label(before, t).unwrap_or_else(|| before.short(t.locale));
            flags.push(("flag neutral", (s.chip_retake)(&from)));
        }
        if item.unoffered && !item.passed {
            let only = match item.offer.only() {
                Some(Season::Winter) => format!(" · {}", s.only_winter),
                Some(Season::Summer) => format!(" · {}", s.only_summer),
                None => String::new(),
            };
            flags.push(("flag", format!("{}{only}", (s.unoffered)(season_word(semester, t)))));
        }
        // What the module asks for and is not in place: said here, as the only place it was before
        // (the row's dialog) is gone.
        if let (Some(id), false) = (item.module_id(), item.passed) {
            let doc = ctx.plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
            if let Some((text, true)) = ctx.needs.with(|needs| needs_text(&doc, needs, id, &item.name, semester, study.now, t)) {
                flags.push(("flag long", text));
            }
        }
        if item.over > 0.0 {
            let area = ready.area_name(item.area, t);
            let required = item.area.and_then(|area| study.progress.get(area)).map_or(0.0, |progress| progress.area.required);
            flags.push(("flag neutral", (s.over_full)(&area, &n(required, t))));
        }
        if let Subject::Row { .. } = item.subject {
            if item.fillers.is_empty() {
                flags.push(("flag neutral", s.placeholder.to_string()));
            } else {
                let names: Vec<String> = ctx.rows.with_untracked(|rows| item.fillers.iter().map(|id| rows.iter().find(|row| row.id == *id).map_or_else(|| id.clone(), |row| row.title.clone())).collect());
                flags.push(("flag neutral", (s.filled_with)(&names.join(", "))));
            }
        }
        let mut again = None;
        if item.failed {
            match item.again {
                Some(later) => flags.push(("st-quiet", (s.failed_again)(&later.label(t.locale)))),
                None => {
                    flags.push(("st-quiet warn", s.failed_open.to_string()));
                    let next = item.offer.next(study.now);
                    again = Some(if study.semester(next).is_some() { next } else { study.now });
                }
            }
        }
        let credits = match &item.subject {
            Subject::Row { credits: Some(text), .. } => Some(folia_plans::plan::credits_in(text, t.locale)),
            _ => item.credits.map(|credits| n(credits, t)),
        };
        let find = match &item.subject {
            Subject::Row { caption, ord, .. } if item.fillers.is_empty() => find_href(ctx, caption, *ord, t),
            _ => None,
        };
        let facts = Facts::of(item.module_id().and_then(|id| ctx.rows.with_untracked(|rows| rows.iter().find(|row| row.id == id).cloned())).as_ref(), t.locale);
        let when = study.semester(semester).map_or(When::Later, |it| it.when);
        Row { status: Status::of(&item, when), area: ready.area_name(item.area, t), tone: ready.tone(item.area), credits, flags, again, find, facts, item }
    }
}

/// The catalog of a plan row's modules (`variants::row_query`).
pub(super) fn find_href(ctx: StudyCtx, caption: &str, ord: i64, t: &Texts) -> Option<String> {
    ctx.plans.with_untracked(|plans| {
        let Plans::Found(source) = plans else { return None };
        let variant = source.variants.iter().find(|variant| variant.full == caption)?;
        let entry = variant.entries.iter().find(|entry| entry.ord == ord)?;
        let (query, _) = variants::row_query(&source.program.slug, variant, entry, &source.areas, t.locale);
        Some(CatalogUrl { query, page: 1, open: None, fill: None }.path())
    })
}

/// A selection drawn over the rows: the row it began at, whether it selects or lets go, what was
/// selected before it.
#[derive(Clone, Debug, PartialEq)]
struct Paint {
    start: usize,
    select: bool,
    base: BTreeSet<String>,
}

/// What is selected while a selection drawn over the rows is at row `at`: the rows between it and
/// where it began selected (or let go of), the others as they were before.
fn painted(order: &[String], paint: &Paint, at: usize) -> BTreeSet<String> {
    let (from, to) = (paint.start.min(at), paint.start.max(at));
    let mut keys = paint.base.clone();
    for key in order.get(from..=to).unwrap_or_default() {
        if paint.select {
            keys.insert(key.clone());
        } else {
            keys.remove(key);
        }
    }
    keys
}

/// Selects the row `key` of semester `s`, or lets it go.
fn toggle(ctx: StudyCtx, s: SemesterKey, key: &str) {
    ctx.selection.update(|selection| {
        if selection.semester != Some(s) {
            *selection = Selection { semester: Some(s), ..Selection::default() };
        }
        if !selection.keys.remove(key) {
            selection.keys.insert(key.to_string());
        }
        selection.anchor = Some(key.to_string());
    });
}

/// Selects the rows from the last one clicked to `key` (Shift), besides those selected.
fn extend(ctx: StudyCtx, s: SemesterKey, order: &[String], key: &str) {
    let Some(at) = order.iter().position(|other| other == key) else { return };
    ctx.selection.update(|selection| {
        if selection.semester != Some(s) {
            *selection = Selection { semester: Some(s), ..Selection::default() };
        }
        let from = selection.anchor.as_ref().and_then(|anchor| order.iter().position(|other| other == anchor)).unwrap_or(at);
        selection.keys.extend(order.get(from.min(at)..=from.max(at)).unwrap_or_default().iter().cloned());
        selection.anchor.get_or_insert_with(|| key.to_string());
    });
}

/// Shift and ↑ or ↓ at row `key` (`by`, ±1; oberflaeche.html of the Merkliste: „Neu, für beide
/// Listen"): the row it goes to, taken in going away from where the range began (`anchor`), the
/// row left let go of coming back towards it.
fn step(ctx: StudyCtx, s: SemesterKey, order: &[String], key: &str, by: isize) -> Option<String> {
    let here = order.iter().position(|other| other == key)?;
    let next = here.checked_add_signed(by).filter(|next| *next < order.len())?;
    let to = order.get(next)?.clone();
    ctx.selection.update(|selection| {
        if selection.semester != Some(s) {
            *selection = Selection { semester: Some(s), ..Selection::default() };
        }
        if !selection.keys.contains(key) || selection.anchor.as_ref().is_none_or(|anchor| !selection.keys.contains(anchor)) {
            selection.keys.insert(key.to_string());
            selection.anchor = Some(key.to_string());
        }
        let anchor = selection.anchor.as_ref().and_then(|anchor| order.iter().position(|other| other == anchor)).unwrap_or(here);
        if next.abs_diff(anchor) > here.abs_diff(anchor) {
            selection.keys.insert(to.clone());
        } else {
            selection.keys.remove(key);
        }
    });
    Some(to)
}

/// How long a finger is held on a row before it selects it.
const HOLD: Duration = Duration::from_millis(450);

/// A press on the rows of a semester and the selection it draws: a finger held on a row a while
/// selects it (a phone's way to begin), and held on, every row it is drawn over; a mouse pressed on
/// one row and drawn onto another does the same at once. What it draws begins at the row pressed
/// and selects, or, begun on a selected row, lets go. The row hears the press (`ItemRow`), the list
/// the moves (`SemesterCard`): a mouse's go to the row under it, not to the one pressed.
#[derive(Clone, Copy)]
struct Draw {
    /// The row pressed (`Item::key`), where, and whether by a finger.
    press: StoredValue<Option<(String, i32, i32, bool)>>,
    /// A finger's press until it was held long enough.
    hold: StoredValue<Option<TimeoutHandle>>,
    paint: StoredValue<Option<Paint>>,
    /// The click that ends a long press or a drawn selection is none.
    drawn: StoredValue<bool>,
}

impl Draw {
    fn new() -> Self {
        Draw { press: StoredValue::new(None), hold: StoredValue::new(None), paint: StoredValue::new(None), drawn: StoredValue::new(false) }
    }

    /// The press is over, or was none.
    fn end(self) {
        if let Some(handle) = self.hold.get_value() {
            handle.clear();
        }
        self.hold.set_value(None);
        self.press.set_value(None);
        self.paint.set_value(None);
    }

    /// A press on row `key` of semester `s` at (`x`, `y`). A finger's selects the row once it was
    /// held long enough.
    fn begin(self, ctx: StudyCtx, s: SemesterKey, order: Memo<Vec<String>>, key: String, x: i32, y: i32, touch: bool) {
        self.end();
        self.drawn.set_value(false);
        self.press.set_value(Some((key, x, y, touch)));
        if touch {
            let held = set_timeout_with_handle(
                move || {
                    self.hold.set_value(None);
                    self.paint_from(ctx, s, &order.get_untracked());
                },
                HOLD,
            );
            self.hold.set_value(held.ok());
        }
    }

    /// The selection drawn from the row pressed begins: that row selected, or let go of.
    fn paint_from(self, ctx: StudyCtx, s: SemesterKey, order: &[String]) {
        let Some((key, ..)) = self.press.get_value() else { return };
        let Some(at) = order.iter().position(|other| *other == key) else { return };
        let base = ctx.selection.with_untracked(|selection| if selection.semester == Some(s) { selection.keys.clone() } else { BTreeSet::new() });
        let start = Paint { start: at, select: !base.contains(&key), base };
        ctx.selection.set(Selection { semester: Some(s), keys: painted(order, &start, at), anchor: Some(key) });
        self.paint.set_value(Some(start));
        self.drawn.set_value(true);
    }

    /// The pointer moved to (`x`, `y`) with the press on: a finger that moves before it was held
    /// long enough scrolls the page; a mouse drawn onto another row begins a selection; one under
    /// way takes in the rows up to where the pointer is.
    fn moved(self, ctx: StudyCtx, s: SemesterKey, order: Memo<Vec<String>>, ev: &PointerEvent) {
        let Some((pressed, x0, y0, touch)) = self.press.get_value() else { return };
        let (x, y) = (ev.client_x(), ev.client_y());
        if self.hold.with_value(Option::is_some) {
            if (x - x0).abs() > 10 || (y - y0).abs() > 10 {
                self.end();
            }
            return;
        }
        let (x, y) = (f64::from(x), f64::from(y));
        if !touch && self.paint.with_value(Option::is_none) && ev.buttons() & 1 == 1 && dom::row_at(x, y).is_some_and(|over| over != pressed) {
            dom::capture(ev);
            self.paint_from(ctx, s, &order.get_untracked());
        }
        if self.paint.with_value(Option::is_none) {
            return;
        }
        let Some(over) = dom::row_at(x, y).or_else(|| dom::row_nearest(y)) else { return };
        let order = order.get_untracked();
        let Some(at) = order.iter().position(|other| *other == over) else { return };
        let keys = self.paint.with_value(|paint| paint.as_ref().map(|paint| painted(&order, paint, at)));
        if let Some(keys) = keys.filter(|keys| ctx.selection.with_untracked(|selection| selection.keys != *keys)) {
            ctx.selection.update(|selection| selection.keys = keys);
        }
    }

    /// Whether a selection is being drawn.
    fn painting(self) -> bool {
        self.paint.with_value(Option::is_some)
    }

    /// Whether the click now ends a long press or a drawn selection; it is the last such.
    fn swallows(self) -> bool {
        let drawn = self.drawn.get_value();
        self.drawn.set_value(false);
        drawn
    }
}

/// One module or row of the plan in a semester, as the catalog's list has a module: the box, which
/// opens it, and its status at its end, which opens its menu.
#[component]
fn ItemRow(ctx: StudyCtx, semester: SemesterKey, key: String, order: Memo<Vec<String>>, draw: Draw) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let row = {
        let key = key.clone();
        Memo::new(move |_| ctx.with_ready(|ready| item_of(ready, semester, &key).map(|item| Row::of(ctx, ready, semester, item, t))).flatten())
    };
    // What a row is stays what it is: its subject and name.
    let Some(first) = row.get_untracked().map(|row| row.item) else { return ().into_any() };
    let key = StoredValue::new(key);
    let selected = Memo::new(move |_| key.with_value(|key| ctx.selection.with(|selection| selection.has(semester, key))));
    let selecting = Memo::new(move |_| ctx.selection.with(|selection| selection.any_in(semester)));
    let status = Memo::new(move |_| row.with(|row| row.as_ref().map_or(Status::Planned, |row| row.status)));

    // A press on the row: what it draws is the list's (`Draw`). The status, „Modul wählen" and
    // „Einplanen" do what they say.
    let touched = StoredValue::new(false);
    let down = move |ev: PointerEvent| {
        let touch = ev.pointer_type() == "touch";
        touched.set_value(touch);
        if ev.button() != 0 || ev.shift_key() || ev.ctrl_key() || ev.meta_key() || (dom::on_control(&ev) && !dom::on_open(&ev)) {
            draw.end();
            return;
        }
        draw.begin(ctx, semester, order, key.get_value(), ev.client_x(), ev.client_y(), touch);
    };
    // A click opens the module (the box is its link); while rows are selected, or with Ctrl or ⌘,
    // it selects the row or lets it go, with Shift the rows up to it.
    let click = move |ev: MouseEvent| {
        // Not one of the keys (Enter): those come without a press.
        if ev.detail() != 0 && draw.swallows() {
            ev.prevent_default();
            return;
        }
        if dom::on_control(&ev) && !dom::on_open(&ev) {
            return;
        }
        if ev.shift_key() {
            ev.prevent_default();
            extend(ctx, semester, &order.get_untracked(), &key.get_value());
        } else if ev.ctrl_key() || ev.meta_key() || selecting.get_untracked() {
            ev.prevent_default();
            toggle(ctx, semester, &key.get_value());
        }
    };
    // The space bar on the box selects the row or lets it go (Enter opens it, or does what a click
    // does while rows are selected); Shift and ↑ ↓ take in the next row (`step`).
    let keys = move |ev: KeyboardEvent| {
        if !dom::on_open(&ev) || ev.ctrl_key() || ev.meta_key() || ev.alt_key() {
            return;
        }
        let by = match ev.key().as_str() {
            " " if !ev.shift_key() => {
                ev.prevent_default();
                toggle(ctx, semester, &key.get_value());
                return;
            }
            "ArrowDown" if ev.shift_key() => 1,
            "ArrowUp" if ev.shift_key() => -1,
            _ => return,
        };
        ev.prevent_default();
        if let Some(to) = step(ctx, semester, &order.get_untracked(), &key.get_value(), by) {
            dom::focus_row(&to);
        }
    };
    // A right click opens the row's menu where the pointer is; a long press of a finger does not.
    let context = move |ev: MouseEvent| {
        ev.prevent_default();
        if !touched.get_value() {
            menu::open_at_pointer(ctx, MenuFor::Item { semester, key: key.get_value(), view: false }, &ev);
        }
    };
    let menu_open = Memo::new(move |_| ctx.menu.with(|menu| menu.as_ref().is_some_and(|menu| matches!(&menu.what, MenuFor::Item { semester: there, key: of, .. } if *there == semester && key.with_value(|key| key == of)))));

    // Its name may come after it is drawn: on a page loaded afresh the catalog's rows of a module
    // outside the program come later.
    let name = {
        let first = first.name.clone();
        Memo::new(move |_| row.with(|row| row.as_ref().map(|row| row.item.name.clone())).unwrap_or_else(|| first.clone()))
    };
    let code = first.module_id().map(str::to_string);
    // The box's link: the module beside the page, or a row's modules in the catalog.
    let current = {
        let id = code.clone();
        Memo::new(move |_| id.as_ref().is_some_and(|id| ctx.url.with(|url| url.open.as_deref() == Some(id.as_str()))))
    };
    let opens = code.is_some() || row.with_untracked(|row| row.as_ref().is_some_and(|row| row.find.is_some()));
    let title = match code.clone() {
        Some(id) => view! {
            <a class="st-name st-open" href=move || module_href(ctx, &id, t) data-noscroll="" draggable="false" aria-current=move || current.get().then_some("true")>
                {move || name.get()}
                {move || selected.get().then(|| view! { <span class="visually-hidden">{format!(" ({})", s.is_selected)}</span> })}
            </a>
        }
        .into_any(),
        None => view! { <span class="st-name">{move || name.get()}</span> }.into_any(),
    };
    view! {
        <li
            class="st-row row-wrap"
            data-key=key.get_value()
            style=move || row.with(|row| row.as_ref().map(|row| format!("--c: {}", row.tone)))
            class:is-passed=move || status.get() == Status::Passed
            class:is-failed=move || status.get() == Status::Failed
            class:is-selected=move || selected.get()
            on:pointerdown=down
            on:click=click
            on:keydown=keys
            on:contextmenu=context
        >
            <div class="row" class:st-inert=!opens aria-current=move || current.get().then_some("true")>
                <div class="t">
                    <b>{title}</b>
                    // The line under the name, as the catalog's: its number, its area where the
                    // catalog has a module's kind, its marks; where the list is narrow its
                    // language and its events.
                    <small>
                        {code.clone().map(|code| view! { <span class="mono">{code}</span> })}
                        {move || row.get().map(|row| {
                            let item_key = row.item.key();
                            view! {
                                <span class="kind st-area"><i style=format!("background: {}", row.tone)></i>{row.area.clone()}</span>
                                {row.flags.into_iter().map(|(class, text)| view! { <span class=class>{text}</span> }).collect_view()}
                                {row.find.map(|href| view! { <a class="st-find st-open" href=t.path(&href) rel="nofollow" draggable="false"><Icon name="search"/>{s.choose_module}</a> })}
                                {row.again.map(|to| view! {
                                    <button class="st-link" type="button" on:click=move |_| { ctx.focus.set(Some(to)); ctx.open(Dialog::Add { semester: to, catalog: false, chosen: vec![item_key.clone()] }); }>{s.plan_it}</button>
                                })}
                                {row.facts.narrow()}
                            }
                        })}
                    </small>
                </div>
                {move || row.with(|row| row.as_ref().map(|row| row.facts.lead()))}
                <span class="lp num">{move || row.with(|row| row.as_ref().and_then(|row| row.credits.clone()))}<small>{t.common.credits_unit}</small></span>
                {move || row.with(|row| row.as_ref().map(|row| row.facts.trail()))}
            </div>
            <button
                class="st-status"
                type="button"
                aria-haspopup="menu"
                aria-expanded=move || if menu_open.get() { "true" } else { "false" }
                aria-label=move || (s.status_of)(status.get().word(t), &name.get())
                title=move || status.get().word(t)
                on:click=move |ev: MouseEvent| menu::open_at_button(ctx, MenuFor::Item { semester, key: key.get_value(), view: false }, &ev)
            >
                <Glyph status=status/>
            </button>
        </li>
    }
    .into_any()
}

/// A module only visited in the semester (`study::VisitItem`; folia/design/kalender/konzepte.html,
/// „Ein Besuchsmodul in „Mein Studium“": „auf „nur besuchen“: mit eigenem Zeichen, ohne LP,
/// außerhalb der Bereiche"): a row as the others are, its name the module's link, under it its
/// number and „nur besuchen · aus dem Kalender", „–" for its credits, and at its end, where the
/// others have their status, the sign of a visit: the button of its menu (`MenuFor::Visit`; a
/// right click opens it too). It is none of the study's rows, so it is not selected with them.
#[component]
fn VisitRow(ctx: StudyCtx, semester: SemesterKey, id: String) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    // What it is and the catalog's facts of it may come after it is drawn: on a page loaded afresh
    // the catalog's rows of a module outside the program come later.
    let visit = {
        let id = id.clone();
        Memo::new(move |_| ctx.with_ready(|ready| ready.study.semester(semester).and_then(|semester| semester.visits.iter().find(|visit| visit.id == id).cloned())).flatten())
    };
    let facts = {
        let id = id.clone();
        Memo::new(move |_| ctx.rows.with(|rows| Facts::of(rows.iter().find(|row| row.id == id), t.locale)))
    };
    let name = {
        let id = id.clone();
        Memo::new(move |_| visit.with(|visit| visit.as_ref().map(|visit| visit.name.clone())).unwrap_or_else(|| id.clone()))
    };
    let unoffered = Memo::new(move |_| visit.with(|visit| visit.as_ref().is_some_and(|visit| visit.unoffered)));
    let current = {
        let id = id.clone();
        Memo::new(move |_| ctx.url.with(|url| url.open.as_deref() == Some(id.as_str())))
    };
    let menu_open = {
        let id = id.clone();
        Memo::new(move |_| ctx.menu.with(|menu| menu.as_ref().is_some_and(|menu| matches!(&menu.what, MenuFor::Visit { semester: there, id: of } if *there == semester && *of == id))))
    };
    let href = {
        let id = id.clone();
        move || module_href(ctx, &id, t)
    };
    let open_menu = {
        let id = id.clone();
        move |ev: MouseEvent| menu::open_at_button(ctx, MenuFor::Visit { semester, id: id.clone() }, &ev)
    };
    let context = {
        let id = id.clone();
        move |ev: MouseEvent| {
            ev.prevent_default();
            menu::open_at_pointer(ctx, MenuFor::Visit { semester, id: id.clone() }, &ev);
        }
    };
    view! {
        <li class="st-row row-wrap st-visit" data-key=format!("v:{id}") on:contextmenu=context>
            <div class="row" aria-current=move || current.get().then_some("true")>
                <div class="t">
                    <b>
                        <a class="st-name st-open" href=href data-noscroll="" draggable="false" aria-current=move || current.get().then_some("true")>{move || name.get()}</a>
                    </b>
                    <small>
                        <span class="mono">{id.clone()}</span>
                        <span class="flag neutral st-visit-line">{s.visit_line}</span>
                        {move || unoffered.get().then(|| view! { <span class="flag">{(s.unoffered)(season_word(semester, t))}</span> })}
                        {move || facts.with(|facts| facts.narrow())}
                    </small>
                </div>
                {move || facts.with(|facts| facts.lead())}
                <span class="lp num">"–"</span>
                {move || facts.with(|facts| facts.trail())}
            </div>
            <button class="st-status" type="button" aria-haspopup="menu" aria-expanded=move || if menu_open.get() { "true" } else { "false" } aria-label=move || (s.status_of)(s.status_visit, &name.get()) title=s.status_visit on:click=open_menu>
                <span class="st-glyph visit" aria-hidden="true"></span>
            </button>
        </li>
    }
}

/// Adds `pick` to semester `s` at once, with a note to take it back.
pub(super) fn add_pick(ctx: StudyCtx, s: SemesterKey, picks: Vec<Pick>, t: &'static Texts) {
    let note = (t.study.added)(picks.len());
    ctx.change_noted(Some(note), move |doc, input| {
        study::add(doc, input, s, &picks, now_secs());
    }, || {});
}

/// The page after the last semester: one more, as a semester of leave if the student says so.
#[component]
pub(super) fn NewSemester(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let info = Memo::new(move |_| {
        ctx.with_ready(|ready| {
            let study = &ready.study;
            let last = study.semesters.last()?;
            let next = last.key.plus(1)?;
            let fs = study.semesters.iter().filter_map(|semester| semester.fs).max().map(|fs| fs.saturating_add(1));
            let plan_fs = ready.setup.core.and(study.regular_end).and_then(|end| study.semester(end)).and_then(|semester| semester.fs);
            Some((last.key, next, fs, plan_fs))
        })
        .flatten()
    });
    let leave = RwSignal::new(false);
    let pages = use_context::<Pages>();
    let append = move |_| {
        let (Some(mine), Some((_, next, ..))) = (ctx.mine, info.get_untracked()) else { return };
        if leave.get_untracked() {
            mine.set_leave(next, true);
        }
        mine.set_until(Some(next));
        ctx.focus.set(Some(next));
    };
    move || {
        info.get().map(|(last, next, fs, plan_fs)| view! {
            <article class="st-card-sem st-new" aria-labelledby="st-new-title">
                <header class="st-sem-head">
                    <button class="icon-btn st-turn hit" type="button" aria-label=s.previous on:click=move |_| turn(ctx, pages, Some(last))><Icon name="chevron-left"/></button>
                    <div class="st-sem-title">
                        <div class="st-sem-name"><SemesterPick ctx semester=next id="st-new-title".to_string() label=s.new_semester.to_string()/></div>
                        <p class="st-sem-sub">{(s.after)(&last.label(t.locale))}</p>
                    </div>
                    <button class="icon-btn st-turn hit" type="button" aria-label=s.next disabled=true><Icon name="chevron-right"/></button>
                </header>
                <div class="st-new-body">
                    <span class="st-new-icon" aria-hidden="true"><Icon name="calendar-plus"/></span>
                    <p class="st-new-title">{s.append_title}</p>
                    <p class="st-sub">{plan_fs.map_or_else(|| s.append_lead_plain.to_string(), s.append_lead)}</p>
                    <button class="btn primary" type="button" id="st-append" on:click=append>
                        <span>{(s.append)(&next.label(t.locale))}</span>
                        {move || (!leave.get()).then(|| fs.map(|fs| view! { <small>{(s.fs_long)(fs)}</small> })).flatten()}
                    </button>
                    <button class="st-check" type="button" role="checkbox" aria-checked=move || if leave.get() { "true" } else { "false" } on:click=move |_| leave.update(|leave| *leave = !*leave)>
                        <span class="st-box" class:with=move || leave.get() aria-hidden="true"><Icon name="check"/></span>
                        <span><b>{s.as_leave}</b><span class="st-sub">{s.leave_note}</span></span>
                    </button>
                </div>
            </article>
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A selection drawn over the rows selects what it passes, from where it began, and keeps the
    /// rest as it was; begun on a selected row it lets go instead; back over a row, that row is as
    /// before.
    #[test]
    fn a_selection_drawn_over_the_rows_selects_what_it_passes() {
        let order: Vec<String> = ["a", "b", "c", "d", "e"].map(String::from).to_vec();
        let set = |keys: &[&str]| keys.iter().map(|key| key.to_string()).collect::<BTreeSet<_>>();
        let down = Paint { start: 1, select: true, base: set(&["e"]) };
        assert_eq!(painted(&order, &down, 3), set(&["b", "c", "d", "e"]));
        assert_eq!(painted(&order, &down, 0), set(&["a", "b", "e"]), "upwards");
        assert_eq!(painted(&order, &down, 1), set(&["b", "e"]), "back where it began");
        let off = Paint { start: 2, select: false, base: set(&["a", "b", "c", "d"]) };
        assert_eq!(painted(&order, &off, 3), set(&["a", "b"]));
        assert_eq!(painted(&order, &down, 9), set(&["e"]), "beyond the rows: nothing");
    }

    /// A module's status: passed wherever it is marked so, not passed in a semester that is over,
    /// under way in the current one, planned in one to come.
    #[test]
    fn a_status_says_where_a_module_stands() {
        let item = |passed: bool, failed: bool| Item {
            subject: Subject::Module { id: "12345".to_string() },
            name: "Datenbanken".to_string(),
            credits: Some(6.0),
            range: false,
            kind: None,
            area: None,
            passed,
            failed,
            retake: None,
            again: None,
            offer: Default::default(),
            unoffered: false,
            over: 0.0,
            plan_fs: None,
            fillers: Vec::new(),
        };
        assert_eq!(Status::of(&item(true, false), When::Past), Status::Passed);
        assert_eq!(Status::of(&item(true, false), When::Later), Status::Passed, "marked as passed ahead of time");
        assert_eq!(Status::of(&item(false, true), When::Past), Status::Failed);
        assert_eq!(Status::of(&item(false, false), When::Now), Status::Running);
        assert_eq!(Status::of(&item(false, false), When::Later), Status::Planned);
    }
}
