//! Texts of the second direction of „Gerät verbinden" (folia/design/profil/konzepte.html, the
//! second step): the code a device shows to get a profile (`sync.rs`, `GetPart`), the scanner of
//! `/sync` (`scan.rs`, `connect.rs`) and what the phone asks before it sends a profile
//! (`give.rs`).

pub struct Texts {
    // ---- the two directions of the dialog ----
    /// „„Lena“ aufs Handy" (the value is the profile's name), „Profil vom Handy holen".
    pub dir_give: fn(&str) -> String,
    pub dir_get: &'static str,
    pub dir_label: &'static str,

    // ---- the code to get a profile ----
    /// While it is made; the QR code, for screen readers.
    pub making: &'static str,
    pub qr_label: &'static str,
    /// What to do on the phone: the bold start, the rest; the camera app as the other way.
    pub how_start: &'static str,
    pub how_rest: &'static str,
    pub how_camera: &'static str,
    /// „Dein Handy zeigt dann diese Ziffern:", and the digits for those who do not see them (the
    /// value is the digits).
    pub digits_lead: &'static str,
    pub digits_label: fn(&str) -> String,
    /// How long it still asks (minutes left, 0: less than one); gone; a new one.
    pub waiting: fn(u64) -> String,
    pub gone: &'static str,
    pub again: &'static str,
    /// What becomes of the profile that comes: into the empty only profile (its name), or a new one.
    pub into_empty: fn(&str) -> String,
    pub into_new: &'static str,
    /// „Kein Handy zur Hand?" and the way to the recovery code.
    pub no_phone: &'static str,
    pub no_phone_way: &'static str,
    /// It came: its name.
    pub came: fn(&str) -> String,
    pub done: &'static str,
    /// The same where the scanner was opened from the start page's setup: back to its last step.
    pub to_setup: &'static str,

    // ---- the scanner ----
    /// The page's line, over the camera, beside it.
    pub scan_hint: &'static str,
    pub scan_private: &'static str,
    pub scan_starting: &'static str,
    /// Why the camera does not show, each with the other way.
    pub scan_denied: &'static str,
    pub scan_no_camera: &'static str,
    pub scan_unsupported: &'static str,
    pub scan_retry: &'static str,
    /// A code read that is not one to connect with; one of another site of Betula.
    pub scan_other: &'static str,
    pub scan_other_site: &'static str,
    /// The other ways of the page: the recovery code; a code shown here for another device.
    pub scan_code: &'static str,
    pub scan_show: &'static str,
    pub scan_show_hint: &'static str,
    /// The way to it from a phone's profile page and its menu: „Code am Rechner scannen".
    pub scan_way: &'static str,

    // ---- the phone, before it sends a profile ----
    pub give_title: &'static str,
    pub give_which: &'static str,
    /// The line under a profile: synced; private, switched on for it.
    pub give_synced: &'static str,
    pub give_private: &'static str,
    /// A private profile chosen: what switching it on means (its name).
    pub give_switch_on: fn(&str) -> String,
    pub give_digits: &'static str,
    /// Under the digits (the profile's name), the answer, while it goes, once it is gone, once the
    /// other device has it.
    pub give_only_if: fn(&str) -> String,
    pub give_yes: fn(&str) -> String,
    pub give_sending: fn(&str) -> String,
    pub give_sent: fn(&str) -> String,
    pub give_joined: fn(&str) -> String,
    /// A code that is no request (read from an address by hand, cut short).
    pub give_bad: &'static str,
    /// Once the other device has it (`folia_widgets::syncdone::Connected`): who has what; from the
    /// setup what was set up besides the program; leaving before it came.
    pub give_said: fn(&str) -> String,
    pub summary_sync: &'static str,
    pub summary_other: &'static str,
    pub dont_wait: &'static str,
    pub give_back: &'static str,
}

