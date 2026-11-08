//! The browser half of Betula Sync: WebCrypto for the keys and the seals, Phloem over HTTP and its
//! WebSocket, one tab of the browser that leads (Web Locks), and what a synced profile keeps in
//! localStorage beside its stores.
//!
//! One tab leads: it alone writes a synced profile's records and talks to Phloem, for the active
//! profile. Another tab's change of a store waits under `betula.sync.c.…` until the leader takes
//! it (a storage event, or the next start), in the order it was made. What came from another
//! device the leader writes into the stores' texts, and every tab's stores read them again
//! (`Profiles::wrote`, storage events). A tab that becomes visible takes the lead: the phone in the
//! hand is the one that is live.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, VecDeque};
use std::rc::Rc;

use folia_design::nav;
use folia_sync::{codec, Store};
use js_sys::{Array, Promise, Uint8Array};
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{CloseEvent, CryptoKey, MessageEvent, WebSocket};

use folia_calendar::day::Day;
use folia_calendar::live::{following, LivePart};
use folia_plans::shares::{join_link, split_link, Share, ShareTo};
use folia_plans::studyplan::PlanDoc;

use super::codes::{b64, unb64, KEY};
use super::core::{Core, DEVICE};
use super::share::{fingerprint, missing, Put, ShareProblem, ONCE_TTL};
use super::request;
use super::wire::{self, In, Out};
use super::{Problem, State, Sync};
use crate::profiles::{key_of, Profiles, KEYS};

// ---- localStorage ----

fn held_key(profile: u16) -> String {
    format!("betula.sync.p{profile}.v1")
}

fn records_key(profile: u16) -> String {
    format!("betula.sync.p{profile}.doc")
}

fn waiting_key(profile: u16) -> String {
    format!("betula.sync.p{profile}.out")
}

/// What the leader says of a profile's connection: `online <n>` or `offline`.
fn status_key(profile: u16) -> String {
    format!("betula.sync.p{profile}.status")
}

/// A profile whose space is gone: private again (`State::Ended`) until it is read.
fn ended_key(profile: u16) -> String {
    format!("betula.sync.p{profile}.ended")
}

/// A tab that is not the leader asks it to leave the space for this device (`leave`).
fn leave_key(profile: u16) -> String {
    format!("betula.sync.p{profile}.leave")
}

const CHANGE_PREFIX: &str = "betula.sync.c.";

/// The difference of the server's clock to this browser's, as the leader last heard it.
const OFFSET_KEY: &str = "betula.sync.offset";

pub fn syncs(profile: u16) -> bool {
    nav::local_get(&held_key(profile)).is_some_and(|text| !text.trim().is_empty())
}

fn now() -> u64 {
    js_sys::Date::now() as u64
}

/// The server's time now, as well as this browser knows it.
fn wall() -> u64 {
    let offset: i64 = nav::local_get(OFFSET_KEY).and_then(|o| o.parse().ok()).unwrap_or(0);
    now().saturating_add_signed(offset)
}

fn random<const N: usize>() -> Option<[u8; N]> {
    let mut bytes = [0u8; N];
    web_sys::window()?.crypto().ok()?.get_random_values_with_u8_array(&mut bytes).ok()?;
    Some(bytes)
}

fn batch_id() -> String {
    random::<9>().map(|b| b64(&b)).unwrap_or_else(|| format!("{}", now()))
}

thread_local! {
    /// The tab's own count of the changes it hands to the leader, for their order.
    static COUNT: RefCell<u64> = const { RefCell::new(0) };
    static ENGINE: RefCell<Option<Rc<Engine>>> = const { RefCell::new(None) };
}

fn engine() -> Option<Rc<Engine>> {
    ENGINE.with(|e| e.borrow().clone())
}

// ---- what a store's change does ----

/// A store of a synced profile changed in this tab: into the records at once where this tab leads
/// for the profile, else for the leader to take.
pub fn changed(profile: u16, key: &str, before: &str, after: &str) {
    let Some(store) = Store::of_storage_key(key) else { return };
    if before == after {
        return;
    }
    if let Some(engine) = engine().filter(|e| e.leads(profile)) {
        engine.change(store, before, after, wall());
        return;
    }
    let count = COUNT.with(|c| {
        let mut c = c.borrow_mut();
        *c += 1;
        *c
    });
    let tab = random::<4>().map(|b| b64(&b)).unwrap_or_default();
    let item = format!("{CHANGE_PREFIX}{profile}.{:013}.{count:09}.{tab}", now());
    nav::local_set(&item, &format!("{}\t{}\t{}\t{}", store.storage_key(), wall(), b64(before.as_bytes()), b64(after.as_bytes())));
}

/// A store's text was written: where this tab leads, the records may hold more than it (what
/// came meanwhile), and the store is given that.
pub fn wrote(profile: u16, key: &str) {
    let Some(store) = Store::of_storage_key(key) else { return };
    if let Some(engine) = engine().filter(|e| e.leads(profile)) {
        engine.core.borrow_mut().touched.insert(store);
        engine.project();
    }
}

/// Profile `profile` goes from this device (deleted): it leaves its space where this tab leads it
/// (else its slot stays taken until the space ends), the others keep it.
pub fn forget(profile: u16) {
    if !syncs(profile) {
        return;
    }
    match engine().filter(|e| e.leads(profile)) {
        Some(engine) => engine.leave(),
        None => drop_state(profile),
    }
}

/// „Nur dieses Gerät trennen": where this tab leads, at once; where another tab may lead (the
/// active profile), it asks that one; else its slot stays taken until the space ends.
pub fn leave(profile: u16, active: u16) {
    if !syncs(profile) {
        return;
    }
    match engine().filter(|e| e.leads(profile)) {
        Some(engine) => engine.leave(),
        None if profile == active => nav::local_set(&leave_key(profile), "1"),
        None => drop_state(profile),
    }
}

/// Takes what a synced profile keeps besides its stores out of this browser.
fn drop_state(profile: u16) {
    for key in [held_key(profile), records_key(profile), waiting_key(profile), status_key(profile), leave_key(profile)] {
        nav::local_set(&key, "");
    }
}

/// The state a page shows, from what the leader keeps.
pub fn state_of(profile: u16) -> State {
    if !syncs(profile) {
        return if nav::local_get(&ended_key(profile)).is_some() { State::Ended } else { State::Private };
    }
    match nav::local_get(&status_key(profile)).as_deref().map(|s| s.split_whitespace().collect::<Vec<_>>()) {
        Some(words) if words.first() == Some(&"online") => State::Online { devices: words.get(1).and_then(|n| n.parse().ok()).unwrap_or(0) },
        _ => State::Offline,
    }
}

// ---- WebCrypto ----

/// What the profile key gives (protocol.md §5).
pub struct Keys {
    pub space: [u8; 16],
    pub token: [u8; 32],
    pub data: CryptoKey,
}

fn subtle() -> Option<web_sys::SubtleCrypto> {
    Some(web_sys::window()?.crypto().ok()?.subtle())
}

async fn wait(promise: Result<Promise, JsValue>) -> Result<JsValue, JsValue> {
    JsFuture::from(promise?).await
}

fn bytes_of(value: &JsValue) -> Vec<u8> {
    Uint8Array::new(value).to_vec()
}

fn usages(list: &[&str]) -> JsValue {
    list.iter().map(|u| JsValue::from_str(u)).collect::<Array>().into()
}

async fn aes_key(raw: &[u8]) -> Result<CryptoKey, JsValue> {
    let subtle = subtle().ok_or(JsValue::NULL)?;
    let key = wait(subtle.import_key_with_str("raw", &Uint8Array::from(raw), "AES-GCM", false, &usages(&["encrypt", "decrypt"]))).await?;
    key.dyn_into::<CryptoKey>()
}

pub async fn derive(k: &[u8; KEY]) -> Result<Keys, JsValue> {
    let subtle = subtle().ok_or(JsValue::NULL)?;
    let base = wait(subtle.import_key_with_str("raw", &Uint8Array::from(&k[..]), "HKDF", false, &usages(&["deriveBits"]))).await?.dyn_into::<CryptoKey>()?;
    let bits = |info: &'static str, length: u32| {
        let params = web_sys::HkdfParams::new_with_str("HKDF", "SHA-256", &Uint8Array::from(info.as_bytes()), &Uint8Array::from(&b"betula-sync-v1"[..]));
        subtle.derive_bits_with_object(&params, &base, length)
    };
    let space: [u8; 16] = bytes_of(&wait(bits("space", 128)).await?).try_into().map_err(|_| JsValue::NULL)?;
    let token: [u8; 32] = bytes_of(&wait(bits("token", 256)).await?).try_into().map_err(|_| JsValue::NULL)?;
    let data = aes_key(&bytes_of(&wait(bits("data", 256)).await?)).await?;
    Ok(Keys { space, token, data })
}

