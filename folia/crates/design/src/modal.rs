//! A modal dialog (owner, 2026-10-27: „es kommt vor, dass immer mehr modal Dialoge verwendet werden
//! und gerade auf dem Handy ist die erste variante immer etwas komisch implementiert. Ich möchte,
//! dass wir das system anpassen, so dass die alle ein konsistentes verhalten vorlegen und das die
//! entwicklung einfacher wird"): one native `<dialog>` the page shows modally. On a wide screen it
//! stands in the middle of the window, in one of three sizes (app.css „The dialogs"); on a phone it
//! is a sheet from below (`data-sheet`), its grabber its one decoration, which `enhance.js` moves as
//! every sheet of a phone („the sheets of a phone"), standing one of three ways (`Sheet`): as tall
//! as it fits, half up and all up, or all up at once; thrown away by a finger, gliding away however
//! it closes.
//!
//! What every dialog needs is here once, where each dialog wrote it again before: it shows while
//! `open` says so; Esc, Back, a click beside it and a finger's throw ask its owner to close it
//! (`on_close`), and so does its × (`ModalHead`); what it holds goes in one piece as it closes, so
//! that the copy a phone lets glide away (`ghostSheet`) shows all of it; a dialog that goes with the
//! component holding it leaves the top layer first; the browser gives the focus back to what opened
//! it. Where its owner places it (`at`, a card beside the slot it was opened from) it stands there
//! instead of the window's middle; Esc closes what stands open in it first, where its owner says so
//! (`on_cancel`); a click beside it can be its owner's (`on_beside`: another slot's card in its
//! place) instead of closing it.
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
//! On a phone it stands as its size says (`ModalSize::sheet`): `S` fits, `M` comes half up, `L`
//! is all up at once; `sheet=Sheet::…` where what it holds asks for another way (a long list of
//! keys half up, a form with a search all up, what changes `Sheet::Auto`, which fits while it can
//! and goes half up past that, a few choices `Sheet::Menu`, as tall as they are). A menu, the
//! filters and the map are sheets of their own (`data-sheet="menu"`, `"fit"`, `"half"`, `"full"`,
//! `"auto"`).
//!
//! What holds its buttons is a `footer.st-dlg-foot` (or carries `data-sheet-foot`): on a phone it
//! stays at the screen's foot while the sheet is half up. What scrolls is a `div.st-dlg-body` (or
//! carries `data-sheet-body`): on a phone it scrolls all up only, and where the sheet fits. What it
//! shows should not go away before the dialog has closed: content read from what `on_close`
//! empties is best read from what it was last (`kept`).

use leptos::prelude::*;

use crate::ui::DialogClose;

/// How large a dialog is on a wide screen, by what it holds (app.css „The dialogs"; owner,
/// 2026-10-16: „Modale menus springen zu viel in der größe. Erstelle dir mal 3 verschiedene
/// größen"): a question or a note (`S`, 440 px wide, as tall as what it says), a form or an
/// overview (`M`, 600 × 600 px), a list to choose from (`L`, 840 × 720 px). On a phone each stands
/// as its kind's `Sheet` does, unless the dialog says otherwise.
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

    /// How a dialog of this size stands on a phone: a question or a note fits, a form or an
    /// overview comes half up, a list to choose from all up at once.
    pub fn sheet(self) -> Sheet {
        match self {
            ModalSize::S => Sheet::Fit,
            ModalSize::M => Sheet::Half,
            ModalSize::L => Sheet::Full,
        }
    }
}

/// Where a placed dialog stands in the window (`Modal`'s `at`), in CSS pixels: its left and its
/// top, the edge that stays still while what it holds grows (it grows downward), and how tall it
/// stays at least while something stands open in it (`least`, 0 for no such height), so that
/// what opens shorter than what it rests with moves no edge either.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub left: f64,
    pub top: f64,
    pub least: f64,
}

impl Placed {
    /// Its style: `--x`, `--y` and `--h`, data as custom properties (R7).
    pub fn style(self) -> String {
        format!("--x:{:.0}px;--y:{:.0}px;--h:{:.0}px", self.left, self.top, self.least)
    }
}

