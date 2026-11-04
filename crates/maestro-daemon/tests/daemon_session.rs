//! Debounced persistence owns captured jobs and completes the final writer.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::support::{launch, runtime::block_on};

use maestro_daemon::{
    api::event_hub::EventHub, app::App, paths::DaemonPaths, settings::model::Config,
};
use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

/// Constructs an empty daemon without starting a child or reading host settings.
fn app() -> App<fn() -> u128> {
    let manifests = Arc::new(RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = launch::launch_inputs(&[], manifests);
    App::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    )
}

/// Owns an isolated root for one named daemon's persistence paths.
fn paths() -> (maestro_test_directory::Scratch, DaemonPaths) {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let paths = DaemonPaths::new(directory.path().to_str().unwrap(), Some("session-test")).unwrap();
    (directory, paths)
}

#[test]
fn daemon_session_dirty_flag_schedules_debounced_save() {
    let mut app = app();
    app.launch.no_session = false;
    app.state.session_dirty = true;
    app.sync_session_save_schedule();
    assert!(!app.state.session_dirty);
    assert!(app.session_save_deadline.is_some());
}

#[test]
fn daemon_next_loop_deadline_includes_session_save_deadline() {
    let mut app = app();
    let now = Instant::now();
    app.session_save_deadline = Some(now + Duration::from_secs(2));
    app.next_resize_poll = now + Duration::from_secs(5);
    app.next_auto_update_check = Some(now + Duration::from_secs(6));
    assert_eq!(
        app.next_loop_deadline(now, false),
        app.session_save_deadline
    );
}

#[test]
fn daemon_headless_next_loop_deadline_ignores_resize_poll() {
    let mut app = app();
    let now = Instant::now();
    app.next_resize_poll = now + Duration::from_millis(100);
    app.session_save_deadline = Some(now + Duration::from_secs(2));
    app.next_auto_update_check = Some(now + Duration::from_secs(6));
    assert_eq!(
        app.next_headless_loop_deadline(now, false),
        app.session_save_deadline
    );
}

#[test]
fn daemon_headless_next_loop_deadline_returns_none_when_resize_poll_is_only_deadline() {
    let mut app = app();
    let now = Instant::now();
    app.next_resize_poll = now.checked_sub(Duration::from_millis(1)).unwrap();
    assert_eq!(app.next_headless_loop_deadline(now, false), None);
}

#[test]
fn daemon_due_session_save_deadline_is_cleared() {
    let (_directory, paths) = paths();
    let mut app = app();
    app.session_save_deadline = Some(Instant::now().checked_sub(Duration::from_secs(1)).unwrap());
    block_on(app.handle_scheduled_tasks(Instant::now(), &paths));
    assert!(app.session_save_deadline.is_none());
}

#[test]
fn daemon_due_session_save_starts_background_writer() {
    let mut app = app();
    let (_directory, paths) = paths();
    app.launch.no_session = false;
    app.state
        .topology
        .new_with_tab("/autosave".into(), None, |_, _| Ok(()))
        .unwrap();
    app.session_save_deadline = Some(Instant::now().checked_sub(Duration::from_secs(1)).unwrap());
    block_on(app.handle_scheduled_tasks(Instant::now(), &paths));
    assert!(app.session_save_thread.is_some());
    assert!(app.session_save_deadline.is_none());
    app.save_session_now(&paths);
    assert!(std::path::Path::new(&paths.snapshot_path()).exists());
    std::fs::remove_dir_all(paths.data_dir()).unwrap();
}

#[test]
fn daemon_background_session_save_reschedules_when_writer_is_busy() {
    let (_directory, paths) = paths();
    let mut app = app();
    app.launch.no_session = false;
    let (release, wait) = std::sync::mpsc::channel();
    app.session_save_thread = Some(std::thread::spawn(move || {
        wait.recv().unwrap();
    }));
    app.start_background_session_save(&paths);
    assert!(app.session_save_thread.is_some());
    assert!(app.session_save_deadline.is_some());
    release.send(()).unwrap();
    app.launch.no_session = true;
    app.save_session_now(&paths);
}

#[test]
fn daemon_final_session_save_joins_background_writer_before_returning() {
    let (_directory, paths) = paths();
    let mut app = app();
    let (release, wait) = std::sync::mpsc::channel();
    let (done, received) = std::sync::mpsc::channel();
    app.session_save_thread = Some(std::thread::spawn(move || {
        wait.recv().unwrap();
        done.send(()).unwrap();
    }));
    let releaser = std::thread::spawn(move || {
        release.send(()).unwrap();
    });
    app.save_session_now(&paths);
    releaser.join().unwrap();
    received.try_recv().unwrap();
    assert!(app.session_save_thread.is_none());
}

