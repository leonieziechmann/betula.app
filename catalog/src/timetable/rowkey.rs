//! The key of one Termin, which survives a reordering of its event's rows.
//!
//! A hidden Termin, a made choice and a calendar entry's UID must name one row of an event. The
//! snapshot has no stable id for it: `event_date.id` is local to one build and `ord` is the row's
//! position on the QIS page, which moves when a row is added above. So a row is named by its event
//! and a fingerprint of what makes it that Termin: group, weekday, start, rhythm and first date.
//! Rows that agree in all five (the same slot in two rooms, mostly) share a key; they are hidden
//! together and merged per date in the feed. A row whose group, weekday, start, rhythm or first
//! date changes gets a new key and shows again, the safe direction. The end time stays out on
//! purpose: a changed end reaches every subscriber as an update of the same calendar entry.

use serde::{Deserialize, Serialize};

use crate::rows_detail::EventDate;

/// The fingerprint's width: 20 bits, five hex digits.
const FP_BITS: u32 = 20;
const FP_MASK: u32 = (1 << FP_BITS) - 1;

/// One Termin: the QIS `veranstid` of its event and the row's `fingerprint`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RowKey {
    pub event: u32,
    /// Below 2^20.
    pub fp: u32,
}

impl RowKey {
    /// The key of a row, or `None` when its event id is not a canonical number that fits a `u32`
    /// (every `veranstid` of the snapshot is; a key of anything else would not survive a round
    /// trip through text).
    pub fn of(date: &EventDate) -> Option<RowKey> {
        Some(RowKey { event: canonical_u32(&date.event_id)?, fp: fingerprint(date) })
    }

    /// `event << 20 | fp`, what a subscription code carries (`pack::set` writes the gaps).
    pub fn packed(self) -> u64 {
        u64::from(self.event) << FP_BITS | u64::from(self.fp & FP_MASK)
    }

    /// The key of a `packed()` value; `None` when the event part is 0 or does not fit a `u32`.
    pub fn unpack(value: u64) -> Option<RowKey> {
        let event = u32::try_from(value >> FP_BITS).ok().filter(|event| *event > 0)?;
        let fp = u32::try_from(value & u64::from(FP_MASK)).ok()?;
        Some(RowKey { event, fp })
    }

    /// `148369-aaf38`: the event id and five lowercase hex digits.
    pub fn text(self) -> String {
        format!("{}-{:05x}", self.event, self.fp & FP_MASK)
    }

    /// The key of `text()`: exactly `^[1-9]\d{0,9}-[0-9a-f]{5}$`, the event fitting a `u32`.
    pub fn parse(text: &str) -> Option<RowKey> {
        let (event, fp) = text.split_once('-')?;
        let fp_ok = fp.len() == 5 && fp.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !fp_ok {
            return None;
        }
        Some(RowKey { event: canonical_u32(event)?, fp: u32::from_str_radix(fp, 16).ok()? })
    }
}

