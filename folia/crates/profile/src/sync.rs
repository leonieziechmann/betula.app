//! „Betula Sync" on the profile page (research/sync/entwurf.html, „Vom privaten Profil zum
//! synchronisierten"): the active profile's state and what can be done with it, where the site
//! offers Betula Sync (`folia_stores::sync::offered`).
//!
//! - **Private**: what it is, and „Betula Sync aktivieren", which first says what the server then
//!   keeps (sealed changes, their sizes and times, random ids; never what is in them) and that a
//!   recovery code is offered, not asked for; then „Aktivieren".
//! - **Synced**: online with how many other devices, or offline; „Weiteres Gerät verbinden" (a QR
//!   code and the link, ten minutes, once), „Wiederherstellungscode zeigen", „Nur dieses Gerät
//!   trennen" and „Sync beenden", each of the last two after a question.
//! - **Ended** on another device, or failed: why, and the way on.
//! - What two devices did at once in the profile, each with the way to where it is shown (the hint
//!   there decides, `folia_widgets::syncnote`).
//! - „Profil von einem anderen Gerät": the recovery code typed in, which brings the profile as a new
//!   one; a link of another device opens „Gerät verbinden" (`connect`).
//!
//! Every action is a button (CLAUDE.md, „No action is a key's alone"). The link and the code are
//! made in the browser and never reach a server; what follows `#` in the link is the key.

use leptos::prelude::*;

use folia_design::nav;
use folia_design::ui::Icon;
use folia_pages::ask::StudyplanModulesAsk;
use folia_data::use_ask;
use folia_routes::url;
use folia_stores::profiles::{Profiles, FIRST, MAX_PROFILES};
use folia_stores::sync::{Conflict, Pairing, Problem, State, Sync};

use crate::i18n::{self, Texts};

const ACTIVATE_ID: &str = "pf-sync-activate";
const ACTIVATE_YES_ID: &str = "pf-sync-yes";
const PAIR_ID: &str = "pf-sync-pair";
const CODE_ID: &str = "pf-sync-code";
const LEAVE_ID: &str = "pf-sync-leave";
const LEAVE_YES_ID: &str = "pf-sync-leave-yes";
const STOP_ID: &str = "pf-sync-stop";
const STOP_YES_ID: &str = "pf-sync-stop-yes";
const RECOVER_ID: &str = "pf-sync-recover";

/// How long „Kopiert" stands in the button's place.
const COPIED_MS: u64 = 1600;

/// What is open in the part, below its actions.
#[derive(Clone, Debug, PartialEq)]
enum Open {
    Nothing,
    /// The explanation before „Aktivieren".
    Explain,
    /// The link for the next device: while it is made, then the link or why there is none.
    Pair(Option<Result<Pairing, Problem>>),
    /// The recovery code, shown.
    Code(String),
    /// The question before „Nur dieses Gerät trennen" or „Sync beenden".
    Leave,
    Stop,
}

/// The sentence of a problem, with what to do.
pub fn problem_text(problem: Problem, t: &Texts) -> String {
    let s = &t.sync;
    match problem {
        Problem::Budget => s.budget.to_string(),
        Problem::SlowDown => s.slow_down.to_string(),
        Problem::Full => s.full.to_string(),
        Problem::BadLink => s.bad_link.to_string(),
        Problem::Unreachable => s.unreachable.to_string(),
        Problem::Unsupported => s.unsupported.to_string(),
        Problem::NoRoom => (s.no_room)(MAX_PROFILES),
    }
}

/// The milliseconds since 1970 of the browser's clock; 0 on the server, which reads none.
pub(crate) fn now_ms() -> u64 {
    #[cfg(feature = "csr")]
    {
        web_sys::js_sys::Date::now().max(0.0) as u64
    }
    #[cfg(not(feature = "csr"))]
    0
}

/// Copies a text into the clipboard (a click's: the browser lets it).
pub(crate) fn copy(text: &str) {
    #[cfg(feature = "csr")]
    if let Some(window) = web_sys::window() {
        let _ = window.navigator().clipboard().write_text(text);
    }
    #[cfg(not(feature = "csr"))]
    let _ = text;
}

/// The minutes a link still opens, rounded down; `None` once it opens no more.
fn minutes_left(until: u64, now: u64) -> Option<u64> {
    (until > now).then(|| (until - now) / 60_000)
}

