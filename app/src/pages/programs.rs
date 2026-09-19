//! The program overview: every program in its current PO version, grouped by degree level.

use catalog::labels::{Code, DegreeLevel, Labelled};
use catalog::queries;
use catalog::rows::Program;
use catalog::url::{self, ProgramTab};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_query_map;

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::ui::ErrorState;

#[component]
pub fn ProgramsPage() -> impl IntoView {
    let source = use_source();
    let status = PageStatus::capture();
    let query = use_query_map();
    let text = Memo::new(move |_| query.read().get("q").unwrap_or_default());
    // Loaded once; typing in the search only filters what is shown.
    let programs = StoredValue::new(source.and_then(|source| source.run(queries::programs)));
    view! {
        <Title text="Studiengänge"/>
        <div class="page"><div class="page-inner">
        {move || match programs.get_value() {
            Err(error) => {
                status.for_error(&error);
                view! { <ErrorState error/> }.into_any()
            }
            Ok(programs) => view! { <Overview programs text=text.get()/> }.into_any(),
        }}
        </div></div>
    }
}

#[component]
fn Overview(programs: Vec<Program>, text: String) -> impl IntoView {
    let all_current = programs.iter().filter(|p| p.is_latest_po).count();
    let current: Vec<&Program> = programs
        .iter()
        .filter(|p| p.is_latest_po)
        .filter(|p| catalog::search::matches(&format!("{} {} {}", p.name, p.degree(), p.degree_level.label()), &text))
        .collect();
    let filtered = !text.trim().is_empty();
    let older = |program: &Program| programs.iter().filter(|p| p.family_key == program.family_key && !p.is_latest_po).count();

    // Known levels in their usual order, then whatever else the data has.
    let mut levels: Vec<Code<DegreeLevel>> = DegreeLevel::ALL.iter().map(|level| Code::Known(*level)).collect();
    for program in &current {
        if !levels.contains(&program.degree_level) {
            levels.push(program.degree_level.clone());
        }
    }

    view! {
        <header class="panel page-head">
            <h1>"Studiengänge"</h1>
            <p class="total">
                <strong>{format::count(current.len() as u64)}</strong>
                {if filtered { format!(" von {all_current} Studiengängen passen zu „{}“", text.trim()) } else { " Studiengänge in ihrer aktuellen Prüfungsordnung".to_string() }}
            </p>
        </header>
        {(filtered && current.is_empty()).then(|| view! {
            <div class="state"><p class="state-title">"Kein Studiengang gefunden"</p><p>"Prüfe die Schreibweise oder suche nach einem Teil des Namens."</p></div>
        })}
        {levels.into_iter().map(|level| {
            let group: Vec<&Program> = current.iter().copied().filter(|p| p.degree_level == level).collect();
            (!group.is_empty()).then(|| view! {
                <section class="panel block">
                    <h2>{level.label().to_string()}" "<span class="tab-count">{group.len()}</span></h2>
                    <ul class="cards">
                        {group.into_iter().map(|p| {
                            let older = older(p);
                            view! {
                                <li class="card">
                                    <a class="card-link" data-walk="program-link" href=url::program_path(&p.slug, ProgramTab::Plan)>
                                        <span class="card-title">{p.name.clone()}</span>
                                        <span class="card-meta">
                                            {p.degree().to_string()}" · PO "{p.po_version.clone()}
                                            {p.study_variant.as_ref().map(|v| format!(" · {}", v.label()))}
                                        </span>
                                        <span class="card-meta">
                                            {p.curricular_modules}" Module"
                                            {p.has_plan.then_some(" · Regelstudienplan")}
                                            {(older > 0).then(|| format!(" · {older} ältere PO"))}
                                        </span>
                                    </a>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                </section>
            })
        }).collect_view()}
    }
}
