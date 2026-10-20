//! Texts of the Stundenplan's store and of „Einplanen" on other pages (`studyplan.rs`): its switch
//! beside „Merken" and in the sidebar of a module's page, the menu at its arrow
//! (`folia_widgets::planmenu`), a row swiped to the right, the note of what it did
//! (`studyplan/planning.rs`) and the dialog that sets up the study on the way
//! (`folia_widgets::plansetup`).

pub struct Texts {
    /// The switch's label: „Einplanen" while the module is not planned where it aims, „Eingeplant"
    /// once it is.
    pub plan: &'static str,
    pub planned: &'static str,
    /// „für „Anwendungsfach“": the placeholder of the Stundenplan the switch plans the module
    /// for; the value is its name.
    pub for_placeholder: fn(&str) -> String,
    /// „geplant: SoSe 2027": the other semesters the module is planned in, written already.
    pub planned_in: fn(&str) -> String,
    /// What a click does, as the switch's tooltip (its shortcut follows): take the module out of
    /// the semester (the value) …
    pub unplan_hint: fn(&str) -> String,
    /// The switch as a command of „Befehle" while the module is planned: „Aus dem Plan nehmen".
    pub unplan: &'static str,
    /// … plan it there for a placeholder (semester, placeholder's name) …
    pub plan_for_hint: fn(&str, &str) -> String,
    /// … and plan it there.
    pub plan_hint: fn(&str) -> String,
    /// „3. FS": a student's semester of study, short, beside a semester.
    pub fs_short: fn(u8) -> String,
    /// The arrow beside „Einplanen", not planned (folia/design/tasten/konzept.html, „Knopf mit
    /// Pfeil"; `folia_widgets::planinto`): its name, its tooltip with its key, and the menu it opens
    /// — „Einplanen in", each semester with whether the module is offered then (`offered`,
    /// `not_offered`) and „jetzt" (`now`) as the menu of „Eingeplant" says them, a check where it is
    /// planned, the semesters behind the student folded.
    pub plan_more: &'static str,
    pub plan_more_hint: &'static str,
    pub plan_into: &'static str,
    pub planned_there: &'static str,
    pub earlier_semesters: &'static str,
    /// What a row swiped to the right uncovers on a module planned where it aims (`swipe.rs`):
    /// „Entfernen" …
    pub remove: &'static str,
    /// … with the line under it, the semester it takes the module out of (the value) …
    pub out_of: fn(&str) -> String,
    /// … and „Entfernt" once it is done (a module planned by the swipe says `planned`).
    pub removed: &'static str,
    /// „bestanden" in the switch's small line where the module is passed in the semester it is
    /// planned in, „bestanden: SoSe 25" where it is passed in another.
    pub passed: &'static str,
    pub passed_at: fn(&str) -> String,

    /// The arrow beside „Eingeplant" (`folia_widgets::planmenu`): its name for screen readers, its
    /// tooltip with its shortcut …
    pub more: &'static str,
    pub more_hint: &'static str,
    /// … the menu's entries, worded as the rows' menus of „Mein Studium" word them: passed or no
    /// longer …
    pub mark_passed: &'static str,
    pub unmark_passed: &'static str,
    /// … „Verschieben nach ›", its submenu's title with the module's name, and in it each semester
    /// with „jetzt" at the current one, whether it offers the module, and „Urlaubssemester" …
    pub move_to: &'static str,
    pub move_title: fn(&str) -> String,
    pub now: &'static str,
    pub offered: &'static str,
    pub not_offered: &'static str,
    pub leave: &'static str,
    /// … the Stundenplan of the semester.
    pub in_timetable: &'static str,

    /// The note after a module was planned: what happened, under the semester („eingeplant",
    /// „verschoben" once moved with the arrows) …
    pub note_planned: &'static str,
    pub note_moved: &'static str,
    /// … „liegt zurück" in a semester behind the student, „kein Angebot" in one its turnus leaves
    /// out …
    pub note_behind: &'static str,
    pub note_not_offered: &'static str,
    /// … the arrows (their key follows in the tooltip) …
    pub earlier: &'static str,
    pub later: &'static str,
    /// … „Weitere Module auch nach" and the semester, the switch of its second row …
    pub keep_for: &'static str,
    /// … and what it does, as its tooltip.
    pub keep_hint: &'static str,
    /// „Datenbanken entfernt": a module taken out, after its title (which gives way where the note
    /// is narrow, the word never).
    pub note_removed: &'static str,
    /// „Datenbanken als bestanden markiert", or „nicht mehr als bestanden markiert", the same way.
    pub note_passed: &'static str,
    pub note_unpassed: &'static str,

