//! The menus of „Mein Studium" (owner, 2026-10-04: „Die drei Punkte sind in der Allgemeinheit
//! verstanden als Kontextmenü"): a row's, at its status (owner, 2026-10-19: „Die merken flag soll
//! getauscht werden mit einem Status symbol") or at a right click on it — „Als bestanden
//! markieren", „Verschieben nach ›" with every semester it can go to and whether that one offers
//! it, „Auswählen" (the first of several to do something to, now that no box is there to tick),
//! „Entfernen"; the module itself only where a click on the entry opens the menu rather than the
//! module, in the Gesamtplan (owner, 2026-10-05: „modul ansehen aus dem kontext menu nehmen, weil
//! man ja per klick auf das feld das auf macht") —; a semester's ⋯ in its head — the Stundenplan,
//! all of its rows selected, a semester of leave, an empty semester added beyond the plan taken
//! out again —; where the rows selected move to; and the list of every semester under the name of
//! the one shown, to go to one directly (owner, 2026-10-19: „auf das semester selbst klicken
//! können, um eine selection … zu bekommen"), each with what it holds, „jetzt" at the current one,
//! and „Semester anfügen" after the last. On a desktop a menu stands at what opened it and a
//! submenu beside it; on a phone a menu is a sheet from below, and a submenu takes its place. The
//! arrow keys go through it, Escape and a click beside it close it, and the focus goes back to what
//! opened it.
//!
//! A module only visited has a menu of its own at its sign (`MenuFor::Visit`; folia/design/kalender/
//! konzepte.html, „Das Menü des Besuchs"): „Im Kalender öffnen", „Nur besuchen" ticked (taken away,
//! the module is planned and counts: that is done here, not in the calendar) and „Entfernen"; a
//! planned module's menu has „Nur besuchen" too, unticked (decided 2026-10-19: „Ein geplantes Modul
//! lässt sich auf „nur besuchen“ stellen").
//!
//! What a menu does (`Act`) is the same for one row and for the rows selected (`focus.rs`), and
//! „Rückgängig" takes it back.

use std::collections::BTreeSet;
use std::time::Duration;

use folia_calendar::semester::SemesterKey;
use folia_plans::study::{self, Item, Subject, When};
use folia_routes::url::StudyplanUrl;
use leptos::prelude::*;

use folia_design::nav;
use folia_design::ui::Icon;

use super::dom::{self, Rect, Step};
use super::focus::find_href;
use super::{item_of, module_href, Ready, Selection, StudyCtx};
use crate::i18n::{self, Texts};

/// What a menu is for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MenuFor {
    /// An item of a semester (`Item::key`); `view`: the menu leads to its module, which a click on
    /// the item does not (the Gesamtplan's entries).
    Item { semester: SemesterKey, key: String, view: bool },
    /// Where the rows selected in a semester move to.
    Move(SemesterKey),
    Semester(SemesterKey),
    /// The list of every semester under the name of the one shown (`focus.rs`, `SemesterPick`).
    Pick(SemesterKey),
    /// A module only visited in a semester, by its number (`focus.rs`, `VisitRow`).
    Visit { semester: SemesterKey, id: String },
}

/// Where a menu opens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Anchor {
    /// Under the button that opened it, or above it where the window has no room below; its right
    /// edge at the button's.
    Button(Rect),
    /// The same with its left edge at the button's: a list under a heading.
    Under(Rect),
    /// At the pointer: a right click.
    Point(f64, f64),
}

/// The menu open over the page.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Menu {
    pub what: MenuFor,
    pub at: Anchor,
}

/// What an entry of a menu, or the bar of the rows selected, does.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Act {
    /// Marks the items as passed, or takes that back.
    Passed { semester: SemesterKey, keys: Vec<String>, passed: bool },
    Move { semester: SemesterKey, keys: Vec<String>, to: SemesterKey },
    Remove { semester: SemesterKey, keys: Vec<String> },
    /// Selects every row of a semester.
    SelectAll(SemesterKey),
    /// Selects a row of a semester, besides those selected there.
    Select { semester: SemesterKey, key: String },
    /// Shows a semester (the one after the last: the page that adds it).
    Focus(SemesterKey),
    Leave { semester: SemesterKey, leave: bool },
    /// Takes the last semester out again: added beyond the plan, and empty.
    RemoveSemester(SemesterKey),
    /// „Nur besuchen" on a planned module (by its item's key): it stays in the week, counting for
    /// nothing; taken away from a visit (by its number): it is planned and counts.
    MakeVisit { semester: SemesterKey, key: String },
    CountVisit { semester: SemesterKey, id: String },
    /// Takes a visit out of its semester.
    Unvisit { semester: SemesterKey, id: String },
}

