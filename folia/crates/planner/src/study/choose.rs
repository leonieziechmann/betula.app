//! „Wahlpflicht", a view of „Mein Studium" (`/study?view=areas`; owner, 2026-10-14, concept A
//! „Ansichten"): what the plan leaves to choose (`folia_plans::choices`), to look up how much is
//! asked and what there is (folia/design/wahlpflicht, A as built after its second round; owner,
//! 2026-11-04: „Hier ist wirklich einfach nur der lookup, wie viel brauche ich aus dem bereich und
//! was gibt es da"). No semester and no button to plan: a module opens its sidebar, whose
//! „Einplanen" plans it for the choice it was opened from (`StudyCtx::aside_hint`).
//!
//! On a desktop alone in the page (the overview and its bar stay with the semesters): the choices
//! on the left by area, in the plan's order, each with its figure, its bar and how many modules
//! there are; the one picked on the right — its name, its stand, what it asks, its lists as tabs
//! and their modules. On a phone the list of the choices, and each a page of its own behind it
//! (`StudyUrl::choice`), its modules as cards; a module opens as the sheet the sidebar is there.
//! What is done looks as what is open, its bar full (owner, the same day: „Was zur hölle ist die
//! anzeige vollständig … Mach doch einfach die progress bar voll").
//!
//! The choices the regulation sums over are one entry (`choices::Group`): the Komplexe des
//! Fachstudiums of Informatik B.Sc., 50 LP of the three, each 10 to 24, the one over 24 to 30 — its
//! bar fills the hatched stretch above 24, which the others' then lose (owner: „nur eins davon darf
//! 30 haben"). What is on the Merkliste comes first in a list and carries the bookmark.

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::semester::SemesterKey;
use folia_model::labels::{Labelled, OfferStatus};
use folia_model::rows::CatalogRow;
use folia_pages::ask::StudyModulesAsk;
use folia_plans::choices::{self, Choice, PlanView, Room, Stand};
use folia_plans::study::{self, Input};
use folia_plans::studyplan::RowRef;
use folia_plans::{bench, variants};
use folia_routes::url::{CatalogUrl, LocalView, StudyUrl};
use folia_shell::chrome::{Back, TopBack};
use folia_stores::bookmarks::Bookmarks;
use folia_stores::studyplan::PlanHint;
use leptos::prelude::*;

use folia_design::ui::{EmptyState, Icon};

