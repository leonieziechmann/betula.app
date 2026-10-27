//! „Offen" and the choice in focus (folia/design/kalender/konzepte.html and handy.html; owner,
//! 2026-10-26: „A mit Fokus und Spur aus C", „nimm H1 mit Vergleichen"). What the plan leaves to
//! choose is a list of its own: beside the week where the page has the column for it („Offen",
//! `OpenPanel`), else a line over the week that opens into the same rows (`OpenLine`). A choice of
//! groups taken into focus (`wahl=`) stands in the week with every group outlined, the rest behind
//! it (`week::focus_slots`); a screen wider than a phone says over the week what to do
//! (`FocusBar`: „Weiter", „Vergleichen", „Fertig" with Esc), a phone lays a sheet under the week
//! with the groups as the Termin's tiles (`PickSheet`, H1 of handy.html). „Vergleichen"
//! (`vergleich=1`) lists every group with its day as a band and what it does to it: in the place
//! of „Offen" beside the week, under the bar where the page has no column beside it, on a phone
//! the sheet all up.
//!
//! A tick acts at once, as the Termin's tiles do (`PlanDoc::tick`), and the Kalender-Abo follows.
//! A choice among a module's events is listed too; its module beside the plan decides it, as
//! before. Last, where a visit meets the plan's Termine it gives way, and „Offen" says so (owner,
//! 2026-10-26: „Biologie trifft VL MIT1 · Di 13:45 · der Besuch gibt nach"), a line to its Termin. What the parts show is built as plain values first (`Line`, `Focus`), which the tests
//! read without rendering.

use std::collections::BTreeMap;

use folia_calendar::day::clock;
use folia_calendar::rowkey::RowKey;
use folia_routes::url::{PlanView, StudyplanUrl};
use folia_shell::pending::same_page;
use folia_shell::tabs::Tabs;
use folia_timetable::choice::{self, Around, Open};
use folia_timetable::clash::{self, When};
use folia_timetable::model::{Event, Timetable, UNNAMED_GROUP};
use folia_timetable::termin;
use folia_timetable::views::{kind_short, short_title};
use leptos::prelude::*;
#[cfg(feature = "csr")]
use leptos::wasm_bindgen::JsCast;

use crate::i18n::{self, studyplan_open::gap};
use folia_design::ui::{Icon, Shortcut};
use super::head::{hue, kind_word, weekday_name};
use super::PlanCtx;

// ---------- what the parts show ----------

/// A line of „Offen": the choice by its kind and module („Übung MIT1"), what to do („eine von 3
/// anhaken"), its module's tone, and where a click leads (the choice into focus, or the module
/// beside the plan).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Line {
    pub name: String,
    pub say: String,
    pub hue: &'static str,
    pub href: String,
    /// The choice in focus.
    pub current: bool,
    /// None of its options is free of the plan.
    pub warn: bool,
    /// A choice among a module's events: its module beside the plan decides it.
    pub module: bool,
    /// Where a visit gives way, not a choice: a click shows the visit's Termin.
    pub gives: bool,
}

/// „Übung MIT1": the kind as QIS names it and the module's short name, as the week names it.
fn choice_name(event: &Event, titles: &BTreeMap<String, String>, t: &i18n::Texts) -> String {
    let title = event.modules.first().and_then(|module| titles.get(module)).map_or(event.title.as_str(), String::as_str);
    format!("{} {}", kind_word(event, t.locale), short_title(title))
}

/// „Ü MIT1": a Termin of the plan in a few letters, as a slot names it; one of one's own by its name
/// („HiWi").
fn slot_name(event: &Event, titles: &BTreeMap<String, String>, t: &i18n::Texts) -> String {
    if let Some(own) = &event.own {
        return super::own::shown_name(own, titles, i18n::texts(t.locale));
    }
    let title = event.modules.first().and_then(|module| titles.get(module)).map_or(event.title.as_str(), String::as_str);
    match kind_short(event, t.locale) {
        short if short.is_empty() => short_title(title),
        short => format!("{short} {}", short_title(title)),
    }
}

/// The lines of „Offen": the open choices (`choice::open`), and the one in focus where a tick has
/// taken it out of them, in its place among the groups.
pub(super) fn lines(table: &Timetable, titles: &BTreeMap<String, String>, base: &StudyplanUrl, focus: Option<usize>, t: &i18n::Texts) -> Vec<Line> {
    let o = &t.studyplan_open;
    let mut open = choice::open(table);
    if let Some(event) = focus.filter(|event| !open.iter().any(|open| matches!(open, Open::Groups { event: e, .. } if e == event))) {
        let options = table.events.get(event).map_or(0, choice::groups);
        let at = open.iter().position(|open| matches!(open, Open::Groups { event: e, .. } if *e > event) || matches!(open, Open::Across { .. })).unwrap_or(open.len());
        open.insert(at, Open::Groups { event, options, blocked: false });
    }
    open.iter()
        .filter_map(|item| {
            let event = table.events.get(item.event())?;
            let hue = hue(event.tone);
            Some(match item {
                Open::Groups { event: index, options, blocked } => Line {
                    name: choice_name(event, titles, t),
                    say: if *blocked { o.none_free.to_string() } else { (o.tick_one_of)(*options) },
                    hue,
                    href: base.with_view(PlanView::Week).with_wahl(Some(&choice::code(event))).path(),
                    current: focus == Some(*index),
                    warn: *blocked,
                    module: false,
                    gives: false,
                },
                Open::Across { take, options, blocked, .. } => {
                    let module = event.modules.first()?;
                    let row = event.rows.iter().find(|row| row.hidden.is_none()).and_then(|row| row.key);
                    Line {
                        name: choice_name(event, titles, t),
                        say: if *blocked { o.none_free.to_string() } else { (o.take_of)(*take, *options) },
                        hue,
                        href: base.with_open(Some(module), row).path(),
                        current: false,
                        warn: *blocked,
                        module: true,
                        gives: false,
                    }
                }
            })
        })
        .chain(gives_lines(table, titles, base, t))
        .collect()
}

