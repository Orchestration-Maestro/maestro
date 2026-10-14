//! Strict edit-render oracle fixtures and palette construction.
#![cfg(test)]
#![allow(dead_code)] // consumers exercise different corpus groups

use maestro_theme::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeOptions};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// One independent rendering query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Owning witness.
    pub test: String,
    /// Consumer operation.
    pub op: String,
    /// Operation inputs.
    pub input: Value,
    /// Source-derived expected output.
    pub expected: Value,
}
/// The complete corpus and its palette.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Foreground role indices.
    colors: BTreeMap<String, Value>,
    /// Background role indices.
    backgrounds: BTreeMap<String, Value>,
    /// Distinct queries.
    cases: Vec<Case>,
}
/// All admitted fixture groups.
const GROUPS: [&str; 10] = [
    "edit_preview_selects_complete_inputs",
    "edit_title_uses_display_alias_precedence",
    "edit_header_selects_preview_before_settled_error",
    "edit_result_errors_keep_text_order_and_suppress_duplicates",
    "edit_result_success_only_emits_a_distinct_nonempty_diff",
    "edit_preview_changes_compare_variant_text_and_first_line",
    "edit_box_layout_keeps_padding_and_preview_spacing",
    "edit_box_layout_wraps_through_existing_widgets",
    "edit_result_without_call_renders_detached_output",
    "edit_theme_replacement_updates_background_and_rebuilt_text",
];
/// Load strict records, verifying inventory and uniqueness before selecting a group.
pub fn cases(test: &str) -> Vec<Case> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/edit-render-cases.json")).unwrap();
    assert_eq!(
        corpus
            .cases
            .iter()
            .map(|case| case.test.as_str())
            .collect::<BTreeSet<_>>(),
        GROUPS.into_iter().collect()
    );
    let mut seen = HashSet::new();
    for case in &corpus.cases {
        assert!(
            seen.insert((case.op.clone(), serde_json::to_string(&case.input).unwrap())),
            "repeated query"
        );
    }
    let selected: Vec<_> = corpus
        .cases
        .into_iter()
        .filter(|case| case.test == test)
        .collect();
    assert!(!selected.is_empty());
    selected
}
/// Convert a palette member through the theme's accepted native value forms.
fn color(value: &Value) -> ColorValue {
    if let Some(index) = value.as_u64() {
        ColorValue::Index(u8::try_from(index).unwrap())
    } else {
        ColorValue::String(value.as_str().unwrap().to_owned())
    }
}
/// Build a fixture theme from every foreground and background member.
pub fn theme(colors: &Value, backgrounds: &Value) -> Theme {
    Theme::new(
        colors
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (ThemeColor::Named(key.clone()), color(value))),
        backgrounds
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (ThemeBg::Named(key.clone()), color(value))),
        ColorMode::Truecolor,
        ThemeOptions::default(),
    )
    .unwrap()
}
/// The corpus palette, with an optional distinguishable index shift.
pub fn palette(shift: u64) -> Theme {
    palette_except(shift, None)
}
/// The corpus palette without one selected role.
pub fn without(role: &str) -> Theme {
    palette_except(0, Some(role))
}
/// Build the same palette while omitting only the requested role.
fn palette_except(shift: u64, omit: Option<&str>) -> Theme {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/edit-render-cases.json")).unwrap();
    let shifted = |map: BTreeMap<String, Value>| -> Value {
        Value::Object(
            map.into_iter()
                .filter(|(key, _)| omit != Some(key.as_str()))
                .map(|(key, value)| (key, Value::from(value.as_u64().unwrap() + shift)))
                .collect(),
        )
    };
    theme(&shifted(corpus.colors), &shifted(corpus.backgrounds))
}
/// A generated numbered preview's fixture representation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Details {
    /// Numbered diff.
    pub diff: String,
    /// First modified line.
    pub first_changed_line: Option<usize>,
}
/// Caught preview failure.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    /// Original error text.
    pub error: String,
}
/// A typed result fixture.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultRecord {
    /// Ordered text/image blocks.
    pub content: Vec<maestro_models::UserBlock>,
    /// Optional generated details.
    pub details: Option<Details>,
}
