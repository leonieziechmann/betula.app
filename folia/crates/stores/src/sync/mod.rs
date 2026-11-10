//! Betula Sync in this browser (research/sync/entwurf.html; the service and its protocol:
//! docs/phloem/protocol.md; the merge: `folia_sync`). A profile is private until „Betula Sync
//! aktivieren": then this browser makes its key, Phloem keeps its records sealed, and every
//! device of the profile holds all of it, online or offline.
//!
//! What a page uses: `Sync::state` of a profile, the actions (`activate`, `pair`, `join`,
//! `recover`, `stop`, `leave`; and the other way round, `request` on the device that wants a
//! profile and `give` on the one that has it: `request`) and the hints of what two devices did at once (`conflicts`). The
//! site offers it where the server says so (`offered`: the `betula-sync` meta tag of the page,
//! `FOLIA_SYNC`); everywhere else every profile is private and nothing here runs.

#[cfg(all(feature = "csr", target_arch = "wasm32"))]
mod browser;
pub mod codes;
pub mod core;
pub mod devices;
pub mod live;
pub mod request;
pub mod share;
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
    /// Synced and connected; `devices`: how many other devices of the profile are online now,
    /// `direct` of them connected with this one directly, `relayed` through Phloem's TURN
    /// (WebRTC, step 2); the others hear of each change through Phloem.
    Online { devices: usize, direct: usize, relayed: usize },
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

/// How long a link or a profile sent waits for the profile to be online with its records at
/// Phloem before it says the service could not be reached (ms): the connection tries every way it
/// has meanwhile (`browser`: the WebSocket, then plain HTTPS).
pub const ONLINE_WAIT_MS: u64 = 30_000;

/// A link for the next device („Weiteres Gerät verbinden"): its QR code and the link itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    /// `https://<host>/sync#<…>` (`url::CONNECT`; `/verbinden#<…>` until 2026-11-08, which leads
    /// there still): what follows `#` never reaches a server.
    pub link: String,
    /// Until when it opens, in milliseconds since 1970 (ten minutes, once).
    pub until: u64,
}

/// A request for a profile of another device („Profil vom Handy holen", `request`): the link this
/// device shows as a QR code, the digits both devices show, until when it asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// The parcel's id (base64url): what `Sync::drop_request` puts away.
    pub id: String,
    /// `https://<host>/pair#<…>` (`url::PAIR`): what follows `#` never reaches a server.
    pub link: String,
    /// „0731": what the phone shows as well, once it has read the code.
    pub digits: String,
    /// Milliseconds since 1970 (ten minutes).
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
    /// The profiles and the Stundenplan, taken where it is made: what the engine does from a
    /// socket or a promise has no context to find them in.
    #[cfg_attr(not(all(feature = "csr", target_arch = "wasm32")), allow(dead_code))]
    profiles: Option<crate::profiles::Profiles>,
    plan: Option<crate::studyplan::Studyplan>,
    /// The Merkliste: its marks and its collections, for the shares of the list.
    marks: Option<crate::bookmarks::Bookmarks>,
    collections: Option<crate::collections::Collections>,
    /// What this browser keeps of each profile's sync (the leader's word on its connection).
    states: RwSignal<Vec<(u16, State)>>,
    /// What an action of this tab is doing or failed at, above what is kept.
    actions: RwSignal<Vec<(u16, State)>>,
    conflicts: RwSignal<Vec<Conflict>>,
    /// The devices of each profile as this browser knows them (`devices::list`).
    devices: RwSignal<Vec<(u16, Devices)>>,
    /// How many parcels of the active profile were opened by another device while this tab led
    /// (`taken`).
    taken: RwSignal<u32>,
}

/// The devices of a synced profile as a page lists them (`devices::list`): this device first, then
/// those online, then by when they were last seen; and how many devices online wrote no record of
/// themselves (a build before the list).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Devices {
    pub list: Vec<devices::Device>,
    pub unnamed: usize,
}

