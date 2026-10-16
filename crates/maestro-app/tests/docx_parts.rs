//! Recorded Word part preparation through public factories.
#![cfg(test)]
#[path = "docx_support/body_tree.rs"]
pub mod body_tree;
#[path = "docx_support/parts.rs"]
mod parts;
use maestro_app::browser::docx_preview::{
    DocxError, EndnotesPart, FooterPart, FootnotesPart, HeaderPart, OwnedXmlElement, SettingsPart,
    StylesPart, ThemePart, parse_body_elements, parse_xml_string,
};
use serde::Deserialize;
use serde_json::Value;

/// Recorded operation arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Public factory route.
    operation: String,
    /// Admitted XML source.
    xml: String,
    /// Cell width policy supplied to the factory.
    #[serde(default)]
    ignore_width: bool,
}
/// Independently recorded success or XML-part failure.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Owning test name.
    test: String,
    /// Complete factory arguments.
    input: Input,
    /// Success or error classification.
    outcome: String,
    /// Complete recorded observation.
    expected: Value,
}
/// Prepare one owned observation after releasing the XML input.
fn observe(input: Input) -> Result<Value, DocxError> {
    if !matches!(
        input.operation.as_str(),
        "styles" | "styles_part" | "theme" | "theme_part" | "settings" | "settings_part"
    ) {
        return observe_body(input);
    }
    let xml = parse_xml_string(&input.xml, false)?;
    match input.operation.as_str() {
        "styles" | "styles_part" => {
            let part = StylesPart::parse_xml(&xml, xml.root(), input.ignore_width)?;
            drop(xml);
            drop(input);
            Ok(parts::styles(part.styles))
        }
        "theme" | "theme_part" => {
            let part = ThemePart::parse_xml(&xml, xml.root())?;
            drop(xml);
            drop(input);
            Ok(parts::theme(part.theme))
        }
        "settings" | "settings_part" => {
            let part = SettingsPart::parse_xml(&xml, xml.root());
            drop(xml);
            drop(input);
            Ok(parts::settings(part.settings))
        }
        _ => unreachable!(),
    }
}
/// Release XML owners before observing typed body-part results.
fn observe_body(input: Input) -> Result<Value, DocxError> {
    let owned = {
        let xml = parse_xml_string(&input.xml, false)?;
        OwnedXmlElement::new(xml.root())
    };
    let operation = input.operation;
    drop(input.xml);
    match operation.as_str() {
        "header" => {
            let part = HeaderPart::parse_xml(&owned)?;
            drop(owned);
            Ok(parts::header(part.root_element))
        }
        "footer" => {
            let part = FooterPart::parse_xml(&owned)?;
            drop(owned);
            Ok(parts::footer(part.root_element))
        }
        "footnotes" => {
            let part = FootnotesPart::parse_xml(&owned)?;
            drop(owned);
            Ok(serde_json::json!(
                part.notes
                    .into_iter()
                    .map(|n| parts::note(&n.base, "footnote"))
                    .collect::<Vec<_>>()
            ))
        }
        "endnotes" => {
            let part = EndnotesPart::parse_xml(&owned)?;
            drop(owned);
            Ok(serde_json::json!(
                part.notes
                    .into_iter()
                    .map(|n| parts::note(&n.base, "endnote"))
                    .collect::<Vec<_>>()
            ))
        }
        "body" => {
            let tree = parse_body_elements(&owned)?;
            drop(owned);
            Ok(body_tree::full_graph(&tree))
        }
        _ => panic!("unknown operation"),
    }
}
/// Replay every unique input owned by this test.
fn check(name: &str) {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/docx/parts.json")).unwrap();
    let mut seen = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        let actual = observe(case.input);
        match case.outcome.as_str() {
            "success" => assert_eq!(actual.unwrap(), case.expected, "case {seen}"),
            "error" => {
                assert!(matches!(actual, Err(DocxError::Xml { .. })), "case {seen}");
                assert_eq!(
                    case.expected,
                    serde_json::json!({"kind":"invalid-xml-part"})
                );
            }
            _ => panic!("unknown outcome"),
        }
        seen += 1;
    }
    assert!(seen > 0);
}

#[test]
fn styles_keep_order_defaults_and_repeated_ids() {
    check("styles_keep_order_defaults_and_repeated_ids");
}

#[test]
fn style_targets_and_default_flags_keep_all_states() {
    check("style_targets_and_default_flags_keep_all_states");
}

#[test]
fn style_metadata_keeps_open_ids_names_links_and_aliases() {
    check("style_metadata_keeps_open_ids_names_links_and_aliases");
}

