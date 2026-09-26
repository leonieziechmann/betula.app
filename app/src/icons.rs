//! The icon set: Lucide 0.453 (ISC license). Generated from lucide-static; add a name here before
//! using it in a view.
//!
//! Served once, as a sprite (`/assets/icons.svg`, `sprite`), which every icon only points at
//! (`<use href="/assets/icons.svg?v=<build>#check"/>`, `ui::Icon`). Until 2026-09-26 the path data
//! was written into the page for every icon: 155 times (30 kB of 129) on a page of the catalog, on
//! which 20 different icons occur, and rendered anew on every such page (owner: „das kann ja auch
//! alles statisch geserved und nur verlinkt werden"). What the stylesheet sets on `.icon` (size,
//! stroke, `fill` of a pressed bookmark) is inherited into the sprite's symbols as before.

/// Where the server serves the sprite; pages link it with their build (`crate::asset`).
pub const SPRITE: &str = "/assets/icons.svg";

/// Every icon: its name and its inner SVG markup (24×24, stroked with `currentColor`).
pub const ICONS: &[(&str, &str)] = &[
    ("arrow-down-up", r#"<path d="m3 16 4 4 4-4"/> <path d="M7 20V4"/> <path d="m21 8-4-4-4 4"/> <path d="M17 4v16"/>"#),
    ("arrow-left", r#"<path d="m12 19-7-7 7-7"/> <path d="M19 12H5"/>"#),
    ("arrow-up-right", r#"<path d="M7 7h10v10"/> <path d="M7 17 17 7"/>"#),
    ("award", r#"<path d="m15.477 12.89 1.515 8.526a.5.5 0 0 1-.81.47l-3.58-2.687a1 1 0 0 0-1.197 0l-3.586 2.686a.5.5 0 0 1-.81-.469l1.514-8.526"/> <circle cx="12" cy="8" r="6"/>"#),
    ("bookmark", r#"<path d="m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z"/>"#),
    ("building-2", r#"<path d="M6 22V4a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v18Z"/> <path d="M6 12H4a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h2"/> <path d="M18 9h2a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-2"/> <path d="M10 6h4"/> <path d="M10 10h4"/> <path d="M10 14h4"/> <path d="M10 18h4"/>"#),
    ("calendar-check-2", r#"<path d="M8 2v4"/> <path d="M16 2v4"/> <path d="M21 14V6a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h8"/> <path d="M3 10h18"/> <path d="m16 20 2 2 4-4"/>"#),
    ("calendar-days", r#"<path d="M8 2v4"/> <path d="M16 2v4"/> <rect width="18" height="18" x="3" y="4" rx="2"/> <path d="M3 10h18"/> <path d="M8 14h.01"/> <path d="M12 14h.01"/> <path d="M16 14h.01"/> <path d="M8 18h.01"/> <path d="M12 18h.01"/> <path d="M16 18h.01"/>"#),
    ("calendar-plus", r#"<path d="M8 2v4"/> <path d="M16 2v4"/> <path d="M21 13V6a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h8"/> <path d="M3 10h18"/> <path d="M16 19h6"/> <path d="M19 16v6"/>"#),
    ("calendar-range", r#"<rect width="18" height="18" x="3" y="4" rx="2"/> <path d="M16 2v4"/> <path d="M3 10h18"/> <path d="M8 2v4"/> <path d="M17 14h-6"/> <path d="M13 18H7"/> <path d="M7 14h.01"/> <path d="M17 18h.01"/>"#),
    ("check", r#"<path d="M20 6 9 17l-5-5"/>"#),
    ("chevron-down", r#"<path d="m6 9 6 6 6-6"/>"#),
    ("chevron-left", r#"<path d="m15 18-6-6 6-6"/>"#),
    ("chevron-right", r#"<path d="m9 18 6-6-6-6"/>"#),
    ("chevrons-up-down", r#"<path d="m7 15 5 5 5-5"/> <path d="m7 9 5-5 5 5"/>"#),
    ("circle-check-big", r#"<path d="M21.801 10A10 10 0 1 1 17 3.335"/> <path d="m9 11 3 3L22 4"/>"#),
    ("clock-3", r#"<circle cx="12" cy="12" r="10"/> <polyline points="12 6 12 12 16.5 12"/>"#),
    ("copy", r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/> <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>"#),
    ("database", r#"<ellipse cx="12" cy="5" rx="9" ry="3"/> <path d="M3 5V19A9 3 0 0 0 21 19V5"/> <path d="M3 12A9 3 0 0 0 21 12"/>"#),
    ("download", r#"<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/> <polyline points="7 10 12 15 17 10"/> <line x1="12" x2="12" y1="15" y2="3"/>"#),
    ("eye", r#"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/> <circle cx="12" cy="12" r="3"/>"#),
    ("eye-off", r#"<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/> <path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/> <path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/> <path d="m2 2 20 20"/>"#),
    ("file-check-2", r#"<path d="M4 22h14a2 2 0 0 0 2-2V7l-5-5H6a2 2 0 0 0-2 2v4"/> <path d="M14 2v4a2 2 0 0 0 2 2h4"/> <path d="m3 15 2 2 4-4"/>"#),
    ("graduation-cap", r#"<path d="M21.42 10.922a1 1 0 0 0-.019-1.838L12.83 5.18a2 2 0 0 0-1.66 0L2.6 9.08a1 1 0 0 0 0 1.832l8.57 3.908a2 2 0 0 0 1.66 0z"/> <path d="M22 10v6"/> <path d="M6 12.5V16a6 3 0 0 0 12 0v-3.5"/>"#),
    ("house", r#"<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/> <path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>"#),
    ("info", r#"<circle cx="12" cy="12" r="10"/> <path d="M12 16v-4"/> <path d="M12 8h.01"/>"#),
    ("languages", r#"<path d="m5 8 6 6"/> <path d="m4 14 6-6 2-3"/> <path d="M2 5h12"/> <path d="M7 2h1"/> <path d="m22 22-5-10-5 10"/> <path d="M14 18h6"/>"#),
    ("leaf", r#"<path d="M11 20A7 7 0 0 1 9.8 6.1C15.5 5 17 4.48 19 2c1 2 2 4.18 2 8 0 5.5-4.78 10-10 10Z"/> <path d="M2 21c0-3 1.85-5.36 5.08-6C9.5 14.52 12 13 13 12"/>"#),
    ("layout-list", r#"<rect width="7" height="7" x="3" y="3" rx="1"/> <rect width="7" height="7" x="3" y="14" rx="1"/> <path d="M14 4h7"/> <path d="M14 9h7"/> <path d="M14 15h7"/> <path d="M14 20h7"/>"#),
    ("map-pin", r#"<path d="M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0"/> <circle cx="12" cy="10" r="3"/>"#),
    ("maximize-2", r#"<polyline points="15 3 21 3 21 9"/> <polyline points="9 21 3 21 3 15"/> <line x1="21" x2="14" y1="3" y2="10"/> <line x1="3" x2="10" y1="21" y2="14"/>"#),
    ("minus", r#"<path d="M5 12h14"/>"#),
    ("moon", r#"<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/>"#),
    ("pause", r#"<rect x="14" y="4" width="4" height="16" rx="1"/> <rect x="6" y="4" width="4" height="16" rx="1"/>"#),
    ("play", r#"<polygon points="6 3 20 12 6 21 6 3"/>"#),
    ("plus", r#"<path d="M5 12h14"/> <path d="M12 5v14"/>"#),
    ("repeat", r#"<path d="m17 2 4 4-4 4"/> <path d="M3 11v-1a4 4 0 0 1 4-4h14"/> <path d="m7 22-4-4 4-4"/> <path d="M21 13v1a4 4 0 0 1-4 4H3"/>"#),
    ("rotate-ccw", r#"<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/> <path d="M3 3v5h5"/>"#),
    ("search", r#"<circle cx="11" cy="11" r="8"/> <path d="m21 21-4.3-4.3"/>"#),
    ("share-2", r#"<circle cx="18" cy="5" r="3"/> <circle cx="6" cy="12" r="3"/> <circle cx="18" cy="19" r="3"/> <line x1="8.59" x2="15.42" y1="13.51" y2="17.49"/> <line x1="15.41" x2="8.59" y1="6.51" y2="10.49"/>"#),
    ("shield-check", r#"<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/> <path d="m9 12 2 2 4-4"/>"#),
    ("shuffle", r#"<path d="M2 18h1.4c1.3 0 2.5-.6 3.3-1.7l6.1-8.6c.7-1.1 2-1.7 3.3-1.7H22"/> <path d="m18 2 4 4-4 4"/> <path d="M2 6h1.9c1.5 0 2.9.9 3.6 2.2"/> <path d="M22 18h-5.9c-1.3 0-2.6-.7-3.3-1.8l-.5-.8"/> <path d="m18 14 4 4-4 4"/>"#),
    ("sliders-horizontal", r#"<line x1="21" x2="14" y1="4" y2="4"/> <line x1="10" x2="3" y1="4" y2="4"/> <line x1="21" x2="12" y1="12" y2="12"/> <line x1="8" x2="3" y1="12" y2="12"/> <line x1="21" x2="16" y1="20" y2="20"/> <line x1="12" x2="3" y1="20" y2="20"/> <line x1="14" x2="14" y1="2" y2="6"/> <line x1="8" x2="8" y1="10" y2="14"/> <line x1="16" x2="16" y1="18" y2="22"/>"#),
    ("snowflake", r#"<line x1="2" x2="22" y1="12" y2="12"/> <line x1="12" x2="12" y1="2" y2="22"/> <path d="m20 16-4-4 4-4"/> <path d="m4 8 4 4-4 4"/> <path d="m16 4-4 4-4-4"/> <path d="m8 20 4-4 4 4"/>"#),
    ("star", r#"<polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>"#),
    ("sun", r#"<circle cx="12" cy="12" r="4"/> <path d="M12 2v2"/> <path d="M12 20v2"/> <path d="m4.93 4.93 1.41 1.41"/> <path d="m17.66 17.66 1.41 1.41"/> <path d="M2 12h2"/> <path d="M20 12h2"/> <path d="m6.34 17.66-1.41 1.41"/> <path d="m19.07 4.93-1.41 1.41"/>"#),
    ("trash-2", r#"<path d="M3 6h18"/> <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/> <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/> <line x1="10" x2="10" y1="11" y2="17"/> <line x1="14" x2="14" y1="11" y2="17"/>"#),
    ("triangle-alert", r#"<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/> <path d="M12 9v4"/> <path d="M12 17h.01"/>"#),
    ("user-round", r#"<circle cx="12" cy="8" r="5"/> <path d="M20 21a8 8 0 0 0-16 0"/>"#),
    ("users-round", r#"<path d="M18 21a8 8 0 0 0-16 0"/> <circle cx="10" cy="8" r="5"/> <path d="M22 20c0-3.37-2-6.5-4-8a5 5 0 0 0-.45-8.3"/>"#),
    ("x", r#"<path d="M18 6 6 18"/> <path d="m6 6 12 12"/>"#),
];

/// The inner SVG markup of an icon.
pub fn markup(name: &str) -> Option<&'static str> {
    ICONS.iter().find(|(known, _)| *known == name).map(|(_, markup)| *markup)
}

/// The sprite: every icon as a `<symbol>` named like the icon.
pub fn sprite() -> String {
    let mut svg = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\">");
    for (name, markup) in ICONS {
        svg.push_str(&format!("<symbol id=\"{name}\" viewBox=\"0 0 24 24\">{markup}</symbol>"));
    }
    svg.push_str("</svg>
");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_in_the_sprite_once() {
        let sprite = sprite();
        for (name, _) in ICONS {
            assert_eq!(sprite.matches(&format!("<symbol id=\"{name}\"")).count(), 1, "{name}");
            assert!(markup(name).is_some());
        }
        assert!(markup("no-such-icon").is_none());
    }
}
