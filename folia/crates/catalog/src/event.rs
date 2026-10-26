//! A Termin's own page (`/event/<key>[/<day>]`; folia/design/termin, the fourth round; owner,
//! 2026-10-26: „Wir sollten das wahrscheinlich Trennen zwischen der Informativen ansicht für die
//! Modulpage und mein studiengang und dann der ansicht bei der Kalender page"): the Termin to look
//! up (`TerminMode::LookUp`), where a line of a module's Termine leads.
//!
//! Its data is the timetable of the modules that link the event, in the newest semester it is held
//! in (`EventAsk`): one answer for every Termin and date of the event. The server renders it the
//! same for everybody (R9): without the visitor's plan, so it says nothing of it (no „nicht im
//! Plan", no group green or red, no day around it), on the day the address names, else on the
//! Termin's first date (it reads no clock). The app shows it on the day named, else on the next,
//! and with the visitor's plan of the event's semester where they have one (the Stundenplan's own
//! `StudyplanAsk`): a module in it is read in the plan, its groups ticked as there, with
//! „Anpassen ›" into the Stundenplan of that semester (one place to adjust; owner, 2026-10-26, „Ja
//! zu allen drei Vorschlägen"); one outside it is measured against the plan, its groups green and
//! red, a clash a line of its own, the Termin hatched among the plan's day, and offers its dates for
//! the visitor's own calendar („In den Kalender"). An event no module links is a 404. Not in the
//! sitemap and `noindex`, its links to other dates `nofollow`: events times dates are many
//! addresses. A page of a while (`nav::passing`): it lies on the tab it was opened from, and Back
//! leaves it.

use std::collections::BTreeMap;

use folia_calendar::day::Day;
use folia_calendar::rowkey::RowKey;
use folia_calendar::select::Selection;
use folia_data::{use_data, DataError, PageStatus};
use folia_design::i18n::use_params_map;
use folia_design::stage::Title;
use folia_design::ui::NotFound;
use folia_pages::ask::{EventAsk, StudyplanAsk};
use folia_pages::StudyplanData;
use folia_routes::url::{self, StudyplanUrl};
use folia_shell::frame::{ErrorState, Plain};
use folia_shell::seo::Seo;
use folia_stores::myprogram::MyProgram;
use folia_stores::studyplan::Studyplan;
use folia_timetable::model::Timetable;
use folia_timetable::termin::Occasion;
use folia_timetable::views::type_text;
use folia_widgets::termin::{module_facts, ticking_clock, Against, TerminCtx, TerminLinks, TerminMode, TerminView};
use leptos::prelude::*;

use crate::i18n;

/// The browser app (`csr`), or the server rendering the page the same for everybody.
const APP: bool = cfg!(feature = "csr");

/// What the page has to show: an address of no Termin or an event no module links (404), an
/// error, or the Termin.
#[derive(Clone, Debug, PartialEq)]
enum Loaded {
    Missing,
    Failed(DataError),
    Found(Box<Found>),
}

/// The Termin and day asked for, with the timetable of the event's modules and the visitor's plan
/// of its semester where they have one (the app's alone, R9). They stay together while the answer
/// for another event is on its way, so the view never reads one Termin in another event's
/// timetable (R16).
#[derive(Clone, Debug, PartialEq)]
struct Found {
    key: RowKey,
    day: Option<Day>,
    event: StudyplanData,
    plan: Option<StudyplanData>,
}

impl Found {
    /// The Termin's module is in the visitor's plan of the event's semester.
    fn planned(&self, plan: Option<Studyplan>) -> bool {
        APP && plan.is_some_and(|plan| self.event.ids.iter().any(|id| plan.is_planned(self.event.key, id)))
    }

    /// The plan's timetable, as the Stundenplan shows it.
    fn plan_table(&self, plan: Option<Studyplan>, mine: Option<MyProgram>) -> Option<Timetable> {
        let (data, plan) = (self.plan.as_ref()?, plan?);
        Some(data.timetable(&plan.selection(self.event.key, mine.map(MyProgram::town).unwrap_or_default())))
    }
}