/// An entry of a menu.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    /// What the checks find it by.
    id: Option<&'static str>,
    icon: Option<&'static str>,
    label: String,
    /// A line under the label: „zählt nicht als Fachsemester".
    hint: Option<&'static str>,
    /// Small, at its end: „angeboten", „nicht angeboten", with its class.
    note: Option<(String, &'static str)>,
    /// The current semester, among those to move to.
    now: bool,
    /// The semester shown, in the list of them: a check at its end.
    current: bool,
    danger: bool,
    does: Does,
}

#[derive(Clone, Debug, PartialEq)]
enum Does {
    Act(Act),
    /// On or off.
    Check(bool, Act),
    /// A way to a page, or to the module beside this one (`true`: the page keeps its place).
    Link(String, bool),
    /// A submenu: its title (on a phone), its entries.
    Sub(String, Vec<Entry>),
    /// A line between entries.
    Line,
}

impl Entry {
    fn new(id: Option<&'static str>, icon: Option<&'static str>, label: impl Into<String>, does: Does) -> Self {
        Entry { id, icon, label: label.into(), hint: None, note: None, now: false, current: false, danger: false, does }
    }

    fn line() -> Self {
        Entry::new(None, None, String::new(), Does::Line)
    }

    /// How tall it stands in a menu on a desktop, in px (app.css: `.st-pop-item`, `.st-pop hr`).
    fn height(&self) -> f64 {
        match self.does {
            Does::Line => LINE,
            _ if self.hint.is_some() => ITEM_HINT,
            _ => ITEM,
        }
    }
}

/// The sizes of a menu on a desktop (app.css, `.st-pop`), to place it before it is drawn.
const WIDTH: f64 = 264.0;
const PAD: f64 = 6.0;
const ITEM: f64 = 36.0;
const ITEM_HINT: f64 = 50.0;
const LINE: f64 = 9.0;
/// How far a menu stays from the window's edges.
const EDGE: f64 = 8.0;

/// What a menu shows: its title (the sheet's, on a phone) and its entries; its width on a desktop.
#[derive(Clone, Debug, PartialEq)]
struct Content {
    title: String,
    entries: Vec<Entry>,
    width: f64,
}

impl Content {
    fn new(title: impl Into<String>, entries: Vec<Entry>) -> Self {
        Content { title: title.into(), entries, width: WIDTH }
    }
}

/// How wide the list of the semesters is: its entries say what each holds.
const PICK_WIDTH: f64 = 340.0;

fn height_of(entries: &[Entry]) -> f64 {
    2.0 * PAD + entries.iter().map(Entry::height).sum::<f64>()
}

/// Where a menu of `size` (width, height) stands for `anchor` in the `room` of the window: its left
/// and top edge. Under a button, its right edge at the button's, and above it where there is no
/// room below; at the pointer to the right and below it, to the left and above where there is no
/// room. Never past the window's edges, nor under the rail.
pub(super) fn place(anchor: Anchor, size: (f64, f64), room: nav::Room) -> (f64, f64) {
    let ((width, height), nav::Room { left: rail, width: view_width, height: view_height }) = (size, room);
    let (left, top) = match anchor {
        Anchor::Button(button) | Anchor::Under(button) => {
            let below = button.bottom + 4.0;
            let above = button.top - 4.0 - height;
            let left = if matches!(anchor, Anchor::Under(_)) { button.left } else { button.right - width };
            (left, if below + height <= view_height - EDGE || above < EDGE { below } else { above })
        }
        Anchor::Point(x, y) => (if x + width <= view_width - EDGE { x } else { x - width }, if y + height <= view_height - EDGE { y } else { y - height }),
    };
    let least = rail + EDGE;
    (left.clamp(least, (view_width - width - EDGE).max(least)), top.clamp(EDGE, (view_height - height - EDGE).max(EDGE)))
}

/// Where a submenu of `size` stands beside the menu at `menu`, whose entry begins `entry_top` px
/// from the window's top: right of the menu, left of it where there is no room; its first entry
/// level with the entry. Not under the rail either.
pub(super) fn place_beside(menu: (f64, f64), entry_top: f64, size: (f64, f64), room: nav::Room) -> (f64, f64) {
    let ((left, _), (width, height), nav::Room { left: rail, width: view_width, height: view_height }) = (menu, size, room);
    let right = left + WIDTH;
    let x = if right - 2.0 + width <= view_width - EDGE { right - 2.0 } else { left + 2.0 - width };
    let least = rail + EDGE;
    (x.clamp(least, (view_width - width - EDGE).max(least)), (entry_top - PAD).clamp(EDGE, (view_height - height - EDGE).max(EDGE)))
}