/// Where the visits give way (`clash::yields`), one line per visit and Termin of the plan it
/// meets, at the first time they meet: „Biologie trifft VL MIT1", „Di 13:45 · der Besuch gibt
/// nach", to the visit's Termin.
fn gives_lines(table: &Timetable, titles: &BTreeMap<String, String>, base: &StudyplanUrl, t: &i18n::Texts) -> Vec<Line> {
    let o = &t.studyplan_open;
    let mut seen: Vec<(usize, usize)> = Vec::new();
    let mut lines = Vec::new();
    for overlap in clash::yields(&table.events) {
        let pair = (overlap.a.0, overlap.b.0);
        if seen.contains(&pair) {
            continue;
        }
        seen.push(pair);
        let (Some(visit), Some(plan)) = (table.events.get(overlap.a.0), table.events.get(overlap.b.0)) else { continue };
        let Some(module) = visit.modules.first() else { continue };
        let name = titles.get(module).map_or(visit.title.as_str(), String::as_str);
        let when = match overlap.when {
            When::Weekly(_) => format!("{} {}", weekday_name(overlap.weekday, t.locale), clock(overlap.from)),
            When::Day(day) => format!("{} {} {}", weekday_name(overlap.weekday, t.locale), day.date(t.locale), clock(overlap.from)),
        };
        let href = match visit.rows.get(overlap.a.1).and_then(|row| row.key) {
            Some(key) => base.with_termin(Some((key, None))).path(),
            None => base.with_open(Some(module), None).path(),
        };
        lines.push(Line {
            name: (o.visit_meets)(&short_title(name), &slot_name(plan, titles, t)),
            say: format!("{when} · {}", o.gives_way),
            hue: hue(visit.tone),
            href,
            current: false,
            warn: false,
            module: false,
            gives: true,
        });
    }
    lines
}

/// A group of the choice in focus, as a tile and as a line of „Vergleichen".
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Group {
    /// Its place among the groups (`data-g`): a phone's sheet turns its tile into its line by it.
    pub at: usize,
    /// The keys of its Termine: one ticks it, all of them untick it.
    pub keys: Vec<RowKey>,
    /// „Mi 09:15", and „Mi 09:15–10:45".
    pub when: String,
    pub span: String,
    /// „HG/0.17", „+1" where it meets in another room besides.
    pub room: String,
    /// QIS's name of the group („Gruppe 4"), where it names one.
    pub name: Option<String>,
    /// Who leads it, as QIS names it.
    pub who: Option<String>,
    pub ticked: bool,
    /// The plan's Termin it meets („VL MIT1"); `None` where it fits. A group of a visit gives way
    /// where it meets one (`gives`): yellow, never a clash's red.
    pub meets: Option<String>,
    pub gives: bool,
    /// Its day, for „Vergleichen".
    pub day: Option<Day>,
    /// What a tooltip and a screen reader say of it.
    pub label: String,
}

/// A group's day in „Vergleichen": the plan's Termine of the weekday on a band from 7 to 19, the
/// group among them, and one line of what it does to the day.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Day {
    pub bars: Vec<Bar>,
    pub note: String,
    /// The line's sign: „map-pin" alone at the campus, „route" another campus, „triangle-alert"
    /// a Termin it meets, else „clock-3"; of a visit's group the meeting gives way (`gives`).
    pub icon: &'static str,
    pub warn: bool,
    pub gives: bool,
}

/// A Termin on a group's band, in per cent of the day from 7 to 19, in its lane.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Bar {
    pub label: String,
    pub hue: &'static str,
    pub left: f32,
    pub width: f32,
    pub lane: usize,
    pub lanes: usize,
    /// The group itself, ticked or not, and a Termin it meets.
    pub own: bool,
    pub ticked: bool,
    pub hit: bool,
}

/// The choice in focus, as the bar over the week, the sheet and „Vergleichen" show it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Focus {
    /// The event's number, which unhides it where a tick needs it.
    pub id: u32,
    /// The module's short name and tone, the kind („Übung") and the module's title.
    pub chip: String,
    pub hue: &'static str,
    pub kind: String,
    pub title: String,
    /// „Übung MIT1", for the bar.
    pub name: String,
    /// „noch keine", „2 von 3".
    pub state: String,
    /// „Hake eine oder mehrere an. Alle 3 passen."
    pub hint: String,
    /// „Bis du eine anhakst, stehen alle 3 gestrichelt im Stundenplan." while none is ticked.
    pub note: Option<String>,
    /// By weekday and time.
    pub groups: Vec<Group>,
    /// „Weiter: Praktikum PP" and where it leads: the next choice still open.
    pub next: Option<(String, String)>,
    /// Where „Fertig" leads, „Vergleichen" (on or off), and back to the tiles from it.
    pub done: String,
    pub compare: String,
    pub tiles: String,
    pub comparing: bool,
}

/// A group's dot: green where it fits, red where it meets the plan, yellow where a visit's group
/// gives way.
fn dot_of(group: &Group) -> &'static str {
    match (group.meets.is_some(), group.gives) {
        (false, _) => "st ok",
        (true, false) => "st no",
        (true, true) => "st gives",
    }
}

/// QIS's „2-Gruppe" as one reads it: „Gruppe 2"; another name as it is.
fn group_word(name: &str) -> String {
    match name.strip_suffix("-Gruppe") {
        Some(number) if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) => format!("Gruppe {number}"),
        _ => name.to_string(),
    }
}

