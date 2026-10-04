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

    // Datenschutzerklärung
    pub summary: Prose,
    pub controller: Controller,
    pub hosting: Prose,
    pub access_log: AccessLog,
    pub browser: Browser,
    pub calendar: Calendar,
    pub share: Share,
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
    /// „Ablauf": how a calendar subscription or a shared link works.
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
}

/// A part that is only text: its heading and its paragraphs (or the points of its list).
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

/// „Verantwortlich": the line before the postal address and the e-mail address.
pub struct Controller {
    pub heading: &'static str,
    pub text: &'static str,
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
}

/// „Speicher im Browser".
pub struct Browser {
    pub heading: &'static str,
    /// The line before the list of what is kept …
    pub intro: &'static str,
    pub kept: &'static [&'static str],
    /// … and the paragraphs after it.
    pub texts: &'static [&'static str],
}

/// „Kalender-Abo".
pub struct Calendar {
    pub heading: &'static str,
    pub intro: &'static str,
    pub course: &'static str,
    pub logs: &'static str,
    pub legal_basis: &'static str,
    pub disclosure: &'static str,
    pub ending: &'static str,
    /// The .ics download, after the facts.
    pub download: &'static str,
}

/// „Stundenplan teilen".
pub struct Share {
    pub heading: &'static str,
    pub intro: &'static str,
    pub course: &'static str,
    pub logs: &'static str,
    pub legal_basis: &'static str,
    pub disclosure: &'static str,
    pub ending: &'static str,
}

/// „Cookies": the gate's cookie, whose name the page writes as code between `before` and
/// `after`, and a paragraph about it.
pub struct Cookies {
    pub heading: &'static str,
    pub before: &'static str,
    pub after: &'static str,
    pub details: &'static str,
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
    /// Finding someone in the access log (Art. 11 DSGVO).
    pub access_log: &'static str,
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
    privacy_lead: "Welche Daten beim Besuch von Betula anfallen, wozu und wie lange.",
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

