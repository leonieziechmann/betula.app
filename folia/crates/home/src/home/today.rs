//! The start page's day (concept A „Heute"; folia/design/startseite/konzepte.html, „Empfehlung"):
//! „Der Tag zuerst: Gruß und Datum, die Semesteruhr, „Als Nächstes" mit Raum, darunter heute als
//! Liste, daneben die Woche, die Prüfungen, das Studium, die Merkliste. Abends rückt „Morgen" nach
//! vorn, in der Prüfungszeit die Prüfungen, in den Ferien … das nächste Semester."
//!
//! The picture is 300 px high on a wide screen at every hour, in every phase and step (owner,
//! 2026-10-20: „Beim PC darf sich das hero banner nicht verschieben"): the sentence about the day
//! keeps to two lines and names modules by their few letters, the card beside it is as high as the
//! picture. On a phone the photo reaches up under the search; what runs under the search on the way
//! down goes soft under the frost every page has there, and the crown's leaves slide down from
//! above with the scroll as the photo leaves the bar (owner, 2026-10-26; app.css `.frost`,
//! enhance.js `--leaves`). On a wide screen the page runs under the bar as well, and the frost lies
//! behind the bar once it is scrolled (app.css „one scroll area").
//!
//! Everything is the visitor's and the browser app's (R9, R20): the Stundenplan's timetable of the
//! current semester (`StudyplanAsk`, the same as the Stundenplan's, so the two never disagree),
//! „Mein Studium"'s progress, the Merkliste. The page reads the clock once a half minute.

use std::collections::BTreeMap;

use folia_calendar::day::{self, Day};
use folia_calendar::rowkey::RowKey;
use folia_calendar::semester::{fachsemester, SemesterKey};
use folia_model::rows::CatalogRow;
use folia_pages::ask::{MetaAsk, PlanSourceAsk, StudyModulesAsk, StudyplanAsk, StudyplanModulesAsk};
use folia_pages::{HomeData, PlanSource, StudyplanData};
use folia_plans::study;
use folia_plans::studyplan::{self as stored, MineDoc, PlanDoc};
use folia_routes::url::{self, PlanView, StudyplanUrl};
use folia_timetable::clash::Weeks;
use folia_timetable::facts::SemesterFacts;
use folia_timetable::model::Timetable;
use folia_timetable::views::AgendaWeek;
use folia_widgets::termin::{module_facts, Against, TerminCtx, TerminLinks, TerminMode, TerminView};
use leptos::prelude::*;

use folia_data::use_ask;
use folia_design::format;
use folia_design::ui::Icon;
use folia_shell::pending::same_page;
use folia_shell::tabs::Tabs;
use folia_stores::bookmarks::Bookmarks;
use folia_stores::myprogram::MyProgram;
use folia_stores::studyplan::Studyplan;

use super::clock::{self, Clock, Kind, Phase};
use super::day::{self as daymodel, Entry, Greeting, State, Today};
use super::photo;
use super::setup::{self, Counted, Done};
use crate::i18n::{self, Texts};

/// How many marked modules the Merkliste's panel names.
const MARKS_SHOWN: usize = 3;

/// The hours the week's columns span, from 07:00 to 20:00.
const WEEK_FROM: u16 = 7 * 60;
const WEEK_TO: u16 = 20 * 60;

