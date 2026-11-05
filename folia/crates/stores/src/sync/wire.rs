//! The frames of Phloem's WebSocket and the bodies of its HTTP calls (docs/phloem/protocol.md §3,
//! §4): JSON, every value of bytes in base64url without padding. A frame of a type this build
//! does not know reads as `In::Other` and is left alone.

use serde::{Deserialize, Serialize};

/// What a device sends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Out {
    Hello { space: String, token: String, device: String, since: u64 },
    Push { id: String, data: String },
    Snapshot { upto: u64, data: String },
    Leave,
    Delete,
}

impl Out {
    pub fn text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
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
        #[serde(default)]
        snapshot: Option<Snapshot>,
        #[serde(default)]
        entries: Vec<Entry>,
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
    },
    Gone,
    #[serde(other)]
    Other,
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
        let hello = Out::Hello { space: "c3BhY2U".into(), token: "dG9rZW4".into(), device: "ZGV2".into(), since: 7 };
        assert_eq!(hello.text(), r#"{"t":"hello","space":"c3BhY2U","token":"dG9rZW4","device":"ZGV2","since":7}"#);
        assert_eq!(Out::Leave.text(), r#"{"t":"leave"}"#);
        let welcome = In::read(r#"{"t":"welcome","slot":2,"head":9,"now":1790000000000,"devices":[1,2],"snapshot":{"upto":4,"data":"AA"},"entries":[{"nr":5,"slot":1,"data":"AQ"}]}"#);
        assert_eq!(
            welcome,
            Some(In::Welcome { slot: 2, head: 9, now: 1_790_000_000_000, devices: vec![1, 2], snapshot: Some(Snapshot { upto: 4, data: "AA".into() }), entries: vec![Entry { nr: 5, slot: 1, data: "AQ".into() }] })
        );
        assert_eq!(In::read(r#"{"t":"welcome","slot":1,"snapshot":null}"#).map(|w| matches!(w, In::Welcome { snapshot: None, .. })), Some(true));
        assert_eq!(In::read(r#"{"t":"error","code":"too_big","id":"x"}"#), Some(In::Error { code: "too_big".into(), id: Some("x".into()) }));
        assert_eq!(In::read(r#"{"t":"signal","from":3}"#), Some(In::Other), "a frame of a later step is left alone");
        assert_eq!(In::read("no json"), None);
        let contents = Contents { v: 1, profiles: vec![Shared { key: "a2V5".into(), name: "Informatik".into() }] };
        let text = serde_json::to_string(&contents).unwrap();
        assert_eq!(serde_json::from_str::<Contents>(&text).unwrap(), contents);
        assert_eq!(serde_json::to_string(&Parcel { id: "aWQ".into(), sealed: "Ym94".into() }).unwrap(), r#"{"id":"aWQ","box":"Ym94"}"#);
    }
}