pub async fn seal(key: &CryptoKey, aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, JsValue> {
    let subtle = subtle().ok_or(JsValue::NULL)?;
    let nonce = random::<12>().ok_or(JsValue::NULL)?;
    let params = web_sys::AesGcmParams::new("AES-GCM", &Uint8Array::from(&nonce[..]));
    params.set_additional_data(&Uint8Array::from(aad));
    let sealed = bytes_of(&wait(subtle.encrypt_with_object_and_u8_array(&params, key, plain)).await?);
    Ok([&nonce[..], &sealed].concat())
}

pub async fn open(key: &CryptoKey, aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>, JsValue> {
    let subtle = subtle().ok_or(JsValue::NULL)?;
    let (nonce, body) = sealed.split_at_checked(12).ok_or(JsValue::NULL)?;
    let params = web_sys::AesGcmParams::new("AES-GCM", &Uint8Array::from(nonce));
    params.set_additional_data(&Uint8Array::from(aad));
    Ok(bytes_of(&wait(subtle.decrypt_with_object_and_u8_array(&params, key, body)).await?))
}

fn aad(space: &[u8; 16], what: &str, upto: Option<u64>) -> Vec<u8> {
    let mut out = space.to_vec();
    out.extend_from_slice(what.as_bytes());
    if let Some(upto) = upto {
        out.extend_from_slice(&upto.to_be_bytes());
    }
    out
}

// ---- HTTP ----

/// Where Phloem answers: the meta tag's base, absolute.
fn base() -> Option<String> {
    let base = super::offered()?;
    if base.starts_with("http") {
        return Some(base);
    }
    let origin = web_sys::window()?.location().origin().ok()?;
    Some(format!("{origin}{base}"))
}

fn socket_url() -> Option<String> {
    let base = base()?;
    let url = if let Some(rest) = base.strip_prefix("https://") { format!("wss://{rest}") } else { base.replacen("http://", "ws://", 1) };
    Some(format!("{}ws", if url.ends_with('/') { url } else { format!("{url}/") }))
}

fn url(path: &str) -> Option<String> {
    let base = base()?;
    Some(format!("{}{path}", if base.ends_with('/') { base } else { format!("{base}/") }))
}

/// A request to Phloem: its status and body.
async fn call(method: &str, path: &str, headers: &[(&str, String)], body: Option<String>) -> Result<(u16, String), Problem> {
    let window = web_sys::window().ok_or(Problem::Unsupported)?;
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    let h = web_sys::Headers::new().map_err(|_| Problem::Unsupported)?;
    for (name, value) in headers {
        let _ = h.set(name, value);
    }
    if let Some(body) = &body {
        let _ = h.set("Content-Type", "application/json");
        init.set_body(&JsValue::from_str(body));
    }
    init.set_headers(&h);
    let request = web_sys::Request::new_with_str_and_init(&url(path).ok_or(Problem::Unsupported)?, &init).map_err(|_| Problem::Unsupported)?;
    let response = JsFuture::from(window.fetch_with_request(&request)).await.map_err(|_| Problem::Unreachable)?.dyn_into::<web_sys::Response>().map_err(|_| Problem::Unreachable)?;
    let text = match response.text() {
        Ok(promise) => JsFuture::from(promise).await.ok().and_then(|t| t.as_string()).unwrap_or_default(),
        Err(_) => String::new(),
    };
    Ok((response.status(), text))
}

fn auth(keys: &Keys) -> Vec<(&'static str, String)> {
    vec![("Betula-Space", b64(&keys.space)), ("Authorization", format!("Bearer {}", b64(&keys.token)))]
}

fn problem_of(status: u16, body: &str) -> Problem {
    let code = serde_json::from_str::<wire::Failure>(body).map(|f| f.error).unwrap_or_default();
    match (status, code.as_str()) {
        (507, _) | (_, "budget") => Problem::Budget,
        (429, _) | (_, "slow_down") => Problem::SlowDown,
        (_, "full") => Problem::Full,
        (404, _) | (401, _) | (403, _) => Problem::BadLink,
        _ => Problem::Unreachable,
    }
}

// ---- the actions ----

/// „Betula Sync aktivieren": a key, a space, and the profile's records as the first snapshot.
pub async fn activate(profile: u16) -> Result<(), Problem> {
    let key = random::<KEY>().ok_or(Problem::Unsupported)?;
    let device = random::<DEVICE>().ok_or(Problem::Unsupported)?;
    let keys = derive(&key).await.map_err(|_| Problem::Unsupported)?;
    let body = serde_json::to_string(&wire::NewSpace { space: b64(&keys.space), token: b64(&keys.token), device: b64(&device) }).map_err(|_| Problem::Unsupported)?;
    let (status, text) = call("POST", "space", &[], Some(body)).await?;
    if status != 201 {
        return Err(problem_of(status, &text));
    }
    let slot = serde_json::from_str::<wire::Made>(&text).map(|m| m.slot).unwrap_or(1);
    let mut core = Core::new(key, device);
    core.set_slot(slot, batch_id);
    for key in KEYS {
        let Some(store) = Store::of_storage_key(key) else { continue };
        let text = crate::profiles::checked(key, &nav::local_get(&key_of(key, profile)).unwrap_or_default());
        core.change(store, "", &text, wall(), batch_id);
    }
    // The first records go as the first snapshot, not as entries.
    core.outbox.clear();
    core.snapshot_due = true;
    keep(profile, &core);
    nav::local_set(&ended_key(profile), "");
    Ok(())
}

/// Keeps a profile's records, as a new device starts them (joining) or as they are.
fn keep(profile: u16, core: &Core) {
    nav::local_set(&records_key(profile), &core.records());
    nav::local_set(&waiting_key(profile), &core.waiting());
    // The key last: a tab that hears of it finds the rest.
    nav::local_set(&held_key(profile), &core.held());
}

/// The key of a synced profile, as this browser keeps it.
fn key_of_profile(profile: u16) -> Option<[u8; KEY]> {
    let held = nav::local_get(&held_key(profile))?;
    let line = held.lines().find_map(|l| l.strip_prefix("key\t"))?;
    unb64(line)?.try_into().ok()
}

pub fn recovery_code(profile: u16) -> Option<String> {
    Some(super::codes::recovery_code(&key_of_profile(profile)?))
}

/// A parcel for the next device: the link and until when it opens.
pub async fn pair(profile: u16, name: String) -> Result<super::Pairing, Problem> {
    let key = key_of_profile(profile).ok_or(Problem::Unsupported)?;
    let keys = derive(&key).await.map_err(|_| Problem::Unsupported)?;
    let id = random::<16>().ok_or(Problem::Unsupported)?;
    let link_key = random::<32>().ok_or(Problem::Unsupported)?;
    let contents = wire::Contents { v: 1, profiles: vec![wire::Shared { key: b64(&key), name }] };
    let plain = serde_json::to_vec(&contents).map_err(|_| Problem::Unsupported)?;
    let sealed = seal(&aes_key(&link_key).await.map_err(|_| Problem::Unsupported)?, b"pair", &plain).await.map_err(|_| Problem::Unsupported)?;
    let body = serde_json::to_string(&wire::Parcel { id: b64(&id), sealed: b64(&sealed) }).map_err(|_| Problem::Unsupported)?;
    let (status, text) = call("POST", "pair", &auth(&keys), Some(body)).await?;
    if status != 201 {
        return Err(problem_of(status, &text));
    }
    let origin = web_sys::window().and_then(|w| w.location().origin().ok()).unwrap_or_default();
    Ok(super::Pairing { link: format!("{origin}{}#{}", folia_routes::url::CONNECT, b64(&[&id[..], &link_key[..]].concat())), until: now() + 10 * 60 * 1000 })
}

/// The profiles of a parcel fetched (`text`, Phloem's answer) for the link part `bytes` (id ‖ key).
async fn opened(bytes: &[u8], text: &str) -> Result<Vec<([u8; KEY], String)>, Problem> {
    let (_, link_key) = bytes.split_at(16);
    let sealed = serde_json::from_str::<wire::Fetched>(text).ok().and_then(|f| unb64(&f.sealed)).ok_or(Problem::BadLink)?;
    let plain = open(&aes_key(link_key).await.map_err(|_| Problem::Unsupported)?, b"pair", &sealed).await.map_err(|_| Problem::BadLink)?;
    let contents: wire::Contents = serde_json::from_slice(&plain).map_err(|_| Problem::BadLink)?;
    Ok(contents.profiles.into_iter().filter_map(|p| Some((unb64(&p.key)?.try_into().ok()?, p.name))).collect())
}

/// The profiles a link of `pair` hands on.
pub async fn unpack(fragment: &str) -> Result<Vec<([u8; KEY], String)>, Problem> {
    let bytes = unb64(fragment.trim_start_matches('#')).filter(|b| b.len() == 48).ok_or(Problem::BadLink)?;
    let (id, _) = bytes.split_at(16);
    let (status, text) = call("GET", "pair", &[("Betula-Pair", b64(id))], None).await?;
    if status != 200 {
        return Err(if status == 404 { Problem::BadLink } else { problem_of(status, &text) });
    }
    opened(&bytes, &text).await
}

/// A profile of this browser that starts syncing with the key of another device's: its stores
/// come from Phloem.
pub fn start(profile: u16, key: [u8; KEY]) -> Result<(), Problem> {
    let device = random::<DEVICE>().ok_or(Problem::Unsupported)?;
    keep(profile, &Core::new(key, device));
    nav::local_set(&ended_key(profile), "");
    Ok(())
}

/// „Sync beenden": the space goes at once; this device is private again with its copy.
pub async fn stop(profile: u16) -> Result<(), Problem> {
    let key = key_of_profile(profile).ok_or(Problem::Unsupported)?;
    let keys = derive(&key).await.map_err(|_| Problem::Unsupported)?;
    let (status, text) = call("DELETE", "space", &auth(&keys), None).await?;
    if !(200..300).contains(&status) && status != 404 && status != 401 {
        return Err(problem_of(status, &text));
    }
    if let Some(engine) = engine().filter(|e| e.leads(profile)) {
        engine.close();
    }
    drop_state(profile);
    Ok(())
}

// ---- „Profil vom Handy holen": a request, and the profile sent to it (`super::request`) ----

thread_local! {
    /// The private keys of this tab's requests, by their id: a request put away is gone from here,
    /// and its questions to Phloem end.
    static REQUESTS: RefCell<Vec<(String, CryptoKey)>> = const { RefCell::new(Vec::new()) };
}

/// A JavaScript object of `pairs`.
fn object(pairs: &[(&str, &JsValue)]) -> js_sys::Object {
    let object = js_sys::Object::new();
    for (name, value) in pairs {
        let _ = js_sys::Reflect::set(&object, &JsValue::from_str(name), value);
    }
    object
}

/// Waits `ms` milliseconds.
async fn sleep(ms: u64) {
    let promise = Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, i32::try_from(ms).unwrap_or(i32::MAX));
        }
    });
    let _ = JsFuture::from(promise).await;
}