use super::focus::{Glyph, Status};
use super::{n, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// Where a module of a list stands for the student, and what it counts for: an entry of the view.
#[derive(Clone, Debug, PartialEq)]
struct Has {
    status: Status,
    semester: SemesterKey,
    entry: Option<usize>,
}

/// A list of an entry, as its tab shows it.
#[derive(Clone, Debug, PartialEq)]
struct Tab {
    name: String,
    modules: Vec<String>,
    /// A group's member: its stand and its room above its most.
    member: Option<(Stand, Room)>,
    /// What counts for it that its lists do not have: what the student let count here as an
    /// exception (`choices::Taken::exception`), shown first, marked as such.
    others: Vec<String>,
    /// What a module of it opened from here is planned for: the first placeholder of its rows
    /// from the current semester on that nothing fills, and the row it counts for, with its name.
    fill: Option<u32>,
    counts: RowRef,
    counts_name: String,
}

/// What the Wahlpflicht lists: a choice of the plan, or the choices the regulation sums over.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    /// Its first choice (an index of `PlanView::choices`): what picks it.
    choice: usize,
    choices: Vec<usize>,
    name: String,
    area: Option<usize>,
    tone: &'static str,
    /// What it asks, in a sentence.
    rule: String,
    stand: Stand,
    /// A group's room above its members' most, which one of them keeps (`Room`).
    room: f64,
    /// „17 Module", „5 Listen · 59 Module".
    count: String,
    tabs: Vec<Tab>,
    /// The FÜS's list is the BTU's catalog; where it has none, the catalog of its row.
    catalog: Option<(String, &'static str)>,
    no_list: bool,
    /// Its placeholders from the current semester on that no module fills.
    places: Vec<SemesterKey>,
}

/// An area of the progress with its entries, in the plan's order.
#[derive(Clone, Debug, PartialEq)]
struct Area {
    name: String,
    tone: &'static str,
    /// „12 / 18 LP": what its entries hold of what they ask.
    figure: String,
    entries: Vec<usize>,
}

/// What the view shows.
#[derive(Clone, Debug, PartialEq)]
struct Pane {
    lead: String,
    areas: Vec<Area>,
    entries: Vec<Entry>,
    /// Every module of the lists the study holds, and where it stands.
    has: BTreeMap<String, Has>,
    /// The first entry with something open, else the first.
    first: usize,
}

impl Pane {
    fn of(input: &Input, ready: &Ready, ctx: StudyCtx, t: &'static Texts) -> Self {
        let v = &t.study_views;
        let study = &ready.study;
        let view: PlanView = choices::plan_view(input, study);
        let now = study.now;
        let doc = input.doc;
        let mut entries: Vec<Entry> = Vec::new();
        for (at, choice) in view.choices.iter().enumerate() {
            if entries.iter().any(|entry| entry.choices.contains(&at)) {
                continue;
            }
            let entry = match view.group_of(at) {
                Some(group) => group_entry(input, ready, &view, group, now, t),
                None => choice_entry(input, ready, ctx, &view, at, choice, now, t),
            };
            entries.push(entry);
        }

        // The areas in the plan's order, as the progress has them.
        let mut areas: Vec<(Option<usize>, Area)> = Vec::new();
        for (at, entry) in entries.iter().enumerate() {
            match areas.iter_mut().find(|(area, _)| *area == entry.area) {
                Some((_, area)) => area.entries.push(at),
                None => areas.push((entry.area, Area { name: ready.area_name(entry.area, t), tone: entry.tone, figure: String::new(), entries: vec![at] })),
            }
        }
        areas.sort_by_key(|(area, _)| area.unwrap_or(usize::MAX));
        let areas: Vec<Area> = areas
            .into_iter()
            .map(|(_, mut area)| {
                let (taken, least) = area.entries.iter().filter_map(|at| entries.get(*at)).fold((0.0, 0.0), |(taken, least), entry| (taken + entry.stand.taken(), least + entry.stand.least));
                area.figure = (v.figure)(&n(taken, t), &n(least, t));
                area
            })
            .collect();

        // Where each module of the lists stands, and the entry it counts for.
        let entry_of = |choice: usize| entries.iter().position(|entry| entry.choices.contains(&choice));
        let mut counted: BTreeMap<String, usize> = BTreeMap::new();
        for (at, choice) in view.choices.iter().enumerate() {
            for taken in &choice.taken {
                if let Some(entry) = entry_of(at) {
                    counted.entry(taken.id.clone()).or_insert(entry);
                }
            }
        }
        for taken in &view.spare {
            if let Some(entry) = view.choices.iter().position(|choice| choice.list_of(&taken.id).is_some()).and_then(entry_of) {
                counted.entry(taken.id.clone()).or_insert(entry);
            }
        }
        let has = entries
            .iter()
            .flat_map(|entry| entry.tabs.iter().flat_map(|tab| tab.others.iter().chain(tab.modules.iter())))
            .filter_map(|id| {
                let (status, semester) = match doc.passed_in(id) {
                    Some(s) => (Status::Passed, s),
                    None => {
                        let s = doc.planned_in(id).into_iter().find(|s| *s >= now)?;
                        (if s == now { Status::Running } else { Status::Planned }, s)
                    }
                };
                Some((id.clone(), Has { status, semester, entry: counted.get(id).copied() }))
            })
            .collect();
        let first = entries.iter().position(|entry| entry.stand.taken() + 1e-6 < entry.stand.least).unwrap_or(0);
        let lead = if entries.is_empty() {
            v.choices_none.to_string()
        } else if view.to_choose > 0.0 {
            (v.choices_lead)(&n(view.to_choose, t))
        } else {
            v.choices_done_lead.to_string()
        };
        Pane { lead, areas, entries, has, first }
    }
}

/// The credits of a choice as the plan prints them, „10–24".
fn credits_text(choice: &Choice, t: &Texts) -> String {
    folia_plans::plan::credits_in(&choice.credits_text, t.locale)
}

/// „17 Module", „5 Listen · 59 Module".
fn count_of(lists: usize, modules: usize, t: &Texts) -> String {
    let v = &t.study_views;
    if lists > 1 {
        (v.lists_count)(lists, modules)
    } else {
        (v.modules)(modules)
    }
}

/// Its rows: their plan's caption and `ord`.
fn rows_of(view: &PlanView, choice: &Choice) -> Vec<(String, i64)> {
    choice.lines.iter().filter_map(|at| view.lines.get(*at)).map(|line| (line.caption.clone(), line.ord)).collect()
}

/// The placeholders of `rows` from `now` on that their modules do not fill yet, in order.
fn open_places(input: &Input, rows: &[(String, i64)], now: SemesterKey) -> Vec<(u32, SemesterKey)> {
    study::open_placeholders(input.doc, input.program_id, rows, &study::catalog_credits(input.rows)).into_iter().filter(|p| p.semester >= now).map(|p| (p.pid, p.semester)).collect()
}

/// Of what counts for `choice`, the modules its lists do not have: exceptions the student made.
fn others_of(taken: &[&choices::Taken], choice: &Choice) -> Vec<String> {
    taken.iter().filter(|taken| choice.list_of(&taken.id).is_none()).map(|taken| taken.id.clone()).collect()
}

/// An entry of one choice.
#[allow(clippy::too_many_arguments)]
fn choice_entry(input: &Input, ready: &Ready, ctx: StudyCtx, view: &PlanView, at: usize, choice: &Choice, now: SemesterKey, t: &'static Texts) -> Entry {
    let v = &t.study_views;
    let credits = credits_text(choice, t);
    let rows = rows_of(view, choice);
    let places = open_places(input, &rows, now);
    let name = bench::short_name(&choice.name);
    let first_row = rows.first().cloned().unwrap_or_default();
    let counts = RowRef::new(input.program_id, &first_row.0, first_row.1);
    let fill = places.first().map(|(pid, _)| *pid);
    let others = others_of(&choice.taken.iter().collect::<Vec<_>>(), choice);
    let tabs: Vec<Tab> = choice
        .lists
        .iter()
        .map(|list| Tab { name: list.name.clone(), modules: list.modules.clone(), member: None, others: others.clone(), fill, counts: counts.clone(), counts_name: name.clone() })
        .collect();
    let mut rule = if choice.fues {
        (v.fues_rule)(&credits)
    } else {
        match choice.lists.as_slice() {
            [] => (v.rule_plain)(&credits),
            [list] => (v.rule_list)(&credits, &list.name),
            lists => (v.rule_one_of)(&credits, lists.len()),
        }
    };
    if let Some((together, others)) = &choice.together {
        let names = match others.split_last() {
            Some((last, [])) => last.clone(),
            Some((last, rest)) => format!("{} {} {last}", rest.join(", "), v.and),
            None => String::new(),
        };
        if !names.is_empty() {
            rule = format!("{rule} {}", (v.together)(&names, &n(*together, t)));
        }
    }
    let catalog = if choice.fues {
        // The program's FÜS list, the BTU's where it has none.
        Some((CatalogUrl { query: variants::fues_list_for(&ready.program, t.locale).0, ..Default::default() }.path(), v.fues_catalog))
    } else if choice.lists.is_empty() {
        // What is found there fills the first open place of it from the current semester on.
        super::focus::find_href(ctx, &first_row.0, first_row.1, fill, t).map(|href| (href, v.search_catalog))
    } else {
        None
    };
    let count = if choice.fues { v.fues_count.to_string() } else { count_of(choice.lists.len(), choice.modules().len(), t) };
    Entry {
        choice: at,
        choices: vec![at],
        name,
        area: choice.area,
        tone: ready.tone(choice.area),
        rule,
        stand: view.stand_of(at),
        room: 0.0,
        count,
        tabs,
        catalog,
        no_list: choice.lists.is_empty() && !choice.fues,
        places: places.into_iter().map(|(_, s)| s).collect(),
    }
}

/// An entry of a group: its members as tabs, each with its stand and room.
fn group_entry(input: &Input, ready: &Ready, view: &PlanView, group: &choices::Group, now: SemesterKey, t: &'static Texts) -> Entry {
    let v = &t.study_views;
    let choices = &view.choices;
    let members = group.members(view);
    let (extra_least, room) = group.extra(choices);
    let extra_rows: Vec<(String, i64)> = group.extras.iter().filter_map(|at| choices.get(*at)).flat_map(|choice| rows_of(view, choice)).collect();
    let mut places: Vec<SemesterKey> = Vec::new();
    let tabs: Vec<Tab> = members
        .iter()
        .filter_map(|member| choices.get(member.choice).map(|choice| (member, choice)))
        .map(|(member, choice)| {
            let own = rows_of(view, choice);
            let rows: Vec<(String, i64)> = extra_rows.iter().cloned().chain(own.iter().cloned()).collect();
            let open = open_places(input, &rows, now);
            places.extend(open.iter().map(|(_, s)| *s));
            let first_row = own.first().cloned().unwrap_or_default();
            let mut seen = BTreeSet::new();
            let modules: Vec<String> = choice.lists.iter().flat_map(|list| list.modules.iter()).filter(|id| seen.insert(id.as_str())).cloned().collect();
            let name = bench::short_name(&choice.name);
            Tab {
                name: name.clone(),
                modules,
                member: Some((member.stand, member.room)),
                others: others_of(&member.taken.iter().collect::<Vec<_>>(), choice),
                fill: open.first().map(|(pid, _)| *pid),
                counts: RowRef::new(input.program_id, &first_row.0, first_row.1),
                counts_name: name,
            }
        })
        .collect();
    places.sort();
    places.dedup();
    let first = members.first().and_then(|member| choices.get(member.choice));
    let (least, most) = first.map_or((0.0, 0.0), |choice| (choice.least, choice.most.max(choice.least)));
    let count_word = (v.count_word)(members.len());
    let mut rule = (v.group_rule)(&n(group.target(choices), t), &count_word, &n(least, t), &n(most, t));
    if room > 1e-6 {
        let extras: Vec<&Choice> = group.extras.iter().filter_map(|at| choices.get(*at)).collect();
        let names = extras.iter().map(|choice| bench::short_name(&choice.name)).collect::<Vec<_>>().join(", ");
        let (first_fs, last_fs) = extras.iter().fold((u8::MAX, 0), |(first, last), choice| (first.min(choice.first), last.max(choice.last)));
        rule = format!("{rule} {}", (v.group_extra)(&n(most + room, t), &n(extra_least, t), &names, &(t.study.fs_span)(first_fs, last_fs), &count_word));
    }
    let modules: BTreeSet<&str> = tabs.iter().flat_map(|tab| tab.modules.iter().map(String::as_str)).collect();
    let count = count_of(tabs.len(), modules.len(), t);
    let area = first.and_then(|choice| choice.area);
    Entry {
        choice: group.choices().min().unwrap_or(0),
        choices: group.choices().collect(),
        name: group.label.clone(),
        area,
        tone: ready.tone(area),
        rule,
        stand: group.stand(view),
        room,
        count,
        tabs,
        catalog: None,
        no_list: false,
        places,
    }
}

/// A module of a list as the view draws it.
#[derive(Clone, Debug, PartialEq)]
struct Module {
    id: String,
    title: String,
    credits: Option<String>,
    phase_out: bool,
    has: Option<Has>,
    /// On the Merkliste.
    marked: bool,
    /// It counts here though no list of it has it: as the student said (`Tab::others`).
    exception: bool,
}

/// The modules of a list: what the student has first, then what is marked, then the rest by title.
/// What is not offered any more is left out, unless the student has it.
fn modules_of(ids: &[String], rows: &BTreeMap<String, CatalogRow>, has: &BTreeMap<String, Has>, marked: &BTreeSet<String>, t: &Texts) -> Vec<Module> {
    let mut out: Vec<Module> = ids
        .iter()
        .filter_map(|id| {
            let row = rows.get(id)?;
            let has = has.get(id).cloned();
            if row.offer_status.known() == Some(OfferStatus::NotOffered) && has.is_none() {
                return None;
            }
            Some(Module {
                id: id.clone(),
                title: row.title.clone(),
                credits: row.credits.map(|credits| folia_design::format::number(credits, t.locale)),
                phase_out: row.offer_status.known() == Some(OfferStatus::PhaseOut),
                has,
                marked: marked.contains(id),
                exception: false,
            })
        })
        .collect();
    out.sort_by_cached_key(|module| (module.has.is_none(), !module.marked, module.title.to_lowercase()));
    out
}

/// The modules of a tab as its list shows them: what counts for it as an exception first, marked
/// as such, then its lists' (`modules_of`).
fn tab_modules(tab: &Tab, rows: &BTreeMap<String, CatalogRow>, has: &BTreeMap<String, Has>, marked: &BTreeSet<String>, t: &Texts) -> Vec<Module> {
    let mut out: Vec<Module> = modules_of(&tab.others, rows, has, marked, t).into_iter().map(|module| Module { exception: true, ..module }).collect();
    out.extend(modules_of(&tab.modules, rows, has, marked, t));
    out
}

/// How many marked modules the lists of an entry hold.
fn marked_in(entry: &Entry, marked: &BTreeSet<String>) -> usize {
    entry.tabs.iter().flat_map(|tab| tab.modules.iter()).filter(|id| marked.contains(*id)).collect::<BTreeSet<_>>().len()
}

/// „17 Module · 1 gemerkt".
fn count_line(entry: &Entry, marked: &BTreeSet<String>, t: &'static Texts) -> String {
    match marked_in(entry, marked) {
        0 => entry.count.clone(),
        count => format!("{} · {}", entry.count, (t.study_views.marked_count)(count)),
    }
}

/// What it asks: „6", „10–24".
fn range_of(stand: &Stand, t: &Texts) -> String {
    if stand.most > stand.least + 1e-6 {
        format!("{}–{}", n(stand.least, t), n(stand.most, t))
    } else {
        n(stand.least, t)
    }
}

/// The figure as a line beside a name: „6 / 10–24 LP".
fn fig(stand: &Stand, t: &'static Texts) -> impl IntoView {
    view! { <span class="st-wfig"><b>{n(stand.taken(), t)}</b>{format!(" / {}\u{a0}{}", range_of(stand, t), t.study_views.col_credits)}</span> }
}

/// The bar of a stand (app.css `.st-wbar`): what is passed in the area's tone, what is planned
/// lighter, the modules beyond need hatched (`Stand::spare`: what the modules it counts bring past
/// its most is no more than taken, owner, 2026-11-09); the least marked where it asks a range. A
/// group's member has its room above its most (`Room`): hatched while it is open to each, part of
/// its bar where it keeps it, gone where another keeps it, yellow where it is over besides the one
/// that keeps it.
pub(super) fn bar(stand: &Stand, room: Option<(Room, f64)>, class: &'static str, t: &'static Texts) -> impl IntoView {
    let (state, extra) = room.unwrap_or((Room::None, 0.0));
    // A member that keeps the room has it in its most already.
    let cap = if state == Room::Kept { stand.most - extra } else { stand.most };
    let scale = (if state == Room::None { stand.most } else { cap + extra }).max(stand.taken()).max(stand.least).max(1e-6);
    let pct = move |x: f64| format!("{:.3}%", (x / scale * 100.0).clamp(0.0, 100.0));
    let taken = stand.taken();
    // Hatched from its most, or where what counts ends past it (8 LP where 6 were asked).
    let limit = if state == Room::Over { cap } else { stand.most.max(stand.counted()) };
    let track = if state == Room::Gone { cap.max(taken) } else { scale };
    let label = (t.study_views.figure)(&n(taken, t), &range_of(stand, t));
    // Its ends round where a part reaches them.
    let passed_end = stand.passed + 1e-6 >= scale;
    let planned_class = match (stand.passed <= 1e-6, taken + 1e-6 >= scale) {
        (true, true) => "pl first end",
        (true, false) => "pl first",
        (false, true) => "pl end",
        (false, false) => "pl",
    };
    view! {
        <span class=format!("st-wbar {class}") role="img" aria-label=label>
            <span class="tr" style=format!("width:{}", pct(track))></span>
            {matches!(state, Room::Open | Room::Over).then(|| view! { <span class=if state == Room::Open { "zn" } else { "zn over" } style=format!("left:{};width:{}", pct(cap), pct(extra))></span> })}
            <span class=if passed_end { "ps end" } else { "ps" } style=format!("width:{}", pct(stand.passed))></span>
            <span class=planned_class style=format!("left:{};width:{}", pct(stand.passed), pct(stand.planned))></span>
            {(taken > limit + 1e-6).then(|| view! { <span class=if state == Room::Over { "ov warn" } else { "ov" } style=format!("left:{};width:{}", pct(limit), pct(taken - limit))></span> })}
            {(stand.least > 0.0 && stand.least + 1e-6 < cap).then(|| view! { <span class="lt" style=format!("left:{}", pct(stand.least))></span> })}
        </span>
    }
}

/// What a group's room is for, and where two members are over their most: said, not refused.
fn room_notes(entry: &Entry, t: &'static Texts) -> Option<impl IntoView> {
    let v = &t.study_views;
    if entry.room <= 1e-6 {
        return None;
    }
    let members: Vec<(&Tab, Stand, Room)> = entry.tabs.iter().filter_map(|tab| tab.member.map(|(stand, room)| (tab, stand, room))).collect();
    let cap = members.iter().map(|(_, stand, room)| if *room == Room::Kept { stand.most - entry.room } else { stand.most }).fold(0.0, f64::max);
    let (cap, top) = (n(cap, t), n(cap + entry.room, t));
    let kept = members.iter().find(|(.., room)| *room == Room::Kept).map(|(tab, ..)| tab.name.clone());
    let over: Vec<String> = members.iter().filter(|(.., room)| *room == Room::Over).map(|(tab, ..)| tab.name.clone()).collect();
    let legend = match &kept {
        Some(name) => (v.room_kept)(name, &cap, &top),
        None => (v.room_open)(&cap, &top),
    };
    let warn = kept.filter(|_| !over.is_empty()).map(|kept| {
        let names = match over.split_last() {
            Some((last, [])) => last.clone(),
            Some((last, rest)) => format!("{} {} {last}", rest.join(", "), v.and),
            None => String::new(),
        };
        (v.room_over)(&names, over.len() > 1, &kept, &cap, &top)
    });
    Some(view! {
        <p class="st-wlegend"><span class="sw" aria-hidden="true"></span><span>{legend}</span></p>
        {warn.map(|warn| view! { <p class="st-wwarn" role="status"><Icon name="triangle-alert"/><span>{warn}</span></p> })}
    })
}

/// The view.
#[component]
pub(super) fn Choose(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    // The way back at the head of a phone's page: from a choice to the list, from the list to the
    // overview, through the history where that is the step before.
    TopBack::follow(move || {
        if !ctx.phone.get() {
            return None;
        }
        Some(match ctx.url.with(|url| url.choice) {
            Some(_) => Back { href: ctx.url.with(|url| url.with_choice(None).with_module(None, false).path()), label: v.choices, history: ctx.choice_back.get() },
            None => Back { href: StudyUrl::default().path(), label: v.to_overview, history: ctx.view_back.get() },
        })
    });
    on_cleanup(move || {
        ctx.aside_hint.try_set(None);
    });
    let pane = Memo::new(move |_| ctx.with_input(|input, ready| Pane::of(input, ready, ctx, t)));
    // What is on the Merkliste, in this browser alone.
    let bookmarks = Bookmarks::expect();
    let marked = Memo::new(move |_| bookmarks.map(|bookmarks| bookmarks.marks().into_iter().map(|mark| mark.id).collect::<BTreeSet<String>>()).unwrap_or_default());
    // The catalog's rows of every module of the lists.
    let ids = Memo::new(move |_| {
        pane.with(|pane| {
            let mut ids: Vec<String> = pane.as_ref().map(|pane| pane.entries.iter().flat_map(|entry| entry.tabs.iter().flat_map(|tab| tab.others.iter().chain(tab.modules.iter()).cloned())).collect()).unwrap_or_default();
            ids.sort();
            ids.dedup();
            ids
        })
    });
    let rows = Memo::new(move |before: Option<&BTreeMap<String, CatalogRow>>| {
        let ids = ids.get();
        if ids.is_empty() {
            return BTreeMap::new();
        }
        let Some(now) = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&StudyModulesAsk { ids }))) else { return BTreeMap::new() };
        let before = before.cloned();
        match now {
            Ok(modules) => modules.rows.into_iter().map(|row| (row.id.clone(), row)).collect(),
            Err(_) => before.unwrap_or_default(),
        }
    });
    // The entry picked: the address's (a phone's page), else the one asked for, else the first
    // with something open.
    let picked = Memo::new(move |_| {
        let asked = ctx.url.with(|url| url.choice).or_else(|| ctx.choice.get());
        pane.with(|pane| {
            let pane = pane.as_ref()?;
            asked.and_then(|at| pane.entries.iter().position(|entry| entry.choices.contains(&at))).or((!pane.entries.is_empty()).then_some(pane.first))
        })
    });
    move || {
        let phone = ctx.phone.get();
        match pane.with(|pane| pane.as_ref().map(|pane| pane.entries.is_empty())) {
            None => ().into_any(),
            Some(true) => view! {
                <section class="panel">
                    <EmptyState title=t.study_views.choices_none hint=t.study_views.choices_none_hint/>
                </section>
            }
            .into_any(),
            Some(false) if phone => view! { <ChoosePhone ctx pane rows picked marked/> }.into_any(),
            Some(false) => view! { <ChooseDesk ctx pane rows picked marked/> }.into_any(),
        }
    }
}

