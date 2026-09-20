//! The module catalog: filters, list and (when a module is open) its preview.
//!
//! The URL is the whole state: `/catalog?…` is the list, `…&open=<id>` the same list with that
//! module previewed next to it (its own page is `/catalog/module/<id>`). The filter controls are
//! links to the list they lead to, so the page works without JavaScript; the browser app adds
//! what links cannot do (pickers with a search, the credit slider).

use std::sync::Arc;

use catalog::filter::{CatalogQuery, ExamPart, KindFilter, Language, PlanSemesterFilter, ProgramRelation, ProgramScope, SortKey};
use catalog::labels::{Campus, Code, Labelled, ModuleKind, OfferStatus, TeachingForm, TurnusParity, TurnusSeason};
use catalog::pages::{self, CatalogChoices, CatalogData};
use catalog::rows::{CatalogRow, Program};
use catalog::url::{self, CatalogUrl, ProgramTab, PAGE_SIZE};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;

use crate::combobox::{ClosePopups, ComboItem, Combobox};
use crate::data::{use_source, PageStatus};
use crate::format;
use crate::nav::{self, keep_position_after_prepend, list_height, list_position};
use crate::pages::module::ModulePanel;
use crate::ui::{ErrorState, Hit, Icon, KindBadge, OfferBadge};

#[component]
pub fn CatalogPage() -> impl IntoView {
    let location = use_location();
    let url = Memo::new(move |_| CatalogUrl::parse(&location.search.get()));
    // Three independent parts of the URL. The filter decides what the list is; `page` only says
    // where the visitor is in it (the list scrolls endlessly and keeps `page` up to date); `open`
    // is the preview. So scrolling and opening a preview re-render neither list nor filters.
    let list_query = Memo::new(move |_| url.get().query);
    let page = Memo::new(move |_| url.get().page);
    let open = Memo::new(move |_| url.get().open);
    let source = use_source();
    let status = PageStatus::capture();

    let list_source = source.clone();
    let list = Memo::new(move |_| {
        // Start at the page the URL names at this moment; later page changes are scrolling.
        let current = CatalogUrl { query: list_query.get(), page: page.get_untracked(), open: None };
        list_source.clone().and_then(|source| source.run(|db| pages::catalog(db, &current))).map(|data| (current, data))
    });
    // The filter panel is rendered once and follows these; only the list is rendered per filter.
    let failed = Memo::new(move |_| list.with(|list| list.as_ref().err().cloned()));
    let facts = Memo::new(move |_| list.with(|list| list.as_ref().map(|(_, data)| Facts::of(data)).unwrap_or_default()));
    // What the pickers offer does not depend on the filter: loaded once, not with every list.
    let choices_source = source.clone();
    let choices = Memo::new(move |_| {
        let loaded = choices_source.clone().and_then(|source| source.run(pages::catalog_choices));
        loaded.map(|choices| Choices::of(&choices)).unwrap_or_default()
    });

    // On a phone a module opens as its own page, never as a preview (the preview would fill the
    // screen anyway, and the page has a history entry of its own to come back from).
    let phone = RwSignal::new(nav::is_phone());
    let navigate = use_navigate();
    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::resize, move |_| {
            if phone.get_untracked() != nav::is_phone() {
                phone.set(nav::is_phone());
            }
        });
        on_cleanup(move || handle.remove());
    });
    Effect::new(move |_| {
        if let (true, Some(id)) = (phone.get(), open.get()) {
            navigate(&url::module_path(&id), NavigateOptions { replace: true, ..Default::default() });
        }
    });
    // Coming back from a module's page, the list shows the row the visitor left it at: the
    // previewed module, or on a phone the row that was tapped. Only the first list of this visit
    // does that; a filter change starts at the top as always.
    let come_back_to = StoredValue::new(open.get_untracked().or_else(nav::recall_row));
    let preview = Memo::new(move |_| match open.get() {
        None => Ok(None),
        Some(id) => source.clone().and_then(|source| source.run(|db| pages::module(db, &id))).map(Some),
    });

    let title = move || match list.get() {
        Ok((_, data)) => match &data.program {
            Some(p) => format!("Module · {} {}", p.name, p.degree()),
            None => "Modulkatalog".to_string(),
        },
        Err(_) => "Modulkatalog".to_string(),
    };

    view! {
        <Title text=title/>
        <div class="work" class:no-detail=move || open.get().is_none()>
            {move || match failed.get() {
                Some(error) => {
                    status.for_error(&error);
                    view! { <div class="page"><ErrorState error/></div> }.into_any()
                }
                None => view! {
                    <Filters query=list_query facts choices open/>
                    // The handle for the panel's width sits in the gap between the two boxes.
                    <div class="resizer between js-only" data-action="resize-filters" role="separator" aria-orientation="vertical" aria-controls="filters" aria-label="Breite der Filter ändern (Pfeiltasten, Doppelklick setzt zurück)" tabindex="0"></div>
                    {move || list.get().ok().map(|(current, data)| {
                        let reveal = come_back_to.try_update_value(Option::take).flatten();
                        view! { <List current data open page phone reveal/> }
                    })}
                }.into_any(),
            }}
            {move || {
                let close_href = url.get().with_open(None).path();
                match preview.get() {
                    Ok(None) | Err(_) => ().into_any(),
                    Ok(Some(Some(data))) => view! {
                        <ModulePanel data close_href/>
                        // Its handle is a sibling, not a child: the panel would clip the part in front of its edge.
                        <div class="resizer preview-edge js-only" data-action="resize-preview" role="separator" aria-orientation="vertical" aria-controls="preview" aria-label="Breite der Vorschau ändern (Pfeiltasten, Doppelklick setzt zurück)" tabindex="0"></div>
                    }.into_any(),
                    Ok(Some(None)) => view! {
                        <section class="panel detail">
                            <div class="state">
                                <p class="state-title">"Modul nicht gefunden"</p>
                                <p>"Dieses Modul steht nicht (mehr) im Modulkatalog der BTU."</p>
                                <a class="btn secondary" href=close_href>"Vorschau schließen"</a>
                            </div>
                        </section>
                    }.into_any(),
                }
            }}
        </div>
    }
}

/// A link that keeps whatever preview is open at the time it is followed.
fn keep_open(target: CatalogUrl, open: Memo<Option<String>>) -> impl Fn() -> String + Clone + Send + Sync + 'static {
    move || target.with_open(open.get().as_deref()).path()
}

