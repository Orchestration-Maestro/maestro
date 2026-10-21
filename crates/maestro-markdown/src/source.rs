//! Container-local source framing.
mod cursor;
mod segmented;
pub(crate) use cursor::Cursor;
pub(crate) use segmented::Segmented;
use std::{
    ops::{Deref, Range},
    sync::Arc,
};
/// Removes a native quote prefix, leaving lazy continuation text intact.
pub(crate) fn strip_quote(line: &str) -> &str {
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count().min(3);
    line[spaces..]
        .strip_prefix('>')
        .map_or(line, |line| line.strip_prefix([' ', '\t']).unwrap_or(line))
}

/// A contiguous window retaining its source allocation.
#[derive(Clone, Debug, Default)]
pub(crate) struct SourceText {
    /// Shared UTF-8 bytes, including bytes outside this selection.
    bytes: Arc<String>,
    /// Selected UTF-8 boundaries in the shared allocation.
    range: Range<usize>,
}
impl Deref for SourceText {
    type Target = str;
    fn deref(&self) -> &str {
        &self.bytes[self.range.clone()]
    }
}
impl SourceText {
    /// Retain selected bytes without a flattened cache or copy.
    fn select(&self, range: Range<usize>) -> Self {
        let _ = &self[range.clone()];
        Self {
            bytes: Arc::clone(&self.bytes),
            range: self.range.start + range.start..self.range.start + range.end,
        }
    }
    /// Publish shared authored bytes with no dependence on parser lifetime.
    pub(crate) fn mapped(&self) -> crate::MappedText {
        crate::MappedText::shared(Arc::clone(&self.bytes), self.range.clone())
    }
    /// Own only this selected operand before changing its bytes.
    fn normalize(&mut self) {
        if self.range != (0..self.bytes.len()) {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.source_bytes += self.len());
            self.bytes = Arc::new(self.to_string());
            self.range = 0..self.bytes.len();
        }
    }
    /// Append to the selected allocation, copying only if another value retains it.
    fn push_str(&mut self, text: &str) {
        self.normalize();
        #[cfg(test)]
        if Arc::strong_count(&self.bytes) > 1 {
            crate::tests::WORK.with_borrow_mut(|work| work.source_bytes += self.len());
        }
        Arc::make_mut(&mut self.bytes).push_str(text);
        self.range.end = self.bytes.len();
    }
}

/// Container-local authored source with ordered native provenance.
#[derive(Clone)]
pub(crate) struct ScopeSource {
    /// Bytes before inline interpretation.
    pub(crate) text: SourceText,
    /// Ordered identity spans or atomic spelling transformations.
    units: Arc<Vec<(Range<usize>, Range<usize>)>>,
}
impl ScopeSource {
    /// Count backing source and provenance allocations once across all retained views.
    #[cfg(test)]
    pub(crate) fn retained(&self, retained: &mut crate::tests::Retained) {
        retained.text(&self.text.bytes);
        if retained.arc(
            &self.units,
            self.units.capacity() * std::mem::size_of::<(Range<usize>, Range<usize>)>(),
        ) {
            retained.mapping_records += self.units.len();
        }
    }

