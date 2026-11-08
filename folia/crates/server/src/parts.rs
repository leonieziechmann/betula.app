//! A calendar subscription that stays current (`/calendar/s/<id ‖ key>.ics`; Betula Sync, step 3:
//! folia/design/teilen/teilen.html, docs/phloem/protocol.md §5). A calendar app sends no key of its
//! own, so the address carries the part's id and its key. Folia fetches the part from Phloem
//! through Traefik, as an outside client does (`--sync-parts`, Traefik's entrypoint `parts`, which
//! routes the site's host to its Phloem and nothing else), opens it in memory for the answer and
//! keeps neither the key nor what the part holds. Its log writes no part of the address
//! (`subscription::redacted_path`), and Traefik's router of these addresses writes no access log.

use std::time::Duration;

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use folia_calendar::live::LivePart;

/// The bytes of a part's id and of its key, as a link holds them (`id ‖ key`).
const ID: usize = 16;
const KEY: usize = 32;

/// How long Folia waits for Phloem: a calendar service asks again later.
const WAIT: Duration = Duration::from_secs(5);

/// Where parts are fetched, and as which host.
pub struct Parts {
    base: String,
    host: Option<String>,
    client: reqwest::Client,
}

/// What fetching a part found.
#[derive(Debug)]
pub enum Fetched {
    /// The part, opened.
    Part(Box<LivePart>),
    /// Not shared any more (Phloem's 410), or no part at all (404, a link that opens nothing):
    /// either way there is no calendar to answer with.
    None,
    /// Phloem could not be asked, or did not answer as it should: a calendar service asks again.
    Unreachable,
}

impl Parts {
    /// The source of parts at `base` (`http://edge_traefik:8083/sync/v1/`), asked as the host of
    /// `public_url`; `None` for an empty base.
    pub fn new(base: &str, public_url: &str) -> Option<Parts> {
        let base = base.trim();
        if base.is_empty() {
            return None;
        }
        let base = if base.ends_with('/') { base.to_string() } else { format!("{base}/") };
        let host = public_url.trim().split_once("://").map(|(_, rest)| rest.split('/').next().unwrap_or(rest).to_string()).filter(|host| !host.is_empty());
        let client = reqwest::Client::builder().timeout(WAIT).build().ok()?;
        Some(Parts { base, host, client })
    }

    /// The part a link names (`id ‖ key`, base64url), opened.
    pub async fn fetch(&self, link: &str) -> Fetched {
        let Some((id, key)) = URL_SAFE_NO_PAD.decode(link).ok().filter(|bytes| bytes.len() == ID + KEY).map(|bytes| { let (id, key) = bytes.split_at(ID); (id.to_vec(), key.to_vec()) }) else {
            return Fetched::None;
        };
        let mut request = self.client.get(format!("{}part", self.base)).header("Betula-Part", URL_SAFE_NO_PAD.encode(&id));
        if let Some(host) = &self.host {
            request = request.header(reqwest::header::HOST, host.as_str());
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(_) => return Fetched::Unreachable,
        };
        match response.status().as_u16() {
            200 => {}
            404 | 410 => return Fetched::None,
            _ => return Fetched::Unreachable,
        }
        let Ok(body) = response.bytes().await else { return Fetched::Unreachable };
        let Some(sealed) = serde_json::from_slice::<Body>(&body).ok().and_then(|body| URL_SAFE_NO_PAD.decode(body.data).ok()) else { return Fetched::Unreachable };
        match open(&key, &id, &sealed).and_then(|plain| String::from_utf8(plain).ok()).and_then(|code| LivePart::from_code(&code)) {
            Some(part) => Fetched::Part(Box::new(part)),
            // A key that does not open it: a link that opens nothing.
            None => Fetched::None,
        }
    }
}

#[derive(serde::Deserialize)]
struct Body {
    data: String,
}

/// Opens a part sealed as the browser seals it: a 12-byte nonce, then AES-256-GCM's ciphertext and
/// tag, the associated data `"part" ‖ id`.
fn open(key: &[u8], id: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    let (nonce, body) = sealed.split_at_checked(12)?;
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let aad = [&b"part"[..], id].concat();
    cipher.decrypt(Nonce::from_slice(nonce), Payload { msg: body, aad: &aad }).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seal(key: &[u8], id: &[u8], plain: &[u8]) -> Vec<u8> {
        let cipher = Aes256Gcm::new_from_slice(key).unwrap();
        let nonce = [7u8; 12];
        let aad = [&b"part"[..], id].concat();
        let mut out = nonce.to_vec();
        out.extend(cipher.encrypt(Nonce::from_slice(&nonce), Payload { msg: plain, aad: &aad }).unwrap());
        out
    }

    #[test]
    fn a_part_opens_with_its_key_and_its_id_alone() {
        let (key, id) = ([3u8; KEY], [5u8; ID]);
        let sealed = seal(&key, &id, b"code");
        assert_eq!(open(&key, &id, &sealed).as_deref(), Some(&b"code"[..]));
        assert_eq!(open(&[4u8; KEY], &id, &sealed), None, "another key");
        assert_eq!(open(&key, &[6u8; ID], &sealed), None, "another part's id: the seal names its own");
        assert_eq!(open(&key, &id, &sealed[..10]), None);
    }

    #[test]
    fn the_source_asks_as_the_site() {
        let parts = Parts::new("http://edge_traefik:8083/sync/v1", "https://canary.betula.app").unwrap();
        assert_eq!((parts.base.as_str(), parts.host.as_deref()), ("http://edge_traefik:8083/sync/v1/", Some("canary.betula.app")));
        assert!(Parts::new("  ", "https://betula.app").is_none());
    }
}
