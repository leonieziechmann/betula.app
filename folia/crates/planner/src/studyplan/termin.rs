//! A Termin beside the plan in its own view (`termin=<key>&on=<day>`, folia/design/termin; owner,
//! 2026-10-26): the view that adjusts the plan (`folia_widgets::termin`), of the plan's timetable.
//! A click on a Termin of the week or of „Termine" opens it in the place of the module's panel,
//! and „Zum Modul" opens that panel in its place, pointing at the Termin; closing it goes back a
//! step where the plan without it is the step before, as the module's panel does. „Ganze Seite"
//! (`&full=1`, owner: „bau als Nächstes Teilen und Ganze Seite") lets it fill the plan's page, as
//! „Vollbild" does a module: the same view in two columns, still the one that adjusts, with
//! „Zurück" to it beside the plan.

use folia_calendar::day::Day;
use folia_calendar::rowkey::RowKey;
use folia_design::stage::Title;
use folia_routes::url::PlanAddress;
use folia_shell::frame::Plain;
use folia_shell::pending::same_page;
use folia_shell::tabs::{Area, Tabs};
use folia_widgets::termin::{module_facts, page_head, ticking_clock, Against, TerminCtx, TerminLinks, TerminMode, TerminPlace, TerminView};
use leptos::prelude::*;

use crate::i18n;
use super::PlanCtx;

/// The address of the plan with the Termin on a day beside it, or (`full`) filling the page.
fn termin_path(ctx: PlanCtx, (key, on): (RowKey, Option<Day>), full: bool) -> String {
    ctx.url.with(|url| PlanAddress { url: url.clone(), full: false }.with_termin(Some((key, on)), full).path())
}

#[component]
pub(super) fn PlanTerminPanel(ctx: PlanCtx) -> impl IntoView {
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
        place: TerminPlace::Beside,
    };
    view! { <TerminView ctx=view/> }
}

/// The Termin filling the plan's page („Ganze Seite", `…&full=1`): its dates and groups lead to
/// their Termin filling the page too, „Zurück" (Esc) to the Termin beside the plan on the day
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
