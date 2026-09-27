//! The access gate: while Betula is in closed testing, one shared password stands in front of
//! the whole site.
//!
//! Off by default; `--access-gate` (`FOLIA_ACCESS_GATE=on`) turns it on. The password is a secret
//! and comes the way Radix gets its key (docs/operations.md §3): a file, a Docker secret, a
//! systemd credential or an environment variable, never a flag. A gate that is on and finds no
//! password keeps the server from starting: it never opens the site by accident.
//!
//! Whoever enters the password gets a cookie: the time the visit ends, signed with a key made
//! from the password. The server keeps no sessions, so a restart or a second replica changes
//! nothing, and a new password ends every visit at once.
//!
//! The gate lies around everything, the page cache included. Open stay only what a login page, a
//! home screen and a supervisor need: `/access`, the stylesheet, the font, the icons, the
//! manifest (browsers fetch it without cookies), `/healthz` and `/livez`, and a `/robots.txt`
//! that turns every crawler away; and a Studienplan's calendar subscription
//! (`/calendar/<code>.ics`, only with a code that decodes): calendar services fetch it from their
//! own servers and send no cookie.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Form;
use hmac::{Hmac, Mac};
use leptos::prelude::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::AppState;

/// The login page, and where its form goes.
pub const PATH: &str = "/access";
/// The name of the secret: `/run/secrets/folia-access-password`, `FOLIA_ACCESS_PASSWORD[_FILE]`.
pub const SECRET: &str = "folia-access-password";
/// The site's only cookie. The privacy notice describes it, its content and its 90 days
/// („Cookies" in app/src/pages/legal.rs): change it with them.
const COOKIE: &str = "betula_access";
/// How long a visit lasts. A new password ends all of them earlier.
const VISIT: Duration = Duration::from_secs(90 * 24 * 60 * 60);
/// Wrong passwords the whole site accepts per window before the form closes for the rest of it.
/// Not per address (the server keeps nothing about visitors): it bounds what guessing can try
/// (about 14 000 a day), at the price that someone guessing keeps others from logging in.
const MAX_FAILURES: u32 = 10;
const WINDOW: Duration = Duration::from_secs(60);

/// Reachable without the password.
const OPEN: &[&str] = &[
    PATH,
    "/healthz",
    crate::api::LIVENESS,
    "/robots.txt",
    app::STYLESHEET,
    app::FONT,
    app::FAVICON,
    app::FAVICON_ICO,
    app::TOUCH_ICON,
    "/apple-touch-icon-precomposed.png",
    app::ICON_192,
    app::ICON_512,
    app::ICON_MASKABLE,
    app::ICON_MASKABLE_LARGE,
    app::ICON_MONOCHROME,
    app::MANIFEST,
];

type Signature = Hmac<Sha256>;

pub struct Gate {
    key: [u8; 32],
    failures: Mutex<(Instant, u32)>,
}

impl Gate {
    pub fn new(password: &str) -> Self {
        Self { key: key_of(password), failures: Mutex::new((Instant::now(), 0)) }
    }

    /// The gate as configured: `None` when it is off, an error when it is on without a password.
    pub fn from_environment(enabled: bool) -> Result<Option<(Self, String)>, String> {
        if !enabled {
            return Ok(None);
        }
        match resolve(SECRET, Path::new("/run/secrets"))? {
            Some((password, source)) => Ok(Some((Self::new(&password), source))),
            None => Err(format!(
                "the access gate is on, but there is no password: provide a Docker secret named \"{SECRET}\", {env}_FILE=/path/to/secret, \
                 a systemd credential named \"{SECRET}\" or the {env} environment variable, or turn the gate off (FOLIA_ACCESS_GATE=off)",
                env = env_name(SECRET)
            )),
        }
    }

    /// The cookie of a visit that ends at `expires` (seconds since 1970).
    fn token(&self, expires: u64) -> Option<String> {
        let signed = signature(&self.key, &format!("visit.{expires}"))?.finalize().into_bytes();
        Some(format!("v1.{expires}.{}", hex(&signed)))
    }

