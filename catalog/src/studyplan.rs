//! The Studienplan and „Mein Studiengang" as stored text, the import of a Regelstudienplan, its
//! placeholders and the default semester.
//!
//! Both live in the visitor's browser and nowhere else (R20), as lines of text under
//! `betula.studyplan.v1` (`PlanDoc`) and `betula.myprogram.v1` (`MineDoc`). The text is read like
//! a URL: every line is checked on its own and a line that fails is dropped alone, every list has
//! a cap, and a text that is too long is read up to a limit, so no stored text breaks the page and
//! none reads as empty. Lines of a tag this build does not know are kept and written back as they
//! were: a newer build may have written them (a blue-green switch, a canary rollback, an old PWA
//! tab), and going back one release must not erase them. The mutating methods keep to the same
//! rules as the reader, so whatever they make reads back whole.
//!
//! The rest is pure logic over the plans `variants` reads. `import` takes a Regelstudienplan over
//! as modules and placeholders from a Fachsemester on; `resolve_placeholder` finds a placeholder's
//! plan row again in a later snapshot; `placeholder_line` says what it asks for the way the
//! catalog's note of a semester does. It lives here and not in the app so that the tests run it
//! against the snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::labels::{Code, Labelled, ModuleKind, Season, TurnusSeason};
use crate::plan::{self, SemesterRequirement};
use crate::queries::MAX_PLANNED;
use crate::rows::CatalogRow;
use crate::rows_detail::PlanEntry;
use crate::timetable::day::Day;
use crate::timetable::kind::{EventKind, KindSet};
use crate::timetable::rowkey::RowKey;
use crate::timetable::select::{Selection, Town, TownChoice, MAX_HIDDEN, MAX_MODULES};
use crate::timetable::semester::{of_fachsemester, SemesterKey};
use crate::timetable::subscription::MAX_CODE;
use crate::url;
use crate::variants::{self, PlanVariant};

/// The most bytes of `betula.studyplan.v1` that are read. What Betula writes within the caps stays
/// below it (a test fills a store to every cap); a longer text is read up to the last line break
/// before the limit, so a store never reads as empty because of its size and the next change
/// cannot wipe a plan. What lies beyond is dropped with the next write, which only text Betula did
/// not write can lose.
pub const MAX_STORED: usize = 512 * 1024;

/// The same for `betula.myprogram.v1`, a handful of short lines.
pub const MAX_MINE: usize = 4 * 1024;

/// The most semesters a plan has lines for: a long study with retakes, and some to spare.
pub const MAX_SEMESTERS: usize = 24;

/// The most placeholders. The plan of one program has at most 47 rows without a module (snapshot
/// of 2026-09-23), so two programs' plans fit.
pub const MAX_PLACEHOLDERS: usize = 100;

/// The most hidden events, the most hidden Termine and the most made choices in all semesters
/// together, each. One semester holds at most `MAX_HIDDEN` of each, as a subscription code does.
pub const MAX_HIDDEN_ALL: usize = 2_000;

/// Lines of a tag this build does not know: how many are kept, and how long each may be in bytes.
const MAX_EXTRA_LINES: usize = 100;
const MAX_EXTRA_BYTES: usize = 1_000;

/// The most characters of a caption or of a plan row's name; the snapshot's longest have 233 and
/// 242.
const MAX_TEXT: usize = 250;

/// The most characters of a placeholder's credits („10–24").
const MAX_CREDITS: usize = 16;

/// The largest `PlanEntry::ord` a placeholder may name; plans have a few hundred rows.
const MAX_ORD: i64 = 100_000;

/// The last Fachsemester a placeholder's span may name.
const MAX_SPAN: u8 = 20;

/// The largest event id a line may name: `^[1-9][0-9]{0,8}$` (the snapshot's `veranstid`s have six
/// digits).
const MAX_EVENT: u32 = 999_999_999;

/// The visitor's Studienplan: modules and placeholders per calendar semester, and per semester
/// what is hidden, what was chosen and the subscription code last handed out.
///
/// Stored as lines (`stored`), the fields separated by a tab (two spaces here), the first field a
/// tag:
///
/// ```text
/// m  <semester>  <module_id>  <added_secs>  <fills>
/// p  <pid>  <semester>  <program_id>  <ord>  <from>-<to>  <credits>  <kind>  <caption>  <name>
/// k  <semester>  <kind>,<kind>,…
/// e  <semester>  <event_id>
/// r  <semester>  <event_id>-<fp 5 hex>
/// c  <semester>  <event_id>-<fp 5 hex>
/// a  <semester>  <code>
/// ```
///
/// `r` hides one Termin, `c` is a made choice („Nur diesen"): of the choice that holds the row,
/// only the option with it is shown. A choice whose row QIS changed no longer matches, and the
/// choice is open again, the safe direction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanDoc {
    /// By semester, and within one in the order they were planned (`m`).
    pub modules: Vec<Planned>,
    /// By pid (`p`).
    pub placeholders: Vec<Placeholder>,
    /// What each semester hides and has chosen (`k`, `e`, `r`, `c`); no semester without any.
    pub hidden: BTreeMap<SemesterKey, SemesterHides>,
    /// The code of the subscription last handed out, per semester (`a`).
    pub subscribed: BTreeMap<SemesterKey, String>,
    /// Lines of tags this build does not know, as they were read.
    pub extra: Vec<String>,
}

/// A module planned into a calendar semester. A module may be planned into several (a module of
/// two semesters, a retake).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub semester: SemesterKey,
    pub module_id: String,
    /// When it was planned, in seconds since 1970; 0 where unknown.
    pub at: u64,
    /// The placeholder it counts for. Only the visitor sets it (the finder's `fill=`, the aside's
    /// „Zählt für"): which module fills which row is never guessed.
    pub fills: Option<u32>,
}

/// A row of a Regelstudienplan without a module, taken over into a semester under its own name.
/// It keeps its own text, so it always shows whatever became of the plan since, and
/// `resolve_placeholder` finds its row again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placeholder {
    /// A number of this browser's own, 1–9999 and unique in the plan; `fill=p<pid>` names it.
    pub pid: u32,
    /// Where it stands: the first semester of its span that was taken over.
    pub semester: SemesterKey,
    pub program_id: String,
    /// The row's `PlanEntry::ord`. With the program it makes a placeholder one row of one plan: a
    /// program has one placeholder per row at most.
    pub ord: i64,
    /// The Fachsemester the row spans, as the plan states them (`plan::semester_span`).
    pub span: (u8, u8),
    /// What the row says about its credits (`plan::credits_of`): „6", „10–24".
    pub credits: Option<String>,
    /// The `ModuleKind` code the plan states for the row.
    pub kind: Option<String>,
    /// The caption of the plan the row comes from; `""` for the one unnamed plan of a program.
    pub caption: String,
    /// The row's name as a page shows it (`plan::shown_name`).
    pub name: String,
}

/// What one semester hides (`k`, `e`, `r`) and has chosen (`c`). Named apart from
/// `timetable::select::HiddenBy`, which says why something is not shown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SemesterHides {
    /// Kinds switched off; an event is hidden only when all its kinds are.
    pub kinds: KindSet,
    pub events: BTreeSet<u32>,
    pub rows: BTreeSet<RowKey>,
    pub chosen: BTreeSet<RowKey>,
}

impl SemesterHides {
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty() && self.events.is_empty() && self.rows.is_empty() && self.chosen.is_empty()
    }
}

impl PlanDoc {
    /// The plan of a stored text. Every line is checked like URL input and dropped alone when it
    /// fails; duplicates count once (the first `m` of a semester and module, the first `p` of a
    /// pid and of a plan row, the last `a` of a semester); the caps keep the first lines in the
    /// order of the text. Never fails.
    pub fn restored(text: &str) -> Self {
        let mut doc = PlanDoc::default();
        // `m` lines come before the `p` lines they may name, so what they fill is set at the end.
        let mut fills: Vec<(SemesterKey, String, u32)> = Vec::new();
        for raw in head(text, MAX_STORED).split('\n') {
            let line: String = raw.chars().filter(|c| *c != '\r').collect();
            let fields: Vec<&str> = line.split('\t').map(str::trim).collect();
            let field = |i: usize| fields.get(i).copied().unwrap_or_default();
            let semester = SemesterKey::parse(field(1));
            match field(0) {
                "" => {}
                "m" => {
                    let Some(s) = semester else { continue };
                    if doc.plan(s, field(2), number(field(3)).unwrap_or(0), None) {
                        if let Some(pid) = number(field(4)).and_then(|pid| u32::try_from(pid).ok()) {
                            fills.push((s, field(2).to_string(), pid));
                        }
                    }
                }
                "p" => {
                    if let Some(placeholder) = placeholder_of(&fields) {
                        doc.add_placeholder(placeholder);
                    }
                }
                "k" => {
                    if let Some(s) = semester {
                        for kind in KindSet::parse_codes(field(2)).iter() {
                            doc.set_kind(s, kind, true);
                        }
                    }
                }
                "e" => {
                    if let (Some(s), Some(event)) = (semester, event_of(field(2))) {
                        doc.set_event(s, event, true);
                    }
                }
                "r" => {
                    if let (Some(s), Some(row)) = (semester, RowKey::parse(field(2))) {
                        doc.set_row(s, row, true);
                    }
                }
                "c" => {
                    // Not `choose`: a text with two choices for one event keeps both, as only
                    // the same line twice counts once.
                    if let (Some(s), Some(row)) = (semester, RowKey::parse(field(2))) {
                        doc.add_to(s, row, |h| &h.chosen, |h| &mut h.chosen);
                    }
                }
                "a" => {
                    if let Some(s) = semester {
                        doc.remember(s, field(2));
                    }
                }
                _ => {
                    if doc.extra.len() < MAX_EXTRA_LINES && line.len() <= MAX_EXTRA_BYTES {
                        doc.extra.push(line.clone());
                    }
                }
            }
        }
        for (s, id, pid) in fills {
            doc.set_fills(s, &id, Some(pid));
        }
        doc
    }

