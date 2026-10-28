//! Worktree commands through the daemon: listing, opening, creating and removing checkouts.
#![cfg(target_os = "linux")]
use crate::{
    repository_support::{init_repo_with_commit, run_git},
    support::{
        api::{
            TestApp, add_workspaces, answer, daemon, daemon_with_workspaces, events_after, failure,
            request, settle, submit, with_live_daemon,
        },
        runtime::block_on,
    },
};
use maestro_daemon::git::discovery::git_space_metadata;
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, ResponseResult, WorkspaceInfo, WorktreeInfo, WorktreeSourceInfo},
};
use maestro_test_directory::Scratch;
use maestro_workspace::workspace::WorktreeSpaceMembership;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

type Answer = Result<ResponseResult, ErrorBody>;

/// A repository named `repo` with one committed file.
fn repository(scratch: &Scratch) -> PathBuf {
    let repo = scratch.path().join("repo");
    init_repo_with_commit(&repo);
    std::fs::write(repo.join("README.md"), "test\n").unwrap();
    run_git(&repo, &["add", "README.md"]);
    run_git(&repo, &["commit", "--quiet", "-m", "readme"]);
    repo
}

/// A linked checkout of `repo` on the new `branch`, beside the repository.
fn checkout(scratch: &Scratch, repo: &Path, branch: &str) -> PathBuf {
    let path = scratch.path().join(branch.replace('/', "-"));
    run_git(
        repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            branch,
            text(&path),
            "HEAD",
        ],
    );
    path
}

/// A path as text, for Git arguments and wire records.
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// The membership of a linked checkout in the repository group of `repo`.
fn linked(repo: &Path, checkout: &Path) -> WorktreeSpaceMembership {
    WorktreeSpaceMembership {
        key: git_space_metadata(repo).unwrap().key,
        label: "repo".into(),
        repo_root: repo.to_owned(),
        checkout_path: checkout.to_owned(),
        is_linked_worktree: true,
    }
}

/// Answers `wire` from outside a runtime, finishing any background work.
fn ask(app: &mut TestApp, wire: Value) -> Answer {
    block_on(answer(app, wire))
}

/// Points the daemon's checkouts at a directory of the scratch root.
fn keep_checkouts_in(app: &mut TestApp, scratch: &Scratch) -> PathBuf {
    let trees = scratch.path().join("trees");
    app.config.worktrees.directory = text(&trees).to_owned();
    trees
}

/// The source and entries of a list answer.
fn listing(result: Answer) -> (WorktreeSourceInfo, Vec<WorktreeInfo>) {
    match result {
        Ok(ResponseResult::WorktreeList { source, worktrees }) => (source, worktrees),
        other => panic!("expected a worktree list: {other:?}"),
    }
}

/// The entry of a list that is on `branch`.
fn on_branch<'a>(worktrees: &'a [WorktreeInfo], branch: &str) -> &'a WorktreeInfo {
    worktrees
        .iter()
        .find(|entry| entry.branch.as_deref() == Some(branch))
        .unwrap_or_else(|| panic!("no entry on {branch}: {worktrees:?}"))
}

/// The tags of every event published after `sequence`, in order.
fn kinds_after(app: &TestApp, sequence: u64) -> Vec<String> {
    events_after(app, sequence)
        .iter()
        .map(|event| {
            serde_json::to_value(event).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

/// A canonical path, so spellings through links compare equal.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap()
}

#[test]
fn list_reports_the_workspace_each_checkout_is_open_in() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/list");
    let mut app = daemon_with_workspaces(&[text(&repo), text(&linked_path)]);
    let result = request(
        &mut app,
        json!({"type": "worktree.list", "workspace_id": "w1"}),
    );
    let (source, worktrees) = listing(result);
    assert_eq!(canonical(Path::new(&source.repo_root)), canonical(&repo));
    assert_eq!(source.repo_name, "repo");
    assert_eq!(source.source_workspace_id.as_deref(), Some("w1"));
    let entry = on_branch(&worktrees, "worktree/list");
    assert_eq!(entry.open_workspace_id.as_deref(), Some("w2"));
    assert!(entry.is_linked_worktree.0);
    assert_eq!(entry.label, "repo");
    let parent = worktrees
        .iter()
        .find(|entry| !entry.is_linked_worktree.0)
        .unwrap();
    assert_eq!(parent.open_workspace_id.as_deref(), Some("w1"));
}

#[test]
fn list_without_a_source_follows_the_active_workspace() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/active");
    let mut app = daemon_with_workspaces(&[text(&repo)]);
    let (source, worktrees) = listing(request(&mut app, json!({"type": "worktree.list"})));
    assert_eq!(source.source_workspace_id.as_deref(), Some("w1"));
    assert_eq!(
        on_branch(&worktrees, "worktree/active").path,
        text(&linked_path)
    );
}

