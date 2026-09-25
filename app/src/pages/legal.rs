//! Impressum and Datenschutz: who runs Betula, and what happens to data when somebody uses it.
//!
//! Placeholders from 2026-09-21 („erstmal nur als Placeholder, aber so, dass wir das nicht
//! vergessen") until the real texts of 2026-09-25, from the owner's facts: name, address and
//! e-mail, no telephone; the server is a Contabo VPS in a data centre in Germany, with a contract
//! for processing on behalf (Art. 28 DSGVO); the mailbox is named only as „mein E-Mail-Anbieter".
//! `PLACEHOLDER` stays the switch `deploy/ship.sh` reads: true again, it keeps these pages out of
//! search engines and every instance that is open to everybody from shipping.
//!
//! The privacy notice says what the software does, so it changes with it. Where each part of it
//! comes from:
//! - „Zugriffsprotokoll": the edge's access log (`deploy/stacks/edge.yml`, „privacy"), 7 days in
//!   Loki (`deploy/config/monitoring/loki.yml`) and 7 days in the host journal, where Traefik logs
//!   through the journald driver (`logging` in `edge.yml`, `deploy/vps/files/journald-betula.conf`);
//!   the rate limit per address (`deploy/config/traefik/dynamic/middlewares.yml`); Folia's own log
//!   without addresses, 30 days (`access_log` in `server/src/main.rs`);
//! - „Speicher im Browser": R20 (docs/frontend.md) and the stores (`bookmarks`, `studyplan`,
//!   `myprogram`, `tabs`, `assets/enhance.js`, `assets/boot.js`, `assets/sw.js`);
//! - „Kalender-Abo": `catalog::timetable::subscription` (what a code carries), `server/src/api.rs`,
//!   the ways to subscribe in `pages/studyplan/export.rs`;
//! - „Cookies": the gate's cookie (`server/src/access.rs`), the only one;
//! - „Namen von Lehrenden": `v_module_lecturer` (docs/schema-v2.md), and how long Radix keeps what
//!   it read (`--event-retention`, `--archive-grace`, docs/operations.md).

use catalog::url;
use leptos::prelude::*;
use leptos_meta::Title;

use crate::seo::Seo;
use crate::ui::{Frame, Icon};

/// True while the texts below are not final. Keep it in step with them; `deploy/ship.sh` reads
/// this line.
pub const PLACEHOLDER: bool = false;

/// Who runs Betula, as both pages name them.
pub const NAME: &str = "Leonie Juna Ziechmann";
const STREET: &str = "Querstraße 23";
const TOWN: &str = "14656 Brieselang";
pub const EMAIL: &str = "info@betula.app";
const MAILTO: &str = "mailto:info@betula.app";

