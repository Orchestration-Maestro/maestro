//! Source-local delimiter observations shared by recursive inline windows.
pub(super) mod deletion;
use crate::MarkedOptions;
use std::ops::Range;

/// Homogeneous delimiter observations indexed by original source bytes.
pub(crate) struct Delimiters {
    /// Exclusive run endpoint, or zero at non-delimiter bytes.
    ends: Vec<usize>,
    /// Caller-selected raw deletion closers, absent where the grammar cannot delete.
    tildes: Option<deletion::DeletionEnds>,
}

impl Delimiters {
    /// Retain each homogeneous delimiter run beside the deletion closers of one source.
    pub(crate) fn new(source: &str, options: MarkedOptions) -> Self {
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
        Self {
            ends,
            tildes: deletion::DeletionEnds::prepare(source, options),
        }
    }

    /// Select the remaining width without crossing the recursive window's end.
    pub(super) fn width(&self, range: &Range<usize>) -> Option<usize> {
        let end = self.ends.get(range.start)?.min(&range.end);
        end.checked_sub(range.start).filter(|width| *width > 0)
    }

    /// Select the first raw closing pair inside the recursive window.
    pub(crate) fn deletion(
        &self,
        text: &str,
        range: &Range<usize>,
    ) -> Option<(usize, Range<usize>)> {
        self.tildes.as_ref()?.find(text, range)
    }
}
