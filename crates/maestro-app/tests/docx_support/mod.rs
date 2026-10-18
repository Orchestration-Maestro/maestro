//! Shared controlled Word-format observations.
#![cfg(test)]
#[path = "../../src/browser/docx_preview/tests/fixture_cases.rs"]
mod fixture_cases;
pub use fixture_cases::cases;
use maestro_app::browser::docx_preview::{
    DocxError, OpenXmlElementTree, OwnedXmlElement, parse_body_elements, parse_paragraph,
    parse_run, parse_xml_string,
};
use serde::Deserialize;

/// Recorded operation and admitted XML.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XmlInput {
    /// Public route, independent of behavior labels.
    pub operation: String,
    /// Complete XML source.
    pub xml: String,
}
/// Parse the source and route its root through the body, paragraph or run parser.
///
/// # Panics
/// Panics if the source is not XML or the operation names no body route.
pub fn framed(input: &XmlInput) -> Result<OpenXmlElementTree, DocxError> {
    let xml = parse_xml_string(&input.xml, true).unwrap();
    let element = OwnedXmlElement::new(xml.root());
    match input.operation.as_str() {
        "body" => parse_body_elements(&element),
        "paragraph" => parse_paragraph(&element),
        "run" => parse_run(&element),
        operation => panic!("unknown operation {operation}"),
    }
}
