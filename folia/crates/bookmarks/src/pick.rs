//! Several modules at once on the Merkliste (folia/design/merken/oberflaeche.html, „Mehrere
//! auswählen"; konzept.html, „Mehrere auf einmal"): selected as in „Mein Studium" since 1.31.0 —
//! no boxes, the row tinted and hatched (app.css `.row-wrap.is-selected`), the bar in the place of
//! the heads of the columns, on a phone over the tab bar.
//!
//! - On a wide screen Ctrl or ⌘ and a click selects a row or lets it go, Shift and a click takes in
//!   the rows from the last one clicked, and a mouse pressed on one row and drawn onto others
//!   selects them (begun on a selected row, it lets them go); „Auswählen" stands in a row's menu,
//!   its right click. While rows are selected, a click selects or lets go.
//! - On a phone a finger held still on a row selects it (`folia_widgets::swipe`), and drawn on,
//!   the rows under it; then a tap selects or lets go. Swiping stays.
//! - Keys, as in „Mein Studium": Shift and ↑ ↓ take in the next row (or let go of the last, back
//!   towards where it began), Ctrl or ⌘ and A every row the list shows, Esc lets go of them, the
//!   space bar at a row selects it.
//! - The bar: „3 ausgewählt · 14 LP", „Alle auswählen" (a wide screen's), „Sammlung" (the menu of
//!   their collections, `folia_widgets::collect`, and S), „Einplanen" (each where its own
//!   „Einplanen" aims, or all into one semester, the semesters as „Verschieben nach" lists them),
//!   „Teilen" (a link for another device, or the list as text), „Entfernen", and letting go. What
//!   it did the note at the bottom says, with „Rückgängig" for all of them.
//! - A range, a drawn selection and „Alle auswählen" take in the rows that are marked; a row
//!   whose mark was taken away here (dimmed until the page is left) only when it is chosen itself.
//! - What leaves the list is let go of: a search, a filter, another view of it.

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::semester::SemesterKey;
use folia_data::use_data;
use folia_design::nav;
use folia_design::ui::{Icon, KeysText};
use folia_plans::study::{fs_of, Offer};
use folia_routes::url;
use folia_stores::bookmarks::{share_fragment, Bookmarks, Brought};
use folia_stores::collections::{Anchor, CollectAsk, Collecting, Collections};
use folia_stores::myprogram::MyProgram;
use folia_stores::studyplan::{aim_now, PlanHint, Planning, Pressed, Studyplan};
use folia_widgets::marknote::{Notice, Notices};
use leptos::ev::{KeyboardEvent, MouseEvent, PointerEvent, TouchEvent};
use leptos::prelude::*;

use crate::bookmarks::{absolute, box_of, copy, menu_style_wide, module_line, notes_of, step_focus, MENU_HINT, MENU_ITEM};

/// How wide the menus of the selection and of a row are on a wide screen: a semester with its
/// Fachsemester, „jetzt" and „2 nicht angeboten" on one line.
const WIDE: f64 = 300.0;
use crate::i18n::{self, Texts};
use folia_design::format;

/// What the list knows of a row the selection takes in: what the bar counts and „Einplanen" aims
/// with.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PickRow {
    pub title: String,
    pub credits: Option<f64>,
    pub offer: Offer,
}

/// The list as the selection reads it: its rows in their order, and what it knows of each.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Listed {
    pub order: Vec<String>,
    pub rows: BTreeMap<String, PickRow>,
}

/// The rows selected, and the row a range begins at (Shift).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Picked {
    pub ids: BTreeSet<String>,
    pub anchor: Option<String>,
}

/// A selection drawn over the rows: the row it began at, whether it selects or lets go, what was
/// selected before it.
#[derive(Clone, Debug, PartialEq)]
struct Paint {
    start: usize,
    select: bool,
    base: BTreeSet<String>,
}

/// What is selected while a selection drawn over the rows is at row `at`: the rows between it and
/// where it began selected (those `takes` takes in, and the row it began at) or let go of, the
/// others as they were before.
fn painted(order: &[String], paint: &Paint, at: usize, takes: impl Fn(&str) -> bool) -> BTreeSet<String> {
    let (from, to) = (paint.start.min(at), paint.start.max(at));
    let mut ids = paint.base.clone();
    for (i, id) in order.iter().enumerate().take(to + 1).skip(from) {
        if !paint.select {
            ids.remove(id);
        } else if i == paint.start || takes(id) {
            ids.insert(id.clone());
        }
    }
    ids
}

/// The rows from `from` to `to` (Shift and a click), those `takes` takes in and `to` itself.
fn range(order: &[String], from: &str, to: &str, takes: impl Fn(&str) -> bool) -> Vec<String> {
    let (Some(a), Some(b)) = (order.iter().position(|id| id == from), order.iter().position(|id| id == to)) else { return vec![to.to_string()] };
    order.iter().enumerate().take(a.max(b) + 1).skip(a.min(b)).filter(|(i, id)| *i == b || takes(id)).map(|(_, id)| id.clone()).collect()
}

/// Shift and ↑ or ↓ at row `at` (`by`, ±1): the row it goes to, and what is selected then — the
/// rows from where the range began (`anchor`) to there, taken in going away from it, let go of
/// coming back towards it.
fn stepped(order: &[String], picked: &Picked, at: &str, by: isize) -> Option<(String, Picked)> {
    let here = order.iter().position(|id| id == at)?;
    let next = here.checked_add_signed(by).filter(|next| *next < order.len())?;
    let mut picked = picked.clone();
    if !picked.ids.contains(at) || picked.anchor.as_deref().is_none_or(|anchor| !picked.ids.contains(anchor)) {
        picked.ids.insert(at.to_string());
        picked.anchor = Some(at.to_string());
    }
    let anchor = picked.anchor.as_deref().and_then(|anchor| order.iter().position(|id| id == anchor)).unwrap_or(here);
    let to = order.get(next)?.clone();
    if next.abs_diff(anchor) > here.abs_diff(anchor) {
        picked.ids.insert(to.clone());
    } else {
        picked.ids.remove(at);
    }
    Some((to, picked))
}