#[test]
fn list_starts_from_a_linked_checkout() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/from-linked");
    let mut app = daemon_with_workspaces(&[text(&repo), text(&linked_path)]);
    for source in [
        json!({"workspace_id": "w2"}),
        json!({"cwd": text(&linked_path)}),
    ] {
        let mut wire = json!({"type": "worktree.list"});
        wire.as_object_mut()
            .unwrap()
            .extend(source.as_object().unwrap().clone());
        let (source, worktrees) = listing(request(&mut app, wire));
        assert_eq!(canonical(Path::new(&source.repo_root)), canonical(&repo));
        assert_eq!(source.source_workspace_id.as_deref(), Some("w1"));
        assert!(
            on_branch(&worktrees, "worktree/from-linked")
                .is_linked_worktree
                .0
        );
    }
}

#[test]
fn list_starts_from_the_membership_a_workspace_carries() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/member");
    let elsewhere = scratch.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    let mut app = daemon_with_workspaces(&[text(&repo), text(&elsewhere)]);
    app.state.topology.workspaces[0].worktree_space = Some(WorktreeSpaceMembership {
        is_linked_worktree: false,
        checkout_path: repo.clone(),
        ..linked(&repo, &repo)
    });
    app.state.topology.workspaces[1].worktree_space = Some(linked(&repo, &linked_path));
    let (source, worktrees) = listing(request(
        &mut app,
        json!({"type": "worktree.list", "workspace_id": "w2"}),
    ));
    assert_eq!(source.source_checkout_path, text(&repo));
    assert_eq!(source.source_workspace_id.as_deref(), Some("w1"));
    assert_eq!(
        on_branch(&worktrees, "worktree/member")
            .open_workspace_id
            .as_deref(),
        Some("w2")
    );
}

#[test]
fn list_keeps_prunable_entries() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/prunable");
    std::fs::remove_dir_all(&linked_path).unwrap();
    let mut app = daemon_with_workspaces(&[text(&repo)]);
    let (_, worktrees) = listing(request(&mut app, json!({"type": "worktree.list"})));
    let entry = on_branch(&worktrees, "worktree/prunable");
    assert!(entry.is_prunable && entry.is_linked_worktree.0);
}

#[test]
fn list_names_a_source_that_cannot_be_used() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let bare = scratch.path().join("plain");
    std::fs::create_dir(&bare).unwrap();
    let mut app = daemon();
    let list = |extra: Value| {
        let mut wire = json!({"type": "worktree.list"});
        wire.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        wire
    };
    assert_eq!(
        request(&mut app, list(json!({}))),
        failure(
            "invalid_request",
            "workspace_id or cwd is required when no workspace is active"
        )
    );
    assert_eq!(
        request(
            &mut app,
            list(json!({"workspace_id": "w1", "cwd": text(&repo)}))
        ),
        failure(
            "invalid_request",
            "only one of workspace_id or cwd may be supplied"
        )
    );
    assert_eq!(
        request(&mut app, list(json!({"workspace_id": "w9"}))),
        failure("workspace_not_found", "workspace w9 not found")
    );
    assert_eq!(
        request(&mut app, list(json!({"cwd": "relative/repo"}))),
        failure("invalid_request", "worktree path must be absolute")
    );
    assert_eq!(
        request(&mut app, list(json!({"cwd": text(&bare)}))),
        failure(
            "not_git_worktree",
            "Maestro worktree actions require a path inside a Git work tree"
        )
    );
    add_workspaces(&mut app, &[text(&bare)]);
    assert_eq!(
        request(&mut app, list(json!({}))),
        failure(
            "not_git_worktree",
            "Maestro worktree actions require a workspace inside a Git work tree"
        )
    );
}

#[test]
fn open_reuses_a_checkout_open_in_a_subdirectory() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/open");
    let nested = linked_path.join("nested");
    std::fs::create_dir(&nested).unwrap();
    let mut app = daemon_with_workspaces(&[text(&repo), text(&nested)]);
    let result = ask(
        &mut app,
        json!({"type": "worktree.open", "workspace_id": "w1",
               "branch": "worktree/open", "focus": true}),
    );
    let Ok(ResponseResult::WorktreeOpened {
        workspace,
        worktree,
        already_open,
        ..
    }) = result
    else {
        panic!("expected an opened worktree: {result:?}");
    };
    assert!(already_open);
    assert_eq!(workspace.workspace_id, "w2");
    assert_eq!(worktree.branch.as_deref(), Some("worktree/open"));
    assert!(worktree.is_linked_worktree.0);
    assert_eq!(app.state.active, Some(1));
    assert_eq!(app.state.topology.workspaces.len(), 2);
    let memberships: Vec<_> = app
        .state
        .topology
        .workspaces
        .iter()
        .map(|workspace| {
            workspace
                .worktree_space
                .as_ref()
                .unwrap()
                .is_linked_worktree
        })
        .collect();
    assert_eq!(memberships, [false, true]);
    assert_eq!(
        kinds_after(&app, 0),
        ["workspace_updated", "workspace_updated", "worktree_opened"]
    );
    assert!(matches!(
        events_after(&app, 0).last(),
        Some(EventData::WorktreeOpened { workspace, already_open: true, .. })
            if workspace.workspace_id == "w2"
    ));
}