/// The part of the profile page, where the site offers Betula Sync.
#[component]
pub fn SyncPart() -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let sync = Sync::expect();
    let (Some(sync), true) = (sync, folia_stores::sync::offered().is_some()) else { return ().into_any() };
    let profiles = Profiles::expect();
    let id = Memo::new(move |_| profiles.map_or(FIRST, Profiles::active));
    let state = Memo::new(move |_| sync.state(id.get()));
    let open = RwSignal::new(Open::Nothing);
    // The profile „Aktivieren" was pressed for: once it syncs, the part shows the QR code and the
    // link for the next device at once (entwurf.html: „Danach zeigt er QR-Code und Link für das
    // nächste Gerät").
    let activated = RwSignal::new(None::<u16>);

    let explain = move |_| {
        open.set(Open::Explain);
        request_animation_frame(|| nav::focus_by_id(ACTIVATE_YES_ID));
    };
    let activate = move |_| {
        open.set(Open::Nothing);
        let profile = id.get_untracked();
        activated.set(Some(profile));
        sync.activate(profile);
    };
    let cancel = move |back: &'static str| {
        open.set(Open::Nothing);
        request_animation_frame(move || nav::focus_by_id(back));
    };
    let pair = Callback::new(move |()| {
        open.set(Open::Pair(None));
        sync.pair(id.get_untracked(), move |made| {
            // Still the question of this profile: a link made for another one is not shown.
            if open.try_with_untracked(|now| matches!(now, Open::Pair(None))).unwrap_or(false) {
                open.try_set(Open::Pair(Some(made)));
            }
        });
    });
    // Another profile: what was open was the other one's. A state that changes closes what
    // belonged to the one before (the explanation once it is on, the questions once it is off),
    // and a profile just switched on offers the next device.
    Effect::new(move |before: Option<(u16, bool)>| {
        let now = (id.get(), state.with(State::synced));
        if before.is_some_and(|before| before != now) {
            if now.1 && activated.get_untracked() == Some(now.0) {
                activated.set(None);
                pair.run(());
            } else {
                open.set(Open::Nothing);
            }
        }
        if before.is_some_and(|before| before.0 != now.0) {
            activated.set(None);
        }
        now
    });
    let show_code = move |_| {
        if let Some(code) = sync.recovery_code(id.get_untracked()) {
            open.set(Open::Code(code));
        }
    };

    let status = move || {
        let (dot, said) = match state.get() {
            State::Private => return None,
            State::Starting => ("wait", s.starting.to_string()),
            State::Online { devices, direct, relayed } => ("on", (s.online)(devices, direct, relayed)),
            State::Offline => ("off", s.offline.to_string()),
            State::Ended => ("off", s.ended.to_string()),
            State::Failed(problem) => ("warn", format!("{} {}", s.failed, problem_text(problem, t))),
        };
        Some(view! {
            <p class="pf-sync-state" role="status" data-state=dot>
                <span class="pf-sync-dot" aria-hidden="true"></span>
                <span>{said}</span>
            </p>
        })
    };

    let body = move || match state.get() {
        State::Private | State::Ended | State::Failed(_) => {
            let again = state.with(|state| !matches!(state, State::Private));
            let label = if state.with(|state| matches!(state, State::Failed(_))) { s.retry } else if again { s.ended_again } else { s.activate };
            if open.with(|open| *open == Open::Explain) {
                view! {
                    <div class="pf-offer pf-sync-explain" role="group" aria-labelledby="pf-sync-explain">
                        <p id="pf-sync-explain"><b>{s.explain_heading}</b></p>
                        <ul>{s.explain_points.iter().map(|point| view! { <li>{*point}</li> }).collect_view()}</ul>
                        <p>{s.explain_code}</p>
                        <div>
                            <button class="mini primary hit" type="button" id=ACTIVATE_YES_ID on:click=activate>{s.explain_yes}</button>
                            <button class="mini hit" type="button" on:click=move |_| cancel(ACTIVATE_ID)>{s.cancel}</button>
                        </div>
                    </div>
                }
                .into_any()
            } else {
                view! {
                    <div class="pf-sync-actions">
                        <button class="action" type="button" id=ACTIVATE_ID on:click=explain>
                            <Icon name="monitor-smartphone"/><span>{label}<small>{s.activate_hint}</small></span>
                        </button>
                    </div>
                }
                .into_any()
            }
        }
        State::Starting => ().into_any(),
        State::Online { .. } | State::Offline => view! {
            <p class="pf-sub pf-sync-lead">{s.synced_lead}</p>
            <div class="pf-sync-actions">
                <button class="action" type="button" id=PAIR_ID aria-expanded=move || if open.with(|open| matches!(open, Open::Pair(_))) { "true" } else { "false" } on:click=move |_| pair.run(())>
                    <Icon name="smartphone"/><span>{s.pair}<small>{s.pair_hint}</small></span>
                </button>
                <button class="action" type="button" id=CODE_ID aria-expanded=move || if open.with(|open| matches!(open, Open::Code(_))) { "true" } else { "false" } on:click=show_code>
                    <Icon name="lock"/><span>{s.code_show}<small>{s.code_hint}</small></span>
                </button>
                <button class="action" type="button" id=LEAVE_ID on:click=move |_| { open.set(Open::Leave); request_animation_frame(|| nav::focus_by_id(LEAVE_YES_ID)); }>
                    <Icon name="x"/><span>{s.leave}<small>{s.leave_hint}</small></span>
                </button>
                <button class="action" type="button" id=STOP_ID on:click=move |_| { open.set(Open::Stop); request_animation_frame(|| nav::focus_by_id(STOP_YES_ID)); }>
                    <Icon name="trash-2"/><span>{s.stop}<small>{s.stop_hint}</small></span>
                </button>
            </div>
        }
        .into_any(),
    };

    // What is open below the actions of a synced profile.
    let opened = move || match open.get() {
        Open::Nothing | Open::Explain => ().into_any(),
        Open::Pair(None) => view! { <p class="pf-sync-wait" role="status">{s.pairing}</p> }.into_any(),
        Open::Pair(Some(Ok(pairing))) => view! { <PairLink pairing again=pair close=Callback::new(move |()| cancel(PAIR_ID))/> }.into_any(),
        Open::Pair(Some(Err(problem))) => view! {
            <p class="action note-action pf-refused" role="status">
                <Icon name="triangle-alert"/><span>{problem_text(problem, t)}</span>
                <button class="mini hit" type="button" on:click=move |_| pair.run(())>{s.retry}</button>
                <button class="mini hit" type="button" on:click=move |_| cancel(PAIR_ID)>{s.close}</button>
            </p>
        }
        .into_any(),
        Open::Code(code) => view! { <RecoveryCode code close=Callback::new(move |()| cancel(CODE_ID))/> }.into_any(),
        Open::Leave => view! {
            <p class="action note-action ask pf-ask">
                <span>{s.leave_question}</span>
                <button class="mini danger hit" type="button" id=LEAVE_YES_ID on:click=move |_| { open.set(Open::Nothing); sync.leave(id.get_untracked()); }>{s.leave_yes}</button>
                <button class="mini hit" type="button" on:click=move |_| cancel(LEAVE_ID)>{s.cancel}</button>
            </p>
        }
        .into_any(),
        Open::Stop => view! {
            <p class="action note-action ask pf-ask">
                <span>{s.stop_question}</span>
                <button class="mini danger hit" type="button" id=STOP_YES_ID on:click=move |_| { open.set(Open::Nothing); sync.stop(id.get_untracked()); }>{s.stop_yes}</button>
                <button class="mini hit" type="button" on:click=move |_| cancel(STOP_ID)>{s.cancel}</button>
            </p>
        }
        .into_any(),
    };

    view! {
        <section class="panel pf-part pf-sync" aria-labelledby="pf-sync">
            <h2 id="pf-sync"><Icon name="monitor-smartphone"/>{s.heading}</h2>
            {move || state.with(|state| *state == State::Private).then(|| view! { <p class="pf-sub pf-sync-lead">{s.private_lead}</p> })}
            {status}
            {body}
            {opened}
            <Conflicts sync/>
            <FromOther sync/>
        </section>
    }
    .into_any()
}

