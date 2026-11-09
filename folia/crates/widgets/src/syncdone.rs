//! Connecting two devices of a profile while it happens and once it is done (owner, 2026-11-09, of
//! the concepts in folia/design/verbunden/konzepte.html: „Ja nimm bitte option a. Aber mach mal eine
//! animation mit progress bar für den sync an sich. So dass man sieht, dass da was passiert").
//!
//! - `ConnectProgress`: the steps of the way as one bar and a list under it — done ones ticked, the
//!   one under way with a pulse, those to come faint. The host says how far it really is
//!   (`reached`); the bar follows a step at a time and shows each for a moment at least
//!   (`STEP_MS`), so that steps of a few milliseconds are seen as well and nothing flashes, and
//!   within the step under way it creeps on slowly, so that a wait looks like work. Once all are
//!   done it says so for a moment (`DONE_MS`) and hands over (`on_done`).
//! - `Connected`: concept A, the success in place of what led to it: a tick, „Verbunden" (from the
//!   setup „Alles eingerichtet" with what was set up), what is on now in three rows, that more
//!   devices are connected on the profile page (owner, the same day: „du kannst später weitere
//!   geräte paaren im profil"), and the host's „Fertig".

use std::time::Duration;

use leptos::prelude::*;

use folia_design::ui::Icon;

use crate::i18n;
use crate::syncpair::now_ms;

/// How long each step shows at least (ms).
const STEP_MS: u64 = 550;
/// How long the full bar shows before what follows (ms).
const DONE_MS: u64 = 450;

/// A step of connecting, as the bar names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// A private profile switched on for it.
    SwitchOn,
    /// The profile's key sealed and left at Phloem.
    Send,
    /// The other device fetching it (the giver waits for it).
    Fetch,
    /// The profile coming here (the device that gets it).
    Receive,
    /// The devices bringing their data in line.
    Sync,
}

impl Phase {
    fn said(self) -> &'static str {
        let s = &i18n::t().syncpair;
        match self {
            Phase::SwitchOn => s.step_switch_on,
            Phase::Send => s.step_send,
            Phase::Fetch => s.step_fetch,
            Phase::Receive => s.step_receive,
            Phase::Sync => s.step_sync,
        }
    }
}

/// The pace of the bar: the step shown, since when, and whether a step is on its way.
#[derive(Clone, Copy)]
struct Pace {
    shown: RwSignal<usize>,
    since: StoredValue<u64>,
    waiting: StoredValue<bool>,
    reached: Signal<usize>,
    steps: usize,
    on_done: Callback<()>,
}

/// One step on, once the step shown had its moment and the way has got further; then the next.
fn advance(pace: Pace) {
    if pace.waiting.try_get_value().unwrap_or(true) {
        return;
    }
    let (Some(at), Some(reached)) = (pace.shown.try_get_untracked(), pace.reached.try_get_untracked()) else { return };
    if at >= pace.steps || reached <= at {
        return;
    }
    let shown_for = now_ms().saturating_sub(pace.since.try_get_value().unwrap_or(0));
    pace.waiting.set_value(true);
    set_timeout(
        move || {
            if pace.shown.try_get_untracked().is_none() {
                return;
            }
            pace.waiting.set_value(false);
            pace.since.set_value(now_ms());
            pace.shown.set(at + 1);
            if at + 1 >= pace.steps {
                set_timeout(move || pace.on_done.run(()), Duration::from_millis(DONE_MS));
            } else {
                advance(pace);
            }
        },
        Duration::from_millis(STEP_MS.saturating_sub(shown_for)),
    );
}

