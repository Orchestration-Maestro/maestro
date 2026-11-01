//! Workspace directories and labels observe the environment without an account fallback.
#![cfg(target_os = "linux")]

use crate::window_title::app;
use maestro_daemon::settings::model::NewTerminalCwdConfig;
use maestro_protocol::shared::Flag;
use std::{collections::BTreeMap, path::PathBuf};

/// How the child process sees `HOME`.
#[derive(Clone, Copy, Debug)]
enum Home {
    /// The variable is absent.
    Unset,
    /// The variable is set and empty.
    Empty,
    /// The variable names a directory.
    Set,
}

/// The `HOME` value the fixture's child receives, when any.
fn variable(case: Home) -> BTreeMap<&'static str, &'static str> {
    match case {
        Home::Unset => BTreeMap::new(),
        Home::Empty => BTreeMap::from([("HOME", "")]),
        Home::Set => BTreeMap::from([("HOME", "/fixture/home")]),
    }
}

/// The home value the daemon must observe for each environment.
fn expected(case: Home) -> Option<PathBuf> {
    match case {
        Home::Unset => None,
        Home::Empty => Some(PathBuf::new()),
        Home::Set => Some("/fixture/home".into()),
    }
}

/// Home, Follow, Path and title labels use the written HOME without account fallback.
#[test]
fn daemon_home_uses_only_the_environment() {
    let cases = [Home::Unset, Home::Empty, Home::Set];
    let Some(cases) = maestro_test_process::run_per_environment(
        "daemon_home::daemon_home_uses_only_the_environment",
        cases,
        |case| variable(*case),
    )
    .unwrap() else {
        return;
    };
    for case in cases {
        let home = expected(case);
        let directory = home
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        for policy in [NewTerminalCwdConfig::Home, NewTerminalCwdConfig::Follow] {
            assert_eq!(new_workspace_directory(policy), directory, "{case:?}");
        }
        for path in ["~", "~/x"] {
            let directory = match (&home, path) {
                (Some(home), "~") => home.clone(),
                (Some(home), _) if home.as_os_str().is_empty() => "/x".into(),
                (Some(home), _) => home.join("x"),
                (None, _) => path.into(),
            };
            assert_eq!(
                new_workspace_directory(NewTerminalCwdConfig::Path(path.into())),
                directory
            );
        }
        let label_home = home.or_else(std::env::home_dir).unwrap();
        if !label_home.as_os_str().is_empty() {
            let label = if matches!(case, Home::Unset) {
                label_home
                    .file_name()
                    .unwrap_or(label_home.as_os_str())
                    .to_string_lossy()
                    .into_owned()
            } else {
                "~".into()
            };
            assert_eq!(window_title(&label_home), label, "{case:?} title");
        }
    }
}

/// The directory the daemon proposes for a new workspace under the home policy.
fn new_workspace_directory(policy: NewTerminalCwdConfig) -> PathBuf {
    let mut app = app();
    app.config.terminal.new_cwd = policy;
    app.state.topology.workspaces.clear();
    app.state.active = None;
    app.state.prompt_new_workspace_name = Flag(true);
    crate::support::runtime::block_on(app.begin_workspace_create()).unwrap();
    app.state.pending_workspace_create_cwd.take().unwrap()
}

/// The window title's workspace label for a terminal working in `home`.
fn window_title(home: &std::path::Path) -> String {
    let mut app = app();
    app.state.terminals.values_mut().next().unwrap().cwd = home.into();
    app.configure_window_title("{workspace}");
    app.window_title().unwrap()
}

/// Missing HOME and an unreadable process directory fall back to the filesystem root.
#[test]
fn daemon_home_without_readable_cwd_uses_root() {
    let Some(_) = maestro_test_process::run_per_environment(
        "daemon_home::daemon_home_without_readable_cwd_uses_root",
        [Home::Unset],
        |case| variable(*case),
    )
    .unwrap() else {
        return;
    };
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    scratch.enter_removed("cwd").unwrap();
    for policy in [NewTerminalCwdConfig::Home, NewTerminalCwdConfig::Follow] {
        assert_eq!(new_workspace_directory(policy), PathBuf::from("/"));
    }
}

/// Creation never shortens the account directory to a home label when HOME is absent.
#[test]
fn daemon_creation_without_home_labels_the_account_directory_by_name() {
    let Some(_) = maestro_test_process::run_per_environment(
        "daemon_home::daemon_creation_without_home_labels_the_account_directory_by_name",
        [Home::Unset],
        |case| variable(*case),
    )
    .unwrap() else {
        return;
    };
    let Some(account) = std::env::home_dir() else {
        return;
    };
    maestro_test_process::TestProcess::named("repository_identity::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            crate::support::runtime::block_on(async {
                let mut config = maestro_daemon::settings::model::Config::default();
                config.terminal.default_shell = "/bin/sh".into();
                let mut app = crate::daemon_runtime::app_with_args(&config, args);
                app.create_workspace_with_options(account.clone(), true)
                    .unwrap();
                let expected = account
                    .file_name()
                    .unwrap_or(account.as_os_str())
                    .to_string_lossy();
                assert_eq!(app.state.topology.workspaces[0].cached_auto_label, expected);
                for (_, runtime) in app.terminal_runtimes.drain_for_handoff() {
                    runtime.shutdown().await;
                }
            });
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}
