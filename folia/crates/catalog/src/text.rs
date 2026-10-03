//! The free texts of a module — learning outcomes, contents, assessment, remarks, prerequisites —
//! are Markdown since schema 10 (docs/radix/schema-v2.md §3, „Module texts"): Radix writes them from the
//! module's page (`radix/internal/parser/markdown.go`). Here they are read into what a page sets of
//! them: paragraphs, lists, strong and emphasized text, line breaks (`blocks`), and into one line
//! of plain text for what describes a page to others (`plain`).
//!
//! The reader reads the CommonMark Radix writes, by CommonMark's rules: paragraphs apart by a
//! blank line; lists („-", „*", „+"; „1.", „1)"), nested by how far their lines are indented;
//! `**strong**` and `*emphasized*` text; a backslash (or two spaces) at the end of a line for a
//! line break, and a backslash before punctuation for the character itself. Radix escapes everything else CommonMark would
//! read as markup, so where a text holds it anyway it is text as it stands — a „#", a „<b>", a
//! „[link](…)": the texts come from the university's pages, and a page of Betula sets text, never
//! markup taken from elsewhere.
//!
//! A list whose items all begin with a label („(1)", „a)", „IV.", „3.1.") is one CommonMark can
//! only write as bullets (it numbers with digits alone); its labels are its markers
//! (`ListKind::Labels`).

/// A block of a text: a paragraph or a list.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    List(List),
}

#[derive(Clone, Debug, PartialEq)]
pub struct List {
    pub kind: ListKind,
    pub items: Vec<Item>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ListKind {
    Bullets,
    /// Numbered from this number.
    Numbers(u64),
    /// Each item's label is its marker (`Item::label`).
    Labels,
}

/// An item of a list: its blocks, a paragraph first as a rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub label: Option<String>,
    pub blocks: Vec<Block>,
}

/// What a paragraph is made of.
#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    /// A line break the text keeps.
    Break,
}

/// The blocks of a text.
pub fn blocks(markdown: &str) -> Vec<Block> {
    let mut reader = Reader::default();
    for line in markdown.lines() {
        reader.line(Line::new(line));
    }
    reader.finish()
}

/// A text as one line: its paragraphs one after the other, the items of a list apart by „·".
/// For a page's description and what a search engine shows of it.
pub fn plain(markdown: &str) -> String {
    plain_blocks(&blocks(markdown)).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn plain_blocks(blocks: &[Block]) -> String {
    let parts: Vec<String> = blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph(inlines) => plain_inlines(inlines),
            Block::List(list) => list
                .items
                .iter()
                .map(|item| {
                    let text = plain_blocks(&item.blocks);
                    item.label.as_ref().map_or_else(|| text.clone(), |label| format!("{label} {text}"))
                })
                .collect::<Vec<_>>()
                .join(" · "),
        })
        .filter(|part| !part.trim().is_empty())
        .collect();
    parts.join(" ")
}

fn plain_inlines(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .map(|inline| match inline {
            Inline::Text(text) => text.clone(),
            Inline::Strong(inner) | Inline::Emphasis(inner) => plain_inlines(inner),
            Inline::Break => " ".to_string(),
        })
        .collect()
}

/// A line of a text: the column its text begins at, and the text.
#[derive(Clone, Copy)]
struct Line<'a> {
    indent: usize,
    text: &'a str,
}

impl<'a> Line<'a> {
    fn new(line: &'a str) -> Self {
        // A tab indents to the next multiple of four columns.
        let indent = line.chars().take_while(|c| matches!(c, ' ' | '\t')).fold(0, |column, c| if c == '\t' { column + 4 - column % 4 } else { column + 1 });
        Line { indent, text: line.trim_start_matches([' ', '\t']) }
    }

    fn blank(&self) -> bool {
        self.text.trim_end_matches([' ', '\t']).is_empty()
    }
}

/// Reads a text line by line, as CommonMark does: the items a line is in are those it is indented
/// to, each with the list it is an item of; a blank line ends a paragraph, not an item.
#[derive(Default)]
struct Reader<'a> {
    /// The text's blocks so far.
    blocks: Vec<Block>,
    /// The items open, the outermost first.
    items: Vec<Open>,
    /// The lines of the paragraph at hand, in the innermost item.
    paragraph: Vec<&'a str>,
}

