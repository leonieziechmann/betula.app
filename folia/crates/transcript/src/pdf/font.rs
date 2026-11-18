//! A font as the text of a page needs it (ISO 32000-1 §9.5–9.10): how its codes are cut out of a
//! string (one byte, or two for a composite font of `Identity-H`), the text each code stands for
//! (its `ToUnicode` map, else the encoding of a simple font), how wide each is, and whether it is
//! bold or italic — the Notenübersicht sets its headings in bold and every title's other language
//! in italic.

use std::collections::BTreeMap;

use super::syntax::{Dict, Item, Object, Parser};
use super::Document;

/// The most codes a `bfrange` may span.
const MAX_RANGE: u32 = 0x1_0000;

/// The most entries of a `ToUnicode` map.
const MAX_ENTRIES: usize = 200_000;

#[derive(Clone, Debug, Default)]
pub struct Font {
    pub bold: bool,
    pub italic: bool,
    /// Codes of two bytes (a composite font).
    pub wide: bool,
    /// The text of each code, by its `ToUnicode` map.
    unicode: BTreeMap<u32, String>,
    /// For a simple font without one: the text of each byte.
    simple: Option<Vec<char>>,
    /// Widths in thousandths of the size: of a simple font from `/FirstChar`, of a composite one
    /// by code (`/W`).
    widths: BTreeMap<u32, f64>,
    default_width: f64,
}

impl Font {
    pub fn load(document: &Document, dict: &Dict) -> Font {
        let base = document.entry(dict, b"BaseFont");
        let name = base.name().unwrap_or_default();
        // Without the subset's prefix (`EAAAAB+ArialNarrow-Italic`).
        let name = name.iter().position(|b| *b == b'+').and_then(|at| name.get(at + 1..)).unwrap_or(name);
        let name = String::from_utf8_lossy(name).to_ascii_lowercase();
        let wide = document.entry(dict, b"Subtype").name() == Some(b"Type0");
        let descendant = if wide { document.entry(dict, b"DescendantFonts").array().and_then(|fonts| fonts.first()).map(|font| document.resolve(font)) } else { None };
        let descendant = descendant.as_ref().and_then(Object::dict);
        let descriptor = document.entry(descendant.unwrap_or(dict), b"FontDescriptor");
        let descriptor = descriptor.dict();
        let flags = descriptor.and_then(|d| d.get(b"Flags")).and_then(Object::int).unwrap_or(0);
        let weight = descriptor.and_then(|d| d.get(b"FontWeight")).and_then(Object::number).unwrap_or(400.0);
        let angle = descriptor.and_then(|d| d.get(b"ItalicAngle")).and_then(Object::number).unwrap_or(0.0);
        let bold = ["bold", "black", "heavy", "semibold", "demi"].iter().any(|word| name.contains(word)) || weight >= 600.0 || flags & (1 << 18) != 0;
        let italic = name.contains("italic") || name.contains("oblique") || flags & (1 << 6) != 0 || angle != 0.0;

        let unicode = document.stream_of(dict.get(b"ToUnicode").unwrap_or(&Object::Null)).map(|(_, data)| to_unicode(&data)).unwrap_or_default();
        let simple = (!wide).then(|| simple_encoding(document, dict));

        let mut widths = BTreeMap::new();
        let default_width;
        if let Some(descendant) = descendant {
            default_width = document.entry(descendant, b"DW").number().unwrap_or(1000.0);
            if let Object::Array(w) = document.entry(descendant, b"W") {
                cid_widths(document, &w, &mut widths);
            }
        } else {
            default_width = descriptor.and_then(|d| d.get(b"MissingWidth")).and_then(Object::number).unwrap_or(500.0);
            let first = document.entry(dict, b"FirstChar").int().and_then(|n| u32::try_from(n).ok()).unwrap_or(0);
            if let Object::Array(w) = document.entry(dict, b"Widths") {
                for (code, width) in (first..=u32::MAX).zip(w.iter().take(256)) {
                    if let Some(width) = document.resolve(width).number() {
                        widths.insert(code, width);
                    }
                }
            }
        }
        Font { bold, italic, wide, unicode, simple, widths, default_width }
    }

    /// The codes of a string: two bytes each for a composite font, else one.
    pub fn codes(&self, bytes: &[u8]) -> Vec<u32> {
        if self.wide {
            bytes.chunks(2).map(|pair| pair.iter().fold(0u32, |code, b| code << 8 | u32::from(*b))).collect()
        } else {
            bytes.iter().map(|b| u32::from(*b)).collect()
        }
    }

    /// The text of a code: by `ToUnicode`, else by the simple font's encoding; nothing for a code
    /// that says nothing.
    pub fn text(&self, code: u32) -> String {
        if let Some(text) = self.unicode.get(&code) {
            return text.clone();
        }
        match &self.simple {
            Some(table) => table.get(usize::try_from(code).unwrap_or(usize::MAX)).filter(|c| **c != '\0').map(|c| c.to_string()).unwrap_or_default(),
            None => String::new(),
        }
    }

