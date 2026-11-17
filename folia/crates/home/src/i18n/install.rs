//! Texts of Betula as an app (`home/install.rs`; folia/design/installieren/konzepte.html, A and B):
//! the setup's last step „App", the card on the day, the guide of a browser that installs by its own
//! menu, and the profile an app on an iPhone or a Mac takes over from Safari.

pub struct Texts {
    /// The setup's chip.
    pub step_app: &'static str,
    /// The step's title and lead, on a phone and on a computer.
    pub title: &'static str,
    pub lead_phone: &'static str,
    pub lead_desk: &'static str,
    /// What one has of it, three points: on a phone, on a computer.
    pub points_phone: [&'static str; 3],
    pub points_desk: [&'static str; 3],
    /// The button that installs, by what it does in this browser: the browser's dialog; the guide
    /// to the home screen, the Dock, the taskbar.
    pub install: &'static str,
    pub to_home: &'static str,
    pub to_dock: &'static str,
    pub to_taskbar: &'static str,
    /// While the browser's dialog is open.
    pub asking: &'static str,
    /// „Später" of the step, and its title: where it comes again.
    pub later: &'static str,
    pub later_title: &'static str,

    // The card on the day.
    pub card_title: &'static str,
    pub card_phone: &'static str,
    pub card_desk: &'static str,
    /// Its ×: later.
    pub card_later: &'static str,
    pub never: &'static str,
    pub never_title: &'static str,

    // The guide of a browser that installs by its own menu.
    pub guide_home: &'static str,
    pub guide_dock: &'static str,
    pub guide_taskbar: &'static str,
    pub guide_lead: &'static str,
    pub steps_ios: [&'static str; 3],
    pub steps_mac: [&'static str; 3],
    pub steps_firefox_android: [&'static str; 3],
    pub steps_firefox_windows: [&'static str; 2],
    /// The profile that goes along to an app with storage of its own.
    pub carry_title: &'static str,
    pub carry_text: &'static str,
    pub copied: &'static str,
    pub not_copied: &'static str,
    pub copy_again: &'static str,
    pub understood: &'static str,

    // The app's first start on an iPhone or a Mac, its profile empty.
    pub take_title: &'static str,
    pub take_text: &'static str,
    pub take: &'static str,
    pub taking: &'static str,
    pub take_none: &'static str,
    pub take_newer: &'static str,
}

pub const DE: Texts = Texts {
    step_app: "App",
    title: "Betula als App?",
    lead_phone: "Leg Betula auf den Home-Bildschirm: Es startet wie jede andere App, ohne Browser drumherum.",
    lead_desk: "Leg Betula ins Dock oder in die Taskleiste: Es startet in einem eigenen Fenster, ohne Browser drumherum.",
    points_phone: [
        "Ein Tipp vom Home-Bildschirm, und du bist in deinem Tag.",
        "Der ganze Bildschirm, ohne Adressleiste und Tabs.",
        "Auch ohne Netz: Stundenplan, Studium und Katalog liegen auf dem Gerät.",
    ],
    points_desk: [
        "Ein Klick aus dem Dock oder der Taskleiste, und du bist in deinem Tag.",
        "Ein eigenes Fenster, ohne Adressleiste und Tabs.",
        "Auch ohne Netz: Stundenplan, Studium und Katalog liegen auf dem Gerät.",
    ],
    install: "Installieren",
    to_home: "Auf den Home-Bildschirm",
    to_dock: "Ins Dock legen",
    to_taskbar: "An die Taskleiste",
    asking: "Installieren …",
    later: "Später",
    later_title: "In ein paar Tagen fragt Betula auf der Startseite noch einmal",

    card_title: "Betula als App",
    card_phone: "Ein Tipp vom Home-Bildschirm, und du bist in deinem Tag. Ohne Browserleiste, auch ohne Netz.",
    card_desk: "Ein Klick aus dem Dock oder der Taskleiste, und du bist in deinem Tag. Eigenes Fenster, auch ohne Netz.",
    card_later: "Später",
    never: "Nicht mehr anzeigen",
    never_title: "Installieren geht weiter über das Menü des Browsers",

    guide_home: "Betula auf den Home-Bildschirm",
    guide_dock: "Betula ins Dock",
    guide_taskbar: "Betula an die Taskleiste",
    guide_lead: "Das macht dein Browser selbst, in wenigen Schritten:",
    steps_ios: [
        "Tippe auf „Teilen“: in Safari unten rechts unter •••, in Chrome oben in der Adressleiste.",
        "Wähle „Zum Home-Bildschirm“, lass „Als Web-App öffnen“ an und tippe „Hinzufügen“.",
        "Öffne Betula vom Home-Bildschirm und tippe „Studium einfügen“.",
    ],
    steps_mac: [
        "Wähle in der Menüleiste „Ablage“ › „Zum Dock hinzufügen …“.",
        "Klicke „Hinzufügen“.",
        "Öffne Betula aus dem Dock und klicke „Studium einfügen“.",
    ],
    steps_firefox_android: [
        "Tippe oben rechts auf ⋮.",
        "Wähle „Zum Startbildschirm hinzufügen“ (oder „App installieren“).",
        "Bestätige mit „Hinzufügen“.",
    ],
    steps_firefox_windows: [
        "Klicke rechts in der Adressleiste auf „Zur Taskleiste hinzufügen“.",
        "Betula öffnet sich in einem eigenen Fenster und bleibt in der Taskleiste.",
    ],
    carry_title: "Dein Studium kommt mit.",
    carry_text: "Die App hat ihren eigenen Speicher. Betula hat dein Profil dafür in die Zwischenablage gelegt.",
    copied: "Kopiert",
    not_copied: "Die Zwischenablage hat es nicht genommen.",
    copy_again: "Noch einmal kopieren",
    understood: "Verstanden",

    take_title: "Schon im Browser eingerichtet?",
    take_text: "Dann liegt dein Profil in der Zwischenablage: Studium, Stundenplan, Merkliste und Notizen kommen mit.",
    take: "Studium einfügen",
    taking: "Wird eingefügt …",
    take_none: "In der Zwischenablage ist kein Profil von Betula. Tippe im Browser noch einmal „Auf den Home-Bildschirm“ und dann hier „Studium einfügen“.",
    take_newer: "Das Profil kommt von einer neueren Betula. Schließe die App, öffne sie wieder und versuch es noch einmal.",
};

pub const EN: Texts = Texts {
    step_app: "App",
    title: "Betula as an app?",
    lead_phone: "Put Betula on your home screen: it starts like any other app, without a browser around it.",
    lead_desk: "Put Betula in your Dock or taskbar: it starts in a window of its own, without a browser around it.",
    points_phone: [
        "One tap on your home screen, and you are in your day.",
        "The whole screen, without an address bar or tabs.",
        "Offline too: your timetable, study and the catalog are on the device.",
    ],
    points_desk: [
        "One click in your Dock or taskbar, and you are in your day.",
        "A window of its own, without an address bar or tabs.",
        "Offline too: your timetable, study and the catalog are on the device.",
    ],
    install: "Install",
    to_home: "Add to home screen",
    to_dock: "Add to Dock",
    to_taskbar: "Add to taskbar",
    asking: "Installing …",
    later: "Later",
    later_title: "In a few days Betula asks again on the start page",

    card_title: "Betula as an app",
    card_phone: "One tap on your home screen, and you are in your day. No browser bar, offline too.",
    card_desk: "One click in your Dock or taskbar, and you are in your day. A window of its own, offline too.",
    card_later: "Later",
    never: "Don't show again",
    never_title: "You can still install from the browser's menu",

    guide_home: "Betula on your home screen",
    guide_dock: "Betula in your Dock",
    guide_taskbar: "Betula in your taskbar",
    guide_lead: "Your browser does this itself, in a few steps:",
    steps_ios: [
        "Tap “Share”: in Safari at the bottom right under •••, in Chrome in the address bar.",
        "Choose “Add to Home Screen”, leave “Open as Web App” on and tap “Add”.",
        "Open Betula from your home screen and tap “Paste study”.",
    ],
    steps_mac: [
        "In the menu bar choose “File” › “Add to Dock …”.",
        "Click “Add”.",
        "Open Betula from the Dock and click “Paste study”.",
    ],
    steps_firefox_android: [
        "Tap ⋮ at the top right.",
        "Choose “Add to Home screen” (or “Install app”).",
        "Confirm with “Add”.",
    ],
    steps_firefox_windows: [
        "Click “Add to taskbar” at the right of the address bar.",
        "Betula opens in a window of its own and stays in the taskbar.",
    ],
    carry_title: "Your study comes along.",
    carry_text: "The app keeps storage of its own. Betula put your profile on the clipboard for it.",
    copied: "Copied",
    not_copied: "The clipboard did not take it.",
    copy_again: "Copy again",
    understood: "Got it",

    take_title: "Set up in the browser already?",
    take_text: "Then your profile is on the clipboard: your study, timetable, saved modules and notes come along.",
    take: "Paste study",
    taking: "Pasting …",
    take_none: "There is no Betula profile on the clipboard. In the browser, tap “Add to home screen” again, then “Paste study” here.",
    take_newer: "The profile comes from a newer Betula. Close the app, open it again and try once more.",
};