/// An item open, and its list so far.
struct Open {
    /// The list's bullet, or the character after its numbers.
    kind: char,
    start: Option<u64>,
    /// The column the item's text begins at.
    column: usize,
    /// Whether the item is empty for good: no line goes into it, though its list may go on.
    ended: bool,
    /// The list's items before this one.
    items: Vec<Item>,
    blocks: Vec<Block>,
}

impl<'a> Reader<'a> {
    fn line(&mut self, line: Line<'a>) {
        if line.blank() {
            self.end_paragraph();
            // An item may begin with one blank line, not two: one still empty is.
            if let Some(open) = self.items.last_mut().filter(|open| open.blocks.is_empty()) {
                open.ended = true;
            }
            return;
        }
        let inside = self.items.iter().take_while(|item| !item.ended && line.indent >= item.column).count();
        let base = inside.checked_sub(1).and_then(|n| self.items.get(n)).map_or(0, |item| item.column);
        let marker = Marker::of(line, base);
        // A line goes on with the paragraph at hand unless it begins a list that may interrupt
        // it; one indented less than the paragraph's item (CommonMark's lazy continuation line)
        // unless it begins an item at all.
        let goes_on = if inside == self.items.len() { !marker.is_some_and(|marker| marker.interrupts()) } else { marker.is_none() };
        if !self.paragraph.is_empty() && goes_on {
            self.paragraph.push(line.text);
            return;
        }
        self.end_paragraph();
        let Some(marker) = marker else {
            self.end_items(inside);
            self.paragraph.push(line.text);
            return;
        };
        self.end_items(inside + 1);
        match self.items.get_mut(inside) {
            Some(open) if open.kind == marker.kind => {
                let blocks = std::mem::take(&mut open.blocks);
                open.items.push(Item { label: None, blocks });
                open.column = marker.column;
                open.ended = false;
            }
            _ => {
                self.end_items(inside);
                self.items.push(Open { kind: marker.kind, start: marker.number, column: marker.column, ended: false, items: Vec::new(), blocks: Vec::new() });
            }
        }
        if !marker.rest.blank() {
            self.line(marker.rest);
        }
    }

    /// The blocks of the innermost item, or the text's.
    fn container(&mut self) -> &mut Vec<Block> {
        match self.items.last_mut() {
            Some(open) => &mut open.blocks,
            None => &mut self.blocks,
        }
    }

    fn end_paragraph(&mut self) {
        let lines = std::mem::take(&mut self.paragraph);
        let inlines = inlines(&lines);
        if !inlines.is_empty() {
            self.container().push(Block::Paragraph(inlines));
        }
    }

    /// Ends the items open from the `depth`-th on, and their lists with them.
    fn end_items(&mut self, depth: usize) {
        while self.items.len() > depth {
            if let Some(open) = self.items.pop() {
                let mut items = open.items;
                items.push(Item { label: None, blocks: open.blocks });
                let list = labelled(open.start, items);
                self.container().push(Block::List(list));
            }
        }
    }

    fn finish(mut self) -> Vec<Block> {
        self.end_paragraph();
        self.end_items(0);
        self.blocks
    }
}

/// How a line begins an item of a list.
#[derive(Clone, Copy)]
struct Marker<'a> {
    /// The bullet, or the character after the number.
    kind: char,
    number: Option<u64>,
    /// The column the item's text begins at: its further lines are indented to it.
    column: usize,
    /// The rest of the line, the item's first.
    rest: Line<'a>,
}

impl<'a> Marker<'a> {
    /// The marker a line begins with, indented less than four columns beyond `base`: „-", „*" or
    /// „+", or up to nine digits and „." or „)", followed by a space or the end of the line.
    fn of(line: Line<'a>, base: usize) -> Option<Self> {
        if !(base..base + 4).contains(&line.indent) {
            return None;
        }
        let digits = line.text.bytes().take_while(u8::is_ascii_digit).count();
        let (number, rest) = line.text.split_at_checked(digits)?;
        let mut chars = rest.chars();
        let kind = chars.next()?;
        let number = match digits {
            0 if matches!(kind, '-' | '*' | '+') => None,
            1..=9 if matches!(kind, '.' | ')') => Some(number.parse().ok()?),
            _ => return None,
        };
        let after = chars.as_str();
        let text = after.trim_start_matches([' ', '\t']);
        if text.len() == after.len() && !text.is_empty() {
            return None;
        }
        let start = line.indent + digits + 1;
        let spaces = after.chars().take_while(|c| matches!(c, ' ' | '\t')).fold(start, |column, c| if c == '\t' { column + 4 - column % 4 } else { column + 1 }) - start;
        // The text begins after the spaces; after none, or more than four, one column after the
        // marker, and the rest is indented in the item.
        let column = if text.is_empty() || spaces > 4 { start + 1 } else { start + spaces };
        Some(Marker { kind, number, column, rest: Line { indent: start + spaces, text } })
    }

