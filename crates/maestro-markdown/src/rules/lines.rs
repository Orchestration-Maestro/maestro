//! Shared monotone physical-line framing for retained block grammar.
use std::{borrow::Cow, ops::Range};

/// Logical grammar input with bounded contiguous operands, independent of storage.
pub(crate) trait Input {
    /// Logical UTF-8 length of the current source prefix.
    fn len(&self) -> usize;

    /// Visit a window without inspecting its preceding logical bytes.
    fn chunks(&self, range: Range<usize>) -> impl Iterator<Item = &str>;

    /// Borrow contiguous grammar operands and join only a split window.
    fn window(&self, range: Range<usize>) -> Cow<'_, str> {
        let mut chunks = self.chunks(range.clone());
        let first = chunks.next().unwrap_or("");
        let Some(second) = chunks.next() else {
            return Cow::Borrowed(first);
        };
        let mut text = String::with_capacity(range.len());
        text.push_str(first);
        text.push_str(second);
        for chunk in chunks {
            text.push_str(chunk);
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.joined_bytes += text.len());
        Cow::Owned(text)
    }
}

impl Input for str {
    fn len(&self) -> usize {
        str::len(self)
    }

    fn chunks(&self, range: Range<usize>) -> impl Iterator<Item = &str> {
        std::iter::once(&self[range])
    }
}

impl Input for crate::source::Segmented {
    fn len(&self) -> usize {
        self.len()
    }

    fn chunks(&self, range: Range<usize>) -> impl Iterator<Item = &str> {
        self.chunks_in(range)
    }
}

/// Offsets into an append-only operand, with no borrowed or copied source.
#[derive(Default)]
pub(crate) struct Cursor {
    /// Bytes already searched for a newline.
    scanned: usize,
    /// Start of the current incomplete line.
    start: usize,
}

impl Cursor {
    /// Return one complete line, searching each newly supplied byte once.
    pub(crate) fn next(&mut self, source: &(impl Input + ?Sized)) -> Option<Range<usize>> {
        let mut scanned = self.scanned;
        let mut end = None;
        for chunk in source.chunks(self.scanned..source.len()) {
            if let Some(offset) = chunk.find('\n') {
                scanned += offset + 1;
                end = Some(scanned);
                break;
            }
            scanned += chunk.len();
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.framing_bytes += scanned - self.scanned);
        self.scanned = scanned;
        let end = end?;
        let line = self.start..end;
        self.start = end;
        Some(line)
    }

    /// Return the final unterminated line at an explicit operand boundary.
    pub(crate) fn finish(&mut self, source: &(impl Input + ?Sized)) -> Option<Range<usize>> {
        let line = (self.start < source.len()).then_some(self.start..source.len());
        self.start = source.len();
        line
    }
}

#[cfg(test)]
mod tests;
