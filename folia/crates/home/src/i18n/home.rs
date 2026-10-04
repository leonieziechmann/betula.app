//! Texts of the start page (`pages/home/mod.rs`), in the order of the page: what search engines
//! read, the first panel, the way in for a first visit (`pages/home/start.rs`), the pictures and
//! the map, the ways into the catalog, what Betula does, and the questions. „Betula im Detail" at
//! the end has a group of its own (`home_detail`).
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

    // The way in for a first visit
    /// „So legst du los", and the line under it.
    pub start_heading: &'static str,
    pub start_lead: &'static str,
    /// The way straight into the search at the top (its title says the keys).
    pub search_now: &'static str,
    pub search_now_title: &'static str,
    /// The three steps, in the order a semester is planned, each with one button. The first one's
    /// is „Studiengang wählen" or „Alle Studiengänge" (`choose_program`, `all_programs`).
    pub step_program: Step,
    pub step_modules: Step,
    pub step_timetable: Step,
    /// The second step's button: the catalog of „Mein Studiengang", else the whole catalog.
    pub mine_modules: &'static str,
    pub to_catalog: &'static str,
    /// The third step's button: the Stundenplan with the Regelstudienplan of „Mein Studiengang"
    /// ready to take over, else the Stundenplan.
    pub take_semester: &'static str,
    pub to_timetable: &'static str,
    /// What a step the visitor has done says in place of where the navigation keeps it: the
    /// program, „5 Module gemerkt", „8 Module eingeplant".
    pub marked: fn(usize) -> String,
    pub planned: fn(usize) -> String,
    /// For screen readers, before the name of a step that is done.
    pub step_done: &'static str,
    /// „Jederzeit unter": before the item of the navigation that keeps a step.
    pub step_where: &'static str,
    /// „Erst einmal verstehen, was Betula ist?", before the ways to the sections that say so.
    pub learn_first: &'static str,
    /// The name of the steps together, for screen readers.
    pub steps_label: &'static str,

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
    /// „Was Betula kann" and „Fragen und Antworten": the headings of their sections, and the
    /// names of the ways to them.
    pub abilities: &'static str,
    pub questions: &'static str,
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

}

/// A picture of the carousel: its tab, its title, and what a click on it does.
pub struct Slide {
    pub tab: &'static str,
    pub title: &'static str,
    pub action: &'static str,
}

/// A step of the way in: its name, and in a line what it does.
pub struct Step {
    pub title: &'static str,
    pub text: &'static str,
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
    description: "Alle Module und Studiengänge der BTU Cottbus-Senftenberg an einem Ort: durchsuchen und filtern, Regelstudienpläne aus den Prüfungsordnungen, dein Stundenplan mit Kalender-Abo. Kostenlos, ohne Konto, auch offline. Inoffiziell, mit Link zum Original.",
    seo_title: "Betula · Modulkatalog für die BTU Cottbus-Senftenberg",
    alternate_names: ["Betula Modulkatalog", "Modulkatalog BTU Cottbus-Senftenberg (inoffiziell)"],
    browser_requirements: "Läuft in jedem aktuellen Browser, auch ohne JavaScript. Mit JavaScript lädt die App den Katalog als SQLite-Datenbank in den Browser (WebAssembly), antwortet ohne Ladezeiten und funktioniert offline.",

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

    start_heading: "So legst du los",
    start_lead: "Drei Schritte, jeder ein Klick. Wer nur ein bestimmtes Modul sucht, tippt Titel oder Nummer oben in die Suche.",
    search_now: "Direkt suchen",
    search_now_title: "Springt in die Suche oben (Strg+K oder /)",
    step_program: Step {
        title: "Studiengang wählen",
        text: "Einmal als „Mein Studiengang“ gewählt, sind Katalog und Stundenplan auf ihn eingestellt.",
    },
    step_modules: Step {
        title: "Module finden und merken",
        text: "Filter für Turnus, Prüfung oder Sprache; das Lesezeichen setzt ein Modul auf deine Merkliste.",
    },
    step_timetable: Step {
        title: "Stundenplan bauen",
        text: "Ein ganzes Fachsemester übernehmen, Überschneidungen sehen, alles in den Kalender holen.",
    },
    mine_modules: "Module deines Studiengangs",
    to_catalog: "Zum Katalog",
    take_semester: "Fachsemester übernehmen",
    to_timetable: "Zum Stundenplan",
    marked: |n| if n == 1 { "1 Modul gemerkt".to_string() } else { format!("{n} Module gemerkt") },
    planned: |n| if n == 1 { "1 Modul eingeplant".to_string() } else { format!("{n} Module eingeplant") },
    step_done: "Erledigt: ",
    step_where: "Jederzeit unter",
    learn_first: "Erst einmal verstehen, was Betula ist?",
    steps_label: "Die drei Schritte",

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

