//! Retained paragraph and setext progress over a caller's growing source prefix.
use crate::MarkedOptions;
use regress::Regex;
use std::{collections::VecDeque, ops::Range, sync::LazyLock};

/// Setext captures independent of source ownership.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Heading {
    /// Label excluding the newline before its underline.
    pub(crate) content: Range<usize>,
    /// Semantic heading level.
    pub(crate) level: usize,
    /// End of the underline and its trailing newlines.
    pub(crate) consumed: usize,
}

/// Independent results: setext precedes paragraph dispatch.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub(crate) struct Recognized {
    /// First setext heading, if its label grammar remains eligible.
    pub(crate) heading: Option<Heading>,
    /// End of paragraph text when no heading is admitted, excluding its final newline.
    pub(crate) paragraph: Option<usize>,
}

/// Whether a growing operand has reached an immutable dispatch result.
pub(crate) enum Verdict {
    /// A later underline, delimiter, or newline can still change admission.
    NeedMore,
    /// Higher-priority alternatives have finished.
    Stable(Recognized),
}

/// Input lifetime after an underline or an explicit final boundary.
enum Boundary {
    /// More source may still be supplied.
    Open,
    /// A non-newline settled an admitted underline's extent.
    HeadingClosed,
    /// The caller supplied the complete operand.
    Finished,
}

/// Line-local predicates compiled once for each distinct caller grammar.
struct Grammar {
    /// Paragraph interruptions, including only a table's header and delimiter.
    paragraph: Option<Regex>,
    /// Setext label exclusions, without the later-line blank check.
    setext: Option<Regex>,
}

/// Compile the same interruptions used by quote and table admission.
fn grammar(options: MarkedOptions) -> Grammar {
    let html = if options == (MarkedOptions::Browser { escape_html: true }) {
        super::LITERAL_HTML_OPENERS.to_owned()
    } else {
        super::html_openers()
    };
    let table = if options == MarkedOptions::Documentation {
        ""
    } else {
        super::TABLE_HEADER
    };
    let paragraph = [
        super::interruption(false, &html),
        if table.is_empty() { "" } else { "|" }.to_owned(),
        if table.is_empty() {
            String::new()
        } else {
            [table, r"(?:\n|$)"].concat()
        },
        "| +\\n".to_owned(),
    ]
    .concat();
    let tag = if options == (MarkedOptions::Browser { escape_html: true }) {
        r"(?![A-Za-z0-9_]|\/[A-Za-z0-9_]+>|\s)"
    } else {
        ""
    };
    let table = if matches!(
        options,
        MarkedOptions::Export | MarkedOptions::Documentation
    ) {
        ""
    } else {
        r"| {0,3}\|?(?:[:\- ]*\|)+[\:\- ]*\n"
    };
    let setext = [
        r"(?:[*+-]|\d{1,9}[.)]) |(?: {4}| {0,3}\t)| {0,3}(?:`{3,}|~{3,})| {0,3}>| {0,3}#{1,6}| {0,3}<",
        tag, r"[^\n>]+>\n", table,
    ].concat();
    Grammar {
        paragraph: super::anchored(&["(?:", &paragraph, ")"].concat()),
        setext: super::anchored(&["(?:", &setext, ")"].concat()),
    }
}

/// Immutable grammar alternatives in the public option order.
static GRAMMARS: LazyLock<[Grammar; 4]> = LazyLock::new(|| {
    [
        grammar(MarkedOptions::Terminal),
        grammar(MarkedOptions::Documentation),
        grammar(MarkedOptions::Browser { escape_html: true }),
        grammar(MarkedOptions::Export),
    ]
});

/// A complete setext underline, with the original capture order.
static UNDERLINE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^ {0,3}(=+|-+) *(?:\n+|$)", "").ok());

/// Monotone line framing; at most two completed lines await lookahead.
pub(crate) struct Recognizer {
    /// Shared immutable caller predicates.
    grammar: &'static Grammar,
    /// Bytes already searched for physical line ends.
    scanned: usize,
    /// Beginning of the not-yet-complete physical line.
    line_start: usize,
    /// Complete lines waiting for one line of lookahead.
    lines: VecDeque<Range<usize>>,
    /// Results retained independently of subsequent input.
    result: Recognized,
    /// Whether another ordinary paragraph line can still be admitted.
    paragraph_open: bool,
    /// Whether another label line or underline can still be admitted.
    setext_open: bool,
    /// Whether a non-newline after the underline has settled its raw end.
    boundary: Boundary,
}

