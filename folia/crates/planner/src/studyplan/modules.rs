//! The modules of the Stundenplan beside the week: the index of what the week shows (owner,
//! 2026-11-17: „Im Kalender geht es dann nur noch darum auszuwählen, was jetzt genau gezeigt werden
//! soll. Oder halt termine selbst zu verwalten."). „Mein Studiengang" is the plan's part; each
//! calendar of one's own has a part of its own after it (`calendars.rs`).
//!
//! A module is one line: its key in its tone as the week names it („EvS"), its title, „Besuch" after
//! a visit's. The line opens the module beside the plan. Under it a row per event that has a Termin
//! — exactly the segments of the overview („bei den Modulen die Kategorien vl, Übung, prak … und man
//! da die Übersicht auswählen kann, wo man alles durchklicken kann", the same day), from the one
//! function both read (`folia_timetable::categories`): the event's word, what the plan holds of it
//! („Di 11:30 · Fr 09:15", „4 Gruppen · offen", „Mo 15:30 · 1 von 4", „ausgeblendet"), the whole row a
//! link to the overview at that event (`termin=<key>&full=1`; an event without any date has none,
//! its row leads to the module beside the plan), and at its end the eye (owner, the same day: „Ich
//! will übrigens dass wir vom x weg kommen und eher ein augen symbol verwneden"). The eye is a switch
//! of the whole event while the plan is edited, `eye` while the week shows it and `eye-off` while it
//! does not; while the plan is looked at its place holds only the crossed eye of a row hidden in
//! part or whole, a quiet way into the mode, so that no text of a row moves when the mode comes. Then the module's Termine of one's own („von dir", each opening its card), and while
//! editing „+ Termin"; a module QIS has no Termin for says so with „Termin eintragen" in both states.
//! While editing, and only while something is hidden, the foot „2 ausgeblendet · Alle einblenden".
//! The last line leads to „Mein Studium".
//!
//! The list manages none of the study's modules (owner, 2026-11-17: „man kann gerade noch module im
//! Kalender verwalten, das ist aber keine gute idee, weil man das ja in mein Studium macht"): no
//! „+ Modul", no ×, no credits, no placeholders, which it had until then.
//!
//! The rows are built as plain values first (`ModuleItem`, `Said`, `RowShown`), which the tests read
//! without rendering, and keyed by them (R5): a row reads its values from a memo of its own, so that
//! the eye clicked stays the same element and keeps the focus, and entering the mode builds no row
//! anew. An eye writes the plan at once: only the selection and the timetable follow (R21).

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::kind::EventKind;
use folia_calendar::rowkey::RowKey;
use folia_calendar::semester::SemesterKey;
use folia_pages::StudyplanData;
use folia_plans::studyplan::{PlanDoc, PlanUndo};
use folia_routes::url::{self, PlanAddress, StudyUrl, StudyplanUrl};
use folia_timetable::categories::{self, EventRow, Out, Summary};
use folia_timetable::clash::Weeks;
use folia_timetable::model::{Event, Timetable};
use folia_timetable::termin::Occasion;
use folia_timetable::views::hidden_count;
use leptos::prelude::*;

use crate::i18n;
use folia_shell::pending::{Change, Pending};
use folia_design::haptic::TapTick;
use folia_design::ui::Icon;
use super::head::{hue, is_past, tone_at};
use super::own::{own_rows, OwnRow};
use super::PlanCtx;

/// A module of the list: its one line.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ModuleItem {
    id: String,
    /// The abbreviation the week names the module by („EvS"); none where the week names it by its
    /// short name, which the title begins with.
    key: Option<String>,
    /// The catalog's title; „Modul 12345" for a module the catalog does not know.
    title: String,
    hue: &'static str,
    /// Only visited (`Selection::visits`).
    visit: bool,
    /// The semester has dates and the module none in QIS („keine Termine in QIS"): it offers
    /// „Termin eintragen", a Termin of one's own of it (folia/design/kalender/termine.html).
    undated: bool,
}

/// The modules of a semester, in plan order, each in the tone the week gives it; those of `visits`
/// only visited. A module without a dated Termin says so where the semester's dates are there to be
/// had (neither past, whose dates are gone, nor unpublished, which the head says for all of them).
fn module_items(data: &StudyplanData, visits: &BTreeSet<String>, t: &'static i18n::Texts) -> Vec<ModuleItem> {
    let dated = !data.counts.is_empty() && !is_past(data);
    let names = data.slot_names();
    data.ids
        .iter()
        .enumerate()
        .map(|(position, id)| {
            let row = data.modules.iter().find(|row| row.id == *id);
            let undated = dated && row.is_some() && !data.schedule.iter().any(|date| date.module_id == *id && date.ord.is_some());
            ModuleItem {
                id: id.clone(),
                key: data.abbrevs.get(id).filter(|abbrev| names.get(id) == Some(*abbrev)).cloned(),
                title: row.map_or_else(|| (t.studyplan_modules.module_numbered)(id), |row| row.title.clone()),
                hue: hue(tone_at(position)),
                visit: visits.contains(id),
                undated,
            }
        })
        .collect()
}

/// What a click on a row's eye writes: the event hidden, shown again, a kind of before switched on
/// again (a `k` line of before 2026-11-04 hides the event: its eye says that reach), or the Termine
/// the student hid one by one shown again where every Termin of the event is hidden so.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Flip {
    Hide(u32),
    Show(u32),
    ShowKind(EventKind),
    ShowTermine(Vec<RowKey>),
}

impl Flip {
    fn apply(&self, doc: &mut PlanDoc, key: SemesterKey) {
        match self {
            Flip::Hide(id) => doc.set_event(key, *id, true),
            Flip::Show(id) => doc.set_event(key, *id, false),
            Flip::ShowKind(kind) => doc.set_kind(key, *kind, false),
            Flip::ShowTermine(rows) => rows.iter().for_each(|row| doc.set_row(key, *row, false)),
        }
    }
}

/// What a row's summary says: its text, then „ · offen" in the tone of what is to decide (`open`),
/// then „ · freiwillig" (`extra`).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Said {
    text: String,
    open: bool,
    extra: bool,
}

impl Said {
    /// The whole of it in one line: „4 Gruppen · offen".
    fn plain(&self, t: &i18n::Texts) -> String {
        let m = &t.studyplan_modules;
        let mut parts = vec![self.text.clone()];
        if self.open {
            parts.push(m.open.to_string());
        }
        if self.extra {
            parts.push(m.optional.to_string());
        }
        parts.join(" · ")
    }
}

