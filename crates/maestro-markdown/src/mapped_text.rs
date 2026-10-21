//! Shared ownership of authored UTF-8 text.
use crate::segments::Segments;
use std::{ops::Range, sync::Arc};

/// Owned authored text whose clones retain shared storage.
///
/// Read chunks without flattening, or explicitly materialize an owned string.
/// Chunk boundaries are an implementation detail, not a serialization format.
#[derive(Clone, Debug)]
pub struct MappedText {
    /// Shared byte operands and their logical offset index.
    storage: Segments<Bytes>,
    /// Final byte for constant-time semantic separator checks.
    last_byte: Option<u8>,
}

/// A UTF-8 window retaining its original byte allocation.
#[derive(Clone, Debug)]
struct Bytes(Arc<String>, Range<usize>);

impl MappedText {
    /// Return the logical UTF-8 byte length without visiting chunks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Whether this value contains no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Visit exact logical UTF-8 bytes in order without flattening them.
    pub fn chunks(&self) -> impl Iterator<Item = &str> {
        let mut pending = vec![(&self.storage, 0..self.len())];
        std::iter::from_fn(move || next_chunk(&mut pending))
    }

    /// Allocate the complete logical text as a new owned string.
    #[must_use]
    pub fn to_owned_string(&self) -> String {
        let mut output = String::with_capacity(self.len());
        for chunk in self.chunks() {
            output.push_str(chunk);
        }
        output
    }

    /// Retain a UTF-8 window of an existing source allocation.
    pub(crate) fn shared(bytes: Arc<String>, range: Range<usize>) -> Self {
        Self {
            last_byte: bytes[range.clone()].as_bytes().last().copied(),
            storage: Segments::new(Bytes(bytes, range.clone()), range.len(), range),
        }
    }

    /// Measure retained ownership depth without recursive diagnostics or destruction.
    #[cfg(test)]
    pub(crate) fn storage_depth(&self) -> usize {
        self.storage.depth()
    }

    /// Count live shared bytes and concatenation metadata without flattening a view.
    #[cfg(test)]
    pub(crate) fn retained(&self, retained: &mut crate::tests::Retained) {
        self.storage
            .retained(retained, |bytes, retained| retained.text(&bytes.0));
    }

    /// Retain a logical UTF-8 selection without materializing its bytes.
    pub(crate) fn select(&self, range: Range<usize>) -> Self {
        if range.is_empty() {
            return Self::default();
        }
        let storage = self.storage.select(range.clone(), |bytes, range| {
            Some(bytes.1.start + range.start..bytes.1.start + range.end)
        });
        let mut tail = &self.storage;
        let mut selected = range.end - 1..range.end;
        let last_byte = loop {
            if let Some((bytes, window)) = tail.leaf() {
                break bytes
                    .0
                    .as_bytes()
                    .get(bytes.1.start + window.start + selected.start)
                    .copied();
            }
            let Some((part, range, _)) = tail.parts(selected).next() else {
                break None;
            };
            tail = part;
            selected = range;
        };
        Self { storage, last_byte }
    }

    /// Whether the authored value already ends in a semantic line separator.
    pub(crate) fn ends_with_newline(&self) -> bool {
        self.last_byte == Some(b'\n')
    }

    /// Extend authored spelling without copying the retained prefix.
    pub(crate) fn append(&mut self, other: &Self) {
        if !other.is_empty() {
            self.storage.append(&other.storage);
            self.last_byte = other.last_byte;
        }
    }
}

/// Advance a byte cursor without recursive traversal of concatenations.
fn next_chunk<'a>(pending: &mut Vec<(&'a Segments<Bytes>, Range<usize>)>) -> Option<&'a str> {
    while let Some((source, range)) = pending.pop() {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.source_views += 1);
        if range.is_empty() {
            continue;
        }
        if let Some((bytes, window)) = source.leaf() {
            let start = bytes.1.start + window.start;
            return Some(&bytes.0[start + range.start..start + range.end]);
        }
        pending.extend(
            source
                .parts(range)
                .rev()
                .map(|(part, range, _)| (part, range)),
        );
    }
    None
}

impl Default for MappedText {
    fn default() -> Self {
        String::new().into()
    }
}

impl From<String> for MappedText {
    fn from(value: String) -> Self {
        let length = value.len();
        Self::shared(Arc::new(value), 0..length)
    }
}
