//! The Gesamtplan as a roadmap (owner, 2026-11-03, on folia/design/gesamtplan/konzepte.html:
//! „Option a ist perfekt bitte setze das um"; asked for as: „so dass es sich eher wie ein progress
//! tracker verhält. Halt so ähnlich wie bei star citizen … Idealer weise trotzdem noch so, dass man
//! die module sehen kann. Also zummindest die abkürzungen", „die progress bars können auch so sein,
//! dass sie unterbrochen sind, wenn man mal ein semester kein modul belegt").
//!
//! The areas as rows, the semesters as columns under the band of the study years, the line of
//! today through the current one (`model.rs` works out what it shows). An area is one bar over the
//! semesters, broken where it has nothing: a piece per module with its abbreviation in the program
//! (`ModuleAbbrevsAsk`), as wide as its share of the semester's credits and never narrower than
//! its label; where the labels do not fit beside each other the semester says „4 Module · 24 LP"
//! (`dom::fit_pieces`). The fuller, the further: passed full in the area's tone, what runs strong,
//! what is planned light, what the plan has open dashed (a click plans it; a row of the plan, a
//! placeholder then, with its module choice in front: `bench::choose_for`), not passed red with
//! ✕, a retake with ↻, not offered ⚠, beyond need hatched — only a module none of whose credits
//! count (owner, 2026-11-09). A click anywhere on an area's head unfolds it (owner, 2026-11-09:
//! „dass man den kompletten bereich als toogle für das aus und einklappen verwenden kann. Der modal
//! mit dem progress ist komplett redundant"): into its subsections where it has them (its subgroups
//! as the Wahlpflicht shows them, each with what it holds against what it asks, „16/10–24"), each
//! of which unfolds into its lanes, else into its lanes at once: a lane per module, the rows of the
//! plan of one name one lane (`RowOf`), a retake tied to its first try by an arrow. One button over
//! the plan unfolds all of it, or folds it once all is unfolded (owner, the same day: „alles
//! auf/zuklappen ein knopf … so dass man halt da im zweifel zwei mal drauf drücken muss"); what is
//! unfolded is kept (`OPEN_KEY`). A row of the plan that its modules fill is carried by them
//! („für „Anwendungsfach“"); one they fill in part is a piece of what it still keeps. The semesters before the current one stand open; „Bisher zuklappen"
//! folds them into one column. What unfolds or folds moves, quiet and quick (owner, 2026-11-05:
//! „animationen für das auf und zuklappen. unauffällig undd snappy einfach so, dass das leicher zu
//! verstehen ist"): a row's subsections and lanes grow in, pushing the rows under them down, and
//! shrink away (`Unfolded`); the columns „Bisher" changes come in from where they went or come
//! from (`Past`). A click on a module (or a right click) opens its menu (`menu.rs`); a module
//! dragged moves to the semester whose column it is let go over, which lights up, and more strongly
//! where it lands; let go over another area's or subsection's row it counts there, where the plan
//! puts it, and with Strg (⌘) held as an exception (`Aim`; owner, 2026-11-09: „damit den
//! angerechneten bereich zu ändern aber nur für bereiche, die an sich im regelstudienplan
//! vorgesehen sind. Für die custom overrides soll es dann nur mit ctrl gehen oder halt über das
//! menu"). „+" in a cell opens the workbench for that semester at that area (`bench.rs`). The
//! title, the band and the semesters' heads stay at the top while the areas scroll under them, as
//! the semester's head does (owner, 2026-11-04: „Ich möchte, dass da der header sticky ist von dem
//! Plan. Also ähnlich wie bei dem Semester plan."): a grid of their own over the areas', each in a
//! box that scrolls sideways and follows the other (`follow`). Too wide for its panel, the plan
//! scrolls sideways by itself to the current semester, the one before it first after the areas'
//! names, which stay at the left (owner, the same day: „dass immer das aktuelle semester passend
//! gezeigt wird"; `show_now`). „Vollbild" at the head's right end lets it fill the page and the
//! browser the screen, the rail and the search staying (`full`), and „Als Bild speichern" draws it
//! as it is shown into a PNG (`picture.rs`).
//! A desktop's view; until Folia 1.69.0 its cells listed the modules' names (the mockup's concept
//! C, owner 2026-10-04).

pub(super) mod model;
mod picture;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use folia_calendar::semester::SemesterKey;
use folia_pages::ask::ModuleAbbrevsAsk;
use folia_plans::bench::{self as groups, Bench, GroupKind};
use folia_plans::choices;
use folia_plans::study::{self, Pick, When};
use leptos::html::Div;
use leptos::prelude::*;

use folia_design::nav;
use folia_design::ui::Icon;

use self::model::{AreaRow, Cell, Column, Fill, Lane, Layout, Piece, SubRow, Subs};
use super::bench::{self, BenchPick};
use super::choose::bar as stand_bar;
use super::dom;
use super::focus::add_pick;
use super::menu::{self, Act, MenuFor};
use super::{n, Selection, StudyCtx};
use crate::i18n::{self, Texts};

/// Where the browser keeps which areas and subsections are unfolded (`localStorage`, a view
/// setting as the view of the semesters is): an area's name, a subsection's `model::sub_key`, one
/// a line.
const OPEN_KEY: &str = "betula.study.open";
/// The grid's panel: what `dom::fit_pieces` measures in, and what a change of width is watched on.
const PANEL_ID: &str = "st-grid";
/// How long the lanes of a row folding stay: as long as they take to shrink away (app.css
/// `rm-lane-out`, .15 s) and a few frames more, at no height then.
const FOLD_OUT: Duration = Duration::from_millis(200);
/// How long the panel says how „Bisher" went (`data-moved`): as long as the columns it changes take
/// to come in (app.css `rm-past-in`, .2 s) and a few frames more.
const PAST_IN: Duration = Duration::from_millis(260);

/// The Gesamtplan's full screen on or off (owner, 2026-11-09: „einen Fullscreen knopf oben rechts
/// haben mit dem klassischen, der den auf den kompletten bildschirm aufzieht. Das soll so sein, dass
/// die nav bar noch funktioniert und oben auch noch die suchleiste ist"): the plan fills the page,
/// the sidebar and the overview aside (app.css `.st-grid-panel.is-full`), and the browser shows the
/// page on the whole screen, the rail and the bar with the search in it. Where the browser does
/// not let it, the plan fills the page all the same. Esc, its button again, or the browser's own
/// way out end both (`StudyPage`).
pub(super) fn full(ctx: StudyCtx, on: bool) {
    if on {
        dom::enter_fullscreen();
    } else {
        dom::leave_fullscreen();
    }
    ctx.full.set(on);
}

/// The areas and subsections unfolded, as the browser keeps them.
fn kept_open() -> BTreeSet<String> {
    nav::local_get(OPEN_KEY).map(|text| text.lines().map(str::to_string).filter(|name| !name.is_empty()).collect()).unwrap_or_default()
}

/// Which rows are unfolded (kept, `OPEN_KEY`: areas by their names, subsections by their keys),
/// and which are folding: their lanes stay while they shrink away (app.css `.rm-out`), each with
/// the turn that folded it, so that the end of a turn leaves alone what a later one folded again.
/// A row unfolded grows its subsections and lanes in (`.rm-in`, `row_view`). With less motion asked
/// for, nothing waits.
#[derive(Clone, Copy)]
struct Unfolded {
    open: RwSignal<BTreeSet<String>>,
    closing: RwSignal<BTreeMap<String, u32>>,
    turn: StoredValue<u32>,
}

impl Unfolded {
    fn new() -> Self {
        Unfolded { open: RwSignal::new(kept_open()), closing: RwSignal::new(BTreeMap::new()), turn: StoredValue::new(0) }
    }

    /// The rows `names` unfolded, every other folded.
    fn keep(self, names: BTreeSet<String>) {
        nav::local_set(OPEN_KEY, &names.iter().cloned().collect::<Vec<_>>().join("\n"));
        let gone: Vec<String> = if dom::still() { Vec::new() } else { self.open.with_untracked(|open| open.difference(&names).cloned().collect()) };
        let turn = self.turn.get_value().wrapping_add(1);
        self.turn.set_value(turn);
        let changed = self.closing.with_untracked(|closing| !gone.is_empty() || closing.keys().any(|name| names.contains(name)));
        if changed {
            self.closing.update(|closing| {
                closing.retain(|name, _| !names.contains(name));
                closing.extend(gone.iter().map(|name| (name.clone(), turn)));
            });
        }
        if !gone.is_empty() {
            set_timeout(
                move || {
                    self.closing.try_update(|closing| closing.retain(|_, at| *at != turn));
                },
                FOLD_OUT,
            );
        }
        self.open.set(names);
    }

