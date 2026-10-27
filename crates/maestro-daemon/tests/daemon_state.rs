//! Daemon geometry before the first attached view.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::support;
use support::{assert_identity_invariants, populate_terminals};

use maestro_daemon::{app::state::DaemonState, settings::model::Config};
use maestro_workspace::layout::{Borders, PaneId, PaneInfo, Rect};

#[test]
fn daemon_pane_size_estimate_uses_headless_size_before_first_view() {
    let mut config = Config::default();
    config.server.headless_cols = 132;
    config.server.headless_rows = 41;
    let mut state = DaemonState::new(&config, || 0, support::manifest_cache());
    assert_eq!(state.estimate_pane_size(), (41, 132));
    let rect = Rect::new(0, 0, 72, 19);
    state.pane_infos.push(PaneInfo {
        id: PaneId::from_raw(1),
        rect,
        inner_rect: rect,
        scrollbar_rect: None,
        borders: Borders::NONE,
        is_focused: true,
    });
    assert_eq!(state.estimate_pane_size(), (19, 72));
}

#[test]
fn daemon_adversarial_identity_state_satisfies_app_invariants_after_mutation() {
    let mut state = DaemonState::new(&Config::default(), || 0, support::manifest_cache());
    state
        .topology
        .new_with_tab("/workspace".into(), None, |_, _| Ok(()))
        .unwrap();
    for _ in 0..3 {
        state
            .topology
            .create_tab_with_runtime(0, |_, _| Ok(()))
            .unwrap()
            .unwrap();
    }
    assert!(state.topology.workspaces[0].close_tab(1));
    assert!(state.topology.workspaces[0].move_tab(0, 3));
    state.topology.workspaces[0].active_tab = 0;
    state.active = Some(0);
    state.selected = 0;
    populate_terminals(&mut state);
    assert_identity_invariants(&state);
    let workspace = &state.topology.workspaces[0];
    assert_ne!(
        workspace.active_tab + 1,
        workspace.tabs[workspace.active_tab].number
    );
    let root = workspace.tabs[0].root_pane;
    let (pane, ()) = state
        .topology
        .split_pane_with_runtime(
            0,
            maestro_workspace::layout::SplitRequest {
                target: root,
                direction: maestro_protocol::schema::SplitDirection::Right,
                ratio: Some(0.5),
                focus_new_pane: true,
            },
            |_, _| Ok(()),
        )
        .unwrap()
        .unwrap();
    assert!(
        state.topology.workspaces[0]
            .public_pane_number(pane)
            .is_some()
    );
    populate_terminals(&mut state);
    assert_identity_invariants(&state);
}
