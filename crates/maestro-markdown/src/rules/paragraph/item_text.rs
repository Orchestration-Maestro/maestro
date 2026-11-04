//! Setext lookahead over item text admitted one line at a time.
use super::{Grammar, Heading, blank, carriage, displayed, matches, selected, underline};
use crate::{
    MarkedOptions,
    rules::lines::{Input, line_end},
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

/// First entry of an ascending list that `before` does not place before the seek.
///
/// A partition point answers `before` a logarithmic number of times, so an attempt that
/// asks again about a list it has grown does not rescan the entries it passed.
fn seek(items: &[usize], mut before: impl FnMut(usize) -> bool) -> Option<usize> {
    let at = items.partition_point(|item| {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.item_seeks += 1);
        before(*item)
    });
    items.get(at).copied()
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

/// One search's classified lines that close it.
#[derive(Clone, Copy)]
enum Search {
    /// The setext label search.
    Setext,
    /// The paragraph interruption search.
    Paragraph,
}

/// The operand a caller has left, from its first unread byte, and the closing angles
/// it displays as text by those offsets.
#[derive(Clone, Copy)]
struct Remainder<'a> {
    /// Unread suffix of the operand this attempt reports on.
    source: &'a Segmented,
    /// Operand offset of that suffix's first byte.
    start: usize,
    /// Closing angles the caller displays as text, by suffix offset.
    escaped: &'a dyn Fn(usize) -> bool,
}

/// Setext lookahead over one operand whose lines are admitted one at a time.
///
/// Lines are classified only as far as an attempt's answer needs them, so a body
/// whose first lines already settle both searches costs its prefix, not its length.
pub(crate) struct ItemText {
    /// Shared immutable caller predicates.
    grammar: &'static Grammar,
    /// Classified lines of the retained operand, contiguous from its start.
    lines: Vec<ItemLine>,
    /// Indices of classified lines that close the setext label search.
    setext_stops: Vec<usize>,
    /// Indices of classified lines that close the paragraph interruption search.
    paragraph_stops: Vec<usize>,
    /// Indices of classified lines carrying an eligible underline.
    underlines: Vec<usize>,
    /// Operand offset of the first unclassified byte.
    framing: usize,
    /// Logical length of the retained operand.
    extent: Option<usize>,
}

impl ItemText {
    /// Begin one caller grammar for an operand not yet supplied.
    pub(crate) fn new(options: MarkedOptions) -> Self {
        Self {
            grammar: selected(options),
            lines: Vec::new(),
            setext_stops: Vec::new(),
            paragraph_stops: Vec::new(),
            underlines: Vec::new(),
            framing: 0,
            extent: None,
        }
    }

    /// Answer this attempt's heading and whether more input can change it.
    ///
    /// `source` is the item text still unread, a suffix of the operand the first call saw,
    /// and `escaped` reports the closing angles the caller displays as text by its offsets.
    pub(crate) fn attempt(
        &mut self,
        source: &Segmented,
        escaped: &dyn Fn(usize) -> bool,
    ) -> (Option<Heading>, bool) {
        let start = match self.extent {
            Some(extent) if extent >= source.len() => extent - source.len(),
            _ => {
                self.extent = Some(source.len());
                0
            }
        };
        let remainder = Remainder {
            source,
            start,
            escaped,
        };
        let index = self.lines.partition_point(|line| line.range.start < start);
        self.ensure(&remainder, index + 1);
        let partial = self.partial(&remainder, index);
        let origin = if partial.is_some() {
            start
        } else {
            match self.lines.get(index) {
                Some(line) => line.range.start,
                None => return (None, true),
            }
        };
        let partial = partial.as_ref();
        let setext = self.closing(&remainder, index, Search::Setext, partial);
        let paragraph = self.closing(&remainder, index, Search::Paragraph, partial);
        match self
            .first_underline(&remainder, origin, setext)
            .filter(|found| *found <= setext)
        {
            Some(found) => {
                let heading = self.heading(&remainder, found, origin);
                let settled = self.settled_by_boundary(&remainder, found);
                (Some(heading), !settled)
            }
            None => (
                None,
                !(self.closed_past(&remainder, setext) && self.closed_past(&remainder, paragraph)),
            ),
        }
    }

    /// Classify the rest of a line that a caller's token stopped inside.
    fn partial(&self, remainder: &Remainder<'_>, index: usize) -> Option<ItemLine> {
        let start = remainder.start;
        let line = self.lines.get(index.checked_sub(1)?)?;
        let rest = line.range.end.checked_sub(start).filter(|rest| *rest > 0)?;
        let end = self
            .lines
            .get(index)
            .map_or(rest, |next| next.range.end - start);
        Some(self.line(remainder.source, 0..rest, end, remainder.escaped))
    }

    /// First underline this attempt can admit, which excludes its own first line.
    fn first_underline(
        &mut self,
        remainder: &Remainder<'_>,
        origin: usize,
        setext: usize,
    ) -> Option<usize> {
        loop {
            let found = seek(&self.underlines, |index| {
                self.lines[index].range.start <= origin + 1
            });
            if let Some(found) = found {
                return Some(found);
            }
            if setext != usize::MAX && self.lines.len() > setext {
                return None;
            }
            if !self.advance(remainder) {
                return None;
            }
        }
    }

