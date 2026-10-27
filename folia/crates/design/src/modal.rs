//! A modal dialog (owner, 2026-10-27: „es kommt vor, dass immer mehr modal Dialoge verwendet werden
//! und gerade auf dem Handy ist die erste variante immer etwas komisch implementiert. Ich möchte,
//! dass wir das system anpassen, so dass die alle ein konsistentes verhalten vorlegen und das die
//! entwicklung einfacher wird"): one native `<dialog>` the page shows modally. On a wide screen it
//! stands in the middle of the window, in one of three sizes (app.css „The dialogs"); on a phone it
//! is a sheet from below (`data-sheet`), its grabber its one decoration, which `enhance.js` moves as
//! every sheet of a phone („the sheets of a phone"): half up and all up, its content scrolling all
//! up only, thrown away by a finger, gliding away however it closes.
//!
//! What every dialog needs is here once, where each dialog wrote it again before: it shows while
//! `open` says so; Esc, Back, a click beside it and a finger's throw ask its owner to close it
//! (`on_close`), and so does its × (`ModalHead`); what it holds goes in one piece as it closes, so
//! that the copy a phone lets glide away (`ghostSheet`) shows all of it; a dialog that goes with the
//! component holding it leaves the top layer first; the browser gives the focus back to what opened
//! it.
//!
//! ```ignore
//! view! {
//!     <Modal open=Signal::derive(move || asked.with(Option::is_some)) on_close=close size=ModalSize::M labelled_by="x-title">
//!         <ModalHead id="x-title" title="Studiengang wechseln" on_close=close/>
//!         <div class="st-dlg-body">…</div>
//!         <footer class="st-dlg-foot">…</footer>
//!     </Modal>
//! }
//! ```
//!
//! What holds its buttons is a `footer.st-dlg-foot` (or carries `data-sheet-foot`): on a phone it
//! stays at the screen's foot while the sheet is half up. What scrolls is a `div.st-dlg-body` (or
//! carries `data-sheet-body`): on a phone it scrolls all up only. What it shows should not go away
//! before the dialog has closed: content read from what `on_close` empties is best read from what
//! it was last (`kept`).

use leptos::prelude::*;

use crate::ui::DialogClose;

/// How large a dialog is on a wide screen, by what it holds (app.css „The dialogs"; owner,
/// 2026-10-16: „Modale menus springen zu viel in der größe. Erstelle dir mal 3 verschiedene
/// größen"): a question or a note (`S`, 440 px wide, as tall as what it says), a form or an
/// overview (`M`, 600 × 600 px), a list to choose from (`L`, 840 × 720 px). On a phone `M` and `L`
/// take the whole height of the screen all up, `S` is as tall as what it holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModalSize {
    #[default]
    S,
    M,
    L,
}

impl ModalSize {
    /// Its class on the dialog (app.css): none for `S`.
    pub fn class(self) -> &'static str {
        match self {
            ModalSize::S => "",
            ModalSize::M => "m",
            ModalSize::L => "l",
        }
    }
}

