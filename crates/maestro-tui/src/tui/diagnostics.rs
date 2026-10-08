//! Diagnostic logs and errors of the frame writer: redraw reasons, the width crash
//! report and the record of differential updates.

use std::fmt::{self, Write as _};
use std::io;
use std::path::{Path, PathBuf};

use crate::images::terminal_image::is_image_line;
use crate::text::utils::visible_width;

use super::TUI;
use super::drawing::Update;
use super::runtime::{LogContext, is_enabled};
use super::screen::{Changes, Frame};

/// Variable that logs every full redraw when set to `1`.
const DEBUG_REDRAW: &str = "MAESTRO_DEBUG_REDRAW";
/// Variable that records every differential update when set to `1`.
const DEBUG_RENDER: &str = "MAESTRO_TUI_DEBUG";
/// Directory that receives the records of every differential update.
const RENDER_DIRECTORY: &str = "/tmp/tui";

/// A file in the application's log directory.
fn agent_log(context: &LogContext, name: &str) -> PathBuf {
    context.home.join(".maestro").join("agent").join(name)
}

/// The report of a frame that holds an oversized line.
fn crash_report(context: &LogContext, frame: &Frame, row: usize, measured: usize) -> String {
    let mut report = format!(
        "Crash at {}\nTerminal width: {}\nLine {row} visible width: {measured}\n\n\
         === All rendered lines ===\n",
        context.iso_time, frame.width
    );
    for (index, line) in frame.lines.iter().enumerate() {
        let _ = writeln!(report, "[{index}] (w={}) {line}", visible_width(line));
    }
    report
}

/// The error text for an oversized line.
fn overflow_message(row: usize, measured: usize, width: usize, log: &Path) -> String {
    format!(
        "Rendered line {row} exceeds terminal width ({measured} > {width}).\n\n\
         This is likely caused by a custom TUI component not truncating its output.\n\
         Use visibleWidth() to measure and truncateToWidth() to truncate lines.\n\n\
         Debug log written to: {}",
        log.display()
    )
}

impl TUI {
    /// Appends the reason of a full redraw to the redraw log when it is switched on.
    ///
    /// # Errors
    ///
    /// Returns the host's file error unchanged.
    pub(super) fn log_redraw(&self, reason: &RedrawReason, frame: &Frame) -> io::Result<()> {
        if !is_enabled(&*self.shared.runtime, DEBUG_REDRAW) {
            return Ok(());
        }
        let context = self.shared.runtime.log_context();
        let previous = self.shared.screen.borrow().previous_lines.len();
        let message = format!(
            "[{}] fullRender: {reason} (prev={previous}, new={}, height={})\n",
            context.iso_time,
            frame.lines.len(),
            frame.height
        );
        let path = agent_log(&context, "maestro-debug.log");
        self.shared.runtime.append_log(&path, &message)
    }

