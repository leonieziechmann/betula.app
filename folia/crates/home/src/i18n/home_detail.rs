//! Texts of the chapters of „Über Betula"'s topics (`home/detail.rs`; „Betula im Detail" at the
//! end of its one page until 2026-10-27): a chapter per feature, each a name, a headline, a line
//! under it and four points, and the words of its picture. Where a picture shows a part of the app
//! (a button, a filter, a chip), the page takes its words from that part's own group (`catalog`,
//! `studyplan_export` …), so that they are the same; what stands here is only what no other part
//! says, and what it quotes of a feature this crate does not know (`quoted_*`) is held to that
//! feature's words by `folia_app::quotes`.
//!
//! Since 2026-11-18 the chapters say what Betula is by now (owner: the pages „sind alle veraltet
//! und könnten etwas interessanter werden"): the Merkliste with its collections and notes, „Mein
//! Studium", the profile kept on the device, Betula Sync and sharing have chapters of their own,
//! and none speaks of an account (owner, the same day: „wir reden nicht von konten, weil die gibt
//! es ja nicht sondern von profilen und lokalen Daten").

pub struct Texts {
    // From a question to the module
    pub flow: Chapter,
    /// The questions people come with, one per way; the steps of each way are the app's own
    /// words, and these two are the only ones it has none for.
    pub questions: [&'static str; 5],
    pub as_you_type: &'static str,
    pub third_semester: &'static str,
    /// Where every way ends, and what stands on it.
    pub module_page: &'static str,
    pub module_parts: [&'static str; 5],

    // Every filter
    pub filters: Chapter,
    /// The three steps of a chip, under the three examples: off, with, without.
    pub steps: [&'static str; 3],
    /// „Diese Auswahl im Katalog: 23 Module": what the board's chips choose (the example, until
    /// they are switched), with the number the catalog has for it (and that number written).
    pub example: fn(u64, &str) -> String,
    pub hints: Hints,

    // The Merkliste
    pub marks: Chapter,
    /// The picture: „Alle" over the collections, three collections of somebody's, and three
    /// marked modules, each its name, what it counts as, and the note it may carry.
    pub shelf_all: &'static str,
    pub shelf_collections: [&'static str; 3],
    pub shelf_rows: [(&'static str, &'static str, Option<&'static str>); 3],

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

    // „Mein Studium"
    pub study: Chapter,
    /// The picture: how far the study is, the areas with what is passed of what they ask
    /// (their names are the regulation's, the same in every language), what the parts of the bar
    /// mean, and the semesters, each with what it holds.
    pub study_headline: &'static str,
    pub study_areas: [(&'static str, &'static str); 4],
    pub study_legend: [&'static str; 3],
    pub study_semesters: [(&'static str, &'static str); 5],
    /// „jetzt": the semester the student is in.
    pub study_now: &'static str,

    // The Stundenplan
    pub timetable: Chapter,
    /// Over the week of the picture, and the names of its modules (the kinds are the app's).
    pub example_week: &'static str,
    pub week_modules: [&'static str; 4],
    /// A Termin of one's own in the week.
    pub week_own: &'static str,

    // The profile
    pub profile: Chapter,
    /// Four figures: the number and what it counts.
    pub zeros: [(&'static str, &'static str); 4],
    /// Where things live: on the device (the Merkliste, the Stundenplan, „Mein Studium" and „Mein
    /// Studiengang" are named as the app names them) …
    pub on_device: &'static str,
    pub settings: &'static str,
    pub catalog_copy: &'static str,
    /// … on the server …
    pub on_server: &'static str,
    pub for_everyone: &'static str,
    pub nothing_of_yours: &'static str,
    pub sealed_copy: &'static str,
    /// … and what goes between them: the address of the page asked for.
    pub only_the_address: &'static str,

    // Betula Sync
    pub sync: Chapter,
    /// The picture: two devices and the server between them. A change as a device shows it, the
    /// same change as the server holds it, and what the server knows of it.
    pub sync_phone: &'static str,
    pub sync_laptop: &'static str,
    pub sync_server: &'static str,
    pub sync_change: &'static str,
    pub sync_sealed: &'static str,
    pub sync_knows: &'static str,
    /// The ways of the key from device to device, and of the changes.
    pub sync_ways: [&'static str; 3],

    // Sharing
    pub sharing: Chapter,
    /// The picture: under each way of the dialog „Teilen" (named by the dialog, `quoted_*`) what
    /// it is, and what comes along with a calendar: what always does, and what only on request.
    pub share_lines: [&'static str; 3],
    pub share_always: &'static str,

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
    pub to_bookmarks: &'static str,
    pub to_programs: &'static str,
    pub to_study: &'static str,
    pub to_studyplan: &'static str,
    pub to_profile: &'static str,
    pub to_privacy: &'static str,
    /// What the tour quotes of the other areas, as they name it themselves (keep in step with
    /// `program::in_catalog`, `studyplan_head::week_a`, `clashes_per_week`, `studyplan_share`,
    /// `studyplan_export`, the Merkliste's `collections` and stands, and the dialog „Teilen";
    /// `folia_app::quotes` holds them to it): „Über Betula" knows no feature's texts but its own.
    pub quoted_in_catalog: &'static str,
    pub quoted_week_a: &'static str,
    pub quoted_one_clash_per_week: &'static str,
    pub quoted_copy_link: &'static str,
    pub quoted_download: &'static str,
    pub quoted_apple: &'static str,
    pub quoted_google: &'static str,
    pub quoted_collections: &'static str,
    pub quoted_planned: &'static str,
    pub quoted_passed: &'static str,
    pub quoted_fixed: &'static str,
    pub quoted_live: &'static str,
    pub quoted_once: &'static str,
    pub quoted_along: &'static str,
    pub quoted_own: &'static str,
    pub quoted_notes: &'static str,
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
    flow: Chapter {
        name: "Der Weg zum Modul",
        title: "Jede Frage hat einen kurzen Weg",
        lead: "Ob du mit einem Suchbegriff kommst, mit deinem Studiengang oder mit einer Lücke in deiner Woche: Alle Wege führen zur selben Modulseite, auf der alles steht.",
        points: [
            Point { title: "Eine Suche, die mitdenkt", text: "Titel, Nummer, Kürzel oder Lehrende eintippen, / springt hinein. „TI“ findet Theoretische Informatik, ein Tippfehler stört nicht, und unter den Treffern stehen „Ähnliche Module“ zum selben Thema." },
            Point { title: "Vorschau neben der Liste", text: "Liste, Filter und Scrollposition bleiben stehen. V zeigt das Modul im Vollbild, Esc schließt es." },
            Point { title: "Jede Ansicht ist ein Link", text: "Suche, Filter und das offene Modul stehen in der Adresse: zum Teilen, zum Speichern und für „Zurück“." },
            Point { title: "Schnell mit der Tastatur", text: "↑ und ↓ gehen durch die Liste, Enter öffnet, M merkt, P plant ein. Strg+K öffnet die Befehle, und jede Taste hat ihren Knopf." },
        ],
    },
    questions: ["„Wie hieß das Modul mit Datenbanken?“", "„Was gibt es zu maschinellem Lernen?“", "„Was steht im 3. Semester an?“", "„Welche Wahlpflicht gibt es auf Englisch?“", "„Was passt noch in meine Woche?“"],
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
    example: |n, written| if n == 1 { "Diese Auswahl im Katalog: 1 Modul".to_string() } else { format!("Diese Auswahl im Katalog: {written} Module") },
    hints: Hints {
        search: "Titel auf Deutsch oder Englisch, Modulnummer, Kürzel und Lehrende, auch Teile davon.",
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

    marks: Chapter {
        name: "Merkliste",
        title: "Merken, ordnen, wiederfinden",
        lead: "Ein Klick aufs Lesezeichen, und das Modul steht auf deiner Merkliste. Sammlungen ordnen, was zusammengehört, und eine Notiz hält fest, warum du es dir gemerkt hast.",
        points: [
            Point { title: "Sammlungen, so viele du brauchst", text: "Für die Wahlpflicht, für den Master, für Tipps aus der Fachschaft. Ein Modul kann in mehreren stehen." },
            Point { title: "Eine Notiz am Modul", text: "„Lena fand die Übungen gut“: Die Notiz steht in der Liste neben dem Modul und bleibt bei dir." },
            Point { title: "Zählt es in meinem Studium?", text: "Die Liste sagt zu jedem Modul, in welchen Bereich deines Studiengangs es gehört und ob es schon geplant oder bestanden ist." },
            Point { title: "Für die Lerngruppe", text: "Eine Sammlung lässt sich als Link weitergeben: als Kopie von jetzt, oder so, dass andere ihr folgen und jede Änderung sehen." },
        ],
    },
    shelf_all: "Alle",
    shelf_collections: ["Wahlpflicht", "Vielleicht im Master", "Tipps aus der Fachschaft"],
    shelf_rows: [
        ("Datenbanken", "6 LP · Wahlpflicht · Praktische Informatik", Some("Grundlage fürs Softwareprojekt")),
        ("Einführung in die Künstliche Intelligenz", "6 LP · Wahlpflicht · Angewandte Informatik", Some("Lena fand die Übungen gut")),
        ("Betriebssysteme I", "8 LP · Pflicht", None),
    ],

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

    study: Chapter {
        name: "Mein Studium",
        title: "Bestanden, geplant, offen: dein Studium auf einen Blick",
        lead: "Du sagst Betula einmal, was du studierst und seit wann. Daraus wird dein Plan bis zum Abschluss, Semester für Semester, mit den Leistungspunkten jedes Bereichs.",
        points: [
            Point { title: "In drei Klicks eingerichtet", text: "Studiengang, Studienbeginn, Fachsemester: Betula füllt die Semester nach Regelstudienplan, und du hakst nur noch ab, was bestanden ist." },
            Point { title: "Leistungspunkte je Bereich", text: "Ein Balken zeigt, was bestanden, geplant und offen ist. Belegst du mehr, als ein Bereich verlangt, sagt Betula auch das." },
            Point { title: "Warnt, bevor es hakt", text: "Beim Einplanen siehst du, ob ein Modul in dem Semester angeboten wird und ob seine Voraussetzungen schon davor liegen." },
            Point { title: "Wenn es anders läuft", text: "Urlaubssemester, ein Semester länger, eine Wiederholung oder ein Wechsel des Studiengangs: Der Plan rechnet neu, und nichts geht verloren." },
        ],
    },
    study_headline: "84 von 180 LP bestanden",
    study_areas: [("Informatik", "54/66"), ("Mathematik", "24/24"), ("Nebenfach", "6/18"), ("FÜS", "0/6")],
    study_legend: ["bestanden", "geplant", "offen"],
    study_semesters: [("1. FS", "30 LP bestanden"), ("2. FS", "30 LP bestanden"), ("3. FS", "24 LP bestanden"), ("4. FS", "30 LP, läuft"), ("5. FS", "18 von 30 LP geplant")],
    study_now: "jetzt",

    timetable: Chapter {
        name: "Stundenplan",
        title: "Dein Semester als Woche, ohne böse Überraschungen",
        lead: "Was du für ein Semester planst, steht hier als Woche. Betula legt die Termine aus dem Vorlesungsverzeichnis hinein und zeigt, was sich wirklich überschneidet.",
        points: [
            Point { title: "Echte Überschneidungen", text: "Gezählt an den Tagen, an denen sich Termine treffen, mit A- und B-Wochen; bei mehreren Übungsgruppen erst, wenn keine mehr frei ist." },
            Point { title: "Prüfungen im Blick", text: "Betula warnt, wenn Prüfungen sich überschneiden oder zu wenig Zeit für den Weg zwischen Cottbus und Senftenberg lassen." },
            Point { title: "Deine eigenen Termine", text: "Lerngruppe, Nebenjob, Sport: Eigenes steht neben den Vorlesungen. Einen Termin aus QIS kannst du verlegen und mit einer Notiz versehen." },
            Point { title: "Findet, was noch passt", text: "„Passt in meinen Stundenplan“ zeigt im Katalog nur Module, die in deine freien Zeiten passen." },
        ],
    },
    example_week: "Beispielwoche",
    week_modules: ["Mathe I", "Programmierung", "Physik", "Wiss. Arbeiten"],
    week_own: "Lerngruppe",

    profile: Chapter {
        name: "Dein Profil",
        title: "Was du planst, liegt bei dir",
        lead: "Merkliste, Stundenplan, „Mein Studium“ und dein Studiengang gehören zu einem Profil, und das Profil liegt lokal auf deinem Gerät. Nach deinem Namen oder einer E-Mail-Adresse fragt Betula nicht.",
        points: [
            Point { title: "Mehrere Profile", text: "Zwei Studiengänge oder ein Plan zum Ausprobieren: Jedes Profil hat seine eigene Merkliste, seinen Stundenplan und sein Studium." },
            Point { title: "Sicherung als Datei", text: "Ein Profil lässt sich als Datei sichern und wieder laden. Die Datei entsteht in deinem Browser." },
            Point { title: "Leeren und löschen", text: "Auf der Seite „Profil“, jederzeit. Direkt danach holt „Rückgängig“ alles zurück." },
            Point { title: "Datensparsam gebaut", text: "Keine Analysedienste, keine Cookies, die dich wiedererkennen, nichts von fremden Servern. Der Server steht in Deutschland." },
        ],
    },
    zeros: [("0", "Angaben zu deiner Person"), ("0", "Tracker und Werbung"), ("0", "Anfragen an fremde Server"), ("7", "Tage, dann ist das Zugriffsprotokoll gelöscht")],
    on_device: "Auf deinem Gerät",
    settings: "Sprache, Farben, Breiten",
    catalog_copy: "Katalog für offline",
    on_server: "Auf dem Server",
    for_everyone: "Katalog und Seiten, für alle gleich",
    nothing_of_yours: "Nichts von dir",
    sealed_copy: "Mit Betula Sync: dein Profil, versiegelt",
    only_the_address: "Der Server sieht nur, welche Seite du aufrufst",

    sync: Chapter {
        name: "Betula Sync",
        title: "Handy, Laptop, Tablet: überall derselbe Stand",
        lead: "Schaltest du Betula Sync für ein Profil ein, gleichen sich deine Geräte ab, sobald sich etwas ändert. Verschlüsselt wird auf deinem Gerät, mit einem Schlüssel, den nur deine Geräte kennen.",
        points: [
            Point { title: "Code scannen, fertig", text: "Das eine Gerät zeigt einen QR-Code, das andere scannt ihn. Kein Passwort, das du dir merken musst." },
            Point { title: "Der Server kann nichts lesen", text: "Er bewahrt versiegelte Änderungen auf, unter zufälligen Kennungen, mit Größe und Zeitpunkt. Was drinsteht, weiß er nicht." },
            Point { title: "Auch ohne Netz", text: "Änderungen warten auf dem Gerät und gehen raus, sobald wieder Netz da ist. Ändern zwei Geräte dieselbe Stelle, zeigt Betula dir beide Fassungen." },
            Point { title: "Jederzeit wieder aus", text: "„Sync beenden“ löscht die Kopie auf dem Server sofort, jedes Gerät behält seine. Und sind alle Geräte weg, holt der Wiederherstellungscode das Profil zurück." },
        ],
    },
    sync_phone: "Dein Handy",
    sync_laptop: "Dein Laptop",
    sync_server: "Server von Betula",
    sync_change: "Datenbanken gemerkt",
    sync_sealed: "versiegelt",
    sync_knows: "kennt nur: zufällige Kennung, Größe, Zeitpunkt",
    sync_ways: ["QR-Code oder Link", "Wiederherstellungscode", "direkt von Gerät zu Gerät, wenn beide online sind"],

    sharing: Chapter {
        name: "Teilen",
        title: "Teilen, was du willst, und nur das",
        lead: "Den Stundenplan für die Lerngruppe, eine Sammlung für den Kommilitonen, den Kalender fürs Handy: Du wählst, was mitkommt, wer es öffnen kann und wie lange.",
        points: [
            Point { title: "Eine Kopie von jetzt", text: "Der Link trägt den Plan selbst. Wer ihn öffnet, kann die Module übernehmen; im Messenger zeigt er eine Vorschau." },
            Point { title: "Oder immer aktuell", text: "Mit Betula Sync folgen andere deinem Kalender oder deiner Liste und sehen jede Änderung, bis du aufhörst zu teilen." },
            Point { title: "Für genau eine Person", text: "Ein Einmal-Link gilt für den, der ihn zuerst öffnet, und höchstens 7 Tage. Bis dahin kannst du ihn zurückziehen." },
            Point { title: "In jeden Kalender", text: "Als Abo für Apple Kalender, Google Kalender oder Outlook, das neue Termine selbst nachlädt, oder als .ics-Datei." },
        ],
    },
    share_lines: ["Eine Kopie von jetzt, im Link selbst", "Jede Änderung kommt an, bis du aufhörst", "Für eine Person, höchstens 7 Tage"],
    share_always: "Module, Gruppen, geänderte Termine",

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
        items: &["Katalog als SQLite-Datenbank im Browser", "Suche und Filter in Millisekunden", "Keine Ladezeiten zwischen den Seiten", "Offline und als App installierbar", "Dein Profil: Merkliste, Studium, Stundenplan"],
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
            Point { title: "Als App aufs Handy", text: "Betula lässt sich auf Handy, Tablet und Rechner installieren und startet dann mit eigenem Symbol, auch ohne Netz." },
            Point { title: "Deutsch und Englisch", text: "Jede Seite unter eigener Adresse; Titel und Beschreibungen der Module stehen, wie die BTU sie schreibt." },
            Point { title: "Hell und dunkel", text: "Folgt deinem System oder deiner Wahl, bis hin zur Birke, die mit den Jahreszeiten ihr Laub wechselt." },
            Point { title: "Tasten und Befehle", text: "/ sucht, Strg+K öffnet die Befehle, ? zeigt alle Tasten. Jede Taste hat ihren Knopf: Nichts geht nur per Tastatur." },
        ],
    },
    phone_alt: "Die Seite eines Moduls am Handy",

    to_catalog: "Zum Modulkatalog",
    to_bookmarks: "Zur Merkliste",
    to_programs: "Zu den Studiengängen",
    to_study: "Zu „Mein Studium“",
    to_studyplan: "Zum Stundenplan",
    to_profile: "Zum Profil",
    to_privacy: "Zur Datenschutzerklärung",
    quoted_in_catalog: "Im Modulkatalog",
    quoted_week_a: "A-Woche",
    quoted_one_clash_per_week: "1 Überschneidung pro Woche",
    quoted_copy_link: "Link zum Teilen kopieren",
    quoted_download: ".ics herunterladen",
    quoted_apple: "Apple Kalender",
    quoted_google: "Google Kalender",
    quoted_collections: "Sammlungen",
    quoted_planned: "Geplant",
    quoted_passed: "Bestanden",
    quoted_fixed: "Fester Stand",
    quoted_live: "Bleibt aktuell",
    quoted_once: "Einmal-Link",
    quoted_along: "Was mitkommt",
    quoted_own: "Eigene Termine",
    quoted_notes: "Notizen an Terminen",
};

pub const EN: Texts = Texts {
    flow: Chapter {
        name: "The way to a module",
        title: "Every question has a short way",
        lead: "Whether you come with a search term, with your degree programme or with a gap in your week: every way leads to the same module page, with everything on it.",
        points: [
            Point { title: "A search that thinks along", text: "Type a title, number, abbreviation or lecturer; / jumps there. “TI” finds Theoretische Informatik, a typo does no harm, and under the results stand “Similar modules” on the same topic." },
            Point { title: "Preview beside the list", text: "The list, the filters and the scroll position stay put. V shows the module full screen, Esc closes it." },
            Point { title: "Every view is a link", text: "Search, filters and the open module are part of the address: to share, to bookmark, and for Back." },
            Point { title: "Quick by keyboard", text: "↑ and ↓ move through the list, Enter opens, M saves, P plans. Ctrl+K opens the commands, and every key has its button." },
        ],
    },
    questions: ["“What was that module on databases called?”", "“What is there on machine learning?”", "“What is due in semester 3?”", "“Which electives are taught in English?”", "“What still fits into my week?”"],
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
    example: |n, written| if n == 1 { "This selection in the catalogue: 1 module".to_string() } else { format!("This selection in the catalogue: {written} modules") },
    hints: Hints {
        search: "German or English title, module number, abbreviation and lecturers, or part of one.",
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

    marks: Chapter {
        name: "Saved modules",
        title: "Save, sort, find again",
        lead: "One click on the bookmark, and the module is among your saved modules. Collections sort what belongs together, and a note keeps why you saved it.",
        points: [
            Point { title: "As many collections as you need", text: "For your electives, for the master's, for tips from fellow students. A module can be in several." },
            Point { title: "A note on the module", text: "“Lena liked the exercises”: the note stands in the list beside the module and stays with you." },
            Point { title: "Does it count in my studies?", text: "For every module the list says which area of your programme it belongs to and whether it is planned or passed already." },
            Point { title: "For your study group", text: "A collection can be passed on as a link: as a copy of now, or so that others follow it and see every change." },
        ],
    },
    shelf_all: "All",
    shelf_collections: ["Electives", "Maybe for the master's", "Tips from fellow students"],
    shelf_rows: [
        ("Datenbanken", "6 CP · elective · Praktische Informatik", Some("Basis for the software project")),
        ("Einführung in die Künstliche Intelligenz", "6 CP · elective · Angewandte Informatik", Some("Lena liked the exercises")),
        ("Betriebssysteme I", "8 CP · compulsory", None),
    ],

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

    study: Chapter {
        name: "My studies",
        title: "Passed, planned, open: your studies at a glance",
        lead: "You tell Betula once what you study and since when. That becomes your plan up to your degree, semester by semester, with the credit points of every area.",
        points: [
            Point { title: "Set up in three clicks", text: "Programme, start of studies, semester: Betula fills the semesters from the standard study plan, and you only tick off what you have passed." },
            Point { title: "Credit points by area", text: "A bar shows what is passed, planned and open. If you take more than an area asks for, Betula says that too." },
            Point { title: "Warns before it snags", text: "While planning you see whether a module is offered in that semester and whether its prerequisites come before it." },
            Point { title: "When things go differently", text: "A semester of leave, one semester more, a retake or a change of programme: the plan counts anew, and nothing is lost." },
        ],
    },
    study_headline: "84 of 180 CP passed",
    study_areas: [("Informatik", "54/66"), ("Mathematik", "24/24"), ("Nebenfach", "6/18"), ("FÜS", "0/6")],
    study_legend: ["passed", "planned", "open"],
    study_semesters: [("Semester 1", "30 CP passed"), ("Semester 2", "30 CP passed"), ("Semester 3", "24 CP passed"), ("Semester 4", "30 CP, running"), ("Semester 5", "18 of 30 CP planned")],
    study_now: "now",

    timetable: Chapter {
        name: "Timetable",
        title: "Your semester as a week, without nasty surprises",
        lead: "What you plan for a semester stands here as a week. Betula puts the dates of the course catalogue into it and shows what really clashes.",
        points: [
            Point { title: "Real clashes", text: "Counted on the days on which dates meet, with A and B weeks; for exercises with several groups only once no group is free any more." },
            Point { title: "Exams in view", text: "Betula warns when exams overlap or leave too little time to get between Cottbus and Senftenberg." },
            Point { title: "Your own events", text: "Study group, job, sports: your own events stand beside the lectures. A session from QIS can be moved and given a note." },
            Point { title: "Finds what still fits", text: "“Fits my timetable” shows only modules in the catalogue that fit into your free time." },
        ],
    },
    example_week: "Example week",
    week_modules: ["Maths I", "Programming", "Physics", "Academic Writing"],
    week_own: "Study group",

    profile: Chapter {
        name: "Your profile",
        title: "What you plan stays with you",
        lead: "Your saved modules, your timetable, “My studies” and your programme belong to a profile, and the profile is kept locally on your device. Betula does not ask for your name or an e-mail address.",
        points: [
            Point { title: "Several profiles", text: "Two degree programmes, or a plan to try things out: every profile has its own saved modules, timetable and studies." },
            Point { title: "A backup as a file", text: "A profile can be saved as a file and loaded again. The file is made in your browser." },
            Point { title: "Empty and delete", text: "On the “Profile” page, at any time. Right after, “Undo” brings everything back." },
            Point { title: "Built to collect little", text: "No analytics, no cookies that recognise you, nothing from other servers. The server stands in Germany." },
        ],
    },
    zeros: [("0", "details about you"), ("0", "trackers and ads"), ("0", "requests to other servers"), ("7", "days, then the access log is deleted")],
    on_device: "On your device",
    settings: "Language, colours, widths",
    catalog_copy: "Catalogue for offline",
    on_server: "On the server",
    for_everyone: "Catalogue and pages, the same for everyone",
    nothing_of_yours: "Nothing of yours",
    sealed_copy: "With Betula Sync: your profile, sealed",
    only_the_address: "The server only sees which page you open",

    sync: Chapter {
        name: "Betula Sync",
        title: "Phone, laptop, tablet: the same state everywhere",
        lead: "If you switch on Betula Sync for a profile, your devices sync as soon as something changes. The encryption happens on your device, with a key only your devices know.",
        points: [
            Point { title: "Scan a code, done", text: "One device shows a QR code, the other scans it. No password to remember." },
            Point { title: "The server cannot read anything", text: "It keeps sealed changes under random ids, with their size and time. It does not know what is in them." },
            Point { title: "Without a network too", text: "Changes wait on the device and go out once there is a network again. If two devices change the same place, Betula shows you both versions." },
            Point { title: "Off again at any time", text: "“End Sync” deletes the copy on the server at once, and every device keeps its own. And if all devices are gone, the recovery code brings the profile back." },
        ],
    },
    sync_phone: "Your phone",
    sync_laptop: "Your laptop",
    sync_server: "Betula's server",
    sync_change: "Datenbanken saved",
    sync_sealed: "sealed",
    sync_knows: "knows only: random id, size, time",
    sync_ways: ["QR code or link", "Recovery code", "directly from device to device when both are online"],

    sharing: Chapter {
        name: "Sharing",
        title: "Share what you want, and only that",
        lead: "Your timetable for the study group, a collection for a fellow student, the calendar for your phone: you choose what comes along, who can open it and for how long.",
        points: [
            Point { title: "A copy of now", text: "The link carries the plan itself. Whoever opens it can add its modules; a messenger shows a preview of it." },
            Point { title: "Or always current", text: "With Betula Sync others follow your calendar or your list and see every change, until you stop sharing." },
            Point { title: "For exactly one person", text: "A one-time link holds for whoever opens it first, and for 7 days at most. Until then you can withdraw it." },
            Point { title: "Into any calendar", text: "As a subscription for Apple Calendar, Google Calendar or Outlook that fetches new dates by itself, or as an .ics file." },
        ],
    },
    share_lines: ["A copy of now, in the link itself", "Every change arrives, until you stop", "For one person, 7 days at most"],
    share_always: "Modules, groups, events you changed",

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
        items: &["Catalogue as an SQLite database in the browser", "Search and filters in milliseconds", "No loading times between pages", "Offline and installable as an app", "Your profile: saved modules, studies, timetable"],
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
            Point { title: "As an app on your phone", text: "Betula installs on a phone, tablet or computer and then starts with its own icon, even without a network." },
            Point { title: "German and English", text: "Every page at its own address; module titles and descriptions are shown as BTU writes them." },
            Point { title: "Light and dark", text: "Follows your system or your choice, down to the birch, whose leaves change with the seasons." },
            Point { title: "Keys and commands", text: "/ searches, Ctrl+K opens the commands, ? shows every key. Every key has its button: nothing works by keyboard alone." },
        ],
    },
    phone_alt: "The page of a module on a phone",

    to_catalog: "To the module catalogue",
    to_bookmarks: "To your saved modules",
    to_programs: "To the degree programmes",
    to_study: "To “My studies”",
    to_studyplan: "To the timetable",
    to_profile: "To your profile",
    to_privacy: "To the privacy policy",
    quoted_in_catalog: "In the module catalogue",
    quoted_week_a: "week A",
    quoted_one_clash_per_week: "1 clash per week",
    quoted_copy_link: "Copy link to share",
    quoted_download: "Download .ics",
    quoted_apple: "Apple Calendar",
    quoted_google: "Google Calendar",
    quoted_collections: "Collections",
    quoted_planned: "Planned",
    quoted_passed: "Passed",
    quoted_fixed: "Fixed copy",
    quoted_live: "Stays current",
    quoted_once: "One-time link",
    quoted_along: "What comes along",
    quoted_own: "Own events",
    quoted_notes: "Notes on events",
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Every language has the same chapters with as much in them, and the words written out
    /// read as they should.
    #[test]
    fn the_languages_match() {
        assert_eq!((DE.without_js.items.len(), DE.with_js.items.len()), (EN.without_js.items.len(), EN.with_js.items.len()));
        for t in [&DE, &EN] {
            let chapters = [&t.flow, &t.filters, &t.marks, &t.plans, &t.study, &t.timetable, &t.profile, &t.sync, &t.sharing, &t.javascript, &t.data, &t.devices];
            let mut names: Vec<&str> = chapters.iter().map(|chapter| chapter.name).collect();
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), chapters.len(), "{names:?}");
            for chapter in chapters {
                assert!(chapter.points.iter().all(|point| !point.title.is_empty() && point.text.ends_with(['.', '“', '”'])), "{}", chapter.name);
            }
        }
        assert_eq!(((DE.example)(206, "206"), (EN.plans_of)("115")), ("Diese Auswahl im Katalog: 206 Module".to_string(), "of 115 in their current examination regulations".to_string()));
        assert_eq!(((DE.example)(1, "1"), (EN.example)(1, "1"), (EN.example)(1206, "1,206")), ("Diese Auswahl im Katalog: 1 Modul".to_string(), "This selection in the catalogue: 1 module".to_string(), "This selection in the catalogue: 1,206 modules".to_string()));
    }

    /// No chapter speaks of an account: there is none (owner, 2026-11-18), and the pages say what
    /// there is instead, a profile kept on the device.
    #[test]
    fn no_chapter_speaks_of_an_account() {
        for t in [&DE, &EN] {
            let chapters = [&t.flow, &t.filters, &t.marks, &t.plans, &t.study, &t.timetable, &t.profile, &t.sync, &t.sharing, &t.javascript, &t.data, &t.devices];
            for chapter in chapters {
                let mut text = format!("{} {} {}", chapter.name, chapter.title, chapter.lead);
                for point in &chapter.points {
                    text.push_str(&format!(" {} {}", point.title, point.text));
                }
                let text = text.to_lowercase();
                assert!(!text.contains("konto") && !text.contains("konten") && !text.contains("account"), "{}", chapter.name);
            }
            assert!(t.zeros.iter().all(|(_, what)| !what.to_lowercase().contains("kont") && !what.to_lowercase().contains("account")));
        }
    }
}
