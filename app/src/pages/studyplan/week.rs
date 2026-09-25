//! The views „Woche" and „Termine" of one semester of the Studienplan.
//!
//! „Woche" is the Regelwoche (`Timetable::regular_week`): a slot per recurring Termin at its
//! weekday and time in the week grid (`crate::week`), and the dates of an event that do not recur
//! gathered into one slot („3 Termine"). Each slot is a link to its module beside the plan,
//! pointing at that Termin (`open`, `row`). A phone has no room for five columns: there the same
//! slots are a list of days. What has no fixed time stands under „Ohne feste Zeit", once for the
//! semester. Where the plan has Termine of A or B weeks only, „A-Woche · B-Woche · A/B" above the
//! week shows one kind of week or both (`PlanCtx::weeks`); a slot that overlaps another in the
//! week shown is red and names the other in its tooltip (owner's redesign of 2026-09-25).
//!
//! „Termine" is the agenda (`Timetable::agenda`): every date by week and day, what is cancelled
//! and why, the holidays, the exams, and the weeks of a break with nothing in them as one line. A
//! plan with A- or B-week Termine names each week of the lecture period „A-Woche" or „B-Woche".
//! What it cannot place on a day (a Termin „nach Vereinbarung", an exam whose date is open)
//! stands under „Ohne Datum". It opens at the current week (the page reads the clock once,
//! `PlanCtx::today`).
//!
//! A view comes back to where the visitor left it for a module (`Place`): after „Vollbild" and
//! „Zurück", and on a phone, where the module was the page, when it is closed.
//!
//! Both views read the timetable and the address without what stands beside the plan (`base`),
//! two siblings of the address (R16). The slot or date being opened is marked from where the app
//! is going (`Pending`), so a click answers in the next frame (R21). What they show is built as
//! plain values first (`PlanSlot`, `Block`, `LooseLine`), which the tests read without rendering;
//! the lists are keyed by those values, so a hidden event redraws the days it was on and no
//! other (R5).

use std::collections::{BTreeMap, BTreeSet};

use catalog::labels::Rhythm;
use catalog::rows_detail::EventDate;
use catalog::timetable::clash::Weeks;
use catalog::timetable::day::{clock, Day};
use catalog::timetable::exams::{ExamShape, Termin};
use catalog::timetable::facts::SemesterFacts;
use catalog::timetable::model::{Event, Row, Timetable};
use catalog::timetable::occur::Every;
use catalog::timetable::rowkey::RowKey;
use catalog::timetable::views::{kind_and_title, kind_short, short_title, type_text, AgendaItem, AgendaWeek, Reach, WeekItem, WeekLabel};
use catalog::url::{self, StudyplanUrl};
use leptos::prelude::*;

use super::head::{hue, tone_at};
use super::PlanCtx;
use crate::format;
use crate::nav;
use crate::pages::catalog::phone_layout;
use crate::pending::Pending;
use crate::ui::Icon;
use crate::week::{GridSlot, WeekGrid};

/// Where this browser tab remembers the link the visitor last left a view by (`Place`).
const LEFT_KEY: &str = "betula.studyplan.left";

const WEEKDAYS: [&str; 7] = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"];

/// The module and the Termin the address puts beside the plan: what a slot or a row is marked by.
type Picked = (Option<String>, Option<RowKey>);

/// „Woche": the Regelwoche as a grid, on a phone as a list of days, and what has no fixed time.
#[component]
pub(super) fn WeekView(ctx: PlanCtx) -> impl IntoView {
    let base = base_of(ctx);
    let picked = picked_of(ctx);
    // What the slots name the modules by, their abbreviations („EvS"; owner, 2026-09-25: only where
    // no title fits): a sibling of the timetable, both derived from the semester's data (R16).
    let titles = Memo::new(move |_| ctx.data.with(|data| data.as_ref().map(|data| data.slot_names()).unwrap_or_default()));
    let slots = Memo::new(move |_| {
        let (base, shown) = (base.get(), ctx.weeks.get());
        titles.with(|titles| ctx.table.with(|table| table.as_ref().map(|table| week_slots(table, &base, titles, shown)).unwrap_or_default()))
    });
    let ab = Memo::new(move |_| ctx.table.with(|table| table.as_ref().is_some_and(has_ab)));
    let loose = Memo::new(move |_| {
        let base = base.get();
        ctx.table.with(|table| table.as_ref().map(|table| loose_lines(table, &base, &table.loose())).unwrap_or_default())
    });
    let phone = phone_layout();
    let week = move || {
        if phone.get() {
            return view! { <DayList slots picked/> }.into_any();
        }
        let grid = Signal::derive(move || {
            let picked = picked.get();
            slots.with(|slots| slots.iter().map(|slot| GridSlot { current: slot.row.is(&picked), ..slot.slot.clone() }).collect::<Vec<_>>())
        });
        view! { <WeekGrid slots=grid/> }.into_any()
    };

    // Back from a module that filled the page („Vollbild", then „Zurück"; the address names it
    // still): at the line it was opened from. On a phone the module beside the plan is the page:
    // there the plan comes back when it is closed.
    Effect::new(move |_| {
        let here = ctx.url.get_untracked();
        if here.open.is_some() && !nav::is_phone() {
            come_back(&here, None);
        }
    });
    back_on_phone(ctx, || None);

    view! {
        {move || ab.get().then(|| view! { <WeekSwitch weeks=ctx.weeks/> })}
        {week}
        <Loose title="Ohne feste Zeit" lines=loose/>
    }
}

/// „A-Woche · B-Woche · A/B": which kind of week the Regelwoche shows. The click answers at once
/// (R21): a signal of the page, the week is worked out again in Rust.
#[component]
fn WeekSwitch(weeks: RwSignal<Weeks>) -> impl IntoView {
    let choice = move |(week, label): (Weeks, &'static str)| {
        view! {
            <button type="button" role="radio" aria-checked=move || if weeks.get() == week { "true" } else { "false" } on:click=move |_| weeks.set(week)>
                {label}
            </button>
        }
    };
    view! {
        <div class="seg sp-weeks" role="radiogroup" aria-label="Woche">
            {[(Weeks::A, "A-Woche"), (Weeks::B, "B-Woche"), (Weeks::All, "A/B")].into_iter().map(choice).collect_view()}
        </div>
    }
}

/// Whether a shown Termin of the plan is held in A or B weeks only: then the week can show one
/// kind of week, and the agenda names its weeks.
fn has_ab(table: &Timetable) -> bool {
    table
        .events
        .iter()
        .filter(|event| event.hidden.is_none())
        .flat_map(|event| event.rows.iter().filter(|row| row.hidden.is_none()))
        .any(|row| matches!(Every::of(&row.date), Some(Every::AWeek | Every::BWeek)))
}

/// „Termine": the agenda by week, opened at the current one, and what has no date.
#[component]
pub(super) fn DatesView(ctx: PlanCtx) -> impl IntoView {
    let base = base_of(ctx);
    let today = ctx.today;
    let blocks = Memo::new(move |_| {
        let base = base.get();
        ctx.table.with(|table| table.as_ref().map(|table| agenda_blocks(table, &table.agenda(), &base, today)).unwrap_or_default())
    });
    let loose = Memo::new(move |_| {
        let base = base.get();
        ctx.table.with(|table| {
            table
                .as_ref()
                .map(|table| loose_lines(table, &base, &undated(table)).into_iter().chain(open_exams(table, &base)).collect())
                .unwrap_or_default()
        })
    });
    // The current week, as the agenda has it now; `None` without a clock.
    let current = move |blocks: &[Block]| today.and_then(|today| scroll_target(blocks, today));

    // The agenda opens once, when it first has something to show: at the current week, or back
    // from a module that filled the page (the address names it still) at the date it was opened
    // from. Later changes of what is shown leave the page where the visitor put it. On a phone,
    // while the module beside the plan is the page, the plan is not shown: it opens when the
    // module is closed (`back_on_phone`).
    let done = StoredValue::new(false);
    Effect::new(move |_| {
        if done.get_value() {
            return;
        }
        let Some(week) = blocks.with(|blocks| (!blocks.is_empty()).then(|| current(blocks))) else { return };
        let here = ctx.url.get_untracked();
        if here.open.is_some() && nav::is_phone() {
            return;
        }
        done.set_value(true);
        match (&here.open, week) {
            (Some(_), week) => come_back(&here, week),
            (None, Some(week)) => reveal(week),
            (None, None) => {}
        }
    });
    back_on_phone(ctx, move || {
        done.set_value(true);
        blocks.with_untracked(|blocks| current(blocks))
    });

    view! {
        <div class="agenda">
            <For each=move || blocks.get() key=|block| block.clone() children=block_view/>
        </div>
        <Loose title="Ohne Datum" lines=loose/>
    }
}

/// The address of the view without the module beside it: what every link of a view starts from.
/// A sibling of the timetable (both follow the address), never read with the address itself (R16).
fn base_of(ctx: PlanCtx) -> Memo<StudyplanUrl> {
    Memo::new(move |_| ctx.url.with(|url| url.with_open(None, None)))
}

/// The module and Termin beside the plan, or where a click is taking them: marked in the next
/// frame (R21), before the router has the address.
fn picked_of(ctx: PlanCtx) -> Memo<Picked> {
    let going = Pending::expect();
    Memo::new(move |_| {
        let (open, row) = match going.and_then(|going| going.search_on(url::STUDYPLAN)) {
            Some(search) => {
                let to = StudyplanUrl::parse(&search);
                (to.open, to.row)
            }
            None => ctx.url.with(|url| (url.open.clone(), url.row.clone())),
        };
        (open, row.as_deref().and_then(RowKey::parse))
    })
}