/// The browser's day and the minute of it; on the server none (it renders no day, R9).
pub fn clock_now() -> Option<(Day, u16)> {
    #[cfg(feature = "csr")]
    {
        let now = web_sys::js_sys::Date::new_0();
        let day = Day::from_ymd(i32::try_from(now.get_full_year()).ok()?, now.get_month() + 1, now.get_date())?;
        Some((day, u16::try_from(now.get_hours() * 60 + now.get_minutes()).ok()?))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The modules' names: in full and in few letters (`StudyplanData::titles`, `slot_names`).
pub(super) type Names = (BTreeMap<String, String>, BTreeMap<String, String>);

/// What the day's parts share.
#[derive(Clone, Copy)]
pub(super) struct Ctx {
    pub(super) now: RwSignal<(Day, u16)>,
    /// The current semester.
    pub(super) key: Memo<Option<SemesterKey>>,
    /// Its planned modules.
    pub(super) planned: Memo<Vec<String>>,
    pub(super) table: Memo<Option<Timetable>>,
    /// The table's agenda, the days with what they hold.
    pub(super) agenda: Memo<Vec<AgendaWeek>>,
    #[cfg_attr(not(feature = "csr"), allow(dead_code))]
    pub(super) names: Memo<Names>,
    pub(super) facts: Memo<Option<SemesterFacts>>,
    /// „Mein Studium"'s numbers.
    pub(super) progress: Memo<Option<Progress>>,
    pub(super) today: Memo<Today>,
    pub(super) clock: Memo<Option<Clock>>,
    pub(super) phase: Memo<Phase>,
    /// Without a study set up („Erst mal umschauen").
    pub(super) guest: bool,
}

impl Ctx {
    /// The same inputs at the minute `now` holds: the day, the clock and the phase of it
    /// (`ahead` works out the minutes to come this way).
    #[cfg_attr(not(feature = "csr"), allow(dead_code))]
    pub(super) fn at(self, now: RwSignal<(Day, u16)>) -> Ctx {
        let (today, clock, phase) = minute_parts(now, self.table, self.agenda, self.names, self.facts);
        Ctx { now, today, clock, phase, ..self }
    }
}

/// The day, the clock and the phase at the minute `now` holds.
fn minute_parts(now: RwSignal<(Day, u16)>, table: Memo<Option<Timetable>>, agenda: Memo<Vec<AgendaWeek>>, names: Memo<Names>, facts: Memo<Option<SemesterFacts>>) -> (Memo<Today>, Memo<Option<Clock>>, Memo<Phase>) {
    let t = i18n::t();
    let today = Memo::new(move |_| {
        let (day, minute) = now.get();
        let words = daymodel::Words { locale: t.locale, exam: t.today.exam };
        table.with(|table| {
            let Some(table) = table else { return Today::default() };
            agenda.with(|agenda| names.with(|(titles, names)| daymodel::today(table, agenda, titles, names, day, minute, &words)))
        })
    });
    let clock = Memo::new(move |_| {
        let (day, minute) = now.get();
        facts.with(|facts| facts.as_ref().and_then(|facts| today.with(|today| clock::clock(facts, day, minute, &today.exam_days))))
    });
    let phase = Memo::new(move |_| {
        let (day, _) = now.get();
        clock.with(|clock| clock.as_ref().map(|clock| clock.phase)).or_else(|| facts.with(|facts| facts.as_ref().map(|facts| clock::phase(facts, day)))).unwrap_or(Phase::Lectures)
    });
    (today, clock, phase)
}

/// The day. `guest`: no study is set up, and the page says what setting one up brings
/// (`on_setup` opens the setup again). `welcome` is the note after the setup, `home` the counts of
/// the ways into the catalog.
#[component]
pub fn TodayView(guest: bool, welcome: RwSignal<Option<Done>>, on_setup: Callback<()>, home: Signal<Option<HomeData>>) -> impl IntoView {
    let t = i18n::t();
    let (start_day, start_minute) = clock_now().unwrap_or((Day(0), 12 * 60));
    let now = RwSignal::new((start_day, start_minute));
    // The clock, read again every half minute while the page is built.
    #[cfg(feature = "csr")]
    {
        if let Ok(handle) = set_interval_with_handle(
            move || {
                if let Some(read) = clock_now() {
                    if now.try_get_untracked().is_some_and(|before| before != read) {
                        now.try_set(read);
                    }
                }
            },
            std::time::Duration::from_secs(30),
        ) {
            on_cleanup(move || handle.clear());
        }
    }
    let plan = Studyplan::expect();
    let mine = MyProgram::expect();
    let meta = use_ask(|| Some(MetaAsk {}));
    let key = Memo::new(move |_| meta.with(|meta| meta.as_ref().and_then(|meta| meta.as_ref().ok()).and_then(|meta| meta.current_semester.as_deref().and_then(SemesterKey::parse))));
    let planned = Memo::new(move |_| match (plan, key.get()) {
        (Some(plan), Some(key)) if !guest => plan.modules_in(key),
        _ => Vec::new(),
    });
    let program = Memo::new(move |_| plan.and_then(|plan| plan.with(|doc| doc.program.clone())).or_else(|| mine.and_then(|mine| mine.with(|doc| doc.program.clone()))));
    let asked = use_ask(move || key.get().map(|key| StudyplanAsk { key, ids: planned.get(), program: program.get(), locale: t.locale }));
    let data = Memo::new(move |_| {
        let key = key.get()?;
        asked.with(|answer| answer.as_ref().and_then(|answer| answer.as_ref().ok()).filter(|data| data.key == key).cloned())
    });
    let selection = Memo::new(move |_| {
        let key = key.get()?;
        let town = mine.map(MyProgram::town).unwrap_or_default();
        Some(plan.map_or_else(|| folia_calendar::select::Selection { town, ..Default::default() }, |plan| plan.selection(key, town)))
    });
    let table = Memo::new(move |_| {
        let selection = selection.get()?;
        data.with(|data| data.as_ref().map(|data| data.timetable(&selection)))
    });
    let facts = Memo::new(move |_| data.with(|data| data.as_ref().map(StudyplanData::facts)));
    let agenda = Memo::new(move |_| table.with(|table| table.as_ref().map(|table| table.agenda(t.locale)).unwrap_or_default()));
    let names: Memo<Names> = Memo::new(move |_| data.with(|data| data.as_ref().map(|data| (data.titles(), data.slot_names())).unwrap_or_default()));
    let progress = progress(key);
    let (today, clock, phase) = minute_parts(now, table, agenda, names, facts);
    let ctx = Ctx { now, key, planned, table, agenda, names, facts, progress, today, clock, phase, guest };
    // What the photo shows over the next days, kept for the next start of the page (`ahead`).
    #[cfg(feature = "csr")]
    if !guest {
        super::ahead::keep(ctx);
    }

    view! {
        <Hero ctx on_setup/>
        <Welcome welcome/>
        {(!guest).then(|| view! { <TerminOver table data plan now/> })}
        {move || {
            let phase = ctx.phase.get();
            let evening = now.get().1 >= 17 * 60;
            let day_over = ctx.today.with(|today| today.coming().count() == 0 && today.running().is_none());
            let has_exams = ctx.today.with(|today| !today.exams.is_empty());
            if guest {
                return view! {
                    <div class="day-cols">
                        <div class="day-col"><MarksPanel/></div>
                        <div class="day-col"><GuestPanel on_setup/></div>
                    </div>
                }.into_any();
            }
            let main = match phase {
                Phase::Exams => view! { <ExamsPanel ctx big=true/><DayPanel ctx/> }.into_any(),
                Phase::Break => view! { <DayPanel ctx/><NextDayPanel ctx/> }.into_any(),
                Phase::Lectures if day_over => view! { <NextDayPanel ctx/><DayPanel ctx/> }.into_any(),
                Phase::Lectures if evening => view! { <DayPanel ctx/><NextDayPanel ctx/> }.into_any(),
                _ => view! { <DayPanel ctx/> }.into_any(),
            };
            let side = match phase {
                Phase::Exams | Phase::Over => view! { <StudyPanel ctx/><MarksPanel/> }.into_any(),
                _ => view! { <WeekPanel ctx/>{has_exams.then(|| view! { <ExamsPanel ctx big=false/> })}<StudyPanel ctx/><MarksPanel/> }.into_any(),
            };
            view! {
                <div class="day-cols">
                    <div class="day-col day-main">{main}</div>
                    <div class="day-col">{side}</div>
                </div>
            }.into_any()
        }}
        <Discover home/>
    }
}

/// The photo with the day on it: the date, the greeting, the day in a sentence, the semester's
/// clock, and the card of what comes next. A function of the minute `ctx.now` holds, so that
/// `ahead` can write it for the minutes to come as the page shows it now. What goes by the minute
/// is marked for it (`data-count`): the sentence, the card's minutes, the clock's running week.
#[component]
pub(super) fn Hero(ctx: Ctx, on_setup: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let now = ctx.now;
    // The photo of the season, fog while the exams run, its shade by the hour.
    let fog = Memo::new(move |_| ctx.phase.get() == Phase::Exams);
    let light = Memo::new(move |_| {
        let (day, minute) = now.get();
        photo::light(day, minute)
    });
    view! {
        // Its buttons are commands of „Befehle" (R25) under „Dein Tag", as the note of a study set up.
        <section class=move || format!("day-hero {}{}", light.get().class(), if fog.get() { " fog" } else { "" }) aria-label=t.today.your_day data-commands=t.today.your_day>
            <div class="day-photo" class:fog=move || fog.get() aria-hidden="true"></div>
            <div class="day-shade" aria-hidden="true"></div>
            <div class="day-greet">
                <p class="day-eyebrow">{move || long_date(now.get().0, t)}</p>
                <h1>{move || format!("{}.", greeting_text(now.get().1, t))}</h1>
                <p class="day-line" data-count="line">{move || sentence(ctx, t)}</p>
                <ClockView ctx/>
            </div>
            <NextCard ctx on_setup/>
        </section>
    }
}

/// The colour of a module of the plan by its tone (1–8), as the Stundenplan gives it
/// (`folia_planner::studyplan::head::hue`): the classes that set `--tint`.
fn hue(tone: u8) -> &'static str {
    const HUES: [&str; 8] = ["t-ice", "t-sun", "t-violet", "t-teal", "t-green", "t-coral", "t-rose", "t-slate"];
    HUES.get(usize::from(tone.saturating_sub(1)) % HUES.len()).copied().unwrap_or("t-ice")
}

/// „Dienstag, 20. Oktober · KW 43".
fn long_date(day: Day, t: &Texts) -> String {
    let (_, month, date) = day.ymd();
    let weekday = t.data.weekdays.get(usize::from(day.weekday().saturating_sub(1))).copied().unwrap_or_default();
    let month = t.today.months.get(usize::try_from(month.saturating_sub(1)).unwrap_or(0)).copied().unwrap_or_default();
    (t.today.long_date)(weekday, date, month, day.iso_week().1)
}

fn greeting_text(minute: u16, t: &Texts) -> &'static str {
    match daymodel::greeting(minute) {
        Greeting::Morning => t.today.good_morning,
        Greeting::Day => t.today.good_day,
        Greeting::Evening => t.today.good_evening,
        Greeting::Night => t.today.good_night,
    }
}

