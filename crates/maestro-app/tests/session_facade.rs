//! Portable application exports retain the session owner's records.
#![cfg(test)]

use maestro_app::{Presence, SessionInfo, is_whitespace_scalar};

#[test]
fn session_facade_uses_original_records() {
    let ids: [maestro_session::Presence<serde_json::Value>; 3] = [
        Presence::Missing,
        Presence::Null,
        Presence::Present(serde_json::json!("identity")),
    ];
    for (index, id) in ids.into_iter().enumerate() {
        let record: maestro_session::SessionInfo = SessionInfo {
            path: "session".into(),
            id,
            cwd: String::new(),
            name: None,
            parent_session_path: Presence::Missing,
            created: 0.0,
            modified: 0.0,
            message_count: 0,
            first_message: String::new(),
            all_messages_text: String::new(),
        };
        match record.id {
            maestro_session::Presence::Missing => assert_eq!(index, 0),
            maestro_session::Presence::Null => assert_eq!(index, 1),
            maestro_session::Presence::Present(value) => {
                assert_eq!(value.as_str(), Some("identity"));
            }
        }
    }
    assert!(is_whitespace_scalar('\u{feff}'));
    assert!(!is_whitespace_scalar('\u{85}'));
}