/// FROZEN (codes and UIDs carry it). FNV-1a 32 over the UTF-8 of
/// `group_name ␟ weekday ␟ start_time ␟ rhythm ␟ first_date` (␟ = 0x1F, NULL → "", weekday in
/// decimal, rhythm = `Code::code()`, the stored text), folded to 20 bits as
/// `(h >> 20) ^ (h & 0xFFFFF)`. Exam rows are hashed as `modules_exams` delivers them, with group
/// and rhythm NULL.
pub fn fingerprint(date: &EventDate) -> u32 {
    let weekday = date.weekday.map(|w| w.to_string());
    let parts = [
        date.group_name.as_deref(),
        weekday.as_deref(),
        date.start_time.as_deref(),
        date.rhythm.as_ref().map(|rhythm| rhythm.code()),
        date.first_date.as_deref(),
    ];
    let mut hash: u32 = 0x811c_9dc5;
    for (i, part) in parts.iter().enumerate() {
        let separator: &[u8] = if i == 0 { b"" } else { b"\x1f" };
        for byte in separator.iter().chain(part.unwrap_or("").as_bytes()) {
            hash ^= u32::from(*byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
    }
    (hash >> FP_BITS) ^ (hash & FP_MASK)
}

/// `^[1-9][0-9]{0,9}$` that fits a `u32`: one spelling per number, so text and number round-trip.
fn canonical_u32(text: &str) -> Option<u32> {
    let bytes = text.as_bytes();
    let first_ok = bytes.first().is_some_and(|b| (b'1'..=b'9').contains(b));
    if !first_ok || bytes.len() > 10 || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::db::{fetch, DbError, FromRow, Row};
    use crate::labels::Code;

    fn row(event: &str, group: Option<&str>, weekday: Option<i64>, start: &str, rhythm: Option<&str>, first: &str) -> EventDate {
        EventDate {
            semester_key: "2026W".into(),
            semester_label: "WiSe 2026/27".into(),
            event_id: event.into(),
            event_number: None,
            event_title: "Test".into(),
            event_type: None,
            group_name: group.map(str::to_string),
            weekday,
            start_time: Some(start.into()),
            end_time: None,
            rhythm: rhythm.map(Code::parse),
            rhythm_raw: None,
            first_date: Some(first.into()),
            last_date: None,
            room: None,
            campus: None,
            instructor: None,
            comment: None,
            source_url: None,
        }
    }

    fn key(date: &EventDate) -> String {
        RowKey::of(date).unwrap().text()
    }

    /// The keys quoted in the design, from the rows of the snapshot of 2026-09-23. They are
    /// frozen: subscribed codes and calendar UIDs carry them.
    #[test]
    fn the_fingerprint_is_frozen() {
        let unnamed = Some("[unbenannt]");
        assert_eq!(key(&row("148701", unnamed, Some(2), "11:30", Some("weekly"), "2026-10-13")), "148701-a2633");
        assert_eq!(key(&row("148369", unnamed, Some(1), "15:30", Some("weekly"), "2026-10-12")), "148369-aaf38");
        assert_eq!(key(&row("148369", unnamed, Some(1), "17:30", Some("weekly"), "2026-10-12")), "148369-a4d12");
        assert_eq!(key(&row("148369", unnamed, Some(2), "15:30", Some("weekly"), "2026-10-13")), "148369-467cf");
        assert_eq!(key(&row("148369", unnamed, Some(2), "17:30", Some("weekly"), "2026-10-13")), "148369-f09d2");
        // An exam as `modules_exams` delivers it: no group, no rhythm.
        assert_eq!(key(&row("148689", None, Some(5), "11:00", None, "2027-03-12")), "148689-485e5");
        // The end time, the room and the last date are no part of it.
        let mut moved = row("148369", unnamed, Some(1), "15:30", Some("weekly"), "2026-10-12");
        moved.end_time = Some("19:00".into());
        moved.last_date = Some("2027-01-25".into());
        moved.room = Some("anderswo".into());
        assert_eq!(key(&moved), "148369-aaf38");
    }

    #[test]
    fn keys_round_trip() {
        let k = RowKey { event: 148_369, fp: 0xa4d12 };
        assert_eq!(k.text(), "148369-a4d12");
        assert_eq!(RowKey::parse("148369-a4d12"), Some(k));
        assert_eq!(k.packed(), (148_369u64 << 20) | 0xa4d12);
        assert_eq!(RowKey::unpack(k.packed()), Some(k));
        assert_eq!(RowKey { event: 7, fp: 0x5 }.text(), "7-00005");
        let top = RowKey { event: u32::MAX, fp: 0xfffff };
        assert_eq!(RowKey::parse(&top.text()), Some(top));
        assert_eq!(RowKey::unpack(top.packed()), Some(top));
        for bad in [
            "148369-zzzzz",
            "148369-A4D12",
            "148369-a4d1",
            "148369-a4d123",
            "0148369-a4d12",
            "0-a4d12",
            "-a4d12",
            "148369a4d12",
            "4294967296-00000",
            "12345678901-00000",
            "+148369-a4d12",
            "148369-a4d12 ",
            "",
        ] {
            assert_eq!(RowKey::parse(bad), None, "{bad:?}");
        }
        assert_eq!(RowKey::unpack(0xfffff), None, "event 0");
        assert_eq!(RowKey::unpack(u64::MAX), None, "event past u32");
        let mut odd = row("0148369", None, Some(1), "15:30", None, "2026-10-12");
        assert_eq!(RowKey::of(&odd), None);
        odd.event_id = "abc".into();
        assert_eq!(RowKey::of(&odd), None);
    }

    /// A row of either view with its `ord`, to count each row once.
    struct Numbered {
        ord: i64,
        date: EventDate,
    }

    impl FromRow for Numbered {
        fn from_row(row: &Row<'_>) -> Result<Self, DbError> {
            Ok(Self { ord: row.int("ord")?, date: EventDate::from_row(row)? })
        }
    }

    /// Groups of rows sharing `(event, fingerprint)`: (groups, groups whose end times differ,
    /// groups whose last dates differ).
    fn sharing(rows: &[Numbered]) -> (usize, usize, usize) {
        let mut seen = BTreeMap::new();
        for row in rows {
            seen.entry((row.date.event_id.clone(), row.ord)).or_insert(&row.date);
        }
        let mut groups: BTreeMap<RowKey, Vec<&EventDate>> = BTreeMap::new();
        for date in seen.values() {
            groups.entry(RowKey::of(date).unwrap()).or_default().push(date);
        }
        let shared: Vec<&Vec<&EventDate>> = groups.values().filter(|rows| rows.len() > 1).collect();
        let differ = |field: fn(&EventDate) -> &Option<String>| {
            shared
                .iter()
                .filter(|rows| rows.iter().map(|d| field(d)).collect::<std::collections::BTreeSet<_>>().len() > 1)
                .count()
        };
        (shared.len(), differ(|d| &d.end_time), differ(|d| &d.last_date))
    }

    /// Rows that share a key, both semesters of the pinned snapshot. A changed number means the
    /// fingerprint or the data moved: read it before changing the frozen key. On any snapshot,
    /// every row has a key.
    #[test]
    fn rows_sharing_a_key() {
        let columns = "semester_key, semester_label, event_id, event_number, event_title, ord, weekday, start_time, \
                       end_time, first_date, last_date, room, campus, comment, source_url";
        let pinned = crate::tests::studyplan_db("rows_sharing_a_key");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(crate::tests::open);
        let teaching: Vec<Numbered> = fetch(
            &db,
            "test",
            &format!(
                "SELECT {columns}, event_type, group_name, rhythm, rhythm_raw, instructor \
                 FROM v_module_schedule WHERE ord IS NOT NULL"
            ),
            &[],
        )
        .unwrap();
        let exams: Vec<Numbered> = fetch(
            &db,
            "test",
            &format!(
                "SELECT {columns}, NULL AS event_type, NULL AS group_name, NULL AS rhythm, NULL AS rhythm_raw, \
                 NULL AS instructor FROM v_module_exam WHERE ord IS NOT NULL"
            ),
            &[],
        )
        .unwrap();
        assert!(!teaching.is_empty() && !exams.is_empty());
        // `sharing` keys every row, and fails on a row without a key.
        let (teaching_groups, exam_groups) = (sharing(&teaching), sharing(&exams));
        if is_pinned {
            assert_eq!(teaching_groups, (196, 11, 9), "teaching: groups, differing end_time, differing last_date");
            assert_eq!(exam_groups, (63, 4, 0), "exams: groups, differing end_time, differing last_date");
        }
    }
}