    /// The width of a code, in thousandths of the size.
    pub fn width(&self, code: u32) -> f64 {
        self.widths.get(&code).copied().unwrap_or(self.default_width)
    }
}

/// `/W` of a CIDFont: `c [w1 w2 …]` from `c` on, or `c_first c_last w` (§9.7.4.3).
fn cid_widths(document: &Document, w: &[Object], widths: &mut BTreeMap<u32, f64>) {
    let mut items = w.iter().map(|item| document.resolve(item));
    while let Some(first) = items.next() {
        let Some(first) = first.int().and_then(|n| u32::try_from(n).ok()) else { break };
        match items.next() {
            Some(Object::Array(list)) => {
                for (code, width) in (first..=u32::MAX).zip(list.iter().take(MAX_RANGE as usize)) {
                    if let Some(width) = width.number() {
                        widths.insert(code, width);
                    }
                }
            }
            Some(last) => {
                let (Some(last), Some(width)) = (last.int().and_then(|n| u32::try_from(n).ok()), items.next().and_then(|w| w.number())) else { break };
                for code in first..=last.min(first.saturating_add(MAX_RANGE)) {
                    widths.insert(code, width);
                }
            }
            None => break,
        }
        if widths.len() > MAX_ENTRIES {
            break;
        }
    }
}

/// A `ToUnicode` CMap: `bfchar` and `bfrange` (§9.10.3), the text in UTF-16BE.
pub fn to_unicode(data: &[u8]) -> BTreeMap<u32, String> {
    let mut map = BTreeMap::new();
    let mut parser = Parser::new(data, 0, false);
    let code = |bytes: &[u8]| bytes.iter().take(4).fold(0u32, |code, b| code << 8 | u32::from(*b));
    while let Some(item) = parser.item() {
        let Item::Keyword(keyword) = item else { continue };
        match keyword.as_slice() {
            b"beginbfchar" => {
                while let Some(Item::Object(Object::Str(source))) = parser.item() {
                    if let Some(Item::Object(target)) = parser.item() {
                        if let Some(text) = target.string().map(utf16) {
                            map.insert(code(&source), text);
                        }
                    }
                    if map.len() > MAX_ENTRIES {
                        return map;
                    }
                }
            }
            b"beginbfrange" => {
                while let (Some(Item::Object(Object::Str(low))), Some(Item::Object(Object::Str(high)))) = (parser.item(), parser.item()) {
                    if !bfrange(&mut parser, code(&low), code(&high), &mut map) || map.len() > MAX_ENTRIES {
                        return map;
                    }
                }
            }
            _ => {}
        }
    }
    map
}

/// One range of a `bfrange` from `low` to `high`, its target read: a text whose last unit counts
/// up, or a text for each code. `false` where what follows is no target.
fn bfrange(parser: &mut Parser, low: u32, high: u32, map: &mut BTreeMap<u32, String>) -> bool {
    let target = parser.item();
    if high < low || high - low > MAX_RANGE {
        return matches!(target, Some(Item::Object(_)));
    }
    match target {
        Some(Item::Object(Object::Str(first))) => {
            // Each code one more in the last unit of the text.
            let mut units: Vec<u16> = first.chunks(2).map(|pair| pair.iter().fold(0u16, |unit, b| unit << 8 | u16::from(*b))).collect();
            for c in low..=high {
                map.insert(c, text_of(&units));
                if let Some(last) = units.last_mut() {
                    *last = last.wrapping_add(1);
                }
            }
            true
        }
        Some(Item::Object(Object::Array(texts))) => {
            for (c, text) in (low..=high).zip(texts.iter()) {
                if let Some(text) = text.string() {
                    map.insert(c, utf16(text));
                }
            }
            true
        }
        _ => false,
    }
}

/// UTF-16BE as text.
fn utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.chunks(2).map(|pair| pair.iter().fold(0u16, |unit, b| unit << 8 | u16::from(*b))).collect();
    text_of(&units)
}

/// UTF-16 units as text, without what stands for no text (`<0000> <ffff>`, a broken surrogate).
fn text_of(units: &[u16]) -> String {
    String::from_utf16_lossy(units).chars().filter(|c| !matches!(c, '\u{fffd}' | '\u{fffe}' | '\u{ffff}')).collect()
}

