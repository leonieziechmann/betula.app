//! What a profile shares and what it follows (Betula Sync, step 3; folia/design/teilen/teilen.html,
//! owner 2026-11-08), as lines of the Stundenplan's text, so that both travel with the profile to
//! each of its devices (`folia_sync`): every device renews the parts of what is shared when a
//! calendar changes, and every device follows what was shared with the profile.
//!
//! ```text
//! s  <id>  <calendar>  <to>  <semester>  <made>  <flags>  <key>  <parcel>  <program>  <name>
//! f  <calendar>  <link>  <since>  <ended>
//! ```
//!
//! `s` is a share in force: its id (the part's while it stays current, else the one-time link's
//! parcel), the calendar it shares (0 the plan's, „Mein Studiengang"), for a person (`p`) or a
//! calendar app (`a`), the semester it was made for (one that stays current goes on with the next,
//! `folia_calendar::live::following`), when it was made, what it is (`l` stays current, `1` one
//! person, `o` with the Termine of one's own, `n` with the notes), the part's key, the waiting
//! parcel's id and key, the program whose abbreviations name the modules, and the name „Für wen"
//! (for the list alone: it never travels in a link or to the server). A share of a fixed copy for
//! everybody with the link is no line: it is the link of today, which nothing can take back.
//!
//! `f` makes a calendar of one's own follow a share of someone else's: its link (the part's id
//! and key), since when, and since when it is not shared any more (0 while it is). The calendar
//! keeps what it showed last; „Behalten" takes the line away, „Löschen" the calendar.
//!
//! A share of a calendar of one's own and a calendar's follow go with the calendar. A build before
//! them keeps the lines as lines of a tag it does not know.

use std::fmt::Write as _;

use folia_calendar::live::LivePart;
use folia_calendar::semester::SemesterKey;
use folia_routes::url;

use crate::studyplan::{Own, PlanDoc, MAX_CALENDAR_ID};

/// The most shares in force: the parts Phloem keeps of a profile.
pub const MAX_SHARES: usize = 32;

/// The most characters of the name „Für wen".
pub const MAX_SHARE_NAME: usize = 40;

/// The bytes of a part's or a parcel's id, of a part's key, and of a link (id and key).
pub const ID_BYTES: usize = 16;
pub const KEY_BYTES: usize = 32;
pub const LINK_BYTES: usize = ID_BYTES + KEY_BYTES;

/// Whom a share is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShareTo {
    /// A person, by a link they open in Betula.
    Person,
    /// A calendar app, by the address it subscribes to.
    App,
}

impl ShareTo {
    pub fn code(self) -> &'static str {
        match self {
            ShareTo::Person => "p",
            ShareTo::App => "a",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "p" => Some(ShareTo::Person),
            "a" => Some(ShareTo::App),
            _ => None,
        }
    }
}

/// A share in force (`s`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Share {
    /// The part's id while it stays current, else the parcel's (`ID_BYTES`, base64url).
    pub id: String,
    /// 0 the plan's calendar, else a calendar of one's own.
    pub calendar: u8,
    pub to: ShareTo,
    /// The semester it was made for.
    pub semester: SemesterKey,
    /// When it was made, seconds since 1970.
    pub made: u64,
    /// It stays current: a part at Phloem, renewed with every change.
    pub live: bool,
    /// One person: a link that opens once (a parcel that waits up to 7 days).
    pub once: bool,
    /// The Termine of one's own come along.
    pub own: bool,
    /// The notes at Termine come along.
    pub notes: bool,
    /// The part's key (`KEY_BYTES`, base64url) while it stays current; empty for a copy.
    pub key: String,
    /// The waiting parcel's id and key (`LINK_BYTES`, base64url); empty once it was opened, or
    /// for a share for everybody with the link.
    pub parcel: String,
    /// The program whose abbreviations name the modules.
    pub program: Option<String>,
    /// „Für wen": for the list alone.
    pub name: String,
}

