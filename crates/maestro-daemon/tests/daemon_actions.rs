//! Workspace reordering preserves logical navigation selection.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::support;
use support::{assert_identity_invariants, populate_terminals};

use maestro_daemon::{app::state::DaemonState, settings::model::Config};

/// Creates pure workspace topology with an active first workspace.
fn app_with_workspaces(names: &[&str]) -> DaemonState<fn() -> u128> {
    let mut state = DaemonState::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        support::manifest_cache(),
    );
    for name in names {
        state
            .topology
            .new_with_tab(format!("/{name}").into(), None, |_, _| Ok(()))
            .unwrap();
    }
    populate_terminals(&mut state);
    state.active = (!names.is_empty()).then_some(0);
    state.toast_config.delay_seconds = 0;
    state.sound.enabled = false;
    if state.active.is_some() {
        state.mode = maestro_daemon::app::state::Mode::Terminal;
    }
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
    let mut state = app_with_workspaces(&["main", "issue", "notes"]);
    mark_worktree(&mut state, 0, false);
    mark_worktree(&mut state, 1, true);
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

#[test]
fn daemon_state_changed_updates_pane() {
    use maestro_daemon::events::AppEvent;
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["test"]);
    let pane = state.topology.workspaces[0].tabs[0].root_pane;
    state.handle_app_event(AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Pi),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });
    let terminal = &state.terminals[state.topology.workspaces[0].terminal_id(pane).unwrap()];
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(terminal.detected_agent, Some(Agent::Pi));
}

#[test]
fn daemon_state_changed_idle_in_background_marks_unseen() {
    use maestro_daemon::{app::state::ToastKind, events::AppEvent, settings::model::ToastDelivery};
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active", "background"]);
    state.toast_config.delivery = ToastDelivery::Workspace;
    state.toast_config.delay_seconds = 0;
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    let terminal = state.topology.workspaces[1]
        .terminal_id(pane)
        .unwrap()
        .clone();
    state.terminals.get_mut(&terminal).unwrap().state = AgentState::Working;
    state.handle_app_event(AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Pi),
        state: AgentState::Idle,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });
    assert!(!state.topology.workspaces[1].pane_state(pane).unwrap().seen);
    assert_eq!(
        state.toast.as_ref().map(|toast| toast.kind),
        Some(ToastKind::Finished)
    );
}

/// Reports the same screen-state envelope used by the background detector.
fn report_state(
    state: &mut DaemonState<fn() -> u128>,
    pane: maestro_workspace::layout::PaneId,
    next: maestro_detection::agents::AgentState,
) -> Vec<maestro_daemon::app::actions::PaneStateUpdate> {
    report_detected(
        state,
        pane,
        maestro_daemon::terminal::state::DetectedStateUpdate {
            agent: Some(maestro_detection::agents::Agent::Pi),
            fallback_state: next,
            visible: maestro_detection::agents::AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: false,
        },
    )
}

#[test]
fn daemon_active_tab_completion_marks_pane_seen() {
    use maestro_detection::agents::AgentState;
    let mut state = app_with_workspaces(&["active"]);
    state.outer_terminal_focus = Some(true);
    let pane = state.topology.workspaces[0].tabs[0].root_pane;
    let terminal = state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    state.terminals.get_mut(&terminal).unwrap().state = AgentState::Working;
    state.topology.workspaces[0]
        .pane_state_mut(pane)
        .unwrap()
        .seen = false;
    report_state(&mut state, pane, AgentState::Idle);
    assert_eq!(state.terminals[&terminal].state, AgentState::Idle);
    assert!(state.topology.workspaces[0].pane_state(pane).unwrap().seen);
}

#[test]
fn daemon_initial_idle_in_background_stays_seen() {
    let mut state = app_with_workspaces(&["active", "background"]);
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    report_state(
        &mut state,
        pane,
        maestro_detection::agents::AgentState::Idle,
    );
    assert!(state.topology.workspaces[1].pane_state(pane).unwrap().seen);
}

#[test]
fn daemon_idle_after_known_unknown_agent_in_background_marks_done() {
    use maestro_detection::agents::AgentState;
    let mut state = app_with_workspaces(&["active", "background"]);
    state.toast_config.delay_seconds = 0;
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    report_state(&mut state, pane, AgentState::Unknown);
    report_state(&mut state, pane, AgentState::Idle);
    assert!(!state.topology.workspaces[1].pane_state(pane).unwrap().seen);
}

#[test]
fn daemon_waiting_sound_plays_even_in_active_workspace() {
    use maestro_daemon::app::actions::notification_sound_for_state_change;
    use maestro_detection::agents::AgentState;
    assert_eq!(
        notification_sound_for_state_change(true, AgentState::Working, AgentState::Blocked),
        Some(maestro_protocol::schema::NotificationShowSound::Request)
    );
}

#[test]
fn daemon_done_sound_only_plays_in_background() {
    use maestro_daemon::app::actions::notification_sound_for_state_change;
    use maestro_detection::agents::AgentState;
    assert_eq!(
        notification_sound_for_state_change(false, AgentState::Working, AgentState::Idle),
        Some(maestro_protocol::schema::NotificationShowSound::Done)
    );
    assert_eq!(
        notification_sound_for_state_change(true, AgentState::Working, AgentState::Idle),
        None
    );
    assert_eq!(
        notification_sound_for_state_change(false, AgentState::Unknown, AgentState::Idle),
        None
    );
}