/// An ECDH key pair on P-256 made for one request or one sending: the private key, which never
/// leaves the browser, and the public one as its 65 bytes.
async fn ecdh_pair() -> Result<(CryptoKey, [u8; request::PUBLIC]), JsValue> {
    let subtle = subtle().ok_or(JsValue::NULL)?;
    let algorithm = object(&[("name", &"ECDH".into()), ("namedCurve", &"P-256".into())]);
    let pair = wait(subtle.generate_key_with_object(&algorithm, false, &usages(&["deriveBits"]))).await?;
    let private = js_sys::Reflect::get(&pair, &"privateKey".into())?.dyn_into::<CryptoKey>()?;
    let public = js_sys::Reflect::get(&pair, &"publicKey".into())?.dyn_into::<CryptoKey>()?;
    let raw = bytes_of(&wait(subtle.export_key("raw", &public)).await?);
    Ok((private, raw.try_into().map_err(|_| JsValue::NULL)?))
}

/// The key a request's parcel is sealed with: ECDH of one side's private key and the other's
/// public one, through HKDF (SHA-256, the parcel's id as salt).
async fn request_key(private: &CryptoKey, public: &[u8; request::PUBLIC], id: &[u8; request::ID]) -> Result<CryptoKey, JsValue> {
    let subtle = subtle().ok_or(JsValue::NULL)?;
    let algorithm = object(&[("name", &"ECDH".into()), ("namedCurve", &"P-256".into())]);
    let theirs = wait(subtle.import_key_with_object("raw", &Uint8Array::from(&public[..]), &algorithm, true, &usages(&[]))).await?.dyn_into::<CryptoKey>()?;
    let params = object(&[("name", &"ECDH".into()), ("public", &theirs)]);
    let shared = bytes_of(&wait(subtle.derive_bits_with_object(&params, private, 256)).await?);
    let base = wait(subtle.import_key_with_str("raw", &Uint8Array::from(&shared[..]), "HKDF", false, &usages(&["deriveBits"]))).await?.dyn_into::<CryptoKey>()?;
    let params = web_sys::HkdfParams::new_with_str("HKDF", "SHA-256", &Uint8Array::from(&b"betula-request"[..]), &Uint8Array::from(&id[..]));
    aes_key(&bytes_of(&wait(subtle.derive_bits_with_object(&params, &base, 256)).await?)).await
}

/// The four digits of a request (`request::digits`).
pub async fn request_digits(id: &[u8; request::ID], public: &[u8; request::PUBLIC]) -> Option<String> {
    let subtle = subtle()?;
    let data = [&b"betula-request"[..], &id[..], &public[..]].concat();
    let hash = bytes_of(&wait(subtle.digest_with_str_and_u8_array("SHA-256", &data)).await.ok()?);
    request::digits(&hash)
}

/// A request for a profile: an id and a key pair, the link this device shows, its digits.
pub async fn make_request() -> Result<super::Request, Problem> {
    let id = random::<{ request::ID }>().ok_or(Problem::Unsupported)?;
    let (private, public) = ecdh_pair().await.map_err(|_| Problem::Unsupported)?;
    let digits = request_digits(&id, &public).await.ok_or(Problem::Unsupported)?;
    REQUESTS.with(|requests| requests.borrow_mut().push((b64(&id), private)));
    let origin = web_sys::window().and_then(|w| w.location().origin().ok()).unwrap_or_default();
    Ok(super::Request { id: b64(&id), link: format!("{origin}{}#{}", folia_routes::url::PAIR, request::fragment(&id, &public)), digits, until: now() + request::REQUEST_MS })
}

/// Puts request `id` away: its key goes, and it asks no more.
pub fn drop_request(id: &str) {
    REQUESTS.with(|requests| requests.borrow_mut().retain(|(of, _)| of != id));
}

fn request_private(id: &str) -> Option<CryptoKey> {
    REQUESTS.with(|requests| requests.borrow().iter().find(|(of, _)| of == id).map(|(_, key)| key.clone()))
}

/// Asks Phloem for the parcel of request `id` (made at `made`) until it is there: the profiles it
/// brings, or why it brings none. `None` once the code asks no more (ten minutes) or was put away.
pub async fn await_request(id: String, made: u64) -> Option<Result<Vec<([u8; KEY], String)>, Problem>> {
    let parcel: [u8; request::ID] = unb64(&id)?.try_into().ok()?;
    loop {
        sleep(request::next_poll(now().saturating_sub(made))?).await;
        let private = request_private(&id)?;
        // A question that does not get through (the network, the service away) is asked again.
        let Ok((status, text)) = call("GET", "pair", &[("Betula-Pair", id.clone())], None).await else { continue };
        match status {
            200 => {
                drop_request(&id);
                return Some(open_request(&private, &parcel, &text).await.ok_or(Problem::BadLink));
            }
            404 => {
                let code = serde_json::from_str::<wire::Failure>(&text).map(|f| f.error).unwrap_or_default();
                if !(code.is_empty() || code == "not_found") {
                    // Opened by someone else, or withdrawn.
                    drop_request(&id);
                    return Some(Err(Problem::BadLink));
                }
            }
            // Too many questions from this address: a pause.
            429 => sleep(30_000).await,
            _ => {}
        }
    }
}

/// The profiles of a request's parcel as Phloem hands it: the sender's public key, then what it
/// sealed to it.
async fn open_request(private: &CryptoKey, parcel: &[u8; request::ID], text: &str) -> Option<Vec<([u8; KEY], String)>> {
    let sealed = serde_json::from_str::<wire::Fetched>(text).ok().and_then(|f| unb64(&f.sealed))?;
    let (theirs, sealed) = sealed.split_at_checked(request::PUBLIC)?;
    let key = request_key(private, &theirs.try_into().ok()?, parcel).await.ok()?;
    let plain = open(&key, b"request", sealed).await.ok()?;
    let contents: wire::Contents = serde_json::from_slice(&plain).ok()?;
    Some(contents.profiles.into_iter().filter_map(|p| Some((unb64(&p.key)?.try_into().ok()?, p.name))).collect())
}

/// Sends profile `profile` (synced), named `name`, to the device whose request is `fragment`: the
/// profile's key sealed to the request's key, left at Phloem under the request's id.
pub async fn give(profile: u16, name: String, fragment: &str) -> Result<(), Problem> {
    let (parcel, public) = request::parse(fragment).ok_or(Problem::BadLink)?;
    let key = key_of_profile(profile).ok_or(Problem::Unsupported)?;
    let keys = derive(&key).await.map_err(|_| Problem::Unsupported)?;
    let (private, own) = ecdh_pair().await.map_err(|_| Problem::Unsupported)?;
    let sealing = request_key(&private, &public, &parcel).await.map_err(|_| Problem::BadLink)?;
    let contents = wire::Contents { v: 1, profiles: vec![wire::Shared { key: b64(&key), name }] };
    let plain = serde_json::to_vec(&contents).map_err(|_| Problem::Unsupported)?;
    let sealed = seal(&sealing, b"request", &plain).await.map_err(|_| Problem::Unsupported)?;
    let body = serde_json::to_string(&wire::Parcel { id: b64(&parcel), sealed: b64(&[&own[..], &sealed].concat()) }).map_err(|_| Problem::Unsupported)?;
    let (status, text) = call("POST", "pair", &auth(&keys), Some(body)).await?;
    match status {
        201 => Ok(()),
        // The code was answered already (by this phone, or another).
        409 => Err(Problem::BadLink),
        _ => Err(problem_of(status, &text)),
    }
}

// ---- the leader ----

