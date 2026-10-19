//! Texts of a module's page and its preview (`pages/module.rs`), and of a module shown in place
//! of a page (`local.rs`).

use folia_calendar::kind::EventKind;

/// What the options of a choice are called (`crate::options`): what their events are where they
/// are all one kind, groups a title names, the offers of a topic, or events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NounOf {
    Kind(EventKind),
    Group,
    Offer,
    Event,
}

/// A noun, one and many, and in German its gender, which the words around it follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Noun {
    pub one: &'static str,
    pub many: &'static str,
    pub gender: Gender,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gender {
    Feminine,
    Neuter,
    Masculine,
}

pub struct Texts {
    // ---- what search engines and link previews read ----
    /// The page's title, the name first: „Analysis I (11101) · Modul der BTU Cottbus-Senftenberg";
    /// the values are the module's title and number.
    pub title: fn(&str, &str) -> String,
    /// The page's description, from the module's contents or learning outcomes: „Analysis I
    /// (11101, 8 LP) an der BTU Cottbus-Senftenberg: <text>"; the values are the title, the
    /// number, the credits (written already) and the text.
    pub description: fn(&str, &str, &str, &str) -> String,
    /// The same for a module without either text, saying what the page tells instead.
    pub description_bare: fn(&str, &str, &str) -> String,
    /// The catalog as the breadcrumbs of the structured data name it.
    pub breadcrumb_catalog: &'static str,
    /// „Regelstudienplan Informatik (B.Sc.), PO 2008": the study plan that places the module in a
    /// semester, as structured data names it; the values are the program, its degree and its PO.
    pub plan_framework: fn(&str, &str, &str) -> String,
    /// „Prüfung Analysis I": an exam date as an event of the structured data; the value is the
    /// title of the event.
    pub exam_event: fn(&str) -> String,

    // ---- the page and the preview ----
    /// The heading of the page's sidebar.
    pub sidebar_title: &'static str,
    /// A module the snapshot does not know, at its own address and in place of a page.
    pub not_found: &'static str,
    pub not_found_hint: &'static str,
    /// The preview's name, for screen readers.
    pub preview: &'static str,
    /// The arrow back in the preview's head, for screen readers, and the tooltips of „Vollbild"
    /// and „Schließen" with their shortcuts.
    pub close_preview: &'static str,
    pub full_view_title: &'static str,
    pub close_preview_title: &'static str,
    /// What the chip beside the module's number is („EvS"): its tooltip, and its name for screen
    /// readers.
    pub abbrev: &'static str,

    // ---- the sidebar of the page ----
    /// „Auf dieser Seite": the jumps to the sections, their heading and name.
    pub on_this_page: &'static str,
    pub actions: &'static str,
    pub copy_link: &'static str,
    /// The link to the module's page at the BTU.
    pub original_at_btu: &'static str,

    // ---- the sections, as their headings and the sidebar's jumps name them ----
    /// „Termine": the week and the list of the module's teaching dates.
    pub dates: &'static str,
    pub exam_dates: &'static str,
    pub at_a_glance: &'static str,
    pub programs: &'static str,
    pub prerequisites: &'static str,
    pub contents: &'static str,
    pub learning_outcomes: &'static str,
    /// „Prüfungsleistung": what the exam asks for, in the words of the module description.
    pub assessment: &'static str,
    pub literature: &'static str,
    pub remarks: &'static str,
    /// „Literatur (12)": the folded list of titles, with how many there are.
    pub literature_count: fn(usize) -> String,

    // ---- the heading ----
    /// Before the modules that follow this one: while it is offered, and once it is replaced.
    pub successor: &'static str,
    pub replaced_by: &'static str,

    // ---- „Auf einen Blick" ----
    /// The labels of the key facts.
    pub exam: &'static str,
    pub grading: &'static str,
    pub duration: &'static str,
    pub places: &'static str,
    pub language: &'static str,
    pub location: &'static str,
    pub responsible: &'static str,
    pub teaching_forms: &'static str,
    /// The values of the facts the app words itself: graded or not, and the places …
    pub graded: &'static str,
    pub ungraded: &'static str,
    /// … „max. 30", limited without a number, not limited.
    pub places_max: fn(i64) -> String,
    pub limited: &'static str,
    pub unlimited: &'static str,

    // ---- „Voraussetzungen" ----
    /// What a linked module is to this one.
    pub mandatory: &'static str,
    pub recommended: &'static str,
    /// The folded texts of the module description.
    pub mandatory_verbatim: &'static str,
    pub recommended_verbatim: &'static str,

    // ---- the source, under the module ----
    pub source: &'static str,
    /// „ · abgerufen 19.09.2026"; the value is the date, written already.
    pub fetched: fn(&str) -> String,
    pub original: &'static str,

