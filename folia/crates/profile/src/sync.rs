//! „Betula Sync" on the profile page (research/sync/entwurf.html, „Vom privaten Profil zum
//! synchronisierten"; folia/design/profil/konzepte.html, the second round): the active profile's
//! state and what can be done with it, where the site offers Betula Sync
//! (`folia_stores::sync::offered`). The part is `#devices` (`url::DEVICES`), where „Geräte" in the menu
//! of the profiles leads.
//!
//! - **Private**: what it is, and „Gerät verbinden".
//! - **Synced**: online with how many other devices, or offline; „Gerät verbinden",
//!   „Wiederherstellungscode", and quiet at the end „Nur dieses Gerät trennen" and „Sync beenden",
//!   each after a question.
//! - **The only profile, empty and private** (a new device): what to do on the device that has it,
//!   and the recovery code typed in (`FromOther`, also under „Profil hinzufügen").
//! - **Ended** on another device, or failed: why, and the way on.
//! - What two devices did at once in the profile, each with the way to where it is shown (the hint
//!   there decides, `folia_widgets::syncnote`).
//!
//! „Gerät verbinden" is a dialog (`Connect`): a private profile first says what the server then
//! keeps (sealed changes, their sizes and times, random ids; never what is in them) and that a
//! recovery code is offered, not asked for, and „Einschalten und Code zeigen" switches it on; then
//! the QR code and the link for the next device (ten minutes, once), „Link kopieren", „Teilen" where
//! the system can (`folia_widgets::syncpair`, which the start page's setup shows as well), and once
//! another device has come while it is open, that it has.
//!
//! Every action is a button (CLAUDE.md, „No action is a key's alone"). The link and the code are
//! made in the browser and never reach a server; what follows `#` in the link is the key.

use leptos::prelude::*;

use folia_design::modal::{Modal, ModalHead, ModalSize};
use folia_design::nav;
use folia_design::ui::Icon;
use folia_pages::ask::StudyplanModulesAsk;
use folia_data::use_ask;
use folia_routes::url;
use folia_stores::profiles::{Profiles, FIRST};
use folia_stores::sync::{Conflict, Pairing, Problem, State, Sync};
use folia_widgets::syncpair::{copy, PairLink, COPIED_MS};

pub use folia_widgets::syncpair::problem_text;

use crate::i18n;

const CONNECT_ID: &str = "pf-sync-connect";
const ACTIVATE_YES_ID: &str = "pf-sync-yes";
const CODE_ID: &str = "pf-sync-code";
const LEAVE_ID: &str = "pf-sync-leave";
const LEAVE_YES_ID: &str = "pf-sync-leave-yes";
const STOP_ID: &str = "pf-sync-stop";
const STOP_YES_ID: &str = "pf-sync-stop-yes";
const RECOVER_ID: &str = "pf-sync-recover";
const DIALOG_TITLE_ID: &str = "pf-connect-title";

/// What is open in the part, below its actions.
#[derive(Clone, Debug, PartialEq)]
enum Open {
    Nothing,
    /// The recovery code, shown.
    Code(String),
    /// The question before „Nur dieses Gerät trennen" or „Sync beenden".
    Leave,
    Stop,
}

/// What the dialog „Gerät verbinden" shows.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    /// What the server keeps, before a private profile is switched on.
    Explain,
    /// Switched on, the link on its way.
    Starting,
    /// The link for the next device: while it is made, then the link or why there is none.
    Pair(Option<Result<Pairing, Problem>>),
    /// Another device came while the link was shown.
    Joined,
}

/// The other devices online of a synced profile; `None` where it is not online.
fn online(state: &State) -> Option<usize> {
    match state {
        State::Online { devices } => Some(*devices),
        _ => None,
    }
}

