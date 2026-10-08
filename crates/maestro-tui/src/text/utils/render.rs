//! Emission of selected graphemes and escapes, and the closing of their endpoints.

use std::ops::Range;

use super::parsed::{Grapheme, Parsed};

/// Terminal reset that ends a truncated result.
pub(super) const RESET: &str = "\x1b[0m";

/// One output string under construction, tracking the style the terminal is left in.
pub(super) struct Emission<'p, 'a> {
    /// Source of the text and escapes being emitted.
    parsed: &'p Parsed<'a>,
    /// Text emitted so far.
    out: String,
    /// Index of the style state the emitted escapes leave the terminal in.
    state: usize,
}

impl<'p, 'a> Emission<'p, 'a> {
    /// Starts an empty output from a clean terminal.
    pub(super) const fn new(parsed: &'p Parsed<'a>) -> Self {
        Self {
            parsed,
            out: String::new(),
            state: 0,
        }
    }

    /// Establishes the canonical escapes of an inherited style state.
    pub(super) fn inherit(&mut self, state: usize) {
        self.parsed.states[state].write_active(&mut self.out);
        self.state = state;
    }

    /// Emits escapes verbatim, in order.
    pub(super) fn events(&mut self, range: Range<usize>) {
        for event in &self.parsed.events[range] {
            self.out.push_str(event.raw);
            if event.is_style() {
                self.state = event.state;
            }
        }
    }

    /// Emits only the metadata escapes, leaving style escapes out.
    pub(super) fn opaque_events(&mut self, range: Range<usize>) {
        for event in self.parsed.events[range]
            .iter()
            .filter(|event| !event.is_style())
        {
            self.out.push_str(event.raw);
        }
    }

    /// Emits a grapheme with every escape anchored at or inside it.
    pub(super) fn grapheme(&mut self, grapheme: &Grapheme) {
        let leading = self.parsed.leading_events(grapheme);
        self.events(leading);
        self.content(grapheme);
    }

    /// Emits the text of a grapheme with the escapes strictly inside it.
    pub(super) fn content(&mut self, grapheme: &Grapheme) {
        let leading = self.parsed.leading_events(grapheme).end;
        let mut position = grapheme.range.start;
        for index in leading..grapheme.events.end {
            let offset = self.parsed.events[index].offset;
            self.out.push_str(&self.parsed.visible[position..offset]);
            self.events(index..index + 1);
            position = offset;
        }
        self.out
            .push_str(&self.parsed.visible[position..grapheme.range.end]);
    }

    /// Closes an open hyperlink with its opener's terminator.
    pub(super) fn close_link(&mut self) {
        self.parsed.states[self.state].write_link_close(&mut self.out);
    }

    /// Ends the output, optionally switching underline off first, and closes any link.
    pub(super) fn finish(mut self, close_underline: bool) -> String {
        if close_underline {
            self.parsed.states[self.state].write_underline_close(&mut self.out);
        }
        self.close_link();
        self.out
    }

    /// Returns the text emitted so far without closing anything.
    pub(super) fn into_string(self) -> String {
        self.out
    }
}
