//! Document-part root: body content, the body's own section group and the background.
use super::super::document::section::parse_section_properties;
use super::super::{
    DocxError, OpenXmlElementTree, OwnedXmlElement, SectionProperties, parse_xml_string,
};
use super::{parse_background, parse_body_elements};
use indexmap::IndexMap;

/// Prepared document-part root: immediate body content with the body's own section group.
#[derive(Debug, Default)]
pub struct WordBody {
    /// Immediate body elements and their arena.
    pub children: OpenXmlElementTree,
    /// Section properties declared directly on the body; empty when the part declares none.
    pub props: SectionProperties,
    /// Background declarations of the document root; empty without a background.
    pub css_style: IndexMap<String, Option<String>>,
}

/// Parse a document part's body, its own section properties and its background.
///
/// `ignore_width` suppresses cell widths in every property group.
///
/// # Errors
/// Returns an XML error for invalid part content or a document without a body.
pub fn parse_document_file(
    input: &OwnedXmlElement,
    ignore_width: bool,
) -> Result<WordBody, DocxError> {
    let xml = parse_xml_string(input.source(), false)?;
    let root = xml.selected(input)?;
    let body = xml.element(root, "body").ok_or_else(|| DocxError::Xml {
        path: None,
        message: "Document part has no body element".into(),
    })?;
    Ok(WordBody {
        children: parse_body_elements(&OwnedXmlElement::new(body), ignore_width)?,
        props: xml
            .element(body, "sectPr")
            .map(|section| parse_section_properties(&xml, section))
            .unwrap_or_default(),
        css_style: xml
            .element(root, "background")
            .map(|background| parse_background(&xml, background))
            .unwrap_or_default(),
    })
}
