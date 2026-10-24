//! Snapshot effects and their ordered diagnostic events.
use super::*;
use crate::logging::tests::capture;
use maestro_test_directory::{Scratch, temporary_root};
use maestro_workspace::persist::snapshot::{SNAPSHOT_VERSION, SessionHistorySnapshot};
use std::fs;

#[test]
fn daemon_snapshot_wrappers_preserve_save_and_clear_order() {
    // Windows runs this too: DaemonPaths joins with '/', which a canonical `\\?\` root refuses.
    let scratch = Scratch::new_in(&temporary_root()).unwrap();
    let paths = DaemonPaths::new(scratch.path().to_str().unwrap(), Some("work")).unwrap();
    let snapshot: SessionSnapshot = serde_json::from_value(serde_json::json!({
        "version":SNAPSHOT_VERSION,"workspaces":[],"active":null,"selected":0
    }))
    .unwrap();
    let history = SessionHistorySnapshot {
        version: SNAPSHOT_VERSION,
        workspaces: vec![],
    };
    let ((), events) = capture(|| save(&paths, &snapshot, Some(&history)));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].fields["event"], "persist.save");
    assert_eq!(events[0].fields["outcome"], "ok");
    assert_eq!(events[0].fields["workspaces"], 0);
    assert_eq!(
        fs::read_to_string(paths.snapshot_path()).unwrap(),
        serde_json::to_string_pretty(&snapshot).unwrap()
    );
    assert_eq!(
        fs::read_to_string(paths.history_path()).unwrap(),
        serde_json::to_string_pretty(&history).unwrap()
    );
    save(&paths, &snapshot, None);
    assert!(!std::path::Path::new(&paths.history_path()).exists());
    save(&paths, &snapshot, Some(&history));
    let ((), events) = capture(|| clear(&paths));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].fields["event"], "persist.clear");
    assert_eq!(events[0].fields["path"], paths.snapshot_path());
    assert!(!std::path::Path::new(&paths.snapshot_path()).exists());
    assert!(!std::path::Path::new(&paths.history_path()).exists());
    assert!(capture(|| clear_history(&paths)).1.is_empty());
    failure_order(&paths, &snapshot, &history);
}

/// Fail topology and history independently through the real filesystem owner.
fn failure_order(
    paths: &DaemonPaths,
    snapshot: &SessionSnapshot,
    history: &SessionHistorySnapshot,
) {
    let topology = paths.snapshot_path();
    let history_path = paths.history_path();
    fs::create_dir(&topology).unwrap();
    fs::write(&history_path, "retain history").unwrap();
    for events in [
        capture(|| save(paths, snapshot, Some(history))).1,
        capture(|| clear(paths)).1,
    ] {
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].level, tracing::Level::ERROR);
        assert_eq!(events[0].fields["outcome"], "error");
        assert_eq!(events[0].fields["path"], topology);
        assert_eq!(fs::read_to_string(&history_path).unwrap(), "retain history");
    }
    fs::remove_dir(&topology).unwrap();
    fs::remove_file(&history_path).unwrap();
    fs::create_dir(&history_path).unwrap();
    history_failure_order(paths, snapshot, history);
}

/// History failures preserve committed topology and retain their own path attribution.
fn history_failure_order(
    paths: &DaemonPaths,
    snapshot: &SessionSnapshot,
    history: &SessionHistorySnapshot,
) {
    let topology = paths.snapshot_path();
    let history_path = paths.history_path();
    let ((), events) = capture(|| save(paths, snapshot, Some(history)));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].fields["event"], "persist.save");
    assert_eq!(events[0].fields["outcome"], "error");
    assert!(std::path::Path::new(&topology).is_file());
    let ((), events) = capture(|| clear_history(paths));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].fields["path"], history_path);
    let ((), events) = capture(|| clear(paths));
    assert_eq!(
        events
            .iter()
            .map(|e| e.fields["outcome"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["error", "ok"]
    );
    assert_eq!(events[0].fields["path"], history_path);
    assert_eq!(events[1].fields["path"], topology);
    assert!(!std::path::Path::new(&topology).exists());
}
