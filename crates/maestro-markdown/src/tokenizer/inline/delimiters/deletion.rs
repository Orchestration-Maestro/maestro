//! Raw closing positions for the caller's deletion grammar.
use crate::MarkedOptions;
use std::ops::Range;

/// Source-owned closing positions for reached deletion openers.
pub(super) struct DeletionEnds {
    /// Nearest admitted closing position at each byte, by closing width.
    next: Vec<[usize; 2]>,
    /// First escape unit the caller grammar cannot admit at or after each byte.
    stop: Vec<usize>,
    /// Whether an admitted content unit ends at a byte boundary.
    tail: Vec<bool>,
    /// Bytes an escape unit consumed; they cannot begin a closing pair.
    escaped: Vec<bool>,
    /// Caller grammar, which fixes the escape unit and the admitted opener width.
    options: MarkedOptions,
}

impl DeletionEnds {
    /// Prepare bounded closing positions, or nothing where the grammar cannot delete.
    pub(super) fn prepare(text: &str, options: MarkedOptions) -> Option<Self> {
        if options == MarkedOptions::Documentation || !text.contains('~') {
            return None;
        }
        let absent = text.len() + 1;
        let mut ends = Self {
            next: vec![[absent; 2]; absent],
            stop: vec![absent; absent],
            tail: vec![false; absent],
            escaped: vec![false; absent],
            options,
        };
        ends.fill_units(text);
        ends.fill_closers(text);
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.deletion_bytes += text.len());
        Some(ends)
    }

    /// Whether the caller's escape unit excludes line terminators.
    fn strict(&self) -> bool {
        matches!(
            self.options,
            MarkedOptions::Terminal | MarkedOptions::Export
        )
    }

    /// Record content-unit boundaries and the escape the strict grammar refuses.
    fn fill_units(&mut self, text: &str) {
        let mut scalars = text.char_indices();
        while let Some((offset, scalar)) = scalars.next() {
            if scalar != '\\' {
                self.tail[offset + scalar.len_utf8()] =
                    scalar != '~' && !maestro_text::is_whitespace_scalar(scalar);
                continue;
            }
            let Some((escaped, inner)) = scalars.next() else {
                break;
            };
            self.escaped[escaped] = true;
            self.tail[escaped + inner.len_utf8()] = true;
            if self.strict() && matches!(inner, '\r' | '\n' | '\u{2028}' | '\u{2029}') {
                self.stop[offset] = offset;
            }
        }
    }

    /// Fill each byte's nearest admitted closing position in one reverse pass.
    fn fill_closers(&mut self, text: &str) {
        for offset in (0..text.len()).rev() {
            self.next[offset] = self.next[offset + 1];
            self.stop[offset] = self.stop[offset].min(self.stop[offset + 1]);
            self.fill_closer(text, offset);
        }
    }

    /// Record this byte as the nearest closing position of each width it closes.
    fn fill_closer(&mut self, text: &str, offset: usize) {
        for width in 1..=2 {
            if self.closes(text, offset, width) {
                self.next[offset][width - 1] = offset;
            }
        }
    }

    /// Whether this byte closes content at the given width without a continuing tilde.
    fn closes(&self, text: &str, offset: usize, width: usize) -> bool {
        self.admits(text, offset, width) && text.as_bytes().get(offset + width) != Some(&b'~')
    }

    /// Whether an admitted content unit ends before an unescaped tilde run of this width.
    fn admits(&self, text: &str, start: usize, width: usize) -> bool {
        self.tail.get(start) == Some(&true)
            && self.escaped.get(start) == Some(&false)
            && text
                .as_bytes()
                .get(start..start + width)
                .is_some_and(|bytes| bytes.iter().all(|&byte| byte == b'~'))
    }

    /// Select the first admitted closing pair inside one recursive window.
    pub(super) fn find(&self, text: &str, range: &Range<usize>) -> Option<(usize, Range<usize>)> {
        let rest = text.get(range.clone())?.strip_prefix('~')?;
        let width = if rest.starts_with('~') { 2 } else { 1 };
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.deletion_bytes += 1);
        if width == 1 && self.strict() {
            return None;
        }
        let body = range.start + width;
        let mut content = text.get(body..range.end)?.chars();
        let first = content.next()?;
        if first == '~' || maestro_text::is_whitespace_scalar(first) {
            return None;
        }
        let first_end = body
            + first.len_utf8()
            + if first == '\\' {
                content.next()?.len_utf8()
            } else {
                0
            };
        let window_end = range.end.checked_sub(width)?;
        let mut closer = self.next.get(first_end)?[width - 1];
        if window_end >= first_end && self.admits(text, window_end, width) {
            closer = closer.min(window_end);
        }
        if closer + width > range.end || self.stop.get(body)? < &closer {
            return None;
        }
        Some((closer + width - range.start, body..closer))
    }
}