#[test]
fn daemon_background_waiting_sets_attention_toast() {
    use maestro_daemon::{app::state::ToastKind, settings::model::ToastDelivery};
    let mut state = app_with_workspaces(&["active", "background"]);
    state.toast_config.delivery = ToastDelivery::Workspace;
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    report_state(
        &mut state,
        pane,
        maestro_detection::agents::AgentState::Blocked,
    );
    let toast = state.toast.as_ref().unwrap();
    assert_eq!(toast.kind, ToastKind::NeedsAttention);
    assert_eq!(toast.title, "pi needs attention");
    assert_eq!(toast.context, "background · 2");
}

#[test]
fn daemon_delayed_background_waiting_schedules_before_toast() {
    use maestro_daemon::{app::state::ToastKind, settings::model::ToastDelivery};
    let mut state = app_with_workspaces(&["active", "background"]);
    state.toast_config.delivery = ToastDelivery::Workspace;
    state.toast_config.delay_seconds = 1;
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    report_state(
        &mut state,
        pane,
        maestro_detection::agents::AgentState::Blocked,
    );
    assert!(state.toast.is_none());
    assert!(state.pending_agent_notifications.contains_key(&pane));
    let deadline = state.next_pending_agent_notification_deadline().unwrap();
    let deliveries = state.drain_due_agent_notifications(deadline);
    assert_eq!(deliveries.len(), 1);
    let toast = state.toast.as_ref().unwrap();
    assert_eq!(toast.kind, ToastKind::NeedsAttention);
    assert_eq!(toast.title, "pi needs attention");
    assert_eq!(toast.context, "background · 2");
    assert!(state.pending_agent_notifications.is_empty());
}

#[test]
fn daemon_delayed_background_waiting_cancels_when_agent_resumes_working() {
    use maestro_daemon::settings::model::ToastDelivery;
    use maestro_detection::agents::AgentState;
    let mut state = app_with_workspaces(&["active", "background"]);
    state.toast_config.delivery = ToastDelivery::Workspace;
    state.toast_config.delay_seconds = 1;
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    report_state(&mut state, pane, AgentState::Blocked);
    let deadline = state.next_pending_agent_notification_deadline().unwrap();
    state.handle_app_event(maestro_daemon::events::AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(maestro_detection::agents::Agent::Pi),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: true,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });
    assert!(state.pending_agent_notifications.is_empty());
    assert!(state.drain_due_agent_notifications(deadline).is_empty());
    assert!(state.toast.is_none());
}

#[test]
fn daemon_delayed_background_waiting_is_suppressed_if_pane_becomes_active() {
    use maestro_daemon::settings::model::ToastDelivery;
    let mut state = app_with_workspaces(&["active", "background"]);
    state.toast_config.delivery = ToastDelivery::Workspace;
    state.toast_config.delay_seconds = 1;
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    report_state(
        &mut state,
        pane,
        maestro_detection::agents::AgentState::Blocked,
    );
    let deadline = state.next_pending_agent_notification_deadline().unwrap();
    state.active = Some(1);
    assert!(state.drain_due_agent_notifications(deadline).is_empty());
    assert!(state.toast.is_none());
}

#[test]
fn daemon_delayed_active_tab_unfocused_keeps_client_notification_available() {
    use maestro_daemon::settings::model::ToastDelivery;
    let mut state = app_with_workspaces(&["active"]);
    state.outer_terminal_focus = Some(false);
    state.toast_config.delivery = ToastDelivery::System;
    state.toast_config.delay_seconds = 1;
    let pane = state.topology.workspaces[0].tabs[0].root_pane;
    report_state(
        &mut state,
        pane,
        maestro_detection::agents::AgentState::Blocked,
    );
    let deadline = state.next_pending_agent_notification_deadline().unwrap();
    let deliveries = state.drain_due_agent_notifications(deadline);
    assert_eq!(deliveries.len(), 1);
    assert!(deliveries[0].toast.is_none());
    assert!(deliveries[0].client_notification.is_some());
    assert!(state.toast.is_none());
}

#[test]
fn daemon_delayed_background_waiting_is_cleared_when_pane_dies() {
    use maestro_daemon::settings::model::ToastDelivery;
    let mut state = app_with_workspaces(&["active", "background"]);
    state.toast_config.delivery = ToastDelivery::Workspace;
    state.toast_config.delay_seconds = 1;
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    report_state(
        &mut state,
        pane,
        maestro_detection::agents::AgentState::Blocked,
    );
    let deadline = state.next_pending_agent_notification_deadline().unwrap();
    state.handle_app_event(maestro_daemon::events::AppEvent::PaneDied { pane_id: pane });
    assert!(state.pending_agent_notifications.is_empty());
    assert!(state.drain_due_agent_notifications(deadline).is_empty());
    assert!(state.toast.is_none());
}

/// Feeds supplied screen signals through the same event consumer as detection.
fn report_detected(
    state: &mut DaemonState<fn() -> u128>,
    pane: maestro_workspace::layout::PaneId,
    update: maestro_daemon::terminal::state::DetectedStateUpdate,
) -> Vec<maestro_daemon::app::actions::PaneStateUpdate> {
    state.handle_app_event(maestro_daemon::events::AppEvent::StateChanged {
        pane_id: pane,
        agent: update.agent,
        state: update.fallback_state,
        visible_blocker: update.visible.visible_blocker,
        visible_working: update.visible.visible_working,
        process_exited: update.process_exited,
        observed_at: std::time::Instant::now(),
    })
}

