//! Decode supplied host bytes for the multiplexer input loop without performing I/O.

use maestro_protocol::frames::{ClientKeyCode, ClientMouseButton, ClientMouseKind, KeyModifiers};
use maestro_tui::stdin_buffer::{
    HostColorScheme, HostMouse, RawInputByteFramer, host_frame_length, parse_host_cell_size_report,
    parse_host_color_scheme_report, parse_host_mouse,
};
use maestro_tui::terminal_theme::{parse_default_color_response, parse_palette_color_response};

use super::{RawInputEvent, TerminalKey, parse_terminal_key_sequence};
use crate::terminal_theme::HostAppearance;

/// Opening bracketed-paste marker.
const PASTE_START: &[u8] = b"\x1b[200~";
/// Closing bracketed-paste marker.
const PASTE_END: &[u8] = b"\x1b[201~";

/// Decode a complete supplied buffer, including one idle flush of partial input.
#[must_use]
pub fn parse_raw_input_bytes_sync(data: &[u8]) -> Vec<RawInputEvent> {
    let mut framer = RawInputFramer::default();
    let mut events = framer.push(data);
    events.extend(framer.flush_timeout());
    events
}

/// Whether the bytes are exactly one complete UTF-8 bracketed paste.
#[must_use]
pub fn is_complete_text_bracketed_paste(data: &[u8]) -> bool {
    data.starts_with(PASTE_START) && host_frame_length(data) == Some(data.len())
}

/// Event projection over the toolkit's sole retained host byte owner.
///
/// The multiplexer input loop supplies bytes and idle flush notifications.
#[derive(Default)]
pub struct RawInputFramer {
    /// Retained byte framing and bounded host-reply windows.
    byte_framer: RawInputByteFramer,
}

impl RawInputFramer {
    /// Select the current host's doubled-Escape policy.
    #[must_use]
    pub fn for_host_input() -> Self {
        Self {
            byte_framer: RawInputByteFramer::for_host_input(),
        }
    }

    /// Project all complete frames from this supplied chunk in order.
    pub fn push(&mut self, data: &[u8]) -> Vec<RawInputEvent> {
        Self::events_from_chunks(self.byte_framer.push(data))
    }

    /// Arm the bounded window after the host colour queries are sent.
    pub fn host_color_query_sent(&mut self) {
        self.byte_framer.host_color_query_sent();
    }

    /// Rearm colour-query windows after host appearance reports.
    pub fn enable_host_color_scheme_change_tracking(&mut self) {
        self.byte_framer.enable_host_color_scheme_change_tracking();
    }

    /// Rearm the appearance-reply window after host focus gain.
    pub fn enable_host_appearance_query_on_focus(&mut self) {
        self.byte_framer.enable_host_appearance_query_on_focus();
    }

    /// Whether any bytes remain pending.
    #[must_use]
    pub fn has_pending_input(&self) -> bool {
        self.byte_framer.has_pending_input()
    }

    /// Whether the current partial frame is a mouse report.
    #[must_use]
    pub fn has_pending_incomplete_mouse_sequence(&self) -> bool {
        self.byte_framer.has_pending_incomplete_mouse_sequence()
    }

    /// Whether an incomplete bracketed paste remains pending.
    #[must_use]
    pub fn has_pending_bracketed_paste(&self) -> bool {
        self.byte_framer.has_pending_bracketed_paste()
    }

    /// Apply one idle flush and project any resulting frames.
    pub fn flush_timeout(&mut self) -> Vec<RawInputEvent> {
        Self::events_from_chunks(self.byte_framer.flush_timeout())
    }

    /// Project complete original chunks, preserving VT evidence on keys.
    fn events_from_chunks(chunks: Vec<Vec<u8>>) -> Vec<RawInputEvent> {
        chunks
            .into_iter()
            .filter_map(|chunk| {
                if chunk == [27] {
                    return Some(RawInputEvent::Key(
                        TerminalKey::new(ClientKeyCode::Esc, KeyModifiers::NONE)
                            .with_vt_bytes(chunk),
                    ));
                }
                extract_one_event(&chunk).map(|(event, _)| {
                    tracing::debug!(raw_bytes = ?chunk, event = ?event, "raw input event parsed");
                    event
                })
            })
            .collect()
    }
}

/// Idle flush deadline for ordinary incomplete input.
const RAW_INPUT_IDLE_FLUSH_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(10);
/// Idle flush deadline while a mouse report remains incomplete.
const MOUSE_ACTIVE_ESCAPE_SEQUENCE_FLUSH_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(150);

/// Select the multiplexer input loop's idle timeout without scheduling a timer.
#[must_use]
pub fn input_flush_timeout(framer: &RawInputFramer) -> std::time::Duration {
    if framer.has_pending_incomplete_mouse_sequence() {
        MOUSE_ACTIVE_ESCAPE_SEQUENCE_FLUSH_TIMEOUT
    } else {
        RAW_INPUT_IDLE_FLUSH_TIMEOUT
    }
}

