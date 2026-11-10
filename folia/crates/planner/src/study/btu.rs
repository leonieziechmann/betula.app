//! What „Ordnungen & Dokumente" of „Mein Studium" shows of the BTU as a whole (2026-11-10: „Also
//! pack da mal noch die ganzen anderen dokumente der BTU mit rein. Also so allgemeine informationen
//! etc."): „Ordnungen der BTU", its regulations for every program — the Rahmenordnung of the
//! student's degree with its reading version, its English translation and its amendments, the
//! Immatrikulationsordnung, the regulation of the FÜS, the law of the state they rest on, and the
//! BTU's own lists of all of them —, and „Rund ums Studium", its pages on examinations, the
//! semester's formalities, its services.
//!
//! The data has none of it: Radix reads the regulations of each program from QIS, not the BTU's
//! list of Grundsatzordnungen. So the list is written here, as the BTU's pages had it on
//! 2026-11-10 (`b-tu.de/studium/im-studium/studienorganisation`, and its page
//! `…/pruefungen/ordnungen-und-formulare/grundsatzordnungen`), every document named by its issue of
//! the Amtsblatt as the program's documents are („ABl. 29/2024": the issue's number begins the
//! name of its file); the head of the panel says how old the list is. `e2e/btu.mjs` says what of it
//! the BTU's pages no longer list, what they list anew, and which of its pages no longer answer.
//! Each page of the BTU in the language of the app, where the BTU has it in both (all of these);
//! the documents are German, the English translations the BTU's own.

use leptos::prelude::*;

use folia_design::ui::Icon;

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

/// A document of the Amtsblatt: its issue („13/2016") and its address.
#[derive(Clone, Copy, Debug)]
struct Issue {
    issue: &'static str,
    url: &'static str,
}

/// A Rahmenordnung as the BTU's page of Grundsatzordnungen lists it: the regulation, its
/// amendments oldest first, the one whose issue holds the reading version of the whole (an index
/// of `amendments`), and the BTU's English translation of that issue.
struct Frame {
    name: fn(&BtuTexts) -> &'static str,
    statute: Issue,
    amendments: &'static [Issue],
    reading: usize,
    english: &'static str,
}

const BACHELOR: Frame = Frame {
    name: |t| t.frame_bachelor,
    statute: Issue { issue: "13/2016", url: "https://opus4.kobv.de/opus4-btu/files/3983/AMbl-13_2016-RahmenO-BA.pdf" },
    amendments: &[
        Issue { issue: "01/2021", url: "https://opus4.kobv.de/opus4-btu/files/5396/AMbl-1_2021_RahmenO_BA_1.AeS.pdf" },
        Issue { issue: "17/2022", url: "https://opus4.kobv.de/opus4-btu/files/6092/AMbl-17_2022_2.AeS_RahmenO_BA.pdf" },
        Issue { issue: "06/2023", url: "https://opus4.kobv.de/opus4-btu/files/6349/06_3.AES_RahmenO_BA.pdf" },
        Issue { issue: "14/2024", url: "https://opus4.kobv.de/opus4-btu/files/6709/14_4.AS_RahmenO_BA_.pdf" },
        Issue { issue: "29/2024", url: "https://opus4.kobv.de/opus4-btu/files/6772/29_RahmenO.pdf" },
        Issue { issue: "35/2026", url: "https://opus4.kobv.de/opus4-btu/files/7446/AMbl-35_2026.pdf" },
    ],
    // „29-RahmenO BA_5.ÄS+Lesefass": the fifth amendment and the reading version after it.
    reading: 4,
    english: "https://www-docs.b-tu.de/studierende/public/rahmenordnungen/bachelor-en/29-RahmnO%20BA_5-Aenderungssatzung_EN.pdf",
};

const MASTER: Frame = Frame {
    name: |t| t.frame_master,
    statute: Issue { issue: "14/2016", url: "https://www-docs.b-tu.de/studierende/public/files/AMbl-14_2016-RahmenO-MA.pdf" },
    amendments: &[
        Issue { issue: "02/2021", url: "https://opus4.kobv.de/opus4-btu/files/5397/AMbl-2_2021_RahmenO_MA_1.AeS.pdf" },
        Issue { issue: "18/2022", url: "https://opus4.kobv.de/opus4-btu/files/6093/AMbl-18_2022_2.AeS_RahmenO_MA.pdf" },
        Issue { issue: "07/2023", url: "https://opus4.kobv.de/opus4-btu/files/6350/07_3.AES_RahmenO_MA.pdf" },
        Issue { issue: "15/2024", url: "https://opus4.kobv.de/opus4-btu/files/6710/15_4.AS_RahmenO_MA.pdf" },
        Issue { issue: "30/2024", url: "https://opus4.kobv.de/opus4-btu/files/6773/30_RahmenO.pdf" },
        Issue { issue: "38/2026", url: "https://opus4.kobv.de/opus4-btu/files/7450/AMbl-38_2026.pdf" },
    ],
    reading: 4,
    english: "https://www-docs.b-tu.de/studierende/public/rahmenordnungen/master-en/29-RahmnO_MA_5-Aenderungssatzung_EN.pdf",
};

