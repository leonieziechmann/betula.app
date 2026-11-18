//! What Betula announces by itself, as a feed: `GET /api/announcements`.
//!
//! The general dates of a semester (`folia_pages::announce`: the lectures begin, they pause, the
//! exam periods begin, the next semester's dates are there), each with the words of a notification
//! in every language of the site. Xylem, the service of the notifications, reads this address
//! every few minutes and tells what it names to the browsers that listen
//! (docs/xylem/protocol.md section 7); the shape below is the one its `announce` takes.
//!
//! The same for everybody (R9), and nothing in it is anybody's: it is what the catalog says, so
//! it is open where the site is in closed testing as well (`access`). It depends on the snapshot
//! and on the day: made once for both (`Snapshot::announcements`), with an ETag of its content.

use std::time::{SystemTime, UNIX_EPOCH};

use folia_calendar::day::{berlin_offset, Day};
use folia_locale::Locale;
use folia_pages::announce::{Notice, What};
use serde_json::{json, Map, Value};

use crate::texts::texts;

/// The feed's address. Not under a language: it carries every language.
pub const PATH: &str = "/api/announcements";

/// The day it is in Berlin at a time (seconds since 1970): the day the notices are counted from.
pub fn berlin_day(unix_seconds: u64) -> Day {
    let utc = Day(i32::try_from(unix_seconds / 86_400).unwrap_or(0));
    let minutes = u16::try_from(unix_seconds % 86_400 / 60).unwrap_or(0);
    // The offset of the UTC day and minute: off by the hour or two around the two nights a year
    // the clocks change, which moves no day.
    let ahead = if berlin_offset(utc, minutes) == "+02:00" { 7_200 } else { 3_600 };
    Day(i32::try_from(unix_seconds.saturating_add(ahead) / 86_400).unwrap_or(0))
}