    /// Whether the item may interrupt a paragraph: a bullet or the number 1, with text.
    fn interrupts(&self) -> bool {
        !self.rest.blank() && self.number.is_none_or(|number| number == 1)
    }
}

/// A paragraph's text before its runs of „*" are matched.
enum Token {
    Text(String),
    Break,
    /// A run of „*": how long it is, how many of it are left, whether it can open and close.
    Run { length: usize, left: usize, open: bool, close: bool },
    Strong(Vec<Token>),
    Emphasis(Vec<Token>),
}

/// The inlines of a paragraph's lines. A backslash at the end of a line breaks it, and so do two
/// spaces, which go with any other white space there; else the line goes on after a space. A
/// backslash before punctuation is the character itself.
fn inlines(lines: &[&str]) -> Vec<Inline> {
    let mut tokens = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        let last = n + 1 == lines.len();
        let mut chars = line.chars().peekable();
        // The character before the one at hand; a line's start counts as white space.
        let mut before = '\n';
        let mut broken = false;
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next_if(char::is_ascii_punctuation) {
                    Some(escaped) => {
                        push_char(&mut tokens, escaped);
                        before = escaped;
                        continue;
                    }
                    None if chars.peek().is_none() && !last => broken = true,
                    None => push_char(&mut tokens, '\\'),
                },
                '*' => {
                    let mut length = 1;
                    while chars.next_if_eq(&'*').is_some() {
                        length += 1;
                    }
                    let after = chars.peek().copied().unwrap_or('\n');
                    tokens.push(Token::Run { length, left: length, open: left_flanking(before, after), close: right_flanking(before, after) });
                }
                c => push_char(&mut tokens, c),
            }
            before = c;
        }
        let mut spaces = 0;
        if let Some(Token::Text(text)) = tokens.last_mut().filter(|_| !broken) {
            spaces = text.len() - text.trim_end_matches(' ').len();
            text.truncate(text.trim_end_matches([' ', '\t']).len());
            if text.is_empty() {
                tokens.pop();
            }
        }
        if !last {
            if broken || spaces >= 2 {
                tokens.push(Token::Break);
            } else {
                push_char(&mut tokens, ' ');
            }
        }
    }
    trim(into_inlines(emphasize(tokens)))
}

fn push_char(tokens: &mut Vec<Token>, c: char) {
    match tokens.last_mut() {
        Some(Token::Text(text)) => text.push(c),
        _ => tokens.push(Token::Text(c.to_string())),
    }
}

/// CommonMark's emphasis: every run of „*" that can close, from the first on, is matched with the
/// nearest run before it that can open — two of each where both have two left (strong), one
/// else (emphasized) —, and what stands between them goes inside. A run that can both open and
/// close matches none whose length makes a multiple of three with its own, unless both are.
/// What is left of a run is text.
fn emphasize(mut tokens: Vec<Token>) -> Vec<Token> {
    let mut at = 0;
    while let Some(token) = tokens.get(at) {
        let &Token::Run { length, left, open, close: true } = token else {
            at += 1;
            continue;
        };
        let opener = (0..at).rev().find_map(|before| match tokens.get(before) {
            Some(&Token::Run { length: opener_length, left: opener_left, open: true, close: opener_close })
                if !((opener_close || open) && (opener_length + length) % 3 == 0 && !(opener_length % 3 == 0 && length % 3 == 0)) =>
            {
                Some((before, opener_left))
            }
            _ => None,
        });
        let Some((opener, opener_left)) = opener else {
            at += 1;
            continue;
        };
        let used = if opener_left >= 2 && left >= 2 { 2 } else { 1 };
        let inner: Vec<Token> = tokens.drain(opener + 1..at).collect();
        tokens.insert(opener + 1, if used == 2 { Token::Strong(inner) } else { Token::Emphasis(inner) });
        // The opener, what it opens, the closer; a run used up goes, one that is not closes again.
        at = opener + 2;
        for run in [at, opener] {
            if let Some(Token::Run { left, .. }) = tokens.get_mut(run) {
                *left -= used;
                if *left == 0 {
                    tokens.remove(run);
                    at -= usize::from(run == opener);
                }
            }
        }
    }
    tokens
}