/// Opens the menu `what` under the button the event was heard on; the focus goes back there.
pub(super) fn open_at_button(ctx: StudyCtx, what: MenuFor, ev: &leptos::ev::MouseEvent) {
    dom::keep_trigger(ev);
    let at = dom::rect_of(ev).map_or_else(|| Anchor::Point(f64::from(ev.client_x()), f64::from(ev.client_y())), Anchor::Button);
    ctx.menu.set(Some(Menu { what, at }));
}

/// Opens the menu `what` under the button the event was heard on, its left edge at the button's.
pub(super) fn open_under_button(ctx: StudyCtx, what: MenuFor, ev: &leptos::ev::MouseEvent) {
    dom::keep_trigger(ev);
    let at = dom::rect_of(ev).map_or_else(|| Anchor::Point(f64::from(ev.client_x()), f64::from(ev.client_y())), Anchor::Under);
    ctx.menu.set(Some(Menu { what, at }));
}

/// Opens the menu `what` at the pointer: a right click.
pub(super) fn open_at_pointer(ctx: StudyCtx, what: MenuFor, ev: &leptos::ev::MouseEvent) {
    ctx.menu.set(Some(Menu { what, at: Anchor::Point(f64::from(ev.client_x()), f64::from(ev.client_y())) }));
}

/// The items of `keys` in semester `s`, as they stand.
fn items_of(ctx: StudyCtx, s: SemesterKey, keys: &[String]) -> Vec<Item> {
    ctx.with_ready(|ready| keys.iter().filter_map(|key| item_of(ready, s, key)).collect()).unwrap_or_default()
}

/// The rows selected in semester `s`, in their order.
pub(super) fn selected_keys(ctx: StudyCtx, s: SemesterKey) -> Vec<String> {
    let chosen = ctx.selection.with_untracked(|selection| if selection.semester == Some(s) { selection.keys.clone() } else { BTreeSet::new() });
    ctx.state.with_untracked(|state| state.ready().and_then(|ready| ready.study.semester(s)).map(|semester| semester.items.iter().map(Item::key).filter(|key| chosen.contains(key)).collect()).unwrap_or_default())
}

/// The name of the module visited in semester `s`, as the study has it.
fn visit_name(ctx: StudyCtx, s: SemesterKey, id: &str) -> String {
    ctx.with_ready(|ready| ready.study.semester(s).and_then(|semester| semester.visits.iter().find(|visit| visit.id == id)).map(|visit| visit.name.clone())).flatten().unwrap_or_else(|| id.to_string())
}

