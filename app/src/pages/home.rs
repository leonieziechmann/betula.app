//! The landing page. It has to answer three questions at a glance: what is this (the catalog of
//! one university, unofficial), what is in it (numbers, and the map of the programs as the one
//! picture of the page), and where do I start (the search, entry links with their counts, the
//! faculties). Below that it says in plain text what the app does and answers what people ask
//! search engines about the modules of the BTU: that text is what the page is found by.
//!
//! The map (`catalog::graph`) is laid out by the web server once per snapshot; this page only
//! draws it (`data::ProgramMapHandle`). Without JavaScript it is a picture whose dots are links;
//! the app adds what a pointer over a dot shows: the program's relatives.

use std::sync::Arc;

use catalog::filter::{ExamPart, Language, TurnusFilter};
use catalog::graph::{Cycle, Layout, ProgramMap};
use catalog::labels::Campus;
use catalog::pages::{self, HomeData};
use catalog::url::{self, CatalogUrl, ProgramTab};
use catalog::CatalogQuery;
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_navigate;

use crate::data::{use_source, PageStatus, ProgramMapHandle};
use crate::format;
use crate::nav;
use crate::seo::{self, Seo};
use crate::ui::{ErrorState, Frame, Icon, Mark, Wordmark};

const DESCRIPTION: &str = "Alle Module und Studiengänge der BTU Cottbus-Senftenberg an einem Ort: durchsuchen, nach Turnus, Prüfung, Sprache und Studiengang filtern, Regelstudienpläne und Voraussetzungen ansehen. Inoffiziell, kostenlos, mit Link zum Original.";

/// A way into the catalog: a filter people come for, with the number of modules behind it.
struct Entry {
    label: &'static str,
    hint: &'static str,
    icon: &'static str,
    query: CatalogQuery,
}

fn entries() -> Vec<Entry> {
    let entry = |label, hint, icon, query| Entry { label, hint, icon, query };
    vec![
        entry("Im Wintersemester", "Module mit Turnus Wintersemester", "snowflake", CatalogQuery { turnus: TurnusFilter { winter: true, ..Default::default() }, ..Default::default() }),
        entry("Im Sommersemester", "Module mit Turnus Sommersemester", "sun", CatalogQuery { turnus: TurnusFilter { summer: true, ..Default::default() }, ..Default::default() }),
        entry("Auf Englisch", "Module, die auf Englisch gelehrt werden", "languages", CatalogQuery { languages: vec![Language::English], ..Default::default() }),
        entry("Fachübergreifendes Studium", "FÜS-Module aller Fakultäten", "shuffle", CatalogQuery { fues: Some(true), ..Default::default() }),
        entry("Ohne Klausur", "Module, deren Prüfung keine Klausur nennt", "file-check-2", CatalogQuery { exam_parts_exclude: vec![ExamPart::Written], ..Default::default() }),
        entry("Mit mündlicher Prüfung", "Module mit mündlicher Prüfung", "users-round", CatalogQuery { exam_parts: vec![ExamPart::Oral], ..Default::default() }),
        entry("In Senftenberg", "Module am Campus Senftenberg", "map-pin", CatalogQuery { campuses: vec![Campus::Senftenberg], ..Default::default() }),
        entry("Unbenotet", "Module, die ohne Note abgeschlossen werden", "circle-check-big", CatalogQuery { graded: Some(false), ..Default::default() }),
    ]
}

/// Searches that show what the search understands: a topic, a number, an abbreviation.
const EXAMPLES: [&str; 5] = ["Lineare Algebra", "Machine Learning", "Thermodynamik", "Datenbanken", "11101"];