    /// The text `restored` reads back as this plan: `m` by semester, then in planning order;
    /// `p` by pid; then per semester `k`, `e`, `r`, `c`, `a`; then the lines of unknown tags.
    /// Control characters in names become spaces, so a name never splits its line.
    pub fn stored(&self) -> String {
        let mut out = String::new();
        let mut modules: Vec<&Planned> = self.modules.iter().filter(|m| url::is_module_id(&m.module_id)).collect();
        modules.sort_by_key(|m| m.semester);
        for m in modules {
            let fills = m.fills.map(|pid| pid.to_string()).unwrap_or_default();
            let _ = writeln!(out, "m\t{}\t{}\t{}\t{}", m.semester.key(), m.module_id, m.at, fills);
        }
        let mut placeholders: Vec<&Placeholder> = self.placeholders.iter().filter(|p| url::is_program_id(&p.program_id)).collect();
        placeholders.sort_by_key(|p| p.pid);
        for p in placeholders {
            let _ = writeln!(
                out,
                "p\t{}\t{}\t{}\t{}\t{}-{}\t{}\t{}\t{}\t{}",
                p.pid,
                p.semester.key(),
                p.program_id,
                p.ord,
                p.span.0,
                p.span.1,
                p.credits.as_deref().and_then(credits_text).unwrap_or_default(),
                p.kind.as_deref().and_then(kind_code).unwrap_or_default(),
                clean(&p.caption),
                clean(&p.name),
            );
        }
        let semesters: BTreeSet<SemesterKey> = self.hidden.keys().chain(self.subscribed.keys()).copied().collect();
        for s in semesters {
            let key = s.key();
            if let Some(hides) = self.hidden.get(&s) {
                if !hides.kinds.is_empty() {
                    let _ = writeln!(out, "k\t{key}\t{}", hides.kinds.codes());
                }
                for event in &hides.events {
                    let _ = writeln!(out, "e\t{key}\t{event}");
                }
                for row in &hides.rows {
                    let _ = writeln!(out, "r\t{key}\t{}", row.text());
                }
                for row in &hides.chosen {
                    let _ = writeln!(out, "c\t{key}\t{}", row.text());
                }
            }
            if let Some(code) = self.subscribed.get(&s).filter(|code| is_code(code)) {
                let _ = writeln!(out, "a\t{key}\t{code}");
            }
        }
        for line in self.extra.iter().filter(|line| !line.contains(['\n', '\r'])) {
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    /// Nothing planned: no module and no placeholder. The stored text may still hold what a
    /// semester hides, a code or lines of a newer build; the store's key goes only when `stored`
    /// is empty.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty() && self.placeholders.is_empty()
    }

    /// The semesters the plan holds modules or placeholders in, in order.
    pub fn semesters(&self) -> Vec<SemesterKey> {
        let held: BTreeSet<SemesterKey> = self.modules.iter().map(|m| m.semester).chain(self.placeholders.iter().map(|p| p.semester)).collect();
        held.into_iter().collect()
    }

    /// The modules of a semester, in the order they were planned.
    pub fn modules_in(&self, s: SemesterKey) -> Vec<String> {
        self.modules.iter().filter(|m| m.semester == s).map(|m| m.module_id.clone()).collect()
    }

    pub fn is_planned(&self, s: SemesterKey, id: &str) -> bool {
        self.modules.iter().any(|m| m.semester == s && m.module_id == id)
    }

    /// The semesters a module is planned in, in order.
    pub fn planned_in(&self, id: &str) -> Vec<SemesterKey> {
        let held: BTreeSet<SemesterKey> = self.modules.iter().filter(|m| m.module_id == id).map(|m| m.semester).collect();
        held.into_iter().collect()
    }

    /// The placeholders standing in a semester, by pid.
    pub fn placeholders_in(&self, s: SemesterKey) -> Vec<&Placeholder> {
        self.placeholders.iter().filter(|p| p.semester == s).collect()
    }

    /// The modules that count for a placeholder, wherever they are planned.
    pub fn fillers(&self, pid: u32) -> Vec<&Planned> {
        self.modules.iter().filter(|m| m.fills == Some(pid)).collect()
    }

    /// What the visitor chose to see of a semester, with the town of Mein Studiengang.
    pub fn selection(&self, s: SemesterKey, town: TownChoice) -> Selection {
        let hides = self.hidden.get(&s);
        Selection {
            hidden_kinds: hides.map(|h| h.kinds).unwrap_or_default(),
            hidden_events: hides.map(|h| h.events.clone()).unwrap_or_default(),
            hidden_rows: hides.map(|h| h.rows.clone()).unwrap_or_default(),
            chosen_rows: hides.map(|h| h.chosen.clone()).unwrap_or_default(),
            town,
        }
    }

    /// Plans a module into a semester, counting for the placeholder `fills` when the plan holds
    /// it. `false` when nothing changed: the id is no module id, the module is planned there
    /// already, or a cap is reached (60 modules in the semester, 400 in all, 24 semesters).
    pub fn plan(&mut self, s: SemesterKey, id: &str, at: u64, fills: Option<u32>) -> bool {
        if !url::is_module_id(id) || self.is_planned(s, id) {
            return false;
        }
        let here = self.modules.iter().filter(|m| m.semester == s).count();
        if here >= MAX_MODULES || self.modules.len() >= MAX_PLANNED || !self.admits(s) {
            return false;
        }
        let fills = fills.filter(|pid| self.placeholders.iter().any(|p| p.pid == *pid));
        // After the semester's last module: the list stays in the order `stored` writes it.
        let at_index = self.modules.partition_point(|m| m.semester <= s);
        self.modules.insert(at_index, Planned { semester: s, module_id: id.to_string(), at, fills });
        true
    }

    /// Takes a module out of a semester, together with what the semester hides and has chosen of
    /// `only_its_events`: the events no other module of the semester links (the page knows them
    /// from its rows). Hidden kinds stay; they are about the semester, not the module.
    pub fn unplan(&mut self, s: SemesterKey, id: &str, only_its_events: &[u32]) {
        self.modules.retain(|m| !(m.semester == s && m.module_id == id));
        if let Some(hides) = self.hidden.get_mut(&s) {
            for event in only_its_events {
                hides.events.remove(event);
            }
            hides.rows.retain(|row| !only_its_events.contains(&row.event));
            hides.chosen.retain(|row| !only_its_events.contains(&row.event));
        }
        self.prune(s);
    }

    /// Moves a module to another semester, with when it was planned and what it counts for. When
    /// it is planned there already, it only leaves `from`; when `to` is full, nothing changes.
    pub fn move_to(&mut self, from: SemesterKey, to: SemesterKey, id: &str) {
        if from == to {
            return;
        }
        let Some(at) = self.modules.iter().position(|m| m.semester == from && m.module_id == id) else {
            return;
        };
        let moved = self.modules.remove(at);
        if !self.is_planned(to, id) && !self.plan(to, id, moved.at, moved.fills) {
            self.modules.insert(at, moved);
        }
    }

    /// Which placeholder a planned module counts for; a pid the plan does not hold means none.
    pub fn set_fills(&mut self, s: SemesterKey, id: &str, pid: Option<u32>) {
        let pid = pid.filter(|pid| self.placeholders.iter().any(|p| p.pid == *pid));
        if let Some(m) = self.modules.iter_mut().find(|m| m.semester == s && m.module_id == id) {
            m.fills = pid;
        }
    }

    /// Removes a placeholder; the modules that counted for it stay and count for nothing.
    pub fn remove_placeholder(&mut self, pid: u32) {
        self.placeholders.retain(|p| p.pid != pid);
        for m in self.modules.iter_mut().filter(|m| m.fills == Some(pid)) {
            m.fills = None;
        }
    }

    pub fn set_kind(&mut self, s: SemesterKey, kind: EventKind, hidden: bool) {
        if !hidden {
            if let Some(hides) = self.hidden.get_mut(&s) {
                hides.kinds = hides.kinds.without(kind);
            }
            self.prune(s);
            return;
        }
        if self.hidden.get(&s).is_some_and(|h| h.kinds.contains(kind)) || !self.admits(s) {
            return;
        }
        let hides = self.hidden.entry(s).or_default();
        hides.kinds = hides.kinds.with(kind);
    }

    /// Hides a whole event of a semester, or shows it again. Beyond the caps (500 per semester,
    /// 2,000 in all) a further one is not hidden.
    pub fn set_event(&mut self, s: SemesterKey, event: u32, hidden: bool) {
        if !hidden {
            self.take_from(s, &event, |h| &mut h.events);
        } else if (1..=MAX_EVENT).contains(&event) {
            self.add_to(s, event, |h| &h.events, |h| &mut h.events);
        }
    }

    /// Hides one Termin of a semester (an eye button), or shows it again; caps as `set_event`.
    pub fn set_row(&mut self, s: SemesterKey, row: RowKey, hidden: bool) {
        if !hidden {
            self.take_from(s, &row, |h| &mut h.rows);
        } else if is_row(row) {
            self.add_to(s, row, |h| &h.rows, |h| &mut h.rows);
        }
    }

    /// Replaces the choice made for an event („Nur diesen") by `row`, a row of that event; `None`
    /// clears it („Alle zeigen").
    pub fn choose(&mut self, s: SemesterKey, event: u32, row: Option<RowKey>) {
        if let Some(hides) = self.hidden.get_mut(&s) {
            hides.chosen.retain(|chosen| chosen.event != event);
        }
        if let Some(row) = row.filter(|row| row.event == event && is_row(*row)) {
            self.add_to(s, row, |h| &h.chosen, |h| &mut h.chosen);
        }
        self.prune(s);
    }

    /// Shows every event and Termin of a semester again and undoes its choices; hidden kinds stay
    /// (they have chips of their own).
    pub fn show_all(&mut self, s: SemesterKey) {
        if let Some(hides) = self.hidden.get_mut(&s) {
            hides.events.clear();
            hides.rows.clear();
            hides.chosen.clear();
        }
        self.prune(s);
    }

    /// Keeps the code of the subscription just handed out for a semester, so the page can tell
    /// when the plan has moved on from it. A text that is no code is not kept.
    pub fn remember(&mut self, s: SemesterKey, code: &str) {
        let code = code.trim();
        if is_code(code) && self.admits(s) {
            self.subscribed.insert(s, code.to_string());
        }
    }

    /// The pid a new placeholder gets: one more than the largest, else the smallest one free.
    pub fn next_pid(&self) -> u32 {
        let largest = self.placeholders.iter().map(|p| p.pid).max().unwrap_or(0);
        if largest < url::MAX_PID {
            return largest + 1;
        }
        (1..=url::MAX_PID).find(|pid| !self.placeholders.iter().any(|p| p.pid == *pid)).unwrap_or(url::MAX_PID)
    }

    /// Takes an import over (`import`): its modules planned at `now`, its placeholders numbered.
    /// What the plan holds meanwhile is left out, so applying it twice adds nothing. Returns how
    /// many modules and placeholders were added.
    pub fn apply(&mut self, import: &Import, now: u64) -> (usize, usize) {
        let mut modules = 0;
        for (s, id) in &import.modules {
            if self.plan(*s, id, now, None) {
                modules += 1;
            }
        }
        let mut placeholders = 0;
        for placeholder in &import.placeholders {
            let pid = self.next_pid();
            if self.add_placeholder(Placeholder { pid, ..placeholder.clone() }) {
                placeholders += 1;
            }
        }
        (modules, placeholders)
    }

    /// Adds a placeholder in pid order, when it is valid, its pid and its plan row are new and the
    /// caps allow it.
    fn add_placeholder(&mut self, p: Placeholder) -> bool {
        let valid = (1..=url::MAX_PID).contains(&p.pid)
            && url::is_program_id(&p.program_id)
            && (1..=MAX_ORD).contains(&p.ord)
            && 1 <= p.span.0
            && p.span.0 <= p.span.1
            && p.span.1 <= MAX_SPAN;
        let taken = self.placeholders.iter().any(|q| q.pid == p.pid || (q.program_id == p.program_id && q.ord == p.ord));
        if !valid || taken || self.placeholders.len() >= MAX_PLACEHOLDERS || !self.admits(p.semester) {
            return false;
        }
        let (credits, kind) = (p.credits.as_deref().and_then(credits_text), p.kind.as_deref().and_then(kind_code));
        let (caption, name) = (clean(&p.caption), clean(&p.name));
        let at = self.placeholders.partition_point(|q| q.pid < p.pid);
        self.placeholders.insert(at, Placeholder { credits, kind, caption, name, ..p });
        true
    }

    /// Adds `item` to one of a semester's lists (`read` and `write` name the same one), within
    /// the caps.
    fn add_to<T: Ord>(&mut self, s: SemesterKey, item: T, read: fn(&SemesterHides) -> &BTreeSet<T>, write: fn(&mut SemesterHides) -> &mut BTreeSet<T>) {
        let here = self.hidden.get(&s).map(read);
        if here.is_some_and(|list| list.contains(&item)) {
            return;
        }
        let full = here.map_or(0, BTreeSet::len) >= MAX_HIDDEN || self.hidden.values().map(|h| read(h).len()).sum::<usize>() >= MAX_HIDDEN_ALL;
        if full || !self.admits(s) {
            return;
        }
        write(self.hidden.entry(s).or_default()).insert(item);
    }

    fn take_from<T: Ord>(&mut self, s: SemesterKey, item: &T, write: fn(&mut SemesterHides) -> &mut BTreeSet<T>) {
        if let Some(hides) = self.hidden.get_mut(&s) {
            write(hides).remove(item);
        }
        self.prune(s);
    }

    /// A semester that hides nothing has no entry, as the text without its lines reads.
    fn prune(&mut self, s: SemesterKey) {
        if self.hidden.get(&s).is_some_and(SemesterHides::is_empty) {
            self.hidden.remove(&s);
        }
    }

    /// Whether a line of semester `s` may be added: the plan has lines of it already, or fewer
    /// than `MAX_SEMESTERS` semesters.
    fn admits(&self, s: SemesterKey) -> bool {
        let held: BTreeSet<SemesterKey> = self
            .modules
            .iter()
            .map(|m| m.semester)
            .chain(self.placeholders.iter().map(|p| p.semester))
            .chain(self.hidden.keys().copied())
            .chain(self.subscribed.keys().copied())
            .collect();
        held.contains(&s) || held.len() < MAX_SEMESTERS
    }
}

/// „Mein Studiengang": the visitor's program, study direction, Studienbeginn and town, stored as
/// lines of a key, a tab (two spaces here) and a value:
///
/// ```text
/// program  048-82-2022
/// name  Elektrotechnik B.Sc. · PO 2022
/// caption  Regelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium
/// start  2026W
/// town  cottbus
/// ```
///
/// Every line is optional and the first valid one of a key wins; keys this build does not know are
/// kept and written back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MineDoc {
    /// `program.id`, never the slug (which changes on a collision).
    pub program: Option<String>,
    /// The program's display name when it was set: what the page says once the id is gone from
    /// the snapshot.
    pub name: Option<String>,
    /// The caption of the chosen plan; `Some("")` for the only or the unnamed plan.
    pub caption: Option<String>,
    /// The caption of the page („Studienplan · Seite 18") that fills the direction row of a core
    /// plan, when one was chosen.
    pub direction: Option<String>,
    pub start: Option<SemesterKey>,
    /// `Derive` is not stored: the town then follows from the plan.
    pub town: TownChoice,
    pub extra: Vec<(String, String)>,
}