/// The Termin of `entry` in its own view over the start page, on its day: the app's path in the
/// page's language. `None` for an exam.
fn termin_href(entry: &Entry, t: &Texts) -> Option<String> {
    entry.key.map(|key| t.path(&url::home_termin_path(Some((key, Some(entry.day))))))
}

/// A Termin of the plan in its own view over the start page (`/?termin=<key>&on=<day>`; owner,
/// 2026-10-26, folia/design/termin): the view that adjusts the plan, as from the Stundenplan,
/// beside the page where it is wide and a sheet over it on a phone. „Zum Modul" leads to the
/// module beside the Stundenplan; closing goes back a step where the start page without it is the
/// step before.
#[component]
fn TerminOver(table: Memo<Option<Timetable>>, data: Memo<Option<StudyplanData>>, plan: Option<Studyplan>, now: RwSignal<(Day, u16)>) -> impl IntoView {
    let t = i18n::t();
    let location = i18n::use_location();
    let asked = Memo::new(move |_| location.search.with(|search| url::home_termin(search)));
    let shown = Memo::new(move |_| asked.with(Option::is_some));
    let modules = Memo::new(move |_| data.with(|data| data.as_ref().map(module_facts).unwrap_or_default()));
    let tabs = Tabs::expect();
    let close = Signal::derive(move || t.path(url::HOME));
    let back = Signal::derive(move || {
        let now = asked.with(|asked| url::home_termin_path(*asked));
        tabs.is_some_and(|tabs| same_page(&tabs.before(&now), url::HOME)).then_some("history")
    });
    let termin = Callback::new(move |(key, on): (RowKey, Option<Day>)| t.path(&url::home_termin_path(Some((key, on)))));
    let module = Callback::new(move |(id, key): (String, Option<RowKey>)| t.path(&StudyplanUrl::default().with_open(Some(&id), key).path()));
    let links = TerminLinks { termin, module, close, back, adjust: None };
    // The page's own clock, read every half minute (`TodayView`).
    let now = Signal::derive(move || Some(now.get()));
    let ctx = TerminCtx { table, asked, modules, mode: TerminMode::Adjust, plan, now, planned: Signal::stored(Some(true)), against: Signal::stored(Against::Itself), links, page: false };
    move || shown.get().then(|| view! { <div class="day-termin"><TerminView ctx/></div> })
}

/// „Mo 21.12.".
fn weekday_date(day: Day, t: &Texts) -> String {
    let weekday = t.data.weekdays_short.get(usize::from(day.weekday().saturating_sub(1))).copied().unwrap_or_default();
    (t.today.weekday_date)(weekday, &day.day_month(t.locale))
}

/// „Morgen" for the day after `today`, else „Am Donnerstag", and „Am 04.01." a week or more
/// ahead, where the weekday alone would name a day of this week.
fn next_day_word(today: Day, next: Day, t: &Texts) -> String {
    if next.0 == today.0 + 1 {
        t.today.tomorrow.to_string()
    } else {
        (t.today.on_weekday)(&day_name(today, next, t))
    }
}

/// „Donnerstag" for a day of the coming week, „04.01." for one further ahead.
fn day_name(today: Day, day: Day, t: &Texts) -> String {
    if day.0 - today.0 < 7 {
        t.data.weekdays.get(usize::from(day.weekday().saturating_sub(1))).copied().unwrap_or_default().to_string()
    } else {
        day.day_month(t.locale)
    }
}

/// „VL EvS": an entry's kind and module in few letters.
fn short_of(entry: &Entry) -> String {
    format!("{} {}", entry.kind_short, entry.short).trim().to_string()
}

fn time(minute: Option<u16>) -> String {
    minute.map(day::clock).unwrap_or_default()
}

/// „Gerade läuft VL EvS in ZHG/HS.C, noch 20 Minuten.": `entry` runs, `left` minutes to go.
pub(super) fn running_line(entry: &Entry, left: u16, t: &Texts) -> String {
    (t.today.running)(&entry.kind_short, &entry.short, entry.room.as_deref(), left)
}

/// What the card says of the minutes of `entry` at `minute`: „noch 1 h 05 min" while it runs,
/// „in 25 min" before it.
pub(super) fn card_minutes(entry: &Entry, running: bool, minute: u16, t: &Texts) -> String {
    match running {
        true => (t.today.minutes_left)(entry.to.map_or(0, |to| to.saturating_sub(minute))),
        false => entry.from.map(|from| (t.today.in_minutes)(from.saturating_sub(minute))).unwrap_or_default(),
    }
}

/// What the card shows of the day itself, where it does: the Termin that runs, else the next one,
/// and whether it runs. Not for a visitor without a study, nor with an empty plan, nor outside the
/// lectures.
pub(super) fn focus(ctx: Ctx) -> Option<(Entry, bool)> {
    if ctx.guest || (ctx.key.get().is_some() && ctx.planned.with(Vec::is_empty)) || ctx.phase.get() != Phase::Lectures {
        return None;
    }
    ctx.today.with(|today| today.running().or_else(|| today.next()).map(|entry| (entry.clone(), entry.state == State::Now)))
}

/// The sentence about the day under the greeting: two lines at most.
pub(super) fn sentence(ctx: Ctx, t: &Texts) -> String {
    let tt = &t.today;
    if ctx.guest {
        return tt.guest.to_string();
    }
    let (day, minute) = ctx.now.get();
    if ctx.key.get().is_some() && ctx.planned.with(Vec::is_empty) {
        return (tt.empty_plan)(&ctx.key.get().map(|key| key.label(t.locale)).unwrap_or_default());
    }
    let phase = ctx.phase.get();
    let clock = ctx.clock.get();
    ctx.today.with(|today| {
        let then = |done: &str| match &today.next_day {
            Some((next, entries)) => {
                let firm: Vec<&Entry> = entries.iter().filter(|entry| entry.firm()).collect();
                match firm.first() {
                    Some(first) => format!("{done} {}", (tt.then_day)(&next_day_word(day, *next, t), firm.len(), &time(first.from), first.room.as_deref())),
                    None => done.to_string(),
                }
            }
            None => done.to_string(),
        };
        match phase {
            Phase::Lectures => {
                if day.weekday() >= 6 && today.firm().count() == 0 {
                    return then(tt.weekend);
                }
                if let Some(running) = today.running() {
                    return running_line(running, running.to.map_or(0, |to| to.saturating_sub(minute)), t);
                }
                if let Some(next) = today.next() {
                    let past = today.firm().any(|entry| entry.state == State::Past);
                    let coming = today.coming().count();
                    return if past { (tt.still_today)(coming, &time(next.from), next.room.as_deref()) } else { (tt.first_today)(coming, &time(next.from), next.room.as_deref()) };
                }
                if today.firm().count() == 0 {
                    then(tt.nothing_today)
                } else {
                    then(tt.day_done)
                }
            }
            Phase::Break => {
                let until = clock.as_ref().and_then(|clock| clock.pause).map(|(_, last, _)| last.day_month(t.locale)).unwrap_or_default();
                let back = today.next_day.as_ref().and_then(|(next, entries)| entries.iter().find(|entry| entry.firm()).map(|first| (tt.back_on)(&weekday_date(*next, t), &time(first.from))));
                match back {
                    Some(back) => format!("{} {back}", (tt.pause_until)(&until)),
                    None => (tt.pause_until)(&until),
                }
            }
            Phase::Exams => match today.exams.first() {
                Some(exam) => {
                    let when = (tt.days_upper)(exam.day.0 - day.0);
                    let at = exam.from.map(day::clock);
                    format!("{} {}", tt.exam_time, (tt.next_exam)(&when, &exam.short, &weekday_date(exam.day, t), at.as_deref()))
                }
                None if !today.exam_days.is_empty() => format!("{} {}", tt.exam_time, tt.exams_written),
                None => format!("{} {}", tt.exam_time, tt.no_exams),
            },
            Phase::Before => {
                let start = clock.as_ref().map(|clock| clock.lectures.0).or_else(|| ctx.facts.with(|facts| facts.as_ref().and_then(|facts| facts.lecture.map(|(first, _)| first))));
                start.map_or_else(|| tt.weekend.to_string(), |start| (tt.lectures_begin)(&weekday_date(start, t)))
            }
            Phase::Over => tt.holidays.to_string(),
        }
    })
}

