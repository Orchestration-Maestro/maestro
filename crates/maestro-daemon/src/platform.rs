//! Safe native hostname, local calendar and process-session observations.

#[cfg(unix)]
pub use maestro_process::foreground::current_process_is_detached_server_daemon;

/// Reads the Linux kernel hostname, without invoking a shell.
#[must_use]
pub fn hostname() -> Option<String> {
    let value = std::fs::read_to_string("/proc/sys/kernel/hostname").ok()?;
    let value = value.trim_end_matches('\n');
    (!value.is_empty()).then(|| value.to_owned())
}

/// Observes the local wall clock through the native timezone database.
#[must_use]
pub fn local_datetime() -> Option<time::PrimitiveDateTime> {
    let local = chrono::Local::now();
    let offset = time::UtcOffset::from_whole_seconds(local.offset().local_minus_utc()).ok()?;
    let value = time::OffsetDateTime::from_unix_timestamp(local.timestamp())
        .ok()?
        .to_offset(offset);
    Some(time::PrimitiveDateTime::new(value.date(), value.time()))
}

/// Shows a native desktop notification when a Linux display and notifier are available.
///
/// # Errors
/// Returns native launch or wait failures other than an absent notifier executable.
#[cfg(target_os = "linux")]
pub fn show_desktop_notification(title: &str, body: Option<&str>) -> std::io::Result<bool> {
    use maestro_process::{OutputMode, SpawnSyncError, SpawnSyncOptions, spawn_sync};
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Ok(false);
    }
    let mut command = maestro_process::command("notify-send");
    command.arg("--").arg(title);
    if let Some(body) = body.filter(|body| !body.is_empty()) {
        command.arg(body);
    }
    match spawn_sync(
        &mut command,
        SpawnSyncOptions::Capture {
            stdout: OutputMode::Discard,
            input: None,
            timeout: None,
            stderr: OutputMode::Discard,
            max_buffer_bytes: None,
        },
    ) {
        Ok(output) => Ok(output.status.success()),
        Err(SpawnSyncError::Spawn(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(false)
        }
        Err(
            SpawnSyncError::Spawn(error)
            | SpawnSyncError::Wait(error)
            | SpawnSyncError::Infrastructure(error),
        ) => Err(error),
        Err(error @ SpawnSyncError::Timeout) => Err(std::io::Error::other(error)),
    }
}
