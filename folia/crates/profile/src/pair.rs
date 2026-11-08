//! „Mit dem Handy verbinden" (`/pair`, Betula Sync; owner, 2026-11-08: „Da muss ein link kommen wie
//! betula.app/pair wo dann der qr code gezeigt wird, den man am handy scannen kann um sich
//! einzuloggen"). The other way round from „Weiteres Gerät verbinden", for a computer, which has
//! no camera to scan with: the device that wants the profile shows the code, and the one that holds
//! it scans.
//!
//! - **The computer** (`/pair`): a question made here (`Sync::ask`: a parcel's id and the key
//!   that opens it), shown as a QR code of `/pair#<id ‖ key>`, ten minutes. It asks Phloem whether
//!   the answer is there, every 3 s in the first 90 s, in which a phone usually answers, then every
//!   10 s (Phloem answers 120 such questions per address and hour); once it is, the profile comes
//!   as from a link of another device (`Sync::collect`, as `connect`), and the page says so.
//! - **The phone** (`/pair#<…>`, opened by its camera): which profile goes to the device that
//!   shows the code, what it holds (or that it is empty, as in a browser that is not the one Betula
//!   is used in), and „Verbinden"; a private profile switches Betula Sync on on the way, with what
//!   the server then keeps above the button. Its answer is the profile's key, sealed with the
//!   question's key (`Sync::give`).
//!
//! What follows `#` never reaches a server; the page takes it out of the address at once. The
//! server writes the same explanation for everybody (R9), `noindex`.

use std::time::Duration;

use leptos::prelude::*;

use folia_design::stage::Title;
use folia_design::ui::Icon;
use folia_routes::url;
use folia_shell::frame::Plain;
use folia_shell::pending::Shape;
use folia_shell::seo::Seo;
use folia_shell::skeleton::AppStandin;
use folia_stores::myprogram::MyProgram;
use folia_stores::profiles::{Profiles, FIRST};
use folia_stores::sync::{Asking, Problem, Sync, ASK_MS};
use folia_widgets::syncpair::{now_ms, problem_text};

use crate::connect::take_fragment;
use crate::i18n;

/// Whether this build is the browser app.
const APP: bool = cfg!(feature = "csr");

/// How long the computer waits before it asks Phloem again, `waited` milliseconds after the code
/// came: often while a phone usually answers, then less, within what Phloem answers an address.
fn pause_after(waited: u64) -> u64 {
    if waited < 90_000 {
        3_000
    } else {
        10_000
    }
}

/// The minutes a code still waits, rounded down; `None` once it waits no more.
fn minutes_left(until: u64, now: u64) -> Option<u64> {
    (until > now).then(|| (until - now) / 60_000)
}

/// Where the computer is.
#[derive(Clone, Debug, PartialEq)]
enum Ask {
    Off,
    Waiting(Asking),
    Gone,
    /// The profile is here, under this name.
    Joined(String),
    Failed(Problem),
}

/// Where the phone is.
#[derive(Clone, Debug, PartialEq)]
enum Give {
    Off,
    NoLink,
    Asking,
    Going,
    Given,
    Failed(Problem),
}

#[component]
fn PairSeo() -> impl IntoView {
    let t = i18n::t();
    view! { <Seo title=t.sync.ask_title description=t.sync.ask_description path=url::PAIR noindex=true/> }
}

#[component]
pub fn PairPage() -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    if !APP {
        return view! {
            <Title text=s.ask_title/>
            <PairSeo/>
            <AppStandin shape=Shape::Text title=s.ask_title hint=s.ask_server_hint/>
        }
        .into_any();
    }
    // Read once, as the page is made: the fragment goes out of the address at once.
    let body = match take_fragment() {
        Some(fragment) => view! { <GiveProfile fragment/> }.into_any(),
        None => view! { <AskCode/> }.into_any(),
    };
    view! {
        <Title text=s.ask_title/>
        <Plain>
            <div class="page-inner pf pf-connect pf-pair-page">
                <PairSeo/>
                <section class="panel pf-part" aria-labelledby="pf-pair">{body}</section>
            </div>
        </Plain>
    }
    .into_any()
}

/// Asks Phloem whether the answer to `asking` is there, and again after a pause while it is not,
/// as long as the page still waits for it.
fn wait_for(sync: Sync, profiles: Option<Profiles>, state: RwSignal<Ask>, asking: Asking) {
    let t = i18n::t();
    let waiting = state.try_with_untracked(|now| *now == Ask::Waiting(asking.clone())).unwrap_or(false);
    if !waiting {
        return;
    }
    if now_ms() >= asking.until {
        state.set(Ask::Gone);
        return;
    }
    let asked = asking.clone();
    sync.collect(&asked, move |result| match result {
        Ok(Some(id)) => {
            let name = profiles.map(|profiles| {
                profiles.switch(id);
                untrack(|| profiles.all()).into_iter().find(|profile| profile.id == id).map(|profile| profile.shown(t.locale)).unwrap_or_default()
            });
            state.try_set(Ask::Joined(name.unwrap_or_default()));
        }
        Ok(None) => {
            let pause = pause_after(now_ms().saturating_sub(asking.until.saturating_sub(ASK_MS)));
            set_timeout(move || wait_for(sync, profiles, state, asking), Duration::from_millis(pause));
        }
        Err(problem) => {
            state.try_set(Ask::Failed(problem));
        }
    });
}

