//! Texts of the pages about Betula, one per topic (`home/topics.rs`; owner, 2026-10-27: „dass
//! die Über Betula Seite geändert wird. Das soll aufgeteilt werden in mehrere kontent pages …
//! Wovon auch ein wenig auf der Landing page verlinkt werden kann"): each topic's name, the
//! heading of its page and the line under it, what search engines show of it, and the hint under
//! its name where a tile leads to it. What the pages say beyond that is `home` and `home_detail`:
//! the first panel, the chapters and the questions, as „Über Betula" had them until 2026-11-18 and
//! brought up to what Betula is since (owner: the pages „sind alle veraltet und könnten etwas
//! interessanter werden"; no topic speaks of an account, there being none).

use folia_routes::url::AboutTopic;

pub struct Texts {
    /// „Mehr über Betula": the tiles that lead to the topics, under a topic's page and on the
    /// start page, and the line under it.
    pub more: &'static str,
    pub more_lead: &'static str,
    /// The name of the row of topics at the head of a topic's page.
    pub topics: &'static str,
    pub overview: Topic,
    pub finding: Topic,
    pub planning: Topic,
    pub data: Topic,
    pub technology: Topic,
    pub questions: Topic,
}

/// A topic: its name (a tile, the tab, the row of topics), the heading of its page and the line
/// under it, its title and description for search engines, and the hint under its name on a tile.
/// The overview's heading names the university, which does not break (`topics::unbroken`); the
/// start page says it as well, beside Betula's name.
pub struct Topic {
    pub name: &'static str,
    pub title: &'static str,
    pub lead: &'static str,
    pub seo_title: &'static str,
    pub description: &'static str,
    pub hint: &'static str,
}

impl Texts {
    pub fn of(&self, topic: AboutTopic) -> &Topic {
        match topic {
            AboutTopic::Overview => &self.overview,
            AboutTopic::Finding => &self.finding,
            AboutTopic::Planning => &self.planning,
            AboutTopic::Data => &self.data,
            AboutTopic::Technology => &self.technology,
            AboutTopic::Questions => &self.questions,
        }
    }
}

pub const DE: Texts = Texts {
    more: "Mehr über Betula",
    more_lead: "Was Betula kann, wo deine Daten liegen, und Antworten auf häufige Fragen.",
    topics: "Themen",
    overview: Topic {
        name: "Überblick",
        title: "Alle Module und Studiengänge der BTU Cottbus-Senftenberg, an einem Ort.",
        lead: "Module mit Inhalten, Voraussetzungen, Prüfung und Terminen, Studiengänge mit dem Regelstudienplan aus ihrer Prüfungsordnung, dazu dein Studium und dein Stundenplan: durchsuchen, planen, im Blick behalten, statt Modulhandbücher zu wälzen.",
        seo_title: "Betula im Überblick · der inoffizielle Modulkatalog für die BTU Cottbus-Senftenberg",
        description: "Was Betula ist und kann: alle Module und Studiengänge der BTU Cottbus-Senftenberg durchsuchen und filtern, Regelstudienpläne aus den Prüfungsordnungen, dein Studium Semester für Semester, ein Stundenplan mit Kalender-Abo. Kostenlos, auf all deinen Geräten, auch offline und ohne JavaScript.",
        hint: "Was Betula ist und kann",
    },
    finding: Topic {
        name: "Module finden",
        title: "Vom Suchbegriff zum passenden Modul",
        lead: "Suche, Vorschau, Filter und Merkliste: Wie du aus allen Modulen der BTU das findest, das zu deinem Studium, deinem Semester und deiner Woche passt, und es nicht wieder aus den Augen verlierst.",
        seo_title: "Module finden · Suche und Filter im Modulkatalog der BTU Cottbus-Senftenberg",
        description: "Wie du in Betula Module der BTU Cottbus-Senftenberg findest: Suche nach Titel, Modulnummer, Kürzel oder Thema, die Vorschau neben der Liste, Filter nach Studiengang, Turnus, Prüfung, Sprache, Standort und mehr, und die Merkliste mit Sammlungen und Notizen.",
        hint: "Suche, Filter und Merkliste",
    },
    planning: Topic {
        name: "Studium planen",
        title: "Vom Regelstudienplan bis zur nächsten Vorlesung",
        lead: "Was die Prüfungsordnung vorsieht, was du daraus machst und wie deine Woche aussieht: Regelstudienplan, „Mein Studium“ und Stundenplan greifen ineinander.",
        seo_title: "Studium planen · Regelstudienpläne und Stundenplan für die BTU Cottbus-Senftenberg",
        description: "Regelstudienpläne aus den Prüfungsordnungen der BTU Cottbus-Senftenberg, „Mein Studium“ mit den Leistungspunkten jedes Bereichs, und ein Stundenplan mit echten Überschneidungen, Übungsgruppen zur Wahl, Prüfungsterminen und Kalender-Abo.",
        hint: "Regelstudienplan, Studium, Stundenplan",
    },
    data: Topic {
        name: "Deine Daten",
        title: "Deine Daten bleiben bei dir",
        lead: "Was du merkst und planst, liegt in deinem Profil auf deinem Gerät, und mit Betula Sync verschlüsselt auf all deinen Geräten. Was Betula über die Module weiß, kommt von der BTU und wird geprüft, bevor es erscheint.",
        seo_title: "Deine Daten · Profile, Betula Sync und woher der Modulkatalog seine Daten hat",
        description: "Wo deine Daten bei Betula liegen: im Profil auf deinem Gerät, mit Betula Sync Ende-zu-Ende-verschlüsselt auf all deinen Geräten. Wie du teilst, was du teilen willst, und woher die Daten des Modulkatalogs kommen.",
        hint: "Profil, Betula Sync, Quellen",
    },
    technology: Topic {
        name: "Technik",
        title: "Läuft überall",
        lead: "Mit und ohne JavaScript, auch offline, am Rechner und als App auf dem Handy, auf Deutsch und auf Englisch.",
        seo_title: "Technik · wie Betula gebaut ist: mit und ohne JavaScript, offline, auf jedem Gerät",
        description: "Wie Betula gebaut ist: Seiten als fertiges HTML vom Server, eine App aus Rust und WebAssembly, die auch offline funktioniert, für Rechner und Handy, auf Deutsch und Englisch.",
        hint: "Offline, ohne JavaScript, jedes Gerät",
    },
    questions: Topic {
        name: "Häufige Fragen",
        title: "Fragen und Antworten",
        lead: "Über Betula, zum Benutzen und fürs Studium an der BTU.",
        seo_title: "Häufige Fragen · Betula, der inoffizielle Modulkatalog der BTU Cottbus-Senftenberg",
        description: "Antworten auf häufige Fragen zu Betula und zum Studium an der BTU Cottbus-Senftenberg: was Betula ist und kostet, wo deine Daten liegen, wie du Module findest und planst, und was Erstsemester wissen sollten.",
        hint: "Über Betula und fürs Studium",
    },
};