/// „Di 11:30", „Di 07:30 A": a weekly Termin by its weekday and start, the week's letter after it.
pub(super) fn weekly_text((day, from, weeks): (u8, u16, Weeks), t: &i18n::Texts) -> String {
    let day = t.data.weekday_short(i64::from(day)).unwrap_or_default();
    let letter = match weeks {
        Weeks::A => " A",
        Weeks::B => " B",
        Weeks::All => "",
    };
    format!("{day} {}{letter}", folia_calendar::day::clock(from))
}

/// What a row says of its event: one or two Termine a week by weekday and start („Di
/// 11:30 · Fr 09:15", „Di 07:30 A"), three or four by their weekdays („Mo · Mi · Fr"), more or single
/// dates alone „5 Termine"; groups none of which is ticked „4 Gruppen · offen", ticked their times and
/// „1 von 4"; an option of an open choice its times and „offen"; nothing with a time „ohne feste
/// Zeit"; an extra ends „· freiwillig"; hidden whole „ausgeblendet", an option another was taken
/// over „nicht gewählt".
fn summary_text(summary: &Summary, t: &i18n::Texts) -> Said {
    let m = &t.studyplan_modules;
    match summary.out {
        Some(Out::Hidden | Out::Kind | Out::Termine) => return Said { text: t.studyplan_week.hidden.to_string(), open: false, extra: false },
        Some(Out::Unchosen) => return Said { text: m.not_chosen.to_string(), open: false, extra: false },
        None => {}
    }
    let mut parts: Vec<String> = Vec::new();
    match summary.weekly.len() {
        // Single dates alone; beside groups they are no Termine of the week to name.
        0 if summary.dates > 0 && summary.groups.is_none() => parts.push((t.studyplan_week.sessions)(summary.dates)),
        0 => {}
        1 | 2 => parts.extend(summary.weekly.iter().map(|weekly| weekly_text(*weekly, t))),
        3 | 4 => {
            let mut days: Vec<u8> = summary.weekly.iter().map(|(day, ..)| *day).collect();
            days.dedup();
            parts.push(days.iter().map(|day| t.data.weekday_short(i64::from(*day)).unwrap_or_default()).collect::<Vec<_>>().join(" · "));
        }
        n => parts.push((t.studyplan_week.sessions)(n + summary.dates)),
    }
    let open = match summary.groups {
        Some((0, of)) => {
            parts.push((m.groups)(of));
            true
        }
        Some((ticked, of)) => {
            parts.push((m.of)(ticked, of));
            false
        }
        None => summary.option_open,
    };
    if parts.is_empty() {
        parts.push(m.no_time.to_string());
    }
    Said { text: parts.join(" · "), open, extra: summary.extra }
}

/// What a row says is hidden of an event that shows, each in its word: „1 Gruppe ausgeblendet",
/// „1 Termin ausgeblendet", „am Fr 20.11. ausgeblendet", „1 Termin · an 2 Tagen ausgeblendet".
/// `None` where nothing of it is hidden, or all of it (the summary says „ausgeblendet").
fn hidden_text(summary: &Summary, t: &i18n::Texts) -> Option<String> {
    let m = &t.studyplan_modules;
    if summary.out.is_some() || summary.hidden == 0 {
        return None;
    }
    let days = summary.hidden_days.len();
    let termine = summary.hidden.saturating_sub(days + summary.hidden_groups);
    let mut parts = Vec::new();
    if summary.hidden_groups > 0 {
        parts.push((m.groups)(summary.hidden_groups));
    }
    if termine > 0 {
        parts.push((t.studyplan_week.sessions)(termine));
    }
    match summary.hidden_days.as_slice() {
        [] => {}
        [day] => parts.push((m.hidden_on)(&format!("{} {}", t.data.weekday_short(i64::from(day.weekday())).unwrap_or_default(), day.day_month(t.locale)))),
        _ => parts.push((m.hidden_days)(days)),
    }
    Some((m.hidden_end)(&parts.join(" · ")))
}

/// A row of an event as it shows now. `eye`: whether the plan shows the event, the eye's name, and
/// what a click writes; none where the row cannot be shown or hidden here. `sign`: what the crossed
/// eye says while looking, of a row hidden in part or whole. `hid`: the same of a row hidden in
/// part, said after its summary while editing (the eye then says that the event shows). `beside`:
/// the row leads to its module beside the plan, not to a page of its own (an event without any
/// date has no overview, `Occasion::find`).
#[derive(Clone, Debug, PartialEq, Eq)]
struct RowShown {
    label: String,
    said: Said,
    href: String,
    beside: bool,
    aria: String,
    title: String,
    pale: bool,
    hid: Option<String>,
    sign: Option<String>,
    eye: Option<(bool, String, Flip)>,
}

/// The row of `event` (`row` of `categories::module_events`) of the module `module`, which the week
/// calls `name`: what it says, where it leads (from the address `base`: the overview at it,
/// `termin=<key>&full=1`, where the Termin has a date — `dated` —, else the module beside the plan,
/// pointing at the Termin, as the week's „ohne feste Zeit" does), and its eye.
fn row_shown(row: &EventRow, event: &Event, module: &str, name: &str, dated: bool, base: &StudyplanUrl, t: &i18n::Texts) -> RowShown {
    let m = &t.studyplan_modules;
    let summary = categories::summary(event);
    let said = summary_text(&summary, t);
    let hid = hidden_text(&summary, t);
    let href = match dated {
        true => PlanAddress { url: base.with_termin(Some((row.key, None))), full: true }.path(),
        false => base.with_open(Some(module), Some(row.key)).path(),
    };
    let whole = match &hid {
        Some(hid) => format!("{} · {hid}", said.plain(t)),
        None => said.plain(t),
    };
    let aria = match dated {
        true => (m.row_label)(&row.label, name, &whole),
        false => (m.row_label_module)(&row.label, name, &whole),
    };
    let sign = match summary.out {
        Some(Out::Hidden | Out::Kind | Out::Termine) => Some(format!("{} {}", row.label, t.studyplan_week.hidden)),
        _ => hid.clone(),
    };
    // The eye's name says the kind in the page's language: in English Betula's label, lower-cased
    // inside the sentence, and what tells two of a kind apart after it ("Show exercise · Mon EvS").
    let kinds = categories::labels(event, t.locale);
    let apart = row.label.strip_prefix(categories::word(event, t.locale).as_str()).unwrap_or_default();
    let named = (m.show_event)(&row.label, &kinds, apart, name);
    let eye = match (summary.out, event.id.parse::<u32>()) {
        (Some(Out::Unchosen), _) | (_, Err(_)) => None,
        (None, Ok(id)) => Some((true, named, Flip::Hide(id))),
        (Some(Out::Hidden), Ok(id)) => Some((false, named, Flip::Show(id))),
        (Some(Out::Termine), Ok(_)) => Some((false, named, Flip::ShowTermine(summary.rows.clone()))),
        (Some(Out::Kind), Ok(_)) => event.kinds.iter().next().map(|kind| (false, (t.studyplan_week.show_kind)(kind.label(t.locale)), Flip::ShowKind(kind))),
    };
    RowShown { label: row.label.clone(), title: format!("{} · {whole}", row.label), said, href, beside: !dated, aria, pale: summary.out.is_some(), hid, sign, eye }
}