/// What a calendar of one's own follows: a share of someone else's (`f`, after the calendar's
/// lines; it goes with the calendar, and comes back with it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Follow {
    /// The part's id and key (`LINK_BYTES`, base64url).
    pub link: String,
    /// When it was followed, seconds since 1970.
    pub since: u64,
    /// Since when it is not shared any more; 0 while it is.
    pub ended: u64,
}

impl Follow {
    /// The part's id and its key (both base64url), from the link.
    pub fn part(&self) -> Option<(String, String)> {
        split_link(&self.link)
    }
}

/// Whether `text` is base64url without padding of `bytes` bytes.
pub fn is_b64(text: &str, bytes: usize) -> bool {
    let len = (bytes * 8).div_ceil(6);
    text.len() == len && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The id and the key of a link (`LINK_BYTES` in base64url), each in base64url: the first 16
/// bytes and the last 32, cut at the characters (16 bytes are 21⅓ characters, so the cut takes the
/// bits apart; this decodes and encodes again).
pub fn split_link(link: &str) -> Option<(String, String)> {
    if !is_b64(link, LINK_BYTES) {
        return None;
    }
    let bytes = decode(link)?;
    let (id, key) = bytes.split_at_checked(ID_BYTES)?;
    Some((encode(id), encode(key)))
}

/// The link of a part's id and key (each base64url).
pub fn join_link(id: &str, key: &str) -> Option<String> {
    let mut bytes = decode(id).filter(|b| b.len() == ID_BYTES)?;
    bytes.extend(decode(key).filter(|b| b.len() == KEY_BYTES)?);
    Some(encode(&bytes))
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Base64url without padding.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk.first().copied().unwrap_or(0), chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let chars = chunk.len() + 1;
        for i in 0..chars {
            let index = ((n >> (18 - 6 * i)) & 63) as usize;
            out.push(char::from(ALPHABET.get(index).copied().unwrap_or(b'A')));
        }
    }
    out
}

/// Base64url without padding, strictly: `None` for anything else, or bits left over.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| ALPHABET.iter().position(|a| *a == c).map(|p| p as u32);
    let bytes = text.as_bytes();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= value(*c)? << (18 - 6 * i);
        }
        let take = chunk.len() - 1;
        for i in 0..take {
            out.push(((n >> (16 - 8 * i)) & 0xff) as u8);
        }
        // The bits after the last byte are zero in a strict text.
        let used = take * 8;
        let bits = chunk.len() * 6;
        if bits > used && n & ((1 << (24 - used)) - 1) & !((1 << (24 - bits)) - 1) != 0 {
            return None;
        }
    }
    Some(out)
}

/// The name „Für wen" as a line keeps it: control characters as spaces, trimmed, at most
/// `MAX_SHARE_NAME` characters.
pub fn share_name(name: &str) -> String {
    let spaced: String = name.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    spaced.trim().chars().take(MAX_SHARE_NAME).collect::<String>().trim_end().to_string()
}

fn number(text: &str) -> Option<u64> {
    (!text.is_empty() && text.len() <= 20 && text.bytes().all(|b| b.is_ascii_digit())).then(|| text.parse().ok()).flatten()
}

impl Share {
    /// Whether it is one a line keeps: a share that stays current has a key, one for one person a
    /// parcel while it waits, one for a calendar app stays current and is for nobody once.
    pub fn is_valid(&self) -> bool {
        let id = is_b64(&self.id, ID_BYTES);
        let key = if self.live { is_b64(&self.key, KEY_BYTES) } else { self.key.is_empty() };
        let parcel = self.parcel.is_empty() || (self.once && is_b64(&self.parcel, LINK_BYTES));
        let what = match self.to {
            ShareTo::Person => self.live || self.once,
            ShareTo::App => self.live && !self.once,
        };
        // A copy for one person is a parcel and nothing else: once opened, there is nothing left.
        let copy = self.live || !self.parcel.is_empty();
        id && key && parcel && what && copy && self.calendar <= MAX_CALENDAR_ID && self.program.as_deref().is_none_or(url::is_program_id)
    }

    fn flags(&self) -> String {
        [(self.live, 'l'), (self.once, '1'), (self.own, 'o'), (self.notes, 'n')].iter().filter(|(on, _)| *on).map(|(_, c)| *c).collect()
    }

