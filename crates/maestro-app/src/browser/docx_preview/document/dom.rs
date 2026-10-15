//! Owned document element records.
use super::super::OwnedXmlElement;
/// Recognized document element kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomType {
    /// Paragraph element.
    Paragraph,
    /// Run element.
    Run,
    /// Break element.
    Break,
    /// `NoBreakHyphen` element.
    NoBreakHyphen,
    /// Text element.
    Text,
    /// Tab element.
    Tab,
    /// Symbol element.
    Symbol,
    /// `FootnoteReference` element.
    FootnoteReference,
    /// `EndnoteReference` element.
    EndnoteReference,
    /// `SimpleField` element.
    SimpleField,
    /// `ComplexField` element.
    ComplexField,
    /// Instruction element.
    Instruction,
    /// Inserted element.
    Inserted,
    /// Deleted element.
    Deleted,
    /// `DeletedText` element.
    DeletedText,
    /// `AltChunk` element.
    AltChunk,
    /// Table element.
    Table,
    /// Hyperlink element.
    Hyperlink,
    /// `SmartTag` element.
    SmartTag,
    /// `BookmarkStart` element.
    BookmarkStart,
    /// `BookmarkEnd` element.
    BookmarkEnd,
    /// `CommentRangeStart` element.
    CommentRangeStart,
    /// `CommentRangeEnd` element.
    CommentRangeEnd,
    /// `MmlMath` element.
    MmlMath,
    /// `MmlMathParagraph` element.
    MmlMathParagraph,
    /// `CommentReference` element.
    CommentReference,
    /// Drawing element.
    Drawing,
    /// `VmlPicture` element.
    VmlPicture,
}
impl DomType {
    /// Return the document kind's interchange spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Run => "run",
            Self::Break => "break",
            Self::NoBreakHyphen => "noBreakHyphen",
            Self::Text => "text",
            Self::Tab => "tab",
            Self::Symbol => "symbol",
            Self::FootnoteReference => "footnoteReference",
            Self::EndnoteReference => "endnoteReference",
            Self::SimpleField => "simpleField",
            Self::ComplexField => "complexField",
            Self::Instruction => "instruction",
            Self::Inserted => "inserted",
            Self::Deleted => "deleted",
            Self::DeletedText => "deletedText",
            Self::AltChunk => "altChunk",
            Self::Table => "table",
            Self::Hyperlink => "hyperlink",
            Self::SmartTag => "smartTag",
            Self::BookmarkStart => "bookmarkStart",
            Self::BookmarkEnd => "bookmarkEnd",
            Self::CommentRangeStart => "commentRangeStart",
            Self::CommentRangeEnd => "commentRangeEnd",
            Self::MmlMath => "mmlMath",
            Self::MmlMathParagraph => "mmlMathParagraph",
            Self::CommentReference => "commentReference",
            Self::Drawing => "drawing",
            Self::VmlPicture => "vmlPicture",
        }
    }
}
/// Raw branch awaiting its semantic parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnparsedKind {
    /// Recognized eventual document element kind.
    Element(DomType),
    /// Unselected alternate-content XML wrapper.
    AlternateContent,
}
/// Typed document payload.
#[derive(Debug)]
pub enum OpenXmlElementData {
    /// Paragraph content.
    Paragraph {
        /// Completed paragraph properties.
        properties: Box<super::paragraph::ParagraphProperties>,
    },
    /// Run content.
    Run {
        /// Whether instructions or complex field markers were encountered.
        field_run: bool,
        /// Completed run properties.
        properties: super::run::RunProperties,
    },
    /// Text content.
    Text {
        /// Descendant text in source order.
        text: String,
    },
    /// `DeletedText` content.
    DeletedText {
        /// Deleted descendant text.
        text: String,
    },
    /// Instruction content.
    Instruction {
        /// Field instruction text.
        text: String,
    },
    /// Break content.
    Break {
        /// Open break type spelling.
        r#break: String,
    },
    /// `NoBreakHyphen` content.
    NoBreakHyphen,
    /// Tab content.
    Tab,
    /// Symbol content.
    Symbol {
        /// CSS font family value.
        font: Option<String>,
        /// Unconverted character spelling.
        char: Option<String>,
    },
    /// `FootnoteReference` content.
    FootnoteReference {
        /// Nullable note identifier.
        id: Option<String>,
    },
    /// `EndnoteReference` content.
    EndnoteReference {
        /// Nullable note identifier.
        id: Option<String>,
    },
    /// `SimpleField` content.
    SimpleField {
        /// Nullable field instruction.
        instruction: Option<String>,
        /// Field lock flag.
        lock: bool,
        /// Field dirty flag.
        dirty: bool,
    },
    /// `ComplexField` content.
    ComplexField {
        /// Nullable open field character type.
        char_type: Option<String>,
        /// Field lock flag.
        lock: bool,
        /// Field dirty flag.
        dirty: bool,
    },
    /// Inserted content.
    Inserted,
    /// Deleted content.
    Deleted,
    /// `AltChunk` content.
    AltChunk {
        /// Nullable relationship identifier.
        id: Option<String>,
    },
    /// Unparsed content.
    Unparsed {
        /// Eventual kind or unselected wrapper.
        kind: UnparsedKind,
        /// Shared complete XML source and selected range.
        element: OwnedXmlElement,
    },
}
/// Common document links.
#[derive(Debug, Default)]
pub struct OpenXmlElementBase {
    /// Ordered child arena positions.
    pub children: Vec<usize>,
    /// Parent arena position when assigned by the parser.
    pub parent: Option<usize>,
}
/// One owned arena element.
#[derive(Debug)]
pub struct OpenXmlElement {
    /// Arena links.
    pub base: OpenXmlElementBase,
    /// Typed element content.
    pub data: OpenXmlElementData,
}
impl OpenXmlElement {
    /// Return the completed or eventual document kind.
    #[must_use]
    pub fn dom_type(&self) -> Option<DomType> {
        match self.data {
            OpenXmlElementData::Unparsed {
                kind: UnparsedKind::AlternateContent,
                ..
            } => None,
            OpenXmlElementData::Unparsed {
                kind: UnparsedKind::Element(kind),
                ..
            } => Some(kind),
            OpenXmlElementData::Paragraph { .. } => Some(DomType::Paragraph),
            OpenXmlElementData::Run { .. } => Some(DomType::Run),
            OpenXmlElementData::Text { .. } => Some(DomType::Text),
            OpenXmlElementData::DeletedText { .. } => Some(DomType::DeletedText),
            OpenXmlElementData::Instruction { .. } => Some(DomType::Instruction),
            OpenXmlElementData::Break { .. } => Some(DomType::Break),
            OpenXmlElementData::NoBreakHyphen => Some(DomType::NoBreakHyphen),
            OpenXmlElementData::Tab => Some(DomType::Tab),
            OpenXmlElementData::Symbol { .. } => Some(DomType::Symbol),
            OpenXmlElementData::FootnoteReference { .. } => Some(DomType::FootnoteReference),
            OpenXmlElementData::EndnoteReference { .. } => Some(DomType::EndnoteReference),
            OpenXmlElementData::SimpleField { .. } => Some(DomType::SimpleField),
            OpenXmlElementData::ComplexField { .. } => Some(DomType::ComplexField),
            OpenXmlElementData::Inserted => Some(DomType::Inserted),
            OpenXmlElementData::Deleted => Some(DomType::Deleted),
            OpenXmlElementData::AltChunk { .. } => Some(DomType::AltChunk),
        }
    }
}
/// Owned roots and arena, including parent-retained containers.
#[derive(Debug, Default)]
pub struct OpenXmlElementTree {
    /// Ordered root arena positions.
    pub roots: Vec<usize>,
    /// Element storage; links refer to positions here.
    pub nodes: Vec<OpenXmlElement>,
}
