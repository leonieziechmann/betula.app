//! The landing page: what the app can do, with a link to each function.

use catalog::{pages, url};
use leptos::prelude::*;
use leptos_meta::Title;

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::ui::{ErrorState, Frame, Mark, Wordmark};

#[component]
pub fn HomePage() -> impl IntoView {
    let source = use_source();
    let status = PageStatus::capture();
    let overview = source.and_then(|source| source.run(pages::overview));
    // The sidebar of the landing page: how big and how fresh the data is.
    let facts = overview.clone().ok();
    let sidebar = move || {
        facts.clone().map(|overview| view! {
            <dl class="side-facts">
                {overview.current_semester.as_ref().map(|s| view! { <div><dt>"Aktuelles Semester"</dt><dd>{s.label.clone()}</dd></div> })}
                {overview.meta.data_changed_at.as_deref().map(|at| view! { <div><dt>"Daten zuletzt geändert"</dt><dd>{format::date(at)}</dd></div> })}
                <div><dt>"Module im Angebot"</dt><dd class="num">{format::count(overview.modules)}</dd></div>
                <div><dt>"Studiengänge"</dt><dd class="num">{format::count(overview.programs)}</dd></div>
            </dl>
            <p class="hint">"Quelle: Modulbeschreibungen und Vorlesungsverzeichnis der BTU Cottbus-Senftenberg. Wo die Quelle nichts sagt, steht hier „nicht angegeben“ statt einer Vermutung."</p>
            <p class="hint">"Betula ist ein inoffizielles Projekt und gehört nicht zur BTU."</p>
        })
    };

    view! {
        <Title text=""/>
        <Frame title="Datenstand" sidebar><div class="page-inner">
        <section class="panel page-head">
            <p class="brand-phone"><span class="logo"><Mark/></span><span><Wordmark small=true/><small>"Modulkatalog · inoffiziell"</small></span></p>
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
                }.into_any()
            }
        }}
        </div></Frame>
    }
}
