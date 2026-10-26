//! The chrome around every page: the rail with the main navigation and the way to the profile
//! page, the top bar with the page's title and search (and on a phone the way back of a page or of
//! its step, `TopBack`, and the way to the profile page, which gives way to it), the navigation at
//! the bottom of a phone (`NavItems`, with the digits of its keys, `NavKey`) with what its tabs
//! hold besides their pages (`TabMenu`), the menu of the profiles (`ProfileMenu`), and what lets
//! the memory of the tabs follow the router (`FollowTabs`).

use folia_routes::url::{self, BookmarksUrl, CollectionView, StudyUrl, StudyView};
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use crate::i18n::{self, use_location};
use crate::languages;
use crate::pending::Pending;
use crate::tabs::{Area, TabAgain, Tabs};
use folia_design::nav;
use folia_design::ui::{self, Icon};
use folia_stores::bookmarks::Bookmarks;
use folia_stores::collections::Collections;
use folia_stores::myprogram::{program_name, MineResolved};
use folia_stores::profiles::{Avatar, Profiles, MAX_NAME, MAX_PROFILES};
use folia_stores::studyplan::Studyplan;

/// The browser app (`csr`): only there is „Mein Studium", the first page of „Studium".
const APP: bool = cfg!(feature = "csr");

/// Lets the memory of the tabs follow the router (it needs the router's context).
#[component]
pub fn FollowTabs() -> impl IntoView {
    if let Some(tabs) = Tabs::expect() {
        tabs.follow();
    }
}

/// Where the tab of `area` leads, seen from `path` (the app's path): where its area was left, else
/// where it starts (`Tabs`); the catalog's first entry of a session is the catalog of „Mein
/// Studiengang" (`mine`). „Studium" is „Mein Studium" in the app; without it (the server's page, no
/// JavaScript) there is nothing to plan, and the tab is the overview of the programs. Tracked: the
/// tab's link follows it, and the tabs built at the start are built there (`stage`).
pub fn tab_target(tabs: Option<Tabs>, mine: Option<MineResolved>, area: Area, path: &str) -> String {
    match (tabs, area) {
        (_, Area::Programs) if !APP => url::PROGRAMS.to_string(),
        (Some(tabs), Area::Catalog) => tabs.href_with_root(area, path, &mine.map_or_else(|| url::CATALOG.to_string(), MineResolved::catalog_href)),
        (Some(tabs), _) => tabs.href(area, path),
        (None, _) => area.root().to_string(),
    }
}

/// `window.__betulaStands(area)` for enhance.js: whether a tap on the tab of `area` (its `data-area`)
/// only swaps the stages.
#[cfg(feature = "csr")]
fn expose_stands(stands: impl Fn(Area) -> bool + 'static) {
    use leptos::wasm_bindgen::{closure::Closure, JsValue};
    let Some(window) = leptos::web_sys::window() else { return };
    let ask = Closure::<dyn Fn(JsValue) -> bool>::new(move |area: JsValue| area.as_string().and_then(|name| Area::from_name(&name)).is_some_and(&stands));
    let _ = leptos::web_sys::js_sys::Reflect::set(&window, &JsValue::from_str("__betulaStands"), &ask.into_js_value());
}

