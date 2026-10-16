//! Core document metadata.
use super::super::document::common::integer_prefix;
use super::super::{XmlElement, XmlParser};
/// Selected core properties; absent fields have not been assigned.
#[derive(Debug, Default)]
pub struct CorePropsDeclaration {
    /// Raw title text.
    pub title: Option<String>,
    /// Raw description text.
    pub description: Option<String>,
    /// Raw subject text.
    pub subject: Option<String>,
    /// Raw creator text.
    pub creator: Option<String>,
    /// Raw keywords text.
    pub keywords: Option<String>,
    /// Raw language text.
    pub language: Option<String>,
    /// Raw last modified by text.
    pub last_modified_by: Option<String>,
    /// Raw revision integer prefix.
    pub revision: Option<f64>,
}
/// Collect immediate core properties in encounter order.
pub(super) fn parse_core_props<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    root: XmlElement<'doc, 'input>,
) -> CorePropsDeclaration {
    let mut props = CorePropsDeclaration::default();
    for child in xml.elements(root, None) {
        let text = child.text_content();
        match child.local_name() {
            "title" => props.title = Some(text),
            "description" => props.description = Some(text),
            "subject" => props.subject = Some(text),
            "creator" => props.creator = Some(text),
            "keywords" => props.keywords = Some(text),
            "language" => props.language = Some(text),
            "lastModifiedBy" => props.last_modified_by = Some(text),
            "revision" if !text.is_empty() => props.revision = Some(integer_prefix(&text, 10)),
            _ => {}
        }
    }
    props
}
