//! Bounded recognition and disposal of incomplete host reply tails.

use super::framing::{
    ControlString, ControlStringFamily, ESC, control_string, control_string_terminator_for_family,
};
use super::mouse::parse_host_mouse;
use super::raw_input::{MAX_DISCARDED_CONTROL_TAIL_BYTES, MAX_ORPHANED_SGR_MOUSE_TAIL_BYTES};

/// Whether an unterminated tail remains plausible within its source family.
pub(super) fn plausible_control_string_tail(family: ControlStringFamily, buffer: &[u8]) -> bool {
    match family {
        ControlStringFamily::Osc => buffer.iter().all(|byte| {
            byte.is_ascii_digit()
                || matches!(
                    *byte,
                    b';' | b':'
                        | b'/'
                        | b'#'
                        | b'?'
                        | b'.'
                        | b'_'
                        | b'-'
                        | b'+'
                        | b'r'
                        | b'g'
                        | b'b'
                        | b'R'
                        | b'G'
                        | b'B'
                        | ESC
                )
        }),
        ControlStringFamily::StTerminated => buffer.last() == Some(&ESC),
        ControlStringFamily::HostReplyCsi => false,
        ControlStringFamily::OrphanedSgrMouseTail => buffer
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(*byte, b';' | b'M' | b'm')),
    }
}
/// Recognize incomplete OSC default-colour replies that must keep waiting.
pub(super) fn starts_with_incomplete_default_color_response(buffer: &[u8]) -> bool {
    matches!(
        control_string(buffer),
        Some(ControlString::Incomplete {
            family: ControlStringFamily::Osc
        })
    ) && matches!(buffer.get(..5), Some(b"\x1b]10;" | b"\x1b]11;"))
}
/// Recognize a proper incomplete prefix of either appearance report.
pub(super) fn starts_with_incomplete_host_color_scheme_report(buffer: &[u8]) -> bool {
    buffer.starts_with(b"\x1b[?")
        && (DARK_REPORT.starts_with(buffer) || LIGHT_REPORT.starts_with(buffer))
        && buffer.len() < DARK_REPORT.len()
}
/// Recognize an incomplete cell-size reply with its source field shapes.
pub(super) fn starts_with_incomplete_host_cell_size_report(buffer: &[u8]) -> bool {
    let Some(body) = buffer.strip_prefix(b"\x1b[") else {
        return false;
    };
    if body.is_empty() || body.last() == Some(&b't') {
        return false;
    }

    let mut params = body.split(|byte| *byte == b';');
    if params.next() != Some(b"6".as_slice()) {
        return false;
    }
    let height = params.next();
    let width = params.next();
    params.next().is_none()
        && height.is_none_or(|value| value.iter().all(u8::is_ascii_digit))
        && width.is_none_or(|value| value.iter().all(u8::is_ascii_digit))
        && !(height.is_some_and(<[u8]>::is_empty) && width.is_some())
}
/// Whether the SGR mouse introducer is followed only by unfinished fields.
pub(super) fn starts_with_incomplete_sgr_mouse_sequence(buffer: &[u8]) -> bool {
    buffer.starts_with(b"\x1b[<")
        && buffer[3..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || *byte == b';')
}
/// Whether a default mouse frame is shorter than six bytes.
pub(super) fn starts_with_incomplete_default_mouse_sequence(buffer: &[u8]) -> bool {
    buffer.starts_with(b"\x1b[M") && buffer.len() < 6
}
/// Recognize a bounded incomplete mouse tail after a flushed Escape.
pub(super) fn starts_with_incomplete_orphaned_sgr_mouse_tail(buffer: &[u8]) -> bool {
    if buffer.len() > MAX_ORPHANED_SGR_MOUSE_TAIL_BYTES {
        return false;
    }
    buffer.len() < 3 && b"[<".starts_with(buffer)
        || buffer.starts_with(b"[<")
            && buffer[2..]
                .iter()
                .all(|byte| byte.is_ascii_digit() || *byte == b';')
}
/// Discard a bounded complete mouse tail only after host numeric admission.
pub(super) fn discard_complete_orphaned_sgr_mouse_tail(buffer: &mut Vec<u8>) -> bool {
    let Some(terminator_len) =
        control_string_terminator_for_family(buffer, ControlStringFamily::OrphanedSgrMouseTail)
    else {
        return false;
    };
    if terminator_len > MAX_ORPHANED_SGR_MOUSE_TAIL_BYTES {
        return false;
    }
    let mut sequence = Vec::with_capacity(terminator_len + 1);
    sequence.push(ESC);
    sequence.extend_from_slice(&buffer[..terminator_len]);
    if parse_host_mouse(&sequence).is_none() {
        return false;
    }
    buffer.drain(..terminator_len);
    true
}
/// Discard an orphaned mouse prefix and arm its bounded tail window.
pub(super) fn discard_or_buffer_orphaned_sgr_mouse_tail(
    buffer: &mut Vec<u8>,
    discard_until: &mut Option<ControlStringFamily>,
    discarded_tail_bytes: &mut usize,
) {
    if !discard_complete_orphaned_sgr_mouse_tail(buffer) {
        *discarded_tail_bytes = buffer.len();
        *discard_until = (*discarded_tail_bytes <= MAX_DISCARDED_CONTROL_TAIL_BYTES)
            .then_some(ControlStringFamily::OrphanedSgrMouseTail);
        buffer.clear();
    }
}
/// Consume bounded CSI parameters and their final, preserving unrelated bytes.
pub(super) fn discard_host_reply_csi_tail(
    buffer: &mut Vec<u8>,
    discarded_tail_bytes: &mut usize,
) -> bool {
    let remaining = MAX_DISCARDED_CONTROL_TAIL_BYTES.saturating_sub(*discarded_tail_bytes);
    let inspected = buffer.len().min(remaining);

    for index in 0..inspected {
        match buffer[index] {
            0x20..=0x3f => {}
            0x40..=0x7e => {
                buffer.drain(..=index);
                return true;
            }
            _ => {
                buffer.drain(..index);
                return true;
            }
        }
    }

    buffer.drain(..inspected);
    *discarded_tail_bytes = discarded_tail_bytes.saturating_add(inspected);
    *discarded_tail_bytes >= MAX_DISCARDED_CONTROL_TAIL_BYTES
}
/// Consume bounded mouse fields and their final, preserving unrelated bytes.
pub(super) fn discard_orphaned_sgr_mouse_tail(
    buffer: &mut Vec<u8>,
    discarded_tail_bytes: &mut usize,
) -> bool {
    let remaining = MAX_DISCARDED_CONTROL_TAIL_BYTES.saturating_sub(*discarded_tail_bytes);
    let inspected = buffer.len().min(remaining);

    for index in 0..inspected {
        match buffer[index] {
            b'0'..=b'9' | b';' => {}
            b'M' | b'm' => {
                buffer.drain(..=index);
                return true;
            }
            _ => {
                buffer.drain(..index);
                return true;
            }
        }
    }

    *discarded_tail_bytes = discarded_tail_bytes.saturating_add(inspected);
    if buffer.len() > inspected {
        buffer.clear();
        return true;
    }

    buffer.clear();
    false
}

