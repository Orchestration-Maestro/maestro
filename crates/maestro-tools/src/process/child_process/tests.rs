//! Recorded command-selection cases.
#![cfg(test)]
use serde::Deserialize;

/// A command and its explicit platform.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Recorded platform.
    platform: String,
    /// Literal executable spelling.
    command: String,
}

/// Expected selection.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Whether shell interpretation is needed.
    value: bool,
}

#[test]
fn windows_shell_predicate_matches_accepted_command_forms() {
    for row in crate::utils::shell::tests::cases("selector") {
        let input: Input = serde_json::from_value(row.input).unwrap();
        let expected: Expected = serde_json::from_value(row.expected).unwrap();
        assert_eq!(
            super::windows_shell(&input.command, input.platform == "win32"),
            expected.value,
            "{}",
            row.id
        );
    }
}
