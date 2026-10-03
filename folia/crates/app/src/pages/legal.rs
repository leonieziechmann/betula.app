//! Impressum and Datenschutz: who runs Betula, and what happens to data when somebody uses it.
//!
//! Placeholders from 2026-09-21 („erstmal nur als Placeholder, aber so, dass wir das nicht
//! vergessen") until the real texts of 2026-09-25, from the owner's facts: name, address and
//! e-mail, no telephone; the server is a Contabo VPS in a data centre in Germany, with a contract
//! for processing on behalf (Art. 28 DSGVO); the mailbox is named only as „mein E-Mail-Anbieter".
//! `PLACEHOLDER` stays the switch `deploy/ship.sh` reads: true again, it keeps these pages out of
//! search engines and every instance that is open to everybody from shipping.
//!
//! Both pages are in every language of the site (2026-09-27). The words are in
//! `i18n/legal.rs`, one struct per part; this module has the parts, their order and their markup.
//! The German text is the one that counts: every other language says at the top that it is a
//! translation, with a link to the German page (`BINDING`).
//!
//! The privacy notice says what the software does, so it changes with it. Where each part of it
//! comes from:
//! - „Zugriffsprotokoll": the edge's access log (`deploy/stacks/edge.yml`, „privacy"), 7 days in
//!   Loki (`deploy/config/monitoring/loki.yml`) and 7 days in the host journal, where Traefik logs
//!   through the journald driver (`logging` in `edge.yml`, `deploy/vps/files/journald-betula.conf`);
//!   the rate limit per address (`deploy/config/traefik/dynamic/middlewares.yml`); Folia's own log
//!   without addresses, 30 days (`access_log` in `folia/crates/server/src/main.rs`);
//! - „Speicher im Browser": R20 (docs/folia/frontend.md) and the stores (`bookmarks`, `studyplan`,
//!   `myprogram`, `tabs`, what the finder compares in `pages/catalog`, `assets/enhance.js`,
//!   `assets/boot.js`, `assets/sw.js`; the semantic search's model, `folia/crates/server/src/semantic.rs`);
//! - „Kalender-Abo": `folia_calendar::subscription` (what a code carries), `folia/crates/server/src/api.rs`,
//!   the ways to subscribe in `pages/studyplan/export.rs`;
//! - „Stundenplan teilen": `folia_calendar::share` (what a code carries), the page's tags and
//!   its picture (`pages/studyplan/mod.rs`, `folia/crates/server/src/api.rs`), the link and the offer
//!   (`pages/studyplan/share.rs`), Folia's log without the code (`access_log` in `folia/crates/server/src/main.rs`);
//! - „Cookies": the gate's cookie (`folia/crates/server/src/access.rs`), the only one;
//! - „Namen von Lehrenden": `v_module_lecturer` (docs/radix/schema-v2.md), and how long Radix keeps what
//!   it read (`--event-retention`, `--archive-grace`, docs/radix/operations.md).

use folia_routes::url;
use leptos::prelude::*;
use leptos_meta::Title;

use crate::i18n::{self, legal::Texts, Locale};
use crate::seo::Seo;
use crate::frame::Frame;
use folia_design::ui::Icon;

/// True while the texts below are not final. Keep it in step with them; `deploy/ship.sh` reads
/// this line.
pub const PLACEHOLDER: bool = false;

/// Who runs Betula, as both pages name them.
pub const NAME: &str = "Leonie Juna Ziechmann";
const STREET: &str = "Querstraße 23";
const TOWN: &str = "14656 Brieselang";
pub const EMAIL: &str = "info@betula.app";
const MAILTO: &str = "mailto:info@betula.app";

/// The supervisory authority the rights name, by its own name and address in every language.
const AUTHORITY: [&str; 3] = ["Die Landesbeauftragte für den Datenschutz und für das Recht auf Akteneinsicht Brandenburg", "Stahnsdorfer Damm 77", "14532 Kleinmachnow"];
const AUTHORITY_URL: &str = "https://www.lda.brandenburg.de/";
const AUTHORITY_SITE: &str = "www.lda.brandenburg.de";

/// When the privacy notice last changed: day, month, year.
const PRIVACY_AS_OF: (u32, u32, i32) = (27, 9, 2026);

/// The language whose text counts. A page in any other language is a translation and links to
/// the page in this one (`legal::Texts::translated`).
const BINDING: Locale = Locale::De;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Legal {
    Imprint,
    Privacy,
}

