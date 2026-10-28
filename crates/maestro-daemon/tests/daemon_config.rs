//! Live daemon preferences retain independent areas through the settings owner.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::config_support;

use config_support::{block_on, fixture, put};
use maestro_daemon::{
    app::state::{Mode, ToastKind},
    settings::model::{
        AgentPanelSortConfig, ConfigReloadStatus, NewTerminalCwdConfig, ShellModeConfig,
        ToastDelivery,
    },
};
use serde_json::json;
use std::{path::Path, time::Instant};

/// Verify accepted launch policy and independent background checks together.
fn assert_launch_and_check_preferences(app: &config_support::TestApp) {
    assert_eq!(app.config.terminal.default_shell, "nu");
    assert_eq!(app.config.terminal.shell_mode, ShellModeConfig::NonLogin);
    assert_eq!(app.config.terminal.new_cwd, NewTerminalCwdConfig::Home);
    assert!(!app.config.update.version_check);
    assert!(!app.config.update.manifest_check);
    assert!(app.next_auto_update_check.is_none());
    assert!(app.next_agent_manifest_update_check.is_none());
    assert!(
        app.config
            .experimental
            .switch_ascii_input_source_in_prefix
            .0
    );
    assert_eq!(
        app.config.ui.right_click_passthrough_modifier.modifiers(),
        Some(maestro_protocol::frames::KeyModifiers::CONTROL)
    );
}

/// Verify the explicit reload's diagnostic and notification feedback.
fn assert_reload_feedback(app: &config_support::TestApp) {
    assert!(app.state.config_diagnostic.is_none());
    assert!(app.state.request_client_config_reload.0);
    let toast = app.state.toast.as_ref().unwrap();
    assert_eq!(toast.kind, ToastKind::UpdateInstalled);
    assert_eq!(toast.title, "reloaded config");
    assert_eq!(toast.context, "using settings.json");
}

