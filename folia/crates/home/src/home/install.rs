//! Betula as an app (owner, 2026-11-17: „Mach mal bitte a und b. Also schon bei der einrichtung
//! fragen und dann, wenn es noch nicht passiert ist dann anzeigen. Bei der einrichtung soll es nur
//! später als option geben und wenn man es dann bei b nicht mehr anzeigen wählt, dann wird es auch
//! nicht mehr angezeigt"; folia/design/installieren/konzepte.html, concepts A and B).
//!
//! - **A, the setup's last step „App"** (`AppStep`): after „Studium anlegen" and Betula Sync, where
//!   this browser can install Betula and it is not installed: „Betula als App?", three points,
//!   „Installieren" and „Später" alone.
//! - **B, the card on the day** (`InstallCard`): where it has not happened yet. Never asked (a study
//!   set up before the step was there, or while the browser could not install yet): at once on the
//!   day; after „Später" once three more days with Betula have passed and a week at least; without
//!   a study on the third day with Betula. × is „Später" again, „Nicht mehr anzeigen" ends it.
//!
//! What „Installieren" does depends on the browser (`Way`): Chromium lets a page open its own dialog
//! (`beforeinstallprompt`, held by `enhance.js`, `window.betulaInstall`); Safari on an iPhone or a
//! Mac, Firefox on Android and on Windows install from their own menus, so there it opens a guide
//! (`GuideDialog`). Where a browser can do neither (Firefox on a Mac or under Linux, a Chromium that
//! has not offered it), nothing is offered. Nothing in an app that runs installed.
//!
//! An app on an iPhone's home screen or in a Mac's Dock keeps storage of its own, apart from
//! Safari's; only cookies come along. So the button of a guide puts the profile on the clipboard
//! (as „Als Datei sichern" writes it, `folia_stores::profiles::Backup`), and the app's setup offers
//! „Studium einfügen" on its first start (`TakeOver`), which takes it into its empty first profile.
//!
//! What is counted (`Kept`) is this browser's, the same in every profile, and never leaves it
//! (`betula.install.v1`): the days Betula's start page was open, „Später", „Nicht mehr anzeigen",
//! installed.

use leptos::prelude::*;

use folia_design::haptic::{self, Haptic};
use folia_design::modal::{Modal, ModalHead, ModalSize, Sheet};
use folia_design::nav;
use folia_design::ui::{Icon, Mark};
use folia_stores::profiles::{Backup, NoBackup, Profiles};

use crate::i18n;

/// Where this browser keeps what is counted (`Kept`).
pub const STORAGE_KEY: &str = "betula.install.v1";

/// How many days with Betula are kept: more than any rule counts.
const KEEP_DAYS: usize = 16;

/// After „Später": the card comes once this many more days with Betula have passed …
const AGAIN_DAYS: usize = 3;
/// … and this many days at least.
const AGAIN_WAIT: i32 = 7;

/// Without a study: the card comes on this day with Betula.
const GUEST_DAYS: usize = 3;

/// What „Installieren" does in this browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    /// The browser's own dialog, which it lets the page open (Chromium).
    Prompt,
    /// Safari on an iPhone or an iPad (and the other browsers there): „Teilen" › „Zum
    /// Home-Bildschirm".
    Ios,
    /// Safari on a Mac (17 on): „Ablage" › „Zum Dock hinzufügen …".
    MacSafari,
    /// Firefox on Android: ⋮ › „Zum Startbildschirm hinzufügen".
    FirefoxAndroid,
    /// Firefox on Windows (143 on): „Zur Taskleiste hinzufügen" in the address bar.
    FirefoxWindows,
}

impl Way {
    /// Whether the installed app keeps storage of its own, so that the profile has to go along.
    pub fn carries(self) -> bool {
        matches!(self, Way::Ios | Way::MacSafari)
    }