/// Does `act`, with a note „Rückgängig" takes it back by where it changes the plan. The rows
/// selected are done with then.
pub(super) fn act(ctx: StudyCtx, act: Act, t: &'static Texts) {
    let s = &t.study;
    match act {
        Act::MakeVisit { semester, key } => {
            let Some(item) = items_of(ctx, semester, std::slice::from_ref(&key)).into_iter().next() else { return };
            let Some(id) = item.module_id().map(str::to_string) else { return };
            ctx.selection.set(Selection::default());
            ctx.change_noted(Some((s.made_visit)(&item.name)), move |doc, _| {
                doc.make_visit(semester, &id);
            }, || {});
        }
        Act::CountVisit { semester, id } => {
            let name = visit_name(ctx, semester, &id);
            ctx.change_noted(Some((s.visit_counts)(&name)), move |doc, _| {
                doc.count_visit(semester, &id);
            }, || {});
        }
        Act::Unvisit { semester, id } => {
            let name = visit_name(ctx, semester, &id);
            ctx.change_noted(Some((s.removed)(&name)), move |doc, _| doc.unvisit(semester, &id, &[]), || {});
        }
        Act::Passed { semester, keys, passed } => {
            let items = items_of(ctx, semester, &keys);
            let note = match (items.as_slice(), passed) {
                ([], _) => return,
                ([one], true) => (s.marked_passed)(&one.name),
                ([one], false) => (s.unmarked_passed)(&one.name),
                (many, true) => (s.n_marked_passed)(many.len()),
                (many, false) => (s.n_unmarked_passed)(many.len()),
            };
            ctx.selection.set(Selection::default());
            ctx.change_noted(Some(note), move |doc, _| {
                for item in &items {
                    study::set_passed(doc, semester, item, passed);
                }
            }, || {});
        }
        Act::Move { semester, keys, to } => {
            let items = items_of(ctx, semester, &keys);
            let label = to.label(t.locale);
            let note = match items.as_slice() {
                [] => return,
                [one] => (s.moved)(&one.name, &label),
                many => (s.n_moved)(many.len(), &label),
            };
            ctx.selection.set(Selection::default());
            ctx.change_noted(Some(note), move |doc, _| {
                for item in &items {
                    study::move_item(doc, semester, to, item);
                }
            }, || {});
        }
        Act::Remove { semester, keys } => {
            let items = items_of(ctx, semester, &keys);
            let note = match items.as_slice() {
                [] => return,
                [one] => (s.removed)(&one.name),
                many => (s.n_removed)(many.len()),
            };
            ctx.selection.set(Selection::default());
            ctx.change_noted(Some(note), move |doc, _| {
                for item in &items {
                    study::remove(doc, semester, item);
                }
            }, || {});
        }
        Act::SelectAll(semester) => {
            let keys: BTreeSet<String> = ctx.with_ready(|ready| ready.study.semester(semester).map(|semester| semester.items.iter().map(Item::key).collect())).flatten().unwrap_or_default();
            ctx.selection.set(Selection { semester: Some(semester), keys, anchor: None });
        }
        Act::Select { semester, key } => {
            ctx.selection.update(|selection| {
                if selection.semester != Some(semester) {
                    *selection = Selection { semester: Some(semester), ..Selection::default() };
                }
                selection.keys.insert(key.clone());
                selection.anchor = Some(key);
            });
        }
        Act::Focus(semester) => ctx.focus.set(Some(semester)),
        Act::Leave { semester, leave } => {
            if let Some(mine) = ctx.mine {
                mine.set_leave(semester, leave);
            }
        }
        Act::RemoveSemester(semester) => {
            let Some(mine) = ctx.mine else { return };
            let (previous, keep) = ctx
                .with_ready(|ready| {
                    let study = &ready.study;
                    let at = study.semesters.iter().position(|other| other.key == semester);
                    let previous = at.and_then(|at| at.checked_sub(1)).and_then(|before| study.semesters.get(before)).map(|other| other.key);
                    (previous, study.regular_end.max(Some(study.now)))
                })
                .unwrap_or_default();
            mine.set_leave(semester, false);
            mine.set_until(previous.filter(|previous| Some(*previous) > keep));
            ctx.focus.set(previous);
        }
    }
}

/// Whether semester `key` can go again: the last, added beyond the plan, empty, still to come.
fn removable(ready: &Ready, key: SemesterKey) -> bool {
    let study = &ready.study;
    let last = study.semesters.last().is_some_and(|last| last.key == key);
    let empty = study.semester(key).is_some_and(|semester| semester.items.is_empty());
    last && empty && ready.setup.until == Some(key) && study.regular_end.is_none_or(|end| key > end) && key > study.now
}

/// The semesters `items` of semester `s` can move to, each a way there: whether it offers them.
/// A semester that is over takes only what was done there or not passed.
fn targets(ready: &Ready, s: SemesterKey, items: &[Item], keys: &[String], t: &Texts) -> Vec<Entry> {
    let st = &t.study;
    if items.is_empty() {
        return Vec::new();
    }
    let back = items.iter().all(|item| item.passed || item.failed);
    ready
        .study
        .semesters
        .iter()
        .filter(|other| other.key != s && (other.when != When::Past || back))
        .map(|other| {
            let unoffered = items.iter().filter(|item| !item.offer.offered(other.key)).count();
            let note = match (unoffered, items.len()) {
                (0, _) => (st.offered.to_string(), "ok"),
                (_, 1) => (st.not_offered.to_string(), "warn"),
                (n, _) => ((st.n_not_offered)(n), "warn"),
            };
            let label = match ready.fs_label(other.key, t) {
                Some(fs) => format!("{} · {fs}", other.key.short(t.locale)),
                None => other.key.short(t.locale),
            };
            Entry { note: Some(note), now: other.when == When::Now, ..Entry::new(None, None, label, Does::Act(Act::Move { semester: s, keys: keys.to_vec(), to: other.key })) }
        })
        .collect()
}