    /// Project a native operand through captured framing, not string-offset subtraction.
    pub(crate) fn local(&self, range: &Range<usize>) -> Option<Range<usize>> {
        self.project(range, false)
    }
    /// Project an authored selection back through the same captured provenance.
    pub(crate) fn native(&self, range: &Range<usize>) -> Option<Range<usize>> {
        self.project(range, true)
    }
    /// Provenance units overlapping the current source window.
    fn unit_slice(&self) -> &[(Range<usize>, Range<usize>)] {
        let first = self
            .units
            .partition_point(|(_, local)| local.end <= self.text.range.start);
        let last = self
            .units
            .partition_point(|(_, local)| local.start < self.text.range.end);
        &self.units[first..last.max(first)]
    }
    /// Clip an identity unit while keeping an atomic transformation indivisible.
    fn unit(&self, (native, local): &(Range<usize>, Range<usize>)) -> (Range<usize>, Range<usize>) {
        let start = local.start.max(self.text.range.start);
        let end = local.end.min(self.text.range.end);
        let native = if native.len() == local.len() {
            native.start + start - local.start..native.start + end - local.start
        } else {
            native.clone()
        };
        (
            native,
            start - self.text.range.start..end - self.text.range.start,
        )
    }
    /// Make selected provenance independent before extending or rewriting it.
    fn normalize(&mut self) {
        if self.text.range != (0..self.text.bytes.len()) {
            self.units = Arc::new(
                self.unit_slice()
                    .iter()
                    .map(|unit| self.unit(unit))
                    .collect(),
            );
            self.text.normalize();
        }
    }
    /// Share boundary projection in both directions, retaining atomic transformations.
    fn project(&self, range: &Range<usize>, reverse: bool) -> Option<Range<usize>> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.projections += 1);
        let units = self.unit_slice();
        let pair = |index| {
            let (native, local) = self.unit(units.get(index)?);
            Some(if reverse {
                (local, native)
            } else {
                (native, local)
            })
        };
        let first = units.partition_point(|unit| {
            let (native, local) = self.unit(unit);
            if reverse {
                local.end <= range.start
            } else {
                native.end < range.start || native.end == range.start && !native.is_empty()
            }
        });
        let last = units
            .partition_point(|unit| {
                let (native, local) = self.unit(unit);
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
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.source_bytes += text.len());
        Self {
            text: SourceText {
                bytes: Arc::new(text.to_owned()),
                range: 0..text.len(),
            },
            units: Arc::new(vec![(0..text.len(), 0..text.len())]),
        }
    }
    /// Retain selected authored pieces and their original coordinates.
    pub(crate) fn select(&self, pieces: impl IntoIterator<Item = Range<usize>>) -> Self {
        let mut pieces = pieces
            .into_iter()
            .filter(|piece| !piece.is_empty())
            .peekable();
        let Some(first) = pieces.next() else {
            return Self {
                text: SourceText::default(),
                units: Arc::default(),
            };
        };
        if pieces.peek().is_none() {
            return Self {
                text: self.text.select(first),
                units: Arc::clone(&self.units),
            };
        }
        let mut output = Cursor::default();
        for range in std::iter::once(first).chain(pieces) {
            output.append(Self {
                text: self.text.select(range),
                units: Arc::clone(&self.units),
            });
        }
        output.materialize()
    }

    /// Source extent of a local selection, including an empty boundary.
    pub(crate) fn extent(&self, range: Range<usize>) -> Range<usize> {
        self.native(&range).unwrap_or_else(|| {
            let offset = self
                .unit_slice()
                .last()
                .map_or(0, |unit| self.unit(unit).0.end);
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
        let mut output = Cursor::default();
        let mut start = 0;
        for (offset, _) in self.text.match_indices(from) {
            output.append(self.select(std::iter::once(start..offset)));
            let mut spelling = self.select(std::iter::empty());
            spelling.spelling(self.extent(offset..offset + from.len()), to);
            output.append(spelling);
            start = offset + from.len();
        }
        output.append(self.select(std::iter::once(start..self.text.len())));
        output.materialize()
    }

    /// Replace a known leading prefix with one mapped spelling transformation.
    pub(crate) fn replace_prefix(&self, length: usize, text: &str) -> Self {
        let mut output = self.select(std::iter::empty());
        let native = if length == 0 {
            let start = self
                .unit_slice()
                .first()
                .map_or(0, |unit| self.unit(unit).0.start);
            start..start
        } else {
            self.extent(0..length)
        };
        output.spelling(native, text);
        let mut pieces = Cursor::default();
        pieces.append(output);
        pieces.append(self.select(std::iter::once(length..self.text.len())));
        pieces.materialize()
    }

    /// Append an atomic spelling while retaining the authored extent it represents.
    fn spelling(&mut self, native: Range<usize>, text: &str) {
        self.normalize();
        let begin = self.text.len();
        self.text.push_str(text);
        Arc::make_mut(&mut self.units).push((native, begin..self.text.len()));
    }

    /// Append a framed span without losing its transformations.
    pub(crate) fn append(&mut self, other: Self) {
        if self.text.is_empty() && self.units.is_empty() {
            *self = other;
            return;
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.joined_bytes += other.text.len());
        self.normalize();
        let base = self.text.len();
        self.text.push_str(&other.text);
        Arc::make_mut(&mut self.units).extend(other.unit_slice().iter().map(|unit| {
            let (native, local) = other.unit(unit);
            (native, base + local.start..base + local.end)
        }));
    }

    /// Discard a semantic suffix while retaining the existing prefix allocation.
    pub(crate) fn truncate(&mut self, end: usize) {
        let _ = &self.text[..end];
        self.text.range.end = self.text.range.start + end;
    }

    /// Add a semantic separator without consuming another authored byte.
    pub(crate) fn newline(&mut self) {
        self.normalize();
        let boundary = self
            .unit_slice()
            .last()
            .map_or(0, |unit| self.unit(unit).0.end);
        let start = self.text.len();
        self.text.push_str("\n");
        Arc::make_mut(&mut self.units).push((boundary..boundary, start..start + 1));
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
    /// Retain authored bytes inside this bounded parser window.
    pub(crate) fn authored(&self, range: Range<usize>) -> crate::MappedText {
        let _ = &self.text[range.clone()];
        self.owner
            .text
            .select(self.offset + range.start..self.offset + range.end)
            .mapped()
    }
    /// Build an owned semantic node from the current window.
    pub(crate) fn node(&self, range: Range<usize>, kind: crate::Kind) -> crate::Node {
        crate::Node {
            authored: self.authored(range.clone()),
            range: self.extent(range),
            kind,
        }
    }
}
