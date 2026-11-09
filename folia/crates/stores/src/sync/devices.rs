//! The devices of a synced profile (folia/design/profil/konzepte.html, the third step: the list of
//! the devices on the profile page). Phloem knows a device by its slot and a random id, nothing
//! more (protocol.md §2): what a device is („Pixel 7 · Chrome · Android") travels sealed in the
//! profile, as a record of its own that each device writes about itself, `D|<device id>`, which no
//! store shows (`folia_sync::Writer::put`; a build that does not know it keeps it and passes it
//! on). Whether a device is online comes from Phloem's presence by slot, how it is connected from
//! the live way (`live`).
//!
//! A device writes its record when it is not there or not as it is (another slot, another browser
//! of the same device id cannot be; a model learnt later), and otherwise once an hour while it is
//! connected: „zuletzt vor 2 Std." is as fresh as that. Another device may mark it removed
//! („Entfernen"): the device that finds its own record so leaves Betula Sync the next time it is
//! online and keeps its copy; until then it stands no more in the list. Phloem cannot take a slot
//! from a device that does not come back: for a lost device „Sync beenden" and a new start is what
//! shuts it out.
//!
//! What is here is plain: what a browser is (`describe`), the record's text, when it is due, the
//! list a page shows.

use super::codes::b64;

/// What the records' keys begin with.
pub const PREFIX: &str = "D|";

/// How often a device connected writes when it was last seen.
pub const SEEN_EVERY_MS: u64 = 60 * 60 * 1000;

/// What a record's text begins with: its layout.
const VERSION: &str = "1";

/// The key of the record of device `device` (its 16 bytes).
pub fn key_of(device: &[u8]) -> String {
    format!("{PREFIX}{}", b64(device))
}

/// The kind of a device, for its icon and its name where no model is known.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    Phone,
    Tablet,
    #[default]
    Computer,
}

impl Kind {
    fn code(self) -> &'static str {
        match self {
            Kind::Phone => "phone",
            Kind::Tablet => "tablet",
            Kind::Computer => "computer",
        }
    }

    fn of_code(code: &str) -> Kind {
        match code {
            "phone" => Kind::Phone,
            "tablet" => Kind::Tablet,
            _ => Kind::Computer,
        }
    }
}

/// What a browser is: its kind, its model where the browser tells it („Pixel 7", „iPhone"), the
/// browser („Chrome", „App" for Betula installed on the home screen) and the system („Android").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Described {
    pub kind: Kind,
    pub model: String,
    pub browser: String,
    pub system: String,
}

/// What a device wrote of itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    /// Its slot in the profile when it wrote this.
    pub slot: u16,
    pub described: Described,
    /// When it joined, and when it was last seen (ms since 1970, the server's time as the device
    /// knows it).
    pub since: u64,
    pub seen: u64,
    /// Another device took it out of the list („Entfernen"): it leaves when it comes.
    pub removed: bool,
}

/// One field of a record: no tab, no line break, at most 40 characters.
fn field(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().chars().take(40).collect()
}

impl Record {
    /// The record's value: `1 ‖ slot ‖ kind ‖ model ‖ browser ‖ system ‖ since ‖ seen ‖ removed`,
    /// tab-separated.
    pub fn text(&self) -> String {
        let d = &self.described;
        [VERSION.to_string(), self.slot.to_string(), d.kind.code().to_string(), field(&d.model), field(&d.browser), field(&d.system), self.since.to_string(), self.seen.to_string(), u8::from(self.removed).to_string()].join("\t")
    }

    /// A record's value read back; `None` for one of another layout (a later build's), which is
    /// left alone.
    pub fn read(text: &str) -> Option<Record> {
        let fields: Vec<&str> = text.split('\t').collect();
        let [version, slot, kind, model, browser, system, since, seen, removed] = fields.as_slice() else { return None };
        if *version != VERSION {
            return None;
        }
        Some(Record {
            slot: slot.parse().ok()?,
            described: Described { kind: Kind::of_code(kind), model: field(model), browser: field(browser), system: field(system) },
            since: since.parse().ok()?,
            seen: seen.parse().ok()?,
            removed: *removed == "1",
        })
    }

    /// The record this device writes now, after `before` (its record as the profile holds it).
    pub fn now(before: Option<&Record>, slot: u16, described: &Described, now: u64) -> Record {
        Record { slot, described: described.clone(), since: before.map_or(now, |b| if b.since == 0 { now } else { b.since }), seen: now, removed: false }
    }
}

/// Whether a device writes its record now: none yet, not as it is, or seen an hour ago.
pub fn due(before: Option<&Record>, slot: u16, described: &Described, now: u64) -> bool {
    match before {
        None => true,
        Some(b) => b.slot != slot || b.described != *described || now.saturating_sub(b.seen) >= SEEN_EVERY_MS,
    }
}

