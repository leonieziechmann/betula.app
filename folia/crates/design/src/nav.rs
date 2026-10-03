//! Browser-only helpers: the virtual list learns where the visitor is and how tall its rows
//! are, pickers place their popups and move the focus. On the server (and before the browser app
//! has taken over) all of them do nothing.

/// What is visible of a virtual list: the offset of the top of the visible part within the
/// list's content (`content_id`, the element that holds the rows), and the height of the visible
/// part. On the desktop the list scrolls with the page's scroll area (`scroller_id`, app.css „one
/// scroll area"), on a phone with the window. In the area the heads (`heads`: the list's head, then
/// the heads of its columns) stay at the top while the rows scroll under them: the visible part
/// begins below the last of them. `None` on the server or if the list is not there.
#[allow(unused_variables)]
pub fn list_viewport(scroller_id: &str, heads: &[&str], content_id: &str) -> Option<(f32, f32)> {
    #[cfg(feature = "csr")]
    {
        let window = web_sys::window()?;
        let document = window.document()?;
        let content = document.get_element_by_id(content_id)?.get_bounding_client_rect();
        if is_phone() {
            let height = window.inner_height().ok()?.as_f64()? as f32;
            return Some(((-content.top() as f32).max(0.0), height));
        }
        let area = document.get_element_by_id(scroller_id)?;
        let area_box = area.get_bounding_client_rect();
        let under_heads = heads.last().and_then(|id| document.get_element_by_id(id)).map_or(area_box.top(), |head| head.get_bounding_client_rect().bottom());
        let top = under_heads.max(area_box.top()) as f32;
        let bottom = (area_box.top() + f64::from(area.client_height())) as f32;
        Some(((top - content.top() as f32).max(0.0), (bottom - top).max(0.0)))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// How tall the heads are together; 0 without them (and on a phone, which shows no heads).
#[cfg(feature = "csr")]
fn height_of(document: &web_sys::Document, ids: &[&str]) -> f32 {
    use wasm_bindgen::JsCast;
    ids.iter()
        .filter_map(|id| document.get_element_by_id(id).and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok()))
        .map(|element| element.offset_height() as f32)
        .sum()
}

/// Puts the list at its start: the scroll area at the top, or (a phone) the window at the top of the page.
#[allow(unused_variables)]
pub fn scroll_list_to_start(rows_id: &str) {
    #[cfg(feature = "csr")]
    {
        let Some(window) = web_sys::window() else { return };
        if is_phone() {
            let options = web_sys::ScrollToOptions::new();
            options.set_top(0.0);
            options.set_behavior(web_sys::ScrollBehavior::Instant);
            window.scroll_to_with_scroll_to_options(&options);
        } else if let Some(rows) = window.document().and_then(|d| d.get_element_by_id(rows_id)) {
            rows.set_scroll_top(0);
        }
    }
}

/// Scrolls the list so that `offset` (within its content) is at the top of the visible part.
#[allow(unused_variables)]
pub fn scroll_list_to(scroller_id: &str, heads: &[&str], content_id: &str, offset: f32) {
    // In the scroll area, from where the content stands in it rather than from what is visible
    // now: what stands above the rows (the note of a semester) scrolls away on the way there, and
    // the heads stand at the top once it has.
    #[cfg(feature = "csr")]
    if !is_phone() {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
        let (Some(area), Some(content)) = (document.get_element_by_id(scroller_id), document.get_element_by_id(content_id)) else { return };
        let start = content.get_bounding_client_rect().top() - area.get_bounding_client_rect().top() + f64::from(area.scroll_top());
        area.set_scroll_top((start + f64::from(offset) - f64::from(height_of(&document, heads))).round() as i32);
        return;
    }
    if let Some((now, _)) = list_viewport(scroller_id, heads, content_id) {
        scroll_list_by(scroller_id, offset - now);
    }
}

/// Scrolls the list by `by` pixels, at once (no smooth scrolling: what is compensated must not be seen).
#[allow(unused_variables)]
pub fn scroll_list_by(rows_id: &str, by: f32) {
    #[cfg(feature = "csr")]
    {
        let Some(window) = web_sys::window() else { return };
        if is_phone() {
            let options = web_sys::ScrollToOptions::new();
            options.set_top(f64::from(by));
            options.set_behavior(web_sys::ScrollBehavior::Instant);
            window.scroll_by_with_scroll_to_options(&options);
        } else if let Some(rows) = window.document().and_then(|d| d.get_element_by_id(rows_id)) {
            rows.set_scroll_top(rows.scroll_top() + by.round() as i32);
        }
    }
}

/// The rendered rows of a virtual list (`[data-i]` children of `content_id`) with their heights.
#[allow(unused_variables)]
pub fn measure_rows(content_id: &str) -> Vec<(usize, f32)> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let Some(content) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(content_id)) else { return Vec::new() };
        let Ok(rows) = content.query_selector_all("[data-i]") else { return Vec::new() };
        (0..rows.length())
            .filter_map(|i| rows.item(i).and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok()))
            .filter_map(|row| Some((row.get_attribute("data-i")?.parse().ok()?, row.offset_height() as f32)))
            .collect()
    }
    #[cfg(not(feature = "csr"))]
    Vec::new()
}

