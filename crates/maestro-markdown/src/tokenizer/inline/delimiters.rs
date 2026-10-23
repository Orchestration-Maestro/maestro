//! Source-local delimiter observations shared by recursive inline windows.
pub(super) mod code_spans;
pub(super) mod deletion;
pub(super) mod label_closers;
use crate::MarkedOptions;
use std::ops::Range;

/// Homogeneous delimiter observations indexed by original source bytes.
pub(crate) struct Delimiters {
    /// Exclusive run endpoint, or zero at non-delimiter bytes.
    ends: Vec<usize>,
    /// Caller-selected raw deletion closers, absent where the grammar cannot delete.
    tildes: Option<deletion::DeletionEnds>,
    /// Closing runs of the caller's code-span grammar, absent without backticks.
    backticks: Option<code_spans::CodeEnds>,
    /// Direct-link and reference bracket observations, absent without `]`.
    labels: Option<label_closers::LabelClosers>,
}

impl Delimiters {
    /// Retain each homogeneous delimiter run beside the closers of one source.
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
            backticks: code_spans::CodeEnds::prepare(source),
            labels: label_closers::LabelClosers::prepare(source),
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

    /// Select the first closing backtick run of the opener's width in the window.
    pub(crate) fn code_span(
        &self,
        text: &str,
        range: &Range<usize>,
    ) -> Option<(usize, Range<usize>)> {
        self.backticks.as_ref()?.find(text, range)
    }

    /// Admit reference matching only where a closing bracket remains in the window.
    pub(crate) fn reference_possible(&self, text: &str, range: &Range<usize>) -> bool {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.reference_bytes += 1);
        let bang = usize::from(text.starts_with('!'));
        text.as_bytes().get(bang) == Some(&b'[')
            && self
                .labels
                .as_ref()
                .is_some_and(|labels| labels.reference_possible(range.start + bang + 1, range.end))
    }

    /// Select the closing pair an authored link label reaches inside the window.
    pub(crate) fn link_closer(&self, text: &str, range: &Range<usize>) -> Option<usize> {
        let bang = usize::from(text.starts_with('!'));
        if text.as_bytes().get(bang)? != &b'[' {
            return None;
        }
        self.labels
            .as_ref()?
            .reachable(range.start + bang + 1, range.end)
    }
}
