//! The trap: an address that asks for what only a program asks for is turned away for a while.
//!
//! Owner, 2026-11-18: „mach mal bitte einen honey pot mit Ban von der ip" — „Also nur temporär".
//! Three kinds of bait, none of which a person or the app ever asks for:
//!
//! - **The link nobody sees.** Every page the server writes ends in a link to
//!   `folia_routes::url::TRAP_LINK` (`folia_app::shell`): `hidden`, out of the tab order and of the
//!   accessibility tree, `rel="nofollow"`. A program that takes every address of a page follows it.
//! - **The address nobody asks for.** The data worker's script names
//!   `folia_routes::url::TRAP_IN_SCRIPT` beside `/api/db` and `/api/status` and never fetches it. A
//!   program that collects addresses from scripts does.
//! - **What this site does not have** (`PROBES`): `.env`, `.git`, WordPress, PHP, admin consoles,
//!   path traversal, the same list the dashboard „Security" counts as scanner probes
//!   (`deploy/config/monitoring/loki-rules/fake/security.yml`).
//!
//! Under the addresses where a code of random letters travels (`/calendar/`, `/cards/`) nothing is
//! a probe: a subscription must never look like one. And no page of the sitemap is a trap (a
//! test of the server walks it).
//!
//! The first two lie under `/api/trap/`, and `robots.txt` has kept every crawler out of `/api/`
//! since before there were traps: a search engine that keeps to it never comes here.
//!
//! Whoever springs one is paused: every request from that address is answered `429` with
//! `Retry-After` for an hour (`--trap-minutes`, `FOLIA_TRAP_MINUTES`; 0 turns the traps off), then
//! everything is as it was. What stays open meanwhile is what the page that says so needs, and
//! `robots.txt`, `/healthz` and `/livez`.
//!
//! - **An address is not a person** (the owner's rule: no „Nein" without a reason that holds, and
//!   somebody who shares an address with a program has given none): a mobile network or a campus
//!   puts many behind one. So the page a paused address gets says what happened and how long it
//!   lasts, and has one button, „Weiter zu Betula" (a POST to `TRAP_RELEASE`), which ends the
//!   pause for the address at once. A program that follows links presses no button; if it springs
//!   a trap again, the address is paused again.
//! - **Never paused:** an address that is no public one — the server asked directly (its own
//!   probe, the warm-up, working on the site) or from a private network, which is where every
//!   IPv6 visitor comes from while `docker_gwbridge` has no IPv6 (deploy/README.md section 15) —
//!   and a request another site made a browser send (`Sec-Fetch-Site: cross-site`): a page
//!   elsewhere must not be able to pause its visitors here with a picture's address.
//! - **What is kept:** in memory only, for as long as the pause lasts: a keyed fingerprint of the
//!   address (IPv4 as it is, IPv6 by its /64, as Traefik's rate limit counts) and when the pause
//!   ends. Not the address, nothing on disk, nothing in the log but that a trap was sprung and of
//!   which kind. A restart forgets every pause. The privacy notice says so („Zugriffsprotokoll").

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{FromRequest, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Form;
use hmac::{Hmac, Mac};
use leptos::prelude::*;
use serde::Deserialize;
use sha2::Sha256;

use folia_routes::url::{TRAP_PREFIX, TRAP_RELEASE};

use crate::AppState;

/// How long an address is paused unless configured, in minutes.
pub const DEFAULT_MINUTES: u64 = 60;
/// More addresses than this are not kept at once (some 5 MB): whoever comes after is not paused.
const MAX_PAUSED: usize = 100_000;

/// What this site does not have, as a part of the path in lower case: the patterns of the
/// dashboard's scanner probes (`security.yml`; a path there is matched as it was sent, here in
/// lower case as well).
const PROBES: &[&str] = &[
    "/.env", "/.git", "/.aws", "/.ssh", "/.vscode", ".ds_store", "/backup", ".sql", ".bak", "/wp-", "wordpress", "/xmlrpc", ".php", "phpmyadmin", "/vendor/", "../", "%2e%2e", "/etc/passwd",
    "/cgi-bin", "/actuator", "/server-status", "/solr", "/jenkins", "/druid", "/swagger", "/hnap1", "/boaform",
];

/// Which bait a path is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bait {
    /// Under `/api/trap/`: the link nobody sees, the address nobody asks for.
    Laid,
    /// What a scanner looks for.
    Probe,
}