/// A menu of the selection or of a row, and where it stands.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PickMenu {
    /// „Einplanen" of the rows selected; `earlier`: with the semesters behind the student unfolded.
    Plan { at: Anchor, earlier: bool },
    /// „Teilen" of the rows selected.
    Share { at: Anchor },
    /// A row's menu, where the pointer is.
    Row { id: String, at: Anchor },
}

impl PickMenu {
    fn at(&self) -> Anchor {
        match self {
            PickMenu::Plan { at, .. } | PickMenu::Share { at } | PickMenu::Row { at, .. } => *at,
        }
    }
}

/// The selection of the Merkliste, shared by the page, its list and the bar.
#[derive(Clone, Copy)]
pub(crate) struct Picking {
    pub picked: RwSignal<Picked>,
    /// The list as it is shown now (`List` sets it as it builds it).
    listed: StoredValue<Listed>,
    /// A mouse's press on a row's link: the module, where.
    press: StoredValue<Option<(String, i32, i32)>>,
    paint: StoredValue<Option<Paint>>,
    /// The click that ends a drawn selection, or a finger's hold, is none.
    drawn: StoredValue<bool>,
    /// The last press was a finger's: its long press (`contextmenu`) opens no menu.
    touched: StoredValue<bool>,
    pub menu: RwSignal<Option<PickMenu>>,
    /// „Notizen mitnehmen" of „Teilen", here and in a collection's menu: off until switched on,
    /// for as long as the page is open (oberflaeche.html: „Notizen bleiben zu Hause, außer du
    /// nimmst sie mit").
    pub notes_along: RwSignal<bool>,
    bookmarks: Option<Bookmarks>,
    /// The note at the foot that says what was taken away, with „Rückgängig".
    notices: Option<Notices>,
    /// What „Einplanen" aims with: the snapshot's current semester, the semester of each module's
    /// newest events, the hint of the list („Passt in meinen Stundenplan").
    current: Memo<Option<SemesterKey>>,
    newest: Memo<BTreeMap<String, SemesterKey>>,
    hint: Memo<Option<PlanHint>>,
}

impl Picking {
    pub fn new(current: Memo<Option<SemesterKey>>, newest: Memo<BTreeMap<String, SemesterKey>>, hint: Memo<Option<PlanHint>>) -> Self {
        Picking {
            picked: RwSignal::new(Picked::default()),
            listed: StoredValue::new(Listed::default()),
            press: StoredValue::new(None),
            paint: StoredValue::new(None),
            drawn: StoredValue::new(false),
            touched: StoredValue::new(false),
            menu: RwSignal::new(None),
            notes_along: RwSignal::new(false),
            bookmarks: Bookmarks::expect(),
            notices: Notices::expect(),
            current,
            newest,
            hint,
        }
    }

    /// The list as it is shown now; what has left it is let go of (after the list is built).
    pub fn list(self, listed: Listed) {
        self.listed.set_value(listed);
    }

    pub fn keep_listed(self) {
        let keeps = self.listed.with_value(|listed| self.picked.with_untracked(|picked| picked.ids.iter().all(|id| listed.rows.contains_key(id))));
        if !keeps {
            self.picked.update(|picked| self.listed.with_value(|listed| picked.ids.retain(|id| listed.rows.contains_key(id))));
        }
    }

    /// Takes `ids` off the Merkliste, as „Entfernen" of the bar does, and lets go of the selection:
    /// the note at the foot says so and has „Rückgängig" (the rows stay, dimmed, `List`).
    pub fn remove(self, ids: Vec<String>) {
        let Some(bookmarks) = self.bookmarks else { return };
        let marked: Vec<String> = ids.into_iter().filter(|id| bookmarks.is_marked_untracked(id)).collect();
        self.clear();
        if marked.is_empty() {
            return;
        }
        let cleared = bookmarks.take_away(&marked);
        if let Some(notices) = self.notices {
            notices.say(Notice::Removed(cleared));
        }
    }

    /// Whether rows are selected. Tracked.
    pub fn any(self) -> bool {
        self.picked.with(|picked| !picked.ids.is_empty())
    }

    pub fn any_untracked(self) -> bool {
        self.picked.with_untracked(|picked| !picked.ids.is_empty())
    }

    pub fn clear(self) {
        if self.any_untracked() || self.picked.with_untracked(|picked| picked.anchor.is_some()) {
            self.picked.set(Picked::default());
        }
    }

    /// The rows selected, in the list's order.
    pub fn ids_untracked(self) -> Vec<String> {
        self.picked.with_untracked(|picked| self.listed.with_value(|listed| listed.order.iter().filter(|id| picked.ids.contains(*id)).cloned().collect()))
    }

    fn marked(self, id: &str) -> bool {
        self.bookmarks.is_some_and(|bookmarks| bookmarks.is_marked_untracked(id))
    }

    fn order(self) -> Vec<String> {
        self.listed.with_value(|listed| listed.order.clone())
    }

    /// Selects the row or lets it go.
    pub fn toggle(self, id: &str) {
        self.picked.update(|picked| {
            if !picked.ids.remove(id) {
                picked.ids.insert(id.to_string());
            }
            picked.anchor = Some(id.to_string());
        });
    }

    /// Shift and a click: the rows from the last one clicked to `id`, besides those selected.
    pub fn extend(self, id: &str) {
        let order = self.order();
        let from = self.picked.with_untracked(|picked| picked.anchor.clone()).unwrap_or_else(|| id.to_string());
        let taken = range(&order, &from, id, |other| self.marked(other));
        self.picked.update(|picked| {
            picked.ids.extend(taken);
            picked.anchor.get_or_insert_with(|| id.to_string());
        });
    }

