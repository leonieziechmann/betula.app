//! One date of a Termin in its own view (folia/design/termin, the fourth round; owner, 2026-10-26:
//! „Ja zu allen drei Vorschlägen, bau das jetzt in Folia").
//!
//! The owner, of the round before: „Beim kallender workflow wenn man auf einen Termin klickt. Ich
//! würde schätzen punkt 1 sind ein paar mehr infos. Punkt 2 ist dann die Termine zu customizen." So
//! the view says first what one wants to know of the date — the module, the kind and the title;
//! the day with arrows to the dates before and after it, where it stands („in 40 min", „läuft ·
//! noch 20 min", „Fällt aus"), the time and the room —, then who leads it and what else is said of
//! it (a clash in red, why QIS cancels it, a note of the room, QIS's remark). From the Stundenplan
//! (`TerminMode::Adjust`) „Im Stundenplan" follows: the groups of its event's choice as tiles to
//! tick, as many as one likes (none ticked: the choice stays open), the event shown or not, this
//! one date hidden — everything at once, and in the Kalender-Abo too. To look up
//! (`TerminMode::LookUp`) the groups are tiles that open their Termin instead. Then what one looks
//! up: the day of the plan around it, the series of its dates, the period and every date that
//! differs, all events of the module in a fixed grid (three to a row; from seven on five and „+N",
//! the Termin's own among them), the module with its exam, who teaches, and the source.
//!
//! Every date, group and event leads to its own Termin by an address of the page (`TerminLinks`),
//! so Back steps back through them, `nofollow` (a Termin's own page is no page search engines
//! list); the actions change the plan in the click (it asks the catalog nothing: the store, the
//! selection, the timetable).
//!
//! Where it stands (`TerminPlace`; the concept: „Sheet, das zur Seite wird"): beside a page, on a
//! phone a sheet over it, with „Teilen", „Ganze Seite" (V) and „Schließen" (Esc) at its head (the
//! owner, 2026-10-26: „bau als Nächstes Teilen und Ganze Seite"); after „Ganze Seite" filling the
//! page it stood beside, the same view in two columns — on the left the day (the ticket, who, „Im
//! Stundenplan" or the groups, the day, the series, the period), on the right what one looks up —
//! with „Zurück" (Esc) to the view beside the page; and as a page of its own
//! (`/event/<key>/<day>`) in the same two columns. „Teilen" hands on that page's address.

use std::collections::BTreeMap;

use folia_calendar::day::{clock, Day};
use folia_calendar::kind::EventKind;
use folia_calendar::rowkey::RowKey;
use folia_calendar::select::HiddenBy;
use folia_pages::StudyplanData;
use folia_routes::url;
use folia_stores::studyplan::Studyplan;
use folia_timetable::clash::Weeks;
use folia_timetable::export;
use folia_timetable::ics;
use folia_timetable::model::{Attendance, Event, Timetable};
use folia_timetable::occur::Every;
use folia_timetable::termin::{self, DateState, Occasion};
use folia_timetable::views::{kind_short, short_title, type_text};
use leptos::prelude::*;

use folia_design::format;
use folia_design::ui::{Icon, Shortcut};
use folia_shell::frame::BackLink;
use folia_shell::tabs::Area;
use crate::i18n::{self, Locale, Texts};

/// Which of the two views (owner, 2026-10-26: „Wir sollten das wahrscheinlich Trennen zwischen der
/// Informativen ansicht für die Modulpage und mein studiengang und dann der ansicht bei der
/// Kalender page"): from the Stundenplan and the start page the one that adjusts the plan, from a
/// module's page the one that looks up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminMode {
    Adjust,
    LookUp,
}

/// What the view knows of a module beside the timetable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModuleFacts {
    pub title: String,
    /// Its abbreviation in the plan's program („EvS"), which the grids name it by.
    pub abbrev: Option<String>,
    pub credits: Option<f64>,
    /// Who is responsible for it, as the catalog writes it.
    pub responsible: Option<String>,
}

/// The modules of a semester's data as the view names them: the title, the few letters a slot
/// names them by (`StudyplanData::slot_names`), the credits and who is responsible.
pub fn module_facts(data: &StudyplanData) -> BTreeMap<String, ModuleFacts> {
    let names = data.slot_names();
    data.modules
        .iter()
        .map(|row| {
            let facts = ModuleFacts { title: row.title.clone(), abbrev: names.get(&row.id).cloned(), credits: row.credits, responsible: row.responsible.clone() };
            (row.id.clone(), facts)
        })
        .collect()
}

/// The addresses the view leads to, as the page writes them: a date of a Termin of the timetable
/// (`None`: its next one), the module beside the page pointing at a Termin, and the page without
/// the view.
#[derive(Clone, Copy)]
pub struct TerminLinks {
    pub termin: Callback<(RowKey, Option<Day>), String>,
    pub module: Callback<(String, Option<RowKey>), String>,
    pub close: Signal<String>,
    /// „history" where closing goes back a step of the history (enhance.js), as the module's
    /// panel does.
    pub back: Signal<Option<&'static str>>,
    /// To look up, the Termin's view in the Stundenplan, where it is adjusted („Anpassen ›"):
    /// `None` where its module is not planned, and from the Stundenplan itself.
    pub adjust: Option<Callback<(RowKey, Option<Day>), String>>,
    /// „Ganze Seite": the Termin on a day filling the page it stands beside (`full=1`); `None`
    /// where the page has no such place.
    pub full: Option<Callback<(RowKey, Option<Day>), String>>,
}

/// Where the view stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminPlace {
    /// Beside a page (`#preview`), as the module's panel stands, on a phone a sheet over it: at
    /// its head „Teilen", „Ganze Seite" where the page has a place for it, and „Schließen"
    /// (`TerminLinks::close`, the page without the view).
    Beside,
    /// Filling the page of `Area` it stood beside („Ganze Seite"): two columns where the page is
    /// wide, „Zurück" to the view beside the page (`TerminLinks::close`) and „Teilen".
    Full(Area),
    /// A page of its own (`/event/<key>/<day>`): the same two columns, and „Teilen".
    Page,
}

/// What a page gives the view.
#[derive(Clone, Copy)]
pub struct TerminCtx {
    /// The timetable the Termin is one of: the plan's semester.
    pub table: Memo<Option<Timetable>>,
    /// The Termin asked for and its day (`None`: its next date).
    pub asked: Memo<Option<(RowKey, Option<Day>)>>,
    /// The timetable's modules by id.
    pub modules: Memo<BTreeMap<String, ModuleFacts>>,
    pub mode: TerminMode,
    /// The plan the view changes; `None` to look up.
    pub plan: Option<Studyplan>,
    /// The day and minute it is, as the page reads the clock; `None` on the server.
    pub now: Signal<Option<(Day, u16)>>,
    /// The Termin's module is in the visitor's plan of the timetable's semester (from the
    /// Stundenplan and the start page always); to look up, one that is not says so and offers
    /// its dates for the visitor's calendar („In den Kalender").
    /// `None` where that is not known: the server, which knows no plan (R9), says neither.
    pub planned: Signal<Option<bool>>,
    /// What a group's dot, a clash and „Der Tag" measure the Termin against.
    pub against: Signal<Against>,
    pub links: TerminLinks,
    pub place: TerminPlace,
}

/// What the view measures a Termin against: whether a group fits, what the date meets, the day
/// around it.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Against {
    /// Its own timetable, which is the visitor's plan (the Stundenplan, the start page, the page
    /// of a planned module).
    #[default]
    Itself,
    /// The visitor's plan of the semester, which does not hold the Termin's module (folia/design/
    /// termin, „Nicht im Plan": the groups green and red, a clash as a line of its own, the
    /// Termin hatched among the plan's day).
    Plan(Box<Timetable>),
    /// Nothing: the server, which knows no plan (R9), and a visitor without a plan of the
    /// semester. No group then says it fits, and there is no day to show.
    Nothing,
}

impl Against {
    /// The plan measured against, and the plan beside the Termin's timetable where that is it.
    fn plans<'a>(&'a self, table: &'a Timetable) -> (Option<&'a Timetable>, Option<&'a Timetable>) {
        match self {
            Against::Itself => (Some(table), None),
            Against::Plan(plan) => (Some(plan), Some(plan)),
            Against::Nothing => (None, None),
        }
    }
}

/// The most tiles of a grid before it folds: six stand, of more five and „+N".
const GRID_FOLD: usize = 6;

/// The browser's day and minute, read again every half minute while the caller stands (the pill
/// beside the date runs with it); on the server none, which renders no day (R9).
pub fn ticking_clock() -> Signal<Option<(Day, u16)>> {
    let now = RwSignal::new(read_clock());
    #[cfg(feature = "csr")]
    {
        if let Ok(handle) = set_interval_with_handle(
            move || {
                let read = read_clock();
                if now.try_get_untracked().is_some_and(|before| before != read) {
                    now.try_set(read);
                }
            },
            std::time::Duration::from_secs(30),
        ) {
            on_cleanup(move || handle.clear());
        }
    }
    now.into()
}

fn read_clock() -> Option<(Day, u16)> {
    #[cfg(feature = "csr")]
    {
        let now = web_sys::js_sys::Date::new_0();
        let day = Day::from_ymd(i32::try_from(now.get_full_year()).ok()?, now.get_month() + 1, now.get_date())?;
        Some((day, u16::try_from(now.get_hours() * 60 + now.get_minutes()).ok()?))
    }
    #[cfg(not(feature = "csr"))]
    None
}

// ---------- what the view shows ----------

