//! Text copying policy and replaceable operating-system effects.

/// Platform branches used by clipboard routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardPlatform {
    /// Linux desktop or headless host.
    Linux,
    /// macOS.
    Darwin,
    /// Windows.
    Win32,
    /// Other native hosts, retaining tool and terminal fallback.
    Other,
}

use super::clipboard_native::ClipboardModule;
use maestro_process::{SpawnSyncError, SpawnSyncOptions};
use std::{io, process::Output};
use tokio::process::Command;

/// Effects consumed by the text-copy policy.
pub trait ClipboardOperations {
    /// Observes the host platform.
    fn platform(&self) -> ClipboardPlatform;
    /// Returns the admitted native module, if available.
    fn clipboard(&self) -> Option<&dyn ClipboardModule>;
    /// Runs a synchronous helper using the process owner's options.
    ///
    /// # Errors
    /// Returns process startup, wait, infrastructure or deadline failures.
    fn spawn_sync(
        &self,
        command: &mut Command,
        options: SpawnSyncOptions<'_>,
    ) -> Result<Output, SpawnSyncError>;
    /// Starts a helper with owned input, without waiting for its completion.
    ///
    /// # Errors
    /// Returns startup failures.
    fn spawn(&self, command: Command, input: Option<Vec<u8>>) -> io::Result<()>;
    /// Accepts the complete terminal frame or returns its original write error.
    ///
    /// # Errors
    /// Returns the output writer's failure.
    fn write_stdout(&self, bytes: &[u8]) -> io::Result<()>;
}

/// Reads a process variable, substituting U+FFFD for undecodable bytes.
pub(super) fn environment(name: &str) -> Option<String> {
    std::env::var_os(name).map(|value| value.to_string_lossy().into_owned())
}

/// Nonempty environment strings select the corresponding route.
fn present(name: &str) -> bool {
    environment(name).is_some_and(|value| !value.is_empty())
}

/// Runs a fixed program; writers use a pipe and five-second limit, probes neither.
fn run_tool(
    operations: &impl ClipboardOperations,
    program: &str,
    args: &[&str],
    text: Option<&str>,
) -> bool {
    operations
        .spawn_sync(
            maestro_process::command(program).args(args),
            SpawnSyncOptions::Capture {
                max_buffer_bytes: None,
                stdout: maestro_process::OutputMode::Discard,
                stderr: maestro_process::OutputMode::Discard,
                input: text.map(str::as_bytes),
                timeout: text.map(|_| std::time::Duration::from_millis(5000)),
            },
        )
        .is_ok_and(|output| output.status.success())
}

/// Tries the two X11 selection owners in order.
fn copy_to_x11_clipboard(operations: &impl ClipboardOperations, text: &str) -> bool {
    run_tool(
        operations,
        "xclip",
        &["-selection", "clipboard"],
        Some(text),
    ) || run_tool(operations, "xsel", &["--clipboard", "--input"], Some(text))
}

/// Prefers an acknowledged Wayland start, then X11 selection owners.
fn copy_to_display(operations: &impl ClipboardOperations, text: &str) -> bool {
    let wayland_display = present("WAYLAND_DISPLAY");
    let x11_display = present("DISPLAY");
    if super::clipboard_image::is_wayland_session(environment)
        && wayland_display
        && run_tool(operations, "which", &["wl-copy"], None)
        && operations
            .spawn(
                maestro_process::command("wl-copy"),
                Some(text.as_bytes().to_vec()),
            )
            .is_ok()
    {
        return true;
    }
    x11_display && copy_to_x11_clipboard(operations, text)
}

/// Copies text using supplied effects.
///
/// # Errors
/// Returns a terminal write error or failure when no route copied the text.
pub async fn copy_to_clipboard_with(
    text: &str,
    operations: &impl ClipboardOperations,
) -> io::Result<()> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let mut copied = if operations.platform() != ClipboardPlatform::Linux
        && let Some(module) = operations.clipboard()
    {
        module.set_text(text).await.is_ok()
    } else {
        false
    };
    let remote = ["SSH_CONNECTION", "SSH_CLIENT", "MOSH_CONNECTION"]
        .iter()
        .any(|name| present(name));
    if !copied {
        copied = match operations.platform() {
            ClipboardPlatform::Darwin => run_tool(operations, "pbcopy", &[], Some(text)),
            ClipboardPlatform::Win32 => run_tool(operations, "clip", &[], Some(text)),
            _ => {
                (present("TERMUX_VERSION")
                    && run_tool(operations, "termux-clipboard-set", &[], Some(text)))
                    || copy_to_display(operations, text)
            }
        };
    }
    if copied && !remote {
        return Ok(());
    }
    let encoded = STANDARD.encode(text);
    if encoded.len() <= 100_000 {
        operations.write_stdout(format!("\x1b]52;c;{encoded}\x07").as_bytes())?;
        return Ok(());
    }
    if copied {
        Ok(())
    } else {
        Err(io::Error::other("Failed to copy to clipboard"))
    }
}

/// Default desktop clipboard, process and terminal effects.
pub struct NativeClipboardOperations;
impl ClipboardOperations for NativeClipboardOperations {
    fn platform(&self) -> ClipboardPlatform {
        match std::env::consts::OS {
            "linux" => ClipboardPlatform::Linux,
            "macos" => ClipboardPlatform::Darwin,
            "windows" => ClipboardPlatform::Win32,
            _ => ClipboardPlatform::Other,
        }
    }
    fn clipboard(&self) -> Option<&dyn ClipboardModule> {
        super::clipboard_native::native_clipboard()
    }
    fn spawn_sync(
        &self,
        command: &mut Command,
        options: SpawnSyncOptions<'_>,
    ) -> Result<Output, SpawnSyncError> {
        maestro_process::spawn_sync(command, options)
    }
    fn spawn(&self, command: Command, input: Option<Vec<u8>>) -> io::Result<()> {
        maestro_process::spawn(command, input).map(drop)
    }
    fn write_stdout(&self, bytes: &[u8]) -> io::Result<()> {
        use std::io::Write as _;
        let mut stdout = io::stdout().lock();
        stdout.write_all(bytes)?;
        stdout.flush()
    }
}

/// Copies text through the default desktop adapter.
///
/// # Errors
/// Returns the errors described by [`copy_to_clipboard_with`].
pub async fn copy_to_clipboard(text: &str) -> io::Result<()> {
    copy_to_clipboard_with(text, &NativeClipboardOperations).await
}
