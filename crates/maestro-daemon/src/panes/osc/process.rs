//! Foreground process policies for history and host colours.

use maestro_process::jobs::ForegroundJob;
use std::borrow::Cow;

/// Whether the observed group contains the shell.
fn foreground_job_is_shell(job: &ForegroundJob, shell_pid: u32) -> bool {
    job.processes.iter().any(|process| process.pid == shell_pid)
}

/// Whether a known primary-screen application needs history-clear compatibility.
fn foreground_job_uses_droid_scrollback_compat(job: &ForegroundJob) -> bool {
    job.processes.iter().any(|process| {
        process.name.eq_ignore_ascii_case("droid")
            || process
                .argv0
                .as_deref()
                .is_some_and(|argv0| argv0.eq_ignore_ascii_case("droid"))
            || process.cmdline.as_deref().is_some_and(|cmdline| {
                cmdline.eq_ignore_ascii_case("droid")
                    || cmdline.starts_with("droid ")
                    || cmdline.to_ascii_lowercase().contains("/droid")
            })
    })
}

/// Whether either history-clear spelling appears in this write.
pub(in crate::panes) fn contains_scrollback_clear_sequence(bytes: &[u8]) -> bool {
    bytes.windows(4).any(|window| window == b"\x1b[3J")
        || bytes.windows(5).any(|window| window == b"\x1b[?3J")
}

/// Remove only history-clear commands, retaining other erasures.
fn strip_scrollback_clear_sequences(bytes: &[u8]) -> Cow<'_, [u8]> {
    if !contains_scrollback_clear_sequence(bytes) {
        return Cow::Borrowed(bytes);
    }

    let mut filtered = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let remaining = &bytes[index..];
        if remaining.starts_with(b"\x1b[3J") {
            index += 4;
            continue;
        }
        if remaining.starts_with(b"\x1b[?3J") {
            index += 5;
            continue;
        }
        filtered.push(bytes[index]);
        index += 1;
    }

    Cow::Owned(filtered)
}

/// Keep primary-screen history for the matching foreground application.
pub(in crate::panes) fn maybe_filter_primary_screen_scrollback_clear<'a>(
    bytes: &'a [u8],
    alternate_screen: bool,
    foreground_job: Option<&ForegroundJob>,
) -> Cow<'a, [u8]> {
    if alternate_screen
        || !contains_scrollback_clear_sequence(bytes)
        || !foreground_job.is_some_and(foreground_job_uses_droid_scrollback_compat)
    {
        return Cow::Borrowed(bytes);
    }

    strip_scrollback_clear_sequences(bytes)
}

/// Restore only after returning to the shell on the primary screen.
pub(in crate::panes) fn should_restore_host_terminal_theme(
    owner_pgid: u32,
    shell_pid: u32,
    alternate_screen: bool,
    foreground_job: Option<&ForegroundJob>,
) -> bool {
    if alternate_screen {
        return false;
    }

    let Some(foreground_job) = foreground_job else {
        return false;
    };

    (cfg!(target_os = "macos") || foreground_job.process_group_id != owner_pgid)
        && foreground_job_is_shell(foreground_job, shell_pid)
}

#[cfg(test)]
mod tests;