#[derive(Clone, Debug, PartialEq)]
struct Shown {
    key: RowKey,
    day: Day,
    head: Head,
    ticket: Ticket,
    people: Vec<Person>,
    hints: Vec<Hint>,
    plan: Option<PlanPart>,
    groups: Option<Groups>,
    the_day: Option<DayPart>,
    series: Option<SeriesPart>,
    range: RangePart,
    events: Vec<EventPart>,
    module: Option<ModulePart>,
    staff: Vec<Person>,
    source: SourcePart,
    /// To look up a Termin outside the plan: its dates as a calendar file („In den Kalender") and
    /// the file's name.
    file: Option<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
struct Head {
    /// To look up, its module is not in the visitor's plan („nicht im Plan").
    out: bool,
    tone: u8,
    chip: String,
    kind: String,
    title: String,
}

#[derive(Clone, Debug, PartialEq)]
struct Ticket {
    date: String,
    prev: Option<Day>,
    next: Option<Day>,
    weeks: Option<Weeks>,
    /// The pill beside the date: its class („no", „soon", „live", none) and words.
    pill: Option<(&'static str, String)>,
    /// „09:15" and „bis 10:45"; `None` for a time unclear.
    time: Option<(String, String)>,
    rooms: Vec<String>,
    /// QIS's name of the room where it says more than the short one, and the campus.
    place: Vec<String>,
    /// „cancel", „off" (the break, a holiday), „past", „note", or none.
    state: &'static str,
    /// How far a running date has run, in percent.
    live: Option<u16>,
}

#[derive(Clone, Debug, PartialEq)]
struct Person {
    name: String,
    role: String,
}

#[derive(Clone, Debug, PartialEq)]
struct Hint {
    warn: bool,
    icon: &'static str,
    text: String,
}

/// „Im Stundenplan": the event's switch and the date's, the groups to tick.
#[derive(Clone, Debug, PartialEq)]
struct PlanPart {
    event: u32,
    kind: String,
    /// The event is not hidden as a whole (by its eye).
    shown: bool,
    sub: String,
    /// The date to hide or show: its words, its key, its day, whether it is hidden.
    date: Option<(String, RowKey, Day, bool)>,
}

#[derive(Clone, Debug, PartialEq)]
struct Groups {
    word: String,
    right: String,
    tiles: Vec<TileShown>,
    note: Option<String>,
    /// To look up, of a module in the plan: how many groups it holds („Eine in deinem
    /// Stundenplan"), beside „Anpassen ›".
    foot: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct TileShown {
    option: usize,
    key: RowKey,
    keys: Vec<RowKey>,
    when: String,
    room: String,
    name: Option<String>,
    ticked: bool,
    current: bool,
    /// The dot: green where it fits (`Some(true)`), red where it meets the plan; yellow where a
    /// group of a visit meets it and gives way (`gives`).
    dot: Option<bool>,
    gives: bool,
    label: String,
}

#[derive(Clone, Debug, PartialEq)]
struct DayPart {
    items: Vec<RibbonItem>,
    lanes: usize,
    /// „danach 11:30 Ü EvS · VG1C/0.03" with the time between, or „danach frei".
    then: Option<(String, Option<String>)>,
}

#[derive(Clone, Debug, PartialEq)]
struct RibbonItem {
    key: Option<RowKey>,
    from: u16,
    to: u16,
    lane: usize,
    short: String,
    label: String,
    tone: u8,
    me: bool,
    gone: bool,
    open: bool,
    /// Of a module outside the plan: hatched among the plan's Termine.
    out: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct SeriesPart {
    right: String,
    dots: Vec<Dot>,
}

#[derive(Clone, Debug, PartialEq)]
struct Dot {
    day: Day,
    class: String,
    label: String,
    link: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct RangePart {
    head: String,
    assumed: bool,
    /// The dates that differ: the day(s), the words, the class („note", „cancel", „off").
    lines: Vec<(String, String, &'static str)>,
}

#[derive(Clone, Debug, PartialEq)]
struct EventPart {
    kind: String,
    one_of: Option<String>,
    tone: u8,
    sessions: Vec<SessionShown>,
}

#[derive(Clone, Debug, PartialEq)]
struct SessionShown {
    key: RowKey,
    when: String,
    room: String,
    me: bool,
    ticked: bool,
    label: String,
}

#[derive(Clone, Debug, PartialEq)]
struct ModulePart {
    id: String,
    line: String,
    exams: Vec<(String, String, Option<String>)>,
}

#[derive(Clone, Debug, PartialEq)]
struct SourcePart {
    what: String,
    qis: Option<String>,
}

// ---------- the view ----------

/// The view of the Termin `ctx.asked` (`section.panel.detail.aside.is-termin#preview`): beside the
/// page where it is wide, as the module's panel stands, a sheet over it on a phone.
#[component]
pub fn TerminView(ctx: TerminCtx) -> impl IntoView {
    let t = i18n::t();
    let shown = Memo::new(move |_| {
        let (now, planned) = (ctx.now.get(), ctx.planned.get());
        ctx.asked.with(|asked| {
            ctx.table.with(|table| {
                ctx.against.with(|against| ctx.modules.with(|modules| asked.and_then(|(key, on)| shown_of(table.as_ref()?, against, key, on, now, modules, (ctx.mode, planned), t))))
            })
        })
    });
    let found = Memo::new(move |_| shown.with(Option::is_some));
    let loading = Memo::new(move |_| ctx.table.with(Option::is_none));
    let head = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| s.head.clone())));
    let close = ctx.links.close;
    let back = ctx.links.back;
    // The Termin and the day shown, which „Teilen" and „Ganze Seite" name, and what a share sheet
    // calls it.
    let at = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| (s.key, s.day))));
    let called = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| format!("{} · {} · {}", s.head.kind, s.head.title, s.ticket.date)).unwrap_or_default()));

    let top = move || {
        let head = head.get();
        let chip = head.as_ref().map(|head| view! { <span class=format!("tm-mod {}", hue(head.tone)) class:out=head.out><i></i>{head.chip.clone()}</span> });
        let kind = head.as_ref().map(|head| view! { <span class="tm-kind">{head.kind.clone()}</span> });
        let out = head.as_ref().is_some_and(|head| head.out).then(|| view! { <span class="np">{t.termin.not_in_plan}</span> });
        // Filling a page: „Zurück" to the view beside it, first (on a phone the arrow at the
        // head says it, app.css). Its address follows the day shown.
        let up = match ctx.place {
            TerminPlace::Full(area) => Some(move || {
                let to = close.with(|close| i18n::app_path(close).unwrap_or(close).to_string());
                view! { <BackLink area to=Some(to)/> }
            }),
            _ => None,
        };
        let tools = match ctx.place {
            // Beside a page: „Teilen" and „Ganze Seite" as their signs (the room is the kind's),
            // „Schließen" in words as on the module's panel. „Ganze Seite" not on a phone's sheet,
            // which goes all up itself (app.css, `.full-view`).
            TerminPlace::Beside => view! {
                <ShareButton at called labeled=false/>
                {ctx.links.full.map(|full| view! {
                    <a class="ghost full-view tm-full" href=move || at.get().map(|(key, day)| full.run((key, Some(day)))).unwrap_or_default() rel="nofollow" data-action="fullscreen" title=t.termin.full_title aria-label=t.termin.full>
                        <Icon name="maximize-2"/><Shortcut keys="V"/>
                    </a>
                })}
                <a class="ghost tm-close" href=move || close.get() rel="nofollow" data-action="close-detail" data-back=move || back.get() data-noscroll="" title=t.termin.close_title><Icon name="x"/>{t.common.close}<Shortcut keys="Esc"/></a>
            }
            .into_any(),
            TerminPlace::Full(_) | TerminPlace::Page => view! { <ShareButton at called labeled=true/> }.into_any(),
        };
        view! {
            <div class="hero-top tm-bar">
                {up}
                {chip}
                {kind}
                {out}
                {tools}
            </div>
            {head.map(|head| view! { <h2 class="tm-title">{head.title}</h2> })}
        }
    };
    // What one adjusts (from the Stundenplan and the start page) or the groups to look at.
    let middle = move || match ctx.mode {
        TerminMode::Adjust => view! { <PlanView ctx shown/> }.into_any(),
        TerminMode::LookUp => view! { <GroupsView ctx shown adjust=false/><CalendarFile ctx shown/> }.into_any(),
    };
    let body = move || {
        if !found.get() {
            return match loading.get() {
                true => ().into_any(),
                false => view! { <p class="hint">{t.termin.gone}</p> }.into_any(),
            };
        }
        match ctx.place {
            TerminPlace::Beside => view! {
                <TicketView ctx shown/>
                <WhoView shown/>
                {middle()}
                <DayView ctx shown/>
                <SeriesView ctx shown/>
                <ModuleView ctx shown/>
                <div class="tm-more">
                    <EventsView ctx shown/>
                    <StaffView shown/>
                    <RangeView shown/>
                </div>
                <SourceView shown/>
            }
            .into_any(),
            // As a page (folia/design/termin, „Die Seite"): the day on the left, what one looks up
            // on the right; one column where the page is narrow.
            TerminPlace::Full(_) | TerminPlace::Page => view! {
                <div class="tm-col">
                    <TicketView ctx shown/>
                    <WhoView shown/>
                    {middle()}
                    <DayView ctx shown/>
                    <SeriesView ctx shown/>
                    <RangeView shown/>
                </div>
                <div class="tm-col tm-look">
                    <ModuleView ctx shown/>
                    <EventsView ctx shown/>
                    <StaffView shown/>
                    <SourceView shown/>
                </div>
            }
            .into_any(),
        }
    };
    // Beside a page `is-module`: on a phone a sheet over it, as a module's panel (app.css,
    // enhance.js). As a page a panel of its own, the page's whole width.
    let (class, id) = match ctx.place {
        TerminPlace::Beside => ("panel detail aside is-module is-termin", Some("preview")),
        TerminPlace::Full(_) => ("panel is-termin tm-page tm-filling", None),
        TerminPlace::Page => ("panel is-termin tm-page", None),
    };
    view! {
        <section class=class id=id aria-label=t.termin.panel>
            <div class="scroll" id="termin-scroll">
                <header class="hero tm-head">{top}</header>
                <div class="dbody tm">{body}</div>
            </div>
        </section>
    }
}

