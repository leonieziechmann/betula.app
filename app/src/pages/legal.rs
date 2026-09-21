//! Impressum and Datenschutz. For now both are placeholders (owner, 2026-09-21: "erstmal nur als
//! Placeholder, aber so, dass wir das nicht vergessen"): each page says so in plain sight and lists
//! what has to stand there before Betula is open to everybody.
//!
//! Not forgetting them is enforced, not hoped for: while `PLACEHOLDER` is true, `deploy/ship.sh`
//! refuses to ship an instance that is open to everybody (`FOLIA_ACCESS_GATE=off`), and the pages
//! are not indexed. The notes on what the privacy notice has to name come from what the software
//! really does (the edge's access log in `deploy/stacks/edge.yml`, the gate's cookie in
//! `server/src/access.rs`, R20 for what stays in the browser); check them again when writing the
//! real text.

use catalog::url;
use leptos::prelude::*;
use leptos_meta::Title;

use crate::seo::Seo;
use crate::ui::{Frame, Icon};

/// True while the texts below are placeholders. Set it to false together with the real texts;
/// `deploy/ship.sh` reads this line.
pub const PLACEHOLDER: bool = true;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Legal {
    Imprint,
    Privacy,
}

impl Legal {
    fn title(self) -> &'static str {
        match self {
            Legal::Imprint => "Impressum",
            Legal::Privacy => "Datenschutz",
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
}

/// What the imprint has to name (§ 5 DDG), as a list of what is still missing.
const IMPRINT_OWED: [(&str, &str); 4] = [
    ("Wer Betula anbietet", "Vor- und Nachname der verantwortlichen Person."),
    ("Eine ladungsfähige Anschrift", "Eine Postanschrift, unter der Post zugestellt werden kann; ein Postfach genügt nicht."),
    ("Wie man schnell Kontakt aufnimmt", "Eine E-Mail-Adresse und ein zweiter schneller Weg, etwa ein Kontaktformular."),
    ("Dass Betula nicht zur BTU gehört", "Der Hinweis, dass Betula ein inoffizielles Projekt ist und verbindlich allein die Angaben der BTU Cottbus-Senftenberg sind."),
];

/// What the privacy notice has to cover: what the software stores or sends today.
const PRIVACY_OWED: [(&str, &str); 7] = [
    ("Wer verantwortlich ist", "Name und Anschrift wie im Impressum."),
    ("Wo Betula läuft", "Der Anbieter des Servers und wo er steht; mit ihm ein Vertrag zur Auftragsverarbeitung."),
    ("Das Zugriffsprotokoll", "Der Server protokolliert jeden Aufruf mit IP-Adresse, Adresse der Seite, Browserkennung und Zeit: in den Protokolldateien des Servers und sieben Tage lang in der Überwachung. Zweck (Betrieb, Abwehr von Missbrauch), Rechtsgrundlage und Speicherdauer gehören hierher."),
    ("Das Cookie der Testphase", "Solange Betula ein Passwort verlangt, merkt sich ein Cookie, dass es eingegeben wurde, und wann dieser Besuch endet."),
    ("Was nur im Browser bleibt", "Merkliste, Einstellungen (hell oder dunkel, Breiten) und die Kopie des Katalogs liegen im Speicher des Browsers und erreichen den Server nie."),
    ("Namen der Lehrenden", "Die Modulbeschreibungen der BTU nennen Verantwortliche und Lehrende. Betula zeigt diese Namen; dafür braucht es eine Rechtsgrundlage und die Information nach Art. 14 DSGVO, mit einem Weg zum Widerspruch."),
    ("Rechte und Beschwerde", "Auskunft, Berichtigung, Löschung, Einschränkung, Widerspruch und das Recht auf Beschwerde bei einer Datenschutz-Aufsichtsbehörde."),
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
    let sidebar = move || view! {
        <nav class="toc legal-toc" aria-label="Rechtliches">
            {[Legal::Imprint, Legal::Privacy].into_iter().map(|other| view! {
                <a href=other.path() aria-current=(other == page).then_some("page")><Icon name=other.icon()/>{other.title()}</a>
            }).collect_view()}
        </nav>
        <div class="fgroup">
            <p class="hint">"Betula ist ein inoffizielles Projekt und gehört nicht zur BTU Cottbus-Senftenberg."</p>
        </div>
    };
    let (owed, lead, description): (&[(&str, &str)], &str, &str) = match page {
        Legal::Imprint => (&IMPRINT_OWED, "Das Impressum sagt, wer hinter Betula steht und wie man diese Person erreicht.", "Wer Betula anbietet und wie man Kontakt aufnimmt."),
        Legal::Privacy => (&PRIVACY_OWED, "Die Datenschutzerklärung sagt, welche Daten beim Besuch von Betula anfallen, wozu und wie lange.", "Welche Daten beim Besuch von Betula anfallen, wozu und wie lange."),
    };
    view! {
        <Title text=page.title()/>
        <Frame title="Rechtliches" sidebar><div class="page-inner legal">
            <Seo title=page.title() description=description path=page.path() noindex=PLACEHOLDER/>
            <section class="panel legal-panel">
                <header class="legal-head">
                    <h1>{page.title()}</h1>
                    <p>{lead}</p>
                </header>
                {PLACEHOLDER.then(|| view! {
                    <p class="note legal-placeholder"><Icon name="info"/><span>
                        <b>"Platzhalter. "</b>"Diese Seite ist noch nicht fertig. Bevor Betula für alle offen ist, steht hier der vollständige Text; bis dahin ist die Seite eine Liste dessen, was noch fehlt."
                    </span></p>
                    <ol class="legal-owed">
                        {owed.iter().map(|(what, detail)| view! {
                            <li><b>{*what}</b><span>{*detail}</span></li>
                        }).collect_view()}
                    </ol>
                })}
            </section>
        </div></Frame>
    }
}