    /// Every row the list shows that is marked.
    pub fn select_all(self) {
        let ids: BTreeSet<String> = self.order().into_iter().filter(|id| self.marked(id)).collect();
        if !ids.is_empty() {
            self.picked.set(Picked { ids, anchor: None });
        }
    }

    /// Whether the list shows a row „Alle auswählen" would take in.
    pub fn can_select_all(self) -> bool {
        self.listed.with_value(|listed| listed.order.iter().any(|id| self.marked(id)))
    }

    /// Whether every row „Alle auswählen" would take in is selected. Tracked.
    fn all_in(self) -> bool {
        self.picked.with(|picked| self.listed.with_value(|listed| listed.order.iter().filter(|id| self.marked(id)).all(|id| picked.ids.contains(id))))
    }

    /// A press on the list: a mouse's on a row's link may draw a selection (on a wide screen; a
    /// phone's begins with a finger held still, `hold`).
    pub fn down(self, ev: &PointerEvent, phone: bool) {
        self.touched.set_value(ev.pointer_type() == "touch");
        self.drawn.set_value(false);
        self.press.set_value(None);
        self.paint.set_value(None);
        if phone || ev.pointer_type() != "mouse" || ev.button() != 0 || ev.shift_key() || ev.ctrl_key() || ev.meta_key() || ev.alt_key() || on_control(ev) {
            return;
        }
        if let Some(id) = row_id_of(ev) {
            self.press.set_value(Some((id, ev.client_x(), ev.client_y())));
        }
    }

    /// A finger held still on row `id` (`folia_widgets::swipe`): the selection drawn from it begins
    /// there, and the finger draws it on.
    pub fn hold(self, id: String) {
        self.press.set_value(Some((id, 0, 0)));
        self.paint_from();
    }

    /// The selection drawn from the row pressed begins: that row selected, or let go of.
    fn paint_from(self) {
        let Some((id, ..)) = self.press.get_value() else { return };
        let order = self.order();
        let Some(at) = order.iter().position(|other| *other == id) else { return };
        let base = self.picked.with_untracked(|picked| picked.ids.clone());
        let paint = Paint { start: at, select: !base.contains(&id), base };
        let ids = painted(&order, &paint, at, |other| self.marked(other));
        self.picked.set(Picked { ids, anchor: Some(id) });
        self.paint.set_value(Some(paint));
        self.drawn.set_value(true);
    }

    /// The pointer moved over the list: a mouse drawn from the row pressed onto another begins a
    /// selection; one under way takes in the rows up to where the pointer is.
    pub fn moved(self, ev: &PointerEvent) {
        let (x, y) = (f64::from(ev.client_x()), f64::from(ev.client_y()));
        if self.paint.with_value(Option::is_none) {
            let Some((pressed, ..)) = self.press.get_value() else { return };
            if ev.buttons() & 1 == 0 {
                self.press.set_value(None);
                return;
            }
            if row_at(x, y).is_none_or(|over| over == pressed) {
                return;
            }
            capture(ev);
            self.paint_from();
        }
        let Some(over) = row_at(x, y).or_else(|| row_nearest(y)) else { return };
        let order = self.order();
        let Some(at) = order.iter().position(|other| *other == over) else { return };
        let ids = self.paint.with_value(|paint| paint.as_ref().map(|paint| painted(&order, paint, at, |other| self.marked(other))));
        if let Some(ids) = ids.filter(|ids| self.picked.with_untracked(|picked| picked.ids != *ids)) {
            self.picked.update(|picked| picked.ids = ids);
        }
    }

    /// The press is over.
    pub fn end(self) {
        self.press.set_value(None);
        self.paint.set_value(None);
    }

    /// While a finger draws a selection the page does not scroll under it: the listener is the
    /// list's own (the app is built without Leptos's delegation of events), so the browser lets it
    /// cancel the move.
    pub fn touch_move(self, ev: &TouchEvent) {
        if self.paint.with_value(Option::is_some) && ev.cancelable() {
            ev.prevent_default();
        }
    }

    /// A click on a row: the end of a drawn selection is none; with Shift it takes in the rows up
    /// to it, with Ctrl or ⌘, or while rows are selected, it selects the row or lets it go; else it
    /// is the link's (the preview, on a phone the module's page).
    pub fn click(self, ev: &MouseEvent) {
        if on_control(ev) {
            return;
        }
        let Some(id) = row_id_of(ev) else { return };
        // Not one of the keys (Enter): those come without a press.
        if ev.detail() != 0 && self.drawn.get_value() {
            self.drawn.set_value(false);
            ev.prevent_default();
            return;
        }
        if ev.shift_key() {
            ev.prevent_default();
            self.extend(&id);
        } else if ev.ctrl_key() || ev.meta_key() || self.any_untracked() {
            ev.prevent_default();
            self.toggle(&id);
        }
    }

    /// A right click on a row opens its menu where the pointer is; a finger's long press selects
    /// (`hold`) and opens nothing.
    pub fn context(self, ev: &MouseEvent) {
        let Some(id) = row_id_of(ev) else { return };
        ev.prevent_default();
        if self.touched.get_value() {
            return;
        }
        let (x, y) = (f64::from(ev.client_x()), f64::from(ev.client_y()));
        self.menu.set(Some(PickMenu::Row { id, at: Anchor { left: x, top: y, right: x + WIDE, bottom: y } }));
    }

