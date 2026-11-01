//! Replaceable command runs for package source lookup, and the home-directory lookup.
use super::PackageFuture;
use std::io;
/// A completed child status and independently decoded streams.
pub struct CommandOutput {
    /// Exit status; absent when terminated without an exit code.
    pub status: Option<i32>,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
}
/// Operands for a captured child command.
pub struct CommandCaptureOptions<'a> {
    /// Authored child working directory.
    pub cwd: Option<&'a str>,
    /// Deadline measured after spawn; absent means unbounded.
    pub timeout: Option<std::time::Duration>,
    /// Environment pairs overlaid on the inherited environment.
    pub env: &'a [(&'a str, &'a str)],
}
/// Command runs supplied to the package manager; it reads files and the environment
/// directly.
pub trait PackageOperations {
    /// Captures stdout until child exit and both stream EOFs.
    /// # Errors
    /// Returns spawn, read, wait, deadline or unsuccessful-status failures.
    fn run_command_capture<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        options: CommandCaptureOptions<'a>,
    ) -> PackageFuture<'a, String>;
    /// Admits owned local work into the caller-driven runtime.
    /// # Errors
    /// Returns the adapter's admission failure.
    fn spawn(
        &self,
        operation: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'static>>,
    ) -> io::Result<()>;
    /// Waits for a captured command with ignored stdin.
    /// # Errors
    /// Returns a native spawn or capture failure.
    fn run_command_sync(&self, command: &str, args: &[String]) -> io::Result<CommandOutput>;
    /// Runs a child with inherited output until it exits; `None` means it ended without a code.
    /// # Errors
    /// Returns the adapter's spawn or wait failure.
    fn run_command<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        cwd: Option<&'a str>,
    ) -> PackageFuture<'a, Option<i32>>;
}

/// Admission into the caller's native local task facility.
#[cfg(not(target_arch = "wasm32"))]
type Admission =
    std::rc::Rc<dyn Fn(std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) -> io::Result<()>>;

/// Native process effects.
///
/// Its asynchronous commands must be driven by a Tokio runtime with process support.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativePackageOperations {
    /// Caller-owned admission for owned local work.
    spawn: Admission,
    /// Caller-owned query for whether output is routed to standard error.
    pub(super) is_stdout_taken_over: std::rc::Rc<dyn Fn() -> bool>,
}
#[cfg(not(target_arch = "wasm32"))]
impl NativePackageOperations {
    /// Stores the caller's stdout-takeover query without calling it.
    #[must_use]
    pub fn new(
        is_stdout_taken_over: std::rc::Rc<dyn Fn() -> bool>,
        local: &std::rc::Rc<tokio::task::LocalSet>,
    ) -> Self {
        let local = std::rc::Rc::downgrade(local);
        Self::with_spawn(is_stdout_taken_over, move |operation| {
            let local = local
                .upgrade()
                .ok_or_else(|| io::Error::other("package runtime is no longer available"))?;
            drop(local.spawn_local(operation));
            Ok(())
        })
    }
    /// Stores supplied work admission without changing any native command route.
    #[must_use]
    pub fn with_spawn(
        is_stdout_taken_over: std::rc::Rc<dyn Fn() -> bool>,
        spawn: impl Fn(std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) -> io::Result<()>
        + 'static,
    ) -> Self {
        Self {
            spawn: std::rc::Rc::new(spawn),
            is_stdout_taken_over,
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl PackageOperations for NativePackageOperations {
    fn run_command_capture<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        options: CommandCaptureOptions<'a>,
    ) -> PackageFuture<'a, String> {
        Box::pin(self.capture_async(command, args, options))
    }
    fn spawn(
        &self,
        operation: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'static>>,
    ) -> io::Result<()> {
        (self.spawn)(operation)
    }
    fn run_command_sync(&self, command: &str, args: &[String]) -> io::Result<CommandOutput> {
        Self::capture(command, args)
    }
    fn run_command<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        cwd: Option<&'a str>,
    ) -> PackageFuture<'a, Option<i32>> {
        self.launch(command, args, cwd)
    }
}

/// Recovers an empty Linux environment, retaining first-equals and last-duplicate semantics.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn recover_environment(
    platform: &str,
    mut inherited: std::collections::BTreeMap<String, String>,
    read: impl FnOnce() -> io::Result<Vec<u8>>,
) -> std::collections::BTreeMap<String, String> {
    if platform != "linux" || !inherited.is_empty() {
        return inherited;
    }
    if let Ok(data) = read() {
        for entry in String::from_utf8_lossy(&data).split('\0') {
            if let Some((key, value)) = entry.split_once('=')
                && !key.is_empty()
            {
                inherited.insert(key.to_owned(), value.to_owned());
            }
        }
    }
    inherited
}
