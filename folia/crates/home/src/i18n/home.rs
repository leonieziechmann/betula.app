//! Texts of „Über Betula" (`home/about.rs` until 2026-10-27), what the start page said until
//! concept A, in the order of that page: what search engines read, the first panel, the pictures
//! and the map, the ways into the catalog, what Betula does, and the questions. Since 2026-10-27
//! they stand on the pages of the topics (`home/topics.rs`): the first panel, the pictures and
//! what Betula does on the overview (`home/overview.rs`), the ways into the catalog on „Module
//! finden", the faculties on „Studium planen", the questions on a page of their own. The chapters
//! of the topics have a group of their own (`home_detail`), and so have the topics (`topics`),
//! with the heading and the line under it of the overview's first panel. The start page's own
//! words are `today` and `setup`; its description and title for search engines are still these
//! (`description`, `seo_title`), and so are the ways into the catalog it shows under „Entdecken".
//!
//! Everything here says what the app really does, and says it plainly (owner, 2026-09-28: the page
//! is what search engines and the assistants built on them know of Betula; comparing it with other
//! tools they took the missing account for missing functions, and did not see that it works
//! without JavaScript or that the study plans are read from the regulations). Where a text names a
//! button or a filter, it names it as the page labels it; the tests below and `folia_app::quotes`
//! hold them together.
//!
//! Since 2026-11-18 it says what Betula is by now (owner: the pages „sind alle veraltet und könnten
//! etwas interessanter werden") — the day on the start page, „Mein Studium", the Merkliste's
//! collections and notes, profiles, Betula Sync and sharing; the keys as they are („/" the search,
//! Strg+K the commands) — and it speaks of no account: there is none, a profile kept on the
//! device is what there is (owner, the same day: „wir reden nicht von konten … sondern von
//! profilen und lokalen Daten"; the test below holds every text here to it).