    /// The keys at a row's link: the space bar selects it or lets it go, Shift and ↑ ↓ take in the
    /// next row (`stepped`).
    pub fn keys(self, ev: &KeyboardEvent) {
        if !on_link(ev) || ev.ctrl_key() || ev.meta_key() || ev.alt_key() {
            return;
        }
        let Some(id) = row_id_of(ev) else { return };
        let by = match ev.key().as_str() {
            " " if !ev.shift_key() => {
                ev.prevent_default();
                self.toggle(&id);
                return;
            }
            // Entf (⌫ on a Mac) at a row: it, or the rows selected, off the Merkliste, with
            // „Rückgängig" (folia/design/tastatur/konzept.html, „Die anderen Seiten": „Entf mit
            // „Rückgängig“"), as „Entfernen" of the bar.
            "Delete" | "Backspace" if !ev.shift_key() && !ev.repeat() => {
                ev.prevent_default();
                let ids = if self.any_untracked() { self.ids_untracked() } else { vec![id] };
                self.remove(ids);
                return;
            }
            "ArrowDown" if ev.shift_key() => 1,
            "ArrowUp" if ev.shift_key() => -1,
            _ => return,
        };
        ev.prevent_default();
        ev.stop_propagation();
        let order = self.order();
        let next = self.picked.with_untracked(|picked| stepped(&order, picked, &id, by));
        if let Some((to, picked)) = next {
            self.picked.set(picked);
            focus_row(&to);
        }
    }

    /// The module's credits and title, as the list knows them.
    fn row(self, id: &str) -> Option<PickRow> {
        self.listed.with_value(|listed| listed.rows.get(id).cloned())
    }
}

/// The bar of the rows selected, in the place of the heads of the columns (on a phone over the tab
/// bar, app.css): how many and their credits, „Alle auswählen" while some are not, „Sammlung",
/// „Einplanen", „Teilen", „Entfernen", and letting go of them. Its words in full where the list has
/// room for them.
#[component]
pub(crate) fn PickBar(picking: Picking) -> impl IntoView {
    let t = i18n::t();
    let b = &t.bookmarks;
    let collecting = Collecting::expect();
    let summary = Memo::new(move |_| {
        let ids = picking.picked.with(|picked| picked.ids.clone());
        let credits: f64 = ids.iter().filter_map(|id| picking.row(id).and_then(|row| row.credits)).sum();
        (ids.len(), credits)
    });
    let all_in = Memo::new(move |_| picking.all_in());
    let collect = move |ev: MouseEvent| {
        let Some(collecting) = collecting else { return };
        let ids = picking.ids_untracked();
        let Some((first, more)) = ids.split_first() else { return };
        collecting.open(CollectAsk { id: first.clone(), title: (t.marks.n_modules)(ids.len()), more: more.to_vec(), at: box_of(&ev), note: false, from_note: None });
    };
    let remove = move |_| picking.remove(picking.ids_untracked());
    view! {
        // Its buttons are commands of „Befehle" (folia_shell::commands) while rows are selected.
        <div class="st-selbar pick-bar" role="toolbar" aria-label=b.selection data-commands=b.selection>
            <span class="st-selbar-count" aria-live="polite">
                <span class="pick-n num">{move || summary.get().0}</span>
                <span class="pick-words">{move || {
                    let (n, credits) = summary.get();
                    let words = (b.n_selected)(n);
                    let words = words.strip_prefix(&n.to_string()).unwrap_or(&words).to_string();
                    view! { {words}<span class="st-long">{format!(" · {}", format::credits(Some(credits), t.locale))}</span> }
                }}</span>
            </span>
            <button class="st-act st-pc" type="button" id="pick-all" title=b.select_all aria-label=b.select_all data-command="" data-command-icon="list-checks" disabled=move || all_in.get() on:click=move |_| picking.select_all()>
                <Icon name="list-checks"/>
                <span class="st-long">{b.select_all}</span>
            </button>
            <span class="st-grow"></span>
            <button class="st-act" type="button" id="pick-collect" data-action="collect" aria-haspopup="dialog" title=t.marks.collect_hint data-command=b.act_collect data-command-keys="Shift+M" on:click=collect>
                <Icon name="tag"/>
                <span>{b.act_collect}</span>
                <Icon name="chevron-down" class="st-pc"/>
            </button>
            <button class="st-act" type="button" id="pick-plan" aria-haspopup="menu" data-command=t.planner.plan on:click=move |ev: MouseEvent| picking.menu.set(box_of(&ev).map(|at| PickMenu::Plan { at, earlier: false }))>
                <Icon name="calendar-plus"/>
                <span>{t.planner.plan}</span>
                <Icon name="chevron-down" class="st-pc"/>
            </button>
            <button class="st-act" type="button" id="pick-share" aria-haspopup="menu" data-command=b.act_share on:click=move |ev: MouseEvent| picking.menu.set(box_of(&ev).map(|at| PickMenu::Share { at }))>
                <Icon name="share-2"/>
                <span>{b.act_share}</span>
            </button>
            <button class="st-act danger" type="button" id="pick-remove" on:click=remove data-command=t.marks.remove data-command-keys="Del">
                <Icon name="trash-2"/>
                <span>{t.marks.remove}</span>
            </button>
            <button class="icon-btn st-act-close hit" type="button" aria-label=b.clear_selection title=b.clear_selection on:click=move |_| picking.clear() data-command="" data-command-keys="Esc">
                <Icon name="x"/>
            </button>
        </div>
    }
}