/// The main navigation. Its items are tabs: each leads to where its area was left (`tabs`), and is
/// a digit away on a keyboard (`Area::key`, `NavKey`).
#[component]
pub fn NavItems() -> impl IntoView {
    let t = i18n::t();
    let location = use_location();
    let tabs = Tabs::expect();
    // The tab of the page the app is going to is current at once, before the page is there.
    let pending = Pending::expect();
    // The profile page is no area's: the way to it is current there (`ProfileButton`), no tab.
    let current = move |area: Area| {
        let path = pending.and_then(|p| p.path()).unwrap_or_else(|| location.pathname.get());
        (path != url::PROFILE && Area::of(&path) == area).then_some("page")
    };
    // The catalog's first entry of a session is the catalog of „Mein Studiengang", while its PO is
    // in the snapshot (A.10); nothing is known of it on the server, whose tab is the plain link (R9).
    let mine = MineResolved::expect();
    // The catalog's tab held on a phone opens the catalog's list with its filters (enhance.js): the
    // list as it was left, else the catalog its tab starts at.
    let catalog_list = move || {
        APP.then(|| t.path(&tabs.and_then(Tabs::catalog_list).unwrap_or_else(|| mine.map_or_else(|| url::CATALOG.to_string(), MineResolved::catalog_href))))
    };
    // Where a tab leads, as the app's path.
    let target = move |area: Area| tab_target(tabs, mine, area, &location.pathname.get());
    let href = move |area: Area| t.path(&target(area));
    // Whether a tap on a tab only swaps the stages, where it leads standing already: then its page
    // follows a finger along the bar (enhance.js, the Achse), `__betulaStands(area)`.
    #[cfg(feature = "csr")]
    if let Some(pending) = pending {
        expose_stands(move |area| untrack(|| pending.stands(&crate::nav::tab_top(area, &target(area)))));
    }
    // A tab tapped where its area is (not the end of a swipe along the bar, enhance.js): its page
    // goes back to where it starts (`TabAgain`). On a phone the tab is a stack of its own: the
    // history becomes Start and the tab's stack as it was left, and in its own tab back to its
    // first page (`nav::tab`). Elsewhere the tab's page goes back through the history where it is
    // the step before (folia/design/zurueck/konzept.html), else it is a step of its own.
    let again = TabAgain::expect();
    let tap = move |area: Area| {
        move |ev: leptos::ev::MouseEvent| {
            let plain = ev.button() == 0 && !(ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key());
            if !plain || ev.default_prevented() {
                return;
            }
            let here = untrack(|| current(area)).is_some();
            if let Some(pending) = pending.filter(|_| APP) {
                let to = untrack(|| target(area));
                if crate::nav::tab(pending, area, &to) || pending.back_if_before(&to) {
                    ev.prevent_default();
                }
            }
            if here {
                if let Some(again) = again {
                    again.tapped(area);
                }
            }
        }
    };
    // The Stundenplan is the timetable of the current semester: its tab counts what that semester
    // holds, not what „Mein Studium" places into later ones; every module planned until the
    // catalog has said which semester is the current one.
    let plan = Studyplan::expect();
    let meta = folia_data::use_ask(|| APP.then_some(folia_pages::ask::MetaAsk {}));
    let semester = Memo::new(move |_| meta.with(|meta| meta.as_ref().and_then(|meta| meta.as_ref().ok()).and_then(|meta| meta.current_semester.as_deref().and_then(folia_calendar::semester::SemesterKey::parse))));
    // Whether the visitor has a collection: the Merkliste's tab has a menu then.
    let collections = Collections::expect();
    let collected = Memo::new(move |_| collections.is_some_and(|collections| collections.with(|shelf| !shelf.collections().is_empty())));
    let planned = Memo::new(move |_| match (plan, semester.get()) {
        (Some(plan), Some(current)) => plan.count_in(current),
        (Some(plan), None) => plan.count(),
        (None, _) => 0,
    });
    // Each a command of „Gehe zu" with its keys (`crate::commands`).
    view! {
        <a class="nav" data-area="home" data-key=Area::Home.key() data-command=t.app.home data-command-group="go" data-command-keys=format!("Nav+{}", Area::Home.key()) href=t.path(url::HOME) on:click=tap(Area::Home) title=t.app.home aria-current=move || current(Area::Home)><span class="ind"><Icon name="house"/><NavKey area=Area::Home/></span>{t.app.home}</a>
        <a class="nav" data-area="catalog" data-key=Area::Catalog.key() data-command=t.app.modules data-command-group="go" data-command-keys=format!("Nav+{}", Area::Catalog.key()) href=move || href(Area::Catalog) on:click=tap(Area::Catalog) data-hold=APP.then_some("filters") data-list=catalog_list title=t.app.modules aria-current=move || current(Area::Catalog)><span class="ind"><Icon name="layout-list"/><NavKey area=Area::Catalog/></span>{t.app.modules}</a>
        <a class="nav" data-area="programs" data-key=Area::Programs.key() data-command=t.app.study data-command-group="go" data-command-keys=format!("Nav+{}", Area::Programs.key()) href=move || href(Area::Programs) on:click=tap(Area::Programs) data-hold=APP.then_some("menu") title=t.app.programs aria-current=move || current(Area::Programs)><span class="ind"><Icon name="graduation-cap"/><NavKey area=Area::Programs/></span>{t.app.study}</a>
        // The marked modules exist in the browser app only (R15). How many there are is known
        // there alone, so the number is never part of server HTML (R9). What the visitor keeps is
        // no page for a crawler (`seo`), here and in the Studienplan.
        // Held, or its right click, it has a menu of the collections (`TabMenu`), once there is one.
        <a class="nav js-only" data-area="bookmarks" data-key=Area::Bookmarks.key() data-command=t.app.bookmarks data-command-group="go" data-command-keys=format!("Nav+{}", Area::Bookmarks.key()) href=move || href(Area::Bookmarks) on:click=tap(Area::Bookmarks) data-hold=move || (APP && collected.get()).then_some("menu") rel="nofollow" title=t.app.bookmarks aria-current=move || current(Area::Bookmarks)>
            <span class="ind">
                <Icon name="bookmark"/>
                <NavKey area=Area::Bookmarks/>
                {move || {
                    let marked = Bookmarks::expect().map(|bookmarks| bookmarks.count()).unwrap_or(0);
                    (marked > 0).then(|| view! { <span class="nav-count num" aria-label=(t.app.marked_count)(marked)>{if marked > 99 { "99+".to_string() } else { marked.to_string() }}</span> })
                }}
            </span>
            {t.app.bookmarks}
        </a>
        // The Studienplan lives in the browser app alone, like the Merkliste (R15), and so does the
        // number of its modules (R9). The number is a memo of its own: most changes of the plan
        // (a hidden Termin, a move) leave it as it is.
        <a class="nav js-only" data-area="studyplan" data-key=Area::Studyplan.key() data-command=t.app.studyplan data-command-group="go" data-command-keys=format!("Nav+{}", Area::Studyplan.key()) href=move || href(Area::Studyplan) on:click=tap(Area::Studyplan) rel="nofollow" title=t.app.studyplan aria-current=move || current(Area::Studyplan)>
            <span class="ind">
                <Icon name="calendar-range"/>
                <NavKey area=Area::Studyplan/>
                {move || {
                    let planned = planned.get();
                    (planned > 0).then(|| view! { <span class="nav-count num" aria-label=(t.app.planned_count)(planned)>{if planned > 99 { "99+".to_string() } else { planned.to_string() }}</span> })
                }}
            </span>
            {t.app.studyplan}
        </a>
    }
}

/// The digit that leads to a tab with the navigation's keys (owner, 2026-10-16: „wenn man ctrl/alt
/// drückt sollen dann die shortcuts bei den nav items angezeigt werden"): beside the tab while they
/// are held, after the words of the system, „Alt + 1" (enhance.js, app.css). A name read out says
/// the tab's own, not its digit.
#[component]
fn NavKey(area: Area) -> impl IntoView {
    view! { <kbd class="nav-key" aria-hidden="true">{area.key()}</kbd> }
}

/// Where a tab's menu stands (`TabMenu`): the tab's area, and its box on the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Held {
    area: Area,
    /// The tab is one of the bar at the bottom of a phone (the menu stands above the bar), else
    /// one of the rail (the menu stands beside it).
    bar: bool,
    top: f64,
    right: f64,
    middle: f64,
}

