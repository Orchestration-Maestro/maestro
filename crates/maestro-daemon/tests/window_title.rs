//! Outer terminal titles use current workspace and focused pane metadata.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::config_support;

use config_support::{TestApp, fixture};

/// Admits one named workspace with retained terminal metadata and no native child.
pub(crate) fn app() -> TestApp {
    let (mut app, _, _) = fixture();
    let (pane, ()) = app
        .state
        .topology
        .new_with_tab("/herd".into(), None, |_, _| Ok(()))
        .unwrap();
    app.state.active = Some(0);
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    app.state.terminals.insert(
        id.clone(),
        maestro_daemon::terminal::state::TerminalState::new(id, "/herd".into()),
    );
    app
}

#[test]
fn daemon_renders_workspace_and_tab_names() {
    let mut app = app();
    app.configure_window_title("{workspace}/{tab}");
    assert_eq!(app.window_title().as_deref(), Some("herd/1"));
    app.state.topology.workspaces[0].tabs[0].custom_name = Some("build".into());
    assert_eq!(app.window_title().as_deref(), Some("herd/build"));
}

#[test]
fn daemon_renders_focused_pane_label_and_terminal_title() {
    let mut app = app();
    app.configure_window_title("{pane}|{terminal_title}");
    let terminal = app.state.terminals.values_mut().next().unwrap();
    terminal.manual_label = Some("api".into());
    terminal.set_terminal_title(Some("⠋ building".into()));
    assert_eq!(app.window_title().as_deref(), Some("api|building"));
}

#[test]
fn daemon_empty_template_disables_window_titles() {
    let mut app = app();
    app.configure_window_title("");
    assert_eq!(app.window_title(), None);
}

#[test]
fn daemon_invalid_template_disables_window_titles() {
    let mut app = app();
    app.configure_window_title("{nope}");
    assert_eq!(app.window_title(), None);
}

#[test]
fn daemon_unset_tokens_render_empty() {
    let mut app = app();
    app.configure_window_title("[{pane}]");
    assert_eq!(app.window_title().as_deref(), Some("[]"));
}

#[test]
fn accepted_title_configuration_reaches_construction_and_reload() {
    let (mut app, mut manager, storage) = fixture();
    assert!(app.window_title_configured());
    assert!(!app.window_title_uses_terminal_title());
    config_support::put(
        &*storage,
        r#"{"multiplexer":{"windowTitle":"{terminal_title}"}}"#,
    );
    config_support::block_on(app.reload_config(&mut manager, std::path::Path::new("/unused")));
    assert!(app.window_title_uses_terminal_title());
    config_support::put(&*storage, r#"{"multiplexer":{"windowTitle":""}}"#);
    config_support::block_on(app.reload_config(&mut manager, std::path::Path::new("/unused")));
    assert!(!app.window_title_configured());
    app.state.active = None;
    app.configure_window_title("literal/{workspace}/{tab}/{pane}/{terminal_title}");
    assert_eq!(app.window_title().as_deref(), Some("literal////"));
}

#[test]
fn workspace_title_retains_home_directory_label() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let mut app = app();
    app.state.terminals.values_mut().next().unwrap().cwd = home.into();
    app.configure_window_title("{workspace}");
    assert_eq!(app.window_title().as_deref(), Some("~"));
}