/// An entry of a menu of the selection or of a row.
struct Item {
    icon: Option<&'static str>,
    label: String,
    hint: Option<String>,
    /// What it means for the modules there („angeboten", „2 nicht angeboten"), and how it reads.
    note: Option<(String, &'static str)>,
    now: bool,
    key: Option<&'static str>,
    danger: bool,
    does: Callback<()>,
}

/// What a menu shows besides its entries: a line before an entry, a label over the next ones, a
/// switch.
enum Part {
    Item(Item),
    Line,
    Label(&'static str),
    Switch(&'static str, RwSignal<bool>),
}

impl Part {
    fn height(&self) -> f64 {
        match self {
            Part::Item(item) if item.hint.is_some() => MENU_HINT,
            Part::Item(_) | Part::Switch(..) => MENU_ITEM,
            Part::Line => 9.0,
            Part::Label(_) => 26.0,
        }
    }
}

/// The menu open, of the selection or of a row: on a wide screen at what opened it, on a phone a
/// sheet from below (app.css `.st-pop`), as the menus of the collections.
#[component]
pub(crate) fn PickMenus(picking: Picking) -> impl IntoView {
    let t = i18n::t();
    let close = move || picking.menu.set(None);
    folia_shell::nav::layer_while(move || picking.menu.with(Option::is_some), close, folia_shell::nav::Kind::Over);
    // Its first entry has the focus as it opens, the arrows go on from there.
    Effect::new(move |_| {
        if picking.menu.with(Option::is_some) {
            request_animation_frame(|| {
                nav::focus_selector(".pick-menu [role=menuitem]");
            });
        }
    });
    let keys = move |ev: KeyboardEvent| {
        let step: i32 = match ev.key().as_str() {
            "ArrowDown" => 1,
            "ArrowUp" => -1,
            "Escape" | "Tab" => {
                ev.prevent_default();
                ev.stop_propagation();
                close();
                return;
            }
            // A key an entry shows (M, ⇧M) is that entry while the menu is open, as it is at the row.
            key if key.chars().count() == 1 && !ev.ctrl_key() && !ev.meta_key() && !ev.alt_key() => {
                let shown = if ev.shift_key() { format!("⇧{}", key.to_uppercase()) } else { key.to_uppercase() };
                if !ev.repeat() && nav::click_selector(&format!(".pick-menu [data-key=\"{shown}\"]")) {
                    ev.prevent_default();
                    ev.stop_propagation();
                }
                return;
            }
            _ => return,
        };
        ev.prevent_default();
        step_focus(step);
    };
    move || {
        let menu = picking.menu.get()?;
        let (title, parts) = match &menu {
            PickMenu::Plan { earlier, .. } => plan_parts(picking, *earlier, t),
            PickMenu::Share { .. } => share_parts(picking, t),
            PickMenu::Row { id, .. } => row_parts(picking, id, t),
        }?;
        let height = parts.iter().map(Part::height).sum::<f64>() + 12.0;
        let style = menu_style_wide(menu.at(), WIDE, height);
        let label = title.clone();
        Some(view! {
            <div class="st-pop-shade" on:mousedown=|ev: MouseEvent| ev.prevent_default() on:click=move |_| close() on:contextmenu=move |ev: MouseEvent| { ev.prevent_default(); close(); }></div>
            <div class="st-pop root view-menu pick-menu" role="menu" aria-label=label style=style on:keydown=keys>
                <p class="st-pop-title st-phone">{title}</p>
                {parts.into_iter().map(|part| match part {
                    Part::Line => view! { <hr/> }.into_any(),
                    Part::Label(text) => view! { <p class="pick-menu-label">{text}</p> }.into_any(),
                    Part::Switch(label, on) => view! {
                        <button class="st-pop-item share-switch" type="button" role="menuitemcheckbox" aria-checked=move || if on.get() { "true" } else { "false" } on:click=move |_| on.update(|on| *on = !*on)>
                            <span class="st-pop-label"><span>{label}</span></span>
                            <span class="collect-switch" aria-hidden="true"></span>
                        </button>
                    }
                    .into_any(),
                    Part::Item(Item { icon, label, hint, note, now, key, danger, does }) => {
                        let with_hint = hint.is_some();
                        view! {
                        <button class="st-pop-item" class:danger=danger class:with-hint=with_hint type="button" role="menuitem" data-key=key.filter(|key| is_key(key)) on:click=move |_| does.run(())>
                            {icon.map(|icon| view! { <Icon name=icon/> })}
                            <span class="st-pop-label">
                                <span>{label}{now.then(|| view! { " " <span class="st-now">{t.bookmarks.now}</span> })}</span>
                                {hint.map(|hint| view! { <small>{hint}</small> })}
                            </span>
                            {note.map(|(text, class)| view! { <span class=format!("st-pop-note {class}")>{(class == "warn").then(|| view! { <Icon name="triangle-alert"/> })}{text}</span> })}
                            {key.map(|key| view! { <kbd class="st-pop-key"><KeysText text=key/></kbd> })}
                        </button>
                        }
                        .into_any()
                    }
                }).collect_view()}
            </div>
        }
        .into_any())
    }
}

/// „Einplanen" of the rows selected (konzept.html: „jedes dorthin, wohin sein „Einplanen" zielt,
/// oder alle in ein Semester"): each where its own switch would plan it, with where that is; then
/// under „Alle in" the semesters from the current one, as far as the plan reaches and at least four,
/// each with its Fachsemester and whether the modules are offered then, „jetzt" at the current one;
/// and folded, the semesters behind the student back to the first Fachsemester (owner,
/// 2026-10-18: „Betula soll nie sagen. Nein, das geht nicht"). What it plans the note says, with
/// „Rückgängig" for all.
fn plan_parts(picking: Picking, earlier: bool, t: &'static Texts) -> Option<(String, Vec<Part>)> {
    let b = &t.bookmarks;
    let plan = Studyplan::expect()?;
    let planning = Planning::expect();
    let notices = Notices::expect();
    let current = picking.current.get_untracked()?;
    let ids = picking.ids_untracked();
    if ids.is_empty() {
        return None;
    }
    let newest = picking.newest.get_untracked();
    let hint = picking.hint.get_untracked();
    let mine = MyProgram::expect();
    let (start, leave) = mine.map(|mine| (mine.start_untracked(), mine.leave())).unwrap_or_default();
    let offer = |id: &str| picking.row(id).map(|row| row.offer).unwrap_or_default();
    let plan_them = move |picks: Vec<(String, SemesterKey)>, moves: bool| {
        Callback::new(move |()| {
            picking.menu.set(None);
            picking.clear();
            plan.plan_many(picks.clone(), current, moves, move |done| {
                if let Some(notices) = notices {
                    notices.say(Notice::Planned(Box::new(done)));
                }
            });
        })
    };
    let mut parts = Vec::new();
    // Each where it aims.
    let aimed: Vec<(String, SemesterKey)> = ids.iter().map(|id| (id.clone(), aim_now(plan, planning, id, current, newest.get(id).copied(), offer(id), hint.as_ref()).target())).collect();
    let mut counts: BTreeMap<SemesterKey, usize> = BTreeMap::new();
    for (_, to) in &aimed {
        *counts.entry(*to).or_default() += 1;
    }
    let where_to = match counts.iter().collect::<Vec<_>>().as_slice() {
        [(semester, _)] => (t.marks.planned_in)(&semester.short(t.locale)),
        many => many.iter().map(|(semester, n)| (t.marks.n_in)(**n, &semester.short(t.locale))).collect::<Vec<_>>().join(", "),
    };
    parts.push(Part::Item(Item { icon: Some("calendar-plus"), label: b.each_aimed.to_string(), hint: Some(where_to), note: None, now: false, key: None, danger: false, does: plan_them(aimed, false) }));
    // All into one semester.
    let unoffered = |s: SemesterKey| ids.iter().filter(|id| !offer(id).offered(s) && newest.get(*id) != Some(&s)).count();
    let semester_item = |s: SemesterKey| {
        let fs = start.and_then(|start| fs_of(start, s, &leave)).map(|fs| (t.planner.fs_short)(fs));
        let label = match fs {
            Some(fs) => format!("{} · {fs}", s.short(t.locale)),
            None => s.short(t.locale),
        };
        let note = match (unoffered(s), ids.len()) {
            (0, _) => (b.offered.to_string(), "ok"),
            (_, 1) => (b.not_offered.to_string(), "warn"),
            (n, _) => ((b.n_not_offered)(n), "warn"),
        };
        let picks: Vec<(String, SemesterKey)> = ids.iter().map(|id| (id.clone(), s)).collect();
        Part::Item(Item { icon: None, label, hint: None, note: Some(note), now: s == current, key: None, danger: false, does: plan_them(picks, true) })
    };
    parts.push(Part::Label(b.all_into));
    let planned_last = plan.with_untracked(|doc| doc.semesters().into_iter().max());
    let last = [current.plus(3), planned_last].into_iter().flatten().max().unwrap_or(current);
    let mut s = current;
    while s <= last {
        parts.push(semester_item(s));
        let Some(next) = s.plus(1) else { break };
        s = next;
    }
    // The semesters behind the student, folded: back to the first Fachsemester (six without a
    // Studienbeginn), the latest first.
    let floor = planning.map_or(current, |planning| planning.floor(current));
    if floor < current {
        if earlier {
            parts.push(Part::Label(b.earlier_semesters));
            let mut s = current.plus(-1);
            while let Some(at) = s.filter(|at| *at >= floor) {
                parts.push(semester_item(at));
                s = at.plus(-1);
            }
        } else {
            let at = picking.menu.with_untracked(|menu| menu.as_ref().map(PickMenu::at));
            parts.push(Part::Item(Item {
                icon: Some("chevron-down"),
                label: b.earlier_semesters.to_string(),
                hint: None,
                note: None,
                now: false,
                key: None,
                danger: false,
                does: Callback::new(move |()| {
                    if let Some(at) = at {
                        picking.menu.set(Some(PickMenu::Plan { at, earlier: true }));
                    }
                }),
            }));
        }
    }
    Some(((b.plan_n)(ids.len()), parts))
}

/// „Teilen" of the rows selected: the link for another device that brings them, and the rows as
/// text, each copied, as the menu of the whole list has them.
fn share_parts(picking: Picking, t: &'static Texts) -> Option<(String, Vec<Part>)> {
    let b = &t.bookmarks;
    let notices = Notices::expect();
    let collections = Collections::expect();
    let ids = picking.ids_untracked();
    if ids.is_empty() {
        return None;
    }
    let count = ids.len();
    let done = move |link: bool| {
        picking.menu.set(None);
        picking.clear();
        if let Some(notices) = notices {
            notices.say(Notice::Copied { link, count });
        }
    };
    let mut parts = Vec::new();
    // The link of these modules, with their notes where „Notizen mitnehmen" is on: worked out as
    // it is clicked, so that the switch counts.
    let linked = ids.clone();
    parts.push(Part::Item(Item {
        icon: Some("link"),
        label: b.copy_link.to_string(),
        hint: None,
        note: None,
        now: false,
        key: None,
        danger: false,
        does: Callback::new(move |()| {
            let notes = if picking.notes_along.get_untracked() { collections.map(|collections| collections.with_untracked(|shelf| notes_of(shelf, &linked))).unwrap_or_default() } else { Vec::new() };
            let brought = Brought { ids: linked.clone(), collections: Vec::new(), notes };
            if let Some(link) = share_fragment(&brought).map(|fragment| absolute(&t.path(&format!("{}#{fragment}", url::BOOKMARKS)))) {
                copy(&link);
                done(true);
            }
        }),
    }));
    parts.push(Part::Switch(b.notes_along, picking.notes_along));
    parts.push(Part::Line);
    let text = ids.iter().filter_map(|id| picking.row(id).map(|row| module_line(id, &row.title, row.credits, t))).collect::<Vec<_>>().join("\n");
    parts.push(Part::Item(Item {
        icon: Some("copy"),
        label: b.copy_text.to_string(),
        hint: None,
        note: None,
        now: false,
        key: None,
        danger: false,
        does: Callback::new(move |()| {
            copy(&text);
            done(false);
        }),
    }));
    Some(((b.share_head)(None, count), parts))
}

/// Whether what an entry shows beside it is a key it answers to: a letter, or Shift and a letter
/// („⇧M"); „Strg Klick" is said, not pressed.
fn is_key(shown: &str) -> bool {
    let mut chars = shown.chars();
    matches!((chars.next(), chars.next(), chars.next()), (Some(_), None, _) | (Some('⇧'), Some(_), None))
}

/// A row's menu (oberflaeche.html, the row's right click): „Sammlung und Notiz …" (⇧M),
/// „Auswählen" or „Abwählen", „Einplanen" where its switch would plan it (or „Aus dem Plan
/// nehmen"), and „Merken" or „Von der Merkliste entfernen" (M).
fn row_parts(picking: Picking, id: &str, t: &'static Texts) -> Option<(String, Vec<Part>)> {
    let b = &t.bookmarks;
    let row = picking.row(id)?;
    let bookmarks = Bookmarks::expect();
    let marked = bookmarks.is_some_and(|bookmarks| bookmarks.is_marked_untracked(id));
    let selected = picking.picked.with_untracked(|picked| picked.ids.contains(id));
    let at = picking.menu.with_untracked(|menu| menu.as_ref().map(PickMenu::at));
    let collecting = Collecting::expect();
    let mut parts = Vec::new();
    if let Some(collecting) = collecting {
        let (id, title) = (id.to_string(), row.title.clone());
        parts.push(Part::Item(Item {
            icon: Some("tag"),
            label: format!("{} …", t.marks.collect),
            hint: None,
            note: None,
            now: false,
            key: Some("⇧M"),
            danger: false,
            does: Callback::new(move |()| {
                picking.menu.set(None);
                collecting.open(CollectAsk { id: id.clone(), title: title.clone(), more: Vec::new(), at, note: false, from_note: None });
            }),
        }));
    }
    {
        let id = id.to_string();
        parts.push(Part::Item(Item {
            icon: Some(if selected { "x" } else { "check" }),
            label: if selected { b.deselect } else { b.select }.to_string(),
            hint: None,
            note: None,
            now: false,
            key: Some(b.select_key),
            danger: false,
            does: Callback::new(move |()| {
                picking.menu.set(None);
                picking.toggle(&id);
            }),
        }));
    }
    if let (Some(plan), Some(planning), Some(current)) = (Studyplan::expect(), Planning::expect(), picking.current.get_untracked()) {
        let source = use_data().ok();
        let newest = picking.newest.with_untracked(|newest| newest.get(id).copied());
        let hint = picking.hint.get_untracked();
        let aim = aim_now(plan, Some(planning), id, current, newest, row.offer, hint.as_ref());
        let (label, icon) = if aim.pressed() { (b.unplan.to_string(), "calendar-minus") } else { (t.planner.plan.to_string(), "calendar-plus") };
        let what = Pressed { id: id.to_string(), title: row.title.clone(), current, offer: row.offer, newest, hint };
        let semester = aim.target().short(t.locale);
        parts.push(Part::Item(Item {
            icon: Some(icon),
            label,
            hint: Some(semester),
            note: None,
            now: false,
            key: None,
            danger: false,
            does: Callback::new(move |()| {
                picking.menu.set(None);
                planning.press(plan, source.clone(), what.clone(), aim.clone(), || {});
            }),
        }));
    }
    parts.push(Part::Line);
    {
        let (id, title) = (id.to_string(), row.title.clone());
        parts.push(Part::Item(Item {
            icon: Some(if marked { "bookmark-minus" } else { "bookmark" }),
            label: if marked { b.unmark } else { t.marks.save }.to_string(),
            hint: None,
            note: None,
            now: false,
            key: Some("M"),
            danger: marked,
            does: Callback::new(move |()| {
                picking.menu.set(None);
                if let Some(bookmarks) = bookmarks {
                    bookmarks.flip(&id, &title);
                }
            }),
        }));
    }
    Some((row.title, parts))
}

/// The page's keys of the selection, while it is in front: Ctrl or ⌘ and A selects every row the
/// list shows (not in a field, where it is the field's), Esc lets go of them where no menu or
/// dialog takes it first. Where Back's watcher has the selection (a phone's `CloseWatcher`), its
/// Esc is the watcher's.
pub(crate) fn page_keys(picking: Picking) {
    let in_front = folia_design::stage::front();
    let collecting = Collecting::expect();
    folia_shell::nav::layer_while(move || picking.any(), move || picking.clear(), folia_shell::nav::Kind::Over);
    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::keydown, move |ev| {
            if ev.default_prevented() || !in_front.get_untracked() || picking.menu.with_untracked(Option::is_some) || collecting.is_some_and(|collecting| collecting.asked_untracked().is_some()) {
                return;
            }
            let key = ev.key();
            if (ev.ctrl_key() || ev.meta_key()) && !ev.alt_key() && !ev.shift_key() && key.eq_ignore_ascii_case("a") && !typing(&ev) {
                ev.prevent_default();
                picking.select_all();
            } else if key == "Escape" && !folia_shell::nav::watched() && picking.any_untracked() {
                ev.prevent_default();
                picking.clear();
            }
        });
        on_cleanup(move || handle.remove());
    });
}