/// The rows of every module of `table`, by module and then by event id, in the overview's order.
/// `names`: what the week calls the modules.
fn rows_of(table: &Timetable, names: &BTreeMap<String, String>, base: &StudyplanUrl, t: &i18n::Texts) -> BTreeMap<String, Vec<(String, RowShown)>> {
    table
        .modules
        .iter()
        .map(|module| {
            let name = names.get(module).map_or(module.as_str(), String::as_str);
            let rows = categories::module_events(table, module, None, t.locale)
                .iter()
                .filter_map(|row| {
                    let event = table.events.get(row.event)?;
                    let dated = Occasion::find(table, row.key, None, None).is_some();
                    Some((event.id.clone(), row_shown(row, event, module, name, dated, base, t)))
                })
                .collect();
            (module.clone(), rows)
        })
        .collect()
}

/// The module beside the plan, and while one is on its way there (`pending`), that one: its line is
/// marked in the next frame (R21).
fn open_shown(ctx: PlanCtx) -> Memo<Option<String>> {
    let going = Pending::expect();
    Memo::new(move |_| {
        let target = going.filter(|going| going.change() == Some(Change::Aside)).and_then(|going| going.search_on(url::STUDYPLAN));
        match target {
            Some(search) => StudyplanUrl::parse(&search).open,
            None => ctx.url.with(|url| url.open.clone()),
        }
    })
}

/// The id of „Alle einblenden", to which the bar's figure „2 ausgeblendet" brings the keyboard.
pub(super) const SHOW_ALL_ID: &str = "sp-show-all";

