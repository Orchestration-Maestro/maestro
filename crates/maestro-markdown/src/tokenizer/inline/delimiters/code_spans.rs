//! Source-owned closing runs of the caller's code-span grammar.
use std::ops::Range;

/// Backtick run starts grouped by run width.
pub(super) struct CodeEnds {
    /// Runs ordered by width, then by start offset.
    runs: Vec<(usize, usize)>,
}

impl CodeEnds {
    /// Record every maximal backtick run, or nothing without backticks.
    pub(super) fn prepare(text: &str) -> Option<Self> {
        if !text.contains('`') {
            return None;
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.inline_code_bytes += text.len());
        let bytes = text.as_bytes();
        let mut runs = Vec::new();
        let mut offset = 0;
        while offset < bytes.len() {
            if bytes[offset] != b'`' {
                offset += 1;
                continue;
            }
            let end = run_end(bytes, offset);
            runs.push((end - offset, offset));
            offset = end;
        }
        runs.sort_unstable();
        Some(Self { runs })
    }

    /// Select the first closing run of the opener's own width inside the window.
    pub(super) fn find(&self, text: &str, range: &Range<usize>) -> Option<(usize, Range<usize>)> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.inline_code_bytes += 1);
        let width = opening_width(text, range)?;
        let body = range.start + width;
        let closer = self.closer(text.as_bytes(), body, range.end, width)?;
        Some((closer + width - range.start, body..closer))
    }

    /// Select the nearest closing run at or after `body` that fits the window.
    fn closer(&self, bytes: &[u8], body: usize, end: usize, width: usize) -> Option<usize> {
        let first = self.runs.partition_point(|run| *run < (width, body));
        let run = self.runs.get(first).filter(|run| run.0 == width)?;
        if run.1 + width <= end {
            return Some(run.1);
        }
        cut_run(bytes, body, end, width)
    }
}

/// The run cut by the window end, when its visible width matches the opener's.
fn cut_run(bytes: &[u8], body: usize, end: usize, width: usize) -> Option<usize> {
    if bytes.get(end.checked_sub(1)?) != Some(&b'`') {
        return None;
    }
    let mut start = end - 1;
    while start > body && bytes[start - 1] == b'`' {
        start -= 1;
    }
    (start > body && end - start == width).then_some(start)
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
