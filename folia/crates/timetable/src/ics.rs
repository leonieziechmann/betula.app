//! An iCalendar writer (RFC 5545): a `Calendar` of entries becomes the text of an `.ics` file.
//!
//! It writes what `export` decided and nothing else. Every entry is one VEVENT: there is no RRULE
//! and no EXDATE, so cancellations, breaks, holidays and room notes are exact per date, and no
//! client's handling of UNTIL or EXDATE next to a TZID can move a Termin. Timed entries are wall
//! clock times in `Europe/Berlin`, whose rules the calendar spells out in its VTIMEZONE, so the
//! client's own time zone database places the dates around the switches of 2026-10-25 and
//! 2027-03-28. All-day entries are dates.
//!
//! There is no clock here either: DTSTAMP is the calendar's `stamp`, the snapshot's
//! `data_changed_at`. An unchanged plan in an unchanged snapshot is therefore the same bytes, in
//! the browser's download and in the server's feed alike, and the feed's ETag holds.

use folia_calendar::day::Day;

/// The time zone of every timed entry; `TIMEZONE` spells out its rules.
pub const TZID: &str = "Europe/Berlin";

/// The most octets of one physical line, its line break not counted (RFC 5545 §3.1).
const LINE_OCTETS: usize = 75;

/// What a calendar says before its name.
const HEAD: [&str; 5] = ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Betula//Studienplan 1//DE", "CALSCALE:GREGORIAN", "METHOD:PUBLISH"];

/// After the name and the description. The refresh interval asks clients that honour it to fetch
/// twice a day; Radix changes the snapshot every few days, so more often would find nothing new.
const REFRESH: [&str; 3] = ["X-WR-TIMEZONE:Europe/Berlin", "REFRESH-INTERVAL;VALUE=DURATION:PT12H", "X-PUBLISHED-TTL:PT12H"];

/// Central European time with its summer time as it has been since 1996: from the last Sunday of
/// March, 02:00, to the last Sunday of October, 03:00.
const TIMEZONE: [&str; 17] = [
    "BEGIN:VTIMEZONE",
    "TZID:Europe/Berlin",
    "BEGIN:DAYLIGHT",
    "TZOFFSETFROM:+0100",
    "TZOFFSETTO:+0200",
    "TZNAME:CEST",
    "DTSTART:19700329T020000",
    "RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU",
    "END:DAYLIGHT",
    "BEGIN:STANDARD",
    "TZOFFSETFROM:+0200",
    "TZOFFSETTO:+0100",
    "TZNAME:CET",
    "DTSTART:19701025T030000",
    "RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU",
    "END:STANDARD",
    "END:VTIMEZONE",
];

/// The DTSTAMP of a calendar whose `stamp` is not one: a fixed value, never the clock, so the
/// bytes still do not change from one fetch to the next.
const EPOCH_STAMP: &str = "19700101T000000Z";

/// A calendar as `write` spells it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Calendar {
    /// `X-WR-CALNAME`: „Studienplan WiSe 2026/27". Left out when empty.
    pub name: String,
    /// `X-WR-CALDESC`: where the dates come from and how fresh they are. Left out when empty.
    pub description: String,
    /// Every entry's DTSTAMP, as `stamp_of` makes it: `20260923T123516Z`.
    pub stamp: String,
    pub entries: Vec<Entry>,
}

/// One VEVENT. Empty texts are left out, as `None` is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub uid: String,
    pub when: When,
    pub summary: String,
    pub location: Option<String>,
    /// Lines separated by `\n`.
    pub description: Option<String>,
    pub url: Option<String>,
    pub categories: Vec<String>,
    /// `STATUS:TENTATIVE` instead of `CONFIRMED`: a date the calendar shows as uncertain.
    pub tentative: bool,
    /// `TRANSP:TRANSPARENT` instead of `OPAQUE`: the entry does not make its time busy.
    pub transparent: bool,
}

