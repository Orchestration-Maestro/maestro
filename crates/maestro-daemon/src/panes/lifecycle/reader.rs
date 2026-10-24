//! One live read path shared by spawned and imported pane terminals.
use super::PaneRuntimeConfig;
use crate::{events::AppEvent, panes::display::PaneTerminal, render_signal::RenderSignal};
use maestro_process::terminal::{channels::PtyIoActorHandle, jobs::ProcessDetectionMode};
use maestro_process::terminal::{
    channels::{PtyIoActor, PtyIoActorConfig, PtyReadResult},
    jobs as foreground,
};
use maestro_workspace::tiling::PaneId;
use std::os::fd::OwnedFd;
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
};
use tokio::{
    sync::{Notify, mpsc},
    task::JoinHandle,
};

/// Terminal effects and workspace delivery shared with the native reader.
pub(super) struct ReadState {
    /// Pane receiving terminal effects.
    pub(super) pane_id: PaneId,
    /// Concrete terminal policy owner.
    pub(super) terminal: Arc<PaneTerminal>,
    /// Direct child session identity.
    pub(super) child_pid: Arc<AtomicU32>,
    /// Last admitted absolute directory report.
    pub(super) reported_cwd: Mutex<Option<PathBuf>>,
    /// Odd during an output transaction, even after publication.
    pub(super) content_seq: AtomicU64,
    /// Revision of detection-relevant content.
    pub(super) detection_content_seq: Arc<AtomicU64>,
    /// Workspace event queue.
    pub(super) events: mpsc::Sender<AppEvent>,
    /// Workspace render notification.
    pub(super) render_notify: Arc<Notify>,
    /// Coalesced render origins.
    pub(super) render_dirty: Arc<RenderSignal>,
    /// Pending render backstops, without captures of this owner.
    pub(super) delayed: Mutex<Vec<JoinHandle<()>>>,
    /// Native foreground lookup policy.
    pub(super) process_detection_mode: ProcessDetectionMode,
}

impl ReadState {
    /// Shares only the reader's concrete terminal and delivery owners.
    pub(super) fn new(config: &PaneRuntimeConfig, terminal: Arc<PaneTerminal>, pid: u32) -> Self {
        Self {
            pane_id: config.pane_id,
            terminal,
            child_pid: Arc::new(AtomicU32::new(pid)),
            reported_cwd: Mutex::new(None),
            content_seq: AtomicU64::new(0),
            detection_content_seq: Arc::new(AtomicU64::new(0)),
            events: config.events.clone(),
            render_notify: Arc::clone(&config.render_notify),
            render_dirty: Arc::clone(&config.render_dirty),
            delayed: Mutex::new(Vec::new()),
            process_detection_mode: config.process_detection_mode,
        }
    }

    /// Drains delayed work so cancellation and joins happen outside the guard.
    pub(super) fn take_delayed(&self) -> Vec<JoinHandle<()>> {
        std::mem::take(
            &mut *self
                .delayed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    /// Applies raw output once, retaining response order independently of user input.
    fn read(&self, bytes: &[u8]) -> PtyReadResult {
        self.content_seq.fetch_add(1, Ordering::AcqRel);
        let pid = self.child_pid.load(Ordering::Acquire);
        let result = self
            .terminal
            .process_pty_bytes(self.pane_id, pid, bytes, || {
                foreground::foreground_job(pid, self.process_detection_mode)
            });
        self.content_seq.fetch_add(1, Ordering::Release);
        super::super::agent_detection::observe_detection_content_change(
            bytes,
            &self.detection_content_seq,
        );
        if result.terminal_bells > 0 {
            self.publish_best_effort(AppEvent::TerminalBell {
                pane_id: self.pane_id,
                count: result.terminal_bells,
            });
        }
        let title =
            result.terminal_title_changed && self.render_dirty.request_terminal_title(self.pane_id);
        let render = result.request_render && self.render_dirty.request_pty(self.pane_id);
        if title || render {
            self.render_notify.notify_one();
        }
        if let Some(delay) = result.render_delay {
            self.schedule_render(delay);
        }
        if let Some(cwd) = result.reported_cwd {
            self.publish_cwd(cwd);
        }
        for content in result.clipboard_writes {
            self.publish_best_effort(AppEvent::ClipboardWrite { content });
        }
        PtyReadResult {
            terminal_responses: result.terminal_responses,
        }
    }

    /// Retains a valid report once without rechecking it on subsequent cwd reads.
    pub(super) fn publish_cwd(&self, cwd: PathBuf) {
        if !cwd.is_absolute() || !cwd.is_dir() {
            return;
        }
        {
            let mut current = self
                .reported_cwd
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if current.as_ref() == Some(&cwd) {
                return;
            }
            *current = Some(cwd.clone());
        }
        self.publish_best_effort(AppEvent::TerminalCwdReported {
            pane_id: self.pane_id,
            cwd,
        });
    }

    /// Publishes reader metadata without blocking terminal byte delivery.
    fn publish_best_effort(&self, event: AppEvent) {
        if let Err(error) = self.events.try_send(event) {
            tracing::warn!(%error, "failed to queue terminal metadata");
        }
    }

    /// Uses the terminal's existing render-settle delay without retaining the runtime.
    fn schedule_render(&self, delay: std::time::Duration) {
        let notify = Arc::clone(&self.render_notify);
        let dirty = Arc::clone(&self.render_dirty);
        let pane = self.pane_id;
        let task = tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            if dirty.request_pty(pane) {
                notify.notify_one();
            }
        });
        self.retain_task(task);
    }

    /// Keeps unfinished delayed work for cancellation and joins outside the guard.
    pub(super) fn retain_task(&self, task: JoinHandle<()>) {
        let finished: Vec<_> = {
            let mut tasks = self
                .delayed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            tasks.push(task);
            tasks.extract_if(.., |task| task.is_finished()).collect()
        };
        drop(finished);
    }
}

/// Installs the same byte policy on spawned and handed-off descriptors.
pub(super) fn spawn_actor(
    fd: OwnedFd,
    shared: Arc<ReadState>,
    imported: bool,
) -> io::Result<PtyIoActorHandle> {
    let exit = Arc::clone(&shared);
    PtyIoActor::spawn(PtyIoActorConfig {
        pane_id: shared.pane_id.raw(),
        master_fd: fd,
        initially_quiesced: imported,
        on_read: Box::new(move |bytes| shared.read(bytes)),
        on_reader_exit: imported.then(|| {
            Box::new(
                move || -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
                    Box::pin(async move {
                        AppEvent::PaneDied {
                            pane_id: exit.pane_id,
                        }
                        .publish(&exit.events)
                        .await;
                    })
                },
            ) as _
        }),
    })
}