/// The date, where it stands, the time and the room.
#[component]
fn TicketView(ctx: TerminCtx, shown: Memo<Option<Shown>>) -> impl IntoView {
    let t = i18n::t();
    let ticket = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| (s.key, s.ticket.clone()))));
    move || {
        ticket.get().map(|(key, ticket)| {
            // The date before and after: ← and → press them (enhance.js, `data-step`).
            let step = |day: Option<Day>, label: &'static str, (icon, way, shortcut): (&'static str, &'static str, &'static str)| match day {
                Some(day) => view! { <a class="pg" href=ctx.links.termin.run((key, Some(day))) rel="nofollow" data-noscroll="" data-step=way aria-keyshortcuts=shortcut aria-label=label title=label><Icon name=icon/></a> }.into_any(),
                None => view! { <span class="pg" aria-hidden="true"><Icon name=icon/></span> }.into_any(),
            };
            let ab = ticket.weeks.map(|weeks| {
                let (a, b) = (weeks == Weeks::A, weeks == Weeks::B);
                let said = (t.termin.week_of)(if a { "A" } else { "B" });
                view! { <span class="abg" title=said.clone() aria-label=said><i class:on=a>"A"</i><i class:on=b>"B"</i></span> }
            });
            let pill = ticket.pill.map(|(class, text)| view! { <span class=format!("pill {class}")>{text}</span> });
            let time = match ticket.time {
                Some((from, to)) => view! { <b>{from}</b><small>{to}</small> }.into_any(),
                None => view! { <b class="unclear">{t.termin.no_time}</b> }.into_any(),
            };
            let rooms = match ticket.rooms.is_empty() {
                true => view! { <b class="plate none">{t.termin.no_room}</b> }.into_any(),
                false => ticket.rooms.iter().map(|room| view! { <b class="plate">{plate(room)}</b> }).collect_view().into_any(),
            };
            let place = (!ticket.place.is_empty()).then(|| view! { <small>{ticket.place.join(" · ")}</small> });
            let live = ticket.live.map(|pct| view! { <div class="tk-prog" aria-hidden="true"><i style=format!("width: {pct}%")></i></div> });
            view! {
                <section class=format!("tk {}", ticket.state) aria-label=t.termin.panel>
                    <div class="tk-date">
                        {step(ticket.prev, t.termin.earlier, ("chevron-left", "prev", "ArrowLeft"))}
                        <b>{ticket.date}</b>
                        {step(ticket.next, t.termin.later, ("chevron-right", "next", "ArrowRight"))}
                        {ab}
                        {pill}
                    </div>
                    <div class="tk-main">
                        <div class="tk-time">{time}</div>
                        <div class="tk-room">{rooms}{place}</div>
                    </div>
                    {live}
                </section>
            }
        })
    }
}

/// „VG1C/0.03" with its slash quieter.
fn plate(room: &str) -> AnyView {
    match room.split_once('/') {
        Some((building, rest)) => view! { {building.to_string()}<span class="sl">"/"</span>{rest.to_string()} }.into_any(),
        None => room.to_string().into_any(),
    }
}

/// Who leads it, and what else is said of it.
#[component]
fn WhoView(shown: Memo<Option<Shown>>) -> impl IntoView {
    let parts = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| (s.people.clone(), s.hints.clone())).unwrap_or_default()));
    move || {
        let (people, hints) = parts.get();
        view! {
            {people.into_iter().map(|person| view! {
                <div class="who"><span class="av"><Icon name="user-round"/></span><span><b>{person.name}</b><small>{person.role}</small></span></div>
            }).collect_view()}
            {hints.into_iter().map(|hint| view! {
                <p class="tm-hint" class:no=hint.warn><Icon name=hint.icon/><span>{hint.text}</span></p>
            }).collect_view()}
        }
    }
}

/// „Im Stundenplan": the groups to tick, the event shown or not, this one date hidden. Each acts at
/// once, and the Kalender-Abo follows the plan.
#[component]
fn PlanView(ctx: TerminCtx, shown: Memo<Option<Shown>>) -> impl IntoView {
    let t = i18n::t();
    let part = Memo::new(move |_| shown.with(|s| s.as_ref().and_then(|s| s.plan.clone())));
    let sem = Memo::new(move |_| ctx.table.with(|table| table.as_ref().map(|table| table.key)));
    let switch = move |_: leptos::ev::MouseEvent| {
        let (Some(plan), Some(sem), Some(part)) = (ctx.plan, sem.get_untracked(), part.get_untracked()) else { return };
        plan.update(|doc| doc.set_event(sem, part.event, part.shown));
    };
    let date = move |_: leptos::ev::MouseEvent| {
        let (Some(plan), Some(sem), Some((_, key, day, hidden))) = (ctx.plan, sem.get_untracked(), part.with_untracked(|part| part.as_ref().and_then(|part| part.date.clone()))) else { return };
        plan.update(|doc| doc.set_date(sem, key, day, !hidden));
    };
    move || {
        part.get().map(|part| {
            let label = (t.termin.show_kind)(&part.kind);
            let on = part.shown;
            let date_row = part.date.clone().map(|(words, _, _, hidden)| {
                view! {
                    <div class="crow">
                        <span class="ci"><Icon name="calendar-minus"/></span>
                        <span><b>{(t.termin.only_on)(&words)}</b><small>{if hidden { t.termin.hidden_others_stay } else { t.termin.others_stay }}</small></span>
                        <button class="sw" type="button" data-action="hide-date" aria-pressed=if hidden { "true" } else { "false" } on:click=date>{if hidden { t.termin.show } else { t.termin.hide }}</button>
                    </div>
                }
            });
            view! {
                <section class="tm-sec cal">
                    <h4>{t.termin.in_plan}<span class="r">{t.termin.in_feed}</span></h4>
                    {on.then(|| view! { <GroupsView ctx shown adjust=true/> })}
                    <div class="crow">
                        <span class="ci">{if on { view! { <Icon name="eye"/> }.into_any() } else { view! { <Icon name="eye-off"/> }.into_any() }}</span>
                        <span><b>{label.clone()}</b><small>{part.sub.clone()}</small></span>
                        <button class="tg" class:on=on type="button" role="switch" aria-checked=if on { "true" } else { "false" } aria-label=label data-action="show-event" on:click=switch><i></i></button>
                    </div>
                    {on.then_some(date_row)}
                </section>
            }
        })
    }
}

/// The groups of the event's choice in a grid of at most three columns: in the Stundenplan
/// (`adjust`) a tap ticks one or takes its tick away, to look up it opens the group's Termin. From
/// seven on five and „+N", what fits first, the Termin's own among them.
#[component]
fn GroupsView(ctx: TerminCtx, shown: Memo<Option<Shown>>, adjust: bool) -> impl IntoView {
    let t = i18n::t();
    let groups = Memo::new(move |_| shown.with(|s| s.as_ref().and_then(|s| s.groups.clone())));
    let more = RwSignal::new(false);
    let sem = Memo::new(move |_| ctx.table.with(|table| table.as_ref().map(|table| table.key)));
    let tick = move |keys: Vec<RowKey>, on: bool| {
        let (Some(plan), Some(sem)) = (ctx.plan, sem.get_untracked()) else { return };
        plan.update(|doc| match (on, keys.first()) {
            (true, Some(first)) => doc.tick(sem, *first, true),
            (true, None) => {}
            (false, _) => keys.iter().for_each(|key| doc.tick(sem, *key, false)),
        });
    };
    move || {
        groups.get().map(|groups| {
            let all = groups.tiles.len();
            let many = all > GRID_FOLD;
            let open = more.get();
            let tiles: Vec<TileShown> = if many && !open { folded(&groups.tiles, |tile| tile.current, |tile| tile.dot == Some(false)) } else { groups.tiles.clone() };
            let rest = all - tiles.len();
            let views = tiles
                .into_iter()
                .map(|tile| {
                    let dot = tile.dot.map(|fits| view! { <i class=match (fits, tile.gives) { (true, _) => "st ok", (false, false) => "st no", (false, true) => "st gives" }></i> });
                    let body = view! {
                        <b>{tile.when.clone()}{dot}</b>
                        <small>{tile.room.clone()}</small>
                        {tile.name.clone().map(|name| view! { <small class="nm">{name}</small> })}
                    };
                    match adjust {
                        true => {
                            let (keys, on) = (tile.keys.clone(), !tile.ticked);
                            view! {
                                <button class="gs pick" class:on=tile.ticked class:this=tile.current type="button" role="checkbox" aria-checked=if tile.ticked { "true" } else { "false" } data-action="tick" title=tile.label.clone() aria-label=tile.label.clone() on:click=move |_| tick(keys.clone(), on)>
                                    {body}
                                    {if tile.ticked { view! { <i class="ck"><Icon name="check"/></i> }.into_any() } else { view! { <i class="cb"></i> }.into_any() }}
                                </button>
                            }
                            .into_any()
                        }
                        false => view! {
                            <a class="gs" class:this=tile.current href=ctx.links.termin.run((tile.key, None)) rel="nofollow" data-noscroll="" title=tile.label.clone() aria-label=tile.label.clone() aria-current=tile.current.then_some("true")>
                                {body}
                                {tile.ticked.then(|| view! { <i class="ck"><Icon name="check"/></i> })}
                            </a>
                        }
                        .into_any(),
                    }
                })
                .collect_view();
            let fold = (many).then(|| match open {
                false => view! { <button class="gs more" type="button" aria-label=(t.termin.more_label)(rest) on:click=move |_| more.set(true)><b>{format!("+{rest}")}</b><small>{t.termin.further}</small></button> }.into_any(),
                true => view! { <button class="gs more" type="button" aria-label=t.termin.fewer on:click=move |_| more.set(false)><b><Icon name="chevron-down" class="up"/></b><small>{t.termin.fewer}</small></button> }.into_any(),
            });
            view! {
                <div class="grp">
                    <div class="grp-h"><span>{groups.word.clone()}</span><span class="r">{groups.right.clone()}</span></div>
                    <div class="grp-seg" style=format!("--n: {}", all.clamp(1, 3))>{views}{fold}</div>
                    {groups.note.clone().map(|note| view! { <p class="grp-n">{note}</p> })}
                    {groups.foot.clone().map(|foot| view! {
                        <p class="grp-f">
                            <span>{foot}</span>
                            {adjust_link(ctx, shown).map(|href| view! { <a class="go2" href=href rel="nofollow">{t.termin.adjust}<Icon name="chevron-right"/></a> })}
                        </p>
                    })}
                </div>
            }
        })
    }
}

/// Of a long grid the first five, `own` always among them (in the last place where it is not):
/// `late` ones last (those that meet the plan), the others in their order.
fn folded<T: Clone>(tiles: &[T], own: impl Fn(&T) -> bool, late: impl Fn(&T) -> bool) -> Vec<T> {
    let mut sorted: Vec<&T> = tiles.iter().filter(|tile| !late(tile)).chain(tiles.iter().filter(|tile| late(tile))).collect();
    let mine = sorted.iter().position(|tile| own(tile));
    sorted.truncate(GRID_FOLD - 1);
    if let (Some(at), Some(last)) = (mine, sorted.last_mut()) {
        if at >= GRID_FOLD - 1 {
            if let Some(tile) = tiles.iter().find(|tile| own(tile)) {
                *last = tile;
            }
        }
    }
    sorted.into_iter().cloned().collect()
}

/// The Termin's view in the Stundenplan, on the date shown („Anpassen ›"), where the page gives
/// one (`TerminLinks::adjust`) and the Termin is in the plan.
fn adjust_link(ctx: TerminCtx, shown: Memo<Option<Shown>>) -> Option<String> {
    if ctx.planned.get() != Some(true) {
        return None;
    }
    let (key, day) = shown.with(|s| s.as_ref().map(|s| (s.key, s.day)))?;
    ctx.links.adjust.map(|adjust| adjust.run((key, Some(day))))
}