    // ---- „Termine" and „Prüfungstermine" ----
    /// „Termine aus dem WiSe 2026/27. Für das SoSe 2027 hat die BTU noch keine Termine zu diesem
    /// Modul veröffentlicht.": the Termine shown are of a semester before the next one; the values
    /// are the two semesters.
    pub dates_from: fn(&str, &str) -> String,
    /// No Termine at all: what the turnus says instead (winter, summer, every semester), and
    /// where it says nothing.
    pub no_dates_winter: &'static str,
    pub no_dates_summer: &'static str,
    pub no_dates_every: &'static str,
    pub no_dates: &'static str,
    /// A row without a time of the week: „Termin offen" for an exam whose date is QIS's
    /// placeholder, else „Zeit offen".
    pub date_open: &'static str,
    pub time_open: &'static str,
    /// „Mo bis 24:00", „bis 24:00": a deadline, an end without a start; the values are the
    /// weekday and the end.
    pub day_until: fn(&str, &str) -> String,
    pub until: fn(&str) -> String,
    /// „In QIS: So 01:00–02:30 · 27.12.2015": what the BTU wrote where the row shows something
    /// else; the value is what it wrote.
    pub in_qis: fn(&str) -> String,
    /// The same, with what is odd about it („In QIS: … · Uhrzeit ungewöhnlich").
    pub in_qis_odd: fn(&str, &str) -> String,
    /// „Uhrzeit ungewöhnlich, so steht es in QIS": what the row shows as stated is odd.
    pub as_in_qis: fn(&str) -> String,
    /// What is odd about an exam date, in the line under it …
    pub unusual_time: &'static str,
    pub ends_before_start: &'static str,
    pub date_outside_semester: &'static str,
    /// … and in the note under the exam dates, which explains QIS's placeholder at 01:00–02:30:
    /// with a date outside the semester (the value), and without.
    pub placeholder_date_note: fn(&str) -> String,
    pub placeholder_time_note: &'static str,
    /// The odd things in a sentence: „eine Uhrzeit außerhalb von 06:00–22:00" (the value is the
    /// hours), „ein Ende vor dem Beginn", „ein Datum weit außerhalb des WiSe 2026/27" (the
    /// semester) …
    pub odd_time: fn(&str) -> String,
    pub odd_end: &'static str,
    pub odd_date: fn(&str) -> String,
    /// … joined by „, " and, before the last, this …
    pub odd_or: &'static str,
    /// … and the sentence they make: „<Eine Uhrzeit …> steht so in QIS, ist für eine Prüfung aber
    /// ungewöhnlich, vermutlich ein Eingabefehler." The value's first letter is made a capital.
    pub odd_note: fn(&str) -> String,
    /// The label of a slot of the week whose row names no type.
    pub slot_date: &'static str,
    /// The small line of a slot of single dates: „1 Termin · 23.02.", „1 Termin", „3 Termine".
    pub one_date_on: fn(&str) -> String,
    pub dates_count: fn(usize) -> String,
    /// A line of the list that gathers the single dates of one slot, before their days: „6
    /// Einzeltermine".
    pub single_dates: fn(usize) -> String,
    /// What a value of most dates of a line leaves out, in parentheses after it: „außer 26.01.".
    pub except_on: fn(&str) -> String,
    /// „Termine als Liste": the line that unfolds the list under the week (its count follows).
    pub dates_list: &'static str,

    // ---- the courses of a module held at two campuses (`crate::courses`) ----
    /// What joins the last two of a list: „ und ".
    pub courses_and: &'static str,
    /// „An zwei Standorten angeboten:" — the campuses follow, each with the dot of its colour, as
    /// `courses_and` joins them, and a full stop —
    pub courses_all: fn(usize) -> String,
    /// … „Du besuchst eines der Angebote."
    pub courses_one_of: &'static str,
    /// „In „Mein Studium“ mit deinem Studiengang zeigt Betula nur sein Angebot.": after
    /// `courses_all` where the page stands for no program.
    pub courses_setup: &'static str,
    /// „Für Soziale Arbeit · Bachelor nennt QIS keines der Angebote.": after `courses_all` where it
    /// stands for a program that takes no course …
    pub courses_undecided: fn(&str) -> String,
    /// … „QIS nennt Elektrotechnik · B.Sc. bei jedem Angebot.": one that takes every course.
    pub courses_every: fn(&str) -> String,
    /// „Nur die Termine für Medizininformatik · B.Sc. (Cottbus-Sachsendorf)." …
    pub courses_mine: fn(&str, &str) -> String,
    /// … „Auch angeboten: Zentralcampus Cottbus." …
    pub courses_also: fn(&str) -> String,
    /// … and the button that shows them, faint, and hides them again.
    pub courses_show: &'static str,
    pub courses_hide: &'static str,
    /// „Hervorgehoben: die Termine für Informatik · B.Sc. (Zentralcampus Cottbus)." …
    pub courses_highlight: fn(&str, &str) -> String,
    /// … „Blass: Cottbus-Sachsendorf."
    pub courses_faint: fn(&str) -> String,
    /// The tooltip of the line: QIS names the program at these Termine …
    pub courses_by_qis: fn(&str) -> String,
    /// … or at none of them, and the campus the program is taught at decides (Radix's
    /// `v_program_campus`: where QIS lists its other events, else its compulsory modules).
    pub courses_by_campus: fn(&str) -> String,
    /// The heading of a course in the list: „Cottbus-Sachsendorf · für Medizininformatik · B.Sc." …
    pub course_for: fn(&str, &str) -> String,
    /// … „Informatik · B.Sc., Mathematik · B.Sc., Physik · B.Sc. und 4 weitere" …
    pub courses_more: fn(&str, usize) -> String,
    /// … and of the Termine in no course: „Für alle Angebote".
    pub courses_shared: &'static str,

