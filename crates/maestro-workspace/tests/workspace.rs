//! Public numbering, cached identity and workspace mutations.
#![cfg(test)]
use super::support::*;
use maestro_protocol::schema::SplitDirection;
use maestro_workspace::{layout::PaneId, workspace::*};

#[test]
fn workspace_handles_use_compact_readable_numbers() {
    let mut owner = topology();
    let first = owner.generate_workspace_id();
    let second = owner.generate_workspace_id();
    assert_eq!(first, "w1");
    assert_eq!(second, "w2");
    assert_ne!(first, second);
    assert!(first.len() <= 3 && second.len() <= 3);
}

#[test]
fn readable_numbers_round_trip_across_digit_carries() {
    for (number, expected) in [
        (1, "1"),
        (9, "9"),
        (10, "A"),
        (31, "Z"),
        (32, "0"),
        (33, "11"),
    ] {
        assert_eq!(encode_public_number(number), expected);
    }
    for value in [1, 9, 10, 31, 32, 33, 1024, 1025] {
        assert_eq!(
            decode_public_number(&encode_public_number(value)),
            Some(value)
        );
    }
}

#[test]
fn restored_workspace_handle_advances_allocator() {
    let mut saved = workspace().workspaces.remove(0);
    saved.id = "wZ".into();
    let mut owner = topology();
    owner.admit_restored_workspace(saved);
    let generated = owner.generate_workspace_id();
    assert_ne!(generated, "wZ");
    assert!(public_workspace_number(&generated) > public_workspace_number("wZ"));
}

#[test]
fn closed_pane_numbers_are_not_reused() {
    let mut owner = workspace();
    let root = owner.workspaces[0].tabs[0].root_pane;
    let second = split(&mut owner, 0, root, SplitDirection::Right);
    let third = split(&mut owner, 0, second, SplitDirection::Down);
    assert_eq!(owner.workspaces[0].public_pane_number(root), Some(1));
    assert_eq!(owner.workspaces[0].public_pane_number(second), Some(2));
    assert_eq!(owner.workspaces[0].public_pane_number(third), Some(3));
    assert!(!owner.workspaces[0].close_pane(second));
    assert_eq!(owner.workspaces[0].public_pane_number(second), None);
    assert_eq!(owner.workspaces[0].public_pane_number(root), Some(1));
    assert_eq!(owner.workspaces[0].public_pane_number(third), Some(3));
    let fourth = split(&mut owner, 0, third, SplitDirection::Right);
    assert_eq!(owner.workspaces[0].public_pane_number(fourth), Some(4));
}

#[test]
fn closed_tab_numbers_are_not_reused() {
    let mut owner = workspace();
    let first = owner.workspaces[0].tabs[0].root_pane;
    let second = add_tab(&mut owner, 0);
    let third = add_tab(&mut owner, 0);
    for (pane, number) in [(first, 1), (second, 2), (third, 3)] {
        let ws = &owner.workspaces[0];
        assert_eq!(
            ws.public_tab_number(ws.find_tab_index_for_pane(pane).unwrap()),
            Some(number)
        );
    }
    assert!(owner.workspaces[0].close_tab(1));
    assert_eq!(owner.workspaces[0].public_tab_number(0), Some(1));
    assert_eq!(owner.workspaces[0].public_tab_number(1), Some(3));
    let fourth = add_tab(&mut owner, 0);
    let ws = &owner.workspaces[0];
    assert_eq!(
        ws.public_tab_number(ws.find_tab_index_for_pane(fourth).unwrap()),
        Some(4)
    );
    assert_invariants(ws);
}

#[test]
fn topology_identity_survives_reordering_and_new_panes() {
    let mut owner = workspace();
    add_tab(&mut owner, 0);
    add_tab(&mut owner, 0);
    owner.workspaces[0].switch_tab(2);
    assert!(owner.workspaces[0].move_tab(2, 0));
    let ws = &owner.workspaces[0];
    assert_ne!(ws.active_tab + 1, ws.tabs[ws.active_tab].number);
    let target = ws.tabs[ws.active_tab].root_pane;
    owner.alloc_pane_id();
    let pane = split(&mut owner, 0, target, SplitDirection::Down);
    assert_ne!(
        usize::try_from(pane.raw()).unwrap(),
        owner.workspaces[0].public_pane_number(pane).unwrap()
    );
    let ws = &mut owner.workspaces[0];
    assert!(ws.move_tab(ws.active_tab, ws.tabs.len()));
    assert_invariants(ws);
}

#[test]
fn rejected_move_returns_original_pane() {
    let mut owner = workspace();
    let pane = owner.workspaces[0].tabs[0].root_pane;
    let taken = owner.workspaces[0].take_pane_for_move(pane).unwrap();
    assert!(taken.workspace_empty);
    owner
        .new_with_tab("/target".into(), None, |_, _| Ok(()))
        .unwrap();
    let expected = taken.moved.clone();
    let recovered = owner.workspaces[1]
        .insert_moved_pane_into_tab(
            0,
            taken.moved,
            &request(PaneId::from_raw(999), SplitDirection::Right),
        )
        .unwrap_err();
    assert_eq!(recovered, expected);
    assert!(!owner.workspaces[1].tabs[0].panes.contains_key(&pane));
}

#[test]
fn display_label_uses_admitted_identity_cache() {
    let mut owner = workspace();
    let directory = std::env::temp_dir().join(format!(
        "maestro-label-{}-{}",
        std::process::id(),
        owner.workspaces[0].id
    ));
    std::fs::create_dir(&directory).unwrap();
    let ws = &mut owner.workspaces[0];
    ws.identity_cwd = directory.clone();
    ws.cached_identity_cwd = directory.clone();
    ws.cached_auto_label = "cached-checkout".into();
    std::fs::remove_dir(directory).unwrap();
    assert_eq!(ws.display_name(None), "cached-checkout");
}

