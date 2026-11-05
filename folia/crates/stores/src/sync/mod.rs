//! Betula Sync in this browser (research/sync/entwurf.html; the service and its protocol:
//! docs/phloem/protocol.md; the merge: `folia_sync`). A profile is private until „Betula Sync
//! aktivieren": then this browser makes its key, Phloem keeps its records sealed, and every
//! device of the profile holds all of it, online or offline.
//!
//! What a page uses: `Sync::state` of a profile, the actions (`activate`, `pair`, `join`,
//! `recover`, `stop`, `leave`) and the hints of what two devices did at once (`conflicts`). The
//! site offers it where the server says so (`offered`: the `betula-sync` meta tag of the page,
//! `FOLIA_SYNC`); everywhere else every profile is private and nothing here runs.

#[cfg(all(feature = "csr", target_arch = "wasm32"))]
mod browser;
pub mod codes;
pub mod core;
pub mod wire;

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
    /// What this browser keeps of each profile's sync (the leader's word on its connection).
    states: RwSignal<Vec<(u16, State)>>,
    /// What an action of this tab is doing or failed at, above what is kept.
    actions: RwSignal<Vec<(u16, State)>>,
    conflicts: RwSignal<Vec<Conflict>>,
}

impl Sync {
    /// Starts Betula Sync for the profiles that sync (in the browser, where the site offers it)
    /// and provides it. Call it after `Profiles::provide` and the stores.
    pub fn provide() -> Self {
        let sync = Sync { states: RwSignal::new(Vec::new()), actions: RwSignal::new(Vec::new()), conflicts: RwSignal::new(Vec::new()) };
        provide_context(sync);
        // Effects run in the browser only.
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        Effect::new(move |started: Option<()>| {
            if started.is_none() && offered().is_some() {
                if let Some(profiles) = crate::profiles::Profiles::expect() {
                    browser::start_tab(sync, profiles);
                    sync.refresh_all(profiles);
                }
            }
        });
        sync
    }

    pub fn expect() -> Option<Self> {
        use_context::<Sync>()
    }

    /// What Betula Sync is for profile `id` here. Tracked.
    pub fn state(self, id: u16) -> State {
        let action = self.actions.with(|actions| actions.iter().find(|(p, _)| *p == id).map(|(_, s)| s.clone()));
        action.or_else(|| self.states.with(|states| states.iter().find(|(p, _)| *p == id).map(|(_, s)| s.clone()))).unwrap_or(State::Private)
    }

