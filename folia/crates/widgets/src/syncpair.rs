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

/// Copies a text into the clipboard (a click's: the browser lets it).
pub fn copy(text: &str) {
    #[cfg(feature = "csr")]
    if let Some(window) = web_sys::window() {
        let _ = window.navigator().clipboard().write_text(text);
    }
    #[cfg(not(feature = "csr"))]
    let _ = text;
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

/// The link for the next device: its QR code, the link to copy, „Teilen" where the system can, how
/// long it still opens; gone, „Neuer Link" (`again`). `close`, where there is one, puts it away
/// („Schließen").
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
            return view! {
                <p class="action note-action pf-refused" role="status">
                    <Icon name="clock-3"/><span>{s.pair_gone}</span>
                    <button class="mini primary hit" type="button" on:click=move |_| again.run(())>{s.pair_new}</button>
                    {close_button()}
                </p>
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
            <div class="pf-pair">
                {qr}
                <div class="pf-pair-text">
                    <p>{s.pair_how}</p>
                    <p class="pf-pair-link"><code>{shown.clone()}</code></p>
                    <p class="pf-pair-left" role="status">{move || left.get().map(s.pair_left)}</p>
                    <div class="pf-pair-actions">
                        <button class="mini primary hit" type="button" on:click=copy_it>
                            <Icon name=if copied.get() { "check" } else { "copy" }/>
                            <span>{move || if copied.get() { s.copied } else { s.copy_link }}</span>
                        </button>
                        {sharing.then(|| {
                            let shared = shared.clone();
                            view! {
                                <button class="mini hit" type="button" on:click=move |_| share(&shared, s.share_title)>
                                    <Icon name="share-2"/><span>{s.share}</span>
                                </button>
                            }
                        })}
                        {close_button()}
                    </div>
                </div>
            </div>
        }
        .into_any()
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
