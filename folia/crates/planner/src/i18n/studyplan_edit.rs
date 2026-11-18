//! Texts of the Stundenplan being edited (`studyplan/mod.rs`, `PlanCtx::edit`; owner, 2026-11-17:
//! „bei dem Kalender einen edit modus hinzuzufügen, weil man dadurch unnötiges ui ausblenden kann"):
//! the head's button, the mode's bar, the phone's dock, the quiet line that leads into the mode, what
//! a screen reader hears, and the mark of a slot whose series has hidden dates.

pub struct Texts {
    /// „Bearbeiten": the head's button (the pencil, `aria-pressed`) …
    pub edit: &'static str,
    /// … its tooltip and its name as a command: „Stundenplan bearbeiten"; pressed, while the plan is
    /// edited, „Bearbeiten beenden", which a click then does.
    pub edit_title: &'static str,
    pub edit_end: &'static str,
    /// The bar's name while editing: „Stundenplan bearbeiten".
    pub editing: &'static str,
    /// The phone's dock: „Bearbeiten", small under it the figures; the part a button that brings
    /// the module list into view („Zu den Modulen": its tooltip).
    pub dock: &'static str,
    pub to_list: &'static str,
    /// Name and tooltip of the quiet line „3 Wahlen offen ›" and of the phone's line while looking:
    /// „3 Wahlen offen: Stundenplan bearbeiten".
    pub open_way: fn(&str) -> String,
    /// The mode said to a screen reader: „Stundenplan bearbeiten: 3 Wahlen offen, 2 ausgeblendet",
    /// without figures „Stundenplan bearbeiten" …
    pub said: fn(&str) -> String,
    /// … and on leaving it: „Bearbeiten beendet".
    pub ended: &'static str,
    /// The mark of a slot whose series has hidden dates, and the sign of a module's row hidden in
    /// part: „1 Termin ausgeblendet", „3 Termine ausgeblendet".
    pub dates_hidden: fn(usize) -> String,
    /// „Gespeicherte Pläne": the plans saved before, in the sidebar while editing.
    pub saved_plans: &'static str,
}

pub const DE: Texts = Texts {
    edit: "Bearbeiten",
    edit_title: "Stundenplan bearbeiten",
    edit_end: "Bearbeiten beenden",
    editing: "Stundenplan bearbeiten",
    dock: "Bearbeiten",
    to_list: "Zu den Modulen",
    open_way: |figures| format!("{figures}: Stundenplan bearbeiten"),
    said: |figures| match figures.trim() {
        "" => "Stundenplan bearbeiten".to_string(),
        figures => format!("Stundenplan bearbeiten: {figures}"),
    },
    ended: "Bearbeiten beendet",
    dates_hidden: |n| match n {
        1 => "1 Termin ausgeblendet".to_string(),
        n => format!("{n} Termine ausgeblendet"),
    },
    saved_plans: "Gespeicherte Pläne",
};

pub const EN: Texts = Texts {
    edit: "Edit",
    edit_title: "Edit your timetable",
    edit_end: "Stop editing",
    editing: "Editing your timetable",
    dock: "Editing",
    to_list: "To the modules",
    open_way: |figures| format!("{figures}: edit your timetable"),
    said: |figures| match figures.trim() {
        "" => "Editing your timetable".to_string(),
        figures => format!("Editing your timetable: {figures}"),
    },
    ended: "Editing ended",
    dates_hidden: |n| match n {
        1 => "1 date hidden".to_string(),
        n => format!("{n} dates hidden"),
    },
    saved_plans: "Saved plans",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_is_said_with_its_figures_or_alone() {
        assert_eq!((DE.said)("3 Wahlen offen, 2 ausgeblendet"), "Stundenplan bearbeiten: 3 Wahlen offen, 2 ausgeblendet");
        assert_eq!((DE.said)(""), "Stundenplan bearbeiten");
        assert_eq!((EN.said)(""), "Editing your timetable");
        assert_eq!((DE.open_way)("3 Wahlen offen"), "3 Wahlen offen: Stundenplan bearbeiten");
        assert_eq!(((DE.dates_hidden)(1), (DE.dates_hidden)(3)), ("1 Termin ausgeblendet".to_string(), "3 Termine ausgeblendet".to_string()));
        assert_eq!(((EN.dates_hidden)(1), (EN.dates_hidden)(3)), ("1 date hidden".to_string(), "3 dates hidden".to_string()));
    }
}
