//! Escape-free visible text, source-ordered escape events and indivisible graphemes.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use super::measure::grapheme_cells;
use super::style::{Hyperlink, StyleState};

/// What an escape sequence does to the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EventKind {
    /// Select Graphic Rendition escape that changes colours or attributes.
    Sgr,
    /// OSC 8 hyperlink open or close.
    Hyperlink,
    /// Any other supported control; layout never interprets it.
    Opaque,
}

/// One recognized escape sequence anchored to a position in the visible text.
#[derive(Clone, Copy, Debug)]
pub(super) struct Event<'a> {
    /// Byte offset in the visible text that the escape precedes.
    pub offset: usize,
    /// The escape exactly as written.
    pub raw: &'a str,
    /// Meaning of the escape.
    pub kind: EventKind,
    /// Index of the style state in effect after this escape.
    pub state: usize,
}

impl Event<'_> {
    /// Whether the escape changes the style state rather than carrying metadata.
    pub(super) const fn is_style(&self) -> bool {
        !matches!(self.kind, EventKind::Opaque)
    }
}

/// A visible grapheme cluster with the escapes written before or inside it.
#[derive(Clone, Debug)]
pub(super) struct Grapheme {
    /// Byte range in the visible text.
    pub range: Range<usize>,
    /// Terminal cells the cluster occupies.
    pub cells: usize,
    /// Index of the style state in effect before the cluster's first escape.
    pub state: usize,
    /// Escapes anchored at the cluster's start or strictly inside it.
    pub events: Range<usize>,
}

/// Text split once into visible content and the escapes between its graphemes.
#[derive(Debug)]
pub(super) struct Parsed<'a> {
    /// Text with every recognized escape removed.
    pub visible: String,
    /// Recognized escapes in source order.
    pub events: Vec<Event<'a>>,
    /// Style snapshots taken at each style transition; index 0 is the default.
    pub states: Vec<StyleState<'a>>,
}

/// The supported escape that starts at byte `pos`, if one starts there.
pub(super) fn recognize(text: &str, pos: usize) -> Option<&str> {
    let rest = text.get(pos..)?.strip_prefix('\x1b')?;
    let length = match rest.as_bytes().first()? {
        b'[' => 2 + rest[1..].find(['m', 'G', 'K', 'H', 'J'])?,
        b']' | b'_' => 1 + string_escape_length(&rest[1..])?,
        _ => return None,
    };
    text.get(pos..pos + length + 1)
}

/// Bytes up to and including the BEL or `ESC \` that ends a string escape body.
fn string_escape_length(body: &str) -> Option<usize> {
    let bytes = body.as_bytes();
    (0..bytes.len()).find_map(|index| match bytes[index] {
        0x07 => Some(index + 1),
        0x1b if bytes.get(index + 1) == Some(&b'\\') => Some(index + 2),
        _ => None,
    })
}

/// Parameters of the leftmost `ESC [ digits-and-semicolons m` inside a CSI escape.
fn sgr_params(code: &str) -> Option<&str> {
    let inner = code.strip_suffix('m')?;
    inner.match_indices("\x1b[").find_map(|(index, _)| {
        let params = &inner[index + 2..];
        params
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b';')
            .then_some(params)
    })
}

/// Last positions at which a CSI or a string escape of a text can still end.
struct Endings {
    /// Position of the last CSI final byte.
    csi: Option<usize>,
    /// Position of the last BEL or `ESC \` terminator byte.
    string: Option<usize>,
}

impl Endings {
    /// Scans `text` once for the last possible terminators.
    fn of(text: &str) -> Self {
        let string_terminator = text.rfind("\x1b\\").map(|index| index + 1);
        Self {
            csi: text.rfind(['m', 'G', 'K', 'H', 'J']),
            string: text.rfind('\x07').max(string_terminator),
        }
    }