#[test]
fn open_label_on_an_open_checkout_renames_and_announces_it() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/label");
    let mut app = daemon_with_workspaces(&[text(&repo), text(&linked_path)]);
    let result = ask(
        &mut app,
        json!({"type": "worktree.open", "workspace_id": "w1",
               "branch": "worktree/label", "label": "review"}),
    );
    let Ok(ResponseResult::WorktreeOpened { workspace, .. }) = result else {
        panic!("expected an opened worktree: {result:?}");
    };
    assert_eq!(
        (workspace.workspace_id.as_str(), workspace.label.as_str()),
        ("w2", "review")
    );
    assert_eq!(app.state.active, Some(0));
    let events = events_after(&app, 0);
    assert!(events.iter().any(|event| matches!(
        event,
        EventData::WorkspaceUpdated { workspace }
            if workspace.workspace_id == "w2"
                && workspace.worktree.as_ref().is_some_and(|worktree| worktree.is_linked_worktree)
    )));
    assert!(events.contains(&EventData::WorkspaceRenamed {
        workspace_id: "w2".into(),
        label: "review".into(),
    }));
}

#[test]
fn open_creates_the_source_checkout_workspace_as_not_already_open() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    with_live_daemon(
        "worktree_commands::open_creates_the_source_checkout_workspace_as_not_already_open",
        async |app| {
            let wire = json!({"type": "worktree.open", "cwd": text(&repo),
                              "path": text(&repo), "label": "source checkout"});
            let result = answer(app, wire).await;
            let Ok(ResponseResult::WorktreeOpened {
                workspace,
                worktree,
                already_open,
                ..
            }) = result
            else {
                panic!("expected an opened worktree: {result:?}");
            };
            assert!(!already_open);
            assert_eq!(workspace.label, "source checkout");
            assert!(!worktree.is_linked_worktree.0);
            assert_eq!(app.state.topology.workspaces.len(), 1);
            assert_eq!(
                kinds_after(app, 0),
                [
                    "workspace_created",
                    "tab_created",
                    "pane_created",
                    "worktree_opened"
                ]
            );
            assert!(matches!(
                &events_after(app, 0)[0],
                EventData::WorkspaceCreated { workspace }
                    if workspace.label == "source checkout"
                        && workspace.worktree.as_ref().is_some_and(|worktree| !worktree.is_linked_worktree)
            ));
        },
    );
}

#[test]
fn open_by_branch_creates_a_workspace_for_a_checkout_not_yet_open() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/new-workspace");
    with_live_daemon(
        "worktree_commands::open_by_branch_creates_a_workspace_for_a_checkout_not_yet_open",
        async |app| {
            add_workspaces(app, &[text(&repo)]);
            let wire = json!({"type": "worktree.open", "workspace_id": "w1",
                              "branch": "worktree/new-workspace", "focus": true});
            let result = answer(app, wire).await;
            let Ok(ResponseResult::WorktreeOpened {
                workspace,
                worktree,
                already_open,
                ..
            }) = result
            else {
                panic!("expected an opened worktree: {result:?}");
            };
            assert!(!already_open);
            assert_eq!(workspace.workspace_id, "w2");
            assert!(workspace.focused);
            assert_eq!(worktree.path, text(&linked_path));
            assert_eq!(worktree.open_workspace_id.as_deref(), Some("w2"));
            let membership = app.state.topology.workspaces[1]
                .worktree_space
                .clone()
                .unwrap();
            assert!(membership.is_linked_worktree);
            assert_eq!(membership.checkout_path, linked_path);
            assert_eq!(
                kinds_after(app, 0),
                [
                    "workspace_updated",
                    "workspace_created",
                    "tab_created",
                    "pane_created",
                    "worktree_opened"
                ]
            );
        },
    );
}

#[test]
fn open_checks_its_request_before_creating_anything() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/checked");
    let mut app = daemon();
    let open = |extra: Value| {
        let mut wire = json!({"type": "worktree.open", "cwd": text(&repo)});
        wire.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        wire
    };
    let exactly_one = failure(
        "invalid_request",
        "exactly one of path or branch is required",
    );
    assert_eq!(ask(&mut app, open(json!({}))), exactly_one);
    let both = open(json!({"path": text(&linked_path), "branch": "worktree/checked"}));
    assert_eq!(ask(&mut app, both), exactly_one);
    assert_eq!(
        ask(&mut app, open(json!({"branch": "no/such"}))),
        failure("worktree_not_found", "worktree branch not found")
    );
    assert_eq!(
        ask(
            &mut app,
            open(json!({"path": text(&scratch.path().join("nothing"))}))
        ),
        failure("worktree_not_found", "worktree path not found")
    );
    assert_eq!(
        ask(&mut app, open(json!({"path": "relative/checkout"}))),
        failure("invalid_request", "worktree path must be absolute")
    );
    assert!(std::env::var_os("HOME").is_some());
    assert_eq!(
        ask(&mut app, open(json!({"path": "~/no-such-checkout"}))),
        failure("worktree_not_found", "worktree path not found")
    );
    let from_linked = json!({"type": "worktree.open", "cwd": text(&linked_path),
                             "branch": "worktree/checked"});
    assert_eq!(
        ask(&mut app, from_linked),
        failure(
            "linked_worktree_source",
            "New and open worktree actions start from the repo parent workspace."
        )
    );
    assert!(app.state.topology.workspaces.is_empty());
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn open_refuses_a_checkout_whose_directory_is_gone_and_an_ambiguous_branch() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let gone = checkout(&scratch, &repo, "worktree/gone");
    std::fs::remove_dir_all(&gone).unwrap();
    let first = checkout(&scratch, &repo, "worktree/twice");
    let second = scratch.path().join("second");
    run_git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "--force",
            text(&second),
            "worktree/twice",
        ],
    );
    let mut app = daemon_with_workspaces(&[text(&repo)]);
    assert_eq!(
        ask(
            &mut app,
            json!({"type": "worktree.open", "path": text(&gone)})
        ),
        failure("worktree_not_found", "worktree cannot be opened")
    );
    assert_eq!(
        ask(
            &mut app,
            json!({"type": "worktree.open", "branch": "worktree/twice"})
        ),
        failure(
            "ambiguous_worktree_branch",
            "multiple worktrees matched branch"
        )
    );
    assert!(first.exists() && second.exists());
}

