//! Source-owned closing positions of the caller's inline label scan.

/// Offset with no reachable closing pair.
const ABSENT: usize = usize::MAX;

/// One step of the label scan over an original source byte.
enum Step {
    /// The scan stops on this byte: it begins a closing `](`.
    Exit,
    /// The scan consumes up to this byte and continues there.
    Continue(usize),
    /// No label element starts with this byte.
    Blocked,
}

/// Nearest closing `](` each label offset can reach.
pub(super) struct LabelClosers {
    /// Byte offset rules can reach a closing pair, or [`ABSENT`].
    reach: Vec<usize>,
}

impl LabelClosers {
    /// Prepare reachable closing pairs in one reverse pass over the source.
    pub(super) fn prepare(text: &str) -> Self {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.link_bytes += text.len());
        let bytes = text.as_bytes();
        let ticks = closing_ticks(bytes);
        let mut reach = vec![ABSENT; bytes.len() + 1];
        for offset in (0..bytes.len()).rev() {
            reach[offset] = match step(bytes, offset, &ticks) {
                Step::Exit => offset,
                Step::Continue(end) => reach.get(end).copied().unwrap_or(ABSENT),
                Step::Blocked => ABSENT,
            };
        }
        Self { reach }
    }

    /// Select the closing pair a label starting at `start` reaches inside the window.
    pub(super) fn reachable(&self, start: usize, end: usize) -> Option<usize> {
        let closer = *self.reach.get(start)?;
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.link_bytes += 1);
        (closer != ABSENT && closer + 1 < end).then_some(closer)
    }
}

/// Byte offset of each backtick run the label scan can consume, or [`ABSENT`].
fn closing_ticks(bytes: &[u8]) -> Vec<usize> {
    let mut ticks = vec![ABSENT; bytes.len() + 1];
    for offset in (0..bytes.len()).rev() {
        ticks[offset] = if bytes[offset] == b'`' {
            offset
        } else {
            ticks[offset + 1]
        };
    }
    ticks
}

/// Classify the label element that starts at `offset`.
fn step(bytes: &[u8], offset: usize, ticks: &[usize]) -> Step {
    if bytes[offset] == b']' && bytes.get(offset + 1) == Some(&b'(') {
        return Step::Exit;
    }
    match bytes[offset] {
        b'\\' => escaped(bytes, offset),
        b'`' => code_span(bytes, offset, ticks),
        b'[' => pair_end(bytes, offset).map_or(Step::Blocked, |end| Step::Continue(end + 1)),
        b']' => Step::Blocked,
        _ => Step::Continue(offset + 1),
    }
}

/// An escape unit consumes the byte after its backslash.
fn escaped(bytes: &[u8], offset: usize) -> Step {
    match offset + 1 {
        next if next < bytes.len() => Step::Continue(next + 1),
        _ => Step::Blocked,
    }
}

/// A code-span element closes on the next backtick run.
fn code_span(bytes: &[u8], offset: usize, ticks: &[usize]) -> Step {
    let mut opener = offset;
    while bytes.get(opener) == Some(&b'`') {
        opener += 1;
    }
    match ticks.get(opener).copied() {
        Some(tick) if tick != ABSENT => {
            let mut end = tick;
            while bytes.get(end) == Some(&b'`') {
                end += 1;
            }
            Step::Continue(end)
        }
        _ => Step::Blocked,
    }
}

/// A nested bracket element closes on the first bracket-free, escape-free end.
fn pair_end(bytes: &[u8], offset: usize) -> Option<usize> {
    let mut cursor = offset + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b']' => return Some(cursor),
            b'\\' => cursor += 2,
            b'[' => return None,
            _ => cursor += 1,
        }
    }
    None
}
