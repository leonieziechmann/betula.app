//! Profiles: several sets of what a visitor keeps, side by side in one browser (owner, 2026-10-14:
//! „eine Profile funktion, so dass man mehrere Profile in Betula anlegen kann"): a second program
//! studied beside the first, a change of program tried out apart from the real plan, a browser two
//! people share. Each profile has its own Merkliste (`bookmarks`), Stundenplan with „Mein Studium"
//! and the saved plans (`studyplan`) and „Mein Studiengang" (`myprogram`). What is a setting of the
//! browser — the language, light or dark, the widths of the panels, what the finder compares
//! (R13) — is the same in every profile.
//!
//! Like everything a visitor keeps, the profiles live in this browser alone (R20): there is no
//! account, and no request or address carries a profile. The list is `betula.profiles.v1`, the
//! active profile and one line per profile, the fields separated by a tab (two spaces here):
//!
//! ```text
//! active   2
//! profile  1  0           Informatik
//! profile  2  1760443200  Zweitstudium
//! ```
//!
//! — its number (1–999), when it was made in seconds since 1970 (0 where unknown) and its name
//! (none: „Profil 2", `Profile::shown`). A profile's stores keep their keys with its number after
//! `betula.` (`betula.p2.bookmarks.v1`, `key_of`); the first profile keeps the plain keys, the ones
//! the browser kept before there were profiles. So nothing moves when profiles come, and a build
//! without them (a rollback, an old tab of the installed app) shows the first profile as it is.
//!
//! The list is read like anything from outside: a line that fails is dropped alone, a profile
//! counts once, no more than `MAX_PROFILES` are read, and it never reads as no profile — without a
//! text it is the first profile alone, which is stored as no text at all. What stands under the
//! plain keys always has its profile: where the list does not name the first one (it was deleted,
//! and a build without profiles wrote there since), the first is listed again.
//!
//! Which profile is active belongs to the browser, not to a tab: a switch in one tab switches every
//! tab, as a mark set in one shows in the others. The stores follow it (`Kept`): they read the new
//! profile's texts when it changes, here or in another tab, and write under the key of the profile
//! whose text they hold, never under another one's.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use folia_locale::Locale;
use folia_plans::studyplan::{MineDoc, PlanDoc, SavedPlans};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use folia_design::nav;

/// The list of profiles and the active one (the script in `<head>` reads it too,
/// `folia_shell::document::HEAD_SCRIPT`).
pub const STORAGE_KEY: &str = "betula.profiles.v1";

/// The number of the first profile, whose stores keep the plain keys.
pub const FIRST: u16 = 1;

/// The most profiles. A profile keeps at most about a megabyte (the caps of its plan and of its
/// saved plans, `folia_plans::studyplan::MAX_STORED`, `MAX_SAVED_STORED`), a real one a few
/// kilobytes, and a browser keeps around five megabytes for a site.
pub const MAX_PROFILES: usize = 8;

/// The most characters of a profile's name.
pub const MAX_NAME: usize = 40;

/// The largest number of a profile: three digits keep a key short.
const MAX_ID: u16 = 999;

/// What a profile keeps, under the keys of the first profile (`key_of` makes another profile's):
/// the Merkliste, the Stundenplan with „Mein Studium", the saved plans and „Mein Studiengang".
pub const KEYS: [&str; STORES] = [crate::bookmarks::STORAGE_KEY, crate::studyplan::STORAGE_KEY, crate::studyplan::SAVED_KEY, crate::myprogram::STORAGE_KEY, crate::collections::STORAGE_KEY];

/// How many stores a profile has (`KEYS`): the Merkliste, the Stundenplan, its saved plans, „Mein
/// Studiengang", and the Merkliste's collections and notes (since Folia 1.35.0).
pub const STORES: usize = 5;

/// What a profile keeps in a tab for the session alone (`sessionStorage`), under the keys of the
/// first profile like `KEYS`: the Stundenplan's own program (`studyplan`). It is no part of a
/// profile's file, and it goes when its profile is emptied or deleted (`forget_session`).
pub const SESSION_KEYS: [&str; 1] = [crate::studyplan::PROGRAM_KEY];

/// Takes out what this tab keeps for the session in profile `id` (`SESSION_KEYS`): emptied,
/// deleted, or made anew under a number a deleted profile had.
fn forget_session(id: u16) {
    for key in SESSION_KEYS {
        nav::session_set(&key_of(key, id), "");
    }
}

/// The key under which profile `id` keeps what the first profile keeps under `key`:
/// `betula.bookmarks.v1` is `betula.p2.bookmarks.v1` for the second.
pub fn key_of(key: &str, id: u16) -> String {
    match key.strip_prefix("betula.") {
        Some(rest) if id != FIRST => format!("betula.p{id}.{rest}"),
        _ => key.to_string(),
    }
}

