//! The bytes of a frame: synchronized output, line resets and cursor motion.

use serde::Serialize;

use crate::images::terminal_image::{delete_kitty_image, is_image_line};
use crate::text::utils::{is_whitespace_scalar, normalize_terminal_output, visible_width};

use super::component::CURSOR_MARKER;
use super::screen::Changes;

/// Starts output the terminal presents all at once.
pub(super) const SYNC_BEGIN: &str = "\x1b[?2026h";
/// Ends output the terminal presents all at once.
pub(super) const SYNC_END: &str = "\x1b[?2026l";
/// Resets styles and closes any hyperlink so a line cannot leak either into the next.
pub(super) const SEGMENT_RESET: &str = "\x1b[0m\x1b]8;;\x07";

/// Starts a Kitty graphics sequence.
const KITTY_SEQUENCE_PREFIX: &str = "\x1b_G";

/// Where the hardware cursor belongs within a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(super) struct CursorPosition {
    /// Row within the frame.
    pub(super) row: usize,
    /// Cell column within the row.
    pub(super) col: usize,
}

/// Removes the cursor marker from the last visible row that holds one and reports where
/// it was.
pub(super) fn extract_cursor_position(
    lines: &mut [String],
    height: usize,
) -> Option<CursorPosition> {
    let viewport_top = lines.len().saturating_sub(height);
    lines
        .iter_mut()
        .enumerate()
        .skip(viewport_top)
        .rev()
        .find_map(|(row, line)| {
            let index = line.find(CURSOR_MARKER)?;
            let col = visible_width(&line[..index]);
            line.replace_range(index..index + CURSOR_MARKER.len(), "");
            Some(CursorPosition { row, col })
        })
}

/// Ends every text line with a style and hyperlink reset after normalizing it; image
/// lines stay as they are.
pub(super) fn apply_line_resets(lines: &mut [String]) {
    for line in lines.iter_mut().filter(|line| !is_image_line(line)) {
        *line = format!("{}{SEGMENT_RESET}", normalize_terminal_output(line));
    }
}

/// The escape that moves the cursor `delta` rows: down when positive, up when negative.
pub(super) fn move_rows(delta: isize) -> String {
    match delta.cmp(&0) {
        std::cmp::Ordering::Greater => format!("\x1b[{delta}B"),
        std::cmp::Ordering::Less => format!("\x1b[{}A", delta.unsigned_abs()),
        std::cmp::Ordering::Equal => String::new(),
    }
}

/// Signed distance from row `current` to row `target`.
pub(super) fn row_delta(current: usize, target: usize) -> isize {
    let distance = isize::try_from(current.abs_diff(target)).unwrap_or(isize::MAX);
    if target >= current {
        distance
    } else {
        -distance
    }
}

/// The synchronized output that draws every line from the top of the screen. When
/// `cleared_images` is present the images it names are deleted, then the screen and
/// scrollback are cleared.
pub(super) fn full_frame(lines: &[String], cleared_images: Option<&[u32]>) -> String {
    let erase = cleared_images.map_or_else(String::new, |images| {
        format!("{}\x1b[2J\x1b[H\x1b[3J", image_deletions(images))
    });
    format!("{SYNC_BEGIN}{erase}{}{SYNC_END}", lines.join("\r\n"))
}

/// The id of the first Kitty graphics sequence on `line`, when its header carries a valid
/// one. Only the first sequence is read, and the first valid `i` parameter wins.
pub(super) fn kitty_image_id(line: &str) -> Option<u32> {
    let start = line.find(KITTY_SEQUENCE_PREFIX)? + KITTY_SEQUENCE_PREFIX.len();
    let header = &line[start..];
    let params = &header[..header.find(';')?];
    params.split(',').find_map(|param| {
        let mut parts = param.split('=');
        match (parts.next()?, parts.next()?) {
            ("i", value) => numeric_image_id(value),
            _ => None,
        }
    })
}

/// Reads a header value as an unsigned 32-bit id: a decimal or exponent literal, or an
/// unsigned `0x`, `0b` or `0o` literal, after trimming ECMAScript whitespace.
fn numeric_image_id(value: &str) -> Option<u32> {
    let text = value.trim_matches(is_whitespace_scalar);
    let id = match text.get(..2) {
        Some("0x" | "0X") => radix_id(&text[2..], 16)?,
        Some("0b" | "0B") => radix_id(&text[2..], 2)?,
        Some("0o" | "0O") => radix_id(&text[2..], 8)?,
        _ => decimal_id(text)?,
    };
    (id > 0).then_some(id)
}

