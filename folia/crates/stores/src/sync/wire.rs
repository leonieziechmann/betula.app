//! The frames of Phloem's WebSocket and the bodies of its HTTP calls (docs/phloem/protocol.md §3,
//! §4): JSON, every value of bytes in base64url without padding. A frame of a type this build
//! does not know reads as `In::Other` and is left alone.

use serde::{Deserialize, Serialize};

/// What a device sends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Out {
    /// `slot`: the one this device had, for a space Phloem made anew after it lost it (0: none).
    Hello {
        space: String,
        token: String,
        device: String,
        since: u64,
        #[serde(skip_serializing_if = "is_zero")]
        slot: u16,
    },
    Push { id: String, data: String },
    Snapshot { upto: u64, data: String },
    Leave,
    Delete,
    /// The parts of shared calendars this device follows (step 3).
    Watch { parts: Vec<String> },
    /// A sealed signal for WebRTC (`live::Talk`) for the device of slot `to` (step 2).
    Signal { to: u16, data: String },
    /// The ICE servers and this socket's TURN credential, please (step 2).
    Turn,
}

impl Out {
    pub fn text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

fn is_zero(n: &u16) -> bool {
    *n == 0
}

/// A snapshot as the welcome carries it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Snapshot {
    pub upto: u64,
    pub data: String,
}

/// An entry: one change of a device, sealed, with its number.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Entry {
    pub nr: u64,
    #[serde(default)]
    pub slot: u16,
    pub data: String,
}

/// What Phloem sends.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum In {
    Welcome {
        slot: u16,
        #[serde(default)]
        head: u64,
        #[serde(default)]
        now: u64,
        #[serde(default)]
        devices: Vec<u16>,
        /// This Phloem takes `signal` and `turn` (step 2); an older one closes a socket on them.
        #[serde(default)]
        live: bool,
        #[serde(default)]
        snapshot: Option<Snapshot>,
        #[serde(default)]
        entries: Vec<Entry>,
        /// The epoch of Phloem's database (since Phloem 0.5.0; empty before).
        #[serde(default)]
        epoch: String,
    },
    Stored {
        id: String,
        nr: u64,
    },
    Entry {
        nr: u64,
        #[serde(default)]
        slot: u16,
        data: String,
    },
    Snapshotted {
        upto: u64,
    },
    Presence {
        #[serde(default)]
        devices: Vec<u16>,
    },
    Compact,
    Error {
        code: String,
        #[serde(default)]
        id: Option<String>,
        /// Of `unauthorized`: the epoch of Phloem's database (since Phloem 0.5.0).
        #[serde(default)]
        epoch: String,
    },
    Gone,
    /// The answer to `watch`: each part's version now (`rev` 0: Phloem does not have it).
    Watching {
        #[serde(default)]
        parts: Vec<PartState>,
    },
    /// A part watched was replaced (`rev`), or deleted (`gone`).
    Part {
        id: String,
        #[serde(default)]
        rev: u64,
        #[serde(default)]
        gone: bool,
    },
    /// A parcel of the space was opened.
    Pair {
        id: String,
        #[serde(default)]
        state: String,
    },
    /// Another device's sealed signal for WebRTC, from slot `from` (step 2).
    Signal {
        from: u16,
        data: String,
    },
    /// The answer to `turn`: the ICE servers, none where the site has no TURN, and the socket's
    /// credential.
    Turn {
        #[serde(default)]
        urls: Vec<String>,
        #[serde(default)]
        user: String,
        #[serde(default)]
        password: String,
    },
    #[serde(other)]
    Other,
}

/// A part's version, as `watching` names it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct PartState {
    pub id: String,
    #[serde(default)]
    pub rev: u64,
    #[serde(default)]
    pub gone: bool,
}

impl In {
    /// A frame as it came; `None` for what is no frame at all.
    pub fn read(text: &str) -> Option<In> {
        serde_json::from_str(text).ok()
    }
}

/// `POST /sync/v1/space`.
#[derive(Clone, Debug, Serialize)]
pub struct NewSpace {
    pub space: String,
    pub token: String,
    pub device: String,
    /// The slot the device had, for a space made anew after Phloem lost it (0: a new profile).
    #[serde(skip_serializing_if = "is_zero")]
    pub slot: u16,
}

/// Its answer.
#[derive(Clone, Debug, Deserialize)]
pub struct Made {
    pub slot: u16,
}

/// `POST /sync/v1/pair`.
#[derive(Clone, Debug, Serialize)]
pub struct Parcel {
    pub id: String,
    #[serde(rename = "box")]
    pub sealed: String,
}

