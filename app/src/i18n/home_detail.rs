//! Texts of „Betula im Detail", the chapters at the end of the start page (`pages/home/detail.rs`):
//! a chapter per feature, each a name, a headline, a line under it and four points, and the words
//! of its picture. Where a picture shows a part of the app (a button, a filter, a chip), the page
//! takes its words from that part's own group (`catalog`, `studyplan_export` …), so that they are
//! the same; what stands here is only what no other part says.

pub struct Texts {
    pub heading: &'static str,
    pub lead: &'static str,
    /// The row of links to the chapters under the heading, for screen readers.
    pub chapters: &'static str,

    // From a question to the module
    pub flow: Chapter,
    /// The questions people come with, one per way; the steps of each way are the app's own
    /// words, and these two are the only ones it has none for.
    pub questions: [&'static str; 4],
    pub as_you_type: &'static str,
    pub third_semester: &'static str,
    /// Where every way ends, and what stands on it.
    pub module_page: &'static str,
    pub module_parts: [&'static str; 5],

    // Every filter
    pub filters: Chapter,
    /// The three steps of a chip, under the three examples: off, with, without.
    pub steps: [&'static str; 3],
    /// „Diese Auswahl im Katalog: 23 Module": the example the board shows chosen, with the number
    /// the catalog has for it (written already).
    pub example: fn(&str) -> String,
    pub hints: Hints,

    // The study plans from the regulations
    pub plans: Chapter,
    /// Under the number of programs with a checked plan: what it counts, and of how many
    /// (written already).
    pub plans_figure: &'static str,
    pub plans_of: fn(&str) -> String,
    /// The two sheets of the picture: the regulation's PDF, and the plan in Betula, with its rows
    /// and the line of the sums.
    pub pdf: &'static str,
    pub in_betula: &'static str,
    pub plan_rows: [&'static str; 5],
    pub plan_sum: &'static str,

    // The Stundenplan
    pub timetable: Chapter,
    /// Over the week of the picture, and the names of its modules (the kinds are the app's).
    pub example_week: &'static str,
    pub week_modules: [&'static str; 4],

    // No account
    pub account: Chapter,
    /// Four figures: the number and what it counts.
    pub zeros: [(&'static str, &'static str); 4],
    /// Where things live: in the browser (the Merkliste, the Stundenplan and „Mein Studiengang"
    /// are named as the app names them) …
    pub in_browser: &'static str,
    pub saved_plans: &'static str,
    pub settings: &'static str,
    pub catalog_copy: &'static str,
    /// … on the server …
    pub on_server: &'static str,
    pub for_everyone: &'static str,
    pub nothing_of_yours: &'static str,
    /// … and the link that carries the Merkliste between them.
    pub after_hash: &'static str,

    // With and without JavaScript
    pub javascript: Chapter,
    pub without_js: Mode,
    pub with_js: Mode,

    // Where the data comes from
    pub data: Chapter,
    /// The birch: the leaves, the trunk and the root, each a name and a line; the sources in the
    /// ground, each with how often it is read.
    pub folia: (&'static str, &'static str),
    pub trunk: (&'static str, &'static str),
    pub radix: (&'static str, &'static str),
    pub sources: [(&'static str, &'static str); 6],

    // Devices and languages
    pub devices: Chapter,
    /// What the phone's screenshot shows, for those who cannot see it (the desktop's is the
    /// carousel's `catalog_alt`).
    pub phone_alt: &'static str,

    /// The ways on at the end of a chapter.
    pub to_catalog: &'static str,
    pub to_programs: &'static str,
    pub to_studyplan: &'static str,
    pub to_privacy: &'static str,
}

/// A chapter: its name (over its headline, and in the row of links to the chapters), the headline,
/// the line under it, and four points, each a few words and a sentence.
pub struct Chapter {
    pub name: &'static str,
    pub title: &'static str,
    pub lead: &'static str,
    pub points: [Point; 4],
}

pub struct Point {
    pub title: &'static str,
    pub text: &'static str,
}

/// A column of the comparison with and without JavaScript: its name and what one gets.
pub struct Mode {
    pub title: &'static str,
    pub items: &'static [&'static str],
}

/// The lines under the groups of the filter board, and the examples typed into its fields.
pub struct Hints {
    pub search: &'static str,
    pub search_example: &'static str,
    pub program: &'static str,
    pub program_typed: &'static str,
    pub program_found: &'static str,
    /// „Mit Studiengang": the filters that come with a program.
    pub with_program: &'static str,
    pub with_program_hint: &'static str,
    pub area_example: &'static str,
    pub dates: &'static str,
    pub exam: &'static str,
    pub credits: &'static str,
    pub sort: &'static str,
    pub sort_hint: &'static str,
    /// On a phone the board shows the groups of its example first, the others after this.
    pub all: &'static str,
}

pub const DE: Texts = Texts {
    heading: "Betula im Detail",
    lead: "Was Betula kann und wie es gebaut ist, Kapitel für Kapitel, mit Bildern aus der App selbst.",
    chapters: "Kapitel",

    flow: Chapter {
        name: "Der Weg zum Modul",
        title: "Jede Frage hat einen kurzen Weg",
        lead: "Ob du mit einem Suchbegriff kommst, mit deinem Studiengang oder mit einer Lücke in deiner Woche: Alle Wege führen zur selben Modulseite, auf der alles steht.",
        points: [
            Point { title: "Suche auf jeder Seite", text: "Titel oder Modulnummer eintippen, Strg+K oder / springt hinein. Die Liste filtert, während du tippst." },
            Point { title: "Vorschau neben der Liste", text: "Liste, Filter und Scrollposition bleiben stehen. F zeigt das Modul als ganze Seite, Esc schließt es." },
            Point { title: "Jede Ansicht ist ein Link", text: "Suche, Filter und das offene Modul stehen in der Adresse: zum Teilen, zum Speichern und für „Zurück“." },
            Point { title: "Schnell mit der Tastatur", text: "↑ und ↓ gehen durch die Liste, Enter öffnet, M merkt. Jedes Kürzel steht neben seinem Knopf." },
        ],
    },
    questions: ["„Wie hieß das Modul mit Datenbanken?“", "„Was steht im 3. Semester an?“", "„Welche Wahlpflicht gibt es auf Englisch?“", "„Was passt noch in meine Woche?“"],
    as_you_type: "Treffer beim Tippen",
    third_semester: "3. Fachsemester",
    module_page: "Die Modulseite",
    module_parts: ["Inhalte und Lernziele", "Prüfung und Leistungspunkte", "Termine als Wochenplan", "Voraussetzungen", "Studiengänge mit Semester"],

    filters: Chapter {
        name: "Filter",
        title: "Ein Filter für jede Frage",
        lead: "Studiengang, Semester, Turnus, Prüfung, Sprache, Standort, Lehrende und mehr: Alle Filter lassen sich frei kombinieren, und fast jeder kennt drei Stufen.",
        points: [
            Point { title: "Mit oder ohne", text: "Ein Klick nimmt nur die Module mit dieser Eigenschaft, ein zweiter schließt sie aus: „alles außer Klausur“." },
            Point { title: "Nichts verschwindet heimlich", text: "Ein Ausschluss entfernt nur, was die Daten ausdrücklich sagen. Ein Modul ohne Angabe bleibt in der Liste." },
            Point { title: "Mit deinem Studiengang", text: "Curriculum oder FÜS-Liste, Pflicht oder Wahlpflicht, Bereich und Fachsemester laut Regelstudienplan." },
            Point { title: "Überall gleich", text: "Am Handy als Blatt zum Wegwischen, ohne JavaScript als Links und Formulare. „Zurücksetzen“ nimmt alles zurück." },
        ],
    },
    steps: ["aus", "nur mit", "alles außer"],
    example: |n| format!("Diese Auswahl im Katalog: {n} Module"),
    hints: Hints {
        search: "Titel auf Deutsch oder Englisch und Modulnummer, auch Teile davon.",
        search_example: "datenbank",
        program: "Verzeiht Tippfehler und kennt Abkürzungen; „Mein Studiengang“ steht obenan.",
        program_typed: "infomatik bsc",
        program_found: "Informatik B.Sc.",
        with_program: "Mit Studiengang",
        with_program_hint: "Ein Fachsemester zeigt auch die Wahlpflichtmodule, die der Plan dort verlangt; „?“ die Module ohne Semester.",
        area_example: "Praktische Informatik",
        dates: "„Bestätigt“: nur Module mit veröffentlichten Terminen. „Passt in meinen Stundenplan“ vergleicht Vorlesungen, Übungen und Prüfungen mit deiner Woche.",
        exam: "Ein Modul mit mehreren Prüfungsteilen steht bei jedem davon.",
        credits: "Zwei Griffe von 0 bis 30, am rechten Ende ohne Obergrenze.",
        sort: "Sortierung",
        sort_hint: "Per Klick auf den Spaltenkopf, auf- und absteigend; im Studiengang in der Reihenfolge seines Plans.",
        all: "Alle Filter zeigen",
    },

    plans: Chapter {
        name: "Regelstudienpläne",
        title: "Aus dem PDF der Prüfungsordnung in eine klare Matrix",
        lead: "Welche Module in welches Semester gehören, steht bei der BTU nur als Tabelle im PDF der Prüfungsordnung. Betula hat die Ordnungen gelesen und ihre Pläne maschinenlesbar gemacht.",
        points: [
            Point { title: "Zelle für Zelle gelesen", text: "Die Semesterspalten kommen aus der Geometrie der Tabelle, samt verbundener Zellen, Spannen wie 10–24 LP und Modulen über mehrere Semester." },
            Point { title: "Gegen die eigenen Summen geprüft", text: "Was nicht aufgeht oder unklar bleibt, wird nicht geraten, sondern von Hand geprüft. Beim Plan steht, woher er stammt." },
            Point { title: "KI nur als Helfer", text: "Ein Sprachmodell ordnet Modulnummern und Arten zu. Semester und Leistungspunkte kommen immer aus den Zellen." },
            Point { title: "Jede Studienrichtung ihr Plan", text: "Und Zeilen ohne Modul, etwa „Wahlpflichtmodul aus der Informatik“, führen zu den Bereichen, die gemeint sind." },
        ],
    },
    plans_figure: "Studiengänge mit geprüftem Regelstudienplan",
    plans_of: |programs| format!("von {programs} in ihrer aktuellen Prüfungsordnung"),
    pdf: "Prüfungsordnung, PDF",
    in_betula: "In Betula",
    plan_rows: ["Mathematik I", "Programmierung", "Wahlpflicht Informatik", "Projekt", "Bachelorarbeit"],
    plan_sum: "Summe laut Ordnung",

    timetable: Chapter {
        name: "Stundenplan",
        title: "Dein Semester als Woche, ohne böse Überraschungen",
        lead: "Plane Module ein, einzeln oder als ganzes Fachsemester aus dem Regelstudienplan. Betula legt die Termine in die Woche und zeigt, was sich wirklich überschneidet.",
        points: [
            Point { title: "Echte Überschneidungen", text: "Gezählt an den Tagen, an denen sich Termine treffen: mit A- und B-Wochen, Terminen in nur einem Teil des Semesters und ausgefallenen Tagen." },
            Point { title: "Übungsgruppen zur Wahl", text: "Eine Überschneidung zählt erst, wenn keine Gruppe mehr frei ist. Mit ✓ und × entscheidest du direkt in der Woche." },
            Point { title: "Prüfungen im Blick", text: "Betula warnt, wenn Prüfungen sich überschneiden oder zu wenig Zeit für den Weg zwischen Cottbus und Senftenberg lassen." },
            Point { title: "Findet, was noch passt", text: "„Passt in meinen Stundenplan“ zeigt im Katalog nur Module, die in deine freien Zeiten passen." },
        ],
    },
    example_week: "Beispielwoche",
    week_modules: ["Mathe I", "Programmierung", "Physik", "Wiss. Arbeiten"],

    account: Chapter {
        name: "Ohne Konto",
        title: "Kein Konto. Und trotzdem fehlt nichts.",
        lead: "Merkliste, Stundenplan, gespeicherte Pläne, Kalender-Abos und dein Studiengang funktionieren ohne Anmeldung, weil sie in deinem Browser liegen statt auf einem Server.",
        points: [
            Point { title: "Auf andere Geräte", text: "„Auf anderes Gerät übertragen“ nimmt die Merkliste im Link mit. Der Teil hinter dem # erreicht nie einen Server." },
            Point { title: "Zum Teilen", text: "„Link zum Teilen kopieren“ gibt einen Stundenplan weiter; wer ihn öffnet, kann die Module übernehmen." },
            Point { title: "In jeden Kalender", text: "Als Abo für Apple Kalender, Google Kalender oder Outlook, das neue Termine selbst nachlädt." },
            Point { title: "Datensparsam gebaut", text: "Keine Analysedienste, keine Cookies, die dich wiedererkennen, nichts von fremden Servern. Der Server steht in Deutschland." },
        ],
    },
    zeros: [("0", "Konten und Passwörter"), ("0", "Tracker und Werbung"), ("0", "Anfragen an fremde Server"), ("7", "Tage, dann ist das Zugriffsprotokoll gelöscht")],
    in_browser: "In deinem Browser",
    saved_plans: "Gespeicherte Pläne",
    settings: "Sprache, Farben, Breiten",
    catalog_copy: "Katalog für offline",
    on_server: "Auf dem Server",
    for_everyone: "Katalog und Seiten, für alle gleich",
    nothing_of_yours: "Nichts von dir",
    after_hash: "Der Teil hinter # bleibt im Browser",

    javascript: Chapter {
        name: "Technik",
        title: "Läuft überall: mit und ohne JavaScript, auch offline",
        lead: "Jede Seite kommt als fertiges HTML vom Server. Mit JavaScript übernimmt eine App aus Rust und WebAssembly und rechnet ab dann direkt in deinem Browser.",
        points: [
            Point { title: "Aus denselben Bausteinen", text: "Server und App zeigen dieselben Seiten aus denselben Komponenten; die App spart sich nur den Weg über das Netz." },
            Point { title: "Aus dem Speicher", text: "Der Server baut jede Seite einmal je Datenstand und liefert sie danach aus dem Speicher." },
            Point { title: "Bereit ohne Netz", text: "Ein Service Worker hält die App bereit, der Katalog liegt in deinem Browser, und Betula lässt sich wie eine App installieren." },
            Point { title: "Leise aktuell", text: "Einen neuen Datenstand lädt die App im Hintergrund und nutzt ihn ab dem nächsten Start." },
        ],
    },
    without_js: Mode {
        title: "Ohne JavaScript",
        items: &["Jede Seite als vollständiges HTML", "Filter als Links und Formulare", "Alle Module, Studiengänge und Pläne", "Für Suchmaschinen und Textbrowser"],
    },
    with_js: Mode {
        title: "Mit JavaScript",
        items: &["Katalog als SQLite-Datenbank im Browser", "Suche und Filter in Millisekunden", "Keine Ladezeiten zwischen den Seiten", "Offline und als App installierbar", "Merkliste und Stundenplan"],
    },

    data: Chapter {
        name: "Daten",
        title: "Von der Wurzel bis ins Blatt",
        lead: "Betula ist gebaut wie die Birke, nach der es heißt: Radix, die Wurzel, holt und prüft die Daten der BTU, Folia, die Blätter, zeigt sie dir.",
        points: [
            Point { title: "Höflich eingelesen", text: "Mit Pausen zwischen den Anfragen und das meiste nachts. Gelesene Seiten werden aufbewahrt." },
            Point { title: "Nur geprüft veröffentlicht", text: "Ein neuer Datenstand erscheint nur, wenn er alle Prüfungen besteht; sonst bleibt der vorige." },
            Point { title: "Ehrlich bei Lücken", text: "„nicht angegeben“ statt Vermutung, Abgeleitetes ist gekennzeichnet, und QIS-Platzhalter wie 01:00 Uhr werden als „Zeit offen“ erkannt." },
            Point { title: "Immer mit Original", text: "Jede Modul- und Studiengangsseite verlinkt auf ihre Quelle bei der BTU." },
        ],
    },
    folia: ("Folia", "die Blätter: die Web-App mit Suche, Filtern und Stundenplan"),
    trunk: ("Datenstand", "geprüft, und nur veröffentlicht, wenn alles stimmt"),
    radix: ("Radix", "die Wurzel: liest die Quellen der BTU, höflich und das meiste nachts"),
    sources: [
        ("Offene Termine", "alle 2 Std."),
        ("Vorlesungsverzeichnis", "jede Nacht"),
        ("FÜS-Liste", "jede 2. Nacht"),
        ("Modulbeschreibungen", "mind. monatlich"),
        ("Modulbäume", "monatlich"),
        ("Prüfungsordnungen", "als PDF"),
    ],

    devices: Chapter {
        name: "Geräte und Sprachen",
        title: "Am Rechner wie am Handy, hell wie dunkel",
        lead: "Am großen Bildschirm stehen Filter, Liste und Vorschau nebeneinander. Am Handy liegt die Navigation unten, und die Filter öffnen als Blatt zum Wegwischen.",
        points: [
            Point { title: "Deutsch und Englisch", text: "Jede Seite unter eigener Adresse; Titel und Beschreibungen der Module stehen, wie die BTU sie schreibt." },
            Point { title: "Hell und dunkel", text: "Folgt deinem System oder deiner Wahl, bis hin zur Birke, die mit den Jahreszeiten ihr Laub wechselt." },
            Point { title: "Tastenkürzel", text: "Strg+K sucht, ↑ und ↓ wählen, Enter öffnet, M merkt, F zeigt ganz, Esc schließt." },
            Point { title: "Links mit Vorschau", text: "Geteilte Module und Studiengänge zeigen im Messenger eine Karte mit Titel und Eckdaten." },
        ],
    },
    phone_alt: "Die Seite eines Moduls am Handy",

    to_catalog: "Zum Modulkatalog",
    to_programs: "Zu den Studiengängen",
    to_studyplan: "Zum Stundenplan",
    to_privacy: "Zur Datenschutzerklärung",
};

pub const EN: Texts = Texts {
    heading: "Betula in detail",
    lead: "What Betula does and how it is built, chapter by chapter, with pictures from the app itself.",
    chapters: "Chapters",

    flow: Chapter {
        name: "The way to a module",
        title: "Every question has a short way",
        lead: "Whether you come with a search term, with your degree programme or with a gap in your week: every way leads to the same module page, with everything on it.",
        points: [
            Point { title: "Search on every page", text: "Type a title or module number; Ctrl+K or / jumps there. The list filters as you type." },
            Point { title: "Preview beside the list", text: "The list, the filters and the scroll position stay put. F shows the module as a full page, Esc closes it." },
            Point { title: "Every view is a link", text: "Search, filters and the open module are part of the address: to share, to bookmark, and for Back." },
            Point { title: "Quick by keyboard", text: "↑ and ↓ move through the list, Enter opens, M saves. Every shortcut stands next to its button." },
        ],
    },
    questions: ["“What was that module on databases called?”", "“What is due in semester 3?”", "“Which electives are taught in English?”", "“What still fits into my week?”"],
    as_you_type: "Results as you type",
    third_semester: "Semester 3",
    module_page: "The module page",
    module_parts: ["Contents and learning outcomes", "Assessment and credit points", "Dates as a weekly schedule", "Prerequisites", "Degree programmes with semester"],

    filters: Chapter {
        name: "Filters",
        title: "A filter for every question",
        lead: "Degree programme, semester, offering, assessment, language, location, lecturers and more: all filters combine freely, and almost every one has three steps.",
        points: [
            Point { title: "With or without", text: "One click keeps only the modules with that property, a second one leaves them out: “everything but a written exam”." },
            Point { title: "Nothing disappears quietly", text: "Leaving something out only removes what the data states. A module without the information stays in the list." },
            Point { title: "With your programme", text: "Curriculum or FÜS list, compulsory or elective, area and semester according to the standard study plan." },
            Point { title: "The same everywhere", text: "On a phone a sheet you swipe away, without JavaScript links and forms. “Reset” takes it all back." },
        ],
    },
    steps: ["off", "only with", "all but"],
    example: |n| format!("This selection in the catalogue: {n} modules"),
    hints: Hints {
        search: "German or English title and module number, or part of one.",
        search_example: "datenbank",
        program: "Forgives typos and knows abbreviations; “My programme” comes first.",
        program_typed: "infomatik bsc",
        program_found: "Informatik B.Sc.",
        with_program: "With a programme",
        with_program_hint: "A semester also lists the electives the plan asks for there; “?” the modules without a semester.",
        area_example: "Praktische Informatik",
        dates: "“Confirmed”: only modules with published dates. “Fits my timetable” compares lectures, exercises and exams with your week.",
        exam: "A module with several parts of assessment is listed under each of them.",
        credits: "Two handles from 0 to 30, no upper limit at the right end.",
        sort: "Order",
        sort_hint: "With a click on the column head, ascending or descending; within a programme in the order of its plan.",
        all: "Show all filters",
    },

    plans: Chapter {
        name: "Study plans",
        title: "From the regulations' PDF into a clear matrix",
        lead: "Which modules belong in which semester is stated at BTU only as a table in the PDF of the examination regulations. Betula has read the regulations and made their plans machine-readable.",
        points: [
            Point { title: "Read cell by cell", text: "The semester columns come from the geometry of the table, including merged cells, ranges such as 10–24 CP and modules over several semesters." },
            Point { title: "Checked against their own totals", text: "What does not add up or stays unclear is not guessed but checked by hand. Every plan says where it comes from." },
            Point { title: "AI only as a helper", text: "A language model assigns module numbers and kinds. Semesters and credit points always come from the cells." },
            Point { title: "Every study direction its plan", text: "And rows without a module, such as “Wahlpflichtmodul aus der Informatik”, lead to the areas they mean." },
        ],
    },
    plans_figure: "degree programmes with a checked standard study plan",
    plans_of: |programs| format!("of {programs} in their current examination regulations"),
    pdf: "Examination regulations, PDF",
    in_betula: "In Betula",
    plan_rows: ["Mathematics I", "Programming", "Elective: Computer Science", "Project", "Bachelor's thesis"],
    plan_sum: "Total as the regulations state",

    timetable: Chapter {
        name: "Timetable",
        title: "Your semester as a week, without nasty surprises",
        lead: "Plan modules one by one or import a whole semester of your standard study plan. Betula puts the dates into the week and shows what really clashes.",
        points: [
            Point { title: "Real clashes", text: "Counted on the days on which dates meet: with A and B weeks, dates in only part of the semester and cancelled days." },
            Point { title: "Exercise groups to choose from", text: "A clash only counts once no group is free any more. ✓ and × decide right in the week." },
            Point { title: "Exams in view", text: "Betula warns when exams overlap or leave too little time to get between Cottbus and Senftenberg." },
            Point { title: "Finds what still fits", text: "“Fits my timetable” shows only modules in the catalogue that fit into your free time." },
        ],
    },
    example_week: "Example week",
    week_modules: ["Maths I", "Programming", "Physics", "Academic Writing"],

    account: Chapter {
        name: "No account",
        title: "No account. And yet nothing is missing.",
        lead: "Saved modules, your timetable, saved plans, calendar subscriptions and your programme all work without signing up, because they live in your browser instead of on a server.",
        points: [
            Point { title: "To other devices", text: "“Move to another device” carries your saved modules in a link. The part after the # never reaches a server." },
            Point { title: "To share", text: "“Copy link to share” passes a timetable on; whoever opens it can add its modules." },
            Point { title: "Into any calendar", text: "As a subscription for Apple Calendar, Google Calendar or Outlook that fetches new dates by itself." },
            Point { title: "Built to collect little", text: "No analytics, no cookies that recognise you, nothing from other servers. The server stands in Germany." },
        ],
    },
    zeros: [("0", "accounts and passwords"), ("0", "trackers and ads"), ("0", "requests to other servers"), ("7", "days, then the access log is deleted")],
    in_browser: "In your browser",
    saved_plans: "Saved plans",
    settings: "Language, colours, widths",
    catalog_copy: "Catalogue for offline",
    on_server: "On the server",
    for_everyone: "Catalogue and pages, the same for everyone",
    nothing_of_yours: "Nothing of yours",
    after_hash: "The part after # stays in the browser",

    javascript: Chapter {
        name: "Technology",
        title: "Runs everywhere: with or without JavaScript, even offline",
        lead: "Every page comes from the server as finished HTML. With JavaScript an app written in Rust and running as WebAssembly takes over and computes right in your browser from then on.",
        points: [
            Point { title: "The same building blocks", text: "Server and app show the same pages from the same components; the app just skips the trip through the network." },
            Point { title: "From memory", text: "The server builds every page once per data set and then delivers it from memory." },
            Point { title: "Ready without a network", text: "A service worker keeps the app ready, the catalogue lives in your browser, and Betula installs like an app." },
            Point { title: "Quietly up to date", text: "The app downloads new data in the background and uses it from the next start." },
        ],
    },
    without_js: Mode {
        title: "Without JavaScript",
        items: &["Every page as complete HTML", "Filters as links and forms", "Every module, programme and plan", "For search engines and text browsers"],
    },
    with_js: Mode {
        title: "With JavaScript",
        items: &["Catalogue as an SQLite database in the browser", "Search and filters in milliseconds", "No loading times between pages", "Offline and installable as an app", "Saved modules and timetable"],
    },

    data: Chapter {
        name: "Data",
        title: "From root to leaf",
        lead: "Betula is built like the birch it is named after: Radix, the root, fetches and checks BTU's data; Folia, the leaves, shows it to you.",
        points: [
            Point { title: "Read politely", text: "With pauses between requests and mostly at night. Pages that were read are kept." },
            Point { title: "Published only when checked", text: "A new data set appears only when it passes every check; otherwise the previous one stays." },
            Point { title: "Honest about gaps", text: "“not stated” instead of a guess, what is derived is marked, and QIS placeholders such as 1 a.m. are recognised as “Time TBA”." },
            Point { title: "Always with the original", text: "Every module and programme page links to its source at BTU." },
        ],
    },
    folia: ("Folia", "the leaves: the web app with search, filters and timetable"),
    trunk: ("Data set", "checked, and published only when everything adds up"),
    radix: ("Radix", "the root: reads BTU's sources, politely and mostly at night"),
    sources: [
        ("Open dates", "every 2 h"),
        ("Course catalogue", "every night"),
        ("FÜS list", "every 2nd night"),
        ("Module descriptions", "at least monthly"),
        ("Module trees", "monthly"),
        ("Examination regulations", "as PDF"),
    ],

    devices: Chapter {
        name: "Devices and languages",
        title: "On a computer and on a phone, light and dark",
        lead: "On a large screen, filters, list and preview stand side by side. On a phone the navigation sits at the bottom, and the filters open as a sheet you swipe away.",
        points: [
            Point { title: "German and English", text: "Every page at its own address; module titles and descriptions are shown as BTU writes them." },
            Point { title: "Light and dark", text: "Follows your system or your choice, down to the birch, whose leaves change with the seasons." },
            Point { title: "Keyboard shortcuts", text: "Ctrl+K searches, ↑ and ↓ select, Enter opens, M saves, F shows in full, Esc closes." },
            Point { title: "Links with a preview", text: "Shared modules and programmes show a card with title and key facts in messengers." },
        ],
    },
    phone_alt: "The page of a module on a phone",

    to_catalog: "To the module catalogue",
    to_programs: "To the degree programmes",
    to_studyplan: "To the timetable",
    to_privacy: "To the privacy notice",
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n;
    use catalog::Locale;

    /// Every language has the same chapters with as much in them, and the words written out
    /// read as they should.
    #[test]
    fn the_languages_match() {
        assert_eq!((DE.without_js.items.len(), DE.with_js.items.len()), (EN.without_js.items.len(), EN.with_js.items.len()));
        for t in [&DE, &EN] {
            let chapters = [&t.flow, &t.filters, &t.plans, &t.timetable, &t.account, &t.javascript, &t.data, &t.devices];
            let mut names: Vec<&str> = chapters.iter().map(|chapter| chapter.name).collect();
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), chapters.len(), "{names:?}");
            for chapter in chapters {
                assert!(chapter.points.iter().all(|point| !point.title.is_empty() && point.text.ends_with(['.', '“', '”'])), "{}", chapter.name);
            }
        }
        assert_eq!(((DE.example)("206"), (EN.plans_of)("115")), ("Diese Auswahl im Katalog: 206 Module".to_string(), "of 115 in their current examination regulations".to_string()));
    }

    /// What a chapter quotes of the app (a button, a filter, a label) is what the app says there,
    /// in every language: renamed there, it has to be renamed here.
    #[test]
    fn the_chapters_quote_the_app_as_it_is() {
        for locale in Locale::ALL {
            let t = i18n::texts(*locale);
            let (open, close) = if *locale == Locale::De { ("„", "“") } else { ("“", "”") };
            let quoted = |label: &str| format!("{open}{label}{close}");
            let d = &t.home_detail;
            let point = |chapter: &Chapter, n: usize| chapter.points.get(n).map(|point| point.text).unwrap_or_default();
            let pairs = [
                (point(&d.filters, 3), quoted(t.common.reset)),
                (point(&d.timetable, 3), quoted(t.catalog.fits)),
                (point(&d.account, 0), quoted(t.bookmarks.transfer)),
                (point(&d.account, 1), quoted(t.studyplan_share.copy_link)),
                (point(&d.data, 2), quoted(t.common.not_stated)),
                (point(&d.data, 2), quoted(t.module.time_open)),
                (d.hints.program, quoted(t.catalog.my_program)),
                (d.hints.dates, quoted(t.catalog.confirmed)),
                (d.hints.dates, quoted(t.catalog.fits)),
            ];
            for (text, label) in pairs {
                assert!(text.contains(&label), "{locale:?}: {label} is not in: {text}");
            }
        }
    }
}
