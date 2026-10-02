//! `DataClient` (docs/folia-refactor.md §6.4): the only way the UI gets data. It sends a page's
//! question to the data worker, keeps the answer per snapshot, and drops what it kept when the
//! worker switches to a new snapshot, so that the page on screen asks again and shows the new
//! data in place. The UI bundle has no `Database`: a query on the main thread cannot happen.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use folia_pages::{decode, encode, Answer, Ask, Reply, Request};
use futures_channel::oneshot;
use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

pub type Outcome = Result<Answer, String>;

struct Inner {
    worker: web_sys::Worker,
    next: Cell<u32>,
    pending: RefCell<HashMap<u32, (Ask, f64, oneshot::Sender<Outcome>)>>,
    kept: RefCell<HashMap<Ask, Answer>>,
    snapshot: RefCell<String>,
}

/// The client, provided through context by the app.
#[derive(Clone)]
pub struct DataClient {
    inner: SendWrapper<Rc<Inner>>,
    /// Counts the snapshots the worker switched to: what reads data tracks it.
    generation: RwSignal<u64>,
    /// What the header says about the data.
    pub status: RwSignal<Option<String>>,
}

fn now() -> f64 {
    web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now())
}

/// The times of the requests, for the checks of the minimal version (`window.__foliaTimings`:
/// `[ask, ms in the worker, ms in all]`).
fn note_timing(ask: &Ask, worker_ms: f64, total_ms: f64) {
    let Some(window) = web_sys::window() else { return };
    let list = js_sys::Reflect::get(&window, &"__foliaTimings".into()).ok().filter(|v| v.is_object()).unwrap_or_else(|| {
        let list = js_sys::Array::new();
        let _ = js_sys::Reflect::set(&window, &"__foliaTimings".into(), &list);
        list.into()
    });
    let entry = js_sys::Array::of3(&format!("{ask:?}").into(), &worker_ms.into(), &total_ms.into());
    list.unchecked_into::<js_sys::Array>().push(&entry);
}

impl DataClient {
    /// Wraps the worker `boot.js` started (it opens the snapshot while the UI bundle loads).
    pub fn new(worker: web_sys::Worker) -> Self {
        let inner = Rc::new(Inner { worker, next: Cell::new(1), pending: RefCell::new(HashMap::new()), kept: RefCell::new(HashMap::new()), snapshot: RefCell::new(String::new()) });
        let client = DataClient { inner: SendWrapper::new(inner.clone()), generation: RwSignal::new(0), status: RwSignal::new(None) };
        let this = client.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| this.receive(event.data()));
        inner.worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget(); // lives as long as the app: the worker is never replaced in the minimal version
        // What the worker said before the UI bundle was there (`boot.js` keeps it): the snapshot
        // it opened, its download's progress.
        if let Some(early) = web_sys::window().and_then(|w| js_sys::Reflect::get(&w, &"__foliaEarly".into()).ok()).filter(|v| v.is_object()) {
            for message in js_sys::Array::from(&early).iter() {
                client.receive(message);
            }
        }
        client
    }

    fn receive(&self, data: JsValue) {
        if let Some(bytes) = data.dyn_ref::<js_sys::Uint8Array>() {
            let Ok(reply) = decode::<Reply>(&bytes.to_vec()) else { return };
            let Some((ask, sent, done)) = self.inner.pending.borrow_mut().remove(&reply.id) else { return };
            note_timing(&ask, reply.ms, now() - sent);
            if *self.inner.snapshot.borrow() == reply.snapshot {
                if let Ok(answer) = &reply.result {
                    self.inner.kept.borrow_mut().insert(ask, answer.clone());
                }
            }
            let _ = done.send(reply.result);
            return;
        }
        // Everything else the worker says is a note: {type: "status", text} or {type: "snapshot", etag}.
        let field = |name: &str| js_sys::Reflect::get(&data, &name.into()).ok().and_then(|v| v.as_string());
        match field("type").as_deref() {
            Some("status") => self.status.set(field("text").filter(|text| !text.is_empty())),
            Some("snapshot") => {
                let etag = field("etag").unwrap_or_default();
                let changed = *self.inner.snapshot.borrow() != etag;
                let first = self.inner.snapshot.borrow().is_empty();
                *self.inner.snapshot.borrow_mut() = etag;
                if changed {
                    self.inner.kept.borrow_mut().clear();
                    if !first {
                        self.generation.update(|n| *n += 1);
                    }
                }
            }
            _ => {}
        }
    }

    /// The answer kept for `ask`, without asking the worker.
    pub fn peek(&self, ask: &Ask) -> Option<Answer> {
        self.inner.kept.borrow().get(ask).cloned()
    }

    /// Asks the worker; the answer is kept for the snapshot it came from.
    pub fn ask(&self, ask: Ask) -> impl std::future::Future<Output = Outcome> {
        let (done, answer) = oneshot::channel();
        let id = self.inner.next.get();
        self.inner.next.set(id.wrapping_add(1));
        let sent = match encode(&Request { id, ask: ask.clone() }) {
            Ok(bytes) => {
                self.inner.pending.borrow_mut().insert(id, (ask, now(), done));
                let message = js_sys::Uint8Array::from(bytes.as_slice());
                self.inner.worker.post_message(&message).map_err(|e| format!("{e:?}"))
            }
            Err(e) => Err(e),
        };
        async move {
            sent?;
            answer.await.unwrap_or_else(|_| Err("the data worker did not answer".to_string()))
        }
    }

    pub fn generation(&self) -> RwSignal<u64> {
        self.generation
    }

    pub fn expect() -> Self {
        expect_context::<DataClient>()
    }
}

