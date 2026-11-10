//! „Gerät verbinden" (`/sync`, Betula Sync): where devices of a profile find each other.
//!
//! - **The scanner** (no `#`; folia/design/profil/konzepte.html, the second step): the camera reads
//!   the code another device shows (`scan::Scanner`). A code that asks for a profile leads to the
//!   question below, a code that gives one joins it, anything else is named and the camera goes on.
//!   Beside it the recovery code and „Hier einen Code zeigen" (the dialog of the profile page, this
//!   profile to another device). A phone's way here is „Gerät verbinden" in the menu of the profiles
//!   and on its profile page.
//! - **A code that asks**, read by the scanner (`/pair#…`, whose page asks the same where the camera
//!   app opens it, `pair`): which profile goes, the digits, „Ja, … verbinden"
//!   (`give::GiveRequest`), in place of the scanner. Opened from the start page's setup
//!   (`/sync?from=setup`, its last step on a phone), the way then leads back there.
//! - **A link of another device** (`#<id ‖ key>`, `folia_widgets::syncpair::PairLink`): the profile
//!   comes as a new one of this browser (`Sync::join`), becomes the active one, and the page says
//!   so, with the way to the Stundenplan and the profile; or why it did not, with what to do.
//!
//! What follows `#` never reaches a server, and the page takes it out of the address and the
//! history as soon as it has read it, before anything else happens. The page exists in the browser
//! app only: the server writes the same explanation for everybody (R9), `noindex`.

use leptos::prelude::*;

use folia_design::stage::Title;
use folia_design::ui::Icon;
use folia_routes::url;
use folia_shell::frame::Plain;
use folia_shell::pending::Shape;
use folia_shell::seo::Seo;
use folia_shell::skeleton::AppStandin;
use folia_stores::profiles::{Profiles, FIRST};
use folia_stores::sync::request::{self, Scanned};
use folia_stores::sync::{Problem, State, Sync};
use folia_widgets::syncdone::{ways, ConnectProgress, Connected};

use crate::give::GiveRequest;
use crate::i18n;
use crate::scan::Scanner;
use crate::sync::{problem_text, Connect, Direction, FromOther};

/// Whether this build is the browser app.
const APP: bool = cfg!(feature = "csr");

/// Where the page is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    /// The camera reads codes.
    Scan,
    /// A code that asks for a profile: what followed `#`.
    Give(String),
    /// A link read, the profile on its way.
    Joining,
    /// Here, under this name.
    Joined(String),
    /// The site does not offer Betula Sync.
    Off,
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
    let c = &t.scan;
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
    // Opened from the start page's setup (`?from=setup`): an answered code leads back there.
    let to_setup = from_setup_query();
    let sync = Sync::expect().filter(|_| folia_stores::sync::offered().is_some());
    let profiles = Profiles::expect();
    let step = RwSignal::new(match (&sync, fragment.as_deref()) {
        (None, _) => Step::Off,
        (Some(_), None) => Step::Scan,
        (Some(_), Some(_)) => Step::Joining,
    });
    let join = move |fragment: String| {
        let Some(sync) = sync else { return };
        step.set(Step::Joining);
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
    };
    if let Some(fragment) = fragment.filter(|_| step.get_untracked() == Step::Joining) {
        join(fragment);
    }
    // What the scanner read and is no code to connect with, for a moment under the camera.
    let note = RwSignal::new(None::<&'static str>);
    let read = Callback::new(move |text: String| match request::scanned(&text, &origin()) {
        Scanned::Request(fragment) => {
            note.set(None);
            step.set(Step::Give(fragment));
        }
        Scanned::Join(fragment) => {
            note.set(None);
            join(fragment);
        }
        Scanned::OtherSite => note.set(Some(c.scan_other_site)),
        Scanned::Other => note.set(Some(c.scan_other)),
    });
    let ways = move |first: (&'static str, &'static str, &'static str)| {
        view! {
            <nav class="pf-ways">
                <a class="btn primary" href=t.path(first.0) rel="nofollow"><Icon name=first.1/><span>{first.2}</span></a>
                <a class="btn secondary" href=t.path(url::PROFILE) rel="nofollow"><Icon name="user-round"/><span>{s.to_profile}</span></a>
            </nav>
        }
    };
    let part = |icon: &'static str, title: &'static str, body: AnyView| {
        view! {
            <section class="panel pf-part" aria-labelledby="pf-connect">
                <h2 id="pf-connect"><Icon name=icon/>{title}</h2>
                {body}
            </section>
        }
        .into_any()
    };
    // Joining shows the way on a bar until the profile is here and in line, then the success
    // (concept A, `folia_widgets::syncdone`): one part for both steps, so that the bar goes on.
    let joining = Memo::new(move |_| matches!(step.get(), Step::Joining | Step::Joined(_)));
    let slow = RwSignal::new(false);
    let shown_done = RwSignal::new(false);
    Effect::new(move |_| {
        if matches!(step.get(), Step::Joined(_)) {
            set_timeout(move || { slow.try_set(true); }, std::time::Duration::from_millis(crate::sync::SYNC_WAIT_MS));
        }
    });
    // In line once this device is online with the profile, its records at Phloem and here: never
    // before (a bar that ended without them said „Verbunden" for a profile that was not there).
    let online = Signal::derive(move || profiles.is_some_and(|profiles| matches!(sync.map(|sync| sync.state(profiles.active())), Some(State::Online { .. }))));
    let reached = Signal::derive(move || match step.get() {
        Step::Joined(_) if online.get() => 2,
        Step::Joined(_) => 1,
        _ => 0,
    });
    let join_part = move || {
        let inner = move || {
            if !shown_done.get() {
                return view! {
                    <p class="pf-sub">{s.connecting}</p>
                    <ConnectProgress phases=ways::getting() reached on_done=Callback::new(move |()| shown_done.set(true))/>
                    {move || (slow.get() && !online.get()).then(|| view! { <p class="pf-sub pf-still" role="status">{s.still_connecting}</p> })}
                }
                .into_any();
            }
            let name = match step.get_untracked() {
                Step::Joined(name) => name,
                _ => String::new(),
            };
            // Done: the start page, with the profile that came (owner, 2026-11-09: „dass man wieder
            // bei der main page landet, wenn das sync fertig ist und nicht im profil").
            view! {
                <Connected title=t.syncpair.connected said=(s.connected)(&name)>
                    <a class="btn primary pf-done" href=t.path(url::HOME) rel="nofollow"><Icon name="check"/><span>{s.done}</span></a>
                </Connected>
            }
            .into_any()
        };
        part("monitor-smartphone", s.connect_title, view! { {inner} }.into_any())
    };
    let body = move || if joining.get() { join_part() } else { match step.get() {
        Step::Joining | Step::Joined(_) => ().into_any(),
        Step::Scan => view! { <ScanPart sync note read/> }.into_any(),
        Step::Give(fragment) => view! { <GiveRequest fragment back=Callback::new(move |()| step.set(Step::Scan)) to_setup/> }.into_any(),
        Step::Off => part(
            "monitor-smartphone",
            s.connect_title,
            view! {
                <p class="pf-empty">{s.connect_off}</p>
                {ways((url::HOME, "house", s.to_start))}
            }
            .into_any(),
        ),
        Step::Failed(problem) => part(
            "monitor-smartphone",
            s.connect_title,
            view! {
                <p class="action note-action pf-refused" role="status"><Icon name="triangle-alert"/><span>{problem_text(problem)}</span></p>
                <nav class="pf-ways">
                    <button class="btn primary" type="button" on:click=move |_| step.set(Step::Scan)><Icon name="scan-line"/><span>{c.scan_retry}</span></button>
                    <a class="btn secondary" href=t.path(url::HOME) rel="nofollow"><Icon name="house"/><span>{s.to_start}</span></a>
                </nav>
            }
            .into_any(),
        ),
    } };
    view! {
        <Title text=s.connect_title/>
        <Plain>
            <div class="page-inner pf pf-connect">
                <ConnectSeo/>
                {body}
            </div>
        </Plain>
    }
    .into_any()
}

