//! The question before something that takes away (owner, 2026-11-09: „Also kannst du bitte (kontext)
//! menus verwenden, um das löschen zu bestätigen. Das andere verschiebt das layout und ist dafür
//! einfach nicht gut geeignet."): a small menu at the button that asked, as the app's menus are
//! (app.css `.st-pop`), a sheet from below on a phone. It says what is asked and what follows, then
//! the answer and „Abbrechen"; nothing on the page moves. The answer has the focus; Esc, a click
//! beside it, „Abbrechen" and Back on a phone close it, and the focus goes back to the button.
//!
//! The profile page asks with it before „Profil leeren", „Profil löschen", „Entfernen" of a device,
//! „Nur dieses Gerät trennen" and „Sync beenden".

use leptos::portal::Portal;
use leptos::prelude::*;

use folia_design::nav;
use folia_design::ui::Icon;

use crate::i18n;

/// The menu's width on a wide screen (app.css `.pf-confirm`).
const WIDTH: f64 = 300.0;
/// How far it stays from the window's edges and from its button.
const EDGE: f64 = 8.0;
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const GAP: f64 = 6.0;

/// What is asked.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    /// The id of the button that asked: the menu stands at it, the focus goes back to it.
    pub at: String,
    /// What is asked („„iPhone“ aus Betula Sync nehmen?"), and what follows.
    pub title: String,
    pub hint: String,
    /// The answer, its icon and its id (what the caller tells its questions apart by).
    pub yes: String,
    pub icon: &'static str,
    pub yes_id: &'static str,
}

/// The menu of `asking` while it holds a question; `on_yes` hears the answer's id.
#[component]
pub(crate) fn Confirm(asking: RwSignal<Option<Question>>, #[prop(into)] on_yes: Callback<&'static str>) -> impl IntoView {
    let t = i18n::t();
    let close = move || {
        if let Some(question) = asking.get_untracked() {
            asking.set(None);
            let at = question.at;
            request_animation_frame(move || nav::focus_by_id(&at));
        }
    };
    folia_shell::nav::layer_while(move || asking.with(Option::is_some), move || asking.set(None), folia_shell::nav::Kind::Over);
    // Where it stands: under its button, its right edge at the button's, or above it where there
    // is no room below (measured once it is drawn).
    let place = RwSignal::new((EDGE, EDGE));
    Effect::new(move |_| {
        let Some(question) = asking.get() else { return };
        place.set(at_button(&question.at, None));
        let yes = question.yes_id;
        request_animation_frame(move || {
            place.set(at_button(&question.at, height_of("pf-confirm")));
            nav::focus_by_id(yes);
        });
    });
    let keys = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() == "Escape" {
            ev.prevent_default();
            ev.stop_propagation();
            close();
        }
    };
    move || {
        asking.get().map(|question| {
            let yes_id = question.yes_id;
            view! {
                <Portal>
                    <div class="st-pop-shade pf-confirm-shade" on:mousedown=|ev: leptos::ev::MouseEvent| ev.prevent_default() on:click=move |_| close()></div>
                    <div
                        id="pf-confirm"
                        data-sheet="auto"
                        class="st-pop pf-confirm"
                        role="alertdialog"
                        aria-modal="true"
                        aria-labelledby="pf-confirm-title"
                        aria-describedby="pf-confirm-hint"
                        style=move || { let (x, y) = place.get(); format!("--x:{x:.0}px;--y:{y:.0}px;--w:{WIDTH:.0}px") }
                        on:keydown=keys
                    >
                        <p class="pf-confirm-title" id="pf-confirm-title">{question.title.clone()}</p>
                        <p class="pf-confirm-hint" id="pf-confirm-hint">{question.hint.clone()}</p>
                        <button class="st-pop-item danger" type="button" id=yes_id on:click=move |_| {
                            asking.set(None);
                            on_yes.run(yes_id);
                        }>
                            <Icon name=question.icon/><span>{question.yes.clone()}</span>
                        </button>
                        <button class="st-pop-item" type="button" on:click=move |_| close()>
                            <Icon name="x"/><span>{t.profile.cancel}</span>
                        </button>
                    </div>
                </Portal>
            }
        })
    }
}

/// The height of the element `id` as drawn.
#[allow(unused_variables)]
fn height_of(id: &str) -> Option<f64> {
    #[cfg(feature = "csr")]
    {
        web_sys::window()?.document()?.get_element_by_id(id).map(|el| el.get_bounding_client_rect().height())
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The menu's place in the window at the button `at`: under it, right-aligned with it, within the
/// window; above it where it is `height` high and there is no room below.
#[allow(unused_variables)]
fn at_button(at: &str, height: Option<f64>) -> (f64, f64) {
    #[cfg(feature = "csr")]
    {
        let Some(window) = web_sys::window() else { return (EDGE, EDGE) };
        let Some(button) = folia_design::stage::by_id(at) else { return (EDGE, EDGE) };
        let rect = button.get_bounding_client_rect();
        let width = window.inner_width().ok().and_then(|w| w.as_f64()).unwrap_or(1280.0);
        let tall = window.inner_height().ok().and_then(|h| h.as_f64()).unwrap_or(800.0);
        let x = (rect.right() - WIDTH).min(width - WIDTH - EDGE).max(EDGE);
        let below = rect.bottom() + GAP;
        let y = match height {
            Some(height) if below + height > tall - EDGE && rect.top() - GAP - height >= EDGE => rect.top() - GAP - height,
            _ => below,
        };
        (x, y)
    }
    #[cfg(not(feature = "csr"))]
    (EDGE, EDGE)
}