/// `POST /sync/v1/pair` of a one-time link: it waits `ttl` seconds.
#[derive(Clone, Debug, Serialize)]
pub struct LongParcel {
    pub id: String,
    #[serde(rename = "box")]
    pub sealed: String,
    pub ttl: u64,
}

/// `GET /sync/v1/pairs`: the space's parcels and what became of them.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Parcels {
    #[serde(default)]
    pub parcels: Vec<ParcelState>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ParcelState {
    pub id: String,
    /// `waiting`, `taken`, `expired`, `withdrawn`.
    pub state: String,
    /// When a waiting one expires, else when it happened (ms).
    #[serde(default)]
    pub at: u64,
}

/// `PUT /sync/v1/part`.
#[derive(Clone, Debug, Serialize)]
pub struct PartBody {
    pub data: String,
}

/// The answer of `PUT /sync/v1/part`.
#[derive(Clone, Debug, Deserialize)]
pub struct PartMade {
    pub rev: u64,
}

/// The answer of `GET /sync/v1/part`.
#[derive(Clone, Debug, Deserialize)]
pub struct PartFetched {
    pub data: String,
    #[serde(default)]
    pub rev: u64,
}

/// What a one-time link's parcel holds, sealed with the link's key (associated data `"share"`):
/// the part to follow and its key, or a copy of now (`folia_calendar::live::LivePart`'s code).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareContents {
    pub v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
}

/// The answer of `GET /sync/v1/pair`.
#[derive(Clone, Debug, Deserialize)]
pub struct Fetched {
    #[serde(rename = "box")]
    pub sealed: String,
}

/// An error's body.
#[derive(Clone, Debug, Deserialize)]
pub struct Failure {
    #[serde(default)]
    pub error: String,
}

/// What a pairing parcel holds, sealed with the link's key: the profiles that come along.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contents {
    pub v: u32,
    pub profiles: Vec<Shared>,
}

