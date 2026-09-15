use std::collections::HashSet;
use leptos::prelude::*;
use crate::models::{ModuleCardItem, PrereqStatus};
use crate::db::evaluate_prerequisites;

/// Reusable table row for displaying a module item with badges, prereq evaluation, and actions
#[component]
pub fn ModuleTableRow(
    item: ModuleCardItem,
    is_completed: bool,
    is_bookmarked: bool,
    show_fues_badge: bool,
    completed_set: HashSet<String>,
    on_open: Callback<String>,
    on_toggle_bookmark: Callback<String>,
    on_toggle_completed: Callback<String>,
) -> impl IntoView {
    let id = item.id.clone();
    let id_c = item.id.clone();
    let id_b = item.id.clone();
    let id_m = item.id.clone();

    let (status, missing_m, missing_r) = evaluate_prerequisites(
        item.prerequisites_mandatory.as_deref(),
        item.prerequisites_recommended.as_deref(),
        &completed_set,
    );

    let successors = item.successor_list();

    let row_cls = {
        let mut c = "table-row".to_string();
        if is_completed { c.push_str(" is-completed"); }
        if is_bookmarked { c.push_str(" is-bookmarked"); }
        c
    };

    view! {
        <tr
            class=row_cls
            on:click=move |_| on_open.run(id_m.clone())
        >
            <td style="text-align: center;" on:click=move |ev| ev.stop_propagation()>
                <div class="table-actions">
                    <button
                        type="button"
                        class=if is_bookmarked { "action-btn-sm btn-star active" } else { "action-btn-sm btn-star" }
                        on:click=move |_| on_toggle_bookmark.run(id_b.clone())
                        title="Merken"
                    >
                        "★"
                    </button>
                    <button
                        type="button"
                        class=if is_completed { "action-btn-sm btn-check active" } else { "action-btn-sm btn-check" }
                        on:click=move |_| on_toggle_completed.run(id_c.clone())
                        title="Bestanden"
                    >
                        "✓"
                    </button>
                </div>
            </td>
            <td><span class="pill-id">"#" {id.clone()}</span></td>
            <td>
                <div class="table-title">{item.title_de.clone()}</div>
                {item.title_en.as_ref().filter(|t| !t.trim().is_empty() && *t != &item.title_de).map(|en| view! {
                    <div class="table-subtitle">{en.clone()}</div>
                })}
                <div class="table-badges-inline">
                    {if show_fues_badge {
                        view! { <span class="pill-badge pill-fues">"FÜS"</span> }.into_any()
                    } else {
                        view! {}.into_any()
                    }}

                    {if item.is_limited() {
                        view! { <span class="pill-badge pill-nc">"🔒 NC"</span> }.into_any()
                    } else { view! {}.into_any() }}

                    {if item.is_not_offered.unwrap_or(0) == 1 {
                        view! { <span class="pill-badge pill-not-offered" style="background: #fef2f2; color: #b91c1c; border: 1px solid #fecaca;">"🚫 Nicht im Angebot"</span> }.into_any()
                    } else if item.is_phase_out.unwrap_or(0) == 1 {
                        view! { <span class="pill-badge pill-phaseout">"⏳ Auslauf"</span> }.into_any()
                    } else { view! {}.into_any() }}

                    {if !successors.is_empty() {
                        view! {
                            <span class="pill-badge" style="background: #eff6ff; color: #1d4ed8; border: 1px solid #bfdbfe;">
                                {format!("➡️ Nachfolge: {}", successors[0])}
                            </span>
                        }.into_any()
                    } else { view! {}.into_any() }}

                    {if let Some(sem) = item.recommended_semester {
                        let sem_text = if sem > 0 { format!("{}. Semester", sem) } else { "Wahlbereich".to_string() };
                        view! {
                            <span class="pill-badge" style="background: #e0f2fe; color: #0369a1; border: 1px solid #bae6fd; font-weight: 600;">
                                {"📋 "}{sem_text}
                            </span>
                        }.into_any()
                    } else { view! {}.into_any() }}

                    {if let Some(ref mt) = item.module_type {
                        view! {
                            <span class="pill-badge" style="background: #f1f5f9; color: #334155; border: 1px solid #e2e8f0;">
                                {mt.clone()}
                            </span>
                        }.into_any()
                    } else { view! {}.into_any() }}
                </div>
            </td>
            <td style="text-align: center;">
                <span class="pill-ects">{item.formatted_credits()} " ECTS"</span>
            </td>
            <td>
                <span class="meta-tag meta-turnus">{item.formatted_turnus()}</span>
            </td>
            <td style="text-align: center;">
                <span class="meta-tag meta-lang">{item.formatted_lang()}</span>
            </td>
            <td>
                {match status {
                    PrereqStatus::Met => view! {
                        <span class="prereq-pill prereq-met">"🟢 OK"</span>
                    }.into_any(),
                    PrereqStatus::RecommendedMissing(_) => view! {
                        <span class="prereq-pill prereq-rec prereq-rec-missing" title=missing_r.join(", ")>
                            "🟡 Empf."
                        </span>
                    }.into_any(),
                    PrereqStatus::Missing(_) => view! {
                        <span class="prereq-pill prereq-miss prereq-missing" title=missing_m.join(", ")>
                            {format!("🔴 Fehlt: {}", missing_m.get(0).unwrap_or(&String::new()))}
                        </span>
                    }.into_any(),
                    PrereqStatus::None => view! {
                        <span class="table-muted-dash">"-"</span>
                    }.into_any(),
                }}
            </td>
            <td style="text-align: center;">
                {if let Some(cnt) = item.events_count {
                    if cnt > 0 {
                        view! {
                            <span class="bottom-item bottom-events">{format!("📅 {}", cnt)}</span>
                        }.into_any()
                    } else {
                        view! { <span class="table-muted-dash">"-"</span> }.into_any()
                    }
                } else {
                    view! { <span class="table-muted-dash">"-"</span> }.into_any()
                }}
            </td>
            <td style="text-align: center;">
                <span class="card-open-arrow">"↗"</span>
            </td>
        </tr>
    }
}