impl Legal {
    fn title(self, t: &Texts) -> &'static str {
        match self {
            Legal::Imprint => t.imprint_title,
            Legal::Privacy => t.privacy_title,
        }
    }

    fn heading(self, t: &Texts) -> &'static str {
        match self {
            Legal::Imprint => t.imprint_heading,
            Legal::Privacy => t.privacy_heading,
        }
    }

    /// The line under the heading, and the page's description for search engines.
    fn lead(self, t: &Texts) -> &'static str {
        match self {
            Legal::Imprint => t.imprint_lead,
            Legal::Privacy => t.privacy_lead,
        }
    }

    fn path(self) -> &'static str {
        match self {
            Legal::Imprint => url::IMPRINT,
            Legal::Privacy => url::PRIVACY,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Legal::Imprint => "info",
            Legal::Privacy => "shield-check",
        }
    }

    fn parts(self) -> &'static [Part] {
        match self {
            Legal::Imprint => &IMPRINT,
            Legal::Privacy => &PRIVACY,
        }
    }

    /// Whether the sidebar lists the parts: the imprint is short enough to see at once.
    fn lists_parts(self) -> bool {
        self == Legal::Privacy
    }
}

/// A section of a page: its anchor (the same in every language), its heading (also its entry
/// under „Auf dieser Seite") and its text, both in the language of the texts handed in.
pub struct Part {
    pub id: &'static str,
    pub heading: fn(&Texts) -> &'static str,
    body: fn(&'static Texts) -> AnyView,
}

static IMPRINT: [Part; 3] = [
    Part { id: "anschrift", heading: |t| t.operator.heading, body: imprint_address },
    Part { id: "kontakt", heading: |t| t.contact.heading, body: imprint_contact },
    Part { id: "hinweis", heading: |t| t.not_btu.heading, body: imprint_note },
];

/// The parts of the privacy notice, in the order of the page.
pub static PRIVACY: [Part; 12] = [
    Part { id: "kurz", heading: |t| t.summary.heading, body: privacy_summary },
    Part { id: "verantwortlich", heading: |t| t.controller.heading, body: privacy_controller },
    Part { id: "server", heading: |t| t.hosting.heading, body: privacy_hosting },
    Part { id: "protokoll", heading: |t| t.access_log.heading, body: privacy_access_log },
    Part { id: "browser", heading: |t| t.browser.heading, body: privacy_browser },
    Part { id: "kalender", heading: |t| t.calendar.heading, body: privacy_calendar },
    Part { id: "teilen", heading: |t| t.share.heading, body: privacy_share },
    Part { id: "cookies", heading: |t| t.cookies.heading, body: privacy_cookies },
    Part { id: "e-mail", heading: |t| t.email.heading, body: privacy_email },
    Part { id: "lehrende", heading: |t| t.lecturers.heading, body: privacy_lecturers },
    Part { id: "weitergabe", heading: |t| t.recipients.heading, body: privacy_recipients },
    Part { id: "rechte", heading: |t| t.rights.heading, body: privacy_rights },
];

#[component]
pub fn ImprintPage() -> impl IntoView {
    view! { <LegalPage page=Legal::Imprint/> }
}

#[component]
pub fn PrivacyPage() -> impl IntoView {
    view! { <LegalPage page=Legal::Privacy/> }
}

#[component]
fn LegalPage(page: Legal) -> impl IntoView {
    let t = i18n::t();
    let texts = &t.legal;
    let sidebar = move || view! {
        <nav class="toc fgroup first legal-toc" aria-label=texts.legal>
            {[Legal::Imprint, Legal::Privacy].into_iter().map(|other| view! {
                <a href=t.path(other.path()) aria-current=(other == page).then_some("page")><Icon name=other.icon()/>{other.title(texts)}</a>
            }).collect_view()}
        </nav>
        {page.lists_parts().then(|| view! {
            // The part the page is at is marked while it scrolls (`data-spy`, enhance.js).
            <nav class="toc fgroup jumps" data-spy="" aria-label=texts.on_this_page>
                <p class="flabel label">{texts.on_this_page}</p>
                {page.parts().iter().enumerate().map(|(i, part)| view! {
                    <a href=format!("#{}", part.id) data-action="jump" aria-current=(i == 0).then_some("location")>{(part.heading)(texts)}</a>
                }).collect_view()}
            </nav>
        })}
        // That Betula is not the BTU's says the ground at the end of every page (`ground::Ground`),
        // and the imprint at length.
    };
    view! {
        <Title text=page.title(texts)/>
        <Frame title=texts.legal sidebar><div class="page-inner legal">
            <Seo title=page.title(texts) description=page.lead(texts) path=page.path() noindex=PLACEHOLDER/>
            {article(page, texts)}
        </div></Frame>
    }
}

/// The text of a page in the language of `t`: heading, lead, the note of a translation, the parts.
fn article(page: Legal, t: &'static Texts) -> impl IntoView {
    // Another language: a link, not a step within the app (`languages`).
    let translated = t.translated.as_ref().map(|note| view! {
        <p class="legal-translation">
            {note.before}
            <a href=BINDING.path(page.path()) hreflang=BINDING.code() lang=BINDING.code() rel="alternate external">{page.heading(&i18n::texts(BINDING).legal)}</a>
            {note.after}
        </p>
    });
    let (day, month, year) = PRIVACY_AS_OF;
    view! {
        <article class="panel legal-panel">
            <header class="legal-head">
                <h1>{page.heading(t)}</h1>
                <p>{page.lead(t)}</p>
                {(page == Legal::Privacy).then(|| view! { <p class="legal-stand">{(t.as_of)(day, month, year)}</p> })}
                {translated}
            </header>
            {page.parts().iter().map(|part| view! {
                <section class="legal-part" id=part.id aria-labelledby=format!("{}-titel", part.id)>
                    <h2 id=format!("{}-titel", part.id)>{(part.heading)(t)}</h2>
                    {(part.body)(t)}
                </section>
            }).collect_view()}
        </article>
    }
}

/// Name and postal address of whoever runs Betula.
fn address() -> impl IntoView {
    view! {
        <address class="legal-address">
            <span>{NAME}</span>
            <span>{STREET}</span>
            <span>{TOWN}</span>
        </address>
    }
}

fn mail_link() -> impl IntoView {
    view! { <a href=MAILTO>{EMAIL}</a> }
}

fn paragraphs(texts: &'static [&'static str]) -> AnyView {
    texts.iter().map(|text| view! { <p>{*text}</p> }).collect_view().into_any()
}

