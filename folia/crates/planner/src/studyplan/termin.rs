//! A Termin of the plan in its own view (`termin=<key>&on=<day>`, folia/design/termin; owner,
//! 2026-10-26): the view that adjusts the plan (`folia_widgets::termin`), of the plan's timetable,
//! in the app's dialog over the plan (owner, 2026-11-03: „muss das ein modal werden"). A click on a
//! Termin of the week or of „Termine" opens it, „Zum Modul" closes it and opens the module's panel
//! beside the plan, pointing at the Termin; closing it goes back a step where the plan without it
//! is the step before, as the module's panel does. „Ganze Seite" (`&full=1`, owner: „bau als
//! Nächstes Teilen und Ganze Seite") lets it fill the plan's page, as „Vollbild" does a module: the
//! same view in two columns, still the one that adjusts, with „Zurück" to it over the plan.

use folia_calendar::day::Day;
use folia_calendar::rowkey::RowKey;
use folia_design::stage::Title;
use folia_routes::url::PlanAddress;
use folia_shell::frame::Plain;
use folia_shell::pending::same_page;
use folia_shell::tabs::{Area, Tabs};
use folia_widgets::termin::{module_facts, page_head, ticking_clock, Against, ChangeActs, OwnActs, TerminCtx, TerminLinks, TerminMode, TerminModal, TerminPlace, TerminView};
use leptos::prelude::*;

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

/// The Termin over the plan in the app's dialog, while `open` (the address asks for it, and not
/// for it filling the page).
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
    let links = TerminLinks { termin, module, close, back, adjust: None, full: Some(full) };
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
    };
    view! { <TerminModal ctx=view open/> }
}

/// The Termin filling the plan's page („Ganze Seite", `…&full=1`): its dates and groups lead to
/// their Termin filling the page too, „Zurück" (Esc) to the Termin over the plan on the day
/// shown. The tab, the area and the history stay the Stundenplan's.
#[component]
pub(super) fn PlanTerminFull(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let asked = Memo::new(move |_| ctx.url.with(|url| url.termin_on()));
    let modules = Memo::new(move |_| ctx.data.with(|data| data.as_ref().ok().map(module_facts).unwrap_or_default()));
    let close = Signal::derive(move || ctx.url.with(|url| t.path(&url.path())));
    let termin = Callback::new(move |at: (RowKey, Option<Day>)| t.path(&termin_path(ctx, at, true)));
    let module = Callback::new(move |(id, key): (String, Option<RowKey>)| ctx.url.with(|url| t.path(&url.with_open(Some(&id), key).path())));
    let links = TerminLinks { termin, module, close, back: Signal::stored(None), adjust: None, full: None };
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
