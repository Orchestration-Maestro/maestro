//! Shared native launch adapters for daemon-owner tests.

use maestro_daemon::{
    app::construction::PaneLaunchInputs, events::AppEvent, render_signal::RenderSignal,
    terminal_theme::TerminalTheme,
};
use maestro_process::pty::{PtySetupEntry, foreground::ProcessDetectionMode};
use maestro_test_directory::temporary_root;
use std::{ffi::OsString, sync::Arc};
use tokio::sync::{Notify, mpsc};

/// Supplies the same existing pane runtime boundary used by native launches.
pub fn launch_inputs(
    args: &[String],
    manifests: Arc<std::sync::RwLock<maestro_detection::manifest::ManifestCache>>,
) -> (PaneLaunchInputs, mpsc::Receiver<AppEvent>) {
    let (events, receiver) = mpsc::channel(8);
    (
        PaneLaunchInputs {
            no_session: true,
            setup_entry: PtySetupEntry {
                executable: std::env::current_exe().unwrap(),
                args: args.iter().map(OsString::from).collect(),
            },
            socket: temporary_root().join("maestro-context-test.sock"),
            executable: None,
            manifests,
            events,
            render_notify: Arc::new(Notify::new()),
            render_dirty: Arc::new(RenderSignal::new()),
            theme: TerminalTheme::default(),
            appearance: None,
            process_detection_mode: ProcessDetectionMode::Native,
        },
        receiver,
    )
}
