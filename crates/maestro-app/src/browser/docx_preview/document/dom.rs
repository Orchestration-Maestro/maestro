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
    /// Embedded picture element.
    Image,
    /// `MmlFraction` element.
    MmlFraction,
    /// `MmlFunction` element.
    MmlFunction,
    /// `MmlFunctionName` element.
    MmlFunctionName,
    /// `MmlNumerator` element.
    MmlNumerator,
    /// `MmlDenominator` element.
    MmlDenominator,
    /// `MmlRadical` element.
    MmlRadical,
    /// `MmlBase` element.
    MmlBase,
    /// `MmlDegree` element.
    MmlDegree,
    /// `MmlSuperscript` element.
    MmlSuperscript,
    /// `MmlSubscript` element.
    MmlSubscript,
    /// `MmlPreSubSuper` element.
    MmlPreSubSuper,
    /// `MmlSubArgument` element.
    MmlSubArgument,
    /// `MmlSuperArgument` element.
    MmlSuperArgument,
    /// `MmlNary` element.
    MmlNary,
    /// `MmlDelimiter` element.
    MmlDelimiter,
    /// `MmlRun` element.
    MmlRun,
    /// `MmlEquationArray` element.
    MmlEquationArray,
    /// `MmlLimit` element.
    MmlLimit,
    /// `MmlLimitLower` element.
    MmlLimitLower,
    /// `MmlMatrix` element.
    MmlMatrix,
    /// `MmlMatrixRow` element.
    MmlMatrixRow,
    /// `MmlBox` element.
    MmlBox,
    /// `MmlBar` element.
    MmlBar,
    /// `MmlGroupChar` element.
    MmlGroupChar,
    /// Drawing element.
    Drawing,
    /// VML shape element.
    VmlElement,
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
            Self::Image => "image",
            Self::MmlFraction => "mmlFraction",
            Self::MmlFunction => "mmlFunction",
            Self::MmlFunctionName => "mmlFunctionName",
            Self::MmlNumerator => "mmlNumerator",
            Self::MmlDenominator => "mmlDenominator",
            Self::MmlRadical => "mmlRadical",
            Self::MmlBase => "mmlBase",
            Self::MmlDegree => "mmlDegree",
            Self::MmlSuperscript => "mmlSuperscript",
            Self::MmlSubscript => "mmlSubscript",
            Self::MmlPreSubSuper => "mmlPreSubSuper",
            Self::MmlSubArgument => "mmlSubArgument",
            Self::MmlSuperArgument => "mmlSuperArgument",
            Self::MmlNary => "mmlNary",
            Self::MmlDelimiter => "mmlDelimiter",
            Self::MmlRun => "mmlRun",
            Self::MmlEquationArray => "mmlEquationArray",
            Self::MmlLimit => "mmlLimit",
            Self::MmlLimitLower => "mmlLimitLower",
            Self::MmlMatrix => "mmlMatrix",
            Self::MmlMatrixRow => "mmlMatrixRow",
            Self::MmlBox => "mmlBox",
            Self::MmlBar => "mmlBar",
            Self::MmlGroupChar => "mmlGroupChar",
            Self::Drawing => "drawing",
            Self::VmlElement => "vmlElement",
            Self::VmlPicture => "vmlPicture",
        }
    }
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
        /// Run or math-run kind assigned by its producer.
        r#type: DomType,
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
    /// Drawing wrapper with ordered CSS values.
    Drawing {
        /// Converted wrapper styles.
        css_style: indexmap::IndexMap<String, Option<String>>,
    },
    /// Picture relationship and geometry.
    Image {
        /// Nullable relationship identifier.
        src: Option<String>,
        /// Ordered converted geometry.
        css_style: indexmap::IndexMap<String, Option<String>>,
        /// Crop sides in left, top, right, bottom order.
        src_rect: Option<[f64; 4]>,
        /// Unnormalized degrees.
        rotation: Option<f64>,
    },
    /// VML picture container.
    VmlPicture,
    /// Completed VML shape.
    VmlElement(super::super::VmlElement),
    /// Bookmark start marker.
    BookmarkStart {
        /// Nullable identifier.
        id: Option<String>,
        /// Nullable authored name.
        name: Option<String>,
        /// Nullable first column.
        col_first: Option<f64>,
        /// Nullable last column.
        col_last: Option<f64>,
    },
    /// Bookmark end marker.
    BookmarkEnd {
        /// Nullable identifier.
        id: Option<String>,
    },
    /// Recursive typed equation element.
    Math {
        /// Selected equation kind.
        r#type: DomType,
        /// Last matching property group.
        props: Option<super::super::MathProperties>,
    },
    /// Inline comment reference.
    CommentReference(super::super::WmlCommentReference),
    /// Comment range opening marker.
    CommentRangeStart(super::super::WmlCommentRangeStart),
    /// Comment range closing marker.
    CommentRangeEnd(super::super::WmlCommentRangeEnd),
    /// Unparsed content.
    Unparsed {
        /// Known eventual document kind.
        kind: DomType,
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
    pub fn dom_type(&self) -> DomType {
        match self.data {
            OpenXmlElementData::CommentReference(_) => DomType::CommentReference,
            OpenXmlElementData::CommentRangeStart(_) => DomType::CommentRangeStart,
            OpenXmlElementData::CommentRangeEnd(_) => DomType::CommentRangeEnd,
            OpenXmlElementData::Unparsed { kind, .. } => kind,
            OpenXmlElementData::Image { .. } => DomType::Image,
            OpenXmlElementData::VmlPicture => DomType::VmlPicture,
            OpenXmlElementData::VmlElement(_) => DomType::VmlElement,
            OpenXmlElementData::Drawing { .. } => DomType::Drawing,
            OpenXmlElementData::Paragraph { .. } => DomType::Paragraph,
            OpenXmlElementData::Run { r#type, .. } | OpenXmlElementData::Math { r#type, .. } => {
                r#type
            }
            OpenXmlElementData::BookmarkStart { .. } => DomType::BookmarkStart,
            OpenXmlElementData::BookmarkEnd { .. } => DomType::BookmarkEnd,
            OpenXmlElementData::Text { .. } => DomType::Text,
            OpenXmlElementData::DeletedText { .. } => DomType::DeletedText,
            OpenXmlElementData::Instruction { .. } => DomType::Instruction,
            OpenXmlElementData::Break { .. } => DomType::Break,
            OpenXmlElementData::NoBreakHyphen => DomType::NoBreakHyphen,
            OpenXmlElementData::Tab => DomType::Tab,
            OpenXmlElementData::Symbol { .. } => DomType::Symbol,
            OpenXmlElementData::FootnoteReference { .. } => DomType::FootnoteReference,
            OpenXmlElementData::EndnoteReference { .. } => DomType::EndnoteReference,
            OpenXmlElementData::SimpleField { .. } => DomType::SimpleField,
            OpenXmlElementData::ComplexField { .. } => DomType::ComplexField,
            OpenXmlElementData::Inserted => DomType::Inserted,
            OpenXmlElementData::Deleted => DomType::Deleted,
            OpenXmlElementData::AltChunk { .. } => DomType::AltChunk,
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

mod numbering;
pub use numbering::{IDomNumbering, NumberingPicBullet};
