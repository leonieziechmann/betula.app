//! The frozen code of a calendar subscription, and the path it travels in.
//!
//! A subscribed calendar is fetched by the calendar service's servers, which send no cookie and
//! know nothing of the visitor's browser: whatever the feed needs to make the Studienplan anew has
//! to be in its address. The owner decided (2026-09-24) what that is: the semester, the planned
//! modules and what is hidden, resolved from the active snapshot on every fetch, so the exams QIS
//! publishes later arrive by themselves and the server keeps nothing. Since 2026-09-25 also the
//! program whose abbreviations the entries name the modules by („VL EvS", as compact as the week
//! the page shows; the owner chose this over each module's own abbreviation, which differs from a
//! program's in about 7 % of its compulsory modules). `Subscription` is that, packed by `pack` into
//! `/calendar/<code>.ics`, or `/en/calendar/<code>.ics` for the same calendar in English: the
//! language is the address's, not the code's.
//!
//! A code outlives releases: a calendar keeps its address for months, across blue-green switches
//! and canary rollbacks. So a code names the layout of the struct it was written in (`VERSION`, four
//! bits of `folia_pack::to_versioned_code`; owner, 2026-09-25), the struct keeps pack's rule for lasting
//! codes within its layout (fields only appended, zero meaning absent), and a reader tolerates what
//! a newer writer may put into the fields it knows: kind bits it does not know are dropped, a town
//! it does not know reads as „derive", and a program of another shape as none.
//! The address is also what the logs see, so Folia's own log writes every path under
//! `/calendar/`, after a language's prefix too, as one fixed text (`redacted_path`). The privacy notice lists what a code carries
//! („Kalender-Abo" in folia/crates/app/src/pages/legal.rs): a field added here is a word added there.

use folia_locale::Locale;
use folia_model::ids::is_program_id;
use serde::{Deserialize, Serialize};

use crate::kind::KindSet;
use crate::rowkey::RowKey;
use crate::select::{MAX_HIDDEN, MAX_MODULES, Selection, TownChoice};
use crate::semester::SemesterKey;

/// The kind of every subscription code. Frozen: it is part of every code's check characters, so a
/// code of another kind (the Merkliste's `bookmarks`) is never read as a calendar.
pub const KIND: &str = "calendar";

/// The layout of `Subscription` that codes are written in. A change the rule for lasting codes
/// does not allow takes the next one, and the reader of this one stays, for the calendars that
/// subscribed to it.
pub const VERSION: u8 = 1;

/// The most characters of a code. The reader checks it before decoding, and the writer refuses a
/// longer code, so the app never hands out an address the server turns away; well below what any
/// calendar service takes as an address.
pub const MAX_CODE: usize = 1024;

/// Where the feed lives: `/calendar/<code>.ics`, after a language's prefix in another language
/// (`/en/calendar/<code>.ics`).
pub const CALENDAR_PREFIX: &str = "/calendar/";

const SUFFIX: &str = ".ics";

/// What Folia's access log writes for every path under `CALENDAR_PREFIX`, valid or not.
const REDACTED: &str = "/calendar/….ics";

/// What a subscribed calendar shows: one semester of a Studienplan.
///
/// FROZEN LAYOUT `VERSION` (folia/crates/pack/src/lib.rs): fields only appended, zero = absent; never reorder,
/// retype, remove. Codes that calendars have subscribed to must read the same in every later
/// release; what goes beyond that is the next layout.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    /// `SemesterKey::index()`: year * 2 + 1 for a winter (2026W = 4053). 0 is refused.
    pub semester: u16,
    /// The planned modules of the semester, by their numeric ids.
    #[serde(with = "folia_pack::set")]
    pub modules: Vec<u32>,
    /// The program whose abbreviations name the modules (`program.id`, `079-82-2008`): the one
    /// the page loads the plan for, the plan's, else „Mein Studiengang". `None`, and an id of
    /// another shape (`url::is_program_id`), name each module by its own.
    pub program: Option<String>,
    /// Bit i = `EventKind::ALL[i]` hidden (lecture 0 … other 10, exam 11). Bits a reader does not
    /// know are ignored.
    pub hidden_kinds: u16,
    /// Events hidden as a whole, by `veranstid`.
    #[serde(with = "folia_pack::set")]
    pub hidden_events: Vec<u32>,
    /// `RowKey::packed()` = event << 20 | fp: single Termine hidden by an eye button.
    #[serde(with = "folia_pack::set")]
    pub hidden_rows: Vec<u64>,
    /// `RowKey::packed()` of made choices („Nur diesen"): the option holding the row is the one
    /// shown.
    #[serde(with = "folia_pack::set")]
    pub chosen_rows: Vec<u64>,
    /// 0 derive from the modules, 1 Cottbus, 2 Senftenberg, 3 both (`TownChoice::code`); a value
    /// a reader does not know reads as 0.
    pub town: u8,
}

