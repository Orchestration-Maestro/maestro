//! Public workspace identity forms preserve positional and stable numbering.
#![cfg(test)]

use maestro_daemon::{app::state::DaemonState, settings::model::Config};

#[test]
fn daemon_legacy_bare_tab_id_uses_tab_position_not_public_tab_number() {
    let mut state = DaemonState::new(&Config::default(), || 0);
    state
        .topology
        .new_with_tab("/workspace".into(), None, |_, _| Ok(()))
        .unwrap();
    for _ in 0..4 {
        state
            .topology
            .create_tab_with_runtime(0, |_, _| Ok(()))
            .unwrap()
            .unwrap();
    }
    assert!(state.topology.workspaces[0].close_tab(1));
    assert_eq!(state.topology.workspaces[0].tabs[2].number, 4);
    assert_eq!(state.topology.workspaces[0].tabs[3].number, 5);
    assert_eq!(state.parse_tab_id("w1:t4"), Some((0, 2)));
    assert_eq!(state.parse_tab_id("w1:4"), Some((0, 3)));
}
