//! Owned comment content and inline markers.
use super::super::OpenXmlElementTree;
use indexmap::IndexMap;
/// Comment metadata with an independent ordinary body arena.
#[derive(Debug, Default)]
pub struct WmlComment {
    /// Raw nullable comment identifier.
    pub id: Option<String>,
    /// Raw nullable author.
    pub author: Option<String>,
    /// Raw nullable initials.
    pub initials: Option<String>,
    /// Unparsed date spelling.
    pub date: Option<String>,
    /// Owned comment body.
    pub children: OpenXmlElementTree,
    /// Root CSS assigned by consumers; preparation starts empty.
    pub css_style: IndexMap<String, Option<String>>,
}

/// Inline comment reference.
#[derive(Debug, Default)]
pub struct WmlCommentReference {
    /// Raw nullable comment identifier.
    pub id: Option<String>,
}

/// Comment range opening marker.
#[derive(Debug, Default)]
pub struct WmlCommentRangeStart {
    /// Raw nullable comment identifier.
    pub id: Option<String>,
}

/// Comment range closing marker.
#[derive(Debug, Default)]
pub struct WmlCommentRangeEnd {
    /// Raw nullable comment identifier.
    pub id: Option<String>,
}