    /// The button that installs, by what it does.
    fn label(self, t: &'static i18n::install::Texts) -> &'static str {
        match self {
            Way::Prompt => t.install,
            Way::Ios | Way::FirefoxAndroid => t.to_home,
            Way::MacSafari => t.to_dock,
            Way::FirefoxWindows => t.to_taskbar,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Way::Prompt | Way::FirefoxAndroid => "download",
            Way::Ios => "share",
            Way::MacSafari | Way::FirefoxWindows => "monitor-down",
        }
    }
}

/// The major version after `name/` in `ua` („Version/18.1" → 18).
fn version_of(ua: &str, name: &str) -> Option<u32> {
    let rest = ua.split(name).nth(1)?;
    rest.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

/// What „Installieren" does in a browser of `ua`: `ios` where it has `navigator.standalone` (an
/// iPhone's or an iPad's), `touch` where the screen takes touch (an iPad asks for the Mac's pages),
/// `held` where it offered its own dialog and the page holds it. `None` where it cannot install, or
/// where a Chromium has not offered it (installed already, or not yet).
pub fn way_of(ua: &str, ios: bool, touch: bool, held: bool) -> Option<Way> {
    let has = |needle: &str| ua.contains(needle);
    if held {
        return Some(Way::Prompt);
    }
    if ios || has("iPhone") || has("iPad") || has("iPod") || (has("Macintosh") && touch) {
        return Some(Way::Ios);
    }
    let firefox = has("Firefox/");
    if firefox && has("Android") {
        return Some(Way::FirefoxAndroid);
    }
    if firefox && has("Windows") && version_of(ua, "Firefox/").is_some_and(|v| v >= 143) {
        return Some(Way::FirefoxWindows);
    }
    let other = ["Chrome/", "Chromium/", "CriOS/", "Edg/", "OPR/", "Firefox/", "FxiOS/"].iter().any(|name| has(name));
    if has("Macintosh") && has("Safari/") && !other && version_of(ua, "Version/").is_some_and(|v| v >= 17) {
        return Some(Way::MacSafari);
    }
    None
}

/// What this browser counted, kept as lines (`betula.install.v1`):
///
/// ```text
/// days\t20404,20405,20409
/// later\t20405
/// never
/// installed
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kept {
    /// The days with Betula (`folia_calendar::day::Day`), the last `KEEP_DAYS`, in their order.
    pub days: Vec<i32>,
    /// The day of the last „Später".
    pub later: Option<i32>,
    /// „Nicht mehr anzeigen".
    pub never: bool,
    /// Installed here: the browser said so, or Betula ran installed.
    pub installed: bool,
}

impl Kept {
    /// What `text` keeps; what is not understood is passed over.
    pub fn restored(text: &str) -> Self {
        let mut kept = Kept::default();
        for line in text.lines() {
            let mut parts = line.split('\t');
            match (parts.next(), parts.next()) {
                (Some("days"), Some(days)) => {
                    let mut days: Vec<i32> = days.split(',').filter_map(|day| day.trim().parse().ok()).collect();
                    days.sort_unstable();
                    days.dedup();
                    let from = days.len().saturating_sub(KEEP_DAYS);
                    kept.days = days.split_off(from);
                }
                (Some("later"), Some(day)) => kept.later = day.trim().parse().ok(),
                (Some("never"), _) => kept.never = true,
                (Some("installed"), _) => kept.installed = true,
                _ => {}
            }
        }
        kept
    }

    /// Its lines; empty where nothing is counted.
    pub fn stored(&self) -> String {
        let mut lines = Vec::new();
        if !self.days.is_empty() {
            lines.push(format!("days\t{}", self.days.iter().map(i32::to_string).collect::<Vec<_>>().join(",")));
        }
        if let Some(day) = self.later {
            lines.push(format!("later\t{day}"));
        }
        if self.never {
            lines.push("never".to_string());
        }
        if self.installed {
            lines.push("installed".to_string());
        }
        lines.into_iter().map(|line| line + "\n").collect()
    }

    /// Counts `day` as a day with Betula.
    pub fn see(&mut self, day: i32) {
        if !self.days.contains(&day) {
            self.days.push(day);
            self.days.sort_unstable();
            let from = self.days.len().saturating_sub(KEEP_DAYS);
            self.days.drain(..from);
        }
    }

    /// Whether the card stands on the day `today`, for a visitor with a study (`study`) or without.
    pub fn due(&self, today: i32, study: bool) -> bool {
        if self.never || self.installed {
            return false;
        }
        match self.later {
            Some(later) => today >= later + AGAIN_WAIT && self.days.iter().filter(|day| **day > later).count() >= AGAIN_DAYS,
            None => study || self.days.len() >= GUEST_DAYS,
        }
    }
}

/// How an answer of the browser's dialog came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    Accepted,
    /// Dismissed, or the dialog could not be opened any more.
    NotNow,
}

/// What the start page shares of installing: what is counted, what the browser said, the guide
/// open and what follows it. Provided by the start page (`HomePage`).
#[derive(Clone, Copy)]
pub struct Install {
    kept: RwSignal<Kept>,
    /// Counts what `enhance.js` said (`betula:install`): the dialog held, or installed.
    heard: RwSignal<u32>,
    /// The guide open, of this way.
    guide: RwSignal<Option<Way>>,
    /// Whether the profile went onto the clipboard for the guide.
    copied: RwSignal<Option<bool>>,
    /// What follows the guide's „Verstanden" (the setup ends).
    then: StoredValue<Option<Callback<()>>>,
}

impl Install {
    /// Reads what this browser counted, counts today, and listens to the browser.
    pub fn provide() -> Self {
        let install = Install { kept: RwSignal::new(Kept::restored(&nav::local_get(STORAGE_KEY).unwrap_or_default())), heard: RwSignal::new(0), guide: RwSignal::new(None), copied: RwSignal::new(None), then: StoredValue::new(None) };
        provide_context(install);
        if cfg!(feature = "csr") {
            let today = super::today::clock_now().map(|(day, _)| day.0);
            let running = running_as_app();
            // A browser that offers its dialog now has Betula not installed, whatever was kept:
            // the offer often comes as the page loads, before the start page listens
            // (`enhance.js` holds it), and an app taken off since must be offered again (owner,
            // 2026-11-18: „ich hatte es … chrome explizit deinstalliert zum testen").
            let offered = !running && bridge_says("can");
            install.change(|kept| {
                if let Some(today) = today {
                    kept.see(today);
                }
                if running {
                    kept.installed = true;
                }
                if offered {
                    kept.installed = false;
                }
            });
            Effect::new(move |_| {
                let handle = window_event_listener_untyped("betula:install", move |_| {
                    let (held, installed) = (bridge_says("can"), bridge_says("installed"));
                    // A browser offers its dialog only while Betula is not installed: one that
                    // offers it again had it taken off.
                    if !running_as_app() && (held || installed) {
                        install.change(|kept| kept.installed = installed);
                    }
                    install.heard.update(|n| *n = n.wrapping_add(1));
                });
                on_cleanup(move || handle.remove());
            });
        }
        install
    }

