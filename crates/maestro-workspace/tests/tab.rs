//! Caller-owned spawn results and pane transactions.
#![cfg(test)]
use super::support::*;
use maestro_protocol::schema::SplitDirection;
use maestro_workspace::{
    spaces::Workspace,
    tab::{MovedPane, SpawnRequest},
    tiling::PaneId,
};
use maestro_workspace::{spaces::WorkspaceTopology, tiling::SplitRequest};
use std::{cell::Cell, rc::Rc};

struct CallerOutput(Rc<Cell<usize>>);

#[test]
fn spawn_output_belongs_to_caller() {
    let mut topology = WorkspaceTopology::new(|| 0xabc);
    let outputs = Rc::new(Cell::new(0));
    for operation in 0..3 {
        let captured = outputs.clone();
        let observed = outputs.clone();
        let spawn = move |id: maestro_workspace::tiling::PaneId,
                          request: maestro_workspace::tab::SpawnRequest| {
            assert_eq!(request.workspace_id, "w1");
            assert_eq!(request.tab_number, if operation == 0 { 1 } else { 2 });
            assert_eq!(request.pane_number, operation + 1);
            assert_eq!(id.raw(), u32::try_from(operation + 1).unwrap());
            assert_eq!(
                request.terminal_id.as_str(),
                format!("term_abc{}", operation + 1)
            );
            captured.set(captured.get() + 1);
            Ok(CallerOutput(captured))
        };
        let (_, output) = match operation {
            0 => topology
                .new_with_tab("/checkout".into(), None, spawn)
                .unwrap(),
            1 => topology.create_tab_with_runtime(0, spawn).unwrap().unwrap(),
            _ => {
                let target = topology.workspaces[0].tabs[1].root_pane;
                topology
                    .split_pane_with_runtime(
                        0,
                        SplitRequest {
                            target,
                            direction: SplitDirection::Right,
                            ratio: Some(0.25),
                            focus_new_pane: true,
                        },
                        spawn,
                    )
                    .unwrap()
                    .unwrap()
            }
        };
        assert!(Rc::ptr_eq(&output.0, &observed));
        assert_eq!(output.0.get(), operation + 1);
        assert_eq!(Rc::strong_count(&outputs), 3);
        drop(output);
        assert_eq!(Rc::strong_count(&outputs), 2);
    }
    assert_eq!(Rc::strong_count(&outputs), 1);
}

#[test]
fn spawn_failure_restores_split_state() {
    let mut owner = workspace();
    let root = owner.workspaces[0].tabs[0].root_pane;
    let second = split(&mut owner, 0, root, SplitDirection::Right);
    owner.workspaces[0].tabs[0].zoomed = true;
    let before = owner.workspaces[0].clone();
    assert!(
        owner
            .split_pane_with_runtime(
                0,
                request(PaneId::from_raw(999), SplitDirection::Right),
                |_, _| -> std::io::Result<()> { panic!("missing target invoked spawn") }
            )
            .is_none()
    );
    let error = owner
        .split_pane_with_runtime(
            0,
            request(second, SplitDirection::Down),
            |_, _| -> std::io::Result<()> { Err(std::io::Error::from_raw_os_error(13)) },
        )
        .unwrap()
        .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(13));
    assert_eq!(owner.workspaces[0], before);
    assert_missing_core_target(&mut owner, &before);
    let mut expected = before.tabs[0].layout.clone();
    assert!(expected.close_focused());
    let mut actual = owner.workspaces[0].tabs[0].layout.clone();
    assert!(actual.close_focused());
    assert_eq!(actual, expected);
    assert_eq!(actual.focused(), root);
    assert_successful_split_controls(&mut owner, second);
}

