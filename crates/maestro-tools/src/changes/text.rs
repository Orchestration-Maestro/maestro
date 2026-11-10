//! Line change sequences: the shortest edit script over line tokens, choosing among
//! equally short scripts the way the edit tool's numbered diffs require.

use crate::changes::sequence::{Kind, diff_sequence};

/// One run of consecutive lines; each value keeps its line separators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change<'a> {
    /// Lines both texts share, taken from the new text.
    Unchanged(&'a str),
    /// Lines only the new text has.
    Added(&'a str),
    /// Lines only the old text has.
    Removed(&'a str),
}

/// One text and how much of it the runs consumed.
struct Side<'a> {
    /// The text.
    text: &'a str,
    /// Its line tokens.
    tokens: Vec<&'a str>,
    /// Tokens consumed.
    token: usize,
    /// Bytes consumed.
    byte: usize,
}

impl<'a> Side<'a> {
    /// Split `text` after every LF; a final line without LF is a token too.
    fn new(text: &'a str) -> Self {
        Self {
            text,
            tokens: text.split_inclusive('\n').collect(),
            token: 0,
            byte: 0,
        }
    }

    /// The next `count` tokens as one slice.
    fn take(&mut self, count: usize) -> &'a str {
        let end = self.byte
            + self.tokens[self.token..self.token + count]
                .iter()
                .map(|t| t.len())
                .sum::<usize>();
        let value = &self.text[self.byte..end];
        (self.token, self.byte) = (self.token + count, end);
        value
    }
}

/// The line change sequence turning `old` into `new`.
#[must_use]
pub fn diff_lines<'a>(old: &'a str, new: &'a str) -> Vec<Change<'a>> {
    let (mut old, mut new) = (Side::new(old), Side::new(new));
    diff_sequence(&old.tokens, &new.tokens)
        .into_iter()
        .map(|(kind, count)| match kind {
            Kind::Unchanged => {
                old.take(count);
                Change::Unchanged(new.take(count))
            }
            Kind::Added => Change::Added(new.take(count)),
            Kind::Removed => Change::Removed(old.take(count)),
        })
        .collect()
}
