//! Container-local source framing.
use std::ops::Range;
/// Removes a native quote prefix, leaving lazy continuation text intact.
pub(crate) fn strip_quote(line: &str) -> &str {
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count().min(3);
    line[spaces..]
        .strip_prefix('>')
        .map_or(line, |line| line.strip_prefix([' ', '\t']).unwrap_or(line))
}

/// Container-local authored source with ordered native provenance.
#[derive(Clone)]
pub(crate) struct ScopeSource {
    /// Bytes before inline interpretation.
    pub(crate) text: String,
    /// Ordered identity spans or atomic spelling transformations.
    units: Vec<(Range<usize>, Range<usize>)>,
}
impl ScopeSource {
    /// Project a native operand through captured framing, not string-offset subtraction.
    pub(crate) fn local(&self, range: &Range<usize>) -> Option<Range<usize>> {
        self.project(range, false)
    }
    /// Project an authored selection back through the same captured provenance.
    pub(crate) fn native(&self, range: &Range<usize>) -> Option<Range<usize>> {
        self.project(range, true)
    }
    /// Share boundary projection in both directions, retaining atomic transformations.
    fn project(&self, range: &Range<usize>, reverse: bool) -> Option<Range<usize>> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.projections += 1);
        let pair = |index| {
            let (native, local) = self.units.get(index)?;
            Some(if reverse {
                (local, native)
            } else {
                (native, local)
            })
        };
        let first = self.units.partition_point(|(native, local)| {
            if reverse {
                local.end <= range.start
            } else {
                native.end < range.start || native.end == range.start && !native.is_empty()
            }
        });
        let last = self
            .units
            .partition_point(|(native, local)| {
                if reverse {
                    local.start < range.end
                } else {
                    native.start < range.end
                }
            })
            .checked_sub(1)?;
        let (from, to) = pair(first)?;
        let start = to.start
            + if from.len() == to.len() {
                range.start.saturating_sub(from.start)
            } else {
                0
            };
        let (from, to) = pair(last)?;
        let end = to.end
            - if from.len() == to.len() {
                from.end.saturating_sub(range.end)
            } else {
                0
            };
        Some(start..end)
    }
}
impl ScopeSource {
    /// Capture an unchanged source buffer with absolute provenance.
    pub(crate) fn identity(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            units: vec![(0..text.len(), 0..text.len())],
        }
    }
    /// Retain selected authored pieces and their original coordinates.
    pub(crate) fn select(&self, pieces: impl IntoIterator<Item = Range<usize>>) -> Self {
        let mut output = Self {
            text: String::new(),
            units: Vec::new(),
        };
        for piece in pieces {
            self.select_piece(piece, &mut output);
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.source_bytes += output.text.len());
        output
    }
    /// Append one authored piece, preserving atomic spelling transformations.
    fn select_piece(&self, piece: Range<usize>, output: &mut Self) {
        if piece.is_empty() {
            return;
        }
        let first = self
            .units
            .partition_point(|(_, local)| local.end <= piece.start);
        let last = self
            .units
            .partition_point(|(_, local)| local.start < piece.end);
        for (native, local) in &self.units[first..last] {
            let start = local.start.max(piece.start);
            let end = local.end.min(piece.end);
            let absolute = if native.len() == local.len() {
                native.start + start - local.start..native.start + end - local.start
            } else {
                native.clone()
            };
            let begin = output.text.len();
            output.text.push_str(&self.text[start..end]);
            output.units.push((absolute, begin..output.text.len()));
        }
    }

    /// Source extent of a local selection, including an empty boundary.
    pub(crate) fn extent(&self, range: Range<usize>) -> Range<usize> {
        self.native(&range).unwrap_or_else(|| {
            let offset = self.units.last().map_or(0, |(native, _)| native.end);
            offset..offset
        })
    }
}

impl ScopeSource {
    /// Unescape label brackets before inline parsing, retaining original byte coordinates.
    pub(crate) fn brackets(&self) -> Self {
        let mut pieces = Vec::new();
        let mut start = 0;
        for (offset, pair) in self.text.as_bytes().windows(2).enumerate() {
            if pair[0] == b'\\' && matches!(pair[1], b'[' | b']') {
                pieces.push(start..offset);
                start = offset + 1;
            }
        }
        pieces.push(start..self.text.len());
        self.select(pieces)
    }

    /// Replace native spelling units without rebuilding earlier transformation spans.
    pub(crate) fn replace(&self, from: &str, to: &str) -> Self {
        let mut output = self.select(std::iter::empty());
        let mut start = 0;
        for (offset, _) in self.text.match_indices(from) {
            self.select_piece(start..offset, &mut output);
            output.spelling(self.extent(offset..offset + from.len()), to);
            start = offset + from.len();
        }
        self.select_piece(start..self.text.len(), &mut output);
        output
    }