/// Is `path` a trap?
pub fn bait(path: &str) -> Option<Bait> {
    if path.starts_with(TRAP_PREFIX) {
        return Some(Bait::Laid);
    }
    // Where a code of random letters travels (a calendar subscription, the card of a shared
    // plan), nothing is a probe: one code in some ten thousands begins with „wp-", and a
    // subscription must never break, nor pause the calendar service that fetches it.
    let own = folia_locale::Locale::split(path).1;
    if own.starts_with(folia_calendar::subscription::CALENDAR_PREFIX) || own.starts_with("/cards/") {
        return None;
    }
    let lower = path.to_ascii_lowercase();
    PROBES.iter().any(|probe| lower.contains(probe)).then_some(Bait::Probe)
}

pub struct Traps {
    key: [u8; 32],
    pause: Duration,
    /// Fingerprints of the paused addresses and when each pause ends; then the size at which
    /// the ended ones are next swept out.
    paused: Mutex<(HashMap<u128, Instant>, usize)>,
    /// How many are kept, so that a request need not ask the table while nobody is paused.
    kept: AtomicUsize,
}

impl Traps {
    /// Traps that pause for `minutes`, with a key of their own; `None` for 0 (off).
    pub fn new(minutes: u64) -> Option<Self> {
        if minutes == 0 {
            return None;
        }
        let mut key = [0u8; 32];
        getrandom::getrandom(&mut key).ok()?;
        Some(Self::with_key(key, Duration::from_secs(minutes * 60)))
    }

    pub fn with_key(key: [u8; 32], pause: Duration) -> Self {
        Self { key, pause, paused: Mutex::new((HashMap::new(), 1024)), kept: AtomicUsize::new(0) }
    }

    pub fn minutes(&self) -> u64 {
        self.pause.as_secs() / 60
    }

    /// How many addresses are kept right now (those whose pause ended and was not swept out yet
    /// among them).
    pub fn kept(&self) -> usize {
        self.kept.load(Ordering::Relaxed)
    }

    /// What is kept of an address: nothing that leads back to it without this process's key.
    fn fingerprint(&self, address: IpAddr) -> u128 {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).expect("HMAC takes a key of any length");
        match address {
            IpAddr::V4(address) => {
                mac.update(b"4");
                mac.update(&address.octets());
            }
            // One customer has a whole /64.
            IpAddr::V6(address) => {
                mac.update(b"6");
                mac.update(&address.octets()[..8]);
            }
        }
        let bytes = mac.finalize().into_bytes();
        u128::from_be_bytes(bytes[..16].try_into().expect("sixteen bytes"))
    }

    /// How much longer `address` is paused, if it is.
    pub fn paused_for(&self, address: IpAddr, now: Instant) -> Option<Duration> {
        if self.kept() == 0 {
            return None;
        }
        let fingerprint = self.fingerprint(address);
        let mut paused = self.paused.lock().ok()?;
        match paused.0.get(&fingerprint).map(|until| until.saturating_duration_since(now)) {
            Some(left) if !left.is_zero() => Some(left),
            Some(_) => {
                paused.0.remove(&fingerprint);
                self.kept.store(paused.0.len(), Ordering::Relaxed);
                None
            }
            None => None,
        }
    }

    /// Pauses `address` from `now`; how long. An address paused already keeps its end.
    pub fn spring(&self, address: IpAddr, now: Instant) -> Option<Duration> {
        let fingerprint = self.fingerprint(address);
        let mut paused = self.paused.lock().ok()?;
        let (table, sweep_at) = &mut *paused;
        if table.len() >= *sweep_at {
            table.retain(|_, until| *until > now);
            *sweep_at = (table.len() * 2).max(1024);
        }
        if table.len() >= MAX_PAUSED && !table.contains_key(&fingerprint) {
            return None;
        }
        let until = *table.entry(fingerprint).and_modify(|until| if *until <= now { *until = now + self.pause }).or_insert(now + self.pause);
        self.kept.store(table.len(), Ordering::Relaxed);
        Some(until.saturating_duration_since(now))
    }

    /// Ends the pause of `address`; whether it was paused.
    pub fn release(&self, address: IpAddr) -> bool {
        let fingerprint = self.fingerprint(address);
        let Ok(mut paused) = self.paused.lock() else { return false };
        let was = paused.0.remove(&fingerprint).is_some();
        self.kept.store(paused.0.len(), Ordering::Relaxed);
        was
    }
}

