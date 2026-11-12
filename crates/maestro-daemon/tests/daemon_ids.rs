//! Stable pane numbers and historical aliases resolve only live panes.
#![cfg(test)]
#![cfg(target_os = "linux")]

#[cfg(unix)]
use maestro_daemon::{settings::model::Config, state::model::DaemonState};
#[cfg(unix)]
#[cfg(unix)]
#[cfg(unix)]
#[cfg(unix)]
#[cfg(unix)]
#[test]
fn workspace_and_tab_spellings_keep_positions_distinct_from_public_numbers() {
    let mut state = DaemonState::new(
        &Config::default(),
        || 0,
        std::sync::Arc::new(std::sync::RwLock::new(
            maestro_detection::definitions::ManifestCache::new(None, None).unwrap(),
        )),
    );
    for cwd in ["/first", "/second"] {
        state
            .topology
            .new_with_tab(cwd.into(), None, |_, _| Ok(()))
            .unwrap();
    }
    state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    assert!(state.topology.workspaces[0].close_tab(0));
    state.topology.workspaces.swap(0, 1);
    assert_eq!(state.parse_workspace_id("w1"), Some(1));
    for form in ["w_1", "1", "w2"] {
        assert_eq!(state.parse_workspace_id(form), Some(0));
    }
    for form in ["w1:t2", "w1:1", "t_w1_1", "t_w_2_1", "2:1"] {
        assert_eq!(state.parse_tab_id(form), Some((1, 0)), "{form}");
    }
    for form in ["", "0", "w_0", "w999", "w_-1", "w_1x"] {
        assert_eq!(state.parse_workspace_id(form), None, "{form}");
    }
    for form in [
        "w1:t1",
        "w1:t0",
        "w1:t!",
        "w1:0",
        "w1:99",
        "t_w1_0",
        "w999:t1",
        "999:1",
        "w1:t2:extra",
    ] {
        assert_eq!(state.parse_tab_id(form), None, "{form}");
    }
    for form in [
        "w1:p0", "w1:p!", "w1-0", "p_0", "p_w1_0", "w999:p1", "999-1", "p_x", "p_999999", "missing",
    ] {
        assert_eq!(state.parse_pane_id(form), None, "{form}");
    }
}
