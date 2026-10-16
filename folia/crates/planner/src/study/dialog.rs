//! The dialogs of „Mein Studium" (`Dialog`): one native `<dialog>` the page shows modally, whose
//! content is the dialog open (`StudyCtx::dialog`). On a desktop it stands in the middle of the
//! window; on a phone it is a sheet from below, which a finger drags down to close it as it does
//! the catalog's filters (owner, 2026-10-05: „dass man die einfach wieder runter sliden kann";
//! `data-sheet`, enhance.js). Escape, „Schließen" and a click beside it close it, and the browser
//! gives the focus back to what opened it.
//!
//! One frame in three sizes (owner, 2026-10-16: „Modale menus springen zu viel in der größe.
//! Erstelle dir mal 3 verschiedene größen, unter den man dann auswählen kann"; app.css „The
//! dialogs"): each kind of dialog has its size, and the frame keeps it while the dialog is open,
//! whatever its content does — „Module hinzufügen" shrank to a third when its tab changed, and
//! „Studiengang wechseln" grew and shrank with the program picked. Its head says what it is and
//! the way out with its key, its foot has the buttons on the right.

use leptos::prelude::*;

use folia_design::ui::DialogClose;

use super::{overview, picker, side, Dialog, StudyCtx};

/// The size of a dialog's kind (app.css „The dialogs"): „l" for a list to choose from, „m" for a
/// form or an overview, „s" for a question or a note. The areas and an area are one size, so that
/// a phone's sheet does not move when an area of the list is opened in it.
fn class_of(dialog: &Dialog) -> &'static str {
    match dialog {
        Dialog::Add { .. } | Dialog::Cell { .. } => "l",
        Dialog::Switch | Dialog::Area(_) | Dialog::Areas => "m",
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

/// The head of a dialog: its title, a line under it, and „Schließen" with its key.
#[component]
pub(super) fn DialogHead(
    ctx: StudyCtx,
    #[prop(into)] title: String,
    #[prop(optional, into)] sub: Option<String>,
    /// A dot in an area's colour before the title.
    #[prop(optional)]
    tone: Option<&'static str>,
) -> impl IntoView {
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
            <DialogClose on_close=Callback::new(move |()| ctx.dialog.set(None))/>
        </header>
    }
}