/// Reads unsigned digits of `radix`; a sign, separator or empty text is not a number.
fn radix_id(digits: &str, radix: u32) -> Option<u32> {
    if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(radix)) {
        return None;
    }
    u32::from_str_radix(digits, radix).ok()
}

/// Reads a decimal literal, with fraction and exponent, whose value is a whole number
/// that fits 32 bits.
fn decimal_id(text: &str) -> Option<u32> {
    let number: f64 = text.parse().ok()?;
    let whole = number.total_cmp(&number.trunc()).is_eq();
    // A whole number below 2^32 prints as plain digits, which parse exactly.
    (whole && (1.0..=f64::from(u32::MAX)).contains(&number))
        .then(|| number.to_string().parse().ok())?
}

/// The ids of the Kitty images on `lines`, each once, in order of first appearance.
pub(super) fn image_ids(lines: &[String]) -> Vec<u32> {
    let mut ids = Vec::new();
    for id in lines.iter().filter_map(|line| kitty_image_id(line)) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// The escapes that delete the images `ids`, in order.
pub(super) fn image_deletions(ids: &[u32]) -> String {
    ids.iter().map(|id| delete_kitty_image(*id)).collect()
}

/// A differential update and where it started.
pub(super) struct Update {
    /// The synchronized output to write.
    pub(super) buffer: String,
    /// Row the terminal cursor was on when the update began.
    pub(super) hardware_row: usize,
    /// Rows moved to reach the first changed row.
    pub(super) line_diff: isize,
    /// Last row drawn.
    pub(super) render_end: usize,
}

/// The synchronized output that redraws rows `changes.first..=changes.last` in place and
/// erases rows the new frame no longer has. The cursor starts on row `hardware_row`.
///
/// The first changed row is always on screen: a retained frame ends on or above the last
/// screen row, so no scrolling is needed to reach it.
pub(super) fn differential(
    previous: &[String],
    lines: &[String],
    changes: &Changes,
    hardware_row: usize,
) -> Update {
    let mut buffer = String::from(SYNC_BEGIN);
    buffer.push_str(&changed_image_deletions(previous, changes));
    let move_target = changes.first - usize::from(changes.append_start);
    let line_diff = row_delta(hardware_row, move_target);
    buffer.push_str(&move_rows(line_diff));
    buffer.push_str(if changes.append_start { "\r\n" } else { "\r" });
    let render_end = changes.last.min(lines.len() - 1);
    for (offset, line) in lines[changes.first..=render_end].iter().enumerate() {
        if offset > 0 {
            buffer.push_str("\r\n");
        }
        buffer.push_str("\x1b[2K");
        buffer.push_str(line);
    }
    if previous.len() > lines.len() {
        let extra = previous.len() - lines.len();
        buffer.push_str(&"\r\n\x1b[2K".repeat(extra));
        buffer.push_str(&move_rows(-row_delta(0, extra)));
    }
    buffer.push_str(SYNC_END);
    Update {
        buffer,
        hardware_row,
        line_diff,
        render_end,
    }
}

/// The synchronized output that erases the rows a shorter frame of `new_len` lines no
/// longer has, then returns the cursor to the last row of the new content.
///
/// The erased rows start one row below that last row, or on row zero when the new frame
/// is empty and has no content row to keep.
pub(super) fn delete_tail(
    previous: &[String],
    new_len: usize,
    changes: &Changes,
    hardware_row: usize,
) -> String {
    let extra = previous.len() - new_len;
    let skipped = usize::from(new_len > 0);
    let motion = move_rows(row_delta(hardware_row, new_len.saturating_sub(1)));
    let rows = vec!["\r\x1b[2K"; extra].join("\x1b[1B");
    let back = move_rows(-row_delta(0, extra + skipped - 1));
    format!(
        "{SYNC_BEGIN}{}{motion}\r{}{rows}{back}{SYNC_END}",
        changed_image_deletions(previous, changes),
        "\x1b[1B".repeat(skipped)
    )
}

/// The escapes that delete the images on the retained rows `changes` covers.
fn changed_image_deletions(previous: &[String], changes: &Changes) -> String {
    let end = changes.last.min(previous.len().saturating_sub(1));
    let rows = previous.get(changes.first..=end).unwrap_or_default();
    image_deletions(&image_ids(rows))
}
