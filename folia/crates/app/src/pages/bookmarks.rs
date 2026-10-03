//! „Merkliste": the modules the visitor has marked (`crate::bookmarks`), as a list like the
//! catalog's, in the same frame: a sidebar as wide as the filter panel, the list, and the preview
//! of the module that is open (`/bookmarks?…&open=<id>`) floating at the right edge.
//!
//! A module opened here stays here (a local view, `crate::local`): „Vollbild" of the preview
//! shows the module's whole page in the list's place (`&full=1`), and on a phone a tap on a row
//! does, so the address, the tab, the history and „Zurück" stay the marked modules' — as on a
//! program's page — and the catalog's tab does not hear of the module.
//!
//! The page exists in the browser app only. Which modules are marked is known to this browser
//! and to nobody else: the server renders the same explanation for everybody (R9), the URL says
//! how the list is shown and never what is on it (R13), and everything the page shows about the
//! modules comes from the local copy of the catalog.
//!
//! - A module whose mark is taken away here stays where it is, dimmed, until the page is left:
//!   a slip is one click to undo, and the list does not jump under the pointer.
//! - Marking and unmarking change numbers, never the list: they need no query, and the rows (and
//!   the focus on them) stay the same elements.
//! - What the snapshot does not know (a module the BTU has taken out of its catalog) is named,
//!   not dropped: unknown stays unknown (R12).

use folia_model::rows::CatalogRow;
use folia_pages::ask::{BookmarksAsk, ModuleAsk};
use folia_pages::BookmarksData;
use folia_routes::url::{self, BookmarkSort, BookmarksUrl, LocalView, Season};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;

use crate::bookmarks::{ids_from_fragment, transfer_fragment, Bookmarks, BrokenLink, Mark, MarkButton, MarkLook};
use crate::data::{use_data, DataError};
use crate::format;
use crate::i18n::{self, use_location};
use crate::local::{self, ModuleInPlace};
use crate::nav;
use crate::pages::catalog::{phone_layout, ListKeys, Row};
use crate::pages::module::ModulePanel;
use crate::pending::{Change, Pending, Shape};
use crate::seo::Seo;
use crate::skeleton::{AppStandin, DetailSkeleton};
use crate::tabs::{self, Area, Tabs};
use crate::ui::{ErrorState, Icon};

/// Whether this build is the browser app: only there is anything marked.
const APP: bool = cfg!(feature = "csr");

const ROWS_ID: &str = "rows";
/// The page's one scroll area: the list and the ground after it.
const SCROLL_ID: &str = "bookmarks-scroll";

type Loaded = Result<BookmarksData, DataError>;

/// The tags of the Merkliste: a page of one visitor, the same address for everybody and nothing to
/// list; what a link preview shows is what the server writes.
#[component]
fn BookmarksSeo() -> impl IntoView {
    let t = i18n::t();
    view! { <Seo title=t.bookmarks.title description=t.bookmarks.description path=url::BOOKMARKS card=crate::seo::BOOKMARKS_CARD noindex=true/> }
}