/// What a tab holds besides its page (owner, 2026-10-06: „wenn man die buttons in der nav bar lange
/// gedrückt hält, dass dann eine special aktion kommt. Bei mein Studium währe das dann eine auswahl
/// von Regelstudienplan, Wahlpflicht, Alle Studiengänge. Das sollte natürlich auch mit
/// [Rechts]klick funktionieren."): a menu at the tab, over the page. „Studium"'s leads to the
/// Regelstudienplan and the Wahlpflicht of „Mein Studium" (where a program is chosen; owner,
/// 2026-10-14, concept A: views of the study, with the student's stand in them) and to all
/// programs. The Merkliste's, once there is a collection, to its views (folia/design/merken/
/// oberflaeche.html, „Der Tab, lange gedrückt"): all of it, „Ohne Sammlung" and each collection,
/// with how many marked modules each holds. enhance.js hears
/// the long press of a finger and the right click and names the tab (`betula:tab-menu` on it,
/// where `NavItems` marked it `data-hold="menu"`); a tap beside the menu, Esc, a way taken or
/// another page closes it. The browser app's alone. (The catalog's tab, held on a phone, opens
/// the catalog with its filters: enhance.js does that by itself.)
#[component]
pub fn TabMenu() -> impl IntoView {
    let t = i18n::t();
    let held = RwSignal::new(None::<Held>);
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        use leptos::wasm_bindgen::JsCast;
        let open = window_event_listener_untyped("betula:tab-menu", move |ev| {
            let Some(tab) = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok()) else { return };
            let Some(area) = tab.get_attribute("data-area").as_deref().and_then(Area::from_name) else { return };
            let rect = tab.get_bounding_client_rect();
            let bar = tab.closest(".bottomnav").ok().flatten().is_some();
            held.set(Some(Held { area, bar, top: rect.top(), right: rect.right(), middle: rect.left() + rect.width() / 2.0 }));
        });
        let keys = window_event_listener(leptos::ev::keydown, move |ev| {
            if ev.key() == "Escape" && held.with_untracked(Option::is_some) && !crate::nav::watched() {
                ev.prevent_default();
                held.set(None);
            }
        });
        on_cleanup(move || {
            open.remove();
            keys.remove();
        });
    });
    // What lies over the page: on Android Back closes it (`nav`).
    crate::nav::layer_while(move || held.with(Option::is_some), move || held.set(None), crate::nav::Kind::Over);
    // Another page: the menu goes.
    let location = use_location();
    Effect::new(move |before: Option<()>| {
        location.pathname.track();
        location.search.track();
        if before.is_some() {
            held.set(None);
        }
    });
    let mine = MineResolved::expect();
    // „Mein Studiengang" while the catalog knows it.
    let program = Memo::new(move |_| mine.and_then(|mine| mine.0.with(|info| info.as_ref().map(|info| info.program.clone()))));
    let first = NodeRef::<leptos::html::A>::new();
    // The first way has the focus: Tab goes on from there, Esc closes the menu.
    Effect::new(move |_| {
        if let Some(first) = first.get() {
            let _ = first.focus();
        }
    });
    let close = move |_| held.set(None);
    let bookmarks = Bookmarks::expect();
    let collections = Collections::expect();
    move || {
        let at = held.get()?;
        // Each way: its icon, its words, where it leads, how many it holds, whether a line is
        // before it.
        let ways: Vec<(&'static str, String, String, Option<usize>, bool)> = match at.area {
            Area::Programs => {
                let mut ways = Vec::new();
                if program.with(Option::is_some) {
                    ways.push(("file-check-2", t.app.to_plan.to_string(), StudyUrl::default().with_view(StudyView::Plan).path(), None, false));
                    ways.push(("list-checks", t.app.to_areas.to_string(), StudyUrl::default().with_view(StudyView::Choices).path(), None, false));
                }
                ways.push(("graduation-cap", t.app.all_programs.to_string(), url::PROGRAMS.to_string(), None, false));
                ways
            }
            Area::Bookmarks => {
                let (Some(bookmarks), Some(collections)) = (bookmarks, collections) else { return None };
                let marked: std::collections::BTreeSet<String> = bookmarks.marks_untracked().into_iter().map(|mark| mark.id).collect();
                collections.with_untracked(|shelf| {
                    if shelf.collections().is_empty() {
                        return Vec::new();
                    }
                    let view = |collection: CollectionView| BookmarksUrl { collection, ..Default::default() }.path();
                    let without = marked.iter().filter(|id| shelf.of(id).is_empty()).count();
                    let mut ways = vec![
                        ("bookmark", t.app.all_marked.to_string(), view(CollectionView::All), Some(marked.len()), false),
                        ("", t.app.without_collection.to_string(), view(CollectionView::Without), Some(without), false),
                    ];
                    for (i, collection) in shelf.collections().iter().enumerate() {
                        let count = shelf.count_in(collection.id, |id| marked.contains(id));
                        ways.push(("tag", collection.name.clone(), view(CollectionView::Of(collection.id)), Some(count), i == 0));
                    }
                    ways
                })
            }
            _ => Vec::new(),
        };
        if ways.is_empty() {
            return None;
        }
        let name = (at.area == Area::Programs).then(|| program.get().map(|program| program_name(&program))).flatten();
        let label = (t.app.shortcuts)(match at.area {
            Area::Programs => t.app.study,
            Area::Catalog => t.app.modules,
            Area::Home => t.app.home,
            Area::Bookmarks => t.app.bookmarks,
            Area::Studyplan => t.app.studyplan,
        });
        // Over the bar, above the tab (app.css keeps it on the screen); beside the rail, at the tab.
        let place = if at.bar { format!("--tab-x: {:.0}px", at.middle) } else { format!("left: {:.0}px; top: {:.0}px", at.right + 8.0, at.top) };
        Some(view! {
            <div class="tab-scrim" on:click=close on:contextmenu=move |ev: leptos::ev::MouseEvent| { ev.prevent_default(); held.set(None) }></div>
            <nav class="tab-menu" class:on-bar=at.bar aria-label=label style=place>
                {name.map(|name| view! { <p class="tab-menu-head">{name}</p> })}
                {ways.into_iter().enumerate().map(|(i, (icon, words, href, count, line))| {
                    let node = if i == 0 { first } else { NodeRef::new() };
                    // A way without an icon keeps its place, so that the words stand in a line.
                    let icon = if icon.is_empty() { view! { <span class="icon" aria-hidden="true"></span> }.into_any() } else { view! { <Icon name=icon/> }.into_any() };
                    view! {
                        {line.then(|| view! { <hr class="tab-menu-rule"/> })}
                        <a class="tab-menu-way" href=t.path(&href) node_ref=node on:click=close>{icon}<span>{words}</span>{count.map(|count| view! { <span class="tab-menu-n num">{count}</span> })}</a>
                    }
                }).collect_view()}
            </nav>
        })
    }
}

#[component]
pub fn Rail() -> impl IntoView {
    let t = i18n::t();
    view! {
        // The navigation is a region of the keyboard and one stop of the Tab key, whose ↑ and ↓ go
        // from one of its buttons to the next (enhance.js, „the keyboard"; folia/design/tastatur/
        // konzept.html, question 8).
        <aside class="rail" data-region="nav" data-stop="y">
            <a class="logo hit" href=t.path(url::HOME) aria-label=t.app.logo_label><ui::Mark/></a>
            <nav aria-label=t.app.main_navigation><NavItems/></nav>
            <div class="rail-end">
                <crate::keys::KeysButton/>
                <ProfileButton/>
                <button class="icon-btn theme-toggle js-only" type="button" data-action="theme" aria-label=t.app.theme_toggle>
                    <Icon name="moon" class="icon-moon"/><Icon name="sun" class="icon-sun"/>
                </button>
            </div>
        </aside>
    }
}

