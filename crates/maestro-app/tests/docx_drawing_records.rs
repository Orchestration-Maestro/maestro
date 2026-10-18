//! Complete owned drawing records through the public Word parser.
#![cfg(test)]
use crate::body_tree;
use crate::docx_support;
use docx_support::{XmlInput, cases};
use maestro_app::browser::docx_preview::DocxError;
use serde_json::json;

/// Replay complete independent graph expectations after releasing input owners.
fn check(name: &str) {
    for (index, case) in cases::<XmlInput>(include_str!("fixtures/docx/drawing-records.json"), name)
        .into_iter()
        .enumerate()
    {
        let tree = docx_support::framed(&case.input);
        if case.expected.get("error_kind").is_some() {
            assert!(matches!(tree, Err(DocxError::Xml { .. })), "case {index}");
            assert_eq!(case.expected, json!({"error_kind":"Xml"}));
        } else {
            assert_eq!(
                body_tree::full_graph(&tree.unwrap()),
                case.expected,
                "case {index}"
            );
        }
    }
}
#[test]
fn drawing_wrappers_select_first_supported_child() {
    check("drawing_wrappers_select_first_supported_child");
}

#[test]
fn drawing_extent_preserves_units_and_last_assignment() {
    check("drawing_extent_preserves_units_and_last_assignment");
}

#[test]
fn drawing_anchor_float_uses_horizontal_alignment() {
    check("drawing_anchor_float_uses_horizontal_alignment");
}

#[test]
fn drawing_position_defaults_and_simple_position_select_one_route() {
    check("drawing_position_defaults_and_simple_position_select_one_route");
}

#[test]
fn drawing_top_bottom_wrap_keeps_alignment_and_full_width() {
    check("drawing_top_bottom_wrap_keeps_alignment_and_full_width");
}

#[test]
fn drawing_no_wrap_collapses_extent_and_keeps_offsets() {
    check("drawing_no_wrap_collapses_extent_and_keeps_offsets");
}

#[test]
fn drawing_wrap_selection_is_last_recognized_child() {
    check("drawing_wrap_selection_is_last_recognized_child");
}

#[test]
fn drawing_graphics_keep_all_supported_picture_children() {
    check("drawing_graphics_keep_all_supported_picture_children");
}

#[test]
fn picture_relationship_and_transform_keep_owned_fields() {
    check("picture_relationship_and_transform_keep_owned_fields");
}

#[test]
fn picture_crop_axes_keep_prefixes_defaults_and_order() {
    check("picture_crop_axes_keep_prefixes_defaults_and_order");
}

#[test]
fn picture_rotation_uses_degree_conversion() {
    check("picture_rotation_uses_degree_conversion");
}

#[test]
fn picture_geometry_keeps_last_direct_extent_and_offset() {
    check("picture_geometry_keeps_last_direct_extent_and_offset");
}

#[test]
fn incomplete_picture_structure_returns_xml_failure() {
    check("incomplete_picture_structure_returns_xml_failure");
}

#[test]
fn vml_shapes_map_supported_tags_and_default_attributes() {
    check("vml_shapes_map_supported_tags_and_default_attributes");
}

#[test]
fn vml_style_and_fill_keep_decoded_text() {
    check("vml_style_and_fill_keep_decoded_text");
}

#[test]
fn vml_line_points_split_without_conversion() {
    check("vml_line_points_split_without_conversion");
}

#[test]
fn vml_stroke_defaults_and_last_child_wins() {
    check("vml_stroke_defaults_and_last_child_wins");
}

#[test]
fn vml_fill_children_do_not_erase_shape_fill() {
    check("vml_fill_children_do_not_erase_shape_fill");
}

#[test]
fn vml_images_override_tag_and_keep_last_relationship() {
    check("vml_images_override_tag_and_keep_last_relationship");
}

#[test]
fn vml_children_and_textboxes_reuse_body_tree() {
    check("vml_children_and_textboxes_reuse_body_tree");
}

#[test]
fn vml_picture_ignores_unsupported_wrappers_and_nested_unknowns() {
    check("vml_picture_ignores_unsupported_wrappers_and_nested_unknowns");
}

#[test]
fn vml_attribute_iteration_and_queries_keep_distinct_precedence() {
    check("vml_attribute_iteration_and_queries_keep_distinct_precedence");
}

#[test]
fn bookmark_markers_keep_strings_and_nullable_columns() {
    check("bookmark_markers_keep_strings_and_nullable_columns");
}

#[test]
fn bookmark_columns_share_numeric_prefix_domains() {
    check("bookmark_columns_share_numeric_prefix_domains");
}

#[test]
fn bookmarks_keep_duplicate_ids_and_source_order() {
    check("bookmarks_keep_duplicate_ids_and_source_order");
}

#[test]
fn bookmarks_parse_through_hidden_paragraph_routes() {
    check("bookmarks_parse_through_hidden_paragraph_routes");
}

#[test]
fn math_tags_build_each_nested_kind() {
    check("math_tags_build_each_nested_kind");
}

#[test]
fn math_properties_keep_absence_null_and_empty_groups() {
    check("math_properties_keep_absence_null_and_empty_groups");
}

#[test]
fn math_property_tokens_use_shared_booleans() {
    check("math_property_tokens_use_shared_booleans");
}

#[test]
fn math_property_groups_replace_records_and_duplicate_fields() {
    check("math_property_groups_replace_records_and_duplicate_fields");
}

#[test]
fn math_runs_keep_text_formatting_and_field_markers() {
    check("math_runs_keep_text_formatting_and_field_markers");
}

#[test]
fn math_unknown_children_do_not_flatten_content() {
    check("math_unknown_children_do_not_flatten_content");
}

#[test]
fn math_nested_runs_keep_detached_parent_identity() {
    check("math_nested_runs_keep_detached_parent_identity");
}

#[test]
fn math_and_vml_composition_keep_existing_property_owners() {
    check("math_and_vml_composition_keep_existing_property_owners");
}

#[test]
fn alternate_content_uses_first_fallback_not_choice() {
    check("alternate_content_uses_first_fallback_not_choice");
}

#[test]
fn alternate_content_uses_first_element_and_dispatches_properties() {
    check("alternate_content_uses_first_element_and_dispatches_properties");
}

#[test]
fn alternate_content_missing_or_empty_fallback_keeps_run() {
    check("alternate_content_missing_or_empty_fallback_keeps_run");
}

#[test]
fn drawing_does_not_discard_earlier_run_children() {
    check("drawing_does_not_discard_earlier_run_children");
}

#[test]
fn drawing_failures_propagate_through_all_parent_routes() {
    check("drawing_failures_propagate_through_all_parent_routes");
}

#[test]
fn drawing_vml_math_records_outlive_released_xml() {
    check("drawing_vml_math_records_outlive_released_xml");
}