/// „Teilen": the address of the Termin's own page on the day shown (`/event/<key>/<day>`, in the
/// page's language), to the device's share sheet on a touch screen (`called` its title there),
/// else into the clipboard with „Link kopiert" in its place for a moment (enhance.js,
/// `share-link`). Beside a page its sign alone (`labeled`: with its word). It needs JavaScript
/// (R15).
#[component]
fn ShareButton(at: Memo<Option<(RowKey, Day)>>, called: Memo<String>, labeled: bool) -> impl IntoView {
    let t = i18n::t();
    let href = move || at.get().map(|(key, day)| t.path(&url::event_path(key, Some(day)))).unwrap_or_default();
    view! {
        <button type="button" class="ghost tm-share js-only" class:icon-only=!labeled data-action="share-link" data-text=href data-absolute="" data-title=move || called.get() title=t.termin.share_title aria-label=t.termin.share>
            <Icon name="share-2"/><span data-label="">{labeled.then_some(t.termin.share)}</span>
        </button>
    }
}

/// „In den Kalender": the dates of a Termin outside the plan as a calendar file, made ahead as a
/// Blob URL, so the click has nothing left to do (R21), as the Stundenplan's „.ics herunterladen".
#[component]
fn CalendarFile(ctx: TerminCtx, shown: Memo<Option<Shown>>) -> impl IntoView {
    let t = i18n::t();
    let file = Memo::new(move |_| shown.with(|s| s.as_ref().and_then(|s| s.file.clone())));
    let url = RwSignal::new(None::<String>);
    #[cfg(feature = "csr")]
    {
        Effect::new(move |before: Option<Option<String>>| {
            if let Some(Some(old)) = before {
                let _ = web_sys::Url::revoke_object_url(&old);
            }
            let made = file.with(|file| file.as_ref().and_then(|(text, _)| object_url(text)));
            url.set(made.clone());
            made
        });
        on_cleanup(move || {
            if let Some(old) = url.get_untracked() {
                let _ = web_sys::Url::revoke_object_url(&old);
            }
        });
    }
    let _ = ctx;
    move || {
        file.get().map(|(_, name)| {
            view! {
                <a class="tm-big" href=move || url.get().unwrap_or_default() download=name title=t.termin.into_calendar_title aria-disabled=move || url.with(Option::is_none).then_some("true")>
                    <Icon name="calendar-plus"/>{t.termin.into_calendar}
                </a>
            }
        })
    }
}

/// The text as a Blob URL of an iCalendar file. Only in the browser.
#[cfg(feature = "csr")]
fn object_url(text: &str) -> Option<String> {
    let parts = web_sys::js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/calendar;charset=utf-8");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options).ok()?;
    web_sys::Url::create_object_url_with_blob(&blob).ok()
}

/// „Der Tag": the plan's Termine of the day on a ribbon from 7 to 19, this one marked; then what
/// comes after it.
#[component]
fn DayView(ctx: TerminCtx, shown: Memo<Option<Shown>>) -> impl IntoView {
    let t = i18n::t();
    let part = Memo::new(move |_| shown.with(|s| s.as_ref().and_then(|s| s.the_day.clone().map(|part| (s.day, part)))));
    move || {
        part.get().map(|(day, part)| {
            const FIRST: u16 = 7 * 60;
            const SPAN: u16 = 12 * 60;
            let at = |minute: u16| f64::from(minute.saturating_sub(FIRST).min(SPAN)) * 100.0 / f64::from(SPAN);
            let lanes = part.lanes.max(1);
            let height = (36.0 - 8.0 - 2.0 * (lanes as f64 - 1.0)) / lanes as f64;
            let blocks = part
                .items
                .into_iter()
                .map(|item| {
                    let style = format!(
                        "left: {:.2}%; width: calc({:.2}% - 2px); top: {:.1}px; height: {:.1}px",
                        at(item.from),
                        at(item.to) - at(item.from),
                        4.0 + item.lane as f64 * (height + 2.0),
                        height
                    );
                    let class = format!(
                        "rb {}{}{}{}{}",
                        hue(item.tone),
                        if item.me { " me" } else { "" },
                        if item.gone { " gone" } else { "" },
                        if item.open { " open" } else { "" },
                        if item.out { " out" } else { "" }
                    );
                    match (item.me, item.key) {
                        (false, Some(key)) => view! { <a class=class style=style href=ctx.links.termin.run((key, Some(day))) rel="nofollow" data-noscroll="" title=item.label.clone() aria-label=item.label>{item.short}</a> }.into_any(),
                        _ => view! { <span class=class style=style title=item.label>{item.short}</span> }.into_any(),
                    }
                })
                .collect_view();
            let hours = [8u16, 10, 12, 14, 16, 18].into_iter().map(|hour| view! { <span style=format!("left: {:.2}%", at(hour * 60))>{hour}</span> }).collect_view();
            let then = part.then.map(|(what, gap)| match what.is_empty() {
                true => view! { <p class="nx"><Icon name="arrow-right"/><span>{t.termin.then_free}</span></p> }.into_any(),
                false => view! { <p class="nx"><Icon name="arrow-right"/><span>{format!("{} {what}", t.termin.then)}</span>{gap.map(|gap| view! { <span class="w">{gap}</span> })}</p> }.into_any(),
            });
            view! {
                <section class="tm-sec">
                    <h4>{t.termin.the_day}</h4>
                    <div class="rib">{blocks}</div>
                    <div class="rt" aria-hidden="true">{hours}</div>
                    {then}
                </section>
            }
        })
    }
}

/// „Reihe": a dot per date, each leading to it, the months under them.
#[component]
fn SeriesView(ctx: TerminCtx, shown: Memo<Option<Shown>>) -> impl IntoView {
    let t = i18n::t();
    let part = Memo::new(move |_| shown.with(|s| s.as_ref().and_then(|s| s.series.clone().map(|part| (s.key, part)))));
    move || {
        part.get().map(|(key, part)| {
            let dots = part
                .dots
                .into_iter()
                .map(|dot| match dot.link {
                    true => view! { <a class=dot.class href=ctx.links.termin.run((key, Some(dot.day))) rel="nofollow" data-noscroll="" title=dot.label.clone() aria-label=dot.label></a> }.into_any(),
                    false => view! { <span class=dot.class title=dot.label></span> }.into_any(),
                })
                .collect_view();
            view! {
                <section class="tm-sec">
                    <h4>{t.termin.series}<span class="r">{part.right}</span></h4>
                    <div class="sdots">{dots}</div>
                </section>
            }
        })
    }
}

/// „Modul": the way to the module beside the page, and its exam.
#[component]
fn ModuleView(ctx: TerminCtx, shown: Memo<Option<Shown>>) -> impl IntoView {
    let t = i18n::t();
    let part = Memo::new(move |_| shown.with(|s| s.as_ref().and_then(|s| s.module.clone().map(|part| (s.key, part)))));
    let grouped = Memo::new(move |_| shown.with(|s| s.as_ref().is_some_and(|s| s.groups.is_some())));
    move || {
        part.get().map(|(key, part)| {
            let exams = part
                .exams
                .into_iter()
                .map(|(date, line, left)| {
                    view! {
                        <div class="mrow plain"><span class="ico"><Icon name="file-check-2"/></span><span><b>{format!("{} {date}", t.termin.exam)}</b><small>{line}</small></span><span class="v">{left}</span></div>
                    }
                })
                .collect_view();
            // To look up a Termin of the plan without groups: the way into the Stundenplan here (with
            // groups it stands under them).
            let adjust = (ctx.mode == TerminMode::LookUp && !grouped.get()).then(|| adjust_link(ctx, shown)).flatten().map(|href| {
                view! { <a class="mrow" href=href rel="nofollow"><span class="ico"><Icon name="calendar-days"/></span><span><b>{t.termin.adjust_row}</b><small>{t.termin.adjust_sub}</small></span><span class="v"></span><Icon name="chevron-right"/></a> }
            });
            view! {
                <section class="tm-sec">
                    <h4>{t.termin.module}</h4>
                    <a class="mrow" href=ctx.links.module.run((part.id.clone(), Some(key))) data-noscroll=""><span class="ico"><Icon name="graduation-cap"/></span><span><b>{t.termin.to_module}</b><small>{part.line}</small></span><span class="v"></span><Icon name="chevron-right"/></a>
                    {adjust}
                    {exams}
                </section>
            }
        })
    }
}

/// „Alle Veranstaltungen" of the module: each event's Termine in a fixed grid, three to a row.
#[component]
fn EventsView(ctx: TerminCtx, shown: Memo<Option<Shown>>) -> impl IntoView {
    let t = i18n::t();
    let events = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| s.events.clone()).unwrap_or_default()));
    let more = RwSignal::new(Vec::<usize>::new());
    move || {
        let events = events.get();
        (!events.is_empty()).then(|| {
            let rows = events
                .into_iter()
                .enumerate()
                .map(|(index, event)| {
                    let all = event.sessions.len();
                    let open = more.with(|more| more.contains(&index));
                    let sessions = match all > GRID_FOLD && !open {
                        true => folded(&event.sessions, |session| session.me, |_| false),
                        false => event.sessions.clone(),
                    };
                    let rest = all - sessions.len();
                    let tiles = sessions
                        .into_iter()
                        .map(|session| {
                            let href = ctx.links.termin.run((session.key, None));
                            view! {
                                <a class="esl" class:me=session.me href=href rel="nofollow" data-noscroll="" title=session.label.clone() aria-label=session.label aria-current=session.me.then_some("true")>
                                    <b>{session.when}</b><small>{session.room}</small>
                                    {session.ticked.then(|| view! { <i class="ck"><Icon name="check"/></i> })}
                                </a>
                            }
                        })
                        .collect_view();
                    let fold = (rest > 0).then(|| view! {
                        <button class="esl more" type="button" aria-label=(t.termin.more_label)(rest) on:click=move |_| more.update(|more| more.push(index))><b>{format!("+{rest}")}</b><small>{t.termin.further}</small></button>
                    });
                    view! {
                        <div class=format!("evr {}", hue(event.tone))>
                            <div class="evh"><b>{event.kind}</b>{event.one_of.map(|one_of| view! { <small>{one_of}</small> })}</div>
                            <div class="esls">{tiles}{fold}</div>
                        </div>
                    }
                })
                .collect_view();
            view! {
                <section class="tm-sec">
                    <h4>{t.termin.all_events}</h4>
                    <div class="evs">{rows}</div>
                </section>
            }
        })
    }
}

