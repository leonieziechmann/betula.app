//! The sidebar of the Studienplan („Anpassen").
//! Work package WP17 of the Studienplan; until then a stub with its fixed signature.

use leptos::prelude::*;

use super::PlanCtx;
use super::export::CalendarGroup;

#[component]
pub(super) fn PlanSidebar(ctx: PlanCtx) -> impl IntoView {
    view! {
        <div class="fgroup first"><p class="hint">"…"</p></div>
        <CalendarGroup ctx/>
    }
}