    summary: Prose {
        heading: "Kurz gesagt",
        texts: &[
            "Kein Konto, keine Werbung, kein Tracking: Betula nutzt keine Analysedienste und setzt keine Cookies, die dich wiedererkennen. Schrift, Symbole und Skripte liefert betula.app selbst aus; von fremden Servern wird nichts geladen.",
            "Merkliste, Stundenplan, „Mein Studium“ und „Mein Studiengang“ liegen nur in deinem Browser.",
            "Der Server protokolliert jeden Aufruf mit IP-Adresse, um Betula zu betreiben und vor Missbrauch zu schützen, und löscht die Einträge nach 7 Tagen.",
            "Betula zeigt die Namen von Lehrenden, die die BTU in ihren Modulbeschreibungen veröffentlicht.",
        ],
    },
    controller: Controller {
        heading: "Verantwortlich",
        text: "Verantwortlich für die Verarbeitung personenbezogener Daten auf betula.app im Sinne der Datenschutz-Grundverordnung (DSGVO) ist:",
    },
    hosting: Prose {
        heading: "Wo Betula läuft",
        texts: &[
            "Betula läuft auf einem gemieteten Server der Contabo GmbH, Welfenstraße 22, 81541 München, in einem Rechenzentrum in Deutschland. Dort wird alles verarbeitet, was beim Besuch anfällt, auch die Protokolle und ihre Auswertung; andere Dienste sind daran nicht beteiligt.",
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
    },
    browser: Browser {
        heading: "Speicher im Browser",
        intro: "Vieles, was du in Betula tust, passiert direkt in deinem Browser. Damit das geht, speichert Betula auf deinem Gerät, im Speicher des Browsers (Local Storage, Session Storage, IndexedDB und Cache Storage):",
        kept: &[
            "deine Merkliste,",
            "deinen Stundenplan mit geplanten Modulen, Platzhaltern, Ausgeblendetem und ausgewählten Terminen, deine gespeicherten Pläne und die Adressen deiner Kalender-Abos,",
            "„Mein Studiengang“ mit Studienrichtung, Studienbeginn, Urlaubssemestern und Standort,",
            "„Mein Studium“ mit den Modulen, die du als bestanden abhakst, und den Semestern, in die du Module legst,",
            "Einstellungen, etwa die Sprache (beim ersten Besuch die deines Browsers), hell oder dunkel, die Breite der Seitenleisten, die Ansicht der Semester in „Mein Studium“ und was „Passt in meinen Stundenplan“ vergleicht,",
            "für die laufende Sitzung, wo du in jedem Bereich zuletzt warst und wie weit du gescrollt hast,",
            "eine Kopie des Katalogs, das Sprachmodell der Suche (sie läuft ganz in deinem Browser), die Dateien der App und bis zu 60 zuletzt besuchte Seiten, damit Betula schnell startet und auch ohne Netz funktioniert.",
        ],
        texts: &[
            "Das alles liegt nur auf deinem Gerät; ich kann es nicht einsehen, und ein Konto gibt es nicht. Ein anderes Gerät hat seine eigenen Daten. Der Link, mit dem du deine Merkliste auf ein anderes Gerät bringst, trägt sie hinter dem „#“, und diesen Teil einer Adresse sendet kein Browser an einen Server.",
            "Den Server erreicht nur, was in der Adresse einer Seite steht, wenn du sie aufrufst, neu lädst oder teilst. Eine Adresse sagt, was gerade angezeigt wird, aber nie, was auf deiner Merkliste oder in deinem Stundenplan steht oder was du bestanden hast: etwa den Studiengang, nach dem der Katalog gefiltert ist (auch „Mein Studiengang“, wenn du den Katalog darüber öffnest), das Modul, das neben Merkliste, Stundenplan oder „Mein Studium“ offen ist, oder Semester und Platzhalter, für die du Module suchst. Solche Adressen stehen wie alle anderen im Zugriffsprotokoll. Deinen Stundenplan bekommt der Server nur, wenn du ihn als Kalender abonnierst oder einen Link zum Teilen weitergibst und dieser aufgerufen wird.",
            "Die Speicherung ist für die Funktionen nötig, die du nutzt, und braucht deshalb keine Einwilligung (§ 25 Abs. 2 Nr. 2 TDDDG). Löschen kannst du alles jederzeit in den Einstellungen deines Browsers, indem du die Websitedaten von betula.app löschst.",
        ],
    },
    calendar: Calendar {
        heading: "Kalender-Abo",
        intro: "Du kannst deinen Stundenplan als Kalender abonnieren. Dein Kalenderdienst bekommt dafür eine Adresse, die mit betula.app/calendar/ beginnt. Sie enthält als Code das Semester, die geplanten Module, den Studiengang, dessen Modulkürzel die Einträge verwenden, den Standort und was du ausgeblendet oder ausgewählt hast.",
        course: "Dein Kalender, etwa von Apple, Google oder Microsoft, ruft die Adresse regelmäßig ab, je nach Dienst von deinem Gerät oder von den Servern des Anbieters. Betula erstellt den Kalender bei jedem Abruf neu aus dem Code und den aktuellen Terminen und speichert dazu nichts.",
        logs: "Im Zugriffsprotokoll steht jeder Abruf mit der vollständigen Adresse und der IP-Adresse, von der er kommt; wie alles dort wird er nach 7 Tagen gelöscht. Das Protokoll der Anwendung enthält den Code nicht.",
        legal_basis: "Art. 6 Abs. 1 lit. f DSGVO. Mein berechtigtes Interesse ist, dir das Abo anzubieten, das du selbst anlegst.",
        disclosure: "Wählst du „Google Kalender“ oder „Outlook“, öffnet dein Browser die Seite dieses Dienstes und übergibt ihm die Adresse; ab da gilt dessen Datenschutzerklärung. Betula selbst gibt nichts weiter. Wer die Adresse kennt, kann den Kalender ebenfalls abrufen: Gib sie nur an Menschen weiter, denen du deinen Plan zeigen möchtest.",
        ending: "Entferne den Kalender in deinem Kalenderdienst; dann wird die Adresse nicht mehr abgerufen.",
        download: "Eine heruntergeladene .ics-Datei entsteht in deinem Browser und erreicht den Server nicht.",
    },
    share: Share {
        heading: "Stundenplan teilen",
        intro: "Du kannst einen Link zu deinem Stundenplan kopieren und weitergeben. Er beginnt mit betula.app/studyplan?share= und enthält als Code das Semester, die geplanten Module in ihrer Reihenfolge und den Studiengang, dessen Modulkürzel verwendet werden; was du ausgeblendet oder ausgewählt hast, enthält er nicht.",
        course: "Wer den Link öffnet, sieht die Module und kann sie in den eigenen Stundenplan übernehmen. Schickst du ihn über einen Messenger oder ein soziales Netzwerk, ruft dieses, je nach Dienst von deinem Gerät oder von den Servern des Anbieters, die Seite und ihr Vorschaubild ab, auf dem die Module stehen. Betula erstellt Seite und Bild bei jedem Abruf aus dem Code und den aktuellen Daten und speichert dazu nichts.",
        logs: "Im Zugriffsprotokoll steht jeder Abruf mit der vollständigen Adresse und der IP-Adresse, von der er kommt; wie alles dort wird er nach 7 Tagen gelöscht. Das Protokoll der Anwendung enthält den Code nicht.",
        legal_basis: "Art. 6 Abs. 1 lit. f DSGVO. Mein berechtigtes Interesse ist, dir das Teilen anzubieten, das du selbst auslöst.",
        disclosure: "Betula selbst gibt nichts weiter. Wer den Link kennt, sieht die Module: Gib ihn nur an Menschen weiter, denen du deinen Plan zeigen möchtest.",
        ending: "Ein weitergegebener Link lässt sich nicht zurückholen. Er zeigt aber nur, was beim Kopieren geplant war; was du danach änderst, erreicht ihn nicht.",
    },
    cookies: Cookies {
        heading: "Cookies",
        before: "Betula setzt keine Cookies, mit einer Ausnahme: Solange Betula nur mit Passwort erreichbar ist, etwa während eines geschlossenen Tests, setzt der Server nach der Eingabe des richtigen Passworts ein Cookie namens ",
        after: ".",
        details: "Es enthält nur, wann dein Zugang endet, und eine Signatur, an der der Server erkennt, dass er es selbst ausgestellt hat; gespeichert wird dazu nichts. Es gilt 90 Tage, wird nur über verschlüsselte Verbindungen an Betula gesendet und ist für Skripte nicht lesbar. Weil es für den Zugang, den du angefragt hast, unbedingt nötig ist, braucht es keine Einwilligung (§ 25 Abs. 2 Nr. 2 TDDDG).",
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
            "Deine Daten verkaufe ich nicht und gebe sie nicht weiter. Andere bekommen sie nur, wie oben beschrieben: Contabo als Betreiber des Servers, mein E-Mail-Anbieter, wenn du mir schreibst, und dein Kalenderdienst, wenn du ein Abo anlegst. Betula selbst übermittelt keine Daten in Länder außerhalb der EU; für einen Kalenderdienst, den du wählst, gilt dessen Datenschutzerklärung. Es gibt keine automatisierten Entscheidungen und keine Profile.",
            "Du musst keine Daten angeben, um Betula zu nutzen. Ohne die Angaben, die dein Browser bei jedem Aufruf mitschickt, etwa die IP-Adresse, lassen sich die Seiten aber nicht ausliefern.",
            "Betula verlinkt auf die Originale bei der BTU und auf andere Seiten. Folgst du einem solchen Link, erfährt die andere Seite nur, dass du von betula.app kommst, nicht von welcher Seite; ab da gilt ihre Datenschutzerklärung.",
        ],
    },
    rights: Rights {
        heading: "Deine Rechte",
        before: "Du hast das Recht auf Auskunft über deine Daten (Art. 15 DSGVO), auf Berichtigung (Art. 16), Löschung (Art. 17) und Einschränkung der Verarbeitung (Art. 18). Verarbeitungen, die auf Art. 6 Abs. 1 lit. f DSGVO beruhen – das sind alle hier beschriebenen –, kannst du aus Gründen, die sich aus deiner besonderen Situation ergeben, widersprechen (Art. 21). Eine formlose E-Mail an ",
        after: " genügt.",
        access_log: "Weil Betula keine Konten kennt, kann ich Einträge im Zugriffsprotokoll nur finden, wenn du mir sagst, von welcher IP-Adresse und wann du Betula aufgerufen hast (Art. 11 DSGVO).",
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
    privacy_lead: "What data arises when you visit Betula, what for and for how long.",
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
        source: "DataClient",
        recipients: "Recipients",
        objection: "Objection",
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

    summary: Prose {
        heading: "In short",
        texts: &[
            "No account, no advertising, no tracking: Betula uses no analytics services and sets no cookies that recognise you. betula.app serves its fonts, icons and scripts itself; nothing is loaded from other servers.",
            "Your saved modules, your timetable, “My studies” and “My programme” are kept only in your browser.",
            "The server logs every request with its IP address, to run Betula and protect it from abuse, and deletes the entries after 7 days.",
            "Betula shows the names of lecturers that BTU publishes in its module descriptions.",
        ],
    },
    controller: Controller {
        heading: "Controller",
        text: "The controller for the processing of personal data on betula.app within the meaning of the General Data Protection Regulation (GDPR; in German, DSGVO) is:",
    },
    hosting: Prose {
        heading: "Where Betula runs",
        texts: &[
            "Betula runs on a rented server of Contabo GmbH, Welfenstraße 22, 81541 München, in a data centre in Germany. Everything that arises during a visit is processed there, including the logs and their analysis; no other services are involved.",
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
    },
    browser: Browser {
        heading: "Storage in your browser",
        intro: "Much of what you do in Betula happens directly in your browser. For this to work, Betula stores on your device, in the browser's storage (Local Storage, Session Storage, IndexedDB and Cache Storage):",
        kept: &[
            "your saved modules,",
            "your timetable with planned modules, placeholders, what you have hidden and the dates you have selected, your saved plans and the addresses of your calendar subscriptions,",
            "“My programme” with study track, start of studies, semesters of leave and location,",
            "“My studies” with the modules you tick off as passed and the semesters you put modules into,",
            "settings, such as the language (on your first visit, your browser's), light or dark, the width of the sidebars, the view of the semesters in “My studies” and what “Fits my timetable” compares,",
            "for the current browser session, where you last were in each area and how far you had scrolled,",
            "a copy of the catalogue, the language model of the search (it runs entirely in your browser), the app's files and up to 60 recently visited pages, so that Betula starts quickly and also works offline.",
        ],
        texts: &[
            "All of this is kept only on your device; I cannot see it, and there are no accounts. Another device has its own data. The link that takes your saved modules to another device carries them after the “#”, and no browser sends that part of an address to a server.",
            "Only what is in the address of a page reaches the server, when you open, reload or share it. An address says what is being shown, but never what is among your saved modules or in your timetable, or what you have passed: for example the degree programme the catalogue is filtered by (also “My programme”, if you open the catalogue through it), the module open beside your saved modules, your timetable or “My studies”, or the semester and placeholder you are looking for modules for. Such addresses are in the access log like all others. The server only gets your timetable if you subscribe to it as a calendar, or pass on a link to share it and that link is opened.",
            "Storing this is necessary for the functions you use and therefore requires no consent (§ 25(2) no. 2 TDDDG). You can delete all of it at any time in your browser's settings by deleting the site data of betula.app.",
        ],
    },
    calendar: Calendar {
        heading: "Calendar subscription",
        intro: "You can subscribe to your timetable as a calendar. For this, your calendar service gets an address that begins with betula.app/calendar/. As a code, it contains the semester, the planned modules, the degree programme whose module abbreviations the entries use, the location and what you have hidden or selected.",
        course: "Your calendar, for example from Apple, Google or Microsoft, fetches the address regularly, depending on the service from your device or from the provider's servers. Betula creates the calendar anew from the code and the current dates on every request and stores nothing for it.",
        logs: "The access log records every request with the full address and the IP address it comes from; like everything there, it is deleted after 7 days. The application's log does not contain the code.",
        legal_basis: "Art. 6(1)(f) GDPR (DSGVO). My legitimate interest is offering you the subscription you set up yourself.",
        disclosure: "If you choose “Google Calendar” or “Outlook”, your browser opens that service's page and hands it the address; from then on, its privacy policy applies. Betula itself passes nothing on. Anyone who knows the address can fetch the calendar too: only give it to people you want to show your plan to.",
        ending: "Remove the calendar in your calendar service; then the address is no longer fetched.",
        download: "A downloaded .ics file is created in your browser and does not reach the server.",
    },
    share: Share {
        heading: "Sharing your timetable",
        intro: "You can copy a link to your timetable and pass it on. It begins with betula.app/studyplan?share= and contains, as a code, the semester, the planned modules in their order and the degree programme whose module abbreviations are used; it does not contain what you have hidden or selected.",
        course: "Whoever opens the link sees the modules and can add them to their own timetable. If you send it via a messenger or a social network, that service fetches the page and its preview image, which shows the modules, depending on the service from your device or from the provider's servers. Betula creates the page and the image from the code and the current data on every request and stores nothing for it.",
        logs: "The access log records every request with the full address and the IP address it comes from; like everything there, it is deleted after 7 days. The application's log does not contain the code.",
        legal_basis: "Art. 6(1)(f) GDPR (DSGVO). My legitimate interest is offering you the sharing you start yourself.",
        disclosure: "Betula itself passes nothing on. Anyone who knows the link sees the modules: only give it to people you want to show your plan to.",
        ending: "A link that has been passed on cannot be taken back. But it only shows what was planned when it was copied; what you change afterwards does not reach it.",
    },
    cookies: Cookies {
        heading: "Cookies",
        before: "Betula sets no cookies, with one exception: while Betula can only be reached with a password, for example during a closed test, the server sets a cookie named ",
        after: " once the correct password has been entered.",
        details: "It contains only when your access ends, and a signature by which the server recognises that it issued the cookie itself; nothing is stored for it. It is valid for 90 days, is sent to Betula only over encrypted connections and cannot be read by scripts. Because it is strictly necessary for the access you asked for, it requires no consent (§ 25(2) no. 2 TDDDG).",
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
            "I do not sell your data and do not pass it on. Others only receive it as described above: Contabo as the operator of the server, my e-mail provider when you write to me, and your calendar service when you set up a subscription. Betula itself transfers no data to countries outside the EU; for a calendar service you choose, its privacy policy applies. There are no automated decisions and no profiles.",
            "You do not have to provide any data to use Betula. Without the information your browser sends with every request, such as the IP address, the pages cannot be delivered, though.",
            "Betula links to the originals at BTU and to other pages. If you follow such a link, the other site only learns that you come from betula.app, not from which page; from then on, its privacy policy applies.",
        ],
    },
    rights: Rights {
        heading: "Your rights",
        before: "Under the GDPR (DSGVO), you have the right of access to your data (Art. 15), to rectification (Art. 16), to erasure (Art. 17) and to restriction of processing (Art. 18). You can object to processing based on Art. 6(1)(f) GDPR (DSGVO) – which is all the processing described here – on grounds relating to your particular situation (Art. 21). An informal e-mail to ",
        after: " is enough.",
        access_log: "Because Betula has no accounts, I can only find entries in the access log if you tell me from which IP address and when you visited Betula; see Art. 11 GDPR (DSGVO).",
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
