//! A picker with a search field, for long lists (programs, lecturers, departments), and `Select`,
//! its button and popup without the search, for a few entries (a Studienbeginn, a study direction).
//!
//! Type a few letters (word starts, initials and typos are understood, `folia_search::fuzzy`), move
//! with the arrow keys, Enter takes the marked entry, Esc closes. It belongs to the browser app;
//! pages rendered without it use a plain `<select>` in its place.
//!
//! What keeps it predictable:
//! - one state: `open`, the typed `text`, and `active`, an index into the rows shown;
//! - the mouse marks an entry only when it moves, so a list scrolling under a resting pointer
//!   does not take the mark away from the keyboard;
//! - a mouse press inside the popup never takes the focus from the search field, and the popup
//!   closes exactly when the focus leaves the picker, however that happens;
//! - the selection is known by id, never by its label.

use folia_search::fuzzy;
use leptos::ev::{FocusEvent, KeyboardEvent, MouseEvent};
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::web_sys;

use crate::i18n;
use crate::nav::{self, PopupPlace};
use crate::ui::Icon;

/// The ranked entries with each group together, so that no heading comes twice: the groups in
/// the order of their best entry (the entries without a group are one as well), each in the order
/// of the ranking. Without a search the ranking is the order of the items, so the groups stand as
/// they were given.
fn grouped<'a>(ranked: &[usize], group: impl Fn(usize) -> &'a str) -> Vec<usize> {
    let mut groups: Vec<(&str, Vec<usize>)> = Vec::new();
    for &index in ranked {
        let name = group(index);
        match groups.iter_mut().find(|(known, _)| *known == name) {
            Some((_, members)) => members.push(index),
            None => groups.push((name, vec![index])),
        }
    }
    groups.into_iter().flat_map(|(_, members)| members).collect()
}

/// The rows of the popup for what is typed, best match first; `None` is the entry that clears the
/// selection. While nothing is typed it stands first, after the entries that stand above it
/// (`ComboItem::first`, at the head of the items).
fn rows_of(items: &[ComboItem], text: &str, clearable: bool) -> Vec<Option<usize>> {
    let ranked = fuzzy::rank(text, items.iter().map(|item| (item.search.as_str(), item.bonus)));
    let mut rows: Vec<Option<usize>> = grouped(&ranked, |index| items.get(index).map(|item| item.group.as_str()).unwrap_or_default()).into_iter().map(Some).collect();
    if clearable && text.trim().is_empty() {
        let at = rows.iter().take_while(|row| row.and_then(|index| items.get(index)).is_some_and(|item| item.first)).count();
        rows.insert(at, None);
    }
    rows
}

/// The items with the one of `id` once more at their head, under `heading` and above the entry
/// that clears the selection (`ComboItem::first`): „Mein Studiengang" in a picker of programs (owner,
/// 2026-10-15: „immer der erste Eintrag beim Dropdown"), which stays in its place among the others
/// as well. Without `id`, or with an id none of them has, the items as they are.
pub fn with_first(items: &[ComboItem], id: Option<&str>, heading: &str) -> Vec<ComboItem> {
    let first = id.and_then(|id| items.iter().find(|item| item.id == id)).map(|item| ComboItem { first: true, ..item.clone().in_group(heading) });
    first.into_iter().chain(items.iter().cloned()).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComboItem {
    pub id: String,
    pub label: String,
    /// Shown smaller next to the label, and searched as well.
    pub detail: String,
    /// Lifts an entry above others that match the search equally well.
    pub bonus: i64,
    /// A heading the entry stands under; entries of one group follow each other. Empty: none.
    pub group: String,
    /// Stands above the entry that clears the selection while nothing is typed (`with_first`).
    pub first: bool,
    search: String,
}

impl ComboItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>, detail: impl Into<String>, bonus: i64) -> Self {
        let (label, detail) = (label.into(), detail.into());
        Self { id: id.into(), search: format!("{label} {detail}"), label, detail, bonus, group: String::new(), first: false }
    }

    /// The heading the entry stands under.
    pub fn in_group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    /// Words the entry is found by as well, without being shown (where it sits in a tree).
    pub fn also_found_by(mut self, words: &str) -> Self {
        self.search = format!("{} {words}", self.search);
        self
    }
}

