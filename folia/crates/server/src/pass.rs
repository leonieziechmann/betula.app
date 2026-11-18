//! The pass: what a browser shows before it gets the catalog (`/api/db`).
//!
//! Owner, 2026-11-18: „ich glaube wir brauchen doch mal ne bot Challenge auf der Seite. Weil ich
//! werde komplett überrant mit anfragen." The site stays open as it is: every page, the files and
//! the calendar feeds answer whoever asks, search engines, link previews and visitors without
//! JavaScript included. What costs is the download of the catalog, 4.4 MB in brotli and 44 MB
//! for a client that takes no compression, which every first visit of the app makes and which
//! clients that only collect addresses from the scripts (`"/api/db"` stands in the data worker)
//! fetch like any other. That download asks for a pass now.
//!
//! A pass is earned with a little work the browser does and the server checks at once: the
//! smallest number for which SHA-256 of a seed the server chose and that number begins with
//! `difficulty` zero bits (18 by default: about 260,000 tries, a tenth of a second on a laptop,
//! half a second on a phone). The data worker does it by itself, nobody sees it, and it is done
//! once per network and week: whoever comes back with the catalog kept asks for nothing, and a
//! new snapshot is downloaded with the pass the browser has. A client that runs no script never
//! gets the catalog, and one that does pays a moment of a processor per network it comes from.
//!
//!   GET /api/db without a pass   401, `WWW-Authenticate: Betula-Pass`, the challenge in JSON:
//!                                `{"challenge": "c1.<issued>.<difficulty>.<seed>.<mac>",
//!                                "seed": "<seed>", "difficulty": 18}`
//!   POST /api/pass               the form `challenge=…&nonce=…` (the number in decimal digits):
//!                                204 and the cookie `betula_pass`, or 403 with a new challenge
//!
//! Nothing is kept on the server: the challenge and the pass are signed with a key the process
//! makes when it starts, and both are bound to the network the request comes from (IPv4 /24,
//! IPv6 /64, as Traefik's rate limit counts them; the address is Traefik's `X-Forwarded-For`),
//! so a pass solved once does not serve a whole pool of addresses. A restart, the other colour of
//! a blue-green switch, or a phone that changed networks asks for a new one, which the worker
//! earns again in the same way. A challenge is good for ten minutes, a pass for seven days.
//! `--pass-difficulty 0` (`FOLIA_PASS_DIFFICULTY=0`) turns it off.

use std::net::IpAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Form, Json};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::AppState;

/// Where a solved challenge goes.
pub const PATH: &str = "/api/pass";
/// The pass, sent only with the requests of the API. The privacy notice describes it („Cookies"
/// in folia/crates/home/src/legal.rs): change it with them.
pub const COOKIE: &str = "betula_pass";
/// The difficulty unless configured: about 2^18 tries.
pub const DEFAULT_DIFFICULTY: u8 = 18;
/// More would keep a phone busy for minutes; the check reads the first 64 bits.
pub const MAX_DIFFICULTY: u8 = 32;
/// How long a pass lasts.
pub const PASS_FOR: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// How long a challenge may be worked on.
const CHALLENGE_FOR: Duration = Duration::from_secs(10 * 60);
/// The digits of a nonce: a u64 has at most 20.
const MAX_NONCE_DIGITS: usize = 20;

type Signature = Hmac<Sha256>;

pub struct Passes {
    key: [u8; 32],
    difficulty: u8,
}

/// Why a solution was not taken.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// Not a challenge of this process, or not one for this network.
    Forged,
    /// Older than ten minutes, or from the future.
    Expired,
    /// Easier than the server asks for now.
    TooEasy,
    /// The number does not do what the challenge asks.
    Wrong,
}

impl Passes {
    /// Passes of the given difficulty, with a key of their own; `None` for 0 (off).
    pub fn new(difficulty: u8) -> Option<Self> {
        if difficulty == 0 {
            return None;
        }
        let mut key = [0u8; 32];
        // Without the system's randomness nobody could trust the key: better no pass than a guessable one.
        getrandom::getrandom(&mut key).ok()?;
        Some(Self::with_key(key, difficulty))
    }

