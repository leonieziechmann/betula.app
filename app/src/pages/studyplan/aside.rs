//! One module beside the plan (`open=<id>`): every Termin of it in its semester, and what the plan
//! shows of them.
//!
//! This panel is where a module's dates are chosen (feature 2). A Termin reads in two lines (owner,
//! 2026-09-25): day and time bold, the room, the weeks („A", „B", „A/B"); under it how many, the
//! campus, first–last. An eye button hides an event, a Termin or an exam sitting and shows it
//! again; in a choice („1 von 4 wählen") the line of an option is the choice, with a radio mark: a
//! click chooses it, a second click or „Wahl aufheben" opens the choice again. What hides an event
//! from elsewhere (a kind switched off in the sidebar, the other town's course of a module taught
//! in both towns) is said once under the event, and its Termine step back. Above, where the module
//! stands in the plan: its credits, the placeholder it counts for, and „Entfernen". At the end, the
//! module's clashes and exam warnings in the words of the semester's notes, each leading to the
//! other module where there is one.
//!
//! The semester is the page's when the module is planned there (the Übersicht names the one of the
//! row clicked in `sem` as well). Otherwise (a module moved to another semester stays open) it is
//! the semester the module is planned in nearest to the page's, and the panel loads that
//! semester's plan itself: the queries of that semester's page, answered from the visit's cache the
//! second time. A module planned in several semesters says where else, and leads there.
//!
//! Choosing asks the catalog nothing (the store, then the selection and the timetable), so it is
//! written in the click. Removing changes what is planned, which the pages query for: the control
//! answers first and the store follows after the next frame (R21). What the panel lists (its
//! events, Termine and options) and what is shown of it (hidden, chosen) are two memos, so an eye
//! button or a choice flips its own state in place and keeps the focus.

use std::collections::{BTreeMap, BTreeSet};

use catalog::labels::{Campus, Code, Rhythm, TurnusSeason};
use catalog::pages::{self, StudyplanData};
use catalog::rows::CatalogRow;
use catalog::studyplan::{PlanDoc, Placeholder};
use catalog::timetable::clash;
use catalog::timetable::day::{clock, Day};
use catalog::timetable::exams::{self, ExamRow, ExamShape, ExamWarning, Termin, TerminAt, WarningKind};
use catalog::timetable::kind::{fold, EventKind, KindSet};
use catalog::timetable::model::{Attendance, Basis, Event, Row, Timetable};
use catalog::timetable::occur::Every;
use catalog::timetable::rowkey::RowKey;
use catalog::timetable::select::{HiddenBy, Town, TownChoice};
use catalog::timetable::semester::{of_fachsemester, SemesterKey};
use catalog::url::{PlanView, StudyplanUrl};
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use super::head::blocked_line;
use super::{full_href, key_of, PlanCtx};
use crate::data::DataError;
use crate::format;
use crate::myprogram::MyProgram;
use crate::nav;
use crate::pending::Pending;
use crate::ui::{ErrorState, Icon, Shortcut};

/// The weekdays as a line names them, Monday first.
const WEEKDAYS: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];

/// The group QIS gives a row that belongs to no group.
const UNNAMED_GROUP: &str = "[unbenannt]";

/// The panel's scrolling part: it starts at the top for every module.
const SCROLL_ID: &str = "plan-module";

/// Where the open module stands in the plan. Read from the address and the store together, like
/// the page's semester (`key_of`, R16).
#[derive(Clone, Debug, PartialEq)]
struct Place {
    id: String,
    /// The semester the page shows.
    shown: SemesterKey,
    /// The semester the panel shows (`home_semester`); `None` when the module is planned nowhere.
    sem: Option<SemesterKey>,
    /// The modules planned in `sem`, in plan order: that semester's timetable is theirs.
    ids: Vec<String>,
    /// „jetzt": `meta.current_semester`.
    current: Option<SemesterKey>,
    /// The Standort of Mein Studiengang, as the selection applies it.
    town: TownChoice,
}

/// What the panel is built from.
#[derive(Clone, Debug, PartialEq)]
enum Loaded {
    /// The module is planned in the page's semester: the page's data and timetable.
    Page,
    /// It is planned in another semester: that semester's plan, loaded for the panel.
    Other(Result<StudyplanData, DataError>),
    /// It is planned nowhere: its row of the catalog, when it has one.
    Nowhere(Result<Option<CatalogRow>, DataError>),
}

/// What the panel knows of the module and its semester besides the timetable.
#[derive(Clone, Debug, PartialEq)]
struct Info {
    place: Place,
    title: Option<String>,
    credits: Option<f64>,
    turnus: Option<TurnusSeason>,
    /// The catalog does not know the module.
    missing: bool,
    /// The semester has dated teaching rows at all.
    dated: bool,
    /// The titles of the semester's planned modules, for the notes.
    titles: BTreeMap<String, String>,
    error: Option<DataError>,
}

/// The panel's head: the module's title and credits.
#[derive(Clone, Debug, PartialEq)]
struct Head {
    title: String,
    credits: Option<f64>,
    missing: bool,
}

/// Where the module stands, as the select of its placeholder offers it.
#[derive(Clone, Debug, PartialEq)]
struct Choices {
    id: String,
    sem: SemesterKey,
    /// The placeholders it can count for, with their labels.
    placeholders: Vec<(u32, String)>,
    fills: Option<u32>,
    /// The other semesters it is planned in (a module over two semesters, a retake).
    elsewhere: Vec<SemesterKey>,
}

/// What the panel lists: the module's events and exams in its semester, with their Termine and
/// the choices made. It changes with the data and with a choice, not with an eye button.
#[derive(Clone, Debug, Default, PartialEq)]
struct Body {
    sem: Option<SemesterKey>,
    error: Option<DataError>,
    /// A line above the events: no dates in this semester, or no plan.
    lead: Option<String>,
    blocks: Vec<Block>,
}

/// Which event or exam of a semester a block is: its key in the panel's keyed list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BlockKey {
    sem: SemesterKey,
    exam: bool,
    index: usize,
}

impl BlockKey {
    /// The block's name in the page (`data-block`), where the focus is handed on inside it: „e3"
    /// for the semester's fourth event, „x0" for its first exam.
    fn name(self) -> String {
        format!("{}{}", if self.exam { "x" } else { "e" }, self.index)
    }

    /// The block of this key in what the panel lists.
    fn block(self, body: &Body) -> Option<&Block> {
        body.blocks.iter().find(|block| block.exam == self.exam && block.index == self.index)
    }

    /// What is shown of this key's block now.
    fn state(self, shown: &[BlockShown]) -> Option<&BlockShown> {
        shown.iter().find(|state| state.exam == self.exam && state.index == self.index)
    }
}

/// An event or an exam of the module.
#[derive(Clone, Debug, PartialEq)]
struct Block {
    exam: bool,
    /// Its index in `Timetable::events` or `Timetable::exams`.
    index: usize,
    /// QIS's `veranstid` as the store keeps it; `None` when it is none (no eye then).
    event: Option<u32>,
    head: String,
    qis: Option<String>,
    items: Vec<Item>,
}

/// A line of an event.
#[derive(Clone, Debug, PartialEq)]
enum Item {
    /// A Termin (the rows of one row key: the same slot in two rooms is one Termin).
    Line(Line),
    /// The event's choice: „1 von 4 wählen" while it is open, „Wahl aufheben" once it is made.
    Choice,
    /// An option of several Termine (a group), named: the row that chooses it, its Termine under it.
    Option { name: String, key: Option<RowKey>, option: usize },
    /// An event QIS lists without any date.
    Undated,
}

#[derive(Clone, Debug, PartialEq)]
struct Line {
    /// Row indices of the event or exam.
    rows: Vec<usize>,
    key: Option<RowKey>,
    text: RowText,
    /// The option of a choice this line alone is: the line itself chooses it.
    choose: Option<usize>,
}

/// A Termin as two lines (owner, 2026-09-25): „**Mo 11:30–13:00** · Verfügungsgebäude 1C - 0.03 ·
/// A/B", and quieter under it „15 Termine · Zentralcampus · 05.10.–25.01.".
#[derive(Clone, Debug, Default, PartialEq)]
struct RowText {
    /// Day and time, bold: „Mo 11:30–13:00"; a date instead of the weekday where the Termin is
    /// not weekly („Do 12.11. 13:45–15:15").
    when: String,
    /// The room as QIS writes it, without the campus (the second line says it).
    room: Option<String>,
    /// Which weeks: „A", „B", „A/B" (every week); „Block", „einmalig" where it is not weekly.
    week: Option<String>,
    /// The second line: group, how many (and how many fall out), campus, first–last.
    detail: Vec<String>,
}