#[test]
fn tab_reorder_preserves_selection_and_public_numbers() {
    let mut owner = workspace();
    let moved_root = owner.workspaces[0].tabs[0].root_pane;
    add_tab(&mut owner, 0);
    add_tab(&mut owner, 0);
    let ws = &mut owner.workspaces[0];
    ws.tabs[1].custom_name = Some("custom".into());
    let active = ws.tabs[2].root_pane;
    ws.switch_tab(2);
    assert!(ws.move_tab(0, ws.tabs.len()));
    assert_eq!(
        (0..ws.tabs.len())
            .map(|i| ws.tab_display_name(i).unwrap())
            .collect::<Vec<_>>(),
        ["custom", "2", "3"]
    );
    assert_eq!(
        ws.tabs.iter().map(|t| t.number).collect::<Vec<_>>(),
        [2, 3, 1]
    );
    assert_eq!(
        ws.tabs
            .iter()
            .map(|t| t.custom_name.as_deref())
            .collect::<Vec<_>>(),
        [Some("custom"), None, None]
    );
    assert_eq!(ws.tabs[2].root_pane, moved_root);
    assert_eq!(ws.tabs[ws.active_tab].root_pane, active);
    assert_invariants(ws);
}

#[test]
fn public_number_decoding_covers_empty_invalid_and_overflow() {
    assert_eq!(encode_public_number(0), "0");
    assert_eq!(decode_public_number(""), Some(0));
    assert_eq!(decode_public_number("0"), Some(32));
    assert_eq!(public_workspace_number("wA"), Some(10));
    assert_eq!(public_workspace_number("A"), None);
    assert_eq!(public_workspace_number("xA"), None);
    assert_eq!(public_pane_id_for_number("wA", 33), "wA:p11");
    assert_eq!(public_tab_id_for_number("wA", 32), "wA:t0");
    for invalid in [
        "a",
        "I",
        "L",
        "O",
        "U",
        "!",
        "111111111111111111111111111111111111111111111111111111111111111111",
    ] {
        assert_eq!(decode_public_number(invalid), None);
    }
}

#[test]
fn git_status_records_use_supplied_identity() {
    use maestro_workspace::workspace::{
        GitSpaceMetadata, GitStatusRefreshDemand, WorkspaceGitStatusSnapshot,
        WorktreeSpaceMembership,
    };
    let cwd = std::path::PathBuf::from("/home/member");
    let checkout = std::path::PathBuf::from("/work/checkout");
    let space = GitSpaceMetadata {
        key: "group".into(),
        checkout_key: "checkout-key".into(),
        repo_name: "group-name".into(),
        repo_root: checkout,
        is_linked_worktree: true,
    };
    let snapshot = WorkspaceGitStatusSnapshot {
        auto_label: "ignored".into(),
        branch: Some("main".into()),
        ahead_behind: Some((2, 3)),
        space: Some(space.clone()),
    };
    assert_refresh_demands(&snapshot, &space, &cwd);
    let empty = WorkspaceGitStatusSnapshot {
        auto_label: "ignored".into(),
        branch: None,
        ahead_behind: None,
        space: None,
    };
    assert_eq!(
        empty
            .into_workspace_status(
                "w1".into(),
                (cwd.clone(), cwd.clone()),
                GitStatusRefreshDemand::default(),
                Some(&cwd)
            )
            .auto_label,
        "~"
    );
    let mut owner = workspace();
    let ws = &mut owner.workspaces[0];
    ws.cached_git_branch = Some("feature".into());
    ws.cached_git_ahead_behind = Some((4, 5));
    ws.cached_git_space = Some(space.clone());
    let membership = WorktreeSpaceMembership {
        key: "managed".into(),
        label: "group".into(),
        repo_root: "/repo".into(),
        checkout_path: "/checkout".into(),
        is_linked_worktree: true,
    };
    ws.worktree_space = Some(membership.clone());
    assert_eq!(ws.branch(), Some("feature".into()));
    assert_eq!(ws.git_ahead_behind(), Some((4, 5)));
    assert_eq!(ws.git_space(), Some(&space));
    assert_eq!(ws.worktree_space(), Some(&membership));
    ws.cached_auto_label = "cached".into();
    assert_eq!(ws.display_name(None), "cached");
    ws.identity_cwd = "/different/fallback".into();
    assert_eq!(ws.display_name(None), "fallback");
    ws.set_custom_name(String::new());
    assert_eq!(ws.display_name(None), "");
}

fn assert_refresh_demands(
    snapshot: &WorkspaceGitStatusSnapshot,
    space: &GitSpaceMetadata,
    cwd: &std::path::PathBuf,
) {
    for (branch, ahead_behind) in [(false, false), (false, true), (true, false), (true, true)] {
        let demand = GitStatusRefreshDemand {
            branch,
            ahead_behind,
        };
        assert_eq!(demand.is_empty(), !branch && !ahead_behind);
        let status = snapshot.clone().into_workspace_status(
            "wA".into(),
            (cwd.clone(), "/cache".into()),
            demand,
            Some(cwd),
        );
        assert_eq!(status.workspace_id, "wA");
        assert_eq!(&status.resolved_identity_cwd, cwd);
        assert_eq!(status.status_cache_key, std::path::PathBuf::from("/cache"));
        assert_eq!(status.demand, demand);
        assert_eq!(status.auto_label, "checkout");
        assert_eq!(status.branch, Some("main".into()));
        assert_eq!(status.ahead_behind, Some((2, 3)));
        assert_eq!(status.space, Some(space.clone()));
    }
}