    pub fn with_key(key: [u8; 32], difficulty: u8) -> Self {
        Self { key, difficulty: difficulty.min(MAX_DIFFICULTY) }
    }

    pub fn difficulty(&self) -> u8 {
        self.difficulty
    }

    fn mac(&self, message: &str) -> Option<Signature> {
        let mut mac = Signature::new_from_slice(&self.key).ok()?;
        mac.update(message.as_bytes());
        Some(mac)
    }

    /// A new challenge for `network`.
    pub fn challenge(&self, network: &str, now: u64) -> Option<(String, String)> {
        let mut random = [0u8; 12];
        getrandom::getrandom(&mut random).ok()?;
        let seed = hex(&random);
        let body = format!("c1.{now}.{}.{seed}", self.difficulty);
        let mac = self.mac(&format!("challenge {body} for {network}"))?.finalize().into_bytes();
        Some((format!("{body}.{}", hex(&mac)), seed))
    }

    /// Checks a solved challenge from `network`.
    pub fn check(&self, challenge: &str, nonce: &str, network: &str, now: u64) -> Result<(), Refused> {
        let mut parts = challenge.split('.');
        let (Some("c1"), Some(issued), Some(difficulty), Some(seed), Some(mac), None) = (parts.next(), parts.next(), parts.next(), parts.next(), parts.next(), parts.next()) else {
            return Err(Refused::Forged);
        };
        let (Ok(issued), Ok(difficulty), Some(mac)) = (issued.parse::<u64>(), difficulty.parse::<u8>(), unhex(mac)) else { return Err(Refused::Forged) };
        let body = format!("c1.{issued}.{difficulty}.{seed}");
        // Compared in constant time (`verify_slice`).
        if !self.mac(&format!("challenge {body} for {network}")).is_some_and(|expected| expected.verify_slice(&mac).is_ok()) {
            return Err(Refused::Forged);
        }
        if issued > now + 5 || now.saturating_sub(issued) > CHALLENGE_FOR.as_secs() {
            return Err(Refused::Expired);
        }
        if difficulty < self.difficulty {
            return Err(Refused::TooEasy);
        }
        if nonce.is_empty() || nonce.len() > MAX_NONCE_DIGITS || !nonce.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Refused::Wrong);
        }
        if leading_zeros(&Sha256::digest(format!("{seed}{nonce}").as_bytes())) < u32::from(difficulty) {
            return Err(Refused::Wrong);
        }
        Ok(())
    }

    /// The pass of `network` until `expires`.
    pub fn pass(&self, network: &str, expires: u64) -> Option<String> {
        let mac = self.mac(&format!("pass p1.{expires} for {network}"))?.finalize().into_bytes();
        Some(format!("p1.{expires}.{}", hex(&mac)))
    }

    fn admits_pass(&self, pass: &str, network: &str, now: u64) -> bool {
        let mut parts = pass.split('.');
        let (Some("p1"), Some(expires), Some(mac), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return false };
        let (Ok(expires), Some(mac)) = (expires.parse::<u64>(), unhex(mac)) else { return false };
        expires > now && self.mac(&format!("pass p1.{expires} for {network}")).is_some_and(|expected| expected.verify_slice(&mac).is_ok())
    }

    /// Does this request carry a pass of its network?
    pub fn admits(&self, headers: &HeaderMap) -> bool {
        let network = network_of(headers);
        let now = unix_now();
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(';'))
            .filter_map(|pair| pair.trim().strip_prefix(COOKIE)?.strip_prefix('='))
            .any(|pass| self.admits_pass(pass, &network, now))
    }

    /// The answer to a request without a pass: a challenge to earn one with.
    pub fn refusal(&self, headers: &HeaderMap, status: StatusCode) -> Response {
        let Some((challenge, seed)) = self.challenge(&network_of(headers), unix_now()) else {
            return (StatusCode::SERVICE_UNAVAILABLE, [(header::CACHE_CONTROL, "no-store")]).into_response();
        };
        let body = serde_json::json!({ "challenge": challenge, "seed": seed, "difficulty": self.difficulty });
        let mut response = (status, [(header::CACHE_CONTROL, "no-store")], Json(body)).into_response();
        if status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Betula-Pass"));
        }
        response
    }
}