#[component]
pub fn BookmarksPage() -> impl IntoView {
    let t = i18n::t();
    // A route of the app: the server writes its document, the app the page (§4.2).
    if !APP {
        return view! {
            <Title text=t.bookmarks.title/>
            <BookmarksSeo/>
            <AppStandin shape=Shape::Bookmarks title=t.bookmarks.server_title hint=t.bookmarks.server_hint/>
        }
        .into_any();
    }
    let location = use_location();
    // The server renders one page for every address of the list (its cache knows the page by its
    // path): there is no list to order or to filter there, and no module to show beside it.
    let url = Memo::new(move |_| if APP { BookmarksUrl::parse(&location.search.get()) } else { BookmarksUrl::default() });
    // Three independent parts of the URL, as in the catalog: the order is the only one that
    // needs a query, the half of the year picks from what is loaded, `open` is the preview.
    let order = Memo::new(move |_| url.with(|url| (url.sort, url.descending)));
    let season = Memo::new(move |_| url.with(|url| url.season));
    let open = Memo::new(move |_| url.with(|url| url.open.clone()));
    let bookmarks = Bookmarks::expect();
    let source = use_data();

    // What the page lists: what was marked when it was opened, and what has been marked since
    // (in another tab). A mark taken away here does not take the module off the page.
    let listed = RwSignal::new(bookmarks.map(|bookmarks| bookmarks.marks_untracked()).unwrap_or_default().into_iter().map(|mark| mark.id).collect::<Vec<_>>());
    Effect::new(move |_| {
        let Some(bookmarks) = bookmarks else { return };
        let new: Vec<String> = bookmarks.marks().into_iter().map(|mark| mark.id).filter(|id| listed.with_untracked(|listed| !listed.contains(id))).collect();
        if !new.is_empty() {
            listed.update(|listed| {
                listed.splice(0..0, new);
            });
        }
    });

    let list_source = source.clone();
    let data: Memo<Loaded> = Memo::new(move |before| {
        let (sort, descending) = order.get();
        let ids = listed.get();
        if ids.is_empty() {
            // Nothing marked (and on the server, always): nothing to ask the catalog.
            return Ok(BookmarksData::default());
        }
        DataError::or_before(list_source.clone().and_then(|source| source.now(&BookmarksAsk { ids, sort, descending })), before)
    });

    // What fills the page: the list, or the module opened from it where that is shown in full —
    // after „Vollbild", and on a phone, where nothing stands beside a page, whatever is opened
    // (`local`). What is listed stays meanwhile: a mark taken away on the module's page leaves the
    // module on the list, dimmed, as it does in the preview.
    let phone = phone_layout();
    let filling = Memo::new(move |_| url.with(|url| local::filling(url, phone.get())));
    let preview = Memo::new(move |before| {
        let now = match open.get() {
            None => Ok(None),
            Some(id) => source.clone().and_then(|source| source.now(&ModuleAsk { id })).map(Some),
        };
        DataError::or_before(now, before)
    });
    // Where the app is going (`pending`): the sidebar shows its order and half of the year at once,
    // the row of a module being opened is marked, a preview being closed is gone.
    let going = Pending::expect();
    let going_to = Memo::new(move |_| going.and_then(|going| going.search_on(url::BOOKMARKS)).map(|search| BookmarksUrl::parse(&search)));
    let shown_url = Memo::new(move |_| going_to.get().unwrap_or_else(|| url.get()));
    let going_open = Memo::new(move |_| going.filter(|going| going.change() == Some(Change::Preview)).and_then(|_| going_to.with(|to| to.as_ref().map(|to| to.open.clone()))));
    let marked = Memo::new(move |_| going_open.get().unwrap_or_else(|| open.get()));

    // Coming back to the list from a module's page — the module that filled the page here, or
    // its own page (a link on it) — the list shows the row the visitor left it at.
    let now = tabs::location_of(&location.pathname.get_untracked(), &location.search.get_untracked());
    let left_at = StoredValue::new(Tabs::expect().and_then(|tabs| tabs::page_below(&tabs.before(&now), "/catalog/module")));

    (move || {
        if let Some(id) = filling.get() {
            left_at.set_value(Some(id.clone()));
            let back = url.with_untracked(|url| local::back_href(url, phone.get_untracked()));
            return view! { <ModuleInPlace id area=Area::Bookmarks back/> }.into_any();
        }
        if let Some(id) = left_at.try_update_value(Option::take).flatten() {
            reveal_row_soon(id);
        }
        view! {
            <Title text=t.bookmarks.title/>
            // One scroll area with the ground at its end, as the catalog (app.css „one scroll area").
            <div class="work framed flowing" id=SCROLL_ID data-keep-scroll="rows">
                <BookmarksSeo/>
                <aside class="panel sidebar" id="sidebar" aria-label=t.bookmarks.title>
                    <div class="panel-head"><h2>{t.bookmarks.title}</h2></div>
                    <div class="body scroll" data-keep-scroll="sidebar"><Sidebar url=shown_url data/></div>
                </aside>
                <div class="resizer between js-only" data-action="resize-filters" role="separator" aria-orientation="vertical" aria-controls="sidebar" aria-label=t.ui.resize_sidebar tabindex="0"></div>
                <section class="panel list" aria-live="polite">
                    <Offer/>
                    {move || {
                        // The parts of the URL first, then what is derived from them (R16).
                        let (season, (sort, descending)) = (season.get(), order.get());
                        match data.get() {
                            Err(error) => view! { <ErrorState error/> }.into_any(),
                            Ok(data) => view! { <List data season sort descending open=marked phone/> }.into_any(),
                        }
                    }}
                    <i class="list-cap" aria-hidden="true"></i>
                </section>
                {move || {
                    match going_open.get() {
                        Some(None) => return ().into_any(),
                        Some(Some(_)) if going.is_some_and(|going| going.waits(Change::Preview)) => return view! { <DetailSkeleton calm=open.get_untracked().is_some()/> }.into_any(),
                        _ => {}
                    }
                    let here = url.get();
                    let close_href = here.with_open(None).path();
                    match preview.get() {
                        Ok(None) | Err(_) => ().into_any(),
                        Ok(Some(Some(data))) => {
                            // „Vollbild" stays among the marked modules: the module fills the list's place.
                            // Both are paths of the app, as `ModulePanel` takes them: the panel writes
                            // them as links.
                            let full_href = local::full_href(&here, &data.module.id);
                            view! {
                                <ModulePanel data close_href full_href=Some(full_href)/>
                                <div class="resizer preview-edge js-only" data-action="resize-preview" role="separator" aria-orientation="vertical" aria-controls="preview" aria-label=t.ui.resize_preview tabindex="0"></div>
                            }
                            .into_any()
                        }
                        Ok(Some(None)) => view! {
                            <section class="panel detail">
                                <div class="state">
                                    <p class="state-title">{t.catalog.module_not_found}</p>
                                    <p>{t.catalog.module_not_in_catalog}</p>
                                    <a class="btn secondary" href=t.path(&close_href)>{t.catalog.close_preview}</a>
                                </div>
                            </section>
                        }.into_any(),
                    }
                }}
                <crate::ground::Ground/>
            </div>
        }
        .into_any()
    })
    .into_any()
}

