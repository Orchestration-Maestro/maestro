//! Source-owned closing positions of each caller grammar's inline label scan.

/// Offset with no reachable closing pair.
const ABSENT: usize = usize::MAX;

/// Nearest closing `](` each label offset can reach.
pub(super) struct LabelClosers {
    /// Nearest closing-pair byte offset each label offset can reach, or [`ABSENT`].
    reach: Vec<usize>,
}

impl LabelClosers {
    /// Prepare reachable closing pairs, or nothing where no label can close.
    ///
    /// One forward pass pairs brackets; one reverse pass then answers every offset.
    pub(super) fn prepare(text: &str) -> Option<Self> {
        if !text.contains("](") {
            return None;
        }
        let bytes = text.as_bytes();
        let pairs = pair_ends(bytes);
        let mut reach = vec![ABSENT; bytes.len() + 1];
        let mut ticks = Ticks::NONE;
        for offset in (0..bytes.len()).rev() {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.link_bytes += 1);
            reach[offset] = match bytes[offset] {
                b'`' => ticks.closer(&reach, offset),
                b']' if bytes.get(offset + 1) == Some(&b'(') => offset,
                b'[' if pairs[offset] != ABSENT => after(&reach, pairs[offset] + 1),
                b']' | b'[' => ABSENT,
                b'\\' => after(&reach, offset + 2),
                _ => reach[offset + 1],
            };
            if bytes[offset] == b'`' {
                ticks.record(offset);
            }
        }
        Some(Self { reach })
    }

    /// Select the closing pair a label starting at `start` reaches inside the window.
    pub(super) fn reachable(&self, start: usize, end: usize) -> Option<usize> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.link_bytes += 1);
        let closer = *self.reach.get(start)?;
        (closer != ABSENT && closer + 1 < end).then_some(closer)
    }
}

/// The `]` each `[` pairs with before a nested bracket, or [`ABSENT`].
fn pair_ends(bytes: &[u8]) -> Vec<usize> {
    let mut pairs = vec![ABSENT; bytes.len()];
    let mut open = ABSENT;
    let mut offset = 0;
    while let Some(byte) = bytes.get(offset) {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.link_bytes += 1);
        match byte {
            b'\\' => {
                offset += 2;
                continue;
            }
            b'[' => open = offset,
            b']' if open != ABSENT => {
                pairs[open] = offset;
                open = ABSENT;
            }
            _ => {}
        }
        offset += 1;
    }
    pairs
}

/// The closer reachable from a byte the scan passes over, or [`ABSENT`].
fn after(reach: &[usize], offset: usize) -> usize {
    reach.get(offset).copied().unwrap_or(ABSENT)
}

/// The backtick runs above the reviewed byte, nearest run first.
struct Ticks {
    /// Start of the nearest run, or [`ABSENT`] when no run starts above.
    start: usize,
    /// Exclusive end of the nearest run, or [`ABSENT`].
    end: usize,
    /// Exclusive end of the run after the nearest one, or [`ABSENT`].
    next: usize,
}

impl Ticks {
    /// The state a preparation starts with, above the last byte.
    const NONE: Self = Self {
        start: ABSENT,
        end: ABSENT,
        next: ABSENT,
    };

    /// Continue the scan for the tick at `offset` wherever a caller grammar closes.
    ///
    /// A run of two or more ticks closes itself, so the scan resumes at the run's
    /// end. The terminal and export grammars pair single ticks instead, and resume
    /// one tick past the opener or past the first tick of the closing run. Admitting
    /// both spellings keeps every label the selected grammar accepts reachable.
    fn closer(&self, reach: &[usize], offset: usize) -> usize {
        if self.start == offset + 1 {
            after(reach, offset + 2)
                .min(after(reach, self.end))
                .min(after(reach, self.next))
        } else if self.start == ABSENT {
            ABSENT
        } else {
            after(reach, self.start + 1).min(after(reach, self.end))
        }
    }

    /// Record the run that starts at `offset`, dropping the runs it precedes.
    fn record(&mut self, offset: usize) {
        if self.start != offset + 1 {
            self.next = self.end;
            self.end = offset + 1;
        }
        self.start = offset;
    }
}
