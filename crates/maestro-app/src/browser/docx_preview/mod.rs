//! In-memory Word package reading and borrowed XML queries.
mod document;
pub use document::common::{LengthUsage, convert_boolean, convert_length};
mod parser;
pub(crate) use parser::xml_parser::decode_text;
pub use parser::xml_parser::{
    OwnedXmlElement, XmlAttribute, XmlElement, XmlParser, parse_xml_string,
};

/// Failure opening an archive or one of its XML parts.
#[derive(Debug)]
pub enum DocxError {
    /// Archive decoding or entry-read failure.
    Archive(String),
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
            Self::Archive(message) => write!(f, "{message}"),
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
mod word_document;
pub use common::{
    part::{Part, PartKind},
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
    UnparsedKind,
};
pub use document_parser::{
    parse_background, parse_body_elements, parse_border_properties, parse_paragraph, parse_run,
};
