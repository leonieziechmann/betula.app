//! A Termin of the plan in its own view (`termin=<key>&on=<day>`, folia/design/termin; owner,
//! 2026-10-26), of the plan's timetable. A click on a Termin of the week or of „Termine" opens the
//! card of that one Termin (`folia_widgets::termin::card`; owner, 2026-11-17: „mehr wie bei google
//! calendar") in the app's dialog over the plan (owner, 2026-11-03: „muss das ein modal werden"),
//! beside the slot it was opened from; a click on another slot opens that slot's card in its place,
//! and closing it goes back a step where the plan without it is the step before, as the module's
//! panel does. The view with the module's events, groups and „Reihe" is the overview (`…&full=1`,
//! „Alle Termine von MIT1"), which fills the plan's page as „Vollbild" does a module (owner: „bau
//! als Nächstes Teilen und Ganze Seite"); its „Zurück" leads to the plan, never to the card.

use folia_calendar::day::Day;
use folia_calendar::rowkey::RowKey;
use folia_design::stage::Title;
use folia_routes::url::PlanAddress;
use folia_shell::frame::Plain;
use folia_shell::pending::same_page;
use folia_shell::tabs::{Area, Tabs};
use folia_shell::pending::Pending;
use folia_stores::collections::Anchor;
use folia_widgets::termin::{module_facts, page_head, ticking_clock, Against, ChangeActs, OwnActs, TerminCtx, TerminLinks, TerminMode, TerminModal, TerminPlace, TerminUndo, TerminView};
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use crate::i18n;
use super::PlanCtx;

/// The address of the plan with the Termin on a day beside it, or (`full`) filling the page.
fn termin_path(ctx: PlanCtx, (key, on): (RowKey, Option<Day>), full: bool) -> String {
    ctx.url.with(|url| PlanAddress { url: url.clone(), full: false }.with_termin(Some((key, on)), full).path())
}

/// What the view does with a Termin of one's own here: „Bearbeiten" opens its form over the plan,
/// „Löschen" takes it away with „Rückgängig" (`own.rs`).
fn own_acts(ctx: PlanCtx) -> OwnActs {
    OwnActs { edit: Callback::new(move |number| super::own::ask_edit(ctx, number)), remove: Callback::new(move |number| super::own::remove(ctx, number)) }
}

/// What the view adds to „Ändern" of a Termin of QIS here: what the change meets of the plan's
/// week, and „Nachholtermin", which opens the form of a Termin of one's own (`own.rs`).
fn change_acts(ctx: PlanCtx) -> ChangeActs {
    ChangeActs { fit: Callback::new(move |change| super::own::change_fit(ctx, change)), makeup: Callback::new(move |(key, day)| super::own::ask_makeup(ctx, key, day)) }
}

/// „Vergleichen ›" of a group: its event's choice in focus, the Termin closed (`with_wahl` enters the
/// mode).
fn choice_link(ctx: PlanCtx) -> Callback<String, String> {
    let t = i18n::t();
    Callback::new(move |code: String| ctx.url.with(|url| t.path(&url.with_wahl(Some(&code)).with_vergleich(true).path())))
}

/// The card hid or ticked a Termin and closed. While the plan is looked at the page's note says so
/// with „Rückgängig" (`own::OwnNote`, the note of what was done to a Termin, one's own or QIS's;
/// `PlanCtx::undo` keeps what the card did, to take back). While it is edited the slot itself answers — full,
/// faint — and nothing is said.
fn done(ctx: PlanCtx) -> Callback<TerminUndo> {
    Callback::new(move |undo: TerminUndo| {
        if !ctx.edit.get_untracked() {
            ctx.undo.set(Some((format!("{}{}", super::own::OWN_NOTE, undo.said), undo.undo)));
        }
    })
}

/// The slot the address marks in the week of a wide page (`aria-current`, `week::picked_of`), on
/// this page's stage (R19): its day's column from left to right (`div.col`,
/// `folia_widgets::week`), itself from top to bottom. `None` where the week does not show it:
/// another week of A and B, „Termine" and „Prüfungen", a Termin the plan leaves out while it is
/// looked at, a page loaded with the Termin in its address (the week comes after the card).
#[allow(clippy::unnecessary_wraps)]
fn slot_box() -> Option<Anchor> {
    #[cfg(feature = "csr")]
    {
        let slot = folia_design::stage::select(".sp-week .week a.slot[aria-current=\"true\"]")?;
        let of = slot.get_bounding_client_rect();
        let column = slot.closest(".col").ok().flatten().map(|column| column.get_bounding_client_rect());
        let (left, right) = column.map_or((of.left(), of.right()), |column| (column.left(), column.right()));
        (of.width() > 0.0).then(|| Anchor { left, top: of.top(), right, bottom: of.bottom() })
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The address of the slot of the week under a point of the window, where it is another Termin's:
/// a click beside the card on another slot opens that slot's card in this one's place (owner,
/// 2026-11-17: „mehr wie bei google calendar").
fn other_slot(ctx: PlanCtx, (x, y): (f64, f64)) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        let asked = ctx.url.with_untracked(|url| url.termin_on()).map(|(key, _)| key);
        folia_design::stage::select_all(".sp-week .week a.slot[href]").into_iter().find_map(|slot| {
            let of = slot.get_bounding_client_rect();
            if x < of.left() || x > of.right() || y < of.top() || y > of.bottom() {
                return None;
            }
            let href = slot.get_attribute("href")?;
            let (key, _) = folia_routes::url::StudyplanUrl::parse(href.split_once('?')?.1).termin_on()?;
            (Some(key) != asked).then_some(href)
        })
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = (ctx, x, y);
        None
    }
}

