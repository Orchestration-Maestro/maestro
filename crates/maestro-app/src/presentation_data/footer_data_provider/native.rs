//! Unix file-system and Git process effects.

use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::{rc::Rc, time::Duration};

use super::operations::{FooterFileKind, FooterFileStats, FooterOperations};
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
    std::fs::metadata(path).ok().map(|m| FooterFileStats {
        mtime_ms: millis(m.mtime(), m.mtime_nsec()),
        ctime_ms: millis(m.ctime(), m.ctime_nsec()),
        size: m.len().to_f64().unwrap_or_default(),
    })
}
/// Convert Unix timestamp components without truncating fractional milliseconds.
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
        let repo_dir = repo_dir.to_owned();
        self.local.spawn_local(async move {
            let output = tokio::process::Command::new("git")
                .arg("--no-optional-locks")
                .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
                .current_dir(repo_dir)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
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

    fn exists(&self, path: &str) -> bool {
        fs::exists(path).unwrap_or(false)
    }

    fn stat_kind(&self, path: &str) -> io::Result<FooterFileKind> {
        let kind = fs::metadata(path)?.file_type();
        Ok(match (kind.is_file(), kind.is_dir()) {
            (true, _) => FooterFileKind::File,
            (_, true) => FooterFileKind::Directory,
            _ => FooterFileKind::Other,
        })
    }

    fn read_text(&self, path: &str) -> io::Result<String> {
        Ok(String::from_utf8_lossy(&fs::read(path)?).into_owned())
    }

    fn current_dir(&self) -> io::Result<String> {
        Ok(std::env::current_dir()?.to_string_lossy().into_owned())
    }

    fn drive_directory(&self, _drive: char) -> Option<String> {
        None
    }

    fn symbolic_ref_sync(&self, repo_dir: &str) -> io::Result<Option<String>> {
        let output = Command::new("git")
            .arg("--no-optional-locks")
            .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
            .current_dir(repo_dir)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()?;
        Ok(output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned()))
    }
}

/// Start one stat poll on the registration-time absolute path, with its metadata as the previous sample.
fn poll(
    local: &LocalSet,
    path: &str,
    interval: Duration,
    listener: Rc<dyn Fn(FooterFileStats, FooterFileStats)>,
) -> JoinHandle<()> {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.into());
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
