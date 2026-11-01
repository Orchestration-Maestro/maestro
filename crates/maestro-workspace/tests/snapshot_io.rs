//! Native supplied-path save, clear and diagnostic behavior.
use super::support::snapshot::{Directory, capture, empty, history};
use maestro_workspace::persist::{clear_path, load, load_history, save_to_paths};
use serde_json::json;
use std::{fs, path::Path};

#[test]
fn pane_history_is_kept_in_its_own_file() {
    let dir = Directory::new();
    let session = dir.path().join("session.json");
    let history_path = dir.path().join("history.json");
    let snapshot = empty();
    let history = history();
    save_to_paths(&session, &history_path, &snapshot, Some(&history)).unwrap();
    let bytes = fs::read(&session).unwrap();
    assert_eq!(bytes, serde_json::to_vec_pretty(&snapshot).unwrap());
    assert!(!String::from_utf8(bytes).unwrap().contains("history"));
    assert!(
        !fs::read_to_string(&session)
            .unwrap()
            .contains("private screen")
    );
    assert!(
        fs::read_to_string(&history_path)
            .unwrap()
            .contains("private screen")
    );
    assert_eq!(load(&session).unwrap().sidebar_width, Some(26));
    assert_eq!(
        serde_json::to_value(load_history(&history_path).unwrap()).unwrap(),
        serde_json::to_value(history).unwrap()
    );
}

#[test]
fn disabling_history_removes_previous_file() {
    let dir = Directory::new();
    let session = dir.path().join("session.json");
    let history_path = dir.path().join("history.json");
    save_to_paths(&session, &history_path, &empty(), Some(&history())).unwrap();
    assert!(load_history(&history_path).is_some());
    save_to_paths(&session, &history_path, &empty(), None).unwrap();
    assert!(!history_path.exists());
    assert!(load(&session).is_some());
}

#[test]
fn clear_removes_saved_snapshot() {
    let dir = Directory::new();
    let path = dir.path().join("snapshot");
    save_to_paths(&path, &dir.path().join("history"), &empty(), None).unwrap();
    assert!(load(&path).is_some());
    clear_path(&path).unwrap();
    assert!(!path.exists());
}

#[test]
fn clear_missing_snapshot_succeeds() {
    let dir = Directory::new();
    let path = dir.path().join("missing");
    clear_path(&path).unwrap();
    clear_path(&path).unwrap();
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[cfg(unix)]
fn assert_link_save(link: &Path, target: &Path) {
    let original = fs::read_link(link).unwrap();
    let mut snapshot = empty();
    snapshot.selected = 7;
    save_to_paths(link, &link.with_extension("history"), &snapshot, None).unwrap();
    assert_eq!(fs::read_link(link).unwrap(), original);
    assert_eq!(load(target).unwrap().selected, 7);
    assert_eq!(
        fs::read(target).unwrap(),
        serde_json::to_vec_pretty(&snapshot).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn saving_keeps_existing_symlink() {
    let dir = Directory::new();
    let target = dir.path().join("real.json");
    let link = dir.path().join("link.json");
    fs::write(&target, b"old").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert_link_save(&link, &target);
}

#[cfg(unix)]
#[test]
fn saving_follows_dangling_symlink() {
    let dir = Directory::new();
    let target = dir.path().join("new-parent/real.json");
    let link = dir.path().join("link.json");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert_link_save(&link, &target);
}

#[cfg(unix)]
#[test]
fn saving_follows_relative_symlink() {
    let dir = Directory::new();
    let target = dir.path().join("real.json");
    let link = dir.path().join("link.json");
    std::os::unix::fs::symlink("real.json", &link).unwrap();
    assert_link_save(&link, &target);
}

#[test]
fn loads_report_each_read_parse_and_version_failure() {
    let dir = Directory::new();
    for is_history in [false, true] {
        let noun = if is_history {
            "session history file"
        } else {
            "session file"
        };
        let attempt = |path: &Path| {
            if is_history {
                load_history(path).is_some()
            } else {
                load(path).is_some()
            }
        };
        assert_missing_and_unreadable(&dir, &attempt, noun);
        let path = dir.path().join("invalid");
        fs::write(&path, "{").unwrap();
        let (loaded, events) = capture(|| attempt(&path));
        assert!(!loaded);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0]["message"],
            format!("failed to parse {noun}, ignoring")
        );
        assert!(!events[0]["err"].is_empty());
        fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({"version":999,"workspaces":[]})).unwrap(),
        )
        .unwrap();
        let (loaded, events) = capture(|| attempt(&path));
        assert!(!loaded);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0]["message"],
            format!("{noun} is from a newer maestro version, ignoring")
        );
        assert_eq!(events[0]["file_version"], "999");
        assert_eq!(events[0]["supported"], "1");
    }
}

