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
use folia_stores::sync::{Conflict, Pairing, Problem, Request, State, Sync};
use folia_widgets::syncpair::{copy, now_ms, PairLink, COPIED_MS};

pub use folia_widgets::syncpair::problem_text;

use crate::confirm::{Confirm, Question};
use crate::i18n;

const CONNECT_ID: &str = "pf-sync-connect";
const ACTIVATE_YES_ID: &str = "pf-sync-yes";
const CODE_ID: &str = "pf-sync-code";
const LEAVE_ID: &str = "pf-sync-leave";
const LEAVE_YES_ID: &str = "pf-sync-leave-yes";
const STOP_ID: &str = "pf-sync-stop";
const STOP_YES_ID: &str = "pf-sync-stop-yes";
const RECOVER_ID: &str = "pf-sync-recover";
const RECOVER_WAY_ID: &str = "pf-sync-code-way";
const GET_ID: &str = "pf-sync-get";
const DIALOG_TITLE_ID: &str = "pf-connect-title";

/// What is open in the part, below its actions.
#[derive(Clone, Debug, PartialEq)]
enum Open {
    Nothing,
    /// The recovery code, shown.
    Code(String),
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

/// Which way the dialog „Gerät verbinden" goes (folia/design/profil/konzepte.html, the second
/// step): this profile to another device, or a profile of another device to this one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// The link and its QR code for the next device (`pair`), after switching on a private profile.
    Give,
    /// A code the other device scans and sends a profile to (`Sync::request`).
    Get,
}

/// The dialog of the profile page and which way it goes, for the parts that open it: Betula Sync,
/// and „Profil hinzufügen" › „Von einem anderen Gerät" in the sidebar.
#[derive(Clone, Copy)]
pub struct ConnectDialog(pub RwSignal<Option<Direction>>);

