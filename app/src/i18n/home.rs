//! Texts of the start page (`pages/home.rs`), in the order of the page: what search engines read,
//! the sidebar, the first panel, the pictures and the map, the ways into the catalog, what Betula
//! does, and the questions.

pub struct Texts {
    /// The page's description for search engines and link previews.
    pub description: &'static str,
    /// Its title for link previews (the tab shows the site's own title, `app::default_title`).
    pub seo_title: &'static str,
    /// Other names of the site, as the structured data gives them („Betula Modulkatalog").
    pub alternate_names: [&'static str; 2],

    // The sidebar
    pub on_this_page: &'static str,
    /// The sections of the page, as the sidebar lists them. The last two are also the headings
    /// of their sections.
    pub overview: &'static str,
    pub ways_in_and_faculties: &'static str,
    pub abilities: &'static str,
    pub questions: &'static str,
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
    pub study_plan: Ability,
    pub prerequisites: Ability,
    pub dates: Ability,
    pub gaps: Ability,
    /// „In Arbeit", and what is.
    pub in_progress: &'static str,
    pub coming: &'static str,

    // The questions
    pub about_betula: Faq,
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
    description: "Alle Module und Studiengänge der BTU Cottbus-Senftenberg an einem Ort: durchsuchen, nach Turnus, Prüfung, Sprache und Studiengang filtern, Regelstudienpläne und Voraussetzungen ansehen. Inoffiziell, kostenlos, mit Link zum Original.",
    seo_title: "Betula · Modulkatalog für die BTU Cottbus-Senftenberg",
    alternate_names: ["Betula Modulkatalog", "Modulkatalog BTU Cottbus-Senftenberg (inoffiziell)"],

    on_this_page: "Auf dieser Seite",
    overview: "Überblick",
    ways_in_and_faculties: "Einstiege und Fakultäten",
    abilities: "Was Betula kann",
    questions: "Fragen und Antworten",
    data: "Datenstand",
    semester: "Semester",
    last_changed: "Zuletzt geändert",
    source: "Quelle",
    source_title: "Modulbeschreibungen, Studiengangsseiten und Vorlesungsverzeichnis der BTU Cottbus-Senftenberg",

    eyebrow: "Inoffiziell · für die BTU Cottbus-Senftenberg",
    title_before: "Alle Module und Studiengänge der ",
    title_after: ", an einem Ort.",
    lead: "Module mit Inhalten, Voraussetzungen, Prüfungsform und Terminen, Studiengänge mit Regelstudienplan: durchsuchbar und filterbar, statt Modulhandbücher zu wälzen.",
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
        text: "Deutscher und englischer Titel und die Modulnummer in einer Suche, über alle Fakultäten. Die Treffer stehen da, während du tippst.",
    },
    filters: Ability {
        title: "Filter, die zusammenpassen",
        text: "Studiengang, Turnus, Lehrform, Prüfungsform, Sprache, Campus, Leistungspunkte, Dozierende. Viele lassen sich auch umkehren: „alles außer Klausur“.",
    },
    study_plan: Ability {
        title: "Der Regelstudienplan als Plan",
        text: "Semester für Semester, mit Pflicht- und Wahlpflichtbereichen, der FÜS-Liste und den Ordnungen des Studiengangs.",
    },
    prerequisites: Ability {
        title: "Voraussetzungen zum Anklicken",
        text: "Was ein Modul voraussetzt und wofür es selbst Voraussetzung ist, führt direkt zum nächsten Modul.",
    },
    dates: Ability {
        title: "Termine aus dem Vorlesungsverzeichnis",
        text: "Vorlesungen, Übungen und Prüfungen als Wochenplan beim Modul und in deinem Studienplan, auch als Kalender-Abo.",
    },
    gaps: Ability {
        title: "Ehrlich bei Lücken",
        text: "Wo die Quelle nichts sagt, steht „nicht angegeben“ und keine Vermutung. Jedes Modul verlinkt auf sein Original bei der BTU.",
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
                "Betula hat zwei Teile. Radix liest die öffentlichen Seiten der BTU ein und baut daraus einen Datenstand: Module, Studiengänge mit ihren Ordnungen und Regelstudienplänen, die Termine des Vorlesungsverzeichnisses. Folia ist die Web-App, die du gerade siehst: Sie lädt den Datenstand einmal in deinen Browser, danach laufen Suche und Filter direkt bei dir. Welche Versionen gerade laufen, steht in der Seitenleiste.",
            ),
            (
                "Woher kommen die Daten?",
                "Aus den öffentlichen Modulbeschreibungen, den Studiengangsseiten mit ihren Prüfungs- und Studienordnungen und dem Vorlesungsverzeichnis der BTU. Betula ordnet sie und macht sie durchsuchbar; am Inhalt wird nichts geändert und nichts ergänzt. Wo die Quelle nichts sagt, steht „nicht angegeben“.",
            ),
            (
                "Wie aktuell ist der Katalog?",
                "Der Datenstand steht in der Seitenleiste dieser Seite und wird mit jedem Einlesen erneuert. Kurzfristige Änderungen, etwa verlegte Termine, stehen zuerst bei der BTU; im Zweifel gilt das Original.",
            ),
            (
                "Kostet das etwas, brauche ich ein Konto?",
                "Nein. Betula ist kostenlos, ohne Anmeldung und ohne Werbung. Der Katalog wird einmal in deinen Browser geladen; danach antworten Suche und Filter ohne Wartezeit.",
            ),
            (
                "Kann ich mir Module merken?",
                "Ja. Jedes Modul hat ein Lesezeichen, die Taste M tut dasselbe. Die Merkliste liegt nur in deinem Browser: kein Konto, und nichts davon erreicht den Server. Ein Link bringt sie auf ein anderes Gerät.",
            ),
        ],
    },
    for_studies: Faq {
        name: "Fürs Studium",
        questions: &[
            (
                "Wie finde ich die Module für mein Studium?",
                "Über deinen Studiengang. Welche Module du belegst, legt seine Studien- und Prüfungsordnung fest; Betula bereitet sie auf und bündelt alles an einem Ort. Unter „Studiengang wählen“ findest du den Regelstudienplan Semester für Semester (wo die Ordnung einen enthält), die Wahlpflichtbereiche und alle Module des Studiengangs. Ein Klick auf ein Modul zeigt Inhalte, Termine und Prüfung.",
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
                "Pflichtmodule belegen alle im Studiengang. Bei Wahlpflichtmodulen wählst du aus einem Bereich, den die Ordnung festlegt, bis die geforderten Leistungspunkte erreicht sind. Betula zeigt die Bereiche jedes Studiengangs unter „Wahlpflicht & Bereiche“.",
            ),
            (
                "Wann wird ein Modul angeboten?",
                "Das sagt der Turnus: im Wintersemester, im Sommersemester oder in jedem Semester. Im Katalog kannst du danach filtern, und die Termine des aktuellen Semesters stehen als Wochenplan beim Modul.",
            ),
            (
                "Wo melde ich mich für Module und Prüfungen an?",
                "Nicht bei Betula. Anmeldungen laufen über die Systeme der BTU; Fristen und Regeln stehen dort und in deiner Prüfungsordnung. Betula hilft beim Planen und verlinkt jedes Modul auf sein Original.",
            ),
        ],
    },
};

