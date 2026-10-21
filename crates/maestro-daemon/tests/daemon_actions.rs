//! Workspace reordering preserves logical navigation selection.
#![cfg(test)]

use maestro_daemon::{app::state::DaemonState, settings::model::Config};

/// Creates pure workspace topology with an active first workspace.
fn app_with_workspaces(names: &[&str]) -> DaemonState<fn() -> u128> {
    let mut state = DaemonState::new(&Config::default(), (|| 0) as fn() -> u128);
    for name in names {
        state
            .topology
            .new_with_tab(format!("/{name}").into(), None, |_, _| Ok(()))
            .unwrap();
    }
    state.active = (!names.is_empty()).then_some(0);
    state
}

/// Reads display names in current topology order.
fn names<C: FnMut() -> u128>(state: &DaemonState<C>) -> Vec<String> {
    state
        .topology
        .workspaces
        .iter()
        .map(|workspace| workspace.display_name(None))
        .collect()
}

#[test]
fn daemon_move_workspace_reorders_without_changing_logical_selection() {
    let mut state = app_with_workspaces(&["a", "b", "c"]);
    let active_id = state.topology.workspaces[1].id.clone();
    let selected_id = state.topology.workspaces[2].id.clone();
    state.active = Some(1);
    state.selected = 2;
    assert!(state.move_workspace(1, 0));
    assert_eq!(names(&state), ["b", "a", "c"]);
    assert_eq!(state.active, Some(0));
    assert_eq!(state.selected, 2);
    assert_eq!(
        state.topology.workspaces[state.active.unwrap()].id,
        active_id
    );
    assert_eq!(state.topology.workspaces[state.selected].id, selected_id);
    assert!(state.session_dirty);
}

#[test]
fn daemon_move_workspace_accepts_insert_at_end() {
    let mut state = app_with_workspaces(&["a", "b", "c"]);
    assert!(state.move_workspace(0, state.topology.workspaces.len()));
    assert_eq!(names(&state), ["b", "c", "a"]);
    state.session_dirty = false;
    assert!(!state.move_workspace(2, 3));
    assert!(!state.move_workspace(4, 0));
    assert!(!state.move_workspace(0, 4));
    assert!(!state.session_dirty);
}

#[test]
fn daemon_move_workspace_block_collects_non_contiguous_members() {
    let mut state = app_with_workspaces(&["child-one", "normal", "parent", "child-two", "tail"]);
    let ids: Vec<_> = state
        .topology
        .workspaces
        .iter()
        .map(|workspace| workspace.id.clone())
        .collect();
    state.active = Some(0);
    state.selected = 4;
    assert!(state.move_workspace_block(
        &[ids[2].clone(), ids[0].clone(), ids[3].clone()],
        Some(&ids[4])
    ));
    assert_eq!(
        names(&state),
        ["normal", "parent", "child-one", "child-two", "tail"]
    );
    assert_eq!(state.topology.workspaces[state.active.unwrap()].id, ids[0]);
    assert_eq!(state.topology.workspaces[state.selected].id, ids[4]);
    assert!(state.session_dirty);
}

#[test]
fn daemon_move_workspace_block_rejects_invalid_and_noop_orders() {
    let mut state = app_with_workspaces(&["a", "b", "c"]);
    let ids: Vec<_> = state
        .topology
        .workspaces
        .iter()
        .map(|workspace| workspace.id.clone())
        .collect();
    assert!(!state.move_workspace_block(&[], None));
    assert!(!state.move_workspace_block(&[ids[0].clone(), ids[0].clone()], None));
    assert!(!state.move_workspace_block(&["missing".into()], None));
    assert!(!state.move_workspace_block(&[ids[0].clone()], Some(&ids[0])));
    assert!(!state.move_workspace_block(&[ids[0].clone()], Some(&ids[1])));
    assert!(!state.move_workspace_block(&[ids[0].clone()], Some("missing")));
    assert_eq!(names(&state), ["a", "b", "c"]);
    assert!(!state.session_dirty);
    assert!(state.move_workspace_block(&[ids[0].clone()], None));
    assert_eq!(names(&state), ["b", "c", "a"]);
}

#[test]
fn daemon_notification_context_formats_resolved_workspace_label() {
    let mut state = app_with_workspaces(&["stale"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    assert_eq!(
        maestro_daemon::app::actions::notification_context(
            &state.topology.workspaces[0],
            "__maestro_projects__",
            0,
            root
        ),
        "__maestro_projects__ · 1"
    );
    let (pane, ()) = state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    state.topology.workspaces[0].tabs[1].custom_name = Some("logs".into());
    assert_eq!(
        maestro_daemon::app::actions::notification_context(
            &state.topology.workspaces[0],
            "resolved",
            2,
            pane
        ),
        "resolved · 3 · logs"
    );
    assert_eq!(
        maestro_daemon::app::actions::notification_context(
            &state.topology.workspaces[0],
            "resolved",
            2,
            maestro_workspace::layout::PaneId::from_raw(999)
        ),
        "resolved · 3"
    );
}