/// The way to the profile page (owner, 2026-10-14; `folia_profile`): at the end of the rail above
/// the switch between light and dark, and on a phone, which has no rail, a box at the right of the
/// search at the head of every page (`top`; owner, 2026-10-14: „für die mobile Version noch ein
/// Konzept wie man an sein Profil rankommt"), as the way back of a page's step is a box at its left
/// (`TopBack`). It shows the active profile's badge (`Avatar`) where there is more than one
/// profile or the one there is has a name, else the icon of a person, and names the profile for
/// those who do not see it. A mouse resting on it, a right click or a long press opens the menu of
/// the profiles (`ProfileMenu`), and so does a tap on the phone's box (owner, 2026-10-19: „die
/// häufigste Aktion wird sein, dass man das nur das Profil wechseln will"): there the page is one of
/// the menu's ways. The profiles live in the browser (R20): the server writes the icon, the same for
/// everybody (R9), and without JavaScript the way leads to the page's stand-in, which offers the
/// other language (`folia_profile`).
#[component]
pub fn ProfileButton(
    #[prop(optional)] top: bool,
    /// The box at the head has given way to the way back (`TopBack`): out of reach until it is back.
    #[prop(optional, into)]
    away: Signal<bool>,
) -> impl IntoView {
    let t = i18n::t();
    let location = use_location();
    let profiles = Profiles::expect().filter(|_| APP);
    let current = Memo::new(move |_| profiles.map(|profiles| (profiles.current(), profiles.count())));
    let badge = Memo::new(move |_| current.get().filter(|(profile, count)| *count > 1 || !profile.name.is_empty()).map(|(profile, _)| profile));
    let label = move || current.with(|current| current.as_ref().map_or_else(|| t.app.profile.to_string(), |(profile, _)| (t.app.profile_of)(&profile.shown(t.locale))));
    // The profile page is current at once, before it is there (`pending`).
    let pending = Pending::expect();
    let here = move || (pending.and_then(|p| p.path()).unwrap_or_else(|| location.pathname.get()) == url::PROFILE).then_some("page");
    let menu = ProfileMenu::expect().filter(|_| APP);
    let node = NodeRef::<leptos::html::A>::new();
    // The phone's box opens the menu (the browser app's): it says so, and whether it is open.
    let opens = top && menu.is_some();
    let open = move || menu.is_some_and(|menu| menu.at.with(|at| at.is_some_and(|at| !at.rail && !at.hover)));
    view! {
        <a
            class=if top { "top-profile profile-btn" } else { "icon-btn profile-btn" }
            href=t.path(url::PROFILE)
            rel="nofollow"
            aria-label=label
            aria-current=here
            aria-haspopup=opens.then_some("true")
            aria-expanded=move || opens.then(|| if open() { "true" } else { "false" })
            inert=move || away.get()
            data-hold=APP.then_some("profiles")
            // A command of „Gehe zu" (`crate::commands`), by its name or the profile's.
            data-command=t.app.profile
            data-command-group="go"
            data-command-words=label
            node_ref=node
            on:pointerenter=move |ev| {
                if let Some(menu) = menu {
                    menu.pointer_on(&ev, true);
                }
            }
            on:pointerleave=move |ev| {
                if let Some(menu) = menu {
                    menu.pointer_off(&ev);
                }
            }
            on:click=move |ev: leptos::ev::MouseEvent| {
                let Some(menu) = menu else { return };
                let plain = ev.button() == 0 && !(ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key());
                // The phone's box: a tap opens the menu, and closes it again.
                if top && plain && folia_design::nav::is_phone() {
                    ev.prevent_default();
                    if open() {
                        menu.close();
                    } else {
                        #[cfg(feature = "csr")]
                        if let Some(way) = node.get_untracked() {
                            menu.open_at(&way, false);
                        }
                    }
                    return;
                }
                // The page chosen: no menu opens over it a moment later.
                menu.close();
            }
        >
            {move || match badge.get() {
                Some(profile) => view! { <Avatar profile/> }.into_any(),
                None => view! { <Icon name="user-round"/> }.into_any(),
            }}
        </a>
    }
}

/// A mouse resting on the way to the profile page opens its menu this much later; one passing over
/// it on its way elsewhere opens nothing.
const HOVER_OPEN: std::time::Duration = std::time::Duration::from_millis(160);

/// A menu the mouse opened goes this much later once the mouse left it and the way: enough to
/// cross the gap between them, or to come back.
const HOVER_CLOSE: std::time::Duration = std::time::Duration::from_millis(280);

/// Where the menu of the profiles stands (`ProfileMenu`), and how it opened.
#[derive(Clone, Copy, Debug, PartialEq)]
struct MenuAt {
    /// At the rail's way: the menu stands beside it and grows up from its foot. Else at the box of
    /// a phone's head: the menu hangs below it, at its right edge.
    rail: bool,
    /// A mouse resting on the way opened it, and it goes when the mouse has left the way and the
    /// menu. Else a right click, the menu key or a long press did: it stays until a choice, a tap
    /// beside it or Esc, and has the focus.
    hover: bool,
    /// The way's box on the screen, and the size of the window.
    bottom: f64,
    right: f64,
    width: f64,
    height: f64,
}

/// The menu of the profiles (owner, 2026-10-14: „wenn man beim pc über das profil in der nav bar
/// links hovert, kann man die profile direkt auswählen. Beim Handy long hold."): the profiles of
/// this browser with their „Mein Studiengang", the active one checked; a click makes one the
/// active profile (after the next frame, R21; the menu goes at once). Below them „Neues Profil"
/// (owner, 2026-10-19: „ein kleines Menü, wo man die profile als option hat, neues Profil anlegen
/// und dann halt profil Einstellungen, und vlt noch change lang"): a name in the menu, none
/// („Profil 3"), and the new profile, empty, is the active one. Then the way to the profile page
/// and the language (`languages::LanguageWays`), which lives in the profile since 2026-10-14. A
/// mouse resting on a way to the profile page (`ProfileButton`) opens it there; a right click or
/// the menu key on it does as well (as on a tab, `TabMenu`), and so do a tap and a finger held on
/// the phone's box (enhance.js, `betula:profile-menu`). The browser app's alone.
#[derive(Clone, Copy)]
pub struct ProfileMenu {
    at: RwSignal<Option<MenuAt>>,
    /// What a mouse's coming or going does a moment later (`HOVER_OPEN`, `HOVER_CLOSE`).
    timer: StoredValue<Option<TimeoutHandle>>,
    /// „Neues Profil" asks for a name.
    naming: RwSignal<bool>,
}

impl ProfileMenu {
    /// Creates the menu's place, closed, and provides it: call it once, in `App`.
    pub fn provide() -> Self {
        let menu = ProfileMenu { at: RwSignal::new(None), timer: StoredValue::new(None), naming: RwSignal::new(false) };
        provide_context(menu);
        menu
    }

    pub fn expect() -> Option<Self> {
        use_context::<Self>()
    }

    /// Drops what a mouse's coming or going was about to do.
    fn cancel(self) {
        if let Some(handle) = self.timer.get_value() {
            handle.clear();
        }
        self.timer.set_value(None);
    }

    fn close(self) {
        self.cancel();
        if self.at.with_untracked(Option::is_some) {
            self.at.set(None);
        }
        if self.naming.get_untracked() {
            self.naming.set(false);
        }
    }

    /// The menu stays until a choice, a tap beside it or Esc, as one a click opened (a name is
    /// being typed into it): a mouse leaving it takes it away no more.
    fn hold(self) {
        self.cancel();
        if self.at.with_untracked(|at| at.is_some_and(|at| at.hover)) {
            self.at.update(|at| {
                if let Some(at) = at {
                    at.hover = false;
                }
            });
        }
    }

    /// A mouse came onto a way to the profile page (`way`) or onto the menu: an open menu stays,
    /// and a closed one opens at the way a moment later (`HOVER_OPEN`), unless the mouse leaves
    /// before.
    fn pointer_on(self, ev: &leptos::ev::PointerEvent, way: bool) {
        if ev.pointer_type() != "mouse" {
            return;
        }
        self.cancel();
        if way && self.at.with_untracked(Option::is_none) {
            self.open_later(ev);
        }
    }

