//! Texts of the Stundenplan's page (`pages/studyplan/mod.rs`).

pub struct Texts {
    /// The page's name: its title, and the start of its heading („Stundenplan WiSe 2026/27").
    pub title: &'static str,
    /// „Anpassen": the sidebar's heading, and the button that opens it as a sheet on a phone.
    pub customise: &'static str,
    /// The last line of the sidebar, on the server as in the app: where the plan lives.
    pub storage_hint: &'static str,
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
    customise: "Anpassen",
    storage_hint: "Dein Stundenplan liegt nur in diesem Browser.",
    seo_description: "Dein Stundenplan: Termine, Prüfungen und Kalender-Abo der geplanten Module.",
    shared_title: |semester| format!("Stundenplan · {semester}"),
    shared_description: |count, names| format!("{count}: {names}. In Betula öffnen und in den eigenen Stundenplan übernehmen."),
    server_title: "Dein Stundenplan erscheint, sobald die App geladen ist.",
    server_hint: "Dafür braucht es JavaScript.",
};

pub const EN: Texts = Texts {
    title: "Timetable",
    customise: "Customise",
    storage_hint: "Your timetable is stored in this browser only.",
    seo_description: "Your timetable: dates, exams and a calendar subscription of the planned modules.",
    shared_title: |semester| format!("Timetable · {semester}"),
    shared_description: |count, names| format!("{count}: {names}. Open in Betula and add them to your own timetable."),
    server_title: "Your timetable appears as soon as the app has loaded.",
    server_hint: "This needs JavaScript.",
};