/// Generic, sortable ModuleTable replacing duplicated table implementations
#[component]
pub fn ModuleTable(
    modules: Signal<Vec<ModuleCardItem>>,
    completed_modules: Signal<HashSet<String>>,
    bookmarked_modules: Signal<HashSet<String>>,
    sort_by: Signal<String>,
    sort_asc: Signal<bool>,
    on_sort: Callback<String>,
    on_open_module: Callback<String>,
    on_toggle_bookmark: Callback<String>,
    on_toggle_completed: Callback<String>,
    #[prop(default = false)] show_fues_badge: bool,
    #[prop(optional, into)] tbody_id: Option<&'static str>,
) -> impl IntoView {
    let th_class = move |col: &'static str| {
        let cur = sort_by.get();
        if cur == col {
            if sort_asc.get() { "th-sortable sorted-asc" } else { "th-sortable sorted-desc" }
        } else {
            "th-sortable"
        }
    };

    let sort_arrow = move |col: &'static str| {
        let cur = sort_by.get();
        if cur == col {
            if sort_asc.get() { "▲" } else { "▼" }
        } else {
            "↕"
        }
    };

    view! {
        <div class="table-container">
            <table class="module-table">
                <thead>
                    <tr>
                        <th style="width: 68px; text-align: center;">"Status"</th>
                        <th
                            class=move || th_class("id")
                            style="width: 75px;"
                            on:click=move |_| on_sort.run("id".to_string())
                        >
                            <div class="th-content">"ID " <span class="sort-indicator">{move || sort_arrow("id")}</span></div>
                        </th>
                        <th
                            class=move || th_class("title")
                            on:click=move |_| on_sort.run("title".to_string())
                        >
                            <div class="th-content">"Modulname & Lehrveranstaltung " <span class="sort-indicator">{move || sort_arrow("title")}</span></div>
                        </th>
                        <th
                            class=move || th_class("ects")
                            style="width: 85px; text-align: center;"
                            on:click=move |_| on_sort.run("ects".to_string())
                        >
                            <div class="th-content th-content-center">"ECTS " <span class="sort-indicator">{move || sort_arrow("ects")}</span></div>
                        </th>
                        <th style="width: 120px;">"Turnus"</th>
                        <th style="width: 65px; text-align: center;">"Sprache"</th>
                        <th style="width: 140px;">"Voraussetzungen"</th>
                        <th
                            class=move || th_class("events")
                            style="width: 90px; text-align: center;"
                            on:click=move |_| on_sort.run("events".to_string())
                        >
                            <div class="th-content th-content-center">"Termine " <span class="sort-indicator">{move || sort_arrow("events")}</span></div>
                        </th>
                        <th style="width: 36px;"></th>
                    </tr>
                </thead>
                <tbody id=tbody_id.unwrap_or("module-table-body")>
                    {move || {
                        let list = modules.get();
                        let comp = completed_modules.get();
                        let bkmk = bookmarked_modules.get();

                        list.into_iter().map(|item| {
                            let id = item.id.clone();
                            let is_c = comp.contains(&id);
                            let is_b = bkmk.contains(&id);

                            view! {
                                <ModuleTableRow
                                    item=item
                                    is_completed=is_c
                                    is_bookmarked=is_b
                                    show_fues_badge=show_fues_badge
                                    completed_set=comp.clone()
                                    on_open=on_open_module
                                    on_toggle_bookmark=on_toggle_bookmark
                                    on_toggle_completed=on_toggle_completed
                                />
                            }
                        }).collect::<Vec<_>>()
                    }}
                </tbody>
            </table>
        </div>
    }
}