#[test]
fn daemon_hidden_render_attempt_keeps_presentation_cadence_available() {
    let mut app = app();
    let first = Instant::now();
    app.record_render_attempt(first, true);
    let hidden = first + Duration::from_millis(16);
    app.record_render_attempt(hidden, false);
    let echo = hidden + Duration::from_millis(1);
    assert!(!app.can_render_now(echo));
    assert!(app.can_present_now(echo));
}

#[test]
fn run_consumes_due_work_and_queued_observations_before_final_save() {
    let (_directory, paths) = paths();
    let mut app = app();
    let past = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    app.config_diagnostic_deadline = Some(past);
    app.state.config_diagnostic = Some("temporary diagnostic".into());
    app.copy_feedback_deadline = Some(past);
    app.state.copy_feedback = Some(maestro_daemon::app::state::CopyFeedback {
        message: "copied".into(),
    });
    app.launch
        .events
        .try_send(maestro_daemon::events::AppEvent::UpdateReady {
            version: "9.9.9".into(),
            install_command: "maestro update".into(),
        })
        .unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(app.run(
            &paths,
            &mut tokio::sync::mpsc::unbounded_channel().1,
            &mut crate::support::api::settings(),
            std::future::ready(()),
        ));
    assert!(app.state.should_quit.0);
    assert!(app.state.config_diagnostic.is_none());
    assert!(app.state.copy_feedback.is_none());
    assert!(app.copy_feedback_deadline.is_none());
    assert_eq!(app.state.update_available.as_deref(), Some("9.9.9"));
    assert!(app.session_save_thread.is_none());
}

#[test]
fn unchanged_toasts_keep_their_original_deadline_and_expire_exactly_once() {
    let (_directory, paths) = paths();
    let mut app = app();
    app.state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    let update = || maestro_daemon::events::AppEvent::UpdateReady {
        version: "9.9.9".into(),
        install_command: "maestro update".into(),
    };
    app.handle_internal_event(update());
    let deadline = app.toast_deadline.unwrap();
    app.handle_internal_event_with_render_impact(update());
    assert_eq!(app.toast_deadline, Some(deadline));
    assert!(!block_on(app.handle_scheduled_tasks(
        deadline.checked_sub(Duration::from_nanos(1)).unwrap(),
        &paths
    )));
    assert!(app.state.toast.is_some());
    assert!(block_on(app.handle_scheduled_tasks(deadline, &paths)));
    assert!(app.state.toast.is_none());
    assert!(app.toast_deadline.is_none());
    assert!(!block_on(app.handle_scheduled_tasks(deadline, &paths)));
}

#[test]
fn disabled_dirty_saves_stay_idle_and_an_empty_enabled_save_removes_both_files() {
    let mut app = app();
    app.state.session_dirty = true;
    app.sync_session_save_schedule();
    assert!(!app.state.session_dirty);
    assert!(app.session_save_deadline.is_none());
    let (_directory, paths) = paths();
    std::fs::create_dir_all(paths.data_dir()).unwrap();
    for path in [paths.snapshot_path(), paths.history_path()] {
        std::fs::write(path, "old snapshot").unwrap();
    }
    app.launch.no_session = false;
    app.start_background_session_save(&paths);
    app.save_session_now(&paths);
    assert!(!std::path::Path::new(&paths.snapshot_path()).exists());
    assert!(!std::path::Path::new(&paths.history_path()).exists());
    std::fs::remove_dir_all(paths.data_dir()).unwrap();
}

#[test]
fn scheduled_metadata_expiry_publishes_presentation_and_token_changes() {
    use maestro_daemon::terminal::state::AgentMetadataReport;
    let (_directory, paths) = paths();
    let mut app = app();
    // No sidebar row asks for repository status, so no refresh deadline joins the metadata ones.
    app.config.ui.sidebar.spaces.rows = Vec::new();
    let (pane, ()) = app
        .state
        .topology
        .new_with_tab("/expiry".into(), None, |_, _| Ok(()))
        .unwrap();
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let mut terminal =
        maestro_daemon::terminal::state::TerminalState::new(id.clone(), "/expiry".into());
    terminal
        .set_agent_metadata(AgentMetadataReport {
            source: "user".into(),
            agent_label: None,
            applies_to_source: None,
            title: Some("temporary".into()),
            display_agent: None,
            state_labels: std::collections::HashMap::new(),
            clear_title: false,
            clear_display_agent: false,
            clear_state_labels: false,
            ttl: Some(Duration::from_secs(30)),
            seq: None,
        })
        .unwrap();
    let deadline = terminal.next_agent_metadata_expiry().unwrap();
    let now = Instant::now();
    let patch = std::collections::HashMap::from([("status".into(), Some("temporary".into()))]);
    terminal
        .metadata_tokens
        .patch(patch.clone(), Some(Duration::ZERO), deadline);
    app.state.terminals.insert(id.clone(), terminal);
    app.state.topology.workspaces[0]
        .metadata_tokens
        .patch(patch, Some(Duration::ZERO), deadline);
    assert_eq!(app.next_headless_loop_deadline(now, false), Some(deadline));
    assert!(!block_on(app.handle_scheduled_tasks(
        deadline.checked_sub(Duration::from_nanos(1)).unwrap(),
        &paths
    )));
    assert!(block_on(app.handle_scheduled_tasks(deadline, &paths)));
    assert!(app.state.terminals[&id].metadata_tokens.values().is_empty());
    assert!(
        app.state.topology.workspaces[0]
            .metadata_tokens
            .values()
            .is_empty()
    );
    assert_metadata_expiry_events(&app.event_hub);
    assert_eq!(app.next_headless_loop_deadline(deadline, false), None);
}