/// The leader's engine for the active profile, when it syncs.
pub struct Engine {
    profile: u16,
    core: RefCell<Core>,
    keys: Keys,
    socket: RefCell<Option<WebSocket>>,
    /// What came over the socket, to be read in order (opening a seal waits for WebCrypto).
    inbox: RefCell<VecDeque<In>>,
    reading: RefCell<bool>,
    sync: Sync,
    profiles: Profiles,
    /// Tries since the last welcome, for the time to the next one.
    tries: RefCell<u32>,
    open: RefCell<bool>,
    /// The records changed and are not kept yet.
    dirty: RefCell<bool>,
    closed: RefCell<bool>,
    /// What was sent on this connection and waits for `stored` (batches) or `snapshotted`.
    in_flight: RefCell<BTreeSet<String>>,
    snapshot_sent: Cell<bool>,
    /// The socket's handlers, dropped with it.
    handlers: RefCell<Option<Handlers>>,
    /// The shares' parts are being renewed, and once more after that.
    renewing: Cell<bool>,
    renew_again: Cell<bool>,
    /// The parts this connection watches (`watch`).
    watched: RefCell<Vec<String>>,
}

struct Handlers {
    _open: Closure<dyn FnMut()>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _close: Closure<dyn FnMut(CloseEvent)>,
}

/// Phloem's close codes (protocol.md §4) after which a device does not come back on its own: a
/// newer socket of the same device took over (4000), or what it sent was refused (1008: then a
/// reload tries again).
fn final_close(code: u16) -> bool {
    code == 4000 || code == 1008
}

impl Engine {
    fn leads(&self, profile: u16) -> bool {
        self.profile == profile && !*self.closed.borrow()
    }

    fn status(&self, devices: Option<usize>) {
        let text = match devices {
            Some(n) => format!("online {n}"),
            None => "offline".to_string(),
        };
        nav::local_set(&status_key(self.profile), &text);
        self.sync.refresh(self.profile);
    }

    fn keep_waiting(&self) {
        nav::local_set(&waiting_key(self.profile), &self.core.borrow().waiting());
    }

    /// Keeps the records, after the stores were given what they hold: what is kept never holds
    /// more than the stores (a start would take what they lack for a deletion).
    fn keep_records(&self) {
        self.project();
        nav::local_set(&records_key(self.profile), &self.core.borrow().records());
        *self.dirty.borrow_mut() = false;
    }

    /// The records changed: kept soon, not with every keystroke.
    fn later(self: &Rc<Self>) {
        if self.dirty.replace(true) {
            return;
        }
        let engine = Rc::clone(self);
        set_timeout(move || {
            if *engine.dirty.borrow() && !*engine.closed.borrow() {
                engine.keep_records();
            }
        }, std::time::Duration::from_millis(800));
    }

    fn change(self: &Rc<Self>, store: Store, before: &str, after: &str, wall: u64) {
        let made = self.core.borrow_mut().change(store, before, after, wall, batch_id);
        if made {
            self.keep_waiting();
            self.later();
            self.send_waiting();
        }
    }

    /// Takes the changes other tabs left, in the order they were made.
    fn take_changes(self: &Rc<Self>) {
        let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else { return };
        let prefix = format!("{CHANGE_PREFIX}{}.", self.profile);
        let mut items: Vec<String> = (0..storage.length().unwrap_or(0)).filter_map(|i| storage.key(i).ok().flatten()).filter(|k| k.starts_with(&prefix)).collect();
        items.sort();
        for item in items {
            let text = storage.get_item(&item).ok().flatten().unwrap_or_default();
            let _ = storage.remove_item(&item);
            let fields: Vec<&str> = text.split('\t').collect();
            let [store, wall, before, after] = fields.as_slice() else { continue };
            let decoded = |b: &str| unb64(b).and_then(|bytes| String::from_utf8(bytes).ok());
            if let (Some(store), Ok(wall), Some(before), Some(after)) = (Store::of_storage_key(store), wall.parse::<u64>(), decoded(before), decoded(after)) {
                self.change(store, &before, &after, wall);
            }
        }
        self.project();
    }

    /// Gives every store whose records changed their text, where it holds something else.
    fn project(&self) {
        let touched: Vec<Store> = std::mem::take(&mut self.core.borrow_mut().touched).into_iter().collect();
        if touched.contains(&Store::Plan) {
            renew_soon();
        }
        let mut wrote = false;
        for store in touched {
            let text = self.core.borrow().text(store);
            let key = key_of(store.storage_key(), self.profile);
            let now = nav::local_get(&key).unwrap_or_default();
            if crate::profiles::checked(store.storage_key(), &now) != text {
                nav::local_set(&key, &text);
                wrote = true;
            }
        }
        if wrote {
            self.profiles.wrote();
        }
        retire(&self.core.borrow().doc);
        self.sync.conflicts_of(&self.core.borrow().doc);
    }

    /// A store's text that differs from the records when the leader starts (written by a tab or a
    /// build that did not sync): what it holds more is a change of this device.
    fn catch_up(self: &Rc<Self>) {
        for key in KEYS {
            let Some(store) = Store::of_storage_key(key) else { continue };
            let stored = crate::profiles::checked(key, &nav::local_get(&key_of(key, self.profile)).unwrap_or_default());
            let records = crate::profiles::checked(key, &self.core.borrow().text(store));
            if stored != records {
                self.change(store, &records, &stored, wall());
            }
            self.core.borrow_mut().touched.insert(store);
        }
        self.project();
    }

    fn send(&self, out: &Out) -> bool {
        match self.socket.borrow().as_ref() {
            Some(socket) if *self.open.borrow() => socket.send_with_str(&out.text()).is_ok(),
            _ => false,
        }
    }

