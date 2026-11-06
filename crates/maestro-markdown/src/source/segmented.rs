//! Shared logical text with provenance independent of chunk boundaries.
use super::{ScopeSource, SourceText};
use crate::{MappedText, segments::Segments};
use std::{ops::Range, sync::Arc};

/// A retained source splice; atomic spellings cover the whole logical value.
#[derive(Clone)]
pub(crate) struct Segmented {
    /// Logical bytes without a contiguous cache.
    text: MappedText,
    /// Offset-indexed provenance using the same shared sequence owner as text.
    projection: Segments<Projection>,
}

/// Source operands retain either their native map or one indivisible spelling.
#[derive(Clone)]
enum Projection {
    /// Existing contiguous provenance and its shared source allocation.
    Source(ScopeSource),
    /// One indivisible spelling, even when its text has many chunks.
    Atomic(Range<usize>, usize),
}

impl From<ScopeSource> for Segmented {
    fn from(source: ScopeSource) -> Self {
        let length = source.text.len();
        let extent = source.extent(0..length);
        Self {
            text: source.text.mapped(),
            projection: Segments::new(Projection::Source(source), length, extent),
        }
    }
}

impl Segmented {
    /// Logical UTF-8 bytes retained by this source splice.
    pub(crate) fn len(&self) -> usize {
        self.text.len()
    }

    /// Publish logical bytes without flattening them.
    pub(crate) fn mapped(&self) -> MappedText {
        self.text.clone()
    }

    /// Append another source handle without joining bytes or provenance arrays.
    pub(crate) fn append(&mut self, other: &Self) {
        self.text.append(&other.text);
        self.projection.append(&other.projection);
    }

    /// Discard a logical suffix without flattening retained body chunks.
    pub(crate) fn truncate(&mut self, end: usize) {
        self.text = self.text.select(0..end);
        self.projection = self.projection.select(0..end, native);
    }

    /// Append a semantic separator at the existing native boundary.
    pub(crate) fn newline(&mut self) {
        let boundary = self.projection.extent.end;
        self.append(&Self::spelling("\n".to_owned().into(), boundary..boundary));
    }

    /// Replace the projection, not the chunks, with one atomic native spelling.
    pub(crate) fn spelling(text: MappedText, extent: Range<usize>) -> Self {
        let length = text.len();
        Self {
            text,
            projection: Segments::new(Projection::Atomic(extent.clone(), length), length, extent),
        }
    }

    /// Logical chunks in their exact authored order.
    pub(crate) fn chunks(&self) -> impl Iterator<Item = &str> {
        self.text.chunks()
    }

    /// Visit only the chunks overlapping one logical window.
    pub(crate) fn chunks_in(&self, range: Range<usize>) -> impl Iterator<Item = &str> {
        self.text.chunks_in(range)
    }

    /// Select the first physical line and its immediately following blank lines.
    pub(crate) fn first_line(&self) -> ScopeSource {
        let mut end = 0;
        let mut boundary = None;
        for byte in self.chunks().flat_map(str::bytes) {
            if let Some(boundary) = boundary
                && !matches!(byte, b' ' | b'\t' | b'\n')
            {
                #[cfg(test)]
                crate::tests::WORK.with_borrow_mut(|work| work.framing_bytes += end + 1);
                return self.select(0..boundary).materialize();
            }
            end += 1;
            if byte == b'\n' {
                boundary = Some(end);
            }
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.framing_bytes += end);
        self.materialize()
    }