/// The list of an entry shown: the one the student has most of for it, else the first.
fn tab_at(entry: &Entry, has: &BTreeMap<String, Has>, at: usize) -> usize {
    entry
        .tabs
        .iter()
        .enumerate()
        .max_by_key(|(i, tab)| (tab.modules.iter().filter(|id| has.get(*id).is_some_and(|has| has.entry == Some(at))).count(), std::cmp::Reverse(*i)))
        .map_or(0, |(i, _)| i)
}

/// A group's entry (the Komplexe): its tabs are its members, which the list on the left picks
/// (owner, 2026-11-05: „Mach mal die auswahl in dem fachbereich oben weg das ist redundant").
fn is_group(entry: &Entry) -> bool {
    entry.tabs.iter().any(|tab| tab.member.is_some())
}

/// The list shown of an entry, `None` for all of a group's: what was picked of it (`picked`, the
/// entry and the list), else the one the student has most of for it, a group's all.
fn shown_of(picked: Option<(usize, Option<usize>)>, at: usize, entry: &Entry, has: &BTreeMap<String, Has>) -> Option<usize> {
    match picked {
        Some((of, shown)) if of == at => shown,
        _ if is_group(entry) => None,
        _ => Some(tab_at(entry, has, at)),
    }
}

/// The lists shown, as sections: one, or all of a group's members, each under its name and stand.
fn sections_of(entry: &Entry, shown: Option<usize>) -> Vec<Tab> {
    match shown {
        Some(at) => entry.tabs.get(at.min(entry.tabs.len().saturating_sub(1))).cloned().into_iter().collect(),
        None => entry.tabs.clone(),
    }
}