    /// Seals and sends what waits (after the welcome).
    fn send_waiting(self: &Rc<Self>) {
        if !*self.open.borrow() {
            return;
        }
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move {
            let due = engine.core.borrow().snapshot_due;
            if due && !engine.snapshot_sent.replace(true) {
                engine.send_snapshot().await;
            }
            let waiting: Vec<_> = engine.core.borrow().outbox.iter().filter(|p| !engine.in_flight.borrow().contains(&p.id)).cloned().collect();
            let aad = aad(&engine.keys.space, "batch", None);
            for pending in waiting {
                if !engine.in_flight.borrow_mut().insert(pending.id.clone()) {
                    continue;
                }
                if let Ok(sealed) = seal(&engine.keys.data, &aad, &pending.batch).await {
                    if !engine.send(&Out::Push { id: pending.id.clone(), data: b64(&sealed) }) {
                        engine.in_flight.borrow_mut().remove(&pending.id);
                    }
                }
            }
        });
    }

    async fn send_snapshot(&self) {
        let (upto, plain) = {
            let core = self.core.borrow();
            (core.nr, core.snapshot())
        };
        if let Ok(sealed) = seal(&self.keys.data, &aad(&self.keys.space, "snapshot", Some(upto)), &plain).await {
            self.send(&Out::Snapshot { upto, data: b64(&sealed) });
        }
    }

    fn connect(self: &Rc<Self>) {
        if *self.closed.borrow() || self.socket.borrow().is_some() {
            return;
        }
        let Some(url) = socket_url() else { return };
        let Ok(socket) = WebSocket::new(&url) else {
            self.retry();
            return;
        };
        let engine = Rc::clone(self);
        let on_open = Closure::<dyn FnMut()>::new(move || {
            *engine.open.borrow_mut() = true;
            let core = engine.core.borrow();
            let hello = Out::Hello { space: b64(&engine.keys.space), token: b64(&engine.keys.token), device: b64(&core.device), since: core.nr };
            drop(core);
            engine.send(&hello);
        });
        let engine = Rc::clone(self);
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if let Some(frame) = event.data().as_string().and_then(|t| In::read(&t)) {
                engine.inbox.borrow_mut().push_back(frame);
                engine.read();
            }
        });
        let engine = Rc::clone(self);
        let on_close = Closure::<dyn FnMut(CloseEvent)>::new(move |event: CloseEvent| {
            *engine.open.borrow_mut() = false;
            engine.socket.borrow_mut().take();
            engine.in_flight.borrow_mut().clear();
            engine.snapshot_sent.set(false);
            if !*engine.closed.borrow() {
                engine.status(None);
                if !final_close(event.code()) {
                    engine.retry();
                }
            }
        });
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        *self.handlers.borrow_mut() = Some(Handlers { _open: on_open, _message: on_message, _close: on_close });
        *self.socket.borrow_mut() = Some(socket);
    }

    /// The next try: soon, then less often, at most every half minute.
    fn retry(self: &Rc<Self>) {
        let tries = {
            let mut t = self.tries.borrow_mut();
            *t = t.saturating_add(1);
            *t
        };
        let wait = (500u64 << tries.min(6)).min(30_000) + random::<1>().map_or(0, |b| u64::from(b[0]) * 4);
        let engine = Rc::clone(self);
        set_timeout(move || engine.connect(), std::time::Duration::from_millis(wait));
    }

    /// Reads what came, one frame after the other.
    fn read(self: &Rc<Self>) {
        if self.reading.replace(true) {
            return;
        }
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move {
            loop {
                let next = engine.inbox.borrow_mut().pop_front();
                let Some(frame) = next else { break };
                engine.frame(frame).await;
            }
            *engine.reading.borrow_mut() = false;
            engine.project();
        });
    }

    async fn entry(&self, nr: u64, data: &str) {
        let Some(sealed) = unb64(data) else { return };
        let aad = aad(&self.keys.space, "batch", None);
        match open(&self.keys.data, &aad, &sealed).await.ok().map(|plain| codec::read_batch(&plain)) {
            Some(Ok(ops)) => {
                self.core.borrow_mut().apply(&ops);
            }
            // A batch of a later build: read once the page has it (a reload).
            Some(Err(codec::Error::Version(_))) => return,
            _ => {}
        }
        self.core.borrow_mut().hold(nr);
    }

    async fn frame(self: &Rc<Self>, frame: In) {
        match frame {
            In::Welcome { slot, head, now: server, devices, snapshot, entries } => {
                *self.tries.borrow_mut() = 0;
                if server > 0 {
                    nav::local_set(OFFSET_KEY, &(server as i64 - now() as i64).to_string());
                }
                if let Some(snapshot) = snapshot {
                    let aad = aad(&self.keys.space, "snapshot", Some(snapshot.upto));
                    if let Some(plain) = match unb64(&snapshot.data) {
                        Some(sealed) => open(&self.keys.data, &aad, &sealed).await.ok(),
                        None => None,
                    } {
                        let _ = self.core.borrow_mut().join(snapshot.upto, &plain);
                    }
                }
                for entry in &entries {
                    self.entry(entry.nr, &entry.data).await;
                }
                {
                    let mut core = self.core.borrow_mut();
                    core.held_up_to(head);
                    let had = core.slot();
                    core.set_slot(slot, batch_id);
                    if had != slot {
                        nav::local_set(&held_key(self.profile), &core.held());
                    }
                }
                folia_plans::ids::set_slot(slot);
                self.project();
                self.keep_waiting();
                self.keep_records();
                self.status(Some(devices.iter().filter(|d| **d != slot).count()));
                self.send_waiting();
                self.watched.borrow_mut().clear();
                self.watch();
                renew_soon();
                self.reconcile_parcels();
            }
            In::Stored { id, nr } => {
                self.in_flight.borrow_mut().remove(&id);
                self.core.borrow_mut().stored(&id, nr);
                self.keep_waiting();
                self.later();
            }
            In::Entry { nr, data, .. } => {
                self.entry(nr, &data).await;
                self.later();
            }
            In::Snapshotted { .. } => {
                self.snapshot_sent.set(false);
                self.core.borrow_mut().snapshot_due = false;
                self.keep_waiting();
            }
            In::Presence { devices } => {
                let slot = self.core.borrow().slot();
                self.status(Some(devices.iter().filter(|d| **d != slot).count()));
            }
            In::Compact => {
                self.core.borrow_mut().snapshot_due = true;
                self.send_waiting();
            }
            In::Error { code, id } => match code.as_str() {
                "unauthorized" => self.ended(),
                // A batch too large: the snapshot carries it. (The snapshot itself, above 6 MiB,
                // which the stores' caps keep a profile below, is not sent again on this
                // connection.)
                "too_big" => {
                    if let Some(id) = id {
                        self.in_flight.borrow_mut().remove(&id);
                        self.core.borrow_mut().outbox.retain(|p| p.id != id);
                        self.core.borrow_mut().snapshot_due = true;
                        self.keep_waiting();
                        self.send_waiting();
                    }
                }
                "compact_first" => {
                    self.core.borrow_mut().snapshot_due = true;
                    self.in_flight.borrow_mut().clear();
                    self.send_waiting();
                }
                "slow_down" => {
                    self.in_flight.borrow_mut().clear();
                    let engine = Rc::clone(self);
                    set_timeout(move || engine.send_waiting(), std::time::Duration::from_secs(2));
                }
                "full" => {
                    self.sync.fail(self.profile, Problem::Full);
                    self.close();
                }
                _ => {}
            },
            In::Gone => self.ended(),
            In::Watching { parts } => {
                let revs = follow_revs();
                for part in parts {
                    if part.gone || part.rev != revs.get(&part.id).copied().unwrap_or(0) {
                        self.refresh_follow(&part.id).await;
                    }
                }
            }
            In::Part { id, .. } => self.refresh_follow(&id).await,
            In::Pair { .. } => self.reconcile_parcels(),
            In::Other => {}
        }
    }

    /// The space is gone: private again, with all it holds.
    fn ended(self: &Rc<Self>) {
        self.close();
        drop_state(self.profile);
        nav::local_set(&ended_key(self.profile), "1");
        self.sync.refresh(self.profile);
    }

    /// „Nur dieses Gerät trennen": what waits goes out first, then the slot is free.
    fn leave(self: &Rc<Self>) {
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move {
            let waiting = engine.core.borrow().outbox.clone();
            let aad = aad(&engine.keys.space, "batch", None);
            for pending in waiting {
                if let Ok(sealed) = seal(&engine.keys.data, &aad, &pending.batch).await {
                    engine.send(&Out::Push { id: pending.id.clone(), data: b64(&sealed) });
                }
            }
            engine.send(&Out::Leave);
            engine.close();
            drop_state(engine.profile);
            engine.sync.refresh(engine.profile);
        });
    }

    /// Lets the socket go (the network is gone); the engine tries again as after any close.
    fn disconnect(&self) {
        if let Some(socket) = self.socket.borrow().as_ref() {
            let _ = socket.close();
        }
        self.status(None);
    }

    fn close(&self) {
        if self.closed.replace(true) {
            return;
        }
        if *self.dirty.borrow() {
            self.keep_records();
        }
        if let Some(socket) = self.socket.borrow_mut().take() {
            socket.set_onopen(None);
            socket.set_onmessage(None);
            socket.set_onclose(None);
            let _ = socket.close();
        }
        self.handlers.borrow_mut().take();
    }
}

// ---- shares (step 3) ----

/// What this device put of each share of a profile (`share::Put`).
fn parts_key(profile: u16) -> String {
    format!("betula.sync.p{profile}.parts")
}

/// The version of each part followed that this browser holds, by the part's id.
const FOLLOWED_KEY: &str = "betula.follow.revs";

fn follow_revs() -> std::collections::BTreeMap<String, u64> {
    nav::local_get(FOLLOWED_KEY).unwrap_or_default().lines().filter_map(|line| line.split_once('\t')).filter_map(|(id, rev)| Some((id.to_string(), rev.parse().ok()?))).collect()
}

fn set_follow_rev(id: &str, rev: Option<u64>) {
    let mut revs = follow_revs();
    match rev {
        Some(rev) => revs.insert(id.to_string(), rev),
        None => revs.remove(id),
    };
    // A browser follows a few dozen at most; the oldest ids are not ordered, so a cap keeps it small.
    while revs.len() > 128 {
        let first = revs.keys().next().cloned();
        if let Some(first) = first {
            revs.remove(&first);
        }
    }
    nav::local_set(FOLLOWED_KEY, &revs.iter().map(|(id, rev)| format!("{id}\t{rev}\n")).collect::<String>());
}

fn today() -> Day {
    Day(i32::try_from(wall() / 86_400_000).unwrap_or(i32::MAX))
}

fn secs() -> u64 {
    wall() / 1_000
}

fn part_aad(id: &[u8]) -> Vec<u8> {
    [&b"part"[..], id].concat()
}

/// What a request's answer says, for a share.
fn share_problem(status: u16, body: &str) -> ShareProblem {
    let code = serde_json::from_str::<wire::Failure>(body).map(|f| f.error).unwrap_or_default();
    match (status, code.as_str()) {
        (_, "too_many") => ShareProblem::TooMany,
        (429 | 507, _) => ShareProblem::Busy,
        (401, _) => ShareProblem::NotSynced,
        (409 | 410, _) => ShareProblem::Ended,
        (404, code) => missing(code),
        _ => ShareProblem::Unreachable,
    }
}

fn unreachable(problem: Problem) -> ShareProblem {
    match problem {
        Problem::Unsupported => ShareProblem::Unsupported,
        _ => ShareProblem::Unreachable,
    }
}

/// Seals a part's code with its key and puts it at Phloem: its version.
async fn put_part(keys: &Keys, id: &str, key: &str, code: &str) -> Result<u64, ShareProblem> {
    let (Some(id_bytes), Some(key_bytes)) = (unb64(id), unb64(key)) else { return Err(ShareProblem::Unsupported) };
    let part_key = aes_key(&key_bytes).await.map_err(|_| ShareProblem::Unsupported)?;
    let sealed = seal(&part_key, &part_aad(&id_bytes), code.as_bytes()).await.map_err(|_| ShareProblem::Unsupported)?;
    let body = serde_json::to_string(&wire::PartBody { data: b64(&sealed) }).map_err(|_| ShareProblem::Unsupported)?;
    let mut headers = auth(keys);
    headers.push(("Betula-Part", id.to_string()));
    let (status, text) = call("PUT", "part", &headers, Some(body)).await.map_err(unreachable)?;
    match status {
        200 | 201 => Ok(serde_json::from_str::<wire::PartMade>(&text).map(|m| m.rev).unwrap_or(1)),
        _ => Err(share_problem(status, &text)),
    }
}

/// Deletes a part of the space („Nicht mehr teilen").
async fn delete_part(keys: &Keys, id: &str) -> Result<(), ShareProblem> {
    let mut headers = auth(keys);
    headers.push(("Betula-Part", id.to_string()));
    let (status, text) = call("DELETE", "part", &headers, None).await.map_err(unreachable)?;
    match status {
        200..=299 | 401 => Ok(()),
        _ => Err(share_problem(status, &text)),
    }
}

/// Takes a waiting parcel back („Zurückziehen").
async fn withdraw(keys: &Keys, id: &str) -> Result<(), ShareProblem> {
    let mut headers = auth(keys);
    headers.push(("Betula-Pair", id.to_string()));
    let (status, text) = call("DELETE", "pair", &headers, None).await.map_err(unreachable)?;
    match status {
        200..=299 | 401 => Ok(()),
        _ => Err(share_problem(status, &text)),
    }
}

