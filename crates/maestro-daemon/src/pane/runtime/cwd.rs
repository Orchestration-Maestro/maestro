//! Directory observations preserve their native spelling and binding time.
use super::PaneRuntime;
use maestro_process::pty::foreground::{self, ProcessDetectionMode};
use std::{path::PathBuf, sync::atomic::Ordering};
use tokio::task::JoinError;

/// Reads an absolute native process directory without a filesystem traversal check.
fn absolute_process_cwd(pid: u32) -> Option<PathBuf> {
    foreground::process_cwd(pid).filter(|cwd| cwd.is_absolute())
}

impl PaneRuntime {
    /// Direct child identity retained for native session observations.
    #[must_use]
    pub fn child_pid(&self) -> Option<u32> {
        let pid = self.shared.child_pid.load(Ordering::Acquire);
        (pid > 0).then_some(pid)
    }

    /// Runs `read` over the direct child on the blocking pool, where the synchronous process
    /// reads belong; the answer is `None` while the pane has no child.
    ///
    /// # Errors
    /// Returns the join error when `read` panics or the runtime shuts down first.
    pub async fn observe_processes<T: Send + 'static>(
        &self,
        read: impl FnOnce(u32, ProcessDetectionMode) -> T + Send + 'static,
    ) -> Result<Option<T>, JoinError> {
        let Some(pid) = self.child_pid() else {
            return Ok(None);
        };
        let mode = self.shared.process_detection_mode;
        tokio::task::spawn_blocking(move || read(pid, mode))
            .await
            .map(Some)
    }

    /// Returns an admitted report first, without checking that directory again.
    #[must_use]
    pub fn cwd(&self) -> Option<PathBuf> {
        self.shared
            .reported_cwd
            .lock()
            .ok()
            .and_then(|cwd| cwd.clone())
            .or_else(|| self.child_pid().and_then(foreground::process_cwd))
    }

    /// Uses a traversable foreground leader directory, falling back to the pane cwd.
    pub async fn follow_cwd(&self) -> Option<PathBuf> {
        self.io
            .foreground_process_group_id()
            .await
            .and_then(absolute_process_cwd)
            .filter(|cwd| cwd.is_dir())
            .or_else(|| self.cwd())
    }

    /// Finds a foreground directory differing from the shell when a member has one.
    pub async fn foreground_cwd(&self) -> Option<PathBuf> {
        let pid = self.child_pid()?;
        let shell = absolute_process_cwd(pid);
        let leader = self
            .io
            .foreground_process_group_id()
            .await
            .or_else(|| foreground::foreground_process_group_id(pid))
            .and_then(absolute_process_cwd);
        let member = || {
            foreground::foreground_job(pid, self.shared.process_detection_mode)?
                .processes
                .into_iter()
                .filter(|process| process.pid != pid)
                .filter_map(|process| absolute_process_cwd(process.pid))
                .find(|cwd| shell.as_ref() != Some(cwd))
        };
        if leader == shell {
            member().or(leader)
        } else {
            leader.or_else(member)
        }
    }
}
