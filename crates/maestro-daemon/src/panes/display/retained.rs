//! Retained logical text, Unicode search and word coordinates.
mod access;
mod words;
use maestro_terminal::{page::cell::Wide, screens::Key, terminal::ScreenTextRow};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
/// Absolute retained cell coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerminalTextPoint {
    /// Absolute physical row.
    pub row: u32,
    /// Physical cell column.
    pub col: u16,
}

/// Search match bound to observed text, width and screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalTextMatch {
    /// Inclusive first cell.
    pub start: TerminalTextPoint,
    /// Inclusive last cell.
    pub end: TerminalTextPoint,
    /// Fingerprint of the exact matched UTF-8 bytes.
    pub source_fingerprint: u64,
    /// Grid width when searched.
    pub scan_cols: u16,
    /// Screen when searched.
    pub scan_screen: Key,
}

/// Copy-mode word navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalWordMotion {
    /// Next punctuation-aware word start.
    NextStart,
    /// Previous punctuation-aware word start.
    PreviousStart,
    /// Next punctuation-aware word end.
    NextEnd,
    /// Next whitespace-delimited word start.
    NextBigStart,
    /// Previous whitespace-delimited word start.
    PreviousBigStart,
    /// Next whitespace-delimited word end.
    NextBigEnd,
}

/// ASCII punctuation separating ordinary copy-mode words.
const COPY_MODE_WORD_SEPARATORS: &str = "!\"#$%&'()*+,-./:;<=>?@[\\]^`{|}~";

/// Whitespace, punctuation or ordinary word text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextClass {
    /// Unicode whitespace.
    Whitespace,
    /// ASCII copy-mode punctuation.
    Separator,
    /// Any other text.
    Word,
}
/// One addressable character or a hard row boundary.
#[derive(Debug)]
struct TextAtom {
    /// Cell coordinate, absent at a hard line boundary.
    point: Option<TerminalTextPoint>,
    /// Inclusive physical column of a wide character's tail.
    end_col: u16,
    /// Navigation class of the first scalar.
    class: TextClass,
}
/// Mapping from a complete grapheme's bytes to inclusive cells.
#[derive(Debug)]
struct TextSpan {
    /// First UTF-8 byte.
    byte_start: usize,
    /// Exclusive final UTF-8 byte.
    byte_end: usize,
    /// First physical cell.
    start: TerminalTextPoint,
    /// Last physical cell.
    end: TerminalTextPoint,
}
/// A logical line joined across soft wraps.
#[derive(Debug, Default)]
struct LogicalTextLine {
    /// Concatenated cell text.
    text: String,
    /// Complete-grapheme boundaries.
    spans: Vec<TextSpan>,
}
/// Text and cell navigation indexes for a retained row window.
#[derive(Debug)]
pub(super) struct RetainedTextBuffer {
    /// Width at observation time.
    cols: u16,
    /// Searchable logical lines.
    lines: Vec<LogicalTextLine>,
    /// Navigation atoms in reading order.
    atoms: Vec<TextAtom>,
}
/// Which indexes the caller needs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum IndexKind {
    /// Search without navigation atoms.
    Search,
    /// Navigation without logical strings.
    Words,
    /// Both indexes for direct conformance witnesses.
    #[cfg(test)]
    Both,
}
impl RetainedTextBuffer {
    /// Build both indexes for direct cell witnesses.
    #[cfg(test)]
    pub(super) fn new(cols: u16, rows: Vec<ScreenTextRow>) -> Self {
        Self::build(cols, rows, 0, IndexKind::Both)
    }
    /// Build only the logical search index.
    fn new_search(cols: u16, rows: Vec<ScreenTextRow>, offset: u32) -> Self {
        Self::build(cols, rows, offset, IndexKind::Search)
    }
    /// Build only the cell navigation index.
    fn new_words(cols: u16, rows: Vec<ScreenTextRow>, offset: u32) -> Self {
        Self::build(cols, rows, offset, IndexKind::Words)
    }
    /// Walk physical cells once, joining only soft-wrapped rows.
    fn build(cols: u16, rows: Vec<ScreenTextRow>, offset: u32, kind: IndexKind) -> Self {
        let mut buffer = Self {
            cols,
            lines: Vec::new(),
            atoms: Vec::new(),
        };
        let mut line = LogicalTextLine::default();
        for (row_index, row) in rows.into_iter().enumerate() {
            let Ok(row_index) = u32::try_from(row_index) else {
                break;
            };
            let y = offset.saturating_add(row_index);
            buffer.push_row(&mut line, y, row, kind);
        }
        if kind != IndexKind::Words && (!line.text.is_empty() || !line.spans.is_empty()) {
            buffer.lines.push(line);
        }
        buffer
    }
    /// Append one physical row and preserve its hard or soft boundary.
    fn push_row(
        &mut self,
        line: &mut LogicalTextLine,
        row_index: u32,
        row: ScreenTextRow,
        kind: IndexKind,
    ) {
        for (x, cell) in row.cells.into_iter().enumerate() {
            let Ok(x) = u16::try_from(x) else {
                break;
            };
            self.push_cell(
                line,
                TerminalTextPoint {
                    row: row_index,
                    col: x,
                },
                &cell,
                kind,
            );
        }
        if row.soft_wrapped {
            return;
        }
        if kind != IndexKind::Words {
            self.lines.push(std::mem::take(line).trimmed());
        }
        if kind != IndexKind::Search {
            self.atoms.push(TextAtom {
                point: None,
                end_col: 0,
                class: TextClass::Whitespace,
            });
        }
    }
    /// Append one complete grapheme without giving wide tails their own index.
    fn push_cell(
        &mut self,
        line: &mut LogicalTextLine,
        start: TerminalTextPoint,
        cell: &maestro_terminal::terminal::ScreenTextCell,
        kind: IndexKind,
    ) {
        if cell.wide == Wide::SpacerTail {
            return;
        }
        if cell.wide == Wide::SpacerHead {
            if kind != IndexKind::Search {
                self.atoms.push(TextAtom {
                    point: Some(start),
                    end_col: start.col,
                    class: self
                        .atoms
                        .last()
                        .map_or(TextClass::Whitespace, |atom| atom.class),
                });
            }
            return;
        }
        let text = terminal_cell_text(&cell.graphemes);
        let end = TerminalTextPoint {
            col: start.col.saturating_add(u16::from(cell.wide == Wide::Wide)),
            ..start
        };
        if kind != IndexKind::Words {
            let byte_start = line.text.len();
            line.text.push_str(&text);
            line.spans.push(TextSpan {
                byte_start,
                byte_end: line.text.len(),
                start,
                end,
            });
        }
        if kind != IndexKind::Search {
            self.atoms.push(TextAtom {
                point: Some(start),
                end_col: end.col,
                class: text_class(&text),
            });
        }
    }
    /// Find literal Unicode-aware matches aligned to complete graphemes.
    pub(super) fn search(
        &self,
        query: &str,
        case_sensitive: bool,
        active_screen: Key,
    ) -> Vec<TerminalTextMatch> {
        if query.is_empty() {
            return Vec::new();
        }
        let Ok(regex) = regex::RegexBuilder::new(&regex::escape(query))
            .case_insensitive(!case_sensitive)
            .build()
        else {
            return Vec::new();
        };
        self.lines
            .iter()
            .flat_map(|line| {
                regex
                    .find_iter(&line.text)
                    .filter_map(|found| line.search_match(found, self.cols, active_screen))
            })
            .collect()
    }