    /// Whether the first non-whitespace scalar from `offset` lies on a terminated line, so
    /// grammar that stopped before it never read this operand's apparent end.
    pub(crate) fn settled_after(&self, offset: usize) -> bool {
        let mut content = false;
        for scalar in self.chunks_in(offset..self.len()).flat_map(str::chars) {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.framing_bytes += scalar.len_utf8());
            if scalar == '\n' && content {
                return true;
            }
            content |= !maestro_text::is_whitespace_scalar(scalar);
        }
        false
    }

    /// Charge the unique text and provenance this splice keeps alive.
    #[cfg(test)]
    pub(crate) fn retained(&self, retained: &mut crate::tests::Retained) {
        self.text.retained(retained);
        self.projection.retained(retained, |value, retained| {
            if let Projection::Source(source) = value {
                source.retained(retained);
            }
        });
    }

    /// Select logical bytes while retaining their shared projection.
    pub(crate) fn select(&self, range: Range<usize>) -> Self {
        Self {
            text: self.text.select(range.clone()),
            projection: self.projection.select(range, native),
        }
    }

    /// Map logical bytes through one retained splice, including atomic spellings.
    pub(crate) fn native(&self, range: &Range<usize>) -> Option<Range<usize>> {
        self.projection.native(range, native)
    }

    /// Whether the logical bytes end with the supplied authored value.
    pub(crate) fn ends_with(&self, body: &MappedText) -> bool {
        let Some(start) = self.len().checked_sub(body.len()) else {
            return false;
        };
        let tail = self.chunks().flat_map(str::as_bytes).skip(start);
        tail.eq(body.chunks().flat_map(str::as_bytes))
    }

    /// Find a native range in the logical value without materializing its text.
    pub(crate) fn local(&self, range: &Range<usize>) -> Option<Range<usize>> {
        self.projection.local(range, local)
    }

    /// Materialize one explicitly admitted contiguous operand with its whole-body map.
    pub(crate) fn materialize(&self) -> ScopeSource {
        let mut prefix = self.prefix();
        prefix.extend(self.len());
        prefix.finish()
    }

    /// Start an empty contiguous prefix that a grammar grows over this operand.
    pub(crate) fn prefix(&self) -> Prefix<'_> {
        match self.projection.leaf() {
            Some((Projection::Source(source), window)) => {
                Prefix::Leaf(source, window.start..window.start)
            }
            _ => Prefix::Copied {
                source: self,
                text: String::new(),
                units: Vec::new(),
            },
        }
    }

    /// Append one logical window's bytes and native map to a contiguous copy that already
    /// holds every byte before it.
    fn copy(
        &self,
        range: Range<usize>,
        text: &mut String,
        units: &mut Vec<(Range<usize>, Range<usize>)>,
    ) {
        let mut pending = vec![(&self.projection, range.clone(), text.len())];
        while let Some((source, range, offset)) = pending.pop() {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.projections += 1);
            if let Some((value, window)) = source.leaf() {
                materialize_leaf(
                    value,
                    window.start + range.start..window.start + range.end,
                    offset,
                    units,
                );
            } else {
                pending.extend(
                    source
                        .parts(range)
                        .rev()
                        .map(|(part, range, start)| (part, range, offset + start)),
                );
            }
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.source_bytes += range.len());
        text.reserve_exact(range.len());
        text.extend(self.chunks_in(range));
    }
}

/// A contiguous prefix of one operand that grows without copying any byte twice.
pub(crate) enum Prefix<'a> {
    /// A window of the operand's single contiguous leaf, selected without copying.
    Leaf(&'a ScopeSource, Range<usize>),
    /// Bytes and native map copied from a spliced operand.
    Copied {
        /// Operand the prefix grows over.
        source: &'a Segmented,
        /// Bytes copied so far.
        text: String,
        /// Native map of the copied bytes.
        units: Vec<(Range<usize>, Range<usize>)>,
    },
}

impl Prefix<'_> {
    /// Grow the prefix through logical byte `end`, copying only the bytes it adds.
    pub(crate) fn extend(&mut self, end: usize) {
        match self {
            Self::Leaf(_, window) => window.end = window.start + end,
            Self::Copied {
                source,
                text,
                units,
            } => source.copy(text.len()..end, text, units),
        }
    }

    /// Borrow the prefix's bytes.
    pub(crate) fn text(&self) -> &str {
        match self {
            Self::Leaf(source, window) => &source.text[window.clone()],
            Self::Copied { text, .. } => text,
        }
    }

    /// Publish the prefix with its native map.
    pub(crate) fn finish(self) -> ScopeSource {
        match self {
            Self::Leaf(source, window) => source.select(std::iter::once(window)),
            Self::Copied { text, units, .. } => ScopeSource {
                text: SourceText {
                    range: 0..text.len(),
                    bytes: Arc::new(text),
                },
                units: Arc::new(units),
            },
        }
    }
}

/// Project one retained source operand to its native coordinates.
fn native(value: &Projection, range: Range<usize>) -> Option<Range<usize>> {
    match value {
        Projection::Source(source) => source.native(&range),
        Projection::Atomic(native, length) => Some(project(native, *length, &range)),
    }
}

/// Invert one retained operand before clipping its selected logical window.
fn local(value: &Projection, range: &Range<usize>) -> Option<Range<usize>> {
    match value {
        Projection::Source(source) => source.local(range),
        Projection::Atomic(native, length) => {
            if range.end <= native.start || range.start >= native.end {
                return None;
            }
            if native.len() == *length {
                Some(
                    range.start.saturating_sub(native.start)
                        ..range.end.min(native.end) - native.start,
                )
            } else {
                Some(0..*length)
            }
        }
    }
}

/// Collect the native units of one selected contiguous or atomic operand.
fn materialize_leaf(
    value: &Projection,
    range: Range<usize>,
    offset: usize,
    units: &mut Vec<(Range<usize>, Range<usize>)>,
) {
    match value {
        Projection::Source(source) => {
            let selected = source.select(std::iter::once(range));
            units.extend(selected.unit_slice().iter().map(|unit| {
                let (native, local) = selected.unit(unit);
                (native, offset + local.start..offset + local.end)
            }));
        }
        Projection::Atomic(native, length) => units.push((
            project(native, *length, &range),
            offset..offset + range.len(),
        )),
    }
}

/// Retain identity offsets when lengths agree, otherwise keep an atomic extent.
fn project(native: &Range<usize>, length: usize, range: &Range<usize>) -> Range<usize> {
    if native.len() == length {
        native.start + range.start..native.start + range.end
    } else {
        native.clone()
    }
}
