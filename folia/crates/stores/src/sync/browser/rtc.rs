//! The live way in the browser (`sync::live`, step 2): an RTCPeerConnection to every other device
//! of the profile online, whose data channel carries each change of this device as the sealed
//! batch Phloem keeps. What comes over it is taken as an entry is, without its number: Phloem
//! hands the same batch on with its number a moment later, and it works once.
//!
//! Of two devices the one of the smaller slot offers and the other answers, so that a connection
//! is never set up from both sides at once. The first attempt takes STUN alone; when it fails (not
//! open within `live::attempt_ms`, or the connection fails) the offering device tries again with TURN,
//! then less and less often (`live::wait_before`); the answering one waits for the next offer. A
//! connection that is open stays until it fails or the other device closes it; one that is not
//! open goes with its device (presence).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::{Rc, Weak};

use js_sys::Reflect;
use leptos::prelude::set_timeout;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Event, MessageEvent, RtcConfiguration, RtcDataChannel, RtcDataChannelInit, RtcDataChannelState, RtcIceCandidateInit, RtcIceServer, RtcPeerConnection, RtcPeerConnectionIceEvent, RtcPeerConnectionState, RtcSdpType,
    RtcSessionDescriptionInit,
};

use folia_sync::codec;

use super::super::codes::{b64, unb64};
use super::super::core::Pending;
use super::super::live::{self, talk_aad, Direct, Ice, Online, Talk};
use super::super::wire::Out;
use super::{aad, open, random, seal, secs, Engine};
use folia_design::nav;

/// One connection to another device.
pub(super) struct Link {
    /// The attempt: a random number of the offering device.
    n: u32,
    /// This device offered (it has the smaller slot).
    offered: bool,
    pc: RtcPeerConnection,
    dc: RtcDataChannel,
    /// The other side's description is set: its candidates go in at once.
    described: Cell<bool>,
    pending: RefCell<Vec<RtcIceCandidateInit>>,
    open: Cell<bool>,
    /// The open connection goes through TURN.
    relayed: Cell<bool>,
    handlers: RefCell<Option<Handlers>>,
}

struct Handlers {
    _ice: Closure<dyn FnMut(RtcPeerConnectionIceEvent)>,
    _state: Closure<dyn FnMut(Event)>,
    _open: Closure<dyn FnMut(Event)>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _close: Closure<dyn FnMut(Event)>,
}

impl Link {
    /// Ends the connection and lets its handlers go (they hold the engine weakly).
    fn shut(&self) {
        self.pc.set_onicecandidate(None);
        self.pc.set_onconnectionstatechange(None);
        self.dc.set_onopen(None);
        self.dc.set_onmessage(None);
        self.dc.set_onclose(None);
        self.dc.close();
        self.pc.close();
        self.handlers.borrow_mut().take();
    }
}

/// The live way's state of the engine.
#[derive(Default)]
pub(super) struct Live {
    links: BTreeMap<u16, Rc<Link>>,
    /// The socket's ICE servers and credential; `None` until `turn` was answered on it.
    ice: Option<Ice>,
    /// `turn` was asked on this socket.
    asked: bool,
    /// Attempts since the last connection that opened, by slot (as the offering device).
    tries: BTreeMap<u16, u32>,
    /// Slots whose next attempt waits for its time.
    waiting: BTreeSet<u16>,
    /// Offers that came before the ICE servers, by slot.
    stashed: BTreeMap<u16, Talk>,
    /// The other devices online, as Phloem said last.
    online: Vec<u16>,
}

fn random_n() -> u32 {
    random::<4>().map_or(1, u32::from_be_bytes)
}

/// Where a profile keeps the slots whose last connection needed TURN (`live::read_relayed`).
pub(super) fn relayed_key(profile: u16) -> String {
    format!("betula.sync.p{profile}.relay")
}

/// The configuration of an attempt: STUN always, TURN with the credential when it takes TURN.
fn configuration(ice: &Ice, relay: bool) -> RtcConfiguration {
    let servers = js_sys::Array::new();
    let stun = ice.stun();
    if !stun.is_empty() {
        let server = RtcIceServer::new();
        server.set_urls(&stun.iter().map(|u| JsValue::from_str(u)).collect::<js_sys::Array>());
        servers.push(&server);
    }
    if relay {
        let server = RtcIceServer::new();
        server.set_urls(&ice.turn().iter().map(|u| JsValue::from_str(u)).collect::<js_sys::Array>());
        server.set_username(&ice.user);
        server.set_credential(&ice.password);
        servers.push(&server);
    }
    let config = RtcConfiguration::new();
    config.set_ice_servers(&servers);
    config
}