/// The bar and its steps. `reached`: the step under way (an index of `phases`), `phases.len()`
/// once all are done; `on_done` once the bar has shown that.
#[component]
pub fn ConnectProgress(phases: Vec<Phase>, #[prop(into)] reached: Signal<usize>, on_done: Callback<()>) -> impl IntoView {
    let s = &i18n::t().syncpair;
    let steps = phases.len().max(1);
    let pace = Pace { shown: RwSignal::new(0), since: StoredValue::new(now_ms()), waiting: StoredValue::new(false), reached, steps, on_done };
    Effect::new(move |_| {
        reached.track();
        advance(pace);
    });
    let shown = pace.shown;
    let said = {
        let phases = phases.clone();
        move || phases.get(shown.get()).map_or(s.connected, |phase| phase.said())
    };
    // The bar: what is done, and the step under way as a segment that fills slowly (made anew for
    // each step, so that its fill begins again).
    let bar = move || {
        let at = shown.get();
        let done = at as f64 * 100.0 / steps as f64;
        let segment = (at < steps).then(|| {
            let width = 100.0 / steps as f64;
            view! { <b style=format!("left:{done:.3}%;width:{width:.3}%")></b> }
        });
        view! {
            <i style=format!("width:{done:.3}%")></i>
            {segment}
        }
    };
    let list = phases
        .iter()
        .enumerate()
        .map(|(index, phase)| {
            let said = phase.said();
            let state = move || match shown.get() {
                at if at > index => "done",
                at if at == index => "now",
                _ => "later",
            };
            view! {
                <li data-state=state>
                    <span class="cp-mark" aria-hidden="true">{move || (shown.get() > index).then(|| view! { <Icon name="check"/> })}</span>
                    <span>{said}</span>
                </li>
            }
        })
        .collect_view();
    view! {
        <div class="cp" role="progressbar" aria-label=s.progress_label aria-valuemin="0" aria-valuemax=steps.to_string() aria-valuenow=move || shown.get().to_string() aria-valuetext=said>
            <div class="cp-bar" aria-hidden="true">{bar}</div>
            <ol class="cp-steps">{list}</ol>
        </div>
    }
}

/// The success (concept A). `title`: „Verbunden" or, from the setup, „Alles eingerichtet"; `said`:
/// who has what now; `summary`: what the setup set up, a line each; `children`: „Fertig", where
/// the host has none of its own (a dialog's foot, the setup card's actions).
#[component]
pub fn Connected(title: &'static str, said: String, #[prop(optional)] summary: Vec<String>, #[prop(optional)] children: Option<Children>) -> impl IntoView {
    let s = &i18n::t().syncpair;
    let icons = ["monitor-smartphone", "zap", "lock"];
    view! {
        <div class="cn" role="status">
            <span class="cn-mark" aria-hidden="true"><Icon name="check"/></span>
            <h3 class="cn-title">{title}</h3>
            <p class="cn-said">{said}</p>
            {(!summary.is_empty()).then(|| view! {
                <ul class="cn-summary">{summary.into_iter().map(|line| view! { <li><Icon name="check"/><span>{line}</span></li> }).collect_view()}</ul>
            })}
            <p class="cn-label">{s.now_on}</p>
            <ul class="cn-gains">
                {icons.into_iter().zip(s.gains).map(|(icon, (gain, line))| view! {
                    <li><span class="cn-tile" aria-hidden="true"><Icon name=icon/></span><span><b>{gain}</b><small>{line}</small></span></li>
                }).collect_view()}
            </ul>
            <p class="cn-later"><Icon name="user-round"/><span>{s.later}<b>{s.later_where}</b>{s.later_after}</span></p>
            {children.map(|children| view! { <div class="cn-done">{children()}</div> })}
        </div>
    }
}

/// The steps of each way, as the hosts take them.
pub mod ways {
    use super::Phase;

    /// The device that gives its profile to the one whose code it read; a private profile is
    /// switched on first.
    pub fn giving(private: bool) -> Vec<Phase> {
        let mut phases = if private { vec![Phase::SwitchOn] } else { Vec::new() };
        phases.extend([Phase::Send, Phase::Fetch, Phase::Sync]);
        phases
    }

    /// The device that gets a profile (by its code or a link of the other's).
    pub fn getting() -> Vec<Phase> {
        vec![Phase::Receive, Phase::Sync]
    }

    /// The device whose link another device opened.
    pub fn handing() -> Vec<Phase> {
        vec![Phase::Fetch, Phase::Sync]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_way_ends_with_the_data_in_line() {
        for phases in [ways::giving(true), ways::giving(false), ways::getting(), ways::handing()] {
            assert_eq!(phases.last(), Some(&Phase::Sync), "{phases:?}");
        }
        assert_eq!(ways::giving(true).first(), Some(&Phase::SwitchOn));
        assert_eq!(ways::giving(false).first(), Some(&Phase::Send));
        for s in [&i18n::syncpair::DE, &i18n::syncpair::EN] {
            assert!(s.gains.iter().all(|(gain, line)| !gain.is_empty() && line.ends_with('.')));
        }
    }
}
