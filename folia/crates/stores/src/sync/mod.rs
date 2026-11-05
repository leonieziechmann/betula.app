//! Betula Sync in this browser (research/sync/entwurf.html; the service and its protocol:
//! docs/phloem/protocol.md; the merge: `folia_sync`). A profile is private until „Betula Sync
//! aktivieren": then this browser makes its key, Phloem keeps its records sealed, and every
//! device of the profile holds all of it, online or offline.
//!
//! What a page uses: `Sync::state` of a profile, the actions (`activate`, `pair`, `join`,
//! `recover`, `stop`, `leave`) and the hints of what two devices did at once (`conflicts`). The
//! site offers it where the server says so (`offered`: the `betula-sync` meta tag of the page,
//! `FOLIA_SYNC`); everywhere else every profile is private and nothing here runs.

use leptos::prelude::*;

use folia_calendar::rowkey::RowKey;
use folia_calendar::semester::SemesterKey;

/// The name of the meta tag that says the site offers Betula Sync; its content is where Phloem
/// answers (empty: `/sync/v1/` of the page's own host).
pub const META: &str = "betula-sync";

/// What Betula Sync is for a profile on this device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// On this device alone.
    Private,
    /// „Betula Sync aktivieren", or joining from a link or a code, is under way.
    Starting,
    /// Synced and connected; `devices`: how many other devices of the profile are online now.
    Online { devices: usize },
    /// Synced, but not connected (no network, the service away): what changes here waits and goes
    /// out when the connection is back.
    Offline,
    /// It was synced, but its space is gone (ended on another device, or 13 months untouched):
    /// private again, with all it holds.
    Ended,
    /// Something a person has to know.
    Failed(Problem),
}

impl State {
    /// Whether the profile is synced (connected or not).
    pub fn synced(&self) -> bool {
        matches!(self, State::Online { .. } | State::Offline)
    }
}

/// Why something of Betula Sync did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The service takes no new profiles now (its budget is reached); synced ones go on.
    Budget,
    /// Too many profiles made from this network within the hour.
    SlowDown,
    /// The profile has 16 devices already.
    Full,
    /// A link or code that opens nothing: mistyped, used already, or a link older than ten minutes.
    BadLink,
    /// The service could not be reached.
    Unreachable,
    /// This browser cannot (no Web Crypto, no storage, no WebSocket).
    Unsupported,
    /// No more profiles fit in this browser (`profiles::MAX_PROFILES`).
    NoRoom,
}

/// A link for the next device („Weiteres Gerät verbinden"): its QR code and the link itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    /// `https://<host>/verbinden#<…>`: what follows `#` never reaches a server.
    pub link: String,
    /// Until when it opens, in milliseconds since 1970 (ten minutes, once).
    pub until: u64,
}

/// What two devices did to one thing at the same time, for the hint beside it. `at` orders them
/// (milliseconds since 1970).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// A module's note (Sammlungen) typed on two devices at once: `kept` stands, `other` is the
    /// version of the other device, to be taken over by hand.
    ModuleNote { module: String, kept: Option<String>, other: String, at: u64 },
    /// The same for a Termin's note in the Stundenplan.
    TerminNote { semester: SemesterKey, row: RowKey, kept: Option<String>, other: String, at: u64 },
    /// A module moved on two devices at once: it stands in every semester it was moved to, and
    /// the person picks.
    Moved { module: String, semesters: Vec<SemesterKey>, at: u64 },
}

impl Conflict {
    pub fn at(&self) -> u64 {
        match self {
            Conflict::ModuleNote { at, .. } | Conflict::TerminNote { at, .. } | Conflict::Moved { at, .. } => *at,
        }
    }
}

/// Betula Sync of this browser, shared through context: every profile private on the server and
/// wherever the site does not offer it.
#[derive(Clone, Copy)]
pub struct Sync {
    states: RwSignal<Vec<(u16, State)>>,
    conflicts: RwSignal<Vec<Conflict>>,
}

impl Sync {
    /// Starts Betula Sync for the profiles that sync (in the browser, where the site offers it)
    /// and provides it. Call it after `Profiles::provide` and the stores.
    pub fn provide() -> Self {
        let sync = Sync { states: RwSignal::new(Vec::new()), conflicts: RwSignal::new(Vec::new()) };
        provide_context(sync);
        sync
    }

    pub fn expect() -> Option<Self> {
        use_context::<Sync>()
    }

    /// What Betula Sync is for profile `id` here. Tracked.
    pub fn state(self, id: u16) -> State {
        self.states.with(|states| states.iter().find(|(p, _)| *p == id).map(|(_, s)| s.clone())).unwrap_or(State::Private)
    }

    /// „Betula Sync aktivieren" for profile `id`: its key, its space at Phloem, its records sealed
    /// there. `state` tells how it goes.
    pub fn activate(self, id: u16) {
        let _ = id;
    }

    /// The recovery code of a synced profile: its key in base32 (Crockford), in groups of four.
    /// Whoever has it has the profile; the page shows it on request only.
    pub fn recovery_code(self, id: u16) -> Option<String> {
        let _ = id;
        None
    }

    /// A link for the next device of profile `id` (synced): a sealed parcel with its key waits
    /// ten minutes at Phloem, for one device.
    pub fn pair(self, id: u16, done: impl FnOnce(Result<Pairing, Problem>) + 'static) {
        let _ = id;
        done(Err(Problem::Unsupported));
    }

    /// Joins the profile a link of `pair` hands on (the part after `#`): a new profile of this
    /// browser, named as it is there, which then syncs. Its number.
    pub fn join(self, fragment: &str, done: impl FnOnce(Result<u16, Problem>) + 'static) {
        let _ = fragment;
        done(Err(Problem::Unsupported));
    }

    /// The same from a recovery code (`recovery_code`), as a new profile named `name`.
    pub fn recover(self, code: &str, name: &str, done: impl FnOnce(Result<u16, Problem>) + 'static) {
        let _ = (code, name);
        done(Err(Problem::Unsupported));
    }

    /// „Sync beenden": Phloem deletes the profile's space at once, and every device of it is
    /// private again, with its copy.
    pub fn stop(self, id: u16) {
        let _ = id;
    }

    /// „Nur dieses Gerät trennen": this device keeps its copy, privately; the others go on.
    pub fn leave(self, id: u16) {
        let _ = id;
    }

    /// What two devices did at once in the active profile, the newest last. Tracked.
    pub fn conflicts(self) -> Vec<Conflict> {
        self.conflicts.get()
    }

    /// The hint is read: it goes (taking the other version over is a change of the store, made
    /// with the store's own methods before).
    pub fn dismiss(self, conflict: &Conflict) {
        self.conflicts.update(|all| all.retain(|c| c != conflict));
    }
}

/// Whether this site offers Betula Sync, and where Phloem answers: the page's `betula-sync` meta
/// tag. `None` on the server and where the site does not offer it.
pub fn offered() -> Option<String> {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    {
        let document = web_sys::window()?.document()?;
        let meta = document.query_selector(&format!("meta[name=\"{META}\"]")).ok()??;
        let base = meta.get_attribute("content").unwrap_or_default();
        Some(if base.trim().is_empty() { "/sync/v1/".to_string() } else { base.trim().to_string() })
    }
    #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
    None
}
