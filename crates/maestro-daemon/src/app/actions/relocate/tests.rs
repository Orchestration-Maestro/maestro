//! Emptied-workspace removal and recovery of a refused move keep identities and selection.

use super::*;
use crate::{settings::model::Config, terminal::state::TerminalState};
use maestro_detection::manifest::ManifestCache;
use maestro_workspace::layout::PaneId;
use std::sync::{Arc, RwLock};

/// Builds `count` workspaces of one tab and one pane each, without child processes.
fn state(count: usize) -> DaemonState<fn() -> u128> {
    let mut state = DaemonState::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        Arc::new(RwLock::new(ManifestCache::new(None, None).unwrap())),
    );
    for index in 0..count {
        state
            .topology
            .new_with_tab(format!("/w{index}").into(), None, |_, _| Ok(()))
            .unwrap();
    }
    for workspace in &state.topology.workspaces {
        for pane in workspace.tabs.iter().flat_map(|tab| tab.panes.values()) {
            let id = pane.attached_terminal_id.clone();
            state.terminals.insert(
                id.clone(),
                TerminalState::new(id, workspace.identity_cwd.clone()),
            );
        }
    }
    state.active = Some(0);
    state
}

/// The recovery context of the workspace at `index` and its tab at `tab`.
fn recovery_of(state: &DaemonState<fn() -> u128>, index: usize, tab: usize) -> PaneMoveRecovery {
    let workspace = &state.topology.workspaces[index];
    PaneMoveRecovery {
        workspace: index,
        workspace_id: workspace.id.clone(),
        workspace_label: workspace.custom_name.clone(),
        tab_label: workspace.tabs[tab].custom_name.clone(),
        worktree_space: workspace.worktree_space.clone(),
        identity_cwd: workspace.identity_cwd.clone(),
    }
}

/// Takes the root pane of the tab at `tab` of the workspace at `index` out of its workspace.
fn take(state: &mut DaemonState<fn() -> u128>, index: usize, tab: usize) -> (PaneId, MovedPane) {
    let pane = state.topology.workspaces[index].tabs[tab].root_pane;
    let taken = state.topology.workspaces[index]
        .take_pane_for_move(pane)
        .unwrap();
    (pane, taken.moved)
}

#[test]
fn removing_an_emptied_workspace_keeps_the_active_and_selected_workspaces() {
    let mut state = state(4);
    state.active = Some(3);
    state.selected = 2;
    state.remove_emptied_workspace(0);
    assert_eq!((state.active, state.selected), (Some(2), 1));
    state.remove_emptied_workspace(2);
    assert_eq!(state.topology.workspaces.len(), 2);
    assert_eq!((state.active, state.selected), (Some(1), 1));
    state.remove_emptied_workspace(1);
    assert_eq!((state.active, state.selected), (Some(0), 0));
    state.active = None;
    state.remove_emptied_workspace(0);
    assert!(state.topology.workspaces.is_empty());
    assert_eq!((state.active, state.selected), (None, 0));
}

#[test]
fn removing_a_workspace_before_the_selection_moves_it_up_and_after_it_leaves_it() {
    let mut state = state(3);
    state.active = Some(1);
    state.selected = 1;
    state.remove_emptied_workspace(2);
    assert_eq!((state.active, state.selected), (Some(1), 1));
    state.remove_emptied_workspace(1);
    assert_eq!((state.active, state.selected), (Some(0), 0));
}

#[test]
fn recovery_restores_a_removed_source_workspace_with_its_identity() {
    let mut state = state(1);
    state.topology.workspaces[0].custom_name = Some("issue".into());
    state.topology.workspaces[0].tabs[0].custom_name = Some("main".into());
    state.topology.workspaces[0].worktree_space = Some(WorktreeSpaceMembership {
        key: "repo-key".into(),
        label: "repo".into(),
        repo_root: "/repo".into(),
        checkout_path: "/repo-issue".into(),
        is_linked_worktree: true,
    });
    let recovery = recovery_of(&state, 0, 0);
    let (pane, moved) = take(&mut state, 0, 0);
    let terminal = moved.pane_state.attached_terminal_id.clone();
    let previous = state.topology.workspaces[0].clone();
    state.remove_emptied_workspace(0);
    assert!(state.topology.workspaces.is_empty());
    state.recover_failed_pane_move(recovery, moved);
    assert_eq!(state.topology.workspaces.len(), 1);
    let restored = &state.topology.workspaces[0];
    assert_eq!(restored.id, previous.id);
    assert_eq!(restored.custom_name.as_deref(), Some("issue"));
    assert_eq!(restored.tabs[0].custom_name.as_deref(), Some("main"));
    assert_eq!(restored.worktree_space, previous.worktree_space);
    assert_eq!(restored.identity_cwd, previous.identity_cwd);
    assert_eq!(restored.tabs[0].terminal_id(pane), Some(&terminal));
    assert_eq!(
        state.parse_pane_id(&format!("{}:p1", previous.id)),
        Some((0, pane))
    );
    assert!(state.session_dirty);
}

#[test]
fn recovery_puts_a_restored_workspace_back_in_its_place() {
    let mut state = state(3);
    state.active = Some(2);
    state.selected = 2;
    let recovery = recovery_of(&state, 1, 0);
    let (_, moved) = take(&mut state, 1, 0);
    let ids: Vec<_> = state
        .topology
        .workspaces
        .iter()
        .map(|workspace| workspace.id.clone())
        .collect();
    state.remove_emptied_workspace(1);
    assert_eq!((state.active, state.selected), (Some(1), 1));
    state.recover_failed_pane_move(recovery, moved);
    let restored: Vec<_> = state
        .topology
        .workspaces
        .iter()
        .map(|workspace| workspace.id.clone())
        .collect();
    assert_eq!(restored, ids);
    assert_eq!((state.active, state.selected), (Some(2), 2));
}

#[test]
fn recovery_adds_a_tab_to_a_surviving_workspace() {
    let mut state = state(1);
    state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    state.topology.workspaces[0].tabs[1].custom_name = Some("logs".into());
    let recovery = recovery_of(&state, 0, 1);
    let (pane, moved) = take(&mut state, 0, 1);
    assert_eq!(state.topology.workspaces[0].tabs.len(), 1);
    let terminal = moved.pane_state.attached_terminal_id.clone();
    state.recover_failed_pane_move(recovery, moved);
    let workspace = &state.topology.workspaces[0];
    assert_eq!(workspace.tabs.len(), 2);
    assert_eq!(workspace.tabs[1].custom_name.as_deref(), Some("logs"));
    assert_eq!(workspace.tabs[1].terminal_id(pane), Some(&terminal));
    assert_eq!(state.topology.workspaces.len(), 1);
    assert!(state.session_dirty);
}