#[component]
pub fn EventPage() -> impl IntoView {
    let t = i18n::t();
    let params = use_params_map();
    let asked = Memo::new(move |_| {
        params.with(|params| {
            let key = RowKey::parse(&params.get("key")?.to_ascii_lowercase())?;
            // A day that is none is no day: the page shows the next date (the server its first).
            Some((key, params.get("day").and_then(|day| Day::parse(&day))))
        })
    });
    let client = use_data();
    let plan = Studyplan::expect();
    let mine = MyProgram::expect();
    let loaded = Memo::new(move |before: Option<&Option<Loaded>>| {
        let Some((key, day)) = asked.get() else { return Some(Loaded::Missing) };
        // On its way: what was shown stays, its own Termin with it.
        let kept = || before.cloned().flatten();
        let client = match client.as_ref() {
            Ok(client) => client,
            Err(error) => return Some(Loaded::Failed(error.clone())),
        };
        let event = match client.get(&EventAsk { event: key.event.to_string(), locale: t.locale }) {
            None => return kept(),
            Some(Ok(Some(event))) => event,
            Some(Ok(None)) => return Some(Loaded::Missing),
            Some(Err(error)) => return Some(Loaded::Failed(error)),
        };
        // The visitor's plan of the event's semester, asked as the Stundenplan asks it.
        let ids = match (APP, plan) {
            (true, Some(plan)) => plan.modules_in(event.key),
            _ => Vec::new(),
        };
        if ids.is_empty() {
            return Some(Loaded::Found(Box::new(Found { key, day, event, plan: None })));
        }
        let program = plan.and_then(|plan| plan.with(|doc| doc.program.clone())).or_else(|| mine.and_then(|mine| mine.with(|doc| doc.program.clone())));
        match client.get(&StudyplanAsk { key: event.key, ids, program, locale: t.locale }) {
            None => kept(),
            Some(Ok(data)) => Some(Loaded::Found(Box::new(Found { key, day, event, plan: Some(data) }))),
            Some(Err(error)) => Some(Loaded::Failed(error)),
        }
    });
    let status = PageStatus::capture();
    // Which of the three the page is: the view below stands while only its Termin changes.
    let phase = Memo::new(move |_| {
        loaded.with(|loaded| match loaded {
            None => None,
            Some(Loaded::Missing) => Some(Ok(false)),
            Some(Loaded::Failed(error)) => Some(Err(error.clone())),
            Some(Loaded::Found(_)) => Some(Ok(true)),
        })
    });
    let found = move |read: &dyn Fn(&Found) -> Option<Timetable>| loaded.with(|loaded| if let Some(Loaded::Found(found)) = loaded { read(found) } else { None });
    // A module of the plan is read in the plan, one outside it in its own timetable, as QIS has it.
    let table = Memo::new(move |_| found(&|found| if found.planned(plan) { found.plan_table(plan, mine) } else { Some(found.event.timetable(&Selection::default())) }));
    let against = Memo::new(move |_| {
        loaded.with(|loaded| match loaded {
            Some(Loaded::Found(found)) if found.planned(plan) => Against::Itself,
            Some(Loaded::Found(found)) => found.plan_table(plan, mine).map_or(Against::Nothing, |table| Against::Plan(Box::new(table))),
            _ => Against::Nothing,
        })
    });
    // Whether it is in the plan: on the server not known (R9).
    let planned = Memo::new(move |_| loaded.with(|loaded| if let Some(Loaded::Found(found)) = loaded { APP.then(|| found.planned(plan)) } else { None }));
    let shown_asked = Memo::new(move |_| loaded.with(|loaded| if let Some(Loaded::Found(found)) = loaded { Some((found.key, found.day)) } else { None }));
    // The modules of both, by id: the chips of the plan's Termine too.
    let modules = Memo::new(move |_| {
        loaded.with(|loaded| match loaded {
            Some(Loaded::Found(found)) => {
                let mut all: BTreeMap<_, _> = found.plan.as_ref().map(module_facts).unwrap_or_default();
                for (id, facts) in module_facts(&found.event) {
                    all.entry(id).or_insert(facts);
                }
                all
            }
            _ => BTreeMap::new(),
        })
    });
    let semester = Memo::new(move |_| loaded.with(|loaded| if let Some(Loaded::Found(found)) = loaded { Some(found.event.key) } else { None }));
    let termin = Callback::new(move |(key, on): (RowKey, Option<Day>)| t.path(&url::event_path(key, on)));
    let module = Callback::new(move |(id, _): (String, Option<RowKey>)| t.path(&url::module_path(&id)));
    // „Anpassen ›": the Termin in the Stundenplan of the event's semester.
    let adjust = Callback::new(move |(key, on): (RowKey, Option<Day>)| {
        let at = StudyplanUrl { sem: semester.get_untracked().map(|semester| semester.key()), ..StudyplanUrl::default() };
        t.path(&at.with_termin(Some((key, on))).path())
    });
    let links = TerminLinks { termin, module, close: Signal::stored(String::new()), back: Signal::stored(None), adjust: Some(adjust) };
    let now = ticking_clock();
    let ctx = TerminCtx {
        table,
        asked: shown_asked,
        modules,
        mode: TerminMode::LookUp,
        plan: None,
        now,
        planned: planned.into(),
        against: against.into(),
        links,
        page: true,
    };
    // The page's title and what it says of itself, on the day shown.
    let head = Memo::new(move |_| {
        let now = now.get();
        table.with(|table| shown_asked.with(|asked| head_of(table.as_ref()?, (*asked)?, now, t)))
    });

    move || match phase.get() {
        // The first answer is on its way: the shell holds the page meanwhile.
        None => ().into_any(),
        Some(Err(error)) => {
            status.for_error(&error);
            view! { <Plain><ErrorState error/></Plain> }.into_any()
        }
        Some(Ok(false)) => {
            status.set(404);
            view! { <Plain><NotFound title=t.termin.not_found hint=t.termin.not_found_hint/></Plain> }.into_any()
        }
        Some(Ok(true)) => view! {
            <Plain class="tm-plain">
                {move || head.get().map(|head| view! {
                    <Title text=head.title.clone()/>
                    <Seo title=head.title description=head.description path=head.path noindex=true/>
                })}
                <div class="page-inner">
                    <TerminView ctx/>
                </div>
            </Plain>
        }
        .into_any(),
    }
}

/// The page's title, description and one address, on the date the view shows.
#[derive(Clone, Debug, PartialEq)]
struct Head {
    title: String,
    description: String,
    path: String,
}

fn head_of(table: &Timetable, (key, day): (RowKey, Option<Day>), now: Option<(Day, u16)>, t: &'static i18n::Texts) -> Option<Head> {
    let o = Occasion::at(table, key, day, now)?;
    let event = table.events.get(o.event)?;
    let kind = type_text(event, t.locale);
    let title = match event.title.trim() {
        "" => event.modules.first().cloned().unwrap_or_default(),
        title => title.to_string(),
    };
    let weekday = t.locale.texts().weekday_short(i64::from(o.day.weekday())).unwrap_or_default();
    let when = format!("{weekday} {}", o.day.date(t.locale));
    let time = o.from.map(|from| format!(" {}", folia_calendar::day::clock(from))).unwrap_or_default();
    let what = folia_timetable::views::kind_and_title(&kind, &title);
    Some(Head {
        title: (t.termin.page_title)(&kind, &title, &when),
        description: (t.termin.page_description)(&what, &format!("{when}{time}")),
        path: url::event_path(key, Some(o.day)),
    })
}
