//! Counted work through the daemon's block-move command.
use crate::{
    api::event_hub::EventHub,
    app::{App, construction::PaneLaunchInputs},
    paths::DaemonPaths,
    render_signal::RenderSignal,
    settings::model::Config,
    terminal_theme::TerminalTheme,
    tests::{WORKSPACE_WORK, block_on},
};
use maestro_detection::manifest::ManifestCache;
use maestro_process::pty::{PtySetupEntry, foreground::ProcessDetectionMode};
use maestro_protocol::schema::ResponseResult;
use maestro_settings::{InMemorySettingsStorage, SettingsManager};
use std::sync::{Arc, RwLock};
use tokio::sync::{Notify, mpsc};

/// Builds a daemon with topology only; this command starts no child process.
fn daemon(count: usize) -> App<fn() -> u128> {
    let config = Config::default();
    let (events, receiver) = mpsc::channel(8);
    let launch = PaneLaunchInputs {
        no_session: true,
        setup_entry: PtySetupEntry {
            executable: std::env::current_exe().unwrap(),
            args: Vec::new(),
        },
        socket: "/nonexistent-maestro-count.sock".into(),
        executable: None,
        manifests: Arc::new(RwLock::new(ManifestCache::new(None, None).unwrap())),
        events,
        render_notify: Arc::new(Notify::new()),
        render_dirty: Arc::new(RenderSignal::new()),
        theme: TerminalTheme::default(),
        appearance: None,
        process_detection_mode: ProcessDetectionMode::Native,
    };
    let mut app = App::new(
        &config,
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    );
    for index in 0..count {
        app.state
            .topology
            .new_with_tab(format!("/w{index}").into(), None, |_, _| Ok(()))
            .unwrap();
    }
    app.state.active = Some(0);
    app
}

#[test]
fn workspace_order_resolution_has_linear_work() {
    for count in [128, 256, 512] {
        let mut app = daemon(count);
        let ids: Vec<_> = app
            .state
            .topology
            .workspaces
            .iter()
            .step_by(2)
            .rev()
            .map(|workspace| workspace.id.clone())
            .collect();
        WORKSPACE_WORK.set(0);
        for id in &ids {
            assert!(app.state.parse_workspace_id(id).is_some());
        }
        assert!(
            WORKSPACE_WORK.get() > count * count / 8,
            "rescan negative control"
        );
        let mut expected: Vec<_> = app
            .state
            .topology
            .workspaces
            .iter()
            .skip(1)
            .step_by(2)
            .map(|workspace| workspace.id.clone())
            .collect();
        expected.extend(ids.iter().cloned());
        let command = serde_json::from_value(serde_json::json!({
            "type": "workspace.move_block", "workspace_ids": ids
        }))
        .unwrap();
        let mut settings = SettingsManager::from_storage(Arc::new(InMemorySettingsStorage::new()));
        let paths = DaemonPaths::new("/nonexistent-maestro-count", None).unwrap();
        WORKSPACE_WORK.set(0);
        let response = block_on(app.handle_api_request(command, &mut settings, &paths)).unwrap();
        let work = WORKSPACE_WORK.get();
        assert!(work >= count, "counter must observe the actual command");
        assert!(work <= 16 * count, "{count} workspaces took {work} visits");
        let ResponseResult::WorkspaceList { workspaces } = response else {
            panic!("workspace list required")
        };
        assert_eq!(
            workspaces
                .into_iter()
                .map(|workspace| workspace.workspace_id)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            app.state.topology.workspaces[app.state.active.unwrap()].id,
            "w1"
        );
        assert_eq!(app.state.topology.workspaces[app.state.selected].id, "w1");
    }
}
