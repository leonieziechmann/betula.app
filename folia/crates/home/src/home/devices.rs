//! The setup's last step, where the site offers Betula Sync: the profile on the phone as well (owner,
//! 2026-11-08: „es gibt ja jetzt betula sync. Mach das mal als step in die einführung mit rein. So
//! als letzten schritt. So dass man es gleich auch auf dem handy hat"). It comes after „Studium
//! anlegen", so that what the phone gets is the study just set up.
//!
//! - **Private**: what Betula Sync does, and what the server then keeps (sealed changes, their sizes
//!   and times, random ids; never what is in them) and that it ends at any time, as the profile
//!   page says it before „Aktivieren"; „Betula Sync aktivieren" switches it on in one click, since
//!   all of that stands above it, and „Später" leaves it for the profile page.
//! - **Synced** (switched on here, or before): the link for the next device at once, its QR code
//!   and the link (`folia_widgets::syncpair`), ten minutes, once. On a phone's layout the other way
//!   round, since a laptop has no camera to scan with (owner, 2026-11-08: „Oh der pc prozess muss
//!   anders sein. Da muss ein link kommen wie betula.app/pair wo dann der qr code gezeigt wird, den
//!   man am handy scannen kann um sich einzuloggen"): the address to open on the laptop, `/pair`,
//!   which shows a code that this phone scans (`folia_profile::pair`), with Betula's own scanner
//!   (`/sync?from=setup`, which leads back here once the code is answered).
//! - **Joined**: once one more device of the profile is online than when the link was made, the
//!   other device has the profile; „Noch ein Gerät verbinden" makes another link.
//!
//! What it says names the phone, the device one has beside the computer; on a phone's layout it
//! names the laptop. „Fertig" (or „Später") ends the setup.

use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_stores::profiles::{Profiles, FIRST};
use folia_stores::sync::{Pairing, Problem, State, Sync};
use folia_widgets::syncdone::{ways, ConnectProgress, Connected};
use folia_widgets::syncpair::{problem_text, PairLink};

use crate::i18n;

/// The icons of the points of a private profile, in the order of their texts.
const POINTS: [&str; 3] = ["lock", "user-round", "trash-2"];

/// The link for the next device, as far as it is.
#[derive(Clone, Debug, PartialEq)]
enum Link {
    None,
    Making,
    Made(Pairing),
    Failed(Problem),
    /// On a phone: the laptop opens `/pair`, and this phone scans its code.
    Elsewhere,
}

