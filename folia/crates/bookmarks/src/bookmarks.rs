//! „Merkliste": the modules the visitor has marked (`folia_stores::bookmarks`), as a list like the
//! catalog's, in the same frame: a sidebar as wide as the filter panel (on a phone a sheet behind
//! „Filter"), the list, and the preview of the module that is open (`/bookmarks?…&open=<id>`)
//! floating at the right edge (folia/design/merken/oberflaeche.html, „Suchen, ordnen, filtern").
//!
//! A module opened here stays here (a local view, `folia_widgets::local`): „Vollbild" of the preview
//! shows the module's whole page in the list's place (`&full=1`), and on a phone a tap on a row
//! does, so the address, the tab, the history and „Zurück" stay the marked modules' — as on a
//! program's page — and the catalog's tab does not hear of the module.
//!
//! The page exists in the browser app only. Which modules are marked is known to this browser
//! and to nobody else: the server renders the same explanation for everybody (R9), the URL says
//! how the list is shown and never what is on it (R13), what is typed into the search at the head
//! stays out of it too (`folia_shell::search::LocalSearch`, R20), and everything the page shows
//! about the modules comes from the local copy of the catalog.
//!
//! - The search at the head searches the marked modules as the catalog searches
//!   (`pages::bookmarks`), and a line under what it found leads into the whole catalog.
//! - The order is at the heads of the columns, as in the catalog („Gemerkt" over the bookmarks);
//!   „Zuletzt gemerkt", „Nach Studium" and „Nach Angebot" fall into sections. On a phone the
//!   sheet of the filters lists the orders.
//! - The filters are a few that separate marked modules: the half of the year, „Stand" in „Mein
//!   Studium", „Zählt in meinem Studium", „Passt in meinen Stundenplan", and four of the catalog's,
//!   folded (owner, 2026-10-19: „hier reichen ein paar, die wirklich relevant sind"). Each says how
//!   many it holds. On a phone their sheet is a draft that only „N Module anzeigen" applies.
//! - Several rows at once (`pick`), selected as in „Mein Studium": a bar in the place of the heads
//!   of the columns with their collections, „Einplanen", „Teilen" and „Entfernen".
//! - A module whose mark is taken away here stays where it is, dimmed, until the page is left:
//!   a slip is one click to undo, and the list does not jump under the pointer.
//! - Marking and unmarking change numbers, never the list: they need no query, and the rows (and
//!   the focus on them) stay the same elements. Planning one changes the mark of „Mein Studium" at
//!   its row, and the list only where „Stand" filters it.
//! - What the snapshot does not know (a module the BTU has taken out of its catalog) is named,
//!   not dropped: unknown stays unknown (R12).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::day::Day;
use folia_calendar::semester::SemesterKey;
use folia_model::labels::{Campus, Code, Labelled, ModuleKind, TurnusSeason};
use folia_model::rows::CatalogRow;
use folia_pages::ask::{BookmarkPlacesAsk, BookmarksAsk, CatalogSummaryAsk, FitAsk, MetaAsk, ModuleAsk, NewestSemestersAsk};
use folia_pages::{BookmarksData, Place};
use folia_plans::studyplan::PlanDoc;
use folia_routes::filter::{CatalogQuery, FitIds, FitsFilter, Language};
use folia_routes::url::{self, BookmarkSort, BookmarksUrl, CatalogUrl, CollectionView, LocalView, Season, Stand};
use leptos::prelude::*;
use folia_design::stage::Title;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;

use folia_stores::bookmarks::{brought_from_fragment, share_fragment, Bookmarks, BrokenLink, Brought, Mark, MarkButton, MarkLook, MAX_MARKS};
use folia_stores::collections::{Anchor, Collections, Refused, Shelf};
use folia_widgets::marknote::{Notice, Notices};
use folia_stores::myprogram::{MineResolved, MyProgram};
use folia_stores::studyplan::{PlanHint, Studyplan};
use folia_data::{use_ask, use_data, DataClient, DataError};
use folia_design::format;
use crate::i18n::{self, use_location, Locale};
use folia_widgets::finder::{finder_on, FitView, Finder};
use folia_widgets::local::ModuleInPlace;
use folia_design::nav;
use folia_widgets::list::{phone_layout, ListKeys, Row, RowMine, RowStatus};
use folia_widgets::module::ModulePanel;
use folia_shell::pending::{Change, Pending, Shape};
use folia_shell::search::LocalSearch;
use folia_shell::seo::Seo;
use folia_shell::skeleton::{AppStandin, DetailSkeleton};
use folia_shell::tabs::{self, Area, Tabs};
use folia_shell::frame::ErrorState;
use folia_design::keys::Keys;
use folia_design::ui::{Hit, Icon, ToggleLink};
use crate::pick::{self, Listed, PickBar, PickMenus, PickRow, Picking};

/// Whether this build is the browser app: only there is anything marked.
const APP: bool = cfg!(feature = "csr");

pub(crate) const ROWS_ID: &str = "rows";
/// From how many marks the head of the whole list says how near the limit it is, and from how many
/// as a warning (`MAX_MARKS`).
const LIMIT_SHOWN: u64 = 240;
const LIMIT_WARN: u64 = 250;
/// The page's one scroll area: the list and the ground after it.
const SCROLL_ID: &str = "bookmarks-scroll";

/// The data worker's answer for the list, with the text it searched: what the page shows of a
/// search is what was found for that text, never the rows before it under the words after it.
#[derive(Clone, Debug, Default, PartialEq)]
struct Answered {
    text: String,
    data: BookmarksData,
}

type Loaded = Result<Answered, DataError>;
type Texts = i18n::Texts;

