//! What „Mein Studium" asks of the browser itself: where a button or the pointer is, which row lies
//! under the pointer while a selection is drawn, what a menu gives the focus back to, the picture a
//! drag of several rows carries. Nothing of it on the server, which has neither.

#[cfg(feature = "csr")]
use wasm_bindgen::JsCast;

/// A box on the screen, in CSS pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// The box of the element an event was heard on (`currentTarget`): the button clicked.
#[allow(unused_variables)]
pub(super) fn rect_of(ev: &leptos::ev::Event) -> Option<Rect> {
    #[cfg(feature = "csr")]
    {
        let element = ev.current_target()?.dyn_into::<web_sys::Element>().ok()?;
        Some(rect(&element))
    }
    #[cfg(not(feature = "csr"))]
    None
}

#[cfg(feature = "csr")]
fn rect(element: &web_sys::Element) -> Rect {
    let r = element.get_bounding_client_rect();
    Rect { left: r.left(), top: r.top(), right: r.right(), bottom: r.bottom() }
}

/// The window's width and height.
pub(super) fn viewport() -> (f64, f64) {
    #[cfg(feature = "csr")]
    {
        let size = web_sys::window().and_then(|window| Some((window.inner_width().ok()?.as_f64()?, window.inner_height().ok()?.as_f64()?)));
        size.unwrap_or((1280.0, 800.0))
    }
    #[cfg(not(feature = "csr"))]
    (1280.0, 800.0)
}

/// The key of the semester's row under the point (`.st-row[data-key]`), if a row is there.
#[allow(unused_variables)]
pub(super) fn row_at(x: f64, y: f64) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        let document = web_sys::window()?.document()?;
        #[allow(clippy::cast_possible_truncation)]
        let element = document.element_from_point(x as f32, y as f32)?;
        element.closest(".st-card-sem .st-row[data-key]").ok()??.get_attribute("data-key")
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The key of the semester's row nearest to the height `y`: for a pointer that left the rows
/// (above the first, below the last) the first or the last.
#[allow(unused_variables)]
pub(super) fn row_nearest(y: f64) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        let document = web_sys::window()?.document()?;
        let rows = document.query_selector_all(".st-card-sem .st-row[data-key]").ok()?;
        let mut best: Option<(f64, String)> = None;
        for i in 0..rows.length() {
            let Some(row) = rows.item(i).and_then(|node| node.dyn_into::<web_sys::Element>().ok()) else { continue };
            let r = row.get_bounding_client_rect();
            let distance = if y < r.top() { r.top() - y } else if y > r.bottom() { y - r.bottom() } else { 0.0 };
            if best.as_ref().is_none_or(|(d, _)| distance < *d) {
                best = row.get_attribute("data-key").map(|key| (distance, key));
            }
        }
        best.map(|(_, key)| key)
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Holds on to the pointer for the element the event was heard on: its moves come there while the
/// button is down, wherever it goes.
#[allow(unused_variables)]
pub(super) fn capture(ev: &leptos::ev::PointerEvent) {
    #[cfg(feature = "csr")]
    if let Some(element) = ev.current_target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()) {
        let _ = element.set_pointer_capture(ev.pointer_id());
    }
}

#[cfg(feature = "csr")]
thread_local! {
    /// What opened the menu open: the focus goes back there when it closes.
    static TRIGGER: std::cell::RefCell<Option<web_sys::HtmlElement>> = const { std::cell::RefCell::new(None) };
}

/// Keeps the element the event was heard on to give the focus back to (`give_focus_back`).
#[allow(unused_variables)]
pub(super) fn keep_trigger(ev: &leptos::ev::Event) {
    #[cfg(feature = "csr")]
    {
        let element = ev.current_target().and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok());
        TRIGGER.with(|trigger| *trigger.borrow_mut() = element);
    }
}

/// Gives the focus back to what opened the menu, if it is still on the page.
pub(super) fn give_focus_back() {
    #[cfg(feature = "csr")]
    {
        let element = TRIGGER.with(|trigger| trigger.borrow_mut().take());
        if let Some(element) = element.filter(|element| element.is_connected()) {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = element.focus_with_options(&options);
        }
    }
}

