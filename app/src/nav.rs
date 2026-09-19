//! Browser-only helpers that turn plain forms into client-side navigation. On the server (and
//! before the browser app has taken over) the same forms simply submit.

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
