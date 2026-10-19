//! Region-relative terminal coordinates.

/// Origin used to interpret a terminal coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// Editable screen area.
    Active,
    /// Visible area, including any scrollback offset.
    Viewport,
    /// Entire retained screen.
    Screen,
    /// Rows preceding the editable area.
    History,
}

/// Column and row relative to a selected terminal region.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Coordinate {
    /// Zero-based column.
    pub x: u16,
    /// Zero-based row, including long scrollback.
    pub y: u32,
}

/// A coordinate with its terminal origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    /// Coordinate relative to the editable area.
    Active(Coordinate),
    /// Coordinate relative to the visible area.
    Viewport(Coordinate),
    /// Coordinate relative to the oldest retained row.
    Screen(Coordinate),
    /// Coordinate relative to the oldest historical row.
    History(Coordinate),
}

impl Point {
    /// Identify the coordinate's origin for internal region traversal.
    pub(crate) fn tag(self) -> Tag {
        match self {
            Self::Active(_) => Tag::Active,
            Self::Viewport(_) => Tag::Viewport,
            Self::Screen(_) => Tag::Screen,
            Self::History(_) => Tag::History,
        }
    }

    /// Return the coordinate independently of its origin.
    #[must_use]
    pub fn coord(self) -> Coordinate {
        match self {
            Self::Active(value)
            | Self::Viewport(value)
            | Self::Screen(value)
            | Self::History(value) => value,
        }
    }
}

impl Coordinate {
    /// Compare both coordinate components.
    #[must_use]
    pub fn eql(self, other: Self) -> bool {
        self == other
    }
}
