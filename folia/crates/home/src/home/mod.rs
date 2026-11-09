//! The start page (concept A, owner 2026-10-20: „Ja setz Konzept A jetzt mal in die tat um";
//! folia/design/startseite/konzepte.html). Betula began as a website and became an app (owner, the
//! same day: „Anfangs ist das ja mehr so als webseite entstanden aber mittlerweile ist das ganze
//! eher so in richtung app Übergegangen"), so the start page begins with the visitor's day: the
//! photo of the season over the greeting, the date and a sentence about the day, the semester
//! clock, and what comes next, then today as a list beside the week, the exams, the study and the
//! Merkliste (`today.rs`). Whoever has set nothing up yet finds the setup there instead, three
//! steps on a card over a taller photo with Betula's mark and name beside it (`setup.rs`), and a
//! fourth for the phone where the site offers Betula Sync (`devices.rs`), and
//! under it what goes without one: the ways into the catalog. What the page said of Betula until
//! then moved to „Über Betula", for new visitors and for search engines, and is on the pages about
//! Betula since 2026-10-27 (`topics.rs`, `overview.rs`), which the start page leads to under
//! „Mehr über Betula".
//!
//! The picture never moves (owner, the same day: „Beim PC darf sich das hero banner nicht
//! verschieben … bei eingerichtet weniger platz … als bei uneingerichtet"): 640 px with the setup,
//! 300 px with the day, at every hour and in every step. The server writes the same page for
//! everybody (R9): the setup's first step, and the day's skeleton, which the stylesheet shows
//! instead where the head script found a program kept (`html.mine`), so that nothing jumps when the
//! app takes over. Like the pages about Betula the page is without a frame (R17): one scroll area,
//! the ground at its end, which names who took the photo.

mod ahead;
mod clock;
mod day;
mod detail;
mod devices;
mod overview;
mod photo;
mod setup;
mod start;
mod today;
pub mod topics;

/// The script the start page writes after the day's skeleton (`ahead`): one of the scripts the
/// pages write inline, which the server's Content-Security-Policy allows by their hashes
/// (`folia_app::inline_scripts`).
pub use ahead::SCRIPT as AHEAD_SCRIPT;

use folia_calendar::semester::SemesterKey;
use folia_model::labels::Season;
use folia_pages::ask::HomeAsk;
use folia_pages::HomeData;
use folia_routes::filter::{Language, TurnusFilter};
use folia_routes::url;
use folia_routes::CatalogQuery;
use leptos::prelude::*;

use folia_data::use_ask;
use folia_design::combobox::ClosePopups;
use folia_design::format;
use folia_design::stage::Title;
use folia_design::ui::{Mark, Wordmark};
use folia_shell::seo::{self, Seo};
use folia_stores::myprogram::MyProgram;

use self::photo::Photo;
use self::setup::Done;
use crate::i18n::{self, Texts};

/// The browser app (`csr`): only there is anything kept, and only there is the day.
const APP: bool = cfg!(feature = "csr");

/// A way into the catalog under „Entdecken": a filter people come for, with the number of modules
/// behind it, its icon tinted after what it is about (`t-…` in app.css).
pub(crate) struct Way {
    pub label: &'static str,
    pub hint: &'static str,
    pub icon: &'static str,
    pub tint: &'static str,
    pub query: CatalogQuery,
    /// The half of the year it is for, where it is one: the start page shows the current one's.
    pub season: Option<Season>,
}

/// The ways the start page asks the counts of, in this order: winter and summer (one of them is
/// shown, the current semester's), English, the FÜS.
pub(crate) fn ways(t: &'static Texts) -> Vec<Way> {
    let t = &t.home;
    vec![
        Way { label: t.winter.label, hint: t.winter.hint, icon: "snowflake", tint: "t-ice", query: CatalogQuery { turnus: TurnusFilter { winter: true, ..Default::default() }, ..Default::default() }, season: Some(Season::Winter) },
        Way { label: t.summer.label, hint: t.summer.hint, icon: "sun", tint: "t-sun", query: CatalogQuery { turnus: TurnusFilter { summer: true, ..Default::default() }, ..Default::default() }, season: Some(Season::Summer) },
        Way { label: t.english.label, hint: t.english.hint, icon: "languages", tint: "t-violet", query: CatalogQuery { languages: vec![Language::English], ..Default::default() }, season: None },
        Way { label: t.fues.label, hint: t.fues.hint, icon: "shuffle", tint: "t-teal", query: CatalogQuery { fues: Some(true), ..Default::default() }, season: None },
    ]
}

