//! A created workspace retains the repository identity discovered at its directory.

use crate::state::test_support::spawning;
use crate::{settings::model::Config, state::App};
use maestro_test_process::{TestProcess, run_alone};

#[test]
fn setup_entry() {
    if maestro_process::terminal::setup_entry().is_some() {
        std::process::exit(1);
    }
}

#[test]
fn daemon_new_workspace_retains_discovered_git_metadata() {
    if run_alone("daemon_new_workspace_retains_discovered_git_metadata").unwrap() {
        return;
    }
    TestProcess::named("state::tests::repository_identity::setup_entry")
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
        maestro_detection::definitions::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = spawning::launch_inputs(args, manifests).unwrap();
    let mut config = Config::default();
    config.terminal.default_shell = "/bin/sh".into();
    let mut app = App::new(
        &config,
        || 0,
        launch,
        receiver,
        crate::service::event_hub::EventHub::default(),
    );
    app.create_workspace_with_options(root.clone(), true)
        .unwrap();
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.git_space().unwrap().repo_root, root);
    assert_eq!(workspace.branch().as_deref(), Some("main"));
    assert_eq!(
        workspace.cached_git_status_key,
        maestro_path::fs::canonicalize(&root).unwrap()
    );
    for (_, runtime) in app.terminal_runtimes.drain_for_handoff() {
        runtime.shutdown().await;
    }
    std::fs::remove_dir_all(root).unwrap();
}