    /// A row's head: the row `key` unfolded, or folded where it is.
    fn toggle(self, key: &str) {
        let mut names = self.open.get_untracked();
        if !names.remove(key) {
            names.insert(key.to_string());
        }
        self.keep(names);
    }

    fn unfold(self, key: &str) {
        let mut names = self.open.get_untracked();
        if names.insert(key.to_string()) {
            self.keep(names);
        }
    }

    /// Whether row `key` is unfolded, and whether it is folding; tracked.
    fn state(self, key: String) -> (Memo<bool>, Memo<bool>) {
        let of = key.clone();
        (Memo::new(move |_| self.open.with(|open| open.contains(&key))), Memo::new(move |_| self.closing.with(|closing| closing.contains_key(&of))))
    }
}

/// Whether the semesters before the current one are folded into „Bisher", and, for a moment after
/// a click, which way they went: the panel's `data-moved` („fold", „unfold"), under which the
/// columns of the semesters before the current one (`.past`) come in, „Bisher" from the right where
/// its semesters were, those semesters from the left where „Bisher" was (app.css `rm-past-in`).
#[derive(Clone, Copy)]
struct Past {
    fold: RwSignal<bool>,
    moved: RwSignal<Option<&'static str>>,
    turn: StoredValue<u32>,
}

impl Past {
    fn new() -> Self {
        Past { fold: RwSignal::new(false), moved: RwSignal::new(None), turn: StoredValue::new(0) }
    }

    fn set(self, fold: bool) {
        if self.fold.get_untracked() == fold {
            return;
        }
        if !dom::still() {
            let turn = self.turn.get_value().wrapping_add(1);
            self.turn.set_value(turn);
            self.moved.set(Some(if fold { "fold" } else { "unfold" }));
            set_timeout(
                move || {
                    if self.turn.try_get_value() == Some(turn) {
                        self.moved.try_set(None);
                    }
                },
                PAST_IN,
            );
        }
        self.fold.set(fold);
    }
}

/// A row of the grid a drag can be let go over (a cell's `data-row`): an area's (none for what
/// counts towards none: „a"), a subsection's („g7", the subgroup), an area's „Weitere" as the area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowAt {
    Area(Option<usize>),
    Group(usize),
}

impl RowAt {
    fn attr(self) -> String {
        match self {
            RowAt::Area(Some(area)) => format!("a{area}"),
            RowAt::Area(None) => "a".to_string(),
            RowAt::Group(group) => format!("g{group}"),
        }
    }

    fn parse(text: &str) -> Option<Self> {
        if let Some(group) = text.strip_prefix('g') {
            return group.parse().ok().map(RowAt::Group);
        }
        let area = text.strip_prefix('a')?;
        if area.is_empty() {
            return Some(RowAt::Area(None));
        }
        area.parse().ok().map(|area| RowAt::Area(Some(area)))
    }
}

/// A subgroup a module dragged may count for: its area, its kind and name, whether the plan puts
/// the module there (`folia_plans::bench::fits`), whether it is a choice whose lists hold it, and
/// whether it has what it asks.
#[derive(Clone, Debug, PartialEq)]
struct Target {
    area: usize,
    kind: GroupKind,
    name: String,
    fits: bool,
    listed: bool,
    complete: bool,
}

/// A module dragged over the Gesamtplan: its semester, its key (`Item::key`) and the row it stands
/// in (its area, its subsection), and, where it may count for another subgroup (a module the plan
/// does not name in a row of its own), every subgroup with what the module is there, and the one
/// it counts for now.
#[derive(Clone, Debug, PartialEq)]
struct Dragged {
    semester: SemesterKey,
    key: String,
    area: Option<usize>,
    group: Option<usize>,
    counts: Option<Counts>,
}

#[derive(Clone, Debug, PartialEq)]
struct Counts {
    id: String,
    name: String,
    /// By the index of `Bench::groups`: in the order of the areas, the Pflicht first.
    targets: Vec<Target>,
    now: Option<usize>,
}

/// Where the pointer is during a drag: the semester of its column, the row of its cell, and
/// whether Strg (⌘) is held.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Over {
    semester: SemesterKey,
    row: Option<RowAt>,
    exception: bool,
}

/// Where a drop lands: the semester, the row the module stands in then (its area and subsection),
/// the subgroup it counts for anew and whether as an exception, and the row the pointer is over
/// that it does not count for.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Landing {
    semester: SemesterKey,
    area: Option<usize>,
    group: Option<usize>,
    count: Option<(usize, bool)>,
    refused: Option<RowAt>,
}

impl Landing {
    /// Whether the drop changes anything: another semester, another subgroup.
    fn changes(&self, from: SemesterKey) -> bool {
        self.semester != from || self.count.is_some()
    }
}

/// Where a module dragged lands, let go at `over` (owner, 2026-11-04: „wenn man ein modul drag and
/// dropped, soll einfach das gesamte semester die dropstelle sein"; 2026-11-09: the row it is let go
/// over is what it counts for, where the plan puts it there, with Strg as an exception): in the
/// semester of the column, and over a subsection's row in that subgroup, over an area's row in the
/// subgroup of the area the plan puts it in (a choice whose lists hold it first, one not complete
/// before one that is), with Strg in the subsection or in the area's first subgroup whatever the
/// plan says. Over its own row, a semester's head, what counts towards none, or a row it may not
/// count for, it stays in its row.
fn land(dragged: &Dragged, over: Over) -> Landing {
    let mut count = None;
    let mut refused = None;
    let row = over.row.filter(|row| match row {
        RowAt::Group(group) => Some(*group) != dragged.group,
        RowAt::Area(area) => area.is_some() && *area != dragged.area,
    });
    match (row, &dragged.counts) {
        (Some(RowAt::Group(group)), Some(counts)) if Some(group) != counts.now => match counts.targets.get(group) {
            Some(target) if target.fits || over.exception => count = Some((group, !target.fits)),
            Some(_) => refused = row,
            None => {}
        },
        (Some(RowAt::Area(Some(area))), Some(counts)) => {
            let own: Vec<(usize, &Target)> = counts.targets.iter().enumerate().filter(|(_, target)| target.area == area).collect();
            let mut fitting: Vec<&(usize, &Target)> = own.iter().filter(|(_, target)| target.fits).collect();
            fitting.sort_by_key(|(at, target)| (!target.listed, target.complete, *at));
            match fitting.first().map(|(at, _)| *at) {
                Some(group) if Some(group) != counts.now => count = Some((group, false)),
                Some(_) => {}
                None if over.exception => count = own.first().map(|(at, _)| (*at, true)),
                None => refused = row,
            }
        }
        (Some(_), None) => refused = row,
        _ => {}
    }
    let (area, group) = match (count, &dragged.counts) {
        (Some((group, _)), Some(counts)) => (counts.targets.get(group).map(|target| target.area), Some(group)),
        _ => (dragged.area, dragged.group),
    };
    Landing { semester: over.semester, area, group, count, refused }
}

/// What a row of the plan is to a module dragged (owner, 2026-11-10: „für die drag and drop
/// animation/farbgebung nochmal eine überarbeitung … so dass man einfacher erkennen kann, wo das
/// rein passt und wo nicht"): where it counts by the plan (its own row among them), where it
/// would count as an exception with Strg, and where it cannot count at all (a row of the plan's
/// own module elsewhere, what counts towards none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fit {
    Fits,
    Exception,
    No,
}

/// What `row` is to the module `dragged` (`Fit`), as `land` decides a drop there: an area's row
/// fits where one of its subgroups does.
fn fit(dragged: &Dragged, row: RowAt) -> Fit {
    let counts = dragged.counts.as_ref();
    match row {
        RowAt::Group(group) => {
            if Some(group) == dragged.group || counts.is_some_and(|counts| counts.now == Some(group)) {
                return Fit::Fits;
            }
            match counts.and_then(|counts| counts.targets.get(group)) {
                Some(target) if target.fits => Fit::Fits,
                Some(_) => Fit::Exception,
                None => Fit::No,
            }
        }
        RowAt::Area(area) if area == dragged.area => Fit::Fits,
        RowAt::Area(None) => Fit::No,
        RowAt::Area(Some(area)) => {
            let Some(counts) = counts else { return Fit::No };
            let mut own = counts.targets.iter().filter(|target| target.area == area).peekable();
            if own.peek().is_none() {
                return Fit::No;
            }
            if own.any(|target| target.fits) { Fit::Fits } else { Fit::Exception }
        }
    }
}