/// Reports process acquisition before any screen-state event.
fn detect_agent(
    state: &mut DaemonState<fn() -> u128>,
    pane: maestro_workspace::layout::PaneId,
    agent: maestro_detection::agents::Agent,
) {
    state.handle_app_event(maestro_daemon::events::AppEvent::AgentProcessDetected {
        pane_id: pane,
        agent,
        observed_at: std::time::Instant::now(),
    });
}

#[test]
fn daemon_first_idle_after_process_detection_is_not_completion() {
    use maestro_daemon::app::{actions::CompletionPolicy, state::ToastKind};
    use maestro_detection::agents::{Agent, AgentState, AgentVisibility};
    let mut state = app_with_workspaces(&["active", "background"]);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    detect_agent(&mut state, pane, Agent::Pi);
    let direct = report_state(&mut state, pane, AgentState::Idle)
        .pop()
        .unwrap();
    assert_eq!(direct.completion, CompletionPolicy::Suppress);
    detect_agent(&mut state, pane, Agent::Pi);
    for next in [AgentState::Working, AgentState::Blocked] {
        report_detected(
            &mut state,
            pane,
            maestro_daemon::terminal::state::DetectedStateUpdate {
                agent: Some(Agent::Pi),
                fallback_state: next,
                process_exited: false,
                visible: AgentVisibility {
                    visible_blocker: next == AgentState::Blocked,
                    visible_working: next == AgentState::Working,
                    visible_idle: false,
                },
            },
        );
    }
    let update = report_state(&mut state, pane, AgentState::Idle)
        .pop()
        .unwrap();
    assert_eq!(update.completion, CompletionPolicy::Suppress);
    assert!(state.topology.workspaces[1].pane_state(pane).unwrap().seen);
    assert_ne!(
        state.toast.as_ref().map(|toast| toast.kind),
        Some(ToastKind::Finished)
    );
    assert_acquired_process_exit_is_completion(&mut state, pane);
}

/// Acquisition suppression ends when the actual detected process exits.
fn assert_acquired_process_exit_is_completion(
    state: &mut DaemonState<fn() -> u128>,
    pane: maestro_workspace::layout::PaneId,
) {
    use maestro_daemon::{app::actions::CompletionPolicy, terminal::state::DetectedStateUpdate};
    use maestro_detection::agents::{Agent, AgentState, AgentVisibility};
    detect_agent(state, pane, Agent::Codex);
    for next in [AgentState::Working, AgentState::Idle] {
        let update = report_detected(
            state,
            pane,
            DetectedStateUpdate {
                agent: Some(Agent::Codex),
                fallback_state: next,
                process_exited: next == AgentState::Idle,
                visible: AgentVisibility {
                    visible_blocker: false,
                    visible_working: next == AgentState::Working,
                    visible_idle: false,
                },
            },
        );
        if next == AgentState::Idle {
            assert_eq!(update.last().unwrap().completion, CompletionPolicy::Notify);
        }
    }
}

#[test]
fn delayed_notifications_recheck_live_context_and_select_independent_outputs() {
    use maestro_daemon::settings::model::ToastDelivery;
    let mut state = app_with_workspaces(&["active", "old"]);
    state.toast_config.delivery = ToastDelivery::Terminal;
    state.toast_config.delay_seconds = 1;
    state.sound.enabled = true;
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    report_state(
        &mut state,
        pane,
        maestro_detection::agents::AgentState::Blocked,
    );
    let deadline = state.next_pending_agent_notification_deadline().unwrap();
    let terminal = state.topology.workspaces[1]
        .terminal_id(pane)
        .unwrap()
        .clone();
    state.terminals.get_mut(&terminal).unwrap().cwd = "/changed".into();
    let deliveries = state.drain_due_agent_notifications(deadline);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].toast.as_ref().unwrap().context, "changed · 2");
    assert_eq!(
        deliveries[0].client_notification.as_ref().unwrap().context,
        "changed · 2"
    );
    assert_eq!(
        deliveries[0].sound,
        Some(maestro_protocol::schema::NotificationShowSound::Request)
    );
    assert!(state.toast.is_none());
}

#[test]
fn daemon_hook_reported_unknown_agent_sets_toast_title_from_label() {
    use maestro_daemon::{app::state::ToastKind, events::AppEvent};
    use maestro_detection::agents::AgentState;
    let mut state = app_with_workspaces(&["active", "background"]);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    let pane_id = state.topology.workspaces[1].tabs[0].root_pane;
    state.handle_app_event(AppEvent::HookStateReported {
        pane_id,
        source: "custom:hermes".into(),
        agent_label: "hermes".into(),
        state: AgentState::Blocked,
        message: None,
        seq: None,
        session_ref: None,
    });
    let toast = state.toast.as_ref().unwrap();
    assert_eq!(toast.kind, ToastKind::NeedsAttention);
    assert_eq!(toast.title, "hermes needs attention");
    assert_eq!(toast.context, "background · 2");
}

#[test]
fn daemon_visible_blocker_overrides_hook_working_and_notifies() {
    use maestro_daemon::{app::state::ToastKind, events::AppEvent};
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active", "background"]);
    state.active = Some(0);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    let bg_pane_id = state.topology.workspaces[1].tabs[0].root_pane;
    let bg_terminal_id = state.topology.workspaces[1]
        .pane_state(bg_pane_id)
        .unwrap()
        .attached_terminal_id
        .clone();

    state.handle_app_event(AppEvent::StateChanged {
        pane_id: bg_pane_id,
        agent: Some(Agent::Codex),
        state: AgentState::Idle,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });
    state.handle_app_event(AppEvent::HookStateReported {
        pane_id: bg_pane_id,
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Working,
        message: None,
        seq: Some(1),
        session_ref: None,
    });
    state.handle_app_event(AppEvent::StateChanged {
        pane_id: bg_pane_id,
        agent: Some(Agent::Codex),
        state: AgentState::Blocked,
        visible_blocker: true,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });

    let terminal = state.terminals.get(&bg_terminal_id).unwrap();
    assert_eq!(terminal.state, AgentState::Blocked);
    let toast = state.toast.as_ref().unwrap();
    assert_eq!(toast.kind, ToastKind::NeedsAttention);
    assert_eq!(toast.title, "codex needs attention");
}