pub struct Texts {
    /// The page's description for search engines and link previews.
    pub description: &'static str,
    /// Its title for link previews (the tab shows the site's own title, `app::default_title`).
    pub seo_title: &'static str,
    /// Other names of the site, as the structured data gives them („Betula Modulkatalog").
    pub alternate_names: [&'static str; 2],
    /// What it takes to run Betula, as the structured data of the app says it
    /// (`browserRequirements`).
    pub browser_requirements: &'static str,

    // The first panel
    /// „Inoffiziell · für die BTU Cottbus-Senftenberg", over the heading (which is the
    /// overview's, `topics::Topic`).
    pub eyebrow: &'static str,
    pub browse_modules: &'static str,
    /// „Studiengang wählen": a button, the action of a picture, and what an answer quotes.
    pub choose_program: &'static str,
    /// The figures beside the text: modules, programs, faculties, and the Termine of the current
    /// semester („im WiSe 2026/27" under them; the name of the semester is written already).
    pub figure_modules: &'static str,
    pub figure_programs: &'static str,
    pub figure_faculties: &'static str,
    pub figure_dates: &'static str,
    pub in_semester: fn(&str) -> String,

    // The pictures
    /// What the pictures are, for screen readers: „Karussell", „Bild" (`aria-roledescription`).
    pub carousel: &'static str,
    pub slide: &'static str,
    /// The name of the pictures together, and of their frame, which the arrow keys turn.
    pub carousel_label: &'static str,
    pub carousel_keys: &'static str,
    /// „2 von 4: Der Katalog".
    pub slide_of: fn(usize, usize, &str) -> String,
    pub previous: &'static str,
    pub next: &'static str,
    /// The tabs under the pictures.
    pub tabs: &'static str,
    /// The button that stops and starts the pictures: its name and its title.
    pub pause_pictures: &'static str,
    pub play_pictures: &'static str,
    pub pause: &'static str,
    pub play: &'static str,
    /// Each picture: its tab, its title and what a click on it does, and the line under it …
    pub map_slide: Slide,
    /// … „62 Studiengänge, verbunden durch gemeinsame Module" …
    pub map_text: fn(usize) -> String,
    pub catalog_slide: Slide,
    /// … „Alle 1.204 Module, gefiltert während du tippst" (the number is written already), or
    /// without the number …
    pub catalog_text: fn(&str) -> String,
    pub catalog_text_plain: &'static str,
    /// … and what the screenshot shows, for those who cannot see it.
    pub catalog_alt: &'static str,
    /// The start page of a study set up, „Mein Studium" and the Stundenplan, each a screenshot of
    /// an example study (`folia/e2e/showcase-shots.mjs`).
    pub day_slide: Slide,
    pub day_text: &'static str,
    pub day_alt: &'static str,
    pub study_slide: Slide,
    pub study_text: &'static str,
    pub study_alt: &'static str,
    pub timetable_slide: Slide,
    pub timetable_text: &'static str,
    pub timetable_alt: &'static str,

    // The map
    /// The dialog the map opens large in: heading, the line under it, its close button.
    pub map_heading: &'static str,
    pub map_hint: &'static str,
    pub close_map: &'static str,
    /// The key of the map.
    pub legend: &'static str,
    pub bachelor: &'static str,
    pub master: &'static str,
    pub other: &'static str,
    pub shared_modules: &'static str,
    /// The line under the map while nothing is picked: „62 Studiengänge", what the lines are,
    /// and the link to all programs.
    pub program_count: fn(usize) -> String,
    pub lines_show: &'static str,
    pub all_programs: &'static str,
    /// „17 Module · Fakultät 1": a program's modules, and its faculty.
    pub program_modules: fn(usize) -> String,
    pub faculty: fn(&str) -> String,
    /// „teilt 8 mit Informatik, 5 mit Mathematik": the closest relatives, joined by „, ".
    pub shares: fn(&str) -> String,
    pub shared_with: fn(usize, &str) -> String,
    /// Over the list of the closest relatives, beside the map.
    pub shared_modules_with: &'static str,
    pub to_program: &'static str,
    pub clear_pick: &'static str,

    // The ways into the catalog, and the programs by faculty
    pub ways_in: &'static str,
    /// „Alle 1.204 Module", „Alle 62 Studiengänge"; the number is written already.
    pub all_modules_count: fn(&str) -> String,
    pub programs_by_faculty: &'static str,
    pub all_programs_count: fn(&str) -> String,
    /// The programs of no faculty, or of more than one.
    pub no_faculty: &'static str,
    /// The ways in: a filter people come for, and what it lists.
    pub winter: Entry,
    pub summer: Entry,
    pub english: Entry,
    pub fues: Entry,
    pub no_written_exam: Entry,
    pub oral_exam: Entry,
    pub senftenberg: Entry,
    pub ungraded: Entry,

    // What Betula does
    /// „Was Betula kann" and „Fragen und Antworten": the headings of their sections, and the
    /// names of the ways to them.
    pub abilities: &'static str,
    pub questions: &'static str,
    pub abilities_lead: &'static str,
    pub search: Ability,
    pub filters: Ability,
    pub plans: Ability,
    pub study: Ability,
    pub timetable: Ability,
    pub dates: Ability,
    pub day: Ability,
    pub marks: Ability,
    pub prerequisites: Ability,
    pub profile: Ability,
    pub offline: Ability,
    pub gaps: Ability,
    /// „In Arbeit", and what is.
    pub in_progress: &'static str,
    pub coming: &'static str,

    // The questions
    pub about_betula: Faq,
    pub using_betula: Faq,
    pub for_studies: Faq,

}

/// A picture of the carousel: its tab, its title, and what a click on it does.
pub struct Slide {
    pub tab: &'static str,
    pub title: &'static str,
    pub action: &'static str,
}

/// A way into the catalog: its name, and what it lists.
pub struct Entry {
    pub label: &'static str,
    pub hint: &'static str,
}

/// Something Betula does: in a few words, and in a sentence or two.
pub struct Ability {
    pub title: &'static str,
    pub text: &'static str,
}

/// A group of questions: its name, and each question with its answer. The answers only say what
/// the app really does; who runs it is the imprint's business.
pub struct Faq {
    pub name: &'static str,
    pub questions: &'static [(&'static str, &'static str)],
}

pub const DE: Texts = Texts {
    description: "Alle Module und Studiengänge der BTU Cottbus-Senftenberg an einem Ort: durchsuchen und filtern, Regelstudienpläne aus den Prüfungsordnungen, dein Studium Semester für Semester, dein Stundenplan mit Kalender-Abo. Kostenlos, auf all deinen Geräten, auch offline. Inoffiziell, mit Link zum Original.",
    seo_title: "Betula · Modulkatalog für die BTU Cottbus-Senftenberg",
    alternate_names: ["Betula Modulkatalog", "Modulkatalog BTU Cottbus-Senftenberg (inoffiziell)"],
    browser_requirements: "Läuft in jedem aktuellen Browser, auch ohne JavaScript. Mit JavaScript lädt die App den Katalog als SQLite-Datenbank in den Browser (WebAssembly), antwortet ohne Ladezeiten und funktioniert offline.",

    eyebrow: "Inoffiziell · für die BTU Cottbus-Senftenberg",
    browse_modules: "Module durchsuchen",
    choose_program: "Studiengang wählen",
    figure_modules: "Module",
    figure_programs: "Studiengänge",
    figure_faculties: "Fakultäten",
    figure_dates: "Termine",
    in_semester: |semester| format!("im {semester}"),

    carousel: "Karussell",
    slide: "Bild",
    carousel_label: "Betula im Bild",
    carousel_keys: "Bilder, mit den Pfeiltasten zu wechseln",
    slide_of: |n, count, title| format!("{n} von {count}: {title}"),
    previous: "Vorheriges Bild",
    next: "Nächstes Bild",
    tabs: "Bilder",
    pause_pictures: "Bilder anhalten",
    play_pictures: "Bilder abspielen",
    pause: "Anhalten",
    play: "Abspielen",
    map_slide: Slide { tab: "Karte", title: "Die Karte", action: "Groß ansehen" },
    map_text: |n| format!("{n} Studiengänge, verbunden durch gemeinsame Module"),
    catalog_slide: Slide { tab: "Katalog", title: "Der Katalog", action: "Zum Katalog" },
    catalog_text: |n| format!("Alle {n} Module, gefiltert während du tippst"),
    catalog_text_plain: "Alle Module, gefiltert während du tippst",
    catalog_alt: "Der Modulkatalog: die Suche „datenbank“ mit sechs Treffern, rechts das Modul Datenbanken mit seinem Wochenplan",
    day_slide: Slide { tab: "Dein Tag", title: "Dein Tag", action: "Zur Startseite" },
    day_text: "Was gerade läuft, was als Nächstes kommt, wann deine Prüfungen sind",
    day_alt: "Die Startseite mit eingerichtetem Studium: die Begrüßung mit dem nächsten Termin, darunter der Tag, die Woche, die Prüfungen und der Stand des Studiums",
    study_slide: Slide { tab: "Studium", title: "Mein Studium", action: "Zu „Mein Studium“" },
    study_text: "Bestanden, geplant, offen: Semester für Semester",
    study_alt: "„Mein Studium“ für Informatik B.Sc.: die bestandenen Leistungspunkte, die Bereiche als Balken, darunter die Module des laufenden Semesters",
    timetable_slide: Slide { tab: "Stundenplan", title: "Der Stundenplan", action: "Zum Stundenplan" },
    timetable_text: "Dein Semester als Woche, mit jeder Überschneidung",
    timetable_alt: "Der Stundenplan eines Semesters als Woche: Vorlesungen, Übungen und ein Praktikum, eine Überschneidung markiert, eine Übungsgruppe noch zu wählen",

    map_heading: "Wie die Studiengänge zusammenhängen",
    map_hint: "Jeder Punkt ist ein Studiengang; eine Linie verbindet zwei, deren Curricula sich Module teilen. Ein Klick auf einen Punkt zeigt, wohin er gehört.",
    close_map: "Karte schließen",
    legend: "Legende",
    bachelor: "Bachelor",
    master: "Master",
    other: "Weitere",
    shared_modules: "Gemeinsame Module",
    program_count: |n| format!("{n} Studiengänge"),
    lines_show: "Linien zeigen gemeinsame Module",
    all_programs: "Alle Studiengänge",
    program_modules: |n| format!("{n} Module"),
    faculty: |code| format!("Fakultät {code}"),
    shares: |relatives| format!("teilt {relatives}"),
    shared_with: |n, program| format!("{n} mit {program}"),
    shared_modules_with: "Gemeinsame Module mit",
    to_program: "Zum Studiengang",
    clear_pick: "Auswahl aufheben",

    ways_in: "Einstiege in den Katalog",
    all_modules_count: |n| format!("Alle {n} Module"),
    programs_by_faculty: "Studiengänge nach Fakultät",
    all_programs_count: |n| format!("Alle {n} Studiengänge"),
    no_faculty: "Fakultätsübergreifend oder nicht eindeutig zuzuordnen",
    winter: Entry { label: "Im Wintersemester", hint: "Module mit Turnus Wintersemester" },
    summer: Entry { label: "Im Sommersemester", hint: "Module mit Turnus Sommersemester" },
    english: Entry { label: "Auf Englisch", hint: "Module, die auf Englisch gelehrt werden" },
    fues: Entry { label: "Fachübergreifendes Studium", hint: "FÜS-Module aller Fakultäten" },
    no_written_exam: Entry { label: "Ohne Klausur", hint: "Module, deren Prüfung keine Klausur nennt" },
    oral_exam: Entry { label: "Mit mündlicher Prüfung", hint: "Module mit mündlicher Prüfung" },
    senftenberg: Entry { label: "In Senftenberg", hint: "Module am Campus Senftenberg" },
    ungraded: Entry { label: "Unbenotet", hint: "Module, die ohne Note abgeschlossen werden" },

    abilities: "Was Betula kann",
    questions: "Fragen und Antworten",
    abilities_lead: "Dieselben Daten wie bei der BTU, so aufbereitet, dass man mit ihnen planen kann.",
    search: Ability {
        title: "Ein Suchfeld für alles",
        text: "Titel auf Deutsch und Englisch, Modulnummer, Kürzel und Lehrende in einer Suche, auf jeder Seite, mit der Taste /. Die Treffer stehen da, während du tippst, und darunter „Ähnliche Module“ zum selben Thema. Strg+K öffnet die Befehle für alles andere.",
    },
    filters: Ability {
        title: "Filter, die zusammenpassen",
        text: "Studiengang, Bereich, Fachsemester, Turnus, Lehrform, Prüfung, Leistungspunkte, Sprache, Standort, Lehrende und mehr, beliebig kombiniert. Fast jeder lässt sich umkehren („alles außer Klausur“), und jede Auswahl ist ein Link.",
    },
    plans: Ability {
        title: "Regelstudienpläne aus den Prüfungsordnungen",
        text: "Aus den PDFs der Prüfungs- und Studienordnungen gelesen und gegen deren eigene Summen geprüft: Semester für Semester, mit Pflicht, Wahlpflicht und jeder Studienrichtung.",
    },
    study: Ability {
        title: "Dein Studium, Semester für Semester",
        text: "„Mein Studium“ zeigt, was bestanden, geplant und offen ist, mit den Leistungspunkten jedes Bereichs. Beim Einplanen siehst du, ob ein Modul in dem Semester angeboten wird und ob seine Voraussetzungen davor liegen.",
    },
    timetable: Ability {
        title: "Ein Stundenplan, der mitdenkt",
        text: "Ein Fachsemester übernehmen oder Module einzeln einplanen: Betula zeigt die Woche, findet Überschneidungen und zu knappe Prüfungen und sucht Module, die noch hineinpassen. Mit eigenen Terminen und Notizen, als .ics-Datei oder Kalender-Abo.",
    },
    dates: Ability {
        title: "Termine aus dem Vorlesungsverzeichnis",
        text: "Vorlesungen, Übungen und Prüfungen als Wochenplan beim Modul, mit Räumen, A- und B-Wochen. Platzhalter aus QIS wie eine Prüfung um 01:00 Uhr erscheinen als „Zeit offen“ statt als falsche Uhrzeit.",
    },
    day: Ability {
        title: "Dein Tag auf der Startseite",
        text: "Was gerade läuft, was als Nächstes kommt und in welchem Raum: Die Startseite zeigt deinen Tag, die Woche, die nächsten Prüfungen und wie weit dein Studium ist.",
    },
    marks: Ability {
        title: "Merkliste mit Sammlungen und Notizen",
        text: "Ein Klick aufs Lesezeichen merkt ein Modul. Sammlungen ordnen, was zusammengehört, eine Notiz hält fest, warum, und die Liste zeigt, was davon in deinem Studium zählt.",
    },
    prerequisites: Ability {
        title: "Voraussetzungen zum Anklicken",
        text: "Was ein Modul zwingend oder empfohlen voraussetzt, führt als Link direkt zu diesem Modul, mit dem Wortlaut der Modulbeschreibung. Ein abgelöstes Modul nennt sein Nachfolgemodul.",
    },
    profile: Ability {
        title: "Deine Daten bleiben bei dir",
        text: "Was du merkst und planst, liegt in deinem Profil auf deinem Gerät. Mit Betula Sync haben es alle deine Geräte, verschlüsselt mit einem Schlüssel, den nur sie kennen. Ohne Werbung, ohne Tracking.",
    },
    offline: Ability {
        title: "Schnell, offline, auch ohne JavaScript",
        text: "Mit JavaScript liegt der Katalog als Datenbank in deinem Browser: Suche und Filter antworten sofort, auch ohne Netz, und Betula lässt sich wie eine App installieren. Ohne JavaScript ist jede Seite vollständiges HTML.",
    },
    gaps: Ability {
        title: "Ehrlich bei Lücken",
        text: "Wo die Quelle nichts sagt, steht „nicht angegeben“ und keine Vermutung, und was Betula ableitet, sagt es dazu. Jedes Modul verlinkt auf sein Original bei der BTU.",
    },
    in_progress: "In Arbeit",
    coming: "Voraussetzungen prüfen: was du mit dem schon Bestandenen belegen kannst",

    about_betula: Faq {
        name: "Über Betula",
        questions: &[
            (
                "Ist Betula ein Angebot der BTU?",
                "Nein. Betula ist ein inoffizielles, unabhängiges Projekt und gehört nicht zur BTU Cottbus-Senftenberg. Verbindlich sind allein die Modulbeschreibungen, Prüfungs- und Studienordnungen der Universität; jede Seite hier verlinkt deshalb auf ihr Original.",
            ),
            (
                "Wer macht Betula?",
                "Betula ist ein privates, unabhängiges Projekt; die BTU hat es weder beauftragt noch geprüft. Wer dahintersteht und wie du Kontakt aufnimmst, steht im Impressum. Hinweise auf Fehler sind willkommen.",
            ),
            (
                "Warum heißt es Betula?",
                "Betula ist der lateinische Name der Birke. Sie ist eine Pionierpflanze und wächst als eine der ersten auf den Flächen, die der Bergbau in der Lausitz hinterlassen hat. Und in BeTUla steckt die BTU.",
            ),
            (
                "Wie funktioniert Betula?",
                "Betula ist nach der Birke benannt, und seine Teile sind es auch. Radix, die Wurzel, liest die öffentlichen Seiten der BTU und die PDFs ihrer Prüfungsordnungen ein, prüft das Ergebnis und veröffentlicht es als Datenstand: Module, Studiengänge mit Regelstudienplänen und Bereichen, die Termine des Vorlesungsverzeichnisses. Folia, die Blätter, ist die Web-App, die du gerade siehst. Ihr Server liefert jede Seite als fertiges HTML; mit JavaScript lädt dein Browser den Datenstand einmal als Datenbank und beantwortet danach Suche und Filter selbst. Und Phloem, das Leitgewebe unter der Rinde, reicht mit Betula Sync die verschlüsselten Änderungen zwischen deinen Geräten weiter. Welche Versionen gerade laufen, steht am Ende dieser Seite und auf der Profilseite.",
            ),
            (
                "Woher kommen die Daten?",
                "Aus öffentlichen Quellen der BTU: den Modulbeschreibungen, dem Vorlesungsverzeichnis und dem Modulbaum der Studiengänge im Portal QIS, der Liste des Fachübergreifenden Studiums und den Prüfungs- und Studienordnungen, die die BTU als PDF veröffentlicht. Betula ordnet die Angaben und macht sie durchsuchbar; am Inhalt wird nichts geändert und nichts ergänzt. Wo die Quelle nichts sagt, steht „nicht angegeben“.",
            ),
            (
                "Wie aktuell ist der Katalog?",
                "Radix liest die Quellen laufend nach, in kleinen Schritten und das meiste nachts: Termine, die noch nicht feststehen, alle zwei Stunden, das Vorlesungsverzeichnis jede Nacht, Modulbeschreibungen und Studiengänge mindestens einmal im Monat. Ein neuer Datenstand erscheint, sobald sich etwas geändert und er alle Prüfungen bestanden hat; wann das zuletzt war, steht am Ende dieser Seite und auf der Profilseite. Kurzfristige Änderungen stehen zuerst bei der BTU, und im Zweifel gilt das Original.",
            ),
            (
                "Kostet Betula etwas?",
                "Nein. Betula ist kostenlos, ohne Werbung und ohne Tracking: kein Analysedienst, keine Cookies, die dich wiedererkennen, und nichts wird von fremden Servern geladen. Es ist ein privates, nicht-kommerzielles Projekt.",
            ),
            (
                "Wo liegen meine Daten?",
                "In deinem Profil, und das liegt lokal auf deinem Gerät, im Speicher deines Browsers: deine Merkliste mit Sammlungen und Notizen, dein Stundenplan, „Mein Studium“ und „Mein Studiengang“. Betula fragt weder nach deinem Namen noch nach einer E-Mail-Adresse, und der Server bekommt nichts davon. Erst wenn du Betula Sync einschaltest, liegt eine Kopie auf dem Server, verschlüsselt mit einem Schlüssel, den nur deine Geräte haben. Die Einzelheiten stehen in der Datenschutzerklärung.",
            ),
        ],
    },
    using_betula: Faq {
        name: "Betula nutzen",
        questions: &[
            (
                "Wie finde ich schnell das richtige Modul?",
                "Tippe oben in die Suche einen Titel, eine Modulnummer, ein Kürzel oder einen Namen aus der Lehre (die Taste / springt hinein); unter den Treffern stehen „Ähnliche Module“, die zum selben Thema passen. Oder fang mit einem Filter an: Studiengang, Bereich, Fachsemester, Turnus, Prüfungsform, Leistungspunkte, Sprache, Standort, Lehrende und mehr. Alle Filter lassen sich kombinieren und fast alle umkehren, etwa zu „alles außer Klausur“. Jede Auswahl steht in der Adresse, lässt sich also teilen, als Lesezeichen speichern und mit „Zurück“ zurücknehmen. Jeden Filter einzeln erklärt die Seite „Module finden“, zu der auf der Startseite „Mehr über Betula“ führt.",
            ),
            (
                "Was zeigt die Startseite?",
                "Sobald dein Studium eingerichtet ist, deinen Tag: was gerade läuft, was als Nächstes kommt und in welchem Raum, dazu die Woche, die nächsten Prüfungen, den Stand deines Studiums und deine Merkliste. Eingerichtet ist es in drei Klicks: Studiengang, Studienbeginn, Fachsemester. Wer sich erst umsehen will, sucht und stöbert ohne Einrichtung.",
            ),
            (
                "Was ist „Mein Studium“?",
                "Dein Plan bis zum Abschluss. Betula füllt die Semester nach dem Regelstudienplan deines Studiengangs, du hakst ab, was bestanden ist, und planst, was kommt. Ein Balken zeigt für jeden Bereich, wie viele Leistungspunkte bestanden, geplant und noch offen sind. Beim Einplanen siehst du, ob ein Modul in dem Semester angeboten wird und ob seine Voraussetzungen davor liegen; Urlaubssemester, Wiederholungen und ein Wechsel des Studiengangs rechnet der Plan mit.",
            ),
            (
                "Was kann der Stundenplan?",
                "Er zeigt ein Semester als Woche. Module kommen mit „Einplanen“ hinein, aus der Merkliste oder als ganzes Fachsemester aus dem Regelstudienplan, dessen Wahlpflichtzeilen zu Platzhaltern werden. Betula zählt Überschneidungen an den Tagen, an denen sich Termine wirklich treffen, mit A- und B-Wochen, bei Übungen mit mehreren Gruppen erst, wenn keine Gruppe mehr frei ist. Es warnt vor Prüfungen, die sich überschneiden oder zu wenig Zeit für den Weg zwischen den Standorten lassen, und „Passt in meinen Stundenplan“ findet Module, die noch hineinpassen. Eigene Termine stehen daneben, und einen Termin aus QIS kannst du verlegen und mit einer Notiz versehen. Mitnehmen lässt sich der Plan als .ics-Datei, als Bild oder als Kalender-Abo, das Änderungen selbst nachlädt.",
            ),
            (
                "Kann ich mir Module merken?",
                "Ja. Jedes Modul hat ein Lesezeichen, die Taste M tut dasselbe. Die Merkliste ordnest du in Sammlungen, zu jedem Modul passt eine Notiz, und sie zeigt, was davon in deinem Studium zählt und schon geplant oder bestanden ist. Sie lässt sich als Text kopieren und dient im Katalog als Filter („Gemerkt“). Sie gehört zu deinem Profil und liegt auf deinem Gerät.",
            ),
            (
                "Kann ich Betula auf mehreren Geräten nutzen?",
                "Ja, mit Betula Sync. Schalte es auf der Seite „Profil“ für ein Profil ein und scanne mit dem anderen Gerät den QR-Code unter „Gerät verbinden“: Von da an haben Handy, Laptop und Tablet denselben Stand, und eine Änderung ist auf den anderen sofort da, auch nach einer Weile ohne Netz. Ohne Sync bringt eine Sicherung als Datei ein Profil auf ein anderes Gerät.",
            ),
            (
                "Wer kann mit Betula Sync meine Daten lesen?",
                "Nur deine Geräte. Dein Browser verschlüsselt jede Änderung, bevor sie das Gerät verlässt, mit einem Schlüssel, den er selbst erzeugt und der nur per QR-Code oder Link von Gerät zu Gerät geht. Der Server bewahrt die versiegelten Änderungen auf und reicht sie weiter; er kennt zufällige Kennungen, Größen und Zeitpunkte, aber weder den Inhalt noch dich: Einen Namen, eine E-Mail-Adresse oder ein Passwort gibt es dafür nicht. „Sync beenden“ löscht die Kopie auf dem Server sofort.",
            ),
            (
                "Was passiert, wenn ich mein Handy verliere oder den Browser leere?",
                "Mit Betula Sync nichts Schlimmes: Jedes andere verbundene Gerät hat das ganze Profil. Sind alle Geräte weg, holt der Wiederherstellungscode es zurück; Betula zeigt ihn dir auf der Seite „Profil“, bewahre ihn auf wie ein Passwort. Ein verlorenes Gerät sperrst du aus, indem du Sync beendest und neu einschaltest. Ohne Sync liegt ein Profil nur in dem einen Browser; dann schützt eine Sicherung als Datei, die du ebenfalls auf der Seite „Profil“ anlegst.",
            ),
            (
                "Kann ich meinen Stundenplan oder meine Merkliste teilen?",
                "Ja, mit „Teilen …“, auf drei Arten. Als fester Stand: ein Link mit einer Kopie von jetzt, den jeder öffnen kann, der ihn hat. So, dass es aktuell bleibt: Andere folgen deinem Kalender oder deiner Liste und sehen jede Änderung, bis du aufhörst zu teilen. Oder als Einmal-Link für genau eine Person, der höchstens 7 Tage gilt. Was aktuell bleibt und der Einmal-Link brauchen Betula Sync, und du wählst dabei, ob eigene Termine und Notizen mitkommen.",
            ),
            (
                "Kann ich für zwei Studiengänge planen?",
                "Ja, mit einem Profil je Studiengang. Jedes Profil hat seine eigene Merkliste, seinen Stundenplan und sein Studium; auf der Seite „Profil“ legst du eines an und wechselst zwischen ihnen. Ein zweites Profil eignet sich auch, um einen anderen Verlauf deines Studiums durchzuspielen.",
            ),
            (
                "Funktioniert Betula ohne JavaScript?",
                "Ja. Jede Seite kommt vom Server als vollständiges HTML: Der Katalog mit allen Filtern, die Modulseiten, Studiengänge und Regelstudienpläne funktionieren ohne JavaScript, als klassische Website, für Textbrowser und Suchmaschinen genauso. Mit JavaScript übernimmt eine App aus Rust und WebAssembly: Sie lädt den Katalog einmal als SQLite-Datenbank in deinen Browser und beantwortet danach jede Suche und jeden Filter selbst, ohne Ladezeiten zwischen den Seiten. Was zu deinem Profil gehört, Merkliste, Stundenplan und „Mein Studium“, braucht JavaScript.",
            ),
            (
                "Funktioniert Betula offline und als App?",
                "Ja. Nach dem ersten Besuch mit JavaScript liegen Katalog und App in deinem Browser, und Betula startet auch ohne Netz. Auf Handy, Tablet und Rechner lässt es sich wie eine App installieren, mit eigenem Symbol auf dem Startbildschirm. Einen neuen Datenstand lädt die App im Hintergrund und nutzt ihn ab dem nächsten Start.",
            ),
            (
                "Gibt es Betula auf Englisch?",
                "Ja. Die Oberfläche gibt es auf Deutsch und Englisch, jede Seite unter ihrer eigenen Adresse (/en). Mit JavaScript öffnet Betula beim ersten Besuch die Sprache deines Browsers und merkt sich deine Wahl. Titel und Beschreibungen der Module stehen so da, wie die BTU sie schreibt; viele Module haben auch einen englischen Titel, und nach Modulen, die auf Englisch gelehrt werden, lässt sich filtern.",
            ),
        ],
    },
    for_studies: Faq {
        name: "Fürs Studium",
        questions: &[
            (
                "Wie finde ich die Module für mein Studium?",
                "Über deinen Studiengang. Welche Module du belegst, legt seine Studien- und Prüfungsordnung fest; Betula bereitet sie auf und bündelt alles an einem Ort. Unter „Studiengang wählen“ findest du den Regelstudienplan Semester für Semester (wo die Ordnung einen enthält), die Wahlpflichtbereiche und alle Module des Studiengangs; „Im Modulkatalog“ öffnet den Katalog darauf eingegrenzt. Ein Klick auf ein Modul zeigt Inhalte, Termine und Prüfung. Als „Mein Studiengang“ gesetzt, ist er in Katalog und Stundenplan gleich voreingestellt.",
            ),
            (
                "Was ist ein Modul, und was steht in einer Modulbeschreibung?",
                "Ein Modul ist eine abgeschlossene Lehreinheit, meist über ein Semester, für die es Leistungspunkte (LP) gibt. Die Modulbeschreibung nennt Inhalte und Lernziele, Lehrformen und Umfang, Voraussetzungen, die Prüfungsleistung, den Turnus und die Verantwortlichen. Zusammen bilden die Beschreibungen das Modulhandbuch eines Studiengangs.",
            ),
            (
                "Was sind Leistungspunkte (LP)?",
                "Leistungspunkte, auch ECTS-Punkte genannt, messen den Arbeitsaufwand eines Moduls: Vorlesung, Übung und Selbststudium zusammen. Ein Punkt steht für etwa 25 bis 30 Stunden. Ein Semester nach Regelstudienplan umfasst in der Regel 30 LP.",
            ),
            (
                "Was bedeuten Pflicht und Wahlpflicht?",
                "Pflichtmodule belegen alle im Studiengang. Bei Wahlpflichtmodulen wählst du aus einem Bereich, den die Ordnung festlegt, bis die geforderten Leistungspunkte erreicht sind. Betula zeigt die Bereiche jedes Studiengangs unter „Wahlpflicht & Bereiche“, und im Katalog grenzt der Filter „Bereich“ die Liste auf einen davon ein.",
            ),
            (
                "Wann wird ein Modul angeboten?",
                "Das sagt der Turnus: im Wintersemester, im Sommersemester oder in jedem Semester, manche Module nur in geraden oder ungeraden Jahren. Im Katalog kannst du nach allem davon filtern. Ob ein Modul im aktuellen Semester wirklich stattfindet, zeigen seine Termine: Sie stehen als Wochenplan beim Modul, und der Filter „Bestätigt“ blendet alle Module ohne veröffentlichte Termine aus.",
            ),
            (
                "Wo melde ich mich für Module und Prüfungen an?",
                "Nicht bei Betula. Anmeldungen laufen über die Systeme der BTU; Fristen und Regeln stehen dort und in deiner Prüfungsordnung. Betula hilft beim Planen und verlinkt jedes Modul auf sein Original.",
            ),
            (
                "Wie sehe ich, wie viele Leistungspunkte mir noch fehlen?",
                "In „Mein Studium“. Dort steht, wie viele Leistungspunkte du bestanden und geplant hast, im Ganzen und für jeden Bereich deines Studiengangs, und was laut Regelstudienplan noch offen ist. Betula rechnet mit dem, was du als bestanden markierst; verbindlich ist dein Notenspiegel bei der BTU.",
            ),
        ],
    },

};

pub const EN: Texts = Texts {
    description: "Every module and degree programme of BTU Cottbus-Senftenberg in one place: search and filter, standard study plans from the examination regulations, your studies semester by semester, your timetable with a calendar subscription. Free, on all your devices, works offline. Unofficial, with a link to the original.",
    seo_title: "Betula · Module catalogue for BTU Cottbus-Senftenberg",
    alternate_names: ["Betula module catalogue", "Module catalogue of BTU Cottbus-Senftenberg (unofficial)"],
    browser_requirements: "Runs in any current browser, also without JavaScript. With JavaScript the app loads the catalogue into the browser as an SQLite database (WebAssembly), answers without loading times and works offline.",

    eyebrow: "Unofficial · for BTU Cottbus-Senftenberg",
    browse_modules: "Browse modules",
    choose_program: "Choose a degree programme",
    figure_modules: "Modules",
    figure_programs: "Degree programmes",
    figure_faculties: "Faculties",
    figure_dates: "Dates",
    in_semester: |semester| format!("in {semester}"),

    carousel: "carousel",
    slide: "slide",
    carousel_label: "Betula in pictures",
    carousel_keys: "Pictures; the arrow keys change them",
    slide_of: |n, count, title| format!("{n} of {count}: {title}"),
    previous: "Previous picture",
    next: "Next picture",
    tabs: "Pictures",
    pause_pictures: "Pause the pictures",
    play_pictures: "Play the pictures",
    pause: "Pause",
    play: "Play",
    map_slide: Slide { tab: "Map", title: "The map", action: "View it large" },
    map_text: |n| format!("{n} degree programmes, linked by shared modules"),
    catalog_slide: Slide { tab: "Catalogue", title: "The catalogue", action: "To the catalogue" },
    catalog_text: |n| format!("All {n} modules, filtered as you type"),
    catalog_text_plain: "All modules, filtered as you type",
    catalog_alt: "The module catalogue: the search “datenbank” with six results, on the right the module Datenbanken with its weekly schedule",
    day_slide: Slide { tab: "Your day", title: "Your day", action: "To the start page" },
    day_text: "What is on right now, what comes next, when your exams are",
    day_alt: "The start page with a study set up: the greeting with the next session, under it the day, the week, the exams and how far the studies are",
    study_slide: Slide { tab: "Studies", title: "My studies", action: "To “My studies”" },
    study_text: "Passed, planned, open: semester by semester",
    study_alt: "“My studies” for Informatik B.Sc.: the credit points passed, the areas as a bar, under it the modules of the current semester",
    timetable_slide: Slide { tab: "Timetable", title: "The timetable", action: "To the timetable" },
    timetable_text: "Your semester as a week, with every clash",
    timetable_alt: "The timetable of a semester as a week: lectures, exercises and a lab course, a clash marked, an exercise group still to choose",

    map_heading: "How the degree programmes are connected",
    map_hint: "Each dot is a degree programme; a line joins two whose curricula share modules. A click on a dot shows where it belongs.",
    close_map: "Close the map",
    legend: "Legend",
    bachelor: "Bachelor",
    master: "Master",
    other: "Other",
    shared_modules: "Shared modules",
    program_count: |n| if n == 1 { "1 degree programme".to_string() } else { format!("{n} degree programmes") },
    lines_show: "Lines show shared modules",
    all_programs: "All degree programmes",
    program_modules: |n| if n == 1 { "1 module".to_string() } else { format!("{n} modules") },
    faculty: |code| format!("Faculty {code}"),
    shares: |relatives| format!("shares {relatives}"),
    shared_with: |n, program| format!("{n} with {program}"),
    shared_modules_with: "Shared modules with",
    to_program: "To the degree programme",
    clear_pick: "Clear the selection",

    ways_in: "Ways into the catalogue",
    all_modules_count: |n| format!("All {n} modules"),
    programs_by_faculty: "Degree programmes by faculty",
    all_programs_count: |n| format!("All {n} degree programmes"),
    no_faculty: "Across faculties or not clearly assigned",
    winter: Entry { label: "In the winter semester", hint: "Modules offered in the winter semester" },
    summer: Entry { label: "In the summer semester", hint: "Modules offered in the summer semester" },
    english: Entry { label: "In English", hint: "Modules taught in English" },
    fues: Entry { label: "Interdisciplinary studies", hint: "FÜS modules of all faculties" },
    no_written_exam: Entry { label: "Without a written exam", hint: "Modules whose assessment names no written exam" },
    oral_exam: Entry { label: "With an oral exam", hint: "Modules with an oral exam" },
    senftenberg: Entry { label: "In Senftenberg", hint: "Modules at the Senftenberg campus" },
    ungraded: Entry { label: "Ungraded", hint: "Modules completed without a grade" },

    abilities: "What Betula does",
    questions: "Questions and answers",
    abilities_lead: "The same data as at BTU, prepared so that you can plan with it.",
    search: Ability {
        title: "One search box for everything",
        text: "German and English titles, module numbers, abbreviations and lecturers in one search, on every page, with the / key. The results appear as you type, and under them “Similar modules” on the same topic. Ctrl+K opens the commands for everything else.",
    },
    filters: Ability {
        title: "Filters that work together",
        text: "Degree programme, area, semester, offering, teaching form, assessment, credit points, language, location, lecturers and more, in any combination. Almost every one can be inverted (“everything but a written exam”), and every selection is a link.",
    },
    plans: Ability {
        title: "Study plans from the regulations",
        text: "Read from the PDFs of the examination and study regulations and checked against their own totals: semester by semester, with compulsory and elective modules and every study direction.",
    },
    study: Ability {
        title: "Your studies, semester by semester",
        text: "“My studies” shows what is passed, planned and open, with the credit points of every area. While planning you see whether a module is offered in that semester and whether its prerequisites come before it.",
    },
    timetable: Ability {
        title: "A timetable that thinks ahead",
        text: "Import a whole semester of your programme or plan modules one by one: Betula shows the week, finds clashes and exams too close together, and looks for modules that still fit. With your own events and notes, as an .ics file or a calendar subscription.",
    },
    dates: Ability {
        title: "Dates from the course catalogue",
        text: "Lectures, exercises and exams as a weekly schedule with the module, with rooms and A and B weeks. Placeholders in QIS, such as an exam at 1 a.m., show as “Time TBA” instead of a wrong time.",
    },
    day: Ability {
        title: "Your day on the start page",
        text: "What is on right now, what comes next and in which room: the start page shows your day, the week, the next exams and how far your studies are.",
    },
    marks: Ability {
        title: "Saved modules with collections and notes",
        text: "One click on the bookmark saves a module. Collections sort what belongs together, a note keeps why, and the list shows what of it counts in your studies.",
    },
    prerequisites: Ability {
        title: "Prerequisites to click on",
        text: "What a module requires, mandatory or recommended, leads straight to that module as a link, with the wording of the module description. A module that was replaced names its successor.",
    },
    profile: Ability {
        title: "Your data stays with you",
        text: "What you save and plan is kept in your profile on your device. With Betula Sync all your devices have it, encrypted with a key only they know. No ads, no tracking.",
    },
    offline: Ability {
        title: "Fast, offline, even without JavaScript",
        text: "With JavaScript the catalogue is a database in your browser: search and filters answer at once, even without a network, and Betula installs like an app. Without JavaScript every page is complete HTML.",
    },
    gaps: Ability {
        title: "Honest about gaps",
        text: "Where the source says nothing, the page says “not stated” and makes no guess, and what Betula derives, it says so. Every module links to its original at BTU.",
    },
    in_progress: "In progress",
    coming: "Check prerequisites: what you can take with what you have passed",

    about_betula: Faq {
        name: "About Betula",
        questions: &[
            (
                "Is Betula a service of BTU?",
                "No. Betula is an unofficial, independent project and not part of BTU Cottbus-Senftenberg. Only the university's module descriptions, examination and study regulations are binding; that is why every page here links to its original.",
            ),
            (
                "Who makes Betula?",
                "Betula is a private, independent project; BTU has neither commissioned nor reviewed it. Who is behind it and how to get in touch is in the legal notice. Reports of errors are welcome.",
            ),
            (
                "Why is it called Betula?",
                "Betula is the Latin name of the birch. It is a pioneer plant and one of the first to grow on the land that mining has left behind in Lusatia. And BeTUla has BTU in it.",
            ),
            (
                "How does Betula work?",
                "Betula is named after the birch, and so are its parts. Radix, the root, reads BTU's public pages and the PDFs of its examination regulations, checks the result and publishes it as a data set: modules, degree programmes with their standard study plans and areas, the dates of the course catalogue. Folia, the leaves, is the web app you are looking at. Its server delivers every page as finished HTML; with JavaScript your browser loads the data set once as a database and from then on answers search and filters itself. And Phloem, the tissue under the bark that carries the sap, hands the encrypted changes of Betula Sync on between your devices. Which versions are running is shown at the end of this page and on the profile page.",
            ),
            (
                "Where does the data come from?",
                "From BTU's public sources: the module descriptions, the course catalogue and the module tree of each degree programme in the QIS portal, the list of interdisciplinary studies (FÜS), and the examination and study regulations BTU publishes as PDFs. Betula organises the information and makes it searchable; nothing in the content is changed and nothing is added. Where the source says nothing, the page says “not stated”.",
            ),
            (
                "How up to date is the catalogue?",
                "Radix reads the sources continuously, in small steps and mostly at night: dates that are not settled yet every two hours, the course catalogue every night, module descriptions and degree programmes at least once a month. A new data set appears as soon as something has changed and it has passed every check; when that last happened is shown at the end of this page and on the profile page. Short-notice changes appear at BTU first, and if in doubt, the original applies.",
            ),
            (
                "Does Betula cost anything?",
                "No. Betula is free, without advertising and without tracking: no analytics, no cookies that recognise you, and nothing is loaded from other servers. It is a private, non-commercial project.",
            ),
            (
                "Where is my data kept?",
                "In your profile, and that is kept locally on your device, in your browser's storage: your saved modules with their collections and notes, your timetable, “My studies” and “My programme”. Betula asks neither for your name nor for an e-mail address, and the server gets none of it. Only when you switch on Betula Sync is a copy kept on the server, encrypted with a key only your devices have. The details are in the privacy policy.",
            ),
        ],
    },
    using_betula: Faq {
        name: "Using Betula",
        questions: &[
            (
                "How do I find the right module quickly?",
                "Type a title, a module number, an abbreviation or a lecturer's name into the search at the top (the / key jumps there); under the results stand “Similar modules” on the same topic. Or start with a filter: degree programme, area, semester, offering, form of assessment, credit points, language, location, lecturers and more. All filters can be combined and almost all inverted, for example into “everything but a written exam”. Every selection is part of the address, so you can share it, bookmark it and go back with the browser's Back button. The page “Finding modules”, which “More about Betula” on the start page leads to, explains every filter.",
            ),
            (
                "What does the start page show?",
                "Once your studies are set up, your day: what is on right now, what comes next and in which room, and with it the week, the next exams, how far your studies are and your saved modules. Setting up takes three clicks: programme, start of studies, semester. If you want to look around first, you can search and browse without setting anything up.",
            ),
            (
                "What is “My studies”?",
                "Your plan up to your degree. Betula fills the semesters from the standard study plan of your programme, you tick off what you have passed and plan what comes. A bar shows for every area how many credit points are passed, planned and still open. While planning you see whether a module is offered in that semester and whether its prerequisites come before it; the plan allows for semesters of leave, retakes and a change of programme.",
            ),
            (
                "What can the timetable do?",
                "It shows one semester as a week. Modules come in with “Plan”, from your saved modules, or as a whole semester of your standard study plan, whose elective rows become placeholders. Betula counts clashes on the days on which dates really meet, with A and B weeks, and for exercises with several groups only when no group is free any more. It warns about exams that overlap or leave too little time to get from one site to the other, and “Fits my timetable” finds modules that still fit. Your own events stand beside them, and a session from QIS can be moved and given a note. You can take the plan along as an .ics file, as a picture, or as a calendar subscription that fetches changes by itself.",
            ),
            (
                "Can I save modules?",
                "Yes. Every module has a bookmark, and the M key does the same. You sort your saved modules into collections, every module can carry a note, and the list shows what of it counts in your studies and is planned or passed already. It can be copied as text and works as a filter in the catalogue (“Saved”). It belongs to your profile and is kept on your device.",
            ),
            (
                "Can I use Betula on several devices?",
                "Yes, with Betula Sync. Switch it on for a profile on the “Profile” page and scan the QR code under “Connect a device” with the other device: from then on your phone, laptop and tablet have the same state, and a change is on the others at once, even after a while without a network. Without Sync, a backup file takes a profile to another device.",
            ),
            (
                "Who can read my data with Betula Sync?",
                "Only your devices. Your browser encrypts every change before it leaves the device, with a key it makes itself and that travels from device to device only by QR code or link. The server keeps the sealed changes and hands them on; it knows random ids, sizes and times, but neither the content nor you: there is no name, e-mail address or password for it. “End Sync” deletes the copy on the server at once.",
            ),
            (
                "What happens if I lose my phone or clear my browser?",
                "With Betula Sync, nothing bad: every other connected device has the whole profile. If all devices are gone, the recovery code brings it back; Betula shows it to you on the “Profile” page, keep it like a password. You shut out a lost device by ending Sync and switching it on again. Without Sync a profile is kept in that one browser only; then a backup file protects it, which you also make on the “Profile” page.",
            ),
            (
                "Can I share my timetable or my saved modules?",
                "Yes, with “Share …”, in three ways. As a fixed copy: a link with a copy of now, which anyone who has it can open. So that it stays current: others follow your calendar or your list and see every change until you stop sharing. Or as a one-time link for exactly one person, which holds for 7 days at most. What stays current and the one-time link need Betula Sync, and you choose whether your own events and notes come along.",
            ),
            (
                "Can I plan for two degree programmes?",
                "Yes, with a profile for each. Every profile has its own saved modules, timetable and studies; on the “Profile” page you create one and switch between them. A second profile is also good for trying out another course of your studies.",
            ),
            (
                "Does Betula work without JavaScript?",
                "Yes. Every page comes from the server as complete HTML: the catalogue with all its filters, module pages, degree programmes and standard study plans work without JavaScript, as a classic website, for text browsers and search engines alike. With JavaScript an app written in Rust and running as WebAssembly takes over: it loads the catalogue into your browser once as an SQLite database and from then on answers every search and every filter itself, without loading times between pages. What belongs to your profile, your saved modules, your timetable and “My studies”, needs JavaScript.",
            ),
            (
                "Does Betula work offline and as an app?",
                "Yes. After the first visit with JavaScript the catalogue and the app are kept in your browser, and Betula starts even without a network. On a phone, tablet or computer it installs like an app, with its own icon on the home screen. The app downloads new data in the background and uses it from the next start.",
            ),
            (
                "Is Betula available in English?",
                "Yes. The interface is in German and English, every page at its own address (/en). With JavaScript, Betula opens in your browser's language on the first visit and remembers your choice. Module titles and descriptions are shown as BTU writes them; many modules also have an English title, and you can filter for modules taught in English.",
            ),
        ],
    },
    for_studies: Faq {
        name: "For your studies",
        questions: &[
            (
                "How do I find the modules for my studies?",
                "Through your degree programme. Which modules you take is set by its study and examination regulations; Betula prepares them and brings everything together in one place. Under “Choose a degree programme” you will find the standard study plan semester by semester (where the regulations contain one), the compulsory elective areas and all modules of the programme; “In the module catalogue” opens the catalogue narrowed down to it. A click on a module shows its contents, dates and exam. Set as “My programme”, it is preselected in the catalogue and the timetable.",
            ),
            (
                "What is a module, and what does a module description say?",
                "A module is a self-contained unit of teaching, usually over one semester, for which you get credit points (CP). The module description names its contents and learning outcomes, forms of teaching and workload, prerequisites, the assessment, when it is offered and who is responsible. Together, the descriptions make up the module handbook of a degree programme.",
            ),
            (
                "What are credit points (CP)?",
                "Credit points, also called ECTS points, measure the workload of a module: lectures, exercises and self-study together. One point stands for about 25 to 30 hours. A semester according to the standard study plan usually comes to 30 CP.",
            ),
            (
                "What do compulsory and compulsory elective mean?",
                "Everyone in the degree programme takes the compulsory modules. For compulsory electives you choose from an area set by the regulations until you have the required credit points. Betula shows the areas of every degree programme under “Electives & areas”, and in the catalogue the “Area” filter narrows the list down to one of them.",
            ),
            (
                "When is a module offered?",
                "Its description says when it is offered: in the winter semester, in the summer semester or every semester, some modules only in even or odd years. You can filter by all of it in the catalogue. Whether a module really takes place in the current semester shows in its dates: they are shown with the module as a weekly schedule, and the “Confirmed” filter hides every module without published dates.",
            ),
            (
                "Where do I register for modules and exams?",
                "Not on Betula. Registration goes through BTU's systems; deadlines and rules are stated there and in your examination regulations. Betula helps you plan and links every module to its original.",
            ),
            (
                "How do I see how many credit points I still need?",
                "In “My studies”. It shows how many credit points you have passed and planned, in all and for every area of your programme, and what is still open according to the standard study plan. Betula counts what you mark as passed; your transcript at BTU is what is binding.",
            ),
        ],
    },

};

#[cfg(test)]
mod tests {
    use super::*;

