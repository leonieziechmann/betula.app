//! Texts of the Impressum and the Datenschutzerklärung (`pages/legal.rs`): one struct per part of
//! a page, in the order of the page, so that each language reads as the whole text.
//!
//! The German text is the one that counts (docs/folia/i18n.md); every other language is a translation
//! for convenience and says so (`translated`), keeping every fact, name, period and legal
//! reference. A change of the German text is a change of every translation. What the parts say
//! and where each comes from in the software: `pages/legal.rs`.

pub struct Texts {
    /// The name of the pages together: the sidebar's heading and its navigation, „Rechtliches".
    pub legal: &'static str,
    /// „Auf dieser Seite": the parts of the privacy notice in the sidebar.
    pub on_this_page: &'static str,
    /// The imprint's title (tab, link previews, sidebar), its heading and the line under it,
    /// which is also its description for search engines.
    pub imprint_title: &'static str,
    pub imprint_heading: &'static str,
    pub imprint_lead: &'static str,
    /// The same for the privacy notice: „Datenschutz", „Datenschutzerklärung".
    pub privacy_title: &'static str,
    pub privacy_heading: &'static str,
    pub privacy_lead: &'static str,
    /// „Stand: 25. September 2026": when the privacy notice last changed (day, month 1–12, year).
    pub as_of: fn(u32, u32, i32) -> String,
    /// A language whose text does not count: the note at the top of each page, before and after
    /// the link to the page in the language that does (the link names that page in its language).
    /// `None` for that language itself.
    pub translated: Option<Translated>,
    /// „E-Mail: " before the address.
    pub email_label: &'static str,
    /// The labels of the facts of a part („Welche Daten", „Zweck" …), the same in every part.
    pub facts: Facts,

    // Impressum
    pub operator: Operator,
    pub contact: Contact,
    pub not_btu: Prose,

    // Datenschutzerklärung, in the order of the page: what Betula is built to do with data, who
    // answers for it, what stays on the device, what leaves it when the student says so (Betula
    // Sync, sharing, a calendar subscription), what the server needs to run, and the rest.
    pub summary: Summary,
    pub controller: Controller,
    pub local: Local,
    pub sync: Sync,
    pub share: Share,
    pub calendar: Calendar,
    pub hosting: Prose,
    pub access_log: AccessLog,
    pub cookies: Cookies,
    pub email: Prose,
    pub lecturers: Lecturers,
    pub recipients: Prose,
    pub rights: Rights,
}

/// The note on a translated page: `before`, the link to the page that counts, `after`.
pub struct Translated {
    pub before: &'static str,
    pub after: &'static str,
}

pub struct Facts {
    /// „Welche Daten".
    pub data: &'static str,
    pub purpose: &'static str,
    /// „Rechtsgrundlage".
    pub legal_basis: &'static str,
    /// „Speicherdauer".
    pub retention: &'static str,
    /// „Ablauf": how Betula Sync works between the devices.
    pub course: &'static str,
    /// „Protokolle".
    pub logs: &'static str,
    /// „Weitergabe".
    pub disclosure: &'static str,
    /// „Beenden".
    pub ending: &'static str,
    /// „Quelle".
    pub source: &'static str,
    /// „Empfänger".
    pub recipients: &'static str,
    /// „Widerspruch".
    pub objection: &'static str,
    /// „Geräte verbinden": how a profile's key gets from one device to another.
    pub pairing: &'static str,
    /// The ways of sharing and of a calendar subscription, by the names the dialog „Teilen" gives
    /// them (`folia_app::quotes` holds them to it): „Fester Stand", „Bleibt aktuell",
    /// „Einmal-Link".
    pub fixed: &'static str,
    pub live: &'static str,
    pub once: &'static str,
}

/// A part that is only text: its heading and its paragraphs.
pub struct Prose {
    pub heading: &'static str,
    pub texts: &'static [&'static str],
}

/// „Angaben nach § 5 DDG": who runs Betula, with the postal address under the line.
pub struct Operator {
    pub heading: &'static str,
    pub run_by: &'static str,
}

/// „Kontakt": the e-mail address and a line under it.
pub struct Contact {
    pub heading: &'static str,
    pub hint: &'static str,
}

/// „Kurz gesagt": what Betula is built to do with data, and the points of the whole notice.
pub struct Summary {
    pub heading: &'static str,
    /// The principle, before the points: as little data as possible.
    pub intro: &'static str,
    pub points: &'static [&'static str],
}

/// „Verantwortlich": the line before the postal address and the e-mail address.
pub struct Controller {
    pub heading: &'static str,
    pub text: &'static str,
}

/// „Profile und lokale Daten": what a profile holds, what else the browser keeps, and that it
/// stays on the device (owner, 2026-11-18: „wir reden nicht von konten, weil die gibt es ja nicht
/// sondern von profilen und lokalen Daten").
pub struct Local {
    pub heading: &'static str,
    /// What a profile is and where it is kept, before the list of what it holds …
    pub intro: &'static str,
    pub profile: &'static [&'static str],
    /// … the line before the list of what the browser keeps besides the profiles …
    pub beside_intro: &'static str,
    pub beside: &'static [&'static str],
    /// … and the paragraphs after it.
    pub texts: &'static [&'static str],
}

/// „Betula Sync": what the service of Betula Sync (Phloem, docs/phloem/protocol.md) keeps of a
/// profile switched on, and what it cannot see.
pub struct Sync {
    pub heading: &'static str,
    pub intro: &'static str,
    /// What the service keeps, one point each …
    pub data: &'static [&'static str],
    /// … and what it cannot read.
    pub not_read: &'static str,
    pub pairing: &'static str,
    pub course: &'static str,
    pub logs: &'static str,
    pub legal_basis: &'static str,
    pub retention: &'static str,
    pub ending: &'static str,
}

/// „Stundenplan und Merkliste teilen": the three ways of the dialog „Teilen", each with what it
/// carries and what a server keeps of it.
pub struct Share {
    pub heading: &'static str,
    pub intro: &'static str,
    /// A copy of now, in the link itself: the Stundenplan's, then the Merkliste's.
    pub fixed: &'static [&'static str],
    /// What stays current: the part at the service, what it holds, what the service keeps of it.
    pub live: &'static [&'static str],
    /// A link for one person.
    pub once: &'static str,
    pub logs: &'static str,
    pub legal_basis: &'static str,
    pub disclosure: &'static str,
    pub ending: &'static str,
}

/// „Kalender-Abo".
pub struct Calendar {
    pub heading: &'static str,
    pub intro: &'static str,
    /// An address that carries the plan of now as a code …
    pub fixed: &'static str,
    /// … and one that stays current, with Betula Sync.
    pub live: &'static str,
    pub logs: &'static str,
    pub legal_basis: &'static str,
    pub disclosure: &'static str,
    pub ending: &'static str,
    /// The .ics download, after the facts.
    pub download: &'static str,
}

/// „Zugriffsprotokoll": the edge's access log and Folia's own.
pub struct AccessLog {
    pub heading: &'static str,
    pub intro: &'static str,
    /// What an entry holds, one point each …
    pub data: &'static [&'static str],
    /// … and what it does not.
    pub not_recorded: &'static str,
    pub purpose: &'static str,
    pub legal_basis: &'static str,
    pub retention: &'static str,
    /// Folia's own log, after the facts.
    pub app_log: &'static str,
    /// The traps (`folia/crates/server/src/trap.rs`): what the server keeps of an address it
    /// turns away for a while.
    pub traps: &'static str,
}

/// „Cookies": the gate's cookie, whose name the page writes as code between `before` and
/// `after`, and a paragraph about it.
pub struct Cookies {
    pub heading: &'static str,
    pub before: &'static str,
    pub after: &'static str,
    pub details: &'static str,
    /// The pass in front of the catalog (`folia/crates/server/src/pass.rs`): around its name, then
    /// what it holds.
    pub pass_before: &'static str,
    pub pass_after: &'static str,
    pub pass_details: &'static str,
}

/// „Namen von Lehrenden" (Art. 14 DSGVO).
pub struct Lecturers {
    pub heading: &'static str,
    pub intro: &'static str,
    pub data: &'static str,
    pub source: &'static str,
    pub purpose: &'static str,
    pub legal_basis: &'static str,
    pub recipients: &'static str,
    pub retention: &'static str,
    /// The objection, before and after the e-mail address.
    pub objection_before: &'static str,
    pub objection_after: &'static str,
}

