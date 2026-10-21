//! Stable pane numbers and historical aliases resolve only live panes.
#![cfg(test)]

use maestro_daemon::{app::state::DaemonState, settings::model::Config};

#[test]
fn public_target_forms_keep_alias_and_number_meaning() {
    let mut state = DaemonState::new(
        &Config::default(),
        || 0,
        std::sync::Arc::new(std::sync::RwLock::new(
            maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
        )),
    );
    let (removed, ()) = state
        .topology
        .new_with_tab("/first".into(), None, |_, _| Ok(()))
        .unwrap();
    state
        .topology
        .new_with_tab("/second".into(), None, |_, _| Ok(()))
        .unwrap();
    let (pane, ()) = state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    assert!(state.topology.workspaces[0].close_tab(0));
    assert_eq!(pane.raw(), 3);
    assert_eq!(state.public_pane_id(0, pane).as_deref(), Some("w1:p2"));
    assert_eq!(state.public_tab_id(0, 0).as_deref(), Some("w1:t2"));
    for spelling in ["w1:p2", "w1-2", "p_3", "p_w1_3"] {
        assert_eq!(state.parse_pane_id(spelling), Some((0, pane)));
    }
    assert_eq!(state.parse_pane_id("w1:p1"), None);
    assert_eq!(state.parse_pane_id("p_w2_3"), None);
    state.pane_id_aliases.insert(99, pane);
    state.public_pane_id_aliases.insert("old-pane".into(), pane);
    assert_eq!(state.parse_pane_id("p_99"), Some((0, pane)));
    assert_eq!(state.parse_pane_id("old-pane"), Some((0, pane)));
    assert_eq!(state.parse_current_public_pane_id("p_99"), None);
    assert_eq!(state.parse_current_public_pane_id("w1:p2"), Some((0, pane)));
    state.pane_id_aliases.insert(pane.raw(), removed);
    state
        .public_pane_id_aliases
        .insert("old-pane".into(), removed);
    assert_eq!(state.parse_pane_id("p_3"), None);
    assert_eq!(state.parse_pane_id("old-pane"), None);
}