/// What the menu `what` shows, as the study stands.
fn content(ctx: StudyCtx, what: &MenuFor, t: &'static Texts) -> Option<Content> {
    let s = &t.study;
    match what {
        MenuFor::Item { semester, key, view } => {
            let semester = *semester;
            let keys = vec![key.clone()];
            let (item, targets) = ctx
                .with_ready(|ready| {
                    let item = item_of(ready, semester, key)?;
                    let targets = targets(ready, semester, std::slice::from_ref(&item), &keys, t);
                    Some((item, targets))
                })
                .flatten()?;
            let mut entries = Vec::new();
            match (&item.subject, item.module_id()) {
                (_, Some(id)) if *view => entries.push(Entry::new(Some("st-menu-view"), Some("arrow-up-right"), s.view_module, Does::Link(module_href(ctx, id, t), true))),
                (_, Some(_)) => {}
                (Subject::Row { caption, ord, .. }, None) if item.fillers.is_empty() => {
                    if let Some(href) = find_href(ctx, caption, *ord, t) {
                        entries.push(Entry::new(Some("st-menu-view"), Some("search"), s.choose_module, Does::Link(t.path(&href), false)));
                    }
                }
                _ => {}
            }
            if !entries.is_empty() {
                entries.push(Entry::line());
            }
            let passed = Act::Passed { semester, keys: keys.clone(), passed: !item.passed };
            entries.push(if item.passed {
                Entry::new(Some("st-menu-passed"), Some("rotate-ccw"), s.unmark_passed, Does::Act(passed))
            } else {
                Entry::new(Some("st-menu-passed"), Some("circle-check-big"), s.mark_passed, Does::Act(passed))
            });
            if !targets.is_empty() {
                entries.push(Entry::new(Some("st-menu-move"), Some("arrow-right-left"), s.move_to, Does::Sub((s.move_title)(&item.name), targets)));
            }
            // A module not passed there may be only visited: in the week, counting for nothing.
            if item.module_id().is_some() && !item.passed {
                entries.push(Entry { hint: Some(s.only_visit_hint), ..Entry::new(Some("st-menu-visit"), None, s.only_visit, Does::Check(false, Act::MakeVisit { semester, key: key.clone() })) });
            }
            if !*view {
                entries.push(Entry::new(Some("st-menu-select"), Some("list-checks"), s.select_one, Does::Act(Act::Select { semester, key: key.clone() })));
            }
            entries.push(Entry::line());
            entries.push(Entry { danger: true, ..Entry::new(Some("st-menu-remove"), Some("trash-2"), s.remove, Does::Act(Act::Remove { semester, keys })) });
            Some(Content::new(item.name, entries))
        }
        MenuFor::Move(semester) => {
            let semester = *semester;
            let keys = selected_keys(ctx, semester);
            ctx.with_ready(|ready| {
                let items: Vec<Item> = keys.iter().filter_map(|key| item_of(ready, semester, key)).collect();
                let entries = targets(ready, semester, &items, &keys, t);
                (!entries.is_empty()).then(|| Content::new((s.move_n)(items.len()), entries))
            })
            .flatten()
        }
        MenuFor::Semester(semester) => {
            let semester = *semester;
            ctx.with_ready(|ready| {
                let here = ready.study.semester(semester)?;
                let timetable = t.path(&StudyplanUrl { sem: Some(semester.key()), ..Default::default() }.path());
                let mut entries = vec![Entry::new(Some("st-menu-timetable"), Some("calendar-range"), s.in_timetable, Does::Link(timetable, false))];
                if !here.items.is_empty() {
                    entries.push(Entry::new(Some("st-menu-select"), Some("list-checks"), s.select_all, Does::Act(Act::SelectAll(semester))));
                }
                entries.push(Entry { hint: Some(s.leave_note), ..Entry::new(Some("st-menu-leave"), None, s.leave, Does::Check(here.leave, Act::Leave { semester, leave: !here.leave })) });
                if removable(ready, semester) {
                    entries.push(Entry::line());
                    entries.push(Entry { danger: true, ..Entry::new(Some("st-menu-drop"), Some("calendar-minus"), s.remove_semester, Does::Act(Act::RemoveSemester(semester))) });
                }
                Some(Content::new(semester.label(t.locale), entries))
            })
            .flatten()
        }
        MenuFor::Pick(shown) => {
            let shown = *shown;
            ctx.with_ready(|ready| {
                let study = &ready.study;
                let mut entries: Vec<Entry> = study
                    .semesters
                    .iter()
                    .map(|semester| {
                        let label = match ready.fs_label(semester.key, t) {
                            Some(fs) => format!("{} · {fs}", semester.key.short(t.locale)),
                            None => semester.key.short(t.locale),
                        };
                        Entry {
                            note: super::focus::figure(ready, semester.key, t),
                            now: semester.when == When::Now,
                            current: semester.key == shown,
                            ..Entry::new(None, None, label, Does::Act(Act::Focus(semester.key)))
                        }
                    })
                    .collect();
                if let Some(end) = study.semesters.last().and_then(|last| last.key.plus(1)) {
                    entries.push(Entry::line());
                    entries.push(Entry { current: end == shown, ..Entry::new(Some("st-menu-append"), Some("calendar-plus"), s.add_semester, Does::Act(Act::Focus(end))) });
                }
                Content { width: PICK_WIDTH, ..Content::new(s.choose_semester, entries) }
            })
        }
        MenuFor::Visit { semester, id } => {
            let (semester, id) = (*semester, id.clone());
            let name = ctx.with_ready(|ready| ready.study.semester(semester)?.visits.iter().find(|visit| visit.id == id).map(|visit| visit.name.clone())).flatten()?;
            let calendar = t.path(&StudyplanUrl { sem: Some(semester.key()), open: Some(id.clone()), ..Default::default() }.path());
            let entries = vec![
                Entry::new(Some("st-menu-calendar"), Some("calendar-range"), s.in_calendar, Does::Link(calendar, false)),
                Entry::line(),
                Entry { hint: Some(s.only_visit_hint), ..Entry::new(Some("st-menu-visit"), None, s.only_visit, Does::Check(true, Act::CountVisit { semester, id: id.clone() })) },
                Entry::line(),
                Entry { danger: true, ..Entry::new(Some("st-menu-remove"), Some("trash-2"), s.remove, Does::Act(Act::Unvisit { semester, id })) },
            ];
            Some(Content::new(name, entries))
        }
    }
}

