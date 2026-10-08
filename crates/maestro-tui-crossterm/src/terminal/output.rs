//! Writes to standard output and the optional write log.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use chrono::{DateTime, Local, TimeZone};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::io::{Errno, write};
use rustix::stdio::stdout;
use tokio::task::JoinHandle;
use tokio::time::{Instant, MissedTickBehavior, interval_at};

use super::{ProcessTerminal, Shared};

/// Enables bracketed paste.
pub(super) const PASTE_ON: &str = "\x1b[?2004h";

/// Disables bracketed paste.
pub(super) const PASTE_OFF: &str = "\x1b[?2004l";

/// Asks the terminal which enhanced keyboard flags it runs.
pub(super) const KEYBOARD_QUERY: &str = "\x1b[?u";

/// Enables enhanced keyboard flags 1, 2 and 4.
pub(super) const KEYBOARD_ON: &str = "\x1b[>7u";

/// Pops the enhanced keyboard flags pushed by [`KEYBOARD_ON`].
pub(super) const KEYBOARD_POP: &str = "\x1b[<u";

/// Enables modified-key reporting for terminals that do not answer the keyboard query.
pub(super) const FALLBACK_ON: &str = "\x1b[>4;2m";

/// Disables the modified-key reporting enabled by [`FALLBACK_ON`].
pub(super) const FALLBACK_OFF: &str = "\x1b[>4;0m";

/// Hides the cursor.
pub(super) const CURSOR_HIDE: &str = "\x1b[?25l";

/// Shows the cursor.
pub(super) const CURSOR_SHOW: &str = "\x1b[?25h";

/// Clears from the cursor to the end of the line.
pub(super) const CLEAR_LINE: &str = "\x1b[K";

/// Clears from the cursor to the end of the screen.
pub(super) const CLEAR_FROM_CURSOR: &str = "\x1b[J";

/// Clears the screen and moves the cursor home.
pub(super) const CLEAR_SCREEN: &str = "\x1b[2J\x1b[H";

/// Shows indeterminate progress.
const PROGRESS_ACTIVE: &str = "\x1b]9;4;3\x07";

/// Clears the progress indicator.
pub(super) const PROGRESS_CLEAR: &str = "\x1b]9;4;0;\x07";

/// How often active progress is shown again so the terminal keeps displaying it.
const KEEPALIVE: Duration = Duration::from_millis(1000);

/// The environment variable naming the write log.
const LOG_VARIABLE: &str = "MAESTRO_TUI_WRITE_LOG";

