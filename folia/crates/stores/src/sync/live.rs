//! The live way between a profile's devices (Betula Sync, step 2; research/sync/entwurf.html,
//! „Live"): WebRTC between every two devices online, each change as the same sealed batch Phloem
//! keeps, at once to the other device and to Phloem; what arrives twice works once. The signals
//! that set a connection up go through Phloem sealed with the live key, so that Phloem cannot put
//! itself in between. The first attempt takes STUN alone (a direct way, in the same network or
//! across the internet); when it does not come about, the next ones take TURN as well.
//!
//! What lives here is pure (the browser half is `browser::rtc`): what a signal says and what it
//! is sealed to, which ICE servers an attempt takes, how long a device waits before it tries
//! again, and the leader's word on the connection.

use serde::{Deserialize, Serialize};

/// What one device tells another to set their connection up, sealed with the live key
/// (protocol.md §5). `n` names the attempt: a random number of the device that offered it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "lowercase")]
pub enum Talk {
    /// The offer of attempt `n`; `relay`: the attempt takes TURN.
    Offer { n: u32, sdp: String, relay: bool },
    Answer { n: u32, sdp: String },
    /// A candidate of attempt `n`.
    Ice {
        n: u32,
        c: String,
        #[serde(default)]
        mid: Option<String>,
        #[serde(default)]
        line: Option<u16>,
    },
    /// Attempt `n` is over on the sender's side (it failed, or the sender closed it).
    Bye { n: u32 },
}

impl Talk {
    pub fn n(&self) -> u32 {
        match self {
            Talk::Offer { n, .. } | Talk::Answer { n, .. } | Talk::Ice { n, .. } | Talk::Bye { n } => *n,
        }
    }

    pub fn bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn read(bytes: &[u8]) -> Option<Talk> {
        serde_json::from_slice(bytes).ok()
    }
}

/// What a signal is sealed to: the space, `signal`, the slot it comes from and the one it is for
/// (two bytes each, big endian). Phloem names the slot a signal came from; a signal it handed to
/// another device than the one meant, or named with another sender, does not open.
pub fn talk_aad(space: &[u8; 16], from: u16, to: u16) -> Vec<u8> {
    let mut out = space.to_vec();
    out.extend_from_slice(b"signal");
    out.extend_from_slice(&from.to_be_bytes());
    out.extend_from_slice(&to.to_be_bytes());
    out
}

/// The ICE servers of the socket and its TURN credential (the frame `turn`; no URLs where the
/// site has no TURN: then two devices find each other in the same network alone).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ice {
    pub urls: Vec<String>,
    pub user: String,
    pub password: String,
}

impl Ice {
    /// The STUN servers: for every attempt.
    pub fn stun(&self) -> Vec<String> {
        self.urls.iter().filter(|u| u.starts_with("stun:") || u.starts_with("stuns:")).cloned().collect()
    }

    /// The TURN servers: for an attempt with TURN, with the credential.
    pub fn turn(&self) -> Vec<String> {
        self.urls.iter().filter(|u| u.starts_with("turn:") || u.starts_with("turns:")).cloned().collect()
    }

    pub fn has_turn(&self) -> bool {
        !self.turn().is_empty() && !self.user.is_empty()
    }
}

/// Whether attempt number `tries` (1 for the first since the last connection) takes TURN: the
/// first goes without, every later one with, where there is TURN; the first too when the last
/// connection with that device needed TURN (`remembered`), so that a page loaded again does not
/// wait for the direct way that was not there before.
pub fn relay(tries: u32, remembered: bool, ice: &Ice) -> bool {
    (tries >= 2 || remembered) && ice.has_turn()
}

/// How long an attempt may take before it counts as failed, in ms: a direct way is there within
/// seconds or not at all; TURN takes its allocations first (and TLS, where only that gets out).
pub fn attempt_ms(relay: bool) -> u64 {
    if relay {
        15_000
    } else {
        8_000
    }
}

/// How long a device remembers that the connection with another needed TURN, in seconds.
pub const REMEMBER_RELAY_SECS: u64 = 24 * 3600;

/// The slots whose last connection needed TURN, with when it opened (seconds), as kept beside the
/// profile (`betula.sync.p<id>.relay`): `<slot>\t<secs>` per line. What is older than a day is
/// left out.
pub fn read_relayed(text: &str, now: u64) -> std::collections::BTreeMap<u16, u64> {
    text.lines()
        .filter_map(|line| {
            let (slot, at) = line.split_once('\t')?;
            Some((slot.parse().ok()?, at.parse().ok()?))
        })
        .filter(|(_, at): &(u16, u64)| now.saturating_sub(*at) < REMEMBER_RELAY_SECS)
        .collect()
}

pub fn relayed_text(relayed: &std::collections::BTreeMap<u16, u64>) -> String {
    relayed.iter().map(|(slot, at)| format!("{slot}\t{at}\n")).collect()
}

/// How long a connection may stay disconnected before it counts as failed, in ms (a phone
/// switching networks comes back within it).
pub const DISCONNECTED_MS: u64 = 6_000;

/// How long the offering device waits before attempt `tries` after the one before failed, in
/// ms: the second (with TURN) at once, then 30 s, 1, 2 and 4 minutes, at most 5.
pub fn wait_before(tries: u32) -> u64 {
    match tries {
        0..=2 => 500,
        n => (15_000u64 << (n - 2).min(5)).min(300_000),
    }
}

/// The largest message the data channel carries: what every browser takes (Safari's SCTP
/// stops at 64 KiB); a larger batch goes through Phloem alone.
pub const MAX_MESSAGE: usize = 60_000;