/// The other devices online of a synced profile; `None` where it is not online.
fn online(state: &State) -> Option<usize> {
    match state {
        State::Online { devices, .. } => Some(*devices),
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
    let dialog = use_context::<ConnectDialog>().map_or_else(|| RwSignal::new(None), |dialog| dialog.0);
    // The recovery code typed in, on a new device.
    let recovering = RwSignal::new(false);
    // „Nur dieses Gerät trennen" and „Sync beenden" ask in a menu at their button (`confirm`).
    let leaving = RwSignal::new(None::<Question>);
    let ask = move |at: &'static str| {
        let (title, hint, yes, icon, yes_id) = if at == LEAVE_ID { (s.leave_question, s.leave_after, s.leave_yes, "x", LEAVE_YES_ID) } else { (s.stop_question, s.stop_after, s.stop_yes, "trash-2", STOP_YES_ID) };
        leaving.set(Some(Question { at: at.to_string(), title: title.to_string(), hint: hint.to_string(), yes: yes.to_string(), icon, yes_id }));
    };
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
            State::Online { devices, direct, relayed } => ("on", (s.online)(devices, direct, relayed)),
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
            <button class="btn primary" type="button" id=CONNECT_ID on:click=move |_| dialog.set(Some(Direction::Give))>
                <Icon name="monitor-smartphone"/><span>{label}</span>
            </button>
        }
    };
    // „Gerät verbinden" on a wide screen opens the dialog; on a phone it leads to the scanner, which
    // shows a code of its own on request (concept, second round: „Am Handy führt „Gerät verbinden“
    // zum Scanner").
    let connect_ways = move || {
        view! {
            <button class="btn primary only-desk" type="button" id=CONNECT_ID on:click=move |_| dialog.set(Some(Direction::Give))>
                <Icon name="monitor-smartphone"/><span>{s.pair}</span>
            </button>
            <a class="btn primary only-phone" id="pf-sync-scan" href=t.path(url::CONNECT) rel="nofollow">
                <Icon name="scan-line"/><span>{s.pair}</span>
            </a>
        }
    };
    let body = move || {
        if alone.get() {
            let c = &t.scan;
            return view! {
                <p class="pf-sub pf-sync-lead only-desk">{s.alone_lead}</p>
                <p class="pf-sub pf-sync-lead only-phone">{s.alone_phone}</p>
                <div class="pf-sync-actions">
                    <button class="btn primary only-desk" type="button" id=GET_ID on:click=move |_| dialog.set(Some(Direction::Get))>
                        <Icon name="download"/><span>{c.dir_get}</span>
                    </button>
                    <a class="btn primary only-phone" href=t.path(url::CONNECT) rel="nofollow"><Icon name="scan-line"/><span>{c.scan_way}</span></a>
                    <button class="btn secondary" type="button" id=RECOVER_WAY_ID aria-expanded=move || if recovering.get() { "true" } else { "false" } on:click=move |_| recovering.update(|open| *open = !*open)>
                        <Icon name="lock"/><span>{c.no_phone_way}</span>
                    </button>
                </div>
                {move || recovering.get().then(|| view! { <FromOther sync id=RECOVER_ID/> })}
            }
            .into_any();
        }
        match state.get() {
            State::Private => view! {
                <p class="pf-sub pf-sync-lead">{s.private_lead}</p>
                <div class="pf-sync-actions">{connect_ways()}</div>
            }
            .into_any(),
            State::Ended => view! { <div class="pf-sync-actions">{connect_button(s.ended_again)}</div> }.into_any(),
            State::Failed(_) => view! { <div class="pf-sync-actions">{connect_button(s.retry)}</div> }.into_any(),
            State::Starting => ().into_any(),
            State::Online { .. } | State::Offline => view! {
                <DeviceList sync id/>
                <p class="pf-sub pf-sync-lead">{s.synced_lead}</p>
                <div class="pf-sync-actions">
                    {connect_ways()}
                    <button class="btn secondary" type="button" id=CODE_ID aria-expanded=move || if open.with(|open| matches!(open, Open::Code(_))) { "true" } else { "false" } on:click=show_code>
                        <Icon name="lock"/><span>{s.code_show}</span>
                    </button>
                </div>
                <p class="pf-sync-quiet">
                    <button class="linkish" type="button" id=LEAVE_ID aria-haspopup="dialog" on:click=move |_| ask(LEAVE_ID)>{s.leave}</button>
                    <button class="linkish" type="button" id=STOP_ID aria-haspopup="dialog" on:click=move |_| ask(STOP_ID)>{s.stop}</button>
                </p>
            }
            .into_any(),
        }
    };

    // What is open below the actions of a synced profile.
    let opened = move || match open.get() {
        Open::Nothing => ().into_any(),
        Open::Code(code) => view! { <RecoveryCode code close=Callback::new(move |()| cancel(CODE_ID))/> }.into_any(),
    };

    view! {
        <section class="panel pf-part pf-sync" id=url::DEVICES aria-labelledby="pf-sync">
            <h2 id="pf-sync"><Icon name="monitor-smartphone"/>{s.heading}</h2>
            {status}
            {body}
            {opened}
            <Confirm asking=leaving on_yes=Callback::new(move |yes: &'static str| match yes {
                LEAVE_YES_ID => sync.leave(id.get_untracked()),
                STOP_YES_ID => sync.stop(id.get_untracked()),
                _ => {}
            })/>
            <Conflicts sync/>
            <Connect sync id state dialog/>
        </section>
    }
    .into_any()
}

