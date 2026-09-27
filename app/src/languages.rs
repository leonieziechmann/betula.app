//! The switch between the languages of the site: a link per language to the page the visitor is
//! on, with what its address says (filters, the module beside it). Another language is another
//! address and a new page load (`rel="external"`: neither the router nor `pending` take it), so
//! the app starts again in that language; what the visitor keeps in this browser (Merkliste,
//! Stundenplan, „Mein Studiengang") belongs to the site, not to a language, and stays.
//!
//! The links depend on the address alone, so the server's page stays the same for everybody (R9).

use leptos::prelude::*;

use crate::i18n::{self, Locale};
use crate::tabs::location_of;

/// The languages as links. `short`: the other languages by their codes („EN"), for the rail;
/// else every language by its name, the current one marked (the ground).
#[component]
pub fn Languages(#[prop(optional)] short: bool) -> impl IntoView {
    let t = i18n::t();
    let location = i18n::use_location();
    let here = Memo::new(move |_| location_of(&location.pathname.get(), &location.search.get()));
    let links = Locale::ALL
        .iter()
        .copied()
        .filter(move |locale| !short || *locale != t.locale)
        .map(|locale| {
            let current = locale == t.locale;
            let label = if short { locale.code().to_uppercase() } else { locale.name().to_string() };
            view! {
                <a
                    href=move || here.with(|here| locale.path(here))
                    hreflang=locale.code()
                    lang=locale.code()
                    rel="alternate external"
                    title=locale.name()
                    aria-current=current.then_some("true")
                >
                    {label}
                </a>
            }
        })
        .collect_view();
    view! { <nav class="languages" class:short=short aria-label=t.common.language>{links}</nav> }
}