/// What the setup did, said by the note that waits for interaction over the page (owner,
/// 2026-11-09: „verwende die wait notification um das anzuzeigen", in place of the card over the
/// day there was): „Eingerichtet" (and „verbunden" where the profile syncs), the program and its
/// Fachsemester under it, „Rückgängig"
/// (`folia_widgets::marknote::Notice::SetUp`).
fn say_set_up(done: Done) {
    let t = i18n::t();
    let Some(notices) = folia_widgets::marknote::Notices::expect() else { return };
    // Connected in the setup's last step: it says so.
    let synced = match (folia_stores::sync::Sync::expect(), folia_stores::profiles::Profiles::expect()) {
        (Some(sync), Some(profiles)) => untrack(|| sync.state(profiles.active()).synced()),
        _ => false,
    };
    notices.say(folia_widgets::marknote::Notice::SetUp(Box::new(folia_widgets::marknote::SetUp {
        said: if synced { t.today.set_up_synced } else { t.today.set_up_said }.to_string(),
        detail: (t.today.set_up)(&done.program, done.fs),
        plan_before: done.plan_before,
        start_before: done.start_before,
    })));
}

/// What the start page shows: the setup, the day without a study („Erst mal umschauen"), or the
/// day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shown {
    Setup,
    Guest,
    Day,
}

/// The snapshot's current semester, as the start page's counts came with it.
fn current_of(home: &HomeData) -> Option<SemesterKey> {
    home.overview.current_semester.as_ref().and_then(|semester| SemesterKey::parse(&semester.key))
}