fn points(texts: &'static [&'static str]) -> impl IntoView {
    view! { <ul>{texts.iter().map(|text| view! { <li>{*text}</li> }).collect_view()}</ul> }
}

/// One of the facts of a part (`<dl class="legal-facts">`): what it is about, and what it says.
fn fact(label: &'static str, text: &'static str) -> impl IntoView {
    view! {
        <div>
            <dt>{label}</dt>
            <dd>{text}</dd>
        </div>
    }
}

// ---------------------------------------------------------------- Impressum

fn imprint_address(t: &'static Texts) -> AnyView {
    view! {
        <p>{t.operator.run_by}</p>
        {address()}
    }
    .into_any()
}

fn imprint_contact(t: &'static Texts) -> AnyView {
    view! {
        <p>{t.email_label}{mail_link()}</p>
        <p>{t.contact.hint}</p>
    }
    .into_any()
}

fn imprint_note(t: &'static Texts) -> AnyView {
    paragraphs(t.not_btu.texts)
}

// ---------------------------------------------------------------- Datenschutz

fn privacy_summary(t: &'static Texts) -> AnyView {
    points(t.summary.texts).into_any()
}

fn privacy_controller(t: &'static Texts) -> AnyView {
    view! {
        <p>{t.controller.text}</p>
        {address()}
        <p>{t.email_label}{mail_link()}</p>
    }
    .into_any()
}

fn privacy_hosting(t: &'static Texts) -> AnyView {
    paragraphs(t.hosting.texts)
}

fn privacy_access_log(t: &'static Texts) -> AnyView {
    let (facts, log) = (&t.facts, &t.access_log);
    view! {
        <p>{log.intro}</p>
        <dl class="legal-facts">
            <div>
                <dt>{facts.data}</dt>
                <dd>
                    {points(log.data)}
                    <p>{log.not_recorded}</p>
                </dd>
            </div>
            {fact(facts.purpose, log.purpose)}
            {fact(facts.legal_basis, log.legal_basis)}
            {fact(facts.retention, log.retention)}
        </dl>
        <p>{log.app_log}</p>
    }
    .into_any()
}

fn privacy_browser(t: &'static Texts) -> AnyView {
    let browser = &t.browser;
    view! {
        <p>{browser.intro}</p>
        {points(browser.kept)}
        {paragraphs(browser.texts)}
    }
    .into_any()
}

