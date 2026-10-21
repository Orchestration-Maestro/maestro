//! Replaceable native effects for package source lookup.
use super::PackageFuture;
use std::io;
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
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
/// Effects supplied to the configured-source manager.
pub trait PackageOperations {
    /// Reads text with replacement decoding for invalid UTF-8.
    /// # Errors
    /// Returns a filesystem read failure.
    fn read_file(&self, path: &str) -> io::Result<String>;
    /// Supplies the temporary base for remote temporary paths.
    fn temp_dir(&self) -> String;
    /// Reads the current offline setting from the environment.
    fn offline_value(&self) -> Option<String>;
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
    /// Whether a file or directory exists at the authored path.
    fn exists(&self, path: &str) -> bool;
    /// Returns the user's home directory.
    /// # Errors
    /// Returns the adapter's home lookup failure.
    fn home_dir(&self) -> io::Result<String>;
    /// Returns the ambient working directory when resolution needs it.
    /// # Errors
    /// Returns the adapter's working-directory lookup failure.
    fn current_dir(&self) -> io::Result<String>;
    /// Returns an ambient per-drive directory when available.
    fn drive_directory(&self, drive: char) -> Option<String>;
    /// Waits for a captured command with ignored stdin.
    /// # Errors
    /// Returns a native spawn or capture failure.
    fn run_command_sync(&self, command: &str, args: &[String]) -> io::Result<CommandOutput>;
    /// Creates a directory and its missing parents.
    /// # Errors
    /// Returns the adapter's creation failure.
    fn create_dir_all(&self, path: &str) -> io::Result<()>;
    /// Exclusively allocates an owned directory below the supplied parent.
    /// # Errors
    /// Returns the native creation failure; existing names are skipped.
    fn create_temp_dir(&self, parent: &str) -> io::Result<String>;
    /// Renames the supplied path to its destination.
    /// # Errors
    /// Returns the native rename failure.
    fn rename_path(&self, from: &str, to: &str) -> io::Result<()>;
    /// Writes UTF-8 text, requiring a new pathname when `exclusive` is true.
    /// Otherwise creates or truncates the destination. Neither mode creates parents.
    /// # Errors
    /// Returns the original open or write failure, including exclusive creation collisions.
    fn write_file(&self, path: &str, text: &str, exclusive: bool) -> io::Result<()>;
    /// Whether a directory has no entries.
    /// # Errors
    /// Returns an open or enumeration failure, including one after an earlier entry.
    fn directory_is_empty(&self, path: &str) -> io::Result<bool>;
    /// Removes a file, link or directory tree; a missing path succeeds.
    /// # Errors
    /// Returns the adapter's removal failure.
    fn remove_path(&self, path: &str) -> io::Result<()>;
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

/// Native filesystem, environment and process effects.
///
/// Its asynchronous commands must be driven by a Tokio runtime with process support.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativePackageOperations {
    /// Weak handle to the caller-driven local runtime.
    local: std::rc::Weak<tokio::task::LocalSet>,
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
        Self {
            local: std::rc::Rc::downgrade(local),
            is_stdout_taken_over,
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl PackageOperations for NativePackageOperations {
    fn read_file(&self, path: &str) -> io::Result<String> {
        std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }
    fn temp_dir(&self) -> String {
        std::env::temp_dir().to_string_lossy().into_owned()
    }
    fn offline_value(&self) -> Option<String> {
        std::env::var("MAESTRO_OFFLINE").ok()
    }
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
        let local = self
            .local
            .upgrade()
            .ok_or_else(|| io::Error::other("package runtime is no longer available"))?;
        drop(local.spawn_local(operation));
        Ok(())
    }
    fn exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
    fn home_dir(&self) -> io::Result<String> {
        home(
            std::env::var_os("HOME").map(|text| text.to_string_lossy().into_owned()),
            || {
                std::env::home_dir()
                    .map(|path| path.to_string_lossy().into_owned())
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::NotFound, "home directory unavailable")
                    })
            },
        )
    }
    fn current_dir(&self) -> io::Result<String> {
        std::env::current_dir().map(|path| path.to_string_lossy().into_owned())
    }
    fn drive_directory(&self, drive: char) -> Option<String> {
        std::env::var_os(format!("={}:", drive.to_ascii_uppercase()))
            .map(|text| text.to_string_lossy().into_owned())
    }
    fn run_command_sync(&self, command: &str, args: &[String]) -> io::Result<CommandOutput> {
        Self::capture(command, args)
    }
    fn create_dir_all(&self, path: &str) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }
    fn create_temp_dir(&self, parent: &str) -> io::Result<String> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let name = format!(
                ".maestro-stage-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let path = std::path::Path::new(parent).join(name);
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(path.to_string_lossy().into_owned()),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }
    fn rename_path(&self, from: &str, to: &str) -> io::Result<()> {
        std::fs::rename(from, to)
    }
    fn write_file(&self, path: &str, text: &str, exclusive: bool) -> io::Result<()> {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .create_new(exclusive)
            .open(path)?
            .write_all(text.as_bytes())
    }
    fn directory_is_empty(&self, path: &str) -> io::Result<bool> {
        let mut empty = true;
        for entry in std::fs::read_dir(path)? {
            entry?;
            empty = false;
        }
        Ok(empty)
    }
    fn remove_path(&self, path: &str) -> io::Result<()> {
        let path = std::path::Path::new(path);
        let removed = match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
            Ok(_) => std::fs::remove_file(path).or_else(|error| {
                if cfg!(windows) {
                    std::fs::remove_dir(path)
                } else {
                    Err(error)
                }
            }),
            Err(error) => Err(error),
        };
        removed.or_else(|error| match error.kind() {
            io::ErrorKind::NotFound => Ok(()),
            _ => Err(error),
        })
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

/// Selects a nonempty authored home before invoking the platform fallback.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn home(
    explicit: Option<String>,
    fallback: impl FnOnce() -> io::Result<String>,
) -> io::Result<String> {
    if let Some(home) = explicit.filter(|text| !text.is_empty()) {
        return Ok(home);
    }
    fallback()
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