/// Scrolls the list to the row of this module once the list is there, and once more after the
/// browser has restored its own idea of the scroll position (a step back through the history).
fn reveal_row_soon(id: String) {
    // An effect runs in the browser only, after the list has been rendered.
    Effect::new(move |_| {
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

/// A list brought over from another device: it arrives in the fragment of the address
/// (`/bookmarks#m=…`, see `bookmarks::transfer_fragment`), which no browser sends to a server.
/// The page asks before it adds anything (a link must not fill somebody's list behind their
/// back), and either answer takes the list out of the address and out of the history entry. A
/// list whose check characters do not fit (a link cut short, a character typed wrong) is named,
/// and nothing of it is offered.
#[component]
fn Offer() -> impl IntoView {
    let t = i18n::t();
    let bookmarks = Bookmarks::expect();
    let location = use_location();
    // The fragment as the browser has it. The router hears of it when a page is opened, not when
    // only the fragment changes (a link pasted into the address of the open list), so the page
    // listens for that itself. Effects run in the browser only: the server offers nothing.
    let fragment = RwSignal::new(String::new());
    Effect::new(move |_| {
        location.hash.track();
        fragment.set(nav::fragment());
    });
    Effect::new(move |_| {
        let handle = window_event_listener_untyped("hashchange", move |_| fragment.set(nav::fragment()));
        on_cleanup(move || handle.remove());
    });
    let offered = Memo::new(move |_| ids_from_fragment(&fragment.get()));
    let navigate = use_navigate();
    let answered = Callback::new(move |add: bool| {
        if let (true, Some(bookmarks), Ok(ids)) = (add, bookmarks, offered.get_untracked()) {
            bookmarks.add_all(&ids);
        }
        // The same page without the fragment, in the same history entry.
        fragment.set(String::new());
        let here = tabs::location_of(&location.pathname.get_untracked(), &location.search.get_untracked());
        navigate(&here, NavigateOptions { replace: true, scroll: false, ..Default::default() });
    });
    move || {
        let ids = match offered.get() {
            Ok(ids) if ids.is_empty() => return None,
            Ok(ids) => ids,
            Err(BrokenLink) => {
                return Some(
                    view! {
                        <div class="offer" role="status">
                            <Icon name="info"/>
                            <p>
                                <b>{t.bookmarks.broken_link}" "</b>
                                {t.bookmarks.broken_link_hint}
                            </p>
                            <button class="mini hit" type="button" id="offer-dismiss" on:click=move |_| answered.run(false)>{t.bookmarks.ok}</button>
                        </div>
                    }
                    .into_any(),
                )
            }
        };
        let new = ids.iter().filter(|id| !bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(id))).count();
        Some(
            view! {
                <div class="offer" role="status">
                    <Icon name="bookmark"/>
                    <p>
                        <b>{(t.bookmarks.from_link)(ids.len())}" "</b>
                        {match new {
                            0 => t.bookmarks.all_there.to_string(),
                            n if n == ids.len() => t.bookmarks.add_question.to_string(),
                            n => (t.bookmarks.some_missing)(n),
                        }}
                    </p>
                    {(new > 0).then(|| view! { <button class="mini primary hit" type="button" id="offer-add" on:click=move |_| answered.run(true)>{t.bookmarks.add}</button> })}
                    <button class="mini hit" type="button" id="offer-dismiss" on:click=move |_| answered.run(false)>{if new > 0 { t.bookmarks.discard } else { t.bookmarks.ok }}</button>
                </div>
            }
            .into_any(),
        )
    }
}

/// What is marked among `rows`, as the header and the sidebar count it: how many modules, their
/// credits, and how many of them state none. Tracks the marks.
fn marked_numbers(bookmarks: Option<Bookmarks>, rows: &[(String, Option<f64>)]) -> (u64, f64, u64) {
    let marked: Vec<&(String, Option<f64>)> = rows.iter().filter(|(id, _)| bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(id))).collect();
    (marked.len() as u64, marked.iter().filter_map(|(_, credits)| *credits).sum(), marked.iter().filter(|(_, credits)| credits.is_none()).count() as u64)
}

#[component]
fn List(data: BookmarksData, season: Option<Season>, sort: BookmarkSort, descending: bool, open: Memo<Option<String>>, phone: RwSignal<bool>) -> impl IntoView {
    let t = i18n::t();
    let bookmarks = Bookmarks::expect();
    let rows: Vec<CatalogRow> = data.rows.iter().filter(|row| season.is_none_or(|season| data.offered_in(season, &row.id))).cloned().collect();
    // A module the snapshot does not know has no turnus: it is listed when nothing is filtered.
    let missing: Vec<String> = if season.is_none() { data.missing.clone() } else { Vec::new() };
    let nothing_marked = data.rows.is_empty() && data.missing.is_empty();
    let nothing_left = !nothing_marked && rows.is_empty() && missing.is_empty();
    let has_list = APP && !nothing_marked;

    let here = BookmarksUrl { season, sort, descending, ..Default::default() };
    let credits: Vec<(String, Option<f64>)> = rows.iter().map(|row| (row.id.clone(), row.credits)).collect();
    let missing_ids: Vec<(String, Option<f64>)> = missing.iter().map(|id| (id.clone(), None)).collect();
    let numbers = Memo::new(move |_| {
        let (known, credits, unstated) = marked_numbers(bookmarks, &credits);
        (known + marked_numbers(bookmarks, &missing_ids).0, credits, unstated)
    });

    let sort_link = |key: BookmarkSort, text: &'static str, class: &'static str| {
        let on = sort == key;
        let next = BookmarksUrl { sort: key, descending: on && !descending, ..here.clone() };
        let arrow = match (on, descending) {
            (true, false) => " ↑",
            (true, true) => " ↓",
            _ => "",
        };
        view! { <a class=class href=move || t.path(&next.with_open(open.get().as_deref()).path()) data-noscroll="" aria-current=on.then_some("true")>{text}{arrow}</a> }
    };
    let without_season = BookmarksUrl { season: None, ..here.clone() };
    let all_href = { let target = without_season.clone(); move || t.path(&target.with_open(open.get().as_deref()).path()) };

    view! {
        <div class="list-head">
            <div class="count-row">
                <span class="count num">{move || format::count(numbers.get().0, t.locale)}</span>
                <span class="count-label">
                    {move || (t.bookmarks.count_label)(numbers.get().0, season.map(|season| season.label(t.locale)))}
                    {move || {
                        let (marked, credits, _) = numbers.get();
                        (marked > 0).then(|| view! { <span class="count-more num">" · "{format::credits(Some(credits), t.locale)}</span> })
                    }}
                </span>
                <div class="list-tools">
                    {has_list.then(|| view! { <ListKeys/> })}
                </div>
            </div>
            <div class="active-filters">
                {season.map(|season| view! {
                    <span class="tag"><em>{t.catalog.turnus}</em>" "{season.label(t.locale)}<a href=all_href.clone() data-noscroll="" aria-label=t.catalog.remove_filter><Icon name="x"/></a></span>
                })}
            </div>
        </div>
        {has_list.then(|| view! {
            <div class="cols label">
                {sort_link(BookmarkSort::Title, t.catalog.col_module, "")}
                <span class="c-resp">{t.catalog.col_responsible}</span>
                <span class="c-exam">{t.catalog.exam}</span>
                {sort_link(BookmarkSort::Credits, t.common.credits_unit, "c-lp")}
                <span class="c-turnus">{t.catalog.turnus}</span>
                <span class="c-lang">{t.catalog.col_language}</span>
                {sort_link(BookmarkSort::Events, t.catalog.dates, "c-events")}
            </div>
        })}
        <div class="rows scroll" id=ROWS_ID>
            {(!APP).then(|| view! {
                <div class="state">
                    <p class="state-title">{t.bookmarks.server_title}</p>
                    <p>{t.bookmarks.server_hint}</p>
                    <a class="btn secondary" href=t.path(url::CATALOG)>{t.common.to_catalog}</a>
                </div>
            })}
            {(APP && nothing_marked).then(|| view! {
                <div class="state">
                    <p class="state-title">{t.bookmarks.empty_title}</p>
                    <p>{t.bookmarks.empty_hint_start}<kbd>"M"</kbd>{t.bookmarks.empty_hint_end}</p>
                    <a class="btn secondary" href=t.path(url::CATALOG)>{t.common.to_catalog}</a>
                </div>
            })}
            {nothing_left.then(|| {
                let season = season.map(|season| season.label(t.locale)).unwrap_or_default();
                view! {
                    <div class="state">
                        <p class="state-title">{(t.bookmarks.nothing_in_season)(season)}</p>
                        <p>{(t.bookmarks.nothing_in_season_hint)(season)}</p>
                        <a class="btn secondary" href=all_href.clone() data-noscroll="">{t.bookmarks.show_all_saved}</a>
                    </div>
                }
            })}
            {rows.into_iter().enumerate().map(|(index, row)| {
                let (target, id, here) = (row.id.clone(), row.id.clone(), here.clone());
                let preview = Signal::derive(move || here.with_open(Some(&target)).path());
                let current = Signal::derive(move || open.get().as_deref() == Some(id.as_str()));
                view! { <Row row preview current phone with_program=false dim_unmarked=true in_place=true shaded=index % 2 == 1 swipe=true/> }
            }).collect_view()}
            {(!missing.is_empty()).then(|| view! {
                <div class="sem">{t.bookmarks.not_in_catalog}</div>
                {missing.into_iter().map(|id| view! { <MissingRow id/> }).collect_view()}
            })}
        </div>
    }
}

/// A marked module the snapshot does not know: named, and its mark can be taken away.
#[component]
fn MissingRow(id: String) -> impl IntoView {
    let t = i18n::t();
    let bookmarks = Bookmarks::expect();
    let unmarked = {
        let id = id.clone();
        Memo::new(move |_| !bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(&id)))
    };
    view! {
        <div class="row-wrap" class:unmarked=move || unmarked.get()>
            <div class="row missing">
                <div class="t">
                    <b>{(t.bookmarks.module_numbered)(&id)}</b>
                    <small>{t.bookmarks.not_in_catalog_row}</small>
                </div>
            </div>
            <MarkButton id=id.clone() title=(t.bookmarks.module_numbered)(&id) look=MarkLook::Row/>
        </div>
    }
}

