//! „Ordnungen & Dokumente", a view of „Mein Studium" (`/study?view=documents`; 2026-11-10: „wenn
//! man bei Mein studium auf Ordnungen und dokumente geht, dann wird da so viel unnötiges zeug
//! angezeigt, das man ja schon in anderen punkten sehen kann … so dass man da nur die dokumente
//! sehen kann und noch so links zu zeug"). Until then its way led to the program's whole page, the
//! regulations at its end, under the plan and the modules „Mein Studium" shows already.
//!
//! The program's regulations (`folia_pages::study_documents`), each a link to its PDF with what it
//! is; under the one the Regelstudienplan was read from, a link to the pages it stands on
//! (`#page=`, where the browsers' viewers of a PDF open it). Then „Mehr zum Studiengang": its
//! module tree in the BTU's course catalogue, its modules in the catalog, its page as everybody
//! sees it; then what the BTU has for every program (`btu.rs`): its regulations, the Rahmenordnung
//! of the student's degree first, and its pages around the study. On a desktop alone in the page, on a phone a page of its own behind the overview (the
//! way back at the head, `regular::view_back`).

use folia_pages::ask::StudyDocumentsAsk;
use folia_pages::StudyDocuments;
use folia_model::labels::DegreeLevel;
use folia_routes::url::{self, ProgramTab, ProgramUrl};
use leptos::prelude::*;

use folia_design::ui::{EmptyState, Icon};

use super::btu::{self, Level};
use super::regular::view_back;
use super::{Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// A regulation as the view lists it.
#[derive(Clone, Debug, PartialEq)]
struct Doc {
    title: String,
    /// „Prüfungs- und Studienordnung", „Änderungssatzung".
    kind: String,
    url: String,
    /// The Regelstudienplan in it: where its first page opens, and its pages („Seite 7").
    plan: Option<(String, Option<String>)>,
}

/// The ways on from the regulations.
#[derive(Clone, Debug, PartialEq)]
struct More {
    /// The module tree in the BTU's course catalogue, where the program has one.
    tree: Option<String>,
    /// The catalog narrowed down to the program, and how many modules it lists.
    catalog: String,
    modules: i64,
    /// The program's page, at the study direction kept.
    page: String,
}

/// „Informatik B.Sc. · PO 2008 - 2. SÄ 2024": the regulation by its whole name, which is what
/// this view is about.
fn program_line(ready: &Ready) -> String {
    format!("{} {} · PO {}", ready.program.name, ready.program.degree(), ready.program.po_version)
}

/// Which Rahmenordnung concerns the student, by the degree: the Lehramt's under its Bachelor and
/// Master, both where the degree is neither.
fn level_of(ready: &Ready) -> Level {
    let level = &ready.program.degree_level;
    if level.is(DegreeLevel::Bachelor) || level.is(DegreeLevel::TeachingBachelor) {
        Level::Bachelor
    } else if level.is(DegreeLevel::Master) || level.is(DegreeLevel::TeachingMaster) {
        Level::Master
    } else {
        Level::Both
    }
}

fn more_of(ready: &Ready) -> More {
    let program = &ready.program;
    let page = ProgramUrl::new(&program.slug, ProgramTab::Plan).with_variant(ready.setup.shown.map_or(1, |shown| shown + 1)).path();
    More {
        tree: Some(program.source_url.trim().to_string()).filter(|tree| !tree.is_empty()),
        catalog: url::program_catalog_path(&program.slug, None),
        modules: program.curricular_modules,
        page,
    }
}

/// Where a PDF opens at a page: its first one of „6, 7" or „9–11". The address itself where
/// nothing says a page or the address has a fragment already.
fn at_page(address: &str, pages: Option<&str>) -> String {
    let first: String = pages.unwrap_or_default().trim().chars().take_while(char::is_ascii_digit).collect();
    if first.is_empty() || address.contains('#') {
        return address.to_string();
    }
    format!("{address}#page={first}")
}

fn docs_of(answer: &StudyDocuments, t: &Texts) -> Vec<Doc> {
    let v = &t.study_views;
    answer
        .documents
        .iter()
        .enumerate()
        .map(|(at, document)| {
            let plan = answer.plan.as_ref().filter(|plan| plan.document == at).map(|plan| (at_page(&document.url, plan.pages.as_deref()), plan.pages.as_deref().map(v.plan_pages)));
            Doc { title: document.title.clone(), kind: document.doc_type.label(t.locale).to_string(), url: document.url.clone(), plan }
        })
        .collect()
}

#[component]
pub(super) fn Documents(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    view_back(ctx);
    let head = Memo::new(move |_| ctx.with_ready(|ready| (program_line(ready), more_of(ready), level_of(ready))));
    let id = Memo::new(move |_| ctx.with_ready(|ready| ready.program.id.clone()));
    // The regulations; while they are on their way (or the question failed) `None`, and what was
    // there stays until the next answer is.
    let docs = Memo::new(move |before: Option<&Option<Vec<Doc>>>| {
        let program_id = id.get()?;
        let now = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&StudyDocumentsAsk { program_id })))?;
        folia_pages::ask::unless_pending(now, before, |now| now.ok().map(|answer| docs_of(&answer, t)))
    });
    move || {
        let (program, more, level) = head.get()?;
        let list = move || view! { <DocList docs/> };
        Some(if ctx.phone.get() {
            view! {
                <div class="st-phone-page st-docs-phone">
                    <header class="st-vhead">
                        <h1>{t.study_views.documents}</h1>
                        <p>{program}</p>
                    </header>
                    <section class="panel st-docs" aria-label=t.study_views.documents_list>{list()}</section>
                    <MoreLinks more/>
                    <btu::Statutes level/>
                    <btu::Around/>
                </div>
            }
            .into_any()
        } else {
            view! {
                <section class="panel st-docs" aria-labelledby="st-docs-title">
                    <header class="st-view-head">
                        <div>
                            <h2 id="st-docs-title">{t.study_views.documents}</h2>
                            <p>{program}</p>
                        </div>
                    </header>
                    {list()}
                </section>
                <MoreLinks more/>
                <btu::Statutes level/>
                <btu::Around/>
            }
            .into_any()
        })
    }
}

