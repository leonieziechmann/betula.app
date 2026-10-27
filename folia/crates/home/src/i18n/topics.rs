//! Texts of the pages of „Über Betula" on its topics (`home/topics.rs`; owner, 2026-10-27: „dass
//! die Über Betula Seite geändert wird. Das soll aufgeteilt werden in mehrere kontent pages …
//! Wovon auch ein wenig auf der Landing page verlinkt werden kann"): each topic's name, the
//! heading of its page and the line under it, what search engines show of it, and the hint under
//! its name where a tile leads to it. What the pages say beyond that is what „Über Betula" said
//! (`home`, `home_detail`): the chapters of „Betula im Detail" and the questions, moved, not
//! rewritten.

use folia_routes::url::AboutTopic;

pub struct Texts {
    /// „Mehr über Betula": the tiles that lead to the topics, on „Über Betula", under a topic's
    /// page and on the start page, and the line under it.
    pub more: &'static str,
    pub more_lead: &'static str,
    /// „Alles über Betula": the way to „Über Betula" in the head of the tiles.
    pub all_about: &'static str,
    /// The way back to „Über Betula" over a topic's heading, and the name of the row of topics.
    pub back: &'static str,
    pub topics: &'static str,
    pub finding: Topic,
    pub planning: Topic,
    pub data: Topic,
    pub technology: Topic,
    pub questions: Topic,
}

/// A topic: its name (a tile, the tab, the row of topics), the heading of its page and the line
/// under it, its title and description for search engines, and the hint under its name on a tile.
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
    more_lead: "Was Betula kann, wie es gebaut ist, und Antworten auf häufige Fragen.",
    all_about: "Alles über Betula",
    back: "Über Betula",
    topics: "Themen",
    finding: Topic {
        name: "Module finden",
        title: "Vom Suchbegriff zum passenden Modul",
        lead: "Suche, Vorschau und Filter: Wie du aus allen Modulen der BTU das findest, das zu deinem Studium, deinem Semester und deiner Woche passt.",
        seo_title: "Module finden · Suche und Filter im Modulkatalog der BTU Cottbus-Senftenberg",
        description: "Wie du in Betula Module der BTU Cottbus-Senftenberg findest: Suche nach Titel oder Modulnummer, die Vorschau neben der Liste, Filter nach Studiengang, Turnus, Prüfung, Sprache, Standort und mehr.",
        hint: "Suche, Vorschau und alle Filter",
    },
    planning: Topic {
        name: "Studium planen",
        title: "Regelstudienplan und Stundenplan",
        lead: "Welche Module in welches Semester gehören, aus den Prüfungsordnungen gelesen, und dein Semester als Woche, in der du siehst, was sich wirklich überschneidet.",
        seo_title: "Studium planen · Regelstudienpläne und Stundenplan für die BTU Cottbus-Senftenberg",
        description: "Regelstudienpläne aus den Prüfungsordnungen der BTU Cottbus-Senftenberg und ein Stundenplan mit echten Überschneidungen, Übungsgruppen zur Wahl, Prüfungsterminen und Kalender-Abo.",
        hint: "Regelstudienpläne und Stundenplan",
    },
    data: Topic {
        name: "Deine Daten",
        title: "Ohne Konto, und woher die Daten kommen",
        lead: "Was du planst und merkst, bleibt in deinem Browser. Und was Betula über die Module weiß, kommt von der BTU und wird geprüft, bevor es erscheint.",
        seo_title: "Deine Daten · Betula ohne Konto, und woher der Modulkatalog seine Daten hat",
        description: "Betula braucht kein Konto: Merkliste, Stundenplan und Studiengang liegen in deinem Browser. Woher die Daten des Modulkatalogs kommen, wie oft sie gelesen und wie sie geprüft werden.",
        hint: "Ohne Konto, und die Quellen",
    },
    technology: Topic {
        name: "Technik",
        title: "Läuft überall",
        lead: "Mit und ohne JavaScript, auch offline, am Rechner und am Handy, auf Deutsch und auf Englisch.",
        seo_title: "Technik · wie Betula gebaut ist: mit und ohne JavaScript, offline, auf jedem Gerät",
        description: "Wie Betula gebaut ist: Seiten als fertiges HTML vom Server, eine App aus Rust und WebAssembly, die auch offline funktioniert, für Rechner und Handy, auf Deutsch und Englisch.",
        hint: "Offline, ohne JavaScript, jedes Gerät",
    },
    questions: Topic {
        name: "Häufige Fragen",
        title: "Fragen und Antworten",
        lead: "Über Betula, zum Benutzen und fürs Studium an der BTU.",
        seo_title: "Häufige Fragen · Betula, der inoffizielle Modulkatalog der BTU Cottbus-Senftenberg",
        description: "Antworten auf häufige Fragen zu Betula und zum Studium an der BTU Cottbus-Senftenberg: was Betula ist und kostet, wie du Module findest und planst, und was Erstsemester wissen sollten.",
        hint: "Über Betula und fürs Studium",
    },
};

pub const EN: Texts = Texts {
    more: "More about Betula",
    more_lead: "What Betula does, how it is built, and answers to common questions.",
    all_about: "All about Betula",
    back: "About Betula",
    topics: "Topics",
    finding: Topic {
        name: "Finding modules",
        title: "From a search term to the right module",
        lead: "Search, preview and filters: how to find, among all of BTU's modules, the one that fits your programme, your semester and your week.",
        seo_title: "Finding modules · search and filters in the module catalogue of BTU Cottbus-Senftenberg",
        description: "How to find modules of BTU Cottbus-Senftenberg in Betula: search by title or module number, the preview beside the list, filters by programme, offering, assessment, language, location and more.",
        hint: "Search, preview and every filter",
    },
    planning: Topic {
        name: "Planning your studies",
        title: "Standard study plan and timetable",
        lead: "Which modules belong in which semester, read from the examination regulations, and your semester as a week that shows what really clashes.",
        seo_title: "Planning your studies · standard study plans and timetable for BTU Cottbus-Senftenberg",
        description: "Standard study plans from the examination regulations of BTU Cottbus-Senftenberg, and a timetable with real clashes, tutorial groups to choose from, exam dates and a calendar subscription.",
        hint: "Study plans and timetable",
    },
    data: Topic {
        name: "Your data",
        title: "No account, and where the data comes from",
        lead: "What you plan and save stays in your browser. And what Betula knows about the modules comes from BTU and is checked before it appears.",
        seo_title: "Your data · Betula without an account, and where the module catalogue gets its data",
        description: "Betula needs no account: saved modules, timetable and programme live in your browser. Where the catalogue's data comes from, how often it is read and how it is checked.",
        hint: "No account, and the sources",
    },
    technology: Topic {
        name: "Technology",
        title: "Runs everywhere",
        lead: "With and without JavaScript, offline as well, on a computer and on a phone, in German and in English.",
        seo_title: "Technology · how Betula is built: with and without JavaScript, offline, on any device",
        description: "How Betula is built: pages as finished HTML from the server, an app in Rust and WebAssembly that also works offline, for computer and phone, in German and English.",
        hint: "Offline, without JavaScript, any device",
    },
    questions: Topic {
        name: "Questions",
        title: "Questions and answers",
        lead: "About Betula, using it, and studying at BTU.",
        seo_title: "Questions · Betula, the unofficial module catalogue of BTU Cottbus-Senftenberg",
        description: "Answers to common questions about Betula and studying at BTU Cottbus-Senftenberg: what Betula is and costs, how to find and plan modules, and what first-year students should know.",
        hint: "About Betula and your studies",
    },
};