impl Sync {
    /// Starts Betula Sync for the profiles that sync (in the browser, where the site offers it)
    /// and provides it. Call it after `Profiles::provide` and the stores.
    pub fn provide() -> Self {
        let sync = Sync {
            profiles: crate::profiles::Profiles::expect(),
            plan: crate::studyplan::Studyplan::expect(),
            marks: crate::bookmarks::Bookmarks::expect(),
            collections: crate::collections::Collections::expect(),
            states: RwSignal::new(Vec::new()),
            actions: RwSignal::new(Vec::new()),
            conflicts: RwSignal::new(Vec::new()),
            devices: RwSignal::new(Vec::new()),
            taken: RwSignal::new(0),
        };
        provide_context(sync);
        // Effects run in the browser only.
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        Effect::new(move |started: Option<()>| {
            if started.is_none() && offered().is_some() {
                if let Some(profiles) = sync.profiles {
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

    /// Where a module stands in the Stundenplan (for the hint of a module moved twice).
    #[cfg_attr(not(all(feature = "csr", target_arch = "wasm32")), allow(dead_code))]
    pub(crate) fn planned_in(self, module: &str) -> Vec<SemesterKey> {
        self.plan.map(|plan| plan.with_untracked(|doc| doc.planned_in(module))).unwrap_or_default()
    }

    /// What Betula Sync is for profile `id` here. Tracked.
    pub fn state(self, id: u16) -> State {
        let action = self.actions.with(|actions| actions.iter().find(|(p, _)| *p == id).map(|(_, s)| s.clone()));
        action.or_else(|| self.states.with(|states| states.iter().find(|(p, _)| *p == id).map(|(_, s)| s.clone()))).unwrap_or(State::Private)
    }

    /// The devices of profile `id` (synced; none for a private one). Tracked.
    pub fn devices(self, id: u16) -> Devices {
        self.devices.with(|all| all.iter().find(|(p, _)| *p == id).map(|(_, d)| d.clone())).unwrap_or_default()
    }

    /// „Entfernen": the device of record `key` (another device of profile `id`) stands in no list
    /// any more and leaves Betula Sync the next time it is online, with its copy.
    #[allow(unused_variables)]
    pub fn remove_device(self, id: u16, key: &str) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            browser::remove_device(id, key);
            self.refresh(id);
        }
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
            let (list, unnamed) = browser::devices_of(id);
            let devices = Devices { list, unnamed };
            if self.devices.with_untracked(|all| all.iter().find(|(p, _)| *p == id).map(|(_, d)| d) != Some(&devices)) {
                self.devices.update(|all| {
                    all.retain(|(p, _)| *p != id);
                    all.push((id, devices));
                });
            }
            if self.profiles.is_some_and(|p| p.active_untracked() == id) {
                let shown = browser::conflicts(self, id);
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
        if let Some(profiles) = self.profiles {
            let shown = browser::shown_conflicts(self, profiles.active_untracked(), &doc.conflicts);
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
                    browser::lead_here(self);
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
    /// ten minutes at Phloem, for one device. Made once the profile is online here with its
    /// records at Phloem (this tab takes the connection for it): a link made before would hand
    /// on a profile that is not there. `Unreachable` when that does not come within
    /// `ONLINE_WAIT_MS`.
    pub fn pair(self, id: u16, done: impl FnOnce(Result<Pairing, Problem>) + 'static) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let name = self
                .profiles
                .and_then(|profiles| profiles.all().into_iter().find(|p| p.id == id))
                .map(|p| p.name)
                .unwrap_or_default();
            browser::lead_here(self);
            leptos::task::spawn_local(async move {
                if !browser::until_online(id, ONLINE_WAIT_MS).await {
                    done(Err(Problem::Unreachable));
                    return;
                }
                done(browser::pair(id, name).await)
            });
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

    /// A request for a profile of another device (`request`): `made` hears the link to show and
    /// its digits (or why there is none), and then, once the other device has sent a profile,
    /// `came` hears the profile of this browser it is now (as `join`'s is). Nothing comes once the
    /// code asks no more (ten minutes) or `drop_request` put it away.
    pub fn request(self, made: impl FnOnce(Result<Request, Problem>) + 'static, came: impl FnOnce(Result<u16, Problem>) + 'static) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        leptos::task::spawn_local(async move {
            match browser::make_request().await {
                Ok(request) => {
                    let (id, made_at) = (request.id.clone(), request.until.saturating_sub(request::REQUEST_MS));
                    made(Ok(request));
                    if let Some(result) = browser::await_request(id, made_at).await {
                        came(result.and_then(|shared| self.adopt(shared)));
                    }
                }
                Err(problem) => made(Err(problem)),
            }
        });
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = came;
            made(Err(Problem::Unsupported));
        }
    }

    /// Puts request `id` away: it asks no more, and a profile sent to it does not come.
    #[allow(unused_variables)]
    pub fn drop_request(self, id: &str) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        browser::drop_request(id);
    }

    /// The digits of a request's part after `#` (`request::parse`), as the device that asks shows
    /// them; `None` for what is no request.
    pub fn request_digits(self, fragment: &str, done: impl FnOnce(Option<String>) + 'static) {
        let Some((id, public)) = request::parse(fragment) else {
            done(None);
            return;
        };
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        leptos::task::spawn_local(async move { done(browser::request_digits(&id, &public).await) });
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = (id, public);
            done(None);
        }
    }

    /// Sends profile `id` to the device whose request is `fragment`: it is switched on first where
    /// it is private (as „Betula Sync aktivieren"), then its key goes there, sealed to the
    /// request's key. The other device joins the profile as one more of its devices.
    pub fn give(self, id: u16, fragment: &str, done: impl FnOnce(Result<(), Problem>) + 'static) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let fragment = fragment.to_string();
            let name = self.profiles.and_then(|profiles| profiles.all().into_iter().find(|p| p.id == id)).map(|p| p.name).unwrap_or_default();
            leptos::task::spawn_local(async move {
                if !browser::syncs(id) {
                    // As `activate_then`.
                    self.act(id, Some(State::Starting));
                    if let Err(problem) = browser::activate(id).await {
                        self.fail(id, problem);
                        done(Err(problem));
                        return;
                    }
                    self.act(id, None);
                    self.refresh(id);
                }
                // As `pair`: the profile goes only once it is at Phloem.
                browser::lead_here(self);
                if !browser::until_online(id, ONLINE_WAIT_MS).await {
                    done(Err(Problem::Unreachable));
                    return;
                }
                done(browser::give(id, name, &fragment).await);
            });
        }
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = (id, fragment);
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

    /// Profiles of this browser for the keys of another device's, the last of them active: a new
    /// profile each, but in a browser new to Betula the first comes into its empty first profile
    /// (a phone that opens the link holds the profile, not a second one beside an empty one).
    #[allow(unused_variables)]
    fn adopt(self, shared: Vec<([u8; codes::KEY], String)>) -> Result<u16, Problem> {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let profiles = self.profiles.ok_or(Problem::Unsupported)?;
            let mut last = Err(Problem::BadLink);
            for (key, name) in shared {
                let id = match profiles.empty_first() {
                    Some(first) => {
                        profiles.rename(first, &name);
                        profiles.switch(first);
                        first
                    }
                    None => profiles.create(&name).ok_or(Problem::NoRoom)?,
                };
                browser::start(id, key)?;
                self.refresh(id);
                last = Ok(id);
            }
            browser::lead_here(self);
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
        if let Some(profiles) = self.profiles {
            browser::leave(id, profiles.active_untracked());
        }
        self.act(id, None);
        self.refresh(id);
    }

    /// „Betula Sync aktivieren" for profile `id`, then `done` with whether it syncs now: for an
    /// offer of a choice Betula Sync makes better („Profil speichern", owner 2026-11-08), which goes
    /// on with that choice.
    pub fn activate_then(self, id: u16, done: impl FnOnce(Result<(), Problem>) + 'static) {
        self.act(id, Some(State::Starting));
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        leptos::task::spawn_local(async move {
            match browser::activate(id).await {
                Ok(()) => {
                    self.act(id, None);
                    self.refresh(id);
                    browser::lead_here(self);
                    done(Ok(()));
                }
                Err(problem) => {
                    self.fail(id, problem);
                    done(Err(problem));
                }
            }
        });
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            self.fail(id, Problem::Unsupported);
            done(Err(Problem::Unsupported));
        }
    }

    /// A new share of the active profile's calendar (folia/design/teilen/teilen.html): its part
    /// from the plan as it is, put at Phloem, and its record in the plan. `done` hears the link's
    /// path (absolute in the page's language is the caller's) or why not.
    pub fn share(self, request: ShareRequest, done: impl FnOnce(Result<Made, share::ShareProblem>) + 'static) {
        let (Some(profiles), Some(plan)) = (self.profiles, self.plan) else {
            done(Err(share::ShareProblem::Unsupported));
            return;
        };
        let profile = profiles.active_untracked();
        if !syncs(profile) {
            done(Err(share::ShareProblem::NotSynced));
            return;
        }
        let template = request.template();
        let today = folia_calendar::day::Day(i32::try_from(now_ms() / 86_400_000).unwrap_or(i32::MAX));
        let (part, left_out) = plan.with_untracked(|doc| doc.live_part(&template, folia_calendar::live::following(template.semester, today)).fitted());
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        leptos::task::spawn_local(async move {
            let result = browser::share(profile, template, part, left_out).await.map(|made| Made { share: made.share, path: made.path, until: made.until, left_out: made.left_out });
            if let Ok(made) = &result {
                if profiles.active_untracked() == profile {
                    let share = made.share.clone();
                    plan.update(|doc| doc.add_share(share));
                }
            }
            done(result);
        });
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = (part, left_out, template);
            done(Err(share::ShareProblem::Unsupported));
        }
    }

    /// „Nicht mehr teilen", „Zurückziehen": the share's part goes at Phloem, its parcel is taken
    /// back, and its record goes. Kept where Phloem could not be reached.
    pub fn unshare(self, id: &str, done: impl FnOnce(Result<(), share::ShareProblem>) + 'static) {
        let (Some(profiles), Some(plan)) = (self.profiles, self.plan) else {
            done(Err(share::ShareProblem::Unsupported));
            return;
        };
        let profile = profiles.active_untracked();
        let Some(found) = plan.with_untracked(|doc| doc.share(id).cloned()) else {
            done(Ok(()));
            return;
        };
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        leptos::task::spawn_local(async move {
            let result = match browser::unshare(profile, &found).await {
                // The space is gone: nothing at Phloem shows it any more.
                Ok(()) | Err(share::ShareProblem::NotSynced) => Ok(()),
                Err(problem) => Err(problem),
            };
            if result.is_ok() && profiles.active_untracked() == profile {
                plan.update(|doc| doc.remove_share(&found.id));
            }
            done(result);
        });
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = (profile, found);
            done(Err(share::ShareProblem::Unsupported));
        }
    }