/// The choice of the event `event` in focus, of the plan's timetable; `url` is the page's address
/// with it (`wahl=`, and `vergleich=` where it is compared).
pub(super) fn focus_of(table: &Timetable, event: usize, titles: &BTreeMap<String, String>, url: &StudyplanUrl, t: &i18n::Texts) -> Option<Focus> {
    let o = &t.studyplan_open;
    let of = table.events.get(event)?;
    let id = of.id.parse::<u32>().ok()?;
    let week = table.regular_week();
    let mut groups: Vec<(u8, u16, Group)> = termin::tiles(table, event, None)
        .into_iter()
        .filter_map(|tile| {
            let row = of.rows.get(tile.session.row)?;
            let (weekday, from, to) = (row.slot_day()?, row.from?, row.to?);
            let when = format!("{} {}", weekday_name(weekday, t.locale), clock(from));
            let span = format!("{when}–{}", clock(to));
            let room = format!("{}{}", row.date.room_shown().unwrap_or_default(), if tile.session.rooms > 0 { " +1" } else { "" });
            let name = row.date.group_name.as_deref().map(str::trim).filter(|name| !name.is_empty() && *name != UNNAMED_GROUP).map(group_word);
            let who = row.date.instructor.as_deref().map(str::trim).filter(|who| !who.is_empty()).map(str::to_string);
            let meets = tile.meets.and_then(|other| table.events.get(other)).map(|other| slot_name(other, titles, t));
            let day = choice::around(table, &week, event, tile.session.row).map(|around| day_of(table, &around, tile.ticked, titles, of, t));
            let said = match &meets {
                Some(what) if of.visit => format!("{} · {}", (o.meets)(what), o.gives_way),
                Some(what) => (o.meets)(what),
                None => o.fits.to_string(),
            };
            let gives = meets.is_some() && of.visit;
            let label = [name.clone(), Some(span.clone()), Some(room.clone()).filter(|room| !room.is_empty()), Some(said), tile.ticked.then(|| o.ticked.to_string())]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ");
            Some((weekday, from, Group { at: 0, keys: tile.keys, when, span, room, name, who, ticked: tile.ticked, meets, gives, day, label }))
        })
        .collect();
    groups.sort_by_key(|(weekday, from, _)| (*weekday, *from));
    let groups: Vec<Group> = groups.into_iter().enumerate().map(|(at, (_, _, group))| Group { at, ..group }).collect();
    let ticked = groups.iter().filter(|group| group.ticked).count();
    let fits: Vec<&Group> = groups.iter().filter(|group| group.meets.is_none()).collect();
    // Whether the groups fit, then what to do: tick one or more, or where some meet the plan, that
    // ticking goes all the same (Betula does not say no).
    let hint = match (fits.len(), groups.len()) {
        (n, all) if n == all => format!("{} {}", (o.all_fit)(all), o.tick_hint),
        (0, _) => format!("{} {}", o.none_fit, o.anyway),
        (1, _) => format!("{} {}", (o.only_fits)(&fits.first().map(|group| group.when.clone()).unwrap_or_default()), o.anyway),
        (n, all) => format!("{} {}", (o.some_fit)(n, all), o.anyway),
    };
    let title = of.modules.first().and_then(|module| titles.get(module)).map_or(of.title.as_str(), String::as_str);
    let base = url.with_wahl(None);
    let next = choice::next_open(table, event).and_then(|next| table.events.get(next)).map(|next| {
        let href = base.with_wahl(Some(&choice::code(next))).with_vergleich(url.vergleich).path();
        ((o.next)(&choice_name(next, titles, t)), href)
    });
    Some(Focus {
        id,
        chip: short_title(title),
        hue: hue(of.tone),
        kind: kind_word(of, t.locale),
        title: title.to_string(),
        name: choice_name(of, titles, t),
        state: if ticked == 0 { o.none_yet.to_string() } else { (o.ticked_of)(ticked, groups.len()) },
        hint,
        note: (ticked == 0).then(|| (o.open_note)(groups.len())),
        next,
        done: base.path(),
        compare: url.with_vergleich(!url.vergleich).path(),
        tiles: url.with_vergleich(false).path(),
        comparing: url.vergleich,
        groups,
    })
}

/// The band of a group's day and its line: alone at the campus, a Termin it meets, else the
/// time after the one before it and until the one after it, and another campus.
fn day_of(table: &Timetable, around: &Around, ticked: bool, titles: &BTreeMap<String, String>, own: &Event, t: &i18n::Texts) -> Day {
    let o = &t.studyplan_open;
    let name = |event: usize| table.events.get(event).map(|event| slot_name(event, titles, t)).unwrap_or_default();
    let (icon, warn, note) = if !around.meets.is_empty() {
        let what = around.meets.iter().map(|event| name(*event)).collect::<Vec<_>>().join(", ");
        let note = if own.visit { format!("{} · {}", (o.meets)(&what), o.gives_way) } else { (o.meets)(&what) };
        ("triangle-alert", true, note)
    } else if around.alone() {
        ("map-pin", false, (o.alone)(&weekday_long(around.day, t)))
    } else {
        let mut parts: Vec<String> = Vec::new();
        if let Some((before, minutes)) = around.before {
            parts.push((o.after)(&gap(minutes), &name(before.event)));
        }
        match around.after {
            Some((after, minutes)) => parts.push((o.until)(&gap(minutes), &name(after.event))),
            None => parts.push(o.free_after.to_string()),
        }
        if around.away {
            parts.push(o.other_campus.to_string());
        }
        (if around.away { "route" } else { "clock-3" }, false, parts.join(", "))
    };
    // The band: the day's Termine and the group, in lanes where they overlap.
    const FIRST: f32 = 7.0 * 60.0;
    const SPAN: f32 = 12.0 * 60.0;
    let mut items: Vec<(u16, u16, Bar)> = around
        .day_items
        .iter()
        .filter_map(|beside| {
            let event = table.events.get(beside.event)?;
            let label = match &event.own {
                Some(mine) => super::own::shown_name(mine, titles, i18n::texts(t.locale)),
                None => short_title(event.modules.first().and_then(|module| titles.get(module)).map_or(event.title.as_str(), String::as_str)),
            };
            Some((beside.from, beside.to, Bar { label, hue: hue(event.tone), left: 0.0, width: 0.0, lane: 0, lanes: 1, own: false, ticked: false, hit: around.meets.contains(&beside.event) }))
        })
        .collect();
    let own_title = own.modules.first().and_then(|module| titles.get(module)).map_or(own.title.as_str(), String::as_str);
    items.push((around.from, around.to, Bar { label: short_title(own_title), hue: hue(own.tone), left: 0.0, width: 0.0, lane: 0, lanes: 1, own: true, ticked, hit: false }));
    items.sort_by_key(|(from, to, bar)| (*from, *to, !bar.own));
    let mut ends: Vec<u16> = Vec::new();
    let mut lanes: Vec<usize> = Vec::new();
    for (from, to, _) in &items {
        let lane = match ends.iter().position(|end| end <= from) {
            Some(lane) => lane,
            None => {
                ends.push(0);
                ends.len() - 1
            }
        };
        if let Some(end) = ends.get_mut(lane) {
            *end = *to;
        }
        lanes.push(lane);
    }
    let count = ends.len().max(1);
    let bars = items
        .into_iter()
        .zip(lanes)
        .map(|((from, to, bar), lane)| {
            let left = ((f32::from(from) - FIRST) / SPAN * 100.0).clamp(0.0, 100.0);
            let width = ((f32::from(to) - f32::from(from)) / SPAN * 100.0).clamp(0.0, 100.0 - left);
            Bar { left, width, lane, lanes: count, ..bar }
        })
        .collect();
    Day { bars, note, icon, warn, gives: warn && own.visit }
}