pub const DE: Texts = Texts {
    dir_give: |name| format!("„{name}“ aufs Handy"),
    dir_get: "Profil vom Handy holen",
    dir_label: "Richtung",

    making: "Code wird vorbereitet …",
    qr_label: "QR-Code für dein Handy, das das Profil schickt",
    how_start: "Öffne Betula auf deinem Handy",
    how_rest: " und scanne diesen Code: Profil-Knopf › Gerät verbinden.",
    how_camera: "Läuft Betula dort im Browser, geht auch die Kamera-App.",
    digits_lead: "Dein Handy zeigt dann diese Ziffern:",
    digits_label: |digits| format!("Ziffern {}", digits.chars().map(String::from).collect::<Vec<_>>().join(" ")),
    waiting: |minutes| match minutes {
        0 => "Wartet auf dein Handy · noch weniger als eine Minute".to_string(),
        1 => "Wartet auf dein Handy · noch 1 Minute".to_string(),
        n => format!("Wartet auf dein Handy · noch {n} Minuten"),
    },
    gone: "Der Code ist abgelaufen.",
    again: "Neuer Code",
    into_empty: |name| format!("Kommt das Profil an, ist es das aktive; das leere „{name}“ geht dabei, es hielt nichts."),
    into_new: "Kommt das Profil an, ist es ein neues Profil dieses Browsers und das aktive.",
    no_phone: "Kein Handy zur Hand?",
    no_phone_way: "Wiederherstellungscode eingeben",
    came: |name| format!("„{name}“ ist jetzt auch auf diesem Gerät."),
    done: "Fertig",
    to_setup: "Zurück zur Einrichtung",

    scan_hint: "Halte die Kamera auf den Code am Rechner",
    scan_private: "Das Bild bleibt auf diesem Gerät. Betula liest nur, ob ein Code darin ist.",
    scan_starting: "Kamera wird eingeschaltet …",
    scan_denied: "Betula darf die Kamera nicht nutzen. Erlaube sie in den Einstellungen des Browsers, oder scanne den Code mit der Kamera-App: Läuft Betula im Browser, öffnet sie ihn hier.",
    scan_no_camera: "Hier gibt es keine Kamera, die Betula nutzen kann. Scanne den Code mit einem anderen Gerät, oder gib den Wiederherstellungscode ein.",
    scan_unsupported: "Dieser Browser gibt die Kamera nicht frei. Scanne den Code mit der Kamera-App, oder gib den Wiederherstellungscode ein.",
    scan_retry: "Erneut versuchen",
    scan_other: "Das ist kein Code von Betula zum Verbinden.",
    scan_other_site: "Dieser Code ist von einer anderen Seite von Betula. Öffne ihn dort.",
    scan_code: "Wiederherstellungscode eingeben",
    scan_show: "Hier einen Code zeigen",
    scan_show_hint: "Für ein Tablet oder ein zweites Handy",
    scan_way: "Code am Rechner scannen",

    give_title: "Ein Rechner möchte ein Profil",
    give_which: "Welches?",
    give_synced: "synchron",
    give_private: "nur auf diesem Gerät, wird dafür eingeschaltet",
    give_switch_on: |name| format!("Für „{name}“ wird dafür Betula Sync eingeschaltet: Der Server bewahrt verschlüsselte Änderungen auf, die er nicht öffnen kann."),
    give_digits: "Zeigt der Rechner diese Ziffern?",
    give_only_if: |name| format!("Nur bestätigen, wenn der Rechner vor dir steht: Er bekommt „{name}“ und bleibt verbunden, bis du ihn trennst."),
    give_yes: |name| format!("Ja, „{name}“ verbinden"),
    give_sending: |name| format!("„{name}“ geht an den Rechner …"),
    give_sent: |name| format!("„{name}“ ist unterwegs: Der Rechner zeigt gleich, dass es angekommen ist."),
    give_joined: |name| format!("Verbunden: Der Rechner hat „{name}“."),
    give_bad: "Dieser Code ist unvollständig. Lass dir am Rechner einen neuen zeigen und scanne ihn noch einmal.",
    give_said: |name| format!("Der Rechner hat jetzt „{name}“."),
    summary_sync: "Betula Sync an",
    summary_other: "Rechner verbunden",
    dont_wait: "Nicht warten",
    give_back: "Abbrechen",
};