/// A profile of this browser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// 1–999, one profile's alone; the first is `FIRST`.
    pub id: u16,
    /// As the visitor named it (`name_of`); empty for one without a name.
    pub name: String,
    /// When it was made, in seconds since 1970; 0 where unknown (the first profile, which the
    /// browser had before there were profiles).
    pub at: u64,
}

impl Profile {
    /// Its name, or „Profil 2" where it has none.
    pub fn shown(&self, locale: Locale) -> String {
        if self.name.is_empty() {
            (crate::i18n::texts(locale).profiles.unnamed)(self.id)
        } else {
            self.name.clone()
        }
    }

    /// What its round badge shows: the first letter or digit of its name, else its number.
    pub fn initial(&self) -> String {
        self.name.chars().find(|c| c.is_alphanumeric()).map_or_else(|| self.id.to_string(), |c| c.to_uppercase().collect())
    }

    /// Its colour, one of the eight soft hues of the design (app.css, `.avatar[data-tone]`), by
    /// its number: profiles made one after the other differ.
    pub fn tone(&self) -> u16 {
        self.id.saturating_sub(1) % 8 + 1
    }
}

/// A profile's round badge: its initial (`Profile::initial`) in its colour (`Profile::tone`).
/// Drawing only: what names the profile is beside it, or the label of the control it is in.
#[component]
pub fn Avatar(#[prop(into)] profile: Signal<Profile>, #[prop(optional)] large: bool) -> impl IntoView {
    view! {
        <span class="avatar" class:large=large data-tone=move || profile.with(|profile| profile.tone().to_string()) aria-hidden="true">
            {move || profile.with(Profile::initial)}
        </span>
    }
}

/// The first profile as the browser had it before there were profiles.
fn first() -> Profile {
    Profile { id: FIRST, name: String::new(), at: 0 }
}

/// The profiles of this browser and the active one.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Registry {
    /// In the order they were made; never empty.
    profiles: Vec<Profile>,
    active: u16,
}

impl Default for Registry {
    fn default() -> Self {
        Registry { profiles: vec![first()], active: FIRST }
    }
}

impl Registry {
    /// The list of a stored text (see the module). `first_keeps`: something stands under the plain
    /// keys, so the first profile is listed whatever the text says.
    fn restored(text: &str, first_keeps: bool) -> Self {
        let mut profiles: Vec<Profile> = Vec::new();
        let mut active = None;
        for line in text.lines() {
            let mut fields = line.split('\t');
            match fields.next().map(str::trim) {
                Some("active") => {
                    if active.is_none() {
                        active = fields.next().and_then(id_of);
                    }
                }
                Some("profile") => {
                    let Some(id) = fields.next().and_then(id_of) else { continue };
                    if profiles.len() == MAX_PROFILES || profiles.iter().any(|profile| profile.id == id) {
                        continue;
                    }
                    let at = fields.next().and_then(|at| at.trim().parse().ok()).unwrap_or(0);
                    profiles.push(Profile { id, name: name_of(fields.next().unwrap_or_default()), at });
                }
                _ => {}
            }
        }
        if profiles.is_empty() || (first_keeps && !profiles.iter().any(|profile| profile.id == FIRST)) {
            profiles.insert(0, first());
        }
        let active = active.filter(|id| profiles.iter().any(|profile| profile.id == *id)).or_else(|| profiles.first().map(|profile| profile.id)).unwrap_or(FIRST);
        Registry { profiles, active }
    }

    /// The text `restored` reads back as this. None for the first profile alone, unnamed: the
    /// browser as it was before there were profiles keeps no list.
    fn stored(&self) -> String {
        if *self == Registry::default() {
            return String::new();
        }
        let mut out = format!("active\t{}\n", self.active);
        for profile in &self.profiles {
            let _ = writeln!(out, "profile\t{}\t{}\t{}", profile.id, profile.at, name_of(&profile.name));
        }
        out
    }

    fn has(&self, id: u16) -> bool {
        self.profiles.iter().any(|profile| profile.id == id)
    }

    fn get(&self, id: u16) -> Option<&Profile> {
        self.profiles.iter().find(|profile| profile.id == id)
    }

    /// The number a new profile gets: the lowest that is free, after the first's (whose keys are
    /// the plain ones: they stay the first profile's).
    fn next_id(&self) -> Option<u16> {
        (FIRST + 1..=MAX_ID).find(|id| !self.has(*id))
    }

    /// With a new profile at the end, which becomes the active one; `None` with `MAX_PROFILES`.
    fn with_new(&self, name: &str, at: u64) -> Option<(Registry, u16)> {
        if self.profiles.len() >= MAX_PROFILES {
            return None;
        }
        let id = self.next_id()?;
        let mut next = self.clone();
        next.profiles.push(Profile { id, name: name_of(name), at });
        next.active = id;
        Some((next, id))
    }

