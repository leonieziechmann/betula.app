//! Texts of the Stundenplan's page (`pages/studyplan/mod.rs`).

pub struct Texts {
    /// The page's name: its title, and the start of its heading („Stundenplan WiSe 2026/27").
    pub title: &'static str,
    /// The last line of the sidebar, on the server as in the app: where the plan lives, in a
    /// private profile (and on the server, which cannot know) and in a synced one (`storage_hint`).
    pub storage_hint: &'static str,
    pub storage_hint_synced: &'static str,
    /// What search engines and link previews read of the page …
    pub seo_description: &'static str,
    /// … and of a plan handed on by a link: „Stundenplan · WiSe 2026/27" (the semester's name) …
    pub shared_title: fn(&str) -> String,
    /// … „3 Module: MIT-1, AuP, EEG. In Betula öffnen und in den eigenen Stundenplan übernehmen."
    /// (the number of modules written already, their short names joined with „, ").
    pub shared_description: fn(&str, &str) -> String,
    /// What the server's page says in the place of the plan, and what it takes to see it.
    pub server_title: &'static str,
    pub server_hint: &'static str,
}

pub const DE: Texts = Texts {
    title: "Stundenplan",
    storage_hint: "Dein Stundenplan liegt nur in diesem Browser.",
    storage_hint_synced: "Dein Stundenplan liegt in diesem Browser und, weil Betula Sync an ist, auf deinen anderen Geräten. Auf dem Server liegt er nur verschlüsselt.",
    seo_description: "Dein Stundenplan: Termine, Prüfungen und Kalender-Abo der geplanten Module.",
    shared_title: |semester| format!("Stundenplan · {semester}"),
    shared_description: |count, names| format!("{count}: {names}. In Betula öffnen und in den eigenen Stundenplan übernehmen."),
    server_title: "Dein Stundenplan erscheint, sobald die App geladen ist.",
    server_hint: "Dafür braucht es JavaScript.",
};

pub const EN: Texts = Texts {
    title: "Timetable",
    storage_hint: "Your timetable is stored in this browser only.",
    storage_hint_synced: "Your timetable is stored in this browser and, as Betula Sync is on, on your other devices. The server holds it encrypted only.",
    seo_description: "Your timetable: dates, exams and a calendar subscription of the planned modules.",
    shared_title: |semester| format!("Timetable · {semester}"),
    shared_description: |count, names| format!("{count}: {names}. Open in Betula and add them to your own timetable."),
    server_title: "Your timetable appears as soon as the app has loaded.",
    server_hint: "This needs JavaScript.",
};

impl Texts {
    /// Where the plan lives, by whether the active profile is synced (`folia_stores::sync::active_synced`):
    /// „nur in diesem Browser" is untrue of a synced one (owner, 2026-11-18).
    pub fn storage_hint(&self, synced: bool) -> &'static str {
        if synced { self.storage_hint_synced } else { self.storage_hint }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use folia_stores::sync::{Problem, State};

    /// Every state a profile can be in: synced (connected or not) and every other.
    fn states() -> Vec<State> {
        vec![
            State::Private,
            State::Starting,
            State::Ending,
            State::Online { devices: 1, direct: 1, relayed: 0 },
            State::Offline,
            State::Ended,
            State::Failed(Problem::Unreachable),
        ]
    }

    /// A synced profile's hint names its other devices and the server's encrypted copy; every
    /// other state keeps „nur in diesem Browser", in both languages.
    #[test]
    fn the_storage_hint_follows_the_profiles_state() {
        for state in states() {
            let synced = matches!(state, State::Online { .. } | State::Offline);
            assert_eq!(state.synced(), synced);
            assert_eq!(DE.storage_hint(state.synced()), if synced { "Dein Stundenplan liegt in diesem Browser und, weil Betula Sync an ist, auf deinen anderen Geräten. Auf dem Server liegt er nur verschlüsselt." } else { "Dein Stundenplan liegt nur in diesem Browser." }, "{state:?}");
            assert_eq!(EN.storage_hint(state.synced()), if synced { "Your timetable is stored in this browser and, as Betula Sync is on, on your other devices. The server holds it encrypted only." } else { "Your timetable is stored in this browser only." }, "{state:?}");
        }
    }
}
