//! Environment reads and flag truth against recorded results.

/// Probe name that no environment sets.
const ABSENT: &str = "MAESTRO_TEXT_ABSENT_ENVIRONMENT_PROBE";

#[test]
fn an_absent_environment_variable_reads_as_no_value() {
    assert_eq!(maestro_text::environment_variable(ABSENT), None);
}

/// Relies on `PATH`, which every supported runner sets.
#[test]
fn a_present_environment_variable_reads_its_value() {
    assert!(maestro_text::environment_variable("PATH").is_some_and(|value| !value.is_empty()));
}

#[test]
fn truthy_env_flags_accept_one_true_or_yes_exactly() {
    for value in [
        None,
        Some(""),
        Some("0"),
        Some("false"),
        Some("2"),
        Some(" true"),
        Some("yes "),
        Some("\u{feff}true"),
        Some("true\u{85}"),
        Some("yeſ"),
        Some("YEſ"),
        Some("ｔrue"),
    ] {
        assert!(!maestro_text::is_truthy_env_flag(value));
    }
    for value in ["1", "true", "TRUE", "TrUe", "yes", "YES", "YeS"] {
        assert!(maestro_text::is_truthy_env_flag(Some(value)));
    }
}

#[test]
fn offline_selections_override_the_ambient_flag() {
    assert!(maestro_text::is_offline_selected(Some(true)));
    assert!(!maestro_text::is_offline_selected(Some(false)));
}
