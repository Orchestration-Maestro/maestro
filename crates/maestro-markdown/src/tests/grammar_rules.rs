//! Direct helper observations, independent of token adaptation.
use serde::Deserialize;
use serde_json::Value;

/// One recorded helper call and its independently recorded return value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Grammar snapshot that supplied the observation.
    version: String,
    /// Stable observation label.
    id: String,
    /// Helper selected by the recorder.
    symbol: String,
    /// Positional arguments, including absent optional column counts.
    args: Vec<Value>,
    /// Recorded return value.
    result: Value,
    /// Recorded UTF-8 boundary for successful closer searches.
    byte_boundary: Option<usize>,
}

/// Read only the observations for one helper.
fn cases(symbol: &str) -> impl Iterator<Item = Case> {
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("../../tests/fixtures/grammar_helpers.json")).unwrap();
    rows.into_iter().filter(move |row| row.symbol == symbol)
}

#[test]
fn cell_splitting_preserves_escape_parity_and_column_count() {
    for case in cases("splitCells") {
        let count = case.args[1].as_u64().map(|n| usize::try_from(n).unwrap());
        let actual = crate::helpers::split_cells(case.args[0].as_str().unwrap(), count);
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            case.result,
            "{} {}",
            case.version,
            case.id
        );
    }
}

#[test]
fn suffix_trimming_removes_only_the_selected_scalar() {
    for case in cases("rtrim") {
        let actual = crate::helpers::rtrim(
            case.args[0].as_str().unwrap(),
            case.args[1].as_str().unwrap().chars().next().unwrap(),
        );
        assert_eq!(
            actual,
            case.result.as_str().unwrap(),
            "{} {}",
            case.version,
            case.id
        );
    }
}

#[test]
fn balanced_closers_preserve_selected_utf8_boundary() {
    for case in cases("findClosingBracket") {
        let input = case.args[0].as_str().unwrap();
        let mut pair = case.args[1].as_str().unwrap().chars();
        let pair = (pair.next().unwrap(), pair.next().unwrap());
        let options = match case.version.as_str() {
            "15.0.4" => crate::MarkedOptions::Export,
            "15.0.12" => crate::MarkedOptions::Browser { escape_html: false },
            _ => crate::MarkedOptions::Terminal,
        };
        let actual = crate::helpers::find_closing_bracket(input, pair, options);
        let expected = case.result.as_i64().unwrap();
        let expected = match expected {
            -1 => crate::helpers::ClosingBracket::Missing,
            -2 => crate::helpers::ClosingBracket::Unclosed,
            _ => crate::helpers::ClosingBracket::Found(case.byte_boundary.unwrap()),
        };
        assert_eq!(actual, expected, "{} {}", case.version, case.id);
    }
}
