//! The launch screens of the installed app on iPhones and iPads. Android draws the splash screen
//! of an installed web app itself, from the manifest (the background colour and the maskable
//! icon). iOS does not: it shows the picture the page names for exactly the screen the app opens
//! on (`apple-touch-startup-image`, one per size, orientation and colour scheme), and without one
//! a blank screen until the page is drawn. It takes the pictures when the app is added to the home
//! screen, from the page as it is then. The head script names those of the screen it runs on
//! (`HEAD_SCRIPT`: `<PATH><width>x<height>[-dark].png`, from `screen` and `devicePixelRatio`), and
//! the server draws the ones of the screens below (`folia/crates/server/src/launch.rs`): the mark in the middle
//! of the page's background, the wordmark at the bottom, light and dark. A screen that is not
//! listed gets no picture and starts blank as before; a new iPhone is one more line here.

/// A screen: its size in CSS pixels, held upright, and how many device pixels one of them has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Screen {
    pub width: u16,
    pub height: u16,
    pub ratio: u8,
}

const fn screen(width: u16, height: u16, ratio: u8) -> Screen {
    Screen { width, height, ratio }
}

/// The iPhones, upright only: their home screen does not turn, so the app opens upright.
pub const PHONES: &[Screen] = &[
    screen(375, 667, 2),  // SE (2nd and 3rd generation), 8
    screen(414, 736, 3),  // 8 Plus
    screen(375, 812, 3),  // X, XS, 11 Pro, 12 mini, 13 mini
    screen(414, 896, 2),  // XR, 11
    screen(414, 896, 3),  // XS Max, 11 Pro Max
    screen(390, 844, 3),  // 12, 12 Pro, 13, 13 Pro, 14, 16e
    screen(428, 926, 3),  // 12 Pro Max, 13 Pro Max, 14 Plus
    screen(393, 852, 3),  // 14 Pro, 15, 15 Pro, 16
    screen(430, 932, 3),  // 14 Pro Max, 15 Plus, 15 Pro Max, 16 Plus
    screen(402, 874, 3),  // 16 Pro, 17, 17 Pro
    screen(420, 912, 3),  // Air
    screen(440, 956, 3),  // 16 Pro Max, 17 Pro Max
];

/// The iPads, which open the app either way up.
pub const TABLETS: &[Screen] = &[
    screen(744, 1133, 2),  // mini (6th generation, A17 Pro)
    screen(768, 1024, 2),  // 9.7" (5th and 6th generation), mini 4 and 5, Air 2
    screen(810, 1080, 2),  // 10.2" (7th to 9th generation)
    screen(820, 1180, 2),  // 10th generation, A16, Air 4 and 5, Air 11" (M2, M3)
    screen(834, 1112, 2),  // Air 3, Pro 10.5"
    screen(834, 1194, 2),  // Pro 11" (1st to 4th generation)
    screen(834, 1210, 2),  // Pro 11" (M4, M5)
    screen(1024, 1366, 2), // Pro 12.9", Air 13" (M2, M3)
    screen(1032, 1376, 2), // Pro 13" (M4, M5)
];

/// Where the pictures are: `<PATH><width>x<height>[-dark].png`, in device pixels.
pub const PATH: &str = "/assets/launch/";

/// One launch screen: a screen in one orientation and one colour scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Picture {
    pub screen: Screen,
    pub landscape: bool,
    pub dark: bool,
}

impl Picture {
    /// Every picture a page names: the phones upright, the tablets both ways, each light and dark.
    pub fn all() -> impl Iterator<Item = Picture> {
        let upright = PHONES.iter().map(|screen| (*screen, false));
        let tablets = TABLETS.iter().flat_map(|screen| [(*screen, false), (*screen, true)]);
        upright.chain(tablets).flat_map(|(screen, landscape)| [false, true].map(|dark| Picture { screen, landscape, dark }))
    }

    /// Width and height in CSS pixels, as the app opens.
    pub fn points(&self) -> (u32, u32) {
        let (width, height) = (u32::from(self.screen.width), u32::from(self.screen.height));
        if self.landscape {
            (height, width)
        } else {
            (width, height)
        }
    }

    /// Width and height in device pixels: the size of the picture.
    pub fn pixels(&self) -> (u32, u32) {
        let (width, height) = self.points();
        let ratio = u32::from(self.screen.ratio);
        (width * ratio, height * ratio)
    }

    pub fn file(&self) -> String {
        let (width, height) = self.pixels();
        format!("{width}x{height}{}.png", if self.dark { "-dark" } else { "" })
    }

    pub fn href(&self) -> String {
        format!("{PATH}{}", self.file())
    }

    /// The picture a file name under `PATH` names, if a page names it.
    pub fn from_file(file: &str) -> Option<Picture> {
        Picture::all().find(|picture| picture.file() == file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_picture_has_its_own_name_and_says_what_it_is() {
        let all: Vec<Picture> = Picture::all().collect();
        assert_eq!(all.len(), 2 * (PHONES.len() + 2 * TABLETS.len()));
        let names: std::collections::HashSet<String> = all.iter().map(Picture::file).collect();
        assert_eq!(names.len(), all.len(), "two screens would share a picture");
        let phone = Picture { screen: screen(393, 852, 3), landscape: false, dark: true };
        assert_eq!(phone.href(), "/assets/launch/1179x2556-dark.png");
        let tablet = Picture { screen: screen(1032, 1376, 2), landscape: true, dark: false };
        assert_eq!((tablet.points(), tablet.pixels(), tablet.file().as_str()), ((1376, 1032), (2752, 2064), "2752x2064.png"));
        assert_eq!(Picture::from_file("2752x2064.png"), Some(tablet));
        // The head script tells a phone from a tablet by the shorter side in device pixels, and
        // names a turned picture for a tablet only.
        assert!(PHONES.iter().all(|s| u32::from(s.width.min(s.height)) * u32::from(s.ratio) < 1400));
        assert!(TABLETS.iter().all(|s| u32::from(s.width.min(s.height)) * u32::from(s.ratio) >= 1400));
        assert!(crate::document::HEAD_SCRIPT.contains("a<1400?") && crate::document::HEAD_SCRIPT.contains(&format!("l.href='{PATH}'")));
        for unknown in ["1179x2556.jpg", "100x100.png", "1179x2556-dim.png", "../1179x2556.png", ""] {
            assert_eq!(Picture::from_file(unknown), None, "{unknown}");
        }
    }
}