/// A part, opened with its key, when it is newer than version `rev` (0: any); `None` when it is
/// that version still.
pub async fn fetch_part(link: &str, rev: u64) -> Result<Option<(LivePart, u64)>, ShareProblem> {
    let (id, key) = split_link(link).ok_or(ShareProblem::Unknown)?;
    let mut headers = vec![("Betula-Part", id.clone())];
    if rev > 0 {
        headers.push(("If-None-Match", format!("\"{rev}\"")));
    }
    let (status, text) = call("GET", "part", &headers, None).await.map_err(unreachable)?;
    match status {
        304 => Ok(None),
        200 => {
            let fetched: wire::PartFetched = serde_json::from_str(&text).map_err(|_| ShareProblem::Unknown)?;
            let sealed = unb64(&fetched.data).ok_or(ShareProblem::Unknown)?;
            let (Some(id_bytes), Some(key_bytes)) = (unb64(&id), unb64(&key)) else { return Err(ShareProblem::Unknown) };
            let part_key = aes_key(&key_bytes).await.map_err(|_| ShareProblem::Unsupported)?;
            let plain = open(&part_key, &part_aad(&id_bytes), &sealed).await.map_err(|_| ShareProblem::Unknown)?;
            let code = String::from_utf8(plain).map_err(|_| ShareProblem::Unknown)?;
            let part = LivePart::from_code(&code).ok_or(ShareProblem::Unknown)?;
            Ok(Some((part, fetched.rev)))
        }
        _ => Err(share_problem(status, &text)),
    }
}

/// What a new share made: its record, the path of its link, until when a one-time link opens, and
/// how many notes or Termine of one's own did not fit in its part.
#[derive(Clone, Debug)]
pub struct Made {
    pub share: Share,
    pub path: String,
    pub until: Option<u64>,
    pub left_out: usize,
}

/// A new share of profile `profile` (synced) as `template` says, its part holding `part`: the
/// part put at Phloem where it stays current, the parcel where it is for one person. The record
/// is the caller's to add to the plan.
pub async fn share(profile: u16, template: Share, part: LivePart, left_out: usize) -> Result<Made, ShareProblem> {
    let profile_key = key_of_profile(profile).ok_or(ShareProblem::NotSynced)?;
    let keys = derive(&profile_key).await.map_err(|_| ShareProblem::Unsupported)?;
    let code = part.code().map_err(|_| ShareProblem::Unsupported)?;
    let mut share = Share { made: secs(), ..template };
    if share.live {
        let id = b64(&random::<16>().ok_or(ShareProblem::Unsupported)?);
        let key = b64(&random::<32>().ok_or(ShareProblem::Unsupported)?);
        let rev = put_part(&keys, &id, &key, &code).await?;
        let mut put = Put::read(&nav::local_get(&parts_key(profile)).unwrap_or_default());
        put.parts.insert(id.clone(), (fingerprint(&code), rev));
        nav::local_set(&parts_key(profile), &put.text());
        share.id = id;
        share.key = key;
    }
    let mut until = None;
    if share.once {
        let parcel_id = random::<16>().ok_or(ShareProblem::Unsupported)?;
        let link_key = random::<32>().ok_or(ShareProblem::Unsupported)?;
        let contents = if share.live { wire::ShareContents { v: 1, part: Some(share.id.clone()), key: Some(share.key.clone()), copy: None } } else { wire::ShareContents { v: 1, part: None, key: None, copy: Some(code.clone()) } };
        let plain = serde_json::to_vec(&contents).map_err(|_| ShareProblem::Unsupported)?;
        let sealed = seal(&aes_key(&link_key).await.map_err(|_| ShareProblem::Unsupported)?, b"share", &plain).await.map_err(|_| ShareProblem::Unsupported)?;
        let body = serde_json::to_string(&wire::LongParcel { id: b64(&parcel_id), sealed: b64(&sealed), ttl: ONCE_TTL }).map_err(|_| ShareProblem::Unsupported)?;
        let answer = call("POST", "pair", &auth(&keys), Some(body)).await.map_err(unreachable);
        let failed = match answer {
            Ok((201, _)) => None,
            Ok((status, text)) => Some(share_problem(status, &text)),
            Err(problem) => Some(problem),
        };
        if let Some(problem) = failed {
            if share.live {
                let _ = delete_part(&keys, &share.id).await;
            }
            return Err(problem);
        }
        share.parcel = b64(&[&parcel_id[..], &link_key[..]].concat());
        if !share.live {
            share.id = b64(&parcel_id);
        }
        until = Some(now() + ONCE_TTL * 1_000);
    }
    let path = if share.once {
        super::share::once_path(&share.parcel)
    } else {
        let link = share.link().ok_or(ShareProblem::Unsupported)?;
        match share.to {
            ShareTo::App => folia_calendar::live::feed_path(&link),
            ShareTo::Person => super::share::live_path(&link),
        }
    };
    Ok(Made { share, path, until, left_out })
}

/// Ends a share at Phloem: its part is deleted, its waiting parcel taken back. The record is the
/// caller's to take away.
pub async fn unshare(profile: u16, share: &Share) -> Result<(), ShareProblem> {
    let profile_key = key_of_profile(profile).ok_or(ShareProblem::NotSynced)?;
    let keys = derive(&profile_key).await.map_err(|_| ShareProblem::Unsupported)?;
    if let Some((parcel, _)) = split_link(&share.parcel) {
        withdraw(&keys, &parcel).await?;
    }
    if share.live {
        delete_part(&keys, &share.id).await?;
        let mut put = Put::read(&nav::local_get(&parts_key(profile)).unwrap_or_default());
        put.parts.remove(&share.id);
        put.deleted.insert(share.id.clone());
        put.trim();
        nav::local_set(&parts_key(profile), &put.text());
    }
    Ok(())
}

/// What a one-time link opened to.
#[derive(Clone, Debug)]
pub enum Opened {
    /// A calendar that stays current: its link (part id and key), what it holds now and its
    /// version.
    Live { link: String, part: LivePart, rev: u64 },
    /// A copy of then.
    Copy(LivePart),
}

/// Opens a one-time link (`#once=…`): its parcel, once.
pub async fn open_once(parcel: &str) -> Result<Opened, ShareProblem> {
    let (id, key) = split_link(parcel).ok_or(ShareProblem::Unknown)?;
    let (status, text) = call("GET", "pair", &[("Betula-Pair", id)], None).await.map_err(unreachable)?;
    if status != 200 {
        return Err(share_problem(status, &text));
    }
    let sealed = serde_json::from_str::<wire::Fetched>(&text).ok().and_then(|f| unb64(&f.sealed)).ok_or(ShareProblem::Unknown)?;
    let link_key = aes_key(&unb64(&key).ok_or(ShareProblem::Unknown)?).await.map_err(|_| ShareProblem::Unsupported)?;
    let plain = open(&link_key, b"share", &sealed).await.map_err(|_| ShareProblem::Unknown)?;
    let contents: wire::ShareContents = serde_json::from_slice(&plain).map_err(|_| ShareProblem::Unknown)?;
    match (contents.part, contents.key, contents.copy) {
        (Some(part), Some(key), _) => {
            let link = join_link(&part, &key).ok_or(ShareProblem::Unknown)?;
            let (part, rev) = fetch_part(&link, 0).await?.ok_or(ShareProblem::Unknown)?;
            Ok(Opened::Live { link, part, rev })
        }
        (_, _, Some(copy)) => Ok(Opened::Copy(LivePart::from_code(&copy).ok_or(ShareProblem::Unknown)?)),
        _ => Err(ShareProblem::Unknown),
    }
}

/// The parcels of profile `profile` and what became of them.
async fn parcel_states(profile: u16) -> Result<Vec<wire::ParcelState>, ShareProblem> {
    let profile_key = key_of_profile(profile).ok_or(ShareProblem::NotSynced)?;
    let keys = derive(&profile_key).await.map_err(|_| ShareProblem::Unsupported)?;
    let (status, text) = call("GET", "pairs", &auth(&keys), None).await.map_err(unreachable)?;
    if status != 200 {
        return Err(share_problem(status, &text));
    }
    Ok(serde_json::from_str::<wire::Parcels>(&text).map(|p| p.parcels).unwrap_or_default())
}

/// What became of the one-time links of a plan: an opened one leaves a share that stays current
/// (its parcel goes) or nothing (a copy); one that expired or was taken back leaves nothing, and
/// the part of what it would have shared goes. The changes, for `PlanDoc::update_share` and
/// `remove_share`, and the parts to delete.
fn parcel_changes(doc: &PlanDoc, states: &[wire::ParcelState]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let (mut opened, mut gone, mut parts) = (Vec::new(), Vec::new(), Vec::new());
    for share in &doc.shares {
        let Some((parcel, _)) = split_link(&share.parcel) else { continue };
        let Some(state) = states.iter().find(|s| s.id == parcel) else { continue };
        match (state.state.as_str(), share.live) {
            ("taken", true) => opened.push(share.id.clone()),
            ("taken", false) | ("expired" | "withdrawn", false) => gone.push(share.id.clone()),
            ("expired" | "withdrawn", true) => {
                gone.push(share.id.clone());
                parts.push(share.id.clone());
            }
            _ => {}
        }
    }
    (opened, gone, parts)
}

