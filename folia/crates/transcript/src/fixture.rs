//! A Notenübersicht as the BTU writes it, made up (Erika Mustermann, Informatik B.Sc.): for the
//! tests and the browser checks, which never see a real one (a real one names a student, her
//! grades, her address). It is written the way the BTU's are, as far as reading them depends on
//! it: the table's heads, its rows with every cell's lower edge drawn, headings in bold, the other
//! language in italic, the values centred in their columns; four composite fonts of `Identity-H`
//! with `ToUnicode` maps; the pages and fonts packed into an object stream, the streams compressed
//! and encrypted with RC4 of 128 bits for the empty user password; a page's coordinates flipped as
//! the BTU's are. `cargo run -p folia-transcript --features fixture --example fixture` writes it
//! (folia/e2e/fixtures/notenuebersicht.pdf is that file).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::pdf::crypt::{md5, rc4};

/// A row of the table.
#[derive(Clone, Debug)]
pub enum Row {
    /// A heading: German over English.
    Heading(&'static str, &'static str),
    /// A module: its title over the other language's, and its values as the columns write them
    /// (grade, status, credits, attempt, semester, date, recognition).
    Module { title: &'static str, other: &'static str, values: [&'static str; 7] },
}

/// How the file is written.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub encrypted: bool,
    /// The cells' lower edges drawn (the BTU's are).
    pub rules: bool,
    /// The pages and fonts in an object stream.
    pub packed: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { encrypted: true, rules: true, packed: true }
    }
}

