//! The overview of the keys (folia/design/tastatur/konzept.html, step 4 „Hilfe": „Der Knopf
//! „Tasten" und ? mit den Tasten des Ortes, … der Schalter für die einzelnen Tasten"): a dialog
//! with what the keys do everywhere and, on a page that has more, at its places (the catalog: the
//! list, the filters, the module beside it; the Merkliste: its list, its collections and filters), the group of where the keyboard was first and marked
//! „Hier"; under them the switch for Betula's single keys (`folia_design::keys::LettersSwitch`).
//!
//! The button that opens it stands in the rail (`KeysButton`; no action is a key's alone, CLAUDE.md
//! „Design"); `?` presses it (enhance.js, „the keyboard"), which hands over where the keyboard was
//! (`data-from`: `side`, `list`, `module`, `nav`, `search`). A page that gets keys of its own adds
//! its groups to `groups`.

use leptos::prelude::*;

use folia_design::keys::{Keys, LettersSwitch};
use folia_design::ui::{DialogClose, Icon};

use crate::i18n::{self, keys::Row, Texts};
use crate::tabs::Area;

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
            _ => None,
        }
    }
}

/// The groups of a page, its own after those of everywhere.
fn groups(area: Area) -> Vec<Group> {
    match area {
        Area::Catalog => vec![Group::Everywhere, Group::List, Group::Filters, Group::Module],
        Area::Bookmarks => vec![Group::Everywhere, Group::Marked, Group::Sidebar, Group::Module],
        _ => vec![Group::Everywhere],
    }
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
/// a phone), Esc and a click beside it close it, and the focus goes back where it was.
#[component]
pub fn KeysHelp() -> impl IntoView {
    let t = i18n::t();
    let Some(KeysOpen(open)) = KeysOpen::expect() else { return ().into_any() };
    let node = NodeRef::<leptos::html::Dialog>::new();
    let location = folia_design::i18n::router_location();
    Effect::new(move |_| {
        let shown = open.with(Option::is_some);
        let Some(element) = node.get() else { return };
        if shown && !element.open() {
            let _ = element.show_modal();
        } else if !shown && element.open() {
            element.close();
        }
    });
    let close = move || open.set(None);
    let body = move || {
        open.get().map(|from| {
            let area = Area::of(&location.pathname.get_untracked());
            let here = Group::of(&from, area);
            let mut shown = groups(area);
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
        <dialog
            class="st-dialog keys-dialog"
            node_ref=node
            data-sheet=""
            aria-labelledby="keys-title"
            on:close=move |_| close()
            // A click on the dim backdrop is a click on the dialog itself.
            on:click=move |ev: leptos::ev::MouseEvent| {
                let on_backdrop = ev.target().and_then(|target| node.get_untracked().map(|dialog| {
                    let dialog: &leptos::web_sys::EventTarget = dialog.as_ref();
                    dialog == &target
                }));
                if on_backdrop == Some(true) {
                    close();
                }
            }
        >
            <div class="st-dlg">
                <header class="st-dlg-head">
                    <span class="st-grip" aria-hidden="true"></span>
                    <div>
                        <h2 id="keys-title">{t.keys.title}</h2>
                        <p>{t.keys.lead}</p>
                    </div>
                    <DialogClose on_close=Callback::new(move |()| close())/>
                </header>
                <div class="st-dlg-body keys-body">
                    <div class="keys-groups">{body}</div>
                    <div class="keys-letters">
                        <p class="label" id="keys-letters">{t.ui.letters}</p>
                        <LettersSwitch labelled_by="keys-letters"/>
                        <p class="keys-letters-hint">{t.ui.letters_hint}</p>
                    </div>
                </div>
            </div>
        </dialog>
    }
    .into_any()
}
