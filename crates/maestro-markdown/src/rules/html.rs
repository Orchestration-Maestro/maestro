//! Retained HTML block boundaries over append-only source prefixes.
use regress::Regex;
use std::{ops::Range, sync::LazyLock};

/// Opening grammar only; body and separator progress belong to the retained cursor.
static OPEN: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(&[
        r"^ {0,3}(?:<(script|pre|style|textarea)[\s>]|(<!--)|(<\?)|(<![A-Z])|(<!\[CDATA\[)|</?(?:",
        super::TAG,
        r#")(?: +|\n|/?>)|<(?!script|pre|style|textarea)[a-z][\w-]*(?: +[a-zA-Z:_][\w.:-]*(?: *= *"[^"\n]*"| *= *'[^'\n]*'| *= *[^\s"'=<>`]+)?)*? */?>(?=[ \t]*(?:\n|$))|</(?!script|pre|style|textarea)[a-z][\w-]*\s*>(?=[ \t]*(?:\n|$)))"#,
    ].concat(), "i").ok()
});

/// A closing tag can put whitespace and its final angle on separate physical lines.
static CLOSING: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"^ {0,3}</(?!script|pre|style|textarea)[a-z][\w-]*\s*$",
        "i",
    )
    .ok()
});

/// State selected by the HTML opener, never by body text.
enum Phase {
    /// Inspect the first physical line.
    Open,
    /// Finish the whitespace-only suffix of a multiline closing tag.
    Closing,
    /// Search a fixed closing delimiter, with an optional same-line suffix.
    Delimiter(String, bool),
    /// A tag block ends at its first blank line.
    Blank,
    /// A closed directive or raw-text block consumes only immediately following newlines.
    Newlines,
    /// Continue complete whitespace-only separator lines.
    BlankTail,
    /// No further input can change the result.
    Done,
}

/// Whether a later source prefix can change the consumed extent.
pub(crate) enum Verdict {
    /// More bytes or the final boundary are required.
    NeedMore,
    /// The opener failed or the end is fixed.
    Stable(Option<usize>),
}

/// Monotone HTML framing with no retained source copy.
pub(crate) struct Recognizer {
    /// Shared physical-line progress.
    lines: super::lines::Cursor,
    /// Current opener/body/terminator state.
    phase: Phase,
    /// Last admitted byte, absent for a rejected opener.
    consumed: Option<usize>,
}

impl Default for Recognizer {
    fn default() -> Self {
        Self {
            lines: super::lines::Cursor::default(),
            phase: Phase::Open,
            consumed: None,
        }
    }
}

impl Recognizer {
    /// Advance over complete physical lines, each once.
    pub(crate) fn extend(&mut self, source: &str) -> Verdict {
        while !matches!(self.phase, Phase::Done) {
            let Some(line) = self.lines.next(source) else {
                break;
            };
            self.consume(source, line);
        }
        if matches!(self.phase, Phase::Done) {
            Verdict::Stable(self.consumed)
        } else {
            Verdict::NeedMore
        }
    }

    /// Resolve an unterminated final line at the operand's final boundary.
    pub(crate) fn finish(&mut self, source: &str) -> Option<usize> {
        if let Verdict::Stable(result) = self.extend(source) {
            return result;
        }
        if let Some(line) = self.lines.finish(source) {
            self.consume(source, line);
        }
        if matches!(self.phase, Phase::Closing) {
            self.consumed = None;
        }
        self.phase = Phase::Done;
        self.consumed
    }

