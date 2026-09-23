//! The format as a whole: values there and back, what codes cost, what they refuse.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::bits::Bits;
use crate::{from_code, text, to_code, Error, MAX_LEN};

/// A random number (xorshift64*): the tests run the same every time.
pub(crate) fn next(state: &mut u64) -> u64 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    state.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

fn there_and_back<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(kind: &str, value: &T) -> String {
    let code = to_code(kind, value).unwrap();
    assert!(code.bytes().all(|c| crate::ALPHABET.as_bytes().contains(&c)), "{code}");
    assert_eq!(from_code::<T>(kind, &code).as_ref(), Ok(value), "{code}");
    code
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct Unit;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct Newtype(i16);

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct Pair(u8, String);

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
enum Shape {
    Unit,
    Newtype(u32),
    Tuple(bool, Option<i8>),
    Struct { name: String, sizes: Vec<u16> },
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct Everything {
    flag: bool,
    small: u8,
    medium: u16,
    large: u32,
    largest: u64,
    signed: (i8, i16, i32, i64),
    floats: (f32, f64),
    letters: (char, char),
    strings: Vec<String>,
    #[serde(with = "crate::bytes")]
    blob: Vec<u8>,
    maybe: (Option<u32>, Option<String>),
    nothing: ((), Unit),
    newtype: Newtype,
    pair: Pair,
    array: [u8; 3],
    shapes: Vec<Shape>,
    nested: Vec<Vec<u16>>,
    map: BTreeMap<String, i32>,
    #[serde(with = "crate::set")]
    set: BTreeSet<u32>,
    #[serde(with = "crate::list")]
    list: Vec<u64>,
}

fn everything() -> Everything {
    Everything {
        flag: true,
        small: 200,
        medium: 2026,
        large: 301_512,
        largest: u64::MAX,
        signed: (i8::MIN, -2, i32::MAX, i64::MIN),
        floats: (-0.5, f64::INFINITY),
        letters: ('ü', '😀'),
        strings: ["", "12204", "0012204", "FUES-7", "Analysis I", "Übung 3.", "Grüße 😀"].map(String::from).to_vec(),
        blob: vec![0, 1, 254, 255],
        maybe: (Some(0), None),
        nothing: ((), Unit),
        newtype: Newtype(-300),
        pair: Pair(7, "sieben".into()),
        array: [1, 2, 3],
        shapes: vec![Shape::Unit, Shape::Newtype(42), Shape::Tuple(false, Some(-1)), Shape::Struct { name: "Übung".into(), sizes: vec![20, 25] }],
        nested: vec![vec![], vec![1], vec![2, 3]],
        map: [("a".to_string(), -1), ("b".to_string(), 1)].into_iter().collect(),
        set: [11101, 11102, 12204, 41_000].into_iter().collect(),
        list: vec![12204, 11101, 12205, 0, u64::MAX, 3],
    }
}

#[test]
fn every_shape_serde_knows_goes_there_and_back() {
    there_and_back("", &everything());
    there_and_back("everything", &everything());
    there_and_back("", &());
    there_and_back("", &Unit);
    there_and_back("", &0u8);
    there_and_back("", &String::new());
    there_and_back("", &Some(Some(false)));
    there_and_back("", &Vec::<Shape>::new());
    for shape in everything().shapes {
        there_and_back("", &shape);
    }
    // Integers at the ends of their ranges, and what lies between.
    let mut state = 1;
    for _ in 0..200 {
        let n = next(&mut state) >> (next(&mut state) % 64);
        there_and_back("", &(n, n as i64, n as u32, n as i32, n as u16, n as i16, n as u8, n as i8));
    }
    there_and_back("", &(u64::MAX, i64::MIN, i64::MAX, u32::MAX, i32::MIN, u16::MAX, i16::MIN, u8::MAX, i8::MIN));
    there_and_back("", &(f64::MIN_POSITIVE, f32::MAX, -0.0f64, 1e-300f64));
    // A sequence whose length is not known before it ends (`collect_seq` of a filtered iterator).
    struct Evens(u32);
    impl Serialize for Evens {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.collect_seq((0..self.0).filter(|n| n % 2 == 0))
        }
    }
    let code = to_code("", &(Evens(9), 7u8)).unwrap();
    assert_eq!(from_code::<(Vec<u32>, u8)>("", &code), Ok((vec![0, 2, 4, 6, 8], 7)));
}

#[test]
fn the_format_is_frozen() {
    // Codes that are out there have to keep their meaning: a change here breaks every link made
    // with them. Zero is the empty payload, and `h = 0` checks it: `AA`.
    assert_eq!(to_code("", &()).unwrap(), "AA");
    // The bits 0 (the format) and 1: the number 2, `C`; h = (0 · 66 + 2 + 1) mod 4091 = 3: `AD`.
    assert_eq!(to_code("", &true).unwrap(), "CAD");
    assert_eq!(to_code("", &(false, None::<u8>, 0u64, "", Vec::<u8>::new())).unwrap(), "AA", "zero at the end costs nothing");
    #[derive(Serialize)]
    struct Plan {
        semester: u16,
        #[serde(with = "crate::set")]
        events: Vec<u32>,
    }
    assert_eq!(to_code("plan", &Plan { semester: 2026, events: vec![301_512, 301_517, 301_530, 302_048] }).unwrap(), "2_w9p_YZ_8713UL");
    assert_eq!(to_code("everything", &everything()).unwrap(), EVERYTHING);
}

/// `everything()` as a code of the kind `everything`.
const EVERYTHING: &str = "qkBJa_pVCaIFnmOpkbNsaaGJR0CceY2ByOPZc1MPu6F5v.uVnBX5TjruNsIAbz~sMG7jjRJ_w9Ba_0saFCcqGXOKP3qp~i0s0poUWfGkMiqdrqDucay2dnTG.2SLQ2pFoFO6BViO70wlX3JDKVFwnz5oXKA8-nxH1344eJVHec7R2AyxWxgFs7uoR_k9Z2mvEOR4mJ7iL8Xx4xxBVr";

#[derive(Serialize, Deserialize, PartialEq, Debug, Default)]
struct Before {
    id: u32,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default)]