/// A module dragged over the Gesamtplan (owner, 2026-11-04: „wenn man ein modul drag and dropped,
/// soll einfach das gesamte semester die dropstelle sein und light gehighlighted werden mit der
/// stelle, wo es final landen soll dann särker"): the semester under the pointer, anywhere in its
/// column (`data-sem`: its head, the areas' cells, the subsections', the lanes'), is where a drop
/// moves it, lit lightly from its head down; the cell where it lands more strongly — its row's, or
/// the row it counts for anew (`land`), marked as an exception where it is one; a row it may not
/// count for says so and what Strg does. Until Folia 1.73.3 only the cell under the pointer took a
/// drop, whichever area's it was; until 1.85.0 a drop never changed what it counts for.
#[derive(Clone, Copy)]
struct Aim {
    what: RwSignal<Option<Dragged>>,
    over: RwSignal<Option<Over>>,
    landing: Memo<Option<Landing>>,
}

impl Aim {
    fn new() -> Self {
        let what: RwSignal<Option<Dragged>> = RwSignal::new(None);
        let over: RwSignal<Option<Over>> = RwSignal::new(None);
        let landing = Memo::new(move |_| {
            let over = over.get()?;
            what.with(|what| what.as_ref().map(|dragged| land(dragged, over)))
        });
        Aim { what, over, landing }
    }

    /// A piece taken up: the item `key` of semester `semester`, in the row of `area` and
    /// subsection `group`. What it may count for is worked out once, here.
    #[allow(clippy::too_many_arguments)]
    fn start(self, ctx: StudyCtx, ev: &leptos::ev::DragEvent, semester: SemesterKey, key: String, area: Option<usize>, group: Option<usize>, t: &'static Texts) {
        let keys = vec![key.clone()];
        dom::drag_data(ev, &keys);
        ctx.drag.set(Some((semester, keys)));
        let counts = key.strip_prefix("m:").and_then(|id| counts_of(ctx, semester, id, t));
        self.what.set(Some(Dragged { semester, key, area, group, counts }));
    }

    /// The drag over: dropped, or let go anywhere else.
    fn end(self, ctx: StudyCtx) {
        ctx.drag.set(None);
        self.what.set(None);
        self.over.set(None);
    }

    /// The pointer over the plan, into a box of it (`dragenter`, each time) or over it (`dragover`,
    /// which a browser may hold back until the drop): the semester of its column and the row of its
    /// cell; a drop is taken where it changes something.
    fn follow(self, ev: &leptos::ev::DragEvent) {
        let Some(from) = self.what.with_untracked(|what| what.as_ref().map(|dragged| dragged.semester)) else { return };
        let at = dom::drop_at(ev).map(|(semester, row)| Over { semester, row: row.as_deref().and_then(RowAt::parse), exception: ev.ctrl_key() || ev.meta_key() });
        let takes = at.is_some_and(|over| self.what.with_untracked(|what| what.as_ref().is_some_and(|dragged| land(dragged, over).changes(from))));
        if takes {
            ev.prevent_default();
            dom::drop_moves(ev);
        }
        if self.over.get_untracked() != at {
            self.over.set(at);
        }
    }

    /// Let go over the plan: the module into the semester under the pointer, counting for the row
    /// it is let go over where it does.
    fn drop(self, ctx: StudyCtx, ev: &leptos::ev::DragEvent, t: &'static Texts) {
        self.follow(ev);
        let (Some(landing), Some(dragged)) = (self.landing.get_untracked(), self.what.get_untracked()) else { return };
        if !landing.changes(dragged.semester) {
            return;
        }
        ev.prevent_default();
        self.end(ctx);
        let (from, to) = (dragged.semester, landing.semester);
        match (landing.count, dragged.counts) {
            (Some((group, exception)), Some(counts)) => {
                let Some(target) = counts.targets.get(group).cloned() else { return };
                move_count(ctx, from, to, dragged.key, counts, target, exception, t);
            }
            _ => menu::act(ctx, Act::Move { semester: from, keys: vec![dragged.key], to }, t),
        }
    }

    /// What `row` is to the module dragged, while one is: one memo for a row's head, its cells and
    /// its lanes.
    fn fit_of(self, row: RowAt) -> Memo<Option<Fit>> {
        Memo::new(move |_| self.what.with(|what| what.as_ref().map(|dragged| fit(dragged, row))))
    }

    /// Whether a module is being dragged.
    fn dragging(self) -> bool {
        self.what.with(Option::is_some)
    }

    /// Whether Strg (⌘) is held over the plan while a module that may count elsewhere is dragged:
    /// the rows it would count for as an exception say so (app.css `[data-ex]`).
    fn exception_held(self) -> bool {
        self.what.with(|what| what.as_ref().is_some_and(|dragged| dragged.counts.is_some())) && self.over.with(|over| over.is_some_and(|over| over.exception))
    }

    /// Whether the piece of item `key` in semester `s` is the one taken up.
    fn taken(self, s: SemesterKey, key: &str) -> bool {
        self.what.with(|what| what.as_ref().is_some_and(|dragged| dragged.semester == s && dragged.key == key))
    }

    /// Whether a drop at the cell of `row` in semester `s` counts there as an exception.
    fn lands_as_exception(self, s: SemesterKey, row: RowAt) -> bool {
        self.landing.with(|landing| landing.is_some_and(|landing| landing.semester == s && landing.count.is_some_and(|(_, exception)| exception)))
            && self.over.with(|over| over.is_some_and(|over| over.row == Some(row)))
    }

    /// Whether semester `s` is where a drop would move the module: its column lit.
    fn column(self, s: SemesterKey) -> bool {
        self.over.with(|over| over.is_some_and(|over| over.semester == s))
    }

    /// Whether the cell of the row of `area` (and subsection `group`, where `sub`) in semester `s`
    /// is where the module lands.
    fn lands(self, s: SemesterKey, area: Option<usize>, group: Option<usize>, sub: bool) -> bool {
        self.landing.with(|landing| landing.is_some_and(|landing| landing.semester == s && landing.area == area && (!sub || landing.group == group)))
    }

    /// Whether the lane of `keys` is the module's, where it lands in its own row in semester `s`.
    fn lands_in(self, s: SemesterKey, area: Option<usize>, group: Option<usize>, sub: bool, keys: &BTreeSet<String>) -> bool {
        self.lands(s, area, group, sub) && self.landing.with(|landing| landing.is_some_and(|landing| landing.count.is_none())) && self.what.with(|what| what.as_ref().is_some_and(|dragged| keys.contains(&dragged.key)))
    }

    /// What the cell of `row` in semester `s` says while the module is over the plan: under the
    /// pointer, that it counts there („zählt hier"), as an exception („als Ausnahme"), or not (and
    /// what Strg does); where it lands instead of a row that does not take it, „landet hier" (in
    /// its area's row).
    fn hint(self, s: SemesterKey, row: RowAt, t: &'static Texts) -> Option<(&'static str, String)> {
        let st = &t.study;
        let landing = self.landing.get()?;
        let over = self.over.get()?;
        if over.semester != s {
            return None;
        }
        if over.row == Some(row) {
            if let Some((_, exception)) = landing.count {
                return Some(if exception { ("st-rm-hint ex", st.drop_as_exception.to_string()) } else { ("st-rm-hint fit", st.drop_counts_here.to_string()) });
            }
            if landing.refused == Some(row) {
                let counts = self.what.with(|what| what.as_ref().is_some_and(|dragged| dragged.counts.is_some()));
                return Some(("st-rm-hint", if counts { (st.drop_exception)(&folia_design::keys::text_here("Mod")) } else { st.drop_refused.to_string() }));
            }
            return None;
        }
        (landing.refused.is_some() && row == RowAt::Area(landing.area)).then(|| ("st-rm-hint fit", st.drop_lands_here.to_string()))
    }
}

/// What module `id` of semester `s` may count for (`Counts`): none where the plan names it in a row
/// of its own (it counts there, as „Zählt für" has it) or has no subgroups.
fn counts_of(ctx: StudyCtx, s: SemesterKey, id: &str, t: &'static Texts) -> Option<Counts> {
    untrack(|| {
        ctx.with_input(|input, ready| {
            let study = &ready.study;
            let view = choices::plan_view(input, study);
            if view.lines.iter().any(|line| line.module_id.as_deref() == Some(id)) {
                return None;
            }
            let bench = Bench::of(study, &view);
            if bench.groups.is_empty() {
                return None;
            }
            let targets = bench
                .groups
                .iter()
                .map(|group| {
                    let choice = group.choice().and_then(|at| view.choices.get(at));
                    let name = match (group.kind, choice) {
                        (GroupKind::Choice(_), Some(choice)) if choice.fues => t.study.fues.to_string(),
                        (GroupKind::Choice(_), Some(choice)) => groups::short_name(&choice.name),
                        _ => format!("{} · {}", ready.area_name(Some(group.area), t), t.study_bench.pflicht),
                    };
                    Target { area: group.area, kind: group.kind, name, fits: groups::fits(input, study, &view, group, id), listed: choice.is_some_and(|choice| choice.list_of(id).is_some()), complete: group.complete }
                })
                .collect();
            let name = study.semester(s).and_then(|semester| semester.items.iter().find(|item| item.module_id() == Some(id))).map_or_else(|| id.to_string(), |item| item.name.clone());
            Some(Counts { id: id.to_string(), name, targets, now: groups::counted_in(&bench, &view, id, s) })
        })
        .flatten()
    })
}