impl MineDoc {
    const KEYS: [&'static str; 6] = ["program", "name", "caption", "direction", "start", "town"];

    /// „Mein Studiengang" of a stored text, read up to `MAX_MINE` bytes. Never fails.
    pub fn restored(text: &str) -> Self {
        let mut doc = MineDoc::default();
        let mut town = None;
        for raw in head(text, MAX_MINE).split('\n') {
            let line: String = raw.chars().filter(|c| *c != '\r').collect();
            let (key, value) = line.split_once('\t').unwrap_or((line.as_str(), ""));
            let (key, value) = (key.trim(), value.trim());
            match key {
                "" => {}
                "program" => {
                    if doc.program.is_none() && url::is_program_id(value) {
                        doc.program = Some(value.to_string());
                    }
                }
                "name" => {
                    if doc.name.is_none() && !value.is_empty() {
                        doc.name = Some(clean(value));
                    }
                }
                "caption" => {
                    if doc.caption.is_none() {
                        doc.caption = Some(clean(value));
                    }
                }
                "direction" => {
                    if doc.direction.is_none() {
                        doc.direction = Some(clean(value));
                    }
                }
                "start" => {
                    if doc.start.is_none() {
                        doc.start = SemesterKey::parse(value);
                    }
                }
                "town" => {
                    if town.is_none() {
                        town = town_of_code(value);
                    }
                }
                _ => {
                    if !doc.extra.iter().any(|(known, _)| known == key) {
                        doc.extra.push((key.to_string(), value.to_string()));
                    }
                }
            }
        }
        doc.town = town.unwrap_or_default();
        doc
    }

    /// „Mein Studiengang" cleared: the program and its plan go; the Studienbeginn and the town
    /// stay, since they are the student's whatever the program.
    pub fn clear_program(&mut self) {
        self.program = None;
        self.name = None;
        self.caption = None;
        self.direction = None;
    }

    /// The text `restored` reads back as this.
    pub fn stored(&self) -> String {
        let mut out = String::new();
        if let Some(program) = self.program.as_deref().filter(|id| url::is_program_id(id)) {
            let _ = writeln!(out, "program\t{program}");
        }
        if let Some(name) = self.name.as_deref().map(clean).filter(|name| !name.is_empty()) {
            let _ = writeln!(out, "name\t{name}");
        }
        if let Some(caption) = &self.caption {
            let _ = writeln!(out, "caption\t{}", clean(caption));
        }
        if let Some(direction) = &self.direction {
            let _ = writeln!(out, "direction\t{}", clean(direction));
        }
        if let Some(start) = self.start {
            let _ = writeln!(out, "start\t{}", start.key());
        }
        let town = match self.town {
            TownChoice::Derive => None,
            TownChoice::Only(town) => Some(town.code()),
            TownChoice::Both => Some("both"),
        };
        if let Some(town) = town {
            let _ = writeln!(out, "town\t{town}");
        }
        for (key, value) in &self.extra {
            let whole = !key.trim().is_empty() && !key.contains(['\t', '\n', '\r']) && !value.contains(['\n', '\r']);
            if whole && !Self::KEYS.contains(&key.as_str()) {
                let _ = writeln!(out, "{key}\t{value}");
            }
        }
        out
    }
}

/// `cottbus` | `senftenberg` | `both`, as `MineDoc` stores the town.
fn town_of_code(code: &str) -> Option<TownChoice> {
    match code {
        "cottbus" => Some(TownChoice::Only(Town::Cottbus)),
        "senftenberg" => Some(TownChoice::Only(Town::Senftenberg)),
        "both" => Some(TownChoice::Both),
        _ => None,
    }
}

/// What taking a Regelstudienplan over adds to a plan (`import`), and its preview.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Import {
    /// The modules and their semesters, by Fachsemester and then in the plan's order.
    pub modules: Vec<(SemesterKey, String)>,
    /// With pid 0: `PlanDoc::apply` numbers them.
    pub placeholders: Vec<Placeholder>,
    /// Rows left out because the plan holds them already („3 schon geplant").
    pub skipped: usize,
    /// The preview: one entry per Fachsemester that gets something, in order.
    pub by_fs: Vec<ImportFs>,
}

/// What one Fachsemester gets from an import: „1. FS · WiSe 2026/27 · 4 Module · 1 Platzhalter".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportFs {
    pub fs: u8,
    pub semester: SemesterKey,
    /// The names of the modules and placeholders as the plan prints them (`plan::shown_name`).
    pub modules: Vec<String>,
    pub placeholders: Vec<String>,
}

/// What taking the plan `core` over from Fachsemester `from_fs` on adds to `doc`, for a study that
/// started in `start`.
///
/// Every row whose span (`plan::semester_span`) ends at or after `from_fs` is taken, at
/// Fachsemester `max(span start, from_fs)`: a row over several semesters stands once, in the first
/// of them that is taken. A row naming a module plans that module; one the plan holds in any
/// semester is left out and counted as „schon geplant", and one the plan names twice is taken
/// once, at its first Fachsemester. Every other row becomes a placeholder under its own name,
/// credits, kind, span and the caption of its plan; one whose row (`program_id`, `ord`) has a
/// placeholder already, filled or not, in whatever semester, is left out and counted. Rows of
/// prose, with neither credits nor a kind, are left out. So importing twice adds nothing.
///
/// `page` is a page that fills a row of `core` (`variants::supplements`), with the `ord` of that
/// row: its rows are taken with the core's, and the row it fills is not.
pub fn import(doc: &PlanDoc, program_id: &str, core: &PlanVariant, page: Option<(&PlanVariant, i64)>, start: SemesterKey, from_fs: u8) -> Import {
    let from_fs = from_fs.max(1);
    let parts = std::iter::once((core, page.map(|(_, filled)| filled))).chain(page.map(|(page, _)| (page, None)));
    let mut rows: Vec<(u8, SemesterKey, &PlanVariant, &PlanEntry)> = Vec::new();
    for (variant, filled) in parts {
        for entry in variant.entries.iter().filter(|entry| Some(entry.ord) != filled) {
            let Some((first, last)) = plan::semester_span(entry) else { continue };
            if last < i64::from(from_fs) {
                continue;
            }
            let Ok(fs) = u8::try_from(first.max(i64::from(from_fs))) else { continue };
            let Some(semester) = of_fachsemester(start, fs) else { continue };
            rows.push((fs, semester, variant, entry));
        }
    }
    // By Fachsemester, and within one in the order of the plan: a module the plan names twice is
    // taken at its first semester, and the preview reads top down.
    rows.sort_by_key(|(fs, ..)| *fs);

    let mut out = Import::default();
    let mut taken: BTreeSet<&str> = BTreeSet::new();
    for (fs, semester, variant, entry) in rows {
        let name = plan::shown_name(&entry.module_name).to_string();
        let is_module = match entry.module_id.as_deref() {
            Some(id) if !url::is_module_id(id) => continue,
            Some(id) => {
                if !doc.planned_in(id).is_empty() {
                    out.skipped += 1;
                    continue;
                }
                if !taken.insert(id) {
                    continue;
                }
                out.modules.push((semester, id.to_string()));
                true
            }
            None => {
                let credits = plan::credits_of(entry).as_deref().and_then(credits_text);
                let kind = entry.kind.as_ref().and_then(Code::known).map(|kind| kind.code().to_string());
                if credits.is_none() && kind.is_none() {
                    continue;
                }
                if doc.placeholders.iter().any(|p| p.program_id == program_id && p.ord == entry.ord) {
                    out.skipped += 1;
                    continue;
                }
                let span = plan::semester_span(entry).and_then(|(from, to)| Some((u8::try_from(from).ok()?, u8::try_from(to).ok()?)));
                let Some(span) = span else { continue };
                out.placeholders.push(Placeholder {
                    pid: 0,
                    semester,
                    program_id: program_id.to_string(),
                    ord: entry.ord,
                    span,
                    credits,
                    kind,
                    caption: clean(&variant.full),
                    name: clean(&name),
                });
                false
            }
        };
        let line = match out.by_fs.iter().position(|line| line.fs == fs) {
            Some(at) => out.by_fs.get_mut(at),
            None => {
                out.by_fs.push(ImportFs { fs, semester, modules: Vec::new(), placeholders: Vec::new() });
                out.by_fs.last_mut()
            }
        };
        if let Some(line) = line {
            match is_module {
                true => line.modules.push(name),
                false => line.placeholders.push(name),
            }
        }
    }
    out
}