/// The sidebar: what belongs to the list as a whole (R17). Its numbers, which half of the year
/// it shows, its order, and what can be done with all of it.
#[component]
fn Sidebar(url: Memo<BookmarksUrl>, data: Memo<Loaded>) -> impl IntoView {
    let t = i18n::t();
    let bookmarks = Bookmarks::expect();
    let known = Memo::new(move |_| data.with(|data| data.as_ref().map(|data| data.rows.iter().map(|row| (row.id.clone(), row.credits)).collect::<Vec<_>>()).unwrap_or_default()));
    let numbers = Memo::new(move |_| {
        let missing = data.with(|data| data.as_ref().map(|data| data.missing.iter().filter(|id| bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(id))).count()).unwrap_or(0));
        let (marked, credits, unstated) = known.with(|known| marked_numbers(bookmarks, known));
        (marked + missing as u64, credits, unstated)
    });
    let in_season = move |season: Season| {
        data.with(|data| data.as_ref().map(|data| data.rows.iter().filter(|row| data.offered_in(season, &row.id) && bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(&row.id))).count()).unwrap_or(0))
    };
    let anything_listed = Memo::new(move |_| data.with(|data| data.as_ref().is_ok_and(|data| !data.rows.is_empty() || !data.missing.is_empty())));

    let season_link = move |season: Option<Season>, label: &'static str| {
        let count = move || match season {
            None => numbers.get().0,
            Some(season) => in_season(season) as u64,
        };
        view! {
            <a
                href=move || t.path(&BookmarksUrl { season, ..url.get() }.path())
                role="radio"
                draggable="false"
                data-noscroll=""
                aria-checked=move || if url.with(|url| url.season == season) { "true" } else { "false" }
            >
                <span class="seg-label">{label}</span><span class="num">{move || format::count(count(), t.locale)}</span>
            </a>
        }
    };
    let order_link = move |sort: BookmarkSort| {
        view! {
            <a
                href=move || t.path(&BookmarksUrl { sort, descending: false, ..url.get() }.path())
                draggable="false"
                data-noscroll=""
                aria-current=move || url.with(|url| url.sort == sort).then_some("page")
            >
                {sort.label(t.locale)}
            </a>
        }
    };

    // The list as text, for a note or a message: number, title, credits (in the page's language).
    let as_text = move || {
        data.with(|data| {
            let Ok(data) = data else { return String::new() };
            let lines: Vec<String> = data
                .rows
                .iter()
                .filter(|row| bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(&row.id)))
                .map(|row| match row.credits {
                    Some(credits) => format!("{}\t{}\t{}", row.id, row.title, format::credits(Some(credits), t.locale)),
                    None => format!("{}\t{}", row.id, row.title),
                })
                .collect();
            lines.join("\n")
        })
    };

    // The list for another device: a link to this page with the marked modules in its fragment,
    // opening there in the language of this page; none for a list no code holds, and then no
    // action.
    let transfer_link = Memo::new(move |_| {
        let ids: Vec<String> = bookmarks.map(|bookmarks| bookmarks.marks()).unwrap_or_default().into_iter().map(|mark| mark.id).collect();
        transfer_fragment(&ids).map(|fragment| t.path(&format!("{}#{fragment}", url::BOOKMARKS)))
    });
    let transferable = Memo::new(move |_| transfer_link.with(Option::is_some));

    // Emptying the list asks first, and can be taken back afterwards.
    let confirming = RwSignal::new(false);
    let cleared = RwSignal::new(None::<Vec<Mark>>);
    let ask = move |_| {
        confirming.set(true);
        request_animation_frame(|| nav::focus_by_id("clear-yes"));
    };
    let clear = move |_| {
        confirming.set(false);
        if let Some(bookmarks) = bookmarks {
            cleared.set(Some(bookmarks.clear()));
            request_animation_frame(|| nav::focus_by_id("clear-undo"));
        }
    };
    let undo = move |_| {
        if let (Some(bookmarks), Some(marks)) = (bookmarks, cleared.get_untracked()) {
            bookmarks.restore(marks);
        }
        cleared.set(None);
    };

    view! {
        {move || anything_listed.get().then(|| view! {
            <div class="fgroup first">
                <p class="flabel label">{t.bookmarks.overview}</p>
                <dl class="side-facts">
                    <div>
                        <dt>{t.bookmarks.saved}</dt>
                        <dd class="num">{move || format::modules(i64::try_from(numbers.get().0).unwrap_or(i64::MAX), t.locale)}</dd>
                    </div>
                    <div>
                        <dt>{t.bookmarks.credit_points}</dt>
                        <dd class="num">
                            {move || format::credits(Some(numbers.get().1), t.locale)}
                            {move || match numbers.get().2 { 0 => None, n => Some(view! { <small>" · "{(t.bookmarks.unstated)(n)}</small> }) }}
                        </dd>
                    </div>
                </dl>
            </div>
            <div class="fgroup">
                <p class="flabel label">{t.bookmarks.offered_in}</p>
                <div class="seg" role="radiogroup" aria-label=t.bookmarks.offered_in>
                    {season_link(None, t.bookmarks.all)}
                    {season_link(Some(Season::Winter), Season::Winter.label(t.locale))}
                    {season_link(Some(Season::Summer), Season::Summer.label(t.locale))}
                </div>
            </div>
            <nav class="toc fgroup" aria-label=t.bookmarks.order>
                <p class="flabel label">{t.bookmarks.order}</p>
                {BookmarkSort::ALL.iter().map(|sort| order_link(*sort)).collect_view()}
            </nav>
            <div class="fgroup actions">
                <p class="flabel label">{t.bookmarks.actions}</p>
                <a class="action" href="#" data-action="copy-text" data-text=as_text><Icon name="copy"/><span>{t.bookmarks.copy_list}</span></a>
                {move || transferable.get().then(|| view! {
                    <a class="action" href="#" data-action="copy-text" data-absolute="" data-text=move || transfer_link.get().unwrap_or_default() title=t.bookmarks.transfer_title>
                        <Icon name="share-2"/><span><span data-label="">{t.bookmarks.transfer}</span><small>{t.bookmarks.transfer_hint}</small></span>
                    </a>
                })}
                {move || match (cleared.get().is_some(), confirming.get()) {
                    (true, _) => view! {
                        <p class="action note-action"><Icon name="check"/><span>{t.bookmarks.cleared}</span><button class="mini hit" type="button" id="clear-undo" on:click=undo>{t.common.undo}</button></p>
                    }.into_any(),
                    (false, true) => view! {
                        <p class="action note-action ask"><span>{t.bookmarks.clear_question}</span><button class="mini danger hit" type="button" id="clear-yes" on:click=clear>{t.bookmarks.clear_yes}</button><button class="mini hit" type="button" on:click=move |_| confirming.set(false)>{t.bookmarks.cancel}</button></p>
                    }.into_any(),
                    (false, false) => (numbers.get().0 > 0).then(|| view! {
                        <button class="action" type="button" on:click=ask><Icon name="trash-2"/><span>{t.bookmarks.clear}</span></button>
                    }).into_any(),
                }}
            </div>
        })}
        <div class="fgroup" class:first=move || !anything_listed.get()>
            <p class="hint storage-hint"><Icon name="shield-check"/><span>{t.bookmarks.storage_hint}</span></p>
        </div>
    }
}