/// The network a request comes from, as a pass is bound to it: the address Traefik saw (the last
/// of `X-Forwarded-For`; Traefik drops what a client sent itself) cut to its /24 or /64; `direct`
/// without one (the server asked without Traefik, while working on the site).
pub fn network_of(headers: &HeaderMap) -> String {
    match address_of(headers) {
        Some(IpAddr::V4(address)) => {
            let [a, b, c, _] = address.octets();
            format!("{a}.{b}.{c}.0/24")
        }
        Some(IpAddr::V6(address)) => {
            let s = address.segments();
            format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
        None => "direct".to_string(),
    }
}

/// The address a request comes from: the one Traefik saw, the last of `X-Forwarded-For` (Traefik
/// drops what a client sent itself); `None` without one (the server asked without Traefik: its
/// own probe, the warm-up, working on the site).
pub fn address_of(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .last()
        .and_then(|last| last.trim().parse::<IpAddr>().ok())
        .map(|address| address.to_canonical())
}

#[derive(Deserialize)]
pub struct Solved {
    #[serde(default)]
    challenge: String,
    #[serde(default)]
    nonce: String,
}

/// `POST /api/pass`: a solved challenge earns the pass of the network it came from.
pub async fn enter(State(state): State<AppState>, headers: HeaderMap, Form(solved): Form<Solved>) -> Response {
    let Some(passes) = state.passes.as_ref() else { return (StatusCode::NO_CONTENT, [(header::CACHE_CONTROL, "no-store")]).into_response() };
    let network = network_of(&headers);
    let now = unix_now();
    if let Err(why) = passes.check(&solved.challenge, &solved.nonce, &network, now) {
        tracing::debug!(component = "pass", event = "pass.refused", reason = ?why, "a solution was not taken");
        return passes.refusal(&headers, StatusCode::FORBIDDEN);
    }
    let Some(pass) = passes.pass(&network, now + PASS_FOR.as_secs()) else { return StatusCode::INTERNAL_SERVER_ERROR.into_response() };
    // Behind the proxy the site is HTTPS and the cookie never travels without it; a development
    // server on plain HTTP could not set a `Secure` cookie at all.
    let secure = headers.get("x-forwarded-proto").and_then(|value| value.to_str().ok()).is_some_and(|proto| proto.eq_ignore_ascii_case("https"));
    let cookie = format!("{COOKIE}={pass}; Path=/api/; Max-Age={}; HttpOnly; SameSite=Strict{}", PASS_FOR.as_secs(), if secure { "; Secure" } else { "" });
    let mut response = (StatusCode::NO_CONTENT, [(header::CACHE_CONTROL, "no-store")]).into_response();
    if let Ok(cookie) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    response
}

/// The zero bits a hash begins with.
pub fn leading_zeros(hash: &[u8]) -> u32 {
    let mut zeros = 0;
    for byte in hash {
        if *byte != 0 {
            return zeros + byte.leading_zeros();
        }
        zeros += 8;
    }
    zeros
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    text.as_bytes().chunks(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()).collect()
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The smallest nonce that solves `seed` at `difficulty`: what the data worker computes
/// (folia/crates/worker/js/data-worker.js, `earnPass`), for the checks.
#[cfg(test)]
pub fn solve(seed: &str, difficulty: u8) -> u64 {
    (0u64..).find(|nonce| leading_zeros(&Sha256::digest(format!("{seed}{nonce}").as_bytes())) >= u32::from(difficulty)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(), HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn a_network_is_a_slash_24_or_a_slash_64() {
        assert_eq!(network_of(&headers(&[("x-forwarded-for", "203.0.113.77")])), "203.0.113.0/24");
        assert_eq!(network_of(&headers(&[("x-forwarded-for", "198.51.100.9, 203.0.113.77")])), "203.0.113.0/24", "the last address is Traefik's");
        assert_eq!(network_of(&headers(&[("x-forwarded-for", "2001:db8:aa:bb:1:2:3:4")])), "2001:db8:aa:bb::/64");
        assert_eq!(network_of(&headers(&[("x-forwarded-for", "::ffff:203.0.113.5")])), "203.0.113.0/24");
        assert_eq!(network_of(&headers(&[])), "direct");
        assert_eq!(network_of(&headers(&[("x-forwarded-for", "nonsense")])), "direct");
    }

    #[test]
    fn leading_zeros_count_bits() {
        assert_eq!(leading_zeros(&[0x80]), 0);
        assert_eq!(leading_zeros(&[0x00, 0x01]), 15);
        assert_eq!(leading_zeros(&[0x00, 0x00, 0x20]), 18);
        assert_eq!(leading_zeros(&[0, 0]), 16);
    }

    #[test]
    fn a_challenge_is_checked_against_its_network_its_age_and_its_work() {
        let passes = Passes::with_key([7; 32], 8);
        let net = "203.0.113.0/24";
        let (challenge, seed) = passes.challenge(net, 1_000_000).unwrap();
        let nonce = solve(&seed, 8).to_string();
        assert_eq!(passes.check(&challenge, &nonce, net, 1_000_000), Ok(()));
        assert_eq!(passes.check(&challenge, &nonce, net, 1_000_000 + 600), Ok(()));
        assert_eq!(passes.check(&challenge, &nonce, net, 1_000_000 + 601), Err(Refused::Expired));
        assert_eq!(passes.check(&challenge, &nonce, "198.51.100.0/24", 1_000_000), Err(Refused::Forged), "bound to its network");
        assert_eq!(Passes::with_key([8; 32], 8).check(&challenge, &nonce, net, 1_000_000), Err(Refused::Forged), "of another process");
        assert_eq!(Passes::with_key([7; 32], 9).check(&challenge, &nonce, net, 1_000_000), Err(Refused::TooEasy));
        let wrong = (0u64..).map(|n| n.to_string()).find(|n| leading_zeros(&Sha256::digest(format!("{seed}{n}").as_bytes())) < 8).unwrap();
        assert_eq!(passes.check(&challenge, &wrong, net, 1_000_000), Err(Refused::Wrong));
        for nonce in ["", "-1", "1e3", " 12", "123456789012345678901"] {
            assert_eq!(passes.check(&challenge, nonce, net, 1_000_000), Err(Refused::Wrong), "{nonce:?}");
        }
        // The seed is part of what is signed: another seed with the same signature is forged.
        let altered = challenge.replacen(&seed, &"0".repeat(seed.len()), 1);
        assert_eq!(passes.check(&altered, "0", net, 1_000_000), Err(Refused::Forged));
        let raised = challenge.replacen(".8.", ".1.", 1);
        assert_eq!(passes.check(&raised, &nonce, net, 1_000_000), Err(Refused::Forged), "the difficulty is signed too");
    }

    #[test]
    fn a_pass_holds_for_its_network_until_it_expires() {
        let passes = Passes::with_key([7; 32], 8);
        let pass = passes.pass("203.0.113.0/24", 2_000).unwrap();
        assert!(passes.admits_pass(&pass, "203.0.113.0/24", 1_999));
        assert!(!passes.admits_pass(&pass, "203.0.113.0/24", 2_000));
        assert!(!passes.admits_pass(&pass, "203.0.114.0/24", 1_000));
        assert!(!passes.admits_pass(&pass.replacen("p1.2000", "p1.9999", 1), "203.0.113.0/24", 1_000));
        assert!(!Passes::with_key([8; 32], 8).admits_pass(&pass, "203.0.113.0/24", 1_000));
        assert!(!passes.admits_pass("p1.9999999999.00", "203.0.113.0/24", 1_000));
        assert!(Passes::new(0).is_none());
        assert_eq!(Passes::with_key([0; 32], 99).difficulty(), MAX_DIFFICULTY);
    }
}
