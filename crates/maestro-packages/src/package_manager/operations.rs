//! Replaceable native effects for package source lookup.
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
/// Effects supplied to the configured-source manager.
pub trait PackageOperations {
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
}

/// Native filesystem, environment and synchronous process effects.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativePackageOperations {
    /// Caller-owned shell selection.
    should_use_shell: fn(&str) -> bool,
}
#[cfg(not(target_arch = "wasm32"))]
impl NativePackageOperations {
    /// Stores the caller's shell selection without observing it.
    #[must_use]
    pub fn new(should_use_shell: fn(&str) -> bool) -> Self {
        Self { should_use_shell }
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl PackageOperations for NativePackageOperations {
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
        use std::process::{Command, Stdio};
        let mut child = if (self.should_use_shell)(command) {
            let mut shell = Command::new(if cfg!(windows) { "cmd.exe" } else { "/bin/sh" });
            if cfg!(windows) {
                shell.args(["/d", "/s", "/c"]);
            } else {
                shell.arg("-c");
            }
            shell.arg(format!("{command} {}", args.join(" ")));
            shell
        } else {
            let mut child = Command::new(command);
            child.args(args);
            child
        };
        let inherited = std::env::vars_os()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect();
        let environment = recover_environment(
            if cfg!(target_os = "linux") {
                "linux"
            } else {
                "other"
            },
            inherited,
            || std::fs::read("/proc/self/environ"),
        );
        let output = child
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .envs(environment)
            .output()?;
        Ok(CommandOutput {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
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