/// A group member's name and stand over its modules.
fn section_head(tab: &Tab, t: &'static Texts) -> Option<impl IntoView> {
    tab.member.map(|(stand, _)| view! { <p class="st-wsub"><span>{tab.name.clone()}</span>{fig(&stand, t)}</p> })
}

/// What „Einplanen" of a module opened from `tab` aims at: its choice's placeholder and row. None
/// for a module the student has: its sidebar says where it is.
fn hint_of(tab: &Tab, module: &Module) -> Option<(String, PlanHint)> {
    module.has.is_none().then(|| (module.id.clone(), PlanHint { semester: None, fill: tab.fill, counts: Some((tab.counts.clone(), tab.counts_name.clone())) }))
}

/// A module's mark: its status where the student has it, else nothing in its place.
fn mark(module: &Module, t: &'static Texts) -> AnyView {
    match &module.has {
        Some(has) => {
            let status = has.status;
            let words = format!("{} · {}", status.word(t), has.semester.label(t.locale));
            let title = words.clone();
            view! { <span class="st-wmk" title=title><Glyph status=status/><span class="visually-hidden">{words}</span></span> }.into_any()
        }
        None => view! { <span class="st-wmk" aria-hidden="true"></span> }.into_any(),
    }
}

/// The small words beside a module's name: marked, counting here as an exception (in `tab`),
/// phasing out, counting for another entry.
fn notes(module: &Module, pane: &Pane, at: usize, tab: &Tab, t: &'static Texts) -> impl IntoView {
    let v = &t.study_views;
    let elsewhere = module.has.as_ref().and_then(|has| has.entry).filter(|entry| *entry != at).and_then(|entry| pane.entries.get(entry)).map(|entry| (v.counts_for)(&entry.name));
    view! {
        {module.marked.then(|| view! { <span class="st-mk" title=v.marked><Icon name="bookmark"/><span class="visually-hidden">{v.marked}</span></span> })}
        {module.exception.then(|| view! { <span class="flag st-exception" title=(t.study.exception_title)(&tab.counts_name)>{t.study.exception}</span> })}
        {module.phase_out.then(|| view! { <small class="warn">{OfferStatus::PhaseOut.label(t.locale)}</small> })}
        {elsewhere.map(|note| view! { <small>{note}</small> })}
    }
}

