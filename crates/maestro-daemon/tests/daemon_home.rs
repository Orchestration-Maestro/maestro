//! The daemon reads the home directory through one lookup: `HOME` as set, else the account.
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

/// The directory the lookup must name for `case`, or `None` when the host has no account.
fn expected(case: Home) -> Option<PathBuf> {
    match case {
        Home::Unset => std::env::home_dir(),
        Home::Empty => Some(PathBuf::new()),
        Home::Set => Some("/fixture/home".into()),
    }
}

#[test]
fn daemon_home_falls_back_to_the_account_directory() {
    let cases = [Home::Unset, Home::Empty, Home::Set];
    let Some(cases) = maestro_test_process::run_per_environment(
        "daemon_home::daemon_home_falls_back_to_the_account_directory",
        cases,
        |case| variable(*case),
    )
    .unwrap() else {
        return;
    };
    for case in cases {
        let Some(home) = expected(case) else {
            eprintln!("skipped {case:?}: the host has no account directory");
            continue;
        };
        assert_eq!(new_workspace_directory(), home, "{case:?} directory");
        if !home.as_os_str().is_empty() {
            assert_eq!(window_title(&home), "~", "{case:?} title");
        }
    }
}

/// The directory the daemon proposes for a new workspace under the home policy.
fn new_workspace_directory() -> PathBuf {
    let mut app = app();
    app.config.terminal.new_cwd = NewTerminalCwdConfig::Home;
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