/// A scrolling filter panel takes its popups away: bump this to close them all.
#[derive(Clone, Copy)]
pub struct ClosePopups(pub RwSignal<u32>);

/// More entries than this are not rendered; typing narrows the list down.
const MAX_SHOWN: usize = 120;
const PAGE_STEP: usize = 8;

/// Whether the popup is open. `false` once the combobox is gone: a combobox its own `on_select`
/// removed still hears the focusout of its search field, and reading a disposed signal panics.
fn is_open(open: RwSignal<bool>) -> bool {
    open.try_get_untracked() == Some(true)
}

#[component]
pub fn Combobox(
    /// Id of the button; the parts of the popup derive theirs from it.
    id: &'static str,
    /// What is being picked, for screen readers: „Studiengang". This and the texts below are in
    /// the page's language (`i18n`).
    label: &'static str,
    /// The button's text while nothing is selected. With `clearable` also the first entry.
    placeholder: &'static str,
    search_placeholder: &'static str,
    #[prop(into)] items: Signal<Vec<ComboItem>>,
    /// The id of the selected entry.
    #[prop(into)] selected: Signal<Option<String>>,
    /// What the button says when no single entry is selected but more than nothing is („5
    /// Bereiche"); clearable like a selection.
    #[prop(optional, into)] summary: Option<Signal<Option<String>>>,
    /// The id of the picked entry; `None` when the selection was cleared.
    #[prop(into)] on_select: Callback<Option<String>>,
    #[prop(optional)] icon: Option<&'static str>,
    /// Offers „nothing selected" as the first entry and as a button next to the selection.
    #[prop(default = true)] clearable: bool,
    /// The popup is as wide as the button, but not narrower than this (long names need room).
    #[prop(default = 340.0)] min_width: f64,
) -> impl IntoView {
    let t = i18n::t();
    let open = RwSignal::new(false);
    let text = RwSignal::new(String::new());
    let active = RwSignal::new(0usize);
    let place = RwSignal::new(None::<PopupPlace>);
    let root = NodeRef::<leptos::html::Div>::new();
    let search_id = format!("{id}-search");
    let list_id = format!("{id}-list");
    let option_id = move |row: usize| format!("{id}-option-{row}");

    // The rows of the popup, best match first; `None` is the entry that clears the selection.
    let rows = Memo::new(move |_| {
        if !open.get() {
            return Vec::new();
        }
        let text = text.get();
        items.with(|items| rows_of(items, &text, clearable))
    });
    let shown = move || rows.with(|rows| rows.len().min(MAX_SHOWN));

    let current = Memo::new(move |_| {
        let picked = selected.get().and_then(|id| items.with(|items| items.iter().find(|item| item.id == id).map(|item| (item.label.clone(), item.detail.clone()))));
        picked.or_else(|| summary.and_then(|summary| summary.get()).map(|text| (text, String::new())))
    });

    let show = {
        let search_id = search_id.clone();
        move || {
            text.set(String::new());
            place.set(nav::popup_place(id, min_width));
            open.set(true);
            // Start at the selected entry.
            let selected = selected.get_untracked();
            let start = rows.with_untracked(|rows| {
                rows.iter().position(|row| match (row, &selected) {
                    (None, None) => true,
                    (Some(index), Some(id)) => items.with_untracked(|items| items.get(*index).is_some_and(|item| &item.id == id)),
                    _ => false,
                })
            });
            let start = start.filter(|row| *row < MAX_SHOWN).unwrap_or(0);
            active.set(start);
            let (search_id, option) = (search_id.clone(), option_id(start));
            request_animation_frame(move || {
                nav::focus_by_id(&search_id);
                nav::reveal_in_list(&option, true);
            });
        }
    };
    let hide = move |return_focus: bool| {
        if is_open(open) {
            open.set(false);
            if return_focus {
                nav::focus_by_id(id);
            }
        }
    };
    let choose = move |row: usize| {
        let Some(picked) = rows.with_untracked(|rows| rows.get(row).copied()) else { return };
        let picked_id = match picked {
            None => None,
            Some(index) => match items.with_untracked(|items| items.get(index).map(|item| item.id.clone())) {
                Some(id) => Some(id),
                None => return,
            },
        };
        hide(true);
        on_select.run(picked_id);
    };
    let mark = move |row: usize| {
        active.set(row);
        nav::reveal_in_list(&option_id(row), false);
    };

    // Whatever moves the popup's anchor closes it: the panel scrolls, the window changes size.
    // A scroll event arrives a frame after the scrolling, possibly after a click that opened the
    // popup in the meantime, so what counts is whether the button has really moved since then.
    // On a phone the popup is not anchored but in place, so neither matters there — and there
    // the keyboard that opens for the search field changes the size of the window and scrolls
    // the field into view, which used to close the popup the moment it opened. A new size only
    // measures again how far in its panel the picker stands, which the popup spans.
    if let Some(ClosePopups(signal)) = use_context::<ClosePopups>() {
        Effect::new(move |_| {
            signal.track();
            if open.get_untracked() && !nav::is_phone() && nav::popup_place(id, min_width) != place.get_untracked() {
                hide(false);
            }
        });
    }
    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::resize, move |_| {
            if !nav::is_phone() {
                hide(false);
            } else if is_open(open) {
                place.set(nav::popup_place(id, min_width));
            }
        });
        on_cleanup(move || handle.remove());
    });

    let toggle = {
        let show = show.clone();
        move |_: MouseEvent| if open.get_untracked() { hide(true) } else { show() }
    };
    // ↑ and ↓ open it, Alt + ↓ as well. In a mode of the keyboard (the catalog's filters, where the
    // arrows go from filter to filter: enhance.js, „the keyboard") only Alt + ↓ and Enter do, as
    // a select of the system has it.
    let on_trigger_key = {
        let show = show.clone();
        move |ev: KeyboardEvent| {
            let arrow = matches!(ev.key().as_str(), "ArrowDown" | "ArrowUp");
            let in_mode = ev.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok()).and_then(|element| element.closest("[data-mode]").ok().flatten()).is_some();
            if arrow && !open.get_untracked() && (ev.alt_key() || !in_mode) {
                ev.prevent_default();
                show();
            }
        }
    };
    let on_search_key = move |ev: KeyboardEvent| {
        let count = shown();
        let step = |by: isize, wrap: bool| {
            if count == 0 {
                return;
            }
            let last = count - 1;
            let now = active.get_untracked().min(last);
            let next = match (by < 0, wrap) {
                (false, true) => if now == last { 0 } else { now + 1 },
                (true, true) => if now == 0 { last } else { now - 1 },
                (false, false) => (now + by.unsigned_abs()).min(last),
                (true, false) => now.saturating_sub(by.unsigned_abs()),
            };
            mark(next);
        };
        match ev.key().as_str() {
            "ArrowDown" => step(1, true),
            "ArrowUp" => step(-1, true),
            "PageDown" => step(PAGE_STEP as isize, false),
            "PageUp" => step(-(PAGE_STEP as isize), false),
            "Enter" => {
                if count > 0 {
                    choose(active.get_untracked().min(count - 1));
                }
            }
            "Escape" => hide(true),
            // Tab moves on; the popup closes when the focus has left.
            _ => return,
        }
        ev.prevent_default();
        ev.stop_propagation();
    };
    let on_input = move |ev: leptos::ev::Event| {
        text.set(event_target_value(&ev));
        active.set(0);
        nav::reveal_in_list(&option_id(0), false);
    };
    let on_focus_out = move |ev: FocusEvent| {
        // A combobox its own `on_select` took off the page (the Studienplan's program picker,
        // the import's) still gets the focusout of its search field, after its signals are gone:
        // nothing is open then, and nothing may be read.
        if !is_open(open) {
            return;
        }
        let stays = ev.related_target().is_some_and(|target| {
            root.get_untracked().is_some_and(|root| target.dyn_ref::<web_sys::Node>().is_some_and(|node| root.contains(Some(node))))
        });
        if !stays {
            hide(false);
        }
    };
    // A press on the popup (an entry, the scrollbar, the frame) must not blur the search field.
    let keep_focus = move |ev: MouseEvent| {
        if event_target::<web_sys::Element>(&ev).tag_name() != "INPUT" {
            ev.prevent_default();
        }
    };

    let popup = move || {
        open.get().then(|| {
            let entries = move || {
                let rows = rows.get();
                let hidden = rows.len().saturating_sub(MAX_SHOWN);
                // A heading where the entries of another group begin (`grouped` keeps each group
                // together: what fits best comes first, its group's heading with it).
                let mut last_group = String::new();
                let entries = rows
                    .into_iter()
                    .take(MAX_SHOWN)
                    .enumerate()
                    .map(|(row, entry)| {
                        let item = entry.and_then(|index| items.with_untracked(|items| items.get(index).cloned()));
                        let is_selected = match (&item, selected.get_untracked()) {
                            (Some(item), Some(id)) => item.id == id,
                            (None, None) => entry.is_none(),
                            _ => false,
                        };
                        // The entries without a group after a group's entries: a line, so that they
                        // do not read as the group's. So is the entry that clears the selection
                        // after the entries that stand above it.
                        let group = item.as_ref().map_or_else(String::new, |item| item.group.clone());
                        let heading = (group != last_group).then_some(group).map(|group| {
                            last_group.clone_from(&group);
                            if group.is_empty() {
                                view! { <li class="combo-group combo-rule" role="presentation"></li> }.into_any()
                            } else {
                                view! { <li class="combo-group" role="presentation">{group}</li> }.into_any()
                            }
                        });
                        let (label, detail) = match item {
                            Some(item) => (item.label, item.detail),
                            None => (placeholder.to_string(), String::new()),
                        };
                        view! {
                            {heading}
                            <li
                                class="combo-option"
                                role="option"
                                id=option_id(row)
                                aria-selected=if is_selected { "true" } else { "false" }
                                class:active=move || active.get() == row
                                on:mousemove=move |_| if active.get_untracked() != row { active.set(row) }
                                on:click=move |_| choose(row)
                            >
                                <span class="combo-label">{label}</span>
                                {(!detail.is_empty()).then(|| view! { <small>{detail}</small> })}
                                {is_selected.then(|| view! { <Icon name="check"/> })}
                            </li>
                        }
                    })
                    .collect_view();
                let none = (shown() == 0).then(|| view! { <li class="combo-empty" role="presentation">{t.combobox.nothing_found}</li> });
                let more = (hidden > 0).then(|| view! { <li class="combo-empty" role="presentation">{(t.combobox.more)(hidden)}</li> });
                (entries, none, more)
            };
            view! {
                <div class="combo-pop" on:mousedown=keep_focus>
                    <div class="combo-search">
                        <Icon name="search"/>
                        <input
                            id=search_id.clone()
                            type="text"
                            role="combobox"
                            aria-expanded="true"
                            aria-autocomplete="list"
                            aria-controls=list_id.clone()
                            aria-activedescendant=move || option_id(active.get())
                            aria-label=search_placeholder
                            placeholder=search_placeholder
                            autocomplete="off"
                            spellcheck="false"
                            prop:value=move || text.get()
                            on:input=on_input
                            on:keydown=on_search_key
                        />
                    </div>
                    <ul class="combo-list scroll" id=list_id.clone() role="listbox" aria-label=label>{entries}</ul>
                    <div class="combo-foot"><kbd>"↑"</kbd><kbd>"↓"</kbd>" "{t.combobox.key_move}" "<kbd>"Enter"</kbd>" "{t.combobox.key_take}" "<kbd>"Esc"</kbd>" "{t.combobox.key_close}</div>
                </div>
            }
        })
    };

    view! {
        <div
            class="combo"
            node_ref=root
            data-open=move || open.get().then_some("true")
            // The popup's place, on the picker so that its button can span the list on a phone
            // (`PopupPlace`): an attribute of its own, so the popup placed again is not drawn
            // anew (its search field would lose the focus).
            style=move || open.get().then(|| place.get().map(|place| place.style())).flatten()
            data-wide=move || (open.get() && place.get().is_some_and(|place| place.wide.is_some())).then_some("")
            on:focusout=on_focus_out
        >
            <button
                type="button"
                class="combo-trigger"
                id=id
                aria-haspopup="listbox"
                // A command of „Befehle" (folia_shell::commands) that opens the picker.
                data-command=label
                data-command-state=move || current.get().map(|(name, _)| name)
                aria-expanded=move || if open.get() { "true" } else { "false" }
                aria-label=move || match current.get() {
                    Some((name, _)) => format!("{label}: {name}"),
                    None => format!("{label}: {placeholder}"),
                }
                // The click decides; a press must not move the focus first (see `keep_focus`).
                on:mousedown=move |ev: MouseEvent| ev.prevent_default()
                on:click=toggle
                on:keydown=on_trigger_key
            >
                {icon.map(|name| view! { <Icon name=name/> })}
                <span class="combo-value" class:placeholder=move || current.get().is_none()>
                    {move || match current.get() {
                        Some((name, detail)) => view! { {name}<small>{detail}</small> }.into_any(),
                        None => placeholder.into_any(),
                    }}
                </span>
                <Icon name="chevrons-up-down" class="combo-chevron"/>
            </button>
            {move || (clearable && current.get().is_some()).then(|| view! {
                <button type="button" class="combo-clear" aria-label=(t.combobox.clear)(label) on:click=move |_| { hide(false); on_select.run(None); }>
                    <Icon name="x"/>
                </button>
            })}
            {popup}
        </div>
    }
}

