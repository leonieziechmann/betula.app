//! The SVGs of `folia/assets` without what no renderer reads (`build/main.rs`): whitespace between
//! tags, comments, and the long way of writing numbers and path data. The drawing stays the same,
//! number for number: nothing is rounded. A number loses its leading zero and its trailing ones
//! (`-0.70` → `-.7`); a path's segment is written relative to where it starts when that is shorter,
//! a line along an axis as `H`/`V`, a command that repeats without its letter, and between numbers
//! only the separators the grammar needs (`2.6.9-1.5`). Quotes, ids and colours stay as they are:
//! the link-preview cards find them as text (`cards::mask`).
//!
//! What it cannot read stays as it was: a tag, a path or a transform it does not understand is
//! copied, and a path is only replaced when reading the new one gives every point of the old.
//! Shared with the server's tests (`src/tests.rs`), which draw every file before and after.

/// `source` without what no renderer reads.
pub fn minify(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    // Inside `<text>`, `<style>` and their kind, whitespace is content.
    let mut verbatim = 0usize;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<!--") {
            // A comment: dropped.
            rest = after.find("-->").and_then(|end| after.get(end + 3..)).unwrap_or_default();
        } else if rest.starts_with("<?") || rest.starts_with("<!") {
            // A declaration, a doctype or CDATA: kept as it is.
            let marker = if rest.starts_with("<![CDATA[") { "]]>" } else if rest.starts_with("<?") { "?>" } else { ">" };
            let end = rest.find(marker).map_or(rest.len(), |end| end + marker.len());
            let (kept, after) = rest.split_at(end);
            out.push_str(kept);
            rest = after;
        } else if rest.starts_with('<') {
            let end = tag_end(rest).map_or(rest.len(), |end| end + 1);
            let (tag, after) = rest.split_at(end);
            let name = tag_name(tag);
            if TEXT_ELEMENTS.contains(&name) && !tag.ends_with("/>") {
                if tag.starts_with("</") {
                    verbatim = verbatim.saturating_sub(1);
                } else {
                    verbatim += 1;
                }
            }
            match rewrite_tag(tag) {
                Some(rewritten) => out.push_str(&rewritten),
                None => out.push_str(tag),
            }
            rest = after;
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let (text, after) = rest.split_at(end);
            if verbatim > 0 || !text.trim().is_empty() {
                out.push_str(text);
            }
            rest = after;
        }
    }
    out
}

/// Elements whose text is drawn or read: whitespace inside them is kept.
const TEXT_ELEMENTS: [&str; 8] = ["text", "tspan", "textPath", "title", "desc", "style", "script", "foreignObject"];

/// Attributes whose value is one plain number.
const NUMBERS: [&str; 21] = [
    "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "rx", "ry", "width", "height", "offset", "opacity", "fill-opacity", "stroke-opacity", "stop-opacity", "stroke-width", "stroke-miterlimit", "stroke-dashoffset",
];

/// Where the tag at the start of `rest` ends: its `>`, outside of quoted values.
fn tag_end(rest: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in rest.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), _) if c == open => quote = None,
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

fn tag_name(tag: &str) -> &str {
    let inner = tag.trim_start_matches('<').trim_start_matches('/');
    let end = inner.find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>').unwrap_or(inner.len());
    inner.get(..end).unwrap_or_default()
}

/// A tag with one space between its attributes and each value as short as it goes; `None` for
/// what it cannot read.
fn rewrite_tag(tag: &str) -> Option<String> {
    if tag.starts_with("</") {
        return Some(format!("</{}>", tag_name(tag)));
    }
    let inner = tag.strip_prefix('<')?.strip_suffix('>')?;
    let (inner, closed) = match inner.strip_suffix('/') {
        Some(inner) => (inner, true),
        None => (inner, false),
    };
    let name = tag_name(tag);
    let mut attributes = inner.get(name.len()..)?;
    let mut out = format!("<{name}");
    loop {
        attributes = attributes.trim_start();
        if attributes.is_empty() {
            break;
        }
        let (attribute, after) = attributes.split_once('=')?;
        let attribute = attribute.trim();
        if attribute.is_empty() || attribute.contains(|c: char| c.is_ascii_whitespace()) {
            return None;
        }
        let after = after.trim_start();
        let quote = after.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        let (value, after) = after.get(1..)?.split_once(quote)?;
        let value = shorter(attribute, value);
        out.push_str(&format!(" {attribute}={quote}{value}{quote}"));
        attributes = after;
    }
    out.push_str(if closed { "/>" } else { ">" });
    Some(out)
}