/// „Lehrende": the module's responsible, and who leads which Termin.
#[component]
fn StaffView(shown: Memo<Option<Shown>>) -> impl IntoView {
    let t = i18n::t();
    let staff = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| s.staff.clone()).unwrap_or_default()));
    move || {
        let staff = staff.get();
        (!staff.is_empty()).then(|| {
            view! {
                <section class="tm-sec">
                    <h4>{t.termin.staff}<span class="r">{t.termin.per_qis}</span></h4>
                    <div class="ppl">
                        {staff.into_iter().map(|person| view! {
                            <div class="who sm"><span class="av"><Icon name="user-round"/></span><span><b>{person.name}</b><small>{person.role}</small></span></div>
                        }).collect_view()}
                    </div>
                </section>
            }
        })
    }
}

/// „Zeitraum und Ausnahmen": the rhythm from the first to the last date, and every date that
/// differs.
#[component]
fn RangeView(shown: Memo<Option<Shown>>) -> impl IntoView {
    let t = i18n::t();
    let part = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| s.range.clone())));
    move || {
        part.get().map(|part| {
            let lines = match part.lines.is_empty() {
                true => view! { <p class="rng">{t.termin.no_exceptions}</p> }.into_any(),
                false => view! { <ul class="exl2">{part.lines.into_iter().map(|(days, text, class)| view! { <li class=class><b>{days}</b><span>{text}</span></li> }).collect_view()}</ul> }.into_any(),
            };
            view! {
                <section class="tm-sec">
                    <h4>{t.termin.range}</h4>
                    <p class="rng">{part.head}</p>
                    {part.assumed.then(|| view! { <p class="rng">{t.termin.assumed}</p> })}
                    {lines}
                </section>
            }
        })
    }
}

/// The source: „Quelle: QIS · Übung 12345 · In QIS öffnen ↗".
#[component]
fn SourceView(shown: Memo<Option<Shown>>) -> impl IntoView {
    let t = i18n::t();
    let part = Memo::new(move |_| shown.with(|s| s.as_ref().map(|s| s.source.clone())));
    move || {
        part.get().map(|part| {
            view! {
                <footer class="tm-src">
                    <span>{t.termin.source}</span>
                    <span>{part.what}</span>
                    {part.qis.map(|href| view! { <span><a href=href rel="noopener" target="_blank">{t.termin.open_in_qis}<Icon name="arrow-up-right"/></a></span> })}
                </footer>
            }
        })
    }
}

// ---------- what is shown, worked out ----------

/// The colour of a module of the plan by its tone (1–8), as the Stundenplan gives it: the classes
/// that set `--tint`.
pub fn hue(tone: u8) -> &'static str {
    const HUES: [&str; 8] = ["t-ice", "t-sun", "t-violet", "t-teal", "t-green", "t-coral", "t-rose", "t-slate"];
    HUES.get(usize::from(tone.saturating_sub(1)) % HUES.len()).copied().unwrap_or("t-ice")
}

fn weekday(day: u8, locale: Locale) -> &'static str {
    locale.texts().weekday_short(i64::from(day)).unwrap_or_default()
}

/// „Mi 04.11.", with the year where it is not this one („Mi 04.11.2027").
fn day_words(day: Day, today: Option<Day>, locale: Locale) -> String {
    let this_year = today.is_none_or(|today| today.ymd().0 == day.ymd().0);
    let date = if this_year { day.day_month(locale) } else { day.date(locale) };
    format!("{} {date}", weekday(day.weekday(), locale))
}

/// What a module is called in a few letters: its abbreviation, else its title cut short, else its
/// id.
fn chip_of(id: Option<&String>, modules: &BTreeMap<String, ModuleFacts>) -> String {
    let facts = id.and_then(|id| modules.get(id));
    facts
        .and_then(|facts| facts.abbrev.clone())
        .or_else(|| facts.map(|facts| short_title(&facts.title)))
        .or_else(|| id.cloned())
        .unwrap_or_default()
}

/// „Köhler, Ekkehard" as one says it: „Ekkehard Köhler", the titles before the surname first
/// („Prof. Dr. Hofstedt, Petra": „Prof. Dr. Petra Hofstedt"); several names and one alone stay.
fn full_name(name: &str) -> String {
    match name.split_once(',') {
        Some((last, first)) if !first.contains(',') && !last.contains('/') && !first.trim().is_empty() => {
            let words: Vec<&str> = last.split_whitespace().collect();
            let titles = words.iter().take_while(|word| word.ends_with('.')).count();
            let (titles, surname) = words.split_at(titles.min(words.len().saturating_sub(1)));
            [titles.join(" "), first.trim().to_string(), surname.join(" ")].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
        }
        _ => name.trim().to_string(),
    }
}

