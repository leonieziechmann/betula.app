//! The views „Woche" and „Termine" of one semester of the Studienplan.
//!
//! „Woche" is the Regelwoche (`Timetable::regular_week`): a slot per recurring Termin at its
//! weekday and time in the week grid (`crate::week`), and the dates of an event that do not recur
//! gathered into one slot („3 Termine"). Each slot is a link to its module beside the plan,
//! pointing at that Termin (`open`, `row`). A phone has no room for five columns: there the same
//! slots are a list of days. What has no fixed time stands under „Ohne feste Zeit", once for the
//! semester.
//!
//! „Termine" is the agenda (`Timetable::agenda`): every date by week and day, what is cancelled
//! and why, the holidays, the exams, and the weeks of a break with nothing in them as one line.
//! It opens at the current week (the page reads the clock once, `PlanCtx::today`).
//!
//! Both views read the timetable and the address without what stands beside the plan (`base`),
//! two siblings of the address (R16). The slot or date being opened is marked from where the app
//! is going (`Pending`), so a click answers in the next frame (R21). What they show is built as
//! plain values first (`PlanSlot`, `Block`, `LooseLine`), which the tests read without rendering;
//! the lists are keyed by those values, so a hidden event redraws the days it was on and no
//! other (R5).

use std::collections::BTreeMap;

use catalog::labels::Rhythm;
use catalog::rows_detail::EventDate;
use catalog::timetable::day::{clock, Day};
use catalog::timetable::exams::{ExamShape, Termin};
use catalog::timetable::facts::SemesterFacts;
use catalog::timetable::kind::EventKind;
use catalog::timetable::model::{Event, Row, Timetable};
use catalog::timetable::occur::Every;
use catalog::timetable::rowkey::RowKey;
use catalog::timetable::views::{AgendaItem, AgendaWeek, Reach, WeekItem, WeekLabel};
use catalog::url::{self, StudyplanUrl};
use leptos::prelude::*;

use super::PlanCtx;
use crate::format;
use crate::pages::catalog::phone_layout;
use crate::pending::Pending;
use crate::ui::Icon;
use crate::week::{GridSlot, WeekGrid};

/// The tones of the plan's modules as the `t-…` classes of app.css, in the order of
/// `Event::tone` (1 … 8: the module's place in the plan, round again after the eighth).
const HUES: [&str; 8] = ["t-ice", "t-sun", "t-violet", "t-teal", "t-green", "t-coral", "t-rose", "t-slate"];

const WEEKDAYS: [&str; 7] = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"];

/// The module and the Termin the address puts beside the plan: what a slot or a row is marked by.
type Picked = (Option<String>, Option<RowKey>);

/// „Woche": the Regelwoche as a grid, on a phone as a list of days, and what has no fixed time.
#[component]
pub(super) fn WeekView(ctx: PlanCtx) -> impl IntoView {
    let base = base_of(ctx);
    let picked = picked_of(ctx);
    let slots = Memo::new(move |_| {
        let base = base.get();
        ctx.table.with(|table| table.as_ref().map(|table| week_slots(table, &base)).unwrap_or_default())
    });
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
    view! {
        {week}
        <Loose title="Ohne feste Zeit" lines=loose/>
    }
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
        ctx.table.with(|table| table.as_ref().map(|table| loose_lines(table, &base, &undated(table))).unwrap_or_default())
    });

    // The agenda opens at the current week, once: when it first has something to show. Later
    // changes of what is shown leave the page where the visitor put it.
    if let Some(today) = today {
        let done = StoredValue::new(false);
        Effect::new(move |_| {
            if done.get_value() {
                return;
            }
            let Some(target) = blocks.with(|blocks| (!blocks.is_empty()).then(|| scroll_target(blocks, today))) else { return };
            done.set_value(true);
            if let Some(id) = target {
                reveal(id);
            }
        });
    }

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

/// Scrolls the week `id` to the top of what scrolls around it (the page's column, on a phone the
/// window), a frame later: after the router has put a page it opened at its top.
#[allow(unused_variables)]
fn reveal(id: String) {
    #[cfg(feature = "csr")]
    request_animation_frame(move || {
        let Some(window) = web_sys::window() else { return };
        let Some(document) = window.document() else { return };
        let Some(element) = document.get_element_by_id(&id) else { return };
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Start);
        element.scroll_into_view_with_scroll_into_view_options(&options);
        // On a phone the window scrolls under the top bar, which stays: the week goes below it.
        if crate::nav::is_phone() {
            if let Some(bar) = document.query_selector(".topbar").ok().flatten() {
                window.scroll_by_with_x_and_y(0.0, -bar.get_bounding_client_rect().bottom());
            }
        }
    });
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

/// The slots of the Regelwoche, in its order (weekday, time, event).
fn week_slots(table: &Timetable, base: &StudyplanUrl) -> Vec<PlanSlot> {
    table.regular_week().iter().filter_map(|item| plan_slot(table, base, item)).collect()
}

