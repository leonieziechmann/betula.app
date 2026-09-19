//! The module catalog. The URL is the only filter state: the filter form is a plain GET
//! form (works without JavaScript), sorting, paging and the program tabs are links.

use catalog::filter::{CatalogQuery, ExamPart, KindFilter, Language, PlanSemesterFilter, ProgramRelation, SortKey};
use catalog::labels::{Campus, ExamForm, Labelled, ModuleKind, OfferStatus, TeachingForm, TurnusParity};
use catalog::pages::{self, CatalogData};
use catalog::rows::CatalogRow;
use catalog::url::{self, CatalogUrl, ProgramTab, PAGE_SIZE};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_location;

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::ui::{EmptyState, ErrorState, KindBadge, OfferBadge};

#[component]
pub fn CatalogPage() -> impl IntoView {
    let location = use_location();
    let catalog_url = Memo::new(move |_| CatalogUrl::parse(&location.search.get()));
    let source = use_source();
    let status = PageStatus::capture();
    view! {
        <Title text="Modulkatalog"/>
        {move || {
            let current = catalog_url.get();
            match source.clone().and_then(|source| source.run(|db| pages::catalog(db, &current))) {
                Err(error) => {
                    status.for_error(&error);
                    view! { <ErrorState error/> }.into_any()
                }
                Ok(data) => view! { <Catalog current data/> }.into_any(),
            }
        }}
    }
}

#[component]
fn Catalog(current: CatalogUrl, data: CatalogData) -> impl IntoView {
    let query = current.query.clone();
    let total = data.page.total;
    let pages_total = total.div_ceil(PAGE_SIZE).max(1);
    let with_program = data.program.is_some();
    let unknown_program = query.program.is_some() && !with_program;

    let heading = match &data.program {
        Some(program) => format!("{} · {}", program.name, program.degree()),
        None => "Modulkatalog".to_string(),
    };
    let shown_from = data.page.offset + 1;
    let shown_to = data.page.offset + data.page.rows.len() as u64;

    view! {
        <div class="catalog">
            <Filters current=current.clone() data=data.clone()/>
            <section class="results" aria-live="polite">
                <header class="results-header">
                    <h1>{heading}</h1>
                    <a class="button button-quiet filter-jump" href="#filters">
                        "Filter"{(query.active_filters() > 0).then(|| format!(" ({})", query.active_filters()))}
                    </a>
                    <p class="total">
                        <strong>{format::count(total)}</strong>
                        {if total == 1 { " Modul" } else { " Module" }}
                        {(total > PAGE_SIZE).then(|| format!(" · {}–{} angezeigt", format::count(shown_from), format::count(shown_to)))}
                    </p>
                    {data.program.as_ref().map(|program| {
                        let tab = |relation: ProgramRelation, label: &str, count: Option<u64>| {
                            let mut target = current.with_page(1);
                            if let Some(scope) = target.query.program.as_mut() {
                                scope.relation = relation;
                            }
                            let active = query.program.as_ref().is_some_and(|scope| scope.relation == relation);
                            view! {
                                <a class="tab" class:active=active href=target.path() aria-current=active.then_some("page")>
                                    {label.to_string()}" "<span class="tab-count">{count.map(format::count)}</span>
                                </a>
                            }
                        };
                        view! {
                            <nav class="tabs" aria-label="Listen des Studiengangs">
                                {tab(ProgramRelation::Curricular, "Curriculum", data.curricular_total)}
                                {tab(ProgramRelation::Fues, "FÜS-Module dieses Studiengangs", data.fues_total)}
                                <a class="tab tab-link" href=url::program_path(&program.slug, ProgramTab::Plan)>"Studiengangsseite →"</a>
                            </nav>
                        }
                    })}
                </header>
                {unknown_program.then(|| view! {
                    <EmptyState title="Diesen Studiengang gibt es nicht (mehr)" hint="Wähle links einen anderen Studiengang oder „Alle Studiengänge“."/>
                })}
                {(total == 0 && !unknown_program).then(|| view! {
                    <EmptyState title="Keine Module gefunden" hint="Nimm Filter zurück oder suche nach einem anderen Begriff."/>
                })}
                {(total > 0).then(|| view! { <ModuleTable current=current.clone() rows=data.page.rows.clone() with_program/> })}
                {(pages_total > 1).then(|| {
                    let page = current.page.min(pages_total);
                    view! {
                        <nav class="pager" aria-label="Seiten">
                            {(page > 1).then(|| view! { <a class="button button-quiet" rel="prev" href=current.with_page(page - 1).path()>"← Zurück"</a> })}
                            <span class="pager-position">"Seite "{page}" von "{pages_total}</span>
                            {(page < pages_total).then(|| view! { <a class="button" rel="next" href=current.with_page(page + 1).path()>"Weiter →"</a> })}
                        </nav>
                    }
                })}
                <p class="freshness">
                    {data.meta.data_changed_at.as_deref().map(|at| format!("Datenstand {}", format::date(at)))}
                </p>
            </section>
        </div>
    }
}