    /// Without profile `id`, and where it stood; `None` for the only profile and one that is not
    /// there. Was it the active one, the profile before it becomes active, else the one after it.
    fn without(&self, id: u16) -> Option<(Registry, Profile, usize)> {
        if self.profiles.len() < 2 {
            return None;
        }
        let at = self.profiles.iter().position(|profile| profile.id == id)?;
        let mut next = self.clone();
        let profile = next.profiles.remove(at);
        if next.active == id {
            next.active = next.profiles.get(at.saturating_sub(1)).map_or(FIRST, |profile| profile.id);
        }
        Some((next, profile, at))
    }

    /// With a deleted profile back where it stood, under its number where that is still free
    /// (else the next free one), active again where it was; `None` with `MAX_PROFILES`.
    fn with_back(&self, deleted: &Deleted) -> Option<(Registry, u16)> {
        if self.profiles.len() >= MAX_PROFILES {
            return None;
        }
        let id = if self.has(deleted.profile.id) { self.next_id()? } else { deleted.profile.id };
        let mut next = self.clone();
        next.profiles.insert(deleted.at.min(next.profiles.len()), Profile { id, ..deleted.profile.clone() });
        if deleted.was_active {
            next.active = id;
        }
        Some((next, id))
    }

    /// `name`, or where a profile is called so already „name (2)", „name (3)" …
    fn fresh(&self, name: &str) -> String {
        let taken = |name: &str| self.profiles.iter().any(|profile| profile.name == name);
        if name.is_empty() || !taken(name) {
            return name.to_string();
        }
        (2..=MAX_PROFILES + 1).map(|n| name_of(&format!("{name} ({n})"))).find(|name| !taken(name)).unwrap_or_else(|| name.to_string())
    }
}

/// A profile's number as the list writes it: 1–999, digits only, no leading zero.
fn id_of(text: &str) -> Option<u16> {
    let text = text.trim();
    let digits = !text.is_empty() && text.len() <= 3 && text.bytes().all(|b| b.is_ascii_digit()) && !text.starts_with('0');
    digits.then(|| text.parse().ok()).flatten().filter(|id| (FIRST..=MAX_ID).contains(id))
}

/// A name as a profile keeps it: what is no word as a space (a tab or a line break would split its
/// line), the spaces folded, at most `MAX_NAME` characters.
pub fn name_of(text: &str) -> String {
    let name = text.split(|c: char| c.is_whitespace() || c.is_control()).filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ");
    match name.char_indices().nth(MAX_NAME) {
        Some((at, _)) => name.get(..at).unwrap_or_default().trim_end().to_string(),
        None => name,
    }
}

/// The profiles of this browser, shared through context: the first alone on the server, always
/// (R9).
#[derive(Clone, Copy)]
pub struct Profiles {
    registry: RwSignal<Registry>,
    /// Counts what this module writes into a profile's stores itself (emptied, put back): the
    /// stores read their texts again (`Kept::follow`).
    written: RwSignal<u32>,
}

impl Profiles {
    /// Reads the list this browser keeps and provides it; another tab may change it (another
    /// profile chosen, one made or deleted there), and this one follows. Call it before the stores
    /// are made: they keep the active profile's texts.
    pub fn provide() -> Self {
        let profiles = Profiles { registry: RwSignal::new(load()), written: RwSignal::new(0) };
        provide_context(profiles);
        // Effects run in the browser only.
        Effect::new(move |_| {
            let handle = window_event_listener_untyped("storage", move |_| {
                let stored = load();
                if profiles.registry.with_untracked(|now| *now != stored) {
                    profiles.registry.set(stored);
                }
            });
            on_cleanup(move || handle.remove());
        });
        profiles
    }

    pub fn expect() -> Option<Self> {
        use_context::<Profiles>()
    }

    /// The active profile's number. Tracked.
    pub fn active(self) -> u16 {
        self.registry.with(|registry| registry.active)
    }

    pub fn active_untracked(self) -> u16 {
        self.registry.with_untracked(|registry| registry.active)
    }

    /// The active profile. Tracked.
    pub fn current(self) -> Profile {
        self.registry.with(|registry| registry.get(registry.active).cloned().unwrap_or_else(first))
    }

    /// Every profile, in the order they were made. Tracked.
    pub fn all(self) -> Vec<Profile> {
        self.registry.with(|registry| registry.profiles.clone())
    }

    /// How many profiles there are. Tracked.
    pub fn count(self) -> usize {
        self.registry.with(|registry| registry.profiles.len())
    }

    /// Whether another profile can be made (`MAX_PROFILES`). Tracked.
    pub fn room(self) -> bool {
        self.count() < MAX_PROFILES
    }

    /// Makes profile `id` the active one, in every tab of the browser.
    pub fn switch(self, id: u16) {
        let mut next = self.registry.get_untracked();
        if next.has(id) && next.active != id {
            next.active = id;
            self.store(next);
        }
    }

    /// Makes a new, empty profile and switches to it; its number, or `None` with `MAX_PROFILES`.
    pub fn create(self, name: &str) -> Option<u16> {
        self.create_with(name, &Default::default())
    }

