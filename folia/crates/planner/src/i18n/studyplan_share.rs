//! Texts of a Stundenplan handed on by a link (`pages/studyplan/share.rs`): „Link zum Teilen
//! kopieren" in the sidebar, and the offer on the page the link opens; the offer of a calendar
//! shared to stay current or by a one-time link (`LiveOffer`), with what a link that opens nothing
//! says, and the offer of Betula Sync beside a calendar followed from a private profile.

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
    /// from the other notes' beginnings (`studyplan_side::cleared`, `head::FROM_MARKED`) …
    pub taken: &'static str,
    /// … and „nichts" where nothing was taken over.
    pub nothing: &'static str,

    // ---- a calendar that stays current, and a one-time link ----
    /// „Geteilter Stundenplan, bleibt aktuell · WiSe 2026/27. ": what the offer of a calendar
    /// that stays current begins with …
    pub live_shared: fn(&str) -> String,
    /// … and, while the catalog answers, it and the offer of a copy without the semester.
    pub live_opened: &'static str,
    pub copy_opened: &'static str,
    /// After the first words of a part that holds no module (yet).
    pub live_empty: &'static str,
    /// „Als Kalender folgen" and its tooltip.
    pub follow: &'static str,
    pub follow_title: &'static str,
    /// How the note after its „Übernehmen" begins: „Aus dem geteilten Stundenplan übernommen: 6
    /// Module". Apart from `taken`, so that the offer of a fixed plan does not show it too.
    pub live_taken: &'static str,
    /// While the part or the parcel is on its way.
    pub link_opening: &'static str,
    /// What a link that opens nothing says (owner, 2026-11-08: „es sollte schon eine
    /// fehlermeldung geben mit link abgelaufen oder so"): a one-time link opened already, one
    /// older than 7 days, one taken back, a calendar not shared any more, a link that names
    /// nothing, the service too busy or away, a browser without Web Crypto, a site without Betula
    /// Sync.
    pub link_taken: &'static str,
    pub link_expired: &'static str,
    pub link_withdrawn: &'static str,
    pub link_ended: &'static str,
    pub link_unknown: &'static str,
    pub link_busy: &'static str,
    pub link_unreachable: &'static str,
    pub link_unsupported: &'static str,
    pub link_not_offered: &'static str,
    /// Beside a calendar followed from a private profile: what Betula Sync makes better, the line
    /// under it, „Profil speichern" and while it saves, and what it says once saved or failed.
    pub upgrade: &'static str,
    pub upgrade_small: &'static str,
    pub upgrade_save: &'static str,
    pub upgrade_saving: &'static str,
    pub upgrade_saved: &'static str,
    pub upgrade_failed: &'static str,
}

