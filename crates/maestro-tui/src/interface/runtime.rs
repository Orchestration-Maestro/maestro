//! Effects the frame writer asks of its host: time, deferred work, environment and files.

use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use crate::terminal::Terminal;

/// A terminal shared between the frame writer and whoever drives it.
pub type TerminalHandle = Rc<RefCell<dyn Terminal>>;

/// Work a runtime runs after its delay; the driver of the runtime receives its error.
pub type RenderCallback = Box<dyn FnOnce() -> io::Result<()>>;

/// A fallible future the host runs on the current thread.
pub type LocalFuture =
    Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>> + 'static>>;

/// A pending [`RenderCallback`].
pub trait RenderTimer {
    /// Prevents the callback from running; cancelling twice does nothing.
    ///
    /// Dropping the handle without cancelling leaves the callback scheduled.
    fn cancel(&mut self);
}

/// Wall-clock and file-system facts the diagnostic logs are written with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogContext {
    /// Home directory the application's log paths are built from; the host chooses its form.
    pub home: PathBuf,
    /// UTC time with millisecond precision, such as `2026-01-02T03:04:05.006Z`.
    pub iso_time: String,
    /// Milliseconds since the Unix epoch.
    pub unix_ms: u64,
    /// Lowercase alphanumeric text that distinguishes log file names created in one millisecond.
    pub nonce: String,
}

/// The host of a frame writer.
///
/// The writer decides what to draw and when; the host supplies the time, defers
/// work, answers environment queries and performs the log file effects.
pub trait TuiRuntime {
    /// Monotonic time since an arbitrary origin.
    fn now(&self) -> Duration;

    /// Runs `callback` after `delay`, never before this call returns, even for a zero
    /// delay.
    fn schedule(&self, delay: Duration, callback: RenderCallback) -> Box<dyn RenderTimer>;

    /// Runs `future` to completion on the current thread, never before this call returns.
    ///
    /// The host owns the future and reports an error it returns; the writer never
    /// prints it.
    fn spawn_local(&self, future: LocalFuture);

    /// The value of an environment variable, if set.
    fn environment(&self, key: &str) -> Option<String>;

    /// The current time, home directory and file name nonce for a diagnostic log.
    fn log_context(&self) -> LogContext;

    /// Appends `contents` to the file at `path` without creating its directory.
    ///
    /// # Errors
    ///
    /// Returns the file-system error unchanged.
    fn append_log(&self, path: &Path, contents: &str) -> io::Result<()>;

    /// Replaces the file at `path` with `contents`, creating its directory first.
    ///
    /// # Errors
    ///
    /// Returns the file-system error unchanged.
    fn write_log(&self, path: &Path, contents: &str) -> io::Result<()>;
}

/// Whether the environment variable `key` is exactly `1`.
pub(super) fn is_enabled(runtime: &dyn TuiRuntime, key: &str) -> bool {
    runtime.environment(key).as_deref() == Some("1")
}