pub const EN: Texts = Texts {
    dir_give: |name| format!("“{name}” to a phone"),
    dir_get: "Get a profile from your phone",
    dir_label: "Direction",

    making: "Preparing the code …",
    qr_label: "QR code for your phone, which sends the profile",
    how_start: "Open Betula on your phone",
    how_rest: " and scan this code: profile button › Connect a device.",
    how_camera: "If Betula runs in the browser there, the camera app works too.",
    digits_lead: "Your phone then shows these digits:",
    digits_label: |digits| format!("Digits {}", digits.chars().map(String::from).collect::<Vec<_>>().join(" ")),
    waiting: |minutes| match minutes {
        0 => "Waiting for your phone · less than a minute left".to_string(),
        1 => "Waiting for your phone · 1 minute left".to_string(),
        n => format!("Waiting for your phone · {n} minutes left"),
    },
    gone: "The code has expired.",
    again: "New code",
    into_empty: |name| format!("Once the profile is here, it is the active one; the empty “{name}” goes, it held nothing."),
    into_new: "Once the profile is here, it is a new profile of this browser and the active one.",
    no_phone: "No phone at hand?",
    no_phone_way: "Enter the recovery code",
    came: |name| format!("“{name}” is now on this device too."),
    done: "Done",
    to_setup: "Back to the set-up",

    scan_hint: "Hold the camera to the code on the computer",
    scan_private: "The picture stays on this device. Betula only reads whether a code is in it.",
    scan_starting: "Turning the camera on …",
    scan_denied: "Betula may not use the camera. Allow it in the browser's settings, or scan the code with the camera app: if Betula runs in the browser, it opens it here.",
    scan_no_camera: "There is no camera here Betula can use. Scan the code with another device, or enter the recovery code.",
    scan_unsupported: "This browser does not give out the camera. Scan the code with the camera app, or enter the recovery code.",
    scan_retry: "Try again",
    scan_other: "This is no code of Betula to connect with.",
    scan_other_site: "This code is from another site of Betula. Open it there.",
    scan_code: "Enter the recovery code",
    scan_show: "Show a code here",
    scan_show_hint: "For a tablet or a second phone",
    scan_way: "Scan the code on the computer",

    give_title: "A computer wants a profile",
    give_which: "Which one?",
    give_synced: "synced",
    give_private: "on this device only, switched on for it",
    give_switch_on: |name| format!("Betula Sync is switched on for “{name}”: the server keeps encrypted changes it cannot open."),
    give_digits: "Does the computer show these digits?",
    give_only_if: |name| format!("Only confirm when the computer is in front of you: it gets “{name}” and stays connected until you disconnect it."),
    give_yes: |name| format!("Yes, connect “{name}”"),
    give_sending: |name| format!("“{name}” is going to the computer …"),
    give_sent: |name| format!("“{name}” is on its way: the computer shows in a moment that it has arrived."),
    give_joined: |name| format!("Connected: the computer has “{name}”."),
    give_bad: "This code is incomplete. Have the computer show a new one and scan it again.",
    give_said: |name| format!("The computer now has “{name}”."),
    summary_sync: "Betula Sync on",
    summary_other: "Computer connected",
    dont_wait: "Don't wait",
    give_back: "Cancel",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digits_are_read_one_by_one() {
        assert_eq!((DE.digits_label)("7241"), "Ziffern 7 2 4 1");
        assert_eq!((EN.digits_label)("0731"), "Digits 0 7 3 1");
        assert!((DE.waiting)(0).contains("weniger als eine Minute") && (EN.waiting)(9).contains("9 minutes"));
    }
}