    /// Makes a profile of what a file brings (`Backup`) and switches to it. Its name is the file's,
    /// told apart from a profile of the same name („Informatik (2)").
    pub fn create_from(self, backup: &Backup) -> Option<u16> {
        let name = self.registry.with_untracked(|registry| registry.fresh(&backup.name));
        self.create_with(&name, &backup.texts)
    }

    fn create_with(self, name: &str, texts: &[String; STORES]) -> Option<u16> {
        let (next, id) = self.registry.with_untracked(|registry| registry.with_new(name, crate::bookmarks::now()))?;
        // Its texts first, then the list: another tab that hears of the profile finds them. What a
        // profile of this number left behind (keys a browser would not remove) is overwritten.
        for (key, text) in KEYS.iter().zip(texts) {
            nav::local_set(&key_of(key, id), text);
        }
        forget_session(id);
        self.store(next);
        Some(id)
    }

    /// Names profile `id` anew (`name_of`; empty: „Profil 2").
    pub fn rename(self, id: u16, name: &str) {
        let mut next = self.registry.get_untracked();
        if let Some(profile) = next.profiles.iter_mut().find(|profile| profile.id == id) {
            profile.name = name_of(name);
        }
        if self.registry.with_untracked(|now| *now != next) {
            self.store(next);
        }
    }

    /// Deletes profile `id` with all it keeps, and hands it back for `undo_delete` (what the page
    /// keeps until it is left). `None` for the only profile: it can be emptied (`clear`), not
    /// deleted. Was it the active one, another one is now.
    pub fn delete(self, id: u16) -> Option<Deleted> {
        let (next, profile, at) = self.registry.with_untracked(|registry| registry.without(id))?;
        let was_active = self.active_untracked() == id;
        let texts = texts_of(id);
        // What it keeps goes first, then the profile: a tab that hears of the list without it
        // finds nothing left under its keys (and does not list the first profile again for it).
        for key in KEYS {
            nav::local_set(&key_of(key, id), "");
        }
        forget_session(id);
        self.store(next);
        Some(Deleted { profile, at, texts, was_active })
    }

    /// Puts a deleted profile back with all it kept („Rückgängig"); `false` where no profile can be
    /// made any more.
    pub fn undo_delete(self, deleted: &Deleted) -> bool {
        let Some((next, id)) = self.registry.with_untracked(|registry| registry.with_back(deleted)) else { return false };
        for (key, text) in KEYS.iter().zip(&deleted.texts) {
            nav::local_set(&key_of(key, id), text);
        }
        self.store(next);
        true
    }

    /// Empties profile `id`: its Merkliste, Stundenplan, „Mein Studium", saved plans and „Mein
    /// Studiengang" go, and the Stundenplan's program of the session; the profile stays. Hands back
    /// what it kept, for `undo_clear` (which puts back no program of the session: the Stundenplan
    /// then shows „Mein Studiengang" again).
    pub fn clear(self, id: u16) -> Cleared {
        let texts = texts_of(id);
        for key in KEYS {
            nav::local_set(&key_of(key, id), "");
        }
        forget_session(id);
        self.wrote();
        Cleared { id, texts }
    }

    /// Puts back what `clear` took („Rückgängig"), in place of what the profile was given since.
    pub fn undo_clear(self, cleared: &Cleared) {
        if !self.registry.with_untracked(|registry| registry.has(cleared.id)) {
            return;
        }
        for (key, text) in KEYS.iter().zip(&cleared.texts) {
            nav::local_set(&key_of(key, cleared.id), text);
        }
        self.wrote();
    }

    /// Profile `id` as a file (`Backup`), named as the page names it.
    pub fn backup(self, id: u16, locale: Locale) -> Option<Backup> {
        let profile = self.registry.with_untracked(|registry| registry.get(id).cloned())?;
        Some(Backup { name: profile.shown(locale), texts: texts_of(id) })
    }

    /// „Mein Studiengang" of profile `id` as it was named when it was set (else its id), as this
    /// browser has it; the line under a profile in a list of them. Untracked.
    pub fn program_of(self, id: u16) -> Option<String> {
        let mine = MineDoc::restored(&nav::local_get(&key_of(crate::myprogram::STORAGE_KEY, id)).unwrap_or_default());
        mine.name.or(mine.program)
    }

    /// The line under each profile wherever the profiles are listed (the profile page, the menu of
    /// the way to it): its „Mein Studiengang" (`program_of`), the active one's as its store has it
    /// now, else „Kein Studiengang". Tracked.
    pub fn program_lines(self, locale: Locale) -> Vec<(u16, String)> {
        let active = self.active();
        let own = crate::myprogram::MyProgram::expect().and_then(|mine| mine.with(|doc| doc.name.clone().or_else(|| doc.program.clone())));
        let none = crate::i18n::texts(locale).profiles.no_program;
        self.registry.with(|registry| {
            registry
                .profiles
                .iter()
                .map(|profile| {
                    let line = if profile.id == active { own.clone() } else { self.program_of(profile.id) };
                    (profile.id, line.unwrap_or_else(|| none.to_string()))
                })
                .collect()
        })
    }