/// The tags of the Merkliste: a page of one visitor, the same address for everybody and nothing to
/// list; what a link preview shows is what the server writes.
#[component]
fn BookmarksSeo() -> impl IntoView {
    let t = i18n::t();
    view! { <Seo title=t.bookmarks.title description=t.bookmarks.description path=url::BOOKMARKS card=folia_shell::seo::BOOKMARKS_CARD noindex=true/> }
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
    let url = Memo::new(move |_| BookmarksUrl::parse(&location.search.get()));
    let open = Memo::new(move |_| url.with(|url| url.open.clone()));
    let bookmarks = Bookmarks::expect();
    let source = use_data();

    // What the page lists: what was marked when it was opened, and what has been marked since
    // (in another tab), each with when it was marked. A mark taken away here does not take the
    // module off the page.
    let listed = RwSignal::new(bookmarks.map(Bookmarks::marks_untracked).unwrap_or_default());
    Effect::new(move |_| {
        let Some(bookmarks) = bookmarks else { return };
        let new: Vec<Mark> = bookmarks.marks().into_iter().filter(|mark| listed.with_untracked(|listed| !listed.iter().any(|kept| kept.id == mark.id))).collect();
        if !new.is_empty() {
            listed.update(|listed| {
                listed.splice(0..0, new);
            });
        }
    });
    // Another profile (chosen here or in another tab, `folia_stores::profiles`): its marks are the
    // list, as if the page had been opened with them; nothing of the other profile's stays on it.
    Effect::new(move |before: Option<u16>| {
        let profile = bookmarks.map_or(0, Bookmarks::profile);
        if before.is_some_and(|before| before != profile) {
            listed.set(bookmarks.map(Bookmarks::marks_untracked).unwrap_or_default());
        }
        profile
    });
    let ids = Memo::new(move |_| listed.with(|listed| listed.iter().map(|mark| mark.id.clone()).collect::<Vec<_>>()));
    let marked_at = Memo::new(move |_| listed.with(|listed| listed.iter().map(|mark| (mark.id.clone(), mark.at)).collect::<BTreeMap<_, _>>()));
    // What is marked now: the numbers count it, as the head does, and not what was unmarked here.
    let marked_now = Memo::new(move |_| bookmarks.map(|bookmarks| bookmarks.marks().into_iter().map(|mark| mark.id).collect::<BTreeSet<_>>()).unwrap_or_default());

    // What is typed into the search at the head: the page's, never the address's.
    let search = LocalSearch::expect();
    let text = Memo::new(move |_| search.map(|search| search.text().trim().to_string()).unwrap_or_default());

    // What only the visitor knows of the modules: their collections and notes
    // (`folia_stores::collections`), in this browser alone.
    let collections = Collections::expect();
    let shelf = Memo::new(move |_| collections.map(|collections| collections.with(Clone::clone)).unwrap_or_default());
    // What the search finds there (konzept.html: „Die Suche findet sie. „Lena" findet, wobei „Lena
    // fragen" notiert ist"), in the browser: the data worker lists it with what it found itself.
    let also = Memo::new(move |_| {
        let text = text.get();
        if text.is_empty() {
            return Vec::new();
        }
        shelf.with(|shelf| ids.with(|ids| ids.iter().filter(|id| shelf.kept_text(id).is_some_and(|kept| folia_search::matches(&kept, &text))).cloned().collect()))
    });

    // „Mein Studiengang", while the catalog knows it in this PO: what „Nach Studium" orders by and
    // what „Zählt in meinem Studium" keeps (`pages::bookmark_places`).
    let mine = MyProgram::expect();
    let resolved = MineResolved::expect();
    let program = Memo::new(move |_| resolved.and_then(MineResolved::exact).map(|program| program.slug));
    let places_source = source.clone().ok();
    let places = Memo::new(move |before: Option<&Option<BTreeMap<String, Place>>>| {
        let slug = program.get()?;
        let ids = ids.get();
        match places_source.as_ref()?.now(&BookmarkPlacesAsk { ids, program: slug }) {
            Ok(places) => Some(places.into_iter().map(|place| (place.id.clone(), place)).collect()),
            Err(error) if error.is_pending() => before.cloned().flatten(),
            Err(_) => None,
        }
    });

    // What „Mein Studium" says of each marked module, against the snapshot's current semester.
    let plan = Studyplan::expect();
    let meta = use_ask(|| Some(MetaAsk {}));
    let current = Memo::new(move |_| meta.get().and_then(Result::ok).and_then(|meta| meta.current_semester).and_then(|key| SemesterKey::parse(&key)));
    let statuses = Memo::new(move |_| {
        let now = current.get();
        ids.with(|ids| plan.map(|plan| plan.with(|doc| ids.iter().filter_map(|id| Status::of(doc, id, now).map(|status| (id.clone(), status))).collect::<BTreeMap<_, _>>())).unwrap_or_default())
    });
    let plan_has_modules = Memo::new(move |_| plan.is_some_and(|plan| !plan.is_empty()));

    // „Passt in meinen Stundenplan": the modules that fit the plan this browser keeps, worked out
    // here as the catalog does (A.7); the rows say how they fit where they fit only in part.
    let fit_source = source.clone().ok();
    let fits = Memo::new(move |_| url.with(|url| url.filter.fits.clone()));
    let fitted = Memo::new(move |_| fits.get().map(|filter| fit_of(&filter, plan, mine, fit_source.as_ref(), t)).unwrap_or_default());
    let fit_view = Memo::new(move |_| fitted.with(|fitted| fitted.view.clone()));
    // „Einplanen" of a row aims at the semester the list is checked against.
    let hint = Memo::new(move |_| fits.with(|fits| fits.as_ref().and_then(|fits| SemesterKey::parse(&fits.semester))).map(|semester| PlanHint { semester: Some(semester), fill: None }));
    provide_context(Finder { view: fit_view, hint });

    // Several at once (`pick`): the rows selected, and what „Einplanen" of them aims with — the
    // semester of each marked module's newest events, asked once for the whole list.
    let newest_source = source.clone().ok();
    let newest = Memo::new(move |before: Option<&BTreeMap<String, SemesterKey>>| {
        let ids = ids.get();
        match newest_source.as_ref().map(|source| source.now(&NewestSemestersAsk { ids })) {
            Some(Ok(found)) => found.into_iter().collect(),
            Some(Err(error)) if error.is_pending() => before.cloned().unwrap_or_default(),
            _ => BTreeMap::new(),
        }
    });
    let picking = Picking::new(current, newest, hint);
    pick::page_keys(picking);

    // „Nach Studium" needs Mein Studiengang; without it „Modul" orders by the title, as the
    // catalog's „Modul" does without a program (folia/design/merken/oberflaeche.html, C).
    let order = Memo::new(move |_| {
        let (sort, descending) = url.with(|url| (url.sort, url.descending));
        let sort = if sort == BookmarkSort::Study && program.with(Option::is_none) { BookmarkSort::Title } else { sort };
        (sort, descending)
    });
    // What the data worker is asked: the address's filters, the text, and what fits the plan.
    let query = Memo::new(move |_| asked(&url.with(|url| url.filter.clone()), &text.get(), &fitted.get()));
    let list_source = source.clone();
    let data: Memo<Loaded> = Memo::new(move |before| {
        if let Some(error) = fitted.with(|fitted| fitted.failed.clone()) {
            return DataError::or_before(Err(error), before);
        }
        let ids = ids.get();
        let query = query.get();
        if ids.is_empty() {
            // Nothing marked: nothing to ask the catalog.
            return Ok(Answered { text: query.text, data: BookmarksData::default() });
        }
        let (sort, descending) = order.get();
        let text = query.text.clone();
        let now = list_source.clone().and_then(|source| source.now(&BookmarksAsk { ids, sort, descending, query, also: also.get() })).map(|data| Answered { text, data });
        DataError::or_before(now, before)
    });

    // The list as it is shown: its sections, what the snapshot does not know. „Mein Studium" changes
    // it only where „Stand" filters it; elsewhere its rows follow it on their own.
    let listing = Memo::new(move |_| {
        let Answered { text, data } = data.get()?;
        let url = url.get();
        let statuses = if url.stand.is_some() { statuses.get() } else { statuses.get_untracked() };
        // A note or a collection changes the rows that show it; the list only in a collection's view.
        let shelf = if url.collection == CollectionView::All { shelf.get_untracked() } else { shelf.get() };
        let (places, marked_at) = (places.get(), marked_at.get());
        let known = Known { statuses: &statuses, places: places.as_ref(), marked_at: &marked_at, marked: None, shelf: Some(&shelf) };
        Ok::<_, DataError>(listing_of(&data, &url, order.get().0, &text, &known, local_day(now_secs()), t))
    });
    // How many of the rows each choice of the sidebar holds.
    let numbers = Memo::new(move |_| {
        let (data, url) = (data.get().map(|answered| answered.data).unwrap_or_default(), url.get());
        let (statuses, places, marked_at, marked, shelf) = (statuses.get(), places.get(), marked_at.get(), marked_now.get(), shelf.get());
        numbers_of(&data, &url, &Known { statuses: &statuses, places: places.as_ref(), marked_at: &marked_at, marked: Some(&marked), shelf: Some(&shelf) }, ids.with(Vec::len))
    });

    // What fills the page: the list, or the module opened from it after „Vollbild" (`local`; until
    // then it stands beside the list, on a phone a sheet over it). What is listed stays meanwhile:
    // a mark taken away on the module's page leaves the module on the list, dimmed, as it does in
    // the preview.
    let phone = phone_layout();
    let filling = Memo::new(move |_| url.with(folia_routes::local::filling));
    let preview_source = source.clone();
    let preview = Memo::new(move |before| {
        let now = match open.get() {
            None => Ok(None),
            Some(id) => preview_source.clone().and_then(|source| source.now(&ModuleAsk { id })).map(Some),
        };
        DataError::or_before(now, before)
    });
    // Where the app is going (`pending`): the sidebar shows the filter and the order at once, the
    // row of a module being opened is marked, a preview being closed is gone.
    let going = Pending::expect();
    let going_to = Memo::new(move |_| going.and_then(|going| going.search_on(url::BOOKMARKS)).map(|search| BookmarksUrl::parse(&search)));
    let shown_url = Memo::new(move |_| going_to.get().unwrap_or_else(|| url.get()));
    let going_open = Memo::new(move |_| going.filter(|going| going.change() == Some(Change::Preview)).and_then(|_| going_to.with(|to| to.as_ref().map(|to| to.open.clone()))));
    let marked = Memo::new(move |_| going_open.get().unwrap_or_else(|| open.get()));

    // On a phone the sidebar is a sheet over the list, and what is picked there is a draft, as in
    // the catalog's: the sheet shows it at once, with the number of modules it holds, and the list
    // follows when „N Module anzeigen" closes it; any other way it goes throws the draft away
    // (owner, 2026-10-18: „Die Filter sollen bei der Mobile Version verworfen werden, wenn man das
    // Fenster schließt"). `None`: the sheet shows the address's filter.
    let draft: RwSignal<Option<BookmarksUrl>> = RwSignal::new(None);
    let panel = Memo::new(move |_| draft.get().unwrap_or_else(|| shown_url.get()));
    Effect::new(move |_| {
        url.track();
        if draft.with_untracked(Option::is_some) {
            draft.set(None);
        }
    });
    let in_front = folia_design::stage::front();
    let apply = move || {
        if !in_front.get_untracked() {
            return;
        }
        let Some(next) = draft.get_untracked() else { return };
        let here = url.get_untracked();
        let next = BookmarksUrl { open: here.open.clone(), full: here.full, ..next };
        if next == here {
            draft.set(None);
            return;
        }
        // A moment later: the sheet has begun to slide away by then, and the browser keeps that
        // going while the list is built.
        let path = next.path();
        set_timeout(
            move || {
                if let Some(going) = going {
                    going.go(&path, NavigateOptions { scroll: false, ..Default::default() });
                }
            },
            std::time::Duration::from_millis(50),
        );
    };
    Effect::new(move |_| {
        let applied = window_event_listener_untyped("betula:sheet-applied", move |_| apply());
        let closed = window_event_listener_untyped("betula:sheet-closed", move |_| draft.set(None));
        let left = window_event_listener_untyped("betula:sheet-left", move |_| draft.set(None));
        on_cleanup(move || {
            applied.remove();
            closed.remove();
            left.remove();
        });
    });
    // A window that grows out of the phone's layout (a tablet turned) has no sheet any more.
    Effect::new(move |_| {
        if !phone.get() {
            apply();
        }
    });
    // A filter that is no link (the credits' fields): into the draft on a phone, else to the list.
    let pick = Callback::new(move |next: BookmarksUrl| {
        if phone.get_untracked() {
            draft.set(Some(next));
        } else if let Some(going) = going {
            going.go_beside(&next.path());
        }
    });
    // A link of the sheet is the list it leads to, so on a phone its address is what the draft
    // becomes; the router must not follow it then (it ignores a click whose default is prevented).
    let into_draft = move |ev: leptos::ev::MouseEvent| {
        if !phone.get_untracked() || ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key() {
            return;
        }
        let Some(href) = nav::link_under(ev.target()) else { return };
        // The link as written carries the language's prefix (`/en/bookmarks?…`).
        let (language, path) = Locale::split(&href);
        if language != t.locale {
            return;
        }
        let Some(search) = path.strip_prefix(url::BOOKMARKS).filter(|rest| rest.is_empty() || rest.starts_with('?')) else { return };
        ev.prevent_default();
        draft.set(Some(BookmarksUrl::parse(search.trim_start_matches('?'))));
    };
    // The number the sheet's button says: what the draft would list.
    let draft_source = source.clone().ok();
    let draft_shown = Memo::new(move |_| {
        let draft = draft.get()?;
        let Answered { text, data } = if url.with(|url| url.filter == draft.filter) {
            data.get().ok()?
        } else {
            let fitted = draft.filter.fits.as_ref().map(|filter| fit_of(filter, plan, mine, draft_source.as_ref(), t)).unwrap_or_default();
            if fitted.failed.is_some() {
                return None;
            }
            let (sort, descending, text) = (order.get().0, order.get().1, text.get());
            let data = draft_source.as_ref()?.now(&BookmarksAsk { ids: ids.get(), sort, descending, query: asked(&draft.filter, &text, &fitted), also: also.get() }).ok()?;
            Answered { text, data }
        };
        let (statuses, places, marked_at, marked, shelf) = (statuses.get(), places.get(), marked_at.get(), marked_now.get(), shelf.get());
        Some(shown_of(&data, &draft, !text.is_empty(), &Known { statuses: &statuses, places: places.as_ref(), marked_at: &marked_at, marked: Some(&marked), shelf: Some(&shelf) }))
    });
    let shown_now = Memo::new(move |_| {
        let (Answered { text, data }, url) = (data.get().unwrap_or_default(), url.get());
        let (statuses, places, marked_at, marked, shelf) = (statuses.get(), places.get(), marked_at.get(), marked_now.get(), shelf.get());
        shown_of(&data, &url, !text.is_empty(), &Known { statuses: &statuses, places: places.as_ref(), marked_at: &marked_at, marked: Some(&marked), shelf: Some(&shelf) })
    });
    let sheet_count = Memo::new(move |_| draft_shown.get().unwrap_or_else(|| shown_now.get()));

    // The views of the list: all of it, „Ohne Sammlung", each collection (`folia_stores::
    // collections`), with how many marked modules each holds; what their parts have open.
    let views = Memo::new(move |_| shelf.with(|shelf| marked_now.with(|marked| views_of(shelf, marked))));
    let go = Callback::new(move |view: CollectionView| {
        let next = BookmarksUrl { collection: view, ..url.get_untracked() };
        if let Some(going) = going {
            going.go(&next.path(), NavigateOptions { scroll: false, ..Default::default() });
        }
    });
    let ui = CollUi { menu: RwSignal::new(None), naming: RwSignal::new(None), typed: RwSignal::new(String::new()), confirming: RwSignal::new(false), sharing: RwSignal::new(false), go };

    // Coming back to the list from a module's page — the module that filled the page here, or
    // its own page (a link on it) — the list shows the row the visitor left it at.
    let now = tabs::location_of(&location.pathname.get_untracked(), &location.search.get_untracked());
    let left_at = StoredValue::new(Tabs::expect().and_then(|tabs| tabs::page_below(&tabs.before(&now), "/catalog/module")));
    let sidebar_data = Memo::new(move |_| data.get().ok().map(|answered| answered.data));
    let any_marked = Memo::new(move |_| ids.with(|ids| !ids.is_empty()));

    (move || {
        if let Some(id) = filling.get() {
            left_at.set_value(Some(id.clone()));
            let back = url.with_untracked(folia_routes::local::back_href);
            return view! { <ModuleInPlace id area=Area::Bookmarks back/> }.into_any();
        }
        if let Some(id) = left_at.try_update_value(Option::take).flatten() {
            reveal_row_soon(id);
        }
        view! {
            <Title text=t.bookmarks.title/>
            // One scroll area with the ground at its end, as the catalog (app.css „one scroll area").
            // The page's regions for the keyboard as the catalog's (enhance.js, „the keyboard";
            // folia/design/tastatur/konzept.html, „Die anderen Seiten"): the content Tab goes round
            // in, the sidebar a mode F leads into.
            <div class="work framed flowing" id=SCROLL_ID data-keep-scroll="rows" data-region="content" data-cycle="">
                <BookmarksSeo/>
                // `data-draft`: on a phone what is picked in the sheet is a draft, which only „N Module
                // anzeigen" applies (`enhance.js`).
                // The sidebar, the collections and the filters, is the page's side region and a mode
                // of the keyboard, as the catalog's filters: F leads in, Tab goes from section to
                // section, the arrows from collection to collection and filter to filter, Esc leads
                // out to the list. In „Befehle" (folia_shell::commands) it is a command of „Gehe zu"
                // with its key, and what it holds is found by typing.
                <aside
                    class="panel sidebar sheet"
                    id="sidebar"
                    data-sheet=""
                    aria-label=t.bookmarks.title
                    aria-describedby="sidebar-leave"
                    aria-keyshortcuts="F"
                    data-region="side"
                    data-mode=""
                    data-draft=""
                    data-command=t.bookmarks.sidebar
                    data-command-icon="sliders-horizontal"
                    data-command-group="go"
                    data-command-keys="F"
                    data-command-act="go side"
                    data-commands=t.bookmarks.title
                    data-commands-typed=""
                    on:click=into_draft
                >
                    <span class="visually-hidden" id="sidebar-leave">{t.bookmarks.sidebar_leave}</span>
                    <div class="panel-head" data-section="plain">
                        // On a phone the sidebar is the sheet behind „Filter", and named so. The key
                        // that leads in, at the head (R10): F.
                        <div class="panel-title"><h2><span class="side-title">{t.bookmarks.title}</span><span class="sheet-title">{t.catalog.filters}</span></h2><span class="js-only" aria-hidden="true"><Keys spec="F"/></span></div>
                        {move || (panel.with(BookmarksUrl::filters_set) > 0).then(|| view! {
                            <a class="ghost hit" style=Hit::y(7.0).style() href=move || t.path(&BookmarksUrl { collection: panel.with(|p| p.collection), sort: panel.with(|p| p.sort), descending: panel.with(|p| p.descending), open: open.get(), ..Default::default() }.path()) data-noscroll="" data-command=t.catalog.reset_all><Icon name="rotate-ccw"/>{t.common.reset}</a>
                        })}
                        <a class="icon-btn sheet-close" href="#" data-action="sheet-close" aria-label=t.catalog.close_filters><Icon name="x"/></a>
                    </div>
                    <div class="body scroll" data-keep-scroll="sidebar">
                        <CollectionsNav ui views here=shown_url/>
                        <Sidebar panel numbers program plan_has_modules current pick any_marked sheet_count/>
                    </div>
                </aside>
                <div class="resizer between js-only" data-action="resize-filters" role="separator" aria-orientation="vertical" aria-controls="sidebar" aria-label=t.ui.resize_sidebar tabindex="0"></div>
                <section class="panel list" aria-live="polite">
                    <Offer/>
                    <CollectionChips ui views here=shown_url/>
                    {move || {
                        // The parts of the URL first, then what is derived from them (R16).
                        let here = shown_url.get();
                        let (sort, _) = order.get();
                        match listing.get() {
                            Err(error) => view! { <ErrorState error/> }.into_any(),
                            Ok(listing) => view! {
                                <List listing here sort program_set=program.with_untracked(Option::is_some) statuses open=marked phone shelf ui picking/>
                            }
                            .into_any(),
                        }
                    }}
                    <i class="list-cap" aria-hidden="true"></i>
                </section>
                <ViewMenu ui here=shown_url data=sidebar_data picking phone/>
                <PickMenus picking/>
                {move || {
                    match going_open.get() {
                        Some(None) => return ().into_any(),
                        Some(Some(_)) if going.is_some_and(|going| going.waits(Change::Preview)) => return view! { <DetailSkeleton calm=open.get_untracked().is_some() module=true/> }.into_any(),
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
                            let full_href = folia_routes::local::full_href(&here, &data.module.id);
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
                <folia_shell::ground::Ground/>
            </div>
        }
        .into_any()
    })
    .into_any()
}

/// The query the data worker answers for a filter of the list: the address's filters, what is
/// typed, and the modules that fit the plan where „Passt in meinen Stundenplan" is on (where they
/// could not be worked out, none: a list that does not know the plan lists nothing that fits).
fn asked(filter: &CatalogQuery, text: &str, fitted: &Fitted) -> CatalogQuery {
    CatalogQuery { text: text.to_string(), fits_ids: filter.fits.as_ref().and(fitted.ids.clone()), ..filter.clone() }
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

// ---- „Passt in meinen Stundenplan" ----

/// What „Passt in meinen Stundenplan" makes of the list (A.7): the modules that fit, or those to
/// leave out, and what the rows say of them.
#[derive(Clone, Debug, Default, PartialEq)]
struct Fitted {
    ids: Option<FitIds>,
    view: FitView,
    /// The local catalog could not answer: the list says so instead of listing nothing.
    failed: Option<DataError>,
}

/// The finder for `filter` against the plan this browser keeps: the semester's planned modules and
/// what the plan hides and has chosen there, as the catalog asks it. Without a plan nothing.
fn fit_of(filter: &FitsFilter, plan: Option<Studyplan>, mine: Option<MyProgram>, source: Option<&DataClient>, t: &'static Texts) -> Fitted {
    let (Some(plan), Some(source)) = (plan, source) else { return Fitted::default() };
    let key = SemesterKey::parse(&filter.semester);
    let planned = key.map(|key| plan.modules_in(key)).unwrap_or_default();
    let town = mine.map(MyProgram::town).unwrap_or_default();
    let selection = key.map(|key| plan.selection(key, town)).unwrap_or_default();
    match source.now(&FitAsk { filter: filter.clone(), plan: planned, selection, locale: t.locale }) {
        Ok(result) => {
            let label = key.map_or_else(|| filter.semester.clone(), |key| key.label(t.locale));
            Fitted {
                // A semester without dates is not checked: everything but the planned modules.
                ids: Some(if !result.has_data || filter.undated { FitIds::Without(result.excluded.clone()) } else { FitIds::Only(result.fitting.clone()) }),
                view: FitView {
                    notes: result.notes.clone(),
                    quiet: result.unknown.clone(),
                    fitting: result.fitting.iter().cloned().collect(),
                    undated_note: (result.has_data && filter.undated).then(|| (t.catalog.fit_undated)(&label)),
                    line: (!result.has_data).then(|| (t.catalog.fit_unpublished)(&label)),
                },
                failed: None,
            }
        }
        Err(error) => Fitted { failed: Some(error), ..Fitted::default() },
    }
}

// ---- what „Mein Studium" says ----

/// Where „Mein Studium" says a marked module stands, as its semesters draw it (owner, 2026-10-19:
/// „Ja mein stadium"): planned for a semester to come, under way in the current one, passed, or
/// planned in one that is over and not passed; with the semester.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Planned(SemesterKey),
    Running(SemesterKey),
    Passed(SemesterKey),
    Failed(SemesterKey),
}

impl Status {
    /// What the plan says of `id`, `now` being the current semester. Planned in several semesters,
    /// the first that is not over counts.
    fn of(doc: &PlanDoc, id: &str, now: Option<SemesterKey>) -> Option<Self> {
        if let Some(passed) = doc.passed_in(id) {
            return Some(Status::Passed(passed));
        }
        let planned = doc.planned_in(id);
        let Some(now) = now else { return planned.last().copied().map(Status::Planned) };
        match planned.iter().copied().find(|semester| *semester >= now) {
            Some(semester) if semester == now => Some(Status::Running(semester)),
            Some(semester) => Some(Status::Planned(semester)),
            None => planned.last().copied().map(Status::Failed),
        }
    }

    /// „Stand": a module planned in a semester that is over and not passed is open again.
    fn stand(self) -> Stand {
        match self {
            Status::Planned(_) | Status::Running(_) => Stand::Planned,
            Status::Passed(_) => Stand::Passed,
            Status::Failed(_) => Stand::Open,
        }
    }

    /// What its row shows: the mark of „Mein Studium" and the semester.
    fn row(self, t: &'static Texts) -> RowStatus {
        let b = &t.bookmarks;
        let (class, word, semester) = match self {
            Status::Planned(semester) => ("planned", b.status_planned, semester),
            Status::Running(semester) => ("running", b.status_running, semester),
            Status::Passed(semester) => ("passed", b.status_passed, semester),
            Status::Failed(semester) => ("failed", b.status_failed, semester),
        };
        RowStatus { class, word, semester: semester.short(t.locale) }
    }
}

/// The „Stand" of a module „Mein Studium" says nothing of: open.
fn stand_of(status: Option<&Status>) -> Stand {
    status.map_or(Stand::Open, |status| status.stand())
}

// ---- the list in its order ----

/// The visitor's calendar day of a moment (seconds since 1970), in the browser's time zone.
fn local_day(at: u64) -> Day {
    let local = i64::try_from(at).unwrap_or(i64::MAX).saturating_sub(utc_offset_minutes(at).saturating_mul(60));
    Day(i32::try_from(local.div_euclid(86_400)).unwrap_or(i32::MAX))
}

/// How far the browser's time zone is behind UTC at a moment, in minutes (`getTimezoneOffset`).
#[cfg(feature = "csr")]
fn utc_offset_minutes(at: u64) -> i64 {
    web_sys::js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(at as f64 * 1000.0)).get_timezone_offset() as i64
}

#[cfg(not(feature = "csr"))]
fn utc_offset_minutes(_at: u64) -> i64 {
    0
}

/// Now, in seconds since 1970.
#[cfg(feature = "csr")]
fn now_secs() -> u64 {
    (web_sys::js_sys::Date::now() / 1000.0).max(0.0) as u64
}

#[cfg(not(feature = "csr"))]
fn now_secs() -> u64 {
    0
}

/// When a mark was set, as the sections of „Zuletzt gemerkt" name it, the newest first: today,
/// yesterday, the rest of this week, last week, then the months; a mark without a time last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum When {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    /// Year and month, the newest first.
    Month(Reverse<(i32, u32)>),
    Unknown,
}

impl When {
    fn of(day: Option<Day>, today: Day) -> Self {
        let Some(day) = day else { return When::Unknown };
        let monday = today.monday();
        if day >= today {
            When::Today
        } else if day == today.plus(-1) {
            When::Yesterday
        } else if day >= monday {
            When::ThisWeek
        } else if day >= monday.plus(-7) {
            When::LastWeek
        } else {
            let (year, month, _) = day.ymd();
            When::Month(Reverse((year, month)))
        }
    }

    fn title(self, today: Day, t: &'static Texts) -> String {
        let b = &t.bookmarks;
        match self {
            When::Today => b.today.to_string(),
            When::Yesterday => b.yesterday.to_string(),
            When::ThisWeek => b.this_week.to_string(),
            When::LastWeek => b.last_week.to_string(),
            When::Month(Reverse((year, month))) => {
                let name = usize::try_from(month).ok().and_then(|month| month.checked_sub(1)).and_then(|index| b.months.get(index)).copied().unwrap_or_default();
                if year == today.ymd().0 { name.to_string() } else { (b.month_year)(name, year) }
            }
            When::Unknown => b.earlier.to_string(),
        }
    }
}

/// The section of „Nach Studium" a module falls into, in the study's order: compulsory, the
/// electives by their area in the order of the module tree, the thesis, the internship, the
/// program's modules without a kind, FÜS, and what lies outside the program
/// (folia/design/merken/konzept.html, „Ordnungen, die Betula kennt").
fn study_section(row: &CatalogRow, places: &BTreeMap<String, Place>, t: &'static Texts) -> ((u8, i64, String), String) {
    let b = &t.bookmarks;
    let kind_label = |kind: ModuleKind| kind.label(t.locale);
    match places.get(&row.id) {
        Some(place) => match place.kind.as_ref().and_then(Code::known) {
            Some(ModuleKind::Compulsory) => ((0, 0, String::new()), kind_label(ModuleKind::Compulsory).to_string()),
            Some(ModuleKind::Elective) if place.area.is_empty() => ((1, i64::MAX, String::new()), kind_label(ModuleKind::Elective).to_string()),
            Some(ModuleKind::Elective) => ((1, place.order, place.area.clone()), (b.kind_in)(kind_label(ModuleKind::Elective), &place.area)),
            Some(ModuleKind::Thesis) => ((2, 0, String::new()), kind_label(ModuleKind::Thesis).to_string()),
            Some(ModuleKind::Internship) => ((3, 0, String::new()), kind_label(ModuleKind::Internship).to_string()),
            Some(ModuleKind::Fues) => ((5, 0, String::new()), kind_label(ModuleKind::Fues).to_string()),
            None => ((4, 0, String::new()), b.in_program.to_string()),
        },
        // The BTU's general list of FÜS modules, which every program's FÜS takes from.
        None if row.is_fues => ((5, 0, String::new()), kind_label(ModuleKind::Fues).to_string()),
        None => ((6, 0, String::new()), b.not_in_program.to_string()),
    }
}

/// The section of „Nach Angebot": winter, summer, every semester, irregularly, not stated.
fn offer_section(row: &CatalogRow, t: &'static Texts) -> (u8, String) {
    let b = &t.bookmarks;
    match row.turnus_season.as_ref().and_then(Code::known) {
        Some(TurnusSeason::Winter) => (0, b.offer_winter.to_string()),
        Some(TurnusSeason::Summer) => (1, b.offer_summer.to_string()),
        Some(TurnusSeason::Both) => (2, b.offer_every.to_string()),
        Some(TurnusSeason::Irregular) => (3, b.offer_irregular.to_string()),
        None => (4, b.offer_unstated.to_string()),
    }
}

/// A part of the list under a heading of its own („Heute", „Wahlpflicht · Praktische Mathematik",
/// „Winter"); a list without sections is one part without one.
#[derive(Clone, Debug, PartialEq)]
struct Part {
    title: Option<String>,
    rows: Vec<CatalogRow>,
}

/// `rows` in sections by `section` (its order and its heading), each keeping the order of the rows
/// in it; the sections the other way round where `reverse`.
fn sectioned<K: Ord>(rows: Vec<CatalogRow>, reverse: bool, section: impl Fn(&CatalogRow) -> (K, String)) -> Vec<Part> {
    let mut sections: BTreeMap<K, Part> = BTreeMap::new();
    for row in rows {
        let (key, title) = section(&row);
        sections.entry(key).or_insert_with(|| Part { title: Some(title), rows: Vec::new() }).rows.push(row);
    }
    let mut parts: Vec<Part> = sections.into_values().collect();
    if reverse {
        parts.reverse();
    }
    parts
}

/// What the page knows besides the rows, to filter them and to order them into sections.
struct Known<'a> {
    statuses: &'a BTreeMap<String, Status>,
    /// Where the modules count in Mein Studiengang; `None` without one, or before it is known.
    places: Option<&'a BTreeMap<String, Place>>,
    /// When each was marked (seconds since 1970; 0 if unknown).
    marked_at: &'a BTreeMap<String, u64>,
    /// What is marked now, where the numbers count only that: a module unmarked on this page is
    /// still listed (dimmed), but counted no more.
    marked: Option<&'a BTreeSet<String>>,
    /// The visitor's collections and notes, for a view of a collection (`BookmarksUrl::collection`).
    shelf: Option<&'a Shelf>,
}