/// Moves the focus among the entries of the menu panel `panel` (a selector): `step` on from the
/// one that has it (wrapping round), or to the first (`Some(0)` from none) or the last.
#[allow(unused_variables)]
pub(super) fn step_focus(panel: &str, step: Step) {
    #[cfg(feature = "csr")]
    {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else { return };
        let Ok(items) = document.query_selector_all(&format!("{panel} [role^=\"menuitem\"]:not([disabled])")) else { return };
        let items: Vec<web_sys::HtmlElement> = (0..items.length()).filter_map(|i| items.item(i)?.dyn_into::<web_sys::HtmlElement>().ok()).collect();
        if items.is_empty() {
            return;
        }
        let active = document.active_element();
        let at = items.iter().position(|item| active.as_ref().is_some_and(|active| active.is_same_node(Some(item))));
        let last = items.len() - 1;
        let to = match (step, at) {
            (Step::First, _) | (Step::Next, None) => 0,
            (Step::Last, _) | (Step::Previous, None) => last,
            (Step::Next, Some(at)) => if at == last { 0 } else { at + 1 },
            (Step::Previous, Some(at)) => if at == 0 { last } else { at - 1 },
        };
        if let Some(item) = items.get(to) {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = item.focus_with_options(&options);
        }
    }
}

/// Where the focus goes among a menu's entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    First,
    Last,
    Next,
    Previous,
}

/// An attribute of the element that has the focus.
#[allow(unused_variables)]
pub(super) fn focused_attribute(name: &str) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        web_sys::window()?.document()?.active_element()?.get_attribute(name)
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Gives a drag the picture of the element `selector` finds („3 Einträge") instead of the row
/// it began on.
#[allow(unused_variables)]
pub(super) fn drag_picture(ev: &leptos::ev::DragEvent, selector: &str) {
    #[cfg(feature = "csr")]
    {
        let picture = web_sys::window().and_then(|window| window.document()).and_then(|document| document.query_selector(selector).ok().flatten());
        if let (Some(data), Some(picture)) = (ev.data_transfer(), picture) {
            data.set_drag_image(&picture, 16, 16);
        }
    }
}

/// Puts what a drag carries into it: the keys of its rows, a move.
#[allow(unused_variables)]
pub(super) fn drag_data(ev: &leptos::ev::DragEvent, keys: &[String]) {
    #[cfg(feature = "csr")]
    if let Some(data) = ev.data_transfer() {
        let _ = data.set_data("text/plain", &keys.join("\n"));
        data.set_effect_allowed("move");
    }
}

/// Follows the module link of the row the event was heard in (`a.st-name`), as a click on it
/// does: the module beside the page. Once this click is over, since the link's own click comes
/// back to the row (which does nothing with a click on a link).
#[allow(unused_variables)]
pub(super) fn follow_name(ev: &leptos::ev::MouseEvent) {
    #[cfg(feature = "csr")]
    {
        let link = ev.current_target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).and_then(|body| body.query_selector("a.st-name").ok().flatten());
        if let Some(link) = link.and_then(|link| link.dyn_into::<web_sys::HtmlElement>().ok()) {
            leptos::prelude::set_timeout(move || link.click(), std::time::Duration::ZERO);
        }
    }
}

/// One step back through the browser's history: where the step before is the page a link leads
/// back to, the same entry as before, and the history does not grow.
pub(super) fn history_back() {
    #[cfg(feature = "csr")]
    if let Some(history) = web_sys::window().and_then(|window| window.history().ok()) {
        let _ = history.back();
    }
}

/// Whether the event began on a link or a button inside the element it is heard on: those do what
/// they say, not what the row does.
#[allow(unused_variables)]
pub(super) fn on_control(ev: &leptos::ev::Event) -> bool {
    #[cfg(feature = "csr")]
    {
        let Some(target) = ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()) else { return false };
        let Some(within) = ev.current_target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()) else { return false };
        target.closest("a[href], button").ok().flatten().is_some_and(|control| {
            let control: &web_sys::Node = control.as_ref();
            within.contains(Some(control))
        })
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// Whether the event began on the module's link of a row (`a.st-name`).
#[allow(unused_variables)]
pub(super) fn on_name(ev: &leptos::ev::Event) -> bool {
    #[cfg(feature = "csr")]
    {
        ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).is_some_and(|target| target.closest("a.st-name").ok().flatten().is_some())
    }
    #[cfg(not(feature = "csr"))]
    false
}
