//! The dialogs of „Mein Studium" (`Dialog`): one native `<dialog>` the page shows modally, whose
//! content is the dialog open (`StudyCtx::dialog`). On a desktop it stands in the middle of the
//! window; on a phone it is a sheet from below, which a finger drags down to close it as it does
//! the catalog's filters (owner, 2026-10-05: „dass man die einfach wieder runter sliden kann";
//! `data-sheet`, enhance.js). Escape, „×" and a click beside it close it, and the browser gives
//! the focus back to what opened it.

use leptos::prelude::*;

use folia_design::ui::Icon;

use super::{overview, picker, side, Dialog, StudyCtx};
use crate::i18n;

/// The class of a dialog's kind: how wide it is.
fn class_of(dialog: &Dialog) -> &'static str {
    match dialog {
        Dialog::Add { .. } | Dialog::Cell { .. } => "wide",
        Dialog::Switch => "medium",
        Dialog::Area(_) | Dialog::Areas => "narrow",
    }
}

#[component]
pub(super) fn DialogHost(ctx: StudyCtx) -> impl IntoView {
    let node = NodeRef::<leptos::html::Dialog>::new();
    let class = Memo::new(move |_| ctx.dialog.with(|dialog| dialog.as_ref().map_or("", class_of)));
    // The dialog shows while one is open, and goes with it.
    Effect::new(move |_| {
        let open = ctx.dialog.with(Option::is_some);
        let Some(element) = node.get() else { return };
        if open && !element.open() {
            let _ = element.show_modal();
        } else if !open && element.open() {
            element.close();
        }
    });
    let close = move || ctx.dialog.set(None);
    view! {
        <dialog
            class=move || format!("st-dialog {}", class.get())
            node_ref=node
            data-sheet=""
            aria-labelledby="st-dialog-title"
            on:close=move |_| close()
            // A click on the dim backdrop is a click on the dialog itself.
            on:click=move |ev: leptos::ev::MouseEvent| {
                let on_backdrop = ev.target().and_then(|target| node.get_untracked().map(|dialog| {
                    let dialog: &leptos::web_sys::EventTarget = dialog.as_ref();
                    dialog == &target
                }));
                if on_backdrop == Some(true) {
                    close();
                }
            }
        >
            {move || ctx.dialog.get().map(|dialog| {
                let content = match dialog {
                    Dialog::Add { semester, catalog, chosen } => view! { <picker::AddDialog ctx semester catalog chosen/> }.into_any(),
                    Dialog::Cell { area, semester } => view! { <picker::CellDialog ctx area semester/> }.into_any(),
                    Dialog::Area(area) => view! { <overview::AreaDialog ctx area/> }.into_any(),
                    Dialog::Areas => view! { <overview::AreasSheet ctx/> }.into_any(),
                    Dialog::Switch => view! { <side::SwitchDialog ctx/> }.into_any(),
                };
                view! { <div class="st-dlg">{content}</div> }
            })}
        </dialog>
    }
}

/// The head of a dialog: its title, a line under it, and „×".
#[component]
pub(super) fn DialogHead(
    ctx: StudyCtx,
    #[prop(into)] title: String,
    #[prop(optional, into)] sub: Option<String>,
    /// A dot in an area's colour before the title.
    #[prop(optional)]
    tone: Option<&'static str>,
) -> impl IntoView {
    let t = i18n::t();
    view! {
        <header class="st-dlg-head">
            <span class="st-grip" aria-hidden="true"></span>
            <div>
                <h2 id="st-dialog-title">
                    {tone.map(|tone| view! { <span class="st-dot" style=format!("--c: {tone}")></span> })}
                    {title}
                </h2>
                {sub.map(|sub| view! { <p>{sub}</p> })}
            </div>
            <button class="icon-btn hit st-dlg-close" type="button" aria-label=t.study.close on:click=move |_| ctx.dialog.set(None)><Icon name="x"/></button>
        </header>
    }
}
