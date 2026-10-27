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
//!
//! Nothing of it jumps (owner, the same day, after a try on her phone: „Animation bitte. Aktuell
//! springt das noch komisch"): the dialog or the sheet glides into the part shown and back, by its
//! top and its height (`Element.animate`, from where it stood to where its new place is; a glide
//! under way goes on from where it is), what is typed in scrolls up smoothly and back. With less
//! motion asked for, all of it is there at once.

/// Watches for the keyboard, once for the life of the app; nothing on the server.
pub fn watch() {
    #[cfg(feature = "csr")]
    browser::watch();
}

#[cfg(feature = "csr")]
mod browser {
    use std::cell::RefCell;
    use std::rc::Rc;

    use js_sys::{Array, Function, Object, Reflect};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{CssStyleDeclaration, Element, HtmlElement, ScrollBehavior, ScrollToOptions, VisualViewport, Window};

    /// How much shorter than the page the part shown has to be: a toolbar of the browser coming and
    /// going, or the bar of shortcuts a tablet shows over a keyboard of its own, stays under it.
    const COVERS: f64 = 120.0;
    /// Between a picker scrolled up and the top of what shows it, and between its list and the
    /// keyboard.
    const MARGIN: f64 = 8.0;
    /// What takes the part of the screen the keyboard leaves on a phone (app.css): a dialog, a sheet.
    const SURFACES: &str = ".st-dialog[open], .filters.open, .filters:target, .sidebar.sheet.open, .sidebar.sheet:target";
    /// How long a dialog or a sheet glides, and how: quick, and slowing into its place (`--ease`).
    const GLIDE_MS: f64 = 280.0;
    const EASE: &str = "cubic-bezier(.2, .7, .2, 1)";
    /// The name of the glides, so that a new one stops the one under way.
    const GLIDE: &str = "keyboard-glide";
    /// How long what scrolled keeps its room below once the keyboard has gone: until it has
    /// scrolled back (else it would jump back to the end of what it holds then).
    const LEAVING_MS: u64 = 900;

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

        /// As far down as it scrolls.
        fn end(&self, window: &Window) -> f64 {
            match self {
                Self::Page => window.document().and_then(|document| document.document_element()).map_or(0.0, |root| f64::from(root.scroll_height() - root.client_height())),
                Self::Box(element) => f64::from(element.scroll_height() - element.client_height()),
            }
        }