    /// Revalidate exact coordinates and text without accepting partial graphemes.
    fn contains_match(&self, text_match: TerminalTextMatch) -> bool {
        self.lines.iter().any(|line| {
            let Ok(start_index) = line
                .spans
                .binary_search_by_key(&text_match.start, |span| span.start)
            else {
                return false;
            };
            let Ok(end_index) = line
                .spans
                .binary_search_by_key(&text_match.end, |span| span.end)
            else {
                return false;
            };
            let start_span = &line.spans[start_index];
            let end_span = &line.spans[end_index];
            start_span.byte_start <= end_span.byte_end
                && text_fingerprint(&line.text[start_span.byte_start..end_span.byte_end])
                    == text_match.source_fingerprint
        })
    }
}
impl LogicalTextLine {
    /// Translate a complete-grapheme regex match into cell coordinates.
    fn search_match(
        &self,
        found: regex::Match<'_>,
        cols: u16,
        screen: Key,
    ) -> Option<TerminalTextMatch> {
        let first = self
            .spans
            .binary_search_by_key(&found.start(), |span| span.byte_start)
            .ok()?;
        let last = self
            .spans
            .binary_search_by_key(&found.end(), |span| span.byte_end)
            .ok()?;
        Some(TerminalTextMatch {
            start: self.spans[first].start,
            end: self.spans[last].end,
            source_fingerprint: text_fingerprint(found.as_str()),
            scan_cols: cols,
            scan_screen: screen,
        })
    }
    /// Remove trailing whitespace and its now-unsearchable spans.
    fn trimmed(mut self) -> Self {
        let len = self.text.trim_end().len();
        while self.spans.last().is_some_and(|span| span.byte_start >= len) {
            self.spans.pop();
        }
        self.text.truncate(len);
        self
    }
}
/// Interpret blank and image-placeholder cells as one space.
pub(super) fn terminal_cell_text(graphemes: &[char]) -> String {
    if graphemes.is_empty() || graphemes.first() == Some(&'\u{10eeee}') {
        " ".to_owned()
    } else {
        graphemes.iter().collect()
    }
}
/// Classify the first scalar as copy-mode whitespace, punctuation or a word.
fn text_class(text: &str) -> TextClass {
    match text.chars().next() {
        None => TextClass::Whitespace,
        Some(ch) if ch.is_whitespace() => TextClass::Whitespace,
        Some(ch) if ch.is_ascii() && COPY_MODE_WORD_SEPARATORS.contains(ch) => TextClass::Separator,
        Some(_) => TextClass::Word,
    }
}
/// Fingerprint exact matched text for stale-result rejection.
fn text_fingerprint(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}