impl RowText {
    /// What follows the bold time on the first line: „Verfügungsgebäude 1C - 0.03 · A/B".
    fn rest(&self) -> Option<String> {
        let parts: Vec<&str> = self.room.iter().chain(&self.week).map(String::as_str).collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    /// The second line: „15 Termine · Zentralcampus · 05.10.–25.01.".
    #[cfg(test)]
    fn detail(&self) -> Option<String> {
        (!self.detail.is_empty()).then(|| self.detail.join(" · "))
    }

    /// The two lines in one piece: „Mo 15:30–17:00 · LG 10/214 · A/B | 14 Termine · Zentralcampus
    /// · 12.10.–25.01.".
    #[cfg(test)]
    fn text(&self) -> String {
        let first = std::iter::once(self.when.clone()).chain(self.rest()).collect::<Vec<_>>().join(" · ");
        match self.detail() {
            Some(detail) => format!("{first} | {detail}"),
            None => first,
        }
    }
}

/// What is shown of a block now. It changes with every eye button.
#[derive(Clone, Debug, Default, PartialEq)]
struct BlockShown {
    /// The block's `exam` and `index`: whose state this is.
    exam: bool,
    index: usize,
    hidden: Option<HiddenBy>,
    /// Why it is hidden when the reason lies elsewhere: „Übungen ausgeblendet", „Standort …".
    reason: Option<String>,
    /// The event's choice while it is shown: of how many options (those whose Termine are not
    /// all hidden), and whether one is chosen.
    choice: Option<(usize, bool)>,
    /// Per item of the block.
    items: Vec<ItemShown>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ItemShown {
    hidden: Option<HiddenBy>,
    /// A Termin in a hard clash.
    clash: bool,
    /// The option it is can be chosen now (its line is a choice).
    choosable: bool,
    /// It is the option chosen.
    chosen: bool,
    /// An exam sitting after the module's first: „2. Termin".
    second: bool,
}

/// One of the module's notes.
#[derive(Clone, Debug, PartialEq)]
struct Note {
    /// A warning (`p.note` with its triangle); else a quiet line.
    warn: bool,
    text: String,
    /// The other module the note is about, and its Termin: the note leads there.
    to: Option<(String, Option<RowKey>)>,
}

/// Everything the panel shows below its head, from one timetable.
#[derive(Clone, Debug, Default, PartialEq)]
struct Shown {
    body: Body,
    blocks: Vec<BlockShown>,
    notes: Vec<Note>,
}

#[component]
pub(super) fn PlanModulePanel(ctx: PlanCtx) -> impl IntoView {
    let going = Pending::expect();
    let town = move || ctx.mine.map(MyProgram::town).unwrap_or_default();

    let place = Memo::new(move |_| {
        let (url, current) = (ctx.url.get(), ctx.current.get());
        let id = url.open.clone()?;
        let town = town();
        let plan = ctx.plan?;
        Some(plan.with(|doc| {
            let shown = key_of(&url, current, doc, ctx.today);
            let sem = home_semester(doc, shown, &id);
            let ids = sem.map(|sem| doc.modules_in(sem)).unwrap_or_default();
            Place { id, shown, sem, ids, current, town }
        }))
    });
    let loaded = Memo::new(move |_| {
        let place = place.get()?;
        let loaded = match place.sem {
            Some(sem) if sem == place.shown => Loaded::Page,
            Some(sem) => Loaded::Other(ctx.source.with_value(|source| match source {
                Some(source) => source.run(|db| pages::studyplan(db, sem, &place.ids)),
                None => Err(unavailable()),
            })),
            None => Loaded::Nowhere(ctx.source.with_value(|source| match source {
                Some(source) => {
                    source.run(|db| pages::studyplan_modules(db, std::slice::from_ref(&place.id))).map(|(rows, _)| rows.into_iter().next())
                }
                None => Err(unavailable()),
            })),
        };
        Some((place, loaded))
    });
    // What another semester hides and has chosen. Its own closure over the address and the store,
    // like `place`: it moves with every eye button, and `loaded` must not.
    let other_selection = Memo::new(move |_| {
        let (url, current) = (ctx.url.get(), ctx.current.get());
        let id = url.open.clone()?;
        let town = town();
        ctx.plan?.with(|doc| {
            let shown = key_of(&url, current, doc, ctx.today);
            let sem = home_semester(doc, shown, &id).filter(|sem| *sem != shown)?;
            Some((sem, doc.selection(sem, town)))
        })
    });
    let table = Memo::new(move |_| {
        loaded.with(|loaded| match loaded {
            Some((_, Loaded::Page)) => ctx.table.get(),
            Some((_, Loaded::Other(Ok(data)))) => other_selection.with(|selection| {
                selection.as_ref().filter(|(sem, _)| *sem == data.key).map(|(_, selection)| data.timetable(selection))
            }),
            _ => None,
        })
    });
    let info = Memo::new(move |_| {
        loaded.with(|loaded| {
            let (place, loaded) = loaded.as_ref()?;
            Some(match loaded {
                Loaded::Page => ctx.data.with(|data| info_of(place, data.as_ref())),
                Loaded::Other(data) => info_of(place, data.as_ref()),
                Loaded::Nowhere(row) => info_nowhere(place, row.as_ref().map(Option::as_ref)),
            })
        })
    });
    let shown_all = Memo::new(move |_| info.with(|info| info.as_ref().map(|info| table.with(|table| shown_of_panel(info, table.as_ref())))));
    let head = Memo::new(move |_| info.with(|info| info.as_ref().map(head_of)));
    let body = Memo::new(move |_| shown_all.with(|all| all.as_ref().map(|all| all.body.clone()).unwrap_or_default()));
    let shown = Memo::new(move |_| shown_all.with(|all| all.as_ref().map(|all| all.blocks.clone()).unwrap_or_default()));
    let notes = Memo::new(move |_| shown_all.with(|all| all.as_ref().map(|all| all.notes.clone()).unwrap_or_default()));

    // Where the module is planned and what it counts for.
    let choices = Memo::new(move |_| {
        let (url, current) = (ctx.url.get(), ctx.current.get());
        let id = url.open.clone()?;
        let start = ctx.mine.and_then(MyProgram::start);
        ctx.plan?.with(|doc| {
            let shown = key_of(&url, current, doc, ctx.today);
            let sem = home_semester(doc, shown, &id)?;
            let fills = doc.modules.iter().find(|m| m.semester == sem && m.module_id == id).and_then(|m| m.fills);
            Some(Choices {
                placeholders: placeholder_choices(doc, sem, start, fills),
                elsewhere: doc.planned_in(&id).into_iter().filter(|other| *other != sem).collect(),
                id,
                sem,
                fills,
            })
        })
    });

    // Another module starts at its top (the panel stays, only its content changes); the Termin the
    // address points at is scrolled into view, once it is there.
    let opened = Memo::new(move |_| ctx.url.with(|url| url.open.clone()));
    let pointed = Memo::new(move |_| ctx.url.with(|url| url.open.clone().zip(url.row.clone())));
    let listed = Memo::new(move |_| body.with(|body| !body.blocks.is_empty()));
    Effect::new(move |before: Option<Option<String>>| {
        let open = opened.get();
        if before.is_some_and(|before| before != open) {
            nav::after_paint(|| nav::scroll_list_to_start(SCROLL_ID));
        }
        if pointed.with(Option::is_some) && listed.get() {
            nav::after_paint(|| {
                nav::reveal_selector("#preview .sp-rowline[aria-current=\"true\"]");
            });
        }
        open
    });

    let close = move || ctx.url.with(|url| url.with_open(None, None).path());
    let full = move || ctx.url.with(|url| url.open.as_deref().map(|id| full_href(url, id)).unwrap_or_default());
    let id = move || ctx.url.with(|url| url.open.clone().unwrap_or_default());

    let heading = move || {
        head.get().map(|head| {
            view! {
                <h2>{head.title}</h2>
                {head.missing.then(|| view! { <p class="hint">"Nicht im Modulkatalog."</p> })}
            }
        })
    };
    // The events as a keyed list (R5): a choice rebuilds the lines of its own event, and every
    // other event stays as it is.
    let error = Memo::new(move |_| body.with(|body| body.error.clone()));
    let lead = Memo::new(move |_| body.with(|body| body.lead.clone()));
    let keys = Memo::new(move |_| {
        body.with(|body| match body.sem {
            Some(sem) => body.blocks.iter().map(|block| BlockKey { sem, exam: block.exam, index: block.index }).collect(),
            None => Vec::new(),
        })
    });
    let notes_view = move || {
        let notes = notes.get();
        (!notes.is_empty()).then(|| view! { <div class="section sp-notes">{notes.into_iter().map(|note| note_view(ctx, note)).collect_view()}</div> })
    };

    view! {
        <section class="panel detail aside" id="preview" aria-label="Modul im Plan">
            <div class="scroll" id=SCROLL_ID>
                <header class="hero">
                    // No arrow back on a phone as well, where the panel is the page: „Schließen"
                    // says it, and a phone has no room for both beside „Modul ansehen".
                    <div class="hero-top">
                        <span class="mono">{id}</span>
                        <a class="ghost" href=full data-action="fullscreen" title="Als ganze Seite öffnen (F)"><Icon name="maximize-2"/>"Modul ansehen"<Shortcut keys="F"/></a>
                        <a class="ghost" href=close data-action="close-detail" title="Schließen (Esc)"><Icon name="x"/>"Schließen"<Shortcut keys="Esc"/></a>
                    </div>
                    {heading}
                    <Actions ctx going head place table choices/>
                </header>
                <div class="dbody">
                    {move || error.get().map(|error| view! { <ErrorState error/> })}
                    {move || lead.get().map(|lead| view! { <p class="note quiet">{lead}</p> })}
                    <For each=move || keys.get() key=|key| *key children=move |key: BlockKey| view! { <BlockView ctx key body shown/> }/>
                    {notes_view}
                </div>
            </div>
        </section>
    }
}

/// Under the title: the credits, and where the module is planned — the placeholder it counts for,
/// „Entfernen" — and the other semesters it is planned in. Removing answers in the next frame and
/// changes the plan after it (R21); „Entfernen" closes the panel, since the module is no longer
/// there. (The Stundenplan is one semester's: the select that moved a module to another semester
/// is gone, owner 2026-09-25.)
#[component]
fn Actions(
    ctx: PlanCtx,
    going: Option<Pending>,
    head: Memo<Option<Head>>,
    place: Memo<Option<Place>>,
    table: Memo<Option<Timetable>>,
    choices: Memo<Option<Choices>>,
) -> impl IntoView {
    let credits = Memo::new(move |_| head.with(|head| head.as_ref().map(|head| format::credits(head.credits))));
    let planned = Memo::new(move |_| choices.with(Option::is_some));
    let placeholders = Memo::new(move |_| choices.with(|c| c.as_ref().map(|c| c.placeholders.clone()).unwrap_or_default()));
    let fills = Memo::new(move |_| choices.with(|c| c.as_ref().and_then(|c| c.fills)));
    let elsewhere = Memo::new(move |_| choices.with(|c| c.as_ref().map(|c| c.elsewhere.clone()).unwrap_or_default()));
    let removing = RwSignal::new(false);

    let set_fills = move |ev: leptos::ev::Event| {
        let Some(plan) = ctx.plan else { return };
        let pid = event_target_value(&ev).parse::<u32>().ok();
        let Some((id, sem)) = choices.with_untracked(|c| c.as_ref().map(|c| (c.id.clone(), c.sem))) else { return };
        plan.update(|doc| doc.set_fills(sem, &id, pid));
    };
    let remove = move |_: leptos::ev::MouseEvent| {
        let Some(plan) = ctx.plan else { return };
        let Some((id, sem)) = place.with_untracked(|place| place.as_ref().and_then(|place| Some((place.id.clone(), place.sem?)))) else { return };
        let events = table.with_untracked(|t| t.as_ref().filter(|t| t.key == sem).map(|t| only_its_events(t, &id)).unwrap_or_default());
        removing.set(true);
        // In place of the panel's own entry: Back leads to where the visitor was before opening
        // the module, not to a panel of a module the plan no longer holds.
        if let Some(going) = going {
            going.go(&ctx.url.with_untracked(|url| url.with_open(None, None).path()), NavigateOptions { replace: true, scroll: false, ..Default::default() });
        }
        plan.update_after_paint(move |doc| doc.unplan(sem, &id, &events));
    };

    let selects = move || {
        planned.get().then(|| {
            view! {
                {move || (!placeholders.with(Vec::is_empty)).then(|| view! {
                    <select aria-label="Zählt für" on:change=set_fills>
                        <option value="" prop:selected=move || fills.get().is_none()>"Für keinen Platzhalter"</option>
                        {move || placeholders.get().into_iter().map(|(pid, label)| view! {
                            <option value=pid.to_string() prop:selected=move || fills.get() == Some(pid)>{label}</option>
                        }).collect_view()}
                    </select>
                })}
                <button class="mini" type="button" on:click=remove aria-busy=move || removing.get().then_some("true")>"Entfernen"</button>
            }
        })
    };
    // „Auch geplant: WiSe 2027/28": the module in another of its semesters, beside the same view
    // (on the Übersicht only the panel changes).
    let also = move || {
        let others = elsewhere.get();
        (!others.is_empty()).then(|| {
            let links = others
                .into_iter()
                .enumerate()
                .map(|(n, sem)| {
                    let href = move || {
                        ctx.url.with(|url| {
                            let open = url.open.clone();
                            StudyplanUrl { sem: Some(sem.key()), open, row: None, ..url.clone() }.path()
                        })
                    };
                    let overview = move || ctx.url.with(|url| (url.view == PlanView::Overview).then_some(""));
                    view! { {(n > 0).then_some(", ")}<a href=href data-noscroll=overview>{sem.label()}</a> }
                })
                .collect_view();
            view! { <p class="hint sp-also">"Auch geplant: "{links}</p> }
        })
    };
    view! {
        {move || credits.get().map(|credits| view! {
            <div class="sp-actions">
                <span class="badge strong num">{credits}</span>
                {selects}
            </div>
        })}
        {also}
    }
}

/// An event or an exam of the module, with its Termine. What it lists follows `body` (a choice
/// changes its lines), what is hidden of it `shown`; its head stays, and so does its eye.
#[component]
fn BlockView(ctx: PlanCtx, key: BlockKey, body: Memo<Body>, shown: Memo<Vec<BlockShown>>) -> impl IntoView {
    let sem = key.sem;
    // The key names one event or exam of the semester, so its number and its link stay; its head
    // names the module's title, which another module sharing the event has otherwise.
    let (event, qis) = body.with_untracked(|body| key.block(body).map(|block| (block.event, block.qis.clone())).unwrap_or_default());
    let head = Memo::new(move |_| body.with(|body| key.block(body).map(|block| block.head.clone()).unwrap_or_default()));
    let items = Memo::new(move |_| body.with(|body| key.block(body).map(|block| block.items.clone()).unwrap_or_default()));
    let state = Memo::new(move |_| shown.with(|shown| key.state(shown).map(|s| (s.hidden, s.reason.clone(), s.choice)).unwrap_or_default()));
    let pressed = Memo::new(move |_| state.with(|s| s.0 == Some(HiddenBy::Event)));
    // The eye stays while the event is shown or hidden by it; another reason is said instead, and
    // an eye that cannot be pressed keeps the head's place.
    let eye = Memo::new(move |_| state.with(|s| matches!(s.0, None | Some(HiddenBy::Event))));
    let elsewhere = Memo::new(move |_| state.with(|s| s.0.is_some_and(|by| by != HiddenBy::Event)));
    let reason = Memo::new(move |_| state.with(|s| s.1.clone()));
    let choice = Memo::new(move |_| state.with(|s| s.2));
    let toggle = move |_: leptos::ev::MouseEvent| {
        if let (Some(plan), Some(event)) = (ctx.plan, event) {
            let hide = !pressed.get_untracked();
            plan.update(|doc| doc.set_event(sem, event, hide));
        }
    };
    let label = move || match (key.exam, pressed.get()) {
        (false, false) => "Veranstaltung ausblenden",
        (false, true) => "Veranstaltung einblenden",
        (true, false) => "Prüfung ausblenden",
        (true, true) => "Prüfung einblenden",
    };
    let lines = move || {
        items
            .get()
            .into_iter()
            .enumerate()
            .map(|(item, it)| {
                let state = Memo::new(move |_| shown.with(|shown| key.state(shown).and_then(|s| s.items.get(item).copied()).unwrap_or_default()));
                item_view(ctx, key.sem, event, it, state, choice)
            })
            .collect_view()
    };
    let hidden = Memo::new(move |_| state.with(|s| s.0.is_some()));
    view! {
        <div class="sp-event" data-block=key.name() data-hidden=move || hidden.get().then_some("")>
            <h3>
                {move || match (event.is_some() && eye.get(), elsewhere.get()) {
                    (true, _) => view! {
                        <button class="eye" type="button" on:click=toggle aria-pressed=move || if pressed.get() { "true" } else { "false" } aria-label=label title=label>
                            {move || eye_icon(pressed.get())}
                        </button>
                    }
                    .into_any(),
                    // An `i`: the head's title is its `span`, which takes the rest of the line.
                    (false, true) => view! { <i class="eye" aria-hidden="true"><Icon name="eye-off"/></i> }.into_any(),
                    (false, false) => view! { <i class="eye" aria-hidden="true"></i> }.into_any(),
                }}
                <span>{move || head.get()}</span>
                {qis.map(|href| view! { <a href=href rel="noopener">"In QIS"</a> })}
            </h3>
            {move || reason.get().map(|reason| view! { <p class="sp-choice">{reason}</p> })}
            {lines}
        </div>
    }
}

/// One item of an event: a Termin, or a line of its choice. In a choice („1 von 4 wählen") the
/// line of an option is the choice itself, with a radio mark at its left (owner, 2026-09-25): a
/// click chooses it, a click on the one chosen opens the choice again, as „Wahl aufheben" does.
/// A choice changes what is shown, not what is listed, so the lines stay and so does the focus.
fn item_view(ctx: PlanCtx, sem: SemesterKey, event: Option<u32>, item: Item, state: Memo<ItemShown>, choice: Memo<Option<(usize, bool)>>) -> AnyView {
    let pick = move |key: Option<RowKey>| {
        move |_: leptos::ev::MouseEvent| {
            let (Some(plan), Some(event), Some(key)) = (ctx.plan, event, key) else { return };
            let chosen = state.with_untracked(|s| s.chosen);
            plan.update(|doc| doc.choose(sem, event, (!chosen).then_some(key)));
        }
    };
    match item {
        Item::Line(line) => line_view(ctx, sem, line, state, pick),
        Item::Choice => {
            let reopen = move |_: leptos::ev::MouseEvent| {
                if let (Some(plan), Some(event)) = (ctx.plan, event) {
                    plan.update(|doc| doc.choose(sem, event, None));
                }
            };
            (move || {
                choice.get().map(|(of, chosen)| {
                    view! {
                        <p class="sp-choice">
                            <span>{format!("1 von {of} wählen")}</span>
                            {chosen.then(|| view! { <button class="mini" type="button" data-action="reopen" on:click=reopen>"Wahl aufheben"</button> })}
                        </p>
                    }
                })
            })
            .into_any()
        }
        Item::Option { name, key, .. } => {
            let text = view! { <span class="rt"><b>{name}</b></span> };
            view! { <div class="sp-rowline sp-option">{pick_button(state, pick(key), text)}</div> }.into_any()
        }
        Item::Undated => view! { <p class="sp-choice">"Termine nicht angegeben"</p> }.into_any(),
    }
}

/// The line of an option as its choice: a radio mark and the text, pressed while it is the one
/// chosen.
fn pick_button<H>(state: Memo<ItemShown>, pick: H, text: impl IntoView + 'static) -> AnyView
where
    H: Fn(leptos::ev::MouseEvent) + Send + Sync + 'static,
{
    let chosen = Memo::new(move |_| state.with(|s| s.chosen));
    let choosable = Memo::new(move |_| state.with(|s| s.choosable));
    let title = move || match (chosen.get(), choosable.get()) {
        (true, true) => Some("Wahl aufheben"),
        (false, true) => Some("Diesen wählen"),
        _ => None,
    };
    view! {
        <button class="rl pick" type="button" data-action="choose" on:click=pick disabled=move || !choosable.get() aria-pressed=move || if chosen.get() { "true" } else { "false" } title=title>
            <i class="radio" aria-hidden="true"></i>
            {text}
        </button>
    }
    .into_any()
}

/// A Termin in its two lines, with its eye; the one the address points at is current. The line
/// of an option is its choice (`pick_button`).
fn line_view<C, H>(ctx: PlanCtx, sem: SemesterKey, line: Line, state: Memo<ItemShown>, pick: C) -> AnyView
where
    C: Fn(Option<RowKey>) -> H + Copy + Send + Sync + 'static,
    H: Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static,
{
    let key = line.key;
    let key_text = key.map(RowKey::text);
    let current = Memo::new(move |_| ctx.url.with(|url| key_text.is_some() && url.row == key_text));
    // Another option chosen: its Termine step back, not struck through (its mark says it).
    let hidden = Memo::new(move |_| state.with(|s| s.hidden.is_some_and(|by| by != HiddenBy::Choice)));
    let off = Memo::new(move |_| state.with(|s| s.hidden == Some(HiddenBy::Choice)));
    let pressed = Memo::new(move |_| state.with(|s| s.hidden == Some(HiddenBy::Row)));
    // Hidden with its event, by a kind or a town: dimmed, without an eye of its own.
    let eye = Memo::new(move |_| state.with(|s| matches!(s.hidden, None | Some(HiddenBy::Row))));
    let clash = Memo::new(move |_| state.with(|s| s.clash));
    let second = Memo::new(move |_| state.with(|s| s.second));
    let toggle = move |_: leptos::ev::MouseEvent| {
        if let (Some(plan), Some(key)) = (ctx.plan, key) {
            let hide = !pressed.get_untracked();
            plan.update(|doc| doc.set_row(sem, key, hide));
        }
    };
    let label = move || if pressed.get() { "Termin einblenden" } else { "Termin ausblenden" };
    let text = &line.text;
    // The second line breaks between its parts, not inside „05.10.–25.01.".
    let detail = (!text.detail.is_empty()).then(|| {
        let parts = text.detail.iter().enumerate().map(|(n, part)| view! { {(n > 0).then_some(" · ")}<span>{part.clone()}</span> }).collect_view();
        view! { <br/><small>{parts}</small> }
    });
    let lines = view! {
        <span class="rt">
            <b>{text.when.clone()}</b>
            {text.rest().map(|rest| format!(" · {rest}"))}
            {detail}
        </span>
    };
    let first = match line.choose {
        Some(_) => pick_button(state, pick(key), lines),
        None => view! { <span class="rl">{lines}</span> }.into_any(),
    };
    view! {
        <div class="sp-rowline" aria-current=move || current.get().then_some("true") data-hidden=move || hidden.get().then_some("") data-off=move || off.get().then_some("")>
            {first}
            {move || clash.get().then(|| view! { <small class="clash">"Überschneidung"</small> })}
            {move || second.get().then(|| view! { <small>"2. Termin"</small> })}
            {key.is_some().then_some(move || eye.get().then(|| view! {
                <button class="eye" type="button" on:click=toggle aria-pressed=move || if pressed.get() { "true" } else { "false" } aria-label=label title=label>
                    {move || eye_icon(pressed.get())}
                </button>
            }))}
        </div>
    }
    .into_any()
}

/// A note of the module; one about another module leads to it beside the plan.
fn note_view(ctx: PlanCtx, note: Note) -> AnyView {
    let class = if note.warn { "note" } else { "note quiet" };
    let icon = note.warn.then(|| view! { <Icon name="triangle-alert"/> });
    match note.to {
        Some((module, row)) => {
            let href = move || ctx.url.with(|url| url.with_open(Some(&module), row).path());
            view! { <a class=class href=href data-noscroll="">{icon}<span>{note.text}</span></a> }.into_any()
        }
        None => view! { <p class=class>{icon}<span>{note.text}</span></p> }.into_any(),
    }
}

fn eye_icon(hidden: bool) -> AnyView {
    if hidden {
        view! { <Icon name="eye-off"/> }.into_any()
    } else {
        view! { <Icon name="eye"/> }.into_any()
    }
}

fn unavailable() -> DataError {
    DataError { unavailable: true, message: "no data source was provided".to_string() }
}

/// The semester the panel shows a module in: the page's when the module is planned there, else
/// the one it is planned in nearest to the page's (of two as near, the earlier), else none.
fn home_semester(doc: &PlanDoc, shown: SemesterKey, id: &str) -> Option<SemesterKey> {
    if doc.is_planned(shown, id) {
        return Some(shown);
    }
    doc.planned_in(id).into_iter().min_by_key(|sem| (sem.index().abs_diff(shown.index()), *sem))
}

fn info_of(place: &Place, data: Result<&StudyplanData, &DataError>) -> Info {
    let mut info = Info {
        place: place.clone(),
        title: None,
        credits: None,
        turnus: None,
        missing: false,
        dated: false,
        titles: BTreeMap::new(),
        error: None,
    };
    match data {
        Ok(data) => {
            if let Some(row) = data.modules.iter().find(|row| row.id == place.id) {
                info.title = Some(row.title.clone());
                info.credits = row.credits;
                info.turnus = row.turnus_season.as_ref().and_then(Code::known);
            }
            info.missing = data.missing.contains(&place.id);
            info.dated = !data.counts.is_empty();
            info.titles = data.titles();
        }
        Err(error) => info.error = Some(error.clone()),
    }
    info
}

/// A module planned nowhere: its row of the catalog; a failed query is an error, not a module
/// the catalog does not know (R3).
fn info_nowhere(place: &Place, row: Result<Option<&CatalogRow>, &DataError>) -> Info {
    let (row, error) = match row {
        Ok(row) => (row, None),
        Err(error) => (None, Some(error.clone())),
    };
    Info {
        place: place.clone(),
        title: row.map(|row| row.title.clone()),
        credits: row.and_then(|row| row.credits),
        turnus: row.and_then(|row| row.turnus_season.as_ref().and_then(Code::known)),
        missing: row.is_none() && error.is_none(),
        dated: false,
        titles: BTreeMap::new(),
        error,
    }
}

fn head_of(info: &Info) -> Head {
    Head { title: info.title.clone().unwrap_or_else(|| info.place.id.clone()), credits: info.credits, missing: info.missing }
}

/// Everything below the head, from the module's semester and its timetable (`None` while the
/// timetable of that semester is not there yet).
fn shown_of_panel(info: &Info, table: Option<&Timetable>) -> Shown {
    let place = &info.place;
    if let Some(error) = &info.error {
        return Shown { body: Body { sem: place.sem, error: Some(error.clone()), ..Body::default() }, ..Shown::default() };
    }
    let Some(sem) = place.sem else {
        return Shown { body: Body { lead: Some("Nicht im Studienplan.".to_string()), ..Body::default() }, ..Shown::default() };
    };
    let Some(t) = table.filter(|t| t.key == sem) else {
        return Shown { body: Body { sem: Some(sem), ..Body::default() }, ..Shown::default() };
    };
    let blocks = blocks_of(t, &place.id, info.title.as_deref());
    Shown {
        blocks: shown_of(t, &blocks),
        notes: notes_of(t, &place.id, &info.titles, place.town),
        body: Body { sem: Some(sem), error: None, lead: lead_of(info, t, sem), blocks },
    }
}

/// The line above the events where the semester has none of the module's (A.5). A semester before
/// the current one is over, whether the data still holds some of its dates or none at all
/// (retention removes them): its dates are not still to come.
fn lead_of(info: &Info, t: &Timetable, sem: SemesterKey) -> Option<String> {
    let label = sem.label();
    if info.dated && !t.without_dates.contains(&info.place.id) {
        return None;
    }
    if info.place.current.is_some_and(|current| sem < current) {
        return Some(format!("{label} ist vorbei; vergangene Termine fehlen im Datenstand."));
    }
    if !info.dated {
        return Some(format!("{label}: noch keine Termine veröffentlicht."));
    }
    let season = match (info.turnus, sem.winter) {
        (Some(TurnusSeason::Summer), true) => " (laut Beschreibung im Sommer)",
        (Some(TurnusSeason::Winter), false) => " (laut Beschreibung im Winter)",
        _ => "",
    };
    Some(format!("Keine Termine im {label}{season}."))
}

/// The module's events and exams in `t`, in the timetable's order, with their lines; what the
/// Standort hides after the rest.
fn blocks_of(t: &Timetable, id: &str, title: Option<&str>) -> Vec<Block> {
    let own = |modules: &[String]| modules.iter().any(|module| module == id);
    let events = t.events.iter().enumerate().filter(|(_, event)| own(&event.modules)).map(|(index, event)| Block {
        exam: false,
        index,
        event: event_number(&event.id),
        head: head_text(&kind_word(event), &event.title, title),
        qis: event.source_url.clone(),
        items: items_of(event),
    });
    let exams = t.exams.iter().enumerate().filter(|(_, exam)| own(&exam.modules)).map(|(index, exam)| {
        let items = if exam.rows.is_empty() {
            vec![Item::Undated]
        } else {
            by_key(exam.rows.iter().map(|row| row.key))
                .into_iter()
                .map(|(key, rows)| {
                    let refs: Vec<&ExamRow> = rows.iter().filter_map(|row| exam.rows.get(*row)).collect();
                    Item::Line(Line { text: exam_text(&refs), rows, key, choose: None })
                })
                .collect()
        };
        Block { exam: true, index, event: event_number(&exam.event_id), head: head_text("Prüfung", &exam.title, title), qis: exam.source_url.clone(), items }
    });
    // The other town's course of a module taught in both comes last: first what the student
    // attends, its exams included.
    // (As the timetable decides it, whatever else hides the event.)
    let other_course = |modules: &[String], town: Option<Town>| {
        let all_tracks = !modules.is_empty() && modules.iter().all(|module| t.tracks.contains(module));
        all_tracks && town.zip(t.town).is_some_and(|(own, shown)| own != shown)
    };
    let (theirs, own_course): (Vec<Block>, Vec<Block>) = events.chain(exams).partition(|block| {
        if block.exam {
            t.exams.get(block.index).is_some_and(|exam| other_course(&exam.modules, exam.town()))
        } else {
            t.events.get(block.index).is_some_and(|event| other_course(&event.modules, event.town()))
        }
    });
    own_course.into_iter().chain(theirs).collect()
}

/// An event's lines: its required Termine, then its choice („1 von 4 wählen") and every option,
/// open or made — a Termin alone is its own choice, a group of several stands under its name,
/// which is the choice.
fn items_of(event: &Event) -> Vec<Item> {
    if event.rows.is_empty() {
        return vec![Item::Undated];
    }
    let lines = |indices: &[usize], with_group: bool| -> Vec<Line> {
        by_key(indices.iter().map(|index| event.rows.get(*index).and_then(|row| row.key)))
            .into_iter()
            .map(|(key, positions)| {
                let rows: Vec<usize> = positions.iter().filter_map(|position| indices.get(*position).copied()).collect();
                let refs: Vec<&Row> = rows.iter().filter_map(|row| event.rows.get(*row)).collect();
                let group = refs.first().filter(|_| with_group).and_then(|row| group_of(row));
                Line { text: row_text(&refs, group), rows, key, choose: None }
            })
            .collect()
    };
    let all: Vec<usize> = (0..event.rows.len()).collect();
    let Attendance::OneOf { options, basis } = &event.attendance else {
        return lines(&all, true).into_iter().map(Item::Line).collect();
    };
    let groups = matches!(basis, Basis::Groups);
    let required: Vec<usize> = all.iter().copied().filter(|index| event.rows.get(*index).is_some_and(|row| row.option.is_none())).collect();
    let mut items: Vec<Item> = lines(&required, true).into_iter().map(Item::Line).collect();
    items.push(Item::Choice);
    for (option, rows) in options.iter().enumerate() {
        let option_lines = lines(rows, groups);
        if option_lines.len() == 1 {
            items.extend(option_lines.into_iter().map(|line| Item::Line(Line { choose: Some(option), ..line })));
            continue;
        }
        let key = option_lines.first().and_then(|line| line.key);
        items.push(Item::Option { name: option_name(event, options, option, groups), key, option });
        // Under the option's name its lines need not repeat it.
        items.extend(lines(rows, false).into_iter().map(Item::Line));
    }
    items
}

/// The rows of one row key together, in the order their keys first appear; a row without a key
/// stands alone. Returns positions in the input.
fn by_key(keys: impl Iterator<Item = Option<RowKey>>) -> Vec<(Option<RowKey>, Vec<usize>)> {
    let mut groups: Vec<(Option<RowKey>, Vec<usize>)> = Vec::new();
    for (position, key) in keys.enumerate() {
        match key.and_then(|key| groups.iter_mut().find(|(known, _)| *known == Some(key))) {
            Some((_, positions)) => positions.push(position),
            None => groups.push((key, vec![position])),
        }
    }
    groups
}

/// What an option is called: its group, else the weekday and start of its first Termin.
fn option_name(event: &Event, options: &[Vec<usize>], option: usize, groups: bool) -> String {
    let first = options.get(option).and_then(|rows| rows.first()).and_then(|row| event.rows.get(*row));
    let named = first.and_then(|row| {
        if groups {
            group_of(row).map(str::to_string)
        } else {
            Some(format!("{} {}", weekday_name(weekday_of(row)?), clock(row.from?)))
        }
    });
    named.unwrap_or_else(|| format!("Gruppe {}", option + 1))
}

/// What is shown of the blocks in `t` now.
fn shown_of(t: &Timetable, blocks: &[Block]) -> Vec<BlockShown> {
    let hard = clash::hard_rows(&t.events);
    blocks
        .iter()
        .map(|block| {
            let none = BlockShown { exam: block.exam, index: block.index, ..BlockShown::default() };
            if block.exam {
                let Some(exam) = t.exams.get(block.index) else { return none };
                let items = block
                    .items
                    .iter()
                    .map(|item| match item {
                        Item::Line(line) => {
                            let rows = || line.rows.iter().filter_map(|row| exam.rows.get(*row));
                            ItemShown { hidden: rows().next().and_then(|row| row.hidden), second: rows().any(|row| row.rank == 2), ..ItemShown::default() }
                        }
                        _ => ItemShown::default(),
                    })
                    .collect();
                let reason = reason_text(exam.hidden, KindSet::default().with(EventKind::Exam), exam.town());
                return BlockShown { hidden: exam.hidden, reason, choice: None, items, ..none };
            }
            let Some(event) = t.events.get(block.index) else { return none };
            // The options that can be chosen: those with a Termin shown, or hidden only because
            // another option is chosen.
            let available: Vec<usize> = match &event.attendance {
                Attendance::OneOf { options, .. } => options
                    .iter()
                    .enumerate()
                    .filter(|(_, rows)| rows.iter().any(|row| event.rows.get(*row).is_some_and(|row| matches!(row.hidden, None | Some(HiddenBy::Choice)))))
                    .map(|(option, _)| option)
                    .collect(),
                Attendance::All => Vec::new(),
            };
            let choice = (event.hidden.is_none() && (available.len() >= 2 || event.chosen.is_some())).then_some((available.len(), event.chosen.is_some()));
            let can = |option: usize| choice.is_some() && available.contains(&option);
            let items = block
                .items
                .iter()
                .map(|item| match item {
                    Item::Line(line) => ItemShown {
                        hidden: line.rows.first().and_then(|row| event.rows.get(*row)).and_then(|row| row.hidden),
                        clash: line.rows.iter().any(|row| hard.contains(&(block.index, *row))),
                        choosable: line.choose.is_some_and(can),
                        chosen: line.choose.is_some() && line.choose == event.chosen,
                        second: false,
                    },
                    Item::Option { option, .. } => ItemShown { choosable: can(*option), chosen: event.chosen == Some(*option), ..ItemShown::default() },
                    _ => ItemShown::default(),
                })
                .collect();
            BlockShown { hidden: event.hidden, reason: reason_text(event.hidden, event.kinds, event.town()), choice, items, ..none }
        })
        .collect()
}

/// Why an event is hidden, when that is decided elsewhere: its kinds switched off in the sidebar
/// („Übungen ausgeblendet"), or the course of the other town („Standort Senftenberg", the town it
/// is held in).
fn reason_text(hidden: Option<HiddenBy>, kinds: KindSet, own: Option<Town>) -> Option<String> {
    match hidden? {
        HiddenBy::Kinds => {
            let words: Vec<&str> = kinds.iter().map(plural).collect();
            let text = match words.as_slice() {
                [] => "Ausgeblendet".to_string(),
                [one] => format!("{one} ausgeblendet"),
                [rest @ .., last] => format!("{} und {last} ausgeblendet", rest.join(", ")),
            };
            Some(text)
        }
        HiddenBy::Town(shown) => {
            let other = match shown {
                Town::Cottbus => Town::Senftenberg,
                Town::Senftenberg => Town::Cottbus,
            };
            Some(format!("Standort {}", own.unwrap_or(other).label()))
        }
        HiddenBy::Event | HiddenBy::Choice | HiddenBy::Row => None,
    }
}

/// A kind as the chips' plural says it switched off.
fn plural(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Lecture => "Vorlesungen",
        EventKind::Exercise => "Übungen",
        EventKind::Seminar => "Seminare",
        EventKind::Practical => "Praktika",
        EventKind::Project => "Projekte",
        EventKind::Tutorial => "Tutorien",
        EventKind::Consultation => "Konsultationen",
        EventKind::Excursion => "Exkursionen",
        EventKind::SelfStudy => "Selbststudium",
        EventKind::Paper => "Hausarbeiten",
        EventKind::Other => "Sonstige",
        EventKind::Exam => "Prüfungen",
    }
}

/// The module's notes in `t`, in the words of the semester's notes (A.5): its hard clashes, its
/// choices without a free option and its hard exam warnings first, then the quiet lines.
fn notes_of(t: &Timetable, id: &str, titles: &BTreeMap<String, String>, town: TownChoice) -> Vec<Note> {
    let own = |modules: &[String]| modules.iter().any(|module| module == id);
    let title = |module: &str| titles.get(module).cloned().unwrap_or_else(|| module.to_string());
    let (mut warnings, mut quiet): (Vec<Note>, Vec<Note>) = (Vec::new(), Vec::new());
    let mut add = |note: Note| {
        let list = if note.warn { &mut warnings } else { &mut quiet };
        if !list.contains(&note) {
            list.push(note);
        }
    };
    for event in t.blocked.iter().filter_map(|index| t.events.get(*index)).filter(|event| own(&event.modules)) {
        add(Note { warn: true, text: blocked_text(event), to: None });
    }
    let warned: Vec<&ExamWarning> = t.exam_warnings.iter().filter(|w| w.a.module_id == id || w.b.module_id == id).collect();
    let termine = if warned.is_empty() { Vec::new() } else { exams::termine(&t.exams, &t.modules) };
    for warning in warned {
        let other = if warning.a.module_id == id { &warning.b.module_id } else { &warning.a.module_id };
        let text = warning_text(warning, id, &termine, &title);
        add(Note { warn: warning.hard, text, to: Some((other.clone(), None)) });
    }
    for (day, modules) in t.place_unknown.iter().filter(|(_, modules)| own(modules)) {
        add(Note { warn: false, text: format!("Ort offen: {} Prüfungen am {}", modules.len(), dated(*day)), to: None });
    }
    if t.tracks.contains(id) && t.town.is_none() && town == TownChoice::Derive {
        add(Note { warn: false, text: "Standort wählen: Cottbus oder Senftenberg".to_string(), to: None });
    }
    warnings.extend(quiet);
    warnings
}

/// „0 von 4 Terminen frei: Übung · Entwicklung von Softwaresystemen", in the words of the
/// semester's notes (`head::blocked_line`), with the event's own title.
fn blocked_text(event: &Event) -> String {
    blocked_line(event, &event.title)
}

/// „Prüfungen gleichzeitig: Mo 08.02.2027 11:00 · Mathematik W-1 · ERP - Integrierte betriebliche
/// Systeme" or „0 min von Zentralcampus nach Senftenberg: Mo 08.02.2027 · Kraftwerkstechnik I bis
/// 10:00 · Gentechnik ab 10:00"; a warning another Termin avoids says which, from the module's
/// side (`avoid_text`).
fn warning_text(warning: &ExamWarning, id: &str, termine: &[(String, Vec<TerminAt>)], title: &dyn Fn(&str) -> String) -> String {
    let (a, b) = (&warning.a, &warning.b);
    let day = dated(warning.day);
    let mut text = match &warning.kind {
        WarningKind::Overlap => {
            format!("Prüfungen gleichzeitig: {day} {} · {} · {}", clock(a.from.max(b.from)), title(&a.module_id), title(&b.module_id))
        }
        WarningKind::Tight { gap, from, to } => format!(
            "{gap} min von {} nach {}: {day} · {} bis {} · {} ab {}",
            campus_name(from),
            campus_name(to),
            title(&a.module_id),
            clock(a.to),
            title(&b.module_id),
            clock(b.from)
        ),
    };
    if let Some(avoid) = warning.avoid.filter(|_| !warning.hard) {
        let (mine, theirs) = if a.module_id == id { (a, b) } else { (b, a) };
        let (my_termine, their_termine) = (termine_of(termine, &mine.module_id), termine_of(termine, &theirs.module_id));
        text.push_str(" · ");
        text.push_str(&avoid_text(warning.day, avoid, (mine, my_termine), (theirs, their_termine), &title(&theirs.module_id)));
    }
    text
}

/// The Termine of `module` in `exams::termine`'s list, earliest first.
fn termine_of<'a>(termine: &'a [(String, Vec<TerminAt>)], module: &str) -> &'a [TerminAt] {
    termine.iter().find(|(id, _)| id == module).map_or(&[], |(_, list)| list.as_slice())
}

