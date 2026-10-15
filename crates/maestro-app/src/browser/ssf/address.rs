//! Zero-based spreadsheet positions and A1 spelling.
use super::WorkbookError;
/// Zero-based cell position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellAddress {
    /// Row index.
    pub r: u32,
    /// Column index.
    pub c: u32,
}
/// Inclusive cell rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Starting cell.
    pub s: CellAddress,
    /// Ending cell.
    pub e: CellAddress,
}
/// Render a zero-based row.
#[must_use]
pub fn encode_row(row: u32) -> String {
    (u64::from(row) + 1).to_string()
}
/// Render a zero-based column.
#[must_use]
pub fn encode_col(column: u32) -> String {
    let mut value = u64::from(column) + 1;
    let mut text = String::new();
    while value > 0 {
        let digit = u8::try_from((value - 1) % 26).unwrap_or(0);
        text.insert(0, char::from(b'A' + digit));
        value = (value - 1) / 26;
    }
    text
}
/// Render an A1 cell address.
#[must_use]
pub fn encode_cell(cell: CellAddress) -> String {
    format!("{}{}", encode_col(cell.c), encode_row(cell.r))
}
/// Decode an A1 cell, accepting absolute markers.
///
/// # Errors
/// Returns an address error for invalid spelling or out-of-range indices.
pub fn decode_cell(value: &str) -> Result<CellAddress, WorkbookError> {
    let spelling = value.replace('$', "");
    let boundary = spelling.bytes().take_while(u8::is_ascii_uppercase).count();
    let (letters, digits) = spelling.split_at(boundary);
    let column = letters.bytes().try_fold(0_u32, |n, c| {
        n.checked_mul(26)?.checked_add(u32::from(c - b'A') + 1)
    });
    let row = digits.parse::<u32>().ok().and_then(|n| n.checked_sub(1));
    match (column.and_then(|n| n.checked_sub(1)), row) {
        (Some(c), Some(r)) if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) => {
            Ok(CellAddress { r, c })
        }
        _ => Err(WorkbookError::Address(value.to_owned())),
    }
}
/// Render a rectangle, compacting identical endpoints.
#[must_use]
pub fn encode_range(range: Range) -> String {
    if range.s == range.e {
        encode_cell(range.s)
    } else {
        format!("{}:{}", encode_cell(range.s), encode_cell(range.e))
    }
}
/// Decode an inclusive A1 rectangle or singleton.
///
/// # Errors
/// Returns the first invalid endpoint's address failure.
pub fn decode_range(value: &str) -> Result<Range, WorkbookError> {
    let (start, end) = value.split_once(':').unwrap_or((value, value));
    Ok(Range {
        s: decode_cell(start)?,
        e: decode_cell(end)?,
    })
}
