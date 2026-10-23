//! In-memory Word package reading and borrowed XML queries.
mod document;
pub use document::common::{LengthUsage, convert_boolean, convert_length};
pub(crate) use document::common::{float_prefix, integer_prefix};
mod parser;
pub(crate) use parser::xml_parser::decode_text;
pub use parser::xml_parser::{
    OwnedXmlElement, XmlAttribute, XmlElement, XmlParser, parse_xml_string,
};

/// Failure reading a document package or building extracted text.
#[derive(Debug)]
pub enum DocxError {
    /// An existing embedded resource has no supplied key.
    FontKey,
    /// Archive decoding or entry-read failure.
    Archive(String),
    /// Extracted text exceeds 512 MiB or its output allocation failed.
    Output(String),
    /// XML admission failure with an optional package path.
    Xml {
        /// Failing package part, if known.
        path: Option<String>,
        /// Native parser diagnostic.
        message: String,
    },
}
impl std::fmt::Display for DocxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FontKey => f.write_str("Embedded font key is missing"),
            Self::Archive(message) | Self::Output(message) => write!(f, "{message}"),
            Self::Xml { path, message } => {
                write!(f, "{}: {message}", path.as_deref().unwrap_or("XML"))
            }
        }
    }
}
impl std::error::Error for DocxError {}
mod common;
pub(crate) use common::open_xml_package::OpenXmlPackage;
#[cfg(test)]
mod tests;
mod utils;
pub use utils::enclose_font_family;
pub(crate) use utils::resolve_path;
mod word_document;
pub use common::{
    part::{Part, PartData, PartKind},
    relationship::Relationship,
};
pub use word_document::WordDocument;

/// XML admission options for the package reader.
#[derive(Clone, Copy)]
pub struct Options {
    /// Remove a leading XML declaration before parsing.
    pub trim_xml_declaration: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            trim_xml_declaration: true,
        }
    }
}

mod document_parser;
pub use document::dom::{
    DomType, OpenXmlElement, OpenXmlElementBase, OpenXmlElementData, OpenXmlElementTree,
    WmlHyperlink, WmlSmartTag, WmlTable, WmlTableCell, WmlTableColumn, WmlTableRow,
};
pub use document_parser::{
    DefaultPropertiesOptions, TableStyle, WordBody, parse_background, parse_body_elements,
    parse_border_properties, parse_default_properties, parse_document_file, parse_paragraph,
    parse_run, parse_table_style,
};

pub use document::border::{Border, Borders};
pub use document::common::CommonProperties;
pub use document::paragraph::{LineSpacing, Numbering, ParagraphProperties, TabStop};
pub use document::run::{RunProperties, VerticalAlign};
pub use document::section::{
    Column, Columns, FooterHeaderReference, PageMargins, PageNumber, PageSize, SectionProperties,
};

mod styles;
pub use document::style::{IDomStyle, IDomSubStyle};
pub use styles::StylesPart;
mod theme;
pub use theme::{DmlColorScheme, DmlFontScheme, DmlFormInfo, DmlTheme, ThemePart};
mod header_footer;
pub use header_footer::{FooterPart, HeaderPart, WmlFooter, WmlHeader};
mod notes;
pub use notes::{EndnotesPart, FootnotesPart, WmlBaseNote, WmlEndnote, WmlFootnote};
mod settings;
pub use settings::{NoteProperties, SettingsPart, WmlSettings};
mod vml;
pub use vml::{VmlAttributeValue, VmlElement, VmlImageHref, VmlTag};

pub use document_parser::MathProperties;

mod document_props;
pub use document_props::{
    CorePropsDeclaration, CorePropsPart, CustomProperty, CustomPropsPart, ExtendedPropsDeclaration,
    ExtendedPropsPart,
};

mod comments;
pub use comments::{
    CommentsExtended, CommentsExtendedPart, CommentsPart, WmlComment, WmlCommentRangeEnd,
    WmlCommentRangeStart, WmlCommentReference,
};

/// Font declarations and embedded resource loading.
pub mod font_table;
/// Raw list definitions and concrete preparation.
pub mod numbering;
pub use document::dom::{IDomNumbering, NumberingPicBullet};