#[test]
fn open_from_a_linked_workspace_is_refused() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/refused");
    let mut app = daemon_with_workspaces(&[text(&repo), text(&linked_path)]);
    app.state.topology.workspaces[1].worktree_space = Some(linked(&repo, &linked_path));
    assert_eq!(
        ask(
            &mut app,
            json!({"type": "worktree.open", "workspace_id": "w2", "branch": "worktree/refused"})
        ),
        failure(
            "linked_worktree_source",
            "New and open worktree actions start from the repo parent workspace."
        )
    );
}

/// Whether each workspace's membership is of a linked checkout, in display order.
fn linked_flags(app: &TestApp) -> Vec<bool> {
    let workspaces = &app.state.topology.workspaces;
    workspaces
        .iter()
        .map(|workspace| {
            workspace
                .worktree_space
                .as_ref()
                .unwrap()
                .is_linked_worktree
        })
        .collect()
}

/// The events that announce a created workspace and then its worktree, once.
fn assert_creation_events(app: &TestApp, workspace: &WorkspaceInfo, worktree: &WorktreeInfo) {
    let kinds = kinds_after(app, 0);
    assert_eq!(
        kinds[kinds.len() - 4..],
        [
            "workspace_created",
            "tab_created",
            "pane_created",
            "worktree_created"
        ]
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| *kind == "worktree_created")
            .count(),
        1
    );
    assert!(matches!(
        events_after(app, 0).last(),
        Some(EventData::WorktreeCreated { workspace: event, worktree: info })
            if event == workspace && info == worktree
    ));
}

#[test]
fn create_opens_a_workspace_and_marks_both_memberships() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    with_live_daemon(
        "worktree_commands::create_opens_a_workspace_and_marks_both_memberships",
        async |app| {
            let trees = keep_checkouts_in(app, &scratch);
            add_workspaces(app, &[text(&repo)]);
            let wire = json!({"type": "worktree.create", "workspace_id": "w1",
                              "branch": "worktree/api-create"});
            let result = answer(app, wire).await;
            let Ok(ResponseResult::WorktreeCreated {
                workspace,
                tab,
                root_pane,
                worktree,
            }) = result
            else {
                panic!("expected a created worktree: {result:?}");
            };
            let expected = trees.join("repo").join("worktree-api-create");
            assert_eq!(worktree.path, text(&expected));
            assert_eq!(worktree.branch.as_deref(), Some("worktree/api-create"));
            assert!(expected.join("README.md").exists());
            assert_eq!(
                (tab.workspace_id.as_str(), root_pane.workspace_id.as_str()),
                ("w2", "w2")
            );
            assert_eq!(
                workspace
                    .worktree
                    .as_ref()
                    .map(|info| info.is_linked_worktree),
                Some(true)
            );
            assert_eq!(linked_flags(app), [false, true]);
            assert_creation_events(app, &workspace, &worktree);
        },
    );
}

#[test]
fn create_from_a_directory_announces_the_parent_with_its_membership() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    with_live_daemon(
        "worktree_commands::create_from_a_directory_announces_the_parent_with_its_membership",
        async |app| {
            keep_checkouts_in(app, &scratch);
            let wire = json!({"type": "worktree.create", "cwd": text(&repo),
                              "branch": "worktree/api-cwd"});
            let result = answer(app, wire).await;
            assert!(
                matches!(result, Ok(ResponseResult::WorktreeCreated { .. })),
                "{result:?}"
            );
            assert_eq!(
                kinds_after(app, 0),
                [
                    "workspace_created",
                    "tab_created",
                    "pane_created",
                    "workspace_created",
                    "tab_created",
                    "pane_created",
                    "worktree_created"
                ]
            );
            assert!(matches!(
                &events_after(app, 0)[0],
                EventData::WorkspaceCreated { workspace }
                    if workspace.worktree.as_ref().is_some_and(|worktree| !worktree.is_linked_worktree)
            ));
        },
    );
}