/// Whether the keys are a field's (what they type, Ctrl A there selects its text).
fn typing(ev: &leptos::ev::KeyboardEvent) -> bool {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let target = ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok());
        target.is_some_and(|target| target.closest("input, textarea, select, [contenteditable], dialog, [role=dialog], [role=menu]").ok().flatten().is_some())
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = ev;
        false
    }
}

/// The module of the row an event happened in: its link's `data-id` (the list's rows, `#rows`).
fn row_id_of(ev: &leptos::ev::Event) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let target = ev.target()?.dyn_into::<web_sys::Element>().ok()?;
        let wrap = target.closest(".row-wrap").ok()??;
        wrap.query_selector("a.row[data-id]").ok()??.get_attribute("data-id")
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = ev;
        None
    }
}

/// Whether the event happened at a row's link (the row itself), not at its bookmark.
fn on_link(ev: &leptos::ev::Event) -> bool {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).is_some_and(|target| target.matches("a.row").unwrap_or(false))
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = ev;
        false
    }
}

/// Whether the event happened on a control of a row other than its link (its bookmark).
fn on_control(ev: &leptos::ev::Event) -> bool {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let Some(target) = ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()) else { return false };
        target.closest(".row-wrap button, .row-wrap input").ok().flatten().is_some()
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = ev;
        false
    }
}

