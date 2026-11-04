//! Shared monotone physical-line framing for retained block grammar.
use std::{borrow::Cow, ops::Range};

/// Logical grammar input with bounded contiguous operands, independent of storage.
pub(crate) trait Input {
    /// Logical UTF-8 length of the current source prefix.
    fn len(&self) -> usize;

    /// Visit a window without inspecting its preceding logical bytes.
    fn chunks(&self, range: Range<usize>) -> impl Iterator<Item = &str>;

    /// Borrow contiguous grammar operands and join only a split window.
    fn window(&self, range: Range<usize>) -> Cow<'_, str>;
}

impl Input for str {
    fn len(&self) -> usize {
        str::len(self)
    }

    fn chunks(&self, range: Range<usize>) -> impl Iterator<Item = &str> {
        std::iter::once(&self[range])
    }

    fn window(&self, range: Range<usize>) -> Cow<'_, str> {
        Cow::Borrowed(&self[range])
    }
}

impl Input for crate::source::Segmented {
    fn len(&self) -> usize {
        self.len()
    }

    fn chunks(&self, range: Range<usize>) -> impl Iterator<Item = &str> {
        self.chunks_in(range)
    }

    fn window(&self, range: Range<usize>) -> Cow<'_, str> {
        let mut chunks = self.chunks_in(range.clone());
        let first = chunks.next().unwrap_or("");
        if chunks.next().is_none() {
            return Cow::Borrowed(first);
        }
        let text = self.mapped().select(range).to_owned_string();
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.joined_bytes += text.len());
        Cow::Owned(text)
    }
}

/// Offsets into an append-only operand, with no borrowed or copied source.
#[derive(Clone, Default)]
pub(crate) struct Cursor {
    /// Bytes already searched for a newline.
    scanned: usize,
    /// Start of the current incomplete line.
    start: usize,
}

impl Cursor {
    /// Return one complete line, searching each newly supplied byte once.
    pub(crate) fn next(&mut self, source: &(impl Input + ?Sized)) -> Option<Range<usize>> {
        let Some(end) = line_end(source, self.scanned) else {
            self.scanned = source.len();
            return None;
        };
        self.scanned = end;
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

/// End of the physical line starting at `from`, terminator included.
///
/// `None` when the operand ends before a terminator. Every byte read is charged once, so
/// a caller framing line by line pays for the bytes it looks at.
pub(super) fn line_end(source: &(impl Input + ?Sized), from: usize) -> Option<usize> {
    let mut scanned = from;
    for chunk in source.chunks(from..source.len()) {
        if let Some(offset) = chunk.find('\n') {
            let end = scanned + offset + 1;
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.framing_bytes += end - from);
            return Some(end);
        }
        scanned += chunk.len();
    }
    #[cfg(test)]
    crate::tests::WORK.with_borrow_mut(|work| work.framing_bytes += scanned - from);
    None
}

#[cfg(test)]
mod tests;
