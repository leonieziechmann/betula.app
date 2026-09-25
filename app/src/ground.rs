//! The birch around the app (owner, 2026-09-25; docs/frontend.md „The birch"): its crown along the
//! top of the view, and the ground at the end of every page with the roots in it. Both are drawn
//! by the stylesheet from the masks in `assets/birch` (`design/birch/birch.mjs`), in the tone of
//! the season the script in `<head>` names; this module only places them.

use catalog::pages;
use catalog::url;
use leptos::prelude::*;

use crate::data::use_source;
use crate::format;
use crate::seo;
use crate::ui::{Icon, Wordmark};

/// The crown: the edge of a birch's crown hangs in from above along the whole top, behind the
/// mark, the title and the search, on every screen the same. Drawing only, and the same for
/// everybody (R9): the season is chosen in the browser.
#[component]
pub fn Crown() -> impl IntoView {
    view! { <div class="crown" aria-hidden="true"></div> }
}

/// The ground: the footer of every page, after all of its content. On the desktop it comes up from
/// the window's edge once the page is at its end, and the panels get shorter for it (`enhance.js`);
/// on a phone it follows the page. It holds what belongs at the bottom of a site — who is behind
/// it, the legal pages, the versions of Folia and Radix (the roots: Radix brings the data) and how
/// fresh the data is.
#[component]
pub fn Ground() -> impl IntoView {
    let facts = use_source().ok().and_then(|source| source.run(pages::ground).ok());
    let radix = facts.as_ref().and_then(|ground| ground.meta.radix_version.clone());
    let changed = facts.as_ref().and_then(|ground| ground.meta.data_changed_at.as_deref().map(format::date));
    let semester = facts.as_ref().and_then(|ground| ground.current_semester.as_ref().map(|s| s.label.clone()));
    view! {
        <footer class="ground">
            <div class="ground-top">
                <div>
                    <p class="ground-name"><Wordmark small=true/><small>"Modulkatalog · inoffiziell"</small></p>
                    <p class="ground-note">"Betula ist ein inoffizielles Projekt und gehört nicht zur BTU."</p>
                </div>
                <nav class="ground-legal" aria-label="Rechtliches">
                    <a href=url::IMPRINT>"Impressum"</a>
                    <a href=url::PRIVACY>"Datenschutz"</a>
                </nav>
            </div>
            <div class="ground-foot">
                <p>
                    <span>"Folia "<span class="ver">{crate::VERSION}</span></span>
                    {radix.map(|version| view! { <span>"Radix "<span class="ver">{version}</span></span> })}
                </p>
                <p>
                    {changed.map(|date| view! { <span>"Daten vom "{date}</span> })}
                    {semester.map(|label| view! { <span>{label}</span> })}
                    <a href=seo::UNIVERSITY_URL rel="noopener">"Quelle: BTU"<Icon name="arrow-up-right"/></a>
                </p>
            </div>
        </footer>
    }
}
