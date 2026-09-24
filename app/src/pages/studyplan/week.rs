//! The views „Woche" (the Regelwoche, a day list on a phone) and „Termine".
//! Work package WP18 of the Studienplan; until then each is a stub with its fixed signature.

use leptos::prelude::*;

use super::PlanCtx;

#[component]
pub(super) fn WeekView(ctx: PlanCtx) -> impl IntoView {
    let _ = ctx;
    view! { <p class="hint">"…"</p> }
}

#[component]
pub(super) fn DatesView(ctx: PlanCtx) -> impl IntoView {
    let _ = ctx;
    view! { <p class="hint">"…"</p> }
}