        /// To `top`, smoothly but where less motion is asked for.
        fn scroll_to(&self, window: &Window, top: f64) {
            let options = ScrollToOptions::new();
            options.set_top(top.round());
            options.set_behavior(if calm(window) { ScrollBehavior::Instant } else { ScrollBehavior::Smooth });
            match self {
                Self::Page => window.scroll_to_with_scroll_to_options(&options),
                Self::Box(element) => element.scroll_to_with_scroll_to_options(&options),
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

    /// Less motion is asked for.
    fn calm(window: &Window) -> bool {
        window.match_media("(prefers-reduced-motion: reduce)").ok().flatten().is_some_and(|query| query.matches())
    }

    /// Where a dialog or a sheet stands: its top and height, and the rounding of its upper corners.
    struct Spot {
        top: f64,
        height: f64,
        corner: String,
    }

    impl Spot {
        /// Where its box stands, less how far its transform moves it: a sheet of a phone half up
        /// is moved down (enhance.js „the sheets of a phone"), and glides there on its own.
        fn of(window: &Window, element: &Element) -> Self {
            let rect = element.get_bounding_client_rect();
            let style = window.get_computed_style(element).ok().flatten();
            let corner = style.as_ref().and_then(|style| style.get_property_value("border-top-left-radius").ok()).unwrap_or_default();
            let moved = style.and_then(|style| style.get_property_value("transform").ok()).map_or(0.0, |transform| moved_down(&transform));
            Self { top: rect.top() - moved, height: rect.height(), corner }
        }

        /// A keyframe of it: placed by its top alone, whatever placed it before.
        fn frame(&self) -> Object {
            let frame = Object::new();
            let corner = if self.corner.is_empty() { "0px" } else { self.corner.as_str() };
            for (key, value) in [
                ("top", format!("{:.1}px", self.top)),
                ("height", format!("{:.1}px", self.height)),
                ("bottom", "auto".to_string()),
                ("marginTop", "0px".to_string()),
                ("maxHeight", "none".to_string()),
                ("borderTopLeftRadius", corner.to_string()),
                ("borderTopRightRadius", corner.to_string()),
            ] {
                let _ = Reflect::set(&frame, &key.into(), &value.into());
            }
            frame
        }
    }

    /// How far down a computed transform moves a box (`matrix(…)`'s sixth value, `matrix3d(…)`'s
    /// fourteenth); none for `none`.
    fn moved_down(transform: &str) -> f64 {
        let Some((kind, rest)) = transform.split_once('(') else { return 0.0 };
        let values: Vec<f64> = rest.trim_end_matches(')').split(',').filter_map(|value| value.trim().parse().ok()).collect();
        let at = if kind.trim() == "matrix3d" { 13 } else { 5 };
        values.get(at).copied().unwrap_or(0.0)
    }

    /// Calls the method `name` of `target` with `args`, if it has one.
    fn call(target: &JsValue, name: &str, args: &[&JsValue]) -> Option<JsValue> {
        let method = Reflect::get(target, &name.into()).ok()?.dyn_into::<Function>().ok()?;
        match args {
            [] => method.call0(target),
            [first] => method.call1(target, first),
            [first, second, ..] => method.call2(target, first, second),
        }
        .ok()
    }

    /// The glide of `element` under way stops (where it is, the element where its style puts it).
    fn stop_glide(element: &Element) {
        let Some(animations) = call(element, "getAnimations", &[]) else { return };
        for animation in Array::from(&animations).iter() {
            if Reflect::get(&animation, &"id".into()).ok().and_then(|id| id.as_string()).as_deref() == Some(GLIDE) {
                call(&animation, "cancel", &[]);
            }
        }
    }

    /// `element` glides from `from` to where its style puts it now.
    fn glide(window: &Window, element: &Element, from: &Spot) {
        let to = Spot::of(window, element);
        if calm(window) || ((from.top - to.top).abs() < 1.0 && (from.height - to.height).abs() < 1.0) {
            return;
        }
        let keyframes = Array::of2(&from.frame(), &to.frame());
        let options = Object::new();
        let _ = Reflect::set(&options, &"duration".into(), &GLIDE_MS.into());
        let _ = Reflect::set(&options, &"easing".into(), &EASE.into());
        let _ = Reflect::set(&options, &"id".into(), &GLIDE.into());
        call(element, "animate", &[&keyframes, &options]);
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
        /// Where it is scrolling to.
        scrolling_to: f64,
        /// How tall the part shown was when the dialogs and sheets were placed in it.
        placed_for: f64,
        /// Counts the keyboard's goings: the room below what scrolled stays until it has scrolled
        /// back, unless the keyboard is up again by then.
        gone: u32,
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

    fn update(shared: &Rc<RefCell<State>>) {
        let state = shared.as_ref();
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
        // A dialog or a sheet glides to its new place: as the keyboard comes or goes, or grows or
        // shrinks; where it stands now is where it glides from (a glide under way stops there).
        let moves = crate::nav::is_phone() && (up != state.up || (up && (viewport.height() - state.placed_for).abs() > 1.0));
        let glides: Vec<(Element, Spot)> = if moves {
            // The open surfaces of the stage in front, and those around the stages.
            crate::stage::select_all(SURFACES)
                .into_iter()
                .map(|surface| {
                    let spot = Spot::of(&window, &surface);
                    stop_glide(&surface);
                    (surface, spot)
                })
                .collect()
        } else {
            Vec::new()
        };
        state.placed_for = if up { viewport.height() } else { 0.0 };
        if up {
            let _ = root.set_attribute("data-keyboard", "");
            let _ = root.remove_attribute("data-keyboard-leaving");
            let _ = style.set_property("--vv-top", &format!("{:.1}px", viewport.offset_top()));
            let _ = style.set_property("--vv-height", &format!("{:.1}px", viewport.height()));
            let _ = style.set_property("--kb-pad", &format!("{:.1}px", viewport.height()));
        } else if state.up {
            let _ = root.remove_attribute("data-keyboard");
            for name in ["--vv-top", "--vv-height", "--kb-room"] {
                let _ = style.remove_property(name);
            }
            // The room below what scrolled (`--kb-pad`) stays while it scrolls back.
            let _ = root.set_attribute("data-keyboard-leaving", "");
            state.gone = state.gone.wrapping_add(1);
            let (shared, gone) = (shared.clone(), state.gone);
            leptos::prelude::set_timeout(
                move || {
                    let still = shared.try_borrow().is_ok_and(|state| state.gone == gone && !state.up);
                    let root = web_sys::window().and_then(|window| window.document()).and_then(|document| document.document_element());
                    if let (true, Some(root)) = (still, root) {
                        let _ = root.remove_attribute("data-keyboard-leaving");
                        if let Some(style) = root.dyn_ref::<HtmlElement>().map(HtmlElement::style) {
                            let _ = style.remove_property("--kb-pad");
                        }
                    }
                },
                std::time::Duration::from_millis(LEAVING_MS),
            );
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
        for (surface, from) in &glides {
            glide(&window, surface, from);
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
        // As far up as what scrolls it goes (while the keyboard is up, as far as it has to:
        // app.css gives it the room), smoothly; measured from where it is now, so that a scroll
        // under way goes on to there.
        let now = scroller.top(window);
        if scroll {
            let by = top.get_bounding_client_rect().top() - scroller.view_top(window, viewport, x) - MARGIN;
            state.scrolling_to = (now + by).clamp(0.0, scroller.end(window).max(0.0));
            scroller.scroll_to(window, state.scrolling_to);
        }
        // A picker's list down to the keyboard, from where it comes to stand.
        if let Some(popup) = picker.and_then(|picker| picker.query_selector(".combo-pop").ok().flatten()) {
            let bottom = viewport.offset_top() + viewport.height() - MARGIN;
            let room = bottom - (popup.get_bounding_client_rect().top() - (state.scrolling_to - now));
            let _ = style.set_property("--kb-room", &format!("{:.0}px", room.floor()));
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
