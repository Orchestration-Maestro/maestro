//! Workspace reordering preserves logical navigation selection.
#![cfg(test)]

mod support;
use support::{assert_identity_invariants, populate_terminals};

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
    populate_terminals(&mut state);
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

#[test]
fn daemon_switch_workspace_updates_active_and_selected() {
    let mut state = app_with_workspaces(&["a", "b", "c"]);
    state.switch_workspace(2);
    assert_eq!(state.active, Some(2));
    assert_eq!(state.selected, 2);
}

/// Adds a right-hand pane using the topology's consuming runtime callback.
fn split(
    state: &mut DaemonState<fn() -> u128>,
    workspace: usize,
) -> maestro_workspace::layout::PaneId {
    let target = state.topology.workspaces[workspace]
        .focused_pane_id()
        .unwrap();
    state
        .topology
        .split_pane_with_runtime(
            workspace,
            maestro_workspace::layout::SplitRequest {
                target,
                direction: maestro_protocol::schema::SplitDirection::Right,
                ratio: Some(0.5),
                focus_new_pane: true,
            },
            |_, _| Ok(()),
        )
        .unwrap()
        .unwrap()
        .0
}

#[test]
fn daemon_last_pane_toggles_to_previous_focus_in_active_tab() {
    let mut state = app_with_workspaces(&["test"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let right = split(&mut state, 0);
    state.focus_pane_in_workspace(0, root);
    state.focus_pane_in_workspace(0, right);
    state.last_pane();
    assert_eq!(state.topology.workspaces[0].focused_pane_id(), Some(root));
    state.last_pane();
    assert_eq!(state.topology.workspaces[0].focused_pane_id(), Some(right));
}

#[test]
fn daemon_removing_background_pane_preserves_last_pane_history() {
    let mut state = app_with_workspaces(&["test"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let right = split(&mut state, 0);
    let background = split(&mut state, 0);
    state.focus_pane_in_workspace(0, root);
    state.focus_pane_in_workspace(0, right);
    state.topology.workspaces[0].remove_pane(background);
    state.last_pane();
    assert_eq!(state.topology.workspaces[0].focused_pane_id(), Some(root));
}

/// Creates a second tab through the sole topology allocator.
fn add_tab(
    state: &mut DaemonState<fn() -> u128>,
    workspace: usize,
) -> maestro_workspace::layout::PaneId {
    state
        .topology
        .create_tab_with_runtime(workspace, |_, _| Ok(()))
        .unwrap()
        .unwrap()
        .0
}

#[test]
fn daemon_last_pane_jumps_across_workspaces_and_tabs() {
    let mut state = app_with_workspaces(&["one", "two"]);
    let first_root = state.topology.workspaces[0].tabs[0].root_pane;
    let second_root = add_tab(&mut state, 1);
    state.focus_pane_in_workspace(0, first_root);
    state.focus_pane_in_workspace(1, second_root);
    state.last_pane();
    assert_eq!(state.active, Some(0));
    assert_eq!(state.topology.workspaces[0].active_tab, 0);
    assert_eq!(
        state.topology.workspaces[0].focused_pane_id(),
        Some(first_root)
    );
    state.last_pane();
    assert_eq!(state.active, Some(1));
    assert_eq!(state.topology.workspaces[1].active_tab, 1);
    assert_eq!(
        state.topology.workspaces[1].focused_pane_id(),
        Some(second_root)
    );
}

#[test]
fn daemon_last_pane_tracks_tab_and_workspace_switches() {
    let mut state = app_with_workspaces(&["one", "two"]);
    let first_root = state.topology.workspaces[0].tabs[0].root_pane;
    let first_second_root = add_tab(&mut state, 0);
    let second_root = state.topology.workspaces[1].tabs[0].root_pane;
    state.switch_workspace_tab(0, 0);
    state.switch_workspace_tab(0, 1);
    state.last_pane();
    assert_eq!(state.active, Some(0));
    assert_eq!(state.topology.workspaces[0].active_tab, 0);
    assert_eq!(
        state.topology.workspaces[0].focused_pane_id(),
        Some(first_root)
    );
    state.last_pane();
    assert_eq!(state.active, Some(0));
    assert_eq!(state.topology.workspaces[0].active_tab, 1);
    assert_eq!(
        state.topology.workspaces[0].focused_pane_id(),
        Some(first_second_root)
    );
    state.switch_workspace(1);
    state.last_pane();
    assert_eq!(state.active, Some(0));
    assert_eq!(state.topology.workspaces[0].active_tab, 1);
    assert_eq!(
        state.topology.workspaces[0].focused_pane_id(),
        Some(first_second_root)
    );
    state.last_pane();
    assert_eq!(state.active, Some(1));
    assert_eq!(
        state.topology.workspaces[1].focused_pane_id(),
        Some(second_root)
    );
}

#[test]
fn daemon_last_pane_tracks_cross_workspace_tab_selection() {
    let mut state = app_with_workspaces(&["one", "two"]);
    let first_root = state.topology.workspaces[0].tabs[0].root_pane;
    let second_first_root = state.topology.workspaces[1].tabs[0].root_pane;
    let second_root = add_tab(&mut state, 1);
    state.switch_workspace_tab(1, 1);
    state.last_pane();
    assert_eq!(state.active, Some(0));
    assert_eq!(
        state.topology.workspaces[0].focused_pane_id(),
        Some(first_root)
    );
    state.last_pane();
    assert_eq!(state.active, Some(1));
    assert_eq!(state.topology.workspaces[1].active_tab, 1);
    assert_eq!(
        state.topology.workspaces[1].focused_pane_id(),
        Some(second_root)
    );
    assert_ne!(second_first_root, second_root);
}

#[test]
fn daemon_switch_workspace_marks_panes_seen() {
    let mut state = app_with_workspaces(&["a", "b"]);
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    state.topology.workspaces[1]
        .pane_state_mut(pane)
        .unwrap()
        .seen = false;
    state.switch_workspace(1);
    assert!(state.topology.workspaces[1].pane_state(pane).unwrap().seen);
}

#[test]
fn daemon_switch_workspace_out_of_bounds_is_noop() {
    let mut state = app_with_workspaces(&["a"]);
    state.switch_workspace(5);
    assert_eq!(state.active, Some(0));
    assert_eq!(state.selected, 0);
    assert!(!state.session_dirty);
}

#[test]
fn focus_navigation_retains_copy_pane_but_clears_cross_pane_selection() {
    use maestro_daemon::{
        app::state::{CopyModeSelection, CopyModeState, Mode},
        selection::Selection,
    };
    let mut state = app_with_workspaces(&["one", "two"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    let other = state.topology.workspaces[1].tabs[0].root_pane;
    state.mode = Mode::Copy;
    state.copy_mode = Some(CopyModeState {
        pane_id: root,
        selection: Some(CopyModeSelection::Character),
    });
    state.selection = Some(Selection::range(root, 0, 1, 3, None));
    state.switch_workspace(0);
    assert_eq!(state.mode, Mode::Copy);
    assert!(state.selection.is_some());
    assert!(state.focus_pane_in_workspace(1, other));
    assert_eq!(state.mode, Mode::Terminal);
    assert!(state.selection.is_none());
    let copy = state.copy_mode.as_ref().unwrap();
    assert_eq!(copy.pane_id, root);
    assert_eq!(copy.selection, None);
    state.last_pane();
    assert_eq!(state.mode, Mode::Copy);
    assert_eq!(state.topology.workspaces[0].focused_pane_id(), Some(root));
}

#[test]
fn daemon_close_workspace_adjusts_indices() {
    let mut state = app_with_workspaces(&["a", "b", "c"]);
    state.selected = 1;
    state.active = Some(1);
    state.close_selected_workspace();
    assert_eq!(state.topology.workspaces.len(), 2);
    assert_eq!(state.selected, 1);
    assert_eq!(state.active, Some(1));
    assert_eq!(state.topology.workspaces[1].display_name(None), "c");
}

#[test]
fn daemon_close_parent_worktree_workspace_closes_group() {
    use maestro_workspace::workspace::WorktreeSpaceMembership;
    let mut state = app_with_workspaces(&["main", "issue", "notes"]);
    for (index, linked) in [(0, false), (1, true)] {
        state.topology.workspaces[index].worktree_space = Some(WorktreeSpaceMembership {
            key: "repo-key".into(),
            label: "project".into(),
            repo_root: "/repo/project".into(),
            checkout_path: if linked {
                "/repo/issue"
            } else {
                "/repo/project"
            }
            .into(),
            is_linked_worktree: linked,
        });
    }
    state.close_selected_workspace();
    assert_eq!(state.topology.workspaces.len(), 1);
    assert_eq!(state.topology.workspaces[0].display_name(None), "notes");
    assert_eq!(state.active, Some(0));
    assert_eq!(state.selected, 0);
}

#[test]
fn daemon_close_last_workspace_clears_active() {
    let mut state = app_with_workspaces(&["only"]);
    state.close_selected_workspace();
    assert!(state.topology.workspaces.is_empty());
    assert_eq!(state.active, None);
    assert_eq!(state.selected, 0);
}

#[test]
fn daemon_close_workspace_at_end_adjusts_selected() {
    let mut state = app_with_workspaces(&["a", "b"]);
    state.selected = 1;
    state.active = Some(1);
    state.close_selected_workspace();
    assert_eq!(state.topology.workspaces.len(), 1);
    assert_eq!(state.selected, 0);
    assert_eq!(state.active, Some(0));
}

#[test]
fn daemon_close_non_focused_workspace_keeps_focus() {
    let mut state = app_with_workspaces(&["a", "b", "c"]);
    state.selected = 1;
    state.active = Some(0);
    state.close_selected_workspace();
    assert_eq!(state.topology.workspaces.len(), 2);
    assert_eq!(names(&state), ["a", "c"]);
    assert_eq!(state.selected, 0);
    assert_eq!(state.active, Some(0));
    assert_identity_invariants(&state);
}

#[test]
fn daemon_close_workspace_removes_unattached_terminal_states() {
    use maestro_daemon::app::state::PluginPaneRecord;
    let mut state = app_with_workspaces(&["one", "two"]);
    let pane = state.topology.workspaces[0].tabs[0].root_pane;
    let terminal = state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    state.plugin_panes.insert(
        pane,
        PluginPaneRecord {
            plugin_id: "example.pane".into(),
            entrypoint: "board".into(),
        },
    );
    state.close_selected_workspace();
    assert!(!state.terminals.contains_key(&terminal));
    assert!(!state.plugin_panes.contains_key(&pane));
    assert_identity_invariants(&state);
}

#[test]
fn daemon_pane_died_last_pane_removes_workspace() {
    let mut state = app_with_workspaces(&["a", "b"]);
    let pane = state.topology.workspaces[0].tabs[0].root_pane;
    state.handle_pane_died(pane);
    assert_eq!(state.topology.workspaces.len(), 1);
    assert_eq!(state.topology.workspaces[0].display_name(None), "b");
    assert_identity_invariants(&state);
}

#[test]
fn daemon_pane_died_last_workspace_enters_navigate() {
    use maestro_daemon::app::state::Mode;
    let mut state = app_with_workspaces(&["only"]);
    state.mode = Mode::Terminal;
    let pane = state.topology.workspaces[0].tabs[0].root_pane;
    state.handle_pane_died(pane);
    assert!(state.topology.workspaces.is_empty());
    assert_eq!(state.mode, Mode::Navigate);
    assert_identity_invariants(&state);
}

#[test]
fn daemon_pane_died_multi_pane_keeps_workspace() {
    let mut state = app_with_workspaces(&["test"]);
    let second = split(&mut state, 0);
    populate_terminals(&mut state);
    state.handle_pane_died(second);
    assert_eq!(state.topology.workspaces.len(), 1);
    assert_eq!(state.topology.workspaces[0].tabs[0].panes.len(), 1);
    assert_identity_invariants(&state);
}

#[test]
fn daemon_pane_died_unknown_pane_is_noop() {
    let mut state = app_with_workspaces(&["test"]);
    state.handle_pane_died(maestro_workspace::layout::PaneId::from_raw(9999));
    assert_eq!(state.topology.workspaces.len(), 1);
    assert_identity_invariants(&state);
}

#[test]
fn pane_death_keeps_logical_focus_after_earlier_workspace_removal() {
    let names: Vec<_> = (0..20).map(|index| format!("ws{index:02}")).collect();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    let mut state = app_with_workspaces(&names);
    state.selected = 1;
    state.active = Some(1);
    let pane = state.topology.workspaces[0].tabs[0].root_pane;
    state.handle_pane_died(pane);
    assert_eq!(state.topology.workspaces.len(), 19);
    assert_eq!(state.topology.workspaces[0].display_name(None), "ws01");
    assert_eq!(state.selected, 0);
    assert_eq!(state.active, Some(0));
    assert_identity_invariants(&state);
}

#[test]
fn pane_death_clears_only_its_selection_and_retires_detached_metadata() {
    use maestro_daemon::{
        app::state::{SelectionAutoscroll, SelectionAutoscrollDirection},
        selection::Selection,
    };
    let mut state = app_with_workspaces(&["active", "background"]);
    let active = state.topology.workspaces[0].tabs[0].root_pane;
    let background = state.topology.workspaces[1].tabs[0].root_pane;
    state.selection = Some(Selection::anchor(active, 0, 0, None));
    state.selection_autoscroll = Some(SelectionAutoscroll {
        direction: SelectionAutoscrollDirection::Down,
        last_mouse_screen_col: 0,
        last_mouse_screen_row: 23,
        inner_rect: maestro_workspace::layout::Rect::new(0, 0, 80, 24),
    });
    state.pane_id_aliases.insert(99, background);
    state
        .public_pane_id_aliases
        .insert("old".into(), background);
    state.handle_pane_died(background);
    assert!(state.selection.is_some());
    assert!(state.selection_autoscroll.is_some());
    assert!(state.pane_id_aliases.is_empty());
    assert!(state.public_pane_id_aliases.is_empty());
    assert_eq!(state.terminal_runtime_shutdowns.len(), 1);
    state.handle_pane_died(active);
    assert!(state.selection.is_none());
    assert!(state.selection_autoscroll.is_none());
    assert!(state.terminals.is_empty());
    assert_eq!(state.terminal_runtime_shutdowns.len(), 2);
    assert_identity_invariants(&state);
}