/// The active filters as removable tags: (group, value, the list without it).
fn tags(current: &CatalogUrl, data: &CatalogData) -> Vec<(String, String, CatalogUrl)> {
    let q = &current.query;
    let mut out: Vec<(String, String, CatalogUrl)> = Vec::new();
    let mut push = |group: &str, value: String, change: &dyn Fn(&mut CatalogQuery)| {
        let mut next = current.with_page(1);
        change(&mut next.query);
        out.push((group.to_string(), value, next));
    };

    if !q.text.trim().is_empty() {
        push("Suche", q.text.trim().to_string(), &|q| q.text.clear());
    }
    if let Some(scope) = &q.program {
        match scope.plan_semester {
            Some(PlanSemesterFilter::Semester(n)) => push("Semester", format!("{n}."), &|q| {
                if let Some(s) = q.program.as_mut() {
                    s.plan_semester = None;
                }
            }),
            Some(PlanSemesterFilter::Unstated) => push("Semester", "ohne Angabe".to_string(), &|q| {
                if let Some(s) = q.program.as_mut() {
                    s.plan_semester = None;
                }
            }),
            None => {}
        }
        let kind_label = |kind: KindFilter| match kind {
            KindFilter::Stated(kind) => kind.label().to_string(),
            KindFilter::Unstated => "nicht angegeben".to_string(),
        };
        for kind in scope.kinds.clone() {
            push("Art", kind_label(kind), &move |q| {
                if let Some(s) = q.program.as_mut() {
                    s.kinds.retain(|k| *k != kind);
                }
            });
        }
        for kind in scope.kinds_exclude.clone() {
            push("Art", format!("ohne {}", kind_label(kind)), &move |q| {
                if let Some(s) = q.program.as_mut() {
                    s.kinds_exclude.retain(|k| *k != kind);
                }
            });
        }
    }
    if q.turnus.winter {
        push("Turnus", "Winter".to_string(), &|q| q.turnus.winter = false);
    }
    if q.turnus.summer {
        push("Turnus", "Sommer".to_string(), &|q| q.turnus.summer = false);
    }
    if q.turnus.irregular {
        push("Turnus", "unregelmäßig".to_string(), &|q| q.turnus.irregular = false);
    }
    if q.turnus.not_winter {
        push("Turnus", "nicht im Winter".to_string(), &|q| q.turnus.not_winter = false);
    }
    if q.turnus.not_summer {
        push("Turnus", "nicht im Sommer".to_string(), &|q| q.turnus.not_summer = false);
    }
    if q.turnus.not_irregular {
        push("Turnus", "nicht unregelmäßig".to_string(), &|q| q.turnus.not_irregular = false);
    }
    if let Some(parity) = q.turnus.year_parity {
        push("Jahre", parity.label().to_string(), &|q| q.turnus.year_parity = None);
    }
    for form in q.teaching_forms.clone() {
        push("Lehrform", form.label().to_string(), &move |q| q.teaching_forms.retain(|f| *f != form));
    }
    for form in q.teaching_forms_exclude.clone() {
        push("Lehrform", format!("ohne {}", form.label()), &move |q| q.teaching_forms_exclude.retain(|f| *f != form));
    }
    for part in q.exam_parts.clone() {
        push("Prüfung", part.label().to_string(), &move |q| q.exam_parts.retain(|p| *p != part));
    }
    for form in q.exam_forms.clone() {
        push("Prüfung", format::exam_short(&Code::Known(form)), &move |q| q.exam_forms.retain(|f| *f != form));
    }
    for part in q.exam_parts_exclude.clone() {
        push("Prüfung", format!("ohne {}", part.label()), &move |q| q.exam_parts_exclude.retain(|p| *p != part));
    }
    for language in q.languages.clone() {
        push("Sprache", language.label().to_string(), &move |q| q.languages.retain(|l| *l != language));
    }
    for language in q.languages_exclude.clone() {
        push("Sprache", format!("nicht {}", language.label()), &move |q| q.languages_exclude.retain(|l| *l != language));
    }
    if q.credits_min.is_some() || q.credits_max.is_some() {
        let label = match (q.credits_min, q.credits_max) {
            (Some(a), Some(b)) => format!("{}–{}", format::number(a), format::number(b)),
            (Some(a), None) => format!("ab {}", format::number(a)),
            (None, Some(b)) => format!("bis {}", format::number(b)),
            (None, None) => String::new(),
        };
        push("LP", label, &|q| {
            q.credits_min = None;
            q.credits_max = None;
        });
    }
    if let Some(graded) = q.graded {
        push("Benotung", if graded { "benotet" } else { "unbenotet" }.to_string(), &|q| q.graded = None);
    }
    if let Some(limited) = q.limited {
        push("Plätze", if limited { "begrenzt" } else { "unbegrenzt" }.to_string(), &|q| q.limited = None);
    }
    if let Some(fues) = q.fues {
        push("FÜS", if fues { "nur FÜS" } else { "ohne FÜS" }.to_string(), &|q| q.fues = None);
    }
    if let Some(n) = q.duration_semesters {
        push("Dauer", format!("{n} Semester"), &|q| q.duration_semesters = None);
    }
    if let Some(id) = q.department_id {
        let label = data.departments.iter().find(|d| d.id == id).map(|d| d.label.clone()).unwrap_or_else(|| id.to_string());
        push("Fachgebiet", label, &|q| q.department_id = None);
    }
    for name in q.lecturers_include.clone() {
        let keep = name.clone();
        push("bei", name, &move |q| q.lecturers_include.retain(|n| *n != keep));
    }
    for name in q.lecturers_exclude.clone() {
        let keep = name.clone();
        push("nicht bei", name, &move |q| q.lecturers_exclude.retain(|n| *n != keep));
    }
    for campus in q.campuses.clone() {
        push("Standort", campus.label().to_string(), &move |q| q.campuses.retain(|c| *c != campus));
    }
    for campus in q.campuses_exclude.clone() {
        push("Standort", format!("nicht {}", campus.label()), &move |q| q.campuses_exclude.retain(|c| *c != campus));
    }
    if q.offer.is_some() {
        push("Status", "auch nicht mehr angebotene".to_string(), &|q| q.offer = None);
    }
    out
}