/// The link for the next device: its QR code, the link to copy, how long it still opens; gone,
/// „Neuer Link".
#[component]
fn PairLink(pairing: Pairing, again: Callback<()>, close: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let now = RwSignal::new(now_ms());
    // The minutes left, counted while it is shown.
    if let Ok(handle) = set_interval_with_handle(move || now.set(now_ms()), std::time::Duration::from_secs(5)) {
        on_cleanup(move || handle.clear());
    }
    let until = pairing.until;
    let left = Memo::new(move |_| minutes_left(until, now.get()));
    let copied = RwSignal::new(false);
    let link = pairing.link.clone();
    let copy_it = move |_| {
        copy(&link);
        copied.set(true);
        set_timeout(move || { let _ = copied.try_set(false); }, std::time::Duration::from_millis(COPIED_MS));
    };
    let code = folia_design::qr::path(&pairing.link);
    let shown = pairing.link.clone();
    move || {
        if left.get().is_none() {
            return view! {
                <p class="action note-action pf-refused" role="status">
                    <Icon name="clock-3"/><span>{s.pair_gone}</span>
                    <button class="mini primary hit" type="button" on:click=move |_| again.run(())>{s.pair_new}</button>
                    <button class="mini hit" type="button" on:click=move |_| close.run(())>{s.close}</button>
                </p>
            }
            .into_any();
        }
        let copy_it = copy_it.clone();
        let qr = code.clone().map(|(path, width)| view! {
            <svg class="pf-qr" viewBox=format!("0 0 {width} {width}") role="img" aria-label=s.qr_label shape-rendering="crispEdges">
                <path d=path/>
            </svg>
        });
        view! {
            <div class="pf-pair">
                {qr}
                <div class="pf-pair-text">
                    <p>{s.pair_how}</p>
                    <p class="pf-pair-link"><code>{shown.clone()}</code></p>
                    <p class="pf-pair-left" role="status">{move || left.get().map(s.pair_left)}</p>
                    <div class="pf-pair-actions">
                        <button class="mini primary hit" type="button" on:click=copy_it>
                            <Icon name=if copied.get() { "check" } else { "copy" }/>
                            <span>{move || if copied.get() { s.copied } else { s.copy_link }}</span>
                        </button>
                        <button class="mini hit" type="button" on:click=move |_| close.run(())>{s.close}</button>
                    </div>
                </div>
            </div>
        }
        .into_any()
    }
}

