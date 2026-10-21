//! Shared monotone physical-line framing for retained block grammar.
use std::ops::Range;

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
    pub(crate) fn next(&mut self, source: &str) -> Option<Range<usize>> {
        let rest = source.get(self.scanned..)?;
        let end = rest.find('\n').map(|offset| self.scanned + offset + 1);
        let scanned = end.unwrap_or(source.len());
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.framing_bytes += scanned - self.scanned);
        self.scanned = scanned;
        let end = end?;
        let line = self.start..end;
        self.start = end;
        Some(line)
    }

    /// Return the final unterminated line at an explicit operand boundary.
    pub(crate) fn finish(&mut self, source: &str) -> Option<Range<usize>> {
        let line = (self.start < source.len()).then_some(self.start..source.len());
        self.start = source.len();
        line
    }
}