/// The semester clock: the weeks of the lectures, the running one wider, the exams one bar.
#[component]
fn ClockView(ctx: Ctx) -> impl IntoView {
    let t = i18n::t();
    move || {
        let clock = ctx.clock.get()?;
        let key = ctx.key.get();
        let (left, right) = caption(&clock, key, t);
        let label = (t.today.clock_label)(&left, &right);
        let pieces = clock
            .pieces
            .iter()
            .map(|piece| {
                let kind = match piece.kind {
                    Kind::Lecture => "lecture",
                    Kind::Pause => "pause",
                    Kind::ExamWeek => "exw",
                    Kind::Exams => "ex",
                };
                let mut title = (t.today.piece)(piece.from.iso_week().1, &piece.from.day_month(t.locale), &piece.to.day_month(t.locale));
                let extra = match piece.kind {
                    Kind::Pause => Some(t.today.piece_pause),
                    Kind::ExamWeek | Kind::Exams => Some(t.today.piece_exams),
                    Kind::Lecture => None,
                };
                if let Some(extra) = extra {
                    title = format!("{title} · {extra}");
                }
                if piece.exam {
                    title = format!("{title} · {}", t.today.piece_exam);
                }
                let class = format!("{kind}{}{}{}", if piece.done { " done" } else { "" }, if piece.fill.is_some() { " cur" } else { "" }, if piece.exam { " dot" } else { "" });
                let style = piece.fill.map(|fill| format!("--fill:{:.1}%", fill * 100.0));
                // The running week fills by the minute (`ahead`).
                let count = piece.fill.map(|_| "fill");
                view! { <i class=class style=style title=title data-count=count></i> }
            })
            .collect_view();
        Some(view! {
            <div class="day-clock" role="img" aria-label=label>
                <div class="day-clock-row">{pieces}</div>
                <div class="day-clock-cap"><span>{left}</span><span>{right}</span></div>
            </div>
        })
    }
}

/// What the clock says under it, left and right.
fn caption(clock: &Clock, key: Option<SemesterKey>, t: &Texts) -> (String, String) {
    let tt = &t.today;
    let dm = |day: Day| day.day_month(t.locale);
    let exams = || match (clock.exams.first(), clock.exams.last()) {
        (Some(first), Some(last)) if first == last => (tt.exam_on)(&dm(*first)),
        (Some(first), Some(last)) => (tt.exams_span)(&dm(*first), &dm(*last)),
        _ => (tt.lectures_until)(&dm(clock.lectures.1.plus(-2))),
    };
    match clock.phase {
        Phase::Lectures => {
            let mut left = (tt.week_of)(clock.week.max(1), clock.weeks);
            if let Some(ab) = clock.ab {
                let letter = match ab {
                    Weeks::A => "A",
                    Weeks::B => "B",
                    Weeks::All => "",
                };
                if !letter.is_empty() {
                    left = format!("{left} · {}", (tt.ab_week)(letter));
                }
            }
            (left, exams())
        }
        Phase::Break => {
            let (left, done) = match clock.pause {
                Some((_, last, true)) => ((tt.christmas_until)(&dm(last)), clock.week),
                Some((_, last, false)) => ((tt.free_until)(&dm(last)), clock.week),
                None => (tt.exam_period.to_string(), clock.week),
            };
            (left, (tt.weeks_done)(done, clock.weeks))
        }
        Phase::Exams => {
            let right = match (clock.exams_left.len(), clock.exams.last()) {
                (0, Some(_)) => tt.all_written.to_string(),
                (0, None) => (tt.semester_until)(&dm(clock.end)),
                (n, last) => (tt.exams_left)(n, &last.map(|last| dm(*last)).unwrap_or_default()),
            };
            (tt.exam_period.to_string(), right)
        }
        Phase::Before => ((tt.lectures_from)(&dm(clock.lectures.0)), (tt.lecture_weeks)(clock.weeks)),
        Phase::Over => {
            let left = key.map(|key| (tt.semester_done)(&key.label(t.locale))).unwrap_or_default();
            let right = key.and_then(|key| key.plus(1)).map(|next| (tt.next_from)(&next.label(t.locale), &dm(next.bounds().0))).unwrap_or_default();
            (left, right)
        }
    }
}