/// One profile of a parcel: its key and its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shared {
    pub key: String,
    #[serde(default)]
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_read_and_write_as_the_protocol_says() {
        let hello = Out::Hello { space: "c3BhY2U".into(), token: "dG9rZW4".into(), device: "ZGV2".into(), since: 7, slot: 0 };
        assert_eq!(hello.text(), r#"{"t":"hello","space":"c3BhY2U","token":"dG9rZW4","device":"ZGV2","since":7}"#);
        let again = Out::Hello { space: "c3BhY2U".into(), token: "dG9rZW4".into(), device: "ZGV2".into(), since: 7, slot: 3 };
        assert_eq!(again.text(), r#"{"t":"hello","space":"c3BhY2U","token":"dG9rZW4","device":"ZGV2","since":7,"slot":3}"#, "the slot it had, for a space made anew");
        assert_eq!(serde_json::to_string(&NewSpace { space: "cw".into(), token: "dA".into(), device: "ZA".into(), slot: 0 }).unwrap(), r#"{"space":"cw","token":"dA","device":"ZA"}"#);
        assert_eq!(serde_json::to_string(&NewSpace { space: "cw".into(), token: "dA".into(), device: "ZA".into(), slot: 2 }).unwrap(), r#"{"space":"cw","token":"dA","device":"ZA","slot":2}"#);
        assert_eq!(Out::Leave.text(), r#"{"t":"leave"}"#);
        let welcome = In::read(r#"{"t":"welcome","slot":2,"head":9,"now":1790000000000,"devices":[1,2],"snapshot":{"upto":4,"data":"AA"},"entries":[{"nr":5,"slot":1,"data":"AQ"}]}"#);
        assert_eq!(
            welcome,
            Some(In::Welcome { slot: 2, head: 9, now: 1_790_000_000_000, devices: vec![1, 2], live: false, snapshot: Some(Snapshot { upto: 4, data: "AA".into() }), entries: vec![Entry { nr: 5, slot: 1, data: "AQ".into() }], epoch: String::new() })
        );
        assert_eq!(In::read(r#"{"t":"welcome","slot":1,"epoch":"ZXBvY2g"}"#).map(|w| matches!(w, In::Welcome { epoch, .. } if epoch == "ZXBvY2g")), Some(true));
        assert_eq!(In::read(r#"{"t":"error","code":"unauthorized","epoch":"ZXBvY2g"}"#), Some(In::Error { code: "unauthorized".into(), id: None, epoch: "ZXBvY2g".into() }));
        assert_eq!(In::read(r#"{"t":"welcome","slot":1,"snapshot":null}"#).map(|w| matches!(w, In::Welcome { snapshot: None, .. })), Some(true));
        assert_eq!(In::read(r#"{"t":"error","code":"too_big","id":"x"}"#), Some(In::Error { code: "too_big".into(), id: Some("x".into()), epoch: String::new() }));
        assert_eq!(In::read(r#"{"t":"later","from":3}"#), Some(In::Other), "a frame of a later step is left alone");
        assert_eq!(In::read("no json"), None);
        let contents = Contents { v: 1, profiles: vec![Shared { key: "a2V5".into(), name: "Informatik".into() }] };
        let text = serde_json::to_string(&contents).unwrap();
        assert_eq!(serde_json::from_str::<Contents>(&text).unwrap(), contents);
        assert_eq!(serde_json::to_string(&Parcel { id: "aWQ".into(), sealed: "Ym94".into() }).unwrap(), r#"{"id":"aWQ","box":"Ym94"}"#);
    }

    #[test]
    fn the_frames_of_shares_read_and_write_as_the_protocol_says() {
        assert_eq!(Out::Watch { parts: vec!["cA".into(), "cQ".into()] }.text(), r#"{"t":"watch","parts":["cA","cQ"]}"#);
        assert_eq!(
            In::read(r#"{"t":"watching","parts":[{"id":"cA","rev":3},{"id":"cQ","gone":true},{"id":"cg","rev":0}]}"#),
            Some(In::Watching { parts: vec![PartState { id: "cA".into(), rev: 3, gone: false }, PartState { id: "cQ".into(), rev: 0, gone: true }, PartState { id: "cg".into(), rev: 0, gone: false }] })
        );
        assert_eq!(In::read(r#"{"t":"part","id":"cA","rev":4}"#), Some(In::Part { id: "cA".into(), rev: 4, gone: false }));
        assert_eq!(In::read(r#"{"t":"part","id":"cA","gone":true}"#), Some(In::Part { id: "cA".into(), rev: 0, gone: true }));
        assert_eq!(In::read(r#"{"t":"pair","id":"cA","state":"taken"}"#), Some(In::Pair { id: "cA".into(), state: "taken".into() }));
        assert_eq!(serde_json::to_string(&LongParcel { id: "aWQ".into(), sealed: "Ym94".into(), ttl: 604_800 }).unwrap(), r#"{"id":"aWQ","box":"Ym94","ttl":604800}"#);
        let live = ShareContents { v: 1, part: Some("cA".into()), key: Some("a2V5".into()), copy: None };
        assert_eq!(serde_json::to_string(&live).unwrap(), r#"{"v":1,"part":"cA","key":"a2V5"}"#);
        assert_eq!(serde_json::from_str::<ShareContents>(r#"{"v":1,"copy":"abc"}"#).unwrap().copy.as_deref(), Some("abc"));
        let parcels: Parcels = serde_json::from_str(r#"{"parcels":[{"id":"cA","state":"waiting","at":5},{"id":"cQ","state":"taken","at":6}]}"#).unwrap();
        assert_eq!(parcels.parcels.len(), 2);
    }

    #[test]
    fn the_frames_of_the_live_way_read_and_write_as_the_protocol_says() {
        assert_eq!(Out::Signal { to: 2, data: "c2ln".into() }.text(), r#"{"t":"signal","to":2,"data":"c2ln"}"#);
        assert_eq!(Out::Turn.text(), r#"{"t":"turn"}"#);
        assert_eq!(In::read(r#"{"t":"signal","from":3,"data":"c2ln"}"#), Some(In::Signal { from: 3, data: "c2ln".into() }));
        assert_eq!(
            In::read(r#"{"t":"turn","urls":["stun:betula.app:3478","turn:betula.app:3478?transport=udp"],"user":"dQ","password":"cA"}"#),
            Some(In::Turn { urls: vec!["stun:betula.app:3478".into(), "turn:betula.app:3478?transport=udp".into()], user: "dQ".into(), password: "cA".into() })
        );
        assert_eq!(In::read(r#"{"t":"turn","urls":[]}"#), Some(In::Turn { urls: vec![], user: String::new(), password: String::new() }));
        let live = |text: &str| matches!(In::read(text), Some(In::Welcome { live: true, .. }));
        assert!(live(r#"{"t":"welcome","slot":1,"live":true}"#), "a Phloem of step 2 says so");
        assert!(!live(r#"{"t":"welcome","slot":1}"#), "an older one says nothing: no signal, no turn to it");
    }
}