/// Writes all of `text` to standard output, waiting while it is full. The reader makes the
/// description nonblocking, and standard output usually shares it.
pub(super) fn emit(text: &str) -> io::Result<()> {
    let mut rest = text.as_bytes();
    while !rest.is_empty() {
        match write(stdout(), rest) {
            Ok(written) => rest = &rest[written..],
            Err(Errno::INTR) => {}
            Err(Errno::AGAIN) => wait_until_writable()?,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Blocks until standard output accepts more.
fn wait_until_writable() -> io::Result<()> {
    let output = stdout();
    let mut waiting = [PollFd::new(&output, PollFlags::OUT)];
    match poll(&mut waiting, None) {
        Ok(_) | Err(Errno::INTR) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Writes `text` when `condition` holds.
pub(super) fn emit_if(condition: bool, text: &str) -> io::Result<()> {
    if condition { emit(text) } else { Ok(()) }
}

/// The first failure among `steps` that have already run, if any.
pub(super) fn first_failure(steps: impl IntoIterator<Item = io::Result<()>>) -> io::Result<()> {
    steps.into_iter().fold(Ok(()), Result::and)
}

/// The sequence that sets the window title to `text`.
pub(super) fn title(text: &str) -> String {
    format!("\x1b]0;{text}\x07")
}

/// The sequence that moves the cursor `lines` down when positive and up when negative, or
/// `None` for zero.
pub(super) fn relative_move(lines: isize) -> Option<String> {
    let distance = lines.unsigned_abs();
    match lines.signum() {
        1 => Some(format!("\x1b[{distance}B")),
        -1 => Some(format!("\x1b[{distance}A")),
        _ => None,
    }
}

/// Where explicit writes are appended, taken from the environment: a variable naming an
/// existing directory selects a file named for the local time and process in it, and any
/// other nonempty value is the file itself.
pub(super) fn log_destination() -> Option<PathBuf> {
    let configured =
        PathBuf::from(std::env::var_os(LOG_VARIABLE).filter(|value| !value.is_empty())?);
    if fs::metadata(&configured).is_ok_and(|metadata| metadata.is_dir()) {
        return Some(configured.join(log_name(&Local::now())));
    }
    Some(configured)
}

/// The log file name for `now` and this process.
fn log_name<Zone: TimeZone>(now: &DateTime<Zone>) -> String
where
    Zone::Offset: std::fmt::Display,
{
    format!(
        "tui-{}-{}.log",
        now.format("%Y-%m-%d_%H-%M-%S"),
        std::process::id()
    )
}

/// Appends `text` to the log at `path`; a failure is not reported.
pub(super) fn append_log(path: &Path, text: &str) {
    let _ = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .and_then(|mut log| log.write_all(text.as_bytes()));
}

impl ProcessTerminal {
    /// Shows progress and keeps it shown, or clears it. Showing it again writes it again but
    /// never starts a second keepalive or restarts the running one; clearing always writes.
    pub(super) fn show_progress(&mut self, active: bool) -> io::Result<()> {
        if !active {
            self.cancel_keepalive();
            return emit(PROGRESS_CLEAR);
        }
        emit(PROGRESS_ACTIVE)?;
        if self.progress.as_ref().is_none_or(JoinHandle::is_finished) {
            let first = Instant::now() + KEEPALIVE;
            let task = keepalive(Rc::clone(&self.shared), first);
            self.progress = Some(self.local.spawn_local(task));
        }
        Ok(())
    }

    /// Cancels the keepalive and reports whether one was running.
    pub(super) fn cancel_keepalive(&mut self) -> bool {
        self.progress.take().inspect(JoinHandle::abort).is_some()
    }
}

/// Shows progress again every [`KEEPALIVE`] from `first`, without catching up on missed ticks,
/// until a write fails.
async fn keepalive(shared: Rc<Shared>, first: Instant) {
    let mut ticks = interval_at(first, KEEPALIVE);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        if let Err(error) = emit(PROGRESS_ACTIVE) {
            return shared.fail(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, TimeZone, Utc};

    use super::log_name;

    /// The name for the instant `utc` seen from `west` hours behind UTC.
    fn name_at(west: i32, utc: (i32, u32, u32, u32, u32, u32)) -> String {
        let (year, month, day, hour, minute, second) = utc;
        let instant = Utc
            .with_ymd_and_hms(year, month, day, hour, minute, second)
            .unwrap();
        let zone = FixedOffset::west_opt(west * 3600).unwrap();
        log_name(&instant.with_timezone(&zone))
    }

    #[test]
    fn log_names_carry_the_local_calendar_fields_and_the_process_id() {
        let pid = std::process::id();
        let cases = [
            (0, (2024, 2, 29, 23, 59, 59), "2024-02-29_23-59-59"),
            (0, (2024, 3, 1, 0, 0, 0), "2024-03-01_00-00-00"),
            (5, (2024, 2, 29, 23, 59, 59), "2024-02-29_18-59-59"),
            (5, (2024, 3, 1, 0, 0, 0), "2024-02-29_19-00-00"),
            (4, (2024, 11, 3, 5, 30, 0), "2024-11-03_01-30-00"),
            (5, (2024, 11, 3, 6, 30, 0), "2024-11-03_01-30-00"),
        ];
        for (west, utc, fields) in cases {
            assert_eq!(name_at(west, utc), format!("tui-{fields}-{pid}.log"));
        }
    }
}