/// When an entry takes place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    /// A day and minutes since its midnight in `Europe/Berlin`. `to` 1440 is 24:00, written as
    /// 00:00 of the next day. An end not after the start is left out: the entry is then a moment,
    /// which RFC 5545 reads from a DTSTART without DTEND.
    Timed { day: Day, from: u16, to: u16 },
    /// Whole days, `last` included (DTEND, which excludes, is the day after it).
    AllDay { first: Day, last: Day },
}

/// The text of the calendar: lines ending in CRLF, the last one too, none longer than 75 octets.
pub fn write(calendar: &Calendar) -> String {
    let mut out = String::with_capacity(1024 + calendar.entries.len() * 512);
    for line in HEAD {
        put(&mut out, line);
    }
    if !calendar.name.is_empty() {
        put(&mut out, &format!("X-WR-CALNAME:{}", escape_text(&calendar.name)));
    }
    if !calendar.description.is_empty() {
        put(&mut out, &format!("X-WR-CALDESC:{}", escape_text(&calendar.description)));
    }
    for line in REFRESH.into_iter().chain(TIMEZONE) {
        put(&mut out, line);
    }
    let stamp = if is_stamp(&calendar.stamp) { calendar.stamp.as_str() } else { EPOCH_STAMP };
    for entry in &calendar.entries {
        write_entry(&mut out, entry, stamp);
    }
    put(&mut out, "END:VCALENDAR");
    out
}

fn write_entry(out: &mut String, entry: &Entry, stamp: &str) {
    put(out, "BEGIN:VEVENT");
    put(out, &format!("UID:{}", escape_text(&entry.uid)));
    put(out, &format!("DTSTAMP:{stamp}"));
    match entry.when {
        When::Timed { day, from, to } => {
            put(out, &format!("DTSTART;TZID={TZID}:{}", local(day, from)));
            if to > from {
                put(out, &format!("DTEND;TZID={TZID}:{}", local(day, to)));
            }
        }
        When::AllDay { first, last } => {
            put(out, &format!("DTSTART;VALUE=DATE:{}", first.compact()));
            put(out, &format!("DTEND;VALUE=DATE:{}", last.max(first).plus(1).compact()));
        }
    }
    put(out, &format!("SUMMARY:{}", escape_text(&entry.summary)));
    for (name, text) in [("LOCATION", &entry.location), ("DESCRIPTION", &entry.description)] {
        if let Some(text) = text.as_deref().filter(|text| !text.is_empty()) {
            put(out, &format!("{name}:{}", escape_text(text)));
        }
    }
    if let Some(url) = entry.url.as_deref().map(uri).filter(|url| !url.is_empty()) {
        put(out, &format!("URL:{url}"));
    }
    let categories: Vec<String> = entry.categories.iter().filter(|c| !c.is_empty()).map(|c| escape_text(c)).collect();
    if !categories.is_empty() {
        put(out, &format!("CATEGORIES:{}", categories.join(",")));
    }
    put(out, if entry.tentative { "STATUS:TENTATIVE" } else { "STATUS:CONFIRMED" });
    put(out, if entry.transparent { "TRANSP:TRANSPARENT" } else { "TRANSP:OPAQUE" });
    put(out, "END:VEVENT");
}

/// One content line, folded, and its CRLF.
fn put(out: &mut String, line: &str) {
    out.push_str(&fold(line));
    out.push_str("\r\n");
}

/// `20261013T113000`: the local date and time of minutes after the day's midnight. 1440 and more
/// fall on the following days, so 24:00 is 00:00 of the next day: RFC 5545 has no hour 24.
fn local(day: Day, minutes: u16) -> String {
    let day = day.plus(i32::from(minutes / 1440));
    let minutes = minutes % 1440;
    format!("{}T{:02}{:02}00", day.compact(), minutes / 60, minutes % 60)
}

/// A URI as a property value. RFC 5545 does not escape URIs, so only what would break the line is
/// dropped: control characters, which no address QIS writes has.
fn uri(url: &str) -> String {
    url.chars().filter(|c| !c.is_ascii_control()).collect()
}

