//! Texts of what search engines and link previews read (`seo.rs`).

pub struct Texts {
    /// What the picture of link previews shows (`folia/assets/og-<season>.png`), for those who
    /// cannot see it.
    pub image_alt: &'static str,
}

pub const DE: Texts = Texts {
    image_alt: "Betula: alle Module und Studiengänge der BTU Cottbus-Senftenberg. Durchsuchen, filtern, Studium planen. Inoffizieller Modulkatalog.",
};

pub const EN: Texts = Texts {
    image_alt: "Betula: every module and degree programme of BTU Cottbus-Senftenberg. Search, filter, plan your studies. Unofficial module catalogue.",
};
