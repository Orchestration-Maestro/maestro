//! Shared ownership of authored UTF-8 text.
use std::sync::Arc;

/// Owned authored text whose clones retain shared storage.
///
/// Read chunks without flattening, or explicitly materialize an owned string.
/// Chunk boundaries are an implementation detail, not a serialization format.
#[derive(Clone, Debug)]
pub struct MappedText {
    /// Shared bytes or concatenation, with no per-view flattened cache.
    storage: Arc<Storage>,
    /// Logical UTF-8 byte length.
    length: usize,
    /// Final byte for constant-time semantic separator checks.
    last_byte: Option<u8>,
}

/// Persistent concatenation keeps extending a value independent of its prefix length.
#[derive(Debug)]
enum Storage {
    /// Caller-owned contiguous bytes.
    Bytes(String),
    /// Shared earlier values in logical order.
    Joined(MappedText, MappedText),
}

impl MappedText {
    /// Return the logical UTF-8 byte length without visiting chunks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Whether this value contains no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Visit exact logical UTF-8 bytes in order without flattening them.
    pub fn chunks(&self) -> impl Iterator<Item = &str> {
        let mut pending = vec![self];
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

    /// Whether the authored value already ends in a semantic line separator.
    pub(crate) fn ends_with_newline(&self) -> bool {
        self.last_byte == Some(b'\n')
    }

    /// Extend authored spelling without copying the retained prefix.
    pub(crate) fn append(&mut self, other: &Self) {
        if other.is_empty() {
            return;
        }
        if self.is_empty() {
            *self = other.clone();
            return;
        }
        *self = Self {
            length: self.len() + other.len(),
            last_byte: other.last_byte,
            storage: Arc::new(Storage::Joined(self.clone(), other.clone())),
        };
    }
}

/// Advance a chunk cursor without recursive traversal of concatenations.
fn next_chunk<'a>(pending: &mut Vec<&'a MappedText>) -> Option<&'a str> {
    while let Some(text) = pending.pop() {
        match text.storage.as_ref() {
            Storage::Bytes(bytes) if !bytes.is_empty() => return Some(bytes.as_str()),
            Storage::Bytes(_) => {}
            Storage::Joined(left, right) => {
                pending.push(right);
                pending.push(left);
            }
        }
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
        Self {
            length: value.len(),
            last_byte: value.as_bytes().last().copied(),
            storage: Arc::new(Storage::Bytes(value)),
        }
    }
}