#[test]
fn live_reload_applies_daemon_areas_without_binding_loader() {
    let (mut app, mut manager, storage) = fixture();
    put(&*storage, &json!({
        "daemon": {"paneShell":"nu", "paneShellMode":"non_login", "newPaneCwd":"home", "headlessCols":160, "headlessRows":50, "detectionCatalogCheck":false},
        "update":{"versionCheck":false},
        "multiplexer": {"agentPanelSort":"priority", "redrawOnFocusGained":false, "copyOnSelect":false, "rightClickPassthroughModifier":"ctrl", "promptNewWorkspaceName":true, "toast":{"delivery":"workspace"}, "switchAsciiInputSourceInPrefix":true}
    }).to_string());
    let (pane, ()) = app
        .state
        .topology
        .new_with_tab("/selected".into(), None, |_, _| Ok(()))
        .unwrap();
    app.state.selection = Some(maestro_daemon::selection::Selection::range(
        pane, 0, 0, 1, None,
    ));
    app.state.selection_autoscroll = Some(maestro_daemon::app::state::SelectionAutoscroll {
        direction: maestro_daemon::app::state::SelectionAutoscrollDirection::Down,
        last_mouse_screen_col: 1,
        last_mouse_screen_row: 1,
        inner_rect: maestro_workspace::layout::Rect::new(0, 0, 2, 2),
    });
    app.next_auto_update_check = Some(Instant::now());
    app.next_agent_manifest_update_check = Some(Instant::now());
    let report = block_on(app.reload_config(&mut manager, Path::new("."))).0;
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert_eq!(app.state.headless_size, (160, 50));
    assert_eq!(app.state.toast_config.delivery, ToastDelivery::Workspace);
    assert_eq!(app.state.agent_panel_sort, AgentPanelSortConfig::Priority);
    assert!(!app.state.redraw_on_focus_gained.0);
    assert!(!app.config.ui.copy_on_select.0);
    assert!(app.state.prompt_new_workspace_name.0);
    assert_launch_and_check_preferences(&app);
    assert_reload_feedback(&app);
    app.state.mode = Mode::Copy;
    assert_eq!(
        block_on(app.reload_config(&mut manager, Path::new(".")))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert_eq!(app.state.mode, Mode::Copy);
    assert_eq!(app.state.selection.as_ref().unwrap().pane_id, pane);
    assert!(app.state.selection_autoscroll.is_some());
}

#[test]
fn live_reload_requests_client_reload_for_host_cursor_only_change() {
    use maestro_daemon::settings::model::HostCursorModeConfig;
    let (mut app, mut manager, storage) = fixture();
    put(&*storage, r#"{"multiplexer":{"hostCursor":"native"}}"#);
    assert!(!app.state.request_client_config_reload.0);
    let report = block_on(app.reload_config(&mut manager, Path::new("."))).0;
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert_eq!(app.config.ui.host_cursor, HostCursorModeConfig::Native);
    assert!(app.state.request_client_config_reload.0);
}

#[test]
fn live_reload_updates_sidebar_width_only_when_config_owned() {
    use maestro_daemon::app::state::SidebarWidthSource;
    let (mut app, mut manager, storage) = fixture();
    assert_eq!(
        app.state.sidebar_width_source,
        SidebarWidthSource::ConfigDefault
    );
    put(&*storage, r#"{"multiplexer":{"sidebarWidth":34}}"#);
    assert_eq!(
        block_on(app.reload_config(&mut manager, Path::new(".")))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert_eq!(app.config.ui.sidebar_width, 34);
    assert_eq!(app.state.sidebar_width, 34);
    app.state.sidebar_width = 31;
    app.state.sidebar_width_source = SidebarWidthSource::Manual;
    put(&*storage, r#"{"multiplexer":{"sidebarWidth":35}}"#);
    assert_eq!(
        block_on(app.reload_config(&mut manager, Path::new(".")))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert_eq!(app.config.ui.sidebar_width, 35);
    assert_eq!(app.state.sidebar_width, 31);
}

#[test]
fn live_reload_updates_sidebar_token_rows() {
    use maestro_daemon::settings::sidebar::{AgentSidebarToken, SpaceSidebarToken};
    let (mut app, mut manager, storage) = fixture();
    put(&*storage, &json!({"multiplexer":{"sidebar":{
        "agents":{"rows":[["state_icon", "$summary"]],"rowGap":1,"rowsByAgent":{"claude":[["terminal_title_stripped"]]}},
        "spaces":{"rows":[["workspace", "$jj_status"]],"rowGap":3}
    }}}).to_string());
    app.state.agent_panel_scroll = 5;
    assert_eq!(
        block_on(app.reload_config(&mut manager, Path::new(".")))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert_eq!(app.state.agent_panel_scroll, 0);
    assert_eq!(
        app.config.ui.sidebar.agents.rows,
        vec![vec![
            AgentSidebarToken::StateIcon,
            AgentSidebarToken::Custom("summary".into())
        ]]
    );
    assert_eq!(
        app.config.ui.sidebar.agents.rows_by_agent["claude"],
        vec![vec![AgentSidebarToken::TerminalTitleStripped]]
    );
    assert_eq!(app.config.ui.sidebar.agents.row_gap, 1);
    assert_eq!(
        app.config.ui.sidebar.spaces.rows,
        vec![vec![
            SpaceSidebarToken::Workspace,
            SpaceSidebarToken::Custom("jj_status".into())
        ]]
    );
    assert_eq!(app.config.ui.sidebar.spaces.row_gap, 3);
    let previous = app.config.ui.sidebar.agents.clone();
    put(
        &*storage,
        r#"{"multiplexer":{"sidebar":{"agents":{"rows":[["agent"]],"rowsByAgent":{"claude-code":[["terminal_title"]]}}}}}"#,
    );
    assert_eq!(
        block_on(app.reload_config(&mut manager, Path::new(".")))
            .0
            .status,
        ConfigReloadStatus::Partial
    );
    assert_eq!(app.config.ui.sidebar.agents, previous);
}

#[test]
fn live_reload_does_not_reset_sidebar_to_startup_state() {
    let (mut app, mut manager, storage) = fixture();
    assert!(!app.state.sidebar_collapsed.0);
    put(
        &*storage,
        r#"{"multiplexer":{"sidebarStartCollapsed":true}}"#,
    );
    assert_eq!(
        block_on(app.reload_config(&mut manager, Path::new(".")))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert!(!app.state.sidebar_collapsed.0);
}

#[test]
fn live_reload_updates_sidebar_collapsed_mode() {
    use maestro_daemon::settings::model::SidebarCollapsedModeConfig;
    let (mut app, mut manager, storage) = fixture();
    assert_eq!(
        app.config.ui.sidebar_collapsed_mode,
        SidebarCollapsedModeConfig::Compact
    );
    put(
        &*storage,
        r#"{"multiplexer":{"sidebarCollapsedMode":"hidden"}}"#,
    );
    assert_eq!(
        block_on(app.reload_config(&mut manager, Path::new(".")))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert_eq!(
        app.config.ui.sidebar_collapsed_mode,
        SidebarCollapsedModeConfig::Hidden
    );
}

#[test]
fn live_reload_updates_sidebar_bounds_and_reclamps() {
    use maestro_daemon::app::state::SidebarWidthSource;
    let (mut app, mut manager, storage) = fixture();
    assert_eq!(app.config.ui.sidebar_min_width, 18);
    assert_eq!(app.config.ui.sidebar_max_width, 36);
    assert_eq!(app.config.ui.mobile_width_threshold, 64);
    app.state.sidebar_width = 30;
    app.state.sidebar_width_source = SidebarWidthSource::Manual;
    for (settings, bound, width) in [
        (json!({"sidebarMaxWidth":24}), 24, 24),
        (json!({"sidebarMaxWidth":60}), 60, 24),
        (json!({"sidebarMinWidth":30}), 36, 30),
    ] {
        put(&*storage, &json!({"multiplexer":settings}).to_string());
        assert_eq!(
            block_on(app.reload_config(&mut manager, Path::new(".")))
                .0
                .status,
            ConfigReloadStatus::Applied
        );
        assert_eq!(app.config.ui.sidebar_max_width, bound);
        assert_eq!(app.state.sidebar_width, width);
    }
    assert_eq!(app.config.ui.sidebar_min_width, 30);
}

#[test]
fn live_reload_updates_mobile_width_threshold() {
    let (mut app, mut manager, storage) = fixture();
    assert_eq!(app.config.ui.mobile_width_threshold, 64);
    put(&*storage, r#"{"multiplexer":{"mobileWidthThreshold":96}}"#);
    assert_eq!(
        block_on(app.reload_config(&mut manager, Path::new(".")))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert_eq!(app.config.ui.mobile_width_threshold, 96);
}

#[test]
fn live_reload_invalid_sidebar_bounds_keeps_previous_ui_and_returns_partial() {
    let (mut app, mut manager, storage) = fixture();
    let original = app.config.ui.clone();
    put(
        &*storage,
        r#"{"multiplexer":{"sidebarMinWidth":50,"sidebarMaxWidth":30,"mouseCapture":false}}"#,
    );
    let report = block_on(app.reload_config(&mut manager, Path::new("."))).0;
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|message| message.contains("sidebarMinWidth")
                && message.contains("sidebarMaxWidth")
                && message.contains("greater"))
    );
    assert_eq!(app.config.ui, original);
    assert_eq!(
        app.state.config_diagnostic.as_deref(),
        Some("settings.json; maestro settings check")
    );
}

#[test]
fn live_reload_applies_known_sibling_and_summarizes_unknown_key() {
    let (mut app, mut manager, storage) = fixture();
    put(
        &*storage,
        r#"{"multiplexer":{"mouseCapture":false,"mouseCaptur":false}}"#,
    );
    let report = block_on(app.reload_config(&mut manager, Path::new("."))).0;
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert_eq!(
        report.diagnostics,
        ["unknown config key multiplexer.mouseCaptur; ignoring key"]
    );
    assert!(!app.config.ui.mouse_capture.0);
    assert_eq!(
        app.state.config_diagnostic.as_deref(),
        Some("settings.json has unknown keys; maestro settings check")
    );
}

#[test]
fn live_reload_preserves_invalid_terminal_section_but_applies_valid_ui() {
    let (mut app, mut manager, storage) = fixture();
    let previous = app.config.terminal.clone();
    put(
        &*storage,
        r#"{"daemon":{"paneShell":"nu","paneShellMode":"sideways","newPaneCwd":"home"},"multiplexer":{"toast":{"delivery":"terminal"}}}"#,
    );
    let report = block_on(app.reload_config(&mut manager, Path::new("."))).0;
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|message| message.contains("invalid terminal config"))
    );
    assert_eq!(app.config.terminal, previous);
    assert_eq!(app.state.toast_config.delivery, ToastDelivery::Terminal);
}

#[test]
fn invalid_settings_document_keeps_live_daemon_areas() {
    let (mut app, mut manager, storage) = fixture();
    put(
        &*storage,
        r#"{"daemon":{"paneShell":"kept"},"multiplexer":{"toast":{"delivery":"terminal"}}}"#,
    );
    assert_eq!(
        block_on(app.reload_config(&mut manager, Path::new(".")))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    let previous = app.config.clone();
    put(&*storage, "{invalid");
    let report = block_on(app.reload_config(&mut manager, Path::new("."))).0;
    assert_eq!(report.status, ConfigReloadStatus::Failed);
    assert_eq!(app.config, previous);
    assert_eq!(app.state.toast_config.delivery, ToastDelivery::Terminal);
    assert_eq!(
        app.state.config_diagnostic.as_deref(),
        Some("settings.json invalid; keeping current config; maestro settings check")
    );
    assert!(app.state.toast.is_none());
}

#[test]
fn live_reload_retains_rejected_ui_and_server_while_selecting_valid_theme() {
    let (mut app, mut manager, storage) = fixture();
    put(
        &*storage,
        r#"{"multiplexer":{"accent":"red"},"daemon":{"headlessCols":120,"headlessRows":40}}"#,
    );
    block_on(app.reload_config(&mut manager, Path::new(".")));
    let previous = app.config.ui.clone();
    put(
        &*storage,
        r#"{"multiplexer":{"sidebarMinWidth":50,"sidebarMaxWidth":30,"theme":{"name":"gruvbox"}},"daemon":{"headlessCols":0,"headlessRows":0}}"#,
    );
    let report = block_on(app.reload_config(&mut manager, Path::new("."))).0;
    assert_eq!(report.status, ConfigReloadStatus::Partial);
    assert_eq!(app.config.ui, previous);
    assert_eq!(app.state.headless_size, (120, 40));
    assert_eq!(app.state.theme_name, "gruvbox");
    assert!(app.state.theme_runtime.legacy_accent.is_none());
}

#[test]
fn disk_reload_observation_and_feedback_are_independently_consumed() {
    let (mut app, mut manager, _) = fixture();
    assert!(!app.take_config_reloaded_from_disk());
    assert_eq!(
        block_on(app.apply_config_from_disk(&mut manager, Path::new("."), false))
            .0
            .status,
        ConfigReloadStatus::Applied
    );
    assert!(app.take_config_reloaded_from_disk());
    assert!(!app.take_config_reloaded_from_disk());
    assert!(app.state.toast.is_none());
    assert!(app.config_diagnostic_deadline.is_none());
}

#[test]
fn no_session_reload_does_not_admit_reenabled_background_checks() {
    let (mut app, mut manager, storage) = fixture();
    assert!(app.launch.no_session);
    put(
        &*storage,
        r#"{"update":{"versionCheck":false},"daemon":{"detectionCatalogCheck":false}}"#,
    );
    block_on(app.reload_config(&mut manager, Path::new(".")));
    put(&*storage, "{}");
    block_on(app.reload_config(&mut manager, Path::new(".")));
    assert!(app.config.update.version_check);
    assert!(app.config.update.manifest_check);
    assert!(app.next_auto_update_check.is_none());
    assert!(app.next_agent_manifest_update_check.is_none());
}

#[test]
fn accepted_history_disable_joins_captured_writer_before_clearing_with_supplied_paths() {
    let (mut app, mut manager, storage) = fixture();
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let root = directory.path().to_path_buf();
    let paths = maestro_daemon::paths::DaemonPaths::new(root.to_str().unwrap(), None).unwrap();
    app.config.experimental.pane_history.0 = true;
    std::fs::write(paths.history_path(), "old history").unwrap();
    let snapshot = maestro_daemon::persist::capture(&app.state, &app.terminal_runtimes);
    let expected = serde_json::to_string_pretty(&snapshot).unwrap();
    let history = maestro_daemon::persist::capture_history(
        &app.state.topology.workspaces,
        &app.terminal_runtimes,
    );
    let (release, wait) = std::sync::mpsc::channel();
    let (ready, waiting) = std::sync::mpsc::channel();
    let target = paths.clone();
    app.session_save_thread = Some(std::thread::spawn(move || {
        ready.send(()).unwrap();
        wait.recv().unwrap();
        maestro_daemon::persist::save(&target, &snapshot, Some(&history));
    }));
    waiting.recv().unwrap();
    put(
        &*storage,
        &json!({"daemon":{"paneHistory":false}}).to_string(),
    );
    let report = block_on(app.reload_config(&mut manager, Path::new("."))).0;
    assert_eq!(report.status, ConfigReloadStatus::Applied);
    assert!(!app.config.experimental.pane_history.0);
    release.send(()).unwrap();
    block_on(app.handle_scheduled_tasks(Instant::now(), &paths));
    assert!(app.session_save_thread.is_none());
    assert!(!Path::new(&paths.history_path()).exists());
    block_on(app.run(
        &paths,
        &mut tokio::sync::mpsc::unbounded_channel().1,
        &mut crate::support::api::settings(),
        std::future::ready(()),
    ));
    assert!(!Path::new(&paths.history_path()).exists());
    assert_eq!(
        std::fs::read_to_string(paths.snapshot_path()).unwrap(),
        expected
    );
    assert!(!app.take_config_reloaded_from_disk());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejected_history_setting_and_accepted_enable_preserve_separate_history_and_snapshot() {
    for value in [json!("invalid"), json!(true)] {
        let (mut app, mut manager, storage) = fixture();
        app.config.experimental.pane_history.0 = true;
        let directory = maestro_test_directory::Scratch::new().unwrap();
        let root = directory.path().to_path_buf();
        let paths = maestro_daemon::paths::DaemonPaths::new(root.to_str().unwrap(), None).unwrap();
        std::fs::write(paths.history_path(), "retained history").unwrap();
        std::fs::write(paths.snapshot_path(), "unrelated snapshot").unwrap();
        put(
            &*storage,
            &json!({"daemon":{"paneHistory":value}}).to_string(),
        );
        let report = block_on(app.reload_config(&mut manager, Path::new("."))).0;
        assert_eq!(
            report.status,
            if value.is_string() {
                ConfigReloadStatus::Partial
            } else {
                ConfigReloadStatus::Applied
            }
        );
        assert!(app.config.experimental.pane_history.0);
        block_on(app.run(
            &paths,
            &mut tokio::sync::mpsc::unbounded_channel().1,
            &mut crate::support::api::settings(),
            std::future::ready(()),
        ));
        assert_eq!(
            std::fs::read_to_string(paths.history_path()).unwrap(),
            "retained history"
        );
        assert_eq!(
            std::fs::read_to_string(paths.snapshot_path()).unwrap(),
            "unrelated snapshot"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn accepted_history_disable_clears_without_enabling_session_saves_or_changing_snapshot() {
    for route in ["observed", "run", "shutdown"] {
        let (mut app, mut manager, storage) = fixture();
        let directory = maestro_test_directory::Scratch::new().unwrap();
        let root = directory.path().to_path_buf();
        let paths = maestro_daemon::paths::DaemonPaths::new(root.to_str().unwrap(), None).unwrap();
        std::fs::write(paths.history_path(), "old history").unwrap();
        std::fs::write(paths.snapshot_path(), "unrelated snapshot").unwrap();
        put(
            &*storage,
            &json!({"daemon":{"paneHistory":false}}).to_string(),
        );
        block_on(app.reload_config(&mut manager, Path::new(".")));
        assert!(app.launch.no_session);
        if route == "observed" {
            assert!(app.take_config_reloaded_from_disk());
        }
        if route == "shutdown" {
            block_on(app.shutdown(&paths));
        } else {
            block_on(app.run(
                &paths,
                &mut tokio::sync::mpsc::unbounded_channel().1,
                &mut crate::support::api::settings(),
                std::future::ready(()),
            ));
        }
        assert!(!Path::new(&paths.history_path()).exists());
        assert_eq!(
            std::fs::read_to_string(paths.snapshot_path()).unwrap(),
            "unrelated snapshot"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