impl Recognizer {
    /// Begin one block operand without retaining its authored bytes.
    pub(crate) fn new(options: MarkedOptions) -> Self {
        let index = match options {
            MarkedOptions::Terminal | MarkedOptions::Browser { escape_html: false } => 0,
            MarkedOptions::Documentation => 1,
            MarkedOptions::Browser { escape_html: true } => 2,
            MarkedOptions::Export => 3,
        };
        Self {
            grammar: &GRAMMARS[index],
            scanned: 0,
            line_start: 0,
            lines: VecDeque::new(),
            result: Recognized::default(),
            paragraph_open: true,
            setext_open: true,
            boundary: Boundary::Open,
        }
    }

    /// Supply an extended prefix; previously framed bytes are never searched again.
    pub(crate) fn extend(&mut self, source: &str) -> Verdict {
        if self.settled() {
            return Verdict::Stable(self.result.clone());
        }
        while self.scanned < source.len() {
            let remaining = &source[self.scanned..];
            let count = remaining.find('\n').map_or(remaining.len(), |end| end + 1);
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.paragraph_bytes += count);
            self.scanned += count;
            if source.as_bytes()[self.scanned - 1] != b'\n' {
                continue;
            }
            self.lines.push_back(self.line_start..self.scanned);
            self.line_start = self.scanned;
            if self.lines.len() == 2 {
                self.consume(source);
            }
            if self.settled() {
                return Verdict::Stable(self.result.clone());
            }
        }
        Verdict::NeedMore
    }

    /// Resolve the caller-owned final boundary, including an unterminated last line.
    pub(crate) fn finish(&mut self, source: &str) -> Recognized {
        if let Verdict::Stable(result) = self.extend(source) {
            return result;
        }
        if self.line_start < source.len() && !self.settled() {
            self.lines.push_back(self.line_start..source.len());
            self.line_start = source.len();
        }
        while !self.lines.is_empty() && !self.settled() {
            self.consume(source);
        }
        self.boundary = Boundary::Finished;
        self.result.clone()
    }

    /// A heading settles before the lower-priority paragraph alternative.
    fn settled(&self) -> bool {
        !matches!(self.boundary, Boundary::Open) || !self.paragraph_open && !self.setext_open
    }

    /// Admit one line with the next line available only as grammar lookahead.
    fn consume(&mut self, source: &str) {
        let Some(line) = self.lines.pop_front() else {
            return;
        };
        let end = self.lines.front().map_or(line.end, |next| next.end);
        let window = &source[line.start..end];
        let text = &source[line.clone()];
        self.heading_line(text, window, &line);
        if self.paragraph_open {
            let content = text.strip_suffix('\n').unwrap_or(text);
            if content.is_empty()
                || line.start != 0 && matches(self.grammar.paragraph.as_ref(), window)
            {
                self.paragraph_open = false;
            } else {
                self.result.paragraph = Some(line.start + content.len());
            }
        }
    }

    /// Prefer an underline before testing whether another label line is legal.
    fn heading_line(&mut self, text: &str, window: &str, line: &Range<usize>) {
        if let Some(heading) = &mut self.result.heading {
            if text == "\n" && matches!(self.boundary, Boundary::Open) {
                heading.consumed = line.end;
            } else {
                self.boundary = Boundary::HeadingClosed;
            }
            return;
        }
        if !self.setext_open {
            return;
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.paragraph_bytes += text.len());
        if line.start > 1
            && let Some(underline) = UNDERLINE.as_ref().and_then(|rule| rule.find(text))
        {
            self.result.heading = Some(Heading {
                content: 0..line.start - 1,
                level: usize::from(text[underline.group(1).unwrap_or_default()].starts_with('-'))
                    + 1,
                consumed: line.start + underline.end(),
            });
            return;
        }
        let blank = line.start != 0
            && text.ends_with('\n')
            && text.chars().all(|scalar| {
                #[cfg(test)]
                crate::tests::WORK
                    .with_borrow_mut(|work| work.paragraph_bytes += scalar.len_utf8());
                maestro_text::is_whitespace_scalar(scalar)
            });
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.paragraph_bytes += text.len());
        self.setext_open = !blank
            && !matches(self.grammar.setext.as_ref(), window)
            && !text.contains(['\r', '\u{2028}', '\u{2029}']);
    }
}

/// Observe a bounded grammar operand and charge it at its owning boundary.
fn matches(rule: Option<&Regex>, source: &str) -> bool {
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.paragraph_bytes += source.len());
    rule.is_some_and(|rule| rule.find(source).is_some())
}
