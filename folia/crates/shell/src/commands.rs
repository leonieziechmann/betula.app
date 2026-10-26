//! „Befehle" (folia/design/tastatur/konzept.html, „Befehle: Betula sagen, was zu tun ist"; owner,
//! 2026-10-20: „mach mal als erstes die command palette, ich glaube das ist am relvantesten"): the
//! button beside the search, or Strg + K (⌘K), opens a field in which one writes what Betula is
//! to do. It finds what the page offers (the module the keyboard is at: „Merken", „Einplanen",
//! „Vollbild"), every filter, the pages, the settings, and every module by its title or number; Enter
//! does it, Shift + Enter does the other thing a command has (a filter „ohne", a module in full).
//! Every row shows its key, and a command run by its name says afterwards that its key does it too
//! („Geht auch mit M"): so the keys one needs are learnt on the way.
//!
//! A page names its commands in attributes, as it names its parts for the keyboard (enhance.js,
//! R24), and the palette reads them from what is shown when it opens (`page::collect`); a command is
//! the button it names, so it does what a click does (no action is a key's alone, CLAUDE.md
//! „Design"):
//!
//! - `data-command` makes an element a command while it is shown (in a closed `<details>` as well):
//!   its value is its name, or empty for the element's own (its `aria-label`, else its text).
//! - `data-command-keys` its keys (a spec of `folia_design::keys`), `data-command-state` what it is
//!   now („mit"), `data-command-icon` an icon of the sprite where not its own.
//! - `data-command-by="event"`: the palette tells the element (`betula-command`, with Shift
//!   `betula-command-alt`) instead of clicking it, for a control that does more than one thing (a
//!   filter's chip: mit, ohne); `data-command-act="<action> <value>"`: what enhance.js does
//!   (`betula:act`: `go side`, the filters as F does).
//! - `data-commands` on a container names its commands' group („Filter", „Für „Analysis I“"); `go`,
//!   `settings` and `help` are the palette's own groups, and `data-command-group` on an element
//!   puts it into one. `data-commands-when="focus"`: only while the keyboard was in it (a row of a
//!   list). `data-commands-typed`: found by typing only (the filters, which are many).
//! - `data-command-of` on a container names what its commands' names are of: „Prüfung: Klausur".
//!
//! The palette has the settings itself (light, dark, the other language, the single keys) and asks
//! the data worker for the modules (`folia_pages::ask::CommandModulesAsk`, the catalog's search).

// What the palette reads and does is the browser's; the server renders its dialog empty.
#![cfg_attr(not(feature = "csr"), allow(dead_code))]

use leptos::prelude::*;

use folia_design::keys::{Keys, COMMANDS};
use folia_design::ui::Icon;

use crate::i18n::{self, Texts};

/// Whether the palette is open: the button in the search field opens it, and Strg + K presses the
/// button (enhance.js).
#[derive(Clone, Copy)]
pub struct CommandsOpen(RwSignal<bool>);

impl CommandsOpen {
    pub fn provide() {
        provide_context(Self(RwSignal::new(false)));
    }

    pub fn expect() -> Option<Self> {
        use_context::<Self>()
    }
}

/// Where a group stands: a page's groups in the order of the page, „Gehe zu", the page's groups
/// found by typing only (the filters), the settings, the help, and last the modules, which the
/// search finds for nearly any text: a command that is named comes before them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Place {
    Page(usize),
    Go,
    Typed(usize),
    Settings,
    Help,
    Modules,
}