#[test]
fn pane_detach_promotes_root_without_losing_viewport_state() {
    let mut owner = workspace();
    let root = owner.workspaces[0].tabs[0].root_pane;
    let second = split(&mut owner, 0, root, SplitDirection::Right);
    let third = split(&mut owner, 0, second, SplitDirection::Down);
    let tab = &mut owner.workspaces[0].tabs[0];
    tab.panes.get_mut(&second).unwrap().seen = false;
    tab.panes.get_mut(&second).unwrap().right_click_passthrough = true;
    let state = tab.panes[&second].clone();
    let root_terminal = tab.terminal_id(root).unwrap().clone();
    tab.zoomed = true;
    assert_eq!(tab.remove_pane(root), Some((root, root_terminal)));
    assert_eq!(tab.root_pane, second);
    assert!(!tab.zoomed);
    assert_eq!(tab.panes[&second], state);
    let third_terminal = tab.terminal_id(third).unwrap().clone();
    tab.zoomed = true;
    assert_eq!(tab.close_pane(third), Some((third, third_terminal)));
    assert_eq!(tab.root_pane, second);
    assert!(!tab.zoomed);
    assert_eq!(tab.remove_pane(second), None);
    assert_eq!(
        tab.take_pane_for_move(maestro_workspace::tiling::PaneId::from_raw(999)),
        None
    );
    let moved = tab.take_pane_for_move(second).unwrap();
    assert_eq!(moved.pane_state, state);
    assert!(tab.panes.is_empty());
    let mut tab = maestro_workspace::tab::Tab::from_existing_pane(1, None, moved);
    let added = owner.alloc_pane_id();
    let terminal = owner.alloc_terminal_id();
    let req = maestro_workspace::tab::SpawnRequest {
        workspace_id: "w1".into(),
        tab_number: 1,
        pane_number: 4,
        terminal_id: terminal,
    };
    tab.split_pane_with_runtime(
        added,
        request(second, SplitDirection::Right),
        req,
        |_, _| Ok(()),
    )
    .unwrap();
    let moved = tab.take_pane_for_move(second).unwrap();
    assert_eq!(tab.root_pane, added);
    assert_eq!(moved.pane_state, state);
}

#[test]
fn pane_moves_preserve_identity_and_recover_rejections() {
    let mut owner = workspace();
    let root = owner.workspaces[0].tabs[0].root_pane;
    let pane = split(&mut owner, 0, root, SplitDirection::Right);
    owner.workspaces[0].pane_state_mut(pane).unwrap().seen = false;
    owner.workspaces[0]
        .pane_state_mut(pane)
        .unwrap()
        .right_click_passthrough = true;
    let number = owner.workspaces[0].public_pane_number(pane);
    let expected = assert_same_workspace_transfer(&mut owner, root, pane);
    let taken = owner.workspaces[0].take_pane_for_move(pane).unwrap();
    owner
        .new_with_tab("/destination".into(), None, |_, _| Ok(()))
        .unwrap();
    let dest = owner.workspaces[1].tabs[0].root_pane;
    owner.workspaces[1]
        .insert_moved_pane_into_tab(0, taken.moved, &request(dest, SplitDirection::Right))
        .unwrap();
    assert_eq!(owner.workspaces[0].public_pane_number(pane), number);
    assert_eq!(owner.workspaces[1].public_pane_number(pane), Some(2));
    assert_eq!(
        owner.workspaces[1].pane_state(pane),
        Some(&expected.pane_state)
    );
    owner.workspaces[0].unregister_moved_pane(pane);
    assert_eq!(owner.workspaces[0].public_pane_number(pane), None);
    let moved = owner.workspaces[1].take_pane_for_move(pane).unwrap().moved;
    let tab = owner.workspaces[1].create_tab_from_existing_pane(moved, Some("recovered".into()));
    assert_eq!(owner.workspaces[1].tabs[tab].root_pane, pane);
    assert_eq!(owner.workspaces[1].public_pane_number(pane), Some(2));
    let moved = owner.workspaces[1].take_pane_for_move(pane).unwrap().moved;
    let recovered = maestro_workspace::spaces::WorkspaceTopology::from_existing_pane(
        &mut owner,
        "/recovered".into(),
        None,
        moved,
    );
    assert_eq!(
        owner.workspaces[recovered].pane_state(pane),
        Some(&expected.pane_state)
    );
    owner.workspaces[1].unregister_moved_pane(pane);
    for workspace in &owner.workspaces {
        assert_invariants(workspace);
    }
}