/// Scrolls the week `id` to the top a frame later: after the router has put a page it opened at
/// its top.
#[allow(unused_variables)]
fn reveal(id: String) {
    #[cfg(feature = "csr")]
    request_animation_frame(move || {
        week_to_top(&id);
    });
}

/// Scrolls the week `id` to the top of what scrolls around it (the page's column, on a phone the
/// window). `false` if the agenda has no such week.
#[allow(unused_variables)]
fn week_to_top(id: &str) -> bool {
    #[cfg(feature = "csr")]
    {
        let Some(window) = web_sys::window() else { return false };
        let Some(document) = window.document() else { return false };
        let Some(element) = document.get_element_by_id(id) else { return false };
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Start);
        element.scroll_into_view_with_scroll_into_view_options(&options);
        // On a phone the window scrolls under the top bar, which stays: the week goes below it.
        if nav::is_phone() {
            if let Some(bar) = document.query_selector(".topbar").ok().flatten() {
                window.scroll_by_with_x_and_y(0.0, -bar.get_bounding_client_rect().bottom());
            }
        }
        true
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// On a phone the module beside the plan is the page, and the plan waits unseen (with the window
/// scrolled for the module). Closing the module brings the view back where the visitor left it
/// (`come_back`), else at `week`.
fn back_on_phone(ctx: PlanCtx, week: impl Fn() -> Option<String> + 'static) {
    Effect::new(move |before: Option<StudyplanUrl>| {
        let here = ctx.url.get();
        if let Some(left) = before.filter(|before| before.open.is_some()) {
            if here.open.is_none() && nav::is_phone() {
                come_back(&left, week());
            }
        }
        here
    });
}

/// Scrolls back to where the visitor left `left` (an address with the module beside the view):
/// to the link they opened, else to its week, else to `week`. Once in the next frame and once more
/// after the browser has restored its own idea of the scroll position (a step back through the
/// history), as the Merkliste does.
fn come_back(left: &StudyplanUrl, week: Option<String>) {
    let place = nav::session_get(LEFT_KEY).as_deref().and_then(Place::restored).filter(|place| place.left(left));
    let pass = move || {
        if let Some(place) = &place {
            if nav::reveal_selector(&place.selector()) || place.week().is_some_and(week_to_top) {
                return;
            }
        }
        if let Some(week) = &week {
            week_to_top(week);
        }
    };
    #[cfg(feature = "csr")]
    {
        let again = pass.clone();
        request_animation_frame(pass);
        set_timeout(again, std::time::Duration::from_millis(220));
    }
    // The server renders no plan and scrolls nothing.
    #[cfg(not(feature = "csr"))]
    let _ = pass;
}

/// What holds a link a view is left by: a week of the agenda (its anchor), the phone's list of
/// days, the lines without a fixed time or date.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Within {
    Week(String),
    Days,
    Loose,
}

/// The link the visitor last opened a module by, and what holds it: where the view comes back to
/// (`come_back`). Kept for the browser tab (`LEFT_KEY`), since a view that the module's whole page
/// replaced is built anew.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Place {
    within: Within,
    href: String,
}

impl Place {
    /// „kw-2026-50 /studyplan?…", „days /studyplan?…", „loose /studyplan?…".
    fn stored(&self) -> String {
        let within = match &self.within {
            Within::Week(id) => id.as_str(),
            Within::Days => "days",
            Within::Loose => "loose",
        };
        format!("{within} {}", self.href)
    }

    /// What `stored` wrote, checked like anything read from storage: an anchor as the agenda
    /// writes it, and a link of the plan made of the characters its addresses have (it becomes
    /// part of a selector).
    fn restored(text: &str) -> Option<Place> {
        let (within, href) = text.split_once(' ')?;
        let within = match within {
            "days" => Within::Days,
            "loose" => Within::Loose,
            id if is_week_id(id) => Within::Week(id.to_string()),
            _ => return None,
        };
        let plain = |c: char| c.is_ascii_alphanumeric() || "/?&=-_.%+".contains(c);
        let ours = href.strip_prefix(url::STUDYPLAN).is_some_and(|rest| rest.starts_with('?')) && href.chars().all(plain);
        ours.then(|| Place { within, href: href.to_string() })
    }

    /// The link in the page: `#kw-2026-50 a[href="…"]`, `.sp-daylist a[href="…"]`.
    fn selector(&self) -> String {
        let within = match &self.within {
            Within::Week(id) => format!("#{id}"),
            Within::Days => ".sp-daylist".to_string(),
            Within::Loose => ".sp-loose".to_string(),
        };
        format!("{within} a[href=\"{}\"]", self.href)
    }

    fn week(&self) -> Option<&str> {
        match &self.within {
            Within::Week(id) => Some(id),
            Within::Days | Within::Loose => None,
        }
    }

    /// Whether the visitor left `url` here: the same semester and view, the same module beside it.
    /// (Another module opened since, from the legend or the notes, is not left from here.)
    fn left(&self, url: &StudyplanUrl) -> bool {
        let there = StudyplanUrl::parse(self.href.split_once('?').map_or("", |(_, query)| query));
        (&there.sem, there.view, &there.open) == (&url.sem, url.view, &url.open)
    }
}

/// `kw-2026-41`, as `week_id` writes it.
fn is_week_id(id: &str) -> bool {
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    id.strip_prefix("kw-").and_then(|rest| rest.split_once('-')).is_some_and(|(year, week)| digits(year) && digits(week))
}

/// Remembers the link a click in `within` follows (`Place`).
fn remember(within: Within, ev: &leptos::ev::MouseEvent) {
    if let Some(href) = nav::link_under(ev.target()) {
        nav::session_set(LEFT_KEY, &Place { within, href }.stored());
    }
}

// ---------- the Regelwoche ----------

/// A slot of the Regelwoche: as the grid draws it, and as a row of the phone's list of days.
#[derive(Clone, Debug, PartialEq)]
struct PlanSlot {
    /// Never `current`: the page marks the slot from the address.
    slot: GridSlot,
    row: DayRow,
}

/// A slot as a row of the list of days: the time, then what it is and what else to know.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DayRow {
    day: u8,
    /// The tone, and `alt` and `clash` as in the grid.
    class: String,
    /// „15:30–17:00"
    time: String,
    /// „Übung · Entwicklung von Softwaresystemen"
    text: String,
    /// „1 von 4", „A-Woche · bis 23.11.", „3 Termine"; empty when there is nothing to add.
    note: String,
    href: String,
    /// The planned modules of its event and its Termine: what the address's `open` and `row` name.
    modules: Vec<String>,
    keys: Vec<RowKey>,
}

impl DayRow {
    /// Whether this is the Termin beside the plan: its module is open and the address points at
    /// one of its rows.
    fn is(&self, (open, row): &Picked) -> bool {
        match (open, row) {
            (Some(open), Some(row)) => self.modules.contains(open) && self.keys.contains(row),
            _ => false,
        }
    }
}

/// The slots of the Regelwoche held in the week `shown`, in its order (weekday, time, event);
/// `titles` are the planned modules' titles by id.
fn week_slots(table: &Timetable, base: &StudyplanUrl, titles: &BTreeMap<String, String>, shown: Weeks) -> Vec<PlanSlot> {
    table.regular_week().iter().filter(|item| item.in_week(shown)).filter_map(|item| plan_slot(table, base, item, titles, shown)).collect()
}

fn plan_slot(table: &Timetable, base: &StudyplanUrl, item: &WeekItem, titles: &BTreeMap<String, String>, week: Weeks) -> Option<PlanSlot> {
    let event = table.events.get(item.event)?;
    let shown = event.rows.get(item.row)?;
    let module = event.modules.first()?;
    let every = Every::of(&shown.date);
    let once = matches!(item.label, WeekLabel::Once { .. });
    let rows: Vec<&Row> = slot_rows(event, item, every).into_iter().filter_map(|r| event.rows.get(r)).collect();
    let notes = qualifiers(&item.label, every, item.alt, &table.facts);
    let hue = hue(event.tone);
    let href = base.with_open(Some(module.as_str()), shown.key).path();

    // The tooltip says in full what the slot shortens: the kind as QIS names it, the whole time,
    // the rooms, and the dates of single ones.
    let mut title = vec![event_text(event), format!("{} {}–{}", day_short(item.day), clock(item.from), clock(item.to))];
    title.extend(notes.iter().cloned());
    title.extend(rooms_long(rows.iter().map(|row| &row.date)));
    if matches!(item.label, WeekLabel::Once { dates, .. } if dates > 1) {
        let mut days: Vec<Day> = rows.iter().flat_map(|row| row.occ.days.iter().copied()).filter(|day| day.weekday() == item.day).collect();
        days.sort_unstable();
        days.dedup();
        title.push(days.iter().map(|day| day.german()).collect::<Vec<_>>().join(", "));
    }
    // What it overlaps in the week shown, by the other slot's label.
    let mut against: Vec<String> = Vec::new();
    for other in item.against_in(week).into_iter().filter_map(|e| table.events.get(e)) {
        let label = slot_label(other, titles);
        if !against.contains(&label) {
            against.push(label);
        }
    }
    let clash = !against.is_empty();
    if clash {
        title.push(format!("überschneidet sich mit {}", against.join(", ")));
    }

    let slot = GridSlot {
        day: item.day,
        from: item.from,
        to: item.to,
        label: slot_label(event, titles),
        small: slot_small(&item.label, every, item.alt, item.from, &table.facts),
        title: title.join(" · "),
        class: if once { "once" } else { "tinted" },
        hue: Some(hue),
        alt: item.alt.is_some(),
        clash,
        href: Some(href.clone()),
        current: false,
    };
    let row = DayRow {
        day: item.day,
        class: classes(hue, &[(item.alt.is_some(), "alt"), (clash, "clash")]),
        time: format!("{}–{}", clock(item.from), clock(item.to)),
        text: event_text(event),
        note: notes.join(" · "),
        href,
        modules: event.modules.clone(),
        keys: rows.iter().filter_map(|row| row.key).collect(),
    };
    Some(PlanSlot { slot, row })
}

