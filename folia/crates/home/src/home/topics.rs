//! The pages of „Über Betula" on its topics, `/about/<topic>` (`url::AboutTopic`; owner,
//! 2026-10-27: „dass die Über Betula Seite geändert wird. Das soll aufgeteilt werden in mehrere
//! kontent pages … Wovon auch ein wenig auf der Landing page verlinkt werden kann oder so"), and
//! the tiles that lead to them („Mehr über Betula", `MoreAbout`): on „Über Betula", under each
//! topic's page and on the start page.
//!
//! Until then „Über Betula" was one page of some 13,000 px: the first panel, the pictures, the
//! ways into the catalog, what Betula does, „Betula im Detail" in eight chapters and the
//! questions. „Über Betula" keeps what it is the way into (the first panel, the pictures, the ways
//! in, what Betula does) and leads to the topics; each topic is a page of its own with its own
//! address, title and description for search engines (the text is what they know of Betula,
//! owner, 2026-09-28), two chapters each, and the questions one: „Module finden" (the way to a
//! module, the filters), „Studium planen" (the Regelstudienpläne, the Stundenplan), „Deine Daten"
//! (no account, where the data comes from), „Technik" (with and without JavaScript, the devices)
//! and „Häufige Fragen". The chapters are those of `detail.rs`, the questions those of `home`;
//! nothing of them was rewritten.
//!
//! A topic's page is as „Über Betula" is, without a frame (R17): its panels in one column in the
//! middle, at most 1600 px wide, the wood beside it. Its head leads back to „Über Betula" and
//! names every topic, the one shown marked; at its end the other topics as tiles.

use folia_design::i18n::use_params_map;
use folia_design::stage::Title;
use folia_design::ui::{Icon, NotFound};
use folia_pages::ask::HomeAsk;
use folia_routes::url::{self, AboutTopic};
use folia_shell::frame::{ErrorState, Plain};
use folia_shell::seo::Seo;
use leptos::prelude::*;

use folia_data::{use_ask, PageStatus};

use super::detail;
use crate::i18n::{self, Texts};

/// How a topic looks where it is named: its icon and its tint (`t-…`, the colours of the
/// chapters it holds).
pub fn look(topic: AboutTopic) -> (&'static str, &'static str) {
    match topic {
        AboutTopic::Finding => ("search", "t-ice"),
        AboutTopic::Planning => ("calendar-range", "t-sun"),
        AboutTopic::Data => ("shield-check", "t-teal"),
        AboutTopic::Technology => ("code", "t-slate"),
        AboutTopic::Questions => ("circle-help", "t-violet"),
    }
}

/// The topics the start page leads to: what someone who comes for the first time wants to know
/// (four, a row of tiles; „Technik" is one step further, on „Über Betula").
pub const ON_THE_START_PAGE: [AboutTopic; 4] = [AboutTopic::Finding, AboutTopic::Planning, AboutTopic::Data, AboutTopic::Questions];

/// Questions and answers, in three groups: about Betula, using it, and about studying (for first
/// semesters).
pub fn questions(t: &'static Texts) -> [&'static i18n::home::Faq; 3] {
    [&t.home.about_betula, &t.home.using_betula, &t.home.for_studies]
}