/// The recovery code shown: what it is, „Code kopieren", „Verbergen".
#[component]
fn RecoveryCode(code: String, close: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let copied = RwSignal::new(false);
    let to_copy = code.clone();
    let copy_it = move |_| {
        copy(&to_copy);
        copied.set(true);
        set_timeout(move || { let _ = copied.try_set(false); }, std::time::Duration::from_millis(COPIED_MS));
    };
    view! {
        <div class="pf-offer pf-sync-code">
            <p>{s.code_lead}</p>
            <p class="pf-code"><code translate="no">{code}</code></p>
            <div>
                <button class="mini primary hit" type="button" on:click=copy_it>
                    {move || view! { <Icon name=if copied.get() { "check" } else { "copy" }/> }}
                    <span>{move || if copied.get() { s.copied } else { s.code_copy }}</span>
                </button>
                <button class="mini hit" type="button" on:click=move |_| close.run(())>{s.code_hide}</button>
            </div>
        </div>
    }
}

/// What two devices did at once in the active profile, a row each with the way to where its hint
/// is; nothing while there is none.
#[component]
fn Conflicts(sync: Sync) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let conflicts = Memo::new(move |_| sync.conflicts());
    // The names of the modules they are about.
    let ids = Memo::new(move |_| {
        let mut ids: Vec<String> = conflicts.with(|all| all.iter().filter_map(|conflict| match conflict {
            Conflict::ModuleNote { module, .. } | Conflict::Moved { module, .. } => Some(module.clone()),
            Conflict::TerminNote { .. } => None,
        }).collect());
        ids.sort();
        ids.dedup();
        ids
    });
    let rows = use_ask(move || {
        let ids = ids.get();
        (!ids.is_empty()).then_some(StudyplanModulesAsk { ids })
    });
    let name = move |id: &str| rows.with(|rows| rows.as_ref().and_then(|rows| rows.as_ref().ok()).and_then(|(rows, _)| rows.iter().find(|row| row.id == id).map(|row| row.title.clone()))).unwrap_or_else(|| id.to_string());
    move || {
        let all = conflicts.get();
        (!all.is_empty()).then(|| {
            let count = all.len();
            view! {
                <div class="pf-sync-conflicts">
                    <p class="label">{(s.conflicts)(count)}</p>
                    <p class="pf-sub">{s.conflicts_hint}</p>
                    <div class="pf-rows">
                        {all.into_iter().rev().map(|conflict| {
                            let (icon, label, href) = match &conflict {
                                Conflict::ModuleNote { module, .. } => ("file-text", (s.conflict_note)(&name(module)), url::module_path(module)),
                                Conflict::TerminNote { row, .. } => ("calendar-clock", s.conflict_termin.to_string(), url::event_path(*row, None)),
                                Conflict::Moved { module, .. } => ("arrow-right-left", (s.conflict_moved)(&name(module)), url::STUDY.to_string()),
                            };
                            view! {
                                <a class="pf-row" href=t.path(&href) rel="nofollow">
                                    <Icon name=icon/>
                                    <span class="pf-row-label">{label}</span>
                                    <span class="pf-row-value">{s.conflict_open}</span>
                                    <Icon name="chevron-right"/>
                                </a>
                            }
                        }).collect_view()}
                    </div>
                </div>
            }
        })
    }
}

