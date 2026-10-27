//! The pages about Betula, one per topic, `/about/<topic>` (`url::AboutTopic`; owner, 2026-10-27:
//! „dass die Über Betula Seite geändert wird. Das soll aufgeteilt werden in mehrere kontent pages
//! … Wovon auch ein wenig auf der Landing page verlinkt werden kann oder so"), and the tiles that
//! lead to them („Mehr über Betula", `MoreAbout`): under each topic's page and on the start page.
//!
//! Until then „Über Betula" was one page of some 13,000 px: the first panel, the pictures, the
//! ways into the catalog, what Betula does, „Betula im Detail" in eight chapters and the
//! questions. Each topic is a page of its own with its own address, title and description for
//! search engines (the text is what they know of Betula, owner, 2026-09-28): „Überblick" (the first
//! panel, the pictures with the map of the programs, what Betula does; `overview.rs`), „Module
//! finden" (the way to a module, the filters, the ways into the catalog), „Studium planen" (the
//! Regelstudienpläne, the Stundenplan, the programs by faculty), „Deine Daten" (no account, where
//! the data comes from), „Technik" (with and without JavaScript, the devices) and „Häufige
//! Fragen". The chapters are those of `detail.rs`, the questions those of `home`; nothing of them
//! was rewritten. „Über Betula" itself, which led to the topics, went the same day (owner: „die
//! /about seite muss komplett raus nur die about/<slug> seiten sind ok. Was du als erhaltenswert
//! siehst dann noch als eigene page"): what it kept is the overview, and its address leads there
//! (`url::ABOUT`).
//!
//! A topic's page is as the start page is, without a frame (R17): its panels in one column in the
//! middle, at most 1600 px wide, the wood beside it. Its head has „Zurück" to the start page, which
//! Esc follows as on every page with a way back (owner, the same day: „und dann natürlich auch
//! noch den shortcut mit esc, um wieder bei der landing page zu bleiben"; `frame::BackLink`, the
//! box at the head of a phone's page), and names every topic, the one shown marked; at its end the
//! other topics as tiles.

use folia_design::combobox::ClosePopups;
use folia_design::i18n::use_params_map;
use folia_design::stage::Title;
use folia_design::ui::{Icon, NotFound};
use folia_pages::ask::HomeAsk;
use folia_routes::url::AboutTopic;
use folia_shell::frame::{BackLink, ErrorState, Plain};
use folia_shell::seo::Seo;
use folia_shell::tabs::Area;
use leptos::prelude::*;

use folia_data::{use_ask, PageStatus, ProgramMapHandle};

use super::{detail, overview};
use crate::i18n::{self, Texts};

/// How a topic looks where it is named: its icon and its tint (`t-…`, the colours of the
/// chapters it holds).
pub fn look(topic: AboutTopic) -> (&'static str, &'static str) {
    match topic {
        AboutTopic::Overview => ("leaf", "t-green"),
        AboutTopic::Finding => ("search", "t-ice"),
        AboutTopic::Planning => ("calendar-range", "t-sun"),
        AboutTopic::Data => ("shield-check", "t-teal"),
        AboutTopic::Technology => ("code", "t-slate"),
        AboutTopic::Questions => ("circle-help", "t-violet"),
    }
}

/// The topics the start page leads to: what someone who comes for the first time wants to know
/// (five, a row of tiles, as many as under a topic's page; „Technik" is one step further, in the
/// row of the topics at the head of each).
pub const ON_THE_START_PAGE: [AboutTopic; 5] = [AboutTopic::Overview, AboutTopic::Finding, AboutTopic::Planning, AboutTopic::Data, AboutTopic::Questions];

/// The university's name, the same in every language.
const UNIVERSITY: &str = "BTU Cottbus-Senftenberg";

/// A text with the university's name in it, the name unbroken (`.nowrap`): a heading balanced
/// over its lines would break it after „Cottbus-". The overview's heading, and what the start page
/// says beside Betula's name.
pub fn unbroken(text: &'static str) -> AnyView {
    match text.split_once(UNIVERSITY) {
        Some((before, after)) => view! { {before}<span class="nowrap">{UNIVERSITY}</span>{after} }.into_any(),
        None => text.into_any(),
    }
}

/// Questions and answers, in three groups: about Betula, using it, and about studying (for first
/// semesters).
pub fn questions(t: &'static Texts) -> [&'static i18n::home::Faq; 3] {
    [&t.home.about_betula, &t.home.using_betula, &t.home.for_studies]
}