/// The regulations, each a link to its PDF; under the one with the Regelstudienplan the way to its
/// pages. A row of skeleton while they are on their way.
#[component]
fn DocList(docs: Memo<Option<Vec<Doc>>>) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    move || match docs.get() {
        None => view! {
            <ul class="doclist sk-sweep" aria-hidden="true">
                <li><span class="docrow"><i class="sk sk-w4"></i></span></li>
            </ul>
        }
        .into_any(),
        Some(docs) if docs.is_empty() => view! { <EmptyState title=v.no_documents hint=v.no_documents_hint/> }.into_any(),
        Some(docs) => view! {
            <ul class="doclist" aria-label=v.documents_list>
                {docs.into_iter().map(|doc| view! {
                    <li>
                        <a class="docrow" href=doc.url.clone() rel="noopener">
                            <Icon name="file-text"/>
                            <span class="docname">{doc.title.clone()}</span>
                            <small>{doc.kind.clone()}</small>
                            <Icon name="arrow-up-right" class="go"/>
                        </a>
                        {doc.plan.clone().map(|(href, pages)| view! {
                            <a class="docrow docpart" href=href rel="noopener">
                                <Icon name="file-check-2"/>
                                <span class="docname">{v.plan_in}</span>
                                {pages.map(|pages| view! { <small>{pages}</small> })}
                                <Icon name="arrow-up-right" class="go"/>
                            </a>
                        })}
                    </li>
                }).collect_view()}
            </ul>
        }
        .into_any(),
    }
}

/// „Mehr zum Studiengang": the module tree at the BTU, the program's modules in the catalog, its
/// page as everybody sees it.
#[component]
fn MoreLinks(more: More) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    view! {
        <section class="panel st-docs" aria-labelledby="st-docs-more">
            <header class="st-view-head">
                <div><h2 id="st-docs-more">{v.more}</h2></div>
            </header>
            <ul class="doclist">
                {more.tree.map(|tree| view! {
                    <li>
                        <a class="docrow" href=tree rel="noopener">
                            <Icon name="list-tree"/>
                            <span class="docname">{v.module_tree}</span>
                            <small>{v.module_tree_line}</small>
                            <Icon name="arrow-up-right" class="go"/>
                        </a>
                    </li>
                })}
                <li>
                    <a class="docrow" href=t.path(&more.catalog) rel="nofollow">
                        <Icon name="layout-list"/>
                        <span class="docname">{v.program_modules}</span>
                        <small>{(v.program_modules_line)(more.modules)}</small>
                        <Icon name="chevron-right" class="go"/>
                    </a>
                </li>
                <li>
                    <a class="docrow" href=t.path(&more.page)>
                        <Icon name="graduation-cap"/>
                        <span class="docname">{v.program_page}</span>
                        <small>{v.program_page_line}</small>
                        <Icon name="chevron-right" class="go"/>
                    </a>
                </li>
            </ul>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Regelstudienplan opens at its first page; an address that says nothing of one, or has a
    /// fragment already, stays as it is.
    #[test]
    fn a_plan_opens_at_its_page() {
        let pdf = "https://opus4.kobv.de/opus4-btu/files/6707/12_Informatik_B.Sc.pdf";
        assert_eq!(at_page(pdf, Some("7")), format!("{pdf}#page=7"));
        assert_eq!(at_page(pdf, Some(" 6, 7")), format!("{pdf}#page=6"));
        assert_eq!(at_page(pdf, Some("9–11")), format!("{pdf}#page=9"));
        assert_eq!(at_page(pdf, None), pdf);
        assert_eq!(at_page(pdf, Some("Anlage 2")), pdf);
        assert_eq!(at_page("https://example.org/a.pdf#x", Some("3")), "https://example.org/a.pdf#x");
    }
}