/// The scanner with its other ways: the recovery code, and a code shown here for another device.
#[component]
fn ScanPart(sync: Option<Sync>, note: RwSignal<Option<&'static str>>, read: Callback<String>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let c = &t.scan;
    let Some(sync) = sync else { return ().into_any() };
    let profiles = Profiles::expect();
    let id = Memo::new(move |_| profiles.map_or(FIRST, Profiles::active));
    let state = Memo::new(move |_| sync.state(id.get()));
    let dialog = RwSignal::new(None::<Direction>);
    let code = RwSignal::new(false);
    // A note stands three seconds.
    Effect::new(move |_| {
        if note.get().is_some() {
            set_timeout(move || { note.try_set(None); }, std::time::Duration::from_secs(3));
        }
    });
    view! {
        <section class="panel pf-part pf-scan" aria-labelledby="pf-connect">
            <h2 id="pf-connect"><Icon name="scan-line"/>{s.connect_title}</h2>
            <Scanner on_read=read/>
            <p class="pf-sub pf-scan-private">{c.scan_private}</p>
            {move || note.get().map(|said| view! { <p class="action note-action pf-refused" role="status"><Icon name="info"/><span>{said}</span></p> })}
            <div class="pf-scan-ways">
                <button class="pf-scan-way" type="button" id="pf-scan-code" aria-expanded=move || if code.get() { "true" } else { "false" } on:click=move |_| code.update(|open| *open = !*open)>
                    <Icon name="lock"/><span class="pf-scan-way-text"><b>{c.scan_code}</b></span><Icon name="chevron-down" class="pf-end"/>
                </button>
                {move || code.get().then(|| view! { <FromOther sync id="pf-scan-code-field"/> })}
                <button class="pf-scan-way" type="button" id="pf-scan-show" on:click=move |_| dialog.set(Some(Direction::Give))>
                    <Icon name="monitor-smartphone"/><span class="pf-scan-way-text"><b>{c.scan_show}</b><small>{c.scan_show_hint}</small></span><Icon name="chevron-right" class="pf-end"/>
                </button>
            </div>
            <Connect sync id state dialog both=false/>
        </section>
    }
    .into_any()
}

/// This site's origin (`https://betula.app`), which a code read must name.
fn origin() -> String {
    #[cfg(feature = "csr")]
    {
        web_sys::window().and_then(|w| w.location().origin().ok()).unwrap_or_default()
    }
    #[cfg(not(feature = "csr"))]
    String::new()
}

/// Whether the page's query says it was opened from the start page's setup (`url::from_setup`).
/// Only in the browser.
fn from_setup_query() -> bool {
    #[cfg(feature = "csr")]
    {
        web_sys::window().and_then(|window| window.location().search().ok()).is_some_and(|search| url::from_setup(&search))
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// The part of the address after `#`, taken out of the address and of the history (the entry is
/// replaced by the same address without it); `None` where there is none. Only in the browser.
pub(crate) fn take_fragment() -> Option<String> {
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
