//! „Mein Studium" (owner, 2026-10-04): the whole study, semester by semester, as it will go and
//! not as the Regelstudienplan prints it. „Der Regelstudienplan ist ja eine Illusion": a module
//! not passed in its semester does not go away, it comes again — first of all — in the next
//! semester that offers it. So every semester from the current one on holds, in this order, what
//! is left over from the semesters before it („nachholen"), what the Regelstudienplan puts into it
//! („fällig"), and what the student planned besides.
//!
//! Nothing of it is stored but what the student says (`PlanDoc`): a module passed (`d`), a row of
//! the plan done (`q`), a module or a row placed into a semester (`m`, `p`: the lines the
//! Stundenplan keeps, so a module placed into the current semester is in its timetable). The rest
//! is worked out anew from the Regelstudienplan, the Studienbeginn and the catalog's turnus
//! whenever one of them changes, and so the plan moves on by itself as the semesters pass: what is
//! not ticked off when a semester ends is due again in the next one that offers it.
//!
//! The rule, for each row of the Regelstudienplan (a row over several Fachsemester counts from its
//! first; a row of prose, with neither credits nor a kind, is no part of it):
//!
//! 1. Passed or done: it stands in the semester it was passed in, ticked off, and nowhere after.
//! 2. Placed by the student into a semester from the current one on: it stands there.
//! 3. Else in the first semester that offers it (its turnus) from its Fachsemester on, and not
//!    before the current one. A row that names no module has no turnus: it stands in each
//!    semester of its span that is not past, and in the current one once its span is.
//!
//! What a semester before the current one held (the plan's rows of its Fachsemester, what was
//! planned there) stays in it, open or ticked off: the past is where the student ticks off what
//! was passed, and what stays open there is due again further on. Modules planned besides the
//! Regelstudienplan stand where they were planned; one that was not passed is not planned again by
//! itself (an elective may be replaced by another one), the row it counted for is.

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::semester::{fachsemester, of_fachsemester, SemesterKey};
use folia_model::labels::{Code, Labelled, ModuleKind, Season, TurnusParity, TurnusSeason};
use folia_model::rows::CatalogRow;
use folia_model::rows_detail::PlanEntry;
use folia_routes::url;

use crate::plan;
use crate::studyplan::{self, folded, DoneRow, Import, ImportFs, PlanDoc};
use crate::variants::PlanVariant;

/// How many semesters after one a turnus is looked for: a module of every other year comes again
/// within four.
const LOOK_AHEAD: i32 = 4;

/// When a module is offered, as the catalog states its turnus. What it does not state does not
/// restrict (R12: not known to be limited is not limited).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offer {
    pub season: Option<TurnusSeason>,
    pub parity: Option<TurnusParity>,
}

impl Offer {
    pub fn of(row: &CatalogRow) -> Self {
        Offer { season: row.turnus_season.as_ref().and_then(Code::known), parity: row.turnus_parity.as_ref().and_then(Code::known) }
    }

    /// Whether semester `s` offers it. A turnus of one season names the half of the year; one of
    /// even or odd years counts by the year the semester begins in (WiSe 2027/28: 2027).
    pub fn offered(self, s: SemesterKey) -> bool {
        let season = match self.season {
            Some(TurnusSeason::Winter) => s.winter,
            Some(TurnusSeason::Summer) => !s.winter,
            _ => true,
        };
        let parity = match self.parity {
            Some(TurnusParity::Even) => s.year.is_multiple_of(2),
            Some(TurnusParity::Odd) => !s.year.is_multiple_of(2),
            None => true,
        };
        season && parity
    }

    /// The first semester from `from` on that offers it; `from` itself where none of the next
    /// ones does (a turnus no semester fits).
    pub fn next(self, from: SemesterKey) -> SemesterKey {
        (0..LOOK_AHEAD).filter_map(|n| from.plus(n)).find(|s| self.offered(*s)).unwrap_or(from)
    }

    /// The last semester before `before` that offers it, not before `floor`.
    pub fn previous(self, before: SemesterKey, floor: SemesterKey) -> Option<SemesterKey> {
        (1..=LOOK_AHEAD).filter_map(|n| before.plus(-n)).take_while(|s| *s >= floor).find(|s| self.offered(*s))
    }

    /// The one half of the year it is offered in, where its turnus names one: „nur im WiSe".
    pub fn only(self) -> Option<Season> {
        match self.season {
            Some(TurnusSeason::Winter) => Some(Season::Winter),
            Some(TurnusSeason::Summer) => Some(Season::Summer),
            _ => None,
        }
    }
}

/// What an item of the study is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
    /// A module: one the Regelstudienplan names, or one planned besides it (`own`).
    Module { id: String, own: bool },
    /// A row of the Regelstudienplan that names no module: a choice („Wahlpflichtmodul aus der
    /// Informatik"), the FÜS, a module the catalog does not know under the row's name. Its `ord`,
    /// the caption of its plan, the Fachsemester it spans and what it says of its credits („6",
    /// „10–24") and its kind (a `ModuleKind` code).
    Row { ord: i64, caption: String, span: (u8, u8), credits: Option<String>, kind: Option<String> },
}

/// Where an item stands, measured against the Regelstudienplan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Where the Regelstudienplan puts it.
    Due,
    /// Left over: the Fachsemester the plan put it into is past and it is not done („nachholen").
    /// `since` is the semester the plan put it into.
    Overdue { since: SemesterKey },
    /// Before its Fachsemester: the student moved it there („vorgezogen").
    Early,
    /// After its Fachsemester: the student moved it there, or the turnus did (the plan's semester
    /// does not offer it, a Studienbeginn in the other half of the year).
    Later,
    /// A module the Regelstudienplan does not name.
    Own,
}

/// One module or row of the study in one semester.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub subject: Subject,
    /// The plan's name for it (`plan::shown_name`), else the catalog's title.
    pub name: String,
    /// What it is worth, as one number: the plan's, else the catalog's; for a row of a range the
    /// least it takes (`range`).
    pub credits: Option<f64>,
    /// The row states a range of credits („10–24"): its share of a semester is not known.
    pub range: bool,
    pub kind: Option<ModuleKind>,
    /// The Fachsemester the Regelstudienplan puts it into (the first of its span).
    pub fs: Option<u8>,
    /// The semester it was passed or done in.
    pub done: Option<SemesterKey>,
    pub place: Place,
    /// The student placed it into this semester (a line of the plan), not the rule.
    pub pinned: bool,
    /// A row the rule places into each semester of its span: it stands in several at once.
    pub spread: bool,
    pub offer: Offer,
    /// The plan's order (its Fachsemester, then its row); a module planned besides the plan after
    /// every row of it, in the order it was planned.
    order: usize,
}