    /// The dialog for a study not set up: „Datenbanken einplanen" (the module's title) …
    pub setup_title: fn(&str) -> String,
    /// … what it is for, where nothing is known and where all is filled in …
    pub setup_lead: &'static str,
    pub setup_lead_filled: &'static str,
    /// … the sentence the rows make: the question while no program is chosen, else „Du studierst"
    /// <program> „im" <„3. Fachsemester"> „." …
    pub ask_program: &'static str,
    pub say_before: &'static str,
    pub say_middle: &'static str,
    pub say_fs: fn(u8) -> String,
    pub say_end: &'static str,
    /// … the rows: the program, the module's programs as a list (its name for screen readers, the
    /// module's title) and the search for any other …
    pub program_row: &'static str,
    pub programs_of: fn(&str) -> String,
    pub search_program: &'static str,
    pub search_placeholder: &'static str,
    pub my_program: &'static str,
    /// … in the picker of a program known, the heading over the module's programs …
    pub with_module: &'static str,
    /// … where the program comes from …
    pub from_filter: &'static str,
    pub from_timetable: &'static str,
    pub from_mine: &'static str,
    /// … where its Regelstudienplan places the module (its semester): a compulsory module, an
    /// elective one, another; or not at all …
    pub compulsory_at: fn(i64) -> String,
    pub elective_at: fn(i64) -> String,
    pub other_at: fn(i64) -> String,
    pub not_in_plan: &'static str,
    /// „Sem." after the semester the plan places the module in, in the list („Pflicht · 3. Sem.") …
    pub semester_short: &'static str,
    /// … the Fachsemester: its row, its value, − and +, since when, and where it comes from …
    pub fs_row: &'static str,
    pub fs_long: fn(u8) -> String,
    pub fs_less: &'static str,
    pub fs_more: &'static str,
    pub since: fn(&str) -> String,
    pub fs_fit: &'static str,
    pub fs_filter: fn(u8, &str) -> String,
    pub fs_assumed: &'static str,
    pub fs_follows: &'static str,
    /// … the module's semester: its row, what its line says …
    pub module_row: &'static str,
    pub this_semester: &'static str,
    /// „für weitere Module gewählt": the semester „Weitere Module auch nach …" chose for the session,
    /// and „Aufheben", which clears that choice.
    pub chosen_note: &'static str,
    pub chosen_clear: &'static str,
    pub in_the_past: &'static str,
    pub not_offered_there: &'static str,
    pub as_in_plan: fn(i64) -> String,
    pub in_plan_at: fn(i64) -> String,
    pub before_start: &'static str,
    pub semester_of: fn(&str) -> String,
    /// … the warnings: a semester behind the student (the semester) …
    pub past_head: fn(&str) -> String,
    pub past_rest: &'static str,
    /// … and a place in the plan behind the student (its Fachsemester and semester, then where the
    /// module goes instead) …
    pub behind_head: fn(u8, &str) -> String,
    pub behind_rest: fn(&str) -> String,
    /// … and the foot.
    pub without_study: &'static str,
    pub cancel: &'static str,
    pub set_up_and_plan: &'static str,
    pub choose_first: &'static str,
}

