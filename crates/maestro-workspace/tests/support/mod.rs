//! Shared topology fixtures without ambient or global state.
#![cfg(test)]
use maestro_protocol::schema::SplitDirection;
use maestro_workspace::{
    spaces::WorkspaceTopology,
    tiling::{PaneId, SplitRequest},
};

pub fn topology() -> WorkspaceTopology<fn() -> u128> {
    WorkspaceTopology::new((|| 0xabc) as fn() -> u128)
}

pub fn workspace() -> WorkspaceTopology<fn() -> u128> {
    let mut owner = topology();
    owner
        .new_with_tab("/checkout".into(), None, |_, _| Ok(()))
        .unwrap();
    owner
}

pub fn request(target: PaneId, direction: SplitDirection) -> SplitRequest {
    SplitRequest {
        target,
        direction,
        ratio: None,
        focus_new_pane: true,
    }
}

pub fn split(
    owner: &mut WorkspaceTopology<fn() -> u128>,
    workspace: usize,
    target: PaneId,
    direction: SplitDirection,
) -> PaneId {
    owner
        .split_pane_with_runtime(workspace, request(target, direction), |_, _| Ok(()))
        .unwrap()
        .unwrap()
        .0
}

pub fn add_tab(owner: &mut WorkspaceTopology<fn() -> u128>, workspace: usize) -> PaneId {
    owner
        .create_tab_with_runtime(workspace, |_, _| Ok(()))
        .unwrap()
        .unwrap()
        .0
}

pub fn assert_invariants(workspace: &maestro_workspace::spaces::Workspace) {
    let mut ids = std::collections::HashSet::new();
    let mut terminals = std::collections::HashSet::new();
    let mut tab_numbers = std::collections::HashSet::new();
    let mut pane_numbers = std::collections::HashSet::new();
    for tab in &workspace.tabs {
        let leaves = tab.layout.pane_ids();
        assert!(leaves.contains(&tab.root_pane));
        assert!(leaves.contains(&tab.layout.focused()));
        assert_eq!(leaves.len(), tab.panes.len());
        assert!((1..workspace.next_public_tab_number).contains(&tab.number));
        assert!(tab_numbers.insert(tab.number));
        for id in leaves {
            assert!(ids.insert(id));
            assert!(terminals.insert(&tab.panes[&id].attached_terminal_id));
            let number = workspace.public_pane_numbers[&id];
            assert!((1..workspace.next_public_pane_number).contains(&number));
            assert!(pane_numbers.insert(number));
        }
    }
    assert_eq!(ids.len(), workspace.public_pane_numbers.len());
    assert!(workspace.tabs.is_empty() || workspace.active_tab < workspace.tabs.len());
}

pub type Topology = WorkspaceTopology<fn() -> u128>;

pub mod snapshot;