impl Item {
    /// The module's id, for a module.
    pub fn module_id(&self) -> Option<&str> {
        match &self.subject {
            Subject::Module { id, .. } => Some(id),
            Subject::Row { .. } => None,
        }
    }

    /// The same item in every semester it stands in: `m:<id>` for a module, `r:<ord>` for a row.
    pub fn key(&self) -> String {
        match &self.subject {
            Subject::Module { id, .. } => format!("m:{id}"),
            Subject::Row { ord, .. } => format!("r:{ord}"),
        }
    }

    pub fn is_open(&self) -> bool {
        self.done.is_none()
    }
}

/// One semester of the study.
#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub semester: SemesterKey,
    /// Its Fachsemester; `None` past the 30th.
    pub fs: Option<u8>,
    /// From the current semester on: what is left over first, then the plan's rows in its order,
    /// then the student's own modules; what is ticked off keeps its place among them. Before it:
    /// in the plan's order.
    pub items: Vec<Item>,
    /// What its items come to, where each states one number of its own here.
    pub credits: f64,
    /// Some item states no such number (no credits, a range, a row over several semesters).
    pub partial: bool,
    /// What the Regelstudienplan puts into its Fachsemester (`PlanVariant::semester_credits`).
    pub planned: Option<f64>,
    /// After the last Fachsemester of the Regelstudienplan.
    pub beyond: bool,
}

impl Term {
    /// The items not ticked off.
    pub fn open(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|item| item.is_open())
    }
}

/// The study as it will go.
#[derive(Clone, Debug, PartialEq)]
pub struct Study {
    pub start: SemesterKey,
    /// The current semester, not before the Studienbeginn.
    pub now: SemesterKey,
    /// The semesters from the Studienbeginn to the one before `now`.
    pub past: Vec<Term>,
    /// `now` and after it: to the end of the Regelstudienzeit, and on to the last semester that
    /// holds anything.
    pub terms: Vec<Term>,
    /// The credits of what is passed and done (a row of a range with the least it takes).
    pub done_credits: f64,
    /// What the Regelstudienplan comes to (`PlanVariant::credits`, `credits_max`); 0 without one.
    pub total: (f64, f64),
    /// The semester of the Regelstudienplan's last Fachsemester.
    pub regular_end: Option<SemesterKey>,
    /// The last semester with anything open; `None` when nothing is.
    pub end: Option<SemesterKey>,
    /// How many items of the semesters before `now` are open: what the page asks to tick off.
    pub past_open: usize,
}

impl Study {
    /// The semester `s` of the study, before `now` or after it.
    pub fn term(&self, s: SemesterKey) -> Option<&Term> {
        self.past.iter().chain(&self.terms).find(|term| term.semester == s)
    }
}

/// What the study is worked out from.
#[derive(Clone, Copy)]
pub struct Input<'a> {
    pub program_id: &'a str,
    /// The plan of the chosen study direction; `None` for a program without one, whose study is
    /// what the student planned and passed.
    pub core: Option<&'a PlanVariant>,
    /// A page that fills a row of `core`, with the `ord` of that row (`variants::supplements`):
    /// its rows count with the core's, and the row it fills does not.
    pub page: Option<(&'a PlanVariant, i64)>,
    pub start: SemesterKey,
    /// The current semester.
    pub now: SemesterKey,
    pub doc: &'a PlanDoc,
    /// The catalog's rows of the plan's modules and of the ones planned or passed besides it:
    /// their titles, credits and turnus.
    pub rows: &'a [CatalogRow],
}

/// A row of the plan as the study takes it.
struct PlanRow<'a> {
    variant: &'a PlanVariant,
    entry: &'a PlanEntry,
    first: u8,
    last: u8,
}

/// The rows of the plan, by Fachsemester and then in the plan's order, as the import takes them.
fn plan_rows<'a>(input: &Input<'a>) -> Vec<PlanRow<'a>> {
    let Some(core) = input.core else { return Vec::new() };
    let parts = std::iter::once((core, input.page.map(|(_, filled)| filled))).chain(input.page.map(|(page, _)| (page, None)));
    let mut rows = Vec::new();
    for (variant, filled) in parts {
        for entry in variant.entries.iter().filter(|entry| Some(entry.ord) != filled) {
            let Some((first, last)) = plan::semester_span(entry) else { continue };
            let (Ok(first), Ok(last)) = (u8::try_from(first), u8::try_from(last)) else { continue };
            if first == 0 {
                continue;
            }
            rows.push(PlanRow { variant, entry, first, last });
        }
    }
    rows.sort_by_key(|row| row.first);
    rows
}

/// Where an item placed into `s` stands against the semesters `span` of the plan.
fn place_of(s: SemesterKey, span: Option<(SemesterKey, SemesterKey)>, now: SemesterKey) -> Place {
    match span {
        Some((first, last)) if last < now && s > last => Place::Overdue { since: first },
        Some((first, _)) if s < first => Place::Early,
        Some((_, last)) if s > last => Place::Later,
        _ => Place::Due,
    }
}

