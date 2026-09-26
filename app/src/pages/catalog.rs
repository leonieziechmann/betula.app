//! The module catalog: filters, list and (when a module is open) its preview.
//!
//! The URL is the whole state: `/catalog?…` is the list, `…&open=<id>` the same list with that
//! module previewed next to it (its own page is `/catalog/module/<id>`). The filter controls are
//! links to the list they lead to, so the page works without JavaScript; the browser app adds
//! what links cannot do (pickers with a search, the credit slider).
//!
//! „Passt in meinen Stundenplan" (`fits=<semester>`) is a filter like „Gemerkt": the address says
//! only that it is on and against which semester, and the browser app works out from the
//! Studienplan it keeps which modules fit (`with_fits`); the server's page, which knows no plan,
//! lists none. What it compares is kept in the browser for the next time it is switched on
//! (`finder_on`).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use catalog::filter::{CatalogQuery, ExamPart, FitIds, FitsFilter, KindFilter, Language, PlanSemesterFilter, ProgramRelation, ProgramScope, SortKey};
use catalog::labels::{Campus, Code, Labelled, ModuleKind, OfferStatus, TeachingForm, TurnusParity, TurnusSeason};
use catalog::pages::{self, CatalogArea, CatalogChoices, CatalogData, CatalogSummary, FitResult};
use catalog::plan::SemesterPlan;
use catalog::queries;
use catalog::rows::{CatalogRow, Department, Program};
use catalog::studyplan::PlanDoc;
use catalog::timetable::fit::{CandidateSet, UNKNOWN_NOTE};
use catalog::timetable::select::Selection;
use catalog::timetable::semester::SemesterKey;
use catalog::url::{self, CatalogUrl, ProgramTab, PAGE_SIZE};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;

use crate::bookmarks::{Bookmarks, MarkButton, MarkLook};
use crate::combobox::{ClosePopups, ComboItem, Combobox};
use crate::data::{use_source, DataError, PageStatus, Source};
use crate::format;
use crate::myprogram::{MineResolved, MyProgram};
use crate::nav;
use crate::pages::module::ModulePanel;
use crate::pending::{Change, Pending};
use crate::seo::Seo;
use crate::skeleton::{DetailSkeleton, RowsSkeleton};
use crate::studyplan::{PlanHint, Studyplan};
use crate::tabs::{self, Tabs};
use crate::ui::{ErrorState, Hit, Icon, KindBadge, OfferBadge};

