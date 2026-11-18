//! The syntax of a PDF file (ISO 32000-1 §7.2, §7.3): its tokens and objects, read from bytes that
//! may be anything. Nothing here panics or reads past an end, every nesting has a depth limit, and
//! every loop moves forward.

/// The deepest arrays and dictionaries nest; a real file nests a few levels.
const MAX_DEPTH: usize = 32;

/// An object of the file. A stream is an indirect object, reached by its reference
/// (`Document::stream`).
#[derive(Clone, Debug, PartialEq)]
pub enum Object {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Str(Vec<u8>),
    Name(Vec<u8>),
    Array(Vec<Object>),
    Dict(Dict),
    Ref(u32, u16),
}

/// A dictionary, its entries in the order of the file; the last of a key counts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dict(pub Vec<(Vec<u8>, Object)>);

impl Dict {
    pub fn get(&self, key: &[u8]) -> Option<&Object> {
        self.0.iter().rev().find(|(k, _)| k.as_slice() == key).map(|(_, value)| value)
    }

    /// Whether `/Type` (or another key) names `name`.
    pub fn is(&self, key: &[u8], name: &[u8]) -> bool {
        self.get(key).and_then(Object::name) == Some(name)
    }
}

impl Object {
    pub fn int(&self) -> Option<i64> {
        match self {
            Object::Int(i) => Some(*i),
            Object::Real(r) if r.fract() == 0.0 && r.abs() < 9.0e15 => Some(*r as i64),
            _ => None,
        }
    }

    pub fn number(&self) -> Option<f64> {
        match self {
            Object::Int(i) => Some(*i as f64),
            Object::Real(r) => Some(*r),
            _ => None,
        }
    }

    pub fn name(&self) -> Option<&[u8]> {
        match self {
            Object::Name(name) => Some(name),
            _ => None,
        }
    }

