//! What „Ordnungen & Dokumente" of „Mein Studium" shows of the BTU as a whole (2026-11-10: „Also
//! pack da mal noch die ganzen anderen dokumente der BTU mit rein. Also so allgemeine informationen
//! etc."): „Ordnungen der BTU", its regulations for every program — the Rahmenordnung of the
//! student's degree with its reading version, its English translation and its amendments, the
//! Immatrikulationsordnung, the regulation of the FÜS, the law of the state they rest on, and the
//! BTU's own lists of all of them —, and „Rund ums Studium", its pages on examinations, the
//! semester's formalities, its services.
//!
//! The regulations come from the snapshot (`folia_pages::btu_documents`): Radix reads the BTU's page
//! of Grundsatzordnungen every night, German and English, and its build makes `v_general_document`
//! of it (docs/radix/schema-v2.md, „General documents"). Every document is named by its issue of the
//! Amtsblatt as the program's documents are („ABl. 29/2024"), which Radix reads from the name of its
//! file; the head of the panel says when Radix read the page. A regulation the snapshot does not
//! have (the page not read yet, a heading the rules no longer recognise) is said to be missing and
//! leads to the BTU's own list, never to an address that may be old.
//!
//! „Rund ums Studium" comes from the snapshot as well (`folia_pages::btu_pages`; owner, 2026-11-11:
//! „Mach die Rund ums Studium Seiten auch über Radix"): Radix reads the BTU's page
//! „Studienorganisation" every night, German and English, and its build makes `v_study_page` of it
//! (docs/radix/schema-v2.md, „Study pages"): its groups, the pages' titles and the sentence under
//! each, as the BTU writes them, in the language of the app. Where the snapshot has none of them
//! (the page not read yet), a row says so and leads to the BTU's page.
//!
//! The BTU's own lists of regulations and forms („Bei der BTU") and its page „Studienorganisation"
//! are written here, each in the language of the app (the BTU has all of them in both);
//! `e2e/btu.mjs` says which of them no longer answer.

use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_model::labels::{GeneralTopic, Labelled};
use folia_pages::ask::{BtuDocumentsAsk, BtuPagesAsk};
use folia_pages::{BtuDocument, BtuDocuments, BtuPage, BtuPages, BtuRegulation};

use super::StudyCtx;
use crate::i18n::study_btu::Texts as BtuTexts;
use crate::i18n::{self, Locale};

const BTU: &str = "https://www.b-tu.de";

/// Which Rahmenordnung concerns the student: the one of the degree, both where it is neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Level {
    Bachelor,
    Master,
    Both,
}

/// The regulations „Ordnungen der BTU" names for a degree, in their order: its Rahmenordnung (both
/// where the degree is neither), then what holds for every program.
fn topics(level: Level) -> Vec<GeneralTopic> {
    let frames = match level {
        Level::Bachelor => vec![GeneralTopic::FrameworkBachelor],
        Level::Master => vec![GeneralTopic::FrameworkMaster],
        Level::Both => vec![GeneralTopic::FrameworkBachelor, GeneralTopic::FrameworkMaster],
    };
    frames.into_iter().chain([GeneralTopic::Enrolment, GeneralTopic::Fues, GeneralTopic::HigherEducationAct]).collect()
}

/// A page of the BTU's site, in German and in English.
struct Page {
    name: fn(&BtuTexts) -> &'static str,
    de: &'static str,
    en: &'static str,
}

/// The BTU's own lists: all its Grundsatzordnungen, the regulations of every program, its forms.
const LISTS: [Page; 3] = [
    Page { name: |t| t.all_statutes, de: "/studium/im-studium/studienorganisation/pruefungen/ordnungen-und-formulare/grundsatzordnungen", en: "/en/study/during-studies/study-organization/examinations/regulations-and-forms/basic-regulations" },
    Page { name: |t| t.all_program_statutes, de: "/studium/im-studium/studienorganisation/pruefungen/ordnungen-und-formulare/pruefungs-und-studienordnungen", en: "/en/study/during-studies/study-organization/examinations/regulations-and-forms/examination-and-study-regulations" },
    Page { name: |t| t.forms, de: "/studium/im-studium/studienorganisation/pruefungen/ordnungen-und-formulare/formulare-antraege", en: "/en/study/during-studies/study-organization/examinations/regulations-and-forms/forms-applications" },
];

/// The BTU's page that lists its pages around the study: where „Rund ums Studium" leads while the
/// snapshot has none of them.
const STUDY_ORGANISATION: Page = Page { name: |t| t.study_organisation, de: "/studium/im-studium/studienorganisation", en: "/en/study/during-studies/study-organization" };

/// The address of a page in the app's language.
fn page_url(page: &Page, locale: Locale) -> String {
    format!("{BTU}{}", if locale == Locale::En { page.en } else { page.de })
}