/// The card beside the greeting: what comes next, as the phase and the hour have it.
#[component]
fn NextCard(ctx: Ctx, on_setup: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    move || {
        let (day, minute) = ctx.now.get();
        if ctx.guest {
            return view! {
                <div class="day-next">
                    <span class="k"><span>{tt.your_study}</span></span>
                    <span class="what">{tt.not_set_up}</span>
                    <span class="acts"><button type="button" class="btn primary sm" on:click=move |_| on_setup.run(()) data-command=""><Icon name="graduation-cap"/>{tt.set_up_study}</button></span>
                </div>
            }
            .into_any();
        }
        if ctx.key.get().is_some() && ctx.planned.with(Vec::is_empty) {
            let import = StudyplanUrl { import: Some("mine".to_string()), ..Default::default() }.path();
            return view! {
                <div class="day-next">
                    <span class="k"><span>{tt.timetable}</span><span>{ctx.key.get().map(|key| key.label(t.locale))}</span></span>
                    <span class="what">{tt.plan_empty_card}</span>
                    <span class="acts"><a class="btn primary sm" href=t.path(&import) rel="nofollow" data-command=""><Icon name="calendar-plus"/>{tt.take_semester}</a></span>
                </div>
            }
            .into_any();
        }
        let phase = ctx.phase.get();
        let focus = focus(ctx);
        let module_link = |entry: &Entry| entry.module.clone().map(|id| t.path(&StudyplanUrl { open: Some(id), ..Default::default() }.path()));
        // The Termin in its own view over the page, else (an exam) its module beside the plan. Two
        // links of two types: erased, so that what ships builds as well (without `erase_components`).
        let link = |entry: &Entry| match termin_href(entry, t) {
            Some(href) => Some(view! { <a class="btn sm" href=href data-noscroll="" data-command="">{tt.termin}</a> }.into_any()),
            None => module_link(entry).map(|href| view! { <a class="btn sm" href=href data-command="">{tt.module}</a> }.into_any()),
        };
        let card = ctx.today.with(|today| {
            if let Some((entry, running)) = &focus {
                let running = *running;
                let after = if running { today.next() } else { today.coming().nth(1) };
                let why = card_minutes(entry, running, minute, t);
                let (head, when) = match running {
                    true => (tt.now, entry.room.clone().unwrap_or_else(|| time(entry.from))),
                    false => (tt.next, time(entry.from)),
                };
                let place = if running { (tt.until)(&time(entry.to)) } else { entry.room.clone().unwrap_or_default() };
                let clash = today.at_once(entry).map(|other| (tt.at_once)(&short_of(other)));
                return Some(view! {
                    <div class="day-next">
                        <span class="k"><span>{head}</span><span>{format!("{}–{}", time(entry.from), time(entry.to))}</span></span>
                        <span class="when"><b>{when}</b><span data-count="card">{why}</span></span>
                        <span class="what">{format!("{} · {}", entry.kind, entry.title)}</span>
                        <span class="where"><Icon name="map-pin"/><span>{place}{clash.map(|clash| format!(" · {clash}"))}</span></span>
                        {after.map(|after| view! { <span class="where dim">{(tt.after)(&time(after.from), &short_of(after), after.room.as_deref())}</span> })}
                        <span class="acts">
                            {link(entry)}
                            <a class="btn sm" href=t.path(url::STUDYPLAN) data-command="">{tt.timetable}</a>
                        </span>
                    </div>
                }
                .into_any());
            }
            if matches!(phase, Phase::Lectures | Phase::Before) || (phase == Phase::Break && today.exams.first().is_none_or(|exam| today.next_day.as_ref().is_some_and(|(next, _)| *next < exam.day))) {
                if let Some((next, entries)) = &today.next_day {
                    let firm: Vec<&Entry> = entries.iter().filter(|entry| entry.firm()).collect();
                    if let Some(first) = firm.first().copied().or_else(|| entries.first()) {
                        let last = firm.last().copied().unwrap_or(first);
                        return Some(view! {
                            <div class="day-next">
                                <span class="k"><span>{next_day_word(day, *next, t)}</span><span>{if next.0 - day.0 < 7 { weekday_date(*next, t) } else { (tt.days)(next.0 - day.0) }}</span></span>
                                <span class="when"><b>{time(first.from)}</b><span>{first.room.clone().unwrap_or_default()}</span></span>
                                <span class="what">{format!("{} · {}", first.kind, first.title)}</span>
                                <span class="where"><Icon name="clock-3"/><span>{(tt.termine_until)(firm.len().max(1), &time(last.to))}</span></span>
                                <span class="acts"><a class="btn sm" href=t.path(url::STUDYPLAN) data-command="">{tt.timetable}</a></span>
                            </div>
                        }
                        .into_any());
                    }
                }
            }
            if let Some(exam) = today.exams.first().filter(|_| phase != Phase::Over) {
                let days = exam.day.0 - day.0;
                let exams_href = t.path(&StudyplanUrl { view: PlanView::Exams, ..Default::default() }.path());
                return Some(view! {
                    <div class="day-next">
                        <span class="k"><span>{tt.next_exam_head}</span><span>{weekday_date(exam.day, t)}</span></span>
                        <span class="when">{match days { 0 => view! { <b>{tt.today_word}</b> }.into_any(), n => view! { <b>{n}</b><span>{(tt.day_word)(n)}</span> }.into_any() }}</span>
                        <span class="what">{exam.title.clone()}</span>
                        <span class="where"><Icon name="map-pin"/><span>{[exam.from.zip(exam.to).map(|(from, to)| format!("{}–{}", day::clock(from), day::clock(to))), exam.room.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · ")}</span></span>
                        <span class="acts">
                            {module_link(exam).map(|href| view! { <a class="btn sm" href=href data-command="">{tt.module}</a> })}
                            <a class="btn sm" href=exams_href data-command="">{tt.all_exams}</a>
                        </span>
                    </div>
                }
                .into_any());
            }
            None
        });
        card.unwrap_or_else(|| view! { <StudyCard ctx/> }.into_any())
    }
}

/// The card of the study, where the day has nothing to come: what is passed of it, and the way to
/// enter results.
#[component]
fn StudyCard(ctx: Ctx) -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    let progress = ctx.progress;
    move || {
        let (passed, total, fs) = progress.with(|progress| progress.as_ref().map_or((0.0, 0.0, None), |p| (p.passed, p.total, p.fs)));
        view! {
            <div class="day-next">
                <span class="k"><span>{tt.your_study}</span><span>{fs.map(|fs| (tt.after_fs)(fs))}</span></span>
                <span class="when"><b>{format::number(passed, t.locale)}</b><span>{(tt.of_credits)(&format::number(total, t.locale))}</span></span>
                <span class="what">{if passed > 0.0 { String::new() } else { tt.nothing_passed.to_string() }}</span>
                <span class="acts"><a class="btn sm" href=t.path(url::STUDY) data-command="">{tt.enter_results}</a></span>
            </div>
        }
    }
}

/// The study's progress as „Mein Studium" works it out.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Progress {
    program: String,
    fs: Option<u8>,
    passed: f64,
    planned: f64,
    total: f64,
}

/// „Mein Studium"'s numbers: the program's plans, the plan, and the catalog's rows of what is
/// planned and passed beyond the plan, in the semester `key`.
fn progress(key: Memo<Option<SemesterKey>>) -> Memo<Option<Progress>> {
    let t = i18n::t();
    let plan = Studyplan::expect();
    let mine = MyProgram::expect();
    let program = Memo::new(move |_| mine.and_then(|mine| mine.with(|doc| doc.program.clone())));
    let source = use_ask(move || program.get().map(|id| PlanSourceAsk { program_id: id, locale: t.locale }));
    let ids = Memo::new(move |_| {
        let mut ids: Vec<String> = plan.map(|plan| plan.with(|doc| doc.modules.iter().map(|m| m.module_id.clone()).chain(doc.passed.iter().map(|p| p.module_id.clone())).collect())).unwrap_or_default();
        ids.sort();
        ids.dedup();
        ids
    });
    let rows = use_ask(move || {
        let ids = ids.get();
        (!ids.is_empty()).then_some(StudyModulesAsk { ids })
    });
    Memo::new(move |_| {
        let now = key.get()?;
        let kept: MineDoc = mine.map(|mine| mine.get()).unwrap_or_default();
        let doc: PlanDoc = plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
        source.with(|source| {
            let source: &PlanSource = source.as_ref()?.as_ref().ok()?.as_ref()?;
            let extra: Vec<CatalogRow> = rows.with(|rows| rows.as_ref().and_then(|rows| rows.as_ref().ok()).map(|rows| rows.rows.clone()).unwrap_or_default());
            let all: Vec<CatalogRow> = source.linked.iter().cloned().chain(extra.into_iter().filter(|row| !source.linked.iter().any(|linked| linked.id == row.id))).collect();
            let index = folia_stores::myprogram::ProgramPlans::new(&source.variants, source.supplements.clone()).shown_index(kept.caption.as_deref().unwrap_or_default(), kept.direction.as_deref()).unwrap_or(0);
            let (core, page) = setup::chosen(source, index);
            let intake = core.and_then(|core| stored::intake_season(core, &source.linked));
            let start = kept.start.unwrap_or_else(|| stored::intake_start(now, intake));
            let input = study::Input { program_id: &source.program.id, core, page, start, now, leave: &kept.leave, until: kept.until, doc: &doc, rows: &all, tree: &source.areas, placements: &source.placements, fues: &source.fues };
            let study = study::study(&input);
            Some(Progress {
                program: format!("{} {}", source.program.name, source.program.degree()),
                fs: study::fs_of(start, now, &kept.leave).or_else(|| fachsemester(now, start)),
                passed: study.passed,
                planned: study.planned,
                total: study.total,
            })
        })
    })
}