#[component]
fn List(
    current: CatalogUrl,
    data: CatalogData,
    open: Memo<Option<String>>,
    page: Memo<u64>,
    phone: RwSignal<bool>,
    /// The row to scroll to once the list is there (it may be on the next page of the list).
    reveal: Option<String>,
) -> impl IntoView {
    let q = current.query.clone();
    let total = data.page.total;
    let pages_total = total.div_ceil(PAGE_SIZE).max(1);
    let start_page = current.page.min(pages_total);
    let with_program = data.program.is_some();
    let unknown_program = q.program.is_some() && !with_program;
    let by_plan = with_program && q.sort == SortKey::Default;
    let label = match (&data.program, q.program.as_ref().map(|s| s.relation)) {
        (Some(_), Some(ProgramRelation::Fues)) => "FÜS-Module dieses Studiengangs",
        (Some(_), _) => "Module im Curriculum",
        _ if total == 1 => "Modul",
        _ => "Module",
    };
    let active = tags(&current, &data);
    let active_count = active.len();

    let sort_link = |key: SortKey, text: &'static str, class: &'static str| {
        let on = current.query.sort == key;
        let mut next = current.with_page(1);
        next.query.descending = on && !current.query.descending;
        next.query.sort = key;
        let arrow = match (on, current.query.descending) {
            (true, false) => " ↑",
            (true, true) => " ↓",
            _ => "",
        };
        view! { <a class=class href=keep_open(next, open) aria-current=on.then_some("true")>{text}{arrow}</a> }
    };

    // The list is a sequence of chunks, one per page. The server renders the page the URL names
    // (with pager links); the browser app appends the next chunk when the visitor gets near the
    // end, prepends on request, and keeps `page` in the URL in step with what is on screen.
    let chunks = RwSignal::new(vec![Chunk { page: start_page, rows: data.page.rows.clone() }]);
    let source = use_source();
    let query = StoredValue::new(current.query.clone());
    let load = move |page_no: u64| -> Option<Vec<CatalogRow>> {
        let source = source.clone().ok()?;
        source.run(|db| catalog::queries::catalog_page(db, &query.get_value(), (page_no - 1) * PAGE_SIZE, PAGE_SIZE)).ok().map(|p| p.rows)
    };
    let load_next = {
        let load = load.clone();
        move || {
            let Some(last_page) = chunks.with_untracked(|c| c.last().map(|c| c.page)) else { return };
            if last_page >= pages_total {
                return;
            }
            if let Some(rows) = load(last_page + 1) {
                chunks.update(|c| c.push(Chunk { page: last_page + 1, rows }));
            }
        }
    };
    let load_previous = move || {
        let Some(first_page) = chunks.with_untracked(|c| c.first().map(|c| c.page)) else { return };
        if first_page <= 1 {
            return;
        }
        if let Some(rows) = load(first_page - 1) {
            let height = list_height(ROWS_ID);
            chunks.update(|c| c.insert(0, Chunk { page: first_page - 1, rows }));
            keep_position_after_prepend(ROWS_ID, height);
        }
    };
    let first_loaded = move || chunks.with(|c| c.first().map(|c| c.page).unwrap_or(1));
    let last_loaded = move || chunks.with(|c| c.last().map(|c| c.page).unwrap_or(1));

    // Scrolling: load more near the end, and let the URL follow the position (replacing the
    // history entry, so Back still leaves the list in one step).
    let navigate = use_navigate();
    let base = StoredValue::new(current.clone());
    let follow = {
        let load_next = load_next.clone();
        move || {
            let Some(position) = list_position(ROWS_ID) else { return };
            if position.near_end {
                load_next();
            }
            if let Some(seen) = position.page.filter(|seen| *seen != page.get_untracked()) {
                let target = base.get_value().with_page(seen).with_open(open.get_untracked().as_deref()).path();
                navigate(&target, NavigateOptions { replace: true, scroll: false, ..Default::default() });
            }
        }
    };
    let follow_window = follow.clone();
    Effect::new(move |_| {
        // On a phone the window scrolls, not the panel.
        let follow = follow_window.clone();
        let handle = window_event_listener(leptos::ev::scroll, move |_| follow());
        on_cleanup(move || handle.remove());
    });
    let load_next_click = load_next.clone();
    let chunk_base = current.clone();
    if let Some(id) = reveal {
        let load_next = load_next.clone();
        Effect::new(move |_| {
            if !nav::reveal_row(ROWS_ID, &id) {
                load_next();
            }
            // Once more in the next frame (the row may have been on the next page), and once
            // more a moment later: coming back through the history, the browser restores the
            // scroll position of a list that was longer above, after this effect has run.
            let (next_frame, later) = (id.clone(), id.clone());
            request_animation_frame(move || {
                nav::reveal_row(ROWS_ID, &next_frame);
            });
            set_timeout(
                move || {
                    nav::reveal_row(ROWS_ID, &later);
                },
                std::time::Duration::from_millis(220),
            );
        });
    }

    view! {
        <section class="panel list" aria-live="polite">
            <div class="list-head">
                <div class="count-row">
                    <span class="count num">{format::count(total)}</span>
                    <span class="count-label">{label}</span>
                    <div class="list-tools">
                        <span class="keys" title="Mit den Pfeiltasten durch die Liste, Enter öffnet die Vorschau"><kbd>"↑"</kbd><kbd>"↓"</kbd>" wählen "<kbd>"Enter"</kbd>" öffnen"</span>
                        <a class="sheet-toggle" href="#filters" data-action="sheet-open">
                            <Icon name="sliders-horizontal"/>"Filter"{(active_count > 0).then(|| view! { <em>{active_count}</em> })}
                        </a>
                        {data.program.as_ref().map(|p| view! {
                            <a class="ghost" href=url::program_path(&p.slug, ProgramTab::Plan)><Icon name="graduation-cap"/>"Studiengangsseite"</a>
                        })}
                    </div>
                </div>
                <div class="active-filters">
                    {active.into_iter().map(|(group, value, target)| view! {
                        <span class="tag"><em>{group}</em>" "{value}<a href=keep_open(target, open) aria-label="Filter entfernen"><Icon name="x"/></a></span>
                    }).collect_view()}
                </div>
            </div>
            <div class="cols label">
                {sort_link(if with_program { SortKey::Default } else { SortKey::Title }, "Modul", "")}
                <span class="c-resp">"Verantwortlich"</span>
                <span class="c-exam">"Prüfung"</span>
                {sort_link(SortKey::Credits, "LP", "c-lp")}
                <span class="c-turnus">"Turnus"</span>
                <span class="c-lang">"Spr."</span>
                {sort_link(SortKey::Events, "Termine", "c-events")}
            </div>
            <div class="rows scroll" id=ROWS_ID data-keep-scroll="rows" on:scroll=move |_| follow()>
                {unknown_program.then(|| view! {
                    <div class="state"><p class="state-title">"Diesen Studiengang gibt es nicht (mehr)"</p><p>"Wähle links einen anderen Studiengang oder „Alle Studiengänge“."</p></div>
                })}
                {(total == 0 && !unknown_program).then(|| view! {
                    <div class="state"><p class="state-title">"Keine Module gefunden"</p><p>"Nimm Filter zurück oder suche nach einem anderen Begriff."</p><a class="btn secondary" href=url::CATALOG>"Alle Filter zurücksetzen"</a></div>
                })}
                {move || (first_loaded() > 1).then(|| {
                    let load_previous = load_previous.clone();
                    view! { <button class="btn secondary load-more" type="button" on:click=move |_| load_previous()>"Vorherige Module laden"</button> }
                })}
                <For each=move || chunks.get() key=|chunk| chunk.page let:chunk>
                    <ChunkRows chunk chunks base=chunk_base.clone() by_plan with_program open page phone/>
                </For>
                {move || (last_loaded() < pages_total).then(|| {
                    let load_next = load_next_click.clone();
                    view! { <button class="btn secondary load-more" type="button" on:click=move |_| load_next()>"Weitere Module laden"</button> }
                })}
                {move || (last_loaded() >= pages_total && total > PAGE_SIZE).then(|| view! { <p class="list-end">"Ende der Liste · "{format::count(total)}" Module"</p> })}
                // Without the browser app (no JavaScript, search engines): plain page links.
                {(pages_total > 1).then(|| view! {
                    <nav class="pager" aria-label="Seiten">
                        {(start_page > 1).then(|| view! { <a class="btn secondary" rel="prev" href=keep_open(current.with_page(start_page - 1), open)>"Zurück"</a> })}
                        <span class="num">"Seite "{start_page}" von "{pages_total}</span>
                        {(start_page < pages_total).then(|| view! { <a class="btn secondary" rel="next" href=keep_open(current.with_page(start_page + 1), open)>"Weiter"</a> })}
                    </nav>
                })}
            </div>
        </section>
    }
}

