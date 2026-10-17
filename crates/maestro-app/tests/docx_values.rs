//! Word scalar conversion behavior.
#![cfg(test)]
use crate::docx_support;
use docx_support::cases;
use maestro_app::browser::docx_preview::{LengthUsage, convert_boolean, convert_length};
use serde::Deserialize;
use serde_json::json;

/// Scalar arguments from the controlled corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Optional source spelling.
    value: Option<String>,
    /// Conversion policy.
    usage: Option<String>,
    /// Boolean fallback.
    fallback: Option<bool>,
}

/// Replay scalar conversions belonging to one behavior.
fn check(name: &str) {
    for case in cases::<Input>(include_str!("fixtures/docx/values.json"), name) {
        let observed = if let Some(usage) = case.input.usage {
            let usage = match usage.as_str() {
                "Dxa" => LengthUsage::Dxa,
                "Emu" => LengthUsage::Emu,
                "FontSize" => LengthUsage::FontSize,
                "Border" => LengthUsage::Border,
                "Point" => LengthUsage::Point,
                "Percent" => LengthUsage::Percent,
                _ => panic!("unknown conversion"),
            };
            json!(convert_length(case.input.value.as_deref(), usage))
        } else {
            json!(convert_boolean(
                case.input.value.as_deref(),
                case.input.fallback
            ))
        };
        assert_eq!(observed, case.expected);
    }
}

#[test]
fn length_values_keep_units_and_numeric_prefixes() {
    check("length_values_keep_units_and_numeric_prefixes");
}

#[test]
fn length_scales_and_border_clamps_are_distinct() {
    check("length_scales_and_border_clamps_are_distinct");
}

#[test]
fn boolean_tokens_preserve_the_selected_fallback() {
    check("boolean_tokens_preserve_the_selected_fallback");
}
