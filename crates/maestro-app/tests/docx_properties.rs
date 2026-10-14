//! Word background and border property observations.
#![cfg(test)]
use maestro_app::browser::docx_preview::{
    parse_background, parse_border_properties, parse_xml_string,
};
use serde::Deserialize;

/// One complete consumer observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Owning behavior test.
    test: String,
    /// Consumer arguments.
    input: Input,
    /// Complete ordered result.
    expected: Expected,
}
/// Property consumer arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Selected consumer.
    operation: String,
    /// Complete source XML.
    xml: String,
    /// Caller-owned initial styles.
    style: Option<Vec<(String, Option<String>)>>,
}
/// Ordered CSS observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Complete ordered styles.
    style: Vec<(String, Option<String>)>,
}
/// Replay every complete input assigned to a behavior.
fn check(name: &str, count: usize) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/docx/property_values.json")).unwrap();
    assert_eq!(cases.len(), 142);
    let inputs: std::collections::HashSet<_> = cases
        .iter()
        .map(|c| (&c.input.operation, &c.input.xml, &c.input.style))
        .collect();
    assert_eq!(inputs.len(), cases.len(), "complete inputs must be unique");
    let selected: Vec<_> = cases.into_iter().filter(|c| c.test == name).collect();
    assert_eq!(selected.len(), count);
    for (index, case) in selected.into_iter().enumerate() {
        let xml = parse_xml_string(&case.input.xml, false).unwrap();
        let style = match case.input.operation.as_str() {
            "borders" => {
                let mut style = case.input.style.unwrap().into_iter().collect();
                parse_border_properties(&xml, xml.root(), &mut style);
                style
            }
            "background" => {
                assert!(case.input.style.is_none());
                parse_background(&xml, xml.root())
            }
            _ => panic!("unknown operation"),
        };
        assert_eq!(
            style.into_iter().collect::<Vec<_>>(),
            case.expected.style,
            "case {index}"
        );
    }
}

#[test]
fn border_styles_cover_all_tokens() {
    check("border_styles_cover_all_tokens", 30);
}

#[test]
fn border_widths_keep_shared_conversion_boundaries() {
    check("border_widths_keep_shared_conversion_boundaries", 19);
}

#[test]
fn none_borders_ignore_missing_or_invalid_components() {
    check("none_borders_ignore_missing_or_invalid_components", 16);
}

#[test]
fn named_border_colors_use_document_rgb_values() {
    check("named_border_colors_use_document_rgb_values", 17);
}

#[test]
fn border_colors_keep_auto_and_hex_spelling() {
    check("border_colors_keep_auto_and_hex_spelling", 6);
}

#[test]
fn border_colors_keep_literal_and_theme_precedence() {
    check("border_colors_keep_literal_and_theme_precedence", 12);
}

#[test]
fn missing_border_components_use_css_defaults() {
    check("missing_border_components_use_css_defaults", 6);
}

#[test]
fn border_sides_select_immediate_children_only() {
    check("border_sides_select_immediate_children_only", 1);
}

#[test]
fn border_logical_aliases_replace_without_reordering() {
    check("border_logical_aliases_replace_without_reordering", 2);
}

#[test]
fn border_updates_preserve_existing_style_entries() {
    check("border_updates_preserve_existing_style_entries", 1);
}

#[test]
fn empty_border_groups_leave_styles_unchanged() {
    check("empty_border_groups_leave_styles_unchanged", 2);
}

#[test]
fn border_attributes_follow_local_name_and_first_attribute() {
    check("border_attributes_follow_local_name_and_first_attribute", 2);
}

#[test]
fn background_colors_share_the_color_owner() {
    check("background_colors_share_the_color_owner", 20);
}

#[test]
fn background_missing_color_keeps_empty_style() {
    check("background_missing_color_keeps_empty_style", 6);
}

#[test]
fn background_reads_only_its_selected_element() {
    check("background_reads_only_its_selected_element", 2);
}