    /// Its line.
    pub fn line(&self) -> String {
        let mut out = String::new();
        let _ = write!(
            out,
            "s\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.id,
            self.calendar,
            self.to.code(),
            self.semester.key(),
            self.made,
            self.flags(),
            self.key,
            self.parcel,
            self.program.as_deref().unwrap_or_default(),
            share_name(&self.name)
        );
        out
    }

    /// The share of a line's fields (the tag first), when it is one.
    pub fn of(fields: &[&str]) -> Option<Share> {
        let field = |i: usize| fields.get(i).copied().unwrap_or_default();
        let flags = field(6);
        let share = Share {
            id: field(1).to_string(),
            calendar: number(field(2)).and_then(|n| u8::try_from(n).ok())?,
            to: ShareTo::from_code(field(3))?,
            semester: SemesterKey::parse(field(4))?,
            made: number(field(5)).unwrap_or(0),
            live: flags.contains('l'),
            once: flags.contains('1'),
            own: flags.contains('o'),
            notes: flags.contains('n'),
            key: field(7).to_string(),
            parcel: field(8).to_string(),
            program: Some(field(9)).filter(|p| !p.is_empty()).map(str::to_string),
            name: share_name(field(10)),
        };
        share.is_valid().then_some(share)
    }

    /// The link of its part (id and key), while it stays current.
    pub fn link(&self) -> Option<String> {
        self.live.then(|| join_link(&self.id, &self.key)).flatten()
    }
}

impl Follow {
    /// Its line, for calendar `calendar`.
    pub fn line(&self, calendar: u8) -> String {
        format!("f\t{calendar}\t{}\t{}\t{}", self.link, self.since, self.ended)
    }

    /// The calendar and what it follows, of a line's fields (the tag first).
    pub fn of(fields: &[&str]) -> Option<(u8, Follow)> {
        let field = |i: usize| fields.get(i).copied().unwrap_or_default();
        let calendar = number(field(1)).and_then(|n| u8::try_from(n).ok()).filter(|id| (1..=MAX_CALENDAR_ID).contains(id))?;
        let follow = Follow { link: field(2).to_string(), since: number(field(3)).unwrap_or(0), ended: number(field(4)).unwrap_or(0) };
        is_b64(&follow.link, LINK_BYTES).then_some((calendar, follow))
    }
}

impl PlanDoc {
    /// A share in force by its id.
    pub fn share(&self, id: &str) -> Option<&Share> {
        self.shares.iter().find(|share| share.id == id)
    }

    /// Adds a share, the last of them; `false` where its calendar is not there, its id is taken,
    /// it is no share or the cap is reached.
    pub fn add_share(&mut self, share: Share) -> bool {
        let calendar = share.calendar == 0 || self.calendar(share.calendar).is_some();
        if !calendar || !share.is_valid() || self.share(&share.id).is_some() || self.shares.len() >= MAX_SHARES {
            return false;
        }
        self.shares.push(Share { name: share_name(&share.name), ..share });
        true
    }

    /// Takes a share away („Nicht mehr teilen", „Zurückziehen"); it, where it was.
    pub fn remove_share(&mut self, id: &str) -> Option<Share> {
        let at = self.shares.iter().position(|share| share.id == id)?;
        Some(self.shares.remove(at))
    }

    /// Changes a share in place by `f` (a parcel opened, the semester gone on); `false` where
    /// nothing changed or what `f` made is no share.
    pub fn update_share(&mut self, id: &str, f: impl FnOnce(&mut Share)) -> bool {
        let Some(share) = self.shares.iter_mut().find(|share| share.id == id) else { return false };
        let mut changed = share.clone();
        f(&mut changed);
        if changed == *share || !changed.is_valid() || changed.id != share.id || changed.calendar != share.calendar {
            return false;
        }
        *share = changed;
        true
    }

    /// The shares of calendar `calendar`, in the order they were made.
    pub fn shares_of(&self, calendar: u8) -> Vec<&Share> {
        self.shares.iter().filter(|share| share.calendar == calendar).collect()
    }