    /// Every language asks the same questions, and where an answer names a button, it names it
    /// as the page labels it.
    #[test]
    fn the_questions_match_in_every_language() {
        for (de, en) in [(&DE.about_betula, &EN.about_betula), (&DE.using_betula, &EN.using_betula), (&DE.for_studies, &EN.for_studies)] {
            assert_eq!(de.questions.len(), en.questions.len(), "{}", de.name);
        }
        for (t, open, close) in [(&DE, "„", "“"), (&EN, "“", "”")] {
            let first = t.for_studies.questions.first().map(|(_, answer)| *answer).unwrap_or_default();
            assert!(first.contains(&format!("{open}{}{close}", t.choose_program)), "{first}");
        }
        assert_eq!(((DE.slide_of)(2, 4, "Der Katalog"), (EN.in_semester)("Winter 2026/27")), ("2 von 4: Der Katalog".to_string(), "in Winter 2026/27".to_string()));
    }

    /// Nothing here speaks of an account: there is none (owner, 2026-11-18), and the texts say
    /// what there is instead, a profile kept on the device. The descriptions, the abilities and
    /// every answer, in every language.
    #[test]
    fn nothing_speaks_of_an_account() {
        for t in [&DE, &EN] {
            let abilities = [&t.search, &t.filters, &t.plans, &t.study, &t.timetable, &t.dates, &t.day, &t.marks, &t.prerequisites, &t.profile, &t.offline, &t.gaps];
            let mut texts: Vec<&str> = vec![t.description, t.browser_requirements, t.abilities_lead];
            texts.extend(abilities.iter().flat_map(|ability| [ability.title, ability.text]));
            texts.extend([&t.about_betula, &t.using_betula, &t.for_studies].iter().flat_map(|faq| faq.questions.iter()).flat_map(|(question, answer)| [*question, *answer]));
            for text in texts {
                let lower = text.to_lowercase();
                assert!(!lower.contains("konto") && !lower.contains("konten") && !lower.contains("account"), "{text}");
            }
        }
    }

}