    fn admits_token(&self, token: &str, now: u64) -> bool {
        let mut parts = token.split('.');
        let (Some("v1"), Some(expires), Some(signed), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return false };
        let (Ok(expires), Some(signed)) = (expires.parse::<u64>(), unhex(signed)) else { return false };
        // Compared in constant time (`verify_slice`).
        expires > now && signature(&self.key, &format!("visit.{expires}")).is_some_and(|expected| expected.verify_slice(&signed).is_ok())
    }

    fn admits(&self, headers: &HeaderMap) -> bool {
        let now = unix_now();
        headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(';'))
            .filter_map(|pair| pair.trim().strip_prefix(COOKIE)?.strip_prefix('='))
            .any(|token| self.admits_token(token, now))
    }

    /// A guess is checked like a cookie: what it signs must equal what the password signs.
    fn knows(&self, guess: &str) -> bool {
        let Some(expected) = signature(&self.key, "password").map(|signature| signature.finalize().into_bytes()) else { return false };
        signature(&key_of(guess), "password").is_some_and(|signature| signature.verify_slice(&expected).is_ok())
    }

    /// Seconds until the form opens again, if too many wrong passwords closed it.
    fn closed_for(&self) -> Option<u64> {
        let failures = self.failures.lock().ok()?;
        let left = WINDOW.checked_sub(failures.0.elapsed())?;
        (failures.1 >= MAX_FAILURES).then(|| left.as_secs().max(1))
    }

    /// Counts a wrong password; the count of this window.
    fn failed(&self) -> u32 {
        let Ok(mut failures) = self.failures.lock() else { return MAX_FAILURES };
        if failures.0.elapsed() >= WINDOW {
            *failures = (Instant::now(), 0);
        }
        failures.1 += 1;
        failures.1
    }
}

fn key_of(password: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"betula access gate v1\0");
    hash.update(password.as_bytes());
    hash.finalize().into()
}