/// The text of each byte of a simple font: WinAnsiEncoding (or a base encoding read as it), with
/// its `/Differences` by glyph name.
fn simple_encoding(document: &Document, dict: &Dict) -> Vec<char> {
    let mut table: Vec<char> = (0u32..256).map(|code| win_ansi(code).unwrap_or('\0')).collect();
    if let Object::Dict(encoding) = document.entry(dict, b"Encoding") {
        if let Object::Array(differences) = document.entry(&encoding, b"Differences") {
            let mut code: Option<usize> = None;
            for item in differences {
                match item {
                    Object::Int(n) => code = usize::try_from(n).ok(),
                    Object::Name(name) => {
                        if let Some(at) = code {
                            if let (Some(slot), Some(c)) = (table.get_mut(at), glyph(&name)) {
                                *slot = c;
                            }
                            code = Some(at + 1);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    table
}

/// A byte of WinAnsiEncoding (Windows-1252, §D.2).
fn win_ansi(code: u32) -> Option<char> {
    const HIGH: [u16; 32] = [
        0x20ac, 0, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017d, 0, 0, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022,
        0x2013, 0x2014, 0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0, 0x017e, 0x0178,
    ];
    match code {
        0x20..=0x7e | 0xa0..=0xff => char::from_u32(code),
        0x80..=0x9f => HIGH.get(usize::try_from(code - 0x80).ok()?).copied().filter(|unit| *unit != 0).and_then(|unit| char::from_u32(u32::from(unit))),
        _ => None,
    }
}

/// The character of a glyph name (the Adobe Glyph List, as far as German and English text need
/// it; `uniXXXX`; a name of one letter or digit).
fn glyph(name: &[u8]) -> Option<char> {
    const NAMES: [(&str, char); 52] = [
        ("space", ' '), ("exclam", '!'), ("quotedbl", '"'), ("numbersign", '#'), ("dollar", '$'), ("percent", '%'), ("ampersand", '&'), ("quotesingle", '\''),
        ("parenleft", '('), ("parenright", ')'), ("asterisk", '*'), ("plus", '+'), ("comma", ','), ("hyphen", '-'), ("period", '.'), ("slash", '/'),
        ("zero", '0'), ("one", '1'), ("two", '2'), ("three", '3'), ("four", '4'), ("five", '5'), ("six", '6'), ("seven", '7'), ("eight", '8'), ("nine", '9'),
        ("colon", ':'), ("semicolon", ';'), ("less", '<'), ("equal", '='), ("greater", '>'), ("question", '?'), ("at", '@'), ("bracketleft", '['),
        ("bracketright", ']'), ("underscore", '_'), ("adieresis", 'ä'), ("odieresis", 'ö'), ("udieresis", 'ü'), ("Adieresis", 'Ä'), ("Odieresis", 'Ö'),
        ("Udieresis", 'Ü'), ("germandbls", 'ß'), ("eacute", 'é'), ("egrave", 'è'), ("endash", '–'), ("emdash", '—'), ("quoteleft", '‘'), ("quoteright", '’'),
        ("quotedblleft", '“'), ("quotedblright", '”'), ("quotedblbase", '„'),
    ];
    let text = std::str::from_utf8(name).ok()?;
    if let Some((_, c)) = NAMES.iter().find(|(known, _)| *known == text) {
        return Some(*c);
    }
    if let Some(hex) = text.strip_prefix("uni").filter(|hex| hex.len() == 4) {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_to_unicode_map_of_chars_and_ranges() {
        let cmap = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n1 begincodespacerange <0000> <FFFF> endcodespacerange\n2 beginbfchar\n<0003> <0054>\n<0010> <00FC>\nendbfchar\n2 beginbfrange\n<0020> <0022> <0061>\n<0030> <0031> [<0066006C> <D83DDE00>]\nendbfrange\nendcmap";
        let map = to_unicode(cmap);
        assert_eq!(map.get(&3).map(String::as_str), Some("T"));
        assert_eq!(map.get(&0x10).map(String::as_str), Some("ü"));
        assert_eq!(map.get(&0x22).map(String::as_str), Some("c"));
        assert_eq!(map.get(&0x30).map(String::as_str), Some("fl"));
        assert_eq!(map.get(&0x31).map(String::as_str), Some("😀"));
    }

    #[test]
    fn win_ansi_and_glyph_names() {
        assert_eq!(win_ansi(0xfc), Some('ü'));
        assert_eq!(win_ansi(0x96), Some('–'));
        assert_eq!(win_ansi(0x81), None);
        assert_eq!(glyph(b"germandbls"), Some('ß'));
        assert_eq!(glyph(b"uni00E4"), Some('ä'));
        assert_eq!(glyph(b"Q"), Some('Q'));
        assert_eq!(glyph(b"g123"), None);
    }

    #[test]
    fn two_bytes_a_code_in_a_composite_font() {
        let font = Font { wide: true, ..Font::default() };
        assert_eq!(font.codes(&[0, 3, 0, 0x10, 1]), vec![3, 0x10, 1]);
        let font = Font::default();
        assert_eq!(font.codes(&[0, 3]), vec![0, 3]);
    }
}
