//! Texts of the link for the next device of Betula Sync (`syncpair.rs`): its QR code, the link, how
//! long it opens; why something of Betula Sync did not happen, each with what to do; and connecting
//! while it happens and once it is done (`syncdone.rs`).

pub struct Texts {
    /// What to do with the link; how long it opens (minutes left, 0: less than one); gone; a new
    /// one.
    pub pair_how: &'static str,
    pub pair_left: fn(u64) -> String,
    pub pair_gone: &'static str,
    pub pair_new: &'static str,
    /// The QR code, for screen readers.
    pub qr_label: &'static str,
    /// „Link kopieren", and „Kopiert" for a moment after.
    pub copy_link: &'static str,
    pub copied: &'static str,
    /// „Teilen" (the system's sharing, where there is one), and the title it hands over.
    pub share: &'static str,
    pub share_title: &'static str,
    pub close: &'static str,
    /// Why something did not happen, each with what to do (`folia_stores::sync::Problem`). The
    /// value of `no_room` is how many profiles a browser keeps.
    pub budget: &'static str,
    pub slow_down: &'static str,
    pub full: &'static str,
    pub bad_link: &'static str,
    pub unreachable: &'static str,
    pub unsupported: &'static str,
    pub no_room: fn(usize) -> String,

    // ---- Connecting, while it happens and once it is done (`syncdone.rs`) ----
    /// The progress bar, for screen readers.
    pub progress_label: &'static str,
    /// The steps: Betula Sync switched on, the profile sent, the other device fetching it, the
    /// profile received, the data alike.
    pub step_switch_on: &'static str,
    pub step_send: &'static str,
    pub step_fetch: &'static str,
    pub step_receive: &'static str,
    pub step_sync: &'static str,
    /// The success: „Verbunden", from the setup „Alles eingerichtet".
    pub connected: &'static str,
    pub all_set: &'static str,
    /// „Jetzt aktiv" and its three rows (a title and a sentence each).
    pub now_on: &'static str,
    pub gains: [(&'static str, &'static str); 3],
    /// More devices later, on the profile page.
    pub later: &'static str,
    pub later_where: &'static str,
    pub later_after: &'static str,
}

pub const DE: Texts = Texts {
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
    share: "Teilen",
    share_title: "Betula Sync",
    close: "Schließen",
    budget: "Betula nimmt gerade keine neuen Profile zum Synchronisieren an. Profile, die schon synchronisieren, laufen weiter. Versuch es in ein paar Tagen noch einmal.",
    slow_down: "Aus deinem Netz wurden in der letzten Stunde zu viele Profile eingerichtet. Versuch es in einer Stunde noch einmal.",
    full: "Dieses Profil ist schon auf 16 Geräten. Trenne zuerst eines davon („Nur dieses Gerät trennen“), dann klappt es.",
    bad_link: "Dieser Link oder Code öffnet nichts: Er ist vertippt, schon benutzt oder der Link ist älter als zehn Minuten. Lass dir auf dem anderen Gerät einen neuen geben.",
    unreachable: "Der Dienst von Betula Sync ist gerade nicht erreichbar. Prüf deine Verbindung und versuch es gleich noch einmal; deine Daten bleiben auf diesem Gerät.",
    unsupported: "Dieser Browser kann nicht synchronisieren: Ihm fehlt Web Crypto, Speicher oder WebSocket, oder er ist im privaten Modus. In einem aktuellen Browser geht es.",
    no_room: |max| format!("In diesem Browser gibt es schon {max} Profile. Lösch eines, dann lässt sich das Profil holen."),

    progress_label: "Verbinden",
    step_switch_on: "Betula Sync wird eingeschaltet",
    step_send: "Profil wird verschlüsselt gesendet",
    step_fetch: "Das andere Gerät holt das Profil ab",
    step_receive: "Profil wird empfangen",
    step_sync: "Daten werden abgeglichen",
    connected: "Verbunden",
    all_set: "Alles eingerichtet",
    now_on: "Jetzt aktiv",
    gains: [
        ("Auf beiden Geräten", "Stundenplan, Studium, Merkliste und Notizen sind auf beiden gleich."),
        ("Sofort abgeglichen", "Was du auf einem Gerät änderst, ist auf dem anderen gleich da. Auch nach einer Weile offline."),
        ("Verschlüsselt, ohne Konto", "Nur deine Geräte haben den Schlüssel; der Server kann nichts lesen."),
    ],
    later: "Weitere Geräte verbindest du jederzeit im Profil unter ",
    later_where: "„Gerät verbinden“",
    later_after: ".",
};

pub const EN: Texts = Texts {
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
    share: "Share",
    share_title: "Betula Sync",
    close: "Close",
    budget: "Betula is not taking new profiles to sync right now. Profiles that sync already go on. Try again in a few days.",
    slow_down: "Too many profiles were set up from your network in the last hour. Try again in an hour.",
    full: "This profile is on 16 devices already. Disconnect one of them first (“Disconnect only this device”), then it works.",
    bad_link: "This link or code opens nothing: it is mistyped, used already, or the link is older than ten minutes. Get a new one on the other device.",
    unreachable: "The service of Betula Sync cannot be reached right now. Check your connection and try again in a moment; your data stays on this device.",
    unsupported: "This browser cannot sync: it lacks Web Crypto, storage or WebSocket, or it is in private mode. A current browser can.",
    no_room: |max| format!("There are {max} profiles in this browser already. Delete one, then the profile can be added."),

    progress_label: "Connecting",
    step_switch_on: "Turning on Betula Sync",
    step_send: "Sending the profile, encrypted",
    step_fetch: "The other device is fetching the profile",
    step_receive: "Receiving the profile",
    step_sync: "Bringing the data in line",
    connected: "Connected",
    all_set: "All set",
    now_on: "Now on",
    gains: [
        ("On both devices", "Timetable, studies, saved modules and notes are the same on both."),
        ("In line at once", "What you change on one device is on the other right away. Even after a while offline."),
        ("Encrypted, no account", "Only your devices have the key; the server cannot read anything."),
    ],
    later: "You can connect more devices at any time in the profile under ",
    later_where: "“Connect a device”",
    later_after: ".",
};
