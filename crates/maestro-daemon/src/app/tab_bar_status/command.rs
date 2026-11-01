//! Owned status-command lifetimes using the shared captured-process stream.

use super::text::command_output_text;
use crate::events::AppEvent;
use maestro_process::child_process::{
    CapturedChild, OutputMode, OutputStream, WaitOptions, wait_for_child_process,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{sync::mpsc, task::AbortHandle, time::Instant};

/// Maximum bytes retained for each output line while all bytes are drained.
const MAX_COMMAND_LINE_BYTES: usize = 4096;

/// Launch inputs bound to one configured segment.
pub(super) struct StatusCommandRequest {
    /// Configuration that owns the result.
    pub generation: u64,
    /// Ordered destination segment.
    pub segment_index: usize,
    /// Supplied shell program text.
    pub command: String,
    /// Lifetime measured before the task's first poll.
    pub timeout: Duration,
    /// Active daemon and workspace identities.
    pub environment: Vec<(String, String)>,
    /// Live focused directory, when available.
    pub cwd: Option<PathBuf>,
}

/// A command's cancellation handle and synchronously terminable descendants.
pub(super) struct StatusCommandTask {
    /// Scheduled future, aborted before removing its process group.
    abort: AbortHandle,
    /// Group registration shared across spawn and synchronous configuration reload.
    control: Arc<StatusCommandControl>,
}

impl Drop for StatusCommandTask {
    fn drop(&mut self) {
        self.abort.abort();
        self.control.terminate();
    }
}

/// Cancels both before native spawn and after group registration.
#[derive(Default)]
struct StatusCommandControl {
    /// Prevents a late registration from surviving removal.
    terminated: AtomicBool,
    /// A detached child is its own group leader on Unix.
    process_group: Mutex<Option<u32>>,
}

impl StatusCommandControl {
    /// Removes group ownership and kills every descendant synchronously.
    fn terminate(&self) {
        self.terminated.store(true, Ordering::Release);
        let group = self
            .process_group
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(group) = group {
            kill_group(group);
        }
    }

    /// Publishes the spawned group or immediately kills it after cancellation.
    fn register(&self, group: u32) {
        let mut registered = self
            .process_group
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.terminated.load(Ordering::Acquire) {
            kill_group(group);
        } else {
            *registered = Some(group);
        }
    }
}

/// Terminates the detached process group through the shared native process owner.
fn kill_group(group: u32) {
    #[cfg(unix)]
    let _ = maestro_process::child_process::kill_process_group(group);
    #[cfg(not(unix))]
    let _ = group;
}

/// Starts the deadline now, independently of the executor's first poll.
pub(super) fn spawn_status_command(
    events: mpsc::Sender<AppEvent>,
    request: StatusCommandRequest,
) -> StatusCommandTask {
    let control = Arc::new(StatusCommandControl::default());
    let task_control = Arc::clone(&control);
    let deadline = Instant::now() + request.timeout;
    let task = tokio::spawn(async move {
        let result = run_status_command(&task_control, &request, deadline).await;
        task_control.terminate();
        let _ = events
            .send(AppEvent::TabBarCommandFinished {
                generation: request.generation,
                segment_index: request.segment_index,
                result,
            })
            .await;
    });
    StatusCommandTask {
        abort: task.abort_handle(),
        control,
    }
}

/// Drains the complete output stream while retaining only a bounded last line.
async fn run_status_command(
    control: &StatusCommandControl,
    request: &StatusCommandRequest,
    deadline: Instant,
) -> Result<Option<String>, String> {
    let timeout_error = || format!("timed out after {}s", request.timeout.as_secs());
    if control.terminated.load(Ordering::Acquire) || Instant::now() >= deadline {
        return Err(timeout_error());
    }
    let mut command = maestro_process::command("/bin/sh");
    command
        .args(["-lc", &request.command])
        .envs(request.environment.iter().map(|(key, value)| (key, value)))
        .kill_on_drop(true);
    if let Some(cwd) = &request.cwd {
        command.current_dir(cwd);
    }
    let mut child = CapturedChild::spawn_detached(command).map_err(|error| error.to_string())?;
    let group = child
        .id()
        .ok_or_else(|| "status command has no process id".to_owned())?;
    control.register(group);
    if control.terminated.load(Ordering::Acquire) {
        return Err("status command was cancelled".into());
    }
    let mut output = LastOutputLine::default();
    let mut sink = |stream, bytes: &[u8]| {
        if stream == OutputStream::Stdout {
            output.push(bytes);
        }
    };
    let wait = wait_for_child_process(
        &mut child,
        WaitOptions {
            stderr: OutputMode::Discard,
            post_exit_grace: None,
        },
        std::future::pending(),
        Some(&mut sink),
    );
    let result = tokio::time::timeout_at(deadline, wait)
        .await
        .map_err(|_| timeout_error())?
        .map_err(|error| error.to_string())?;
    if result.status.success() {
        Ok(command_output_text(output.last()))
    } else {
        Err(format!("exited with {}", result.status))
    }
}

/// Last-line accumulator independent of chunk boundaries and total output size.
#[derive(Default)]
struct LastOutputLine {
    /// Bounded bytes since the latest newline.
    current: Vec<u8>,
    /// Bounded most recent terminated line.
    previous: Vec<u8>,
    /// Whether the final observed byte was a newline.
    ended_with_newline: bool,
}

impl LastOutputLine {
    /// Scans each byte once while retaining at most two bounded lines.
    fn push(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == b'\n' {
                self.previous = std::mem::take(&mut self.current);
                self.ended_with_newline = true;
                continue;
            }
            if self.current.len() < MAX_COMMAND_LINE_BYTES {
                self.current.push(byte);
            }
            self.ended_with_newline = false;
        }
    }

    /// Selects the last line under the same trailing-newline rule as text lines.
    fn last(&self) -> &[u8] {
        if self.ended_with_newline {
            &self.previous
        } else {
            &self.current
        }
    }
}

#[cfg(test)]
mod tests;