    /// Reads again what this browser keeps of profile `id`.
    #[allow(unused_variables)]
    pub(crate) fn refresh(self, id: u16) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let state = browser::state_of(id);
            self.states.update(|states| {
                states.retain(|(p, _)| *p != id);
                states.push((id, state));
            });
            if crate::profiles::Profiles::expect().is_some_and(|p| p.active_untracked() == id) {
                let shown = browser::conflicts(id);
                if self.conflicts.with_untracked(|now| *now != shown) {
                    self.conflicts.set(shown);
                }
            }
        }
    }

    #[cfg_attr(not(all(feature = "csr", target_arch = "wasm32")), allow(dead_code))]
    pub(crate) fn refresh_all(self, profiles: crate::profiles::Profiles) {
        for profile in profiles.all() {
            self.refresh(profile.id);
        }
    }

    fn act(self, id: u16, state: Option<State>) {
        self.actions.update(|actions| {
            actions.retain(|(p, _)| *p != id);
            if let Some(state) = state {
                actions.push((id, state));
            }
        });
    }

    pub(crate) fn fail(self, id: u16, problem: Problem) {
        self.act(id, Some(State::Failed(problem)));
    }

    /// The conflicts the leader's records hold for the active profile.
    #[allow(unused_variables)]
    #[cfg_attr(not(all(feature = "csr", target_arch = "wasm32")), allow(dead_code))]
    pub(crate) fn conflicts_of(self, doc: &folia_sync::Doc) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        if let Some(profiles) = crate::profiles::Profiles::expect() {
            let shown = browser::shown_conflicts(profiles.active_untracked(), &doc.conflicts);
            if self.conflicts.with_untracked(|now| *now != shown) {
                self.conflicts.set(shown);
            }
        }
    }

    /// „Betula Sync aktivieren" for profile `id`: its key, its space at Phloem, its records sealed
    /// there. `state` tells how it goes.
    pub fn activate(self, id: u16) {
        self.act(id, Some(State::Starting));
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        leptos::task::spawn_local(async move {
            match browser::activate(id).await {
                Ok(()) => {
                    self.act(id, None);
                    self.refresh(id);
                    browser::lead_again(self);
                }
                Err(problem) => self.fail(id, problem),
            }
        });
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        self.fail(id, Problem::Unsupported);
    }

    /// The recovery code of a synced profile: its key in base32 (Crockford), in groups of four.
    /// Whoever has it has the profile; the page shows it on request only.
    #[allow(unused_variables)]
    pub fn recovery_code(self, id: u16) -> Option<String> {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        return browser::recovery_code(id);
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        None
    }

    /// A link for the next device of profile `id` (synced): a sealed parcel with its key waits
    /// ten minutes at Phloem, for one device.
    pub fn pair(self, id: u16, done: impl FnOnce(Result<Pairing, Problem>) + 'static) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let name = crate::profiles::Profiles::expect()
                .and_then(|profiles| profiles.all().into_iter().find(|p| p.id == id))
                .map(|p| p.name)
                .unwrap_or_default();
            leptos::task::spawn_local(async move { done(browser::pair(id, name).await) });
        }
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = id;
            done(Err(Problem::Unsupported));
        }
    }

    /// Joins the profile a link of `pair` hands on (the part after `#`): a new profile of this
    /// browser, named as it is there, which then syncs. Its number.
    pub fn join(self, fragment: &str, done: impl FnOnce(Result<u16, Problem>) + 'static) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let fragment = fragment.to_string();
            leptos::task::spawn_local(async move {
                let result = match browser::unpack(&fragment).await {
                    Ok(shared) => self.adopt(shared),
                    Err(problem) => Err(problem),
                };
                done(result);
            });
        }
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = fragment;
            done(Err(Problem::Unsupported));
        }
    }

    /// The same from a recovery code (`recovery_code`), as a new profile named `name`.
    pub fn recover(self, code: &str, name: &str, done: impl FnOnce(Result<u16, Problem>) + 'static) {
        match codes::key_of_code(code) {
            Some(key) => done(self.adopt(vec![(key, name.to_string())])),
            None => done(Err(Problem::BadLink)),
        }
    }

    /// New profiles of this browser for the keys of another device's; the last of them active.
    #[allow(unused_variables)]
    fn adopt(self, shared: Vec<([u8; codes::KEY], String)>) -> Result<u16, Problem> {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let profiles = crate::profiles::Profiles::expect().ok_or(Problem::Unsupported)?;
            let mut last = Err(Problem::BadLink);
            for (key, name) in shared {
                let id = profiles.create(&name).ok_or(Problem::NoRoom)?;
                browser::start(id, key)?;
                self.refresh(id);
                last = Ok(id);
            }
            browser::lead_again(self);
            last
        }
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        Err(Problem::Unsupported)
    }

    /// „Sync beenden": Phloem deletes the profile's space at once, and every device of it is
    /// private again, with its copy.
    pub fn stop(self, id: u16) {
        self.act(id, Some(State::Starting));
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        leptos::task::spawn_local(async move {
            match browser::stop(id).await {
                Ok(()) => {
                    self.act(id, None);
                    self.refresh(id);
                }
                Err(problem) => self.fail(id, problem),
            }
        });
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        self.fail(id, Problem::Unsupported);
    }

    /// „Nur dieses Gerät trennen": this device keeps its copy, privately; the others go on.
    pub fn leave(self, id: u16) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        if let Some(profiles) = crate::profiles::Profiles::expect() {
            browser::leave(id, profiles.active_untracked());
        }
        self.act(id, None);
        self.refresh(id);
    }

    /// What two devices did at once in the active profile, the newest last. Tracked.
    pub fn conflicts(self) -> Vec<Conflict> {
        self.conflicts.get()
    }

    /// The hint is read: it goes (taking the other version over is a change of the store, made
    /// with the store's own methods before), in every tab.
    pub fn dismiss(self, conflict: &Conflict) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        if let Some(profiles) = crate::profiles::Profiles::expect() {
            browser::dismiss(profiles.active_untracked(), conflict);
        }
        self.conflicts.update(|all| all.retain(|c| c != conflict));
    }
}