#[test]
fn tab_mutations_cover_indices_focus_and_seen() {
    let mut owner = workspace();
    let root = owner.workspaces[0].tabs[0].root_pane;
    assert!(
        owner
            .create_tab_with_runtime(99, |_, _| -> std::io::Result<()> {
                panic!("missing workspace spawned")
            })
            .is_none()
    );
    assert_invalid_tab_edits(&mut owner, root);
    add_tab(&mut owner, 0);
    add_tab(&mut owner, 0);
    let ws = &mut owner.workspaces[0];
    assert_tab_seen_and_names(ws, root);
    let selected = ws.focused_pane_id();
    assert!(ws.move_tab(0, 3));
    assert_eq!(ws.focused_pane_id(), selected);
    assert!(ws.move_tab(2, 0));
    assert_eq!(ws.focused_pane_id(), selected);
    assert!(!ws.move_tab(1, 2));
    assert!(!ws.move_tab(1, 1));
    assert!(ws.close_tab(0));
    assert_eq!(ws.focused_pane_id(), selected);
    ws.switch_tab(ws.tabs.len() - 1);
    assert!(ws.close_tab(ws.active_tab_index()));
    assert_eq!(ws.active_tab_index(), 0);
    assert_eq!(ws.focused_pane_id(), selected);
    assert_invariants(ws);
}