#[test]
fn paired_save_keeps_topology_if_history_fails() {
    let dir = Directory::new();
    let session = dir.path().join("session");
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, b"not a directory").unwrap();
    let path = blocker.join("history");
    let mut snapshot = empty();
    snapshot.selected = 7;
    assert!(save_to_paths(&session, &path, &snapshot, Some(&history())).is_err());
    assert_eq!(load(&session).unwrap().selected, 7);
    snapshot.selected = 8;
    // Unix refuses a path below a file; Windows reports it absent, which removal accepts.
    let refused =
        fs::remove_file(&path).is_err_and(|error| error.kind() != std::io::ErrorKind::NotFound);
    assert_eq!(
        save_to_paths(&session, &path, &snapshot, None).is_err(),
        refused
    );
    assert_eq!(load(&session).unwrap().selected, 8);
    let history_path = dir.path().join("kept-history");
    fs::write(&history_path, b"old history").unwrap();
    assert!(
        save_to_paths(
            &blocker.join("session"),
            &history_path,
            &snapshot,
            Some(&history())
        )
        .is_err()
    );
    assert_eq!(fs::read(history_path).unwrap(), b"old history");
}

#[test]
fn clear_preserves_link_target_and_reports_other_errors() {
    let dir = Directory::new();
    let target = dir.path().join("target");
    fs::write(&target, b"kept").unwrap();
    #[cfg(unix)]
    {
        let link = dir.path().join("link");
        std::os::unix::fs::symlink("target", &link).unwrap();
        clear_path(&link).unwrap();
        assert!(fs::symlink_metadata(&link).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"kept");
    }
    let error = clear_path(dir.path()).unwrap_err();
    assert_ne!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(fs::read(target).unwrap(), b"kept");
}

/// Checks absence separately from a reached native read failure.
fn assert_missing_and_unreadable(dir: &Directory, attempt: &impl Fn(&Path) -> bool, noun: &str) {
    let (loaded, events) = capture(|| attempt(&dir.path().join("missing")));
    assert!(!loaded);
    assert!(events.is_empty());
    let (loaded, events) = capture(|| attempt(dir.path()));
    assert!(!loaded);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["message"], format!("failed to read {noun}"));
    assert!(!events[0]["err"].is_empty());
}

#[cfg(unix)]
#[test]
fn saving_preserves_snapshot_target_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = maestro_test_directory::Scratch::new().unwrap();
    let target = dir.path().join("session.json");
    let link = dir.path().join("link.json");
    fs::write(&target, b"old").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    std::os::unix::fs::symlink("session.json", &link).unwrap();
    save_to_paths(&link, &dir.path().join("history"), &empty(), None).unwrap();
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(fs::read_link(link).unwrap(), Path::new("session.json"));
    assert!(load(&target).is_some());
}

#[cfg(unix)]
#[test]
fn clearing_interrupted_history_prevents_later_recovery_from_resurrecting_it() {
    let dir = Directory::new();
    let parent = dir.path().join("saved");
    fs::create_dir(&parent).unwrap();
    let history_path = parent.join("history.json");
    let Some(restore) = maestro_test_directory::deny(&parent, 0o555).unwrap() else {
        return;
    };
    let error = maestro_lock::replace(
        &history_path,
        b"private interrupted history",
        maestro_lock::Replace::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    drop(restore);
    assert!(!history_path.exists());
    clear_path(&history_path).unwrap();
    save_to_paths(&parent.join("session.json"), &history_path, &empty(), None).unwrap();
    let _holder = maestro_lock::acquire_write(&history_path, true).unwrap();
    assert!(
        !history_path.exists(),
        "a cleared recovery record resurrected private history"
    );
}