impl Known<'_> {
    fn counts(&self, id: &str) -> bool {
        self.marked.is_none_or(|marked| marked.contains(id))
    }
}

/// The filters the page applies itself (the data worker applies the others).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Facet {
    Season,
    Stand,
    Counts,
}

/// Whether `row` passes the filters the page applies itself: the half of the year, „Stand", „Zählt
/// in meinem Studium"; all but `except`, for the numbers of that filter's choices.
fn keeps(data: &BookmarksData, row: &CatalogRow, url: &BookmarksUrl, known: &Known, except: Option<Facet>) -> bool {
    if !in_view(&row.id, url, known) {
        return false;
    }
    let season = except == Some(Facet::Season) || url.season.is_none_or(|season| data.offered_in(season, &row.id));
    let stand = except == Some(Facet::Stand) || url.stand.is_none_or(|stand| stand_of(known.statuses.get(&row.id)) == stand);
    let counts = except == Some(Facet::Counts) || !url.counts || counts_in_study(row, known.places);
    season && stand && counts
}

/// Whether a module is in the part of the list the view shows: all of it, what is in no
/// collection, or a collection (a collection that is not there shows nothing).
fn in_view(id: &str, url: &BookmarksUrl, known: &Known) -> bool {
    match url.collection {
        CollectionView::All => true,
        CollectionView::Without => known.shelf.is_none_or(|shelf| shelf.of(id).is_empty()),
        CollectionView::Of(collection) => known.shelf.is_some_and(|shelf| shelf.holds(collection, id)),
    }
}

/// Whether a module counts in Mein Studiengang: it has a place in the program, or it is on the
/// BTU's general list of FÜS modules. Without the program known every module passes: the page
/// cannot tell, and does not pretend to.
fn counts_in_study(row: &CatalogRow, places: Option<&BTreeMap<String, Place>>) -> bool {
    places.is_none_or(|places| row.is_fues || places.contains_key(&row.id))
}

/// Whether the list shows everything marked: no filter, no search. Only then are the modules the
/// snapshot does not know listed, as nothing can be said of them.
fn unfiltered(url: &BookmarksUrl, searching: bool) -> bool {
    url.filters_set() == 0 && !searching
}

/// The list as the page shows it.
#[derive(Clone, Debug, Default, PartialEq)]
struct Listing {
    parts: Vec<Part>,
    /// Marked modules the snapshot does not know, listed while nothing filters the list.
    missing: Vec<String>,
    /// What the search searched (empty: nothing), and whether the filters filter.
    text: String,
    filtering: bool,
}

impl Listing {
    fn count(&self) -> usize {
        self.parts.iter().map(|part| part.rows.len()).sum::<usize>() + self.missing.len()
    }
}

/// The rows of `data` the page's own filters keep, in sections where the order has them: the time
/// of marking, the study, the offer. A search keeps the order without sections: what it found is
/// a few rows, and the headings would only stand between them.
fn listing_of(data: &BookmarksData, url: &BookmarksUrl, sort: BookmarkSort, text: &str, known: &Known, today: Day, t: &'static Texts) -> Listing {
    let searching = !text.is_empty();
    let rows: Vec<CatalogRow> = data.rows.iter().filter(|row| keeps(data, row, url, known, None)).cloned().collect();
    let parts = match (sort, known.places) {
        _ if rows.is_empty() => Vec::new(),
        _ if searching => vec![Part { title: None, rows }],
        (BookmarkSort::Added, _) => sectioned(rows, url.descending, |row| {
            let when = When::of(known.marked_at.get(&row.id).copied().filter(|at| *at > 0).map(local_day), today);
            (when, when.title(today, t))
        }),
        (BookmarkSort::Study, Some(places)) => sectioned(rows, url.descending, |row| study_section(row, places, t)),
        (BookmarkSort::Offer, _) => sectioned(rows, url.descending, |row| offer_section(row, t)),
        _ => vec![Part { title: None, rows }],
    };
    let missing = if unfiltered(url, searching) { data.missing.iter().filter(|id| in_view(id, url, known)).cloned().collect() } else { Vec::new() };
    Listing { parts, missing, text: text.to_string(), filtering: url.filters_set() > 0 }
}

/// How many marked modules `url` lists of `data`: the number the button of the phone's sheet says.
fn shown_of(data: &BookmarksData, url: &BookmarksUrl, searching: bool, known: &Known) -> usize {
    let missing = if unfiltered(url, searching) { data.missing.iter().filter(|id| known.counts(id) && in_view(id, url, known)).count() } else { 0 };
    data.rows.iter().filter(|row| known.counts(&row.id) && keeps(data, row, url, known, None)).count() + missing
}

/// How many of the rows each choice of the sidebar holds, under the other filters (concept: „Jeder
/// Filter mit seinen Zahlen").
#[derive(Clone, Debug, Default, PartialEq)]
struct Numbers {
    /// „Angeboten im": every row the other filters keep, and those offered in winter, in summer.
    seasons_all: usize,
    winter: usize,
    summer: usize,
    /// „Stand": open, planned, passed.
    open: usize,
    planned: usize,
    passed: usize,
    /// Whether the marked modules are not all offered alike: „Angeboten im" is there only then.
    seasons_apart: bool,
    /// Whether „Mein Studium" says something of any marked module: „Stand" is there only then.
    any_status: bool,
    /// „Zählt in meinem Studium".
    counting: usize,
}

/// `listed`: how many modules the page lists in all, the ones the snapshot does not know with them.
fn numbers_of(data: &BookmarksData, url: &BookmarksUrl, known: &Known, listed: usize) -> Numbers {
    // Of all the marked modules, whatever filters the list: the filters stay where they are while
    // the list is searched or filtered.
    let all = listed.saturating_sub(data.missing.len());
    let apart = |offered: usize| offered > 0 && offered < all;
    let mut numbers = Numbers { seasons_apart: apart(data.winter.len()) || apart(data.summer.len()), any_status: !known.statuses.is_empty(), ..Numbers::default() };
    for row in data.rows.iter().filter(|row| known.counts(&row.id)) {
        if keeps(data, row, url, known, Some(Facet::Season)) {
            numbers.seasons_all += 1;
            numbers.winter += usize::from(data.offered_in(Season::Winter, &row.id));
            numbers.summer += usize::from(data.offered_in(Season::Summer, &row.id));
        }
        if keeps(data, row, url, known, Some(Facet::Stand)) {
            match stand_of(known.statuses.get(&row.id)) {
                Stand::Open => numbers.open += 1,
                Stand::Planned => numbers.planned += 1,
                Stand::Passed => numbers.passed += 1,
            }
        }
        if keeps(data, row, url, known, Some(Facet::Counts)) && counts_in_study(row, known.places) {
            numbers.counting += 1;
        }
    }
    numbers
}

/// The tags above the list for what filters it, each with the list without it.
fn tags(url: &BookmarksUrl, program_set: bool, t: &'static Texts) -> Vec<(&'static str, String, BookmarksUrl)> {
    let mut tags = Vec::new();
    let f = &url.filter;
    if let Some(season) = url.season {
        tags.push((t.catalog.turnus, season.label(t.locale).to_string(), BookmarksUrl { season: None, ..url.clone() }));
    }
    if let Some(stand) = url.stand {
        tags.push((t.bookmarks.stand, stand.label(t.locale).to_string(), BookmarksUrl { stand: None, ..url.clone() }));
    }
    if url.counts && program_set {
        tags.push(("", t.bookmarks.counts.to_string(), BookmarksUrl { counts: false, ..url.clone() }));
    }
    if let Some(fits) = &f.fits {
        let semester = SemesterKey::parse(&fits.semester).map_or_else(|| fits.semester.clone(), |key| key.short(t.locale));
        let mut without = url.clone();
        without.filter.fits = None;
        tags.push((t.catalog.tag_fits, semester, without));
    }
    for language in &f.languages {
        let mut without = url.clone();
        without.filter.languages.retain(|other| other != language);
        tags.push((t.catalog.language, language.label(t.locale).to_string(), without));
    }
    if f.credits_min.is_some() || f.credits_max.is_some() {
        let number = |value: f64| format::number(value, t.locale);
        let range = match (f.credits_min, f.credits_max) {
            (Some(low), Some(high)) if low == high => number(low),
            (Some(low), Some(high)) => format!("{}–{}", number(low), number(high)),
            (Some(low), None) => (t.catalog.credits_from)(&number(low)),
            (None, Some(high)) => (t.catalog.credits_up_to)(&number(high)),
            (None, None) => String::new(),
        };
        let mut without = url.clone();
        (without.filter.credits_min, without.filter.credits_max) = (None, None);
        tags.push((t.common.credits_unit, range, without));
    }
    for campus in &f.campuses {
        let mut without = url.clone();
        without.filter.campuses.retain(|other| other != campus);
        tags.push((t.catalog.location, campus.label(t.locale).to_string(), without));
    }
    if let Some(graded) = f.graded {
        let mut without = url.clone();
        without.filter.graded = None;
        tags.push((t.catalog.tag_grading, if graded { t.catalog.graded } else { t.catalog.ungraded }.to_string(), without));
    }
    tags
}

// ---- a list brought over from another device ----

/// How a link's modules are kept.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Keep {
    /// Marked, with the notes that came along: „Nur merken", „Hinzufügen".
    Marked,
    /// Marked, and in every collection the link brings (the link for another device).
    Everything,
    /// Marked, and in the collection of this name („Als Sammlung merken", „Zu ‚Für Lena‘
    /// hinzufügen", „Anderer Name …").
    Named(String),
}

