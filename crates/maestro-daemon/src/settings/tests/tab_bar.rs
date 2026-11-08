//! Right-side tab-bar entry preferences.

use super::super::tabs::TabBarRightEntryConfig;
use serde_json::json;

#[test]
fn tab_entries_preserve_command_defaults() {
    let entries: Vec<TabBarRightEntryConfig> = serde_json::from_value(json!([
        {"type":"zoom"},{"type":"hostname"},{"type":"datetime"},
        {"type":"text","text":"status"},{"type":"command","command":"date"}
    ]))
    .unwrap();
    assert_eq!(
        entries,
        [
            TabBarRightEntryConfig::Zoom {},
            TabBarRightEntryConfig::Hostname {},
            TabBarRightEntryConfig::Datetime {
                format: "%H:%M".into()
            },
            TabBarRightEntryConfig::Text {
                text: "status".into()
            },
            TabBarRightEntryConfig::Command {
                command: "date".into(),
                interval_seconds: 5,
                timeout_seconds: 2
            },
        ]
    );
    for entry in [
        json!({"type":"zoom","extra":1}),
        json!({"type":"hostname","extra":1}),
        json!({"type":"datetime","extra":1}),
        json!({"type":"text","text":"ok","extra":1}),
        json!({"type":"command","command":"ok","extra":1}),
    ] {
        assert!(serde_json::from_value::<TabBarRightEntryConfig>(entry).is_err());
    }
}

#[test]
fn tab_entry_diagnostics_cover_format_and_schedules() {
    use super::super::tabs::{parse_tab_bar_datetime_format, tab_bar_right_diagnostics};
    assert_eq!(
        parse_tab_bar_datetime_format("").unwrap_err(),
        "datetime format is empty"
    );
    assert!(
        parse_tab_bar_datetime_format("%Q")
            .unwrap_err()
            .starts_with("invalid datetime format:")
    );
    assert!(
        parse_tab_bar_datetime_format("%z")
            .unwrap_err()
            .starts_with("unsupported datetime format:")
    );
    assert!(parse_tab_bar_datetime_format("%H:%M").is_ok());
    let entries: Vec<TabBarRightEntryConfig> = serde_json::from_value(json!([
        {"type":"datetime","format":""}, {"type":"datetime","format":"%Q"},
        {"type":"datetime","format":"%z"},
        {"type":"command","command":" ","intervalSeconds":0,"timeoutSeconds":0},
        {"type":"command","command":"ok","intervalSeconds":31_536_001,"timeoutSeconds":3601}
    ]))
    .unwrap();
    let errors = tab_bar_right_diagnostics(&entries);
    assert_eq!(
        errors[0],
        "multiplexer.tabBarRight[0] datetime format is empty; hiding entry"
    );
    assert!(errors[1].starts_with("multiplexer.tabBarRight[1] has invalid datetime format:"));
    assert!(errors[2].starts_with("multiplexer.tabBarRight[2] has unsupported datetime format:"));
    assert_eq!(
        &errors[3..],
        [
            "multiplexer.tabBarRight[3] command is empty; hiding entry",
            "multiplexer.tabBarRight[3] intervalSeconds must be at least 1; hiding entry",
            "multiplexer.tabBarRight[3] timeoutSeconds must be at least 1; hiding entry",
            "multiplexer.tabBarRight[4] intervalSeconds may be at most 31536000; hiding entry",
            "multiplexer.tabBarRight[4] timeoutSeconds may be at most 3600; hiding entry",
        ]
    );
    let valid: Vec<TabBarRightEntryConfig> = serde_json::from_value(json!([
        {"type":"command","command":"ok","intervalSeconds":1,"timeoutSeconds":1},
        {"type":"command","command":"ok","intervalSeconds":31_536_000,"timeoutSeconds":3600}
    ]))
    .unwrap();
    assert!(tab_bar_right_diagnostics(&valid).is_empty());
    let entries = vec![TabBarRightEntryConfig::Zoom {}; 16];
    assert!(tab_bar_right_diagnostics(&entries).is_empty());
    let mut entries = entries;
    entries.push(TabBarRightEntryConfig::Datetime {
        format: String::new(),
    });
    assert_eq!(
        tab_bar_right_diagnostics(&entries),
        ["multiplexer.tabBarRight may contain at most 16 entries; ignoring extras"]
    );
}