#[component]
pub fn HomePage() -> impl IntoView {
    let t = i18n::t();
    let queries: Vec<CatalogQuery> = ways(t).into_iter().map(|way| way.query).collect();
    let asked = use_ask(move || Some(HomeAsk { entries: queries.clone() }));
    let home = Signal::derive(move || asked.get().and_then(Result::ok));
    let now = Signal::derive(move || home.with(|home| home.as_ref().and_then(current_of)));

    // The site itself and its search: what a search engine knows of the address of the site.
    let data = vec![serde_json::json!({
        "@type": "WebSite",
        "@id": seo::absolute("/#website"),
        "url": seo::absolute(url::HOME),
        "name": seo::SITE_NAME,
        "alternateName": t.home.alternate_names,
        "description": t.home.description,
        "inLanguage": t.locale.code(),
        "about": { "@type": "CollegeOrUniversity", "name": seo::UNIVERSITY, "alternateName": "BTU Cottbus-Senftenberg", "url": seo::UNIVERSITY_URL },
        "potentialAction": {
            "@type": "SearchAction",
            "target": { "@type": "EntryPoint", "urlTemplate": format!("{}?q={{search_term_string}}", seo::absolute(url::CATALOG)) },
            "query-input": "required name=search_term_string",
        },
    })];

    // A picker's popup stands where its button was: scrolling the page takes it away.
    let close_popups = RwSignal::new(0u32);
    provide_context(ClosePopups(close_popups));

    // The visitor: a program kept is a study set up; „Erst mal umschauen" folds the setup away for
    // the visit; „Studium anlegen" leaves a note („Eingerichtet · …", one that waits for interaction). Where the
    // setup has a step after „Studium anlegen" (Betula Sync), the setup stays until it ends.
    let mine = MyProgram::expect();
    let kept = Memo::new(move |_| APP && mine.is_some_and(|mine| mine.with(|doc| doc.program.is_some())));
    let looking = RwSignal::new(false);
    let finishing = RwSignal::new(false);
    // What „Studium anlegen" did while the setup's last step stands: the scanner's „Fertig" ends the
    // setup here (`/?setup=done`).
    let parked = RwSignal::new(None::<Done>);
    let on_written = Callback::new(move |()| finishing.set(true));
    let on_parked = Callback::new(move |done: Done| parked.set(Some(done)));
    let on_done = Callback::new(move |done: Done| {
        finishing.set(false);
        looking.set(false);
        parked.set(None);
        say_set_up(done);
    });
    // „Erst mal umschauen" leaves the setup, also one that has set up a study already (its chip led
    // back to step 1): then the day.
    let on_look = Callback::new(move |()| {
        finishing.set(false);
        looking.set(true);
    });
    let on_setup = Callback::new(move |()| looking.set(false));
    // The start page filled with its Termin („Ganze Seite", `today::TerminOver`, there with a
    // study set up): its own parts make way (app.css). A class of the page, not a `:has()` of it:
    // a `:has()` whose subject was every child but one had each node put in anywhere restyle the
    // whole document, in the first frame of every click.
    let location = i18n::use_location();
    let termin_full = Memo::new(move |_| kept.get() && location.search.with(|search| url::home_termin_full(search)));
    // The scanner's „Fertig" from the setup's last step (`/?setup=done`, folia_profile::give): the
    // setup ends here with what it did, and the address lets go of the query.
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        if !location.search.with(|search| url::setup_done(search)) {
            return;
        }
        if let Some(going) = folia_shell::pending::Pending::expect() {
            going.go(&t.path(url::HOME), leptos_router::NavigateOptions { replace: true, scroll: false, ..Default::default() });
        }
        match parked.get_untracked() {
            Some(done) => on_done.run(done),
            None => finishing.set(false),
        }
    });
    // The head script's `html.mine` (a program kept) follows a study set up or undone here, so that
    // the start page's skeleton is the one the page then is (folia-shell, `skeleton`).
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        let on = kept.get();
        if let Some(root) = document().document_element() {
            let _ = root.class_list().toggle_with_force("mine", on);
        }
    });

    // What the page shows, a memo: the setup is not built anew when its study is written while it
    // still shows its last step.
    let shown = Memo::new(move |_| match (kept.get() && !finishing.get(), looking.get()) {
        (true, _) => Shown::Day,
        (false, true) => Shown::Guest,
        (false, false) => Shown::Setup,
    });
    let body = move || {
        if !APP {
            // The server's page (R9): the setup's first step, and the day's skeleton for a browser
            // whose head script found a program kept.
            return view! {
                <div class="day-first-wrap"><FirstVisit now on_done on_look on_written on_parked home server=true/></div>
                <div class="day-sk-wrap"><TodaySkeleton/></div>
            }
            .into_any();
        }
        match shown.get() {
            Shown::Day => view! { <today::TodayView guest=false on_setup home/> }.into_any(),
            Shown::Guest => view! { <today::TodayView guest=true on_setup home/> }.into_any(),
            Shown::Setup => view! { <FirstVisit now on_done on_look on_written on_parked home server=false/> }.into_any(),
        }
    };

    view! {
        <Title text=""/>
        <div class="work flowing solo day-page" id="page-scroll" on:scroll=move |_| close_popups.update(|n| *n = n.wrapping_add(1))>
            <div class="page"><div class="page-inner day" class:termin-full=move || termin_full.get()>
                <Seo title=t.home.seo_title description=t.home.description path=url::HOME data/>
                {body}
            </div></div>
            <folia_shell::ground::Ground credit=ViewFn::from(move || view! { <Credits/> })/>
        </div>
    }
}

