//! Texts of „Mein Studiengang" (`myprogram.rs`).

pub struct Texts {
    /// The button on a program's page, pressed: „Mein Studiengang" …
    pub mine: &'static str,
    /// … and not: „Als meinen Studiengang setzen".
    pub set_mine: &'static str,
    /// The pressed button's title: a click takes the program away again.
    pub unset_title: &'static str,
    /// The title of the button that would replace what is kept: „Ersetzt: Informatik B.Sc. · PO
    /// 2008" (another program), „Ersetzt: PA und IoT" (another plan of this one).
    pub replaces: fn(&str) -> String,
}

pub const DE: Texts = Texts {
    mine: "Mein Studiengang",
    set_mine: "Als meinen Studiengang setzen",
    unset_title: "Dein Studiengang. Noch einmal hebt das auf",
    replaces: |kept| format!("Ersetzt: {kept}"),
};

pub const EN: Texts = Texts {
    mine: "My programme",
    set_mine: "Set as my programme",
    unset_title: "Your degree programme. Click again to undo",
    replaces: |kept| format!("Replaces: {kept}"),
};