/// The day of an RFC 3339 time („2026-11-10T03:12:00Z"): day, month, year.
fn day_of(time: &str) -> Option<(u32, u32, i32)> {
    let date = time.get(..10)?;
    let mut parts = date.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok().filter(|month| (1..=12).contains(month))?;
    let day = parts.next()?.parse().ok().filter(|day| (1..=31).contains(day))?;
    Some((day, month, year))
}

/// A row: a document or a page, what it is, and that it leads away from Betula.
fn row(icon: &'static str, name: String, line: Option<String>, href: String, part: bool) -> AnyView {
    view! {
        <a class="docrow" class:docpart=part href=href rel="noopener">
            <Icon name=icon/>
            <span class="docname">{name}</span>
            {line.map(|line| view! { <small>{line}</small> })}
            <Icon name="arrow-up-right" class="go"/>
        </a>
    }
    .into_any()
}

/// A page of the BTU's around the study: its title, under it the sentence the BTU writes about it.
fn page_row(page: BtuPage) -> AnyView {
    view! {
        <a class="docrow pagerow" href=page.url rel="noopener">
            <Icon name="info"/>
            <span class="docname">{page.title}</span>
            {page.summary.map(|summary| view! { <small>{summary}</small> })}
            <Icon name="arrow-up-right" class="go"/>
        </a>
    }
    .into_any()
}

/// What a regulation's row says under its name: its issue of the Amtsblatt, the law where it is
/// published.
fn line_of(topic: GeneralTopic, document: &BtuDocument, b: &'static BtuTexts) -> Option<String> {
    document.issue.as_deref().map(b.issue).or_else(|| (topic == GeneralTopic::HigherEducationAct).then(|| b.law_line.to_string()))
}

/// A regulation: its row, under it its reading version where there is one, and its amendments and
/// the English translation — a Rahmenordnung's amendments numbered and folded after the
/// translation, another's each by its title before it.
fn regulation_view(regulation: BtuRegulation, b: &'static BtuTexts, locale: Locale) -> AnyView {
    let topic = regulation.topic;
    let numbered = matches!(topic, GeneralTopic::FrameworkBachelor | GeneralTopic::FrameworkMaster);
    let reading = regulation.reading.and_then(|at| regulation.amendments.get(at).map(|issue| (at, issue.clone())));
    let title = |document: &BtuDocument| if locale == Locale::En { document.title_en.clone().unwrap_or_else(|| document.title.clone()) } else { document.title.clone() };
    let plain = (!numbered).then(|| regulation.amendments.iter().map(|amendment| row("file-text", title(amendment), amendment.issue.as_deref().map(b.issue), amendment.url.clone(), true)).collect_view());
    let folded = if !numbered || regulation.amendments.is_empty() {
        ().into_any()
    } else {
        view! {
            <details class="st-docs-fold">
                <summary class="docrow docpart">
                    <Icon name="layers"/>
                    <span class="docname">{(b.amendments)(regulation.amendments.len())}</span>
                    <Icon name="chevron-down" class="go"/>
                </summary>
                {regulation.amendments.iter().enumerate().map(|(at, amendment)| row("file-text", (b.amendment)(at + 1), amendment.issue.as_deref().map(b.issue), amendment.url.clone(), true)).collect_view()}
            </details>
        }
        .into_any()
    };
    view! {
        <li>
            {row("file-text", topic.label(locale).to_string(), line_of(topic, &regulation.statute, b), regulation.statute.url.clone(), false)}
            {reading.map(|(at, issue)| row("file-check-2", (b.reading)(at + 1), issue.issue.as_deref().map(b.issue), issue.url, true))}
            {plain}
            {regulation.english.map(|english| row("languages", b.english.to_string(), Some(b.not_binding.to_string()), english, true))}
            {folded}
        </li>
    }
    .into_any()
}

/// „Ordnungen der BTU": the regulations of `level`'s degree and of every program, from the snapshot.
#[component]
pub(super) fn Statutes(ctx: StudyCtx, level: Level) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_btu;
    // The regulations; while they are on their way `None`, and what was there stays until the next
    // answer is. A question that failed has none of them: each is said to be missing.
    let documents = Memo::new(move |before: Option<&Option<BtuDocuments>>| {
        let now = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&BtuDocumentsAsk {})))?;
        folia_pages::ask::unless_pending(now, before, |now| match now {
            Ok(documents) => Some(documents),
            Err(error) if error.is_pending() => None,
            Err(_) => Some(BtuDocuments::default()),
        })
    });
    let as_of = move || documents.get().and_then(|documents| documents.fetched).and_then(|fetched| day_of(&fetched)).map(|(day, month, year)| (b.as_of)(day, month, year));
    let all = page_url(&LISTS[0], t.locale);
    let list = move || match documents.get() {
        None => view! {
            <ul class="doclist sk-sweep" aria-hidden="true">
                <li><span class="docrow"><i class="sk sk-w4"></i></span></li>
                <li><span class="docrow"><i class="sk sk-w3"></i></span></li>
            </ul>
        }
        .into_any(),
        Some(documents) => {
            let all = all.clone();
            view! {
                <ul class="doclist">
                    {topics(level).into_iter().map(|topic| match documents.regulations.iter().find(|regulation| regulation.topic == topic) {
                        Some(regulation) => regulation_view(regulation.clone(), b, t.locale),
                        None => view! { <li>{row("circle-help", topic.label(t.locale).to_string(), Some(b.missing.to_string()), all.clone(), false)}</li> }.into_any(),
                    }).collect_view()}
                </ul>
            }
            .into_any()
        }
    };
    view! {
        <section class="panel st-docs" aria-labelledby="st-docs-btu">
            <header class="st-view-head">
                <div>
                    <h2 id="st-docs-btu">{b.statutes}</h2>
                    <p>{b.statutes_lead}{move || as_of().map(|as_of| format!(" {as_of}"))}</p>
                </div>
            </header>
            {list}
            <p class="label st-docs-label">{b.at_btu}</p>
            <ul class="doclist">
                {LISTS.iter().map(|page| view! { <li>{row("list-tree", (page.name)(b).to_string(), Some(b.btu_site.to_string()), page_url(page, t.locale), false)}</li> }).collect_view()}
            </ul>
        </section>
    }
}