/// The placeholders of an entry no module fills: what the student put in for it.
fn places_of(entry: &Entry, now: SemesterKey, phone: bool, t: &'static Texts) -> impl IntoView {
    let v = &t.study_views;
    entry
        .places
        .iter()
        .map(|s| {
            let status = if *s == now { Status::Running } else { Status::Planned };
            let line = (v.place_line)(&s.label(t.locale));
            view! {
                <div class=if phone { "st-wplace card" } else { "st-wplace" }>
                    <span class="st-wmk" title=status.word(t)><Glyph status=status/></span>
                    <span class="st-wrow-nm"><b>{v.place}</b><small>{line}</small></span>
                </div>
            }
        })
        .collect_view()
}

/// The head of an entry: its name and figure, its bar, what it asks, a group's room.
fn head(entry: &Entry, phone: bool, t: &'static Texts) -> impl IntoView {
    view! {
        <div class="st-wph" style=format!("--c: {}", entry.tone)>
            <div class="st-wpt">
                {if phone { view! { <h1>{entry.name.clone()}</h1> }.into_any() } else { view! { <h3>{entry.name.clone()}</h3> }.into_any() }}
                {fig(&entry.stand, t)}
            </div>
            {bar(&entry.stand, None, "big", t)}
            <p class="st-wrule">{entry.rule.clone()}</p>
            {room_notes(entry, t)}
        </div>
    }
}