/// What avoids a soft exam warning on `day`, from the module's side (the module page's words,
/// `pages::overlay`): its own Termin on the day `avoid` free of the other's („Zweittermin 11.03.
/// passt", „Erstermin 25.02. passt" where that is its earliest), else the other module's („Analysis
/// I am 25.02. passt"), else „andere Termine passen" (only a change of both avoids it).
fn avoid_text(day: Day, avoid: Day, mine: (&Termin, &[TerminAt]), theirs: (&Termin, &[TerminAt]), name: &str) -> String {
    let both = || "andere Termine passen".to_string();
    let my_issue = mine.1.iter().find(|at| at.day == day && at.termin == *mine.0);
    let their_issue = theirs.1.iter().find(|at| at.day == day && at.termin == *theirs.0);
    let (Some(my_issue), Some(their_issue)) = (my_issue, their_issue) else {
        return both();
    };
    let instead = |list: &[TerminAt], issue: &TerminAt, against: &TerminAt| {
        list.iter().position(|at| at.day == avoid && at != issue && exams::collision(at, against).is_none())
    };
    if let Some(index) = instead(mine.1, my_issue, their_issue) {
        let rank = if index == 0 { "Erstermin" } else { "Zweittermin" };
        return format!("{rank} {} passt", avoid.short());
    }
    if instead(theirs.1, their_issue, my_issue).is_some() {
        return format!("{name} am {} passt", avoid.short());
    }
    both()
}