fn privacy_calendar(t: &'static Texts) -> AnyView {
    let (facts, calendar) = (&t.facts, &t.calendar);
    view! {
        <p>{calendar.intro}</p>
        <dl class="legal-facts">
            {fact(facts.course, calendar.course)}
            {fact(facts.logs, calendar.logs)}
            {fact(facts.legal_basis, calendar.legal_basis)}
            {fact(facts.disclosure, calendar.disclosure)}
            {fact(facts.ending, calendar.ending)}
        </dl>
        <p>{calendar.download}</p>
    }
    .into_any()
}

fn privacy_share(t: &'static Texts) -> AnyView {
    let (facts, share) = (&t.facts, &t.share);
    view! {
        <p>{share.intro}</p>
        <dl class="legal-facts">
            {fact(facts.course, share.course)}
            {fact(facts.logs, share.logs)}
            {fact(facts.legal_basis, share.legal_basis)}
            {fact(facts.disclosure, share.disclosure)}
            {fact(facts.ending, share.ending)}
        </dl>
    }
    .into_any()
}

fn privacy_cookies(t: &'static Texts) -> AnyView {
    let cookies = &t.cookies;
    view! {
        <p>{cookies.before}<code>"betula_access"</code>{cookies.after}</p>
        <p>{cookies.details}</p>
    }
    .into_any()
}

fn privacy_email(t: &'static Texts) -> AnyView {
    paragraphs(t.email.texts)
}

fn privacy_lecturers(t: &'static Texts) -> AnyView {
    let (facts, lecturers) = (&t.facts, &t.lecturers);
    view! {
        <p>{lecturers.intro}</p>
        <dl class="legal-facts">
            {fact(facts.data, lecturers.data)}
            {fact(facts.source, lecturers.source)}
            {fact(facts.purpose, lecturers.purpose)}
            {fact(facts.legal_basis, lecturers.legal_basis)}
            {fact(facts.recipients, lecturers.recipients)}
            {fact(facts.retention, lecturers.retention)}
            <div>
                <dt>{facts.objection}</dt>
                <dd>{lecturers.objection_before}{mail_link()}{lecturers.objection_after}</dd>
            </div>
        </dl>
    }
    .into_any()
}

fn privacy_recipients(t: &'static Texts) -> AnyView {
    paragraphs(t.recipients.texts)
}

fn privacy_rights(t: &'static Texts) -> AnyView {
    let rights = &t.rights;
    view! {
        <p>{rights.before}{mail_link()}{rights.after}</p>
        <p>{rights.access_log}</p>
        <p>{rights.complaint}</p>
        <address class="legal-address">
            {AUTHORITY.iter().map(|line| view! { <span>{*line}</span> }).collect_view()}
            <span><a href=AUTHORITY_URL rel="noopener">{AUTHORITY_SITE}</a></span>
        </address>
    }
    .into_any()
}

// Rendering to HTML needs the server's build (`ssr`), as in `cargo test -p folia-app -p folia-server`.
#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    /// Every language names who runs Betula and how to reach them, with every part; a page in
    /// a language whose text does not count links the German page, which does.
    #[test]
    fn every_language_has_every_part_and_a_translation_links_the_german_text() {
        for page in [Legal::Imprint, Legal::Privacy] {
            for locale in Locale::ALL.iter().copied() {
                let texts = &i18n::texts(locale).legal;
                let html = article(page, texts).to_html().replace("<!>", "");
                for text in [NAME, STREET, TOWN, &format!("href=\"{MAILTO}\"")] {
                    assert!(html.contains(text), "{locale:?} {}: {text}", page.path());
                }
                for part in page.parts() {
                    assert!(html.contains(&format!("id=\"{}\"", part.id)) && html.contains((part.heading)(texts)), "{locale:?}: {}", part.id);
                }
                let original = format!("href=\"{}\" hreflang=\"de\" lang=\"de\" rel=\"alternate external\"", page.path());
                assert_eq!(html.contains(&original), locale != BINDING, "{locale:?} {}: {html}", page.path());
            }
        }
        let privacy = |locale| article(Legal::Privacy, &i18n::texts(locale).legal).to_html().replace("<!>", "");
        let (de, en) = (privacy(Locale::De), privacy(Locale::En));
        assert!(de.contains("<h1>Datenschutzerklärung</h1>") && de.contains("Stand: 27. September 2026"), "{de}");
        assert!(en.contains("<h1>Privacy policy</h1>") && en.contains("As of 27 September 2026") && en.contains(">Datenschutzerklärung</a>"), "{en}");
        assert!(en.contains("Art. 6(1)(f) GDPR (DSGVO)") && !en.contains("Rechtsgrundlage"), "{en}");
    }
}