/// Where an entry without a list looks: the FÜS's catalog, the catalog of its row.
fn catalog_way(entry: &Entry, t: &'static Texts) -> impl IntoView {
    let v = &t.study_views;
    entry.catalog.clone().map(|(href, label)| {
        view! {
            <div class="st-wnote">
                {entry.no_list.then(|| view! { <p class="hint">{v.no_list}</p> })}
                <a class="action" href=t.path(&href)><Icon name="search"/><span>{label}</span><Icon name="chevron-right"/></a>
            </div>
        }
    })
}

/// On a wide screen: the entries on the left, the one picked on the right.
#[component]
fn ChooseDesk(ctx: StudyCtx, pane: Memo<Option<Pane>>, rows: Memo<BTreeMap<String, CatalogRow>>, picked: Memo<Option<usize>>, marked: Memo<BTreeSet<String>>) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    let entry = Memo::new(move |_| {
        let at = picked.get()?;
        pane.with(|pane| pane.as_ref().and_then(|pane| pane.entries.get(at).cloned().map(|entry| (at, entry))))
    });
    // The list picked of an entry (a member on the left, a tab), for that entry alone.
    let list_picked = RwSignal::new(None::<(usize, Option<usize>)>);
    let left = move || {
        pane.with(|pane| {
            let pane = pane.as_ref()?;
            let picked = picked.get();
            let chosen = list_picked.get();
            Some(
                pane.areas
                    .iter()
                    .map(|area| {
                        view! {
                            <p class="st-sl-h" style=format!("--c: {}", area.tone)><span class="st-dot"></span><span>{area.name.clone()}</span><small>{area.figure.clone()}</small></p>
                            {area.entries.iter().filter_map(|at| pane.entries.get(*at).map(|entry| (*at, entry))).map(|(at, entry)| {
                                let choice = entry.choice;
                                let current = picked == Some(at);
                                let count = marked.with(|marked| count_line(entry, marked, t));
                                let room = entry.room;
                                let shown = shown_of(chosen, at, entry, &pane.has);
                                let kids = is_group(entry).then(|| view! {
                                    <div class="st-sl-kids">
                                        {entry.tabs.iter().enumerate().filter_map(|(i, tab_of)| tab_of.member.map(|member| (i, tab_of, member))).map(|(i, tab_of, (stand, state))| view! {
                                            <button class="st-sl-kid" type="button" style=format!("--c: {}", entry.tone) aria-current=(current && shown == Some(i)).then_some("true") on:click=move |_| { ctx.choice.set(Some(choice)); list_picked.set(Some((at, Some(i)))); }>
                                                <span>{tab_of.name.clone()}</span>{fig(&stand, t)}
                                                {bar(&stand, Some((state, room)), "thin", t)}
                                            </button>
                                        }).collect_view()}
                                    </div>
                                });
                                view! {
                                    <button class="st-sl" type="button" style=format!("--c: {}", entry.tone) title=entry.name.clone() aria-current=current.then_some("true") on:click=move |_| { ctx.choice.set(Some(choice)); list_picked.set(None); }>
                                        <span class="st-sl-t"><b>{entry.name.clone()}</b>{fig(&entry.stand, t)}</span>
                                        {bar(&entry.stand, None, "", t)}
                                        <small>{count}</small>
                                    </button>
                                    {kids}
                                }
                            }).collect_view()}
                        }
                    })
                    .collect_view(),
            )
        })
    };
    let right = move || {
        // The pane before the entry picked of it (R16), as `left`.
        let has = pane.with(|pane| pane.as_ref().map(|pane| pane.has.clone()));
        let open = ctx.url.with(|url| url.open.clone());
        let now = ctx.now.get();
        let (at, entry) = entry.get()?;
        let has = has?;
        let shown = shown_of(list_picked.get(), at, &entry, &has);
        // A group's members are picked on the left; other lists by their tabs.
        let tabs = (entry.tabs.len() > 1 && !is_group(&entry)).then(|| {
            view! {
                <div class="st-wtabs" role="group" aria-label=v.lists style=format!("--c: {}", entry.tone)>
                    {entry.tabs.iter().enumerate().map(|(i, tab_of)| view! {
                        <button type="button" aria-pressed=if shown == Some(i) { "true" } else { "false" } on:click=move |_| list_picked.set(Some((at, Some(i))))>
                            <span>{tab_of.name.clone()}<small>{tab_of.modules.len()}</small></span>
                        </button>
                    }).collect_view()}
                </div>
            }
        });
        let list = sections_of(&entry, shown).into_iter().map(|tab_of| {
            let modules = marked.with(|marked| rows.with(|rows| tab_modules(&tab_of, rows, &has, marked, t)));
            pane.with(|pane| {
                let pane = pane.as_ref()?;
                Some(view! {
                    {section_head(&tab_of, t)}
                    <div class="st-wrows">
                        {modules.into_iter().map(|module| {
                            let href = super::module_href(ctx, &module.id, t);
                            let hint = hint_of(&tab_of, &module);
                            let is_open = open.as_deref() == Some(module.id.as_str());
                            view! {
                                <a class="st-wrow" class:has=module.has.is_some() class:is-open=is_open href=href data-noscroll="" on:click=move |_| ctx.aside_hint.set(hint.clone())>
                                    {mark(&module, t)}
                                    <span class="st-wrow-nm"><b>{module.title.clone()}</b>{notes(&module, pane, at, &tab_of, t)}</span>
                                    <span class="st-wrow-id mono">{module.id.clone()}</span>
                                    <span class="st-wrow-lp num">{module.credits.clone().map(|credits| format!("{credits}\u{a0}{}", v.col_credits))}</span>
                                    <Icon name="chevron-right"/>
                                </a>
                            }
                        }).collect_view()}
                    </div>
                })
            })
        }).collect_view();
        Some(view! {
            {head(&entry, false, t)}
            {tabs}
            {catalog_way(&entry, t)}
            {(!entry.places.is_empty()).then(|| view! { <div class="st-wrows st-wplaces">{places_of(&entry, now, false, t)}</div> })}
            {list}
        })
    };
    view! {
        <section class="panel st-wp" aria-labelledby="st-wp-title">
            <header class="st-view-head">
                <div>
                    <h2 id="st-wp-title">{v.choices}</h2>
                    <p>{move || pane.with(|pane| pane.as_ref().map(|pane| pane.lead.clone()))}</p>
                </div>
            </header>
            <div class="st-split">
                <nav class="st-split-l" aria-label=v.areas>{left}</nav>
                <div class="st-split-r">{right}</div>
            </div>
        </section>
    }
}