pub const DE: Texts = Texts {
    copy_link: "Link zum Teilen kopieren",
    with_modules: "mit den Modulen und Gruppen dieses Semesters",
    link_title: "Der Link trägt das Semester, die geplanten Module und die angehakten Gruppen: Wer ihn öffnet, kann die Module in den eigenen Stundenplan übernehmen oder deinen Plan als eigenen Kalender daneben legen, und die Vorschau in Messengern zeigt sie.",
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
    nothing: "nichts",

    live_shared: |semester| format!("Geteilter Stundenplan, bleibt aktuell · {semester}. "),
    live_opened: "Geteilter Stundenplan, bleibt aktuell.",
    copy_opened: "Geteilter Stundenplan.",
    live_empty: "Noch keine Module: Sobald welche geplant sind, stehen sie in deinem Kalender.",
    follow: "Als Kalender folgen",
    follow_title: "Den Plan als eigenen Kalender neben deinen legen: Jede Änderung kommt an, bis er nicht mehr geteilt wird. Er zählt nicht für dein Studium.",
    live_taken: "Aus dem geteilten Stundenplan übernommen: ",
    link_opening: "Der geteilte Stundenplan wird geöffnet …",
    link_taken: "Dieser Link wurde schon geöffnet. Ein Einmal-Link gilt für eine Person: Jemand hat ihn vor dir geöffnet. Wenn du das nicht warst, bitte um einen neuen Link.",
    link_expired: "Dieser Link ist abgelaufen. Einmal-Links gelten höchstens 7 Tage. Bitte um einen neuen.",
    link_withdrawn: "Dieser Link wurde zurückgezogen, bevor ihn jemand geöffnet hat. Bitte um einen neuen.",
    link_ended: "Dieser Kalender wird nicht mehr geteilt.",
    link_unknown: "Dieser Link öffnet nichts: Er ist vertippt oder abgelaufen.",
    link_busy: "Betula Sync nimmt gerade nichts an. Versuch es in ein paar Minuten noch einmal.",
    link_unreachable: "Der Dienst von Betula Sync ist gerade nicht erreichbar. Prüf deine Verbindung und lade die Seite gleich noch einmal.",
    link_unsupported: "Dieser Browser kann den geteilten Stundenplan nicht öffnen: Ihm fehlt Web Crypto. In einem aktuellen Browser geht es.",
    link_not_offered: "Diese Seite bietet Betula Sync nicht an, mit dem der Stundenplan geteilt wurde. Öffne den Link auf betula.app.",
    upgrade: "Mit Betula Sync kommt jede Änderung sofort an, und der Kalender auf jedes deiner Geräte.",
    upgrade_small: "Verschlüsselt, ohne Konto.",
    upgrade_save: "Profil speichern",
    upgrade_saving: "Wird gespeichert …",
    upgrade_saved: "Profil gespeichert: Jede Änderung kommt jetzt sofort an, auf jedem deiner Geräte.",
    upgrade_failed: "Das hat nicht geklappt. Versuch es noch einmal.",
};

pub const EN: Texts = Texts {
    copy_link: "Copy link to share",
    with_modules: "with this semester's modules and groups",
    link_title: "The link carries the semester, the planned modules and the groups ticked: whoever opens it can add the modules to their own timetable or put your plan beside theirs as a calendar of its own, and previews in messengers show them.",
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
    nothing: "nothing",

    live_shared: |semester| format!("Shared timetable, kept up to date · {semester}. "),
    live_opened: "Shared timetable, kept up to date.",
    copy_opened: "Shared timetable.",
    live_empty: "No modules yet: once some are planned, they stand in your calendar.",
    follow: "Follow as a calendar",
    follow_title: "Put the plan beside yours as a calendar of its own: every change arrives until it is no longer shared. It does not count for your study.",
    live_taken: "Added from the shared timetable: ",
    link_opening: "Opening the shared timetable …",
    link_taken: "This link was opened already. A one-time link is for one person: someone opened it before you. If that was not you, ask for a new link.",
    link_expired: "This link has expired. One-time links last 7 days at most. Ask for a new one.",
    link_withdrawn: "This link was taken back before anyone opened it. Ask for a new one.",
    link_ended: "This calendar is no longer shared.",
    link_unknown: "This link opens nothing: it is mistyped or has expired.",
    link_busy: "Betula Sync takes nothing new right now. Try again in a few minutes.",
    link_unreachable: "The Betula Sync service cannot be reached right now. Check your connection and reload the page in a moment.",
    link_unsupported: "This browser cannot open the shared timetable: it lacks Web Crypto. A current browser can.",
    link_not_offered: "This site does not offer Betula Sync, which the timetable was shared with. Open the link on betula.app.",
    upgrade: "With Betula Sync every change arrives at once, and the calendar on every device of yours.",
    upgrade_small: "Encrypted, no account.",
    upgrade_save: "Save profile",
    upgrade_saving: "Saving …",
    upgrade_saved: "Profile saved: every change now arrives at once, on every device of yours.",
    upgrade_failed: "That did not work. Try again.",
};