/// The computer: the code to scan, while it waits; then the profile.
#[component]
fn AskCode() -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let sync = Sync::expect().filter(|_| folia_stores::sync::offered().is_some());
    let profiles = Profiles::expect();
    let state = RwSignal::new(Ask::Off);
    let ask = move || {
        let Some(sync) = sync else { return };
        match sync.ask() {
            Some(asking) => {
                state.set(Ask::Waiting(asking.clone()));
                // The first question once the phone has had a moment.
                set_timeout(move || wait_for(sync, profiles, state, asking), Duration::from_millis(pause_after(0)));
            }
            None => state.set(Ask::Failed(Problem::Unsupported)),
        }
    };
    ask();
    let now = RwSignal::new(now_ms());
    if let Ok(handle) = set_interval_with_handle(move || now.set(now_ms()), Duration::from_secs(5)) {
        on_cleanup(move || handle.clear());
    }
    let ways = move |first: (&'static str, &'static str, &'static str)| {
        view! {
            <nav class="pf-ways">
                <a class="btn primary" href=t.path(first.0) rel="nofollow"><Icon name=first.1/><span>{first.2}</span></a>
                <a class="btn secondary" href=t.path(url::PROFILE) rel="nofollow"><Icon name="user-round"/><span>{s.to_profile}</span></a>
            </nav>
        }
    };
    let body = move || match state.get() {
        Ask::Off => view! {
            <p class="pf-empty">{s.ask_off}</p>
            {ways((url::HOME, "house", s.to_start))}
        }
        .into_any(),
        Ask::Waiting(asking) => {
            let until = asking.until;
            let qr = folia_design::qr::path(&asking.link).map(|(path, width)| view! {
                <svg class="pf-qr" viewBox=format!("0 0 {width} {width}") role="img" aria-label=s.ask_qr_label shape-rendering="crispEdges" data-link=asking.link.clone()>
                    <path d=path/>
                </svg>
            });
            view! {
                <div class="pf-pair pf-asking">
                    {qr}
                    <div class="pf-pair-text">
                        <p>{s.ask_lead}</p>
                        <p class="pf-sync-state" role="status" data-state="wait"><span class="pf-sync-dot" aria-hidden="true"></span><span>{s.ask_waiting}</span></p>
                        <p class="pf-pair-left">{move || minutes_left(until, now.get()).map(s.ask_left)}</p>
                    </div>
                </div>
                <p class="pf-empty">{s.ask_file}" "<a href=t.path(url::PROFILE) rel="nofollow">{s.to_profile}</a></p>
            }
            .into_any()
        }
        Ask::Gone => view! {
            <p class="action note-action pf-refused" role="status">
                <Icon name="clock-3"/><span>{s.ask_gone}</span>
                <button class="mini primary hit" type="button" on:click=move |_| ask()>{s.ask_new}</button>
            </p>
        }
        .into_any(),
        Ask::Joined(name) => view! {
            <p class="pf-sync-state" role="status" data-state="on"><span class="pf-sync-dot" aria-hidden="true"></span><span>{(s.connected)(&name)}</span></p>
            <p class="pf-empty">{s.connected_hint}</p>
            {ways((url::HOME, "house", s.to_start))}
        }
        .into_any(),
        Ask::Failed(problem) => view! {
            <p class="action note-action pf-refused" role="status">
                <Icon name="triangle-alert"/><span>{problem_text(problem)}</span>
                <button class="mini hit" type="button" on:click=move |_| ask()>{s.ask_new}</button>
            </p>
        }
        .into_any(),
    };
    // Once the minutes are over while it waits, the code goes.
    Effect::new(move |_| {
        let gone = state.with(|state| matches!(state, Ask::Waiting(asking) if asking.until <= now.get()));
        if gone {
            state.set(Ask::Gone);
        }
    });
    view! {
        <h2 id="pf-pair"><Icon name="monitor-smartphone"/>{s.ask_title}</h2>
        {body}
    }
}