pub const EN: Texts = Texts {
    more: "More about Betula",
    more_lead: "What Betula does, where your data is kept, and answers to common questions.",
    topics: "Topics",
    overview: Topic {
        name: "Overview",
        title: "Every module and degree programme of BTU Cottbus-Senftenberg, in one place.",
        lead: "Modules with their contents, prerequisites, assessment and dates, degree programmes with the standard study plan from their examination regulations, and your studies and your timetable: to search, to plan, to keep in view, instead of wading through module handbooks.",
        seo_title: "Betula at a glance · the unofficial module catalogue for BTU Cottbus-Senftenberg",
        description: "What Betula is and does: search and filter every module and degree programme of BTU Cottbus-Senftenberg, standard study plans from the examination regulations, your studies semester by semester, a timetable with a calendar subscription. Free, on all your devices, works offline and without JavaScript.",
        hint: "What Betula is and does",
    },
    finding: Topic {
        name: "Finding modules",
        title: "From a search term to the right module",
        lead: "Search, preview, filters and saved modules: how to find, among all of BTU's modules, the one that fits your programme, your semester and your week, and not lose sight of it again.",
        seo_title: "Finding modules · search and filters in the module catalogue of BTU Cottbus-Senftenberg",
        description: "How to find modules of BTU Cottbus-Senftenberg in Betula: search by title, module number, abbreviation or topic, the preview beside the list, filters by programme, offering, assessment, language, location and more, and saved modules with collections and notes.",
        hint: "Search, filters and saved modules",
    },
    planning: Topic {
        name: "Planning your studies",
        title: "From the standard study plan to your next lecture",
        lead: "What the examination regulations provide for, what you make of it and what your week looks like: the standard study plan, “My studies” and the timetable work hand in hand.",
        seo_title: "Planning your studies · standard study plans and timetable for BTU Cottbus-Senftenberg",
        description: "Standard study plans from the examination regulations of BTU Cottbus-Senftenberg, “My studies” with the credit points of every area, and a timetable with real clashes, tutorial groups to choose from, exam dates and a calendar subscription.",
        hint: "Study plan, studies, timetable",
    },
    data: Topic {
        name: "Your data",
        title: "Your data stays with you",
        lead: "What you save and plan is kept in your profile on your device, and with Betula Sync encrypted on all your devices. What Betula knows about the modules comes from BTU and is checked before it appears.",
        seo_title: "Your data · profiles, Betula Sync, and where the module catalogue gets its data",
        description: "Where your data is kept with Betula: in your profile on your device, and with Betula Sync end-to-end encrypted on all your devices. How you share what you want to share, and where the catalogue's data comes from.",
        hint: "Profile, Betula Sync, sources",
    },
    technology: Topic {
        name: "Technology",
        title: "Runs everywhere",
        lead: "With and without JavaScript, offline as well, on a computer and as an app on your phone, in German and in English.",
        seo_title: "Technology · how Betula is built: with and without JavaScript, offline, on any device",
        description: "How Betula is built: pages as finished HTML from the server, an app in Rust and WebAssembly that also works offline, for computer and phone, in German and English.",
        hint: "Offline, without JavaScript, any device",
    },
    questions: Topic {
        name: "Questions",
        title: "Questions and answers",
        lead: "About Betula, using it, and studying at BTU.",
        seo_title: "Questions · Betula, the unofficial module catalogue of BTU Cottbus-Senftenberg",
        description: "Answers to common questions about Betula and studying at BTU Cottbus-Senftenberg: what Betula is and costs, where your data is kept, how to find and plan modules, and what first-year students should know.",
        hint: "About Betula and your studies",
    },
};