#[test]
fn create_checks_out_an_existing_branch_and_applies_label_and_focus() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    run_git(&repo, &["branch", "existing"]);
    with_live_daemon(
        "worktree_commands::create_checks_out_an_existing_branch_and_applies_label_and_focus",
        async |app| {
            keep_checkouts_in(app, &scratch);
            add_workspaces(app, &[text(&repo)]);
            let wire = json!({"type": "worktree.create", "workspace_id": "w1",
                              "branch": "existing", "label": "Review", "focus": true});
            let result = answer(app, wire).await;
            let Ok(ResponseResult::WorktreeCreated {
                workspace,
                worktree,
                ..
            }) = result
            else {
                panic!("expected a created worktree: {result:?}");
            };
            assert_eq!(worktree.branch.as_deref(), Some("existing"));
            assert_eq!(workspace.label, "Review");
            assert_eq!(app.state.active, Some(1));
            let shown = std::process::Command::new("git")
                .arg("-C")
                .arg(&worktree.path)
                .args(["branch", "--show-current"])
                .output()
                .unwrap();
            assert_eq!(String::from_utf8(shown.stdout).unwrap().trim(), "existing");
        },
    );
}

#[test]
fn create_without_a_branch_generates_one_below_the_configured_directory() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    with_live_daemon(
        "worktree_commands::create_without_a_branch_generates_one_below_the_configured_directory",
        async |app| {
            let trees = keep_checkouts_in(app, &scratch);
            add_workspaces(app, &[text(&repo)]);
            let result = answer(
                app,
                json!({"type": "worktree.create", "workspace_id": "w1"}),
            )
            .await;
            let Ok(ResponseResult::WorktreeCreated { worktree, .. }) = result else {
                panic!("expected a created worktree: {result:?}");
            };
            let branch = worktree.branch.unwrap();
            assert!(branch.starts_with("worktree/"), "{branch}");
            let folder = maestro_daemon::git::worktree::branch_to_path_slug(&branch);
            assert_eq!(worktree.path, text(&trees.join("repo").join(folder)));
        },
    );
}

#[test]
fn create_checks_its_request_before_touching_anything() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let mut app = daemon_with_workspaces(&[text(&repo)]);
    let create = |extra: Value| {
        let mut wire = json!({"type": "worktree.create", "workspace_id": "w1"});
        wire.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        wire
    };
    assert_eq!(
        ask(&mut app, create(json!({"branch": "   "}))),
        failure("invalid_request", "branch is required")
    );
    assert_eq!(
        ask(
            &mut app,
            create(json!({"branch": "worktree/x", "path": "relative-checkout"}))
        ),
        failure("invalid_request", "worktree path must be absolute")
    );
    assert_eq!(
        ask(
            &mut app,
            create(json!({"branch": "worktree/x", "cwd": text(&repo)}))
        ),
        failure(
            "invalid_request",
            "only one of workspace_id or cwd may be supplied"
        )
    );
    let relative =
        json!({"type": "worktree.create", "cwd": "relative-repo", "branch": "worktree/x"});
    assert_eq!(
        ask(&mut app, relative),
        failure("invalid_request", "worktree path must be absolute")
    );
    let blank_from_cwd = json!({"type": "worktree.create", "cwd": text(&repo), "branch": " "});
    let mut empty = daemon();
    assert_eq!(
        ask(&mut empty, blank_from_cwd),
        failure("invalid_request", "branch is required")
    );
    assert!(empty.state.topology.workspaces.is_empty());
    assert_eq!(app.state.topology.workspaces.len(), 1);
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn create_failure_is_reported_and_leaves_the_checkout_free_to_retry() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let current = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["branch", "--show-current"])
        .output()
        .unwrap();
    let current = String::from_utf8(current.stdout).unwrap().trim().to_owned();
    let mut app = daemon_with_workspaces(&[text(&repo)]);
    keep_checkouts_in(&mut app, &scratch);
    let wire = json!({"type": "worktree.create", "workspace_id": "w1", "branch": current});
    for _ in 0..2 {
        let Err(error) = ask(&mut app, wire.clone()) else {
            panic!("a branch checked out elsewhere cannot be added");
        };
        assert_eq!(error.code, "worktree_create_failed", "{}", error.message);
        assert!(error.message.contains(&current), "{}", error.message);
    }
    assert_eq!(app.state.topology.workspaces.len(), 1);
}