    abilities: "Was Betula kann",
    questions: "Fragen und Antworten",
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
                "Betula hat zwei Teile, benannt nach der Birke. Radix, die Wurzel, liest die öffentlichen Seiten der BTU und die PDFs ihrer Prüfungsordnungen ein, prüft das Ergebnis und veröffentlicht es als Datenstand: Module, Studiengänge mit Regelstudienplänen und Bereichen, die Termine des Vorlesungsverzeichnisses. Folia, die Blätter, ist die Web-App, die du gerade siehst. Ihr Server liefert jede Seite als fertiges HTML; mit JavaScript lädt dein Browser den Datenstand einmal als Datenbank und beantwortet danach Suche und Filter selbst. Welche Versionen gerade laufen, steht am Ende jeder Seite.",
            ),
            (
                "Woher kommen die Daten?",
                "Aus öffentlichen Quellen der BTU: den Modulbeschreibungen, dem Vorlesungsverzeichnis und dem Modulbaum der Studiengänge im Portal QIS, der Liste des Fachübergreifenden Studiums und den Prüfungs- und Studienordnungen, die die BTU als PDF veröffentlicht. Betula ordnet die Angaben und macht sie durchsuchbar; am Inhalt wird nichts geändert und nichts ergänzt. Wo die Quelle nichts sagt, steht „nicht angegeben“.",
            ),
            (
                "Wie aktuell ist der Katalog?",
                "Radix liest die Quellen laufend nach, in kleinen Schritten und das meiste nachts: Termine, die noch nicht feststehen, alle zwei Stunden, das Vorlesungsverzeichnis jede Nacht, Modulbeschreibungen und Studiengänge mindestens einmal im Monat. Ein neuer Datenstand erscheint, sobald sich etwas geändert und er alle Prüfungen bestanden hat; wann das zuletzt war, steht am Ende jeder Seite. Kurzfristige Änderungen stehen zuerst bei der BTU, und im Zweifel gilt das Original.",
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

};

pub const EN: Texts = Texts {
    description: "Every module and degree programme of BTU Cottbus-Senftenberg in one place: search and filter, standard study plans from the examination regulations, your timetable with a calendar subscription. Free, no account, works offline. Unofficial, with a link to the original.",
    seo_title: "Betula · Module catalogue for BTU Cottbus-Senftenberg",
    alternate_names: ["Betula module catalogue", "Module catalogue of BTU Cottbus-Senftenberg (unofficial)"],
    browser_requirements: "Runs in any current browser, also without JavaScript. With JavaScript the app loads the catalogue into the browser as an SQLite database (WebAssembly), answers without loading times and works offline.",

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

    start_heading: "How to get started",
    start_lead: "Three steps, one click each. Looking for one particular module? Type its title or number into the search at the top.",
    search_now: "Search now",
    search_now_title: "Jumps into the search at the top (Ctrl+K or /)",
    step_program: Step {
        title: "Choose a degree programme",
        text: "Chosen once as “My programme”, it sets up the catalogue and your timetable.",
    },
    step_modules: Step {
        title: "Find and save modules",
        text: "Filters for semester, assessment or language; the bookmark puts a module on your saved list.",
    },
    step_timetable: Step {
        title: "Build your timetable",
        text: "Take over a whole semester of your plan, see clashes, get it all into your calendar.",
    },
    mine_modules: "Modules of your programme",
    to_catalog: "To the catalogue",
    take_semester: "Take over a semester",
    to_timetable: "To the timetable",
    marked: |n| if n == 1 { "1 module saved".to_string() } else { format!("{n} modules saved") },
    planned: |n| if n == 1 { "1 module planned".to_string() } else { format!("{n} modules planned") },
    step_done: "Done: ",
    step_where: "Always under",
    learn_first: "Want to understand what Betula is first?",
    steps_label: "The three steps",

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

    abilities: "What Betula does",
    questions: "Questions and answers",
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
                "Betula has two parts, named after the birch. Radix, the root, reads BTU's public pages and the PDFs of its examination regulations, checks the result and publishes it as a data set: modules, degree programmes with their standard study plans and areas, the dates of the course catalogue. Folia, the leaves, is the web app you are looking at. Its server delivers every page as finished HTML; with JavaScript your browser loads the data set once as a database and from then on answers search and filters itself. Which versions are running is shown at the end of every page.",
            ),
            (
                "Where does the data come from?",
                "From BTU's public sources: the module descriptions, the course catalogue and the module tree of each degree programme in the QIS portal, the list of interdisciplinary studies (FÜS), and the examination and study regulations BTU publishes as PDFs. Betula organises the information and makes it searchable; nothing in the content is changed and nothing is added. Where the source says nothing, the page says “not stated”.",
            ),
            (
                "How up to date is the catalogue?",
                "Radix reads the sources continuously, in small steps and mostly at night: dates that are not settled yet every two hours, the course catalogue every night, module descriptions and degree programmes at least once a month. A new data set appears as soon as something has changed and it has passed every check; when that last happened is shown at the end of every page. Short-notice changes appear at BTU first, and if in doubt, the original applies.",
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

}