/// Whether the connection's selected pair goes through TURN, by its stats: the transport's
/// selected pair (Chromium, Safari), else the pair marked selected (Firefox).
fn relayed_in(report: &JsValue) -> bool {
    let get = |o: &JsValue, k: &str| Reflect::get(o, &JsValue::from_str(k)).ok().filter(|v| !v.is_undefined() && !v.is_null());
    let text = |o: &JsValue, k: &str| get(o, k).and_then(|v| v.as_string()).unwrap_or_default();
    let mut stats: BTreeMap<String, JsValue> = BTreeMap::new();
    if let Ok(Some(entries)) = js_sys::try_iter(report) {
        for entry in entries.flatten() {
            let entry: js_sys::Array = entry.unchecked_into();
            if let Some(id) = entry.get(0).as_string() {
                stats.insert(id, entry.get(1));
            }
        }
    }
    let selected = stats.values().find(|s| text(s, "type") == "transport").map(|t| text(t, "selectedCandidatePairId")).and_then(|id| stats.get(&id).cloned());
    let pair = selected.or_else(|| stats.values().find(|s| text(s, "type") == "candidate-pair" && get(s, "selected").and_then(|v| v.as_bool()) == Some(true)).cloned());
    let Some(pair) = pair else { return false };
    ["localCandidateId", "remoteCandidateId"].iter().any(|side| stats.get(&text(&pair, side)).is_some_and(|c| text(c, "candidateType") == "relay"))
}

impl Engine {
    fn mine(&self) -> u16 {
        self.core.borrow().slot()
    }

    fn current(&self, slot: u16, n: u32) -> Option<Rc<Link>> {
        self.live.borrow().links.get(&slot).filter(|l| l.n == n).cloned()
    }

    /// Phloem said who is online (welcome, presence).
    pub(super) fn online(self: &Rc<Self>, devices: &[u16]) {
        let mine = self.mine();
        let others: Vec<u16> = devices.iter().copied().filter(|d| *d != mine && *d != 0).collect();
        let gone: Vec<u16> = {
            let mut live = self.live.borrow_mut();
            live.online = others.clone();
            live.tries.retain(|slot, _| others.contains(slot));
            live.stashed.retain(|slot, _| others.contains(slot));
            live.links.iter().filter(|(slot, link)| !others.contains(slot) && !link.open.get()).map(|(slot, _)| *slot).collect()
        };
        for slot in gone {
            self.unlink(slot);
        }
        self.connect_live();
        self.status();
    }

    /// What the leader says of the connections now.
    pub(super) fn online_now(&self) -> Online {
        let live = self.live.borrow();
        let open = || live.links.values().filter(|l| l.open.get());
        Online { devices: live.online.len().max(open().count()), direct: open().filter(|l| !l.relayed.get()).count(), relayed: open().filter(|l| l.relayed.get()).count() }
    }

    /// Offers to every device online this one offers to and has no connection with; asks for the
    /// ICE servers first.
    fn connect_live(self: &Rc<Self>) {
        let (known, asked) = {
            let live = self.live.borrow();
            (live.ice.is_some(), live.asked)
        };
        if !known {
            if !asked && !self.live.borrow().online.is_empty() && self.send(&Out::Turn) {
                self.live.borrow_mut().asked = true;
            }
            return;
        }
        let mine = self.mine();
        let todo: Vec<u16> = {
            let live = self.live.borrow();
            live.online.iter().copied().filter(|slot| mine < *slot && !live.links.contains_key(slot) && !live.waiting.contains(slot)).collect()
        };
        for slot in todo {
            self.offer(slot);
        }
    }

    /// The socket's ICE servers came (`turn`).
    pub(super) fn got_ice(self: &Rc<Self>, ice: Ice) {
        let stashed = {
            let mut live = self.live.borrow_mut();
            live.ice = Some(ice);
            std::mem::take(&mut live.stashed)
        };
        for (slot, talk) in stashed {
            if let Talk::Offer { n, sdp, relay } = talk {
                self.answer(slot, n, sdp, relay);
            }
        }
        self.connect_live();
    }

