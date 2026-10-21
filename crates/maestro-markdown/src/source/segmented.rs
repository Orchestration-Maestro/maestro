//! Shared logical text with provenance independent of chunk boundaries.
use super::{ScopeSource, SourceText};
use crate::MappedText;
use std::{ops::Range, sync::Arc};

/// A retained source splice; atomic spellings cover the whole logical value.
#[derive(Clone)]
pub(crate) struct Segmented {
    /// Logical bytes without a contiguous cache.
    text: MappedText,
    /// Shared projection before selecting this logical window.
    projection: Arc<Projection>,
    /// Selected logical boundaries in the shared projection.
    window: Range<usize>,
    /// Bounding native extent for pruning ordered projection branches.
    extent: Range<usize>,
}

/// Provenance shares the same concatenation boundaries as the logical bytes.
enum Projection {
    /// Existing contiguous provenance and its shared source allocation.
    Source(ScopeSource),
    /// Earlier source splices in logical order.
    Joined(Segmented, Segmented),
    /// One indivisible spelling, even when its text has many chunks.
    Atomic(Range<usize>, usize),
}

impl From<ScopeSource> for Segmented {
    fn from(source: ScopeSource) -> Self {
        let extent = source.extent(0..source.text.len());
        Self {
            extent,
            text: source.text.mapped(),
            window: 0..source.text.len(),
            projection: Arc::new(Projection::Source(source)),
        }
    }
}

impl Segmented {
    /// Logical UTF-8 bytes retained by this source splice.
    pub(crate) fn len(&self) -> usize {
        self.window.len()
    }

    /// Publish logical bytes without flattening them.
    pub(crate) fn mapped(&self) -> MappedText {
        self.text.clone()
    }

    /// Append another source handle without joining bytes or provenance arrays.
    pub(crate) fn append(&mut self, other: Self) {
        if other.len() == 0 {
            return;
        }
        if self.len() == 0 {
            *self = other;
            return;
        }
        let mut text = self.text.clone();
        text.append(&other.text);
        *self = Self {
            extent: self.extent.start..other.extent.end,
            window: 0..self.len() + other.len(),
            text,
            projection: Arc::new(Projection::Joined(self.clone(), other)),
        };
    }

    /// Discard a logical suffix without flattening retained body chunks.
    pub(crate) fn truncate(&mut self, end: usize) {
        self.extent = self
            .native(&(0..end))
            .unwrap_or(self.extent.start..self.extent.start);
        self.text = self.text.select(0..end);
        self.window.end = self.window.start + end;
    }

    /// Append a semantic separator at the existing native boundary.
    pub(crate) fn newline(&mut self) {
        let boundary = self.native(&(0..self.len())).map_or(0, |range| range.end);
        let text: MappedText = "\n".to_owned().into();
        self.append(Self {
            extent: boundary..boundary,
            text,
            projection: Arc::new(Projection::Atomic(boundary..boundary, 1)),
            window: 0..1,
        });
    }

    /// Replace the projection, not the chunks, with one atomic native spelling.
    pub(crate) fn spelling(text: MappedText, native: Range<usize>) -> Self {
        let length = text.len();
        Self {
            extent: native.clone(),
            text,
            projection: Arc::new(Projection::Atomic(native, length)),
            window: 0..length,
        }
    }

    /// Logical chunks in their exact authored order.
    pub(crate) fn chunks(&self) -> impl Iterator<Item = &str> {
        self.text.chunks()
    }

    /// Select logical bytes while retaining their shared projection.
    pub(crate) fn select(&self, mut range: Range<usize>) -> Self {
        let mut source = self;
        loop {
            if range == (0..source.len()) {
                return source.clone();
            }
            let absolute = source.window.start + range.start..source.window.start + range.end;
            match source.projection.as_ref() {
                Projection::Joined(left, _) if absolute.end <= left.len() => {
                    source = left;
                    range = absolute;
                }
                Projection::Joined(left, right) if absolute.start >= left.len() => {
                    range = absolute.start - left.len()..absolute.end - left.len();
                    source = right;
                }
                _ => {
                    return Self {
                        extent: source
                            .native(&range)
                            .unwrap_or(source.extent.end..source.extent.end),
                        text: source.text.select(range),
                        projection: Arc::clone(&source.projection),
                        window: absolute,
                    };
                }
            }
        }
    }

