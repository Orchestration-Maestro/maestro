//! Debounced owned session jobs through the existing snapshot storage owner.

use super::App;
use crate::{paths::DaemonPaths, persist};
use maestro_workspace::persist::snapshot::{SessionHistorySnapshot, SessionSnapshot};
use std::time::{Duration, Instant};

/// Delay after a dirty observation before capturing the current session.
const SESSION_SAVE_DEBOUNCE: Duration = Duration::from_secs(5);
/// Retry when a previous captured job still owns the writer.
const SESSION_SAVE_BUSY_RETRY: Duration = Duration::from_millis(250);

/// Captured work independent of the mutable daemon and its process registry.
enum SessionSaveJob {
    /// No workspaces remain, so remove persisted structure and history.
    Clear,
    /// Owned structure and optional separate history.
    Save {
        /// Captured workspace and pane metadata.
        snapshot: SessionSnapshot,
        /// Optional primary-screen history captured with the same job.
        history: Option<SessionHistorySnapshot>,
    },
}

impl<C: FnMut() -> u128> App<C> {
    /// Settles captured history before clearing an accepted persistence disable.
    pub(super) fn finish_history_reload(&mut self, paths: &DaemonPaths) {
        self.take_config_reloaded_from_disk();
        if std::mem::take(&mut self.clear_history_after_reload) {
            if let Some(thread) = self.session_save_thread.take() {
                let _ = thread.join();
            }
            persist::clear_history(paths);
        }
    }

    /// Debounces dirty observations without scheduling disabled persistence.
    pub fn sync_session_save_schedule(&mut self) {
        if std::mem::take(&mut self.state.session_dirty) && !self.launch.no_session {
            self.session_save_deadline = Some(Instant::now() + SESSION_SAVE_DEBOUNCE);
        }
    }

    /// Captures the current session without letting the writer borrow daemon state.
    fn capture_session_save_job(&self) -> SessionSaveJob {
        if self.state.topology.workspaces.is_empty() {
            SessionSaveJob::Clear
        } else {
            SessionSaveJob::Save {
                snapshot: persist::capture(&self.state, &self.terminal_runtimes),
                history: self.config.experimental.pane_history.0.then(|| {
                    persist::capture_history(
                        &self.state.topology.workspaces,
                        &self.terminal_runtimes,
                    )
                }),
            }
        }
    }

    /// Starts one captured writer or retains later dirty work while the writer is busy.
    pub fn start_background_session_save(&mut self, paths: &DaemonPaths) {
        if self.launch.no_session {
            self.session_save_deadline = None;
            return;
        }
        if self
            .session_save_thread
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
            && let Some(thread) = self.session_save_thread.take()
        {
            let _ = thread.join();
        }
        if self.session_save_thread.is_some() {
            self.session_save_deadline = Some(Instant::now() + SESSION_SAVE_BUSY_RETRY);
            return;
        }
        let job = self.capture_session_save_job();
        let target = paths.clone();
        self.session_save_deadline = None;
        match std::thread::Builder::new()
            .name("maestro-session-save".into())
            .spawn(move || run_session_save_job(&target, job))
        {
            Ok(thread) => self.session_save_thread = Some(thread),
            Err(error) => {
                tracing::warn!(%error, "failed to spawn session save thread; saving inline");
                run_session_save_job(paths, self.capture_session_save_job());
            }
        }
    }

    /// Joins the existing writer even when persistence is disabled, then saves current state.
    pub fn save_session_now(&mut self, paths: &DaemonPaths) {
        if let Some(thread) = self.session_save_thread.take() {
            let _ = thread.join();
        }
        if !self.launch.no_session {
            run_session_save_job(paths, self.capture_session_save_job());
        }
        self.session_save_deadline = None;
    }
}

/// Performs a captured job through the single snapshot effect owner.
fn run_session_save_job(paths: &DaemonPaths, job: SessionSaveJob) {
    match job {
        SessionSaveJob::Clear => persist::clear(paths),
        SessionSaveJob::Save { snapshot, history } => {
            persist::save(paths, &snapshot, history.as_ref());
        }
    }
}