/// A list brought by a link: it arrives in the fragment of the address (`/bookmarks#m=…`, see
/// `bookmarks::share_fragment`), which no browser sends to a server. The page asks before it adds
/// anything (a link must not fill somebody's list behind their back), and either answer takes the
/// list out of the address and out of the history entry. A list whose check characters do not fit
/// (a link cut short, a character typed wrong) is named, and nothing of it is offered.
///
/// What it offers follows what the link brings (folia/design/merken/oberflaeche.html, „Weitergeben
/// und Ankommen"): modules alone „Hinzufügen"; a collection „Als Sammlung merken" or „Nur merken",
/// and where a collection of its name is there „Zu ‚Für Lena‘ hinzufügen" or „Anderer Name …",
/// which asks for one that is not; the link for another device every collection at once. Notes
/// come along where a module has none. More than there is room for: the first that fit.
#[component]
fn Offer() -> impl IntoView {
    let t = i18n::t();
    let b = &t.bookmarks;
    let bookmarks = Bookmarks::expect();
    let collections = Collections::expect();
    let location = use_location();
    // The fragment as the browser has it. The router hears of it when a page is opened, not when
    // only the fragment changes (a link pasted into the address of the open list), so the page
    // listens for that itself. Effects run in the browser only: the server offers nothing.
    let fragment = RwSignal::new(String::new());
    Effect::new(move |_| {
        location.hash.track();
        fragment.set(nav::fragment());
    });
    // The fragment of the window is the page's in front (`folia_design::stage`).
    let in_front = folia_design::stage::front();
    Effect::new(move |_| {
        let handle = window_event_listener_untyped("hashchange", move |_| {
            if in_front.get_untracked() {
                fragment.set(nav::fragment());
            }
        });
        on_cleanup(move || handle.remove());
    });
    let brought = Memo::new(move |_| brought_from_fragment(&fragment.get()));
    // „Anderer Name …": the name typed, and whether it is one there is.
    let naming = RwSignal::new(None::<String>);
    let taken = RwSignal::new(false);
    let navigate = use_navigate();
    let done = Callback::new(move |keep: Option<Keep>| {
        if let (Some(keep), Some(bookmarks), Ok(brought)) = (keep, bookmarks, brought.get_untracked()) {
            bookmarks.add_all(&brought.ids);
            if let Some(collections) = collections {
                let named: Vec<(String, Vec<String>)> = match keep {
                    Keep::Marked => Vec::new(),
                    Keep::Everything => brought.collections.clone(),
                    Keep::Named(name) => vec![(name, brought.collections.first().map_or_else(|| brought.ids.clone(), |(_, ids)| ids.clone()))],
                };
                collections.update(|shelf| shelf.adopt(&named, &brought.notes, |id| bookmarks.is_marked_untracked(id)));
            }
        }
        naming.set(None);
        taken.set(false);
        // The same page without the fragment, in the same history entry.
        fragment.set(String::new());
        let here = tabs::location_of(&location.pathname.get_untracked(), &location.search.get_untracked());
        navigate(&here, NavigateOptions { replace: true, scroll: false, ..Default::default() });
    });
    let other_name = move || {
        let name = folia_stores::collections::clean_name(&naming.get_untracked().unwrap_or_default());
        if name.is_empty() {
            return;
        }
        if collections.is_some_and(|collections| collections.with_untracked(|shelf| shelf.find(&name).is_some())) {
            taken.set(true);
            return;
        }
        done.run(Some(Keep::Named(name)));
    };
    move || {
        let brought = match brought.get() {
            Ok(brought) if brought.ids.is_empty() => return None,
            Ok(brought) => brought,
            Err(BrokenLink) => {
                return Some(
                    view! {
                        <div class="offer" role="status">
                            <Icon name="info"/>
                            <p>
                                <b>{b.broken_link}" "</b>
                                {b.broken_link_hint}
                            </p>
                            <button class="mini hit" type="button" id="offer-dismiss" on:click=move |_| done.run(None)>{b.ok}</button>
                        </div>
                    }
                    .into_any(),
                )
            }
        };
        let all = brought.ids.len();
        let new = brought.ids.iter().filter(|id| !bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(id))).count();
        // A link that brings more than the Merkliste has room for offers what fits
        // (oberflaeche.html, „Bei 256 ist Schluss": „Ein Link mit mehr bietet an, was passt").
        let room = bookmarks.map_or(new, Bookmarks::room);
        let fits = new.min(room);
        let short = new > room;
        let full = new > 0 && room == 0;
        let room_line = move || if full { (b.none_fit)(MAX_MARKS) } else { (b.room_for)(room) };
        let discard = move || view! { <button class="mini hit" type="button" id="offer-dismiss" on:click=move |_| done.run(None)>{b.discard}</button> };
        let (head, line, answers) = match brought.collections.as_slice() {
            // Modules alone, as before collections.
            [] => {
                let line = match new {
                    0 => b.all_there.to_string(),
                    _ if short => room_line(),
                    n if n == all => b.add_question.to_string(),
                    n => (b.some_missing)(n),
                };
                let answers = view! {
                    {(fits > 0).then(|| view! { <button class="mini primary hit" type="button" id="offer-add" on:click=move |_| done.run(Some(Keep::Marked))>{if short { (b.first_n)(fits) } else { b.add.to_string() }}</button> })}
                    {if fits > 0 { discard().into_any() } else { view! { <button class="mini hit" type="button" id="offer-dismiss" on:click=move |_| done.run(None)>{b.ok}</button> }.into_any() }}
                }
                .into_any();
                ((b.from_link)(all), line, answers)
            }
            // A collection's link: one collection that holds all of them.
            [(name, members)] if members.len() == all => {
                // A collection of that name here is named as it is here („für lena" from the
                // link, „Für Lena" here).
                let there = collections.and_then(|collections| collections.with(|shelf| shelf.find(name).and_then(|id| shelf.collection(id).map(|collection| collection.name.clone()))));
                let exists = there.is_some();
                let name = name.clone();
                let line = if full || short {
                    room_line()
                } else if exists {
                    b.collection_exists.to_string()
                } else if new < all {
                    (b.already_marked)(all - new, all)
                } else {
                    String::new()
                };
                let answers = if naming.with(Option::is_some) {
                    view! {
                        <label class="offer-name">
                            <span class="visually-hidden">{b.other_name_label}</span>
                            <input type="text" id="offer-name" placeholder=b.other_name_label autocomplete="off" maxlength="60"
                                prop:value=move || naming.get().unwrap_or_default()
                                on:input=move |ev| { naming.set(Some(event_target_value(&ev))); taken.set(false); }
                                on:keydown=move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
                                    "Enter" => { ev.prevent_default(); other_name(); }
                                    "Escape" => { ev.prevent_default(); ev.stop_propagation(); naming.set(None); taken.set(false); }
                                    _ => {}
                                }/>
                            {move || taken.get().then(|| view! { <small class="warn">{b.name_taken}</small> })}
                        </label>
                        <button class="mini primary hit" type="button" id="offer-name-keep" on:click=move |_| other_name()>{b.keep_named}</button>
                        <button class="mini hit" type="button" on:click=move |_| { naming.set(None); taken.set(false); }>{b.cancel}</button>
                    }
                    .into_any()
                } else if let Some(there) = there {
                    let into = name.clone();
                    view! {
                        <button class="mini primary hit" type="button" id="offer-add" on:click=move |_| done.run(Some(Keep::Named(into.clone())))>{(b.add_to)(&there)}</button>
                        <button class="mini hit" type="button" id="offer-other" on:click=move |_| {
                            naming.set(Some(String::new()));
                            request_animation_frame(|| nav::focus_by_id("offer-name"));
                        }>{b.other_name}</button>
                        {discard()}
                    }
                    .into_any()
                } else {
                    let into = name.clone();
                    view! {
                        <button class="mini primary hit" type="button" id="offer-add" on:click=move |_| done.run(Some(Keep::Named(into.clone())))>{b.keep_collection}</button>
                        <button class="mini hit" type="button" id="offer-only" on:click=move |_| done.run(Some(Keep::Marked))>{b.only_mark}</button>
                        {discard()}
                    }
                    .into_any()
                };
                ((b.from_link_named)(&name, all), line, answers)
            }
            // The link for another device: every collection at once.
            many => {
                let line = if full || short { room_line() } else { (b.new_of)(new) };
                let answers = view! {
                    <button class="mini primary hit" type="button" id="offer-add" on:click=move |_| done.run(Some(Keep::Everything))>{if short && fits > 0 { (b.first_n)(fits) } else { b.add.to_string() }}</button>
                    {discard()}
                }
                .into_any();
                ((b.from_link_many)(all, many.len()), line, answers)
            }
        };
        Some(
            view! {
                <div class="offer" role="status">
                    <Icon name="bookmark"/>
                    <p>
                        <b>{head}</b>
                        {(!line.is_empty()).then(|| format!(" {line}"))}
                    </p>
                    <span class="offer-answers">{answers}</span>
                </div>
            }
            .into_any(),
        )
    }
}

// ---- the list ----

/// What is marked among `rows`, as the head counts it: how many modules and their credits. Tracks
/// the marks.
fn marked_numbers(bookmarks: Option<Bookmarks>, rows: &[(String, Option<f64>)]) -> (u64, f64) {
    let marked: Vec<&(String, Option<f64>)> = rows.iter().filter(|(id, _)| bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(id))).collect();
    (marked.len() as u64, marked.iter().filter_map(|(_, credits)| *credits).sum())
}

#[component]
fn List(
    listing: Listing,
    /// The address the list stands for (where the app is going, if it is going there).
    here: BookmarksUrl,
    /// The order the rows are in („Nach Studium" without a program is the title's).
    sort: BookmarkSort,
    program_set: bool,
    statuses: Memo<BTreeMap<String, Status>>,
    open: Memo<Option<String>>,
    phone: RwSignal<bool>,
    /// The visitor's collections and notes: what each row says of them, the view's name.
    shelf: Memo<Shelf>,
    ui: CollUi,
    /// The rows selected (`pick`).
    picking: Picking,
) -> impl IntoView {
    let t = i18n::t();
    let bookmarks = Bookmarks::expect();
    let any_marked = bookmarks.is_some_and(|bookmarks| bookmarks.count_untracked() > 0) || listing.count() > 0;
    // The view the list shows: its name after the number, and the row leaves it out.
    let viewed = here.collection;
    let view_name = match viewed {
        CollectionView::Of(id) => shelf.with_untracked(|shelf| shelf.collection(id).map(|collection| collection.name.clone())),
        _ => None,
    };
    let has_rows = listing.count() > 0;
    let Listing { parts, missing, text, filtering } = listing;
    let searching = !text.is_empty();

    // The list as the selection reads it; what has left it is let go of.
    let mut listed = Listed::default();
    for row in parts.iter().flat_map(|part| &part.rows) {
        listed.order.push(row.id.clone());
        listed.rows.insert(row.id.clone(), PickRow { title: row.title.clone(), credits: row.credits, offer: folia_plans::study::Offer::of(row) });
    }
    picking.list(listed);
    Effect::new(move |_| picking.keep_listed());

    let credits: Vec<(String, Option<f64>)> = parts.iter().flat_map(|part| &part.rows).map(|row| (row.id.clone(), row.credits)).chain(missing.iter().map(|id| (id.clone(), None))).collect();
    let numbers = Memo::new(move |_| marked_numbers(bookmarks, &credits));
    // While searching: how many are marked at all.
    let all_marked = Memo::new(move |_| bookmarks.map_or(0, Bookmarks::count) as u64);
    // Near the limit the head of the whole list says so, „248 / 256", from 250 as a warning
    // (oberflaeche.html, „Bei 256 ist Schluss"); else no limit is seen anywhere.
    let whole = viewed == CollectionView::All && !searching && here.filters_set() == 0;
    let near = Memo::new(move |_| whole && all_marked.get() >= LIMIT_SHOWN);
    let at_limit = move || near.get() && all_marked.get() >= LIMIT_WARN;

    // The heads of the columns are the order (folia/design/merken/oberflaeche.html, „die Sortierung
    // … eleganter"): a click on another one orders by it, its first way; a second turns it round.
    // „Modul" is the study's order with Mein Studiengang, else the title's.
    let module_order = if program_set { BookmarkSort::Study } else { BookmarkSort::Title };
    let head = {
        let here = here.clone();
        move |orders: &'static [BookmarkSort], first: BookmarkSort, text: &'static str, class: &'static str| {
            let on = orders.contains(&sort);
            let next = if on { BookmarksUrl { sort, descending: !here.descending, ..here.clone() } } else { BookmarksUrl { sort: first, descending: false, ..here.clone() } };
            let down = first.first_down() != (on && here.descending);
            let arrow = on.then_some(if down { " ↓" } else { " ↑" });
            view! { <a class=class href=move || t.path(&next.with_open(open.get().as_deref()).path()) data-noscroll="" aria-current=on.then_some("true")>{text}{arrow}</a> }
        }
    };
    // The heads of the columns, and in their place the bar of the rows selected (`pick`): as tall,
    // so that no row moves when it comes.
    let cols = move || {
        let module_head = head(if program_set { &[BookmarkSort::Study, BookmarkSort::Title] } else { &[BookmarkSort::Title, BookmarkSort::Study] }, module_order, t.catalog.col_module, "c-module");
        view! {
            // The heads are one stop of Tab, ← and → go from one to the next (enhance.js).
            <div class="cols label" role="toolbar" aria-label=t.catalog.sort_label data-stop="x">
                {module_head}
                <span class="c-resp">{t.catalog.col_responsible}</span>
                <span class="c-exam">{t.catalog.exam}</span>
                {head(&[BookmarkSort::Credits], BookmarkSort::Credits, t.common.credits_unit, "c-lp")}
                {head(&[BookmarkSort::Offer], BookmarkSort::Offer, t.catalog.turnus, "c-turnus")}
                <span class="c-lang">{t.catalog.col_language}</span>
                {head(&[BookmarkSort::Events], BookmarkSort::Events, t.catalog.dates, "c-events")}
                {head(&[BookmarkSort::Added], BookmarkSort::Added, t.bookmarks.col_marked, "c-marked")}
            </div>
        }
    };

    let tags = tags(&here, program_set, t);
    // How many filters are set: the badge of the phone's „Filter" counts them, and an order other
    // than the first, which the sheet holds as well.
    let filters = here.filters_set();
    let fit_line = use_context::<Finder>();
    let without_filters = BookmarksUrl { collection: here.collection, sort: here.sort, descending: here.descending, ..Default::default() };
    let all_view = BookmarksUrl { collection: CollectionView::All, ..here.clone() };
    let all_view_href = move || t.path(&all_view.with_open(open.get().as_deref()).path());
    let all_href = { let target = without_filters.clone(); move || t.path(&target.with_open(open.get().as_deref()).path()) };
    let only_season = here.season.filter(|_| filters == 1);
    let badge = filters + usize::from(here.sort != BookmarkSort::Added || here.descending);

    // The rows, shaded by their place in the whole list across its sections.
    let mut index = 0usize;
    let mut views: Vec<AnyView> = Vec::new();
    for part in parts {
        if let Some(title) = part.title {
            views.push(view! { <div class="sem">{title}" "<small class="num">{part.rows.len()}</small></div> }.into_any());
        }
        for row in part.rows {
            let (target, id, here, status_id) = (row.id.clone(), row.id.clone(), here.clone(), row.id.clone());
            let preview = Signal::derive(move || here.with_open(Some(&target)).path());
            let current = Signal::derive(move || open.get().as_deref() == Some(id.as_str()));
            let status = Signal::derive(move || statuses.with(|statuses| statuses.get(&status_id).map(|status| status.row(t))));
            // Its collections but the one the list shows, and its note: one memo a row (R5).
            let mine_id = row.id.clone();
            let mine = Memo::new(move |_| {
                shelf.with(|shelf| RowMine {
                    collections: shelf.of(&mine_id).into_iter().filter(|collection| viewed != CollectionView::Of(*collection)).filter_map(|collection| shelf.collection(collection).map(|collection| collection.name.clone())).collect(),
                    note: shelf.note(&mine_id).map(str::to_string),
                })
            });
            let shaded = index % 2 == 1;
            index += 1;
            // Selected or not: one memo a row (R5); a finger held still on it selects it.
            let selected = {
                let id = row.id.clone();
                Memo::new(move |_| picking.picked.with(|picked| picked.ids.contains(&id)))
            };
            let on_hold = Callback::new(move |id: String| picking.hold(id));
            views.push(view! { <Row row preview current phone with_program=false dim_unmarked=true shaded swipe=true status mine selected on_hold/> }.into_any());
        }
    }

    view! {
        <div class="list-head">
            <div class="count-row">
                <span class="count num" class:warn=at_limit>{move || {
                    if near.get() { format!("{} / {}", all_marked.get(), MAX_MARKS) } else { format::count(numbers.get().0, t.locale) }
                }}</span>
                <span class="count-label">
                    {let view_name = view_name.clone(); move || match (searching, viewed, view_name.as_deref()) {
                        (true, _, _) => (t.bookmarks.count_found)(all_marked.get()),
                        (false, CollectionView::Without, _) => t.bookmarks.without_label.to_string(),
                        (false, CollectionView::Of(_), Some(name)) => (t.bookmarks.in_collection)(name),
                        _ => (t.bookmarks.count_label)(numbers.get().0, here.season.map(|season| season.label(t.locale)).filter(|_| filters == 1)),
                    }}
                    {move || {
                        let (marked, credits) = numbers.get();
                        (marked > 0).then(|| view! { <span class="count-more num">" · "{format::credits(Some(credits), t.locale)}</span> })
                    }}
                </span>
                <div class="list-tools">
                    {has_rows.then(|| view! { <ListKeys select=true/> })}
                    // On a phone the menu of what the list shows: the collection's, else the whole
                    // list's (on a wide screen they are in the sidebar).
                    {any_marked.then(|| {
                        let of = match viewed { CollectionView::Of(id) if view_name.is_some() => Some(id), _ => None };
                        let label = view_name.as_deref().map_or_else(|| t.bookmarks.list_menu.to_string(), |name| (t.bookmarks.collection_menu)(name));
                        view! {
                            <button class="view-more icon-btn" type="button" aria-haspopup="menu" aria-label=label.clone() title=label on:click=move |ev| {
                                ui.confirming.set(false);
                                ui.menu.set(box_of(&ev).map(|at| (of, at)));
                            }><Icon name="ellipsis"/></button>
                        }
                    })}
                    {any_marked.then(|| view! {
                        <a class="sheet-toggle" href="#sidebar" data-action="sheet-open">
                            <Icon name="sliders-horizontal"/>{t.catalog.filters}{(badge > 0).then(|| view! { <em>{badge}</em> })}
                        </a>
                    })}
                </div>
            </div>
            <div class="active-filters">
                {tags.into_iter().map(|(group, value, target)| view! {
                    <span class="tag">
                        {(!group.is_empty()).then(|| view! { <em>{group}</em>" " })}
                        {value}
                        <a href=move || t.path(&target.with_open(open.get().as_deref()).path()) data-noscroll="" aria-label=t.catalog.remove_filter><Icon name="x"/></a>
                    </span>
                }).collect_view()}
            </div>
            // The finder against a semester without dates: nothing could be checked.
            {move || fit_line.and_then(|finder| finder.view.with(|view| view.line.clone())).map(|line| view! { <p class="hint fit-line">{line}</p> })}
        </div>
        {has_rows.then(|| {
            let selecting = Memo::new(move |_| picking.any());
            move || if selecting.get() { view! { <PickBar picking/> }.into_any() } else { cols().into_any() }
        })}
        // The rows are one stop of Tab, ↑ and ↓ go from row to row (enhance.js, „the keyboard").
        <div
            class="rows scroll"
            id=ROWS_ID
            data-stop="y"
            on:pointerdown=move |ev: leptos::ev::PointerEvent| picking.down(&ev, phone.get_untracked())
            on:pointermove=move |ev: leptos::ev::PointerEvent| picking.moved(&ev)
            on:pointerup=move |_| picking.end()
            on:pointercancel=move |_| picking.end()
            on:touchmove=move |ev: leptos::ev::TouchEvent| picking.touch_move(&ev)
            on:click=move |ev: leptos::ev::MouseEvent| picking.click(&ev)
            on:contextmenu=move |ev: leptos::ev::MouseEvent| picking.context(&ev)
            on:keydown=move |ev: leptos::ev::KeyboardEvent| picking.keys(&ev)
            // A row's link is not dragged away: a mouse drawn over the rows selects them.
            on:dragstart=move |ev: leptos::ev::DragEvent| ev.prevent_default()
        >
            {(!any_marked).then(|| view! {
                <div class="state">
                    <p class="state-title">{t.bookmarks.empty_title}</p>
                    <p>{t.bookmarks.empty_hint_start}<kbd>"M"</kbd>{t.bookmarks.empty_hint_end}</p>
                    <a class="btn secondary" href=t.path(url::CATALOG)>{t.common.to_catalog}</a>
                </div>
            })}
            {(any_marked && !has_rows && searching).then(|| view! {
                <div class="state">
                    <p class="state-title">{(t.bookmarks.nothing_found)(&text)}</p>
                    <p>{t.bookmarks.nothing_found_hint}</p>
                </div>
            })}
            {(any_marked && !has_rows && !searching && filtering).then(|| match only_season {
                Some(season) => {
                    let season = season.label(t.locale);
                    view! {
                        <div class="state">
                            <p class="state-title">{(t.bookmarks.nothing_in_season)(season)}</p>
                            <p>{(t.bookmarks.nothing_in_season_hint)(season)}</p>
                            <a class="btn secondary" href=all_href.clone() data-noscroll="">{t.bookmarks.show_all_saved}</a>
                        </div>
                    }
                    .into_any()
                }
                None => view! {
                    <div class="state">
                        <p class="state-title">{t.bookmarks.nothing_filtered}</p>
                        <p>{t.bookmarks.nothing_filtered_hint}</p>
                        <a class="btn secondary" href=all_href.clone() data-noscroll="">{t.bookmarks.show_all_saved}</a>
                    </div>
                }
                .into_any(),
            })}
            // A view with nothing in it: an empty collection, one that is not there, „Ohne Sammlung"
            // with everything in collections.
            {(any_marked && !has_rows && !searching && !filtering).then(|| match (viewed, view_name.clone()) {
                (CollectionView::Of(_), Some(name)) => view! {
                    <div class="state">
                        <p class="state-title">{(t.bookmarks.empty_collection)(&name)}</p>
                        <p>{t.bookmarks.empty_collection_hint}</p>
                    </div>
                }
                .into_any(),
                (CollectionView::Of(_), None) => view! {
                    <div class="state">
                        <p class="state-title">{t.bookmarks.no_such_collection}</p>
                        <p>{t.bookmarks.no_such_collection_hint}</p>
                        <a class="btn secondary" href=all_view_href.clone() data-noscroll="">{t.bookmarks.show_all_saved}</a>
                    </div>
                }
                .into_any(),
                (CollectionView::All, _) => ().into_any(),
                (CollectionView::Without, _) => view! {
                    <div class="state">
                        <p class="state-title">{t.bookmarks.nothing_without}</p>
                        <p>{t.bookmarks.nothing_without_hint}</p>
                        <a class="btn secondary" href=all_view_href.clone() data-noscroll="">{t.bookmarks.show_all_saved}</a>
                    </div>
                }
                .into_any(),
            })}
            {views}
            {(!missing.is_empty()).then(|| view! {
                <div class="sem">{t.bookmarks.not_in_catalog}</div>
                {missing.into_iter().map(|id| view! { <MissingRow id/> }).collect_view()}
            })}
            {(searching && any_marked).then(|| view! { <CatalogLine text=text.clone()/> })}
        </div>
    }
}

