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
//! The BTU's pages (its lists of regulations and forms, „Rund ums Studium") are written here, each
//! in the language of the app (the BTU has all of them in both); `e2e/btu.mjs` says which of them no
//! longer answer.

use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_model::labels::{GeneralTopic, Labelled};
use folia_pages::ask::BtuDocumentsAsk;
use folia_pages::{BtuDocument, BtuDocuments, BtuRegulation};

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

/// A group of pages under its name.
struct Group {
    name: fn(&BtuTexts) -> &'static str,
    pages: &'static [Page],
}

/// „Rund ums Studium": the BTU's pages, in groups as its menu of „Studienorganisation" has them.
const AROUND: [Group; 3] = [
    Group { name: |t| t.exams, pages: &[
        Page { name: |t| t.exams_register, de: "/studium/im-studium/studienorganisation/pruefungen/an-abmeldung-von-modulpruefungen", en: "/en/study/during-studies/study-organization/examinations/registering-for-examinations" },
        Page { name: |t| t.exams_deadlines, de: "/studium/im-studium/studienorganisation/pruefungen/pruefungs-studienfristen", en: "/en/study/during-studies/study-organization/examinations/examinations-deadlines-and-study-time-limits" },
        Page { name: |t| t.exams_illness, de: "/studium/im-studium/studienorganisation/pruefungen/krankheit-verhinderung", en: "/en/study/during-studies/study-organization/examinations/illness-and-incapability" },
        Page { name: |t| t.exams_work, de: "/studium/im-studium/studienorganisation/pruefungen/pruefungsleistungen", en: "/en/study/during-studies/study-organization/examinations/academic-work" },
    ] },
    Group { name: |t| t.semester, pages: &[
        Page { name: |t| t.dates, de: "/studium/im-studium/studienorganisation/quicklinks/termine-fristen", en: "/en/study/during-studies/study-organization/quicklinks/dates-and-deadlines" },
        Page { name: |t| t.reregister, de: "/studium/im-studium/studienorganisation/formalia/rueckmeldung", en: "/en/study/during-studies/study-organization/formalities/registering-for-the-following-semester" },
        Page { name: |t| t.fee, de: "/studium/im-studium/studienorganisation/formalia/semestergebuehr", en: "/en/study/during-studies/study-organization/formalities/semester-fee" },
        Page { name: |t| t.leave, de: "/studium/im-studium/studienorganisation/formalia/beurlaubung", en: "/en/study/during-studies/study-organization/formalities/leave-of-absence" },
        Page { name: |t| t.change, de: "/studium/im-studium/studienorganisation/formalia/studiengangwechsel", en: "/en/study/during-studies/study-organization/formalities/change-of-degree-programme" },
        Page { name: |t| t.exmatriculation, de: "/studium/im-studium/studienorganisation/formalia/exmatrikulation", en: "/en/study/during-studies/study-organization/formalities/removal-from-the-register-of-students" },
        Page { name: |t| t.card, de: "/studium/im-studium/studienorganisation/formalia/studierendenausweis", en: "/en/study/during-studies/study-organization/formalities/identification-card-chip-card" },
    ] },
    Group { name: |t| t.help, pages: &[
        Page { name: |t| t.service, de: "/studium/im-studium/studienorganisation/quicklinks/studierendenservice", en: "/en/study/during-studies/study-organization/quicklinks/student-services" },
        Page { name: |t| t.advice, de: "/studium/vor-dem-studium/beratung-hilfe", en: "/en/study/before-studying/advice-and-support" },
        Page { name: |t| t.portals, de: "/studium/im-studium/studienorganisation/quicklinks/portale", en: "/en/study/during-studies/study-organization/quicklinks/portals" },
    ] },
];

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

/// „Rund ums Studium": the BTU's pages on examinations, the semester's formalities, its services.
#[component]
pub(super) fn Around() -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_btu;
    view! {
        <section class="panel st-docs" aria-labelledby="st-docs-around">
            <header class="st-view-head">
                <div>
                    <h2 id="st-docs-around">{b.around}</h2>
                    <p>{b.around_lead}</p>
                </div>
            </header>
            {AROUND.iter().map(|group| {
                let (name, pages) = ((group.name)(b), group.pages);
                view! {
                    <p class="label st-docs-label">{name}</p>
                    <ul class="doclist" aria-label=name>
                        {pages.iter().map(|page| view! { <li>{row("info", (page.name)(b).to_string(), None, page_url(page, t.locale), false)}</li> }).collect_view()}
                    </ul>
                }
            }).collect_view()}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every page in both languages.
    #[test]
    fn the_pages_of_the_btu_are_in_the_app_language() {
        let first = &AROUND[0].pages[0];
        assert_eq!(page_url(first, Locale::De), "https://www.b-tu.de/studium/im-studium/studienorganisation/pruefungen/an-abmeldung-von-modulpruefungen");
        assert_eq!(page_url(first, Locale::En), "https://www.b-tu.de/en/study/during-studies/study-organization/examinations/registering-for-examinations");
        let advice = AROUND[2].pages.iter().find(|page| page.de.contains("beratung")).expect("Beratung und Hilfe");
        assert_eq!(page_url(advice, Locale::De), "https://www.b-tu.de/studium/vor-dem-studium/beratung-hilfe");
        assert_eq!(page_url(advice, Locale::En), "https://www.b-tu.de/en/study/before-studying/advice-and-support");
        assert_eq!(page_url(&LISTS[0], Locale::De), "https://www.b-tu.de/studium/im-studium/studienorganisation/pruefungen/ordnungen-und-formulare/grundsatzordnungen");
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
