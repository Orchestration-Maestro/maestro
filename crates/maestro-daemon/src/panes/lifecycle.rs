//! Owned native pane processes and terminal I/O.
mod cwd;
mod handoff;
mod input;
mod launch;
mod shutdown;
pub use launch::{AgentDetection, SpawnInitialState};
mod reader;
pub use input::FocusEvent;

use super::display::PaneTerminal;
use crate::{
    events::AppEvent,
    render_signal::RenderSignal,
    terminal_theme::{HostAppearance, TerminalTheme},
};
use maestro_detection::definitions::ManifestCache;
use maestro_process::terminal::{
    PtySetupEntry, PtySize, channels::PtyIoActorHandle, jobs::ProcessDetectionMode,
};
use maestro_workspace::tiling::PaneId;
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
    control: Arc<super::monitoring::DetectionControl>,
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
    /// Projects the live cursor into a visible viewport, clipping outside its bounds.
    #[must_use]
    pub fn cursor_state(
        &self,
        area: maestro_workspace::tiling::Rect,
        show_cursor: bool,
    ) -> Option<super::display::TerminalCursorState> {
        if !show_cursor {
            return None;
        }
        let cursor = self
            .shared
            .terminal
            .cursor_state(std::time::Instant::now())?;
        if cursor.x >= area.width || cursor.y >= area.height {
            return None;
        }
        Some(super::display::TerminalCursorState {
            x: area.x.checked_add(cursor.x)?,
            y: area.y.checked_add(cursor.y)?,
            ..cursor
        })
    }

    /// Returns nonzero total pixel dimensions when both products fit.
    #[must_use]
    pub fn pixel_size(&self) -> Option<(u32, u32)> {
        let size = self.size.get();
        let width = u32::from(size.cols).checked_mul(size.cell_width_px)?;
        let height = u32::from(size.rows).checked_mul(size.cell_height_px)?;
        (width > 0 && height > 0).then_some((width, height))
    }

    /// Stops I/O and terminates the owned session without waiting for event capacity.
    pub async fn shutdown(mut self) {
        let _ = self.begin_shutdown().await;
    }

    /// Stops native work while the daemon retains this runtime until exit delivery completes.
    pub(crate) async fn begin_shutdown(&mut self) -> Option<JoinHandle<()>> {
        self.stop_detection().await;
        self.io.shutdown();
        shutdown::shutdown_processes(
            self.shared.child_pid.load(Ordering::Acquire),
            self.child_wait_completed.as_deref(),
        );
        self.preserve_processes_on_drop = true;
        for task in self.shared.take_delayed() {
            task.abort();
            let _ = task.await;
        }
        self.watcher.take()
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
            shutdown::shutdown_processes(
                self.shared.child_pid.load(Ordering::Acquire),
                self.child_wait_completed.as_deref(),
            );
        }
    }
}

#[cfg(test)]
mod tests;
