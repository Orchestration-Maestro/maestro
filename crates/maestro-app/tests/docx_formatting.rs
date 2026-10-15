//! Ordered Word formatting observations through the consuming parsers.
#![cfg(test)]
use indexmap::IndexMap;
use maestro_app::browser::docx_preview::{
    DefaultPropertiesOptions, parse_default_properties, parse_table_style, parse_xml_string,
};
use serde::Deserialize;

/// Ordered nullable CSS entries.
type Entries = Vec<(String, Option<String>)>;
/// One complete controlled observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Owning behavior.
    test: String,
    /// Complete consumer arguments.
    input: Input,
    /// Complete output.
    expected: Expected,
}
/// Arguments selected by the consuming operation.
#[derive(Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
enum Input {
    /// Default-property dispatch.
    #[serde(rename = "default")]
    Default {
        /// Complete XML source.
        xml: String,
        /// Initial main map.
        style: Entries,
        /// Optional initial child map.
        #[serde(rename = "childStyle")]
        child_style: Option<Entries>,
        /// Cell-width suppression.
        #[serde(rename = "ignoreWidth")]
        ignore_width: bool,
        /// Synchronous interception actions.
        handler: Vec<Action>,
    },
    /// Conditional table-style preparation.
    #[serde(rename = "tableStyle")]
    TableStyle {
        /// Complete XML source.
        xml: String,
        /// Forwarded cell-width suppression.
        #[serde(rename = "ignoreWidth")]
        ignore_width: bool,
    },
}
/// A handler's selected action.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Action {
    /// Matching local name.
    name: String,
    /// Whether generic dispatch is skipped.
    consume: bool,
    /// Main-map writes before returning.
    write: Entries,
}
/// Main-map view immediately before interception.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Seen {
    /// Immediate child's local name.
    name: String,
    /// Complete ordered main map.
    style: Entries,
}
/// One owned selector record.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Style {
    /// Selected CSS target.
    target: String,
    /// Selected modifier.
    r#mod: String,
    /// Ordered nullable CSS values.
    values: Entries,
}
/// Operation-specific complete observations.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// In-place mutation and callback transcript.
    Default {
        /// Complete main map.
        style: Entries,
        /// Complete optional child map.
        #[serde(rename = "childStyle")]
        child_style: Option<Entries>,
        /// Ordered callback views.
        seen: Vec<Seen>,
    },
    /// Owned table-style results.
    TableStyle {
        /// Ordered records, including repeated and empty records.
        styles: Vec<Style>,
    },
}
/// Replay one operation, observing returned records after XML is dropped.
fn observe(input: Input) -> Expected {
    match input {
        Input::Default {
            xml,
            style,
            child_style,
            ignore_width,
            handler,
        } => observe_default(&xml, style, child_style, ignore_width, &handler),
        Input::TableStyle { xml, ignore_width } => {
            let records = {
                let xml = parse_xml_string(&xml, false).unwrap();
                parse_table_style(&xml, xml.root(), ignore_width)
            };
            Expected::TableStyle {
                styles: records
                    .into_iter()
                    .map(|s| Style {
                        target: s.target,
                        r#mod: s.r#mod,
                        values: s.values.into_iter().collect(),
                    })
                    .collect(),
            }
        }
    }
}
/// Replay default properties with the default configuration on the false route.
fn observe_default(
    xml: &str,
    style: Entries,
    child_style: Option<Entries>,
    ignore_width: bool,
    handler: &[Action],
) -> Expected {
    let xml = parse_xml_string(xml, false).unwrap();
    let mut style: IndexMap<_, _> = style.into_iter().collect();
    let mut child_style: Option<IndexMap<_, _>> = child_style.map(|s| s.into_iter().collect());
    let mut seen = Vec::new();
    let mut callback = |element: maestro_app::browser::docx_preview::XmlElement<'_, '_>,
                        style: &mut IndexMap<String, Option<String>>| {
        seen.push(Seen {
            name: element.local_name().into(),
            style: style.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        });
        if let Some(action) = handler.iter().find(|a| a.name == element.local_name()) {
            style.extend(action.write.iter().cloned());
            action.consume
        } else {
            false
        }
    };
    let mut options = DefaultPropertiesOptions::default();
    if ignore_width {
        options.ignore_width = true;
    }
    if !handler.is_empty() {
        options.handler = Some(&mut callback);
    }
    parse_default_properties(&xml, xml.root(), &mut style, child_style.as_mut(), options);
    Expected::Default {
        style: style.into_iter().collect(),
        child_style: child_style.map(|s| s.into_iter().collect()),
        seen,
    }
}
/// Check the envelope, complete-input uniqueness and all rows of one behavior.
fn check(name: &str, count: usize) {
    let source = include_str!("fixtures/docx/formatting.json");
    let raw: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    let inputs: std::collections::HashSet<_> = raw.iter().map(|c| c["input"].to_string()).collect();
    assert_eq!(inputs.len(), raw.len(), "complete inputs must be unique");
    let cases: Vec<Case> = serde_json::from_str(source).unwrap();
    assert_eq!(cases.len(), 412);
    let selected: Vec<_> = cases.into_iter().filter(|c| c.test == name).collect();
    assert_eq!(selected.len(), count);
    for (index, case) in selected.into_iter().enumerate() {
        assert_eq!(observe(case.input), case.expected, "case {index}");
    }
}