impl Subscription {
    /// The code of the subscription, at most `MAX_CODE` characters of `folia_pack::ALPHABET`.
    ///
    /// `Err(folia_pack::Error::TooLong)` when the code would be longer: too much is hidden for one
    /// address, and the page says so. `Err(folia_pack::Error::Malformed)` for a subscription that no
    /// reader takes (no module, a semester outside 2000–2099, more modules or entries than the
    /// caps), so no code comes out that `from_code` would refuse.
    pub fn code(&self) -> Result<String, folia_pack::Error> {
        let code = folia_pack::to_versioned_code(KIND, VERSION, self)?;
        if code.len() > MAX_CODE {
            return Err(folia_pack::Error::TooLong);
        }
        if Self::from_code(&code).is_none() {
            return Err(folia_pack::Error::Malformed);
        }
        Ok(code)
    }

    /// The subscription of a code, or `None` for anything the feed must not serve: the wrong
    /// length or characters, failed check characters, another kind, another layout than
    /// `VERSION`, a code with a field this build does not know (pack's `Trailing`), a semester
    /// outside 2000–2099, no module, more than `MAX_MODULES` modules or more than `MAX_HIDDEN`
    /// entries in a list.
    ///
    /// Within the known fields it reads what a newer build may write: kind bits no `EventKind`
    /// has are masked, a town number it does not know becomes 0 (derive), and a program that is
    /// no program id none.
    pub fn from_code(code: &str) -> Option<Subscription> {
        // The cheap checks first: the gate runs this on every request under `/calendar/`.
        let shaped = (1..=MAX_CODE).contains(&code.len()) && code.bytes().all(is_code_byte);
        if !shaped {
            return None;
        }
        let mut subscription: Subscription = folia_pack::from_versioned_code(KIND, VERSION, code).ok()?;
        SemesterKey::from_index(subscription.semester)?;
        let within = (1..=MAX_MODULES).contains(&subscription.modules.len())
            && [subscription.hidden_events.len(), subscription.hidden_rows.len(), subscription.chosen_rows.len()].iter().all(|len| *len <= MAX_HIDDEN);
        if !within {
            return None;
        }
        subscription.hidden_kinds = KindSet(subscription.hidden_kinds).known().0;
        subscription.town = TownChoice::from_code(subscription.town).code();
        subscription.program = subscription.program.filter(|id| is_program_id(id));
        Some(subscription)
    }

    /// The semester, or `None` for an index no key has.
    pub fn key(&self) -> Option<SemesterKey> {
        SemesterKey::from_index(self.semester)
    }

    /// The planned modules' ids as the catalog writes them, ascending and each once, as a code
    /// carries them (whatever order a subscription made in the app holds them in).
    pub fn module_ids(&self) -> Vec<String> {
        let mut ids = self.modules.clone();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter().map(|id| id.to_string()).collect()
    }