/// „Deine Rechte".
pub struct Rights {
    pub heading: &'static str,
    /// The rights, before and after the e-mail address.
    pub before: &'static str,
    pub after: &'static str,
    /// Whom Betula can find: nobody by name (Art. 11 DSGVO); an entry of the access log by its
    /// address and time, a profile at Betula Sync not at all.
    pub finding: &'static str,
    /// The line before the supervisory authority's address.
    pub complaint: &'static str,
}

pub const DE: Texts = Texts {
    legal: "Rechtliches",
    on_this_page: "Auf dieser Seite",
    imprint_title: "Impressum",
    imprint_heading: "Impressum",
    imprint_lead: "Wer Betula betreibt und wie du Kontakt aufnimmst.",
    privacy_title: "Datenschutz",
    privacy_heading: "Datenschutzerklärung",
    privacy_lead: "Was du in Betula anlegst, liegt in deinem Profil auf deinem Gerät. Hier steht, welche Daten darüber hinaus anfallen, wozu und wie lange.",
    as_of: |day, month, year| format!("Stand: {day}. {} {year}", month_of(&MONTHS_DE, month)),
    translated: None,
    email_label: "E-Mail: ",
    facts: Facts {
        data: "Welche Daten",
        purpose: "Zweck",
        legal_basis: "Rechtsgrundlage",
        retention: "Speicherdauer",
        course: "Ablauf",
        logs: "Protokolle",
        disclosure: "Weitergabe",
        ending: "Beenden",
        source: "Quelle",
        recipients: "Empfänger",
        objection: "Widerspruch",
        pairing: "Geräte verbinden",
        fixed: "Fester Stand",
        live: "Bleibt aktuell",
        once: "Einmal-Link",
    },

    operator: Operator { heading: "Angaben nach § 5 DDG", run_by: "Betula wird betrieben von:" },
    contact: Contact { heading: "Kontakt", hint: "Fragen, Hinweise auf Fehler und Anregungen zu Betula gern per E-Mail." },
    not_btu: Prose {
        heading: "Kein Angebot der BTU",
        texts: &[
            "Betula ist ein privates, nicht-kommerzielles Projekt. Es gehört nicht zur Brandenburgischen Technischen Universität Cottbus-Senftenberg (BTU), und die BTU hat es weder beauftragt noch geprüft.",
            "Die Angaben zu Modulen, Studiengängen, Ordnungen und Terminen stammen von den öffentlichen Seiten der BTU. Betula ordnet sie und macht sie durchsuchbar, ändert aber nichts an ihrem Inhalt. Trotz aller Sorgfalt kann beim Einlesen etwas fehlen, falsch gelesen werden oder veralten. Verbindlich sind allein die Angaben der BTU; jede Seite von Betula verlinkt deshalb auf ihr Original.",
            "Für die Inhalte verlinkter Seiten ist allein verantwortlich, wer sie anbietet.",
        ],
    },

    summary: Summary {
        heading: "Kurz gesagt",
        intro: "Betula erhebt so wenig Daten wie möglich: nur, was der Betrieb der Seite und die Funktionen brauchen, die du selbst nutzt.",
        points: &[
            "Dein Profil liegt auf deinem Gerät. Merkliste, Stundenplan, „Mein Studium“ und „Mein Studiengang“ speichert dein Browser; nach deinem Namen oder einer E-Mail-Adresse fragt Betula nicht.",
            "Auf den Server kommt ein Profil nur, wenn du Betula Sync dafür einschaltest, und dann verschlüsselt: Nur deine Geräte können es öffnen, der Server nicht.",
            "Geteilt wird nur, was du selbst teilst, als Link oder als Kalender-Abo, und nur mit dem, was dafür nötig ist.",
            "Der Server protokolliert jeden Aufruf mit IP-Adresse, um Betula zu betreiben und vor Missbrauch zu schützen, und löscht die Einträge nach 7 Tagen.",
            "Keine Werbung, kein Tracking: Betula nutzt keine Analysedienste und setzt keine Cookies, die dich wiedererkennen. Schrift, Symbole und Skripte liefert betula.app selbst aus; von fremden Servern wird nichts geladen.",
            "Betula zeigt die Namen von Lehrenden, die die BTU in ihren Modulbeschreibungen veröffentlicht.",
        ],
    },
    controller: Controller {
        heading: "Verantwortlich",
        text: "Verantwortlich für die Verarbeitung personenbezogener Daten auf betula.app im Sinne der Datenschutz-Grundverordnung (DSGVO) ist:",
    },
    local: Local {
        heading: "Profile und lokale Daten",
        intro: "Was du in Betula anlegst, gehört zu einem Profil, und ein Profil liegt lokal auf deinem Gerät, im Speicher deines Browsers. Wer du bist, muss Betula dafür nicht wissen: Es fragt weder nach deinem Namen noch nach einer E-Mail-Adresse. Du kannst mehrere Profile anlegen; jedes hat seine eigenen Daten. Zu einem Profil gehören:",
        profile: &[
            "sein Name und der Zeitpunkt, an dem du es angelegt hast,",
            "deine Merkliste mit ihren Sammlungen, deinen Notizen zu den gemerkten Modulen und dem Standard für neue Merker,",
            "dein Stundenplan mit geplanten Modulen, Platzhaltern, Ausgeblendetem und ausgewählten Terminen, deinen eigenen Terminen, deinen Änderungen an Terminen aus QIS und deinen Notizen zu Terminen, deine gespeicherten Pläne und Kalender und die Adressen deiner Kalender-Abos,",
            "„Mein Studiengang“ mit Studienrichtung, Studienbeginn, Urlaubssemestern und Standort,",
            "„Mein Studium“ mit den Modulen, die du als bestanden abhakst, und den Semestern, in die du Module legst,",
            "was du teilst und wem du folgst: deine Freigaben mit ihren Schlüsseln und dem Namen, den du ihnen in der Liste „Geteilt“ gibst, und die geteilten Kalender und Listen, denen du folgst, mit ihren Links und dem letzten Stand einer Liste.",
        ],
        beside_intro: "Daneben speichert Betula lokal, was es zum Arbeiten braucht:",
        beside: &[
            "welches Profil gerade aktiv ist, und für die Startseite eine fertige Ansicht deiner nächsten Tage aus seinem Stundenplan, damit „Als Nächstes“ sofort dasteht, wenn du Betula öffnest; sie geht, wenn du ein Profil leerst oder löschst,",
            "Einstellungen, etwa die Sprache (beim ersten Besuch die deines Browsers), hell oder dunkel, die Breite der Seitenleisten, die Ansicht der Semester in „Mein Studium“ und was „Passt in meinen Stundenplan“ vergleicht,",
            "für die laufende Sitzung, wo du in jedem Bereich zuletzt warst, welchen Studiengang du im Stundenplan gewählt hast und wie weit du gescrollt hast,",
            "ob Betula hier als App installiert ist, an welchen Tagen du die Startseite geöffnet hast (die letzten 16) und ob du „Später“ oder „Nicht mehr anzeigen“ gewählt hast, damit Betula nur selten anbietet, sich zu installieren,",
            "eine Kopie des Katalogs, das Sprachmodell der Suche (sie läuft ganz in deinem Browser), die Dateien der App und bis zu 60 zuletzt besuchte Seiten, damit Betula schnell startet und auch ohne Netz funktioniert,",
            "für ein Profil mit Betula Sync seinen Schlüssel, eine zufällige Kennung dieses Geräts und den Stand des Abgleichs, mit den Änderungen, die noch nicht beim Dienst sind, und den Fassungen einer Notiz, die auf zwei Geräten gleichzeitig geändert wurde, und für einen Tag, welche deiner anderen Geräte nur über die Weiterleitung des Dienstes zu erreichen waren.",
        ],
        texts: &[
            "Das alles liegt auf deinem Gerät, und ich kann es nicht einsehen. Ein anderes Gerät oder ein anderer Browser hat seine eigenen Profile. Sicherst du ein Profil als Datei, entsteht die Datei in deinem Browser, und auch eine Sicherung, die du lädst, liest nur dein Browser. Legst du Betula auf einem iPhone oder Mac auf den Home-Bildschirm oder ins Dock, legt Betula dein Profil in die Zwischenablage deines Geräts, weil die App dort ihren eigenen Speicher hat, und die App liest es nur, wenn du „Studium einfügen“ tippst; auch das bleibt auf deinem Gerät.",
            "Liest du in „Mein Studium“ deine Notenübersicht ein, öffnet dein Browser die PDF auf deinem Gerät, in einem Hintergrundprozess, der danach beendet wird: Sie erreicht keinen Server und wird nicht gespeichert, und gelesen werden nur dein Studiengang, dein Studienbeginn und deine Module mit ihrem Status, keine Noten und nichts über dich. Was du übernimmst, gehört danach zu „Mein Studium“ wie alles, was du dort abhakst.",
            "Den Server erreicht davon nur, was in der Adresse einer Seite steht, wenn du sie aufrufst, neu lädst oder teilst. Eine Adresse sagt, was gerade angezeigt wird, aber nie, was auf deiner Merkliste oder in deinem Stundenplan steht, was du bestanden hast oder in deiner Merkliste suchst; eine Sammlung nennt sie nur bei ihrer Nummer, nie bei ihrem Namen. In einer Adresse steht etwa der Studiengang, nach dem der Katalog gefiltert ist (auch „Mein Studiengang“, wenn du den Katalog darüber öffnest), das Modul, das neben Merkliste, Stundenplan oder „Mein Studium“ offen ist, oder Semester und Platzhalter, für die du Module suchst. Solche Adressen stehen wie alle anderen im Zugriffsprotokoll.",
            "Mehr verlässt dein Gerät nur, wenn du es selbst auslöst: wenn du Betula Sync einschaltest, etwas teilst oder deinen Stundenplan als Kalender abonnierst. Was dann wohin geht, steht in den nächsten drei Teilen.",
            "Für all das nutzt Betula den Speicher deines Browsers (Local Storage, Session Storage, IndexedDB und Cache Storage). Die Speicherung ist für die Funktionen nötig, die du nutzt, und braucht deshalb keine Einwilligung (§ 25 Abs. 2 Nr. 2 TDDDG). Ein Profil leeren oder löschen kannst du auf der Seite „Profil“, alles zusammen jederzeit in den Einstellungen deines Browsers, indem du die Websitedaten von betula.app löschst.",
        ],
    },
    sync: Sync {
        heading: "Betula Sync",
        intro: "Betula Sync hält ein Profil auf deinen Geräten gleich. Es ist aus, bis du es für ein Profil einschaltest: auf der Seite „Profil“ oder dort, wo du etwas nutzt, das Sync braucht, etwa ein weiteres Gerät oder eine Freigabe, die aktuell bleibt. Betula sagt es dir jedes Mal vorher. Dann verschlüsselt dein Browser jede Änderung mit einem Schlüssel, den er selbst erzeugt und nur an deine eigenen Geräte weitergibt, und schickt sie an den Dienst von Betula Sync auf dem Server von Betula. Der Dienst bewahrt sie auf und reicht sie an deine anderen Geräte weiter; öffnen kann er sie nicht.",
        data: &[
            "die verschlüsselten Änderungen und einen verschlüsselten Gesamtstand des Profils, jeweils mit Größe und Zeitpunkt,",
            "zufällige Kennungen für das Profil und für jedes verbundene Gerät, mit der Nummer, die ein Gerät im Profil hat, dem Zeitpunkt, zu dem das Profil beim Dienst eingerichtet wurde, und dem des letzten Abrufs,",
            "einen Prüfwert, mit dem deine Geräte zeigen, dass sie zum Profil gehören; dein Browser leitet ihn aus dem Schlüssel ab, der Schlüssel selbst erreicht den Server nie,",
            "wenn du ein weiteres Gerät verbindest, für zehn Minuten ein verschlüsseltes Päckchen mit dem Schlüssel, das nur das neue Gerät öffnen kann und das nach dem Abholen gelöscht wird,",
            "was du mit Betula Sync teilst, als verschlüsselte Teile und Päckchen („Stundenplan und Merkliste teilen“).",
        ],
        not_read: "Was in deinem Profil steht, also Merkliste, Sammlungen, Notizen, Stundenplan, „Mein Studium“ und „Mein Studiengang“, kann der Dienst nicht lesen. Das gilt auch für die Liste deiner Geräte auf der Seite „Profil“: Jedes Gerät legt verschlüsselt im Profil ab, was es ist (Browser, System, bei manchen Handys das Modell, wie der Browser es nennt) und wann es zuletzt verbunden war, für deine anderen Geräte. Wem ein Profil gehört, weiß der Dienst nicht: Einen Namen, eine E-Mail-Adresse oder ein Passwort gibt es dafür nicht.",
        pairing: "Den Schlüssel bringt der Link oder QR-Code von „Gerät verbinden“ hinter dem „#“ auf dein anderes Gerät, und diesen Teil einer Adresse sendet kein Browser an einen Server. Bei „Mit dem Handy verbinden“ (/pair) geht es andersherum: Der Computer zeigt einen QR-Code mit einem öffentlichen Schlüssel hinter dem „#“, den er nur dafür macht, dein Handy legt damit verschlüsselt den Schlüssel des Profils beim Dienst ab, höchstens zehn Minuten, und der Computer holt ihn einmal ab; bis dahin fragt er alle paar Sekunden beim Dienst nach. Öffnen kann ihn nur dieser Computer, auch wer den QR-Code sieht, nicht. Die Kamera nutzt Betula dafür nur, wenn du einen Code scannst; das Bild bleibt auf deinem Gerät. Der Wiederherstellungscode ist derselbe Schlüssel zum Aufschreiben; er bleibt bei dir.",
        course: "Solange ein Gerät verbunden ist, hält es eine Verbindung zum Dienst offen; der Dienst sagt deinen anderen Geräten, wie viele Geräte des Profils gerade online sind. Sind zwei Geräte eines Profils zugleich online, verbinden sie sich auch direkt miteinander (WebRTC), und jede Änderung geht zugleich an das andere Gerät und an den Dienst. Dafür tauschen sie über den Dienst verschlüsselte Angaben zum Verbindungsaufbau aus, darunter ihre Netzadressen; der Dienst kann sie nicht lesen, und sie gehen nur an deine eigenen Geräte. Wo kein direkter Weg möglich ist, etwa im Mobilfunk, leitet der Dienst die verschlüsselten Datenpakete zwischen deinen Geräten weiter (TURN), ohne sie öffnen zu können: nur an die Adressen deiner anderen verbundenen Geräte und mit einem Zugang, der nur gilt, solange die Verbindung des Geräts steht.",
        logs: "Im Zugriffsprotokoll stehen die Anfragen an den Dienst wie jeder andere Aufruf, mit IP-Adresse, aber ohne Kennungen und ohne Inhalt; wie alles dort werden sie nach 7 Tagen gelöscht. Der Dienst selbst bewahrt keine IP-Adressen auf: Er kennt die Adresse eines Geräts, solange dessen Verbindung steht, und zählt im Arbeitsspeicher, wie viele Profile in der letzten Stunde von einer Adresse eingerichtet und wie viele Päckchen, geteilte Kalender und Listen von ihr abgerufen wurden, um Missbrauch zu bremsen. Für die Weiterleitung kennt er die Adressen der beteiligten Geräte nur im Arbeitsspeicher, solange sie läuft, und zählt, wie viel er weitergeleitet hat, ohne Adressen.",
        legal_basis: "Art. 6 Abs. 1 lit. f DSGVO. Mein berechtigtes Interesse ist, dir den Abgleich anzubieten, den du selbst einschaltest. Schlüssel, Gerätekennung und Stand des Abgleichs im Speicher deines Browsers braucht diese Funktion und deshalb keine Einwilligung (§ 25 Abs. 2 Nr. 2 TDDDG).",
        retention: "Bis du Sync beendest. Ältere Änderungen fasst ein Gerät von Zeit zu Zeit zu einem neuen Gesamtstand zusammen, und der Dienst löscht sie dann. Ruft 13 Monate lang kein Gerät das Profil ab, löscht der Dienst alles dazu. Eine Sicherung beim Dienst gibt es nicht: Gehen seine Daten verloren, legen deine Geräte das Profil beim nächsten Verbinden mit dem, was sie haben, von selbst wieder an.",
        ending: "„Sync beenden“ auf der Seite „Profil“ löscht alles zu diesem Profil sofort beim Dienst; jedes Gerät behält seine Kopie und ist wieder privat. „Nur dieses Gerät trennen“ nimmt nur dieses Gerät heraus.",
    },
    share: Share {
        heading: "Stundenplan und Merkliste teilen",
        intro: "Du kannst deinen Stundenplan, deine Merkliste oder eine Sammlung weitergeben, an andere oder an dein eigenes anderes Gerät. Geteilt wird nur, was du selbst teilst, und du wählst, wie:",
        fixed: &[
            "Eine Kopie von jetzt, als Link, der alles selbst enthält. Der Link zu deinem Stundenplan beginnt mit betula.app/studyplan?share= und enthält als Code das Semester, die geplanten Module in ihrer Reihenfolge, die Gruppen, die du ausgewählt hast, deine Änderungen an Terminen aus QIS (Tag, Zeit und den Raum, den du eingibst) und den Studiengang, dessen Modulkürzel verwendet werden; was du ausgeblendet hast, deine Notizen und deine eigenen Termine enthält er nicht. Wer ihn öffnet, sieht die Module und kann sie in den eigenen Stundenplan übernehmen oder deinen Plan mit deinen Gruppen und Änderungen als eigenen Kalender neben den eigenen legen; der bleibt in seinem Browser. Schickst du den Link über einen Messenger oder ein soziales Netzwerk, ruft dieses, je nach Dienst von deinem Gerät oder von den Servern des Anbieters, die Seite und ihr Vorschaubild ab, auf dem die Module stehen. Betula erstellt Seite und Bild bei jedem Abruf aus dem Code und den aktuellen Daten und speichert dazu nichts.",
            "Der Link zu deiner Merkliste trägt sie mit ihren Sammlungen und Notizen hinter dem „#“, und diesen Teil einer Adresse sendet kein Browser an einen Server; der Link einer Sammlung trägt ihren Namen, Notizen nur, wenn du sie mitnimmst.",
        ],
        live: &[
            "Mit Betula Sync kannst du so teilen, dass der Stundenplan, die Merkliste oder die Sammlung bei den anderen aktuell bleibt. Dann legt dein Browser beim Dienst von Betula Sync einen verschlüsselten Teil ab und erneuert ihn bei jeder Änderung; der Link trägt hinter dem „#“, den kein Browser an einen Server sendet, eine zufällige Kennung und den Schlüssel dazu. Der Dienst kann den Teil nicht öffnen.",
            "Der Teil eines Stundenplans enthält das Semester, die Module, die ausgewählten Gruppen und deine Änderungen an Terminen aus QIS, auf deinen Wunsch auch deine eigenen Termine und Notizen; der Teil einer Merkliste oder Sammlung die gemerkten Module, die Namen der Sammlungen und, wenn du sie mitnimmst, deine Notizen.",
            "Der Dienst bewahrt den Teil mit einer zufälligen Kennung, Größe und Zeitpunkt auf, bis du nicht mehr teilst, und danach 13 Monate lang nur die Kennung mit dem Vermerk, dass er nicht mehr geteilt wird. Folgst du einem geteilten Kalender oder einer geteilten Liste, kennt der Dienst, solange dein Gerät verbunden ist, im Arbeitsspeicher die Kennungen der Teile, über deren Änderungen es Bescheid bekommt.",
        ],
        once: "Ein Link für eine Person, ebenfalls mit Betula Sync: ein verschlüsseltes Päckchen beim Dienst, das höchstens 7 Tage wartet und beim ersten Öffnen gelöscht wird; auch sein Schlüssel steht hinter dem „#“. Danach bewahrt der Dienst 30 Tage lang nur auf, ob es geöffnet wurde, abgelaufen ist oder zurückgezogen wurde.",
        logs: "Einen festen Link zu deinem Stundenplan hält das Zugriffsprotokoll bei jedem Abruf mit der vollständigen Adresse und der IP-Adresse fest, von der er kommt; wie alles dort wird er nach 7 Tagen gelöscht. Das Protokoll der Anwendung enthält den Code nicht. Was hinter dem „#“ steht, erreicht kein Protokoll, und die Anfragen an den Dienst von Betula Sync stehen ohne Kennungen und ohne Inhalt im Zugriffsprotokoll („Betula Sync“).",
        legal_basis: "Art. 6 Abs. 1 lit. f DSGVO. Mein berechtigtes Interesse ist, dir das Teilen anzubieten, das du selbst auslöst.",
        disclosure: "Betula selbst gibt nichts weiter. Wer einen Link kennt, sieht, was er zeigt: Gib ihn nur an Menschen weiter, denen du deinen Plan oder deine Liste zeigen möchtest.",
        ending: "Ein fester Link lässt sich nicht zurückholen. Er zeigt aber nur, was beim Kopieren geplant oder gemerkt war; was du danach änderst, erreicht ihn nicht. „Nicht mehr teilen“ löscht einen Teil, der aktuell bleibt, sofort beim Dienst, „Zurückziehen“ ein Päckchen, das noch niemand geöffnet hat. Wer einem Kalender oder einer Liste folgte, behält den letzten Stand.",
    },
    calendar: Calendar {
        heading: "Kalender-Abo",
        intro: "Du kannst deinen Stundenplan als Kalender abonnieren. Dein Kalenderdienst, etwa von Apple, Google oder Microsoft, bekommt dafür eine Adresse und ruft sie regelmäßig ab, je nach Dienst von deinem Gerät oder von den Servern des Anbieters.",
        fixed: "Die Adresse beginnt mit betula.app/calendar/ und enthält als Code das Semester, die geplanten Module, den Studiengang, dessen Modulkürzel die Einträge verwenden, den Standort, was du ausgeblendet oder ausgewählt hast, und deine Änderungen an Terminen aus QIS: Tag, Zeit und den Raum, den du eingibst. Deine Notizen und deine eigenen Termine enthält sie nicht. Betula erstellt den Kalender bei jedem Abruf neu aus dem Code und den aktuellen Terminen und speichert dazu nichts.",
        live: "Mit Betula Sync kann ein Abo aktuell bleiben, ohne dass sich seine Adresse ändert. Sie beginnt dann mit betula.app/calendar/s/ und enthält statt eines Codes eine zufällige Kennung und den Schlüssel zu genau diesem Kalender, weil ein Kalenderdienst keinen eigenen Schlüssel mitschicken kann. Dein Browser legt beim Dienst von Betula Sync einen verschlüsselten Teil ab und erneuert ihn bei jeder Änderung; er enthält, was der Code eines Abos enthält, und auf deinen Wunsch deine eigenen Termine und Notizen. Bei jedem Abruf holt Betula den Teil beim Dienst, öffnet ihn mit dem Schlüssel aus der Adresse nur im Arbeitsspeicher, erstellt den Kalender und vergisst Schlüssel und Inhalt. Was der Dienst zu einem Teil aufbewahrt und wie lange, steht unter „Stundenplan und Merkliste teilen“.",
        logs: "Den Abruf einer Adresse mit festem Stand hält das Zugriffsprotokoll mit der vollständigen Adresse und der IP-Adresse fest, von der er kommt; wie alles dort wird er nach 7 Tagen gelöscht. Das Protokoll der Anwendung enthält den Code nicht. Für Adressen, die aktuell bleiben, schreibt das Zugriffsprotokoll keine Zeile, und das Protokoll der Anwendung enthält sie nicht.",
        legal_basis: "Art. 6 Abs. 1 lit. f DSGVO. Mein berechtigtes Interesse ist, dir das Abo anzubieten, das du selbst anlegst.",
        disclosure: "Wählst du „Google Kalender“ oder „Outlook“, öffnet dein Browser die Seite dieses Dienstes und übergibt ihm die Adresse; ab da gilt dessen Datenschutzerklärung. Betula selbst gibt nichts weiter. Wer die Adresse kennt, kann den Kalender ebenfalls abrufen: Gib sie nur an Menschen weiter, denen du deinen Plan zeigen möchtest.",
        ending: "Entferne den Kalender in deinem Kalenderdienst; dann wird die Adresse nicht mehr abgerufen. Ein Abo, das aktuell bleibt, nimmst du im Dialog „Teilen“ mit „Nicht mehr teilen“ zurück: Danach zeigt die Adresse nichts mehr.",
        download: "Eine heruntergeladene .ics-Datei entsteht in deinem Browser, mit deinen Notizen zu Terminen, und erreicht den Server nicht.",
    },
    hosting: Prose {
        heading: "Wo Betula läuft",
        texts: &[
            "Betula läuft auf einem gemieteten Server der Contabo GmbH, Welfenstraße 22, 81541 München, in einem Rechenzentrum in Deutschland. Dort wird alles verarbeitet, was beim Besuch anfällt, auch die Protokolle und ihre Auswertung, und dort läuft der Dienst von Betula Sync; andere Dienste sind daran nicht beteiligt.",
            "Contabo stellt den Server bereit und verarbeitet die Daten darauf nur in meinem Auftrag; dafür besteht ein Vertrag zur Auftragsverarbeitung (Art. 28 DSGVO). Die Verbindung zwischen deinem Browser und Betula ist verschlüsselt (HTTPS).",
        ],
    },
    access_log: AccessLog {
        heading: "Zugriffsprotokoll",
        intro: "Bei jedem Aufruf schickt dein Browser Angaben mit, ohne die der Server keine Seite ausliefern kann. Der Server hält jeden Aufruf in einem Zugriffsprotokoll fest.",
        data: &[
            "deine IP-Adresse und der Port deines Geräts,",
            "Datum und Uhrzeit,",
            "die aufgerufene Adresse, auch mit dem Teil nach dem „?“, in dem etwa Suchbegriffe und Filter stehen,",
            "die Art der Anfrage und der Verbindung, Statuscode, Größe und Dauer der Antwort,",
            "die Kennung deines Browsers (User-Agent).",
        ],
        not_recorded: "Nicht erfasst werden die Seite, von der du kommst, Cookies und alle übrigen Angaben deines Browsers.",
        purpose: "Betula stabil und sicher betreiben, Fehler finden, Angriffe und Missbrauch erkennen und abwehren. Dazu zählt der Server auch, wie viele Anfragen in kurzer Zeit von einer IP-Adresse kommen, und bremst, wenn es zu viele werden; diese Zählung liegt nur im Arbeitsspeicher.",
        legal_basis: "Art. 6 Abs. 1 lit. f DSGVO. Mein berechtigtes Interesse ist ein sicherer und funktionierender Betrieb.",
        retention: "Die Einträge stehen im Systemprotokoll des Servers und in einer Protokollauswertung auf demselben Server und werden an beiden Stellen nach 7 Tagen automatisch gelöscht.",
        app_log: "Daneben führt die Anwendung ein eigenes Protokoll: welche Seite wann aufgerufen wurde, ohne den Teil nach dem „?“, mit Statuscode und Dauer, aber ohne IP-Adresse und ohne Browserkennung. In der Protokollauswertung wird es nach 30 Tagen gelöscht.",
        traps: "Gegen Programme, die die Seite massenhaft abrufen, legt Betula Adressen aus, die kein Mensch und kein Teil der App aufruft, etwa einen Link, der auf keiner Seite zu sehen ist. Ruft ein Programm eine davon ab, beantwortet der Server Anfragen von dessen IP-Adresse eine Stunde lang nicht. Dafür merkt er sich bis zum Ablauf einen Fingerabdruck der Adresse, gebildet mit einem Schlüssel, den nur der laufende Server kennt, und nur im Arbeitsspeicher: nicht die Adresse selbst, nichts auf der Festplatte und nichts im Protokoll der Anwendung. Weil sich viele eine Adresse teilen können, zeigt die Seite, die du dann siehst, einen Knopf, mit dem es sofort weitergeht. Rechtsgrundlage ist Art. 6 Abs. 1 lit. f DSGVO; mein berechtigtes Interesse ist, dass Betula für alle erreichbar bleibt.",
    },
    cookies: Cookies {
        heading: "Cookies",
        before: "Betula setzt keine Cookies für Werbung oder Statistik, nur zwei, die für den Betrieb unbedingt nötig sind. Solange Betula nur mit Passwort erreichbar ist, etwa während eines geschlossenen Tests, setzt der Server nach der Eingabe des richtigen Passworts ein Cookie namens ",
        after: ".",
        details: "Es enthält nur, wann dein Zugang endet, und eine Signatur, an der der Server erkennt, dass er es selbst ausgestellt hat; gespeichert wird dazu nichts. Es gilt 90 Tage, wird nur über verschlüsselte Verbindungen an Betula gesendet und ist für Skripte nicht lesbar. Weil es für den Zugang, den du angefragt hast, unbedingt nötig ist, braucht es keine Einwilligung (§ 25 Abs. 2 Nr. 2 TDDDG).",
        pass_before: "Bevor die App den Katalog herunterlädt, löst sie im Hintergrund eine kleine Rechenaufgabe, die der Server stellt (meist in Bruchteilen einer Sekunde), und bekommt dafür ein Cookie namens ",
        pass_after: ". So bekommen Programme, die die Seite massenhaft abrufen, den Katalog nicht, ohne selbst zu rechnen, und Betula bleibt für alle erreichbar.",
        pass_details: "Es enthält nur, wann es abläuft, und eine Signatur, an der der Server erkennt, dass er es selbst ausgestellt hat, für das Netz, aus dem du Betula aufrufst (die ersten drei Teile einer IPv4-Adresse oder die ersten vier Blöcke einer IPv6-Adresse). Die Adresse selbst steht nicht darin, und gespeichert wird dazu nichts; die IP-Adresse der Anfrage verwendet der Server dafür auf derselben Grundlage wie für das Zugriffsprotokoll (Art. 6 Abs. 1 lit. f DSGVO). Es gilt 7 Tage, wird nur über verschlüsselte Verbindungen und nur mit den Anfragen der App an Betula gesendet und ist für Skripte nicht lesbar. Weil es für den Abruf des Katalogs, den du angefragt hast, unbedingt nötig ist, braucht es keine Einwilligung (§ 25 Abs. 2 Nr. 2 TDDDG).",
    },
    email: Prose {
        heading: "E-Mail",
        texts: &["Schreibst du mir eine E-Mail, verarbeite ich deine E-Mail-Adresse, deinen Namen, wenn du ihn nennst, und deine Nachricht, um dir zu antworten. Rechtsgrundlage ist Art. 6 Abs. 1 lit. f DSGVO; mein berechtigtes Interesse ist, Anfragen zu beantworten. Die Nachrichten liegen bei meinem E-Mail-Anbieter. Ich lösche sie, sobald dein Anliegen erledigt ist und sie nicht mehr gebraucht werden."],
    },
    lecturers: Lecturers {
        heading: "Namen von Lehrenden",
        intro: "Die Modulbeschreibungen und das Vorlesungsverzeichnis der BTU nennen, wer ein Modul verantwortet und wer eine Veranstaltung hält. Betula übernimmt diese Angaben und zeigt sie bei den Modulen und Terminen; im Katalog lässt sich nach ihnen filtern. Die folgenden Angaben richten sich an die genannten Personen (Art. 14 DSGVO).",
        data: "Name, akademischer Titel und Rolle (etwa verantwortlich oder lehrend), jeweils mit dem Modul oder der Veranstaltung.",
        source: "die öffentlich zugänglichen Seiten der BTU Cottbus-Senftenberg, vor allem die Modulbeschreibungen und das Vorlesungsverzeichnis im Portal QIS.",
        purpose: "Studierende sollen an einer Stelle sehen, wer ein Modul verantwortet und lehrt, so wie die BTU es selbst veröffentlicht.",
        legal_basis: "Art. 6 Abs. 1 lit. f DSGVO. Das berechtigte Interesse ist, diese von der BTU veröffentlichten Angaben übersichtlich und durchsuchbar zugänglich zu machen. Betula zeigt sie im selben Zusammenhang wie die BTU und ergänzt nichts.",
        recipients: "alle, die Betula aufrufen; mit dem Katalog landen die Angaben auch in der Kopie in ihrem Browser. Die Seiten der Module können von Suchmaschinen erfasst werden.",
        retention: "Betula baut seinen Datenstand bei jedem Einlesen aus dem aktuellen Stand der BTU. Ändert oder entfernt die BTU eine Angabe, ändert sie sich oder verschwindet mit dem nächsten Datenstand auch hier. Termine werden einen Monat nach ihrem letzten Datum gelöscht, Kopien von Seiten, die die BTU nicht mehr führt, nach sieben Tagen.",
        objection_before: "Du kannst der Verarbeitung jederzeit widersprechen (Art. 21 DSGVO); eine E-Mail an ",
        objection_after: " genügt. Ist eine Angabe falsch, sag bitte Bescheid.",
    },
    recipients: Prose {
        heading: "Weitergabe und Links",
        texts: &[
            "Deine Daten verkaufe ich nicht und gebe sie nicht weiter. Andere bekommen sie nur, wie oben beschrieben: Contabo als Betreiber des Servers, mein E-Mail-Anbieter, wenn du mir schreibst, dein Kalenderdienst, wenn du ein Abo anlegst, und wem du selbst einen Link gibst. Betula selbst übermittelt keine Daten in Länder außerhalb der EU; für einen Kalenderdienst, den du wählst, gilt dessen Datenschutzerklärung. Es gibt keine automatisierten Entscheidungen und kein Profiling im Sinne der DSGVO; ein Profil in Betula ist nur der Ort, an dem deine eigenen Daten liegen.",
            "Du musst keine Daten angeben, um Betula zu nutzen. Ohne die Angaben, die dein Browser bei jedem Aufruf mitschickt, etwa die IP-Adresse, lassen sich die Seiten aber nicht ausliefern.",
            "Betula verlinkt auf die Originale bei der BTU und auf andere Seiten. Folgst du einem solchen Link, erfährt die andere Seite nur, dass du von betula.app kommst, nicht von welcher Seite; ab da gilt ihre Datenschutzerklärung.",
        ],
    },
    rights: Rights {
        heading: "Deine Rechte",
        before: "Du hast das Recht auf Auskunft über deine Daten (Art. 15 DSGVO), auf Berichtigung (Art. 16), Löschung (Art. 17) und Einschränkung der Verarbeitung (Art. 18). Verarbeitungen, die auf Art. 6 Abs. 1 lit. f DSGVO beruhen – das sind alle hier beschriebenen –, kannst du aus Gründen, die sich aus deiner besonderen Situation ergeben, widersprechen (Art. 21). Eine formlose E-Mail an ",
        after: " genügt.",
        finding: "Betula weiß nicht, wer hinter einem Aufruf oder einem Profil steht (Art. 11 DSGVO). Einträge im Zugriffsprotokoll kann ich deshalb nur finden, wenn du mir sagst, von welcher IP-Adresse und wann du Betula aufgerufen hast. Ein Profil auf deinem Gerät kann ich nicht einsehen. Bei Betula Sync liegt ein Profil unter einer zufälligen Kennung, ohne Namen, und lesen kann ich es nicht: Was darin steht, zeigt dir dein eigenes Gerät, und löschen kannst du es jederzeit selbst mit „Sync beenden“.",
        complaint: "Außerdem kannst du dich bei einer Datenschutz-Aufsichtsbehörde beschweren (Art. 77 DSGVO), zum Beispiel bei der für Brandenburg zuständigen:",
    },
};