    pub fn expect() -> Option<Self> {
        use_context::<Install>()
    }

    fn change(self, change: impl FnOnce(&mut Kept)) {
        let mut kept = self.kept.get_untracked();
        change(&mut kept);
        if self.kept.with_untracked(|now| *now != kept) {
            nav::local_set(STORAGE_KEY, &kept.stored());
            self.kept.set(kept);
        }
    }

    /// What „Installieren" does here now; `None` where nothing is offered: installed, running as
    /// the app, or a browser that cannot. Tracked.
    pub fn way(self) -> Option<Way> {
        self.heard.track();
        if running_as_app() {
            return None;
        }
        // The browser's offer first: it is made only where Betula is not installed.
        if bridge_says("can") {
            return Some(Way::Prompt);
        }
        if self.kept.with(|kept| kept.installed) {
            return None;
        }
        browser_way()
    }

    /// Whether the card stands on the day. Tracked.
    pub fn due(self, study: bool) -> bool {
        let Some(today) = super::today::clock_now().map(|(day, _)| day.0) else { return false };
        self.way().is_some() && self.kept.with(|kept| kept.due(today, study))
    }

    /// „Später": asked again in a few days.
    pub fn later(self) {
        let today = super::today::clock_now().map(|(day, _)| day.0);
        self.change(|kept| kept.later = today.or(kept.later));
    }

