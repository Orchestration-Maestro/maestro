//! Retained fenced-code captures over append-only source prefixes.
use regress::Regex;
use std::{ops::Range, sync::LazyLock};

/// An admitted fence independent of source ownership.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fence {
    /// Opening indentation removed from eligible body lines.
    pub(crate) indent: usize,
    /// Language spelling before decoding.
    pub(crate) info: Range<usize>,
    /// Body excluding the newline before the closing fence or final boundary.
    pub(crate) body: Option<Range<usize>>,
    /// Raw end excluding a closing fence's following newline.
    pub(crate) consumed: usize,
}

/// Whether another source prefix can affect admission.
pub(crate) enum Verdict {
    /// The opening line, body or closing line is incomplete.
    NeedMore,
    /// No later bytes can change these captures.
    Stable(Option<Fence>),
}

/// Opening grammar, applied only to the first physical line.
static OPEN: LazyLock<Option<Regex>> =
    LazyLock::new(|| super::anchored(r" {0,3}(`{3,}(?=[^`\n]*(?:\n|$))|~{3,})([^\n]*)(?:\n|$)"));

/// Monotone physical-line cursor retaining only offsets and captures.
#[derive(Default)]
pub(crate) struct Recognizer {
    /// Shared monotone physical-line framing.
    lines: super::lines::Cursor,
    /// Opening delimiter, absent before admission.
    delimiter: Option<Range<usize>>,
    /// First byte of possible body content.
    body_start: usize,
    /// Captures retained after the opening line.
    result: Option<Fence>,
    /// Whether a rejected opener or a closing fence settled the result.
    stable: bool,
}

impl Recognizer {
    /// Supply an extended prefix without revisiting completed lines.
    pub(crate) fn extend(&mut self, source: &str) -> Verdict {
        while !self.stable {
            let Some(line) = self.lines.next(source) else {
                break;
            };
            self.consume(source, line);
        }
        if self.stable {
            Verdict::Stable(self.result.clone())
        } else {
            Verdict::NeedMore
        }
    }

    /// Settle an unterminated opener, closing line or body at the final boundary.
    pub(crate) fn finish(&mut self, source: &str) -> Option<Fence> {
        if let Verdict::Stable(result) = self.extend(source) {
            return result;
        }
        if let Some(line) = self.lines.finish(source) {
            self.consume(source, line);
        }
        if !self.stable
            && let Some(result) = &mut self.result
        {
            result.consumed = source.len();
            result.body = (source.len() > self.body_start)
                .then(|| self.body_start..source.len() - usize::from(source.ends_with('\n')));
        }
        self.stable = true;
        self.result.clone()
    }

    /// Admit one opener or test one complete body line for closure.
    fn consume(&mut self, source: &str, line: Range<usize>) {
        let text = &source[line.clone()];
        charge(text.len());
        let Some(delimiter) = &self.delimiter else {
            let matched = OPEN.as_ref().and_then(|rule| rule.find(text));
            self.result = matched.as_ref().and_then(|matched| {
                self.delimiter = matched.group(1);
                self.body_start = matched.end();
                Some(Fence {
                    indent: self.delimiter.as_ref()?.start,
                    info: matched.group(2)?,
                    body: None,
                    consumed: matched.end(),
                })
            });
            self.stable = self.result.is_none();
            return;
        };
        let content = text.strip_suffix('\n').unwrap_or(text);
        let indent = content.bytes().take_while(|byte| *byte == b' ').count();
        let closed = indent <= 3
            && content[indent..]
                .strip_prefix(&source[delimiter.clone()])
                .is_some_and(|tail| {
                    tail.trim_start_matches(['~', '`'])
                        .bytes()
                        .all(|byte| byte == b' ')
                });
        if closed && let Some(result) = &mut self.result {
            result.consumed = line.start + content.len();
            result.body = (line.start > self.body_start)
                .then_some(self.body_start..line.start.saturating_sub(1));
            self.stable = true;
        }
    }
}

/// Charge line-local grammar operands at the owner.
fn charge(bytes: usize) {
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.fence_bytes += bytes);
    #[cfg(not(test))]
    let _ = bytes;
}
