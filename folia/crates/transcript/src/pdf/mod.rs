//! What of a PDF file a Notenübersicht needs (ISO 32000-1): its objects, also those packed into
//! object streams, its streams decrypted and inflated, its pages, and on each page the text with
//! where it stands and the horizontal lines (`content`). Read from bytes that may be anything:
//! nothing panics, and every structure has a limit.
//!
//! The objects are found by scanning the file for `n g obj`, not through its cross-reference
//! table: the table is a shortcut that a file in good order and a damaged one need not have alike,
//! and a scan finds what the table points to. Where a number is written twice (an update appended
//! to the file), the later one counts.

pub mod content;
pub mod crypt;
pub mod font;
pub mod inflate;
pub mod syntax;

use std::collections::{BTreeMap, BTreeSet};

use syntax::{find, is_regular, is_space, Dict, Item, Object, Parser};

/// The most pages read; a Notenübersicht has a few.
const MAX_PAGES: usize = 50;

/// The most objects one object stream may hold.
const MAX_PACKED: usize = 100_000;

/// Why a file cannot be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Not a PDF file, or none whose pages can be found.
    NotPdf,
    /// It opens only with a password.
    Password,
    /// Its encryption is one this reader does not know.
    Encryption,
}

/// Where an object is written.
#[derive(Clone, Copy, Debug)]
enum Place {
    /// In the file, just after its `obj`.
    File(usize),
    /// In an object stream: which of the document's, and where in its data.
    Packed(usize, usize),
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    place: Place,
    gen: u16,
    /// Where in the file it was written: the later of two counts.
    at: usize,
}

/// A PDF file, opened.
pub struct Document<'a> {
    data: &'a [u8],
    objects: BTreeMap<u32, Entry>,
    packs: Vec<Vec<u8>>,
    trailer: Dict,
    key: Option<crypt::Key>,
}

/// A page: its dictionary and its resources (its own or those it inherits).
pub struct PageRef {
    pub dict: Dict,
    pub resources: Dict,
}