    /// „Nicht mehr anzeigen".
    pub fn never(self) {
        self.change(|kept| kept.never = true);
    }

    /// „Installieren": the browser's dialog where it lets the page open it (`done` hears how it
    /// came out); else the guide of this browser, the profile on the clipboard where the app keeps
    /// storage of its own, and `done` follows its „Verstanden" (as `NotNow`: whether it was done,
    /// the browser does not say). A click's.
    pub fn start(self, way: Way, done: Callback<Asked>) {
        if way == Way::Prompt {
            let outcome = bridge_call("prompt", None);
            // Without `enhance.js` nothing can be asked.
            if outcome.is_none() {
                self.later();
                done.run(Asked::NotNow);
                return;
            }
            then_text(outcome, move |answer| {
                let asked = if answer == "accepted" { Asked::Accepted } else { Asked::NotNow };
                match asked {
                    Asked::Accepted => {
                        haptic::play(Haptic::Success);
                        self.change(|kept| kept.installed = true);
                    }
                    Asked::NotNow => self.later(),
                }
                done.run(asked);
            });
            return;
        }
        if way.carries() {
            self.copy_profile();
        }
        self.then.set_value(Some(Callback::new(move |()| done.run(Asked::NotNow))));
        self.guide.set(Some(way));
    }

    /// The active profile onto the clipboard, as „Als Datei sichern" writes it.
    fn copy_profile(self) {
        let t = i18n::t();
        let Some(backup) = Profiles::expect().and_then(|profiles| profiles.backup(profiles.active_untracked(), t.locale)) else {
            self.copied.set(Some(false));
            return;
        };
        self.copied.set(None);
        let copied = self.copied;
        then_text(bridge_call("copy", Some(&backup.file())), move |answer| {
            let _ = copied.try_set(Some(answer == "true"));
        });
    }