/// On a phone: the entries as cards, each a page of its own (`StudyUrl::choice`).
#[component]
fn ChoosePhone(ctx: StudyCtx, pane: Memo<Option<Pane>>, rows: Memo<BTreeMap<String, CatalogRow>>, picked: Memo<Option<usize>>, marked: Memo<BTreeSet<String>>) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    let page = Memo::new(move |_| ctx.url.with(|url| url.choice).and(picked.get()));
    let list = move || {
        pane.with(|pane| {
            let pane = pane.as_ref()?;
            Some(view! {
                <header class="st-vhead">
                    <h1>{v.choices}</h1>
                    <p>{pane.lead.clone()}</p>
                </header>
                {pane.areas.iter().map(|area| view! {
                    <p class="st-wsec" style=format!("--c: {}", area.tone)><span class="st-dot"></span><span>{area.name.clone()}</span><small>{area.figure.clone()}</small></p>
                    {area.entries.iter().filter_map(|at| pane.entries.get(*at)).map(|entry| {
                        let href = ctx.url.with(|url| url.with_choice(Some(entry.choice)).with_module(None, false).path());
                        let count = marked.with(|marked| count_line(entry, marked, t));
                        let room = entry.room;
                        view! {
                            <a class="st-wcard" href=t.path(&href) style=format!("--c: {}", entry.tone) on:click=move |_| ctx.choice_back.set(true)>
                                <span class="st-wcard-t"><b>{entry.name.clone()}</b>{fig(&entry.stand, t)}<Icon name="chevron-right"/></span>
                                {bar(&entry.stand, None, "", t)}
                                <small>{count}</small>
                                {entry.tabs.iter().filter_map(|tab| tab.member.map(|member| (tab, member))).map(|(tab, (stand, state))| view! {
                                    <span class="st-wcard-kid"><span>{tab.name.clone()}</span>{fig(&stand, t)}{bar(&stand, Some((state, room)), "thin", t)}</span>
                                }).collect_view()}
                            </a>
                        }
                    }).collect_view()}
                }).collect_view()}
            })
        })
    };
    let detail = move |at: usize| {
        let (entry, has) = pane.with(|pane| pane.as_ref().and_then(|pane| pane.entries.get(at).cloned().map(|entry| (entry, pane.has.clone()))))?;
        let tab = RwSignal::new(shown_of(None, at, &entry, &has));
        let chips = (entry.tabs.len() > 1 && !is_group(&entry)).then(|| {
            view! {
                <div class="st-chiprow small" role="group" aria-label=v.lists>
                    {entry.tabs.iter().enumerate().map(|(i, tab_of)| view! {
                        <button class="st-fchip" type="button" style=format!("--c: {}", entry.tone) aria-pressed=move || if tab.get() == Some(i) { "true" } else { "false" } on:click=move |_| tab.set(Some(i))>
                            {tab_of.name.clone()}<small>{tab_of.modules.len()}</small>
                        </button>
                    }).collect_view()}
                </div>
            }
        });
        let shown_entry = entry.clone();
        let tone = entry.tone;
        let cards = move || {
            sections_of(&shown_entry, tab.get())
                .into_iter()
                .map(|tab_of| {
                    let modules = marked.with(|marked| rows.with(|rows| tab_modules(&tab_of, rows, &has, marked, t)));
                    pane.with(|pane| {
                        let pane = pane.as_ref()?;
                        Some(view! {
                            {section_head(&tab_of, t)}
                            <div class="st-wcards" style=format!("--c: {tone}")>
                                {modules.into_iter().map(|module| {
                                    let href = super::module_href(ctx, &module.id, t);
                                    let hint = hint_of(&tab_of, &module);
                                    view! {
                                        <a class="st-wcard-m" class:has=module.has.is_some() href=href data-noscroll="" on:click=move |_| ctx.aside_hint.set(hint.clone())>
                                            {mark(&module, t)}
                                            <span class="st-wrow-nm"><b>{module.title.clone()}</b>{notes(&module, pane, at, &tab_of, t)}<span class="st-wrow-id mono">{module.id.clone()}</span></span>
                                            <span class="st-lp">{module.credits.clone()}<small>{v.col_credits}</small></span>
                                            <Icon name="chevron-right"/>
                                        </a>
                                    }
                                }).collect_view()}
                            </div>
                        })
                    })
                })
                .collect_view()
        };
        let now = ctx.now.get_untracked();
        Some(view! {
            {head(&entry, true, t)}
            {chips}
            {catalog_way(&entry, t)}
            {(!entry.places.is_empty()).then(|| view! { <div class="st-wcards st-wplaces" style=format!("--c: {}", entry.tone)>{places_of(&entry, now, true, t)}</div> })}
            {cards}
        })
    };
    view! {
        <div class="st-phone-page st-wp-phone">
            {move || match page.get() {
                Some(at) => detail(at).into_any(),
                None => list().into_any(),
            }}
        </div>
    }
}