/// The devices of a synced profile (folia/design/profil/konzepte.html, the third step;
/// `folia_stores::sync::devices`): this one first („jetzt"), then those online (directly, relayed,
/// or through Betula), then by when they were last seen; a device online that wrote nothing of
/// itself as one more. „Entfernen" asks first in a menu at its button (`confirm`), then the device
/// stands in no list any more and leaves the next time it is online, with its copy.
#[component]
fn DeviceList(sync: Sync, id: Memo<u16>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let devices = Memo::new(move |_| sync.devices(id.get()));
    // „vor 2 Std." moves on while the page stands.
    let now = RwSignal::new(now_ms());
    if let Ok(handle) = set_interval_with_handle(move || now.set(now_ms()), std::time::Duration::from_secs(30)) {
        on_cleanup(move || handle.clear());
    }
    // „Entfernen" asks in a menu at its button (`confirm`); the device it asks about: its record's
    // key.
    let asking = RwSignal::new(None::<Question>);
    let removing = StoredValue::new(None::<String>);
    let row = move |(at, device): (usize, folia_stores::sync::devices::Device)| {
        use folia_stores::sync::devices::Link;
        let shown = device.shown(t.locale);
        let line = device.line(t.locale);
        let (state, when) = match (device.mine, device.online) {
            (true, _) => ("now", Signal::derive(move || s.device_now.to_string())),
            (false, Some(Link::Direct)) => ("on", Signal::derive(move || s.device_direct.to_string())),
            (false, Some(Link::Relayed)) => ("on", Signal::derive(move || s.device_relayed.to_string())),
            (false, Some(Link::Phloem)) => ("on", Signal::derive(move || s.device_online.to_string())),
            (false, None) => {
                let seen = device.seen;
                ("off", Signal::derive(move || if seen == 0 { String::new() } else { (s.device_seen)(now.get().saturating_sub(seen)) }))
            }
        };
        let key = device.key.clone();
        let button = format!("pf-device-remove-{at}");
        let asked = {
            let button = button.clone();
            Memo::new(move |_| asking.with(|asking| asking.as_ref().is_some_and(|q| q.at == button)))
        };
        let remove = (!device.mine).then(|| {
            let shown = shown.clone();
            view! {
                <button class="linkish pf-device-remove" type="button" id=button.clone() aria-label=(s.device_remove_label)(&shown) aria-haspopup="dialog" aria-expanded=move || if asked.get() { "true" } else { "false" } on:click=move |_| {
                    removing.set_value(Some(key.clone()));
                    asking.set(Some(Question {
                        at: button.clone(),
                        title: (s.device_remove_question)(&shown),
                        hint: s.device_remove_hint.to_string(),
                        yes: s.device_remove_yes.to_string(),
                        icon: "x",
                        yes_id: "pf-device-remove-yes",
                    }));
                }>{s.device_remove}</button>
            }
        });
        view! {
            <li class="pf-device" class:mine=device.mine data-key=device.key.clone()>
                <span class="pf-tile" aria-hidden="true"><Icon name=device.icon()/></span>
                <span class="pf-device-text"><b>{shown.clone()}</b><small>{line}</small></span>
                <span class="pf-when" data-state=state>{when}</span>
                {remove}
            </li>
        }
    };
    let remove = Callback::new(move |_: &'static str| {
        if let Some(key) = removing.get_value() {
            sync.remove_device(id.get_untracked(), &key);
        }
    });
    let list = move || {
        let all = devices.get();
        (!all.list.is_empty() || all.unnamed > 0).then(|| {
            let unnamed = all.unnamed;
            view! {
                <ul class="pf-devices" aria-label=s.devices_label>
                    {all.list.into_iter().enumerate().map(row).collect_view()}
                    {(unnamed > 0).then(|| view! {
                        <li class="pf-device">
                            <span class="pf-tile" aria-hidden="true"><Icon name="monitor-smartphone"/></span>
                            <span class="pf-device-text"><b>{(s.devices_unnamed)(unnamed)}</b><small>{s.devices_unnamed_hint}</small></span>
                            <span class="pf-when" data-state="on">{s.device_online}</span>
                        </li>
                    })}
                </ul>
            }
        })
    };
    view! {
        {list}
        <Confirm asking on_yes=remove/>
    }
}