/// „Eingerichtet: …" after „Studium anlegen", with „Rückgängig", until it is closed or the page is
/// left.
#[component]
fn Welcome(welcome: RwSignal<Option<Done>>) -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    let undo = move |_| {
        let Some(done) = welcome.get_untracked() else { return };
        if let Some(plan) = Studyplan::expect() {
            let before = done.plan_before.clone();
            plan.update_after_paint(move |doc| *doc = before);
        }
        if let Some(mine) = MyProgram::expect() {
            mine.clear_program();
            mine.set_start(done.start_before);
        }
        welcome.set(None);
    };
    move || {
        welcome.get().map(|done| {
            let passed = done.passed.map(|credits| format::number(credits, t.locale));
            view! {
                <div class="day-welcome" role="status" data-commands=tt.your_day>
                    <span class="day-welcome-ok" aria-hidden="true"><Icon name="check"/></span>
                    <div>
                        <b>{(tt.set_up)(&done.program, done.fs, &done.start.label(t.locale))}</b>
                        <small>{(tt.set_up_detail)(done.modules, done.placeholders, passed.as_deref())}" "{tt.stays_here}</small>
                    </div>
                    <span class="day-welcome-acts">
                        <a class="btn sm" href=t.path(url::STUDYPLAN) data-command=""><Icon name="calendar-plus"/>{tt.to_timetable}</a>
                        <button type="button" class="btn sm ghost" on:click=undo data-command=""><Icon name="rotate-ccw"/>{tt.undo}</button>
                    </span>
                    <button type="button" class="icon-btn day-welcome-x" aria-label=tt.close on:click=move |_| welcome.set(None)><Icon name="x"/></button>
                </div>
            }
        })
    }
}

/// One Termin of a list: its time, its module's colour, what and where, and how it stands.
fn entry_row(entry: &Entry, minute: Option<u16>, others: &Today, t: &Texts) -> impl IntoView {
    let tt = &t.today;
    let chip = match (entry.cancelled.is_some(), entry.open, entry.state, minute) {
        (true, ..) => Some(("warn", tt.cancelled.to_string())),
        (_, true, _, _) => Some(("warn", tt.group_open.to_string())),
        (_, _, State::Now, Some(minute)) => Some(("acc", (tt.running_chip)(entry.to.map_or(0, |to| to.saturating_sub(minute))))),
        (_, _, State::Next, Some(minute)) => entry.from.map(|from| ("ink", (tt.in_minutes)(from.saturating_sub(minute)))),
        (_, _, State::Past, Some(_)) => Some(("", tt.past.to_string())),
        _ => None,
    };
    let clash = (entry.firm() && minute.is_some()).then(|| others.at_once(entry).map(|other| format!("{} {}", tt.overlaps, short_of(other)))).flatten();
    let href = termin_href(entry, t).or_else(|| entry.module.clone().map(|id| t.path(&StudyplanUrl { open: Some(id), ..Default::default() }.path()))).unwrap_or_else(|| t.path(url::STUDYPLAN));
    let state = match entry.state {
        State::Past if minute.is_some() => "past",
        State::Now if minute.is_some() => "now",
        State::Next if minute.is_some() => "next",
        _ => "",
    };
    view! {
        <a class=format!("day-ev {state} {}", hue(entry.tone)) class:cancelled=entry.cancelled.is_some() class:open=entry.open href=href data-noscroll="">
            <span class="day-ev-t num"><b>{time(entry.from)}</b><span>{time(entry.to)}</span></span>
            <span class="day-ev-bar" aria-hidden="true"></span>
            <span class="day-ev-w">
                <b>{format!("{} · {}", entry.kind, entry.title)}</b>
                <small>
                    {entry.room.clone().map(|room| view! { <Icon name="map-pin"/><span>{room}</span> })}
                    {entry.note.clone().map(|note| view! { <span class="day-ev-note">{format!(" · {note}")}</span> })}
                    {entry.cancelled.clone().filter(|reason| !reason.is_empty()).map(|reason| view! { <span>{format!(" · {reason}")}</span> })}
                </small>
            </span>
            <span class="day-ev-st">
                {clash.map(|clash| view! { <span class="day-chip warn"><Icon name="triangle-alert"/>{clash}</span> })}
                {chip.map(|(class, text)| view! { <span class=format!("day-chip {class}")>{text}</span> })}
            </span>
        </a>
    }
}

/// „Heute": the Termine of today in their order, a line at the hour.
#[component]
fn DayPanel(ctx: Ctx) -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    move || {
        let (day, minute) = ctx.now.get();
        let phase = ctx.phase.get();
        ctx.today.with(|today| {
            let firm = today.firm().count();
            let open = today.entries.iter().filter(|entry| entry.open).count();
            let mut sub = format!("{} · {}", weekday_date(day, t), (tt.count_termine)(firm));
            if open > 0 {
                sub = format!("{sub} · {}", (tt.open_groups)(open));
            }
            let body = if today.entries.is_empty() {
                let (head, text) = match phase {
                    Phase::Break => {
                        let pause = ctx.clock.with(|clock| clock.as_ref().and_then(|clock| clock.pause));
                        (tt.no_lectures.to_string(), pause.map(|(first, last, _)| (tt.pause_text)(&first.day_month(t.locale), &last.day_month(t.locale))).unwrap_or_default())
                    }
                    Phase::Exams => (tt.no_lectures.to_string(), ctx.clock.with(|clock| clock.as_ref().map(|clock| (tt.exams_text)(&clock.lectures.1.plus(-2).day_month(t.locale)))).unwrap_or_default()),
                    Phase::Over => (tt.holidays_head.to_string(), ctx.key.get().and_then(|key| key.plus(1)).map(|next| (tt.holidays_text)(&next.label(t.locale))).unwrap_or_default()),
                    Phase::Before => (tt.no_lectures.to_string(), ctx.clock.with(|clock| clock.as_ref().map(|clock| (tt.before_text)(&weekday_date(clock.lectures.0, t)))).unwrap_or_default()),
                    Phase::Lectures if day.weekday() >= 6 => (tt.weekend.trim_end_matches('.').to_string(), tt.weekend_text.to_string()),
                    Phase::Lectures => (tt.nothing_today.trim_end_matches('.').to_string(), tt.nothing_planned_text.to_string()),
                };
                view! { <div class="day-empty"><b>{head}</b><span>{text}</span></div> }.into_any()
            } else {
                // The line of the hour stands before the first Termin not over yet.
                let at = today.entries.iter().position(|entry| entry.state != State::Past).unwrap_or(today.entries.len());
                let rows = today.entries.iter().enumerate().map(|(i, entry)| {
                    let line = (i == at).then(|| view! { <div class="day-nowline num">{day::clock(minute)}</div> });
                    view! { {line}{entry_row(entry, Some(minute), today, t)} }
                }).collect_view();
                let tail = (at == today.entries.len()).then(|| view! { <div class="day-nowline num">{day::clock(minute)}</div> });
                view! { <div class="day-list">{rows}{tail}</div> }.into_any()
            };
            view! {
                <section class="panel day-panel" id="heute">
                    <header class="day-head"><h2>{tt.today}</h2><span class="sub">{sub}</span><a class="ghost" href=t.path(url::STUDYPLAN)>{tt.timetable}<Icon name="arrow-right"/></a></header>
                    {body}
                </section>
            }
        })
    }
}

