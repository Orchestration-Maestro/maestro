//! Deferred placement records.

use super::ParseError;
/// Deferred image placement parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Display {
    /// Image id.
    pub image_id: u32,
    /// Image number.
    pub image_number: u32,
    /// Placement id.
    pub placement_id: u32,
    /// Source rectangle left edge in pixels.
    pub x: u32,
    /// Source rectangle top edge in pixels.
    pub y: u32,
    /// Source rectangle width in pixels.
    pub width: u32,
    /// Source rectangle height in pixels.
    pub height: u32,
    /// Horizontal offset within the first cell, in pixels.
    pub x_offset: u32,
    /// Vertical offset within the first cell, in pixels.
    pub y_offset: u32,
    /// Displayed width in terminal cells.
    pub columns: u32,
    /// Displayed height in terminal cells.
    pub rows: u32,
    /// Cursor movement.
    pub cursor_movement: CursorMovement,
    /// Virtual placement.
    pub virtual_placement: bool,
    /// Parent id.
    pub parent_id: u32,
    /// Parent placement id.
    pub parent_placement_id: u32,
    /// Signed horizontal displacement relative to the parent.
    pub horizontal_offset: i32,
    /// Signed vertical displacement relative to the parent.
    pub vertical_offset: i32,
    /// Signed placement layer.
    pub z: i32,
}

/// Cursor placement after display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CursorMovement {
    /// Move the cursor after the displayed image.
    #[default]
    After,
    /// Leave the cursor unchanged.
    None,
}

impl Display {
    /// Project the selected control fields into their typed record.
    pub(super) fn parse(keys: &super::parser::Keys) -> Result<Self, ParseError> {
        Ok(Self {
            image_id: keys.get(b'i', 0),
            image_number: keys.get(b'I', 0),
            placement_id: keys.get(b'p', 0),
            x: keys.get(b'x', 0),
            y: keys.get(b'y', 0),
            width: keys.get(b'w', 0),
            height: keys.get(b'h', 0),
            x_offset: keys.get(b'X', 0),
            y_offset: keys.get(b'Y', 0),
            columns: keys.get(b'c', 0),
            rows: keys.get(b'r', 0),
            parent_id: keys.get(b'P', 0),
            parent_placement_id: keys.get(b'Q', 0),
            horizontal_offset: keys.signed(b'H'),
            vertical_offset: keys.signed(b'V'),
            z: keys.signed(b'z'),
            cursor_movement: match keys.get(b'C', 0) {
                0 => CursorMovement::After,
                1 => CursorMovement::None,
                _ => return Err(ParseError::InvalidFormat),
            },
            virtual_placement: match keys.get(b'U', 0) {
                0 => false,
                1 => true,
                _ => return Err(ParseError::InvalidFormat),
            },
        })
    }
}