/// The inlines of matched tokens: what is left of a run is its „*", texts side by side are one.
fn into_inlines(tokens: Vec<Token>) -> Vec<Inline> {
    let mut inlines: Vec<Inline> = Vec::new();
    for token in tokens {
        let inline = match token {
            Token::Text(text) => Inline::Text(text),
            Token::Run { left, .. } => Inline::Text("*".repeat(left)),
            Token::Break => Inline::Break,
            Token::Strong(inner) => Inline::Strong(into_inlines(inner)),
            Token::Emphasis(inner) => Inline::Emphasis(into_inlines(inner)),
        };
        match (inlines.last_mut(), inline) {
            (Some(Inline::Text(before)), Inline::Text(text)) => before.push_str(&text),
            (_, inline) => inlines.push(inline),
        }
    }
    inlines
}

/// A run of „*" that can open: no white space after it, and no punctuation unless there is
/// white space or punctuation before it.
fn left_flanking(before: char, after: char) -> bool {
    !space(after) && (!punctuation(after) || space(before) || punctuation(before))
}

/// A run of „*" that can close: the same, the other way round.
fn right_flanking(before: char, after: char) -> bool {
    !space(before) && (!punctuation(before) || space(after) || punctuation(after))
}

/// White space as CommonMark counts it: Unicode's spaces (Zs), tab, line feed, form feed and
/// carriage return.
fn space(c: char) -> bool {
    matches!(u32::from(c), 0x09 | 0x0A | 0x0C | 0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x202F | 0x205F | 0x3000)
}

/// Punctuation as CommonMark counts it: Unicode's punctuation and symbols. Beyond ASCII it is
/// taken as what is no letter, digit, white space or control character, nor one of the marks,
/// format and private characters a text in Latin script carries: combining accents, the soft
/// hyphen, zero-width characters, variation selectors, a symbol font's characters. That is
/// Unicode's categories but for the marks of other scripts and a few letters in circles („Ⓐ").
fn punctuation(c: char) -> bool {
    match u32::from(c) {
        0..=0x7F => c.is_ascii_punctuation(),
        0xAD | 0x300..=0x36F | 0x200B..=0x200F | 0x2028..=0x202E | 0x2060..=0x206F | 0x20D0..=0x20FF | 0xFE00..=0xFE0F | 0xFEFF | 0xE000..=0xF8FF => false,
        _ => !(c.is_alphanumeric() || c.is_control() || space(c)),
    }
}

/// White space at a paragraph's ends goes, and so does a line break there.
fn trim(mut inlines: Vec<Inline>) -> Vec<Inline> {
    while matches!(inlines.last(), Some(Inline::Break)) {
        inlines.pop();
    }
    while matches!(inlines.first(), Some(Inline::Break)) {
        inlines.remove(0);
    }
    if let Some(Inline::Text(text)) = inlines.first_mut() {
        *text = text.trim_start().to_string();
    }
    if let Some(Inline::Text(text)) = inlines.last_mut() {
        *text = text.trim_end().to_string();
    }
    inlines.retain(|inline| !matches!(inline, Inline::Text(text) if text.is_empty()));
    inlines
}

