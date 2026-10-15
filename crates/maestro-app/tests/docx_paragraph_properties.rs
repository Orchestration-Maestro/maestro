//! Owned paragraph and run properties through public parsing operations.
#![cfg(test)]
#[path = "docx_support/body_tree.rs"]
pub mod body_tree;
mod docx_support;
use docx_support::cases;
use maestro_app::browser::docx_preview::{
    OwnedXmlElement, parse_body_elements, parse_paragraph, parse_run, parse_xml_string,
};
use serde::Deserialize;

/// Recorded operation and admitted XML.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Public parsing route.
    operation: String,
    /// Complete XML source.
    xml: String,
}
/// Compare complete owned results after releasing input owners.
fn check(name: &str) {
    for (index, case) in cases::<Input>(
        include_str!("fixtures/docx/paragraph-properties.json"),
        name,
    )
    .into_iter()
    .enumerate()
    {
        let tree = {
            let xml = parse_xml_string(&case.input.xml, true).unwrap();
            let input = OwnedXmlElement::new(xml.root());
            match case.input.operation.as_str() {
                "paragraph" => parse_paragraph(&input),
                "run" => parse_run(&input),
                "body" => parse_body_elements(&input),
                _ => panic!("unknown operation"),
            }
            .unwrap()
        };
        assert_eq!(body_tree::full_graph(&tree), case.expected, "case {index}");
    }
}
#[test]
fn paragraph_common_records_keep_raw_color_and_font_size() {
    check("paragraph_common_records_keep_raw_color_and_font_size");
}

#[test]
fn paragraph_flags_keep_defaults_without_css() {
    check("paragraph_flags_keep_defaults_without_css");
}

#[test]
fn paragraph_numbering_keeps_id_and_level_domains() {
    check("paragraph_numbering_keeps_id_and_level_domains");
}

#[test]
fn paragraph_outline_levels_use_shared_numeric_prefixes() {
    check("paragraph_outline_levels_use_shared_numeric_prefixes");
}

#[test]
fn paragraph_tabs_keep_all_records_even_without_experimental_layout() {
    check("paragraph_tabs_keep_all_records_even_without_experimental_layout");
}

#[test]
fn paragraph_tab_positions_share_length_conversion() {
    check("paragraph_tab_positions_share_length_conversion");
}

#[test]
fn paragraph_spacing_updates_metadata_and_the_same_css() {
    check("paragraph_spacing_updates_metadata_and_the_same_css");
}

#[test]
fn paragraph_text_alignment_updates_both_views() {
    check("paragraph_text_alignment_updates_both_views");
}

#[test]
fn paragraph_and_run_style_names_keep_nullable_text() {
    check("paragraph_and_run_style_names_keep_nullable_text");
}

#[test]
fn paragraph_run_properties_keep_common_metadata_only() {
    check("paragraph_run_properties_keep_common_metadata_only");
}

#[test]
fn paragraph_frames_float_only_drop() {
    check("paragraph_frames_float_only_drop");
}

#[test]
fn paragraph_property_groups_reset_css_not_metadata() {
    check("paragraph_property_groups_reset_css_not_metadata");
}

#[test]
fn run_property_groups_reset_css_not_metadata() {
    check("run_property_groups_reset_css_not_metadata");
}

#[test]
fn run_vertical_alignment_keeps_present_null_and_tag_choices() {
    check("run_vertical_alignment_keeps_present_null_and_tag_choices");
}

#[test]
fn property_namespace_gates_metadata_not_generic_css() {
    check("property_namespace_gates_metadata_not_generic_css");
}

#[test]
fn property_selection_ignores_nested_unknown_nodes() {
    check("property_selection_ignores_nested_unknown_nodes");
}

#[test]
fn property_selection_uses_first_local_attribute() {
    check("property_selection_uses_first_local_attribute");
}

#[test]
fn conditional_bit_strings_keep_class_order() {
    check("conditional_bit_strings_keep_class_order");
}

#[test]
fn conditional_attribute_flags_use_the_shared_correction() {
    check("conditional_attribute_flags_use_the_shared_correction");
}

#[test]
fn conditional_bit_string_presence_overrides_flags() {
    check("conditional_bit_string_presence_overrides_flags");
}

#[test]
fn field_run_updates_preserve_previously_parsed_properties() {
    check("field_run_updates_preserve_previously_parsed_properties");
}

#[test]
fn body_and_hidden_paragraph_routes_keep_owned_properties() {
    check("body_and_hidden_paragraph_routes_keep_owned_properties");
}

#[test]
fn paragraph_css_reuses_the_existing_formatting_owner() {
    check("paragraph_css_reuses_the_existing_formatting_owner");
}

#[test]
fn run_css_reuses_the_existing_formatting_owner() {
    check("run_css_reuses_the_existing_formatting_owner");
}

#[test]
fn integrated_formatting_keeps_landed_root_corrections() {
    check("integrated_formatting_keeps_landed_root_corrections");
}