const fn module(title: &'static str, other: &'static str, values: [&'static str; 7]) -> Row {
    Row::Module { title, other, values }
}

/// The made-up study: two pages, every status, a title over two lines, a module taught in
/// English (its English title first, in italic), headings in a row.
pub fn example_pages() -> Vec<Vec<Row>> {
    vec![
        vec![
            Row::Heading("Gesamtkonto", "Total Account"),
            Row::Heading("Grundstudium", "Basic Studies"),
            Row::Heading("Komplex Informatik", "Computer Science"),
            module("Theoretische Informatik", "Theoretical Computer Science", ["3,0", "BE", "8", "2", "WS 2024/25", "25.03.2025", ""]),
            module("Algorithmieren und Programmieren", "Design of Algorithms and Programming", ["2,3", "BE", "10", "1", "SS 2023", "29.09.2023", ""]),
            module("Programmierpraktikum", "Programming Laboratory", ["", "BE", "4", "1", "WS 2022/23", "", ""]),
            module(
                "Elektrische und elektronische Grundlagen der Informatik",
                "Electrical and Electronic Foundation of Computer Engineering",
                ["3,0", "BE", "6", "1", "WS 2022/23", "23.02.2023", ""],
            ),
            Row::Heading("Komplex Mathematik", "Mathematics"),
            module("Mathematik IT-1 (Diskrete Mathematik)", "Mathematics IT-1 (Discrete Mathematics)", ["2,7", "BE", "8", "1", "WS 2022/23", "22.03.2023", ""]),
            module("Mathematik IT-2 (Lineare Algebra)", "Mathematics IT-2 (Linear Algebra)", ["5,0", "NB", "0", "1", "SS 2023", "07.08.2023", ""]),
        ],
        vec![
            Row::Heading("Fachstudium", "Specific Studies"),
            module("Bachelor-Arbeit", "Bachelor Thesis", ["", "AN", "0", "1", "SS 2026", "23.11.2026", ""]),
            Row::Heading("Praktische Informatik", "Practical Computer Science"),
            module("Datenbanken", "Database Systems", ["2,0", "BE", "6", "1", "WS 2024/25", "18.03.2025", ""]),
            module("Graph Theory", "Graphentheorie", ["3,3", "BE", "8", "1", "WS 2025/26", "", ""]),
            module("Einführung Information Retrieval", "Introduction to Information Retrieval", ["5,0", "EN", "0", "3", "SS 2025", "", ""]),
            module("Sicherheitsengineering", "Security Engineering", ["1,7", "BE", "6", "1", "", "", "ja"]),
            Row::Heading("Zusatzmodule", "Additional Modules"),
            module("Einführung in die Constraint-Programmierung", "Introduction to Constraint Programming", ["2,0", "BE", "6", "1", "WS 2024/25", "", ""]),
        ],
    ]
}

/// The sentence above the table, German and English.
const SENTENCE: [&str; 2] = [
    "Erika Mustermann, geb. am 12. August 2003 in Musterstadt, studiert seit 1. Oktober 2022 im Bachelor-Studiengang Informatik (universitäres Profil) und erzielte bisher folgende Leistungen:",
    "Erika Mustermann, born on 12 August 2003 in Musterstadt, is currently enrolled in the Bachelor programme Computer Science (research-oriented profile) since 1 October 2022 and has to date achieved the following results:",
];

const FOOTER: [&str; 2] = [
    "Diese Notenübersicht ist maschinell erstellt und verifiziert. Sie trägt daher keine Unterschrift.",
    "This transcript of records was generated automatically. It does not have a signature.",
];

/// The made-up Notenübersicht as the BTU writes it.
pub fn example() -> Vec<u8> {
    pdf(&example_pages(), Options::default())
}

/// The page's height: A4.
const HEIGHT: f64 = 841.889;
const LEFT: f64 = 56.692;
/// The columns' edges after the title's.
const EDGES: [f64; 8] = [255.117, 283.463, 314.644, 379.84, 416.69, 467.712, 515.901, 566.922];
const SIZE: f64 = 10.0;
const LINE: f64 = 10.0;
const PAD: f64 = 2.5;
/// The widest a title's line is before it wraps.
const WRAP: f64 = 190.0;

/// The fonts, as the BTU names them: regular, bold, italic, bold italic.
const FONTS: [(&str, &str); 4] = [("F15", "ArialNarrow"), ("F16", "ArialNarrow-Bold"), ("F17", "ArialNarrow-Italic"), ("F18", "ArialNarrow-BoldItalic")];

/// The width of a character in thousandths, near enough to Arial Narrow's.
fn width(c: char) -> u32 {
    match c {
        ' ' | 'i' | 'l' | 'j' | 'I' | '.' | ',' | ':' | ';' | '\'' | '|' => 228,
        'f' | 't' | 'r' | '(' | ')' | '-' | '/' => 273,
        'm' | 'w' | 'M' | 'W' => 683,
        c if c.is_uppercase() => 546,
        _ => 456,
    }
}

fn text_width(text: &str) -> f64 {
    text.chars().map(|c| f64::from(width(c))).sum::<f64>() * SIZE / 1000.0
}

/// The lines a title wraps into within the title's column.
fn wrap(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split(' ') {
        match lines.last_mut() {
            Some(line) if text_width(&format!("{line}{word}")) <= WRAP => {
                line.push_str(word);
                line.push(' ');
            }
            _ => lines.push(format!("{word} ")),
        }
    }
    lines.into_iter().map(|line| line.trim_end().to_string()).collect()
}

/// What a page draws: text by font, and the rules.
struct Draw {
    content: String,
    /// The characters each font shows.
    used: [BTreeSet<char>; 4],
}

impl Draw {
    /// Text at `x`, `y` (from the top), in font `font` (0–3), as the BTU writes it: a `Tm` that
    /// flips back, a `TJ` of two-byte codes.
    fn text(&mut self, font: usize, x: f64, y: f64, text: &str) {
        if let Some(used) = self.used.get_mut(font) {
            used.extend(text.chars());
        }
        let name = FONTS.get(font).map_or("F15", |(name, _)| name);
        let _ = writeln!(self.content, "BT\n/{name} {SIZE} Tf\n1 0 0 -1 {x:.3} {y:.3} Tm [<{}>] TJ\nET", hex_codes(text));
    }

    fn rule(&mut self, x0: f64, x1: f64, y: f64) {
        let _ = writeln!(self.content, "q\n1 0 0 1 {x1:.3} {y:.3} cm -1 0 0 -1 0 0 cm 0 0 m {w:.3} 0 l {w:.3} 0.5 l 0 0.5 l h W\nn\n0 G\n[] 0 d 1 w\n0 0 m {w:.3} 0 l S\nQ", w = x1 - x0);
    }
}

/// A text's codes: its characters' (each a CID of its own, the same in every font).
fn hex_codes(text: &str) -> String {
    text.chars().map(|c| format!("{:04X}", cid(c))).collect()
}

/// The CID of a character: its code point where it fits two bytes.
fn cid(c: char) -> u32 {
    let code = u32::from(c);
    if code < 0xffff { code } else { 0x3f }
}

/// The pages drawn: the letterhead and the sentence on the first, the table's heads on each, the
/// rows, the footer.
fn draw(pages: &[Vec<Row>], options: Options) -> (Vec<String>, [BTreeSet<char>; 4]) {
    let mut out = Vec::new();
    let mut used: [BTreeSet<char>; 4] = Default::default();
    let pages_len = pages.len();
    for (index, rows) in pages.iter().enumerate() {
        let mut draw = Draw { content: format!("q\n1 0 0 -1 0 {HEIGHT} cm\n"), used: std::mem::take(&mut used) };
        let mut y = 120.0;
        if index == 0 {
            draw.text(1, LEFT, 116.0, "BTU Cottbus - Senftenberg");
            draw.text(0, LEFT, 140.0, "Erika Mustermann");
            draw.text(0, LEFT, 152.0, "Musterweg 1");
            draw.text(1, LEFT, 240.0, "Notenübersicht / ");
            draw.text(3, 137.0, 240.0, "Transcript of Records");
            for (at, sentence) in SENTENCE.iter().enumerate() {
                let font = if at == 0 { 0 } else { 2 };
                let mut line_y = 270.0 + 30.0 * at as f64;
                for line in wrap_wide(sentence) {
                    draw.text(font, LEFT, line_y, &line);
                    line_y += 12.0;
                }
            }
            y = 410.0;
        }
        // The heads: German in bold over English in bold italic, each centred in its column.
        let heads = [
            ("Modultitel", "Module Title"),
            ("Note", "Grade"),
            ("Status", "Status"),
            ("Leistungspunkte", "Credits"),
            ("Versuch", "Attempt"),
            ("Semester", "Semester"),
            ("Datum", "Date"),
            ("Anerkennung", "Recognition"),
        ];
        for (column, (german, english)) in heads.iter().enumerate() {
            for (line, (text, font)) in [(german, 1), (english, 3)].into_iter().enumerate() {
                let x = if column == 0 { LEFT } else { centred(column, text) };
                draw.text(font, x, y + 8.0 + LINE * line as f64, text);
            }
        }
        y += 20.5;
        if options.rules {
            row_rules(&mut draw, y);
        }
        for row in rows {
            let (lines, values): (Vec<(String, usize)>, Option<&[&str; 7]>) = match row {
                Row::Heading(german, english) => (vec![(german.to_string(), 1), (english.to_string(), 3)], None),
                Row::Module { title, other, values } => {
                    // A title taught in English is written in italic, its German in regular.
                    let english_first = title.starts_with("Graph") || title.starts_with("Proseminar Computers");
                    let (first, second) = if english_first { (2, 0) } else { (0, 2) };
                    let mut lines: Vec<(String, usize)> = wrap(title).into_iter().map(|line| (line, first)).collect();
                    lines.extend(wrap(other).into_iter().map(|line| (line, second)));
                    (lines, Some(values))
                }
            };
            let height = LINE * lines.len() as f64 + 2.0 * PAD + 4.5;
            for (at, (line, font)) in lines.iter().enumerate() {
                draw.text(*font, LEFT, y + PAD + 8.0 + LINE * at as f64, line);
            }
            if let Some(values) = values {
                let middle = y + height / 2.0 + 3.5;
                for (column, value) in values.iter().enumerate() {
                    if !value.is_empty() {
                        draw.text(0, centred(column + 1, value), middle, value);
                    }
                }
            }
            y += height;
            if options.rules {
                row_rules(&mut draw, y);
            }
        }
        if index + 1 == pages_len {
            draw.text(1, LEFT, y + 30.0, "Die Bachelorprüfung ist noch nicht abgeschlossen. Es besteht weiterhin Prüfungsanspruch.");
            draw.text(0, LEFT, y + 60.0, "Status:");
            draw.text(0, 120.0, y + 60.0, "BE = bestanden / passed");
        }
        for (at, line) in FOOTER.iter().enumerate() {
            draw.text(1 + 2 * at, LEFT, 770.0 + 15.0 * at as f64, line);
        }
        draw.content.push_str("Q\n");
        used = draw.used;
        out.push(draw.content);
    }
    (out, used)
}

/// The x where `text` stands centred in column `column` (1: the grade's).
fn centred(column: usize, text: &str) -> f64 {
    let left = EDGES.get(column.saturating_sub(1)).copied().unwrap_or(LEFT);
    let right = EDGES.get(column).copied().unwrap_or(left + 50.0);
    (left + right) / 2.0 - text_width(text) / 2.0
}

/// The lines of a sentence across the page.
fn wrap_wide(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split(' ') {
        match lines.last_mut() {
            Some(line) if text_width(&format!("{line}{word}")) <= 470.0 => {
                line.push_str(word);
                line.push(' ');
            }
            _ => lines.push(format!("{word} ")),
        }
    }
    lines
}

/// Every cell's lower edge at `y`, as the BTU draws them.
fn row_rules(draw: &mut Draw, y: f64) {
    let mut left = LEFT;
    for right in EDGES {
        draw.rule(left, right, y);
        left = right;
    }
}

/// The padding of a password (ISO 32000-1, Algorithm 2).
const PASSWORD_PAD: [u8; 32] = [
    0x28, 0xBF, 0x4E, 0x5E, 0x4E, 0x75, 0x8A, 0x41, 0x64, 0x00, 0x4E, 0x56, 0xFF, 0xFA, 0x01, 0x08, 0x2E, 0x2E, 0x00, 0xB6, 0xD0, 0x68, 0x3E, 0x80, 0x2F, 0x0C,
    0xA9, 0xFE, 0x64, 0x53, 0x69, 0x7A,
];

/// The permissions the BTU's file grants: printing and the extraction of text for accessibility
/// and other purposes, no copying.
const PERMISSIONS: i32 = -60;

/// `/O` and `/U` and the file's key for the owner password `owner` and the empty user password
/// (Algorithms 3, 2 and 5, revision 3, 128 bits).
fn keys(id: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut owner = b"owner".to_vec();
    owner.extend_from_slice(PASSWORD_PAD.get(..32 - 5).unwrap_or_default());
    let mut hash = md5(&owner);
    for _ in 0..50 {
        hash = md5(&hash);
    }
    let mut o = rc4(&hash, &PASSWORD_PAD);
    for i in 1..=19u8 {
        let step: Vec<u8> = hash.iter().map(|b| b ^ i).collect();
        o = rc4(&step, &o);
    }
    let mut input = PASSWORD_PAD.to_vec();
    input.extend_from_slice(&o);
    input.extend_from_slice(&(PERMISSIONS as u32).to_le_bytes());
    input.extend_from_slice(id);
    let mut key = md5(&input);
    for _ in 0..50 {
        key = md5(&key);
    }
    let mut check = PASSWORD_PAD.to_vec();
    check.extend_from_slice(id);
    let mut u = rc4(&key, &md5(&check));
    for i in 1..=19u8 {
        let step: Vec<u8> = key.iter().map(|b| b ^ i).collect();
        u = rc4(&step, &u);
    }
    u.extend_from_slice(&[0; 16]);
    (o, u, key.to_vec())
}

/// `data` in the zlib format with stored blocks: what FlateDecode reads, without a compressor.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = if data.is_empty() { vec![&[][..]] } else { data.chunks(0xffff).collect() };
    let last = chunks.len() - 1;
    for (at, chunk) in chunks.iter().enumerate() {
        out.push(u8::from(at == last));
        let length = chunk.len() as u16;
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&(!length).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend_from_slice(&(b << 16 | a).to_be_bytes());
    out
}

/// The PDF of `pages`.
pub fn pdf(pages: &[Vec<Row>], options: Options) -> Vec<u8> {
    let (contents, used) = draw(pages, options);
    let id: Vec<u8> = md5(b"Erika Mustermann").to_vec();
    let (o, u, key) = keys(&id);
    let crypt = |num: u32, data: &[u8]| -> Vec<u8> {
        if !options.encrypted {
            return data.to_vec();
        }
        let mut input = key.clone();
        input.extend_from_slice(num.to_le_bytes().get(..3).unwrap_or_default());
        input.extend_from_slice(&[0, 0]);
        rc4(&md5(&input), data)
    };

    // The objects, by number: (number, body, a stream's data).
    let pages_count = contents.len() as u32;
    // 1 catalog, 2 pages, 3.. the pages, then their contents, then per font: the Type0, the
    // CIDFont, the ToUnicode; then the encryption; the object stream; the xref stream.
    let page_num = |at: u32| 3 + at;
    let content_num = |at: u32| 3 + pages_count + at;
    let font_num = |at: u32| 3 + 2 * pages_count + 3 * at;
    let encrypt_num = 3 + 2 * pages_count + 12;
    let objstm_num = encrypt_num + 1;
    let xref_num = encrypt_num + 2;

    let fonts_dict: String = FONTS.iter().enumerate().map(|(at, (name, _))| format!("/{name} {} 0 R", font_num(at as u32))).collect::<Vec<_>>().join(" ");
    let kids: String = (0..pages_count).map(|at| format!("{} 0 R", page_num(at))).collect::<Vec<_>>().join(" ");
    let mut plain: BTreeMap<u32, String> = BTreeMap::new();
    let mut streams: BTreeMap<u32, (String, Vec<u8>)> = BTreeMap::new();
    plain.insert(1, "<< /Type /Catalog /Pages 2 0 R >>".to_string());
    plain.insert(2, format!("<< /Type /Pages /Kids [{kids}] /Count {pages_count} /Resources << /Font << {fonts_dict} >> >> >>"));
    for (at, content) in contents.iter().enumerate() {
        let at = at as u32;
        plain.insert(page_num(at), format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.276 {HEIGHT}] /Contents {} 0 R >>", content_num(at)));
        streams.insert(content_num(at), ("/Filter /FlateDecode".to_string(), zlib_stored(content.as_bytes())));
    }
    for (at, (_, base)) in FONTS.iter().enumerate() {
        let num = font_num(at as u32);
        let chars = used.get(at).cloned().unwrap_or_default();
        let widths: String = chars.iter().map(|c| format!("{} [{}]", cid(*c), width(*c))).collect::<Vec<_>>().join(" ");
        // The subset's tag, as the BTU's: EAAAAA, EAAAAB …
        let tag = format!("EAAAA{}", char::from(b'A' + at as u8));
        plain.insert(num, format!("<< /Type /Font /Subtype /Type0 /BaseFont /{tag}+{base} /Encoding /Identity-H /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>", num + 1, num + 2));
        plain.insert(
            num + 1,
            format!("<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{tag}+{base} /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> /DW 0 /W [{widths}] /CIDToGIDMap /Identity >>"),
        );
        let mut cmap = String::from("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
        // At most 100 entries a block (§9.10.3).
        let mut entries = vec!["<0000> <ffff>".to_string()];
        for c in &chars {
            let mut units = [0u16; 2];
            let units: String = c.encode_utf16(&mut units).iter().map(|unit| format!("{unit:04x}")).collect();
            entries.push(format!("<{:04x}> <{units}>", cid(*c)));
        }
        for block in entries.chunks(100) {
            let _ = writeln!(cmap, "{} beginbfchar\n{}\nendbfchar", block.len(), block.join("\n"));
        }
        cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        streams.insert(num + 2, ("/Filter /FlateDecode".to_string(), zlib_stored(cmap.as_bytes())));
    }

    // The pages and the fonts' dictionaries packed, where they are.
    let packed: Vec<u32> = if options.packed { plain.keys().copied().filter(|num| *num != 1).collect() } else { Vec::new() };
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets: BTreeMap<u32, usize> = BTreeMap::new();
    fn write(out: &mut Vec<u8>, offsets: &mut BTreeMap<u32, usize>, num: u32, body: &[u8]) {
        offsets.insert(num, out.len());
        out.extend_from_slice(format!("{num} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    for (num, body) in &plain {
        if !packed.contains(num) {
            write(&mut out, &mut offsets, *num, body.as_bytes());
        }
    }
    for (num, (dict, data)) in &streams {
        let data = crypt(*num, data);
        let mut body = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
        body.extend_from_slice(&data);
        body.extend_from_slice(b"\nendstream");
        write(&mut out, &mut offsets, *num, &body);
    }
    let mut index_of: BTreeMap<u32, usize> = BTreeMap::new();
    if !packed.is_empty() {
        let mut head = String::new();
        let mut objects = String::new();
        for (at, num) in packed.iter().enumerate() {
            index_of.insert(*num, at);
            let _ = write!(head, "{num} {} ", objects.len());
            objects.push_str(plain.get(num).map_or("null", String::as_str));
            objects.push('\n');
        }
        let data = zlib_stored(format!("{head}\n{objects}").as_bytes());
        let data = crypt(objstm_num, &data);
        let mut body = format!("<< /Type /ObjStm /N {} /First {} /Filter /FlateDecode /Length {} >>\nstream\n", packed.len(), head.len() + 1, data.len()).into_bytes();
        body.extend_from_slice(&data);
        body.extend_from_slice(b"\nendstream");
        write(&mut out, &mut offsets, objstm_num, &body);
    }
    let hex_id: String = id.iter().map(|b| format!("{b:02x}")).collect();
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    if options.encrypted {
        let body = format!("<< /Filter /Standard /V 2 /R 3 /Length 128 /P {PERMISSIONS} /O <{}> /U <{}> >>", hex(&o), hex(&u));
        write(&mut out, &mut offsets, encrypt_num, body.as_bytes());
    }
    // The cross-reference stream: offsets, and where each packed object is.
    let size = xref_num + 1;
    let start = out.len();
    let mut entries: Vec<u8> = Vec::new();
    for num in 0..size {
        let (kind, field2, field3): (u8, u32, u16) = if num == xref_num {
            (1, start as u32, 0)
        } else if let Some(at) = index_of.get(&num) {
            (2, objstm_num, *at as u16)
        } else if let Some(offset) = offsets.get(&num) {
            (1, *offset as u32, 0)
        } else {
            (0, 0, if num == 0 { 0xffff } else { 0 })
        };
        entries.push(kind);
        entries.extend_from_slice(&field2.to_be_bytes());
        entries.extend_from_slice(&field3.to_be_bytes());
    }
    let encrypt = if options.encrypted { format!(" /Encrypt {encrypt_num} 0 R") } else { String::new() };
    let mut body = format!("<< /Type /XRef /Size {size} /W [1 4 2] /Root 1 0 R{encrypt} /ID [<{hex_id}> <{hex_id}>] /Length {} >>\nstream\n", entries.len()).into_bytes();
    body.extend_from_slice(&entries);
    body.extend_from_slice(b"\nendstream");
    write(&mut out, &mut offsets, xref_num, &body);
    out.extend_from_slice(format!("startxref\n{start}\n%%EOF\n").as_bytes());
    out
}