const ENROLMENT: Issue = Issue { issue: "01/2020", url: "https://opus4.kobv.de/opus4-btu/files/5109/AMbl-01_2020_ImmaO.pdf" };
const ENROLMENT_SUSPENSION: Issue = Issue { issue: "10/2022", url: "https://opus4.kobv.de/opus4-btu/files/6002/AMbl-10_2022_ImmaO_Aussetzung_Paragraph_16.pdf" };
const ENROLMENT_ENGLISH: &str = "https://www-docs.b-tu.de/studierende/public/ImmaO/ImmaO_28.01.2020_englih.pdf";
const FUES: &str = "http://opus.kobv.de/btu/volltexte/2010/1748/pdf/04_FAS_Rili_230410.pdf";
const LAW: &str = "https://bravors.brandenburg.de/gesetze/bbghg";

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

/// A Rahmenordnung: the regulation, under it its reading version, the English translation, and
/// its amendments folded.
fn frame_view(frame: &'static Frame, b: &'static BtuTexts) -> AnyView {
    let reading = frame.amendments.get(frame.reading).copied();
    view! {
        <li>
            {row("file-text", (frame.name)(b).to_string(), Some((b.issue)(frame.statute.issue)), frame.statute.url.to_string(), false)}
            {reading.map(|issue| row("file-check-2", (b.reading)(frame.reading + 1), Some((b.issue)(issue.issue)), issue.url.to_string(), true))}
            {row("languages", b.english.to_string(), Some(b.not_binding.to_string()), frame.english.to_string(), true)}
            <details class="st-docs-fold">
                <summary class="docrow docpart">
                    <Icon name="layers"/>
                    <span class="docname">{(b.amendments)(frame.amendments.len())}</span>
                    <Icon name="chevron-down" class="go"/>
                </summary>
                {frame.amendments.iter().enumerate().map(|(at, issue)| row("file-text", (b.amendment)(at + 1), Some((b.issue)(issue.issue)), issue.url.to_string(), true)).collect_view()}
            </details>
        </li>
    }
    .into_any()
}

/// „Ordnungen der BTU".
#[component]
pub(super) fn Statutes(level: Level) -> impl IntoView {
    let t = i18n::t();
    let b = &t.study_btu;
    let frames: Vec<&'static Frame> = match level {
        Level::Bachelor => vec![&BACHELOR],
        Level::Master => vec![&MASTER],
        Level::Both => vec![&BACHELOR, &MASTER],
    };
    view! {
        <section class="panel st-docs" aria-labelledby="st-docs-btu">
            <header class="st-view-head">
                <div>
                    <h2 id="st-docs-btu">{b.statutes}</h2>
                    <p>{b.statutes_lead}</p>
                </div>
            </header>
            <ul class="doclist">
                {frames.into_iter().map(|frame| frame_view(frame, b)).collect_view()}
                <li>
                    {row("file-text", b.enrolment.to_string(), Some((b.issue)(ENROLMENT.issue)), ENROLMENT.url.to_string(), false)}
                    {row("file-text", b.enrolment_suspension.to_string(), Some((b.issue)(ENROLMENT_SUSPENSION.issue)), ENROLMENT_SUSPENSION.url.to_string(), true)}
                    {row("languages", b.english.to_string(), Some(b.not_binding.to_string()), ENROLMENT_ENGLISH.to_string(), true)}
                </li>
                <li>{row("file-text", b.fues.to_string(), Some(b.fues_line.to_string()), FUES.to_string(), false)}</li>
                <li>{row("file-text", b.law.to_string(), Some(b.law_line.to_string()), LAW.to_string(), false)}</li>
            </ul>
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

    /// The issue of a document begins the name of its file (`AMbl-13_2016…`, `29_RahmenO.pdf`),
    /// so a name and an address that drift apart are seen here; the reading version is an amendment.
    #[test]
    fn each_document_is_named_by_its_issue() {
        let issues = [BACHELOR.statute, MASTER.statute, ENROLMENT, ENROLMENT_SUSPENSION].into_iter().chain(BACHELOR.amendments.iter().copied()).chain(MASTER.amendments.iter().copied());
        for Issue { issue, url } in issues {
            let (number, year) = issue.split_once('/').expect("number/year");
            let file = url.rsplit('/').next().unwrap_or_default().trim_start_matches("AMbl-").trim_start_matches("AMBl-");
            let lead: String = file.chars().take_while(char::is_ascii_digit).collect();
            assert_eq!(lead.parse::<u32>().ok(), number.parse::<u32>().ok(), "{url}");
            assert!(!file.contains("_20") || file.contains(&format!("_{year}")), "{url}: {year}");
        }
        assert!(BACHELOR.reading < BACHELOR.amendments.len() && MASTER.reading < MASTER.amendments.len());
    }
}