    /// The number the next profile made gets, for the name it has until it is named („Profil 3");
    /// `None` with `MAX_PROFILES`. Tracked.
    pub fn next_id(self) -> Option<u16> {
        self.registry.with(|registry| registry.with_new("", 0).map(|(_, id)| id))
    }

    fn store(self, registry: Registry) {
        nav::local_set(STORAGE_KEY, &registry.stored());
        self.registry.set(registry);
    }

    fn wrote(self) {
        self.written.update(|written| *written = written.wrapping_add(1));
    }
}

/// The list this browser keeps. The first profile alone on the server and where storage is
/// refused.
fn load() -> Registry {
    let first_keeps = KEYS.iter().any(|key| nav::local_get(key).is_some_and(|text| !text.trim().is_empty()));
    Registry::restored(&nav::local_get(STORAGE_KEY).unwrap_or_default(), first_keeps)
}

/// What profile `id` keeps, as this browser has it: the text of each of `KEYS`, empty for none.
fn texts_of(id: u16) -> [String; STORES] {
    KEYS.map(|key| nav::local_get(&key_of(key, id)).unwrap_or_default())
}

/// A deleted profile with all it kept, until the page is left: „Rückgängig" puts it back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deleted {
    pub profile: Profile,
    /// Where it stood in the list.
    at: usize,
    texts: [String; STORES],
    was_active: bool,
}

/// What an emptied profile kept, until the page is left: „Rückgängig" puts it back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cleared {
    id: u16,
    texts: [String; STORES],
}

/// Where a store of `KEYS` keeps its text: its key in the profile whose text it holds, which follows
/// the active profile. A store reads and writes through it, so it never writes one profile's text
/// under another one's key: after a switch it writes under the old key until it has read the new
/// profile's text (`follow`), which happens before a visitor can change anything.
#[derive(Clone, Copy)]
pub(crate) struct Kept {
    key: &'static str,
    /// The profile whose text the store holds.
    profile: RwSignal<u16>,
}

impl Kept {
    /// The store `key` (one of `KEYS`) in the active profile; in the first where there are no
    /// profiles (a test).
    pub(crate) fn of(key: &'static str) -> Self {
        let profile = Profiles::expect().map_or(FIRST, Profiles::active_untracked);
        Kept { key, profile: RwSignal::new(profile) }
    }

    /// The text this browser keeps for the store; `None` on the server and where storage is
    /// refused.
    pub(crate) fn read(self) -> Option<String> {
        nav::local_get(&key_of(self.key, self.profile.get_untracked()))
    }

    /// Keeps the store's text; an empty one takes the key out (`nav::local_set`).
    pub(crate) fn write(self, text: &str) {
        nav::local_set(&key_of(self.key, self.profile.get_untracked()), text);
    }

    /// What this tab keeps beside the store's text for the session (`sessionStorage`, one of
    /// `SESSION_KEYS`), in the profile whose text the store holds; `None` on the server.
    pub(crate) fn session_read(self, key: &str) -> Option<String> {
        nav::session_get(&key_of(key, self.profile.get_untracked()))
    }

    /// Keeps it for the session; an empty one takes the key out (`nav::session_set`).
    pub(crate) fn session_write(self, key: &str, text: &str) {
        nav::session_set(&key_of(key, self.profile.get_untracked()), text);
    }

    /// The profile whose text the store holds. Tracked.
    pub(crate) fn profile(self) -> u16 {
        self.profile.get()
    }

    pub(crate) fn profile_untracked(self) -> u16 {
        self.profile.get_untracked()
    }

    /// Calls `reload` whenever the store's text may have changed under it: another tab wrote it,
    /// another profile became the active one (here or in another tab), or `Profiles` wrote it
    /// itself (emptied, put back). `reload` reads the text again (`read`) and sets it where it
    /// differs. Effects run in the browser only.
    pub(crate) fn follow(self, reload: impl Fn() + Clone + 'static) {
        let heard = reload.clone();
        Effect::new(move |_| {
            let reload = heard.clone();
            let handle = window_event_listener_untyped("storage", move |_| reload());
            on_cleanup(move || handle.remove());
        });
        let Some(profiles) = Profiles::expect() else { return };
        Effect::new(move |before: Option<()>| {
            let active = profiles.active();
            profiles.written.track();
            let moved = self.profile.get_untracked() != active;
            if moved {
                self.profile.set(active);
            }
            // The first run is the store's making, which has read its text, unless the profile
            // changed meanwhile.
            if moved || before.is_some() {
                reload();
            }
        });
    }
}

/// What a backup file says it is (`"betula": "profile"`), and the layout it is written in. A change
/// beyond adding a field takes the next layout; a file of a later one is not read (`NoBackup::Newer`).
const BACKUP_KIND: &str = "profile";
const BACKUP_VERSION: u32 = 1;

/// The largest file read as a backup: what the caps of a profile's stores allow, written as JSON
/// (a tab or a line break takes two characters there), with room.
pub const MAX_BACKUP: usize = 4 * 1024 * 1024;

