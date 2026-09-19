//! Browser-only helpers: plain forms become client-side navigation, and the endless list learns
//! where the visitor is. On the server (and before the browser app has taken over) all of them
//! do nothing: forms submit, and the list is one page with pager links.

/// What happened to the form.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FormEvent {
    /// A field changed. On a phone this does nothing: the filter sheet has an apply button.
    Change,
    Submit,
}

/// The query string the GET form of this event would submit, and the submission prevented.
/// `None` on the server, for events outside a form, and for changes on a phone.
#[allow(unused_variables)]
pub fn form_query(ev: &leptos::ev::Event, kind: FormEvent) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let target = ev.target()?;
        let form: web_sys::HtmlFormElement = match target.dyn_ref::<web_sys::HtmlFormElement>() {
            Some(form) => form.clone(),
            None => target.dyn_ref::<web_sys::Element>()?.closest("form").ok()??.dyn_into().ok()?,
        };
        if kind == FormEvent::Change {
            let phone = web_sys::window()?.match_media("(max-width: 900px)").ok()??.matches();
            if phone {
                return None;
            }
        }
        ev.prevent_default();
        let data = web_sys::FormData::new_with_form(&form).ok()?;
        let params = web_sys::UrlSearchParams::new_with_str_sequence_sequence(&data).ok()?;
        Some(String::from(params.to_string()))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Where the visitor is in a list made of `[data-page]` chunks.
pub struct ListPosition {
    /// The page of the chunk at the top of what is visible.
    pub page: Option<u64>,
    /// Close enough to the end that the next chunk should be there before it is reached.
    pub near_end: bool,
}

/// Reads the position from the DOM. The list scrolls inside its panel on the desktop and with
/// the window on a phone; both are handled. `None` on the server or if the list is not there.
#[allow(unused_variables)]
pub fn list_position(rows_id: &str) -> Option<ListPosition> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let window = web_sys::window()?;
        let document = window.document()?;
        let rows = document.get_element_by_id(rows_id)?;
        let own_scroll = rows.scroll_height() > rows.client_height() + 4;

        let line = if own_scroll { rows.get_bounding_client_rect().top() + 60.0 } else { 140.0 };
        let chunks = rows.query_selector_all("[data-page]").ok()?;
        let mut page = None;
        for i in 0..chunks.length() {
            let Some(chunk) = chunks.item(i).and_then(|node| node.dyn_into::<web_sys::Element>().ok()) else { continue };
            if chunk.get_bounding_client_rect().bottom() > line {
                page = chunk.get_attribute("data-page").and_then(|n| n.parse().ok());
                break;
            }
        }

        let near_end = if own_scroll {
            f64::from(rows.scroll_height() - rows.scroll_top() - rows.client_height()) < 900.0
        } else {
            let root = document.document_element()?;
            let viewport = window.inner_height().ok()?.as_f64()?;
            f64::from(root.scroll_height()) - viewport - window.scroll_y().ok()? < 1200.0
        };
        Some(ListPosition { page, near_end })
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The height of the list's content, to be handed to `keep_position_after_prepend`.
#[allow(unused_variables)]
pub fn list_height(rows_id: &str) -> f64 {
    #[cfg(feature = "csr")]
    {
        let rows = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(rows_id));
        rows.map(|rows| f64::from(rows.scroll_height())).unwrap_or(0.0)
    }
    #[cfg(not(feature = "csr"))]
    0.0
}

/// After rows were added above: move the scroll position by what was added, so that what the
/// visitor was looking at stays where it was.
#[allow(unused_variables)]
pub fn keep_position_after_prepend(rows_id: &'static str, height_before: f64) {
    #[cfg(feature = "csr")]
    leptos::prelude::request_animation_frame(move || {
        let Some(window) = web_sys::window() else { return };
        let Some(rows) = window.document().and_then(|d| d.get_element_by_id(rows_id)) else { return };
        let added = f64::from(rows.scroll_height()) - height_before;
        if added <= 0.0 {
            return;
        }
        if rows.scroll_height() > rows.client_height() + 4 {
            rows.set_scroll_top(rows.scroll_top() + added as i32);
        } else {
            window.scroll_by_with_x_and_y(0.0, added);
        }
    });
}