/// Moves module `key` from semester `from` to `to` (where they differ) and lets it count for
/// `target` there, an exception where `exception` (`bench::count_for`, „Zählt für" by a drop), with
/// one note that „Rückgängig" takes both back by.
#[allow(clippy::too_many_arguments)]
fn move_count(ctx: StudyCtx, from: SemesterKey, to: SemesterKey, key: String, counts: Counts, target: Target, exception: bool, t: &'static Texts) {
    let (s, b) = (&t.study, &t.study_bench);
    let note = match (from == to, exception) {
        (true, false) => (b.counted_note)(&counts.name, &target.name),
        (true, true) => (b.counted_exception_note)(&counts.name, &target.name),
        (false, false) => (s.moved_counted)(&counts.name, &to.label(t.locale), &target.name),
        (false, true) => (s.moved_counted_exception)(&counts.name, &to.label(t.locale), &target.name),
    };
    ctx.selection.set(Selection::default());
    let id = counts.id;
    ctx.change_noted(
        Some(note),
        move |doc, input| {
            let before = study::study(input);
            if from != to {
                if let Some(item) = before.semester(from).and_then(|semester| semester.items.iter().find(|item| item.key() == key)) {
                    study::move_item(doc, from, to, item);
                }
            }
            let view = choices::plan_view(input, &before);
            let bench = Bench::of(&before, &view);
            let group = bench.find(target.area, target.kind).and_then(|at| bench.groups.get(at));
            groups::count_for(doc, &view, to, &id, group, input.program_id);
        },
        || {},
    );
}

#[component]
pub(super) fn Grid(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    // The semesters before the current one stand open: with bars they are narrow and say what is
    // done already.
    let past = Past::new();
    let unfolded = Unfolded::new();
    let today = crate::studyplan::today();
    // The modules' abbreviations in the program: of every module planned, passed or open in the plan.
    let abbrevs = Memo::new(move |before: Option<&BTreeMap<String, String>>| {
        let ask = ctx.with_ready(|ready| {
            let mut ids: Vec<String> = ready.study.semesters.iter().flat_map(|semester| semester.items.iter().filter_map(|item| item.module_id().map(str::to_string))).collect();
            ids.extend(ready.lines.iter().filter_map(|line| match &line.suggestion.pick {
                Pick::Module { id, .. } => Some(id.clone()),
                Pick::Row { .. } => None,
            }));
            ids.sort();
            ids.dedup();
            ModuleAbbrevsAsk { ids, program: Some(ready.program.id.clone()) }
        });
        let Some(ask) = ask.filter(|ask| !ask.ids.is_empty()) else { return BTreeMap::new() };
        let Some(now) = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&ask))) else { return BTreeMap::new() };
        folia_pages::ask::unless_pending(now, before, |now| now.unwrap_or_default())
    });
    // The study before what is made of it (R16), its subsections from the plan's view of it.
    let layout = Memo::new(move |_| {
        let fold = past.fold.get();
        ctx.with_input(|input, ready| {
            let subs = Subs::of(input, ready);
            abbrevs.with(|abbrevs| Layout::of(ready, &subs, fold, abbrevs, today, t))
        })
    });
    let any_past = Memo::new(move |_| ctx.with_ready(|ready| ready.study.semesters.iter().any(|semester| semester.when == When::Past)).unwrap_or(false));
    // Every row that unfolds, and whether all of them are: the one button says what it does.
    let keys = Memo::new(move |_| layout.with(|layout| layout.as_ref().map(Layout::keys).unwrap_or_default()));
    let all_open = Memo::new(move |_| keys.with(|keys| unfolded.open.with(|open| !keys.is_empty() && keys.is_subset(open))));
    // A module dragged over the plan: where it goes, and where it lands.
    let aim = Aim::new();
    // The panel's head and the heads of the semesters stay at the top while the areas scroll under
    // them (app.css), so they scroll sideways in a box of their own: either box follows the other,
    // moved by a wheel, a finger, the keys or the focus. Both boxes stay as the plan changes (an
    // area unfolded), and so does how far they are scrolled.
    let heads = NodeRef::<Div>::new();
    let body = NodeRef::<Div>::new();
    // Where this last put each box (px): the scroll that sets off there is not followed back, which
    // would pull a glide of the other one back by a frame.
    let heads_put = StoredValue::new(None::<f64>);
    let body_put = StoredValue::new(None::<f64>);
    // Whether the plan was scrolled sideways otherwise than by `show_now`: by the student, the
    // keys, the focus.
    let by_hand = StoredValue::new(false);
    // The current semester in view (owner, 2026-11-04: „die scrollposition von der seite so
    // anzupassen, dass immer das aktuelle semester passend gezeigt wird"): the one before it first
    // after the areas' names, then it and those to come (`dom::now_left`). Once the plan is there and
    // as „Bisher" folds or unfolds the semesters before it (`force`); as its width changes only
    // while nobody has scrolled it sideways, which it does not undo.
    let show_now = move |force: bool| {
        if !force && by_hand.get_value() {
            return;
        }
        let Some(x) = heads.get_untracked().and_then(|heads| dom::now_left(&heads)) else { return };
        by_hand.set_value(false);
        for (node, put) in [(heads, heads_put), (body, body_put)] {
            if let Some(element) = node.get_untracked() {
                put_left(&element, put, x);
            }
        }
    };
    let placed = StoredValue::new(None::<bool>);
    Effect::new(move |_| {
        let fold = past.fold.get();
        if layout.with(Option::is_some) && placed.get_value() != Some(fold) {
            placed.set_value(Some(fold));
            request_animation_frame(move || show_now(true));
        }
    });
    // Whether the pieces fit beside each other, and how tall the head is that stays at the top:
    // measured after every change and whenever the grid's width changes (the window, the sidebar,
    // the full screen).
    let measure = || {
        dom::fit_pieces(PANEL_ID);
        dom::measure_top(PANEL_ID);
    };
    Effect::new(move |_| {
        layout.track();
        unfolded.open.track();
        ctx.full.track();
        request_animation_frame(measure);
    });
    // Only a change of its width: its height changes with every frame of a row unfolding or
    // folding, and measuring all of the plan then made those frames stutter (until Folia 1.89.2).
    let width = StoredValue::new(None::<f64>);
    Effect::new(move |_| {
        let watch = nav::watch_size(PANEL_ID, move || {
            let now = dom::width_of(PANEL_ID);
            if width.get_value().is_some_and(|before| (before - now).abs() < 0.5) {
                return;
            }
            width.set_value(Some(now));
            measure();
            show_now(false);
        });
        on_cleanup(move || drop(watch));
    });
    // „Als Bild speichern": the plan as it is shown — what is unfolded, „Bisher" — as a PNG.
    let save = move |_| {
        let Some(layout) = layout.get_untracked() else { return };
        let open = unfolded.open.get_untracked();
        let program = ctx.with_ready(|ready| format!("{} {}", ready.program.name, ready.program.degree())).unwrap_or_default();
        let day = today.map(|day| day.date(t.locale)).unwrap_or_default();
        let heading = picture::Heading {
            title: s.grid_title.to_string(),
            subtitle: (s.picture_subtitle)(&program, &day),
            site: crate::studyplan::site_host(),
            legend: picture::legend(t),
        };
        let name = (s.picture_name)(&today.map(|day| day.iso()).unwrap_or_default());
        picture::save(&layout, &open, &heading, &name, t);
    };
    let is_full = Memo::new(move |_| ctx.full.get());
    view! {
        <section
            class="panel st-grid-panel st-rm"
            class:is-full=move || is_full.get()
            id=PANEL_ID
            aria-label=s.grid_title
            data-moved=move || past.moved.get()
            data-drag=move || aim.dragging().then_some("")
            data-ex=move || aim.exception_held().then_some("")
            on:dragenter=move |ev: leptos::ev::DragEvent| aim.follow(&ev)
            on:dragover=move |ev: leptos::ev::DragEvent| aim.follow(&ev)
            on:dragleave=move |ev: leptos::ev::DragEvent| if dom::drag_left(&ev) { aim.over.set(None) }
            on:drop=move |ev: leptos::ev::DragEvent| aim.drop(ctx, &ev, t)
        >
            <div class="st-rm-top">
                <div class="st-sems-head">
                    <h2>{s.grid_title}</h2>
                    <span class="st-grow"></span>
                    // While a module is dragged, what the rows' colours say, over the buttons.
                    {move || aim.dragging().then(|| {
                        let elsewhere = aim.what.with(|what| what.as_ref().is_some_and(|dragged| dragged.counts.is_some()));
                        view! {
                            <div class="st-rm-dragkey" aria-hidden="true">
                                <span><i class="k fit"></i>{s.drag_key_fits}</span>
                                <span><i class="k no"></i>{s.drag_key_no}</span>
                                {elsewhere.then(|| view! { <span><i class="k ex"></i>{(s.drag_key_exception)(&folia_design::keys::text_here("Mod"))}</span> })}
                            </div>
                        }
                    })}
                    <button class="st-link st-rm-all" type="button" on:click=move |_| {
                        if all_open.get_untracked() { unfolded.keep(BTreeSet::new()) } else { unfolded.keep(keys.get_untracked()) }
                    }>
                        {move || if all_open.get() {
                            view! { <Icon name="chevrons-up"/>{s.collapse_all} }.into_any()
                        } else {
                            view! { <Icon name="chevrons-down"/>{s.expand_all} }.into_any()
                        }}
                    </button>
                    {move || any_past.get().then(|| view! {
                        <button class="st-link" type="button" aria-expanded=move || if past.fold.get() { "false" } else { "true" } on:click=move |_| past.set(!past.fold.get_untracked())>
                            {move || if past.fold.get() { s.expand_past } else { s.collapse_past }}
                        </button>
                    })}
                    <button class="st-link st-rm-save" type="button" on:click=save><Icon name="image"/>{s.save_picture}</button>
                    <button
                        class="icon-btn st-rm-full"
                        type="button"
                        aria-pressed=move || if is_full.get() { "true" } else { "false" }
                        aria-label=move || if is_full.get() { s.full_screen_leave } else { s.full_screen }
                        title=move || if is_full.get() { format!("{} · Esc", s.full_screen_leave) } else { s.full_screen.to_string() }
                        on:click=move |_| full(ctx, !ctx.full.get_untracked())
                    >
                        {move || if is_full.get() { view! { <Icon name="minimize"/> }.into_any() } else { view! { <Icon name="maximize"/> }.into_any() }}
                    </button>
                </div>
                <div class="st-grid-scroll st-rm-top-scroll" node_ref=heads on:scroll=move |_| follow(heads, body, heads_put, body_put, by_hand)>
                    {move || layout.get().map(|layout| {
                        let today_at = layout.today.map(|(at, _)| at);
                        view! {
                            <div class="st-grid st-rm-heads" style=layout.style()>
                                <div class="st-rm-corner"></div>
                                {layout.bands.iter().map(|band| view! { <div class="st-rm-band" style=format!("grid-column: span {}", band.span)>{band.label.clone()}</div> }).collect_view()}
                                <div class="st-grid-corner"></div>
                                {layout.columns.iter().cloned().enumerate().map(|(at, column)| head_view(ctx, column, today_at == Some(at), aim, t)).collect_view()}
                            </div>
                        }
                    })}
                </div>
            </div>
            <div class="st-grid-scroll" node_ref=body on:scroll=move |_| follow(body, heads, body_put, heads_put, by_hand)>
                {move || layout.get().map(|layout| {
                    let style = layout.style();
                    let today_at = layout.today.map(|(at, _)| at);
                    let columns: Vec<(&'static str, Option<SemesterKey>)> = layout.columns.iter().map(|column| (column_class(column), column.semester())).collect();
                    let frame = RowFrame { ctx, today_at, columns, unfolded, past, aim, t };
                    // Each row follows what is unfolded by itself: one unfolded or folded, the
                    // others stay as they are, and its head keeps the focus.
                    view! {
                        <div class="st-grid st-rm-grid" style=style>
                            {layout.rows.into_iter().map(|row| row_view(&frame, row)).collect_view()}
                            {today_at.map(|at| view! {
                                <div class="st-rm-foot rm-now" style=format!("grid-column: {}", at + 2)>
                                    <span class="st-rm-flag">{layout.today_flag.clone()}</span>
                                </div>
                            })}
                        </div>
                    }
                })}
            </div>
            <p class="st-grid-legend st-rm-legend">
                <span><i class="st-rm-sw passed"></i>{s.legend_passed}</span>
                <span><i class="st-rm-sw running"></i>{s.legend_running}</span>
                <span><i class="st-rm-sw planned"></i>{s.legend_planned}</span>
                <span><i class="st-rm-sw open"></i>{s.legend_plan_open}</span>
                <span><Icon name="x" class="no"/>{s.legend_failed}</span>
                <span><Icon name="repeat"/>{s.legend_retake}</span>
                <span class="warn"><Icon name="triangle-alert"/>{s.legend_offer}</span>
                <span><i class="st-rm-sw over"></i>{s.legend_over}</span>
                <span><i class="st-rm-sw today"></i>{s.legend_today}</span>
                <span class="st-grow"></span>
                <span class="st-rm-help">{(s.grid_help)(&folia_design::keys::text_here("Mod"))}</span>
            </p>
        </section>
    }
}

