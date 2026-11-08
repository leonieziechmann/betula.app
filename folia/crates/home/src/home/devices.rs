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
//!   and the link (`folia_widgets::syncpair`), ten minutes, once.
//! - **Joined**: once one more device of the profile is online than when the link was made, the
//!   other device has the profile; „Noch ein Gerät verbinden" makes another link.
//!
//! What it says names the phone, the device one has beside the computer; on a phone's layout it
//! names the laptop. „Fertig" (or „Später") ends the setup.

use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_stores::profiles::{Profiles, FIRST};
use folia_stores::sync::{Pairing, Problem, State, Sync};
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

    let pair = move || {
        let Some(sync) = sync else { return };
        before.set(match state.get_untracked() {
            State::Online { devices } => devices,
            _ => 0,
        });
        joined.set(false);
        link.set(Link::Making);
        sync.pair(id.get_untracked(), move |made| {
            // Still the question of this step: a link made for what is gone is not shown.
            if link.try_with_untracked(|now| *now == Link::Making).unwrap_or(false) {
                link.try_set(match made {
                    Ok(pairing) => Link::Made(pairing),
                    Err(problem) => Link::Failed(problem),
                });
            }
        });
    };
    // Synced, switched on here or before: the link for the next device at once (entwurf.html:
    // „Danach zeigt er QR-Code und Link für das nächste Gerät").
    Effect::new(move |_| {
        if synced.get() && link.with_untracked(|link| *link == Link::None) {
            pair();
        }
    });
    Effect::new(move |_| {
        if let State::Online { devices } = state.get() {
            if devices > before.get_untracked() && link.with_untracked(|link| matches!(link, Link::Made(_))) {
                joined.set(true);
            }
        }
    });
    let activate = move |_| {
        if let (Some(sync), false) = (sync, state.with_untracked(|state| *state == State::Starting)) {
            sync.activate(id.get_untracked());
        }
    };
    let finish = move |_| on_finish.run(());

    let title = move || match (synced.get(), joined.get(), phone) {
        (true, true, _) => s.joined_title,
        (true, false, false) => s.link_title,
        (true, false, true) => s.link_title_phone,
        (false, _, false) => s.devices_title,
        (false, _, true) => s.devices_title_phone,
    };
    let lead = move || match (synced.get(), joined.get(), phone) {
        (true, true, _) => s.joined_lead,
        (true, false, false) => s.link_lead,
        (true, false, true) => s.link_lead_phone,
        (false, _, false) => s.devices_lead,
        (false, _, true) => s.devices_lead_phone,
    };

    // What the step holds: the points before it is on; the link once it is; the other device once
    // it has the profile.
    let held = move || {
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
        if joined.get() {
            return view! {
                <p class="ob-note ok" role="status"><Icon name="circle-check-big"/><span>{s.joined}</span></p>
                <button type="button" class="btn sm ob-more" on:click=move |_| pair() data-command=""><Icon name="smartphone"/>{s.pair_more}</button>
            }
            .into_any();
        }
        match link.get() {
            Link::None | Link::Making => view! { <p class="ob-wait" role="status">{s.link_making}</p> }.into_any(),
            Link::Made(pairing) => view! { <PairLink pairing again=Callback::new(move |()| pair())/> }.into_any(),
            Link::Failed(problem) => view! {
                <p class="ob-note" role="status"><Icon name="triangle-alert"/><span>{s.failed}" "{problem_text(problem)}</span></p>
                <button type="button" class="btn sm ob-more" on:click=move |_| pair() data-command="">{s.retry}</button>
            }
            .into_any(),
        }
    };

    let actions = move || {
        if synced.get() {
            return view! { <button type="button" class="btn primary ob-finish" on:click=finish data-command=""><Icon name="check"/>{s.finish}</button> }.into_any();
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
        <h1 class="ob-title">{title}</h1>
        <p class="ob-lede">{lead}</p>
        <div class="ob-fill" data-scroll-shade="">{held}</div>
        <div class="ob-actions">{actions}</div>
    }
}