/// The step: its title, lead, what it holds and its actions, as the card's other steps have them.
/// `on_finish` ends the setup.
#[component]
pub fn DevicesStep(on_finish: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.setup;
    let sync = Sync::expect();
    let profiles = Profiles::expect();
    let id = Memo::new(move |_| profiles.map_or(FIRST, Profiles::active));
    let state = Memo::new(move |_| sync.map_or(State::Failed(Problem::Unsupported), |sync| sync.state(id.get())));
    let synced = Memo::new(move |_| state.with(State::synced));
    // On a phone's layout the other device is a computer.
    let phone = folia_design::nav::is_phone();
    let link = RwSignal::new(Link::None);
    // How many other devices were online when the link was made; once more are, the next device
    // has the profile, and that stays said.
    let before = RwSignal::new(0usize);
    let joined = RwSignal::new(false);
    // What Phloem had said of opened parcels when the link was made: once more, the phone has
    // fetched it. Then the way on a bar, then the success (`folia_widgets::syncdone`).
    let taken_before = StoredValue::new(0u32);
    let shown_done = RwSignal::new(false);

    let pair = move || {
        let Some(sync) = sync else { return };
        before.set(match state.get_untracked() {
            State::Online { devices, .. } => devices,
            _ => 0,
        });
        joined.set(false);
        shown_done.set(false);
        taken_before.set_value(sync.taken());
        link.set(Link::Making);
        sync.pair(id.get_untracked(), move |made| {
            // Still the question of this step: a link made for what is gone is not shown.
            if link.try_with_untracked(|now| *now == Link::Making).unwrap_or(false) {
                // The link waits for the profile to be online (`Sync::pair`): the devices online
                // now are the ones to compare with.
                if let Some(State::Online { devices, .. }) = state.try_get_untracked() {
                    before.try_set(devices);
                }
                link.try_set(match made {
                    Ok(pairing) => Link::Made(pairing),
                    Err(problem) => Link::Failed(problem),
                });
            }
        });
    };
    // On a phone the laptop asks (`/pair`): no link of this phone's, only how many devices are
    // online now.
    let elsewhere = move || {
        before.set(match state.get_untracked() {
            State::Online { devices, .. } => devices,
            _ => 0,
        });
        joined.set(false);
        shown_done.set(false);
        link.set(Link::Elsewhere);
    };
    let next = move || if phone { elsewhere() } else { pair() };
    // Synced, switched on here or before: the link for the next device at once (entwurf.html:
    // „Danach zeigt er QR-Code und Link für das nächste Gerät").
    Effect::new(move |_| {
        if synced.get() && link.with_untracked(|link| *link == Link::None) {
            next();
        }
    });
    Effect::new(move |_| {
        if let State::Online { devices, .. } = state.get() {
            if devices > before.get_untracked() && link.with_untracked(|link| matches!(link, Link::Made(_) | Link::Elsewhere)) {
                joined.set(true);
            }
        }
    });
    // Where the laptop finds `/pair`: this site's host.
    let address = {
        #[cfg(feature = "csr")]
        let host = web_sys::window().and_then(|window| window.location().host().ok()).unwrap_or_default();
        #[cfg(not(feature = "csr"))]
        let host = String::new();
        format!("{host}{}", folia_routes::url::PAIR)
    };
    let activate = move |_| {
        if let (Some(sync), false) = (sync, state.with_untracked(|state| *state == State::Starting)) {
            sync.activate(id.get_untracked());
        }
    };
    let finish = move |_| on_finish.run(());

    // The bar from the moment the phone fetched the link (or came), then the success.
    let handing = Memo::new(move |_| {
        let taken = link.with(|link| matches!(link, Link::Made(_))) && sync.is_some_and(|sync| sync.taken() != taken_before.get_value());
        match (joined.get(), shown_done.get()) {
            (true, true) => Some(true),
            (true, false) => Some(false),
            (false, _) => taken.then_some(false),
        }
    });
    let handed = Signal::derive(move || if joined.get() { 2 } else { 1 });
    let title = move || match (synced.get(), phone) {
        (true, false) => s.link_title,
        (true, true) => s.link_title_phone,
        (false, false) => s.devices_title,
        (false, true) => s.devices_title_phone,
    };
    let lead = move || match (synced.get(), phone) {
        (true, false) => s.link_lead,
        (true, true) => s.link_lead_phone,
        (false, false) => s.devices_lead,
        (false, true) => s.devices_lead_phone,
    };

    // What the step holds: the points before it is on; the link once it is; the other device once
    // it has the profile.
    let held_step = move || {
        if !synced.get() {
            let failed = state.with(|state| match state {
                State::Failed(problem) => Some(*problem),
                _ => None,
            });
            return view! {
                {failed.map(|problem| view! { <p class="ob-note" role="status"><Icon name="triangle-alert"/><span>{s.failed}" "{problem_text(problem)}</span></p> })}
                <ul class="ob-points">
                    {POINTS.into_iter().zip(s.devices_points).map(|(icon, point)| view! { <li><span class="ob-point-icon" aria-hidden="true"><Icon name=icon/></span><span>{point}</span></li> }).collect_view()}
                </ul>
            }
            .into_any();
        }
        match link.get() {
            Link::None | Link::Making => view! { <p class="ob-wait" role="status">{s.link_making}</p> }.into_any(),
            Link::Made(pairing) => view! { <PairLink pairing again=Callback::new(move |()| pair())/> }.into_any(),
            Link::Elsewhere => view! {
                <div class="ob-elsewhere">
                    <p>{s.elsewhere_open}</p>
                    <p class="ob-address"><Icon name="monitor"/><code translate="no">{address.clone()}</code></p>
                    <p>{s.elsewhere_scan}</p>
                    // Betula's own scanner (`/sync`), so that the code comes to this app's profile:
                    // the camera app opens it in the browser, which on an iPhone keeps its own
                    // storage apart from an app on the home screen.
                    <a class="btn primary ob-scan" href=t.path(&folia_routes::url::connect_from_setup()) rel="nofollow" data-command=""><Icon name="scan-line"/>{s.scan_here}</a>
                </div>
            }
            .into_any(),
            Link::Failed(problem) => view! {
                <p class="ob-note" role="status"><Icon name="triangle-alert"/><span>{s.failed}" "{problem_text(problem)}</span></p>
                <button type="button" class="btn sm ob-more" on:click=move |_| pair() data-command="">{s.retry}</button>
            }
            .into_any(),
        }
    };

    let held = move || match handing.get() {
        Some(false) => view! { <ConnectProgress phases=ways::handing() reached=handed on_done=Callback::new(move |()| shown_done.set(true))/> }.into_any(),
        // „Fertig" is the step's own, under the card's actions.
        Some(true) => view! { <Connected title=t.syncpair.all_set said=s.joined.to_string()/> }.into_any(),
        None => {
            let held_step = held_step.clone();
            view! { {held_step} }.into_any()
        }
    };

    let actions = move || {
        if synced.get() {
            // While the phone's scanner is the way on, it is the step's one confirming button.
            let quiet = link.with(|link| *link == Link::Elsewhere) && handing.get() != Some(true);
            return view! { <button type="button" class="btn ob-finish" class:primary=!quiet on:click=finish data-command=""><Icon name="check"/>{s.finish}</button> }.into_any();
        }
        let starting = state.with(|state| *state == State::Starting);
        let label = if starting {
            s.activating
        } else if state.with(|state| matches!(state, State::Failed(_))) {
            s.retry
        } else {
            s.activate
        };
        view! {
            <button type="button" class="btn ghost ob-later" title=s.later_title on:click=finish data-command="">{s.later}</button>
            <button type="button" class="btn primary ob-activate" aria-disabled=starting.then_some("true") on:click=activate data-command=""><Icon name="monitor-smartphone"/>{label}</button>
        }
        .into_any()
    };

    view! {
        // Once done, the success says what the title and the lead said.
        {move || (handing.get() != Some(true)).then(|| view! {
            <h1 class="ob-title">{title}</h1>
            <p class="ob-lede">{lead}</p>
        })}
        <div class="ob-fill" data-scroll-shade="">{held}</div>
        <div class="ob-actions">{actions}</div>
    }
}
