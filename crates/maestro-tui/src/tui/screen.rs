//! The frame the terminal shows and the choice of how to bring it up to date.

use std::io;

use super::TUI;
use super::diagnostics::RedrawReason;
use super::drawing::{self, CursorPosition, Update};

/// A terminal extent the retained frame was drawn for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Extent {
    /// No frame has been drawn.
    #[default]
    Unknown,
    /// A forced redraw discarded the extent, so any current extent differs.
    Stale,
    /// The extent of the retained frame.
    Known(usize),
}

impl Extent {
    /// Whether a frame was drawn for a different extent than `size`.
    fn differs_from(self, size: usize) -> bool {
        match self {
            Self::Unknown => false,
            Self::Stale => true,
            Self::Known(known) => known != size,
        }
    }

    /// The extent of the retained frame, absent after a forced redraw discarded it.
    fn retained(self) -> Option<usize> {
        match self {
            Self::Known(size) => Some(size),
            Self::Unknown | Self::Stale => None,
        }
    }
}

/// Environment-dependent choices that shape a plan.
pub(super) struct Policy {
    /// Whether shrinking content clears the screen.
    pub(super) clear_on_shrink: bool,
    /// Whether height changes keep the retained frame.
    pub(super) mobile: bool,
}

/// A frame ready to draw.
pub(super) struct Frame {
    /// Lines with their cursor marker removed and their resets applied.
    pub(super) lines: Vec<String>,
    /// Where the hardware cursor belongs.
    pub(super) cursor: Option<CursorPosition>,
    /// Terminal width in columns.
    pub(super) width: usize,
    /// Terminal height in rows.
    pub(super) height: usize,
}

/// The rows that differ between the retained frame and a new one.
pub(super) struct Changes {
    /// First differing row.
    pub(super) first: usize,
    /// Last differing row.
    pub(super) last: usize,
    /// Whether the new frame only adds rows below the retained ones.
    pub(super) append_start: bool,
}

/// How the terminal is brought up to date with a frame.
pub(super) enum Plan {
    /// Draw every line, after clearing the screen and scrollback unless this is the first
    /// frame.
    Full(RedrawReason),
    /// Erase rows the new frame no longer has.
    DeleteTail {
        /// The rows that changed.
        changes: Changes,
        /// First content row on screen.
        viewport_top: usize,
    },
    /// Redraw only the rows that changed.
    Differential {
        /// The rows that changed.
        changes: Changes,
        /// First content row on screen.
        viewport_top: usize,
    },
    /// Nothing changed; only the cursor may move.
    Unchanged {
        /// First content row on screen.
        viewport_top: usize,
    },
}

/// What the terminal currently shows, as far as the writer knows.
#[derive(Default)]
pub(super) struct Screen {
    /// Lines of the latest frame.
    pub(super) previous_lines: Vec<String>,
    /// Ids of the images on the latest frame.
    pub(super) previous_image_ids: Vec<u32>,
    /// Width the latest frame was drawn for.
    pub(super) previous_width: Extent,
    /// Height the latest frame was drawn for.
    pub(super) previous_height: Extent,
    /// Row of the end of the content.
    pub(super) cursor_row: usize,
    /// Row the terminal cursor is on.
    pub(super) hardware_cursor_row: usize,
    /// The most rows the terminal has been asked to hold.
    pub(super) max_lines_rendered: usize,
    /// First content row on screen.
    pub(super) previous_viewport_top: usize,
    /// Full redraws begun so far.
    pub(super) full_redraws: usize,
}

impl Screen {
    /// Chooses how to bring the terminal up to date with `frame`.
    pub(super) fn plan(&self, frame: &Frame, policy: &Policy) -> Plan {
        let width_changed = self.previous_width.differs_from(frame.width);
        let height_changed = self.previous_height.differs_from(frame.height);
        if self.previous_lines.is_empty() && !width_changed && !height_changed {
            return Plan::Full(RedrawReason::First);
        }
        if width_changed {
            return Plan::Full(RedrawReason::Width {
                from: self.previous_width.retained(),
                to: frame.width,
            });
        }
        if height_changed && !policy.mobile {
            return Plan::Full(RedrawReason::Height {
                from: self.previous_height.retained(),
                to: frame.height,
            });
        }
        if policy.clear_on_shrink && frame.lines.len() < self.max_lines_rendered {
            return Plan::Full(RedrawReason::ClearOnShrink {
                max_lines_rendered: self.max_lines_rendered,
            });
        }
        let viewport_top = self.viewport_top_after_resize(frame.height, height_changed);
        let Some(changes) = self.changes(&frame.lines) else {
            return Plan::Unchanged { viewport_top };
        };
        let target_row = frame.lines.len().saturating_sub(1);
        if changes.first >= frame.lines.len() {
            if target_row < viewport_top {
                return Plan::Full(RedrawReason::DeletedLinesMovedViewport {
                    target_row,
                    viewport_top,
                });
            }
            return Plan::DeleteTail {
                changes,
                viewport_top,
            };
        }
        if changes.first < viewport_top {
            return Plan::Full(RedrawReason::ChangedAboveViewport {
                first_changed: changes.first,
                viewport_top,
            });
        }
        Plan::Differential {
            changes,
            viewport_top,
        }
    }

