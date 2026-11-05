//! Texts of Betula Sync on the profile page (`sync.rs`) and of „Gerät verbinden" (`connect.rs`).

pub struct Texts {
    // ---- the part of the profile page ----
    /// „Betula Sync": the part's heading.
    pub heading: &'static str,
    /// What a private profile is, and what switching it on does.
    pub private_lead: &'static str,
    /// „Betula Sync aktivieren" and the line under it.
    pub activate: &'static str,
    pub activate_hint: &'static str,
    /// Before it is switched on: what the server then keeps, as a heading and points, what the
    /// recovery code is for, and the answers.
    pub explain_heading: &'static str,
    pub explain_points: &'static [&'static str],
    pub explain_code: &'static str,
    pub explain_yes: &'static str,
    pub cancel: &'static str,
    /// While it is switched on, or a profile comes from another device.
    pub starting: &'static str,
    /// A synced profile, online with how many other devices of it; offline.
    pub online: fn(usize) -> String,
    pub offline: &'static str,
    /// What a synced profile is, under its state.
    pub synced_lead: &'static str,
    /// Ended on another device (or untouched for 13 months): private again; „Wieder aktivieren".
    pub ended: &'static str,
    pub ended_again: &'static str,
    /// Something failed: the line before why, and „Erneut versuchen".
    pub failed: &'static str,
    pub retry: &'static str,
    /// Why something did not happen, each with what to do (`folia_stores::sync::Problem`). The
    /// value of `no_room` is how many profiles a browser keeps.
    pub budget: &'static str,
    pub slow_down: &'static str,
    pub full: &'static str,
    pub bad_link: &'static str,
    pub unreachable: &'static str,
    pub unsupported: &'static str,
    pub no_room: fn(usize) -> String,

    // ---- a synced profile's actions ----
    /// „Weiteres Gerät verbinden" and the line under it.
    pub pair: &'static str,
    pub pair_hint: &'static str,
    /// While the link is made; what to do with it; how long it opens (minutes left, 0: less than
    /// one); gone; a new one.
    pub pairing: &'static str,
    pub pair_how: &'static str,
    pub pair_left: fn(u64) -> String,
    pub pair_gone: &'static str,
    pub pair_new: &'static str,
    /// The QR code, for screen readers.
    pub qr_label: &'static str,
    /// „Link kopieren", and „Kopiert" for a moment after.
    pub copy_link: &'static str,
    pub copied: &'static str,
    pub close: &'static str,
    /// „Wiederherstellungscode zeigen", the line under it, what it is, „Code kopieren",
    /// „Verbergen".
    pub code_show: &'static str,
    pub code_hint: &'static str,
    pub code_lead: &'static str,
    pub code_copy: &'static str,
    pub code_hide: &'static str,
    /// „Nur dieses Gerät trennen": the line under it, its question and its answer.
    pub leave: &'static str,
    pub leave_hint: &'static str,
    pub leave_question: &'static str,
    pub leave_yes: &'static str,
    /// „Sync beenden": the same.
    pub stop: &'static str,
    pub stop_hint: &'static str,
    pub stop_question: &'static str,
    pub stop_yes: &'static str,

    // ---- a profile from another device ----
    /// The heading, what to do, the field of the recovery code (its label and an example) and its
    /// button; done: the new profile's name.
    pub other_heading: &'static str,
    pub other_lead: &'static str,
    pub code_field: &'static str,
    pub code_placeholder: &'static str,
    pub recover: &'static str,
    pub recovered: fn(&str) -> String,

    // ---- what two devices did at once ----
    /// „2 Stellen auf zwei Geräten gleichzeitig geändert"; the value is how many.
    pub conflicts: fn(usize) -> String,
    pub conflicts_hint: &'static str,
    /// A module's note, a Termin's note, a module moved; the values are the module's name.
    pub conflict_note: fn(&str) -> String,
    pub conflict_termin: &'static str,
    pub conflict_moved: fn(&str) -> String,
    /// The way to it.
    pub conflict_open: &'static str,

    // ---- „Gerät verbinden" (`/verbinden`) ----
    pub connect_title: &'static str,
    pub connect_description: &'static str,
    /// The server's page, which runs nothing.
    pub connect_server_title: &'static str,
    pub connect_server_hint: &'static str,
    /// While the profile comes, and when it is here; the value is its name.
    pub connecting: &'static str,
    pub connected: fn(&str) -> String,
    pub connected_hint: &'static str,
    /// A link without its part after `#`, and a site that does not offer Betula Sync.
    pub connect_none: &'static str,
    pub connect_off: &'static str,
    /// The ways on.
    pub to_studyplan: &'static str,
    pub to_profile: &'static str,
    pub to_start: &'static str,
}