#[test]
fn create_completes_for_the_repository_after_its_source_workspace_changed() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    with_live_daemon(
        "worktree_commands::create_completes_for_the_repository_after_its_source_workspace_changed",
        async |app| {
            keep_checkouts_in(app, &scratch);
            add_workspaces(app, &[text(&repo)]);
            let wire = json!({"type": "worktree.create", "workspace_id": "w1",
                              "branch": "worktree/moved-on"});
            let pending = submit(app, wire).await;
            let other = WorktreeSpaceMembership {
                key: "other-key".into(),
                label: "other".into(),
                repo_root: "/repo/other".into(),
                checkout_path: "/repo/other".into(),
                is_linked_worktree: false,
            };
            app.state.topology.workspaces[0].worktree_space = Some(other.clone());
            let result = settle(app, pending).await;
            assert!(
                matches!(result, Ok(ResponseResult::WorktreeCreated { .. })),
                "{result:?}"
            );
            let workspaces = &app.state.topology.workspaces;
            assert_eq!(workspaces.len(), 3);
            assert_eq!(workspaces[0].worktree_space.as_ref(), Some(&other));
            let parent = workspaces[1].worktree_space.as_ref().unwrap();
            assert!(!parent.is_linked_worktree && parent.checkout_path == repo);
        },
    );
}

#[test]
fn create_refuses_a_checkout_another_operation_owns() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let linked_path = checkout(&scratch, &repo, "worktree/busy");
    with_live_daemon(
        "worktree_commands::create_refuses_a_checkout_another_operation_owns",
        async |app| {
            add_workspaces(app, &[text(&repo), text(&linked_path)]);
            keep_checkouts_in(app, &scratch);
            app.state.topology.workspaces[1].worktree_space = Some(linked(&repo, &linked_path));
            let busy = failure(
                "worktree_operation_in_progress",
                "worktree operation is already in progress for this checkout",
            );
            let removing = submit(
                app,
                json!({"type": "worktree.remove", "workspace_id": "w2"}),
            )
            .await;
            let create = json!({"type": "worktree.create", "workspace_id": "w1",
                                "branch": "worktree/other", "path": text(&linked_path)});
            assert_eq!(answer(app, create).await, busy);
            let next =
                json!({"type": "worktree.create", "workspace_id": "w1", "branch": "worktree/next"});
            let adding = submit(app, next.clone()).await;
            assert_eq!(answer(app, next).await, busy);
            let removed = settle(app, removing).await;
            assert!(
                matches!(removed, Ok(ResponseResult::WorktreeRemoved { .. })),
                "{removed:?}"
            );
            let added = settle(app, adding).await;
            assert!(
                matches!(added, Ok(ResponseResult::WorktreeCreated { .. })),
                "{added:?}"
            );
        },
    );
}

/// A parent workspace and a linked workspace of `repo` on `branch`, the parent active.
fn parent_and_child(scratch: &Scratch, repo: &Path, branch: &str) -> (TestApp, PathBuf) {
    let linked_path = checkout(scratch, repo, branch);
    let mut app = daemon_with_workspaces(&[text(repo), text(&linked_path)]);
    app.state.topology.workspaces[0].worktree_space = Some(WorktreeSpaceMembership {
        is_linked_worktree: false,
        checkout_path: repo.to_owned(),
        ..linked(repo, repo)
    });
    app.state.topology.workspaces[1].worktree_space = Some(linked(repo, &linked_path));
    (app, linked_path)
}

#[test]
fn create_from_a_workspace_with_membership_keeps_it_as_the_source() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    with_live_daemon(
        "worktree_commands::create_from_a_workspace_with_membership_keeps_it_as_the_source",
        async |app| {
            keep_checkouts_in(app, &scratch);
            add_workspaces(app, &[text(&repo)]);
            let parent = WorktreeSpaceMembership {
                is_linked_worktree: false,
                checkout_path: repo.clone(),
                ..linked(&repo, &repo)
            };
            app.state.topology.workspaces[0].worktree_space = Some(parent.clone());
            let wire = json!({"type": "worktree.create", "workspace_id": "w1",
                              "branch": "worktree/member-source"});
            let result = answer(app, wire).await;
            assert!(
                matches!(result, Ok(ResponseResult::WorktreeCreated { .. })),
                "{result:?}"
            );
            assert_eq!(app.state.topology.workspaces.len(), 2);
            assert_eq!(
                app.state.topology.workspaces[0].worktree_space.as_ref(),
                Some(&parent)
            );
            assert_eq!(
                kinds_after(app, 0),
                [
                    "workspace_created",
                    "tab_created",
                    "pane_created",
                    "worktree_created"
                ]
            );
        },
    );
}

#[test]
fn finishing_an_operation_marks_the_view_for_rendering() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let (mut app, _) = parent_and_child(&scratch, &repo, "worktree/render");
    block_on(async {
        let pending = submit(
            &mut app,
            json!({"type": "worktree.remove", "workspace_id": "w2"}),
        )
        .await;
        let _ = app.launch.render_dirty.take();
        assert!(!app.launch.render_dirty.is_pending());
        let result = settle(&mut app, pending).await;
        assert!(
            matches!(result, Ok(ResponseResult::WorktreeRemoved { .. })),
            "{result:?}"
        );
        assert!(app.launch.render_dirty.is_pending());
    });
}