/// A page's data: `value` is the newest answer, kept while the next one is on its way (so a page
/// never flashes empty), `slow` turns true when a wait has lasted longer than the threshold.
#[derive(Clone, Copy)]
pub struct Loaded {
    pub value: RwSignal<Option<Outcome>>,
    pub slow: RwSignal<bool>,
}

/// The threshold after which a wait shows its skeleton (`pending::SLOW_MS` today).
pub const SLOW_MS: u32 = 50;

/// Asks for `ask` whenever it changes or the worker switches to a new snapshot. What the client
/// kept is there in the first render already, which is what lets the app take over a site page
/// without a frame of skeleton.
pub fn use_ask(ask: impl Fn() -> Ask + Send + Sync + 'static) -> Loaded {
    let client = DataClient::expect();
    let current = Memo::new(move |_| ask());
    let generation = client.generation();
    let value = RwSignal::new(client.peek(&current.get_untracked()).map(Ok));
    let slow = RwSignal::new(false);
    // The question on its way, if any.
    let waiting = StoredValue::new(None::<Ask>);
    Effect::new(move |_| {
        let wanted = current.get();
        generation.track();
        if let Some(answer) = client.peek(&wanted) {
            waiting.set_value(None);
            value.set(Some(Ok(answer)));
            slow.set(false);
            return;
        }
        waiting.set_value(Some(wanted.clone()));
        let late = wanted.clone();
        set_timeout(
            // The page may be gone by now (another route): every access is a `try_` one, since
            // what a page made is disposed with it (R2).
            move || {
                if waiting.try_with_value(|w| w.as_ref() == Some(&late)) == Some(true) {
                    slow.try_set(true);
                }
            },
            std::time::Duration::from_millis(u64::from(SLOW_MS)),
        );
        let client = client.clone();
        leptos::task::spawn_local(async move {
            let outcome = client.ask(wanted.clone()).await;
            if waiting.try_with_value(|w| w.as_ref() == Some(&wanted)) == Some(true) {
                waiting.try_set_value(None);
                value.try_set(Some(outcome));
                slow.try_set(false);
            }
        });
    });
    Loaded { value, slow }
}