const ABILITIES: [(&str, &str, &str); 6] = [
    ("search", "Ein Suchfeld für alle Module", "Deutscher und englischer Titel und die Modulnummer in einer Suche, über alle Fakultäten. Die Treffer stehen da, während du tippst."),
    ("sliders-horizontal", "Filter, die zusammenpassen", "Studiengang, Turnus, Lehrform, Prüfungsform, Sprache, Campus, Leistungspunkte, Dozierende. Viele lassen sich auch umkehren: „alles außer Klausur“."),
    ("calendar-range", "Der Regelstudienplan als Plan", "Semester für Semester, mit Pflicht- und Wahlpflichtbereichen, der FÜS-Liste und den Ordnungen des Studiengangs."),
    ("repeat", "Voraussetzungen zum Anklicken", "Was ein Modul voraussetzt und wofür es selbst Voraussetzung ist, führt direkt zum nächsten Modul."),
    ("calendar-days", "Termine aus dem Vorlesungsverzeichnis", "Vorlesungen und Übungen des Semesters stehen als Wochenplan beim Modul, die Prüfungstermine gleich darunter."),
    ("shield-check", "Ehrlich bei Lücken", "Wo die Quelle nichts sagt, steht „nicht angegeben“ und keine Vermutung. Jedes Modul verlinkt auf sein Original bei der BTU."),
];

const QUESTIONS: [(&str, &str); 6] = [
    (
        "Ist Betula ein Angebot der BTU?",
        "Nein. Betula ist ein inoffizielles, unabhängiges Projekt und gehört nicht zur BTU Cottbus-Senftenberg. Verbindlich sind allein die Modulbeschreibungen, Prüfungs- und Studienordnungen der Universität; jede Seite hier verlinkt deshalb auf ihr Original.",
    ),
    (
        "Woher kommen die Daten?",
        "Aus den öffentlichen Modulbeschreibungen, den Studiengangsseiten und dem Vorlesungsverzeichnis der BTU. Betula liest sie regelmäßig ein, ordnet sie und macht sie durchsuchbar. Am Inhalt wird nichts geändert und nichts ergänzt.",
    ),
    (
        "Was ist ein Modul, und was steht in einer Modulbeschreibung?",
        "Ein Modul ist eine abgeschlossene Lehreinheit, meist über ein Semester, für die es Leistungspunkte (LP) gibt. Die Modulbeschreibung nennt Inhalte und Lernziele, Lehrformen und Umfang, Voraussetzungen, die Prüfungsleistung, den Turnus und die Verantwortlichen. Zusammen bilden die Beschreibungen das Modulhandbuch eines Studiengangs.",
    ),
    (
        "Was bedeutet FÜS?",
        "FÜS steht für das fachübergreifende Studium: Module außerhalb des eigenen Fachs, die viele Studiengänge der BTU in einem bestimmten Umfang verlangen. Im Katalog lassen sich FÜS-Module eigens filtern, und jeder Studiengang zeigt die Liste, die er anerkennt.",
    ),
    (
        "Wie aktuell ist der Katalog?",
        "Der Datenstand steht auf dieser Seite und wird mit jedem Einlesen erneuert. Kurzfristige Änderungen, etwa verlegte Termine, stehen zuerst bei der BTU; im Zweifel gilt das Original.",
    ),
    (
        "Kostet das etwas, brauche ich ein Konto?",
        "Nein. Betula ist kostenlos, ohne Anmeldung und ohne Werbung. Der Katalog wird einmal in deinen Browser geladen; danach antworten Suche und Filter ohne Wartezeit.",
    ),
];