/// „Mehr über Betula": a tile for each of `topics`, its icon in its tint, its name and what it is
/// about. `day`: on the start page, where it follows „Entdecken" and has its head; else the head
/// of the panels of a topic's page.
#[component]
pub fn MoreAbout(topics: Vec<AboutTopic>, #[prop(optional)] day: bool) -> impl IntoView {
    let t = i18n::t();
    let count = topics.len();
    let head = if day {
        view! { <header class="day-head"><h2 id="mehr-titel">{t.topics.more}</h2><span class="sub">{t.topics.more_lead}</span></header> }.into_any()
    } else {
        view! { <header class="block-head"><h2 id="mehr-titel">{t.topics.more}</h2><p>{t.topics.more_lead}</p></header> }.into_any()
    };
    view! {
        <section class="panel about-ways" class:on-day=day aria-labelledby="mehr-titel" data-commands=t.topics.more>
            {head}
            <div class="about-tiles" style=format!("--n:{count}")>
                {topics.into_iter().map(|topic| {
                    let (icon, tint) = look(topic);
                    let words = t.topics.of(topic);
                    view! {
                        <a class=format!("about-tile {tint}") href=t.path(&topic.path()) data-command=words.name>
                            <span class="about-tile-i"><Icon name=icon/></span>
                            <span class="about-tile-t"><b>{words.name}</b><small>{words.hint}</small></span>
                            <Icon name="chevron-right"/>
                        </a>
                    }
                }).collect_view()}
            </div>
        </section>
    }
}

/// The row of the topics at the head of a topic's page: each by its icon and name, the one shown
/// (`current`) marked.
#[component]
pub(super) fn TopicNav(current: AboutTopic) -> impl IntoView {
    let t = i18n::t();
    view! {
        <nav class="topic-nav" aria-label=t.topics.topics>
            {AboutTopic::ALL.into_iter().map(|each| {
                let (icon, tint) = look(each);
                view! {
                    <a class=tint href=t.path(&each.path()) aria-current=(each == current).then_some("page")><Icon name=icon/>{t.topics.of(each).name}</a>
                }
            }).collect_view()}
        </nav>
    }
}

/// The page of a topic. Its answer comes before it (the shell holds the page meanwhile), the same
/// for every topic, so that the way from one to the next asks nothing: the snapshot's figures and
/// faculties, how many modules the ways into the catalog find (`overview::queries`) and, after
/// them, the filter board's example (`detail::example`). An address of no topic is the page that
/// does not exist.
#[component]
pub fn TopicPage() -> impl IntoView {
    let t = i18n::t();
    let status = PageStatus::capture();
    let params = use_params_map();
    let topic = Memo::new(move |_| params.with(|params| params.get_str("topic").and_then(AboutTopic::of_slug)));
    let ways = overview::queries(t);
    let example_at = ways.len();
    let queries: Vec<_> = ways.into_iter().chain([detail::example()]).collect();
    let asked = use_ask(move || topic.get().map(|_| HomeAsk { entries: queries.clone() }));
    let map = use_context::<ProgramMapHandle>().map(|handle| handle.0);
    let page = move || {
        let Some(topic) = topic.get() else {
            status.set(404);
            return Some(view! { <Plain><NotFound title=t.app.not_found_title hint=t.app.not_found_hint/></Plain> }.into_any());
        };
        let map = map.clone();
        asked.get().map(|loaded| {
            let words = t.topics.of(topic);
            // A picker's popup (the overview's „Studiengang wählen") stands where its button was:
            // scrolling the page takes it away, as the filter panel of the catalog does. Given to
            // this page's view alone, not to the stage's other pages, and before the picker asks
            // for it.
            let close_popups = RwSignal::new(0u32);
            provide_context(ClosePopups(close_popups));
            let home = match &loaded {
                Ok(home) => Some(home.clone()),
                Err(error) => {
                    status.for_error(error);
                    None
                }
            };
            // What a topic's page says to search engines beside its text: the app on the
            // overview, the questions where they stand.
            let data = match topic {
                AboutTopic::Overview => Some(overview::app_data(t)),
                AboutTopic::Questions => Some(serde_json::json!({
                    "@type": "FAQPage",
                    "mainEntity": questions(t).into_iter().flat_map(|faq| faq.questions.iter()).map(|(question, answer)| serde_json::json!({
                        "@type": "Question",
                        "name": question,
                        "acceptedAnswer": { "@type": "Answer", "text": answer },
                    })).collect::<Vec<_>>(),
                })),
                _ => None,
            };
            let others: Vec<AboutTopic> = AboutTopic::ALL.into_iter().filter(|other| *other != topic).collect();
            let body = match (topic, home) {
                // The overview's first panel is its head, with the figures beside it.
                (AboutTopic::Overview, home) => {
                    let modules = home.as_ref().map(|home| home.overview.modules);
                    let failed = loaded.as_ref().err().map(|error| view! { <ErrorState error=error.clone()/> });
                    view! {
                        <overview::Hero home/>
                        <overview::Showcase map modules/>
                        {failed}
                        <overview::Abilities/>
                    }
                    .into_any()
                }
                (topic, home) => {
                    let (_, tint) = look(topic);
                    let content = match (home, &loaded) {
                        (Some(_), _) if topic == AboutTopic::Questions => view! { <Questions/> }.into_any(),
                        (Some(home), _) => {
                            let plans = Some((home.overview.plans, home.overview.programs));
                            let example = home.entry_counts.get(example_at).copied();
                            let list = match topic {
                                AboutTopic::Finding => Some(view! { <overview::Ways home=home.clone()/> }.into_any()),
                                AboutTopic::Planning => Some(view! { <overview::Faculties home=home.clone()/> }.into_any()),
                                _ => None,
                            };
                            view! { <detail::Chapters topic plans example/>{list} }.into_any()
                        }
                        (None, Err(error)) => view! { <ErrorState error=error.clone()/> }.into_any(),
                        (None, Ok(_)) => ().into_any(),
                    };
                    view! {
                        <header class=format!("panel topic-head {tint}")>
                            <BackLink area=Area::Home/>
                            <h1>{words.title}</h1>
                            <p class="lead">{words.lead}</p>
                            <TopicNav current=topic/>
                        </header>
                        {content}
                    }
                    .into_any()
                }
            };
            // No frame (R17), as the start page: one scroll area with the ground at its end.
            // The tab's title with the page, as every page that follows its address writes it: a
            // title that read the address later would read it after a page rendered beside
            // others on the server is gone (`a_page_rendered_beside_others_keeps_its_title`).
            view! {
                <Title text=words.name/>
                <div class="work flowing solo" id="page-scroll" on:scroll=move |_| close_popups.update(|n| *n = n.wrapping_add(1))>
                <div class="page home-page"><div class="page-inner home topic-page">
                <Seo title=words.seo_title description=words.description path=topic.path() data=data.into_iter().collect()/>
                {body}
                <MoreAbout topics=others/>
                </div></div>
                <folia_shell::ground::Ground/>
                </div>
            }
            .into_any()
        })
    };
    page
}

