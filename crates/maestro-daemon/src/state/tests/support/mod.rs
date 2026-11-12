//! Shared daemon state admission and invariant observations.

pub(crate) mod api;
pub(crate) mod layouts;
pub(crate) mod spawning;

pub(crate) use api::settings;

use crate::state::model::DaemonState;
use std::sync::Arc;

/// Check topology identity, focus and metadata membership after each mutation.
pub(crate) fn assert_identity_invariants(state: &DaemonState) {
    use std::collections::HashSet;
    assert_navigation_invariants(state);
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

/// Navigation positions are valid for both empty and populated topology.
fn assert_navigation_invariants(state: &DaemonState) {
    if state.topology.workspaces.is_empty() {
        assert_eq!(state.active, None);
        assert_eq!(state.selected, 0);
        return;
    }
    assert!(state.selected < state.topology.workspaces.len());
    assert!(
        state
            .active
            .is_some_and(|index| index < state.topology.workspaces.len())
    );
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

/// Builds one isolated detection snapshot for state and any pane runtimes under test.
pub(crate) fn manifest_cache()
-> Arc<std::sync::RwLock<maestro_detection::definitions::ManifestCache>> {
    Arc::new(std::sync::RwLock::new(
        maestro_detection::definitions::ManifestCache::new(None, None).unwrap(),
    ))
}
