//! „Mit dem Handy verbinden" (`/pair`, Betula Sync; owner, 2026-11-08: „Da muss ein link kommen wie
//! betula.app/pair wo dann der qr code gezeigt wird, den man am handy scannen kann um sich
//! einzuloggen"). The other way round from the link of „Gerät verbinden", for a computer, which has
//! no camera to scan with: the device that wants the profile shows the code, and the one that holds
//! it scans (folia/design/profil/konzepte.html, the second step).
//!
//! - **The computer** (`/pair`): the code and its four digits (`sync::GetPart`, as the profile
//!   page's dialog „Profil vom Handy holen" shows them): `/pair#<id ‖ public key>`
//!   (`folia_stores::sync::request`), ten minutes; once the phone has sent a profile, it is the
//!   active one here, and the page says so.
//! - **The phone** (`/pair#<…>`, opened by its camera app, or read by Betula's scanner on `/sync`):
//!   which profile goes, the digits, „Ja, „Lena“ verbinden" (`give::GiveRequest`); a profile that
//!   holds nothing in this browser says so first (a camera app may open the code in a browser that
//!   is not the one Betula is used in).
//!
//! What follows `#` never reaches a server; the page takes it out of the address at once. The
//! server writes the same explanation for everybody (R9), `noindex`.

use leptos::prelude::*;

use folia_design::stage::Title;
use folia_design::ui::Icon;
use folia_routes::url;
use folia_shell::frame::Plain;
use folia_shell::pending::Shape;
use folia_shell::seo::Seo;
use folia_shell::skeleton::AppStandin;
use folia_stores::sync::request;
use folia_stores::sync::Sync;

use crate::connect::take_fragment;
use crate::give::GiveRequest;
use crate::i18n;
use crate::sync::GetPart;

/// Whether this build is the browser app.
const APP: bool = cfg!(feature = "csr");

#[component]
fn PairSeo() -> impl IntoView {
    let t = i18n::t();
    view! { <Seo title=t.sync.ask_title description=t.sync.ask_description path=url::PAIR noindex=true/> }
}

#[component]
pub fn PairPage() -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    if !APP {
        return view! {
            <Title text=s.ask_title/>
            <PairSeo/>
            <AppStandin shape=Shape::Text title=s.ask_title hint=s.ask_server_hint/>
        }
        .into_any();
    }
    // Read once, as the page is made: the fragment goes out of the address at once.
    let fragment = take_fragment();
    let sync = Sync::expect().filter(|_| folia_stores::sync::offered().is_some());
    // The phone left the question unanswered.
    let left = RwSignal::new(false);
    let ways = move || {
        view! {
            <nav class="pf-ways">
                <a class="btn primary" href=t.path(url::HOME) rel="nofollow"><Icon name="house"/><span>{s.to_start}</span></a>
                <a class="btn secondary" href=t.path(url::PROFILE) rel="nofollow"><Icon name="user-round"/><span>{s.to_profile}</span></a>
            </nav>
        }
    };
    let plain = move |said: &'static str| {
        view! {
            <section class="panel pf-part" aria-labelledby="pf-pair">
                <h2 id="pf-pair"><Icon name="monitor-smartphone"/>{s.ask_title}</h2>
                <p class="pf-empty">{said}</p>
                {ways()}
            </section>
        }
        .into_any()
    };
    let body = match (sync, fragment) {
        (None, _) => plain(s.ask_off),
        (Some(_), Some(fragment)) => {
            if request::parse(&fragment).is_none() {
                plain(t.scan.give_bad)
            } else {
                (move || if left.get() { plain(s.ask_left_unanswered) } else { view! { <GiveRequest fragment=fragment.clone() back=Callback::new(move |()| left.set(true))/> }.into_any() }).into_any()
            }
        }
        (Some(sync), None) => {
            let got = RwSignal::new(false);
            view! {
                <section class="panel pf-part pf-asking" aria-labelledby="pf-pair">
                    <h2 id="pf-pair"><Icon name="monitor-smartphone"/>{s.ask_title}</h2>
                    <div class="pf-connect-body">
                        <GetPart sync got/>
                    </div>
                    {move || got.get().then(ways)}
                </section>
            }
            .into_any()
        }
    };
    view! {
        <Title text=s.ask_title/>
        <Plain>
            <div class="page-inner pf pf-connect pf-pair-page">
                <PairSeo/>
                {body}
            </div>
        </Plain>
    }
    .into_any()
}