/// How a sheet of a phone stands (`data-sheet`; enhance.js and app.css „the sheets of a phone";
/// owner, 2026-11-03: „dass es entweder fit ist, also so dass es nur ein kleines fenster ist. Da
/// würde ich die größe zwischen 40% bis 70% vom screen erlauben. Und da gibt es dann kein full
/// screen. Dann die Option mit 50% und Full screen. Und dann noch die Option für sofort full
/// screen"; and a menu, 2026-11-17: „eine menu option hinzuzufügen, wo der content einfach perfekt
/// passt und das modal auch nicht auf full gezogen werden kann"). A finger throws each away; each
/// comes up from below.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sheet {
    /// A small window: as tall as what it holds, between 40 and 70 % of the screen, and no higher;
    /// what it holds beyond scrolls in it. A question, a note.
    Fit,
    /// Half up as it comes, all up by a finger or its grabber; half up nothing in it scrolls, and
    /// its buttons stand at the screen's foot. What is read and worked in beside the page: a form,
    /// an overview, a module, the filters.
    #[default]
    Half,
    /// All up at once, the whole screen: what needs all of it, a list to choose from.
    Full,
    /// As what it holds says, for what changes (owner, 2026-11-03: „ein Auto Modus, der zwischen
    /// fit und Half entscheidet für dynamischen content"): `Fit` while it holds no more than fits
    /// (70 % of the screen), `Half` once it holds more, and back (enhance.js `autoMode`). A list
    /// that grows and shrinks, a search's answers, what differs from one to the next.
    Auto,
    /// A menu: exactly as tall as what it holds, however little that is (no 40 % as `Fit` has), up
    /// to the whole height, where what it holds beyond scrolls in it; one stop, which no finger
    /// pulls higher (it gives a little and comes back). The menus of a row, of ⋯, of „Einplanen in
    /// …", their submenus, and a question asked in a menu's place (every `.st-pop`).
    Menu,
}

impl Sheet {
    /// Its `data-sheet`.
    pub fn attr(self) -> &'static str {
        match self {
            Sheet::Fit => "fit",
            Sheet::Half => "half",
            Sheet::Full => "full",
            Sheet::Auto => "auto",
            Sheet::Menu => "menu",
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
    /// How it stands on a phone, where its size's way (`ModalSize::sheet`) does not suit what it
    /// holds.
    #[prop(optional, into)]
    sheet: Option<Signal<Sheet>>,
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
    /// Where it stands on a wide screen instead of the window's middle (`Placed`; app.css
    /// `.st-dialog.placed`). `None`: the middle. A phone's sheet stands as every sheet.
    #[prop(optional, into)]
    at: Signal<Option<Placed>>,
    /// Esc while something stands open inside it: `true` where its owner closed that and the dialog
    /// stays, `false` where the dialog closes as ever. One Esc, one layer.
    #[prop(optional)]
    on_cancel: Option<Callback<(), bool>>,
    /// A click beside it at this point of the window: `true` where its owner took it (what it shows
    /// changes), `false` where the dialog closes as ever.
    #[prop(optional_no_strip)]
    on_beside: Option<Callback<(f64, f64), bool>>,
    children: ChildrenFn,
) -> impl IntoView {
    let node = NodeRef::<leptos::html::Dialog>::new();
    // A click beside it a moment after it came was meant for what opened it (the second click of
    // a slow double click on „+ Termin" closed the form it had opened): it takes none for 400 ms.
    let born = crate::nav::Newborn::now();
    // Shown while `open` says so, from the frame it is in the page.
    Effect::new(move |_| {
        let open = open.get();
        let Some(dialog) = node.get() else { return };
        if open && !dialog.open() {
            born.again();
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
        if on_backdrop(node, &ev) && !born.swallows(&ev) {
            let point = (f64::from(ev.client_x()), f64::from(ev.client_y()));
            if !on_beside.is_some_and(|ask| ask.run(point)) {
                on_close.run(());
            }
        }
    };
    let class = move || {
        let size = size.get().class();
        let placed = if at.get().is_some() { "placed" } else { "" };
        ["st-dialog", size, placed, class].iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join(" ")
    };
    view! {
        <dialog
            class=class
            node_ref=node
            data-sheet=move || sheet.map_or_else(|| size.get().sheet(), |sheet| sheet.get()).attr()
            aria-labelledby=labelled_by
            aria-label=move || label.map(|label| label.get())
            tabindex=focus_itself.then_some("-1")
            autofocus=focus_itself
            style=move || at.get().map(Placed::style)
            on:cancel=move |ev: leptos::ev::Event| {
                if on_cancel.is_some_and(|ask| ask.run(())) {
                    ev.prevent_default();
                }
            }
            on:close=move |_| on_close.run(())
            on:click=beside
            // The press of such a click neither takes the keyboard from the field it has.
            on:mousedown=move |ev: leptos::ev::MouseEvent| {
                if on_backdrop(node, &ev) {
                    born.swallows(&ev);
                }
            }
        >
            {move || open.get().then(|| view! { <div class="st-dlg">{children()}</div> })}
        </dialog>
    }
}

/// Whether a dialog stands open over the page other than one `but` matches (a selector): the form
/// „Neuer Termin", the keys' overview, a card. A listener of Esc for what lies under the dialogs
/// leaves the key to the one in front (one Esc, the topmost layer). The copy of a dialog in the
/// picture held over a step is none.
pub fn dialog_in_front(but: Option<&str>) -> bool {
    #[cfg(feature = "csr")]
    {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else { return false };
        let Ok(open) = document.query_selector_all("dialog[open]") else { return false };
        (0..open.length()).filter_map(|at| open.item(at)).filter_map(|node| wasm_bindgen::JsCast::dyn_into::<web_sys::Element>(node).ok()).any(|dialog| {
            dialog.closest(".pending-page.held").ok().flatten().is_none() && !but.is_some_and(|but| dialog.matches(but).unwrap_or(false))
        })
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = but;
        false
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
