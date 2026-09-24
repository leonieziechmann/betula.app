//! The dates a row's note cancels, and those it moves (`cancelled_dates`).
//!
//! QIS lists per row the dates it „fällt aus am", each as `DD.MM.YYYY:` with an optional reason,
//! and Radix keeps the column as free text (`'05.10.2026: 12.10.2026:'`,
//! `'22.01.2027: Raumwechsel'`). A listed date cancels its occurrence unless the reason is a room
//! note (a new room, a time and a room): then the occurrence is held and the note attaches. A reason
//! that names another date („findet am 10.12.2026 statt") is told to the visitor, but no occurrence
//! is invented from it. Exam rows do not expose the column.

use super::day::Day;
use super::kind::fold;

/// One date of the column: the date, whether it is cancelled, the reason as written, and the date
/// the reason moves it to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CancelNote {
    pub day: Day,
    /// False for a room note: the occurrence is held there.
    pub cancels: bool,
    /// The text after the date up to the next one, trimmed; `None` when there is none.
    pub note: Option<String>,
    /// The date the note says it takes place instead. Informational only.
    pub moved_to: Option<Day>,
}

/// `DD.MM.YYYY:`
const TOKEN_LEN: usize = 11;

/// In a folded note: the occurrence is held, in another room.
const ROOM_WORDS: [&str; 3] = ["raumwechsel", "raumänderung", "ersatzweise"];

/// In a folded note: the note may name the date the occurrence moves to.
const MOVED_WORDS: [&str; 4] = ["statt", "takes place", "verleg", "verschoben"];

/// Every `DD.MM.YYYY:` of the text with its note, in the order written. Text before the first
/// date, and a text without one, give nothing.
pub fn parse(text: &str) -> Vec<CancelNote> {
    let tokens = tokens(text);
    let mut notes = Vec::with_capacity(tokens.len());
    for (i, (start, day)) in tokens.iter().enumerate() {
        let end = tokens.get(i + 1).map_or(text.len(), |(next, _)| *next);
        let note = text
            .get(start + TOKEN_LEN..end)
            .map(str::trim)
            .filter(|note| !note.is_empty())
            .map(str::to_string);
        let folded = note.as_deref().map(fold).unwrap_or_default();
        let moved_to = if MOVED_WORDS.iter().any(|word| folded.contains(word)) {
            note.as_deref().and_then(first_date)
        } else {
            None
        };
        notes.push(CancelNote { day: *day, cancels: !is_room_note(&folded), note, moved_to });
    }
    notes
}

/// The start and the day of every `DD.MM.YYYY:` with a real date, left to right.
fn tokens(text: &str) -> Vec<(usize, Day)> {
    let mut found = Vec::new();
    let mut i = 0;
    while i + TOKEN_LEN <= text.len() {
        match date_at(text, i).filter(|_| text.as_bytes().get(i + TOKEN_LEN - 1) == Some(&b':')) {
            Some(day) => {
                found.push((i, day));
                i += TOKEN_LEN;
            }
            None => i += 1,
        }
    }
    found
}

/// The first `DD.MM.YYYY` of a note that is not followed by `:` (that would be the next listed
/// date, not the one it moves to).
fn first_date(note: &str) -> Option<Day> {
    (0..note.len()).find_map(|i| date_at(note, i).filter(|_| note.as_bytes().get(i + TOKEN_LEN - 1) != Some(&b':')))
}

/// The date `DD.MM.YYYY` starting at byte `i`, unless it is the tail of a longer number.
fn date_at(text: &str, i: usize) -> Option<Day> {
    let after_digit = i.checked_sub(1).and_then(|before| text.as_bytes().get(before)).is_some_and(u8::is_ascii_digit);
    if after_digit {
        return None;
    }
    Day::parse_german(text.get(i..i + TOKEN_LEN - 1)?)
}

/// A note that moves the occurrence to another room: „Raumwechsel", „ersatzweise im …", or a
/// time and a room („09:00 - 12:15 Uhr: HG 0.18"). A bare hour („10 Uhr:") counts as a time: in
/// doubt the occurrence stays, which shows more rather than less.
fn is_room_note(folded: &str) -> bool {
    ROOM_WORDS.iter().any(|word| folded.contains(word))
        || folded.match_indices("uhr:").any(|(at, _)| ends_with_time(folded.get(..at).unwrap_or("").trim_end()))
}

