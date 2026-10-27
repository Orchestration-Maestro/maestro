//! Native shell selection, inheritance and detached-child cleanup.
use indexmap::{IndexMap, IndexSet};
use maestro_process::{OutputMode, SpawnSyncOptions, spawn_sync};
use std::{
    ffi::OsString,
    io,
    path::Path,
    sync::{Mutex, OnceLock},
    time::Duration,
};

/// Native environment entries in observation order, including explicitly unset values.
pub type ShellEnv = IndexMap<OsString, Option<OsString>>;

/// The selected shell executable and command arguments.
#[derive(Debug, PartialEq, Eq)]
pub struct ShellConfig {
    /// Executable spelling used for spawning.
    pub shell: String,
    /// Arguments before the literal command.
    pub args: Vec<String>,
}

/// Selects a shell synchronously, observing existence rather than executability.
///
/// # Errors
/// Returns an error for a missing explicit path or unavailable Windows bash.
pub fn get_shell_config(custom_shell_path: Option<&str>) -> io::Result<ShellConfig> {
    select_shell(
        custom_shell_path,
        cfg!(windows),
        |key| std::env::var(key).ok(),
        |path| Path::new(path).exists(),
        probe,
    )
}

/// Runs the platform's discovery command through synchronous capture.
pub(super) fn probe(program: &str, operand: &str, deadline: Duration) -> Option<String> {
    let mut command = maestro_process::command(program);
    command.arg(operand);
    let output = spawn_sync(
        &mut command,
        SpawnSyncOptions::Capture {
            max_buffer_bytes: None,
            stdout: OutputMode::Capture,
            input: None,
            timeout: Some(deadline),
            stderr: OutputMode::Discard,
        },
    )
    .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Selects with supplied platform observations, keeping early reads lazy.
pub(super) fn select_shell(
    custom: Option<&str>,
    windows: bool,
    mut env: impl FnMut(&str) -> Option<String>,
    mut exists: impl FnMut(&str) -> bool,
    mut probe: impl FnMut(&str, &str, Duration) -> Option<String>,
) -> io::Result<ShellConfig> {
    let shell = if let Some(path) = custom.filter(|path| !path.is_empty()) {
        if !exists(path) {
            return Err(io::Error::other(format!(
                "Custom shell path not found: {path}"
            )));
        }
        path.to_owned()
    } else if windows {
        select_windows(&mut env, &mut exists, &mut probe)?
    } else if exists("/bin/bash") {
        "/bin/bash".to_owned()
    } else {
        first_match(probe("which", "bash", Duration::from_millis(5000)))
            .unwrap_or_else(|| "sh".to_owned())
    };
    Ok(ShellConfig {
        shell,
        args: vec!["-c".to_owned()],
    })
}

/// Takes the first line after trimming the whole probe result.
fn first_match(output: Option<String>) -> Option<String> {
    let output = output?;
    let trimmed = output.trim_matches(maestro_text::is_whitespace_scalar);
    let first = trimmed
        .split_once('\n')
        .map_or(trimmed, |(line, _)| line.strip_suffix('\r').unwrap_or(line));
    (!first.is_empty()).then(|| first.to_owned())
}

/// Selects known Windows installations before checking the first PATH match.
fn select_windows(
    env: &mut impl FnMut(&str) -> Option<String>,
    exists: &mut impl FnMut(&str) -> bool,
    probe: &mut impl FnMut(&str, &str, Duration) -> Option<String>,
) -> io::Result<String> {
    let paths: Vec<_> = ["ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(env)
        .filter(|root| !root.is_empty())
        .map(|root| format!("{root}\\Git\\bin\\bash.exe"))
        .collect();
    if let Some(path) = paths.iter().find(|path| exists(path)) {
        return Ok(path.clone());
    }
    if let Some(path) = first_match(probe("where", "bash.exe", Duration::from_millis(5000)))
        .filter(|path| exists(path))
    {
        return Ok(path);
    }
    let searched = paths
        .iter()
        .map(|path| format!("  {path}"))
        .collect::<Vec<_>>()
        .join("\n");
    Err(io::Error::other(format!(
        "No bash shell found. Options:\n  1. Install Git for Windows: https://git-scm.com/download/win\n  2. Add your bash to PATH (Cygwin, MSYS2, etc.)\n  3. Set shellPath in settings.json\n\nSearched Git Bash in:\n{searched}"
    )))
}

/// Reads a fresh native environment and inserts `bin_dir` in its first PATH entry.
#[must_use]
pub fn get_shell_env(bin_dir: &str) -> ShellEnv {
    shell_env(
        maestro_process::environment()
            .map(|(key, value)| (key, Some(value)))
            .collect(),
        bin_dir,
        cfg!(windows),
    )
}

/// Prepends only when exact nonempty delimiter-separated membership is absent.
pub(super) fn shell_env(mut env: ShellEnv, bin_dir: &str, windows: bool) -> ShellEnv {
    let key = env
        .keys()
        .find(|key| key.eq_ignore_ascii_case("path"))
        .cloned()
        .unwrap_or_else(|| OsString::from("PATH"));
    let current = env.get(&key).cloned().flatten().unwrap_or_default();
    let delimiter = if windows { b';' } else { b':' };
    let present = current
        .as_encoded_bytes()
        .split(|byte| *byte == delimiter)
        .filter(|entry| !entry.is_empty())
        .any(|entry| entry == bin_dir.as_bytes());
    let mut updated = OsString::from(bin_dir);
    if present {
        updated = current;
    } else if !current.is_empty() {
        if !updated.is_empty() {
            updated.push(if windows { ";" } else { ":" });
        }
        updated.push(current);
    }
    env.insert(key, Some(updated));
    env
}

/// The single insertion-ordered detached PID registry.
fn tracked() -> &'static Mutex<IndexSet<u32>> {
    static PIDS: OnceLock<Mutex<IndexSet<u32>>> = OnceLock::new();
    PIDS.get_or_init(|| Mutex::new(IndexSet::new()))
}

/// Records a detached PID once, in insertion order.
pub fn track_detached_child_pid(pid: u32) {
    tracked()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(pid);
}

/// Removes a PID without changing the order of other entries.
pub fn untrack_detached_child_pid(pid: u32) {
    tracked()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .shift_remove(&pid);
}

/// Drains tracked PIDs and attempts tree cleanup outside the registry lock.
pub fn kill_tracked_detached_children() {
    cleanup(kill_process_tree);
}

/// Requests forced tree termination, ignoring native failures.
pub fn kill_process_tree(pid: u32) {
    #[cfg(unix)]
    tree_kill(
        pid,
        maestro_process::child_process::kill_process_group,
        maestro_process::child_process::kill_process,
    );
    #[cfg(windows)]
    let _ = maestro_process::child_process::request_tree_kill(pid);
}

/// Drains before invoking platform effects, without holding the registry lock.
pub(super) fn cleanup(mut kill: impl FnMut(u32)) {
    let pids = std::mem::take(
        &mut *tracked()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    for pid in pids {
        kill(pid);
    }
}

/// Applies Unix group-first fallback without interpreting failures.
#[cfg(unix)]
pub(super) fn tree_kill(
    pid: u32,
    mut group: impl FnMut(u32) -> io::Result<()>,
    mut direct: impl FnMut(u32) -> io::Result<()>,
) {
    if group(pid).is_err() {
        let _ = direct(pid);
    }
}