/// A placeholder's row in the snapshot the page has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resolved<'a> {
    /// The plan and the row the placeholder stands for.
    Row(&'a PlanVariant, &'a PlanEntry),
    /// The program is there, the row is not recognisable any more: „nicht mehr im
    /// Regelstudienplan".
    Changed,
    /// The program itself is gone from the snapshot.
    Gone,
}

/// Finds a placeholder's row in its program's plans (`None`: the program is gone). The plan: the
/// one under the stored caption, else the only one, else the one with the most rows of the stored
/// name. The row in it, by its name folded (case, spacing, and the dashes, stars and footnote
/// marks around it do not count): the same `ord` and name; else the only row of that name and
/// span; else the only row of that name. Anything else is `Changed`: a guess could fill the
/// wrong row.
pub fn resolve_placeholder<'a>(p: &Placeholder, variants: Option<&'a [PlanVariant]>) -> Resolved<'a> {
    let Some(variants) = variants else {
        return Resolved::Gone;
    };
    let name = folded(&p.name);
    let named = |variant: &PlanVariant| variant.entries.iter().filter(|entry| folded(&entry.module_name) == name).count();
    let only_plan = match variants {
        [only] => Some(only),
        _ => None,
    };
    let variant = variants::variant_for(variants, &p.caption).or(only_plan).or_else(|| {
        let most = variants.iter().map(named).max().filter(|most| *most > 0)?;
        variants.iter().find(|variant| named(variant) == most)
    });
    let Some(variant) = variant else {
        return Resolved::Changed;
    };
    let same_name: Vec<&PlanEntry> = variant.entries.iter().filter(|entry| folded(&entry.module_name) == name).collect();
    let span = (i64::from(p.span.0), i64::from(p.span.1));
    let row = same_name
        .iter()
        .copied()
        .find(|entry| entry.ord == p.ord)
        .or_else(|| only(same_name.iter().copied().filter(|entry| plan::semester_span(entry) == Some(span))))
        .or_else(|| only(same_name.iter().copied()));
    match row {
        Some(entry) => Resolved::Row(variant, entry),
        None => Resolved::Changed,
    }
}

/// A plan row's name as `resolve_placeholder` compares it.
fn folded(name: &str) -> String {
    plan::shown_name(name).to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The one item, or `None` for none or several.
fn only<T>(mut items: impl Iterator<Item = T>) -> Option<T> {
    let first = items.next()?;
    items.next().is_none().then_some(first)
}

/// What a placeholder asks for, in the format of the catalog's note of a semester's requirements
/// (`catalog.rs::plan_note`): „≥ 6 LP Anwendungsfach: „Mathematik“, … oder „Physik“". A page
/// writes `credits` in bold, then `lead`, then after a colon the `areas` („„A“, „B“ oder „C““),
/// else the `tail` (`text` does exactly that).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceholderLine {
    /// „≥ 6 LP" for a choice (a module of the area may have more), „6 LP" for one module,
    /// „10–24 LP" for a range; the spaces are non-breaking.
    pub credits: Option<String>,
    /// The row's name, or „aus dem Bereich" / „aus den Bereichen" where the name only repeats its
    /// areas.
    pub lead: Option<String>,
    /// The names of the areas the row takes its modules from.
    pub areas: Vec<String>,
    pub tail: Option<&'static str>,
}

impl PlaceholderLine {
    /// The line as plain text.
    pub fn text(&self) -> String {
        let mut out = String::new();
        if let Some(credits) = &self.credits {
            out.push_str(credits);
        }
        if let Some(lead) = &self.lead {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(lead);
        }
        let rest = match (self.areas.as_slice(), self.tail) {
            ([], None) => None,
            ([], Some(tail)) => Some(tail.to_string()),
            (areas, _) => Some(quoted_list(areas)),
        };
        if let Some(rest) = rest {
            out.push_str(": ");
            out.push_str(&rest);
        }
        out
    }
}

/// „„A“, „B“ oder „C“".
fn quoted_list(items: &[String]) -> String {
    let mut out = String::new();
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(if i + 1 < items.len() { ", " } else { " oder " });
        }
        let _ = write!(out, "„{item}“");
    }
    out
}

/// The line of a placeholder: from `requirement`, what `plan::requirement_of` makes of its row
/// when `resolve_placeholder` found one; else from the stored name, credits and kind, without
/// areas and without a claim about where its modules come from.
pub fn placeholder_line(p: &Placeholder, requirement: Option<&SemesterRequirement>) -> PlaceholderLine {
    let (name, credits, single, fues) = match requirement {
        Some(row) => (row.shown_name().to_string(), row.credits.clone(), row.single, row.fues),
        None => {
            let stored = stored_entry(p);
            (plan::shown_name(&p.name).to_string(), p.credits.clone(), plan::is_single_module(&stored), plan::is_fues(&stored))
        }
    };
    let areas: Vec<String> = requirement.map(|row| row.areas.iter().map(|area| area.name().to_string()).collect()).unwrap_or_default();
    // A choice asks for at least that much; one module, or a range, is what it says.
    let credits = credits.map(|credits| match single || credits.contains(['–', '-']) {
        true => format!("{credits}\u{a0}LP"),
        false => format!("≥\u{a0}{credits}\u{a0}LP"),
    });
    let name = (!name.is_empty()).then_some(name);
    let (lead, areas, tail) = if single {
        (name, Vec::new(), Some("unter diesem Namen nicht im Katalog"))
    } else if fues {
        (Some("Fachübergreifendes Studium".to_string()), Vec::new(), None)
    } else if areas.is_empty() {
        // Without its row there is no telling where the modules come from.
        (name, areas, requirement.map(|_| "alle Wahlpflichtmodule"))
    } else if requirement.is_some_and(SemesterRequirement::named_by_areas) {
        let lead = if areas.len() > 1 { "aus den Bereichen" } else { "aus dem Bereich" };
        (Some(lead.to_string()), areas, None)
    } else {
        (name, areas, None)
    };
    PlaceholderLine { credits, lead, areas, tail }
}

/// A placeholder as the plan row it was taken from, as far as it keeps it: enough for
/// `plan::is_single_module` and `plan::is_fues`.
fn stored_entry(p: &Placeholder) -> PlanEntry {
    PlanEntry {
        ord: p.ord,
        module_id: None,
        module_name: p.name.clone(),
        semester: None,
        start_semester: Some(i64::from(p.span.0)),
        end_semester: Some(i64::from(p.span.1)),
        semester_span: None,
        credits: None,
        min_credits: None,
        max_credits: None,
        kind: p.kind.as_deref().map(Code::parse),
        kind_raw: None,
        study_section: None,
        subject_area: None,
        specialization: (!p.caption.is_empty()).then(|| p.caption.clone()),
        catalog_title: None,
        credits_differ_from_catalog: false,
        source_page: None,
    }
}

/// Whether a plan is for a winter or a summer intake. No source says so; the modules do: those the
/// plan puts into one odd Fachsemester (1, 3, 5 …, the semesters of the intake's season) are
/// offered in that season. A season wins with at least twice as many modules as the other; else,
/// or with none, `None`. `linked` are the catalog's rows of the plan's modules.
pub fn intake_season(variant: &PlanVariant, linked: &[CatalogRow]) -> Option<Season> {
    let mut counted: BTreeSet<&str> = BTreeSet::new();
    let (mut winter, mut summer) = (0usize, 0usize);
    for entry in &variant.entries {
        let (Some(id), Some((first, last))) = (entry.module_id.as_deref(), plan::semester_span(entry)) else {
            continue;
        };
        if first != last || first.rem_euclid(2) != 1 || !counted.insert(id) {
            continue;
        }
        match linked.iter().find(|row| row.id == id).and_then(|row| row.turnus_season.as_ref()).and_then(Code::known) {
            Some(TurnusSeason::Winter) => winter += 1,
            Some(TurnusSeason::Summer) => summer += 1,
            _ => {}
        }
    }
    if winter > 0 && winter >= 2 * summer {
        Some(Season::Winter)
    } else if summer > 0 && summer >= 2 * winter {
        Some(Season::Summer)
    } else {
        None
    }
}

/// The Studienbeginn the import suggests when none is stored: the current semester when it is of
/// the plan's intake season, else the one before it; the current one when the season is unknown.
pub fn intake_start(current: SemesterKey, intake: Option<Season>) -> SemesterKey {
    match intake {
        Some(season) if current.season() != season => current.plus(-1).unwrap_or(current),
        _ => current,
    }
}

/// The semester the bare `/studyplan` shows: the one before `current` while the plan holds modules
/// in it and its half-year has not ended (`today` at the latest on 31.03. after a winter, 30.09.
/// after a summer), else `current`. Radix moves the current semester on as soon as the next one
/// has dated events, during the winter's exam weeks; without this the page would jump to the
/// summer while the student's exams are still ahead. Without `today` (the server) it is `current`.
pub fn default_semester(current: SemesterKey, doc: &PlanDoc, today: Option<Day>) -> SemesterKey {
    let Some(today) = today else {
        return current;
    };
    match current.plus(-1) {
        Some(before) if doc.modules.iter().any(|m| m.semester == before) && today <= before.bounds().1 => before,
        _ => current,
    }
}

/// The text up to the last line break before `limit` bytes; all of it when it is not longer.
fn head(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let cut = text.as_bytes().get(..limit).and_then(|bytes| bytes.iter().rposition(|b| *b == b'\n')).unwrap_or(0);
    text.get(..cut).unwrap_or_default()
}

/// A text field as the store keeps it: control characters as spaces (a tab or a line break would
/// split its line), trimmed, at most `MAX_TEXT` characters, cut at a character.
fn clean(text: &str) -> String {
    let spaced: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let trimmed = spaced.trim();
    match trimmed.char_indices().nth(MAX_TEXT) {
        Some((at, _)) => trimmed.get(..at).unwrap_or(trimmed).trim_end().to_string(),
        None => trimmed.to_string(),
    }
}

/// A decimal number of ASCII digits only (no sign, no space).
fn number(text: &str) -> Option<u64> {
    let digits = !text.is_empty() && text.len() <= 20 && text.bytes().all(|b| b.is_ascii_digit());
    digits.then(|| text.parse().ok()).flatten()
}

/// An event id as a line may name it: `^[1-9][0-9]{0,8}$`.
fn event_of(text: &str) -> Option<u32> {
    if text.starts_with('0') {
        return None;
    }
    number(text).and_then(|n| u32::try_from(n).ok()).filter(|event| (1..=MAX_EVENT).contains(event))
}

/// A Termin's key that reads back from its text.
fn is_row(row: RowKey) -> bool {
    row.event > 0 && row.fp < 1 << 20
}