/// The rows behind a slot of the Regelwoche: the shown rows of its event at its weekday and
/// time, of its option and its rhythm (one slot in two rooms, or in two ranges one after the
/// other, as `regular_week` merges them).
fn slot_rows(event: &Event, item: &WeekItem, every: Option<Every>) -> Vec<usize> {
    let option = event.rows.get(item.row).and_then(|row| row.option);
    event
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.hidden.is_none() && row.from == Some(item.from) && row.to == Some(item.to) && row.option == option)
        .filter(|(_, row)| Every::of(&row.date) == every)
        .filter(|(_, row)| match every {
            Some(_) => row.occ.template.map(|pattern| pattern.weekday).or_else(|| row.occ.days.first().map(|day| day.weekday())) == Some(item.day),
            None => row.occ.days.iter().any(|day| day.weekday() == item.day),
        })
        .map(|(r, _)| r)
        .collect()
}

/// What a slot says besides its time: which option of a choice not yet made, the weeks it meets
/// in, the part of the lecture period, or how many single dates it gathers.
fn qualifiers(label: &WeekLabel, every: Option<Every>, alt: Option<(usize, usize)>, facts: &SemesterFacts) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some((options, _)) = alt {
        parts.push(format!("1 von {options}"));
    }
    match label {
        WeekLabel::Every(every) => parts.extend(rhythm_word(*every)),
        WeekLabel::Partial { .. } => {
            parts.extend(every.and_then(rhythm_word));
            parts.extend(label.reach(facts).map(reach_text));
        }
        WeekLabel::Once { dates: 1, first } => parts.push(format!("1 Termin · {}", first.short())),
        WeekLabel::Once { dates, .. } => parts.push(format!("{dates} Termine")),
    }
    parts
}

/// The small line of a slot in the grid: its time, then what `qualifiers` adds; single dates are
/// told by their count.
fn slot_small(label: &WeekLabel, every: Option<Every>, alt: Option<(usize, usize)>, from: u16, facts: &SemesterFacts) -> String {
    let notes = qualifiers(label, every, alt, facts);
    match label {
        WeekLabel::Once { .. } => notes.join(" · "),
        _ => std::iter::once(clock(from)).chain(notes).collect::<Vec<_>>().join(" · "),
    }
}

fn rhythm_word(every: Every) -> Option<String> {
    match every {
        Every::Week => None,
        Every::AWeek => Some("A-Woche".to_string()),
        Every::BWeek => Some("B-Woche".to_string()),
        Every::FourWeeks => Some("4-wöch.".to_string()),
    }
}

fn reach_text(reach: Reach) -> String {
    match reach {
        Reach::Until(last) => format!("bis {}", last.short()),
        Reach::From(first) => format!("ab {}", first.short()),
        Reach::Between(first, last) => format!("{}–{}", first.short(), last.short()),
    }
}

/// The rows of the list of days, one group per weekday in the order of the week; each day's rows
/// in the order they come (the Regelwoche's: by time).
fn day_groups(rows: impl IntoIterator<Item = DayRow>) -> Vec<(u8, Vec<DayRow>)> {
    let mut days: BTreeMap<u8, Vec<DayRow>> = BTreeMap::new();
    for row in rows {
        days.entry(row.day).or_default().push(row);
    }
    days.into_iter().collect()
}

/// The Regelwoche on a phone: a list of days, each Termin a row as tall as a finger.
#[component]
fn DayList(slots: Memo<Vec<PlanSlot>>, picked: Memo<Picked>) -> impl IntoView {
    let groups = Memo::new(move |_| slots.with(|slots| day_groups(slots.iter().map(|slot| slot.row.clone()))));
    let day = move |(day, rows): (u8, Vec<DayRow>)| {
        let rows = rows
            .into_iter()
            .map(|row| {
                let marked = row.clone();
                let current = move || picked.with(|picked| marked.is(picked)).then_some("true");
                let note = (!row.note.is_empty()).then(|| view! { <small>{row.note}</small> });
                view! {
                    <a class=row.class href=row.href data-noscroll="" aria-current=current>
                        <span class="t">{row.time}</span>
                        <span>{row.text}{note}</span>
                    </a>
                }
            })
            .collect_view();
        view! {
            <section>
                <h3 class="label">{weekday_name(day)}</h3>
                {rows}
            </section>
        }
    };
    view! {
        <div class="sp-daylist" on:click=|ev| remember(Within::Days, &ev)>
            <For each=move || groups.get() key=|group| group.clone() children=day/>
        </div>
    }
}

// ---------- the agenda ----------

/// A part of the agenda as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Block {
    /// A week: its anchor (`kw-<year>-<week>`), its head, and the days with something in them;
    /// no day at all for a week of the lecture period with nothing in it.
    Week { id: String, head: String, days: Vec<DayLine>, from: Day, to: Day },
    /// Weeks of a break with nothing in them, as one line („21.12.–03.01. vorlesungsfrei"),
    /// anchored as the first of them.
    Break { id: String, text: String, from: Day, to: Day },
}

impl Block {
    fn id(&self) -> &str {
        match self {
            Block::Week { id, .. } | Block::Break { id, .. } => id,
        }
    }

    /// The Monday of its first week and the Sunday of its last.
    fn span(&self) -> (Day, Day) {
        match self {
            Block::Week { from, to, .. } | Block::Break { from, to, .. } => (*from, *to),
        }
    }
}

/// A day of the agenda: „Mo 05.10." (today marked), the holiday's name, the dates.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DayLine {
    when: String,
    today: bool,
    holiday: Option<&'static str>,
    items: Vec<ItemLine>,
}

/// A date of the agenda: the time, what it is, where (or why it does not take place).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ItemLine {
    /// The tone, and `alt`, `cancelled` or `exam`.
    class: String,
    /// „09:15–10:45", „ganztägig", „Zeit offen", „bis 19.02." (an exam's window or deadline).
    time: String,
    /// „Übung · Entwicklung von Softwaresystemen", „Prüfung · …"
    text: String,
    /// The rooms, a room note as QIS writes it, „fällt aus · <reason>", „2. Termin"; before them
    /// „1 von 4" for an option of a choice not yet made.
    small: String,
    title: String,
    href: String,
    /// An exam the plan's exam warnings name.
    warn: bool,
}

/// The anchor of a week: `kw-2026-41`. The year belongs to it: rows can run for more than a year,
/// and then a week number comes twice (`AgendaWeek::iso_week`).
fn week_id((year, week): (i32, u8)) -> String {
    format!("kw-{year}-{week}")
}

/// „KW 41 · 05.–11.10.2026", „KW 44 · 26.10.–01.11.2026", „KW 53 · 28.12.2026–03.01.2027"
fn week_head(monday: Day, week: u8) -> String {
    let sunday = monday.plus(6);
    let ((y1, m1, d1), (y2, m2, _)) = (monday.ymd(), sunday.ymd());
    let span = if (y1, m1) == (y2, m2) {
        format!("{d1:02}.–{}", sunday.german())
    } else if y1 == y2 {
        format!("{}–{}", monday.short(), sunday.german())
    } else {
        format!("{}–{}", monday.german(), sunday.german())
    };
    format!("KW {week} · {span}")
}

/// The agenda as the page shows it: a block per week, and the weeks of a break that hold nothing
/// (not even a single date; a holiday alone does not count) as one line.
fn agenda_blocks(table: &Timetable, weeks: &[AgendaWeek], base: &StudyplanUrl, today: Option<Day>) -> Vec<Block> {
    let mut blocks = Vec::new();
    let ab = has_ab(table);
    // The quiet weeks of a break not yet written: the first one's anchor and Monday, the last
    // one's Monday.
    let mut quiet: Option<(String, Day, Day)> = None;
    let flush = |quiet: &mut Option<(String, Day, Day)>, blocks: &mut Vec<Block>| {
        if let Some((id, from, last)) = quiet.take() {
            let to = last.plus(6);
            blocks.push(Block::Break { id, text: format!("{}–{} vorlesungsfrei", from.short(), to.short()), from, to });
        }
    };
    for week in weeks {
        if week.break_week && week.days.iter().all(|day| day.items.is_empty()) {
            match &mut quiet {
                Some((_, _, last)) if last.plus(7) == week.monday => *last = week.monday,
                _ => {
                    flush(&mut quiet, &mut blocks);
                    quiet = Some((week_id(week.iso_week), week.monday, week.monday));
                }
            }
            continue;
        }
        flush(&mut quiet, &mut blocks);
        // An open choice is one line in the week, on the first day it could be attended, instead
        // of a line per option and day (owner review 2026-09-25).
        let choices = open_choices(table, week);
        // Per choice, the day its line stands on.
        let mut said: BTreeMap<usize, Day> = BTreeMap::new();
        let mut days = Vec::new();
        for day in &week.days {
            let is_today = today == Some(day.day);
            let mut items = Vec::new();
            for item in &day.items {
                let Some(e) = open_option(table, item) else {
                    items.extend(agenda_item(table, base, day.day, item));
                    continue;
                };
                let dates = choices.get(&e).map_or(&[][..], Vec::as_slice);
                // A cancelled date is no day to attend, unless the week has no other.
                if (item.cancelled.is_none() || dates.is_empty()) && !said.contains_key(&e) {
                    said.insert(e, day.day);
                    items.extend(choice_item(table, base, e, dates));
                } else if is_today && said.get(&e) != Some(&day.day) {
                    // Today keeps its line: the options that meet today stand as they are
                    // („1 von 4 · LG 10/214"), below the week's line on an earlier day.
                    items.extend(agenda_item(table, base, day.day, item));
                }
            }
            if items.is_empty() && day.holiday.is_none() {
                continue;
            }
            days.push(DayLine {
                when: format!("{} {}", day_short(day.day.weekday()), day.day.short()),
                today: today == Some(day.day),
                holiday: day.holiday,
                items,
            });
        }
        let mut head = week_head(week.monday, week.iso_week.1);
        if week.break_week {
            head.push_str(" · vorlesungsfrei");
        }
        match table.facts.ab_week(week.monday).filter(|_| ab) {
            Some(Weeks::A) => head.push_str(" · A-Woche"),
            Some(Weeks::B) => head.push_str(" · B-Woche"),
            _ => {}
        }
        blocks.push(Block::Week { id: week_id(week.iso_week), head, days, from: week.monday, to: week.monday.plus(6) });
    }
    flush(&mut quiet, &mut blocks);
    blocks
}

