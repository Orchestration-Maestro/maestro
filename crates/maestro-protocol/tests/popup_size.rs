//! Popup size parsing at the command-line and JSON boundaries.
#![cfg(test)]

use maestro_protocol::shared::PopupSize;

#[test]
/// Command-line sizes distinguish cells from percentages.
fn cli_size_units_select_cells_or_percentages() {
    assert_eq!(PopupSize::parse_cli("120"), Ok(PopupSize::Cells(120)));
    assert_eq!(PopupSize::parse_cli("80%"), Ok(PopupSize::Percent(80)));
}

#[test]
/// Percentages require digits and a value from one through one hundred.
fn cli_percentages_reject_zero_overflow_and_missing_digits() {
    assert!(PopupSize::parse_cli("0%").is_err());
    assert!(PopupSize::parse_cli("101%").is_err());
    assert!(PopupSize::parse_cli("%").is_err());
}

#[test]
/// JSON strings represent percentages, not cell counts.
fn json_size_strings_require_a_percent_suffix() {
    assert!(serde_json::from_value::<PopupSize>(serde_json::json!("120")).is_err());
    assert_eq!(
        serde_json::from_value::<PopupSize>(serde_json::json!("80%")).unwrap(),
        PopupSize::Percent(80)
    );
}