/// A placeholder's credits as the store keeps them: at most 16 characters of digits, `,`, `.` and
/// dashes.
fn credits_text(text: &str) -> Option<String> {
    let valid = !text.is_empty() && text.chars().count() <= MAX_CREDITS && text.chars().all(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | '–' | '-'));
    valid.then(|| text.to_string())
}

/// A `ModuleKind` code this build knows.
fn kind_code(text: &str) -> Option<String> {
    ModuleKind::from_code(text).map(|kind| kind.code().to_string())
}

/// A subscription code as the store keeps it: 1–`MAX_CODE` characters of `pack::ALPHABET`.
fn is_code(code: &str) -> bool {
    (1..=MAX_CODE).contains(&code.len()) && code.bytes().all(|b| pack::ALPHABET.as_bytes().contains(&b))
}

/// The placeholder of a `p` line's fields; `PlanDoc::add_placeholder` checks the ranges.
fn placeholder_of(fields: &[&str]) -> Option<Placeholder> {
    let field = |i: usize| fields.get(i).copied().unwrap_or_default();
    let (from, to) = field(5).split_once('-')?;
    let small = |text: &str| number(text.trim()).and_then(|n| u8::try_from(n).ok());
    Some(Placeholder {
        pid: number(field(1)).and_then(|n| u32::try_from(n).ok())?,
        semester: SemesterKey::parse(field(2))?,
        program_id: field(3).to_string(),
        ord: number(field(4)).and_then(|n| i64::try_from(n).ok())?,
        span: (small(from)?, small(to)?),
        credits: credits_text(field(6)),
        kind: kind_code(field(7)),
        caption: clean(field(8)),
        name: clean(field(9)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::area_fixtures::{self as real, row};
    use crate::pages;
    use crate::queries;
    use crate::url::BookmarkSort;
    use crate::variants::{plan_variants, supplements};

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn termin(event: u32, fp: u32) -> RowKey {
        RowKey { event, fp }
    }

    /// A placeholder of Informatik's plan, to be changed field by field.
    fn placeholder() -> Placeholder {
        Placeholder {
            pid: 0,
            semester: key("2026W"),
            program_id: "079-82-2008".to_string(),
            ord: 15,
            span: (1, 1),
            credits: Some("6".to_string()),
            kind: Some("elective".to_string()),
            caption: String::new(),
            name: "Anwendungsfach".to_string(),
        }
    }

    fn with_placeholders(placeholders: Vec<Placeholder>) -> Import {
        Import { placeholders, ..Default::default() }
    }

    /// A subscription code (the pinned one of `subscription.rs`).
    const CODE: &str = "CQpJeFAKchJKBgdlgf0e7Hwl_4S";

    /// The design's example (B.2): Informatik's first semester after the import, with one hidden
    /// kind, one hidden event and one choice.
    const EXAMPLE: &str = "m\t2026W\t12104\t1790000000\t\n\
        m\t2026W\t12107\t1790000000\t\n\
        m\t2026W\t12102\t1790000000\t\n\
        m\t2026W\t11112\t1790000000\t\n\
        p\t1\t2026W\t079-82-2008\t17\t1-1\t6\tfues\t\tFachübergreifendes Studium\n\
        k\t2026W\ttutorial\n\
        e\t2026W\t149408\n\
        c\t2026W\t148369-a4d12\n";

    fn example() -> PlanDoc {
        let w = key("2026W");
        let mut doc = PlanDoc::default();
        for id in ["12104", "12107", "12102", "11112"] {
            assert!(doc.plan(w, id, 1_790_000_000, None));
        }
        let fues = Placeholder { ord: 17, kind: Some("fues".to_string()), name: "Fachübergreifendes Studium".to_string(), ..placeholder() };
        assert_eq!(doc.apply(&with_placeholders(vec![fues]), 0), (0, 1));
        doc.set_kind(w, EventKind::Tutorial, true);
        doc.set_event(w, 149408, true);
        doc.choose(w, 148369, Some(termin(148369, 0xa4d12)));
        doc
    }

    #[test]
    fn the_store_reads_back_what_it_wrote() {
        let (w, s) = (key("2026W"), key("2027S"));
        let mut doc = example();
        assert_eq!(doc.stored(), EXAMPLE);
        assert_eq!(PlanDoc::restored(EXAMPLE), doc);
        assert_eq!(PlanDoc::restored(&EXAMPLE.replace('\n', "\r\n")), doc, "a \\r is dropped");

        // Every tag, and a line of a newer build.
        assert!(doc.plan(s, "11113", 1_800_000_000, Some(1)));
        assert!(doc.plan(w, "11103", 5, Some(7)), "a pid the plan does not hold is no placeholder");
        assert!(!doc.plan(w, "12104", 6, None), "planned there already");
        doc.set_row(s, termin(150132, 0x0abcd), true);
        doc.remember(s, CODE);
        doc.extra.push("z\t2027S\twhat a newer build writes".to_string());
        let text = doc.stored();
        assert_eq!(PlanDoc::restored(&text), doc);
        for line in ["m\t2026W\t11103\t5\t\n", "m\t2027S\t11113\t1800000000\t1\n", "r\t2027S\t150132-0abcd\n", &format!("a\t2027S\t{CODE}\n")] {
            assert!(text.contains(line), "{line:?} in\n{text}");
        }
        assert!(text.ends_with("c\t2026W\t148369-a4d12\nr\t2027S\t150132-0abcd\na\t2027S\tCQpJeFAKchJKBgdlgf0e7Hwl_4S\nz\t2027S\twhat a newer build writes\n"));

        // What the page reads of it.
        assert!(!doc.is_empty());
        assert_eq!(doc.modules_in(w), ["12104", "12107", "12102", "11112", "11103"]);
        assert_eq!(doc.semesters(), [w, s]);
        assert_eq!(doc.planned_in("11113"), [s]);
        assert!(doc.is_planned(w, "11103") && !doc.is_planned(s, "11103"));
        assert_eq!(doc.fillers(1).iter().map(|m| m.module_id.as_str()).collect::<Vec<_>>(), ["11113"]);
        assert_eq!(doc.placeholders_in(w).len(), 1);
        assert!(doc.placeholders_in(s).is_empty());
        let selection = doc.selection(w, TownChoice::Only(Town::Cottbus));
        assert_eq!(selection.hidden_kinds, KindSet::default().with(EventKind::Tutorial));
        assert_eq!(selection.hidden_events, BTreeSet::from([149408]));
        assert_eq!(selection.chosen_rows, BTreeSet::from([termin(148369, 0xa4d12)]));
        assert!(selection.hidden_rows.is_empty());
        assert_eq!(selection.town, TownChoice::Only(Town::Cottbus));
        assert_eq!(doc.selection(key("2030S"), TownChoice::Derive), Selection::default());

        // Nothing planned, but the text keeps what it holds.
        let mut rest = doc.clone();
        rest.modules.clear();
        rest.placeholders.clear();
        assert!(rest.is_empty() && !rest.stored().is_empty());
        assert!(PlanDoc::default().stored().is_empty());
    }

    #[test]
    fn hostile_text_is_dropped_line_by_line() {
        let w = key("2026W");
        let mut text = String::new();
        for line in [
            "m\t2026W\t<script>alert(1)</script>\t1\t",
            "m\t2026X\t12104\t1\t",
            "m\t2026W\t12104\tsoon\tp1",
            "m\t2026W\t12104\t5\t",
            "p\t0\t2026W\t079-82-2008\t1\t1-1\t6\t\t\tpid 0",
            "p\t1\t2026W\t079-82-2008\t2\t1-1\t<b>6</b>\tbogus\t\tName\twith\ttabs",
            "p\t1\t2026W\t079-82-2008\t3\t1-1\t6\t\t\tthe same pid again",
            "p\t2\t2026W\t../../etc\t4\t1-1\t6\t\t\tno program id",
            "p\t3\t2026W\t079-82-2008\t5\t3-2\t6\t\t\ta span backwards",
            "p\t4\t2026W\t079-82-2008\t6\t1-21\t6\t\t\tpast the 20th semester",
            "p\t5\t2026W\t079-82-2008",
            "r\t2026W\t148369-zzzzz",
            "c\t2026W\t148369-AAF38",
            "e\t2026W\t0149408",
            "e\t2026W\t1234567890",
            "k\t2026W\tlecture,unknown,<script>",
            "a\t2026W\tnot a code!",
            "\t\t\t",
        ] {
            text.push_str(line);
            text.push('\n');
        }
        for n in 0..10_000 {
            text.push_str(&format!("junk {n}\n"));
        }
        let start = text.len();
        let mut event = 100_000_000u32;
        while text.len() - start < 200 * 1024 {
            text.push_str(&format!("e\t2026W\t{event}\n"));
            event += 1;
        }
        let doc = PlanDoc::restored(&text);
        assert!(!doc.is_empty());
        assert_eq!(doc.modules, vec![Planned { semester: w, module_id: "12104".to_string(), at: 0, fills: None }]);
        assert_eq!(
            doc.placeholders,
            vec![Placeholder { pid: 1, ord: 2, credits: None, kind: None, name: "Name".to_string(), ..placeholder() }],
            "the fields after the name are no part of it"
        );
        let hides = &doc.hidden[&w];
        assert_eq!(hides.kinds, KindSet::default().with(EventKind::Lecture));
        assert_eq!(hides.events.len(), MAX_HIDDEN, "the first 500 of one semester");
        assert_eq!(hides.events.first(), Some(&100_000_000));
        assert!(hides.rows.is_empty() && hides.chosen.is_empty());
        assert!(doc.subscribed.is_empty());
        assert_eq!(doc.extra.len(), 100);
        assert_eq!(doc.extra.first().map(String::as_str), Some("junk 0"));
        assert_eq!(PlanDoc::restored(&doc.stored()), doc);

        // Names never split their line, and stay at 250 characters.
        let mut doc = PlanDoc::default();
        doc.apply(&with_placeholders(vec![placeholder()]), 0);
        doc.placeholders[0].name = "Wahl\tpflicht\n<script>x</script>\r".to_string();
        doc.placeholders[0].caption = format!("{}\u{7}", "ä".repeat(300));
        let written = doc.stored();
        assert_eq!(written.lines().count(), 1, "{written}");
        let read = PlanDoc::restored(&written);
        assert_eq!(read.placeholders[0].name, "Wahl pflicht <script>x</script>");
        assert_eq!(read.placeholders[0].caption, "ä".repeat(250));
        assert_eq!(PlanDoc::restored(&read.stored()), read);
    }

    #[test]
    fn a_long_text_is_read_up_to_its_limit() {
        let mut text = String::from("m\t2026W\t11111\t1\t\n");
        while text.len() < MAX_STORED - 100 {
            text.push_str("m\tno semester\n");
        }
        // A line that ends 10 bytes before the limit, and a line across it: read up to the limit,
        // it would plan the module „33".
        let pad = MAX_STORED - 10 - text.len();
        text.push_str(&format!("m\t{}\n", "-".repeat(pad - 3)));
        assert_eq!(text.len(), MAX_STORED - 10);
        text.push_str("m\t2026W\t33333\t1\t\n");
        while text.len() < 600 * 1024 {
            text.push_str("m\t2026W\t22222\t1\t\n");
        }
        let doc = PlanDoc::restored(&text);
        assert_eq!(doc.modules_in(key("2026W")), ["11111"]);
        assert!(doc.extra.is_empty());
        // A text without a line break before the limit holds no line that could be whole.
        assert_eq!(PlanDoc::restored(&"x".repeat(MAX_STORED + 1)), PlanDoc::default());
    }

    #[test]
    fn a_store_full_at_every_cap_is_read_back_whole() {
        let semesters: Vec<SemesterKey> = (0..24).map(|n| key("2020W").plus(n).unwrap()).collect();
        let mut doc = PlanDoc::default();
        // The longest fields there are: 4-byte characters in names, 3-byte dashes in credits.
        let long = |n: usize| Placeholder {
            pid: 0,
            semester: semesters[n % 24],
            program_id: "ABCDEFGH-ABCDEFGH-ABCDEFGH-ABCDEFGH-ABCDEFGH".to_string(),
            ord: MAX_ORD - n as i64,
            span: (MAX_SPAN, MAX_SPAN),
            credits: Some("–".repeat(MAX_CREDITS)),
            kind: Some("internship".to_string()),
            caption: "😀".repeat(MAX_TEXT),
            name: "😀".repeat(MAX_TEXT + 10),
        };
        assert_eq!(doc.apply(&with_placeholders((0..MAX_PLACEHOLDERS).map(long).collect()), 0), (0, MAX_PLACEHOLDERS));
        assert_eq!(doc.apply(&with_placeholders(vec![Placeholder { ord: 1, ..long(0) }]), 0), (0, 0), "100 placeholders");
        let mut n = 0;
        for s in &semesters {
            for _ in 0..MAX_MODULES {
                if n < MAX_PLANNED {
                    assert!(doc.plan(*s, &format!("M{n:031}"), u64::MAX, Some(100)));
                    n += 1;
                }
            }
        }
        assert!(!doc.plan(semesters[0], "one-more", 0, None), "60 in a semester");
        assert!(!doc.plan(semesters[23], "one-more", 0, None), "400 in all");
        for (i, s) in semesters.iter().enumerate() {
            for kind in EventKind::ALL {
                doc.set_kind(*s, kind, true);
            }
            doc.remember(*s, &"~".repeat(MAX_CODE));
            for k in 0..MAX_HIDDEN as u32 {
                let n = i as u32 * MAX_HIDDEN as u32 + k;
                doc.set_event(*s, MAX_EVENT - n, true);
                doc.set_row(*s, termin(u32::MAX - n, 0xfffff), true);
                doc.choose(*s, u32::MAX - n, Some(termin(u32::MAX - n, 0xffffe)));
            }
        }
        let total = |pick: fn(&SemesterHides) -> usize| doc.hidden.values().map(pick).sum::<usize>();
        assert_eq!((total(|h| h.events.len()), total(|h| h.rows.len()), total(|h| h.chosen.len())), (2_000, 2_000, 2_000));
        assert_eq!(doc.hidden[&semesters[0]].events.len(), MAX_HIDDEN);
        assert!(!doc.hidden[&semesters[4]].kinds.is_empty() && doc.hidden[&semesters[4]].events.is_empty());
        let later = key("2020W").plus(24).unwrap();
        doc.set_kind(later, EventKind::Lecture, true);
        doc.remember(later, CODE);
        assert!(!doc.hidden.contains_key(&later) && !doc.subscribed.contains_key(&later), "24 semesters");
        for n in 0..MAX_EXTRA_LINES {
            doc.extra.push(format!("z{n:03}\t{}", "y".repeat(MAX_EXTRA_BYTES - 5)));
        }
        let text = doc.stored();
        assert!(text.len() < MAX_STORED, "{} bytes", text.len());
        assert_eq!(PlanDoc::restored(&text), doc);
    }

    #[test]
    fn a_plan_row_has_one_placeholder() {
        let text = "p\t1\t2026W\t079-82-2008\t15\t3-3\t6\telective\t\tAnwendungsfach\n\
            p\t2\t2027S\t079-82-2008\t15\t3-3\t6\telective\t\tAnwendungsfach\n\
            p\t3\t2027S\t048-82-2022\t15\t3-3\t6\telective\t\tAnwendungsfach\n";
        let mut doc = PlanDoc::restored(text);
        assert_eq!(doc.placeholders.iter().map(|p| (p.pid, p.semester.key())).collect::<Vec<_>>(), [(1, "2026W".to_string()), (3, "2027S".to_string())]);
        let twin = Placeholder { semester: key("2028S"), ord: 15, ..placeholder() };
        assert_eq!(doc.apply(&with_placeholders(vec![twin]), 0), (0, 0));
        assert_eq!(doc.next_pid(), 4);
    }

    #[test]
    fn unknown_tags_survive_a_write() {
        let text = "x-future\tsomething\nm\t2026W\t12104\t1\t\nq\t2026W\t1\n";
        let mut doc = PlanDoc::restored(text);
        assert!(doc.plan(key("2027S"), "11113", 2, None));
        doc.unplan(key("2026W"), "12104", &[]);
        assert_eq!(doc.stored(), "m\t2027S\t11113\t2\t\nx-future\tsomething\nq\t2026W\t1\n");
    }

    #[test]
    fn a_choice_replaces_the_one_before() {
        let w = key("2026W");
        let mut doc = PlanDoc::default();
        doc.choose(w, 148369, Some(termin(148369, 0xaaf38)));
        doc.choose(w, 148369, Some(termin(148369, 0xa4d12)));
        doc.choose(w, 148370, Some(termin(148370, 0x12345)));
        assert_eq!(doc.hidden[&w].chosen, BTreeSet::from([termin(148369, 0xa4d12), termin(148370, 0x12345)]));
        doc.choose(w, 148369, Some(termin(999, 1)));
        assert_eq!(doc.hidden[&w].chosen, BTreeSet::from([termin(148370, 0x12345)]), "a row of another event only clears the choice");
        doc.choose(w, 148370, None);
        assert!(doc.hidden.is_empty());
        assert_eq!(doc.stored(), "");
        // A text with two choices for one event keeps both; the next choice replaces them.
        let mut doc = PlanDoc::restored("c\t2026W\t148369-aaf38\nc\t2026W\t148369-a4d12\n");
        assert_eq!(doc.hidden[&w].chosen.len(), 2);
        doc.choose(w, 148369, Some(termin(148369, 0x467cf)));
        assert_eq!(doc.hidden[&w].chosen, BTreeSet::from([termin(148369, 0x467cf)]));
    }

    #[test]
    fn unplanning_a_module_drops_what_it_hid() {
        let w = key("2026W");
        let mut doc = PlanDoc::default();
        assert!(doc.plan(w, "12104", 1, None) && doc.plan(w, "12102", 1, None));
        doc.set_kind(w, EventKind::Tutorial, true);
        doc.set_event(w, 148369, true);
        doc.set_row(w, termin(148370, 0x1), true);
        doc.choose(w, 148369, Some(termin(148369, 0xa4d12)));
        doc.set_event(w, 149408, true);
        doc.unplan(w, "12104", &[148369, 148370]);
        assert_eq!(doc.modules_in(w), ["12102"]);
        let tutorials = KindSet::default().with(EventKind::Tutorial);
        assert_eq!(doc.hidden[&w], SemesterHides { kinds: tutorials, events: BTreeSet::from([149408]), ..Default::default() });
        doc.show_all(w);
        assert_eq!(doc.hidden[&w], SemesterHides { kinds: tutorials, ..Default::default() }, "kinds stay");
        doc.set_kind(w, EventKind::Tutorial, false);
        assert!(doc.hidden.is_empty());
        // Showing what was not hidden, or hiding an id no line can hold, changes nothing.
        doc.set_event(w, 1, false);
        doc.set_event(w, 0, true);
        doc.set_event(w, 1_000_000_000, true);
        doc.set_row(w, termin(148369, 1 << 20), true);
        assert!(doc.hidden.is_empty());
    }

    #[test]
    fn modules_move_and_fill_placeholders() {
        let (w, s) = (key("2026W"), key("2027S"));
        let mut doc = PlanDoc::default();
        assert_eq!(doc.next_pid(), 1);
        doc.apply(&with_placeholders(vec![placeholder()]), 0);
        assert!(doc.plan(w, "11103", 1, Some(1)));
        doc.move_to(w, s, "11103");
        assert_eq!(doc.modules, vec![Planned { semester: s, module_id: "11103".to_string(), at: 1, fills: Some(1) }]);
        // Planned there already: it only leaves the other semester.
        assert!(doc.plan(w, "11103", 2, None));
        doc.move_to(w, s, "11103");
        assert_eq!(doc.planned_in("11103"), [s]);
        assert_eq!(doc.modules[0].at, 1);
        doc.set_fills(s, "11103", None);
        assert_eq!(doc.fillers(1).len(), 0);
        doc.set_fills(s, "11103", Some(1));
        doc.remove_placeholder(1);
        assert!(doc.placeholders.is_empty() && doc.modules[0].fills.is_none());
        doc.set_fills(s, "11103", Some(1));
        assert!(doc.modules[0].fills.is_none(), "no placeholder of that pid");
        // A full semester takes no module from another.
        let mut full = PlanDoc::default();
        for n in 0..MAX_MODULES {
            full.plan(w, &format!("{}", 20_000 + n), 0, None);
        }
        full.plan(s, "11103", 0, None);
        full.move_to(s, w, "11103");
        assert_eq!(full.planned_in("11103"), [s]);
        // Past the largest pid, the smallest free one.
        let doc = PlanDoc::restored("p\t9999\t2026W\t079-82-2008\t1\t1-1\t6\t\t\tA\np\t1\t2026W\t079-82-2008\t2\t1-1\t6\t\t\tB\n");
        assert_eq!(doc.placeholders.iter().map(|p| p.pid).collect::<Vec<_>>(), [1, 9999]);
        assert_eq!(doc.next_pid(), 2);
    }

    /// A row of a plan named `caption` over the semesters `span`.
    fn plan_row(ord: i64, name: &str, span: (i64, i64), caption: &str) -> PlanEntry {
        PlanEntry {
            ord,
            semester: None,
            start_semester: Some(span.0),
            end_semester: Some(span.1),
            specialization: (!caption.is_empty()).then(|| caption.to_string()),
            ..row(name, 1, Some(ModuleKind::Elective), None)
        }
    }

    #[test]
    fn a_placeholder_finds_its_row_again() {
        let stored = Placeholder { ord: 15, span: (3, 3), caption: "Regelstudienplan".to_string(), ..placeholder() };
        let ord_of = |resolved: Resolved| match resolved {
            Resolved::Row(_, entry) => Some(entry.ord),
            _ => None,
        };
        let resolve = |entries: Vec<PlanEntry>| ord_of(resolve_placeholder(&stored, Some(&plan_variants(&entries, &[]))));
        let caption = "Regelstudienplan";
        // The same plan: the row of the same ord, whatever else is called so.
        assert_eq!(resolve(vec![plan_row(15, "Anwendungsfach", (3, 3), caption), plan_row(16, "Anwendungsfach", (4, 4), caption)]), Some(15));
        // A renamed caption: the only plan.
        assert_eq!(resolve(vec![plan_row(15, "Anwendungsfach", (3, 3), "Regelstudienplan 2024")]), Some(15));
        // Among several plans, none under the caption: the one with the most rows of the name,
        // spelled as it may be.
        let several = vec![
            plan_row(1, "Anwendungsfach", (1, 1), "A"),
            plan_row(15, "Anwendungsfach", (3, 3), "B"),
            plan_row(16, "–  anwendungsfach¹", (4, 4), "B"),
        ];
        let variants = plan_variants(&several, &[]);
        assert!(matches!(resolve_placeholder(&stored, Some(&variants)), Resolved::Row(plan, entry) if plan.full == "B" && entry.ord == 15));
        // A shifted ord: the only row of the name and the span …
        assert_eq!(
            resolve(vec![plan_row(1, "Neu", (1, 1), caption), plan_row(16, "Anwendungsfach", (3, 3), caption), plan_row(17, "Anwendungsfach", (4, 4), caption)]),
            Some(16)
        );
        // … else the only row of the name.
        assert_eq!(resolve(vec![plan_row(20, "Anwendungsfach", (5, 5), caption)]), Some(20));
        // Two rows of the name and neither of the span: no guess.
        assert_eq!(resolve(vec![plan_row(20, "Anwendungsfach", (5, 5), caption), plan_row(21, "Anwendungsfach", (6, 6), caption)]), None);
        // A renamed row, and several plans of which none is recognisable.
        assert_eq!(resolve_placeholder(&stored, Some(&plan_variants(&[plan_row(15, "Nebenfach", (3, 3), caption)], &[]))), Resolved::Changed);
        let strangers = plan_variants(&[plan_row(1, "Mathematik", (1, 1), "A"), plan_row(2, "Physik", (1, 1), "B")], &[]);
        assert_eq!(resolve_placeholder(&stored, Some(&strangers)), Resolved::Changed);
        assert_eq!(resolve_placeholder(&stored, Some(&[])), Resolved::Changed);
        // The program gone.
        assert_eq!(resolve_placeholder(&stored, None), Resolved::Gone);
    }

    #[test]
    fn a_placeholder_reads_like_the_catalogs_note() {
        let areas = real::areas(real::INFORMATIK_BSC);
        let rows = real::informatik_bsc_rows();
        let text = |entry: PlanEntry| {
            let requirement = plan::requirement_of(&entry, "", &areas, &rows);
            placeholder_line(&placeholder(), Some(&requirement)).text()
        };
        let named = |name: &str, kind: ModuleKind| row(name, 3, Some(kind), None);
        // A choice from the areas the name points at.
        let line = placeholder_line(&placeholder(), Some(&plan::requirement_of(&named("Anwendungsfach", ModuleKind::Elective), "", &areas, &rows)));
        assert_eq!(line.credits.as_deref(), Some("≥\u{a0}6\u{a0}LP"));
        assert_eq!(line.lead.as_deref(), Some("Anwendungsfach"));
        assert_eq!(line.tail, None);
        assert_eq!(
            line.text(),
            "≥\u{a0}6\u{a0}LP Anwendungsfach: „Mathematik“, „Maschinenbau / Elektrotechnik“, „Wirtschaftswissenschaften“, „Bauingenieurwesen“ oder „Physik“"
        );
        // One module the catalog does not know under this name.
        assert_eq!(
            text(named("Raumbezogene Datenbanken und GIS", ModuleKind::Compulsory)),
            "6\u{a0}LP Raumbezogene Datenbanken und GIS: unter diesem Namen nicht im Katalog"
        );
        // The FÜS: the finder's link is the list.
        assert_eq!(text(named("Fachübergreifendes Studium", ModuleKind::Fues)), "≥\u{a0}6\u{a0}LP Fachübergreifendes Studium");
        // A name that points at no area.
        assert_eq!(text(named("Wahlpflichtmodul 3", ModuleKind::Elective)), "≥\u{a0}6\u{a0}LP Wahlpflichtmodul 3: alle Wahlpflichtmodule");
        // A name that only repeats its area, and a range.
        assert_eq!(text(named("Modul aus dem Bereich Praktische Mathematik", ModuleKind::Elective)), "≥\u{a0}6\u{a0}LP aus dem Bereich: „Praktische Mathematik“");
        let range = PlanEntry { credits: None, min_credits: Some(10.0), max_credits: Some(24.0), ..named("Komplex Praktische Informatik", ModuleKind::Elective) };
        assert_eq!(text(range), "10–24\u{a0}LP aus dem Bereich: „Praktische Informatik“");

        // Without its row: the stored text, and no claim where the modules come from.
        let alone = |p: Placeholder| placeholder_line(&p, None).text();
        assert_eq!(alone(placeholder()), "≥\u{a0}6\u{a0}LP Anwendungsfach");
        assert_eq!(
            alone(Placeholder { kind: Some("compulsory".to_string()), name: "Raumbezogene Datenbanken und GIS".to_string(), ..placeholder() }),
            "6\u{a0}LP Raumbezogene Datenbanken und GIS: unter diesem Namen nicht im Katalog"
        );
        assert_eq!(alone(Placeholder { name: "Modul aus dem FÜS-Katalog der BTU".to_string(), ..placeholder() }), "≥\u{a0}6\u{a0}LP Fachübergreifendes Studium");
        assert_eq!(alone(Placeholder { credits: Some("10–24".to_string()), name: "Komplex Praktische Informatik".to_string(), ..placeholder() }), "10–24\u{a0}LP Komplex Praktische Informatik");
        assert_eq!(alone(Placeholder { credits: None, ..placeholder() }), "Anwendungsfach");
    }

    #[test]
    fn the_default_semester_waits_for_the_winters_exams() {
        let (w, s) = (key("2026W"), key("2027S"));
        let mut doc = PlanDoc::default();
        doc.plan(w, "12104", 0, None);
        let day = Day::parse;
        assert_eq!(default_semester(s, &doc, day("2027-03-01")), w);
        assert_eq!(default_semester(s, &doc, day("2027-03-31")), w);
        assert_eq!(default_semester(s, &doc, day("2027-04-02")), s);
        assert_eq!(default_semester(s, &PlanDoc::default(), day("2027-03-01")), s, "nothing planned in the winter");
        assert_eq!(default_semester(s, &doc, None), s);
        assert_eq!(default_semester(w, &doc, day("2026-10-01")), w);
    }

    const MINE: &str = "program\t048-82-2022\n\
        name\tElektrotechnik B.Sc. · PO 2022\n\
        caption\tRegelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium\n\
        start\t2026W\n\
        town\tcottbus\n";

    #[test]
    fn my_program_reads_back_what_it_wrote() {
        let doc = MineDoc::restored(MINE);
        assert_eq!(
            doc,
            MineDoc {
                program: Some("048-82-2022".to_string()),
                name: Some("Elektrotechnik B.Sc. · PO 2022".to_string()),
                caption: Some("Regelstudienplan der Studienrichtungen MIT und EET im grundständigen Studium".to_string()),
                direction: None,
                start: Some(key("2026W")),
                town: TownChoice::Only(Town::Cottbus),
                extra: Vec::new(),
            }
        );
        assert_eq!(doc.stored(), MINE);
        let mut cleared = doc.clone();
        cleared.clear_program();
        assert_eq!(cleared.stored(), "start	2026W
town	cottbus
", "Studienbeginn and town stay");
        // A core plan and its page, the unnamed plan's caption, both towns, a key of a newer build.
        let doc = MineDoc {
            program: Some("370-82-2023".to_string()),
            name: Some("Wirtschaftsingenieurwesen B.Sc. · PO 2023".to_string()),
            caption: Some(String::new()),
            direction: Some("Studienplan · Seite 18".to_string()),
            town: TownChoice::Both,
            extra: vec![("theme".to_string(), "dark\tand quiet".to_string())],
            ..doc
        };
        let text = doc.stored();
        assert!(text.contains("caption\t\n") && text.contains("direction\tStudienplan · Seite 18\n") && text.contains("town\tboth\n"));
        assert_eq!(MineDoc::restored(&text), doc);
        // The first valid line of a key wins.
        let doc = MineDoc::restored("program\t../x\nprogram\t079-82-2008\nprogram\t048-82-2022\nstart\t2026X\nstart\t2027s\ntown\tmars\ntown\tsenftenberg\ntown\tboth\nname\t\nname\tInformatik\r\n");
        assert_eq!(doc.program.as_deref(), Some("079-82-2008"));
        assert_eq!(doc.start, Some(key("2027S")));
        assert_eq!(doc.town, TownChoice::Only(Town::Senftenberg));
        assert_eq!(doc.name.as_deref(), Some("Informatik"));
        assert_eq!((doc.caption, doc.direction), (None, None));
        // Read up to 4 KB; a derived town is not written.
        let long = format!("program\t079-82-2008\n{}start\t2026W\n", "x\ty\n".repeat(2000));
        let doc = MineDoc::restored(&long);
        assert_eq!((doc.program.as_deref(), doc.start), (Some("079-82-2008"), None));
        assert_eq!(doc.extra, vec![("x".to_string(), "y".to_string())]);
        assert_eq!(MineDoc::default().stored(), "");
    }

    fn catalog_row(id: &str, turnus: &str) -> CatalogRow {
        CatalogRow {
            id: id.to_string(),
            title: id.to_string(),
            title_de: None,
            title_en: None,
            credits: None,
            turnus_season: Some(Code::parse(turnus)),
            turnus_parity: None,
            offer_status: Code::parse("active"),
            teaches_german: None,
            teaches_english: None,
            is_fues: false,
            is_limited: None,
            department: None,
            teaching_events: 0,
            exam_form: None,
            responsible: None,
            kind: None,
            plan_semester: None,
            area: None,
        }
    }

    #[test]
    fn the_intake_season_follows_the_odd_semesters() {
        let linked = |id: &str, semester: i64| PlanEntry { module_id: Some(id.to_string()), ..row(id, semester, Some(ModuleKind::Compulsory), None) };
        let rows = [catalog_row("A", "winter"), catalog_row("B", "winter"), catalog_row("C", "summer"), catalog_row("D", "summer"), catalog_row("E", "both"), catalog_row("F", "summer")];
        let season = |entries: Vec<PlanEntry>| intake_season(&plan_variants(&entries, &[]).remove(0), &rows);
        // Two winters to one summer; an even semester and a module of both seasons do not count.
        assert_eq!(season(vec![linked("A", 1), linked("B", 3), linked("C", 3), linked("D", 2), linked("E", 1)]), Some(Season::Winter));
        // A module the plan names twice counts once.
        assert_eq!(season(vec![linked("A", 1), linked("C", 3), linked("C", 5)]), None);
        assert_eq!(season(vec![linked("A", 1), linked("C", 3), linked("F", 5)]), Some(Season::Summer));
        assert_eq!(season(vec![linked("A", 1), linked("B", 3), linked("C", 3), linked("F", 5)]), None);
        assert_eq!(season(vec![linked("E", 1), linked("D", 2)]), None);
        let w = key("2026W");
        assert_eq!(intake_start(w, Some(Season::Winter)), w);
        assert_eq!(intake_start(w, Some(Season::Summer)), key("2026S"));
        assert_eq!(intake_start(w, None), w);
        assert_eq!(intake_start(key("2027S"), Some(Season::Winter)), w);
    }

    #[test]
    fn an_import_places_rows_by_their_fachsemester() {
        let module = |ord: i64, id: &str, span: (i64, i64)| PlanEntry { module_id: Some(id.to_string()), ..plan_row(ord, &format!("Modul {id}"), span, "") };
        let entries = vec![
            module(1, "10001", (1, 1)),
            module(2, "10002", (2, 2)),
            module(3, "10001", (3, 3)),
            PlanEntry { credits: None, min_credits: Some(10.0), max_credits: Some(24.0), ..plan_row(4, "- Wahlpflicht*", (2, 4), "") },
            PlanEntry { credits: None, kind: None, ..plan_row(5, "Hinweis zum Auslandssemester", (1, 1), "") },
            module(6, "10003", (5, 6)),
        ];
        let core = plan_variants(&entries, &[]).remove(0);
        let (w, program) = (key("2026W"), "079-82-2008");

        // From the third Fachsemester: what ends before it is left out, a span stands at the first
        // semester taken.
        let third = import(&PlanDoc::default(), program, &core, None, w, 3);
        assert_eq!(third.modules, [(key("2027W"), "10001".to_string()), (key("2028W"), "10003".to_string())]);
        assert_eq!(
            third.placeholders,
            [Placeholder { semester: key("2027W"), ord: 4, span: (2, 4), credits: Some("10–24".to_string()), name: "Wahlpflicht".to_string(), ..placeholder() }]
        );
        assert_eq!(third.skipped, 0);
        assert_eq!(
            third.by_fs,
            [
                ImportFs { fs: 3, semester: key("2027W"), modules: vec!["Modul 10001".to_string()], placeholders: vec!["Wahlpflicht".to_string()] },
                ImportFs { fs: 5, semester: key("2028W"), modules: vec!["Modul 10003".to_string()], placeholders: Vec::new() },
            ]
        );

        // From the first: a module the plan names twice once, at its first semester; prose never.
        let first = import(&PlanDoc::default(), program, &core, None, w, 0);
        assert_eq!(first.modules.iter().map(|(s, id)| (s.key(), id.as_str())).collect::<Vec<_>>(), [("2026W".to_string(), "10001"), ("2027S".to_string(), "10002"), ("2028W".to_string(), "10003")]);
        assert_eq!(first.placeholders.iter().map(|p| (p.semester, p.ord)).collect::<Vec<_>>(), [(key("2027S"), 4)]);
        assert_eq!(first.skipped, 0);

        // Twice: nothing new, and every row that would have been counted.
        let mut doc = PlanDoc::default();
        assert_eq!(doc.apply(&first, 7), (3, 1));
        assert_eq!(doc.placeholders[0].pid, 1);
        let again = import(&doc, program, &core, None, w, 1);
        assert_eq!((again.modules.len(), again.placeholders.len(), again.skipped), (0, 0, 5));
        assert!(again.by_fs.is_empty());
        assert_eq!(doc.apply(&again, 8), (0, 0));

        // A page that fills row 4: its rows come in, row 4 does not.
        let page_rows = vec![PlanEntry { specialization: Some("Studienplan · Seite 7".to_string()), ..module(10, "10010", (2, 2)) }];
        let page = plan_variants(&page_rows, &[]).remove(0);
        let both = import(&PlanDoc::default(), program, &core, Some((&page, 4)), w, 1);
        assert!(both.placeholders.is_empty());
        assert!(both.modules.contains(&(key("2027S"), "10010".to_string())));
    }

    /// The design's pinned imports (C.19) on the snapshot they were taken from; on any snapshot,
    /// every plan of every program taken over twice adds nothing the second time, reads back
    /// whole, and finds each of its placeholders' rows again.
    #[test]
    fn the_regelstudienplan_is_imported() {
        let pinned = crate::tests::studyplan_db("the_regelstudienplan_is_imported");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(crate::tests::open);
        let plans = |program: &str| plan_variants(&queries::program_plan_entries(&db, program).unwrap(), &queries::program_plan_totals(&db, program).unwrap());
        let w = key("2026W");

        let mut imported = 0;
        for program in queries::programs(&db).unwrap().into_iter().filter(|program| program.has_plan) {
            let variants = plans(&program.id);
            let pages = supplements(&variants);
            for (i, core) in variants.iter().enumerate() {
                let page = pages.iter().find(|supplement| supplement.core == i).map(|supplement| (&variants[supplement.page], supplement.ord));
                let mut doc = PlanDoc::default();
                let first = import(&doc, &program.id, core, page, w, 1);
                let listed: usize = first.by_fs.iter().map(|line| line.modules.len() + line.placeholders.len()).sum();
                assert_eq!(listed, first.modules.len() + first.placeholders.len(), "{} {}", program.id, core.full);
                assert_eq!(doc.apply(&first, 1), (first.modules.len(), first.placeholders.len()), "{} {}", program.id, core.full);
                let again = import(&doc, &program.id, core, page, w, 1);
                assert!(again.modules.is_empty() && again.placeholders.is_empty(), "{} {}", program.id, core.full);
                assert!(again.skipped >= first.modules.len() + first.placeholders.len());
                assert_eq!(PlanDoc::restored(&doc.stored()), doc, "{} {}", program.id, core.full);
                for p in &doc.placeholders {
                    assert!(matches!(resolve_placeholder(p, Some(&variants)), Resolved::Row(_, entry) if entry.ord == p.ord), "{} {}: {}", program.id, core.full, p.name);
                }
                imported += 1;
            }
        }
        assert!(imported > 100, "{imported} plans");
        if !is_pinned {
            return;
        }

        // Informatik B.Sc. 2008, started 2026W, from the first semester.
        let informatik = plans("079-82-2008");
        let [plan] = informatik.as_slice() else { panic!("Informatik prints one plan") };
        let fs1 = import(&PlanDoc::default(), "079-82-2008", plan, None, w, 1);
        assert_eq!((fs1.modules.len(), fs1.placeholders.len(), fs1.skipped), (13, 10, 0));
        assert_eq!(fs1.modules.iter().filter(|(s, _)| *s == w).map(|(_, id)| id.as_str()).collect::<Vec<_>>(), ["12104", "12107", "12102", "11112"]);
        assert_eq!(fs1.placeholders.iter().filter(|p| p.semester == w).map(|p| p.ord).collect::<Vec<_>>(), [17]);
        assert_eq!(
            fs1.by_fs.first(),
            Some(&ImportFs {
                fs: 1,
                semester: w,
                modules: ["Entwicklung von Softwaresystemen", "Elektrische und elektronische Grundlagen der Informatik", "Programmierpraktikum", "Mathematik IT-1 (Diskrete Mathematik)"]
                    .map(String::from)
                    .to_vec(),
                placeholders: vec!["Fachübergreifendes Studium".to_string()],
            })
        );
        // Its first semester alone, with one hidden kind, one hidden event and one choice, is the
        // design's example of the stored text.
        let only_fs1 = Import {
            modules: fs1.modules.iter().filter(|(s, _)| *s == w).cloned().collect(),
            placeholders: fs1.placeholders.iter().filter(|p| p.semester == w).cloned().collect(),
            ..Default::default()
        };
        let mut doc = PlanDoc::default();
        assert_eq!(doc.apply(&only_fs1, 1_790_000_000), (4, 1));
        doc.set_kind(w, EventKind::Tutorial, true);
        doc.set_event(w, 149408, true);
        doc.choose(w, 148369, Some(termin(148369, 0xa4d12)));
        assert_eq!(doc.stored(), EXAMPLE);

        // Started 2025W, from the third semester.
        let fs3 = import(&PlanDoc::default(), "079-82-2008", plan, None, key("2025W"), 3);
        let modules = |s: &str| fs3.modules.iter().filter(|(k, _)| *k == key(s)).map(|(_, id)| id.clone()).collect::<Vec<_>>();
        let rows = |s: &str| fs3.placeholders.iter().filter(|p| p.semester == key(s)).map(|p| p.ord).collect::<Vec<_>>();
        assert_eq!((modules("2026W"), modules("2027S"), modules("2027W")), (vec!["11787".to_string(), "12202".to_string(), "11213".to_string()], vec!["12204".to_string(), "12214".to_string()], vec!["12333".to_string()]));
        assert_eq!((rows("2026W"), rows("2027S"), rows("2027W")), (vec![15], vec![14, 16, 18], vec![19, 20, 21, 22]));
        assert_eq!(fs3.modules.len() + fs3.placeholders.len(), 14);

        // Twice adds nothing, also after a placeholder was filled.
        let mut doc = PlanDoc::default();
        doc.apply(&fs1, 1);
        let anwendungsfach = doc.placeholders.iter().find(|p| p.ord == 15).unwrap().pid;
        assert!(doc.plan(key("2027W"), "11103", 2, Some(anwendungsfach)));
        let again = import(&doc, "079-82-2008", plan, None, w, 1);
        assert_eq!(again, Import { skipped: 23, ..Default::default() });

        // Informatik starts in the winter.
        let ids: Vec<String> = plan.entries.iter().filter_map(|entry| entry.module_id.clone()).collect();
        let linked = pages::bookmarks(&db, &ids, BookmarkSort::Added, false).unwrap().rows;
        assert_eq!(intake_season(plan, &linked), Some(Season::Winter));

        // Wirtschaftsingenieurwesen 2023: the core plan with „Seite 18", which fills core row 16.
        let wing = plans("370-82-2023");
        let supplement = supplements(&wing).into_iter().find(|supplement| wing[supplement.page].full == "Studienplan · Seite 18").unwrap();
        assert_eq!(supplement.ord, 16);
        let (core, page) = (&wing[supplement.core], &wing[supplement.page]);
        let both = import(&PlanDoc::default(), "370-82-2023", core, Some((page, 16)), w, 1);
        assert!(!both.placeholders.iter().any(|p| p.ord == 16), "the page is what fills row 16");
        let semester_of = |id: &str| both.modules.iter().find(|(_, module)| module == id).map(|(s, _)| s.key());
        let placed: Vec<Option<String>> = ["11915", "12981", "31102", "11675", "36308"].map(semester_of).to_vec();
        assert_eq!(placed, ["2026W", "2027S", "2027W", "2027W", "2028S"].map(|s| Some(s.to_string())).to_vec());
        let row24 = both.placeholders.iter().find(|p| p.ord == 24).unwrap();
        assert_eq!((row24.span, row24.semester, row24.caption.as_str()), ((4, 6), key("2028S"), "Studienplan · Seite 18"));
        // „offen": row 16 stays, one placeholder over the six semesters, and no page comes in.
        let open = import(&PlanDoc::default(), "370-82-2023", core, None, w, 1);
        let row16: Vec<&Placeholder> = open.placeholders.iter().filter(|p| p.ord == 16).collect();
        assert_eq!(row16.iter().map(|p| (p.span, p.semester)).collect::<Vec<_>>(), [((1, 6), w)]);
        assert!(semester_of("11915").is_some() && !open.modules.iter().any(|(_, id)| id == "11915"));
    }
}
