//! The dialogs of „Mein Studium" (`Dialog`): one dialog the page shows modally
//! (`folia_design::modal::Modal`), whose content is the dialog open (`StudyCtx::dialog`). On a
//! desktop it stands in the middle of the window; on a phone it is a sheet from below, as every
//! modal of a phone (owner, 2026-10-27; enhance.js „the sheets of a phone"): half up and all up,
//! thrown away by a finger. Escape, „Schließen", Back and a click beside it close it, and the
//! browser gives the focus back to what opened it.
//!
//! One frame in three sizes (owner, 2026-10-16: „Modale menus springen zu viel in der größe.
//! Erstelle dir mal 3 verschiedene größen, unter den man dann auswählen kann"; app.css „The
//! dialogs"): each kind of dialog has its size, and the frame keeps it while the dialog is open,
//! whatever its content does — „Module hinzufügen" shrank to a third when its tab changed, and
//! „Studiengang wechseln" grew and shrank with the program picked. Its head says what it is and
//! the way out with its key, its foot has the buttons on the right.

use leptos::prelude::*;

use folia_design::modal::{kept, Modal, ModalHead, ModalSize};

use super::{overview, picker, side, Dialog, StudyCtx};

/// The size of a dialog's kind (app.css „The dialogs"): „l" for a list to choose from, „m" for a
/// form or an overview. The areas and an area are one size, so that a phone's sheet does not move
/// when an area of the list is opened in it.
fn size_of(dialog: &Dialog) -> ModalSize {
    match dialog {
        Dialog::Add { .. } | Dialog::Cell { .. } => ModalSize::L,
        Dialog::Switch | Dialog::Area(_) | Dialog::Areas => ModalSize::M,
    }
}

#[component]
pub(super) fn DialogHost(ctx: StudyCtx) -> impl IntoView {
    // What it shows stays while it goes (a phone's sheet glides away with it).
    let shown = kept(move || ctx.dialog.get());
    let size = Signal::derive(move || shown.with(|dialog| dialog.as_ref().map_or(ModalSize::S, size_of)));
    view! {
        <Modal open=Signal::derive(move || ctx.dialog.with(Option::is_some)) on_close=move || ctx.dialog.set(None) size labelled_by="st-dialog-title">
            {move || shown.get().map(|dialog| match dialog {
                Dialog::Add { semester, catalog, chosen } => view! { <picker::AddDialog ctx semester catalog chosen/> }.into_any(),
                Dialog::Cell { area, semester } => view! { <picker::CellDialog ctx area semester/> }.into_any(),
                Dialog::Area(area) => view! { <overview::AreaDialog ctx area/> }.into_any(),
                Dialog::Areas => view! { <overview::AreasSheet ctx/> }.into_any(),
                Dialog::Switch => view! { <side::SwitchDialog ctx/> }.into_any(),
            })}
        </Modal>
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
    let lead = tone.map(|tone| ViewFn::from(move || view! { <span class="st-dot" style=format!("--c: {tone}")></span> }));
    view! { <ModalHead id="st-dialog-title" title sub lead on_close=move || ctx.dialog.set(None)/> }
}
