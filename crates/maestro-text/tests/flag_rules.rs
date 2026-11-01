//! Environment flag truth against recorded results.

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
