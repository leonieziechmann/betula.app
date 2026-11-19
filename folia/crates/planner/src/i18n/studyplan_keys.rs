//! Texts of the Stundenplan with the keyboard (folia/design/tastatur/konzept.html, „Die anderen
//! Seiten"; enhance.js, „the keyboard"): the sidebar as a mode and as a command of „Gehe zu".

pub struct Texts {
    /// The sidebar as a command of „Gehe zu" (F), and the way out of it that a screen reader hears
    /// at either of its ends.
    pub sidebar: &'static str,
    pub leave: &'static str,
    /// The sidebar's section of „Teilen …", by its name.
    pub share: &'static str,
    /// In „Befehle", beside the week shown of A, B and A/B.
    pub shown: &'static str,
}

pub const DE: Texts = Texts {
    sidebar: "Kalender und Export",
    leave: "Escape führt zur Woche",
    share: "Teilen",
    shown: "gezeigt",
};

pub const EN: Texts = Texts {
    sidebar: "Calendars and export",
    leave: "Escape leads to the week",
    share: "Share",
    shown: "shown",
};
