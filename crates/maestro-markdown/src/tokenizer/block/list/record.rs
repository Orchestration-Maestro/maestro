//! What an item recorded about its body lines for the lists inside the body.
use maestro_text::is_whitespace_scalar;
use std::rc::Rc;

/// How a recorded body line is spelled, as far as a list inside the body needs to admit it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Shape {
    /// A tab, or whitespace other than spaces on an otherwise blank line, which an item
    /// decides from the line itself.
    Other,
    /// Only spaces, which every item indentation admits.
    Blank,
    /// Leading spaces then text.
    Text {
        /// Leading spaces.
        columns: usize,
        /// Bytes through the last non-whitespace scalar.
        trimmed: usize,
    },
}

impl Shape {
    /// The spelling of one line, without its terminator.
    pub(super) fn of(line: &str) -> Self {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.framing_bytes += line.len());
        if line.contains('\t') {
            return Self::Other;
        }
        let columns = line.bytes().take_while(|byte| *byte == b' ').count();
        match line.trim_end_matches(is_whitespace_scalar).len() {
            0 if columns == line.len() => Self::Blank,
            0 => Self::Other,
            trimmed => Self::Text { columns, trimmed },
        }
    }

    /// The spelling once `indent` leading spaces are cut, when an item with that
    /// indentation admits the line as aligned.
    pub(super) fn dedent(self, indent: usize) -> Option<(usize, Self)> {
        match self {
            Self::Other => None,
            Self::Blank => Some((0, Self::Blank)),
            Self::Text { columns, trimmed } => (columns >= indent).then(|| {
                (
                    indent,
                    Self::Text {
                        columns: columns - indent,
                        trimmed: trimmed - indent,
                    },
                )
            }),
        }
    }
}

/// One terminated body line an item recorded.
#[derive(Clone, Copy)]
pub(super) struct BodyLine {
    /// Logical end of the line, terminator included.
    pub(super) end: usize,
    /// Spelling of the line.
    pub(super) shape: Shape,
}

/// The lines an item recorded while it admitted its body, so a list inside that body
/// frames each physical line without scanning it again and takes the aligned ones as
/// recorded.
#[derive(Clone)]
pub(crate) struct LineEnds {
    /// Each terminated body line, in ascending order of end.
    pub(super) ends: Rc<Vec<BodyLine>>,
    /// Start of the framed operand within the recorded body.
    pub(super) base: usize,
}

impl LineEnds {
    /// The same line ends for an operand that starts `offset` bytes later.
    pub(crate) fn after(&self, offset: usize) -> Self {
        Self {
            ends: Rc::clone(&self.ends),
            base: self.base + offset,
        }
    }

    /// End of the physical line that starts at `offset` in an operand of `length` bytes,
    /// which is where a scan for its terminator would stop.
    pub(super) fn line_end(&self, offset: usize, length: usize) -> usize {
        let start = self.base + offset;
        let index = self.ends.partition_point(|line| line.end <= start);
        self.ends
            .get(index)
            .map_or(length, |line| (line.end - self.base).min(length))
    }

    /// The terminated lines of an operand of `length` bytes from the one that starts at
    /// `offset`, as each line's end in the operand and its spelling.
    pub(super) fn recorded(
        &self,
        offset: usize,
        length: usize,
    ) -> impl Iterator<Item = (usize, Shape)> {
        let start = self.base + offset;
        let index = self.ends.partition_point(|line| line.end <= start);
        let begins = index
            .checked_sub(1)
            .map_or(0, |previous| self.ends[previous].end);
        let from = if begins == start {
            index
        } else {
            self.ends.len()
        };
        self.ends[from..]
            .iter()
            .map(|line| (line.end - self.base, line.shape))
            .take_while(move |(end, _)| *end <= length)
    }
}
