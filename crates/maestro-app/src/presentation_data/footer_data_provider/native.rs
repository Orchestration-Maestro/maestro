//! Native file-system and Git process effects.

use maestro_process::child_process::{CapturedChild, WaitOptions, wait_for_child_process};
use maestro_process::{OutputMode, SpawnSyncError, SpawnSyncOptions, spawn_sync};
use std::io;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::{rc::Rc, time::Duration};
use tokio::process::Command;

use super::operations::{FooterFileStats, FooterOperations};
use maestro_watch::fs_watch::{FsWatcher, NativeWatchOperations, WatchOperations, WatchTimer};
use num_traits::ToPrimitive;
use tokio::task::{JoinHandle, LocalSet};

/// Footer effects on the local file system and the `git` command.
pub struct NativeFooterOperations {
    /// Caller-driven native task ownership.
    local: Rc<LocalSet>,
    /// Shared directory notifications and timers.
    watch: NativeWatchOperations,
}
impl NativeFooterOperations {
    /// Supply the task set that the caller drives for native effects.
    #[must_use]
    pub fn new(local: Rc<LocalSet>) -> Self {
        Self {
            watch: NativeWatchOperations::new(Rc::clone(&local)),
            local,
        }
    }
}
impl WatchOperations for NativeFooterOperations {
    fn watch(
        &self,
        path: &str,
        listener: Rc<dyn Fn(Option<String>)>,
        on_error: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn FsWatcher>> {
        self.watch.watch(path, listener, on_error)
    }
    fn schedule(&self, delay: Duration, callback: Box<dyn FnOnce()>) -> Box<dyn WatchTimer> {
        self.watch.schedule(delay, callback)
    }
}
/// Read the three metadata dimensions selected by footer polling.
fn stats(path: &Path) -> Option<FooterFileStats> {
    std::fs::metadata(path).ok().map(|m| {
        let (mtime_ms, ctime_ms) = times(&m);
        FooterFileStats {
            mtime_ms,
            ctime_ms,
            size: m.len().to_f64().unwrap_or_default(),
        }
    })
}
/// The modification and change times in milliseconds.
#[cfg(unix)]
fn times(m: &std::fs::Metadata) -> (f64, f64) {
    (
        millis(m.mtime(), m.mtime_nsec()),
        millis(m.ctime(), m.ctime_nsec()),
    )
}
/// The modification time in milliseconds, twice.
#[cfg(windows)]
fn times(m: &std::fs::Metadata) -> (f64, f64) {
    let since = |time: Duration| {
        millis(
            time.as_secs().to_i64().unwrap_or_default(),
            i64::from(time.subsec_nanos()),
        )
    };
    let modified = m.modified().map_or(0.0, |time| {
        match time.duration_since(std::time::UNIX_EPOCH) {
            Ok(after) => since(after),
            Err(before) => -since(before.duration()),
        }
    });
    // shortcut: stable Rust has no Windows change time; use the modification time until it does.
    (modified, modified)
}
/// Convert timestamp components since the Unix epoch without truncating fractional milliseconds.
fn millis(seconds: i64, nanos: i64) -> f64 {
    seconds.to_f64().unwrap_or_default() * 1000.0 + nanos.to_f64().unwrap_or_default() / 1_000_000.0
}
/// One polling registration; close and drop both revoke its task.
struct Polling(JoinHandle<()>);
impl Drop for Polling {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl FsWatcher for Polling {
    fn close(&mut self) -> io::Result<()> {
        self.0.abort();
        Ok(())
    }
}

impl FooterOperations for NativeFooterOperations {
    fn symbolic_ref_async(
        &self,
        repo_dir: &str,
        callback: Box<dyn FnOnce(io::Result<Option<String>>)>,
    ) {
        let repo_dir = anchor(repo_dir);
        self.local.spawn_local(async move {
            let mut command = symbolic_command(&repo_dir);
            let output = async {
                let mut child = CapturedChild::spawn(&mut command)?;
                wait_for_child_process(
                    &mut child,
                    WaitOptions {
                        stderr: OutputMode::Discard,
                        post_exit_grace: None,
                    },
                    std::future::pending(),
                    None,
                )
                .await
            }
            .await;
            callback(output.map(|output| {
                output
                    .status
                    .success()
                    .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
            }));
        });
    }
    fn watch_file(
        &self,
        path: &str,
        interval: Duration,
        listener: Rc<dyn Fn(FooterFileStats, FooterFileStats)>,
    ) -> Box<dyn FsWatcher> {
        Box::new(Polling(poll(&self.local, path, interval, listener)))
    }

    fn symbolic_ref_sync(&self, repo_dir: &str) -> io::Result<Option<String>> {
        let output = spawn_sync(
            &mut symbolic_command(Path::new(repo_dir)),
            SpawnSyncOptions::Capture {
                max_buffer_bytes: None,
                stdout: OutputMode::Capture,
                input: None,
                timeout: None,
                stderr: OutputMode::Discard,
            },
        )
        .map_err(|error| match error {
            SpawnSyncError::Infrastructure(error)
            | SpawnSyncError::Spawn(error)
            | SpawnSyncError::Wait(error) => error,
            SpawnSyncError::Timeout => io::Error::other("Git lookup timed out"),
        })?;
        Ok(output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned()))
    }
}

/// Prepare the exact Git operands on the caller-selected directory.
fn symbolic_command(repo_dir: &Path) -> Command {
    let mut command = maestro_process::command("git");
    command
        .arg("--no-optional-locks")
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .current_dir(repo_dir);
    command
}

/// Bind an effect's path now, retaining lexical fallback when resolution fails.
fn anchor(path: &str) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.into())
}

/// Start one stat poll on the registration-time absolute path, with its metadata as the previous sample.
fn poll(
    local: &LocalSet,
    path: &str,
    interval: Duration,
    listener: Rc<dyn Fn(FooterFileStats, FooterFileStats)>,
) -> JoinHandle<()> {
    let path = anchor(path);
    let mut last = stats(&path);
    let mut previous = last.unwrap_or_default();
    local.spawn_local(async move {
        let mut ticks = tokio::time::interval_at(tokio::time::Instant::now() + interval, interval);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticks.tick().await;
            let current = stats(&path);
            if current == last {
                continue;
            }
            listener(current.unwrap_or_default(), previous);
            tokio::task::yield_now().await;
            previous = current.unwrap_or(previous);
            last = current;
        }
    })
}