#[test]
fn remove_needs_force_for_a_dirty_checkout() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let (mut app, linked_path) = parent_and_child(&scratch, &repo, "worktree/dirty");
    std::fs::write(linked_path.join("README.md"), "dirty\n").unwrap();
    let Err(refused) = ask(
        &mut app,
        json!({"type": "worktree.remove", "workspace_id": "w2"}),
    ) else {
        panic!("a dirty checkout must not be removed without force");
    };
    assert_eq!(refused.code, "dirty_worktree_requires_force");
    assert!(linked_path.exists());
    assert_eq!(app.state.topology.workspaces.len(), 2);
    let forced = ask(
        &mut app,
        json!({"type": "worktree.remove", "workspace_id": "w2", "force": true}),
    );
    assert_eq!(
        forced,
        Ok(ResponseResult::WorktreeRemoved {
            workspace_id: "w2".into(),
            path: text(&linked_path).into(),
            forced: true,
        })
    );
    assert!(!linked_path.exists());
    assert_eq!(app.state.topology.workspaces.len(), 1);
}

#[test]
fn remove_closes_the_workspace_then_announces_the_removal() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let (mut app, _) = parent_and_child(&scratch, &repo, "worktree/removed");
    app.state.active = Some(1);
    app.state.selected = 1;
    let result = ask(
        &mut app,
        json!({"type": "worktree.remove", "workspace_id": "w2"}),
    );
    assert!(
        matches!(
            result,
            Ok(ResponseResult::WorktreeRemoved { forced: false, .. })
        ),
        "{result:?}"
    );
    assert_eq!(
        kinds_after(&app, 0),
        ["workspace_closed", "worktree_removed"]
    );
    let events = events_after(&app, 0);
    assert!(matches!(
        &events[0],
        EventData::WorkspaceClosed { workspace_id, workspace: Some(workspace) }
            if workspace_id == "w2"
                && workspace.worktree.as_ref().is_some_and(|worktree| worktree.is_linked_worktree)
    ));
    assert!(matches!(
        &events[1],
        EventData::WorktreeRemoved { workspace_id, workspace: Some(workspace), worktree, forced: false }
            if workspace_id == "w2"
                && workspace.workspace_id == "w2"
                && worktree.branch.as_deref() == Some("worktree/removed")
                && worktree.is_linked_worktree.0
                && worktree.open_workspace_id.is_none()
    ));
    assert!(app.state.terminal_runtime_shutdowns.is_empty());
    assert_eq!((app.state.active, app.state.selected), (Some(0), 0));
}

#[test]
fn remove_names_a_workspace_that_is_not_a_linked_checkout() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let (mut app, _) = parent_and_child(&scratch, &repo, "worktree/not-linked");
    let remove = |id: &str| json!({"type": "worktree.remove", "workspace_id": id});
    assert_eq!(
        ask(&mut app, remove("w9")),
        failure("workspace_not_found", "workspace w9 not found")
    );
    assert_eq!(
        ask(&mut app, remove("w1")),
        failure(
            "not_linked_worktree",
            "workspace is not a linked worktree checkout"
        )
    );
    app.state.topology.workspaces[0].worktree_space = None;
    assert_eq!(
        ask(&mut app, remove("w1")),
        failure(
            "not_linked_worktree",
            "workspace is not a Maestro-managed worktree checkout"
        )
    );
    assert_eq!(app.state.topology.workspaces.len(), 2);
}

#[test]
fn remove_rejects_a_second_request_for_the_same_workspace_or_checkout() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let (mut app, linked_path) = parent_and_child(&scratch, &repo, "worktree/twice");
    let twin = scratch.path().join("twin");
    std::fs::create_dir(&twin).unwrap();
    add_workspaces(&mut app, &[text(&twin)]);
    app.state.topology.workspaces[2].worktree_space = Some(linked(&repo, &linked_path));
    let busy = failure(
        "worktree_operation_in_progress",
        "worktree operation is already in progress for this checkout",
    );
    block_on(async {
        let first = submit(
            &mut app,
            json!({"type": "worktree.remove", "workspace_id": "w2", "force": true}),
        )
        .await;
        let same_workspace = json!({"type": "worktree.remove", "workspace_id": "w2"});
        assert_eq!(answer(&mut app, same_workspace).await, busy);
        let same_checkout = json!({"type": "worktree.remove", "workspace_id": "w3", "force": true});
        assert_eq!(answer(&mut app, same_checkout).await, busy);
        let result = settle(&mut app, first).await;
        assert!(
            matches!(
                result,
                Ok(ResponseResult::WorktreeRemoved { forced: true, .. })
            ),
            "{result:?}"
        );
    });
}

