//! Setext lookahead over item text admitted one line at a time.
use super::{Grammar, Heading, UNDERLINE, matches, selected};
use crate::{
    MarkedOptions,
    rules::lines::{Cursor, Input},
    source::Segmented,
};
use std::ops::Range;

/// How one physical line affects a search that a later line can close.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// The line leaves the search open.
    Open,
    /// The line closes the search only when a later line of an attempt reads it.
    Later,
    /// The line closes the search wherever an attempt starts.
    Closed,
}

/// How one physical line ends.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// The line is a bare newline.
    Newline,
    /// The line holds content and ends with a terminator.
    Terminated,
    /// The operand ends inside the line.
    Unterminated,
}

/// One classified line of an operand admitted line by line.
struct ItemLine {
    /// Authored range of the line, including its terminator.
    range: Range<usize>,
    /// Heading level of an eligible underline on this line.
    underline: Option<usize>,
    /// End of the underline and its trailing newlines within the line.
    consumed: usize,
    /// Effect on the setext label search.
    setext: Stop,
    /// Effect on the paragraph interruption search.
    paragraph: Stop,
    /// How the line ends.
    ending: Ending,
}

/// First line at or after a given one that closes each search.
#[derive(Clone, Copy)]
struct Next {
    /// First line closing the setext label search.
    setext: usize,
    /// First line closing the paragraph interruption search.
    paragraph: usize,
}

/// Setext lookahead over one operand whose lines are admitted one at a time.
pub(crate) struct ItemText {
    /// Shared immutable caller predicates.
    grammar: &'static Grammar,
    /// Classified lines of the retained operand.
    lines: Vec<ItemLine>,
    /// Per-line look-up of each search's next stop.
    next: Vec<Next>,
    /// Indices of the lines carrying an eligible underline.
    underlines: Vec<usize>,
    /// Complete lines a growing-input pass can process.
    framed: usize,
    /// Logical length of the retained operand.
    extent: Option<usize>,
}

impl ItemText {
    /// Begin one caller grammar for an operand not yet supplied.
    pub(crate) fn new(options: MarkedOptions) -> Self {
        Self {
            grammar: selected(options),
            lines: Vec::new(),
            next: Vec::new(),
            underlines: Vec::new(),
            framed: 0,
            extent: None,
        }
    }

    /// Answer this attempt's heading and whether more input can change it.
    ///
    /// `source` is the item text still unread, a suffix of the operand the first call saw.
    pub(crate) fn attempt(&mut self, source: &Segmented) -> (Option<Heading>, bool) {
        let start = match self.extent {
            Some(extent) if extent >= source.len() => extent - source.len(),
            _ => {
                self.classify(source);
                self.extent = Some(source.len());
                0
            }
        };
        let index = self.lines.partition_point(|line| line.range.start < start);
        let Some(line) = self.lines.get(index) else {
            return (None, true);
        };
        let origin = line.range.start;
        let setext = self.closing(index, ItemLine::setext, |next| next.setext);
        let paragraph = self.closing(index, ItemLine::paragraph, |next| next.paragraph);
        match self.underline(origin).filter(|found| *found <= setext) {
            Some(found) => (
                Some(self.heading(found, origin)),
                !self.settled_by_boundary(found),
            ),
            None => (None, !self.ended(setext, paragraph)),
        }
    }

    /// First underline this attempt can admit, which excludes its own first line.
    fn underline(&self, origin: usize) -> Option<usize> {
        let rest = self
            .underlines
            .partition_point(|found| self.lines[*found].range.start <= origin + 1);
        self.underlines.get(rest).copied()
    }

    /// First line closing one search, honouring an attempt's exempt first line.
    fn closing(&self, index: usize, kind: fn(&ItemLine) -> Stop, next: fn(Next) -> usize) -> usize {
        let Some(entry) = self.next.get(index) else {
            return usize::MAX;
        };
        let found = next(*entry);
        if found != index || kind(&self.lines[index]) != Stop::Later {
            return found;
        }
        self.next
            .get(index + 1)
            .map_or(usize::MAX, |entry| next(*entry))
    }

    /// Whether the growing-input pass has settled both searches.
    fn ended(&self, setext: usize, paragraph: usize) -> bool {
        let processed = self.framed.saturating_sub(1);
        setext < processed && paragraph < processed
    }