pub const DE: Texts = Texts {
    plan: "Einplanen",
    planned: "Eingeplant",
    for_placeholder: |name| format!("für „{name}“"),
    planned_in: |semesters| format!("geplant: {semesters}"),
    unplan_hint: |semester| format!("Eingeplant in {semester}. Noch einmal nimmt das Modul aus dem Plan"),
    unplan: "Aus dem Plan nehmen",
    plan_for_hint: |semester, name| format!("In {semester} einplanen, für „{name}“"),
    plan_hint: |semester| format!("In {semester} einplanen"),
    fs_short: |n| format!("{n}. FS"),
    plan_more: "In ein anderes Semester",
    plan_more_hint: "In ein anderes Semester (⇧P)",
    plan_into: "Einplanen in",
    planned_there: "eingeplant",
    earlier_semesters: "Frühere Semester",
    remove: "Entfernen",
    out_of: |semester| format!("aus {semester}"),
    removed: "Entfernt",
    passed: "bestanden",
    passed_at: |semester| format!("bestanden: {semester}"),

    more: "Semester und bestanden",
    more_hint: "Semester ändern, als bestanden markieren (⇧P)",
    mark_passed: "Als bestanden markieren",
    unmark_passed: "Bestanden zurücknehmen",
    move_to: "Verschieben nach",
    move_title: |name| format!("„{name}“ verschieben nach"),
    now: "jetzt",
    offered: "angeboten",
    not_offered: "nicht angeboten",
    leave: "Urlaubssemester",
    in_timetable: "Im Stundenplan öffnen",

    note_planned: "eingeplant",
    note_moved: "verschoben",
    note_behind: "liegt zurück",
    note_not_offered: "kein Angebot",
    earlier: "Ein Semester früher",
    later: "Ein Semester später",
    keep_for: "Weitere Module auch nach",
    keep_hint: "Für diese Sitzung: „Einplanen“ plant auch die nächsten Module in dieses Semester",
    note_removed: "entfernt",
    note_passed: "als bestanden markiert",
    note_unpassed: "nicht mehr als bestanden markiert",

    setup_title: |title| format!("{title} einplanen"),
    setup_lead: "Richte dabei dein Studium ein, dann stehen Fachsemester dabei.",
    setup_lead_filled: "Richte dabei dein Studium ein, dann stehen Fachsemester dabei. Alles ist vorausgefüllt; ändere, was nicht stimmt.",
    ask_program: "Was studierst du?",
    say_before: "Du studierst ",
    say_middle: " im ",
    say_fs: |n| format!("{n}. Fachsemester"),
    say_end: ".",
    program_row: "Studiengang",
    programs_of: |title| format!("Studiengänge von {title}"),
    search_program: "Anderen Studiengang suchen …",
    search_placeholder: "Studiengang suchen",
    my_program: "Mein Studiengang",
    with_module: "Studiengänge mit diesem Modul",
    from_filter: "aus deinem Filter im Katalog",
    from_timetable: "aus deinem Stundenplan",
    from_mine: "aus „Mein Studium“",
    compulsory_at: |n| format!("Pflichtmodul im {n}. Semester"),
    elective_at: |n| format!("Wahlpflichtmodul im {n}. Semester"),
    other_at: |n| format!("im {n}. Semester des Regelstudienplans"),
    not_in_plan: "Modul nicht in seinem Regelstudienplan",
    semester_short: "Sem.",
    fs_row: "Fachsemester",
    fs_long: |n| format!("{n}. Fachsemester"),
    fs_less: "Ein Fachsemester weniger",
    fs_more: "Ein Fachsemester mehr",
    since: |semester| format!("seit {semester}"),
    fs_fit: "passend zum Plan",
    fs_filter: |n, semester| format!("aus deinem Filter: das {n}. Semester ist bei dir {semester}"),
    fs_assumed: "angenommen",
    fs_follows: "folgt aus dem Regelstudienplan",
    module_row: "Einplanen in",
    this_semester: "dieses Semester",
    chosen_note: "für weitere Module gewählt",
    chosen_clear: "Aufheben",
    in_the_past: "liegt in der Vergangenheit",
    not_offered_there: "in diesem Semester nicht angeboten",
    as_in_plan: |n| format!("wie im Regelstudienplan ({n}. Semester)"),
    in_plan_at: |n| format!("im Regelstudienplan im {n}. Semester"),
    before_start: "vor Studienbeginn",
    semester_of: |title| format!("Semester von {title}"),
    past_head: |semester| format!("{semester} liegt hinter dir."),
    past_rest: "Betula plant es trotzdem dort ein, etwa wenn du es schon belegt hast.",
    behind_head: |n, semester| format!("Laut Regelstudienplan gehört es ins {n}. Fachsemester, bei dir {semester}."),
    behind_rest: |semester| format!("Das liegt hinter dir, darum steht es im {semester}; mit ‹ geht es auch dorthin."),
    without_study: "Ohne Studium einplanen",
    cancel: "Abbrechen",
    set_up_and_plan: "Einrichten und einplanen",
    choose_first: "Wähle zuerst deinen Studiengang",
};