/// The next day with Termine: „Morgen", „Am Donnerstag", „Ab Montag" after a break.
#[component]
fn NextDayPanel(ctx: Ctx) -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    move || {
        let (day, _) = ctx.now.get();
        let phase = ctx.phase.get();
        ctx.today.with(|today| {
            let (next, entries) = today.next_day.as_ref()?;
            let head = if phase == Phase::Break { (tt.from_weekday)(&day_name(day, *next, t)) } else { next_day_word(day, *next, t) };
            let firm = entries.iter().filter(|entry| entry.firm()).count();
            Some(view! {
                <section class="panel day-panel">
                    <header class="day-head"><h2>{head}</h2><span class="sub">{format!("{} · {}", weekday_date(*next, t), (tt.count_termine)(firm))}</span><a class="ghost" href=t.path(url::STUDYPLAN)>{tt.timetable}<Icon name="arrow-right"/></a></header>
                    <div class="day-list">{entries.iter().map(|entry| entry_row(entry, None, today, t)).collect_view()}</div>
                </section>
            })
        })
    }
}

/// „Diese Woche": its days side by side, each Termin a bar at its time in its module's colour.
#[component]
fn WeekPanel(ctx: Ctx) -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    move || {
        let (day, _) = ctx.now.get();
        let phase = ctx.phase.get();
        // In a break the week it goes on in.
        let shown = match phase {
            Phase::Break => ctx.today.with(|today| today.next_day.as_ref().map(|(next, _)| *next)).unwrap_or(day),
            _ => day,
        };
        let monday = shown.monday();
        let facts = ctx.facts.get();
        let ab = facts.as_ref().and_then(|facts| facts.ab_week(shown)).and_then(|weeks| match weeks {
            Weeks::A => Some((tt.ab_week)("A")),
            Weeks::B => Some((tt.ab_week)("B")),
            Weeks::All => None,
        });
        let names = (BTreeMap::new(), BTreeMap::new());
        let words = daymodel::Words { locale: t.locale, exam: tt.exam };
        let days: Vec<(Day, Vec<Entry>)> = ctx.table.with(|table| {
            ctx.agenda.with(|agenda| {
                let week: Vec<AgendaWeek> = agenda.iter().filter(|week| week.monday == monday).cloned().collect();
                (0..5)
                    .map(|offset| {
                        let on = monday.plus(offset);
                        let entries = table.as_ref().map(|table| daymodel::today(table, &week, &names.0, &names.1, on, 0, &words).entries).unwrap_or_default();
                        (on, entries)
                    })
                    .collect()
            })
        });
        let span = f64::from(WEEK_TO - WEEK_FROM);
        let cols = days.into_iter().map(|(on, entries)| {
            let firm = entries.iter().filter(|entry| entry.firm() && !entry.exam).count();
            let weekday = t.data.weekdays_short.get(usize::from(on.weekday().saturating_sub(1))).copied().unwrap_or_default();
            let bars = entries.iter().filter(|entry| entry.cancelled.is_none()).filter_map(|entry| {
                let (from, to) = (entry.from?, entry.to?);
                let top = (f64::from(from.clamp(WEEK_FROM, WEEK_TO) - WEEK_FROM) / span * 100.0).clamp(0.0, 100.0);
                let height = (f64::from(to.clamp(WEEK_FROM, WEEK_TO).saturating_sub(from.clamp(WEEK_FROM, WEEK_TO))) / span * 100.0).max(2.0);
                let class = format!("{}{}{}", hue(entry.tone), if entry.open { " open" } else { "" }, if entry.exam { " exam" } else { "" });
                let style = format!("top:{top:.1}%;height:{height:.1}%");
                let said = format!("{} {}–{} · {} · {}", weekday_date(entry.day, t), time(entry.from), time(entry.to), entry.kind, entry.title);
                // A Termin leads to its own view over the page; an exam is a mark.
                Some(match termin_href(entry, t) {
                    Some(href) => view! { <a class=class style=style href=href data-noscroll="" title=said.clone() aria-label=said></a> }.into_any(),
                    None => view! { <i class=class style=style title=said></i> }.into_any(),
                })
            }).collect_view();
            view! {
                <div class="day-wk-d" class:today=on == day>
                    <span><b>{weekday}</b>{(firm > 0).then_some(firm)}</span>
                    <div class="day-wk-col">{bars}</div>
                </div>
            }
        }).collect_view();
        let week_href = t.path(url::STUDYPLAN);
        view! {
            <section class="panel day-panel">
                <header class="day-head"><h2>{if monday == day.monday() { tt.this_week.to_string() } else { (tt.week_from)(&monday.day_month(t.locale)) }}</h2><span class="sub">{(tt.week_sub)(monday.iso_week().1, ab.as_deref())}</span><a class="ghost" href=week_href>{tt.week_link}<Icon name="arrow-right"/></a></header>
                <div class="day-wk">{cols}</div>
            </section>
        }
    }
}

/// The exams still to come, the next first.
#[component]
fn ExamsPanel(ctx: Ctx, big: bool) -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    move || {
        let (day, _) = ctx.now.get();
        let semester = ctx.key.get().map(|key| key.label(t.locale)).unwrap_or_default();
        ctx.today.with(|today| {
            let rows = today.exams.iter().map(|exam| {
                let (_, month, date) = exam.day.ymd();
                let month = t.data.months_short.get(usize::try_from(month.saturating_sub(1)).unwrap_or(0)).copied().unwrap_or_default();
                let days = exam.day.0 - day.0;
                let weekday = t.data.weekdays_short.get(usize::from(exam.day.weekday().saturating_sub(1))).copied().unwrap_or_default();
                let when = [Some(weekday.to_string()), exam.from.zip(exam.to).map(|(from, to)| format!("{}–{}", day::clock(from), day::clock(to)))].into_iter().flatten().collect::<Vec<_>>().join(" ");
                let place = [Some(when), exam.room.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · ");
                let href = exam.module.clone().map(|id| t.path(&StudyplanUrl { view: PlanView::Exams, open: Some(id), ..Default::default() }.path())).unwrap_or_default();
                view! {
                    <a class="day-ex" class:soon=days <= 7 href=href>
                        <span class="day-ex-d num"><b>{format!("{date:02}")}</b><small>{month}</small></span>
                        <span class="day-ex-w"><b>{exam.title.clone()}</b><small>{place}</small></span>
                        <span class="day-ex-in">{(tt.days)(days)}</span>
                    </a>
                }
            }).collect_view();
            let all = t.path(&StudyplanUrl { view: PlanView::Exams, ..Default::default() }.path());
            view! {
                <section class="panel day-panel">
                    <header class="day-head"><h2>{if big { tt.your_exams } else { tt.exams }}</h2><span class="sub">{(tt.exams_in)(today.exams.len(), &semester)}</span><a class="ghost" href=all>{tt.all}<Icon name="arrow-right"/></a></header>
                    <div class="day-exl">{rows}</div>
                </section>
            }
        })
    }
}

