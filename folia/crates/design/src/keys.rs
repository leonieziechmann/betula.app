//! Keys as Betula writes them (owner, 2026-10-20: „Mach das in general, dass kombis mit + zwischen
//! den keys angezeigt wird oder halt mit icon wie bei apple typisch"; „verwende shift + enter. Das
//! versteht jeder und ist sogar klarer"): on Windows and Linux each key of a combination is a key
//! cap of its own with „+" between them (Strg + K, Shift + Enter), on a Mac the system's signs run
//! together in one cap (⌘K, ⇧↩). Both are written into the page; the stylesheet shows the one of
//! the system (`html.mac`, which the head script sets before the page is drawn), so nothing changes
//! when the app takes over. enhance.js writes keys the same way (`caps`, the keyboard's section).
//!
//! A key is written as its spec: the keys of a combination joined by „+", each as it is named —
//! `Mod+K`, `Shift+Enter`, `Esc`, `F`, `?`, `↑`. `Mod` is Strg (Ctrl in English) or ⌘, `Del` Entf
//! or ⌫, `Home` and `End` Pos1 and Ende, `PgUp` and `PgDn` Bild ↑ and Bild ↓; a key not named
//! here is written as it is („F6", „0–9"). A spec of a single „+" is the plus key. `Nav+1` is the
//! navigation's keys before a digit, which differ by system and are written by the stylesheet
//! (`--nav-keys`, enhance.js; `.nav-combo`).

use leptos::prelude::*;

use crate::i18n::{self, Locale};

/// The search's key (`ui::Shortcut` in the search field, on the start page; folia/design/tastatur/
/// konzept.html, question 2: „/ bleibt das Suchfeld und steht darin").
pub const SEARCH: &str = "/";

/// The keys of the commands (`folia_shell::commands`: „Befehle" in the search field), which find
/// modules as well.
pub const COMMANDS: &str = "Mod+K";

/// What a key is called on a PC in a language, and on a Mac (where the sign is the same in every
/// language).
fn name(key: &str, locale: Locale, mac: bool) -> String {
    let named = match (key, mac, locale) {
        ("Mod", false, Locale::De) | ("Ctrl", false, Locale::De) => "Strg",
        ("Mod", false, Locale::En) | ("Ctrl", false, Locale::En) => "Ctrl",
        ("Mod", true, _) => "⌘",
        ("Ctrl", true, _) => "⌃",
        ("Alt", true, _) => "⌥",
        ("Shift", true, _) => "⇧",
        ("Enter", true, _) => "↩",
        ("Del", true, _) => "⌫",
        ("Del", false, Locale::De) => "Entf",
        ("Del", false, Locale::En) => "Del",
        ("Home", true, _) => "fn ←",
        ("End", true, _) => "fn →",
        ("PgUp", true, _) => "fn ↑",
        ("PgDn", true, _) => "fn ↓",
        ("Home", false, Locale::De) => "Pos1",
        ("End", false, Locale::De) => "Ende",
        ("PgUp", false, Locale::De) => "Bild ↑",
        ("PgDn", false, Locale::De) => "Bild ↓",
        ("PgUp", false, Locale::En) => "Page ↑",
        ("PgDn", false, Locale::En) => "Page ↓",
        ("Space", _, Locale::De) => "Leertaste",
        ("Space", _, Locale::En) => "Space",
        (other, ..) => other,
    };
    named.to_string()
}

/// The keys of a spec, each as it is called (`name`).
pub fn names(spec: &str, locale: Locale, mac: bool) -> Vec<String> {
    if spec == "+" || !spec.contains('+') {
        return vec![name(spec, locale, mac)];
    }
    spec.split('+').filter(|key| !key.is_empty()).map(|key| name(key, locale, mac)).collect()
}

/// A spec as words, for a tooltip or what a screen reader is told: „Strg + K", on a Mac „⌘K".
pub fn text(spec: &str, locale: Locale, mac: bool) -> String {
    let keys = names(spec, locale, mac);
    if mac { keys.concat() } else { keys.join(" + ") }
}

/// The same for the system the browser app runs on (a tooltip, which no stylesheet can choose; on
/// the server the PC's).
pub fn text_here(spec: &str) -> String {
    text(spec, i18n::locale(), crate::ui::is_mac())
}

/// Whether a spec is one of Betula's keys of a single character (F, V, M, P, /, ?, with Shift as
/// well): the switch for single keys turns those off, and their hints with them (WCAG 2.1.4; the
/// root's `data-letters="off"`, app.css). The arrows are no such key, and neither is a key that
/// only works while its control has the focus (+ and − at a chip).
pub fn is_letter(spec: &str) -> bool {
    let key = spec.strip_prefix("Shift+").unwrap_or(spec);
    let mut chars = key.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_ascii_alphanumeric() || c == '/' || c == '?')
}