#[test]
fn failed_tab_spawn_consumes_only_reserved_tab_number() {
    let mut owner = workspace();
    let before = owner.workspaces[0].clone();
    let error = owner
        .create_tab_with_runtime(0, |_, req| -> std::io::Result<()> {
            assert_eq!(req.tab_number, 2);
            assert_eq!(req.pane_number, 2);
            Err(std::io::Error::from_raw_os_error(13))
        })
        .unwrap()
        .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(13));
    let ws = &owner.workspaces[0];
    assert_eq!(ws.tabs, before.tabs);
    assert_eq!(ws.active_tab, before.active_tab);
    assert_eq!(ws.public_pane_numbers, before.public_pane_numbers);
    assert_eq!(ws.next_public_pane_number, 2);
    let (pane, ()) = owner
        .create_tab_with_runtime(0, |_, req| {
            assert_eq!(req.tab_number, 3);
            assert_eq!(req.pane_number, 2);
            Ok(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(owner.workspaces[0].public_pane_number(pane), Some(2));
    assert_eq!(owner.workspaces[0].public_tab_number(1), Some(3));
    assert_invariants(&owner.workspaces[0]);
}

fn assert_missing_core_target(owner: &mut Topology, before: &Workspace) {
    let new_id = owner.alloc_pane_id();
    let terminal = owner.alloc_terminal_id();
    let missing = owner.workspaces[0].tabs[0]
        .split_pane_with_runtime(
            new_id,
            request(PaneId::from_raw(999), SplitDirection::Down),
            SpawnRequest {
                workspace_id: "w1".into(),
                tab_number: 1,
                pane_number: 3,
                terminal_id: terminal,
            },
            |_, _| -> std::io::Result<()> { panic!("missing tab target invoked spawn") },
        )
        .unwrap_err();
    assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        missing.to_string(),
        "split target pane is not in the layout"
    );
    assert_eq!(&owner.workspaces[0], before);
}

fn assert_successful_split_controls(owner: &mut Topology, second: PaneId) {
    for focus in [false, true] {
        let request = SplitRequest {
            target: second,
            direction: SplitDirection::Down,
            ratio: Some(0.25),
            focus_new_pane: focus,
        };
        owner.workspaces[0].tabs[0].zoomed = true;
        let (pane, terminal) = owner
            .split_pane_with_runtime(0, request, |_, req| Ok(req.terminal_id))
            .unwrap()
            .unwrap();
        let ws = &owner.workspaces[0];
        let tab = &ws.tabs[0];
        assert!(!tab.zoomed);
        assert_eq!(tab.terminal_id(pane), Some(&terminal));
        assert_eq!(tab.layout.focused(), if focus { pane } else { second });
        assert_eq!(
            tab.layout
                .splits(maestro_workspace::tiling::Rect::new(0, 0, 80, 24))
                .last()
                .unwrap()
                .ratio
                .to_bits(),
            0.25f32.to_bits()
        );
        assert_eq!(ws.public_pane_number(pane), Some(if focus { 4 } else { 3 }));
    }
}

fn assert_same_workspace_transfer(owner: &mut Topology, root: PaneId, pane: PaneId) -> MovedPane {
    let number = owner.workspaces[0].public_pane_number(pane);
    let taken = owner.workspaces[0].take_pane_for_move(pane).unwrap();
    assert_eq!(taken.removed_tab_idx, None);
    assert!(!taken.workspace_empty);
    let expected = taken.moved.clone();
    let mut moved = taken.moved;
    for (tab, target) in [(99, root), (0, PaneId::from_raw(999))] {
        moved = owner.workspaces[0]
            .insert_moved_pane_into_tab(tab, moved, &request(target, SplitDirection::Right))
            .unwrap_err();
        assert_eq!(moved, expected);
    }
    let next = owner.workspaces[0].next_public_pane_number;
    owner.workspaces[0]
        .insert_moved_pane_into_tab(0, moved, &request(root, SplitDirection::Down))
        .unwrap();
    assert_eq!(owner.workspaces[0].public_pane_number(pane), number);
    assert_eq!(owner.workspaces[0].next_public_pane_number, next);
    let duplicate = maestro_workspace::tab::MovedPane {
        pane_id: pane,
        pane_state: owner.workspaces[0].pane_state(pane).unwrap().clone(),
    };
    assert_eq!(
        owner.workspaces[0]
            .insert_moved_pane_into_tab(0, duplicate.clone(), &request(root, SplitDirection::Right))
            .unwrap_err(),
        duplicate
    );

    expected
}

fn assert_invalid_tab_edits(owner: &mut Topology, root: PaneId) {
    let ws = &mut owner.workspaces[0];
    assert!(ws.close_pane(root));
    assert!(!ws.close_pane(maestro_workspace::tiling::PaneId::from_raw(999)));
    assert_eq!(ws.focused_pane_id(), Some(root));
    assert!(!ws.close_tab(0));
    assert!(!ws.close_tab(99));
    assert!(!ws.move_tab(99, 0));
    assert!(!ws.move_tab(0, 99));
    assert!(!ws.move_tab(0, 0));
    ws.switch_tab(99);
    assert_eq!(ws.active_tab_index(), 0);
    assert_eq!(ws.focused_pane_id(), Some(root));
}

fn assert_tab_seen_and_names(ws: &mut Workspace, root: PaneId) {
    for tab in &mut ws.tabs {
        for state in tab.panes.values_mut() {
            state.seen = false;
        }
    }
    ws.switch_tab(1);
    assert!(ws.tabs[1].panes.values().all(|p| p.seen));
    assert!(!ws.tabs[0].panes[&root].seen);
    assert!(ws.tabs[2].panes.values().all(|p| !p.seen));
    assert_eq!(ws.tab_display_name(99), None);
    assert_eq!(ws.active_tab_display_name(), Some("2".into()));
    assert!(ws.active_tab().unwrap().is_auto_named());
    ws.active_tab_mut().unwrap().set_custom_name(String::new());
    assert!(!ws.active_tab().unwrap().is_auto_named());
    assert_eq!(ws.active_tab_display_name(), Some(String::new()));
}
