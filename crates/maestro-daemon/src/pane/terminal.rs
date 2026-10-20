//! Retained pane terminal records.

/// Scroll position within retained terminal rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollMetrics {
    /// Rows between the viewport bottom and the live bottom.
    pub offset_from_bottom: usize,
    /// Greatest available bottom-relative offset.
    pub max_offset_from_bottom: usize,
    /// Rows in the viewport.
    pub viewport_rows: usize,
}

/// Cursor position and presentation within a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCursorState {
    /// Viewport column.
    pub x: u16,
    /// Viewport row.
    pub y: u16,
    /// Whether to draw the cursor.
    pub visible: bool,
    /// DECSCUSR parameter, with zero selecting the terminal default.
    pub shape: u8,
}
