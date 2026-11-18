//! The link for the next device of Betula Sync („Gerät verbinden"): its QR code, the link to copy
//! or to hand to the system's sharing where it has one, how long it still opens, and once it is gone
//! „Neuer Link"; and what a problem of Betula
//! Sync says (`folia_stores::sync::Problem`). The profile page's part shows it, and the setup of
//! the start page in its last step (owner, 2026-11-08: „es gibt ja jetzt betula sync. Mach das mal
//! als step in die einführung mit rein … So dass man es gleich auch auf dem handy hat").
//!
//! The link is made in the browser and never reaches a server: what follows `#` in it is the key
//! (docs/folia/sync.md, „The actions").

use leptos::prelude::*;

use folia_design::haptic::{self, Haptic};
use folia_design::ui::Icon;
use folia_stores::profiles::MAX_PROFILES;
use folia_stores::sync::{Pairing, Problem};

use crate::i18n;

/// How long „Kopiert" stands in the button's place.
pub const COPIED_MS: u64 = 1600;

/// The sentence of a problem, with what to do.
pub fn problem_text(problem: Problem) -> String {
    let s = &i18n::t().syncpair;
    match problem {
        Problem::Budget => s.budget.to_string(),
        Problem::SlowDown => s.slow_down.to_string(),
        Problem::Full => s.full.to_string(),
        Problem::BadLink => s.bad_link.to_string(),
        Problem::Unreachable => s.unreachable.to_string(),
        Problem::Unsupported => s.unsupported.to_string(),
        Problem::NoRoom => (s.no_room)(MAX_PROFILES),
    }
}

/// The milliseconds since 1970 of the browser's clock; 0 on the server, which reads none.
pub fn now_ms() -> u64 {
    #[cfg(feature = "csr")]
    {
        web_sys::js_sys::Date::now().max(0.0) as u64
    }
    #[cfg(not(feature = "csr"))]
    0
}

/// Copies a text into the clipboard (a click's: the browser lets it), and the hand feels it done.
pub fn copy(text: &str) {
    #[cfg(feature = "csr")]
    if let Some(window) = web_sys::window() {
        let _ = window.navigator().clipboard().write_text(text);
    }
    #[cfg(not(feature = "csr"))]
    let _ = text;
    haptic::play(Haptic::Success);
}

/// Whether the system can share a link (the Web Share API: phones, and some desktops).
pub fn can_share() -> bool {
    #[cfg(feature = "csr")]
    {
        web_sys::window().is_some_and(|window| web_sys::js_sys::Reflect::has(&window.navigator(), &"share".into()).unwrap_or(false))
    }
    #[cfg(not(feature = "csr"))]
    false
}

/// Hands a link to the system's sharing (a click's: the browser lets it).
pub fn share(link: &str, title: &str) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        use web_sys::js_sys::{Function, Object, Reflect};
        let Some(navigator) = web_sys::window().map(|window| window.navigator()) else { return };
        let Some(share) = Reflect::get(&navigator, &"share".into()).ok().and_then(|share| share.dyn_into::<Function>().ok()) else { return };
        let data = Object::new();
        let _ = Reflect::set(&data, &"url".into(), &link.into());
        let _ = Reflect::set(&data, &"title".into(), &title.into());
        // A share the visitor cancels rejects its promise; nothing to do then.
        let _ = share.call1(&navigator, &data);
    }
    #[cfg(not(feature = "csr"))]
    let _ = (link, title);
}

/// The minutes a link still opens, rounded down; `None` once it opens no more.
fn minutes_left(until: u64, now: u64) -> Option<u64> {
    (until > now).then(|| (until - now) / 60_000)
}