#[component]
pub fn HomePage() -> impl IntoView {
    let source = use_source();
    let status = PageStatus::capture();
    let entries = entries();
    let queries: Vec<CatalogQuery> = entries.iter().map(|entry| entry.query.clone()).collect();
    let loaded = source.and_then(|source| source.run(|db| pages::home(db, &queries)));
    let map = use_context::<ProgramMapHandle>().map(|handle| handle.0);

    let sidebar = {
        let facts = loaded.clone().ok();
        let has_map = map.is_some();
        move || view! {
            <nav class="toc jumps" aria-label="Auf dieser Seite">
                <p class="flabel label">"Auf dieser Seite"</p>
                {has_map.then(|| view! { <a href="#karte" data-action="jump">"Karte der Studiengänge"</a> })}
                <a href="#einstiege" data-action="jump">"Einstiege"</a>
                <a href="#funktionen" data-action="jump">"Was Betula kann"</a>
                <a href="#fragen" data-action="jump">"Fragen und Antworten"</a>
            </nav>
            {facts.clone().map(|home| view! {
                <div class="fgroup">
                    <p class="flabel label">"Datenstand"</p>
                    <dl class="side-facts">
                        {home.overview.current_semester.as_ref().map(|s| view! { <div><dt>"Aktuelles Semester"</dt><dd>{s.label.clone()}</dd></div> })}
                        {home.overview.meta.data_changed_at.as_deref().map(|at| view! { <div><dt>"Daten zuletzt geändert"</dt><dd>{format::date(at)}</dd></div> })}
                    </dl>
                    <p class="hint">"Quelle: Modulbeschreibungen und Vorlesungsverzeichnis der BTU Cottbus-Senftenberg."</p>
                    <p class="hint">"Betula ist ein inoffizielles Projekt und gehört nicht zur BTU."</p>
                </div>
            })}
        }
    };

    let data = vec![
        serde_json::json!({
            "@type": "WebSite",
            "@id": seo::absolute("/#website"),
            "url": seo::absolute(url::HOME),
            "name": seo::SITE_NAME,
            "alternateName": ["Betula Modulkatalog", "Modulkatalog BTU Cottbus-Senftenberg (inoffiziell)"],
            "description": DESCRIPTION,
            "inLanguage": "de",
            "about": { "@type": "CollegeOrUniversity", "name": seo::UNIVERSITY, "alternateName": "BTU Cottbus-Senftenberg", "url": seo::UNIVERSITY_URL },
            "potentialAction": {
                "@type": "SearchAction",
                "target": { "@type": "EntryPoint", "urlTemplate": format!("{}?q={{search_term_string}}", seo::absolute(url::CATALOG)) },
                "query-input": "required name=search_term_string",
            },
        }),
        serde_json::json!({
            "@type": "FAQPage",
            "mainEntity": QUESTIONS.iter().map(|(question, answer)| serde_json::json!({
                "@type": "Question",
                "name": question,
                "acceptedAnswer": { "@type": "Answer", "text": answer },
            })).collect::<Vec<_>>(),
        }),
    ];

    view! {
        <Title text=""/>
        <Frame title="Start" sidebar><div class="page-inner home">
        <Seo title="Betula · Modulkatalog für die BTU Cottbus-Senftenberg" description=DESCRIPTION path=url::HOME data/>
        {match loaded {
            Err(error) => {
                status.for_error(&error);
                view! { <Intro home=None/><ErrorState error/> }.into_any()
            }
            Ok(home) => view! {
                <Intro home=Some(home.clone())/>
                {map.map(|map| view! { <MapSection map/> })}
                <Entries entries home=home.clone()/>
            }.into_any(),
        }}
        <section class="panel abilities" id="funktionen" aria-labelledby="funktionen-titel">
            <header class="block-head">
                <h2 id="funktionen-titel">"Was Betula kann"</h2>
                <p>"Dieselben Daten wie bei der BTU, so aufbereitet, dass man mit ihnen planen kann."</p>
            </header>
            <ul class="ability-grid">
                {ABILITIES.iter().map(|(icon, title, text)| view! {
                    <li class="ability">
                        <span class="ico"><Icon name=icon/></span>
                        <h3>{*title}</h3>
                        <p>{*text}</p>
                    </li>
                }).collect_view()}
            </ul>
            <p class="soon-line">
                <span class="label">"In Arbeit"</span>
                <span><Icon name="bookmark"/>"Merkliste und Studienverlauf: Module merken, bestandene abhaken"</span>
                <span><Icon name="calendar-range"/>"Semesterplaner: der eigene Stundenplan aus den Terminen"</span>
            </p>
        </section>
        <section class="panel questions" id="fragen" aria-labelledby="fragen-titel">
            <header class="block-head">
                <h2 id="fragen-titel">"Fragen und Antworten"</h2>
            </header>
            <dl class="qa">
                {QUESTIONS.iter().map(|(question, answer)| view! {
                    <div class="qa-row">
                        <dt>{*question}</dt>
                        <dd>{*answer}</dd>
                    </div>
                }).collect_view()}
            </dl>
            <p class="source">
                "Betula · inoffizieller Modulkatalog für die BTU Cottbus-Senftenberg"
                <a href=seo::UNIVERSITY_URL rel="noopener">"Zur BTU"<Icon name="arrow-up-right"/></a>
            </p>
        </section>
        </div></Frame>
    }
}