/// What `user_agent` is, with the model a browser tells apart (`model`: Chrome's
/// `userAgentData`), whether Betula runs installed (`standalone`) and whether the screen takes
/// touch (`touch`: an iPad asks for the Mac's pages).
pub fn describe(user_agent: &str, model: Option<&str>, standalone: bool, touch: bool) -> Described {
    let ua = user_agent;
    let has = |needle: &str| ua.contains(needle);
    let ipad = has("iPad") || (has("Macintosh") && touch);
    let iphone = has("iPhone") || has("iPod");
    let android = has("Android");
    let system = if iphone {
        "iOS"
    } else if ipad {
        "iPadOS"
    } else if android {
        "Android"
    } else if has("Windows") {
        "Windows"
    } else if has("CrOS") {
        "ChromeOS"
    } else if has("Macintosh") || has("Mac OS X") {
        "macOS"
    } else if has("Linux") {
        "Linux"
    } else {
        ""
    };
    let kind = if iphone || (android && has("Mobile")) {
        Kind::Phone
    } else if ipad || android {
        Kind::Tablet
    } else {
        Kind::Computer
    };
    let browser = if standalone {
        "App"
    } else if has("EdgA/") || has("EdgiOS/") || has("Edg/") {
        "Edge"
    } else if has("SamsungBrowser/") {
        "Samsung Internet"
    } else if has("OPR/") || has("OPT/") {
        "Opera"
    } else if has("Firefox/") || has("FxiOS/") {
        "Firefox"
    } else if has("CriOS/") || has("Chrome/") || has("Chromium/") {
        "Chrome"
    } else if has("Safari/") {
        "Safari"
    } else {
        ""
    };
    let model = match model.map(str::trim).filter(|m| !m.is_empty() && *m != "K") {
        Some(model) => model.to_string(),
        None if iphone => "iPhone".to_string(),
        None if ipad => "iPad".to_string(),
        None if system == "macOS" => "Mac".to_string(),
        None => String::new(),
    };
    Described { kind, model: field(&model), browser: browser.to_string(), system: system.to_string() }
}

/// How a device online is connected with this one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// Through Phloem alone.
    Phloem,
    /// Directly (WebRTC).
    Direct,
    /// Through Phloem's TURN.
    Relayed,
}

/// The slots online and how each is connected, as the leader keeps them
/// (`betula.sync.p<id>.slots`: `3 d`, `5 r`, `7 -`, a line each).
pub fn slots_text(slots: &[(u16, Link)]) -> String {
    slots.iter().map(|(slot, link)| format!("{slot} {}\n", match link { Link::Direct => "d", Link::Relayed => "r", Link::Phloem => "-" })).collect()
}

pub fn read_slots(text: &str) -> Vec<(u16, Link)> {
    text.lines()
        .filter_map(|line| {
            let (slot, link) = line.trim().split_once(' ')?;
            let link = match link.trim() {
                "d" => Link::Direct,
                "r" => Link::Relayed,
                _ => Link::Phloem,
            };
            Some((slot.parse().ok()?, link))
        })
        .collect()
}

/// A device as a page lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// The record's key (`D|…`): what `Sync::remove_device` takes.
    pub key: String,
    /// This device.
    pub mine: bool,
    pub described: Described,
    pub since: u64,
    pub seen: u64,
    /// `None`: not online now.
    pub online: Option<Link>,
}

impl Device {
    /// Its name: „Dieser Rechner" for this device, else the model („Pixel 7") or its kind
    /// („Rechner").
    pub fn shown(&self, locale: crate::i18n::Locale) -> String {
        let t = &crate::i18n::texts(locale).devices;
        let kind = self.described.kind;
        if self.mine {
            return match kind {
                Kind::Phone => t.this_phone,
                Kind::Tablet => t.this_tablet,
                Kind::Computer => t.this_computer,
            }
            .to_string();
        }
        if !self.described.model.is_empty() {
            return self.described.model.clone();
        }
        match kind {
            Kind::Phone => t.phone,
            Kind::Tablet => t.tablet,
            Kind::Computer => t.computer,
        }
        .to_string()
    }

    /// The line under its name: „Chrome · Android", the model for this device („Pixel 7 · Chrome").
    pub fn line(&self, locale: crate::i18n::Locale) -> String {
        let t = &crate::i18n::texts(locale).devices;
        let d = &self.described;
        let browser = if d.browser == "App" { t.app } else { d.browser.as_str() };
        let model = if self.mine { d.model.as_str() } else { "" };
        [model, browser, d.system.as_str()].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ")
    }

    /// Its icon (`folia_design::icons`).
    pub fn icon(&self) -> &'static str {
        match self.described.kind {
            Kind::Phone => "smartphone",
            Kind::Tablet => "tablet",
            Kind::Computer => "laptop",
        }
    }
}