/// A TEXT value (RFC 5545 §3.3.11): backslash, semicolon and comma escaped, a line break written
/// as `\n`, a carriage return dropped. The other ASCII control characters but the tab, which TEXT
/// does not allow, are dropped as well: they come only from damaged source text.
pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push('\t'),
            c if c.is_ascii_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// A content line folded to at most 75 octets per physical line (RFC 5545 §3.1): each
/// continuation starts with CRLF and a space, and the space counts as its first octet. A line is
/// broken only between characters, never inside a character's UTF-8 bytes, so an „Ü" whose second
/// octet would be the 76th moves to the next line whole. No CRLF at the end: `write` adds it.
pub fn fold(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + line.len() / (LINE_OCTETS - 1) * 3);
    let mut octets = 0;
    for c in line.chars() {
        let width = c.len_utf8();
        if octets + width > LINE_OCTETS {
            out.push_str("\r\n ");
            octets = 1;
        }
        out.push(c);
        octets += width;
    }
    out
}

/// The DTSTAMP of a moment in RFC 3339 and UTC, as Radix writes `meta.data_changed_at`:
/// `2026-09-23T12:35:16Z` → `20260923T123516Z`. A fraction of a second is dropped; an offset
/// other than `Z` or `+00:00`, and anything that is no valid moment, gives `None`.
pub fn stamp_of(rfc3339_utc: &str) -> Option<String> {
    let text = rfc3339_utc.trim();
    let day = Day::parse(text.get(..10)?)?;
    let rest = text.get(10..)?.strip_prefix(|c: char| matches!(c, 'T' | 't' | ' '))?;
    let [h1, h2, b':', m1, m2, b':', s1, s2, zone @ ..] = rest.as_bytes() else {
        return None;
    };
    let digits = [*h1, *h2, *m1, *m2, *s1, *s2];
    if !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let two = |high: u8, low: u8| u32::from(high - b'0') * 10 + u32::from(low - b'0');
    // Second 60 is a leap second, which RFC 5545's DATE-TIME allows as well.
    if two(*h1, *h2) > 23 || two(*m1, *m2) > 59 || two(*s1, *s2) > 60 {
        return None;
    }
    let zone = match zone.strip_prefix(b".") {
        Some(fraction) => {
            let digits = fraction.iter().take_while(|d| d.is_ascii_digit()).count();
            if digits == 0 {
                return None;
            }
            fraction.get(digits..)?
        }
        None => zone,
    };
    if !matches!(zone, b"Z" | b"z" | b"+00:00") {
        return None;
    }
    let time: String = digits.iter().map(|d| char::from(*d)).collect();
    Some(format!("{}T{time}Z", day.compact()))
}