impl<'a> Document<'a> {
    pub fn open(data: &'a [u8]) -> Result<Document<'a>, Refusal> {
        if find(data.get(..1024).unwrap_or(data), b"%PDF-", 0).is_none() {
            return Err(Refusal::NotPdf);
        }
        let (objects, trailers) = scan(data);
        let mut trailer = Dict::default();
        for dict in trailers {
            for (key, value) in dict.0 {
                trailer.0.push((key, value));
            }
        }
        let mut document = Document { data, objects, packs: Vec::new(), trailer, key: None };
        if let Some(encrypt) = document.trailer.get(b"Encrypt").cloned() {
            let encrypt = document.resolve(&encrypt);
            let encrypt = encrypt.dict().ok_or(Refusal::Encryption)?;
            let id = document.trailer.get(b"ID").and_then(Object::array).and_then(|ids| ids.first()).and_then(Object::string).unwrap_or_default().to_vec();
            document.key = Some(crypt::Key::open(encrypt, &id).map_err(|refusal| match refusal {
                crypt::Refusal::Password => Refusal::Password,
                crypt::Refusal::Unsupported => Refusal::Encryption,
            })?);
        }
        document.unpack();
        Ok(document)
    }

    /// The objects of the file's object streams, where no later definition of their number stands.
    fn unpack(&mut self) {
        let streams: Vec<(u32, usize)> = self.objects.iter().filter(|(_, entry)| matches!(entry.place, Place::File(_))).map(|(num, entry)| (*num, entry.at)).collect();
        for (num, at) in streams {
            // Only an object stream is decoded here: not the fonts' files and pictures.
            if !self.get(num).and_then(|object| object.dict().map(|dict| dict.is(b"Type", b"ObjStm"))).unwrap_or(false) {
                continue;
            }
            let Some((dict, data)) = self.stream(num) else { continue };
            let count = dict.get(b"N").and_then(Object::int).and_then(|n| usize::try_from(n).ok()).unwrap_or(0).min(MAX_PACKED);
            let first = dict.get(b"First").and_then(Object::int).and_then(|n| usize::try_from(n).ok()).unwrap_or(0);
            let index = self.packs.len();
            let mut parser = Parser::new(&data, 0, false);
            let mut found = Vec::new();
            for _ in 0..count {
                let (Some(Item::Object(Object::Int(n))), Some(Item::Object(Object::Int(offset)))) = (parser.item(), parser.item()) else { break };
                let (Ok(n), Ok(offset)) = (u32::try_from(n), usize::try_from(offset)) else { break };
                found.push((n, first.saturating_add(offset)));
            }
            for (n, offset) in found {
                let later = self.objects.get(&n).is_some_and(|entry| entry.at > at);
                if !later {
                    self.objects.insert(n, Entry { place: Place::Packed(index, offset), gen: 0, at });
                }
            }
            self.packs.push(data);
        }
    }

    /// Object `num`; a stream's dictionary for a stream.
    pub fn get(&self, num: u32) -> Option<Object> {
        let entry = self.objects.get(&num)?;
        match entry.place {
            Place::File(at) => match Parser::new(self.data, at, true).item()? {
                Item::Object(object) => Some(object),
                Item::Keyword(_) => None,
            },
            Place::Packed(index, at) => match Parser::new(self.packs.get(index)?, at, true).item()? {
                Item::Object(object) => Some(object),
                Item::Keyword(_) => None,
            },
        }
    }

    /// `object`, a reference followed (a few steps at most, a cycle ends in null).
    pub fn resolve(&self, object: &Object) -> Object {
        let mut now = object.clone();
        for _ in 0..8 {
            match now {
                Object::Ref(num, _) => now = self.get(num).unwrap_or(Object::Null),
                other => return other,
            }
        }
        Object::Null
    }

    /// The entry `key` of `dict`, its reference followed.
    pub fn entry(&self, dict: &Dict, key: &[u8]) -> Object {
        dict.get(key).map(|value| self.resolve(value)).unwrap_or(Object::Null)
    }

    /// Stream `num`: its dictionary and its data, decrypted and decoded. `None` for an object that
    /// is no stream and for data of a filter not read here.
    pub fn stream(&self, num: u32) -> Option<(Dict, Vec<u8>)> {
        let entry = self.objects.get(&num)?;
        let Place::File(at) = entry.place else { return None };
        let mut parser = Parser::new(self.data, at, true);
        let Some(Item::Object(Object::Dict(dict))) = parser.item() else { return None };
        if !parser.keyword_is(b"stream") {
            return None;
        }
        let length = self.resolve(dict.get(b"Length").unwrap_or(&Object::Null)).int().and_then(|n| usize::try_from(n).ok());
        let raw = self.data.get(stream_range(self.data, parser.pos(), length))?;
        // A cross-reference stream is never encrypted (§7.6.1).
        let data = match &self.key {
            Some(key) if !dict.is(b"Type", b"XRef") => key.decrypt(num, entry.gen, raw),
            _ => raw.to_vec(),
        };
        let data = decode(&dict, data)?;
        Some((dict, data))
    }

    /// The data of a stream a value refers to.
    pub fn stream_of(&self, object: &Object) -> Option<(Dict, Vec<u8>)> {
        match object {
            Object::Ref(num, _) => self.stream(*num),
            _ => None,
        }
    }

    /// The pages, in order, each with its resources (inherited from its parents where it has
    /// none of its own).
    pub fn pages(&self) -> Vec<PageRef> {
        let root = self.entry(&self.trailer, b"Root");
        let Some(root) = root.dict() else { return Vec::new() };
        let mut pages = Vec::new();
        let mut seen = BTreeSet::new();
        // (node, resources inherited); a stack instead of recursion.
        let mut stack: Vec<(Object, Option<Dict>)> = vec![(root.get(b"Pages").cloned().unwrap_or(Object::Null), None)];
        while let Some((node, inherited)) = stack.pop() {
            if pages.len() >= MAX_PAGES || seen.len() > 10 * MAX_PAGES {
                break;
            }
            if let Object::Ref(num, _) = node {
                if !seen.insert(num) {
                    continue;
                }
            }
            let node = self.resolve(&node);
            let Some(dict) = node.dict() else { continue };
            let resources = self.entry(dict, b"Resources").dict().cloned().or(inherited);
            match dict.get(b"Kids").map(|kids| self.resolve(kids)) {
                Some(Object::Array(kids)) => {
                    // Reversed onto the stack, so the first comes first.
                    for kid in kids.into_iter().rev() {
                        stack.push((kid, resources.clone()));
                    }
                }
                _ => pages.push(PageRef { dict: dict.clone(), resources: resources.unwrap_or_default() }),
            }
        }
        pages
    }

    /// The content of a page: its streams one after another.
    pub fn contents(&self, page: &PageRef) -> Vec<u8> {
        let streams: Vec<Object> = match page.dict.get(b"Contents") {
            Some(Object::Array(parts)) => parts.clone(),
            Some(reference @ Object::Ref(..)) => match self.resolve(reference) {
                Object::Array(parts) => parts,
                _ => vec![reference.clone()],
            },
            _ => Vec::new(),
        };
        let mut out = Vec::new();
        for part in streams {
            if let Some((_, data)) = self.stream_of(&part) {
                out.extend_from_slice(&data);
                out.push(b'\n');
            }
        }
        out
    }
}

/// Where the data of a stream lies, its keyword `stream` ending at `after`: by its `/Length`
/// where `endstream` follows it, else up to the next `endstream` (a length that is a reference not
/// yet known, or wrong).
fn stream_range(data: &[u8], after: usize, length: Option<usize>) -> std::ops::Range<usize> {
    let mut start = after;
    if data.get(start) == Some(&b'\r') {
        start += 1;
    }
    if data.get(start) == Some(&b'\n') {
        start += 1;
    }
    if let Some(length) = length {
        let end = start.saturating_add(length);
        let mut at = end;
        while data.get(at).is_some_and(|b| is_space(*b)) {
            at += 1;
        }
        if data.get(at..).is_some_and(|rest| rest.starts_with(b"endstream")) {
            return start..end;
        }
    }
    let Some(mut end) = find(data, b"endstream", start) else { return start..data.len() };
    if end > start && data.get(end - 1) == Some(&b'\n') {
        end -= 1;
    }
    if end > start && data.get(end - 1) == Some(&b'\r') {
        end -= 1;
    }
    start..end
}

/// The data of a stream through its filters: FlateDecode alone (and none), without predictors.
fn decode(dict: &Dict, data: Vec<u8>) -> Option<Vec<u8>> {
    let filters: Vec<&[u8]> = match dict.get(b"Filter") {
        None | Some(Object::Null) => Vec::new(),
        Some(Object::Name(name)) => vec![name.as_slice()],
        Some(Object::Array(names)) => names.iter().filter_map(Object::name).collect(),
        Some(_) => return None,
    };
    let predictor = |parms: Option<&Object>| parms.and_then(Object::dict).and_then(|parms| parms.get(b"Predictor")).and_then(Object::int).unwrap_or(1);
    let mut data = data;
    for (index, filter) in filters.iter().enumerate() {
        let parms = match dict.get(b"DecodeParms") {
            Some(Object::Array(all)) => all.get(index),
            other => other,
        };
        if predictor(parms) > 1 {
            return None;
        }
        data = match *filter {
            b"FlateDecode" | b"Fl" => inflate::zlib(&data)?,
            _ => return None,
        };
    }
    Some(data)
}

/// The objects written in the file (`n g obj`), by number, and the trailers: the dictionaries
/// after `trailer` and those of cross-reference streams, in the order of the file.
fn scan(data: &[u8]) -> (BTreeMap<u32, Entry>, Vec<Dict>) {
    let mut objects = BTreeMap::new();
    let mut trailers: Vec<(usize, Dict)> = Vec::new();
    let mut pos = 0;
    while let Some(found) = find(data, b"obj", pos) {
        pos = found + 3;
        if data.get(found + 3).is_some_and(|b| is_regular(*b)) {
            continue;
        }
        let Some((num, gen)) = header_before(data, found) else { continue };
        objects.insert(num, Entry { place: Place::File(found + 3), gen, at: found });
        // Past the object's stream, so that its data is not scanned.
        let mut parser = Parser::new(data, found + 3, true);
        if let Some(Item::Object(Object::Dict(dict))) = parser.item() {
            if parser.keyword_is(b"stream") {
                let length = dict.get(b"Length").and_then(Object::int).and_then(|n| usize::try_from(n).ok());
                pos = pos.max(stream_range(data, parser.pos(), length).end);
            }
            if dict.is(b"Type", b"XRef") {
                trailers.push((found, dict));
            }
        }
    }
    let mut pos = 0;
    while let Some(found) = find(data, b"trailer", pos) {
        pos = found + 7;
        if let Some(Item::Object(Object::Dict(dict))) = Parser::new(data, found + 7, true).item() {
            trailers.push((found, dict));
        }
    }
    trailers.sort_by_key(|(at, _)| *at);
    (objects, trailers.into_iter().map(|(_, dict)| dict).collect())
}

/// `num gen` written before the `obj` at `at`, the number standing alone.
fn header_before(data: &[u8], at: usize) -> Option<(u32, u16)> {
    let mut end = at;
    let space = |i: usize| data.get(i).is_some_and(|b| is_space(*b));
    let digit = |i: usize| data.get(i).is_some_and(u8::is_ascii_digit);
    // Whitespace, the generation, whitespace, the number.
    let mut i = end.checked_sub(1)?;
    if !space(i) {
        return None;
    }
    while space(i) {
        i = i.checked_sub(1)?;
    }
    end = i + 1;
    while digit(i) {
        i = match i.checked_sub(1) {
            Some(before) => before,
            None => break,
        };
    }
    let gen_start = if digit(i) { i } else { i + 1 };
    let gen = std::str::from_utf8(data.get(gen_start..end)?).ok()?.parse::<u16>().ok()?;
    let mut i = gen_start.checked_sub(1)?;
    if !space(i) {
        return None;
    }
    while space(i) {
        i = i.checked_sub(1)?;
    }
    let num_end = i + 1;
    while digit(i) {
        i = match i.checked_sub(1) {
            Some(before) => before,
            None => break,
        };
    }
    let num_start = if digit(i) { i } else { i + 1 };
    if num_start > 0 && data.get(num_start - 1).is_some_and(|b| is_regular(*b)) {
        return None;
    }
    let num = std::str::from_utf8(data.get(num_start..num_end)?).ok()?.parse::<u32>().ok()?;
    Some((num, gen))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &[u8] = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 /Resources << /Font << >> >> >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R /Contents 4 0 R >>\nendobj\n4 0 obj\n<< /Length 5 0 R >>\nstream\nBT /F1 10 Tf 0 0 Td (Hi) Tj ET\nendstream\nendobj\n5 0 obj\n30\nendobj\ntrailer\n<< /Root 1 0 R /Size 6 >>\n%%EOF\n";

    #[test]
    fn objects_by_a_scan_and_a_length_by_reference() {
        let document = Document::open(SMALL).unwrap();
        let pages = document.pages();
        assert_eq!(pages.len(), 1);
        assert!(pages[0].resources.get(b"Font").is_some(), "inherited");
        assert_eq!(document.contents(&pages[0]), b"BT /F1 10 Tf 0 0 Td (Hi) Tj ET\n");
    }

    #[test]
    fn the_number_before_obj() {
        assert_eq!(header_before(b"12 0 obj", 5), Some((12, 0)));
        assert_eq!(header_before(b"\n7  3\r\nobj", 7), Some((7, 3)));
        assert_eq!(header_before(b"x12 0 obj", 6), None);
        assert_eq!(header_before(b"endobj", 3), None);
        assert_eq!(header_before(b"0 obj", 2), None);
    }

    #[test]
    fn not_a_pdf() {
        assert!(matches!(Document::open(b"hello"), Err(Refusal::NotPdf)));
        assert!(Document::open(b"%PDF-1.7\n").map(|document| document.pages().is_empty()).unwrap_or(false));
    }
}
