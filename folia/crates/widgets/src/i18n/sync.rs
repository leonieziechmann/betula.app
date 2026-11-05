//! Texts of the hints of Betula Sync (`syncnote.rs`): what two devices of a profile did to one
//! thing at the same time.

pub struct Texts {
    /// The hint at a note typed on two devices at once: its heading, the line before the other
    /// version, and its two answers.
    pub note_heading: &'static str,
    pub note_other: &'static str,
    /// The other version deleted the note.
    pub note_other_none: &'static str,
    pub take_other: &'static str,
    pub keep: &'static str,
    /// The hint at a module moved on two devices at once: „„Mathematik I“ wurde auf zwei Geräten
    /// verschoben und steht jetzt in WiSe 2026/27 und SoSe 2027."; the values are the module's
    /// name and the semesters, joined (`and`).
    pub moved: fn(&str, &str) -> String,
    /// „und" between the last two semesters of a list.
    pub and: &'static str,
    /// „Nur in WiSe 2026/27": keeps the module in that semester alone.
    pub only_in: fn(&str) -> String,
    pub keep_all: &'static str,
}

pub const DE: Texts = Texts {
    note_heading: "Gleichzeitig auf einem anderen Gerät geändert",
    note_other: "Dort stand:",
    note_other_none: "Dort wurde die Notiz gelöscht.",
    take_other: "Diese Fassung übernehmen",
    keep: "Behalten",
    moved: |module, semesters| format!("„{module}“ wurde auf zwei Geräten gleichzeitig verschoben und steht jetzt in {semesters}."),
    and: "und",
    only_in: |semester| format!("Nur in {semester}"),
    keep_all: "Alle behalten",
};

pub const EN: Texts = Texts {
    note_heading: "Changed on another device at the same time",
    note_other: "There it said:",
    note_other_none: "There the note was deleted.",
    take_other: "Take this version",
    keep: "Keep",
    moved: |module, semesters| format!("“{module}” was moved on two devices at the same time and is now in {semesters}."),
    and: "and",
    only_in: |semester| format!("Only in {semester}"),
    keep_all: "Keep all",
};
