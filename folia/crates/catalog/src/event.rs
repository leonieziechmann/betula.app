//! A Termin's own page (`/event/<key>[/<day>]`; folia/design/termin, the fourth round; owner,
//! 2026-10-26: „Wir sollten das wahrscheinlich Trennen zwischen der Informativen ansicht für die
//! Modulpage und mein studiengang und dann der ansicht bei der Kalender page"): the Termin to look
//! up (`TerminMode::LookUp`, `lookup`), where a line of a module's Termine leads from its preview
//! and from „Vollbild" in another area (on the module's own page the Termin opens beside it),
//! „Ganze Seite" of the Termin beside its module's page, and „Teilen".
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

use folia_calendar::day::Day;
use folia_calendar::rowkey::RowKey;
use folia_data::PageStatus;
use folia_design::i18n::use_params_map;
use folia_design::stage::Title;
use folia_design::ui::NotFound;
use folia_routes::url;
use folia_shell::frame::{ErrorState, Plain};
use folia_shell::seo::Seo;
use folia_timetable::model::Timetable;
use folia_widgets::termin::{page_head, ticking_clock, TerminLinks, TerminPlace, TerminView};
use leptos::prelude::*;

use crate::i18n;
use crate::lookup::look_up;

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
    let found = look_up(asked);
    let status = PageStatus::capture();
    let termin = Callback::new(move |(key, on): (RowKey, Option<Day>)| t.path(&url::event_path(key, on)));
    let module = Callback::new(move |(id, _): (String, Option<RowKey>)| t.path(&url::module_path(&id)));
    let links = TerminLinks { termin, module, close: Signal::stored(String::new()), back: Signal::stored(None), adjust: Some(found.adjust()), full: None };
    let now = ticking_clock();
    let ctx = found.ctx(links, TerminPlace::Page, now);
    // The page's title and what it says of itself, on the day shown.
    let head = Memo::new(move |_| {
        let now = now.get();
        found.table.with(|table| found.asked.with(|asked| head_of(table.as_ref()?, (*asked)?, now, t)))
    });

    // Which of the three the page is: the view below stands while only its Termin changes.
    move || match found.phase.get() {
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

fn head_of(table: &Timetable, asked: (RowKey, Option<Day>), now: Option<(Day, u16)>, t: &'static i18n::Texts) -> Option<Head> {
    let head = page_head(table, asked, now, t.locale, &t.termin)?;
    Some(Head { title: head.title, description: head.description, path: url::event_path(asked.0, Some(head.day)) })
}
