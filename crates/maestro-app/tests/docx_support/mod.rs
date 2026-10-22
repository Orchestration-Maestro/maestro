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
    /// Cell-width suppression for every property group.
    #[serde(default)]
    pub ignore_width: bool,
}
/// Parse the source and route its root, or a document's body, through the body parsers.
///
/// # Panics
/// Panics if the source is not XML or the operation names no body route.
pub fn framed(input: &XmlInput) -> Result<OpenXmlElementTree, DocxError> {
    let xml = parse_xml_string(&input.xml, true).unwrap();
    let element = OwnedXmlElement::new(xml.root());
    let ignore_width = input.ignore_width;
    match input.operation.as_str() {
        "body" => parse_body_elements(&element, ignore_width),
        "document" => parse_body_elements(
            &OwnedXmlElement::new(xml.element(xml.root(), "body").unwrap()),
            ignore_width,
        ),
        "paragraph" => parse_paragraph(&element, ignore_width),
        "run" => parse_run(&element, ignore_width),
        operation => panic!("unknown operation {operation}"),
    }
}