/// The menu open, over the page: a desktop's at what opened it, a phone's as a sheet.
#[component]
pub(super) fn MenuHost(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let shown = Memo::new(move |_| {
        let menu = ctx.menu.get()?;
        content(ctx, &menu.what, t).map(|content| (menu.at, content))
    });
    // The submenu open: its entry's place among the entries; whether the keys opened it.
    let sub = RwSignal::new(None::<usize>);
    let by_keys = RwSignal::new(false);
    // A submenu closes a while after the pointer goes to another entry: time to cross to it.
    let closing = StoredValue::new(None::<TimeoutHandle>);
    let stay = move || {
        if let Some(handle) = closing.get_value() {
            handle.clear();
            closing.set_value(None);
        }
    };
    let leave_sub = move || {
        if sub.get_untracked().is_some() && closing.get_value().is_none() {
            let handle = set_timeout_with_handle(
                move || {
                    closing.set_value(None);
                    sub.try_set(None);
                },
                Duration::from_millis(300),
            );
            closing.set_value(handle.ok());
        }
    };
    // A menu opened: its first entry has the focus (the arrows go on from there); in the list of the
    // semesters the one shown.
    Effect::new(move |_| {
        let open = ctx.menu.with(Option::is_some);
        stay();
        sub.set(None);
        if open {
            request_animation_frame(|| dom::step_focus(".st-pop.root", Step::Current));
        }
    });
    Effect::new(move |_| {
        if sub.get().is_some() && by_keys.get_untracked() {
            request_animation_frame(|| dom::step_focus(".st-pop.sub", Step::First));
        }
    });
    let close = move || {
        stay();
        ctx.menu.set(None);
        dom::give_focus_back();
    };
    let run = move |act: Act| {
        close();
        self::act(ctx, act, t);
    };
    let open_sub = move |i: usize, keys: bool| {
        stay();
        by_keys.set(keys);
        sub.set(Some(i));
    };
    let close_sub = move || {
        stay();
        if let Some(i) = sub.get_untracked() {
            sub.set(None);
            nav::focus_selector(&format!(".st-pop.root [data-entry=\"{i}\"]"));
        }
    };
    let root_keys = move |ev: leptos::ev::KeyboardEvent| {
        let step = match ev.key().as_str() {
            "ArrowDown" => Some(Step::Next),
            "ArrowUp" => Some(Step::Previous),
            "Home" => Some(Step::First),
            "End" => Some(Step::Last),
            "ArrowRight" => {
                let at = dom::focused_attribute("data-entry").and_then(|i| i.parse::<usize>().ok());
                let has_sub = at.is_some_and(|at| shown.with_untracked(|shown| shown.as_ref().is_some_and(|(_, content)| matches!(content.entries.get(at).map(|entry| &entry.does), Some(Does::Sub(..))))));
                if let (Some(at), true) = (at, has_sub) {
                    ev.prevent_default();
                    open_sub(at, true);
                }
                None
            }
            "Escape" => {
                ev.prevent_default();
                ev.stop_propagation();
                close();
                None
            }
            "Tab" => {
                ev.prevent_default();
                close();
                None
            }
            _ => None,
        };
        if let Some(step) = step {
            ev.prevent_default();
            dom::step_focus(".st-pop.root", step);
        }
    };
    let sub_keys = move |ev: leptos::ev::KeyboardEvent| {
        let step = match ev.key().as_str() {
            "ArrowDown" => Some(Step::Next),
            "ArrowUp" => Some(Step::Previous),
            "Home" => Some(Step::First),
            "End" => Some(Step::Last),
            "ArrowLeft" | "Escape" => {
                ev.prevent_default();
                ev.stop_propagation();
                close_sub();
                None
            }
            "Tab" => {
                ev.prevent_default();
                close();
                None
            }
            _ => None,
        };
        if let Some(step) = step {
            ev.prevent_default();
            dom::step_focus(".st-pop.sub", step);
        }
    };

    // One entry; `i` its place among the menu's entries, `None` in a submenu.
    let entry = move |i: Option<usize>, entry: Entry| -> AnyView {
        let Entry { id, icon, label, hint, note, now, current, danger, does } = entry;
        let element_id = id.map(str::to_string).or_else(|| i.map(|i| format!("st-pop-e{i}")));
        let icon = move || icon.map(|icon| view! { <Icon name=icon/> });
        let words = view! {
            <span class="st-pop-label">
                <span>{label}{now.then(|| view! { " " <span class="st-now">{t.study.now}</span> })}</span>
                {hint.map(|hint| view! { <small>{hint}</small> })}
            </span>
        };
        let note = note.map(|(text, class)| view! { <span class=format!("st-pop-note {class}")>{(class == "warn").then(|| view! { <Icon name="triangle-alert"/> })}{text}</span> });
        // The semester shown, in the list of them.
        let mark = current.then(|| view! { <Icon name="check" class="st-pop-current"/> });
        // A mouse moved onto another entry leaves the submenu. Only a move counts, not a menu
        // drawn under a pointer that stands still, and a phone opens a submenu by a tap alone.
        let hovers = move |ev: &leptos::ev::PointerEvent| ev.pointer_type() == "mouse" && !ctx.phone.get_untracked();
        let others = move |ev: leptos::ev::PointerEvent| {
            if i.is_some() && hovers(&ev) {
                leave_sub();
            }
        };
        match does {
            Does::Line => view! { <hr/> }.into_any(),
            Does::Link(href, keep) => view! {
                <a class="st-pop-item" role="menuitem" id=element_id href=href data-noscroll=keep.then_some("") on:click=move |_| ctx.menu.set(None) on:pointermove=others>
                    {icon()}{words}{note}
                </a>
            }
            .into_any(),
            Does::Act(act) => view! {
                <button class="st-pop-item" class:danger=danger type="button" role="menuitem" id=element_id aria-current=current.then_some("true") on:click=move |_| run(act.clone()) on:pointermove=others>
                    {icon()}{words}{note}{mark}
                </button>
            }
            .into_any(),
            Does::Check(on, act) => view! {
                <button class="st-pop-item with-hint" type="button" role="menuitemcheckbox" aria-checked=if on { "true" } else { "false" } id=element_id on:click=move |_| run(act.clone()) on:pointermove=others>
                    <span class="st-box" class:with=on aria-hidden="true"><Icon name="check"/></span>{words}
                </button>
            }
            .into_any(),
            Does::Sub(..) => {
                let at = i.unwrap_or_default();
                view! {
                    <button
                        class="st-pop-item"
                        type="button"
                        role="menuitem"
                        aria-haspopup="menu"
                        aria-expanded=move || if sub.get() == Some(at) { "true" } else { "false" }
                        id=element_id
                        data-entry=at
                        // A click opens it (and keeps it open where the pointer opened it already); from
                        // the keys (Enter, Space) its first entry takes the focus.
                        on:click=move |ev: leptos::ev::MouseEvent| open_sub(at, ev.detail() == 0)
                        on:pointermove=move |ev: leptos::ev::PointerEvent| if hovers(&ev) && sub.get_untracked() != Some(at) { open_sub(at, false) }
                    >
                        {icon()}{words}<Icon name="chevron-right" class="st-pop-more"/>
                    </button>
                }
                .into_any()
            }
        }
    };

    // The menu is drawn anew only when it is another; a submenu opening or closing leaves it as it
    // is (with the focus and the pointer where they are).
    move || {
        let (at, content) = shown.get()?;
        let view_size = nav::room();
        let height = height_of(&content.entries).min(view_size.height - 2.0 * EDGE);
        let width = content.width;
        let (left, top) = place(at, (width, height), view_size);
        let entries = content.entries.clone();
        let sub_panel = move || {
            let open = sub.get()?;
            let Some(Does::Sub(title, sub_entries)) = entries.get(open).map(|entry| entry.does.clone()) else { return None };
            let entry_top = top + PAD + entries.get(..open).unwrap_or_default().iter().map(Entry::height).sum::<f64>();
            let size = (WIDTH, height_of(&sub_entries).min(view_size.height - 2.0 * EDGE));
            let (x, y) = place_beside((left, top), entry_top, size, view_size);
            let label = title.clone();
            Some(view! {
                <div class="st-pop sub" role="menu" aria-label=label style=format!("--x:{x:.0}px;--y:{y:.0}px") on:keydown=sub_keys on:pointerenter=move |_| stay()>
                    <button class="st-pop-back st-phone" type="button" on:click=move |_| close_sub()><Icon name="chevron-left"/><span>{title}</span></button>
                    {sub_entries.into_iter().map(|sub_entry| entry(None, sub_entry)).collect_view()}
                </div>
            })
        };
        let (title, label) = (content.title.clone(), content.title.clone());
        Some(view! {
            <div class="st-pop-shade" on:mousedown=|ev: leptos::ev::MouseEvent| ev.prevent_default() on:click=move |_| close() on:contextmenu=move |ev: leptos::ev::MouseEvent| { ev.prevent_default(); close(); } on:wheel=move |_| ctx.menu.set(None)></div>
            <div class="st-pop root" class:has-sub=move || sub.get().is_some() role="menu" aria-label=label style=format!("--x:{left:.0}px;--y:{top:.0}px;--w:{width:.0}px") on:keydown=root_keys>
                <p class="st-pop-title st-phone">{title}</p>
                {content.entries.into_iter().enumerate().map(|(i, root_entry)| entry(Some(i), root_entry)).collect_view()}
            </div>
            {sub_panel}
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: nav::Room = nav::Room::window(1440.0, 900.0);

    fn button(left: f64, top: f64) -> Anchor {
        Anchor::Button(Rect { left, top, right: left + 32.0, bottom: top + 32.0 })
    }

    /// Under its button, the right edges flush; above it at the window's bottom; never past an
    /// edge. At the pointer, to the right and below, else to the left and above.
    #[test]
    fn a_menu_stands_where_the_window_has_room() {
        assert_eq!(place(button(900.0, 300.0), (264.0, 200.0), VIEW), (668.0, 336.0));
        assert_eq!(place(button(900.0, 800.0), (264.0, 200.0), VIEW), (668.0, 596.0), "above the button");
        assert_eq!(place(button(100.0, 300.0), (264.0, 200.0), VIEW).0, 8.0, "not past the left edge");
        assert_eq!(place(button(100.0, 300.0), (264.0, 200.0), nav::Room { left: 52.0, ..VIEW }).0, 60.0, "not under the rail");
        assert_eq!(place(Anchor::Point(500.0, 400.0), (264.0, 200.0), VIEW), (500.0, 400.0));
        // The list of the semesters under the name of the one shown: their left edges flush.
        assert_eq!(place(Anchor::Under(Rect { left: 400.0, top: 300.0, right: 560.0, bottom: 332.0 }), (340.0, 200.0), VIEW), (400.0, 336.0));
        assert_eq!(place(Anchor::Point(1300.0, 850.0), (264.0, 200.0), VIEW), (1036.0, 650.0));
        // Taller than the window: from its top.
        assert_eq!(place(button(900.0, 300.0), (264.0, 2000.0), VIEW).1, 8.0);
        // Beside the menu, or left of it at the right edge.
        assert_eq!(place_beside((600.0, 300.0), 340.0, (264.0, 300.0), VIEW), (862.0, 334.0));
        assert_eq!(place_beside((1150.0, 300.0), 340.0, (264.0, 300.0), VIEW), (888.0, 334.0));
    }
}