/// A campus as a line names it: „Zentralcampus", „Sachsendorf", „Senftenberg".
fn campus_name(campus: &Code<Campus>) -> String {
    match campus.known() {
        Some(Campus::Zentralcampus) => "Zentralcampus".to_string(),
        Some(Campus::Sachsendorf) => "Sachsendorf".to_string(),
        Some(Campus::Senftenberg) => "Senftenberg".to_string(),
        Some(Campus::Nord) => "Cottbus Nord".to_string(),
        None => campus.label().to_string(),
    }
}

/// A Termin's two lines (`RowText`): „Mo 15:30–17:00 · LG 10/214 · A/B" and „14 Termine ·
/// Zentralcampus · 12.10.–25.01.". `rows` share one row key (the same slot in several rooms, or
/// with a later end in one of them): the latest end, every room, the days of all. A Termin that is
/// not weekly names its date instead of the weekday („Do 12.11. 13:45–15:15 · … · einmalig", a
/// block its first and last day); a range QIS leaves open is the lecture period's
/// („Vorlesungszeit", R12).
fn row_text(rows: &[&Row], group: Option<&str>) -> RowText {
    let Some(first) = rows.first() else {
        return RowText::default();
    };
    let date = &first.date;
    let rhythm = date.rhythm.as_ref().and_then(Code::known);
    let every = Every::of(date);
    let first_day = date.first_date.as_deref().and_then(Day::parse);
    let last_day = date.last_date.as_deref().and_then(Day::parse);
    let block = every.is_none() && rhythm == Some(Rhythm::Block);
    let single = every.is_none() && !block && (rhythm == Some(Rhythm::Single) || (first_day.is_some() && first_day == last_day));
    let stated = range(rows).filter(|_| !rows.iter().any(|row| row.occ.assumed));

    let from = rows.iter().filter_map(|row| row.from).min();
    let to = rows.iter().filter_map(|row| row.to).max();
    let time = match (from, to) {
        (Some(from), Some(to)) => format!("{}–{}", clock(from), clock(to)),
        _ if first.occ.all_day => "ganztags".to_string(),
        _ if date.start_time.is_some() => "Zeit unklar".to_string(),
        _ => "Zeit offen".to_string(),
    };
    // The day: the weekday of a weekly Termin, the date of a single one, the days of a block.
    let day = match (single, block) {
        (true, _) => first_day.map(|day| format!("{} {}", weekday_name(day.weekday()), day.short())),
        (false, true) => stated.map(|(a, b)| if a == b { a.short() } else { span(a, b) }),
        (false, false) => weekday_of(first).map(|weekday| weekday_name(weekday).to_string()),
    };
    let when = match day {
        Some(day) => format!("{day} {time}"),
        None => time,
    };
    let week = match (every, rhythm) {
        _ if single => "einmalig".to_string(),
        (Some(Every::Week), _) => "A/B".to_string(),
        (Some(Every::AWeek), _) => "A".to_string(),
        (Some(Every::BWeek), _) => "B".to_string(),
        (Some(Every::FourWeeks), _) => "4-wöchentlich".to_string(),
        (None, Some(Rhythm::Block)) => "Block".to_string(),
        (None, Some(Rhythm::Other)) => date.rhythm_raw.as_deref().map(str::trim).filter(|raw| !raw.is_empty()).unwrap_or("nach Absprache").to_string(),
        (None, _) => "Rhythmus offen".to_string(),
    };

    let days: BTreeSet<Day> = rows.iter().flat_map(|row| row.occ.days.iter().copied()).collect();
    let cancelled: BTreeSet<Day> =
        rows.iter().flat_map(|row| row.occ.cancelled.iter().map(|(day, ..)| *day)).filter(|day| !days.contains(day)).collect();
    let mut detail: Vec<String> = group.map(str::to_string).into_iter().collect();
    if !single && !days.is_empty() {
        detail.push(match days.len() {
            1 => "1 Termin".to_string(),
            n => format!("{n} Termine"),
        });
    }
    match (single, cancelled.len()) {
        (_, 0) => {}
        (true, _) => detail.push("fällt aus".to_string()),
        (false, 1) => detail.push("1 fällt aus".to_string()),
        (false, n) => detail.push(format!("{n} fallen aus")),
    }
    detail.extend(campuses(rows.iter().map(|row| row.date.campus.as_ref())));
    if !single && !block {
        if rows.iter().any(|row| row.occ.assumed) {
            detail.push("Vorlesungszeit".to_string());
        } else if let Some((a, b)) = stated {
            detail.push(if a == b { a.short() } else { format!("{}–{}", a.short(), b.short()) });
        }
    }
    RowText { when, room: rooms(rows.iter().map(|row| row.date.room.as_deref())), week: Some(week), detail }
}

