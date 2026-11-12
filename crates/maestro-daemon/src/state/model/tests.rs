//! The save mark is set by structural edits and read once.

use super::*;
use crate::settings::model::Config;
use maestro_detection::definitions::ManifestCache;

/// Builds empty state with a clear save mark.
fn state() -> DaemonState {
    DaemonState::new(
        &Config::default(),
        || 0,
        Arc::new(RwLock::new(ManifestCache::new(None, None).unwrap())),
    )
}

#[test]
fn new_state_has_no_pending_save() {
    assert!(!state().take_session_dirty());
}

#[test]
fn edit_topology_returns_the_result_and_marks_the_session() {
    let mut state = state();
    let count = state.edit_topology(|state| state.topology.workspaces.len());
    assert_eq!(count, 0);
    assert!(state.take_session_dirty());
}

#[test]
fn taking_the_mark_clears_it() {
    let mut state = state();
    state.edit_topology(|_| ());
    assert!(state.take_session_dirty());
    assert!(!state.take_session_dirty());
}

#[test]
fn conditional_mark_follows_the_change() {
    let mut state = state();
    state.mark_session_dirty_if(false);
    assert!(!state.take_session_dirty());
    state.mark_session_dirty_if(true);
    state.mark_session_dirty_if(false);
    assert!(state.take_session_dirty());
}