/// `/about`, where „Über Betula" stood until 2026-10-27: the browser app goes on to the overview at
/// once, in the same entry of the history, as the server answers the address with a redirect for
/// good (`folia-server`). The app comes there only by itself: an entry of the history from before,
/// a link kept somewhere.
#[component]
pub fn AboutRedirect() -> impl IntoView {
    #[cfg(feature = "csr")]
    if let Some(going) = folia_shell::pending::Pending::expect() {
        request_animation_frame(move || going.go(&AboutTopic::Overview.path(), leptos_router::NavigateOptions { replace: true, ..Default::default() }));
    }
}

/// The questions: the list, and an answer opens in place. The text is in the page either way (and
/// in the page's FAQPage data).
#[component]
fn Questions() -> impl IntoView {
    let t = i18n::t();
    view! {
        <section class="panel questions" id="fragen" aria-label=t.home.questions>
            {questions(t).into_iter().map(|faq| view! {
                <div class="faq">
                    <h2 class="faq-group label">{faq.name}</h2>
                    {faq.questions.iter().map(|(question, answer)| view! {
                        <details>
                            <summary>{*question}<Icon name="plus"/></summary>
                            <p>{*answer}</p>
                        </details>
                    }).collect_view()}
                </div>
            }).collect_view()}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The overview's heading, which the start page says as well, names the university in every
    /// language, so that it stands unbroken (`unbroken`); every topic has its look and its words.
    #[test]
    fn every_topic_has_its_words() {
        for locale in folia_locale::Locale::ALL {
            let t = i18n::texts(*locale);
            assert!(t.topics.overview.title.contains(UNIVERSITY), "{locale:?}");
            for topic in AboutTopic::ALL {
                let words = t.topics.of(topic);
                assert!(!words.name.is_empty() && !words.title.is_empty() && !words.seo_title.is_empty() && !words.description.is_empty() && !words.hint.is_empty(), "{locale:?} {topic:?}");
            }
        }
        let tints: std::collections::HashSet<_> = AboutTopic::ALL.into_iter().map(|topic| look(topic).1).collect();
        assert_eq!(tints.len(), AboutTopic::ALL.len(), "a tint of its own for every topic");
        assert!(ON_THE_START_PAGE.first() == Some(&AboutTopic::Overview) && ON_THE_START_PAGE.len() == AboutTopic::ALL.len() - 1);
    }
}