/// The module of the row at (`x`, `y`) on the screen.
#[allow(unused_variables)]
fn row_at(x: f64, y: f64) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        let document = web_sys::window()?.document()?;
        #[allow(clippy::cast_possible_truncation)]
        let element = document.element_from_point(x as f32, y as f32)?;
        let wrap = element.closest(".rows .row-wrap").ok()??;
        wrap.query_selector("a.row[data-id]").ok()??.get_attribute("data-id")
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The module of the list's row nearest to the height `y`: for a pointer that left the rows (above
/// the first, below the last) the first or the last.
#[allow(unused_variables)]
fn row_nearest(y: f64) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        let mut best: Option<(f64, String)> = None;
        for link in folia_design::stage::select_all(&format!("#{} a.row[data-id]", crate::bookmarks::ROWS_ID)) {
            let r = link.get_bounding_client_rect();
            if r.height() <= 0.0 {
                continue;
            }
            let distance = if y < r.top() { r.top() - y } else if y > r.bottom() { y - r.bottom() } else { 0.0 };
            if best.as_ref().is_none_or(|(d, _)| distance < *d) {
                best = link.get_attribute("data-id").map(|id| (distance, id));
            }
        }
        best.map(|(_, id)| id)
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Holds on to the pointer for the list: its moves come there while the button is down, wherever
/// it goes.
#[allow(unused_variables)]
fn capture(ev: &PointerEvent) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        if let Some(element) = ev.current_target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()) {
            let _ = element.set_pointer_capture(ev.pointer_id());
        }
    }
}