    /// Map logical bytes through one retained splice, including atomic spellings.
    pub(crate) fn native(&self, range: &Range<usize>) -> Option<Range<usize>> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.projections += 1);
        if *range == (0..self.len()) {
            return Some(self.extent.clone());
        }
        let range = self.window.start + range.start..self.window.start + range.end;
        match self.projection.as_ref() {
            Projection::Source(source) => source.native(&range),
            Projection::Atomic(native, length) => Some(project(native, *length, &range)),
            Projection::Joined(left, right) => {
                let start = range.start.min(left.len());
                let end = range.end.min(left.len());
                if range.end <= left.len() {
                    left.native(&(start..end))
                } else if range.start >= left.len() {
                    right.native(&(range.start - left.len()..range.end - left.len()))
                } else {
                    Some(
                        left.native(&(start..end))?.start
                            ..right.native(&(0..range.end - left.len()))?.end,
                    )
                }
            }
        }
    }

    /// Find a native range in the logical value without materializing its text.
    pub(crate) fn local(&self, range: &Range<usize>) -> Option<Range<usize>> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.projections += 1);
        if range.start <= self.extent.start && range.end >= self.extent.end {
            return Some(0..self.len());
        }
        if range.start > self.extent.end || range.end < self.extent.start {
            return None;
        }
        let result = match self.projection.as_ref() {
            Projection::Source(source) => source.local(range)?,
            Projection::Atomic(native, length) => {
                if range.end <= native.start || range.start >= native.end {
                    return None;
                }
                if native.len() == *length {
                    range.start.saturating_sub(native.start)
                        ..range.end.min(native.end) - native.start
                } else {
                    0..*length
                }
            }
            Projection::Joined(left, right) => {
                let a = left.local(range);
                let b = right.local(range);
                match (a, b) {
                    (Some(a), Some(b)) => a.start..left.len() + b.end,
                    (Some(a), None) => a,
                    (None, Some(b)) => left.len() + b.start..left.len() + b.end,
                    (None, None) => return None,
                }
            }
        };
        let start = result.start.max(self.window.start);
        let end = result.end.min(self.window.end);
        (start <= end).then(|| start - self.window.start..end - self.window.start)
    }

    /// Materialize one explicitly admitted contiguous operand with its whole-body map.
    pub(crate) fn materialize(&self) -> ScopeSource {
        if let Projection::Source(source) = self.projection.as_ref() {
            return source.select(std::iter::once(self.window.clone()));
        }
        let mut units = Vec::new();
        let mut pending = vec![(self, self.window.clone(), 0)];
        while let Some((source, range, offset)) = pending.pop() {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.projections += 1);
            source.materialize_parts(range, offset, &mut pending, &mut units);
        }
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.source_bytes += self.len());
        ScopeSource {
            text: SourceText {
                bytes: Arc::new(self.text.to_owned_string()),
                range: 0..self.len(),
            },
            units: Arc::new(units),
        }
    }
    /// Collect projection units for the selected branches of one shared splice.
    fn materialize_parts<'a>(
        &'a self,
        range: Range<usize>,
        offset: usize,
        pending: &mut Vec<(&'a Self, Range<usize>, usize)>,
        units: &mut Vec<(Range<usize>, Range<usize>)>,
    ) {
        match self.projection.as_ref() {
            Projection::Source(source) => {
                let selected = source.select(std::iter::once(range));
                units.extend(selected.unit_slice().iter().map(|unit| {
                    let (native, local) = selected.unit(unit);
                    (native, offset + local.start..offset + local.end)
                }));
            }
            Projection::Atomic(native, length) => {
                units.push((
                    project(native, *length, &range),
                    offset..offset + range.len(),
                ));
            }
            Projection::Joined(left, right) => {
                let split = left.len();
                if range.end > split {
                    let start = range.start.saturating_sub(split);
                    let right_range =
                        right.window.start + start..right.window.start + range.end - split;
                    pending.push((
                        right,
                        right_range,
                        offset + split.saturating_sub(range.start),
                    ));
                }
                if range.start < split {
                    pending.push((
                        left,
                        left.window.start + range.start..left.window.start + range.end.min(split),
                        offset,
                    ));
                }
            }
        }
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
