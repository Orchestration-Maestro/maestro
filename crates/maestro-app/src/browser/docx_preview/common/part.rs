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

/// Retained XML, relationships and prepared semantic data.
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
    /// Prepared semantic data for implemented part kinds.
    pub data: Option<PartData>,
}

/// Single retained semantic representation on a cached package part.
#[derive(Debug)]
pub enum PartData {
    /// Core document properties.
    CoreProps(Box<super::super::CorePropsPart>),
    /// Extended application properties.
    ExtendedProps(Box<super::super::ExtendedPropsPart>),
    /// Raw custom property values.
    CustomProps(Box<super::super::CustomPropsPart>),
    /// Shared comments with owned body arenas.
    Comments(Box<super::super::CommentsPart>),
    /// Shared extended comment links.
    CommentsExtended(Box<super::super::CommentsExtendedPart>),
}
impl PartData {
    /// Prepare supported semantic data from already admitted XML.
    pub(crate) fn parse_xml(
        kind: PartKind,
        xml: &super::super::XmlParser<'_>,
    ) -> Result<Option<Self>, super::super::DocxError> {
        use super::super::{
            CommentsExtendedPart, CommentsPart, CorePropsPart, CustomPropsPart, ExtendedPropsPart,
            OwnedXmlElement,
        };
        let root = xml.root();
        Ok(Some(match kind {
            PartKind::CoreProps => Self::CoreProps(Box::new(CorePropsPart::parse_xml(xml, root))),
            PartKind::ExtendedProps => {
                Self::ExtendedProps(Box::new(ExtendedPropsPart::parse_xml(xml, root)))
            }
            PartKind::CustomProps => {
                Self::CustomProps(Box::new(CustomPropsPart::parse_xml(xml, root)?))
            }
            PartKind::Comments => Self::Comments(Box::new(CommentsPart::parse_xml(
                &OwnedXmlElement::new(root),
            )?)),
            PartKind::CommentsExtended => {
                Self::CommentsExtended(Box::new(CommentsExtendedPart::parse_xml(xml, root)))
            }
            _ => return Ok(None),
        }))
    }
}
