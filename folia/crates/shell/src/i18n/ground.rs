//! Texts of the ground at the end of every page (`ground.rs`).

pub struct Texts {
    pub note: &'static str,
    /// The name of the navigation to the legal pages, for screen readers. „Über Betula" stood
    /// there as well until 2026-10-27 (owner: „nimm das bitte Über Betula aus dem Footer raus");
    /// the start page leads to it.
    pub legal: &'static str,
    pub imprint: &'static str,
    pub privacy: &'static str,
    /// „Daten vom 19.09.2026".
    pub data_of: fn(&str) -> String,
    pub source: &'static str,
}

pub const DE: Texts = Texts {
    note: "Betula ist ein inoffizielles Projekt und gehört nicht zur BTU.",
    legal: "Über diese Seite",
    imprint: "Impressum",
    privacy: "Datenschutz",
    data_of: |date| format!("Daten vom {date}"),
    source: "Quelle: BTU",
};

pub const EN: Texts = Texts {
    note: "Betula is an unofficial project and not part of BTU.",
    legal: "About this site",
    imprint: "Legal notice",
    privacy: "Privacy",
    data_of: |date| format!("Data as of {date}"),
    source: "Source: BTU",
};