    /// Opens the part a link names (`#live=…`), without taking anything: what it shows now.
    pub fn open_live(self, link: &str, done: impl FnOnce(Result<(folia_calendar::live::LivePart, u64), share::ShareProblem>) + 'static) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let link = link.to_string();
            leptos::task::spawn_local(async move {
                done(browser::fetch_part(&link, 0).await.and_then(|found| found.ok_or(share::ShareProblem::Unknown)));
            });
        }
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = link;
            done(Err(share::ShareProblem::Unsupported));
        }
    }

    /// Opens a one-time link (`#once=…`): its parcel, once; what is in it.
    pub fn open_once(self, parcel: &str, done: impl FnOnce(Result<Opened, share::ShareProblem>) + 'static) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let parcel = parcel.to_string();
            leptos::task::spawn_local(async move {
                done(browser::open_once(&parcel).await.map(|opened| match opened {
                    browser::Opened::Live { link, part, rev } => Opened::Live { link, part, rev },
                    browser::Opened::Copy(part) => Opened::Copy(part),
                }));
            });
        }
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = parcel;
            done(Err(share::ShareProblem::Unsupported));
        }
    }

    /// A new share of the active profile's Merkliste, or of one of its collections (the dialog of
    /// the Merkliste, as of a calendar): its part from the list as it is, put at Phloem, and its
    /// record beside the collections. `done` hears the link's path or why not.
    pub fn share_list(self, request: ListShareRequest, done: impl FnOnce(Result<ListMade, share::ShareProblem>) + 'static) {
        let (Some(profiles), Some(marks), Some(collections)) = (self.profiles, self.marks, self.collections) else {
            done(Err(share::ShareProblem::Unsupported));
            return;
        };
        let profile = profiles.active_untracked();
        if !syncs(profile) {
            done(Err(share::ShareProblem::NotSynced));
            return;
        }
        let template = request.template();
        let ids: Vec<String> = marks.marks_untracked().into_iter().map(|mark| mark.id).collect();
        let (part, full) = collections.with_untracked(|shelf| (shelf.list_part(&template, &ids), shelf.shares().len() >= crate::collections::MAX_LIST_SHARES));
        if full {
            done(Err(share::ShareProblem::TooMany));
            return;
        }
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        leptos::task::spawn_local(async move {
            let result = browser::share_list(profile, template, part).await.map(|made| ListMade { share: made.share, path: made.path, until: made.until, left_out: made.left_out });
            if let Ok(made) = &result {
                if profiles.active_untracked() == profile {
                    let share = made.share.clone();
                    collections.update(|shelf| shelf.add_share(share));
                }
            }
            done(result);
        });
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = (part, template);
            done(Err(share::ShareProblem::Unsupported));
        }
    }

    /// „Nicht mehr teilen", „Zurückziehen" of a share of the Merkliste, as `unshare` of a
    /// calendar's.
    pub fn unshare_list(self, id: &str, done: impl FnOnce(Result<(), share::ShareProblem>) + 'static) {
        let (Some(profiles), Some(collections)) = (self.profiles, self.collections) else {
            done(Err(share::ShareProblem::Unsupported));
            return;
        };
        let profile = profiles.active_untracked();
        let Some(found) = collections.with_untracked(|shelf| shelf.share(id).cloned()) else {
            done(Ok(()));
            return;
        };
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        leptos::task::spawn_local(async move {
            let result = match browser::unshare_list(profile, &found).await {
                Ok(()) | Err(share::ShareProblem::NotSynced) => Ok(()),
                Err(problem) => Err(problem),
            };
            if result.is_ok() && profiles.active_untracked() == profile {
                collections.update(|shelf| shelf.remove_share(&found.id));
            }
            done(result);
        });
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = (profile, found);
            done(Err(share::ShareProblem::Unsupported));
        }
    }

    /// Opens the list a link of the Merkliste names (`/bookmarks#live=…`), without taking
    /// anything: the code of what it holds now (`bookmarks::ListPart`).
    pub fn open_list(self, link: &str, done: impl FnOnce(Result<String, share::ShareProblem>) + 'static) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let link = link.to_string();
            leptos::task::spawn_local(async move {
                done(browser::fetch_list(&link, 0).await.and_then(|found| found.map(|(code, _)| code).ok_or(share::ShareProblem::Unknown)));
            });
        }
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = link;
            done(Err(share::ShareProblem::Unsupported));
        }
    }

    /// Opens a one-time link of the Merkliste (`/bookmarks#once=…`): its parcel, once.
    pub fn open_once_list(self, parcel: &str, done: impl FnOnce(Result<OpenedList, share::ShareProblem>) + 'static) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        {
            let parcel = parcel.to_string();
            leptos::task::spawn_local(async move {
                done(browser::open_once_list(&parcel).await.map(|opened| match opened {
                    browser::OpenedList::Live { link, code } => OpenedList::Live { link, code },
                    browser::OpenedList::Copy(code) => OpenedList::Copy(code),
                }));
            });
        }
        #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
        {
            let _ = parcel;
            done(Err(share::ShareProblem::Unsupported));
        }
    }

    /// The calendars and the lists of the Merkliste the active profile follows are given what
    /// their share holds now (the page opened, or came into view): by the leader where the
    /// profile syncs, by this tab where it is private.
    pub fn refresh_follows(self) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        if let (Some(profiles), Some(plan)) = (self.profiles, self.plan) {
            if offered().is_none() {
                return;
            }
            let profile = profiles.active_untracked();
            let collections = self.collections;
            leptos::task::spawn_local(async move {
                browser::refresh_follows(profile, plan, profiles).await;
                if let Some(collections) = collections {
                    browser::refresh_lists(profile, collections, profiles).await;
                }
            });
        }
    }

    /// Asks what became of the active profile's one-time links (the dialog opened).
    pub fn check_parcels(self) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        if let Some(profiles) = self.profiles {
            browser::check_parcels(profiles.active_untracked());
        }
    }

    /// What two devices did at once in the active profile, the newest last. Tracked.
    /// How many parcels of the active profile another device opened while this tab led: once it
    /// grows, the device a profile went to (`give`, a link of `pair`) has fetched it. Tracked.
    pub fn taken(self) -> u32 {
        self.taken.get()
    }

    /// A parcel of the active profile was opened (Phloem's `pair` frame, `taken`).
    #[cfg_attr(not(all(feature = "csr", target_arch = "wasm32")), allow(dead_code))]
    pub(crate) fn parcel_taken(self) {
        self.taken.update(|n| *n = n.wrapping_add(1));
    }

    pub fn conflicts(self) -> Vec<Conflict> {
        self.conflicts.get()
    }

    /// The hint is read: it goes (taking the other version over is a change of the store, made
    /// with the store's own methods before), in every tab.
    pub fn dismiss(self, conflict: &Conflict) {
        #[cfg(all(feature = "csr", target_arch = "wasm32"))]
        if let Some(profiles) = self.profiles {
            browser::dismiss(profiles.active_untracked(), conflict);
        }
        self.conflicts.update(|all| all.retain(|c| c != conflict));
    }
}