/// A spec as key caps: on a PC each key a cap, „+" between them; on a Mac one cap of signs. Both
/// are written where they differ, and the stylesheet shows the one of the system. A single key of
/// Betula's own carries `k-letter`, which the switch for single keys hides.
#[component]
pub fn Keys(#[prop(into)] spec: String) -> impl IntoView {
    let spec = spec.as_str();
    // The navigation's keys before a digit: what they are the stylesheet writes (`.nav-combo`).
    if let Some(digit) = spec.strip_prefix("Nav+") {
        return view! { <kbd class="nav-combo">{digit.to_string()}</kbd> }.into_any();
    }
    let locale = i18n::locale();
    let pc = names(spec, locale, false);
    let mac = names(spec, locale, true).concat();
    let letter = if is_letter(spec) { " k-letter" } else { "" };
    match pc.as_slice() {
        [one] if *one == mac => view! { <kbd class=format!("k{letter}")>{mac}</kbd> }.into_any(),
        [one] => view! {
            <kbd class=format!("k k-pc{letter}")>{one.clone()}</kbd>
            <kbd class=format!("k k-mac{letter}")>{mac}</kbd>
        }
        .into_any(),
        keys => {
            let caps = keys.iter().enumerate().map(|(i, key)| view! { {(i > 0).then(|| view! { <i class="plus">"+"</i> })}<kbd>{key.clone()}</kbd> }).collect_view();
            view! {
                <span class=format!("kk k-pc{letter}")>{caps}</span>
                <kbd class=format!("k k-mac{letter}")>{mac}</kbd>
            }
            .into_any()
        }
    }
}

/// Where this browser keeps the switch for single keys: `off`, or nothing for on (R13: a personal
/// setting of the browser, the same in every profile). The head script reads it before the page is
/// drawn (`data-letters` on the root), enhance.js writes it (`data-action="letters-set"`).
pub const LETTERS_KEY: &str = "betula.letters.v1";

/// Whether Betula's single keys are on (the root's `data-letters`); on the server, where no
/// browser has said otherwise, they are.
pub fn letters_on() -> bool {
    #[cfg(feature = "csr")]
    let on = web_sys::window().and_then(|window| window.document()).and_then(|document| document.document_element()).is_none_or(|root| root.get_attribute("data-letters").as_deref() != Some("off"));
    #[cfg(not(feature = "csr"))]
    let on = true;
    on
}

/// The switch for Betula's single keys (WCAG 2.1.4; folia/design/tastatur/konzept.html, question
/// 4: on unless switched off): in the overview of the keys and in the profile. enhance.js keeps it
/// (`data-action="letters-set"`) and says so (`betula:letters`). `labelled_by` names the element
/// that names it.
#[component]
pub fn LettersSwitch(labelled_by: &'static str) -> impl IntoView {
    let t = i18n::t();
    let on = RwSignal::new(letters_on());
    // Effects run in the browser only.
    Effect::new(move |_| {
        on.set(letters_on());
        let handle = window_event_listener_untyped("betula:letters", move |_| on.set(letters_on()));
        on_cleanup(move || handle.remove());
    });
    let checked = move |want: bool| if on.get() == want { "true" } else { "false" };
    view! {
        <div class="seg letters-seg" role="radiogroup" aria-labelledby=labelled_by>
            <button type="button" role="radio" data-action="letters-set" data-letters-value="on" aria-checked=move || checked(true)>{t.ui.letters_on}</button>
            <button type="button" role="radio" data-action="letters-set" data-letters-value="off" aria-checked=move || checked(false)>{t.ui.letters_off}</button>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A combination is written with „+" on a PC and as the Mac's signs on a Mac; a single key as
    /// it is called; Strg is Ctrl in English.
    #[test]
    fn keys_as_each_system_writes_them() {
        assert_eq!(text("Mod+K", Locale::De, false), "Strg + K");
        assert_eq!(text("Mod+K", Locale::En, false), "Ctrl + K");
        assert_eq!(text("Mod+K", Locale::De, true), "⌘K");
        assert_eq!(text("Shift+Enter", Locale::De, false), "Shift + Enter");
        assert_eq!(text("Shift+Enter", Locale::De, true), "⇧↩");
        assert_eq!(text("Shift+M", Locale::En, true), "⇧M");
        assert_eq!(text("Del", Locale::De, false), "Entf");
        assert_eq!(text("PgUp", Locale::De, false), "Bild ↑");
        assert_eq!(text("+", Locale::De, false), "+");
        assert_eq!(text("Mod+F6", Locale::De, false), "Strg + F6");
        assert_eq!(text("Esc", Locale::En, true), "Esc");
    }

    /// The single keys of Betula's own are letters, a slash and a question mark, with or without
    /// Shift; the arrows, Esc, the chip's + and combinations with Strg are not.
    #[test]
    fn which_keys_are_single_letters() {
        for spec in ["F", "V", "M", "Shift+M", "P", "Shift+P", "/", "?"] {
            assert!(is_letter(spec), "{spec}");
        }
        for spec in ["↑", "Esc", "Enter", "Mod+K", "+", "−", "Del", "F6", "0–9", "Shift+Enter"] {
            assert!(!is_letter(spec), "{spec}");
        }
    }
}