    /// Fails with a crash report when a text line is wider than the terminal: the report
    /// is written, the terminal is restored and nothing of the frame is drawn.
    ///
    /// # Errors
    ///
    /// Returns the width error, or the host's or terminal's error if cleanup fails.
    pub(super) fn reject_overflow(&self, frame: &Frame) -> io::Result<()> {
        let oversized = frame
            .lines
            .iter()
            .enumerate()
            .filter(|(_, line)| !is_image_line(line))
            .map(|(row, line)| (row, visible_width(line)))
            .find(|&(_, measured)| measured > frame.width);
        let Some((row, measured)) = oversized else {
            return Ok(());
        };
        let context = self.shared.runtime.log_context();
        let path = agent_log(&context, "maestro-crash.log");
        let report = crash_report(&context, frame, row, measured);
        self.shared.runtime.write_log(&path, &report)?;
        self.stop()?;
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            overflow_message(row, measured, frame.width, &path),
        ))
    }

    /// Writes the record of a differential update when it is switched on.
    ///
    /// # Errors
    ///
    /// Returns the host's file error unchanged.
    pub(super) fn record_differential(
        &self,
        frame: &Frame,
        changes: &Changes,
        update: &Update,
        viewport_top: usize,
    ) -> io::Result<()> {
        if !is_enabled(&*self.shared.runtime, DEBUG_RENDER) {
            return Ok(());
        }
        let report = {
            let screen = self.shared.screen.borrow();
            let render_end = update.render_end;
            format!(
                "firstChanged: {}\nviewportTop: {viewport_top}\ncursorRow: {}\nheight: {}\n\
                 lineDiff: {}\nhardwareCursorRow: {}\nrenderEnd: {render_end}\n\
                 finalCursorRow: {render_end}\ncursorPos: {}\nnewLines.length: {}\n\
                 previousLines.length: {}\n\n=== newLines ===\n{}\n\n=== previousLines ===\n{}\n\n\
                 === buffer ===\n{}",
                changes.first,
                screen.cursor_row,
                frame.height,
                update.line_diff,
                update.hardware_row,
                serde_json::to_string(&frame.cursor)?,
                frame.lines.len(),
                screen.previous_lines.len(),
                serde_json::to_string_pretty(&frame.lines)?,
                serde_json::to_string_pretty(&screen.previous_lines)?,
                serde_json::to_string(&update.buffer)?,
            )
        };
        let context = self.shared.runtime.log_context();
        let name = format!("render-{}-{}.log", context.unix_ms, context.nonce);
        self.shared
            .runtime
            .write_log(&Path::new(RENDER_DIRECTORY).join(name), &report)
    }
}

/// Why the whole screen is redrawn instead of updated in place.
pub(super) enum RedrawReason {
    /// Nothing has been drawn yet.
    First,
    /// The terminal changed width.
    Width {
        /// Width of the retained frame, absent after a forced redraw.
        from: Option<usize>,
        /// Current width.
        to: usize,
    },
    /// The terminal changed height.
    Height {
        /// Height of the retained frame, absent after a forced redraw.
        from: Option<usize>,
        /// Current height.
        to: usize,
    },
    /// Content shrank below the most rows ever drawn while clear-on-shrink is on.
    ClearOnShrink {
        /// The most rows drawn so far.
        max_lines_rendered: usize,
    },
    /// Erasing rows would scroll the viewport upward.
    DeletedLinesMovedViewport {
        /// Last row of the new content.
        target_row: usize,
        /// First content row on screen.
        viewport_top: usize,
    },
    /// A changed row is above the screen.
    ChangedAboveViewport {
        /// First changed row.
        first_changed: usize,
        /// First content row on screen.
        viewport_top: usize,
    },
}

impl RedrawReason {
    /// Whether the screen and scrollback are cleared before the redraw.
    pub(super) fn clears(&self) -> bool {
        !matches!(self, Self::First)
    }
}

/// A retained extent as logged; a forced redraw discarded it and logs `-1`.
fn extent(retained: Option<usize>) -> String {
    retained.map_or_else(|| "-1".to_owned(), |size| size.to_string())
}

impl fmt::Display for RedrawReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::First => formatter.write_str("first render"),
            Self::Width { from, to } => {
                write!(
                    formatter,
                    "terminal width changed ({} -> {to})",
                    extent(*from)
                )
            }
            Self::Height { from, to } => {
                write!(
                    formatter,
                    "terminal height changed ({} -> {to})",
                    extent(*from)
                )
            }
            Self::ClearOnShrink { max_lines_rendered } => {
                write!(
                    formatter,
                    "clearOnShrink (maxLinesRendered={max_lines_rendered})"
                )
            }
            Self::DeletedLinesMovedViewport {
                target_row,
                viewport_top,
            } => write!(
                formatter,
                "deleted lines moved viewport up ({target_row} < {viewport_top})"
            ),
            Self::ChangedAboveViewport {
                first_changed,
                viewport_top,
            } => write!(
                formatter,
                "firstChanged < viewportTop ({first_changed} < {viewport_top})"
            ),
        }
    }
}