#[test]
fn daemon_reserved_native_state_report_does_not_override_screen_state() {
    use maestro_daemon::events::AppEvent;
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active"]);
    state.active = Some(0);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    let pane_id = state.topology.workspaces[0].tabs[0].root_pane;
    let terminal_id = state.topology.workspaces[0]
        .pane_state(pane_id)
        .unwrap()
        .attached_terminal_id
        .clone();

    state.handle_app_event(AppEvent::StateChanged {
        pane_id,
        agent: Some(Agent::Claude),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });
    state.handle_app_event(AppEvent::HookStateReported {
        pane_id,
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Blocked,
        message: None,
        seq: Some(1),
        session_ref: maestro_daemon::agent_resume::AgentSessionRef::id("claude-session"),
    });
    let terminal = state.terminals.get(&terminal_id).unwrap();
    assert_eq!(terminal.state, AgentState::Working);
    assert!(terminal.hook_authority.is_none());
    assert!(terminal.persisted_agent_session.is_some());

    state.handle_app_event(AppEvent::StateChanged {
        pane_id,
        agent: Some(Agent::Claude),
        state: AgentState::Idle,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });

    let terminal = state.terminals.get(&terminal_id).unwrap();
    assert_eq!(terminal.state, AgentState::Idle);
    assert!(state.toast.is_none());
}

#[test]
fn daemon_official_release_preserves_process_owned_agent_identity() {
    use maestro_daemon::events::AppEvent;
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active"]);
    let pane_id = state.topology.workspaces[0].tabs[0].root_pane;
    let terminal_id = state.topology.workspaces[0]
        .pane_state(pane_id)
        .unwrap()
        .attached_terminal_id
        .clone();

    state.handle_app_event(AppEvent::StateChanged {
        pane_id,
        agent: Some(Agent::Pi),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: true,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });
    let terminal = state.terminals.get_mut(&terminal_id).unwrap();
    terminal.set_persisted_agent_session(maestro_daemon::agent_resume::PersistedAgentSession {
        source: "maestro:pi".into(),
        agent: "pi".into(),
        session_ref: maestro_daemon::agent_resume::AgentSessionRef::path(
            std::env::current_dir()
                .unwrap()
                .join("release-session.jsonl")
                .display()
                .to_string(),
        )
        .unwrap(),
    });
    terminal.set_hook_authority_with_session_ref(maestro_daemon::terminal::state::HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: Some(1),
    });
    terminal.set_agent_name("reviewer".into());
    state.session_dirty = false;

    let updates = state.handle_app_event(AppEvent::HookAgentReleased {
        pane_id,
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        known_agent: Some(Agent::Pi),
        seq: Some(2),
    });

    assert!(updates.is_empty());
    let terminal = &state.terminals[&terminal_id];
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(terminal.detected_agent, Some(Agent::Pi));
    assert_eq!(terminal.agent_name.as_deref(), Some("reviewer"));
    assert!(terminal.full_lifecycle_hook_authority_active());
    assert!(!state.session_dirty);
}

#[test]
fn daemon_session_metadata_report_refreshes_session_without_overriding_screen_state() {
    use maestro_daemon::events::AppEvent;
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active"]);
    let pane_id = state.topology.workspaces[0].tabs[0].root_pane;
    let terminal_id = state.topology.workspaces[0]
        .pane_state(pane_id)
        .unwrap()
        .attached_terminal_id
        .clone();

    state.handle_app_event(AppEvent::StateChanged {
        pane_id,
        agent: Some(Agent::Devin),
        state: AgentState::Idle,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });
    state.handle_app_event(AppEvent::HookStateReported {
        pane_id,
        source: "maestro:devin".into(),
        agent_label: "devin".into(),
        state: AgentState::Working,
        message: None,
        seq: Some(1),
        session_ref: maestro_daemon::agent_resume::AgentSessionRef::id("devin-session"),
    });

    let terminal = state.terminals.get(&terminal_id).unwrap();
    assert_eq!(terminal.state, AgentState::Idle);
    assert!(terminal.hook_authority.is_none());
    assert!(terminal.persisted_agent_session.is_some());
}

#[test]
fn daemon_hidden_custom_session_ref_only_update_marks_session_dirty_without_visible_update() {
    use maestro_daemon::events::AppEvent;
    use maestro_detection::agents::AgentState;
    let mut state = app_with_workspaces(&["active"]);
    let pane_id = state.topology.workspaces[0].tabs[0].root_pane;
    let test_dir = std::env::current_dir().unwrap();
    let first_session = test_dir.join("one.jsonl").display().to_string();
    let second_session = test_dir.join("two.jsonl").display().to_string();

    let first_updates = state.handle_app_event(AppEvent::HookStateReported {
        pane_id,
        source: "custom:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        seq: Some(20),
        session_ref: maestro_daemon::agent_resume::AgentSessionRef::path(first_session),
    });
    assert_eq!(first_updates.len(), 1);
    state.session_dirty = false;

    let second_updates = state.handle_app_event(AppEvent::HookStateReported {
        pane_id,
        source: "custom:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        seq: Some(21),
        session_ref: maestro_daemon::agent_resume::AgentSessionRef::path(second_session),
    });

    assert!(second_updates.is_empty());
    assert!(state.session_dirty);
}

