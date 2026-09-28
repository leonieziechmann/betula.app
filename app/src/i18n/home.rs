//! Texts of the start page (`pages/home.rs`), in the order of the page: what search engines read,
//! the sidebar, the first panel, the pictures and the map, the ways into the catalog, what Betula
//! does, the questions, and Betula in detail.
//!
//! Everything here says what the app really does, and says it plainly (owner, 2026-09-28: the page
//! is what search engines and the assistants built on them know of Betula; comparing it with other
//! tools they took the missing account for missing functions, and did not see that it works
//! without JavaScript or that the study plans are read from the regulations). Where a text names a
//! button or a filter, it names it as the page labels it; the tests below hold them together.

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

    // The sidebar
    pub on_this_page: &'static str,
    /// The sections of the page, as the sidebar lists them. „Was Betula kann" and „Fragen und
    /// Antworten" are also the headings of their sections; the last one's heading is
    /// `details.heading`.
    pub overview: &'static str,
    pub ways_in_and_faculties: &'static str,
    pub abilities: &'static str,
    pub questions: &'static str,
    pub in_detail: &'static str,
    /// „Datenstand": the facts about the data, with their labels.
    pub data: &'static str,
    pub semester: &'static str,
    pub last_changed: &'static str,
    pub source: &'static str,
    /// What the link „BTU" leads to, as its title says.
    pub source_title: &'static str,

    // The first panel
    /// „Inoffiziell · für die BTU Cottbus-Senftenberg", over the heading.
    pub eyebrow: &'static str,
    /// The heading, before and after the university's name (which does not break).
    pub title_before: &'static str,
    pub title_after: &'static str,
    pub lead: &'static str,
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
    pub plan_slide: Slide,
    pub plan_text: &'static str,
    pub plan_alt: &'static str,
    pub module_slide: Slide,
    pub module_text: &'static str,
    pub module_alt: &'static str,

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
    pub abilities_lead: &'static str,
    pub search: Ability,
    pub filters: Ability,
    pub plans: Ability,
    pub timetable: Ability,
    pub dates: Ability,
    pub prerequisites: Ability,
    pub account: Ability,
    pub offline: Ability,
    pub gaps: Ability,
    /// „In Arbeit", and what is.
    pub in_progress: &'static str,
    pub coming: &'static str,

    // The questions
    pub about_betula: Faq,
    pub using_betula: Faq,
    pub for_studies: Faq,

    // Betula in detail
    pub details: Details,
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

/// „Betula im Detail": what the page says above, at length and in one piece — how one finds
/// what one is looking for, every filter, the plans, the Stundenplan, and how Betula is built.
pub struct Details {
    pub heading: &'static str,
    pub lead: &'static str,
    /// From a question to the module: the ways through the app.
    pub flow: Chapter,
    /// Every filter of the catalog: the list stands between its two parts of text.
    pub filters: Chapter,
    pub filter: Filters,
    /// The study plans read from the regulations: the number of plans stands between its two parts.
    pub plans: Chapter,
    /// „112 von 148 Studiengängen …": the current programs with a checked plan, and all current
    /// programs; the numbers are written already.
    pub plans_count: fn(&str, &str) -> String,
    pub timetable: Chapter,
    pub account: Chapter,
    pub javascript: Chapter,
    pub data: Chapter,
    pub devices: Chapter,
    /// The ways on at the end of a chapter.
    pub to_catalog: &'static str,
    pub to_programs: &'static str,
    pub to_studyplan: &'static str,
    pub to_privacy: &'static str,
}

/// A chapter of „Betula im Detail": its heading, and its paragraphs before and after what the
/// page puts in its middle (the list of filters, the number of plans); most have nothing there.
pub struct Chapter {
    pub title: &'static str,
    pub text: &'static [&'static str],
    pub more: &'static [&'static str],
}

/// What each filter of the catalog does, in the order of its panel. Their names are the panel's
/// own (`catalog::Texts`); only the order of the list has none there.
pub struct Filters {
    pub search: &'static str,
    pub program: &'static str,
    pub list: &'static str,
    pub kind: &'static str,
    pub area: &'static str,
    pub plan_semester: &'static str,
    pub confirmed: &'static str,
    pub fits: &'static str,
    pub offered_in: &'static str,
    pub teaching_form: &'static str,
    pub exam: &'static str,
    pub credits: &'static str,
    pub language: &'static str,
    pub properties: &'static str,
    pub lecturers: &'static str,
    pub department: &'static str,
    pub duration: &'static str,
    pub years: &'static str,
    pub location: &'static str,
    pub not_offered: &'static str,
    pub sort_name: &'static str,
    pub sort: &'static str,
}

