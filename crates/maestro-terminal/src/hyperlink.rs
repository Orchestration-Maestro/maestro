//! Owned hyperlink byte identities and their logical string storage.

use std::ops::Range;

/// A page-local opaque hyperlink identity.
pub type Id = u16;

/// Authored explicit bytes or a generated implicit numeric identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HyperlinkId {
    /// Byte-exact authored identity.
    Explicit(Vec<u8>),
    /// Generated numeric identity.
    Implicit(u32),
}

/// A decoded hyperlink whose bytes remain owned independently of its caller.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Hyperlink {
    /// Explicit or implicit identity, independent of the URI.
    pub id: HyperlinkId,
    /// Byte-exact URI, without a UTF-8 restriction.
    pub uri: Vec<u8>,
}

/// Logical string chunks belonging to one deduplicated identity.
#[derive(Clone)]
pub(crate) struct PageEntry {
    /// URI string allocation.
    pub(crate) uri_chunks: Range<usize>,
    /// Explicit identity string allocation, when present.
    pub(crate) id_chunks: Option<Range<usize>>,
}

/// The shared lifetime owner used for both style and hyperlink tables.
pub(crate) type Set = crate::ref_counted_set::RefCountedSet<Hyperlink>;

impl Hyperlink {
    /// Chunk-rounded string bytes counted once per distinct identity.
    pub(crate) fn string_bytes(&self) -> usize {
        use crate::bitmap_allocator::BitmapAllocator;
        let id_bytes = match &self.id {
            HyperlinkId::Explicit(bytes) => {
                BitmapAllocator::<32>::bytes_required::<u8>(bytes.len())
            }
            HyperlinkId::Implicit(_) => 0,
        };
        BitmapAllocator::<32>::bytes_required::<u8>(self.uri.len()) + id_bytes
    }
}
