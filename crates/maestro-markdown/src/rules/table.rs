//! Retained table captures over growing source, using bounded header and row grammar.
use crate::MarkedOptions;
use regress::Regex;
use std::{ops::Range, sync::LazyLock};

/// Source captures independent of table cell interpretation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Table {
    /// Header excluding its line terminator.
    pub(crate) header: Range<usize>,
    /// Alignment spelling excluding its line terminator.
    pub(crate) delimiter: Range<usize>,
    /// Body rows, absent when the delimiter ends at the input boundary.
    pub(crate) body: Option<Range<usize>>,
    /// Raw end including trailing empty lines.
    pub(crate) consumed: usize,
}

/// Admission lifetime at an append-only operand.
pub(crate) enum Verdict {
    /// More bytes can change captures or consumption.
    NeedMore,
    /// Later source cannot change this result.
    Stable(Option<Table>),
}

/// Table admission position without overlapping state flags.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Awaiting the header's physical line end.
    Header,
    /// Awaiting the delimiter's physical line end.
    Delimiter,
    /// Admitting ordinary body rows.
    Body,
    /// Consuming only empty lines after the body ends.
    Trailing,
    /// Later bytes cannot change the captures.
    Stable,
}

/// Compile one complete row with the shared block interruptions.
fn row(html: &str) -> Option<Regex> {
    super::anchored(
        &[
            r"(?!(?: *\n|",
            &super::interruption(true, html),
            r")).*(?:\n|$)",
        ]
        .concat(),
    )
}

/// Shared header spelling also used by paragraph interruption.
static HEADER: LazyLock<Option<Regex>> =
    LazyLock::new(|| super::anchored(&[super::TABLE_HEADER, r"(?:\n|$)"].concat()));
/// Ordinary caller row interruptions.
static ROW: LazyLock<Option<Regex>> = LazyLock::new(|| row(&super::html_openers()));
/// Literal-markup caller row interruptions.
static LITERAL_ROW: LazyLock<Option<Regex>> = LazyLock::new(|| row(super::LITERAL_HTML_OPENERS));

/// Monotone line framing; only capture offsets survive each supplied prefix.
pub(crate) struct Recognizer {
    /// Immutable caller row policy.
    row: Option<&'static Regex>,
    /// Current header, body or final-boundary position.
    phase: Phase,
    /// Bytes already searched for line endings.
    scanned: usize,
    /// Beginning of the current incomplete physical line.
    line_start: usize,
    /// Captures once a complete header and delimiter have been admitted.
    result: Option<Table>,
}

impl Recognizer {
    /// Begin one operand in the caller's table policy.
    pub(crate) fn new(options: MarkedOptions) -> Self {
        Self {
            row: if options == (MarkedOptions::Browser { escape_html: true }) {
                LITERAL_ROW.as_ref()
            } else {
                ROW.as_ref()
            },
            phase: if options == MarkedOptions::Documentation {
                Phase::Stable
            } else {
                Phase::Header
            },
            scanned: 0,
            line_start: 0,
            result: None,
        }
    }

    /// Supply an extended prefix without revisiting completed physical lines.
    pub(crate) fn extend(&mut self, source: &str) -> Verdict {
        while self.phase != Phase::Stable && self.scanned < source.len() {
            let rest = &source[self.scanned..];
            let count = rest.find('\n').map_or(rest.len(), |end| end + 1);
            charge(count);
            self.scanned += count;
            if source.as_bytes()[self.scanned - 1] == b'\n' {
                self.consume(source, self.line_start..self.scanned);
                self.line_start = self.scanned;
            }
        }
        if self.phase == Phase::Stable {
            Verdict::Stable(self.result.clone())
        } else {
            Verdict::NeedMore
        }
    }

    /// Settle the final unterminated line or delimiter at the caller's boundary.
    pub(crate) fn finish(&mut self, source: &str) -> Option<Table> {
        if let Verdict::Stable(result) = self.extend(source) {
            return result;
        }
        if self.line_start < source.len() {
            self.consume(source, self.line_start..source.len());
        }
        self.phase = Phase::Stable;
        self.result.clone()
    }

    /// Dispatch one complete line to the header, body or trailing-newline phase.
    fn consume(&mut self, source: &str, line: Range<usize>) {
        if self.phase == Phase::Header {
            self.phase = Phase::Delimiter;
        } else if self.phase == Phase::Delimiter {
            let header = &source[..line.end];
            charge(header.len());
            self.result = HEADER
                .as_ref()
                .and_then(|rule| rule.find(header))
                .and_then(|matched| {
                    Some(Table {
                        header: matched.group(1)?,
                        delimiter: matched.group(2)?,
                        body: header.ends_with('\n').then_some(line.end..line.end),
                        consumed: line.end,
                    })
                });
            self.phase = if self.result.is_some() {
                Phase::Body
            } else {
                Phase::Stable
            };
        } else {
            self.body_line(source, line);
        }
    }

    /// Preserve the body capture separately from greedily consumed empty lines.
    fn body_line(&mut self, source: &str, line: Range<usize>) {
        let text = &source[line.clone()];
        charge(text.len());
        let admitted =
            self.phase == Phase::Body && self.row.is_some_and(|rule| rule.find(text).is_some());
        let Some(result) = &mut self.result else {
            return;
        };
        if admitted {
            if let Some(body) = &mut result.body {
                body.end = line.end;
            }
            result.consumed = line.end;
        } else if text == "\n" {
            self.phase = Phase::Trailing;
            result.consumed = line.end;
        } else {
            self.phase = Phase::Stable;
        }
    }
}

/// Charge bytes at their actual framing or grammar-consumption boundary.
fn charge(bytes: usize) {
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.table_bytes += bytes);
    #[cfg(not(test))]
    let _ = bytes;
}