/// The card of a Termin over the plan in the app's dialog, while `open` (the address asks for it,
/// and not for it filling the page): beside its slot, and another slot's card in its place on a
/// click there.
#[component]
pub(super) fn PlanTerminModal(ctx: PlanCtx, #[prop(into)] open: Signal<bool>) -> impl IntoView {
    let t = i18n::t();
    let asked = Memo::new(move |_| ctx.url.with(|url| url.termin_on()));
    let modules = Memo::new(move |_| ctx.data.with(|data| data.as_ref().ok().map(module_facts).unwrap_or_default()));
    let tabs = Tabs::expect();
    let close = Signal::derive(move || ctx.url.with(|url| t.path(&url.with_termin(None).path())));
    let back = Signal::derive(move || {
        let page = ctx.url.with(|url| url.with_termin(None).path());
        let now = ctx.url.with(|url| url.path());
        tabs.is_some_and(|tabs| same_page(&tabs.before(&now), &page)).then_some("history")
    });
    let termin = Callback::new(move |at: (RowKey, Option<Day>)| t.path(&termin_path(ctx, at, false)));
    let module = Callback::new(move |(id, key): (String, Option<RowKey>)| ctx.url.with(|url| t.path(&url.with_open(Some(&id), key).path())));
    let full = Callback::new(move |at: (RowKey, Option<Day>)| t.path(&termin_path(ctx, at, true)));
    let links = TerminLinks { termin, module, close, back, adjust: None, full: Some(full), choice: Some(choice_link(ctx)) };
    let view = TerminCtx {
        table: ctx.table,
        asked,
        modules,
        mode: TerminMode::Adjust,
        plan: ctx.plan,
        now: ticking_clock(),
        planned: Signal::stored(Some(true)),
        against: Signal::stored(Against::Itself),
        links,
        place: TerminPlace::Dialog,
        own: Some(own_acts(ctx)),
        change: Some(change_acts(ctx)),
        done: Some(done(ctx)),
    };
    // Taken while the component is built: a listener has no owner to ask.
    let going = Pending::expect();
    let beside = Callback::new(move |at: (f64, f64)| match (going, other_slot(ctx, at)) {
        (Some(going), Some(to)) => {
            // In the card's place: Back still leaves to the plan in one step.
            going.go(&to, NavigateOptions { replace: true, scroll: false, ..Default::default() });
            true
        }
        _ => false,
    });
    view! { <TerminModal ctx=view open anchor=Callback::new(|()| slot_box()) beside/> }
}

/// The overview of a Termin filling the plan's page („Alle Termine von MIT1", `…&full=1`): its
/// dates and groups lead to their Termin filling the page too, „Zurück" (Esc) to the plan — a step
/// back where the plan is the step before, else in its place — never to the card (`edit` and `view`
/// stay as they were). The tab, the area and the history stay the Stundenplan's.
#[component]
pub(super) fn PlanTerminFull(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let asked = Memo::new(move |_| ctx.url.with(|url| url.termin_on()));
    let modules = Memo::new(move |_| ctx.data.with(|data| data.as_ref().ok().map(module_facts).unwrap_or_default()));
    let close = Signal::derive(move || ctx.url.with(|url| t.path(&url.with_termin(None).path())));
    let termin = Callback::new(move |at: (RowKey, Option<Day>)| t.path(&termin_path(ctx, at, true)));
    let module = Callback::new(move |(id, key): (String, Option<RowKey>)| ctx.url.with(|url| t.path(&url.with_open(Some(&id), key).path())));
    let links = TerminLinks { termin, module, close, back: Signal::stored(None), adjust: None, full: None, choice: Some(choice_link(ctx)) };
    let now = ticking_clock();
    let view = TerminCtx {
        table: ctx.table,
        asked,
        modules,
        mode: TerminMode::Adjust,
        plan: ctx.plan,
        now,
        planned: Signal::stored(Some(true)),
        against: Signal::stored(Against::Itself),
        links,
        place: TerminPlace::Full(Area::Studyplan),
        own: Some(own_acts(ctx)),
        change: Some(change_acts(ctx)),
        done: None,
    };
    // The tab's title names the Termin on the day shown.
    let title = Memo::new(move |_| {
        let now = now.get();
        ctx.table.with(|table| asked.with(|asked| page_head(table.as_ref()?, (*asked)?, now, t.locale, &t.termin))).map(|head| head.title)
    });
    view! {
        <Plain class="tm-plain">
            {move || title.get().map(|text| view! { <Title text/> })}
            <div class="page-inner">
                <TerminView ctx=view/>
            </div>
        </Plain>
    }
}
