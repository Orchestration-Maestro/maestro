//! Repository identity admission stays distinct from live terminal observations.
#![cfg(test)]
#![cfg(target_os = "linux")]
use crate::repository_support::{init_repo_with_commit, run_git};
use crate::support::launch;
use maestro_daemon::{
    app::App,
    settings::model::Config,
    terminal::{runtime_registry::TerminalRuntimeRegistry, state::TerminalState},
};
use maestro_test_process::{TestProcess, run_alone};
use maestro_workspace::{
    terminal_id::TerminalId,
    workspace::{Workspace, WorkspaceTopology},
};
use std::{collections::HashMap, path::PathBuf};

#[test]
fn setup_entry() {
    if maestro_process::pty::setup_entry().is_some() {
        std::process::exit(1);
    }
}

#[test]
fn daemon_new_workspace_retains_discovered_git_metadata() {
    if run_alone("daemon_new_workspace_retains_discovered_git_metadata").unwrap() {
        return;
    }
    TestProcess::named("repository_identity::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(check_admitted_repository(args));
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}

async fn check_admitted_repository(args: &[String]) {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    let manifests = std::sync::Arc::new(std::sync::RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = launch::launch_inputs(args, manifests);
    let mut config = Config::default();
    config.terminal.default_shell = "/bin/sh".into();
    let mut app = App::new(
        &config,
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        maestro_daemon::api::event_hub::EventHub::default(),
    );
    app.create_workspace_with_options(root.clone(), true)
        .unwrap();
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.git_space().unwrap().repo_root, root);
    assert_eq!(workspace.branch().as_deref(), Some("main"));
    assert_eq!(
        workspace.cached_git_status_key,
        std::fs::canonicalize(&root).unwrap()
    );
    for (_, runtime) in app.terminal_runtimes.drain_for_handoff() {
        runtime.shutdown().await;
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn daemon_linked_worktree_auto_label_uses_checkout_name_not_repo_name() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let scratch = directory.path().to_path_buf();
    let repo = scratch.join("maestro");
    let checkout = scratch.join("testr56");
    init_repo_with_commit(&repo);
    run_git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "testr56",
            checkout.to_str().unwrap(),
            "HEAD",
        ],
    );
    let space = maestro_daemon::git::discovery::git_space_metadata(&checkout).unwrap();
    let label = maestro_daemon::git::discovery::automatic_workspace_label(
        &checkout,
        &space.repo_root,
        None,
    );
    assert_eq!(space.repo_name, repo.file_name().unwrap().to_str().unwrap());
    assert_eq!(label, checkout.file_name().unwrap().to_str().unwrap());
    std::fs::remove_dir_all(scratch).unwrap();
}

fn workspace() -> (Workspace, TerminalId) {
    let mut topology = WorkspaceTopology::new(|| 0);
    let (pane, ()) = topology
        .new_with_tab("/old/workspace".into(), None, |_, _| Ok(()))
        .unwrap();
    let workspace = topology.workspaces.remove(0);
    let terminal = workspace.terminal_id(pane).unwrap().clone();
    (workspace, terminal)
}

#[test]
fn daemon_terminal_aware_display_name_uses_latest_admitted_identity_cache() {
    let (mut workspace, terminal) = workspace();
    workspace.cached_identity_cwd = PathBuf::from("/new/repo/deep");
    workspace.cached_auto_label = "repo".into();
    let terminals = HashMap::from([(
        terminal.clone(),
        TerminalState::new(terminal, "/new/repo/deep".into()),
    )]);
    assert_eq!(
        maestro_daemon::workspace::display_name_from_terminals(&workspace, &terminals, None),
        "repo"
    );
    assert_eq!(workspace.display_name(None), "workspace");
}

#[test]
fn daemon_workspace_identity_follows_first_tab_root_pane_cwd() {
    let (workspace, terminal) = workspace();
    let terminals = HashMap::from([(
        terminal.clone(),
        TerminalState::new(terminal, "/maestro-test/pion".into()),
    )]);
    let runtimes = TerminalRuntimeRegistry::new();
    assert_eq!(
        maestro_daemon::workspace::display_name_from(&workspace, &terminals, &runtimes, None),
        "pion"
    );
    assert_eq!(
        maestro_daemon::workspace::resolved_identity_cwd_from(&workspace, &terminals, &runtimes),
        Some(PathBuf::from("/maestro-test/pion"))
    );
}
