//! Replacement of read-only session and history files by supplied-path saves.
use super::support::snapshot::{Directory, empty};
use maestro_workspace::persist::save_to_paths;
use std::{fs, path::Path};

/// The Unix mode a file created in `dir` receives from the process's creation mask.
#[cfg(unix)]
fn default_mode(dir: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    let probe = dir.join("mode-probe");
    fs::write(&probe, b"").unwrap();
    let mode = fs::metadata(&probe).unwrap().permissions().mode() & 0o777;
    fs::remove_file(probe).unwrap();
    mode
}

/// Marks `path` read-only through the platform's native permission model.
fn make_read_only(path: &Path) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    #[cfg(unix)]
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o444);
    #[cfg(not(unix))]
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
#[test]
fn read_only_session_and_history_files_are_replaced_with_the_default_mode() {
    use super::support::snapshot::history;
    use std::os::unix::fs::PermissionsExt;
    let dir = Directory::new();
    let session = dir.path().join("session.json");
    let history_path = dir.path().join("history.json");
    save_to_paths(&session, &history_path, &empty(), Some(&history())).unwrap();
    make_read_only(&session);
    make_read_only(&history_path);
    let mut changed = empty();
    changed.sidebar_width = Some(31);

    save_to_paths(&session, &history_path, &changed, Some(&history())).unwrap();

    assert_eq!(
        fs::read(&session).unwrap(),
        serde_json::to_vec_pretty(&changed).unwrap()
    );
    for path in [&session, &history_path] {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            default_mode(dir.path())
        );
    }
}

#[cfg(windows)]
#[test]
fn read_only_session_file_is_refused_and_left_unchanged() {
    let dir = Directory::new();
    let session = dir.path().join("session.json");
    let history_path = dir.path().join("history.json");
    save_to_paths(&session, &history_path, &empty(), None).unwrap();
    make_read_only(&session);
    let before = fs::read(&session).unwrap();
    let mut changed = empty();
    changed.sidebar_width = Some(31);

    let error = save_to_paths(&session, &history_path, &changed, None).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(fs::read(&session).unwrap(), before);
}
