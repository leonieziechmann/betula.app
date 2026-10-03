//! Content codings: what goes out compressed, and how (`Accept-Encoding`, `Content-Encoding`).
//!
//! Brotli to every client that takes it (every browser does, over HTTPS), gzip to one that takes
//! only gzip (some crawlers and fetchers of link previews), the plain bytes to the rest. What is
//! made once and kept — the files of the app, the map of the programs, the snapshot — is
//! compressed at brotli's best, off the threads that answer requests; what is made as it is asked
//! for — pages, calendar feeds — at a quality as fast as gzip's.
//!
//! Measured 2026-09-30 against gzip -6, which everything went out as before: the stylesheet 63 →
//! 51 kB, sql.js's WASM 319 → 276 kB, the browser bundle 2.8 → 1.7 MB, the snapshot 7.6 → 4.4 MB
//! (brotli's window of megabytes finds what repeats far apart, gzip's of 32 kB does not), the
//! start page 118 → 95 kB, a module 7.8 → 7.4 kB.

use std::io::{Read, Write};

use axum::body::Bytes;
use axum::http::{header, HeaderMap, HeaderValue};

/// Pages and calendar feeds, compressed as they are made: as fast as gzip -6 (the start page in
/// 15 ms, a module in 2), and a twentieth to a fifth smaller.
pub const FAST: u32 = 5;
/// What is made once and kept: another tenth to a fifth smaller than `FAST`, ten to a hundred
/// times slower (the stylesheet 0.4 s, sql.js's WASM 1.1 s).
pub const BEST: u32 = 11;
/// What is made once but larger than `LARGE_BYTES`, where the best would take too long for the
/// first visitor who asks: the browser bundle (34 MB) in 2 s instead of 31 s for a tenth more,
/// the sitemap (4 MB) in 0.3 s instead of 8.5 s for a twentieth more.
pub const LARGE: u32 = 9;
pub const LARGE_BYTES: usize = 1 << 20;

/// A content coding of an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coding {
    Brotli,
    Gzip,
    Identity,
}

impl Coding {
    /// What the client takes best, by the weights of its `Accept-Encoding` (RFC 9110 §12.5.3):
    /// brotli where it weighs brotli at least as much as gzip, gzip where it takes only that, the
    /// plain bytes where it takes neither.
    pub fn of(headers: &HeaderMap) -> Coding {
        let (brotli, gzip) = weights(headers);
        if brotli > 0.0 && brotli >= gzip {
            Coding::Brotli
        } else if gzip > 0.0 {
            Coding::Gzip
        } else {
            Coding::Identity
        }
    }

    /// Whether the client takes this coding at all (the plain bytes it always does).
    pub fn taken_by(self, headers: &HeaderMap) -> bool {
        let (brotli, gzip) = weights(headers);
        match self {
            Coding::Brotli => brotli > 0.0,
            Coding::Gzip => gzip > 0.0,
            Coding::Identity => true,
        }
    }

    /// The `Content-Encoding` of an answer in this coding.
    pub fn header(self) -> Option<HeaderValue> {
        match self {
            Coding::Brotli => Some(HeaderValue::from_static("br")),
            Coding::Gzip => Some(HeaderValue::from_static("gzip")),
            Coding::Identity => None,
        }
    }
}

/// The weights of brotli and gzip in a client's `Accept-Encoding`, 0 for what it does not take.
/// `*` stands for every coding it does not name; a weight that cannot be read is no weight.
fn weights(headers: &HeaderMap) -> (f32, f32) {
    let (mut brotli, mut gzip, mut other) = (None, None, None);
    for value in headers.get_all(header::ACCEPT_ENCODING).iter().filter_map(|value| value.to_str().ok()) {
        for item in value.split(',') {
            let mut parts = item.split(';').map(str::trim);
            let name = parts.next().unwrap_or_default();
            let weight = parts.find_map(|part| part.strip_prefix("q=").or_else(|| part.strip_prefix("Q="))).map_or(1.0, |q| q.trim().parse::<f32>().unwrap_or(1.0));
            if name.eq_ignore_ascii_case("br") {
                brotli = Some(weight);
            } else if name.eq_ignore_ascii_case("gzip") || name.eq_ignore_ascii_case("x-gzip") {
                gzip = Some(weight);
            } else if name == "*" {
                other = Some(weight);
            }
        }
    }
    (brotli.or(other).unwrap_or(0.0), gzip.or(other).unwrap_or(0.0))
}

/// `body` compressed with brotli at `quality`; empty should that fail (it does not, into memory).
pub fn brotli(body: &[u8], quality: u32) -> Bytes {
    let mut compressed = Vec::with_capacity(body.len() / 4);
    match brotli_stream(&mut &body[..], &mut compressed, quality, body.len()) {
        Ok(()) => Bytes::from(compressed),
        Err(_) => Bytes::new(),
    }
}

