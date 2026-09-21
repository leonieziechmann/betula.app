//! A picker with a search field, for long lists (programs, lecturers, departments).
//!
//! Type a few letters (word starts, initials and typos are understood, `catalog::fuzzy`), move
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

use catalog::fuzzy;
use leptos::ev::{FocusEvent, KeyboardEvent, MouseEvent};
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use leptos::web_sys;

use crate::nav::{self, PopupPlace};
use crate::ui::Icon;

/// The ranked entries with each group together, so that no heading comes twice: the entries
/// without a group first (the list's own, they have no heading to stand under), then the groups
/// in the order of their best entry, each in the order of the ranking.
fn grouped<'a>(ranked: &[usize], group: impl Fn(usize) -> &'a str) -> Vec<usize> {
    let mut groups: Vec<(&str, Vec<usize>)> = vec![("", Vec::new())];
    for &index in ranked {
        let name = group(index);
        match groups.iter_mut().find(|(known, _)| *known == name) {
            Some((_, members)) => members.push(index),
            None => groups.push((name, vec![index])),
        }
    }
    groups.into_iter().flat_map(|(_, members)| members).collect()
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
    search: String,
}

impl ComboItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>, detail: impl Into<String>, bonus: i64) -> Self {
        let (label, detail) = (label.into(), detail.into());
        Self { id: id.into(), search: format!("{label} {detail}"), label, detail, bonus, group: String::new() }
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

#[component]
pub fn Combobox(
    /// Id of the button; the parts of the popup derive theirs from it.
    id: &'static str,
    /// What is being picked, for screen readers: „Studiengang".
    label: &'static str,
    /// The button's text while nothing is selected. With `clearable` also the first entry.
    placeholder: &'static str,
    search_placeholder: &'static str,
    #[prop(into)] items: Signal<Vec<ComboItem>>,
    /// The id of the selected entry.
    #[prop(into)] selected: Signal<Option<String>>,
    /// The id of the picked entry; `None` when the selection was cleared.
    #[prop(into)] on_select: Callback<Option<String>>,
    #[prop(optional)] icon: Option<&'static str>,
    /// Offers „nothing selected" as the first entry and as a button next to the selection.
    #[prop(default = true)] clearable: bool,
    /// The popup is as wide as the button, but not narrower than this (long names need room).
    #[prop(default = 340.0)] min_width: f64,
) -> impl IntoView {
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
        items.with(|items| {
            let mut rows: Vec<Option<usize>> = Vec::new();
            if clearable && text.trim().is_empty() {
                rows.push(None);
            }
            let ranked = fuzzy::rank(&text, items.iter().map(|item| (item.search.as_str(), item.bonus)));
            rows.extend(grouped(&ranked, |index| items.get(index).map(|item| item.group.as_str()).unwrap_or_default()).into_iter().map(Some));
            rows
        })
    });
    let shown = move || rows.with(|rows| rows.len().min(MAX_SHOWN));

    let current = Memo::new(move |_| {
        let id = selected.get()?;
        items.with(|items| items.iter().find(|item| item.id == id).map(|item| (item.label.clone(), item.detail.clone())))
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
        if open.get_untracked() {
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
    // the field into view, which used to close the popup the moment it opened.
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
            }
        });
        on_cleanup(move || handle.remove());
    });

    let toggle = {
        let show = show.clone();
        move |_: MouseEvent| if open.get_untracked() { hide(true) } else { show() }
    };
    let on_trigger_key = {
        let show = show.clone();
        move |ev: KeyboardEvent| {
            if matches!(ev.key().as_str(), "ArrowDown" | "ArrowUp") && !open.get_untracked() {
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
            let style = place.get().map(|place| place.style()).unwrap_or_default();
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
                        let heading = item
                            .as_ref()
                            .map(|item| item.group.clone())
                            .filter(|group| !group.is_empty() && *group != last_group)
                            .map(|group| {
                                last_group.clone_from(&group);
                                view! { <li class="combo-group" role="presentation">{group}</li> }
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
                let none = (shown() == 0).then(|| view! { <li class="combo-empty" role="presentation">"Nichts gefunden"</li> });
                let more = (hidden > 0).then(|| view! { <li class="combo-empty" role="presentation">{format!("{hidden} weitere – tippe, um sie zu finden")}</li> });
                (entries, none, more)
            };
            view! {
                <div class="combo-pop" style=style on:mousedown=keep_focus>
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
                    <div class="combo-foot"><kbd>"↑"</kbd><kbd>"↓"</kbd>" wählen "<kbd>"Enter"</kbd>" übernehmen "<kbd>"Esc"</kbd>" schließen"</div>
                </div>
            }
        })
    };

    view! {
        <div class="combo" node_ref=root data-open=move || open.get().then_some("true") on:focusout=on_focus_out>
            <button
                type="button"
                class="combo-trigger"
                id=id
                aria-haspopup="listbox"
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
                <button type="button" class="combo-clear" aria-label=format!("{label}: Auswahl aufheben") on:click=move |_| { hide(false); on_select.run(None); }>
                    <Icon name="x"/>
                </button>
            })}
            {popup}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::grouped;

    #[test]
    fn a_heading_never_comes_twice_and_the_entries_without_one_come_first() {
        let groups = ["", "Nebenfach", "", "Nebenfach", "Informatik-Vertiefung"];
        let group = |index: usize| groups.get(index).copied().unwrap_or_default();
        // Ranked by the search: a Nebenfach entry fits best, then one without a heading …
        assert_eq!(grouped(&[3, 0, 4, 1, 2], group), vec![0, 2, 3, 1, 4]);
        // … and without groups the ranking stays as it is.
        assert_eq!(grouped(&[2, 0], |_| ""), vec![2, 0]);
    }
}