#[test]
fn remove_failure_is_reported_and_frees_the_checkout() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let (mut app, linked_path) = parent_and_child(&scratch, &repo, "worktree/unlisted");
    run_git(
        &repo,
        &["worktree", "remove", "--force", text(&linked_path)],
    );
    for _ in 0..2 {
        let Err(error) = ask(
            &mut app,
            json!({"type": "worktree.remove", "workspace_id": "w2"}),
        ) else {
            panic!("Git does not list the checkout");
        };
        assert_eq!(error.code, "worktree_remove_failed", "{}", error.message);
    }
    assert_eq!(app.state.topology.workspaces.len(), 2);
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn remove_announces_its_snapshot_after_the_workspace_changed() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let (mut app, linked_path) = parent_and_child(&scratch, &repo, "worktree/changed");
    let moved = linked(&repo, Path::new("/repo/other"));
    block_on(async {
        let pending = submit(
            &mut app,
            json!({"type": "worktree.remove", "workspace_id": "w2", "force": true}),
        )
        .await;
        app.state.topology.workspaces[1].worktree_space = Some(moved.clone());
        let result = settle(&mut app, pending).await;
        assert_eq!(
            result,
            Ok(ResponseResult::WorktreeRemoved {
                workspace_id: "w2".into(),
                path: text(&linked_path).into(),
                forced: true,
            })
        );
    });
    assert_eq!(kinds_after(&app, 0), ["worktree_removed"]);
    assert_eq!(app.state.topology.workspaces.len(), 2);
    assert_eq!(
        app.state.topology.workspaces[1].worktree_space.as_ref(),
        Some(&moved)
    );
}

#[test]
fn remove_shuts_down_the_workspace_processes_first_only_when_forced() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    let first = checkout(&scratch, &repo, "worktree/process-one");
    let second = checkout(&scratch, &repo, "worktree/process-two");
    with_live_daemon(
        "worktree_commands::remove_shuts_down_the_workspace_processes_first_only_when_forced",
        async |app| {
            add_workspaces(app, &[text(&repo)]);
            for checkout in [&first, &second] {
                let index = app
                    .create_workspace_with_options(checkout.clone(), false)
                    .unwrap();
                app.state.topology.workspaces[index].worktree_space = Some(linked(&repo, checkout));
            }
            assert_eq!(app.terminal_runtimes.len(), 2);
            let gentle = submit(
                app,
                json!({"type": "worktree.remove", "workspace_id": "w2"}),
            )
            .await;
            assert_eq!(app.terminal_runtimes.len(), 2);
            let forced = submit(
                app,
                json!({"type": "worktree.remove", "workspace_id": "w3", "force": true}),
            )
            .await;
            assert_eq!(app.terminal_runtimes.len(), 1);
            assert!(matches!(
                settle(app, gentle).await,
                Ok(ResponseResult::WorktreeRemoved { .. })
            ));
            assert!(matches!(
                settle(app, forced).await,
                Ok(ResponseResult::WorktreeRemoved { .. })
            ));
            assert!(app.terminal_runtimes.is_empty());
            assert_eq!(app.state.topology.workspaces.len(), 1);
        },
    );
}

#[test]
fn direct_handling_refuses_the_commands_that_wait_for_git() {
    let mut app = daemon();
    for (kind, params) in [
        ("worktree.create", json!({})),
        ("worktree.remove", json!({"workspace_id": "w1"})),
    ] {
        let mut wire = json!({"type": kind});
        wire.as_object_mut()
            .unwrap()
            .extend(params.as_object().unwrap().clone());
        assert_eq!(
            request(&mut app, wire),
            failure(
                "invalid_request",
                &format!("{kind} is handled asynchronously by the app runtime")
            )
        );
    }
}

#[test]
fn the_daemon_loop_answers_a_create_when_git_finishes() {
    let scratch = Scratch::new().unwrap();
    let repo = repository(&scratch);
    with_live_daemon(
        "worktree_commands::the_daemon_loop_answers_a_create_when_git_finishes",
        async |app| {
            use maestro_daemon::{api::RequestMessage, paths::DaemonPaths};
            use maestro_protocol::{
                rpc_types::RpcCommand,
                schema::{EmptyParams, WorktreeCreateParams},
            };
            use tokio::sync::{mpsc, oneshot};
            let trees = keep_checkouts_in(app, &scratch);
            add_workspaces(app, &[text(&repo)]);
            let paths = DaemonPaths::new(text(&scratch.path().join("agent")), None).unwrap();
            let (sender, mut requests) = mpsc::unbounded_channel();
            let message = |command| {
                let (respond_to, response) = oneshot::channel();
                let (_attempted, response_write_complete) = oneshot::channel();
                let request = RequestMessage {
                    command,
                    respond_to,
                    response_write_complete,
                };
                (request, response)
            };
            let (create, created) = message(RpcCommand::WorktreeCreate(WorktreeCreateParams {
                workspace_id: Some("w1".into()),
                branch: Some("worktree/loop".into()),
                ..WorktreeCreateParams::default()
            }));
            let (stop, _stopped) = message(RpcCommand::ServerStop(EmptyParams {}));
            sender.send(create).unwrap();
            let mut settings = crate::support::api::settings();
            let ((), answered) = tokio::join!(
                app.run(&paths, &mut requests, &mut settings, std::future::pending()),
                async {
                    let answered = created.await.unwrap();
                    sender.send(stop).unwrap();
                    answered
                }
            );
            let Ok(ResponseResult::WorktreeCreated { worktree, .. }) = answered else {
                panic!("expected a created worktree: {answered:?}");
            };
            assert_eq!(
                worktree.path,
                text(&trees.join("repo").join("worktree-loop"))
            );
        },
    );
}