/// The devices of a profile: the records (`(key, value)`, as the profile holds them), this device's
/// key, the slots online. This device first, then those online, then by when they were last seen;
/// none removed; and how many devices online wrote no record (a build before the list).
pub fn list(records: &[(String, String)], mine: &str, online: &[(u16, Link)]) -> (Vec<Device>, usize) {
    let mut devices: Vec<(Device, u16)> = records
        .iter()
        .filter_map(|(key, value)| {
            let record = Record::read(value)?;
            if record.removed {
                return None;
            }
            let is_mine = key == mine;
            let link = online.iter().find(|(slot, _)| *slot == record.slot).map(|(_, link)| *link);
            Some((Device { key: key.clone(), mine: is_mine, described: record.described, since: record.since, seen: record.seen, online: if is_mine { None } else { link } }, record.slot))
        })
        .collect();
    // Two records of one slot (a device that left without a word, and the one that has its slot
    // now): only the newer one is online.
    let mut by_slot: Vec<(u16, u64)> = Vec::new();
    for (device, slot) in &devices {
        match by_slot.iter_mut().find(|(s, _)| s == slot) {
            Some(newest) if newest.1 < device.since => newest.1 = device.since,
            Some(_) => {}
            None => by_slot.push((*slot, device.since)),
        }
    }
    for (device, slot) in &mut devices {
        if by_slot.iter().any(|(s, since)| s == slot && *since > device.since) {
            device.online = None;
        }
    }
    let named: Vec<u16> = devices.iter().filter(|(d, _)| d.online.is_some()).map(|(_, slot)| *slot).collect();
    let mine_slot = devices.iter().find(|(d, _)| d.mine).map(|(_, slot)| *slot);
    let unnamed = online.iter().filter(|(slot, _)| !named.contains(slot) && Some(*slot) != mine_slot).count();
    let mut devices: Vec<Device> = devices.into_iter().map(|(d, _)| d).collect();
    devices.sort_by(|a, b| b.mine.cmp(&a.mine).then(b.online.is_some().cmp(&a.online.is_some())).then(b.seen.cmp(&a.seen)));
    (devices, unnamed)
}

/// The records as the leader keeps them for every tab (`betula.sync.p<id>.devices`): a line each,
/// `key ‖ value`.
pub fn records_text(records: &[(String, String)]) -> String {
    records.iter().map(|(key, value)| format!("{key}\t{value}\n")).collect()
}