    /// What calendar `calendar` follows, where it follows a share.
    pub fn follow_of(&self, calendar: u8) -> Option<&Follow> {
        self.calendar(calendar).and_then(|c| c.follow.as_ref())
    }

    /// The calendars that follow a share, with what they follow.
    pub fn follows(&self) -> Vec<(u8, &Follow)> {
        self.calendars.iter().filter_map(|c| Some((c.id, c.follow.as_ref()?))).collect()
    }

    /// Makes a calendar of one's own follow a share's link; `false` where the calendar is not
    /// there, it follows one already, or the link is none.
    pub fn follow(&mut self, calendar: u8, link: &str, since: u64) -> bool {
        let Some(cal) = self.calendars.iter_mut().find(|c| c.id == calendar) else { return false };
        if cal.follow.is_some() || !is_b64(link, LINK_BYTES) {
            return false;
        }
        cal.follow = Some(Follow { link: link.to_string(), since, ended: 0 });
        true
    }

    /// The share a calendar follows is not shared any more (since `at`): the calendar keeps what
    /// it showed last. `false` where nothing changed.
    pub fn end_follow(&mut self, calendar: u8, at: u64) -> bool {
        match self.calendars.iter_mut().find(|c| c.id == calendar).and_then(|c| c.follow.as_mut()) {
            Some(follow) if follow.ended == 0 => {
                follow.ended = at.max(1);
                true
            }
            _ => false,
        }
    }

    /// „Behalten": the calendar follows nothing any more and stays as it is.
    pub fn unfollow(&mut self, calendar: u8) -> bool {
        self.calendars.iter_mut().find(|c| c.id == calendar).is_some_and(|c| c.follow.take().is_some())
    }

    /// The part a share holds now, from the plan as it is: one semester of its calendar (the
    /// semester it shows today, `semester`), with what it takes along. A share for a person
    /// carries nothing of what is hidden.
    pub fn live_part(&self, share: &Share, semester: SemesterKey) -> LivePart {
        use folia_calendar::live::Along;
        use folia_calendar::select::TownChoice;
        let along = Along { hidden: share.to == ShareTo::App, own: share.own, notes: share.notes };
        if share.calendar == 0 {
            let selection = self.selection(semester, TownChoice::Derive);
            return LivePart::of(semester, &self.week_in(semester), &self.visits_in(semester), share.program.as_deref(), &selection, along);
        }
        match self.calendar(share.calendar) {
            Some(calendar) => {
                let mut selection = calendar.selection(semester);
                selection.notes = calendar.notes.iter().filter(|((s, _), _)| *s == semester).map(|((_, row), text)| (*row, text.clone())).collect();
                LivePart::of(semester, &calendar.modules_in(semester), &[], calendar.program.as_deref(), &selection, along)
            }
            None => LivePart { semester: semester.index(), ..LivePart::default() },
        }
    }