#[test]
fn scheduled_managed_timeout_publishes_the_changed_pane() {
    use maestro_daemon::terminal::state::{ManagedAgentLaunch, TerminalState};
    let (_directory, paths) = paths();
    let mut app = app();
    let (pane, ()) = app
        .state
        .topology
        .new_with_tab("/managed".into(), None, |_, _| Ok(()))
        .unwrap();
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let now = Instant::now();
    let mut terminal = TerminalState::new(id.clone(), "/managed".into());
    terminal.begin_managed_agent(
        ManagedAgentLaunch {
            name: "worker".into(),
            kind: maestro_detection::agents::Agent::Codex,
            settle_delay: Duration::from_secs(1),
            timeout: Duration::from_secs(2),
        },
        now,
    );
    app.state.terminals.insert(id.clone(), terminal);
    assert!(block_on(
        app.handle_scheduled_tasks(now + Duration::from_secs(2), &paths)
    ));
    assert!(app.state.terminals[&id].agent_name.is_none());
    assert!(
        app.event_hub
            .events_after(0)
            .iter()
            .any(|(_, event)| matches!(event,
                maestro_protocol::events::EventData::PaneUpdated { pane } if pane.pane_id == "w1:p1"
            ))
    );
}

/// Observes all three metadata publications after the scheduled consuming operation.
fn assert_metadata_expiry_events(hub: &EventHub) {
    use maestro_protocol::events::EventData;
    let events = hub.events_after(0);
    assert!(
        events.iter().any(|(_, event)| matches!(
            event,
            EventData::PaneAgentStatusChanged { title: None, .. }
        ))
    );
    assert!(events.iter().any(|(_, event)| matches!(event, EventData::PaneUpdated { pane } if pane.tokens.is_empty() && pane.revision > 0)));
    assert!(events.iter().any(|(_, event)| matches!(event, EventData::WorkspaceMetadataUpdated { workspace } if workspace.tokens.is_empty())));
}

#[test]
fn simultaneous_metadata_and_managed_expiry_still_publishes_the_managed_pane() {
    use maestro_daemon::terminal::state::{AgentMetadataReport, ManagedAgentLaunch, TerminalState};
    let (_directory, paths) = paths();
    let mut app = app();
    let (pane, ()) = app
        .state
        .topology
        .new_with_tab("/combined".into(), None, |_, _| Ok(()))
        .unwrap();
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let mut terminal = TerminalState::new(id.clone(), "/combined".into());
    terminal.begin_managed_agent(
        ManagedAgentLaunch {
            name: "worker".into(),
            kind: maestro_detection::agents::Agent::Codex,
            settle_delay: Duration::ZERO,
            timeout: Duration::ZERO,
        },
        Instant::now(),
    );
    terminal
        .set_agent_metadata(AgentMetadataReport {
            source: "user".into(),
            agent_label: None,
            applies_to_source: None,
            title: Some("temporary".into()),
            display_agent: None,
            state_labels: std::collections::HashMap::new(),
            clear_title: false,
            clear_display_agent: false,
            clear_state_labels: false,
            ttl: Some(Duration::ZERO),
            seq: None,
        })
        .unwrap();
    app.state.terminals.insert(id.clone(), terminal);
    assert!(block_on(app.handle_scheduled_tasks(Instant::now(), &paths)));
    assert!(app.state.terminals[&id].agent_name.is_none());
    assert_eq!(
        app.event_hub
            .events_after(0)
            .iter()
            .filter(|(_, event)| matches!(event,
                maestro_protocol::events::EventData::PaneUpdated { pane } if pane.pane_id == "w1:p1"
            ))
            .count(),
        1
    );
}