fn plan_slot(table: &Timetable, base: &StudyplanUrl, item: &WeekItem) -> Option<PlanSlot> {
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
    let mut title = vec![type_text(event), event.title.clone(), format!("{} {}–{}", day_short(item.day), clock(item.from), clock(item.to))];
    title.extend(notes.iter().cloned());
    title.extend(rooms(rows.iter().map(|row| &row.date)));
    if matches!(item.label, WeekLabel::Once { dates, .. } if dates > 1) {
        let mut days: Vec<Day> = rows.iter().flat_map(|row| row.occ.days.iter().copied()).filter(|day| day.weekday() == item.day).collect();
        days.sort_unstable();
        days.dedup();
        title.push(days.iter().map(|day| day.german()).collect::<Vec<_>>().join(", "));
    }
    if item.clash {
        title.push("überschneidet sich".to_string());
    }

    let slot = GridSlot {
        day: item.day,
        from: item.from,
        to: item.to,
        label: format!("{} {}", kind_short(event), event.title),
        small: slot_small(&item.label, every, item.alt, item.from, &table.facts),
        title: title.join(" · "),
        class: if once { "once" } else { "tinted" },
        hue: Some(hue),
        alt: item.alt.is_some(),
        clash: item.clash,
        href: Some(href.clone()),
        current: false,
    };
    let row = DayRow {
        day: item.day,
        class: classes(hue, &[(item.alt.is_some(), "alt"), (item.clash, "clash")]),
        time: format!("{}–{}", clock(item.from), clock(item.to)),
        text: format!("{} · {}", type_text(event), event.title),
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
        <div class="sp-daylist">
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
        let days = week
            .days
            .iter()
            .map(|day| DayLine {
                when: format!("{} {}", day_short(day.day.weekday()), day.day.short()),
                today: today == Some(day.day),
                holiday: day.holiday,
                items: day.items.iter().filter_map(|item| agenda_item(table, base, day.day, item)).collect(),
            })
            .collect();
        let mut head = week_head(week.monday, week.iso_week.1);
        if week.break_week {
            head.push_str(" · vorlesungsfrei");
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
    let text = format!("{} · {}", type_text(event), event.title);
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
    // An exam takes the tone of its first planned module, as that module's events do.
    let position = table.modules.iter().position(|planned| planned == module).unwrap_or_default();
    let tone = u8::try_from(position % HUES.len()).unwrap_or_default().saturating_add(1);
    let time = match first.shape {
        ExamShape::Sitting { from, to, .. } => format!("{}–{}", clock(from), clock(to)),
        ExamShape::Deadline { .. } => format!("bis {}", first.date.end_time.as_deref().unwrap_or("24:00")),
        ExamShape::Window { last, .. } => format!("bis {}", last.short()),
        ExamShape::DayOnly { .. } => "Zeit offen".to_string(),
        ExamShape::Open => "Termin offen".to_string(),
    };
    let text = format!("Prüfung · {}", exam.title);
    let rooms = rooms(item.rows.iter().filter_map(|r| exam.rows.get(*r)).map(|row| &row.date));
    let small: Vec<String> = (first.rank == 2).then(|| "2. Termin".to_string()).into_iter().chain(rooms).collect();
    // A warning names a module's Termin by its day and start; this sitting is that Termin.
    let named = |termin: &Termin| exam.modules.contains(&termin.module_id) && Some(termin.from) == item.from;
    let warn = table.exam_warnings.iter().any(|warning| warning.day == day && (named(&warning.a) || named(&warning.b)));
    let title: Vec<String> = [text.clone(), format!("{} {time}", day.german())].into_iter().chain(small.iter().cloned()).collect();
    Some(ItemLine {
        class: classes(hue(tone), &[(true, "exam")]),
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
            view! {
                <section class="agenda-week" id=id>
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
                text: format!("{} · {}", type_text(event), event.title),
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
        return "keine Angaben".to_string();
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
                <div class="sp-loose">
                    <For each=move || lines.get() key=|line| line.clone() children=line/>
                </div>
            }
        })
    }
}

// ---------- words ----------

fn hue(tone: u8) -> &'static str {
    HUES.get(usize::from(tone.saturating_sub(1)) % HUES.len()).copied().unwrap_or("t-slate")
}

/// The tone first, then the names of the flags that are on.
fn classes(hue: &'static str, flags: &[(bool, &'static str)]) -> String {
    std::iter::once(hue).chain(flags.iter().filter(|(on, _)| *on).map(|(_, name)| *name)).collect::<Vec<_>>().join(" ")
}

/// What QIS calls an event („Übung", „Vorlesung/Übung", „Laborausbildung"), else its kinds.
fn type_text(event: &Event) -> String {
    match event.type_raw.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
        Some(text) => text.to_string(),
        None => event.kinds.iter().map(EventKind::label).collect::<Vec<_>>().join("/"),
    }
}

/// The kinds in a slot's few letters: „VL", „Ü", „VL/Ü".
fn kind_short(event: &Event) -> String {
    event.kinds.iter().map(EventKind::short).collect::<Vec<_>>().join("/")
}

/// The rooms of rows, each once, as QIS writes them: „HG 0.20 / HG 0.19".
fn rooms<'a>(dates: impl Iterator<Item = &'a EventDate>) -> Option<String> {
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
            },
        }
    }

    fn table(schedule: &[DateRow]) -> Timetable {
        let facts = winter();
        let modules = ["12104".to_string()];
        let input = Input { key: key(), semester: None, facts: &facts, modules: &modules, schedule, exams: &[], sws: &[] };
        Timetable::build(&input, &Selection::default())
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
        let slots = week_slots(&table, &plain());
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
        let items: Vec<(String, &ItemLine)> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Week { days, .. } => Some(days),
                Block::Break { .. } => None,
            })
            .flatten()
            .flat_map(|day| day.items.iter().map(move |item| (day.when.clone(), item)))
            .collect();
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
    }
}