pub fn read_records(text: &str) -> Vec<(String, String)> {
    text.lines().filter_map(|line| line.split_once('\t')).filter(|(key, _)| key.starts_with(PREFIX)).map(|(key, value)| (key.to_string(), value.to_string())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIXEL: &str = "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36";
    const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1";
    const IPAD_AS_MAC: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15";
    const FIREFOX_WINDOWS: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:132.0) Gecko/20100101 Firefox/132.0";
    const EDGE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
    const SAMSUNG_TABLET: &str = "Mozilla/5.0 (Linux; Android 14; SM-X710) AppleWebKit/537.36 (KHTML, like Gecko) SamsungBrowser/26.0 Chrome/122.0.0.0 Safari/537.36";
    const CHROME_LINUX: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

    fn d(kind: Kind, model: &str, browser: &str, system: &str) -> Described {
        Described { kind, model: model.into(), browser: browser.into(), system: system.into() }
    }

    #[test]
    fn a_browser_says_what_it_is() {
        assert_eq!(describe(PIXEL, Some("Pixel 7"), false, true), d(Kind::Phone, "Pixel 7", "Chrome", "Android"));
        assert_eq!(describe(PIXEL, None, false, true), d(Kind::Phone, "", "Chrome", "Android"));
        assert_eq!(describe(IPHONE, None, false, true), d(Kind::Phone, "iPhone", "Safari", "iOS"));
        assert_eq!(describe(IPHONE, None, true, true), d(Kind::Phone, "iPhone", "App", "iOS"));
        assert_eq!(describe(IPAD_AS_MAC, None, false, true), d(Kind::Tablet, "iPad", "Safari", "iPadOS"));
        assert_eq!(describe(IPAD_AS_MAC, None, false, false), d(Kind::Computer, "Mac", "Safari", "macOS"));
        assert_eq!(describe(FIREFOX_WINDOWS, None, false, false), d(Kind::Computer, "", "Firefox", "Windows"));
        assert_eq!(describe(EDGE, None, false, false), d(Kind::Computer, "", "Edge", "Windows"));
        assert_eq!(describe(SAMSUNG_TABLET, Some("SM-X710"), false, true), d(Kind::Tablet, "SM-X710", "Samsung Internet", "Android"));
        assert_eq!(describe(CHROME_LINUX, None, false, false), d(Kind::Computer, "", "Chrome", "Linux"));
        assert_eq!(describe("", None, false, false), d(Kind::Computer, "", "", ""));
    }

    #[test]
    fn a_record_reads_back_and_keeps_its_fields_in_one_line() {
        let record = Record { slot: 3, described: d(Kind::Phone, "Pixel\t7\n", "Chrome", "Android"), since: 1_000, seen: 2_000, removed: false };
        let text = record.text();
        assert_eq!(text, "1\t3\tphone\tPixel 7\tChrome\tAndroid\t1000\t2000\t0");
        let read = Record::read(&text).unwrap();
        assert_eq!((read.slot, read.described.model.as_str(), read.since, read.seen, read.removed), (3, "Pixel 7", 1_000, 2_000, false));
        assert_eq!(Record::read(&Record { removed: true, ..read.clone() }.text()).map(|r| r.removed), Some(true));
        // Another layout is left alone.
        assert_eq!(Record::read("2\t3\tphone\tx"), None);
        assert_eq!(Record::read("1\tthree\tphone\t\t\t\t0\t0\t0"), None);
        assert_eq!(field(&"x".repeat(100)).len(), 40);
    }

    #[test]
    fn a_device_writes_when_it_is_new_changed_or_an_hour_on() {
        let me = d(Kind::Phone, "Pixel 7", "Chrome", "Android");
        let kept = Record::now(None, 3, &me, 10_000);
        assert_eq!((kept.since, kept.seen), (10_000, 10_000));
        assert!(due(None, 3, &me, 10_000));
        assert!(!due(Some(&kept), 3, &me, 10_000 + SEEN_EVERY_MS - 1));
        assert!(due(Some(&kept), 3, &me, 10_000 + SEEN_EVERY_MS));
        assert!(due(Some(&kept), 4, &me, 10_001));
        assert!(due(Some(&kept), 3, &d(Kind::Phone, "", "Chrome", "Android"), 10_001));
        // Written again, it keeps when it joined.
        assert_eq!(Record::now(Some(&kept), 3, &me, 99_000).since, 10_000);
    }

    #[test]
    fn a_device_is_named_by_its_model_or_its_kind() {
        use crate::i18n::Locale;
        let device = |mine: bool, described: Described| Device { key: "D|x".into(), mine, described, since: 0, seen: 0, online: None };
        let pixel = device(false, d(Kind::Phone, "Pixel 7", "Chrome", "Android"));
        assert_eq!((pixel.shown(Locale::De), pixel.line(Locale::De), pixel.icon()), ("Pixel 7".to_string(), "Chrome · Android".to_string(), "smartphone"));
        let laptop = device(false, d(Kind::Computer, "", "Firefox", "Windows"));
        assert_eq!((laptop.shown(Locale::De), laptop.shown(Locale::En)), ("Rechner".to_string(), "Computer".to_string()));
        let mine = device(true, d(Kind::Phone, "iPhone", "App", "iOS"));
        assert_eq!((mine.shown(Locale::De), mine.line(Locale::De)), ("Dieses Handy".to_string(), "iPhone · App · iOS".to_string()));
        assert_eq!(device(false, d(Kind::Tablet, "", "", "")).line(Locale::En), "");
    }

    #[test]
    fn the_list_puts_this_device_first_and_the_ones_online_next() {
        let rec = |slot: u16, model: &str, since: u64, seen: u64, removed: bool| Record { slot, described: d(Kind::Phone, model, "Chrome", "Android"), since, seen, removed }.text();
        let records = vec![
            ("D|me".to_string(), rec(1, "Laptop", 1, 900, false)),
            ("D|old".to_string(), rec(2, "Alt", 1, 100, false)),
            ("D|pixel".to_string(), rec(3, "Pixel 7", 5, 500, false)),
            ("D|gone".to_string(), rec(4, "Weg", 1, 800, true)),
            ("D|left".to_string(), rec(5, "Zuvor", 1, 700, false)),
            ("D|now5".to_string(), rec(5, "Jetzt", 9, 600, false)),
            ("D|later".to_string(), "2\tsomething newer".to_string()),
        ];
        let online = [(1, Link::Phloem), (3, Link::Direct), (5, Link::Relayed), (6, Link::Phloem)];
        let (devices, unnamed) = list(&records, "D|me", &online);
        let shown: Vec<(&str, Option<Link>)> = devices.iter().map(|d| (d.described.model.as_str(), d.online)).collect();
        assert_eq!(shown, [("Laptop", None), ("Jetzt", Some(Link::Relayed)), ("Pixel 7", Some(Link::Direct)), ("Zuvor", None), ("Alt", None)]);
        assert!(devices[0].mine && !devices[1].mine);
        // Slot 6 is online without a record of its own.
        assert_eq!(unnamed, 1);
        assert_eq!(read_records(&records_text(&records)), records);
        assert_eq!(read_slots(&slots_text(&online)), online);
    }
}
