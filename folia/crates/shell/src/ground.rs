//! The birch around the app (owner, 2026-09-25; docs/folia/frontend.md „The birch"): its crown along the
//! top of the view, a wood of birches behind every page (owner, 2026-09-29), and the ground at the
//! end of every page with the roots in it. Both are drawn
//! by the stylesheet from the masks in `assets/birch` (`design/birch/birch.mjs`), in the tone of
//! the season the script in `<head>` names; this module only places them.

use folia_pages::ask::GroundAsk;
use folia_routes::url;
use leptos::prelude::*;

use folia_data::use_ask;
use folia_design::format;
use crate::i18n;
use crate::seo;
use folia_design::ui::{Icon, Wordmark};

/// The crown: the edge of a birch's crown hangs in from above along the whole top, behind the
/// mark, the title and the search, on every screen the same. Drawing only, and the same for
/// everybody (R9): the season is chosen in the browser.
#[component]
pub fn Crown() -> impl IntoView {
    view! { <div class="crown" aria-hidden="true"></div> }
}

/// The wood: birches cut out of the background's grey, behind every page, standing on the window's
/// edge or on the ground as it comes up (`enhance.js`). It fills the room beside a page narrower
/// than the window and shows wherever the panels leave a gap. Drawing only, the same for everybody;
/// the masks are `design/forest/forest.mjs`'s.
#[component]
pub fn Wood() -> impl IntoView {
    view! { <div class="wood" aria-hidden="true"></div> }
}

/// The ground: the footer of every page, after all of its content. On the desktop a page that flows
/// (the catalog) has it as the end of its own scroll area; on the others it comes up from the
/// window's edge once the page is at its end, and the panels get shorter for it (`enhance.js`);
/// on a phone it follows the page, and ends at the window's lower edge under a page shorter than
/// the window. It holds what belongs at the bottom of a site — who is behind it, the legal pages,
/// the versions of Folia and Radix (the roots: Radix brings the data) and how fresh the data is.
#[component]
pub fn Ground() -> impl IntoView {
    let t = i18n::t();
    // The data's facts, once they are there: the rest of the ground stands from the first frame.
    let facts = use_ask(|| Some(GroundAsk {}));
    let fact = move |of: fn(&folia_pages::Ground) -> Option<String>| move || facts.get().and_then(Result::ok).as_ref().and_then(of);
    let radix = fact(|ground| ground.meta.radix_version.clone());
    let changed = move || facts.get().and_then(Result::ok).and_then(|ground| ground.meta.data_changed_at.as_deref().map(|date| format::date(date, t.locale)));
    // The semester by its key, in the page's language (the snapshot's label is German).
    let semester = move || facts.get().and_then(Result::ok).and_then(|ground| ground.current_semester.as_ref().map(|s| folia_calendar::semester::SemesterKey::parse(&s.key).map_or_else(|| s.label.clone(), |key| key.label(t.locale))));
    view! {
        <footer class="ground">
            <div class="ground-top">
                <div>
                    <p class="ground-name"><Wordmark small=true/><small>{t.common.tagline}</small></p>
                    <p class="ground-note">{t.ground.note}</p>
                </div>
                <nav class="ground-legal" aria-label=t.ground.legal>
                    <a href=t.path(url::IMPRINT)>{t.ground.imprint}</a>
                    <a href=t.path(url::PRIVACY)>{t.ground.privacy}</a>
                </nav>
            </div>
            <div class="ground-foot">
                <p>
                    <span>"Folia "<span class="ver">{crate::document::VERSION}</span></span>
                    {move || radix().map(|version| view! { <span>"Radix "<span class="ver">{version}</span></span> })}
                </p>
                <p>
                    {move || changed().map(|date| view! { <span>{(t.ground.data_of)(&date)}</span> })}
                    {move || semester().map(|label| view! { <span>{label}</span> })}
                    <a href=seo::UNIVERSITY_URL rel="noopener">{t.ground.source}<Icon name="arrow-up-right"/></a>
                </p>
            </div>
        </footer>
    }
}