/// The part of the profile page, where the site offers Betula Sync; `empty`: whether the active
/// profile keeps nothing.
#[component]
pub fn SyncPart(#[prop(into)] empty: Signal<bool>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let sync = Sync::expect();
    let (Some(sync), true) = (sync, folia_stores::sync::offered().is_some()) else { return ().into_any() };
    let profiles = Profiles::expect();
    let id = Memo::new(move |_| profiles.map_or(FIRST, Profiles::active));
    let state = Memo::new(move |_| sync.state(id.get()));
    // The only profile, empty and private: a new device, which takes a profile rather than gives one.
    let alone = Memo::new(move |_| profiles.is_some_and(|profiles| profiles.count() < 2) && state.with(|state| *state == State::Private) && empty.get());
    let open = RwSignal::new(Open::Nothing);
    let dialog = RwSignal::new(false);
    // Another profile, or a state that changes: what was open belonged to the one before.
    Effect::new(move |before: Option<(u16, bool)>| {
        let now = (id.get(), state.with(State::synced));
        if before.is_some_and(|before| before != now) {
            open.set(Open::Nothing);
        }
        now
    });
    let show_code = move |_| {
        if open.with_untracked(|open| matches!(open, Open::Code(_))) {
            open.set(Open::Nothing);
        } else if let Some(code) = sync.recovery_code(id.get_untracked()) {
            open.set(Open::Code(code));
        }
    };
    let cancel = move |back: &'static str| {
        open.set(Open::Nothing);
        request_animation_frame(move || nav::focus_by_id(back));
    };

    let status = move || {
        let (dot, said) = match state.get() {
            State::Private => return None,
            State::Starting => ("wait", s.starting.to_string()),
            State::Online { devices } => ("on", (s.online)(devices)),
            State::Offline => ("off", s.offline.to_string()),
            State::Ended => ("off", s.ended.to_string()),
            State::Failed(problem) => ("warn", format!("{} {}", s.failed, problem_text(problem))),
        };
        Some(view! {
            <p class="pf-sync-state" role="status" data-state=dot>
                <span class="pf-sync-dot" aria-hidden="true"></span>
                <span>{said}</span>
            </p>
        })
    };

    let connect_button = move |label: &'static str| {
        view! {
            <button class="btn primary" type="button" id=CONNECT_ID on:click=move |_| dialog.set(true)>
                <Icon name="monitor-smartphone"/><span>{label}</span>
            </button>
        }
    };
    let body = move || {
        if alone.get() {
            return view! {
                <p class="pf-sub pf-sync-lead">{s.alone_lead}</p>
                <FromOther sync id=RECOVER_ID/>
            }
            .into_any();
        }
        match state.get() {
            State::Private => view! {
                <p class="pf-sub pf-sync-lead">{s.private_lead}</p>
                <div class="pf-sync-actions">{connect_button(s.pair)}</div>
            }
            .into_any(),
            State::Ended => view! { <div class="pf-sync-actions">{connect_button(s.ended_again)}</div> }.into_any(),
            State::Failed(_) => view! { <div class="pf-sync-actions">{connect_button(s.retry)}</div> }.into_any(),
            State::Starting => ().into_any(),
            State::Online { .. } | State::Offline => view! {
                <p class="pf-sub pf-sync-lead">{s.synced_lead}</p>
                <div class="pf-sync-actions">
                    {connect_button(s.pair)}
                    <button class="btn secondary" type="button" id=CODE_ID aria-expanded=move || if open.with(|open| matches!(open, Open::Code(_))) { "true" } else { "false" } on:click=show_code>
                        <Icon name="lock"/><span>{s.code_show}</span>
                    </button>
                </div>
                <p class="pf-sync-quiet">
                    <button class="linkish" type="button" id=LEAVE_ID on:click=move |_| { open.set(Open::Leave); request_animation_frame(|| nav::focus_by_id(LEAVE_YES_ID)); }>{s.leave}</button>
                    <button class="linkish" type="button" id=STOP_ID on:click=move |_| { open.set(Open::Stop); request_animation_frame(|| nav::focus_by_id(STOP_YES_ID)); }>{s.stop}</button>
                </p>
            }
            .into_any(),
        }
    };

    // What is open below the actions of a synced profile.
    let opened = move || match open.get() {
        Open::Nothing => ().into_any(),
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
        <section class="panel pf-part pf-sync" id=url::DEVICES aria-labelledby="pf-sync">
            <h2 id="pf-sync"><Icon name="monitor-smartphone"/>{s.heading}</h2>
            {status}
            {body}
            {opened}
            <Conflicts sync/>
            <Connect sync id state dialog/>
        </section>
    }
    .into_any()
}

