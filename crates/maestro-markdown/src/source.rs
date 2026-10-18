//! Container-local source framing.
use std::ops::Range;
/// Removes a native quote prefix, leaving lazy continuation text intact.
pub(crate) fn strip_quote(line: &str) -> &str {
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count().min(3);
    line[spaces..]
        .strip_prefix('>')
        .map_or(line, |line| line.strip_prefix(' ').unwrap_or(line))
}

/// Container-local authored source with ordered native provenance.
#[derive(Clone)]
pub(crate) struct ScopeSource {
    /// Bytes before inline interpretation.
    pub(crate) text: String,
    /// Source-ordered starts of lazy container continuations that cannot open tables.
    pub(crate) lazy_lines: Vec<usize>,
    /// Ordered identity spans or atomic table-pipe transformations.
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
    /// Bytes remaining in the current identity span or atomic transformed unit.
    pub(crate) fn remaining(&self, offset: usize) -> usize {
        let index = self.units.partition_point(|(_, local)| local.end <= offset);
        self.units
            .get(index)
            .map_or(0, |(_, local)| local.end - offset)
    }
    /// Share boundary projection in both directions, retaining atomic transformations.
    fn project(&self, range: &Range<usize>, reverse: bool) -> Option<Range<usize>> {
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
                native.end <= range.start
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
    /// Record equal spans and the table parser's one atomic spelling transform.
    fn push(&mut self, mut text: &str, mut native: usize, cell: bool) {
        while !text.is_empty() {
            let end = if cell {
                text.find("\\|").unwrap_or(text.len())
            } else {
                text.len()
            };
            let size = if end == 0 { 2 } else { end };
            let shown = if end == 0 { "|" } else { &text[..end] };
            let start = self.text.len();
            self.text.push_str(shown);
            self.units
                .push((native..native + size, start..self.text.len()));
            native += size;
            text = &text[size..];
        }
    }
}

impl ScopeSource {
    /// Capture an unchanged source buffer with absolute provenance.
    pub(crate) fn identity(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            units: vec![(0..text.len(), 0..text.len())],
            lazy_lines: Vec::new(),
        }
    }
    /// Retain selected authored pieces and their original coordinates.
    pub(crate) fn select(&self, pieces: impl IntoIterator<Item = Range<usize>>) -> Self {
        let mut output = Self {
            text: String::new(),
            units: Vec::new(),
            lazy_lines: Vec::new(),
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
        let base = output.text.len();
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
        let first = self
            .lazy_lines
            .partition_point(|offset| *offset < piece.start);
        let last = self
            .lazy_lines
            .partition_point(|offset| *offset < piece.end);
        output.lazy_lines.extend(
            self.lazy_lines[first..last]
                .iter()
                .map(|offset| base + offset - piece.start),
        );
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

    /// Apply table-pipe spelling once while preserving each atomic source unit.
    pub(crate) fn pipes(&self) -> Self {
        let mut output = Self {
            text: String::new(),
            units: Vec::new(),
            lazy_lines: Vec::new(),
        };
        for (native, local) in &self.units {
            output.push(&self.text[local.clone()], native.start, true);
        }
        output
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
    /// Retain only selected construct bytes, never the unconsumed document suffix.
    pub(crate) fn select(&self, pieces: impl IntoIterator<Item = Range<usize>>) -> ScopeSource {
        self.owner.select(
            pieces
                .into_iter()
                .map(|range| self.offset + range.start..self.offset + range.end),
        )
    }
    /// A lazy continuation cannot introduce a table at this local boundary.
    pub(crate) fn is_lazy(&self, offset: usize) -> bool {
        self.owner
            .lazy_lines
            .binary_search(&(self.offset + offset))
            .is_ok()
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