    /// Opens the menu at the way the mouse of `ev` came onto, a moment later (`HOVER_OPEN`).
    fn open_later(self, ev: &leptos::ev::PointerEvent) {
        #[cfg(feature = "csr")]
        {
            use leptos::wasm_bindgen::JsCast;
            let Some(way) = ev.current_target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok()) else { return };
            let open = move || {
                self.timer.set_value(None);
                self.open_at(&way, true);
            };
            self.timer.set_value(set_timeout_with_handle(open, HOVER_OPEN).ok());
        }
        #[cfg(not(feature = "csr"))]
        let _ = (ev, HOVER_OPEN);
    }

    /// A mouse left a way to the profile page or the menu: a menu the mouse opened goes a moment
    /// later (`HOVER_CLOSE`), unless the mouse comes back onto either.
    fn pointer_off(self, ev: &leptos::ev::PointerEvent) {
        if ev.pointer_type() != "mouse" {
            return;
        }
        self.cancel();
        if self.at.with_untracked(|at| at.is_some_and(|at| at.hover)) {
            let close = move || {
                self.timer.set_value(None);
                self.at.set(None);
            };
            self.timer.set_value(set_timeout_with_handle(close, HOVER_CLOSE).ok());
        }
    }

    /// Opens the menu at `way`, a way to the profile page.
    #[cfg(feature = "csr")]
    fn open_at(self, way: &leptos::web_sys::Element, hover: bool) {
        let rect = way.get_bounding_client_rect();
        let size = |value: Result<leptos::wasm_bindgen::JsValue, leptos::wasm_bindgen::JsValue>| value.ok().and_then(|value| value.as_f64()).unwrap_or(0.0);
        let (width, height) = leptos::web_sys::window().map_or((0.0, 0.0), |window| (size(window.inner_width()), size(window.inner_height())));
        let rail = way.closest(".rail").ok().flatten().is_some();
        self.cancel();
        self.naming.set(false);
        self.at.set(Some(MenuAt { rail, hover, bottom: rect.bottom(), right: rect.right(), width, height }));
    }
}

/// The focus back on the way to the profile page the menu stood at, where the focus was in the
/// menu (it goes with the menu).
fn focus_way(rail: bool) {
    #[cfg(feature = "csr")]
    {
        use leptos::wasm_bindgen::JsCast;
        let Some(document) = leptos::web_sys::window().and_then(|window| window.document()) else { return };
        let inside = document.active_element().is_some_and(|active| active.closest(".profile-menu").ok().flatten().is_some());
        let way = document.query_selector(if rail { ".rail .profile-btn" } else { ".topbar .top-profile" }).ok().flatten();
        if let Some(way) = way.filter(|_| inside).and_then(|way| way.dyn_into::<leptos::web_sys::HtmlElement>().ok()) {
            let _ = way.focus();
        }
    }
    #[cfg(not(feature = "csr"))]
    let _ = rail;
}

/// The ids the focus is moved to in the menu of the profiles: „Neues Profil", and its field.
const NEW_WAY_ID: &str = "pm-new";
const NEW_NAME_ID: &str = "pm-new-name";

/// The menu of the profiles where `ProfileMenu` opened it (see there).
#[component]
pub fn ProfileSwitcher() -> impl IntoView {
    let t = i18n::t();
    let menu = ProfileMenu::expect().filter(|_| APP);
    let profiles = Profiles::expect().filter(|_| APP);
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        use leptos::wasm_bindgen::JsCast;
        let Some(menu) = menu else { return };
        let open = window_event_listener_untyped("betula:profile-menu", move |ev| {
            if let Some(way) = ev.target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok()) {
                menu.open_at(&way, false);
            }
        });
        let keys = window_event_listener(leptos::ev::keydown, move |ev| {
            let Some(at) = menu.at.get_untracked() else { return };
            if ev.key() == "Escape" && !crate::nav::watched() {
                ev.prevent_default();
                // One Esc, one thing: the name asked for goes first, then the menu.
                if menu.naming.get_untracked() {
                    menu.naming.set(false);
                    request_animation_frame(|| nav::focus_by_id(NEW_WAY_ID));
                    return;
                }
                focus_way(at.rail);
                menu.close();
            }
        });
        on_cleanup(move || {
            open.remove();
            keys.remove();
        });
    });
    // What lies over the page: on Android Back closes it (`nav`).
    if let Some(menu) = menu {
        crate::nav::layer_while(move || menu.at.with(|at| at.is_some_and(|at| !at.hover)), move || menu.close(), crate::nav::Kind::Over);
    }
    // Another page: the menu goes.
    let location = use_location();
    Effect::new(move |before: Option<()>| {
        location.pathname.track();
        location.search.track();
        if let Some(menu) = menu.filter(|_| before.is_some()) {
            menu.close();
        }
    });
    // Opened by a click or a finger, the active profile has the focus: Tab goes on from there, Esc
    // closes the menu. A mouse resting on the way leaves the focus where it is.
    let checked = NodeRef::<leptos::html::Button>::new();
    Effect::new(move |_| {
        if let Some(checked) = checked.get() {
            if menu.is_some_and(|menu| menu.at.with_untracked(|at| at.is_some_and(|at| !at.hover))) {
                let _ = checked.focus();
            }
        }
    });
    // Where the menu stands, and whether it stays (a click opened it, or a name is being typed into
    // it): two memos, so that a menu a mouse opened stays as it is when it is held (`hold`).
    let placed = Memo::new(move |_| menu.and_then(|menu| menu.at.get()).map(|at| (at.rail, at.bottom, at.right, at.width, at.height)));
    let held = Memo::new(move |_| menu.is_some_and(|menu| menu.at.with(|at| at.is_some_and(|at| !at.hover))));
    let name = RwSignal::new(String::new());
    move || {
        let menu = menu?;
        let profiles = profiles?;
        let (rail, bottom, right, width, height) = placed.get()?;
        let active = profiles.active();
        let lines = profiles.program_lines(t.locale);
        let choose = move |id: u16| {
            focus_way(rail);
            menu.close();
            if id != profiles.active_untracked() {
                nav::after_paint(move || profiles.switch(id));
            }
        };
        let close = move |_| menu.close();
        // Beside the rail, its foot at the way's; below the phone's box, at its right edge.
        let place = if rail { format!("left: {:.0}px; bottom: {:.0}px", right + 8.0, (height - bottom).max(8.0)) } else { format!("top: {:.0}px; right: {:.0}px", bottom + 8.0, (width - right).max(8.0)) };
        let items = profiles
            .all()
            .into_iter()
            .map(|profile| {
                let id = profile.id;
                let on = id == active;
                let line = lines.iter().find(|(of, _)| *of == id).map(|(_, line)| line.clone()).unwrap_or_default();
                let node = if on { checked } else { NodeRef::new() };
                view! {
                    <button class="pm-item" type="button" role="radio" aria-checked=if on { "true" } else { "false" } node_ref=node on:click=move |_| choose(id)>
                        <Avatar profile=profile.clone()/>
                        <span class="pm-item-text">
                            <b>{profile.shown(t.locale)}</b>
                            <small>{line}</small>
                        </span>
                        <Icon name="check"/>
                    </button>
                }
            })
            .collect_view();
        // „Neues Profil": a name (none: „Profil 3"), and the new profile, empty, is the active one,
        // as on the profile page. Where there are as many profiles as there can be, it says so.
        let room = Memo::new(move |_| profiles.room());
        let next = Memo::new(move |_| profiles.next_id().map(|id| (folia_stores::i18n::texts(t.locale).profiles.unnamed)(id)).unwrap_or_default());
        let start = move |_| {
            if !room.get_untracked() {
                return;
            }
            menu.hold();
            name.set(String::new());
            menu.naming.set(true);
            request_animation_frame(|| nav::focus_by_id(NEW_NAME_ID));
        };
        let create = move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let typed = name.get_untracked();
            focus_way(rail);
            menu.close();
            // The menu goes at once; the profile, and every store reading its empty texts, after
            // the next frame (R21).
            nav::after_paint(move || {
                profiles.create(&typed);
            });
        };
        let new_profile = move || {
            if menu.naming.get() {
                view! {
                    <form class="pm-new" on:submit=create>
                        <input
                            id=NEW_NAME_ID
                            type="text"
                            maxlength=MAX_NAME.to_string()
                            aria-label=t.app.new_profile_name
                            placeholder=move || next.get()
                            autocomplete="off"
                            enterkeyhint="done"
                            prop:value=move || name.get()
                            on:input=move |ev| name.set(event_target_value(&ev))
                        />
                        <button class="mini primary hit" type="submit">{t.app.create_profile}</button>
                    </form>
                }
                .into_any()
            } else {
                view! {
                    <button class="tab-menu-way pm-new-way" type="button" id=NEW_WAY_ID aria-disabled=move || (!room.get()).then_some("true") on:click=start>
                        <Icon name="user-round-plus"/>
                        <span>{t.app.new_profile}{move || (!room.get()).then(|| view! { <small>{(t.app.profiles_full)(MAX_PROFILES)}</small> })}</span>
                    </button>
                }
                .into_any()
            }
        };
        Some(view! {
            {move || held.get().then(|| view! { <div class="tab-scrim" on:click=close on:contextmenu=move |ev: leptos::ev::MouseEvent| { ev.prevent_default(); menu.close() }></div> })}
            <div
                class="tab-menu profile-menu"
                class:at-rail=rail
                role="group"
                aria-label=t.app.switch_profile
                style=place
                on:pointerenter=move |ev| menu.pointer_on(&ev, false)
                on:pointerleave=move |ev| menu.pointer_off(&ev)
            >
                <p class="tab-menu-head">{t.app.profiles}</p>
                <div class="pm-list" role="radiogroup" aria-label=t.app.profiles>{items}</div>
                {new_profile}
                <hr class="tab-menu-rule"/>
                <a class="tab-menu-way" href=t.path(url::PROFILE) rel="nofollow" on:click=close><Icon name="sliders-horizontal"/><span>{t.app.profile_and_settings}</span></a>
                <languages::LanguageWays/>
            </div>
        })
    }
}