/// A message on the data channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Direct {
    /// One change: the sealed batch, as Phloem keeps it (`space ‖ batch`).
    Push { data: String },
}

impl Direct {
    pub fn text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn read(text: &str) -> Option<Direct> {
        if text.len() > 4 * MAX_MESSAGE {
            return None;
        }
        serde_json::from_str(text).ok()
    }
}

/// The leader's word on the connection to Phloem and to the other devices online: kept in
/// `betula.sync.p<id>.status` as `online <devices> direct <d> relayed <r>`, or `offline`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Online {
    /// Other devices of the profile online.
    pub devices: usize,
    /// ... connected with this one directly (WebRTC, the same network or across the internet).
    pub direct: usize,
    /// ... connected through Phloem's TURN.
    pub relayed: usize,
}

pub fn status_text(online: Option<Online>) -> String {
    match online {
        Some(o) => format!("online {} direct {} relayed {}", o.devices, o.direct, o.relayed),
        None => "offline".to_string(),
    }
}

/// The word read back; `None` for offline (or nothing kept). A word of an older build
/// (`online <n>`) reads with no direct connection.
pub fn read_status(text: &str) -> Option<Online> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.first() != Some(&"online") {
        return None;
    }
    let number = |at: usize| words.get(at).and_then(|n| n.parse().ok()).unwrap_or(0);
    let named = |name: &str| words.iter().position(|w| *w == name).map_or(0, |at| number(at + 1));
    Some(Online { devices: number(1), direct: named("direct"), relayed: named("relayed") })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signal_reads_back_and_is_sealed_to_both_slots() {
        for talk in [
            Talk::Offer { n: 7, sdp: "v=0\r\n".into(), relay: true },
            Talk::Answer { n: 7, sdp: "v=0\r\n".into() },
            Talk::Ice { n: 7, c: "candidate:1 1 udp 2122260223 192.168.1.2 50000 typ host".into(), mid: Some("0".into()), line: Some(0) },
            Talk::Bye { n: 7 },
        ] {
            assert_eq!(Talk::read(&talk.bytes()), Some(talk.clone()));
            assert_eq!(talk.n(), 7);
        }
        assert_eq!(Talk::read(br#"{"k":"ice","n":1,"c":"x"}"#), Some(Talk::Ice { n: 1, c: "x".into(), mid: None, line: None }));
        assert_eq!(Talk::read(b"{\"k\":\"other\"}"), None);
        let space = [9u8; 16];
        let aad = talk_aad(&space, 2, 5);
        assert_eq!(aad.len(), 16 + 6 + 4);
        assert_eq!(&aad[16..22], b"signal");
        assert_eq!(&aad[22..], &[0, 2, 0, 5]);
        assert_ne!(talk_aad(&space, 5, 2), aad, "the way it goes is sealed too");
    }

    #[test]
    fn the_first_attempt_goes_without_turn() {
        let ice = Ice {
            urls: vec!["stun:betula.app:3478".into(), "turn:betula.app:3478?transport=udp".into(), "turns:turn.betula.app:443?transport=tcp".into()],
            user: "u".into(),
            password: "p".into(),
        };
        assert_eq!(ice.stun(), vec!["stun:betula.app:3478".to_string()]);
        assert_eq!(ice.turn().len(), 2);
        assert!(!relay(1, false, &ice) && relay(2, false, &ice) && relay(9, false, &ice));
        assert!(relay(1, true, &ice), "the last connection needed TURN: the first attempt takes it");
        let none = Ice::default();
        assert!(!none.has_turn() && !relay(2, true, &none) && none.stun().is_empty());
        assert!(!relay(2, false, &Ice { user: String::new(), ..ice }), "no credential, no TURN");
        assert!(attempt_ms(false) < attempt_ms(true));
    }

    #[test]
    fn a_device_remembers_for_a_day_which_ones_needed_turn() {
        let kept = [(2u16, 1_000u64), (5, 1_000 + REMEMBER_RELAY_SECS)].into_iter().collect();
        let text = relayed_text(&kept);
        assert_eq!(text, format!("2\t1000\n5\t{}\n", 1_000 + REMEMBER_RELAY_SECS));
        let now = 1_000 + REMEMBER_RELAY_SECS + 10;
        assert_eq!(read_relayed(&text, now).into_iter().collect::<Vec<_>>(), vec![(5, 1_000 + REMEMBER_RELAY_SECS)]);
        assert!(read_relayed("x\ty\n3\n", now).is_empty());
    }

    #[test]
    fn a_device_waits_longer_after_every_failed_attempt() {
        assert_eq!(wait_before(2), 500);
        assert_eq!(wait_before(3), 30_000);
        assert_eq!(wait_before(4), 60_000);
        assert_eq!(wait_before(5), 120_000);
        assert_eq!(wait_before(6), 240_000);
        assert_eq!(wait_before(7), 300_000);
        assert_eq!(wait_before(40), 300_000);
    }

    #[test]
    fn the_status_reads_back() {
        let o = Online { devices: 2, direct: 1, relayed: 1 };
        assert_eq!(read_status(&status_text(Some(o))), Some(o));
        assert_eq!(read_status(&status_text(None)), None);
        assert_eq!(read_status("online 3"), Some(Online { devices: 3, direct: 0, relayed: 0 }));
        assert_eq!(read_status(""), None);
    }

    #[test]
    fn a_message_on_the_channel_reads_back() {
        let m = Direct::Push { data: "AAAA".into() };
        assert_eq!(Direct::read(&m.text()), Some(m));
        assert_eq!(Direct::read(&"x".repeat(4 * MAX_MESSAGE + 1)), None);
        assert_eq!(Direct::read("{\"t\":\"other\"}"), None);
    }
}