/// Scrolls the box `to` sideways as far as `from` is, which has just scrolled: the heads with the
/// areas, or the areas with the heads. Not where `from` is where this put it (`from_put`): that
/// scroll followed the other box already. Any other scroll is the student's (`by_hand`).
fn follow(from: NodeRef<Div>, to: NodeRef<Div>, from_put: StoredValue<Option<f64>>, to_put: StoredValue<Option<f64>>, by_hand: StoredValue<bool>) {
    let (Some(from), Some(to)) = (from.get_untracked(), to.get_untracked()) else { return };
    let x = dom::left_of(&from);
    if from_put.try_update_value(Option::take).flatten().is_some_and(|put| (put - x).abs() < 0.5) {
        return;
    }
    by_hand.set_value(true);
    put_left(&to, to_put, x);
}

/// Scrolls a box of the Gesamtplan sideways to `x` and keeps where that put it (`put`), so that
/// the scroll it sets off is not followed back (`follow`); nothing where it is there already, which
/// sets off no scroll.
fn put_left(element: &leptos::web_sys::Element, put: StoredValue<Option<f64>>, x: f64) {
    let before = dom::left_of(element);
    if (before - x).abs() >= 0.5 {
        dom::scroll_row_to(element, x);
        let now = dom::left_of(element);
        put.set_value(((now - before).abs() >= 0.5).then_some(now));
    }
}

/// The class of a column's cells: the semesters that are over grey, the current one green, one of
/// leave hatched.
fn column_class(column: &Column) -> &'static str {
    match column {
        Column::Past { .. } => "past",
        Column::Semester(head) if head.leave => "leave",
        Column::Semester(head) if head.now => "is-now",
        Column::Semester(head) if head.past => "past",
        Column::Semester(_) => "",
    }
}

/// An area's tone as the style of what is drawn in it: `--c`, and on the sun's full surface near
/// black text (`--c-on`; on the other tones the panel's colour, white by day).
fn tone_style(tone: &str) -> String {
    if tone == "var(--t-sun)" {
        format!("--c: {tone}; --c-on: var(--rm-on-sun)")
    } else {
        format!("--c: {tone}")
    }
}

/// A semester's head: its name, Fachsemester, figure and load; a click shows it as the semester.
fn head_view(ctx: StudyCtx, column: Column, now_line: bool, aim: Aim, t: &'static Texts) -> AnyView {
    let s = &t.study;
    match column {
        Column::Past { span, credits } => view! {
            <div class="st-col-head past"><div class="st-col-name">{s.so_far}</div><div class="st-sub"><span>{span}</span><b class="ok">"✓ "{credits}</b></div></div>
        }
        .into_any(),
        Column::Semester(head) => {
            let key = head.key;
            view! {
                <button type="button" class="st-col-head" class:is-now=head.now class:past=head.past class:rm-now=now_line class:rm-drop=move || aim.column(key) data-sem=key.key() on:click=move |_| {
                    ctx.focus.set(Some(key));
                    ctx.view.set(super::View::Semester);
                }>
                    <div class="st-col-name">{head.label}{head.now.then(|| view! { <span class="st-now">{s.now}</span> })}</div>
                    <div class="st-sub"><span>{head.fs}</span><b class:heavy=head.heavy>{head.figure}</b></div>
                    <span class="st-load"><span style=format!("flex-grow: {}", head.load.0)></span><span style=format!("flex-grow: {}", (head.load.1 - head.load.0).max(0.0))></span></span>
                </button>
            }
            .into_any()
        }
    }
}