/// A way back at the head of a phone's page, left of the search (owner, 2026-10-05: „wenn man in
/// der semester ansicht ist, soll es oben links neben der search bar im gleichen style eine
/// quadratische box sein mit einem zurück pfeil"): a box as the search is and as wide as it is
/// high, an arrow in it. While it is there, the way to the profile page at the right of the search
/// gives way (owner, 2026-10-15: „wenn der angezeigt wird, dann soll die profilauswahl ausgeblendet
/// werden. Mach das mal als wiederverwendbares system"); the two slide out and in, and the search
/// makes room (app.css). A wide screen has neither.
///
/// Every page's „Zurück" is that box on a phone (owner, 2026-10-15: „mach den zurückbutton mal
/// überall nach oben neben die suche, wo aktuell noch so einer gelisted ist"): a page's
/// `frame::BackLink` puts it there, leading where the link leads, and steps aside itself (app.css).
/// A page with a step below its first one that has no such link (a semester of „Mein Studium", on
/// either side of its overview) puts it there while that step shows and takes it away when it goes;
/// both through `follow`, the browser app's alone. The server writes the bar before the page and
/// knows nothing of its links, so the address says which pages have the box from their first paint
/// (`back_at_head`): the server writes it as the app has it, and nothing moves at the takeover.
/// The page hears it pressed as well (`presses`): where it leads where the app is already (the
/// page's step not in the address yet), the page goes back itself.
#[derive(Clone, Copy)]
pub struct TopBack {
    /// Where the box leads, and which page's `follow` put it there.
    back: RwSignal<Option<(u64, Back)>>,
    /// The last of those numbers given out.
    given: StoredValue<u64>,
    pressed: RwSignal<u32>,
}

/// Where the way back at the head of a phone's page leads by the page's address (`back_at_head`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Up {
    /// The list of the area as it was left (`Tabs::list`).
    List(Area),
    /// „Mein Studium" as it was left (`Tabs::study`), up from the program overview its „Alle
    /// Studiengänge" leads to (owner, 2026-10-17: „wenn man auf alle Studiengänge geht, dann fehlt
    /// da ein zurück button, um zur mein Studium ansicht zurück zu kommen"). The browser app's
    /// alone, as „Mein Studium" is: without JavaScript the overview is where „Studium" starts, and
    /// its bar has no way back (app.css).
    Study,
    /// Where the visitor was before, from a page of a while (`nav::passing`: the profile page; owner,
    /// 2026-10-19: „dass die Profile page, als temporäre seite getreated werden soll wo man dann
    /// oben den zurück pfeil hat und genau wieder da landet, wo man davor war"): back through the
    /// history, to that page as it was left; Start where nothing is known (a reload, a link from
    /// outside, the server's page).
    Before,
}

/// The pages that have their way back at the head of a phone's page from their first paint
/// (`TopBack`), by their address, and where it leads until the page says where (the server's
/// page, where it is the link of its „Zurück" as well): a program's page — with whatever fills it,
/// which has a way back of its own — and a module's own page up to the list of their area, the
/// program overview up to „Mein Studium", the profile page to where the visitor was before. A page
/// with a `BackLink` that the server renders is named here, or the bar changes when the app takes
/// over.
pub fn back_at_head(path: &str) -> Option<Up> {
    if path == url::PROGRAMS {
        Some(Up::Study)
    } else if crate::nav::passing(path) {
        Some(Up::Before)
    } else if crate::tabs::page_below(path, url::PROGRAMS).is_some() {
        Some(Up::List(Area::Programs))
    } else if crate::tabs::page_below(path, &format!("{}/module", url::CATALOG)).is_some() {
        Some(Up::List(Area::Catalog))
    } else {
        None
    }
}

/// Where the box at the head leads (`TopBack`).
#[derive(Clone, Debug, PartialEq)]
pub struct Back {
    /// The path, without the language's prefix.
    pub href: String,
    /// What it is called (the arrow's words for those who do not see it).
    pub label: &'static str,
    /// The step before in the history is where it leads: it goes back there instead, the same
    /// entry as before, and the history does not grow.
    pub history: bool,
}

