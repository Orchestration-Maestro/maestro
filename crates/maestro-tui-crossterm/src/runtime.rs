//! The production host of the terminal toolkit on a caller-driven Tokio [`LocalSet`].

use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, SecondsFormat, Utc};
use maestro_tui::terminal::OutputErrorHandler;
use maestro_tui::tui::{LocalFuture, LogContext, RenderCallback, RenderTimer, TuiRuntime};
use tokio::task::{JoinHandle, LocalSet};
use tokio::time::{Instant, sleep_until};

use crate::output_errors::{FailureSource, OutputErrors};

/// Runs the toolkit's timers and futures on the [`LocalSet`] its caller drives.
///
/// Available on every native target; `ProcessTerminal` exists on Unix only. Creating the host
/// starts no runtime, thread or task. Work runs only while the caller drives the set inside a
/// Tokio runtime with the time driver enabled, and dropping the set drops the work still
/// pending. The tasks the host spawns hold no handle to the host itself; what a caller's
/// callback or future captures stays alive until that work finishes, is cancelled or is
/// dropped with the set.
///
/// [`schedule`](TuiRuntime::schedule) measures its delay from the call, and the callback runs
/// once, never inline. Dropping a timer handle leaves the callback scheduled;
/// [`cancel`](RenderTimer::cancel) prevents it, and its captures are released once the set
/// next processes the cancellation. A callback or future that returns an error has the
/// `Display` text of the error and then of each source error written to standard error, each
/// followed by a newline; the host then keeps running. A failure to write there reaches the
/// handlers [`on_output_error`](TuiRuntime::on_output_error) registered, once, after the lock
/// on standard error is released.
///
/// [`log_context`](TuiRuntime::log_context) reports the process's home directory (empty when
/// the platform has none), the current UTC time and a random lowercase hexadecimal nonce.
/// The log operations act on the given path as the operating system resolves it, so the
/// errors they return are the operating system's.
///
/// ```no_run
/// # #[cfg(unix)] {
/// use std::rc::Rc;
///
/// use maestro_tui_crossterm::{ProcessTerminal, ProcessTuiRuntime};
/// use tokio::task::LocalSet;
///
/// let local = Rc::new(LocalSet::new());
/// let runtime = ProcessTuiRuntime::new(Rc::clone(&local));
/// let terminal = ProcessTerminal::new(local);
/// # drop((runtime, terminal));
/// # }
/// ```
pub struct ProcessTuiRuntime {
    /// The set every timer and future runs on.
    local: Rc<LocalSet>,
    /// The instant [`now`](TuiRuntime::now) counts from.
    origin: Instant,
    /// The callbacks registered for output failures of the host's own standard error.
    errors: Rc<OutputErrors>,
}

impl ProcessTuiRuntime {
    /// Creates a host whose work runs on `local`, which the caller drives.
    #[must_use]
    pub fn new(local: Rc<LocalSet>) -> Self {
        Self {
            local,
            origin: Instant::now(),
            errors: Rc::new(OutputErrors::default()),
        }
    }
}

/// A scheduled callback; dropping the handle detaches it.
struct Timer(JoinHandle<()>);

impl RenderTimer for Timer {
    fn cancel(&mut self) {
        self.0.abort();
    }
}

/// Prints `error` and each of its sources, each followed by a newline; a failing standard
/// error is announced to the registered handlers once, after its lock is released.
fn report(errors: &OutputErrors, error: &dyn Error) {
    let written = {
        let mut sink = io::stderr().lock();
        let mut outcome = Ok(());
        let mut next = Some(error);
        while let Some(current) = next {
            outcome = outcome.and(writeln!(sink, "{current}"));
            next = current.source();
        }
        outcome
    };
    if let Err(failure) = written {
        errors.notify_source(FailureSource::Stderr, &failure);
    }
}

/// UTC text with millisecond precision for `unix_ms` milliseconds since the epoch.
fn timestamp(unix_ms: u64) -> String {
    let millis = i64::try_from(unix_ms).unwrap_or(i64::MAX);
    DateTime::<Utc>::from_timestamp_millis(millis)
        .unwrap_or(DateTime::UNIX_EPOCH)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

impl TuiRuntime for ProcessTuiRuntime {
    fn now(&self) -> Duration {
        Instant::now().saturating_duration_since(self.origin)
    }

    fn schedule(&self, delay: Duration, callback: RenderCallback) -> Box<dyn RenderTimer> {
        let deadline = Instant::now() + delay;
        let errors = Rc::clone(&self.errors);
        let task = self.local.spawn_local(async move {
            sleep_until(deadline).await;
            if let Err(error) = callback() {
                report(&errors, &error);
            }
        });
        Box::new(Timer(task))
    }

    fn spawn_local(&self, future: LocalFuture) {
        let errors = Rc::clone(&self.errors);
        drop(self.local.spawn_local(async move {
            if let Err(error) = future.await {
                report(&errors, &*error);
            }
        }));
    }

    fn environment(&self, key: &str) -> Option<String> {
        std::env::var_os(key).map(|value| value.to_string_lossy().into_owned())
    }

    fn log_context(&self) -> LogContext {
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
            });
        LogContext {
            home: home_directory(),
            iso_time: timestamp(unix_ms),
            unix_ms,
            nonce: format!("{:x}", rand::random::<u64>()),
        }
    }

    fn append_log(&self, path: &Path, contents: &str) -> io::Result<()> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?
            .write_all(contents.as_bytes())
    }

    fn write_log(&self, path: &Path, contents: &str) -> io::Result<()> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, contents)
    }

    fn on_output_error(&self, handler: OutputErrorHandler) -> Box<dyn Fn()> {
        self.errors.register(handler)
    }
}

/// The home directory: on Unix the `HOME` variable verbatim, empty included,
/// and the account directory only when it is unset.
fn home_directory() -> PathBuf {
    maestro_path::homedir(
        std::env::var_os("HOME")
            .filter(|_| cfg!(unix))
            .map(PathBuf::from),
        std::env::home_dir,
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests;