/// A dialog shown modally while `open` says so; on a phone a sheet from below. Its content is drawn
/// as it opens and goes as it closes.
#[component]
pub fn Modal(
    /// Shown while true.
    #[prop(into)]
    open: Signal<bool>,
    /// Asked to close: Esc, Back, a click beside it, a finger's throw on a phone. The owner of
    /// `open` makes it false; the dialog has closed by then (the browser's own way out) or closes.
    #[prop(into)]
    on_close: Callback<()>,
    /// Its size on a wide screen (it may change with what it holds).
    #[prop(optional, into)]
    size: Signal<ModalSize>,
    /// Classes of its own besides `st-dialog` and its size (`keys-dialog`).
    #[prop(optional)]
    class: &'static str,
    /// The id of what names it (`ModalHead`'s).
    #[prop(optional)]
    labelled_by: Option<&'static str>,
    /// What names it where nothing in it does.
    #[prop(optional, into)]
    label: Option<Signal<String>>,
    /// It takes the focus itself as it opens, not its first field: the screen's keyboard of a phone
    /// does not come up at once over what it shows.
    #[prop(optional)]
    focus_itself: bool,
    children: ChildrenFn,
) -> impl IntoView {
    let node = NodeRef::<leptos::html::Dialog>::new();
    // Shown while `open` says so, from the frame it is in the page.
    Effect::new(move |_| {
        let open = open.get();
        let Some(dialog) = node.get() else { return };
        if open && !dialog.open() {
            let _ = dialog.show_modal();
            if focus_itself {
                let _ = dialog.focus();
            }
        } else if !open && dialog.open() {
            dialog.close();
        }
    });
    // Gone with what holds it: out of the top layer before it leaves the page.
    on_cleanup(move || {
        if let Some(dialog) = node.try_get_untracked().flatten() {
            if dialog.open() {
                dialog.close();
            }
        }
    });
    let beside = move |ev: leptos::ev::MouseEvent| {
        if on_backdrop(node, &ev) {
            on_close.run(());
        }
    };
    let class = move || {
        let size = size.get().class();
        ["st-dialog", size, class].iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join(" ")
    };
    view! {
        <dialog
            class=class
            node_ref=node
            data-sheet=""
            aria-labelledby=labelled_by
            aria-label=move || label.map(|label| label.get())
            tabindex=focus_itself.then_some("-1")
            autofocus=focus_itself
            on:close=move |_| on_close.run(())
            on:click=beside
        >
            {move || open.get().then(|| view! { <div class="st-dlg">{children()}</div> })}
        </dialog>
    }
}

/// Whether `ev`, a click on the dialog `node`, was on its dim backdrop: on the dialog itself, outside
/// its box (its grabber, a phone's, is the dialog's own too, inside its box).
pub fn on_backdrop(node: NodeRef<leptos::html::Dialog>, ev: &leptos::ev::MouseEvent) -> bool {
    #[cfg(feature = "csr")]
    {
        let Some(dialog) = node.get_untracked() else { return false };
        let on_dialog = ev.target().is_some_and(|target| {
            let dialog: &leptos::web_sys::EventTarget = dialog.as_ref();
            dialog == &target
        });
        let element: &web_sys::Element = dialog.as_ref();
        let rect = element.get_bounding_client_rect();
        let (x, y) = (f64::from(ev.client_x()), f64::from(ev.client_y()));
        on_dialog && (x < rect.left() || x > rect.right() || y < rect.top() || y > rect.bottom())
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = (node, ev);
        false
    }
}

/// The head of a dialog: what it is (its title, named by `id` for `Modal`'s `labelled_by`, a line
/// under it) and its way out, „× Schließen Esc" on a wide screen (`DialogClose`), out of sight on a
/// phone, whose sheet has its grabber alone.
#[component]
pub fn ModalHead(
    /// The id of its title.
    id: &'static str,
    #[prop(into)] title: String,
    /// The line under the title.
    #[prop(into, default = None)]
    sub: Option<String>,
    /// Before the title (an area's dot).
    #[prop(default = None)]
    lead: Option<ViewFn>,
    /// What closes the dialog.
    #[prop(into)]
    on_close: Callback<()>,
    /// What the head holds besides, under the line.
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    view! {
        <header class="st-dlg-head">
            <div>
                <h2 id=id>{lead.map(|lead| lead.run())}{title}</h2>
                {sub.map(|sub| view! { <p>{sub}</p> })}
                {children.map(|children| children())}
            </div>
            <DialogClose on_close/>
        </header>
    }
}

/// What `value` was last, while it is something: what a dialog shows stays what it was until the
/// dialog has gone, though its owner empties it as it asks the dialog to close.
pub fn kept<T: Clone + PartialEq + Send + Sync + 'static>(value: impl Fn() -> Option<T> + Send + Sync + 'static) -> Memo<Option<T>> {
    Memo::new(move |before: Option<&Option<T>>| value().or_else(|| before.cloned().flatten()))
}