/// A translation for convenience (British English, docs/folia/i18n.md). „DSGVO" is the GDPR; each
/// reference names both, so that it can be found in the German text that counts.
pub const EN: Texts = Texts {
    legal: "Legal",
    on_this_page: "On this page",
    imprint_title: "Legal notice",
    imprint_heading: "Legal notice",
    imprint_lead: "Who runs Betula and how to get in touch.",
    privacy_title: "Privacy",
    privacy_heading: "Privacy policy",
    privacy_lead: "What you create in Betula is kept in your profile on your device. This page says what data arises beyond that, what for and for how long.",
    as_of: |day, month, year| format!("As of {day} {} {year}", month_of(&MONTHS_EN, month)),
    translated: Some(Translated {
        before: "This English version is a translation for your convenience. Only the German text is legally binding: ",
        after: ".",
    }),
    email_label: "E-mail: ",
    facts: Facts {
        data: "What data",
        purpose: "Purpose",
        legal_basis: "Legal basis",
        retention: "Retention period",
        course: "How it works",
        logs: "Logs",
        disclosure: "Disclosure",
        ending: "Ending it",
        source: "Source",
        recipients: "Recipients",
        objection: "Objection",
        pairing: "Connecting devices",
        fixed: "Fixed copy",
        live: "Stays current",
        once: "One-time link",
    },

    operator: Operator { heading: "Information pursuant to § 5 DDG", run_by: "Betula is run by:" },
    contact: Contact { heading: "Contact", hint: "Questions, reports of errors and suggestions about Betula are welcome by e-mail." },
    not_btu: Prose {
        heading: "Not a service of BTU",
        texts: &[
            "Betula is a private, non-commercial project. It does not belong to the Brandenburg University of Technology Cottbus-Senftenberg (BTU), and BTU has neither commissioned nor reviewed it.",
            "The information on modules, degree programmes, regulations and dates comes from BTU's public pages. Betula organises it and makes it searchable, but changes nothing in its content. Despite all care, something may be missed, misread or become out of date when it is read in. Only BTU's information is binding; that is why every page of Betula links to its original.",
            "Responsibility for the content of linked pages lies solely with whoever provides them.",
        ],
    },

    summary: Summary {
        heading: "In short",
        intro: "Betula collects as little data as possible: only what running the site and the functions you use yourself require.",
        points: &[
            "Your profile is kept on your device. Your browser stores your saved modules, timetable, “My studies” and “My programme”; Betula does not ask for your name or an e-mail address.",
            "A profile reaches the server only if you switch on Betula Sync for it, and then encrypted: only your devices can open it, the server cannot.",
            "Only what you share yourself is shared, as a link or as a calendar subscription, and only with what that takes.",
            "The server logs every request with its IP address, to run Betula and protect it from abuse, and deletes the entries after 7 days.",
            "No advertising, no tracking: Betula uses no analytics services and sets no cookies that recognise you. betula.app serves its fonts, icons and scripts itself; nothing is loaded from other servers.",
            "Betula shows the names of lecturers that BTU publishes in its module descriptions.",
        ],
    },
    controller: Controller {
        heading: "Controller",
        text: "The controller for the processing of personal data on betula.app within the meaning of the General Data Protection Regulation (GDPR; in German, DSGVO) is:",
    },
    local: Local {
        heading: "Profiles and local data",
        intro: "What you create in Betula belongs to a profile, and a profile is kept locally on your device, in your browser's storage. Betula does not need to know who you are for this: it asks neither for your name nor for an e-mail address. You can create several profiles; each has its own data. A profile holds:",
        profile: &[
            "its name and when you created it,",
            "your saved modules with their collections, your notes on them and the default for new saves,",
            "your timetable with planned modules, placeholders, what you have hidden and the dates you have selected, your own events, your changes to sessions from QIS and your notes on sessions, your saved plans and calendars and the addresses of your calendar subscriptions,",
            "“My programme” with study track, start of studies, semesters of leave and location,",
            "“My studies” with the modules you tick off as passed and the semesters you put modules into,",
            "what you share and what you follow: your shares with their keys and the name you give them in the list “Shared”, and the shared calendars and lists you follow, with their links and a list's last state.",
        ],
        beside_intro: "Besides that, Betula stores locally what it needs to work:",
        beside: &[
            "which profile is active, and for the start page a ready view of your next days from its timetable, so that “Next” is there at once when you open Betula; it goes when you empty or delete a profile,",
            "settings, such as the language (on your first visit, your browser's), light or dark, the width of the sidebars, the view of the semesters in “My studies” and what “Fits my timetable” compares,",
            "for the current browser session, where you last were in each area, which degree programme you chose in the timetable and how far you had scrolled,",
            "whether Betula is installed as an app here, on which days you opened the start page (the last 16) and whether you chose “Later” or “Don't show again”, so that Betula offers to install itself only now and then,",
            "a copy of the catalogue, the language model of the search (it runs entirely in your browser), the app's files and up to 60 recently visited pages, so that Betula starts quickly and also works offline,",
            "for a profile with Betula Sync, its key, a random id of this device and the state of the syncing, with the changes not yet at the service and the versions of a note that was changed on two devices at the same time, and for one day which of your other devices could be reached only through the service's relay.",
        ],
        texts: &[
            "All of this is kept on your device, and I cannot see it. Another device or another browser has its own profiles. If you save a profile as a file, the file is made in your browser, and a backup you load is read only by your browser too. If you put Betula on the home screen or in the Dock of an iPhone or a Mac, Betula puts your profile on your device's clipboard, because the app keeps storage of its own there, and the app reads it only when you tap “Paste study”; this too stays on your device.",
            "When you read your transcript in “My studies”, your browser opens the PDF on your device, in a background process that is ended afterwards: it reaches no server and is not stored, and only your programme, the start of your studies and your modules with their status are read, no grades and nothing about you. What you take over then belongs to “My studies” like everything you tick off there.",
            "Of all this, only what is in the address of a page reaches the server, when you open, reload or share it. An address says what is being shown, but never what is among your saved modules or in your timetable, what you have passed or what you search for among your saved modules; it names a collection only by its number, never by its name. An address holds for example the degree programme the catalogue is filtered by (also “My programme”, if you open the catalogue through it), the module open beside your saved modules, your timetable or “My studies”, or the semester and placeholder you are looking for modules for. Such addresses are in the access log like all others.",
            "More than that leaves your device only when you start it yourself: when you switch on Betula Sync, share something or subscribe to your timetable as a calendar. The next three parts say what then goes where.",
            "For all of this Betula uses your browser's storage (Local Storage, Session Storage, IndexedDB and Cache Storage). Storing it is necessary for the functions you use and therefore requires no consent (§ 25(2) no. 2 TDDDG). You can empty or delete a profile on the “Profile” page, and all of it at any time in your browser's settings by deleting the site data of betula.app.",
        ],
    },
    sync: Sync {
        heading: "Betula Sync",
        intro: "Betula Sync keeps a profile the same on your devices. It is off until you switch it on for a profile: on the “Profile” page, or where you use something that needs Sync, such as another device or a share that stays current. Betula tells you each time before it does. Then your browser encrypts every change with a key it makes itself and passes only to your own devices, and sends it to the service of Betula Sync on Betula's server. The service keeps the changes and hands them on to your other devices; it cannot open them.",
        data: &[
            "the encrypted changes and an encrypted overall state of the profile, each with its size and time,",
            "random ids for the profile and for every connected device, with the number a device has in the profile, the time the profile was set up at the service and the time of the last fetch,",
            "a check value by which your devices show that they belong to the profile; your browser derives it from the key, and the key itself never reaches the server,",
            "when you connect another device, for ten minutes an encrypted parcel with the key, which only the new device can open and which is deleted once it is fetched,",
            "what you share with Betula Sync, as encrypted parts and parcels (“Sharing your timetable and saved modules”).",
        ],
        not_read: "What is in your profile, such as saved modules, collections, notes, timetable, “My studies” and “My programme”, the service cannot read. Nor the list of your devices on the “Profile” page: each device leaves in the profile, encrypted, what it is (browser, system and, for some phones, the model as the browser names it) and when it was last connected, for your other devices. The service does not know whose profile it is: there is no name, e-mail address or password for it.",
        pairing: "The link or QR code of “Connect a device” takes the key to your other device after the “#”, and no browser sends that part of an address to a server. “Connect with your phone” (/pair) goes the other way round: the computer shows a QR code with a public key after the “#”, made for this alone, your phone uses it to leave the profile's key with the service, encrypted, for ten minutes at most, and the computer fetches it once; until then it asks the service every few seconds. Only this computer can open it, not even someone who sees the QR code. Betula uses the camera for this only while you scan a code; the picture stays on your device. The recovery code is the same key, to write down; it stays with you.",
        course: "While a device is connected, it keeps a connection to the service open; the service tells your other devices how many devices of the profile are online right now. When two devices of a profile are online at the same time, they also connect with each other directly (WebRTC), and every change goes to the other device and to the service at once. For this they exchange encrypted details for setting up the connection through the service, among them their network addresses; the service cannot read them, and they go to your own devices only. Where no direct way is possible, for instance in a mobile network, the service relays the encrypted packets between your devices (TURN) without being able to open them: only to the addresses of your other connected devices, and with an access that holds only while the device's connection lasts.",
        logs: "The access log records the requests to the service like every other request, with the IP address, but without ids and without content; like everything there, they are deleted after 7 days. The service itself keeps no IP addresses: it knows a device's address while its connection lasts, and counts in memory how many profiles were set up from one address in the last hour and how many parcels, shared calendars and lists were fetched from it, to slow down abuse. For relaying, it knows the addresses of the devices involved in memory only, while the relaying lasts, and counts how much it relayed, without addresses.",
        legal_basis: "Art. 6(1)(f) GDPR (DSGVO). My legitimate interest is offering you the syncing you switch on yourself. The key, the device's id and the state of the syncing in your browser's storage are necessary for this function and therefore require no consent (§ 25(2) no. 2 TDDDG).",
        retention: "Until you end Sync. From time to time a device combines older changes into a new overall state, and the service then deletes them. If no device fetches the profile for 13 months, the service deletes everything about it. The service keeps no backup: if its data is lost, your devices set the profile up there again by themselves the next time they connect, from what they hold.",
        ending: "“End Sync” on the “Profile” page deletes everything about this profile at the service at once; every device keeps its copy and is private again. “Disconnect only this device” takes only this device out.",
    },
    share: Share {
        heading: "Sharing your timetable and saved modules",
        intro: "You can pass on your timetable, your saved modules or a collection, to others or to another device of your own. Only what you share yourself is shared, and you choose how:",
        fixed: &[
            "A copy of now, as a link that holds everything itself. The link to your timetable begins with betula.app/studyplan?share= and contains, as a code, the semester, the planned modules in their order, the groups you have chosen, your changes to sessions from QIS (day, time and the room you enter) and the degree programme whose module abbreviations are used; it does not contain what you have hidden, your notes or your own events. Whoever opens it sees the modules and can add them to their own timetable or put your plan, with your groups and changes, beside theirs as a calendar of its own; it stays in their browser. If you send the link via a messenger or a social network, that service fetches the page and its preview image, which shows the modules, depending on the service from your device or from the provider's servers. Betula creates the page and the image from the code and the current data on every request and stores nothing for it.",
            "The link to your saved modules carries them with their collections and notes after the “#”, and no browser sends that part of an address to a server; the link of a collection carries its name, and notes only if you include them.",
        ],
        live: &[
            "With Betula Sync you can share so that the timetable, the saved modules or the collection stay current for the others. Then your browser leaves an encrypted part at the service of Betula Sync and renews it with every change; the link carries a random id and the key to it after the “#”, which no browser sends to a server. The service cannot open the part.",
            "A timetable's part contains the semester, the modules, the groups you chose and your changes to sessions from QIS, if you wish also your own events and notes; the part of your saved modules or of a collection contains the saved modules, the names of the collections and, if you include them, your notes.",
            "The service keeps the part with a random id, its size and time until you stop sharing, and after that for 13 months only the id with a note that it is no longer shared. If you follow a shared calendar or list, the service knows in memory, while your device is connected, the ids of the parts whose changes it is told about.",
        ],
        once: "A link for one person, with Betula Sync as well: an encrypted parcel at the service that waits at most 7 days and is deleted when it is first opened; its key, too, is after the “#”. After that the service keeps for 30 days only whether it was opened, expired or withdrawn.",
        logs: "The access log records every request of a fixed link to your timetable with the full address and the IP address it comes from; like everything there, it is deleted after 7 days. The application's log does not contain the code. What is after the “#” reaches no log, and the requests to the service of Betula Sync are in the access log without ids and without content (“Betula Sync”).",
        legal_basis: "Art. 6(1)(f) GDPR (DSGVO). My legitimate interest is offering you the sharing you start yourself.",
        disclosure: "Betula itself passes nothing on. Anyone who knows a link sees what it shows: only give it to people you want to show your plan or your list to.",
        ending: "A fixed link cannot be taken back. But it only shows what was planned or saved when it was copied; what you change afterwards does not reach it. “Stop sharing” deletes a part that stays current at the service at once, “Withdraw” a parcel nobody has opened yet. Whoever followed a calendar or a list keeps the last state.",
    },
    calendar: Calendar {
        heading: "Calendar subscription",
        intro: "You can subscribe to your timetable as a calendar. For this, your calendar service, for example from Apple, Google or Microsoft, gets an address and fetches it regularly, depending on the service from your device or from the provider's servers.",
        fixed: "The address begins with betula.app/calendar/ and contains, as a code, the semester, the planned modules, the degree programme whose module abbreviations the entries use, the location, what you have hidden or selected, and your changes to sessions from QIS: day, time and the room you enter. It does not contain your notes or your own events. Betula creates the calendar anew from the code and the current dates on every request and stores nothing for it.",
        live: "With Betula Sync a subscription can stay current without its address changing. The address then begins with betula.app/calendar/s/ and contains, instead of a code, a random id and the key to exactly this calendar, because a calendar service cannot send a key of its own. Your browser leaves an encrypted part at the service of Betula Sync and renews it with every change; it contains what the code of a subscription contains and, if you wish, your own events and notes. On every request Betula fetches the part from the service, opens it with the key from the address in memory only, creates the calendar and forgets key and content. What the service keeps of a part, and for how long, is under “Sharing your timetable and saved modules”.",
        logs: "The access log records every request of an address with a fixed copy with the full address and the IP address it comes from; like everything there, it is deleted after 7 days. The application's log does not contain the code. For addresses that stay current the access log writes no line, and the application's log does not contain them.",
        legal_basis: "Art. 6(1)(f) GDPR (DSGVO). My legitimate interest is offering you the subscription you set up yourself.",
        disclosure: "If you choose “Google Calendar” or “Outlook”, your browser opens that service's page and hands it the address; from then on, its privacy policy applies. Betula itself passes nothing on. Anyone who knows the address can fetch the calendar too: only give it to people you want to show your plan to.",
        ending: "Remove the calendar in your calendar service; then the address is no longer fetched. A subscription that stays current you take back in the dialog “Share” with “Stop sharing”: after that the address shows nothing.",
        download: "A downloaded .ics file is created in your browser, with your notes on sessions, and does not reach the server.",
    },
    hosting: Prose {
        heading: "Where Betula runs",
        texts: &[
            "Betula runs on a rented server of Contabo GmbH, Welfenstraße 22, 81541 München, in a data centre in Germany. Everything that arises during a visit is processed there, including the logs and their analysis, and the service of Betula Sync runs there too; no other services are involved.",
            "Contabo provides the server and processes the data on it only on my behalf; there is a data processing agreement for this under Art. 28 GDPR (DSGVO). The connection between your browser and Betula is encrypted (HTTPS).",
        ],
    },
    access_log: AccessLog {
        heading: "Access log",
        intro: "With every request, your browser sends information without which the server cannot deliver a page. The server records every request in an access log.",
        data: &[
            "your IP address and the port of your device,",
            "date and time,",
            "the address requested, including the part after the “?”, which holds such things as search terms and filters,",
            "the kind of request and connection, the status code, size and duration of the response,",
            "your browser's identifier (user agent).",
        ],
        not_recorded: "The page you came from, cookies and all other information from your browser are not recorded.",
        purpose: "To run Betula reliably and securely, to find errors, and to detect and fend off attacks and abuse. For this, the server also counts how many requests come from one IP address in a short time, and slows them down when there are too many; this count is kept only in memory.",
        legal_basis: "Art. 6(1)(f) GDPR (DSGVO). My legitimate interest is secure and functioning operation.",
        retention: "The entries are kept in the server's system log and in a log analysis on the same server, and are deleted automatically in both places after 7 days.",
        app_log: "In addition, the application keeps a log of its own: which page was requested when, without the part after the “?”, with status code and duration, but without IP address and without browser identifier. In the log analysis it is deleted after 30 days.",
        traps: "Against programs that fetch the site in bulk, Betula lays out addresses that no person and no part of the app requests, such as a link that cannot be seen on any page. If a program requests one of them, the server does not answer requests from its IP address for one hour. For this it keeps a fingerprint of the address until the hour is over, made with a key only the running server knows, and in memory only: not the address itself, nothing on disk and nothing in the application's log. Because many people can share one address, the page you then see has a button that lets you go on at once. The legal basis is Art. 6(1)(f) GDPR (DSGVO); my legitimate interest is that Betula stays reachable for everyone.",
    },
    cookies: Cookies {
        heading: "Cookies",
        before: "Betula sets no cookies for advertising or statistics, only two that are strictly necessary for running it. While Betula can only be reached with a password, for example during a closed test, the server sets a cookie named ",
        after: " once the correct password has been entered.",
        details: "It contains only when your access ends, and a signature by which the server recognises that it issued the cookie itself; nothing is stored for it. It is valid for 90 days, is sent to Betula only over encrypted connections and cannot be read by scripts. Because it is strictly necessary for the access you asked for, it requires no consent (§ 25(2) no. 2 TDDDG).",
        pass_before: "Before the app downloads the catalogue, it solves a small computing task set by the server in the background (usually in a fraction of a second) and receives a cookie named ",
        pass_after: " for it. This way programs that fetch the site in bulk do not get the catalogue without doing the work themselves, and Betula stays reachable for everyone.",
        pass_details: "It contains only when it expires, and a signature by which the server recognises that it issued the cookie itself, for the network you reach Betula from (the first three parts of an IPv4 address or the first four groups of an IPv6 address). The address itself is not in it, and nothing is stored for it; the server uses the request's IP address for this on the same basis as for the access log (Art. 6(1)(f) GDPR (DSGVO)). It is valid for 7 days, is sent to Betula only over encrypted connections and only with the app's requests, and cannot be read by scripts. Because it is strictly necessary for downloading the catalogue you asked for, it requires no consent (§ 25(2) no. 2 TDDDG).",
    },
    email: Prose {
        heading: "E-mail",
        texts: &["If you write me an e-mail, I process your e-mail address, your name if you give it, and your message in order to answer you. The legal basis is Art. 6(1)(f) GDPR (DSGVO); my legitimate interest is answering enquiries. The messages are kept with my e-mail provider. I delete them as soon as your matter has been dealt with and they are no longer needed."],
    },
    lecturers: Lecturers {
        heading: "Names of lecturers",
        intro: "BTU's module descriptions and course catalogue name who is responsible for a module and who teaches a course. Betula takes this information and shows it with the modules and their dates; the catalogue can be filtered by it. The following information is addressed to the persons named, as Art. 14 GDPR (DSGVO) requires.",
        data: "Name, academic title and role (for example responsible or teaching), each with the module or the course.",
        source: "the publicly accessible pages of BTU Cottbus-Senftenberg, above all the module descriptions and the course catalogue in the QIS portal.",
        purpose: "Students should be able to see in one place who is responsible for a module and who teaches it, just as BTU publishes it itself.",
        legal_basis: "Art. 6(1)(f) GDPR (DSGVO). The legitimate interest is making this information published by BTU accessible in a clear and searchable way. Betula shows it in the same context as BTU and adds nothing.",
        recipients: "everyone who visits Betula; with the catalogue, the information also ends up in the copy in their browser. The pages of the modules may be indexed by search engines.",
        retention: "Betula builds its data from BTU's current state every time it reads it in. If BTU changes or removes an item, it changes or disappears here too with the next update of the data. Dates are deleted one month after their last occurrence, copies of pages BTU no longer lists after seven days.",
        objection_before: "You can object to the processing at any time under Art. 21 GDPR (DSGVO); an e-mail to ",
        objection_after: " is enough. If any information is wrong, please let me know.",
    },
    recipients: Prose {
        heading: "Disclosure and links",
        texts: &[
            "I do not sell your data and do not pass it on. Others only receive it as described above: Contabo as the operator of the server, my e-mail provider when you write to me, your calendar service when you set up a subscription, and whoever you give a link to yourself. Betula itself transfers no data to countries outside the EU; for a calendar service you choose, its privacy policy applies. There are no automated decisions and no profiling within the meaning of the GDPR (DSGVO); a profile in Betula is only the place where your own data is kept.",
            "You do not have to provide any data to use Betula. Without the information your browser sends with every request, such as the IP address, the pages cannot be delivered, though.",
            "Betula links to the originals at BTU and to other pages. If you follow such a link, the other site only learns that you come from betula.app, not from which page; from then on, its privacy policy applies.",
        ],
    },
    rights: Rights {
        heading: "Your rights",
        before: "Under the GDPR (DSGVO), you have the right of access to your data (Art. 15), to rectification (Art. 16), to erasure (Art. 17) and to restriction of processing (Art. 18). You can object to processing based on Art. 6(1)(f) GDPR (DSGVO) – which is all the processing described here – on grounds relating to your particular situation (Art. 21). An informal e-mail to ",
        after: " is enough.",
        finding: "Betula does not know who is behind a request or a profile; see Art. 11 GDPR (DSGVO). I can therefore only find entries in the access log if you tell me from which IP address and when you visited Betula. A profile on your device I cannot see. At Betula Sync a profile is kept under a random id, without a name, and I cannot read it: your own device shows you what is in it, and you can delete it yourself at any time with “End Sync”.",
        complaint: "You also have the right to lodge a complaint with a data protection supervisory authority under Art. 77 GDPR (DSGVO), for example with the one responsible for Brandenburg:",
    },
};

const MONTHS_DE: [&str; 12] = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"];
const MONTHS_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// The month (1–12) of `months`, spelled out; nothing for what is no month.
fn month_of(months: &[&'static str; 12], month: u32) -> &'static str {
    usize::try_from(month).ok().and_then(|month| month.checked_sub(1)).and_then(|i| months.get(i)).copied().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_date_of_the_privacy_notice_in_each_language() {
        assert_eq!((DE.as_of)(25, 9, 2026), "Stand: 25. September 2026");
        assert_eq!((EN.as_of)(5, 10, 2026), "As of 5 October 2026");
    }

    #[test]
    fn only_the_language_that_counts_says_nothing_about_a_translation() {
        assert!(DE.translated.is_none());
        assert!(EN.translated.is_some());
    }
}
