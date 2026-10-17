//! The screen's keyboard of a phone or a tablet (owner, 2026-10-17: „dann muss das Fenster auch
//! hoch gehen, wenn das virtual keyboard rein kommt, so dass man noch sehen kann was man macht.
//! Also das modal kann ruhig full screen werden, wenn man das Keyboard öffnet. (Aber nur bei
//! onscreen keyboard)").
//!
//! The keyboard covers the lower part of the screen without telling the page. A browser shrinks
//! the part of the page it shows (`window.visualViewport`) and leaves the page as it is (Chrome on
//! Android, Safari), or shrinks the page with it (Firefox on Android). So the keyboard counts as up
//! while a text field has the focus and the part shown is markedly shorter than the page was at
//! this width: never on a computer, whose window made lower is lower all through, nor with a
//! keyboard of wire or radio, which covers nothing.
//!
//! While it is up, the root carries `data-keyboard` and where the part shown is (`--vv-top`,
//! `--vv-height`), and app.css makes a dialog or a sheet on a phone that part, from edge to edge,
//! without its foot. What is typed in goes to the top of what scrolls it, so that what follows it
//! shows: a picker, whose list ends above the keyboard (`--kb-room`), or any other text field
//! (owner, the same day: „Aber das kann ruhig bei jeder combobox/Textfeld passieren"), but one in
//! what stays where it is (the page's bar of the search, a dialog's bar). Once the keyboard has
//! gone, what scrolled is back where it was.

/// Watches for the keyboard, once for the life of the app; nothing on the server.
pub fn watch() {
    #[cfg(feature = "csr")]
    browser::watch();
}

#[cfg(feature = "csr")]
mod browser {
    use std::cell::RefCell;
    use std::rc::Rc;

    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{CssStyleDeclaration, Element, HtmlElement, VisualViewport, Window};

    /// How much shorter than the page the part shown has to be: a toolbar of the browser coming and
    /// going, or the bar of shortcuts a tablet shows over a keyboard of its own, stays under it.
    const COVERS: f64 = 120.0;
    /// Between a picker scrolled up and the top of what shows it, and between its list and the
    /// keyboard.
    const MARGIN: f64 = 8.0;

    /// What scrolls a field: a box (a dialog's body, a sheet's) or the page.
    enum Scroller {
        Page,
        Box(Element),
    }

    impl Scroller {
        /// The nearest box around `element` that scrolls (its content taller than it, its
        /// `overflow-y` letting it scroll), else the page.
        fn of(window: &Window, element: &Element) -> Self {
            let mut at = element.parent_element();
            while let Some(element) = at {
                if matches!(element.tag_name().as_str(), "BODY" | "HTML") {
                    break;
                }
                let scrolls = window.get_computed_style(&element).ok().flatten().and_then(|style| style.get_property_value("overflow-y").ok()).is_some_and(|value| matches!(value.as_str(), "auto" | "scroll"));
                if scrolls && element.scroll_height() > element.client_height() {
                    return Self::Box(element);
                }
                at = element.parent_element();
            }
            Self::Page
        }

        fn top(&self, window: &Window) -> f64 {
            match self {
                Self::Page => window.scroll_y().unwrap_or(0.0),
                Self::Box(element) => f64::from(element.scroll_top()),
            }
        }

        fn scroll_to(&self, window: &Window, top: f64) {
            match self {
                Self::Page => window.scroll_to_with_x_and_y(window.scroll_x().unwrap_or(0.0), top),
                Self::Box(element) => element.set_scroll_top(top.round() as i32),
            }
        }

        /// Where a field scrolled up comes to stand: the top of the box, or of the part of the
        /// page shown, below what stays at its top (a phone's bar of the search) above `x`.
        fn view_top(&self, window: &Window, viewport: &VisualViewport, x: f64) -> f64 {
            match self {
                Self::Box(element) => element.get_bounding_client_rect().top(),
                Self::Page => {
                    let top = viewport.offset_top();
                    let mut at = window.document().and_then(|document| document.element_from_point(x as f32, (top + 1.0) as f32));
                    while let Some(element) = at {
                        if stays(window, &element) {
                            return top.max(element.get_bounding_client_rect().bottom());
                        }
                        at = element.parent_element();
                    }
                    top
                }
            }
        }