/// A backup file as JSON has it.
#[derive(Serialize, Deserialize)]
struct BackupFile {
    betula: String,
    version: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    stores: BTreeMap<String, String>,
}

/// A profile as a file („Als Datei sichern"): to keep it apart from the browser, which may lose its
/// data, or to take it to another device without a server in between. The file is made in the
/// browser and read there (`folia_profile`); it reaches no server. JSON:
///
/// ```text
/// {"betula": "profile", "version": 1, "name": "Zweitstudium",
///  "stores": {"betula.bookmarks.v1": "12204\t1758391200\n", "betula.myprogram.v1": "program\t…", …}}
/// ```
///
/// Its stores are named by the first profile's keys, whichever profile they come from, and hold
/// what the stores write. A file read back is checked store by store, as each store reads its text
/// (`checked`): what fails is dropped line by line, and a store the file does not name is empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Backup {
    pub name: String,
    /// The text of each of `KEYS`.
    pub texts: [String; STORES],
}

/// A file that is no backup this build reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoBackup {
    /// No backup of Betula, or one cut short.
    Unreadable,
    /// One of a later layout than this build reads.
    Newer,
    /// Larger than a backup can be (`MAX_BACKUP`).
    TooLarge,
}

impl Backup {
    /// The file's text.
    pub fn file(&self) -> String {
        let stores = KEYS.iter().zip(&self.texts).filter(|(_, text)| !text.is_empty()).map(|(key, text)| (key.to_string(), text.clone())).collect();
        serde_json::to_string_pretty(&BackupFile { betula: BACKUP_KIND.to_string(), version: BACKUP_VERSION, name: self.name.clone(), stores }).unwrap_or_default()
    }

    /// The backup a file holds, read like anything from outside.
    pub fn read(text: &str) -> Result<Self, NoBackup> {
        if text.len() > MAX_BACKUP {
            return Err(NoBackup::TooLarge);
        }
        let file: BackupFile = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| NoBackup::Unreadable)?;
        if file.betula != BACKUP_KIND || file.version == 0 {
            return Err(NoBackup::Unreadable);
        }
        if file.version > BACKUP_VERSION {
            return Err(NoBackup::Newer);
        }
        let texts = KEYS.map(|key| checked(key, file.stores.get(key).map(String::as_str).unwrap_or_default()));
        Ok(Backup { name: name_of(&file.name), texts })
    }

    /// What it keeps, counted.
    pub fn summary(&self) -> Summary {
        Summary::of(&self.texts)
    }
}

/// A store's text as its store reads it and writes it back (`KEYS`).
fn checked(key: &str, text: &str) -> String {
    if key == crate::bookmarks::STORAGE_KEY {
        crate::bookmarks::checked(text)
    } else if key == crate::studyplan::STORAGE_KEY {
        // The Stundenplan's program is the session's, never the stored plan's.
        PlanDoc::restored(text).stored_without_program()
    } else if key == crate::studyplan::SAVED_KEY {
        SavedPlans::restored(text).stored()
    } else if key == crate::myprogram::STORAGE_KEY {
        MineDoc::restored(text).stored()
    } else if key == crate::collections::STORAGE_KEY {
        crate::collections::checked(text)
    } else {
        String::new()
    }
}

/// What a profile keeps, counted: what a backup offered from a file holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Modules on the Merkliste.
    pub marked: usize,
    /// Modules in the Stundenplan (each once, however many semesters it is planned into), and the
    /// semesters they are in.
    pub planned: usize,
    pub semesters: usize,
    /// Modules passed („Mein Studium").
    pub passed: usize,
    /// Plans saved („Plan speichern").
    pub saved: usize,
    /// „Mein Studiengang" as it was named when it was set, else its id.
    pub program: Option<String>,
    /// The Merkliste's collections, and its modules with a note.
    pub collections: usize,
    pub notes: usize,
}

impl Summary {
    fn of(texts: &[String; STORES]) -> Self {
        let [marks, plan, saved, mine, shelf] = texts;
        let plan = PlanDoc::restored(plan);
        let mine = MineDoc::restored(mine);
        let (collections, notes) = crate::collections::count(shelf);
        Summary {
            collections,
            notes,
            marked: crate::bookmarks::count(marks),
            planned: plan.modules.iter().map(|planned| planned.module_id.as_str()).collect::<BTreeSet<_>>().len(),
            semesters: plan.semesters().len(),
            passed: plan.passed.len(),
            saved: SavedPlans::restored(saved).plans.len(),
            program: mine.name.or(mine.program),
        }
    }

    /// Nothing kept at all.
    pub fn is_empty(&self) -> bool {
        *self == Summary::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: u16, name: &str, at: u64) -> Profile {
        Profile { id, name: name.to_string(), at }
    }

