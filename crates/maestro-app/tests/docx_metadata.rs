//! Typed Word metadata through public factories.
#![cfg(test)]
#[path = "docx_support/body_tree.rs"]
pub mod body_tree;
mod docx_support;
#[path = "docx_support/metadata.rs"]
pub mod metadata;
use docx_support::cases;
use maestro_app::browser::docx_preview::{
    CorePropsPart, CustomPropsPart, DocxError, ExtendedPropsPart, parse_xml_string,
};
use serde::Deserialize;
use serde_json::json;
/// Factory input from the controlled corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Selected metadata factory.
    operation: String,
    /// Admitted XML source.
    xml: String,
}
/// Dispatch by recorded operation and compare the complete public result.
fn check(name: &str) {
    for case in cases::<Input>(include_str!("fixtures/docx/metadata.json"), name) {
        let xml = parse_xml_string(&case.input.xml, true).unwrap();
        let observed = match case.input.operation.as_str() {
            "core" => metadata::core(&CorePropsPart::parse_xml(&xml, xml.root()).props),
            "extended" => metadata::extended(&ExtendedPropsPart::parse_xml(&xml, xml.root()).props),
            "custom" => match CustomPropsPart::parse_xml(&xml, xml.root()) {
                Ok(part) => metadata::custom(&part.props),
                Err(DocxError::Xml { .. }) => json!({"kind":"invalid-xml-part"}),
                Err(DocxError::Archive(_)) => panic!("unexpected archive error"),
            },
            _ => panic!("unexpected metadata operation"),
        };
        assert_eq!(observed, case.expected);
    }
}
#[test]
fn core_fields_preserve_text_and_absence() {
    check("core_fields_preserve_text_and_absence");
}
#[test]
fn core_revisions_keep_empty_updates_and_numeric_prefixes() {
    check("core_revisions_keep_empty_updates_and_numeric_prefixes");
}
#[test]
fn extended_fields_preserve_text_and_absence() {
    check("extended_fields_preserve_text_and_absence");
}
#[test]
fn metadata_selection_uses_immediate_local_names() {
    check("metadata_selection_uses_immediate_local_names");
}
#[test]
fn metadata_duplicates_replace_only_encountered_values() {
    check("metadata_duplicates_replace_only_encountered_values");
}
#[test]
fn extended_counters_keep_prefix_nan_and_negative_zero() {
    check("extended_counters_keep_prefix_nan_and_negative_zero");
}
#[test]
fn custom_values_keep_names_qualified_types_and_source_order() {
    check("custom_values_keep_names_qualified_types_and_source_order");
}
#[test]
fn custom_values_skip_nonvalue_nodes() {
    check("custom_values_skip_nonvalue_nodes");
}
#[test]
fn custom_missing_value_returns_xml_failure() {
    check("custom_missing_value_returns_xml_failure");
}
