//! Texts of „Offen" and the choice in focus (`studyplan/open.rs`; folia/design/kalender): the list
//! of what the plan leaves to choose, the bar over the week, the sheet of a phone with the groups
//! as tiles, and „Vergleichen" with each group's day.

pub struct Texts {
    // ---- „Offen" ----
    /// The heading of the list.
    pub open: &'static str,
    /// The phone's line: „2 Wahlen offen".
    pub open_count: fn(usize) -> String,
    /// A choice's line: „eine von 3 anhaken" …
    pub tick_one_of: fn(usize) -> String,
    /// … of a choice among a module's events: „1 von 4 wählen" …
    pub take_of: fn(usize, usize) -> String,
    /// … and where none of its options is free.
    pub none_free: &'static str,
    /// What a click on a line does: the choice into focus, the one in focus, the module beside
    /// the plan (a choice among a module's events is decided there).
    pub choose: &'static str,
    pub in_focus: &'static str,
    pub in_module: &'static str,

    // ---- the choice in focus ----
    /// Its state: „noch keine angehakt", „2 von 3 angehakt".
    pub none_yet: &'static str,
    pub ticked_of: fn(usize, usize) -> String,
    /// Whether the groups fit, and what to do: „Alle 3 passen." and „Hake eine oder mehrere an." …
    pub all_fit: fn(usize) -> String,
    pub tick_hint: &'static str,
    /// … „Nur Do 09:15 passt, die anderen überschneiden sich." …
    pub only_fits: fn(&str) -> String,
    /// … „2 von 7 passen, die anderen überschneiden sich." …
    pub some_fit: fn(usize, usize) -> String,
    /// … „Keine passt ohne Überschneidung." …
    pub none_fit: &'static str,
    /// … and where some meet the plan, that ticking goes all the same (Betula does not say no).
    pub anyway: &'static str,
    /// What an untouched choice shows meanwhile: „Bis du eine anhakst, stehen alle 3 gestrichelt
    /// im Stundenplan."
    pub open_note: fn(usize) -> String,
    /// The buttons: „Weiter: Übung MIT1", „Vergleichen", the tiles again, „Fertig", „Schließen".
    pub next: fn(&str) -> String,
    pub compare: &'static str,
    /// „Vergleichen" closed beside the week of a wide page.
    pub compare_close: &'static str,
    pub tiles: &'static str,
    pub done: &'static str,
    pub close: &'static str,
    /// The word over the tiles: „Gruppen".
    pub groups: &'static str,
    /// From seven groups on: „+2 weitere", „weniger".
    pub more: fn(usize) -> String,
    pub further: &'static str,
    pub fewer: &'static str,

    // ---- a group, as a tile and in „Vergleichen" ----
    /// „Pilling leitet"
    pub leads: fn(&str) -> String,
    /// „am 04.11. in LG1A/304" is QIS's; „+1 Raum" where a group meets in two rooms.
    pub another_room: &'static str,
    /// What it does to its day: „allein am Mittwoch" …
    pub alone: fn(&str) -> String,
    /// … „45 min nach Ü MIT1" …
    pub after: fn(&str, &str) -> String,
    /// … „45 min bis VL MIT1" …
    pub until: fn(&str, &str) -> String,
    /// … „danach frei" …
    pub free_after: &'static str,
    /// … „anderer Campus" …
    pub other_campus: &'static str,
    /// … „trifft VL MIT1".
    pub meets: fn(&str) -> String,
    /// What a tile and a line say to a screen reader and in a tooltip: „passt", „trifft …",
    /// „angehakt".
    pub fits: &'static str,
    pub ticked: &'static str,
}

/// „45 min", „1 h 15", „3 h".
pub fn gap(minutes: u16) -> String {
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m}"),
    }
}

pub const DE: Texts = Texts {
    open: "Offen",
    open_count: |n| match n {
        1 => "1 Wahl offen".to_string(),
        n => format!("{n} Wahlen offen"),
    },
    tick_one_of: |n| format!("eine von {n} anhaken"),
    take_of: |take, of| format!("{take} von {of} wählen"),
    none_free: "keine passt ohne Überschneidung",
    choose: "Wählen",
    in_focus: "im Fokus",
    in_module: "Im Modul",

    none_yet: "noch keine angehakt",
    ticked_of: |ticked, of| format!("{ticked} von {of} angehakt"),
    tick_hint: "Hake eine oder mehrere an.",
    all_fit: |n| match n {
        2 => "Beide passen.".to_string(),
        n => format!("Alle {n} passen."),
    },
    only_fits: |which| format!("Nur {which} passt, die anderen überschneiden sich."),
    some_fit: |fit, of| format!("{fit} von {of} passen, die anderen überschneiden sich."),
    none_fit: "Keine passt ohne Überschneidung.",
    anyway: "Anhaken geht trotzdem.",
    open_note: |n| format!("Bis du eine anhakst, stehen alle {n} gestrichelt im Stundenplan."),
    next: |what| format!("Weiter: {what}"),
    compare: "Vergleichen",
    compare_close: "Vergleich schließen",
    tiles: "Zurück zu den Kacheln",
    done: "Fertig",
    close: "Schließen",
    groups: "Gruppen",
    more: |n| format!("{n} weitere zeigen"),
    further: "weitere",
    fewer: "weniger",

    leads: |who| format!("{who} leitet"),
    another_room: "+1 Raum",
    alone: |day| format!("allein am {day}"),
    after: |gap, what| format!("{gap} nach {what}"),
    until: |gap, what| format!("{gap} bis {what}"),
    free_after: "danach frei",
    other_campus: "anderer Campus",
    meets: |what| format!("trifft {what}"),
    fits: "passt",
    ticked: "angehakt",
};

pub const EN: Texts = Texts {
    open: "Open",
    open_count: |n| match n {
        1 => "1 choice open".to_string(),
        n => format!("{n} choices open"),
    },
    tick_one_of: |n| format!("tick one of {n}"),
    take_of: |take, of| format!("choose {take} of {of}"),
    none_free: "none fits without a clash",
    choose: "Choose",
    in_focus: "in focus",
    in_module: "In the module",

    none_yet: "none ticked yet",
    ticked_of: |ticked, of| format!("{ticked} of {of} ticked"),
    tick_hint: "Tick one or more.",
    all_fit: |n| match n {
        2 => "Both fit.".to_string(),
        n => format!("All {n} fit."),
    },
    only_fits: |which| format!("Only {which} fits, the others clash."),
    some_fit: |fit, of| format!("{fit} of {of} fit, the others clash."),
    none_fit: "None fits without a clash.",
    anyway: "You can tick one all the same.",
    open_note: |n| format!("Until you tick one, all {n} stand dashed in the timetable."),
    next: |what| format!("Next: {what}"),
    compare: "Compare",
    compare_close: "Close the comparison",
    tiles: "Back to the tiles",
    done: "Done",
    close: "Close",
    groups: "Groups",
    more: |n| format!("Show {n} more"),
    further: "more",
    fewer: "fewer",

    leads: |who| format!("led by {who}"),
    another_room: "+1 room",
    alone: |day| format!("alone on {day}"),
    after: |gap, what| format!("{gap} after {what}"),
    until: |gap, what| format!("{gap} until {what}"),
    free_after: "free after it",
    other_campus: "another campus",
    meets: |what| format!("meets {what}"),
    fits: "fits",
    ticked: "ticked",
};