/// A `ResizeObserver` on one element; dropping it stops the watching. Wrapped so that a cleanup
/// (which has to be `Send`) can hold it: the browser app has one thread.
#[cfg(feature = "csr")]
pub struct SizeWatch {
    observer: web_sys::ResizeObserver,
    _callback: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::js_sys::Array)>,
}

#[cfg(feature = "csr")]
impl Drop for SizeWatch {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}

#[cfg(not(feature = "csr"))]
pub struct SizeWatch;

/// Calls `on_change` whenever the element's size changes (the panel is dragged, the window
/// changes, the preview opens). `None` on the server or without the element.
#[allow(unused_variables)]
pub fn watch_size(id: &str, on_change: impl Fn() + 'static) -> Option<send_wrapper::SendWrapper<SizeWatch>> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let element = web_sys::window()?.document()?.get_element_by_id(id)?;
        let callback = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::js_sys::Array)>::new(move |_entries| on_change());
        let observer = web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()).ok()?;
        observer.observe(&element);
        Some(send_wrapper::SendWrapper::new(SizeWatch { observer, _callback: callback }))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// A `scroll` listener on one element; dropping it removes the listener. Wrapped like `SizeWatch`.
#[cfg(feature = "csr")]
pub struct ScrollWatch {
    target: web_sys::EventTarget,
    callback: wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>,
}

#[cfg(feature = "csr")]
impl Drop for ScrollWatch {
    fn drop(&mut self) {
        use wasm_bindgen::JsCast;
        let _ = self.target.remove_event_listener_with_callback("scroll", self.callback.as_ref().unchecked_ref());
    }
}

#[cfg(not(feature = "csr"))]
pub struct ScrollWatch;