/// „Profil von einem anderen Gerät": the recovery code typed in brings the profile as a new one,
/// which becomes the active one; or why it does not.
#[component]
fn FromOther(sync: Sync) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let profiles = Profiles::expect();
    let typed = RwSignal::new(String::new());
    // While it comes; then its name, or why it did not.
    let going = RwSignal::new(false);
    let result = RwSignal::new(None::<Result<String, Problem>>);
    let recover = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let code = typed.get_untracked();
        if code.trim().is_empty() || going.get_untracked() {
            return;
        }
        let Some(profiles) = profiles else { return };
        if !untrack(|| profiles.room()) {
            result.set(Some(Err(Problem::NoRoom)));
            return;
        }
        going.set(true);
        result.set(None);
        sync.recover(code.trim(), "", move |done| {
            going.try_set(false);
            let said = done.map(|id| {
                profiles.switch(id);
                untrack(|| profiles.all()).into_iter().find(|profile| profile.id == id).map(|profile| profile.shown(t.locale)).unwrap_or_default()
            });
            if said.is_ok() {
                typed.try_set(String::new());
            }
            result.try_set(Some(said));
        });
    };
    view! {
        <div class="pf-sync-other">
            <p class="label">{s.other_heading}</p>
            <p class="pf-sub">{s.other_lead}</p>
            <form class="pf-rename pf-recover" on:submit=recover>
                <input
                    id=RECOVER_ID
                    type="text"
                    aria-label=s.code_field
                    placeholder=s.code_placeholder
                    autocomplete="off"
                    autocapitalize="characters"
                    spellcheck="false"
                    translate="no"
                    prop:value=move || typed.get()
                    on:input=move |ev| typed.set(event_target_value(&ev))
                />
                <button class="mini primary hit" type="submit" aria-disabled=move || going.get().then_some("true")>{s.recover}</button>
            </form>
            {move || going.get().then(|| view! { <p class="pf-sync-wait" role="status">{s.starting}</p> })}
            {move || result.get().map(|done| match done {
                Ok(name) => view! {
                    <p class="action note-action" role="status">
                        <Icon name="check"/><span>{(s.recovered)(&name)}</span>
                        <button class="mini hit" type="button" on:click=move |_| result.set(None)>{t.profile.ok}</button>
                    </p>
                }
                .into_any(),
                Err(problem) => view! {
                    <p class="action note-action pf-refused" role="status">
                        <Icon name="triangle-alert"/><span>{problem_text(problem, t)}</span>
                        <button class="mini hit" type="button" on:click=move |_| result.set(None)>{t.profile.ok}</button>
                    </p>
                }
                .into_any(),
            })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_says_how_long_it_opens() {
        let until = 10 * 60_000;
        assert_eq!(minutes_left(until, 0), Some(10));
        assert_eq!(minutes_left(until, 30_000), Some(9));
        assert_eq!(minutes_left(until, until - 1), Some(0));
        assert_eq!((minutes_left(until, until), minutes_left(until, until + 1)), (None, None));
        for t in [&i18n::DE, &i18n::EN] {
            assert!((t.sync.pair_left)(0).contains(if std::ptr::eq(t, &i18n::DE) { "weniger als eine Minute" } else { "less than a minute" }));
        }
    }

    #[test]
    fn every_problem_says_what_to_do() {
        for t in [&i18n::DE, &i18n::EN] {
            for problem in [Problem::Budget, Problem::SlowDown, Problem::Full, Problem::BadLink, Problem::Unreachable, Problem::Unsupported, Problem::NoRoom] {
                let said = problem_text(problem, t);
                assert!(said.len() > 40 && said.ends_with('.'), "{problem:?}: {said}");
            }
            assert!(problem_text(Problem::NoRoom, t).contains(&MAX_PROFILES.to_string()));
        }
    }
}
