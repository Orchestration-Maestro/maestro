//! Typed native resource imports used by the component adapter.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionFuture, ExtensionResult};

/// Native resource operations implemented by generated and controlled imports.
pub trait NativeImports {
    /// Forwards the native filesystem operation unchanged.
    fn save_file(&self, path: &str, bytes: &[u8], mode: u32) -> Result<Option<String>, String>;
    /// Forwards the native filesystem operation unchanged.
    fn file_mode(&self, path: &str) -> Result<u32, String>;
    /// Forwards the native filesystem operation unchanged.
    fn set_file_mode(&self, path: &str, mode: u32) -> Result<(), String>;
    /// Forwards the native filesystem operation unchanged.
    fn create_dir(&self, path: &str, mode: u32) -> Result<(), String>;

    /// Independently owned native watch registration.
    type Watcher: 'static;
    /// Stops the original native watch.
    fn watcher_close(&self, watcher: &Self::Watcher) -> ExtensionResult<()>;
    /// Host-owned cancellation flag.
    type Signal: 'static;
    /// Sends supplied operands with an original borrowed host signal.
    fn fetch<'a>(
        &'a self,
        request: crate::bindings::maestro::extension::host::HttpRequest,
        signal: Option<&'a Self::Signal>,
    ) -> ExtensionFuture<'a, Self::Response>;
    /// Waits using the selected native duration and borrowed cancellation.
    fn sleep<'a>(
        &'a self,
        duration: (u64, u32),
        signal: Option<&'a Self::Signal>,
    ) -> ExtensionFuture<'a, ()>;

    /// Independently retained native HTTP response.
    type Response: 'static;
    /// Consumes the original unused body as a native-parsed JSON value.
    fn response_json<'a>(
        &'a self,
        response: &'a Self::Response,
    ) -> ExtensionFuture<'a, serde_json::Value>;
    /// Consumes the original unused body as native-decoded text.
    fn response_text<'a>(&'a self, response: &'a Self::Response) -> ExtensionFuture<'a, String>;
    /// Reads the native response status.
    fn response_status(&self, response: &Self::Response) -> u16;
    /// Reads the original native reason phrase.
    fn response_status_text(&self, response: &Self::Response) -> String;
    /// Reads normalized response header pairs.
    fn response_headers(&self, response: &Self::Response) -> Vec<(String, String)>;
    /// Aliases the original consuming body without resetting it.
    fn response_body(&self, response: &Self::Response) -> Option<Self::Readable>;

    /// Retained native child identity.
    type Child: 'static;
    /// Retained raw reader identity.
    type Readable: 'static;
    /// Retained raw writer identity.
    type Writable: 'static;
    /// Launches an owned native child.
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: crate::child_process::SpawnOptions,
    ) -> ExtensionFuture<'_, Self::Child>;
    /// Signals an arbitrary native PID or group.
    fn kill(&self, pid: i32, signal: Option<String>) -> ExtensionResult<()>;
    /// Reads the admitted child's PID.
    fn child_pid(&self, child: &Self::Child) -> Option<u32>;
    /// Acquires the admitted input pipe without replacing it.
    fn child_stdin(&self, child: &Self::Child) -> Option<Self::Writable>;
    /// Acquires the admitted output pipe without replacing it.
    fn child_stdout(&self, child: &Self::Child) -> Option<Self::Readable>;
    /// Acquires the admitted error pipe without replacing it.
    fn child_stderr(&self, child: &Self::Child) -> Option<Self::Readable>;
    /// Waits for this child's output close and status.
    fn child_wait<'a>(
        &'a self,
        child: &'a Self::Child,
    ) -> ExtensionFuture<'a, crate::child_process::ExitStatus>;
    /// Signals this admitted child only while it is live.
    fn child_kill(&self, child: &Self::Child, signal: Option<String>) -> ExtensionResult<bool>;
    /// Reads one native byte chunk.
    fn readable_read<'a>(
        &'a self,
        reader: &'a Self::Readable,
    ) -> ExtensionFuture<'a, Option<Vec<u8>>>;
    /// Releases a raw consumer.
    fn readable_close(&self, reader: &Self::Readable) -> ExtensionResult<()>;
    /// Writes all the admitted bytes.
    fn writable_write<'a>(
        &'a self,
        writer: &'a Self::Writable,
        bytes: &'a [u8],
    ) -> ExtensionFuture<'a, ()>;
    /// Closes input after admitted writes.
    fn writable_end<'a>(&'a self, writer: &'a Self::Writable) -> ExtensionFuture<'a, ()>;
}
