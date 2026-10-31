//! Native effects supplied by the application without moving lifecycle into the adapter.
use std::{future::Future, pin::Pin, rc::Rc};

/// Cancellable native delay owned by the tools runtime.
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::process::sleep::sleep;

/// A local native operation with its unchanged failure text.
pub type IoFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + 'a>>;

/// Selection of one native standard descriptor.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Stdio {
    /// Retained raw byte pipe.
    #[default]
    Pipe,
    /// Native null device.
    Ignore,
    /// Corresponding host descriptor.
    Inherit,
}
/// Literal child launch selections; a present environment replaces ambient values.
#[derive(Default)]
pub struct SpawnOptions {
    /// Omitted inherits the host directory.
    pub cwd: Option<String>,
    /// Entire supplied environment, including a present-empty selection.
    pub env: Option<Vec<(String, String)>>,
    /// Creates a native session where supported.
    pub detached: bool,
    /// Independent stdin, stdout and stderr selections.
    pub stdio: [Stdio; 3],
}
/// Numeric exit and signal termination remain distinct.
#[derive(Debug, Eq, PartialEq)]
pub struct ExitStatus {
    /// Absent for signal termination.
    pub code: Option<i32>,
    /// Absent for numeric exit.
    pub signal: Option<String>,
}
/// Independently retained consuming byte reader.
pub trait Readable {
    /// Reads available raw bytes or observes EOF.
    fn read(&self) -> IoFuture<'_, Option<Vec<u8>>>;
    /// Closes only this consumer.
    ///
    /// # Errors
    /// Returns the native close failure.
    fn close(&self) -> Result<(), String>;
}
/// Independently retained ordered byte writer.
pub trait Writable {
    /// Writes every admitted byte, including across partial native writes.
    fn write<'a>(&'a self, bytes: &'a [u8]) -> IoFuture<'a, ()>;
    /// Closes after earlier admitted writes.
    fn end(&self) -> IoFuture<'_, ()>;
}
/// Retained child controls and the original selected descriptors.
pub trait ChildProcess {
    /// Original native process ID.
    fn pid(&self) -> Option<u32>;
    /// Original input pipe, absent unless selected.
    fn stdin(&self) -> Option<Rc<dyn Writable>>;
    /// Original output pipe, absent unless selected.
    fn stdout(&self) -> Option<Rc<dyn Readable>>;
    /// Original error pipe, absent unless selected.
    fn stderr(&self) -> Option<Rc<dyn Readable>>;
    /// Observes the sole native waiter's exit and output-close completion.
    fn wait(&self) -> IoFuture<'_, ExitStatus>;
    /// Signals this process only before its native exit.
    ///
    /// # Errors
    /// Returns the native signal failure.
    fn kill(&self, signal: Option<String>) -> Result<bool, String>;
}
/// One HTTP attempt with the original caller cancellation.
pub struct Request {
    /// Supplied destination.
    pub url: String,
    /// Omitted selects GET.
    pub method: Option<String>,
    /// Header normalization belongs to the native HTTP owner.
    pub headers: Vec<(String, String)>,
    /// Omitted and present-empty remain distinct.
    pub body: Option<Vec<u8>>,
    /// Original caller signal retained into body consumption.
    pub signal: Option<crate::Cancellation>,
}
/// Independently retained native response.
pub trait Response {
    /// Parses the original unused body with the shared text decoder and JSON parser.
    fn json(&self) -> IoFuture<'_, serde_json::Value>;
    /// Collects the original unused body through the shared HTTP and text owners.
    fn text(&self) -> IoFuture<'_, String>;
    /// Native status, including non-success outcomes.
    fn status(&self) -> u16;
    /// Whether the status is within 200 through 299 inclusive.
    fn ok(&self) -> bool {
        (200..300).contains(&self.status())
    }
    /// Original native reason phrase.
    fn status_text(&self) -> String;
    /// Native HTTP owner's normalized response headers.
    fn headers(&self) -> Vec<(String, String)>;
    /// An alias of the original body; obtaining it does not consume bytes.
    fn body(&self) -> Option<std::rc::Rc<dyn Readable>>;
}
/// Application-supplied native effects available to component resources.
pub trait NativeIo {
    /// Atomically publishes bytes, preserving existing permissions.
    ///
    /// New files use `mode`; success carries an optional durability warning.
    /// Final symbolic links are followed, including dangling targets.
    ///
    /// # Errors
    /// Returns the native failure before publication.
    fn save_file(&self, path: String, bytes: Vec<u8>, mode: u32) -> IoFuture<'_, Option<String>>;
    /// Reads native Unix permission bits, including file type bits.
    ///
    /// # Errors
    /// Returns native metadata errors or unsupported on non-Unix hosts.
    fn file_mode(&self, path: &str) -> Result<u32, String>;
    /// Changes native Unix permission bits, following symbolic links.
    ///
    /// # Errors
    /// Returns native permission errors or unsupported on non-Unix hosts.
    fn set_file_mode(&self, path: &str, mode: u32) -> Result<(), String>;
    /// Creates directories recursively with the requested Unix creation mode.
    ///
    /// Existing directories are unchanged. Windows does not configure ACLs.
    ///
    /// # Errors
    /// Returns native directory creation errors.
    fn create_dir(&self, path: &str, mode: u32) -> Result<(), String>;

    /// Watches a supplied native file without a callback-scoped lifetime.
    ///
    /// # Errors
    /// Returns the native registration failure.
    fn watch(&self, path: String, listener: Rc<dyn Fn()>) -> Result<Rc<dyn FsWatcher>, String>;
    /// Sends one native HTTP attempt without retry or an implicit timeout.
    fn fetch(&self, request: Request) -> IoFuture<'_, Rc<dyn Response>>;
    /// Launches literal command operands without shell or environment rewriting.
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: SpawnOptions,
    ) -> IoFuture<'_, Rc<dyn ChildProcess>>;
    /// Signals a PID, or a native process group where supported.
    ///
    /// # Errors
    /// Returns the native signal failure.
    fn kill(&self, pid: i32, signal: Option<String>) -> Result<(), String>;
}

/// An owned native notification registration; dropping it stops delivery.
pub trait FsWatcher {
    /// Stops notifications from the original registration.
    ///
    /// # Errors
    /// Returns the native close failure.
    fn close(&self) -> Result<(), String>;
}