pub const EN: Texts = Texts {
    plan: "Plan",
    planned: "Planned",
    for_placeholder: |name| format!("for \u{201c}{name}\u{201d}"),
    planned_in: |semesters| format!("planned: {semesters}"),
    unplan_hint: |semester| format!("Planned for {semester}. Once more removes the module from the plan"),
    unplan: "Remove from the plan",
    plan_for_hint: |semester, name| format!("Plan for {semester}, for \u{201c}{name}\u{201d}"),
    plan_hint: |semester| format!("Plan for {semester}"),
    fs_short: |n| format!("{} sem.", folia_design::i18n::format::ordinal(i64::from(n))),
    plan_more: "Into another semester",
    plan_more_hint: "Into another semester (⇧P)",
    plan_into: "Plan for",
    planned_there: "planned",
    earlier_semesters: "Earlier semesters",
    remove: "Remove",
    out_of: |semester| format!("from {semester}"),
    removed: "Removed",
    passed: "passed",
    passed_at: |semester| format!("passed: {semester}"),

    more: "Semester and passed",
    more_hint: "Change the semester, mark as passed (⇧P)",
    mark_passed: "Mark as passed",
    unmark_passed: "Remove passed mark",
    move_to: "Move to",
    move_title: |name| format!("Move \u{201c}{name}\u{201d} to"),
    now: "now",
    offered: "offered",
    not_offered: "not offered",
    leave: "Semester of leave",
    in_timetable: "Open in the timetable",

    note_planned: "planned",
    note_moved: "moved",
    note_behind: "in the past",
    note_not_offered: "not offered",
    earlier: "One semester earlier",
    later: "One semester later",
    keep_for: "Plan further modules for",
    keep_hint: "For this session: \u{201c}Plan\u{201d} puts the next modules into this semester as well",
    note_removed: "removed",
    note_passed: "marked as passed",
    note_unpassed: "no longer marked as passed",

    setup_title: |title| format!("Plan {title}"),
    setup_lead: "Set up your studies on the way, and your semesters of study show with the plan.",
    setup_lead_filled: "Set up your studies on the way, and your semesters of study show with the plan. All is filled in; change what is not right.",
    ask_program: "What do you study?",
    say_before: "You study ",
    say_middle: " in ",
    say_fs: |n| format!("semester {n}"),
    say_end: ".",
    program_row: "Degree programme",
    programs_of: |title| format!("Degree programmes of {title}"),
    search_program: "Find another programme …",
    search_placeholder: "Search degree programmes",
    my_program: "My programme",
    with_module: "Programmes with this module",
    from_filter: "from your filter in the catalogue",
    from_timetable: "from your timetable",
    from_mine: "from \u{201c}My studies\u{201d}",
    compulsory_at: |n| format!("Compulsory in semester {n}"),
    elective_at: |n| format!("Compulsory elective in semester {n}"),
    other_at: |n| format!("in semester {n} of the study plan"),
    not_in_plan: "Not in its study plan",
    semester_short: "sem.",
    fs_row: "Semester",
    fs_long: |n| format!("Semester {n}"),
    fs_less: "One semester less",
    fs_more: "One semester more",
    since: |semester| format!("since {semester}"),
    fs_fit: "to fit the plan",
    fs_filter: |n, semester| format!("from your filter: semester {n} is your {semester}"),
    fs_assumed: "assumed",
    fs_follows: "follows from the study plan",
    module_row: "Plan for",
    this_semester: "this semester",
    chosen_note: "chosen for further modules",
    chosen_clear: "Clear",
    in_the_past: "lies in the past",
    not_offered_there: "not offered in this semester",
    as_in_plan: |n| format!("as in the study plan (semester {n})"),
    in_plan_at: |n| format!("semester {n} in the study plan"),
    before_start: "before your studies",
    semester_of: |title| format!("Semester of {title}"),
    past_head: |semester| format!("{semester} is behind you."),
    past_rest: "Betula plans it there all the same, say if you took it then.",
    behind_head: |n, semester| format!("The study plan has it in semester {n}, for you {semester}."),
    behind_rest: |semester| format!("That is behind you, so it goes into {semester}; ‹ takes it back there too."),
    without_study: "Plan without setting up",
    cancel: "Cancel",
    set_up_and_plan: "Set up and plan",
    choose_first: "Choose your programme first",
};