    #[test]
    fn the_first_profile_keeps_the_plain_keys() {
        assert_eq!(key_of("betula.bookmarks.v1", FIRST), "betula.bookmarks.v1");
        assert_eq!(key_of("betula.bookmarks.v1", 2), "betula.p2.bookmarks.v1");
        assert_eq!(key_of("betula.studyplan.saved.v1", 17), "betula.p17.studyplan.saved.v1");
        // Every key of a profile is another key than every key of every other profile.
        let mut all = BTreeSet::new();
        for id in FIRST..=MAX_ID {
            for key in KEYS {
                assert!(all.insert(key_of(key, id)), "{key} of {id}");
            }
        }
        // And none of them is a key of the browser's settings or of the list.
        for setting in ["betula.profiles.v1", "betula.theme", "betula.language", "betula.finder", "betula.tabs", "betula.plan.shape", "betula.study.view"] {
            assert!(!all.contains(setting), "{setting}");
        }
    }

    #[test]
    fn what_is_stored_is_read_like_anything_from_outside() {
        // No text: the first profile alone, active, and it writes no text either.
        assert_eq!(Registry::restored("", false), Registry::default());
        assert_eq!(Registry::default().stored(), "");

        let registry = Registry { profiles: vec![profile(1, "Informatik", 0), profile(2, "Zweitstudium", 1_760_443_200)], active: 2 };
        assert_eq!(registry.stored(), "active\t2\nprofile\t1\t0\tInformatik\nprofile\t2\t1760443200\tZweitstudium\n");
        assert_eq!(Registry::restored(&registry.stored(), true), registry);

        // Numbers that cannot be one, a profile twice, a time that is none, a name with a tab and
        // spaces in it, an active profile that is not listed.
        let read = Registry::restored("profile\t0\t1\tNull\nprofile\t1000\t1\tZu groß\nprofile\t07\t1\tNull\nprofile\tx\t1\tx\nprofile\t3\tgestern\t  Erst  \t semester\nprofile\t3\t5\tNoch mal\nactive\t9\nactive\t3\n\u{0}\n", false);
        assert_eq!(read, Registry { profiles: vec![profile(3, "Erst", 0)], active: 3 });
        // Nothing that reads as a profile: the first alone.
        assert_eq!(Registry::restored("garbage\nactive\t2\n", false), Registry::default());
        // No more than MAX_PROFILES.
        let many: String = (1..=MAX_PROFILES as u16 + 5).map(|id| format!("profile\t{id}\t0\t\n")).collect();
        assert_eq!(Registry::restored(&many, false).profiles.len(), MAX_PROFILES);
    }

    #[test]
    fn what_stands_under_the_plain_keys_always_has_its_profile() {
        // The first profile was deleted, and a build without profiles wrote under its keys since.
        let without_first = "active\t2\nprofile\t2\t5\tZweitstudium\n";
        assert_eq!(Registry::restored(without_first, false).profiles, vec![profile(2, "Zweitstudium", 5)]);
        let back = Registry::restored(without_first, true);
        assert_eq!((back.profiles, back.active), (vec![first(), profile(2, "Zweitstudium", 5)], 2));
    }

    #[test]
    fn names_are_one_line_and_not_too_long() {
        assert_eq!(name_of("  Zweit\tstudium \n "), "Zweit studium");
        assert_eq!(name_of("\u{7}"), "");
        let long = "Ä".repeat(MAX_NAME + 10);
        assert_eq!(name_of(&long).chars().count(), MAX_NAME);
        // Unnamed: its number, in the page's language; its badge the number too.
        let unnamed = profile(2, "", 0);
        assert_eq!((unnamed.shown(Locale::De), unnamed.shown(Locale::En), unnamed.initial()), ("Profil 2".to_string(), "Profile 2".to_string(), "2".to_string()));
        // Named: its name; its badge the first letter or digit of it, whatever stands before.
        let named = profile(3, "„élan vital“", 0);
        assert_eq!((named.shown(Locale::En), named.initial()), ("„élan vital“".to_string(), "É".to_string()));
        assert_eq!(profile(4, "2. Studium", 0).initial(), "2");
        // Eight hues, by number.
        assert_eq!((profile(1, "", 0).tone(), profile(8, "", 0).tone(), profile(9, "", 0).tone()), (1, 8, 1));
    }