enum Mode {
    #[default]
    Off,
    On,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Default)]
struct After {
    id: u32,
    name: Option<String>,
    tags: Vec<String>,
    hidden: bool,
    count: u16,
    mode: Mode,
    #[serde(with = "crate::set")]
    events: Vec<u32>,
}

#[test]
fn a_field_added_at_the_end_reads_as_absent_from_older_codes() {
    let before = there_and_back("", &Before { id: 12204 });
    // Absent, the new fields add nothing: the same code, whichever version wrote it.
    assert_eq!(to_code("", &After { id: 12204, ..After::default() }).unwrap(), before);
    assert_eq!(from_code::<After>("", &before), Ok(After { id: 12204, ..After::default() }));
    // A code with a new field set is more than the old version knows.
    let after = there_and_back("", &After { id: 12204, mode: Mode::On, ..After::default() });
    assert_eq!(from_code::<Before>("", &after), Err(Error::Trailing));
    let after = there_and_back("", &After { id: 12204, events: vec![1], ..After::default() });
    assert_eq!(from_code::<Before>("", &after), Err(Error::Trailing));
}

#[test]
fn what_the_format_does_not_do_is_refused() {
    // The bits do not say what they hold, so nothing can be read that asks them.
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    #[serde(untagged)]
    enum Either {
        Number(u32),
        Text(String),
    }
    let code = to_code("", &Either::Number(5)).unwrap();
    assert!(matches!(from_code::<Either>("", &code), Err(Error::Unsupported(_))));
    // Fields are known by their place: one left out would shift the rest.
    #[derive(Serialize)]
    struct Sparse {
        #[serde(skip_serializing_if = "Option::is_none")]
        first: Option<u32>,
        second: u32,
    }
    assert!(matches!(to_code("", &Sparse { first: None, second: 1 }), Err(Error::Unsupported(_))));
    assert!(to_code("", &Sparse { first: Some(0), second: 1 }).is_ok());
    assert!(matches!(to_code("", &1u128), Err(Error::Message(_))));
    // A value read as a type it does not fit.
    let code = to_code("", &300u64).unwrap();
    assert_eq!(from_code::<u16>("", &code), Ok(300));
    assert_eq!(from_code::<u8>("", &code), Err(Error::Malformed));
    assert_eq!(from_code::<i8>("", &to_code("", &-129i64).unwrap()), Err(Error::Malformed));
    assert_eq!(from_code::<char>("", &to_code("", &0xd800u32).unwrap()), Err(Error::Malformed));
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    enum Three {
        A,
        B,
        C,
    }
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    enum Two {
        A,
        B,
    }
    assert_eq!(from_code::<Two>("", &to_code("", &Three::B).unwrap()), Ok(Two::B));
    assert_eq!(from_code::<Two>("", &to_code("", &Three::C).unwrap()), Err(Error::Malformed));
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Wide(#[serde(with = "crate::set")] Vec<u64>);
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Narrow(#[serde(with = "crate::set")] BTreeSet<u8>);
    assert!(matches!(from_code::<Narrow>("", &to_code("", &Wide(vec![1, 300])).unwrap()), Err(Error::Message(_))));
    // Another kind, another version of the format.
    assert_eq!(from_code::<bool>("plan", &to_code("bookmarks", &true).unwrap()), Err(Error::Check));
    let mut bits = Bits::default();
    bits.gamma(1);
    assert_eq!(from_code::<()>("", &text::encode("", &bits).unwrap()), Err(Error::Format));
}

#[test]
fn a_code_that_checks_out_cannot_make_a_reader_do_much() {
    // Whatever bits a code carries, a reader answers, and soon: no panic, no endless loop, no list
    // longer than the bits it would take.
    let mut state = 99;
    for round in 0..3000 {
        let mut bits = Bits::default();
        for _ in 0..(round % 400) {
            bits.push(!next(&mut state).is_multiple_of(3));
        }
        let code = text::encode("", &bits).unwrap();
        let _ = from_code::<Everything>("", &code);
        let _ = from_code::<Vec<Shape>>("", &code);
        let _ = from_code::<(Vec<()>, BTreeMap<u8, Vec<String>>)>("", &code);
        let _ = from_code::<Vec<Vec<Vec<u8>>>>("", &code);
    }
    // A list of a million nothings, in a code of a few characters.
    let mut bits = Bits::default();
    bits.gamma(0);
    bits.delta(1_000_000);
    let code = text::encode("", &bits).unwrap();
    assert!(code.len() < 10);
    assert_eq!(from_code::<Vec<()>>("", &code), Err(Error::Malformed));
    assert_eq!(from_code::<Vec<bool>>("", &code), Err(Error::Malformed));
    assert_eq!(from_code::<String>("", &code), Err(Error::Malformed));
    // As deep as a reader follows, and one level deeper.
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    enum Tree {
        Leaf,
        Node(Box<Tree>),
    }
    let deep = |depth: usize| (0..depth).fold(Tree::Leaf, |tree, _| Tree::Node(Box::new(tree)));
    there_and_back("", &deep(63));
    assert_eq!(from_code::<Tree>("", &to_code("", &deep(64)).unwrap()), Err(Error::Malformed));
}

#[test]
fn sets_and_lists_of_integers() {
    let mut state = 17;
    for round in 0..300u64 {
        let len = (next(&mut state) % 200) as usize;
        let spread = 1u64 << (next(&mut state) % 64);
        let base = if round % 5 == 0 { u64::MAX - spread } else { next(&mut state) % 100_000 };
        let values: Vec<u64> = (0..len).map(|_| base.wrapping_add(next(&mut state) % spread)).collect();
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Both {
            #[serde(with = "crate::set")]
            set: BTreeSet<u64>,
            #[serde(with = "crate::list")]
            list: Vec<u64>,
        }
        there_and_back("", &Both { set: values.iter().copied().collect(), list: values });
    }
    // A set takes any order and anything twice, and gives it back once, in ascending order.
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Set(#[serde(with = "crate::set")] Vec<u32>);
    let code = to_code("", &Set(vec![30, 10, 20, 10])).unwrap();
    assert_eq!(from_code::<Set>("", &code), Ok(Set(vec![10, 20, 30])));
    // Close together is cheaper than far apart; a list keeps its order at a price.
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct List(#[serde(with = "crate::list")] Vec<u32>);
    let close = to_code("", &Set((12_000..12_050).collect())).unwrap();
    let apart = to_code("", &Set((0..50).map(|n| n * 90_000).collect())).unwrap();
    assert!(close.len() * 5 < apart.len(), "{close} {apart}");
    assert_eq!(there_and_back("", &List(vec![12204, 11101, 12205])).len(), 12);
    // Any other format sees plain integers.
    let json = serde_json::to_string(&(Set(vec![3, 1]), List(vec![3, 1]))).unwrap();
    assert_eq!(json, "[[1,3],[3,1]]");
    assert_eq!(serde_json::from_str::<(Set, List)>(&json).unwrap(), (Set(vec![1, 3]), List(vec![3, 1])));
}

/// Module numbers as somebody marks them: a few departments, several from one in a row.
fn marked(state: &mut u64, n: usize) -> Vec<u64> {
    let departments = [11_000u64, 12_000, 13_000, 41_000];
    let mut ids: Vec<u64> = Vec::new();
    while ids.len() < n {
        let department = departments[(next(state) % 4) as usize];
        for _ in 0..1 + next(state) % 6 {
            let id = department + next(state) % 900;
            if !ids.contains(&id) && ids.len() < n {
                ids.push(id);
            }
        }
    }
    ids
}

#[test]
fn module_numbers_take_two_or_three_characters_each() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct List(#[serde(with = "crate::list")] Vec<u64>);
    let mut state = 3;
    for (n, most) in [(1, 7), (5, 20), (20, 55), (100, 240), (2000, 4500)] {
        let ids = marked(&mut state, n);
        let code = there_and_back("bookmarks", &List(ids.clone()));
        let text = ids.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
        assert!(code.len() <= most && (n == 1 || code.len() * 5 < text.len() * 3), "{n} ids: {} characters, {} as text", code.len(), text.len());
    }
}

#[test]
fn bytes_take_eight_bits_each() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Blob(#[serde(with = "crate::bytes")] Vec<u8>);
    let mut state = 23;
    let blob: Vec<u8> = (0..600).map(|_| next(&mut state) as u8).collect();
    let code = there_and_back("", &Blob(blob.clone()));
    // 600 bytes are 4800 bits, 795 characters and a few for the length and the check.
    assert!((795..=800).contains(&code.len()), "{}", code.len());
    // As a plain Vec<u8>, a byte takes the bits its value needs: 14 for 200, and a zero at the end
    // nothing.
    assert!(there_and_back("", &vec![200u8; 600]).len() > 1300);
    assert_eq!(there_and_back("", &vec![0u8; 600]).len(), 5);
    // The longest code there is, there and back.
    let longest: Vec<u8> = (0..(MAX_LEN - 10) * 6 / 8).map(|_| next(&mut state) as u8).collect();
    let code = there_and_back("", &Blob(longest));
    assert!(code.len() <= MAX_LEN);
    assert_eq!(to_code("", &Blob(vec![1; MAX_LEN * 7 / 8])), Err(Error::TooLong));
}
