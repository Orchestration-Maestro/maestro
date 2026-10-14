//! Native text-run admission at scalar boundaries.
#![cfg(test)]
use serde::Deserialize;
/// One independent run observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Ordered native strings.
    runs: Vec<String>,
    /// Joined retained strings.
    expected: String,
}
#[test]
fn pdf_run_filter_keeps_padding_duplicates_and_scalar_boundaries() {
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("../../../../../tests/fixtures/pdf/runs.json")).unwrap();
    assert_eq!(rows.len(), 29);
    for row in rows {
        assert_eq!(super::pdf_page_text(row.runs), row.expected);
    }
}