    // ---- the choices among a module's events, and its extras (`crate::options`) ----
    /// What the options are called.
    pub noun: fn(NounOf) -> Noun,
    /// The headings of the list: what everyone attends …
    pub choice_must: &'static str,
    /// … a choice: „Eine der 3 Übungen", „Zwei der 4 Übungen" (how many one takes, of how many) …
    pub choice_of: fn(u32, usize, Noun) -> String,
    /// … of options of differing sizes: „Übungen für 6 SWS" (the SWS written already) …
    pub choice_for_sws: fn(&str, Noun) -> String,
    /// … the options QIS lists other programs at, „Für andere Studiengänge" …
    pub choice_others: &'static str,
    /// … and the extras: „Zusätzlich, freiwillig".
    pub choice_extras: &'static str,
    /// „9 weitere anzeigen": the options of a long choice after its first.
    pub choice_more: fn(usize) -> String,
    /// „für Bauingenieurwesen, Maschinenbau": whom QIS lists an option for, in its line and slot.
    pub choice_for: fn(&str) -> String,
    /// An option without a time of the week, where the line over the week names its times.
    pub choice_no_time: &'static str,
    /// The line over the week. One event more than once, each for other programs: „Die Übung gibt
    /// es dreimal, je für andere Studiengänge. Du besuchst eine." (how many, the noun) …
    pub choice_program: fn(usize, Noun) -> String,
    /// … where the page stands for no program …
    pub choice_setup: &'static str,
    /// … for a program QIS lists at none of the options, and at each …
    pub choice_none: fn(&str) -> String,
    pub choice_every: fn(&str) -> String,
    /// … for Mein Studiengang: „Die Übung für Bauingenieurwesen · B.Sc.: Di 07:30." (the noun, the
    /// program, the option's times) …
    pub choice_mine: fn(Noun, &str, &str) -> String,
    /// … where QIS lists it at several: „Für Umweltingenieurwesen · Bachelor nennt QIS zwei der 3
    /// Übungen (Mi 13:45 und Do 11:30); du besuchst eine." (the program, how many it is listed at,
    /// of how many, how many one takes, the noun, their times) …
    pub choice_mine_some: fn(&str, usize, usize, u32, Noun, &str) -> String,
    /// … „Auch angeboten, für andere Studiengänge: Mi 13:45 und Do 11:30." …
    pub choice_also: fn(&str) -> String,
    /// … for another program: „Hervorgehoben: die Übung für Bauingenieurwesen · B.Sc. (Di 07:30)."
    /// and „Hervorgehoben: zwei der 3 Übungen, die QIS für … nennt (…); du besuchst eine." …
    pub choice_highlight: fn(Noun, &str, &str) -> String,
    pub choice_highlight_some: fn(&str, usize, usize, u32, Noun, &str) -> String,
    /// … „Blass: die für andere Studiengänge."
    pub choice_faint: &'static str,
    /// Groups a title names: „Kontrapunkt/Tonsatz I und Gehörbildung I gibt es in 3 Gruppen. Du
    /// besuchst eine, beide in derselben Gruppe." (what, how many groups, how many events each) …
    pub choice_groups: fn(&str, usize, usize) -> String,
    /// … one event more than once: „Das Seminar gibt es zweimal. Du besuchst eines." …
    pub choice_twice: fn(usize, Noun) -> String,
    /// A topic to choose: „Wahlangebot: Du belegst eines der 14 Projekte (je 4 SWS)." (how many one
    /// takes, of how many, the noun, the SWS of each where they are the same, written already) …
    pub choice_topic: fn(u32, usize, Noun, Option<&str>) -> String,
    /// … of options of differing sizes: „Wahlangebot: Du belegst Übungen für 6 SWS." …
    pub choice_topic_sws: fn(Noun, &str) -> String,
    /// … and where QIS lists the program at some: „QIS nennt Soziale Arbeit · Bachelor bei 7
    /// davon, sie stehen zuerst."
    pub choice_named: fn(&str, usize) -> String,
    /// „Zusätzlich, freiwillig: Aufbaukurs Höhere Mathematik - T3." (the extras, joined already)
    pub choice_extra_note: fn(&str) -> String,