/// The study of `input`.
pub fn study(input: &Input) -> Study {
    let start = input.start;
    let now = input.now.max(start);
    let rows = plan_rows(input);
    let catalog: BTreeMap<&str, &CatalogRow> = input.rows.iter().map(|row| (row.id.as_str(), row)).collect();
    // Every item, with the semester it stands in.
    let mut placed: Vec<(SemesterKey, Item)> = Vec::new();
    let mut named: BTreeSet<&str> = BTreeSet::new();

    for (order, row) in rows.iter().enumerate() {
        let span = of_fachsemester(start, row.first).zip(of_fachsemester(start, row.last));
        match row.entry.module_id.as_deref() {
            Some(id) if !url::is_module_id(id) => {}
            Some(id) => {
                // A module the plan names twice counts once, at its first Fachsemester.
                if !named.insert(id) {
                    continue;
                }
                let linked = catalog.get(id).copied();
                let item = Item {
                    subject: Subject::Module { id: id.to_string(), own: false },
                    name: plan::shown_name(&row.entry.module_name).to_string(),
                    credits: row.entry.credits.or(row.entry.min_credits).or(linked.and_then(|row| row.credits)),
                    range: false,
                    kind: row.entry.kind.as_ref().and_then(Code::known),
                    fs: Some(row.first),
                    done: None,
                    place: Place::Due,
                    pinned: false,
                    spread: false,
                    offer: linked.map(Offer::of).unwrap_or_default(),
                    order,
                };
                placed.extend(module_places(input, now, id, span, item));
            }
            None => placed.extend(row_places(input, now, row, span, order)),
        }
    }

    // The modules planned or passed besides the plan, in the order they were planned.
    let mut own: Vec<&str> = Vec::new();
    for id in input.doc.modules.iter().map(|m| m.module_id.as_str()).chain(input.doc.passed.iter().map(|p| p.module_id.as_str())) {
        if !named.contains(id) && !own.contains(&id) {
            own.push(id);
        }
    }
    for (n, id) in own.into_iter().enumerate() {
        let linked = catalog.get(id).copied();
        let item = Item {
            subject: Subject::Module { id: id.to_string(), own: true },
            name: linked.map_or_else(|| id.to_string(), |row| row.title.clone()),
            credits: linked.and_then(|row| row.credits),
            range: false,
            kind: None,
            fs: None,
            done: None,
            place: Place::Own,
            pinned: true,
            spread: false,
            offer: linked.map(Offer::of).unwrap_or_default(),
            order: rows.len() + n,
        };
        placed.extend(own_places(input, now, id, item));
    }

    let regular_end = input.core.and_then(|core| u8::try_from(core.semesters).ok()).filter(|n| *n > 0).and_then(|n| of_fachsemester(start, n));
    let mut by_semester: BTreeMap<SemesterKey, Vec<Item>> = BTreeMap::new();
    for (s, item) in placed {
        by_semester.entry(s).or_default().push(item);
    }
    let last = [Some(now), regular_end, by_semester.keys().next_back().copied()].into_iter().flatten().max().unwrap_or(now);
    let term = |s: SemesterKey, items: Vec<Item>| -> Term {
        let fs = fachsemester(s, start);
        let counted = |item: &&Item| item.credits.is_some() && !item.range && !item.spread;
        let credits = items.iter().filter(counted).filter_map(|item| item.credits).sum();
        let partial = items.iter().any(|item| !counted(&item));
        let planned = fs.zip(input.core).filter(|(fs, core)| i64::from(*fs) <= core.semesters).and_then(|(fs, core)| core.semester_credits(i64::from(fs)));
        Term { semester: s, fs, items, credits, partial, planned, beyond: regular_end.is_some_and(|end| s > end) }
    };
    let mut past = Vec::new();
    let mut terms = Vec::new();
    let mut s = start;
    while s <= last {
        let mut items = by_semester.remove(&s).unwrap_or_default();
        if s < now {
            items.sort_by_key(|item| item.order);
            past.push(term(s, items));
        } else {
            // What is ticked off keeps its place: the row a click ticks stays under the pointer.
            items.sort_by_key(|item| {
                let group = match item.place {
                    Place::Overdue { .. } => 0,
                    Place::Own => 2,
                    _ => 1,
                };
                let since = match item.place {
                    Place::Overdue { since } => Some(since),
                    _ => None,
                };
                (group, since, item.order)
            });
            terms.push(term(s, items));
        }
        let Some(next) = s.plus(1) else { break };
        s = next;
    }

    let done_rows: BTreeSet<i64> =
        past.iter().chain(&terms).flat_map(|term| &term.items).filter(|item| item.done.is_some()).filter_map(|item| match &item.subject {
            Subject::Row { ord, .. } => Some(*ord),
            Subject::Module { .. } => None,
        }).collect();
    // A module planned for a row that is done counts with the row, not besides it.
    let counted_by_row = |item: &Item| match &item.subject {
        Subject::Module { id, own: true } => input.doc.modules.iter().filter(|m| m.module_id == *id).filter_map(|m| m.fills).any(|pid| {
            input.doc.placeholders.iter().any(|p| p.pid == pid && p.program_id == input.program_id && done_rows.contains(&p.ord))
        }),
        _ => false,
    };
    let done_credits = past
        .iter()
        .chain(&terms)
        .flat_map(|term| &term.items)
        .filter(|item| item.done.is_some() && !counted_by_row(item))
        .filter_map(|item| item.credits)
        .sum();
    let end = terms.iter().rev().find(|term| term.open().next().is_some()).map(|term| term.semester);
    let past_open = past.iter().map(|term| term.open().count()).sum();
    Study {
        start,
        now,
        past,
        terms,
        done_credits,
        total: input.core.map_or((0.0, 0.0), |core| (core.credits, core.credits_max)),
        regular_end,
        end,
        past_open,
    }
}

/// Where a module of the plan stands (`study`'s rule): once where it was passed, or where it was
/// left open before `now` and where it is due from `now` on.
fn module_places(input: &Input, now: SemesterKey, id: &str, span: Option<(SemesterKey, SemesterKey)>, item: Item) -> Vec<(SemesterKey, Item)> {
    let start = input.start;
    let pins = input.doc.planned_in(id);
    if let Some(passed) = input.doc.passed_in(id) {
        let at = passed.clamp(start, now);
        return vec![(at, Item { done: Some(passed), place: place_of(at, span, now), pinned: pins.contains(&at), ..item })];
    }
    let mut out = Vec::new();
    // What happened: the last semester before `now` it was planned in, else the plan's own.
    let before = pins.iter().copied().filter(|s| *s >= start && *s < now).max().or(span.map(|(first, _)| first).filter(|first| *first < now));
    if let Some(s) = before {
        out.push((s, Item { place: place_of(s, span, now), pinned: pins.contains(&s), ..item.clone() }));
    }
    let later: Vec<SemesterKey> = pins.iter().copied().filter(|s| *s >= now).collect();
    if later.is_empty() {
        let from = span.map_or(now, |(first, _)| first.max(now));
        let s = item.offer.next(from);
        out.push((s, Item { place: place_of(s, span, now), ..item }));
    } else {
        for s in later {
            out.push((s, Item { place: place_of(s, span, now), pinned: true, ..item.clone() }));
        }
    }
    out
}