    /// Replace a known leading prefix with one mapped spelling transformation.
    pub(crate) fn replace_prefix(&self, length: usize, text: &str) -> Self {
        let mut output = self.select(std::iter::empty());
        let native = if length == 0 {
            let start = self.units.first().map_or(0, |(native, _)| native.start);
            start..start
        } else {
            self.extent(0..length)
        };
        output.spelling(native, text);
        self.select_piece(length..self.text.len(), &mut output);
        output
    }

    /// Append an atomic spelling while retaining the authored extent it represents.
    fn spelling(&mut self, native: Range<usize>, text: &str) {
        let begin = self.text.len();
        self.text.push_str(text);
        self.units.push((native, begin..self.text.len()));
    }

    /// Append a framed span without losing its transformations.
    pub(crate) fn append(&mut self, other: Self) {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.joined_bytes += other.text.len());
        let base = self.text.len();
        self.text.push_str(&other.text);
        self.units.extend(
            other
                .units
                .into_iter()
                .map(|(native, local)| (native, base + local.start..base + local.end)),
        );
    }

    /// Discard a semantic suffix while retaining the existing prefix allocation.
    pub(crate) fn truncate(&mut self, end: usize) {
        self.text.truncate(end);
        let count = self.units.partition_point(|(_, local)| local.start < end);
        self.units.truncate(count);
        if let Some((native, local)) = self.units.last_mut()
            && local.end > end
        {
            if native.len() == local.len() {
                native.end -= local.end - end;
            }
            local.end = end;
        }
    }

    /// Add a semantic separator without consuming another authored byte.
    pub(crate) fn newline(&mut self) {
        let boundary = self.units.last().map_or(0, |(native, _)| native.end);
        let start = self.text.len();
        self.text.push('\n');
        self.units.push((boundary..boundary, start..start + 1));
    }

    /// Apply table-pipe spelling once while preserving each atomic source unit.
    pub(crate) fn pipes(&self) -> Self {
        self.replace("\\|", "|")
    }
}

/// Borrowed parser window with provenance in its retained source owner.
#[derive(Clone, Copy)]
pub(crate) struct Source<'a> {
    /// Current authored bytes, without copying an unconsumed suffix.
    pub(crate) text: &'a str,
    /// Full mapped source for coordinate projection.
    owner: &'a ScopeSource,
    /// Start of this window in the mapped source.
    offset: usize,
}
impl ScopeSource {
    /// Borrow a parser window without allocating text or mapping units.
    pub(crate) fn view(&self, range: Range<usize>) -> Source<'_> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.source_views += 1);
        Source {
            text: &self.text[range.clone()],
            owner: self,
            offset: range.start,
        }
    }
}
impl Source<'_> {
    /// Borrow a bounded grammar prefix without copying source or projection units.
    pub(crate) fn prefix(&self, end: usize) -> Source<'_> {
        Source {
            text: &self.text[..end],
            owner: self.owner,
            offset: self.offset,
        }
    }
    /// The whole mapped source text and this window's start in it.
    pub(crate) fn whole(&self) -> (&str, usize) {
        (&self.owner.text, self.offset)
    }
    /// Borrow the unconsumed suffix without copying its text or mapping.
    pub(crate) fn suffix(&self, start: usize) -> Source<'_> {
        Source {
            text: &self.text[start..],
            owner: self.owner,
            offset: self.offset + start,
        }
    }
    /// Retain only selected construct bytes, never the unconsumed document suffix.
    pub(crate) fn select(&self, pieces: impl IntoIterator<Item = Range<usize>>) -> ScopeSource {
        self.owner.select(
            pieces
                .into_iter()
                .map(|range| self.offset + range.start..self.offset + range.end),
        )
    }
    /// Project an original extent into this borrowed window's local coordinates.
    pub(crate) fn local(&self, range: &Range<usize>) -> Option<Range<usize>> {
        let local = self.owner.local(range)?;
        Some(local.start.checked_sub(self.offset)?..local.end.checked_sub(self.offset)?)
    }
    /// Project a construct from this borrowed window to original document coordinates.
    pub(crate) fn extent(&self, range: Range<usize>) -> Range<usize> {
        self.owner
            .extent(self.offset + range.start..self.offset + range.end)
    }
    /// Build an owned semantic node from the current window.
    pub(crate) fn node(&self, range: Range<usize>, kind: crate::Kind) -> crate::Node {
        crate::Node {
            authored: self.text[range.clone()].to_owned(),
            range: self.extent(range),
            kind,
        }
    }
}
