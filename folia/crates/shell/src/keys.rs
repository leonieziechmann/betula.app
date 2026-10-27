//! The overview of the keys (folia/design/tastatur/konzept.html, step 4 „Hilfe": „Der Knopf
//! „Tasten" und ? mit den Tasten des Ortes, … der Schalter für die einzelnen Tasten"): a dialog
//! with what the keys do everywhere and, on a page that has more, at its places (the catalog: the
//! list, the filters, the module beside it; the Merkliste: its list, its collections and filters;
//! a Termin in its own view, wherever it stands), the group of where the keyboard was first and
//! marked „Hier"; under them the switch for Betula's single keys
//! (`folia_design::keys::LettersSwitch`).
//!
//! The button that opens it stands in the rail (`KeysButton`; no action is a key's alone, CLAUDE.md
//! „Design"); `?` presses it (enhance.js, „the keyboard"), which hands over where the keyboard was
//! (`data-from`: `side`, `list`, `module`, `nav`, `search`). A page that gets keys of its own adds
//! its groups to `groups`.

use leptos::prelude::*;

use folia_design::keys::{Keys, LettersSwitch};
use folia_design::modal::{kept, Modal, ModalHead};
use folia_design::ui::Icon;

use crate::i18n::{self, keys::Row, Texts};
use crate::tabs::Area;
use folia_routes::url;

/// Whether the overview is open, and from where (`data-from` of the button: a place of
/// `Place::of`, or nothing for a click).
#[derive(Clone, Copy)]
pub struct KeysOpen(RwSignal<Option<String>>);

impl KeysOpen {
    pub fn provide() {
        provide_context(Self(RwSignal::new(None)));
    }

    pub fn expect() -> Option<Self> {
        use_context::<Self>()
    }
}

/// A group of the overview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    Everywhere,
    List,
    Filters,
    Module,
    Marked,
    Sidebar,
    Termin,
}