#[test]
fn daemon_custom_release_clears_report_owned_agent() {
    use maestro_daemon::events::AppEvent;
    use maestro_detection::agents::AgentState;
    let mut state = app_with_workspaces(&["active"]);
    let pane_id = state.topology.workspaces[0].tabs[0].root_pane;
    let terminal_id = state.topology.workspaces[0]
        .pane_state(pane_id)
        .unwrap()
        .attached_terminal_id
        .clone();
    state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .set_hook_authority_with_session_ref(maestro_daemon::terminal::state::HookReport {
            source: "custom:agent".into(),
            agent_label: "custom-agent".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(1),
        });

    state.handle_app_event(AppEvent::HookAgentReleased {
        pane_id,
        source: "custom:agent".into(),
        agent_label: "custom-agent".into(),
        known_agent: None,
        seq: Some(2),
    });

    let terminal = &state.terminals[&terminal_id];
    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.state, AgentState::Unknown);
}

#[test]
fn daemon_terminal_cwd_report_updates_terminal_cwd_and_marks_session_dirty() {
    use maestro_daemon::events::AppEvent;
    let mut state = app_with_workspaces(&["active"]);
    let pane_id = state.topology.workspaces[0].tabs[0].root_pane;
    let terminal_id = state.topology.workspaces[0]
        .pane_state(pane_id)
        .unwrap()
        .attached_terminal_id
        .clone();
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let cwd = directory.path().to_path_buf();
    state.session_dirty = false;

    let updates = state.handle_app_event(AppEvent::TerminalCwdReported {
        pane_id,
        cwd: cwd.clone(),
    });

    assert!(updates.is_empty());
    assert_eq!(state.terminals.get(&terminal_id).unwrap().cwd, cwd);
    assert!(state.session_dirty);
    let _ = std::fs::remove_dir_all(cwd);
}

#[test]
fn daemon_background_idle_sets_finished_toast() {
    use maestro_daemon::{app::state::ToastKind, events::AppEvent};
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active", "background"]);
    state.active = Some(0);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    let bg_pane_id = state.topology.workspaces[1].tabs[0].root_pane;
    let bg_terminal_id = state.topology.workspaces[1]
        .pane_state(bg_pane_id)
        .unwrap()
        .attached_terminal_id
        .clone();
    state.terminals.get_mut(&bg_terminal_id).unwrap().state = AgentState::Working;

    state.handle_app_event(AppEvent::StateChanged {
        pane_id: bg_pane_id,
        agent: Some(Agent::Droid),
        state: AgentState::Idle,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });

    let toast = state.toast.as_ref().unwrap();
    assert_eq!(toast.kind, ToastKind::Finished);
    assert_eq!(toast.title, "droid finished");
    assert_eq!(toast.context, "background · 2");
    let target = toast.target.as_ref().expect("toast target");
    assert_eq!(&target.workspace_id, &state.topology.workspaces[1].id);
    assert_eq!(target.pane_id, bg_pane_id);
}

#[test]
fn daemon_background_toast_includes_tab_name_when_workspace_has_multiple_tabs() {
    use maestro_daemon::{app::state::ToastKind, events::AppEvent};
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active", "background"]);
    state.active = Some(0);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    state.topology.workspaces[1].tabs[0].set_custom_name("main".into());
    let bg_pane_id = add_tab(&mut state, 1);
    populate_terminals(&mut state);
    let tab = state.topology.workspaces[1].tabs.last_mut().unwrap();
    tab.set_custom_name("logs".into());
    state.topology.workspaces[1].active_tab = 0;

    state.handle_app_event(AppEvent::StateChanged {
        pane_id: bg_pane_id,
        agent: Some(Agent::Pi),
        state: AgentState::Blocked,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });

    let toast = state.toast.as_ref().unwrap();
    assert_eq!(toast.kind, ToastKind::NeedsAttention);
    assert_eq!(toast.title, "pi needs attention");
    assert_eq!(toast.context, "background · 2 · logs");
}

#[test]
fn daemon_background_tab_in_active_workspace_still_sets_toast() {
    use maestro_daemon::{app::state::ToastKind, events::AppEvent};
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active"]);
    state.active = Some(0);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    state.topology.workspaces[0].tabs[0].set_custom_name("main".into());
    let bg_pane_id = add_tab(&mut state, 0);
    populate_terminals(&mut state);
    let tab = state.topology.workspaces[0].tabs.last_mut().unwrap();
    tab.set_custom_name("logs".into());
    state.topology.workspaces[0].active_tab = 0;

    state.handle_app_event(AppEvent::StateChanged {
        pane_id: bg_pane_id,
        agent: Some(Agent::Pi),
        state: AgentState::Blocked,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });

    let toast = state.toast.as_ref().unwrap();
    assert_eq!(toast.kind, ToastKind::NeedsAttention);
    assert_eq!(toast.title, "pi needs attention");
    assert_eq!(toast.context, "active · 1 · logs");
}

#[test]
fn daemon_active_workspace_active_tab_does_not_set_toast() {
    use maestro_daemon::events::AppEvent;
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active"]);
    state.active = Some(0);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    let pane_id = state.topology.workspaces[0].tabs[0].root_pane;

    state.handle_app_event(AppEvent::StateChanged {
        pane_id,
        agent: Some(Agent::Pi),
        state: AgentState::Blocked,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });

    assert!(state.toast.is_none());
}

