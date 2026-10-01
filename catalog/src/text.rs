//! The free texts of a module — learning outcomes, contents, assessment, remarks, prerequisites —
//! are Markdown since schema 10 (docs/schema-v2.md §3, „Module texts"): Radix writes them from the
//! module's page (`internal/parser/markdown.go`). Here they are read into what a page sets of
//! them: paragraphs, lists, strong and emphasized text, line breaks (`blocks`), and into one line
//! of plain text for what describes a page to others (`plain`).
//!
//! Nothing else of Markdown reaches a page. A heading is a strong paragraph, a quote its
//! paragraphs, code and HTML their text, a link and an image their words: the texts come from the
//! university's pages, and a page of Betula sets text, never markup taken from elsewhere.
//!
//! A list whose items all begin with a label („(1)", „a)", „IV.", „3.1.") is one CommonMark can
//! only write as bullets (it numbers with digits alone); its labels are its markers
//! (`ListKind::Labels`).

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

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
    let mut reader = Reader { containers: vec![Vec::new()], ..Reader::default() };
    for event in Parser::new_ext(markdown, Options::empty()) {
        reader.event(event);
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

/// How the inlines of an open span end up in the one around it.
#[derive(Clone, Copy, PartialEq)]
enum Span {
    /// A paragraph; `true` where the parser opened none (the items of a tight list).
    Paragraph(bool),
    Heading,
    Strong,
    Emphasis,
    /// A link, an image, struck text …: only its words count.
    Words,
}

/// Builds the blocks from the parser's events: the blocks of the text and of each open item (the
/// text's first), the open lists, the open spans of the paragraph at hand.
#[derive(Default)]
struct Reader {
    containers: Vec<Vec<Block>>,
    lists: Vec<(Option<u64>, Vec<Item>)>,
    spans: Vec<(Span, Vec<Inline>)>,
    /// Inside a code block: its lines are kept.
    code: bool,
}

impl Reader {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) if self.code => {
                for (n, line) in text.split('\n').enumerate() {
                    if n > 0 {
                        self.inline(Inline::Break);
                    }
                    self.text(line);
                }
            }
            Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) | Event::InlineMath(text) | Event::DisplayMath(text) => {
                self.text(&text);
            }
            Event::FootnoteReference(label) => self.text(&label),
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => self.inline(Inline::Break),
            Event::Rule | Event::TaskListMarker(_) => self.close_paragraph(),
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph | Tag::HtmlBlock => self.open(Span::Paragraph(false)),
            Tag::CodeBlock(_) => {
                self.open(Span::Paragraph(false));
                self.code = true;
            }
            Tag::Heading { .. } => self.open(Span::Heading),
            Tag::List(start) => {
                self.close_paragraph();
                self.lists.push((start, Vec::new()));
            }
            Tag::Item => {
                self.close_paragraph();
                self.containers.push(Vec::new());
            }
            Tag::Strong => self.open_inline(Span::Strong),
            Tag::Emphasis => self.open_inline(Span::Emphasis),
            Tag::Link { .. } | Tag::Image { .. } | Tag::Strikethrough | Tag::Superscript | Tag::Subscript => self.open_inline(Span::Words),
            // A quote, a table, a definition …: their paragraphs stand where they are.
            _ => self.close_paragraph(),
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::HtmlBlock | TagEnd::Heading(_) => self.close_paragraph(),
            TagEnd::CodeBlock => {
                self.code = false;
                self.close_paragraph();
            }
            TagEnd::List(_) => {
                self.close_paragraph();
                if let Some((start, items)) = self.lists.pop() {
                    let list = labelled(start, items);
                    self.block(Block::List(list));
                }
            }
            TagEnd::Item => {
                self.close_paragraph();
                let blocks = self.containers.pop().unwrap_or_default();
                if let Some((_, items)) = self.lists.last_mut() {
                    items.push(Item { label: None, blocks });
                }
            }
            TagEnd::Strong | TagEnd::Emphasis | TagEnd::Link | TagEnd::Image | TagEnd::Strikethrough | TagEnd::Superscript | TagEnd::Subscript => {
                self.close_inline()
            }
            _ => {}
        }
    }

    /// Opens a paragraph's span; one already open (a heading in an item's text) is closed first.
    fn open(&mut self, span: Span) {
        self.close_paragraph();
        self.spans.push((span, Vec::new()));
    }

    /// Opens a span inside a paragraph, and the paragraph where none is open (a tight item).
    fn open_inline(&mut self, span: Span) {
        if self.spans.is_empty() {
            self.spans.push((Span::Paragraph(true), Vec::new()));
        }
        self.spans.push((span, Vec::new()));
    }

    fn close_inline(&mut self) {
        if !matches!(self.spans.last(), Some((Span::Strong | Span::Emphasis | Span::Words, _))) {
            return;
        }
        if let Some((span, inner)) = self.spans.pop() {
            match span {
                Span::Strong => self.inline(Inline::Strong(inner)),
                Span::Emphasis => self.inline(Inline::Emphasis(inner)),
                _ => inner.into_iter().for_each(|inline| self.inline(inline)),
            }
        }
    }

    fn text(&mut self, text: &str) {
        if !text.is_empty() {
            self.inline(Inline::Text(text.to_string()));
        }
    }

    fn inline(&mut self, inline: Inline) {
        if self.spans.is_empty() {
            self.spans.push((Span::Paragraph(true), Vec::new()));
        }
        if let Some((_, inlines)) = self.spans.last_mut() {
            match (inlines.last_mut(), inline) {
                (Some(Inline::Text(before)), Inline::Text(text)) => before.push_str(&text),
                (_, inline) => inlines.push(inline),
            }
        }
    }

    /// Closes the paragraph at hand with all spans still open in it.
    fn close_paragraph(&mut self) {
        while matches!(self.spans.last(), Some((Span::Strong | Span::Emphasis | Span::Words, _))) {
            self.close_inline();
        }
        if let Some((span, inlines)) = self.spans.pop() {
            let inlines = trim(inlines);
            if inlines.is_empty() {
                return;
            }
            self.block(Block::Paragraph(if span == Span::Heading { vec![Inline::Strong(inlines)] } else { inlines }));
        }
    }

    fn block(&mut self, block: Block) {
        if let Some(blocks) = self.containers.last_mut() {
            blocks.push(block);
        }
    }

    /// The text's blocks. The parser ends every list and item it begins, so nothing is open here.
    fn finish(mut self) -> Vec<Block> {
        self.close_paragraph();
        self.containers.into_iter().flatten().collect()
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

/// The list a parsed list is: a bullet list whose items all begin with a label of one kind is a
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

    #[test]
    fn paragraphs_and_line_breaks() {
        assert_eq!(
            blocks("Die Studierenden sollen\nlernen.\n\n**Lecture:**\\\nThe lecture deals with *chemicals*."),
            vec![
                paragraph(vec![text("Die Studierenden sollen lernen.")]),
                paragraph(vec![Inline::Strong(vec![text("Lecture:")]), Inline::Break, text("The lecture deals with "), Inline::Emphasis(vec![text("chemicals")]), text(".")]),
            ]
        );
    }

    #[test]
    fn lists_nest_and_number_from_their_start() {
        let markdown = "1. Drei Präsentationen (45%):\n   - Themen\n   - Fortschritt\n\n   (jeweils 5 Punkte)\n2. Seminararbeit";
        assert_eq!(
            blocks(markdown),
            vec![Block::List(List {
                kind: ListKind::Numbers(1),
                items: vec![
                    item(
                        None,
                        vec![
                            paragraph(vec![text("Drei Präsentationen (45%):")]),
                            Block::List(List { kind: ListKind::Bullets, items: vec![item(None, vec![paragraph(vec![text("Themen")])]), item(None, vec![paragraph(vec![text("Fortschritt")])])] }),
                            paragraph(vec![text("(jeweils 5 Punkte)")]),
                        ]
                    ),
                    item(None, vec![paragraph(vec![text("Seminararbeit")])]),
                ],
            })]
        );
        assert_eq!(blocks("3. drei\n4. vier").first().map(|b| matches!(b, Block::List(List { kind: ListKind::Numbers(3), .. }))), Some(true));
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
            blocks("# Titel\n\n<script>alert(1)</script>\n\nEin [Link](https://example.org) und `Code` und <b>HTML</b>.\n\n> Zitat"),
            vec![
                paragraph(vec![Inline::Strong(vec![text("Titel")])]),
                paragraph(vec![text("<script>alert(1)</script>")]),
                paragraph(vec![text("Ein Link und Code und <b>HTML</b>.")]),
                paragraph(vec![text("Zitat")]),
            ]
        );
    }

    #[test]
    fn escapes_are_text() {
        assert_eq!(blocks("1\\. Semester: Grundlagen\\*innen \\- \\_x\\_"), vec![paragraph(vec![text("1. Semester: Grundlagen*innen - _x_")])]);
    }

    #[test]
    fn plain_text_for_a_description() {
        let markdown = "Die Studierenden sollen\n\n- Gleichungssysteme lösen\n- **Beweise** führen\n\nIn der ersten\\\nVorlesung.";
        assert_eq!(plain(markdown), "Die Studierenden sollen Gleichungssysteme lösen · Beweise führen In der ersten Vorlesung.");
        assert_eq!(plain("- (1) Wissen\n- (2) Anwenden"), "(1) Wissen · (2) Anwenden");
        assert_eq!(plain(""), "");
    }
}