fn agenda_item(table: &Timetable, base: &StudyplanUrl, day: Day, item: &AgendaItem) -> Option<ItemLine> {
    match (item.event, item.exam) {
        (Some(e), _) => teaching_item(table, base, day, item, e),
        (None, Some(x)) => exam_item(table, base, day, item, x),
        (None, None) => None,
    }
}

/// The event of `item` when the item is a date of an option of a choice not yet made („1 von 4").
fn open_option(table: &Timetable, item: &AgendaItem) -> Option<usize> {
    let e = item.event?;
    let event = table.events.get(e)?;
    let row = event.rows.get(item.row)?;
    (event.unresolved() && row.option.is_some()).then_some(e)
}

/// A week's open choices: per event, its options' dates in the week that take place, in order.
fn open_choices<'a>(table: &Timetable, week: &'a AgendaWeek) -> BTreeMap<usize, Vec<(Day, &'a AgendaItem)>> {
    let mut choices: BTreeMap<usize, Vec<(Day, &AgendaItem)>> = BTreeMap::new();
    for day in &week.days {
        for item in &day.items {
            if let Some(e) = open_option(table, item) {
                let dates = choices.entry(e).or_default();
                if item.cancelled.is_none() {
                    dates.push((day.day, item));
                }
            }
        }
    }
    choices
}

/// The one line of a week for a choice not yet made: „Praktikum · Programmierpraktikum" and
/// „1 von 4 (Mo, Mi, Do, Fr)", the days its options take place on in the week; the time where all
/// of them share one. It opens the module at the first of them, where „Nur diesen" makes the
/// choice. `dates` are the week's dates of its options (`open_choices`); none when all of them
/// are cancelled.
fn choice_item(table: &Timetable, base: &StudyplanUrl, e: usize, dates: &[(Day, &AgendaItem)]) -> Option<ItemLine> {
    let event = table.events.get(e)?;
    let module = event.modules.first()?;
    let options = event.visible_options().len();
    let mut days: Vec<&str> = Vec::new();
    for (day, _) in dates {
        let short = day_short(day.weekday());
        if !days.contains(&short) {
            days.push(short);
        }
    }
    let times: BTreeSet<(Option<u16>, Option<u16>)> = dates.iter().map(|(_, item)| (item.from, item.to)).collect();
    let time = match times.into_iter().collect::<Vec<_>>().as_slice() {
        [(Some(from), Some(to))] => format!("{}–{}", clock(*from), clock(*to)),
        _ => String::new(),
    };
    let small = match days.is_empty() {
        true => format!("1 von {options} · fällt aus"),
        false => format!("1 von {options} ({})", days.join(", ")),
    };
    let text = event_text(event);
    let whens = dates.iter().map(|(day, item)| match (item.from, item.to) {
        (Some(from), Some(to)) => format!("{} {}–{}", day_short(day.weekday()), clock(from), clock(to)),
        _ => day_short(day.weekday()).to_string(),
    });
    let title: Vec<String> = [text.clone(), small.clone()].into_iter().chain(whens).collect();
    let first = dates.first().and_then(|(_, item)| event.rows.get(item.row)).or_else(|| event.rows.iter().find(|row| row.option.is_some()));
    Some(ItemLine {
        class: classes(hue(event.tone), &[(true, "alt")]),
        time,
        text,
        small,
        title: title.join(" · "),
        href: base.with_open(Some(module.as_str()), first.and_then(|row| row.key)).path(),
        warn: false,
    })
}

fn teaching_item(table: &Timetable, base: &StudyplanUrl, day: Day, item: &AgendaItem, e: usize) -> Option<ItemLine> {
    let event = table.events.get(e)?;
    let first = event.rows.get(item.row)?;
    let module = event.modules.first()?;
    let rows: Vec<&Row> = item.rows.iter().filter_map(|r| event.rows.get(*r)).collect();
    let alt = event.unresolved() && first.option.is_some();
    let time = match (item.from, item.to) {
        (Some(from), Some(to)) => format!("{}–{}", clock(from), clock(to)),
        _ if rows.iter().any(|row| row.occ.all_day) => "ganztägig".to_string(),
        _ => "Zeit offen".to_string(),
    };
    let text = event_text(event);
    let rooms = rooms(rows.iter().map(|row| &row.date));

    let mut small = Vec::new();
    if alt {
        small.push(format!("1 von {}", event.visible_options().len()));
    }
    // A room note says where the date is instead of the row's room, so it takes the room's place.
    let said = match (&item.cancelled, &item.note) {
        (Some(reason), _) if reason.is_empty() => Some("fällt aus".to_string()),
        (Some(reason), _) => Some(format!("fällt aus · {reason}")),
        (None, Some(note)) => Some(note.clone()),
        (None, None) => None,
    };
    small.extend(said.clone().or_else(|| rooms.clone()));

    let mut title = vec![text.clone(), format!("{} {time}", day.german())];
    title.extend(rooms);
    title.extend(said);
    Some(ItemLine {
        class: classes(hue(event.tone), &[(alt, "alt"), (item.cancelled.is_some(), "cancelled")]),
        time,
        text,
        small: small.join(" · "),
        title: title.join(" · "),
        href: base.with_open(Some(module.as_str()), first.key).path(),
        warn: false,
    })
}

fn exam_item(table: &Timetable, base: &StudyplanUrl, day: Day, item: &AgendaItem, x: usize) -> Option<ItemLine> {
    let exam = table.exams.get(x)?;
    let first = exam.rows.get(item.row)?;
    let module = exam.modules.first()?;
    let time = match first.shape {
        ExamShape::Sitting { from, to, .. } => format!("{}–{}", clock(from), clock(to)),
        ExamShape::Deadline { .. } => format!("bis {}", first.date.end_time.as_deref().unwrap_or("24:00")),
        ExamShape::Window { last, .. } => format!("bis {}", last.short()),
        ExamShape::DayOnly { .. } => "Zeit offen".to_string(),
        // The agenda has no day for it: it stands under „Ohne Datum" (`open_exams`).
        ExamShape::Open => return None,
    };
    let text = kind_and_title(EXAM, &exam.title);
    let rooms = rooms(item.rows.iter().filter_map(|r| exam.rows.get(*r)).map(|row| &row.date));
    let small: Vec<String> = (first.rank == 2).then(|| "2. Termin".to_string()).into_iter().chain(rooms).collect();
    // A warning names a module's Termin by its day and start; this sitting is that Termin.
    let named = |termin: &Termin| exam.modules.contains(&termin.module_id) && Some(termin.from) == item.from;
    let warn = table.exam_warnings.iter().any(|warning| warning.day == day && (named(&warning.a) || named(&warning.b)));
    let title: Vec<String> = [text.clone(), format!("{} {time}", day.german())].into_iter().chain(small.iter().cloned()).collect();
    Some(ItemLine {
        class: classes(exam_hue(table, module), &[(true, "exam")]),
        time,
        text,
        small: small.join(" · "),
        title: title.join(" · "),
        href: base.with_open(Some(module.as_str()), first.key).path(),
        warn,
    })
}

/// The week to open the agenda at: the current one, or the next one shown after a gap between
/// weeks. None before the agenda begins and after it ends; none either when that is the first
/// block, where the agenda begins anyway and the semester's head and notes stay in view.
fn scroll_target(blocks: &[Block], today: Day) -> Option<String> {
    let first = blocks.first()?;
    if today < first.span().0 {
        return None;
    }
    let target = blocks.iter().find(|block| block.span().1 >= today)?;
    (target != first).then(|| target.id().to_string())
}

fn block_view(block: Block) -> impl IntoView {
    match block {
        Block::Break { id, text, .. } => view! { <p class="agenda-break" id=id>{text}</p> }.into_any(),
        Block::Week { id, head, days, .. } => {
            let empty = days.is_empty().then(|| view! { <small>" · keine Termine"</small> });
            let within = Within::Week(id.clone());
            view! {
                <section class="agenda-week" id=id on:click=move |ev| remember(within.clone(), &ev)>
                    <h3>{head}{empty}</h3>
                    {days.into_iter().map(day_view).collect_view()}
                </section>
            }
            .into_any()
        }
    }
}

fn day_view(day: DayLine) -> impl IntoView {
    view! {
        <div class="agenda-day" class:today=day.today>
            <span class="when">{day.when}</span>
            <div>
                {day.holiday.map(|name| view! { <span class="holiday">{name}</span> })}
                {day.items.into_iter().map(item_view).collect_view()}
            </div>
        </div>
    }
}

