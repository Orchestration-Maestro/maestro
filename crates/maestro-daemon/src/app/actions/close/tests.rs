//! Targeted tab retirement keeps unrelated topology and records intact.

use super::*;
use crate::{
    app::state::{PaneFocusTarget, PluginPaneRecord},
    settings::model::Config,
    terminal::state::TerminalState,
};
use maestro_detection::manifest::ManifestCache;
use std::sync::{Arc, RwLock};

/// Builds two workspaces with two tabs each, without child processes.
fn state() -> DaemonState<fn() -> u128> {
    let mut state = DaemonState::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        Arc::new(RwLock::new(ManifestCache::new(None, None).unwrap())),
    );
    for index in 0..2 {
        state
            .topology
            .new_with_tab(format!("/w{index}").into(), None, |_, _| Ok(()))
            .unwrap();
        state
            .topology
            .create_tab_with_runtime(index, |_, _| Ok(()))
            .unwrap()
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

#[test]
fn targeted_tab_retirement_cleans_only_its_owned_records() {
    let mut state = state();
    let workspace = &state.topology.workspaces[1];
    let pane = workspace.tabs[1].root_pane;
    let terminal = workspace.tabs[1].terminal_id(pane).unwrap().clone();
    state.previous_pane_focus = Some(PaneFocusTarget {
        workspace_id: workspace.id.clone(),
        pane_id: pane,
    });
    state.plugin_panes.insert(
        pane,
        PluginPaneRecord {
            plugin_id: "plugin".into(),
            entrypoint: "pane".into(),
        },
    );
    assert!(state.retire_tab(1, 1));
    assert_eq!(state.topology.workspaces[0].tabs.len(), 2);
    assert_eq!(state.topology.workspaces[1].tabs.len(), 1);
    assert_eq!(state.active, Some(0));
    assert_eq!(state.topology.workspaces[1].active_tab, 0);
    assert!(!state.plugin_panes.contains_key(&pane));
    assert!(state.previous_pane_focus.is_none());
    assert!(!state.terminals.contains_key(&terminal));
    assert_eq!(state.terminals.len(), 3);
    assert_eq!(state.terminal_runtime_shutdowns, [terminal]);
}

#[test]
fn targeted_tab_retirement_rejects_missing_and_final_tabs() {
    let mut state = state();
    assert!(!state.retire_tab(2, 0));
    assert!(!state.retire_tab(1, 2));
    assert_eq!(state.terminals.len(), 4);
    assert!(state.retire_tab(1, 1));
    assert!(!state.retire_tab(1, 0));
    assert_eq!(state.topology.workspaces.len(), 2);
    assert_eq!(state.topology.workspaces[1].tabs.len(), 1);
    assert_eq!(state.terminals.len(), 3);
    assert_eq!(state.terminal_runtime_shutdowns.len(), 1);
}