/// Calls `on_scroll` whenever the element scrolls (the page's scroll area). `None` on the server or
/// without the element.
#[allow(unused_variables)]
pub fn watch_scroll(id: &str, on_scroll: impl Fn() + 'static) -> Option<send_wrapper::SendWrapper<ScrollWatch>> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let target: web_sys::EventTarget = web_sys::window()?.document()?.get_element_by_id(id)?.into();
        let callback = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(move |_event| on_scroll());
        let options = web_sys::AddEventListenerOptions::new();
        options.set_passive(true);
        target.add_event_listener_with_callback_and_add_event_listener_options("scroll", callback.as_ref().unchecked_ref(), &options).ok()?;
        Some(send_wrapper::SendWrapper::new(ScrollWatch { target, callback }))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// How wide the element is on the screen, in CSS pixels. `None` on the server or without it.
#[allow(unused_variables)]
pub fn width_of(id: &str) -> Option<f64> {
    #[cfg(feature = "csr")]
    {
        Some(web_sys::window()?.document()?.get_element_by_id(id)?.get_bounding_client_rect().width())
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Where the popup of a picker goes. On a computer it is fixed to the window, so no panel clips
/// it: under its button, or above it when there is more room. On a phone it opens in place, under
/// its button, and spans the panel the picker stands in (`inset`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopupPlace {
    pub left: f64,
    pub top: Option<f64>,
    pub bottom: Option<f64>,
    pub width: f64,
    pub max_height: f64,
    /// How far the picker stands in from the left and the right edge of the panel it is in;
    /// `None` outside a panel.
    pub inset: Option<(f64, f64)>,
}

impl PopupPlace {
    /// Custom properties for `.combo-pop`: its place in the window, which a phone ignores, and
    /// how far in its panel the picker stands (`--pl`, `--pr`), which only a phone uses.
    pub fn style(&self) -> String {
        let vertical = match (self.top, self.bottom) {
            (Some(top), _) => format!("--y:{top:.0}px"),
            (None, Some(bottom)) => format!("--b:{bottom:.0}px"),
            (None, None) => String::new(),
        };
        let inset = self.inset.map(|(left, right)| format!("--pl:{left:.1}px;--pr:{right:.1}px;")).unwrap_or_default();
        format!("--x:{:.0}px;--w:{:.0}px;--h:{:.0}px;{inset}{vertical}", self.left, self.width, self.max_height)
    }
}

/// `None` on the server or if the button is not there.
#[allow(unused_variables)]
pub fn popup_place(trigger_id: &str, min_width: f64) -> Option<PopupPlace> {
    #[cfg(feature = "csr")]
    {
        let window = web_sys::window()?;
        let trigger = window.document()?.get_element_by_id(trigger_id)?;
        let rect = trigger.get_bounding_client_rect();
        let viewport_width = window.inner_width().ok()?.as_f64()?;
        let viewport_height = window.inner_height().ok()?.as_f64()?;
        let width = rect.width().max(min_width).min(viewport_width - 16.0);
        let left = rect.left().min(viewport_width - width - 8.0).max(8.0);
        let (below, above) = (viewport_height - rect.bottom() - 12.0, rect.top() - 12.0);
        // The popup's box (`.combo`, around the button) in its panel.
        let inset = trigger.closest(".combo").ok().flatten().zip(trigger.closest(".panel").ok().flatten()).map(|(combo, panel)| {
            let (combo, panel) = (combo.get_bounding_client_rect(), panel.get_bounding_client_rect());
            (combo.left() - panel.left(), panel.right() - combo.right())
        });
        Some(if below >= 300.0 || below >= above {
            PopupPlace { left, top: Some(rect.bottom() + 4.0), bottom: None, width, max_height: below.clamp(160.0, 460.0), inset }
        } else {
            PopupPlace { left, top: None, bottom: Some(viewport_height - rect.top() + 4.0), width, max_height: above.clamp(160.0, 460.0), inset }
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

/// `run` once the browser has shown what is there now: after the next frame (R21, „A click
/// answers first"). A click that starts work flips its own state, and the work follows here, so
/// the click is seen in the next frame whatever the work costs. A hidden tab has no frames, so
/// there (and should a frame not come) a timeout of 250 ms runs it instead. It runs once: the
/// frame and the timeout share a flag, and whichever comes first takes `run`. Nothing on the
/// server, which has neither frames nor clicks.
#[allow(unused_variables)]
pub fn after_paint(run: impl FnOnce() + 'static) {
    #[cfg(feature = "csr")]
    {
        use std::cell::Cell;
        use std::rc::Rc;

        use leptos::prelude::{request_animation_frame, set_timeout};

        let slot = Rc::new(Cell::new(Some(run)));
        let once = move || {
            if let Some(run) = slot.take() {
                run();
            }
        };
        let hidden = web_sys::window().and_then(|w| w.document()).is_some_and(|d| d.hidden());
        if hidden {
            set_timeout(once, std::time::Duration::ZERO);
            return;
        }
        let fallback = once.clone();
        request_animation_frame(move || set_timeout(once, std::time::Duration::ZERO));
        set_timeout(fallback, std::time::Duration::from_millis(250));
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

/// How tall the window is, in CSS pixels; `None` on the server.
pub fn screen_height() -> Option<f32> {
    #[cfg(feature = "csr")]
    {
        web_sys::window()?.inner_height().ok()?.as_f64().map(|height| height as f32)
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// What this browser session remembers under `key` (`sessionStorage`). `None` on the server.
#[allow(unused_variables)]
pub fn session_get(key: &str) -> Option<String> {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    {
        web_sys::window()?.session_storage().ok()??.get_item(key).ok()?
    }
    #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
    None
}

#[allow(unused_variables)]
pub fn session_set(key: &str, value: &str) {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
        let _ = storage.set_item(key, value);
    }
}

/// What this browser remembers under `key` (`localStorage`, so it outlives the session): a
/// personal view setting or what a visitor keeps (R20), never part of the URL and never part of
/// server HTML (R9). `None` on the server, and in a browser that refuses storage.
#[allow(unused_variables)]
pub fn local_get(key: &str) -> Option<String> {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    {
        web_sys::window()?.local_storage().ok()??.get_item(key).ok()?
    }
    #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
    None
}

/// An empty value takes the key out: a browser that keeps nothing stores nothing.
#[allow(unused_variables)]
pub fn local_set(key: &str, value: &str) {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = if value.is_empty() { storage.remove_item(key) } else { storage.set_item(key, value) };
    }
}

/// The number in the `data-i` of the element an event hit, or of the closest element around it
/// that has one: one handler for many small targets (the dots of the landing page's map).
pub fn index_under(target: Option<leptos::web_sys::EventTarget>) -> Option<usize> {
    use leptos::wasm_bindgen::JsCast;
    let element = target?.dyn_into::<leptos::web_sys::Element>().ok()?;
    element.closest("[data-i]").ok()??.get_attribute("data-i")?.parse().ok()
}

/// The address a click on this element follows: the `href` of the link it is in, as written
/// (`/catalog?…`). `None` outside a link.
pub fn link_under(target: Option<leptos::web_sys::EventTarget>) -> Option<String> {
    use leptos::wasm_bindgen::JsCast;
    let element = target?.dyn_into::<leptos::web_sys::Element>().ok()?;
    element.closest("a[href]").ok()??.get_attribute("href")
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

/// Moves the focus to the first element that matches `selector` (scrolled to only if it is out of
/// view), for a control that took the place of the one the visitor used: without it the focus
/// falls back to the start of the page. `false` if there is none.
#[allow(unused_variables)]
pub fn focus_selector(selector: &str) -> bool {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let found = web_sys::window().and_then(|w| w.document()).and_then(|d| d.query_selector(selector).ok().flatten());
        let Some(element) = found.and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) else { return false };
        element.focus().is_ok()
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

/// The fragment of the address as the browser has it, with its `#`. The router learns of a
/// fragment when a page is opened, not when only the fragment changes. Empty on the server,
/// which never sees a fragment anyway: browsers do not send it.
pub fn fragment() -> String {
    #[cfg(feature = "csr")]
    {
        web_sys::window().and_then(|w| w.location().hash().ok()).unwrap_or_default()
    }
    #[cfg(not(feature = "csr"))]
    String::new()
}
