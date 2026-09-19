//! The module catalog: filters, list and (when a module is open) its detail panel, side by side.
//!
//! The URL is the whole state: `/catalog?…` is the list, `…&open=<id>` the same list with that
//! module previewed next to it (its own page is `/catalog/module/<id>`). Filters are a plain GET form, every other control is a link,
//! so the page works without JavaScript; with it, `enhance.js` submits the form on change.

use catalog::filter::{CatalogQuery, ExamPart, KindFilter, Language, PlanSemesterFilter, ProgramRelation, SortKey};
use catalog::labels::{Campus, Code, Labelled, ModuleKind, OfferStatus, TeachingForm, TurnusParity, TurnusSeason};
use catalog::pages::{self, CatalogData};
use catalog::rows::CatalogRow;
use catalog::url::{self, CatalogUrl, ProgramTab, PAGE_SIZE};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;

use crate::data::{use_source, PageStatus};
use crate::format;
use crate::nav::{form_query, keep_position_after_prepend, list_height, list_position, FormEvent};
use crate::pages::module::ModulePanel;
use crate::ui::{ErrorState, Icon, KindBadge, OfferBadge};

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
            {move || match list.get() {
                Err(error) => {
                    status.for_error(&error);
                    view! { <div class="page"><ErrorState error/></div> }.into_any()
                }
                Ok((current, data)) => view! {
                    <Filters current=current.clone() data=data.clone() open/>
                    <List current data open page/>
                }.into_any(),
            }}
            {move || {
                let close_href = url.get().with_open(None).path();
                match preview.get() {
                    Ok(None) | Err(_) => ().into_any(),
                    Ok(Some(Some(data))) => view! { <ModulePanel data close_href/> }.into_any(),
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
        for kind in scope.kinds.clone() {
            let label = match kind {
                KindFilter::Stated(kind) => kind.label().to_string(),
                KindFilter::Unstated => "nicht angegeben".to_string(),
            };
            push("Art", label, &move |q| {
                if let Some(s) = q.program.as_mut() {
                    s.kinds.retain(|k| *k != kind);
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
    if let Some(parity) = q.turnus.year_parity {
        push("Jahre", parity.label().to_string(), &|q| q.turnus.year_parity = None);
    }
    for form in q.teaching_forms.clone() {
        push("Lehrform", form.label().to_string(), &move |q| q.teaching_forms.retain(|f| *f != form));
    }
    for part in q.exam_parts.clone() {
        push("Prüfung", part.label().to_string(), &move |q| q.exam_parts.retain(|p| *p != part));
    }
    for form in q.exam_forms.clone() {
        push("Prüfung", format::exam_short(&Code::Known(form)), &move |q| q.exam_forms.retain(|f| *f != form));
    }
    for language in q.languages.clone() {
        let label = if language == Language::German { "Deutsch" } else { "Englisch" };
        push("Sprache", label.to_string(), &move |q| q.languages.retain(|l| *l != language));
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
    if q.offer.is_some() {
        push("Status", "auch nicht mehr angebotene".to_string(), &|q| q.offer = None);
    }
    out
}

#[component]
fn List(current: CatalogUrl, data: CatalogData, open: Memo<Option<String>>, page: Memo<u64>) -> impl IntoView {
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
    let chunks = RwSignal::new(vec![Chunk { page: start_page, rows: data.page.rows.clone(), continues: None }]);
    let source = use_source();
    let query = StoredValue::new(current.query.clone());
    let load = move |page_no: u64| -> Option<Vec<CatalogRow>> {
        let source = source.clone().ok()?;
        source.run(|db| catalog::queries::catalog_page(db, &query.get_value(), (page_no - 1) * PAGE_SIZE, PAGE_SIZE)).ok().map(|p| p.rows)
    };
    let load_next = {
        let load = load.clone();
        move || {
            let Some((last_page, last_group)) = chunks.with_untracked(|c| c.last().map(|c| (c.page, c.rows.last().map(|r| r.plan_semester)))) else { return };
            if last_page >= pages_total {
                return;
            }
            if let Some(rows) = load(last_page + 1) {
                chunks.update(|c| c.push(Chunk { page: last_page + 1, rows, continues: last_group }));
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
            chunks.update(|c| c.insert(0, Chunk { page: first_page - 1, rows, continues: None }));
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
                    <ChunkRows chunk base=chunk_base.clone() by_plan with_program open page/>
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
fn Row(row: CatalogRow, base: CatalogUrl, open: Memo<Option<String>>, page: Memo<u64>, with_program: bool) -> impl IntoView {
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
    let href = move || base.with_page(page.get()).with_open(Some(&target)).path();
    view! {
        <a class="row" href=href aria-current=move || (open.get().as_deref() == Some(id.as_str())).then_some("true")>
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

/// One page of the list. `continues` is the plan semester the previous chunk ended with, so a
/// group that runs across two chunks gets its header only once.
#[derive(Clone, PartialEq)]
struct Chunk {
    page: u64,
    rows: Vec<CatalogRow>,
    continues: Option<Option<i64>>,
}

const ROWS_ID: &str = "rows";

#[component]
fn ChunkRows(chunk: Chunk, base: CatalogUrl, by_plan: bool, with_program: bool, open: Memo<Option<String>>, page: Memo<u64>) -> impl IntoView {
    // Group headers follow the study plan when the list is in plan order.
    let mut last_group = chunk.continues;
    let rows = chunk
        .rows
        .iter()
        .map(|row| {
            let header = (by_plan && last_group != Some(row.plan_semester)).then(|| match row.plan_semester {
                Some(n) => format!("{n}. Semester"),
                None => "Ohne Semesterangabe im Regelstudienplan".to_string(),
            });
            last_group = Some(row.plan_semester);
            view! {
                {header.map(|text| view! { <div class="sem">{text}</div> })}
                <Row row=row.clone() base=base.clone() open page with_program/>
            }
        })
        .collect_view();
    view! { <div class="chunk" data-page=chunk.page>{rows}</div> }
}

/// The filter panel: a GET form whose field names are the URL parameters of `CatalogUrl`.
#[component]
fn Filters(current: CatalogUrl, data: CatalogData, open: Memo<Option<String>>) -> impl IntoView {
    let q: CatalogQuery = current.query.clone();
    let scope = q.program.clone();
    let chip = |name: &'static str, value: &'static str, label: String, checked: bool, icon: Option<&'static str>| {
        view! {
            <label class="chip">
                <input type="checkbox" name=name value=value checked=checked/>
                {icon.map(|name| view! { <Icon name=name/> })}
                {label}
            </label>
        }
    };
    let option = |value: String, label: String, selected: bool| view! { <option value=value selected=selected>{label}</option> };
    let tri = |name: &'static str, label: &'static str, state: Option<bool>, yes: (&'static str, &'static str), no: (&'static str, &'static str)| {
        view! {
            <label class="field">
                <span>{label}</span>
                <span class="select-wrap plain">
                    <select name=name>
                        {option(String::new(), "egal".to_string(), state.is_none())}
                        {option(yes.0.to_string(), yes.1.to_string(), state == Some(true))}
                        {option(no.0.to_string(), no.1.to_string(), state == Some(false))}
                    </select>
                    <Icon name="chevrons-up-down"/>
                </span>
            </label>
        }
    };

    let has_plan = data.program.as_ref().is_some_and(|p| p.has_plan);
    let selected_slug = scope.as_ref().map(|s| s.program_slug.clone()).unwrap_or_default();
    let kinds = scope.as_ref().map(|s| s.kinds.clone()).unwrap_or_default();
    let semester = scope.as_ref().and_then(|s| s.plan_semester);
    let fues_list = scope.as_ref().is_some_and(|s| s.relation == ProgramRelation::Fues);
    let sort_code = match q.sort {
        SortKey::Default => None,
        SortKey::Title => Some("title"),
        SortKey::Id => Some("id"),
        SortKey::Credits => Some("ects"),
        SortKey::Events => Some("events"),
    };
    let show_all_status = q.offer.as_ref().is_some_and(|offer| offer.contains(&OfferStatus::NotOffered));
    let more_open = !q.lecturers_include.is_empty()
        || !q.lecturers_exclude.is_empty()
        || q.department_id.is_some()
        || q.duration_semesters.is_some()
        || q.limited.is_some()
        || q.fues.is_some()
        || q.graded.is_some()
        || !q.campuses.is_empty()
        || q.offer.is_some()
        || q.turnus.year_parity.is_some();
    let relation_link = |relation: ProgramRelation, label: &'static str, count: Option<u64>| {
        let mut next = current.with_page(1);
        if let Some(scope) = next.query.program.as_mut() {
            scope.relation = relation;
        }
        let on = scope.as_ref().is_some_and(|s| s.relation == relation);
        view! { <a href=keep_open(next, open) aria-current=on.then_some("true")>{label}<span>{count.map(format::count)}</span></a> }
    };

    // In the browser app a change of the form is a navigation, not a page load.
    let navigate = use_navigate();
    let go = move |ev: leptos::ev::Event, kind: FormEvent| {
        if let Some(query) = form_query(&ev, kind) {
            navigate(&CatalogUrl::parse(&query).path(), NavigateOptions { scroll: false, ..Default::default() });
        }
    };
    let go_on_submit = go.clone();

    view! {
        <aside class="panel filters" id="filters" aria-label="Filter">
            <form method="get" action=url::CATALOG data-autosubmit="" on:change=move |ev| go(ev, FormEvent::Change) on:submit=move |ev| go_on_submit(ev.into(), FormEvent::Submit)>
                <div class="panel-head">
                    <h2>"Filter"</h2>
                    <a class="ghost" href=keep_open(CatalogUrl::default(), open)><Icon name="rotate-ccw"/>"Zurücksetzen"</a>
                    <a class="icon-btn sheet-close" href="#" data-action="sheet-close" aria-label="Filter schließen"><Icon name="x"/></a>
                </div>
                <div class="body scroll" data-keep-scroll="filters">
                    {move || open.get().map(|id| view! { <input type="hidden" name="open" value=id/> })}
                    {fues_list.then(|| view! { <input type="hidden" name="list" value="fues"/> })}
                    {sort_code.map(|code| view! { <input type="hidden" name="sort" value=code/> })}
                    {q.descending.then(|| view! { <input type="hidden" name="desc" value="1"/> })}
                    {(!q.text.trim().is_empty()).then(|| view! { <input type="hidden" name="q" value=q.text.clone()/> })}

                    <label class="select-wrap">
                        <Icon name="graduation-cap"/>
                        <span class="visually-hidden">"Studiengang"</span>
                        <select name="program">
                            {option(String::new(), "Alle Studiengänge".to_string(), selected_slug.is_empty())}
                            {data.programs.iter().map(|p| {
                                let label = format!("{} · {} · PO {}", p.name, p.degree(), p.po_version);
                                option(p.slug.clone(), label, p.slug == selected_slug)
                            }).collect_view()}
                        </select>
                        <Icon name="chevrons-up-down"/>
                    </label>
                    {data.program.is_some().then(|| view! {
                        <nav class="seg" aria-label="Liste">
                            {relation_link(ProgramRelation::Curricular, "Curriculum", data.curricular_total)}
                            {relation_link(ProgramRelation::Fues, "FÜS", data.fues_total)}
                        </nav>
                    })}

                    {data.program.is_some().then(|| view! {
                        <div class="fgroup">
                            <div class="flabel label">"Modulart"</div>
                            <div class="chips">
                                {[ModuleKind::Compulsory, ModuleKind::Elective, ModuleKind::Thesis, ModuleKind::Internship].iter()
                                    .map(|kind| chip("kind", kind.code(), kind.label().to_string(), kinds.contains(&KindFilter::Stated(*kind)), None)).collect_view()}
                                {chip("kind", "none", "Nicht angegeben".to_string(), kinds.contains(&KindFilter::Unstated), None)}
                            </div>
                            {if has_plan {
                                view! {
                                    <label class="field">
                                        <span>"Fachsemester laut Regelstudienplan"</span>
                                        <span class="select-wrap plain">
                                            <select name="semester">
                                                {option(String::new(), "alle".to_string(), semester.is_none())}
                                                {(1u8..=10).map(|n| option(n.to_string(), format!("{n}. Semester"), semester == Some(PlanSemesterFilter::Semester(n)))).collect_view()}
                                                {option("none".to_string(), "ohne Angabe im Plan".to_string(), semester == Some(PlanSemesterFilter::Unstated))}
                                            </select>
                                            <Icon name="chevrons-up-down"/>
                                        </span>
                                    </label>
                                }.into_any()
                            } else {
                                view! { <p class="hint">"Für diesen Studiengang liegt kein geprüfter Regelstudienplan vor, Fachsemester sind deshalb nicht bekannt."</p> }.into_any()
                            }}
                        </div>
                    }.into_any())}

                    {view! {
                        <div class="fgroup">
                            <div class="flabel label">"Angeboten im"</div>
                            <div class="chips">
                                {chip("turnus", "winter", "Winter".to_string(), q.turnus.winter, Some("snowflake"))}
                                {chip("turnus", "summer", "Sommer".to_string(), q.turnus.summer, Some("sun"))}
                                {chip("turnus", "irregular", "Unregelmäßig".to_string(), q.turnus.irregular, None)}
                            </div>
                        </div>
                        <div class="fgroup">
                            <div class="flabel label">"Lehrform"</div>
                            <div class="chips">
                                {[TeachingForm::Lecture, TeachingForm::Exercise, TeachingForm::Seminar, TeachingForm::Practical, TeachingForm::Project, TeachingForm::Excursion]
                                    .iter().map(|form| chip("form", form.code(), form.label().to_string(), q.teaching_forms.contains(form), None)).collect_view()}
                            </div>
                        </div>
                    }.into_any()}

                    {view! {
                        <details class="fgroup" open=!q.exam_parts.is_empty()>
                            <summary class="label">"Prüfung"</summary>
                            <div class="chips">
                                {ExamPart::ALL.iter().map(|part| chip("exam", part.code(), part.label().to_string(), q.exam_parts.contains(part), None)).collect_view()}
                            </div>
                        </details>
                        <div class="fgroup">
                            <div class="flabel label">"Leistungspunkte"</div>
                            <div class="range">
                                <input type="number" name="ects_min" min="0" max="60" step="0.5" inputmode="decimal" aria-label="Leistungspunkte mindestens" placeholder="von" value=q.credits_min.map(|n| n.to_string())/>
                                <span>"–"</span>
                                <input type="number" name="ects_max" min="0" max="60" step="0.5" inputmode="decimal" aria-label="Leistungspunkte höchstens" placeholder="bis" value=q.credits_max.map(|n| n.to_string())/>
                            </div>
                        </div>
                        <div class="fgroup">
                            <div class="flabel label">"Sprache"</div>
                            <div class="chips">
                                {chip("lang", "de", "Deutsch".to_string(), q.languages.contains(&Language::German), None)}
                                {chip("lang", "en", "Englisch".to_string(), q.languages.contains(&Language::English), None)}
                            </div>
                        </div>
                    }.into_any()}

                    {view! {
                        <details class="fgroup more" open=more_open>
                            <summary class="label">"Weitere Filter"</summary>
                            <label class="field">
                                <span>"Lehrt oder verantwortet"</span>
                                <input type="text" name="lecturer" list="lecturers" value=q.lecturers_include.first().cloned().unwrap_or_default() placeholder="Nachname, Vorname"/>
                            </label>
                            {q.lecturers_include.iter().skip(1).map(|name| view! { <input type="hidden" name="lecturer" value=name.clone()/> }).collect_view()}
                            <label class="field">
                                <span>"Nicht bei"</span>
                                <input type="text" name="not-lecturer" list="lecturers" value=q.lecturers_exclude.first().cloned().unwrap_or_default() placeholder="Nachname, Vorname"/>
                            </label>
                            {q.lecturers_exclude.iter().skip(1).map(|name| view! { <input type="hidden" name="not-lecturer" value=name.clone()/> }).collect_view()}
                            <datalist id="lecturers">
                                {data.lecturers.iter().map(|l| view! { <option value=l.name.clone()></option> }).collect_view()}
                            </datalist>
                            <label class="field">
                                <span>"Fachgebiet"</span>
                                <span class="select-wrap plain">
                                    <select name="department">
                                        {option(String::new(), "alle".to_string(), q.department_id.is_none())}
                                        {data.departments.iter().map(|d| option(d.id.to_string(), format!("{} ({})", d.label, d.modules), q.department_id == Some(d.id))).collect_view()}
                                    </select>
                                    <Icon name="chevrons-up-down"/>
                                </span>
                            </label>
                            {tri("graded", "Benotung", q.graded, ("yes", "benotet"), ("no", "unbenotet"))}
                            {tri("limited", "Teilnehmerbegrenzung", q.limited, ("yes", "nur begrenzte Module"), ("no", "nur unbegrenzte Module"))}
                            {tri("fues", "Allgemeine FÜS-Liste", q.fues, ("only", "nur FÜS-Module"), ("none", "keine FÜS-Module"))}
                            <label class="field">
                                <span>"Dauer"</span>
                                <span class="select-wrap plain">
                                    <select name="duration">
                                        {option(String::new(), "egal".to_string(), q.duration_semesters.is_none())}
                                        {option("1".to_string(), "1 Semester".to_string(), q.duration_semesters == Some(1))}
                                        {option("2".to_string(), "2 Semester".to_string(), q.duration_semesters == Some(2))}
                                    </select>
                                    <Icon name="chevrons-up-down"/>
                                </span>
                            </label>
                            <label class="field">
                                <span>"Nur in bestimmten Jahren"</span>
                                <span class="select-wrap plain">
                                    <select name="years">
                                        {option(String::new(), "egal".to_string(), q.turnus.year_parity.is_none())}
                                        {option(TurnusParity::Even.code().to_string(), "in geraden Jahren angeboten".to_string(), q.turnus.year_parity == Some(TurnusParity::Even))}
                                        {option(TurnusParity::Odd.code().to_string(), "in ungeraden Jahren angeboten".to_string(), q.turnus.year_parity == Some(TurnusParity::Odd))}
                                    </select>
                                    <Icon name="chevrons-up-down"/>
                                </span>
                            </label>
                            <div class="flabel label">"Standort"</div>
                            <div class="chips">
                                {[Campus::Zentralcampus, Campus::Sachsendorf, Campus::Senftenberg]
                                    .iter().map(|campus| chip("campus", campus.code(), campus.label().to_string(), q.campuses.contains(campus), None)).collect_view()}
                            </div>
                            <p class="hint">"Der Standort ist nur für Module mit Raumangaben in diesem Semester bekannt. Dieser Filter zeigt deshalb nur solche Module."</p>
                            {data.program.is_none().then(|| view! {
                                <div class="chips">{chip("status", "all", "Auch nicht mehr angebotene Module".to_string(), show_all_status, None)}</div>
                            })}
                        </details>
                    }.into_any()}
                </div>
                <div class="filter-actions"><button class="btn primary" type="submit">"Filter anwenden"</button></div>
            </form>
        </aside>
    }
}