#[component]
fn Row(
    row: CatalogRow,
    base: CatalogUrl,
    open: Memo<Option<String>>,
    page: Memo<u64>,
    phone: RwSignal<bool>,
    with_program: bool,
    /// Set on the first row of a page: how the list knows which page is on screen.
    starts_page: Option<u64>,
) -> impl IntoView {
    let language = format::languages(row.teaches_german, row.teaches_english);
    let (turnus_icon, turnus_text) = match row.turnus_season.as_ref().and_then(|s| s.known()) {
        Some(TurnusSeason::Winter) => ("snowflake", "Winter".to_string()),
        Some(TurnusSeason::Summer) => ("sun", "Sommer".to_string()),
        Some(TurnusSeason::Both) => ("repeat", "jedes Sem.".to_string()),
        Some(TurnusSeason::Irregular) => ("shuffle", "unregelm.".to_string()),
        None => ("minus", row.turnus_season.as_ref().map(|s| s.label().to_string()).unwrap_or_else(|| "k. A.".to_string())),
    };
    let events = match row.teaching_events {
        0 => "noch keine Termine".to_string(),
        1 => "1 Termin".to_string(),
        n => format!("{n} Termine"),
    };
    let has_events = row.teaching_events > 0;
    let id = row.id.clone();
    let target = row.id.clone();
    // The preview next to the list; on a phone the module's own page.
    let href = move || {
        if phone.get() {
            url::module_path(&target)
        } else {
            base.with_page(page.get()).with_open(Some(&target)).path()
        }
    };
    view! {
        <a class="row" href=href data-noscroll="" data-id=row.id.clone() data-page=starts_page aria-current=move || (open.get().as_deref() == Some(id.as_str())).then_some("true")>
            <div class="t">
                <b>{row.title.clone()}</b>
                <small>
                    <span class="mono">{row.id.clone()}</span>
                    {with_program.then(|| view! { <KindBadge kind=row.kind.clone()/> })}
                    <OfferBadge status=row.offer_status.clone()/>
                    {(row.is_fues && !with_program).then(|| view! { <span class="flag neutral">"FÜS"</span> })}
                    {(row.is_limited == Some(true)).then(|| view! { <span class="flag neutral">"begrenzte Plätze"</span> })}
                    <span class="narrow-only">{language.map(|l| format!("{l} · "))}{events.clone()}</span>
                </small>
            </div>
            <span class="resp">{row.responsible.clone()}</span>
            <span class="exam">{row.exam_form.as_ref().map(format::exam_short)}</span>
            <span class="lp num">{row.credits.map(format::number)}<small>"LP"</small></span>
            <span class="turnus" title=turnus_text.clone()><Icon name=turnus_icon/><span class="txt">{turnus_text.clone()}</span></span>
            <span class="lang" class:unknown=language.is_none()>{language.unwrap_or("k. A.")}</span>
            <span class="events" class:none=!has_events>
                {has_events.then(|| view! { <Icon name="calendar-check-2"/> })}
                {if has_events { events } else { "noch keine".to_string() }}
            </span>
        </a>
    }
}

/// One page of the list.
#[derive(Clone, PartialEq)]
struct Chunk {
    page: u64,
    rows: Vec<CatalogRow>,
}

const ROWS_ID: &str = "rows";

/// The rows of one page. They are direct children of the scrolling list (the wrapper has no box
/// of its own), so a sticky group header stays in place for as long as its group is on screen,
/// across the border between two pages.
#[component]
fn ChunkRows(
    chunk: Chunk,
    chunks: RwSignal<Vec<Chunk>>,
    base: CatalogUrl,
    by_plan: bool,
    with_program: bool,
    open: Memo<Option<String>>,
    page: Memo<u64>,
    phone: RwSignal<bool>,
) -> impl IntoView {
    // Group headers follow the study plan when the list is in plan order. A group that began on
    // the page before (which may be loaded later, above this one) gets no second header.
    let own_page = chunk.page;
    let group_before = Memo::new(move |_| {
        chunks.with(|all| {
            let index = all.iter().position(|c| c.page == own_page)?;
            all.get(index.checked_sub(1)?)?.rows.last().map(|row| row.plan_semester)
        })
    });
    let header = |semester: Option<i64>| match semester {
        Some(n) => format!("{n}. Semester"),
        None => "Ohne Semesterangabe im Regelstudienplan".to_string(),
    };
    let mut last_group: Option<Option<i64>> = None;
    let rows = chunk
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let group = row.plan_semester;
            let first = index == 0;
            let new_group = by_plan && last_group != Some(group);
            last_group = Some(group);
            view! {
                {(new_group && first).then(|| view! {
                    {move || (group_before.get() != Some(group)).then(|| view! { <div class="sem">{header(group)}</div> })}
                })}
                {(new_group && !first).then(|| view! { <div class="sem">{header(group)}</div> })}
                <Row row=row.clone() base=base.clone() open page phone with_program starts_page=first.then_some(chunk.page)/>
            }
        })
        .collect_view();
    view! { <div class="chunk">{rows}</div> }
}

/// Whether this build is the browser app. Pages rendered on the server get plain form controls
/// where the app has pickers, so they work without JavaScript.
const APP: bool = cfg!(feature = "csr");

/// The slider covers 0 to 30 credits; its right end means „no upper limit".
const CREDITS_MAX: f64 = 30.0;

/// What the filter panel shows besides the filter itself. Kept apart from the rows of the
/// list, so that the panel stays (focus, scroll position, open pickers) while the list changes.
#[derive(Clone, Default, PartialEq)]
struct Facts {
    program: Option<Program>,
    curricular_total: Option<u64>,
    fues_total: Option<u64>,
    plan_semesters: Vec<i64>,
    total: u64,
}

impl Facts {
    fn of(data: &CatalogData) -> Self {
        Self {
            program: data.program.clone(),
            curricular_total: data.curricular_total,
            fues_total: data.fues_total,
            plan_semesters: data.plan_semesters.clone(),
            total: data.page.total,
        }
    }
}

/// What the pickers offer: the same for every filter, it changes only with the snapshot.
#[derive(Clone, Default, PartialEq)]
struct Choices {
    programs: Vec<ComboItem>,
    departments: Vec<ComboItem>,
    lecturers: Vec<ComboItem>,
}

impl Choices {
    fn of(data: &CatalogChoices) -> Self {
        // A program is its name, the short degree and the year of its PO: one shape for all
        // (owner decision 2026-09-20; amendments are not part of it). Where two programs would
        // read the same, and only there, the form of study tells them apart.
        let short = |p: &Program| (p.name.clone(), p.degree().to_string(), p.po_year);
        let variant = |p: &Program| p.study_variant.as_ref().map(format::variant_short);
        Self {
            programs: data
                .programs
                .iter()
                .map(|p| {
                    let year = p.po_year.map(|year| year.to_string()).unwrap_or_else(|| p.po_version.clone());
                    let alike = data.programs.iter().filter(|other| short(other) == short(p)).count() > 1;
                    let detail = match variant(p).filter(|_| alike) {
                        Some(variant) => format!("{} · {year} · {variant}", p.degree()),
                        None => format!("{} · {year}", p.degree()),
                    };
                    ComboItem::new(p.slug.clone(), p.name.clone(), detail, i64::from(p.is_latest_po))
                })
                .collect(),
            departments: data.departments.iter().map(|d| ComboItem::new(d.id.to_string(), d.label.clone(), format!("{} Module", d.modules), 0)).collect(),
            lecturers: data.lecturers.iter().map(|l| ComboItem::new(l.name.clone(), l.name.clone(), l.title.clone().unwrap_or_default(), 0)).collect(),
        }
    }
}

/// The catalog that `change` leads to from the current filter: what a control links to.
fn target(query: Memo<CatalogQuery>, open: Memo<Option<String>>, change: impl FnOnce(&mut CatalogQuery)) -> String {
    let mut next = query.get();
    change(&mut next);
    CatalogUrl { query: next, page: 1, open: open.get() }.path()
}

/// The same from an event handler, which has nothing to track.
fn target_now(query: Memo<CatalogQuery>, open: Memo<Option<String>>, change: impl FnOnce(&mut CatalogQuery)) -> String {
    let mut next = query.get_untracked();
    change(&mut next);
    CatalogUrl { query: next, page: 1, open: open.get_untracked() }.path()
}

/// A filter value is off, wanted, or unwanted („keine Vorträge").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tri {
    Off,
    With,
    Without,
}