#[component]
pub fn CatalogPage() -> impl IntoView {
    let location = use_location();
    // The server's page lays nothing beside itself: `open` (the preview) is the app's, so the
    // server renders the list the address names as if it were not there, every row leading to
    // the module's own page. The app turns such an address into the preview. The placeholder a
    // list looks for (`fill`) is the app's too: only its plan button reads it. The server caches
    // the page under the address without it, so the page must be that address's page, canonical
    // address and robots line included.
    let url = Memo::new(move |_| {
        let mut url = CatalogUrl::parse(&location.search.get());
        if !APP {
            url.open = None;
            url.fill = None;
        }
        url
    });
    // Independent parts of the URL. The filter decides what the list is; `page` only says where
    // the visitor is in it (the list scrolls endlessly and keeps `page` up to date); `open` is the
    // preview. So scrolling and opening a preview re-render neither list nor filters. `fill` is
    // the placeholder a module found here would fill, kept by every link of the list.
    let list_query = Memo::new(move |_| url.get().query);
    let bookmarks = Bookmarks::expect();
    let source = use_source();
    // What the finder switch makes of the filter: the modules that fit the Studienplan, worked
    // out here from the plan this browser keeps (the server's page has none, R9). Then the marks.
    // `asked` reads `fitted` alone, never `list_query` with it (R16); the notes of the rows are
    // a sibling of it, so a changed note reaches its row even when the list stays the same.
    let (plan, mine) = (Studyplan::expect().filter(|_| APP), MyProgram::expect());
    let fit_source = source.clone().ok();
    let fitted = Memo::new(move |_| with_fits(list_query.get(), plan, mine, fit_source.as_ref()));
    let asked = Memo::new(move |_| fitted.with(|fitted| fitted.failed.clone().map_or_else(|| Ok(with_marks(fitted.query.clone(), bookmarks)), Err)));
    let fit_view = Memo::new(move |_| fitted.with(|fitted| fitted.view.clone()));
    let page = Memo::new(move |_| url.get().page);
    let open = Memo::new(move |_| url.get().open);
    let fill = Memo::new(move |_| url.get().fill);
    // What „Einplanen" aims at, in the preview and on the module's page a phone opens: the
    // semester the finder checks against, and the placeholder a module found here fills
    // (`studyplan::target_semester`).
    let hint = Memo::new(move |_| {
        let here = url.get();
        let semester = here.query.fits.as_ref().and_then(|fits| SemesterKey::parse(&fits.semester));
        (semester.is_some() || here.fill.is_some()).then_some(PlanHint { semester, fill: here.fill })
    });
    if APP {
        provide_context(Finder { view: fit_view, hint });
    }
    let status = PageStatus::capture();
    let phone = phone_layout();

    let list_source = source.clone();
    let list = Memo::new(move |_| {
        // Start at the page the URL names at this moment; later page changes are scrolling. The
        // list is the base of its rows, pager, sort and tag links and of the address scrolling
        // writes, so it carries `fill`, tracked rather than read once like `page`: a row whose
        // link named another placeholder than the address would plan into the wrong one.
        let query = asked.get()?;
        let current = CatalogUrl { query, page: page.get_untracked(), open: None, fill: fill.get() };
        list_source.clone().and_then(|source| source.run(|db| pages::catalog(db, &current))).map(|data| (current, data))
    });
    // The filter panel is rendered once and follows these; only the list is rendered per filter.
    let failed = Memo::new(move |_| list.with(|list| list.as_ref().err().cloned()));
    let facts = Memo::new(move |_| list.with(|list| list.as_ref().map(|(_, data)| Facts::of(data)).unwrap_or_default()));
    // What the pickers offer does not depend on the filter: loaded once, not with every list, and
    // where the host has it ready for the snapshot (`PickerChoices`), not even that.
    let ready = use_context::<PickerChoices>();
    let choices_source = source.clone();
    let choices = Memo::new(move |_| {
        if let Some(ready) = &ready {
            return ready.0.clone();
        }
        let loaded = choices_source.clone().and_then(|source| source.run(pages::catalog_choices));
        Arc::new(loaded.map(|choices| Choices::of(&choices)).unwrap_or_default())
    });

    // On a phone the filter panel is a sheet over the list, and what is picked there is a
    // draft: the sheet shows it at once, with the number of modules it holds, and the list
    // follows once, when the sheet goes. Rebuilding the list behind the sheet with every tap kept
    // a phone busy for a third of a second each time (owner, 2026-09-22: „lagt ganz schön").
    // `None`: the panel shows the filter of the URL.
    let draft: RwSignal<Option<CatalogQuery>> = RwSignal::new(None);
    // A filter on its way to the router (`pending`) is what the panel shows at once: the toggle
    // is flipped in the frame after the click, the list follows in the next.
    let going = Pending::expect();
    let going_to = Memo::new(move |_| going.and_then(|p| p.search_on(url::CATALOG)).map(|search| CatalogUrl::parse(&search)));
    let going_query = Memo::new(move |_| going_to.with(|to| to.as_ref().map(|to| to.query.clone())));
    let panel_query = Memo::new(move |_| draft.get().or_else(|| going_query.get()).unwrap_or_else(|| list_query.get()));
    // The draft the count of the sheet is for. With the finder on, what fits takes a moment to
    // work out the first time (every module of the semester is built once), so the count follows
    // the tap a frame later and the tapped chip answers first (R21); otherwise at once.
    let counted: RwSignal<Option<CatalogQuery>> = RwSignal::new(None);
    Effect::new(move |_| {
        let next = draft.get();
        if next.as_ref().is_some_and(|query| query.fits.is_some()) {
            nav::after_paint(move || {
                if draft.try_with_untracked(|now| *now == next).unwrap_or(false) {
                    counted.try_set(next);
                }
            });
        } else {
            counted.set(next);
        }
    });
    let summary_source = source.clone();
    let draft_facts = Memo::new(move |_| {
        let fitted = with_fits(counted.get()?, plan, mine, summary_source.as_ref().ok());
        let query = with_marks(fitted.query, bookmarks);
        summary_source.clone().and_then(|source| source.run(|db| pages::catalog_summary(db, &query))).ok().map(|summary| Facts::of_summary(&summary))
    });
    let panel_facts = Memo::new(move |_| draft_facts.get().unwrap_or_else(|| facts.get()));
    // A filter from elsewhere (the draft applied, a tag taken away above the list, Back) is what
    // the panel shows from then on.
    Effect::new(move |_| {
        list_query.track();
        if draft.with_untracked(Option::is_some) {
            draft.set(None);
        }
    });
    let apply = move || {
        let Some(query) = draft.get_untracked() else { return };
        if query == list_query.get_untracked() {
            draft.set(None);
            return;
        }
        // A moment later: the sheet has begun to slide away by then, and the browser keeps that
        // going while the list is built. (Not animation frames: a hidden tab has none, and the
        // list would never follow.)
        let path = CatalogUrl { query, page: 1, open: open.get_untracked(), fill: fill.get_untracked() }.path();
        set_timeout(
            move || {
                if let Some(going) = going {
                    going.go(&path, NavigateOptions { scroll: false, ..Default::default() });
                }
            },
            std::time::Duration::from_millis(50),
        );
    };
    // The sheet went (`enhance.js`: its buttons, a swipe down, a tap beside it, Esc, Back, which
    // takes the sheet's own step in the history): the list follows the draft. Where Back left
    // the page with the sheet open, the draft goes with it.
    Effect::new(move |_| {
        let closed = window_event_listener_untyped("betula:sheet-closed", move |_| apply());
        let left = window_event_listener_untyped("betula:sheet-left", move |_| draft.set(None));
        on_cleanup(move || {
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

    // On a phone a module opens as its own page, never as a preview (the preview would fill the
    // screen anyway, and the page has a history entry of its own to come back from). It takes
    // along what „Einplanen" aims at (`?plan=…&fill=…`), as the preview's „Vollbild" does.
    let navigate = use_navigate();
    Effect::new(move |_| {
        if let (true, Some(id)) = (phone.get(), open.get()) {
            let hint = hint.get_untracked().map(|hint| hint.query()).unwrap_or_default();
            navigate(&format!("{}{hint}", url::module_path(&id)), NavigateOptions { replace: true, ..Default::default() });
        }
    });
    // Coming back from a module's page, the list shows the row the visitor left it at: the
    // previewed module, or (a phone has no preview) the module whose page was open just before.
    // Only the first list of this visit does that; a filter change starts at the top as always.
    let now = tabs::location_of(&location.pathname.get_untracked(), &location.search.get_untracked());
    let left_at = Tabs::expect().and_then(|tabs| tabs::page_below(&tabs.before(&now), "/catalog/module"));
    let come_back_to = StoredValue::new(open.get_untracked().or(left_at));
    // The first list of this visit takes the page as it is; a list rendered after it replaces the
    // one before, whose panel (the same element) still stands where the visitor left it.
    let first_list = StoredValue::new(true);
    // The filter of the list before, as the address has it, and the row at the top of the screen
    // there: a list of the same filter that holds other modules (a module planned while the
    // finder is on, a mark taken away under „Gemerkt") keeps that row where it was instead of
    // starting at the top.
    let last_filter: StoredValue<Option<CatalogQuery>> = StoredValue::new(None);
    let top_row: StoredValue<Option<Anchor>> = StoredValue::new(None);
    let preview = Memo::new(move |_| match open.get() {
        None => Ok(None),
        Some(id) => source.clone().and_then(|source| source.run(|db| pages::module(db, &id))).map(Some),
    });
    // The module being opened or closed beside the list (`Some` while that is on its way): its row
    // is marked at once, and a closed preview is gone at once.
    let going_open = Memo::new(move |_| going.filter(|p| p.change() == Some(Change::Preview)).and_then(|_| going_to.with(|to| to.as_ref().map(|to| to.open.clone()))));
    let marked = Memo::new(move |_| going_open.get().unwrap_or_else(|| open.get()));

    // The server's page names its page of the list (a page of the site of its own, below); the
    // app's list scrolls through all of them and keeps the title it started with.
    let page_of = move |title: String| match list.with(|list| list.as_ref().ok().map(|(current, _)| current.page)) {
        Some(page) if page > 1 && !APP => format!("{title}, Seite {page}"),
        _ => title,
    };
    let title = move || {
        page_of(match list.get() {
            Ok((_, data)) => match &data.program {
                Some(p) => format!("Module · {} {}", p.name, p.degree()),
                None => "Modulkatalog: alle Module der BTU Cottbus-Senftenberg".to_string(),
            },
            Err(_) => "Modulkatalog".to_string(),
        })
    };

    view! {
        <Title text=title/>
        <div class="work" class:no-detail=move || open.get().is_none()>
            // The unfiltered list is a page of the site on each of its pages, with an address of its
            // own: the pages are how a crawler reaches every module, those that no program's page
            // links included, and a page that is not listed is one whose links a search engine
            // stops following in the end. Filters, the preview and a page past the end are views
            // (their links are followed, they are not listed).
            {move || {
                let here = url.get();
                let pages = list.with(|list| list.as_ref().ok().map(|(_, data)| data.page.total.div_ceil(PAGE_SIZE).max(1)));
                let listed = here.with_page(1) == CatalogUrl::default() && pages.is_some_and(|pages| here.page <= pages);
                let description = "Alle Module der BTU Cottbus-Senftenberg durchsuchen und filtern: nach Studiengang, Turnus, Lehrform, Prüfungsform, Sprache, Campus, Leistungspunkten und Dozierenden.";
                let (title, description) = match (here.page, pages) {
                    (page, Some(pages)) if page > 1 => (
                        format!("Modulkatalog der BTU Cottbus-Senftenberg, Seite {page}"),
                        format!("Seite {page} von {pages} des Modulkatalogs. {description}"),
                    ),
                    _ => ("Modulkatalog der BTU Cottbus-Senftenberg".to_string(), description.to_string()),
                };
                view! {
                    <Seo
                        title=title
                        description=description
                        path=here.path()
                        noindex=!listed
                    />
                }
            }}
            {move || match failed.get() {
                Some(error) => {
                    status.for_error(&error);
                    view! { <div class="page"><ErrorState error/></div> }.into_any()
                }
                None => view! {
                    <Filters query=panel_query facts=panel_facts choices open fill draft phone/>
                    // The handle for the panel's width sits in the gap between the two boxes.
                    <div class="resizer between js-only" data-action="resize-filters" role="separator" aria-orientation="vertical" aria-controls="filters" aria-label="Breite der Filter ändern (Pfeiltasten, Doppelklick setzt zurück)" tabindex="0"></div>
                    {move || list.get().ok().map(|(current, data)| {
                        let reveal = come_back_to.try_update_value(Option::take).flatten();
                        let fresh = first_list.try_update_value(|first| std::mem::replace(first, false)).unwrap_or(false);
                        let filter = addressed(&current.query);
                        let same = last_filter.try_update_value(|last| last.replace(filter.clone()) == Some(filter)).unwrap_or(false);
                        let stay = (same && !fresh).then(|| top_row.get_value()).flatten();
                        view! { <List current data open marked page phone reveal fresh stay top_row/> }
                    })}
                }.into_any(),
            }}
            {move || {
                match going_open.get() {
                    Some(None) => return ().into_any(),
                    Some(Some(_)) if going.is_some_and(|p| p.waits(Change::Preview)) => return view! { <DetailSkeleton calm=open.get_untracked().is_some()/> }.into_any(),
                    _ => {}
                }
                let close_href = url.get().with_open(None).path();
                match preview.get() {
                    Ok(None) | Err(_) => ().into_any(),
                    Ok(Some(Some(data))) => view! {
                        <ModulePanel data close_href hint/>
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

/// Whether the phone layout is in use, kept up to date while the window changes its size. A list
/// needs it because its rows lead to the module's page there and to a preview elsewhere.
pub(crate) fn phone_layout() -> RwSignal<bool> {
    let phone = RwSignal::new(nav::is_phone());
    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::resize, move |_| {
            if phone.get_untracked() != nav::is_phone() {
                phone.set(nav::is_phone());
            }
        });
        on_cleanup(move || handle.remove());
    });
    phone
}

/// „Gemerkt" is a filter like any other, but what is marked lives in the browser: the URL says
/// only whether the filter is on, and the query gets the ids here, where they never reach a link
/// or the server (R20). Marking a module while the filter is on changes the list, and only then.
fn with_marks(mut query: CatalogQuery, bookmarks: Option<Bookmarks>) -> CatalogQuery {
    if let (Some(marked), Some(bookmarks)) = (query.marked, bookmarks) {
        let ids: Vec<String> = bookmarks.marks().into_iter().map(|mark| mark.id).collect();
        if marked {
            query.only_ids = Some(ids);
        } else {
            query.without_ids = ids;
        }
    }
    query
}

/// The part of a query its address holds: without the ids the browser fills in (the marks and
/// what fits the plan).
fn addressed(query: &CatalogQuery) -> CatalogQuery {
    CatalogQuery { only_ids: None, without_ids: Vec::new(), fits_ids: None, ..query.clone() }
}

/// What „Passt in meinen Stundenplan" makes of a query (A.7): the query with the modules the
/// browser worked out from the plan (`fits_ids`), and what the list says about them.
#[derive(Clone, Debug, Default, PartialEq)]
struct Fitted {
    query: CatalogQuery,
    view: FitView,
    /// The local catalog could not answer: the list says so instead of listing nothing.
    failed: Option<DataError>,
}

/// What the finder says beside the list: a small note at a row that fits only in part or could
/// not be checked („Übung 1 von 3 frei", „keine festen Termine"), and a line under the tags when
/// the semester has no dates yet.
#[derive(Clone, Debug, Default, PartialEq)]
struct FitView {
    /// By module id, as the finder words them.
    notes: BTreeMap<String, String>,
    /// The modules whose note says why they could not be checked (`FitResult::unknown`): a quiet
    /// note, where a partial fit's warns.
    quiet: BTreeSet<String>,
    /// The modules checked that do not clash.
    fitting: BTreeSet<String>,
    /// „keine Termine im WiSe 2026/27": with „auch ohne Termine", what a listed module that was
    /// not checked says.
    undated_note: Option<String>,
    /// „SoSe 2027: noch keine Termine veröffentlicht.": nothing could be checked.
    line: Option<String>,
}

impl FitView {
    /// The note at a module's row, and whether it is a quiet one (not checked, rather than
    /// fitting only in part). `no_termine`: the row says „noch keine Termine" itself, so a note
    /// that would only say the same again („keine festen Termine", „keine Termine im WiSe
    /// 2026/27") is left out (owner, 2026-09-23: „Viel Redundanz").
    fn note_of(&self, id: &str, no_termine: bool) -> Option<(String, bool)> {
        match self.notes.get(id) {
            Some(note) if no_termine && note.starts_with(UNKNOWN_NOTE) => None,
            Some(note) => Some((note.clone(), self.quiet.contains(id))),
            None => self.undated_note.clone().filter(|_| !no_termine && !self.fitting.contains(id)).map(|note| (note, true)),
        }
    }
}

/// What the catalog's list tells its rows and its head of the Studienplan (`CatalogPage` provides
/// it; other lists of modules have none): the finder's view, and what „Einplanen" aims at, which
/// a row takes along to the module's page on a phone.
#[derive(Clone, Copy)]
struct Finder {
    view: Memo<FitView>,
    hint: Memo<Option<PlanHint>>,
}

/// What the finder keeps between its runs, for the visit (the local catalog does not change
/// during one, so neither do its answers): the part that does not depend on the plan (every
/// module of the semester built once, `CandidateSet`), and the last answer with what it was for.
/// A plan change then re-runs only the check; a filter that leaves the finder alone (another
/// program, a search), and coming back from a module's page, run nothing at all. Not reactive
/// (R16): it only saves work.
#[derive(Default)]
struct FitCache {
    set: Option<CandidateSet>,
    last: Option<(FitAsked, FitResult)>,
}

/// What an answer of the finder was for: the switch, the semester's planned modules, and what
/// the plan hides and has chosen there.
type FitAsked = (FitsFilter, Vec<String>, Selection);

thread_local! {
    static FIT_CACHE: RefCell<FitCache> = RefCell::new(FitCache::default());
}

/// Fills `fits_ids` from the plan when the query has the finder switch on, like `with_marks` the
/// marks, and reads the plan only then (A.7, D.7). Without a plan (the server's page) the ids
/// stay unfilled, and the query then lists nothing.
fn with_fits(query: CatalogQuery, plan: Option<Studyplan>, mine: Option<MyProgram>, source: Option<&Source>) -> Fitted {
    let Some(filter) = query.fits.clone() else { return Fitted { query, ..Fitted::default() } };
    let (Some(plan), Some(source)) = (plan, source) else { return fitted(query, &filter, None) };
    let key = SemesterKey::parse(&filter.semester);
    let planned = key.map(|key| plan.modules_in(key)).unwrap_or_default();
    let town = mine.map(MyProgram::town).unwrap_or_default();
    let selection = key.map(|key| plan.selection(key, town)).unwrap_or_default();
    let asked: FitAsked = (filter.clone(), planned, selection);
    let kept = FIT_CACHE.with(|cache| cache.try_borrow().ok().and_then(|cache| cache.last.as_ref().filter(|(last, _)| *last == asked).map(|(_, result)| result.clone())));
    if let Some(result) = kept {
        return fitted(query, &filter, Some(&result));
    }
    // The candidates are taken out while the finder runs, so nothing is borrowed across it.
    let mut set = FIT_CACHE.with(|cache| cache.try_borrow_mut().ok().and_then(|mut cache| cache.set.take()));
    let answer = source.run(|db| pages::fit(db, &filter, &asked.1, &asked.2, &mut set));
    FIT_CACHE.with(|cache| {
        if let Ok(mut cache) = cache.try_borrow_mut() {
            cache.set = set;
            cache.last = answer.as_ref().ok().map(|result| (asked, result.clone()));
        }
    });
    match answer {
        Ok(result) => fitted(query, &filter, Some(&result)),
        Err(error) => Fitted { failed: Some(error), ..fitted(query, &filter, None) },
    }
}

/// The query and the view of a finder's answer (`None`: no plan to check against). A semester
/// without dates is not checked, so it lists every module but the planned ones; with „auch ohne
/// Termine" every module but the clashing and the planned ones; else the modules checked that fit.
fn fitted(mut query: CatalogQuery, filter: &FitsFilter, result: Option<&FitResult>) -> Fitted {
    let Some(result) = result else {
        query.fits_ids = None;
        return Fitted { query, ..Fitted::default() };
    };
    let label = semester_label(&filter.semester);
    query.fits_ids = Some(if !result.has_data || filter.undated { FitIds::Without(result.excluded.clone()) } else { FitIds::Only(result.fitting.clone()) });
    let view = FitView {
        notes: result.notes.clone(),
        quiet: result.unknown.clone(),
        fitting: result.fitting.iter().cloned().collect(),
        undated_note: (result.has_data && filter.undated).then(|| format!("keine Termine im {label}")),
        line: (!result.has_data).then(|| format!("{label}: noch keine Termine veröffentlicht.")),
    };
    Fitted { query, view, failed: None }
}

/// „WiSe 2026/27" for `2026W`; a key that is none as it stands.
fn semester_label(key: &str) -> String {
    SemesterKey::parse(key).map(SemesterKey::label).unwrap_or_else(|| key.to_string())
}

/// The semester „Passt in meinen Stundenplan" checks against once it is switched on: that of the
/// placeholder the list is looked through for (`fill`), while the plan holds it, else the current
/// one. So the switch turned off and on again checks the semester „Einplanen" then plans into
/// (`studyplan::target_semester`), not another one.
fn finder_semester(doc: &PlanDoc, fill: Option<u32>, current: SemesterKey) -> SemesterKey {
    fill.and_then(|pid| doc.placeholders.iter().find(|placeholder| placeholder.pid == pid)).map_or(current, |placeholder| placeholder.semester)
}

/// Where this browser remembers what „Passt in meinen Stundenplan" compares, for the next time it
/// is switched on (owner, 2026-09-26: the choice below it is kept, not reset with every switch):
/// the part of the address that says it (`fits-skip=exam&fits-undated=1`), nothing while it
/// compares everything. A view setting like the width of the panel (R13): never in server HTML.
const FINDER_KEY: &str = "betula.finder";

/// The finder switched on for `semester` (the catalog's switch, „+ Modul" and „Modul finden" of the
/// Stundenplan): comparing what it compared when this browser had it on last, everything the
/// first time. The server's page knows nothing of it (R9).
pub(crate) fn finder_on(semester: SemesterKey) -> FitsFilter {
    finder_kept(nav::local_get(FINDER_KEY).as_deref(), semester)
}

/// `finder_on` with what is stored. Read as the address is read (what comes from storage is
/// checked like what comes from a URL, R20), and a choice that compares no class at all is none.
fn finder_kept(stored: Option<&str>, semester: SemesterKey) -> FitsFilter {
    let all = FitsFilter::all(&semester.key());
    let Some(stored) = stored.filter(|stored| !stored.is_empty()) else { return all };
    CatalogUrl::parse(&format!("fits={}&{stored}", all.semester)).query.fits.filter(|kept| kept.lectures || kept.exercises || kept.exams).unwrap_or(all)
}

/// What `FINDER_KEY` keeps of the finder: its pairs of the address without the semester, written
/// by the address's own codec. Empty while it compares everything (`nav::local_set` then takes
/// the key out).
fn finder_text(fits: &FitsFilter) -> String {
    let only_finder = CatalogUrl { query: CatalogQuery { fits: Some(fits.clone()), ..CatalogQuery::default() }, ..CatalogUrl::default() };
    only_finder.to_query_string().split('&').filter(|pair| !pair.starts_with("fits=")).collect::<Vec<_>>().join("&")
}

/// What to leave out when nothing fits: the classes compared besides the lectures, which have to
/// be free anyway.
fn fit_advice(filter: &FitsFilter) -> &'static str {
    match (filter.exercises, filter.exams, filter.lectures) {
        (true, true, _) => "Übungen oder Prüfungen abwählen.",
        (true, false, _) => "Übungen abwählen.",
        (false, true, _) => "Prüfungen abwählen.",
        (false, false, true) => "Vorlesungen abwählen.",
        (false, false, false) => "Nimm Filter zurück oder suche nach einem anderen Begriff.",
    }
}

/// A link that keeps whatever preview is open at the time it is followed.
fn keep_open(target: CatalogUrl, open: Memo<Option<String>>) -> impl Fn() -> String + Clone + Send + Sync + 'static {
    move || target.with_open(open.get().as_deref()).path()
}

/// The active filters as removable tags: (group, value, the list without it). `areas` and
/// `departments` name what the URL has as a number.
fn tags(current: &CatalogUrl, areas: &[CatalogArea], departments: &[Department]) -> Vec<(String, String, CatalogUrl)> {
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
        // One tag per area: a row of the plan may have opened the list with several.
        for id in scope.areas.clone() {
            let label = areas.iter().find(|area| area.id == id).map(|area| area.name().to_string()).unwrap_or_else(|| format!("Bereich {id}"));
            push("Bereich", label, &move |q| {
                if let Some(s) = q.program.as_mut() {
                    s.areas.retain(|area| *area != id);
                }
            });
        }
    }
    if let Some(scheduled) = q.scheduled {
        push("Termine", if scheduled { "bestätigt" } else { "noch keine" }.to_string(), &|q| q.scheduled = None);
    }
    if let Some(fits) = &q.fits {
        push("Passt in", semester_label(&fits.semester), &|q| q.fits = None);
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
        let label = departments.iter().find(|d| d.id == id).map(|d| d.label.clone()).unwrap_or_else(|| id.to_string());
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
    if let Some(marked) = q.marked {
        push("Merkliste", if marked { "nur gemerkte" } else { "ohne gemerkte" }.to_string(), &|q| q.marked = None);
    }
    out
}

#[component]
fn List(
    current: CatalogUrl,
    data: CatalogData,
    open: Memo<Option<String>>,
    /// The row that is marked as open: `open`, or the module on its way there.
    marked: Memo<Option<String>>,
    page: Memo<u64>,
    phone: RwSignal<bool>,
    /// The row to scroll to once the list is there.
    reveal: Option<String>,
    /// The first list of the visit (`true`), or one that replaces the list of the filter before.
    fresh: bool,
    /// A list that replaces one of the same filter: the row the list before had at the top.
    stay: Option<Anchor>,
    /// Where the row at the top of the screen is kept for the list that may replace this one.
    top_row: StoredValue<Option<Anchor>>,
) -> impl IntoView {
    let q = current.query.clone();
    let total = data.page.total;
    let pages_total = total.div_ceil(PAGE_SIZE).max(1);
    let start_page = current.page.min(pages_total);
    let with_program = data.program.is_some();
    let unknown_program = q.program.is_some() && !with_program;
    let label = match (&data.program, q.program.as_ref().map(|s| s.relation)) {
        (Some(_), Some(ProgramRelation::Fues)) => "FÜS-Module dieses Studiengangs",
        (Some(_), _) => "Module im Curriculum",
        _ if total == 1 => "Modul",
        _ => "Module",
    };
    // The tags are those of the filter on its way (`pending`) as soon as it is clicked, so that the
    // head of the list has its height before the rows come.
    let going = Pending::expect();
    let going_url = Memo::new(move |_| going.filter(|going| going.change() == Some(Change::List)).and_then(|going| going.search_on(url::CATALOG)).map(|search| CatalogUrl::parse(&search)));
    let active = {
        let (current, areas, departments) = (current.clone(), data.areas.clone(), data.departments.clone());
        Memo::new(move |_| tags(&going_url.get().unwrap_or_else(|| current.clone()), &areas, &departments))
    };
    let active_count = move || active.with(Vec::len);
    let fit_line = use_context::<Finder>();

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

    // What the list says instead of rows.
    let without_fits = {
        let mut next = current.with_page(1);
        next.query.fits = None;
        next.path()
    };
    // An empty list with the finder on is the finder's doing only when the rest of the filter
    // holds modules (a search for nothing is not helped by comparing fewer classes): one count,
    // asked only then.
    let finder_emptied = APP
        && total == 0
        && q.fits.is_some()
        && use_source()
            .and_then(|source| source.run(|db| queries::catalog_count(db, &CatalogQuery { fits: None, fits_ids: None, ..data.effective.clone() })))
            .is_ok_and(|count| count > 0);
    let states = view! {
        {unknown_program.then(|| view! {
            <div class="state"><p class="state-title">"Diesen Studiengang gibt es nicht (mehr)"</p><p>"Wähle links einen anderen Studiengang oder „Alle Studiengänge“."</p></div>
        })}
        {(total == 0 && !unknown_program).then(|| match (q.marked == Some(true), q.fits.as_ref(), APP) {
            // Nothing fits the plan: the classes that could be left out of the comparison.
            (false, Some(fits), true) if finder_emptied => view! {
                <div class="state">
                    <p class="state-title">{format!("Kein Modul passt in deinen Stundenplan für {}", semester_label(&fits.semester))}</p>
                    <p>{fit_advice(fits)}</p>
                </div>
            }.into_any(),
            (false, Some(_), false) => view! {
                <div class="state">
                    <p class="state-title">"Deinen Stundenplan kennt nur dein Browser"</p>
                    <p>"Diese Seite kommt vom Server, und dort liegt nichts von dir. Mit JavaScript zeigt die App hier die Module, die in deinen Stundenplan passen."</p>
                    <a class="btn secondary" href=without_fits.clone()>"Ohne diesen Filter"</a>
                </div>
            }.into_any(),
            (true, _, true) => view! {
                <div class="state">
                    <p class="state-title">"Keine gemerkten Module in dieser Liste"</p>
                    <p>"Kein gemerktes Modul passt zu den übrigen Filtern."</p>
                    <a class="btn secondary" href=url::BOOKMARKS>"Zur Merkliste"</a>
                </div>
            }.into_any(),
            (true, _, false) => view! {
                <div class="state">
                    <p class="state-title">"Deine Merkliste kennt nur dein Browser"</p>
                    <p>"Diese Seite kommt vom Server, und dort liegt nichts von dir. Mit JavaScript zeigt die App hier deine gemerkten Module."</p>
                    <a class="btn secondary" href=url::CATALOG>"Alle Module zeigen"</a>
                </div>
            }.into_any(),
            _ => view! {
                <div class="state"><p class="state-title">"Keine Module gefunden"</p><p>"Nimm Filter zurück oder suche nach einem anderen Begriff."</p><a class="btn secondary" href=url::CATALOG>"Alle Filter zurücksetzen"</a></div>
            }.into_any(),
        })}
    }
    .into_any();

    // What stands above the rows scrolls with them: the note of a semester scrolls away, the heads
    // of the columns stay at the top (owner, 2026-09-23: the note fixed above the list left room
    // for two rows).
    let head = view! {
        {plan_note(data.semester_plan.as_ref(), &current, open)}
        <div class="cols label" id=HEAD_ID>
            {sort_link(if with_program { SortKey::Default } else { SortKey::Title }, "Modul", "")}
            <span class="c-resp">"Verantwortlich"</span>
            <span class="c-exam">"Prüfung"</span>
            {sort_link(SortKey::Credits, "LP", "c-lp")}
            <span class="c-turnus">"Turnus"</span>
            <span class="c-lang">"Spr."</span>
            {sort_link(SortKey::Events, "Termine", "c-events")}
        </div>
    }
    .into_any();

    // The browser app renders only what is on screen of the whole list; the server renders the
    // page the URL names, with pager links (no JavaScript, search engines).
    let rows = if APP {
        view! { <VirtualRows current=current.clone() query=data.effective.clone() first=data.page.rows.clone() total open marked page phone with_program reveal fresh stay top_row head states/> }.into_any()
    } else {
        view! { <PlainRows current=current.clone() rows=data.page.rows.clone() start_page pages_total open phone with_program head states/> }.into_any()
    };

    // Another filter on its way: the rows it replaces stand as a skeleton (`pending`).
    let waiting = move || going.is_some_and(|p| p.waits(Change::List));
    view! {
        <section class="panel list" aria-live="polite" data-pending=move || waiting().then_some("")>
            <div class="list-head">
                <div class="count-row">
                    <span class="count num">{format::count(total)}</span>
                    <span class="count-label">{label}</span>
                    <div class="list-tools">
                        <span class="keys" title="Mit den Pfeiltasten durch die Liste, Enter öffnet die Vorschau, M merkt das gewählte Modul"><kbd>"↑"</kbd><kbd>"↓"</kbd>" wählen "<kbd>"Enter"</kbd>" öffnen "<kbd>"M"</kbd>" merken"</span>
                        <a class="sheet-toggle" href="#filters" data-action="sheet-open">
                            <Icon name="sliders-horizontal"/>"Filter"{move || (active_count() > 0).then(|| view! { <em>{active_count()}</em> })}
                        </a>
                        {data.program.as_ref().map(|p| view! {
                            <a class="ghost" href=url::program_path(&p.slug, ProgramTab::Plan)><Icon name="graduation-cap"/>"Studiengangsseite"</a>
                        })}
                    </div>
                </div>
                <div class="active-filters">
                    {move || active.get().into_iter().map(|(group, value, target)| view! {
                        <span class="tag"><em>{group}</em>" "{value}<a href=keep_open(target, open) aria-label="Filter entfernen"><Icon name="x"/></a></span>
                    }).collect_view()}
                </div>
                // The finder against a semester without dates: nothing could be checked.
                {move || fit_line.and_then(|finder| finder.view.with(|view| view.line.clone())).map(|line| view! { <p class="hint fit-line">{line}</p> })}
            </div>
            {rows}
        </section>
    }
}

/// With a semester of a program chosen: what its plan asks for there besides the modules it
/// places in it, a line for each row — how much, what the plan calls it, and where the list takes
/// the modules for it from: „≥ 6 LP Anwendungsfach: „Mathematik“, … oder „Physik“". A name that
/// only repeats its areas is left out („≥ 6 LP aus dem Bereich: „Praktische Mathematik“" for
/// „Modul aus dem Bereich Praktische Mathematik"; owner, 2026-09-23: „Viel Redundanz"), and so
/// are the areas the name fits less well. Where the modules are taken from is derived from the
/// names, and the note says so; the areas are links to the list narrowed down to them.
fn plan_note(plan: Option<&SemesterPlan>, current: &CatalogUrl, open: Memo<Option<String>>) -> Option<AnyView> {
    let plan = plan.filter(|plan| !plan.requirements.is_empty())?;
    let area_link = |id: i64, label: &str| {
        let mut target = current.with_page(1);
        if let Some(scope) = target.query.program.as_mut() {
            scope.areas = vec![id];
        }
        view! { <a href=keep_open(target, open) data-noscroll="">"„"{label.to_string()}"“"</a> }
    };
    let links = |areas: &[CatalogArea]| {
        areas
            .iter()
            .enumerate()
            .map(|(i, area)| view! {
                {(i > 0).then(|| if i + 1 < areas.len() { ", " } else { " oder " })}
                {area_link(area.id, area.name())}
            })
            .collect_view()
    };
    let fues_list = {
        let mut target = current.with_page(1);
        if let Some(scope) = target.query.program.as_mut() {
            scope.relation = ProgramRelation::Fues;
            scope.areas.clear();
        }
        keep_open(target, open)
    };
    let rows = plan
        .requirements
        .iter()
        .map(|row| {
            // A choice asks for at least that much (a module of the area may have more); one
            // module, or a range, is what it says.
            let credits = row.credits.as_ref().map(|credits| {
                let at_least = if row.single || credits.contains('–') { "" } else { "≥\u{a0}" };
                view! { <b>{format!("{at_least}{credits}\u{a0}LP")}</b>" " }
            });
            let what = if row.single {
                view! { {row.shown_name().to_string()}": unter diesem Namen nicht im Katalog" }.into_any()
            } else if row.fues {
                view! { "Fachübergreifendes Studium: siehe "<a href=fues_list.clone() data-noscroll="">"FÜS-Liste"</a> }.into_any()
            } else if row.areas.is_empty() {
                view! { {row.shown_name().to_string()}": alle Wahlpflichtmodule" }.into_any()
            } else if row.named_by_areas() {
                view! { {if row.ambiguous() { "aus den Bereichen: " } else { "aus dem Bereich: " }}{links(&row.areas)} }.into_any()
            } else {
                view! { {row.shown_name().to_string()}": "{links(&row.areas)} }.into_any()
            };
            view! { <li>{credits}{what}</li> }
        })
        .collect_view();
    // A row that names the FÜS or a single module is what it says; for every other one the list
    // takes what the name points at.
    let derived = plan.requirements.iter().any(|row| !row.single && !row.fues);
    Some(view! {
        <div class="plan-note">
            <Icon name="info"/>
            <div>
                <p class="plan-note-lead">"Der Regelstudienplan sieht im "{plan.semester}". Semester außerdem vor:"</p>
                <ul>{rows}</ul>
                {derived.then(|| view! { <p class="plan-note-hint">"Welche Module gemeint sind, ist aus den Namen im Plan abgeleitet."</p> })}
            </div>
        </div>
    }.into_any())
}

/// One page of rows, as the server renders it: what the URL names, each row leading to the
/// module's own page (nothing stands beside the server's list), and links to the pages before
/// and after it.
#[component]
fn PlainRows(
    current: CatalogUrl,
    rows: Vec<CatalogRow>,
    start_page: u64,
    pages_total: u64,
    open: Memo<Option<String>>,
    phone: RwSignal<bool>,
    with_program: bool,
    /// What stands above the rows and scrolls with them (`List`).
    head: AnyView,
    states: AnyView,
) -> impl IntoView {
    view! {
        <div class="rows scroll" id=ROWS_ID data-keep-scroll="rows">
            {head}
            {states}
            {rows.into_iter().map(|row| {
                let (target, id) = (row.id.clone(), row.id.clone());
                let preview = Signal::derive(move || url::module_path(&target));
                let current = Signal::derive(move || open.get().as_deref() == Some(id.as_str()));
                view! { <Row row preview current phone with_program/> }
            }).collect_view()}
            {(pages_total > 1).then(|| view! {
                <nav class="pager" aria-label="Seiten">
                    {(start_page > 1).then(|| view! { <a class="btn secondary" rel="prev" href=keep_open(current.with_page(start_page - 1), open)>"Zurück"</a> })}
                    <span class="num">"Seite "{start_page}" von "{pages_total}</span>
                    {(start_page < pages_total).then(|| view! { <a class="btn secondary" rel="next" href=keep_open(current.with_page(start_page + 1), open)>"Weiter"</a> })}
                </nav>
            })}
        </div>
    }
}

const ROWS_ID: &str = "rows";
/// The heads of the columns, which stay at the top of the list while its rows scroll under them.
const HEAD_ID: &str = "rows-head";
/// The element that holds the rows of the virtual list, as tall as the whole list.
const VLIST_ID: &str = "rows-virtual";
/// Rows rendered beyond what is visible, above and below: room for the keyboard to move and for
/// a scroll to land before the next rows are there.
const BUFFER: usize = 10;
/// Pages of rows kept on either side of the visitor's place; farther ones are dropped and loaded
/// again when the visitor comes back (a page is one query of a few milliseconds).
const KEEP_PAGES: usize = 4;
/// What a row is taken to be as tall as until it has been measured (the stylesheet's rows).
const ROW_DESKTOP: f32 = 58.0;
const ROW_PHONE: f32 = 88.0;

/// Where the visitor is in the list: the place of the row at the top of the screen, and the
/// modules on screen from the top down (where their page is loaded), each with how far the list
/// is scrolled past the top of its row (below zero for a row further down). A list that replaces
/// this one keeps the first of them it still holds where it stood: the row at the top often
/// leaves the list itself (planned with the finder on, it clashes with the module planned).
#[derive(Clone, Debug, Default, PartialEq)]
struct Anchor {
    index: usize,
    rows: Vec<(String, f32)>,
}

/// The whole list, of which only what is on screen (and a little around it) is rendered: the
/// rows stand at their offsets inside an element as tall as the list, so the scrollbar has the
/// length of the list from the start. Rows are measured once they are rendered and estimated
/// (at the average of the measured ones) until then; when a row above what is visible turns out
/// taller or shorter than estimated, the list scrolls by the difference, so nothing jumps under
/// the visitor's eyes. Pages of rows are loaded when their rows come near and dropped again
/// when they are far. `page` in the URL follows the row at the top of the screen.
#[component]
fn VirtualRows(
    current: CatalogUrl,
    /// The query the page ran: the URL's with what the page derived (`CatalogData::effective`).
    query: CatalogQuery,
    /// The rows of the page the URL names: what the server rendered, loaded already.
    first: Vec<CatalogRow>,
    total: u64,
    open: Memo<Option<String>>,
    marked: Memo<Option<String>>,
    page: Memo<u64>,
    phone: RwSignal<bool>,
    with_program: bool,
    /// The row to scroll to once the list is there: the module the visitor comes back from.
    reveal: Option<String>,
    /// The first list of the visit (`true`), or one that replaces the list of the filter before.
    fresh: bool,
    /// A list that replaces one of the same filter: the row to keep at the top of the screen.
    stay: Option<Anchor>,
    /// Where the row at the top of the screen is kept, for the list that may replace this one.
    top_row: StoredValue<Option<Anchor>>,
    /// What stands above the rows and scrolls with them (`List`).
    head: AnyView,
    states: AnyView,
) -> impl IntoView {
    let total = usize::try_from(total).unwrap_or(0);
    let per_page = usize::try_from(PAGE_SIZE).unwrap_or(50).max(1);
    let pages_total = total.div_ceil(per_page).max(1);
    let start_page = usize::try_from(current.page).unwrap_or(1).clamp(1, pages_total);
    let start_query = query.clone();
    let query = StoredValue::new(query);
    let source = use_source().ok();
    // The pages of rows the list holds, by page number.
    let loaded: RwSignal<BTreeMap<usize, Vec<CatalogRow>>> = RwSignal::new(BTreeMap::from([(start_page, first)]));
    // The height of every row that has been measured, the sum and the number of them (for the
    // estimate of the others), and a counter that changes whenever a height does.
    let heights: StoredValue<Vec<Option<f32>>> = StoredValue::new(vec![None; total]);
    let measured = StoredValue::new((0.0f32, 0usize));
    let layout = RwSignal::new(0u32);
    // The rows that are rendered: `first..end`.
    let window = RwSignal::new((0usize, (BUFFER * 3).min(total)));
    // Frames, timeouts and the size watcher outlive the list when a filter replaces it: what
    // they call must not touch the list's values after that (R2).
    let alive = Arc::new(AtomicBool::new(true));
    on_cleanup({
        let alive = alive.clone();
        move || alive.store(false, Ordering::Relaxed)
    });

    let estimate = move || {
        let (sum, count) = measured.get_value();
        match count {
            0 => if phone.get_untracked() { ROW_PHONE } else { ROW_DESKTOP },
            count => sum / count as f32,
        }
    };
    let offset_of = move |index: usize| -> f32 {
        let estimate = estimate();
        heights.with_value(|heights| heights.iter().take(index).map(|height| height.unwrap_or(estimate)).sum())
    };
    let index_at = move |offset: f32| -> usize {
        let estimate = estimate();
        heights.with_value(|heights| {
            let mut bottom = 0.0;
            for (index, height) in heights.iter().enumerate() {
                bottom += height.unwrap_or(estimate);
                if bottom > offset {
                    return index;
                }
            }
            heights.len().saturating_sub(1)
        })
    };

    // The pages whose rows are in `first..end` are loaded; pages far from there are dropped.
    let ensure_loaded = move |first: usize, end: usize| {
        let Some(source) = source.clone() else { return };
        let (first_page, last_page) = (first / per_page + 1, end.saturating_sub(1) / per_page + 1);
        let missing: Vec<usize> = (first_page..=last_page).filter(|p| loaded.with_untracked(|loaded| !loaded.contains_key(p))).collect();
        let far: Vec<usize> = loaded.with_untracked(|loaded| loaded.keys().copied().filter(|p| *p + KEEP_PAGES < first_page || *p > last_page + KEEP_PAGES).collect());
        if missing.is_empty() && far.is_empty() {
            return;
        }
        let fetched: Vec<(usize, Vec<CatalogRow>)> = missing
            .into_iter()
            .filter_map(|p| {
                let offset = u64::try_from((p - 1) * per_page).ok()?;
                let rows = source.run(|db| catalog::queries::catalog_page(db, &query.get_value(), offset, PAGE_SIZE)).ok()?.rows;
                Some((p, rows))
            })
            .collect();
        loaded.update(|loaded| {
            for p in far {
                loaded.remove(&p);
            }
            for (p, rows) in fetched {
                loaded.insert(p, rows);
            }
        });
    };

    // Where the visitor is: which rows to render, which pages to hold, and what `page` in the
    // URL says (replacing the history entry, so Back still leaves the list in one step).
    let navigate = use_navigate();
    let base = StoredValue::new(current.clone());
    let alive_follow = alive.clone();
    let follow = move || {
        if total == 0 || !alive_follow.load(Ordering::Relaxed) {
            return;
        }
        let Some((offset, viewport)) = nav::list_viewport(ROWS_ID, HEAD_ID, VLIST_ID) else { return };
        let first_visible = index_at(offset);
        let last_visible = index_at(offset + viewport);
        let range = (first_visible.saturating_sub(BUFFER), (last_visible + 1 + BUFFER).min(total));
        ensure_loaded(range.0, range.1);
        let (guess, mut top) = (estimate(), offset_of(first_visible));
        let mut rows = Vec::new();
        for index in first_visible..=last_visible.min(total.saturating_sub(1)) {
            let id = loaded.with_untracked(|loaded| loaded.get(&(index / per_page + 1)).and_then(|rows| rows.get(index % per_page)).map(|row| row.id.clone()));
            rows.extend(id.map(|id| (id, offset - top)));
            top += heights.with_value(|heights| heights.get(index).copied().flatten()).unwrap_or(guess);
        }
        top_row.try_update_value(|anchor| *anchor = Some(Anchor { index: first_visible, rows }));
        if window.get_untracked() != range {
            window.set(range);
        }
        let seen = (first_visible / per_page + 1) as u64;
        if seen != page.get_untracked() {
            let target = base.get_value().with_page(seen).with_open(open.get_untracked().as_deref()).path();
            navigate(&target, NavigateOptions { replace: true, scroll: false, ..Default::default() });
        }
    };

    // The rendered rows are measured. A row that turns out to differ from what it was taken for
    // moves everything below it; where that is above the visible part, the list scrolls by the
    // difference and the visitor sees nothing move. So do the rows above that were never
    // rendered, all of them at once, when the average they are taken for changes with the new
    // measurements: in a list that starts far down (a page of its own, one that replaces a list
    // of the same filter) they are most of what lies above. `true` if any height changed.
    let alive_measure = alive.clone();
    let measure = move || -> bool {
        if !alive_measure.load(Ordering::Relaxed) {
            return false;
        }
        let Some((offset, _)) = nav::list_viewport(ROWS_ID, HEAD_ID, VLIST_ID) else { return false };
        let (guess, first_visible) = (estimate(), index_at(offset));
        // (index, what it was taken for, what it is, whether it lies above the visible part)
        let mut changes: Vec<(usize, f32, f32, bool)> = Vec::new();
        for (index, height) in nav::measure_rows(VLIST_ID) {
            let known = heights.with_value(|heights| heights.get(index).copied().flatten());
            if known.is_some_and(|known| (known - height).abs() < 0.5) {
                continue;
            }
            let was = known.unwrap_or(guess);
            changes.push((index, was, height, offset_of(index) + was <= offset + 0.5));
        }
        if changes.is_empty() {
            return false;
        }
        let mut shift = 0.0;
        for (index, was, now, above) in changes {
            let known = heights.with_value(|heights| heights.get(index).is_some_and(Option::is_some));
            heights.update_value(|heights| {
                if let Some(slot) = heights.get_mut(index) {
                    *slot = Some(now);
                }
            });
            measured.update_value(|(sum, count)| {
                if known {
                    *sum += now - was;
                } else {
                    *sum += now;
                    *count += 1;
                }
            });
            if above {
                shift += now - was;
            }
        }
        let unmeasured = heights.with_value(|heights| heights.iter().take(first_visible).filter(|height| height.is_none()).count());
        shift += unmeasured as f32 * (estimate() - guess);
        if shift.abs() >= 0.5 {
            nav::scroll_list_by(ROWS_ID, shift);
        }
        layout.update(|n| *n = n.wrapping_add(1));
        true
    };

    // Rendered rows are measured once they are there, and again whenever the list changes its
    // width (the filter panel is dragged, the window changes), which changes how titles wrap.
    let (after_render, measure_rendered) = (follow.clone(), measure.clone());
    Effect::new(move |_| {
        window.track();
        loaded.track();
        let (follow, measure) = (after_render.clone(), measure_rendered.clone());
        request_animation_frame(move || {
            if measure() {
                follow();
            }
        });
    });
    let (on_resize, measure_resized) = (follow.clone(), measure.clone());
    Effect::new(move |_| {
        let (follow, measure) = (on_resize.clone(), measure_resized.clone());
        let watch = nav::watch_size(VLIST_ID, move || {
            if measure() {
                follow();
            }
        });
        on_cleanup(move || drop(watch));
    });
    // On a phone the window scrolls, not the panel.
    let follow_window = follow.clone();
    Effect::new(move |_| {
        let follow = follow_window.clone();
        let handle = window_event_listener(leptos::ev::scroll, move |_| follow());
        on_cleanup(move || handle.remove());
    });

    // Where the list starts: at the row the visitor comes back to (in the middle of the screen),
    // else at the page the URL names. Once, when the list is there; and once more a moment later
    // for a way back through the history, where the browser restores a scroll position of its own
    // after this has run.
    let (at_start, alive_start) = (follow.clone(), alive.clone());
    let start_source = use_source().ok();
    Effect::new(move |_| {
        // A list that replaces another (a filter changed) starts at the top: the panel is the
        // element the list before scrolled, and where that stood would be taken for this list's
        // page. Before anything measures: the frame after the render already follows the scroll.
        // One of the same filter (the plan or the marks changed what it holds) keeps the first row
        // on screen that is still in it where it stood, so the visitor stays where they were.
        if !fresh && stay.is_none() {
            nav::scroll_list_to_start(ROWS_ID);
        }
        let position = |id: &str| {
            let source = start_source.clone()?;
            let index = source.run(|db| catalog::queries::catalog_position(db, &start_query, id)).ok().flatten()?;
            Some(usize::try_from(index).unwrap_or(0).min(total.saturating_sub(1)))
        };
        // A row of the page the list starts with, the one the address names: where the top of
        // the screen was. Found there without asking the catalog, which ranks the whole list for
        // each module it is asked about.
        let loaded_at = |id: &str| loaded.with_untracked(|loaded| loaded.get(&start_page).and_then(|rows| rows.iter().position(|row| row.id == id))).map(|at| (start_page - 1) * per_page + at);
        // The row to scroll to, and how far into it.
        let target = match (&stay, reveal.as_deref().and_then(position)) {
            (Some(anchor), _) => anchor
                .rows
                .iter()
                .find_map(|(id, into)| loaded_at(id).map(|index| (index, *into)))
                .or_else(|| anchor.rows.iter().take(3).find_map(|(id, into)| position(id).map(|index| (index, *into))))
                .or_else(|| (total > 0).then(|| (anchor.index.min(total - 1), 0.0))),
            (None, Some(index)) => Some((index, 0.0)),
            (None, None) if start_page > 1 => Some(((start_page - 1) * per_page, 0.0)),
            (None, None) => None,
        };
        let (follow, alive) = (at_start.clone(), alive_start.clone());
        let go = move |center: bool| {
            // The list may have been replaced by then (a filter, a moment after coming back).
            if !alive.load(Ordering::Relaxed) {
                return;
            }
            if let Some((index, into)) = target {
                let viewport = nav::list_viewport(ROWS_ID, HEAD_ID, VLIST_ID).map(|(_, viewport)| viewport).unwrap_or(0.0);
                let offset = if center { offset_of(index) - (viewport - estimate()) / 2.0 } else { offset_of(index) + into };
                nav::scroll_list_to(ROWS_ID, HEAD_ID, VLIST_ID, offset.max(0.0));
            }
            follow();
        };
        let (now, later) = (go.clone(), go);
        let center = reveal.is_some();
        request_animation_frame(move || now(center));
        if center {
            set_timeout(move || later(true), std::time::Duration::from_millis(220));
        }
    });

    let on_scroll = follow.clone();
    let row_at = move |index: usize| loaded.with(|loaded| loaded.get(&(index / per_page + 1)).and_then(|rows| rows.get(index % per_page)).cloned());
    let base_rows = current.clone();
    let going = Pending::expect();
    view! {
        <div class="rows scroll virtual" id=ROWS_ID data-keep-scroll="rows" on:scroll=move |_| on_scroll()>
            {head}
            {move || going.is_some_and(|p| p.waits(Change::List)).then(|| view! { <RowsSkeleton/> })}
            {states}
            <div class="vlist" id=VLIST_ID style=move || { layout.track(); format!("--h:{:.0}px", offset_of(total)) }>
                <For each=move || { let (first, end) = window.get(); first..end } key=|index| *index children=move |index: usize| {
                    let base = base_rows.clone();
                    let top = move || {
                        layout.track();
                        format!("--top:{:.0}px", offset_of(index))
                    };
                    view! {
                        {move || row_at(index).map(|row| {
                            let (base, target, id) = (base.clone(), row.id.clone(), row.id.clone());
                            let preview = Signal::derive(move || base.with_page(page.get()).with_open(Some(&target)).path());
                            let current = Signal::derive(move || marked.get().as_deref() == Some(id.as_str()));
                            view! { <div class="vrow" data-i=index style=top><Row row preview current phone with_program/></div> }
                        })}
                    }
                }/>
            </div>
            {(total > per_page).then(|| view! { <p class="list-end">"Ende der Liste · "{format::count(total as u64)}" Module"</p> })}
        </div>
    }
}

/// A module as a row of a list: the catalog's, and the list of marked modules. The whole row is
/// a link; the mark at its end is a button next to the link, not inside it.
#[component]
pub(crate) fn Row(
    row: CatalogRow,
    /// Where the row leads on the desktop: in the app its list with this module previewed next
    /// to it, on the server's page the module's own page. On a phone it leads to the module's
    /// own page either way, unless the list shows its modules `in_place`.
    #[prop(into)] preview: Signal<String>,
    /// This module is the one previewed.
    #[prop(into)] current: Signal<bool>,
    phone: RwSignal<bool>,
    with_program: bool,
    /// In the list of marked modules a module whose mark was taken away stays where it is,
    /// dimmed, so that a slip is one click to undo.
    #[prop(optional)] dim_unmarked: bool,
    /// The list shows its modules in place (a local view, `crate::local`): on a phone as well
    /// the row leads to `preview`, where the module is the page, not to the module's own page.
    #[prop(optional)] in_place: bool,
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
    let target = row.id.clone();
    let finder = use_context::<Finder>();
    // The preview next to the list; on a phone the module's own page, or where the list shows it
    // in place, the module filling the list's page. The module's page takes along what „Einplanen"
    // aims at from the catalog (`?plan=…&fill=…`), as the preview's „Vollbild" does.
    let href = move || {
        if phone.get() && !in_place {
            let hint = finder.and_then(|finder| finder.hint.get()).map(|hint| hint.query()).unwrap_or_default();
            format!("{}{hint}", url::module_path(&target))
        } else {
            preview.get()
        }
    };
    let unmarked = dim_unmarked.then(|| {
        let (bookmarks, id) = (Bookmarks::expect(), row.id.clone());
        Memo::new(move |_| !bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(&id)))
    });
    // With „Passt in meinen Stundenplan" on: how the module fits, where it fits only in part
    // („Übung 1 von 3 frei") or could not be checked. One memo a row (R5); the catalog's list
    // alone has it.
    let fit_note = finder.map(|finder| {
        let id = row.id.clone();
        Memo::new(move |_| finder.view.with(|view| view.note_of(&id, !has_events)))
    });
    let fit_note = move || fit_note.and_then(|note| note.get()).map(|(text, quiet)| view! { <span class="flag fit-note" class:neutral=quiet>{text}</span> });
    view! {
        <div class="row-wrap" class:unmarked=move || unmarked.is_some_and(|unmarked| unmarked.get())>
            <a class="row" href=href data-noscroll="" data-id=row.id.clone() aria-current=move || current.get().then_some("true")>
                <div class="t">
                    <b>{row.title.clone()}</b>
                    <small>
                        <span class="mono">{row.id.clone()}</span>
                        {with_program.then(|| view! { <KindBadge kind=row.kind.clone()/> })}
                        // Inside a program the study plan's semester stands at the row (the
                        // list is in plan order, without headings between the semesters).
                        {with_program.then(|| row.plan_semester.map(|n| view! { <span class="plan-sem">{n}". Semester"</span> }))}
                        <OfferBadge status=row.offer_status.clone()/>
                        {(row.is_fues && !with_program).then(|| view! { <span class="flag neutral">"FÜS"</span> })}
                        {(row.is_limited == Some(true)).then(|| view! { <span class="flag neutral">"begrenzte Plätze"</span> })}
                        {fit_note}
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
            // Marking belongs to the browser app (R9, R15). The server leaves the button out and the
            // stylesheet keeps its place at the end of the row, so nothing moves at the takeover.
            {APP.then(|| view! { <MarkButton id=row.id.clone() title=row.title.clone() look=MarkLook::Row/> })}
        </div>
    }
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
    areas: Vec<CatalogArea>,
    total: u64,
}

impl Facts {
    fn of(data: &CatalogData) -> Self {
        Self {
            program: data.program.clone(),
            curricular_total: data.curricular_total,
            fues_total: data.fues_total,
            plan_semesters: data.plan_semesters.clone(),
            areas: data.areas.clone(),
            total: data.page.total,
        }
    }

    /// The same for the draft of the phone's filter sheet, which has no list yet.
    fn of_summary(summary: &CatalogSummary) -> Self {
        Self {
            program: summary.program.clone(),
            curricular_total: summary.curricular_total,
            fues_total: summary.fues_total,
            plan_semesters: summary.plan_semesters.clone(),
            areas: summary.areas.clone(),
            total: summary.total,
        }
    }
}

/// An area as the picker offers it: its name and how many modules it holds, nothing else (owner,
/// 2026-09-21: the path of the tree beside the name pushed the names into „Wahlpflichtmod…"),
/// under the heading of its section (`pages::catalog_areas`: „Nebenfach" for Mathematik,
/// Physik …). The full label and the path still find the area when they are typed.
fn area_item(area: &CatalogArea) -> ComboItem {
    ComboItem::new(area.id.to_string(), area.name().to_string(), format::modules(i64::try_from(area.modules).unwrap_or(i64::MAX)), 0)
        .also_found_by(&format!("{} {}", area.label, area.path))
        .in_group(area.section.clone().unwrap_or_default())
}

/// What the pickers of the catalog offer, made once per snapshot by a host that can (the server,
/// `server/src/snapshot.rs`) and handed to every render: every program, department and person does
/// not change with the filter, and loading them was 12 of the 30 ms a page of the catalog cost the
/// server (load test 2026-09-26: the persons alone 8.5 ms). A page without it loads them itself.
#[derive(Clone)]
pub struct PickerChoices(Arc<Choices>);

impl PickerChoices {
    pub fn of(data: &CatalogChoices) -> Self {
        Self(Arc::new(Choices::of(data)))
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
            departments: data.departments.iter().map(|d| ComboItem::new(d.id.to_string(), d.label.clone(), format::modules(d.modules), 0)).collect(),
            lecturers: data.lecturers.iter().map(|l| ComboItem::new(l.name.clone(), l.name.clone(), l.title.clone().unwrap_or_default(), 0)).collect(),
        }
    }
}

/// The program picker's entries with „Mein Studiengang" (its slug) once more at the top, under
/// that heading; the others stay as they are, the program among them.
fn mine_first(programs: &[ComboItem], mine: Option<&str>) -> Vec<ComboItem> {
    let first = mine.and_then(|slug| programs.iter().find(|item| item.id == slug)).map(|item| item.clone().in_group("Mein Studiengang"));
    first.into_iter().chain(programs.iter().cloned()).collect()
}

/// The catalog that `change` leads to from the current filter: what a control links to. It keeps
/// the module previewed and the placeholder the list is looked through for (`fill`).
fn target(query: Memo<CatalogQuery>, open: Memo<Option<String>>, fill: Memo<Option<u32>>, change: impl FnOnce(&mut CatalogQuery)) -> String {
    let mut next = query.get();
    change(&mut next);
    CatalogUrl { query: next, page: 1, open: open.get(), fill: fill.get() }.path()
}

/// The filter that `change` makes of the current one, from an event handler (nothing to track).
fn changed(query: Memo<CatalogQuery>, change: impl FnOnce(&mut CatalogQuery)) -> CatalogQuery {
    let mut next = query.get_untracked();
    change(&mut next);
    next
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
type Held = dyn Fn(&CatalogQuery) -> bool + Send + Sync;

/// How a chip reads its state from the filter and writes it back.
#[derive(Clone)]
struct Toggle {
    read: Arc<ReadTri>,
    write: Arc<WriteTri>,
    /// Off → with → without → off. Otherwise only off ↔ with.
    excludes: bool,
    /// When the filter is such that the chip has to stay as it is (the last class the finder
    /// compares), and why: it is no link then.
    held: Option<(Arc<Held>, &'static str)>,
}

impl Toggle {
    fn new(read: impl Fn(&CatalogQuery) -> Tri + Send + Sync + 'static, write: impl Fn(&mut CatalogQuery, Tri) + Send + Sync + 'static) -> Self {
        Self { read: Arc::new(read), write: Arc::new(write), excludes: true, held: None }
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
    fill: Memo<Option<u32>>,
    toggle: Toggle,
    #[prop(into)] label: String,
    icon: Option<&'static str>,
    /// What the chip means, where its label says it short.
    #[prop(optional)]
    title: Option<&'static str>,
    /// A chip of „Passt in meinen Stundenplan", which only the browser app can act on: part of the
    /// server's page all the same, kept in its place but not shown until the app runs (`.fit-chip`,
    /// R9, R15), so the filters below it do not move at the takeover.
    #[prop(optional)]
    finder: bool,
    /// Switched on, it shows more choices under it (the classes the finder compares): a chevron
    /// at its end says so, like the head of an accordion, and points down while they show
    /// (owner, 2026-09-26: „so dass man sieht dass dann noch mehr kommt").
    #[prop(optional)]
    opens: bool,
) -> impl IntoView {
    let read = toggle.read.clone();
    let state = Memo::new(move |_| query.with(|q| read(q)));
    let excludes = toggle.excludes;
    let why = toggle.held.as_ref().map(|(_, why)| *why);
    let held_by = toggle.held.clone();
    let held = Memo::new(move |_| held_by.as_ref().is_some_and(|(held, _)| query.with(|q| held(q))));
    // The link reads the filter itself and not `state` or `held`. A closure that reads a memo
    // derived from `query` before `query` misses a change of `query` whenever the derived value
    // stays the same (reactive_graph 0.2.14 does not mark the observer that made a memo
    // recompute, and the derived memo then reports „unchanged"). That was the first toggle of the
    // panel losing the rest of the filter. Rule (docs/frontend.md, R16): in one closure read the
    // source, not a memo derived from it and the source.
    let href = move || {
        if toggle.held.as_ref().is_some_and(|(held, _)| query.with(|q| held(q))) {
            return None;
        }
        Some(target(query, open, fill, |q| {
            let next = toggle.after((toggle.read)(q));
            (toggle.write)(q, next)
        }))
    };
    let name = label.clone();
    view! {
        <a
            class="chip"
            class:fit-chip=finder
            href=href
            aria-disabled=move || held.get().then_some("true")
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
            aria-expanded=move || opens.then(|| if state.get() == Tri::With { "true" } else { "false" })
            aria-label=move || match state.get() {
                Tri::Without => format!("{name}: ausgeschlossen"),
                _ => name.clone(),
            }
            title=move || match (held.get(), state.get(), excludes) {
                (true, _, _) => why,
                (false, Tri::Off, true) => Some("Klick: nur mit · zweiter Klick: ohne"),
                (false, Tri::With, true) => Some("Nur mit. Noch ein Klick schließt aus"),
                (false, Tri::Without, _) => Some("Ausgeschlossen. Ein Klick hebt das auf"),
                _ => title,
            }
        >
            <span class="box"><Icon name="check"/><Icon name="x"/></span>
            {icon.map(|name| view! { <Icon name=name/> })}
            <span class="chip-label">{label}</span>
            {opens.then(|| view! { <Icon name="chevron-right" class="chip-more"/> })}
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

fn segmented(query: Memo<CatalogQuery>, open: Memo<Option<String>>, fill: Memo<Option<u32>>, label: &'static str, choices: Vec<Choice>) -> impl IntoView {
    let links = choices
        .into_iter()
        .map(|choice| {
            let Choice { label, title, count, is_on, choose } = choice;
            view! {
                <a
                    href=move || target(query, open, fill, |q| choose(q))
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
/// On a phone, where the panel is a sheet over the list, both change the sheet's `draft`
/// instead, and the list follows when the sheet closes (`CatalogPage`).
#[component]
fn Filters(
    query: Memo<CatalogQuery>,
    facts: Memo<Facts>,
    choices: Memo<Arc<Choices>>,
    open: Memo<Option<String>>,
    /// The placeholder the list is looked through for: every link of the panel keeps it, only
    /// „Zurücksetzen" drops it with the rest.
    fill: Memo<Option<u32>>,
    draft: RwSignal<Option<CatalogQuery>>,
    phone: RwSignal<bool>,
) -> impl IntoView {
    let going = Pending::expect();
    let go = Callback::new(move |next: CatalogQuery| {
        if phone.get_untracked() {
            draft.set(Some(next));
        } else if let Some(going) = going {
            going.go(&CatalogUrl { query: next, page: 1, open: open.get_untracked(), fill: fill.get_untracked() }.path(), NavigateOptions { scroll: false, ..Default::default() });
        }
    });
    // A link of the panel is the list it leads to, so on a phone its address is what the draft
    // becomes; the router must not follow it then (it ignores a click whose default is prevented).
    let into_draft = move |ev: leptos::ev::MouseEvent| {
        if !phone.get_untracked() || ev.default_prevented() || ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key() {
            return;
        }
        let Some(href) = nav::link_under(ev.target()) else { return };
        let Some(search) = href.strip_prefix(url::CATALOG).filter(|rest| rest.is_empty() || rest.starts_with('?')) else { return };
        ev.prevent_default();
        draft.set(Some(CatalogUrl::parse(search).query));
    };
    let close_popups = RwSignal::new(0u32);
    provide_context(ClosePopups(close_popups));

    // Parts that come and go are keyed on what they depend on, not on the whole filter.
    let program = Memo::new(move |_| facts.with(|f| f.program.clone()));
    let semesters = Memo::new(move |_| facts.with(|f| f.plan_semesters.clone()));
    let areas = Memo::new(move |_| facts.with(|f| f.areas.clone()));
    // The areas belong to the curriculum; the FÜS list of a program has none.
    let curricular = Memo::new(move |_| query.with(|q| q.program.as_ref().is_none_or(|scope| scope.relation == ProgramRelation::Curricular)));
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

    let chip = move |label: &str, icon: Option<&'static str>, toggle: Toggle| view! { <Chip query open fill toggle label=label.to_string() icon/> };

    // ---- program ----
    let program_picker = if APP {
        // „Mein Studiengang" first, under its own heading, while its PO is in the snapshot (A.10);
        // it stays in its place among all the others as well.
        let mine = MineResolved::expect();
        let mine_slug = Memo::new(move |_| mine.and_then(MineResolved::exact).map(|program| program.slug));
        let items = Memo::new(move |_| {
            let mine = mine_slug.get();
            choices.with(|c| mine_first(&c.programs, mine.as_deref()))
        });
        let selected = Memo::new(move |_| query.with(|q| q.program.as_ref().map(|scope| scope.program_slug.clone())));
        let pick = Callback::new(move |slug: Option<String>| {
            go.run(changed(query, |q| match slug {
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
                            // An area is part of the curriculum's tree: the FÜS list has none.
                            if relation == ProgramRelation::Fues {
                                scope.areas.clear();
                            }
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
                    {segmented(query, open, fill, "Fachsemester", all)}
                }
                .into_any()
            };
            // ---- area: „Praktische Informatik" and the like, out of the tree — only the areas a
            // student chooses from (the Pflichtmodule are taken anyway; owner, 2026-09-21) ----
            let area_part = move || {
                // The sections in the order the picker shows them: those without a heading first.
                let sections = pages::area_sections(&areas.get());
                (curricular.get() && !sections.is_empty()).then(|| {
                    let picker = if APP {
                        let items = StoredValue::new(sections.iter().flat_map(|(_, areas)| areas.iter().map(area_item)).collect::<Vec<_>>());
                        let chosen = move || query.with(|q| q.program.as_ref().map(|scope| scope.areas.clone()).unwrap_or_default());
                        let selected = Signal::derive(move || match chosen().as_slice() {
                            [id] => Some(id.to_string()),
                            _ => None,
                        });
                        // Several areas (from a row of the plan): the button says how many, the
                        // tags above the list say which; picking one replaces them.
                        // One area the picker does not offer (a fixed one, opened from the program's
                        // page) is named all the same.
                        let summary = Signal::derive(move || match chosen().as_slice() {
                            [] => None,
                            [id] => areas.with(|areas| areas.iter().find(|area| area.id == *id && !area.choice).map(|area| area.name().to_string())),
                            several => Some(format!("{} Bereiche", several.len())),
                        });
                        let pick = Callback::new(move |id: Option<String>| {
                            go.run(changed(query, |q| {
                                if let Some(scope) = q.program.as_mut() {
                                    scope.areas = id.and_then(|id| id.parse().ok()).into_iter().collect();
                                }
                            }))
                        });
                        view! {
                            <Combobox id="pick-area" label="Bereich" placeholder="Alle Bereiche" search_placeholder="Bereich suchen" icon="layout-list" min_width=440.0 items=Signal::derive(move || items.get_value()) selected summary on_select=pick/>
                        }
                        .into_any()
                    } else {
                        let chosen = query.with_untracked(|q| q.program.as_ref().map(|scope| scope.areas.clone()).unwrap_or_default());
                        let selected = match chosen.as_slice() {
                            [id] => Some(*id),
                            _ => None,
                        };
                        // Several areas stay one choice of the plain select, so that sending the
                        // form keeps them.
                        let several = (chosen.len() > 1).then(|| {
                            let value = chosen.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
                            view! { <option value=value selected=true>{format!("{} Bereiche", chosen.len())}</option> }
                        });
                        view! {
                            <span class="select-wrap plain">
                                <select name="area" aria-label="Bereich">
                                    <option value="">"Alle Bereiche"</option>
                                    {several}
                                    {sections.into_iter().map(|(group, areas)| {
                                        let options = areas.into_iter().map(|area| view! {
                                            <option value=area.id.to_string() selected=selected == Some(area.id)>{format!("{} ({})", area.name(), area.modules)}</option>
                                        }).collect_view();
                                        match group {
                                            Some(group) => view! { <optgroup label=group>{options}</optgroup> }.into_any(),
                                            None => options.into_any(),
                                        }
                                    }).collect_view()}
                                </select>
                                <Icon name="chevrons-up-down"/>
                            </span>
                        }
                        .into_any()
                    };
                    view! {
                        <div class="flabel label">"Bereich"</div>
                        {picker}
                    }
                })
            };
            view! {
                {segmented(query, open, fill, "Liste", vec![
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
                    {area_part}
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
            move || target(query, open, fill, |q| place(q, &name, wanted))
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
                go.run(changed(query, |q| q.lecturers_include.push(name)));
            }
        });
        view! {
            <Combobox id="pick-lecturer" label="Lehrende" placeholder="Person hinzufügen" search_placeholder="Name suchen" icon="users-round" items selected=Signal::derive(|| None::<String>) on_select=add clearable=false/>
        }
        .into_any()
    } else {
        // Without the app a name is typed. No list of every person to pick from (a `<datalist>`
        // of 705 names, 28 kB on every page of the catalog until 2026-09-26): the server's catalog
        // is there to lead search engines to the modules, and the persons stand on the module's
        // own page (owner: „das soll nur auf die Modulseite").
        view! {
            <label class="field">
                <span class="visually-hidden">"Lehrt oder verantwortet"</span>
                <input type="text" name="lecturer" placeholder="Nachname, Vorname"/>
            </label>
        }
        .into_any()
    };

    // ---- department ----
    let department_picker = if APP {
        let items = Memo::new(move |_| choices.with(|c| c.departments.clone()));
        let selected = Memo::new(move |_| query.with(|q| q.department_id.map(|id| id.to_string())));
        let pick = Callback::new(move |id: Option<String>| go.run(changed(query, |q| q.department_id = id.and_then(|id| id.parse().ok()))));
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

    // ---- „Passt in meinen Stundenplan" (until 2026-09-26 „Passt in meinen Plan", which read as
    // the Regelstudienplan): only the browser app can act on it, as only it knows the plan.
    // The server's page has its chips all the same, the same for everybody (the semester is the
    // snapshot's), kept in their place but not shown until the app runs (R9, R15): the chip does
    // not fit beside „Bestätigt", and the filters below would move at the takeover.
    // Switched on it checks against the semester „Einplanen" would plan into: a placeholder's
    // („Modul finden", `fill`) or the snapshot's current one, comparing what it compared the last
    // time it was on (`finder_on`).
    let current = use_source().ok().and_then(|source| source.run(queries::meta).ok()).and_then(|meta| meta.current_semester).and_then(|key| SemesterKey::parse(&key));
    let plan = Studyplan::expect().filter(|_| APP);
    let fits_on = Memo::new(move |_| query.with(|q| q.fits.is_some()));
    let finder_chip = current.map(|current| {
        // `fill` and the plan are sources of their own (R16): neither is derived from the other.
        let aim = move || plan.map_or(current, |plan| {
            let fill = fill.get();
            plan.with(|doc| finder_semester(doc, fill, current))
        });
        let toggle = Toggle {
            excludes: false,
            ..Toggle::new(
                |q| if q.fits.is_some() { Tri::With } else { Tri::Off },
                move |q, state| q.fits = (state == Tri::With).then(|| finder_on(aim())),
            )
        };
        // The label alone, without the semester it checks (owner, 2026-09-26: „nur mit Passt in
        // meinen Stundenplan ohne das semester"; switched on, its tag above the list names it). No
        // icon either: the chip needs the width for its label. The chevron at its end says that
        // the classes it compares open under it.
        view! { <Chip query open fill toggle label="Passt in meinen Stundenplan" icon=None finder=true opens=true/> }
    });
    // What it compares while it is on is what it compares when it is switched on next, here and
    // from the Stundenplan (`finder_on`): kept whenever it changes, in a phone's sheet as well.
    let compared = Memo::new(move |_| query.with(|q| q.fits.as_ref().map(finder_text)));
    Effect::new(move |_| {
        if let Some(text) = compared.get() {
            nav::local_set(FINDER_KEY, &text);
        }
    });
    // While it is on: which classes it compares (all by default, and one at least: comparing
    // none would list every module of the semester as fitting), and whether modules without a
    // dated row in the semester, which cannot be checked, are listed too (not by default).
    let class = |get: fn(&FitsFilter) -> bool, set: fn(&mut FitsFilter, bool), compared: bool| Toggle {
        excludes: false,
        held: compared.then(|| {
            let last: Arc<Held> = Arc::new(move |q: &CatalogQuery| q.fits.as_ref().is_some_and(|fits| get(fits) && [fits.lectures, fits.exercises, fits.exams].into_iter().filter(|on| *on).count() == 1));
            (last, "Mindestens eine Art wird verglichen")
        }),
        ..Toggle::new(
            move |q| if q.fits.as_ref().is_some_and(get) { Tri::With } else { Tri::Off },
            move |q, state| {
                if let Some(fits) = q.fits.as_mut() {
                    set(fits, state == Tri::With);
                }
            },
        )
    };
    let finder_options = move || {
        fits_on.get().then(|| view! {
            <div class="chips fit-chip">
                <Chip query open fill toggle=class(|f| f.lectures, |f, on| f.lectures = on, true) label="Vorlesungen" icon=None/>
                <Chip query open fill toggle=class(|f| f.exercises, |f, on| f.exercises = on, true) label="Übungen" icon=None title="Übungen, Seminare, Praktika, Projekte, Tutorien …"/>
                <Chip query open fill toggle=class(|f| f.exams, |f, on| f.exams = on, true) label="Prüfungen" icon=None/>
                <Chip query open fill toggle=class(|f| f.undated, |f, on| f.undated = on, false) label="auch ohne Termine" icon=None title="Auch Module ohne Termine in diesem Semester: sie lassen sich nicht prüfen"/>
            </div>
        })
    };

    // Without the app the pickers above are form fields; what the links set travels with them.
    let carried = move || {
        (!APP).then(|| {
            let pairs = url::parse_pairs(&CatalogUrl { query: query.get(), page: 1, open: open.get(), fill: None }.to_query_string());
            pairs
                .into_iter()
                .filter(|(name, _)| !matches!(name.as_str(), "program" | "area" | "department" | "ects_min" | "ects_max"))
                .map(|(name, value)| view! { <input type="hidden" name=name value=value/> })
                .collect_view()
        })
    };

    view! {
        // `data-draft`: the sheet of a phone is a step of its own in the history (`enhance.js`).
        <aside class="panel filters" id="filters" aria-label="Filter" data-draft=APP.then_some("") on:click=into_draft>
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

                    // Next to the semesters of the plan (owner, 2026-09-23): a module without
                    // published dates probably does not take place.
                    <div class="fgroup">
                        <div class="flabel label">"Termine"</div>
                        <div class="chips">
                            {chip("Bestätigt", Some("calendar-check-2"), Toggle::flag(|q| q.scheduled, |q, value| q.scheduled = value))}
                            {finder_chip}
                        </div>
                        {finder_options}
                    </div>
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
                    <Credits query go/>
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
                            // Marking works in the browser app only, so the filter is there only
                            // (R15); without it the chip would promise what no link can keep.
                            {APP.then(|| chip("Gemerkt", Some("bookmark"), Toggle::flag(|q| q.marked, |q, value| q.marked = value)))}
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
                        {segmented(query, open, fill, "Dauer", vec![
                            Choice::new("Egal", |q| q.duration_semesters.is_none(), |q| q.duration_semesters = None),
                            Choice::new("1 Semester", |q| q.duration_semesters == Some(1), |q| q.duration_semesters = Some(1)),
                            Choice::new("2 Semester", |q| q.duration_semesters == Some(2), |q| q.duration_semesters = Some(2)),
                        ])}
                        <div class="flabel label">"Nur in bestimmten Jahren"</div>
                        {segmented(query, open, fill, "Jahre", vec![
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
fn Credits(query: Memo<CatalogQuery>, go: Callback<CatalogQuery>) -> impl IntoView {
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
                    on:change=move |ev| { let value = dragged(&ev, true); go.run(changed(query, |q| q.credits_min = (value > 0.0).then_some(value))) }/>
                <input type="range" min="0" max="30" step="1" aria-label="Leistungspunkte höchstens"
                    value=start_high.to_string() prop:value=move || high.get().to_string()
                    on:input=move |ev| { dragged(&ev, false); }
                    on:change=move |ev| { let value = dragged(&ev, false); go.run(changed(query, |q| q.credits_max = (value < CREDITS_MAX).then_some(value))) }/>
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
                    on:change=move |ev| if APP { go.run(changed(query, |q| q.credits_min = typed(&ev))) }/>
                <span>"–"</span>
                <input type="number" name="ects_max" min="0" max="60" step="0.5" inputmode="decimal" aria-label="Leistungspunkte höchstens" placeholder="bis"
                    value=query.with_untracked(|q| q.credits_max.map(|n| n.to_string()))
                    prop:value=move || query.with(|q| q.credits_max.map(|n| n.to_string()).unwrap_or_default())
                    on:change=move |ev| if APP { go.run(changed(query, |q| q.credits_max = typed(&ev))) }/>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn mein_studiengang_heads_the_program_picker() {
        let programs = vec![ComboItem::new("bachelor-bwl-2021", "BWL", "B.Sc. · 2021", 1), ComboItem::new("bachelor-informatik-2008", "Informatik", "B.Sc. · 2008", 1)];
        let listed = mine_first(&programs, Some("bachelor-informatik-2008"));
        assert_eq!(listed.iter().map(|item| (item.id.as_str(), item.group.as_str())).collect::<Vec<_>>(), [("bachelor-informatik-2008", "Mein Studiengang"), ("bachelor-bwl-2021", ""), ("bachelor-informatik-2008", "")]);
        // None set, or a slug the picker does not know: the picker as it was.
        assert_eq!(mine_first(&programs, None), programs);
        assert_eq!(mine_first(&programs, Some("bachelor-weg-1999")), programs);
    }

    fn switched_on(filter: FitsFilter) -> CatalogQuery {
        CatalogQuery { text: "analysis".to_string(), fits: Some(filter), ..CatalogQuery::default() }
    }

    /// FS1 of Informatik against WiSe 2026/27, as `pages::fit` answers it: 12330 fits, 12101 has
    /// rows without times, 12974 only a retake, 13583 fits in part; 11103 clashes, 12104 is
    /// planned.
    fn answer() -> FitResult {
        FitResult {
            has_data: true,
            fitting: ids(&["12101", "12330", "12974", "13583"]),
            excluded: ids(&["11103", "12104"]),
            notes: BTreeMap::from([
                ("12101".to_string(), "keine festen Termine".to_string()),
                ("12974".to_string(), "nur Wiederholungsprüfung".to_string()),
                ("13583".to_string(), "Übung 1 von 3 frei".to_string()),
            ]),
            unknown: BTreeSet::from(["12101".to_string(), "12974".to_string()]),
        }
    }

    #[test]
    fn the_finder_asks_the_plan_only_when_it_is_on() {
        // Without the switch the query is left as it is.
        let plain = CatalogQuery { text: "analysis".to_string(), marked: Some(true), ..CatalogQuery::default() };
        assert_eq!(with_fits(plain.clone(), None, None, None), Fitted { query: plain, ..Fitted::default() });
        // With it, but without a plan to check against (the server's page): no ids, which lists
        // nothing, and nothing to say at the rows.
        let on = switched_on(FitsFilter::all("2026W"));
        let unknown = with_fits(on.clone(), None, None, None);
        assert_eq!((unknown.query, unknown.view, unknown.failed), (on, FitView::default(), None));
    }

    #[test]
    fn the_list_holds_what_was_checked_and_fits() {
        // By default the modules checked that fit, with the notes of those that fit in part or
        // could not be checked.
        let filter = FitsFilter::all("2026W");
        let checked = fitted(switched_on(filter.clone()), &filter, Some(&answer()));
        assert_eq!(checked.query.fits_ids, Some(FitIds::Only(ids(&["12101", "12330", "12974", "13583"]))));
        assert_eq!(checked.query.text, "analysis");
        // A partial fit warns; what could not be checked, a retake alone included, is quiet.
        assert_eq!(checked.view.note_of("13583", false), Some(("Übung 1 von 3 frei".to_string(), false)));
        assert_eq!(checked.view.note_of("12101", false), Some(("keine festen Termine".to_string(), true)));
        assert_eq!(checked.view.note_of("12974", false), Some(("nur Wiederholungsprüfung".to_string(), true)));
        assert_eq!((checked.view.note_of("12330", false), checked.view.note_of("11454", false), checked.view.line.clone()), (None, None, None));
        // A row that says „noch keine Termine" itself is not told „keine festen Termine" beside
        // it; what else the finder found out it still is.
        assert_eq!(checked.view.note_of("12101", true), None);
        assert_eq!(checked.view.note_of("12974", true).map(|(note, _)| note).as_deref(), Some("nur Wiederholungsprüfung"));

        // „auch ohne Termine": every module but the clashing and the planned ones, and a module
        // that could not be checked says why — unless its row says so already.
        let undated = FitsFilter { undated: true, ..filter.clone() };
        let also = fitted(switched_on(undated.clone()), &undated, Some(&answer()));
        assert_eq!(also.query.fits_ids, Some(FitIds::Without(ids(&["11103", "12104"]))));
        assert_eq!(also.view.note_of("13164", false), Some(("keine Termine im WiSe 2026/27".to_string(), true)));
        assert_eq!(also.view.note_of("11454", true), None);
        assert_eq!((also.view.note_of("12330", false), also.view.note_of("13583", false).map(|(_, quiet)| quiet)), (None, Some(false)));

        // A semester without dates is not checked: every module but the planned ones, one line
        // above the list, nothing at the rows.
        let summer = FitsFilter::all("2027S");
        let unpublished = FitResult { has_data: false, excluded: ids(&["12204"]), ..FitResult::default() };
        let open = fitted(switched_on(summer.clone()), &summer, Some(&unpublished));
        assert_eq!(open.query.fits_ids, Some(FitIds::Without(ids(&["12204"]))));
        assert_eq!(open.view.line.as_deref(), Some("SoSe 2027: noch keine Termine veröffentlicht."));
        assert_eq!(open.view.note_of("11454", false), None);
        let open_too = fitted(switched_on(FitsFilter { undated: true, ..summer.clone() }), &summer, Some(&unpublished));
        assert_eq!(open_too.query.fits_ids, Some(FitIds::Without(ids(&["12204"]))));
    }

    #[test]
    fn the_switch_checks_the_semester_of_the_placeholder() {
        // „Modul finden" of a placeholder in SoSe 2027, the switch turned off and on again: it
        // checks SoSe 2027 again, where „Einplanen" plans the module into, not the current one.
        let doc = PlanDoc::restored("p\t2\t2027S\t079-82-2008\t30\t2-2\t6\tfues\t\tFachübergreifendes Studium 2");
        let (current, summer) = (SemesterKey::parse("2026W").unwrap(), SemesterKey::parse("2027S").unwrap());
        assert_eq!(finder_semester(&doc, Some(2), current), summer);
        // No placeholder, or one the plan does not hold (any more): the current semester.
        assert_eq!(finder_semester(&doc, None, current), current);
        assert_eq!(finder_semester(&doc, Some(7), current), current);
        assert_eq!(finder_semester(&PlanDoc::default(), Some(2), current), current);
    }

    #[test]
    fn switched_on_again_the_finder_compares_what_it_compared_last() {
        let summer = SemesterKey::parse("2027S").unwrap();
        // The first time: every class compared, modules without dates left out.
        assert_eq!(finder_kept(None, summer), FitsFilter::all("2027S"));
        // What it compared is kept as the address says it, and holds for any semester.
        let chosen = FitsFilter { exams: false, undated: true, ..FitsFilter::all("2026W") };
        assert_eq!(finder_text(&chosen), "fits-skip=exam&fits-undated=1");
        assert_eq!(finder_kept(Some(&finder_text(&chosen)), summer), FitsFilter { semester: "2027S".to_string(), ..chosen });
        let lectures_only = FitsFilter { exercises: false, exams: false, ..FitsFilter::all("2026W") };
        assert_eq!(finder_kept(Some(&finder_text(&lectures_only)), summer), FitsFilter { semester: "2027S".to_string(), ..lectures_only });
        // Everything compared keeps nothing: the key goes.
        assert_eq!(finder_text(&FitsFilter::all("2026W")), "");
        assert_eq!(finder_kept(Some(""), summer), FitsFilter::all("2027S"));
        // Read as an address is read: what it does not know is left out, another semester or
        // another filter changes nothing, and a choice that compares nothing is none.
        let odd = finder_kept(Some("fits-skip=EXAM,yoga&fits=1999W&marked=only&fits-undated=yes"), summer);
        assert_eq!(odd, FitsFilter { exams: false, ..FitsFilter::all("2027S") });
        assert_eq!(finder_kept(Some("fits-skip=lecture,exercise,exam"), summer), FitsFilter::all("2027S"));
        assert_eq!(finder_kept(Some("&&=#?"), summer), FitsFilter::all("2027S"));
    }

    #[test]
    fn what_the_finder_adds_to_the_list_stays_out_of_its_address() {
        let filter = FitsFilter { exams: false, ..FitsFilter::all("2026W") };
        let mut query = fitted(switched_on(filter.clone()), &filter, Some(&answer())).query;
        query.only_ids = Some(ids(&["12330"]));
        query.without_ids = ids(&["11103"]);
        assert_eq!(addressed(&query), switched_on(filter));
        // The tag names the semester and takes the switch away with the rest of the list kept.
        let current = CatalogUrl { query: switched_on(FitsFilter::all("2026W")), fill: Some(3), ..CatalogUrl::default() };
        let found = tags(&current, &[], &[]).into_iter().find(|(group, _, _)| group == "Passt in");
        let (_, value, without) = found.unwrap();
        assert_eq!((value.as_str(), without.path()), ("WiSe 2026/27", "/catalog?q=analysis&fill=p3".to_string()));
    }

    #[test]
    fn nothing_fits_names_what_to_leave_out() {
        let all = FitsFilter::all("2026W");
        assert_eq!(fit_advice(&all), "Übungen oder Prüfungen abwählen.");
        assert_eq!(fit_advice(&FitsFilter { exams: false, ..all.clone() }), "Übungen abwählen.");
        assert_eq!(fit_advice(&FitsFilter { exercises: false, ..all.clone() }), "Prüfungen abwählen.");
        assert_eq!(fit_advice(&FitsFilter { exercises: false, exams: false, ..all }), "Vorlesungen abwählen.");
    }
}
