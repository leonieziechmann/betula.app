//! The view „Übersicht" and the import of a Regelstudienplan.
//! Work package WP20 of the Studienplan; until then each is a stub with its fixed signature.

use leptos::prelude::*;

use super::PlanCtx;

#[component]
pub(super) fn OverviewView(ctx: PlanCtx) -> impl IntoView {
    let _ = ctx;
    view! { <p class="hint">"…"</p> }
}

#[component]
pub(super) fn ImportPanel(ctx: PlanCtx) -> impl IntoView {
    let _ = ctx;
    view! { <p class="hint">"…"</p> }
}
