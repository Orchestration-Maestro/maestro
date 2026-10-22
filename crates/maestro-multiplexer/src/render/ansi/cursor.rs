//! Final host cursor placement after a frame's cells are painted.

use maestro_protocol::frames::FrameData;
use std::fmt::Write as _;

/// Cursor facts the encoder carries from one frame to the next.
#[derive(Clone, Copy, Default)]
pub(super) struct CursorMemory {
    /// Last position where a frame showed the cursor.
    last_visible: Option<(u16, u16)>,
    /// Last DECSCUSR parameter written to the host.
    last_shape: u8,
}

/// Final cursor state for the host terminal.
#[derive(Clone, Copy)]
pub(super) struct HostCursorState {
    /// Clamped column and row.
    position: (u16, u16),
    /// Whether the host shows the cursor.
    pub(super) visible: bool,
    /// DECSCUSR parameter from 0 to 6, where 0 is the terminal default.
    shape: u8,
}

impl CursorMemory {
    /// Resolve a frame's cursor; without one, park the hidden cursor where it
    /// was last shown, or at the bottom-right cell.
    pub(super) fn resolve(&mut self, frame: &FrameData) -> HostCursorState {
        let Some(cursor) = &frame.cursor else {
            let position = self.last_visible.map_or_else(
                || {
                    (
                        frame.width.saturating_sub(1),
                        frame.height.saturating_sub(1),
                    )
                },
                |(x, y)| clamp_cursor_position(frame, x, y),
            );
            return HostCursorState {
                position,
                visible: false,
                shape: 0,
            };
        };
        let position = clamp_cursor_position(frame, cursor.x, cursor.y);
        if cursor.visible {
            self.last_visible = Some(position);
        }
        HostCursorState {
            position,
            visible: cursor.visible,
            shape: if cursor.shape <= 6 { cursor.shape } else { 0 },
        }
    }

    /// Move the still hidden cursor, set a changed shape, then apply its
    /// visibility, so nothing observes the cursor before it arrives.
    pub(super) fn write(&mut self, out: &mut String, cursor: HostCursorState) {
        write_position(out, cursor.position);
        if cursor.shape != self.last_shape {
            let _ = write!(out, "\x1b[{} q", cursor.shape);
            self.last_shape = cursor.shape;
        }
        write_visibility(out, cursor.visible);
    }
}

impl HostCursorState {
    /// Repeat only the position and visibility, for input methods that miss
    /// cursor moves inside synchronized output.
    pub(super) fn write_anchor(self, out: &mut String) {
        write_position(out, self.position);
        write_visibility(out, self.visible);
    }
}

/// Keep a cursor position inside the frame.
pub(super) fn clamp_cursor_position(frame: &FrameData, x: u16, y: u16) -> (u16, u16) {
    (
        x.min(frame.width.saturating_sub(1)),
        y.min(frame.height.saturating_sub(1)),
    )
}

/// Move the host cursor to a zero-based cell.
pub(super) fn write_position(out: &mut String, (x, y): (u16, u16)) {
    let _ = write!(out, "\x1b[{};{}H", u32::from(y) + 1, u32::from(x) + 1);
}

/// Show or hide the host cursor.
fn write_visibility(out: &mut String, visible: bool) {
    out.push_str(if visible { "\x1b[?25h" } else { "\x1b[?25l" });
}
