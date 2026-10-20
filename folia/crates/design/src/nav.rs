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
        let content = crate::stage::by_id(content_id)?.get_bounding_client_rect();
        if is_phone() {
            let height = window.inner_height().ok()?.as_f64()? as f32;
            return Some(((-content.top() as f32).max(0.0), height));
        }
        let area = crate::stage::by_id(scroller_id)?;
        let area_box = area.get_bounding_client_rect();
        let under_heads = heads.last().and_then(|id| crate::stage::by_id(id)).map_or(area_box.top(), |head| head.get_bounding_client_rect().bottom());
        let top = under_heads.max(area_box.top()) as f32;
        let bottom = (area_box.top() + f64::from(area.client_height())) as f32;
        Some(((top - content.top() as f32).max(0.0), (bottom - top).max(0.0)))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// How tall the heads are together; 0 without them (and on a phone, which shows no heads).
#[cfg(feature = "csr")]
fn height_of(ids: &[&str]) -> f32 {
    use wasm_bindgen::JsCast;
    ids.iter()
        .filter_map(|id| crate::stage::by_id(id).and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok()))
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
        } else if let Some(rows) = crate::stage::by_id(rows_id) {
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
        let (Some(area), Some(content)) = (crate::stage::by_id(scroller_id), crate::stage::by_id(content_id)) else { return };
        let start = content.get_bounding_client_rect().top() - area.get_bounding_client_rect().top() + f64::from(area.scroll_top());
        area.set_scroll_top((start + f64::from(offset) - f64::from(height_of(heads))).round() as i32);
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
        } else if let Some(rows) = crate::stage::by_id(rows_id) {
            rows.set_scroll_top(rows.scroll_top() + by.round() as i32);
        }
    }
}

/// The rendered rows of a virtual list (`[data-i]` children of `content_id`) with their heights, to
/// the fraction of a pixel the browser lays them out at: a row of two lines is 85.4 px high on a
/// phone, and taken as 85 every one of them moved the rows below it up by 0.4 px, against the
/// server's page they replace at the takeover (1.2 px by the eighth row).
#[allow(unused_variables)]
pub fn measure_rows(content_id: &str) -> Vec<(usize, f32)> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let Some(content) = crate::stage::by_id(content_id) else { return Vec::new() };
        let Ok(rows) = content.query_selector_all("[data-i]") else { return Vec::new() };
        (0..rows.length())
            .filter_map(|i| rows.item(i).and_then(|node| node.dyn_into::<web_sys::HtmlElement>().ok()))
            .filter_map(|row| Some((row.get_attribute("data-i")?.parse().ok()?, row.get_bounding_client_rect().height() as f32)))
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
        let element = crate::stage::by_id(id)?;
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
        let target: web_sys::EventTarget = crate::stage::by_id(id)?.into();
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
        Some(crate::stage::by_id(id)?.get_bounding_client_rect().width())
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The part of the window a menu fixed to it may take, in CSS pixels: from `left` — past the rail
/// on a wide screen, which is drawn over the page (owner, 2026-10-20: a collection's menu went
/// under it, „Positionier das einfach so, dass es nicht dahinter kommen kann") — to the window's
/// right edge (`width`), from its top to `height`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    pub left: f64,
    pub width: f64,
    pub height: f64,
}

impl Room {
    /// A window `width` × `height` with nothing over its left edge: a phone, the server, a test.
    pub const fn window(width: f64, height: f64) -> Self {
        Room { left: 0.0, width, height }
    }
}

/// The room of the window as it is now: its size, and where the rail ends (the rail is not drawn on
/// a phone). On the server 1280 × 800 without a rail.
pub fn room() -> Room {
    #[cfg(feature = "csr")]
    {
        let Some(window) = web_sys::window() else { return Room::window(1280.0, 800.0) };
        let width = window.inner_width().ok().and_then(|width| width.as_f64()).unwrap_or(1280.0);
        let height = window.inner_height().ok().and_then(|height| height.as_f64()).unwrap_or(800.0);
        let rail = window.document().and_then(|document| document.query_selector("aside.rail").ok().flatten()).map(|rail| rail.get_bounding_client_rect());
        let left = rail.filter(|rect| rect.width() > 0.0).map_or(0.0, |rect| rect.right().clamp(0.0, width));
        Room { left, width, height }
    }
    #[cfg(not(feature = "csr"))]
    Room::window(1280.0, 800.0)
}

/// Where the popup of a picker goes. On a computer it is fixed to the window, so no panel clips
/// it: under its button, or above it when there is more room. On a phone it opens in place, under
/// its button, and spans what the picker stands in (`inset`): a dialog, a sheet, a panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopupPlace {
    pub left: f64,
    pub top: Option<f64>,
    pub bottom: Option<f64>,
    pub width: f64,
    pub max_height: f64,
    /// How far the picker stands in from the left and the right edge of the dialog, the sheet or
    /// the panel it is in; `None` outside of them.
    pub inset: Option<(f64, f64)>,
    /// The picker's button spans its line (nothing beside it, as wide as the picker): on a phone,
    /// open, it spans the width of its list as well, so that the two are one box (`data-wide`).
    /// Where its content begins and ends, from the picker's edges, stays (`--tx`, `--tr`).
    pub wide: Option<(f64, f64)>,
}