/// Under what the search found among the marked modules: the same text in the whole catalog,
/// with how many modules it finds there (folia/design/merken/konzept.html: „Der Weg in den ganzen
/// Katalog bleibt eine Zeile").
#[component]
fn CatalogLine(text: String) -> impl IntoView {
    let t = i18n::t();
    let query = CatalogQuery { text: text.clone(), ..Default::default() };
    let href = t.path(&CatalogUrl { query: query.clone(), ..Default::default() }.path());
    let summary = use_ask(move || Some(CatalogSummaryAsk { query: query.clone() }));
    let found = move || match summary.get() {
        Some(Ok(summary)) if summary.total > 0 => (t.bookmarks.catalog_modules)(summary.total, &format::count(summary.total, t.locale)),
        _ => t.bookmarks.catalog_search.to_string(),
    };
    view! {
        <a class="catalog-line" href=href>
            <Icon name="search"/>
            <span>{(t.bookmarks.in_catalog)(&text)}" "<small>"· "{found}</small></span>
            <Icon name="chevron-right"/>
        </a>
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

// ---- the sidebar ----

/// The sidebar: what belongs to the list as a whole (R17), on a phone the sheet behind „Filter".
/// Its filters, each with how many modules it holds and there once it separates something
/// (folia/design/merken/konzept.html, „Filter, die nur da sind, wenn sie etwas trennen"); on a
/// phone the orders; and what can be done with all of the list. Rendered once: what it shows
/// follows `panel`, the address's filter or the phone's draft.
#[component]
fn Sidebar(
    panel: Memo<BookmarksUrl>,
    numbers: Memo<Numbers>,
    program: Memo<Option<String>>,
    plan_has_modules: Memo<bool>,
    current: Memo<Option<SemesterKey>>,
    pick: Callback<BookmarksUrl>,
    any_marked: Memo<bool>,
    sheet_count: Memo<usize>,
) -> impl IntoView {
    let t = i18n::t();
    let link = move |change: fn(&mut BookmarksUrl)| {
        move || {
            let mut next = panel.get();
            change(&mut next);
            t.path(&next.path())
        }
    };

    // „Angeboten im": there while the marked modules are not all offered alike, or it filters.
    let season_link = move |season: Option<Season>, label: &'static str| {
        let count = move || numbers.with(|n| match season {
            None => n.seasons_all,
            Some(Season::Winter) => n.winter,
            Some(Season::Summer) => n.summer,
        });
        view! {
            <a
                href=move || t.path(&BookmarksUrl { season, ..panel.get() }.path())
                role="radio"
                draggable="false"
                data-noscroll=""
                aria-checked=move || if panel.with(|url| url.season == season) { "true" } else { "false" }
                data-command=label
                data-command-state=move || panel.with(|url| url.season == season).then_some(t.catalog.chosen)
            >
                <span class="seg-label">{label}</span><span class="num">{move || format::count(count() as u64, t.locale)}</span>
            </a>
        }
    };
    let seasons_apart = Memo::new(move |_| numbers.with(|n| n.seasons_apart) || panel.with(|url| url.season.is_some()));
    // „Stand": there once „Mein Studium" says something of a marked module, or it filters.
    // One choice at a time; the chosen one again takes the filter away. Chips, not a segmented
    // control: „Bestanden" and its number need the width.
    let stand_chips = move || {
        Stand::ALL
            .iter()
            .map(|stand| {
                let stand = *stand;
                let count = numbers.with(|n| match stand {
                    Stand::Open => n.open,
                    Stand::Planned => n.planned,
                    Stand::Passed => n.passed,
                });
                view! {
                    <ToggleLink
                        label=stand.label(t.locale)
                        count
                        on=Signal::derive(move || panel.with(|url| url.stand == Some(stand)))
                        href=Signal::derive(move || {
                            let mut next = panel.get();
                            next.stand = if next.stand == Some(stand) { None } else { Some(stand) };
                            t.path(&next.path())
                        })
                    />
                }
            })
            .collect_view()
    };
    let stand_shown = Memo::new(move |_| numbers.with(|n| n.any_status) || panel.with(|url| url.stand.is_some()));
    // „Mein Studium": what counts in Mein Studiengang, with one; what fits the Stundenplan, once it
    // holds something.
    let counts_shown = Memo::new(move |_| program.with(Option::is_some));
    let fits_shown = Memo::new(move |_| current.with(Option::is_some) && (plan_has_modules.get() || panel.with(|url| url.filter.fits.is_some())));
    let study_shown = Memo::new(move |_| counts_shown.get() || fits_shown.get());

    // „Weitere Filter": four of the catalog's, folded unless one of them is set.
    let more_open = panel.with_untracked(|url| {
        let f = &url.filter;
        !f.languages.is_empty() || f.credits_min.is_some() || f.credits_max.is_some() || !f.campuses.is_empty() || f.graded.is_some()
    });
    let language_chip = move |language: Language| view! {
        <ToggleLink
            label=language.label(t.locale)
            on=Signal::derive(move || panel.with(|url| url.filter.languages.contains(&language)))
            href=Signal::derive(move || {
                let mut next = panel.get();
                let languages = &mut next.filter.languages;
                if languages.contains(&language) { languages.retain(|other| *other != language) } else { *languages = Language::ALL.iter().copied().filter(|other| languages.contains(other) || *other == language).collect() }
                t.path(&next.path())
            })
        />
    };
    let campus_chip = move |campus: Campus| view! {
        <ToggleLink
            label=campus.label(t.locale)
            on=Signal::derive(move || panel.with(|url| url.filter.campuses.contains(&campus)))
            href=Signal::derive(move || {
                let mut next = panel.get();
                let campuses = &mut next.filter.campuses;
                if campuses.contains(&campus) { campuses.retain(|other| *other != campus) } else { *campuses = Campus::ALL.iter().copied().filter(|other| campuses.contains(other) || *other == campus).collect() }
                t.path(&next.path())
            })
        />
    };
    // The credits as the catalog's fields have them, from and to.
    let credits_field = move |low: bool| {
        let value = move || panel.with(|url| if low { url.filter.credits_min } else { url.filter.credits_max }).map(|value| value.to_string()).unwrap_or_default();
        let changed = move |ev: leptos::ev::Event| {
            let typed = event_target_value(&ev).trim().replace(',', ".").parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0);
            let mut next = panel.get_untracked();
            if low { next.filter.credits_min = typed } else { next.filter.credits_max = typed }
            pick.run(next);
        };
        view! {
            <input
                type="number"
                min="0"
                max="60"
                step="0.5"
                inputmode="decimal"
                placeholder=if low { t.catalog.from } else { t.catalog.to }
                aria-label=if low { t.catalog.credits_min } else { t.catalog.credits_max }
                prop:value=value
                on:change=changed
            />
        }
    };

    // The orders, as the heads of the columns are on a wide screen (there this list is hidden):
    // the study's with Mein Studiengang, else the title's.
    let order_link = move |sort: BookmarkSort| {
        view! {
            <a
                href=move || {
                    let here = panel.get();
                    let on = here.sort == sort;
                    t.path(&BookmarksUrl { sort, descending: on && !here.descending, ..here }.path())
                }
                draggable="false"
                data-noscroll=""
                aria-current=move || panel.with(|url| url.sort == sort).then_some("page")
            >
                {sort.label(t.locale)}
                {move || panel.with(|url| url.sort == sort).then(|| if sort.first_down() != panel.with(|url| url.descending) { " ↓" } else { " ↑" })}
            </a>
        }
    };
    let orders = move || {
        let study_or_title = if program.with(Option::is_some) { BookmarkSort::Study } else { BookmarkSort::Title };
        [BookmarkSort::Added, study_or_title, BookmarkSort::Offer, BookmarkSort::Credits, BookmarkSort::Events].into_iter().map(order_link).collect_view()
    };

    view! {
        <Show when=move || any_marked.get()>
            <Show when=move || seasons_apart.get()>
                <div class="fgroup" data-section="">
                    <p class="flabel label">{t.bookmarks.offered_in}</p>
                    <div class="seg" role="radiogroup" aria-label=t.bookmarks.offered_in data-command-of=t.bookmarks.offered_in>
                        {season_link(None, t.bookmarks.all)}
                        {season_link(Some(Season::Winter), Season::Winter.label(t.locale))}
                        {season_link(Some(Season::Summer), Season::Summer.label(t.locale))}
                    </div>
                </div>
            </Show>
            <Show when=move || stand_shown.get()>
                <div class="fgroup" data-section="">
                    <p class="flabel label">{t.bookmarks.stand}</p>
                    <div class="chips" data-command-of=t.bookmarks.stand>{stand_chips}</div>
                </div>
            </Show>
            <Show when=move || study_shown.get()>
                <div class="fgroup" data-section="">
                    <p class="flabel label">{t.bookmarks.my_study}</p>
                    <div class="chips" data-command-of=t.bookmarks.my_study>
                        {move || counts_shown.get().then(|| view! {
                            <ToggleLink
                                label=t.bookmarks.counts
                                count=numbers.with(|n| n.counting)
                                on=Signal::derive(move || panel.with(|url| url.counts))
                                href=Signal::derive(link(|url| url.counts = !url.counts))
                            />
                        })}
                        {move || fits_shown.get().then(|| current.get().map(|semester| view! {
                            <ToggleLink
                                label=t.catalog.fits
                                on=Signal::derive(move || panel.with(|url| url.filter.fits.is_some()))
                                href=Signal::derive(move || {
                                    let mut next = panel.get();
                                    next.filter.fits = if next.filter.fits.is_some() { None } else { Some(finder_on(semester)) };
                                    t.path(&next.path())
                                })
                            />
                        }))}
                    </div>
                </div>
            </Show>
            <details class="fgroup more" data-section="" open=more_open>
                <summary class="label">{t.catalog.more_filters}</summary>
                <div class="flabel label">{t.catalog.language}</div>
                <div class="chips" data-command-of=t.catalog.language>{Language::ALL.iter().map(|language| language_chip(*language)).collect_view()}</div>
                <div class="flabel label">{t.catalog.credit_points}</div>
                <div class="range">{credits_field(true)}<span>"–"</span>{credits_field(false)}</div>
                <div class="flabel label">{t.catalog.location}</div>
                <div class="chips" data-command-of=t.catalog.location>{[Campus::Zentralcampus, Campus::Sachsendorf, Campus::Senftenberg].into_iter().map(campus_chip).collect_view()}</div>
                <div class="flabel label">{t.catalog.exam}</div>
                <div class="chips" data-command-of=t.catalog.exam>
                    <ToggleLink
                        label=t.catalog.graded_chip
                        on=Signal::derive(move || panel.with(|url| url.filter.graded == Some(true)))
                        href=Signal::derive(link(|url| url.filter.graded = if url.filter.graded == Some(true) { None } else { Some(true) }))
                    />
                </div>
            </details>
            <nav class="toc fgroup order-list" aria-label=t.bookmarks.order>
                <p class="flabel label">{t.bookmarks.order}</p>
                {orders}
            </nav>
        </Show>
        <div class="filter-actions sheet-only" data-sheet-foot="">
            <a class="btn primary show" href="#" data-action="sheet-apply">{move || {
                let n = sheet_count.get() as u64;
                (t.catalog.show)(n, &format::count(n, t.locale))
            }}</a>
        </div>
    }
}

