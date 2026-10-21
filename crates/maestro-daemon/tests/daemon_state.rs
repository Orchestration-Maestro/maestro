//! Daemon geometry before the first attached view.
#![cfg(test)]

use maestro_daemon::{app::state::DaemonState, settings::model::Config};
use maestro_workspace::layout::{Borders, PaneId, PaneInfo, Rect};

#[test]
fn daemon_pane_size_estimate_uses_headless_size_before_first_view() {
    let mut config = Config::default();
    config.server.headless_cols = 132;
    config.server.headless_rows = 41;
    let mut state = DaemonState::new(&config, || 0);
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
    let mut state = DaemonState::new(&Config::default(), || 0);
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

/// Admit terminal metadata for each pane using its topology-owned identity.
fn populate_terminals<C: FnMut() -> u128>(state: &mut DaemonState<C>) {
    for workspace in &state.topology.workspaces {
        for pane in workspace.tabs.iter().flat_map(|tab| tab.panes.values()) {
            state
                .terminals
                .entry(pane.attached_terminal_id.clone())
                .or_insert_with(|| {
                    maestro_daemon::terminal::state::TerminalState::new(
                        pane.attached_terminal_id.clone(),
                        workspace.identity_cwd.clone(),
                    )
                });
        }
    }
}

/// Check topology identity, focus and metadata membership after each mutation.
fn assert_identity_invariants<C: FnMut() -> u128>(state: &DaemonState<C>) {
    use std::collections::HashSet;
    assert!(state.selected < state.topology.workspaces.len());
    assert!(
        state
            .active
            .is_some_and(|index| index < state.topology.workspaces.len())
    );
    let mut workspaces = HashSet::new();
    let mut panes = HashSet::new();
    let mut terminals = HashSet::new();
    for workspace in &state.topology.workspaces {
        assert!(workspaces.insert(&workspace.id));
        assert!(workspace.active_tab < workspace.tabs.len());
        let mut numbers = HashSet::new();
        for tab in &workspace.tabs {
            assert!(numbers.insert(tab.number));
            assert!(tab.number < workspace.next_public_tab_number);
            assert_tab_geometry(tab);
            for (pane, viewport) in &tab.panes {
                assert!(panes.insert(*pane));
                assert!(terminals.insert(&viewport.attached_terminal_id));
                let number = workspace.public_pane_number(*pane).unwrap();
                assert!(number < workspace.next_public_pane_number);
                let terminal = &state.terminals[&viewport.attached_terminal_id];
                assert_eq!(terminal.id, viewport.attached_terminal_id);
            }
        }
    }
}

/// Each split leaf has exactly one viewport, including the root and focus.
fn assert_tab_geometry(tab: &maestro_workspace::tab::Tab) {
    use std::collections::HashSet;
    assert!(tab.panes.contains_key(&tab.root_pane));
    assert!(tab.panes.contains_key(&tab.layout.focused()));
    assert_eq!(
        tab.layout.pane_ids().into_iter().collect::<HashSet<_>>(),
        tab.panes.keys().copied().collect()
    );
}