/// Whether an enabled focus redraw is required by the supplied events.
#[must_use]
pub fn events_require_host_surface_redraw(
    events: &[RawInputEvent],
    redraw_on_focus_gained: bool,
) -> bool {
    redraw_on_focus_gained && events_require_host_terminal_appearance_query(events)
}

/// Whether focus gain requests an appearance query from the multiplexer input loop.
#[must_use]
pub fn events_require_host_terminal_appearance_query(events: &[RawInputEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, RawInputEvent::OuterFocusGained))
}

/// Whether an appearance report requests colour queries from the multiplexer input loop.
#[must_use]
pub fn events_require_host_terminal_theme_query(events: &[RawInputEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, RawInputEvent::HostColorSchemeChanged(_)))
}

/// Project the first complete frame using the shared toolkit admission engine.
pub(super) fn extract_one_event(buffer: &[u8]) -> Option<(RawInputEvent, usize)> {
    let len = host_frame_length(buffer)?;
    let frame = &buffer[..len];
    let event = if let Some(content) = frame.strip_prefix(PASTE_START) {
        RawInputEvent::Paste(
            std::str::from_utf8(content.strip_suffix(PASTE_END)?)
                .ok()?
                .to_owned(),
        )
    } else if frame.starts_with(b"\x1b[M") {
        parse_host_mouse(frame).map_or(RawInputEvent::Unsupported, project_mouse)
    } else {
        let text = std::str::from_utf8(frame).ok()?;
        if frame.first() == Some(&27) {
            project_escape(frame, text)
        } else {
            RawInputEvent::Key(
                parse_terminal_key_sequence(text)?
                    .with_text_commit()
                    .with_vt_bytes(frame.to_vec()),
            )
        }
    };
    Some((event, len))
}

/// Decode host reports before projecting escape-prefixed keys.
fn project_escape(frame: &[u8], text: &str) -> RawInputEvent {
    if let Some((kind, color)) = parse_default_color_response(text) {
        return RawInputEvent::HostDefaultColor { kind, color };
    }
    if let Some((index, color)) = parse_palette_color_response(text) {
        return RawInputEvent::HostPaletteColors {
            colors: vec![(index, color)],
        };
    }
    match frame {
        b"\x1b[I" => return RawInputEvent::OuterFocusGained,
        b"\x1b[O" => return RawInputEvent::OuterFocusLost,
        _ => {}
    }
    if let Some(scheme) = parse_host_color_scheme_report(frame) {
        return RawInputEvent::HostColorSchemeChanged(match scheme {
            HostColorScheme::Dark => HostAppearance::Dark,
            HostColorScheme::Light => HostAppearance::Light,
        });
    }
    if let Some((width_px, height_px)) = parse_host_cell_size_report(frame) {
        return RawInputEvent::HostCellSizeReport {
            width_px,
            height_px,
        };
    }
    if let Some(mouse) = parse_host_mouse(frame) {
        return project_mouse(mouse);
    }
    if let Some(key) = parse_terminal_key_sequence(text) {
        return RawInputEvent::Key(key.with_vt_bytes(frame.to_vec()));
    }
    tracing::debug!(sequence = ?text, "dropping unsupported escape sequence");
    RawInputEvent::Unsupported
}

/// Map admitted host mouse classes onto the existing wire records.
fn project_mouse(mouse: HostMouse) -> RawInputEvent {
    let button = (mouse.cb & 3) | ((mouse.cb & 0xc0) >> 4);
    let dragging = mouse.cb & 32 != 0;
    let kind = match button {
        0..=2 => {
            let button = match button {
                0 => ClientMouseButton::Left,
                1 => ClientMouseButton::Middle,
                _ => ClientMouseButton::Right,
            };
            if dragging {
                ClientMouseKind::Drag(button)
            } else if mouse.release {
                ClientMouseKind::Up(button)
            } else {
                ClientMouseKind::Down(button)
            }
        }
        3 if !dragging => ClientMouseKind::Up(ClientMouseButton::Left),
        _ if dragging => ClientMouseKind::Moved,
        4 => ClientMouseKind::ScrollUp,
        5 => ClientMouseKind::ScrollDown,
        6 => ClientMouseKind::ScrollLeft,
        _ => ClientMouseKind::ScrollRight,
    };
    let mut modifiers = KeyModifiers::NONE;
    for (bit, wire) in [
        (4, KeyModifiers::SHIFT),
        (8, KeyModifiers::ALT),
        (16, KeyModifiers::CONTROL),
    ] {
        if mouse.cb & bit != 0 {
            modifiers |= wire;
        }
    }
    RawInputEvent::Mouse {
        kind,
        column: mouse.column,
        row: mouse.row,
        modifiers,
    }
}