/// What this is, the way in, and how much is in it.
#[component]
fn Intro(home: Option<HomeData>) -> impl IntoView {
    let figures = home.map(|home| {
        // The faculties are the numbered ones; centres that offer programs too are listed below.
        let faculties = home.faculties.iter().filter(|(department, _)| department.as_ref().is_some_and(|d| d.code.chars().all(|c| c.is_ascii_digit()))).count();
        let semester = home.overview.current_semester.clone();
        view! {
            <dl class="figures">
                <div><dt>"Module im Angebot"</dt><dd class="num">{format::count(home.overview.modules)}</dd></div>
                <div><dt>"Studiengänge"</dt><dd class="num">{format::count(home.overview.programs)}</dd></div>
                <div><dt>"Fakultäten"</dt><dd class="num">{faculties}</dd></div>
                {semester.map(|semester| view! {
                    <div><dt>"Veranstaltungen im "{semester.label.clone()}</dt><dd class="num">{format::count(semester.teaching_events.max(0) as u64)}</dd></div>
                })}
            </dl>
        }
    });
    view! {
        <section class="panel intro">
            <div class="intro-text">
                <p class="brand-phone"><span class="logo"><Mark/></span><span><Wordmark small=true/><small>"Modulkatalog · inoffiziell"</small></span></p>
                <p class="eyebrow">"Inoffizieller Modulkatalog · BTU Cottbus-Senftenberg"</p>
                <h1>"Alle Module und Studiengänge der BTU Cottbus-Senftenberg, an einem Ort."</h1>
                <p class="lead">
                    "Betula macht das Studienangebot der BTU durchsuchbar: Module mit Inhalten, Voraussetzungen, Prüfungsform und Terminen, "
                    "Studiengänge mit Regelstudienplan und Wahlpflichtbereichen. Filtern statt Modulhandbücher wälzen."
                </p>
                <p class="intro-actions">
                    <a class="btn primary" href=url::CATALOG><Icon name="layout-list"/>"Module durchsuchen"</a>
                    <a class="btn secondary" href=url::PROGRAMS><Icon name="graduation-cap"/>"Studiengang wählen"</a>
                </p>
                <p class="examples">
                    <span class="label">"Zum Beispiel"</span>
                    {EXAMPLES.iter().map(|text| {
                        let href = CatalogUrl { query: CatalogQuery { text: text.to_string(), ..Default::default() }, ..Default::default() }.path();
                        view! { <a class="example" href=href rel="nofollow"><Icon name="search"/>{*text}</a> }
                    }).collect_view()}
                </p>
            </div>
            {figures}
        </section>
    }
}

/// The entry links with their counts, and the faculties: two lists on the same lines.
#[component]
fn Entries(entries: Vec<Entry>, home: HomeData) -> impl IntoView {
    let counts = home.entry_counts.clone();
    view! {
        <div class="home-lists" id="einstiege">
            <section class="panel linklist-panel" aria-labelledby="einstiege-titel">
                <header class="block-head">
                    <h2 id="einstiege-titel">"Einstiege in den Katalog"</h2>
                    <a class="ghost" href=url::CATALOG>"Alle "{format::count(home.overview.modules)}" Module"<Icon name="chevron-right"/></a>
                </header>
                <ul class="rowlist">
                    {entries.into_iter().zip(counts).map(|(entry, count)| {
                        let href = CatalogUrl { query: entry.query, ..Default::default() }.path();
                        view! {
                            <li><a class="rowlink" href=href rel="nofollow">
                                <span class="ico"><Icon name=entry.icon/></span>
                                <span class="rowlink-text"><b>{entry.label}</b><small>{entry.hint}</small></span>
                                <span class="rowlink-count num">{format::count(count)}</span>
                                <Icon name="chevron-right"/>
                            </a></li>
                        }
                    }).collect_view()}
                </ul>
            </section>
            <section class="panel linklist-panel" aria-labelledby="fakultaeten-titel">
                <header class="block-head">
                    <h2 id="fakultaeten-titel">"Studiengänge nach Fakultät"</h2>
                    <a class="ghost" href=url::PROGRAMS>"Alle "{format::count(home.overview.programs)}" Studiengänge"<Icon name="chevron-right"/></a>
                </header>
                <ul class="rowlist">
                    {home.faculties.iter().map(|(department, programs)| {
                        let (code, name, anchor) = match department {
                            Some(d) if d.code.chars().all(|c| c.is_ascii_digit()) => (format!("F{}", d.code), d.name_de.clone(), format!("fakultaet-{}", d.id)),
                            Some(d) => (d.code.clone(), d.name_de.clone(), format!("fakultaet-{}", d.id)),
                            None => ("–".to_string(), "Fakultätsübergreifend oder nicht eindeutig zuzuordnen".to_string(), "ohne-fakultaet".to_string()),
                        };
                        let full_name = name.clone();
                        view! {
                            <li><a class="rowlink" href=format!("{}#{anchor}", url::PROGRAMS) title=full_name>
                                <span class="ico code">{code}</span>
                                <span class="rowlink-text"><b>{name}</b></span>
                                <span class="rowlink-count num">{*programs}</span>
                                <Icon name="chevron-right"/>
                            </a></li>
                        }
                    }).collect_view()}
                </ul>
            </section>
        </div>
    }
}

