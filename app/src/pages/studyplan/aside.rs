//! One module beside the plan, with every Termin and the choice of one group.
//! Work package WP19 of the Studienplan; until then a stub with its fixed signature: the panel's
//! box with the ways out of it („Modul ansehen" in full, „Schließen").

use leptos::prelude::*;

use super::{full_href, PlanCtx};
use crate::ui::{Icon, Shortcut};

#[component]
pub(super) fn PlanModulePanel(ctx: PlanCtx) -> impl IntoView {
    let id = Memo::new(move |_| ctx.url.with(|url| url.open.clone()).unwrap_or_default());
    let full = move || ctx.url.with(|url| full_href(url, &id.get()));
    let close = move || ctx.url.with(|url| url.with_open(None, None).path());
    view! {
        <section class="panel detail aside" id="preview" aria-label="Modul im Plan">
            <div class="scroll">
                <header class="hero">
                    <div class="hero-top">
                        <span class="mono">{id}</span>
                        <a class="ghost" href=full data-action="fullscreen" title="Als ganze Seite öffnen (F)"><Icon name="maximize-2"/>"Modul ansehen"<Shortcut keys="F"/></a>
                        <a class="ghost" href=close data-action="close-detail" title="Schließen (Esc)"><Icon name="x"/>"Schließen"<Shortcut keys="Esc"/></a>
                    </div>
                </header>
                <div class="dbody"><p class="hint">"…"</p></div>
            </div>
        </section>
    }
}