type ReadTri = dyn Fn(&CatalogQuery) -> Tri + Send + Sync;
type WriteTri = dyn Fn(&mut CatalogQuery, Tri) + Send + Sync;

/// How a chip reads its state from the filter and writes it back.
#[derive(Clone)]
struct Toggle {
    read: Arc<ReadTri>,
    write: Arc<WriteTri>,
    /// Off → with → without → off. Otherwise only off ↔ with.
    excludes: bool,
}

impl Toggle {
    fn new(read: impl Fn(&CatalogQuery) -> Tri + Send + Sync + 'static, write: impl Fn(&mut CatalogQuery, Tri) + Send + Sync + 'static) -> Self {
        Self { read: Arc::new(read), write: Arc::new(write), excludes: true }
    }

    /// A value that is wanted when in the first list and unwanted when in the second.
    fn in_lists<T: PartialEq + Copy + Send + Sync + 'static>(
        value: T,
        lists: fn(&CatalogQuery) -> (&Vec<T>, &Vec<T>),
        lists_mut: fn(&mut CatalogQuery) -> (&mut Vec<T>, &mut Vec<T>),
    ) -> Self {
        Self::new(
            move |q| {
                let (with, without) = lists(q);
                if with.contains(&value) {
                    Tri::With
                } else if without.contains(&value) {
                    Tri::Without
                } else {
                    Tri::Off
                }
            },
            move |q, state| {
                let (with, without) = lists_mut(q);
                with.retain(|v| *v != value);
                without.retain(|v| *v != value);
                match state {
                    Tri::With => with.push(value),
                    Tri::Without => without.push(value),
                    Tri::Off => {}
                }
            },
        )
    }

    /// „Only such modules" / „no such modules" on a yes-no property.
    fn flag(get: fn(&CatalogQuery) -> Option<bool>, set: fn(&mut CatalogQuery, Option<bool>)) -> Self {
        Self::new(
            move |q| match get(q) {
                Some(true) => Tri::With,
                Some(false) => Tri::Without,
                None => Tri::Off,
            },
            move |q, state| {
                set(
                    q,
                    match state {
                        Tri::With => Some(true),
                        Tri::Without => Some(false),
                        Tri::Off => None,
                    },
                )
            },
        )
    }

    fn after(&self, state: Tri) -> Tri {
        match state {
            Tri::Off => Tri::With,
            Tri::With if self.excludes => Tri::Without,
            Tri::With | Tri::Without => Tri::Off,
        }
    }
}

/// A toggle: a link to the list with the next state of its value. The small box on its left
/// shows the state (empty, ticked, crossed), so that it reads as a switch and not as a button.
#[component]
fn Chip(
    query: Memo<CatalogQuery>,
    open: Memo<Option<String>>,
    toggle: Toggle,
    #[prop(into)] label: String,
    icon: Option<&'static str>,
) -> impl IntoView {
    let read = toggle.read.clone();
    let state = Memo::new(move |_| query.with(|q| read(q)));
    let excludes = toggle.excludes;
    // The link reads the filter itself and not `state`. A closure that reads a memo derived from
    // `query` before `query` misses a change of `query` whenever the derived value stays the
    // same (reactive_graph 0.2.14 does not mark the observer that made a memo recompute, and the
    // derived memo then reports „unchanged"). That was the first toggle of the panel losing the
    // rest of the filter. Rule (docs/frontend.md, R16): in one closure read the source, not a
    // memo derived from it and the source.
    let href = move || {
        target(query, open, |q| {
            let next = toggle.after((toggle.read)(q));
            (toggle.write)(q, next)
        })
    };
    let name = label.clone();
    view! {
        <a
            class="chip"
            href=href
            role="checkbox"
            rel="nofollow"
            draggable="false"
            data-noscroll=""
            data-state=move || match state.get() {
                Tri::Off => "off",
                Tri::With => "with",
                Tri::Without => "without",
            }
            aria-checked=move || match state.get() {
                Tri::Off => "false",
                Tri::With => "true",
                Tri::Without => "mixed",
            }
            aria-label=move || match state.get() {
                Tri::Without => format!("{name}: ausgeschlossen"),
                _ => name.clone(),
            }
            title=move || match (state.get(), excludes) {
                (Tri::Off, true) => Some("Klick: nur mit · zweiter Klick: ohne"),
                (Tri::With, true) => Some("Nur mit. Noch ein Klick schließt aus"),
                (Tri::Without, _) => Some("Ausgeschlossen. Ein Klick hebt das auf"),
                _ => None,
            }
        >
            <span class="box"><Icon name="check"/><Icon name="x"/></span>
            {icon.map(|name| view! { <Icon name=name/> })}
            <span class="chip-label">{label}</span>
        </a>
    }
}

/// One of a few: a row of links that fills the width, the chosen one raised.
struct Choice {
    label: String,
    title: Option<&'static str>,
    count: Option<Signal<Option<u64>>>,
    is_on: Arc<dyn Fn(&CatalogQuery) -> bool + Send + Sync>,
    choose: Arc<dyn Fn(&mut CatalogQuery) + Send + Sync>,
}

impl Choice {
    fn new(
        label: impl Into<String>,
        is_on: impl Fn(&CatalogQuery) -> bool + Send + Sync + 'static,
        choose: impl Fn(&mut CatalogQuery) + Send + Sync + 'static,
    ) -> Self {
        Self { label: label.into(), title: None, count: None, is_on: Arc::new(is_on), choose: Arc::new(choose) }
    }
}

fn segmented(query: Memo<CatalogQuery>, open: Memo<Option<String>>, label: &'static str, choices: Vec<Choice>) -> impl IntoView {
    let links = choices
        .into_iter()
        .map(|choice| {
            let Choice { label, title, count, is_on, choose } = choice;
            view! {
                <a
                    href=move || target(query, open, |q| choose(q))
                    role="radio"
                    rel="nofollow"
                    draggable="false"
                    data-noscroll=""
                    title=title
                    aria-checked=move || if query.with(|q| is_on(q)) { "true" } else { "false" }
                >
                    {label}
                    {count.map(|count| view! { <span class="num">{move || count.get().map(format::count)}</span> })}
                </a>
            }
        })
        .collect_view();
    view! { <div class="seg" role="radiogroup" aria-label=label>{links}</div> }
}