/// The phone: which profile goes where, „Verbinden"; then that it went.
#[component]
fn GiveProfile(fragment: String) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let sync = Sync::expect().filter(|_| folia_stores::sync::offered().is_some());
    let profiles = Profiles::expect();
    let id = Memo::new(move |_| profiles.map_or(FIRST, Profiles::active));
    let synced = Memo::new(move |_| sync.is_some_and(|sync| sync.state(id.get()).synced()));
    let step = RwSignal::new(match &sync {
        None => Give::Off,
        Some(_) if !Sync::is_link(&fragment) => Give::NoLink,
        Some(_) => Give::Asking,
    });
    let name = Memo::new(move |_| {
        let id = id.get();
        profiles.and_then(|profiles| profiles.all().into_iter().find(|profile| profile.id == id)).map(|profile| profile.shown(t.locale)).unwrap_or_default()
    });
    let program = Memo::new(move |_| MyProgram::expect().and_then(|mine| mine.with(|doc| doc.name.clone())));
    let empty = Memo::new(move |_| profiles.is_some_and(|profiles| profiles.holds_nothing(id.get())));
    let go = move |_| {
        let Some(sync) = sync else { return };
        if step.get_untracked() == Give::Going {
            return;
        }
        step.set(Give::Going);
        let profile = id.get_untracked();
        let fragment = fragment.clone();
        let give = move || sync.give(profile, &fragment, move |done| { step.try_set(match done { Ok(()) => Give::Given, Err(problem) => Give::Failed(problem) }); });
        if sync.state(profile).synced() {
            give();
        } else {
            sync.activate_then(profile, move |done| match done {
                Ok(()) => give(),
                Err(problem) => {
                    step.try_set(Give::Failed(problem));
                }
            });
        }
    };
    let to_start = move || view! {
        <nav class="pf-ways">
            <a class="btn primary" href=t.path(url::HOME) rel="nofollow"><Icon name="house"/><span>{s.to_start}</span></a>
            <a class="btn secondary" href=t.path(url::PROFILE) rel="nofollow"><Icon name="user-round"/><span>{s.to_profile}</span></a>
        </nav>
    };
    let body = move || match step.get() {
        Give::Off => view! { <p class="pf-empty">{s.ask_off}</p>{to_start()} }.into_any(),
        Give::NoLink => view! { <p class="pf-empty">{s.give_none}</p>{to_start()} }.into_any(),
        Give::Asking | Give::Going => {
            let going = step.with(|step| *step == Give::Going);
            view! {
                <p class="pf-sub">{move || (s.give_lead)(&name.get())}</p>
                <p class="pf-give-what"><Icon name="user-round"/><b>{move || name.get()}</b>{move || program.get().map(|program| view! { <span>{(s.give_program)(&program)}</span> })}</p>
                {move || empty.get().then(|| view! { <p class="action note-action pf-refused" role="status"><Icon name="triangle-alert"/><span>{s.give_empty}</span></p> })}
                {move || (!synced.get()).then(|| view! {
                    <div class="pf-offer pf-sync-explain">
                        <p><b>{s.explain_heading}</b></p>
                        <ul>{s.explain_points.iter().map(|point| view! { <li>{*point}</li> }).collect_view()}</ul>
                    </div>
                })}
                <nav class="pf-ways">
                    <button class="btn primary pf-give" type="button" aria-disabled=going.then_some("true") on:click=go.clone()>
                        <Icon name="monitor-smartphone"/>
                        <span>{move || if going { s.give_going } else if synced.get() { s.give_yes } else { s.give_activate }}</span>
                    </button>
                    <a class="btn secondary" href=t.path(url::HOME) rel="nofollow">{s.cancel}</a>
                </nav>
            }
            .into_any()
        }
        Give::Given => view! {
            <p class="pf-sync-state" role="status" data-state="on"><span class="pf-sync-dot" aria-hidden="true"></span><span>{s.given}</span></p>
            <p class="pf-empty">{s.given_hint}</p>
            {to_start()}
        }
        .into_any(),
        Give::Failed(problem) => view! {
            <p class="action note-action pf-refused" role="status"><Icon name="triangle-alert"/><span>{problem_text(problem)}</span></p>
            {to_start()}
        }
        .into_any(),
    };
    view! {
        <h2 id="pf-pair"><Icon name="monitor-smartphone"/>{s.give_title}</h2>
        {body}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Often while a phone usually answers, then less: ten minutes stay under the 120 questions an
    /// hour Phloem answers an address.
    #[test]
    fn the_computer_asks_within_what_phloem_answers() {
        let mut waited = 0;
        let mut asked = 0;
        while waited < ASK_MS {
            waited += pause_after(waited);
            asked += 1;
        }
        assert!(asked <= 90, "{asked}");
        assert_eq!((pause_after(0), pause_after(120_000)), (3_000, 10_000));
        assert_eq!((minutes_left(600_000, 0), minutes_left(600_000, 599_999), minutes_left(600_000, 600_000)), (Some(10), Some(0), None));
    }
}