/// „Mehr über Betula": a tile for each of `topics`, its icon in its tint, its name and what it is
/// about, and in the head the way to „Über Betula" (`all`, not on „Über Betula" itself). `day`:
/// on the start page, where it follows „Entdecken" and has its head; else the head of the panels
/// of „Über Betula".
#[component]
pub fn MoreAbout(topics: Vec<AboutTopic>, #[prop(optional)] all: bool, #[prop(optional)] day: bool) -> impl IntoView {
    let t = i18n::t();
    let count = topics.len();
    let to_all = move || all.then(|| view! { <a class="ghost" href=t.path(url::ABOUT) data-command="">{t.topics.all_about}<Icon name="arrow-right"/></a> });
    let head = if day {
        view! { <header class="day-head"><h2 id="mehr-titel">{t.topics.more}</h2><span class="sub">{t.topics.more_lead}</span>{to_all()}</header> }.into_any()
    } else {
        view! { <header class="block-head"><h2 id="mehr-titel">{t.topics.more}</h2>{to_all()}<p>{t.topics.more_lead}</p></header> }.into_any()
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

/// The page of a topic. Its answer (the snapshot's figures: how many programs have a checked plan,
/// how many modules the filter board's example finds) comes before it; the shell holds the page
/// meanwhile. An address of no topic is the page that does not exist.
#[component]
pub fn TopicPage() -> impl IntoView {
    let t = i18n::t();
    let status = PageStatus::capture();
    let params = use_params_map();
    let topic = Memo::new(move |_| params.with(|params| params.get_str("topic").and_then(AboutTopic::of_slug)));
    let asked = use_ask(move || topic.get().map(|_| HomeAsk { entries: vec![detail::example()] }));
    let page = move || {
        let Some(topic) = topic.get() else {
            status.set(404);
            return Some(view! { <Plain><NotFound title=t.app.not_found_title hint=t.app.not_found_hint/></Plain> }.into_any());
        };
        asked.get().map(|loaded| {
            let words = t.topics.of(topic);
            let (plans, example) = match &loaded {
                Ok(home) => (Some((home.overview.plans, home.overview.programs)), home.entry_counts.first().copied()),
                Err(error) => {
                    status.for_error(error);
                    (None, None)
                }
            };
            // The questions as the page's structured data, where they stand.
            let data = (topic == AboutTopic::Questions).then(|| {
                serde_json::json!({
                    "@type": "FAQPage",
                    "mainEntity": questions(t).into_iter().flat_map(|faq| faq.questions.iter()).map(|(question, answer)| serde_json::json!({
                        "@type": "Question",
                        "name": question,
                        "acceptedAnswer": { "@type": "Answer", "text": answer },
                    })).collect::<Vec<_>>(),
                })
            });
            let others: Vec<AboutTopic> = AboutTopic::ALL.into_iter().filter(|other| *other != topic).collect();
            let (_, tint) = look(topic);
            // No frame (R17), as „Über Betula": one scroll area with the ground at its end.
            // The tab's title with the page, as every page that follows its address writes it: a
            // title that read the address later would read it after a page rendered beside
            // others on the server is gone (`a_page_rendered_beside_others_keeps_its_title`).
            view! {
                <Title text=words.name/>
                <div class="work flowing solo" id="page-scroll">
                <div class="page home-page"><div class="page-inner home topic-page">
                <Seo title=words.seo_title description=words.description path=topic.path() data=data.into_iter().collect()/>
                <header class=format!("panel topic-head {tint}")>
                    <a class="topic-back" href=t.path(url::ABOUT)><Icon name="chevron-left"/>{t.topics.back}</a>
                    <h1>{words.title}</h1>
                    <p class="lead">{words.lead}</p>
                    <nav class="topic-nav" aria-label=t.topics.topics>
                        {AboutTopic::ALL.into_iter().map(|each| {
                            let (icon, tint) = look(each);
                            view! {
                                <a class=tint href=t.path(&each.path()) aria-current=(each == topic).then_some("page")><Icon name=icon/>{t.topics.of(each).name}</a>
                            }
                        }).collect_view()}
                    </nav>
                </header>
                {match (&loaded, topic) {
                    (Err(error), _) => view! { <ErrorState error=error.clone()/> }.into_any(),
                    (Ok(_), AboutTopic::Questions) => view! { <Questions/> }.into_any(),
                    (Ok(_), topic) => view! { <detail::Chapters topic plans example/> }.into_any(),
                }}
                <MoreAbout topics=others all=true/>
                </div></div>
                <folia_shell::ground::Ground/>
                </div>
            }
            .into_any()
        })
    };
    page
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
