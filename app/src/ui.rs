//! Small building blocks every page uses. (The full design system is phase 3; these
//! already follow its rules: classes from the stylesheet, no inline styles.)

use catalog::labels::{Code, ModuleKind, OfferStatus, KIND_UNKNOWN};
use leptos::prelude::*;
use leptos_meta::Title;

use crate::data::DataError;

/// A query failed or there is no snapshot: say so, never an empty list.
#[component]
pub fn ErrorState(error: DataError) -> impl IntoView {
    let (title, hint) = if error.unavailable {
        ("Der Katalog ist gerade nicht verfügbar", "Die Daten werden noch geladen. Bitte versuche es gleich noch einmal.")
    } else {
        ("Etwas ist schiefgelaufen", "Die Seite konnte nicht geladen werden. Bitte lade sie neu.")
    };
    view! {
        <Title text=title/>
        <section class="state state-error" role="alert">
            <h1>{title}</h1>
            <p>{hint}</p>
            <p class="state-detail">{error.message}</p>
            <p><a class="button" href="">"Neu laden"</a></p>
        </section>
    }
}

#[component]
pub fn NotFound(#[prop(into)] title: String, #[prop(into)] hint: String) -> impl IntoView {
    view! {
        <Title text=title.clone()/>
        <section class="state">
            <h1>{title}</h1>
            <p>{hint}</p>
            <p>
                <a class="button" href=catalog::url::CATALOG>"Zum Modulkatalog"</a>" "
                <a class="button button-quiet" href=catalog::url::PROGRAMS>"Zu den Studiengängen"</a>
            </p>
        </section>
    }
}

#[component]
pub fn EmptyState(#[prop(into)] title: String, #[prop(into)] hint: String) -> impl IntoView {
    view! {
        <div class="state state-empty">
            <p class="state-title">{title}</p>
            <p>{hint}</p>
        </div>
    }
}

/// „Pflicht", „Wahlpflicht" …, or „Art nicht angegeben": never a default.
#[component]
pub fn KindBadge(kind: Option<Code<ModuleKind>>) -> impl IntoView {
    match kind {
        Some(kind) => view! { <span class=format!("badge badge-kind badge-{}", kind.code())>{kind.label().to_string()}</span> }.into_any(),
        None => view! { <span class="badge badge-unknown">{KIND_UNKNOWN}</span> }.into_any(),
    }
}

/// Only what deviates from "is offered" gets a badge.
#[component]
pub fn OfferBadge(status: Code<OfferStatus>) -> impl IntoView {
    (!status.is(OfferStatus::Active))
        .then(|| view! { <span class=format!("badge badge-offer badge-{}", status.code())>{status.label().to_string()}</span> })
}

/// A label with its value; shows „nicht angegeben" instead of hiding an unknown value.
#[component]
pub fn Fact(#[prop(into)] label: String, value: Option<String>) -> impl IntoView {
    let unknown = value.is_none();
    view! {
        <div class="fact">
            <dt>{label}</dt>
            <dd class:unknown=unknown>{value.unwrap_or_else(|| "nicht angegeben".to_string())}</dd>
        </div>
    }
}

/// Free text from a module page: paragraphs on blank lines, line breaks kept.
#[component]
pub fn Prose(text: String) -> impl IntoView {
    text.split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .map(|paragraph| view! { <p class="prose">{paragraph.to_string()}</p> })
        .collect_view()
}