    // ---- „Studiengänge" ----
    /// „12 Curricula", beside the heading.
    pub curricula: fn(usize) -> String,
    pub no_curriculum: &'static str,
    /// „Außerdem als fachübergreifendes Studium (FÜS) anrechenbar in 40 Studiengängen."
    pub fues: fn(usize) -> String,
    /// „3 weitere Nennungen gehören zu Studiengängen, die nicht im Katalog stehen."
    pub unresolved: fn(usize) -> String,
    /// „Alle 12 anzeigen": the button under the first curricula that shows the rest; the value is
    /// how many there are.
    pub programs_all: fn(usize) -> String,
}

pub const DE: Texts = Texts {
    title: |title, id| format!("{title} ({id}) · Modul der BTU Cottbus-Senftenberg"),
    description: |title, id, credits, text| format!("{title} ({id}, {credits}) an der BTU Cottbus-Senftenberg: {text}"),
    description_bare: |title, id, credits| format!("{title} (Modul {id}, {credits}) an der BTU Cottbus-Senftenberg: Turnus, Prüfung, Voraussetzungen und Studiengänge."),
    breadcrumb_catalog: "Modulkatalog",
    plan_framework: |program, degree, po| format!("Regelstudienplan {program} ({degree}), PO {po}"),
    exam_event: |title| format!("Prüfung {title}"),

    sidebar_title: "Modul",
    not_found: "Modul nicht gefunden",
    not_found_hint: "Dieses Modul steht nicht (mehr) im Modulkatalog der BTU.",
    preview: "Modulvorschau",
    close_preview: "Vorschau schließen",
    full_view_title: "Als ganze Seite öffnen (F)",
    close_preview_title: "Vorschau schließen (Esc)",
    abbrev: "Kürzel",

    on_this_page: "Auf dieser Seite",
    actions: "Aktionen",
    copy_link: "Link kopieren",
    original_at_btu: "Original bei der BTU",

    dates: "Termine",
    exam_dates: "Prüfungstermine",
    at_a_glance: "Auf einen Blick",
    programs: "Studiengänge",
    prerequisites: "Voraussetzungen",
    contents: "Inhalte",
    learning_outcomes: "Lernziele",
    assessment: "Prüfungsleistung",
    literature: "Literatur",
    remarks: "Bemerkungen",
    literature_count: |n| format!("Literatur ({n})"),

    successor: "Nachfolgemodul: ",
    replaced_by: "Wird abgelöst durch: ",

    exam: "Prüfung",
    grading: "Benotung",
    duration: "Dauer",
    places: "Plätze",
    language: "Sprache",
    location: "Standort",
    responsible: "Verantwortlich",
    teaching_forms: "Lehrformen",
    graded: "benotet",
    ungraded: "unbenotet",
    places_max: |n| format!("max. {n}"),
    limited: "begrenzt",
    unlimited: "unbegrenzt",

    mandatory: "zwingend",
    recommended: "empfohlen",
    mandatory_verbatim: "Zwingend, im Wortlaut",
    recommended_verbatim: "Empfohlen, im Wortlaut",

    source: "Quelle: Modulbeschreibung der BTU",
    fetched: |date| format!(" · abgerufen {date}"),
    original: "Original",

    dates_from: |semester, next| format!("Termine aus dem {semester}. Für das {next} hat die BTU noch keine Termine zu diesem Modul veröffentlicht."),
    no_dates_winter: "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul im Wintersemester angeboten.",
    no_dates_summer: "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul im Sommersemester angeboten.",
    no_dates_every: "Noch keine Termine veröffentlicht. Laut Modulbeschreibung wird das Modul jedes Semester angeboten.",
    no_dates: "Zu diesem Modul sind keine Termine veröffentlicht.",
    date_open: "Termin offen",
    time_open: "Zeit offen",
    day_until: |day, end| format!("{day} bis {end}"),
    until: |end| format!("bis {end}"),
    in_qis: |stated| format!("In QIS: {stated}"),
    in_qis_odd: |stated, odd| format!("In QIS: {stated} · {odd}"),
    as_in_qis: |odd| format!("{odd}, so steht es in QIS"),
    unusual_time: "Uhrzeit ungewöhnlich",
    ends_before_start: "Ende vor Beginn",
    date_outside_semester: "Datum außerhalb des Semesters",
    placeholder_date_note: |semester| {
        format!("01:00–02:30 ist in QIS ein Platzhalter für eine Prüfung ohne festen Termin (oft „nach Vereinbarung“), und das Datum dazu passt nicht ins {semester}. Betula zeigt beides nicht; was die BTU angibt, steht in der Zeile.")
    },
    placeholder_time_note: "01:00–02:30 ist in QIS ein Platzhalter für eine Prüfung ohne feste Uhrzeit (oft „nach Vereinbarung“). Betula zeigt ihn nicht als Uhrzeit; was die BTU angibt, steht in der Zeile.",
    odd_time: |hours| format!("eine Uhrzeit außerhalb von {hours}"),
    odd_end: "ein Ende vor dem Beginn",
    odd_date: |semester| format!("ein Datum weit außerhalb des {semester}"),
    odd_or: " oder ",
    odd_note: |list| format!("{list} steht so in QIS, ist für eine Prüfung aber ungewöhnlich, vermutlich ein Eingabefehler."),
    slot_date: "Termin",
    one_date_on: |day| format!("1 Termin · {day}"),
    dates_count: |n| if n == 1 { "1 Termin".to_string() } else { format!("{n} Termine") },
    single_dates: |n| if n == 1 { "1 Einzeltermin".to_string() } else { format!("{n} Einzeltermine") },
    except_on: |days| format!("außer {days}"),
    dates_list: "Termine als Liste",

    courses_and: " und ",
    courses_all: |n| {
        let n = match n {
            2 => "zwei".to_string(),
            3 => "drei".to_string(),
            4 => "vier".to_string(),
            n => n.to_string(),
        };
        format!("An {n} Standorten angeboten:")
    },
    courses_one_of: "Du besuchst eines der Angebote.",
    courses_setup: "Mit deinem Studiengang in „Mein Studium“ zeigt Betula nur sein Angebot.",
    courses_undecided: |program| format!("Für {program} nennt QIS keines der Angebote."),
    courses_every: |program| format!("QIS nennt {program} bei jedem Angebot."),
    courses_mine: |program, places| format!("Nur die Termine für {program} ({places})."),
    courses_also: |places| format!("Auch angeboten: {places}."),
    courses_show: "Auch zeigen",
    courses_hide: "Wieder ausblenden",
    courses_highlight: |program, places| format!("Hervorgehoben: die Termine für {program} ({places})."),
    courses_faint: |places| format!("Blass: {places}."),
    courses_by_qis: |program| format!("QIS nennt {program} bei diesen Terminen."),
    courses_by_campus: |program| format!("QIS nennt {program} bei keinem der Angebote. Betula nimmt das Angebot an dem Standort, an dem der Studiengang studiert wird: wo QIS die meisten seiner Veranstaltungen nennt, sonst wo die meisten seiner Pflichtmodule stattfinden."),
    course_for: |place, programs| format!("{place} · für {programs}"),
    courses_more: |programs, n| if n == 1 { format!("{programs} und einen weiteren") } else { format!("{programs} und {n} weitere") },
    courses_shared: "Für alle Angebote",

    noun: |of| {
        let (one, many, gender) = match of {
            NounOf::Kind(EventKind::Lecture) => ("Vorlesung", "Vorlesungen", Gender::Feminine),
            NounOf::Kind(EventKind::Exercise) => ("Übung", "Übungen", Gender::Feminine),
            NounOf::Kind(EventKind::Seminar) => ("Seminar", "Seminare", Gender::Neuter),
            NounOf::Kind(EventKind::Practical) => ("Praktikum", "Praktika", Gender::Neuter),
            NounOf::Kind(EventKind::Project) => ("Projekt", "Projekte", Gender::Neuter),
            NounOf::Kind(EventKind::Tutorial) => ("Tutorium", "Tutorien", Gender::Neuter),
            NounOf::Kind(EventKind::Consultation) => ("Konsultation", "Konsultationen", Gender::Feminine),
            NounOf::Kind(EventKind::Excursion) => ("Exkursion", "Exkursionen", Gender::Feminine),
            NounOf::Group => ("Gruppe", "Gruppen", Gender::Feminine),
            NounOf::Offer => ("Angebot", "Angebote", Gender::Neuter),
            NounOf::Kind(_) | NounOf::Event => ("Veranstaltung", "Veranstaltungen", Gender::Feminine),
        };
        Noun { one, many, gender }
    },
    choice_must: "Pflicht",
    choice_of: |pick, n, noun| format!("{} der {n} {}", capital(&de_pick(pick, noun.gender)), noun.many),
    choice_for_sws: |sws, noun| format!("{} für {sws} SWS", noun.many),
    choice_others: "Für andere Studiengänge",
    choice_extras: "Zusätzlich, freiwillig",
    choice_more: |n| format!("{n} weitere anzeigen"),
    choice_for: |programs| format!("für {programs}"),
    choice_no_time: "ohne Termin",
    choice_program: |n, noun| format!("{} {} gibt es {}, je für andere Studiengänge. Du besuchst {}.", capital(de_the(noun.gender)), noun.one, de_times(n), de_pick(1, noun.gender)),
    choice_setup: "Mit deinem Studiengang in „Mein Studium“ zeigt Betula nur, was QIS für ihn nennt.",
    choice_none: |program| format!("Für {program} nennt QIS keinen dieser Termine."),
    choice_every: |program| format!("QIS nennt {program} bei jedem dieser Termine."),
    choice_mine: |noun, program, times| format!("{} {} für {program}: {times}.", capital(de_the(noun.gender)), noun.one),
    choice_mine_some: |program, own, n, pick, noun, times| {
        format!("Für {program} nennt QIS {} der {n} {} ({times}); du besuchst {}.", de_number(own), noun.many, de_pick(pick, noun.gender))
    },
    choice_also: |times| format!("Auch angeboten, für andere Studiengänge: {times}."),
    choice_highlight: |noun, program, times| format!("Hervorgehoben: {} {} für {program} ({times}).", de_the(noun.gender), noun.one),
    choice_highlight_some: |program, own, n, pick, noun, times| {
        format!("Hervorgehoben: {} der {n} {}, die QIS für {program} nennt ({times}); du besuchst {}.", de_number(own), noun.many, de_pick(pick, noun.gender))
    },
    choice_faint: "Blass: die für andere Studiengänge.",
    choice_groups: |what, n, each| match each {
        0 | 1 => format!("{what} gibt es in {n} Gruppen. Du besuchst eine."),
        2 => format!("{what} gibt es in {n} Gruppen. Du besuchst eine, beide in derselben Gruppe."),
        _ => format!("{what} gibt es in {n} Gruppen. Du besuchst eine, alle in derselben Gruppe."),
    },
    choice_twice: |n, noun| format!("{} {} gibt es {}. Du besuchst {}.", capital(de_the(noun.gender)), noun.one, de_times(n), de_pick(1, noun.gender)),
    choice_topic: |pick, n, noun, each| {
        let each = each.map(|sws| format!(" (je {sws} SWS)")).unwrap_or_default();
        format!("Wahlangebot: Du belegst {} der {n} {}{each}.", de_pick(pick, noun.gender), noun.many)
    },
    choice_topic_sws: |noun, sws| format!("Wahlangebot: Du belegst {} für {sws} SWS.", noun.many),
    choice_named: |program, n| format!("QIS nennt {program} bei {n} davon, sie stehen zuerst."),
    choice_extra_note: |extras| format!("Zusätzlich, freiwillig: {extras}."),

    curricula: |n| format!("{n} Curricula"),
    no_curriculum: "Das Modul gehört zu keinem Curriculum eines Studiengangs im Katalog.",
    fues: |n| format!("Außerdem als fachübergreifendes Studium (FÜS) anrechenbar in {n} Studiengängen."),
    unresolved: |n| format!("{n} weitere Nennungen gehören zu Studiengängen, die nicht im Katalog stehen."),
    programs_all: |n| format!("Alle {n} anzeigen"),
};

