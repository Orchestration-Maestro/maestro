//! Extended document metadata.
use super::super::document::common::integer_prefix;
use super::super::{XmlElement, XmlParser};
/// Selected application properties.
#[derive(Debug, Default)]
pub struct ExtendedPropsDeclaration {
    /// Raw template text.
    pub template: Option<String>,
    /// Raw pages integer prefix.
    pub pages: Option<f64>,
    /// Raw words integer prefix.
    pub words: Option<f64>,
    /// Raw characters integer prefix.
    pub characters: Option<f64>,
    /// Raw application text.
    pub application: Option<String>,
    /// Raw lines integer prefix.
    pub lines: Option<f64>,
    /// Raw paragraphs integer prefix.
    pub paragraphs: Option<f64>,
    /// Raw company text.
    pub company: Option<String>,
    /// Raw app version text.
    pub app_version: Option<String>,
}
/// Collect immediate extended properties in encounter order.
pub(super) fn parse_extended_props<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    root: XmlElement<'doc, 'input>,
) -> ExtendedPropsDeclaration {
    let mut props = ExtendedPropsDeclaration::default();
    for child in xml.elements(root, None) {
        let text = child.text_content();
        match child.local_name() {
            "Template" => props.template = Some(text),
            "Pages" => props.pages = Some(integer_prefix(&text, 10)),
            "Words" => props.words = Some(integer_prefix(&text, 10)),
            "Characters" => props.characters = Some(integer_prefix(&text, 10)),
            "Application" => props.application = Some(text),
            "Lines" => props.lines = Some(integer_prefix(&text, 10)),
            "Paragraphs" => props.paragraphs = Some(integer_prefix(&text, 10)),
            "Company" => props.company = Some(text),
            "AppVersion" => props.app_version = Some(text),
            _ => {}
        }
    }
    props
}