/// What a new share is to be (the dialog's choices).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareRequest {
    /// 0 the plan's calendar, else a calendar of one's own.
    pub calendar: u8,
    pub to: folia_plans::shares::ShareTo,
    /// The semester shown.
    pub semester: SemesterKey,
    pub live: bool,
    pub once: bool,
    pub own: bool,
    pub notes: bool,
    /// The program whose abbreviations name the modules (the page's).
    pub program: Option<String>,
    /// „Für wen".
    pub name: String,
}

impl ShareRequest {
    fn template(&self) -> folia_plans::shares::Share {
        folia_plans::shares::Share {
            id: String::new(),
            calendar: self.calendar,
            to: self.to,
            semester: self.semester,
            made: 0,
            live: self.live,
            once: self.once && self.to == folia_plans::shares::ShareTo::Person,
            own: self.own,
            notes: self.notes,
            key: String::new(),
            parcel: String::new(),
            program: self.program.clone(),
            name: self.name.clone(),
        }
    }
}

/// What a new share of the Merkliste is to be (its dialog's choices).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListShareRequest {
    /// The collection; 0: the whole Merkliste.
    pub collection: u32,
    pub live: bool,
    /// For one person: a one-time link.
    pub once: bool,
    pub notes: bool,
    /// „Für wen".
    pub name: String,
}