/// A first visit: the setup on the taller photo, Betula's mark and name beside it on a wide screen
/// (owner, 2026-10-20: „Bei der PC version ist beim einrichten rechts mega viel Platz. Mach da mal
/// noch das Betula logo + Schriftzug rein"), and under it the ways into the catalog.
#[component]
fn FirstVisit(now: Signal<Option<SemesterKey>>, on_done: Callback<Done>, on_look: Callback<()>, on_written: Callback<()>, on_parked: Callback<Done>, home: Signal<Option<HomeData>>, server: bool) -> impl IntoView {
    let t = i18n::t();
    let faculties = today::faculties_of(home);
    let figures = move || {
        home.get().map(|home| {
            let mut parts = vec![format!("{} {}", format::count(home.overview.modules, t.locale), t.home.figure_modules), format!("{} {}", format::count(home.overview.programs, t.locale), t.home.figure_programs)];
            if let Some(semester) = &home.overview.current_semester {
                let name = SemesterKey::parse(&semester.key).map_or_else(|| semester.label.clone(), |key| key.label(t.locale));
                parts.push(format!("{} {} {}", format::count(semester.teaching_events.max(0) as u64, t.locale), t.home.figure_dates, (t.home.in_semester)(&name)));
            }
            parts.join(" · ")
        })
    };
    let card = if server { view! { <setup::ServerCard faculties/> }.into_any() } else { view! { <setup::SetupCard now faculties on_done on_look on_written on_parked/> }.into_any() };
    view! {
        <section class="day-hero day-first" aria-labelledby="ob-title">
            <div class="day-photo" aria-hidden="true"></div>
            <div class="day-shade" aria-hidden="true"></div>
            {card}
            <div class="day-brand">
                <div class="day-brand-mark">
                    <span class="day-brand-logo"><Mark/></span>
                    <Wordmark/>
                    <span class="day-brand-tag">{t.common.tagline}</span>
                </div>
                <p class="day-brand-say">
                    <b>{topics::unbroken(t.topics.overview.title)}</b>
                    <span>{figures}</span>
                </p>
            </div>
        </section>
        <today::Discover home first=true/>
        // What Betula is, a step further (owner, 2026-10-27: „Wovon auch ein wenig auf der Landing
        // page verlinkt werden kann").
        <topics::MoreAbout topics=topics::ON_THE_START_PAGE.to_vec() day=true/>
    }
}

/// The day's skeleton, as the server writes it for a visitor with a program kept (`html.mine`):
/// the picture as high as the day's, the panels as they come. Right after the picture the script
/// that puts the day in its place where the browser app kept it (`ahead`), before anything is
/// painted. Its parts are hidden from screen readers, the day put in place of one is not.
#[component]
fn TodaySkeleton() -> impl IntoView {
    view! {
        <section class="day-hero day-hero-sk" aria-hidden="true">
            <div class="day-photo"></div>
            <div class="day-shade"></div>
            <div class="day-greet">
                <i class="sk sk-w2 day-sk-light"></i>
                <i class="sk sk-w4 sk-big day-sk-light"></i>
                <i class="sk sk-w5 day-sk-light"></i>
            </div>
            <div class="day-next day-next-sk"></div>
        </section>
        <script inner_html=ahead::SCRIPT></script>
        <div class="day-cols" aria-hidden="true">
            <div class="day-col day-main"><div class="panel sk-sweep day-panel-sk tall"></div></div>
            <div class="day-col"><div class="panel sk-sweep day-panel-sk"></div><div class="panel sk-sweep day-panel-sk"></div></div>
        </div>
    }
}

/// Who took the photo, in the ground: one line per photo, the stylesheet shows the one shown
/// (`data-season`, and the fog while the exams run), and only while the start page is in front.
/// In the page's own ground, which a wide screen shows, and in the app's (`folia_app`), which a
/// phone shows under every page.
#[component]
pub fn Credits() -> impl IntoView {
    let t = i18n::t();
    Photo::ALL
        .map(|photo| {
            let (name, link) = photo.by();
            view! {
                <span class="photo-credit" data-photo=photo.code()>{t.today.photo_by}": "<a href=link rel="noopener">{name}</a>", "<a href="https://unsplash.com" rel="noopener">"Unsplash"</a></span>
            }
        })
        .collect_view()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ways in the start page shows: the current semester's half of the year first.
    #[test]
    fn the_ways_are_asked_in_their_order() {
        let ways = ways(i18n::texts(folia_locale::Locale::De));
        let seasons: Vec<Option<Season>> = ways.iter().map(|way| way.season).collect();
        assert_eq!(seasons, [Some(Season::Winter), Some(Season::Summer), None, None]);
        assert!(ways.iter().all(|way| way.query != CatalogQuery::default()));
    }
}