/// Whether profile `id` syncs in this browser.
#[allow(unused_variables)]
pub(crate) fn syncs(id: u16) -> bool {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    return offered().is_some() && browser::syncs(id);
    #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
    false
}

/// A store of a synced profile changed from `before` to `after` (both as it writes its text).
#[allow(unused_variables)]
pub(crate) fn changed(id: u16, key: &str, before: &str, after: &str) {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    browser::changed(id, key, before, after);
}

/// A store's text was written.
#[allow(unused_variables)]
pub(crate) fn wrote(id: u16, key: &str) {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    if syncs(id) {
        browser::wrote(id, key);
    }
}

/// Profile `id` goes from this browser: a synced one leaves its space for this device alone.
#[allow(unused_variables)]
pub(crate) fn forget(id: u16) {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    browser::forget(id);
}

/// What `folia_sync` keeps of two devices at once, as a page shows it: a note's text in place of
/// its line, and a module moved twice where it stands in two semesters now (`planned_in`).
pub fn shown(conflicts: &[folia_sync::Conflict], planned_in: impl Fn(&str) -> Vec<SemesterKey>) -> Vec<Conflict> {
    use folia_plans::studyplan::PlanDoc;
    let mut out = Vec::new();
    for c in conflicts {
        let at = c.at.ms;
        if let Some(module) = c.key.strip_prefix("C|n|") {
            let note = |line: &str| line.splitn(3, '\t').nth(2).map(unescape);
            out.push(Conflict::ModuleNote { module: module.to_string(), kept: c.kept.as_deref().and_then(note), other: note(&c.other).unwrap_or_default(), at });
        } else if let Some(rest) = c.key.strip_prefix("P|b|0|") {
            let mut parts = rest.split('|');
            let (Some(semester), Some(row)) = (parts.next().and_then(SemesterKey::parse), parts.next().and_then(RowKey::parse)) else { continue };
            let note = |line: &str| PlanDoc::restored(line).note_of(semester, row).map(str::to_string);
            out.push(Conflict::TerminNote { semester, row, kept: c.kept.as_deref().and_then(note), other: note(&c.other).unwrap_or_default(), at });
        } else if let Some(rest) = c.key.strip_prefix("P|mv|") {
            let Some(module) = rest.split('|').nth(1) else { continue };
            let semesters = planned_in(module);
            if c.other.is_empty() && semesters.len() > 1 {
                out.push(Conflict::Moved { module: module.to_string(), semesters, at });
            }
        }
    }
    out.sort_by_key(Conflict::at);
    out
}

/// A note of the Sammlungen as `Shelf::stored` writes it, read back.
fn unescape(note: &str) -> String {
    let mut text = String::new();
    let mut chars = note.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => text.push('\n'),
                Some('t') => text.push('\t'),
                Some(other) => text.push(other),
                None => {}
            }
        } else {
            text.push(c);
        }
    }
    text
}

/// Whether this site offers Betula Sync, and where Phloem answers: the page's `betula-sync` meta
/// tag. `None` on the server and where the site does not offer it.
pub fn offered() -> Option<String> {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    {
        thread_local! {
            static OFFERED: std::cell::OnceCell<Option<String>> = const { std::cell::OnceCell::new() };
        }
        OFFERED.with(|offered| {
            offered
                .get_or_init(|| {
                    let document = web_sys::window()?.document()?;
                    let meta = document.query_selector(&format!("meta[name=\"{META}\"]")).ok()??;
                    let base = meta.get_attribute("content").unwrap_or_default();
                    Some(if base.trim().is_empty() { "/sync/v1/".to_string() } else { base.trim().to_string() })
                })
                .clone()
        })
    }
    #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
    None
}
