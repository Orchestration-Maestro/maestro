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