impl TopBack {
    /// Creates the box's place, empty, and provides it: call it once, in `App`.
    pub fn provide() -> Self {
        let back = TopBack { back: RwSignal::new(None), given: StoredValue::new(0), pressed: RwSignal::new(0) };
        provide_context(back);
        back
    }

    pub fn expect() -> Option<Self> {
        use_context::<Self>()
    }

    /// Shows the box where a page (its `BackLink`, its step) wants it, leading where `back` says
    /// whenever that changes, and takes it away with the page. What another page put there in the
    /// meantime stays: the page the app goes to may say where before the one it leaves is gone.
    pub fn follow(back: impl Fn() -> Option<Back> + 'static) {
        let Some(top) = Self::expect() else { return };
        let me = top.given.get_value().wrapping_add(1);
        top.given.set_value(me);
        Effect::new(move |_| top.put(me, back()));
        on_cleanup(move || top.put(me, None));
    }

    /// `me`'s way back, or none where `me` has put the box there.
    fn put(self, me: u64, back: Option<Back>) {
        let next = match back {
            Some(back) => Some((me, back)),
            None if self.back.with_untracked(|now| now.as_ref().is_some_and(|(owner, _)| *owner == me)) => None,
            None => return,
        };
        if self.back.with_untracked(|now| *now != next) {
            self.back.set(next);
        }
    }

    /// How often the box was pressed. Tracked.
    pub fn presses(self) -> u32 {
        self.pressed.get()
    }
}

/// Where the box of `TopBack` leads on the page the app is at or going to (`pending`): where the
/// page puts it (its `BackLink`, its step), else where its address puts it (`back_at_head`) — the
/// list of its area as it was left (`Tabs::list`), „Mein Studium" as it was left, or, from a page
/// of a while, the page before it, through the history where that is the step before, as
/// `BackLink` does. On the way to another page (another path) the page that is left has no say: the
/// bar follows the click, the box there or gone at once.
fn top_back_shown() -> Memo<Option<Back>> {
    let t = i18n::t();
    let location = use_location();
    let going = Pending::expect();
    let tabs = Tabs::expect();
    let back = TopBack::expect();
    Memo::new(move |_| {
        let headed = going.and_then(|going| going.path());
        let elsewhere = headed.as_ref().is_some_and(|path| *path != location.pathname.get());
        if let Some((_, step)) = back.and_then(|back| back.back.get()).filter(|_| !elsewhere) {
            return Some(step);
        }
        let path = headed.unwrap_or_else(|| location.pathname.get());
        let now = crate::tabs::location_of(&location.pathname.get(), &location.search.get());
        let href = match back_at_head(&path)? {
            Up::List(area) => tabs.map_or_else(|| area.back_root().to_string(), |tabs| tabs.list(area)),
            Up::Study => tabs.map_or_else(|| url::STUDY.to_string(), Tabs::study),
            // On the way there the page before it is the one the app is at.
            Up::Before => Some(if elsewhere { now.clone() } else { tabs.map(|tabs| tabs.before(&now)).unwrap_or_default() })
                .filter(|before| before.starts_with('/') && !crate::nav::passing(before.split(['?', '#']).next().unwrap_or(before)))
                .unwrap_or_else(|| url::HOME.to_string()),
        };
        let history = tabs.is_some_and(|tabs| tabs.before(&now) == href);
        Some(Back { href, label: t.common.back, history })
    })
}

/// The box of `TopBack`: there while a page or its address puts it there (`shown`). Where it went,
/// it keeps its place and its arrow and slides out, out of reach.
#[component]
fn TopBackBox(shown: Memo<Option<Back>>) -> impl IntoView {
    let t = i18n::t();
    let location = use_location();
    let back = TopBack::expect();
    let last = Memo::new(move |before: Option<&Option<Back>>| shown.get().or_else(|| before.cloned().flatten()));
    let on = move || shown.with(Option::is_some);
    // The arrow is Back (owner, 2026-10-18: „wenn oben links ein back pfeil ist das immer gleich
    // sein, wie die handy back option"): what lies over the page first, else back through the
    // history where the step before is where it leads, as the browser says it at the moment of
    // the press (`nav::came_from`); else its link leads up.
    let click = move |ev: leptos::ev::MouseEvent| {
        if ev.button() != 0 || ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key() {
            return;
        }
        let Some((href, guess)) = shown.with_untracked(|back| back.as_ref().map(|back| (back.href.clone(), back.history))) else { return };
        // The first use of a page come to from outside lays what lies above it under it.
        crate::nav::seed();
        if crate::nav::arrow(crate::nav::came_from(&href, || guess)) {
            ev.prevent_default();
        } else if let Some(back) = back {
            // The link leads on; where it leads where the app is already, the page hears the press
            // and goes back itself (`presses`). Not where the arrow closed something or went back:
            // one press, one step.
            back.pressed.update(|presses| *presses = presses.wrapping_add(1));
        }
    };
    view! {
        <a
            class="top-back"
            class:on=on
            inert=move || !on()
            href=move || last.with(|back| back.as_ref().map(|back| t.path(&back.href)))
            // „Mein Studium", where the program overview's leads, is no page for search engines.
            rel=move || last.with(|back| back.as_ref().and_then(|back| crate::seo::nofollow(&back.href)))
            aria-label=move || last.with(|back| back.as_ref().map(|back| back.label))
            title=move || last.with(|back| back.as_ref().map(|back| back.label))
            // A step of the page stays where the page is; the way to another page goes to its top
            // or to where it was left, as `BackLink` does.
            data-noscroll=move || last.with(|back| back.as_ref().is_some_and(|back| back.href.split(['?', '#']).next() == Some(location.pathname.get().as_str()))).then_some("")
            on:click=click
        >
            <Icon name="arrow-left"/>
        </a>
    }
}