/// The filter panel. It is rendered once and then follows `query`: every control is a link to
/// the list it leads to (so it works without JavaScript, and in the app the router turns the
/// click into a navigation), except the pickers and the credit slider, which have handlers.
#[component]
fn Filters(query: Memo<CatalogQuery>, facts: Memo<Facts>, choices: Memo<Choices>, open: Memo<Option<String>>) -> impl IntoView {
    let navigate = use_navigate();
    let go = Callback::new(move |path: String| navigate(&path, NavigateOptions { scroll: false, ..Default::default() }));
    let close_popups = RwSignal::new(0u32);
    provide_context(ClosePopups(close_popups));

    // Parts that come and go are keyed on what they depend on, not on the whole filter.
    let program = Memo::new(move |_| facts.with(|f| f.program.clone()));
    let semesters = Memo::new(move |_| facts.with(|f| f.plan_semesters.clone()));
    let chosen_lecturers = Memo::new(move |_| {
        let mut names = query.with(|q| [q.lecturers_include.clone(), q.lecturers_exclude.clone()].concat());
        names.sort();
        names.dedup();
        names
    });
    let more_open = query.with_untracked(|q| {
        !q.lecturers_include.is_empty()
            || !q.lecturers_exclude.is_empty()
            || q.department_id.is_some()
            || q.duration_semesters.is_some()
            || !q.campuses.is_empty()
            || !q.campuses_exclude.is_empty()
            || q.offer.is_some()
            || q.turnus.year_parity.is_some()
    });

    let chip = move |label: &str, icon: Option<&'static str>, toggle: Toggle| view! { <Chip query open toggle label=label.to_string() icon/> };

    // ---- program ----
    let program_picker = if APP {
        let items = Memo::new(move |_| choices.with(|c| c.programs.clone()));
        let selected = Memo::new(move |_| query.with(|q| q.program.as_ref().map(|scope| scope.program_slug.clone())));
        let pick = Callback::new(move |slug: Option<String>| {
            go.run(target_now(query, open, |q| match slug {
                Some(slug) => q.program.get_or_insert_with(ProgramScope::default).program_slug = slug,
                None => q.program = None,
            }))
        });
        view! {
            <Combobox id="pick-program" label="Studiengang" placeholder="Alle Studiengänge" search_placeholder="Studiengang suchen" icon="graduation-cap" min_width=480.0 items selected on_select=pick/>
        }
        .into_any()
    } else {
        view! {
            <label class="select-wrap">
                <Icon name="graduation-cap"/>
                <span class="visually-hidden">"Studiengang"</span>
                <select name="program">
                    <option value="">"Alle Studiengänge"</option>
                    {move || {
                        let selected = query.with(|q| q.program.as_ref().map(|scope| scope.program_slug.clone()));
                        choices.with(|c| c.programs.iter().map(|p| {
                            let is_selected = selected.as_deref() == Some(p.id.as_str());
                            view! { <option value=p.id.clone() selected=is_selected>{format!("{} · {}", p.label, p.detail)}</option> }
                        }).collect_view())
                    }}
                </select>
                <Icon name="chevrons-up-down"/>
            </label>
        }
        .into_any()
    };

    let program_part = move || {
        program.get().map(|_| {
            let relation = |relation: ProgramRelation, label: &'static str, count: Signal<Option<u64>>| Choice {
                count: Some(count),
                ..Choice::new(
                    label,
                    move |q: &CatalogQuery| q.program.as_ref().is_some_and(|scope| scope.relation == relation),
                    move |q: &mut CatalogQuery| {
                        if let Some(scope) = q.program.as_mut() {
                            scope.relation = relation;
                        }
                    },
                )
            };
            let kind = |kind: KindFilter| {
                Toggle::new(
                    move |q| match &q.program {
                        Some(scope) if scope.kinds.contains(&kind) => Tri::With,
                        Some(scope) if scope.kinds_exclude.contains(&kind) => Tri::Without,
                        _ => Tri::Off,
                    },
                    move |q, state| {
                        if let Some(scope) = q.program.as_mut() {
                            scope.kinds.retain(|k| *k != kind);
                            scope.kinds_exclude.retain(|k| *k != kind);
                            match state {
                                Tri::With => scope.kinds.push(kind),
                                Tri::Without => scope.kinds_exclude.push(kind),
                                Tri::Off => {}
                            }
                        }
                    },
                )
            };
            let semester_choice = |label: String, title: Option<&'static str>, value: Option<PlanSemesterFilter>| Choice {
                title,
                ..Choice::new(
                    label,
                    move |q: &CatalogQuery| q.program.as_ref().and_then(|scope| scope.plan_semester) == value,
                    move |q: &mut CatalogQuery| {
                        if let Some(scope) = q.program.as_mut() {
                            scope.plan_semester = value;
                        }
                    },
                )
            };
            let semester_part = move || {
                let semesters = semesters.get();
                if semesters.is_empty() {
                    return view! { <p class="hint">"Für diesen Studiengang liegt kein geprüfter Regelstudienplan vor, Fachsemester sind deshalb nicht bekannt."</p> }.into_any();
                }
                let mut all = vec![semester_choice("Alle".to_string(), None, None)];
                all.extend(semesters.iter().filter_map(|n| u8::try_from(*n).ok()).map(|n| {
                    semester_choice(n.to_string(), None, Some(PlanSemesterFilter::Semester(n)))
                }));
                all.push(semester_choice("?".to_string(), Some("Module, die der Regelstudienplan keinem Semester zuordnet"), Some(PlanSemesterFilter::Unstated)));
                view! {
                    <div class="flabel label">"Fachsemester laut Plan"</div>
                    {segmented(query, open, "Fachsemester", all)}
                }
                .into_any()
            };
            view! {
                {segmented(query, open, "Liste", vec![
                    relation(ProgramRelation::Curricular, "Curriculum", Signal::derive(move || facts.with(|f| f.curricular_total))),
                    relation(ProgramRelation::Fues, "FÜS", Signal::derive(move || facts.with(|f| f.fues_total))),
                ])}
                <div class="fgroup">
                    <div class="flabel label">"Modulart"</div>
                    <div class="chips">
                        {[ModuleKind::Compulsory, ModuleKind::Elective, ModuleKind::Thesis, ModuleKind::Internship]
                            .iter().map(|k| chip(k.label(), None, kind(KindFilter::Stated(*k)))).collect_view()}
                        {chip("Nicht angegeben", None, kind(KindFilter::Unstated))}
                    </div>
                    {semester_part}
                </div>
            }
        })
    };

    // ---- lecturers ----
    // A chosen person is a row: on the left the switch between + (wanted) and × (unwanted), on
    // the right the button that takes the person out again. Wanted persons are alternatives
    // (Meer or Köhler), unwanted ones are all left out (neither Lambers nor Hofstedt).
    let person = move |name: String| {
        let place = move |q: &mut CatalogQuery, name: &str, wanted: Option<bool>| {
            q.lecturers_include.retain(|n| n != name);
            q.lecturers_exclude.retain(|n| n != name);
            match wanted {
                Some(true) => q.lecturers_include.push(name.to_string()),
                Some(false) => q.lecturers_exclude.push(name.to_string()),
                None => {}
            }
        };
        let title = choices.with_untracked(|c| c.lecturers.iter().find(|item| item.id == name).map(|item| item.detail.clone())).unwrap_or_default();
        let link = |wanted: Option<bool>| {
            let name = name.clone();
            move || target(query, open, |q| place(q, &name, wanted))
        };
        let is_unwanted = {
            let name = name.clone();
            move || query.with(|q| q.lecturers_exclude.contains(&name))
        };
        let (unwanted_1, unwanted_2, unwanted_3) = (is_unwanted.clone(), is_unwanted.clone(), is_unwanted);
        view! {
            <div class="person" data-state=move || if unwanted_1() { "without" } else { "with" }>
                <div class="seg mini" role="radiogroup" aria-label=name.clone()>
                    <a href=link(Some(true)) role="radio" rel="nofollow" draggable="false" data-noscroll="" class="plus" title="Module mit dieser Person" aria-label="mit" aria-checked=move || if unwanted_2() { "false" } else { "true" }><Icon name="plus"/></a>
                    <a href=link(Some(false)) role="radio" rel="nofollow" draggable="false" data-noscroll="" class="cross" title="Module ohne diese Person" aria-label="ohne" aria-checked=move || if unwanted_3() { "true" } else { "false" }><Icon name="x"/></a>
                </div>
                <span class="person-name"><b>{name.clone()}</b>{(!title.is_empty()).then(|| view! { <small>{title}</small> })}</span>
                <a class="icon-btn remove" href=link(None) rel="nofollow" draggable="false" data-noscroll="" title="Entfernen" aria-label=format!("{name} entfernen")><Icon name="trash-2"/></a>
            </div>
        }
    };
    let lecturer_picker = if APP {
        let items = Memo::new(move |_| {
            let chosen = chosen_lecturers.get();
            choices.with(|c| c.lecturers.iter().filter(|item| !chosen.contains(&item.id)).cloned().collect::<Vec<_>>())
        });
        let add = Callback::new(move |name: Option<String>| {
            if let Some(name) = name {
                go.run(target_now(query, open, |q| q.lecturers_include.push(name)));
            }
        });
        view! {
            <Combobox id="pick-lecturer" label="Lehrende" placeholder="Person hinzufügen" search_placeholder="Name suchen" icon="users-round" items selected=Signal::derive(|| None::<String>) on_select=add clearable=false/>
        }
        .into_any()
    } else {
        view! {
            <label class="field">
                <span class="visually-hidden">"Lehrt oder verantwortet"</span>
                <input type="text" name="lecturer" list="lecturers" placeholder="Nachname, Vorname"/>
            </label>
            <datalist id="lecturers">
                {move || choices.with(|c| c.lecturers.iter().map(|l| view! { <option value=l.id.clone()></option> }).collect_view())}
            </datalist>
        }
        .into_any()
    };

    // ---- department ----
    let department_picker = if APP {
        let items = Memo::new(move |_| choices.with(|c| c.departments.clone()));
        let selected = Memo::new(move |_| query.with(|q| q.department_id.map(|id| id.to_string())));
        let pick = Callback::new(move |id: Option<String>| go.run(target_now(query, open, |q| q.department_id = id.and_then(|id| id.parse().ok()))));
        view! {
            <Combobox id="pick-department" label="Fachgebiet" placeholder="Alle Fachgebiete" search_placeholder="Fachgebiet suchen" icon="building-2" items selected on_select=pick/>
        }
        .into_any()
    } else {
        view! {
            <span class="select-wrap plain">
                <select name="department" aria-label="Fachgebiet">
                    <option value="">"Alle Fachgebiete"</option>
                    {move || {
                        let selected = query.with(|q| q.department_id.map(|id| id.to_string()));
                        choices.with(|c| c.departments.iter().map(|d| {
                            let is_selected = selected.as_deref() == Some(d.id.as_str());
                            view! { <option value=d.id.clone() selected=is_selected>{format!("{} ({})", d.label, d.detail)}</option> }
                        }).collect_view())
                    }}
                </select>
                <Icon name="chevrons-up-down"/>
            </span>
        }
        .into_any()
    };

    // Without the app the pickers above are form fields; what the links set travels with them.
    let carried = move || {
        (!APP).then(|| {
            let pairs = url::parse_pairs(&CatalogUrl { query: query.get(), page: 1, open: open.get() }.to_query_string());
            pairs
                .into_iter()
                .filter(|(name, _)| !matches!(name.as_str(), "program" | "department" | "ects_min" | "ects_max"))
                .map(|(name, value)| view! { <input type="hidden" name=name value=value/> })
                .collect_view()
        })
    };

    view! {
        <aside class="panel filters" id="filters" aria-label="Filter">
            <form method="get" action=url::CATALOG data-autosubmit="" on:submit=move |ev| if APP { ev.prevent_default() }>
                <div class="panel-head">
                    <h2>"Filter"</h2>
                    <a class="ghost hit" style=Hit::y(7.0).style() href=move || CatalogUrl { open: open.get(), ..Default::default() }.path() data-noscroll=""><Icon name="rotate-ccw"/>"Zurücksetzen"</a>
                    <a class="icon-btn sheet-close" href="#" data-action="sheet-close" aria-label="Filter schließen"><Icon name="x"/></a>
                </div>
                <div class="body scroll" data-keep-scroll="filters" on:scroll=move |_| close_popups.update(|n| *n = n.wrapping_add(1))>
                    {carried}
                    {program_picker}
                    {program_part}

                    <div class="fgroup">
                        <div class="flabel label">"Angeboten im"<span class="legend"><i class="box with"><Icon name="check"/></i>"mit"<i class="box without"><Icon name="x"/></i>"ohne"</span></div>
                        <div class="chips">
                            {chip("Winter", Some("snowflake"), Toggle::new(
                                |q| if q.turnus.winter { Tri::With } else if q.turnus.not_winter { Tri::Without } else { Tri::Off },
                                |q, state| (q.turnus.winter, q.turnus.not_winter) = (state == Tri::With, state == Tri::Without),
                            ))}
                            {chip("Sommer", Some("sun"), Toggle::new(
                                |q| if q.turnus.summer { Tri::With } else if q.turnus.not_summer { Tri::Without } else { Tri::Off },
                                |q, state| (q.turnus.summer, q.turnus.not_summer) = (state == Tri::With, state == Tri::Without),
                            ))}
                            {chip("Unregelmäßig", Some("shuffle"), Toggle::new(
                                |q| if q.turnus.irregular { Tri::With } else if q.turnus.not_irregular { Tri::Without } else { Tri::Off },
                                |q, state| (q.turnus.irregular, q.turnus.not_irregular) = (state == Tri::With, state == Tri::Without),
                            ))}
                        </div>
                    </div>
                    <div class="fgroup">
                        <div class="flabel label">"Lehrform"</div>
                        <div class="chips">
                            {[TeachingForm::Lecture, TeachingForm::Exercise, TeachingForm::Seminar, TeachingForm::Practical, TeachingForm::Project, TeachingForm::Excursion]
                                .iter().map(|form| chip(form.label(), None, Toggle::in_lists(*form, |q| (&q.teaching_forms, &q.teaching_forms_exclude), |q| (&mut q.teaching_forms, &mut q.teaching_forms_exclude)))).collect_view()}
                        </div>
                    </div>
                    <div class="fgroup">
                        <div class="flabel label">"Prüfung"</div>
                        <div class="chips">
                            {ExamPart::ALL.iter().map(|part| chip(part.short_label(), None, Toggle::in_lists(*part, |q| (&q.exam_parts, &q.exam_parts_exclude), |q| (&mut q.exam_parts, &mut q.exam_parts_exclude)))).collect_view()}
                        </div>
                    </div>
                    <Credits query open go/>
                    <div class="fgroup">
                        <div class="flabel label">"Sprache"</div>
                        <div class="chips">
                            {Language::ALL.iter().map(|language| chip(language.label(), None, Toggle::in_lists(*language, |q| (&q.languages, &q.languages_exclude), |q| (&mut q.languages, &mut q.languages_exclude)))).collect_view()}
                        </div>
                    </div>
                    <div class="fgroup">
                        <div class="flabel label">"Eigenschaften"</div>
                        <div class="chips">
                            {chip("Benotet", None, Toggle::flag(|q| q.graded, |q, value| q.graded = value))}
                            {chip("Begrenzte Plätze", None, Toggle::flag(|q| q.limited, |q, value| q.limited = value))}
                            {chip("FÜS-Liste", None, Toggle::flag(|q| q.fues, |q, value| q.fues = value))}
                        </div>
                    </div>

                    <details class="fgroup more" open=more_open>
                        <summary class="label">"Weitere Filter"</summary>
                        <div class="flabel label">"Lehrende"</div>
                        {lecturer_picker}
                        {move || {
                            let names = chosen_lecturers.get();
                            (!names.is_empty()).then(|| view! {
                                <div class="people">{names.into_iter().map(person).collect_view()}</div>
                                <p class="hint people-hint"><span><b>"+"</b>"mindestens eine dieser Personen"</span><span><b>"×"</b>"keine dieser Personen"</span></p>
                            })
                        }}
                        <div class="flabel label">"Fachgebiet"</div>
                        {department_picker}
                        <div class="flabel label">"Dauer"</div>
                        {segmented(query, open, "Dauer", vec![
                            Choice::new("Egal", |q| q.duration_semesters.is_none(), |q| q.duration_semesters = None),
                            Choice::new("1 Semester", |q| q.duration_semesters == Some(1), |q| q.duration_semesters = Some(1)),
                            Choice::new("2 Semester", |q| q.duration_semesters == Some(2), |q| q.duration_semesters = Some(2)),
                        ])}
                        <div class="flabel label">"Nur in bestimmten Jahren"</div>
                        {segmented(query, open, "Jahre", vec![
                            Choice::new("Egal", |q| q.turnus.year_parity.is_none(), |q| q.turnus.year_parity = None),
                            Choice::new("Gerade", |q| q.turnus.year_parity == Some(TurnusParity::Even), |q| q.turnus.year_parity = Some(TurnusParity::Even)),
                            Choice::new("Ungerade", |q| q.turnus.year_parity == Some(TurnusParity::Odd), |q| q.turnus.year_parity = Some(TurnusParity::Odd)),
                        ])}
                        <div class="flabel label">"Standort"</div>
                        <div class="chips">
                            {[Campus::Zentralcampus, Campus::Sachsendorf, Campus::Senftenberg]
                                .iter().map(|campus| chip(campus.label(), None, Toggle::in_lists(*campus, |q| (&q.campuses, &q.campuses_exclude), |q| (&mut q.campuses, &mut q.campuses_exclude)))).collect_view()}
                        </div>
                        <p class="hint">"Der Standort ist nur für Module mit Raumangaben in diesem Semester bekannt."</p>
                        {move || program.get().is_none().then(|| view! {
                            <div class="chips">
                                {chip("Nicht mehr angebotene zeigen", None, Toggle {
                                    excludes: false,
                                    ..Toggle::new(
                                        |q| if q.offer.as_ref().is_some_and(|offer| offer.contains(&OfferStatus::NotOffered)) { Tri::With } else { Tri::Off },
                                        |q, state| q.offer = (state == Tri::With).then(|| OfferStatus::ALL.to_vec()),
                                    )
                                })}
                            </div>
                        })}
                    </details>
                </div>
                <div class="filter-actions">
                    <button class="btn primary apply" type="submit">"Filter anwenden"</button>
                    <a class="btn primary show" href="#" data-action="sheet-close">{move || format::count(facts.with(|f| f.total))}" Module anzeigen"</a>
                </div>
            </form>
        </aside>
    }
}