/// „Mittwoch": the weekday in full, after „am".
fn weekday_long(day: u8, t: &i18n::Texts) -> String {
    t.data.weekday(i64::from(day)).unwrap_or_default().to_string()
}

// ---------- the parts ----------

/// What the parts read: the lines of „Offen" and the choice in focus, of the plan's timetable and
/// the address. `url` and `titles` are siblings of `table` (R16).
fn built(ctx: PlanCtx) -> Memo<(Vec<Line>, Option<Focus>)> {
    let t = i18n::t();
    let url = Memo::new(move |_| ctx.url.get());
    let titles = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default()));
    Memo::new(move |_| {
        let url = url.get();
        let base = url.with_open(None, None);
        titles.with(|titles| {
            ctx.table.with(|table| {
                let Some(table) = table.as_ref() else { return (Vec::new(), None) };
                let event = url.wahl.as_deref().filter(|_| url.view.is_weekly()).and_then(|code| choice::find(table, code));
                let focus = event.and_then(|event| focus_of(table, event, titles, &url, t));
                (lines(table, titles, &base, event, t), focus)
            })
        })
    })
}

/// The choice in focus out of `built`, for the parts that outlast a tick: their rows keep their
/// places, and a list its scrolling, while a box is ticked.
fn in_focus(built: Memo<(Vec<Line>, Option<Focus>)>) -> Memo<Option<Focus>> {
    Memo::new(move |_| built.with(|(_, focus)| focus.clone()))
}

/// A field of the choice in focus, nothing without one.
fn field<T: Default>(focus: Memo<Option<Focus>>, read: impl Fn(&Focus) -> T) -> T {
    focus.with(|focus| focus.as_ref().map(read).unwrap_or_default())
}

/// A tick of a group, or its tick taken away: as the Termin's tiles do it, the event and the
/// group's Termine shown again where the visitor had hidden them (`week::Act::Tick`), after the
/// next frame (R21).
fn tick(ctx: PlanCtx, id: u32, keys: Vec<RowKey>, on: bool) {
    let Some(plan) = ctx.plan else { return };
    let semester = ctx.key.get_untracked();
    plan.update_after_paint(move |doc| match (on, keys.first().copied()) {
        (true, Some(first)) => {
            doc.set_event(semester, id, false);
            keys.iter().for_each(|key| doc.set_row(semester, *key, false));
            doc.tick(semester, first, true);
        }
        (true, None) => {}
        (false, _) => keys.iter().for_each(|key| doc.tick(semester, *key, false)),
    });
}

/// The tick a click on `group` makes, of the choice in focus.
fn ticks(ctx: PlanCtx, focus: Memo<Option<Focus>>, group: &Group) -> impl Fn(leptos::ev::MouseEvent) + 'static {
    let (keys, on) = (group.keys.clone(), !group.ticked);
    move |_| {
        if let Some(id) = focus.with_untracked(|focus| focus.as_ref().map(|focus| focus.id)) {
            tick(ctx, id, keys.clone(), on);
        }
    }
}

/// Where „Fertig" goes back a step of the history: where the plan without the choice in focus is
/// the step before (as closing the module beside the plan does).
fn back_of(ctx: PlanCtx) -> Signal<Option<&'static str>> {
    let tabs = Tabs::expect();
    Signal::derive(move || {
        let page = ctx.url.with(|url| url.with_wahl(None).path());
        let now = ctx.url.with(StudyplanUrl::path);
        tabs.is_some_and(|tabs| same_page(&tabs.before(&now), &page)).then_some("history")
    })
}

/// A line of „Offen" as a link.
fn line_view(line: Line, t: &'static i18n::Texts) -> impl IntoView {
    let o = &t.studyplan_open;
    let go = match (line.current, line.module, line.gives) {
        (true, ..) => o.in_focus,
        (false, _, true) => o.look,
        (false, true, false) => o.in_module,
        (false, false, false) => o.choose,
    };
    view! {
        <a class=format!("sp-open-row {}", line.hue) class:current=line.current class:warn=line.warn class:gives=line.gives href=t.path(&line.href) data-noscroll="" aria-current=line.current.then_some("true")>
            <i class="od" aria-hidden="true"></i>
            <span class="t"><b>{line.name}</b><small>{line.say}</small></span>
            <span class="go">{go}{(!line.current).then(|| view! { <Icon name="chevron-right"/> })}</span>
        </a>
    }
}

