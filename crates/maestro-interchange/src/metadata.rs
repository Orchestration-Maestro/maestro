//! Front matter records a guest shares with its host: the ordered value tree of a document's
//! metadata header and that tree paired with the document's body.
//!
//! The resource owner and guest authoring re-export these records at their own paths.

/// An ordered metadata value retaining scalar kinds, nonfinite numbers included.
#[derive(Clone, Debug, PartialEq)]
pub enum FrontmatterValue {
    /// A null scalar.
    Null,
    /// A boolean scalar.
    Bool(bool),
    /// A numeric scalar retaining signed zero, NaN and infinities.
    Number(f64),
    /// A text scalar.
    String(String),
    /// An ordered collection.
    Sequence(Vec<Self>),
    /// A string-keyed collection in insertion order.
    Mapping(Vec<(String, Self)>),
}

/// Resolved metadata with its extracted document body.
#[derive(Debug, PartialEq)]
pub struct ParsedFrontmatter {
    /// The complete value tree.
    pub frontmatter: FrontmatterValue,
    /// Body text, trimmed only when a header is present.
    pub body: String,
}