    /// First content row on screen once the terminal has the height `height`.
    fn viewport_top_after_resize(&self, height: usize, height_changed: bool) -> usize {
        if !height_changed {
            return self.previous_viewport_top;
        }
        let buffer_length = match self.previous_height {
            Extent::Known(previous) => self.previous_viewport_top + previous,
            Extent::Unknown | Extent::Stale => height,
        };
        buffer_length.saturating_sub(height)
    }

    /// Forgets the retained frame so the next frame is drawn from scratch.
    pub(super) fn forget_frame(&mut self) {
        self.previous_lines.clear();
        self.previous_width = Extent::Stale;
        self.previous_height = Extent::Stale;
        self.cursor_row = 0;
        self.hardware_cursor_row = 0;
        self.max_lines_rendered = 0;
        self.previous_viewport_top = 0;
    }

    /// The rows that differ from `lines`, or `None` when the frames are equal.
    fn changes(&self, lines: &[String]) -> Option<Changes> {
        let previous = &self.previous_lines;
        let shared = previous.len().min(lines.len());
        let differs = |row: &usize| previous[*row] != lines[*row];
        let first_diff = (0..shared).find(differs);
        let (first, last) = if previous.len() == lines.len() {
            (first_diff?, (0..shared).rfind(differs)?)
        } else {
            (
                first_diff.unwrap_or(shared),
                previous.len().max(lines.len()) - 1,
            )
        };
        let last = (first..previous.len())
            .rfind(|&row| drawing::kitty_image_id(&previous[row]).is_some())
            .map_or(last, |row| last.max(row));
        Some(Changes {
            first,
            last,
            append_start: lines.len() > previous.len() && first == previous.len() && first > 0,
        })
    }

    /// Updates the row bookkeeping after a differential update.
    fn after_differential(&mut self, frame: &Frame, update: &Update, viewport_top: usize) {
        let rows = frame.lines.len();
        self.cursor_row = rows.saturating_sub(1);
        self.hardware_cursor_row = update.render_end;
        self.max_lines_rendered = self.max_lines_rendered.max(rows);
        self.previous_viewport_top =
            viewport_top.max((update.render_end + 1).saturating_sub(frame.height));
    }

    /// Remembers `frame` as what the terminal shows.
    fn commit(&mut self, frame: Frame) {
        self.previous_image_ids = drawing::image_ids(&frame.lines);
        self.previous_lines = frame.lines;
        self.previous_width = Extent::Known(frame.width);
        self.previous_height = Extent::Known(frame.height);
    }

    /// Updates the row bookkeeping after a full draw, which takes the content to end on the
    /// frame's last row; `clear` restarts the count of most rows drawn instead of raising it.
    fn after_full(&mut self, frame: &Frame, clear: bool) {
        let rows = frame.lines.len();
        self.cursor_row = rows.saturating_sub(1);
        self.hardware_cursor_row = self.cursor_row;
        self.max_lines_rendered = if clear {
            rows
        } else {
            self.max_lines_rendered.max(rows)
        };
        self.previous_viewport_top = rows.max(frame.height) - frame.height;
    }
}

impl TUI {
    /// Draws the next frame.
    pub(super) fn do_render(&self) -> io::Result<()> {
        let (width, height) = {
            let terminal = self.shared.terminal.borrow();
            (terminal.columns(), terminal.rows())
        };
        let mut lines = self.render_children(width);
        let cursor = drawing::extract_cursor_position(&mut lines, height);
        drawing::apply_line_resets(&mut lines);
        let frame = Frame {
            lines,
            cursor,
            width,
            height,
        };
        self.reject_overflow(&frame)?;
        let policy = Policy {
            clear_on_shrink: self.shared.clear_on_shrink.get(),
            mobile: self
                .shared
                .runtime
                .environment("TERMUX_VERSION")
                .is_some_and(|version| !version.is_empty()),
        };
        let plan = self.shared.screen.borrow().plan(&frame, &policy);
        match plan {
            Plan::Full(reason) => self.full_render(frame, &reason),
            Plan::DeleteTail {
                changes,
                viewport_top,
            } => self.delete_tail(frame, &changes, viewport_top),
            Plan::Differential {
                changes,
                viewport_top,
            } => self.differential(frame, &changes, viewport_top),
            Plan::Unchanged { viewport_top } => {
                self.position_hardware_cursor(&frame)?;
                let mut screen = self.shared.screen.borrow_mut();
                screen.previous_viewport_top = viewport_top;
                screen.previous_height = Extent::Known(frame.height);
                Ok(())
            }
        }
    }