/// Whether the text ends with `H`, `HH`, `H:MM` or `HH.MM` and the like.
fn ends_with_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    let hours = match bytes {
        [rest @ .., b':' | b'.', m1, m2] if m1.is_ascii_digit() && m2.is_ascii_digit() => rest,
        _ => bytes,
    };
    matches!(hours, [.., h] if h.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// One line per note: `2026-12-17 x [takes place on 10.12.2026] -> 2026-12-10`, `keep` for a
    /// held occurrence.
    fn show(text: &str) -> Vec<String> {
        parse(text)
            .iter()
            .map(|n| {
                let mut line = format!("{} {}", n.day.iso(), if n.cancels { "x" } else { "keep" });
                if let Some(note) = &n.note {
                    line.push_str(&format!(" [{note}]"));
                }
                if let Some(moved) = n.moved_to {
                    line.push_str(&format!(" -> {}", moved.iso()));
                }
                line
            })
            .collect()
    }

    /// Every distinct `cancelled_dates` of the snapshot of 2026-09-23, and what it says.
    const SNAPSHOT: &[(&str, &[&str])] = &[
        ("02.07.2026: BP10 Simulationstraining", &["2026-07-02 x [BP10 Simulationstraining]"]),
        ("02.12.2026: Enfällt wegen Fakultätsratssitzung", &["2026-12-02 x [Enfällt wegen Fakultätsratssitzung]"]),
        ("02.12.2026: Verlegung wegen Fakultätsratssitzung", &["2026-12-02 x [Verlegung wegen Fakultätsratssitzung]"]),
        ("04.05.2026:", &["2026-05-04 x"]),
        ("05.10.2026:", &["2026-10-05 x"]),
        ("05.10.2026: 12.10.2026:", &["2026-10-05 x", "2026-10-12 x"]),
        ("07.12.2026: online-asynchrone Lehre", &["2026-12-07 x [online-asynchrone Lehre]"]),
        ("08.01.2027: Info über Fachgebiet", &["2027-01-08 x [Info über Fachgebiet]"]),
        ("10.07.2026: BT8 24.07.2026: Praxis", &["2026-07-10 x [BT8]", "2026-07-24 x [Praxis]"]),
        ("13.04.2026:", &["2026-04-13 x"]),
        ("13.05.2026: 24.06.2026:", &["2026-05-13 x", "2026-06-24 x"]),
        ("15.04.2026: BT6 22.07.2026: Praxis", &["2026-04-15 x [BT6]", "2026-07-22 x [Praxis]"]),
        (
            "15.05.2026: LV findet ersatzweise im LG 3A - SR 338 statt.",
            &["2026-05-15 keep [LV findet ersatzweise im LG 3A - SR 338 statt.]"],
        ),
        (
            "15.05.2026: LV findet ersatzweise im LG 3B - SR 101 statt.",
            &["2026-05-15 keep [LV findet ersatzweise im LG 3B - SR 101 statt.]"],
        ),
        ("15.06.2026:", &["2026-06-15 x"]),
        (
            "15.09.2026: Prüfung Nanoelectronics (PD DR. U. Wulf)",
            &["2026-09-15 x [Prüfung Nanoelectronics (PD DR. U. Wulf)]"],
        ),
        ("16.12.2026: MHB", &["2026-12-16 x [MHB]"]),
        (
            "17.06.2026: LV findet ersatzweise im LG 1A HS 2 statt.",
            &["2026-06-17 keep [LV findet ersatzweise im LG 1A HS 2 statt.]"],
        ),
        ("17.07.2026: BT8 24.07.2026: Praxis", &["2026-07-17 x [BT8]", "2026-07-24 x [Praxis]"]),
        (
            "17.09.2026: Prüfung findet am 24.09.2026 statt.",
            &["2026-09-17 x [Prüfung findet am 24.09.2026 statt.] -> 2026-09-24"],
        ),
        ("17.12.2026: MHB", &["2026-12-17 x [MHB]"]),
        ("17.12.2026: takes place on 10.12.2026", &["2026-12-17 x [takes place on 10.12.2026] -> 2026-12-10"]),
        (
            "18.08.2026: 09:00 - 12:15 Uhr: HG 0.18 / 12:15 - 15:00 Uhr: MENSA-Besprechungsraum",
            &["2026-08-18 keep [09:00 - 12:15 Uhr: HG 0.18 / 12:15 - 15:00 Uhr: MENSA-Besprechungsraum]"],
        ),
        ("19.01.2027: Projektwoche", &["2027-01-19 x [Projektwoche]"]),
        ("19.06.2026: BT16 24.07.2026: Praxis", &["2026-06-19 x [BT16]", "2026-07-24 x [Praxis]"]),
        (
            "19.10.2026: LV findet ersatzweise im ZHG HS A statt.",
            &["2026-10-19 keep [LV findet ersatzweise im ZHG HS A statt.]"],
        ),
        ("21.04.2026: Beginn 14.04. 30.06.2026:", &["2026-04-21 x [Beginn 14.04.]", "2026-06-30 x"]),
        ("21.10.2026: keine Lehrveranstaltung", &["2026-10-21 x [keine Lehrveranstaltung]"]),
        ("21.12.2026: 28.12.2026:", &["2026-12-21 x", "2026-12-28 x"]),
        ("21.12.2026: Holiday 28.12.2026: Holiday", &["2026-12-21 x [Holiday]", "2026-12-28 x [Holiday]"]),
        ("22.01.2027: Raumwechsel", &["2027-01-22 keep [Raumwechsel]"]),
        ("22.05.2026: BT24 24.07.2026: Praxis", &["2026-05-22 x [BT24]", "2026-07-24 x [Praxis]"]),
        (
            "22.05.2026: Symposium 03.07.2026: BP10 Simulationstraining",
            &["2026-05-22 x [Symposium]", "2026-07-03 x [BP10 Simulationstraining]"],
        ),
        ("22.07.2026: Praxis", &["2026-07-22 x [Praxis]"]),
        ("22.12.2026:", &["2026-12-22 x"]),
        (
            "23.04.2026: LV findet ersatzweise im LG 3B-101 statt.",
            &["2026-04-23 keep [LV findet ersatzweise im LG 3B-101 statt.]"],
        ),
        ("23.06.2026: online asynchron 21.07.2026: Praxis", &["2026-06-23 x [online asynchron]", "2026-07-21 x [Praxis]"]),
        ("23.11.2026: online-asynchrone Lehre", &["2026-11-23 x [online-asynchrone Lehre]"]),
        ("23.12.2026:", &["2026-12-23 x"]),
        ("23.12.2026: 30.12.2026:", &["2026-12-23 x", "2026-12-30 x"]),
        ("24.12.2026: 31.12.2026:", &["2026-12-24 x", "2026-12-31 x"]),
        ("25.01.2027: online-asynchrone Lehre", &["2027-01-25 x [online-asynchrone Lehre]"]),
        (
            "27.10.2026: Projektwoche 01.12.2026: Projektwoche 22.12.2026: F 29.12.2026: F 19.01.2027: Projektwoche",
            &[
                "2026-10-27 x [Projektwoche]",
                "2026-12-01 x [Projektwoche]",
                "2026-12-22 x [F]",
                "2026-12-29 x [F]",
                "2027-01-19 x [Projektwoche]",
            ],
        ),
        (
            "28.10.2026: Switched to seminar 23.12.2026: Holiday 30.12.2026: Holiday",
            &["2026-10-28 x [Switched to seminar]", "2026-12-23 x [Holiday]", "2026-12-30 x [Holiday]"],
        ),
        ("29.12.2026:", &["2026-12-29 x"]),
        ("30.06.2026:", &["2026-06-30 x"]),
        ("30.12.2026:", &["2026-12-30 x"]),
        ("31.12.2026:", &["2026-12-31 x"]),
        ("M: FG 2 Physiologie", &[]),
        ("S: FG 2 - Angewandte Physik und Biomechanik", &[]),
    ];

    #[test]
    fn every_note_of_the_snapshot() {
        for (text, expected) in SNAPSHOT {
            assert_eq!(show(text), *expected, "{text:?}");
        }
    }

    /// The table above is the snapshot's whole column: a new wording shows up here.
    #[test]
    fn the_table_is_the_snapshots_column() {
        let Some(db) = crate::tests::studyplan_db("the_table_is_the_snapshots_column") else {
            return;
        };
        // The table, not a view: exam rows keep the column too, and only `event_date` shows theirs.
        let stored: BTreeSet<String> =
            crate::tests::column(&db, "SELECT DISTINCT cancelled_dates FROM event_date WHERE cancelled_dates IS NOT NULL")
                .into_iter()
                .collect();
        let table: BTreeSet<String> = SNAPSHOT.iter().map(|(text, _)| text.to_string()).collect();
        assert_eq!(stored, table);
    }

    #[test]
    fn the_design_cases() {
        assert_eq!(show("05.10.2026: 12.10.2026:"), ["2026-10-05 x", "2026-10-12 x"]);
        assert_eq!(parse("22.01.2027: Raumwechsel").first().map(|n| n.cancels), Some(false));
        assert_eq!(
            parse("17.12.2026: takes place on 10.12.2026").first().and_then(|n| n.moved_to),
            Day::from_ymd(2026, 12, 10)
        );
        assert_eq!(parse("21.04.2026: Beginn 14.04. 30.06.2026:").len(), 2);
    }

    #[test]
    fn odd_text_gives_what_it_can() {
        assert!(parse("").is_empty());
        assert!(parse("fällt aus").is_empty());
        // Text before the first date is no note of anything.
        assert_eq!(show("vorher 05.10.2026: danach"), ["2026-10-05 x [danach]"]);
        // Not a date: the 31st of November, a longer number, a date without the colon.
        assert!(parse("31.11.2026:").is_empty());
        assert!(parse("105.10.2026:").is_empty());
        assert!(parse("05.10.2026").is_empty());
        // The whole of a note up to the next date, whatever its letters.
        assert_eq!(show("05.10.2026: Raumänderung → LG 10 12.10.2026:"), ["2026-10-05 keep [Raumänderung → LG 10]", "2026-10-12 x"]);
        assert_eq!(show("05.10.2026: verschoben auf 07.10.2026"), ["2026-10-05 x [verschoben auf 07.10.2026] -> 2026-10-07"]);
        assert_eq!(show("05.10.2026: ab 10 Uhr: LG 10"), ["2026-10-05 keep [ab 10 Uhr: LG 10]"]);
        assert_eq!(show("05.10.2026: 9.30 Uhr: LG 10"), ["2026-10-05 keep [9.30 Uhr: LG 10]"]);
        assert_eq!(show("05.10.2026: Uhr: kaputt"), ["2026-10-05 x [Uhr: kaputt]"]);
        assert_eq!(show("ü05.10.2026:ä"), ["2026-10-05 x [ä]"]);
    }
}