// ---- the collections ----

/// How many marked modules each view of the list holds: all, those in no collection, each
/// collection in its order (konzept.html: „Ihre Zahlen sind ihre eigenen").
#[derive(Clone, Debug, Default, PartialEq)]
struct Views {
    all: usize,
    without: usize,
    each: Vec<(u32, String, usize)>,
}

fn views_of(shelf: &Shelf, marked: &BTreeSet<String>) -> Views {
    Views {
        all: marked.len(),
        without: marked.iter().filter(|id| shelf.of(id).is_empty()).count(),
        each: shelf.collections().iter().map(|collection| (collection.id, collection.name.clone(), shelf.count_in(collection.id, |module| marked.contains(module)))).collect(),
    }
}

/// What the collections' parts of the page have open (the sidebar's on a wide screen, the chips
/// over the list on a phone): a menu, the field that names a collection.
#[derive(Clone, Copy)]
struct CollUi {
    /// The menu open: of the whole list (`None`) or of a collection, at the box of what opened it.
    menu: RwSignal<Option<(Option<u32>, Anchor)>>,
    /// The field that names a collection: a new one (`None`) or the one renamed.
    naming: RwSignal<Option<Option<u32>>>,
    /// What is typed into it.
    typed: RwSignal<String>,
    /// The menu of the whole list asks before it empties it.
    confirming: RwSignal<bool>,
    /// A collection's menu shows „Teilen" (oberflaeche.html, „Weitergeben": „ein Menü mit drei
    /// Zeilen").
    sharing: RwSignal<bool>,
    /// Goes to a view of the list, keeping its filters and its order.
    go: Callback<CollectionView>,
}

impl CollUi {
    fn rename(self, id: u32, name: &str) {
        self.menu.set(None);
        self.typed.set(name.to_string());
        self.naming.set(Some(Some(id)));
    }
}

/// The box of the element an event was heard on.
pub(crate) fn box_of(ev: &leptos::ev::Event) -> Option<Anchor> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let rect = ev.current_target()?.dyn_into::<web_sys::Element>().ok()?.get_bounding_client_rect();
        Some(Anchor { left: rect.left(), top: rect.top(), right: rect.right(), bottom: rect.bottom() })
    }
    #[cfg(not(feature = "csr"))]
    {
        let _ = ev;
        None
    }
}

/// The field that names a collection, where its row or its chip stands: Enter makes it (a name
/// there is that collection, which opens) or renames it (a name another one has is refused), Esc
/// and leaving it let it go.
#[component]
fn NameField(ui: CollUi, chip: bool) -> impl IntoView {
    let t = i18n::t();
    let b = &t.bookmarks;
    let collections = Collections::expect();
    let renaming = ui.naming.get_untracked().flatten();
    let input = NodeRef::<leptos::html::Input>::new();
    let full = RwSignal::new(false);
    request_animation_frame(move || {
        if let Some(input) = input.get_untracked() {
            let _ = input.focus();
            input.select();
        }
    });
    let taken = Memo::new(move |_| {
        let typed = ui.typed.get();
        collections.and_then(|collections| collections.with(|shelf| shelf.find(&typed))).filter(|found| Some(*found) != renaming)
    });
    let finish = move || {
        ui.naming.set(None);
        ui.typed.set(String::new());
    };
    let enter = move || {
        let Some(collections) = collections else { return };
        let name = ui.typed.get_untracked();
        match renaming {
            None => match collections.update(|shelf| shelf.create(&name)) {
                Ok(id) | Err(Refused::Taken(id)) => {
                    finish();
                    ui.go.run(CollectionView::Of(id));
                }
                Err(Refused::Empty) => finish(),
                Err(Refused::Full) => full.set(true),
            },
            Some(id) => match collections.update(|shelf| shelf.rename(id, &name)) {
                Ok(()) | Err(Refused::Empty) => finish(),
                Err(Refused::Taken(_) | Refused::Full) => {}
            },
        }
    };
    let keys = move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
        "Enter" => {
            ev.prevent_default();
            enter();
        }
        "Escape" => {
            ev.prevent_default();
            ev.stop_propagation();
            finish();
        }
        _ => {}
    };
    let said = move || {
        if full.get() {
            Some(t.marks.collections_full)
        } else {
            taken.get().map(|_| if renaming.is_some() { b.taken } else { b.taken_open })
        }
    };
    view! {
        <span class=if chip { "coll-chip input" } else { "coll-item input" } class:taken=move || said().is_some()>
            <input
                node_ref=input
                type="text"
                maxlength="60"
                autocomplete="off"
                enterkeyhint="done"
                placeholder=b.collection_name
                aria-label=if renaming.is_some() { b.collection_name } else { b.new_collection_name }
                prop:value=move || ui.typed.get()
                on:input=move |ev| {
                    full.set(false);
                    ui.typed.set(event_target_value(&ev));
                }
                on:keydown=keys
                on:blur=move |_| finish()
            />
        </span>
        {move || said().map(|said| view! { <span class="coll-taken" role="status">{said}</span> })}
    }
}

/// The views of the list on a wide screen, at the top of the sidebar (oberflaeche.html, „Am
/// Rechner", 1): „Alle", „Ohne Sammlung" once there is a collection, each collection with its
/// number, „+ Neue Sammlung". The one shown is dark. A collection's ⋯ (on hover or focus, and a right
/// click) opens its menu, F2 renames it, Entf (⌫ on a Mac) asks whether to delete it; „Alle" has a
/// menu of its own.
#[component]
fn CollectionsNav(ui: CollUi, views: Memo<Views>, here: Memo<BookmarksUrl>) -> impl IntoView {
    let t = i18n::t();
    let b = &t.bookmarks;
    let item = move |view: CollectionView, name: String, count: usize| {
        let menu_of = match view {
            CollectionView::All => Some(None),
            CollectionView::Of(id) => Some(Some(id)),
            CollectionView::Without => None,
        };
        let label = match view {
            CollectionView::Of(_) => (b.collection_menu)(&name),
            _ => b.list_menu.to_string(),
        };
        let open_menu = move |ev: leptos::ev::MouseEvent| {
            if let Some(of) = menu_of {
                ev.prevent_default();
                ui.confirming.set(false);
                ui.menu.set(box_of(&ev).map(|at| (of, at)));
            }
        };
        let (renamed, command_name) = (name.clone(), name.clone());
        let keys = move |ev: leptos::ev::KeyboardEvent| {
            let CollectionView::Of(id) = view else { return };
            match ev.key().as_str() {
                "F2" => {
                    ev.prevent_default();
                    ui.rename(id, &renamed);
                }
                // What „Löschen" in its menu does, asked first: a key is soon pressed (owner,
                // 2026-10-20: „wo man dann aber nochmal gefragt wird"). The question stands where
                // the menu would, its „Löschen" with the focus.
                "Delete" | "Backspace" if !ev.repeat() && !ev.ctrl_key() && !ev.meta_key() && !ev.alt_key() => {
                    ev.prevent_default();
                    ui.sharing.set(false);
                    ui.confirming.set(true);
                    ui.menu.set(box_of(&ev).map(|at| (Some(id), at)));
                }
                _ => {}
            }
        };
        view! {
            // A command of „Befehle" (folia_shell::commands): „Sammlung: Für Lena".
            <div class="coll-item" class:derived=view == CollectionView::Without class:menu=move || menu_of.is_some_and(|of| ui.menu.with(|menu| menu.as_ref().is_some_and(|(open, _)| *open == of))) data-command-of=b.collection>
                <a
                    href=move || t.path(&BookmarksUrl { collection: view, ..here.get() }.path())
                    data-command=command_name
                    aria-current=move || here.with(|url| url.collection == view).then_some("page")
                    data-noscroll=""
                    on:keydown=keys
                    on:contextmenu=open_menu
                >
                    <span class="coll-name">{name}</span>
                    <span class="coll-count num">{count}</span>
                </a>
                {menu_of.is_some().then(|| view! {
                    <button class="coll-more" type="button" aria-haspopup="menu" aria-label=label.clone() title=label on:click=open_menu><Icon name="ellipsis"/></button>
                })}
            </div>
        }
    };
    view! {
        // A section of the sidebar's mode, whose ↑ and ↓ go from collection to collection (enhance.js).
        <nav class="fgroup coll-nav" aria-label=b.collections data-section="">
            <p class="flabel label">{b.collections}</p>
            {move || {
                let Views { all, without, each } = views.get();
                let naming = ui.naming.get();
                let mut parts: Vec<AnyView> = vec![item(CollectionView::All, t.bookmarks.all.to_string(), all).into_any()];
                if !each.is_empty() {
                    parts.push(item(CollectionView::Without, t.marks.without_collection.to_string(), without).into_any());
                }
                for (id, name, count) in each {
                    parts.push(if naming == Some(Some(id)) { view! { <NameField ui chip=false/> }.into_any() } else { item(CollectionView::Of(id), name, count).into_any() });
                }
                parts.push(if naming == Some(None) { view! { <NameField ui chip=false/> }.into_any() } else { add_button(ui, false) });
                parts
            }}
        </nav>
    }
}

/// How long a finger rests on a chip before its menu comes (ms).
const PRESS: u64 = 480;

/// The views of the list on a phone, as chips over the list (oberflaeche.html, „Am Handy", 2;
/// owner, 2026-10-19: „Ja gruppen als chips ist gut"), once there is a collection: they scroll
/// sideways. „+" makes one; a chip held (or a right click) opens its menu.
#[component]
fn CollectionChips(ui: CollUi, views: Memo<Views>, here: Memo<BookmarksUrl>) -> impl IntoView {
    let t = i18n::t();
    let b = &t.bookmarks;
    let chip = move |view: CollectionView, name: String, count: usize| {
        let menu_of = match view {
            CollectionView::All => Some(None),
            CollectionView::Of(id) => Some(Some(id)),
            CollectionView::Without => None,
        };
        // A finger that rests: the menu, and the tap that would follow is not a tap.
        let pressing = StoredValue::new(None::<TimeoutHandle>);
        let pressed = StoredValue::new(false);
        let rest = move |ev: leptos::ev::PointerEvent| {
            pressed.set_value(false);
            if ev.pointer_type() == "mouse" {
                return;
            }
            let Some(of) = menu_of else { return };
            let at = box_of(&ev);
            let handle = set_timeout_with_handle(
                move || {
                    pressed.set_value(true);
                    ui.confirming.set(false);
                    ui.menu.set(at.map(|at| (of, at)));
                },
                std::time::Duration::from_millis(PRESS),
            );
            pressing.set_value(handle.ok());
        };
        let lift = move |_: leptos::ev::PointerEvent| {
            if let Some(handle) = pressing.try_update_value(Option::take).flatten() {
                handle.clear();
            }
        };
        let click = move |ev: leptos::ev::MouseEvent| {
            if pressed.get_value() {
                ev.prevent_default();
                pressed.set_value(false);
            }
        };
        let context = move |ev: leptos::ev::MouseEvent| {
            ev.prevent_default();
            if let Some(of) = menu_of {
                if !pressed.get_value() {
                    ui.confirming.set(false);
                    ui.menu.set(box_of(&ev).map(|at| (of, at)));
                }
            }
        };
        view! {
            <a
                class="coll-chip"
                class:derived=view == CollectionView::Without
                href=move || t.path(&BookmarksUrl { collection: view, ..here.get() }.path())
                aria-current=move || here.with(|url| url.collection == view).then_some("page")
                data-noscroll=""
                draggable="false"
                on:pointerdown=rest
                on:pointerup=lift
                on:pointercancel=lift
                on:pointerleave=lift
                on:click=click
                on:contextmenu=context
            >
                {name}<span class="num">{count}</span>
            </a>
        }
    };
    move || {
        let Views { all, without, each } = views.get();
        if each.is_empty() && ui.naming.with(Option::is_none) {
            return None;
        }
        let naming = ui.naming.get();
        let mut parts: Vec<AnyView> = vec![chip(CollectionView::All, t.bookmarks.all.to_string(), all).into_any(), chip(CollectionView::Without, t.marks.without_collection.to_string(), without).into_any()];
        for (id, name, count) in each {
            parts.push(if naming == Some(Some(id)) { view! { <NameField ui chip=true/> }.into_any() } else { chip(CollectionView::Of(id), name, count).into_any() });
        }
        parts.push(if naming == Some(None) { view! { <NameField ui chip=true/> }.into_any() } else { add_button(ui, true) });
        Some(view! { <nav class="coll-chips" aria-label=b.collections>{parts}</nav> })
    }
}

/// „+ Neue Sammlung": its field in its place.
fn add_button(ui: CollUi, chip: bool) -> AnyView {
    let t = i18n::t();
    let start = move |_| {
        ui.typed.set(String::new());
        ui.naming.set(Some(None));
    };
    if chip {
        view! {
            <button class="coll-chip add" type="button" aria-label=t.bookmarks.new_collection title=t.bookmarks.new_collection on:click=start>
                <Icon name="plus"/>
            </button>
        }
        .into_any()
    } else {
        view! {
            <button class="coll-item add" type="button" on:click=start data-command="">
                <Icon name="plus"/><span class="coll-name">{t.bookmarks.new_collection}</span>
            </button>
        }
        .into_any()
    }
}

/// An entry of a menu of the page: its icon, its words, the key beside it, whether it is the one
/// that takes something away.
struct Entry {
    icon: &'static str,
    label: String,
    hint: Option<&'static str>,
    key: Option<&'static str>,
    danger: bool,
    does: Callback<()>,
}

/// The sizes of a menu on a wide screen (app.css `.st-pop`), to place it before it is drawn.
pub(crate) const MENU_WIDTH: f64 = 248.0;
pub(crate) const MENU_ITEM: f64 = 36.0;
pub(crate) const MENU_HINT: f64 = 50.0;
const MENU_EDGE: f64 = 8.0;

/// Where a menu `wide` px wide and `height` tall stands for what opened it at `at` in the `room`
/// of the window: under it, its right edge at that one's, above it where there is no room below;
/// never past the window's edges, nor under the rail.
fn menu_place(at: Anchor, wide: f64, height: f64, room: nav::Room) -> (f64, f64) {
    let least = room.left + MENU_EDGE;
    let left = (at.right - wide).clamp(least, (room.width - wide - MENU_EDGE).max(least));
    let below = at.bottom + 4.0;
    let top = if below + height <= room.height - MENU_EDGE { below } else { (at.top - 4.0 - height).max(MENU_EDGE) };
    (left, top)
}

/// Copies a text into the clipboard (a click's: the browser lets it).
pub(crate) fn copy(text: &str) {
    #[cfg(feature = "csr")]
    if let Some(window) = web_sys::window() {
        let _ = window.navigator().clipboard().write_text(text);
    }
    #[cfg(not(feature = "csr"))]
    let _ = text;
}

/// The address of this site a path of it becomes.
pub(crate) fn absolute(path: &str) -> String {
    #[cfg(feature = "csr")]
    if let Some(origin) = web_sys::window().and_then(|window| window.location().origin().ok()) {
        return format!("{origin}{path}");
    }
    path.to_string()
}