    #[test]
    fn profiles_are_made_deleted_and_put_back() {
        let one = Registry::default();
        // The only profile is not deleted.
        assert_eq!(one.without(FIRST), None);
        let (two, id) = one.with_new("  Zweitstudium ", 7).unwrap();
        assert_eq!((id, two.active, two.get(2).cloned()), (2, 2, Some(profile(2, "Zweitstudium", 7))));
        let (three, id) = two.with_new("", 8).unwrap();
        assert_eq!((id, three.active), (3, 3));

        // The active profile deleted: the one before it becomes active.
        let (after, gone, at) = three.without(3).unwrap();
        assert_eq!((after.active, gone.id, at), (2, 3, 2));
        // The first deleted while another is active: that one stays; a number is free again, and the
        // first's stays the first's.
        let (no_first, _, at) = three.without(FIRST).unwrap();
        assert_eq!((no_first.active, at, no_first.next_id()), (3, 0, Some(4)));
        let (no_first_two, _, _) = no_first.without(2).unwrap();
        assert_eq!(no_first_two.next_id(), Some(2));
        // The first deleted while it is active: the one after it.
        let first_active = Registry { active: FIRST, ..three.clone() };
        assert_eq!(first_active.without(FIRST).unwrap().0.active, 2);
        assert_eq!(three.without(9), None);

        // Put back where it stood, active again where it was, under its number while it is free.
        let deleted = Deleted { profile: profile(2, "Zweitstudium", 7), at: 1, texts: Default::default(), was_active: true };
        let (alone, _, _) = three.without(2).unwrap();
        let (back, id) = alone.with_back(&deleted).unwrap();
        assert_eq!((id, back.active, back.profiles.iter().map(|p| p.id).collect::<Vec<_>>()), (2, 2, vec![1, 2, 3]));
        // Its number taken meanwhile: the next free one.
        let (back, id) = three.with_back(&deleted).unwrap();
        assert_eq!((id, back.profiles.iter().map(|p| p.id).collect::<Vec<_>>()), (4, vec![1, 4, 2, 3]));

        // No more than MAX_PROFILES.
        let mut full = Registry::default();
        while let Some((next, _)) = full.with_new("x", 0) {
            full = next;
        }
        assert_eq!((full.profiles.len(), full.with_back(&deleted)), (MAX_PROFILES, None));
    }

    #[test]
    fn a_name_taken_is_told_apart() {
        let registry = Registry { profiles: vec![profile(1, "Informatik", 0), profile(2, "Informatik (2)", 0)], active: 1 };
        assert_eq!(registry.fresh("Informatik"), "Informatik (3)");
        assert_eq!(registry.fresh("Physik"), "Physik");
        assert_eq!(registry.fresh(""), "");
    }

    #[test]
    fn a_profile_travels_as_a_file() {
        // What the stores write: two marks, a plan of one module in one semester with one passed,
        // „Mein Studiengang".
        let semester = |key| folia_calendar::semester::SemesterKey::parse(key).unwrap();
        let mut plan = PlanDoc::default();
        assert!(plan.plan(semester("2026W"), "12204", 1_758_391_200, None) && plan.pass(semester("2025W"), "11101"));
        let mine = MineDoc::restored("program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\nstart\t2025W\n");
        let backup = Backup { name: "Zweitstudium".to_string(), texts: ["12204\t1758391200\n11101\t1758391100\n".to_string(), plan.stored(), String::new(), mine.stored(), "c\t1\tFür Lena\nm\t12204\t1\nn\t12204\tLena fragen\n".to_string()] };
        let file = backup.file();
        assert!(file.contains("\"betula\": \"profile\"") && file.contains("\"version\": 1"), "{file}");
        // An empty store is not written.
        assert!(!file.contains("saved.v1"), "{file}");
        assert_eq!(Backup::read(&file), Ok(backup.clone()));
        assert_eq!(backup.summary(), Summary { marked: 2, planned: 1, semesters: 1, passed: 1, saved: 0, program: Some("Informatik B.Sc. · PO 2008".to_string()), collections: 1, notes: 1 });
        // A byte order mark in front, as an editor may write it.
        assert_eq!(Backup::read(&format!("\u{feff}{file}")), Ok(backup));
    }

    #[test]
    fn a_file_is_read_like_anything_from_outside() {
        for unreadable in ["", "{}", "[]", "not json", "{\"betula\":\"bookmarks\",\"version\":1}", "{\"betula\":\"profile\",\"version\":0}", "{\"betula\":\"profile\"}"] {
            assert_eq!(Backup::read(unreadable), Err(NoBackup::Unreadable), "{unreadable}");
        }
        assert_eq!(Backup::read("{\"betula\":\"profile\",\"version\":2}"), Err(NoBackup::Newer));
        assert_eq!(Backup::read(&" ".repeat(MAX_BACKUP + 1)), Err(NoBackup::TooLarge));
        // What each store holds is read as the store reads it: ids that cannot be ids, a program
        // that cannot be one, stores it does not know, a name of several lines.
        let odd = r#"{"betula":"profile","version":1,"name":"A\nB","extra":true,"stores":{"betula.bookmarks.v1":"<script>\t1\n12204\t5\n","betula.myprogram.v1":"program\t../../etc\n","betula.theme":"dark","betula.p2.bookmarks.v1":"11101\t1\n","betula.collections.v1":"c\t1\tFür Lena\nm\t<script>\t1\nm\t12204\t1,7\n"}}"#;
        let read = Backup::read(odd).unwrap();
        assert_eq!(read.name, "A B");
        assert_eq!(read.texts, ["12204\t5\n".to_string(), String::new(), String::new(), String::new(), "c\t1\tFür Lena\nm\t12204\t1\n".to_string()]);
        assert!(Backup::read(r#"{"betula":"profile","version":1}"#).unwrap().summary().is_empty());
    }
}
