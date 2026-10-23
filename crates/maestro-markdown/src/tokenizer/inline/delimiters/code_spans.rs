//! Source-owned closing runs of the caller's code-span grammar.
use std::ops::Range;

/// One maximal backtick run.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Run {
    /// Number of backticks in the run.
    width: usize,
    /// Byte offset of the run's first backtick.
    start: usize,
}

/// Backtick run starts grouped by run width.
pub(super) struct CodeEnds {
    /// Runs ordered by width, then by start offset.
    runs: Vec<Run>,
}

impl CodeEnds {
    /// Record every maximal backtick run, or nothing without backticks.
    pub(super) fn prepare(text: &str) -> Option<Self> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.inline_code_bytes += text.len());
        let bytes = text.as_bytes();
        let mut runs = Vec::new();
        let mut offset = 0;
        while let Some(found) = bytes[offset..].iter().position(|byte| *byte == b'`') {
            let start = offset + found;
            let end = run_end(bytes, start);
            runs.push(Run {
                width: end - start,
                start,
            });
            offset = end;
        }
        runs.sort_unstable();
        (!runs.is_empty()).then_some(Self { runs })
    }

    /// Select the first closing run of the opener's own width inside the window.
    pub(super) fn find(&self, text: &str, range: &Range<usize>) -> Option<(usize, Range<usize>)> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.inline_code_bytes += 1);
        let width = opening_width(text, range)?;
        let body = range.start + width;
        let first = self
            .runs
            .partition_point(|run| *run < Run { width, start: body });
        let closer = self
            .runs
            .get(first)
            .filter(|run| run.width == width && run.start + width <= range.end)?
            .start;
        Some((closer + width - range.start, body..closer))
    }
}

/// The opener run's width, clipped to the query window.
fn opening_width(text: &str, range: &Range<usize>) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(range.start)? != &b'`' {
        return None;
    }
    let end = run_end(bytes, range.start).min(range.end);
    (end > range.start).then(|| end - range.start)
}

/// Exclusive endpoint of the homogeneous backtick run starting at `start`.
pub(super) fn run_end(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while bytes.get(end) == Some(&b'`') {
        end += 1;
    }
    end
}