/// `body` compressed to be kept: at brotli's best, or at `LARGE` when it is larger than
/// `LARGE_BYTES`.
pub fn brotli_to_keep(body: &[u8]) -> Bytes {
    brotli(body, if body.len() > LARGE_BYTES { LARGE } else { BEST })
}

/// Compresses the `size` bytes `input` holds into `output`, with a window as large as they need:
/// a larger one would only cost memory. At most brotli's 16 MiB, which every browser reads, and
/// 4 MiB at qualities 10 and 11, whose memory grows with the window: the snapshot (44 MB) at 11
/// took about 220 MB with 16 MiB and 120 MB with 4 MiB, for 4.2 and 4.4 MB.
pub fn brotli_stream(input: &mut impl Read, output: &mut impl Write, quality: u32, size: usize) -> std::io::Result<()> {
    let quality = quality.min(BEST);
    let lgwin = window_bits(size).min(if quality >= 10 { 22 } else { 24 });
    let params = brotli::enc::BrotliEncoderParams { quality: quality as i32, lgwin, size_hint: size, ..Default::default() };
    brotli::BrotliCompress(input, output, &params).map(|_| ())
}

/// The smallest window that holds `size` bytes, in bits: brotli's window is 2^bits − 16 bytes,
/// with 10 to 24 bits.
fn window_bits(size: usize) -> i32 {
    (10..24).find(|bits| (1usize << bits) - 16 >= size).unwrap_or(24)
}

/// What `brotli` compressed; `None` for what is not brotli.
pub fn unbrotli(compressed: &[u8]) -> Option<Bytes> {
    let mut body = Vec::with_capacity(compressed.len() * 5);
    brotli::Decompressor::new(compressed, 64 * 1024).read_to_end(&mut body).ok()?;
    Some(Bytes::from(body))
}

pub fn gzip(body: &[u8]) -> Bytes {
    let mut encoder = flate2::write::GzEncoder::new(Vec::with_capacity(body.len() / 4), flate2::Compression::new(6));
    match encoder.write_all(body).and_then(|_| encoder.finish()) {
        Ok(compressed) => Bytes::from(compressed),
        Err(_) => Bytes::new(),
    }
}

#[cfg(test)]
pub fn gunzip(compressed: &[u8]) -> Option<Bytes> {
    let mut body = Vec::with_capacity(compressed.len() * 5);
    flate2::read::GzDecoder::new(compressed).read_to_end(&mut body).ok()?;
    Some(Bytes::from(body))
}

/// `body` in `coding`: the compressed form when it is one and smaller, else the plain bytes and
/// `Identity`. `compress` makes the form, and is only asked when one is wanted.
pub fn smaller(body: Bytes, coding: Coding, compress: impl FnOnce(&[u8]) -> Bytes) -> (Bytes, Coding) {
    if coding == Coding::Identity {
        return (body, Coding::Identity);
    }
    let compressed = compress(&body);
    if !compressed.is_empty() && compressed.len() < body.len() {
        (compressed, coding)
    } else {
        (body, Coding::Identity)
    }
}

/// A body made once and kept (a file of the app, the map of the programs, the sitemap), with its
/// compressed forms: each made when a client first asks for it, off the threads that answer
/// requests, and kept beside it; whoever asks while it is being made waits for it. Brotli is
/// `brotli_to_keep`'s, unless it came made ahead of time (`with_brotli`).
pub struct Kept {
    plain: Bytes,
    brotli: tokio::sync::OnceCell<Bytes>,
    gzip: tokio::sync::OnceCell<Bytes>,
}

impl Kept {
    pub fn new(plain: Bytes) -> Self {
        Self { plain, brotli: tokio::sync::OnceCell::new(), gzip: tokio::sync::OnceCell::new() }
    }

    /// With the brotli it was compressed to ahead of time (the wood's masks, `birch::brotli`).
    pub fn with_brotli(plain: Bytes, brotli: Bytes) -> Self {
        Self { plain, brotli: tokio::sync::OnceCell::new_with(Some(brotli)), gzip: tokio::sync::OnceCell::new() }
    }

