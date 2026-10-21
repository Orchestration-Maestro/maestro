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

/// The application exposes the resource records, the canonical path query and the progress callback.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_facade_reexports_progress_and_resource_owners() {
    use maestro_app::{ResourceEntry, ResourceFileType, SessionListProgress, canonicalize_path};
    let path = "missing-session-facade";
    assert_eq!(canonicalize_path(path), path);
    let entry: maestro_resources::ResourceEntry = ResourceEntry {
        name: "session".into(),
        file_type: ResourceFileType::File,
    };
    assert_eq!(entry.file_type, maestro_resources::ResourceFileType::File);
    let mut seen = None;
    let mut observe = |loaded, total| {
        seen = Some((loaded, total));
        Ok(())
    };
    let progress: &mut SessionListProgress<'_> = &mut observe;
    progress(2, 3).unwrap();
    assert_eq!(seen, Some((2, 3)));
    assert_eq!(
        maestro_app::current_directory().unwrap(),
        std::env::current_dir().unwrap().to_string_lossy()
    );
    assert_eq!(
        maestro_app::drive_directories(),
        maestro_resources::drive_directories()
    );
}