/// „Mein Studiengang": a line per module and under it its rows, its Termine of one's own, „+ Termin"
/// while editing; those of no module of the week; the foot „2 ausgeblendet · Alle einblenden" while
/// editing; the way to „Mein Studium".
#[component]
pub(super) fn ModuleList(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let m = &t.studyplan_modules;
    // The modules only visited: of the selection, a sibling of the data (R16).
    let visits = Memo::new(move |_| ctx.selection.with(|(_, selection)| selection.visits.clone()));
    let items = Memo::new(move |_| visits.with(|visits| ctx.data.with(|data| data.as_ref().map(|data| module_items(data, visits, t)).unwrap_or_default())));
    let names = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default()));
    let base = Memo::new(move |_| ctx.url.get());
    // The rows of the events: of the timetable, the slots' names and the address, siblings (R16).
    let rows = Memo::new(move |_| base.with(|base| names.with(|names| ctx.table.with(|table| table.as_ref().map(|table| rows_of(table, names, base, t)).unwrap_or_default()))));
    // The Termine of one's own of the plan: the selection's, named as the week's slots name them,
    // in the tone of their module (the data, a sibling).
    let owns = Memo::new(move |_| {
        let held = ctx.selection.with(|(_, selection)| selection.own.clone());
        ctx.data.with(|data| data.as_ref().map(|data| own_rows(&held, &data.slot_names(), &data.ids, t)).unwrap_or_default())
    });
    let hidden = Memo::new(move |_| ctx.table.with(|table| table.as_ref().map_or(0, |table| hidden_count(table).total())));
    let open = open_shown(ctx);
    let semester = Memo::new(move |_| ctx.key.get().label(t.locale));
    let study = t.path(&StudyUrl { plan: true, ..Default::default() }.path());

    // A row of an event, its values read from a memo of its own (R5).
    let category = move |module: String, event: String| {
        let shown = {
            let (module, event) = (module.clone(), event.clone());
            Memo::new(move |_| rows.with(|rows| rows.get(&module).and_then(|rows| rows.iter().find(|(id, _)| *id == event)).map(|(_, row)| row.clone())))
        };
        let label = Memo::new(move |_| shown.with(|row| row.as_ref().map(|row| row.label.clone()).unwrap_or_default()));
        let said = Memo::new(move |_| shown.with(|row| row.as_ref().map(|row| row.said.clone())));
        let href = Memo::new(move |_| shown.with(|row| row.as_ref().map(|row| t.path(&row.href)).unwrap_or_default()));
        let beside = Memo::new(move |_| shown.with(|row| row.as_ref().is_some_and(|row| row.beside)));
        let aria = Memo::new(move |_| shown.with(|row| row.as_ref().map(|row| row.aria.clone()).unwrap_or_default()));
        let title = Memo::new(move |_| shown.with(|row| row.as_ref().map(|row| row.title.clone()).unwrap_or_default()));
        let pale = Memo::new(move |_| shown.with(|row| row.as_ref().is_some_and(|row| row.pale)));
        let hid = Memo::new(move |_| shown.with(|row| row.as_ref().and_then(|row| row.hid.clone())));
        let sign = Memo::new(move |_| shown.with(|row| row.as_ref().and_then(|row| row.sign.clone())));
        let has_eye = Memo::new(move |_| shown.with(|row| row.as_ref().is_some_and(|row| row.eye.is_some())));
        let on = Memo::new(move |_| shown.with(|row| row.as_ref().and_then(|row| row.eye.as_ref()).is_some_and(|(on, ..)| *on)));
        let named = Memo::new(move |_| shown.with(|row| row.as_ref().and_then(|row| row.eye.as_ref()).map(|(_, name, _)| name.clone()).unwrap_or_default()));
        let flip = move |_| {
            let Some(plan) = ctx.plan else { return };
            let Some(flip) = shown.with_untracked(|row| row.as_ref().and_then(|row| row.eye.as_ref()).map(|(.., flip)| flip.clone())) else { return };
            let key = ctx.key.get_untracked();
            plan.update(|doc| flip.apply(doc, key));
        };
        // The summary in its parts, each kept whole: where the row is too narrow for it, it wraps
        // after a „·" and the row grows, in both states alike — never a word cut in its middle.
        let summary = move || {
            said.get().map(|said| {
                // A part with a figure („Mo 15:30", „4 Gruppen") is one; one of words alone („ohne
                // feste Zeit") may wrap between them where even it does not fit.
                let part_of = |part: &str| match part.bytes().any(|byte| byte.is_ascii_digit()) {
                    true => view! { <span class="sp-cat-p">{part.to_string()}</span> }.into_any(),
                    false => view! { <span class="sp-cat-p words">{part.to_string()}</span> }.into_any(),
                };
                let mut parts: Vec<AnyView> = said.text.split(" · ").map(part_of).collect();
                if said.open {
                    parts.push(view! { <em class="open sp-cat-p words">{m.open}</em> }.into_any());
                }
                if said.extra {
                    parts.push(view! { <span class="sp-cat-p words">{m.optional}</span> }.into_any());
                }
                let last = parts.len().saturating_sub(1);
                parts.into_iter().enumerate().map(|(at, part)| view! { {part}{(at < last).then_some("\u{a0}· ")} }).collect_view()
            })
        };
        // A row hidden in part says so after its summary while editing (its eye then says that the
        // event shows). Its place is kept while looking, unseen, so that the summary wraps alike in
        // both states.
        let part = move || hid.get().map(|hid| view! { <i class="sp-cat-hid" role="img" title=hid.clone() aria-label=hid><Icon name="eye-off"/></i> });
        // The eye at the row's end: a switch while editing; while looking its place, empty, or the
        // crossed eye of a row hidden in part or whole, a quiet way into the mode, where the eye
        // brings it back (owner, 2026-11-17: „im edit modus auch Termine leicht anzeigen kann, die
        // man ausgeblendet hat").
        let eye = move || match ctx.edit.get() && has_eye.get() {
            true => view! {
                <button
                    class="eye sp-cat-eye tap-host"
                    type="button"
                    role="switch"
                    data-action="show-event"
                    aria-checked=move || if on.get() { "true" } else { "false" }
                    aria-label=move || named.get()
                    title=move || named.get()
                    on:click=flip
                >
                    {move || match on.get() {
                        true => view! { <Icon name="eye"/> }.into_any(),
                        false => view! { <Icon name="eye-off"/> }.into_any(),
                    }}
                    <TapTick/>
                </button>
            }
            .into_any(),
            false => match (ctx.edit.get(), sign.get()) {
                (false, Some(sign)) => {
                    let name = (t.studyplan_edit.open_way)(&sign);
                    let into = move || ctx.url.with(|url| t.path(&url.with_edit(true).path()));
                    view! { <a class="eye sp-cat-way" href=into data-noscroll="" aria-label=name.clone() title=name><Icon name="eye-off"/></a> }.into_any()
                }
                _ => view! { <i class="eye" aria-hidden="true"></i> }.into_any(),
            },
        };
        view! {
            <div class="sp-cat" class:off=move || pale.get() data-event=event>
                // A row leads to a page of its own (the overview), which opens at its top; one
                // without a date to its module beside the plan, which keeps the page where it is.
                <a class="sp-cat-row" href=move || href.get() aria-label=move || aria.get() title=move || title.get() data-noscroll=move || beside.get().then_some("")>
                    <b>{move || label.get()}</b>
                    <span class="sp-cat-sum">{summary}{part}</span>
                    <Icon name="chevron-right"/>
                </a>
                {eye}
            </div>
        }
    };
    // A Termin of one's own of the module: its card, in both states (it is deleted there and by the
    // bin on its slot, not hidden: no eye; owner, 2026-11-17: „da wo es entfernt eine Mülltonne").
    let own_row = move |row: OwnRow| {
        let href = move || ctx.url.with(|url| t.path(&url.with_termin(Some((row.key, None))).path()));
        let current = Memo::new(move |_| ctx.url.with(|url| url.termin_on().map(|(key, _)| key) == Some(row.key)));
        // Its time alone, as the rows of QIS's Termine say theirs; its place is its card's.
        view! {
            <div class="sp-cat own">
                <a class="sp-cat-row" href=href aria-current=move || current.get().then_some("true") data-noscroll="" title=row.name.clone()>
                    <b>{row.label.clone()}</b>
                    <span class="sp-cat-sum"><span class="sp-key own" role="img" title=t.studyplan_own.mark aria-label=t.studyplan_own.mark><Icon name="user-round"/></span><span class="sp-cat-p">{row.when.clone()}</span></span>
                    <Icon name="chevron-right"/>
                </a>
                <i class="eye" aria-hidden="true"></i>
            </div>
        }
    };
    let module = move |item: ModuleItem| {
        let id = item.id.clone();
        let current = {
            let id = id.clone();
            Memo::new(move |_| open.with(|open| open.as_deref() == Some(id.as_str())))
        };
        let beside = {
            let id = id.clone();
            move || ctx.url.with(|url| t.path(&url.with_open(Some(&id), None).path()))
        };
        let events = {
            let id = id.clone();
            Memo::new(move |_| rows.with(|rows| rows.get(&id).map(|rows| rows.iter().map(|(event, _)| event.clone()).collect::<Vec<_>>()).unwrap_or_default()))
        };
        let mine = {
            let id = id.clone();
            Memo::new(move |_| owns.with(|owns| owns.iter().filter(|row| row.module.as_deref() == Some(id.as_str())).cloned().collect::<Vec<_>>()))
        };
        let key = match item.key.clone() {
            Some(key) => view! { <span class="sp-key">{key}</span> }.into_any(),
            None => view! { <i class="sp-key"></i> }.into_any(),
        };
        let visit = item.visit.then(|| view! { <span class="sp-visit">{m.visit}</span> });
        // What „+ Termin" and „Termin eintragen" name the module by: as the week does.
        let called = names.with_untracked(|names| names.get(&id).cloned()).unwrap_or_else(|| item.title.clone());
        let enter_title = (t.studyplan_own.enter_title)(&called);
        // „keine Termine in QIS · Termin eintragen ›" in both states (it fills a gap, as the empty
        // week's „+ Termin" does); the way
        // goes once the module has a Termin of one's own.
        let none = item.undated.then(|| {
            let (module, title) = (id.clone(), enter_title.clone());
            view! {
                <p class="sp-cat-none">
                    <span>{m.no_dates}</span>
                    {move || mine.with(Vec::is_empty).then(|| {
                        let module = module.clone();
                        view! {
                            " · "
                            <button class="sp-enter" type="button" title=title.clone() on:click=move |_| super::own::ask_today(ctx, 0, Some(module.clone()))>
                                {t.studyplan_own.enter}<Icon name="chevron-right"/>
                            </button>
                        }
                    })}
                </p>
            }
        });
        // Named for its module („Termin für EvS eintragen"): the list holds one under each.
        let add_name = (t.studyplan_own.add_for)(&called);
        let add = {
            let (module, title) = (id.clone(), enter_title);
            move || {
                let module = module.clone();
                ctx.edit.get().then(|| view! {
                    <button class="sp-own-add" type="button" data-action="own-add" title=title.clone() aria-label=add_name.clone() on:click=move |_| super::own::ask_today(ctx, 0, Some(module.clone()))>
                        <Icon name="plus"/>{t.studyplan_own.add}
                    </button>
                })
            }
        };
        let row_of = {
            let id = id.clone();
            move |event: String| category(id.clone(), event)
        };
        // The column of the rows' words is as wide as the longest of the module's (in letters, a
        // custom property the stylesheet turns into a width within a share of the row): what tells
        // two of a kind apart stands at a word's end („Vorlesung · Mi 07:30").
        let word = {
            let id = id.clone();
            Memo::new(move |_| {
                let rows = rows.with(|rows| rows.get(&id).map(|rows| rows.iter().map(|(_, row)| row.label.chars().count()).max().unwrap_or(0)).unwrap_or(0));
                mine.with(|mine| mine.iter().map(|row| row.label.chars().count()).max().unwrap_or(0)).max(rows)
            })
        };
        view! {
            <div class=format!("sp-mod {}", item.hue) data-id=id.clone()>
                <a class="sp-row sp-mod-row" class:visit=item.visit href=beside aria-current=move || current.get().then_some("true") data-noscroll="" title=item.title.clone()>
                    {key}
                    <b>{item.title.clone()}</b>
                    {visit}
                </a>
                <div class="sp-cats" style=move || format!("--word: {}", word.get().max(4))>
                    <For each=move || events.get() key=|event| event.clone() children=row_of/>
                    <For each=move || mine.get() key=|row| row.clone() children=own_row/>
                    {none}
                    {add}
                </div>
            </div>
        }
    };
    let show_all = move |_| {
        let Some(plan) = ctx.plan else { return };
        let (key, n) = (ctx.key.get_untracked(), hidden.get_untracked());
        let undo = PlanUndo::made(plan.with_untracked(Clone::clone), |doc| doc.show_hidden(key));
        ctx.undo.set(Some((format!("{}{}", super::own::OWN_NOTE, (m.shown_again)(n)), undo)));
        plan.update(move |doc| doc.show_hidden(key));
    };
    view! {
        <section class="sp-list" aria-label=m.list>
            <div class="sp-list-head">
                <h2 class="label">{m.heading}</h2>
            </div>
            {move || items.with(Vec::is_empty).then(|| view! { <p class="sp-cal-came">{(t.studyplan_calendars.none_in)(&semester.get())}</p> })}
            <For each=move || items.get() key=|item| item.clone() children=module/>
            // The Termine of one's own of no module of the week, and „+ Termin" (`own.rs`).
            <super::own::OwnList ctx calendar=0 hue=Signal::stored("t-own")/>
            // While editing, what is hidden can be brought back at once (`PlanDoc::show_hidden`).
            {move || {
                let n = hidden.get();
                (ctx.edit.get() && n > 0).then(|| view! {
                    <p class="sp-list-foot">
                        <span>{(t.studyplan_head.hidden)(n)}</span>" · "
                        <button class="link sp-show-all" type="button" id=SHOW_ALL_ID data-action="show-hidden" on:click=show_all>{t.studyplan_head.unhide_all}</button>
                    </p>
                })
            }}
            // The modules of the study are planned in „Mein Studium" (owner, 2026-10-26 and
            // 2026-11-17).
            <a class="sp-way" href=study>
                <span>{m.to_study}</span><Icon name="chevron-right"/>
            </a>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use folia_calendar::select::Selection;
    use folia_model::labels::Code;
    use folia_model::rows::{CatalogRow, Meta};
    use folia_model::rows_detail::{DateCount, DateRow, EventDate};
    use folia_calendar::day::Day;
    use folia_model::rows_detail::ChoiceOption;
    use folia_timetable::model::Input;

    use super::super::week::tests::{row, winter};

    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn catalog_row(id: &str, title: &str, credits: Option<f64>) -> CatalogRow {
        CatalogRow {
            id: id.into(),
            title: title.into(),
            title_de: None,
            title_en: None,
            credits,
            turnus_season: None,
            turnus_parity: None,
            offer_status: Code::parse("active"),
            teaches_german: None,
            teaches_english: None,
            is_fues: false,
            is_limited: None,
            department: None,
            teaching_events: 0,
            exam_form: None,
            responsible: None,
            kind: None,
            kind_source: None,
            kind_basis: None,
            plan_semester: None,
            area: None,
            courses: 0,
            choices: Vec::new(),
            marks: None,
            fues_note: None,
        }
    }

    fn dated(module: &str) -> DateRow {
        DateRow {
            module_id: module.into(),
            ord: Some(1),
            cancelled_dates: None,
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: "148701".into(),
                event_number: None,
                event_title: String::new(),
                event_type: Some("Vorlesung".into()),
                group_name: None,
                weekday: Some(2),
                start_time: Some("11:30".into()),
                end_time: Some("13:00".into()),
                rhythm: Some(Code::parse("weekly")),
                rhythm_raw: None,
                first_date: Some("2026-10-06".into()),
                last_date: Some("2027-01-26".into()),
                room: None,
                campus: None,
                instructor: None,
                comment: None,
                source_url: None,
                room_short: None,
                series: None,
                track: None,
                choice: None,
                option: None,
                extra: false,
            },
        }
    }

    /// Informatik's first semester in plan order: 12104 with its abbreviation and a Termin, 12107
    /// whose abbreviation another planned module shares, 11112 without any Termin, and 13000,
    /// which the catalog does not know.
    fn first_semester() -> StudyplanData {
        StudyplanData {
            key: key("2026W"),
            locale: folia_locale::Locale::De,
            label: "WiSe 2026/27".into(),
            semester: None,
            meta: Meta { current_semester: Some("2026W".into()), ..Default::default() },
            ids: vec!["12104".into(), "12107".into(), "11112".into(), "13000".into()],
            modules: vec![
                catalog_row("11112", "Mathematik IT-1", Some(8.0)),
                catalog_row("12104", "Entwicklung von Softwaresystemen", Some(6.0)),
                catalog_row("12107", "Elektrische und elektronische Grundlagen der Informatik", Some(7.5)),
            ],
            missing: vec!["13000".into()],
            schedule: vec![dated("12104"), dated("12107")],
            exams: Vec::new(),
            sws: Vec::new(),
            counts: vec![DateCount { rhythm: Some(Code::parse("weekly")), first_date: "2026-10-05".into(), last_date: Some("2027-01-29".into()), dates: 40 }],
            abbrevs: [("12104", "EvS"), ("12107", "MIT"), ("11112", "MIT")].iter().map(|(id, abbrev)| (id.to_string(), abbrev.to_string())).collect(),
            program: Some("079-82-2008".into()),
            taken: Vec::new(),
            choices: Vec::new(),
            listed: Vec::new(),
        }
    }

    #[test]
    fn a_planned_module_reads_as_a_row_of_the_catalog() {
        let item = |id: &str, key: Option<&str>, title: &str, hue: &'static str, undated: bool| ModuleItem { id: id.into(), key: key.map(str::to_string), title: title.into(), hue, visit: false, undated };
        assert_eq!(
            module_items(&first_semester(), &BTreeSet::new(), &i18n::DE),
            [
                item("12104", Some("EvS"), "Entwicklung von Softwaresystemen", "t-ice", false),
                // An abbreviation two planned modules share names neither: the week says its name.
                item("12107", None, "Elektrische und elektronische Grundlagen der Informatik", "t-sun", false),
                item("11112", None, "Mathematik IT-1", "t-violet", true),
                // The catalog does not know it: it is no module to enter a Termin for.
                item("13000", None, "Modul 13000", "t-teal", false),
            ]
        );
        // A semester without published dates says nothing of a module's own.
        let unpublished = StudyplanData { counts: Vec::new(), ..first_semester() };
        assert!(!module_items(&unpublished, &BTreeSet::new(), &i18n::DE)[2].undated);
        // A visit is one; in English a module the catalog does not know is named so.
        let visits: BTreeSet<String> = ["12107".to_string()].into();
        let english = module_items(&first_semester(), &visits, &i18n::EN);
        assert!(english[1].visit && !english[0].visit);
        assert_eq!(english[3].title, "Module 13000");
    }

    fn said(summary: Summary) -> String {
        summary_text(&summary, &i18n::DE).plain(&i18n::DE)
    }

    /// Every case of the summary, each in its words.
    #[test]
    fn a_row_says_what_the_plan_holds_of_its_event() {
        let week = |day: u8, from: u16| (day, from, Weeks::All);
        assert_eq!(said(Summary { weekly: vec![week(2, 690), week(5, 555)], ..Summary::default() }), "Di 11:30 · Fr 09:15");
        assert_eq!(said(Summary { weekly: vec![(2, 450, Weeks::A), week(2, 555)], ..Summary::default() }), "Di 07:30 A · Di 09:15");
        assert_eq!(said(Summary { weekly: vec![(2, 450, Weeks::B)], ..Summary::default() }), "Di 07:30 B");
        assert_eq!(said(Summary { weekly: vec![week(1, 555), week(3, 555), week(5, 555)], ..Summary::default() }), "Mo · Mi · Fr");
        assert_eq!(said(Summary { weekly: (1..=5).map(|day| week(day, 555)).collect(), ..Summary::default() }), "5 Termine");
        assert_eq!(said(Summary { dates: 5, ..Summary::default() }), "5 Termine");
        assert_eq!(said(Summary { groups: Some((0, 4)), ..Summary::default() }), "4 Gruppen · offen");
        assert_eq!(said(Summary { dates: 1, groups: Some((0, 3)), ..Summary::default() }), "3 Gruppen · offen");
        assert!(summary_text(&Summary { groups: Some((0, 4)), ..Summary::default() }, &i18n::DE).open);
        assert_eq!(said(Summary { weekly: vec![week(1, 930)], groups: Some((1, 4)), ..Summary::default() }), "Mo 15:30 · 1 von 4");
        assert_eq!(said(Summary { weekly: vec![week(1, 930), week(2, 930)], groups: Some((2, 4)), ..Summary::default() }), "Mo 15:30 · Di 15:30 · 2 von 4");
        assert_eq!(said(Summary { weekly: vec![week(2, 450)], option_open: true, ..Summary::default() }), "Di 07:30 · offen");
        assert_eq!(said(Summary::default()), "ohne feste Zeit");
        assert_eq!(said(Summary { weekly: vec![week(2, 930)], extra: true, ..Summary::default() }), "Di 15:30 · freiwillig");
        assert_eq!(said(Summary { weekly: vec![week(2, 930)], extra: true, out: Some(Out::Hidden), ..Summary::default() }), "ausgeblendet");
        assert_eq!(said(Summary { out: Some(Out::Kind), ..Summary::default() }), "ausgeblendet");
        assert_eq!(said(Summary { out: Some(Out::Unchosen), ..Summary::default() }), "nicht gewählt");
        // In English.
        let english = summary_text(&Summary { weekly: vec![week(1, 930)], groups: Some((1, 4)), ..Summary::default() }, &i18n::EN).plain(&i18n::EN);
        assert_eq!(english, "Mon 15:30 · 1 of 4");
        assert_eq!(summary_text(&Summary { groups: Some((0, 4)), ..Summary::default() }, &i18n::EN).plain(&i18n::EN), "4 groups · open");
    }

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    /// The timetable of rows of 2026W with the choices among their events `choices`.
    fn table(schedule: &[DateRow], modules: &[&str], choices: &[ChoiceOption], selection: &Selection) -> Timetable {
        let facts = winter();
        let modules: Vec<String> = modules.iter().map(|id| id.to_string()).collect();
        let input = Input { key: key("2026W"), semester: None, facts: &facts, modules: &modules, schedule, exams: &[], sws: &[], taken: &[], choices, listed: &[] };
        Timetable::build(&input, selection)
    }

    fn rowkey(row: &DateRow) -> folia_calendar::rowkey::RowKey {
        folia_calendar::rowkey::RowKey::of(&row.date).unwrap()
    }

    const TERM: (&str, &str) = ("2026-10-05", "2027-01-29");

    /// EvS of the example week: a lecture on Tuesday and Friday, an exercise of two groups, and a
    /// tutorial beside them.
    fn evs() -> Vec<DateRow> {
        let group = |ord: i64, name: &str, weekday: i64, time: (&str, &str)| {
            let mut row = row("148369", ord, "Übung", "weekly", weekday, time, TERM);
            row.date.group_name = Some(name.into());
            row
        };
        let mut tutorial = row("150132", 1, "Tutorium", "weekly", 2, ("15:30", "17:00"), TERM);
        tutorial.date.extra = true;
        vec![
            row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), TERM),
            row("148701", 2, "Vorlesung", "weekly", 5, ("09:15", "10:45"), TERM),
            group(1, "1-Gruppe", 1, ("15:30", "17:00")),
            group(2, "2-Gruppe", 1, ("17:30", "19:00")),
            tutorial,
        ]
    }

    fn shown(selection: &Selection) -> Vec<(String, RowShown)> {
        let names: BTreeMap<String, String> = [("12104".to_string(), "EvS".to_string())].into();
        let t = table(&evs(), &["12104"], &[], selection);
        rows_of(&t, &names, &StudyplanUrl::parse("edit=1"), &i18n::DE).remove("12104").unwrap()
    }

    #[test]
    fn a_row_leads_to_the_overview_and_carries_its_eye() {
        let rows = shown(&Selection::default());
        let words: Vec<(&str, &str, String)> = rows.iter().map(|(id, row)| (id.as_str(), row.label.as_str(), row.said.plain(&i18n::DE))).collect();
        // Kind first, as the overview's segments: the lecture, the exercise, the tutorial — though
        // the three share one title and the exercise's id is the lowest.
        assert_eq!(words, [("148701", "Vorlesung", "Di 11:30 · Fr 09:15".to_string()), ("148369", "Übung", "2 Gruppen · offen".to_string()), ("150132", "Tutorium", "Di 15:30 · freiwillig".to_string())]);
        let of = |rows: &[(String, RowShown)], id: &str| rows.iter().find(|(event, _)| event == id).map(|(_, row)| row.clone()).unwrap();
        let lecture = of(&rows, "148701");
        // The overview at its first Termin, in the mode it was opened from.
        assert_eq!(lecture.href, format!("/studyplan?edit=1&termin={}&full=1", rowkey(&evs()[0]).text()));
        assert_eq!(lecture.aria, "Vorlesung EvS, Di 11:30 · Fr 09:15: alle Termine");
        assert!(!lecture.beside && lecture.title == "Vorlesung · Di 11:30 · Fr 09:15");
        assert_eq!(lecture.eye, Some((true, "Vorlesung EvS zeigen".to_string(), Flip::Hide(148701))));
        assert!(!lecture.pale && lecture.sign.is_none() && lecture.hid.is_none());
        // Hidden whole: pale, „ausgeblendet", the crossed eye as a sign, the eye shows it again.
        let hidden = shown(&Selection { hidden_events: [148701].into(), ..Selection::default() });
        let lecture = of(&hidden, "148701");
        assert_eq!((lecture.said.text.as_str(), lecture.pale, lecture.sign.as_deref()), ("ausgeblendet", true, Some("Vorlesung ausgeblendet")));
        assert_eq!(lecture.eye, Some((false, "Vorlesung EvS zeigen".to_string(), Flip::Show(148701))));
        // The Tuesday hidden: the Friday stays, and the row says one Termin is hidden.
        let part = shown(&Selection { hidden_rows: [rowkey(&evs()[0])].into(), ..Selection::default() });
        let lecture = of(&part, "148701");
        assert_eq!((lecture.said.text.as_str(), lecture.hid.as_deref(), lecture.sign.as_deref(), lecture.pale), ("Fr 09:15", Some("1 Termin ausgeblendet"), Some("1 Termin ausgeblendet"), false));
        assert!(lecture.eye.as_ref().is_some_and(|(on, ..)| *on));
        // A date hidden: named by its day; a group hidden: a group.
        let date = shown(&Selection { hidden_dates: [(rowkey(&evs()[1]), d("2026-11-20"))].into(), ..Selection::default() });
        assert_eq!(of(&date, "148701").hid.as_deref(), Some("am Fr 20.11. ausgeblendet"));
        let group = shown(&Selection { hidden_rows: [rowkey(&evs()[2])].into(), ..Selection::default() });
        assert_eq!(of(&group, "148369").hid.as_deref(), Some("1 Gruppe ausgeblendet"));
        // Every Termin hidden by itself (one click on each line's eye in the module's panel): the
        // row says „ausgeblendet", its eye is crossed, and a click shows those Termine again.
        let both = [rowkey(&evs()[0]), rowkey(&evs()[1])];
        let all = shown(&Selection { hidden_rows: both.into(), ..Selection::default() });
        let lecture = of(&all, "148701");
        let mut keys = both.to_vec();
        keys.sort_unstable();
        assert_eq!((lecture.said.text.as_str(), lecture.pale, lecture.sign.as_deref()), ("ausgeblendet", true, Some("Vorlesung ausgeblendet")));
        assert_eq!(lecture.eye, Some((false, "Vorlesung EvS zeigen".to_string(), Flip::ShowTermine(keys))));
        // A kind of before switched off: its eye switches the kind on again and says so.
        use folia_calendar::kind::KindSet;
        let kind = shown(&Selection { hidden_kinds: KindSet::default().with(EventKind::Tutorial), ..Selection::default() });
        assert_eq!(of(&kind, "150132").eye, Some((false, "„Tutorium“ wieder einblenden".to_string(), Flip::ShowKind(EventKind::Tutorial))));
    }

    #[test]
    fn an_option_another_was_taken_over_has_no_eye() {
        // Two exercises of 12104, one of which one takes (`v_module_choice`).
        let option = |event: &str, option: u32, weekday: i64| {
            let mut row = row(event, 1, "Übung", "weekly", weekday, ("07:30", "09:00"), TERM);
            (row.date.choice, row.date.option) = (Some(1), Some(option));
            row
        };
        let choice = |option: u32, event: &str| ChoiceOption {
            module_id: "12104".into(),
            semester_key: "2026W".into(),
            choice: 1,
            course: None,
            kind: Code::parse("group"),
            basis: Code::parse("form"),
            sws: Some(2.0),
            pick: 1,
            option,
            event_id: event.into(),
            label: None,
            event_sws: Some(2.0),
        };
        let rows = [option("61", 1, 2), option("62", 2, 3)];
        let t = table(&rows, &["12104"], &[choice(1, "61"), choice(2, "62")], &Selection { chosen_rows: [rowkey(&rows[1])].into(), ..Selection::default() });
        let all = rows_of(&t, &BTreeMap::new(), &StudyplanUrl::default(), &i18n::DE).remove("12104").unwrap();
        let (_, other) = &all[0];
        assert_eq!((other.said.text.as_str(), other.pale, other.eye.is_none()), ("nicht gewählt", true, true));
        assert!(all[1].1.eye.is_some());
        // „Offen" says of the choice decided by „Diese nehmen" what was taken, whichever of its
        // events its line was named by — never „nicht gewählt".
        let fates = super::super::open::fates(&t, &i18n::DE);
        assert_eq!(fates.get("x61").map(String::as_str), Some("Mi 07:30 ✓"));
        assert_eq!(fates.get("x62").map(String::as_str), Some("Mi 07:30 ✓"));
    }

    /// An eye writes an `e` line, shows it again without one, and a kind is switched on; the ticks
    /// stay as they were.
    #[test]
    fn an_eye_hides_and_shows_its_event() {
        let w = key("2026W");
        let mut doc = PlanDoc::default();
        assert!(doc.plan(w, "12104", 1, None));
        let group = rowkey(&evs()[2]);
        doc.tick(w, group, true);
        let ticks = doc.selection(w, Default::default()).chosen_rows;
        Flip::Hide(148701).apply(&mut doc, w);
        assert!(doc.selection(w, Default::default()).hidden_events.contains(&148701));
        Flip::Show(148701).apply(&mut doc, w);
        assert!(doc.selection(w, Default::default()).hidden_events.is_empty());
        doc.set_kind(w, EventKind::Tutorial, true);
        Flip::ShowKind(EventKind::Tutorial).apply(&mut doc, w);
        assert!(doc.selection(w, Default::default()).hidden_kinds.is_empty());
        assert_eq!(doc.selection(w, Default::default()).chosen_rows, ticks);
    }

    /// The eye of every Termin hidden shows them again, and nothing else: the ticks stay.
    #[test]
    fn an_eye_shows_the_termine_hidden_one_by_one() {
        let w = key("2026W");
        let mut doc = PlanDoc::default();
        assert!(doc.plan(w, "12104", 1, None));
        let (tuesday, friday) = (rowkey(&evs()[0]), rowkey(&evs()[1]));
        doc.tick(w, rowkey(&evs()[2]), true);
        doc.set_row(w, tuesday, true);
        doc.set_row(w, friday, true);
        let t = table(&evs(), &["12104"], &[], &doc.selection(w, Default::default()));
        let rows = rows_of(&t, &BTreeMap::new(), &StudyplanUrl::default(), &i18n::DE).remove("12104").unwrap();
        let (_, lecture) = rows.iter().find(|(id, _)| id == "148701").unwrap();
        let Some((false, _, flip)) = lecture.eye.clone() else { panic!("{:?}", lecture.eye) };
        flip.apply(&mut doc, w);
        let after = doc.selection(w, Default::default());
        assert!(after.hidden_rows.is_empty() && after.hidden_events.is_empty());
        assert_eq!(after.chosen_rows, [rowkey(&evs()[2])].into());
    }

    /// A Termin without any date has no overview: its row leads to the module beside the plan,
    /// pointing at it, and keeps the page's place.
    #[test]
    fn a_row_without_a_date_leads_to_the_module() {
        let mut colloquium = row("148098", 1, "Kolloquium", "weekly", 1, ("09:15", "10:45"), TERM);
        (colloquium.date.weekday, colloquium.date.start_time, colloquium.date.end_time, colloquium.date.first_date, colloquium.date.last_date) = (None, None, None, None, None);
        let rows = [row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), TERM), colloquium.clone()];
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let names: BTreeMap<String, String> = [("12104".to_string(), "EvS".to_string())].into();
        let all = rows_of(&t, &names, &StudyplanUrl::parse("edit=1"), &i18n::DE).remove("12104").unwrap();
        let (_, kolloquium) = all.iter().find(|(id, _)| id == "148098").unwrap();
        assert_eq!(kolloquium.href, format!("/studyplan?edit=1&open=12104&row={}", rowkey(&colloquium).text()));
        assert!(kolloquium.beside && kolloquium.aria.ends_with(": im Modul"));
        assert_eq!(kolloquium.said.plain(&i18n::DE), "ohne feste Zeit");
        let (_, lecture) = all.iter().find(|(id, _)| id == "148701").unwrap();
        assert!(!lecture.beside);
    }

    /// In English the eye's name takes Betula's label of the kind, never QIS's word lower-cased.
    #[test]
    fn english_eyes_take_the_kinds_labels() {
        let names: BTreeMap<String, String> = [("12104".to_string(), "EvS".to_string())].into();
        let mut rows = evs();
        rows.push(row("148702", 1, "Übung", "weekly", 3, ("09:15", "10:45"), TERM));
        let t = table(&rows, &["12104"], &[], &Selection::default());
        let all = rows_of(&t, &names, &StudyplanUrl::default(), &i18n::EN).remove("12104").unwrap();
        let words: Vec<(&str, &str)> = all.iter().map(|(_, row)| (row.label.as_str(), row.eye.as_ref().map_or("", |(_, name, _)| name.as_str()))).collect();
        assert_eq!(words, [("Lecture", "Show lecture EvS"), ("Exercise · Mon", "Show exercise · Mon EvS"), ("Exercise · Wed", "Show exercise · Wed EvS"), ("Tutorial", "Show tutorial EvS")]);
    }
}