    /// What a following calendar gets of a part: its modules, groups, changes, Termine of one's
    /// own and notes of the part's semester in place of what it had there, its program; the other
    /// semesters as they were (what the share showed before stays to look at). Its Termine of one's
    /// own keep their numbers where they stand at the same place, so that what names them stays.
    /// `false` where nothing changed.
    pub fn take_part(&mut self, calendar: u8, part: &LivePart) -> bool {
        let Some(s) = part.key() else { return false };
        let before = self.clone();
        let mut numbers: Vec<u32> = self.calendar(calendar).map(|c| c.own.iter().filter(|own| own.semester == s).map(|own| own.termin.number).collect()).unwrap_or_default();
        let termine = part.own_termine();
        // New numbers for the Termine beyond those that stand; never one of another calendar.
        while numbers.len() < termine.len() {
            let taken: std::collections::BTreeSet<u32> = self.own.iter().chain(self.calendars.iter().flat_map(|c| c.own.iter())).map(|own| own.termin.number).chain(numbers.iter().copied()).collect();
            let largest = taken.last().copied().unwrap_or(0);
            let Some(next) = crate::ids::after(crate::ids::Kind::Own, largest, folia_calendar::own::MAX_OWN_NUMBER, |n| !taken.contains(&n)) else { break };
            numbers.push(next);
        }
        let renumbered: Vec<(u32, u32)> = termine.iter().zip(numbers.iter()).map(|(termin, number)| (termin.number, *number)).collect();
        let Some(cal) = self.calendars.iter_mut().find(|c| c.id == calendar) else { return false };
        cal.program = part.program.clone();
        cal.modules.retain(|(semester, _)| *semester != s);
        cal.chosen.retain(|(semester, _)| *semester != s);
        cal.changes.retain(|(semester, ..), _| *semester != s);
        cal.own.retain(|own| own.semester != s);
        cal.notes.retain(|(semester, _), _| *semester != s);
        for id in part.module_ids() {
            cal.add_module(s, &id);
        }
        for row in part.chosen_rows() {
            cal.set_tick(s, row, true);
        }
        for change in part.change_list() {
            cal.set_change(s, change);
        }
        for (termin, (_, number)) in termine.into_iter().zip(renumbered.iter()) {
            let termin = folia_calendar::own::OwnTermin { number: *number, ..termin };
            if termin.is_valid() && cal.own.len() < crate::studyplan::MAX_OWN {
                cal.own.push(Own { semester: s, termin });
            }
        }
        let key_of = |row: folia_calendar::rowkey::RowKey| match row.event.checked_sub(folia_calendar::own::OWN_EVENT_BASE) {
            Some(theirs) if row.fp == 0 => renumbered.iter().find(|(old, _)| *old == theirs).map(|(_, mine)| folia_calendar::rowkey::RowKey { event: folia_calendar::own::OWN_EVENT_BASE + mine, fp: 0 }),
            _ => Some(row),
        };
        for (row, text) in part.note_list() {
            if let Some(row) = key_of(row) {
                cal.set_note(s, row, &text);
            }
        }
        *self != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_goes_there_and_back_strictly() {
        for bytes in [&b""[..], b"f", b"fo", b"foo", b"foob", &[0xff; 16], &[0u8; 32], &[7u8; 48]] {
            let text = encode(bytes);
            assert_eq!(decode(&text).as_deref(), Some(bytes), "{text}");
            assert!(!text.contains('='));
        }
        assert_eq!(encode(&[0xfb, 0xff]), "-_8");
        assert_eq!(decode("-_8").as_deref(), Some(&[0xfb, 0xff][..]));
        // Bits left over after the last byte, a character too many, another alphabet: nothing.
        for wrong in ["-_9", "A", "ab+/", "ab=="] {
            assert_eq!(decode(wrong), None, "{wrong}");
        }
        assert_eq!(encode(&[0u8; 16]).len(), 22);
        assert_eq!(encode(&[0u8; 48]).len(), 64);
    }

    use folia_calendar::kind::EventKind;
    use folia_calendar::own::{OwnRhythm, OwnTermin};
    use folia_calendar::rowkey::RowKey;

    fn key(s: &str) -> SemesterKey {
        SemesterKey::parse(s).unwrap()
    }

    fn share(n: u8, calendar: u8) -> Share {
        Share {
            id: encode(&[n; ID_BYTES]),
            calendar,
            to: ShareTo::Person,
            semester: key("2026W"),
            made: 1_794_000_000,
            live: true,
            once: true,
            own: true,
            notes: false,
            key: encode(&[n.wrapping_add(1); KEY_BYTES]),
            parcel: encode(&[n.wrapping_add(2); LINK_BYTES]),
            program: Some("079-82-2008".into()),
            name: "Lena\tvom Sport".into(),
        }
    }

    fn termin(number: u32, name: &str) -> OwnTermin {
        OwnTermin { number, rhythm: OwnRhythm::Week, weekday: 2, dates: vec![], from: 600, to: 720, period: None, module: None, name: name.into(), place: String::new(), skip: vec![] }
    }

    #[test]
    fn shares_and_follows_come_back_from_their_lines() {
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2026W"), "12104", 1, None));
        let lena = doc.add_calendar("Lena", None, key("2026W"), &["11109".to_string()], &[], &[]).unwrap();
        assert!(doc.add_share(share(1, 0)));
        assert!(doc.add_share(Share { to: ShareTo::App, once: false, parcel: String::new(), ..share(2, lena) }));
        assert!(doc.follow(lena, &encode(&[9; LINK_BYTES]), 1_794_000_100));
        assert!(doc.calendars[0].set_note(key("2026W"), RowKey { event: 148_701, fp: 0xb7025 }, "Folien\nvorher lesen"));
        let text = doc.stored_without_program();
        assert!(text.contains("\ns\t") && text.contains("\nf\t") && text.contains(&format!("\nb\t{lena}\t2026W\t")), "{text}");
        let back = PlanDoc::restored(&text);
        assert_eq!(back, doc);
        assert_eq!(back.share(&encode(&[1; ID_BYTES])).map(|s| s.name.as_str()), Some("Lena vom Sport"), "a name never splits its line");
        assert_eq!(back.follow_of(lena).map(|f| f.since), Some(1_794_000_100));
        assert_eq!(back.calendars[0].note_of(key("2026W"), RowKey { event: 148_701, fp: 0xb7025 }), Some("Folien vorher lesen"));
        // Taken away with its calendar, a share goes; what the calendar followed comes back with it.
        let mut gone = back.clone();
        let (at, calendar) = gone.remove_calendar(lena).unwrap();
        assert_eq!(gone.shares.len(), 1);
        assert!(gone.put_calendar(at, calendar));
        assert!(gone.follow_of(lena).is_some());
    }

    #[test]
    fn a_share_is_one_a_line_keeps_or_none() {
        let mut doc = PlanDoc::default();
        // A copy for everybody is no share; one for a calendar app stays current and opens often.
        for wrong in [
            Share { live: false, once: false, key: String::new(), parcel: String::new(), ..share(1, 0) },
            Share { to: ShareTo::App, ..share(1, 0) },
            Share { key: String::new(), ..share(1, 0) },
            Share { id: "short".into(), ..share(1, 0) },
            Share { live: false, key: String::new(), parcel: String::new(), ..share(1, 0) },
            share(1, 3),
        ] {
            assert!(!doc.add_share(wrong.clone()), "{wrong:?}");
        }
        // A copy for one person is its parcel while it waits.
        assert!(doc.add_share(Share { live: false, key: String::new(), ..share(1, 0) }));
        assert!(!doc.add_share(share(1, 0)), "one id once");
        for n in 2..=MAX_SHARES as u8 {
            assert!(doc.add_share(share(n, 0)), "{n}");
        }
        assert!(!doc.add_share(share(200, 0)), "32 at most");
        // Opened: the parcel goes, a share that stays current stays.
        assert!(doc.update_share(&encode(&[2; ID_BYTES]), |s| s.parcel.clear()));
        assert!(!doc.update_share(&encode(&[2; ID_BYTES]), |s| s.parcel.clear()), "nothing changed");
        assert!(!doc.update_share(&encode(&[1; ID_BYTES]), |s| s.parcel.clear()), "a copy without its parcel is nothing");
    }

    #[test]
    fn a_part_for_a_person_leaves_out_what_is_hidden_and_what_is_not_taken_along() {
        let w = key("2026W");
        let mut doc = PlanDoc::default();
        for id in ["12104", "12107"] {
            assert!(doc.plan(w, id, 1, None));
        }
        assert!(doc.visit(w, "13977", 1));
        doc.set_kind(w, EventKind::Tutorial, true);
        doc.set_event(w, 149_408, true);
        doc.choose(w, 148_369, Some(RowKey { event: 148_369, fp: 0xa4d12 }));
        assert!(doc.add_own(0, w, termin(0, "Arbeit")).is_some());
        assert!(doc.set_note(w, RowKey { event: 148_369, fp: 0xa4d12 }, "Folien"));
        let person = doc.live_part(&Share { own: false, notes: false, ..share(1, 0) }, w);
        assert_eq!(person.module_ids(), ["12104", "12107", "13977"]);
        assert_eq!(person.visits, [13977]);
        assert_eq!(person.chosen_rows(), [RowKey { event: 148_369, fp: 0xa4d12 }]);
        assert!(person.hidden_events.is_empty() && person.hidden_kinds == 0 && person.own.is_empty() && person.notes.is_empty());
        let app = doc.live_part(&Share { to: ShareTo::App, once: false, parcel: String::new(), own: true, notes: true, ..share(2, 0) }, w);
        assert_eq!((app.hidden_events.as_slice(), app.own.len(), app.notes.len()), (&[149_408][..], 1, 1));
        assert!(app.hidden_kinds != 0);
        // Another semester of the plan: what it holds there, nothing at all here.
        assert!(doc.live_part(&share(1, 0), key("2027S")).modules.is_empty());
    }

    #[test]
    fn a_following_calendar_takes_its_part_and_keeps_its_numbers() {
        let w = key("2026W");
        let mut max = PlanDoc::default();
        assert!(max.plan(w, "12104", 1, None));
        let arbeit = max.add_own(0, w, termin(0, "Arbeit")).unwrap();
        let sport = max.add_own(0, w, termin(0, "Sport")).unwrap();
        assert!(max.set_note(w, RowKey { event: folia_calendar::own::OWN_EVENT_BASE + sport, fp: 0 }, "Turnschuhe"));
        let part = max.live_part(&Share { notes: true, ..share(1, 0) }, w);
        let mut lena = PlanDoc::default();
        assert!(lena.add_own(0, w, termin(0, "Eigenes")).is_some());
        let id = lena.add_calendar("", None, w, &[], &[], &[]).unwrap();
        assert!(lena.follow(id, &encode(&[1; LINK_BYTES]), 5));
        assert!(lena.take_part(id, &part));
        assert!(!lena.take_part(id, &part), "the same part changes nothing");
        let cal = lena.calendar(id).unwrap().clone();
        assert_eq!(cal.modules_in(w), ["12104"]);
        let numbers: Vec<u32> = cal.own.iter().map(|own| own.termin.number).collect();
        assert_eq!(numbers.len(), 2);
        assert!(!numbers.contains(&1), "Lena's own Termin keeps its number: {numbers:?}");
        let sport_here = cal.own.iter().find(|own| own.termin.name == "Sport").map(|own| own.termin.number).unwrap();
        assert_eq!(cal.note_of(w, RowKey { event: folia_calendar::own::OWN_EVENT_BASE + sport_here, fp: 0 }), Some("Turnschuhe"), "a note names the Termin by its number here");
        // Max renames a Termin and plans another module: the numbers stay where the places stay.
        let _ = arbeit;
        max.own[0].termin.name = "Job".into();
        assert!(max.plan(w, "12107", 2, None));
        assert!(lena.take_part(id, &max.live_part(&Share { notes: true, ..share(1, 0) }, w)));
        let after = lena.calendar(id).unwrap();
        assert_eq!(after.own.iter().map(|own| own.termin.number).collect::<Vec<_>>(), numbers);
        assert_eq!(after.modules_in(w), ["12104", "12107"]);
        assert_eq!(after.own[0].termin.name, "Job");
        // The next semester: what the share showed of the last one stays to look at.
        let s = key("2027S");
        assert!(max.plan(s, "11113", 3, None));
        assert!(lena.take_part(id, &max.live_part(&share(1, 0), s)));
        let later = lena.calendar(id).unwrap();
        assert_eq!((later.modules_in(w).len(), later.modules_in(s)), (2, vec!["11113".to_string()]));
        // Not shared any more: it stays as it is until „Behalten" or „Löschen".
        assert!(lena.end_follow(id, 9) && !lena.end_follow(id, 10));
        assert_eq!(lena.follow_of(id).map(|f| f.ended), Some(9));
        assert!(lena.unfollow(id) && lena.follow_of(id).is_none() && lena.calendar(id).is_some());
    }

    #[test]
    fn a_link_is_an_id_and_a_key() {
        let id = encode(&[1u8; 16]);
        let key = encode(&[2u8; 32]);
        let link = join_link(&id, &key).unwrap();
        assert_eq!(link.len(), 64);
        assert_eq!(split_link(&link), Some((id, key)));
        assert_eq!(split_link("short"), None);
    }
}