/// The focus to the row of module `id`, brought into view where it is not.
#[allow(unused_variables)]
fn focus_row(id: &str) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let selector = format!("#{} a.row[data-id=\"{}\"]", crate::bookmarks::ROWS_ID, id.replace('"', ""));
        let Some(link) = folia_design::stage::select(&selector).and_then(|link| link.dyn_into::<web_sys::HtmlElement>().ok()) else { return };
        let options = web_sys::FocusOptions::new();
        options.set_prevent_scroll(true);
        let _ = link.focus_with_options(&options);
        let place = web_sys::ScrollIntoViewOptions::new();
        place.set_block(web_sys::ScrollLogicalPosition::Nearest);
        link.scroll_into_view_with_scroll_into_view_options(&place);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| id.to_string()).collect()
    }

    fn set(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|id| id.to_string()).collect()
    }

    /// A drawn selection takes in the marked rows it passes, and the one it began at; begun on a
    /// selected row it lets go of them.
    #[test]
    fn a_drawn_selection_takes_in_the_marked_rows_it_passes() {
        let order = ids(&["a", "b", "c", "d", "e"]);
        let marked = |id: &str| id != "c";
        let paint = Paint { start: 1, select: true, base: set(&["e"]) };
        assert_eq!(painted(&order, &paint, 3, marked), set(&["b", "d", "e"]));
        // Back above where it began.
        assert_eq!(painted(&order, &paint, 0, marked), set(&["a", "b", "e"]));
        // Begun on an unmarked row: that one is taken in, being chosen itself.
        let on_dimmed = Paint { start: 2, select: true, base: BTreeSet::new() };
        assert_eq!(painted(&order, &on_dimmed, 2, marked), set(&["c"]));
        let letting_go = Paint { start: 1, select: false, base: set(&["a", "b", "c", "d"]) };
        assert_eq!(painted(&order, &letting_go, 2, marked), set(&["a", "d"]));
    }

    #[test]
    fn shift_and_a_click_takes_in_the_rows_from_the_last_one() {
        let order = ids(&["a", "b", "c", "d"]);
        assert_eq!(range(&order, "d", "a", |id| id != "b"), ids(&["a", "c", "d"]));
        // Clicked itself, an unmarked row is taken in; one the list no longer has is no range.
        assert_eq!(range(&order, "a", "b", |id| id != "b"), ids(&["a", "b"]));
        assert_eq!(range(&order, "gone", "c", |_| true), ids(&["c"]));
    }

    /// Shift and ↓ from a row takes in the next, Shift and ↑ back lets it go again, and on past
    /// where it began takes in the other way.
    #[test]
    fn shift_and_the_arrows_extend_the_selection_from_where_it_began() {
        let order = ids(&["a", "b", "c", "d"]);
        let none = Picked::default();
        let (to, picked) = stepped(&order, &none, "b", 1).unwrap();
        assert_eq!((to.as_str(), picked.ids.clone(), picked.anchor.as_deref()), ("c", set(&["b", "c"]), Some("b")));
        let (to, picked) = stepped(&order, &picked, "c", 1).unwrap();
        assert_eq!((to.as_str(), picked.ids.clone()), ("d", set(&["b", "c", "d"])));
        // Back towards where it began: the last one goes.
        let (to, picked) = stepped(&order, &picked, "d", -1).unwrap();
        assert_eq!((to.as_str(), picked.ids.clone()), ("c", set(&["b", "c"])));
        let (_, picked) = stepped(&order, &picked, "c", -1).unwrap();
        let (to, picked) = stepped(&order, &picked, "b", -1).unwrap();
        assert_eq!((to.as_str(), picked.ids.clone()), ("a", set(&["a", "b"])));
        // Not past the ends.
        assert_eq!(stepped(&order, &picked, "a", -1), None);
        assert_eq!(stepped(&order, &picked, "d", 1).map(|(to, _)| to), None);
    }
}