impl ListShareRequest {
    fn template(&self) -> crate::collections::ListShare {
        crate::collections::ListShare {
            id: String::new(),
            collection: self.collection,
            made: 0,
            live: self.live,
            once: self.once,
            notes: self.notes,
            key: String::new(),
            parcel: String::new(),
            name: crate::collections::clean_name(&self.name).chars().take(crate::collections::MAX_SHARE_NAME).collect(),
        }
    }
}

/// What a new share of the Merkliste made: its record, the path of its link
/// (`/bookmarks#live=…`, `/bookmarks#once=…`), until when a one-time link opens (ms), and how many
/// notes did not fit in its part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListMade {
    pub share: crate::collections::ListShare,
    pub path: String,
    pub until: Option<u64>,
    pub left_out: usize,
}

/// What a one-time link of the Merkliste opened to: a list that stays current (its link, the code
/// of what it holds now) or the code of a copy (`bookmarks::ListPart`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenedList {
    Live { link: String, code: String },
    Copy(String),
}

/// What a new share made: its record, the path of its link (`/studyplan#live=…`,
/// `/studyplan#once=…`, `/calendar/s/….ics`), until when a one-time link opens (ms), and how many
/// notes or Termine of one's own did not fit in its part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Made {
    pub share: folia_plans::shares::Share,
    pub path: String,
    pub until: Option<u64>,
    pub left_out: usize,
}

/// What a one-time link opened to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opened {
    /// A calendar that stays current: its link (part id and key), what it holds now, its version.
    Live { link: String, part: folia_calendar::live::LivePart, rev: u64 },
    /// A copy of then.
    Copy(folia_calendar::live::LivePart),
}

/// Milliseconds since 1970, as this browser knows them (the server's clock where it heard it).
fn now_ms() -> u64 {
    #[cfg(all(feature = "csr", target_arch = "wasm32"))]
    return js_sys::Date::now() as u64;
    #[cfg(not(all(feature = "csr", target_arch = "wasm32")))]
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
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