    /// The socket closed: its TURN credential is gone with it; open connections stay.
    pub(super) fn socket_closed(&self) {
        let mut live = self.live.borrow_mut();
        live.ice = None;
        live.asked = false;
        live.stashed.clear();
    }

    /// The engine closes: every connection with it.
    pub(super) fn close_live(&self) {
        let links: Vec<Rc<Link>> = std::mem::take(&mut self.live.borrow_mut().links).into_values().collect();
        for link in links {
            link.shut();
        }
        *self.live.borrow_mut() = Live::default();
    }

    fn unlink(&self, slot: u16) {
        let link = self.live.borrow_mut().links.remove(&slot);
        if let Some(link) = link {
            link.shut();
        }
    }

    /// A connection for attempt `n` with the device of `slot`, in place of any before it.
    fn link(self: &Rc<Self>, slot: u16, n: u32, offered: bool, relay: bool) -> Option<Rc<Link>> {
        self.unlink(slot);
        let ice = self.live.borrow().ice.clone().unwrap_or_default();
        let pc = RtcPeerConnection::new_with_configuration(&configuration(&ice, relay)).ok()?;
        let init = RtcDataChannelInit::new();
        init.set_negotiated(true);
        init.set_id(0);
        init.set_ordered(false);
        let dc = pc.create_data_channel_with_data_channel_dict("betula", &init);
        let weak: Weak<Engine> = Rc::downgrade(self);
        let on = move |f: &dyn Fn(&Rc<Engine>)| {
            if let Some(engine) = weak.upgrade() {
                f(&engine);
            }
        };
        let on_ice = {
            let on = on.clone();
            Closure::<dyn FnMut(RtcPeerConnectionIceEvent)>::new(move |event: RtcPeerConnectionIceEvent| {
                let Some(c) = event.candidate() else { return };
                let talk = Talk::Ice { n, c: c.candidate(), mid: c.sdp_mid(), line: c.sdp_m_line_index() };
                if !c.candidate().is_empty() {
                    on(&|engine| engine.talk_soon(slot, talk.clone()));
                }
            })
        };
        let on_state = {
            let on = on.clone();
            let pc = pc.clone();
            Closure::<dyn FnMut(Event)>::new(move |_: Event| match pc.connection_state() {
                RtcPeerConnectionState::Failed => on(&|engine| engine.failed(slot, n)),
                RtcPeerConnectionState::Disconnected => on(&|engine| {
                    let weak = Rc::downgrade(engine);
                    set_timeout(
                        move || {
                            if let Some(engine) = weak.upgrade() {
                                if engine.current(slot, n).is_some_and(|l| l.pc.connection_state() == RtcPeerConnectionState::Disconnected) {
                                    engine.failed(slot, n);
                                }
                            }
                        },
                        std::time::Duration::from_millis(live::DISCONNECTED_MS),
                    );
                }),
                RtcPeerConnectionState::Connected => on(&|engine| engine.route(slot, n)),
                _ => {}
            })
        };
        let on_open = {
            let on = on.clone();
            Closure::<dyn FnMut(Event)>::new(move |_: Event| on(&|engine| engine.opened(slot, n)))
        };
        let on_message = {
            let on = on.clone();
            Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                if let Some(text) = event.data().as_string() {
                    on(&|engine| engine.direct_in(&text));
                }
            })
        };
        let on_close = Closure::<dyn FnMut(Event)>::new(move |_: Event| on(&|engine| engine.failed(slot, n)));
        pc.set_onicecandidate(Some(on_ice.as_ref().unchecked_ref()));
        pc.set_onconnectionstatechange(Some(on_state.as_ref().unchecked_ref()));
        dc.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        dc.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        dc.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        let link = Rc::new(Link {
            n,
            offered,
            pc,
            dc,
            described: Cell::new(false),
            pending: RefCell::new(Vec::new()),
            open: Cell::new(false),
            relayed: Cell::new(false),
            handlers: RefCell::new(Some(Handlers { _ice: on_ice, _state: on_state, _open: on_open, _message: on_message, _close: on_close })),
        });
        self.live.borrow_mut().links.insert(slot, Rc::clone(&link));
        let weak = Rc::downgrade(self);
        set_timeout(
            move || {
                if let Some(engine) = weak.upgrade() {
                    if engine.current(slot, n).is_some_and(|l| !l.open.get()) {
                        engine.failed(slot, n);
                    }
                }
            },
            std::time::Duration::from_millis(live::attempt_ms(relay)),
        );
        Some(link)
    }

    /// The next attempt with the device of `slot`, as the offering device.
    fn offer(self: &Rc<Self>, slot: u16) {
        let tries = {
            let mut live = self.live.borrow_mut();
            let tries = live.tries.entry(slot).or_insert(0);
            *tries += 1;
            *tries
        };
        let remembered = live::read_relayed(&nav::local_get(&relayed_key(self.profile)).unwrap_or_default(), secs()).contains_key(&slot);
        let relay = live::relay(tries, remembered, &self.live.borrow().ice.clone().unwrap_or_default());
        let n = random_n();
        let Some(link) = self.link(slot, n, true, relay) else { return };
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move {
            let made = async {
                let offer = JsFuture::from(link.pc.create_offer()).await?;
                let sdp = Reflect::get(&offer, &JsValue::from_str("sdp"))?.as_string().unwrap_or_default();
                let desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                desc.set_sdp(&sdp);
                JsFuture::from(link.pc.set_local_description(&desc)).await?;
                Ok::<String, JsValue>(sdp)
            }
            .await;
            match made {
                Ok(sdp) if engine.current(slot, n).is_some() => engine.talk(slot, Talk::Offer { n, sdp, relay }).await,
                Ok(_) => {}
                Err(_) => engine.failed(slot, n),
            }
        });
    }

    /// Answers an offer, as the device of the larger slot.
    fn answer(self: &Rc<Self>, slot: u16, n: u32, sdp: String, relay: bool) {
        let Some(link) = self.link(slot, n, false, relay) else { return };
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move {
            let made = async {
                let desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                desc.set_sdp(&sdp);
                JsFuture::from(link.pc.set_remote_description(&desc)).await?;
                engine.described(&link).await;
                let answer = JsFuture::from(link.pc.create_answer()).await?;
                let sdp = Reflect::get(&answer, &JsValue::from_str("sdp"))?.as_string().unwrap_or_default();
                let desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                desc.set_sdp(&sdp);
                JsFuture::from(link.pc.set_local_description(&desc)).await?;
                Ok::<String, JsValue>(sdp)
            }
            .await;
            match made {
                Ok(sdp) if engine.current(slot, n).is_some() => engine.talk(slot, Talk::Answer { n, sdp }).await,
                Ok(_) => {}
                Err(_) => engine.failed(slot, n),
            }
        });
    }

    /// The other side's description is set: the candidates that waited for it go in.
    async fn described(&self, link: &Link) {
        link.described.set(true);
        let pending = std::mem::take(&mut *link.pending.borrow_mut());
        for c in pending {
            let _ = JsFuture::from(link.pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&c))).await;
        }
    }

    /// A sealed signal of the device of slot `from`.
    pub(super) async fn signal(self: &Rc<Self>, from: u16, data: &str) {
        let mine = self.mine();
        let Some(sealed) = unb64(data) else { return };
        let Ok(plain) = open(&self.keys.live, &talk_aad(&self.keys.space, from, mine), &sealed).await else { return };
        let Some(talk) = Talk::read(&plain) else { return };
        match talk {
            Talk::Offer { n, sdp, relay } if mine > from => {
                if self.live.borrow().ice.is_some() {
                    self.answer(from, n, sdp, relay);
                } else {
                    self.live.borrow_mut().stashed.insert(from, Talk::Offer { n, sdp, relay });
                    self.connect_live();
                }
            }
            Talk::Answer { n, sdp } => {
                let Some(link) = self.current(from, n).filter(|l| l.offered) else { return };
                let desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                desc.set_sdp(&sdp);
                if JsFuture::from(link.pc.set_remote_description(&desc)).await.is_ok() {
                    self.described(&link).await;
                } else {
                    self.failed(from, n);
                }
            }
            Talk::Ice { n, c, mid, line } => {
                let Some(link) = self.current(from, n) else { return };
                let init = RtcIceCandidateInit::new(&c);
                init.set_sdp_mid(mid.as_deref());
                init.set_sdp_m_line_index(line);
                if link.described.get() {
                    let _ = JsFuture::from(link.pc.add_ice_candidate_with_opt_rtc_ice_candidate_init(Some(&init))).await;
                } else {
                    link.pending.borrow_mut().push(init);
                }
            }
            Talk::Bye { n } => {
                if let Some(link) = self.current(from, n) {
                    self.unlink(from);
                    if link.offered {
                        self.retry_link(from);
                    }
                    self.status();
                }
            }
            Talk::Offer { .. } => {} // from a larger slot: it answers, it never offers
        }
    }

    /// Seals a signal for the device of `to` and sends it through Phloem.
    async fn talk(&self, to: u16, talk: Talk) {
        let aad = talk_aad(&self.keys.space, self.mine(), to);
        if let Ok(sealed) = seal(&self.keys.live, &aad, &talk.bytes()).await {
            self.send(&Out::Signal { to, data: b64(&sealed) });
        }
    }

    fn talk_soon(self: &Rc<Self>, to: u16, talk: Talk) {
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move { engine.talk(to, talk).await });
    }

    /// Attempt `n` with the device of `slot` failed, or the connection ended: the other side
    /// hears it, and the offering device tries again in its time.
    fn failed(self: &Rc<Self>, slot: u16, n: u32) {
        let Some(link) = self.current(slot, n) else { return };
        self.unlink(slot);
        self.talk_soon(slot, Talk::Bye { n });
        if link.offered {
            self.retry_link(slot);
        }
        self.status();
    }

    fn retry_link(self: &Rc<Self>, slot: u16) {
        let next = self.live.borrow().tries.get(&slot).copied().unwrap_or(0) + 1;
        if !self.live.borrow_mut().waiting.insert(slot) {
            return;
        }
        let weak = Rc::downgrade(self);
        set_timeout(
            move || {
                if let Some(engine) = weak.upgrade() {
                    engine.live.borrow_mut().waiting.remove(&slot);
                    if !*engine.closed.borrow() {
                        engine.connect_live();
                    }
                }
            },
            std::time::Duration::from_millis(live::wait_before(next)),
        );
    }

    fn opened(self: &Rc<Self>, slot: u16, n: u32) {
        let Some(link) = self.current(slot, n) else { return };
        link.open.set(true);
        self.live.borrow_mut().tries.remove(&slot);
        self.route(slot, n);
        self.status();
    }

    /// Finds out whether the open connection goes through TURN.
    fn route(self: &Rc<Self>, slot: u16, n: u32) {
        let Some(link) = self.current(slot, n) else { return };
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move {
            if let Ok(report) = JsFuture::from(link.pc.get_stats()).await {
                let relayed = relayed_in(&report);
                link.relayed.set(relayed);
                engine.status();
                if link.offered {
                    let key = relayed_key(engine.profile);
                    let mut kept = live::read_relayed(&nav::local_get(&key).unwrap_or_default(), secs());
                    let had = kept.contains_key(&slot);
                    if relayed {
                        kept.insert(slot, secs());
                    } else {
                        kept.remove(&slot);
                    }
                    if relayed || had {
                        nav::local_set(&key, &live::relayed_text(&kept));
                    }
                }
            }
        });
    }

    /// Sends this device's new batches over every open connection (`change`).
    pub(super) fn direct_out(self: &Rc<Self>, batches: Vec<Pending>) {
        let open: Vec<Rc<Link>> = self.live.borrow().links.values().filter(|l| l.open.get() && l.dc.ready_state() == RtcDataChannelState::Open).cloned().collect();
        if open.is_empty() || batches.is_empty() {
            return;
        }
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move {
            let aad = aad(&engine.keys.space, "batch", None);
            for pending in batches {
                let Ok(sealed) = seal(&engine.keys.data, &aad, &pending.batch).await else { continue };
                let text = Direct::Push { data: b64(&sealed) }.text();
                if text.len() > live::MAX_MESSAGE {
                    continue; // through Phloem alone
                }
                for link in &open {
                    let _ = link.dc.send_with_str(&text);
                }
            }
        });
    }

    /// A batch that came over a connection: taken as an entry is, without its number.
    fn direct_in(self: &Rc<Self>, text: &str) {
        let Some(Direct::Push { data }) = Direct::read(text) else { return };
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move {
            let Some(sealed) = unb64(&data) else { return };
            let aad = aad(&engine.keys.space, "batch", None);
            let Ok(plain) = open(&engine.keys.data, &aad, &sealed).await else { return };
            if let Ok(ops) = codec::read_batch(&plain) {
                if engine.core.borrow_mut().apply(&ops) > 0 {
                    engine.later();
                    engine.project();
                }
            }
        });
    }
}