/// Today in Berlin.
pub fn today() -> Day {
    berlin_day(SystemTime::now().duration_since(UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or(0))
}

/// The feed of some notices: `{"announcements": [{"key", "topic", "at", "for", "link", "texts"}]}`.
/// `at` is left out for a notice without an hour of its own; `texts` has a title and a body per
/// language, and for a language whose page has another address its own link.
pub fn feed(notices: &[Notice]) -> String {
    let entries: Vec<Value> = notices.iter().map(entry).collect();
    json!({ "announcements": entries }).to_string()
}

fn entry(notice: &Notice) -> Value {
    // The page a tap opens: the day (the start page), or the Stundenplan for a semester to plan.
    let page = match notice.what {
        What::Dates => folia_routes::url::STUDYPLAN,
        _ => "/",
    };
    let mut languages = Map::new();
    for locale in Locale::ALL.iter().copied() {
        let (title, body) = words(notice, locale);
        let mut text = Map::new();
        text.insert("title".to_string(), Value::String(title));
        text.insert("body".to_string(), Value::String(body));
        let link = locale.path(page);
        if link != page {
            text.insert("link".to_string(), Value::String(link));
        }
        languages.insert(locale.code().to_string(), Value::Object(text));
    }
    let mut out = Map::new();
    out.insert("key".to_string(), Value::String(notice.key()));
    out.insert("topic".to_string(), Value::String(notice.topic().to_string()));
    if let Some((day, minutes)) = notice.at {
        let at = format!("{}T{:02}:{:02}:00{}", day.iso(), minutes / 60, minutes % 60, berlin_offset(day, minutes));
        out.insert("at".to_string(), Value::String(at));
    }
    out.insert("for".to_string(), Value::String(format!("{}h", notice.hours)));
    out.insert("link".to_string(), Value::String(page.to_string()));
    out.insert("texts".to_string(), Value::Object(languages));
    Value::Object(out)
}

/// A notice's title and body in a language.
fn words(notice: &Notice, locale: Locale) -> (String, String) {
    let t = texts(locale);
    let semester = notice.semester.label(locale);
    match notice.what {
        What::LecturesBegin => (t.lectures_begin.to_string(), (t.lectures_begin_text)(&semester)),
        What::Break { weeks, resume, christmas, .. } => {
            let title = match (christmas, weeks) {
                (true, _) => t.christmas_break,
                (false, 1) => t.break_week,
                (false, _) => t.break_weeks,
            };
            (title.to_string(), (t.break_text)(&resume.day_month(locale)))
        }
        What::Exams { nr, last } => {
            let title = match nr {
                1 => t.exams_begin,
                2 => t.exams_begin_second,
                _ => t.exams_begin_further,
            };
            (title.to_string(), (t.exams_text)(&last.day_month(locale), &semester))
        }
        What::Dates => ((t.dates_title)(&semester), (t.dates_text)(&semester)),
    }
}

#[cfg(test)]
mod tests {
    use folia_calendar::semester::SemesterKey;

    use super::*;

    fn d(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn at(text: &str) -> u64 {
        // "2026-11-18 23:30" in UTC.
        let (day, clock) = text.split_once(' ').unwrap();
        let (hours, minutes) = clock.split_once(':').unwrap();
        u64::try_from(d(day).0).unwrap() * 86_400 + hours.parse::<u64>().unwrap() * 3_600 + minutes.parse::<u64>().unwrap() * 60
    }

    #[test]
    fn the_day_is_berlins() {
        for (utc, berlin) in [
            ("2026-11-18 12:00", "2026-11-18"),
            ("2026-11-18 22:59", "2026-11-18"), // 23:59 in winter
            ("2026-11-18 23:00", "2026-11-19"), // midnight
            ("2026-07-01 21:59", "2026-07-01"), // 23:59 in summer
            ("2026-07-01 22:00", "2026-07-02"),
            ("2026-12-31 23:30", "2027-01-01"),
        ] {
            assert_eq!(berlin_day(at(utc)).iso(), berlin, "{utc}");
        }
    }

    #[test]
    fn a_feed_of_every_kind_of_notice() {
        let winter = SemesterKey::parse("2026W").unwrap();
        let summer = SemesterKey::parse("2027S").unwrap();
        let notices = [
            Notice { semester: winter, what: What::LecturesBegin, at: Some((d("2026-10-05"), 7 * 60)), hours: 12 },
            Notice {
                semester: winter,
                what: What::Break { nr: 1, weeks: 2, resume: d("2027-01-04"), christmas: true },
                at: Some((d("2026-12-18"), 15 * 60)),
                hours: 60,
            },
            Notice { semester: winter, what: What::Exams { nr: 1, last: d("2027-02-20") }, at: Some((d("2027-02-08"), 7 * 60)), hours: 12 },
            Notice { semester: winter, what: What::Exams { nr: 2, last: d("2027-03-20") }, at: Some((d("2027-03-08"), 7 * 60)), hours: 12 },
            Notice { semester: summer, what: What::Dates, at: None, hours: 72 },
        ];
        let feed: Value = serde_json::from_str(&feed(&notices)).unwrap();
        let entries = feed["announcements"].as_array().unwrap();
        assert_eq!(entries.len(), 5);

        // The moment is Berlin's, with the offset of its day: summer time in October, not in December.
        assert_eq!(entries[0]["key"], "lectures-2026w-begin");
        assert_eq!(entries[0]["at"], "2026-10-05T07:00:00+02:00");
        assert_eq!(entries[0]["topic"], "semester");
        assert_eq!(entries[0]["texts"]["de"]["title"], "Die Vorlesungszeit beginnt");
        assert_eq!(entries[0]["texts"]["de"]["body"], "Heute starten die Vorlesungen im WiSe 2026/27.");
        assert_eq!(entries[0]["texts"]["en"]["body"], "The lectures of Winter 2026/27 start today.");

        assert_eq!(entries[1]["key"], "breaks-2026w-1");
        assert_eq!(entries[1]["at"], "2026-12-18T15:00:00+01:00");
        assert_eq!((&entries[1]["for"], &entries[1]["topic"]), (&json!("60h"), &json!("breaks")));
        assert_eq!(entries[1]["texts"]["de"]["title"], "Weihnachtspause");
        assert_eq!(entries[1]["texts"]["de"]["body"], "Ab Montag ist vorlesungsfrei. Weiter geht es am 04.01.");
        assert_eq!(entries[1]["texts"]["en"]["title"], "Christmas break");
        assert_eq!(entries[1]["texts"]["en"]["body"], "No lectures from Monday. They resume on 4 Jan.");

        assert_eq!(entries[2]["key"], "exams-2026w-1");
        assert_eq!(entries[2]["texts"]["de"]["title"], "Die Prüfungsphase beginnt");
        assert_eq!(entries[2]["texts"]["de"]["body"], "Bis 20.02. stehen viele Prüfungen des WiSe 2026/27 an. Viel Erfolg!");
        assert_eq!(entries[3]["texts"]["de"]["title"], "Die zweite Prüfungsphase beginnt");
        assert_eq!(entries[3]["texts"]["en"]["title"], "The second exam period begins");

        // No hour of its own: no "at". It opens the Stundenplan, in each language at its address.
        assert_eq!(entries[4]["key"], "dates-2027s");
        assert!(entries[4].get("at").is_none());
        assert_eq!((&entries[4]["for"], &entries[4]["link"]), (&json!("72h"), &json!("/studyplan")));
        assert_eq!(entries[4]["texts"]["de"]["title"], "Die Termine fürs SoSe 2027 sind da");
        assert!(entries[4]["texts"]["de"].get("link").is_none());
        assert_eq!(entries[4]["texts"]["en"]["link"], "/en/studyplan");
        assert_eq!(entries[0]["link"], "/");
        assert_eq!(entries[0]["texts"]["en"]["link"], "/en");

        // What Xylem takes: a title of 120 and a body of 600 characters at most, in every language.
        for entry in entries {
            for locale in Locale::ALL.iter().copied() {
                let text = &entry["texts"][locale.code()];
                let (title, body) = (text["title"].as_str().unwrap(), text["body"].as_str().unwrap());
                assert!(!title.is_empty() && title.chars().count() <= 120 && !body.is_empty() && body.chars().count() <= 600, "{entry}");
            }
        }
        assert_eq!(super::feed(&[]), r#"{"announcements":[]}"#);

        // The other weeks without lectures, and a third exam period.
        let rest = [
            Notice { semester: winter, what: What::Break { nr: 2, weeks: 1, resume: d("2026-11-23"), christmas: false }, at: None, hours: 60 },
            Notice { semester: winter, what: What::Break { nr: 3, weeks: 2, resume: d("2026-11-30"), christmas: false }, at: None, hours: 60 },
            Notice { semester: winter, what: What::Exams { nr: 3, last: d("2027-03-27") }, at: None, hours: 12 },
        ];
        let feed: Value = serde_json::from_str(&super::feed(&rest)).unwrap();
        assert_eq!(feed["announcements"][0]["texts"]["de"]["title"], "Vorlesungsfreie Woche");
        assert_eq!(feed["announcements"][1]["texts"]["de"]["title"], "Vorlesungsfreie Wochen");
        assert_eq!(feed["announcements"][1]["texts"]["en"]["title"], "Weeks without lectures");
        assert_eq!(feed["announcements"][2]["texts"]["de"]["title"], "Eine weitere Prüfungsphase beginnt");
    }
}
