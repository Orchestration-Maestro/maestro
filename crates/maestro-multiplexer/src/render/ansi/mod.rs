//! Frame encoding into terminal bytes that repaint only changed cells.
//!
//! Each frame runs inside synchronized output with the host cursor hidden
//! while cells are painted. The final cursor position, shape and visibility
//! follow, and outside Windows the cursor anchor repeats after the frame so
//! input methods place candidate windows at the real input position; Windows
//! Terminal shows that repeat as cursor movement, so it is skipped there.

mod cursor;
mod redraw;
mod style;
#[cfg(test)]
mod tests;

use cursor::CursorMemory;
use maestro_app::daemon::render_prof;
use maestro_protocol::frames::FrameData;
use ratatui::style::Modifier;
use redraw::Changes;
use std::time::Instant;

/// Ends synchronized output.
const SYNC_OUTPUT_END: &str = "\x1b[?2026l";

/// Offset of the last synchronized-output end in encoded frame bytes.
#[must_use]
pub fn final_sync_output_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(SYNC_OUTPUT_END.len())
        .rposition(|window| window == SYNC_OUTPUT_END.as_bytes())
}

/// Terminal bytes for one frame, with the cursor memory they leave behind.
pub struct EncodedBlit {
    /// Escape sequences and text ready for the host terminal.
    pub bytes: Vec<u8>,
    /// Whether every cell was painted rather than only the changed ones.
    pub full: bool,
    /// Cursor memory once these bytes are written.
    cursor: CursorMemory,
}

/// Diffs frames into terminal bytes against the last committed frame.
#[derive(Default)]
pub struct BlitEncoder {
    /// Frame the host terminal shows.
    last_frame: Option<FrameData>,
    /// Cursor memory after that frame.
    cursor: CursorMemory,
}

impl BlitEncoder {
    /// An encoder whose first frame clears the screen and paints every cell.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Encode a frame against the committed one, or paint every cell when
    /// `repaint` is set or the size changed.
    #[must_use]
    pub fn encode(&self, frame: &FrameData, repaint: bool) -> EncodedBlit {
        self.encode_inner(frame, self.shown(repaint), false)
    }

    /// Encode a frame whose cursor is drawn as cell content, keeping the host
    /// cursor hidden at the cursor position.
    #[must_use]
    pub fn encode_with_suppressed_visible_cursor(
        &self,
        frame: &FrameData,
        repaint: bool,
    ) -> EncodedBlit {
        self.encode_inner(frame, self.shown(repaint), true)
    }

    /// The committed frame to diff against, unless every cell is repainted.
    fn shown(&self, repaint: bool) -> Option<&FrameData> {
        self.last_frame.as_ref().filter(|_| !repaint)
    }

    /// Encode with the platform's anchor policy, recording opt-in profile
    /// statistics.
    fn encode_inner(
        &self,
        frame: &FrameData,
        previous: Option<&FrameData>,
        suppress_visible_cursor: bool,
    ) -> EncodedBlit {
        let full = diff_base(frame, previous).is_none();
        let stats = render_prof::enabled().then(|| ProfBlitStats::new(frame, previous));
        let started = render_prof::timer();
        let mut cursor = self.cursor;
        let policy = BlitPolicy {
            repeat_ime_anchor: !cfg!(windows),
            clear_before_full_redraw: self.last_frame.is_none(),
            suppress_visible_cursor,
        };
        let bytes = blit_frame(frame, previous, &mut cursor, policy);
        if let Some(stats) = stats {
            stats.record(bytes.len(), full, started);
        }
        EncodedBlit {
            bytes,
            full,
            cursor,
        }
    }

    /// Record that the host now shows a frame and its encoded bytes.
    pub fn commit(&mut self, frame: FrameData, encoded: &EncodedBlit) {
        self.cursor = encoded.cursor;
        self.last_frame = Some(frame);
    }

    /// Whether the host already shows this frame.
    #[must_use]
    pub fn is_current(&self, frame: &FrameData) -> bool {
        self.last_frame.as_ref() == Some(frame)
    }

    /// Frame the host shows, if any was committed.
    #[must_use]
    pub fn last_frame(&self) -> Option<&FrameData> {
        self.last_frame.as_ref()
    }
}