fn item_view(item: ItemLine) -> impl IntoView {
    let small = (!item.small.is_empty()).then(|| view! { <small>{item.small}</small> });
    view! {
        <a class=format!("agenda-item {}", item.class) href=item.href data-noscroll="" title=item.title>
            <i></i>
            <span class="t">{item.time}</span>
            {item.warn.then(|| view! { <Icon name="triangle-alert"/> })}
            <span>{item.text}</span>
            {small}
        </a>
    }
}

// ---------- what has no fixed time ----------

/// A Termin without a fixed time or date, or an event without any.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LooseLine {
    hue: &'static str,
    /// „Praktikum · Programmierpraktikum"
    text: String,
    /// What QIS says of it: „Mo · nach Vereinbarung · 12.10.–25.01. · HG 0.20", „ohne Termine".
    small: String,
    href: String,
}

/// The lines of `entries` (`(event, row)`, `None` for an event without dates), in their order,
/// which is the plan's: by module.
fn loose_lines(table: &Timetable, base: &StudyplanUrl, entries: &[(usize, Option<usize>)]) -> Vec<LooseLine> {
    entries
        .iter()
        .filter_map(|(e, r)| {
            let event = table.events.get(*e)?;
            let module = event.modules.first()?;
            let row = r.and_then(|r| event.rows.get(r));
            Some(LooseLine {
                hue: hue(event.tone),
                text: event_text(event),
                small: row.map_or_else(|| "ohne Termine".to_string(), |row| row_facts(&row.date)),
                href: base.with_open(Some(module.as_str()), row.and_then(|row| row.key)).path(),
            })
        })
        .collect()
}

/// What the agenda cannot place on a day: the shown events without any date, and their shown rows
/// with neither a held nor a cancelled date (a rhythm like „nach Absprache", a pattern without a
/// lecture period, a weekly row the break takes whole).
fn undated(table: &Timetable) -> Vec<(usize, Option<usize>)> {
    let mut entries = Vec::new();
    for (e, event) in table.events.iter().enumerate().filter(|(_, event)| event.hidden.is_none()) {
        if event.rows.is_empty() {
            entries.push((e, None));
        }
        for (r, row) in event.rows.iter().enumerate().filter(|(_, row)| row.hidden.is_none()) {
            if row.occ.days.is_empty() && row.occ.cancelled.is_empty() {
                entries.push((e, Some(r)));
            }
        }
    }
    entries
}

/// The shown exam Termine without a date (QIS lists the exam, its date is open), in the plan's
/// order: the agenda has no day for them. „Prüfungen" names them as well.
fn open_exams(table: &Timetable, base: &StudyplanUrl) -> Vec<LooseLine> {
    let mut lines = Vec::new();
    for exam in table.exams.iter().filter(|exam| exam.hidden.is_none()) {
        let Some(module) = exam.modules.first() else { continue };
        for row in exam.rows.iter().filter(|row| row.hidden.is_none() && matches!(row.shape, ExamShape::Open)) {
            let small: Vec<String> = std::iter::once("Termin offen".to_string()).chain(rooms(std::iter::once(&row.date))).collect();
            let line = LooseLine {
                hue: exam_hue(table, module),
                text: kind_and_title(EXAM, &exam.title),
                small: small.join(" · "),
                href: base.with_open(Some(module.as_str()), row.key).path(),
            };
            // Two open dates of one exam that say the same are one line (and one key of the list).
            if !lines.contains(&line) {
                lines.push(line);
            }
        }
    }
    lines
}

/// A row as QIS gives it, in what it has: weekday and time, rhythm, dates, room.
fn row_facts(date: &EventDate) -> String {
    let said = date.rhythm_raw.clone().filter(|raw| !raw.trim().is_empty());
    let rhythm = match &date.rhythm {
        // Radix's `other` is whatever QIS wrote („nach Absprache" when it wrote nothing else).
        Some(code) if code.is(Rhythm::Other) => said.or_else(|| Some(code.label().to_string())),
        Some(code) => Some(code.label().to_string()),
        None => said,
    };
    let first = date.first_date.as_deref().and_then(Day::parse);
    let last = date.last_date.as_deref().and_then(Day::parse);
    let dates = match (first, last) {
        (Some(first), Some(last)) if first == last => Some(first.short()),
        (Some(first), Some(last)) => Some(format!("{}–{}", first.short(), last.short())),
        (Some(first), None) => Some(format!("ab {}", first.short())),
        (None, _) => None,
    };
    let parts: Vec<String> = format::time_slot(date.weekday, date.start_time.as_deref(), date.end_time.as_deref())
        .into_iter()
        .chain(rhythm)
        .chain(dates)
        .chain(date.room.clone().filter(|room| !room.trim().is_empty()))
        .collect();
    if parts.is_empty() {
        return "nicht angegeben".to_string();
    }
    parts.join(" · ")
}

/// „Ohne feste Zeit" (or „Ohne Datum") and its lines; nothing when there are none.
#[component]
fn Loose(title: &'static str, lines: Memo<Vec<LooseLine>>) -> impl IntoView {
    let some = Memo::new(move |_| lines.with(|lines| !lines.is_empty()));
    // What QIS says stands in the line's text, where it wraps with it: a line has no time for
    // the agenda's column of times to align it with.
    let line = |line: LooseLine| {
        view! {
            <a class=format!("agenda-item {}", line.hue) href=line.href data-noscroll="">
                <i></i>
                <span>{line.text}" "<small>{line.small}</small></span>
            </a>
        }
    };
    move || {
        some.get().then(|| {
            view! {
                <h3 class="label">{title}</h3>
                <div class="sp-loose" on:click=|ev| remember(Within::Loose, &ev)>
                    <For each=move || lines.get() key=|line| line.clone() children=line/>
                </div>
            }
        })
    }
}

// ---------- words ----------

/// What an exam is called in the plan's lines.
const EXAM: &str = "Prüfung";

/// An exam takes the tone of its first planned module, as that module's events do.
fn exam_hue(table: &Timetable, module: &str) -> &'static str {
    hue(tone_at(table.modules.iter().position(|planned| planned == module).unwrap_or_default()))
}

/// The tone first, then the names of the flags that are on.
fn classes(hue: &'static str, flags: &[(bool, &'static str)]) -> String {
    std::iter::once(hue).chain(flags.iter().filter(|(on, _)| *on).map(|(_, name)| *name)).collect::<Vec<_>>().join(" ")
}

/// „Übung · Entwicklung von Softwaresystemen": what an event is, then its title.
fn event_text(event: &Event) -> String {
    kind_and_title(&type_text(event), &event.title)
}

/// A slot's label: the kinds in their few letters, then the short name of the event's module
/// (`views::short_title`; owner review 2026-09-25): „VL Entwicklung von Softwaresystemen",
/// „Prak Programmierpraktikum", „Ü Mathematik IT-1". The event's own title where the plan's
/// data has no title of its module.
fn slot_label(event: &Event, titles: &BTreeMap<String, String>) -> String {
    let title = event.modules.first().and_then(|module| titles.get(module)).map_or(event.title.as_str(), String::as_str);
    let name = short_title(title);
    match kind_short(event) {
        short if short.is_empty() => name,
        short => format!("{short} {name}"),
    }
}

/// The rooms of rows, each once, as a student reads them at a glance: „HG/0.20 / HG/0.19" (the
/// short forms of schema 9, QIS's names where a row has none).
fn rooms<'a>(dates: impl Iterator<Item = &'a EventDate>) -> Option<String> {
    let mut rooms: Vec<&str> = Vec::new();
    for room in dates.filter_map(EventDate::room_shown) {
        if !rooms.contains(&room) {
            rooms.push(room);
        }
    }
    (!rooms.is_empty()).then(|| rooms.join(" / "))
}

/// The rooms of rows, each once, as QIS writes them („Hauptgebäude - HG 0.20 - Zentralcampus"):
/// for a tooltip, which has the room for them.
fn rooms_long<'a>(dates: impl Iterator<Item = &'a EventDate>) -> Option<String> {
    let mut rooms: Vec<&str> = Vec::new();
    for room in dates.filter_map(|date| date.room.as_deref()).map(str::trim).filter(|room| !room.is_empty()) {
        if !rooms.contains(&room) {
            rooms.push(room);
        }
    }
    (!rooms.is_empty()).then(|| rooms.join(" / "))
}

/// „Montag" for 1.
fn weekday_name(day: u8) -> &'static str {
    WEEKDAYS.get(usize::from(day).wrapping_sub(1)).copied().unwrap_or_default()
}