/// The view of the Termin `key` of `table` on `on` (its next date without), as it is at `now`,
/// measured `against` the plan. `seen`: the view's mode, and whether the Termin's module is in
/// the visitor's plan (`None`: not known).
#[allow(clippy::too_many_arguments)]
fn shown_of(table: &Timetable, against: &Against, key: RowKey, on: Option<Day>, now: Option<(Day, u16)>, modules: &BTreeMap<String, ModuleFacts>, seen: (TerminMode, Option<bool>), t: &'static Texts) -> Option<Shown> {
    let (mode, planned) = seen;
    // The plan the Termin is measured against, and the plan beside `table` where that is it.
    let (measure, beside) = against.plans(table);
    let locale = t.locale;
    let today = now.map(|(day, _)| day);
    let o = Occasion::at(table, key, on, now)?;
    let event = table.events.get(o.event)?;
    let first = event.rows.get(*o.rows.first()?)?;
    let module = event.modules.first();
    let kind = type_text(event, locale);
    let first_kind = event.kinds.iter().next();
    let title = match event.title.trim() {
        "" => module.and_then(|id| modules.get(id)).map(|facts| facts.title.clone()).unwrap_or_default(),
        title => title.to_string(),
    };
    let head = Head { out: mode == TerminMode::LookUp && planned == Some(false), tone: event.tone, chip: chip_of(module, modules), kind: kind.clone(), title };

    // The ticket.
    let past = now.is_some_and(|(day, minute)| o.day < day || (o.day == day && o.to.is_some_and(|to| to <= minute)));
    let live = now.and_then(|(day, minute)| {
        let (from, to) = (o.from?, o.to?);
        (o.day == day && from <= minute && minute < to && o.state.held()).then(|| (minute - from) * 100 / (to - from).max(1))
    });
    let pill = match &o.state {
        DateState::Cancelled(..) => Some(("no", t.termin.cancelled.to_string())),
        DateState::Skipped(why) => Some(("", why.text(locale).to_string())),
        DateState::Hidden => Some(("", t.termin.hidden.to_string())),
        _ if o.hidden.is_some() => Some(("", t.termin.hidden.to_string())),
        _ => now.map(|(day, minute)| match (live, past, o.day.0 - day.0) {
            (Some(_), _, _) => ("live", (t.termin.running)(&(t.termin.minutes)(o.to.unwrap_or(minute).saturating_sub(minute)))),
            (None, true, _) => ("", t.termin.past.to_string()),
            (None, false, 0) => ("soon", (t.termin.in_time)(&(t.termin.minutes)(o.from.unwrap_or(minute).saturating_sub(minute)))),
            (None, false, 1) => ("soon", t.termin.tomorrow.to_string()),
            (None, false, 2) => ("", t.termin.day_after.to_string()),
            (None, false, days) => ("", (t.termin.in_days)(days)),
        }),
    };
    let date_row = o.dates.iter().find(|dated| dated.day == o.day).and_then(|dated| event.rows.get(dated.row)).unwrap_or(first);
    let mut rooms: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for row in event.rows.iter().filter(|row| row.key.is_some() && row.key == date_row.key) {
        if let Some(room) = row.date.room_shown().map(str::to_string).filter(|room| !rooms.contains(room)) {
            rooms.push(room);
        }
        if let Some(name) = row.date.room.as_deref().map(str::trim).filter(|name| !name.is_empty() && Some(*name) != row.date.room_shown()).map(str::to_string) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    // The campus, unless QIS's name of the room says it already („… - Zentralcampus").
    let campus = date_row.date.campus.as_ref().map(|campus| campus.label(locale).to_string()).filter(|campus| {
        let first = campus.split_whitespace().next().unwrap_or_default();
        !names.iter().any(|name| name.contains(first))
    });
    let state = match (&o.state, past) {
        (DateState::Cancelled(..), _) => "cancel",
        (DateState::Skipped(_), _) => "off",
        (DateState::Note(_), false) => "note",
        (_, true) => "past",
        _ => "",
    };
    let ticket = Ticket {
        date: day_words(o.day, today, locale),
        prev: o.step(true),
        next: o.step(false),
        weeks: o.weeks,
        pill,
        time: o.from.zip(o.to).map(|(from, to)| (clock(from), (t.termin.until)(&clock(to)))),
        rooms,
        place: names.into_iter().chain(campus).collect(),
        state,
        live,
    };

    // Who leads it.
    let leads: Vec<String> = o.rows.iter().filter_map(|row| event.rows.get(*row)?.date.instructor.clone()).map(|name| full_name(&name)).fold(Vec::new(), |mut all, name| {
        if !name.is_empty() && !all.contains(&name) {
            all.push(name);
        }
        all
    });
    let responsible = module.and_then(|id| modules.get(id)).and_then(|facts| facts.responsible.clone()).filter(|name| !name.trim().is_empty());
    let people: Vec<Person> = match (leads.is_empty(), &responsible) {
        (false, _) => vec![Person { name: leads.join(" / "), role: (t.termin.leads)(first_kind).to_string() }],
        (true, Some(name)) => vec![Person { name: full_name(name), role: t.termin.responsible.to_string() }],
        (true, None) => Vec::new(),
    };

    // What else is said of it.
    let mut hints = Vec::new();
    if let Some((plan, met)) = measure.and_then(|plan| Some((plan, termin::clash_on(table, &o, beside)?))) {
        let other = plan.events.get(met.event);
        let what = other.map(|other| format!("{} {}, {}–{}", kind_short(other, locale), chip_of(other.modules.first(), modules), clock(met.from), clock(met.to))).unwrap_or_default();
        hints.push(Hint { warn: true, icon: "triangle-alert", text: (t.termin.meets)(&what) });
    }
    match &o.state {
        DateState::Cancelled(reason, instead) => {
            if !reason.trim().is_empty() {
                hints.push(Hint { warn: true, icon: "calendar-minus", text: reason.trim().to_string() });
            }
            if let Some(day) = instead {
                hints.push(Hint { warn: false, icon: "arrow-right", text: (t.termin.moved_to)(&day_words(*day, today, locale)) });
            }
        }
        DateState::Note(note) => hints.push(Hint { warn: false, icon: "map-pin", text: note.clone() }),
        _ => {}
    }
    if let Some(why) = o.hidden {
        let text = match why {
            HiddenBy::Event | HiddenBy::Row => t.termin.hidden_event.to_string(),
            HiddenBy::Choice => t.termin.other_group.to_string(),
            HiddenBy::Option => t.termin.other_option.to_string(),
            HiddenBy::Program => t.termin.other_programs.to_string(),
            HiddenBy::Kinds => (t.termin.kind_off)(&kind),
            HiddenBy::Town(_) | HiddenBy::Course(_) => t.termin.other_place.to_string(),
        };
        hints.push(Hint { warn: false, icon: "eye-off", text });
    }
    if let Some(comment) = first.date.comment.as_deref().map(str::trim).filter(|comment| !comment.is_empty()) {
        hints.push(Hint { warn: false, icon: "info", text: comment.to_string() });
    }

    // The groups, and „Im Stundenplan".
    let groups = groups_of(table, (measure, beside), &o, event, (first_kind, modules), seen, t);
    let plan = match mode {
        TerminMode::LookUp => None,
        TerminMode::Adjust => event.id.parse::<u32>().ok().map(|id| {
            let shown = event.hidden != Some(HiddenBy::Event);
            let sub = match (shown, matches!(event.attendance, Attendance::OneOf { .. })) {
                (false, _) => t.termin.no_day,
                (true, true) => t.termin.with_ticked,
                (true, false) => t.termin.every_day,
            };
            let hideable = matches!(o.state, DateState::Held | DateState::Note(_) | DateState::Hidden) && o.dates.iter().filter(|dated| dated.state.held()).count() > 1;
            let date = o.date_key(table).filter(|_| hideable).map(|key| (day_words(o.day, today, locale), key, o.day, o.state == DateState::Hidden));
            PlanPart { event: id, kind: kind.clone(), shown, sub: sub.to_string(), date }
        }),
    };

    // The day, where there is a plan to show it of.
    let the_day = (o.from.is_some() && measure.is_some()).then(|| day_part(table, beside, &o, (modules, head.out), t));

    // The series.
    let series = series_part(&o, today, t);

    // The period and what differs.
    let range = range_part(event, &o, today, t);

    // All events of the module.
    let events = module.map(|id| events_part(table, id, &o, t)).unwrap_or_default();

    // The module and its exam.
    let module_part = module.map(|id| {
        let facts = modules.get(id);
        let line = [facts.map(|facts| facts.title.clone()), Some(id.clone()), facts.and_then(|facts| facts.credits).map(|credits| format::credits(Some(credits), locale))].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        let exams = table
            .exams
            .iter()
            .filter(|exam| exam.modules.contains(id))
            .flat_map(|exam| exam.rows.iter().map(move |row| (exam, row)))
            .filter_map(|(_, row)| {
                let day = row.date.first_date.as_deref().and_then(Day::parse)?;
                let times = folia_timetable::occur::times(&row.date).map(|(from, to)| format!("{}–{}", clock(from), clock(to)));
                let when = [Some(weekday(day.weekday(), locale).to_string()), times, row.date.room_shown().map(str::to_string)].into_iter().flatten().collect::<Vec<_>>().join(" · ");
                let left = today.map(|today| match day.0 - today.0 {
                    days if days < 0 => t.termin.past.to_string(),
                    0 => t.termin.today.to_string(),
                    days => (t.termin.in_days)(days),
                });
                Some((day.day_month(locale), when, left))
            })
            .take(3)
            .collect();
        ModulePart { id: id.clone(), line, exams }
    });

    // Who teaches.
    let staff = staff_part(table, id_or_empty(module), responsible.as_deref(), t);

    let what = [Some(kind.clone()), event.number.clone()].into_iter().flatten().collect::<Vec<_>>().join(" ");
    let source = SourcePart { what, qis: event.source_url.clone().or_else(|| first.date.source_url.clone()) };
    // Its dates for one's own calendar, to look up a Termin outside the plan: the feed's entries of
    // a timetable that holds it alone.
    let file = (mode == TerminMode::LookUp && planned == Some(false)).then(|| {
        let titles: BTreeMap<String, String> = modules.iter().map(|(id, facts)| (id.clone(), facts.title.clone())).collect();
        let names: BTreeMap<String, String> = modules.iter().filter_map(|(id, facts)| Some((id.clone(), facts.abbrev.clone()?))).collect();
        let calendar = export::calendar_of(&termin::alone(table, &o), &titles, &names, "", "", locale);
        let name = format!("{} {}", head.chip, kind);
        (ics::write(&calendar), (t.termin.calendar_file)(name.trim()))
    });
    Some(Shown { key, day: o.day, head, ticket, people, hints, plan, groups, the_day, series, range, events, module: module_part, staff, source, file })
}

/// What a page of the Termin says of itself, on the date the view shows at `now`: the browser
/// tab's title („Praktikum · Programmierpraktikum · Mo 02.11.2026"), the description of a link
/// preview, and the day.
#[derive(Clone, Debug, PartialEq)]
pub struct PageHead {
    pub title: String,
    pub description: String,
    pub day: Day,
}

/// The `PageHead` of the Termin `key` on `on` (its next date at `now` without).
pub fn page_head(table: &Timetable, (key, on): (RowKey, Option<Day>), now: Option<(Day, u16)>, locale: Locale, words: &'static i18n::termin::Texts) -> Option<PageHead> {
    let o = Occasion::at(table, key, on, now)?;
    let event = table.events.get(o.event)?;
    let kind = type_text(event, locale);
    let title = match event.title.trim() {
        "" => event.modules.first().cloned().unwrap_or_default(),
        title => title.to_string(),
    };
    let weekday = locale.texts().weekday_short(i64::from(o.day.weekday())).unwrap_or_default();
    let when = format!("{weekday} {}", o.day.date(locale));
    let time = o.from.map(|from| format!(" {}", clock(from))).unwrap_or_default();
    let what = folia_timetable::views::kind_and_title(&kind, &title);
    Some(PageHead { title: (words.page_title)(&kind, &title, &when), description: (words.page_description)(&what, &format!("{when}{time}")), day: o.day })
}

fn id_or_empty(id: Option<&String>) -> &str {
    id.map_or("", String::as_str)
}

/// The groups of the event's choice, when it has one: „Übungsgruppen", how many ticked, the tiles.
fn groups_of(table: &Timetable, plans: (Option<&Timetable>, Option<&Timetable>), o: &Occasion, event: &Event, named: (Option<EventKind>, &BTreeMap<String, ModuleFacts>), seen: (TerminMode, Option<bool>), t: &'static Texts) -> Option<Groups> {
    let locale = t.locale;
    let (kind, modules) = named;
    let (mode, planned) = seen;
    let (plan, beside) = plans;
    let tiles = termin::tiles(table, o.event, beside);
    if tiles.len() < 2 {
        return None;
    }
    let ticked = tiles.iter().filter(|tile| tile.ticked).count();
    let open = ticked == 0;
    let own_option = o.rows.first().and_then(|row| event.rows.get(*row)).and_then(|row| row.option);
    let shown: Vec<TileShown> = tiles
        .iter()
        .filter_map(|tile| {
            let row = event.rows.get(tile.session.row)?;
            let when = match (row.slot_day(), row.from) {
                (Some(day), Some(from)) => format!("{} {}", weekday(day, locale), clock(from)),
                (Some(day), None) => weekday(day, locale).to_string(),
                _ => row.date.first_date.as_deref().and_then(Day::parse).map(|day| day.day_month(locale)).unwrap_or_default(),
            };
            let room = format!("{}{}", row.date.room_shown().unwrap_or_default(), if tile.session.rooms > 0 { " +1" } else { "" });
            let name = row.date.group_name.as_deref().map(str::trim).filter(|name| !name.is_empty() && *name != folia_timetable::model::UNNAMED_GROUP).map(group_word);
            let meets = plan.and_then(|plan| plan.events.get(tile.meets?)).map(|other| format!("{} {}", kind_short(other, locale), chip_of(other.modules.first(), modules)));
            // Green only where it helps to choose: nothing ticked yet, or to look up; neither
            // without a plan to fit.
            let dot = match (&meets, open || mode == TerminMode::LookUp) {
                _ if plan.is_none() => None,
                (Some(_), _) => Some(false),
                (None, true) => Some(true),
                (None, false) => None,
            };
            let said = match (&meets, dot) {
                (Some(what), _) => (t.termin.meets_said)(what),
                (None, Some(true)) => t.termin.fits_said.to_string(),
                _ => String::new(),
            };
            let at = if room.is_empty() { String::new() } else { format!(", {room}") };
            let label = format!("{}{when}{at}{said}{}", name.as_ref().map(|name| format!("{name}, ")).unwrap_or_default(), if tile.ticked { t.termin.ticked_said } else { "" });
            let gives = meets.is_some() && event.visit;
            Some(TileShown { option: tile.option, key: tile.session.key, keys: tile.keys.clone(), when, room, name, ticked: tile.ticked, current: Some(tile.option) == own_option, dot, gives, label })
        })
        .collect();
    let right = match (mode, open) {
        (TerminMode::LookUp, _) => (t.termin.one_of)(shown.len()),
        (TerminMode::Adjust, true) => t.termin.none_yet.to_string(),
        (TerminMode::Adjust, false) => (t.termin.ticked_of)(ticked, shown.len()),
    };
    let note = (mode == TerminMode::Adjust && open && event.hidden.is_none()).then(|| (t.termin.open_note)(shown.len()));
    let foot = (mode == TerminMode::LookUp && planned == Some(true)).then(|| (t.termin.in_your_plan)(ticked));
    Some(Groups { word: (t.termin.groups)(kind).to_string(), right, tiles: shown, note, foot })
}

/// QIS's „2-Gruppe" as one reads it: „Gruppe 2".
fn group_word(name: &str) -> String {
    match name.strip_suffix("-Gruppe") {
        Some(number) if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) => format!("Gruppe {number}"),
        _ => name.to_string(),
    }
}