/// An attribute's value as short as it goes, or as it was.
fn shorter(attribute: &str, value: &str) -> String {
    let short = match attribute {
        "d" => path(value),
        "transform" | "gradientTransform" | "patternTransform" => transforms(value),
        "viewBox" => numbers(value).map(|numbers| numbers.join(" ")),
        _ if NUMBERS.contains(&attribute) => single_number(value),
        _ => None,
    };
    short.unwrap_or_else(|| value.to_string())
}

/// A number exactly as it is written: `mantissa` × 10^-`scale`.
#[derive(Clone, Copy, Debug)]
struct Decimal {
    mantissa: i64,
    scale: u32,
}

/// More digits than any file here writes; a number beyond it leaves its path as it is.
const MAX_SCALE: u32 = 12;

impl Decimal {
    const ZERO: Decimal = Decimal { mantissa: 0, scale: 0 };

    fn aligned(self, other: Decimal) -> Option<(i64, i64, u32)> {
        let scale = self.scale.max(other.scale);
        let widen = |d: Decimal| d.mantissa.checked_mul(10i64.checked_pow(scale - d.scale)?);
        Some((widen(self)?, widen(other)?, scale))
    }

    fn add(self, other: Decimal) -> Option<Decimal> {
        let (a, b, scale) = self.aligned(other)?;
        Some(Decimal { mantissa: a.checked_add(b)?, scale })
    }

    fn sub(self, other: Decimal) -> Option<Decimal> {
        let (a, b, scale) = self.aligned(other)?;
        Some(Decimal { mantissa: a.checked_sub(b)?, scale })
    }

    fn same(self, other: Decimal) -> bool {
        self.aligned(other).is_some_and(|(a, b, _)| a == b)
    }

    /// As short as it is written: no sign on zero, no leading zero, no trailing zeros.
    fn text(self) -> String {
        let (mut mantissa, mut scale) = (self.mantissa, self.scale);
        while scale > 0 && mantissa % 10 == 0 {
            mantissa /= 10;
            scale -= 1;
        }
        let sign = if mantissa < 0 { "-" } else { "" };
        let digits = mantissa.unsigned_abs().to_string();
        let scale = scale as usize;
        if scale == 0 {
            format!("{sign}{digits}")
        } else if digits.len() > scale {
            let (whole, fraction) = digits.split_at(digits.len() - scale);
            format!("{sign}{whole}.{fraction}")
        } else {
            format!("{sign}.{}{digits}", "0".repeat(scale - digits.len()))
        }
    }
}

/// Reads numbers, flags and command letters as the grammar of path data has them.
struct Cursor<'a> {
    rest: &'a str,
}

impl<'a> Cursor<'a> {
    fn skip_separators(&mut self) {
        self.rest = self.rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
    }

    fn at_end(&mut self) -> bool {
        self.skip_separators();
        self.rest.is_empty()
    }

    /// The command letter that comes next, if one does.
    fn command(&mut self) -> Option<char> {
        self.skip_separators();
        let c = self.rest.chars().next().filter(|c| "MmLlHhVvCcSsQqTtAaZz".contains(*c))?;
        self.rest = self.rest.get(1..)?;
        Some(c)
    }