impl PopupPlace {
    /// Custom properties for the picker (`.combo`, whose popup has them as well): the popup's
    /// place in the window, which a phone ignores, and how far in what it stands in the picker
    /// stands (`--pl`, `--pr`), which only a phone uses.
    pub fn style(&self) -> String {
        let vertical = match (self.top, self.bottom) {
            (Some(top), _) => format!("--y:{top:.0}px"),
            (None, Some(bottom)) => format!("--b:{bottom:.0}px"),
            (None, None) => String::new(),
        };
        let inset = self.inset.map(|(left, right)| format!("--pl:{left:.1}px;--pr:{right:.1}px;")).unwrap_or_default();
        let content = self.wide.map(|(left, right)| format!("--tx:{left:.1}px;--tr:{right:.1}px;")).unwrap_or_default();
        format!("--x:{:.0}px;--w:{:.0}px;--h:{:.0}px;{inset}{content}{vertical}", self.left, self.width, self.max_height)
    }
}

/// `None` on the server or if the button is not there.
#[allow(unused_variables)]
pub fn popup_place(trigger_id: &str, min_width: f64) -> Option<PopupPlace> {
    #[cfg(feature = "csr")]
    {
        let window = web_sys::window()?;
        let trigger = crate::stage::by_id(trigger_id)?;
        let rect = trigger.get_bounding_client_rect();
        let viewport_width = window.inner_width().ok()?.as_f64()?;
        let viewport_height = window.inner_height().ok()?.as_f64()?;
        let width = rect.width().max(min_width).min(viewport_width - 16.0);
        let left = rect.left().min(viewport_width - width - 8.0).max(8.0);
        let (below, above) = (viewport_height - rect.bottom() - 12.0, rect.top() - 12.0);
        // The popup's box (`.combo`, around the button) in what it stands in: a dialog (a sheet on
        // a phone), else its panel (the filters' sheet is one).
        let combo = trigger.closest(".combo").ok().flatten();
        let inset = combo.clone().zip(trigger.closest("dialog, .panel").ok().flatten()).map(|(combo, surface)| {
            let (combo, surface) = (combo.get_bounding_client_rect(), surface.get_bounding_client_rect());
            (combo.left() - surface.left(), surface.right() - combo.right())
        });
        // A button that spans its line: nothing beside it, its picker across what it stands in
        // but for the room at the edges (a card's padding and the dialog's, at most 40 px each),
        // and the button as wide as its picker (a card's row's reaches out past it, an open one
        // past its list's edges: no narrower is what counts). One among the words of a line (a
        // step's) keeps its size.
        let spans = inset.is_some_and(|(left, right)| left <= 40.0 && right <= 40.0);
        let wide = combo.filter(|combo| spans && alone_on_its_line(combo)).and_then(|combo| {
            let picker = combo.get_bounding_client_rect();
            if rect.width() < picker.width() - 12.0 {
                return None;
            }
            let style = window.get_computed_style(&trigger).ok().flatten()?;
            let padding = |side: &str| style.get_property_value(side).ok()?.trim_end_matches("px").parse::<f64>().ok();
            Some((rect.left() + padding("padding-left")? - picker.left(), picker.right() - rect.right() + padding("padding-right")?))
        });
        Some(if below >= 300.0 || below >= above {
            PopupPlace { left, top: Some(rect.bottom() + 4.0), bottom: None, width, max_height: below.clamp(160.0, 460.0), inset, wide }
        } else {
            PopupPlace { left, top: None, bottom: Some(viewport_height - rect.top() + 4.0), width, max_height: above.clamp(160.0, 460.0), inset, wide }
        })
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Nothing of the element's parent stands beside it: whatever else it holds is above or below it
/// (a picker's label over it on a phone, a line under it), or not shown.
#[cfg(feature = "csr")]
fn alone_on_its_line(element: &web_sys::Element) -> bool {
    let Some(parent) = element.parent_element() else { return true };
    let rect = element.get_bounding_client_rect();
    let siblings = parent.children();
    (0..siblings.length()).filter_map(|index| siblings.item(index)).filter(|sibling| sibling != element).all(|sibling| {
        let other = sibling.get_bounding_client_rect();
        other.width() == 0.0 || other.height() == 0.0 || other.bottom() <= rect.top() + 1.0 || other.top() >= rect.bottom() - 1.0
    })
}

/// Moves the focus without scrolling anything; text fields get their content selected.
#[allow(unused_variables)]
pub fn focus_by_id(id: &str) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let element = crate::stage::by_id(id);
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
        let entry = crate::stage::by_id(entry_id);
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
    if let Some(filters) = crate::stage::by_id("filters") {
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

/// The server's page still stands in front of the app as a picture (`assets/boot.js`,
/// `takePicture`): the app has just taken over, and nobody sees what it builds. `false` on the
/// server.
pub fn behind_picture() -> bool {
    #[cfg(feature = "csr")]
    {
        web_sys::window().and_then(|w| w.document()).and_then(|d| d.query_selector("body.takeover-picture").ok().flatten()).is_some()
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

/// An empty value takes the key out, as `local_set`'s does.
#[allow(unused_variables)]
pub fn session_set(key: &str, value: &str) {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
        let _ = if value.is_empty() { storage.remove_item(key) } else { storage.set_item(key, value) };
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
        let found = crate::stage::select(selector);
        let Some(element) = found else { return false };
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Center);
        element.scroll_into_view_with_scroll_into_view_options(&options);
        true
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// Clicks what `selector` finds on the page in front (an entry of a menu whose key was pressed).
/// `false` without it, and on the server.
#[allow(unused_variables)]
pub fn click_selector(selector: &str) -> bool {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let Some(element) = crate::stage::select(selector).and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok()) else { return false };
        element.click();
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
        let found = crate::stage::select(selector);
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
        let rows = crate::stage::by_id(rows_id);
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