/// Reverse the visible cursor's cell so the cursor is drawn as content.
#[must_use]
pub fn frame_with_drawn_cursor(mut frame: FrameData) -> FrameData {
    let index = frame
        .cursor
        .as_ref()
        .filter(|cursor| cursor.visible)
        .map(|cursor| {
            let (x, y) = cursor::clamp_cursor_position(&frame, cursor.x, cursor.y);
            usize::from(y) * usize::from(frame.width) + usize::from(x)
        });
    if let Some(cell) = index.and_then(|index| frame.cells.get_mut(index)) {
        cell.modifier ^= Modifier::REVERSED.bits();
    }
    frame
}

/// Output policy for one frame.
#[derive(Clone, Copy)]
struct BlitPolicy {
    /// Repeat the final cursor anchor after synchronized output.
    repeat_ime_anchor: bool,
    /// Clear the screen before painting every cell.
    clear_before_full_redraw: bool,
    /// Keep a visible cursor hidden because the frame draws it.
    suppress_visible_cursor: bool,
}

/// The shown frame a frame can be diffed against: one of the same size.
fn diff_base<'a>(frame: &FrameData, previous: Option<&'a FrameData>) -> Option<&'a FrameData> {
    previous.filter(|previous| (previous.width, previous.height) == (frame.width, frame.height))
}

/// Encode a frame against the frame the host shows, if any.
fn blit_frame(
    frame: &FrameData,
    previous: Option<&FrameData>,
    cursor: &mut CursorMemory,
    policy: BlitPolicy,
) -> Vec<u8> {
    // Start each frame with no hyperlink, so unlinked cells never inherit one.
    let mut out = String::from("\x1b[?2026h\x1b[?25l\x1b]8;;\x1b\\");
    if let Some(previous) = diff_base(frame, previous) {
        redraw::write_changed_cells(&mut out, frame, previous);
    } else {
        if policy.clear_before_full_redraw {
            out.push_str("\x1b[2J");
        }
        redraw::write_all_cells(&mut out, frame);
    }
    let mut host = cursor.resolve(frame);
    host.visible &= !policy.suppress_visible_cursor;
    cursor.write(&mut out, host);
    out.push_str(SYNC_OUTPUT_END);
    if policy.repeat_ime_anchor {
        host.write_anchor(&mut out);
    }
    out.into_bytes()
}

/// Cell counts of one encoded frame for the opt-in render profiler.
#[derive(Default)]
struct ProfBlitStats {
    /// Cells in the frame.
    scanned_cells: u64,
    /// Cells painted.
    changed_cells: u64,
    /// Runs of adjacent painted cells within a row.
    changed_runs: u64,
}

impl ProfBlitStats {
    /// Count a frame's painted cells; a full redraw counts every unskipped
    /// cell as its own run.
    fn new(frame: &FrameData, previous: Option<&FrameData>) -> Self {
        let scanned_cells = frame.cells.len() as u64;
        let Some(previous) = diff_base(frame, previous) else {
            let changed_cells = frame.cells.iter().filter(|cell| !cell.skip).count() as u64;
            return Self {
                scanned_cells,
                changed_cells,
                changed_runs: changed_cells,
            };
        };
        let mut stats = Self {
            scanned_cells,
            ..Self::default()
        };
        let links = redraw::sanitized_hyperlinks(frame);
        let previous_links = redraw::sanitized_hyperlinks(previous);
        let changes = Changes {
            frame,
            previous,
            links: &links,
            previous_links: &previous_links,
        };
        let mut last = None;
        changes.for_each(|(col, row), _, _| {
            stats.changed_cells += 1;
            if last != Some((col.wrapping_sub(1), row)) {
                stats.changed_runs += 1;
            }
            last = Some((col, row));
        });
        stats
    }

    /// Report the counts with the encoded size and duration.
    fn record(self, bytes: usize, full: bool, started: Option<Instant>) {
        render_prof::duration_since("ansi_encode.total", started);
        render_prof::counter("ansi_encode.bytes", bytes as u64);
        render_prof::counter("ansi_encode.scanned_cells", self.scanned_cells);
        render_prof::counter("ansi_encode.changed_cells", self.changed_cells);
        render_prof::counter("ansi_encode.changed_runs", self.changed_runs);
        render_prof::event(if full {
            "ansi_encode.full"
        } else {
            "ansi_encode.partial"
        });
    }
}
