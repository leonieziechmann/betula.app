//! A QR code as inline SVG (`qrcodegen`): the link for the next device of a synced profile
//! (folia_profile's `sync.rs`), the link of a calendar shared (the planner's „Teilen"). Dark
//! modules on a light ground with four modules of quiet zone, whatever the theme: a camera reads
//! inverted codes badly, so in the dark the code stands on a light tile of its own (app.css,
//! `.pf-qr`).
//!
//! And a QR code read from a camera's frame (`read`, `rqrr`): Betula's scanner, where the browser
//! has no BarcodeDetector of its own (Safari, Firefox; folia_profile's `scan.rs`).

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

/// The text of a QR code in a frame of `width` × `height` pixels, given as their brightness
/// (0 black … 255 white, row by row); `None` where none is read. The first code that reads wins.
pub fn read(luma: &[u8], width: usize, height: usize) -> Option<String> {
    if width == 0 || height == 0 || luma.len() < width * height {
        return None;
    }
    let mut frame = rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| luma.get(y * width + x).copied().unwrap_or(255));
    frame.detect_grids().into_iter().find_map(|grid| grid.decode().ok().map(|(_, text)| text))
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

    /// A code drawn as a camera would see it: `scale` pixels a module, the quiet zone, a grey
    /// light ground and a dark grey ink.
    fn frame(text: &str, scale: usize) -> (Vec<u8>, usize) {
        let code = QrCode::encode_text(text, QrCodeEcc::Medium).unwrap();
        let modules = usize::try_from(code.size()).unwrap() + 2 * BORDER as usize;
        let side = modules * scale;
        let mut luma = vec![215u8; side * side];
        for y in 0..side {
            for x in 0..side {
                let (mx, my) = (i32::try_from(x / scale).unwrap() - BORDER, i32::try_from(y / scale).unwrap() - BORDER);
                if code.get_module(mx, my) {
                    luma[y * side + x] = 40;
                }
            }
        }
        (luma, side)
    }

    #[test]
    fn a_code_drawn_is_read_again() {
        let link = format!("https://canary.betula.app{}#request={}", folia_routes::url::CONNECT, "Ab-_".repeat(27));
        let (luma, side) = frame(&link, 4);
        assert_eq!(read(&luma, side, side).as_deref(), Some(link.as_str()));
        // In a larger frame, off its middle.
        let wide = side + 120;
        let mut bigger = vec![190u8; wide * (side + 40)];
        for y in 0..side {
            bigger[(y + 30) * wide + 100..(y + 30) * wide + 100 + side].copy_from_slice(&luma[y * side..(y + 1) * side]);
        }
        assert_eq!(read(&bigger, wide, side + 40).as_deref(), Some(link.as_str()));
        // Nothing to read, and a frame that is not as large as it says.
        assert_eq!(read(&vec![128u8; 64 * 48], 64, 48), None);
        assert_eq!(read(&luma, side + 1, side), None);
        assert_eq!(read(&[], 0, 0), None);
    }
}