/// „Der Tag": the plan's Termine of the day in lanes, and what comes after this one. `beside`: the
/// plan, where `table` is not it; the Termin then stands among the plan's, hatched where its
/// module is `out` of the plan.
fn day_part(table: &Timetable, beside: Option<&Timetable>, o: &Occasion, named: (&BTreeMap<String, ModuleFacts>, bool), t: &'static Texts) -> DayPart {
    let locale = t.locale;
    let (modules, out) = named;
    let plan = beside.unwrap_or(table);
    // Each Termin with whether it is this one, which stands in `table`; the others in `plan`.
    let mine = |other: &termin::Beside| beside.is_none() && other.event == o.event && (other.key == Some(o.key) || o.rows.contains(&other.row));
    let mut besides: Vec<(bool, termin::Beside)> = termin::day_of(plan, o.day).into_iter().map(|other| (mine(&other), other)).collect();
    if !besides.iter().any(|(me, _)| *me) {
        if let (Some(from), Some(to), Some(row)) = (o.from, o.to, o.rows.first()) {
            besides.push((true, termin::Beside { event: o.event, row: *row, key: Some(o.key), from, to, cancelled: matches!(o.state, DateState::Cancelled(..)), open: false }));
            besides.sort_by_key(|(me, other)| (other.from, other.to, !*me, other.event));
        }
    }
    let mut lanes: Vec<u16> = Vec::new();
    let items = besides
        .iter()
        .map(|(me, beside)| {
            let lane = match lanes.iter().position(|end| *end <= beside.from) {
                Some(lane) => lane,
                None => {
                    lanes.push(0);
                    lanes.len() - 1
                }
            };
            if let Some(end) = lanes.get_mut(lane) {
                *end = beside.to;
            }
            let event = if *me { table } else { plan }.events.get(beside.event);
            let chip = chip_of(event.and_then(|event| event.modules.first()), modules);
            let room = event.and_then(|event| event.rows.get(beside.row)).and_then(|row| row.date.room_shown()).unwrap_or_default();
            let kind = event.map(|event| kind_short(event, locale)).unwrap_or_default();
            RibbonItem {
                key: beside.key,
                from: beside.from,
                to: beside.to,
                lane,
                short: chip.clone(),
                label: format!("{}–{} · {kind} {chip} · {room}", clock(beside.from), clock(beside.to)),
                tone: event.map_or(1, |event| event.tone),
                me: *me,
                gone: beside.cancelled,
                open: beside.open,
                out: *me && out,
            }
        })
        .collect();
    let then = match o.state {
        DateState::Cancelled(..) | DateState::Skipped(_) => None,
        _ => Some(match termin::after(table, o, beside) {
            Some(next) => {
                let event = plan.events.get(next.event);
                let row = event.and_then(|event| event.rows.get(next.row));
                let room = row.and_then(|row| row.date.room_shown()).unwrap_or_default();
                let what = format!("{} {} {} · {room}", clock(next.from), event.map(|event| kind_short(event, locale)).unwrap_or_default(), chip_of(event.and_then(|event| event.modules.first()), modules));
                let gap = o.to.map(|to| (t.termin.pause)(&(t.termin.minutes)(next.from.saturating_sub(to))));
                (what, gap)
            }
            None => (String::new(), None),
        }),
    };
    DayPart { items, lanes: lanes.len(), then }
}

/// „Reihe": a dot per date, and where this one stands among them.
fn series_part(o: &Occasion, today: Option<Day>, t: &'static Texts) -> Option<SeriesPart> {
    let locale = t.locale;
    if o.dates.len() < 2 {
        return None;
    }
    let (nth, count, left) = o.place(today);
    let right = match nth {
        Some(nth) => format!("{} · {}", (t.termin.nth_of)(nth, count), (t.termin.left)(left)),
        None => format!("{} · {}", (t.termin.dates)(count), (t.termin.left)(left)),
    };
    let dots = o
        .dates
        .iter()
        .map(|dated| {
            let gone = today.is_some_and(|today| dated.day < today);
            let (state, said) = match &dated.state {
                DateState::Cancelled(..) => ("cancel", Some(t.termin.falls_out.to_string())),
                DateState::Skipped(why) => ("off", Some(why.text(locale).to_string())),
                DateState::Hidden => ("hid", Some(t.termin.hidden.to_string())),
                DateState::Note(note) => ("note", Some(note.clone())),
                DateState::Held if gone => ("past", Some(t.termin.dot_past.to_string())),
                DateState::Held => ("", None),
            };
            let class = ["sd", state, if today == Some(dated.day) { "today" } else { "" }, if dated.day == o.day { "sel" } else { "" }].into_iter().filter(|class| !class.is_empty()).collect::<Vec<_>>().join(" ");
            let label = format!("{}{}", day_words(dated.day, today, locale), said.map(|said| format!(" · {said}")).unwrap_or_default());
            Dot { day: dated.day, class, label, link: dated.state.steppable() }
        })
        .collect();
    Some(SeriesPart { right, dots })
}

/// „Zeitraum und Ausnahmen".
fn range_part(event: &Event, o: &Occasion, today: Option<Day>, t: &'static Texts) -> RangePart {
    let locale = t.locale;
    let first = o.rows.first().and_then(|row| event.rows.get(*row));
    let rhythm = match first.and_then(|row| row.every) {
        Some(Every::Week) => t.termin.weekly.to_string(),
        Some(Every::AWeek) => (t.termin.fortnightly)("A"),
        Some(Every::BWeek) => (t.termin.fortnightly)("B"),
        Some(Every::ThreeWeeks) => (t.termin.every_weeks)(3),
        Some(Every::FourWeeks) => (t.termin.every_weeks)(4),
        None => match first.and_then(|row| row.date.rhythm.as_ref()).and_then(|rhythm| rhythm.known()) {
            Some(folia_model::labels::Rhythm::Block) => t.termin.block.to_string(),
            _ => t.termin.single.to_string(),
        },
    };
    let when = first.and_then(|row| Some(format!("{} {}–{}", weekday(row.slot_day()?, locale), clock(row.from?), clock(row.to?))));
    let held: Vec<Day> = o.dates.iter().filter(|dated| dated.state.held()).map(|dated| dated.day).collect();
    let span = held.first().zip(held.last()).map(|(a, b)| if a == b { day_words(*a, today, locale) } else { format!("{}–{}", a.day_month(locale), b.date(locale)) });
    let head = [Some(rhythm), when, span].into_iter().flatten().collect::<Vec<_>>().join(", ") + &format!(" · {}", (t.termin.dates)(held.len()));
    let mut lines: Vec<(String, String, &'static str)> = Vec::new();
    for dated in &o.dates {
        match &dated.state {
            DateState::Note(note) => lines.push((dated.day.day_month(locale), note.clone(), "note")),
            DateState::Cancelled(reason, _) => {
                let text = match reason.trim() {
                    "" => t.termin.falls_out.to_string(),
                    reason => format!("{}: {reason}", t.termin.falls_out),
                };
                lines.push((dated.day.day_month(locale), text, "cancel"));
            }
            DateState::Hidden => lines.push((dated.day.day_month(locale), t.termin.hidden.to_string(), "hid")),
            _ => {}
        }
    }
    // The dates of the break and the holidays, each reason once with its days.
    let mut off: Vec<(String, Vec<Day>)> = Vec::new();
    for dated in &o.dates {
        if let DateState::Skipped(why) = &dated.state {
            let why = why.text(locale).to_string();
            match off.iter_mut().find(|(known, _)| *known == why) {
                Some((_, days)) => days.push(dated.day),
                None => off.push((why, vec![dated.day])),
            }
        }
    }
    for (why, days) in off {
        lines.push((days.iter().map(|day| day.day_month(locale)).collect::<Vec<_>>().join(" "), why, "off"));
    }
    let assumed = first.is_some_and(|row| row.occ.assumed);
    RangePart { head, assumed, lines }
}

/// „Alle Veranstaltungen" of the module `id`, in the timetable's order of its events.
fn events_part(table: &Timetable, id: &str, o: &Occasion, t: &'static Texts) -> Vec<EventPart> {
    let locale = t.locale;
    table
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| event.modules.iter().any(|module| module == id))
        .filter_map(|(index, event)| {
            let sessions: Vec<SessionShown> = termin::sessions(event)
                .into_iter()
                .filter_map(|session| {
                    let row = event.rows.get(session.row)?;
                    let when = match (row.every, row.slot_day(), row.from) {
                        (Some(every), Some(day), Some(from)) => {
                            let ab = match every {
                                Every::AWeek => " · A",
                                Every::BWeek => " · B",
                                _ => "",
                            };
                            format!("{} {}{ab}", weekday(day, locale), clock(from))
                        }
                        _ => row.occ.days.first().or(row.occ.hidden.first()).map(|day| format!("{} {}", weekday(day.weekday(), locale), day.day_month(locale))).unwrap_or_default(),
                    };
                    let room = format!("{}{}", row.date.room_shown().unwrap_or_default(), if session.rooms > 0 { " +1" } else { "" });
                    let me = index == o.event && (session.key == o.key || o.rows.contains(&session.row));
                    let ticked = row.option.is_some_and(|option| event.chosen.contains(&option));
                    let label = format!("{} {when}, {room}", type_text(event, locale));
                    Some(SessionShown { key: session.key, when, room, me, ticked, label })
                })
                .collect();
            if sessions.is_empty() {
                return None;
            }
            let one_of = match &event.attendance {
                Attendance::OneOf { options, .. } if options.len() > 1 => Some((t.termin.one_of)(options.len())),
                _ => None,
            };
            Some(EventPart { kind: type_text(event, locale), one_of, tone: event.tone, sessions })
        })
        .collect()
}

/// „Lehrende": the module's responsible, then who leads which Termin („leitet die Übung Mo 11:30
/// und Mi 11:30").
fn staff_part(table: &Timetable, id: &str, responsible: Option<&str>, t: &'static Texts) -> Vec<Person> {
    let locale = t.locale;
    let mut out: Vec<Person> = responsible.map(|name| Person { name: full_name(name), role: t.termin.responsible.to_string() }).into_iter().collect();
    let mut led: Vec<(String, Option<EventKind>, Vec<String>)> = Vec::new();
    for event in table.events.iter().filter(|event| event.modules.iter().any(|module| module == id)) {
        for session in termin::sessions(event) {
            let Some(row) = event.rows.get(session.row) else { continue };
            let Some(name) = row.date.instructor.as_deref().map(full_name).filter(|name| !name.is_empty()) else { continue };
            let when = match (row.slot_day(), row.from) {
                (Some(day), Some(from)) => format!("{} {}", weekday(day, locale), clock(from)),
                _ => continue,
            };
            match led.iter_mut().find(|(known, ..)| *known == name) {
                Some((_, _, whens)) if !whens.contains(&when) => whens.push(when),
                Some(_) => {}
                None => led.push((name, event.kinds.iter().next(), vec![when])),
            }
        }
    }
    for (name, kind, whens) in led {
        if out.iter().any(|person| person.name == name) {
            continue;
        }
        let and = if locale == Locale::De { " und " } else { " and " };
        out.push(Person { name, role: format!("{} {}", (t.termin.leads)(kind), whens.join(and)) });
    }
    out
}

