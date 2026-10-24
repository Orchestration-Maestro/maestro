//! Owned allocation and native identity representations.
#![cfg(test)]
use super::support::*;
use maestro_protocol::schema::SplitDirection;
use maestro_workspace::{
    pane_state::PaneState,
    spaces::WorktreeSpaceMembership,
    terminal_id::TerminalId,
    tiling::{PaneId, TileLayout},
};

#[test]
fn one_topology_owner_allocates_and_reseeds_all_ids() {
    let mut owner = workspace();
    let root = owner.workspaces[0].tabs[0].root_pane;
    let second = split(&mut owner, 0, root, SplitDirection::Right);
    let third = add_tab(&mut owner, 0);
    let (fourth, ()) = owner
        .new_with_tab("/second".into(), None, |_, _| Ok(()))
        .unwrap();
    assert_eq!(
        [root.raw(), second.raw(), third.raw(), fourth.raw()],
        [1, 2, 3, 4]
    );
    let mut saved = workspace().workspaces.remove(0);
    saved.id = "wZ".into();
    let old_root = saved.tabs[0].root_pane;
    let old = saved.tabs[0].panes.remove(&old_root).unwrap();
    let restored = PaneId::from_raw(400);
    saved.tabs[0].panes.insert(restored, old);
    saved.tabs[0].root_pane = restored;
    saved.tabs[0].layout = TileLayout::new(restored);
    saved.tabs[0].number = 50;
    saved.tabs[0]
        .panes
        .get_mut(&restored)
        .unwrap()
        .attached_terminal_id =
        serde_json::from_value(serde_json::json!("saved-terminal")).unwrap();
    saved.public_pane_numbers.clear();
    saved.public_pane_numbers.insert(restored, 40);
    saved.next_public_pane_number = 41;
    saved.next_public_tab_number = 51;
    let index = owner.admit_restored_workspace(saved);
    assert_eq!(
        owner.workspaces[index]
            .terminal_id(restored)
            .unwrap()
            .as_str(),
        "term_abc5"
    );
    let next = split(&mut owner, index, restored, SplitDirection::Down);
    assert!(next.raw() > 400);
    assert_eq!(owner.workspaces[index].public_pane_number(next), Some(41));
    let tab = add_tab(&mut owner, index);
    assert_eq!(owner.workspaces[index].public_tab_number(1), Some(51));
    assert!(tab.raw() > next.raw());
    assert_eq!(owner.generate_workspace_id(), "w0");
    owner.workspaces.clear();
    let (pane, ()) = owner
        .new_with_tab("/after-close".into(), None, |_, _| Ok(()))
        .unwrap();
    assert!(pane.raw() > tab.raw());
    assert_eq!(owner.workspaces[0].id, "w11");
    assert_invariants(&owner.workspaces[0]);
}

#[test]
fn independent_topologies_have_independent_allocator_state() {
    let mut first = topology();
    let mut second = topology();
    assert_eq!(first.alloc_pane_id(), second.alloc_pane_id());
    assert_eq!(first.alloc_terminal_id(), second.alloc_terminal_id());
    assert_eq!(
        first.generate_workspace_id(),
        second.generate_workspace_id()
    );
    first
        .new_with_tab("/first".into(), None, |_, _| Ok(()))
        .unwrap();
    first
        .new_with_tab("/next".into(), None, |_, _| Ok(()))
        .unwrap();
    assert_eq!(second.alloc_pane_id().raw(), 2);
    assert_eq!(second.alloc_terminal_id().as_str(), "term_abc2");
    assert_eq!(second.generate_workspace_id(), "w2");
    let root = first.workspaces[0].tabs[0].root_pane;
    let split_id = split(&mut first, 0, root, SplitDirection::Right);
    let tab = add_tab(&mut first, 1);
    assert!(tab.raw() > split_id.raw());
}

#[test]
fn identity_and_membership_records_preserve_native_shapes() {
    let pane = PaneId::from_raw(23);
    assert_eq!(pane.raw(), 23);
    assert_eq!(serde_json::to_value(pane).unwrap(), serde_json::json!(23));
    assert_eq!(
        serde_json::from_value::<PaneId>(serde_json::json!(23)).unwrap(),
        pane
    );
    let mut owner = topology();
    let terminal = owner.alloc_terminal_id();
    assert_eq!(terminal.as_str(), "term_abc1");
    assert_eq!(terminal.to_string(), terminal.as_str());
    assert_eq!(
        serde_json::to_value(&terminal).unwrap(),
        serde_json::json!("term_abc1")
    );
    assert_eq!(
        serde_json::from_value::<TerminalId>(serde_json::json!("term_abc1")).unwrap(),
        terminal
    );
    let state = PaneState::new(terminal.clone());
    assert_eq!(state.attached_terminal_id, terminal);
    assert!(state.seen);
    assert!(!state.right_click_passthrough);
    let membership = WorktreeSpaceMembership {
        key: "group".into(),
        label: "label".into(),
        repo_root: "/repo".into(),
        checkout_path: "/checkout".into(),
        is_linked_worktree: true,
    };
    let value = serde_json::json!({"key":"group","label":"label","repo_root":"/repo","checkout_path":"/checkout","is_linked_worktree":true});
    assert_eq!(serde_json::to_value(&membership).unwrap(), value);
    assert_eq!(
        serde_json::from_value::<WorktreeSpaceMembership>(value).unwrap(),
        membership
    );
}