/// A coordinate as the SVG gets it: one decimal.
fn px(value: f64) -> String {
    ((value * 10.0).round() / 10.0).to_string()
}

/// The links of a layout as one path per weight: three elements instead of three hundred.
fn link_paths(map: &ProgramMap, layout: &Layout) -> [String; 3] {
    let mut paths = [String::new(), String::new(), String::new()];
    for link in &map.links {
        let (Some(a), Some(b)) = (layout.dots.get(link.a), layout.dots.get(link.b)) else { continue };
        let weight = if link.similarity >= 0.5 { 2 } else if link.similarity >= 0.2 { 1 } else { 0 };
        if let Some(path) = paths.get_mut(weight) {
            path.push_str(&format!("M{} {}L{} {}", a.0, a.1, b.0, b.1));
        }
    }
    paths
}

#[component]
fn MapSection(map: Arc<ProgramMap>) -> impl IntoView {
    // The program under the pointer or with the focus (the app only; server HTML has no state).
    let hot = RwSignal::new(None::<usize>);
    let navigate = use_navigate();
    let slugs: Vec<String> = map.programs.iter().map(|program| program.slug.clone()).collect();
    // The router takes clicks on HTML links only; the dots are SVG links.
    let on_click = move |ev: leptos::ev::MouseEvent| {
        if ev.button() != 0 || ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key() {
            return;
        }
        let Some(slug) = nav::index_under(ev.target()).and_then(|i| slugs.get(i).cloned()) else { return };
        ev.prevent_default();
        navigate(&url::program_path(&slug, ProgramTab::Plan), Default::default());
    };

    let info = {
        let map = map.clone();
        move || {
            let Some((i, program)) = hot.get().and_then(|i| map.programs.get(i).map(|program| (i, program))) else {
                return view! {
                    <span class="map-legend">
                        <span><i class="dot-key bachelor"></i>"Bachelor"</span>
                        <span><i class="dot-key master"></i>"Master"</span>
                        <span><i class="dot-key other"></i>"Weitere"</span>
                        <span class="quiet">"Größe: Module im Curriculum · Linie: gemeinsame Module"</span>
                    </span>
                }.into_any();
            };
            let relatives: Vec<String> = map
                .relatives(i)
                .into_iter()
                .take(3)
                .filter_map(|(other, shared)| map.programs.get(other).map(|other| format!("{shared} mit {}", other.title())))
                .collect();
            view! {
                <span class="map-hot-info">
                    <b>{program.name.clone()}</b>" "{program.degree.clone()}{program.variant.as_ref().map(|variant| format!(" ({variant})"))}
                    <span class="quiet">" · "{program.modules}" Module"{(!relatives.is_empty()).then(|| format!(" · teilt {}", relatives.join(", ")))}</span>
                </span>
            }.into_any()
        }
    };

    let sheets = [("map-wide", map.wide.clone()), ("map-tall", map.tall.clone())].map(|(class, layout)| {
        let paths = link_paths(&map, &layout);
        let hot_links = {
            let (map, layout) = (map.clone(), layout.clone());
            move || {
                let i = hot.get()?;
                let from = *layout.dots.get(i)?;
                let mut path = String::new();
                let mut relatives = Vec::new();
                for (other, _) in map.relatives(i) {
                    if let (Some(to), Some(program)) = (layout.dots.get(other), map.programs.get(other)) {
                        path.push_str(&format!("M{} {}L{} {}", from.0, from.1, to.0, to.1));
                        relatives.push((*to, program.cycle.code()));
                    }
                }
                // Drawn over the map (which steps back): the links, the relatives as they are, the program itself.
                Some(view! {
                    <g class="map-hot">
                        <path d=path/>
                        {relatives.into_iter().map(|((x, y, r), cycle)| view! { <g><circle class=cycle cx=px(x) cy=px(y) r=px(r)/></g> }).collect_view()}
                        <circle class="self" cx=px(from.0) cy=px(from.1) r=px(from.2)/>
                    </g>
                })
            }
        };
        let [weak, medium, strong] = paths;
        view! {
            <svg class=format!("map {class}") class:has-hot=move || hot.get().is_some() viewBox=format!("0 0 {} {}", layout.width, layout.height) style=format!("--map-font:{}px", layout.font)>
                <path class="map-links" d=weak/>
                <path class="map-links medium" d=medium/>
                <path class="map-links strong" d=strong/>
                // Virtual oversizing (R14): every dot takes the pointer 5 units around it. The halos lie
                // under all dots, so in a tight group a dot is never covered by its neighbour's halo.
                {map.programs.iter().zip(layout.dots.iter()).enumerate().map(|(i, (program, (x, y, r)))| view! {
                    <g>
                        <a class="map-halo" href=url::program_path(&program.slug, ProgramTab::Plan) data-i=i tabindex="-1" aria-hidden="true">
                            <circle cx=px(*x) cy=px(*y) r=px(*r + 5.0)/>
                        </a>
                    </g>
                }).collect_view()}
                {map.programs.iter().zip(layout.dots.iter()).enumerate().map(|(i, (program, (x, y, r)))| {
                    let label = format!("{} · {} Module", program.title(), program.modules);
                    let tooltip = label.clone();
                    let class = match program.cycle {
                        Cycle::Bachelor => "map-dot bachelor",
                        Cycle::Master => "map-dot master",
                        Cycle::Other => "map-dot other",
                    };
                    view! {
                        <g>
                            <a class=class href=url::program_path(&program.slug, ProgramTab::Plan) data-i=i aria-label=label>
                                <title>{tooltip}</title>
                                <circle cx=px(*x) cy=px(*y) r=px(*r)/>
                            </a>
                        </g>
                    }
                }).collect_view()}
                {hot_links}
                {layout.names.iter().filter_map(|name| map.programs.get(name.program).map(|program| view! {
                    <g><text class="map-name" x=px(name.x) y=px(name.y) text-anchor=name.anchor.code()>{program.name.clone()}</text></g>
                })).collect_view()}
            </svg>
        }
    });

    view! {
        <section class="panel home-map" id="karte" aria-labelledby="karte-titel">
            <header class="block-head">
                <h2 id="karte-titel">"Wie die Studiengänge zusammenhängen"</h2>
                <p>
                    "Jeder Punkt ist ein Studiengang. Eine Linie verbindet zwei, deren Curricula sich Module teilen; "
                    "je mehr sie teilen, desto näher stehen sie beieinander. So werden die Nachbarschaften der Universität sichtbar."
                </p>
            </header>
            <p class="map-info" aria-live="polite">{info}</p>
            <div
                class="map-sheet"
                on:click=on_click
                on:pointerover=move |ev| hot.set(nav::index_under(ev.target()))
                on:pointerleave=move |_| hot.set(None)
                on:focusin=move |ev| hot.set(nav::index_under(ev.target()))
                on:focusout=move |_| hot.set(None)
            >
                {sheets}
            </div>
        </section>
    }
}