/// Where a row of the plan without a module stands: as a module does, but without a turnus, and
/// in each semester of its span that is not past where the student placed it nowhere.
fn row_places(input: &Input, now: SemesterKey, row: &PlanRow, span: Option<(SemesterKey, SemesterKey)>, order: usize) -> Vec<(SemesterKey, Item)> {
    let entry = row.entry;
    let credits = plan::credits_of(entry);
    let kind = entry.kind.as_ref().and_then(Code::known);
    // A row of prose is no part of the study, as it is none of the import.
    if credits.is_none() && kind.is_none() {
        return Vec::new();
    }
    let range = matches!((entry.credits, entry.min_credits, entry.max_credits), (None, Some(min), Some(max)) if min != max);
    let item = Item {
        subject: Subject::Row { ord: entry.ord, caption: row.variant.full.clone(), span: (row.first, row.last), credits, kind: kind.map(|kind| kind.code().to_string()) },
        name: plan::shown_name(&entry.module_name).to_string(),
        credits: entry.credits.or(entry.min_credits),
        range,
        kind,
        fs: Some(row.first),
        done: None,
        place: Place::Due,
        pinned: false,
        spread: false,
        offer: Offer::default(),
        order,
    };
    let start = input.start;
    if let Some(done) = done_mark(input, row) {
        let at = done.clamp(start, now);
        return vec![(at, Item { done: Some(done), place: place_of(at, span, now), ..item })];
    }
    let pins: Vec<SemesterKey> =
        input.doc.placeholders.iter().filter(|p| p.program_id == input.program_id && p.ord == entry.ord).map(|p| p.semester).collect();
    let mut out = Vec::new();
    let before = pins.iter().copied().filter(|s| *s >= start && *s < now).max().or(span.map(|(first, _)| first).filter(|first| *first < now));
    if let Some(s) = before {
        out.push((s, Item { place: place_of(s, span, now), pinned: pins.contains(&s), ..item.clone() }));
    }
    // The student's semesters from `now` on, and the rule's: the semesters of its span after the
    // last of them (a row over two semesters taken into this one's timetable is still due in the
    // next), or with none of them every semester of its span not past, else `now`.
    let later: Vec<SemesterKey> = pins.iter().copied().filter(|s| *s >= now).collect();
    let after = later.iter().max().copied();
    let mut rule: Vec<SemesterKey> = Vec::new();
    if let Some((first, last)) = span.filter(|(_, last)| *last >= now) {
        let mut s = first.max(now);
        while s <= last {
            if after.is_none_or(|after| s > after) {
                rule.push(s);
            }
            let Some(next) = s.plus(1) else { break };
            s = next;
        }
    } else if later.is_empty() {
        rule.push(now);
    }
    let spread = later.len() + rule.len() > 1;
    for (s, pinned) in later.into_iter().map(|s| (s, true)).chain(rule.into_iter().map(|s| (s, false))) {
        out.push((s, Item { place: place_of(s, span, now), pinned, spread, ..item.clone() }));
    }
    out
}

/// Where a module planned besides the plan stands: where it was passed, else where it was planned
/// (the last semester before `now` and every one from `now` on).
fn own_places(input: &Input, now: SemesterKey, id: &str, item: Item) -> Vec<(SemesterKey, Item)> {
    let start = input.start;
    if let Some(passed) = input.doc.passed_in(id) {
        return vec![(passed.clamp(start, now), Item { done: Some(passed), ..item })];
    }
    let pins = input.doc.planned_in(id);
    let before = pins.iter().copied().filter(|s| *s >= start && *s < now).max();
    before.into_iter().chain(pins.iter().copied().filter(|s| *s >= now)).map(|s| (s, item.clone())).collect()
}

/// The semester a row of the plan was done in: its mark (`q`) by the row's `ord` and name, or by
/// its name alone where the plan has one row of that name (a later snapshot numbers the rows
/// anew).
fn done_mark(input: &Input, row: &PlanRow) -> Option<SemesterKey> {
    mark_of(input, row.entry).map(|mark| mark.semester)
}

fn mark_of<'a>(input: &Input<'a>, entry: &PlanEntry) -> Option<&'a DoneRow> {
    let name = folded(&entry.module_name);
    let marks = || input.doc.done_rows.iter().filter(|mark| mark.program_id == input.program_id);
    if let Some(mark) = marks().find(|mark| mark.ord == entry.ord && folded(&mark.name) == name) {
        return Some(mark);
    }
    let same_name = plan_rows(input).into_iter().filter(|row| row.entry.module_id.is_none() && folded(&row.entry.module_name) == name).count();
    // A mark of another row's `ord` is that row's, whatever it is called.
    let taken = |mark: &&DoneRow| plan_rows(input).iter().any(|row| row.entry.ord == mark.ord && folded(&row.entry.module_name) == folded(&mark.name));
    marks().filter(|mark| !taken(mark)).find(|mark| same_name == 1 && folded(&mark.name) == name)
}

/// The row of the plan with this `ord`, with its plan.
fn row_of<'a>(input: &Input<'a>, ord: i64) -> Option<(&'a PlanVariant, &'a PlanEntry)> {
    plan_rows(input).into_iter().find(|row| row.entry.ord == ord && row.entry.module_id.is_none()).map(|row| (row.variant, row.entry))
}

/// Where „später" moves an item of semester `from`: the next semester after it that offers it,
/// within the 30 Fachsemester a study may have.
pub fn later(item: &Item, from: SemesterKey, start: SemesterKey) -> Option<SemesterKey> {
    let next = item.offer.next(from.plus(1)?);
    fachsemester(next, start).map(|_| next)
}

/// Where „früher" moves it: the last semester before `from` that offers it, not before `now`.
pub fn earlier(item: &Item, from: SemesterKey, now: SemesterKey) -> Option<SemesterKey> {
    item.offer.previous(from, now)
}

/// Moves an item into semester `to` as its one semester from `now` on (`PlanDoc::place`,
/// `place_row`): a module of the plan is marked as the plan's. `at` is when, for a module planned
/// anew. `false` when nothing changed.
pub fn move_item(doc: &mut PlanDoc, input: &Input, item: &Item, to: SemesterKey, at: u64) -> bool {
    let now = input.now.max(input.start);
    match &item.subject {
        Subject::Module { id, own } => doc.place(id, to, now, at, !own),
        Subject::Row { ord, .. } => {
            let Some((variant, entry)) = row_of(input, *ord) else { return false };
            let name = plan::shown_name(&entry.module_name).to_string();
            let Some(template) = studyplan::row_placeholder(input.program_id, variant, entry, to, &name) else { return false };
            doc.place_row(template, now)
        }
    }
}

