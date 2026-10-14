//! Texts of the profiles wherever a profile is named (`profiles.rs`).

pub struct Texts {
    /// A profile without a name: „Profil 2"; the value is its number.
    pub unnamed: fn(u16) -> String,
}

pub const DE: Texts = Texts { unnamed: |n| format!("Profil {n}") };

pub const EN: Texts = Texts { unnamed: |n| format!("Profile {n}") };