pub const EN: Texts = Texts {
    title: |title, id| format!("{title} ({id}) · Module of BTU Cottbus-Senftenberg"),
    description: |title, id, credits, text| format!("{title} ({id}, {credits}) at BTU Cottbus-Senftenberg: {text}"),
    description_bare: |title, id, credits| format!("{title} (module {id}, {credits}) at BTU Cottbus-Senftenberg: when it is offered, exam, prerequisites and degree programmes."),
    breadcrumb_catalog: "Module catalogue",
    plan_framework: |program, degree, po| format!("Standard study plan {program} ({degree}), PO {po}"),
    exam_event: |title| format!("Exam {title}"),

    sidebar_title: "Module",
    not_found: "Module not found",
    not_found_hint: "This module is not (or no longer) in BTU's module catalogue.",
    preview: "Module preview",
    close_preview: "Close preview",
    full_view_title: "Open as a full page (F)",
    close_preview_title: "Close preview (Esc)",
    abbrev: "Abbreviation",

    on_this_page: "On this page",
    actions: "Actions",
    copy_link: "Copy link",
    original_at_btu: "Original at BTU",

    dates: "Dates",
    exam_dates: "Exam dates",
    at_a_glance: "At a glance",
    programs: "Degree programmes",
    prerequisites: "Prerequisites",
    contents: "Contents",
    learning_outcomes: "Learning outcomes",
    assessment: "Assessment",
    literature: "Literature",
    remarks: "Remarks",
    literature_count: |n| format!("Literature ({n})"),

    successor: "Successor module: ",
    replaced_by: "Replaced by: ",

    exam: "Exam",
    grading: "Grading",
    duration: "Duration",
    places: "Places",
    language: "Language",
    location: "Location",
    responsible: "Responsible",
    teaching_forms: "Teaching forms",
    graded: "graded",
    ungraded: "ungraded",
    places_max: |n| format!("max. {n}"),
    limited: "limited",
    unlimited: "unlimited",

    mandatory: "mandatory",
    recommended: "recommended",
    mandatory_verbatim: "Mandatory, as written",
    recommended_verbatim: "Recommended, as written",

    source: "Source: BTU's module description",
    fetched: |date| format!(" · retrieved {date}"),
    original: "Original",

    dates_from: |semester, next| format!("Dates from {semester}. BTU has not published any dates for this module for {next} yet."),
    no_dates_winter: "No dates published yet. According to the module description, the module is offered in the winter semester.",
    no_dates_summer: "No dates published yet. According to the module description, the module is offered in the summer semester.",
    no_dates_every: "No dates published yet. According to the module description, the module is offered every semester.",
    no_dates: "No dates have been published for this module.",
    date_open: "Date TBA",
    time_open: "Time TBA",
    day_until: |day, end| format!("{day} until {end}"),
    until: |end| format!("until {end}"),
    in_qis: |stated| format!("In QIS: {stated}"),
    in_qis_odd: |stated, odd| format!("In QIS: {stated} · {odd}"),
    as_in_qis: |odd| format!("{odd}, as stated in QIS"),
    unusual_time: "Unusual time",
    ends_before_start: "End before start",
    date_outside_semester: "Date outside the semester",
    placeholder_date_note: |semester| {
        format!("In QIS, 01:00–02:30 is a placeholder for an exam without a fixed date (often \u{201c}by arrangement\u{201d}), and the date given with it does not fall within {semester}. Betula shows neither; what BTU states is in the row.")
    },
    placeholder_time_note: "In QIS, 01:00–02:30 is a placeholder for an exam without a fixed time (often \u{201c}by arrangement\u{201d}). Betula does not show it as a time; what BTU states is in the row.",
    odd_time: |hours| format!("a time outside {hours}"),
    odd_end: "an end before the start",
    odd_date: |semester| format!("a date far outside {semester}"),
    odd_or: " or ",
    odd_note: |list| format!("{list} is what QIS states, but it is unusual for an exam, probably an input error."),
    slot_date: "Session",
    one_date_on: |day| format!("1 date · {day}"),
    dates_count: |n| if n == 1 { "1 date".to_string() } else { format!("{n} dates") },
    single_dates: |n| if n == 1 { "1 single date".to_string() } else { format!("{n} single dates") },
    except_on: |days| format!("except {days}"),
    dates_list: "Dates as a list",

    courses_and: " and ",
    courses_all: |n| {
        let n = match n {
            2 => "two".to_string(),
            3 => "three".to_string(),
            4 => "four".to_string(),
            n => n.to_string(),
        };
        format!("Offered at {n} campuses:")
    },
    courses_one_of: "You attend one of them.",
    courses_setup: "With your degree programme set up in “My studies”, Betula shows only its dates.",
    courses_undecided: |program| format!("QIS names none of them for {program}."),
    courses_every: |program| format!("QIS names {program} at each of them."),
    courses_mine: |program, places| format!("Only the dates for {program} ({places})."),
    courses_also: |places| format!("Also offered: {places}."),
    courses_show: "Show them too",
    courses_hide: "Hide them again",
    courses_highlight: |program, places| format!("Highlighted: the dates for {program} ({places})."),
    courses_faint: |places| format!("Faint: {places}."),
    courses_by_qis: |program| format!("QIS names {program} at these dates."),
    courses_by_campus: |program| format!("QIS names {program} at none of them. Betula takes the one on the campus where the programme is taught: where QIS names most of its courses, else where most of its compulsory modules are held."),
    course_for: |place, programs| format!("{place} · for {programs}"),
    courses_more: |programs, n| format!("{programs} and {n} more"),
    courses_shared: "For every course",

    noun: |of| {
        let (one, many) = match of {
            NounOf::Kind(EventKind::Lecture) => ("lecture", "lectures"),
            NounOf::Kind(EventKind::Exercise) => ("exercise class", "exercise classes"),
            NounOf::Kind(EventKind::Seminar) => ("seminar", "seminars"),
            NounOf::Kind(EventKind::Practical) => ("practical", "practicals"),
            NounOf::Kind(EventKind::Project) => ("project", "projects"),
            NounOf::Kind(EventKind::Tutorial) => ("tutorial", "tutorials"),
            NounOf::Kind(EventKind::Consultation) => ("consultation", "consultations"),
            NounOf::Kind(EventKind::Excursion) => ("excursion", "excursions"),
            NounOf::Group => ("group", "groups"),
            NounOf::Offer => ("option", "options"),
            NounOf::Kind(_) | NounOf::Event => ("class", "classes"),
        };
        Noun { one, many, gender: Gender::Neuter }
    },
    choice_must: "Compulsory",
    choice_of: |pick, n, noun| format!("{} of the {n} {}", capital(&en_number(pick as usize)), noun.many),
    choice_for_sws: |sws, noun| format!("{} for {sws} SWS", capital(noun.many)),
    choice_others: "For other programmes",
    choice_extras: "Additional, optional",
    choice_more: |n| format!("Show {n} more"),
    choice_for: |programs| format!("for {programs}"),
    choice_no_time: "no date",
    choice_program: |n, noun| format!("The {} is held {}, each for other programmes. You attend one.", noun.one, en_times(n)),
    choice_setup: "With your degree programme set up in “My studies”, Betula shows only what QIS names for it.",
    choice_none: |program| format!("QIS names {program} at none of them."),
    choice_every: |program| format!("QIS names {program} at each of them."),
    choice_mine: |noun, program, times| format!("The {} for {program}: {times}.", noun.one),
    choice_mine_some: |program, own, n, pick, noun, times| {
        format!("For {program}, QIS names {} of the {n} {} ({times}); you attend {}.", en_number(own), noun.many, en_number(pick as usize))
    },
    choice_also: |times| format!("Also offered, for other programmes: {times}."),
    choice_highlight: |noun, program, times| format!("Highlighted: the {} for {program} ({times}).", noun.one),
    choice_highlight_some: |program, own, n, pick, noun, times| {
        format!("Highlighted: {} of the {n} {}, which QIS names for {program} ({times}); you attend {}.", en_number(own), noun.many, en_number(pick as usize))
    },
    choice_faint: "Faint: those for other programmes.",
    choice_groups: |what, n, each| match each {
        0 | 1 => format!("{what} is held in {n} groups. You attend one."),
        2 => format!("{what} are held in {n} groups. You attend one, both in the same group."),
        _ => format!("{what} are held in {n} groups. You attend one, all in the same group."),
    },
    choice_twice: |n, noun| format!("The {} is held {}. You attend one.", noun.one, en_times(n)),
    choice_topic: |pick, n, noun, each| {
        let each = each.map(|sws| format!(" ({sws} SWS each)")).unwrap_or_default();
        format!("Elective: you take {} of the {n} {}{each}.", en_number(pick as usize), noun.many)
    },
    choice_topic_sws: |noun, sws| format!("Elective: you take {} for {sws} SWS.", noun.many),
    choice_named: |program, n| format!("QIS names {program} at {n} of them, which come first."),
    choice_extra_note: |extras| format!("Additional, optional: {extras}."),

    curricula: |n| if n == 1 { "1 curriculum".to_string() } else { format!("{n} curricula") },
    no_curriculum: "The module is in no curriculum of a degree programme in the catalogue.",
    fues: |n| {
        if n == 1 {
            "Also creditable as interdisciplinary studies (FÜS) in 1 degree programme.".to_string()
        } else {
            format!("Also creditable as interdisciplinary studies (FÜS) in {n} degree programmes.")
        }
    },
    unresolved: |n| {
        if n == 1 {
            "1 further mention belongs to a degree programme that is not in the catalogue.".to_string()
        } else {
            format!("{n} further mentions belong to degree programmes that are not in the catalogue.")
        }
    },
    programs_all: |n| format!("Show all {n}"),
};