/// `YYYYMMDDTHHMMSSZ`, the shape of a DATE-TIME in UTC.
fn is_stamp(text: &str) -> bool {
    text.len() == 16
        && text.bytes().enumerate().all(|(at, c)| match at {
            8 => c == b'T',
            15 => c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    fn entry(uid: &str, when: When, summary: &str) -> Entry {
        Entry { uid: uid.into(), when, summary: summary.into(), location: None, description: None, url: None, categories: vec![], tentative: false, transparent: false }
    }

    /// The physical lines of a written calendar, which must end in CRLF.
    fn lines(text: &str) -> Vec<&str> {
        assert!(text.ends_with("\r\n"), "no CRLF at the end");
        text.strip_suffix("\r\n").unwrap().split("\r\n").collect()
    }

    /// A lecture, an exam period and a second sitting that ends at midnight: every property
    /// `export` writes, escaping and folding included, byte for byte.
    #[test]
    fn a_calendar_of_three_entries() {
        let calendar = Calendar {
            name: "Studienplan WiSe 2026/27".into(),
            description: "Betula (inoffiziell) · Termine laut QIS, Stand 23.09.2026".into(),
            stamp: "20260923T123516Z".into(),
            entries: vec![
                Entry {
                    uid: "148701-a2633-20261013@betula.app".into(),
                    when: When::Timed { day: day("2026-10-13"), from: 690, to: 780 },
                    summary: "Entwicklung von Softwaresystemen · Vorlesung".into(),
                    location: Some("Zentrales Hörsaalgebäude - Audimax 1 - Zentralcampus".into()),
                    description: Some("Modul 12104 Entwicklung von Softwaresystemen\nwöchentlich 13.10.2026–26.01.2027\nQuelle: QIS".into()),
                    url: Some("https://www.b-tu.de/qisserver3/rds?state=verpublish&status=init&vmfile=no&moduleCall=webInfo&publishConfFile=webInfo&publishSubDir=veranstaltung&veranstid=148701".into()),
                    categories: vec!["Vorlesung".into()],
                    tentative: false,
                    transparent: false,
                },
                Entry {
                    uid: "150664-12345-20270215@betula.app".into(),
                    when: When::AllDay { first: day("2027-02-15"), last: day("2027-02-26") },
                    summary: "Mathematik; Teil 1 · Prüfungszeitraum".into(),
                    location: None,
                    description: None,
                    url: None,
                    categories: vec!["Prüfung".into(), "a,b".into()],
                    tentative: false,
                    transparent: true,
                },
                Entry {
                    uid: "148005-0abcd-20270311@betula.app".into(),
                    when: When::Timed { day: day("2027-03-11"), from: 1320, to: 1440 },
                    summary: "Spät · Prüfung · 2. Termin".into(),
                    location: Some(String::new()),
                    description: Some("C:\\temp".into()),
                    url: None,
                    categories: vec![],
                    tentative: true,
                    transparent: false,
                },
            ],
        };
        let expected = concat!(
            "BEGIN:VCALENDAR\r\n",
            "VERSION:2.0\r\n",
            "PRODID:-//Betula//Studienplan 1//DE\r\n",
            "CALSCALE:GREGORIAN\r\n",
            "METHOD:PUBLISH\r\n",
            "X-WR-CALNAME:Studienplan WiSe 2026/27\r\n",
            "X-WR-CALDESC:Betula (inoffiziell) · Termine laut QIS\\, Stand 23.09.2026\r\n",
            "X-WR-TIMEZONE:Europe/Berlin\r\n",
            "REFRESH-INTERVAL;VALUE=DURATION:PT12H\r\n",
            "X-PUBLISHED-TTL:PT12H\r\n",
            "BEGIN:VTIMEZONE\r\n",
            "TZID:Europe/Berlin\r\n",
            "BEGIN:DAYLIGHT\r\n",
            "TZOFFSETFROM:+0100\r\n",
            "TZOFFSETTO:+0200\r\n",
            "TZNAME:CEST\r\n",
            "DTSTART:19700329T020000\r\n",
            "RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\n",
            "END:DAYLIGHT\r\n",
            "BEGIN:STANDARD\r\n",
            "TZOFFSETFROM:+0200\r\n",
            "TZOFFSETTO:+0100\r\n",
            "TZNAME:CET\r\n",
            "DTSTART:19701025T030000\r\n",
            "RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\n",
            "END:STANDARD\r\n",
            "END:VTIMEZONE\r\n",
            "BEGIN:VEVENT\r\n",
            "UID:148701-a2633-20261013@betula.app\r\n",
            "DTSTAMP:20260923T123516Z\r\n",
            "DTSTART;TZID=Europe/Berlin:20261013T113000\r\n",
            "DTEND;TZID=Europe/Berlin:20261013T130000\r\n",
            "SUMMARY:Entwicklung von Softwaresystemen · Vorlesung\r\n",
            "LOCATION:Zentrales Hörsaalgebäude - Audimax 1 - Zentralcampus\r\n",
            "DESCRIPTION:Modul 12104 Entwicklung von Softwaresystemen\\nwöchentlich 13.1\r\n",
            " 0.2026–26.01.2027\\nQuelle: QIS\r\n",
            "URL:https://www.b-tu.de/qisserver3/rds?state=verpublish&status=init&vmfile=\r\n",
            " no&moduleCall=webInfo&publishConfFile=webInfo&publishSubDir=veranstaltung&\r\n",
            " veranstid=148701\r\n",
            "CATEGORIES:Vorlesung\r\n",
            "STATUS:CONFIRMED\r\n",
            "TRANSP:OPAQUE\r\n",
            "END:VEVENT\r\n",
            "BEGIN:VEVENT\r\n",
            "UID:150664-12345-20270215@betula.app\r\n",
            "DTSTAMP:20260923T123516Z\r\n",
            "DTSTART;VALUE=DATE:20270215\r\n",
            "DTEND;VALUE=DATE:20270227\r\n",
            "SUMMARY:Mathematik\\; Teil 1 · Prüfungszeitraum\r\n",
            "CATEGORIES:Prüfung,a\\,b\r\n",
            "STATUS:CONFIRMED\r\n",
            "TRANSP:TRANSPARENT\r\n",
            "END:VEVENT\r\n",
            "BEGIN:VEVENT\r\n",
            "UID:148005-0abcd-20270311@betula.app\r\n",
            "DTSTAMP:20260923T123516Z\r\n",
            "DTSTART;TZID=Europe/Berlin:20270311T220000\r\n",
            "DTEND;TZID=Europe/Berlin:20270312T000000\r\n",
            "SUMMARY:Spät · Prüfung · 2. Termin\r\n",
            "DESCRIPTION:C:\\\\temp\r\n",
            "STATUS:TENTATIVE\r\n",
            "TRANSP:OPAQUE\r\n",
            "END:VEVENT\r\n",
            "END:VCALENDAR\r\n",
        );
        let written = write(&calendar);
        assert_eq!(written, expected);
        for line in lines(&written) {
            assert!(line.len() <= LINE_OCTETS, "{line:?}");
        }
    }

    #[test]
    fn lines_fold_between_characters() {
        // 74 octets, then „Übung": its two octets would be the 75th and the 76th.
        let line = format!("SUMMARY:{}Übung", "x".repeat(66));
        let folded = fold(&line);
        assert_eq!(folded, format!("SUMMARY:{}\r\n Übung", "x".repeat(66)));
        assert_eq!(folded.split("\r\n").next().unwrap().len(), 74);
        // Exactly 75 octets stay one line; one more starts a continuation.
        let full = "y".repeat(75);
        assert_eq!(fold(&full), full);
        assert_eq!(fold(&"y".repeat(76)), format!("{}\r\n y", "y".repeat(75)));
        // A continuation holds 74 octets after its space.
        assert_eq!(fold(&"z".repeat(75 + 74 + 1)), format!("{}\r\n {}\r\n z", "z".repeat(75), "z".repeat(74)));
        assert_eq!(fold(""), "");
        // Unfolding gives the line back, whatever its characters.
        let mixed = "DESCRIPTION:".to_string() + &"Prüfung · Übung – 😀 ".repeat(20);
        let folded = fold(&mixed);
        assert_eq!(folded.replace("\r\n ", ""), mixed);
        assert!(folded.split("\r\n").all(|line| line.len() <= LINE_OCTETS));
    }

    #[test]
    fn text_is_escaped() {
        assert_eq!(escape_text("a,b;c\\d\ne"), "a\\,b\\;c\\\\d\\ne");
        assert_eq!(escape_text("zwei\r\nZeilen"), "zwei\\nZeilen");
        assert_eq!(escape_text("tab\there\u{7}bell\u{7f}"), "tab\therebell");
        assert_eq!(escape_text("Hörsaal: 1 · „A\""), "Hörsaal: 1 · „A\"");
    }

    #[test]
    fn all_day_entries_end_the_day_after() {
        let mut calendar = Calendar { name: "x".into(), stamp: "20260923T123516Z".into(), ..Calendar::default() };
        calendar.entries.push(entry("a@betula.app", When::AllDay { first: day("2027-03-12"), last: day("2027-03-12") }, "Abgabe"));
        calendar.entries.push(entry("b@betula.app", When::AllDay { first: day("2026-12-31"), last: day("2026-12-31") }, "Silvester"));
        // A range given backwards is its first day alone.
        calendar.entries.push(entry("c@betula.app", When::AllDay { first: day("2027-01-05"), last: day("2027-01-01") }, "Rückwärts"));
        let written = write(&calendar);
        assert!(written.contains("DTSTART;VALUE=DATE:20270312\r\nDTEND;VALUE=DATE:20270313\r\n"), "{written}");
        assert!(written.contains("DTSTART;VALUE=DATE:20261231\r\nDTEND;VALUE=DATE:20270101\r\n"), "{written}");
        assert!(written.contains("DTSTART;VALUE=DATE:20270105\r\nDTEND;VALUE=DATE:20270106\r\n"), "{written}");
        assert!(!written.contains("X-WR-CALDESC"), "an empty description is left out");
    }

    #[test]
    fn midnight_is_the_next_day() {
        let mut calendar = Calendar { stamp: "20260923T123516Z".into(), ..Calendar::default() };
        calendar.entries.push(entry("a@betula.app", When::Timed { day: day("2027-01-31"), from: 1080, to: 1440 }, "bis 24 Uhr"));
        // An end not after the start leaves DTEND out.
        calendar.entries.push(entry("b@betula.app", When::Timed { day: day("2027-02-01"), from: 600, to: 600 }, "Punkt"));
        let written = write(&calendar);
        assert!(written.contains("DTSTART;TZID=Europe/Berlin:20270131T180000\r\nDTEND;TZID=Europe/Berlin:20270201T000000\r\n"), "{written}");
        assert!(written.contains("DTSTART;TZID=Europe/Berlin:20270201T100000\r\nSUMMARY:Punkt\r\n"), "{written}");
        assert!(!written.contains("X-WR-CALNAME"), "an empty name is left out");
    }

    #[test]
    fn every_line_is_short_and_ends_in_crlf() {
        let long = "Lehrveranstaltung mit einem sehr langen Titel, Übungen; Seminare \\ Praktika – ".repeat(6);
        let mut calendar = Calendar { name: long.clone(), description: long.clone(), stamp: "20260923T123516Z".into(), entries: vec![] };
        for n in 0..40u16 {
            let mut e = entry(&format!("{n}-00000-20261013@betula.app"), When::Timed { day: day("2026-10-13"), from: 480 + n, to: 570 + n }, &long);
            e.location = Some(long.clone());
            e.description = Some(format!("{long}\n{long}"));
            e.url = Some(format!("https://example.org/{}\r\n", "a".repeat(200)));
            e.categories = vec![long.clone(), "Übung".into()];
            calendar.entries.push(e);
        }
        let written = write(&calendar);
        let joined = written.replace("\r\n", "");
        assert!(!joined.contains('\n') && !joined.contains('\r'), "a bare line break");
        for line in lines(&written) {
            assert!(line.len() <= LINE_OCTETS, "{} octets: {line:?}", line.len());
        }
        assert_eq!(written.matches("BEGIN:VEVENT\r\n").count(), 40);
    }

    #[test]
    fn stamps_of_moments() {
        assert_eq!(stamp_of("2026-09-23T12:35:16Z").as_deref(), Some("20260923T123516Z"));
        assert_eq!(stamp_of(" 2026-09-23t12:35:16.250z ").as_deref(), Some("20260923T123516Z"));
        assert_eq!(stamp_of("2026-09-23 12:35:16+00:00").as_deref(), Some("20260923T123516Z"));
        for bad in [
            "",
            "2026-09-23",
            "2026-09-23T12:35Z",
            "2026-09-23T12:35:16",
            "2026-09-23T12:35:16+02:00",
            "2026-02-30T12:35:16Z",
            "2026-09-23T24:00:00Z",
            "2026-09-23T12:35:16.Z",
            "2026-09-23T1a:35:16Z",
            "2026-09-23T12:35:16Zx",
            "2026-09-23T12:35:16Ü",
        ] {
            assert_eq!(stamp_of(bad), None, "{bad:?}");
        }
        // A calendar whose stamp is none still writes a valid, fixed one.
        let calendar = Calendar { stamp: "gestern".into(), entries: vec![entry("a@betula.app", When::AllDay { first: day("2027-03-12"), last: day("2027-03-12") }, "x")], ..Calendar::default() };
        assert!(write(&calendar).contains("DTSTAMP:19700101T000000Z\r\n"));
    }
}
