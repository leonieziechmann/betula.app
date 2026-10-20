//! The photo over the start page (concept A; owner, 2026-10-20: „Option A mit den
//! Jahreszeitenbildern ist ja mal der hammer"): birches in spring, summer, autumn and winter, and
//! fog while the exams run (folia/design/startseite/konzepte.html, „Mehr Bilder"). The five come
//! with the build (`/assets/season/<code>.webp`, about 140–350 kB each) and a browser fetches one:
//! the stylesheet picks the season's by the date the head script names (`data-season`), so the
//! server's page is the same for everybody (R9), and the app turns it into the fog while the exams
//! run (`Photo::Fog`, `class="fog"`). The ground names who took the one shown, as Unsplash's licence
//! is glad of („Foto: John Price, Unsplash"); the stylesheet shows the line of the photo shown.

use folia_calendar::day::Day;

/// One of the photos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Photo {
    Spring,
    Summer,
    Autumn,
    Winter,
    /// The time of the exams, whatever the season.
    Fog,
}

impl Photo {
    pub const ALL: [Photo; 5] = [Photo::Spring, Photo::Summer, Photo::Autumn, Photo::Winter, Photo::Fog];

    /// The season's photo of a month (1–12), as the head script names the season: March to May
    /// spring, June to August summer, September to November autumn, the rest winter. (The
    /// stylesheet picks it by the head script's `data-season`; the tests hold the two together.)
    #[cfg(test)]
    pub fn of_month(month: u32) -> Photo {
        match month {
            3..=5 => Photo::Spring,
            6..=8 => Photo::Summer,
            9..=11 => Photo::Autumn,
            _ => Photo::Winter,
        }
    }

    /// Its name in the stylesheet and in its address (`/assets/season/<code>.webp`).
    pub fn code(self) -> &'static str {
        match self {
            Photo::Spring => "spring",
            Photo::Summer => "summer",
            Photo::Autumn => "autumn",
            Photo::Winter => "winter",
            Photo::Fog => "fog",
        }
    }

    /// Who took it, and their page at Unsplash.
    pub fn by(self) -> (&'static str, &'static str) {
        match self {
            Photo::Spring => ("Ann", "https://unsplash.com/@potato_potato_potato"),
            Photo::Summer => ("Elisa", "https://unsplash.com/@elisaih"),
            Photo::Autumn => ("John Price", "https://unsplash.com/@johnprice"),
            Photo::Winter => ("Marek Okon", "https://unsplash.com/@marekokon"),
            Photo::Fog => ("Emma Waleij", "https://unsplash.com/@waleij"),
        }
    }
}

/// How light it is over the photo: the shade is a night's blue at night and warm around sunrise
/// and sunset (concept A drew the sky by the hour; the photo keeps its day and only its shade
/// follows the hour).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Light {
    Day,
    Dusk,
    Night,
}

impl Light {
    /// Its class on the picture; none by day.
    pub fn class(self) -> &'static str {
        match self {
            Light::Day => "",
            Light::Dusk => "dusk",
            Light::Night => "night",
        }
    }
}

/// The light at `minute` (since local midnight) of `day` in Cottbus: night from 40 minutes before
/// sunrise back and 50 minutes after sunset on, dusk around them.
pub fn light(day: Day, minute: u16) -> Light {
    let (rise, set) = sun(day);
    let minute = f64::from(minute);
    if minute < rise - 40.0 || minute > set + 50.0 {
        Light::Night
    } else if minute < rise + 50.0 || minute > set - 60.0 {
        Light::Dusk
    } else {
        Light::Day
    }
}

/// Sunrise and sunset in Cottbus (51.76 °N, 14.33 °E) on `day`, in minutes after local midnight
/// (Berlin's time, summer time included): the sun's declination and the equation of time of the
/// day of the year (NOAA's approximation), good to a few minutes, which is all a shade needs.
pub fn sun(day: Day) -> (f64, f64) {
    use std::f64::consts::PI;
    let (year, _, _) = day.ymd();
    let first = Day::from_ymd(year, 1, 1).unwrap_or(day);
    let g = 2.0 * PI / 365.0 * f64::from(day.0 - first.0);
    let declination = 0.006918 - 0.399912 * g.cos() + 0.070257 * g.sin() - 0.006758 * (2.0 * g).cos() + 0.000907 * (2.0 * g).sin() - 0.002697 * (3.0 * g).cos() + 0.00148 * (3.0 * g).sin();
    let equation = 229.18 * (0.000075 + 0.001868 * g.cos() - 0.032077 * g.sin() - 0.014615 * (2.0 * g).cos() - 0.040849 * (2.0 * g).sin());
    let latitude = 51.76_f64.to_radians();
    let hour_angle = ((90.833_f64.to_radians().cos() / (latitude.cos() * declination.cos())) - latitude.tan() * declination.tan()).clamp(-1.0, 1.0).acos().to_degrees();
    let noon = 720.0 - 4.0 * 14.33 - equation;
    let offset = if folia_calendar::day::berlin_offset(day, 720) == "+02:00" { 120.0 } else { 60.0 };
    (noon - 4.0 * hour_angle + offset, noon + 4.0 * hour_angle + offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cottbus's sunrise and sunset on the prototype's days (timeanddate.com: 20.10.2026 07:36 and
    /// 18:00, 21.12.2026 08:09 and 15:55), within a few minutes; the light of the day by the hour.
    #[test]
    fn the_sun_rises_and_sets_over_cottbus() {
        let day = |iso: &str| Day::parse(iso).unwrap();
        for (iso, rise, set) in [("2026-10-20", 7 * 60 + 36, 18 * 60), ("2026-12-21", 8 * 60 + 9, 15 * 60 + 55)] {
            let (r, s) = sun(day(iso));
            assert!((r - f64::from(rise)).abs() < 8.0 && (s - f64::from(set)).abs() < 8.0, "{iso}: {r} {s}");
        }
        let october = day("2026-10-20");
        assert_eq!([light(october, 6 * 60), light(october, 7 * 60 + 40), light(october, 12 * 60), light(october, 18 * 60 + 20), light(october, 22 * 60)], [Light::Night, Light::Dusk, Light::Day, Light::Dusk, Light::Night]);
    }

    /// The months of each season as the head script counts them (`folia_shell::document`), and
    /// every photo of the build, by its address.
    #[test]
    fn a_month_has_its_season_and_every_photo_is_there() {
        let seasons: Vec<&str> = (1..=12).map(|month| Photo::of_month(month).code()).collect();
        assert_eq!(seasons, ["winter", "winter", "spring", "spring", "spring", "summer", "summer", "summer", "autumn", "autumn", "autumn", "winter"]);
        let script = folia_shell::document::HEAD_SCRIPT;
        assert!(script.contains("d.dataset.season=n<2||n>10?'winter':n<5?'spring':n<8?'summer':'autumn'"), "the head script names the season by the month as `of_month` does");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/season");
        for photo in Photo::ALL {
            let path = dir.join(format!("{}.webp", photo.code()));
            let bytes = std::fs::read(&path).unwrap();
            assert!(bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP".as_slice()) && bytes.len() < 400_000, "{path:?}");
            assert!(photo.by().1.starts_with("https://unsplash.com/@"));
        }
    }
}