/// „Mein Studium": what is passed and planned of the plan, and what is open.
#[component]
fn StudyPanel(ctx: Ctx) -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    let progress = ctx.progress;
    move || {
        progress.get().map(|p| {
            let total = if p.total > 0.0 { p.total } else { (p.passed + p.planned).max(1.0) };
            let open = (p.total - p.passed - p.planned).max(0.0);
            let share = |value: f64| format!("width:{:.2}%", (value / total * 100.0).clamp(0.0, 100.0));
            view! {
                <section class="panel day-panel">
                    <header class="day-head"><h2>{tt.my_study}</h2><span class="sub">{(tt.study_sub)(&p.program, p.fs)}</span><a class="ghost" href=t.path(url::STUDY)>{tt.open}<Icon name="arrow-right"/></a></header>
                    <div class="day-prog">
                        <p class="day-prog-fig"><b class="num">{format::number(p.passed, t.locale)}</b><span>{(tt.passed_of)(&format::number(p.total, t.locale))}</span></p>
                        <div class="day-prog-bar" aria-hidden="true"><i class="passed" style=share(p.passed)></i><i class="planned" style=share(p.planned)></i></div>
                        <p class="day-prog-key">
                            <span><i class="passed"></i>{format!("{} {}", format::number(p.passed, t.locale), tt.passed)}</span>
                            <span><i class="planned"></i>{format!("{} {}", format::number(p.planned, t.locale), tt.planned)}</span>
                            <span><i class="open"></i>{format!("{} {}", format::number(open, t.locale), tt.left)}</span>
                        </p>
                    </div>
                </section>
            }
        })
    }
}

/// The Merkliste: the newest marked modules.
#[component]
fn MarksPanel() -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    let marks = Bookmarks::expect();
    let count = Memo::new(move |_| marks.map_or(0, Bookmarks::count));
    let ids = Memo::new(move |_| marks.map(|marks| marks.marks().into_iter().take(MARKS_SHOWN).map(|mark| mark.id).collect::<Vec<String>>()).unwrap_or_default());
    let rows = use_ask(move || {
        let ids = ids.get();
        (!ids.is_empty()).then_some(StudyplanModulesAsk { ids })
    });
    move || {
        let count = count.get();
        let found: Vec<CatalogRow> = rows.with(|rows| rows.as_ref().and_then(|rows| rows.as_ref().ok()).map(|(rows, _)| rows.clone()).unwrap_or_default());
        let listed = ids.get().into_iter().filter_map(|id| found.iter().find(|row| row.id == id).cloned()).map(|row| {
            let turnus = row.turnus_season.is_some().then(|| format::turnus(row.turnus_season.as_ref(), row.turnus_parity.as_ref(), t.locale));
            let facts = [Some(row.id.clone()), row.credits.map(|credits| format!("{} {}", format::number(credits, t.locale), tt.credits)), turnus].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            view! {
                <a class="day-bm" href=t.path(&url::module_path(&row.id))>
                    <Icon name="bookmark"/>
                    <span><b>{row.title.clone()}</b><small>{facts}</small></span>
                </a>
            }
        }).collect_view();
        view! {
            <section class="panel day-panel">
                <header class="day-head"><h2>{tt.bookmarks}</h2><span class="sub">{(tt.modules)(count)}</span><a class="ghost" href=t.path(url::BOOKMARKS)>{tt.all}<Icon name="arrow-right"/></a></header>
                {if count == 0 { view! { <p class="day-empty-line">{tt.no_bookmarks}</p> }.into_any() } else { view! { <div class="day-bml">{listed}</div> }.into_any() }}
            </section>
        }
    }
}

/// Without a study: what setting one up brings, and the way to it.
#[component]
fn GuestPanel(on_setup: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    view! {
        <section class="panel day-panel day-guest">
            <header class="day-head"><h2>{tt.your_study}</h2></header>
            <p class="day-empty-line">{tt.guest}</p>
            <p class="day-guest-acts"><button type="button" class="btn primary" on:click=move |_| on_setup.run(()) data-command=""><Icon name="graduation-cap"/>{tt.set_up_study}</button></p>
        </section>
    }
}

/// „Entdecken": ways into the catalog with how many modules each holds, and the programs.
#[component]
pub fn Discover(home: Signal<Option<HomeData>>, #[prop(optional)] first: bool) -> impl IntoView {
    let t = i18n::t();
    let tt = &t.today;
    let ways = super::ways(t);
    move || {
        let home = home.get();
        let counts = home.as_ref().map(|home| home.entry_counts.clone()).unwrap_or_default();
        let programs = home.as_ref().map(|home| home.overview.programs);
        let faculties = home.as_ref().map(|home| home.faculties.iter().filter(|(department, _)| department.as_ref().is_some_and(|d| !d.code.is_empty() && d.code.chars().all(|c| c.is_ascii_digit()))).map(|(department, _)| department.as_ref().map(|d| d.code.clone())).collect::<std::collections::BTreeSet<_>>().len());
        let season = home.as_ref().and_then(super::current_of).map(SemesterKey::season);
        let tiles = ways.iter().zip(counts.iter().map(Some).chain(std::iter::repeat(None))).filter(|(way, _)| way.season.is_none() || way.season == season).map(|(way, count)| {
            let href = t.path(&folia_routes::url::CatalogUrl { query: way.query.clone(), ..Default::default() }.path());
            view! {
                <a class=format!("day-way {}", way.tint) href=href rel="nofollow" data-command=way.label>
                    <span class="day-way-i"><Icon name=way.icon/></span>
                    <span class="day-way-t"><b>{way.label}</b><small>{way.hint}</small></span>
                    <span class="day-way-n num">{count.map(|count| format::count(*count, t.locale))}</span>
                </a>
            }
        }).collect_view();
        let (head, sub) = if first { (tt.without_setup, tt.without_setup_sub) } else { (tt.discover, tt.discover_sub) };
        view! {
            <section class="panel day-discover" id="entdecken" data-commands=head>
                <header class="day-head">
                    <h2>{head}</h2><span class="sub">{sub}</span>
                    {first.then(|| view! { <a class="ghost" href=t.path(url::ABOUT)>{tt.what_is_betula}<Icon name="arrow-right"/></a> })}
                </header>
                <div class="day-ways">
                    {tiles}
                    <a class="day-way t-sun" href=t.path(url::PROGRAMS) data-command=tt.all_programs>
                        <span class="day-way-i"><Icon name="graduation-cap"/></span>
                        <span class="day-way-t"><b>{tt.all_programs}</b><small>{faculties.map(|n| (tt.faculties)(n))}</small></span>
                        <span class="day-way-n num">{programs.map(|n| format::count(n, t.locale))}</span>
                    </a>
                </div>
            </section>
        }
    }
}

/// The faculties of the drawing as `HomeData` counts them: none until the answer is there.
pub fn faculties_of(home: Signal<Option<HomeData>>) -> Signal<Option<Counted>> {
    Signal::derive(move || home.get().map(|home| home.faculties))
}