/// „Rund ums Studium": the BTU's pages around the study, in the groups its page
/// „Studienorganisation" has them, from the snapshot.
#[component]
pub(super) fn Around(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_btu;
    // The pages; while they are on their way `None`, and what was there stays until the next answer
    // is. A question that failed has none of them: the row that leads to the BTU says so.
    let pages = Memo::new(move |before: Option<&Option<BtuPages>>| {
        let now = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&BtuPagesAsk { locale: t.locale })))?;
        folia_pages::ask::unless_pending(now, before, |now| match now {
            Ok(pages) => Some(pages),
            Err(error) if error.is_pending() => None,
            Err(_) => Some(BtuPages::default()),
        })
    });
    let list = move || match pages.get() {
        None => view! {
            <ul class="doclist sk-sweep" aria-hidden="true">
                <li><span class="docrow"><i class="sk sk-w4"></i></span></li>
                <li><span class="docrow"><i class="sk sk-w3"></i></span></li>
            </ul>
        }
        .into_any(),
        Some(pages) if pages.groups.is_empty() => view! {
            <ul class="doclist">
                <li>{row("circle-help", b.study_organisation.to_string(), Some(b.missing.to_string()), page_url(&STUDY_ORGANISATION, t.locale), false)}</li>
            </ul>
        }
        .into_any(),
        Some(pages) => pages
            .groups
            .into_iter()
            .map(|group| {
                view! {
                    <p class="label st-docs-label">{group.name.clone()}</p>
                    <ul class="doclist" aria-label=group.name>
                        {group.pages.into_iter().map(|page| view! { <li>{page_row(page)}</li> }).collect_view()}
                    </ul>
                }
            })
            .collect_view()
            .into_any(),
    };
    view! {
        <section class="panel st-docs" aria-labelledby="st-docs-around">
            <header class="st-view-head">
                <div>
                    <h2 id="st-docs-around">{b.around}</h2>
                    <p>{b.around_lead}</p>
                </div>
            </header>
            {list}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every page in both languages.
    #[test]
    fn the_pages_of_the_btu_are_in_the_app_language() {
        assert_eq!(page_url(&STUDY_ORGANISATION, Locale::De), "https://www.b-tu.de/studium/im-studium/studienorganisation");
        assert_eq!(page_url(&STUDY_ORGANISATION, Locale::En), "https://www.b-tu.de/en/study/during-studies/study-organization");
        assert_eq!(page_url(&LISTS[0], Locale::De), "https://www.b-tu.de/studium/im-studium/studienorganisation/pruefungen/ordnungen-und-formulare/grundsatzordnungen");
        assert_eq!(page_url(&LISTS[2], Locale::En), "https://www.b-tu.de/en/study/during-studies/study-organization/examinations/regulations-and-forms/forms-applications");
    }

    /// The Rahmenordnung of the degree first, both where the degree is neither, then what holds for
    /// every program.
    #[test]
    fn each_degree_has_its_rahmenordnung() {
        use GeneralTopic::*;
        assert_eq!(topics(Level::Bachelor), [FrameworkBachelor, Enrolment, Fues, HigherEducationAct]);
        assert_eq!(topics(Level::Master), [FrameworkMaster, Enrolment, Fues, HigherEducationAct]);
        assert_eq!(topics(Level::Both), [FrameworkBachelor, FrameworkMaster, Enrolment, Fues, HigherEducationAct]);
    }

    /// The day Radix read the page, from its time; nothing of what is no time.
    #[test]
    fn the_list_is_as_old_as_its_reading() {
        assert_eq!(day_of("2026-11-10T03:12:00Z"), Some((10, 11, 2026)));
        assert_eq!(day_of("2026-13-10T03:12:00Z"), None);
        assert_eq!(day_of("gestern"), None);
    }
}