    /// First line closing one search, honouring an attempt's exempt first line.
    ///
    /// A partly consumed first line closes the search at itself, so it answers
    /// below every line an underline can occupy.
    fn closing(
        &mut self,
        remainder: &Remainder<'_>,
        index: usize,
        search: Search,
        partial: Option<&ItemLine>,
    ) -> usize {
        let Some(first) = partial else {
            let found = self.stop(remainder, index, search);
            if found != index || self.effect(index, search) != Stop::Later {
                return found;
            }
            return self.stop(remainder, index + 1, search);
        };
        if Self::search(first, search) == Stop::Closed {
            return index.saturating_sub(1);
        }
        self.stop(remainder, index, search)
    }

    /// One classified line's effect on the selected search.
    fn effect(&self, index: usize, search: Search) -> Stop {
        self.lines
            .get(index)
            .map_or(Stop::Open, |line| Self::search(line, search))
    }

    /// One classified line's effect on a search.
    fn search(line: &ItemLine, search: Search) -> Stop {
        match search {
            Search::Setext => line.setext,
            Search::Paragraph => line.paragraph,
        }
    }

    /// First line at or after `index` that closes one search.
    fn stop(&mut self, remainder: &Remainder<'_>, index: usize, search: Search) -> usize {
        loop {
            let stops = match search {
                Search::Setext => &self.setext_stops,
                Search::Paragraph => &self.paragraph_stops,
            };
            if let Some(found) = seek(stops, |line| line < index) {
                return found;
            }
            if !self.advance(remainder) {
                return usize::MAX;
            }
        }
    }

    /// Whether a terminated line follows the one closing a search.
    ///
    /// This is the growing-input pass's settled condition without measuring the operand's
    /// remaining length.
    fn closed_past(&mut self, remainder: &Remainder<'_>, index: usize) -> bool {
        if index == usize::MAX {
            return false;
        }
        self.ensure(remainder, index + 2);
        self.lines
            .get(index + 1)
            .is_some_and(|line| line.ending != Ending::Unterminated)
    }

    /// Keep the underline's label, level and closed extent.
    fn heading(&mut self, remainder: &Remainder<'_>, index: usize, origin: usize) -> Heading {
        let line = &self.lines[index];
        let mut consumed = line.range.start + line.consumed;
        let mut rest = index + 1;
        loop {
            if self.lines.len() <= rest && !self.advance(remainder) {
                break;
            }
            let Some(next) = self.lines.get(rest) else {
                break;
            };
            if next.ending != Ending::Newline {
                break;
            }
            consumed = next.range.end;
            rest += 1;
        }
        let content = 0..self.lines[index].range.start - 1 - origin;
        let level = self.lines[index].underline.unwrap_or(1);
        Heading {
            content,
            level,
            consumed: consumed - origin,
        }
    }

    /// Whether a processed non-newline line after the underline settles its extent.
    fn settled_by_boundary(&mut self, remainder: &Remainder<'_>, index: usize) -> bool {
        let mut next = index + 1;
        loop {
            self.ensure(remainder, next + 1);
            let Some(line) = self.lines.get(next) else {
                return false;
            };
            if line.ending != Ending::Newline {
                break;
            }
            next += 1;
        }
        self.closed_past(remainder, next)
    }

    /// Classify lines until `needed` are known or the operand is exhausted.
    fn ensure(&mut self, remainder: &Remainder<'_>, needed: usize) {
        while self.lines.len() < needed && self.advance(remainder) {}
    }

    /// Classify the next physical line of the operand, if any bytes remain.
    fn advance(&mut self, remainder: &Remainder<'_>) -> bool {
        let (source, start) = (remainder.source, remainder.start);
        // Classification never trails the caller's consumed prefix, so the remaining line
        // bytes are always inside the supplied suffix.
        let local = self.framing.saturating_sub(start);
        if local >= source.len() {
            return false;
        }
        let end = line_end(source, local).unwrap_or(source.len());
        // An unterminated next line still closes the window at the operand's end, so a
        // header and delimiter pair it holds can interrupt this line.
        let lookahead = line_end(source, end).unwrap_or(source.len());
        let line = self.line(source, local..end, lookahead, remainder.escaped);
        let position = self.lines.len();
        if line.setext != Stop::Open {
            self.setext_stops.push(position);
        }
        if line.paragraph != Stop::Open {
            self.paragraph_stops.push(position);
        }
        if line.underline.is_some() {
            self.underlines.push(position);
        }
        self.framing = start + end;
        self.lines.push(ItemLine {
            range: start + local..start + end,
            ..line
        });
        true
    }

    /// Classify one physical line with the shared per-line predicates.
    fn line(
        &self,
        source: &Segmented,
        range: Range<usize>,
        end: usize,
        escaped: &dyn Fn(usize) -> bool,
    ) -> ItemLine {
        let window = displayed(source.window(range.start..end), range.start, escaped);
        let text = source.window(range.clone());
        let text = text.as_ref();
        let found = underline(text).filter(|_| range.start > 1);
        let grammar = matches(self.grammar.setext.as_ref(), &window);
        let separator = carriage(text);
        let interrupted = matches(self.grammar.paragraph.as_ref(), &window);
        let empty = text.strip_suffix('\n').unwrap_or(text).is_empty();
        ItemLine {
            underline: found.map(|(level, _)| level),
            consumed: found.map_or(0, |(_, end)| end),
            setext: match (grammar || separator, blank(text)) {
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
}