fn signature(key: &[u8; 32], message: &str) -> Option<Signature> {
    // HMAC takes a key of any length, so this never fails; if it did, nobody would get in.
    let mut signature = Signature::new_from_slice(key).ok()?;
    signature.update(message.as_bytes());
    Some(signature)
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

/// `folia-access-password` → `FOLIA_ACCESS_PASSWORD`.
fn env_name(name: &str) -> String {
    name.to_ascii_uppercase().replace('-', "_")
}

/// The secret and where it was found, in Radix's order (`internal/secrets`): the file named by
/// `<NAME>_FILE`, a Docker secret, a systemd credential, the environment variable. A source that
/// is configured but unreadable is an error, never a reason to look further.
fn resolve(name: &str, docker_secrets: &Path) -> Result<Option<(String, String)>, String> {
    let env = env_name(name);
    let read = |path: PathBuf| -> Result<Option<String>, String> {
        match std::fs::read_to_string(&path) {
            Ok(text) if text.trim().is_empty() => Err(format!("{} is empty", path.display())),
            Ok(text) => Ok(Some(text.trim().to_string())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("{}: {error}", path.display())),
        }
    };

    if let Some(path) = std::env::var_os(format!("{env}_FILE")).filter(|path| !path.is_empty()) {
        return match read(PathBuf::from(&path))? {
            Some(value) => Ok(Some((value, format!("file named by {env}_FILE")))),
            None => Err(format!("{env}_FILE: {} does not exist", PathBuf::from(&path).display())),
        };
    }
    for file in [name.to_string(), name.replace('-', "_")] {
        if let Some(value) = read(docker_secrets.join(&file))? {
            return Ok(Some((value, format!("Docker secret {file}"))));
        }
    }
    if let Some(dir) = std::env::var_os("CREDENTIALS_DIRECTORY").filter(|dir| !dir.is_empty()) {
        if let Some(value) = read(PathBuf::from(dir).join(name))? {
            return Ok(Some((value, "systemd credential".to_string())));
        }
    }
    match std::env::var(&env) {
        Ok(value) if !value.trim().is_empty() => Ok(Some((value.trim().to_string(), format!("environment variable {env}")))),
        _ => Ok(None),
    }
}

/// Middleware around the whole site.
pub async fn gate(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(gate) = state.gate.as_ref() else { return next.run(request).await };
    let path = request.uri().path();
    // The launch screens of iOS are the home screen's like its icons (`app::launch`); another name
    // under their path stays behind the gate.
    let launch_screen = path.strip_prefix(app::launch::PATH).is_some_and(|file| app::launch::Picture::from_file(file).is_some());
    // A calendar feed passes when its code decodes: the owner's decision of 2026-09-24, since a
    // calendar service has no password to give. The check characters turn guesses away before any
    // handler runs, and what it shows is the QIS schedule of the modules the code names; every
    // other path under `/calendar/` stays behind the gate. Its answer is `private` already.
    // Every language's manifest is the home screen's, like the icons.
    let manifest = catalog::Locale::split(path).1 == app::MANIFEST;
    if OPEN.contains(&path) || launch_screen || manifest || catalog::timetable::subscription::is_feed_path(path) {
        return next.run(request).await;
    }
    if !gate.admits(request.headers()) {
        return turn_away(&request);
    }
    let mut response = next.run(request).await;
    // What lies behind the gate belongs to this visitor's browser, not to a cache shared with others.
    let private = response.headers().get(header::CACHE_CONTROL).and_then(|value| value.to_str().ok()).filter(|value| value.contains("public")).map(|value| value.replace("public", "private"));
    if let Some(value) = private.and_then(|value| HeaderValue::from_str(&value).ok()) {
        response.headers_mut().insert(header::CACHE_CONTROL, value);
    }
    response
}

/// Someone opening a page is led to the login page and back; everything else hears 401.
fn turn_away(request: &Request) -> Response {
    let wants_page = matches!(*request.method(), Method::GET | Method::HEAD)
        && request.headers().get(header::ACCEPT).and_then(|value| value.to_str().ok()).is_some_and(|value| value.contains("text/html"));
    if !wants_page {
        return (StatusCode::UNAUTHORIZED, [(header::CACHE_CONTROL, "no-store")], "Betula is in closed testing: open /access and enter the password.\n").into_response();
    }
    let wanted = way_back(request.uri().path_and_query().map(|wanted| wanted.as_str()));
    let location = if wanted == "/" { PATH.to_string() } else { format!("{PATH}?next={}", catalog::url::encode(&wanted)) };
    (StatusCode::FOUND, [(header::LOCATION, location), (header::CACHE_CONTROL, "no-store".to_string())]).into_response()
}

/// Where a login leads: a path of this site and nothing else (never another host, never the login page).
fn way_back(next: Option<&str>) -> String {
    let next = next.unwrap_or("/");
    let local = next.starts_with('/') && !next.starts_with("//") && next.len() <= 2048 && !next.chars().any(|c| c == '\\' || c.is_control());
    let login = next == PATH || next.strip_prefix(PATH).is_some_and(|rest| rest.starts_with(['?', '/', '#']));
    if local && !login { next.to_string() } else { "/".to_string() }
}

#[derive(Deserialize)]
pub struct Wanted {
    next: Option<String>,
}

#[derive(Deserialize)]
pub struct Login {
    #[serde(default)]
    password: String,
    next: Option<String>,
}

fn redirect(to: &str) -> Response {
    (StatusCode::SEE_OTHER, [(header::LOCATION, to.to_string()), (header::CACHE_CONTROL, "no-store".to_string())]).into_response()
}

/// `GET /access`: the login page. Whoever is in already (or meets no gate) goes on.
pub async fn page(State(state): State<AppState>, Query(wanted): Query<Wanted>, headers: HeaderMap) -> Response {
    let next = way_back(wanted.next.as_deref());
    match state.gate.as_ref() {
        Some(gate) if !gate.admits(&headers) => login_page(&state, StatusCode::OK, &next, None),
        _ => redirect(&next),
    }
}

/// `POST /access`: the password opens the gate for this browser.
pub async fn enter(State(state): State<AppState>, headers: HeaderMap, Form(login): Form<Login>) -> Response {
    let next = way_back(login.next.as_deref());
    let Some(gate) = state.gate.as_ref() else { return redirect(&next) };

    if let Some(seconds) = gate.closed_for() {
        let mut response = login_page(&state, StatusCode::TOO_MANY_REQUESTS, &next, Some(language_of(&next).gate_closed));
        response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        return response;
    }
    if !gate.knows(login.password.trim()) {
        let failures = gate.failed();
        tracing::warn!(component = "access", event = "access.denied", failures, closed = failures >= MAX_FAILURES, "a wrong access password was entered");
        return login_page(&state, StatusCode::UNAUTHORIZED, &next, Some(language_of(&next).gate_wrong));
    }

    let Some(token) = gate.token(unix_now() + VISIT.as_secs()) else { return StatusCode::INTERNAL_SERVER_ERROR.into_response() };
    // Behind the proxy the site is HTTPS and the cookie must never travel without it; a
    // development server on plain HTTP could not set a `Secure` cookie at all.
    let secure = headers.get("x-forwarded-proto").and_then(|value| value.to_str().ok()).is_some_and(|proto| proto.eq_ignore_ascii_case("https"));
    let cookie = format!("{COOKIE}={token}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax{}", VISIT.as_secs(), if secure { "; Secure" } else { "" });
    tracing::info!(component = "access", event = "access.granted", "the access password was entered");
    let mut response = redirect(&next);
    if let Ok(cookie) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    response
}

/// The texts of the login page: in the language of the page the visitor is on the way to.
fn language_of(next: &str) -> &'static crate::texts::Texts {
    crate::texts::texts(catalog::Locale::split(next).0)
}

