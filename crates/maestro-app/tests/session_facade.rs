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

/// The application exposes the original resource port and progress callback.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn session_facade_reexports_progress_and_resource_owners() {
    use maestro_app::{
        NativeResourceOperations, ResourceEntry, ResourceFileType, ResourceOperations,
        SessionListProgress, canonicalize_path,
    };
    let operations: &dyn ResourceOperations = &NativeResourceOperations;
    let path = "missing-session-facade";
    assert_eq!(canonicalize_path(path, operations), path);
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
    let custom: &dyn ResourceOperations = &VirtualResources;
    assert_eq!(
        canonicalize_path("session", custom),
        std::path::Path::new("virtual-root")
            .join("session")
            .to_string_lossy()
    );
    let entries: Vec<ResourceEntry> = custom.read_dir(std::path::Path::new("ignored")).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "virtual");
    assert_eq!(entries[0].file_type, ResourceFileType::File);
}

/// A replacement resource adapter accessed solely through application types.
#[cfg(not(target_arch = "wasm32"))]
struct VirtualResources;
#[cfg(not(target_arch = "wasm32"))]
impl maestro_app::ResourceOperations for VirtualResources {
    fn exists(&self, path: &std::path::Path) -> bool {
        maestro_app::NativeResourceOperations.exists(path)
    }
    fn read_dir(
        &self,
        _path: &std::path::Path,
    ) -> std::io::Result<Vec<maestro_app::ResourceEntry>> {
        Ok(vec![maestro_app::ResourceEntry {
            name: "virtual".into(),
            file_type: maestro_app::ResourceFileType::File,
        }])
    }
    fn read_file(&self, path: &std::path::Path) -> std::io::Result<String> {
        maestro_app::NativeResourceOperations.read_file(path)
    }
    fn metadata(&self, path: &std::path::Path) -> std::io::Result<maestro_app::ResourceFileType> {
        maestro_app::NativeResourceOperations.metadata(path)
    }
    fn symlink_metadata(
        &self,
        path: &std::path::Path,
    ) -> std::io::Result<maestro_app::ResourceFileType> {
        maestro_app::NativeResourceOperations.symlink_metadata(path)
    }
    fn canonicalize(&self, path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
        Ok(std::path::Path::new("virtual-root").join(path))
    }
    fn current_directory(&self) -> std::io::Result<String> {
        maestro_app::NativeResourceOperations.current_directory()
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        maestro_app::NativeResourceOperations.drive_directories()
    }
}