/// The list a read list is: a bullet list whose items all begin with a label of one kind is a
/// list of labels (Radix writes „- (a) Absorption"), the label taken off the item's text.
fn labelled(start: Option<u64>, mut items: Vec<Item>) -> List {
    if let Some(start) = start {
        return List { kind: ListKind::Numbers(start), items };
    }
    let labels: Vec<Option<(String, LabelKind)>> = items.iter().map(label_of).collect();
    let first = labels.first().and_then(|label| label.as_ref().map(|(_, kind)| *kind));
    let all = items.len() >= 2 && first.is_some() && labels.iter().all(|label| label.as_ref().map(|(_, kind)| *kind) == first);
    if !all {
        return List { kind: ListKind::Bullets, items };
    }
    for (item, label) in items.iter_mut().zip(labels) {
        if let (Some((label, _)), Some(Block::Paragraph(inlines))) = (label, item.blocks.first_mut()) {
            if let Some(Inline::Text(text)) = inlines.first_mut() {
                *text = text.get(label.len()..).unwrap_or_default().trim_start().to_string();
            }
            if matches!(inlines.first(), Some(Inline::Text(text)) if text.is_empty()) {
                inlines.remove(0);
            }
            item.label = Some(label);
        }
    }
    List { kind: ListKind::Labels, items }
}

/// The kind of a label: how it is written, never what it counts.
#[derive(Clone, Copy, PartialEq)]
enum LabelKind {
    /// „(1)", „(a)", „(iv)"
    Brackets(Counter),
    /// „1)", „a.", „IV."
    Closed(Counter, char),
    /// „3.1", „3.1."
    Section,
}

#[derive(Clone, Copy, PartialEq)]
enum Counter {
    Digits,
    Lower,
    Upper,
}

/// The label an item's text begins with, followed by a space.
fn label_of(item: &Item) -> Option<(String, LabelKind)> {
    let Some(Block::Paragraph(inlines)) = item.blocks.first() else { return None };
    let Some(Inline::Text(text)) = inlines.first() else { return None };
    let token = text.split(' ').next()?;
    if token.len() == text.len() {
        return None;
    }
    let kind = label_kind(token)?;
    Some((token.to_string(), kind))
}

fn label_kind(token: &str) -> Option<LabelKind> {
    if let Some(inner) = token.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')) {
        return counter(inner).map(LabelKind::Brackets);
    }
    let digits = |s: &str| !s.is_empty() && s.len() <= 2 && s.bytes().all(|b| b.is_ascii_digit());
    if let Some((major, minor)) = token.strip_suffix('.').unwrap_or(token).split_once('.') {
        return (digits(major) && digits(minor)).then_some(LabelKind::Section);
    }
    let (head, delimiter) = match (token.strip_suffix('.'), token.strip_suffix(')')) {
        (Some(head), _) => (head, '.'),
        (_, Some(head)) => (head, ')'),
        _ => return None,
    };
    counter(head).map(|counter| LabelKind::Closed(counter, delimiter))
}

