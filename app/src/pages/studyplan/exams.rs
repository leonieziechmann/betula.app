//! The view „Prüfungen".
//! Work package WP20 of the Studienplan; until then a stub with its fixed signature.

use leptos::prelude::*;

use super::PlanCtx;

#[component]
pub(super) fn ExamsView(ctx: PlanCtx) -> impl IntoView {
    let _ = ctx;
    view! { <p class="hint">"…"</p> }
}
