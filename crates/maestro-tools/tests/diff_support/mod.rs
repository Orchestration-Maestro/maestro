//! Strict recorded renderer inputs and their public-output driver.
#![cfg(test)]

use crate::display_support::{Env, NoLanguages};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use maestro_tools::{RenderDiffOptions, render_diff};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
    rc::Rc,
};

/// Recorded color operand.
#[derive(Deserialize, serde::Serialize)]
#[serde(untagged)]
enum Color {
    /// Palette index.
    Index(u8),
    /// Hexadecimal RGB.
    Hex(String),
}
impl Color {
    /// Convert a recorded color for the theme constructor.
    fn value(&self) -> ColorValue {
        match self {
            Self::Index(i) => ColorValue::Index(*i),
            Self::Hex(s) => ColorValue::String(s.clone()),
        }
    }
}
/// All recorded render queries.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Default palette.
    palette: BTreeMap<String, Color>,
    /// Queries indexed by their consuming test.
    groups: BTreeMap<String, Vec<Case>>,
}
/// One output witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Public arguments.
    input: Input,
    /// Exact styled output.
    expected: String,
    /// Whether escape boundaries were corrected.
    #[serde(default)]
    corrected: bool,
}
/// Supplied invocation arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Numbered or raw diff.
    diff: String,
    /// Inert rendering options.
    #[serde(default)]
    options: Options,
    /// Optional palette replacement.
    colors: Option<BTreeMap<String, Color>>,
    /// Optional color mode replacement.
    mode: Option<String>,
}
/// Recorded optional path.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    /// Accepted path.
    #[serde(rename = "filePath")]
    file_path: Option<String>,
}
/// Assert the complete output of a named group.
pub fn group(name: &str) {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/diff-render-cases.json")).unwrap();
    inventory(&corpus);
    let cases = corpus.groups.get(name).unwrap();
    assert!(!cases.is_empty());
    for (index, case) in cases.iter().enumerate() {
        let colors = case.input.colors.as_ref().unwrap_or(&corpus.palette);
        let mode = match case.input.mode.as_deref() {
            None => ColorMode::Color256,
            Some("truecolor") => ColorMode::Truecolor,
            _ => panic!("unknown color mode"),
        };
        let theme = Theme::new(
            colors
                .iter()
                .map(|(key, value)| (ThemeColor::Named(key.clone()), value.value())),
            [],
            mode,
            ThemeOptions::default(),
        )
        .unwrap();
        let theme = if name == "diff_default_themes_keep_visible_signs_and_role_colors" {
            let shipped =
                crate::display_support::shipped_theme(["dark", "light"][index], Some(mode));
            for key in colors.keys() {
                let color = ThemeColor::Named(key.clone());
                assert_eq!(
                    shipped.get_fg_ansi(&color).unwrap(),
                    theme.get_fg_ansi(&color).unwrap()
                );
            }
            shipped
        } else {
            Rc::new(theme)
        };
        let env = Env::with(theme, Rc::new(NoLanguages));
        let options = RenderDiffOptions {
            file_path: case.input.options.file_path.clone(),
        };
        let actual = render_diff(&case.input.diff, &options, &env.live).unwrap();
        assert_eq!(actual, case.expected, "group {name}, case {index}");
        if case.corrected {
            assert_escapes(&case.input.diff, &actual);
        }
    }
}
/// Recognized source escapes survive the corrected decoration.
fn assert_escapes(source: &str, actual: &str) {
    let mut offset = 0;
    while offset < source.len() {
        if let Some(code) = maestro_tui::extract_ansi_code(source, offset) {
            assert!(actual.contains(code.code));
            offset += code.length;
        } else {
            offset += source[offset..].chars().next().unwrap().len_utf8();
        }
    }
}

/// Close the group inventory and reject duplicated public queries across groups.
fn inventory(corpus: &Corpus) {
    let mut expected = [
        "diff_raw_lines_preserve_framing",
        "diff_line_fields_follow_display_grammar",
        "diff_standalone_rows_keep_order_and_tabs",
        "diff_only_single_line_pairs_use_inverse",
        "diff_changed_tokens_keep_context_and_indentation",
        "diff_replacement_whitespace_keeps_common_boundaries",
        "diff_insertions_deduplicate_surrounding_whitespace",
        "diff_deletions_reconcile_both_context_sides",
        "diff_edge_deletions_use_whitespace_overlap",
        "diff_word_boundaries_preserve_latin_ranges_and_punctuation",
        "diff_whitespace_uses_the_shared_scalar_set",
        "diff_ambiguous_word_runs_match_recorded_selection",
        "diff_options_do_not_select_syntax_highlighting",
        "diff_inverse_preserves_embedded_style_boundaries",
        "diff_theme_roles_follow_supplied_colors",
        "diff_default_themes_keep_visible_signs_and_role_colors",
    ];
    expected.sort_unstable();
    assert_eq!(
        corpus.groups.keys().map(String::as_str).collect::<Vec<_>>(),
        expected
    );
    let mut queries = HashSet::new();
    let mut corrections = 0;
    for cases in corpus.groups.values() {
        for case in cases {
            assert!(
                queries.insert(
                    serde_json::to_string(&(
                        &case.input.diff,
                        &case.input.options.file_path,
                        &case.input.colors,
                        &case.input.mode
                    ))
                    .unwrap()
                )
            );
            corrections += usize::from(case.corrected);
        }
    }
    assert_eq!(queries.len(), 769);
    assert_eq!(corrections, 7);
}
