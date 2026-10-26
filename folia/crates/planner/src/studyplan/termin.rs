//! A Termin beside the plan in its own view (`termin=<key>&on=<day>`, folia/design/termin; owner,
//! 2026-10-26): the view that adjusts the plan (`folia_widgets::termin`), of the plan's timetable.
//! A click on a Termin of the week or of „Termine" opens it in the place of the module's panel,
//! and „Zum Modul" opens that panel in its place, pointing at the Termin; closing it goes back a
//! step where the plan without it is the step before, as the module's panel does.

use folia_calendar::day::Day;
use folia_calendar::rowkey::RowKey;
use folia_shell::pending::same_page;
use folia_shell::tabs::Tabs;
use folia_widgets::termin::{module_facts, ticking_clock, TerminCtx, TerminLinks, TerminMode, TerminView};
use leptos::prelude::*;

use crate::i18n;
use super::PlanCtx;

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
    let termin = Callback::new(move |(key, on): (RowKey, Option<Day>)| ctx.url.with(|url| t.path(&url.with_termin(Some((key, on))).path())));
    let module = Callback::new(move |(id, key): (String, Option<RowKey>)| ctx.url.with(|url| t.path(&url.with_open(Some(&id), key).path())));
    let links = TerminLinks { termin, module, close, back };
    let view = TerminCtx { table: ctx.table, asked, modules, mode: TerminMode::Adjust, plan: ctx.plan, now: ticking_clock(), links };
    view! { <TerminView ctx=view/> }
}