/// An exam sitting's line: „Fr 12.03.2027 11:00–13:00", a window „08.–19.02. nach Absprache",
/// a deadline „So 14.02. bis 24:00", „Termin offen"; with its room, and its campus under it.
fn exam_text(rows: &[&ExamRow]) -> RowText {
    let Some(first) = rows.first() else {
        return RowText::default();
    };
    let when = match &first.shape {
        ExamShape::Sitting { day, from, to } => {
            let latest = rows.iter().filter_map(|row| if let ExamShape::Sitting { to, .. } = row.shape { Some(to) } else { None }).max().unwrap_or(*to);
            format!("{} {}–{}", dated(*day), clock(*from), clock(latest))
        }
        ExamShape::Deadline { day } => format!("{} {} bis 24:00", weekday_name(day.weekday()), day.short()),
        ExamShape::Window { first, last } => format!("{} nach Absprache", span(*first, *last)),
        ExamShape::DayOnly { day } => format!("{} Uhrzeit offen", dated(*day)),
        ExamShape::Open => "Termin offen".to_string(),
    };
    let detail = campuses(rows.iter().map(|row| row.date.campus.as_ref())).into_iter().collect();
    RowText { when, room: rooms(rows.iter().map(|row| row.date.room.as_deref())), week: None, detail }
}