/// An address that may be paused: a public one. Not the private networks, the loopback or the
/// link-local ones (the server's own checks, and every IPv6 visitor while they reach Traefik from
/// the bridge's gateway), nor the carriers' shared space (100.64.0.0/10), which no visitor comes from.
pub fn public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(a) => {
            let [first, second, ..] = a.octets();
            !(a.is_private() || a.is_loopback() || a.is_link_local() || a.is_unspecified() || a.is_broadcast() || (first == 100 && second & 0xc0 == 64))
        }
        IpAddr::V6(a) => {
            let first = a.segments()[0];
            !(a.is_loopback() || a.is_unspecified() || first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80)
        }
    }
}

/// A request a page of another site made the browser send (a picture, a frame, a link there):
/// browsers say so themselves, and no page can make them say otherwise.
fn from_another_site(headers: &HeaderMap) -> bool {
    headers.get("sec-fetch-site").and_then(|value| value.to_str().ok()).is_some_and(|site| site.eq_ignore_ascii_case("cross-site"))
}

#[derive(Deserialize)]
struct Released {
    next: Option<String>,
}

/// Middleware around the whole site, the gate of closed testing included: a paused address hears
/// so before anything else, and a trap is one whether the site is open or not.
pub async fn guard(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(traps) = state.traps.as_ref() else { return next.run(request).await };
    let address = crate::pass::address_of(request.headers()).filter(|address| public(*address));
    let now = Instant::now();
    let path = request.uri().path();

    // The button of the page below: the pause of this address ends, and the visitor goes on.
    if path == TRAP_RELEASE && request.method() == Method::POST {
        let released = address.is_some_and(|address| traps.release(address));
        if released {
            tracing::info!(component = "trap", event = "trap.released", paused = traps.kept(), "somebody pressed the button: the pause of an address ended");
        }
        let wanted = Form::<Released>::from_request(request, &()).await.ok().and_then(|Form(form)| form.next);
        let next = crate::access::way_back(wanted.as_deref());
        let next = if bait(next.split(['?', '#']).next().unwrap_or_default()).is_some() { "/".to_string() } else { next };
        return (StatusCode::SEE_OTHER, [(header::LOCATION, next), (header::CACHE_CONTROL, "no-store".to_string())]).into_response();
    }

    let open = crate::access::OPEN.contains(&path);
    if let Some(left) = address.filter(|_| !open).and_then(|address| traps.paused_for(address, now)) {
        return paused(&state, &request, left);
    }
    let Some(kind) = bait(path) else { return next.run(request).await };
    // A trap, sprung or not, is nothing this site has.
    let sprung = address.filter(|_| !from_another_site(request.headers())).and_then(|address| traps.spring(address, now));
    match sprung {
        Some(left) => {
            tracing::info!(component = "trap", event = "trap.sprung", kind = ?kind, minutes = traps.minutes(), paused = traps.kept(), "a trap was sprung: its address is paused");
            paused(&state, &request, left)
        }
        None => (StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, "no-store")], "Not found.\n").into_response(),
    }
}