/// „Gerät verbinden", the dialog, either way (`Direction`; `both`: with the choice of it at its
/// head). This profile to another device (`Step`): a private profile is switched on first, then the
/// link for the next device; another device that comes meanwhile is told. A profile of another
/// device to this one: `GetPart`.
#[component]
pub(crate) fn Connect(sync: Sync, id: Memo<u16>, state: Memo<State>, dialog: RwSignal<Option<Direction>>, #[prop(default = true)] both: bool) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let c = &t.scan;
    let direction = Memo::new(move |_| dialog.get());
    // A profile came from the other device: „Fertig" at the foot.
    let got = RwSignal::new(false);
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
    // Opened this way: a synced profile shows its link at once, a private one first what switching
    // on means.
    Effect::new(move |was: Option<Option<Direction>>| {
        let now = direction.get();
        if now != was.flatten() {
            got.set(false);
        }
        if now == Some(Direction::Give) && was.flatten() != Some(Direction::Give) {
            if untrack(|| state.with(State::synced)) {
                pair();
            } else {
                step.set(Step::Explain);
                request_animation_frame(|| nav::focus_by_id(ACTIVATE_YES_ID));
            }
        }
        now
    });
    // Switched on: the link follows; another device online while the link stands: it came.
    Effect::new(move |_| {
        let now = state.get();
        if direction.get_untracked() != Some(Direction::Give) {
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
            dialog.set(None);
        }
        now
    });
    let close = Callback::new(move |()| dialog.set(None));
    let activate = move |_| {
        step.set(Step::Starting);
        sync.activate(id.get_untracked());
    };
    let name = move || profiles_name(id.get_untracked());
    // What the server keeps, each point with its icon: sealed, sizes and times, gone after a while.
    const KEEPS: [&str; 3] = ["lock", "clock-3", "calendar-clock"];
    let give = move || match step.get() {
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
    let checked = |on: bool| if on { "true" } else { "false" };
    // The choice of the way, above what it changes.
    let ways = move || {
        both.then(|| {
            view! {
                <div class="seg pf-dirs" role="radiogroup" aria-label=c.dir_label>
                    <button type="button" role="radio" id="pf-dir-give" aria-checked=move || checked(direction.get() == Some(Direction::Give)) on:click=move |_| dialog.set(Some(Direction::Give))>
                        <Icon name="smartphone"/><span>{move || (c.dir_give)(&profiles_name(id.get()))}</span>
                    </button>
                    <button type="button" role="radio" id="pf-dir-get" aria-checked=move || checked(direction.get() == Some(Direction::Get)) on:click=move |_| dialog.set(Some(Direction::Get))>
                        <Icon name="download"/><span>{c.dir_get}</span>
                    </button>
                </div>
            }
        })
    };
    let body = move || match direction.get() {
        Some(Direction::Give) => give().into_any(),
        Some(Direction::Get) => view! { <GetPart sync got/> }.into_any(),
        None => ().into_any(),
    };
    // The buttons at the dialog's foot, below what scrolls (`folia_design::modal`).
    let foot = move || match (direction.get(), step.get()) {
        // Done, either way: the dialog closes and the start page comes (owner, 2026-11-09: „dass
        // man wieder bei der main page landet, wenn das sync fertig ist und nicht im profil").
        (Some(Direction::Get), _) => got.get().then(|| {
            view! {
                <footer class="st-dlg-foot">
                    <a class="btn primary pf-done" href=t.path(url::HOME) rel="nofollow" on:click=move |_| close.run(())>{c.done}</a>
                </footer>
            }
            .into_any()
        }),
        (_, Step::Explain) => Some(view! {
            <footer class="st-dlg-foot">
                <button class="btn secondary" type="button" on:click=move |_| close.run(())>{s.cancel}</button>
                <button class="btn primary" type="button" id=ACTIVATE_YES_ID on:click=activate>{s.explain_yes}</button>
            </footer>
        }
        .into_any()),
        (_, Step::Joined) => Some(view! {
            <footer class="st-dlg-foot">
                <a class="btn primary pf-done" href=t.path(url::HOME) rel="nofollow" on:click=move |_| close.run(())>{s.done}</a>
            </footer>
        }
        .into_any()),
        _ => None,
    };
    view! {
        <Modal open=Signal::derive(move || direction.get().is_some()) on_close=close size=ModalSize::M class="pf-connect-dlg" labelled_by=DIALOG_TITLE_ID>
            <ModalHead id=DIALOG_TITLE_ID title=s.pair on_close=close/>
            <div class="st-dlg-body pf-connect-body">{ways}{body}</div>
            {foot}
        </Modal>
    }
}

/// What the receiving way of the dialog shows (`Get`).
#[derive(Clone, Debug, PartialEq)]
enum Receiving {
    Making,
    Showing(Request),
    /// The profile came: its name.
    Came(String),
    Failed(Problem),
}

/// „Profil vom Handy holen": a code the phone scans (`Sync::request`), the digits it shows then,
/// how long the code asks; and once a profile came, its name (`got`). The code is put away when
/// the part goes; „Kein Handy zur Hand?" offers the recovery code.
#[component]
pub(crate) fn GetPart(sync: Sync, got: RwSignal<bool>) -> impl IntoView {
    let t = i18n::t();
    let c = &t.scan;
    let profiles = Profiles::expect();
    let step = RwSignal::new(Receiving::Making);
    // The request asking now, to put away.
    let current = StoredValue::new(None::<String>);
    let recovering = RwSignal::new(false);
    let now = RwSignal::new(now_ms());
    if let Ok(handle) = set_interval_with_handle(move || now.set(now_ms()), std::time::Duration::from_secs(5)) {
        on_cleanup(move || handle.clear());
    }
    let start = move || {
        if let Some(old) = current.get_value() {
            sync.drop_request(&old);
        }
        current.set_value(None);
        step.set(Receiving::Making);
        sync.request(
            move |made| match made {
                // The part went while the code was made: it asks for nobody.
                Ok(request) if step.try_get_untracked().is_none() => sync.drop_request(&request.id),
                Ok(request) => {
                    current.set_value(Some(request.id.clone()));
                    now.set(now_ms());
                    step.set(Receiving::Showing(request));
                }
                Err(problem) => {
                    step.try_set(Receiving::Failed(problem));
                }
            },
            move |came| {
                current.try_set_value(None);
                let next = match came {
                    Ok(id) => Receiving::Came(profiles.map(|profiles| {
                        profiles.switch(id);
                        untrack(|| profiles.all()).into_iter().find(|profile| profile.id == id).map(|profile| profile.shown(t.locale)).unwrap_or_default()
                    }).unwrap_or_default()),
                    Err(problem) => Receiving::Failed(problem),
                };
                got.try_set(matches!(next, Receiving::Came(_)));
                step.try_set(next);
            },
        );
    };
    start();
    on_cleanup(move || {
        if let Some(id) = current.try_get_value().flatten() {
            sync.drop_request(&id);
        }
    });
    // Where the profile that comes goes: into the empty only profile, or beside the others.
    let into = move || match profiles.and_then(|profiles| profiles.empty_first()) {
        Some(first) => (c.into_empty)(&profiles_name(first)),
        None => c.into_new.to_string(),
    };
    let showing = move |request: Request| {
        let left = Memo::new(move |_| (request.until > now.get()).then(|| (request.until - now.get()) / 60_000));
        let code = folia_design::qr::path(&request.link);
        let link = request.link.clone();
        let digits = request.digits.clone();
        move || {
            if left.get().is_none() {
                return view! {
                    <p class="action note-action pf-refused" role="status">
                        <Icon name="clock-3"/><span>{c.gone}</span>
                        <button class="mini primary hit" type="button" on:click=move |_| start()>{c.again}</button>
                    </p>
                }
                .into_any();
            }
            let qr = code.clone().map(|(path, width)| view! {
                <svg class="pf-qr" viewBox=format!("0 0 {width} {width}") role="img" aria-label=c.qr_label shape-rendering="crispEdges">
                    <path d=path/>
                </svg>
            });
            view! {
                <div class="pf-pair pf-get" data-link=link.clone()>
                    {qr}
                    <div class="pf-pair-text">
                        <p><b>{c.how_start}</b>{c.how_rest}</p>
                        <p class="pf-sub">{c.how_camera}</p>
                        <p>{c.digits_lead}</p>
                        <crate::give::Digits digits=digits.clone()/>
                        <p class="pf-pair-left" role="status">{move || left.get().map(c.waiting)}</p>
                    </div>
                </div>
                <p class="pf-sub">{into()}</p>
            }
            .into_any()
        }
    };
    let body = move || match step.get() {
        Receiving::Making => view! { <p class="pf-sync-wait" role="status">{c.making}</p> }.into_any(),
        Receiving::Showing(request) => showing(request).into_any(),
        Receiving::Came(name) => view! { <p class="pf-joined" role="status"><Icon name="check"/><span>{(c.came)(&name)}</span></p> }.into_any(),
        Receiving::Failed(problem) => view! {
            <p class="action note-action pf-refused" role="status">
                <Icon name="triangle-alert"/><span>{problem_text(problem)}</span>
                <button class="mini primary hit" type="button" on:click=move |_| start()>{c.again}</button>
            </p>
        }
        .into_any(),
    };
    view! {
        {body}
        {move || (!matches!(step.get(), Receiving::Came(_))).then(|| view! {
            <p class="pf-get-other">
                <span>{c.no_phone}</span>" "
                <button class="linkish" type="button" id="pf-get-code-way" aria-expanded=move || if recovering.get() { "true" } else { "false" } on:click=move |_| recovering.update(|open| *open = !*open)>{c.no_phone_way}</button>
            </p>
            {move || recovering.get().then(|| view! { <FromOther sync id="pf-get-code"/> })}
        })}
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