#[test]
fn default_alignment_keeps_aliases_and_open_values() {
    check("default_alignment_keeps_aliases_and_open_values", 28);
}

#[test]
fn default_colors_keep_each_auto_policy() {
    check("default_colors_keep_each_auto_policy", 30);
}

#[test]
fn default_colors_keep_absent_values() {
    check("default_colors_keep_absent_values", 6);
}

#[test]
fn default_font_sizes_keep_null_units_and_order() {
    check("default_font_sizes_keep_null_units_and_order", 13);
}

#[test]
fn default_width_policy_selects_only_cell_width() {
    check("default_width_policy_selects_only_cell_width", 4);
}

#[test]
fn default_widths_keep_type_and_missing_branches() {
    check("default_widths_keep_type_and_missing_branches", 36);
}

#[test]
fn default_boolean_styles_keep_literal_fallbacks() {
    check("default_boolean_styles_keep_literal_fallbacks", 60);
}

#[test]
fn default_visibility_and_direction_leave_false_seeds() {
    check("default_visibility_and_direction_leave_false_seeds", 20);
}

#[test]
fn default_language_keeps_optional_text() {
    check("default_language_keeps_optional_text", 6);
}

#[test]
fn default_ignored_children_do_not_recurse() {
    check("default_ignored_children_do_not_recurse", 1);
}

#[test]
fn default_handler_observes_and_mutates_real_style() {
    check("default_handler_observes_and_mutates_real_style", 2);
}

#[test]
fn default_child_styles_route_only_table_borders_and_margins() {
    check(
        "default_child_styles_route_only_table_borders_and_margins",
        3,
    );
}

#[test]
fn default_local_borders_do_not_target_child_style() {
    check("default_local_borders_do_not_target_child_style", 2);
}

#[test]
fn default_border_shorthand_reuses_missing_component_fix() {
    check("default_border_shorthand_reuses_missing_component_fix", 3);
}

#[test]
fn default_cell_spacing_keeps_width_and_separate_layout() {
    check("default_cell_spacing_keeps_width_and_separate_layout", 11);
}

#[test]
fn default_table_layout_defaults_unless_exact_fixed() {
    check("default_table_layout_defaults_unless_exact_fixed", 7);
}

#[test]
fn underlines_keep_every_style_and_missing_early_return() {
    check("underlines_keep_every_style_and_missing_early_return", 21);
}

#[test]
fn underline_colors_share_the_existing_color_owner() {
    check("underline_colors_share_the_existing_color_owner", 7);
}

#[test]
fn font_families_keep_selected_fields_order_and_deduplication() {
    check(
        "font_families_keep_selected_fields_order_and_deduplication",
        13,
    );
}

#[test]
fn font_families_reuse_shared_invalid_name_serialization() {
    check("font_families_reuse_shared_invalid_name_serialization", 2);
}

#[test]
fn indentation_keeps_hanging_and_logical_side_precedence() {
    check("indentation_keeps_hanging_and_logical_side_precedence", 11);
}

#[test]
fn table_indentation_uses_the_same_attribute_selection() {
    check("table_indentation_uses_the_same_attribute_selection", 1);
}

#[test]
fn spacing_runs_only_in_paragraph_properties() {
    check("spacing_runs_only_in_paragraph_properties", 3);
}

#[test]
fn paragraph_spacing_keeps_all_line_rules_and_number_spellings() {
    check(
        "paragraph_spacing_keeps_all_line_rules_and_number_spellings",
        72,
    );
}

#[test]
fn paragraph_spacing_keeps_order_with_font_size() {
    check("paragraph_spacing_keeps_order_with_font_size", 2);
}

#[test]
fn cell_margins_use_only_immediate_physical_sides() {
    check("cell_margins_use_only_immediate_physical_sides", 2);
}

#[test]
fn row_heights_preserve_rule_outputs_and_nulls() {
    check("row_heights_preserve_rule_outputs_and_nulls", 24);
}

#[test]
fn position_uses_a_valid_css_key() {
    check("position_uses_a_valid_css_key", 1);
}

#[test]
fn position_key_shares_the_vertical_alignment_writer() {
    check("position_key_shares_the_vertical_alignment_writer", 3);
}

#[test]
fn default_property_replacement_keeps_first_key_slots() {
    check("default_property_replacement_keeps_first_key_slots", 1);
}

#[test]
fn table_style_regions_keep_all_selectors_and_targets() {
    check("table_style_regions_keep_all_selectors_and_targets", 8);
}

#[test]
fn table_style_unsupported_regions_are_empty() {
    check("table_style_unsupported_regions_are_empty", 7);
}

#[test]
fn table_style_records_keep_child_order_and_owned_values() {
    check("table_style_records_keep_child_order_and_owned_values", 1);
}

#[test]
fn default_dispatch_uses_local_names_and_first_attributes() {
    check("default_dispatch_uses_local_names_and_first_attributes", 1);
}