/// Puts an item back where the rule places it: it leaves every semester from `now` on.
pub fn reset_item(doc: &mut PlanDoc, input: &Input, item: &Item) {
    let now = input.now.max(input.start);
    match &item.subject {
        Subject::Module { id, .. } => doc.unplace(id, now),
        Subject::Row { ord, .. } => doc.unplace_row(input.program_id, *ord, now),
    }
}

/// Ticks an item off as passed or done in semester `s` (never after the current one), or opens
/// it again (`done` false), taking whatever mark it has away.
pub fn set_done(doc: &mut PlanDoc, input: &Input, item: &Item, s: SemesterKey, done: bool) -> bool {
    let s = s.min(input.now);
    match &item.subject {
        Subject::Module { id, .. } if done => doc.pass(s, id),
        Subject::Module { id, .. } => {
            let had = doc.passed_in(id).is_some();
            doc.unpass(id);
            had
        }
        Subject::Row { ord, caption, .. } => {
            let mark = row_of(input, *ord).and_then(|(_, entry)| mark_of(input, entry)).map(|mark| mark.ord);
            if !done {
                let had = mark.is_some();
                if let Some(ord) = mark {
                    doc.reopen_row(input.program_id, ord);
                }
                return had;
            }
            if let Some(ord) = mark.filter(|marked| marked != ord) {
                doc.reopen_row(input.program_id, ord);
            }
            doc.finish_row(DoneRow { semester: s, program_id: input.program_id.to_string(), ord: *ord, caption: caption.clone(), name: item.name.clone() })
        }
    }
}