    /// Inspect one line for an opener or the selected family's boundary.
    fn consume(&mut self, source: &str, line: Range<usize>) {
        let text = &source[line.clone()];
        charge(text.len());
        if matches!(self.phase, Phase::Open) {
            self.open(text);
        }
        match &self.phase {
            Phase::Open | Phase::Done => {}
            Phase::Closing => {
                if text
                    .trim_matches(maestro_text::is_whitespace_scalar)
                    .is_empty()
                {
                    return;
                }
                if line.start > 0
                    && text
                        .trim_start_matches(maestro_text::is_whitespace_scalar)
                        .strip_prefix('>')
                        .is_some_and(|tail| tail.trim_matches([' ', '\t', '\n']).is_empty())
                {
                    self.phase = Phase::Blank;
                    self.consumed = Some(line.end);
                } else if line.start > 0 {
                    self.phase = Phase::Done;
                }
            }
            Phase::Delimiter(delimiter, tail) => {
                self.consumed = Some(line.end);
                if let Some((end, phase)) = delimiter_end(text, &line, delimiter, *tail) {
                    self.consumed = Some(end);
                    self.phase = phase;
                }
            }
            Phase::Blank => {
                self.consumed = Some(line.end);
                if text == "\n" || line.start > 0 && text.trim_matches([' ', '\t', '\n']).is_empty()
                {
                    self.phase = Phase::BlankTail;
                }
            }
            Phase::BlankTail => {
                if text.ends_with('\n') && text.trim_matches([' ', '\t', '\n']).is_empty() {
                    self.consumed = Some(line.end);
                } else {
                    self.phase = Phase::Done;
                }
            }
            Phase::Newlines => {
                let count = text.bytes().take_while(|byte| *byte == b'\n').count();
                self.consumed = Some(line.start + count);
                if count < text.len() {
                    self.phase = Phase::Done;
                }
            }
        }
    }

    /// Select the opener's delimiter family without presenting the remaining body to regex.
    fn open(&mut self, text: &str) {
        let Some(matched) = OPEN.as_ref().and_then(|rule| rule.find(text)) else {
            self.phase = if CLOSING
                .as_ref()
                .is_some_and(|rule| rule.find(text).is_some())
            {
                Phase::Closing
            } else {
                Phase::Done
            };
            return;
        };
        self.phase = if let Some(tag) = matched.group(1) {
            Phase::Delimiter(format!("</{}>", &text[tag]), true)
        } else if matched.group(2).is_some() {
            Phase::Delimiter("-->".to_owned(), true)
        } else if matched.group(3).is_some() {
            Phase::Delimiter("?>".to_owned(), false)
        } else if matched.group(4).is_some() {
            Phase::Delimiter(">".to_owned(), false)
        } else if matched.group(5).is_some() {
            Phase::Delimiter("]]>".to_owned(), false)
        } else {
            Phase::Blank
        };
    }
}

/// Charge HTML-local predicates separately from physical-line framing.
pub(crate) fn charge(bytes: usize) {
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.html_bytes += bytes);
    #[cfg(not(test))]
    let _ = bytes;
}

/// Find the selected delimiter in one line and retain its permitted trailing bytes.
fn delimiter_end(
    text: &str,
    line: &Range<usize>,
    delimiter: &str,
    tail: bool,
) -> Option<(usize, Phase)> {
    let indent = text.bytes().take_while(|byte| *byte == b' ').count();
    let opening = if text[indent..].starts_with("<!--") {
        4
    } else {
        2
    };
    let start = if line.start == 0 {
        (indent + opening).min(text.len())
    } else {
        0
    };
    let body = &text[start..];
    let special =
        line.start == 0 && delimiter == "-->" && (body.starts_with('>') || body.starts_with("->"));
    let end = body
        .as_bytes()
        .windows(delimiter.len())
        .position(|part| part.eq_ignore_ascii_case(delimiter.as_bytes()));
    if !special && end.is_none() {
        return None;
    }
    let end = if tail {
        text.len()
    } else {
        start + end? + delimiter.len()
    };
    let newlines = text[end..]
        .bytes()
        .take_while(|byte| *byte == b'\n')
        .count();
    let end = line.start + end + newlines;
    let phase = if end == line.end {
        Phase::Newlines
    } else {
        Phase::Done
    };
    Some((end, phase))
}