/// Applies `f` to every item and says whether it changed anything for any (each one is applied,
/// unlike `any`, which stops at the first).
fn every<T>(items: &[T], mut f: impl FnMut(&T) -> bool) -> bool {
    let mut changed = false;
    for item in items {
        changed |= f(item);
    }
    changed
}

thread_local! {
    /// The latest call of `renew_soon`: only the last of a burst renews.
    static RENEW: Cell<u64> = const { Cell::new(0) };
}

/// The parts of the active profile's shares are renewed soon: once the plan has been left alone
/// for a moment.
fn renew_soon() {
    let call = RENEW.with(|r| {
        r.set(r.get() + 1);
        r.get()
    });
    set_timeout(
        move || {
            if RENEW.with(Cell::get) == call {
                if let Some(engine) = engine() {
                    engine.renew();
                }
            }
        },
        std::time::Duration::from_millis(1_500),
    );
}

impl Engine {
    /// The plan's text as its store holds it now.
    fn plan_text(&self) -> String {
        let key = Store::Plan.storage_key();
        crate::profiles::checked(key, &nav::local_get(&key_of(key, self.profile)).unwrap_or_default())
    }

    /// Changes the plan as its store would (`f` says whether it changed anything): the text, its
    /// records and every tab's store.
    fn edit_plan(self: &Rc<Self>, f: impl FnOnce(&mut PlanDoc) -> bool) {
        if *self.closed.borrow() {
            return;
        }
        let before = self.plan_text();
        let mut doc = PlanDoc::restored(&before);
        if !f(&mut doc) {
            return;
        }
        let after = doc.stored_without_program();
        if after == before {
            return;
        }
        nav::local_set(&key_of(Store::Plan.storage_key(), self.profile), &after);
        self.change(Store::Plan, &before, &after, wall());
        self.profiles.wrote();
    }

    /// Renews the parts of the shares whose calendar changed since this device put them, moves
    /// a share on to the next semester when it begins, deletes the parts of shares that are gone,
    /// and watches what the plan follows.
    fn renew(self: &Rc<Self>) {
        if self.renewing.replace(true) {
            self.renew_again.set(true);
            return;
        }
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move {
            loop {
                engine.renew_once().await;
                if !engine.renew_again.replace(false) || *engine.closed.borrow() {
                    break;
                }
            }
            engine.renewing.set(false);
        });
    }

    async fn renew_once(self: &Rc<Self>) {
        self.watch();
        if !*self.open.borrow() || *self.closed.borrow() {
            return;
        }
        let doc = PlanDoc::restored(&self.plan_text());
        let day = today();
        let moved: Vec<(String, folia_calendar::semester::SemesterKey)> =
            doc.shares.iter().filter(|s| s.live).filter_map(|s| Some((s.id.clone(), Some(following(s.semester, day)).filter(|now| *now != s.semester)?))).collect();
        if !moved.is_empty() {
            self.edit_plan(|d| every(&moved, |(id, semester)| d.update_share(id, |s| s.semester = *semester)));
        }
        let mut put = Put::read(&nav::local_get(&parts_key(self.profile)).unwrap_or_default());
        let mut ended: Vec<String> = Vec::new();
        for share in doc.shares.iter().filter(|s| s.live) {
            let semester = following(share.semester, day);
            let Ok(code) = doc.live_part(share, semester).fitted().0.code() else { continue };
            let print = fingerprint(&code);
            if put.parts.get(&share.id).is_some_and(|(kept, _)| *kept == print) {
                continue;
            }
            match put_part(&self.keys, &share.id, &share.key, &code).await {
                Ok(rev) => {
                    put.parts.insert(share.id.clone(), (print, rev));
                }
                // Deleted meanwhile (on another device, or with the space): the share is over.
                Err(ShareProblem::Ended) => ended.push(share.id.clone()),
                Err(_) => break,
            }
        }
        if !ended.is_empty() {
            self.edit_plan(|d| every(&ended, |id| d.remove_share(id).is_some()));
        }
        // The shares gone from the records (on any device): their parts and parcels go, once.
        let gone: Vec<String> = self.core.borrow().doc.entries.iter().filter(|(key, e)| key.starts_with("P|s|") && e.val.is_none()).filter_map(|(key, _)| Some(key.strip_prefix("P|s|")?.to_string())).filter(|id| !put.deleted.contains(id)).collect();
        for id in gone {
            if delete_part(&self.keys, &id).await.is_ok() {
                let _ = withdraw(&self.keys, &id).await;
                put.parts.remove(&id);
                put.deleted.insert(id);
            }
        }
        put.trim();
        nav::local_set(&parts_key(self.profile), &put.text());
    }

    /// Tells Phloem the parts the plan follows, where they changed since it last heard.
    fn watch(&self) {
        if !*self.open.borrow() {
            return;
        }
        let doc = PlanDoc::restored(&self.plan_text());
        let mut parts: Vec<String> = doc.follows().into_iter().filter(|(_, f)| f.ended == 0).filter_map(|(_, f)| f.part().map(|(id, _)| id)).collect();
        parts.sort();
        parts.dedup();
        parts.truncate(64);
        let changed = *self.watched.borrow() != parts;
        if changed && self.send(&Out::Watch { parts: parts.clone() }) {
            *self.watched.borrow_mut() = parts;
        }
    }

    /// Fetches a part the plan follows anew and gives its calendar what it holds; a part that is
    /// gone leaves the calendar as it is, not shared any more.
    async fn refresh_follow(self: &Rc<Self>, id: &str) {
        let doc = PlanDoc::restored(&self.plan_text());
        let followers: Vec<(u8, String)> = doc.follows().into_iter().filter(|(_, f)| f.ended == 0 && f.part().is_some_and(|(part, _)| part == id)).map(|(calendar, f)| (calendar, f.link.clone())).collect();
        let Some((_, link)) = followers.first().cloned() else { return };
        let rev = follow_revs().get(id).copied().unwrap_or(0);
        match fetch_part(&link, rev).await {
            Ok(Some((part, rev))) => {
                self.edit_plan(|d| every(&followers, |(calendar, _)| d.take_part(*calendar, &part)));
                set_follow_rev(id, Some(rev));
            }
            Ok(None) => {}
            Err(ShareProblem::Ended) => {
                let at = secs();
                self.edit_plan(|d| every(&followers, |(calendar, _)| d.end_follow(*calendar, at)));
                set_follow_rev(id, None);
                self.watch();
            }
            Err(_) => {}
        }
    }

    /// Asks what became of the plan's one-time links, and changes the shares accordingly.
    fn reconcile_parcels(self: &Rc<Self>) {
        let doc = PlanDoc::restored(&self.plan_text());
        if !doc.shares.iter().any(|s| !s.parcel.is_empty()) {
            return;
        }
        let engine = Rc::clone(self);
        leptos::task::spawn_local(async move {
            let Ok(states) = parcel_states(engine.profile).await else { return };
            let doc = PlanDoc::restored(&engine.plan_text());
            let (opened, gone, parts) = parcel_changes(&doc, &states);
            engine.edit_plan(|d| {
                let a = every(&opened, |id| d.update_share(id, |s| s.parcel.clear()));
                every(&gone, |id| d.remove_share(id).is_some()) || a
            });
            for id in parts {
                let _ = delete_part(&engine.keys, &id).await;
            }
        });
    }
}

/// Follows what the active profile follows where no engine of this tab does it (a private
/// profile): fetches each part anew and gives its calendar what it holds.
pub async fn refresh_follows(profile: u16, plan: crate::studyplan::Studyplan, profiles: Profiles) {
    if let Some(engine) = engine().filter(|e| e.leads(profile)) {
        let ids: Vec<String> = PlanDoc::restored(&engine.plan_text()).follows().into_iter().filter(|(_, f)| f.ended == 0).filter_map(|(_, f)| f.part().map(|(id, _)| id)).collect();
        for id in ids {
            engine.refresh_follow(&id).await;
        }
        return;
    }
    if syncs(profile) {
        // Another tab leads this profile: it follows.
        return;
    }
    let follows: Vec<(u8, String)> = plan.with_untracked(|doc| doc.follows().into_iter().filter(|(_, f)| f.ended == 0).map(|(calendar, f)| (calendar, f.link.clone())).collect());
    for (calendar, link) in follows {
        let Some((id, _)) = split_link(&link) else { continue };
        let rev = follow_revs().get(&id).copied().unwrap_or(0);
        let result = fetch_part(&link, rev).await;
        if profiles.active_untracked() != profile {
            return;
        }
        match result {
            Ok(Some((part, rev))) => {
                plan.update(|doc| doc.take_part(calendar, &part));
                set_follow_rev(&id, Some(rev));
            }
            Err(ShareProblem::Ended) => {
                let at = secs();
                plan.update(|doc| doc.end_follow(calendar, at));
                set_follow_rev(&id, None);
            }
            _ => {}
        }
    }
}

/// Asks the leader to look at what became of the one-time links (the dialog opened).
pub fn check_parcels(profile: u16) {
    if let Some(engine) = engine().filter(|e| e.leads(profile)) {
        engine.reconcile_parcels();
    }
}