#[component]
pub fn TopBar() -> impl IntoView {
    let t = i18n::t();
    let location = use_location();
    // Title and search belong to the page the app is at or going to (`pending`).
    let going = Pending::expect();
    let path_now = Memo::new(move |_| going.and_then(|p| p.path()).unwrap_or_else(|| location.pathname.get()));
    let area_now = Memo::new(move |_| Area::of(&path_now.get()));
    // „Mein Studium" is the visitor's study, not a list of programs: its search is the modules'.
    let study_now = Memo::new(move |_| path_now.get() == url::STUDY);
    // The profile page is no area's, and names itself (the start page's area writes the wordmark).
    let profile_now = Memo::new(move |_| path_now.get() == url::PROFILE);
    let pending = StoredValue::new(None::<TimeoutHandle>);
    // The way back at the head, and the way to the profile page giving way to it (`TopBack`).
    let back = top_back_shown();
    let backed = Signal::derive(move || back.with(Option::is_some));
    // The program overview's way back leads to „Mein Studium", the browser app's alone (`Up::Study`):
    // the server writes the bar as the app has it, for the app's first paint, and without
    // JavaScript the box is not there and the way to the profile page stays in reach (app.css).
    let app_back = Memo::new(move |_| back_at_head(&path_now.get()) == Some(Up::Study));
    let away = Signal::derive(move || backed.get() && (APP || !app_back.get()));

    // The search belongs to the page: programs on the program overview, the marked modules on the
    // Merkliste, modules everywhere else. In the browser app it filters while typing (replacing the
    // history entry, not adding one). On the Merkliste what is typed stays out of the address: it
    // may name what only a note of the visitor's says (`search::LocalSearch`, R20).
    let local = crate::search::LocalSearch::expect();
    let on_input = move |ev: leptos::ev::Event| {
        let text = event_target_value(&ev);
        if let Some(handle) = pending.get_value() {
            handle.clear();
        }
        if let (Area::Bookmarks, Some(local)) = (area_now.get_untracked(), local) {
            pending.set_value(set_timeout_with_handle(move || local.set(text.clone()), std::time::Duration::from_millis(140)).ok());
            return;
        }
        // The list of a text typed before is not built any more once it is worked out: this key's
        // takes its place (`Pending::typed`).
        if let Some(going) = going {
            going.typed();
        }
        let run = move || {
            // On top of where the visitor is headed: a filter clicked a moment ago stays.
            let (path, search) = Pending::shown_of(going, location.pathname, location.search);
            let target = if area_now.get_untracked() == Area::Programs && !study_now.get_untracked() {
                let mut next = if path == url::PROGRAMS { url::ProgramsUrl::parse(&search) } else { Default::default() };
                next.text = text.trim().to_string();
                next.path()
            } else {
                let mut next = if path == url::CATALOG { url::CatalogUrl::parse(&search) } else { Default::default() };
                next.query.text = text.clone();
                next.page = 1;
                // What is typed is searched for the best matches first: an order chosen before
                // gives way to relevance (owner, 2026-09-30), and a column orders the matches again.
                next.query.sort = folia_routes::filter::SortKey::Default;
                next.query.descending = false;
                next.path()
            };
            // The first key on a page is a step of its own, so that Back leads to the page as it
            // was before the search; the keys after it take its place. On a phone the search of the
            // page it is on takes its place from the first key (folia/design/zurueck/konzept.html).
            let typed_before = url::parse_pairs(&search).iter().any(|(key, value)| key == "q" && !value.trim().is_empty());
            let replace = target.split('?').next() == Some(path.as_str()) && (typed_before || crate::nav::phone());
            if let Some(going) = going {
                going.go_quietly(&target, NavigateOptions { replace, scroll: false, ..Default::default() });
            }
        };
        pending.set_value(set_timeout_with_handle(run, std::time::Duration::from_millis(140)).ok());
    };

    view! {
        <header class="topbar" class:backed=backed class:app-back=app_back>
            <TopBackBox shown=back/>
            {move || {
                let study = study_now.get();
                let programs = area_now.get() == Area::Programs && !study;
                // The Merkliste searches itself in the app; the server writes its explanation.
                let marked = APP && area_now.get() == Area::Bookmarks && local.is_some();
                let modules = t.app.search_modules_placeholder;
                let (title, action, placeholder) = match area_now.get() {
                    Area::Programs if study => (t.app.my_studies, url::CATALOG, modules),
                    Area::Programs => (t.app.programs, url::PROGRAMS, t.app.search_programs_placeholder),
                    Area::Catalog => (t.app.modules, url::CATALOG, modules),
                    // The form's action is the catalog's all the same: without the app, Enter
                    // searches there.
                    Area::Bookmarks if marked => (t.app.bookmarks, url::CATALOG, t.app.search_bookmarks_placeholder),
                    Area::Bookmarks => (t.app.bookmarks, url::CATALOG, modules),
                    Area::Studyplan => (t.app.studyplan, url::CATALOG, modules),
                    Area::Home => (t.app.home, url::CATALOG, modules),
                };
                let initial = match (marked, local) {
                    (true, Some(local)) => local.text_untracked(),
                    _ => url::parse_pairs(&Pending::shown_of(going, location.pathname, location.search).1)
                        .into_iter()
                        .find(|(key, _)| key == "q")
                        .map(|(_, value)| value)
                        .unwrap_or_default(),
                };
                let label = match (programs, marked) {
                    (true, _) => t.app.search_programs,
                    (false, true) => t.app.search_bookmarks,
                    (false, false) => t.app.search_modules,
                };
                let title = if profile_now.get() { t.app.profile } else { title };
                // The start page carries the name: next to the mark in the rail it reads as the logo.
                let home = area_now.get() == Area::Home && !profile_now.get();
                let heading = if home {
                    view! { <h1><ui::Wordmark small=true/></h1><small>{t.common.tagline}</small> }.into_any()
                } else {
                    view! { <h1>{title}</h1> }.into_any()
                };
                view! {
                    <div class="crumb" class:brand=home>{heading}</div>
                    <form class="search" role="search" method="get" action=t.path(action) data-live-search="" data-region="search">
                        <Icon name="search"/>
                        <label class="visually-hidden" for="topsearch">{label}</label>
                        <input id="topsearch" type="search" name="q" value=initial placeholder=placeholder autocomplete="off" on:input=on_input data-command=t.commands.search_field data-command-group="go" data-command-keys=folia_design::keys::SEARCH/>
                        <ui::Shortcut keys=folia_design::keys::SEARCH/>
                    </form>
                    // „Befehle" beside the search, a box of its own as tall as the search's (owner,
                    // 2026-10-26: „Mach die searchbar auf die länge, die sie vor der Befehlspalette
                    // hatte, lasse diese aber gleich groß"): in the search it made the bar look long
                    // and lopsided.
                    <crate::commands::CommandsButton/>
                }
            }}
            <ProfileButton top=true away/>
        </header>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program's page and a module's own page have their way back at the head from their first
    /// paint, by their address, up to their area's list, the program overview up to „Mein
    /// Studium", and the profile page to where the visitor was before; the other lists, „Mein
    /// Studium" and the other pages do not.
    #[test]
    fn the_pages_with_a_way_back_have_it_at_the_head() {
        for path in ["/programs/bachelor-informatik-2008", "/programs/bachelor-informatik-2008/areas", "/programs/bachelor-informatik-2008/plan"] {
            assert_eq!(back_at_head(path), Some(Up::List(Area::Programs)), "{path}");
        }
        assert_eq!(back_at_head("/catalog/module/11103"), Some(Up::List(Area::Catalog)));
        assert_eq!(back_at_head("/programs"), Some(Up::Study));
        assert_eq!(back_at_head("/profile"), Some(Up::Before));
        for path in ["/programs/", "/study", "/catalog", "/catalog/module", "/catalog/module/", "/bookmarks", "/studyplan", "/profile/x", "/", "/impressum"] {
            assert_eq!(back_at_head(path), None, "{path}");
        }
    }
}