/// The login page: a document of its own with the site's stylesheet, no app and no script but
/// the one that applies the remembered theme. Works without JavaScript. The stylesheet carries
/// the build like on every page (`app::BuildId`). It speaks the language of the page the visitor
/// is on the way to.
fn login_page(state: &AppState, status: StatusCode, next: &str, problem: Option<&'static str>) -> Response {
    let locale = catalog::Locale::split(next).0;
    let (t, app_texts) = (crate::texts::texts(locale), app::i18n::texts(locale));
    let next = next.to_string();
    let stylesheet = app::BuildId(state.build_id.clone()).asset(app::STYLESHEET);
    let html = view! {
        <!DOCTYPE html>
        <html lang=locale.code()>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover"/>
                <meta name="color-scheme" content="light dark"/>
                <meta name="robots" content="noindex, nofollow"/>
                <meta name="theme-color" content=app::THEME_LIGHT/>
                <title>{format!("{} · Betula", t.gate_title)}</title>
                <link rel="icon" href=app::FAVICON_ICO sizes="32x32"/>
                <link rel="icon" type="image/svg+xml" href=app::FAVICON/>
                <link rel="apple-touch-icon" href=app::TOUCH_ICON/>
                // `as` comes first: after a value the macro would read it as a cast.
                <link as="font" rel="preload" type="font/woff2" crossorigin="anonymous" href=app::FONT/>
                <link rel="stylesheet" href=stylesheet/>
                <style inner_html=app::VIEW_TRANSITION_STYLE></style>
                <script inner_html=app::HEAD_SCRIPT></script>
            </head>
            <body class="gate">
                <main class="gate-main">
                    // The lockup stands on the page like in the app: on a panel the light mark would vanish.
                    <div class="gate-brand">
                        <span class="logo"><app::ui::Mark/></span>
                        <span><app::ui::Wordmark/><small>{app_texts.common.tagline}</small></span>
                    </div>
                    <section class="gate-panel">
                        <h1>{t.gate_title}</h1>
                        <p>{t.gate_text}</p>
                        <form method="post" action=PATH>
                            <input type="hidden" name="next" value=next/>
                            // Password managers file a password under a name.
                            <input class="visually-hidden" type="text" name="username" value="Betula" autocomplete="username" tabindex="-1" aria-hidden="true"/>
                            <label for="password">{t.gate_password}</label>
                            <input id="password" type="password" name="password" required autofocus autocomplete="current-password" aria-describedby=problem.map(|_| "problem") aria-invalid=problem.map(|_| "true")/>
                            {problem.map(|problem| view! { <p class="gate-problem" id="problem" role="alert">{problem}</p> })}
                            <button class="button" type="submit">{t.gate_open}</button>
                        </form>
                    </section>
                </main>
            </body>
        </html>
    }
    .to_html();
    (
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store"), (header::HeaderName::from_static("x-robots-tag"), "noindex, nofollow")],
        html,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_visit_is_signed_and_ends() {
        let gate = Gate::new("birke im tagebau");
        let token = gate.token(2_000).unwrap();
        assert!(gate.admits_token(&token, 1_999));
        assert!(!gate.admits_token(&token, 2_000), "the visit is over");
        // A later end needs a new signature, and another password signs differently.
        assert!(!gate.admits_token(&token.replace("v1.2000.", "v1.9000."), 1_999));
        assert!(!Gate::new("birke im tagebau ").admits_token(&token, 1_999));
        assert!(Gate::new("birke im tagebau").admits_token(&token, 1_999), "no state but the password: a restart keeps every visit");
        for broken in ["", "v1", "v1.2000", "v1.2000.", "v1.2000.zz", "v2.2000.00", &format!("{token}.x"), &token[..token.len() - 1]] {
            assert!(!gate.admits_token(broken, 0), "{broken}");
        }

        assert!(gate.knows("birke im tagebau") && !gate.knows("birke") && !gate.knows(""));

        let mut headers = HeaderMap::new();
        assert!(!gate.admits(&headers));
        let live = gate.token(unix_now() + 60).unwrap();
        headers.append(header::COOKIE, HeaderValue::from_static("theme=dark"));
        headers.append(header::COOKIE, HeaderValue::from_str(&format!("other=1; {COOKIE}={live}")).unwrap());
        assert!(gate.admits(&headers));
    }

    #[test]
    fn a_login_leads_nowhere_but_into_the_site() {
        assert_eq!(way_back(Some("/catalog?q=mathe&open=11101")), "/catalog?q=mathe&open=11101");
        for foreign in ["//evil.example", "https://evil.example/", "/\\evil.example", "catalog", "", "/a\nb", "/access", "/access?next=/access", "/access/"] {
            assert_eq!(way_back(Some(foreign)), "/", "{foreign:?}");
        }
        assert_eq!(way_back(Some("/accessories")), "/accessories");
        assert_eq!(way_back(None), "/");
    }

    #[test]
    fn too_many_wrong_passwords_close_the_form() {
        let gate = Gate::new("x");
        for _ in 0..MAX_FAILURES - 1 {
            gate.failed();
            assert!(gate.closed_for().is_none());
        }
        assert_eq!(gate.failed(), MAX_FAILURES);
        assert!(gate.closed_for().is_some_and(|seconds| (1..=WINDOW.as_secs()).contains(&seconds)));
    }

    #[test]
    fn the_password_is_found_like_radix_finds_its_key() {
        let dir = std::env::temp_dir().join(format!("folia-access-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A name no other test and no real environment uses.
        let name = "folia-test-gate-secret";
        assert_eq!(resolve(name, &dir).unwrap(), None);

        std::fs::write(dir.join("folia_test_gate_secret"), "  aus dem secret\n").unwrap();
        assert_eq!(resolve(name, &dir).unwrap(), Some(("aus dem secret".to_string(), "Docker secret folia_test_gate_secret".to_string())));
        std::fs::write(dir.join(name), "\n").unwrap();
        assert!(resolve(name, &dir).unwrap_err().contains("is empty"), "an empty secret is an error, not a reason to look further");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