        /// `element`, or a box it is in short of this, stays where it is as this scrolls (the
        /// page's bar of the search, a dialog's bar over its body).
        fn passes_over(&self, window: &Window, element: &Element) -> bool {
            let mut at = Some(element.clone());
            while let Some(element) = at {
                let end = match self {
                    Self::Box(scroller) => scroller == &element,
                    Self::Page => matches!(element.tag_name().as_str(), "BODY" | "HTML"),
                };
                if end {
                    return false;
                }
                if stays(window, &element) {
                    return true;
                }
                at = element.parent_element();
            }
            false
        }
    }

    /// Fixed to the window or sticking to the top of what scrolls.
    fn stays(window: &Window, element: &Element) -> bool {
        let position = window.get_computed_style(element).ok().flatten().and_then(|style| style.get_property_value("position").ok()).unwrap_or_default();
        matches!(position.as_str(), "sticky" | "fixed")
    }

    #[derive(Default)]
    struct State {
        up: bool,
        /// The page's width, and the tallest the page was at it where a finger points (there the
        /// keyboard may shrink the page as well).
        tall: (f64, f64),
        /// What was scrolled up for the keyboard (a picker, a field), what scrolled it and where
        /// that stood before.
        scrolled: Option<(Element, Scroller, f64)>,
        /// How tall the part shown was when it was scrolled up.
        scrolled_for: f64,
    }

    /// A field the screen's keyboard comes up for.
    fn takes_text(element: &Element) -> bool {
        if element.has_attribute("readonly") || element.has_attribute("disabled") {
            return false;
        }
        match element.tag_name().as_str() {
            "TEXTAREA" => true,
            "INPUT" => {
                let kind = element.get_attribute("type").unwrap_or_default().to_ascii_lowercase();
                !matches!(kind.as_str(), "checkbox" | "radio" | "button" | "submit" | "reset" | "range" | "color" | "file" | "image" | "hidden")
            }
            _ => element.dyn_ref::<HtmlElement>().is_some_and(HtmlElement::is_content_editable),
        }
    }

    fn number(value: Result<JsValue, JsValue>) -> f64 {
        value.ok().and_then(|value| value.as_f64()).unwrap_or(0.0)
    }

    fn update(state: &RefCell<State>) {
        let Some(window) = web_sys::window() else { return };
        let (Some(document), Some(viewport)) = (window.document(), window.visual_viewport()) else { return };
        let Some(root) = document.document_element() else { return };
        let Some(style) = root.dyn_ref::<HtmlElement>().map(HtmlElement::style) else { return };
        let width = number(window.inner_width());
        let page = number(window.inner_height()).max(f64::from(root.client_height()));
        let shown = viewport.height() * viewport.scale();
        let focused = document.active_element().filter(takes_text);
        let finger = window.match_media("(pointer: coarse)").ok().flatten().is_some_and(|query| query.matches());
        let mut state = state.borrow_mut();
        if (state.tall.0 - width).abs() > 0.5 {
            state.tall = (width, 0.0);
        }
        state.tall.1 = state.tall.1.max(page);
        let page = if finger { state.tall.1 } else { page };
        let up = focused.is_some() && page - shown > COVERS;
        if up {
            let _ = root.set_attribute("data-keyboard", "");
            let _ = style.set_property("--vv-top", &format!("{:.1}px", viewport.offset_top()));
            let _ = style.set_property("--vv-height", &format!("{:.1}px", viewport.height()));
        } else if state.up {
            let _ = root.remove_attribute("data-keyboard");
            for name in ["--vv-top", "--vv-height", "--kb-room"] {
                let _ = style.remove_property(name);
            }
        }
        let was = std::mem::replace(&mut state.up, up);
        match focused {
            Some(field) if up => reveal(&mut state, &window, &viewport, &style, &field),
            _ if was && !up => {
                if let Some((_, scroller, top)) = state.scrolled.take() {
                    scroller.scroll_to(&window, top);
                }
            }
            _ => {}
        }
    }

