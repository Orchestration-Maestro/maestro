//! Owned note records with static note kinds.
use super::super::OpenXmlElementTree;
/// Shared raw note metadata and body content.
#[derive(Debug, Default)]
pub struct WmlBaseNote {
    /// Nullable raw note identifier.
    pub id: Option<String>,
    /// Nullable open note type.
    pub note_type: Option<String>,
    /// Owned body content.
    pub children: OpenXmlElementTree,
}
/// Footnote with shared note data.
#[derive(Debug, Default)]
pub struct WmlFootnote {
    /// Owned metadata and body.
    pub base: WmlBaseNote,
}
/// Endnote with shared note data.
#[derive(Debug, Default)]
pub struct WmlEndnote {
    /// Owned metadata and body.
    pub base: WmlBaseNote,
}