#[cfg(test)]
mod tests {
    use folia_calendar::select::Selection;
    use folia_timetable::model::tests::{d, table, teaching};

    use super::*;

    fn modules() -> BTreeMap<String, ModuleFacts> {
        [("12104".to_string(), ModuleFacts { title: "Entwicklung von Softwaresystemen".into(), abbrev: Some("EvS".into()), credits: Some(6.0), responsible: Some("Köhler, Ekkehard".into()) })].into_iter().collect()
    }

    #[test]
    fn a_date_reads_when_where_and_what_it_meets() {
        let mut lecture = teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45");
        lecture.0.date.room_short = Some("ZHG/HS.C".into());
        lecture.0.date.instructor = Some("Köhler, Ekkehard".into());
        lecture.0.cancelled_dates = Some("17.11.2026: Krankheit".into());
        let rows = vec![
            lecture,
            teaching("12104", "200", 1, "Übung", 1, "11:30", "13:00").group("1-Gruppe"),
            teaching("12104", "200", 2, "Übung", 2, "09:15", "10:45").group("2-Gruppe"),
        ];
        let key = rows[0].key();
        let t = table(&rows, &["12104"], &[], &Selection { chosen_rows: [rows[2].key()].into_iter().collect(), ..Selection::default() });
        let texts = &i18n::DE;
        let shown = shown_of(&t, &Against::Itself, key, Some(d("2026-11-10")), Some((d("2026-11-10"), 9 * 60 + 35)), &modules(), (TerminMode::Adjust, Some(true)), texts).unwrap();
        assert_eq!((shown.head.chip.as_str(), shown.head.kind.as_str()), ("EvS", "Vorlesung"));
        assert_eq!(shown.ticket.date, "Di 10.11.");
        assert_eq!(shown.ticket.time, Some(("09:15".to_string(), "bis 10:45".to_string())));
        assert_eq!(shown.ticket.rooms, ["ZHG/HS.C"]);
        assert_eq!(shown.ticket.pill, Some(("live", "läuft · noch 1 h 10 min".to_string())));
        assert_eq!(shown.people, [Person { name: "Ekkehard Köhler".into(), role: "hält die Vorlesung".into() }]);
        // The ticked group 2 meets the lecture.
        assert!(shown.hints.first().is_some_and(|hint| hint.warn && hint.text.starts_with("Überschneidet sich mit Ü EvS, 09:15")), "{:?}", shown.hints);
        // A week on: QIS cancels it, with its reason; the series counts it as none held.
        let cancelled = shown_of(&t, &Against::Itself, key, Some(d("2026-11-17")), Some((d("2026-11-10"), 600)), &modules(), (TerminMode::Adjust, Some(true)), texts).unwrap();
        assert_eq!(cancelled.ticket.pill, Some(("no", "Fällt aus".to_string())));
        assert!(cancelled.hints.iter().any(|hint| hint.text == "Krankheit"));
        assert!(cancelled.range.lines.iter().any(|(day, text, class)| day == "17.11." && text == "fällt aus: Krankheit" && *class == "cancel"));
        // The lecture has no groups; its event can be hidden, the date too.
        assert!(shown.groups.is_none());
        let plan = shown.plan.unwrap();
        assert!(plan.shown && plan.date.is_some_and(|(words, ..)| words == "Di 10.11."));
    }

    #[test]
    fn the_groups_are_tiles_to_tick() {
        let rows = vec![
            teaching("12104", "100", 1, "Vorlesung", 2, "09:15", "10:45"),
            teaching("12104", "200", 1, "Übung", 1, "11:30", "13:00").group("1-Gruppe"),
            teaching("12104", "200", 2, "Übung", 2, "09:15", "10:45").group("2-Gruppe"),
            teaching("12104", "200", 3, "Übung", 3, "11:30", "13:00").group("3-Gruppe"),
        ];
        let open = table(&rows, &["12104"], &[], &Selection::default());
        let texts = &i18n::DE;
        let shown = shown_of(&open, &Against::Itself, rows[1].key(), None, Some((d("2026-11-04"), 600)), &modules(), (TerminMode::Adjust, Some(true)), texts).unwrap();
        let groups = shown.groups.unwrap();
        assert_eq!((groups.word.as_str(), groups.right.as_str()), ("Übungsgruppen", "noch keine"));
        assert_eq!(groups.note.as_deref(), Some("Bis du eine anhakst, stehen alle 3 gestrichelt im Stundenplan."));
        assert_eq!(groups.tiles.iter().map(|tile| (tile.when.as_str(), tile.name.as_deref(), tile.dot, tile.current)).collect::<Vec<_>>(), [
            ("Mo 11:30", Some("Gruppe 1"), Some(true), true),
            ("Di 09:15", Some("Gruppe 2"), Some(false), false),
            ("Mi 11:30", Some("Gruppe 3"), Some(true), false),
        ]);
        assert!(groups.tiles[1].label.contains("überschneidet sich mit VL EvS"), "{}", groups.tiles[1].label);
        // Two ticked: „2 von 3", no green where nothing is left to help choose.
        let ticked = table(&rows, &["12104"], &[], &Selection { chosen_rows: [rows[1].key(), rows[3].key()].into_iter().collect(), ..Selection::default() });
        let groups = shown_of(&ticked, &Against::Itself, rows[1].key(), None, None, &modules(), (TerminMode::Adjust, Some(true)), texts).unwrap().groups.unwrap();
        assert_eq!(groups.right, "2 von 3");
        assert_eq!(groups.tiles.iter().map(|tile| (tile.ticked, tile.dot)).collect::<Vec<_>>(), [(true, None), (false, Some(false)), (true, None)]);
        assert_eq!(groups.note, None);
        // To look up a module in the plan: „eine von 3", how many the plan holds, no file.
        let look = shown_of(&ticked, &Against::Itself, rows[1].key(), None, None, &modules(), (TerminMode::LookUp, Some(true)), texts).unwrap();
        let groups = look.groups.unwrap();
        assert_eq!((groups.right.as_str(), groups.foot.as_deref()), ("eine von 3", Some("2 in deinem Stundenplan")));
        assert!(look.plan.is_none() && look.file.is_none() && !look.head.out);
        // One outside the plan: its chip dashed, „nicht im Plan", its dates as a file of their own;
        // without a plan to fit no group is green or red, and there is no day.
        let outside = shown_of(&open, &Against::Nothing, rows[1].key(), None, None, &modules(), (TerminMode::LookUp, Some(false)), texts).unwrap();
        assert!(outside.head.out && outside.groups.as_ref().is_some_and(|groups| groups.foot.is_none()));
        assert!(outside.groups.as_ref().is_some_and(|groups| groups.tiles.iter().all(|tile| tile.dot.is_none())));
        assert!(outside.the_day.is_none() && !outside.hints.iter().any(|hint| hint.warn));
        let (text, name) = outside.file.unwrap();
        assert_eq!(name, "Termin EvS Übung.ics");
        assert!(text.starts_with("BEGIN:VCALENDAR") && text.matches("BEGIN:VEVENT").count() == 1, "{text}");
        // Beside a plan that holds a lecture on Tuesdays at 09:15 and an exercise on Mondays at
        // 13:45: group 2 meets the plan, the others fit; Monday's group stands hatched among the
        // plan's day, the exercise after it.
        let theirs = [teaching("12107", "300", 1, "Vorlesung", 2, "09:15", "10:45"), teaching("12107", "400", 1, "Übung", 1, "13:45", "15:15")];
        let plan = Against::Plan(Box::new(table(&theirs, &["12107"], &[], &Selection::default())));
        let beside = shown_of(&open, &plan, rows[1].key(), Some(d("2026-11-09")), None, &modules(), (TerminMode::LookUp, Some(false)), texts).unwrap();
        assert_eq!(beside.groups.unwrap().tiles.iter().map(|tile| tile.dot).collect::<Vec<_>>(), [Some(true), Some(false), Some(true)]);
        let day = beside.the_day.unwrap();
        assert_eq!(day.items.iter().map(|item| (item.from, item.me, item.out)).collect::<Vec<_>>(), [(690, true, true), (825, false, false)]);
        assert!(day.then.is_some_and(|(what, _)| what.starts_with("13:45 Ü")));
        // Group 2's date meets the plan's lecture: a line of its own.
        let two = shown_of(&open, &plan, rows[2].key(), Some(d("2026-11-10")), None, &modules(), (TerminMode::LookUp, Some(false)), texts).unwrap();
        assert!(two.hints.first().is_some_and(|hint| hint.warn && hint.text.starts_with("Überschneidet sich mit VL")), "{:?}", two.hints);
    }

    #[test]
    fn a_long_grid_folds_to_five_with_its_own() {
        let tiles: Vec<usize> = (0..9).collect();
        assert_eq!(folded(&tiles, |tile| *tile == 7, |_| false), [0, 1, 2, 3, 7]);
        assert_eq!(folded(&tiles, |tile| *tile == 2, |_| false), [0, 1, 2, 3, 4]);
        // Those that meet the plan last.
        assert_eq!(folded(&tiles, |tile| *tile == 0, |tile| *tile < 3), [3, 4, 5, 6, 0]);
    }

    #[test]
    fn names_read_as_one_says_them() {
        assert_eq!(full_name("Köhler, Ekkehard"), "Ekkehard Köhler");
        assert_eq!(full_name("Prof. Dr. rer. nat. habil. Hofstedt, Petra"), "Prof. Dr. rer. nat. habil. Petra Hofstedt");
        assert_eq!(full_name("Köhler"), "Köhler");
        assert_eq!(full_name("Bleicher / Freymann"), "Bleicher / Freymann");
        assert_eq!(group_word("2-Gruppe"), "Gruppe 2");
        assert_eq!(group_word("Gruppe A"), "Gruppe A");
    }
}