/// What running a command does.
#[derive(Clone, Debug, PartialEq)]
enum Run {
    /// An element of the page (its index among those collected): clicked, focused (a field) or
    /// told (`betula-command`).
    Element(usize, How),
    /// What enhance.js does for a switch (`betula:act`): its action and value.
    Act(String, String),
    /// A module found: beside the catalog's list, with Shift its page.
    Module(String),
    /// An address loaded anew (the page in another language).
    Address(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum How {
    Click,
    Focus,
    Tell,
}

/// A command as the palette lists it.
#[derive(Clone, Debug, PartialEq)]
struct Item {
    place: Place,
    heading: String,
    label: String,
    sub: Option<String>,
    keys: Option<String>,
    /// The address of an icon of the sprite (`/assets/icons.svg?v=…#bookmark`).
    icon: Option<String>,
    /// What it is found by, folded (`folia_search::fold`): its name, state, heading and words.
    words: String,
    typed_only: bool,
    run: Run,
}

impl Item {
    fn new(place: Place, heading: &str, label: String, run: Run) -> Self {
        Self { place, heading: heading.to_string(), label, sub: None, keys: None, icon: None, words: String::new(), typed_only: false, run }
    }

    /// Its words, folded, once its parts are set.
    fn found_by(mut self, more: &str) -> Self {
        self.words = folia_search::fold(&format!("{} {} {} {more}", self.label, self.sub.as_deref().unwrap_or(""), self.heading));
        self
    }
}

/// The items to show for `text`, in their order: without a text what is not found by typing only
/// (the filters, the modules, the settings wait for it); with one those all of whose words they
/// hold, each group in its place and in it a name that begins with the text first, then a name with
/// a word that begins with its first word, then the rest. The modules are the search's answer to
/// the text already, and come as it ordered them.
fn shown(items: &[Item], text: &str) -> Vec<usize> {
    let folded = folia_search::fold(text.trim());
    let tokens: Vec<&str> = folded.split_whitespace().collect();
    let mut found: Vec<(Place, u8, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            if tokens.is_empty() {
                let waits = item.typed_only || matches!(item.place, Place::Modules | Place::Settings);
                return (!waits).then_some((item.place, 0, i));
            }
            // The modules are what the catalog's search found for the text, typos forgiven: in its
            // order.
            if item.place == Place::Modules {
                return Some((item.place, 0, i));
            }
            if !tokens.iter().all(|token| item.words.contains(token)) {
                return None;
            }
            let label = folia_search::fold(&item.label);
            let first = tokens.first().copied().unwrap_or_default();
            let rank = if label.starts_with(folded.as_str()) {
                0
            } else if label.split(|c: char| !c.is_alphanumeric()).any(|word| word.starts_with(first)) {
                1
            } else {
                2
            };
            Some((item.place, rank, i))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(.., i)| i).take(80).collect()
}

/// The group a command's `data-commands` (or `data-command-group`) names, and its heading.
fn group(name: &str, first: usize, typed: bool, t: &'static Texts) -> (Place, String) {
    match name {
        "go" => (Place::Go, t.commands.go.to_string()),
        "settings" => (Place::Settings, t.commands.settings.to_string()),
        "help" => (Place::Help, t.commands.help.to_string()),
        "" | "page" => (if typed { Place::Typed(first) } else { Place::Page(first) }, t.commands.page.to_string()),
        named => (if typed { Place::Typed(first) } else { Place::Page(first) }, named.to_string()),
    }
}

/// The palette's own commands: light, dark or the system's, the page in the other language,
/// Betula's single keys on or off. `theme` is the one chosen (`None`: the system's), `here` the
/// address of this page in each other language.
fn settings(t: &'static Texts, theme: Option<&str>, letters: bool, here: &[(String, String)]) -> Vec<Item> {
    let heading = t.commands.settings;
    let now = |on: bool| on.then(|| t.commands.now.to_string());
    let theme_item = |label: &str, value: &str, icon: &str, on: bool| Item {
        sub: now(on),
        icon: Some(icon_href(icon)),
        ..Item::new(Place::Settings, heading, label.to_string(), Run::Act("theme-set".into(), value.into()))
    }
    .found_by(t.commands.theme_words);
    let mut items = vec![
        theme_item(t.commands.theme_light, "light", "sun", theme == Some("light")),
        theme_item(t.commands.theme_dark, "dark", "moon", theme == Some("dark")),
        theme_item(t.commands.theme_system, "system", "monitor", theme.is_none()),
    ];
    items.extend(here.iter().map(|(name, href)| Item { icon: Some(icon_href("languages")), ..Item::new(Place::Settings, heading, name.clone(), Run::Address(href.clone())) }.found_by(t.commands.language_words)));
    let (label, value) = if letters { (t.commands.letters_off, "off") } else { (t.commands.letters_on, "on") };
    items.push(Item { icon: Some(icon_href("keyboard")), ..Item::new(Place::Settings, heading, label.to_string(), Run::Act("letters-set".into(), value.into())) }.found_by(t.commands.letters_words));
    items
}

/// The address of an icon of the sprite.
fn icon_href(name: &str) -> String {
    format!("{}#{name}", folia_design::asset(folia_design::icons::SPRITE))
}

/// A module found, as a command: its title, its number, beside the list (Shift: its page).
fn module_item(t: &'static Texts, id: &str, title: &str) -> Item {
    Item { sub: Some(id.to_string()), icon: Some(icon_href("layout-list")), ..Item::new(Place::Modules, t.commands.modules, title.to_string(), Run::Module(id.to_string())) }.found_by(&format!("{id} {}", t.commands.module_words))
}

/// The button beside the search that opens the palette: its icon, „Befehle" and its keys; part of
/// the search's region for the keyboard (Ctrl + F6, Esc into the list). The browser app's alone
/// (R15); not on a phone, where there are no keys and every command is a tap away (app.css).
#[component]
pub fn CommandsButton() -> impl IntoView {
    let t = i18n::t();
    let open = CommandsOpen::expect();
    let title = format!("{} ({})", t.commands.button_title, folia_design::keys::text_here(COMMANDS));
    view! {
        <button
            class="cmd-btn js-only"
            type="button"
            data-action="commands"
            data-region="search"
            aria-haspopup="dialog"
            aria-keyshortcuts="Control+K Meta+K"
            title=title
            on:click=move |_| {
                if let Some(CommandsOpen(open)) = open {
                    open.set(true);
                }
            }
        >
            <Icon name="zap"/><span class="cmd-btn-label">{t.commands.button}</span><Keys spec=COMMANDS/>
        </button>
    }
}

/// The palette, once in the app: a dialog shown modally near the top of the window, its field a
/// combobox whose list is the commands (the ARIA pattern screen readers know), the number of what
/// it finds told. Esc and a click beside it close it, and the focus goes back where it was, where
/// the command then runs.
#[component]
pub fn CommandPalette() -> impl IntoView {
    let t = i18n::t();
    let Some(CommandsOpen(open)) = CommandsOpen::expect() else { return ().into_any() };
    let node = NodeRef::<leptos::html::Dialog>::new();
    let field = NodeRef::<leptos::html::Input>::new();
    let text = RwSignal::new(String::new());
    let active = RwSignal::new(0usize);
    let items = RwSignal::new(Vec::<Item>::new());
    let taught = RwSignal::new(None::<String>);
    // Which note is shown: a newer one is not taken away by the time of the one before.
    let taught_at = StoredValue::new(0u64);
    #[cfg(feature = "csr")]
    let found = StoredValue::new_local(Vec::<web_sys::Element>::new());

    // The modules, asked for once the text rests (as the search waits for the keys to rest).
    let settled = RwSignal::new(String::new());
    Effect::new(move |_| {
        let now = text.get();
        set_timeout(
            move || {
                if text.with_untracked(|text| *text == now) {
                    settled.try_set(now);
                }
            },
            std::time::Duration::from_millis(120),
        );
    });
    let modules = folia_data::use_ask(move || {
        let text = settled.get();
        (open.get() && folia_search::fold(text.trim()).chars().filter(|c| c.is_alphanumeric()).count() >= 2).then_some(folia_pages::ask::CommandModulesAsk { text, limit: 8 })
    });
    let module_items = Memo::new(move |_| {
        if text.with(|text| text.trim().is_empty()) {
            return Vec::new();
        }
        modules.with(|found| found.as_ref().and_then(|found| found.as_ref().ok()).map(|rows| rows.iter().map(|row| module_item(t, &row.id, &row.title)).collect()).unwrap_or_default())
    });
    // Whether the modules for the text are still on their way.
    let searching = Memo::new(move |_| text.with(|text| folia_search::fold(text.trim()).chars().filter(|c| c.is_alphanumeric()).count() >= 2) && (settled.get() != text.get() || modules.with(Option::is_none)));

    let all = Memo::new(move |_| {
        let mut all = items.get();
        all.extend(module_items.get());
        all
    });
    let rows = Memo::new(move |_| {
        let all = all.get();
        text.with(|text| shown(&all, text)).into_iter().filter_map(|i| all.get(i).cloned()).collect::<Vec<Item>>()
    });
    // A new text chooses its first row again, and a list grown shorter its last.
    Effect::new(move |_| {
        text.track();
        active.set(0);
    });
    Effect::new(move |_| {
        let n = rows.with(Vec::len);
        if active.get_untracked() >= n {
            active.set(n.saturating_sub(1));
        }
    });

    // Opened: what the page offers now, read before the dialog takes the focus.
    Effect::new(move |_| {
        let shown_now = open.get();
        let Some(dialog) = node.get() else { return };
        if shown_now && !dialog.open() {
            text.set(String::new());
            active.set(0);
            #[cfg(feature = "csr")]
            {
                let (page, elements) = page::collect(t);
                found.set_value(elements);
                let mut all = page;
                all.extend(settings(t, page::theme().as_deref(), folia_design::keys::letters_on(), &page::languages()));
                items.set(all);
            }
            let _ = dialog.show_modal();
            if let Some(field) = field.get() {
                let _ = field.focus();
            }
        } else if !shown_now && dialog.open() {
            dialog.close();
        }
    });

    // The active row in view.
    Effect::new(move |_| {
        let i = active.get();
        rows.track();
        #[cfg(feature = "csr")]
        request_animation_frame(move || page::into_view(&format!("cmd-o-{i}")));
        #[cfg(not(feature = "csr"))]
        let _ = i;
    });

    let close = move || open.set(false);
    // Runs a row: the dialog closes (the focus back where it was), then the command.
    let run = move |i: usize, alt: bool| {
        let Some(item) = rows.with_untracked(|rows| rows.get(i).cloned()) else { return };
        close();
        if let Some(dialog) = node.get_untracked() {
            if dialog.open() {
                dialog.close();
            }
        }
        #[cfg(feature = "csr")]
        found.with_value(|elements| page::run(&item.run, alt, elements, t));
        #[cfg(not(feature = "csr"))]
        let _ = alt;
        // What it was run by: its key does it too.
        if let Some(keys) = item.keys.filter(|keys| !folia_design::keys::is_letter(keys) || folia_design::keys::letters_on()) {
            taught.set(Some((t.commands.taught)(&folia_design::keys::text_here(&keys))));
            let at = taught_at.get_value().wrapping_add(1);
            taught_at.set_value(at);
            set_timeout(
                move || {
                    if taught_at.try_get_value() == Some(at) {
                        taught.try_set(None);
                    }
                },
                std::time::Duration::from_millis(2600),
            );
        }
    };

    let on_key = move |ev: leptos::ev::KeyboardEvent| {
        let n = rows.with_untracked(Vec::len);
        let step = |by: isize| {
            if n > 0 {
                let now = active.get_untracked() as isize;
                active.set((now + by).rem_euclid(n as isize) as usize);
            }
        };
        match ev.key().as_str() {
            "ArrowDown" => {
                ev.prevent_default();
                step(1);
            }
            "ArrowUp" => {
                ev.prevent_default();
                step(-1);
            }
            "PageDown" => {
                ev.prevent_default();
                step(6);
            }
            "PageUp" => {
                ev.prevent_default();
                step(-6);
            }
            "Enter" if !ev.is_composing() => {
                ev.prevent_default();
                run(active.get_untracked(), ev.shift_key() || ev.ctrl_key() || ev.meta_key());
            }
            "Tab" => ev.prevent_default(),
            "k" | "K" if (ev.ctrl_key() || ev.meta_key()) && !ev.alt_key() => {
                ev.prevent_default();
                close();
            }
            _ => {}
        }
    };

    let list = move || {
        let rows = rows.get();
        let mut groups: Vec<(String, Vec<(usize, Item)>)> = Vec::new();
        for (i, item) in rows.into_iter().enumerate() {
            match groups.last_mut() {
                Some((heading, members)) if *heading == item.heading => members.push((i, item)),
                _ => groups.push((item.heading.clone(), vec![(i, item)])),
            }
        }
        groups
            .into_iter()
            .enumerate()
            .map(|(g, (heading, members))| {
                let (id, labelled_by) = (format!("cmd-g-{g}"), format!("cmd-g-{g}"));
                view! {
                    <div class="cmd-group" role="group" aria-labelledby=labelled_by>
                        <div class="cmd-heading" id=id role="presentation">{heading}</div>
                        {members.into_iter().map(|(i, item)| {
                            let icon = item.icon.clone().map(|href| view! { <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" inner_html=format!("<use href=\"{href}\"/>")></svg> });
                            view! {
                                <div
                                    class="cmd-opt"
                                    role="option"
                                    id=format!("cmd-o-{i}")
                                    aria-selected=move || if active.get() == i { "true" } else { "false" }
                                    on:pointermove=move |_| if active.get_untracked() != i { active.set(i) }
                                    on:click=move |ev: leptos::ev::MouseEvent| run(i, ev.shift_key())
                                >
                                    <span class="cmd-icon">{icon}</span>
                                    <span class="cmd-label">{item.label.clone()}</span>
                                    {item.sub.clone().map(|sub| view! { <span class="cmd-sub">{sub}</span> })}
                                    {item.keys.clone().map(|keys| view! { <span class="cmd-keys"><Keys spec=keys/></span> })}
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }
            })
            .collect_view()
    };
    let empty = move || text.with(|text| text.trim().is_empty());
    let nothing = move || !empty() && rows.with(Vec::is_empty) && !searching.get();
    let count = move || if empty() { String::new() } else { (t.commands.count)(rows.with(Vec::len)) };

    view! {
        <dialog
            class="cmd-dialog"
            node_ref=node
            aria-label=t.commands.title
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
            <div class="cmd-box">
                <div class="cmd-field">
                    <Icon name="search"/>
                    <input
                        node_ref=field
                        type="text"
                        role="combobox"
                        aria-expanded="true"
                        aria-controls="cmd-list"
                        aria-autocomplete="list"
                        aria-activedescendant=move || rows.with(|rows| (!rows.is_empty()).then(|| format!("cmd-o-{}", active.get())))
                        aria-label=t.commands.field
                        placeholder=t.commands.placeholder
                        autocomplete="off"
                        spellcheck="false"
                        data-prepares-search=""
                        prop:value=move || text.get()
                        on:input=move |ev| text.set(event_target_value(&ev))
                        on:keydown=on_key
                    />
                </div>
                <div class="cmd-list" id="cmd-list" role="listbox" aria-label=t.commands.title>{list}</div>
                {move || nothing().then(|| view! { <p class="cmd-none">{t.commands.nothing}</p> })}
                {move || (searching.get() && rows.with(|rows| !rows.iter().any(|row| row.place == Place::Modules))).then(|| view! { <p class="cmd-none">{t.commands.searching}</p> })}
                {move || empty().then(|| view! {
                    <div class="cmd-finds">
                        <p class="cmd-heading">{t.commands.typing_finds}</p>
                        <p><Icon name="sliders-horizontal"/>{t.commands.finds_filters}</p>
                        <p><Icon name="layout-list"/>{t.commands.finds_modules}</p>
                        <p><Icon name="sun"/>{t.commands.finds_settings}</p>
                    </div>
                })}
                <div class="cmd-foot" aria-hidden="true">
                    <span><kbd>"↑"</kbd><kbd>"↓"</kbd>{t.commands.foot_choose}</span>
                    <span><Keys spec="Enter"/>{t.commands.foot_run}</span>
                    <span><Keys spec="Shift+Enter"/>{t.commands.foot_alt}</span>
                </div>
                <p class="visually-hidden" role="status">{count}</p>
            </div>
        </dialog>
        <p class="cmd-taught" role="status" class:on=move || taught.with(Option::is_some)>{move || taught.get()}</p>
    }
    .into_any()
}

/// What the palette reads from the page and does there: the browser's alone.
#[cfg(feature = "csr")]
mod page {
    use leptos::wasm_bindgen::JsCast;
    use leptos::web_sys::{self, Element, HtmlElement, HtmlInputElement};

    use super::{group, How, Item, Run};
    use crate::i18n::{self, Locale, Texts};

    fn document() -> Option<web_sys::Document> {
        web_sys::window().and_then(|window| window.document())
    }

    /// Whether an element is shown: it has boxes, or it waits in a closed `<details>` that has; not
    /// on a tab's page behind the one in front (`stage`), nor where the page is inert.
    fn shown(el: &Element) -> bool {
        if el.closest(".page-stage:not(.front), [inert]").ok().flatten().is_some() || el.get_attribute("aria-disabled").as_deref() == Some("true") {
            return false;
        }
        el.get_client_rects().length() > 0 || el.closest("details:not([open])").ok().flatten().is_some_and(|details| details.get_client_rects().length() > 0)
    }

    /// An element's own name: its `aria-label`, else its text with its spaces made one.
    fn name_of(el: &Element) -> String {
        let label = el.get_attribute("aria-label").filter(|label| !label.trim().is_empty()).or_else(|| el.text_content()).unwrap_or_default();
        label.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// The icon an element shows itself: its first of the sprite's that is not a chip's box.
    fn icon_of(el: &Element) -> Option<String> {
        if let Some(icon) = el.get_attribute("data-command-icon") {
            return Some(super::icon_href(&icon));
        }
        let uses = el.query_selector_all("svg use").ok()?;
        (0..uses.length()).filter_map(|i| uses.item(i)?.dyn_into::<Element>().ok()).find(|use_| use_.closest(".box").ok().flatten().is_none()).and_then(|use_| use_.get_attribute("href"))
    }

    /// The commands the page names (see the module's head), in its order, and their elements.
    pub fn collect(t: &'static Texts) -> (Vec<Item>, Vec<Element>) {
        let Some(document) = document() else { return (Vec::new(), Vec::new()) };
        // Where the keyboard was: a row's commands are there only while it is in the row.
        let from = document.active_element();
        let Ok(list) = document.query_selector_all("[data-command]") else { return (Vec::new(), Vec::new()) };
        let letters = folia_design::keys::letters_on();
        let mut items = Vec::new();
        let mut elements = Vec::new();
        let mut firsts: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for i in 0..list.length() {
            let Some(el) = list.item(i).and_then(|node| node.dyn_into::<Element>().ok()) else { continue };
            if !shown(&el) {
                continue;
            }
            let container = el.parent_element().and_then(|parent| parent.closest("[data-commands]").ok().flatten());
            if let Some(container) = &container {
                if container.get_attribute("data-commands-when").as_deref() == Some("focus") && !from.as_ref().is_some_and(|from| container.contains(Some(&**from))) {
                    continue;
                }
            }
            let name = el.get_attribute("data-command-group").or_else(|| container.as_ref().and_then(|c| c.get_attribute("data-commands"))).unwrap_or_default();
            let typed = container.as_ref().is_some_and(|c| c.has_attribute("data-commands-typed"));
            // The groups of the page stand in the order in which they first come.
            let first = firsts.iter().position(|known| *known == name).unwrap_or_else(|| {
                firsts.push(name.clone());
                firsts.len() - 1
            });
            let (place, heading) = group(&name, first, typed, t);
            let own = el.get_attribute("data-command").filter(|name| !name.trim().is_empty()).unwrap_or_else(|| name_of(&el));
            if own.is_empty() {
                continue;
            }
            let of = el.parent_element().and_then(|parent| parent.closest("[data-command-of]").ok().flatten()).and_then(|of| of.get_attribute("data-command-of")).filter(|of| !of.is_empty());
            let label = match of {
                Some(of) => format!("{of}: {own}"),
                None => own,
            };
            if !seen.insert((heading.clone(), label.clone())) {
                continue;
            }
            let keys = el.get_attribute("data-command-keys").filter(|keys| !keys.is_empty() && (letters || !folia_design::keys::is_letter(keys)));
            let run = if let Some((action, value)) = el.get_attribute("data-command-act").as_deref().and_then(|act| act.split_once(' ')) {
                Run::Act(action.to_string(), value.to_string())
            } else {
                let how = if el.get_attribute("data-command-by").as_deref() == Some("event") {
                    How::Tell
                } else if el.dyn_ref::<HtmlInputElement>().is_some() {
                    How::Focus
                } else {
                    How::Click
                };
                Run::Element(elements.len(), how)
            };
            let item = Item {
                sub: el.get_attribute("data-command-state").filter(|state| !state.is_empty()),
                keys,
                icon: icon_of(&el),
                typed_only: typed,
                ..Item::new(place, &heading, label, run)
            }
            .found_by(el.get_attribute("data-command-words").as_deref().unwrap_or(""));
            items.push(item);
            elements.push(el);
        }
        (items, elements)
    }

    /// The theme chosen in this browser (`light`, `dark`), `None` for the system's.
    pub fn theme() -> Option<String> {
        document().and_then(|document| document.document_element()).and_then(|root| root.get_attribute("data-theme"))
    }

    /// This page in each other language: its name in itself and its address.
    pub fn languages() -> Vec<(String, String)> {
        let t = i18n::t();
        let Some(location) = web_sys::window().map(|window| window.location()) else { return Vec::new() };
        let (path, search) = (location.pathname().unwrap_or_default(), location.search().unwrap_or_default());
        let (_, inner) = Locale::split(&path);
        let here = format!("{inner}{search}");
        Locale::ALL.iter().copied().filter(|locale| *locale != t.locale).map(|locale| (locale.name().to_string(), locale.path(&here))).collect()
    }

    /// The row of the list with `id` in view.
    pub fn into_view(id: &str) {
        let Some(el) = document().and_then(|document| document.get_element_by_id(id)) else { return };
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Nearest);
        el.scroll_into_view_with_scroll_into_view_options(&options);
    }

    /// Does what a command says, the dialog closed and the focus back where it was.
    pub fn run(run: &Run, alt: bool, elements: &[Element], t: &'static Texts) {
        match run {
            Run::Element(i, how) => {
                let Some(el) = elements.get(*i) else { return };
                if !el.is_connected() {
                    return;
                }
                match how {
                    How::Click => {
                        // In a closed <details> („Weitere Filter"), which opens on the way.
                        if let Some(details) = el.closest("details:not([open])").ok().flatten() {
                            let _ = details.set_attribute("open", "");
                        }
                        if let Some(el) = el.dyn_ref::<HtmlElement>() {
                            el.click();
                        }
                    }
                    How::Focus => {
                        if let Some(input) = el.dyn_ref::<HtmlInputElement>() {
                            let _ = input.focus();
                            input.select();
                        }
                    }
                    How::Tell => {
                        if let Ok(event) = web_sys::Event::new(if alt { "betula-command-alt" } else { "betula-command" }) {
                            let _ = el.dispatch_event(&event);
                        }
                    }
                }
            }
            Run::Act(action, value) => act(action, value),
            Run::Module(id) => module(id, alt, t),
            Run::Address(href) => {
                if let Some(window) = web_sys::window() {
                    let _ = window.location().assign(href);
                }
            }
        }
    }

    /// What enhance.js does for a switch of the page (`betula:act`, „the keyboard" and the
    /// switches): `theme-set`, `letters-set`, `go`.
    fn act(action: &str, value: &str) {
        let Some(document) = document() else { return };
        let init = web_sys::CustomEventInit::new();
        init.set_detail(&leptos::wasm_bindgen::JsValue::from_str(&format!("{action} {value}")));
        if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("betula:act", &init) {
            let _ = document.dispatch_event(&event);
        }
    }

    /// A module found: beside the catalog's list as it is (or as its tab left it), with Shift its
    /// own page.
    fn module(id: &str, full: bool, t: &'static Texts) {
        use folia_routes::url::{self, CatalogUrl};
        let to = if full {
            url::module_path(id)
        } else {
            let location = web_sys::window().map(|window| window.location());
            let (path, search) = location.map(|location| (location.pathname().unwrap_or_default(), location.search().unwrap_or_default())).unwrap_or_default();
            let (_, path) = Locale::split(&path);
            let list = if path == url::CATALOG {
                format!("{path}{search}")
            } else {
                crate::tabs::Tabs::expect().map(|tabs| tabs.list(crate::tabs::Area::Catalog)).unwrap_or_else(|| url::CATALOG.to_string())
            };
            let search = list.split_once('?').map(|(_, search)| search).unwrap_or("");
            let mut next = CatalogUrl::parse(search);
            next.open = Some(id.to_string());
            next.path()
        };
        let to = t.path(&to);
        match crate::pending::Pending::expect() {
            Some(going) => going.go(&to, Default::default()),
            None => {
                if let Some(window) = web_sys::window() {
                    let _ = window.location().assign(&to);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(place: Place, label: &str, typed: bool) -> Item {
        Item { typed_only: typed, ..Item::new(place, "Gruppe", label.to_string(), Run::Act(String::new(), String::new())) }.found_by("")
    }

    /// Without a text the page's commands and „Gehe zu" show, the filters, modules and settings
    /// wait for it; with one every group keeps its place, and in it what begins with the text
    /// comes first.
    #[test]
    fn what_shows_and_in_which_order() {
        let items = vec![
            item(Place::Typed(1), "Prüfung: Klausur", true),
            item(Place::Go, "Merkliste", false),
            item(Place::Page(0), "Merken", false),
            item(Place::Settings, "Dunkel", false),
            item(Place::Modules, "Klausurtechnik", false),
            item(Place::Typed(1), "Sprache: Englisch", true),
            item(Place::Page(0), "Sammlung und Notiz", false),
        ];
        assert_eq!(shown(&items, ""), vec![2, 6, 1]);
        assert_eq!(shown(&items, "  "), vec![2, 6, 1]);
        // Folded: „klausur" finds „Klausur", in its group before the module, which the search
        // found for the text (with a typo as well).
        assert_eq!(shown(&items, "klausur"), vec![0, 4]);
        assert_eq!(shown(&items, "MERK"), vec![2, 1, 4]);
        assert_eq!(shown(&items, "prufung klaus"), vec![0, 4]);
        assert_eq!(shown(&items, "dunkel"), vec![3, 4]);
        // A name that begins with the text before one with a word that does, before the rest.
        let items = vec![item(Place::Page(0), "Alle Filter zurücksetzen", false), item(Place::Page(0), "Filter", false), item(Place::Page(0), "Weitere Filter", false)];
        assert_eq!(shown(&items, "filter"), vec![1, 0, 2]);
    }

    /// The settings: the theme chosen and the system's marked „jetzt", the other language, and
    /// the single keys turned the other way than they are.
    #[test]
    fn the_settings_say_what_holds_now() {
        let t = i18n::texts(i18n::Locale::De);
        let items = settings(t, Some("dark"), true, &[("English".into(), "/en/catalog".into())]);
        let labels: Vec<(&str, Option<&str>)> = items.iter().map(|item| (item.label.as_str(), item.sub.as_deref())).collect();
        assert_eq!(labels, vec![("Hell", None), ("Dunkel", Some("jetzt")), ("Hell oder dunkel wie das System", None), ("English", None), ("Einzelne Tasten (F, ?, M …) ausschalten", None)]);
        assert_eq!(items.get(3).map(|item| &item.run), Some(&Run::Address("/en/catalog".into())));
        let items = settings(t, None, false, &[]);
        assert_eq!(items.get(2).and_then(|item| item.sub.as_deref()), Some("jetzt"));
        assert_eq!(items.last().map(|item| &item.run), Some(&Run::Act("letters-set".into(), "on".into())));
        // Found by what they are about.
        assert!(items.iter().any(|item| item.words.contains("nacht")));
    }

    /// A page's groups by their names: the palette's own ones in their places, the others where
    /// the page has them, after „Gehe zu" when found by typing only.
    #[test]
    fn the_groups_a_page_names() {
        let t = i18n::texts(i18n::Locale::De);
        assert_eq!(group("go", 3, false, t), (Place::Go, "Gehe zu".to_string()));
        assert_eq!(group("help", 0, false, t), (Place::Help, "Hilfe".to_string()));
        assert_eq!(group("Filter", 2, true, t), (Place::Typed(2), "Filter".to_string()));
        assert_eq!(group("", 0, false, t), (Place::Page(0), "Auf dieser Seite".to_string()));
        assert!(Place::Page(5) < Place::Go && Place::Go < Place::Typed(0) && Place::Typed(9) < Place::Settings && Place::Help < Place::Modules);
    }
}