/// How long letters typed one after the other look for one entry together.
const TYPED_MS: u64 = 700;

/// The entry that typed letters lead to: the next after `from` that begins with them. A letter
/// typed again and again goes from one entry it begins to the next, as in the browser's own list.
fn typed_row(items: &[ComboItem], typed: &str, from: usize) -> Option<usize> {
    let typed = typed.to_lowercase();
    let first = typed.chars().next()?;
    let (needle, start) = if typed.chars().all(|c| c == first) { (first.to_string(), from + 1) } else { (typed, from) };
    let count = items.len().min(MAX_SHOWN);
    (0..count).map(|step| (start + step) % count).find(|row| items.get(*row).is_some_and(|item| item.label.to_lowercase().starts_with(&needle)))
}

/// A choice among a few entries that need no search (owner, 2026-10-16: „Dropdown menus allgemein
/// sind im browser styling passe das mal an, so dass das mehr mit der combobox matcht"): the
/// combobox's button and popup without its search field, in place of the browser's `<select>`,
/// whose list each browser and system draws its own way (Windows a blue bar on grey, a phone a
/// wheel). The button keeps the focus while the popup is open (the select-only combobox of the
/// ARIA patterns): ↑ ↓, Pos1, Ende, Bild ↑ ↓ mark an entry, Enter or the space bar takes it, Esc
/// closes the popup and nothing else, letters jump to the entry they begin; the mouse as in the
/// combobox, and on a phone it opens in place as the combobox does.
#[component]
pub fn Select(
    /// Id of the button; the parts of the popup derive theirs from it.
    id: &'static str,
    /// What is being picked, for screen readers: „Studienbeginn".
    label: &'static str,
    #[prop(into)] items: Signal<Vec<ComboItem>>,
    /// The id of the selected entry.
    #[prop(into)]
    selected: Signal<String>,
    /// The id of the entry picked, when it is another than the one selected.
    #[prop(into)]
    on_select: Callback<String>,
    #[prop(optional, into)] disabled: Signal<bool>,
    /// What the button says while no entry is selected.
    #[prop(optional)]
    placeholder: &'static str,
    #[prop(optional)] icon: Option<&'static str>,
    /// The popup is as wide as the button, but not narrower than this (its foot, the keys, needs
    /// 300 px).
    #[prop(default = 300.0)]
    min_width: f64,
    /// The smaller button, for a bar of tools („Plan-Semester").
    #[prop(optional)]
    small: bool,
) -> impl IntoView {
    let t = i18n::t();
    let open = RwSignal::new(false);
    let active = RwSignal::new(0usize);
    let place = RwSignal::new(None::<PopupPlace>);
    let typed = RwSignal::new((String::new(), 0u32));
    let root = NodeRef::<leptos::html::Div>::new();
    let list_id = format!("{id}-list");
    let option_id = move |row: usize| format!("{id}-option-{row}");
    let count = move || items.with(|items| items.len().min(MAX_SHOWN));

    let current = Memo::new(move |_| {
        let selected = selected.get();
        items.with(|items| items.iter().find(|item| item.id == selected).map(|item| (item.label.clone(), item.detail.clone())))
    });
    let mark = move |row: usize| {
        active.set(row);
        nav::reveal_in_list(&option_id(row), false);
    };
    // Opens the popup with `at` marked, else the selected entry, in the middle of the list.
    let show = move |at: Option<usize>| {
        if disabled.get_untracked() {
            return;
        }
        place.set(nav::popup_place(id, min_width));
        open.set(true);
        let selected = selected.get_untracked();
        let start = at.or_else(|| items.with_untracked(|items| items.iter().position(|item| item.id == selected))).filter(|row| *row < MAX_SHOWN).unwrap_or(0);
        active.set(start);
        let option = option_id(start);
        request_animation_frame(move || nav::reveal_in_list(&option, true));
    };
    let hide = move || {
        if is_open(open) {
            open.set(false);
        }
    };
    let choose = move |row: usize| {
        let Some(picked) = items.with_untracked(|items| items.get(row).map(|item| item.id.clone())) else { return };
        hide();
        nav::focus_by_id(id);
        if picked != selected.get_untracked() {
            on_select.run(picked);
        }
    };
    // Letters typed: the entry they lead to, from the one marked (or selected, while closed).
    let type_letter = move |letter: String| -> Option<usize> {
        let (mut text, generation) = typed.get_untracked();
        text.push_str(&letter);
        let generation = generation.wrapping_add(1);
        typed.set((text.clone(), generation));
        set_timeout(
            move || {
                if typed.try_with_untracked(|(_, now)| *now == generation) == Some(true) {
                    typed.set((String::new(), generation));
                }
            },
            std::time::Duration::from_millis(TYPED_MS),
        );
        let from = if open.get_untracked() { active.get_untracked() } else { items.with_untracked(|items| items.iter().position(|item| item.id == selected.get_untracked())).unwrap_or(0) };
        items.with_untracked(|items| typed_row(items, &text, from))
    };

    // Whatever moves the button closes the popup on a computer (see `Combobox`).
    if let Some(ClosePopups(signal)) = use_context::<ClosePopups>() {
        Effect::new(move |_| {
            signal.track();
            if open.get_untracked() && !nav::is_phone() && nav::popup_place(id, min_width) != place.get_untracked() {
                hide();
            }
        });
    }
    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::resize, move |_| {
            if !nav::is_phone() {
                hide();
            } else if is_open(open) {
                place.set(nav::popup_place(id, min_width));
            }
        });
        on_cleanup(move || handle.remove());
    });

    let on_key = move |ev: KeyboardEvent| {
        if disabled.get_untracked() {
            return;
        }
        let key = ev.key();
        let last = count().saturating_sub(1);
        // A letter (the space bar too while letters are being typed).
        let letter = (key.chars().count() == 1 && !ev.ctrl_key() && !ev.meta_key() && !ev.alt_key() && (key != " " || typed.with_untracked(|(text, _)| !text.is_empty()))).then(|| key.clone());
        if !open.get_untracked() {
            match key.as_str() {
                "ArrowDown" | "ArrowUp" | "Enter" | " " => show(None),
                "Home" => show(Some(0)),
                "End" => show(Some(last)),
                _ => match letter.and_then(type_letter) {
                    Some(row) => show(Some(row)),
                    None => return,
                },
            }
            ev.prevent_default();
            return;
        }
        let now = active.get_untracked().min(last);
        match key.as_str() {
            "ArrowDown" => mark((now + 1).min(last)),
            "ArrowUp" => mark(now.saturating_sub(1)),
            "PageDown" => mark((now + PAGE_STEP).min(last)),
            "PageUp" => mark(now.saturating_sub(PAGE_STEP)),
            "Home" => mark(0),
            "End" => mark(last),
            "Enter" => choose(now),
            " " if letter.is_none() => choose(now),
            "Escape" => {
                hide();
                nav::focus_by_id(id);
            }
            // Tab moves on; the popup closes when the focus has left.
            "Tab" => return,
            _ => match letter.and_then(type_letter) {
                Some(row) => mark(row),
                None => return,
            },
        }
        ev.prevent_default();
        ev.stop_propagation();
    };
    let on_focus_out = move |ev: FocusEvent| {
        if !is_open(open) {
            return;
        }
        let stays = ev.related_target().is_some_and(|target| {
            root.get_untracked().is_some_and(|root| target.dyn_ref::<web_sys::Node>().is_some_and(|node| root.contains(Some(node))))
        });
        if !stays {
            hide();
        }
    };
    // A press on the popup (an entry, the scrollbar, the frame) must not take the focus from the
    // button.
    let keep_focus = move |ev: MouseEvent| ev.prevent_default();

    let popup = {
        let list_id = list_id.clone();
        move || {
            open.get().then(|| {
                let entries = move || {
                    let selected = selected.get_untracked();
                    items.with(|items| {
                        items
                            .iter()
                            .take(MAX_SHOWN)
                            .enumerate()
                            .map(|(row, item)| {
                                let is_selected = item.id == selected;
                                let detail = item.detail.clone();
                                view! {
                                    <li
                                        class="combo-option"
                                        role="option"
                                        id=option_id(row)
                                        data-value=item.id.clone()
                                        aria-selected=if is_selected { "true" } else { "false" }
                                        class:active=move || active.get() == row
                                        on:mousemove=move |_| if active.get_untracked() != row { active.set(row) }
                                        on:click=move |_| choose(row)
                                    >
                                        <span class="combo-label">{item.label.clone()}</span>
                                        {(!detail.is_empty()).then(|| view! { <small>{detail}</small> })}
                                        {is_selected.then(|| view! { <Icon name="check"/> })}
                                    </li>
                                }
                            })
                            .collect_view()
                    })
                };
                view! {
                    <div class="combo-pop" on:mousedown=keep_focus>
                        <ul class="combo-list scroll" id=list_id.clone() role="listbox" aria-label=label>{entries}</ul>
                        <div class="combo-foot"><kbd>"↑"</kbd><kbd>"↓"</kbd>" "{t.combobox.key_move}" "<kbd>"Enter"</kbd>" "{t.combobox.key_take}" "<kbd>"Esc"</kbd>" "{t.combobox.key_close}</div>
                    </div>
                }
            })
        }
    };

    view! {
        <div
            class="combo select"
            class:small=small
            node_ref=root
            data-open=move || open.get().then_some("true")
            style=move || open.get().then(|| place.get().map(|place| place.style())).flatten()
            data-wide=move || (open.get() && place.get().is_some_and(|place| place.wide.is_some())).then_some("")
            on:focusout=on_focus_out
        >
            <button
                type="button"
                class="combo-trigger"
                id=id
                role="combobox"
                aria-haspopup="listbox"
                data-command=label
                data-command-state=move || current.get().map(|(name, _)| name)
                aria-expanded=move || if open.get() { "true" } else { "false" }
                aria-controls=list_id
                aria-activedescendant=move || open.get().then(|| option_id(active.get()))
                aria-label=move || match current.get() {
                    Some((name, _)) => format!("{label}: {name}"),
                    None => format!("{label}: {placeholder}"),
                }
                // What is selected, as a `<select>` has its value (the checks read it).
                data-value=move || selected.get()
                disabled=move || disabled.get()
                on:click=move |_| {
                    if open.get_untracked() {
                        hide();
                    } else {
                        show(None);
                    }
                    nav::focus_by_id(id);
                }
                on:keydown=on_key
            >
                {icon.map(|name| view! { <Icon name=name/> })}
                <span class="combo-value" class:placeholder=move || current.get().is_none()>
                    {move || match current.get() {
                        Some((name, detail)) => view! { {name}{(!detail.is_empty()).then(|| view! { <small>{detail}</small> })} }.into_any(),
                        None => placeholder.into_any(),
                    }}
                </span>
                <Icon name="chevrons-up-down" class="combo-chevron"/>
            </button>
            {popup}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use leptos::prelude::*;

    use super::{grouped, is_open, rows_of, typed_row, with_first, ComboItem};

    #[test]
    fn a_combobox_that_is_gone_is_closed() {
        let owner = Owner::new();
        let open = owner.with(|| RwSignal::new(true));
        assert!(is_open(open));
        open.set(false);
        assert!(!is_open(open));
        open.set(true);
        // Its owner disposed, as when `on_select` took it off the page: closed, no panic.
        owner.cleanup();
        assert!(!is_open(open));
    }

    #[test]
    fn a_heading_never_comes_twice_and_the_best_group_comes_first() {
        let groups = ["", "Nebenfach", "", "Nebenfach", "Informatik-Vertiefung"];
        let group = |index: usize| groups.get(index).copied().unwrap_or_default();
        // Ranked by the search: a Nebenfach entry fits best, so the Nebenfach comes first, then
        // the entries without a heading, then the rest.
        assert_eq!(grouped(&[3, 0, 4, 1, 2], group), vec![3, 1, 0, 2, 4]);
        // Without a search the order stays as given …
        assert_eq!(grouped(&[0, 2, 1, 3, 4], group), vec![0, 2, 1, 3, 4]);
        // … and without groups the ranking stays as it is.
        assert_eq!(grouped(&[2, 0], |_| ""), vec![2, 0]);
    }

    #[test]
    fn the_first_entry_stands_above_nothing_selected() {
        let programs = vec![ComboItem::new("bwl", "BWL", "B.Sc. · PO 2021", 1), ComboItem::new("inf", "Informatik", "B.Sc. · PO 2008", 1)];
        let items = with_first(&programs, Some("inf"), "Mein Studiengang");
        assert_eq!(items.iter().map(|item| (item.id.as_str(), item.group.as_str(), item.first)).collect::<Vec<_>>(), [("inf", "Mein Studiengang", true), ("bwl", "", false), ("inf", "", false)]);
        // Nothing typed: „Mein Studiengang", then „Alle Studiengänge", then all of them in their order.
        assert_eq!(rows_of(&items, "", true), [Some(0), None, Some(1), Some(2)]);
        assert_eq!(rows_of(&items, " ", false), [Some(0), Some(1), Some(2)]);
        // Typed: what fits best, „Mein Studiengang" by its heading when it is among it.
        assert_eq!(rows_of(&items, "bwl", true), [Some(1)]);
        assert_eq!(rows_of(&items, "inf", true), [Some(0), Some(2)]);
        // None set, or one the picker does not know: the items as they were, nothing selected first.
        assert_eq!(with_first(&programs, None, "Mein Studiengang"), programs);
        assert_eq!(with_first(&programs, Some("weg"), "Mein Studiengang"), programs);
        assert_eq!(rows_of(&programs, "", true), [None, Some(0), Some(1)]);
    }

    #[test]
    fn letters_lead_to_the_entry_they_begin() {
        let starts: Vec<ComboItem> = ["WiSe 2026/27", "SoSe 2026", "WiSe 2025/26", "SoSe 2025"].iter().map(|label| ComboItem::new(*label, *label, "", 0)).collect();
        // One letter again and again: from one entry it begins to the next, round the end.
        assert_eq!(typed_row(&starts, "s", 0), Some(1));
        assert_eq!(typed_row(&starts, "ss", 1), Some(3));
        assert_eq!(typed_row(&starts, "s", 3), Some(1));
        // Several letters: the first entry from the one marked on that begins with all of them.
        assert_eq!(typed_row(&starts, "wise 2025", 0), Some(2));
        assert_eq!(typed_row(&starts, "WISE 2026", 1), Some(0));
        // Nothing begins so, or nothing typed.
        assert_eq!(typed_row(&starts, "x", 0), None);
        assert_eq!(typed_row(&starts, "", 0), None);
        assert_eq!(typed_row(&[], "w", 0), None);
    }
}