    fn number(&mut self) -> Option<Decimal> {
        self.skip_separators();
        let bytes = self.rest.as_bytes();
        let mut i = 0;
        let negative = match bytes.first() {
            Some(b'-') => {
                i += 1;
                true
            }
            Some(b'+') => {
                i += 1;
                false
            }
            _ => false,
        };
        let mut mantissa: i64 = 0;
        let mut digits = 0;
        let mut scale: i64 = 0;
        let push = |mantissa: &mut i64, digit: u8| -> Option<()> {
            *mantissa = mantissa.checked_mul(10)?.checked_add(i64::from(digit - b'0'))?;
            Some(())
        };
        while let Some(digit) = bytes.get(i).copied().filter(u8::is_ascii_digit) {
            push(&mut mantissa, digit)?;
            digits += 1;
            i += 1;
        }
        if bytes.get(i) == Some(&b'.') {
            i += 1;
            while let Some(digit) = bytes.get(i).copied().filter(u8::is_ascii_digit) {
                push(&mut mantissa, digit)?;
                digits += 1;
                scale += 1;
                i += 1;
            }
        }
        if digits == 0 {
            return None;
        }
        if matches!(bytes.get(i), Some(b'e' | b'E')) {
            let mut j = i + 1;
            let exponent_negative = match bytes.get(j) {
                Some(b'-') => {
                    j += 1;
                    true
                }
                Some(b'+') => {
                    j += 1;
                    false
                }
                _ => false,
            };
            let start = j;
            let mut exponent: i64 = 0;
            while let Some(digit) = bytes.get(j).copied().filter(u8::is_ascii_digit) {
                exponent = exponent.checked_mul(10)?.checked_add(i64::from(digit - b'0'))?;
                j += 1;
            }
            // An `e` without digits after it belongs to what follows, and no path letter is `e`.
            if j == start {
                return None;
            }
            scale -= if exponent_negative { -exponent } else { exponent };
            i = j;
        }
        while scale < 0 {
            mantissa = mantissa.checked_mul(10)?;
            scale += 1;
        }
        let scale = u32::try_from(scale).ok().filter(|scale| *scale <= MAX_SCALE)?;
        self.rest = self.rest.get(i..)?;
        Some(Decimal { mantissa: if negative { -mantissa } else { mantissa }, scale })
    }

    /// An arc's flag: one `0` or `1`, which may stand right before what follows it.
    fn flag(&mut self) -> Option<bool> {
        self.skip_separators();
        let flag = match self.rest.chars().next()? {
            '0' => false,
            '1' => true,
            _ => return None,
        };
        self.rest = self.rest.get(1..)?;
        Some(flag)
    }

    fn point(&mut self) -> Option<Point> {
        Some(Point { x: self.number()?, y: self.number()? })
    }
}

#[derive(Clone, Copy, Debug)]
struct Point {
    x: Decimal,
    y: Decimal,
}

impl Point {
    const ORIGIN: Point = Point { x: Decimal::ZERO, y: Decimal::ZERO };

    fn add(self, other: Point) -> Option<Point> {
        Some(Point { x: self.x.add(other.x)?, y: self.y.add(other.y)? })
    }

    fn sub(self, other: Point) -> Option<Point> {
        Some(Point { x: self.x.sub(other.x)?, y: self.y.sub(other.y)? })
    }

    fn same(self, other: Point) -> bool {
        self.x.same(other.x) && self.y.same(other.y)
    }
}

/// A segment of a path, every point absolute.
#[derive(Clone, Copy, Debug)]
enum Segment {
    Move(Point),
    Line(Point),
    Cubic(Point, Point, Point),
    SmoothCubic(Point, Point),
    Quadratic(Point, Point),
    SmoothQuadratic(Point),
    Arc { radii: (Decimal, Decimal), rotation: Decimal, large: bool, sweep: bool, to: Point },
    Close,
}

impl Segment {
    fn same(&self, other: &Segment) -> bool {
        use Segment::*;
        match (self, other) {
            (Move(a), Move(b)) | (Line(a), Line(b)) | (SmoothQuadratic(a), SmoothQuadratic(b)) => a.same(*b),
            (Cubic(a1, a2, a), Cubic(b1, b2, b)) => a1.same(*b1) && a2.same(*b2) && a.same(*b),
            (SmoothCubic(a2, a), SmoothCubic(b2, b)) | (Quadratic(a2, a), Quadratic(b2, b)) => a2.same(*b2) && a.same(*b),
            (Arc { radii: ra, rotation: oa, large: la, sweep: sa, to: a }, Arc { radii: rb, rotation: ob, large: lb, sweep: sb, to: b }) => {
                ra.0.same(rb.0) && ra.1.same(rb.1) && oa.same(*ob) && la == lb && sa == sb && a.same(*b)
            }
            (Close, Close) => true,
            _ => false,
        }
    }
}

