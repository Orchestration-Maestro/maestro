//! Owned source segments awaiting a bounded contiguous grammar operand.
use super::{ScopeSource, Segmented};
use std::collections::VecDeque;

/// A single-owner input queue; moving a remainder retains its source allocations.
#[derive(Default)]
pub(crate) struct Cursor {
    /// Selected source spans in consumption order, without joined backing strings.
    pieces: VecDeque<Segmented>,
    /// Remaining logical UTF-8 bytes.
    length: usize,
}

impl Cursor {
    /// Remaining logical UTF-8 length without visiting source segments.
    pub(crate) fn len(&self) -> usize {
        self.length
    }

    /// Whether every queued byte has been consumed.
    pub(crate) fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Retain another framed selection without copying its bytes or provenance.
    pub(crate) fn append(&mut self, source: ScopeSource) {
        if !source.text.is_empty() || !source.units.is_empty() {
            self.length += source.text.len();
            self.pieces.push_back(source.into());
        }
    }

    /// Transfer queued segments in their existing logical order.
    pub(crate) fn extend(&mut self, mut other: Self) {
        self.length += other.length;
        self.pieces.append(&mut other.pieces);
    }

    /// Return one lookahead operand without copying its bytes.
    pub(crate) fn prepend(&mut self, source: ScopeSource) {
        self.length += source.text.len();
        self.pieces.push_front(source.into());
    }

    /// Queue a shared logical splice without materializing it.
    pub(crate) fn segmented(&mut self, source: Segmented) {
        self.length += source.len();
        self.pieces.push_back(source);
    }

    /// Seal the queue as shared text and provenance, not a contiguous operand.
    pub(crate) fn seal(mut self) -> Segmented {
        let mut output = self
            .pieces
            .pop_front()
            .unwrap_or_else(|| ScopeSource::identity("").into());
        for piece in self.pieces {
            output.append(piece);
        }
        output
    }

    /// Transfer up to the requested bytes, splitting at a caller-validated UTF-8 boundary.
    pub(crate) fn take(&mut self, length: usize) -> Self {
        let mut output = Self::default();
        let mut remaining = length.min(self.len());
        while remaining > 0 {
            let Some(piece) = self.pieces.pop_front() else {
                break;
            };
            let end = remaining.min(piece.len());
            if end < piece.len() {
                self.pieces.push_front(piece.select(end..piece.len()));
            }
            output.length += end;
            output.pieces.push_back(piece.select(0..end));
            self.length -= end;
            remaining -= end;
        }
        output
    }

    /// Transfer the next physical line without flattening neighboring segments.
    pub(crate) fn line(&mut self) -> Option<Self> {
        if self.is_empty() {
            return None;
        }
        let mut length = 0;
        for chunk in self.pieces.iter().flat_map(Segmented::chunks) {
            let end = chunk.find('\n').map(|offset| offset + 1);
            let scanned = end.unwrap_or(chunk.len());
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.framing_bytes += scanned);
            length += scanned;
            if end.is_some() {
                break;
            }
        }
        Some(self.take(length))
    }

    /// Join only the operand admitted to a contiguous grammar consumer.
    pub(crate) fn materialize(self) -> ScopeSource {
        self.seal().materialize()
    }
}
