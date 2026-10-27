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

use folia_calendar::day::{clock, minutes, Day};
use folia_calendar::kind::{EventKind, KindSet};
use folia_calendar::own::{OwnRhythm, OwnTermin, MAX_OWN_DATES, MAX_OWN_NUMBER, MAX_OWN_TEXT};
use folia_calendar::rowkey::RowKey;
use folia_calendar::select::{MAX_HIDDEN, MAX_MODULES, Selection, Town, TownChoice};
use folia_calendar::semester::{of_fachsemester, SemesterKey};
use folia_calendar::subscription::MAX_CODE;
use folia_locale::Locale;
use folia_model::ids::MAX_PLANNED;
use folia_model::labels::{Code, Labelled, ModuleKind, Season, TurnusSeason};
use folia_model::rows::CatalogRow;
use folia_model::rows_detail::PlanEntry;
use folia_routes::url;

use crate::plan::{self, SemesterRequirement};
use crate::variants::{self, PlanVariant};

/// The most bytes of `betula.studyplan.v1` that are read. What Betula writes within the caps stays
/// below it (a test fills a store to every cap); a longer text is read up to the last line break
/// before the limit, so a store never reads as empty because of its size and the next change
/// cannot wipe a plan. What lies beyond is dropped with the next write, which only text Betula did
/// not write can lose. 512 KiB until 2026-10-26, when the hidden dates (`h`) came, and 640 KiB
/// until 2026-10-27, when the Termine of one's own (`t`) came: a build before them reads the first
/// 640 KiB, which hold everything but a store full to the new caps.
pub const MAX_STORED: usize = 1024 * 1024;

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

/// The most modules marked as passed: as many as a plan may hold.
pub const MAX_PASSED: usize = MAX_PLANNED;

/// The most rows marked as done: as many as a plan may hold placeholders.
pub const MAX_DONE_ROWS: usize = MAX_PLACEHOLDERS;

/// The most calendars of one's own beside „Mein Studiengang" (`Calendar`): as many as there are
/// profiles, more than a week can show beside the plan.
pub const MAX_CALENDARS: usize = 8;

/// The most characters of a calendar's name, as of a profile's.
pub const MAX_CALENDAR_NAME: usize = 40;

/// The most modules of one calendar in all its semesters together, and the most groups ticked in
/// it: a calendar is a semester or two of somebody's plan, and eight full ones keep the store
/// within its limit.
pub const MAX_CALENDAR_MODULES: usize = 60;
pub const MAX_CALENDAR_TICKS: usize = 200;

/// The largest number of a calendar: two digits keep its lines short.
const MAX_CALENDAR_ID: u8 = 99;

/// The most Termine of one's own (`t`) of one calendar in one semester, and of all calendars and
/// semesters together (folia/design/kalender/termine.html: „im Vorschlag 100 eigene Termine im
/// Semester"). A week holds a handful; the caps keep the store within its limit.
pub const MAX_OWN: usize = 100;
pub const MAX_OWN_ALL: usize = 200;

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

/// The sixth field of an `m` line for a module taken over from the Regelstudienplan.
const FROM_PLAN: &str = "plan";

/// What `PlanDoc::holds_saved` compares of a semester: the modules, the visits, the placeholders'
/// plan rows, what is hidden or chosen, and the Termine of one's own without their numbers.
type Shape<'a> = (BTreeSet<&'a str>, BTreeSet<&'a str>, BTreeSet<(&'a str, i64)>, Option<&'a SemesterHides>, BTreeSet<OwnTermin>);

/// The visitor's Studienplan: modules and placeholders per calendar semester, and per semester
/// what is hidden, what was chosen and the subscription code last handed out.
///
/// Stored as lines (`stored`), the fields separated by a tab (two spaces here), the first field a
/// tag:
///
/// ```text
/// g  <program_id>
/// m  <semester>  <module_id>  <added_secs>  <fills>  [plan]
/// v  <semester>  <module_id>  <added_secs>
/// p  <pid>  <semester>  <program_id>  <ord>  <from>-<to>  <credits>  <kind>  <caption>  <name>
/// k  <semester>  <kind>,<kind>,…
/// e  <semester>  <event_id>
/// r  <semester>  <event_id>-<fp 5 hex>
/// c  <semester>  <event_id>-<fp 5 hex>
/// h  <semester>  <event_id>-<fp 5 hex>  <date>,<date>,…
/// a  <semester>  <code>
/// d  <semester>  <module_id>
/// q  <semester>  <program_id>  <ord>  <caption>  <name>
/// l  <calendar>  <view>  <program_id>  <name>
/// n  <calendar>  <semester>  <module_id>
/// o  <calendar>  <semester>  <event_id>-<fp 5 hex>
/// t  <number>  <calendar>  <semester>  <rhythm>  <weekday|dates>  <from>-<to>  <first>  <last>  <module>  <name>  <place>  <skip>
/// ```
///
/// `g` is the program the timetable is planned for (`program`): in a saved plan, not in the text
/// the browser keeps of the Stundenplan (`stored_without_program`). `plan` marks a module taken over
/// from the Regelstudienplan (`Planned::from_plan`). `r` hides one Termin, `c` ticks an option of a
/// choice: of the choice that holds the row, the options ticked are shown, as many as there are
/// lines (owner, 2026-10-26: several groups, all alike). A tick whose row QIS changed no longer
/// matches, and with none left the choice is open again, the safe direction. `h` hides single
/// dates of one Termin („Nur am 04.11. ausblenden", 2026-10-26), its days joined by commas as `k`
/// joins its kinds; a build before it keeps the lines as lines of a tag it does not know.
///
/// The `m` and `p` lines are the semesters of „Mein Studium" as well (`study`): a semester's
/// timetable is its part of the study, one plan for both pages.
///
/// `v` is a module only visited in a semester („nur besuchen", `Visit`): in its week, counting
/// for nothing. A module is planned or visited in a semester, never both. A build before it keeps
/// the lines as lines of a tag it does not know: it shows the visit nowhere and counts nothing of
/// it, the safe direction, and the next build has it again.
///
/// `d` and `q` are what „Mein Studium" counts as done (`study`, owner 2026-10-04): a module passed
/// („bestanden") and a row of a Regelstudienplan without a module that is done („erledigt"), each
/// with the semester it was done in. They belong to no timetable: „Plan leeren" leaves them, and a
/// saved or a shared timetable does not carry them. A build before them keeps them as lines of a
/// tag it does not know.
///
/// `l`, `n` and `o` are the calendars of one's own beside „Mein Studiengang" (`Calendar`, owner
/// 2026-10-26): a calendar by its number with how it stands in the week (`full`, `strip`, `off`,
/// empty for as a new one stands), its program (empty for none) and its name; a module of it in a
/// semester; a group ticked in it. They belong to no timetable of the plan's own: „Plan leeren", a
/// saved plan and a shared one leave them out. A build before them keeps them as lines of a tag it
/// does not know.
///
/// `t` is a Termin of one's own (`Own`, folia/design/kalender/termine.html; owner, 2026-10-27:
/// „Passt alles, bau Schritt 1 jetzt in Folia"): its number (its key, `OwnTermin::key`), its
/// calendar (0 the plan's, else a calendar of one's own by its number, whose `l` line comes
/// first), its semester, how it recurs (`w`, `a`, `b`, or `d` on dates of its own), its weekday or
/// its dates joined by commas, its time, the first and the last day of one that recurs where they
/// are not the lecture period's (else empty), the module it belongs to (empty for none), its name
/// and place, and the days it is not held on („Nur am … ausblenden"), joined by commas. The plan's
/// go with its semester: „Plan leeren" takes them, a saved plan carries them; a calendar's go with
/// it. A build before them keeps them as lines of a tag it does not know (at most 100 of them).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanDoc {
    /// The program the timetable is planned for, picked in its sidebar or taken with a
    /// Regelstudienplan (`g`); `None`: „Mein Studiengang". It gives the import its plans. A saved
    /// or a shared plan carries it; the browser keeps the Stundenplan's for the session alone,
    /// not in the plan's text (`stored_without_program`, owner 2026-10-15).
    pub program: Option<String>,
    /// By semester, and within one in the order they were planned (`m`).
    pub modules: Vec<Planned>,
    /// The modules only visited, by semester, and within one in the order they came (`v`).
    pub visits: Vec<Visit>,
    /// By pid (`p`).
    pub placeholders: Vec<Placeholder>,
    /// What each semester hides and has chosen (`k`, `e`, `r`, `c`); no semester without any.
    pub hidden: BTreeMap<SemesterKey, SemesterHides>,
    /// The code of the subscription last handed out, per semester (`a`).
    pub subscribed: BTreeMap<SemesterKey, String>,
    /// The modules passed, one each, in the order they were marked (`d`).
    pub passed: Vec<Passed>,
    /// The rows of a Regelstudienplan without a module that are done, one per row (`q`).
    pub done_rows: Vec<DoneRow>,
    /// The calendars of one's own, in the order they were made (`l`, `n`, `o`, their `t`).
    pub calendars: Vec<Calendar>,
    /// The Termine of one's own of the plan's calendar, „Mein Studiengang", in the order they came
    /// (`t` of calendar 0).
    pub own: Vec<Own>,
    /// Lines of tags this build does not know, as they were read.
    pub extra: Vec<String>,
}

/// How a calendar of one's own stands in the week (folia/design/kalender, owner 2026-10-26: „A
/// mit Fokus und Spur aus C"): in full beside the plan's Termine, as a strip at the edge of each
/// day, or not at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CalendarView {
    Full,
    Strip,
    Off,
}

impl CalendarView {
    pub const ALL: [Self; 3] = [Self::Full, Self::Strip, Self::Off];

    /// The word its line keeps.
    pub fn code(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Strip => "strip",
            Self::Off => "off",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.code() == code)
    }
}

/// A calendar of one's own beside „Mein Studiengang" (owner, 2026-10-26: „Kallender, die die
/// nutzer selbst erstellen können, um zum beispiel für ihre freunde einen speraten kallender zu
/// haben"): its modules per semester and the groups ticked in it, each calendar with a choice of
/// its own („Ein Modul darf in zwei Kalendern stehen, jeder mit seiner Gruppenwahl"). Nothing of
/// it counts for the study: it is a calendar, not a plan (owner: „module, die nicht in Mein
/// studium eingetragen werden zählen nicht zum studium").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Calendar {
    /// 1–99, one per calendar of the plan: its lines go by it, and its colour.
    pub id: u8,
    /// At most `MAX_CALENDAR_NAME` characters; a page names one without a name by its number.
    pub name: String,
    /// How it stands in the week; `None` as a new one stands: in full while it holds at most
    /// eight Termine a week, as a strip beyond (owner, 2026-10-26), until its switch says.
    pub view: Option<CalendarView>,
    /// The program whose abbreviations name its modules and whose courses it takes (a shared
    /// plan's), where it has one.
    pub program: Option<String>,
    /// Its modules by semester, in the order they came.
    pub modules: Vec<(SemesterKey, String)>,
    /// The groups ticked in it, by semester (as `SemesterHides::chosen` of the plan's own).
    pub chosen: BTreeSet<(SemesterKey, RowKey)>,
    /// Its Termine of one's own, in the order they came (its `t` lines).
    pub own: Vec<Own>,
}

/// A Termin of one's own in a semester (`t`): of the plan's calendar (`PlanDoc::own`) or of one of
/// one's own (`Calendar::own`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Own {
    pub semester: SemesterKey,
    pub termin: OwnTermin,
}

/// A Termin of one's own taken away (`PlanDoc::remove_own`): its calendar, where it stood in its
/// list, and it, for „Rückgängig" (`PlanDoc::put_own`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovedOwn {
    pub calendar: u8,
    pub at: usize,
    pub own: Own,
}

impl Calendar {
    /// Its modules in semester `s`, in the order they came.
    pub fn modules_in(&self, s: SemesterKey) -> Vec<String> {
        self.modules.iter().filter(|(semester, _)| *semester == s).map(|(_, id)| id.clone()).collect()
    }

    /// What it shows of semester `s`: its own ticks and its Termine of one's own, in the town its
    /// modules are held in.
    pub fn selection(&self, s: SemesterKey) -> Selection {
        let chosen_rows = self.chosen.iter().filter(|(semester, _)| *semester == s).map(|(_, row)| *row).collect();
        Selection { chosen_rows, own: own_of_semester(&self.own, s), ..Selection::default() }
    }

    /// Adds a module in semester `s`, within the caps; `false` where nothing changed.
    fn add_module(&mut self, s: SemesterKey, id: &str) -> bool {
        let held = self.modules.iter().any(|(semester, module)| *semester == s && module == id);
        if !url::is_module_id(id) || held || self.modules.len() >= MAX_CALENDAR_MODULES {
            return false;
        }
        self.modules.push((s, id.to_string()));
        true
    }

    /// Ticks a group in semester `s` or takes the tick away, within the cap.
    fn set_tick(&mut self, s: SemesterKey, row: RowKey, on: bool) {
        if !on {
            self.chosen.remove(&(s, row));
        } else if is_row(row) && self.chosen.len() < MAX_CALENDAR_TICKS {
            self.chosen.insert((s, row));
        }
    }
}

/// A module the student passed („bestanden"), and the semester they passed it in (`d`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passed {
    pub semester: SemesterKey,
    pub module_id: String,
}

/// A row of a Regelstudienplan without a module that the student has done („erledigt": an
/// elective whose credits are reached, the FÜS, a module the catalog does not know under the
/// row's name), and the semester it was done in (`q`). Like a placeholder it keeps the row's
/// caption and name, so that it finds its row again when a later snapshot numbers the rows anew
/// (`study`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoneRow {
    pub semester: SemesterKey,
    pub program_id: String,
    /// The row's `PlanEntry::ord`.
    pub ord: i64,
    /// The caption of the plan the row comes from; `""` for the one unnamed plan of a program.
    pub caption: String,
    /// The row's name as a page shows it (`plan::shown_name`).
    pub name: String,
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
    /// Taken over from a row of the Regelstudienplan (`apply`), stored as a sixth field `plan`.
    /// A derived Standort is taken from these (`selection`). A build before this field writes the
    /// line back without it, and the Standort is then derived from every module, as it was.
    pub from_plan: bool,
}

/// A module only visited in a semester („nur besuchen", decided 2026-10-19: „Ein geplantes Modul
/// lässt sich auf „nur besuchen“ stellen: in der Woche ja, in den LP nicht"; 2026-10-26: „+ Modul"
/// in „Mein Studiengang" makes one, „Besuchmodul kann pro kallender gemacht werden … Aber beim
/// Studien Plan zählt es dann als besucher modul"). Its Termine stand in the week of its semester
/// and give way where they meet the plan's; it counts no credits, for no area and no placeholder,
/// and is never passed. „Mein Studium" lists it apart, where its „Nur besuchen" taken away makes
/// it a planned module (`count_visit`), and „Nur besuchen" on a planned one makes it a visit
/// (`make_visit`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visit {
    pub semester: SemesterKey,
    pub module_id: String,
    /// When it came, in seconds since 1970; 0 where unknown.
    pub at: u64,
}

/// A row of a Regelstudienplan without a module, taken over into a semester under its own name.
/// It keeps its own text, so it always shows whatever became of the plan since, and
/// `resolve_placeholder` finds its row again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placeholder {
    /// A number of this browser's own, 1–9999 and unique in the plan; `fill=p<pid>` names it.
    pub pid: u32,
    /// Where it stands: the semester it was taken over into.
    pub semester: SemesterKey,
    pub program_id: String,
    /// The row's `PlanEntry::ord`. With the program it makes a placeholder one row of one plan,
    /// and a row has one placeholder per semester at most: a row over several Fachsemester („5.–6.
    /// FS") stands in each semester one of them was taken over into (owner, 2026-09-26: not in the
    /// first alone), and what counts for it in all of them together is what the row asks for
    /// (`PlanDoc::same_row`).
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

/// What one semester hides (`k`, `e`, `r`, `h`) and has ticked (`c`). Named apart from
/// `timetable::select::HiddenBy`, which says why something is not shown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SemesterHides {
    /// Kinds switched off; an event is hidden only when all its kinds are.
    pub kinds: KindSet,
    pub events: BTreeSet<u32>,
    pub rows: BTreeSet<RowKey>,
    pub chosen: BTreeSet<RowKey>,
    /// Single dates of Termine hidden, by the Termin's key and the day.
    pub dates: BTreeSet<(RowKey, Day)>,
}

impl SemesterHides {
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty() && self.events.is_empty() && self.rows.is_empty() && self.chosen.is_empty() && self.dates.is_empty()
    }
}

