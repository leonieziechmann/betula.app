//! Browser-only helpers: the endless list learns where the visitor is, pickers place their
//! popups and move the focus. On the server (and before the browser app has taken over) all of
//! them do nothing.

/// Where the visitor is in a list whose pages each start with a `[data-page]` row.
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
        // The page on screen is the last one that starts above the line.
        let starts = rows.query_selector_all("[data-page]").ok()?;
        let mut page = None;
        for i in 0..starts.length() {
            let Some(start) = starts.item(i).and_then(|node| node.dyn_into::<web_sys::Element>().ok()) else { continue };
            if page.is_some() && start.get_bounding_client_rect().top() > line {
                break;
            }
            page = start.get_attribute("data-page").and_then(|n| n.parse().ok());
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

/// Where the popup of a picker goes (fixed to the window, so no panel clips it): under its
/// button, or above it when there is more room.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopupPlace {
    pub left: f64,
    pub top: Option<f64>,
    pub bottom: Option<f64>,
    pub width: f64,
    pub max_height: f64,
}

impl PopupPlace {
    /// Custom properties for `.combo-pop`; a phone ignores them and shows the popup in place.
    pub fn style(&self) -> String {
        let vertical = match (self.top, self.bottom) {
            (Some(top), _) => format!("--y:{top:.0}px"),
            (None, Some(bottom)) => format!("--b:{bottom:.0}px"),
            (None, None) => String::new(),
        };
        format!("--x:{:.0}px;--w:{:.0}px;--h:{:.0}px;{vertical}", self.left, self.width, self.max_height)
    }
}

/// `None` on the server or if the button is not there.
#[allow(unused_variables)]
pub fn popup_place(trigger_id: &str, min_width: f64) -> Option<PopupPlace> {
    #[cfg(feature = "csr")]
    {
        let window = web_sys::window()?;
        let rect = window.document()?.get_element_by_id(trigger_id)?.get_bounding_client_rect();
        let viewport_width = window.inner_width().ok()?.as_f64()?;
        let viewport_height = window.inner_height().ok()?.as_f64()?;
        let width = rect.width().max(min_width).min(viewport_width - 16.0);
        let left = rect.left().min(viewport_width - width - 8.0).max(8.0);
        let (below, above) = (viewport_height - rect.bottom() - 12.0, rect.top() - 12.0);
        Some(if below >= 300.0 || below >= above {
            PopupPlace { left, top: Some(rect.bottom() + 4.0), bottom: None, width, max_height: below.clamp(160.0, 460.0) }
        } else {
            PopupPlace { left, top: None, bottom: Some(viewport_height - rect.top() + 4.0), width, max_height: above.clamp(160.0, 460.0) }
        })
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Moves the focus without scrolling anything; text fields get their content selected.
#[allow(unused_variables)]
pub fn focus_by_id(id: &str) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let element = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(id));
        let Some(element) = element.and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) else { return };
        let options = web_sys::FocusOptions::new();
        options.set_prevent_scroll(true);
        let _ = element.focus_with_options(&options);
        if let Some(input) = element.dyn_ref::<web_sys::HtmlInputElement>() {
            input.select();
        }
    }
}

/// Scrolls the list around an entry (its parent, and nothing else) so that the entry is
/// visible; `center` puts it in the middle, for the first look at a long list.
#[allow(unused_variables)]
pub fn reveal_in_list(entry_id: &str, center: bool) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let entry = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(entry_id));
        let Some(entry) = entry.and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) else { return };
        let Some(list) = entry.parent_element() else { return };
        let (top, height, view) = (entry.offset_top(), entry.offset_height(), list.client_height());
        if center {
            list.set_scroll_top((top - (view - height) / 2).max(0));
        } else if top < list.scroll_top() {
            list.set_scroll_top((top - 4).max(0));
        } else if top + height > list.scroll_top() + view {
            list.set_scroll_top(top + height - view + 4);
        }
    }
}

/// Closes the filter sheet of the phone layout.
pub fn close_filter_sheet() {
    #[cfg(feature = "csr")]
    if let Some(filters) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id("filters")) {
        let _ = filters.class_list().remove_1("open");
    }
}

/// The phone layout (one column, bottom bar) is in use. `false` on the server.
pub fn is_phone() -> bool {
    #[cfg(feature = "csr")]
    {
        web_sys::window().and_then(|w| w.match_media("(max-width: 900px)").ok().flatten()).is_some_and(|query| query.matches())
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// What this browser session remembers under `key` (`sessionStorage`). `None` on the server.
#[allow(unused_variables)]
pub fn session_get(key: &str) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        web_sys::window()?.session_storage().ok()??.get_item(key).ok()?
    }
    #[cfg(not(feature = "csr"))]
    None
}

#[allow(unused_variables)]
pub fn session_set(key: &str, value: &str) {
    #[cfg(feature = "csr")]
    if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
        let _ = storage.set_item(key, value);
    }
}

/// What this browser remembers under `key` (`localStorage`, so it outlives the session): a
/// personal view setting, never part of the URL and never part of server HTML (R9). `None` on
/// the server, and in a browser that refuses storage.
#[allow(unused_variables)]
pub fn local_get(key: &str) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        web_sys::window()?.local_storage().ok()??.get_item(key).ok()?
    }
    #[cfg(not(feature = "csr"))]
    None
}

#[allow(unused_variables)]
pub fn local_set(key: &str, value: &str) {
    #[cfg(feature = "csr")]
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(key, value);
    }
}

/// The number in the `data-i` of the element an event hit, or of the closest element around it
/// that has one: one handler for many small targets (the dots of the landing page's map).
pub fn index_under(target: Option<leptos::web_sys::EventTarget>) -> Option<usize> {
    use leptos::wasm_bindgen::JsCast;
    let element = target?.dyn_into::<leptos::web_sys::Element>().ok()?;
    element.closest("[data-i]").ok()??.get_attribute("data-i")?.parse().ok()
}

/// Scrolls the first element that matches `selector` into the middle of what scrolls around it.
/// `false` if there is none.
#[allow(unused_variables)]
pub fn reveal_selector(selector: &str) -> bool {
    #[cfg(feature = "csr")]
    {
        let found = web_sys::window().and_then(|w| w.document()).and_then(|d| d.query_selector(selector).ok().flatten());
        let Some(element) = found else { return false };
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Center);
        element.scroll_into_view_with_scroll_into_view_options(&options);
        true
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// Scrolls the list so that the row of this module is in the middle. `false` if the row is not
/// (yet) part of the list.
#[allow(unused_variables)]
pub fn reveal_row(rows_id: &str, module_id: &str) -> bool {
    #[cfg(feature = "csr")]
    {
        let rows = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(rows_id));
        let safe = module_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        let row = rows.filter(|_| safe).and_then(|rows| rows.query_selector(&format!("a.row[data-id=\"{module_id}\"]")).ok().flatten());
        let Some(row) = row else { return false };
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Center);
        row.scroll_into_view_with_scroll_into_view_options(&options);
        true
    }
    #[cfg(not(feature = "csr"))]
    false
}