    /// Whether an escape starting at `position` has any terminator after it.
    fn can_end(&self, text: &str, position: usize) -> bool {
        let last = match text.as_bytes().get(position + 1) {
            Some(b'[') => self.csi,
            Some(b']' | b'_') => self.string,
            _ => None,
        };
        last.is_some_and(|last| last > position + 1)
    }
}

impl<'a> Parsed<'a> {
    /// Splits `text` into visible content and escape events.
    pub(super) fn parse(text: &'a str) -> Self {
        let mut parsed = Self {
            visible: String::with_capacity(text.len()),
            events: Vec::new(),
            states: vec![StyleState::default()],
        };
        let endings = Endings::of(text);
        let mut index = 0;
        while let Some(found) = text[index..].find('\x1b') {
            let position = index + found;
            parsed.visible.push_str(&text[index..position]);
            let code = endings
                .can_end(text, position)
                .then(|| recognize(text, position))
                .flatten();
            if let Some(code) = code {
                parsed.push_event(code);
                index = position + code.len();
            } else {
                parsed.visible.push('\x1b');
                index = position + 1;
            }
        }
        parsed.visible.push_str(&text[index..]);
        parsed
    }

    /// Records one escape and the style state it leads to.
    fn push_event(&mut self, code: &'a str) {
        let mut state = self.states[self.events.last().map_or(0, |event| event.state)];
        let kind = if let Some(change) = Hyperlink::parse(code) {
            state.apply_link(change);
            EventKind::Hyperlink
        } else if let Some(params) = sgr_params(code) {
            state.apply_sgr(params);
            EventKind::Sgr
        } else {
            EventKind::Opaque
        };
        let index = if kind == EventKind::Opaque {
            self.events.last().map_or(0, |event| event.state)
        } else {
            self.states.push(state);
            self.states.len() - 1
        };
        self.events.push(Event {
            offset: self.visible.len(),
            raw: code,
            kind,
            state: index,
        });
    }

    /// Visible text of a grapheme.
    pub(super) fn text(&self, grapheme: &Grapheme) -> &str {
        &self.visible[grapheme.range.clone()]
    }

    /// Escapes written directly before a grapheme, outside its text.
    pub(super) fn leading_events(&self, grapheme: &Grapheme) -> Range<usize> {
        let start = grapheme.events.start;
        let leading = self.events[grapheme.events.clone()]
            .iter()
            .take_while(|event| event.offset == grapheme.range.start)
            .count();
        start..start + leading
    }

    /// Byte ranges of the lines separated by literal newlines.
    pub(super) fn lines(&self) -> Vec<Range<usize>> {
        let mut start = 0;
        let mut lines: Vec<_> = self
            .visible
            .match_indices('\n')
            .map(|(end, _)| {
                let line = start..end;
                start = end + 1;
                line
            })
            .collect();
        lines.push(start..self.visible.len());
        lines
    }

    /// Indices of the escapes anchored in `span`, endpoints included.
    pub(super) fn events_in(&self, span: &Range<usize>) -> Range<usize> {
        let first = self
            .events
            .partition_point(|event| event.offset < span.start);
        let end = self
            .events
            .partition_point(|event| event.offset <= span.end);
        first..end
    }

    /// Graphemes of `span`; a tab occupies `tab_cells` cells.
    pub(super) fn graphemes(
        &self,
        span: Range<usize>,
        tab_cells: usize,
    ) -> impl Iterator<Item = Grapheme> + '_ {
        let mut cursor = self.events_in(&span).start;
        let base = span.start;
        self.visible[span]
            .grapheme_indices(true)
            .map(move |(offset, text)| {
                let range = base + offset..base + offset + text.len();
                let first = cursor;
                while self
                    .events
                    .get(cursor)
                    .is_some_and(|event| event.offset < range.end)
                {
                    cursor += 1;
                }
                Grapheme {
                    cells: if text == "\t" {
                        tab_cells
                    } else {
                        grapheme_cells(text)
                    },
                    state: first
                        .checked_sub(1)
                        .map_or(0, |last| self.events[last].state),
                    events: first..cursor,
                    range,
                }
            })
    }
}
