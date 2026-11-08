//! The Content-Security-Policy of every HTML answer (docs/folia/frontend.md, „Content-Security-Policy").
//!
//! What it is for: Betula keeps a student's profiles and Betula Sync's keys in the browser, so a
//! script that got into a page (a hole in how a text is written out, a poisoned snapshot) is what
//! could do harm there. The policy lets a page run only the site's own files and the few scripts
//! the pages write inline, named by their hashes (`folia_app::inline_scripts`), and talk to no other
//! host: an injected script neither runs nor sends anything elsewhere.
//!
//! Hashes, not nonces: a page is the same for everybody (R9) and kept so, in the server's cache and
//! in the service worker's, and every inline script is a fixed text. A nonce would have to be new
//! in every answer, and so would every page. The hashes are computed when the server starts, from
//! the same texts the pages are written with, so a script changed changes its hash with it.
//!
//! `style` attributes are allowed (`style-src-attr 'unsafe-inline'`): the app sets its sizes and
//! places as custom properties in them (`style="--list-h:…"`), a hundred places, server side and in
//! the browser. A style cannot run a script, and what it could load (a picture, a font) only comes
//! from the site itself (`img-src`, `font-src`). `<style>` elements are allowed by their hashes as
//! the scripts are, and stylesheets only from the site.

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use base64::Engine;
use sha2::{Digest, Sha256};

/// The policy of the site at `public_url`, offering Betula Sync at `sync` (`AppState::sync`).
pub fn policy(public_url: &str, sync: Option<&str>) -> String {
    let scripts: Vec<String> = folia_app::inline_scripts().iter().map(|script| hash(script)).collect();
    let styles: Vec<String> = folia_app::INLINE_STYLES.iter().map(|style| hash(style)).collect();
    // The site itself, and where Betula Sync reaches Phloem: its WebSocket by name as well, since
    // not every browser counts a `wss:` address of its own host as `'self'` yet.
    let mut connect = vec!["'self'".to_string()];
    if let Some(sync) = sync {
        let phloem = if sync.starts_with("http://") || sync.starts_with("https://") { sync } else { public_url };
        if let Some(origin) = origin(phloem) {
            if !origin.starts_with("ws") && phloem != public_url {
                connect.push(origin.clone());
            }
            connect.push(socket(&origin));
        }
    }
    connect.dedup();
    [
        "default-src 'self'".to_string(),
        // The app is WebAssembly (`'wasm-unsafe-eval'`, which lets a page compile it and runs no
        // text as a script); every script is a file of the site or one of the inline ones.
        format!("script-src 'self' 'wasm-unsafe-eval' {}", scripts.join(" ")),
        // A browser that knows no `-elem` and `-attr` reads this one: everything inline (with a hash
        // beside it a browser would ignore `'unsafe-inline'` and refuse every `style` attribute).
        "style-src 'self' 'unsafe-inline'".to_string(),
        format!("style-src-elem 'self' {}", styles.join(" ")),
        "style-src-attr 'unsafe-inline'".to_string(),
        // `data:`: the small SVGs app.css draws with.
        "img-src 'self' data:".to_string(),
        "font-src 'self'".to_string(),
        format!("connect-src {}", connect.join(" ")),
        // The service worker and the workers of the data and of the semantic search.
        "worker-src 'self'".to_string(),
        "manifest-src 'self'".to_string(),
        "object-src 'none'".to_string(),
        "base-uri 'none'".to_string(),
        // The login form of closed testing.
        "form-action 'self'".to_string(),
        // No page of the site is shown in a frame, its own included (`X-Frame-Options: DENY`).
        "frame-ancestors 'none'".to_string(),
    ]
    .join("; ")
}

/// How a policy names an inline script or style: `'sha256-…'` of its text as it stands in the page.
fn hash(text: &str) -> String {
    format!("'sha256-{}'", base64::engine::general_purpose::STANDARD.encode(Sha256::digest(text.as_bytes())))
}

/// `https://host:port` of an address, without its path; `None` for one without a scheme and host,
/// or with a character no host has (a space or a `;` would end the directive it stands in).
fn origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let fits = |part: &str, more: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || more.contains(c));
    (fits(scheme, "+-.") && fits(host, ".-:[]")).then(|| format!("{}://{host}", scheme.to_ascii_lowercase()))
}

/// The WebSocket form of an origin: `wss://` for `https://`, `ws://` for `http://`.
fn socket(origin: &str) -> String {
    if let Some(rest) = origin.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = origin.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        origin.to_string()
    }
}

/// Puts the policy on every HTML answer: the pages, cached or not, the login page of closed
/// testing, a 404, a 503 of a busy server. Files, feeds and the snapshot go without: a policy rules
/// a document, and a worker's script would hand it on to its worker, which needs none of its own.
pub async fn send(State(policy): State<HeaderValue>, request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let html = response.headers().get(header::CONTENT_TYPE).and_then(|value| value.to_str().ok()).is_some_and(|value| value.starts_with("text/html"));
    if html {
        response.headers_mut().insert(header::CONTENT_SECURITY_POLICY, policy);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_policy_names_every_inline_script_and_style_by_its_hash() {
        let policy = policy("https://betula.app", None);
        for script in folia_app::inline_scripts() {
            assert!(policy.contains(&hash(&script)), "{policy}");
        }
        assert!(policy.contains(&hash(folia_app::VIEW_TRANSITION_STYLE)), "{policy}");
        // The known value of a known text (`echo -n "alert(1)" | openssl sha256 -binary | base64`).
        assert_eq!(hash("alert(1)"), "'sha256-bhHHL3z2vDgxUt0W3dWQOrprscmda2Y5pLsLg4GF+pI='");
        for strict in ["default-src 'self'", "object-src 'none'", "base-uri 'none'", "frame-ancestors 'none'", "form-action 'self'"] {
            assert!(policy.split("; ").any(|directive| directive == strict), "{strict}: {policy}");
        }
        assert!(!policy.contains(" 'unsafe-eval'"), "{policy}");
        let script_src = policy.split("; ").find(|directive| directive.starts_with("script-src ")).unwrap();
        assert!(!script_src.contains("'unsafe-inline'") && !script_src.contains("data:") && !script_src.contains("http"), "{script_src}");
    }

    #[test]
    fn betula_sync_may_be_reached_where_the_site_offers_it() {
        let connect = |public_url: &str, sync: Option<&str>| {
            policy(public_url, sync).split("; ").find_map(|directive| directive.strip_prefix("connect-src ").map(str::to_string)).unwrap()
        };
        assert_eq!(connect("https://betula.app", None), "'self'");
        // On the page's own host (`/sync/v1/`): its WebSocket by name.
        assert_eq!(connect("https://canary.betula.app", Some("")), "'self' wss://canary.betula.app");
        assert_eq!(connect("http://127.0.0.1:8420", Some("/sync/v1/")), "'self' ws://127.0.0.1:8420");
        // Phloem on a port of its own while working on the site: that host, by HTTP and WebSocket.
        assert_eq!(connect("http://127.0.0.1:8420", Some("http://127.0.0.1:8200/sync/v1/")), "'self' http://127.0.0.1:8200 ws://127.0.0.1:8200");
        assert_eq!(origin("HTTPS://Example.org:8443/a?b#c").as_deref(), Some("https://Example.org:8443"));
        assert_eq!(origin("/sync/v1/"), None);
        assert_eq!(connect("https://betula.app", Some("https://x.example; script-src *")), "'self'", "no address");
    }
}
