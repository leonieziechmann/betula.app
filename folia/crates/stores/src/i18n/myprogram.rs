//! Texts of „Mein Studiengang" (`myprogram.rs`).

pub struct Texts {
    /// What the visitor's own program is called wherever the app names it: „Mein Studiengang".
    pub mine: &'static str,
}

pub const DE: Texts = Texts { mine: "Mein Studiengang" };

pub const EN: Texts = Texts { mine: "My programme" };
