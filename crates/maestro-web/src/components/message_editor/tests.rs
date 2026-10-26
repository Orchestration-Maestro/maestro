//! Send admission, the reported size limit and level spellings.
use super::*;

#[test]
fn drafts_send_with_visible_text_or_an_attachment_while_no_file_loads() {
    assert!(!can_send("", 0, false));
    assert!(
        !can_send(" \n\t\u{a0}\u{feff}\u{3000}", 0, false),
        "whitespace alone"
    );
    assert!(can_send("  hi ", 0, false));
    assert!(can_send("", 1, false), "an attachment alone");
    assert!(!can_send("hi", 1, true), "never while files load");
}

#[test]
fn size_limit_reports_whole_mebibytes_rounding_halves_up() {
    assert_eq!(limit_megabytes(MAX_FILE_SIZE), 20);
    assert_eq!(limit_megabytes(1024 * 1024 + 512 * 1024), 2);
    assert_eq!(limit_megabytes(1024 * 1024 + 512 * 1024 - 1), 1);
    assert_eq!(limit_megabytes(0), 0);
}

#[test]
fn levels_use_their_stored_spelling_in_menu_order() {
    let values: Vec<_> = THINKING_LEVELS.iter().map(level_value).collect();
    assert_eq!(values, ["off", "minimal", "low", "medium", "high"]);
    for level in THINKING_LEVELS {
        assert_eq!(
            serde_json::to_value(&level).unwrap(),
            serde_json::Value::from(level_value(&level))
        );
    }
}

#[test]
fn defaults_follow_the_documented_limits_and_controls() {
    let props = MessageEditorProps::default();
    assert_eq!(
        (props.max_files, props.max_file_size),
        (10, 20 * 1024 * 1024)
    );
    assert_eq!(props.accepted_types, ACCEPTED_TYPES);
    assert_eq!(
        props.controls,
        EditorControls {
            attachments: true,
            model: true,
            thinking: true
        }
    );
    assert_eq!(props.thinking_level, ThinkingLevel::Off);
}