/// „Offen" beside the week (a page wide enough for the column beside it; app.css shows it there
/// only): the open choices, the one in focus marked, and while it is compared, „Vergleichen" in
/// their place.
#[component]
pub(super) fn OpenPanel(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let o = &t.studyplan_open;
    let built = built(ctx);
    let focus = in_focus(built);
    // On a phone the sheet under the week compares the groups (`PickSheet`).
    let comparing = Memo::new(move |_| field(focus, |focus| focus.comparing));
    // The lines and how many of them are still open: the one in focus as long as none of its
    // groups is ticked.
    let lines = Memo::new(move |_| {
        built.with(|(lines, focus)| {
            let ticked = focus.as_ref().is_some_and(|focus| focus.groups.iter().any(|group| group.ticked));
            (lines.clone(), lines.iter().filter(|line| !line.gives && (!line.current || !ticked)).count())
        })
    });
    let list = move || {
        let (lines, count) = lines.get();
        (!lines.is_empty()).then(|| {
            view! {
                <section class="sp-open" aria-label=o.open>
                    <div class="sp-list-head"><h2 class="label">{o.open}</h2><span class="lp num">{count}</span></div>
                    <div class="sp-open-rows">{lines.into_iter().map(|line| line_view(line, t)).collect_view()}</div>
                </section>
            }
        })
    };
    move || match comparing.get() && !ctx.phone.get() {
        true => view! {
            <section class=move || field(focus, |focus| format!("sp-open sp-compare {}", focus.hue)) aria-label=o.compare>
                <div class="sp-list-head">
                    <h2 class="label">{o.compare}</h2>
                    <span class="nm">{move || field(focus, |focus| focus.name.clone())}</span>
                    <a class="icon-btn" href=move || field(focus, |focus| t.path(&focus.compare)) data-noscroll="" data-replace="" aria-label=o.compare_close title=o.compare_close><Icon name="x"/></a>
                </div>
                <CompareList ctx focus/>
            </section>
        }
        .into_any(),
        false => view! { {list} }.into_any(),
    }
}

/// „Offen" over the week where the page is narrow (a phone, a small window; app.css shows it there
/// only): one line, „2 Wahlen offen" and their names, that opens into the lines.
#[component]
pub(super) fn OpenLine(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let built = built(ctx);
    let open = Memo::new(move |_| built.with(|(lines, _)| lines.iter().filter(|line| !line.current).cloned().collect::<Vec<Line>>()));
    move || {
        let open = open.get();
        (!open.is_empty()).then(|| {
            // What is to choose first; where nothing is, where a visit gives way.
            let choices = open.iter().filter(|line| !line.gives).count();
            let said = match choices {
                0 => (t.studyplan_open.gives_count)(open.len()),
                n => (t.studyplan_open.open_count)(n),
            };
            let names = open.iter().filter(|line| choices == 0 || !line.gives).map(|line| line.name.clone()).collect::<Vec<_>>().join(" · ");
            view! {
                <details class="sp-open-line">
                    <summary>
                        <span class="ic" aria-hidden="true"><Icon name="list-checks"/></span>
                        <span class="t"><b>{said}</b><small>{names}</small></span>
                        <Icon name="chevron-down" class="chev"/>
                    </summary>
                    <div class="sp-open-rows">{open.into_iter().map(|line| line_view(line, t)).collect_view()}</div>
                </details>
            }
        })
    }
}

/// „Fertig", with Esc (enhance.js clicks `data-action="close-focus"`): back to the plan without
/// the choice in focus, a step back in the history where that was the step before.
fn done_link(focus: Memo<Option<Focus>>, back: Signal<Option<&'static str>>, t: &'static i18n::Texts, big: bool) -> impl IntoView {
    let class = if big { "big" } else { "btn sp-done" };
    view! {
        <a class=class href=move || field(focus, |focus| t.path(&focus.done)) data-action="close-focus" data-back=move || back.get() data-noscroll="">
            {t.studyplan_open.done}
            {(!big).then(|| view! { <Shortcut keys="Esc"/> })}
        </a>
    }
}

/// Over the week of a wide page, while a choice is in focus: the choice, how many are ticked,
/// what to do and whether the groups fit, „Weiter" to the next choice still open, „Vergleichen"
/// and „Fertig" (Esc). The ticking is the week's. Compared where the page has no column beside the
/// week, the groups' days stand under the bar (app.css shows them there only; beside the week they
/// are `OpenPanel`'s).
#[component]
pub(super) fn FocusBar(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let o = &t.studyplan_open;
    let focus = in_focus(built(ctx));
    let open = Memo::new(move |_| focus.with(Option::is_some));
    let comparing = Memo::new(move |_| field(focus, |focus| focus.comparing));
    let back = back_of(ctx);
    let compared = move || {
        comparing.get().then(|| view! { <div class=move || field(focus, |focus| format!("sp-cmp-fold {}", focus.hue)) aria-label=o.compare role="region"><CompareList ctx focus/></div> })
    };
    move || {
        open.get().then(|| {
            let next = move || {
                field(focus, |focus| focus.next.clone()).map(|(label, href)| view! { <a class="btn" href=t.path(&href) data-noscroll="">{label}</a> })
            };
            view! {
                <div class=move || field(focus, |focus| format!("sp-focusbar {}", focus.hue)) role="region" aria-label=move || field(focus, |focus| focus.name.clone())>
                    <span class="fb-text">
                        <span class="what"><i aria-hidden="true"></i><b>{move || field(focus, |focus| focus.name.clone())}</b><small>{move || field(focus, |focus| focus.state.clone())}</small></span>
                        <span class="say" title=move || field(focus, |focus| focus.hint.clone())>{move || field(focus, |focus| focus.hint.clone())}</span>
                    </span>
                    {next}
                    <a
                        class="btn"
                        class:on=move || field(focus, |focus| focus.comparing)
                        href=move || field(focus, |focus| t.path(&focus.compare))
                        data-noscroll=""
                        data-replace=""
                        aria-pressed=move || if field(focus, |focus| focus.comparing) { "true" } else { "false" }
                    >
                        {o.compare}
                    </a>
                    {done_link(focus, back, t, false)}
                </div>
                {compared}
            }
        })
    }
}