/// The numbers the profile had and gave up, which a new calendar, Termin of one's own,
/// placeholder or collection does not take again (`folia_plans::ids`).
fn retire(doc: &folia_sync::Doc) {
    use folia_plans::ids::{set_retired, Kind};
    let retired = folia_sync::retired(doc);
    set_retired(Kind::Calendar, retired.calendars);
    set_retired(Kind::Own, retired.own);
    set_retired(Kind::Placeholder, retired.placeholders);
    set_retired(Kind::Collection, retired.collections);
}

thread_local! {
    /// Counts the starts of `lead`: one that waited for WebCrypto while a later one began gives way.
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

/// Starts the engine for the active profile, where it syncs and this tab leads.
async fn lead(sync: Sync, profiles: Profiles) {
    let profile = profiles.active_untracked();
    if let Some(old) = engine() {
        if old.profile == profile && !*old.closed.borrow() && syncs(profile) {
            return;
        }
        old.close();
    }
    ENGINE.with(|e| e.borrow_mut().take());
    let generation = GENERATION.with(|g| {
        g.set(g.get() + 1);
        g.get()
    });
    if !syncs(profile) {
        folia_plans::ids::set_slot(0);
        folia_plans::ids::clear_retired();
        return;
    }
    let held = nav::local_get(&held_key(profile)).unwrap_or_default();
    let records = nav::local_get(&records_key(profile)).unwrap_or_default();
    let waiting = nav::local_get(&waiting_key(profile)).unwrap_or_default();
    let Some(core) = Core::restore(&held, &records, &waiting) else { return };
    let Ok(keys) = derive(&core.key).await else {
        sync.fail(profile, Problem::Unsupported);
        return;
    };
    if profiles.active_untracked() != profile || !syncs(profile) || GENERATION.with(Cell::get) != generation || !leads() {
        return;
    }
    folia_plans::ids::set_slot(core.slot());
    retire(&core.doc);
    let engine = Rc::new(Engine {
        profile,
        core: RefCell::new(core),
        keys,
        socket: RefCell::new(None),
        inbox: RefCell::new(VecDeque::new()),
        reading: RefCell::new(false),
        sync,
        profiles,
        tries: RefCell::new(0),
        open: RefCell::new(false),
        dirty: RefCell::new(false),
        closed: RefCell::new(false),
        in_flight: RefCell::new(BTreeSet::new()),
        snapshot_sent: Cell::new(false),
        handlers: RefCell::new(None),
        renewing: Cell::new(false),
        renew_again: Cell::new(false),
        watched: RefCell::new(Vec::new()),
    });
    ENGINE.with(|e| *e.borrow_mut() = Some(Rc::clone(&engine)));
    engine.take_changes();
    if nav::local_get(&leave_key(profile)).is_some() {
        engine.leave();
        return;
    }
    engine.status(None);
    engine.catch_up();
    engine.connect();
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["navigator", "locks"], js_name = request, catch)]
    fn request_lock(name: &str, options: &JsValue, callback: &js_sys::Function) -> Result<Promise, JsValue>;
}

thread_local! {
    static LEADS: RefCell<bool> = const { RefCell::new(false) };
}

/// Asks for the lead of this browser's tabs; `steal` takes it from another tab (this one became
/// visible). Without Web Locks the tab leads alone.
fn ask_lead(sync: Sync, profiles: Profiles, steal: bool) {
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"mode".into(), &"exclusive".into());
    if steal {
        let _ = js_sys::Reflect::set(&options, &"steal".into(), &JsValue::TRUE);
    }
    let callback = Closure::<dyn FnMut(JsValue) -> Promise>::new(move |_lock: JsValue| {
        LEADS.with(|l| *l.borrow_mut() = true);
        leptos::task::spawn_local(lead(sync, profiles));
        // Held while the tab lives.
        Promise::new(&mut |_, _| {})
    });
    match request_lock("betula.sync", &options, callback.as_ref().unchecked_ref()) {
        Ok(promise) => {
            callback.forget();
            leptos::task::spawn_local(async move {
                // Settles only when another tab took the lead.
                let _ = JsFuture::from(promise).await;
                LEADS.with(|l| *l.borrow_mut() = false);
                if let Some(engine) = engine() {
                    engine.close();
                }
                ENGINE.with(|e| e.borrow_mut().take());
            });
        }
        Err(_) => {
            LEADS.with(|l| *l.borrow_mut() = true);
            leptos::task::spawn_local(lead(sync, profiles));
        }
    }
}

fn leads() -> bool {
    LEADS.with(|l| *l.borrow())
}

/// Starts it in this tab: asks for the lead, follows the active profile, takes what other tabs
/// leave, and comes back online when the network does.
pub fn start_tab(sync: Sync, profiles: Profiles) {
    ask_lead(sync, profiles, false);
    // The active profile changes: the leader follows it.
    Effect::new(move |_| {
        let _ = profiles.active();
        if leads() {
            leptos::task::spawn_local(lead(sync, profiles));
        }
        sync.refresh_all(profiles);
    });
    let storage = window_event_listener_untyped("storage", move |event| {
        let key = event.dyn_ref::<web_sys::StorageEvent>().and_then(|e| e.key()).unwrap_or_default();
        let active = profiles.active_untracked();
        if key.starts_with(CHANGE_PREFIX) {
            if let Some(engine) = engine().filter(|e| e.leads(active)) {
                engine.take_changes();
            }
        } else if key.starts_with("betula.sync.") {
            if key == leave_key(active) && nav::local_get(&key).is_some() {
                if let Some(engine) = engine().filter(|e| e.leads(active)) {
                    engine.leave();
                }
            } else if key == held_key(active) && leads() {
                leptos::task::spawn_local(lead(sync, profiles));
            }
            sync.refresh_all(profiles);
        } else if let Some(engine) = engine().filter(|e| e.leads(active)) {
            // Another tab wrote a store's text: the records may hold more.
            if let Some(store) = KEYS.iter().find(|k| key_of(k, active) == key).and_then(|k| Store::of_storage_key(k)) {
                engine.core.borrow_mut().touched.insert(store);
                engine.project();
            }
        }
    });
    let online = window_event_listener_untyped("online", move |_| {
        if let Some(engine) = engine() {
            *engine.tries.borrow_mut() = 0;
            engine.connect();
        }
    });
    // The browser knows it has no network: offline at once, not after the socket times out; what
    // changes waits and goes out when the network is back.
    let offline = window_event_listener_untyped("offline", move |_| {
        if let Some(engine) = engine() {
            engine.disconnect();
        }
    });
    let visible = window_event_listener_untyped("visibilitychange", move |_| {
        let shown = web_sys::window().and_then(|w| w.document()).is_some_and(|d| d.visibility_state() == web_sys::VisibilityState::Visible);
        if shown && !leads() {
            ask_lead(sync, profiles, true);
        } else if !shown {
            if let Some(engine) = engine() {
                if *engine.dirty.borrow() {
                    engine.keep_records();
                }
            }
        } else if let Some(engine) = engine() {
            engine.connect();
        }
    });
    on_cleanup(move || {
        storage.remove();
        online.remove();
        offline.remove();
        visible.remove();
    });
}


/// Leads again where this tab leads: the active profile may sync now (activated, joined).
pub fn lead_again(sync: Sync) {
    if let Some(profiles) = sync.profiles {
        if leads() {
            leptos::task::spawn_local(lead(sync, profiles));
        }
    }
}

/// The hints a page has read (`dismiss`), in every tab.
fn seen_key(profile: u16) -> String {
    format!("betula.sync.p{profile}.seen")
}

fn mark(conflict: &super::Conflict) -> String {
    match conflict {
        super::Conflict::ModuleNote { module, at, .. } => format!("n|{module}|{at}"),
        super::Conflict::TerminNote { semester, row, at, .. } => format!("b|{}|{}|{at}", semester.key(), row.text()),
        super::Conflict::Moved { module, at, .. } => format!("m|{module}|{at}"),
    }
}

pub fn dismiss(profile: u16, conflict: &super::Conflict) {
    let mut seen = nav::local_get(&seen_key(profile)).unwrap_or_default();
    seen.push_str(&mark(conflict));
    seen.push('\n');
    // The last hundred: older conflicts are gone from the records anyway (fifty at most).
    let lines: Vec<&str> = seen.lines().collect();
    let keep = lines.len().saturating_sub(100);
    nav::local_set(&seen_key(profile), &lines.get(keep..).unwrap_or_default().join("\n"));
}

/// The conflicts of the leader's records, as a page shows them, without the ones read.
pub fn shown_conflicts(sync: Sync, profile: u16, conflicts: &[folia_sync::Conflict]) -> Vec<super::Conflict> {
    let seen = nav::local_get(&seen_key(profile)).unwrap_or_default();
    super::shown(conflicts, |module| sync.planned_in(module)).into_iter().filter(|c| !seen.lines().any(|line| line == mark(c))).collect()
}

/// The same from what the leader keeps (any tab).
pub fn conflicts(sync: Sync, profile: u16) -> Vec<super::Conflict> {
    let records = nav::local_get(&records_key(profile)).unwrap_or_default();
    let kept = records.lines().find_map(|l| l.strip_prefix("conflicts\t")).and_then(unb64).and_then(|bytes| codec::read_conflicts(&bytes).ok()).unwrap_or_default();
    shown_conflicts(sync, profile, &kept)
}