#[test]
fn daemon_active_workspace_active_tab_keeps_workspace_toast_suppressed_when_outer_terminal_is_unfocused()
 {
    use maestro_daemon::events::AppEvent;
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active"]);
    state.active = Some(0);
    state.outer_terminal_focus = Some(false);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    let pane_id = state.topology.workspaces[0].tabs[0].root_pane;

    state.handle_app_event(AppEvent::StateChanged {
        pane_id,
        agent: Some(Agent::Pi),
        state: AgentState::Blocked,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });

    assert!(state.toast.is_none());
}

#[test]
fn daemon_active_tab_suppression_preserves_unknown_focus_behavior() {
    use maestro_daemon::app::actions::active_tab_suppresses_notifications;
    assert!(active_tab_suppresses_notifications(true, None));
    assert!(active_tab_suppresses_notifications(true, Some(true)));
    assert!(!active_tab_suppresses_notifications(true, Some(false)));
    assert!(!active_tab_suppresses_notifications(false, None));
}

#[test]
fn daemon_update_ready_sets_manual_update_toast() {
    use maestro_daemon::{app::state::ToastKind, events::AppEvent};
    let mut state = app_with_workspaces(&[]);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;

    let updates = state.handle_app_event(AppEvent::UpdateReady {
        version: "0.5.0".into(),
        install_command: "maestro update".into(),
    });

    assert!(updates.is_empty());
    assert_eq!(state.update_available.as_deref(), Some("0.5.0"));
    assert!(state.latest_release_notes_available);
    assert!(state.update_dismissed);
    let toast = state.toast.as_ref().expect("update toast");
    assert_eq!(toast.kind, ToastKind::UpdateInstalled);
    assert_eq!(toast.title, "v0.5.0 available");
    assert_eq!(
        toast.context,
        "detach, run `maestro update`, then follow its restart guidance"
    );
}

#[test]
fn daemon_update_ready_uses_event_install_command_in_toast() {
    use maestro_daemon::events::AppEvent;
    let mut state = app_with_workspaces(&[]);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;

    state.handle_app_event(AppEvent::UpdateReady {
        version: "0.5.0".into(),
        install_command: "brew update && brew upgrade maestro".into(),
    });

    assert_eq!(
        state.update_install_command,
        "brew update && brew upgrade maestro"
    );
    let toast = state.toast.as_ref().expect("update toast");
    assert_eq!(
        toast.context,
        "detach, run `brew update && brew upgrade maestro`, then restart this Maestro session when ready"
    );
}

#[test]
fn update_guidance_keeps_install_manager_and_delivery_policies_separate() {
    use maestro_daemon::{events::AppEvent, settings::model::ToastDelivery};
    let mut state = app_with_workspaces(&[]);
    state.toast_config.delivery = ToastDelivery::Workspace;
    for (command, expected) in [
        (
            "update through Nix",
            "detach, update through Nix, then restart this Maestro session when ready",
        ),
        (
            "mise upgrade maestro",
            "detach, run `mise upgrade maestro`, then restart this Maestro session when ready",
        ),
        (
            "custom-installer --update",
            "detach, run `custom-installer --update`, then restart this Maestro session when ready",
        ),
    ] {
        assert!(
            state
                .handle_app_event(AppEvent::UpdateReady {
                    version: "0.6.0".into(),
                    install_command: command.into(),
                })
                .is_empty()
        );
        assert_eq!(state.toast.as_ref().unwrap().context, expected);
    }
    let previous = state.toast.clone();
    for delivery in [
        ToastDelivery::Off,
        ToastDelivery::Terminal,
        ToastDelivery::System,
    ] {
        state.toast_config.delivery = delivery;
        state.handle_app_event(AppEvent::UpdateReady {
            version: "0.7.0".into(),
            install_command: "separate-installer".into(),
        });
        assert_eq!(state.update_available.as_deref(), Some("0.7.0"));
        assert_eq!(state.update_install_command, "separate-installer");
        assert_eq!(state.toast, previous);
    }
}

#[test]
fn daemon_agent_detection_manifest_update_event_updates_status_and_toast() {
    use maestro_daemon::{app::state::ToastKind, events::AppEvent};
    use maestro_detection::{
        agents::Agent,
        manifest_update::{ManifestUpdateCommit, ManifestUpdateStatus, ManifestVersion},
    };
    let mut state = app_with_workspaces(&[]);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    let updates = state.handle_app_event(AppEvent::AgentDetectionManifestsUpdated {
        updated: vec![ManifestUpdateCommit {
            agent: Agent::Codex,
            version: ManifestVersion::parse("2026.06.10.1").unwrap(),
        }],
        status: ManifestUpdateStatus {
            last_result: Some("checked".into()),
            ..Default::default()
        },
    });
    assert!(updates.is_empty());
    assert_eq!(
        state.agent_manifest_update_status.last_result.as_deref(),
        Some("checked")
    );
    let toast = state.toast.as_ref().expect("manifest update toast");
    assert_eq!(toast.kind, ToastKind::UpdateInstalled);
    assert_eq!(toast.title, "Agent detection rules updated");
    assert_eq!(toast.context, "codex 2026.06.10.1");
}