/// The groups of the choice in focus as tiles of the Termin's grid (at most three a row; from seven
/// on five, those that fit first, and „+N"), each a box to tick. `more`: all of them shown.
#[component]
fn Tiles(ctx: PlanCtx, focus: Memo<Option<Focus>>, more: RwSignal<bool>) -> impl IntoView {
    let t = i18n::t();
    let o = &t.studyplan_open;
    let all = move || field(focus, |focus| focus.groups.len());
    let shown = move || {
        let open = more.get();
        field(focus, |focus| {
            let mut groups = focus.groups.clone();
            if groups.len() > 6 && !open {
                groups.sort_by_key(|group| group.meets.is_some());
                groups.truncate(5);
            }
            groups
        })
    };
    let tile = move |group: Group| {
        let dot = dot_of(&group);
        view! {
            <button class="gs pick" class:on=group.ticked type="button" role="checkbox" aria-checked=if group.ticked { "true" } else { "false" } data-g=group.at title=group.label.clone() aria-label=group.label.clone() on:click=ticks(ctx, focus, &group)>
                <b>{group.when.clone()}<i class=dot></i></b>
                <small>{group.room.clone()}</small>
                {group.name.clone().map(|name| view! { <small class="nm">{name}</small> })}
                {if group.ticked { view! { <i class="ck"><Icon name="check"/></i> }.into_any() } else { view! { <i class="cb"></i> }.into_any() }}
            </button>
        }
    };
    let fold = move || {
        let all = all();
        (all > 6).then(|| match more.get() {
            false => view! { <button class="gs more" type="button" aria-label=(o.more)(all - 5) on:click=move |_| more.set(true)><b>{format!("+{}", all - 5)}</b><small>{o.further}</small></button> }.into_any(),
            true => view! { <button class="gs more" type="button" aria-label=o.fewer on:click=move |_| more.set(false)><b><Icon name="chevron-down" class="up"/></b><small>{o.fewer}</small></button> }.into_any(),
        })
    };
    view! {
        <div class="grp-seg" style=move || format!("--n: {}", all().clamp(1, 3))>
            <For each=shown key=|group| (group.label.clone(), group.ticked) children=tile/>
            {fold}
        </div>
    }
}

/// „Vergleichen": every group a line with its box, its time and room, who leads it, its day as a
/// band among the plan's Termine and what it does to the day.
#[component]
fn CompareList(ctx: PlanCtx, focus: Memo<Option<Focus>>) -> impl IntoView {
    let groups = move || field(focus, |focus| focus.groups.clone());
    let row = move |group: Group| {
        let sub: Vec<String> = [group.name.clone(), group.who.clone()].into_iter().flatten().collect();
        let day = group.day.clone();
        view! {
            <button class="sp-cmp-row" class:on=group.ticked type="button" role="checkbox" aria-checked=if group.ticked { "true" } else { "false" } data-g=group.at aria-label=group.label.clone() on:click=ticks(ctx, focus, &group)>
                <span class="cbx" aria-hidden="true">{group.ticked.then(|| view! { <Icon name="check"/> })}</span>
                <span class="cm">
                    <span class="ct"><b>{group.span.clone()}</b><i class=dot_of(&group)></i><span class="cr">{group.room.clone()}</span></span>
                    {(!sub.is_empty()).then(|| view! { <span class="cs">{sub.join(" · ")}</span> })}
                    {day.map(|day| view! {
                        <span class="rib" class:gives=day.gives aria-hidden="true">
                            {day.bars.into_iter().map(|bar| {
                                let class = format!("rb {}{}{}", bar.hue, if bar.own { if bar.ticked { " me" } else { " cand" } } else { "" }, if bar.hit { " hit" } else { "" });
                                let style = format!("left: {:.2}%; width: calc({:.2}% - 2px); --lane: {}; --lanes: {}", bar.left, bar.width, bar.lane, bar.lanes);
                                view! { <span class=class style=style>{bar.label}</span> }
                            }).collect_view()}
                        </span>
                        <span class="rt" aria-hidden="true">{[8u16, 10, 12, 14, 16, 18].into_iter().map(|hour| view! { <span style=format!("left: {:.2}%", (f32::from(hour) - 7.0) / 12.0 * 100.0)>{hour}</span> }).collect_view()}</span>
                        <span class="cn" class:warn=day.warn class:gives=day.gives><Icon name=day.icon/>{day.note}</span>
                    })}
                </span>
            </button>
        }
    };
    view! {
        <div class="sp-cmp">
            <For each=groups key=|group| (group.label.clone(), group.day.as_ref().map(|day| day.note.clone())) children=row/>
        </div>
    }
}

/// The page on a phone goes so far that the week stands under the bar at the top, over the sheet
/// (the stylesheet makes it as tall as that leaves, `--pick-h`).
#[cfg(feature = "csr")]
fn week_under_bar() {
    let Some(window) = leptos::web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let Some(week) = folia_design::stage::select(".sp-view") else { return };
    let bar = document.query_selector(".topbar").ok().flatten().map_or(0.0, |bar| bar.get_bounding_client_rect().bottom());
    let top = week.get_bounding_client_rect().top() + window.scroll_y().unwrap_or(0.0) - bar - 4.0;
    let still = window.match_media("(prefers-reduced-motion: reduce)").ok().flatten().is_some_and(|query| query.matches());
    let options = leptos::web_sys::ScrollToOptions::new();
    options.set_top(top.max(0.0));
    options.set_behavior(if still { leptos::web_sys::ScrollBehavior::Instant } else { leptos::web_sys::ScrollBehavior::Smooth });
    window.scroll_to_with_scroll_to_options(&options);
}

/// The height of the sheet for the stylesheet (`--pick-h` on the document), or none.
#[cfg(feature = "csr")]
fn pick_height(height: Option<f64>) {
    let Some(root) = leptos::prelude::document().document_element() else { return };
    let style = root.unchecked_ref::<leptos::web_sys::HtmlElement>().style();
    let _ = match height {
        Some(height) => style.set_property("--pick-h", &format!("{height:.0}px")),
        None => style.remove_property("--pick-h").map(|_| ()),
    };
}