/// „Gerät verbinden", the dialog (`Step`): a private profile is switched on first, then the link for
/// the next device; another device that comes meanwhile is told.
#[component]
fn Connect(sync: Sync, id: Memo<u16>, state: Memo<State>, dialog: RwSignal<bool>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let step = RwSignal::new(Step::Explain);
    // The other devices online when the link was shown: one more is the device that came.
    let before = StoredValue::new(None::<usize>);
    let pair = move || {
        step.set(Step::Pair(None));
        before.set_value(state.with_untracked(online));
        sync.pair(id.get_untracked(), move |made| {
            // Still the question of this profile: a link made for another one is not shown.
            if step.try_with_untracked(|now| matches!(now, Step::Pair(None))).unwrap_or(false) {
                step.try_set(Step::Pair(Some(made)));
            }
        });
    };
    // Opened: a synced profile shows its link at once, a private one first what switching on means.
    Effect::new(move |was: Option<bool>| {
        let open = dialog.get();
        if open && was != Some(true) {
            if untrack(|| state.with(State::synced)) {
                pair();
            } else {
                step.set(Step::Explain);
                request_animation_frame(|| nav::focus_by_id(ACTIVATE_YES_ID));
            }
        }
        open
    });
    // Switched on: the link follows; another device online while the link stands: it came.
    Effect::new(move |_| {
        let now = state.get();
        if !dialog.get_untracked() {
            return;
        }
        match step.get_untracked() {
            Step::Starting if now.synced() => pair(),
            Step::Pair(Some(Ok(_))) if online(&now).zip(before.get_value()).is_some_and(|(now, then)| now > then) => step.set(Step::Joined),
            _ => {}
        }
    });
    // Another profile: the dialog was about the one before.
    Effect::new(move |was: Option<u16>| {
        let now = id.get();
        if was.is_some_and(|was| was != now) {
            dialog.set(false);
        }
        now
    });
    let close = Callback::new(move |()| dialog.set(false));
    let activate = move |_| {
        step.set(Step::Starting);
        sync.activate(id.get_untracked());
    };
    let name = move || profiles_name(id.get_untracked());
    // What the server keeps, each point with its icon: sealed, sizes and times, gone after a while.
    const KEEPS: [&str; 3] = ["lock", "clock-3", "calendar-clock"];
    let body = move || match step.get() {
        Step::Explain => view! {
            <div class="pf-sync-explain">
                <p class="pf-explain-head">{s.explain_heading}</p>
                <ul class="pf-keeps">
                    {s.explain_points.iter().zip(KEEPS).map(|(point, icon)| view! { <li><span class="pf-tile" aria-hidden="true"><Icon name=icon/></span><span>{*point}</span></li> }).collect_view()}
                </ul>
                <p class="pf-explain-code"><Icon name="shield-check"/><span>{s.explain_code}</span></p>
            </div>
        }
        .into_any(),
        Step::Starting | Step::Pair(None) => view! { <p class="pf-sync-wait" role="status">{if matches!(step.get_untracked(), Step::Starting) { s.starting } else { s.pairing }}</p> }.into_any(),
        Step::Pair(Some(Ok(pairing))) => view! { <PairLink pairing again=Callback::new(move |()| pair())/> }.into_any(),
        Step::Pair(Some(Err(problem))) => view! {
            <p class="action note-action pf-refused" role="status">
                <Icon name="triangle-alert"/><span>{problem_text(problem)}</span>
                <button class="mini hit" type="button" on:click=move |_| pair()>{s.retry}</button>
            </p>
        }
        .into_any(),
        Step::Joined => view! { <p class="pf-joined" role="status"><Icon name="check"/><span>{(s.joined)(&name())}</span></p> }.into_any(),
    };
    // The buttons at the dialog's foot, below what scrolls (`folia_design::modal`).
    let foot = move || match step.get() {
        Step::Explain => Some(view! {
            <footer class="st-dlg-foot">
                <button class="btn secondary" type="button" on:click=move |_| close.run(())>{s.cancel}</button>
                <button class="btn primary" type="button" id=ACTIVATE_YES_ID on:click=activate>{s.explain_yes}</button>
            </footer>
        }
        .into_any()),
        Step::Joined => Some(view! {
            <footer class="st-dlg-foot">
                <button class="btn primary" type="button" on:click=move |_| close.run(())>{s.done}</button>
            </footer>
        }
        .into_any()),
        _ => None,
    };
    view! {
        <Modal open=Signal::derive(move || dialog.get()) on_close=close size=ModalSize::M class="pf-connect-dlg" labelled_by=DIALOG_TITLE_ID>
            <ModalHead id=DIALOG_TITLE_ID title=s.pair on_close=close/>
            <div class="st-dlg-body pf-connect-body">{body}</div>
            {foot}
        </Modal>
    }
}

/// The name of profile `id` as it is shown.
fn profiles_name(id: u16) -> String {
    let t = i18n::t();
    Profiles::expect().and_then(|profiles| untrack(|| profiles.all()).into_iter().find(|profile| profile.id == id)).map(|profile| profile.shown(t.locale)).unwrap_or_default()
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
pub(crate) fn FromOther(sync: Sync, id: &'static str) -> impl IntoView {
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
                    id=id
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
                        <Icon name="triangle-alert"/><span>{problem_text(problem)}</span>
                        <button class="mini hit" type="button" on:click=move |_| result.set(None)>{t.profile.ok}</button>
                    </p>
                }
                .into_any(),
            })}
        </div>
    }
}