pub const DE: Texts = Texts {
    heading: "Betula Sync",
    private_lead: "Aus: Dieses Profil liegt nur in diesem Browser. Mit Betula Sync bleibt es auf deinem Handy, Laptop und jedem weiteren Gerät gleich, auch wenn du auf zweien gleichzeitig etwas änderst.",
    activate: "Betula Sync aktivieren",
    activate_hint: "Für dieses Profil, verschlüsselt",
    explain_heading: "Was der Server dann aufbewahrt",
    explain_points: &[
        "Deine Änderungen, verschlüsselt mit einem Schlüssel, den nur deine Geräte haben. Der Server kann sie nicht öffnen und weiß nicht, was drinsteht.",
        "Wie groß sie sind und wann sie kamen, dazu zufällige Kennungen für das Profil und jedes Gerät. Mehr nicht: kein Name, kein Konto, keine E-Mail-Adresse.",
        "Wird das Profil 13 Monate lang nicht abgerufen oder beendest du Sync, ist alles auf dem Server weg. Deine Geräte behalten ihre Kopie.",
    ],
    explain_code: "Danach kannst du dir einen Wiederherstellungscode zeigen lassen. Ohne ihn ist die Kopie auf dem Server verloren, wenn alle deine Geräte weg sind; mit ihm holst du das Profil auf jedes neue Gerät.",
    explain_yes: "Aktivieren",
    cancel: "Abbrechen",
    starting: "Wird eingerichtet …",
    online: |others| match others {
        0 => "Synchronisiert · gerade kein anderes Gerät online".to_string(),
        1 => "Synchronisiert · 1 weiteres Gerät online".to_string(),
        n => format!("Synchronisiert · {n} weitere Geräte online"),
    },
    offline: "Synchronisiert · gerade offline: Änderungen warten hier und gehen raus, sobald wieder Netz da ist",
    synced_lead: "Jede Änderung geht sofort an deine anderen Geräte dieses Profils. Was zwei Geräte gleichzeitig an derselben Stelle ändern, zeigt Betula dir dort.",
    ended: "Sync wurde auf einem anderen Gerät beendet oder das Profil 13 Monate lang nicht abgerufen. Es ist wieder privat und hat alles behalten.",
    ended_again: "Wieder aktivieren",
    failed: "Das hat nicht geklappt.",
    retry: "Erneut versuchen",
    budget: "Betula nimmt gerade keine neuen Profile zum Synchronisieren an. Profile, die schon synchronisieren, laufen weiter. Versuch es in ein paar Tagen noch einmal.",
    slow_down: "Aus deinem Netz wurden in der letzten Stunde zu viele Profile eingerichtet. Versuch es in einer Stunde noch einmal.",
    full: "Dieses Profil ist schon auf 16 Geräten. Trenne zuerst eines davon („Nur dieses Gerät trennen“), dann klappt es.",
    bad_link: "Dieser Link oder Code öffnet nichts: Er ist vertippt, schon benutzt oder der Link ist älter als zehn Minuten. Lass dir auf dem anderen Gerät einen neuen geben.",
    unreachable: "Der Dienst von Betula Sync ist gerade nicht erreichbar. Prüf deine Verbindung und versuch es gleich noch einmal; deine Daten bleiben auf diesem Gerät.",
    unsupported: "Dieser Browser kann nicht synchronisieren: Ihm fehlt Web Crypto, Speicher oder WebSocket, oder er ist im privaten Modus. In einem aktuellen Browser geht es.",
    no_room: |max| format!("In diesem Browser gibt es schon {max} Profile. Lösch eines, dann lässt sich das Profil holen."),

    pair: "Weiteres Gerät verbinden",
    pair_hint: "QR-Code oder Link, zehn Minuten gültig",
    pairing: "Link wird vorbereitet …",
    pair_how: "Scanne den Code mit der Kamera des anderen Geräts oder öffne den Link dort. Er gilt einmal: Wer ihn hat, bekommt dieses Profil.",
    pair_left: |minutes| match minutes {
        0 => "Noch weniger als eine Minute gültig".to_string(),
        1 => "Noch 1 Minute gültig".to_string(),
        n => format!("Noch {n} Minuten gültig"),
    },
    pair_gone: "Der Link ist abgelaufen.",
    pair_new: "Neuer Link",
    qr_label: "QR-Code des Links für das andere Gerät",
    copy_link: "Link kopieren",
    copied: "Kopiert",
    close: "Schließen",
    code_show: "Wiederherstellungscode zeigen",
    code_hint: "Holt das Profil ohne deine Geräte zurück",
    code_lead: "Wer diesen Code hat, hat das Profil. Bewahre ihn auf wie ein Passwort, etwa in deinem Passwortmanager; auf ein neues Gerät holst du das Profil damit unter „Profil von einem anderen Gerät“.",
    code_copy: "Code kopieren",
    code_hide: "Verbergen",
    leave: "Nur dieses Gerät trennen",
    leave_hint: "Die anderen Geräte synchronisieren weiter",
    leave_question: "Dieses Gerät aus Betula Sync nehmen? Es behält seine Kopie, die anderen Geräte gleichen weiter ab.",
    leave_yes: "Trennen",
    stop: "Sync beenden",
    stop_hint: "Löscht die Kopie auf dem Server",
    stop_question: "Sync für dieses Profil auf allen Geräten beenden? Die Kopie auf dem Server wird sofort gelöscht; jedes Gerät behält seine und ist wieder privat.",
    stop_yes: "Beenden",

    other_heading: "Profil von einem anderen Gerät",
    other_lead: "Öffne dort „Weiteres Gerät verbinden“ und scanne den Code, oder gib hier den Wiederherstellungscode ein. Das Profil kommt als neues Profil dazu.",
    code_field: "Wiederherstellungscode",
    code_placeholder: "XXXX-XXXX-…",
    recover: "Profil holen",
    recovered: |name| format!("„{name}“ ist jetzt auch auf diesem Gerät."),

    conflicts: |n| if n == 1 { "1 Stelle auf zwei Geräten gleichzeitig geändert".to_string() } else { format!("{n} Stellen auf zwei Geräten gleichzeitig geändert") },
    conflicts_hint: "Die spätere Fassung gilt, die andere liegt daneben. Entscheide dort, welche bleibt.",
    conflict_note: |module| format!("Notiz zu „{module}“"),
    conflict_termin: "Notiz an einem Termin",
    conflict_moved: |module| format!("„{module}“ verschoben"),
    conflict_open: "Ansehen",

    connect_title: "Gerät verbinden",
    connect_description: "Ein Profil von Betula mit seinen anderen Geräten verbinden: Betula Sync, verschlüsselt.",
    connect_server_title: "Gerät verbinden",
    connect_server_hint: "Der Link holt ein Profil von deinem anderen Gerät. Das macht dein Browser selbst, mit JavaScript: Öffne den Link in einem aktuellen Browser.",
    connecting: "Hole das Profil von deinem anderen Gerät …",
    connected: |name| format!("„{name}“ ist jetzt auch auf diesem Gerät."),
    connected_hint: "Es bleibt mit deinen anderen Geräten gleich: Was du hier änderst, ist dort sofort da, und umgekehrt.",
    connect_none: "Dieser Link ist unvollständig. Öffne ihn noch einmal vom anderen Gerät aus oder lass dir dort einen neuen geben („Weiteres Gerät verbinden“).",
    connect_off: "Betula Sync ist auf dieser Seite nicht eingeschaltet. Öffne den Link auf der Seite, auf der du ihn bekommen hast.",
    to_studyplan: "Zum Stundenplan",
    to_profile: "Zum Profil",
    to_start: "Zur Startseite",
};

