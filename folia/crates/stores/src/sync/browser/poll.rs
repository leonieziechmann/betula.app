//! The socket over plain HTTPS (Phloem 0.4.0, protocol.md §4 „The socket over HTTPS"): where the
//! WebSocket does not come about (a proxy or a scanner that breaks its upgrade, a network that
//! lets only plain requests through), the same frames go as requests to the same host. One GET at
//! a time waits for what Phloem says; what the device says goes in POSTs, one after the other, in
//! order. Everything else (the hello, the welcome, every frame, the close codes) is the socket's.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use super::call;

/// The close code of a session that ended without one from Phloem (the network, a request that
/// failed): the device comes back as after a socket that was lost.
pub(super) const LOST: u16 = 0;

#[derive(Serialize)]
struct Frames<'a> {
    frames: &'a [String],
}

#[derive(Deserialize)]
struct Opened {
    poll: String,
}

#[derive(Deserialize)]
struct Said {
    #[serde(default)]
    frames: Vec<String>,
    #[serde(default)]
    close: Option<Closed>,
}

#[derive(Deserialize)]
struct Closed {
    #[serde(default)]
    code: u16,
}

/// One session: what waits to be sent, and the session once Phloem opened it.
pub(super) struct Poll {
    id: RefCell<Option<String>>,
    queue: RefCell<VecDeque<String>>,
    /// A POST is under way (the open, or frames): the next waits for it, so that order holds.
    sending: Cell<bool>,
    ended: Cell<bool>,
    on_frame: Box<dyn Fn(String)>,
    on_end: RefCell<Option<Box<dyn FnOnce(u16)>>>,
}

impl Poll {
    /// A session that opens with the first frame sent (the hello): `on_frame` hears every frame
    /// Phloem says, `on_end` once how it ended (a close code of §4, or `LOST`).
    pub(super) fn new(on_frame: impl Fn(String) + 'static, on_end: impl FnOnce(u16) + 'static) -> Rc<Poll> {
        Rc::new(Poll { id: RefCell::new(None), queue: RefCell::new(VecDeque::new()), sending: Cell::new(false), ended: Cell::new(false), on_frame: Box::new(on_frame), on_end: RefCell::new(Some(Box::new(on_end))) })
    }

    /// Sends a frame, after the ones before it.
    pub(super) fn send(self: &Rc<Self>, text: String) -> bool {
        if self.ended.get() {
            return false;
        }
        self.queue.borrow_mut().push_back(text);
        self.flush();
        true
    }

    /// Ends the session here; Phloem lets it go when it hears nothing more (as a socket lost).
    pub(super) fn close(&self) {
        self.end(LOST);
    }

    fn end(&self, code: u16) {
        if self.ended.replace(true) {
            return;
        }
        self.queue.borrow_mut().clear();
        if let Some(on_end) = self.on_end.borrow_mut().take() {
            on_end(code);
        }
    }

    fn flush(self: &Rc<Self>) {
        if self.sending.get() || self.ended.get() || self.queue.borrow().is_empty() {
            return;
        }
        self.sending.set(true);
        let poll = Rc::clone(self);
        leptos::task::spawn_local(async move {
            loop {
                let frames: Vec<String> = poll.queue.borrow_mut().drain(..).collect();
                if frames.is_empty() || poll.ended.get() {
                    break;
                }
                let body = serde_json::to_string(&Frames { frames: &frames }).unwrap_or_default();
                let id = poll.id.borrow().clone();
                let ok = match id {
                    // The first POST opens the session with the hello; then the GETs begin.
                    None => match call("POST", "poll", &[], Some(body)).await {
                        Ok((201, text)) => match serde_json::from_str::<Opened>(&text) {
                            Ok(opened) => {
                                *poll.id.borrow_mut() = Some(opened.poll);
                                poll.listen();
                                true
                            }
                            Err(_) => false,
                        },
                        _ => false,
                    },
                    Some(id) => matches!(call("POST", "poll", &[("Betula-Poll", id)], Some(body)).await, Ok((204 | 200, _))),
                };
                if !ok {
                    poll.end(LOST);
                    break;
                }
            }
            poll.sending.set(false);
        });
    }

    /// The GETs, one after the other, until the session ends.
    fn listen(self: &Rc<Self>) {
        let poll = Rc::clone(self);
        leptos::task::spawn_local(async move {
            loop {
                let Some(id) = poll.id.borrow().clone() else { break };
                if poll.ended.get() {
                    break;
                }
                let said = match call("GET", "poll", &[("Betula-Poll", id)], None).await {
                    Ok((200, text)) => serde_json::from_str::<Said>(&text).ok(),
                    _ => None,
                };
                if poll.ended.get() {
                    break;
                }
                let Some(said) = said else {
                    poll.end(LOST);
                    break;
                };
                for frame in said.frames {
                    (poll.on_frame)(frame);
                }
                if let Some(closed) = said.close {
                    poll.end(closed.code);
                    break;
                }
            }
        });
    }
}
