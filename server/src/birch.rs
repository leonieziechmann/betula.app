//! The birch around the app as files (docs/frontend.md „The birch"): the masks
//! `design/birch/birch.mjs` draws into `app/assets/birch`, embedded once. The stylesheet gets them
//! under `/assets/birch/` (`api::birch`) and colours them by its tokens; the link-preview cards
//! (`cards`) draw the crown into their top edge in the season's tone, with a head of their own
//! (`<season>-card-head.svg`, not served: its clearing fits the card's wordmark, not the page's
//! title).
//!
//! The wood behind the start page (`<season>-wood-back.svg`, `-front.svg`) is drawn by
//! `design/forest/forest.mjs`.
//!
//! Every file is a mask: shape in black and nothing else (what hangs behind at half strength).

/// The season the crown is drawn in, as the site follows the year (the script in `<head>`,
/// `app::shell`): March–May spring, June–August summer, September–November autumn,
/// December–February winter.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub const ALL: [Season; 4] = [Season::Spring, Season::Summer, Season::Autumn, Season::Winter];

    /// The season of a month (1–12).
    pub fn of_month(month: u32) -> Season {
        match month {
            3..=5 => Season::Spring,
            6..=8 => Season::Summer,
            9..=11 => Season::Autumn,
            _ => Season::Winter,
        }
    }

    /// The season at a moment (seconds since 1970, UTC). The site's script goes by the visitor's
    /// clock, so in the night a month ends the two may differ for an hour or two.
    pub fn at(unix_seconds: u64) -> Season {
        let day = catalog::timetable::day::Day(i32::try_from(unix_seconds / 86_400).unwrap_or(0));
        Season::of_month(day.ymd().1)
    }

    /// The season now, by the server's clock.
    pub fn now() -> Season {
        Season::at(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or(0))
    }

    /// As the files and `data-season` name it.
    pub fn name(self) -> &'static str {
        match self {
            Season::Spring => "spring",
            Season::Summer => "summer",
            Season::Autumn => "autumn",
            Season::Winter => "winter",
        }
    }

    /// The crown's tone in the light theme (`--crown` in app/assets/app.css, as sRGB): a link
    /// preview is light, whatever the reader's theme.
    pub fn tone(self) -> [u8; 3] {
        match self {
            // oklch(.765 .094 130)
            Season::Spring => [0x9f, 0xbf, 0x7f],
            // oklch(.716 .073 148)
            Season::Summer => [0x84, 0xb0, 0x8a],
            // oklch(.765 .105 80)
            Season::Autumn => [0xd6, 0xab, 0x61],
            // oklch(.575 .029 40)
            Season::Winter => [0x89, 0x73, 0x6c],
        }
    }

    /// Spring's catkins in their own tone (`--crown-ck`: oklch(.79 .113 100)); in every other
    /// season the catkins are part of the crown.
    pub fn catkins_tone(self) -> Option<[u8; 3]> {
        (self == Season::Spring).then_some([0xcb, 0xbc, 0x62])
    }
}

/// The crown of a season as a link-preview card hangs it: a head of its own (300 × 64, the
/// clearing fitted over the card's wordmark, the mark in front of the crown before it), the site's
/// tile after it (1200 × 64, repeated to the right edge), and in spring the catkins of both, which
/// are coloured apart.
pub struct Crown {
    pub head: &'static str,
    /// The head's width in its own units (its height is 64, as the tile's).
    pub head_width: f32,
    pub tile: &'static str,
    pub catkins: Option<(&'static str, &'static str)>,
}

pub fn card_crown(season: Season) -> Crown {
    let tile = |name: &str| file(&format!("{}-{name}.svg", season.name())).unwrap_or_default();
    let head = match season {
        Season::Spring => include_str!("../../app/assets/birch/spring-card-head.svg"),
        Season::Summer => include_str!("../../app/assets/birch/summer-card-head.svg"),
        Season::Autumn => include_str!("../../app/assets/birch/autumn-card-head.svg"),
        Season::Winter => include_str!("../../app/assets/birch/winter-card-head.svg"),
    };
    Crown {
        head,
        head_width: 300.0,
        tile: tile("crown"),
        catkins: (season == Season::Spring).then(|| (include_str!("../../app/assets/birch/spring-card-head-ck.svg"), tile("crown-ck"))),
    }
}

/// A file of the birch by its name under `/assets/birch/`.
pub fn file(name: &str) -> Option<&'static str> {
    Some(match name {
        "spring-crown.svg" => include_str!("../../app/assets/birch/spring-crown.svg"),
        "spring-crown-ck.svg" => include_str!("../../app/assets/birch/spring-crown-ck.svg"),
        "spring-crown-head.svg" => include_str!("../../app/assets/birch/spring-crown-head.svg"),
        "spring-crown-head-ck.svg" => include_str!("../../app/assets/birch/spring-crown-head-ck.svg"),
        "summer-crown.svg" => include_str!("../../app/assets/birch/summer-crown.svg"),
        "summer-crown-head.svg" => include_str!("../../app/assets/birch/summer-crown-head.svg"),
        "autumn-crown.svg" => include_str!("../../app/assets/birch/autumn-crown.svg"),
        "autumn-crown-head.svg" => include_str!("../../app/assets/birch/autumn-crown-head.svg"),
        "winter-crown.svg" => include_str!("../../app/assets/birch/winter-crown.svg"),
        "winter-crown-head.svg" => include_str!("../../app/assets/birch/winter-crown-head.svg"),
        "roots.svg" => include_str!("../../app/assets/birch/roots.svg"),
        "litter.svg" => include_str!("../../app/assets/birch/litter.svg"),
        "spring-wood-back.svg" => include_str!("../../app/assets/birch/spring-wood-back.svg"),
        "spring-wood-front.svg" => include_str!("../../app/assets/birch/spring-wood-front.svg"),
        "summer-wood-back.svg" => include_str!("../../app/assets/birch/summer-wood-back.svg"),
        "summer-wood-front.svg" => include_str!("../../app/assets/birch/summer-wood-front.svg"),
        "autumn-wood-back.svg" => include_str!("../../app/assets/birch/autumn-wood-back.svg"),
        "autumn-wood-front.svg" => include_str!("../../app/assets/birch/autumn-wood-front.svg"),
        "winter-wood-back.svg" => include_str!("../../app/assets/birch/winter-wood-back.svg"),
        "winter-wood-front.svg" => include_str!("../../app/assets/birch/winter-wood-front.svg"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seasons_follow_the_months_as_the_site_does() {
        let seasons: Vec<&str> = (1..=12).map(|month| Season::of_month(month).name()).collect();
        assert_eq!(seasons, ["winter", "winter", "spring", "spring", "spring", "summer", "summer", "summer", "autumn", "autumn", "autumn", "winter"]);
        // 2026-09-26 12:00 UTC, 2026-12-01 00:00 UTC, 2027-03-01 00:00 UTC.
        assert_eq!(Season::at(1_790_424_000), Season::Autumn);
        assert_eq!(Season::at(1_796_083_200), Season::Winter);
        assert_eq!(Season::at(1_803_859_200), Season::Spring);
    }

    #[test]
    fn every_season_has_its_crown() {
        for season in Season::ALL {
            let crown = card_crown(season);
            assert!(crown.head.starts_with("<svg") && crown.tile.starts_with("<svg"), "{season:?}");
            assert_eq!(crown.catkins.is_some(), season.catkins_tone().is_some());
        }
    }
}