pub const DE: Texts = Texts {
    description: "Alle Module und Studiengänge der BTU Cottbus-Senftenberg an einem Ort: durchsuchen und filtern, Regelstudienpläne aus den Prüfungsordnungen, dein Stundenplan mit Kalender-Abo. Kostenlos, ohne Konto, auch offline. Inoffiziell, mit Link zum Original.",
    seo_title: "Betula · Modulkatalog für die BTU Cottbus-Senftenberg",
    alternate_names: ["Betula Modulkatalog", "Modulkatalog BTU Cottbus-Senftenberg (inoffiziell)"],
    browser_requirements: "Läuft in jedem aktuellen Browser, auch ohne JavaScript. Mit JavaScript lädt die App den Katalog als SQLite-Datenbank in den Browser (WebAssembly), antwortet ohne Ladezeiten und funktioniert offline.",

    on_this_page: "Auf dieser Seite",
    overview: "Überblick",
    ways_in_and_faculties: "Einstiege und Fakultäten",
    abilities: "Was Betula kann",
    questions: "Fragen und Antworten",
    in_detail: "Im Detail",
    data: "Datenstand",
    semester: "Semester",
    last_changed: "Zuletzt geändert",
    source: "Quelle",
    source_title: "Modulbeschreibungen, Studiengangsseiten und Vorlesungsverzeichnis der BTU Cottbus-Senftenberg",

    eyebrow: "Inoffiziell · für die BTU Cottbus-Senftenberg",
    title_before: "Alle Module und Studiengänge der ",
    title_after: ", an einem Ort.",
    lead: "Module mit Inhalten, Voraussetzungen, Prüfung und Terminen, Studiengänge mit dem Regelstudienplan aus ihrer Prüfungsordnung, dazu dein eigener Stundenplan: durchsuchbar und filterbar, statt Modulhandbücher zu wälzen.",
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
    // „Studienplan" names the visitor's own plan now (the area „Plan"); the program's plan is the
    // Regelstudienplan.
    plan_slide: Slide { tab: "Regelstudienplan", title: "Der Regelstudienplan", action: "Studiengang wählen" },
    plan_text: "Semester für Semester, als Matrix",
    plan_alt: "Der Regelstudienplan von Informatik B.Sc. als Matrix: Module mal Semester, mit Pflicht und Wahlpflicht",
    module_slide: Slide { tab: "Modul", title: "Ein Modul", action: "Beispiel ansehen" },
    module_text: "Inhalte, Prüfung und Termine auf einer Seite",
    module_alt: "Die Seite des Moduls Datenbanken: Inhalte, Lernziele, Prüfungsleistung und der Wochenplan der Termine",

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

    abilities_lead: "Dieselben Daten wie bei der BTU, so aufbereitet, dass man mit ihnen planen kann.",
    search: Ability {
        title: "Ein Suchfeld für alle Module",
        text: "Deutscher und englischer Titel und die Modulnummer in einer Suche, über alle Fakultäten und auf jeder Seite, mit Strg+K oder /. Die Treffer stehen da, während du tippst.",
    },
    filters: Ability {
        title: "Filter, die zusammenpassen",
        text: "Studiengang, Bereich, Fachsemester, Turnus, Lehrform, Prüfung, Leistungspunkte, Sprache, Standort, Lehrende und mehr, beliebig kombiniert. Fast jeder lässt sich umkehren („alles außer Klausur“), und jede Auswahl ist ein Link.",
    },
    plans: Ability {
        title: "Regelstudienpläne aus den Prüfungsordnungen",
        text: "Aus den PDFs der Prüfungs- und Studienordnungen gelesen und gegen deren eigene Summen geprüft: Semester für Semester, mit Pflicht, Wahlpflicht und jeder Studienrichtung.",
    },
    timetable: Ability {
        title: "Ein Stundenplan, der mitdenkt",
        text: "Module einplanen oder ein ganzes Fachsemester übernehmen: Betula zeigt die Woche, findet Überschneidungen und zu knappe Prüfungen und sucht Module, die noch hineinpassen. Als .ics-Datei oder Kalender-Abo.",
    },
    dates: Ability {
        title: "Termine aus dem Vorlesungsverzeichnis",
        text: "Vorlesungen, Übungen und Prüfungen als Wochenplan beim Modul, mit Räumen, A- und B-Wochen. Platzhalter aus QIS wie eine Prüfung um 01:00 Uhr erscheinen als „Zeit offen“ statt als falsche Uhrzeit.",
    },
    prerequisites: Ability {
        title: "Voraussetzungen zum Anklicken",
        text: "Was ein Modul zwingend oder empfohlen voraussetzt, führt als Link direkt zu diesem Modul, mit dem Wortlaut der Modulbeschreibung. Ein abgelöstes Modul nennt sein Nachfolgemodul.",
    },
    account: Ability {
        title: "Kein Konto, und nichts fehlt",
        text: "Merkliste, Stundenplan, gespeicherte Pläne und dein Studiengang liegen in deinem Browser statt auf einem Server. Per Link kommen sie auf andere Geräte, per Abo in deinen Kalender. Ohne Anmeldung, Werbung oder Tracking.",
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
    coming: "Studienverlauf: bestandene Module abhaken, Voraussetzungen prüfen",

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
                "Betula hat zwei Teile, benannt nach der Birke. Radix, die Wurzel, liest die öffentlichen Seiten der BTU und die PDFs ihrer Prüfungsordnungen ein, prüft das Ergebnis und veröffentlicht es als Datenstand: Module, Studiengänge mit Regelstudienplänen und Bereichen, die Termine des Vorlesungsverzeichnisses. Folia, die Blätter, ist die Web-App, die du gerade siehst. Ihr Server liefert jede Seite als fertiges HTML; mit JavaScript lädt dein Browser den Datenstand einmal als Datenbank und beantwortet danach Suche und Filter selbst. Welche Versionen gerade laufen, steht am Ende jeder Seite.",
            ),
            (
                "Woher kommen die Daten?",
                "Aus öffentlichen Quellen der BTU: den Modulbeschreibungen, dem Vorlesungsverzeichnis und dem Modulbaum der Studiengänge im Portal QIS, der Liste des Fachübergreifenden Studiums und den Prüfungs- und Studienordnungen, die die BTU als PDF veröffentlicht. Betula ordnet die Angaben und macht sie durchsuchbar; am Inhalt wird nichts geändert und nichts ergänzt. Wo die Quelle nichts sagt, steht „nicht angegeben“.",
            ),
            (
                "Wie aktuell ist der Katalog?",
                "Radix liest die Quellen laufend nach, in kleinen Schritten und das meiste nachts: Termine, die noch nicht feststehen, alle zwei Stunden, das Vorlesungsverzeichnis jede Nacht, Modulbeschreibungen und Studiengänge mindestens einmal im Monat. Ein neuer Datenstand erscheint, sobald sich etwas geändert und er alle Prüfungen bestanden hat; wann das zuletzt war, steht in der Seitenleiste. Kurzfristige Änderungen stehen zuerst bei der BTU, und im Zweifel gilt das Original.",
            ),
            (
                "Kostet Betula etwas?",
                "Nein. Betula ist kostenlos, ohne Werbung und ohne Tracking: kein Analysedienst, keine Cookies, die dich wiedererkennen, und nichts wird von fremden Servern geladen. Es ist ein privates, nicht-kommerzielles Projekt.",
            ),
            (
                "Warum gibt es kein Konto? Fehlt dadurch etwas?",
                "Nein. Das Konto fehlt mit Absicht, nicht die Funktionen: Merkliste, Stundenplan mit Platzhaltern und ausgewählten Terminen, gespeicherte Pläne, „Mein Studiengang“ und deine Einstellungen funktionieren ohne Anmeldung, weil sie in deinem Browser liegen statt auf einem Server. Niemand sonst kann sie einsehen, und es gibt weder Passwort noch Profil. Auf ein anderes Gerät kommen Merkliste und Stundenplan per Link, in Apple Kalender, Google Kalender oder Outlook per Abo.",
            ),
        ],
    },
    using_betula: Faq {
        name: "Betula nutzen",
        questions: &[
            (
                "Wie finde ich schnell das richtige Modul?",
                "Tippe oben in die Suche einen Titel oder eine Modulnummer (Strg+K oder / springt hinein), oder fang mit einem Filter an: Studiengang, Bereich, Fachsemester, Turnus, Prüfungsform, Leistungspunkte, Sprache, Standort, Lehrende und mehr. Alle Filter lassen sich kombinieren und fast alle umkehren, etwa zu „alles außer Klausur“. Jede Auswahl steht in der Adresse, lässt sich also teilen, als Lesezeichen speichern und mit „Zurück“ zurücknehmen. Jeden Filter einzeln erklärt der Abschnitt „Betula im Detail“ am Ende dieser Seite.",
            ),
            (
                "Was kann der Stundenplan?",
                "Er zeigt ein Semester als Woche. Module kommen mit „Einplanen“ hinein, aus der Merkliste oder als ganzes Fachsemester aus dem Regelstudienplan, dessen Wahlpflichtzeilen zu Platzhaltern werden. Betula zählt Überschneidungen an den Tagen, an denen sich Termine wirklich treffen, mit A- und B-Wochen, bei Übungen mit mehreren Gruppen erst, wenn keine Gruppe mehr frei ist. Es warnt vor Prüfungen, die sich überschneiden oder zu wenig Zeit für den Weg zwischen den Standorten lassen, und „Passt in meinen Stundenplan“ findet Module, die noch hineinpassen. Mitnehmen lässt sich der Plan als .ics-Datei, als Kalender-Abo, das Änderungen selbst nachlädt, oder als Link zum Teilen.",
            ),
            (
                "Kann ich mir Module merken?",
                "Ja. Jedes Modul hat ein Lesezeichen, die Taste M tut dasselbe. Die Merkliste zählt deine Module und Leistungspunkte, trennt Winter und Sommer, lässt sich als Text kopieren und dient im Katalog als Filter („Gemerkt“). Sie liegt nur in deinem Browser: kein Konto, und nichts davon erreicht den Server.",
            ),
            (
                "Kann ich Betula auf mehreren Geräten nutzen?",
                "Ja, ohne Konto. „Auf anderes Gerät übertragen“ kopiert einen Link, der deine Merkliste hinter dem „#“ der Adresse trägt, und diesen Teil schickt kein Browser an einen Server. Einen Stundenplan gibst du mit „Link zum Teilen kopieren“ weiter, an dein anderes Gerät oder an andere Studierende, die seine Module dann übernehmen können. Und ein Kalender-Abo zeigt deinen Plan auf jedem Gerät, auf dem dein Kalender läuft.",
            ),
            (
                "Funktioniert Betula ohne JavaScript?",
                "Ja. Jede Seite kommt vom Server als vollständiges HTML: Der Katalog mit allen Filtern, die Modulseiten, Studiengänge und Regelstudienpläne funktionieren ohne JavaScript, als klassische Website, für Textbrowser und Suchmaschinen genauso. Mit JavaScript übernimmt eine App aus Rust und WebAssembly: Sie lädt den Katalog einmal als SQLite-Datenbank in deinen Browser und beantwortet danach jede Suche und jeden Filter selbst, ohne Ladezeiten zwischen den Seiten. Was in deinem Browser liegt, Merkliste und Stundenplan, braucht JavaScript.",
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
        ],
    },

    details: Details {
        heading: "Betula im Detail",
        lead: "Was Betula kann, wie du damit findest, was du suchst, und wie es gebaut ist, ausführlich und an einem Stück.",
        flow: Chapter {
            title: "Von der Frage zum Modul",
            text: &[
                "Betula ist um die Fragen gebaut, mit denen man einen Modulkatalog aufschlägt: Was steht im dritten Semester an? Welche Wahlpflichtmodule gibt es auf Englisch? Was passt noch in meine Woche? Für jede gibt es einen kurzen Weg, und alle enden bei derselben Modulseite: Inhalte und Lernziele, Prüfung, Leistungspunkte, Turnus, Voraussetzungen, die Termine als Wochenplan und die Studiengänge, in denen das Modul vorkommt, jeweils mit dem Semester laut Regelstudienplan.",
                "Die Suche steht oben auf jeder Seite und findet Module über ihren deutschen oder englischen Titel und ihre Nummer, über alle Fakultäten. Die Startseite bietet fertige Einstiege mit der Zahl ihrer Treffer, etwa „Auf Englisch“ oder „Ohne Klausur“, dazu die Studiengänge nach Fakultät und eine Karte, die zeigt, welche Studiengänge sich Module teilen.",
                "Vom Studiengang aus führt der Regelstudienplan zu seinen Modulen und „Wahlpflicht & Bereiche“ zu allem, was zur Wahl steht. „Im Modulkatalog“ öffnet den Katalog genau auf diesen Studiengang, einen Bereich oder eine Zeile des Plans eingegrenzt. Wählst du dort ein Fachsemester, stehen neben den Pflichtmodulen auch die Wahlpflichtmodule, die der Plan in diesem Semester verlangt.",
                "Im Katalog öffnet ein Klick die Vorschau neben der Liste, und Liste, Filter und Scrollposition bleiben, wie sie waren. F zeigt das Modul als ganze Seite, Esc schließt es, ↑ und ↓ gehen durch die Liste, Enter öffnet, M merkt. Jeder aktive Filter steht über der Liste und lässt sich dort einzeln entfernen; findet eine Kombination nichts, schlägt die Seite vor, was du zurücknehmen kannst.",
                "Jede Ansicht hat ihre eigene Adresse: Suche, Filter, Sortierung und das geöffnete Modul stehen im Link. Du kannst ihn teilen oder als Lesezeichen speichern, und „Zurück“ im Browser führt immer dorthin, wo du vorher warst. Start, Module, Studium, Merkliste und Stundenplan merken sich wie Tabs, wo du sie verlassen hast.",
            ],
            more: &[],
        },
        filters: Chapter {
            title: "Alle Filter des Modulkatalogs",
            text: &["Alle Filter lassen sich frei kombinieren. Die meisten haben drei Stufen: Ein Klick nimmt nur die Module mit dieser Eigenschaft, ein zweiter schließt sie aus, ein dritter hebt den Filter auf. Zweimal „Klausur“ heißt also „alles außer Klausur“. Ein Ausschluss entfernt nur, was die Daten ausdrücklich sagen: Ein Modul ohne Angabe bleibt in der Liste, statt stillschweigend zu verschwinden."],
            more: &[
                "Lehrende, Fachgebiet, Dauer, Jahre, Standort und nicht mehr angebotene Module stehen unter „Weitere Filter“. „Zurücksetzen“ nimmt alle Filter auf einmal zurück.",
                "Auf dem Handy öffnen die Filter als Blatt von unten, das sich wegwischen lässt. Die Liste folgt erst, wenn das Blatt zu ist, und sein Knopf zeigt vorher schon, wie viele Module es werden. Ohne JavaScript sind die Filter Links und Formularfelder und funktionieren genauso.",
                "Die Übersicht der Studiengänge filtert nach Abschluss (Bachelor, Master, Lehramt, Promotion, Sonstige), Studienform (Dual, Doppelabschluss, Teilzeit & Fern) und „Mit Regelstudienplan“; ihre Suche findet jeden Teil eines Namens, Groß- und Kleinschreibung und Umlaute egal.",
            ],
        },
        filter: Filters {
            search: "Deutscher oder englischer Titel oder Modulnummer, auch Teile davon; in der App filtert die Liste schon beim Tippen.",
            program: "Grenzt auf einen Studiengang ein. Die Auswahl verzeiht Tippfehler und kennt Abkürzungen und Anfangsbuchstaben („infomatik bsc“), „Mein Studiengang“ steht obenan.",
            list: "Mit Studiengang: sein Curriculum oder seine FÜS-Liste, die fachübergreifenden Module, die er anerkennt.",
            kind: "Mit Studiengang: Pflicht, Wahlpflicht, Abschlussarbeit, Praktikum oder „Nicht angegeben“, wie Regelstudienplan, Modulbeschreibung oder Modulbaum es sagen.",
            area: "Mit Studiengang: die Bereiche seines Modulbaums, aus denen man wählt, etwa „Praktische Informatik“ oder ein Nebenfach, jeweils mit allem darunter; von einer Zeile des Plans aus auch mehrere zugleich.",
            plan_semester: "Mit Regelstudienplan: das Semester, in das er ein Modul legt, oder „?“ für Module ohne Semester. Ein Semester zeigt auch die Wahlpflichtmodule, die der Plan dort verlangt, und sagt, woraus sie abgeleitet sind.",
            confirmed: "Nur Module mit veröffentlichten Terminen, also die, die sehr wahrscheinlich stattfinden; ausgeschlossen nur die ohne.",
            fits: "Nur Module, die in die freien Zeiten deines Stundenplans passen. Du wählst, was verglichen wird: Vorlesungen, Übungen und Prüfungen. Vorlesungen müssen alle frei sein, bei Übungen genügt eine freie Gruppe, bei Prüfungen ein Termin ohne Konflikt; Module ohne Termine kommen nur mit „auch ohne Termine“ dazu.",
            offered_in: "Winter, Sommer oder Unregelmäßig, nach dem Turnus der Modulbeschreibung.",
            teaching_form: "Vorlesung, Übung, Seminar, Praktikum, Projekt, Exkursion.",
            exam: "Klausur, Mündlich, Hausarbeit, Vortrag, Projekt, Praktisch; ein Modul mit mehreren Prüfungsteilen erscheint bei jedem davon.",
            credits: "Ein Schieberegler mit zwei Griffen von 0 bis 30 Leistungspunkten, am rechten Ende ohne Obergrenze, darunter die genauen Zahlen zum Eintippen.",
            language: "Deutsch oder Englisch.",
            properties: "Benotet, Begrenzte Plätze, FÜS-Liste und, in der App, Gemerkt: nur deine gemerkten Module oder alle anderen.",
            lecturers: "Eine oder mehrere Personen, die ein Modul verantworten oder lehren, mit Suche. Gesuchte (+) gelten als „eine davon“, ausgeschlossene (×) als „keine davon“.",
            department: "Das Fachgebiet, das ein Modul anbietet, mit Suche.",
            duration: "1 Semester oder 2 Semester.",
            years: "Gerade oder Ungerade, für Module, die nur jedes zweite Jahr stattfinden.",
            location: "Zentralcampus Cottbus, Cottbus-Sachsendorf oder Senftenberg, bekannt für Module mit Räumen im aktuellen Semester.",
            not_offered: "Auch Module, die die BTU nicht mehr anbietet; im Katalog eines Studiengangs stehen sie ohnehin, soweit sein Curriculum sie nennt.",
            sort_name: "Sortierung",
            sort: "Nach Titel, Leistungspunkten oder Zahl der Termine, auf- und absteigend, per Klick auf den Spaltenkopf; im Studiengang in der Reihenfolge seines Plans.",
        },
        plans: Chapter {
            title: "Regelstudienpläne aus den Prüfungsordnungen",
            text: &[
                "Welche Module in welchem Semester vorgesehen sind, steht bei der BTU nur in den Prüfungs- und Studienordnungen, als Tabellen in PDF-Dateien. Betula hat diese Ordnungen heruntergeladen und ihre Pläne maschinenlesbar gemacht. Die Semesterspalten liest ein eigener Parser aus der Geometrie der Tabellen, Zelle für Zelle, auch verbundene Zellen, Spannen wie „10–24 LP“ und Module über mehrere Semester. Ein Sprachmodell hilft nur, Modulnummern und Arten zuzuordnen; was es über Semester und Leistungspunkte sagt, wird durch die Werte der Zellen ersetzt.",
                "Jeder Plan wird gegen die Summen geprüft, die die Ordnung selbst druckt, und mit dem Modulkatalog abgeglichen. Was nicht eindeutig ist, wird nicht geraten, sondern für eine Prüfung von Hand zurückgelegt. Beim Plan steht, wann er aus der Ordnung übernommen und geprüft wurde, und meist auch, auf welcher Seite der Ordnung er steht. Mit einem Modul des Katalogs wird eine Zeile nur verknüpft, wenn Nummer oder Titel es zweifelsfrei benennen, und hat ein Studiengang mehrere Studienrichtungen, hat jede ihren eigenen Plan.",
            ],
            more: &["Der Plan steht als Matrix aus Modulen und Semestern, wie die Ordnung ihn druckt, oder als Liste, mit den Summen der Ordnung. Zeilen, die kein Modul nennen, etwa „Wahlpflichtmodul aus der Informatik“, führen zu den Bereichen, die gemeint sind. „Wahlpflicht & Bereiche“ zeigt den ganzen Modulbaum des Studiengangs, und jeder Studiengang hat seine eigene Liste fachübergreifender Module (FÜS)."],
        },
        plans_count: |plans, programs| format!("Im jetzigen Datenstand hat Betula so für {plans} von {programs} Studiengängen in ihrer aktuellen Prüfungsordnung einen geprüften Regelstudienplan."),
        timetable: Chapter {
            title: "Der Stundenplan",
            text: &[
                "Der Stundenplan zeigt ein Semester als Woche, deine Module in einer Spalte daneben. Module kommen mit „Einplanen“ von jeder Modulseite hinein, aus der Merkliste oder als ganzes Fachsemester aus dem Regelstudienplan; Zeilen des Plans, die eine Wahl lassen, werden zu Platzhaltern, für die „Modul finden“ passende Module im Katalog sucht.",
                "Überschneidungen zählt Betula an den Tagen, an denen sich Termine wirklich treffen: mit A- und B-Wochen, Terminen nur in einem Teil des Semesters und ausgefallenen Tagen. Gibt es für eine Übung mehrere Gruppen, ist eine Überschneidung erst dann eine, wenn keine Gruppe mehr frei ist, und Termine nimmst du direkt in der Woche mit ✓ oder lässt sie mit × weg. Bei Prüfungen warnt Betula, wenn sie sich überschneiden oder zu wenig Zeit für den Weg zwischen Cottbus und Senftenberg oder zwischen zwei Standorten in Cottbus lassen.",
                "Mit „Passt in meinen Stundenplan“ listet der Katalog nur Module, die noch hineinpassen. Den Plan gibt es als .ics-Datei und als Kalender-Abo für Apple Kalender, Google Kalender oder Outlook, das geänderte Termine selbst nachlädt. Mehrere Pläne lassen sich unter eigenem Namen speichern, und „Link zum Teilen kopieren“ gibt einen Plan weiter: Die Vorschau im Messenger zeigt seine Module, und wer den Link öffnet, kann sie übernehmen.",
                "„Mein Studiengang“ merkt sich deinen Studiengang mit Studienrichtung, Studienbeginn und Standort; Katalog und Stundenplan beginnen dann darin.",
            ],
            more: &[],
        },
        account: Chapter {
            title: "Ohne Konto, und trotzdem vollständig",
            text: &[
                "Betula verzichtet mit Absicht auf Konten, nicht auf Funktionen. Merkliste, Stundenplan, gespeicherte Pläne, Kalender-Abos, „Mein Studiengang“ und Einstellungen wie Sprache und Farbschema funktionieren ohne Anmeldung, weil sie in deinem Browser liegen statt auf einem Server. Niemand sonst kann sie einsehen, es gibt kein Passwort, das verloren gehen kann, und kein Profil über dich.",
                "Was sonst ein Konto erledigt, erledigen Links. „Auf anderes Gerät übertragen“ kopiert einen Link, der deine Merkliste hinter dem „#“ trägt, und diesen Teil einer Adresse schickt kein Browser an einen Server. Ein Stundenplan wandert per „Link zum Teilen kopieren“ auf ein anderes Gerät oder zu anderen Studierenden, und ein Kalender-Abo zeigt ihn überall, wo dein Kalender läuft.",
                "Betula nutzt keine Analysedienste und keine Cookies, die dich wiedererkennen, und lädt nichts von fremden Servern: Schrift, Symbole und Skripte kommen von betula.app selbst. Der Server steht in einem Rechenzentrum in Deutschland und löscht sein Zugriffsprotokoll nach 7 Tagen.",
            ],
            more: &[],
        },
        javascript: Chapter {
            title: "Mit und ohne JavaScript, online und offline",
            text: &[
                "Jede Seite von Betula kommt vom Server als vollständiges HTML. Ohne JavaScript ist Betula eine klassische Website: Der Katalog blättert in Seiten, Filter sind Links und Formularfelder, und alle Module, Studiengänge und Regelstudienpläne sind da, für Suchmaschinen und Textbrowser genauso wie für alle, die JavaScript abgeschaltet haben. Der Server baut jede Seite einmal je Datenstand und liefert sie danach aus dem Speicher.",
                "Mit JavaScript übernimmt die App, geschrieben in Rust und als WebAssembly im Browser. Sie lädt den Katalog einmal als SQLite-Datenbank, komprimiert einige Megabyte, und beantwortet danach jede Suche und jeden Filter selbst, meist in wenigen Millisekunden: keine Ladezeiten zwischen den Seiten, eine Liste, die beim Tippen filtert, und ein Klick, der sofort sichtbar wird. Die App zeigt dieselben Seiten wie der Server, aus denselben Bausteinen, nur ohne den Umweg über das Netz.",
                "Danach läuft Betula auch offline: Ein Service Worker hält die App bereit, der Katalog liegt in deinem Browser, und auf Handy, Tablet und Rechner lässt sich Betula wie eine App installieren. Einen neuen Datenstand lädt die App im Hintergrund und nutzt ihn ab dem nächsten Start.",
            ],
            more: &[],
        },
        data: Chapter {
            title: "Woher die Daten kommen und wie sie geprüft werden",
            text: &[
                "Radix, der Teil von Betula, der die Daten sammelt, liest die öffentlichen Quellen der BTU: die Modulbeschreibungen, das Vorlesungsverzeichnis mit Terminen und Räumen und den Modulbaum jedes Studiengangs im Portal QIS, die Liste des Fachübergreifenden Studiums und die Prüfungsordnungen. Radix fragt höflich an, mit Pausen zwischen den Anfragen und das meiste nachts, und bewahrt die gelesenen Seiten auf, sodass sich ein Fehler beim Einlesen ohne neue Anfragen beheben lässt.",
                "Aus diesen Seiten baut Radix einen Datenstand, prüft ihn auf Widersprüche und gegen die Zahlen des vorigen und veröffentlicht ihn nur, wenn alles stimmt; sonst bleibt der vorige in Betrieb. Jede Angabe kennt ihre Quelle, und wo mehrere Quellen etwas dazu sagen, entscheidet eine feste Rangfolge.",
                "Wo die Quelle nichts sagt, steht „nicht angegeben“. Was Betula ableitet, etwa die Fakultät eines Studiengangs oder welche Bereiche eine Zeile des Plans meint, sagt es dazu. Wo die BTU selbst Unstimmiges einträgt, etwa Prüfungen um 01:00 Uhr nachts als Platzhalter für „nach Vereinbarung“, zeigt Betula „Zeit offen“ und nennt den Eintrag, wie er in QIS steht. Jede Modul- und Studiengangsseite verlinkt auf ihr Original bei der BTU.",
            ],
            more: &[],
        },
        devices: Chapter {
            title: "Für jedes Gerät, in zwei Sprachen",
            text: &[
                "Am Rechner stehen Filter, Liste und Vorschau nebeneinander, und ihre Breiten lassen sich ziehen. Am Handy liegt die Navigation unten, die Filter öffnen als Blatt zum Wegwischen, und durch die Woche des Stundenplans wischt man zwischen A- und B-Woche. Hell und dunkel folgen dem System oder deiner Wahl, und jedes Tastenkürzel steht neben seinem Knopf.",
                "Die Oberfläche spricht Deutsch und Englisch, jede Seite unter eigener Adresse; Titel und Beschreibungen der Module stehen, wie die BTU sie schreibt. Geteilte Links zeigen in Messengern eine Vorschau mit dem Titel und den wichtigsten Angaben des Moduls oder Studiengangs.",
            ],
            more: &[],
        },
        to_catalog: "Zum Modulkatalog",
        to_programs: "Zu den Studiengängen",
        to_studyplan: "Zum Stundenplan",
        to_privacy: "Zur Datenschutzerklärung",
    },
};

