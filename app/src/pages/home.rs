//! The landing page: what the app can do, with a link to each function.

use catalog::{pages, url};
use leptos::prelude::*;
use leptos_meta::Title;

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::ui::ErrorState;

#[component]
pub fn HomePage() -> impl IntoView {
    let source = use_source();
    let status = PageStatus::capture();
    let overview = source.and_then(|source| source.run(pages::overview));

    view! {
        <Title text=""/>
        <section class="hero">
            <h1>"Finde die Module, die zu deinem Studium passen."</h1>
            <p class="lead">
                "Alle Module, Studiengänge und Regelstudienpläne der BTU Cottbus-Senftenberg an einem Ort: "
                "schnell durchsuchbar, ehrlich bei Lücken, mit Link zur Quelle."
            </p>
        </section>
        {match overview {
            Err(error) => {
                status.for_error(&error);
                view! { <ErrorState error/> }.into_any()
            }
            Ok(overview) => {
                let semester = overview.current_semester.as_ref().map(|s| s.label.clone());
                view! {
                    <ul class="features">
                        <li class="feature">
                            <a class="feature-link" href=url::CATALOG>
                                <h2>"Modulkatalog"</h2>
                                <p>"Filtere nach Studiengang, Turnus, Lehrform, Prüfung, Sprache und Dozierenden."</p>
                                <p class="feature-number">{format::count(overview.modules)}" Module im Angebot"</p>
                            </a>
                        </li>
                        <li class="feature">
                            <a class="feature-link" href=url::PROGRAMS>
                                <h2>"Studiengänge"</h2>
                                <p>"Regelstudienplan, Wahlpflichtbereiche, FÜS-Liste und Ordnungen je Studiengang."</p>
                                <p class="feature-number">{format::count(overview.programs)}" Studiengänge"</p>
                            </a>
                        </li>
                        <li class="feature feature-soon">
                            <div class="feature-link">
                                <h2>"Merkliste & Studienverlauf"</h2>
                                <p>"Module merken, bestandene abhaken und sehen, welche Voraussetzungen erfüllt sind."</p>
                                <p class="feature-number">"in Arbeit"</p>
                            </div>
                        </li>
                        <li class="feature feature-soon">
                            <div class="feature-link">
                                <h2>"Semesterplaner"</h2>
                                <p>"Stelle dir deinen Stundenplan für das kommende Semester zusammen."</p>
                                <p class="feature-number">"geplant"</p>
                            </div>
                        </li>
                    </ul>
                    <p class="freshness">
                        {semester.map(|label| format!("Aktuelles Semester: {label} · "))}
                        {overview.meta.data_changed_at.as_deref().map(|at| format!("Datenstand {}", format::date(at)))}
                        " · Quelle: Modulbeschreibungen und Vorlesungsverzeichnis der BTU"
                    </p>
                }.into_any()
            }
        }}
    }
}
