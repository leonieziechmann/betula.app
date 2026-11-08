//! „Gerät verbinden" (`/sync`, Betula Sync): the page the link of another device opens
//! (`folia_widgets::syncpair::PairLink`). What follows `#` in the link opens a sealed parcel with the profile's key; a
//! browser never sends that part to a server, and the page takes it out of the address and the
//! history as soon as it has read it, before anything else happens. Then the profile comes as a new
//! one of this browser (`Sync::join`), becomes the active one, and the page says so, with the way
//! to the Stundenplan and the profile; or why it did not, with what to do.
//!
//! The page exists in the browser app only: the server writes the same explanation for everybody
//! (R9), `noindex`, and no page links it.

use leptos::prelude::*;

use folia_design::stage::Title;
use folia_design::ui::Icon;
use folia_routes::url;
use folia_shell::frame::Plain;
use folia_shell::pending::Shape;
use folia_shell::seo::Seo;
use folia_shell::skeleton::AppStandin;
use folia_stores::profiles::Profiles;
use folia_stores::sync::{Problem, Sync};

use crate::i18n;
use crate::sync::problem_text;

/// Whether this build is the browser app.
const APP: bool = cfg!(feature = "csr");

/// Where the page is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    /// The link read, the profile on its way.
    Joining,
    /// Here, under this name.
    Joined(String),
    /// The site does not offer Betula Sync.
    Off,
    /// A link without its part after `#`.
    NoLink,
    Failed(Problem),
}

#[component]
fn ConnectSeo() -> impl IntoView {
    let t = i18n::t();
    view! { <Seo title=t.sync.connect_title description=t.sync.connect_description path=url::CONNECT noindex=true/> }
}

#[component]
pub fn ConnectPage() -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    if !APP {
        return view! {
            <Title text=s.connect_title/>
            <ConnectSeo/>
            <AppStandin shape=Shape::Text title=s.connect_server_title hint=s.connect_server_hint/>
        }
        .into_any();
    }
    // Read once, as the page is made: the fragment goes out of the address at once.
    let fragment = take_fragment();
    let sync = Sync::expect().filter(|_| folia_stores::sync::offered().is_some());
    let step = RwSignal::new(match (&sync, fragment.as_deref()) {
        (None, _) => Step::Off,
        (Some(_), None) => Step::NoLink,
        (Some(_), Some(_)) => Step::Joining,
    });
    if let (Some(sync), Some(fragment)) = (sync, fragment) {
        let profiles = Profiles::expect();
        sync.join(&fragment, move |done| {
            let next = match done {
                Ok(id) => {
                    let name = profiles.map(|profiles| {
                        profiles.switch(id);
                        untrack(|| profiles.all()).into_iter().find(|profile| profile.id == id).map(|profile| profile.shown(t.locale)).unwrap_or_default()
                    });
                    Step::Joined(name.unwrap_or_default())
                }
                Err(problem) => Step::Failed(problem),
            };
            step.try_set(next);
        });
    }
    let ways = move |first: (&'static str, &'static str, &'static str)| {
        view! {
            <nav class="pf-ways">
                <a class="btn primary" href=t.path(first.0) rel="nofollow"><Icon name=first.1/><span>{first.2}</span></a>
                <a class="btn secondary" href=t.path(url::PROFILE) rel="nofollow"><Icon name="user-round"/><span>{s.to_profile}</span></a>
            </nav>
        }
    };
    let body = move || match step.get() {
        Step::Joining => view! {
            <p class="pf-sync-state" role="status" data-state="wait"><span class="pf-sync-dot" aria-hidden="true"></span><span>{s.connecting}</span></p>
        }
        .into_any(),
        Step::Joined(name) => view! {
            <p class="pf-sync-state" role="status" data-state="on"><span class="pf-sync-dot" aria-hidden="true"></span><span>{(s.connected)(&name)}</span></p>
            <p class="pf-empty">{s.connected_hint}</p>
            {ways((url::STUDYPLAN, "calendar-range", s.to_studyplan))}
        }
        .into_any(),
        Step::Off => view! {
            <p class="pf-empty">{s.connect_off}</p>
            {ways((url::HOME, "house", s.to_start))}
        }
        .into_any(),
        Step::NoLink => view! {
            <p class="pf-empty">{s.connect_none}</p>
            {ways((url::HOME, "house", s.to_start))}
        }
        .into_any(),
        Step::Failed(problem) => view! {
            <p class="action note-action pf-refused" role="status"><Icon name="triangle-alert"/><span>{problem_text(problem)}</span></p>
            {ways((url::HOME, "house", s.to_start))}
        }
        .into_any(),
    };
    view! {
        <Title text=s.connect_title/>
        <Plain>
            <div class="page-inner pf pf-connect">
                <ConnectSeo/>
                <section class="panel pf-part" aria-labelledby="pf-connect">
                    <h2 id="pf-connect"><Icon name="monitor-smartphone"/>{s.connect_title}</h2>
                    {body}
                </section>
            </div>
        </Plain>
    }
    .into_any()
}

/// The part of the address after `#`, taken out of the address and of the history (the entry is
/// replaced by the same address without it); `None` where there is none. Only in the browser.
fn take_fragment() -> Option<String> {
    #[cfg(feature = "csr")]
    {
        let window = web_sys::window()?;
        let location = window.location();
        let hash = location.hash().ok()?;
        let fragment = hash.strip_prefix('#').unwrap_or(&hash).trim().to_string();
        if !hash.is_empty() {
            let rest = format!("{}{}", location.pathname().unwrap_or_default(), location.search().unwrap_or_default());
            if let Ok(history) = window.history() {
                let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&rest));
            }
        }
        Some(fragment).filter(|fragment| !fragment.is_empty())
    }
    #[cfg(not(feature = "csr"))]
    None
}