/// Every room, each once, in order, without the campus QIS ends it with (the second line names
/// it): „Verfügungsgebäude 1C - 0.03 / LG 10/214".
fn rooms<'a>(rooms: impl Iterator<Item = Option<&'a str>>) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for room in rooms.flatten().map(without_campus).filter(|room| !room.is_empty()) {
        if !seen.contains(&room) {
            seen.push(room);
        }
    }
    (!seen.is_empty()).then(|| seen.join(" / "))
}

/// A room as QIS writes it without its last part where that is the campus: „Verfügungsgebäude
/// 1C - 0.03 - Zentralcampus", „… - Campus Senftenberg".
fn without_campus(room: &str) -> &str {
    let room = room.trim();
    match room.rsplit_once(" - ") {
        Some((rest, last)) if last.to_lowercase().contains("campus") && !rest.trim().is_empty() => rest.trim(),
        _ => room,
    }
}

/// The campuses of a Termin, each once: „Zentralcampus".
fn campuses<'a>(codes: impl Iterator<Item = Option<&'a Code<Campus>>>) -> Option<String> {
    let mut seen: Vec<String> = Vec::new();
    for name in codes.flatten().map(campus_name) {
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    (!seen.is_empty()).then(|| seen.join(" / "))
}

/// The stated range of rows: the earliest first date to the latest last date.
fn range(rows: &[&Row]) -> Option<(Day, Day)> {
    let first = rows.iter().map(|row| row.date.first_date.as_deref().and_then(Day::parse)).collect::<Option<Vec<_>>>()?;
    let last = rows.iter().map(|row| row.date.last_date.as_deref().and_then(Day::parse)).collect::<Option<Vec<_>>>()?;
    Some((*first.iter().min()?, *last.iter().max()?))
}

/// „Fr 12.03.2027".
fn dated(day: Day) -> String {
    format!("{} {}", weekday_name(day.weekday()), day.german())
}

/// „08.–19.02." within a month, else „28.01.–05.02.".
fn span(first: Day, last: Day) -> String {
    let ((first_year, first_month, first_day), (last_year, last_month, _)) = (first.ymd(), last.ymd());
    if (first_year, first_month) == (last_year, last_month) {
        format!("{first_day:02}.–{}", last.short())
    } else {
        format!("{}–{}", first.short(), last.short())
    }
}

fn weekday_name(weekday: u8) -> &'static str {
    WEEKDAYS.get(usize::from(weekday).wrapping_sub(1)).copied().unwrap_or("")
}

/// A row's weekday as QIS states it, 1 = Monday.
fn weekday_of(row: &Row) -> Option<u8> {
    row.date.weekday.and_then(|weekday| u8::try_from(weekday).ok()).filter(|weekday| (1..=7).contains(weekday))
}

/// The group QIS names; its „[unbenannt]" is none.
fn group_of(row: &Row) -> Option<&str> {
    row.date.group_name.as_deref().map(str::trim).filter(|name| !name.is_empty() && *name != UNNAMED_GROUP)
}

/// What an event is called in a line: its type as QIS writes it („Übung", „Vorlesung/Übung"),
/// else its first kind.
fn kind_word(event: &Event) -> String {
    match event.type_raw.as_deref().map(str::trim).filter(|kind| !kind.is_empty()) {
        Some(kind) => kind.to_string(),
        None => event.kinds.iter().next().map_or("Termin", EventKind::label).to_string(),
    }
}

/// An event's head: what it is, and its title where that is not the module's own (the panel's
/// title says it already).
fn head_text(what: &str, title: &str, module: Option<&str>) -> String {
    let title = title.trim();
    if title.is_empty() || module.is_some_and(|module| fold(module) == fold(title)) {
        what.to_string()
    } else {
        format!("{what} · {title}")
    }
}

/// A `veranstid` as the store keeps it: digits without a leading zero that fit a `u32`.
fn event_number(id: &str) -> Option<u32> {
    let canonical = !id.is_empty() && !id.starts_with('0') && id.bytes().all(|byte| byte.is_ascii_digit());
    canonical.then(|| id.parse().ok()).flatten()
}

/// The events and exams only `id` links in `t`: what the store keeps hidden or chosen of them
/// goes when the module is removed from the semester (B.2).
fn only_its_events(t: &Timetable, id: &str) -> Vec<u32> {
    let only = |modules: &[String]| matches!(modules, [only] if only == id);
    t.events
        .iter()
        .filter(|event| only(&event.modules))
        .map(|event| event.id.as_str())
        .chain(t.exams.iter().filter(|exam| only(&exam.modules)).map(|exam| exam.event_id.as_str()))
        .filter_map(event_number)
        .collect()
}

/// The placeholders a module planned in `sem` can count for: those whose span reaches `sem`, and
/// the one it counts for now, wherever that stands.
fn placeholder_choices(doc: &PlanDoc, sem: SemesterKey, start: Option<SemesterKey>, fills: Option<u32>) -> Vec<(u32, String)> {
    doc.placeholders
        .iter()
        .filter(|p| Some(p.pid) == fills || covers(p, sem, start))
        .map(|p| (p.pid, format!("Für „{}“", p.name)))
        .collect()
}

/// Whether a placeholder spans `sem`: it stands in its first semester, and runs to the last
/// Fachsemester of its span (counted from the Studienbeginn, else from where it stands).
fn covers(p: &Placeholder, sem: SemesterKey, start: Option<SemesterKey>) -> bool {
    if sem < p.semester {
        return false;
    }
    let last = start
        .and_then(|start| of_fachsemester(start, p.span.1))
        .filter(|last| *last >= p.semester)
        .or_else(|| p.semester.plus(i32::from(p.span.1.saturating_sub(p.span.0))))
        .unwrap_or(p.semester);
    sem <= last
}

#[cfg(test)]
mod tests {
    use catalog::rows_detail::{DateRow, EventDate, ModuleSws};
    use catalog::timetable::facts::SemesterFacts;
    use catalog::timetable::model::Input;
    use catalog::timetable::select::Selection;

    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn day(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    /// 2026W as the data says it: lectures 05.10.2026–31.01.2027, the break 21.12.–03.01., A weeks
    /// from the first week.
    fn winter() -> SemesterFacts {
        SemesterFacts {
            lecture: Some((day("2026-10-05"), day("2027-01-31"))),
            breaks: vec![(day("2026-12-21"), day("2027-01-03"))],
            a_week: Some(day("2026-10-05")),
            ..SemesterFacts::derive(key("2026W"), None, &[])
        }
    }

    /// A teaching row of 12104 in 2026W, weekly from the first week on its weekday; the methods
    /// change what a case needs.
    struct Fx(DateRow);

    fn teaching(event: &str, ord: i64, kind: &str, weekday: i64, from: &str, to: &str) -> Fx {
        let first = day("2026-10-05").plus(i32::try_from(weekday).unwrap() - 1);
        Fx(DateRow {
            module_id: "12104".into(),
            ord: Some(ord),
            cancelled_dates: None,
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: event.into(),
                event_number: None,
                event_title: "Entwicklung von Softwaresystemen".into(),
                event_type: Some(kind.into()),
                group_name: Some(UNNAMED_GROUP.into()),
                weekday: Some(weekday),
                start_time: Some(from.into()),
                end_time: Some(to.into()),
                rhythm: Some(Code::parse("weekly")),
                rhythm_raw: None,
                first_date: Some(first.iso()),
                last_date: Some(first.plus(7 * 16).iso()),
                room: Some(format!("Raum {event}/{ord}")),
                campus: Some(Code::parse("zentralcampus")),
                instructor: None,
                comment: None,
                source_url: Some(format!("https://qis.example/{event}")),
            },
        })
    }