impl PlanDoc {
    /// The plan of a stored text. Every line is checked like URL input and dropped alone when it
    /// fails; duplicates count once (the first `m` of a semester and module, the first `p` of a
    /// pid and of a plan row in a semester, the last `a` of a semester); the caps keep the first
    /// lines in the order of the text. Never fails. (A build before 2026-09-26 keeps the first `p`
    /// of a plan row in any semester: going back to it drops a row's placeholders in its later
    /// semesters, and their modules then count for nothing.)
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
                "g" => {
                    if doc.program.is_none() && url::is_program_id(field(1)) {
                        doc.program = Some(field(1).to_string());
                    }
                }
                "m" => {
                    let Some(s) = semester else { continue };
                    if doc.plan(s, field(2), number(field(3)).unwrap_or(0), None) {
                        if let Some(pid) = number(field(4)).and_then(|pid| u32::try_from(pid).ok()) {
                            fills.push((s, field(2).to_string(), pid));
                        }
                        if field(5) == FROM_PLAN {
                            doc.mark_from_plan(s, field(2));
                        }
                    }
                }
                "v" => {
                    // After the `m` lines: a module planned there as well stays planned.
                    if let Some(s) = semester {
                        doc.visit(s, field(2), number(field(3)).unwrap_or(0));
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
                "h" => {
                    if let (Some(s), Some(row)) = (semester, RowKey::parse(field(2))) {
                        for day in field(3).split(',').filter_map(|iso| Day::parse(iso.trim())) {
                            doc.set_date(s, row, day, true);
                        }
                    }
                }
                "a" => {
                    if let Some(s) = semester {
                        doc.remember(s, field(2));
                    }
                }
                "d" => {
                    // The first mark of a module counts.
                    if let Some(s) = semester.filter(|_| doc.passed_in(field(2)).is_none()) {
                        doc.pass(s, field(2));
                    }
                }
                "l" => {
                    let id = number(field(1)).and_then(|id| u8::try_from(id).ok());
                    if let Some(id) = id.filter(|id| (1..=MAX_CALENDAR_ID).contains(id) && doc.calendar(*id).is_none() && doc.calendars.len() < MAX_CALENDARS) {
                        let program = Some(field(3)).filter(|program| url::is_program_id(program)).map(str::to_string);
                        doc.calendars.push(Calendar { id, name: calendar_name(field(4)), view: CalendarView::from_code(field(2)), program, modules: Vec::new(), chosen: BTreeSet::new(), own: Vec::new() });
                    }
                }
                "n" => {
                    let id = number(field(1)).and_then(|id| u8::try_from(id).ok());
                    if let (Some(calendar), Some(s)) = (id.and_then(|id| doc.calendars.iter_mut().find(|calendar| calendar.id == id)), SemesterKey::parse(field(2))) {
                        calendar.add_module(s, field(3));
                    }
                }
                "o" => {
                    let id = number(field(1)).and_then(|id| u8::try_from(id).ok());
                    if let (Some(calendar), Some(s), Some(row)) = (id.and_then(|id| doc.calendars.iter_mut().find(|calendar| calendar.id == id)), SemesterKey::parse(field(2)), RowKey::parse(field(3))) {
                        calendar.set_tick(s, row, true);
                    }
                }
                "t" => {
                    // After the `l` line of its calendar; the first line of a number counts.
                    if let Some((calendar, own)) = own_of(&fields) {
                        doc.insert_own(calendar, own);
                    }
                }
                "q" => {
                    let ord = field(3).parse::<i64>().ok();
                    if let (Some(s), Some(ord)) = (semester, ord) {
                        if doc.done_row(field(2), ord).is_none() {
                            doc.finish_row(DoneRow { semester: s, program_id: field(2).to_string(), ord, caption: field(4).to_string(), name: field(5).to_string() });
                        }
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

    /// The text `restored` reads back as this plan: `g`; `m` by semester, then in planning order;
    /// `v` the same; `p` by pid; `d` and `q` in the order they were marked; then per semester `k`, `e`, `r`,
    /// `c`, `h` (one line per Termin), `a`; the plan's `t` in the order they came; per calendar its
    /// `l`, its `n` in the order they came, its `o` and its `t`; then the lines of unknown tags. Control characters in names become spaces, so a
    /// name never splits its line.
    pub fn stored(&self) -> String {
        self.text(self.program.as_deref())
    }

    /// The text of the plan without its program (no `g`): what the browser's store of the
    /// Stundenplan keeps, whose program is the session's (`folia_stores::studyplan`).
    pub fn stored_without_program(&self) -> String {
        self.text(None)
    }

    fn text(&self, program: Option<&str>) -> String {
        let mut out = String::new();
        if let Some(program) = program.filter(|id| url::is_program_id(id)) {
            let _ = writeln!(out, "g\t{program}");
        }
        let mut modules: Vec<&Planned> = self.modules.iter().filter(|m| url::is_module_id(&m.module_id)).collect();
        modules.sort_by_key(|m| m.semester);
        for m in modules {
            let fills = m.fills.map(|pid| pid.to_string()).unwrap_or_default();
            let from = if m.from_plan { format!("\t{FROM_PLAN}") } else { String::new() };
            let _ = writeln!(out, "m\t{}\t{}\t{}\t{}{from}", m.semester.key(), m.module_id, m.at, fills);
        }
        let mut visits: Vec<&Visit> = self.visits.iter().filter(|v| url::is_module_id(&v.module_id)).collect();
        visits.sort_by_key(|v| v.semester);
        for v in visits {
            let _ = writeln!(out, "v\t{}\t{}\t{}", v.semester.key(), v.module_id, v.at);
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
        for p in self.passed.iter().filter(|p| url::is_module_id(&p.module_id)) {
            let _ = writeln!(out, "d\t{}\t{}", p.semester.key(), p.module_id);
        }
        for row in self.done_rows.iter().filter(|row| url::is_program_id(&row.program_id)) {
            let _ = writeln!(out, "q\t{}\t{}\t{}\t{}\t{}", row.semester.key(), row.program_id, row.ord, clean(&row.caption), clean(&row.name));
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
                let mut dates: BTreeMap<RowKey, Vec<String>> = BTreeMap::new();
                for (row, day) in &hides.dates {
                    dates.entry(*row).or_default().push(day.iso());
                }
                for (row, days) in dates {
                    let _ = writeln!(out, "h\t{key}\t{}\t{}", row.text(), days.join(","));
                }
            }
            if let Some(code) = self.subscribed.get(&s).filter(|code| is_code(code)) {
                let _ = writeln!(out, "a\t{key}\t{code}");
            }
        }
        for own in &self.own {
            let _ = writeln!(out, "{}", own_line(0, own));
        }
        for calendar in &self.calendars {
            let id = calendar.id;
            let view = calendar.view.map(CalendarView::code).unwrap_or_default();
            let program = calendar.program.as_deref().filter(|program| url::is_program_id(program)).unwrap_or_default();
            let _ = writeln!(out, "l\t{id}\t{view}\t{program}\t{}", calendar_name(&calendar.name));
            for (semester, module) in &calendar.modules {
                let _ = writeln!(out, "n\t{id}\t{}\t{module}", semester.key());
            }
            for (semester, row) in &calendar.chosen {
                let _ = writeln!(out, "o\t{id}\t{}\t{}", semester.key(), row.text());
            }
            for own in &calendar.own {
                let _ = writeln!(out, "{}", own_line(id, own));
            }
        }
        for line in self.extra.iter().filter(|line| !line.contains(['\n', '\r'])) {
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    /// Nothing planned: no module, no visit, no placeholder and no Termin of one's own of the plan's
    /// calendar. The stored text may still hold what a semester hides, a code, the calendars of
    /// one's own or lines of a newer build; the store's key goes only when `stored` is empty.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty() && self.visits.is_empty() && self.placeholders.is_empty() && self.own.is_empty()
    }

    /// The semesters the plan holds modules, visits or placeholders in, in order.
    pub fn semesters(&self) -> Vec<SemesterKey> {
        let held: BTreeSet<SemesterKey> =
            self.modules.iter().map(|m| m.semester).chain(self.visits.iter().map(|v| v.semester)).chain(self.placeholders.iter().map(|p| p.semester)).collect();
        held.into_iter().collect()
    }

    /// The modules of a semester, in the order they were planned.
    pub fn modules_in(&self, s: SemesterKey) -> Vec<String> {
        self.modules.iter().filter(|m| m.semester == s).map(|m| m.module_id.clone()).collect()
    }

    /// The modules only visited in a semester, in the order they came.
    pub fn visits_in(&self, s: SemesterKey) -> Vec<String> {
        self.visits.iter().filter(|v| v.semester == s).map(|v| v.module_id.clone()).collect()
    }

    /// What the week of a semester holds: its planned modules, then its visits.
    pub fn week_in(&self, s: SemesterKey) -> Vec<String> {
        let mut ids = self.modules_in(s);
        ids.extend(self.visits_in(s));
        ids
    }

    pub fn is_visited(&self, s: SemesterKey, id: &str) -> bool {
        self.visits.iter().any(|v| v.semester == s && v.module_id == id)
    }

    /// How many modules a semester's week holds, planned and visited: at most `MAX_MODULES`,
    /// what a subscription carries.
    fn held_in(&self, s: SemesterKey) -> usize {
        self.modules.iter().filter(|m| m.semester == s).count() + self.visits.iter().filter(|v| v.semester == s).count()
    }

    /// Takes a module into a semester only to visit it (`Visit`). One passed in that semester
    /// (its mark stays when it is taken out of the week) comes back planned: a visit is never
    /// passed, and the module counts there. `false` when nothing changed: the id is no module id,
    /// the module is planned or visited there already (a planned one becomes a visit by
    /// `make_visit`), or a cap is reached (the semester's week holds 60 modules, 24 semesters).
    pub fn visit(&mut self, s: SemesterKey, id: &str, at: u64) -> bool {
        if !url::is_module_id(id) || self.is_planned(s, id) || self.is_visited(s, id) {
            return false;
        }
        if self.passed_in(id) == Some(s) {
            return self.plan(s, id, at, None);
        }
        if self.held_in(s) >= MAX_MODULES || self.visits.len() >= MAX_PLANNED || !self.admits(s) {
            return false;
        }
        let at_index = self.visits.partition_point(|v| v.semester <= s);
        self.visits.insert(at_index, Visit { semester: s, module_id: id.to_string(), at });
        true
    }

    /// Takes a visit out of a semester, together with what the semester hides and has chosen of
    /// `only_its_events`, as `unplan` does.
    pub fn unvisit(&mut self, s: SemesterKey, id: &str, only_its_events: &[u32]) {
        self.visits.retain(|v| !(v.semester == s && v.module_id == id));
        self.forget_events(s, only_its_events);
    }

    /// „Nur besuchen" taken away: the visit becomes a module planned into the semester, which
    /// counts, keeping when it came and what the semester has chosen of it. `false` when nothing
    /// changed (no such visit).
    pub fn count_visit(&mut self, s: SemesterKey, id: &str) -> bool {
        let Some(at) = self.visits.iter().find(|v| v.semester == s && v.module_id == id).map(|v| v.at) else { return false };
        self.plan(s, id, at, None)
    }

    /// „Nur besuchen" on a planned module: it leaves the study and stays in the week as a visit,
    /// keeping when it was planned and what the semester has chosen of it; what it counted for
    /// counts for nothing now. Not for a module passed in that semester (a visit is never passed).
    /// `false` when nothing changed.
    pub fn make_visit(&mut self, s: SemesterKey, id: &str) -> bool {
        if self.passed_in(id) == Some(s) || self.visits.len() >= MAX_PLANNED {
            return false;
        }
        let Some(index) = self.modules.iter().position(|m| m.semester == s && m.module_id == id) else { return false };
        let planned = self.modules.remove(index);
        let at_index = self.visits.partition_point(|v| v.semester <= s);
        self.visits.insert(at_index, Visit { semester: s, module_id: planned.module_id, at: planned.at });
        true
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

    /// The placeholders of the plan row `p` stands for, `p` among them, by pid: one in every
    /// semester the row was taken over into.
    pub fn same_row(&self, p: &Placeholder) -> Vec<&Placeholder> {
        self.placeholders.iter().filter(|q| q.program_id == p.program_id && q.ord == p.ord).collect()
    }

    /// What the visitor chose to see of a semester, with the town of Mein Studiengang and the
    /// modules only visited (`Selection::visits`). A town left to derive is derived from the
    /// semester's modules taken over from the Regelstudienplan that count for no placeholder,
    /// where it has any (`Selection::town_from`): an elective planned beside them, or one found
    /// for a placeholder, never turns it.
    pub fn selection(&self, s: SemesterKey, town: TownChoice) -> Selection {
        let hides = self.hidden.get(&s);
        let imported: BTreeSet<String> =
            self.modules.iter().filter(|m| m.semester == s && m.from_plan && m.fills.is_none()).map(|m| m.module_id.clone()).collect();
        Selection {
            hidden_kinds: hides.map(|h| h.kinds).unwrap_or_default(),
            hidden_events: hides.map(|h| h.events.clone()).unwrap_or_default(),
            hidden_rows: hides.map(|h| h.rows.clone()).unwrap_or_default(),
            chosen_rows: hides.map(|h| h.chosen.clone()).unwrap_or_default(),
            hidden_dates: hides.map(|h| h.dates.clone()).unwrap_or_default(),
            town,
            town_from: (!imported.is_empty()).then_some(imported),
            visits: self.visits_in(s).into_iter().collect(),
            own: own_of_semester(&self.own, s),
        }
    }

    /// Plans a module into a semester, counting for the placeholder `fills` when the plan holds
    /// it; a visit of it there becomes the planned module (planning it is what makes it count).
    /// `false` when nothing changed: the id is no module id, the module is planned there
    /// already, or a cap is reached (60 modules in the semester's week, 400 in all, 24
    /// semesters).
    pub fn plan(&mut self, s: SemesterKey, id: &str, at: u64, fills: Option<u32>) -> bool {
        if !url::is_module_id(id) || self.is_planned(s, id) {
            return false;
        }
        let visited = self.visits.iter().position(|v| v.semester == s && v.module_id == id);
        let here = self.held_in(s) - usize::from(visited.is_some());
        if here >= MAX_MODULES || self.modules.len() >= MAX_PLANNED || !self.admits(s) {
            return false;
        }
        if let Some(index) = visited {
            self.visits.remove(index);
        }
        let fills = fills.filter(|pid| self.placeholders.iter().any(|p| p.pid == *pid));
        // After the semester's last module: the list stays in the order `stored` writes it.
        let at_index = self.modules.partition_point(|m| m.semester <= s);
        self.modules.insert(at_index, Planned { semester: s, module_id: id.to_string(), at, fills, from_plan: false });
        true
    }

    /// Marks a planned module as taken over from the Regelstudienplan.
    pub(crate) fn mark_from_plan(&mut self, s: SemesterKey, id: &str) {
        if let Some(m) = self.modules.iter_mut().find(|m| m.semester == s && m.module_id == id) {
            m.from_plan = true;
        }
    }

    /// Takes a module out of a semester, together with what the semester hides and has chosen of
    /// `only_its_events`: the events no other module of the semester links (the page knows them
    /// from its rows). Hidden kinds stay; they are about the semester, not the module.
    pub fn unplan(&mut self, s: SemesterKey, id: &str, only_its_events: &[u32]) {
        self.modules.retain(|m| !(m.semester == s && m.module_id == id));
        self.forget_events(s, only_its_events);
    }

    /// What semester `s` hides and has chosen of `events`, taken away. Hidden kinds stay.
    fn forget_events(&mut self, s: SemesterKey, events: &[u32]) {
        if let Some(hides) = self.hidden.get_mut(&s) {
            for event in events {
                hides.events.remove(event);
            }
            hides.rows.retain(|row| !events.contains(&row.event));
            hides.chosen.retain(|row| !events.contains(&row.event));
            hides.dates.retain(|(row, _)| !events.contains(&row.event));
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
        if self.is_planned(to, id) {
            // The copy that stays is the one the Regelstudienplan placed, if either was.
            if moved.from_plan {
                self.mark_from_plan(to, id);
            }
            return;
        }
        if !self.plan(to, id, moved.at, moved.fills) {
            self.modules.insert(at, moved);
        } else if moved.from_plan {
            self.mark_from_plan(to, id);
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

    /// Ticks one option of a choice, or takes the tick away (`on`): the row is ticked beside the
    /// others of its event, however many (owner, 2026-10-26: several groups, all alike), and
    /// taking the last one away opens the choice again. Caps as `set_event`.
    pub fn tick(&mut self, s: SemesterKey, row: RowKey, on: bool) {
        if !on {
            self.take_from(s, &row, |h| &mut h.chosen);
        } else if is_row(row) {
            self.add_to(s, row, |h| &h.chosen, |h| &mut h.chosen);
        }
    }

    /// Hides one date of a Termin („Nur am 04.11. ausblenden"), or shows it again; caps as
    /// `set_event`, a date counting as an entry. A day outside the semester's half-year is none of
    /// its dates and is not hidden.
    pub fn set_date(&mut self, s: SemesterKey, row: RowKey, day: Day, hidden: bool) {
        let (first, last) = s.bounds();
        if !hidden {
            self.take_from(s, &(row, day), |h| &mut h.dates);
        } else if is_row(row) && (first..=last).contains(&day) {
            self.add_to(s, (row, day), |h| &h.dates, |h| &mut h.dates);
        }
    }

    /// Replaces the ticks of an event by `row`, a row of that event („Nur diesen"); `None` takes
    /// them all away („Alle zeigen"), and the choice is open again.
    pub fn choose(&mut self, s: SemesterKey, event: u32, row: Option<RowKey>) {
        if let Some(hides) = self.hidden.get_mut(&s) {
            hides.chosen.retain(|chosen| chosen.event != event);
        }
        if let Some(row) = row.filter(|row| row.event == event && is_row(*row)) {
            self.add_to(s, row, |h| &h.chosen, |h| &mut h.chosen);
        }
        self.prune(s);
    }

    /// Shows every event, Termin and date of a semester again and undoes its choices; hidden kinds
    /// stay (they have chips of their own).
    pub fn show_all(&mut self, s: SemesterKey) {
        if let Some(hides) = self.hidden.get_mut(&s) {
            hides.events.clear();
            hides.rows.clear();
            hides.chosen.clear();
            hides.dates.clear();
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

    /// The semester a module was passed in, when it was (`d`).
    pub fn passed_in(&self, id: &str) -> Option<SemesterKey> {
        self.passed.iter().find(|p| p.module_id == id).map(|p| p.semester)
    }

    /// Marks a module as passed in semester `s`, or moves its mark there. `false` when nothing
    /// changed: the id is no module id, the mark says `s` already, or the cap is reached.
    pub fn pass(&mut self, s: SemesterKey, id: &str) -> bool {
        if !url::is_module_id(id) {
            return false;
        }
        if let Some(passed) = self.passed.iter_mut().find(|p| p.module_id == id) {
            let changed = passed.semester != s;
            passed.semester = s;
            return changed;
        }
        if self.passed.len() >= MAX_PASSED {
            return false;
        }
        self.passed.push(Passed { semester: s, module_id: id.to_string() });
        true
    }

    /// Takes the mark of a passed module away.
    pub fn unpass(&mut self, id: &str) {
        self.passed.retain(|p| p.module_id != id);
    }

    /// The mark of a program's plan row that is done, by the row's `ord` as it was marked.
    pub fn done_row(&self, program_id: &str, ord: i64) -> Option<&DoneRow> {
        self.done_rows.iter().find(|row| row.program_id == program_id && row.ord == ord)
    }

    /// Marks a row of a plan as done, or moves its mark to the row's new semester. `false` when
    /// nothing changed: no program id, an `ord` no plan has, the same mark, or the cap.
    pub fn finish_row(&mut self, row: DoneRow) -> bool {
        if !url::is_program_id(&row.program_id) || !(1..=MAX_ORD).contains(&row.ord) {
            return false;
        }
        let row = DoneRow { caption: clean(&row.caption), name: clean(&row.name), ..row };
        if let Some(done) = self.done_rows.iter_mut().find(|done| done.program_id == row.program_id && done.ord == row.ord) {
            let changed = *done != row;
            *done = row;
            return changed;
        }
        if self.done_rows.len() >= MAX_DONE_ROWS {
            return false;
        }
        self.done_rows.push(row);
        true
    }

    /// Takes the mark of a done row away.
    pub fn reopen_row(&mut self, program_id: &str, ord: i64) {
        self.done_rows.retain(|row| !(row.program_id == program_id && row.ord == ord));
    }

    /// A calendar of one's own by its number.
    pub fn calendar(&self, id: u8) -> Option<&Calendar> {
        self.calendars.iter().find(|calendar| calendar.id == id)
    }

    /// Makes a calendar of one's own, the last of them: named `name`, with the modules `modules`
    /// and the groups `chosen` of semester `s` (a plan handed on, `folia_calendar::share`) and the
    /// program `program`, standing as a new one stands. Its number, the smallest one free; `None`
    /// at the cap.
    pub fn add_calendar(&mut self, name: &str, program: Option<&str>, s: SemesterKey, modules: &[String], chosen: &[RowKey]) -> Option<u8> {
        if self.calendars.len() >= MAX_CALENDARS {
            return None;
        }
        let id = (1..=MAX_CALENDAR_ID).find(|id| self.calendar(*id).is_none())?;
        let program = program.filter(|program| url::is_program_id(program)).map(str::to_string);
        let mut calendar = Calendar { id, name: calendar_name(name), view: None, program, modules: Vec::new(), chosen: BTreeSet::new(), own: Vec::new() };
        for module in modules {
            calendar.add_module(s, module);
        }
        for row in chosen {
            calendar.set_tick(s, *row, true);
        }
        self.calendars.push(calendar);
        Some(id)
    }

    /// Names a calendar anew; `false` where nothing changed.
    pub fn rename_calendar(&mut self, id: u8, name: &str) -> bool {
        let name = calendar_name(name);
        match self.calendars.iter_mut().find(|calendar| calendar.id == id) {
            Some(calendar) if calendar.name != name => {
                calendar.name = name;
                true
            }
            _ => false,
        }
    }

    /// How a calendar stands in the week from now on, whatever it holds.
    pub fn set_calendar_view(&mut self, id: u8, view: CalendarView) {
        if let Some(calendar) = self.calendars.iter_mut().find(|calendar| calendar.id == id) {
            calendar.view = Some(view);
        }
    }

    /// „+ Modul" in a calendar of one's own: the module in its semester `s`, counting nowhere
    /// (owner, 2026-10-26: „bei jedem kallender kann man auch andere module hinyufügen"). `false`
    /// when nothing changed: no such calendar, no module id, the module there already, or the cap.
    pub fn add_to_calendar(&mut self, id: u8, s: SemesterKey, module: &str) -> bool {
        self.calendars.iter_mut().find(|calendar| calendar.id == id).is_some_and(|calendar| calendar.add_module(s, module))
    }

    /// Takes a module out of a calendar of one's own in semester `s`, with the groups ticked of
    /// `only_its_events` there (the events no other module of the calendar links).
    pub fn remove_from_calendar(&mut self, id: u8, s: SemesterKey, module: &str, only_its_events: &[u32]) {
        if let Some(calendar) = self.calendars.iter_mut().find(|calendar| calendar.id == id) {
            calendar.modules.retain(|(semester, held)| !(*semester == s && held == module));
            calendar.chosen.retain(|(semester, row)| !(*semester == s && only_its_events.contains(&row.event)));
        }
    }

    /// Ticks a group in a calendar of one's own, or takes the tick away; its own choice, apart from
    /// the plan's (`tick`).
    pub fn tick_in_calendar(&mut self, id: u8, s: SemesterKey, row: RowKey, on: bool) {
        if let Some(calendar) = self.calendars.iter_mut().find(|calendar| calendar.id == id) {
            calendar.set_tick(s, row, on);
        }
    }

    /// Takes a calendar away; it and where it stood, for „Rückgängig" (`put_calendar`).
    pub fn remove_calendar(&mut self, id: u8) -> Option<(usize, Calendar)> {
        let at = self.calendars.iter().position(|calendar| calendar.id == id)?;
        Some((at, self.calendars.remove(at)))
    }

    /// Puts a calendar taken away back where it stood; `false` where its number is taken by now or
    /// the cap is reached.
    pub fn put_calendar(&mut self, at: usize, calendar: Calendar) -> bool {
        if self.calendar(calendar.id).is_some() || self.calendars.len() >= MAX_CALENDARS {
            return false;
        }
        self.calendars.insert(at.min(self.calendars.len()), calendar);
        true
    }

    /// The Termine of one's own of calendar `calendar` (0 the plan's, „Mein Studiengang") in
    /// semester `s`, in the order they came.
    pub fn own_in(&self, calendar: u8, s: SemesterKey) -> Vec<&OwnTermin> {
        self.own_list(calendar).map(|list| list.iter().filter(|own| own.semester == s).map(|own| &own.termin).collect()).unwrap_or_default()
    }

    /// A Termin of one's own by its number, with its calendar (0 the plan's).
    pub fn own_termin(&self, number: u32) -> Option<(u8, &Own)> {
        let plan = self.own.iter().find(|own| own.termin.number == number).map(|own| (0, own));
        plan.or_else(|| self.calendars.iter().find_map(|calendar| calendar.own.iter().find(|own| own.termin.number == number).map(|own| (calendar.id, own))))
    }

    /// „+ Termin": a Termin of one's own in calendar `calendar` (0 the plan's) and semester `s`,
    /// numbered anew, its texts as the store keeps them. Its number; `None` where it is no Termin
    /// (`OwnTermin::is_valid`), the calendar is none of the plan's, or a cap is reached.
    pub fn add_own(&mut self, calendar: u8, s: SemesterKey, termin: OwnTermin) -> Option<u32> {
        let number = self.next_own_number()?;
        let termin = kept_own(OwnTermin { number, ..termin })?;
        self.insert_own(calendar, Own { semester: s, termin }).then_some(number)
    }

    /// „Speichern" of a Termin of one's own changed: what it is, and the calendar it stands in, its
    /// number and semester kept (a changed calendar takes it at its end, within the caps). `false`
    /// where nothing changed: no such Termin, no Termin now, or a cap of the calendar asked for.
    pub fn set_own(&mut self, number: u32, calendar: u8, termin: OwnTermin) -> bool {
        let Some(termin) = kept_own(OwnTermin { number, ..termin }) else { return false };
        let Some((from, semester)) = self.own_termin(number).map(|(from, own)| (from, own.semester)) else { return false };
        if from != calendar {
            let Some(removed) = self.remove_own(number) else { return false };
            if self.insert_own(calendar, Own { semester, termin }) {
                return true;
            }
            self.put_own(removed);
            return false;
        }
        let held = self.own_list_mut(calendar).and_then(|list| list.iter_mut().find(|own| own.termin.number == number));
        match held {
            Some(own) if own.termin != termin => {
                own.termin = termin;
                true
            }
            _ => false,
        }
    }

    /// „Nur am … ausblenden" of a Termin of one's own: the day is skipped, or (`skipped` false)
    /// held again. `false` where nothing changed: no such Termin, the day skipped already, or the
    /// cap of its skipped days.
    pub fn skip_own(&mut self, number: u32, day: Day, skipped: bool) -> bool {
        let Some(calendar) = self.own_termin(number).map(|(calendar, _)| calendar) else { return false };
        let Some(own) = self.own_list_mut(calendar).and_then(|list| list.iter_mut().find(|own| own.termin.number == number)) else { return false };
        let skip = &mut own.termin.skip;
        match (skipped, skip.binary_search(&day)) {
            (true, Err(at)) if skip.len() < MAX_OWN_DATES => skip.insert(at, day),
            (false, Ok(at)) => {
                skip.remove(at);
            }
            _ => return false,
        }
        true
    }

    /// Takes a Termin of one's own away („Löschen"); it, its calendar and where it stood, for
    /// „Rückgängig" (`put_own`).
    pub fn remove_own(&mut self, number: u32) -> Option<RemovedOwn> {
        let (calendar, _) = self.own_termin(number)?;
        let list = self.own_list_mut(calendar)?;
        let at = list.iter().position(|own| own.termin.number == number)?;
        Some(RemovedOwn { calendar, at, own: list.remove(at) })
    }

    /// Puts a Termin of one's own taken away back where it stood; `false` where its number is
    /// taken by now, its calendar is gone, or a cap is reached.
    pub fn put_own(&mut self, removed: RemovedOwn) -> bool {
        let RemovedOwn { calendar, at, own } = removed;
        if !self.own_fits(calendar, &own) {
            return false;
        }
        match self.own_list_mut(calendar) {
            Some(list) => {
                list.insert(at.min(list.len()), own);
                true
            }
            None => false,
        }
    }

    /// Adds a Termin of one's own as read or made, at the end of its calendar's list, where it fits
    /// (`own_fits`).
    fn insert_own(&mut self, calendar: u8, own: Own) -> bool {
        if !self.own_fits(calendar, &own) {
            return false;
        }
        match self.own_list_mut(calendar) {
            Some(list) => {
                list.push(own);
                true
            }
            None => false,
        }
    }

    /// Whether a Termin of one's own may be added to calendar `calendar`: it is one, its number is
    /// free, the calendar is the plan's or one of one's own, the caps allow it, and the plan's
    /// semesters admit its semester.
    fn own_fits(&self, calendar: u8, own: &Own) -> bool {
        let Some(list) = self.own_list(calendar) else { return false };
        let here = list.iter().filter(|held| held.semester == own.semester).count();
        let all = self.own.len() + self.calendars.iter().map(|calendar| calendar.own.len()).sum::<usize>();
        own.termin.is_valid() && self.own_termin(own.termin.number).is_none() && here < MAX_OWN && all < MAX_OWN_ALL && (calendar != 0 || self.admits(own.semester))
    }

    /// The number a new Termin of one's own gets: one more than the largest, else the smallest one
    /// free.
    fn next_own_number(&self) -> Option<u32> {
        let numbers: BTreeSet<u32> = self.own.iter().chain(self.calendars.iter().flat_map(|calendar| calendar.own.iter())).map(|own| own.termin.number).collect();
        match numbers.last().copied().unwrap_or(0) {
            largest if largest < MAX_OWN_NUMBER => Some(largest + 1),
            _ => (1..=MAX_OWN_NUMBER).find(|number| !numbers.contains(number)),
        }
    }

    fn own_list(&self, calendar: u8) -> Option<&Vec<Own>> {
        match calendar {
            0 => Some(&self.own),
            id => self.calendar(id).map(|calendar| &calendar.own),
        }
    }

    fn own_list_mut(&mut self, calendar: u8) -> Option<&mut Vec<Own>> {
        match calendar {
            0 => Some(&mut self.own),
            id => self.calendars.iter_mut().find(|calendar| calendar.id == id).map(|calendar| &mut calendar.own),
        }
    }

    /// Empties the timetable of semester `s` („Plan leeren"): its modules, visits and placeholders
    /// (what counted for them elsewhere counts for nothing), what it hides and has chosen. The
    /// other semesters stay, and so do the program, what was passed or done, the code of the
    /// subscription (the page tells that the plan has moved on from it) and the lines of a newer
    /// build.
    pub fn clear_semester(&mut self, s: SemesterKey) {
        let gone: Vec<u32> = self.placeholders.iter().filter(|p| p.semester == s).map(|p| p.pid).collect();
        for pid in gone {
            self.remove_placeholder(pid);
        }
        self.modules.retain(|m| m.semester != s);
        self.visits.retain(|v| v.semester != s);
        self.own.retain(|own| own.semester != s);
        self.hidden.remove(&s);
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
    /// What the plan holds meanwhile is left out, so applying it twice adds nothing. Every module
    /// the Regelstudienplan names is marked as taken over from it, the ones it adds and the ones
    /// the plan held already (`Import::held`). Returns how many modules and placeholders were
    /// added.
    pub fn apply(&mut self, import: &Import, now: u64) -> (usize, usize) {
        let mut modules = 0;
        for (s, id) in &import.modules {
            if self.plan(*s, id, now, None) {
                modules += 1;
            }
            self.mark_from_plan(*s, id);
        }
        for id in &import.held {
            for m in self.modules.iter_mut().filter(|m| m.module_id == *id) {
                m.from_plan = true;
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

    /// The timetable of one semester as a plan of its own, the program with it: what „Plan
    /// speichern" keeps (`SavedPlan`). A module keeps what it counts for only where that
    /// placeholder stands in the semester too.
    pub fn semester_only(&self, s: SemesterKey) -> PlanDoc {
        let placeholders: Vec<Placeholder> = self.placeholders.iter().filter(|p| p.semester == s).cloned().collect();
        let modules = self
            .modules
            .iter()
            .filter(|m| m.semester == s)
            .map(|m| Planned { fills: m.fills.filter(|pid| placeholders.iter().any(|p| p.pid == *pid)), ..m.clone() })
            .collect();
        let hidden = self.hidden.get(&s).map(|hides| BTreeMap::from([(s, hides.clone())])).unwrap_or_default();
        let visits = self.visits.iter().filter(|v| v.semester == s).cloned().collect();
        let own = self.own.iter().filter(|own| own.semester == s).cloned().collect();
        PlanDoc { program: self.program.clone(), modules, visits, placeholders, hidden, own, ..PlanDoc::default() }
    }

    /// Replaces the timetable of semester `s` by a saved one (`semester_only`, of whatever
    /// semester it was saved in): its modules, visits, placeholders (numbered anew, what counts
    /// for them with them), what it hides and has chosen, and its program. The other semesters
    /// stay, and so does the subscription code (the page tells that the plan has moved on from
    /// it).
    pub fn put_semester(&mut self, s: SemesterKey, saved: &PlanDoc) {
        let gone: Vec<u32> = self.placeholders.iter().filter(|p| p.semester == s).map(|p| p.pid).collect();
        for pid in gone {
            self.remove_placeholder(pid);
        }
        self.modules.retain(|m| m.semester != s);
        self.visits.retain(|v| v.semester != s);
        self.own.retain(|own| own.semester != s);
        self.hidden.remove(&s);
        self.program = saved.program.clone();
        let mut pids: BTreeMap<u32, u32> = BTreeMap::new();
        for p in &saved.placeholders {
            let pid = self.next_pid();
            if self.add_placeholder(Placeholder { pid, semester: s, ..p.clone() }) {
                pids.insert(p.pid, pid);
            }
        }
        for m in &saved.modules {
            if self.plan(s, &m.module_id, m.at, m.fills.and_then(|pid| pids.get(&pid).copied())) && m.from_plan {
                self.mark_from_plan(s, &m.module_id);
            }
        }
        for v in &saved.visits {
            self.visit(s, &v.module_id, v.at);
        }
        // Numbered anew: the numbers of another time may be taken by now.
        for own in &saved.own {
            self.add_own(0, s, own.termin.clone());
        }
        for hides in saved.hidden.values() {
            for kind in hides.kinds.iter() {
                self.set_kind(s, kind, true);
            }
            for event in &hides.events {
                self.set_event(s, *event, true);
            }
            for row in &hides.rows {
                self.set_row(s, *row, true);
            }
            for row in &hides.chosen {
                self.add_to(s, *row, |h| &h.chosen, |h| &mut h.chosen);
            }
            // A plan saved in another semester names days of that one, which are none of `s`'s.
            for (row, day) in &hides.dates {
                self.set_date(s, *row, *day, true);
            }
        }
    }

    /// Whether semester `s` holds the timetable `saved` holds (`semester_only`, of any semester):
    /// the same modules, placeholder rows, Termine of one's own and what is hidden and chosen,
    /// whenever each was planned and however the placeholders and the Termine are numbered. A loaded plan is marked while this holds. The
    /// program is not compared: the Stundenplan's is the session's (2026-10-15), and the plan is
    /// the same one in the next session and with another program picked.
    pub fn holds_saved(&self, s: SemesterKey, saved: &PlanDoc) -> bool {
        let mut there = PlanDoc::default();
        there.put_semester(s, saved);
        self.semester_only(s).shape(s) == there.shape(s)
    }

    /// What `holds_saved` compares of semester `s`.
    fn shape(&self, s: SemesterKey) -> Shape<'_> {
        let modules = self.modules.iter().filter(|m| m.semester == s).map(|m| m.module_id.as_str()).collect();
        let visits = self.visits.iter().filter(|v| v.semester == s).map(|v| v.module_id.as_str()).collect();
        let rows = self.placeholders.iter().filter(|p| p.semester == s).map(|p| (p.program_id.as_str(), p.ord)).collect();
        let own = self.own.iter().filter(|own| own.semester == s).map(|own| OwnTermin { number: 0, ..own.termin.clone() }).collect();
        (modules, visits, rows, self.hidden.get(&s), own)
    }

    /// Adds a placeholder in pid order, when it is valid, its pid is new, its plan row has none in
    /// its semester yet, and the caps allow it.
    pub(crate) fn add_placeholder(&mut self, p: Placeholder) -> bool {
        let valid = (1..=url::MAX_PID).contains(&p.pid)
            && url::is_program_id(&p.program_id)
            && (1..=MAX_ORD).contains(&p.ord)
            && 1 <= p.span.0
            && p.span.0 <= p.span.1
            && p.span.1 <= MAX_SPAN;
        let taken = self.placeholders.iter().any(|q| q.pid == p.pid || (q.program_id == p.program_id && q.ord == p.ord && q.semester == p.semester));
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
            .chain(self.visits.iter().map(|v| v.semester))
            .chain(self.placeholders.iter().map(|p| p.semester))
            .chain(self.own.iter().map(|own| own.semester))
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
/// leave  2026S 2027W
/// until  2029S
/// town  cottbus
/// ```
///
/// `leave` are the semesters of leave („Urlaubssemester"), which count as no Fachsemester, and
/// `until` the last semester of the study where the student added semesters beyond the
/// Regelstudienzeit („Semester anfügen", `study`). Every line is optional and the first valid one
/// of a key wins; keys this build does not know are kept and written back.
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
    /// The semesters of leave, at most `MAX_SEMESTERS`.
    pub leave: BTreeSet<SemesterKey>,
    /// The last semester of the study, where it was set beyond the Regelstudienzeit.
    pub until: Option<SemesterKey>,
    /// `Derive` is not stored: the town then follows from the plan.
    pub town: TownChoice,
    pub extra: Vec<(String, String)>,
}

impl MineDoc {
    const KEYS: [&'static str; 8] = ["program", "name", "caption", "direction", "start", "leave", "until", "town"];

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
                "leave" => {
                    if doc.leave.is_empty() {
                        doc.leave = value.split_whitespace().filter_map(SemesterKey::parse).take(MAX_SEMESTERS).collect();
                    }
                }
                "until" => {
                    if doc.until.is_none() {
                        doc.until = SemesterKey::parse(value);
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

    /// „Mein Studiengang" cleared: the program and its plan go; the Studienbeginn, the semesters of
    /// leave, the end and the town stay, since they are the student's whatever the program.
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
        if !self.leave.is_empty() {
            let keys: Vec<String> = self.leave.iter().take(MAX_SEMESTERS).map(|s| s.key()).collect();
            let _ = writeln!(out, "leave\t{}", keys.join(" "));
        }
        if let Some(until) = self.until {
            let _ = writeln!(out, "until\t{}", until.key());
        }
        if let Some(town) = town_code(self.town) {
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

/// The code `town_of_code` reads; `Derive` is not stored.
fn town_code(town: TownChoice) -> Option<&'static str> {
    match town {
        TownChoice::Derive => None,
        TownChoice::Only(town) => Some(town.code()),
        TownChoice::Both => Some("both"),
    }
}

/// The most saved plans („Plan speichern").
pub const MAX_SAVED: usize = 30;

/// The most bytes of `betula.studyplan.saved.v1` that are read: thirty timetables of a semester
/// are far below it; a longer text is read up to the last line break before it.
pub const MAX_SAVED_STORED: usize = 512 * 1024;

/// The most characters of a saved plan's name.
pub const MAX_SAVED_NAME: usize = 60;

/// A timetable kept under a name („Plan speichern", for a friend or a second try), to be loaded
/// again with a click: one semester's plan (`PlanDoc::semester_only`, the program with it) and the
/// Standort it was seen with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedPlan {
    pub name: String,
    pub town: TownChoice,
    pub doc: PlanDoc,
}

impl SavedPlan {
    /// How many modules it holds.
    pub fn modules(&self) -> usize {
        self.doc.modules.len()
    }
}

/// The saved plans of this browser (R20: in `localStorage` alone, never in an address or a
/// request), stored as blocks: a line `plan  <name>  <town>`, then the lines of its plan as
/// `PlanDoc::stored` writes them. Read like every store: a line that fails is dropped alone (a bad
/// line of a plan by `PlanDoc::restored`; a line before the first `plan` line), a block without a
/// name, with a name taken before, or with nothing planned is dropped, and at most `MAX_SAVED`
/// plans are read, in the order of the text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedPlans {
    pub plans: Vec<SavedPlan>,
}

impl SavedPlans {
    pub fn restored(text: &str) -> Self {
        let mut out = SavedPlans::default();
        let mut block: Option<(String, TownChoice, String)> = None;
        for raw in head(text, MAX_SAVED_STORED).split('\n') {
            let line: String = raw.chars().filter(|c| *c != '\r').collect();
            let mut fields = line.split('\t');
            if fields.next().map(str::trim) == Some("plan") {
                if let Some((name, town, body)) = block.take() {
                    out.add(&name, town, PlanDoc::restored(&body));
                }
                let name = fields.next().unwrap_or_default().to_string();
                let town = town_of_code(fields.next().unwrap_or_default().trim()).unwrap_or_default();
                block = Some((name, town, String::new()));
            } else if let Some((_, _, body)) = block.as_mut() {
                body.push_str(&line);
                body.push('\n');
            }
        }
        if let Some((name, town, body)) = block {
            out.add(&name, town, PlanDoc::restored(&body));
        }
        out
    }

    /// The text `restored` reads back as these plans.
    pub fn stored(&self) -> String {
        let mut out = String::new();
        for plan in &self.plans {
            let _ = writeln!(out, "plan\t{}\t{}", saved_name(&plan.name), town_code(plan.town).unwrap_or_default());
            out.push_str(&plan.doc.stored());
        }
        out
    }

    /// Keeps a plan under `name`, in the place of one of the same name; `false` when nothing was
    /// kept: no name, nothing planned, or `MAX_SAVED` plans of other names.
    pub fn save(&mut self, name: &str, town: TownChoice, doc: PlanDoc) -> bool {
        let name = saved_name(name);
        if name.is_empty() || doc.is_empty() {
            return false;
        }
        match self.plans.iter_mut().find(|plan| plan.name == name) {
            Some(plan) => {
                *plan = SavedPlan { name, town, doc };
                true
            }
            None => self.add(&name, town, doc),
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.plans.retain(|plan| plan.name != name);
    }

    pub fn get(&self, name: &str) -> Option<&SavedPlan> {
        self.plans.iter().find(|plan| plan.name == name)
    }

    /// A new plan at the end, within the caps.
    fn add(&mut self, name: &str, town: TownChoice, doc: PlanDoc) -> bool {
        let name = saved_name(name);
        if name.is_empty() || doc.is_empty() || self.plans.len() >= MAX_SAVED || self.get(&name).is_some() {
            return false;
        }
        self.plans.push(SavedPlan { name, town, doc });
        true
    }
}

/// A saved plan's name as it is kept: `clean`, at most `MAX_SAVED_NAME` characters.
fn saved_name(name: &str) -> String {
    let name = clean(name);
    match name.char_indices().nth(MAX_SAVED_NAME) {
        Some((at, _)) => name.get(..at).unwrap_or_default().trim_end().to_string(),
        None => name,
    }
}

/// What taking a Regelstudienplan over adds to a plan (`import`), and its preview.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Import {
    /// The modules and their semesters, by Fachsemester and then in the plan's order.
    pub modules: Vec<(SemesterKey, String)>,
    /// With pid 0: `PlanDoc::apply` numbers them.
    pub placeholders: Vec<Placeholder>,
    /// Modules and placeholder rows left out because the plan holds them already („3 schon
    /// geplant"); a module the plan names twice counts once.
    pub skipped: usize,
    /// The modules among them: `PlanDoc::apply` marks them as taken over from the Regelstudienplan
    /// wherever they are planned, so that one the student planned before, or a plan stored before
    /// the mark existed, gets it with the next import.
    pub held: Vec<String>,
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
/// semester is left out and counted as „schon geplant", and one the plan names twice is taken or
/// counted once, at its first Fachsemester. Every other row becomes a placeholder under its own name,
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
                // A repeat first: a module the plan names twice counts once, as it is added once,
                // so a second import's „schon geplant" is what the first one added.
                if !taken.insert(id) {
                    continue;
                }
                if !doc.planned_in(id).is_empty() {
                    out.skipped += 1;
                    out.held.push(id.to_string());
                    continue;
                }
                out.modules.push((semester, id.to_string()));
                true
            }
            None => {
                let Some(placeholder) = row_placeholder(program_id, variant, entry, semester, &name) else { continue };
                if doc.placeholders.iter().any(|p| p.program_id == program_id && p.ord == entry.ord) {
                    out.skipped += 1;
                    continue;
                }
                out.placeholders.push(placeholder);
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

/// The placeholder a plan row without a module becomes in `semester` (pid 0: `PlanDoc::apply`
/// numbers it): its name, credits, kind, span and the caption of its plan. `None` for a row of
/// prose, with neither credits nor a kind.
pub(crate) fn row_placeholder(program_id: &str, variant: &PlanVariant, entry: &PlanEntry, semester: SemesterKey, name: &str) -> Option<Placeholder> {
    let credits = plan::credits_of(entry).as_deref().and_then(credits_text);
    let kind = entry.kind.as_ref().and_then(Code::known).map(|kind| kind.code().to_string());
    if credits.is_none() && kind.is_none() {
        return None;
    }
    let (from, to) = plan::semester_span(entry)?;
    Some(Placeholder {
        pid: 0,
        semester,
        program_id: program_id.to_string(),
        ord: entry.ord,
        span: (u8::try_from(from).ok()?, u8::try_from(to).ok()?),
        credits,
        kind,
        caption: clean(&variant.full),
        name: clean(name),
    })
}

/// What taking one Fachsemester of the plan `core` over adds to the timetable of `semester` (the
/// Stundenplan's „Importieren"): every row whose span holds `fs`, placed into `semester`. A row
/// naming a module plans it, unless the semester holds it already (counted as „schon geplant";
/// another semester does not count, the timetable is this one); every other row becomes a
/// placeholder, unless its row has one in the semester already. A row over several Fachsemester
/// comes with each of them, into the semester it is taken over into (owner, 2026-09-26: „nur dem
/// ersten Semester zugeordnet" was wrong), and its placeholders there count together
/// (`PlanDoc::same_row`). Rows of prose never come. `page` as in `import`. So taking the same
/// Fachsemester twice adds nothing.
pub fn import_fs(doc: &PlanDoc, program_id: &str, core: &PlanVariant, page: Option<(&PlanVariant, i64)>, semester: SemesterKey, fs: u8) -> Import {
    let parts = std::iter::once((core, page.map(|(_, filled)| filled))).chain(page.map(|(page, _)| (page, None)));
    let mut out = Import::default();
    let mut line = ImportFs { fs, semester, modules: Vec::new(), placeholders: Vec::new() };
    let mut taken: BTreeSet<&str> = BTreeSet::new();
    for (variant, filled) in parts {
        for entry in variant.entries.iter().filter(|entry| Some(entry.ord) != filled) {
            let Some((first, last)) = plan::semester_span(entry) else { continue };
            if !(first..=last).contains(&i64::from(fs)) {
                continue;
            }
            let name = plan::shown_name(&entry.module_name).to_string();
            match entry.module_id.as_deref() {
                Some(id) if !url::is_module_id(id) => {}
                Some(id) => {
                    if !taken.insert(id) {
                        continue;
                    }
                    if doc.is_planned(semester, id) {
                        out.skipped += 1;
                        out.held.push(id.to_string());
                        continue;
                    }
                    out.modules.push((semester, id.to_string()));
                    line.modules.push(name);
                }
                None => {
                    let Some(placeholder) = row_placeholder(program_id, variant, entry, semester, &name) else { continue };
                    if doc.placeholders.iter().any(|p| p.program_id == program_id && p.ord == entry.ord && p.semester == semester) {
                        out.skipped += 1;
                        continue;
                    }
                    out.placeholders.push(placeholder);
                    line.placeholders.push(name);
                }
            }
        }
    }
    if !line.modules.is_empty() || !line.placeholders.is_empty() {
        out.by_fs.push(line);
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
pub(crate) fn folded(name: &str) -> String {
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
/// else the `tail` (`text` does exactly that). Its words are in `locale`; the names of the row
/// and of the areas are the plan's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceholderLine {
    /// „≥ 6 LP" for a choice (a module of the area may have more), „6 LP" for one module,
    /// „10–24 LP" for a range; the spaces are non-breaking.
    pub credits: Option<String>,
    /// The same without the unit („≥ 6", „6", „10–24"), for a page that writes the unit itself.
    pub amount: Option<String>,
    /// The row's name, or „aus dem Bereich" / „aus den Bereichen" where the name only repeats its
    /// areas.
    pub lead: Option<String>,
    /// The names of the areas the row takes its modules from.
    pub areas: Vec<String>,
    pub tail: Option<&'static str>,
    /// The language of the line (`placeholder_line`'s), in which `text` lists the areas.
    pub locale: Locale,
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
            (areas, _) => Some(quoted_list(areas, self.locale)),
        };
        if let Some(rest) = rest {
            out.push_str(": ");
            out.push_str(&rest);
        }
        out
    }
}

/// „„A“, „B“ oder „C“" in German, "“A”, “B” or “C”" in English.
fn quoted_list(items: &[String], locale: Locale) -> String {
    let texts = crate::i18n::texts(locale);
    let mut out = String::new();
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            if i + 1 < items.len() {
                out.push_str(", ");
            } else {
                let _ = write!(out, " {} ", texts.or);
            }
        }
        out.push_str(&(texts.quoted)(item));
    }
    out
}

/// The line of a placeholder in `locale`: from `requirement`, what `plan::requirement_of` makes of
/// its row when `resolve_placeholder` found one; else from the stored name, credits and kind,
/// without areas and without a claim about where its modules come from.
pub fn placeholder_line(p: &Placeholder, requirement: Option<&SemesterRequirement>, locale: Locale) -> PlaceholderLine {
    let texts = crate::i18n::texts(locale);
    let (name, credits, single, fues) = match requirement {
        Some(row) => (row.shown_name().to_string(), row.credits.clone(), row.single, row.fues),
        None => {
            let stored = stored_entry(p);
            let credits = p.credits.as_deref().map(|credits| plan::credits_in(credits, locale));
            (plan::shown_name(&p.name).to_string(), credits, plan::is_single_module(&stored), plan::is_fues(&stored))
        }
    };
    let areas: Vec<String> = requirement.map(|row| row.areas.iter().map(|area| area.name().to_string()).collect()).unwrap_or_default();
    // A choice asks for at least that much; one module, or a range, is what it says.
    let amount = credits.map(|credits| match single || credits.contains(['–', '-']) {
        true => credits,
        false => format!("≥\u{a0}{credits}"),
    });
    let name = (!name.is_empty()).then_some(name);
    let (lead, areas, tail) = if single {
        (name, Vec::new(), Some(texts.not_in_catalog))
    } else if fues {
        (Some(texts.fues.to_string()), Vec::new(), None)
    } else if areas.is_empty() {
        // Without its row there is no telling where the modules come from.
        (name, areas, requirement.map(|_| texts.all_electives))
    } else if requirement.is_some_and(SemesterRequirement::named_by_areas) {
        let lead = if areas.len() > 1 { texts.from_areas } else { texts.from_area };
        (Some(lead.to_string()), areas, None)
    } else {
        (name, areas, None)
    };
    PlaceholderLine { credits: amount.as_deref().map(texts.credits), amount, lead, areas, tail, locale }
}

/// Whether a placeholder asks for a module of the Fachübergreifendes Studium, by its stored kind
/// or name (`plan::is_fues`): its modules are the FÜS list, whatever the program's tree says.
pub fn is_fues_placeholder(p: &Placeholder) -> bool {
    plan::is_fues(&stored_entry(p))
}

/// Whether a placeholder asks for one module the plan names without a link to the catalog
/// (Pflicht, Abschlussarbeit, Praktikum: `plan::is_single_module`), not for a choice.
pub fn is_single_placeholder(p: &Placeholder) -> bool {
    plan::is_single_module(&stored_entry(p))
}

/// The most credits a placeholder's row takes: the upper end of a range („10–24" → 24), else the
/// number it states („7,5"); `None` where it states none.
pub fn most_credits(p: &Placeholder) -> Option<f64> {
    let text = p.credits.as_deref()?;
    let last = text.rsplit(['–', '-']).next()?.trim().replace(',', ".");
    last.parse::<f64>().ok().filter(|most| most.is_finite() && *most > 0.0)
}

/// Whether a placeholder's row takes another module, given the credits of the modules that count
/// for it in all its semesters (`None` where the catalog states none). A row that asks for one
/// module takes one; a choice takes modules while their credits stay below the most it takes
/// (owner, 2026-09-26: „die wo das nicht nur ein einziges Modul ist, sondern so ne Range" take
/// more), so „≥ 6" takes one module of 6 and „10–24" up to 24. Where the row or a module states no
/// credits, there is no telling, and it takes more.
pub fn takes_more(p: &Placeholder, chosen: &[Option<f64>]) -> bool {
    if chosen.is_empty() {
        return true;
    }
    if is_single_placeholder(p) {
        return false;
    }
    let known: Option<f64> = chosen.iter().copied().sum();
    match (most_credits(p), known) {
        (Some(most), Some(sum)) => sum + 1e-9 < most,
        _ => true,
    }
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

/// Whether a plan is for a winter or a summer intake. Where its caption says so („Regelstudienplan
/// (Beispiel: Studienbeginn im Sommersemester", „… Beispiel für Start im Wintersemester"), that is
/// the answer: 18 plans of the snapshot of 2026-09-23 name it, and for 10 of them, 6 summer
/// starts among them, the turnus of their modules cannot tell, so the import would suggest a winter
/// start for a plan that says it begins in the summer. Otherwise the modules say it: those the plan
/// puts into one odd Fachsemester (1, 3, 5 …, the semesters of the intake's season) are offered in
/// that season. A season wins with at least twice as many modules as the other; else, or with
/// none, `None`. `linked` are the catalog's rows of the plan's modules.
pub fn intake_season(variant: &PlanVariant, linked: &[CatalogRow]) -> Option<Season> {
    if let Some(season) = stated_intake(&variant.full) {
        return Some(season);
    }
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

/// The season a plan's caption names for the start of studies: it speaks of the start („beginn",
/// „start") and names one season's semester, not both.
fn stated_intake(caption: &str) -> Option<Season> {
    let caption = caption.to_lowercase();
    if !caption.contains("beginn") && !caption.contains("start") {
        return None;
    }
    match (caption.contains("wintersemester"), caption.contains("sommersemester")) {
        (true, false) => Some(Season::Winter),
        (false, true) => Some(Season::Summer),
        _ => None,
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

/// A calendar's name as the store keeps it: as a text field (`clean`), at most
/// `MAX_CALENDAR_NAME` characters.
fn calendar_name(name: &str) -> String {
    let name = clean(name);
    match name.char_indices().nth(MAX_CALENDAR_NAME) {
        Some((at, _)) => name.get(..at).unwrap_or(&name).trim_end().to_string(),
        None => name,
    }
}

/// The Termine of one's own of a list in semester `s`, in the order they came.
fn own_of_semester(list: &[Own], s: SemesterKey) -> Vec<OwnTermin> {
    list.iter().filter(|own| own.semester == s).map(|own| own.termin.clone()).collect()
}

/// A Termin of one's own as the store keeps it: its name and place as text fields (`clean`), at
/// most `MAX_OWN_TEXT` characters; `None` where it is no Termin then.
fn kept_own(termin: OwnTermin) -> Option<OwnTermin> {
    let text = |text: &str| {
        let text = clean(text);
        match text.char_indices().nth(MAX_OWN_TEXT) {
            Some((at, _)) => text.get(..at).unwrap_or(&text).trim_end().to_string(),
            None => text,
        }
    };
    let module = termin.module.filter(|module| url::is_module_id(module));
    let termin = OwnTermin { name: text(&termin.name), place: text(&termin.place), module, ..termin };
    termin.is_valid().then_some(termin)
}

/// The `t` line of a Termin of one's own of calendar `calendar`.
fn own_line(calendar: u8, own: &Own) -> String {
    let termin = &own.termin;
    let days = |days: &[Day]| days.iter().map(|day| day.iso()).collect::<Vec<_>>().join(",");
    let when = match termin.rhythm {
        OwnRhythm::Dates => days(&termin.dates),
        _ => termin.weekday.to_string(),
    };
    let (first, last) = termin.period.map(|(first, last)| (first.iso(), last.iso())).unwrap_or_default();
    format!(
        "t\t{}\t{calendar}\t{}\t{}\t{when}\t{}-{}\t{first}\t{last}\t{}\t{}\t{}\t{}",
        termin.number,
        own.semester.key(),
        termin.rhythm.code(),
        clock(termin.from),
        clock(termin.to),
        termin.module.as_deref().unwrap_or_default(),
        clean(&termin.name),
        clean(&termin.place),
        days(&termin.skip),
    )
}

/// The Termin of one's own of a `t` line and its calendar; `None` for a line that is none. Dates
/// are taken in order and each once; a name or a place too long is cut as the store cuts it.
fn own_of(fields: &[&str]) -> Option<(u8, Own)> {
    let field = |i: usize| fields.get(i).copied().unwrap_or_default();
    let number = number(field(1)).and_then(|n| u32::try_from(n).ok())?;
    let calendar = number_of_calendar(field(2))?;
    let semester = SemesterKey::parse(field(3))?;
    let rhythm = OwnRhythm::from_code(field(4))?;
    let days = |text: &str| -> Option<Vec<Day>> {
        let days: BTreeSet<Day> = text.split(',').map(str::trim).filter(|iso| !iso.is_empty()).map(Day::parse).collect::<Option<_>>()?;
        Some(days.into_iter().collect())
    };
    let (weekday, dates) = match rhythm {
        OwnRhythm::Dates => (0, days(field(5))?),
        _ => (field(5).parse::<u8>().ok()?, Vec::new()),
    };
    let (from, to) = field(6).split_once('-')?;
    let period = match (field(7), field(8)) {
        ("", "") => None,
        (first, last) => Some((Day::parse(first)?, Day::parse(last)?)),
    };
    let module = Some(field(9)).filter(|module| !module.is_empty()).map(str::to_string);
    let termin = OwnTermin {
        number,
        rhythm,
        weekday,
        dates,
        from: minutes(from)?,
        to: minutes(to)?,
        period,
        module,
        name: field(10).to_string(),
        place: field(11).to_string(),
        skip: days(field(12))?,
    };
    Some((calendar, Own { semester, termin: kept_own(termin)? }))
}

/// The calendar a `t` line names: 0 for the plan's, or the number of one of one's own.
fn number_of_calendar(text: &str) -> Option<u8> {
    number(text).and_then(|id| u8::try_from(id).ok()).filter(|id| *id <= MAX_CALENDAR_ID)
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

/// A subscription code as the store keeps it: 1–`MAX_CODE` characters of `folia_pack::ALPHABET`.
fn is_code(code: &str) -> bool {
    (1..=MAX_CODE).contains(&code.len()) && code.bytes().all(|b| folia_pack::ALPHABET.as_bytes().contains(&b))
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
    use folia_model::labels::OfferStatus;
    use folia_query as queries;
    use folia_routes::CatalogQuery;

    use crate::area_fixtures::{row, self as real};
    use crate::variants::{plan_variants, supplements};
    use super::*;

    /// The catalog's rows of `ids`, offered or not, as the Merkliste lists them.
    fn linked_rows(db: &dyn folia_model::Database, ids: &[String]) -> Vec<CatalogRow> {
        let query = CatalogQuery { only_ids: Some(ids.to_vec()), offer: Some(OfferStatus::ALL.to_vec()), ..Default::default() };
        queries::catalog_page(db, &query, 0, ids.len() as u64).unwrap().rows
    }

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
    const CODE: &str = "b3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0";

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
        assert!(text.ends_with("c\t2026W\t148369-a4d12\nr\t2027S\t150132-0abcd\na\t2027S\tb3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0\nz\t2027S\twhat a newer build writes\n"));

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
        assert_eq!(doc.modules, vec![Planned { semester: w, module_id: "12104".to_string(), at: 0, fills: None, from_plan: false }]);
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
        // The longest fields there are: 4-byte characters in names, 3-byte dashes in credits, the
        // largest ord in every program, 4-digit pids.
        let long = |n: usize| Placeholder {
            pid: 0,
            semester: semesters[n % 24],
            program_id: format!("{n:08}-ABCDEFGH-ABCDEFGH-ABCDEFGH-ABCDEFGH"),
            ord: MAX_ORD,
            span: (MAX_SPAN, MAX_SPAN),
            credits: Some("–".repeat(MAX_CREDITS)),
            kind: Some("internship".to_string()),
            caption: "😀".repeat(MAX_TEXT),
            name: "😀".repeat(MAX_TEXT + 10),
        };
        assert_eq!(doc.apply(&with_placeholders((0..MAX_PLACEHOLDERS).map(long).collect()), 0), (0, MAX_PLACEHOLDERS));
        assert_eq!(doc.apply(&with_placeholders(vec![Placeholder { ord: 1, ..long(0) }]), 0), (0, 0), "100 placeholders");
        // `apply` numbers them from 1; the stored text may hold any pid up to 9999.
        for (p, pid) in doc.placeholders.iter_mut().zip(url::MAX_PID + 1 - MAX_PLACEHOLDERS as u32..) {
            p.pid = pid;
        }
        assert_eq!(doc.placeholders.last().map(|p| p.pid), Some(url::MAX_PID));
        let mut n = 0;
        for s in &semesters {
            for _ in 0..MAX_MODULES {
                if n < MAX_PLANNED {
                    assert!(doc.plan(*s, &format!("M{n:031}"), u64::MAX, Some(url::MAX_PID)));
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
                // One date of a Termin of its own: a line each, the longest the dates can write.
                doc.set_date(*s, termin(u32::MAX - n, 0xffffd), s.bounds().0.plus(k as i32 % 180), true);
            }
        }
        let total = |pick: fn(&SemesterHides) -> usize| doc.hidden.values().map(pick).sum::<usize>();
        assert_eq!(
            (total(|h| h.events.len()), total(|h| h.rows.len()), total(|h| h.chosen.len()), total(|h| h.dates.len())),
            (2_000, 2_000, 2_000, 2_000)
        );
        assert_eq!(doc.hidden[&semesters[0]].events.len(), MAX_HIDDEN);
        assert!(!doc.hidden[&semesters[4]].kinds.is_empty() && doc.hidden[&semesters[4]].events.is_empty());
        let later = key("2020W").plus(24).unwrap();
        doc.set_kind(later, EventKind::Lecture, true);
        doc.remember(later, CODE);
        assert!(!doc.hidden.contains_key(&later) && !doc.subscribed.contains_key(&later), "24 semesters");
        // Eight calendars with their longest names, full of modules and ticks in the last semester.
        let modules: Vec<String> = (0..MAX_CALENDAR_MODULES).map(|n| format!("M{n:031}")).collect();
        let ticks: Vec<RowKey> = (0..MAX_CALENDAR_TICKS as u32).map(|n| termin(u32::MAX - n, 0xfffff)).collect();
        for _ in 0..MAX_CALENDARS {
            let id = doc.add_calendar(&"😀".repeat(MAX_CALENDAR_NAME), Some("00000000-ABCDEFGH-ABCDEFGH-ABCDEFGH-ABCDEFGH"), semesters[23], &modules, &ticks).unwrap();
            doc.set_calendar_view(id, CalendarView::Strip);
        }
        // The Termine of one's own at their caps, the longest each can be: 4-byte characters, every
        // date and every skipped day, half of them in the plan's last semester and half in a calendar.
        let dates: Vec<Day> = (0..MAX_OWN_DATES as i32).map(|n| semesters[23].bounds().0.plus(n)).collect();
        let longest = OwnTermin {
            number: 0,
            rhythm: OwnRhythm::Dates,
            weekday: 0,
            dates: dates.clone(),
            from: 0,
            to: 24 * 60,
            period: None,
            module: Some(format!("M{:031}", 0)),
            name: "😀".repeat(MAX_OWN_TEXT),
            place: "😀".repeat(MAX_OWN_TEXT),
            skip: dates,
        };
        let last = doc.calendars.last().map(|calendar| calendar.id).unwrap();
        for n in 0..MAX_OWN_ALL {
            let calendar = if n < MAX_OWN { 0 } else { last };
            assert!(doc.add_own(calendar, semesters[23], OwnTermin { from: n as u16, ..longest.clone() }).is_some(), "{n}");
        }
        assert_eq!(doc.own.iter().chain(doc.calendars.iter().flat_map(|calendar| calendar.own.iter())).map(|own| own.termin.number).max(), Some(MAX_OWN_ALL as u32));
        for n in 0..MAX_EXTRA_LINES {
            doc.extra.push(format!("z{n:03}\t{}", "y".repeat(MAX_EXTRA_BYTES - 5)));
        }
        let text = doc.stored();
        assert!(text.len() < MAX_STORED, "{} bytes", text.len());
        assert_eq!(PlanDoc::restored(&text), doc);
    }

    #[test]
    fn ticks_stand_beside_each_other_and_dates_are_kept_per_termin() {
        let w = key("2026W");
        let day = |iso: &str| Day::parse(iso).unwrap();
        let mut doc = example();
        // A tick beside the choice made: both stay (owner, 2026-10-26: several groups, alike).
        doc.tick(w, termin(148369, 0xaaf38), true);
        assert_eq!(doc.hidden[&w].chosen, BTreeSet::from([termin(148369, 0xa4d12), termin(148369, 0xaaf38)]));
        doc.tick(w, termin(148369, 0xa4d12), false);
        assert_eq!(doc.hidden[&w].chosen, BTreeSet::from([termin(148369, 0xaaf38)]));
        // Dates hidden: one line per Termin, its days in order; a day of another half-year is none.
        let (mit, evs) = (termin(148304, 0xe4bf3), termin(148701, 0xb7025));
        doc.set_date(w, mit, day("2026-11-18"), true);
        doc.set_date(w, mit, day("2026-11-04"), true);
        doc.set_date(w, evs, day("2026-11-06"), true);
        doc.set_date(w, evs, day("2027-04-01"), true);
        let text = doc.stored();
        assert!(text.contains("h\t2026W\t148304-e4bf3\t2026-11-04,2026-11-18\n"), "{text}");
        assert!(text.contains("h\t2026W\t148701-b7025\t2026-11-06\n"), "{text}");
        assert_eq!(PlanDoc::restored(&text), doc);
        assert_eq!(doc.selection(w, TownChoice::Derive).hidden_dates, BTreeSet::from([(mit, day("2026-11-04")), (mit, day("2026-11-18")), (evs, day("2026-11-06"))]));
        doc.set_date(w, mit, day("2026-11-04"), false);
        assert_eq!(doc.hidden[&w].dates.len(), 2);
        // Unplanning a module takes the dates of its events with it; „Alle zeigen" shows every date.
        doc.unplan(w, "11112", &[148304]);
        assert_eq!(doc.hidden[&w].dates, BTreeSet::from([(evs, day("2026-11-06"))]));
        doc.show_all(w);
        assert!(doc.hidden[&w].dates.is_empty());
        // A line of hand-made text keeps the days it can read.
        let read = PlanDoc::restored("h\t2026W\t148304-e4bf3\t2026-11-04,04.11.2026,,2026-02-30\nh\t2026W\tnonsense\t2026-11-04\n");
        assert_eq!(read.hidden[&w].dates, BTreeSet::from([(mit, day("2026-11-04"))]));
    }

    #[test]
    fn a_plan_row_has_one_placeholder_per_semester() {
        let text = "p\t1\t2026W\t079-82-2008\t19\t5-6\t10–24\telective\t\tKomplex Grundlagen der Informatik\n\
            p\t2\t2027S\t079-82-2008\t19\t5-6\t10–24\telective\t\tKomplex Grundlagen der Informatik\n\
            p\t3\t2026W\t079-82-2008\t19\t5-6\t10–24\telective\t\tKomplex Grundlagen der Informatik\n\
            p\t4\t2027S\t048-82-2022\t19\t5-6\t10–24\telective\t\tKomplex Grundlagen der Informatik\n";
        let mut doc = PlanDoc::restored(text);
        // The row in the winter and in the summer; a second one in the winter is dropped.
        assert_eq!(doc.placeholders.iter().map(|p| (p.pid, p.semester.key())).collect::<Vec<_>>(), [(1, "2026W".to_string()), (2, "2027S".to_string()), (4, "2027S".to_string())]);
        assert_eq!(PlanDoc::restored(&doc.stored()), doc);
        let komplex = Placeholder { ord: 19, span: (5, 6), ..placeholder() };
        assert_eq!(doc.apply(&with_placeholders(vec![komplex.clone()]), 0), (0, 0), "the winter has it");
        assert_eq!(doc.apply(&with_placeholders(vec![Placeholder { semester: key("2027W"), ..komplex }]), 0), (0, 1));
        assert_eq!(doc.next_pid(), 6);
        // The row's placeholders in all its semesters; another program's row of the same `ord` is
        // another row.
        let first = doc.placeholders[0].clone();
        assert_eq!(doc.same_row(&first).iter().map(|p| p.pid).collect::<Vec<_>>(), [1, 2, 5]);
        assert_eq!(doc.same_row(&doc.placeholders[2].clone()).iter().map(|p| p.pid).collect::<Vec<_>>(), [4]);
    }

    #[test]
    fn a_choice_takes_modules_up_to_its_credits() {
        let row = |credits: Option<&str>, kind: &str| Placeholder { credits: credits.map(str::to_string), kind: Some(kind.to_string()), ..placeholder() };
        let (komplex, anwendungsfach, open, thesis) = (row(Some("10–24"), "elective"), row(Some("6"), "elective"), row(None, "elective"), row(Some("12"), "thesis"));
        assert_eq!((most_credits(&komplex), most_credits(&anwendungsfach), most_credits(&open)), (Some(24.0), Some(6.0), None));
        assert_eq!(most_credits(&row(Some("7,5"), "elective")), Some(7.5));
        // Nothing counts for it yet: every row takes a module.
        assert!([&komplex, &anwendungsfach, &open, &thesis].iter().all(|p| takes_more(p, &[])));
        // „10–24": modules up to 24.
        assert!(takes_more(&komplex, &[Some(6.0)]) && takes_more(&komplex, &[Some(6.0), Some(12.0)]));
        assert!(!takes_more(&komplex, &[Some(12.0), Some(12.0)]) && !takes_more(&komplex, &[Some(18.0), Some(8.0)]));
        // „≥ 6": one module of 6; one of 5 leaves room.
        assert!(!takes_more(&anwendungsfach, &[Some(6.0)]) && takes_more(&anwendungsfach, &[Some(5.0)]));
        // One module where the row asks for one; no telling where credits are missing.
        assert!(!takes_more(&thesis, &[Some(6.0)]));
        assert!(takes_more(&open, &[Some(30.0)]) && takes_more(&anwendungsfach, &[None]));
        assert!(is_single_placeholder(&thesis) && !is_single_placeholder(&komplex));
    }

    /// Lena's plan as her link hands it on: four modules of 2026W and two groups ticked.
    fn lenas(doc: &mut PlanDoc) -> u8 {
        let modules: Vec<String> = ["11109", "13977", "12160", "12229"].iter().map(|id| id.to_string()).collect();
        doc.add_calendar("Lena", Some("013-82-2018"), key("2026W"), &modules, &[termin(150210, 0x1a2b3), termin(150211, 0x00001)]).unwrap()
    }

    fn day(iso: &str) -> Day {
        Day::parse(iso).unwrap()
    }

    /// Friday's job, 13:00–17:00, every week of the lecture period.
    fn job() -> OwnTermin {
        OwnTermin {
            number: 0,
            rhythm: OwnRhythm::Week,
            weekday: 5,
            dates: Vec::new(),
            from: 13 * 60,
            to: 17 * 60,
            period: None,
            module: None,
            name: "Arbeit".to_string(),
            place: "Café Mosaik".to_string(),
            skip: Vec::new(),
        }
    }

    #[test]
    fn termine_of_ones_own_read_back_whole() {
        let w = key("2026W");
        let mut doc = example();
        let lena = lenas(&mut doc);
        assert_eq!(doc.add_own(0, w, job()), Some(1));
        let group = OwnTermin { rhythm: OwnRhythm::AWeek, weekday: 2, from: 17 * 60 + 15, to: 18 * 60 + 45, module: Some("13700".to_string()), name: "Lerngruppe".to_string(), place: "IKMZ 2.12".to_string(), ..job() };
        assert_eq!(doc.add_own(0, w, group), Some(2));
        let block = OwnTermin { rhythm: OwnRhythm::Dates, weekday: 0, dates: vec![day("2026-11-12"), day("2026-11-19")], from: 9 * 60, to: 24 * 60, name: "Block\tseminar".to_string(), place: String::new(), ..job() };
        assert_eq!(doc.add_own(0, w, block), Some(3));
        let volleyball = OwnTermin { weekday: 4, from: 18 * 60, to: 19 * 60 + 30, period: Some((day("2026-10-15"), day("2027-01-28"))), name: "Volleyball".to_string(), place: "Sporthalle 1".to_string(), ..job() };
        assert_eq!(doc.add_own(0, w, volleyball), Some(4));
        let theirs = OwnTermin { rhythm: OwnRhythm::BWeek, weekday: 3, from: 13 * 60, to: 17 * 60, name: String::new(), place: String::new(), ..job() };
        assert_eq!(doc.add_own(lena, w, theirs), Some(5));
        assert!(doc.skip_own(1, day("2026-10-30"), true));
        assert!(doc.skip_own(1, day("2026-11-06"), true));
        let text = doc.stored();
        assert!(text.starts_with(EXAMPLE), "{text}");
        for line in [
            "t\t1\t0\t2026W\tw\t5\t13:00-17:00\t\t\t\tArbeit\tCafé Mosaik\t2026-10-30,2026-11-06\n",
            "t\t2\t0\t2026W\ta\t2\t17:15-18:45\t\t\t13700\tLerngruppe\tIKMZ 2.12\t\n",
            "t\t3\t0\t2026W\td\t2026-11-12,2026-11-19\t09:00-24:00\t\t\t\tBlock seminar\t\t\n",
            "t\t4\t0\t2026W\tw\t4\t18:00-19:30\t2026-10-15\t2027-01-28\t\tVolleyball\tSporthalle 1\t\n",
            "o\t1\t2026W\t150211-00001\nt\t5\t1\t2026W\tb\t3\t13:00-17:00\t\t\t\t\t\t\n",
        ] {
            assert!(text.contains(line), "{line:?} in {text}");
        }
        let back = PlanDoc::restored(&text);
        assert_eq!(back, doc);
        // The plan's semester shows the plan's, Lena's calendar its own.
        let plan = back.selection(w, TownChoice::Derive).own;
        assert_eq!(plan.iter().map(|own| own.number).collect::<Vec<_>>(), [1, 2, 3, 4]);
        assert_eq!(plan.first().map(|own| own.skip.len()), Some(2));
        assert_eq!(back.calendar(lena).unwrap().selection(w).own.iter().map(|own| own.number).collect::<Vec<_>>(), [5]);
        assert!(back.selection(key("2027S"), TownChoice::Derive).own.is_empty());
        assert_eq!(back.own_in(0, w).len(), 4);
        assert_eq!(back.own_termin(5).map(|(calendar, own)| (calendar, own.semester)), Some((lena, w)));
        // A Termin of one's own plans nothing and counts for nothing.
        assert_eq!(back.semesters(), doc.semesters());
        assert!(!back.is_planned(w, "13700") && !back.is_visited(w, "13700"));
    }

    #[test]
    fn a_termin_of_ones_own_is_changed_skipped_and_taken_away_with_a_way_back() {
        let w = key("2026W");
        let mut doc = PlanDoc::default();
        let lena = lenas(&mut doc);
        let job = doc.add_own(0, w, job()).unwrap();
        let named = OwnTermin { name: format!("  {}  ", "Arbeit".repeat(20)), place: "Café\nMosaik".to_string(), module: Some("not a module".to_string()), ..self::job() };
        assert!(doc.set_own(job, 0, named));
        let (_, own) = doc.own_termin(job).unwrap();
        assert_eq!((own.termin.name.chars().count(), own.termin.place.as_str(), own.termin.module.as_deref(), own.termin.number), (MAX_OWN_TEXT, "Café Mosaik", None, job));
        assert!(!doc.set_own(job, 0, own.termin.clone()), "nothing changed");
        assert!(!doc.set_own(job, 0, OwnTermin { from: 17 * 60, to: 13 * 60, ..self::job() }), "no Termin");
        assert!(!doc.set_own(99, 0, self::job()), "no such Termin");
        // Into Lena's calendar and back, its number kept.
        assert!(doc.set_own(job, lena, self::job()));
        assert_eq!((doc.own_in(0, w).len(), doc.own_in(lena, w).len(), doc.own_termin(job).map(|(calendar, _)| calendar)), (0, 1, Some(lena)));
        assert!(!doc.set_own(job, 9, self::job()), "no such calendar");
        assert!(doc.set_own(job, 0, self::job()));
        // A day skipped, held again; the cap of the days.
        assert!(doc.skip_own(job, day("2026-10-30"), true));
        assert!(!doc.skip_own(job, day("2026-10-30"), true));
        assert!(doc.skip_own(job, day("2026-10-30"), false));
        assert!(!doc.skip_own(job, day("2026-10-30"), false));
        for n in 0..MAX_OWN_DATES as i32 {
            assert!(doc.skip_own(job, day("2026-10-02").plus(7 * n), true));
        }
        assert!(!doc.skip_own(job, day("2027-12-31"), true), "{MAX_OWN_DATES} days");
        // Away, and back where it stood; a new one takes the next number.
        let second = doc.add_own(0, w, OwnTermin { weekday: 1, ..self::job() }).unwrap();
        assert_eq!(second, job + 1);
        let gone = doc.remove_own(job).unwrap();
        assert_eq!((gone.calendar, gone.at, doc.own_in(0, w).len()), (0, 0, 1));
        assert!(doc.remove_own(job).is_none());
        assert!(doc.put_own(gone.clone()));
        assert_eq!(doc.own_in(0, w).iter().map(|own| own.number).collect::<Vec<_>>(), [job, second]);
        assert!(!doc.put_own(gone), "its number is taken");
        // A calendar's go with it, and come back with it.
        let theirs = doc.add_own(lena, w, self::job()).unwrap();
        let (at, calendar) = doc.remove_calendar(lena).unwrap();
        assert!(doc.own_termin(theirs).is_none());
        assert!(doc.put_calendar(at, calendar));
        assert_eq!(doc.own_termin(theirs).map(|(calendar, _)| calendar), Some(lena));
        // „Plan leeren" takes the plan's of the semester, and leaves the calendars' and the others.
        let later = doc.add_own(0, key("2027S"), self::job()).unwrap();
        doc.clear_semester(w);
        assert_eq!((doc.own_termin(job), doc.own_termin(theirs).is_some(), doc.own_termin(later).is_some()), (None, true, true));
    }

    #[test]
    fn a_saved_plan_carries_its_termine_of_ones_own() {
        let w = key("2026W");
        let mut doc = example();
        doc.add_own(0, w, job()).unwrap();
        doc.add_own(0, key("2027S"), OwnTermin { weekday: 2, ..job() }).unwrap();
        let saved = doc.semester_only(w);
        assert_eq!(saved.own.len(), 1);
        assert!(doc.holds_saved(w, &saved));
        // Into another plan whose numbers are taken: numbered anew, and still the same plan.
        let mut other = PlanDoc::default();
        let first = other.add_own(0, key("2027S"), OwnTermin { weekday: 3, ..job() }).unwrap();
        other.put_semester(w, &saved);
        assert_eq!(other.own_in(0, w).iter().map(|own| (own.number, own.weekday)).collect::<Vec<_>>(), [(first + 1, 5)]);
        assert!(other.holds_saved(w, &saved));
        other.skip_own(first + 1, day("2026-10-30"), true);
        assert!(!other.holds_saved(w, &saved), "a skipped day is a change");
    }

    #[test]
    fn termine_of_ones_own_keep_to_their_caps_and_hostile_lines_drop_alone() {
        let w = key("2026W");
        let mut doc = PlanDoc::default();
        for n in 0..MAX_OWN {
            assert!(doc.add_own(0, w, OwnTermin { from: n as u16, to: n as u16 + 1, ..job() }).is_some());
        }
        assert_eq!(doc.add_own(0, w, job()), None, "{MAX_OWN} in a calendar's semester");
        let lena = lenas(&mut doc);
        for n in 0..MAX_OWN {
            assert!(doc.add_own(lena, key("2027S"), OwnTermin { from: n as u16, to: n as u16 + 1, ..job() }).is_some());
        }
        assert_eq!(doc.add_own(0, key("2027W"), job()), None, "{MAX_OWN_ALL} in all");
        assert_eq!(doc.add_own(7, w, job()), None, "no such calendar");

        let text = "t\t1\t0\t2026W\tw\t5\t13:00-17:00\t\t\t\tArbeit\t\t\n                    t\t1\t0\t2026W\tw\t1\t08:00-09:00\t\t\t\tsame number\t\t\n                    t\t2\t3\t2026W\tw\t5\t13:00-17:00\t\t\t\tno calendar 3\t\t\n                    t\t3\t0\t2026W\tw\t8\t13:00-17:00\t\t\t\tno weekday\t\t\n                    t\t4\t0\t2026W\tw\t5\t17:00-13:00\t\t\t\tbackwards\t\t\n                    t\t5\t0\t2026W\tx\t5\t13:00-17:00\t\t\t\tno rhythm\t\t\n                    t\t6\t0\t2026W\td\t\t13:00-17:00\t\t\t\tno date\t\t\n                    t\t7\t0\t2026W\td\t2026-11-19,2026-11-12,2026-11-12\t13:00-17:00\t\t\t\tdates in any order\t\t\n                    t\t8\t0\t2026W\tw\t5\t13:00-17:00\t2026-10-15\t\t\tone end\t\t\n                    t\t0\t0\t2026W\tw\t5\t13:00-17:00\t\t\t\tnumber 0\t\t\n                    t\t100000\t0\t2026W\tw\t5\t13:00-17:00\t\t\t\ttoo large\t\t\n                    t\t9\t0\t2026W\tw\t5\t13:00-17:00\t\t\t../x\tbad module\t\t2026-10-30,nope\n                    t\t10\t0\t2026W\tw\t5\t13:00-17:00\t\t\t../x\tbad module, no skip\t\t\n                    t\t11\t0\t2026W\tw\t5\t1:00-17:00\t\t\t\tclock\t\t\n";
        let read = PlanDoc::restored(text);
        let names: Vec<(u32, &str, Option<&str>)> = read.own.iter().map(|own| (own.termin.number, own.termin.name.as_str(), own.termin.module.as_deref())).collect();
        assert_eq!(names, [(1, "Arbeit", None), (7, "dates in any order", None), (10, "bad module, no skip", None)]);
        assert_eq!(read.own_termin(7).map(|(_, own)| own.termin.dates.len()), Some(2));
    }

    #[test]
    fn calendars_of_ones_own_read_back_whole() {
        let mut doc = example();
        let lena = lenas(&mut doc);
        let empty = doc.add_calendar("Lerngruppe\tMathe", None, key("2026W"), &[], &[]).unwrap();
        assert_eq!((lena, empty), (1, 2));
        doc.set_calendar_view(empty, CalendarView::Strip);
        let text = doc.stored();
        assert!(
            text.ends_with(
                "l\t1\t\t013-82-2018\tLena\n\
                 n\t1\t2026W\t11109\nn\t1\t2026W\t13977\nn\t1\t2026W\t12160\nn\t1\t2026W\t12229\n\
                 o\t1\t2026W\t150210-1a2b3\no\t1\t2026W\t150211-00001\n\
                 l\t2\tstrip\t\tLerngruppe Mathe\n"
            ),
            "{text}"
        );
        assert!(text.starts_with(EXAMPLE), "the plan's lines first, as before");
        let back = PlanDoc::restored(&text);
        assert_eq!(back, doc);
        let calendar = back.calendar(lena).unwrap();
        assert_eq!((calendar.view, calendar.program.as_deref(), calendar.modules_in(key("2026W")).len(), calendar.modules_in(key("2027S")).len()), (None, Some("013-82-2018"), 4, 0));
        assert_eq!(calendar.selection(key("2026W")).chosen_rows.len(), 2);
        assert!(calendar.selection(key("2027S")).chosen_rows.is_empty());
        // A calendar is no part of the plan: its modules are not planned, and its ticks are its own.
        assert!(!back.is_planned(key("2026W"), "11109"));
        assert_eq!(back.selection(key("2026W"), TownChoice::Derive).chosen_rows.len(), 1);
    }

    #[test]
    fn a_calendar_is_named_switched_and_taken_away_with_a_way_back() {
        let mut doc = PlanDoc::default();
        let lena = lenas(&mut doc);
        assert!(!doc.rename_calendar(lena, "  Lena  "), "trimmed, the same name");
        assert!(doc.rename_calendar(lena, &"Lena".repeat(20)));
        assert_eq!(doc.calendar(lena).map(|calendar| calendar.name.chars().count()), Some(MAX_CALENDAR_NAME));
        doc.set_calendar_view(lena, CalendarView::Off);
        doc.tick_in_calendar(lena, key("2026W"), termin(150212, 2), true);
        doc.tick_in_calendar(lena, key("2026W"), termin(150210, 0x1a2b3), false);
        assert_eq!(doc.calendar(lena).map(|calendar| (calendar.view, calendar.chosen.len())), Some((Some(CalendarView::Off), 2)));
        let second = doc.add_calendar("Jonas", None, key("2026W"), &["12104".to_string()], &[]).unwrap();
        let (at, gone) = doc.remove_calendar(lena).unwrap();
        assert_eq!((at, doc.calendars.len(), doc.calendar(lena)), (0, 1, None));
        // A new one takes the smallest number free; once it is taken, the old one cannot come back.
        let third = doc.add_calendar("Kim", None, key("2026W"), &[], &[]).unwrap();
        assert_eq!(third, lena);
        assert!(!doc.put_calendar(at, gone.clone()));
        doc.remove_calendar(third);
        assert!(doc.put_calendar(at, gone));
        assert_eq!(doc.calendars.iter().map(|calendar| calendar.id).collect::<Vec<_>>(), [lena, second], "where it stood");
        // „Plan leeren", a saved plan and the semester put back leave the calendars as they are.
        let before = doc.calendars.clone();
        doc.clear_semester(key("2026W"));
        assert!(doc.semester_only(key("2026W")).calendars.is_empty());
        let saved = example().semester_only(key("2026W"));
        doc.put_semester(key("2026W"), &saved);
        assert_eq!(doc.calendars, before);
    }

    #[test]
    fn calendars_keep_to_their_caps_and_hostile_lines_drop_alone() {
        let text = "l\t0\t\t\tZero\n\
            l\t100\t\t\tHundred\n\
            l\t7\tsideways\tnot a program\tSeven\n\
            l\t7\tfull\t\tSeven again\n\
            n\t7\t2026W\t12104\n\
            n\t7\t2026W\t12104\n\
            n\t7\tlater\t12107\n\
            n\t8\t2026W\t12107\n\
            n\t7\t2026W\t../../etc\n\
            o\t7\t2026W\t148369-a4d12\n\
            o\t7\t2026W\t148369\n\
            o\t9\t2026W\t148369-a4d12\n\
            l\t8\tstrip\t\t\n";
        let doc = PlanDoc::restored(text);
        let ids: Vec<(u8, &str, Option<CalendarView>)> = doc.calendars.iter().map(|calendar| (calendar.id, calendar.name.as_str(), calendar.view)).collect();
        assert_eq!(ids, [(7, "Seven", None), (8, "", Some(CalendarView::Strip))]);
        let seven = doc.calendar(7).unwrap();
        assert_eq!((seven.program.clone(), seven.modules.clone(), seven.chosen.len()), (None, vec![(key("2026W"), "12104".to_string())], 1));
        assert!(doc.calendar(8).unwrap().modules.is_empty(), "a module named before its calendar is none of it");
        // Eight calendars at most, sixty modules and two hundred ticks each.
        let mut doc = PlanDoc::default();
        let many: Vec<String> = (10_000..10_100).map(|id| id.to_string()).collect();
        let ticks: Vec<RowKey> = (0..300).map(|n| termin(150_000 + n, 0xfffff)).collect();
        for n in 0..MAX_CALENDARS {
            assert!(doc.add_calendar(&format!("Kalender {n}"), Some("079-82-2008"), key("2026W"), &many, &ticks).is_some());
        }
        assert_eq!(doc.add_calendar("Neun", None, key("2026W"), &[], &[]), None);
        assert!(doc.calendars.iter().all(|calendar| calendar.modules.len() == MAX_CALENDAR_MODULES && calendar.chosen.len() == MAX_CALENDAR_TICKS));
        assert_eq!(PlanDoc::restored(&doc.stored()), doc);
    }

    #[test]
    fn a_visit_stands_in_the_week_counts_nothing_and_reads_back_whole() {
        let (w, s) = (key("2026W"), key("2027S"));
        let mut doc = PlanDoc::default();
        assert!(doc.plan(w, "12104", 10, None));
        // „+ Modul" in „Mein Studiengang": Denkmalpflege, only visited.
        assert!(doc.visit(w, "13676", 20));
        assert!(!doc.visit(w, "13676", 21), "once a semester");
        assert!(!doc.visit(w, "12104", 22), "a planned module becomes a visit by „Nur besuchen“ alone");
        assert!(!doc.visit(w, "../13676", 22));
        assert!(doc.visit(s, "13676", 23), "another semester visits it again");
        // One passed in the semester, taken out of its week (the mark stays), comes back planned.
        let mut passed = PlanDoc::default();
        assert!(passed.plan(w, "11103", 1, None) && passed.pass(w, "11103"));
        passed.unplan(w, "11103", &[]);
        assert!(passed.visit(w, "11103", 2));
        assert!(passed.is_planned(w, "11103") && !passed.is_visited(w, "11103") && passed.passed_in("11103") == Some(w));
        assert_eq!(doc.modules_in(w), ["12104"]);
        assert_eq!((doc.visits_in(w), doc.week_in(w)), (vec!["13676".to_string()], vec!["12104".to_string(), "13676".to_string()]));
        assert!(!doc.is_planned(w, "13676") && doc.is_visited(w, "13676") && doc.planned_in("13676").is_empty(), "a visit is planned nowhere");
        assert_eq!(doc.selection(w, TownChoice::Derive).visits, BTreeSet::from(["13676".to_string()]));
        assert!(doc.selection(key("2027W"), TownChoice::Derive).visits.is_empty());
        assert_eq!(doc.semesters(), [w, s]);
        assert!(!PlanDoc { modules: Vec::new(), ..doc.clone() }.is_empty(), "visits alone are a plan");
        // The text: after the planned modules, and back whole.
        let text = doc.stored();
        assert_eq!(text, "m\t2026W\t12104\t10\t\nv\t2026W\t13676\t20\nv\t2027S\t13676\t23\n");
        assert_eq!(PlanDoc::restored(&text), doc);
        // Planned and visited in one semester, in either order: planned.
        for text in ["v\t2026W\t12104\t5\nm\t2026W\t12104\t1\t\n", "m\t2026W\t12104\t1\t\nv\t2026W\t12104\t5\n", "v\t2026W\t12104\t5\nv\t2026W\t12104\t6\n"] {
            let read = PlanDoc::restored(text);
            assert_eq!(read.week_in(w), ["12104"], "{text}");
        }

        // „Nur besuchen" taken away (in „Mein Studium"): it counts, when it came kept.
        let mut counted = doc.clone();
        assert!(counted.count_visit(w, "13676"));
        assert_eq!(counted.modules.iter().find(|m| m.module_id == "13676").map(|m| (m.semester, m.at)), Some((w, 20)));
        assert!(!counted.is_visited(w, "13676") && counted.is_visited(s, "13676"));
        assert!(!counted.count_visit(w, "13676"), "no visit left there");
        // Planning a visited module (the catalog's „Einplanen") makes it count as well.
        let mut planned = doc.clone();
        assert!(planned.plan(w, "13676", 30, None));
        assert_eq!((planned.week_in(w), planned.visits_in(s)), (vec!["12104".to_string(), "13676".to_string()], vec!["13676".to_string()]));

        // „Nur besuchen" on a planned module: it leaves the study and stays in the week, what it
        // counted for is gone; one passed there stays as it is.
        let mut visited = doc.clone();
        let pid = visited.next_pid();
        assert!(visited.add_placeholder(Placeholder { pid, ..placeholder() }));
        visited.set_fills(w, "12104", Some(pid));
        visited.tick(w, termin(148369, 0xa4d12), true);
        assert!(visited.make_visit(w, "12104"));
        assert!(visited.modules_in(w).is_empty() && visited.fillers(pid).is_empty());
        assert_eq!(visited.visits_in(w), ["13676", "12104"]);
        assert_eq!(visited.visits.iter().find(|v| v.module_id == "12104").map(|v| v.at), Some(10));
        assert_eq!(visited.selection(w, TownChoice::Derive).chosen_rows.len(), 1, "what the semester chose of it stays");
        assert!(!visited.make_visit(w, "12104"));
        assert!(visited.count_visit(w, "12104") && visited.pass(w, "12104"));
        assert!(!visited.make_visit(w, "12104"), "a passed module is no visit");

        // Taken out: with what the semester hides and chose of its own events alone.
        let mut out = doc.clone();
        out.tick(w, termin(150210, 0x1a2b3), true);
        out.tick(w, termin(148369, 0xa4d12), true);
        out.unvisit(w, "13676", &[150210]);
        assert!(out.visits_in(w).is_empty() && out.is_visited(s, "13676"));
        assert_eq!(out.selection(w, TownChoice::Derive).chosen_rows, BTreeSet::from([termin(148369, 0xa4d12)]));

        // „Plan leeren" empties the week, visits and all; a saved plan keeps them and puts them back.
        let saved = doc.semester_only(w);
        assert_eq!(saved.visits_in(w), ["13676"]);
        let mut cleared = doc.clone();
        cleared.clear_semester(w);
        assert!(cleared.week_in(w).is_empty() && cleared.is_visited(s, "13676"));
        assert!(!cleared.holds_saved(w, &saved));
        cleared.put_semester(w, &saved);
        assert_eq!(cleared.week_in(w), ["12104", "13676"]);
        assert!(cleared.holds_saved(w, &saved));
        let mut other = cleared.clone();
        other.unvisit(w, "13676", &[]);
        assert!(!other.holds_saved(w, &saved), "a visit less is another plan");
    }

    #[test]
    fn a_semesters_week_holds_sixty_modules_planned_and_visited() {
        let w = key("2026W");
        let mut doc = PlanDoc::default();
        for n in 0..MAX_MODULES - 1 {
            assert!(doc.plan(w, &(10_000 + n).to_string(), 1, None));
        }
        assert!(doc.visit(w, "20000", 2));
        assert!(!doc.visit(w, "20001", 2) && !doc.plan(w, "20001", 2, None), "the week is full");
        // A visit that becomes planned keeps its place.
        assert!(doc.count_visit(w, "20000"));
        assert_eq!(doc.modules_in(w).len(), MAX_MODULES);
        assert_eq!(PlanDoc::restored(&doc.stored()), doc);
    }

    #[test]
    fn unknown_tags_survive_a_write() {
        let text = "x-future\tsomething\nm\t2026W\t12104\t1\t\ny\t2026W\t1\n";
        let mut doc = PlanDoc::restored(text);
        assert!(doc.plan(key("2027S"), "11113", 2, None));
        doc.unplan(key("2026W"), "12104", &[]);
        assert_eq!(doc.stored(), "m\t2027S\t11113\t2\t\nx-future\tsomething\ny\t2026W\t1\n");
    }

    #[test]
    fn what_was_passed_and_done_is_kept_with_the_plan() {
        let (w, s) = (key("2026W"), key("2027S"));
        let row = |ord: i64, name: &str| DoneRow { semester: s, program_id: "079-82-2008".into(), ord, caption: String::new(), name: name.into() };
        let mut doc = example();
        assert!(doc.pass(w, "12104"));
        assert!(!doc.pass(w, "12104"), "passed there already");
        assert!(doc.pass(s, "12104"), "the mark moves to the semester it was passed in");
        assert!(!doc.pass(w, "<script>"));
        assert!(doc.finish_row(row(15, "Anwendungs\tfach")));
        assert!(!doc.finish_row(row(15, "Anwendungs fach")), "the same mark");
        assert!(!doc.finish_row(DoneRow { program_id: "../etc".into(), ..row(16, "x") }));
        assert!(!doc.finish_row(row(0, "x")));
        let text = doc.stored();
        assert!(text.contains("d\t2027S\t12104\n") && text.contains("q\t2027S\t079-82-2008\t15\t\tAnwendungs fach\n"), "{text}");
        assert_eq!(PlanDoc::restored(&text), doc);
        assert_eq!(doc.passed_in("12104"), Some(s));
        assert_eq!(doc.done_row("079-82-2008", 15).map(|row| row.name.as_str()), Some("Anwendungs fach"));
        // Neither is planned: the timetable does not hear of them.
        assert_eq!(doc.modules_in(s), Vec::<String>::new());

        // Hostile and repeated lines are dropped alone; the first mark of a module or a row counts.
        let read = PlanDoc::restored(
            "d\t2026X\t11001\nd\t2026W\t<b>11002</b>\nd\t2026W\t11001\nd\t2027S\t11001\nq\t2026W\t../x\t3\t\tA\nq\t2026W\t079-82-2008\tthree\t\tA\nq\t2026W\t079-82-2008\t3\t\tA\nq\t2027S\t079-82-2008\t3\t\tB\n",
        );
        assert_eq!(read.passed, [Passed { semester: w, module_id: "11001".into() }]);
        assert_eq!(read.done_rows, [DoneRow { semester: w, ord: 3, name: "A".into(), ..row(3, "") }]);
        assert!(read.is_empty() && read.extra.is_empty());
        let many: String = (0..MAX_PASSED + 5).map(|n| format!("d\t2026W\t{}\n", 10_000 + n)).collect();
        assert_eq!(PlanDoc::restored(&many).passed.len(), MAX_PASSED);

        doc.unpass("12104");
        doc.reopen_row("079-82-2008", 15);
        assert!(doc.passed.is_empty() && doc.done_rows.is_empty());
        assert_eq!(doc.stored(), EXAMPLE);
    }

    #[test]
    fn plan_leeren_empties_the_semester_alone() {
        let (w, s) = (key("2026W"), key("2027S"));
        let mut doc = example();
        assert!(doc.plan(s, "11113", 1, Some(1)));
        doc.remember(w, CODE);
        assert!(doc.pass(w, "12104"));
        doc.extra.push("z\tnewer".into());
        doc.program = Some("079-82-2008".into());
        doc.clear_semester(w);
        assert!(doc.modules_in(w).is_empty() && doc.placeholders_in(w).is_empty() && !doc.hidden.contains_key(&w));
        assert_eq!(doc.modules_in(s), ["11113"]);
        assert_eq!(doc.fillers(1).len(), 0, "the placeholder it counted for went with the semester");
        assert_eq!((doc.passed_in("12104"), doc.subscribed.get(&w).map(String::as_str), doc.program.as_deref(), doc.extra.len()), (Some(w), Some(CODE), Some("079-82-2008"), 1));
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
        assert_eq!(doc.modules, vec![Planned { semester: s, module_id: "11103".to_string(), at: 1, fills: Some(1), from_plan: false }]);
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

    #[test]
    fn the_standort_is_derived_from_what_the_regelstudienplan_placed() {
        let (w, s) = (key("2026W"), key("2027S"));
        let ids = |selection: &Selection| selection.town_from.as_ref().map(|from| from.iter().cloned().collect::<Vec<_>>());
        let mut doc = PlanDoc::default();
        // Nothing taken over: every module decides, as before.
        assert!(doc.plan(w, "11103", 1, None));
        assert_eq!(ids(&doc.selection(w, TownChoice::Derive)), None);
        // Taken over: those decide; one planned later, or one found for a placeholder, does not.
        let import = Import { modules: vec![(w, "12104".to_string()), (w, "12107".to_string()), (s, "12204".to_string())], ..with_placeholders(vec![placeholder()]) };
        doc.apply(&import, 2);
        assert!(doc.plan(w, "13000", 3, Some(1)));
        assert_eq!(ids(&doc.selection(w, TownChoice::Derive)), Some(vec!["12104".to_string(), "12107".to_string()]));
        assert_eq!(ids(&doc.selection(s, TownChoice::Only(Town::Cottbus))), Some(vec!["12204".to_string()]));
        // The mark is kept in the text, moves with the module, and an old line without it reads.
        let stored = doc.stored();
        assert!(stored.contains("m\t2026W\t12104\t2\t\tplan\n") && stored.contains("m\t2026W\t11103\t1\t\n"), "{stored}");
        assert_eq!(PlanDoc::restored(&stored), doc);
        doc.move_to(w, key("2027W"), "12107");
        assert_eq!(ids(&doc.selection(key("2027W"), TownChoice::Derive)), Some(vec!["12107".to_string()]));
        assert!(!PlanDoc::restored("m\t2026W\t12104\t2\t\n").modules.iter().any(|m| m.from_plan));
        assert!(!PlanDoc::restored("m\t2026W\t12104\t2\t\tplanned\n").modules.iter().any(|m| m.from_plan));

        // Moved onto a copy of its own that was planned by hand: the copy takes the mark.
        assert!(doc.plan(key("2028S"), "12104", 4, None));
        doc.move_to(w, key("2028S"), "12104");
        assert_eq!(ids(&doc.selection(key("2028S"), TownChoice::Derive)), Some(vec!["12104".to_string()]));

        // Planned before the import, or in a plan stored before the mark: the next import that
        // names it marks it, wherever it is planned, and adds nothing.
        let mut old = PlanDoc::restored("m\t2026W\t12104\t2\t\nm\t2027S\t11103\t2\t\n");
        let again = Import { modules: vec![(w, "12107".to_string())], held: vec!["12104".to_string(), "11103".to_string()], ..Import::default() };
        assert_eq!(old.apply(&again, 5), (1, 0));
        assert_eq!(ids(&old.selection(w, TownChoice::Derive)), Some(vec!["12104".to_string(), "12107".to_string()]));
        assert_eq!(ids(&old.selection(s, TownChoice::Derive)), Some(vec!["11103".to_string()]));
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
        let resolve = |entries: Vec<PlanEntry>| ord_of(resolve_placeholder(&stored, Some(&plan_variants(&entries, &[], Locale::De))));
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
        let variants = plan_variants(&several, &[], Locale::De);
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
        assert_eq!(resolve_placeholder(&stored, Some(&plan_variants(&[plan_row(15, "Nebenfach", (3, 3), caption)], &[], Locale::De))), Resolved::Changed);
        let strangers = plan_variants(&[plan_row(1, "Mathematik", (1, 1), "A"), plan_row(2, "Physik", (1, 1), "B")], &[], Locale::De);
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
            let requirement = plan::requirement_of(&entry, "", &areas, &rows, Locale::De);
            placeholder_line(&placeholder(), Some(&requirement), Locale::De).text()
        };
        let named = |name: &str, kind: ModuleKind| row(name, 3, Some(kind), None);
        // A choice from the areas the name points at.
        let line = placeholder_line(&placeholder(), Some(&plan::requirement_of(&named("Anwendungsfach", ModuleKind::Elective), "", &areas, &rows, Locale::De)), Locale::De);
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
        let alone = |p: Placeholder| placeholder_line(&p, None, Locale::De).text();
        assert_eq!(alone(placeholder()), "≥\u{a0}6\u{a0}LP Anwendungsfach");
        assert_eq!(
            alone(Placeholder { kind: Some("compulsory".to_string()), name: "Raumbezogene Datenbanken und GIS".to_string(), ..placeholder() }),
            "6\u{a0}LP Raumbezogene Datenbanken und GIS: unter diesem Namen nicht im Katalog"
        );
        assert_eq!(alone(Placeholder { name: "Modul aus dem FÜS-Katalog der BTU".to_string(), ..placeholder() }), "≥\u{a0}6\u{a0}LP Fachübergreifendes Studium");
        assert_eq!(alone(Placeholder { credits: Some("10–24".to_string()), name: "Komplex Praktische Informatik".to_string(), ..placeholder() }), "10–24\u{a0}LP Komplex Praktische Informatik");
        assert_eq!(alone(Placeholder { credits: None, ..placeholder() }), "Anwendungsfach");

        // In English: the words and the credits the page's, the names the plan's.
        let english = |entry: PlanEntry| {
            let requirement = plan::requirement_of(&entry, "", &areas, &rows, Locale::En);
            placeholder_line(&placeholder(), Some(&requirement), Locale::En).text()
        };
        let line = placeholder_line(&placeholder(), Some(&plan::requirement_of(&named("Anwendungsfach", ModuleKind::Elective), "", &areas, &rows, Locale::En)), Locale::En);
        assert_eq!((line.credits.as_deref(), line.amount.as_deref()), (Some("≥\u{a0}6\u{a0}CP"), Some("≥\u{a0}6")));
        assert_eq!(
            line.text(),
            "≥\u{a0}6\u{a0}CP Anwendungsfach: “Mathematik”, “Maschinenbau / Elektrotechnik”, “Wirtschaftswissenschaften”, “Bauingenieurwesen” or “Physik”"
        );
        assert_eq!(
            english(named("Raumbezogene Datenbanken und GIS", ModuleKind::Compulsory)),
            "6\u{a0}CP Raumbezogene Datenbanken und GIS: not in the catalogue under this name"
        );
        assert_eq!(english(named("Fachübergreifendes Studium", ModuleKind::Fues)), "≥\u{a0}6\u{a0}CP Interdisciplinary studies");
        assert_eq!(english(named("Wahlpflichtmodul 3", ModuleKind::Elective)), "≥\u{a0}6\u{a0}CP Wahlpflichtmodul 3: all compulsory elective modules");
        assert_eq!(english(named("Modul aus dem Bereich Praktische Mathematik", ModuleKind::Elective)), "≥\u{a0}6\u{a0}CP from the area: “Praktische Mathematik”");
        let half = |p: Placeholder| placeholder_line(&p, None, Locale::En).text();
        assert_eq!(half(Placeholder { credits: Some("7,5".to_string()), ..placeholder() }), "≥\u{a0}7.5\u{a0}CP Anwendungsfach");
        assert_eq!(placeholder_line(&Placeholder { credits: Some("7,5".to_string()), ..placeholder() }, None, Locale::De).amount.as_deref(), Some("≥\u{a0}7,5"));
        // The FÜS by its name or its kind; any other row is none.
        assert!(is_fues_placeholder(&Placeholder { name: "Modul aus dem FÜS-Katalog der BTU".to_string(), ..placeholder() }));
        assert!(is_fues_placeholder(&Placeholder { kind: Some("fues".to_string()), name: "Wahlmodul".to_string(), ..placeholder() }));
        assert!(!is_fues_placeholder(&placeholder()));
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
                leave: BTreeSet::new(),
                until: None,
                town: TownChoice::Only(Town::Cottbus),
                extra: Vec::new(),
            }
        );
        assert_eq!(doc.stored(), MINE);
        // Semesters of leave and the last semester, kept when the program goes.
        let mut away = MineDoc::restored(&format!("{MINE}leave\t2027W 2027S x 2027S\nuntil\t2031S\n"));
        assert_eq!((away.leave.iter().map(|s| s.key()).collect::<Vec<_>>(), away.until), (vec!["2027S".to_string(), "2027W".to_string()], Some(key("2031S"))));
        assert!(away.stored().ends_with("start\t2026W\nleave\t2027S 2027W\nuntil\t2031S\ntown\tcottbus\n"));
        assert_eq!(MineDoc::restored(&away.stored()), away);
        away.clear_program();
        assert_eq!(away.stored(), "start\t2026W\nleave\t2027S 2027W\nuntil\t2031S\ntown\tcottbus\n");
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
            kind_source: None,
            kind_basis: None,
            plan_semester: None,
            area: None,
            courses: 0,
            choices: Vec::new(),
            marks: None,
        }
    }

    #[test]
    fn the_intake_season_follows_the_odd_semesters() {
        let linked = |id: &str, semester: i64| PlanEntry { module_id: Some(id.to_string()), ..row(id, semester, Some(ModuleKind::Compulsory), None) };
        let rows = [catalog_row("A", "winter"), catalog_row("B", "winter"), catalog_row("C", "summer"), catalog_row("D", "summer"), catalog_row("E", "both"), catalog_row("F", "summer")];
        let season = |entries: Vec<PlanEntry>| intake_season(&plan_variants(&entries, &[], Locale::De).remove(0), &rows);
        // Two winters to one summer; an even semester and a module of both seasons do not count.
        assert_eq!(season(vec![linked("A", 1), linked("B", 3), linked("C", 3), linked("D", 2), linked("E", 1)]), Some(Season::Winter));
        // A module the plan names twice counts once.
        assert_eq!(season(vec![linked("A", 1), linked("C", 3), linked("C", 5)]), None);
        assert_eq!(season(vec![linked("A", 1), linked("C", 3), linked("F", 5)]), Some(Season::Summer));
        assert_eq!(season(vec![linked("A", 1), linked("B", 3), linked("C", 3), linked("F", 5)]), None);
        assert_eq!(season(vec![linked("E", 1), linked("D", 2)]), None);
        // A caption that names the start is the answer, whatever the modules say or leave open.
        let captioned = |caption: &str, entries: Vec<PlanEntry>| {
            let entries: Vec<PlanEntry> =
                entries.into_iter().map(|entry| PlanEntry { specialization: Some(caption.to_string()), ..entry }).collect();
            intake_season(&plan_variants(&entries, &[], Locale::De).remove(0), &rows)
        };
        assert_eq!(captioned("Regelstudienplans (Beispiel: Studienbeginn im Sommersemester", vec![linked("E", 1)]), Some(Season::Summer));
        assert_eq!(captioned("Regelstudienplan – Beispiel für Start im Wintersemester", vec![linked("C", 1), linked("F", 3)]), Some(Season::Winter));
        // Both seasons, or a season that is not the start: the modules decide.
        assert_eq!(captioned("Studienbeginn im Wintersemester und im Sommersemester", vec![linked("C", 1), linked("F", 3)]), Some(Season::Summer));
        assert_eq!(captioned("Praxisphase im Sommersemester", vec![linked("A", 1), linked("B", 3)]), Some(Season::Winter));
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
        let core = plan_variants(&entries, &[], Locale::De).remove(0);
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

        // Twice: nothing new, and „schon geplant" is what the first import added: 10001, named
        // twice, counts once.
        let mut doc = PlanDoc::default();
        assert_eq!(doc.apply(&first, 7), (3, 1));
        assert_eq!(doc.placeholders[0].pid, 1);
        let again = import(&doc, program, &core, None, w, 1);
        assert_eq!((again.modules.len(), again.placeholders.len(), again.skipped), (0, 0, 4));
        assert_eq!(again.held, ["10001", "10002", "10003"]);
        assert!(again.by_fs.is_empty());
        assert_eq!(doc.apply(&again, 8), (0, 0));

        // A page that fills row 4: its rows come in, row 4 does not.
        let page_rows = vec![PlanEntry { specialization: Some("Studienplan · Seite 7".to_string()), ..module(10, "10010", (2, 2)) }];
        let page = plan_variants(&page_rows, &[], Locale::De).remove(0);
        let both = import(&PlanDoc::default(), program, &core, Some((&page, 4)), w, 1);
        assert!(both.placeholders.is_empty());
        assert!(both.modules.contains(&(key("2027S"), "10010".to_string())));
    }

    #[test]
    fn a_row_over_several_fachsemester_comes_with_each_of_them() {
        let module = |ord: i64, id: &str, span: (i64, i64)| PlanEntry { module_id: Some(id.to_string()), ..plan_row(ord, &format!("Modul {id}"), span, "") };
        let entries = vec![
            module(1, "10005", (5, 5)),
            PlanEntry { credits: None, min_credits: Some(10.0), max_credits: Some(24.0), ..plan_row(2, "Komplex Praktische Informatik", (5, 6), "") },
            module(3, "10006", (6, 6)),
        ];
        let core = plan_variants(&entries, &[], Locale::De).remove(0);
        let (w, s, program) = (key("2026W"), key("2027S"), "079-82-2008");
        let mut doc = PlanDoc::default();
        // The fifth Fachsemester into the winter, the sixth into the summer: the row stands in both.
        let fifth = import_fs(&doc, program, &core, None, w, 5);
        assert_eq!(fifth.placeholders.iter().map(|p| (p.semester, p.ord, p.span)).collect::<Vec<_>>(), [(w, 2, (5, 6))]);
        assert_eq!(doc.apply(&fifth, 1), (1, 1));
        let sixth = import_fs(&doc, program, &core, None, s, 6);
        let placed: Vec<(SemesterKey, i64)> = sixth.placeholders.iter().map(|p| (p.semester, p.ord)).collect();
        assert_eq!((sixth.modules.clone(), placed, sixth.skipped), (vec![(s, "10006".to_string())], vec![(s, 2)], 0));
        assert_eq!(doc.apply(&sixth, 2), (1, 1));
        let winter = doc.placeholders_in(w)[0].clone();
        assert_eq!(doc.same_row(&winter).iter().map(|p| p.semester).collect::<Vec<_>>(), [w, s]);
        // Each Fachsemester again adds nothing; the sixth into the winter only what the winter
        // lacks.
        assert_eq!(import_fs(&doc, program, &core, None, s, 6).skipped, 2);
        let again = import_fs(&doc, program, &core, None, w, 6);
        assert_eq!((again.modules.len(), again.placeholders.len(), again.skipped), (1, 0, 1));
    }

    /// The design's pinned imports (C.19) on the snapshot they were taken from; on any snapshot,
    /// every plan of every program taken over twice adds nothing the second time, reads back
    /// whole, and finds each of its placeholders' rows again.
    #[test]
    fn the_regelstudienplan_is_imported() {
        let pinned = folia_test_support::studyplan_db("the_regelstudienplan_is_imported");
        let is_pinned = pinned.is_some();
        let db = pinned.unwrap_or_else(folia_test_support::open);
        let plans = |program: &str| plan_variants(&queries::program_plan_entries(&db, program).unwrap(), &queries::program_plan_totals(&db, program).unwrap(), Locale::De);
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
                assert_eq!(again.skipped, first.modules.len() + first.placeholders.len(), "{} {}", program.id, core.full);
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
        // The design's example, each module marked as taken over from the plan.
        assert_eq!(doc.stored(), EXAMPLE.replace("1790000000\t\n", "1790000000\t\tplan\n"));

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
        assert_eq!(Import { held: Vec::new(), ..again.clone() }, Import { skipped: 23, ..Default::default() });
        // What it holds of the plan's modules is marked by that import.
        let (held, all): (BTreeSet<&String>, BTreeSet<&String>) = (again.held.iter().collect(), fs1.modules.iter().map(|(_, id)| id).collect());
        assert_eq!(held, all);

        // Informatik starts in the winter.
        let ids: Vec<String> = plan.entries.iter().filter_map(|entry| entry.module_id.clone()).collect();
        let linked = linked_rows(&db, &ids);
        assert_eq!(intake_season(plan, &linked), Some(Season::Winter));

        // The plans whose caption names the start: the caption is the answer, and no module
        // majority says the other season. Without it, the modules leave 10 of the 18 open, 6
        // summer starts among them (Elektrotechnik M.Sc. 2018's „Beispiel: Studienbeginn im
        // Sommersemester"), and the import would suggest a winter start for those.
        let mut stated: Vec<(Season, Option<Season>)> = Vec::new();
        for program in queries::programs(&db).unwrap().into_iter().filter(|program| program.has_plan) {
            for variant in plans(&program.id) {
                let Some(season) = stated_intake(&variant.full) else {
                    continue;
                };
                let ids: Vec<String> = variant.entries.iter().filter_map(|entry| entry.module_id.clone()).collect();
                let linked = linked_rows(&db, &ids);
                assert_eq!(intake_season(&variant, &linked), Some(season), "{} {}", program.id, variant.full);
                let by_modules = intake_season(&PlanVariant { full: String::new(), ..variant.clone() }, &linked);
                assert!(by_modules.is_none_or(|by| by == season), "{} {}: {by_modules:?}", program.id, variant.full);
                stated.push((season, by_modules));
            }
        }
        assert_eq!(stated.len(), 18);
        assert_eq!(stated.iter().filter(|(_, by_modules)| by_modules.is_none()).count(), 10);
        assert_eq!(stated.iter().filter(|(season, by)| by.is_none() && *season == Season::Summer).count(), 6);
        let summer = plans("048-90-2018").into_iter().find(|v| v.full.ends_with("Studienbeginn im Sommersemester")).unwrap();
        assert_eq!(intake_start(w, intake_season(&summer, &[])), key("2026S"));

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

    /// The design's example with its program, and a module of another semester.
    fn timetable() -> PlanDoc {
        let mut doc = example();
        doc.program = Some("079-82-2008".to_string());
        assert!(doc.plan(key("2027S"), "11113", 1, None));
        doc
    }

    #[test]
    fn saved_plans_read_back_what_they_wrote() {
        let w = key("2026W");
        let doc = timetable();
        assert!(doc.stored().starts_with("g\t079-82-2008\nm\t2026W\t12104"));
        assert_eq!(PlanDoc::restored(&doc.stored()), doc);

        let mut saved = SavedPlans::default();
        assert!(saved.save(" Informatik 1. FS ", TownChoice::Only(Town::Cottbus), doc.semester_only(w)));
        assert!(saved.save("Für Lea", TownChoice::Derive, doc.semester_only(key("2027S"))));
        assert!(!saved.save("Leer", TownChoice::Derive, doc.semester_only(key("2030S"))), "nothing planned");
        assert!(!saved.save(" \t ", TownChoice::Derive, doc.semester_only(w)), "no name");
        let text = saved.stored();
        assert!(text.starts_with("plan\tInformatik 1. FS\tcottbus\ng\t079-82-2008\nm\t2026W\t12104\t1790000000\t\n"), "{text}");
        assert_eq!(SavedPlans::restored(&text), saved);
        assert_eq!(SavedPlans::restored(&text.replace('\n', "\r\n")), saved);
        let first = saved.get("Informatik 1. FS").unwrap();
        assert_eq!((first.modules(), first.doc.placeholders.len(), first.doc.semesters()), (4, 1, vec![w]));
        assert_eq!(first.doc.hidden.keys().copied().collect::<Vec<_>>(), [w]);

        // The same name replaces the plan in its place; a removed one is gone.
        assert!(saved.save("Informatik 1. FS", TownChoice::Both, doc.semester_only(key("2027S"))));
        assert_eq!(saved.plans.iter().map(|plan| (plan.name.as_str(), plan.modules())).collect::<Vec<_>>(), [("Informatik 1. FS", 1), ("Für Lea", 1)]);
        saved.remove("Für Lea");
        assert_eq!(saved.plans.len(), 1);
        assert_eq!(SavedPlans::restored(""), SavedPlans::default());
    }

    #[test]
    fn saved_plans_keep_to_their_caps() {
        let doc = timetable().semester_only(key("2026W"));
        let mut saved = SavedPlans::default();
        for n in 0..MAX_SAVED {
            assert!(saved.save(&format!("Plan {n}"), TownChoice::Derive, doc.clone()));
        }
        assert!(!saved.save("Einer zu viel", TownChoice::Derive, doc.clone()));
        assert!(saved.save("Plan 3", TownChoice::Both, doc.clone()), "replacing one is no new one");
        assert_eq!(SavedPlans::restored(&saved.stored()), saved);
        // A text of more plans reads the first thirty.
        let more = format!("{}plan\tNoch einer\t\n{}", saved.stored(), doc.stored());
        assert_eq!(SavedPlans::restored(&more), saved);
        // A long name is cut; a long text is read up to its limit.
        let mut one = SavedPlans::default();
        assert!(one.save(&"Ä".repeat(100), TownChoice::Derive, doc.clone()));
        assert_eq!(one.plans[0].name.chars().count(), MAX_SAVED_NAME);
        let long = format!("{}{}", one.stored(), "x".repeat(MAX_SAVED_STORED));
        assert_eq!(SavedPlans::restored(&long), one);
    }

    #[test]
    fn a_bad_line_of_the_saved_plans_is_dropped_alone() {
        let text = "before any plan\n\
            plan\tGut\tsenftenberg\n\
            g\t079-82-2008\n\
            m\t2026W\t12104\t1\t\n\
            m\tWiSe\t12107\t1\t\n\
            m\t2026W\t../../etc\t1\t\n\
            e\t2026W\t0\n\
            m\t2026W\t11112\t1\t\tplan\n\
            plan\t\t\n\
            m\t2026W\t12102\t1\t\n\
            plan\tGut\t\n\
            m\t2026W\t12102\t1\t\n\
            plan\tNichts drin\tmars\n\
            e\t2026W\t149408\n\
            plan\tAuch gut\tnowhere\n\
            m\t2026W\t12102\t1\t\n";
        let saved = SavedPlans::restored(text);
        // No name, a name taken before, nothing planned: the block goes; a bad line alone.
        assert_eq!(saved.plans.iter().map(|plan| plan.name.as_str()).collect::<Vec<_>>(), ["Gut", "Auch gut"]);
        let good = &saved.plans[0];
        assert_eq!((good.town, good.doc.program.as_deref()), (TownChoice::Only(Town::Senftenberg), Some("079-82-2008")));
        assert_eq!(good.doc.modules_in(key("2026W")), ["12104", "11112"]);
        assert!(good.doc.hidden.is_empty());
        assert_eq!(saved.plans[1].town, TownChoice::Derive, "an unknown town is derived");
    }

    #[test]
    fn a_saved_plan_loads_into_the_semester_shown() {
        let (w, s) = (key("2026W"), key("2027S"));
        let mut doc = timetable();
        let pid = doc.placeholders[0].pid;
        assert!(doc.plan(w, "11103", 3, Some(pid)));
        let saved = doc.semester_only(w);
        assert_eq!(saved.fillers(pid).len(), 1);
        assert!(doc.holds_saved(w, &saved));

        // Another timetable in its place: the semester is replaced, the summer stays.
        let mut working = PlanDoc::default();
        assert!(working.plan(w, "12330", 1, None) && working.plan(s, "11113", 1, None));
        working.set_event(w, 150132, true);
        assert!(!working.holds_saved(w, &saved));
        working.put_semester(w, &saved);
        assert!(working.holds_saved(w, &saved));
        assert_eq!(working.modules_in(w), ["12104", "12107", "12102", "11112", "11103"]);
        assert_eq!(working.modules_in(s), ["11113"]);
        assert_eq!(working.program.as_deref(), Some("079-82-2008"));
        assert_eq!(working.selection(w, TownChoice::Derive).hidden_events, BTreeSet::from([149408]));
        // Its placeholder, numbered anew, is still what 11103 counts for.
        let p = working.placeholders_in(w)[0].pid;
        assert_eq!(working.fillers(p).iter().map(|m| m.module_id.as_str()).collect::<Vec<_>>(), ["11103"]);
        // Saved in the winter, loaded in the summer: the summer holds it.
        let mut summer = PlanDoc::default();
        summer.put_semester(s, &saved);
        assert!(summer.holds_saved(s, &saved) && summer.modules_in(s).len() == 5);
        // The next session, or another program picked: the same plan.
        let next_session = PlanDoc { program: None, ..working.clone() };
        assert!(next_session.holds_saved(w, &saved));
        // A change marks it no longer.
        working.unplan(w, "12104", &[]);
        assert!(!working.holds_saved(w, &saved));
    }

    /// „Importieren" in the Stundenplan: Informatik B.Sc. 2008, 1. FS, into the winter shown.
    #[test]
    fn one_fachsemester_is_imported() {
        let Some(db) = folia_test_support::studyplan_db("one_fachsemester_is_imported") else { return };
        let variants = plan_variants(&queries::program_plan_entries(&db, "079-82-2008").unwrap(), &queries::program_plan_totals(&db, "079-82-2008").unwrap(), Locale::De);
        let [plan] = variants.as_slice() else { panic!("Informatik prints one plan") };
        let w = key("2026W");
        let fs1 = import_fs(&PlanDoc::default(), "079-82-2008", plan, None, w, 1);
        assert_eq!(fs1.modules.iter().map(|(s, id)| (*s, id.as_str())).collect::<Vec<_>>(), [(w, "12104"), (w, "12107"), (w, "12102"), (w, "11112")]);
        assert_eq!(fs1.placeholders.iter().map(|p| (p.semester, p.ord, p.name.as_str(), p.credits.as_deref())).collect::<Vec<_>>(), [(w, 17, "Fachübergreifendes Studium", Some("6"))]);
        assert_eq!(fs1.by_fs.len(), 1);

        // A module planned in another semester is still this timetable's; the same Fachsemester
        // twice adds nothing.
        let mut doc = PlanDoc::default();
        assert!(doc.plan(key("2025W"), "12104", 1, None));
        assert!(doc.plan(w, "11112", 1, None));
        let first = import_fs(&doc, "079-82-2008", plan, None, w, 1);
        assert_eq!((first.modules.len(), first.placeholders.len(), first.skipped, first.held.clone()), (3, 1, 1, vec!["11112".to_string()]));
        assert_eq!(doc.apply(&first, 2), (3, 1));
        let again = import_fs(&doc, "079-82-2008", plan, None, w, 1);
        assert_eq!((again.modules.len(), again.placeholders.len(), again.skipped), (0, 0, 5));
        assert!(doc.modules.iter().filter(|m| m.semester == w).all(|m| m.from_plan));

        // The third Fachsemester into the same winter comes on top.
        let fs3 = import_fs(&doc, "079-82-2008", plan, None, w, 3);
        assert_eq!(fs3.modules.iter().map(|(_, id)| id.as_str()).collect::<Vec<_>>(), ["11787", "12202", "11213"]);
        assert_eq!(fs3.placeholders.iter().map(|p| p.ord).collect::<Vec<_>>(), [15]);

        // The fifth into the winter, the sixth into the summer after it: the three Komplexe (10–24
        // LP) and „Seminar oder Praktikum" span both, and stand in both.
        let (mut late, s) = (PlanDoc::default(), key("2027S"));
        let fs5 = import_fs(&late, "079-82-2008", plan, None, w, 5);
        assert_eq!(fs5.placeholders.iter().map(|p| (p.ord, p.span, p.credits.as_deref())).collect::<Vec<_>>(), [(19, (5, 6), Some("10–24")), (20, (5, 6), Some("10–24")), (21, (5, 6), Some("10–24")), (22, (5, 6), Some("4"))]);
        late.apply(&fs5, 1);
        let fs6 = import_fs(&late, "079-82-2008", plan, None, s, 6);
        assert_eq!(fs6.placeholders.iter().map(|p| (p.semester, p.ord)).collect::<Vec<_>>(), [(s, 19), (s, 20), (s, 21), (s, 22)]);
        late.apply(&fs6, 2);
        assert_eq!(import_fs(&late, "079-82-2008", plan, None, s, 6).placeholders.len(), 0);
    }
}
