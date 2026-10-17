//! Controlled numbering and embedded-font observations.
#![cfg(test)]
use crate::docx_support::cases;
use maestro_app::browser::docx_preview::{
    font_table::{EmbedFontType, FontTablePart},
    parse_xml_string,
};
use serde::Deserialize;
use serde_json::json;

/// Strict operation-specific arguments from the controlled corpus.
#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Input {
    /// Font declaration input.
    #[serde(rename = "fonts")]
    Fonts { xml: String },
    /// Raw and prepared numbering input.
    #[serde(rename = "numbering")]
    Numbering {
        xml: String,
        #[serde(rename = "ignoreWidth", default)]
        ignore_width: bool,
    },
    /// Opaque bytes through the public resource loader.
    #[serde(rename = "bytes")]
    Bytes { data: Vec<u8>, key: String },
    /// Controlled retained package input.
    #[serde(rename = "resource")]
    Resource {
        resource: String,
        key: Option<String>,
        #[serde(default)]
        reload: bool,
    },
}
/// Observe a font table after its borrowed parser has been dropped.
fn fonts(source: String) -> serde_json::Value {
    let part = {
        let xml = parse_xml_string(&source, false).unwrap();
        FontTablePart::parse_xml(&xml, xml.root())
    };
    drop(source);
    json!(part.fonts.iter().map(|f| {
        let mut out = json!({"name":f.name,"embedFontRefs":f.embed_font_refs.iter().map(|r| json!({"id":r.id,"key":r.key,"type":match r.r#type {EmbedFontType::Regular=>"regular",EmbedFontType::Bold=>"bold",EmbedFontType::Italic=>"italic",EmbedFontType::BoldItalic=>"boldItalic"}})).collect::<Vec<_>>()});
        if let Some(v)= &f.family { out["family"]=json!(v); }
        if let Some(v)= &f.alt_name { out["altName"]=json!(v); }
        out
    }).collect::<Vec<_>>())
}
/// Replay the complete selected observation after dropping source owners.
fn check(name: &str) {
    for case in cases::<Input>(include_str!("fixtures/docx/numbering-fonts.json"), name) {
        let observed = match case.input {
            Input::Fonts { xml } => fonts(xml),
            Input::Numbering {
                xml: source,
                ignore_width,
            } => {
                let part = {
                    let xml = parse_xml_string(&source, false).unwrap();
                    maestro_app::browser::docx_preview::numbering::NumberingPart::parse_xml(
                        &xml,
                        xml.root(),
                        ignore_width,
                    )
                };
                drop(source);
                crate::numbering_observation::observe(&part)
            }
            Input::Bytes { data, key } => {
                crate::font_resources::observe("present", Some(&data), Some(&key), false)
            }
            Input::Resource {
                resource,
                key,
                reload,
            } => crate::font_resources::observe(&resource, None, key.as_deref(), reload),
        };
        assert_eq!(observed, case.expected);
    }
}
#[test]
fn font_tables_keep_order_duplicates_and_unknown_children() {
    check("font_tables_keep_order_duplicates_and_unknown_children");
}

#[test]
fn font_metadata_keeps_null_empty_and_last_writes() {
    check("font_metadata_keeps_null_empty_and_last_writes");
}

#[test]
fn embedded_font_variants_keep_order_and_nullable_fields() {
    check("embedded_font_variants_keep_order_and_nullable_fields");
}

#[test]
fn font_selection_uses_immediate_local_names_and_first_attributes() {
    check("font_selection_uses_immediate_local_names_and_first_attributes");
}

#[test]
fn numbering_arrays_preserve_order_duplicates_and_null_pictures() {
    check("numbering_arrays_preserve_order_duplicates_and_null_pictures");
}

#[test]
fn numbering_identifiers_and_abstract_metadata_keep_text() {
    check("numbering_identifiers_and_abstract_metadata_keep_text");
}

#[test]
fn level_fields_keep_strings_and_numeric_prefix_domains() {
    check("level_fields_keep_strings_and_numeric_prefix_domains");
}

#[test]
fn override_records_keep_order_nullable_start_and_nested_levels() {
    check("override_records_keep_order_nullable_start_and_nested_levels");
}

#[test]
fn picture_records_follow_first_immediate_shape_chain() {
    check("picture_records_follow_first_immediate_shape_chain");
}

#[test]
fn numbering_selection_keeps_local_names_and_qualified_properties() {
    check("numbering_selection_keeps_local_names_and_qualified_properties");
}

#[test]
fn level_properties_reuse_raw_paragraph_and_common_run_readers() {
    check("level_properties_reuse_raw_paragraph_and_common_run_readers");
}

#[test]
fn property_group_replacement_does_not_clear_accumulated_dom_styles() {
    check("property_group_replacement_does_not_clear_accumulated_dom_styles");
}

#[test]
fn dom_levels_keep_defaults_suffixes_and_distinct_style_maps() {
    check("dom_levels_keep_defaults_suffixes_and_distinct_style_maps");
}

#[test]
fn numbering_width_policy_reaches_both_style_maps() {
    check("numbering_width_policy_reaches_both_style_maps");
}

#[test]
fn dom_style_writes_keep_order_and_replace_values_in_place() {
    check("dom_style_writes_keep_order_and_replace_values_in_place");
}

#[test]
fn unreferenced_definitions_and_missing_targets_do_not_invent_instances() {
    check("unreferenced_definitions_and_missing_targets_do_not_invent_instances");
}

#[test]
fn concrete_instances_share_definition_without_losing_ids() {
    check("concrete_instances_share_definition_without_losing_ids");
}

#[test]
fn instance_order_preserves_abstract_then_concrete_then_level_order() {
    check("instance_order_preserves_abstract_then_concrete_then_level_order");
}

#[test]
fn picture_links_resolve_after_their_abstract_definition() {
    check("picture_links_resolve_after_their_abstract_definition");
}

#[test]
fn picture_selection_keeps_first_match_and_missing_neighbors() {
    check("picture_selection_keeps_first_match_and_missing_neighbors");
}

#[test]
fn start_overrides_change_only_the_selected_instance_level() {
    check("start_overrides_change_only_the_selected_instance_level");
}

#[test]
fn level_overrides_replace_the_level_before_start_override() {
    check("level_overrides_replace_the_level_before_start_override");
}

#[test]
fn later_overrides_win_without_touching_other_levels() {
    check("later_overrides_win_without_touching_other_levels");
}

#[test]
fn override_only_levels_are_retained_for_the_concrete_instance() {
    check("override_only_levels_are_retained_for_the_concrete_instance");
}

#[test]
fn font_bytes_xor_only_the_first_two_key_blocks() {
    check("font_bytes_xor_only_the_first_two_key_blocks");
}

#[test]
fn font_keys_keep_scrubbing_reverse_byte_order_and_partial_hex() {
    check("font_keys_keep_scrubbing_reverse_byte_order_and_partial_hex");
}

#[test]
fn font_resource_reads_preserve_missing_empty_and_key_failure_phases() {
    check("font_resource_reads_preserve_missing_empty_and_key_failure_phases");
}

#[test]
fn font_resource_reloads_keep_archive_payload_unchanged() {
    check("font_resource_reloads_keep_archive_payload_unchanged");
}