    /// What the visitor chose to see, as the timetable applies it. Kind bits and row keys this
    /// build does not know are left out; an unknown town is `Derive`, from every module of the
    /// code (a town the page derived from the imported modules alone comes as the town it is,
    /// `export::town_of`).
    pub fn selection(&self) -> Selection {
        let rows = |packed: &[u64]| packed.iter().filter_map(|value| RowKey::unpack(*value)).collect();
        Selection {
            hidden_kinds: KindSet(self.hidden_kinds).known(),
            hidden_events: self.hidden_events.iter().copied().collect(),
            hidden_rows: rows(&self.hidden_rows),
            chosen_rows: rows(&self.chosen_rows),
            town: TownChoice::from_code(self.town),
            town_from: None,
        }
    }
}

/// The address of a code: `/calendar/<code>.ics`, the feed in German; `Locale::path` makes it
/// another language's (`/en/calendar/<code>.ics`).
pub fn path(code: &str) -> String {
    format!("{CALENDAR_PREFIX}{code}{SUFFIX}")
}

/// The code of a feed's path, when the path has the shape: `CALENDAR_PREFIX`, 1 to `MAX_CODE`
/// characters of `folia_pack::ALPHABET`, `.ics`, nothing else. `%HH` escapes of alphabet characters are
/// decoded first (RFC 3986 §6.2.2.2: `%7E` is `~`, and axum's `Path` reads it so), so the gate and
/// the handler agree on what a path names; any other escape (`%2F`, `%20`) fails. The shape only:
/// `/calendar/a.ics.ics` has it (code `a.ics`, since `.` is in the alphabet), and whether a code
/// decodes is `Subscription::from_code`'s question. A language's prefix comes first where it has
/// one (`/en/calendar/<code>.ics`, the feed in English; `Locale::split`), and names the same code.
pub fn code_of_path(path: &str) -> Option<String> {
    let rest = Locale::split(path).1.strip_prefix(CALENDAR_PREFIX)?;
    // An escape spells one character in three bytes: anything longer cannot have the shape.
    if rest.len() > 3 * (MAX_CODE + SUFFIX.len()) {
        return None;
    }
    let mut file = String::with_capacity(rest.len());
    let mut bytes = rest.bytes();
    while let Some(byte) = bytes.next() {
        let byte = match byte {
            b'%' => (hex(bytes.next()?)? << 4) | hex(bytes.next()?)?,
            byte => byte,
        };
        if !is_code_byte(byte) {
            return None;
        }
        file.push(char::from(byte));
    }
    let code = file.strip_suffix(SUFFIX)?;
    (1..=MAX_CODE).contains(&code.len()).then(|| code.to_string())
}

/// What the gate lets through without the password: a path of the shape whose code decodes. The
/// check characters turn garbage away before any handler runs; calendar services send no cookie.
pub fn is_feed_path(path: &str) -> bool {
    code_of_path(path).and_then(|code| Subscription::from_code(&code)).is_some()
}

/// The path as Folia's access log writes it: every path under `CALENDAR_PREFIX` becomes
/// `/calendar/….ics`, valid or not, because a code names the modules someone plans and what they
/// hide. So does every path that has it further on: the feed in English (`/en/calendar/…`), and
/// what is no address of the site but reaches the log all the same (`/de/calendar/…` is
/// redirected, `/en/en/calendar/…` not found). Every other path is left as it is.
pub fn redacted_path(path: &str) -> &str {
    if path.contains(CALENDAR_PREFIX) {
        REDACTED
    } else {
        path
    }
}

/// Whether a byte is a character of a code (`folia_pack::ALPHABET`, all ASCII).
fn is_code_byte(byte: u8) -> bool {
    folia_pack::ALPHABET.as_bytes().contains(&byte)
}