/// A text with its first letter a capital.
fn capital(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// „zwei" … „zwölf", else the digits.
fn de_number(n: usize) -> String {
    const WORDS: [&str; 13] = ["null", "eins", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun", "zehn", "elf", "zwölf"];
    WORDS.get(n).filter(|_| n >= 2).map(|word| word.to_string()).unwrap_or_else(|| n.to_string())
}

/// How many one takes, as the object of a sentence: „eine" (Übung), „eines" (Projekt), „einen"
/// (Kurs), „zwei".
fn de_pick(pick: u32, gender: Gender) -> String {
    match (pick, gender) {
        (1, Gender::Feminine) => "eine".to_string(),
        (1, Gender::Neuter) => "eines".to_string(),
        (1, Gender::Masculine) => "einen".to_string(),
        (n, _) => de_number(n as usize),
    }
}

/// The definite article of a noun: „die", „das", „der".
fn de_the(gender: Gender) -> &'static str {
    match gender {
        Gender::Feminine => "die",
        Gender::Neuter => "das",
        Gender::Masculine => "der",
    }
}

/// „zweimal" … „zwölfmal", else „13-mal".
fn de_times(n: usize) -> String {
    match n {
        2..=12 => format!("{}mal", de_number(n)),
        n => format!("{n}-mal"),
    }
}

/// „one" … „twelve", else the digits.
fn en_number(n: usize) -> String {
    const WORDS: [&str; 13] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];
    WORDS.get(n).map(|word| word.to_string()).unwrap_or_else(|| n.to_string())
}

/// „twice", „three times", …
fn en_times(n: usize) -> String {
    match n {
        2 => "twice".to_string(),
        n => format!("{} times", en_number(n)),
    }
}
