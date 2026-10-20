//! Owned native pane processes and terminal I/O.
mod cwd;
mod handoff;
mod input;
mod launch;
mod lifecycle;
pub use launch::{AgentDetection, SpawnInitialState};
mod reader;
pub use input::FocusEvent;

use super::terminal::PaneTerminal;
use crate::{
    events::AppEvent,
    render_signal::RenderSignal,
    terminal_theme::{HostAppearance, TerminalTheme},
};
use maestro_detection::manifest::ManifestCache;
use maestro_process::pty::{
    PtySetupEntry, PtySize, actor::PtyIoActorHandle, foreground::ProcessDetectionMode,
};
use maestro_workspace::layout::PaneId;
use std::{
    cell::Cell,
    path::PathBuf,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    sync::{Notify, mpsc},
    task::JoinHandle,
};

/// Supplied launch geometry, adapters and workspace delivery owners.
pub struct PaneRuntimeConfig {
    /// Workspace pane identity.
    pub pane_id: PaneId,
    /// Initial terminal geometry.
    pub size: PtySize,
    /// Retained terminal history byte limit.
    pub scrollback_limit_bytes: usize,
    /// Initial host colour palette.
    pub theme: TerminalTheme,
    /// Initial host appearance.
    pub appearance: Option<HostAppearance>,
    /// Workspace event destination.
    pub events: mpsc::Sender<AppEvent>,
    /// Workspace render wakeup.
    pub render_notify: Arc<Notify>,
    /// Shared render origins.
    pub render_dirty: Arc<RenderSignal>,
    /// Replaceable compiled detection snapshot.
    pub manifests: Arc<RwLock<ManifestCache>>,
    /// Native foreground probe policy.
    pub process_detection_mode: ProcessDetectionMode,
    /// Whether newly spawned panes run fallback agent detection.
    pub agent_detection: AgentDetection,
    /// Caller executable implementing terminal child setup.
    pub setup_entry: PtySetupEntry,
    /// Workspace socket advertised to child commands.
    pub socket: PathBuf,
    /// Optional application executable advertised to child commands.
    pub executable: Option<PathBuf>,
}

/// Live pane terminal, process session and owned background work.
pub struct PaneRuntime {
    /// Shared reader observations, with no reference back to this runtime.
    shared: Arc<reader::ReadState>,
    /// Sole terminal I/O handle owned by this runtime.
    io: PtyIoActorHandle,
    /// Current terminal and pixel geometry.
    size: Cell<PtySize>,
    /// Direct child reaping observation; absent for imported descriptors.
    child_wait_completed: Option<Arc<AtomicBool>>,
    /// Child reaper and lossless death publication.
    watcher: Option<JoinHandle<()>>,
    /// Lifecycle authority and release/reset requests.
    control: Arc<super::detection_task::DetectionControl>,
    /// Owned fallback detector, cancelled on runtime teardown.
    detect_handle: Option<JoinHandle<()>>,
    /// Whether session ownership stays with the exporting server.
    preserve_processes_on_drop: bool,
}

impl std::ops::Deref for PaneRuntime {
    type Target = PaneTerminal;
    fn deref(&self) -> &Self::Target {
        &self.shared.terminal
    }
}

impl PaneRuntime {
    /// Stops I/O, terminates the owned session and joins its child watcher.
    pub async fn shutdown(mut self) {
        self.stop_detection().await;
        self.io.shutdown();
        lifecycle::shutdown_processes(
            self.shared.child_pid.load(Ordering::Acquire),
            self.child_wait_completed.as_deref(),
        );
        self.preserve_processes_on_drop = true;
        let _ = self.io.wait_closed().await;
        for task in self.shared.take_delayed() {
            task.abort();
            let _ = task.await;
        }
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.await;
        }
    }
}

impl Drop for PaneRuntime {
    fn drop(&mut self) {
        if let Some(task) = &self.detect_handle {
            task.abort();
        }
        self.io.shutdown();
        for task in self.shared.take_delayed() {
            task.abort();
        }
        if !self.preserve_processes_on_drop {
            lifecycle::shutdown_processes(
                self.shared.child_pid.load(Ordering::Acquire),
                self.child_wait_completed.as_deref(),
            );
        }
    }
}

#[cfg(test)]
mod tests;
