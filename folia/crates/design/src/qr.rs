//! A QR code as inline SVG (`qrcodegen`): the link for the next device of a synced profile
//! (folia_profile's `sync.rs`), the link of a calendar shared (the planner's „Teilen"). Dark
//! modules on a light ground with four modules of quiet zone, whatever the theme: a camera reads
//! inverted codes badly, so in the dark the code stands on a light tile of its own (app.css,
//! `.pf-qr`).

use qrcodegen::{QrCode, QrCodeEcc};

/// The four modules of quiet zone around the code that the standard asks for.
const BORDER: i32 = 4;

/// The SVG path of the dark modules of `text`'s QR code (medium error correction), and the width of
/// its view box in modules, quiet zone included; `None` for a text too long for any QR code.
pub fn path(text: &str) -> Option<(String, i32)> {
    let code = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let size = code.size();
    let mut path = String::new();
    for y in 0..size {
        // A run of dark modules in a row is one rectangle: a quarter of the path of one per module.
        let mut x = 0;
        while x < size {
            if !code.get_module(x, y) {
                x += 1;
                continue;
            }
            let start = x;
            while x < size && code.get_module(x, y) {
                x += 1;
            }
            path.push_str(&format!("M{} {}h{}v1h-{}z", start + BORDER, y + BORDER, x - start, x - start));
        }
    }
    Some((path, size + 2 * BORDER))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_becomes_a_code_with_its_quiet_zone() {
        let link = format!("https://betula.app{}#{}", folia_routes::url::CONNECT, "A".repeat(64));
        let (drawn, width) = path(&link).unwrap();
        // 88 characters in bytes at medium correction: version 6, 41 modules, and the zone.
        assert_eq!(width, 41 + 8);
        assert!(drawn.starts_with("M4 4h7v1h-7z"), "the finder pattern's first row: {}", &drawn[..40]);
        assert!(drawn.split('M').skip(1).all(|run| !run.is_empty()));
        assert!(path(&"x".repeat(4000)).is_none(), "longer than any QR code");
    }
}