    /// Keep the underline's label, level and closed extent.
    fn heading(&self, index: usize, origin: usize) -> Heading {
        let line = &self.lines[index];
        let mut consumed = line.range.start + line.consumed;
        let mut rest = index + 1;
        while let Some(next) = self.lines.get(rest) {
            if next.ending != Ending::Newline {
                break;
            }
            consumed = next.range.end;
            rest += 1;
        }
        Heading {
            content: 0..line.range.start - 1 - origin,
            level: line.underline.unwrap_or(1),
            consumed: consumed - origin,
        }
    }

    /// Whether a processed non-newline line after the underline settles its extent.
    fn settled_by_boundary(&self, index: usize) -> bool {
        let processed = self.framed.saturating_sub(1);
        let first = index + 1;
        first < processed
            && self.lines[first..processed]
                .iter()
                .any(|line| line.ending != Ending::Newline)
    }

    /// Classify every line of the retained operand once.
    fn classify(&mut self, source: &Segmented) {
        let mut cursor = Cursor::default();
        let mut ranges = Vec::new();
        while let Some(range) = cursor.next(source) {
            ranges.push(range);
        }
        if let Some(range) = cursor.finish(source) {
            ranges.push(range);
        }
        let mut lines = Vec::with_capacity(ranges.len());
        for (index, range) in ranges.iter().enumerate() {
            let end = ranges.get(index + 1).map_or(range.end, |next| next.end);
            lines.push(self.line(source, range.clone(), end));
        }
        self.framed = lines
            .iter()
            .filter(|line| line.ending != Ending::Unterminated)
            .count();
        self.underlines = (0..lines.len())
            .filter(|index| lines[*index].underline.is_some())
            .collect();
        self.lines = lines;
        self.index();
    }

    /// Classify one physical line with the recognizer's own predicates.
    fn line(&self, source: &Segmented, range: Range<usize>, end: usize) -> ItemLine {
        let window = source.window(range.start..end);
        let text = source.window(range.clone());
        let text = text.as_ref();
        let underline = UNDERLINE
            .as_ref()
            .and_then(|rule| rule.find(text))
            .filter(|_| range.start > 1);
        let blank = text.ends_with('\n') && text.chars().all(maestro_text::is_whitespace_scalar);
        let grammar = matches(self.grammar.setext.as_ref(), &window);
        let carriage = text.contains(['\r', '\u{2028}', '\u{2029}']);
        let empty = text.strip_suffix('\n').unwrap_or(text).is_empty();
        let interrupted = matches(self.grammar.paragraph.as_ref(), &window);
        ItemLine {
            underline: underline.as_ref().map(|found| {
                usize::from(text[found.group(1).unwrap_or_default()].starts_with('-')) + 1
            }),
            consumed: underline.as_ref().map_or(0, regress::Match::end),
            setext: match (grammar || carriage, blank) {
                (true, _) => Stop::Closed,
                (false, true) => Stop::Later,
                (false, false) => Stop::Open,
            },
            paragraph: match (empty, interrupted) {
                (true, _) => Stop::Closed,
                (false, true) => Stop::Later,
                (false, false) => Stop::Open,
            },
            ending: match (text.ends_with('\n'), text == "\n") {
                (false, _) => Ending::Unterminated,
                (true, true) => Ending::Newline,
                (true, false) => Ending::Terminated,
            },
            range,
        }
    }

    /// Resolve each line's next setext stop and paragraph stop.
    fn index(&mut self) {
        let mut next = Next {
            setext: usize::MAX,
            paragraph: usize::MAX,
        };
        self.next = vec![next; self.lines.len()];
        for index in (0..self.lines.len()).rev() {
            let line = &self.lines[index];
            if line.setext != Stop::Open {
                next.setext = index;
            }
            if line.paragraph != Stop::Open {
                next.paragraph = index;
            }
            self.next[index] = next;
        }
    }
}

impl ItemLine {
    /// Effect of this line on the setext label search.
    fn setext(&self) -> Stop {
        self.setext
    }

    /// Effect of this line on the paragraph interruption search.
    fn paragraph(&self) -> Stop {
        self.paragraph
    }
}