/// The link for the next device: its QR code, how long it still opens under it, what to do and
/// that it opens once, the link on one line, „Teilen" where the system can and „Link kopieren";
/// gone, the code faded under the sign to renew it and „Neuer Link" (`again`). `close`, where there
/// is one, puts it away („Schließen").
///
/// In order of weight (owner, 2026-11-18: „Bitte ordne das und mach visuelle Hierarchie"): the code
/// first and largest, then what to do, the rest smaller. In the dialog „Gerät verbinden" the code
/// stands in the middle without a box of its own, and on a phone „Link kopieren" is a bar across the
/// sheet's foot with „Teilen" over it (owner, the same day: „link kopieren unten als breiten balken
/// und der teilen button mit dem betula üblichen icon darüber"; app.css, „The link for the next
/// device"). The link keeps all of itself in the page, shown on one line.
#[component]
pub fn PairLink(pairing: Pairing, again: Callback<()>, #[prop(optional)] close: Option<Callback<()>>) -> impl IntoView {
    let s = &i18n::t().syncpair;
    let now = RwSignal::new(now_ms());
    // The minutes left, counted while it is shown.
    if let Ok(handle) = set_interval_with_handle(move || now.set(now_ms()), std::time::Duration::from_secs(5)) {
        on_cleanup(move || handle.clear());
    }
    let until = pairing.until;
    let left = Memo::new(move |_| minutes_left(until, now.get()));
    let copied = RwSignal::new(false);
    let link = pairing.link.clone();
    let copy_it = move |_| {
        copy(&link);
        copied.set(true);
        set_timeout(move || { let _ = copied.try_set(false); }, std::time::Duration::from_millis(COPIED_MS));
    };
    let code = folia_design::qr::path(&pairing.link);
    let shown = pairing.link.clone();
    let shared = pairing.link.clone();
    let sharing = can_share();
    let close_button = move || close.map(|close| view! { <button class="mini hit" type="button" on:click=move |_| close.run(())>{s.close}</button> });
    move || {
        if left.get().is_none() {
            // Gone, in the same place (owner, 2026-11-18: „wenn das Ding dann abgelaufen ist möchte
            // ich noch einen retry knopf"): the old code faded under the sign to renew it, which a
            // tap renews as the button does; that it is gone; „Neuer Link" where „Link kopieren"
            // stood. The faded code is the pointer's alone: the keys reach the button.
            let faded = code.clone().map(|(path, width)| view! {
                <svg class="pf-qr" viewBox=format!("0 0 {width} {width}") aria-hidden="true" shape-rendering="crispEdges">
                    <path d=path/>
                </svg>
            });
            return view! {
                <div class="pf-pair pf-link" data-gone="">
                    <div class="pf-link-code">
                        <button class="pf-link-veil" type="button" tabindex="-1" aria-hidden="true" on:click=move |_| again.run(())>
                            {faded}
                            <span class="pf-link-renew"><Icon name="refresh-cw"/></span>
                        </button>
                    </div>
                    <div class="pf-pair-text">
                        <p class="pf-link-how" role="status">{s.pair_gone}</p>
                        <p class="pf-link-hint">{s.pair_gone_hint}</p>
                        <div class="pf-pair-actions">
                            <button class="btn primary pf-link-new" type="button" on:click=move |_| again.run(())>
                                <Icon name="refresh-cw"/><span>{s.pair_new}</span>
                            </button>
                            {close_button()}
                        </div>
                    </div>
                </div>
            }
            .into_any();
        }
        let copy_it = copy_it.clone();
        let qr = code.clone().map(|(path, width)| view! {
            <svg class="pf-qr" viewBox=format!("0 0 {width} {width}") role="img" aria-label=s.qr_label shape-rendering="crispEdges">
                <path d=path/>
            </svg>
        });
        view! {
            <div class="pf-pair pf-link">
                <div class="pf-link-code">
                    {qr}
                    <p class="pf-link-left" role="status" data-low=move || left.get().is_some_and(|minutes| minutes < 2).then_some("")>
                        <Icon name="clock-3"/><span>{move || left.get().map(s.pair_left)}</span>
                    </p>
                </div>
                <div class="pf-pair-text">
                    <p class="pf-link-how">{s.pair_how}</p>
                    <p class="pf-link-once"><Icon name="lock"/><span>{s.pair_once}</span></p>
                    <p class="pf-pair-link"><Icon name="link"/><code translate="no">{shown.clone()}</code></p>
                    <div class="pf-pair-actions">
                        {sharing.then(|| {
                            let shared = shared.clone();
                            view! {
                                <button class="btn secondary pf-link-share" type="button" on:click=move |_| share(&shared, s.share_title)>
                                    <Icon name="share-2"/><span>{s.share}</span>
                                </button>
                            }
                        })}
                        <button class="btn primary pf-link-copy" type="button" on:click=copy_it>
                            <Icon name=if copied.get() { "check" } else { "copy" }/>
                            <span>{move || if copied.get() { s.copied } else { s.copy_link }}</span>
                        </button>
                        {close_button()}
                    </div>
                </div>
            </div>
        }
        .into_any()
    }
}

/// While a link is made (`said`: what happens): the link's frame with a blank where its code comes,
/// so that the code, a new one after „Neuer Link" too, comes in its place and nothing jumps.
#[component]
pub fn PairWait(said: &'static str) -> impl IntoView {
    view! {
        <div class="pf-pair pf-link" aria-busy="true">
            <div class="pf-link-code"><span class="pf-qr pf-link-blank sk-sweep" aria-hidden="true"></span></div>
            <div class="pf-pair-text"><p class="pf-link-how" role="status">{said}</p></div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_says_how_long_it_opens() {
        let until = 10 * 60_000;
        assert_eq!(minutes_left(until, 0), Some(10));
        assert_eq!(minutes_left(until, 30_000), Some(9));
        assert_eq!(minutes_left(until, until - 1), Some(0));
        assert_eq!((minutes_left(until, until), minutes_left(until, until + 1)), (None, None));
        assert!((i18n::syncpair::DE.pair_left)(0).contains("weniger als eine Minute"));
        assert!((i18n::syncpair::EN.pair_left)(0).contains("less than a minute"));
    }

    #[test]
    fn every_problem_says_what_to_do() {
        for s in [&i18n::syncpair::DE, &i18n::syncpair::EN] {
            let said = [s.budget, s.slow_down, s.full, s.bad_link, s.unreachable, s.unsupported].map(str::to_string).into_iter().chain([(s.no_room)(MAX_PROFILES)]);
            for said in said {
                assert!(said.len() > 40 && said.ends_with('.'), "{said}");
            }
            assert!((s.no_room)(MAX_PROFILES).contains(&MAX_PROFILES.to_string()));
        }
        // The page's language picks them (German without one).
        assert!(problem_text(Problem::Unreachable).starts_with("Der Dienst"));
    }
}
