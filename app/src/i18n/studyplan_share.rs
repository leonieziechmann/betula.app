//! Texts of a Stundenplan handed on by a link (`pages/studyplan/share.rs`): „Link zum Teilen
//! kopieren" in the sidebar, and the offer on the page the link opens.

pub struct Texts {
    // ---- the link ----
    /// „Link zum Teilen kopieren", with what it carries under it …
    pub copy_link: &'static str,
    pub with_modules: &'static str,
    /// … and its tooltip.
    pub link_title: &'static str,
    /// The tooltip of the greyed-out link with nothing planned.
    pub nothing_planned: &'static str,

    // ---- the offer ----
    /// „Geteilter Stundenplan · WiSe 2026/27. ": what the offer begins with (the semester's name).
    pub shared: fn(&str) -> String,
    /// After the modules of a plan none of which is planned yet: „In deinen Stundenplan
    /// übernehmen?" …
    pub take_all: &'static str,
    /// … some of which are (how many are not): „2 davon fehlen in deinem Stundenplan.
    /// Übernehmen?" …
    pub take_missing: fn(usize) -> String,
    /// … every one of which is (the modules' short names, joined with „, "): „MIT-1, AuP. Alles
    /// davon steht schon in deinem Stundenplan." …
    pub held: fn(&str) -> String,
    /// … of another semester than the one the page shows …
    pub elsewhere: fn(&str) -> String,
    /// … or none of whose modules the catalog knows.
    pub unknown: &'static str,
    /// The offer's buttons: take the modules over, leave the plan as it is, or only close the
    /// offer where there is nothing to take.
    pub take: &'static str,
    pub dismiss: &'static str,
    pub ok: &'static str,
    /// How the note after „Übernehmen" begins: „Aus dem Link übernommen: 6 Module". It must differ
    /// from the other notes' beginnings (`studyplan_import::imported`, `studyplan_side::cleared`).
    pub taken: &'static str,
}

pub const DE: Texts = Texts {
    copy_link: "Link zum Teilen kopieren",
    with_modules: "mit den Modulen dieses Semesters",
    link_title: "Der Link trägt das Semester und die geplanten Module: Wer ihn öffnet, kann sie in den eigenen Stundenplan übernehmen, und die Vorschau in Messengern zeigt sie.",
    nothing_planned: "Nichts geplant",

    shared: |semester| format!("Geteilter Stundenplan · {semester}. "),
    take_all: "In deinen Stundenplan übernehmen?",
    take_missing: |n| format!("{} davon fehlen in deinem Stundenplan. Übernehmen?", if n == 1 { "Eins".to_string() } else { n.to_string() }),
    held: |names| format!("{names}. Alles davon steht schon in deinem Stundenplan."),
    elsewhere: |names| format!("{names}. Dein Stundenplan zeigt ein anderes Semester."),
    unknown: "Seine Module stehen nicht (mehr) im Modulkatalog der BTU.",
    take: "Übernehmen",
    dismiss: "Verwerfen",
    ok: "In Ordnung",
    taken: "Aus dem Link übernommen: ",
};

pub const EN: Texts = Texts {
    copy_link: "Copy link to share",
    with_modules: "with this semester's modules",
    link_title: "The link carries the semester and the planned modules: whoever opens it can add them to their own timetable, and previews in messengers show them.",
    nothing_planned: "Nothing planned",

    shared: |semester| format!("Shared timetable · {semester}. "),
    take_all: "Add them to your timetable?",
    take_missing: |n| match n {
        1 => "One of them is missing from your timetable. Add it?".to_string(),
        n => format!("{n} of them are missing from your timetable. Add them?"),
    },
    held: |names| format!("{names}. All of them are already in your timetable."),
    elsewhere: |names| format!("{names}. Your timetable shows another semester."),
    unknown: "Its modules are not (or no longer) in the BTU's module catalogue.",
    take: "Add",
    dismiss: "Dismiss",
    ok: "OK",
    taken: "Added from the link: ",
};