/// Dark appearance report.
const DARK_REPORT: &[u8] = b"\x1b[?997;1n";
/// Light appearance report.
const LIGHT_REPORT: &[u8] = b"\x1b[?997;2n";

/// Decode an exact dark or light host appearance report.
#[must_use]
pub fn parse_host_color_scheme_report(buffer: &[u8]) -> Option<HostColorScheme> {
    match buffer {
        DARK_REPORT => Some(HostColorScheme::Dark),
        LIGHT_REPORT => Some(HostColorScheme::Light),
        _ => None,
    }
}

/// A host terminal's reported background appearance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostColorScheme {
    /// Dark background.
    Dark,
    /// Light background.
    Light,
}

/// Decode positive host cell dimensions in width, height order.
#[must_use]
pub fn host_cell_size(buffer: &[u8]) -> Option<(u32, u32)> {
    let body = buffer.strip_prefix(b"\x1b[")?.strip_suffix(b"t")?;
    let text = std::str::from_utf8(body).ok()?;
    let mut params = text.split(';');
    if params.next()? != "6" {
        return None;
    }
    let height: u32 = params.next()?.parse().ok()?;
    let width: u32 = params.next()?.parse().ok()?;
    (params.next().is_none() && width > 0 && height > 0).then_some((width, height))
}

/// Whether this is a supported default or palette colour reply.
pub(super) fn is_host_color_response(buffer: &[u8]) -> bool {
    std::str::from_utf8(buffer).is_ok_and(|text| {
        crate::terminal_theme::parse_default_color_response(text).is_some()
            || crate::terminal_theme::parse_palette_color_response(text).is_some()
    })
}
