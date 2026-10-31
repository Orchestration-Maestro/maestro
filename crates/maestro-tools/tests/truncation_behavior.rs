//! Recorded output comparisons through the public helpers.

use maestro_tools::{TruncationOptions, format_size, truncate_head, truncate_line, truncate_tail};
use serde::Deserialize;
use serde_json::Value;

/// A native count or the corpus's target-independent unlimited marker.
#[derive(Deserialize)]
#[serde(untagged)]
enum Budget {
    /// A finite count.
    Count(usize),
    /// The unlimited marker.
    Marker(String),
}

impl Budget {
    /// Resolve a corpus budget without changing observed output.
    fn resolve(self) -> usize {
        match self {
            Self::Count(value) => value,
            Self::Marker(marker) => {
                assert_eq!(marker, "unlimited");
                usize::MAX
            }
        }
    }
}

/// Independently optional limits.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Options {
    /// Line budget.
    max_lines: Option<Budget>,
    /// Byte budget.
    max_bytes: Option<Budget>,
}

/// One consumed oracle invocation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    /// Owning test name.
    test: String,
    /// Failure label.
    rule: String,
    /// Operation name.
    op: String,
    /// String input.
    #[serde(default)]
    text: String,
    /// Selection limits.
    #[serde(default)]
    options: Options,
    /// Scalar limit.
    max_chars: Option<usize>,
    /// Size input.
    bytes: Option<f64>,
    /// Independently recorded output tree.
    expected: Value,
}

/// Compare every output field in the selected nonempty test group.
fn check(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/truncation_cases.json"))?;
    let mut count = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        count += 1;
        let options = TruncationOptions {
            max_lines: case.options.max_lines.map(Budget::resolve),
            max_bytes: case.options.max_bytes.map(Budget::resolve),
        };
        let actual = match case.op.as_str() {
            "head" => serde_json::to_value(truncate_head(&case.text, options))?,
            "tail" => serde_json::to_value(truncate_tail(&case.text, options))?,
            "line" => serde_json::to_value(truncate_line(&case.text, case.max_chars))?,
            "size" => serde_json::to_value(format_size(case.bytes.ok_or("missing size input")?))?,
            _ => return Err("unsupported operation".into()),
        };
        let mut expected = case.expected;
        for key in ["maxLines", "maxBytes"] {
            if expected.get(key) == Some(&Value::String("unlimited".into())) {
                expected[key] = serde_json::to_value(usize::MAX)?;
            }
        }
        assert_eq!(actual, expected, "{}", case.rule);
    }
    assert!(count > 0, "empty test group");
    Ok(())
}

#[test]
fn head_fitting_input_retains_bytes_and_counts() -> Result<(), Box<dyn std::error::Error>> {
    check("head_fitting_input_retains_bytes_and_counts")
}

#[test]
fn head_line_quota_keeps_first_entries() -> Result<(), Box<dyn std::error::Error>> {
    check("head_line_quota_keeps_first_entries")
}

#[test]
fn head_oversized_first_line_is_empty() -> Result<(), Box<dyn std::error::Error>> {
    check("head_oversized_first_line_is_empty")
}

#[test]
fn tail_fitting_input_retains_bytes_and_counts() -> Result<(), Box<dyn std::error::Error>> {
    check("tail_fitting_input_retains_bytes_and_counts")
}

#[test]
fn tail_line_quota_keeps_last_entries() -> Result<(), Box<dyn std::error::Error>> {
    check("tail_line_quota_keeps_last_entries")
}

#[test]
fn tail_partial_last_line_starts_on_utf8_boundary() -> Result<(), Box<dyn std::error::Error>> {
    check("tail_partial_last_line_starts_on_utf8_boundary")
}

#[test]
fn line_clipping_keeps_fitting_text() -> Result<(), Box<dyn std::error::Error>> {
    check("line_clipping_keeps_fitting_text")
}

#[test]
fn line_clipping_uses_exact_suffix() -> Result<(), Box<dyn std::error::Error>> {
    check("line_clipping_uses_exact_suffix")
}

#[test]
fn size_formatting_uses_byte_kib_mib_boundaries() -> Result<(), Box<dyn std::error::Error>> {
    check("size_formatting_uses_byte_kib_mib_boundaries")
}

#[test]
fn independent_defaults_keep_the_other_limit() -> Result<(), Box<dyn std::error::Error>> {
    check("independent_defaults_keep_the_other_limit")
}

#[test]
fn default_limits_apply_at_the_boundary() -> Result<(), Box<dyn std::error::Error>> {
    check("default_limits_apply_at_the_boundary")
}

#[test]
fn head_byte_quota_includes_newline_cost() -> Result<(), Box<dyn std::error::Error>> {
    check("head_byte_quota_includes_newline_cost")
}

#[test]
fn head_complete_line_quota_wins_at_tie() -> Result<(), Box<dyn std::error::Error>> {
    check("head_complete_line_quota_wins_at_tie")
}

#[test]
fn head_zero_limits_preserve_entry_metadata() -> Result<(), Box<dyn std::error::Error>> {
    check("head_zero_limits_preserve_entry_metadata")
}

#[test]
fn tail_byte_quota_keeps_complete_suffix_lines() -> Result<(), Box<dyn std::error::Error>> {
    check("tail_byte_quota_keeps_complete_suffix_lines")
}

#[test]
fn partial_tail_reports_bytes_even_at_line_quota() -> Result<(), Box<dyn std::error::Error>> {
    check("partial_tail_reports_bytes_even_at_line_quota")
}

#[test]
fn complete_tail_line_quota_still_reports_lines() -> Result<(), Box<dyn std::error::Error>> {
    check("complete_tail_line_quota_still_reports_lines")
}

#[test]
fn tail_zero_limits_distinguish_empty_selection_from_partial_entry()
-> Result<(), Box<dyn std::error::Error>> {
    check("tail_zero_limits_distinguish_empty_selection_from_partial_entry")
}

#[test]
fn unlimited_line_budget_leaves_byte_limit_active() -> Result<(), Box<dyn std::error::Error>> {
    check("unlimited_line_budget_leaves_byte_limit_active")
}

#[test]
fn unlimited_byte_budget_leaves_line_limit_active() -> Result<(), Box<dyn std::error::Error>> {
    check("unlimited_byte_budget_leaves_line_limit_active")
}

#[test]
fn line_clipping_counts_unicode_scalars() -> Result<(), Box<dyn std::error::Error>> {
    check("line_clipping_counts_unicode_scalars")
}

#[test]
fn size_formatting_rounds_positive_half_ties_up() -> Result<(), Box<dyn std::error::Error>> {
    check("size_formatting_rounds_positive_half_ties_up")
}

#[test]
fn size_formatting_retains_number_spelling() -> Result<(), Box<dyn std::error::Error>> {
    check("size_formatting_retains_number_spelling")
}

#[test]
fn tail_keeps_whole_scalars_of_an_oversized_line_ending_in_a_newline() {
    let text = format!("ab{}\n", "€".repeat(10));
    let result = truncate_tail(
        &text,
        TruncationOptions {
            max_lines: None,
            max_bytes: Some(11),
        },
    );
    assert_eq!(result.content, "€€€\n");
    assert_eq!((result.total_bytes, result.total_lines), (33, 2));
    assert_eq!((result.output_bytes, result.output_lines), (10, 2));
    assert!(result.truncated && result.last_line_partial);
    assert!(matches!(
        result.truncated_by,
        Some(maestro_tools::TruncationBy::Bytes)
    ));
    assert_eq!(result.max_bytes, 11);
}