/// Path data as its segments, every point absolute; `None` for what does not follow the grammar.
fn segments(data: &str) -> Option<Vec<Segment>> {
    let mut cursor = Cursor { rest: data };
    let mut segments = Vec::new();
    let (mut current, mut start) = (Point::ORIGIN, Point::ORIGIN);
    let mut command: Option<char> = None;
    while !cursor.at_end() {
        if let Some(letter) = cursor.command() {
            command = Some(letter);
        }
        let letter = command?;
        let relative = letter.is_ascii_lowercase();
        let at = |p: Point| if relative { current.add(p) } else { Some(p) };
        let segment = match letter.to_ascii_uppercase() {
            'M' => {
                let to = at(cursor.point()?)?;
                start = to;
                // Pairs after a move are lines.
                command = Some(if relative { 'l' } else { 'L' });
                Segment::Move(to)
            }
            'L' => Segment::Line(at(cursor.point()?)?),
            'H' => {
                let x = cursor.number()?;
                Segment::Line(Point { x: if relative { current.x.add(x)? } else { x }, y: current.y })
            }
            'V' => {
                let y = cursor.number()?;
                Segment::Line(Point { x: current.x, y: if relative { current.y.add(y)? } else { y } })
            }
            'C' => Segment::Cubic(at(cursor.point()?)?, at(cursor.point()?)?, at(cursor.point()?)?),
            'S' => Segment::SmoothCubic(at(cursor.point()?)?, at(cursor.point()?)?),
            'Q' => Segment::Quadratic(at(cursor.point()?)?, at(cursor.point()?)?),
            'T' => Segment::SmoothQuadratic(at(cursor.point()?)?),
            'A' => {
                let radii = (cursor.number()?, cursor.number()?);
                let rotation = cursor.number()?;
                let (large, sweep) = (cursor.flag()?, cursor.flag()?);
                Segment::Arc { radii, rotation, large, sweep, to: at(cursor.point()?)? }
            }
            // 'Z': nothing may follow it without a letter of its own.
            _ => {
                command = None;
                Segment::Close
            }
        };
        current = match segment {
            Segment::Move(to) | Segment::Line(to) | Segment::Cubic(_, _, to) | Segment::SmoothCubic(_, to) | Segment::Quadratic(_, to) | Segment::SmoothQuadratic(to) | Segment::Arc { to, .. } => to,
            Segment::Close => start,
        };
        segments.push(segment);
    }
    Some(segments)
}

/// Numbers written one after the other with only the separators the grammar needs: none before
/// a minus, none before a point when the number before has one already.
#[derive(Default)]
struct Writer {
    out: String,
    /// The number just written (`None` after a letter): whether it has a point.
    last_has_point: Option<bool>,
}

impl Writer {
    fn letter(&mut self, letter: char) {
        self.out.push(letter);
        self.last_has_point = None;
    }

    fn number(&mut self, number: &str) {
        if let Some(had_point) = self.last_has_point {
            let glued = number.starts_with('-') || (number.starts_with('.') && had_point);
            if !glued {
                self.out.push(' ');
            }
        }
        self.out.push_str(number);
        self.last_has_point = Some(number.contains('.'));
    }
}

/// One way to write a segment: its letter and its numbers.
struct Form {
    letter: char,
    numbers: Vec<String>,
}

fn texts(points: &[Point]) -> Vec<String> {
    points.iter().flat_map(|p| [p.x.text(), p.y.text()]).collect()
}

/// The ways to write `segment` from `current`: relative and absolute, a line along an axis also
/// with `h`/`H` or `v`/`V`. The relative one comes first and wins a tie: its small numbers repeat,
/// which compression likes.
fn forms(segment: &Segment, current: Point) -> Option<Vec<Form>> {
    let rel = |p: Point| p.sub(current);
    let pair = |upper: char, points: &[Point]| -> Option<Vec<Form>> {
        let relative: Option<Vec<Point>> = points.iter().map(|p| rel(*p)).collect();
        Some(vec![Form { letter: upper.to_ascii_lowercase(), numbers: texts(&relative?) }, Form { letter: upper, numbers: texts(points) }])
    };
    Some(match *segment {
        Segment::Move(to) => pair('M', &[to])?,
        Segment::Line(to) => {
            let mut forms = pair('L', &[to])?;
            if to.y.same(current.y) {
                forms.push(Form { letter: 'h', numbers: vec![to.x.sub(current.x)?.text()] });
                forms.push(Form { letter: 'H', numbers: vec![to.x.text()] });
            }
            if to.x.same(current.x) {
                forms.push(Form { letter: 'v', numbers: vec![to.y.sub(current.y)?.text()] });
                forms.push(Form { letter: 'V', numbers: vec![to.y.text()] });
            }
            forms
        }
        Segment::Cubic(c1, c2, to) => pair('C', &[c1, c2, to])?,
        Segment::SmoothCubic(c2, to) => pair('S', &[c2, to])?,
        Segment::Quadratic(c1, to) => pair('Q', &[c1, to])?,
        Segment::SmoothQuadratic(to) => pair('T', &[to])?,
        Segment::Arc { radii, rotation, large, sweep, to } => {
            let head = [radii.0.text(), radii.1.text(), rotation.text(), u8::from(large).to_string(), u8::from(sweep).to_string()];
            let with = |letter: char, end: Point| Form { letter, numbers: head.iter().cloned().chain(texts(&[end])).collect() };
            vec![with('a', rel(to)?), with('A', to)]
        }
        Segment::Close => vec![Form { letter: 'z', numbers: Vec::new() }],
    })
}

