//! The sidebar group „Kalender": the .ics download and the subscription.
//! Work package WP21 of the Studienplan; until then a stub with its fixed signature.

use leptos::prelude::*;

use super::PlanCtx;

#[component]
pub(super) fn CalendarGroup(ctx: PlanCtx) -> impl IntoView {
    let _ = ctx;
    view! { <div class="fgroup"><p class="hint">"…"</p></div> }
}