impl Group {
    fn title(self, t: &'static Texts) -> &'static str {
        match self {
            Group::Everywhere => t.keys.everywhere,
            Group::List => t.keys.list,
            Group::Filters => t.keys.filters,
            Group::Module => t.keys.module,
            Group::Marked => t.keys.marked,
            Group::Sidebar => t.keys.sidebar,
            Group::Termin => t.keys.termin,
        }
    }

    fn rows(self, t: &'static Texts) -> &'static [Row] {
        match self {
            Group::Everywhere => t.keys.everywhere_rows,
            Group::List => t.keys.list_rows,
            Group::Filters => t.keys.filters_rows,
            Group::Module => t.keys.module_rows,
            Group::Marked => t.keys.marked_rows,
            Group::Sidebar => t.keys.sidebar_rows,
            Group::Termin => t.keys.termin_rows,
        }
    }

    /// The group of a place the keyboard was at (`data-from`) on a page of `area`.
    fn of(place: &str, area: Area) -> Option<Self> {
        match (place, area) {
            ("side", Area::Bookmarks) => Some(Group::Sidebar),
            ("list", Area::Bookmarks) => Some(Group::Marked),
            ("side", _) => Some(Group::Filters),
            ("list", _) => Some(Group::List),
            ("module", _) => Some(Group::Module),
            ("termin", _) => Some(Group::Termin),
            _ => None,
        }
    }
}

/// The groups of a page, its own after those of everywhere; a Termin's where one stands in its
/// own view (`termin`: beside the page, filling it, or as its own page).
fn groups(area: Area, termin: bool) -> Vec<Group> {
    let mut groups = match area {
        Area::Catalog => vec![Group::Everywhere, Group::List, Group::Filters, Group::Module],
        Area::Bookmarks => vec![Group::Everywhere, Group::Marked, Group::Sidebar, Group::Module],
        _ => vec![Group::Everywhere],
    };
    if termin {
        groups.push(Group::Termin);
    }
    groups
}

/// A Termin stands in its own view at this address: its own page, or `termin=` over a page.
fn termin_at(path: &str, search: &str) -> bool {
    url::event_of(path).is_some() || url::parse_pairs(search).iter().any(|(key, value)| key == "termin" && !value.trim().is_empty())
}

/// The key caps of a row: its keys as alternatives; `Nav` the navigation's, written by the
/// stylesheet as the system has them (`--nav-keys`, enhance.js) before the digits.
fn caps(keys: &'static [&'static str]) -> AnyView {
    keys.iter()
        .enumerate()
        .map(|(i, key)| {
            let cap = if *key == "Nav" { view! { <kbd class="nav-combo">"1–5"</kbd> }.into_any() } else { view! { <Keys spec=*key/> }.into_any() };
            view! { {(i > 0).then(|| view! { <i class="alt">"·"</i> })}{cap} }
        })
        .collect_view()
        .into_any()
}

/// The button in the rail that opens the overview: the keyboard's icon, and `?` beside it while
/// the navigation's keys are held, as the tabs show their digits (app.css `.hold-key`). The browser
/// app's alone (R15).
#[component]
pub fn KeysButton() -> impl IntoView {
    let t = i18n::t();
    let open = KeysOpen::expect();
    let on_click = move |ev: leptos::ev::MouseEvent| {
        use leptos::wasm_bindgen::JsCast;
        // Where the keyboard was, as `?` says it; a click says nothing.
        let from = ev.current_target().and_then(|target| target.dyn_into::<leptos::web_sys::Element>().ok()).and_then(|button| {
            let from = button.get_attribute("data-from");
            let _ = button.remove_attribute("data-from");
            from
        });
        if let Some(KeysOpen(open)) = open {
            open.set(Some(from.unwrap_or_default()));
        }
    };
    view! {
        <button class="icon-btn keys-btn js-only" type="button" data-action="keys" aria-keyshortcuts="?" aria-label=t.keys.button title=t.keys.button_title data-command=t.keys.all data-command-group="help" data-command-keys="?" on:click=on_click>
            <Icon name="keyboard"/><kbd class="hold-key k-letter" aria-hidden="true">"?"</kbd>
        </button>
    }
}

/// The overview, once in the app: a dialog shown modally (in the middle of the window, a sheet on
/// a phone; `folia_design::modal`), Esc and a click beside it close it, and the focus goes back
/// where it was.
#[component]
pub fn KeysHelp() -> impl IntoView {
    let t = i18n::t();
    let Some(KeysOpen(open)) = KeysOpen::expect() else { return ().into_any() };
    let location = folia_design::i18n::router_location();
    // Where the keyboard was when it opened; kept while it goes.
    let from = kept(move || open.get());
    let close = move || open.set(None);
    let body = move || {
        from.get().map(|from| {
            let path = location.pathname.get_untracked();
            let area = Area::of(&path);
            let here = Group::of(&from, area);
            let mut shown = groups(area, termin_at(&path, &location.search.get_untracked()));
            // Where the keyboard was comes first.
            if let Some(here) = here.filter(|here| shown.contains(here)) {
                shown.retain(|group| *group != here);
                shown.insert(0, here);
            }
            shown
                .into_iter()
                .map(|group| {
                    let is_here = here == Some(group);
                    view! {
                        <section class="keys-group" class:here=is_here>
                            <h3>{group.title(t)}{is_here.then(|| view! { <span class="keys-here">{t.keys.here}</span> })}</h3>
                            <dl>
                                {group.rows(t).iter().map(|(keys, words)| {
                                    // A row of single keys alone goes with them when they are off.
                                    let letters = keys.iter().all(|key| folia_design::keys::is_letter(key));
                                    view! { <div class="keys-row" class:k-letter=letters><dt>{caps(keys)}</dt><dd>{*words}</dd></div> }
                                }).collect_view()}
                            </dl>
                        </section>
                    }
                })
                .collect_view()
        })
    };
    view! {
        <Modal open=Signal::derive(move || open.with(Option::is_some)) on_close=close class="keys-dialog" labelled_by="keys-title">
            <ModalHead id="keys-title" title=t.keys.title on_close=close>
                <p>{t.keys.lead}</p>
            </ModalHead>
            <div class="st-dlg-body keys-body">
                <div class="keys-groups">{body}</div>
                <div class="keys-letters">
                    <p class="label" id="keys-letters">{t.ui.letters}</p>
                    <LettersSwitch labelled_by="keys-letters"/>
                    <p class="keys-letters-hint">{t.ui.letters_hint}</p>
                </div>
            </div>
        </Modal>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a Termin stands in its own view, the overview has its keys too.
    #[test]
    fn a_termin_brings_its_keys() {
        assert!(termin_at("/event/148370-eb017/2026-11-02", ""));
        assert!(termin_at("/studyplan", "?termin=148370-eb017&on=2026-11-02&full=1"));
        assert!(termin_at("/", "?termin=148370-eb017"));
        assert!(!termin_at("/studyplan", "?open=12104") && !termin_at("/profile", "") && !termin_at("/", "?termin="));
        assert_eq!(groups(Area::Studyplan, true), [Group::Everywhere, Group::Termin]);
        assert_eq!(groups(Area::Catalog, false), [Group::Everywhere, Group::List, Group::Filters, Group::Module]);
        assert_eq!(Group::of("termin", Area::Home), Some(Group::Termin));
    }
}