/// What every row of the grid is drawn with.
#[derive(Clone)]
struct RowFrame {
    ctx: StudyCtx,
    today_at: Option<usize>,
    columns: Vec<(&'static str, Option<SemesterKey>)>,
    unfolded: Unfolded,
    past: Past,
    aim: Aim,
    t: &'static Texts,
}

/// Which row a cell, a lane or a piece is of, for what a drop there does and where it lands: its
/// area, its subsection (`sub`: the row is one; `group` none for „Weitere") and what that is, and
/// its tone.
#[derive(Clone, Copy)]
struct Of {
    area: Option<usize>,
    group: Option<usize>,
    kind: Option<GroupKind>,
    sub: bool,
    tone: &'static str,
}

impl Of {
    fn row_at(self) -> RowAt {
        match (self.sub, self.group) {
            (true, Some(group)) => RowAt::Group(group),
            _ => RowAt::Area(self.area),
        }
    }
}

/// An area: its head and its bar over the semesters, then its subsections or its lanes where it is
/// unfolded, and while it folds.
fn row_view(frame: &RowFrame, row: AreaRow) -> AnyView {
    let RowFrame { unfolded, t, .. } = frame.clone();
    let s = &t.study;
    let (passed, planned, rest, over) = row.bar;
    let name = row.name.clone();
    let (open, folding) = unfolded.state(name.clone());
    let of = Of { area: row.area, group: None, kind: None, sub: false, tone: row.tone };
    let aim = frame.aim;
    let fit = aim.fit_of(of.row_at());
    // A row of one lane does not unfold (`AreaRow::unfolds`).
    let unfolds = row.unfolds();
    let toggle = {
        let name = name.clone();
        move |_| unfolded.toggle(&name)
    };
    let unfold = {
        let name = name.clone();
        move || {
            if unfolds {
                unfolded.unfold(&name);
            }
        }
    };
    let figure = if row.done {
        view! { <span class="st-card-lp done"><Icon name="check"/>{row.figure.clone()}</span> }.into_any()
    } else {
        view! { <span class="st-card-lp" class:over=row.over>{row.figure.clone()}</span> }.into_any()
    };
    let label = {
        let name = name.clone();
        move || if open.get() { (s.collapse_area)(&name) } else { (s.expand_area)(&name) }
    };
    let cells = row.cells.clone();
    // What it unfolds into: there at once as the plan is shown (a reload, a change of the plan),
    // growing in as the area unfolds, shrinking away as it folds (app.css `.rm-in`, `.rm-out`).
    let (lanes, subs) = (row.lanes, row.subs);
    let last_lane = lanes.len();
    let shown = Memo::new(move |_| open.get() || folding.get());
    let first = StoredValue::new(true);
    let inner_frame = frame.clone();
    let inner = move || {
        let came = !first.get_value();
        first.set_value(false);
        (unfolds && shown.get()).then(|| {
            if subs.is_empty() {
                lanes.iter().cloned().enumerate().map(|(i, lane)| lane_view(&inner_frame, lane, of, fit, i + 1 == last_lane, came, folding.into(), 0)).collect_view().into_any()
            } else {
                subs.iter().cloned().map(|sub| sub_view(&inner_frame, sub, of, came, folding)).collect_view().into_any()
            }
        })
    };
    let text = view! {
        <span class="st-rm-area-t">
            <span class="st-card-head"><span class="st-dot"></span><span class="st-card-name">{row.name.clone()}</span>{figure}</span>
            <span class="st-mini">
                <span class="passed" style=format!("flex-grow: {passed}")></span>
                <span class="planned" style=format!("flex-grow: {planned}")></span>
                <span class="open" style=format!("flex-grow: {rest}")></span>
                <span class="over" style=format!("flex-grow: {over}")></span>
            </span>
        </span>
    };
    let head = if unfolds {
        view! {
            <button type="button" class="st-rm-area" aria-expanded=move || if open.get() { "true" } else { "false" } aria-label=label on:click=toggle>
                <span class="st-rm-tog" aria-hidden="true"><Icon name="chevron-right"/></span>
                {text}
            </button>
        }
        .into_any()
    } else {
        view! { <div class="st-rm-area still"><span class="st-rm-tog" aria-hidden="true"></span>{text}</div> }.into_any()
    };
    view! {
        <div class="st-row-head st-rm-head" class:is-open=move || unfolds && open.get() class:rm-fit=move || fit.get() == Some(Fit::Fits) class:rm-ex=move || fit.get() == Some(Fit::Exception) class:rm-no=move || fit.get() == Some(Fit::No) style=tone_style(row.tone)>
            {head}
        </div>
        {cells.into_iter().enumerate().map(|(at, cell)| cell_view(frame, cell, of, fit, &name, frame.today_at == Some(at), unfold.clone(), !unfolds, None)).collect_view()}
        {inner}
    }
    .into_any()
}

/// A subsection of an unfolded area: its head (its name, what it holds against what it asks and
/// the Wahlpflicht's bar of it) and its bar over the semesters, then its lanes where it is
/// unfolded. It grows in with its area (`came`) and shrinks away while its area folds.
fn sub_view(frame: &RowFrame, sub: SubRow, area: Of, came: bool, area_folding: Memo<bool>) -> AnyView {
    let RowFrame { unfolded, t, .. } = frame.clone();
    let s = &t.study;
    let key = sub.key.clone();
    let (open, folding) = unfolded.state(key.clone());
    let of = Of { area: area.area, group: sub.group, kind: sub.kind, sub: true, tone: area.tone };
    let fit = frame.aim.fit_of(of.row_at());
    // A place for one module does not unfold (`SubRow::unfolds`).
    let unfolds = sub.unfolds();
    let toggle = {
        let key = key.clone();
        move |_| unfolded.toggle(&key)
    };
    let unfold = {
        let key = key.clone();
        move || {
            if unfolds {
                unfolded.unfold(&key);
            }
        }
    };
    let label = {
        let name = sub.name.clone();
        move || if open.get() { (s.collapse_area)(&name) } else { (s.expand_area)(&name) }
    };
    // Its lanes shrink away as it folds or as its area does.
    let out = Memo::new(move |_| folding.get() || area_folding.get());
    let shown = Memo::new(move |_| open.get() || folding.get());
    let lanes = sub.lanes;
    let last_lane = lanes.len();
    let first = StoredValue::new(true);
    let inner_frame = frame.clone();
    let inner = move || {
        // Unfolded with its area, its lanes come with it.
        let lanes_came = if first.get_value() { came } else { true };
        first.set_value(false);
        (unfolds && shown.get()).then(|| lanes.iter().cloned().enumerate().map(|(i, lane)| lane_view(&inner_frame, lane, of, fit, i + 1 == last_lane, lanes_came, out.into(), 1)).collect_view())
    };
    let figure = if sub.done {
        view! { <span class="st-card-lp done"><Icon name="check"/>{sub.figure.clone()}</span> }.into_any()
    } else {
        view! { <span class="st-card-lp">{sub.figure.clone()}</span> }.into_any()
    };
    let stand = sub.stand.map(|(stand, room)| stand_bar(&stand, room, "st-rm-wbar", t));
    let text = view! {
        <span class="st-rm-area-t">
            <span class="st-card-head"><span class="st-card-name">{sub.name.clone()}</span></span>
            <span class="st-rm-subline">{stand}{figure}</span>
        </span>
    };
    let head = if unfolds {
        view! {
            <button type="button" class="st-rm-area" aria-expanded=move || if open.get() { "true" } else { "false" } aria-label=label title=sub.title.clone() on:click=toggle>
                <span class="st-rm-tog" aria-hidden="true"><Icon name="chevron-right"/></span>
                {text}
            </button>
        }
        .into_any()
    } else {
        view! { <div class="st-rm-area still" title=sub.title.clone()><span class="st-rm-tog" aria-hidden="true"></span>{text}</div> }.into_any()
    };
    view! {
        <div
            class="st-row-head st-rm-head st-rm-sub"
            class:is-open=move || unfolds && open.get()
            class:rm-in=came
            class:rm-out=move || area_folding.get()
            class:rm-fit=move || fit.get() == Some(Fit::Fits)
            class:rm-ex=move || fit.get() == Some(Fit::Exception)
            class:rm-no=move || fit.get() == Some(Fit::No)
            style=tone_style(area.tone)
        >
            {head}
        </div>
        {sub.cells.into_iter().enumerate().map(|(at, cell)| cell_view(frame, cell, of, fit, &sub.name, frame.today_at == Some(at), unfold.clone(), !unfolds, Some((came, area_folding)))).collect_view()}
        {inner}
    }
    .into_any()
}

/// A cell of a row's bar: its pieces, „n Module" where they do not fit (`dom::fit_pieces`), and
/// „+" for the workbench. A subsection's cell (`sub`: whether it came, and its area folding) grows
/// in and shrinks away with it. In a row that does not unfold (`still`) no „n Module" stands for
/// its pieces, whose lanes would have them one by one: they narrow, their labels cut, each still
/// its own button (app.css `.st-rm-cell.still`).
#[allow(clippy::too_many_arguments)]
fn cell_view(frame: &RowFrame, cell: Cell, of: Of, fit: Memo<Option<Fit>>, row_name: &str, now_line: bool, unfold: impl Fn() + Clone + 'static, still: bool, sub: Option<(bool, Memo<bool>)>) -> AnyView {
    let RowFrame { ctx, past, aim, t, .. } = frame.clone();
    let s = &t.study;
    let tone = of.tone;
    let (came, folding) = match sub {
        Some((came, folding)) => (came, Some(folding)),
        None => (false, None),
    };
    let out = move || folding.is_some_and(|folding| folding.get());
    match cell {
        Cell::Past { passed, any_passed, failed, title } => view! {
            <div class="st-cell st-rm-cell past" class:sub=of.sub class:rm-in=came class:rm-out=out class:rm-fit=move || fit.get() == Some(Fit::Fits) class:rm-ex=move || fit.get() == Some(Fit::Exception) class:rm-no=move || fit.get() == Some(Fit::No) style=tone_style(tone)>
                {(any_passed || failed > 0).then(|| view! {
                    <button type="button" class="st-rm-p stub" class:passed=any_passed class:failed=!any_passed title=title.clone() aria-label=title on:click=move |_| past.set(false)>
                        {any_passed.then(|| view! { <Icon name="check"/><span class="lb">{n(passed, t)}</span> })}
                        {(failed > 0).then(|| view! { <span class="st-rm-badge">"✕ "{failed}</span> })}
                    </button>
                })}
            </div>
        }
        .into_any(),
        Cell::Semester(cell) => {
            let semester = cell.key;
            let real: Vec<&Piece> = cell.pieces.iter().filter(|piece| piece.fill != Fill::Open).collect();
            let open: Vec<&Piece> = cell.pieces.iter().filter(|piece| piece.fill == Fill::Open).collect();
            let empty = cell.pieces.is_empty();
            // What stands for them all where they do not fit: „4 Module · 24 LP", what is open
            // beside it.
            let sum_fill = if !real.is_empty() && real.iter().all(|piece| piece.fill == Fill::Passed) {
                Fill::Passed
            } else if !real.is_empty() && real.iter().all(|piece| piece.fill == Fill::Row) {
                // Rows of the plan alone: dashed, as they are.
                Fill::Row
            } else if cell.when == When::Now {
                Fill::Running
            } else if cell.when == When::Past {
                Fill::Passed
            } else {
                Fill::Planned
            };
            let failed = real.iter().filter(|piece| piece.fill == Fill::Failed).count();
            let credits: f64 = real.iter().map(|piece| piece.grow).sum();
            // The modules by their number, a row of the plan without one as what it is: „3 Module",
            // „2 Platzhalter", „1 Modul · 1 Platzhalter".
            let places = real.iter().filter(|piece| piece.fill == Fill::Row).count();
            let modules = real.len() - places;
            let count = match (modules, places) {
                (_, 0) => folia_design::format::modules(i64::try_from(modules).unwrap_or(0), t.locale),
                (0, _) => (s.n_placeholders)(places),
                _ => format!("{} · {}", folia_design::format::modules(i64::try_from(modules).unwrap_or(0), t.locale), (s.n_placeholders)(places)),
            };
            let sum_title = real.iter().map(|piece| format!("{} · {}", piece.label, piece.title)).collect::<Vec<_>>().join("\n");
            let open_title = open.iter().map(|piece| piece.title.clone()).collect::<Vec<_>>().join("\n");
            let open_count = open.len();
            // One open piece says what it is („+10", „BS1"), several how many they are.
            let open_label = match open.as_slice() {
                [one] => one.label.clone(),
                _ => (s.n_open)(open_count),
            };
            let sum_credits = (s.credits)(&n(credits, t));
            let pieces = cell.pieces.clone();
            let for_cell = (s.add_in_cell_for)(row_name, &semester.label(t.locale));
            let unfold_sum = unfold.clone();
            let unfold_open = unfold.clone();
            let row_at = of.row_at();
            let hint = move || aim.hint(semester, row_at, t).map(|(class, text)| view! { <span class=class aria-hidden="true">{text}</span> });
            // A subsection's „+" opens the workbench at its subgroup, an area's at the one due.
            let pick = match (of.area, of.kind) {
                (Some(area), Some(kind)) => BenchPick::Group(area, kind),
                (Some(area), None) => BenchPick::Area(area),
                (None, _) => BenchPick::Plan,
            };
            view! {
                <div
                    class="st-cell st-rm-cell"
                    class:sub=of.sub
                    class:still=still
                    class:rm-in=came
                    class:rm-out=out
                    class:is-now=cell.when == When::Now
                    class:past=cell.when == When::Past
                    class:leave=cell.leave
                    class:rm-now=now_line
                    class:rm-drop=move || aim.column(semester)
                    class:rm-drop-here=move || aim.lands(semester, of.area, of.group, of.sub)
                    class:rm-drop-no=move || aim.landing.with(|landing| landing.is_some_and(|landing| landing.semester == semester && landing.refused == Some(row_at)))
                    class:rm-land-ex=move || aim.lands_as_exception(semester, row_at)
                    class:rm-fit=move || fit.get() == Some(Fit::Fits)
                    class:rm-ex=move || fit.get() == Some(Fit::Exception)
                    class:rm-no=move || fit.get() == Some(Fit::No)
                    style=tone_style(tone)
                    data-sem=semester.key()
                    data-row=row_at.attr()
                >
                    {(!empty).then(|| view! {
                        <div class="st-rm-seg">
                            {pieces.into_iter().map(|piece| piece_view(frame, piece, semester, of, unfold.clone())).collect_view()}
                        </div>
                        {(!still).then(|| view! {
                            <div class="st-rm-sum">
                                {(!real.is_empty()).then(|| view! {
                                    <button type="button" class=format!("st-rm-p sum {}{}", sum_fill.class(), if cell.when == When::Now { " now" } else { "" }) title=sum_title.clone() aria-label=format!("{count} · {sum_credits}") on:click=move |_| unfold_sum()>
                                        <span class="lb">{count.clone()}<span class="st-rm-lp">" · "{sum_credits.clone()}</span></span>
                                        {(failed > 0).then(|| view! { <span class="st-rm-badge">"✕ "{failed}</span> })}
                                    </button>
                                })}
                                {(open_count > 0).then(|| view! {
                                    <button type="button" class="st-rm-p open sum-open" title=open_title.clone() aria-label=open_title on:click=move |_| unfold_open()>
                                        {(open_count > 1).then(|| view! { <Icon name="plus"/> })}<span class="lb">{open_label.clone()}</span>
                                    </button>
                                })}
                            </div>
                        })}
                    })}
                    <button type="button" class="st-cell-add" class:st-rm-add=!empty title=for_cell.clone() aria-label=for_cell on:click=move |_| bench::enter(ctx, semester, pick)>
                        <Icon name="plus"/>{empty.then_some(s.add_in_cell)}
                    </button>
                    {hint}
                </div>
            }
            .into_any()
        }
    }
}

/// A piece of a bar: its menu, its drag; what the plan has open plans itself, or unfolds the row
/// where it stands for several rows.
fn piece_view(frame: &RowFrame, piece: Piece, semester: SemesterKey, of: Of, unfold: impl Fn() + Clone + 'static) -> AnyView {
    let RowFrame { ctx, aim, t, .. } = frame.clone();
    let class = format!("st-rm-p {}{}{}", piece.fill.class(), if piece.now { " now" } else { "" }, if piece.over { " over" } else { "" });
    let style = format!("flex-grow: {}", piece.grow);
    let inner = piece_inner(&piece, None);
    if piece.fill == Fill::Open {
        let picks = piece.picks.clone();
        return view! {
            <button type="button" class=class style=style title=piece.title.clone() aria-label=format!("{} – {}", piece.label, piece.title) on:click=move |_| {
                if picks.is_empty() { unfold() } else { add_pick(ctx, semester, picks.clone(), t) }
            }>{inner}</button>
        }
        .into_any();
    }
    let key = StoredValue::new(piece.key.clone());
    view! {
        <button
            type="button"
            class=class
            style=style
            title=piece.title.clone()
            aria-label=format!("{}, {}", piece.label, piece.title)
            aria-haspopup="menu"
            draggable="true"
            class:is-dragged=move || key.with_value(|key| aim.taken(semester, key))
            on:dragstart=move |ev: leptos::ev::DragEvent| aim.start(ctx, &ev, semester, key.get_value(), of.area, of.group, t)
            on:dragend=move |_| aim.end(ctx)
            on:click=move |ev: leptos::ev::MouseEvent| menu::open_at_button(ctx, MenuFor::Item { semester, key: key.get_value(), view: true }, &ev)
            on:contextmenu=move |ev: leptos::ev::MouseEvent| {
                ev.prevent_default();
                menu::open_at_pointer(ctx, MenuFor::Item { semester, key: key.get_value(), view: true }, &ev);
            }
        >{inner}</button>
    }
    .into_any()
}

/// What a piece shows: ✕, + or ? before its label, ↻ and ⚠ after it; in a lane its word between,
/// a row of the plan's only its word (the lane's name says which row: „? Platzhalter").
fn piece_inner(piece: &Piece, word: Option<&'static str>) -> AnyView {
    let pre = match piece.fill {
        Fill::Failed => Some("x"),
        Fill::Open => Some(if piece.retake { "repeat" } else { "plus" }),
        Fill::Row => Some("circle-help"),
        _ => None,
    };
    let retake = piece.retake && piece.fill != Fill::Open;
    let check = word.is_some() && piece.fill == Fill::Passed;
    let (label, word) = match word {
        Some(word) if piece.row.is_some() => (word.to_string(), None),
        _ => (piece.label.clone(), word),
    };
    view! {
        {pre.map(|name| view! { <Icon name=name/> })}
        <span class="lb">{label}{word.map(|word| view! { <span class="st-rm-w">{word}</span> })}</span>
        {retake.then(|| view! { <Icon name="repeat"/> })}
        {piece.unoffered.then(|| view! { <Icon name="triangle-alert" class="warn"/> })}
        {check.then(|| view! { <Icon name="check"/> })}
    }
    .into_any()
}

/// A lane: the module's name and credits, its piece in each semester it is in, a retake's arrow.
/// Each of its cells grows in where it `came` with its row unfolding, and shrinks away while the
/// row is `folding`: all together, so that the row of the grid does. `depth` 1 for a subsection's.
#[allow(clippy::too_many_arguments)]
fn lane_view(frame: &RowFrame, lane: Lane, of: Of, fit: Memo<Option<Fit>>, last: bool, came: bool, folding: Signal<bool>, depth: u8) -> AnyView {
    let RowFrame { aim, today_at, columns, .. } = frame.clone();
    let (name, title) = (lane.name.clone(), lane.title.clone());
    let keys = StoredValue::new(lane.keys.clone());
    let row_at = of.row_at();
    view! {
        <div class="st-rm-lane" class:deep={depth > 0} class:last=last class:rm-in=came class:rm-out=move || folding.get() class:rm-fit=move || fit.get() == Some(Fit::Fits) class:rm-ex=move || fit.get() == Some(Fit::Exception) class:rm-no=move || fit.get() == Some(Fit::No) style=tone_style(of.tone)>
            <span class="st-rm-lane-name" title=title>{name}</span>
            <span class="st-rm-lane-lp">{lane.credits.clone()}</span>
        </div>
        {lane.bars.into_iter().enumerate().map(|(at, bar)| {
            let through = lane.through.contains(&at);
            let into = lane.into == Some(at);
            let (kind, semester) = columns.get(at).copied().unwrap_or_default();
            view! {
                <div
                    class=format!("st-rm-lc {kind}")
                    class:last=last
                    class:rm-in=came
                    class:rm-out=move || folding.get()
                    class:rm-now=today_at == Some(at)
                    class:link=through
                    class:link-in=into
                    class:rm-drop=move || semester.is_some_and(|s| aim.column(s))
                    class:rm-drop-here=move || semester.is_some_and(|s| keys.with_value(|keys| aim.lands_in(s, of.area, of.group, of.sub, keys)))
                    class:rm-fit=move || fit.get() == Some(Fit::Fits)
                    class:rm-ex=move || fit.get() == Some(Fit::Exception)
                    class:rm-no=move || fit.get() == Some(Fit::No)
                    style=tone_style(of.tone)
                    data-sem=semester.map(SemesterKey::key)
                    data-row=row_at.attr()
                >
                    {bar.into_iter().map(|(piece, word)| lane_bar(frame, piece, word, of)).collect_view()}
                </div>
            }
        }).collect_view()}
    }
    .into_any()
}

/// A module's piece in its lane: as in the bar, with what it is („bestanden").
fn lane_bar(frame: &RowFrame, piece: Piece, word: &'static str, of: Of) -> AnyView {
    let RowFrame { ctx, aim, t, .. } = frame.clone();
    // A lane's piece is the bar's: the same menu and drag.
    let semester = piece.semester;
    let class = format!("st-rm-bar st-rm-p {}{}{}", piece.fill.class(), if piece.now { " now" } else { "" }, if piece.over { " over" } else { "" });
    let inner = piece_inner(&piece, Some(word));
    if piece.fill == Fill::Open {
        let picks = piece.picks.clone();
        return view! {
            <button type="button" class=class title=piece.title.clone() aria-label=format!("{} – {}", piece.label, piece.title) on:click=move |_| add_pick(ctx, semester, picks.clone(), t)>{inner}</button>
        }
        .into_any();
    }
    let key = StoredValue::new(piece.key.clone());
    view! {
        <button
            type="button"
            class=class
            title=piece.title.clone()
            aria-label=format!("{}, {}", piece.label, piece.title)
            aria-haspopup="menu"
            draggable="true"
            class:is-dragged=move || key.with_value(|key| aim.taken(semester, key))
            on:dragstart=move |ev: leptos::ev::DragEvent| aim.start(ctx, &ev, semester, key.get_value(), of.area, of.group, t)
            on:dragend=move |_| aim.end(ctx)
            on:click=move |ev: leptos::ev::MouseEvent| menu::open_at_button(ctx, MenuFor::Item { semester, key: key.get_value(), view: true }, &ev)
            on:contextmenu=move |ev: leptos::ev::MouseEvent| {
                ev.prevent_default();
                menu::open_at_pointer(ctx, MenuFor::Item { semester, key: key.get_value(), view: true }, &ev);
            }
        >{inner}</button>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn target(area: usize, fits: bool, listed: bool) -> Target {
        Target { area, kind: GroupKind::Pflicht, name: String::new(), fits, listed, complete: false }
    }

    /// A module of the Komplex Praktische Informatik (subgroup 3 of the Fachstudium, area 2), let go
    /// over rows of the plan: over another Komplex that lists it it counts there; over the
    /// Nebenfach's row, where the plan does not put it, it stays — but for Strg, an exception in
    /// the area's first subgroup; over its own row and over a semester's head it only moves.
    #[test]
    fn a_drop_counts_where_the_plan_puts_it() {
        let targets = vec![target(1, false, false), target(1, false, false), target(2, true, true), target(2, true, true), target(2, false, false)];
        let dragged = Dragged { semester: key("2027S"), key: "m:12330".to_string(), area: Some(2), group: Some(3), counts: Some(Counts { id: "12330".to_string(), name: "Datenbanken".to_string(), targets, now: Some(3) }) };
        let over = |row: Option<RowAt>, exception: bool| Over { semester: key("2027W"), row, exception };
        let other = land(&dragged, over(Some(RowAt::Group(2)), false));
        assert_eq!((other.count, other.area, other.group), (Some((2, false)), Some(2), Some(2)));
        let refused = land(&dragged, over(Some(RowAt::Group(4)), false));
        assert_eq!((refused.count, refused.refused, refused.group), (None, Some(RowAt::Group(4)), Some(3)));
        assert_eq!(land(&dragged, over(Some(RowAt::Group(4)), true)).count, Some((4, true)));
        let area = land(&dragged, over(Some(RowAt::Area(Some(1))), false));
        assert_eq!((area.count, area.refused), (None, Some(RowAt::Area(Some(1)))));
        assert_eq!(land(&dragged, over(Some(RowAt::Area(Some(1))), true)).count, Some((0, true)));
        let own = land(&dragged, over(Some(RowAt::Group(3)), false));
        assert_eq!((own.count, own.refused, own.area), (None, None, Some(2)));
        assert!(own.changes(key("2027S")), "into another semester");
        assert!(!land(&dragged, Over { semester: key("2027S"), row: Some(RowAt::Group(3)), exception: false }).changes(key("2027S")), "nothing to do");
        let head = land(&dragged, over(None, false));
        assert_eq!((head.count, head.refused), (None, None));
        // A module the plan names in a row of its own counts there: nowhere else.
        let named = Dragged { counts: None, ..dragged };
        assert_eq!(land(&named, over(Some(RowAt::Group(2)), true)).refused, Some(RowAt::Group(2)));
    }

    #[test]
    fn a_row_is_named_in_its_cells() {
        for row in [RowAt::Area(Some(3)), RowAt::Area(None), RowAt::Group(12)] {
            assert_eq!(RowAt::parse(&row.attr()), Some(row));
        }
        assert_eq!(RowAt::parse("x"), None);
    }
}