    /// What `field` types in at the top of what scrolls it, so that what follows it shows: the
    /// picker it searches, its list down to the keyboard, else the field (with the box it stands
    /// in, a search's), each with its name over it. Not one in what stays where it is.
    fn reveal(state: &mut State, window: &Window, viewport: &VisualViewport, style: &CssStyleDeclaration, field: &Element) {
        let picker = field.closest(".combo[data-open]").ok().flatten();
        let unit = picker.clone().or_else(|| field.closest("label").ok().flatten()).unwrap_or_else(|| field.clone());
        // Scrolled up once for the keyboard (again as it grows or shrinks), so that what the
        // finger scrolls meanwhile stays where it went.
        let fresh = !state.scrolled.as_ref().is_some_and(|(scrolled, ..)| scrolled == &unit);
        let scroll = fresh || (viewport.height() - state.scrolled_for).abs() > 1.0;
        state.scrolled_for = viewport.height();
        if fresh {
            // Another one scrolled up before: what scrolled it back first.
            if let Some((_, scroller, top)) = state.scrolled.take() {
                scroller.scroll_to(window, top);
            }
            let scroller = Scroller::of(window, &unit);
            if scroller.passes_over(window, &unit) {
                return;
            }
            let before = scroller.top(window);
            state.scrolled = Some((unit.clone(), scroller, before));
        }
        let Some((_, scroller, _)) = state.scrolled.as_ref() else { return };
        // Its name over it goes up with it (a card's row, a group of filters).
        let head = unit.previous_element_sibling().filter(|label| {
            label.matches("label, .label, .flabel, legend, h2, h3, h4").unwrap_or(false) && label.get_bounding_client_rect().bottom() <= unit.get_bounding_client_rect().top() + 1.0
        });
        let top = head.as_ref().unwrap_or(&unit);
        let x = {
            let rect = unit.get_bounding_client_rect();
            (rect.left() + rect.right()) / 2.0
        };
        let popup = picker.and_then(|picker| picker.query_selector(".combo-pop").ok().flatten());
        // As far up as what scrolls it goes, and a picker's list down to the keyboard; a shorter
        // list may let it scroll less far, so the two are measured again until they hold.
        let bottom = viewport.offset_top() + viewport.height() - MARGIN;
        for _ in 0..3 {
            if scroll {
                let by = top.get_bounding_client_rect().top() - scroller.view_top(window, viewport, x) - MARGIN;
                scroller.scroll_to(window, scroller.top(window) + by);
            }
            let Some(popup) = popup.as_ref() else { break };
            let room = bottom - popup.get_bounding_client_rect().top();
            let _ = style.set_property("--kb-room", &format!("{:.0}px", room.floor()));
            if popup.get_bounding_client_rect().bottom() <= bottom + 0.5 {
                break;
            }
        }
    }

    pub(super) fn watch() {
        let Some(window) = web_sys::window() else { return };
        let (Some(document), Some(viewport)) = (window.document(), window.visual_viewport()) else { return };
        let state = Rc::new(RefCell::new(State::default()));
        let now = {
            let state = state.clone();
            Closure::<dyn Fn()>::new(move || update(&state))
        };
        // The focus moves from one field to the next through the page itself: what counts is
        // where it has come to, a moment later.
        let soon = {
            let state = state.clone();
            Closure::<dyn Fn()>::new(move || {
                let state = state.clone();
                leptos::prelude::set_timeout(move || update(&state), std::time::Duration::ZERO);
            })
        };
        let listen = |target: &web_sys::EventTarget, event: &str, handler: &Closure<dyn Fn()>| {
            let _ = target.add_event_listener_with_callback(event, handler.as_ref().unchecked_ref());
        };
        listen(&viewport, "resize", &now);
        listen(&viewport, "scroll", &now);
        listen(&window, "resize", &now);
        listen(&document, "focusin", &soon);
        listen(&document, "focusout", &soon);
        update(&state);
        // For the life of the app.
        now.forget();
        soon.forget();
    }
}
