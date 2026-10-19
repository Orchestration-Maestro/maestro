//! Typed control-sequence parameters.

/// Tab-stop clearing operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabClear {
    /// Clear the destination at the current column.
    Current,
    /// Clear all retained destinations.
    All,
    /// Unsupported parameter, ignored by the terminal.
    Unknown(u8),
}

/// Direction of line erasure relative to the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EraseLine {
    /// Cursor through the right edge.
    Right,
    /// Left edge through the cursor.
    Left,
    /// Entire row, retaining soft-wrap metadata.
    Complete,
    /// Right erase only when the cursor has no pending wrap.
    RightUnlessPendingWrap,
    /// Unsupported parameter, leaving all state unchanged.
    Unknown(u16),
}

/// Region of display erasure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EraseDisplay {
    /// Cursor through the bottom of the active area.
    Below,
    /// Top of the active area through the cursor.
    Above,
    /// Entire active area, preserving a trailing prompt in history.
    Complete,
    /// Retained history only.
    Scrollback,
    /// Move nonempty active rows into history.
    ScrollComplete,
}
