//! Texts of the profiles wherever a profile is named (`profiles.rs`).

pub struct Texts {
    /// A profile without a name: „Profil 2"; the value is its number.
    pub unnamed: fn(u16) -> String,
    /// The line under a profile without „Mein Studiengang" (`program_lines`).
    pub no_program: &'static str,
    /// The mark beside a profile that Betula Sync keeps (`SyncMark`): its tooltip, and its name
    /// for those who do not see it.
    pub synced: &'static str,
}

pub const DE: Texts = Texts { unnamed: |n| format!("Profil {n}"), no_program: "Kein Studiengang", synced: "Mit Betula Sync synchronisiert" };

pub const EN: Texts = Texts { unnamed: |n| format!("Profile {n}"), no_program: "No degree programme", synced: "Synced with Betula Sync" };