    /// The body in `coding`, made if it is not made yet; the plain bytes and `Identity` for
    /// `Identity`, or where the compressed form would be no smaller.
    pub async fn get(&self, coding: Coding) -> (Bytes, Coding) {
        let (form, make): (_, fn(&[u8]) -> Bytes) = match coding {
            Coding::Brotli => (&self.brotli, brotli_to_keep),
            Coding::Gzip => (&self.gzip, gzip),
            Coding::Identity => return (self.plain.clone(), Coding::Identity),
        };
        let plain = self.plain.clone();
        let compressed = form.get_or_init(|| async move { tokio::task::spawn_blocking(move || make(&plain)).await.unwrap_or_default() }).await;
        if !compressed.is_empty() && compressed.len() < self.plain.len() {
            (compressed.clone(), coding)
        } else {
            (self.plain.clone(), Coding::Identity)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coding(accept: &[&str]) -> Coding {
        let mut headers = HeaderMap::new();
        for value in accept {
            headers.append(header::ACCEPT_ENCODING, HeaderValue::from_str(value).unwrap());
        }
        Coding::of(&headers)
    }

    #[test]
    fn brotli_to_whoever_takes_it() {
        // Browsers over HTTPS, and a browser over plain HTTP (Chrome names no `br` there).
        assert_eq!(coding(&["gzip, deflate, br, zstd"]), Coding::Brotli);
        assert_eq!(coding(&["gzip, deflate"]), Coding::Gzip);
        assert_eq!(coding(&[]), Coding::Identity);
        assert_eq!(coding(&[""]), Coding::Identity);
        assert_eq!(coding(&["identity"]), Coding::Identity);
        // The weights decide; brotli wins where they are equal.
        assert_eq!(coding(&["br;q=0.5, gzip"]), Coding::Gzip);
        assert_eq!(coding(&["gzip;q=0.8, br;q=0.8"]), Coding::Brotli);
        assert_eq!(coding(&["br;q=0, gzip"]), Coding::Gzip);
        assert_eq!(coding(&["br; q=0.000"]), Coding::Identity);
        assert_eq!(coding(&["gzip;q=0"]), Coding::Identity);
        assert_eq!(coding(&["BR;Q=1"]), Coding::Brotli);
        // `*` is every coding not named; several headers are one list.
        assert_eq!(coding(&["*"]), Coding::Brotli);
        assert_eq!(coding(&["*;q=0.1, br;q=0"]), Coding::Gzip);
        assert_eq!(coding(&["deflate", "br"]), Coding::Brotli);
        assert_eq!(coding(&["x-gzip"]), Coding::Gzip);
        // What it takes besides the best.
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate, br"));
        assert!(Coding::Brotli.taken_by(&headers) && Coding::Gzip.taken_by(&headers) && Coding::Identity.taken_by(&headers));
        headers.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("br"));
        assert!(!Coding::Gzip.taken_by(&headers));
    }

    #[test]
    fn what_is_compressed_unpacks_to_itself() {
        let html = "<!DOCTYPE html><p>Grundlagen der Informatik</p>".repeat(500);
        for quality in [FAST, LARGE, BEST] {
            let compressed = brotli(html.as_bytes(), quality);
            assert!(compressed.len() < html.len() / 20, "{quality}: {}", compressed.len());
            assert_eq!(unbrotli(&compressed).as_deref(), Some(html.as_bytes()), "{quality}");
        }
        assert_eq!(unbrotli(&brotli(b"", BEST)).as_deref(), Some(&b""[..]));
        assert_eq!(unbrotli(b"not brotli at all"), None);
        let zipped = gzip(html.as_bytes());
        assert_eq!(gunzip(&zipped).as_deref(), Some(html.as_bytes()));
        // A window as large as the body needs, never beyond what a browser reads.
        assert_eq!((window_bits(0), window_bits(1008), window_bits(1009), window_bits(261_817), window_bits(44_000_000)), (10, 10, 11, 18, 24));
    }

    #[tokio::test]
    async fn what_is_kept_is_compressed_once_as_it_is_asked_for() {
        let css = Bytes::from(".crown { mask-image: url(\"/assets/birch/summer-crown.svg\"); }\n".repeat(200));
        let kept = Kept::new(css.clone());
        let (brotli, coding) = kept.get(Coding::Brotli).await;
        assert_eq!((coding, unbrotli(&brotli)), (Coding::Brotli, Some(css.clone())));
        assert_eq!(kept.get(Coding::Brotli).await.0.as_ptr(), brotli.as_ptr(), "made once, then kept");
        let (zipped, coding) = kept.get(Coding::Gzip).await;
        assert_eq!((coding, gunzip(&zipped)), (Coding::Gzip, Some(css.clone())));
        assert_eq!(kept.get(Coding::Identity).await, (css.clone(), Coding::Identity));
        // Made ahead of time: sent as it came.
        let ahead = Kept::with_brotli(css.clone(), brotli.clone());
        assert_eq!(ahead.get(Coding::Brotli).await, (brotli, Coding::Brotli));
        // A body that compresses to nothing smaller goes out plain.
        let tiny = Kept::new(Bytes::from_static(b"{}"));
        assert_eq!(tiny.get(Coding::Brotli).await, (Bytes::from_static(b"{}"), Coding::Identity));
    }

    #[test]
    fn a_form_that_saves_nothing_is_not_sent() {
        let tiny = Bytes::from_static(b"ok");
        assert_eq!(smaller(tiny.clone(), Coding::Brotli, |body| brotli(body, BEST)), (tiny.clone(), Coding::Identity));
        let text = Bytes::from("Betula ".repeat(100));
        let (sent, coding) = smaller(text.clone(), Coding::Brotli, |body| brotli(body, FAST));
        assert_eq!(coding, Coding::Brotli);
        assert_eq!(unbrotli(&sent), Some(text.clone()));
        assert_eq!(smaller(text.clone(), Coding::Identity, |_| unreachable!()), (text, Coding::Identity));
    }
}