#[test]
fn manifest_update_refreshes_the_shared_runtime_snapshot_without_a_toast() {
    use maestro_daemon::{events::AppEvent, settings::model::ToastDelivery};
    use maestro_detection::{
        agents::Agent,
        manifest::{ManifestCache, ManifestSource},
        manifest_update::{ManifestUpdateCommit, ManifestUpdateStatus, ManifestVersion},
    };
    use std::sync::{Arc, RwLock};
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    let cache = Arc::new(RwLock::new(
        ManifestCache::new(Some(root.clone()), None).unwrap(),
    ));
    let mut state = DaemonState::new(&Config::default(), || 0, Arc::clone(&cache));
    state.agent_manifest_summaries.clear();
    assert!(state.agent_manifest_summaries.is_empty());
    state.handle_app_event(AppEvent::AgentDetectionManifestsUpdated {
        updated: Vec::new(),
        status: ManifestUpdateStatus::default(),
    });
    let codex = state
        .agent_manifest_summaries
        .iter()
        .find(|item| item.agent == Agent::Codex)
        .unwrap();
    assert_eq!(codex.active_source, ManifestSource::Bundled);
    std::fs::write(
        root.join("codex.toml"),
        r#"id = "codex"
version = "2026.10.21.1"
[[rules]]
id = "working"
state = "working"
contains = ["controlled working"]
"#,
    )
    .unwrap();
    cache.write().unwrap().reload_manifests().unwrap();
    state.toast_config.delivery = ToastDelivery::Off;
    state.handle_app_event(AppEvent::AgentDetectionManifestsUpdated {
        updated: vec![ManifestUpdateCommit {
            agent: Agent::Codex,
            version: ManifestVersion::parse("2026.10.21.1").unwrap(),
        }],
        status: ManifestUpdateStatus {
            last_result: Some("updated".into()),
            ..Default::default()
        },
    });
    let codex = state
        .agent_manifest_summaries
        .iter()
        .find(|item| item.agent == Agent::Codex)
        .unwrap();
    assert_eq!(
        codex.active_source,
        ManifestSource::Override(root.join("codex.toml"))
    );
    assert_eq!(codex.active_version.as_deref(), Some("2026.10.21.1"));
    assert!(state.toast.is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn daemon_toggle_zoom_works() {
    let mut state = app_with_workspaces(&["test"]);
    let root = state.topology.workspaces[0].tabs[0].root_pane;
    state
        .topology
        .split_pane_with_runtime(
            0,
            maestro_workspace::layout::SplitRequest {
                target: root,
                direction: maestro_protocol::schema::SplitDirection::Right,
                ratio: None,
                focus_new_pane: true,
            },
            |_, _| Ok(()),
        )
        .unwrap()
        .unwrap();
    assert!(!state.topology.workspaces[0].tabs[0].zoomed);
    state.toggle_zoom();
    assert!(state.topology.workspaces[0].tabs[0].zoomed);
    state.toggle_zoom();
    assert!(!state.topology.workspaces[0].tabs[0].zoomed);
}

#[test]
fn daemon_toggle_zoom_single_pane_noop() {
    let mut state = app_with_workspaces(&["test"]);
    state.toggle_zoom();
    assert!(!state.topology.workspaces[0].tabs[0].zoomed);
    assert!(!state.session_dirty);
}

#[test]
fn daemon_close_pane_removes_from_workspace() {
    let mut state = app_with_workspaces(&["test"]);
    let closed = split(&mut state, 0);
    populate_terminals(&mut state);
    assert_eq!(state.topology.workspaces[0].tabs[0].panes.len(), 2);
    state.plugin_panes.insert(
        closed,
        maestro_daemon::app::state::PluginPaneRecord {
            plugin_id: "example.pane".into(),
            entrypoint: "board".into(),
        },
    );
    state.close_pane();
    assert_eq!(state.topology.workspaces[0].tabs[0].panes.len(), 1);
    assert!(!state.plugin_panes.contains_key(&closed));
    assert_identity_invariants(&state);
}

#[test]
fn daemon_pane_process_exit_publish_marks_agent_idle_before_pane_removal() {
    use maestro_daemon::{
        app::{actions::AgentRelease, state::ToastKind},
        events::AppEvent,
    };
    use maestro_detection::agents::{Agent, AgentState};
    let mut state = app_with_workspaces(&["active", "background"]);
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    state.active = Some(1);
    let pane_id = state.topology.workspaces[0].tabs[0].root_pane;
    let terminal_id = state.topology.workspaces[0]
        .terminal_id(pane_id)
        .unwrap()
        .clone();
    state.handle_app_event(AppEvent::StateChanged {
        pane_id,
        agent: Some(Agent::Pi),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: true,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    });
    assert_eq!(state.terminals[&terminal_id].state, AgentState::Working);
    let update = state
        .publish_pane_process_exit_if_agent(pane_id)
        .expect("process exit update");
    assert!(!state.pane_is_in_active_tab(update.ws_idx, pane_id));
    assert_eq!(update.previous_state, AgentState::Working);
    assert_eq!(update.state, AgentState::Idle);
    assert_eq!(update.agent_label.as_deref(), Some("pi"));
    assert_eq!(update.known_agent, Some(Agent::Pi));
    assert_eq!(
        update.release,
        AgentRelease::Released(maestro_protocol::schema::AgentStatus::Done)
    );
    assert!(matches!(
        state.toast.as_ref().map(|toast| toast.kind),
        Some(ToastKind::Finished)
    ));
}

#[test]
fn daemon_close_pane_removes_unattached_terminal_state() {
    let mut state = app_with_workspaces(&["test"]);
    let pane = split(&mut state, 0);
    populate_terminals(&mut state);
    let terminal = state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    state.close_pane();
    assert!(!state.terminals.contains_key(&terminal));
    assert_identity_invariants(&state);
}

#[test]
fn daemon_close_tab_removes_unattached_terminal_states() {
    let mut state = app_with_workspaces(&["test"]);
    let (pane, ()) = state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    populate_terminals(&mut state);
    state.topology.workspaces[0].switch_tab(1);
    let terminal = state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    state.plugin_panes.insert(
        pane,
        maestro_daemon::app::state::PluginPaneRecord {
            plugin_id: "example.pane".into(),
            entrypoint: "board".into(),
        },
    );
    state.close_tab();
    assert!(!state.terminals.contains_key(&terminal));
    assert!(!state.plugin_panes.contains_key(&pane));
    assert_identity_invariants(&state);
}

#[test]
fn daemon_close_tab_closes_active_workspace_not_selected_workspace() {
    let mut state = app_with_workspaces(&["selected", "active"]);
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    let terminal = state.topology.workspaces[1]
        .terminal_id(pane)
        .unwrap()
        .clone();
    state.active = Some(1);
    state.selected = 0;
    state.close_tab();
    assert_eq!(state.topology.workspaces.len(), 1);
    assert_eq!(state.topology.workspaces[0].display_name(None), "selected");
    assert!(!state.terminals.contains_key(&terminal));
    assert_identity_invariants(&state);
}

#[test]
fn daemon_close_pane_last_pane_closes_active_workspace_not_selected_workspace() {
    let mut state = app_with_workspaces(&["selected", "active"]);
    let pane = state.topology.workspaces[1].tabs[0].root_pane;
    let terminal = state.topology.workspaces[1]
        .terminal_id(pane)
        .unwrap()
        .clone();
    state.active = Some(1);
    state.selected = 0;
    state.close_pane();
    assert_eq!(state.topology.workspaces.len(), 1);
    assert_eq!(state.topology.workspaces[0].display_name(None), "selected");
    assert!(!state.terminals.contains_key(&terminal));
    assert_identity_invariants(&state);
}

/// Assigns supplied repository-group membership without invoking later discovery effects.
fn mark_worktree(state: &mut DaemonState<fn() -> u128>, index: usize, linked: bool) {
    state.topology.workspaces[index].worktree_space =
        Some(maestro_workspace::workspace::WorktreeSpaceMembership {
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

#[test]
fn daemon_close_pane_last_pane_in_parent_worktree_group_prompts() {
    use maestro_daemon::app::state::Mode;
    let mut state = app_with_workspaces(&["parent", "child"]);
    mark_worktree(&mut state, 0, false);
    mark_worktree(&mut state, 1, true);
    state.active = Some(0);
    state.selected = 1;
    let deferred = state.close_pane();
    assert!(deferred);
    assert_eq!(state.mode, Mode::ConfirmClose);
    assert_eq!(state.selected, 0);
    assert_eq!(state.topology.workspaces.len(), 2);
}

#[test]
fn daemon_close_tab_in_linked_worktree_closes_workspace_only() {
    let mut state = app_with_workspaces(&["selected", "active"]);
    mark_worktree(&mut state, 1, true);
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let checkout = directory.path().to_path_buf();
    state.topology.workspaces[1]
        .worktree_space
        .as_mut()
        .unwrap()
        .checkout_path = checkout.clone();
    state.active = Some(1);
    state.selected = 0;
    state.close_tab();
    assert!(checkout.is_dir());
    assert_eq!(state.topology.workspaces.len(), 1);
    assert_eq!(state.topology.workspaces[0].display_name(None), "selected");
    std::fs::remove_dir(checkout).unwrap();
}

#[test]
fn daemon_close_tab_last_tab_in_parent_worktree_group_prompts() {
    use maestro_daemon::app::state::Mode;
    let mut state = app_with_workspaces(&["parent", "child"]);
    mark_worktree(&mut state, 0, false);
    mark_worktree(&mut state, 1, true);
    state.active = Some(0);
    state.selected = 1;
    let deferred = state.close_tab();
    assert!(deferred);
    assert_eq!(state.mode, Mode::ConfirmClose);
    assert_eq!(state.selected, 0);
    assert_eq!(state.topology.workspaces.len(), 2);
}

#[test]
fn daemon_close_pane_last_pane_in_linked_worktree_closes_workspace_only() {
    let mut state = app_with_workspaces(&["selected", "active"]);
    mark_worktree(&mut state, 1, true);
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let checkout = directory.path().to_path_buf();
    state.topology.workspaces[1]
        .worktree_space
        .as_mut()
        .unwrap()
        .checkout_path = checkout.clone();
    state.active = Some(1);
    state.selected = 0;
    state.close_pane();
    assert!(checkout.is_dir());
    assert_eq!(state.topology.workspaces.len(), 1);
    assert_eq!(state.topology.workspaces[0].display_name(None), "selected");
    std::fs::remove_dir(checkout).unwrap();
}

#[test]
fn daemon_close_pane_last_pane_in_parent_worktree_group_closes_when_confirmation_disabled() {
    let mut state = app_with_workspaces(&["parent", "child", "notes"]);
    mark_worktree(&mut state, 0, false);
    mark_worktree(&mut state, 1, true);
    state.confirm_close = maestro_protocol::shared::Flag(false);
    state.active = Some(0);
    state.selected = 0;
    let deferred = state.close_pane();
    assert!(!deferred);
    assert_eq!(state.topology.workspaces.len(), 1);
    assert_eq!(state.topology.workspaces[0].display_name(None), "notes");
}