/// What „In den Stundenplan" adds to the timetable of a term's semester (`PlanDoc::apply`): its
/// open modules, those of the plan marked as taken over from it, and its open rows as
/// placeholders. What the timetable holds of them already is left out and counted, so taking a
/// term twice adds nothing.
pub fn timetable_import(input: &Input, term: &Term) -> Import {
    let s = term.semester;
    let mut out = Import::default();
    let mut line = ImportFs { fs: term.fs.unwrap_or_default(), semester: s, modules: Vec::new(), placeholders: Vec::new() };
    for item in term.open() {
        match &item.subject {
            Subject::Module { id, own } => {
                if input.doc.is_planned(s, id) {
                    out.skipped += 1;
                    if !own {
                        out.held.push(id.clone());
                    }
                    continue;
                }
                out.modules.push((s, id.clone()));
                line.modules.push(item.name.clone());
            }
            Subject::Row { ord, .. } => {
                if input.doc.placeholders.iter().any(|p| p.program_id == input.program_id && p.ord == *ord && p.semester == s) {
                    out.skipped += 1;
                    continue;
                }
                let Some((variant, entry)) = row_of(input, *ord) else { continue };
                if let Some(placeholder) = studyplan::row_placeholder(input.program_id, variant, entry, s, &item.name) {
                    out.placeholders.push(placeholder);
                    line.placeholders.push(item.name.clone());
                }
            }
        }
    }
    if !line.modules.is_empty() || !line.placeholders.is_empty() {
        out.by_fs.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use folia_locale::Locale;
    use folia_model::labels::OfferStatus;

    use super::*;
    use crate::area_fixtures::row;
    use crate::variants::plan_variants;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    const PROGRAM: &str = "079-82-2008";

    /// A module row of a plan: its id, Fachsemester and credits.
    fn module(ord: i64, id: &str, fs: i64, credits: f64) -> PlanEntry {
        PlanEntry { ord, module_id: Some(id.to_string()), credits: Some(credits), ..row(&format!("Modul {id}"), fs, Some(ModuleKind::Compulsory), None) }
    }

    /// A row without a module over the Fachsemester `from`–`to`.
    fn choice(ord: i64, name: &str, from: i64, to: i64, credits: (f64, f64)) -> PlanEntry {
        let (min, max) = credits;
        let one = (min == max).then_some(min);
        PlanEntry {
            ord,
            semester: None,
            start_semester: Some(from),
            end_semester: Some(to),
            credits: one,
            min_credits: Some(min),
            max_credits: Some(max),
            ..row(name, from, Some(ModuleKind::Elective), None)
        }
    }

    fn offered(id: &str, turnus: &str, credits: f64) -> CatalogRow {
        CatalogRow {
            id: id.to_string(),
            title: format!("Titel {id}"),
            title_de: None,
            title_en: None,
            credits: Some(credits),
            turnus_season: Some(Code::parse(turnus)),
            turnus_parity: None,
            offer_status: Code::Known(OfferStatus::Active),
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

    /// A plan of four Fachsemester for a winter start: winter modules in the odd ones, summer
    /// modules in the even ones, an elective in the 3rd and a choice over the 3rd and 4th.
    fn plan() -> PlanVariant {
        let entries = vec![
            module(1, "11001", 1, 6.0),
            module(2, "11002", 1, 6.0),
            module(3, "11003", 2, 8.0),
            module(4, "11004", 3, 6.0),
            choice(5, "Wahlpflichtmodul aus der Informatik", 3, 3, (6.0, 6.0)),
            module(6, "11006", 4, 6.0),
            choice(7, "Komplex Praktische Informatik", 3, 4, (10.0, 24.0)),
            PlanEntry { ord: 8, credits: None, kind: None, ..row("Summe", 4, None, None) },
        ];
        plan_variants(&entries, &[], Locale::De).remove(0)
    }

    fn catalog() -> Vec<CatalogRow> {
        vec![offered("11001", "winter", 6.0), offered("11002", "winter", 6.0), offered("11003", "summer", 8.0), offered("11004", "winter", 6.0), offered("11006", "summer", 6.0), offered("12345", "both", 5.0)]
    }

    fn input<'a>(core: &'a PlanVariant, doc: &'a PlanDoc, rows: &'a [CatalogRow], start: &str, now: &str) -> Input<'a> {
        Input { program_id: PROGRAM, core: Some(core), page: None, start: key(start), now: key(now), doc, rows }
    }

    /// The items of a semester as `key`s, in their order.
    fn keys(study: &Study, s: &str) -> Vec<String> {
        study.term(key(s)).map(|term| term.items.iter().map(Item::key).collect()).unwrap_or_default()
    }

    #[test]
    fn a_first_semester_is_the_regelstudienplan() {
        let (core, rows, doc) = (plan(), catalog(), PlanDoc::default());
        let study = study(&input(&core, &doc, &rows, "2026W", "2026W"));
        assert!(study.past.is_empty());
        assert_eq!(study.terms.iter().map(|term| term.semester.key()).collect::<Vec<_>>(), ["2026W", "2027S", "2027W", "2028S"]);
        assert_eq!(keys(&study, "2026W"), ["m:11001", "m:11002"]);
        assert_eq!(keys(&study, "2027S"), ["m:11003"]);
        // The choice over two semesters stands in both; the row of prose nowhere.
        assert_eq!(keys(&study, "2027W"), ["m:11004", "r:5", "r:7"]);
        assert_eq!(keys(&study, "2028S"), ["r:7", "m:11006"]);
        assert!(study.terms.iter().flat_map(|term| &term.items).all(|item| item.place == Place::Due && !item.pinned));
        // Its credits: a range over two semesters is no number of either.
        let third = study.term(key("2027W")).unwrap();
        assert_eq!((third.credits, third.partial, third.fs), (12.0, true, Some(3)));
        assert_eq!((study.term(key("2026W")).unwrap().credits, study.term(key("2026W")).unwrap().partial), (12.0, false));
        assert_eq!((study.regular_end, study.end, study.done_credits), (Some(key("2028S")), Some(key("2028S")), 0.0));
    }

    #[test]
    fn what_is_not_passed_comes_first_in_the_next_semester_that_offers_it() {
        let (core, rows) = (plan(), catalog());
        let mut doc = PlanDoc::default();
        // The third semester; of the first two only one module passed.
        assert!(doc.pass(key("2026W"), "11001"));
        let study = study(&input(&core, &doc, &rows, "2026W", "2027W"));
        // The past keeps its semesters: ticked off, or open.
        assert_eq!(keys(&study, "2026W"), ["m:11001", "m:11002"]);
        assert_eq!(keys(&study, "2027S"), ["m:11003"]);
        assert_eq!(study.past_open, 2);
        let past = study.term(key("2026W")).unwrap();
        assert_eq!(past.items.iter().map(|item| item.done.is_some()).collect::<Vec<_>>(), [true, false]);
        // The winter module left over comes first in this winter, the summer one in the summer.
        assert_eq!(keys(&study, "2027W"), ["m:11002", "m:11004", "r:5", "r:7"]);
        assert_eq!(keys(&study, "2028S"), ["m:11003", "r:7", "m:11006"]);
        let now = study.term(key("2027W")).unwrap();
        assert_eq!(now.items[0].place, Place::Overdue { since: key("2026W") });
        assert_eq!(study.term(key("2028S")).unwrap().items[0].place, Place::Overdue { since: key("2027S") });
        assert_eq!(now.credits, 18.0);
        assert_eq!(study.done_credits, 6.0);
    }

    #[test]
    fn a_studienbeginn_in_the_other_half_of_the_year_follows_the_turnus() {
        let (core, rows, doc) = (plan(), catalog(), PlanDoc::default());
        // A summer start of a plan for the winter: the 1st Fachsemester is a summer, which offers
        // neither winter module, so they come in the winter after it, and so on.
        let study = study(&input(&core, &doc, &rows, "2027S", "2027S"));
        assert_eq!(keys(&study, "2027S"), Vec::<String>::new());
        assert_eq!(keys(&study, "2027W"), ["m:11001", "m:11002"]);
        let shifted = study.term(key("2027W")).unwrap();
        assert!(shifted.items.iter().all(|item| item.place == Place::Later && !item.pinned && item.offer.only() == Some(Season::Winter)));
        // The summer module of the 2nd Fachsemester (a winter here) waits for the summer of the 3rd,
        // the winter module of the 3rd for the winter of the 4th; the rows have no turnus.
        assert_eq!(keys(&study, "2028S"), ["m:11003", "r:5", "r:7"]);
        assert_eq!(keys(&study, "2028W"), ["m:11004", "r:7"]);
        assert_eq!(keys(&study, "2029S"), ["m:11006"]);
        // The study runs past the Regelstudienzeit by the semester the turnus costs.
        assert_eq!(study.regular_end, Some(key("2028W")));
        assert_eq!(study.end, Some(key("2029S")));
        assert!(study.term(key("2029S")).unwrap().beyond);
    }

    #[test]
    fn what_the_student_moves_stays_where_it_was_moved() {
        let (core, rows) = (plan(), catalog());
        let mut doc = PlanDoc::default();
        let now = key("2026W");
        let first = {
            let study = study(&input(&core, &doc, &rows, "2026W", "2026W"));
            study.term(now).unwrap().items[1].clone()
        };
        // „später": the next winter, the summer between offers it not.
        assert_eq!(later(&first, now, key("2026W")), Some(key("2027W")));
        let before = doc.clone();
        assert!(move_item(&mut doc, &input(&core, &before, &rows, "2026W", "2026W"), &first, key("2027W"), 7));
        let study_after = study(&input(&core, &doc, &rows, "2026W", "2026W"));
        assert_eq!(keys(&study_after, "2026W"), ["m:11001"]);
        // In the plan's order there: a module of the 1st Fachsemester before those of the 3rd.
        assert_eq!(keys(&study_after, "2027W"), ["m:11002", "m:11004", "r:5", "r:7"]);
        let moved = study_after.term(key("2027W")).unwrap().items.iter().find(|item| item.key() == "m:11002").unwrap().clone();
        assert_eq!((moved.place, moved.pinned), (Place::Later, true));
        assert_eq!(doc.planned_in("11002"), [key("2027W")]);
        assert!(doc.modules.iter().all(|m| m.from_plan && m.at == 7));
        // „früher" back to the first winter, and „zurücksetzen" lets the rule place it again.
        assert_eq!(earlier(&moved, key("2027W"), now), Some(now));
        let before = doc.clone();
        reset_item(&mut doc, &input(&core, &before, &rows, "2026W", "2026W"), &moved);
        assert!(doc.modules.is_empty());
        assert_eq!(keys(&study(&input(&core, &doc, &rows, "2026W", "2026W")), "2026W"), ["m:11001", "m:11002"]);
    }

    #[test]
    fn rows_are_ticked_off_and_moved_as_rows() {
        let (core, rows) = (plan(), catalog());
        let mut doc = PlanDoc::default();
        let at = |doc: &PlanDoc, now: &str| study(&input(&core, doc, &rows, "2026W", now));
        let third = at(&doc, "2027W");
        let choice = third.term(key("2027W")).unwrap().items.iter().find(|item| item.key() == "r:5").unwrap().clone();
        assert_eq!((choice.credits, choice.range, choice.spread), (Some(6.0), false, false));
        let budget = third.term(key("2027W")).unwrap().items.iter().find(|item| item.key() == "r:7").unwrap().clone();
        assert_eq!((budget.credits, budget.range, budget.spread), (Some(10.0), true, true));

        // Ticked off: it stands where it was done, in its place, and counts with what it takes.
        let before = doc.clone();
        assert!(set_done(&mut doc, &input(&core, &before, &rows, "2026W", "2027W"), &choice, key("2027W"), true));
        assert_eq!(doc.done_rows, [DoneRow { semester: key("2027W"), program_id: PROGRAM.into(), ord: 5, caption: String::new(), name: "Wahlpflichtmodul aus der Informatik".into() }]);
        let done = at(&doc, "2027W");
        // Nothing of the first year is passed: its winter modules come first.
        assert_eq!(keys(&done, "2027W"), ["m:11001", "m:11002", "m:11004", "r:5", "r:7"]);
        assert_eq!(done.done_credits, 6.0);
        // A later snapshot numbers the rows anew: the mark finds its row by its name.
        let renumbered = PlanVariant {
            entries: core.entries.iter().map(|entry| PlanEntry { ord: entry.ord + 10, ..entry.clone() }).collect(),
            ..core.clone()
        };
        let moved_on = study(&input(&renumbered, &doc, &rows, "2026W", "2027W"));
        assert!(moved_on.term(key("2027W")).unwrap().items.iter().any(|item| item.key() == "r:15" && item.done == Some(key("2027W"))));
        // Opened again, it is due again.
        let before = doc.clone();
        assert!(set_done(&mut doc, &input(&core, &before, &rows, "2026W", "2027W"), &choice, key("2027W"), false));
        assert!(doc.done_rows.is_empty());

        // „später" for the row over two semesters: it is the next one's alone.
        assert_eq!(later(&budget, key("2027W"), key("2026W")), Some(key("2028S")));
        let before = doc.clone();
        assert!(move_item(&mut doc, &input(&core, &before, &rows, "2026W", "2027W"), &budget, key("2028S"), 0));
        let moved = at(&doc, "2027W");
        assert_eq!(keys(&moved, "2027W"), ["m:11001", "m:11002", "m:11004", "r:5"]);
        assert_eq!(keys(&moved, "2028S"), ["m:11003", "r:7", "m:11006"]);
        assert!(moved.term(key("2028S")).unwrap().items.iter().any(|item| item.key() == "r:7" && item.pinned && !item.spread));
        let before = doc.clone();
        reset_item(&mut doc, &input(&core, &before, &rows, "2026W", "2027W"), &budget);
        assert!(doc.placeholders.is_empty());

        // Past its span and not done, a row is due now, among what is left over; the summer
        // modules left over wait for the summer.
        let late = at(&PlanDoc::default(), "2028W");
        assert_eq!(keys(&late, "2028W"), ["m:11001", "m:11002", "m:11004", "r:5", "r:7"]);
        assert_eq!(keys(&late, "2029S"), ["m:11003", "m:11006"]);
        assert!(late.terms.iter().flat_map(|term| &term.items).all(|item| matches!(item.place, Place::Overdue { .. })));
        assert!(late.term(key("2028W")).unwrap().beyond);
        assert_eq!(late.past_open, 7);
    }

    #[test]
    fn modules_planned_besides_the_plan_stand_where_they_were_planned() {
        let (core, rows) = (plan(), catalog());
        let mut doc = PlanDoc::default();
        // An elective tried in the summer and not passed, one planned for the next summer that the
        // catalog does not know, and one passed in the first winter without being planned.
        assert!(doc.plan(key("2027S"), "12345", 1, None));
        assert!(doc.plan(key("2028S"), "12346", 1, None));
        assert!(doc.pass(key("2026W"), "12347"));
        let study = study(&input(&core, &doc, &rows, "2026W", "2027W"));
        assert_eq!(keys(&study, "2027S"), ["m:11003", "m:12345"]);
        assert_eq!(keys(&study, "2026W"), ["m:11001", "m:11002", "m:12347"]);
        assert_eq!(keys(&study, "2028S"), ["m:11003", "r:7", "m:11006", "m:12346"]);
        let own = |s: &str, id: &str| study.term(key(s)).unwrap().items.iter().find(|item| item.module_id() == Some(id)).unwrap().clone();
        assert_eq!((own("2027S", "12345").place, own("2027S", "12345").name.as_str(), own("2027S", "12345").credits), (Place::Own, "Titel 12345", Some(5.0)));
        assert_eq!((own("2028S", "12346").name.as_str(), own("2028S", "12346").credits), ("12346", None));
        assert!(study.term(key("2028S")).unwrap().partial);
        // The elective not passed is not planned again by itself.
        assert!(study.terms.iter().all(|term| term.items.iter().all(|item| item.module_id() != Some("12345"))));
        assert_eq!(own("2026W", "12347").done, Some(key("2026W")));
        // „später" for an own module of every semester: the next one.
        assert_eq!(later(&own("2028S", "12346"), key("2028S"), key("2026W")), Some(key("2028W")));
    }

    #[test]
    fn a_semester_goes_into_the_timetable_once() {
        let (core, rows) = (plan(), catalog());
        let mut doc = PlanDoc::default();
        assert!(doc.pass(key("2026W"), "11001"));
        let now = key("2027W");
        let study_before = study(&input(&core, &doc, &rows, "2026W", "2027W"));
        let import = timetable_import(&input(&core, &doc, &rows, "2026W", "2027W"), study_before.term(now).unwrap());
        assert_eq!(import.modules, [(now, "11002".to_string()), (now, "11004".to_string())]);
        assert_eq!(import.placeholders.iter().map(|p| (p.ord, p.semester, p.span)).collect::<Vec<_>>(), [(5, now, (3, 3)), (7, now, (3, 4))]);
        assert_eq!(import.by_fs.len(), 1);
        assert_eq!(doc.apply(&import, 9), (2, 2));
        assert_eq!(doc.modules_in(now), ["11002", "11004"]);
        assert!(doc.modules.iter().all(|m| m.from_plan));
        // The same semester again: what it holds is all there, and the row over two semesters is
        // still due in the next one.
        let study_after = study(&input(&core, &doc, &rows, "2026W", "2027W"));
        assert_eq!(keys(&study_after, "2027W"), keys(&study_before, "2027W"));
        assert!(study_after.term(now).unwrap().items.iter().all(|item| item.pinned));
        assert_eq!(keys(&study_after, "2028S"), ["m:11003", "r:7", "m:11006"]);
        let again = timetable_import(&input(&core, &doc, &rows, "2026W", "2027W"), study_after.term(now).unwrap());
        assert!(again.modules.is_empty() && again.placeholders.is_empty());
        assert_eq!((again.skipped, again.held.len()), (4, 2));
    }

    #[test]
    fn a_turnus_says_which_semesters_offer_a_module() {
        let winter = Offer { season: Some(TurnusSeason::Winter), parity: None };
        let odd_summer = Offer { season: Some(TurnusSeason::Summer), parity: Some(TurnusParity::Odd) };
        assert!(winter.offered(key("2026W")) && !winter.offered(key("2027S")));
        assert_eq!(winter.next(key("2027S")), key("2027W"));
        assert_eq!(odd_summer.next(key("2028S")), key("2029S"));
        assert_eq!(odd_summer.previous(key("2029S"), key("2026W")), Some(key("2027S")));
        assert_eq!(odd_summer.previous(key("2029S"), key("2028S")), None);
        // Unknown and „unregelmäßig" restrict nothing; a turnus no semester fits leaves it where it is.
        assert!(Offer::default().offered(key("2027S")) && Offer { season: Some(TurnusSeason::Irregular), parity: None }.offered(key("2027S")));
        assert_eq!(Offer::default().next(key("2027S")), key("2027S"));
        assert_eq!((winter.only(), Offer::default().only()), (Some(Season::Winter), None));
    }

    /// The catalog's rows of `ids`, offered or not.
    fn linked_rows(db: &dyn folia_model::Database, ids: &[String]) -> Vec<CatalogRow> {
        let query = folia_routes::CatalogQuery { only_ids: Some(ids.to_vec()), offer: Some(OfferStatus::ALL.to_vec()), ..Default::default() };
        folia_query::catalog_page(db, &query, 0, ids.len() as u64).unwrap().rows
    }

    /// Every plan of the snapshot, as a student in the third semester with nothing ticked off:
    /// every module of the plan stands once from the current semester on, in a semester that
    /// offers it, what is left over first; the current semester goes into the timetable once; and
    /// ticking the past off leaves nothing left over.
    #[test]
    fn every_plan_of_the_snapshot_is_studied() {
        let db = folia_test_support::open();
        let now = key("2026W");
        let mut studied = 0;
        for program in folia_query::programs(&db).unwrap().into_iter().filter(|program| program.has_plan) {
            let entries = folia_query::program_plan_entries(&db, &program.id).unwrap();
            let variants = plan_variants(&entries, &folia_query::program_plan_totals(&db, &program.id).unwrap(), Locale::De);
            let pages = crate::variants::supplements(&variants);
            let ids: Vec<String> = entries.iter().filter_map(|entry| entry.module_id.clone()).collect();
            let rows = linked_rows(&db, &ids);
            for (i, core) in variants.iter().enumerate().filter(|(i, _)| !pages.iter().any(|page| page.page == *i)) {
                let page = pages.iter().find(|page| page.core == i).map(|page| (&variants[page.page], page.ord));
                let context = format!("{} {}", program.id, core.full);
                // A function, not a closure: each call borrows the plan of its own moment.
                fn made<'a>(program: &'a str, core: &'a PlanVariant, page: Option<(&'a PlanVariant, i64)>, doc: &'a PlanDoc, rows: &'a [CatalogRow]) -> Input<'a> {
                    Input { program_id: program, core: Some(core), page, start: SemesterKey { year: 2025, winter: true }, now: SemesterKey { year: 2026, winter: true }, doc, rows }
                }
                macro_rules! input {
                    ($doc:expr) => {
                        made(&program.id, core, page, $doc, &rows)
                    };
                }
                let mut doc = PlanDoc::default();
                let fresh = study(&input!(&doc));
                let mut seen: BTreeMap<String, usize> = BTreeMap::new();
                for term in &fresh.terms {
                    let mut overdue = true;
                    for item in &term.items {
                        let left_over = matches!(item.place, Place::Overdue { .. });
                        assert!(overdue || !left_over, "{context}: what is left over comes first in {}", term.semester.key());
                        overdue = left_over;
                        if let Some(id) = item.module_id() {
                            *seen.entry(id.to_string()).or_default() += 1;
                            let offering = (0..LOOK_AHEAD).filter_map(|n| term.semester.plus(-n)).any(|s| item.offer.offered(s));
                            assert!(item.offer.offered(term.semester) || !offering, "{context}: {id} in {}", term.semester.key());
                        }
                    }
                }
                assert!(seen.values().all(|n| *n == 1), "{context}: {seen:?}");
                assert!(fresh.terms.first().is_some_and(|term| term.semester == now) && fresh.past.len() == 2, "{context}");

                // The current semester into the timetable, twice.
                let term = fresh.term(now).unwrap().clone();
                let import = timetable_import(&input!(&doc), &term);
                let (modules, placeholders) = (import.modules.len(), import.placeholders.len());
                assert_eq!(doc.apply(&import, 1), (modules, placeholders), "{context}");
                assert!(term.open().filter_map(Item::module_id).all(|id| doc.is_planned(now, id)), "{context}");
                let after = study(&input!(&doc));
                let again = timetable_import(&input!(&doc), after.term(now).unwrap());
                assert!(again.modules.is_empty() && again.placeholders.is_empty(), "{context}");
                assert_eq!(PlanDoc::restored(&doc.stored()), doc, "{context}");

                // The past ticked off: nothing is left over.
                let past: Vec<(SemesterKey, Item)> = fresh.past.iter().flat_map(|term| term.items.iter().map(|item| (term.semester, item.clone()))).collect();
                for (s, item) in &past {
                    let before = doc.clone();
                    set_done(&mut doc, &input!(&before), item, *s, true);
                }
                let ticked = study(&input!(&doc));
                assert_eq!(ticked.past_open, 0, "{context}");
                assert!(ticked.terms.iter().flat_map(|term| &term.items).all(|item| !matches!(item.place, Place::Overdue { .. })), "{context}");
                studied += 1;
            }
        }
        assert!(studied > 100, "{studied} plans");
    }
}