/// The menu of a collection, or of the whole list (oberflaeche.html, „Sammlungen verwalten"; „Was
/// eine Sammlung betrifft, steht in ihrem Menü"): on a wide screen at what opened it, on a phone a
/// sheet from below (app.css `.st-pop`). A collection's: „Umbenennen" (F2), „Teilen", „Nach oben",
/// „Nach unten", „Löschen" (Entf) — which, chosen, asks nothing: its modules stay marked, and the
/// note after it offers „Rückgängig" and to take away those that were in it alone; its key asks
/// first, here and at the collection in the sidebar (owner, 2026-10-20). The list's: the link for
/// another device, the list as text, „Merkliste leeren", which asks. The focus goes back to what
/// opened it when it closes and leaves the focus nowhere.
#[component]
fn ViewMenu(ui: CollUi, here: Memo<BookmarksUrl>, data: Memo<Option<BookmarksData>>, picking: Picking, phone: RwSignal<bool>) -> impl IntoView {
    let t = i18n::t();
    let b = &t.bookmarks;
    let bookmarks = Bookmarks::expect();
    let collections = Collections::expect();
    let notices = Notices::expect();
    let close = move || {
        ui.menu.set(None);
        ui.confirming.set(false);
        ui.sharing.set(false);
    };
    folia_shell::nav::layer_while(move || ui.menu.with(Option::is_some), close, folia_shell::nav::Kind::Over);
    // What had the focus as it opened (a collection in the sidebar, its ⋯) has it back as it closes,
    // where nothing else took it (Esc, „Abbrechen", a click beside it); where that is gone (its
    // collection deleted), the view the sidebar shows.
    let opener = StoredValue::new(None::<send_wrapper::SendWrapper<leptos::web_sys::HtmlElement>>);
    Effect::new(move |was: Option<bool>| {
        let open = ui.menu.with(Option::is_some);
        match (was.unwrap_or(false), open) {
            (false, true) => opener.set_value(focused()),
            (true, false) => {
                let back = opener.try_update_value(Option::take).flatten();
                request_animation_frame(move || focus_back(back));
            }
            _ => {}
        }
        open
    });
    // Its first entry has the focus as it opens, the arrows go on from there.
    Effect::new(move |_| {
        if ui.menu.with(Option::is_some) {
            request_animation_frame(|| {
                nav::focus_selector(".view-menu [role=menuitem]");
            });
        }
    });
    let keys = move |ev: leptos::ev::KeyboardEvent| {
        let step: i32 = match ev.key().as_str() {
            "ArrowDown" => 1,
            "ArrowUp" => -1,
            "Escape" | "Tab" => {
                ev.prevent_default();
                ev.stop_propagation();
                close();
                return;
            }
            // The keys its entries show, of a collection's menu: F2 renames it, Entf (⌫) asks
            // whether to delete it — once: in the question the key does nothing more.
            key @ ("F2" | "Delete" | "Backspace") => {
                let Some((Some(id), _)) = ui.menu.get_untracked() else { return };
                ev.prevent_default();
                ev.stop_propagation();
                if key == "F2" {
                    let name = collections.and_then(|collections| collections.with_untracked(|shelf| shelf.collection(id).map(|collection| collection.name.clone())));
                    if let Some(name) = name {
                        close();
                        ui.rename(id, &name);
                    }
                } else if !ui.confirming.get_untracked() && !ev.repeat() && !ev.ctrl_key() && !ev.meta_key() && !ev.alt_key() {
                    ui.sharing.set(false);
                    ui.confirming.set(true);
                    request_animation_frame(|| nav::focus_by_id("delete-yes"));
                }
                return;
            }
            _ => return,
        };
        ev.prevent_default();
        step_focus(step);
    };
    move || {
        let (of, at) = ui.menu.get()?;
        let entries: Vec<Entry> = match of {
            Some(id) => {
                let collections = collections?;
                let (name, first, last) = collections.with_untracked(|shelf| {
                    let all = shelf.collections();
                    let at = all.iter().position(|collection| collection.id == id);
                    (all.iter().find(|collection| collection.id == id).map(|collection| collection.name.clone()), at == Some(0), at.is_some_and(|at| at + 1 == all.len()))
                });
                let renamed = name?;
                // „Löschen": its modules stay marked; the note after it offers „Rückgängig", and to
                // take away those that were in it alone („Auch 2 entfernen").
                let delete = Callback::new(move |()| {
                    close();
                    let Some(deleted) = collections.update(|shelf| shelf.delete(id)) else { return };
                    let alone: Vec<String> = collections.with_untracked(|shelf| deleted.modules.iter().filter(|module| shelf.of(module).is_empty() && bookmarks.is_some_and(|bookmarks| bookmarks.is_marked_untracked(module))).cloned().collect());
                    if let Some(notices) = notices {
                        notices.say(Notice::Deleted { deleted, alone, taken: None });
                    }
                    if here.with_untracked(|url| url.collection == CollectionView::Of(id)) {
                        ui.go.run(CollectionView::All);
                    }
                });
                if ui.confirming.get() {
                    // Its key asked: „‚Für Lena‘ löschen?", what stays, „Löschen" with the focus.
                    let kept = bookmarks.map(Bookmarks::marks_untracked).unwrap_or_default().iter().filter(|mark| collections.with_untracked(|shelf| shelf.holds(id, &mark.id))).count();
                    let question = (b.delete_question)(&renamed);
                    let label = question.clone();
                    let height = 2.0 * MENU_ITEM + 60.0 + if kept > 0 { 18.0 } else { 0.0 };
                    return Some(view! {
                        <div class="st-pop-shade" on:mousedown=|ev: leptos::ev::MouseEvent| ev.prevent_default() on:click=move |_| close()></div>
                        <div data-sheet="" class="st-pop root view-menu ask" role="alertdialog" aria-label=label style=menu_style(at, height) on:keydown=keys>
                            <p class="view-menu-ask">{question}{(kept > 0).then(|| view! { <small>{(b.delete_keeps)(kept)}</small> })}</p>
                            <div class="view-menu-answers">
                                <button class="mini danger hit" type="button" role="menuitem" id="delete-yes" on:click=move |_| delete.run(())>{b.delete}</button>
                                <button class="mini hit" type="button" role="menuitem" on:click=move |_| close()>{b.cancel}</button>
                            </div>
                        </div>
                    }.into_any());
                }
                if ui.sharing.get() {
                    // „Teilen": its marked modules, with its name, and their notes where
                    // „Notizen mitnehmen" is on.
                    let ids: Vec<String> = bookmarks.map(Bookmarks::marks_untracked).unwrap_or_default().into_iter().map(|mark| mark.id).filter(|module| collections.with_untracked(|shelf| shelf.holds(id, module))).collect();
                    let count = ids.len();
                    let along = picking.notes_along;
                    let head = (b.share_head)(Some(&renamed), count);
                    let (linked, name) = (ids.clone(), renamed.clone());
                    let copy_link = move |_| {
                        let notes = if along.get_untracked() { collections.with_untracked(|shelf| notes_of(shelf, &linked)) } else { Vec::new() };
                        let brought = Brought { ids: linked.clone(), collections: vec![(name.clone(), linked.clone())], notes };
                        if let Some(link) = share_fragment(&brought).map(|fragment| absolute(&t.path(&format!("{}#{fragment}", url::BOOKMARKS)))) {
                            copy(&link);
                            close();
                            if let Some(notices) = notices {
                                notices.say(Notice::Copied { link: true, count });
                            }
                        }
                    };
                    let text = data.with_untracked(|data| as_text(data.as_ref(), |module| ids.iter().any(|id| id == module), t));
                    let copy_text = move |_| {
                        copy(&text);
                        close();
                        if let Some(notices) = notices {
                            notices.say(Notice::Copied { link: false, count });
                        }
                    };
                    let label = head.clone();
                    return Some(view! {
                        <div class="st-pop-shade" on:mousedown=|ev: leptos::ev::MouseEvent| ev.prevent_default() on:click=move |_| close()></div>
                        <div data-sheet="" class="st-pop root view-menu share-menu" role="menu" aria-label=label style=menu_style(at, 3.0 * MENU_ITEM + 9.0 + 12.0 + 26.0) on:keydown=keys>
                            <p class="view-menu-head">{head}</p>
                            <button class="st-pop-item" type="button" role="menuitem" disabled=count == 0 on:click=copy_link>
                                <Icon name="link"/><span class="st-pop-label"><span>{b.copy_link}</span></span>
                            </button>
                            <button class="st-pop-item share-switch" type="button" role="menuitemcheckbox" aria-checked=move || if along.get() { "true" } else { "false" } on:click=move |_| along.update(|on| *on = !*on)>
                                <span class="st-pop-label"><span>{b.notes_along}</span></span>
                                <span class="collect-switch" aria-hidden="true"></span>
                            </button>
                            <hr/>
                            <button class="st-pop-item" type="button" role="menuitem" disabled=count == 0 on:click=copy_text>
                                <Icon name="copy"/><span class="st-pop-label"><span>{b.copy_text}</span></span>
                            </button>
                        </div>
                    }.into_any());
                }
                let shown_name = renamed.clone();
                let mut entries = vec![Entry { icon: "pencil", label: b.rename.to_string(), hint: None, key: Some("F2"), danger: false, does: Callback::new(move |()| ui.rename(id, &shown_name)) }];
                entries.push(Entry {
                    icon: "share-2",
                    label: b.act_share.to_string(),
                    hint: None,
                    key: None,
                    danger: false,
                    does: Callback::new(move |()| {
                        ui.sharing.set(true);
                        request_animation_frame(|| {
                            nav::focus_selector(".view-menu [role=menuitem]");
                        });
                    }),
                });
                if !first {
                    entries.push(Entry { icon: "arrow-up", label: b.move_up.to_string(), hint: None, key: None, danger: false, does: Callback::new(move |()| { collections.update(|shelf| shelf.shift(id, true)); close(); }) });
                }
                if !last {
                    entries.push(Entry { icon: "arrow-down", label: b.move_down.to_string(), hint: None, key: None, danger: false, does: Callback::new(move |()| { collections.update(|shelf| shelf.shift(id, false)); close(); }) });
                }
                entries.push(Entry { icon: "trash-2", label: b.delete.to_string(), hint: None, key: Some(b.delete_key), danger: true, does: delete });
                entries
            }
            None => {
                let marks: Vec<String> = bookmarks.map(Bookmarks::marks_untracked).unwrap_or_default().into_iter().map(|mark| mark.id).collect();
                let count = marks.len();
                if ui.confirming.get() {
                    return Some(view! {
                        <div class="st-pop-shade" on:mousedown=|ev: leptos::ev::MouseEvent| ev.prevent_default() on:click=move |_| close()></div>
                        <div data-sheet="" class="st-pop root view-menu ask" role="alertdialog" aria-label=b.clear style=menu_style(at, 2.0 * MENU_ITEM + 60.0) on:keydown=keys>
                            <p class="view-menu-ask">{(b.clear_question)(count)}</p>
                            <div class="view-menu-answers">
                                <button class="mini danger hit" type="button" role="menuitem" id="clear-yes" on:click=move |_| {
                                    close();
                                    if let Some(bookmarks) = bookmarks {
                                        let cleared = bookmarks.clear();
                                        if let Some(notices) = notices {
                                            notices.say(Notice::Cleared(cleared));
                                        }
                                    }
                                }>{b.clear_yes}</button>
                                <button class="mini hit" type="button" role="menuitem" on:click=move |_| close()>{b.cancel}</button>
                            </div>
                        </div>
                    }.into_any());
                }
                let text = data.with_untracked(|data| as_text(data.as_ref(), |id| bookmarks.is_some_and(|bookmarks| bookmarks.is_marked_untracked(id)), t));
                // The link for another device takes everything along: the collections with their
                // marked modules, and every note (konzept.html: „Das eigene Gerät bekommt alles").
                let everything = collections.map(|collections| {
                    collections.with_untracked(|shelf| {
                        let held = shelf.collections().iter().map(|collection| (collection.name.clone(), marks.iter().filter(|id| shelf.holds(collection.id, id)).cloned().collect())).collect();
                        (held, notes_of(shelf, &marks))
                    })
                });
                let (held, notes) = everything.unwrap_or_default();
                let link = share_fragment(&Brought { ids: marks.clone(), collections: held, notes }).map(|fragment| absolute(&t.path(&format!("{}#{fragment}", url::BOOKMARKS))));
                let mut entries = Vec::new();
                if let Some(link) = link.filter(|_| count > 0) {
                    entries.push(Entry {
                        icon: "share-2",
                        label: b.transfer.to_string(),
                        hint: Some(b.transfer_hint),
                        key: None,
                        danger: false,
                        does: Callback::new(move |()| {
                            copy(&link);
                            close();
                            if let Some(notices) = notices {
                                notices.say(Notice::Copied { link: true, count });
                            }
                        }),
                    });
                }
                if count > 0 {
                    entries.push(Entry {
                        icon: "copy",
                        label: b.copy_text.to_string(),
                        hint: None,
                        key: None,
                        danger: false,
                        does: Callback::new(move |()| {
                            copy(&text);
                            close();
                            if let Some(notices) = notices {
                                notices.say(Notice::Copied { link: false, count });
                            }
                        }),
                    });
                    entries.push(Entry {
                        icon: "trash-2",
                        label: b.clear.to_string(),
                        hint: None,
                        key: None,
                        danger: true,
                        does: Callback::new(move |()| {
                            ui.confirming.set(true);
                            // The answer has the focus, as the question had it in the sidebar.
                            request_animation_frame(|| nav::focus_by_id("clear-yes"));
                        }),
                    });
                }
                if entries.is_empty() {
                    return None;
                }
                entries
            }
        };
        // On a phone, whose bar of the rows selected has no room for it, „Alle auswählen" heads the
        // menu of what the list shows (oberflaeche.html: „„Alle auswählen" nimmt, was die Liste
        // gerade zeigt").
        let shown = match (of, here.with_untracked(|url| url.collection)) {
            (Some(id), CollectionView::Of(viewed)) => id == viewed,
            (None, CollectionView::All | CollectionView::Without) => true,
            _ => false,
        };
        let mut entries = entries;
        if shown && phone.get_untracked() && picking.can_select_all() {
            entries.insert(0, Entry { icon: "list-checks", label: b.select_all.to_string(), hint: None, key: None, danger: false, does: Callback::new(move |()| { close(); picking.select_all(); }) });
        }
        let title = match of {
            Some(id) => collections.and_then(|collections| collections.with_untracked(|shelf| shelf.collection(id).map(|collection| collection.name.clone()))).unwrap_or_default(),
            None => t.bookmarks.title.to_string(),
        };
        let height = entries.iter().map(|entry| if entry.hint.is_some() { MENU_HINT } else { MENU_ITEM }).sum::<f64>() + 12.0 + if entries.iter().any(|entry| entry.danger) { 9.0 } else { 0.0 };
        let label = title.clone();
        Some(view! {
            <div class="st-pop-shade" on:mousedown=|ev: leptos::ev::MouseEvent| ev.prevent_default() on:click=move |_| close() on:contextmenu=move |ev: leptos::ev::MouseEvent| { ev.prevent_default(); close(); }></div>
            <div data-sheet="" class="st-pop root view-menu" role="menu" aria-label=label style=menu_style(at, height) on:keydown=keys>
                <p class="st-pop-title st-phone">{title}</p>
                {entries.into_iter().map(|Entry { icon, label, hint, key, danger, does }| view! {
                    {danger.then(|| view! { <hr/> })}
                    <button class="st-pop-item" class:danger=danger class:with-hint=hint.is_some() type="button" role="menuitem" on:click=move |_| does.run(())>
                        <Icon name=icon/>
                        <span class="st-pop-label"><span>{label}</span>{hint.map(|hint| view! { <small>{hint}</small> })}</span>
                        {key.map(|key| view! { <kbd class="st-pop-key">{key}</kbd> })}
                    </button>
                }).collect_view()}
            </div>
        }.into_any())
    }
}