    /// Draws every line in one pass, after clearing the screen and scrollback unless the
    /// reason is the first frame.
    fn full_render(&self, frame: Frame, reason: &RedrawReason) -> io::Result<()> {
        self.log_redraw(reason, &frame)?;
        let clear = reason.clears();
        let buffer = {
            let mut screen = self.shared.screen.borrow_mut();
            screen.full_redraws += 1;
            drawing::full_frame(
                &frame.lines,
                clear.then_some(&screen.previous_image_ids[..]),
            )
        };
        self.write(&buffer)?;
        self.shared.screen.borrow_mut().after_full(&frame, clear);
        self.position_hardware_cursor(&frame)?;
        self.shared.screen.borrow_mut().commit(frame);
        Ok(())
    }

    /// Erases the rows a shorter frame no longer has.
    fn delete_tail(&self, frame: Frame, changes: &Changes, viewport_top: usize) -> io::Result<()> {
        let target_row = frame.lines.len().saturating_sub(1);
        let buffer = {
            let screen = self.shared.screen.borrow();
            let hardware_row = screen.hardware_cursor_row;
            drawing::delete_tail(
                &screen.previous_lines,
                frame.lines.len(),
                changes,
                hardware_row,
            )
        };
        self.write(&buffer)?;
        {
            let mut screen = self.shared.screen.borrow_mut();
            screen.cursor_row = target_row;
            screen.hardware_cursor_row = target_row;
        }
        self.position_hardware_cursor(&frame)?;
        let mut screen = self.shared.screen.borrow_mut();
        screen.commit(frame);
        screen.previous_viewport_top = viewport_top;
        Ok(())
    }

    /// Redraws only the changed rows.
    fn differential(&self, frame: Frame, changes: &Changes, viewport_top: usize) -> io::Result<()> {
        let update = {
            let screen = self.shared.screen.borrow();
            let hardware_row = screen.hardware_cursor_row;
            drawing::differential(&screen.previous_lines, &frame.lines, changes, hardware_row)
        };
        self.record_differential(&frame, changes, &update, viewport_top)?;
        self.write(&update.buffer)?;
        self.shared
            .screen
            .borrow_mut()
            .after_differential(&frame, &update, viewport_top);
        self.position_hardware_cursor(&frame)?;
        self.shared.screen.borrow_mut().commit(frame);
        Ok(())
    }

    /// Puts the terminal cursor where the frame asks for it, or hides it.
    fn position_hardware_cursor(&self, frame: &Frame) -> io::Result<()> {
        let Some(cursor) = frame.cursor.filter(|_| !frame.lines.is_empty()) else {
            return self.shared.terminal.borrow_mut().hide_cursor();
        };
        let hardware_row = self.shared.screen.borrow().hardware_cursor_row;
        let motion = drawing::move_rows(drawing::row_delta(hardware_row, cursor.row));
        self.write(&format!("{motion}\x1b[{}G", cursor.col + 1))?;
        self.shared.screen.borrow_mut().hardware_cursor_row = cursor.row;
        if self.shared.show_hardware_cursor.get() {
            self.shared.terminal.borrow_mut().show_cursor()
        } else {
            self.shared.terminal.borrow_mut().hide_cursor()
        }
    }

    /// Moves the cursor to the line below the retained content so the shell prompt that
    /// follows does not overwrite it.
    pub(super) fn move_below_content(&self) -> io::Result<()> {
        let (rows, hardware_row) = {
            let screen = self.shared.screen.borrow();
            (screen.previous_lines.len(), screen.hardware_cursor_row)
        };
        if rows == 0 {
            return Ok(());
        }
        let motion = drawing::move_rows(drawing::row_delta(hardware_row, rows));
        if !motion.is_empty() {
            self.write(&motion)?;
        }
        self.write("\r\n")
    }

    /// Writes `data` to the terminal.
    fn write(&self, data: &str) -> io::Result<()> {
        self.shared.terminal.borrow_mut().write(data)
    }
}
