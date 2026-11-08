//! „Profil vom Handy holen" (folia/design/profil/konzepte.html, the second step): a device that
//! wants a profile shows a code, and a device that has it scans the code and sends it there. The
//! code is a link, `https://<host>/pair#<id ‖ key>` (`url::PAIR`, the page the computer shows it
//! on, which the phone's camera app opens as well): `id` (16 bytes) names the parcel the other
//! device leaves at Phloem (`POST /sync/v1/pair` takes the id its sender chooses), `key` is
//! the asking device's public key (ECDH, P-256, 65 bytes uncompressed). The sender seals the
//! profile's key to it with a key of its own made for this once, so whoever photographs the code
//! learns nothing that opens the parcel. Both devices show four digits of the code (`digits`): the
//! person compares them and so knows the phone scanned the screen in front of them.
//!
//! What is here holds no key and calls no service: it reads and writes the link and says how
//! often the asking device asks Phloem whether the parcel is there (`next_poll`). The browser's part
//! is `browser.rs`.

use folia_routes::url;

use super::codes::{b64, unb64};

/// The bytes of a request: the parcel's id and the asking device's public key.
pub const ID: usize = 16;
pub const PUBLIC: usize = 65;

/// How long a code asks: ten minutes, as a link of `pair` opens.
pub const REQUEST_MS: u64 = 10 * 60 * 1000;

/// The part after `#` of a request for parcel `id` to the key `public`.
pub fn fragment(id: &[u8; ID], public: &[u8; PUBLIC]) -> String {
    b64(&[&id[..], &public[..]].concat())
}

/// The parcel's id and the public key of a request's part after `#` (with or without the `#`);
/// `None` for anything else, a key that is not an uncompressed point among it.
pub fn parse(fragment: &str) -> Option<([u8; ID], [u8; PUBLIC])> {
    let bytes = unb64(fragment.trim().trim_start_matches('#'))?;
    if bytes.len() != ID + PUBLIC {
        return None;
    }
    let (id, public) = bytes.split_at(ID);
    let public: [u8; PUBLIC] = public.try_into().ok()?;
    (public.first() == Some(&4)).then_some((id.try_into().ok()?, public))
}

/// The four digits both devices show, of the first bytes of the request's hash (SHA-256 of
/// `"betula-request" ‖ id ‖ key`, which the browser computes): „0731".
pub fn digits(hash: &[u8]) -> Option<String> {
    let first: [u8; 4] = hash.get(..4)?.try_into().ok()?;
    Some(format!("{:04}", u32::from_be_bytes(first) % 10_000))
}

/// How long the asking device waits before it asks Phloem again, `elapsed` milliseconds after the
/// code was made; `None` once the code asks no more. Often at first, when the phone is at hand,
/// then less: some 70 questions in ten minutes, within the 120 an address may ask in an hour.
pub fn next_poll(elapsed: u64) -> Option<u64> {
    match elapsed {
        e if e < 60_000 => Some(3_000),
        e if e < 3 * 60_000 => Some(6_000),
        e if e < REQUEST_MS => Some(15_000),
        _ => None,
    }
}

/// What a code the scanner read is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scanned {
    /// A request of a device that wants a profile (`/pair#…`): its part after `#`.
    Request(String),
    /// A link of another device that gives one (`/sync#…`, `Sync::pair`): its part after `#`.
    Join(String),
    /// A link of Betula on another site (`canary.` and the site, say): Betula Sync of one does not
    /// reach the other.
    OtherSite,
    /// Something else.
    Other,
}

/// What `text`, read from a code, is for this site at `origin` (`https://betula.app`): each kind
/// of code at its own page.
pub fn scanned(text: &str, origin: &str) -> Scanned {
    let text = text.trim();
    let Some((before, fragment)) = text.split_once('#') else { return Scanned::Other };
    let (address, kind) = if let Some(address) = before.strip_suffix(url::PAIR).filter(|_| parse(fragment).is_some()) {
        (address, Scanned::Request(fragment.to_string()))
    } else if let Some(address) = before.strip_suffix(url::CONNECT).filter(|_| unb64(fragment).is_some_and(|bytes| bytes.len() == 48)) {
        (address, Scanned::Join(fragment.to_string()))
    } else {
        return Scanned::Other;
    };
    let ours = address == origin;
    if ours {
        kind
    } else if address.starts_with("https://") && address.contains("betula.app") {
        Scanned::OtherSite
    } else {
        Scanned::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; PUBLIC] {
        let mut key = [7u8; PUBLIC];
        key[0] = 4;
        key
    }

    #[test]
    fn a_request_is_written_and_read_again() {
        let id = [1u8; ID];
        let made = fragment(&id, &key());
        assert_eq!(made.len(), 108, "{made}");
        assert_eq!(parse(&made), Some((id, key())));
        assert_eq!(parse(&format!("#{made}")), Some((id, key())));
        // Not a request: a link of `pair` (48 bytes), too short, a compressed key.
        assert_eq!(parse(&b64(&[5u8; 48])), None);
        assert_eq!(parse(&made[..made.len() - 4]), None);
        let mut compressed = key();
        compressed[0] = 2;
        assert_eq!(parse(&fragment(&id, &compressed)), None);
    }

    #[test]
    fn the_digits_are_four_of_the_hash() {
        assert_eq!(digits(&[0, 0, 0, 7, 9]).as_deref(), Some("0007"));
        assert_eq!(digits(&[0xff, 0xff, 0xff, 0xff]).as_deref(), Some("7295"));
        assert_eq!(digits(&[1, 2, 3]), None);
    }

    #[test]
    fn the_questions_thin_out_and_end_after_ten_minutes() {
        let mut at = 0;
        let mut asked = 0;
        while let Some(wait) = next_poll(at) {
            at += wait;
            asked += 1;
        }
        assert!((60..=90).contains(&asked), "{asked} questions");
        assert!((REQUEST_MS..REQUEST_MS + 15_000).contains(&at));
        assert_eq!(next_poll(0), Some(3_000));
        assert_eq!(next_poll(REQUEST_MS), None);
    }

    #[test]
    fn a_code_read_is_told_apart() {
        let origin = "https://betula.app";
        let request = fragment(&[3u8; ID], &key());
        let join = b64(&[5u8; 48]);
        assert_eq!(scanned(&format!("{origin}/pair#{request}"), origin), Scanned::Request(request.clone()));
        assert_eq!(scanned(&format!(" {origin}/sync#{join}\n"), origin), Scanned::Join(join.clone()));
        assert_eq!(scanned(&format!("https://canary.betula.app/pair#{request}"), origin), Scanned::OtherSite);
        assert_eq!(scanned(&format!("https://evil.example/pair#{request}"), origin), Scanned::Other);
        // Each kind of code at its own page only.
        assert_eq!(scanned(&format!("{origin}/sync#{request}"), origin), Scanned::Other);
        assert_eq!(scanned(&format!("{origin}/pair#{join}"), origin), Scanned::Other);
        assert_eq!(scanned(&format!("{origin}/studyplan#{join}"), origin), Scanned::Other);
        assert_eq!(scanned(&format!("{origin}/sync#nonsense"), origin), Scanned::Other);
        assert_eq!(scanned("WIFI:S:Netz;;", origin), Scanned::Other);
    }
}
