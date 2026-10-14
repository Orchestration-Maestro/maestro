//! Recognized XML package parts.
use super::relationship::Relationship;

/// Kind of XML part admitted by the package reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    /// Main document.
    Document,
    /// Font declarations.
    FontTable,
    /// List numbering.
    Numbering,
    /// Styles.
    Styles,
    /// Theme.
    Theme,
    /// Footnotes.
    Footnotes,
    /// Endnotes.
    Endnotes,
    /// Footer.
    Footer,
    /// Header.
    Header,
    /// Core metadata.
    CoreProps,
    /// Extended metadata.
    ExtendedProps,
    /// Custom metadata.
    CustomProps,
    /// Document settings.
    Settings,
    /// Comments.
    Comments,
    /// Extended comments.
    CommentsExtended,
}

impl PartKind {
    /// Select a supported relationship URI without accepting unrelated namespaces.
    pub(crate) fn from_type(uri: &str) -> Option<Self> {
        match uri {
            "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" => {
                Some(Self::CoreProps)
            }
            "http://schemas.openxmlformats.org/package/2006/relationships/metadata/custom-properties" => {
                Some(Self::CustomProps)
            }
            "http://schemas.microsoft.com/office/2011/relationships/commentsExtended" => {
                Some(Self::CommentsExtended)
            }
            _ => match uri.strip_prefix(
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/",
            )? {
                "officeDocument" => Some(Self::Document),
                "fontTable" => Some(Self::FontTable),
                "numbering" => Some(Self::Numbering),
                "styles" => Some(Self::Styles),
                "theme" => Some(Self::Theme),
                "footnotes" => Some(Self::Footnotes),
                "endnotes" => Some(Self::Endnotes),
                "footer" => Some(Self::Footer),
                "header" => Some(Self::Header),
                "extended-properties" => Some(Self::ExtendedProps),
                "settings" => Some(Self::Settings),
                "comments" => Some(Self::Comments),
                _ => None,
            },
        }
    }
}

/// Retained XML and relationships; semantic interpretation belongs to consumers.
#[derive(Debug)]
pub struct Part {
    /// Resolved relationship-path cache key.
    pub path: String,
    /// Supported part classification.
    pub kind: PartKind,
    /// Ordered relationships, absent for missing or empty relationship files.
    pub rels: Option<Vec<Relationship>>,
    /// Lossily decoded source XML, before declaration handling.
    pub xml: String,
}
