//! Vibrations: what the hand feels (owner, 2026-11-17: „ich möchte dass in der ganzen app vibrations
//! richtig umgesetzt werden und die Benutzung unterstreichen und satisfying macht";
//! docs/folia/frontend.md, „Vibrations"). One vocabulary for the whole app, played by `enhance.js`
//! (`window.betulaHaptic`), which knows the device, its patterns and whether this browser switched
//! them off (`betula.haptics.v1`). Each kind is for one kind of moment (`Haptic`); what flips or
//! picks (a switch, a segment, a tab) ticks there by itself, so the app plays what only it knows:
//! what a gesture crossed, what an action did and how it went.
//!
//! An iPhone has no Vibration API: it ticks only for a native switch that the finger's own tap
//! toggles. A button that should tick there carries `TapTick`, a label for such a switch laid over
//! it, shown on an iPhone alone (`html.tap-ticks`, app.css).

use leptos::prelude::*;

/// What the hand feels, from the lightest to the strongest: within a moment (`enhance.js`,
/// `HAPTIC_GAP`) the stronger of two is played and the same again is not felt twice, so an action
/// and the switch that started it are one feeling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Haptic {
    /// A pick or a detent: a step of ‹ ›, a finger past the point where letting go does something,
    /// a row joining a selection drawn; a mark taken away, lighter than one given.
    Tick,
    /// Something small given or taken back: a mark, „Rückgängig", a module taken out.
    Tap,
    /// A finger held long enough: what it holds is its own now.
    Hold,
    /// Something done that one built: planned, set up, copied, devices connected.
    Success,
    /// Done, but unusual (CLAUDE.md, „Betula does not say no"): planned where the module is not
    /// offered, into a semester behind the student.
    Warning,
    /// What cannot be: the Merkliste full, a code that is none of Betula's, a file refused.
    Error,
}

impl Haptic {
    /// Its name for `enhance.js` (and `data-haptic`).
    pub fn name(self) -> &'static str {
        match self {
            Haptic::Tick => "tick",
            Haptic::Tap => "tap",
            Haptic::Hold => "hold",
            Haptic::Success => "success",
            Haptic::Warning => "warning",
            Haptic::Error => "error",
        }
    }

    /// Done, and `unusual` or not: a warning where it is.
    pub fn done(unusual: bool) -> Self {
        if unusual {
            Haptic::Warning
        } else {
            Haptic::Success
        }
    }
}

/// Plays `kind` where the device can and this browser has not switched it off; nothing on the
/// server, nothing without `enhance.js`.
pub fn play(kind: Haptic) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let Some(window) = web_sys::window() else { return };
        let Some(play) = js_sys::Reflect::get(&window, &"betulaHaptic".into()).ok().and_then(|play| play.dyn_into::<js_sys::Function>().ok()) else { return };
        let _ = play.call1(&window, &kind.name().into());
    }
    #[cfg(not(feature = "csr"))]
    let _ = kind;
}

/// Where this browser keeps the switch for vibrations: `off`, or nothing for on (a setting of the
/// browser, the same in every profile, as light and dark). `enhance.js` writes it
/// (`data-action="haptics-set"`) and says so (`betula:haptics`).
pub const HAPTICS_KEY: &str = "betula.haptics.v1";

/// Whether vibrations are on in this browser; on where nothing says otherwise.
pub fn on() -> bool {
    crate::nav::local_get(HAPTICS_KEY).as_deref() != Some("off")
}

/// The switch an iPhone's tap toggles for its tick: a label as large as the button it lies in
/// (which must be a `type="button"` with a place of its own, app.css `.tap-tick`), shown on an
/// iPhone alone. The tap lands on the label, whose click is the button's as any click inside it;
/// the switch's own click, which the label hands on, stops there, or the button would hear it
/// twice. Nobody but the hand knows of it: no stop of the Tab key, nothing a screen reader reads.
#[component]
pub fn TapTick() -> impl IntoView {
    use leptos::tachys::html::attribute::custom::CustomAttribute;
    // `switch` is no attribute the view macro knows: given by its name.
    let switch = leptos::html::input().r#type("checkbox").attr("switch", "").tabindex(-1).on(leptos::ev::click, |ev| ev.stop_propagation());
    view! { <label class="tap-tick" aria-hidden="true">{switch}</label> }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names are the ones `enhance.js` knows (HAPTICS), and a done unusual is a warning.
    #[test]
    fn names_and_what_done_says() {
        let names: Vec<&str> = [Haptic::Tick, Haptic::Tap, Haptic::Hold, Haptic::Success, Haptic::Warning, Haptic::Error].map(Haptic::name).to_vec();
        assert_eq!(names, ["tick", "tap", "hold", "success", "warning", "error"]);
        let script = include_str!("../../../assets/enhance.js");
        for name in names {
            assert!(script.contains(&format!("    {name}: [")), "{name}");
        }
        assert_eq!(Haptic::done(true), Haptic::Warning);
        assert_eq!(Haptic::done(false), Haptic::Success);
    }
}