pub const EN: Texts = Texts {
    description: "Every module and degree programme of BTU Cottbus-Senftenberg in one place: search, filter by semester offered, exam, language and degree programme, and see standard study plans and prerequisites. Unofficial, free, with a link to the original.",
    seo_title: "Betula · Module catalogue for BTU Cottbus-Senftenberg",
    alternate_names: ["Betula module catalogue", "Module catalogue of BTU Cottbus-Senftenberg (unofficial)"],

    on_this_page: "On this page",
    overview: "Overview",
    ways_in_and_faculties: "Ways in and faculties",
    abilities: "What Betula does",
    questions: "Questions and answers",
    data: "Data",
    semester: "Semester",
    last_changed: "Last changed",
    source: "Source",
    source_title: "Module descriptions, degree programme pages and course catalogue of BTU Cottbus-Senftenberg",

    eyebrow: "Unofficial · for BTU Cottbus-Senftenberg",
    title_before: "Every module and degree programme of ",
    title_after: ", in one place.",
    lead: "Modules with their contents, prerequisites, form of assessment and dates, degree programmes with their standard study plan: searchable and filterable, instead of wading through module handbooks.",
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
        text: "German and English titles and the module number in one search, across all faculties. The results appear as you type.",
    },
    filters: Ability {
        title: "Filters that work together",
        text: "Degree programme, semester offered, form of teaching, form of assessment, language, campus, credit points, lecturers. Many can also be inverted: “everything but a written exam”.",
    },
    study_plan: Ability {
        title: "The standard study plan as a plan",
        text: "Semester by semester, with compulsory and compulsory elective areas, the FÜS list and the regulations of the degree programme.",
    },
    prerequisites: Ability {
        title: "Prerequisites to click on",
        text: "What a module requires, and what it is itself a prerequisite for, leads straight to the next module.",
    },
    dates: Ability {
        title: "Dates from the course catalogue",
        text: "Lectures, exercises and exams as a weekly schedule with the module and in your timetable, also as a calendar subscription.",
    },
    gaps: Ability {
        title: "Honest about gaps",
        text: "Where the source says nothing, the page says “not stated” and makes no guess. Every module links to its original at BTU.",
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
                "Betula has two parts. Radix reads BTU's public pages and builds a data set from them: modules, degree programmes with their regulations and standard study plans, the dates of the course catalogue. Folia is the web app you are looking at: it loads the data set into your browser once, and after that search and filters run right on your device. Which versions are running is shown in the sidebar.",
            ),
            (
                "Where does the data come from?",
                "From BTU's public module descriptions, the pages of its degree programmes with their examination and study regulations, and its course catalogue. Betula organises it and makes it searchable; nothing in the content is changed and nothing is added. Where the source says nothing, the page says “not stated”.",
            ),
            (
                "How up to date is the catalogue?",
                "The date of the data is in the sidebar of this page, and the data is renewed every time it is read in. Short-notice changes, such as moved dates, appear at BTU first; if in doubt, the original applies.",
            ),
            (
                "Does it cost anything, do I need an account?",
                "No. Betula is free, without sign-up and without advertising. The catalogue is loaded into your browser once; after that, search and filters answer without waiting.",
            ),
            (
                "Can I save modules?",
                "Yes. Every module has a bookmark, and the M key does the same. Your saved modules are kept only in your browser: no account, and none of it reaches the server. A link takes them to another device.",
            ),
        ],
    },
    for_studies: Faq {
        name: "For your studies",
        questions: &[
            (
                "How do I find the modules for my studies?",
                "Through your degree programme. Which modules you take is set by its study and examination regulations; Betula prepares them and brings everything together in one place. Under “Choose a degree programme” you will find the standard study plan semester by semester (where the regulations contain one), the compulsory elective areas and all modules of the programme. A click on a module shows its contents, dates and exam.",
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
                "Everyone in the degree programme takes the compulsory modules. For compulsory electives you choose from an area set by the regulations until you have the required credit points. Betula shows the areas of every degree programme under “Electives & areas”.",
            ),
            (
                "When is a module offered?",
                "Its description says when it is offered: in the winter semester, in the summer semester or every semester. You can filter by it in the catalogue, and the dates of the current semester are shown with the module as a weekly schedule.",
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
        for (de, en) in [(&DE.about_betula, &EN.about_betula), (&DE.for_studies, &EN.for_studies)] {
            assert_eq!(de.questions.len(), en.questions.len(), "{}", de.name);
        }
        for (t, open, close) in [(&DE, "„", "“"), (&EN, "“", "”")] {
            let first = t.for_studies.questions.first().map(|(_, answer)| *answer).unwrap_or_default();
            assert!(first.contains(&format!("{open}{}{close}", t.choose_program)), "{first}");
        }
        assert_eq!(((DE.slide_of)(2, 4, "Der Katalog"), (EN.in_semester)("Winter 2026/27")), ("2 von 4: Der Katalog".to_string(), "in Winter 2026/27".to_string()));
    }
}