/// Credits: a slider with two thumbs for the usual range, and the two numbers next to it for
/// exact values (they also are what a plain form submits).
#[component]
fn Credits(query: Memo<CatalogQuery>, open: Memo<Option<String>>, go: Callback<String>) -> impl IntoView {
    let on_slider = |q: &CatalogQuery| {
        (q.credits_min.unwrap_or(0.0).clamp(0.0, CREDITS_MAX), q.credits_max.unwrap_or(CREDITS_MAX).clamp(0.0, CREDITS_MAX))
    };
    let (start_low, start_high) = query.with_untracked(on_slider);
    // Where the thumbs are while they are dragged; the filter follows when they are let go.
    let low = RwSignal::new(start_low);
    let high = RwSignal::new(start_high);
    Effect::new(move |_| {
        let (now_low, now_high) = query.with(on_slider);
        low.set(now_low);
        high.set(now_high);
    });

    let dragged = move |ev: &leptos::ev::Event, lower: bool| {
        let value = event_target_value(ev).parse::<f64>().unwrap_or(0.0);
        let value = if lower { value.min(high.get_untracked()) } else { value.max(low.get_untracked()) };
        // The thumb may not pass the other one: put it back where it is allowed to be.
        event_target::<leptos::web_sys::HtmlInputElement>(ev).set_value(&value.to_string());
        if lower { low.set(value) } else { high.set(value) }
        value
    };
    let typed = move |ev: &leptos::ev::Event| event_target_value(ev).trim().replace(',', ".").parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0);
    let summary = move || match (low.get(), high.get()) {
        (a, b) if a <= 0.0 && b >= CREDITS_MAX => "alle".to_string(),
        (a, b) if b >= CREDITS_MAX => format!("ab {} LP", format::number(a)),
        (a, b) if a <= 0.0 => format!("bis {} LP", format::number(b)),
        (a, b) if a == b => format!("{} LP", format::number(a)),
        (a, b) => format!("{}–{} LP", format::number(a), format::number(b)),
    };

    view! {
        <div class="fgroup credits">
            <div class="flabel label">"Leistungspunkte"<span>{summary}</span></div>
            <div
                class="slider js-only"
                style=move || format!("--from:{:.4};--to:{:.4}", low.get() / CREDITS_MAX, high.get() / CREDITS_MAX)
                // Both thumbs at the right end: the lower one has to be the one on top.
                data-low-on-top=move || (low.get() > CREDITS_MAX / 2.0).then_some("")
            >
                <input type="range" min="0" max="30" step="1" aria-label="Leistungspunkte mindestens"
                    value=start_low.to_string() prop:value=move || low.get().to_string()
                    on:input=move |ev| { dragged(&ev, true); }
                    on:change=move |ev| { let value = dragged(&ev, true); go.run(target_now(query, open, |q| q.credits_min = (value > 0.0).then_some(value))) }/>
                <input type="range" min="0" max="30" step="1" aria-label="Leistungspunkte höchstens"
                    value=start_high.to_string() prop:value=move || high.get().to_string()
                    on:input=move |ev| { dragged(&ev, false); }
                    on:change=move |ev| { let value = dragged(&ev, false); go.run(target_now(query, open, |q| q.credits_max = (value < CREDITS_MAX).then_some(value))) }/>
            </div>
            // Each mark sits exactly under the place of the knob for its value.
            <div class="scale js-only" aria-hidden="true">
                {[0u8, 6, 12, 18, 24, 30].iter().map(|mark| view! {
                    <span style=format!("--at:{:.4}", f64::from(*mark) / CREDITS_MAX)>{if *mark == 30 { "30+".to_string() } else { mark.to_string() }}</span>
                }).collect_view()}
            </div>
            <div class="range">
                <input type="number" name="ects_min" min="0" max="60" step="0.5" inputmode="decimal" aria-label="Leistungspunkte mindestens" placeholder="von"
                    value=query.with_untracked(|q| q.credits_min.map(|n| n.to_string()))
                    prop:value=move || query.with(|q| q.credits_min.map(|n| n.to_string()).unwrap_or_default())
                    on:change=move |ev| if APP { go.run(target_now(query, open, |q| q.credits_min = typed(&ev))) }/>
                <span>"–"</span>
                <input type="number" name="ects_max" min="0" max="60" step="0.5" inputmode="decimal" aria-label="Leistungspunkte höchstens" placeholder="bis"
                    value=query.with_untracked(|q| q.credits_max.map(|n| n.to_string()))
                    prop:value=move || query.with(|q| q.credits_max.map(|n| n.to_string()).unwrap_or_default())
                    on:change=move |ev| if APP { go.run(target_now(query, open, |q| q.credits_max = typed(&ev))) }/>
            </div>
        </div>
    }
}
