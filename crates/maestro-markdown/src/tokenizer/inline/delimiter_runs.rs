//! Source-local delimiter endpoints shared by recursive inline windows.
use std::ops::Range;

/// Homogeneous delimiter endpoints indexed by original source bytes.
pub(super) struct DelimiterRuns {
    /// Exclusive run endpoint, or zero at non-delimiter bytes.
    ends: Vec<usize>,
}

impl DelimiterRuns {
    /// Retain each homogeneous delimiter run in one source traversal.
    pub(super) fn new(source: &str) -> Self {
        let mut ends = vec![0; source.len()];
        let mut runs = source.bytes().enumerate().peekable();
        while let Some((start, marker)) = runs.next() {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.emphasis_left += 1);
            if !matches!(marker, b'*' | b'_') {
                continue;
            }
            let mut end = start + 1;
            while let Some((index, _)) = runs.next_if(|(_, next)| *next == marker) {
                #[cfg(test)]
                crate::tests::WORK.with_borrow_mut(|work| work.emphasis_left += 1);
                end = index + 1;
            }
            ends[start..end].fill(end);
        }
        Self { ends }
    }

    /// Select the remaining width without crossing the recursive window's end.
    pub(super) fn width(&self, range: &Range<usize>) -> Option<usize> {
        let end = self.ends.get(range.start)?.min(&range.end);
        end.checked_sub(range.start).filter(|width| *width > 0)
    }
}