/// The sheet of a phone while a choice is in focus (H1 of handy.html): under the week, the groups
/// as tiles, „Vergleichen", and one large action, „Weiter" to the next choice still open or
/// „Fertig"; closing it is its ×, and Esc. Compared, it is all up and lists the groups with their
/// days. A finger moves it as the module's sheet (enhance.js; owner, 2026-10-26: „Der Modale Dialog
/// muss auf jeden Fall mit der swipe Geste nach unten geschlossen werden können und mit der swipe
/// Geste nach oben maximiert werden können"): down from half up it goes away, up it goes all up and
/// back, each tile turning into its line of „Vergleichen" as far as the sheet has come („Wenn man
/// den Modal maximiert, sollen während dessen die Termine aus der 3 spalten Ansicht in die
/// Vergleichsliste Morphen. Passend zum Progress").
#[component]
pub(super) fn PickSheet(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let o = &t.studyplan_open;
    let focus = in_focus(built(ctx));
    let open = Memo::new(move |_| field(focus, |focus| Some(focus.id)));
    let comparing = Memo::new(move |_| field(focus, |focus| focus.comparing));
    let back = back_of(ctx);
    let sheet = NodeRef::<leptos::html::Section>::new();
    let more = RwSignal::new(false);
    // Another choice starts folded; the week above takes what the sheet of tiles leaves of the
    // screen, and comes up under the bar when the sheet opens.
    Effect::new(move |before: Option<Option<u32>>| {
        let now = open.get();
        if before.is_some_and(|before| before != now) {
            more.set(false);
        }
        #[cfg(feature = "csr")]
        if now.is_none() {
            pick_height(None);
        }
        now
    });
    Effect::new(move |opened: Option<bool>| {
        // What changes the sheet's height: the choice, its groups and hint, and the tiles all shown.
        let shown = focus.with(|focus| focus.as_ref().map(|focus| (focus.comparing, focus.groups.len(), focus.next.is_some(), focus.note.is_some())));
        let _ = more.get();
        let Some((comparing, ..)) = shown else { return false };
        #[cfg(feature = "csr")]
        {
            let first = opened != Some(true);
            request_animation_frame(move || {
                if let (Some(sheet), false) = (sheet.get_untracked(), comparing) {
                    pick_height(Some(sheet.offset_height().into()));
                }
                if first {
                    request_animation_frame(week_under_bar);
                }
            });
        }
        #[cfg(not(feature = "csr"))]
        let _ = (comparing, opened);
        true
    });
    on_cleanup(move || {
        #[cfg(feature = "csr")]
        pick_height(None);
    });
    let compare_href = move || field(focus, |focus| t.path(&focus.compare));
    // Both ways of showing the groups are there, the sheet shows one (`.full`): a finger that moves
    // the sheet between half up and all up turns each tile into its line as far as it has come
    // (enhance.js, app.css `.sp-pick.morph`), by the place of its group (`data-g`).
    let body = move || view! {
        <div class="sp-pick-body">
            <div class="grp">
                <div class="grp-h">
                    <span>{o.groups}</span>
                    <span class="r">{move || field(focus, |focus| focus.state.clone())}</span>
                    <a class="go2" href=compare_href data-noscroll="" data-replace="">{o.compare}<Icon name="chevron-right"/></a>
                </div>
                <Tiles ctx focus more/>
                <p class="grp-n">{move || field(focus, |focus| focus.note.clone().unwrap_or_else(|| focus.hint.clone()))}</p>
            </div>
            <div class="sp-pick-list"><CompareList ctx focus/></div>
        </div>
    };
    let action = move || match field(focus, |focus| focus.next.clone()) {
        Some((label, href)) => view! { <a class="big" href=t.path(&href) data-noscroll="">{label}<Icon name="chevron-right"/></a> }.into_any(),
        None => done_link(focus, back, t, true).into_any(),
    };
    // ‹ back to the tiles, shown all up (app.css): a finger moving the sheet up brings it in as far
    // as the sheet has come.
    let tiles_again = move || view! { <a class="ib to-tiles" href=move || field(focus, |focus| t.path(&focus.tiles)) data-noscroll="" data-replace="" aria-label=o.tiles title=o.tiles><Icon name="chevron-left"/></a> };
    // The tone alone sets the classes, which then hold what the script adds while a finger moves
    // the sheet (`.morph`): a tick does not set them again.
    let hue = Memo::new(move |_| field(focus, |focus| focus.hue));
    move || {
        open.get().is_some().then(|| {
            view! {
                <section
                    class=move || format!("sp-pick {}", hue.get())
                    class:full=move || comparing.get()
                    node_ref=sheet
                    aria-label=move || field(focus, |focus| focus.name.clone())
                >
                    <span class="grab" aria-hidden="true"></span>
                    <div class="sp-pick-head">
                        {tiles_again}
                        <span class="mod"><i aria-hidden="true"></i>{move || field(focus, |focus| focus.chip.clone())}</span>
                        <span class="kd">{move || field(focus, |focus| focus.kind.clone())}</span>
                        <a class="ib end" href=move || field(focus, |focus| t.path(&focus.done)) data-action="close-focus" data-back=move || back.get() data-noscroll="" aria-label=o.close title=o.close><Icon name="x"/></a>
                    </div>
                    {body()}
                    {action}
                </section>
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use folia_calendar::select::TownChoice;
    use folia_model::rows_detail::DateRow;
    use folia_plans::studyplan::PlanDoc;
    use folia_timetable::model::Input;

    use super::super::week::tests::{key, plain, row, winter};
    use super::*;

    /// Programmierpraktikum (148370) in four groups, Mo, Mi, Do 11:30 and Fr 13:45; a lecture on
    /// Monday morning (148702) and one on Thursday at 11:00 over the Thursday group, of the module
    /// 12104 („EvS").
    fn schedule() -> Vec<DateRow> {
        let group = |ord: i64, weekday: i64, time: (&str, &str), first: &str| {
            let mut row = row("148370", ord, "Praktikum", "weekly", weekday, time, (first, "2027-01-29"));
            row.date.group_name = Some(format!("{ord}-Gruppe"));
            row
        };
        vec![
            group(1, 1, ("11:30", "13:00"), "2026-10-05"),
            group(2, 3, ("11:30", "13:00"), "2026-10-07"),
            group(3, 4, ("11:30", "13:00"), "2026-10-08"),
            group(4, 5, ("13:45", "15:15"), "2026-10-09"),
            row("148702", 1, "Vorlesung", "weekly", 1, ("09:15", "10:45"), ("2026-10-05", "2027-01-25")),
            row("148702", 2, "Vorlesung", "weekly", 4, ("11:00", "12:30"), ("2026-10-08", "2027-01-28")),
        ]
    }

    fn plan_table(doc: &PlanDoc) -> Timetable {
        let (facts, schedule, modules) = (winter(), schedule(), vec!["12104".to_string()]);
        let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams: &[], sws: &[], taken: &[], choices: &[], listed: &[] };
        Timetable::build(&input, &doc.selection(key(), TownChoice::Derive))
    }

    fn short() -> BTreeMap<String, String> {
        BTreeMap::from([("12104".to_string(), "EvS".to_string())])
    }

    fn practical(table: &Timetable) -> usize {
        table.events.iter().position(|event| event.id == "148370").unwrap()
    }

    #[test]
    fn offen_lists_what_is_left_to_choose_and_the_choice_in_focus() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        let table = plan_table(&doc);
        let e = practical(&table);
        fn said(lines: &[Line]) -> Vec<(&str, &str, &str, bool)> {
            lines.iter().map(|line| (line.name.as_str(), line.say.as_str(), line.href.as_str(), line.current)).collect()
        }
        assert_eq!(said(&lines(&table, &short(), &plain(), None, &i18n::DE)), [("Praktikum EvS", "eine von 4 anhaken", "/studyplan?sem=2026W&wahl=e148370", false)]);
        assert_eq!(said(&lines(&table, &short(), &plain(), Some(e), &i18n::EN)), [("Praktikum EvS", "tick one of 4", "/studyplan?sem=2026W&wahl=e148370", true)]);
        // Ticked, it is no longer open, but stays in the list while it is in focus.
        let key_of = table.events[e].rows[0].key.unwrap();
        doc.tick(key(), key_of, true);
        let ticked = plan_table(&doc);
        assert!(lines(&ticked, &short(), &plain(), None, &i18n::DE).is_empty());
        assert_eq!(said(&lines(&ticked, &short(), &plain(), Some(e), &i18n::DE)), [("Praktikum EvS", "eine von 4 anhaken", "/studyplan?sem=2026W&wahl=e148370", true)]);
    }

    #[test]
    fn the_choice_in_focus_says_whether_its_groups_fit_and_what_they_do_to_their_day() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key(), "12104", 1, None));
        let table = plan_table(&doc);
        let e = practical(&table);
        let url = plain().with_wahl(Some("e148370"));
        let focus = focus_of(&table, e, &short(), &url, &i18n::DE).unwrap();
        assert_eq!((focus.name.as_str(), focus.chip.as_str(), focus.kind.as_str(), focus.state.as_str()), ("Praktikum EvS", "EvS", "Praktikum", "noch keine angehakt"));
        assert_eq!(focus.hint, "3 von 4 passen, die anderen überschneiden sich. Anhaken geht trotzdem.");
        assert_eq!(focus.note.as_deref(), Some("Bis du eine anhakst, stehen alle 4 gestrichelt im Stundenplan."));
        // By weekday: QIS's name of each group, what it meets, what it does to its day.
        let groups: Vec<(&str, Option<&str>, Option<&str>, &str)> = focus
            .groups
            .iter()
            .map(|group| (group.span.as_str(), group.name.as_deref(), group.meets.as_deref(), group.day.as_ref().map_or("", |day| day.note.as_str())))
            .collect();
        assert_eq!(
            groups,
            [
                ("Mo 11:30–13:00", Some("Gruppe 1"), None, "45 min nach VL EvS, danach frei"),
                ("Mi 11:30–13:00", Some("Gruppe 2"), None, "allein am Mittwoch"),
                ("Do 11:30–13:00", Some("Gruppe 3"), Some("VL EvS"), "trifft VL EvS"),
                ("Fr 13:45–15:15", Some("Gruppe 4"), None, "allein am Freitag"),
            ]
        );
        // Monday's band: the lecture, then the group, one lane each in their own time.
        let monday = focus.groups[0].day.clone().unwrap();
        assert_eq!(monday.bars.iter().map(|bar| (bar.label.as_str(), bar.own, bar.lanes)).collect::<Vec<_>>(), [("EvS", false, 1), ("EvS", true, 1)]);
        assert_eq!((monday.icon, monday.warn), ("clock-3", false));
        assert_eq!(focus.groups[2].day.as_ref().map(|day| (day.icon, day.warn, day.bars.iter().filter(|bar| bar.hit).count())), Some(("triangle-alert", true, 1)));
        // The only choice: no „Weiter"; „Fertig" leaves the focus, „Vergleichen" compares, ‹ goes
        // back to the tiles; each group by its place, which its tile and its line carry.
        assert_eq!((focus.next.clone(), focus.done.as_str(), focus.compare.as_str(), focus.tiles.as_str()), (None, "/studyplan?sem=2026W", "/studyplan?sem=2026W&wahl=e148370&vergleich=1", "/studyplan?sem=2026W&wahl=e148370"));
        assert_eq!(focus.groups.iter().map(|group| group.at).collect::<Vec<_>>(), [0, 1, 2, 3]);
        // Ticked: how many, without the note of an untouched choice.
        let first = table.events[e].rows[0].key.unwrap();
        doc.tick(key(), first, true);
        let ticked = plan_table(&doc);
        let focus = focus_of(&ticked, e, &short(), &url.with_vergleich(true), &i18n::DE).unwrap();
        assert_eq!((focus.state.as_str(), focus.note.clone(), focus.comparing, focus.compare.as_str(), focus.tiles.as_str()), ("1 von 4 angehakt", None, true, "/studyplan?sem=2026W&wahl=e148370", "/studyplan?sem=2026W&wahl=e148370"));
        assert!(focus.groups[0].ticked && focus.groups[0].day.as_ref().is_some_and(|day| day.bars.iter().any(|bar| bar.own && bar.ticked)));
        assert_eq!(focus.groups.iter().filter(|group| group.ticked).count(), 1);
    }

    #[test]
    fn a_group_name_reads_as_one_says_it() {
        assert_eq!(group_word("2-Gruppe"), "Gruppe 2");
        assert_eq!(group_word("Gruppe A"), "Gruppe A");
        assert_eq!(gap(45), "45 min");
        assert_eq!(gap(75), "1 h 15");
        assert_eq!(gap(180), "3 h");
    }
}