    /// The guide's „Verstanden" (and its ×): later, and what follows it.
    fn guide_done(self) {
        self.guide.set(None);
        self.later();
        if let Some(then) = self.then.try_update_value(Option::take).flatten() {
            then.run(());
        }
    }
}

/// Whether Betula runs as the installed app: the display modes of an app's window, or an iPhone's
/// home screen (`navigator.standalone`). Not `fullscreen`, which a browser's window in full screen
/// (F11) matches as well. `false` on the server.
pub fn running_as_app() -> bool {
    #[cfg(feature = "csr")]
    {
        let Some(window) = web_sys::window() else { return false };
        let mode = |query: &str| window.match_media(query).ok().flatten().is_some_and(|list| list.matches());
        let standalone = web_sys::js_sys::Reflect::get(&window.navigator(), &"standalone".into()).ok().and_then(|value| value.as_bool()).unwrap_or(false);
        standalone || ["(display-mode: standalone)", "(display-mode: minimal-ui)", "(display-mode: window-controls-overlay)"].iter().any(|query| mode(query))
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// Whether Betula runs as an app that keeps storage of its own, apart from the browser it came
/// from (an iPhone's home screen, a Mac's Dock): there a profile goes along by the clipboard.
pub fn apart_from_browser() -> bool {
    running_as_app() && browser_way().is_some_and(Way::carries)
}

/// What this browser's „Installieren" would do, whatever is installed (`way_of`).
fn browser_way() -> Option<Way> {
    #[cfg(feature = "csr")]
    {
        let window = web_sys::window()?;
        let navigator = window.navigator();
        let ua = navigator.user_agent().unwrap_or_default();
        let ios = web_sys::js_sys::Reflect::has(&navigator, &"standalone".into()).unwrap_or(false);
        let touch = navigator.max_touch_points() > 1;
        way_of(&ua, ios, touch, bridge_says("can"))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// `window.betulaInstall[name]()` as a yes or no (`enhance.js`); no without the script.
fn bridge_says(name: &str) -> bool {
    #[cfg(feature = "csr")]
    {
        bridge_call(name, None).and_then(|value| value.as_bool()).unwrap_or(false)
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = name;
        false
    }
}

#[cfg(feature = "csr")]
type JsAnswer = Option<wasm_bindgen::JsValue>;
#[cfg(not(feature = "csr"))]
type JsAnswer = Option<()>;

/// Calls `window.betulaInstall[name](arg)`; what it answered, none without the script.
#[allow(unused_variables)]
fn bridge_call(name: &str, arg: Option<&str>) -> JsAnswer {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        use web_sys::js_sys::{Function, Reflect};
        let window = web_sys::window()?;
        let bridge = Reflect::get(&window, &"betulaInstall".into()).ok().filter(|bridge| bridge.is_object())?;
        let function = Reflect::get(&bridge, &name.into()).ok()?.dyn_into::<Function>().ok()?;
        match arg {
            Some(arg) => function.call1(&bridge, &arg.into()).ok(),
            None => function.call0(&bridge).ok(),
        }
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// What a promise of the bridge resolves to, as text (`true` for a yes), to `done`; nothing where
/// there is no promise. `done` runs with the owner of the call, so that what it says finds the
/// page's context (the note after the setup, `Notices`), which an answer coming later has not.
#[allow(unused_variables)]
fn then_text(promise: JsAnswer, done: impl FnOnce(String) + 'static) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::{JsCast, JsValue};
        use web_sys::js_sys::{Function, Reflect};
        let Some(promise) = promise else { return };
        let Some(then) = Reflect::get(&promise, &"then".into()).ok().and_then(|then| then.dyn_into::<Function>().ok()) else { return };
        let owner = Owner::current();
        let callback = Closure::once_into_js(move |value: JsValue| {
            let text = value.as_string().or_else(|| value.as_bool().map(|yes| yes.to_string())).unwrap_or_default();
            match owner {
                Some(owner) => owner.with(|| done(text)),
                None => done(text),
            }
        });
        let _ = then.call1(&promise, &callback);
    }
}

/// The setup's last step (A): „Betula als App?", what one has of it, „Installieren" (or the guide's
/// button) and „Später" alone. Either ends the setup (`on_finish`): the browser's dialog once it is
/// answered, a guide once it is understood.
#[component]
pub fn AppStep(on_finish: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.install;
    let install = Install::expect();
    let phone = nav::is_phone();
    // The way as the step came: a Chromium's dialog is gone from `way` once it is opened.
    let way = install.and_then(|install| untrack(|| install.way()));
    let asking = RwSignal::new(false);
    let points = if phone { s.points_phone } else { s.points_desk };
    let start = move |_| {
        let (Some(install), Some(way), false) = (install, way, asking.get_untracked()) else { return };
        asking.set(way == Way::Prompt);
        install.start(way, Callback::new(move |_| on_finish.run(())));
    };
    let later = move |_| {
        if let Some(install) = install {
            install.later();
        }
        on_finish.run(());
    };
    view! {
        <h1 class="ob-title">{s.title}</h1>
        <p class="ob-lede">{if phone { s.lead_phone } else { s.lead_desk }}</p>
        <div class="ob-fill" data-scroll-shade="">
            <ul class="ob-points">
                {["smartphone", "maximize", "wifi-off"].into_iter().zip(points).map(|(icon, point)| view! {
                    <li><span class="ob-point-icon" aria-hidden="true"><Icon name=if icon == "smartphone" && !phone { "monitor" } else { icon }/></span><span>{point}</span></li>
                }).collect_view()}
            </ul>
        </div>
        <div class="ob-actions">
            <button type="button" class="btn ghost ob-later" title=s.later_title on:click=later data-command="">{s.later}</button>
            {way.map(|way| view! {
                <button type="button" class="btn primary ob-install" aria-disabled=move || asking.get().then_some("true") on:click=start data-command="">
                    <Icon name=way.icon()/>{move || if asking.get() { s.asking } else { way.label(s) }}
                </button>
            })}
        </div>
    }
}

/// The card on the day (B): where this browser can install Betula, it is not installed and it is
/// due (`Kept::due`). „Installieren" (or the guide's button), × for later, „Nicht mehr anzeigen".
#[component]
pub fn InstallCard(study: bool) -> impl IntoView {
    let t = i18n::t();
    let s = &t.install;
    let Some(install) = Install::expect() else { return ().into_any() };
    let phone = nav::is_phone();
    let asking = RwSignal::new(false);
    let shown = Memo::new(move |_| install.due(study).then(|| install.way()).flatten());
    view! {
        {move || shown.get().map(|way| {
            let start = move |_| {
                if asking.get_untracked() {
                    return;
                }
                asking.set(way == Way::Prompt);
                install.start(way, Callback::new(move |_| asking.set(false)));
            };
            view! {
                <section class="panel day-panel day-install" aria-labelledby="day-install-title" data-commands=s.card_title>
                    <span class="day-install-icon" aria-hidden="true"><Mark/></span>
                    <div class="day-install-text">
                        <h2 id="day-install-title">{s.card_title}</h2>
                        <p>{if phone { s.card_phone } else { s.card_desk }}</p>
                        <p class="day-install-acts">
                            <button type="button" class="btn primary sm" aria-disabled=move || asking.get().then_some("true") on:click=start data-command="">
                                <Icon name=way.icon()/>{move || if asking.get() { s.asking } else { way.label(s) }}
                            </button>
                            <button type="button" class="btn ghost sm" title=s.never_title on:click=move |_| install.never() data-command="">{s.never}</button>
                        </p>
                    </div>
                    <button type="button" class="day-install-x" aria-label=s.card_later title=s.card_later on:click=move |_| install.later() data-command=s.card_later>
                        <Icon name="x"/>
                    </button>
                </section>
            }
        })}
    }
    .into_any()
}

/// The guide of a browser that installs by its own menu: its steps, and on an iPhone or a Mac the
/// profile that goes along. Once on the start page, for the step and the card.
#[component]
pub fn GuideDialog() -> impl IntoView {
    let t = i18n::t();
    let s = &t.install;
    let Some(install) = Install::expect() else { return ().into_any() };
    // What it shows stays while it glides away (`folia_design::modal`: read from what it was last).
    let shown = Memo::new(move |before: Option<&Way>| install.guide.get().or(before.copied()).unwrap_or(Way::Ios));
    let close = Callback::new(move |()| install.guide_done());
    let title = move || match shown.get() {
        Way::MacSafari => s.guide_dock,
        Way::FirefoxWindows => s.guide_taskbar,
        _ => s.guide_home,
    };
    let steps = move || -> Vec<&'static str> {
        match shown.get() {
            Way::Ios | Way::Prompt => s.steps_ios.to_vec(),
            Way::MacSafari => s.steps_mac.to_vec(),
            Way::FirefoxAndroid => s.steps_firefox_android.to_vec(),
            Way::FirefoxWindows => s.steps_firefox_windows.to_vec(),
        }
    };
    let carry = move || {
        shown.get().carries().then(|| {
            let said = move || match install.copied.get() {
                Some(true) => view! { <span class="inst-got"><Icon name="check"/>{s.copied}</span> }.into_any(),
                Some(false) => view! {
                    <span class="inst-failed">{s.not_copied}</span>
                    <button type="button" class="btn sm" on:click=move |_| install.copy_profile() data-command="">{s.copy_again}</button>
                }
                .into_any(),
                None => ().into_any(),
            };
            view! {
                <div class="inst-carry">
                    <span class="inst-carry-icon" aria-hidden="true"><Icon name="clipboard-list"/></span>
                    <div><p><b>{s.carry_title}</b>" "{s.carry_text}</p><p class="inst-said">{said}</p></div>
                </div>
            }
        })
    };
    view! {
        <Modal open=Signal::derive(move || install.guide.with(Option::is_some)) on_close=close size=ModalSize::S sheet=Signal::derive(|| Sheet::Menu) class="inst-dlg" labelled_by="inst-title">
            {move || view! { <ModalHead id="inst-title" title=title() on_close=close/> }}
            <div class="st-dlg-body inst-body">
                <p class="inst-lead">{s.guide_lead}</p>
                <ol class="inst-steps">{move || steps().into_iter().map(|step| view! { <li>{step}</li> }).collect_view()}</ol>
                {carry}
            </div>
            <footer class="st-dlg-foot">
                <button type="button" class="btn primary" on:click=move |_| close.run(()) data-command="">{s.understood}</button>
            </footer>
        </Modal>
    }
    .into_any()
}

/// The first start of an app that keeps storage of its own (an iPhone's home screen, a Mac's Dock),
/// its profile empty: „Studium einfügen" takes the profile the browser put on the clipboard into
/// it. Above the setup's first step.
#[component]
pub fn TakeOver() -> impl IntoView {
    let t = i18n::t();
    let s = &t.install;
    let Some(profiles) = Profiles::expect() else { return ().into_any() };
    if !apart_from_browser() || untrack(|| profiles.empty_first()).is_none() {
        return ().into_any();
    }
    let taking = RwSignal::new(false);
    let problem = RwSignal::new(None::<&'static str>);
    let take = move |_| {
        if taking.get_untracked() {
            return;
        }
        taking.set(true);
        problem.set(None);
        then_text(bridge_call("paste", None), move |text| {
            taking.try_set(false);
            match Backup::read(&text) {
                Ok(backup) if !backup.summary().is_empty() => {
                    haptic::play(Haptic::Success);
                    let _ = profiles.take_over(&backup);
                }
                Err(NoBackup::Newer) => {
                    haptic::play(Haptic::Error);
                    problem.try_set(Some(s.take_newer));
                }
                _ => {
                    haptic::play(Haptic::Error);
                    problem.try_set(Some(s.take_none));
                }
            }
        });
    };
    view! {
        <div class="ob-take">
            <p><b>{s.take_title}</b>" "{s.take_text}</p>
            <button type="button" class="btn primary ob-take-btn" aria-disabled=move || taking.get().then_some("true") on:click=take data-command="">
                <Icon name="clipboard-list"/>{move || if taking.get() { s.taking } else { s.take }}
            </button>
            {move || problem.get().map(|said| view! { <p class="ob-note" role="status"><Icon name="triangle-alert"/><span>{said}</span></p> })}
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIXEL: &str = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
    const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 26_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.1 Mobile/15E148 Safari/604.1";
    const IPHONE_CHROME: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 26_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/140.0.0.0 Mobile/15E148 Safari/604.1";
    const MAC_SAFARI: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.1 Safari/605.1.15";
    const MAC_SAFARI_16: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Safari/605.1.15";
    const MAC_CHROME: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
    const FIREFOX_ANDROID: &str = "Mozilla/5.0 (Android 15; Mobile; rv:145.0) Gecko/145.0 Firefox/145.0";
    const FIREFOX_WINDOWS: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:145.0) Gecko/20100101 Firefox/145.0";
    const FIREFOX_WINDOWS_OLD: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0";
    const FIREFOX_MAC: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.5; rv:145.0) Gecko/20100101 Firefox/145.0";
    const FIREFOX_LINUX: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:145.0) Gecko/20100101 Firefox/145.0";

    /// What „Installieren" does in each browser: Chromium's dialog where it offered it (and nothing
    /// where it did not: installed, or not yet), the guides of the others, nothing where a browser
    /// cannot install.
    #[test]
    fn each_browser_installs_its_way() {
        assert_eq!(way_of(PIXEL, false, true, true), Some(Way::Prompt));
        assert_eq!(way_of(PIXEL, false, true, false), None);
        assert_eq!(way_of(MAC_CHROME, false, false, false), None);
        assert_eq!(way_of(IPHONE, true, true, false), Some(Way::Ios));
        assert_eq!(way_of(IPHONE_CHROME, true, true, false), Some(Way::Ios));
        // An iPad that asks for the Mac's pages: its touch tells it apart.
        assert_eq!(way_of(MAC_SAFARI, false, true, false), Some(Way::Ios));
        assert_eq!(way_of(MAC_SAFARI, false, false, false), Some(Way::MacSafari));
        assert_eq!(way_of(MAC_SAFARI_16, false, false, false), None);
        assert_eq!(way_of(FIREFOX_ANDROID, false, true, false), Some(Way::FirefoxAndroid));
        assert_eq!(way_of(FIREFOX_WINDOWS, false, false, false), Some(Way::FirefoxWindows));
        assert_eq!(way_of(FIREFOX_WINDOWS_OLD, false, false, false), None);
        assert_eq!(way_of(FIREFOX_MAC, false, false, false), None);
        assert_eq!(way_of(FIREFOX_LINUX, false, false, false), None);
        // Only the apps of Safari's keep storage of their own.
        assert!(Way::Ios.carries() && Way::MacSafari.carries());
        assert!(!Way::Prompt.carries() && !Way::FirefoxAndroid.carries() && !Way::FirefoxWindows.carries());
    }

    #[test]
    fn what_is_counted_is_kept_as_lines() {
        let kept = Kept { days: vec![20400, 20402], later: Some(20402), never: true, installed: true };
        assert_eq!(kept.stored(), "days\t20400,20402\nlater\t20402\nnever\ninstalled\n");
        assert_eq!(Kept::restored(&kept.stored()), kept);
        assert_eq!(Kept::default().stored(), "");
        assert_eq!(Kept::restored("days\t3,x,1,3\nwhat\t1\nlater\tsoon\n"), Kept { days: vec![1, 3], ..Kept::default() });
        // A day is counted once, and only the last ones are kept.
        let mut kept = Kept::default();
        for day in (1..=40).chain([40, 12]) {
            kept.see(day);
        }
        assert_eq!(kept.days, (25..=40).collect::<Vec<_>>());
    }

    /// When the card stands (owner, 2026-11-17: „wenn es noch nicht passiert ist dann anzeigen";
    /// „wenn man es dann bei b nicht mehr anzeigen wählt, dann wird es auch nicht mehr angezeigt").
    #[test]
    fn the_card_comes_when_it_has_not_happened_yet() {
        // Never asked: at once with a study, on the third day with Betula without one.
        let mut kept = Kept::default();
        kept.see(100);
        assert!(kept.due(100, true));
        assert!(!kept.due(100, false));
        kept.see(101);
        kept.see(105);
        assert!(kept.due(105, false));
        // „Später" (in the setup or the card's ×): three more days with Betula, a week at least.
        let mut kept = Kept { days: vec![100], later: Some(100), ..Kept::default() };
        for day in [101, 102, 103] {
            kept.see(day);
            assert!(!kept.due(day, true), "{day}");
        }
        assert!(!kept.due(106, true));
        assert!(kept.due(107, true));
        let mut sparse = Kept { days: vec![100], later: Some(100), ..Kept::default() };
        sparse.see(120);
        sparse.see(130);
        assert!(!sparse.due(130, true));
        sparse.see(131);
        assert!(sparse.due(131, true));
        // „Nicht mehr anzeigen", and installed: never again.
        assert!(!Kept { never: true, ..kept.clone() }.due(107, true));
        assert!(!Kept { installed: true, ..kept }.due(107, true));
    }
}
