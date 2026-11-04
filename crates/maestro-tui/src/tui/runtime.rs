//! Effects the frame writer asks of its host: time, deferred work and log context.

use std::cell::RefCell;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

use crate::terminal::{OutputErrorHandler, Terminal};

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
/// The writer decides what to draw and when; the host supplies the time and defers
/// work.
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

    /// The current time, home directory and file name nonce for a diagnostic log.
    fn log_context(&self) -> LogContext;

    /// Registers `handler` for the failures of the host's own output; the returned operation
    /// removes it and does nothing when repeated.
    fn on_output_error(&self, handler: OutputErrorHandler) -> Box<dyn Fn()> {
        let _ = handler;
        Box::new(|| ())
    }
}

/// The value of the environment variable `key`, substituting replacement characters for
/// bytes that are not UTF-8.
pub(super) fn environment(key: &str) -> Option<String> {
    std::env::var_os(key).map(|value| value.to_string_lossy().into_owned())
}

/// Whether the environment variable `key` is exactly `1`.
pub(super) fn is_enabled(key: &str) -> bool {
    environment(key).as_deref() == Some("1")
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt as _;

    #[cfg(unix)]
    use super::environment;

    /// Variable the probe sets to a value that is not UTF-8.
    #[cfg(unix)]
    const PROBE: &str = "MAESTRO_TUI_ENVIRONMENT_PROBE";
    /// Variable that marks the child this test re-runs itself in.
    #[cfg(unix)]
    const CHILD: &str = "MAESTRO_TUI_ENVIRONMENT_CHILD";

    /// The reader keeps a value the platform cannot decode as text, with replacement
    /// characters, instead of dropping it.
    #[cfg(unix)]
    #[test]
    fn environment_decodes_non_utf8_values_with_replacement_characters() {
        const TEST: &str = "environment_decodes_non_utf8_values_with_replacement_characters";
        if std::env::var_os(CHILD).is_some() {
            assert_eq!(environment(PROBE).as_deref(), Some("A\u{fffd}B"));
            return;
        }
        let output = maestro_test_process::TestProcess::current(TEST)
            .unwrap()
            .env_clear()
            .env(PROBE, std::ffi::OsStr::from_bytes(b"A\xffB"))
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