/// Path data as short as it goes, or `None` when it cannot be read or written back exactly.
fn path(data: &str) -> Option<String> {
    let original = segments(data)?;
    let mut writer = Writer::default();
    // The letter a segment may leave out: the one before it, or after a move the line it implies.
    let mut implied: Option<char> = None;
    let (mut current, mut start) = (Point::ORIGIN, Point::ORIGIN);
    for (i, segment) in original.iter().enumerate() {
        let mut best: Option<(usize, Writer, char)> = None;
        for form in forms(segment, current)? {
            // A path starts with its move written out, as absolute as it is.
            if i == 0 && form.letter != 'M' {
                continue;
            }
            // A move is never implied by the line before it.
            let omit = implied == Some(form.letter) && !matches!(segment, Segment::Move(_));
            let mut trial = Writer { out: String::new(), last_has_point: writer.last_has_point };
            if !omit {
                trial.letter(form.letter);
            }
            for number in &form.numbers {
                trial.number(number);
            }
            if best.as_ref().is_none_or(|(length, ..)| trial.out.len() < *length) {
                best = Some((trial.out.len(), trial, form.letter));
            }
        }
        let (_, trial, letter) = best?;
        writer.out.push_str(&trial.out);
        writer.last_has_point = trial.last_has_point;
        implied = match letter {
            'M' => Some('L'),
            'm' => Some('l'),
            'z' | 'Z' => None,
            letter => Some(letter),
        };
        current = match *segment {
            Segment::Move(to) => {
                start = to;
                to
            }
            Segment::Line(to) | Segment::Cubic(_, _, to) | Segment::SmoothCubic(_, to) | Segment::Quadratic(_, to) | Segment::SmoothQuadratic(to) | Segment::Arc { to, .. } => to,
            Segment::Close => start,
        };
    }
    // Written back, the path has to be the one it was, point for point.
    let written = segments(&writer.out)?;
    let same = written.len() == original.len() && written.iter().zip(&original).all(|(a, b)| a.same(b));
    (same && writer.out.len() <= data.len()).then_some(writer.out)
}

/// The numbers of a list, each as short as it goes.
fn numbers(value: &str) -> Option<Vec<String>> {
    let mut cursor = Cursor { rest: value };
    let mut numbers = Vec::new();
    while !cursor.at_end() {
        numbers.push(cursor.number()?.text());
    }
    Some(numbers)
}

fn single_number(value: &str) -> Option<String> {
    match numbers(value)?.as_slice() {
        [number] => Some(number.clone()),
        _ => None,
    }
}