#[test]
fn style_aliases_preserve_empty_pieces_and_fail_without_value() {
    check("style_aliases_preserve_empty_pieces_and_fail_without_value");
}

#[test]
fn default_styles_select_first_nested_property_in_source_order() {
    check("default_styles_select_first_nested_property_in_source_order");
}

#[test]
fn styles_dispatch_all_property_groups_and_conditional_regions() {
    check("styles_dispatch_all_property_groups_and_conditional_regions");
}

#[test]
fn style_properties_keep_separate_css_and_metadata_views() {
    check("style_properties_keep_separate_css_and_metadata_views");
}

#[test]
fn repeated_style_properties_replace_metadata_not_css_history() {
    check("repeated_style_properties_replace_metadata_not_css_history");
}

#[test]
fn style_width_policy_reaches_generic_and_conditional_paths() {
    check("style_width_policy_reaches_generic_and_conditional_paths");
}

#[test]
fn styles_use_immediate_local_names_and_first_attributes() {
    check("styles_use_immediate_local_names_and_first_attributes");
}

#[test]
fn styles_ignore_nonrendering_metadata_without_diagnostics() {
    check("styles_ignore_nonrendering_metadata_without_diagnostics");
}

#[test]
fn part_results_outlive_package_and_xml_inputs() {
    check("part_results_outlive_package_and_xml_inputs");
}

#[test]
fn theme_absence_and_replacement_stay_distinct() {
    check("theme_absence_and_replacement_stay_distinct");
}

#[test]
fn theme_colors_select_srgb_before_system_fallback() {
    check("theme_colors_select_srgb_before_system_fallback");
}

#[test]
fn theme_color_order_and_last_values_are_preserved() {
    check("theme_color_order_and_last_values_are_preserved");
}

#[test]
fn theme_font_groups_keep_independent_typefaces() {
    check("theme_font_groups_keep_independent_typefaces");
}

#[test]
fn theme_font_missing_elements_differ_from_missing_attributes() {
    check("theme_font_missing_elements_differ_from_missing_attributes");
}

#[test]
fn theme_part_matches_direct_preparation() {
    check("theme_part_matches_direct_preparation");
}

#[test]
fn header_contents_reuse_owned_body_parsing() {
    check("header_contents_reuse_owned_body_parsing");
}

#[test]
fn footer_contents_reuse_owned_body_parsing() {
    check("footer_contents_reuse_owned_body_parsing");
}

#[test]
fn header_footer_keep_shared_source_after_input_drop() {
    check("header_footer_keep_shared_source_after_input_drop");
}

#[test]
fn notes_preserve_kind_ids_types_order_and_duplicates() {
    check("notes_preserve_kind_ids_types_order_and_duplicates");
}

#[test]
fn note_children_select_only_the_requested_immediate_kind() {
    check("note_children_select_only_the_requested_immediate_kind");
}

#[test]
fn footnotes_reuse_body_metadata_and_parent_identity() {
    check("footnotes_reuse_body_metadata_and_parent_identity");
}

#[test]
fn endnotes_reuse_body_metadata_and_parent_identity() {
    check("endnotes_reuse_body_metadata_and_parent_identity");
}

#[test]
fn settings_absent_empty_and_last_groups_stay_distinct() {
    check("settings_absent_empty_and_last_groups_stay_distinct");
}

#[test]
fn settings_tab_lengths_share_numeric_and_whitespace_rules() {
    check("settings_tab_lengths_share_numeric_and_whitespace_rules");
}

#[test]
fn settings_hyphenation_uses_exact_nullable_boolean_tokens() {
    check("settings_hyphenation_uses_exact_nullable_boolean_tokens");
}

#[test]
fn note_settings_keep_format_and_nullable_default_ids() {
    check("note_settings_keep_format_and_nullable_default_ids");
}

#[test]
fn settings_use_immediate_local_names_without_mixing_notes() {
    check("settings_use_immediate_local_names_without_mixing_notes");
}

#[test]
fn settings_part_matches_direct_preparation() {
    check("settings_part_matches_direct_preparation");
}

#[test]
fn borrowed_framing_matches_existing_public_body_routes() {
    check("borrowed_framing_matches_existing_public_body_routes");
}

/// Note parts propagate structural picture failures from the shared parser.
#[test]
fn notes_propagate_malformed_picture_errors() {
    for (operation, tag) in [("footnotes", "footnote"), ("endnotes", "endnote")] {
        let input = Input {
            operation: operation.into(),
            xml: format!(
                "<notes><{tag}><p><r><drawing><inline><graphic/></inline></drawing></r></p></{tag}></notes>"
            ),
            ignore_width: false,
        };
        assert!(matches!(observe_body(input), Err(DocxError::Xml { .. })));
    }
}
