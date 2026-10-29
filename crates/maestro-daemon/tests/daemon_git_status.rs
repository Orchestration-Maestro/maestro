//! Repository status observations applied to retained workspaces.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::support;
use maestro_daemon::{
    app::state::DaemonState, settings::model::Config,
    terminal::runtime_registry::TerminalRuntimeRegistry,
};
use maestro_workspace::workspace::{
    GitSpaceMetadata, GitStatusRefreshDemand, WorkspaceGitStatus, WorktreeSpaceMembership,
};
use std::path::PathBuf;

/// Both observations demanded.
const ALL: GitStatusRefreshDemand = GitStatusRefreshDemand {
    branch: true,
    ahead_behind: true,
};

/// Retained state with one workspace per name, each rooted at its own directory.
fn state_with_workspaces(names: &[&str]) -> DaemonState<fn() -> u128> {
    let mut state = DaemonState::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        support::manifest_cache(),
    );
    for name in names {
        state
            .topology
            .new_with_tab(format!("/workspaces/{name}").into(), None, |_, _| Ok(()))
            .unwrap();
    }
    support::populate_terminals(&mut state);
    state
}

/// An observation of workspace `index` for the directory it currently follows.
fn status(state: &DaemonState<fn() -> u128>, index: usize) -> WorkspaceGitStatus {
    let workspace = &state.topology.workspaces[index];
    WorkspaceGitStatus {
        workspace_id: workspace.id.clone(),
        resolved_identity_cwd: workspace.identity_cwd.clone(),
        status_cache_key: workspace.identity_cwd.clone(),
        demand: ALL,
        auto_label: "label".into(),
        branch: None,
        ahead_behind: None,
        space: None,
    }
}

#[test]
fn daemon_apply_workspace_git_statuses_updates_matching_workspace() {
    let mut state = state_with_workspaces(&["one", "two"]);
    let result = WorkspaceGitStatus {
        auto_label: "one".into(),
        branch: Some("main".into()),
        ahead_behind: Some((2, 1)),
        ..status(&state, 0)
    };

    let changed = state.apply_workspace_git_statuses(&TerminalRuntimeRegistry::new(), vec![result]);

    assert!(changed);
    assert_eq!(
        state.topology.workspaces[0].branch().as_deref(),
        Some("main")
    );
    assert_eq!(
        state.topology.workspaces[0].git_ahead_behind(),
        Some((2, 1))
    );
    assert_eq!(state.topology.workspaces[1].git_ahead_behind(), None);
}

#[test]
fn daemon_apply_workspace_git_statuses_ignores_stale_cwd() {
    let mut state = state_with_workspaces(&["one"]);
    state.topology.workspaces[0].cached_git_branch = Some("old".into());
    state.topology.workspaces[0].cached_git_ahead_behind = Some((1, 0));
    let missing = PathBuf::from("/definitely/not/current");
    let result = WorkspaceGitStatus {
        resolved_identity_cwd: missing.clone(),
        status_cache_key: missing,
        auto_label: "stale".into(),
        branch: Some("main".into()),
        ahead_behind: Some((0, 1)),
        ..status(&state, 0)
    };

    let changed = state.apply_workspace_git_statuses(&TerminalRuntimeRegistry::new(), vec![result]);

    assert!(!changed);
    assert_eq!(
        state.topology.workspaces[0].branch().as_deref(),
        Some("old")
    );
    assert_eq!(
        state.topology.workspaces[0].git_ahead_behind(),
        Some((1, 0))
    );

    let current = WorkspaceGitStatus {
        branch: Some("main".into()),
        ..status(&state, 0)
    };
    assert!(state.apply_workspace_git_statuses(&TerminalRuntimeRegistry::new(), vec![current]));
    assert_eq!(
        state.topology.workspaces[0].branch().as_deref(),
        Some("main")
    );
}

#[test]
fn daemon_apply_workspace_git_statuses_ignores_unrequested_branch_changes() {
    let mut state = state_with_workspaces(&["one"]);
    state.topology.workspaces[0].cached_auto_label = "one".into();
    state.topology.workspaces[0].cached_git_branch = Some("old".into());
    let result = WorkspaceGitStatus {
        demand: GitStatusRefreshDemand {
            branch: false,
            ahead_behind: true,
        },
        auto_label: "one".into(),
        branch: Some("new".into()),
        ..status(&state, 0)
    };

    let changed = state.apply_workspace_git_statuses(&TerminalRuntimeRegistry::new(), vec![result]);

    assert!(!changed);
    assert_eq!(
        state.topology.workspaces[0].branch().as_deref(),
        Some("old")
    );

    let requested = WorkspaceGitStatus {
        branch: Some("new".into()),
        auto_label: "one".into(),
        ..status(&state, 0)
    };
    assert!(state.apply_workspace_git_statuses(&TerminalRuntimeRegistry::new(), vec![requested]));
    assert_eq!(
        state.topology.workspaces[0].branch().as_deref(),
        Some("new")
    );
}

#[test]
fn daemon_apply_workspace_git_statuses_clears_missing_git_status() {
    let mut state = state_with_workspaces(&["one"]);
    state.topology.workspaces[0].cached_auto_label = "one".into();
    state.topology.workspaces[0].cached_git_branch = Some("main".into());
    state.topology.workspaces[0].cached_git_ahead_behind = Some((1, 2));
    let result = WorkspaceGitStatus {
        auto_label: "one".into(),
        ..status(&state, 0)
    };

    let changed = state.apply_workspace_git_statuses(&TerminalRuntimeRegistry::new(), vec![result]);

    assert!(changed);
    assert_eq!(state.topology.workspaces[0].branch(), None);
    assert_eq!(state.topology.workspaces[0].git_ahead_behind(), None);
}

#[test]
fn daemon_apply_workspace_git_statuses_does_not_change_worktree_membership() {
    let mut state = state_with_workspaces(&["one"]);
    state.topology.workspaces[0].worktree_space = Some(WorktreeSpaceMembership {
        key: "repo".into(),
        label: "repo".into(),
        repo_root: "/repo".into(),
        checkout_path: "/repo-worktree".into(),
        is_linked_worktree: true,
    });
    let membership = state.topology.workspaces[0].worktree_space().cloned();
    let result = WorkspaceGitStatus {
        auto_label: "other".into(),
        branch: Some("scratch".into()),
        space: Some(GitSpaceMetadata {
            key: "other-repo-key".into(),
            checkout_key: "/other/checkout".into(),
            repo_name: "other".into(),
            repo_root: "/other/repo".into(),
            is_linked_worktree: false,
        }),
        ..status(&state, 0)
    };

    let changed = state.apply_workspace_git_statuses(&TerminalRuntimeRegistry::new(), vec![result]);

    assert!(changed);
    assert_eq!(
        state.topology.workspaces[0].worktree_space().cloned(),
        membership
    );
}
