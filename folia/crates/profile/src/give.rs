//! What a phone asks before it sends a profile to the device whose code it read (`/sync`,
//! folia/design/profil/konzepte.html, the second step): which profile (the active one first; a
//! private one is switched on for it), the four digits the other device shows beside its code, and
//! „Ja, „Lena“ verbinden". Then the profile's key goes there, sealed to the code's key
//! (`folia_stores::sync::Sync::give`), and a bar shows the way as it goes (owner, 2026-11-09: „mach
//! mal eine animation mit progress bar für den sync an sich"): Betula Sync switched on where it was
//! private, the profile sent, the other device fetching it (Phloem's word that the parcel was
//! opened), the data in line (one more device of the profile online). Then the success of concept
//! A (folia/design/verbunden/konzepte.html, `folia_widgets::syncdone::Connected`) with „Fertig",
//! which leads to the start page; from the setup it ends the setup there (`/?setup=done`).

use leptos::prelude::*;

use folia_design::ui::Icon;
use folia_routes::url;
use folia_stores::myprogram::MyProgram;
use folia_stores::profiles::{Avatar, Profiles};
use folia_stores::sync::{Problem, State, Sync};
use folia_widgets::syncdone::{ways, ConnectProgress, Connected};

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

/// What the part shows, coarser than `Step`: the bar stays the same while the way goes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shows {
    Asking,
    Going,
    Done,
    Failed,
}

/// The question for the request `fragment` (what followed `#` in its link); `back` leaves it
/// unanswered. `to_setup`: the scanner was opened from the start page's setup, and „Fertig" ends
/// the setup on the start page.
#[component]
pub fn GiveRequest(fragment: String, #[prop(into)] back: Callback<()>, #[prop(optional)] to_setup: bool) -> impl IntoView {
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
    // The bar's steps (switched on first where the profile was private), what Phloem had said of
    // opened parcels when it went, and whether the bar has shown it all.
    let phases = RwSignal::new(ways::giving(false));
    let taken_before = StoredValue::new(0u32);
    let shown_done = RwSignal::new(false);
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
            phases.set(ways::giving(!sync.state(id).synced()));
            taken_before.set_value(sync.taken());
            shown_done.set(false);
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
    // How far the way really is, as an index of the bar's steps.
    let reached = Signal::derive(move || {
        let all = phases.with(Vec::len);
        let private = phases.with(|phases| phases.first() == Some(&folia_widgets::syncdone::Phase::SwitchOn));
        let offset = usize::from(private);
        match step.get() {
            Step::Sending(_) if private && !sync.state(chosen.get_untracked()).synced() => 0,
            Step::Sending(_) => offset,
            Step::Sent(..) if sync.taken() != taken_before.get_value() => offset + 2,
            Step::Sent(..) => offset + 1,
            Step::Joined(_) => all,
            Step::Asking | Step::Failed(_) => 0,
        }
    });
    let progress = move |name: String| {
        view! {
            <p class="pf-sub">{(c.give_sending)(&name)}</p>
            <ConnectProgress phases=phases.get_untracked() reached on_done=Callback::new(move |()| shown_done.set(true))/>
            <nav class="pf-ways pf-wait-ways">
                <a class="btn ghost" href=t.path(url::HOME) rel="nofollow">{c.dont_wait}</a>
            </nav>
        }
    };
    // Done: the start page (owner, 2026-11-09: „dass man wieder bei der main page landet, wenn das
    // sync fertig ist und nicht im profil"); from the setup the setup ends there.
    let connected = move |name: String| {
        let (title, summary) = if to_setup {
            let program = MyProgram::expect().and_then(|mine| untrack(|| mine.with(|doc| doc.name.clone())));
            (t.syncpair.all_set, program.into_iter().chain([c.summary_sync.to_string(), c.summary_other.to_string()]).collect())
        } else {
            (t.syncpair.connected, Vec::new())
        };
        let href = if to_setup { url::home_setup_done() } else { url::HOME.to_string() };
        view! {
            <Connected title said=(c.give_said)(&name) summary>
                <a class="btn primary pf-done" href=t.path(&href) rel="nofollow"><Icon name="check"/><span>{c.done}</span></a>
            </Connected>
        }
    };
    // What the part shows: built anew only when that changes, so that the bar goes on through the
    // steps of the way.
    let shows = Memo::new(move |_| match step.get() {
        Step::Asking => Shows::Asking,
        Step::Failed(_) => Shows::Failed,
        Step::Joined(_) if shown_done.get() => Shows::Done,
        Step::Sending(_) | Step::Sent(..) | Step::Joined(_) => Shows::Going,
    });
    let going_name = move || match step.get_untracked() {
        Step::Sending(name) | Step::Sent(name, _) | Step::Joined(name) => name,
        Step::Asking | Step::Failed(_) => name.get_untracked(),
    };
    let body = move || match (shows.get(), step.get_untracked()) {
        (Shows::Asking, _) => asking().into_any(),
        (Shows::Going, _) => progress(going_name()).into_any(),
        (Shows::Done, _) => connected(going_name()).into_any(),
        (Shows::Failed, Step::Failed(problem)) => view! {
            <p class="action note-action pf-refused" role="status"><Icon name="triangle-alert"/><span>{problem_text(problem)}</span></p>
            {asking()}
        }
        .into_any(),
        (Shows::Failed, _) => asking().into_any(),
    };
    view! {
        <section class="panel pf-part pf-give" aria-labelledby="pf-give">
            // Once done, the success's own title says it.
            {move || (shows.get() != Shows::Done).then(|| view! { <h2 id="pf-give"><Icon name="monitor"/>{c.give_title}</h2> })}
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