pub const EN: Texts = Texts {
    heading: "Betula Sync",
    private_lead: "Off: this profile is kept in this browser only. With Betula Sync it stays the same on your phone, your laptop and every other device, even when you change something on two of them at once.",
    activate: "Turn on Betula Sync",
    activate_hint: "For this profile, encrypted",
    explain_heading: "What the server then keeps",
    explain_points: &[
        "Your changes, encrypted with a key only your devices have. The server cannot open them and does not know what is in them.",
        "How large they are and when they came, and random ids for the profile and each device. Nothing more: no name, no account, no e-mail address.",
        "If the profile is not fetched for 13 months, or you end Sync, everything on the server is gone. Your devices keep their copy.",
    ],
    explain_code: "Afterwards you can show a recovery code. Without it the copy on the server is lost when all your devices are gone; with it you get the profile onto any new device.",
    explain_yes: "Turn on",
    cancel: "Cancel",
    starting: "Setting up …",
    online: |others| match others {
        0 => "Synced · no other device online right now".to_string(),
        1 => "Synced · 1 other device online".to_string(),
        n => format!("Synced · {n} other devices online"),
    },
    offline: "Synced · offline right now: changes wait here and go out as soon as there is a network again",
    synced_lead: "Every change goes to your other devices of this profile at once. What two devices change in the same place at the same time, Betula shows you there.",
    ended: "Sync was ended on another device, or the profile was not fetched for 13 months. It is private again and has kept everything.",
    ended_again: "Turn on again",
    failed: "That did not work.",
    retry: "Try again",
    budget: "Betula is not taking new profiles to sync right now. Profiles that sync already go on. Try again in a few days.",
    slow_down: "Too many profiles were set up from your network in the last hour. Try again in an hour.",
    full: "This profile is on 16 devices already. Disconnect one of them first (“Disconnect only this device”), then it works.",
    bad_link: "This link or code opens nothing: it is mistyped, used already, or the link is older than ten minutes. Get a new one on the other device.",
    unreachable: "The service of Betula Sync cannot be reached right now. Check your connection and try again in a moment; your data stays on this device.",
    unsupported: "This browser cannot sync: it lacks Web Crypto, storage or WebSocket, or it is in private mode. A current browser can.",
    no_room: |max| format!("There are {max} profiles in this browser already. Delete one, then the profile can be added."),

    pair: "Connect another device",
    pair_hint: "QR code or link, valid for ten minutes",
    pairing: "Preparing the link …",
    pair_how: "Scan the code with the camera of the other device, or open the link there. It works once: whoever has it gets this profile.",
    pair_left: |minutes| match minutes {
        0 => "Valid for less than a minute".to_string(),
        1 => "Valid for 1 more minute".to_string(),
        n => format!("Valid for {n} more minutes"),
    },
    pair_gone: "The link has expired.",
    pair_new: "New link",
    qr_label: "QR code of the link for the other device",
    copy_link: "Copy link",
    copied: "Copied",
    close: "Close",
    code_show: "Show recovery code",
    code_hint: "Gets the profile back without your devices",
    code_lead: "Whoever has this code has the profile. Keep it like a password, for example in your password manager; on a new device you get the profile with it under “Profile from another device”.",
    code_copy: "Copy code",
    code_hide: "Hide",
    leave: "Disconnect only this device",
    leave_hint: "The other devices go on syncing",
    leave_question: "Take this device out of Betula Sync? It keeps its copy, the other devices go on syncing.",
    leave_yes: "Disconnect",
    stop: "End Sync",
    stop_hint: "Deletes the copy on the server",
    stop_question: "End Sync for this profile on every device? The copy on the server is deleted at once; every device keeps its own and is private again.",
    stop_yes: "End",

    other_heading: "Profile from another device",
    other_lead: "Open “Connect another device” there and scan the code, or enter the recovery code here. The profile is added as a new profile.",
    code_field: "Recovery code",
    code_placeholder: "XXXX-XXXX-…",
    recover: "Get profile",
    recovered: |name| format!("“{name}” is now on this device too."),

    conflicts: |n| if n == 1 { "1 place changed on two devices at the same time".to_string() } else { format!("{n} places changed on two devices at the same time") },
    conflicts_hint: "The later version counts, the other lies beside it. Decide there which one stays.",
    conflict_note: |module| format!("Note on “{module}”"),
    conflict_termin: "Note at an appointment",
    conflict_moved: |module| format!("“{module}” moved"),
    conflict_open: "View",

    connect_title: "Connect a device",
    connect_description: "Connect a Betula profile with its other devices: Betula Sync, encrypted.",
    connect_server_title: "Connect a device",
    connect_server_hint: "The link gets a profile from your other device. Your browser does that itself, with JavaScript: open the link in a current browser.",
    connecting: "Getting the profile from your other device …",
    connected: |name| format!("“{name}” is now on this device too."),
    connected_hint: "It stays the same as on your other devices: what you change here is there at once, and the other way round.",
    connect_none: "This link is incomplete. Open it again from the other device, or get a new one there (“Connect another device”).",
    connect_off: "Betula Sync is not switched on on this site. Open the link on the site where you got it.",
    to_studyplan: "To the timetable",
    to_profile: "To the profile",
    to_start: "To the start page",
};