/// What a paused address hears: a page that says why, how long, and how a person goes on, where a
/// page was asked for; a line of text else.
fn paused(state: &AppState, request: &Request, left: Duration) -> Response {
    let seconds = left.as_secs().max(1);
    let minutes = seconds.div_ceil(60);
    let wants_page = matches!(*request.method(), Method::GET | Method::HEAD)
        && request.headers().get(header::ACCEPT).and_then(|value| value.to_str().ok()).is_some_and(|value| value.contains("text/html"));
    let mut response = if wants_page {
        // Where the visitor wanted to go, unless that was the trap itself.
        let wanted = request.uri().path_and_query().map(|wanted| wanted.as_str()).filter(|_| bait(request.uri().path()).is_none());
        let html = page(state, &crate::access::way_back(wanted), minutes);
        (StatusCode::TOO_MANY_REQUESTS, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
    } else {
        let text = format!(
            "Betula does not answer requests from this address for about {minutes} more minute(s): a program asked from it for what only a trap for bots holds. A person goes on at {}/ with one button.\n",
            state.public_url
        );
        (StatusCode::TOO_MANY_REQUESTS, text).into_response()
    };
    let headers = response.headers_mut();
    headers.insert(header::RETRY_AFTER, HeaderValue::from(seconds));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("x-robots-tag", HeaderValue::from_static("noindex, nofollow"));
    response
}

/// The page of a pause: a document of its own like the login page of closed testing
/// (`access::login_page`), with the site's stylesheet and no script but the one that applies the
/// remembered theme; its button works without JavaScript. In the language of the page the
/// visitor wanted.
fn page(state: &AppState, next: &str, minutes: u64) -> String {
    let locale = folia_locale::Locale::split(next).0;
    let (t, app_texts) = (crate::texts::texts(locale), folia_app::i18n::texts(locale));
    let next = next.to_string();
    let stylesheet = folia_app::BuildId(state.build_id.clone()).asset(folia_app::STYLESHEET);
    view! {
        <!DOCTYPE html>
        <html lang=locale.code()>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"/>
                <meta name="color-scheme" content="light dark"/>
                <meta name="robots" content="noindex, nofollow"/>
                <meta name="theme-color" content=folia_app::THEME_LIGHT/>
                <title>{format!("{} · Betula", t.trap_title)}</title>
                <link rel="icon" href=folia_app::FAVICON_ICO sizes="32x32"/>
                <link rel="icon" type="image/svg+xml" href=folia_app::FAVICON/>
                <link rel="apple-touch-icon" href=folia_app::TOUCH_ICON/>
                // `as` comes first: after a value the macro would read it as a cast.
                <link as="font" rel="preload" type="font/woff2" crossorigin="anonymous" href=folia_app::FONT/>
                <link rel="stylesheet" href=stylesheet/>
                <style inner_html=folia_app::VIEW_TRANSITION_STYLE></style>
                <script inner_html=folia_app::HEAD_SCRIPT></script>
            </head>
            <body class="gate">
                <main class="gate-main">
                    <div class="gate-brand">
                        <span class="logo"><folia_design::ui::Mark/></span>
                        <span><folia_design::ui::Wordmark/><small>{app_texts.common.tagline}</small></span>
                    </div>
                    <section class="gate-panel">
                        <h1>{t.trap_title}</h1>
                        <p class="gate-why">{(t.trap_text)(minutes)}</p>
                        <p>{t.trap_shared}</p>
                        <form method="post" action=TRAP_RELEASE>
                            <input type="hidden" name="next" value=next/>
                            <button class="button" type="submit" autofocus>{t.trap_go_on}</button>
                        </form>
                    </section>
                </main>
            </body>
        </html>
    }
    .to_html()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn bait_is_what_the_site_does_not_have() {
        for path in ["/api/trap/modules", "/api/trap/catalog.db", "/api/trap/", "/api/trap/release", "/api/trap/anything/else"] {
            assert_eq!(bait(path), Some(Bait::Laid), "{path}");
        }
        for path in ["/.env", "/app/.env.local", "/.git/config", "/wp-login.php", "/WP-ADMIN/", "/index.php", "/phpMyAdmin/", "/cgi-bin/luci", "/a/../../etc/passwd", "/%2e%2e/%2e%2e/x", "/backup.zip", "/dump.sql", "/actuator/health", "/HNAP1"] {
            assert_eq!(bait(path), Some(Bait::Probe), "{path}");
        }
        // The site's own addresses, and what a person may mistype.
        for path in [
            "/", "/en", "/catalog", "/catalog/module/11101", "/programs/informatik-bsc/plan", "/api/db", "/api/status", "/api/pass", "/api/map.json", "/api/trap", "/assets/sql-wasm.js", "/assets/sql-wasm.wasm",
            "/pkg/folia_client_bg.wasm", "/models/e5-de-en-0123456789abcdef.bin", "/calendar/abc.ics", "/calendar/s/abc.ics", "/cards/module/11101.png", "/robots.txt", "/sitemap.xml", "/.well-known/security.txt",
            "/.well-known/change-password", "/favicon.ico", "/sw.js", "/sync/v1/ws", "/no-such-page", "/study", "/studyplan", "/privacy", "/about/overview",
        ] {
            assert_eq!(bait(path), None, "{path}");
        }
        // A code of random letters is never a probe, whatever it happens to spell.
        for path in ["/calendar/wp-Abc123.ics", "/calendar/s/WP-xyz_solr.ics", "/en/calendar/wp-Abc123.ics", "/cards/studyplan/wp-Abc.png", "/en/cards/studyplan/solrX.png", "/calendar/backupQ.ics"] {
            assert_eq!(bait(path), None, "{path}");
        }
        assert_eq!(bait("/calendar.php"), Some(Bait::Probe));
        for address in [folia_routes::url::TRAP_LINK, folia_routes::url::TRAP_IN_SCRIPT, TRAP_RELEASE] {
            assert!(address.starts_with(TRAP_PREFIX) && address.starts_with("/api/"), "{address}: under what robots.txt keeps crawlers out of");
        }
    }

    #[test]
    fn only_a_public_address_is_paused() {
        for address in ["203.0.113.7", "141.43.1.1", "8.8.8.8", "2001:db8::1", "2a02:8109:1234:5678::9"] {
            assert!(public(ip(address)), "{address}");
        }
        for address in ["127.0.0.1", "10.0.5.2", "172.18.0.1", "192.168.1.20", "169.254.1.1", "0.0.0.0", "100.64.0.1", "100.127.255.254", "::1", "::", "fd00::1", "fe80::1"] {
            assert!(!public(ip(address)), "{address}");
        }
        assert!(public(ip("100.63.0.1")) && public(ip("100.128.0.1")), "beside the carriers' shared space");
    }

    #[test]
    fn a_pause_ends_by_itself_or_by_the_button() {
        let traps = Traps::with_key([5; 32], Duration::from_secs(3600));
        let (start, bot, other) = (Instant::now(), ip("203.0.113.7"), ip("203.0.113.8"));
        assert_eq!((traps.paused_for(bot, start), traps.kept()), (None, 0));
        assert_eq!(traps.spring(bot, start), Some(Duration::from_secs(3600)));
        assert_eq!(traps.paused_for(bot, start + Duration::from_secs(600)), Some(Duration::from_secs(3000)));
        assert_eq!(traps.paused_for(other, start), None, "the address beside it is not paused");
        // Sprung again while paused: the pause keeps its end.
        assert_eq!(traps.spring(bot, start + Duration::from_secs(600)), Some(Duration::from_secs(3000)));
        // After the hour everything is as it was, and nothing is kept.
        assert_eq!((traps.paused_for(bot, start + Duration::from_secs(3600)), traps.kept()), (None, 0));
        // The button ends it at once.
        traps.spring(bot, start);
        assert!(traps.release(bot) && !traps.release(bot));
        assert_eq!((traps.paused_for(bot, start), traps.kept()), (None, 0));
        // IPv6 by its /64, IPv4 as it is; another key keeps another fingerprint.
        traps.spring(ip("2001:db8:1:2::aaaa"), start);
        assert!(traps.paused_for(ip("2001:db8:1:2:ffff::1"), start).is_some() && traps.paused_for(ip("2001:db8:1:3::aaaa"), start).is_none());
        assert_ne!(traps.fingerprint(bot), Traps::with_key([6; 32], Duration::from_secs(60)).fingerprint(bot));
        assert_ne!(traps.fingerprint(ip("0.0.0.1")), traps.fingerprint(ip("::1")));
        assert!(Traps::new(0).is_none());
        assert_eq!(Traps::new(90).map(|traps| traps.minutes()), Some(90));
    }

    #[test]
    fn ended_pauses_are_swept_out_as_the_table_grows() {
        let traps = Traps::with_key([5; 32], Duration::from_secs(60));
        let start = Instant::now();
        for n in 0..1024u32 {
            traps.spring(IpAddr::V4((0x0b00_0000 + n).into()), start);
        }
        assert_eq!(traps.kept(), 1024);
        // The next one after their pauses ended finds the table at its mark and sweeps them out.
        traps.spring(ip("203.0.113.7"), start + Duration::from_secs(61));
        assert_eq!(traps.kept(), 1);
    }
}