/// What counts in a label: digits, a letter, a Roman numeral.
fn counter(token: &str) -> Option<Counter> {
    let roman = |s: &str, numerals: &str| !s.is_empty() && s.len() <= 6 && s.chars().all(|c| numerals.contains(c));
    let mut chars = token.chars();
    match (chars.next(), chars.next()) {
        _ if !token.is_empty() && token.len() <= 2 && token.bytes().all(|b| b.is_ascii_digit()) => Some(Counter::Digits),
        (Some(c), None) if c.is_ascii_lowercase() => Some(Counter::Lower),
        (Some(c), None) if c.is_ascii_uppercase() => Some(Counter::Upper),
        _ if roman(token, "ivx") => Some(Counter::Lower),
        _ if roman(token, "IVX") => Some(Counter::Upper),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Inline {
        Inline::Text(s.to_string())
    }

    fn paragraph(inlines: Vec<Inline>) -> Block {
        Block::Paragraph(inlines)
    }

    fn item(label: Option<&str>, blocks: Vec<Block>) -> Item {
        Item { label: label.map(str::to_string), blocks }
    }

    fn bullets(items: &[&str]) -> Block {
        Block::List(List { kind: ListKind::Bullets, items: items.iter().map(|it| item(None, vec![paragraph(vec![text(it)])])).collect() })
    }

    #[test]
    fn paragraphs_and_line_breaks() {
        assert_eq!(
            blocks("Die Studierenden sollen\nlernen.\n\n**Lecture:**\\\nThe lecture deals with *chemicals*."),
            vec![
                paragraph(vec![text("Die Studierenden sollen lernen.")]),
                paragraph(vec![Inline::Strong(vec![text("Lecture:")]), Inline::Break, text("The lecture deals with "), Inline::Emphasis(vec![text("chemicals")]), text(".")]),
            ]
        );
        // A backslash at the end of a paragraph is one, and so is one before what is no punctuation.
        assert_eq!(blocks("C:\\Daten\\\n\nEnde\\"), vec![paragraph(vec![text("C:\\Daten\\")]), paragraph(vec![text("Ende\\")])]);
    }

    #[test]
    fn lists_nest_and_number_from_their_start() {
        let markdown = "1. Drei Präsentationen (45%):\n   - Themen\n   - Fortschritt\n\n   (jeweils 5 Punkte)\n2. Seminararbeit";
        assert_eq!(
            blocks(markdown),
            vec![Block::List(List {
                kind: ListKind::Numbers(1),
                items: vec![
                    item(None, vec![paragraph(vec![text("Drei Präsentationen (45%):")]), bullets(&["Themen", "Fortschritt"]), paragraph(vec![text("(jeweils 5 Punkte)")])]),
                    item(None, vec![paragraph(vec![text("Seminararbeit")])]),
                ],
            })]
        );
        assert_eq!(blocks("3. drei\n4. vier").first().map(|b| matches!(b, Block::List(List { kind: ListKind::Numbers(3), .. }))), Some(true));
        // Radix writes the items of a list one under the other, its lines indented to the item's
        // text: „10. " takes four columns.
        assert_eq!(
            blocks("9. neun\n10. zehn,\n    weiter"),
            vec![Block::List(List { kind: ListKind::Numbers(9), items: vec![item(None, vec![paragraph(vec![text("neun")])]), item(None, vec![paragraph(vec![text("zehn, weiter")])])] })]
        );
    }

    #[test]
    fn a_list_goes_on_with_markers_of_its_kind() {
        // Two lists in a row take different markers (Radix alternates them), a blank line between
        // items keeps the list, and an item indented less than its sister's text is her sister.
        assert_eq!(blocks("- a\n- b\n\n* c"), vec![bullets(&["a", "b"]), bullets(&["c"])]);
        assert_eq!(blocks("1. a\n\n2. b\n1) c").len(), 2);
        assert_eq!(blocks("- a\n\n- b\n - c"), vec![bullets(&["a", "b", "c"])]);
        assert_eq!(blocks("- a\n  - b"), vec![Block::List(List { kind: ListKind::Bullets, items: vec![item(None, vec![paragraph(vec![text("a")]), bullets(&["b"])])] })]);
        // A line that only goes on with an item's text is the item's, after a blank line it is not.
        assert_eq!(blocks("- a\nweiter\n\nAbsatz"), vec![bullets(&["a weiter"]), paragraph(vec![text("Absatz")])]);
        // A list may interrupt a paragraph with a bullet or the number 1 only.
        assert_eq!(blocks("Text\n2. zwei"), vec![paragraph(vec![text("Text 2. zwei")])]);
        assert_eq!(blocks("Text:\n- eins"), vec![paragraph(vec![text("Text:")]), bullets(&["eins"])]);
        assert_eq!(blocks("-Strich und 1.Wort"), vec![paragraph(vec![text("-Strich und 1.Wort")])]);
    }

    #[test]
    fn strong_and_emphasized_text_by_the_rules_of_commonmark() {
        let strong = |s: &str| Inline::Strong(vec![text(s)]);
        let emphasis = |s: &str| Inline::Emphasis(vec![text(s)]);
        assert_eq!(blocks("**Voraussetzung:** keine"), vec![paragraph(vec![strong("Voraussetzung:"), text(" keine")])]);
        assert_eq!(blocks("Teil**zwei**er"), vec![paragraph(vec![text("Teil"), strong("zwei"), text("er")])]);
        assert_eq!(blocks("„**Zitat**“ und *(Klammer)*."), vec![paragraph(vec![text("„"), strong("Zitat"), text("“ und "), emphasis("(Klammer)"), text(".")])]);
        assert_eq!(blocks("*a **b** c*"), vec![paragraph(vec![Inline::Emphasis(vec![text("a "), strong("b"), text(" c")])])]);
        assert_eq!(blocks("*foo**bar**baz*"), vec![paragraph(vec![Inline::Emphasis(vec![text("foo"), strong("bar"), text("baz")])])]);
        assert_eq!(blocks("***a***"), vec![paragraph(vec![Inline::Emphasis(vec![strong("a")])])]);
        // What closes nothing, or opens nothing, is text.
        assert_eq!(blocks("**a*"), vec![paragraph(vec![text("*"), emphasis("a")])]);
        assert_eq!(blocks("2 * 3 ** 4 und a*b"), vec![paragraph(vec![text("2 * 3 ** 4 und a*b")])]);
        assert_eq!(blocks("** a**"), vec![paragraph(vec![text("** a**")])]);
    }

    #[test]
    fn punctuation_is_what_unicode_counts_as_punctuation_and_symbols() {
        for c in ['.', '„', '“', '–', '…', '§', '°', '€', '©', '→', '✓', '•'] {
            assert!(punctuation(c), "{c}");
        }
        // Letters and digits are none, and nor are a decomposed „ü"'s accent, the soft hyphen, a
        // zero-width space, an emoji's variation selector and a bullet of Word's symbol font.
        for c in ['a', 'ä', 'ß', '7', '²', '½', 'Ⅳ'].into_iter().chain([0x308, 0xAD, 0x200B, 0xFE0F, 0xF0B7].into_iter().filter_map(char::from_u32)) {
            assert!(!punctuation(c), "{c:?}");
        }
    }

    #[test]
    fn labels_of_one_kind_are_the_markers_of_their_list() {
        assert_eq!(
            blocks("- (a) Absorption\n- (b) **Electrons** and holes"),
            vec![Block::List(List {
                kind: ListKind::Labels,
                items: vec![
                    item(Some("(a)"), vec![paragraph(vec![text("Absorption")])]),
                    item(Some("(b)"), vec![paragraph(vec![Inline::Strong(vec![text("Electrons")]), text(" and holes")])]),
                ],
            })]
        );
        for labelled in ["- a) eins\n- b) zwei", "- I. Wiederholung\n- II. Situationen", "- 3.1. Konzept\n- 3.2. Zugänge", "- (iv) vier\n- (v) fünf"] {
            assert!(matches!(blocks(labelled).first(), Some(Block::List(List { kind: ListKind::Labels, .. }))), "{labelled}");
        }
        // One label is no list of labels, nor are labels of two kinds, nor a word.
        for bullets in ["- (a) eins", "- (a) eins\n- b) zwei", "- Teil eins\n- Teil zwei", "- A. Müller\n- Klausur", "- (Teil) eins\n- (Teil) zwei"] {
            assert!(matches!(blocks(bullets).first(), Some(Block::List(List { kind: ListKind::Bullets, .. }))), "{bullets}");
        }
    }

    #[test]
    fn nothing_but_text_reaches_a_page() {
        assert_eq!(
            blocks("# Titel\n\n<script>alert(1)</script>\n\nEin [Link](https://example.org) und `Code` und <b>HTML</b> &amp; mehr.\n\n> Zitat\n\n    eingerückt"),
            vec![
                paragraph(vec![text("# Titel")]),
                paragraph(vec![text("<script>alert(1)</script>")]),
                paragraph(vec![text("Ein [Link](https://example.org) und `Code` und <b>HTML</b> &amp; mehr.")]),
                paragraph(vec![text("> Zitat")]),
                paragraph(vec![text("eingerückt")]),
            ]
        );
    }

    #[test]
    fn escapes_are_text() {
        assert_eq!(blocks("1\\. Semester: Grundlagen\\*innen \\- \\_x\\_ \\&amp; \\<b> \\\\"), vec![paragraph(vec![text("1. Semester: Grundlagen*innen - _x_ &amp; <b> \\")])]);
        assert_eq!(blocks("\\**kein Fett**"), vec![paragraph(vec![text("*"), Inline::Emphasis(vec![text("kein Fett")]), text("*")])]);
    }

    #[test]
    fn plain_text_for_a_description() {
        let markdown = "Die Studierenden sollen\n\n- Gleichungssysteme lösen\n- **Beweise** führen\n\nIn der ersten\\\nVorlesung.";
        assert_eq!(plain(markdown), "Die Studierenden sollen Gleichungssysteme lösen · Beweise führen In der ersten Vorlesung.");
        assert_eq!(plain("- (1) Wissen\n- (2) Anwenden"), "(1) Wissen · (2) Anwenden");
        assert_eq!(plain(""), "");
    }
}
