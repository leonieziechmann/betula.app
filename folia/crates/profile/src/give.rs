//! What a phone asks before it sends a profile to the device whose code it read (`/sync`,
//! folia/design/profil/konzepte.html, the second step): which profile (the active one first; a
//! private one is switched on for it), the four digits the other device shows beside its code, and
//! „Ja, „Lena“ verbinden". Then the profile's key goes there, sealed to the code's key
//! (`folia_stores::sync::Sync::give`), and the page says once the other device has come.

use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_routes::url;
use folia_stores::profiles::{Avatar, Profiles};
use folia_stores::sync::{Problem, State, Sync};

use crate::i18n;
use crate::sync::problem_text;

/// The id the focus goes to: the answer.
const YES_ID: &str = "pf-give-yes";

/// Where the page is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Asking,
    /// The profile goes (and is switched on first where it is private).
    Sending(String),
    /// Gone to Phloem: the profile's name, and its other devices online then.
    Sent(String, Option<usize>),
    /// The other device has come.
    Joined(String),
    Failed(Problem),
}

/// The question for the request `fragment` (what followed `#` in its link); `back` leaves it
/// unanswered.
#[component]
pub fn GiveRequest(fragment: String, #[prop(into)] back: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let c = &t.scan;
    let (Some(sync), Some(profiles)) = (Sync::expect(), Profiles::expect()) else { return ().into_any() };
    let all = Memo::new(move |_| profiles.all());
    let chosen = RwSignal::new(profiles.active_untracked());
    let name = Memo::new(move |_| all.with(|all| all.iter().find(|p| p.id == chosen.get()).map(|p| p.shown(t.locale)).unwrap_or_default()));
    let synced = Memo::new(move |_| sync.state(chosen.get()).synced());
    // A profile that holds nothing here: the camera app may have opened the code in a browser that is
    // not the one Betula is used in.
    let empty = Memo::new(move |_| {
        all.track();
        profiles.holds_nothing(chosen.get())
    });
    // The digits, once the browser has them; `Some(None)`: no request.
    let digits = RwSignal::new(None::<Option<String>>);
    sync.request_digits(&fragment, move |made| {
        digits.try_set(Some(made));
        request_animation_frame(|| folia_design::nav::focus_by_id(YES_ID));
    });
    let step = RwSignal::new(Step::Asking);
    let online = move |state: &State| match state {
        State::Online { devices, .. } => Some(*devices),
        _ => None,
    };
    // Sent: one more device of the profile online is the one that came.
    Effect::new(move |_| {
        if let Step::Sent(name, before) = step.get() {
            let now = online(&sync.state(chosen.get_untracked()));
            if now.zip(before).is_some_and(|(now, before)| now > before) || (before.is_none() && now.is_some_and(|now| now > 0)) {
                step.set(Step::Joined(name));
            }
        }
    });
    let send = {
        let fragment = fragment.clone();
        move |_| {
            if !matches!(step.get_untracked(), Step::Asking | Step::Failed(_)) {
                return;
            }
            let (id, shown) = (chosen.get_untracked(), name.get_untracked());
            step.set(Step::Sending(shown.clone()));
            sync.give(id, &fragment, move |done| {
                step.try_set(match done {
                    Ok(()) => Step::Sent(shown, online(&sync.state(id))),
                    Err(problem) => Step::Failed(problem),
                });
            });
        }
    };
    let pick = move || {
        all.get()
            .into_iter()
            .map(|profile| {
                let id = profile.id;
                let shown = profile.shown(t.locale);
                let line = if sync.state(id).synced() { c.give_synced } else { c.give_private };
                view! {
                    <label class="pf-give-item" class:on=move || chosen.get() == id>
                        <input type="radio" name="pf-give" prop:checked=move || chosen.get() == id on:change=move |_| chosen.set(id)/>
                        <Avatar profile/>
                        <span class="pf-give-text"><b>{shown}</b><small>{line}</small></span>
                    </label>
                }
            })
            .collect_view()
    };
    let asking = move || {
        let send = send.clone();
        match digits.get() {
            Some(None) => view! {
                <p class="action note-action pf-refused" role="status"><Icon name="triangle-alert"/><span>{c.give_bad}</span></p>
                <div class="pf-give-actions"><button class="btn secondary" type="button" on:click=move |_| back.run(())>{c.give_back}</button></div>
            }
            .into_any(),
            shown => view! {
                <fieldset class="pf-give-pick">
                    <legend class="label">{c.give_which}</legend>
                    {pick}
                </fieldset>
                {move || empty.get().then(|| view! { <p class="action note-action pf-refused" role="status"><Icon name="triangle-alert"/><span>{t.sync.give_empty}</span></p> })}
                {move || (!synced.get()).then(|| view! { <p class="pf-sub">{(c.give_switch_on)(&name.get())}</p> })}
                <div class="pf-give-check">
                    <p><b>{c.give_digits}</b></p>
                    {shown.flatten().map(|digits| view! { <Digits digits/> })}
                    <p class="pf-sub">{move || (c.give_only_if)(&name.get())}</p>
                </div>
                <div class="pf-give-actions">
                    <button class="btn primary" type="button" id=YES_ID on:click=send><Icon name="check"/><span>{move || (c.give_yes)(&name.get())}</span></button>
                    <button class="btn secondary" type="button" on:click=move |_| back.run(())>{c.give_back}</button>
                </div>
            }
            .into_any(),
        }
    };
    let ways = move || {
        view! {
            <nav class="pf-ways">
                <a class="btn primary" href=t.path(url::PROFILE) rel="nofollow"><Icon name="check"/><span>{c.done}</span></a>
            </nav>
        }
    };
    let body = move || match step.get() {
        Step::Asking => asking().into_any(),
        Step::Sending(name) => view! {
            <p class="pf-sync-state" role="status" data-state="wait"><span class="pf-sync-dot" aria-hidden="true"></span><span>{(c.give_sending)(&name)}</span></p>
        }
        .into_any(),
        Step::Sent(name, _) => view! {
            <p class="pf-sync-state" role="status" data-state="wait"><span class="pf-sync-dot" aria-hidden="true"></span><span>{(c.give_sent)(&name)}</span></p>
            {ways()}
        }
        .into_any(),
        Step::Joined(name) => view! {
            <p class="pf-joined" role="status"><Icon name="check"/><span>{(c.give_joined)(&name)}</span></p>
            {ways()}
        }
        .into_any(),
        Step::Failed(problem) => view! {
            <p class="action note-action pf-refused" role="status"><Icon name="triangle-alert"/><span>{problem_text(problem)}</span></p>
            {asking()}
        }
        .into_any(),
    };
    view! {
        <section class="panel pf-part pf-give" aria-labelledby="pf-give">
            <h2 id="pf-give"><Icon name="monitor"/>{c.give_title}</h2>
            {body}
        </section>
    }
    .into_any()
}

/// Four digits, each in a box of its own, read one by one.
#[component]
pub(crate) fn Digits(digits: String) -> impl IntoView {
    let t = i18n::t();
    view! {
        <p class="pf-digits" role="img" aria-label=(t.scan.digits_label)(&digits)>
            {digits.chars().map(|digit| view! { <span aria-hidden="true">{digit.to_string()}</span> }).collect_view()}
        </p>
    }
}