/// A transform list with its numbers as short as they go and a space between them, as the SVG
/// grammar asks for (`translate(-.7 -7.5) rotate(-38.4)`).
fn transforms(value: &str) -> Option<String> {
    let mut out = Vec::new();
    let mut rest = value.trim();
    while !rest.is_empty() {
        let (name, after) = rest.split_once('(')?;
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        let (arguments, after) = after.split_once(')')?;
        out.push(format!("{name}({})", numbers(arguments)?.join(" ")));
        rest = after.trim_start();
        rest = rest.strip_prefix(',').unwrap_or(rest).trim_start();
    }
    Some(out.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_lose_what_does_not_count() {
        for (written, short) in [("0.70", ".7"), ("-0.078", "-.078"), ("1200", "1200"), ("12.50", "12.5"), ("-0", "0"), ("+5", "5"), ("1e2", "100"), ("2.5E-3", ".0025"), ("007", "7"), ("-.5", "-.5")] {
            assert_eq!(single_number(written).as_deref(), Some(short), "{written}");
        }
        for not_a_number in ["", "1px", "50%", "1 2", "e5", "."] {
            assert_eq!(single_number(not_a_number), None, "{not_a_number}");
        }
    }

    #[test]
    fn a_path_is_written_shorter_and_stays_the_same() {
        // The shape of the birch's leaves: absolute lines of one decimal each.
        assert_eq!(path("M0 34L9.8 36.6L15.8 39.2L20.7 41.8Z").as_deref(), Some("M0 34l9.8 2.6 6 2.6 4.9 2.6z"));
        // Lines along an axis, and a close after which a relative move starts at the start.
        assert_eq!(path("M0 7h13v3H0zM23 12h9v3h-9z").as_deref(), Some("M0 7h13v3H0zm23 5h9v3h-9z"));
        // Curves keep their kind; a point before a point needs no space when the first has one.
        assert_eq!(path("M 10 10 C 20 20, 40 20, 50 10 S 80 0, 90 10 Q 95 20 100 10 T 120 10").as_deref(), Some("M10 10c10 10 30 10 40 0s30-10 40 0q5 10 10 0t20 0"));
        assert_eq!(path("M0 0L0.5 0.5L1 0.5").as_deref(), Some("M0 0l.5.5H1"));
        // Arcs keep their flags apart from the numbers around them.
        assert_eq!(path("M10 10A5 5 0 0 1 20 20a5,5 30 1,0 10 0").as_deref(), Some("M10 10a5 5 0 0 1 10 10 5 5 30 1 0 10 0"));
        // A dot drawn with round caps is a line of no length, and stays one.
        assert_eq!(path("M5 5L5 5").as_deref(), Some("M5 5h0"));
    }

    #[test]
    fn what_is_not_path_data_stays_as_it_was() {
        for data in ["", "10 10", "M10", "M10 10 Z 5", "M1e 2", "M10 10 A5 5 0 2 1 20 20"] {
            assert_eq!(path(data).filter(|short| short != data), None, "{data}");
        }
        assert_eq!(minify("<svg><path d=\"M10\"/></svg>"), "<svg><path d=\"M10\"/></svg>");
    }

    #[test]
    fn every_path_written_is_read_back_the_same() {
        // What `path` promises for any input: its answer draws the same segments.
        for data in ["M0 0l1.25-3.5 2e1 .5zm-1-1H3V-4", "m1 1 2 2 3 3", "M1 1 2 2L3 3l4 4h5v6", "M0,0 Q1,1 2,0 T4,0 t2 0", "M.5.5-.5-.5"] {
            let short = path(data).unwrap_or_else(|| data.to_string());
            let (a, b) = (segments(data).unwrap_or_default(), segments(&short).unwrap_or_default());
            assert!(a.len() == b.len() && a.iter().zip(&b).all(|(a, b)| a.same(b)), "{data} → {short}");
        }
    }

    #[test]
    fn transforms_keep_a_space_between_their_numbers() {
        assert_eq!(transforms("translate(-0.7 -7.5) rotate(-38.4) scale(-0.078 0.087)").as_deref(), Some("translate(-.7 -7.5) rotate(-38.4) scale(-.078 .087)"));
        assert_eq!(transforms("matrix(1,0,0,1,0.5,0.50)").as_deref(), Some("matrix(1 0 0 1 .5 .5)"));
        assert_eq!(transforms("rotate(1) (2)"), None);
    }

    #[test]
    fn a_file_keeps_its_text_ids_quotes_and_colours() {
        let svg = "<?xml version=\"1.0\"?>\n<!-- drawn by hand -->\n<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 32.0 32\">\n  <defs><g id=\"lf-a\"><path d=\"M0 0L0 10\" stroke=\"#000\" stroke-width=\"6.0\"/></g></defs>\n  <use href=\"#lf-a\" transform=\"translate(1.50 2)\"/>\n  <text x=\"0.5\" y='1'> a  b </text>\n</svg>\n";
        assert_eq!(
            minify(svg),
            "<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 32 32\"><defs><g id=\"lf-a\"><path d=\"M0 0v10\" stroke=\"#000\" stroke-width=\"6\"/></g></defs><use href=\"#lf-a\" transform=\"translate(1.5 2)\"/><text x=\".5\" y='1'> a  b </text></svg>"
        );
    }
}