pub const EN: Texts = Texts {
    description: "Every module and degree programme of BTU Cottbus-Senftenberg in one place: search and filter, standard study plans from the examination regulations, your timetable with a calendar subscription. Free, no account, works offline. Unofficial, with a link to the original.",
    seo_title: "Betula · Module catalogue for BTU Cottbus-Senftenberg",
    alternate_names: ["Betula module catalogue", "Module catalogue of BTU Cottbus-Senftenberg (unofficial)"],
    browser_requirements: "Runs in any current browser, also without JavaScript. With JavaScript the app loads the catalogue into the browser as an SQLite database (WebAssembly), answers without loading times and works offline.",

    on_this_page: "On this page",
    overview: "Overview",
    ways_in_and_faculties: "Ways in and faculties",
    abilities: "What Betula does",
    questions: "Questions and answers",
    in_detail: "In detail",
    data: "Data",
    semester: "Semester",
    last_changed: "Last changed",
    source: "Source",
    source_title: "Module descriptions, degree programme pages and course catalogue of BTU Cottbus-Senftenberg",

    eyebrow: "Unofficial · for BTU Cottbus-Senftenberg",
    title_before: "Every module and degree programme of ",
    title_after: ", in one place.",
    lead: "Modules with their contents, prerequisites, assessment and dates, degree programmes with the standard study plan from their examination regulations, and your own timetable: searchable and filterable, instead of wading through module handbooks.",
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
    plan_slide: Slide { tab: "Standard study plan", title: "The standard study plan", action: "Choose a degree programme" },
    plan_text: "Semester by semester, as a matrix",
    plan_alt: "The standard study plan of Informatik B.Sc. as a matrix: modules by semester, with compulsory and compulsory elective modules",
    module_slide: Slide { tab: "Module", title: "A module", action: "See an example" },
    module_text: "Contents, exam and dates on one page",
    module_alt: "The page of the module Datenbanken: contents, learning outcomes, assessment and the weekly schedule of its dates",

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

    abilities_lead: "The same data as at BTU, prepared so that you can plan with it.",
    search: Ability {
        title: "One search box for all modules",
        text: "German and English titles and the module number in one search, across all faculties and on every page, with Ctrl+K or /. The results appear as you type.",
    },
    filters: Ability {
        title: "Filters that work together",
        text: "Degree programme, area, semester, offering, teaching form, assessment, credit points, language, location, lecturers and more, in any combination. Almost every one can be inverted (“everything but a written exam”), and every selection is a link.",
    },
    plans: Ability {
        title: "Study plans from the regulations",
        text: "Read from the PDFs of the examination and study regulations and checked against their own totals: semester by semester, with compulsory and elective modules and every study direction.",
    },
    timetable: Ability {
        title: "A timetable that thinks ahead",
        text: "Plan modules or import a whole semester of your programme: Betula shows the week, finds clashes and exams too close together, and looks for modules that still fit. As an .ics file or a calendar subscription.",
    },
    dates: Ability {
        title: "Dates from the course catalogue",
        text: "Lectures, exercises and exams as a weekly schedule with the module, with rooms and A and B weeks. Placeholders in QIS, such as an exam at 1 a.m., show as “Time TBA” instead of a wrong time.",
    },
    prerequisites: Ability {
        title: "Prerequisites to click on",
        text: "What a module requires, mandatory or recommended, leads straight to that module as a link, with the wording of the module description. A module that was replaced names its successor.",
    },
    account: Ability {
        title: "No account, nothing missing",
        text: "Saved modules, your timetable, saved plans and your programme live in your browser instead of on a server. Links take them to other devices, a subscription into your calendar. No sign-up, no ads, no tracking.",
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
    coming: "Study progress: tick off the modules you have passed, check prerequisites",

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
                "Betula has two parts, named after the birch. Radix, the root, reads BTU's public pages and the PDFs of its examination regulations, checks the result and publishes it as a data set: modules, degree programmes with their standard study plans and areas, the dates of the course catalogue. Folia, the leaves, is the web app you are looking at. Its server delivers every page as finished HTML; with JavaScript your browser loads the data set once as a database and from then on answers search and filters itself. Which versions are running is shown at the end of every page.",
            ),
            (
                "Where does the data come from?",
                "From BTU's public sources: the module descriptions, the course catalogue and the module tree of each degree programme in the QIS portal, the list of interdisciplinary studies (FÜS), and the examination and study regulations BTU publishes as PDFs. Betula organises the information and makes it searchable; nothing in the content is changed and nothing is added. Where the source says nothing, the page says “not stated”.",
            ),
            (
                "How up to date is the catalogue?",
                "Radix reads the sources continuously, in small steps and mostly at night: dates that are not settled yet every two hours, the course catalogue every night, module descriptions and degree programmes at least once a month. A new data set appears as soon as something has changed and it has passed every check; when that last happened is shown in the sidebar. Short-notice changes appear at BTU first, and if in doubt, the original applies.",
            ),
            (
                "Does Betula cost anything?",
                "No. Betula is free, without advertising and without tracking: no analytics, no cookies that recognise you, and nothing is loaded from other servers. It is a private, non-commercial project.",
            ),
            (
                "Why is there no account? Is anything missing without one?",
                "No. The account is left out on purpose, not the features: saved modules, a timetable with placeholders and chosen dates, saved plans, “My programme” and your settings all work without signing up, because they live in your browser instead of on a server. Nobody else can see them, and there is no password and no profile. Links take your saved modules and your timetable to another device, and a subscription takes the timetable into Apple Calendar, Google Calendar or Outlook.",
            ),
        ],
    },
    using_betula: Faq {
        name: "Using Betula",
        questions: &[
            (
                "How do I find the right module quickly?",
                "Type a title or a module number into the search at the top (Ctrl+K or / jumps there), or start with a filter: degree programme, area, semester, offering, form of assessment, credit points, language, location, lecturers and more. All filters can be combined and almost all inverted, for example into “everything but a written exam”. Every selection is part of the address, so you can share it, bookmark it and go back with the browser's Back button. The section “Betula in detail” at the end of this page explains every filter.",
            ),
            (
                "What can the timetable do?",
                "It shows one semester as a week. Modules come in with “Plan”, from your saved modules, or as a whole semester of your standard study plan, whose elective rows become placeholders. Betula counts clashes on the days on which dates really meet, with A and B weeks, and for exercises with several groups only when no group is free any more. It warns about exams that overlap or leave too little time to get from one site to the other, and “Fits my timetable” finds modules that still fit. You can take the plan along as an .ics file, as a calendar subscription that fetches changes by itself, or as a link to share.",
            ),
            (
                "Can I save modules?",
                "Yes. Every module has a bookmark, and the M key does the same. Your saved modules are counted with their credit points, split into winter and summer, can be copied as text and work as a filter in the catalogue (“Saved”). They are kept only in your browser: no account, and none of it reaches the server.",
            ),
            (
                "Can I use Betula on several devices?",
                "Yes, without an account. “Move to another device” copies a link that carries your saved modules behind the “#” of the address, and no browser sends that part to a server. You pass a timetable on with “Copy link to share”, to your other device or to other students, who can then add its modules to their own. And a calendar subscription shows your plan on every device your calendar runs on.",
            ),
            (
                "Does Betula work without JavaScript?",
                "Yes. Every page comes from the server as complete HTML: the catalogue with all its filters, module pages, degree programmes and standard study plans work without JavaScript, as a classic website, for text browsers and search engines alike. With JavaScript an app written in Rust and running as WebAssembly takes over: it loads the catalogue into your browser once as an SQLite database and from then on answers every search and every filter itself, without loading times between pages. What lives in your browser, your saved modules and your timetable, needs JavaScript.",
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
        ],
    },

    details: Details {
        heading: "Betula in detail",
        lead: "What Betula does, how it helps you find what you are looking for, and how it is built, at length and in one piece.",
        flow: Chapter {
            title: "From a question to the module",
            text: &[
                "Betula is built around the questions people bring to a module catalogue: what is due in the third semester? Which compulsory electives are taught in English? What still fits into my week? Each has a short way, and all of them end at the same module page: contents and learning outcomes, assessment, credit points, when it is offered, prerequisites, the dates as a weekly schedule, and the degree programmes the module belongs to, each with its semester according to the standard study plan.",
                "The search sits at the top of every page and finds modules by their German or English title and their number, across all faculties. The home page offers ready-made ways in with the number of their results, such as “In English” or “Without a written exam”, the degree programmes by faculty, and a map of which programmes share modules.",
                "From a degree programme, the standard study plan leads to its modules and “Electives & areas” to everything there is to choose from. “In the module catalogue” opens the catalogue narrowed down to exactly this programme, an area or a row of the plan. Pick a semester there, and besides the compulsory modules you see the compulsory electives the plan asks for in that semester.",
                "In the catalogue a click opens the preview beside the list, and the list, the filters and the scroll position stay as they were. F shows the module as a full page, Esc closes it, ↑ and ↓ move through the list, Enter opens, M saves. Every active filter stands above the list, where it can be removed on its own; when a combination finds nothing, the page suggests what to take back.",
                "Every view has its own address: the search, the filters, the order and the open module are part of the link. You can share it or bookmark it, and the browser's Back always leads to where you were before. Home, Modules, Study, Saved and Timetable remember where you left them, like tabs.",
            ],
            more: &[],
        },
        filters: Chapter {
            title: "Every filter of the module catalogue",
            text: &["All filters can be combined freely. Most have three steps: one click keeps only the modules with that property, a second one leaves them out, a third one clears the filter. Two clicks on “Written” therefore mean “everything but a written exam”. Leaving something out only removes what the data states: a module without the information stays in the list instead of silently disappearing."],
            more: &[
                "Lecturers, chair, duration, years, location and modules no longer offered are under “More filters”. “Reset” takes back all filters at once.",
                "On a phone the filters open as a sheet from below that you can swipe away. The list follows only once the sheet is closed, and its button already shows how many modules it will be. Without JavaScript the filters are links and form fields and work just the same.",
                "The overview of the degree programmes filters by degree (Bachelor, Master, Teacher training, Doctorate, Other), form of study (Dual, Double degree, Part-time & distance) and “With standard study plan”; its search finds any part of a name, whatever the case and with or without umlauts.",
            ],
        },
        filter: Filters {
            search: "German or English title or module number, or part of one; in the app the list filters as you type.",
            program: "Narrows the list down to one degree programme. The picker forgives typos and knows abbreviations and initials (“infomatik bsc”); “My programme” comes first.",
            list: "With a programme: its curriculum or its FÜS list, the interdisciplinary modules it accepts.",
            kind: "With a programme: compulsory, compulsory elective, thesis, internship or “Not stated”, as the standard study plan, the module description or the module tree says.",
            area: "With a programme: the areas of its module tree that students choose from, such as “Praktische Informatik” or a minor subject, each with everything below it; from a row of the plan, several at once.",
            plan_semester: "With a standard study plan: the semester it places a module in, or “?” for modules without one. A semester also lists the compulsory electives the plan asks for there, and says what they were derived from.",
            confirmed: "Only modules with published dates, the ones very likely to take place; excluded, only those without.",
            fits: "Only modules that fit into the free time of your timetable. You choose what is compared: lectures, exercises and exams. Lectures all have to be free, for exercises one free group is enough, for exams one date without a conflict; modules without dates come along only with “also without dates”.",
            offered_in: "Winter, summer or irregularly, as the module description states.",
            teaching_form: "Lecture, exercise, seminar, practical, project, excursion.",
            exam: "Written, oral, paper, presentation, project, practical; a module with several parts of assessment shows up under each of them.",
            credits: "A slider with two handles from 0 to 30 credit points, no upper limit at its right end, and the exact numbers below it to type in.",
            language: "German or English.",
            properties: "Graded, limited places, FÜS list and, in the app, saved: only your saved modules, or all the others.",
            lecturers: "One or more people responsible for or teaching a module, with a search. Wanted ones (+) mean “any of them”, left-out ones (×) “none of them”.",
            department: "The chair that offers a module, with a search.",
            duration: "1 semester or 2 semesters.",
            years: "Even or odd, for modules that only take place every other year.",
            location: "Central Campus Cottbus, Cottbus-Sachsendorf or Senftenberg, known for modules with rooms in the current semester.",
            not_offered: "Also modules BTU no longer offers; in the catalogue of a programme they are there anyway, as far as its curriculum names them.",
            sort_name: "Order",
            sort: "By title, credit points or number of dates, ascending or descending, with a click on the column head; within a programme in the order of its plan.",
        },
        plans: Chapter {
            title: "Standard study plans from the regulations",
            text: &[
                "Which modules are planned for which semester is stated at BTU only in the examination and study regulations, as tables in PDF files. Betula has downloaded these regulations and made their plans machine-readable. A parser of its own reads the semester columns from the geometry of the tables, cell by cell, including merged cells, ranges such as “10–24 CP” and modules over several semesters. A language model only helps to assign module numbers and kinds; whatever it says about semesters and credit points is replaced by the values of the cells.",
                "Every plan is checked against the totals the regulations print themselves and compared with the module catalogue. What is not clear is not guessed but set aside for a check by hand. A plan says when it was taken from the regulations and checked, and mostly on which page of them it stands. A row is linked to a module of the catalogue only where its number or title names it beyond doubt, and a programme with several study directions has a plan for each.",
            ],
            more: &["The plan is shown as a matrix of modules and semesters, as the regulations print it, or as a list, with the regulations' own totals. Rows that name no module, such as “Wahlpflichtmodul aus der Informatik”, lead to the areas they mean. “Electives & areas” shows the whole module tree of the programme, and every programme has its own list of interdisciplinary modules (FÜS)."],
        },
        plans_count: |plans, programs| format!("In the current data, Betula has a standard study plan read and checked this way for {plans} of {programs} degree programmes in their current examination regulations."),
        timetable: Chapter {
            title: "The timetable",
            text: &[
                "The timetable shows one semester as a week, with your modules in a column beside it. Modules come in with “Plan” from any module page, from your saved modules, or as a whole semester of the standard study plan; rows of the plan that leave a choice become placeholders, for which “Find a module” looks up matching modules in the catalogue.",
                "Betula counts clashes on the days on which dates really meet: with A and B weeks, dates in only part of the semester and cancelled days. Where an exercise has several groups, a clash only counts once no group is free any more, and you take dates or leave them out right in the week, with ✓ and ×. For exams, Betula warns when they overlap or leave too little time to get between Cottbus and Senftenberg, or between two sites in Cottbus.",
                "With “Fits my timetable” the catalogue lists only modules that still fit in. The plan comes as an .ics file and as a calendar subscription for Apple Calendar, Google Calendar or Outlook that fetches changed dates by itself. Several plans can be saved under names of their own, and “Copy link to share” passes a plan on: the preview in a messenger shows its modules, and whoever opens the link can add them to their own.",
                "“My programme” remembers your degree programme with its study direction, start of studies and location; the catalogue and the timetable then start in it.",
            ],
            more: &[],
        },
        account: Chapter {
            title: "No account, and nothing missing",
            text: &[
                "Betula leaves out accounts on purpose, not features. Saved modules, the timetable, saved plans, calendar subscriptions, “My programme” and settings such as language and colour scheme all work without signing up, because they live in your browser instead of on a server. Nobody else can see them, there is no password to lose and no profile of you.",
                "What an account would do, links do. “Move to another device” copies a link that carries your saved modules behind the “#”, and no browser sends that part of an address to a server. A timetable moves to another device or to other students with “Copy link to share”, and a calendar subscription shows it wherever your calendar runs.",
                "Betula uses no analytics and no cookies that recognise you, and loads nothing from other servers: fonts, icons and scripts come from betula.app itself. The server stands in a data centre in Germany and deletes its access log after 7 days.",
            ],
            more: &[],
        },
        javascript: Chapter {
            title: "With and without JavaScript, online and offline",
            text: &[
                "Every page of Betula comes from the server as complete HTML. Without JavaScript Betula is a classic website: the catalogue turns pages, filters are links and form fields, and every module, degree programme and standard study plan is there, for search engines and text browsers as much as for anyone who has switched JavaScript off. The server builds every page once per data set and then delivers it from memory.",
                "With JavaScript the app takes over, written in Rust and running as WebAssembly in the browser. It loads the catalogue once as an SQLite database, a few megabytes compressed, and from then on answers every search and every filter itself, mostly within a few milliseconds: no loading times between pages, a list that filters as you type, and a click that shows at once. The app shows the same pages as the server, built from the same parts, just without the trip through the network.",
                "After that Betula also works offline: a service worker keeps the app ready, the catalogue is kept in your browser, and on a phone, tablet or computer Betula installs like an app. The app downloads new data in the background and uses it from the next start.",
            ],
            more: &[],
        },
        data: Chapter {
            title: "Where the data comes from and how it is checked",
            text: &[
                "Radix, the part of Betula that collects the data, reads BTU's public sources: the module descriptions, the course catalogue with its dates and rooms and the module tree of every degree programme in the QIS portal, the list of interdisciplinary studies and the examination regulations. Radix asks politely, with pauses between requests and mostly at night, and keeps the pages it has read, so that a mistake in reading one can be fixed without asking again.",
                "From these pages Radix builds a data set, checks it for contradictions and against the figures of the previous one, and publishes it only when everything adds up; otherwise the previous one stays in service. Every piece of information knows its source, and where several sources say something about it, a fixed order of precedence decides.",
                "Where the source says nothing, the page says “not stated”. What Betula derives, such as the faculty of a programme or which areas a row of the plan means, it says so. Where BTU itself enters something odd, such as exams at 1 a.m. as placeholders for “by arrangement”, Betula shows “Time TBA” and names the entry as it stands in QIS. Every module and programme page links to its original at BTU.",
            ],
            more: &[],
        },
        devices: Chapter {
            title: "On every device, in two languages",
            text: &[
                "On a computer, filters, list and preview stand side by side, and their widths can be dragged. On a phone the navigation sits at the bottom, the filters open as a sheet you swipe away, and in the timetable's week you swipe between the A and B weeks. Light and dark follow your system or your choice, and every keyboard shortcut is written next to its button.",
                "The interface speaks German and English, every page at its own address; module titles and descriptions are shown as BTU writes them. Shared links show a preview in messengers, with the title and the key facts of the module or degree programme.",
            ],
            more: &[],
        },
        to_catalog: "To the module catalogue",
        to_programs: "To the degree programmes",
        to_studyplan: "To the timetable",
        to_privacy: "To the privacy notice",
    },
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n;
    use catalog::filter::{ExamPart, Language};
    use catalog::labels::{Campus, Labelled, ModuleKind, TeachingForm};
    use catalog::url::{FormGroup, LevelGroup, ProgramTab};
    use catalog::Locale;

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

    /// What the answers and „Betula im Detail" quote of the page (a button, a view, a filter, a
    /// way in) is what the page says there, in every language: renamed there, it has to be
    /// renamed here.
    #[test]
    fn the_texts_quote_the_page_as_it_is() {
        for locale in Locale::ALL {
            let t = i18n::texts(*locale);
            let (open, close) = if *locale == Locale::De { ("„", "“") } else { ("“", "”") };
            let quoted = |label: &str| format!("{open}{label}{close}");
            let home = &t.home;
            let answer =|question: usize, faq: &Faq| faq.questions.get(question).map(|(_, answer)| *answer).unwrap_or_default();
            let details = &home.details;
            let chapter = |chapter: &Chapter| [chapter.text, chapter.more].concat().join(" ");
            let areas = ProgramTab::Areas.label(*locale);
            let pairs: Vec<(String, String)> = vec![
                (answer(0, &home.using_betula).to_string(), quoted(details.heading)),
                (answer(1, &home.using_betula).to_string(), quoted(t.studyplan_head.plan)),
                (answer(1, &home.using_betula).to_string(), quoted(t.catalog.fits)),
                (answer(2, &home.using_betula).to_string(), quoted(t.catalog.saved_chip)),
                (answer(3, &home.using_betula).to_string(), quoted(t.bookmarks.transfer)),
                (answer(3, &home.using_betula).to_string(), quoted(t.studyplan_share.copy_link)),
                (answer(0, &home.for_studies).to_string(), quoted(t.program.in_catalog)),
                (answer(0, &home.for_studies).to_string(), quoted(t.myprogram.mine)),
                (answer(3, &home.for_studies).to_string(), quoted(areas)),
                (answer(3, &home.for_studies).to_string(), quoted(t.catalog.area)),
                (answer(4, &home.for_studies).to_string(), quoted(t.catalog.confirmed)),
                (answer(7, &home.about_betula).to_string(), quoted(t.myprogram.mine)),
                (chapter(&details.flow), quoted(home.english.label)),
                (chapter(&details.flow), quoted(home.no_written_exam.label)),
                (chapter(&details.flow), quoted(areas)),
                (chapter(&details.flow), quoted(t.program.in_catalog)),
                (chapter(&details.filters), quoted(t.catalog.more_filters)),
                (chapter(&details.filters), quoted(t.common.reset)),
                (chapter(&details.filters), quoted(t.programs.with_plan)),
                (chapter(&details.filters), quoted(ExamPart::Written.short_label(*locale))),
                (chapter(&details.plans), quoted(areas)),
                (chapter(&details.timetable), quoted(t.studyplan_head.plan)),
                (chapter(&details.timetable), quoted(&t.studyplan_modules.find_module.replace('\u{a0}', " "))),
                (chapter(&details.timetable), quoted(t.catalog.fits)),
                (chapter(&details.timetable), quoted(t.studyplan_share.copy_link)),
                (chapter(&details.timetable), quoted(t.myprogram.mine)),
                (chapter(&details.account), quoted(t.bookmarks.transfer)),
                (chapter(&details.account), quoted(t.studyplan_share.copy_link)),
                (chapter(&details.account), quoted(t.myprogram.mine)),
                (details.filter.program.to_string(), quoted(t.catalog.my_program)),
                (details.filter.kind.to_string(), quoted(t.catalog.not_stated)),
                (details.filter.fits.to_string(), quoted(t.catalog.undated)),
            ];
            for (text, label) in pairs {
                assert!(text.contains(&label), "{locale:?}: {label} is not in: {text}");
            }
        }
    }

    /// „Betula im Detail" lists the values of each filter as the panel offers them (compared
    /// without case: running text writes „Vorlesung" or "lecture" as the sentence needs).
    #[test]
    fn the_filters_are_listed_as_the_panel_offers_them() {
        for locale in Locale::ALL {
            let t = i18n::texts(*locale);
            let f = &t.home.details.filter;
            let filters = &t.home.details.filters;
            let after = filters.more.join(" ");
            let has = |text: &str, value: &str| text.to_lowercase().contains(&value.to_lowercase());
            let listed: Vec<(&str, Vec<String>)> = vec![
                (f.kind, [ModuleKind::Compulsory, ModuleKind::Elective, ModuleKind::Thesis, ModuleKind::Internship].iter().map(|k| k.label(*locale).to_string()).collect()),
                (f.fits, vec![t.catalog.lectures.to_string(), t.catalog.exercises.to_string(), t.catalog.exams.to_string()]),
                (f.offered_in, vec![t.catalog.winter_chip.to_string(), t.catalog.summer_chip.to_string(), t.catalog.irregular_chip.to_string()]),
                (f.teaching_form, [TeachingForm::Lecture, TeachingForm::Exercise, TeachingForm::Seminar, TeachingForm::Practical, TeachingForm::Project, TeachingForm::Excursion].iter().map(|form| form.label(*locale).to_string()).collect()),
                (f.exam, ExamPart::ALL.iter().map(|part| part.short_label(*locale).to_string()).collect()),
                (f.language, Language::ALL.iter().map(|language| language.label(*locale).to_string()).collect()),
                (f.properties, vec![t.catalog.graded_chip.to_string(), t.catalog.limited_chip.to_string(), t.catalog.fues_list.to_string(), t.catalog.saved_chip.to_string()]),
                (f.duration, vec![(t.catalog.semesters)(1), (t.catalog.semesters)(2)]),
                (f.years, vec![t.catalog.even.to_string(), t.catalog.odd.to_string()]),
                (f.location, [Campus::Zentralcampus, Campus::Sachsendorf, Campus::Senftenberg].iter().map(|campus| campus.label(*locale).to_string()).collect()),
                (after.as_str(), LevelGroup::ALL.iter().map(|level| level.label(*locale).to_string()).collect()),
                (after.as_str(), FormGroup::ALL.iter().map(|form| form.label(*locale).to_string()).collect()),
            ];
            for (text, values) in listed {
                for value in values {
                    assert!(has(text, &value), "{locale:?}: {value} is not in: {text}");
                }
            }
        }
    }
}