/// The element that has the focus, if one has it (not the page's body).
fn focused() -> Option<send_wrapper::SendWrapper<leptos::web_sys::HtmlElement>> {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let document = web_sys::window()?.document()?;
        let active = document.active_element()?.dyn_into::<web_sys::HtmlElement>().ok()?;
        let body = document.body()?;
        (!active.is_same_node(Some(&body))).then(|| send_wrapper::SendWrapper::new(active))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Gives the focus back to `back` as a menu closes, where the focus is nowhere (the menu took it
/// with it); where `back` is gone, to the view the sidebar shows, then to its first.
fn focus_back(back: Option<send_wrapper::SendWrapper<leptos::web_sys::HtmlElement>>) {
    #[cfg(feature = "csr")]
    {
        let Some(document) = web_sys::window().and_then(|window| window.document()) else { return };
        let lost = match (document.active_element(), document.body()) {
            (Some(active), Some(body)) => active.is_same_node(Some(&body)),
            (None, _) => true,
            _ => false,
        };
        if !lost {
            return;
        }
        // On a page in front only: one behind keeps its own focus for when it comes back.
        if let Some(back) = back.filter(|back| back.is_connected() && back.closest(".page-stage:not(.front)").ok().flatten().is_none()) {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = back.focus_with_options(&options);
            return;
        }
        if !nav::focus_selector(".coll-nav a[aria-current=page]") {
            nav::focus_selector(".coll-nav a");
        }
    }
    #[cfg(not(feature = "csr"))]
    let _ = back;
}

/// The style that puts a menu of `height` where it stands for `at`.
pub(crate) fn menu_style(at: Anchor, height: f64) -> String {
    menu_style_wide(at, MENU_WIDTH, height)
}

/// The same for a menu `wide` px wide.
pub(crate) fn menu_style_wide(at: Anchor, wide: f64, height: f64) -> String {
    let (left, top) = menu_place(at, wide, height, nav::room());
    format!("--x:{left:.0}px;--y:{top:.0}px;--w:{wide:.0}px")
}

/// Moves the focus among the entries of the menu open: on from the one that has it, round.
pub(crate) fn step_focus(by: i32) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let Some(document) = web_sys::window().and_then(|window| window.document()) else { return };
        // The entries and the switches among them („Notizen mitnehmen").
        let Ok(list) = document.query_selector_all(".view-menu :is([role=menuitem], [role=menuitemcheckbox])") else { return };
        let items: Vec<web_sys::HtmlElement> = (0..list.length()).filter_map(|i| list.item(i)).filter_map(|node| node.dyn_into::<web_sys::HtmlElement>().ok()).collect();
        if items.is_empty() {
            return;
        }
        let active = document.active_element();
        let at = items.iter().position(|item| active.as_ref().is_some_and(|active| active.is_same_node(Some(item))));
        let count = items.len() as i32;
        let to = at.map_or(if by > 0 { 0 } else { count - 1 }, |at| (at as i32 + by).rem_euclid(count));
        if let Some(item) = usize::try_from(to).ok().and_then(|to| items.get(to)) {
            let _ = item.focus();
        }
    }
    #[cfg(not(feature = "csr"))]
    let _ = by;
}

/// The marked modules among `data` as text, for a note or a message: number, title, credits (in
/// the page's language), one a line.
fn as_text(data: Option<&BookmarksData>, keep: impl Fn(&str) -> bool, t: &'static Texts) -> String {
    let Some(data) = data else { return String::new() };
    data.rows.iter().filter(|row| keep(&row.id)).map(|row| module_line(&row.id, &row.title, row.credits, t)).collect::<Vec<_>>().join("\n")
}

/// A module as a line of a message (konzept.html, „Weitergeben": „Als Text für eine Nachricht:
/// Nummer, Titel, LP und der Link zur Seite jedes Moduls, damit die Freundin nur tippen muss"):
/// „11103 Analysis I · 8 LP · https://…/catalog/module/11103".
pub(crate) fn module_line(id: &str, title: &str, credits: Option<f64>, t: &'static Texts) -> String {
    let link = absolute(&t.path(&url::module_path(id)));
    match credits {
        Some(credits) => format!("{id} {title} · {} · {link}", format::credits(Some(credits), t.locale)),
        None => format!("{id} {title} · {link}"),
    }
}

/// The notes of `ids` that have one, as a link carries them.
pub(crate) fn notes_of(shelf: &Shelf, ids: &[String]) -> Vec<(String, String)> {
    ids.iter().filter_map(|id| shelf.note(id).map(|note| (id.clone(), note.to_string()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: &str) -> SemesterKey {
        SemesterKey::parse(code).unwrap()
    }

    #[test]
    fn what_mein_studium_says_of_a_marked_module() {
        let mut doc = PlanDoc::default();
        let (now, before, later) = (key("2026W"), key("2025W"), key("2027S"));
        assert_eq!(Status::of(&doc, "11103", Some(now)), None);
        assert_eq!(stand_of(None), Stand::Open);
        assert!(doc.plan(later, "11103", 0, None));
        assert_eq!(Status::of(&doc, "11103", Some(now)), Some(Status::Planned(later)));
        assert!(doc.plan(now, "11103", 0, None));
        // Planned twice: the semester under way counts.
        assert_eq!(Status::of(&doc, "11103", Some(now)), Some(Status::Running(now)));
        assert!(doc.plan(before, "12104", 0, None));
        // Planned in a semester that is over and not passed: open again, as „Stand" sees it.
        assert_eq!(Status::of(&doc, "12104", Some(now)), Some(Status::Failed(before)));
        assert_eq!(Status::Failed(before).stand(), Stand::Open);
        assert!(doc.pass(before, "12104"));
        assert_eq!(Status::of(&doc, "12104", Some(now)), Some(Status::Passed(before)));
        assert_eq!(Status::Passed(before).stand(), Stand::Passed);
        // Without the current semester known, the last semester it is planned in.
        assert_eq!(Status::of(&doc, "11103", None), Some(Status::Planned(later)));
    }

    /// Under what opened it, its right edge at that one's; on a wide screen never under the rail
    /// (owner, 2026-10-20: a collection's ⋯ in a narrow sidebar put its menu behind it).
    #[test]
    fn a_menu_stays_out_from_under_the_rail() {
        let dots = Anchor { left: 268.0, top: 200.0, right: 296.0, bottom: 228.0 };
        let window = nav::Room::window(1100.0, 800.0);
        assert_eq!(menu_place(dots, MENU_WIDTH, 160.0, window), (48.0, 232.0));
        assert_eq!(menu_place(dots, MENU_WIDTH, 160.0, nav::Room { left: 52.0, ..window }), (60.0, 232.0));
        // Room enough beside the rail: right under the ⋯ as before.
        let wide = Anchor { left: 400.0, right: 428.0, ..dots };
        assert_eq!(menu_place(wide, MENU_WIDTH, 160.0, nav::Room { left: 52.0, ..window }).0, 180.0);
        // Above it at the window's bottom.
        let low = Anchor { top: 740.0, bottom: 768.0, ..wide };
        assert_eq!(menu_place(low, MENU_WIDTH, 160.0, nav::Room { left: 52.0, ..window }), (180.0, 576.0));
    }

    #[test]
    fn the_sections_of_the_time_of_marking() {
        // Wednesday, 2026-10-21.
        let today = Day::from_ymd(2026, 10, 21).unwrap();
        let day = |y, m, d| Some(Day::from_ymd(y, m, d).unwrap());
        assert_eq!(When::of(day(2026, 10, 21), today), When::Today);
        assert_eq!(When::of(day(2026, 10, 20), today), When::Yesterday);
        assert_eq!(When::of(day(2026, 10, 19), today), When::ThisWeek);
        assert_eq!(When::of(day(2026, 10, 12), today), When::LastWeek);
        assert_eq!(When::of(day(2026, 10, 11), today), When::Month(Reverse((2026, 10))));
        assert_eq!(When::of(day(2025, 9, 30), today), When::Month(Reverse((2025, 9))));
        assert_eq!(When::of(None, today), When::Unknown);
        // A clock a little ahead is still today.
        assert_eq!(When::of(day(2026, 10, 22), today), When::Today);
        // The newest first.
        let mut order = vec![When::Unknown, When::Month(Reverse((2025, 9))), When::LastWeek, When::Month(Reverse((2026, 10))), When::Today];
        order.sort();
        assert_eq!(order, [When::Today, When::LastWeek, When::Month(Reverse((2026, 10))), When::Month(Reverse((2025, 9))), When::Unknown]);
        // On a Monday yesterday was last week, and is „Gestern" all the same.
        let monday = Day::from_ymd(2026, 10, 19).unwrap();
        assert_eq!(When::of(day(2026, 10, 18), monday), When::Yesterday);
        assert_eq!(When::of(day(2026, 10, 17), monday), When::LastWeek);
        let t = i18n::texts(Locale::De);
        assert_eq!(When::Month(Reverse((2026, 9))).title(today, t), "September");
        assert_eq!(When::Month(Reverse((2025, 12))).title(today, t), "Dezember 2025");
        assert_eq!(When::Unknown.title(today, t), "Früher");
    }

    fn row(id: &str, season: Option<TurnusSeason>, fues: bool) -> CatalogRow {
        CatalogRow {
            id: id.to_string(),
            title: format!("Modul {id}"),
            title_de: None,
            title_en: None,
            credits: Some(6.0),
            turnus_season: season.map(Code::Known),
            turnus_parity: None,
            offer_status: Code::parse("active"),
            teaches_german: Some(true),
            teaches_english: None,
            is_fues: fues,
            is_limited: None,
            department: None,
            teaching_events: 0,
            exam_form: None,
            responsible: None,
            kind: None,
            kind_source: None,
            kind_basis: None,
            plan_semester: None,
            area: None,
            courses: 0,
            choices: Vec::new(),
            marks: None,
        }
    }

    #[test]
    fn the_list_falls_into_sections_and_its_filters_count() {
        let t = i18n::texts(Locale::De);
        let rows = vec![
            row("1", Some(TurnusSeason::Winter), false),
            row("2", Some(TurnusSeason::Summer), false),
            row("3", Some(TurnusSeason::Both), true),
            row("4", None, false),
        ];
        let data = BookmarksData { rows, winter: vec!["1".into(), "3".into()], summer: vec!["2".into(), "3".into()], missing: vec!["99999".into()] };
        let place = |id: &str, kind: ModuleKind, area: &str, order: i64| (id.to_string(), Place { id: id.to_string(), kind: Some(Code::Known(kind)), area: area.to_string(), order });
        let places: BTreeMap<String, Place> = [place("1", ModuleKind::Elective, "Praktische Mathematik", 6), place("2", ModuleKind::Compulsory, "", 2)].into_iter().collect();
        let statuses: BTreeMap<String, Status> = [("2".to_string(), Status::Passed(key("2025W")))].into_iter().collect();
        let marked_at = BTreeMap::new();
        let known = Known { statuses: &statuses, places: Some(&places), marked_at: &marked_at, marked: None, shelf: None };
        let today = Day::from_ymd(2026, 10, 21).unwrap();
        let titles = |listing: &Listing| listing.parts.iter().map(|part| (part.title.clone().unwrap_or_default(), part.rows.iter().map(|row| row.id.clone()).collect::<Vec<_>>())).collect::<Vec<_>>();
        let strings = |items: &[(&str, &[&str])]| items.iter().map(|(title, ids)| (title.to_string(), ids.iter().map(|id| id.to_string()).collect::<Vec<_>>())).collect::<Vec<_>>();

        // „Nach Studium": compulsory, the electives by area, FÜS (the general list), the rest.
        let study = listing_of(&data, &BookmarksUrl { sort: BookmarkSort::Study, ..Default::default() }, BookmarkSort::Study, "", &known, today, t);
        assert_eq!(titles(&study), strings(&[("Pflicht", &["2"]), ("Wahlpflicht · Praktische Mathematik", &["1"]), ("FÜS", &["3"]), ("Gehört nicht zu deinem Studiengang", &["4"])]));
        assert_eq!(study.missing, ["99999"], "nothing filters the list: what the snapshot does not know is named");
        // The other way round: the sections turned, the rows in each as they were.
        let back = listing_of(&data, &BookmarksUrl { sort: BookmarkSort::Study, descending: true, ..Default::default() }, BookmarkSort::Study, "", &known, today, t);
        assert_eq!(titles(&back).first().map(|(title, _)| title.as_str()), Some("Gehört nicht zu deinem Studiengang"));
        // „Nach Angebot".
        let offer = listing_of(&data, &BookmarksUrl { sort: BookmarkSort::Offer, ..Default::default() }, BookmarkSort::Offer, "", &known, today, t);
        assert_eq!(titles(&offer), strings(&[("Winter", &["1"]), ("Sommer", &["2"]), ("Jedes Semester", &["3"]), ("Ohne Angabe", &["4"])]));
        // A search: one part, and nothing of what the snapshot does not know.
        let found = listing_of(&data, &BookmarksUrl::default(), BookmarkSort::Added, "Analysis", &known, today, t);
        assert_eq!((found.parts.len(), found.parts.first().and_then(|part| part.title.clone()), found.missing.len()), (1, None, 0));
        // Marks without a time: „Früher".
        let added = listing_of(&data, &BookmarksUrl::default(), BookmarkSort::Added, "", &known, today, t);
        assert_eq!(titles(&added), strings(&[("Früher", &["1", "2", "3", "4"])]));

        // The page's own filters, and the numbers of their choices under the others.
        let winter = BookmarksUrl { season: Some(Season::Winter), ..Default::default() };
        assert_eq!(shown_of(&data, &winter, false, &known), 2);
        let passed = BookmarksUrl { stand: Some(Stand::Passed), ..Default::default() };
        assert_eq!(shown_of(&data, &passed, false, &known), 1);
        let counting = BookmarksUrl { counts: true, ..Default::default() };
        assert_eq!(shown_of(&data, &counting, false, &known), 3, "the program's two and the general FÜS list");
        let numbers = numbers_of(&data, &winter, &known, 5);
        assert_eq!((numbers.seasons_all, numbers.winter, numbers.summer, numbers.seasons_apart), (4, 2, 2, true));
        assert_eq!((numbers.open, numbers.planned, numbers.passed, numbers.counting), (2, 0, 0, 2), "under the winter's filter");
        assert!(numbers.any_status);
        // Without the program known, „Zählt" cannot tell, and keeps everything.
        let unknown = Known { places: None, ..known };
        assert_eq!(shown_of(&data, &counting, false, &unknown), 4);
        // A module unmarked on the page is still listed, but counted no more.
        let still: BTreeSet<String> = ["1", "2", "4", "99999"].map(String::from).into_iter().collect();
        let counted = Known { marked: Some(&still), ..unknown };
        assert_eq!((shown_of(&data, &BookmarksUrl::default(), false, &counted), numbers_of(&data, &BookmarksUrl::default(), &counted, 5).seasons_all), (4, 3));
        // All offered alike: no choice of the half of the year.
        let alike = BookmarksData { winter: vec!["1".into(), "2".into(), "3".into(), "4".into()], summer: Vec::new(), ..data.clone() };
        assert!(!numbers_of(&alike, &BookmarksUrl::default(), &known, 5).seasons_apart);
        assert_eq!(listing_of(&data, &BookmarksUrl::default(), BookmarkSort::Title, "", &counted, today, t).count(), 5);
    }

    #[test]
    fn the_tags_name_each_filter_and_take_it_away() {
        let t = i18n::texts(Locale::De);
        let url = BookmarksUrl::parse("turnus=winter&stand=planned&counts=1&lang=en&ects_min=5&ects_max=10&campus=senftenberg&graded=yes&sort=ects");
        let tags = tags(&url, true, t);
        let named: Vec<(String, String)> = tags.iter().map(|(group, value, _)| (group.to_string(), value.clone())).collect();
        assert_eq!(
            named,
            [("Turnus", "Winter"), ("Stand", "Geplant"), ("", "Zählt in meinem Studium"), ("Sprache", "Englisch"), ("LP", "5–10"), ("Standort", "Senftenberg"), ("Benotung", "benotet")]
                .map(|(group, value)| (group.to_string(), value.to_string()))
        );
        // Each tag leads to the list without its filter, keeping the rest and the order.
        assert!(tags.iter().all(|(_, _, without)| without.filters_set() == url.filters_set() - 1 && without.sort == BookmarkSort::Credits));
        // Without Mein Studiengang „Zählt" is no tag: it filters nothing then.
        assert!(!self::tags(&url, false, t).iter().any(|(_, value, _)| value == "Zählt in meinem Studium"));
    }
}