    pub fn string(&self) -> Option<&[u8]> {
        match self {
            Object::Str(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn dict(&self) -> Option<&Dict> {
        match self {
            Object::Dict(dict) => Some(dict),
            _ => None,
        }
    }

    pub fn array(&self) -> Option<&[Object]> {
        match self {
            Object::Array(items) => Some(items),
            _ => None,
        }
    }
}

pub fn is_space(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

pub fn is_delimiter(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

pub fn is_regular(b: u8) -> bool {
    !is_space(b) && !is_delimiter(b)
}

/// What the lexer reads.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Int(i64),
    Real(f64),
    Str(Vec<u8>),
    Name(Vec<u8>),
    Keyword(Vec<u8>),
    ArrayOpen,
    ArrayClose,
    DictOpen,
    DictClose,
}

/// What the parser reads: an object, or a keyword (`obj`, `stream`, an operator of a content
/// stream).
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Object(Object),
    Keyword(Vec<u8>),
}

/// Tokens and objects from `data`, from a position on.
pub struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
    /// Whether `n g R` is a reference: in the file's objects, not in a content stream.
    refs: bool,
}

impl<'a> Parser<'a> {
    pub fn new(data: &'a [u8], pos: usize, refs: bool) -> Self {
        Parser { data, pos, refs }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<u8> {
        self.data.get(self.pos.saturating_add(ahead)).copied()
    }

    /// Past whitespace and comments.
    pub fn skip_space(&mut self) {
        while let Some(b) = self.peek() {
            if is_space(b) {
                self.pos += 1;
            } else if b == b'%' {
                while let Some(b) = self.peek() {
                    if b == b'\n' || b == b'\r' {
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    /// The next token; `None` at the end. A stray `)`, `>`, `{` or `}` is passed over.
    pub fn token(&mut self) -> Option<Token> {
        loop {
            self.skip_space();
            let b = self.peek()?;
            match b {
                b'[' => {
                    self.pos += 1;
                    return Some(Token::ArrayOpen);
                }
                b']' => {
                    self.pos += 1;
                    return Some(Token::ArrayClose);
                }
                b'<' if self.peek_at(1) == Some(b'<') => {
                    self.pos += 2;
                    return Some(Token::DictOpen);
                }
                b'>' if self.peek_at(1) == Some(b'>') => {
                    self.pos += 2;
                    return Some(Token::DictClose);
                }
                b'<' => return Some(Token::Str(self.hex_string())),
                b'(' => return Some(Token::Str(self.literal_string())),
                b'/' => return Some(Token::Name(self.name())),
                b')' | b'>' | b'{' | b'}' => self.pos += 1,
                _ => {
                    let start = self.pos;
                    while self.peek().is_some_and(is_regular) {
                        self.pos += 1;
                    }
                    let word = self.data.get(start..self.pos).unwrap_or_default();
                    return Some(number(word).unwrap_or_else(|| Token::Keyword(word.to_vec())));
                }
            }
        }
    }

    /// `<…>`: pairs of hex digits, whitespace between them ignored, an odd last digit followed by
    /// 0 (§7.3.4.3).
    fn hex_string(&mut self) -> Vec<u8> {
        self.pos += 1;
        let mut out = Vec::new();
        let mut high: Option<u8> = None;
        while let Some(b) = self.peek() {
            self.pos += 1;
            if b == b'>' {
                break;
            }
            let Some(digit) = hex_digit(b) else { continue };
            match high.take() {
                Some(h) => out.push(h << 4 | digit),
                None => high = Some(digit),
            }
        }
        if let Some(h) = high {
            out.push(h << 4);
        }
        out
    }

    /// `(…)`: balanced parentheses, the escapes of §7.3.4.2, an end of line read as `\n`.
    fn literal_string(&mut self) -> Vec<u8> {
        self.pos += 1;
        let mut out = Vec::new();
        let mut depth = 1usize;
        while let Some(b) = self.peek() {
            self.pos += 1;
            match b {
                b'(' => {
                    depth += 1;
                    out.push(b);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(b);
                }
                b'\r' => {
                    if self.peek() == Some(b'\n') {
                        self.pos += 1;
                    }
                    out.push(b'\n');
                }
                b'\\' => {
                    let Some(e) = self.peek() else { break };
                    self.pos += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\r' => {
                            if self.peek() == Some(b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut value = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'7') => {
                                        value = value * 8 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((value & 0xff) as u8);
                        }
                        other => out.push(other),
                    }
                }
                other => out.push(other),
            }
        }
        out
    }

    /// `/Name`, with its `#xx` escapes.
    fn name(&mut self) -> Vec<u8> {
        self.pos += 1;
        let mut out = Vec::new();
        while let Some(b) = self.peek() {
            if !is_regular(b) {
                break;
            }
            self.pos += 1;
            if b == b'#' {
                if let (Some(h), Some(l)) = (self.peek().and_then(hex_digit), self.peek_at(1).and_then(hex_digit)) {
                    self.pos += 2;
                    out.push(h << 4 | l);
                    continue;
                }
            }
            out.push(b);
        }
        out
    }

    /// The next object or keyword; `None` at the end.
    pub fn item(&mut self) -> Option<Item> {
        self.item_at(0)
    }

    fn item_at(&mut self, depth: usize) -> Option<Item> {
        loop {
            let token = self.token()?;
            return Some(match token {
                Token::Int(n) => Item::Object(self.reference(n).unwrap_or(Object::Int(n))),
                Token::Real(r) => Item::Object(Object::Real(r)),
                Token::Str(s) => Item::Object(Object::Str(s)),
                Token::Name(n) => Item::Object(Object::Name(n)),
                Token::Keyword(k) => match k.as_slice() {
                    b"true" => Item::Object(Object::Bool(true)),
                    b"false" => Item::Object(Object::Bool(false)),
                    b"null" => Item::Object(Object::Null),
                    _ => Item::Keyword(k),
                },
                Token::ArrayOpen if depth >= MAX_DEPTH => {
                    self.skip_nested();
                    Item::Object(Object::Null)
                }
                Token::DictOpen if depth >= MAX_DEPTH => {
                    self.skip_nested();
                    Item::Object(Object::Null)
                }
                Token::ArrayOpen => Item::Object(Object::Array(self.array(depth + 1))),
                Token::DictOpen => Item::Object(Object::Dict(self.dict(depth + 1))),
                // A stray close: passed over.
                Token::ArrayClose | Token::DictClose => continue,
            });
        }
    }

    /// `n g R` after an integer `n` that was read, where references are read; else nothing is
    /// consumed.
    fn reference(&mut self, n: i64) -> Option<Object> {
        if !self.refs {
            return None;
        }
        let back = self.pos;
        let found = (|| {
            let Some(Token::Int(g)) = self.token() else { return None };
            let Some(Token::Keyword(r)) = self.token() else { return None };
            (r == b"R").then_some(())?;
            Some(Object::Ref(u32::try_from(n).ok()?, u16::try_from(g).ok()?))
        })();
        if found.is_none() {
            self.pos = back;
        }
        found
    }

    fn array(&mut self, depth: usize) -> Vec<Object> {
        let mut items = Vec::new();
        loop {
            self.skip_space();
            match self.peek() {
                None => break,
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                _ => {}
            }
            match self.item_at(depth) {
                Some(Item::Object(object)) => items.push(object),
                // A keyword in an array (not of this syntax): passed over.
                Some(Item::Keyword(_)) => {}
                None => break,
            }
        }
        items
    }

    fn dict(&mut self, depth: usize) -> Dict {
        let mut entries = Vec::new();
        loop {
            self.skip_space();
            match (self.peek(), self.peek_at(1)) {
                (None, _) => break,
                (Some(b'>'), Some(b'>')) => {
                    self.pos += 2;
                    break;
                }
                _ => {}
            }
            let at = self.pos;
            match self.token() {
                Some(Token::Name(key)) => match self.item_at(depth) {
                    Some(Item::Object(value)) => entries.push((key, value)),
                    Some(Item::Keyword(_)) => {}
                    None => break,
                },
                // A dictionary ended by `]` or by a keyword: what follows is not its.
                Some(Token::ArrayClose) | Some(Token::Keyword(_)) => {
                    self.pos = at;
                    break;
                }
                Some(_) => {}
                None => break,
            }
        }
        Dict(entries)
    }

    /// Past the rest of an array or a dictionary nested too deep, counting without recursion.
    fn skip_nested(&mut self) {
        let mut depth = 1usize;
        while depth > 0 {
            match self.token() {
                Some(Token::ArrayOpen | Token::DictOpen) => depth += 1,
                Some(Token::ArrayClose | Token::DictClose) => depth -= 1,
                Some(_) => {}
                None => break,
            }
        }
    }

    /// Whether the next token is the keyword `word`; consumed if it is.
    pub fn keyword_is(&mut self, word: &[u8]) -> bool {
        let back = self.pos;
        match self.token() {
            Some(Token::Keyword(k)) if k == word => true,
            _ => {
                self.pos = back;
                false
            }
        }
    }

    /// Past the data of an inline image (`BI … ID <data> EI`), the operands of `BI` already
    /// read: to the `EI` that stands alone after whitespace.
    pub fn skip_inline_image(&mut self) {
        // The image's dictionary, up to `ID`.
        loop {
            match self.token() {
                Some(Token::Keyword(k)) if k == b"ID" => break,
                Some(_) => {}
                None => return,
            }
        }
        // One whitespace byte after `ID`, then the data.
        self.pos += 1;
        while self.pos < self.data.len() {
            let here = self.data.get(self.pos..).unwrap_or_default();
            let before = self.pos.checked_sub(1).and_then(|at| self.data.get(at)).copied();
            if here.starts_with(b"EI") && before.is_some_and(is_space) && here.get(2).is_none_or(|b| is_space(*b)) {
                self.pos += 2;
                return;
            }
            self.pos += 1;
        }
    }
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// A number token: an optional sign, digits with at most one point, at least one digit
/// (§7.3.3). An integer too large for 64 bits is read as a real.
fn number(word: &[u8]) -> Option<Token> {
    let (negative, digits) = match word.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, word),
    };
    if !digits.contains(&b'.') {
        let integer = digits.iter().try_fold(0i64, |value, b| if b.is_ascii_digit() { value.checked_mul(10)?.checked_add(i64::from(b - b'0')) } else { None });
        if let Some(value) = integer.filter(|_| !digits.is_empty()) {
            return Some(Token::Int(if negative { -value } else { value }));
        }
    }
    let value = decimal(digits)?;
    Some(Token::Real(if negative { -value } else { value }))
}

/// Digits with at most one point, at least one digit: „12", „.5", „4.", „3.25". Read by hand
/// rather than by `str::parse`, whose general reader of floats is most of a small module's size.
pub fn decimal(digits: &[u8]) -> Option<f64> {
    let (mut value, mut scale, mut point, mut any) = (0.0f64, 1.0f64, false, false);
    for b in digits {
        match b {
            b'0'..=b'9' => {
                any = true;
                let digit = f64::from(b - b'0');
                if point {
                    scale /= 10.0;
                    value += digit * scale;
                } else {
                    value = value * 10.0 + digit;
                }
            }
            b'.' if !point => point = true,
            _ => return None,
        }
    }
    (any && value.is_finite()).then_some(value)
}

/// Where `needle` comes first in `data` from `from` on.
pub fn find(data: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let rest = data.get(from..)?;
    rest.windows(needle.len()).position(|window| window == needle).map(|at| at + from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(text: &str, refs: bool) -> Vec<Item> {
        let mut parser = Parser::new(text.as_bytes(), 0, refs);
        std::iter::from_fn(|| parser.item()).collect()
    }

    #[test]
    fn reads_the_objects_of_section_7_3() {
        let read = items("<< /Type /Page /Kids [1 0 R 2 0 R] /N -3.5 /S (a\\(b\\)c\\101) /H <48 65 6>> /B true /Z null >>", true);
        let [Item::Object(Object::Dict(dict))] = read.as_slice() else { unreachable!("{read:?}") };
        assert!(dict.is(b"Type", b"Page"));
        assert_eq!(dict.get(b"Kids"), Some(&Object::Array(vec![Object::Ref(1, 0), Object::Ref(2, 0)])));
        assert_eq!(dict.get(b"N"), Some(&Object::Real(-3.5)));
        assert_eq!(dict.get(b"S").and_then(Object::string), Some(b"a(b)cA".as_slice()));
        assert_eq!(dict.get(b"H").and_then(Object::string), Some(b"He`".as_slice()));
        assert_eq!(dict.get(b"B"), Some(&Object::Bool(true)));
        assert_eq!(dict.get(b"Z"), Some(&Object::Null));
    }

    #[test]
    fn a_content_stream_is_operands_and_operators() {
        let read = items("1 0 0 -1 0 841.889 cm /F1 10 Tf [<0001> -250 <0002>] TJ % a comment\nT* (x) '", false);
        assert_eq!(read.iter().filter(|item| matches!(item, Item::Keyword(_))).count(), 5);
        assert_eq!(read.get(6), Some(&Item::Keyword(b"cm".to_vec())));
        assert!(matches!(read.get(10), Some(Item::Object(Object::Array(parts))) if parts.len() == 3));
    }

    #[test]
    fn names_and_numbers_as_written() {
        assert_eq!(items("/A#20B .5 -.25 4. +7 1e5", false), vec![
            Item::Object(Object::Name(b"A B".to_vec())),
            Item::Object(Object::Real(0.5)),
            Item::Object(Object::Real(-0.25)),
            Item::Object(Object::Real(4.0)),
            Item::Object(Object::Int(7)),
            Item::Keyword(b"1e5".to_vec()),
        ]);
    }

    #[test]
    fn garbage_ends_without_a_panic() {
        for text in ["((((", "<<<<<<", "[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[", ")>>]]}{", "/", "<", "(\\", "1 0", "BI /W 1 ID xx"] {
            let _ = items(text, true);
            let mut parser = Parser::new(text.as_bytes(), 0, false);
            parser.skip_inline_image();
        }
        let deep = "[".repeat(10_000);
        assert_eq!(items(&deep, true).len(), 1);
    }

    #[test]
    fn an_inline_image_is_passed_over() {
        let text = b"BI /W 2 /H 1 ID \x00EI\xff EI Q";
        let mut parser = Parser::new(text, 2, false);
        parser.skip_inline_image();
        assert_eq!(parser.item(), Some(Item::Keyword(b"Q".to_vec())));
    }
}
