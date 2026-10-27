//! Portable platform guidance through the keyboard-help formatter.
use super::editing_keys;

#[test]
fn hotkeys_newline_guidance_covers_supported_hosts() {
    let ordinary = editing_keys(false);
    let windows = editing_keys(true);
    let row = "| `Shift+Enter` | New line |";
    let windows_row = "| `Shift+Enter` | New line (Ctrl+Enter on Windows Terminal) |";
    assert_eq!(
        ordinary.lines().find(|line| line.contains("New line")),
        Some(row)
    );
    assert_eq!(
        windows.lines().find(|line| line.contains("New line")),
        Some(windows_row)
    );
    assert_eq!(windows, ordinary.replace(row, windows_row));
}

/// One independently recorded extension label.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelCase {
    /// The witness associated with this input.
    test: String,
    /// The supplied shortcut spelling.
    input: LabelInput,
    /// The displayed label.
    expected: LabelExpected,
}

/// The unformatted shortcut.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelInput {
    /// Its authored spelling.
    key: String,
}

/// The recorded word-start capitalization.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelExpected {
    /// The complete label.
    result: String,
}

#[test]
fn hotkeys_capitalize_extension_word_starts() {
    let cases: Vec<LabelCase> =
        serde_json::from_str(include_str!("../../../tests/fixtures/hotkey_labels.json")).unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        assert_eq!(case.test, "hotkeys_capitalize_extension_word_starts");
        assert_eq!(
            super::capitalize_shortcut(&case.input.key),
            case.expected.result
        );
    }
}