#[component]
fn ModuleTable(current: CatalogUrl, rows: Vec<CatalogRow>, with_program: bool) -> impl IntoView {
    let sort_link = |key: SortKey, label: &'static str| {
        let active = current.query.sort == key;
        let mut target = current.with_page(1);
        target.query.descending = active && !current.query.descending;
        target.query.sort = key;
        let direction = match (active, current.query.descending) {
            (true, false) => Some("ascending"),
            (true, true) => Some("descending"),
            _ => None,
        };
        view! {
            <th scope="col" aria-sort=direction>
                <a class="sort" class:active=active href=target.path()>
                    {label}{direction.map(|d| if d == "ascending" { " ↑" } else { " ↓" })}
                </a>
            </th>
        }
    };

    view! {
        <div class="table-scroll">
            <table class="modules">
                <thead>
                    <tr>
                        {sort_link(SortKey::Id, "Nr.")}
                        {sort_link(SortKey::Title, "Modul")}
                        {sort_link(SortKey::Credits, "LP")}
                        <th scope="col">"Turnus"</th>
                        <th scope="col">"Sprache"</th>
                        {with_program.then(|| view! { <th scope="col">"Art"</th><th scope="col">"Semester"</th> })}
                        {sort_link(SortKey::Events, "Termine")}
                    </tr>
                </thead>
                <tbody>
                    {rows.into_iter().map(|row| {
                        let other_title = match (&row.title_de, &row.title_en) {
                            (Some(de), Some(en)) if de != en => Some(if row.title == *de { en.clone() } else { de.clone() }),
                            _ => None,
                        };
                        let language = format::languages(row.teaches_german, row.teaches_english);
                        view! {
                            <tr>
                                <td class="cell-id">{row.id.clone()}</td>
                                <td class="cell-title">
                                    <a href=url::module_path(&row.id)>{row.title.clone()}</a>
                                    {other_title.map(|title| view! { <span class="subtitle">{title}</span> })}
                                    <span class="badges">
                                        <OfferBadge status=row.offer_status.clone()/>
                                        {row.is_fues.then(|| view! { <span class="badge badge-fues">"FÜS"</span> })}
                                        {(row.is_limited == Some(true)).then(|| view! { <span class="badge">"begrenzte Plätze"</span> })}
                                    </span>
                                </td>
                                <td class="cell-number">{row.credits.map(format::number)}</td>
                                <td>{format::turnus(row.turnus_season.as_ref(), row.turnus_parity.as_ref())}</td>
                                <td class:unknown=language.is_none()>{language.unwrap_or("k. A.")}</td>
                                {with_program.then(|| view! {
                                    <td><KindBadge kind=row.kind.clone()/></td>
                                    <td class="cell-number" class:unknown=row.plan_semester.is_none()>
                                        {row.plan_semester.map(|n| format!("{n}.")).unwrap_or_else(|| "–".to_string())}
                                    </td>
                                })}
                                <td class="cell-number">{(row.teaching_events > 0).then_some(row.teaching_events)}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

/// The filter sidebar: a GET form whose field names are the URL parameters of `CatalogUrl`.
#[component]
fn Filters(current: CatalogUrl, data: CatalogData) -> impl IntoView {
    let q: CatalogQuery = current.query.clone();
    let scope = q.program.clone();
    let active = q.active_filters();

    let check = |name: &'static str, value: &'static str, label: String, checked: bool| {
        view! {
            <label class="check">
                <input type="checkbox" name=name value=value checked=checked/>
                <span>{label}</span>
            </label>
        }
    };
    let option = |value: String, label: String, selected: bool| view! { <option value=value selected=selected>{label}</option> };
    let tri = |name: &'static str, label: &'static str, state: Option<bool>, yes: (&'static str, &'static str), no: (&'static str, &'static str)| {
        view! {
            <label class="field">
                <span>{label}</span>
                <select name=name>
                    {option(String::new(), "egal".to_string(), state.is_none())}
                    {option(yes.0.to_string(), yes.1.to_string(), state == Some(true))}
                    {option(no.0.to_string(), no.1.to_string(), state == Some(false))}
                </select>
            </label>
        }
    };

    let has_plan = data.program.as_ref().is_some_and(|p| p.has_plan);
    let selected_slug = scope.as_ref().map(|s| s.program_slug.clone()).unwrap_or_default();
    let kinds = scope.as_ref().map(|s| s.kinds.clone()).unwrap_or_default();
    let semester = scope.as_ref().and_then(|s| s.plan_semester);
    let fues_list = scope.as_ref().is_some_and(|s| s.relation == ProgramRelation::Fues);
    let sort_code = match q.sort {
        SortKey::Title => None,
        SortKey::Id => Some("id"),
        SortKey::Credits => Some("ects"),
        SortKey::Events => Some("events"),
    };
    let show_all_status = q.offer.as_ref().is_some_and(|offer| offer.contains(&OfferStatus::NotOffered));

    view! {
        <aside class="filters" id="filters" aria-label="Filter">
            <form method="get" action=url::CATALOG>
                // What the links set and the form has to keep.
                {fues_list.then(|| view! { <input type="hidden" name="list" value="fues"/> })}
                {sort_code.map(|code| view! { <input type="hidden" name="sort" value=code/> })}
                {q.descending.then(|| view! { <input type="hidden" name="desc" value="1"/> })}

                <label class="field">
                    <span>"Suche"</span>
                    <input type="search" name="q" value=q.text.clone() placeholder="Titel oder Modulnummer"/>
                </label>

                {view! { <details class="group" open>
                    <summary>"Studiengang"</summary>
                    <label class="field">
                        <span>"Studiengang & Prüfungsordnung"</span>
                        <select name="program">
                            {option(String::new(), "Alle Studiengänge".to_string(), selected_slug.is_empty())}
                            {data.programs.iter().map(|p| {
                                let label = format!("{} · {} · PO {}", p.name, p.degree(), p.po_version);
                                option(p.slug.clone(), label, p.slug == selected_slug)
                            }).collect_view()}
                        </select>
                    </label>
                    {data.program.is_some().then(|| view! {
                        <fieldset class="checks">
                            <legend>"Modulart"</legend>
                            {ModuleKind::ALL.iter().map(|kind| check("kind", kind.code(), kind.label().to_string(), kinds.contains(&KindFilter::Stated(*kind)))).collect_view()}
                            {check("kind", "none", "Art nicht angegeben".to_string(), kinds.contains(&KindFilter::Unstated))}
                        </fieldset>
                        {if has_plan {
                            view! {
                                <label class="field">
                                    <span>"Fachsemester laut Regelstudienplan"</span>
                                    <select name="semester">
                                        {option(String::new(), "alle".to_string(), semester.is_none())}
                                        {(1u8..=10).map(|n| option(n.to_string(), format!("{n}. Semester"), semester == Some(PlanSemesterFilter::Semester(n)))).collect_view()}
                                        {option("none".to_string(), "ohne Angabe im Plan".to_string(), semester == Some(PlanSemesterFilter::Unstated))}
                                    </select>
                                </label>
                            }.into_any()
                        } else {
                            view! { <p class="hint">"Für diesen Studiengang liegt kein geprüfter Regelstudienplan vor; Fachsemester sind deshalb nicht bekannt."</p> }.into_any()
                        }}
                    })}
                </details> }.into_any()}

                {view! { <details class="group" open=!q.lecturers_include.is_empty() || !q.lecturers_exclude.is_empty() || q.department_id.is_some()>
                    <summary>"Dozierende"</summary>
                    <label class="field">
                        <span>"Lehrt oder verantwortet"</span>
                        <input type="text" name="lecturer" list="lecturers" value=q.lecturers_include.first().cloned().unwrap_or_default() placeholder="Nachname, Vorname"/>
                    </label>
                    {q.lecturers_include.iter().skip(1).map(|name| view! { <input type="hidden" name="lecturer" value=name.clone()/> }).collect_view()}
                    <label class="field">
                        <span>"Nicht bei"</span>
                        <input type="text" name="not-lecturer" list="lecturers" value=q.lecturers_exclude.first().cloned().unwrap_or_default() placeholder="Nachname, Vorname"/>
                    </label>
                    {q.lecturers_exclude.iter().skip(1).map(|name| view! { <input type="hidden" name="not-lecturer" value=name.clone()/> }).collect_view()}
                    <datalist id="lecturers">
                        {data.lecturers.iter().map(|l| view! { <option value=l.name.clone()></option> }).collect_view()}
                    </datalist>
                    <label class="field">
                        <span>"Fachgebiet"</span>
                        <select name="department">
                            {option(String::new(), "alle".to_string(), q.department_id.is_none())}
                            {data.departments.iter().map(|d| option(d.id.to_string(), format!("{} ({})", d.label, d.modules), q.department_id == Some(d.id))).collect_view()}
                        </select>
                    </label>
                </details> }.into_any()}

                {view! { <details class="group" open>
                    <summary>"Turnus & Lehrformen"</summary>
                    <fieldset class="checks">
                        <legend>"Angeboten im"</legend>
                        {check("turnus", "winter", "Wintersemester".to_string(), q.turnus.winter)}
                        {check("turnus", "summer", "Sommersemester".to_string(), q.turnus.summer)}
                        {check("turnus", "irregular", "unregelmäßig".to_string(), q.turnus.irregular)}
                    </fieldset>
                    <label class="field">
                        <span>"Nur in bestimmten Jahren"</span>
                        <select name="years">
                            {option(String::new(), "egal".to_string(), q.turnus.year_parity.is_none())}
                            {option(TurnusParity::Even.code().to_string(), "in geraden Jahren angeboten".to_string(), q.turnus.year_parity == Some(TurnusParity::Even))}
                            {option(TurnusParity::Odd.code().to_string(), "in ungeraden Jahren angeboten".to_string(), q.turnus.year_parity == Some(TurnusParity::Odd))}
                        </select>
                    </label>
                    <fieldset class="checks">
                        <legend>"Lehrformen"</legend>
                        {[TeachingForm::Lecture, TeachingForm::Exercise, TeachingForm::Seminar, TeachingForm::Practical, TeachingForm::Project, TeachingForm::Excursion]
                            .iter().map(|form| check("form", form.code(), form.label().to_string(), q.teaching_forms.contains(form))).collect_view()}
                    </fieldset>
                    <label class="field">
                        <span>"Dauer"</span>
                        <select name="duration">
                            {option(String::new(), "egal".to_string(), q.duration_semesters.is_none())}
                            {option("1".to_string(), "1 Semester".to_string(), q.duration_semesters == Some(1))}
                            {option("2".to_string(), "2 Semester".to_string(), q.duration_semesters == Some(2))}
                        </select>
                    </label>
                </details> }.into_any()}

                {view! { <details class="group" open=!q.exam_forms.is_empty() || !q.exam_parts.is_empty() || q.graded.is_some() || q.limited.is_some() || q.fues.is_some() || q.credits_min.is_some() || q.credits_max.is_some() || q.offer.is_some()>
                    <summary>"Kriterien"</summary>
                    <fieldset class="checks">
                        <legend>"Prüfung"</legend>
                        {ExamForm::ALL.iter().map(|form| check("exam", form.code(), form.label().to_string(), q.exam_forms.contains(form))).collect_view()}
                        {ExamPart::ALL.iter().map(|part| check("exam", part.code(), part.label().to_string(), q.exam_parts.contains(part))).collect_view()}
                    </fieldset>
                    {tri("graded", "Benotung", q.graded, ("yes", "benotet"), ("no", "unbenotet"))}
                    {tri("limited", "Teilnehmerbegrenzung", q.limited, ("yes", "nur begrenzte Module"), ("no", "nur unbegrenzte Module"))}
                    {tri("fues", "Allgemeine FÜS-Liste", q.fues, ("only", "nur FÜS-Module"), ("none", "keine FÜS-Module"))}
                    <div class="field field-range">
                        <span>"Leistungspunkte"</span>
                        <input type="number" name="ects_min" min="0" max="60" step="0.5" inputmode="decimal" aria-label="Leistungspunkte mindestens" placeholder="von" value=q.credits_min.map(|n| n.to_string())/>
                        <input type="number" name="ects_max" min="0" max="60" step="0.5" inputmode="decimal" aria-label="Leistungspunkte höchstens" placeholder="bis" value=q.credits_max.map(|n| n.to_string())/>
                    </div>
                    {data.program.is_none().then(|| check("status", "all", "auch nicht mehr angebotene Module anzeigen".to_string(), show_all_status))}
                </details> }.into_any()}

                {view! { <details class="group" open=!q.campuses.is_empty() || !q.languages.is_empty()>
                    <summary>"Standort & Sprache"</summary>
                    <fieldset class="checks">
                        <legend>"Sprache"</legend>
                        {check("lang", "de", "Deutsch".to_string(), q.languages.contains(&Language::German))}
                        {check("lang", "en", "Englisch".to_string(), q.languages.contains(&Language::English))}
                    </fieldset>
                    <fieldset class="checks">
                        <legend>"Standort"</legend>
                        {[Campus::Zentralcampus, Campus::Sachsendorf, Campus::Senftenberg]
                            .iter().map(|campus| check("campus", campus.code(), campus.label().to_string(), q.campuses.contains(campus))).collect_view()}
                        <p class="hint">"Der Standort ist nur für Module bekannt, die in diesem Semester Raumangaben haben. Dieser Filter zeigt deshalb nur solche Module."</p>
                    </fieldset>
                </details> }.into_any()}

                <div class="filter-actions">
                    <button class="button" type="submit">"Filter anwenden"</button>
                    {(active > 0).then(|| view! { <a class="button button-quiet" href=url::CATALOG>"Zurücksetzen ("{active}")"</a> })}
                </div>
            </form>
        </aside>
    }
}
