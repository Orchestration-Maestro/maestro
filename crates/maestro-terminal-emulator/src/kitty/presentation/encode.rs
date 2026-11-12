//! Kitty graphics commands the host terminal receives.
//!
//! Transmit commands write their control keys after `a=` and carry the payload in
//! base64 chunks of raw payload bytes: the first chunk holds the keys and its
//! `m=`, every later chunk holds only its `m=`. Display and delete commands hold
//! control keys and no payload. The regular-file command sends a base64 path as
//! one unchunked payload under the control keys its caller supplies.

use std::fmt::Write as _;
use std::io::Write as _;

use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{
    ClippedPlacement, HostPlacement,
    clip::clipped_placement,
    identity::{host_placement_id, kitty_format_code},
};

/// Raw payload bytes carried by one Kitty transmission chunk.
pub(super) const KITTY_CHUNK_BYTES: usize = 3072;

/// Command that transmits and displays a file without sending its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectFileCommand {
    /// Cursor movement written before the command.
    pub leading: Vec<u8>,
    /// Control keys, without the payload field.
    pub control: String,
}

/// Transmit-and-display command for a placement's final visible position.
#[must_use]
pub fn direct_file_command(
    placement: &HostPlacement,
    host_image_id: u32,
) -> Option<DirectFileCommand> {
    let (clipped, format_code) = clipped_placement(placement)?;
    let placement_id = host_placement_id(&placement.source_key, &placement.placement);
    let mut control = format!(
        "a=T,f={format_code},s={},v={},i={host_image_id},p={placement_id},c={},r={},z={},C=1,q=0",
        placement.placement.image_width,
        placement.placement.image_height,
        clipped.cols,
        clipped.rows,
        placement.placement.z,
    );
    append_placement_controls(&mut control, clipped);
    Some(DirectFileCommand {
        leading: format!("\x1b[{};{}H", clipped.y + 1, clipped.x + 1).into_bytes(),
        control,
    })
}

/// Regular-file command that loads the image the host reads from a path.
#[cfg(unix)]
pub fn encode_kitty_regular_file(out: &mut Vec<u8>, leading: &[u8], control: &str, path: &str) {
    let payload = STANDARD.encode(path.as_bytes());
    out.extend_from_slice(b"\x1b7");
    out.extend_from_slice(leading);
    let _ = write!(out, "\x1b_G{control},t=f;{payload}\x1b\\");
    out.extend_from_slice(b"\x1b8");
}

/// Command that drops an uploaded image and its placements.
pub(super) fn encode_delete_image(out: &mut Vec<u8>, id: u32) {
    let _ = write!(out, "\x1b_Ga=d,d=I,i={id},q=2;\x1b\\");
}

/// Command that drops one placement and keeps its image.
pub(super) fn encode_delete_placement(out: &mut Vec<u8>, host_id: u32, placement_id: u32) {
    let _ = write!(out, "\x1b_Ga=d,d=i,i={host_id},p={placement_id},q=2;\x1b\\");
}

/// Upload command for a placement's pixels, or false when it carries none.
pub(super) fn encode_upload_image(
    out: &mut Vec<u8>,
    placement: &HostPlacement,
    format_code: u32,
    host_id: u32,
) -> bool {
    if placement.placement.data.is_empty() {
        return false;
    }
    let control = format!(
        "a=t,t=d,f={format_code},s={},v={},i={host_id},q=2",
        placement.placement.image_width, placement.placement.image_height,
    );
    encode_kitty_data(out, &control, &placement.placement.data);
    true
}

/// One command that transmits pixels and displays them, or false without pixels.
pub(super) fn encode_transmit_and_display(
    out: &mut Vec<u8>,
    placement: &HostPlacement,
    clipped: ClippedPlacement,
    host_id: u32,
    placement_id: u32,
) -> bool {
    if placement.placement.data.is_empty() {
        return false;
    }
    let format_code = kitty_format_code(placement.placement.format);
    let _ = write!(out, "\x1b[{};{}H", clipped.y + 1, clipped.x + 1);
    let mut control = format!(
        "a=T,t=d,f={format_code},s={},v={},i={host_id},p={placement_id},c={},r={},z={},C=1,q=2",
        placement.placement.image_width,
        placement.placement.image_height,
        clipped.cols,
        clipped.rows,
        placement.placement.z,
    );
    append_placement_controls(&mut control, clipped);
    encode_kitty_data(out, &control, &placement.placement.data);
    true
}

/// Command that displays an image the host already holds.
pub(super) fn encode_display_placement(
    out: &mut Vec<u8>,
    clipped: ClippedPlacement,
    host_id: u32,
    placement_id: u32,
    z: i32,
) {
    let _ = write!(out, "\x1b[{};{}H", clipped.y + 1, clipped.x + 1);
    let mut control = format!(
        "a=p,i={host_id},p={placement_id},c={},r={},z={z},C=1,q=2",
        clipped.cols, clipped.rows,
    );
    append_placement_controls(&mut control, clipped);
    let _ = write!(out, "\x1b_G{control};\x1b\\");
}

/// Append the optional source and offset keys, omitting zero values.
fn append_placement_controls(control: &mut String, clipped: ClippedPlacement) {
    if clipped.source_x > 0 {
        let _ = write!(control, ",x={}", clipped.source_x);
    }
    if clipped.source_y > 0 {
        let _ = write!(control, ",y={}", clipped.source_y);
    }
    if clipped.source_width > 0 {
        let _ = write!(control, ",w={}", clipped.source_width);
    }
    if clipped.source_height > 0 {
        let _ = write!(control, ",h={}", clipped.source_height);
    }
    if clipped.x_offset > 0 {
        let _ = write!(control, ",X={}", clipped.x_offset);
    }
    if clipped.y_offset > 0 {
        let _ = write!(control, ",Y={}", clipped.y_offset);
    }
}

/// Chunked base64 payload, with the continuation flag on every chunk but the last.
fn encode_kitty_data(out: &mut Vec<u8>, control: &str, data: &[u8]) {
    let mut chunks = data.chunks(KITTY_CHUNK_BYTES).peekable();
    let Some(first) = chunks.next() else {
        return;
    };
    let more = u8::from(chunks.peek().is_some());
    let _ = write!(
        out,
        "\x1b_G{control},m={more};{}\x1b\\",
        STANDARD.encode(first)
    );
    while let Some(chunk) = chunks.next() {
        let more = u8::from(chunks.peek().is_some());
        let _ = write!(out, "\x1b_Gm={more};{}\x1b\\", STANDARD.encode(chunk));
    }
}