    impl Fx {
        fn module(mut self, module: &str) -> Self {
            self.0.module_id = module.into();
            self
        }

        fn title(mut self, title: &str) -> Self {
            self.0.date.event_title = title.into();
            self
        }

        fn range(mut self, first: &str, last: &str) -> Self {
            self.0.date.first_date = Some(first.into());
            self.0.date.last_date = Some(last.into());
            self
        }

        fn rhythm(mut self, rhythm: &str) -> Self {
            self.0.date.rhythm = Some(Code::parse(rhythm));
            self
        }

        fn room(mut self, room: &str) -> Self {
            self.0.date.room = Some(room.into());
            self
        }

        fn group(mut self, group: &str) -> Self {
            self.0.date.group_name = Some(group.into());
            self
        }

        fn cancelled(mut self, text: &str) -> Self {
            self.0.cancelled_dates = Some(text.into());
            self
        }

        fn undated(mut self) -> Self {
            (self.0.date.first_date, self.0.date.last_date) = (None, None);
            self
        }
    }

    /// An exam row of 12104 on one day, in the Audimax; an empty time is none.
    fn exam(event: &str, ord: i64, on: &str, from: &str, to: &str) -> DateRow {
        let time = |t: &str| Some(t.to_string()).filter(|t| !t.is_empty());
        DateRow {
            module_id: "12104".into(),
            ord: Some(ord),
            cancelled_dates: None,
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: event.into(),
                event_number: None,
                event_title: "Entwicklung von Softwaresystemen".into(),
                event_type: None,
                group_name: None,
                weekday: Day::parse(on).map(|d| i64::from(d.weekday())),
                start_time: time(from),
                end_time: time(to),
                rhythm: None,
                rhythm_raw: None,
                first_date: Some(on.into()),
                last_date: Some(on.into()),
                room: Some("Zentrales Hörsaalgebäude - Audimax 1 - Zentralcampus".into()),
                campus: Some(Code::parse("zentralcampus")),
                instructor: None,
                comment: None,
                source_url: None,
            },
        }
    }

    fn table(rows: &[Fx], exams: &[DateRow], modules: &[&str], sws: &[ModuleSws], selection: &Selection) -> Timetable {
        let facts = winter();
        let schedule: Vec<DateRow> = rows.iter().map(|row| row.0.clone()).collect();
        let modules: Vec<String> = modules.iter().map(|id| id.to_string()).collect();
        let input = Input { key: facts.key, semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams, sws };
        Timetable::build(&input, selection)
    }

    /// The four Übung slots of 148369 (2 SWS, 90 minutes each): one of them is attended.
    fn uebung() -> Vec<Fx> {
        vec![
            teaching("148369", 1, "Übung", 1, "15:30", "17:00").range("2026-10-12", "2027-01-25").room("LG 10/214"),
            teaching("148369", 2, "Übung", 1, "17:30", "19:00").range("2026-10-12", "2027-01-25"),
            teaching("148369", 3, "Übung", 2, "15:30", "17:00").range("2026-10-13", "2027-01-26"),
            teaching("148369", 4, "Übung", 2, "17:30", "19:00").range("2026-10-13", "2027-01-26"),
        ]
    }

    fn two_sws() -> Vec<ModuleSws> {
        vec![ModuleSws { module_id: "12104".into(), form: Code::parse("exercise"), sws: 2.0 }]
    }

    /// A block as it reads, a line that is a choice with its radio mark.
    fn texts(block: &Block) -> Vec<String> {
        block
            .items
            .iter()
            .map(|item| match item {
                Item::Line(line) => format!("{}{}", if line.choose.is_some() { "( ) " } else { "" }, line.text.text()),
                Item::Choice => "1 von n wählen".to_string(),
                Item::Option { name, .. } => format!("( ) {name}"),
                Item::Undated => "Termine nicht angegeben".to_string(),
            })
            .collect()
    }

    #[test]
    fn a_termin_reads_when_where_which_weeks_and_under_it_how_many() {
        let t = table(&uebung(), &[], &["12104"], &two_sws(), &Selection::default());
        let first = t.events.first().unwrap().rows.first().unwrap();
        // Twice in the break (21.12., 28.12.): 14 of 16 Mondays are held.
        assert_eq!(row_text(&[first], None).text(), "Mo 15:30–17:00 · LG 10/214 · A/B | 14 Termine · Zentralcampus · 12.10.–25.01.");
        // The day and the time are bold, the rest follows; how many and where under it.
        let text = row_text(&[first], None);
        assert_eq!((text.when.as_str(), text.rest().as_deref()), ("Mo 15:30–17:00", Some("LG 10/214 · A/B")));

        // A-weeks, a group, a cancellation; the campus QIS ends the room with is the second line's.
        let rows = [teaching("1", 1, "Übung", 2, "07:30", "09:00")
            .rhythm("week_a")
            .group("1-Gruppe")
            .cancelled("20.10.2026: krank")
            .room("Hauptgebäude - HG 0.20 - Zentralcampus")];
        let t = table(&rows, &[], &["12104"], &[], &Selection::default());
        let row = t.events.first().unwrap().rows.first().unwrap();
        assert_eq!(
            row_text(&[row], group_of(row)).text(),
            "Di 07:30–09:00 · Hauptgebäude - HG 0.20 · A | 1-Gruppe · 7 Termine · 1 fällt aus · Zentralcampus · 06.10.–26.01."
        );

        // A single date names its date, not a weekday and a count.
        let rows = [teaching("2", 1, "Übung", 2, "11:45", "13:15").rhythm("single").range("2027-02-23", "2027-02-23").room("HG 0.20")];
        let t = table(&rows, &[], &["12104"], &[], &Selection::default());
        let row = t.events.first().unwrap().rows.first().unwrap();
        assert_eq!(row_text(&[row], None).text(), "Di 23.02. 11:45–13:15 · HG 0.20 · einmalig | Zentralcampus");

        // A block names its days.
        let rows = [teaching("8", 1, "Übung", 1, "09:00", "16:00").rhythm("block").range("2027-02-22", "2027-02-26").room("HG 0.20")];
        let t = table(&rows, &[], &["12104"], &[], &Selection::default());
        let row = t.events.first().unwrap().rows.first().unwrap();
        assert!(row_text(&[row], None).text().starts_with("22.–26.02. 09:00–16:00 · HG 0.20 · Block | "), "{}", row_text(&[row], None).text());

        // Without a range the lecture period stands in, and the line says so.
        let rows = [teaching("3", 1, "Übung", 3, "09:15", "10:45").undated()];
        let t = table(&rows, &[], &["12104"], &[], &Selection::default());
        let row = t.events.first().unwrap().rows.first().unwrap();
        assert_eq!(row_text(&[row], None).text(), "Mi 09:15–10:45 · Raum 3/1 · A/B | 15 Termine · Zentralcampus · Vorlesungszeit");

        // One Termin in two rooms, one of them ending later: one line, the latest end, both rooms.
        let mut late = teaching("4", 2, "Übung", 4, "09:15", "10:45").room("B");
        late.0.date.end_time = Some("11:30".into());
        let rows = [teaching("4", 1, "Übung", 4, "09:15", "10:45").room("A"), late];
        let t = table(&rows, &[], &["12104"], &[], &Selection::default());
        let block = blocks_of(&t, "12104", Some("Entwicklung von Softwaresystemen")).remove(0);
        assert_eq!(texts(&block), vec!["Do 09:15–11:30 · A / B · A/B | 15 Termine · Zentralcampus · 08.10.–28.01.".to_string()]);
        assert_eq!(without_campus("Gebäude 14.E - SFB - 14E.202 Rechner- Pool II - Campus Senftenberg"), "Gebäude 14.E - SFB - 14E.202 Rechner- Pool II");
        assert_eq!(without_campus(" LG 10/214 "), "LG 10/214");
    }

    #[test]
    fn the_line_of_an_option_is_its_choice_open_or_made() {
        let t = table(&uebung(), &[], &["12104"], &two_sws(), &Selection::default());
        let blocks = blocks_of(&t, "12104", Some("Entwicklung von Softwaresystemen"));
        let block = blocks.first().unwrap();
        // The event's title is the module's: its head says only what it is.
        assert_eq!(block.head, "Übung");
        assert_eq!(
            texts(block),
            vec![
                "1 von n wählen",
                "( ) Mo 15:30–17:00 · LG 10/214 · A/B | 14 Termine · Zentralcampus · 12.10.–25.01.",
                "( ) Mo 17:30–19:00 · Raum 148369/2 · A/B | 14 Termine · Zentralcampus · 12.10.–25.01.",
                "( ) Di 15:30–17:00 · Raum 148369/3 · A/B | 14 Termine · Zentralcampus · 13.10.–26.01.",
                "( ) Di 17:30–19:00 · Raum 148369/4 · A/B | 14 Termine · Zentralcampus · 13.10.–26.01.",
            ]
        );
        let shown = shown_of(&t, &blocks);
        // Each state names its block: the keyed list finds it by that, not by its place.
        assert_eq!(shown.iter().map(|s| (s.exam, s.index)).collect::<Vec<_>>(), blocks.iter().map(|b| (b.exam, b.index)).collect::<Vec<_>>());
        assert_eq!(shown.first().unwrap().choice, Some((4, false)));
        assert!(shown.first().unwrap().items.iter().skip(1).all(|item| item.choosable && !item.chosen));

        // One Termin hidden: 1 of 3, and the hidden one cannot be chosen.
        let hidden = RowKey::of(&uebung().get(1).unwrap().0.date).unwrap();
        let selection = Selection { hidden_rows: BTreeSet::from([hidden]), ..Selection::default() };
        let t = table(&uebung(), &[], &["12104"], &two_sws(), &selection);
        let shown = shown_of(&t, &blocks_of(&t, "12104", None));
        let first = shown.first().unwrap();
        assert_eq!(first.choice, Some((3, false)));
        assert_eq!(first.items.get(2).map(|item| (item.hidden, item.choosable)), Some((Some(HiddenBy::Row), false)));

        // Mo 15:30 chosen: every option stays listed, the one chosen is pressed, and the others
        // can still be chosen instead (or the choice opened again).
        let chosen = RowKey::of(&uebung().first().unwrap().0.date).unwrap();
        let selection = Selection { chosen_rows: BTreeSet::from([chosen]), ..Selection::default() };
        let t = table(&uebung(), &[], &["12104"], &two_sws(), &selection);
        let after = blocks_of(&t, "12104", Some("Entwicklung von Softwaresystemen"));
        assert_eq!(after, blocks, "a choice changes what is shown, not what is listed");
        let shown = shown_of(&t, &after);
        let first = shown.first().unwrap();
        assert_eq!(first.choice, Some((4, true)));
        let states: Vec<(bool, bool, Option<HiddenBy>)> = first.items.iter().skip(1).map(|item| (item.chosen, item.choosable, item.hidden)).collect();
        assert_eq!(
            states,
            vec![(true, true, None), (false, true, Some(HiddenBy::Choice)), (false, true, Some(HiddenBy::Choice)), (false, true, Some(HiddenBy::Choice))]
        );
    }

    #[test]
    fn groups_of_several_termine_are_chosen_under_their_name() {
        let rows = [
            teaching("5", 1, "Übung", 1, "09:15", "10:45").group("Gruppe A").title("Übung zu EvS"),
            teaching("5", 2, "Übung", 3, "09:15", "10:45").group("Gruppe A").title("Übung zu EvS"),
            teaching("5", 3, "Übung", 2, "13:45", "15:15").group("Gruppe B").title("Übung zu EvS"),
        ];
        let t = table(&rows, &[], &["12104"], &[], &Selection::default());
        let block = blocks_of(&t, "12104", Some("Entwicklung von Softwaresystemen")).remove(0);
        assert_eq!(block.head, "Übung · Übung zu EvS");
        assert_eq!(
            texts(&block),
            vec![
                "1 von n wählen",
                "( ) Gruppe A",
                "Mo 09:15–10:45 · Raum 5/1 · A/B | 15 Termine · Zentralcampus · 05.10.–25.01.",
                "Mi 09:15–10:45 · Raum 5/2 · A/B | 15 Termine · Zentralcampus · 07.10.–27.01.",
                "( ) Di 13:45–15:15 · Raum 5/3 · A/B | Gruppe B · 15 Termine · Zentralcampus · 06.10.–26.01.",
            ]
        );
    }

    #[test]
    fn what_hides_an_event_elsewhere_is_said_once() {
        let rows = [teaching("6", 1, "Vorlesung/Übung", 1, "09:15", "10:45")];
        let hidden = KindSet::default().with(EventKind::Lecture).with(EventKind::Exercise);
        let t = table(&rows, &[], &["12104"], &[], &Selection { hidden_kinds: hidden, ..Selection::default() });
        let shown = shown_of(&t, &blocks_of(&t, "12104", None));
        let first = shown.first().unwrap();
        assert_eq!(first.reason.as_deref(), Some("Vorlesungen und Übungen ausgeblendet"));
        assert_eq!(first.items.first().unwrap().hidden, Some(HiddenBy::Kinds));
        let town = reason_text(Some(HiddenBy::Town(Town::Cottbus)), KindSet::default(), Some(Town::Senftenberg));
        assert_eq!(town.as_deref(), Some("Standort Senftenberg"));
        assert_eq!(reason_text(Some(HiddenBy::Event), KindSet::default(), None), None);
    }

    #[test]
    fn exam_sittings_read_as_their_shape() {
        let exams = [exam("148689", 1, "2027-03-12", "11:00", "13:00"), exam("148689", 2, "2027-03-26", "", "")];
        let t = table(&[teaching("7", 1, "Vorlesung", 2, "11:30", "13:00")], &exams, &["12104"], &[], &Selection::default());
        let blocks = blocks_of(&t, "12104", Some("Entwicklung von Softwaresystemen"));
        let sittings = blocks.iter().find(|block| block.exam).unwrap();
        assert_eq!(sittings.head, "Prüfung");
        assert_eq!(
            texts(sittings),
            vec![
                "Fr 12.03.2027 11:00–13:00 · Zentrales Hörsaalgebäude - Audimax 1 | Zentralcampus",
                "Fr 26.03.2027 Uhrzeit offen · Zentrales Hörsaalgebäude - Audimax 1 | Zentralcampus",
            ]
        );
        assert_eq!(span(day("2027-02-08"), day("2027-02-19")), "08.–19.02.");
        assert_eq!(span(day("2027-01-28"), day("2027-02-05")), "28.01.–05.02.");
    }

    #[test]
    fn a_clash_is_no_note_of_the_panel() {
        let rows = [
            teaching("149408", 1, "Vorlesung", 2, "07:30", "09:00").module("12102").title("Programmierpraktikum"),
            teaching("148134", 1, "Vorlesung", 2, "07:30", "09:00")
                .module("12107")
                .title("Elektrische und elektronische Grundlagen der Informatik")
                .rhythm("week_a"),
        ];
        let t = table(&rows, &[], &["12102", "12107"], &[], &Selection::default());
        let titles = BTreeMap::from([("12102".to_string(), "Programmierpraktikum".to_string())]);
        // The clash stands at its rows („Überschneidung") and in the week, not as a card here
        // (owner's redesign of 2026-09-25).
        assert!(notes_of(&t, "12102", &titles, TownChoice::Derive).is_empty());
        // A module without a clash has no note of it.
        assert!(notes_of(&t, "12104", &titles, TownChoice::Derive).is_empty());
        // Removing 12102 takes what the store keeps of its own event along, not 12107's.
        assert_eq!(only_its_events(&t, "12102"), vec![149408]);
    }

    #[test]
    fn a_semester_without_the_modules_dates_says_why() {
        let info = |id: &str, sem: &str, dated: bool, turnus: Option<TurnusSeason>| Info {
            place: Place {
                id: id.into(),
                shown: key(sem),
                sem: Some(key(sem)),
                ids: vec![id.into()],
                current: Some(key("2026W")),
                town: TownChoice::Derive,
            },
            title: None,
            credits: None,
            turnus,
            missing: false,
            dated,
            titles: BTreeMap::new(),
            error: None,
        };
        // 12104 has a Termin, 11103 none.
        let t = table(&[teaching("7", 1, "Vorlesung", 2, "11:30", "13:00")], &[], &["12104", "11103"], &[], &Selection::default());
        let lead = |info: Info| lead_of(&info, &t, info.place.sem.unwrap());
        // A semester before the current one is over, whether the data still has dates of it or
        // none at all (retention removes them).
        let over = Some("WiSe 2025/26 ist vorbei; vergangene Termine fehlen im Datenstand.".to_string());
        assert_eq!(lead(info("11103", "2025W", false, None)), over);
        assert_eq!(lead(info("11103", "2025W", true, None)), over);
        assert_eq!(lead(info("12104", "2025W", false, None)), over);
        assert_eq!(lead(info("12104", "2025W", true, None)), None);
        // A later one without any data: its dates are still to come.
        assert_eq!(lead(info("11103", "2027S", false, None)).as_deref(), Some("SoSe 2027: noch keine Termine veröffentlicht."));
        // A semester with data but none of the module's, and what its description says.
        assert_eq!(
            lead(info("11103", "2026W", true, Some(TurnusSeason::Summer))).as_deref(),
            Some("Keine Termine im WiSe 2026/27 (laut Beschreibung im Sommer).")
        );
        assert_eq!(lead(info("11103", "2026W", true, Some(TurnusSeason::Both))).as_deref(), Some("Keine Termine im WiSe 2026/27."));
        assert_eq!(lead(info("12104", "2026W", true, None)), None);
    }

    #[test]
    fn a_failed_query_is_an_error_not_a_module_the_catalog_lacks() {
        let place = Place { id: "11103".into(), shown: key("2026W"), sem: None, ids: Vec::new(), current: Some(key("2026W")), town: TownChoice::Derive };
        let error = DataError { unavailable: false, message: "database is locked".into() };
        let failed = info_nowhere(&place, Err(&error));
        assert!(!failed.missing);
        let shown = shown_of_panel(&failed, None);
        assert_eq!((shown.body.error, shown.body.lead), (Some(error), None));
        let unknown = info_nowhere(&place, Ok(None));
        assert!(unknown.missing);
        assert_eq!(shown_of_panel(&unknown, None).body.lead.as_deref(), Some("Nicht im Studienplan."));
    }

    #[test]
    fn the_panel_shows_a_module_where_it_is_planned() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2026W"), "12104", 1, None));
        assert!(doc.plan(key("2027S"), "12204", 1, None) && doc.plan(key("2027W"), "12204", 2, None));
        assert_eq!(home_semester(&doc, key("2026W"), "12104"), Some(key("2026W")));
        // The Übersicht shows the default semester: a module of another one is shown in its own.
        assert_eq!(home_semester(&doc, key("2026W"), "12204"), Some(key("2027S")));
        assert_eq!(home_semester(&doc, key("2028S"), "12204"), Some(key("2027W")));
        assert_eq!(home_semester(&doc, key("2026W"), "11103"), None);
    }

    #[test]
    fn a_module_counts_for_the_placeholders_its_semester_has() {
        let placeholder = |pid: u32, semester: &str, span: (u8, u8), name: &str| Placeholder {
            pid,
            semester: key(semester),
            program_id: "079-82-2008".into(),
            ord: i64::from(pid),
            span,
            credits: Some("6".into()),
            kind: None,
            caption: String::new(),
            name: name.into(),
        };
        let doc = PlanDoc {
            placeholders: vec![
                placeholder(1, "2026W", (1, 1), "Fachübergreifendes Studium"),
                placeholder(2, "2026W", (1, 3), "Anwendungsfach"),
                placeholder(3, "2027S", (2, 2), "Wahlpflichtmodul 1"),
            ],
            ..PlanDoc::default()
        };
        let pids = |choices: Vec<(u32, String)>| choices.into_iter().map(|(pid, _)| pid).collect::<Vec<_>>();
        assert_eq!(pids(placeholder_choices(&doc, key("2026W"), Some(key("2026W")), None)), vec![1, 2]);
        // The span of FS 1–3 reaches the summer; counted from where it stands it does too.
        assert_eq!(pids(placeholder_choices(&doc, key("2027S"), Some(key("2026W")), None)), vec![2, 3]);
        assert_eq!(pids(placeholder_choices(&doc, key("2027S"), None, None)), vec![2, 3]);
        // The one it counts for stays offered wherever it stands.
        assert_eq!(pids(placeholder_choices(&doc, key("2027W"), Some(key("2026W")), Some(1))), vec![1, 2]);
        let first = placeholder_choices(&doc, key("2026W"), None, None);
        assert_eq!(first.first().map(|(_, label)| label.as_str()), Some("Für „Fachübergreifendes Studium“"));
    }
}