/// The value of a hex digit, either case.
fn hex(byte: u8) -> Option<u8> {
    char::from(byte).to_digit(16).and_then(|value| u8::try_from(value).ok())
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::kind::EventKind;
    use crate::select::Town;
    use super::*;

    /// Informatik B.Sc., FS1 in WiSe 2026/27: the four planned modules, the Sachsendorf lecture
    /// hidden and „Nur diesen" on the Monday 17:30 Übung of 148369.
    fn first_semester() -> Subscription {
        Subscription {
            semester: 4053,
            modules: vec![11112, 12102, 12104, 12107],
            program: Some("079-82-2008".into()),
            hidden_kinds: 0,
            hidden_events: vec![149_408],
            hidden_rows: vec![],
            chosen_rows: vec![RowKey { event: 148_369, fp: 0xa4d12 }.packed()],
            town: 0,
        }
    }

    /// Subscribed codes: never change. Calendars subscribe to this address; if the test fails,
    /// the layout moved, and every published subscription with it. Printed by the first run of
    /// layout 1 (2026-09-25) and pinned.
    pub(crate) const FIRST_SEMESTER_CODE: &str = "b3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0";

    /// Every field set and none equal to another, so that moving or retyping any of them changes
    /// the code: the first plan's zeros would not notice two of them swapped.
    fn every_field() -> Subscription {
        Subscription {
            semester: 4054,
            modules: vec![11107, 12104, 13693],
            program: Some("G29-82-2025".into()),
            hidden_kinds: KindSet::default().with(EventKind::Tutorial).with(EventKind::Exam).0,
            hidden_events: vec![148_130, 150_349],
            hidden_rows: vec![RowKey { event: 148_369, fp: 0xaaf38 }.packed()],
            chosen_rows: vec![RowKey { event: 148_304, fp: 0x467cf }.packed(), RowKey { event: 148_370, fp: 0xf09d2 }.packed()],
            town: 2,
        }
    }

    /// Subscribed codes: never change (see `FIRST_SEMESTER_CODE`).
    const EVERY_FIELD_CODE: &str = "Zy3_j2JR3ZOvpql-PMBgMDxxl3eoec6JV8.GhPQYxLyjEOKK7RqcT_W4PRm.HL6R";

    #[test]
    fn the_code_of_a_plan_is_frozen() {
        assert_eq!(first_semester().code().unwrap(), FIRST_SEMESTER_CODE, "subscribed codes: never change");
        assert_eq!(every_field().code().unwrap(), EVERY_FIELD_CODE, "subscribed codes: never change");
        assert_eq!(Subscription::from_code(EVERY_FIELD_CODE), Some(every_field()));
        assert_eq!(Subscription::from_code(FIRST_SEMESTER_CODE), Some(first_semester()));
        assert!(is_feed_path(&path(FIRST_SEMESTER_CODE)));
    }

    #[test]
    fn codes_round_trip() {
        let plan = first_semester();
        let code = plan.code().unwrap();
        assert_eq!(Subscription::from_code(&code), Some(plan.clone()));
        assert_eq!(code_of_path(&path(&code)).as_deref(), Some(code.as_str()));
        let every = Subscription {
            semester: SemesterKey::new(2027, false).unwrap().index(),
            modules: (0..60).map(|n| 11_101 + n * 37).collect(),
            program: Some("013-D8-2022".into()),
            hidden_kinds: KindSet::default().with(EventKind::Tutorial).with(EventKind::Exam).0,
            hidden_events: vec![148_130, 149_408, 150_349],
            hidden_rows: vec![RowKey { event: 148_369, fp: 0xaaf38 }.packed(), RowKey { event: 148_370, fp: 0x00001 }.packed()],
            chosen_rows: vec![RowKey { event: 148_304, fp: 0xfffff }.packed()],
            town: 2,
        };
        assert_eq!(Subscription::from_code(&every.code().unwrap()), Some(every.clone()));
        for town in 0..=3 {
            let chosen = Subscription { town, ..first_semester() };
            assert_eq!(Subscription::from_code(&chosen.code().unwrap()).map(|s| s.town), Some(town));
        }
        // The set fields keep neither order nor twins: the code carries each id once, ascending.
        let shuffled = Subscription { modules: vec![12107, 11112, 12104, 12102, 12104], ..first_semester() };
        assert_eq!(shuffled.code(), plan.code());
        assert_eq!(shuffled.module_ids(), ["11112", "12102", "12104", "12107"]);
    }

    #[test]
    fn what_a_code_selects() {
        let plan = Subscription {
            hidden_kinds: KindSet::default().with(EventKind::Tutorial).0 | 1 << 13,
            hidden_rows: vec![RowKey { event: 148_369, fp: 0xaaf38 }.packed(), 0xfffff],
            town: 1,
            ..first_semester()
        };
        assert_eq!(plan.key(), SemesterKey::parse("2026W"));
        let selection = plan.selection();
        assert_eq!(selection.hidden_kinds, KindSet::default().with(EventKind::Tutorial));
        assert_eq!(selection.hidden_events.into_iter().collect::<Vec<_>>(), [149_408]);
        // A packed key of event 0 is no key.
        assert_eq!(selection.hidden_rows.into_iter().collect::<Vec<_>>(), [RowKey { event: 148_369, fp: 0xaaf38 }]);
        assert_eq!(selection.chosen_rows.into_iter().collect::<Vec<_>>(), [RowKey { event: 148_369, fp: 0xa4d12 }]);
        assert_eq!(selection.town, TownChoice::Only(Town::Cottbus));
        assert_eq!(Subscription { town: 9, ..first_semester() }.selection().town, TownChoice::Derive);
        assert_eq!(Subscription { semester: 0, ..first_semester() }.key(), None);
    }

    /// Codes of the right kind that the feed must still refuse, written past `code()`'s checks.
    #[test]
    fn codes_beyond_the_caps_are_refused() {
        let raw = |subscription: &Subscription| folia_pack::to_versioned_code(KIND, VERSION, subscription).unwrap();
        let consecutive = |n: u32| -> Vec<u32> { (0..n).map(|i| 140_000 + i).collect() };
        let packed = |n: u32| -> Vec<u64> { (0..n).map(|i| RowKey { event: 148_369, fp: i }.packed()).collect() };
        let refused = [
            Subscription { semester: 0, ..first_semester() },
            Subscription { semester: 3999, ..first_semester() },
            Subscription { semester: 4200, ..first_semester() },
            Subscription { modules: vec![], ..first_semester() },
            Subscription { modules: consecutive(61), ..first_semester() },
            Subscription { hidden_events: consecutive(501), ..first_semester() },
            Subscription { hidden_rows: packed(501), ..first_semester() },
            Subscription { chosen_rows: packed(501), ..first_semester() },
        ];
        for subscription in refused {
            let code = raw(&subscription);
            assert!(code.len() <= MAX_CODE, "{} characters: the cap, not the length, must refuse it", code.len());
            assert_eq!(Subscription::from_code(&code), None, "{subscription:?}");
            assert_eq!(subscription.code(), Err(folia_pack::Error::Malformed), "{subscription:?}");
        }
        // At the caps, a code is still read.
        let full = Subscription { modules: consecutive(60), hidden_events: consecutive(500), hidden_rows: packed(500), chosen_rows: packed(500), ..first_semester() };
        let code = full.code().unwrap();
        assert_eq!(Subscription::from_code(&code), Some(full));
    }

    #[test]
    fn other_codes_are_refused() {
        let plan = first_semester();
        let code = plan.code().unwrap();
        // The Merkliste's kind, with the very same value.
        assert_eq!(Subscription::from_code(&folia_pack::to_versioned_code("bookmarks", VERSION, &plan).unwrap()), None);
        // A later layout than this build reads.
        assert_eq!(Subscription::from_code(&folia_pack::to_versioned_code(KIND, VERSION + 1, &plan).unwrap()), None);
        // A newer build's code with a field appended that is set: pack refuses what it cannot read.
        assert_eq!(Subscription::from_code(&folia_pack::to_versioned_code(KIND, VERSION, &(plan.clone(), 1u8)).unwrap()), None);
        // … and reads it when the new field is zero, as it is when absent.
        assert_eq!(Subscription::from_code(&folia_pack::to_versioned_code(KIND, VERSION, &(plan.clone(), 0u8)).unwrap()), Some(plan.clone()));
        // A character changed, the code cut short, characters no code has, the wrong lengths.
        let mut changed = code.clone().into_bytes();
        changed[0] = if changed[0] == b'A' { b'B' } else { b'A' };
        assert_eq!(Subscription::from_code(std::str::from_utf8(&changed).unwrap()), None);
        assert_eq!(Subscription::from_code(&code[..code.len() - 1]), None);
        assert_eq!(Subscription::from_code(&format!("{code} ")), None);
        assert_eq!(Subscription::from_code(&format!("{code}/")), None);
        assert_eq!(Subscription::from_code(""), None);
        assert_eq!(Subscription::from_code(&"A".repeat(MAX_CODE + 1)), None);
        assert_eq!(Subscription::from_code("x"), None);
    }

    #[test]
    fn a_reader_tolerates_what_a_newer_writer_sets() {
        let plan = first_semester();
        // A kind this build does not know yet (bit 13) is masked, the known ones kept.
        let written = |subscription: &Subscription| folia_pack::to_versioned_code(KIND, VERSION, subscription).unwrap();
        let newer = Subscription { hidden_kinds: 1 << 13 | EventKind::Tutorial.bit(), ..plan.clone() };
        let read = Subscription::from_code(&written(&newer)).unwrap();
        assert_eq!(read.hidden_kinds, EventKind::Tutorial.bit());
        // A town number this build does not know reads as „derive".
        let newer = Subscription { town: 7, ..plan.clone() };
        assert_eq!(Subscription::from_code(&written(&newer)).map(|s| s.town), Some(0));
        // A program id of a shape this build does not know reads as none: each module's own
        // abbreviation.
        for odd in ["079_82_2008", "", "079-82-2008-1-2-3", "Informatik"] {
            let newer = Subscription { program: Some(odd.into()), ..plan.clone() };
            assert_eq!(Subscription::from_code(&written(&newer)).map(|s| s.program), Some(None), "{odd:?}");
        }
        // A code written before the later fields existed reads with them absent.
        #[derive(Serialize)]
        struct FirstTwo {
            semester: u16,
            #[serde(with = "folia_pack::set")]
            modules: Vec<u32>,
        }
        let old = folia_pack::to_versioned_code(KIND, VERSION, &FirstTwo { semester: 4053, modules: vec![12104, 11112] }).unwrap();
        assert_eq!(Subscription::from_code(&old), Some(Subscription { semester: 4053, modules: vec![11112, 12104], ..Subscription::default() }));
    }

    /// 400 hidden Termine of a real semester's spread: far past 1,024 characters, so `code()`
    /// refuses and the page says „Zu viel ausgeblendet für ein Abo." instead of handing out an
    /// address the server turns away.
    #[test]
    fn too_much_hidden_is_too_long() {
        let mut seed: u64 = 0x2026_0924;
        let mut next = || {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            seed >> 33
        };
        let rows: Vec<u64> = (0..400).map(|_| RowKey { event: 143_216 + (next() % 10_000) as u32, fp: (next() & 0xfffff) as u32 }.packed()).collect();
        let plan = Subscription { hidden_rows: rows, ..first_semester() };
        assert_eq!(plan.code(), Err(folia_pack::Error::TooLong));
    }

    #[test]
    fn the_shape_of_a_feed_path() {
        assert_eq!(code_of_path("/calendar/Ab-_.~9.ics").as_deref(), Some("Ab-_.~9"));
        assert_eq!(code_of_path("/calendar/%7E9.ics").as_deref(), Some("~9"));
        assert_eq!(code_of_path("/calendar/%7e9%2Eics").as_deref(), Some("~9"));
        assert_eq!(code_of_path("/calendar/a.ics.ics").as_deref(), Some("a.ics"));
        assert_eq!(code_of_path(&format!("/calendar/{}.ics", "A".repeat(MAX_CODE))).map(|c| c.len()), Some(MAX_CODE));
        for bad in [
            "/calendar/".to_string(),
            "/calendar/.ics".to_string(),
            "/calendar/a b.ics".to_string(),
            "/calendar/a%20b.ics".to_string(),
            "/calendar/%2F.ics".to_string(),
            "/calendar/%.ics".to_string(),
            "/calendar/%7.ics".to_string(),
            "/calendar/%GG.ics".to_string(),
            "/calendar/a/b.ics".to_string(),
            "/calendar/a.ics/".to_string(),
            "/calendar/a".to_string(),
            "/calendar/a.ICS".to_string(),
            "/calendar/ä.ics".to_string(),
            format!("/calendar/{}.ics", "A".repeat(MAX_CODE + 1)),
            format!("/calendar/{}.ics", "%41".repeat(MAX_CODE + 1)),
            "/calendarx/a.ics".to_string(),
            "/Calendar/a.ics".to_string(),
            "calendar/a.ics".to_string(),
        ] {
            assert_eq!(code_of_path(&bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_gate_opens_for_codes_that_decode() {
        let code = first_semester().code().unwrap();
        assert!(is_feed_path(&path(&code)));
        // The first character escaped, as a client may write it.
        let (first, rest) = code.split_at(1);
        let escaped = format!("/calendar/%{:02X}{rest}.ics", first.as_bytes()[0]);
        assert!(is_feed_path(&escaped), "{escaped}");
        assert!(!is_feed_path("/calendar/a.ics.ics"));
        assert!(!is_feed_path("/calendar/x.ics"));
        assert!(!is_feed_path("/calendar/abc"));
        assert!(!is_feed_path(&format!("/calendar/{code}")));
        assert!(!is_feed_path(&format!("/calendar/{code}.ics/")));
        assert!(!is_feed_path(&path(&folia_pack::to_versioned_code("bookmarks", VERSION, &first_semester()).unwrap())));
    }

    #[test]
    fn the_log_keeps_no_code() {
        assert_eq!(redacted_path("/calendar/anything"), "/calendar/….ics");
        assert_eq!(redacted_path(&path(&first_semester().code().unwrap())), "/calendar/….ics");
        assert_eq!(redacted_path("/calendar/"), "/calendar/….ics");
        assert_eq!(redacted_path("/calendar/a/b%2F.ics?x=1"), "/calendar/….ics");
        assert_eq!(redacted_path("/catalog/module/12104"), "/catalog/module/12104");
        assert_eq!(redacted_path("/calendar"), "/calendar");
        assert_eq!(redacted_path("/"), "/");
    }

    #[test]
    fn the_feed_in_english_is_the_same_code() {
        let code = first_semester().code().unwrap();
        let english = Locale::En.path(&path(&code));
        assert_eq!(english, format!("/en/calendar/{code}.ics"));
        assert_eq!(code_of_path(&english), Some(code.clone()));
        assert!(is_feed_path(&english));
        assert_eq!(code_of_path("/en/calendar/%7E9.ics").as_deref(), Some("~9"));
        // Only a language of the site: `/de/…` is no address of it, and neither is a prefix glued on.
        for other in [format!("/de/calendar/{code}.ics"), format!("/fr/calendar/{code}.ics"), format!("/encalendar/{code}.ics"), format!("/en/en/calendar/{code}.ics")] {
            assert_eq!(code_of_path(&other), None, "{other}");
            assert!(!is_feed_path(&other), "{other}");
        }
    }

    #[test]
    fn the_log_keeps_no_code_in_any_language() {
        let code = first_semester().code().unwrap();
        for path in [
            Locale::En.path(&path(&code)),
            "/en/calendar/anything".to_string(),
            "/en/calendar/".to_string(),
            "/en/calendar/a/b%2F.ics?x=1".to_string(),
            // No address of the site, but a request for it reaches the log all the same.
            format!("/de/calendar/{code}.ics"),
            format!("/en/en/calendar/{code}.ics"),
            format!("//calendar/{code}.ics"),
        ] {
            assert_eq!(redacted_path(&path), "/calendar/….ics", "{path}");
        }
        for kept in ["/en", "/en/", "/en/calendar", "/en/catalog/module/12104", "/en/programs/calendar", "/en/calendarx/a.ics"] {
            assert_eq!(redacted_path(kept), kept);
        }
    }
}