/// When the privacy notice last changed.
const PRIVACY_AS_OF: &str = "25. September 2026";

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

    fn heading(self) -> &'static str {
        match self {
            Legal::Imprint => "Impressum",
            Legal::Privacy => "Datenschutzerklärung",
        }
    }

    /// The line under the heading, and the page's description for search engines.
    fn lead(self) -> &'static str {
        match self {
            Legal::Imprint => "Wer Betula betreibt und wie du Kontakt aufnimmst.",
            Legal::Privacy => "Welche Daten beim Besuch von Betula anfallen, wozu und wie lange.",
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

/// A section of a page: its anchor, its heading (also its entry under „Auf dieser Seite") and its text.
pub struct Part {
    pub id: &'static str,
    pub heading: &'static str,
    body: fn() -> AnyView,
}

static IMPRINT: [Part; 3] = [
    Part { id: "anschrift", heading: "Angaben nach § 5 DDG", body: imprint_address },
    Part { id: "kontakt", heading: "Kontakt", body: imprint_contact },
    Part { id: "hinweis", heading: "Kein Angebot der BTU", body: imprint_note },
];

/// The parts of the privacy notice, in the order of the page.
pub static PRIVACY: [Part; 11] = [
    Part { id: "kurz", heading: "Kurz gesagt", body: privacy_summary },
    Part { id: "verantwortlich", heading: "Verantwortlich", body: privacy_controller },
    Part { id: "server", heading: "Wo Betula läuft", body: privacy_hosting },
    Part { id: "protokoll", heading: "Zugriffsprotokoll", body: privacy_access_log },
    Part { id: "browser", heading: "Speicher im Browser", body: privacy_browser },
    Part { id: "kalender", heading: "Kalender-Abo", body: privacy_calendar },
    Part { id: "cookies", heading: "Cookies", body: privacy_cookies },
    Part { id: "e-mail", heading: "E-Mail", body: privacy_email },
    Part { id: "lehrende", heading: "Namen von Lehrenden", body: privacy_lecturers },
    Part { id: "weitergabe", heading: "Weitergabe und Links", body: privacy_recipients },
    Part { id: "rechte", heading: "Deine Rechte", body: privacy_rights },
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
        <nav class="toc fgroup first legal-toc" aria-label="Rechtliches">
            {[Legal::Imprint, Legal::Privacy].into_iter().map(|other| view! {
                <a href=other.path() aria-current=(other == page).then_some("page")><Icon name=other.icon()/>{other.title()}</a>
            }).collect_view()}
        </nav>
        {page.lists_parts().then(|| view! {
            // The part the page is at is marked while it scrolls (`data-spy`, enhance.js).
            <nav class="toc fgroup jumps" data-spy="" aria-label="Auf dieser Seite">
                <p class="flabel label">"Auf dieser Seite"</p>
                {page.parts().iter().enumerate().map(|(i, part)| view! {
                    <a href=format!("#{}", part.id) data-action="jump" aria-current=(i == 0).then_some("location")>{part.heading}</a>
                }).collect_view()}
            </nav>
        })}
        // That Betula is not the BTU's says the ground at the end of every page (`ground::Ground`),
        // and the imprint at length.
    };
    view! {
        <Title text=page.title()/>
        <Frame title="Rechtliches" sidebar><div class="page-inner legal">
            <Seo title=page.title() description=page.lead() path=page.path() noindex=PLACEHOLDER/>
            <article class="panel legal-panel">
                <header class="legal-head">
                    <h1>{page.heading()}</h1>
                    <p>{page.lead()}</p>
                    {(page == Legal::Privacy).then(|| view! { <p class="legal-stand">"Stand: "{PRIVACY_AS_OF}</p> })}
                </header>
                {page.parts().iter().map(|part| view! {
                    <section class="legal-part" id=part.id aria-labelledby=format!("{}-titel", part.id)>
                        <h2 id=format!("{}-titel", part.id)>{part.heading}</h2>
                        {(part.body)()}
                    </section>
                }).collect_view()}
            </article>
        </div></Frame>
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

// ---------------------------------------------------------------- Impressum

fn imprint_address() -> AnyView {
    view! {
        <p>"Betula wird betrieben von:"</p>
        {address()}
    }
    .into_any()
}

fn imprint_contact() -> AnyView {
    view! {
        <p>"E-Mail: "{mail_link()}</p>
        <p>"Fragen, Hinweise auf Fehler und Anregungen zu Betula gern per E-Mail."</p>
    }
    .into_any()
}

fn imprint_note() -> AnyView {
    view! {
        <p>"Betula ist ein privates, nicht-kommerzielles Projekt. Es gehört nicht zur Brandenburgischen Technischen Universität Cottbus-Senftenberg (BTU), und die BTU hat es weder beauftragt noch geprüft."</p>
        <p>"Die Angaben zu Modulen, Studiengängen, Ordnungen und Terminen stammen von den öffentlichen Seiten der BTU. Betula ordnet sie und macht sie durchsuchbar, ändert aber nichts an ihrem Inhalt. Trotz aller Sorgfalt kann beim Einlesen etwas fehlen, falsch gelesen werden oder veralten. Verbindlich sind allein die Angaben der BTU; jede Seite von Betula verlinkt deshalb auf ihr Original."</p>
        <p>"Für die Inhalte verlinkter Seiten ist allein verantwortlich, wer sie anbietet."</p>
    }
    .into_any()
}

// ---------------------------------------------------------------- Datenschutz

fn privacy_summary() -> AnyView {
    view! {
        <ul>
            <li>"Kein Konto, keine Werbung, kein Tracking: Betula nutzt keine Analysedienste und setzt keine Cookies, die dich wiedererkennen. Schrift, Symbole und Skripte liefert betula.app selbst aus; von fremden Servern wird nichts geladen."</li>
            <li>"Merkliste, Stundenplan und „Mein Studiengang“ liegen nur in deinem Browser."</li>
            <li>"Der Server protokolliert jeden Aufruf mit IP-Adresse, um Betula zu betreiben und vor Missbrauch zu schützen, und löscht die Einträge nach 7 Tagen."</li>
            <li>"Betula zeigt die Namen von Lehrenden, die die BTU in ihren Modulbeschreibungen veröffentlicht."</li>
        </ul>
    }
    .into_any()
}

fn privacy_controller() -> AnyView {
    view! {
        <p>"Verantwortlich für die Verarbeitung personenbezogener Daten auf betula.app im Sinne der Datenschutz-Grundverordnung (DSGVO) ist:"</p>
        {address()}
        <p>"E-Mail: "{mail_link()}</p>
    }
    .into_any()
}

fn privacy_hosting() -> AnyView {
    view! {
        <p>"Betula läuft auf einem gemieteten Server der Contabo GmbH, Welfenstraße 22, 81541 München, in einem Rechenzentrum in Deutschland. Dort wird alles verarbeitet, was beim Besuch anfällt, auch die Protokolle und ihre Auswertung; andere Dienste sind daran nicht beteiligt."</p>
        <p>"Contabo stellt den Server bereit und verarbeitet die Daten darauf nur in meinem Auftrag; dafür besteht ein Vertrag zur Auftragsverarbeitung (Art. 28 DSGVO). Die Verbindung zwischen deinem Browser und Betula ist verschlüsselt (HTTPS)."</p>
    }
    .into_any()
}

fn privacy_access_log() -> AnyView {
    view! {
        <p>"Bei jedem Aufruf schickt dein Browser Angaben mit, ohne die der Server keine Seite ausliefern kann. Der Server hält jeden Aufruf in einem Zugriffsprotokoll fest."</p>
        <dl class="legal-facts">
            <div>
                <dt>"Welche Daten"</dt>
                <dd>
                    <ul>
                        <li>"deine IP-Adresse und der Port deines Geräts,"</li>
                        <li>"Datum und Uhrzeit,"</li>
                        <li>"die aufgerufene Adresse, auch mit dem Teil nach dem „?“, in dem etwa Suchbegriffe und Filter stehen,"</li>
                        <li>"die Art der Anfrage und der Verbindung, Statuscode, Größe und Dauer der Antwort,"</li>
                        <li>"die Kennung deines Browsers (User-Agent)."</li>
                    </ul>
                    <p>"Nicht erfasst werden die Seite, von der du kommst, Cookies und alle übrigen Angaben deines Browsers."</p>
                </dd>
            </div>
            <div>
                <dt>"Zweck"</dt>
                <dd>"Betula stabil und sicher betreiben, Fehler finden, Angriffe und Missbrauch erkennen und abwehren. Dazu zählt der Server auch, wie viele Anfragen in kurzer Zeit von einer IP-Adresse kommen, und bremst, wenn es zu viele werden; diese Zählung liegt nur im Arbeitsspeicher."</dd>
            </div>
            <div>
                <dt>"Rechtsgrundlage"</dt>
                <dd>"Art. 6 Abs. 1 lit. f DSGVO. Mein berechtigtes Interesse ist ein sicherer und funktionierender Betrieb."</dd>
            </div>
            <div>
                <dt>"Speicherdauer"</dt>
                <dd>"Die Einträge stehen im Systemprotokoll des Servers und in einer Protokollauswertung auf demselben Server und werden an beiden Stellen nach 7 Tagen automatisch gelöscht."</dd>
            </div>
        </dl>
        <p>"Daneben führt die Anwendung ein eigenes Protokoll: welche Seite wann aufgerufen wurde, ohne den Teil nach dem „?“, mit Statuscode und Dauer, aber ohne IP-Adresse und ohne Browserkennung. In der Protokollauswertung wird es nach 30 Tagen gelöscht."</p>
    }
    .into_any()
}

fn privacy_browser() -> AnyView {
    view! {
        <p>"Vieles, was du in Betula tust, passiert direkt in deinem Browser. Damit das geht, speichert Betula auf deinem Gerät, im Speicher des Browsers (Local Storage, Session Storage, IndexedDB und Cache Storage):"</p>
        <ul>
            <li>"deine Merkliste,"</li>
            <li>"deinen Stundenplan mit geplanten Modulen, Platzhaltern, Ausgeblendetem und ausgewählten Terminen, deine gespeicherten Pläne und die Adressen deiner Kalender-Abos,"</li>
            <li>"„Mein Studiengang“ mit Studienrichtung, Studienbeginn und Standort,"</li>
            <li>"Einstellungen, etwa hell oder dunkel und die Breite der Seitenleisten,"</li>
            <li>"für die laufende Sitzung, wo du in jedem Bereich zuletzt warst und wie weit du gescrollt hast,"</li>
            <li>"eine Kopie des Katalogs, die Dateien der App und bis zu 60 zuletzt besuchte Seiten, damit Betula schnell startet und auch ohne Netz funktioniert."</li>
        </ul>
        <p>"Das alles liegt nur auf deinem Gerät; ich kann es nicht einsehen, und ein Konto gibt es nicht. Ein anderes Gerät hat seine eigenen Daten. Der Link, mit dem du deine Merkliste auf ein anderes Gerät bringst, trägt sie hinter dem „#“, und diesen Teil einer Adresse sendet kein Browser an einen Server."</p>
        <p>"Den Server erreicht nur, was in der Adresse einer Seite steht, wenn du sie aufrufst, neu lädst oder teilst. Eine Adresse sagt, was gerade angezeigt wird, aber nie, was auf deiner Merkliste oder in deinem Stundenplan steht: etwa den Studiengang, nach dem der Katalog gefiltert ist (auch „Mein Studiengang“, wenn du den Katalog darüber öffnest), das Modul, das neben Merkliste oder Stundenplan offen ist, oder Semester und Platzhalter, für die du Module suchst. Solche Adressen stehen wie alle anderen im Zugriffsprotokoll. Deinen Stundenplan bekommt der Server nur, wenn du ihn als Kalender abonnierst."</p>
        <p>"Die Speicherung ist für die Funktionen nötig, die du nutzt, und braucht deshalb keine Einwilligung (§ 25 Abs. 2 Nr. 2 TDDDG). Löschen kannst du alles jederzeit in den Einstellungen deines Browsers, indem du die Websitedaten von betula.app löschst."</p>
    }
    .into_any()
}

fn privacy_calendar() -> AnyView {
    view! {
        <p>"Du kannst deinen Stundenplan als Kalender abonnieren. Dein Kalenderdienst bekommt dafür eine Adresse, die mit betula.app/calendar/ beginnt. Sie enthält als Code das Semester, die geplanten Module, den Studiengang, dessen Modulkürzel die Einträge verwenden, den Standort und was du ausgeblendet oder ausgewählt hast."</p>
        <dl class="legal-facts">
            <div>
                <dt>"Ablauf"</dt>
                <dd>"Dein Kalender, etwa von Apple, Google oder Microsoft, ruft die Adresse regelmäßig ab, je nach Dienst von deinem Gerät oder von den Servern des Anbieters. Betula erstellt den Kalender bei jedem Abruf neu aus dem Code und den aktuellen Terminen und speichert dazu nichts."</dd>
            </div>
            <div>
                <dt>"Protokolle"</dt>
                <dd>"Im Zugriffsprotokoll steht jeder Abruf mit der vollständigen Adresse und der IP-Adresse, von der er kommt; wie alles dort wird er nach 7 Tagen gelöscht. Das Protokoll der Anwendung enthält den Code nicht."</dd>
            </div>
            <div>
                <dt>"Rechtsgrundlage"</dt>
                <dd>"Art. 6 Abs. 1 lit. f DSGVO. Mein berechtigtes Interesse ist, dir das Abo anzubieten, das du selbst anlegst."</dd>
            </div>
            <div>
                <dt>"Weitergabe"</dt>
                <dd>"Wählst du „Google Kalender“ oder „Outlook“, öffnet dein Browser die Seite dieses Dienstes und übergibt ihm die Adresse; ab da gilt dessen Datenschutzerklärung. Betula selbst gibt nichts weiter. Wer die Adresse kennt, kann den Kalender ebenfalls abrufen: Gib sie nur an Menschen weiter, denen du deinen Plan zeigen möchtest."</dd>
            </div>
            <div>
                <dt>"Beenden"</dt>
                <dd>"Entferne den Kalender in deinem Kalenderdienst; dann wird die Adresse nicht mehr abgerufen."</dd>
            </div>
        </dl>
        <p>"Eine heruntergeladene .ics-Datei entsteht in deinem Browser und erreicht den Server nicht."</p>
    }
    .into_any()
}

fn privacy_cookies() -> AnyView {
    view! {
        <p>"Betula setzt keine Cookies, mit einer Ausnahme: Solange Betula nur mit Passwort erreichbar ist, etwa während eines geschlossenen Tests, setzt der Server nach der Eingabe des richtigen Passworts ein Cookie namens "<code>"betula_access"</code>"."</p>
        <p>"Es enthält nur, wann dein Zugang endet, und eine Signatur, an der der Server erkennt, dass er es selbst ausgestellt hat; gespeichert wird dazu nichts. Es gilt 90 Tage, wird nur über verschlüsselte Verbindungen an Betula gesendet und ist für Skripte nicht lesbar. Weil es für den Zugang, den du angefragt hast, unbedingt nötig ist, braucht es keine Einwilligung (§ 25 Abs. 2 Nr. 2 TDDDG)."</p>
    }
    .into_any()
}

fn privacy_email() -> AnyView {
    view! {
        <p>"Schreibst du mir eine E-Mail, verarbeite ich deine E-Mail-Adresse, deinen Namen, wenn du ihn nennst, und deine Nachricht, um dir zu antworten. Rechtsgrundlage ist Art. 6 Abs. 1 lit. f DSGVO; mein berechtigtes Interesse ist, Anfragen zu beantworten. Die Nachrichten liegen bei meinem E-Mail-Anbieter. Ich lösche sie, sobald dein Anliegen erledigt ist und sie nicht mehr gebraucht werden."</p>
    }
    .into_any()
}

fn privacy_lecturers() -> AnyView {
    view! {
        <p>"Die Modulbeschreibungen und das Vorlesungsverzeichnis der BTU nennen, wer ein Modul verantwortet und wer eine Veranstaltung hält. Betula übernimmt diese Angaben und zeigt sie bei den Modulen und Terminen; im Katalog lässt sich nach ihnen filtern. Die folgenden Angaben richten sich an die genannten Personen (Art. 14 DSGVO)."</p>
        <dl class="legal-facts">
            <div>
                <dt>"Welche Daten"</dt>
                <dd>"Name, akademischer Titel und Rolle (etwa verantwortlich oder lehrend), jeweils mit dem Modul oder der Veranstaltung."</dd>
            </div>
            <div>
                <dt>"Quelle"</dt>
                <dd>"die öffentlich zugänglichen Seiten der BTU Cottbus-Senftenberg, vor allem die Modulbeschreibungen und das Vorlesungsverzeichnis im Portal QIS."</dd>
            </div>
            <div>
                <dt>"Zweck"</dt>
                <dd>"Studierende sollen an einer Stelle sehen, wer ein Modul verantwortet und lehrt, so wie die BTU es selbst veröffentlicht."</dd>
            </div>
            <div>
                <dt>"Rechtsgrundlage"</dt>
                <dd>"Art. 6 Abs. 1 lit. f DSGVO. Das berechtigte Interesse ist, diese von der BTU veröffentlichten Angaben übersichtlich und durchsuchbar zugänglich zu machen. Betula zeigt sie im selben Zusammenhang wie die BTU und ergänzt nichts."</dd>
            </div>
            <div>
                <dt>"Empfänger"</dt>
                <dd>"alle, die Betula aufrufen; mit dem Katalog landen die Angaben auch in der Kopie in ihrem Browser. Die Seiten der Module können von Suchmaschinen erfasst werden."</dd>
            </div>
            <div>
                <dt>"Speicherdauer"</dt>
                <dd>"Betula baut seinen Datenstand bei jedem Einlesen aus dem aktuellen Stand der BTU. Ändert oder entfernt die BTU eine Angabe, ändert sie sich oder verschwindet mit dem nächsten Datenstand auch hier. Termine werden einen Monat nach ihrem letzten Datum gelöscht, Kopien von Seiten, die die BTU nicht mehr führt, nach sieben Tagen."</dd>
            </div>
            <div>
                <dt>"Widerspruch"</dt>
                <dd>"Du kannst der Verarbeitung jederzeit widersprechen (Art. 21 DSGVO); eine E-Mail an "{mail_link()}" genügt. Ist eine Angabe falsch, sag bitte Bescheid."</dd>
            </div>
        </dl>
    }
    .into_any()
}

fn privacy_recipients() -> AnyView {
    view! {
        <p>"Deine Daten verkaufe ich nicht und gebe sie nicht weiter. Andere bekommen sie nur, wie oben beschrieben: Contabo als Betreiber des Servers, mein E-Mail-Anbieter, wenn du mir schreibst, und dein Kalenderdienst, wenn du ein Abo anlegst. Betula selbst übermittelt keine Daten in Länder außerhalb der EU; für einen Kalenderdienst, den du wählst, gilt dessen Datenschutzerklärung. Es gibt keine automatisierten Entscheidungen und keine Profile."</p>
        <p>"Du musst keine Daten angeben, um Betula zu nutzen. Ohne die Angaben, die dein Browser bei jedem Aufruf mitschickt, etwa die IP-Adresse, lassen sich die Seiten aber nicht ausliefern."</p>
        <p>"Betula verlinkt auf die Originale bei der BTU und auf andere Seiten. Folgst du einem solchen Link, erfährt die andere Seite nur, dass du von betula.app kommst, nicht von welcher Seite; ab da gilt ihre Datenschutzerklärung."</p>
    }
    .into_any()
}

fn privacy_rights() -> AnyView {
    view! {
        <p>"Du hast das Recht auf Auskunft über deine Daten (Art. 15 DSGVO), auf Berichtigung (Art. 16), Löschung (Art. 17) und Einschränkung der Verarbeitung (Art. 18). Verarbeitungen, die auf Art. 6 Abs. 1 lit. f DSGVO beruhen – das sind alle hier beschriebenen –, kannst du aus Gründen, die sich aus deiner besonderen Situation ergeben, widersprechen (Art. 21). Eine formlose E-Mail an "{mail_link()}" genügt."</p>
        <p>"Weil Betula keine Konten kennt, kann ich Einträge im Zugriffsprotokoll nur finden, wenn du mir sagst, von welcher IP-Adresse und wann du Betula aufgerufen hast (Art. 11 DSGVO)."</p>
        <p>"Außerdem kannst du dich bei einer Datenschutz-Aufsichtsbehörde beschweren (Art. 77 DSGVO), zum Beispiel bei der für Brandenburg zuständigen:"</p>
        <address class="legal-address">
            <span>"Die Landesbeauftragte für den Datenschutz und für das Recht auf Akteneinsicht Brandenburg"</span>
            <span>"Stahnsdorfer Damm 77"</span>
            <span>"14532 Kleinmachnow"</span>
            <span><a href="https://www.lda.brandenburg.de/" rel="noopener">"www.lda.brandenburg.de"</a></span>
        </address>
    }
    .into_any()
}