/// „Mo" for 1.
fn day_short(day: u8) -> &'static str {
    weekday_name(day).get(..2).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use catalog::labels::Code;
    use catalog::rows_detail::DateRow;
    use catalog::timetable::day::holidays;
    use catalog::timetable::model::Input;
    use catalog::timetable::select::Selection;
    use catalog::timetable::semester::SemesterKey;
    use catalog::timetable::views::AgendaDay;

    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn key() -> SemesterKey {
        SemesterKey::parse("2026W").unwrap()
    }

    /// 2026W as the snapshot derives it: lectures 05.10.2026–31.01.2027, a break over the turn
    /// of the year, A weeks from the first.
    fn winter() -> SemesterFacts {
        let bounds = key().bounds();
        let holidays = [2026, 2027].into_iter().flat_map(holidays).filter(|(day, _)| (bounds.0..=bounds.1).contains(day)).collect();
        SemesterFacts {
            key: key(),
            bounds,
            lecture: Some((d("2026-10-05"), d("2027-01-31"))),
            breaks: vec![(d("2026-12-21"), d("2027-01-03"))],
            a_week: Some(d("2026-10-05")),
            holidays,
        }
    }

    /// A dated row of module 12104 in 2026W: event, ord, QIS's type, rhythm, weekday, times, dates.
    fn row(event: &str, ord: i64, kind: &str, rhythm: &str, weekday: i64, time: (&str, &str), dates: (&str, &str)) -> DateRow {
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
                event_type: Some(kind.into()),
                group_name: None,
                weekday: Some(weekday),
                start_time: Some(time.0.into()),
                end_time: Some(time.1.into()),
                rhythm: Some(Code::parse(rhythm)),
                rhythm_raw: None,
                first_date: Some(dates.0.into()),
                last_date: Some(dates.1.into()),
                room: Some("HG 0.20".into()),
                campus: None,
                instructor: None,
                comment: None,
                source_url: None,
                room_short: None,
            },
        }
    }

    fn table(schedule: &[DateRow]) -> Timetable {
        planned(&["12104"], schedule, &[])
    }

    /// The title of the module the rows belong to.
    fn titles() -> BTreeMap<String, String> {
        BTreeMap::from([("12104".to_string(), "Entwicklung von Softwaresystemen".to_string())])
    }

    /// The timetable of `modules` planned in this order, with teaching and exam rows.
    fn planned(modules: &[&str], schedule: &[DateRow], exams: &[DateRow]) -> Timetable {
        let facts = winter();
        let modules: Vec<String> = modules.iter().map(|id| id.to_string()).collect();
        let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule, exams, sws: &[] };
        Timetable::build(&input, &Selection::default())
    }

    /// An exam row: module, event, ord, title, its dates (none: the date is open), its times
    /// (empty: none), its room.
    fn exam(module: &str, event: &str, ord: i64, title: &str, dates: Option<(&str, &str)>, time: (&str, &str), room: Option<&str>) -> DateRow {
        let time_of = |t: &str| Some(t.to_string()).filter(|t| !t.is_empty());
        let mut exam = row(event, ord, "Prüfung", "single", 1, ("", ""), ("", ""));
        exam.module_id = module.into();
        exam.date.event_title = title.into();
        exam.date.event_type = None;
        (exam.date.weekday, exam.date.rhythm) = (None, None);
        (exam.date.start_time, exam.date.end_time) = (time_of(time.0), time_of(time.1));
        exam.date.first_date = dates.map(|(first, _)| first.into());
        exam.date.last_date = dates.map(|(_, last)| last.into());
        exam.date.room = room.map(str::to_string);
        exam
    }

    /// The items of the agenda with their day („Do 11.02.").
    fn agenda_items(blocks: &[Block]) -> Vec<(String, &ItemLine)> {
        blocks
            .iter()
            .filter_map(|block| match block {
                Block::Week { days, .. } => Some(days),
                Block::Break { .. } => None,
            })
            .flatten()
            .flat_map(|day| day.items.iter().map(move |item| (day.when.clone(), item)))
            .collect()
    }

    fn plain() -> StudyplanUrl {
        StudyplanUrl::parse("sem=2026W")
    }

    #[test]
    fn a_slot_says_its_time_and_how_often() {
        let facts = winter();
        let small = |label: WeekLabel, every: Option<Every>, alt| slot_small(&label, every, alt, 690, &facts);
        assert_eq!(small(WeekLabel::Every(Every::Week), Some(Every::Week), None), "11:30");
        assert_eq!(small(WeekLabel::Every(Every::AWeek), Some(Every::AWeek), None), "11:30 · A-Woche");
        assert_eq!(small(WeekLabel::Every(Every::BWeek), Some(Every::BWeek), None), "11:30 · B-Woche");
        assert_eq!(small(WeekLabel::Every(Every::FourWeeks), Some(Every::FourWeeks), None), "11:30 · 4-wöch.");
        // A part of the lecture period: which end it leaves out, with its weeks.
        let until = WeekLabel::Partial { first: d("2026-10-06"), last: d("2026-11-24") };
        assert_eq!(small(until.clone(), Some(Every::Week), None), "11:30 · bis 24.11.");
        assert_eq!(small(until, Some(Every::AWeek), None), "11:30 · A-Woche · bis 24.11.");
        assert_eq!(small(WeekLabel::Partial { first: d("2026-12-07"), last: d("2027-01-25") }, Some(Every::Week), None), "11:30 · ab 07.12.");
        assert_eq!(small(WeekLabel::Partial { first: d("2026-10-26"), last: d("2026-11-23") }, Some(Every::Week), None), "11:30 · 26.10.–23.11.");
        // Single dates are counted, a lone one named.
        assert_eq!(small(WeekLabel::Once { dates: 3, first: d("2026-11-04") }, None, None), "3 Termine");
        assert_eq!(small(WeekLabel::Once { dates: 1, first: d("2027-02-23") }, None, None), "1 Termin · 23.02.");
        // An option of a choice not made says so right after its time.
        assert_eq!(small(WeekLabel::Every(Every::Week), Some(Every::Week), Some((4, 1))), "11:30 · 1 von 4");
    }

    #[test]
    fn the_regelwoche_links_each_slot_to_its_termin() {
        let schedule = [
            row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), ("2026-10-06", "2027-01-26")),
            row("148134", 1, "Übung", "week_a", 2, ("07:30", "09:00"), ("2026-10-06", "2026-11-17")),
            row("148019", 1, "Übung", "single", 2, ("11:45", "13:15"), ("2027-02-23", "2027-02-23")),
        ];
        let table = table(&schedule);
        let slots = week_slots(&table, &plain(), &titles(), Weeks::All);
        let shown: Vec<(&str, &str, &str, &str)> =
            slots.iter().map(|slot| (slot.slot.label.as_str(), slot.slot.small.as_str(), slot.slot.class, slot.row.note.as_str())).collect();
        assert_eq!(
            shown,
            [
                ("Ü Entwicklung von Softwaresystemen", "07:30 · A-Woche · bis 17.11.", "tinted", "A-Woche · bis 17.11."),
                ("VL Entwicklung von Softwaresystemen", "11:30", "tinted", ""),
                ("Ü Entwicklung von Softwaresystemen", "1 Termin · 23.02.", "once", "1 Termin · 23.02."),
            ]
        );
        // The label names the module in its short name, whatever the event is called.
        let long = BTreeMap::from([("12104".to_string(), "Elektrische und elektronische Grundlagen der Informatik".to_string())]);
        let slots = week_slots(&table, &plain(), &long, Weeks::All);
        assert_eq!(slots.get(1).map(|slot| slot.slot.label.as_str()), Some("VL Elektrische und elektronische …"));
        assert_eq!(slots.get(1).map(|slot| slot.row.text.as_str()), Some("Vorlesung · Entwicklung von Softwaresystemen"));
        // Without the module's title, the event's stands in.
        let slots = week_slots(&table, &plain(), &BTreeMap::new(), Weeks::All);
        assert_eq!(slots.get(1).map(|slot| slot.slot.label.as_str()), Some("VL Entwicklung von Softwaresystemen"));
        let lecture = slots.get(1).unwrap();
        let key = table.events.iter().find(|event| event.id == "148701").and_then(|event| event.rows.first()).and_then(|row| row.key).unwrap();
        assert_eq!(lecture.slot.href.as_deref(), Some(format!("/studyplan?sem=2026W&open=12104&row={}", key.text()).as_str()));
        assert_eq!((lecture.slot.hue, lecture.row.class.as_str()), (Some("t-ice"), "t-ice"));
        assert_eq!((lecture.row.time.as_str(), lecture.row.text.as_str()), ("11:30–13:00", "Vorlesung · Entwicklung von Softwaresystemen"));
        assert_eq!(lecture.slot.title, "Vorlesung · Entwicklung von Softwaresystemen · Di 11:30–13:00 · HG 0.20");
        // The slot is the one beside the plan when its module is open and the address names it.
        assert!(lecture.row.is(&(Some("12104".into()), Some(key))));
        assert!(!lecture.row.is(&(Some("12107".into()), Some(key))));
        assert!(!lecture.row.is(&(Some("12104".into()), None)));
        assert!(!slots.first().unwrap().row.is(&(Some("12104".into()), Some(key))));
    }

    #[test]
    fn a_phone_lists_the_slots_by_day() {
        let at = |day: u8, text: &str| DayRow {
            day,
            class: "t-ice".into(),
            time: String::new(),
            text: text.into(),
            note: String::new(),
            href: String::new(),
            modules: Vec::new(),
            keys: Vec::new(),
        };
        let groups = day_groups([at(2, "a"), at(1, "b"), at(2, "c"), at(5, "d")]);
        let shown: Vec<(u8, Vec<&str>)> = groups.iter().map(|(day, rows)| (*day, rows.iter().map(|row| row.text.as_str()).collect())).collect();
        assert_eq!(shown, [(1, vec!["b"]), (2, vec!["a", "c"]), (5, vec!["d"])]);
        assert_eq!((weekday_name(1), weekday_name(7), weekday_name(0), weekday_name(8)), ("Montag", "Sonntag", "", ""));
        assert_eq!((day_short(3), day_short(9)), ("Mi", ""));
    }

    fn week(monday: &str, break_week: bool, days: Vec<AgendaDay>) -> AgendaWeek {
        let monday = d(monday);
        AgendaWeek { monday, iso_week: monday.iso_week(), break_week, days }
    }

    fn holiday(day: &str, name: &'static str) -> AgendaDay {
        AgendaDay { day: d(day), holiday: Some(name), items: Vec::new() }
    }

    #[test]
    fn the_agenda_is_anchored_by_week_and_folds_a_quiet_break() {
        assert_eq!(week_id((2026, 41)), "kw-2026-41");
        assert_eq!(week_id((2027, 1)), "kw-2027-1");
        assert_eq!(week_head(d("2026-10-05"), 41), "KW 41 · 05.–11.10.2026");
        assert_eq!(week_head(d("2026-10-26"), 44), "KW 44 · 26.10.–01.11.2026");
        assert_eq!(week_head(d("2026-12-28"), 53), "KW 53 · 28.12.2026–03.01.2027");

        let table = table(&[]);
        let weeks = [
            week("2026-12-14", false, Vec::new()),
            week("2026-12-21", true, vec![holiday("2026-12-25", "1. Weihnachtstag"), holiday("2026-12-26", "2. Weihnachtstag")]),
            week("2026-12-28", true, vec![holiday("2027-01-01", "Neujahr")]),
            week("2027-01-04", false, Vec::new()),
        ];
        let blocks = agenda_blocks(&table, &weeks, &plain(), Some(d("2026-12-15")));
        let shown: Vec<(&str, String)> = blocks
            .iter()
            .map(|block| match block {
                Block::Week { id, head, days, .. } => (id.as_str(), format!("{head} ({} Tage)", days.len())),
                Block::Break { id, text, .. } => (id.as_str(), text.clone()),
            })
            .collect();
        assert_eq!(
            shown,
            [
                ("kw-2026-51", "KW 51 · 14.–20.12.2026 (0 Tage)".to_string()),
                ("kw-2026-52", "21.12.–03.01. vorlesungsfrei".to_string()),
                ("kw-2027-1", "KW 1 · 04.–10.01.2027 (0 Tage)".to_string()),
            ]
        );

        // A break week that holds a date stays a week of its own and says it is one of the break;
        // the quiet one after it is a line.
        let mut busy = [week("2026-12-21", true, vec![holiday("2026-12-25", "1. Weihnachtstag")]), week("2026-12-28", true, Vec::new())];
        if let Some(day) = busy.get_mut(0).and_then(|week| week.days.get_mut(0)) {
            day.items.push(AgendaItem { event: None, exam: None, row: 0, rows: Vec::new(), from: None, to: None, cancelled: None, note: None });
        }
        let blocks = agenda_blocks(&table, &busy, &plain(), None);
        assert!(matches!(blocks.first(), Some(Block::Week { head, .. }) if head == "KW 52 · 21.–27.12.2026 · vorlesungsfrei"), "{blocks:?}");
        assert!(matches!(blocks.get(1), Some(Block::Break { text, id, .. }) if text == "28.12.–03.01. vorlesungsfrei" && id == "kw-2026-53"), "{blocks:?}");
    }

    #[test]
    fn the_agenda_opens_at_the_current_week() {
        let table = table(&[]);
        let weeks: Vec<AgendaWeek> = ["2026-10-05", "2026-10-12", "2026-10-26"].into_iter().map(|monday| week(monday, false, Vec::new())).collect();
        let blocks = agenda_blocks(&table, &weeks, &plain(), None);
        // Before the semester, and in its first week, the agenda stays at its start.
        assert_eq!(scroll_target(&blocks, d("2026-09-24")), None);
        assert_eq!(scroll_target(&blocks, d("2026-10-07")), None);
        // In a later week, that week; in a gap between the weeks shown, the next one.
        assert_eq!(scroll_target(&blocks, d("2026-10-18")).as_deref(), Some("kw-2026-42"));
        assert_eq!(scroll_target(&blocks, d("2026-10-20")).as_deref(), Some("kw-2026-44"));
        // After its last week nothing is current.
        assert_eq!(scroll_target(&blocks, d("2026-11-02")), None);
        assert_eq!(scroll_target(&[], d("2026-10-20")), None);
    }

    #[test]
    fn the_agenda_says_what_each_date_is() {
        let mut lecture = row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), ("2026-10-06", "2027-01-26"));
        lecture.cancelled_dates = Some("13.10.2026: Projektwoche 19.01.2027: Raumwechsel".into());
        let table = table(&[lecture]);
        let blocks = agenda_blocks(&table, &table.agenda(), &plain(), Some(d("2026-10-06")));
        let items = agenda_items(&blocks);
        let (when, first) = items.first().unwrap();
        assert_eq!((when.as_str(), first.time.as_str(), first.text.as_str()), ("Di 06.10.", "11:30–13:00", "Vorlesung · Entwicklung von Softwaresystemen"));
        assert_eq!((first.class.as_str(), first.small.as_str()), ("t-ice", "HG 0.20"));
        assert!(first.href.starts_with("/studyplan?sem=2026W&open=12104&row=148701-"), "{}", first.href);
        let (_, gone) = items.iter().find(|(when, _)| when == "Di 13.10.").unwrap();
        assert_eq!((gone.class.as_str(), gone.small.as_str()), ("t-ice cancelled", "fällt aus · Projektwoche"));
        let (_, moved) = items.iter().find(|(when, _)| when == "Di 19.01.").unwrap();
        assert_eq!((moved.small.as_str(), moved.title.as_str()), ("Raumwechsel", "Vorlesung · Entwicklung von Softwaresystemen · 19.01.2027 11:30–13:00 · HG 0.20 · Raumwechsel"));
        // Today is marked, the break is one line, a holiday names itself.
        assert!(blocks.iter().any(|block| matches!(block, Block::Week { days, .. } if days.iter().any(|day| day.today && day.when == "Di 06.10."))));
        assert!(blocks.iter().any(|block| matches!(block, Block::Break { text, .. } if text == "21.12.–03.01. vorlesungsfrei")));
        assert!(blocks.iter().any(|block| matches!(block, Block::Week { days, .. } if days.iter().any(|day| day.holiday == Some("Reformationstag") && day.when == "Sa 31.10."))));
    }

    #[test]
    fn the_agenda_says_when_each_exam_is_and_which_collide() {
        let exams = [
            // 12104: a first sitting that overlaps 12107's, a second one a month later, a
            // deadline; 12107: that sitting, a window, a day without a time, an open date.
            exam("12104", "90", 1, "Entwicklung von Softwaresystemen", Some(("2027-02-11", "2027-02-11")), ("11:00", "13:00"), Some("Audimax 1")),
            exam("12104", "90", 2, "Entwicklung von Softwaresystemen", Some(("2027-03-12", "2027-03-12")), ("11:00", "13:00"), Some("Audimax 1")),
            exam("12104", "94", 1, "Abgabe Softwareprojekt", Some(("2027-02-14", "2027-02-14")), ("", "24:00"), None),
            exam("12107", "91", 1, "Prüfung Mathematik", Some(("2027-02-11", "2027-02-11")), ("12:00", "14:00"), Some("HG 0.20")),
            exam("12107", "92", 1, "Hausarbeit", Some(("2027-02-01", "2027-02-05")), ("", ""), None),
            exam("12107", "93", 1, "Kolloquium", Some(("2027-02-15", "2027-02-15")), ("", ""), None),
            exam("12107", "95", 1, "Klausur Statistik", None, ("", ""), Some("HG 0.19")),
            exam("12107", "95", 2, "Klausur Statistik", None, ("", ""), Some("HG 0.19")),
        ];
        let table = planned(&["12104", "12107"], &[], &exams);
        let blocks = agenda_blocks(&table, &table.agenda(), &plain(), None);
        let shown: Vec<(String, &str, &str, &str, &str, bool)> = agenda_items(&blocks)
            .into_iter()
            .map(|(when, item)| (when, item.time.as_str(), item.text.as_str(), item.small.as_str(), item.class.as_str(), item.warn))
            .collect();
        assert_eq!(
            shown,
            [
                // A window stands on its first day only.
                ("Mo 01.02.".to_string(), "bis 05.02.", "Prüfung · Hausarbeit", "", "t-sun exam", false),
                // The two sittings that overlap both carry the warning; a title that says it is
                // an exam stands alone.
                ("Do 11.02.".to_string(), "11:00–13:00", "Prüfung · Entwicklung von Softwaresystemen", "Audimax 1", "t-ice exam", true),
                ("Do 11.02.".to_string(), "12:00–14:00", "Prüfung Mathematik", "HG 0.20", "t-sun exam", true),
                ("So 14.02.".to_string(), "bis 24:00", "Prüfung · Abgabe Softwareprojekt", "", "t-ice exam", false),
                ("Mo 15.02.".to_string(), "Zeit offen", "Prüfung · Kolloquium", "", "t-sun exam", false),
                // The later sitting of a module says so first, and on its own day it warns of nothing.
                ("Fr 12.03.".to_string(), "11:00–13:00", "Prüfung · Entwicklung von Softwaresystemen", "2. Termin · Audimax 1", "t-ice exam", false),
            ]
        );
        let (_, first) = agenda_items(&blocks).into_iter().find(|(when, _)| when == "Do 11.02.").unwrap();
        assert!(first.href.starts_with("/studyplan?sem=2026W&open=12104&row=90-"), "{}", first.href);

        // The open date has no day: it stands under „Ohne Datum", in its module's tone, once
        // however often QIS lists it.
        let open = open_exams(&table, &plain());
        let lines: Vec<(&str, &str, &str)> = open.iter().map(|line| (line.hue, line.text.as_str(), line.small.as_str())).collect();
        assert_eq!(lines, [("t-sun", "Prüfung · Klausur Statistik", "Termin offen · HG 0.19")]);
        assert!(open.iter().all(|line| line.href.starts_with("/studyplan?sem=2026W&open=12107")), "{open:?}");
    }

    /// `views::kind_and_title` has its own test; here a slot and a line of real events.
    #[test]
    fn a_title_that_says_what_it_is_stands_alone() {
        let mut tutorial = row("150001", 1, "Tutorium", "weekly", 3, ("09:15", "10:45"), ("2026-10-07", "2027-01-27"));
        tutorial.date.event_title = "Tutorium Höhere Mathematik W-1".into();
        let table = table(&[tutorial, row("148701", 1, "Vorlesung", "weekly", 2, ("11:30", "13:00"), ("2026-10-06", "2027-01-26"))]);
        let math = BTreeMap::from([("12104".to_string(), "Höhere Mathematik W-1 (Analysis)".to_string())]);
        let labels: Vec<(String, String)> = table.events.iter().map(|event| (slot_label(event, &math), event_text(event))).collect();
        // A slot names the module in its short name after the kind's few letters.
        assert!(labels.contains(&("Tut Höhere Mathematik W-1".to_string(), "Tutorium Höhere Mathematik W-1".to_string())), "{labels:?}");
        assert!(labels.contains(&("VL Höhere Mathematik W-1".to_string(), "Vorlesung · Entwicklung von Softwaresystemen".to_string())), "{labels:?}");
    }

    #[test]
    fn an_open_choice_is_one_line_a_week() {
        // Programmierpraktikum (148370): four groups, Mo, Mi, Do 11:30 and Fr 13:45.
        let group = |ord: i64, weekday: i64, time: (&str, &str), first: &str| {
            let mut row = row("148370", ord, "Praktikum", "weekly", weekday, time, (first, "2027-01-29"));
            row.date.event_title = "Programmierpraktikum".into();
            row.date.group_name = Some(format!("{ord}-Gruppe"));
            row
        };
        let mut monday = group(1, 1, ("11:30", "13:00"), "2026-10-05");
        monday.cancelled_dates = Some("12.10.2026: Krankheit".into());
        let schedule = [monday, group(2, 3, ("11:30", "13:00"), "2026-10-07"), group(3, 4, ("11:30", "13:00"), "2026-10-08"), group(4, 5, ("13:45", "15:15"), "2026-10-09")];
        let facts = winter();
        let modules = vec!["12104".to_string()];
        let build = |selection: &Selection| Timetable::build(&Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule: &schedule, exams: &[], sws: &[] }, selection);

        let open = build(&Selection::default());
        let blocks = agenda_blocks(&open, &open.agenda(), &plain(), None);
        let week = |id: &str| -> Vec<(String, String, String, String)> {
            blocks
                .iter()
                .filter_map(|block| match block {
                    Block::Week { id: at, days, .. } if at == id => Some(days),
                    _ => None,
                })
                .flatten()
                .flat_map(|day| day.items.iter().map(move |item| (day.when.clone(), item.time.clone(), item.text.clone(), item.small.clone())))
                .collect()
        };
        let line = |when: &str, time: &str, small: &str| (when.to_string(), time.to_string(), "Praktikum · Programmierpraktikum".to_string(), small.to_string());
        // One line on the first day the choice could be attended, not four; the times differ.
        assert_eq!(week("kw-2026-41"), [line("Mo 05.10.", "", "1 von 4 (Mo, Mi, Do, Fr)")]);
        // A cancelled option is no day to attend: the line stands on the next.
        assert_eq!(week("kw-2026-42"), [line("Mi 14.10.", "", "1 von 4 (Mi, Do, Fr)")]);
        let (_, first) = agenda_items(&blocks).into_iter().next().unwrap();
        assert!(first.href.starts_with("/studyplan?sem=2026W&open=12104&row=148370-"), "{}", first.href);
        assert!(first.class.ends_with(" alt") && first.title.contains("Mo 11:30–13:00 · Mi 11:30–13:00"), "{first:?}");

        // Today keeps its line (review 2026-09-25): its options stand as they are, and on the day
        // of the week's line nothing comes twice.
        let on = |today: &str| {
            let blocks = agenda_blocks(&open, &open.agenda(), &plain(), Some(d(today)));
            let days: Vec<(String, bool, Vec<String>)> = blocks
                .into_iter()
                .filter_map(|block| match block {
                    Block::Week { id, days, .. } if id == "kw-2026-41" || id == "kw-2026-42" => Some(days),
                    _ => None,
                })
                .flatten()
                .map(|day| (day.when, day.today, day.items.into_iter().map(|item| format!("{} {}", item.time, item.small)).collect()))
                .collect();
            days
        };
        let day = |when: &str, today: bool, items: &[&str]| (when.to_string(), today, items.iter().map(|item| item.to_string()).collect::<Vec<_>>());
        assert_eq!(
            on("2026-10-08"),
            [
                day("Mo 05.10.", false, &[" 1 von 4 (Mo, Mi, Do, Fr)"]),
                day("Do 08.10.", true, &["11:30–13:00 1 von 4 · HG 0.20"]),
                day("Mi 14.10.", false, &[" 1 von 4 (Mi, Do, Fr)"]),
            ]
        );
        assert_eq!(on("2026-10-05").first(), Some(&day("Mo 05.10.", true, &[" 1 von 4 (Mo, Mi, Do, Fr)"])));
        // Today's option is cancelled: it says so, and the week's line stands on the next day.
        assert_eq!(
            on("2026-10-12").get(1..),
            Some(&[day("Mo 12.10.", true, &["11:30–13:00 1 von 4 · fällt aus · Krankheit"]), day("Mi 14.10.", false, &[" 1 von 4 (Mi, Do, Fr)"])][..])
        );

        // Chosen: that group's dates, one line each, as every other Termin.
        let chosen = open.events.iter().flat_map(|event| &event.rows).find(|row| row.date.weekday == Some(3)).and_then(|row| row.key).unwrap();
        let decided = build(&Selection { chosen_rows: [chosen].into(), ..Selection::default() });
        let blocks = agenda_blocks(&decided, &decided.agenda(), &plain(), None);
        let items = agenda_items(&blocks);
        assert!(items.iter().all(|(when, item)| when.starts_with("Mi") && item.time == "11:30–13:00" && !item.small.starts_with("1 von")), "{items:?}");
        assert!(items.len() > 10);
    }

    #[test]
    fn a_view_remembers_the_link_it_was_left_by() {
        let href = "/studyplan?sem=2026W&view=dates&open=12104&row=148701-b7025";
        let place = Place { within: Within::Week("kw-2026-50".into()), href: href.into() };
        assert_eq!(Place::restored(&place.stored()), Some(place.clone()));
        assert_eq!(place.selector(), format!("#kw-2026-50 a[href=\"{href}\"]"));
        assert_eq!(place.week(), Some("kw-2026-50"));
        let days = Place { within: Within::Days, href: "/studyplan?open=12104".into() };
        assert_eq!(Place::restored(&days.stored()), Some(days.clone()));
        assert_eq!((days.selector().as_str(), days.week()), (".sp-daylist a[href=\"/studyplan?open=12104\"]", None));
        assert_eq!(Place::restored("loose /studyplan?view=dates&open=12107").map(|place| place.within), Some(Within::Loose));
        // What is read back is checked: a week's anchor, a link of the plan, nothing that would
        // break out of the selector.
        for bad in ["kw-2026 /studyplan?open=1", "week /studyplan?open=1", "days /catalog?open=1", "days /studyplan?open=1\"]", "days /studyplanx", "days"] {
            assert_eq!(Place::restored(bad), None, "{bad}");
        }
        // It is where the visitor left the view only for the same semester, view and module.
        assert!(place.left(&StudyplanUrl::parse("sem=2026W&view=dates&open=12104")));
        assert!(place.left(&StudyplanUrl::parse("sem=2026W&view=dates&open=12104&row=148701-aaaaa")));
        assert!(!place.left(&StudyplanUrl::parse("sem=2026W&view=dates&open=12107")));
        assert!(!place.left(&StudyplanUrl::parse("sem=2026W&open=12104")));
        assert!(!place.left(&StudyplanUrl::parse("sem=2027S&view=dates&open=12104")));
    }

    #[test]
    fn what_has_no_fixed_time_says_what_is_known() {
        let mut arranged = row("148455", 1, "Praktikum", "other", 1, ("", ""), ("2026-10-12", "2027-01-25"));
        arranged.date.start_time = None;
        arranged.date.end_time = None;
        arranged.date.rhythm_raw = Some("nach Vereinbarung".into());
        let mut dateless = row("150000", 1, "Seminar", "single", 1, ("09:15", "10:45"), ("", ""));
        dateless.ord = None;
        dateless.date.first_date = None;
        dateless.date.last_date = None;
        let table = table(&[arranged, dateless]);
        let lines = loose_lines(&table, &plain(), &table.loose());
        let shown: Vec<(&str, &str)> = lines.iter().map(|line| (line.text.as_str(), line.small.as_str())).collect();
        assert_eq!(
            shown,
            [
                ("Praktikum · Entwicklung von Softwaresystemen", "Mo · nach Vereinbarung · 12.10.–25.01. · HG 0.20"),
                ("Seminar · Entwicklung von Softwaresystemen", "ohne Termine"),
            ]
        );
        assert_eq!(lines.last().map(|line| line.href.as_str()), Some("/studyplan?sem=2026W&open=12104"));
        // The agenda places neither, so its „Ohne Datum" has both.
        assert_eq!(undated(&table), table.loose());
        // A row that says nothing says so (R12).
        let mut nothing = row("150002", 1, "Seminar", "other", 1, ("", ""), ("", ""));
        nothing.date = EventDate { weekday: None, start_time: None, end_time: None, rhythm: None, first_date: None, last_date: None, room: None, ..nothing.date };
        assert_eq!(row_facts(&nothing.date), "nicht angegeben");
    }
}
