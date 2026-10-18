//! Owned section records through the existing paragraph and body operations.
#![cfg(test)]
use crate::body_tree;
use crate::docx_support;
use docx_support::{XmlInput, cases};

/// Compare complete owned results after releasing input owners.
fn check(name: &str) {
    for (index, case) in
        cases::<XmlInput>(include_str!("fixtures/docx/section-properties.json"), name)
            .into_iter()
            .enumerate()
    {
        let tree = docx_support::framed(&case.input).unwrap();
        assert_eq!(body_tree::full_graph(&tree), case.expected, "case {index}");
    }
}
#[test]
fn section_geometry_uses_each_shared_length() {
    check("section_geometry_uses_each_shared_length");
}

#[test]
fn section_groups_keep_absence_empty_and_last_records() {
    check("section_groups_keep_absence_empty_and_last_records");
}

#[test]
fn section_types_and_orientation_keep_open_text() {
    check("section_types_and_orientation_keep_open_text");
}

#[test]
fn section_columns_keep_number_and_nested_order() {
    check("section_columns_keep_number_and_nested_order");
}

#[test]
fn section_booleans_keep_independent_defaults() {
    check("section_booleans_keep_independent_defaults");
}

#[test]
fn section_header_footer_references_keep_kind_and_order() {
    check("section_header_footer_references_keep_kind_and_order");
}

#[test]
fn section_page_numbers_keep_strings_and_numeric_prefixes() {
    check("section_page_numbers_keep_strings_and_numeric_prefixes");
}

#[test]
fn section_borders_keep_raw_fields_and_units() {
    check("section_borders_keep_raw_fields_and_units");
}

#[test]
fn section_borders_select_physical_sides_and_last_values() {
    check("section_borders_select_physical_sides_and_last_values");
}

#[test]
fn section_selection_uses_immediate_local_names() {
    check("section_selection_uses_immediate_local_names");
}

#[test]
fn section_presence_and_group_replacement_keep_other_metadata() {
    check("section_presence_and_group_replacement_keep_other_metadata");
}

#[test]
fn section_border_widths_clamp_without_changing_offsets() {
    check("section_border_widths_clamp_without_changing_offsets");
}

#[test]
fn section_border_flags_are_independent_and_nullable() {
    check("section_border_flags_are_independent_and_nullable");
}

#[test]
fn section_columns_convert_each_length_without_cross_wiring() {
    check("section_columns_convert_each_length_without_cross_wiring");
}

#[test]
fn section_parent_selection_uses_the_shared_qualified_owner() {
    check("section_parent_selection_uses_the_shared_qualified_owner");
}

#[test]
fn section_body_routes_keep_sections_and_hidden_parent_identity() {
    check("section_body_routes_keep_sections_and_hidden_parent_identity");
}

/// `SectionProperties`, `PageSize`, `PageMargins`, `Columns`, `Column`,
/// `FooterHeaderReference`, `PageNumber`, `Borders` and `Border` survive input release.
#[test]
fn section_records_outlive_all_xml_input_owners() {
    check("section_records_outlive_all_xml_input_owners");
}
