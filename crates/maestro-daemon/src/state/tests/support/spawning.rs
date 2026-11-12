//! Native launch adapters for the daemon-owner tests.

use crate::{
    events::AppEvent, render_signal::RenderSignal, state::construction::PaneLaunchInputs,
    terminal_theme::TerminalTheme,
};
use maestro_process::terminal::{PtySetupEntry, jobs::ProcessDetectionMode};
use std::{ffi::OsString, sync::Arc};
use tokio::sync::{Notify, mpsc};

/// Supplies the same existing pane runtime boundary used by native launches.
pub(crate) fn launch_inputs(
    args: &[String],
    manifests: Arc<std::sync::RwLock<maestro_detection::definitions::ManifestCache>>,
) -> (PaneLaunchInputs, mpsc::Receiver<AppEvent>) {
    let (events, receiver) = mpsc::channel(8);
    (
        PaneLaunchInputs {
            no_session: true,
            setup_entry: PtySetupEntry {
                executable: std::env::current_exe().unwrap(),
                args: args.iter().map(OsString::from).collect(),
            },
            socket: "/nonexistent/maestro-daemon-state.sock".into(),
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
